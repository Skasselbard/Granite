use crate::petri_net::function::{Local, LocalKey, VirtualMemory};
use pnml;
use pnml::{NodeRef, PageRef, PetriNet, Result};
use rustc::mir;
use std::clone::Clone;
use std::collections::HashMap;

//    .-----.
// .-( start )------------------------------------------.
// |  '-----'            BasicBlock                     |
// |     |                                              |
// |     v                .-----------------------.     |
// |   .---.            .-----.  Statements       |     |
// |   |   |---------->( start )                  |     |
// |   '---'            '-----'                   |     |
// |     |                |                       |     |
// |     v                |                       |     |
// | .-------.            |                       |     |
// |( working )           |                       |     |
// | '-------'            |                       |     |
// |     |                |                       |     |
// |     v                |                       |     |
// |   .---.             .---.                    |     |
// |   |   |<-----------( end )                   |     |
// |   '---'             '---'                    |     |
// |     |                '-----------------------'     |
// |     v                                              |
// | .------.                                           |
// |( choice )----------------------------------.       |
// | '------'        |       |       |          |       |
// |     -.          |       |       |          |       |
// |.-----|----------|-------|-------|----------|-----. |
// ||     |          |  Terminator   |          |     | |
// ||     |          |       |       |          |     | |
// ||     v          v       v       v          v     | |
// ||  .-----.      .-.     .-.     .-.      .-----.  | |
// ''-( end_1 )----(   )---(   )---(   )----( end_N )-'-'
//     '-----'      '-'     '-'     '-'      '-----'
#[derive(Debug)]
pub struct BasicBlock {
    page: pnml::PageRef,
    start_place: NodeRef,
    end_place: NodeRef,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct Statement {
    start_place: NodeRef,
    stmt_transition: NodeRef,
    end_place: NodeRef,
}

impl BasicBlock {
    pub fn new<'net>(
        net: &'net mut PetriNet,
        page: &PageRef,
        start_place: &NodeRef,
    ) -> Result<Self> {
        let page = page.clone();
        let start_place = start_place.clone();
        let end_place = net.add_place(&page)?;
        // TODO: add flow
        let statements = Vec::new();
        //TODO: add statements
        Ok(BasicBlock {
            page,
            start_place,
            end_place,
            statements,
        })
    }

    pub fn add_statement<'net>(
        &mut self,
        net: &'net mut PetriNet,
        statement: &mir::Statement<'_>,
        virt_memory: &VirtualMemory,
    ) -> Result<()> {
        // if its the first statement, it shares the first place with the basic block
        // otherwise the and place of the last statement is the start place of the new one
        let start_place = {
            if let Some(statement) = self.statements.last() {
                statement.end_place().clone()
            } else {
                self.start_place().clone()
            }
        };

        self.statements.push(Statement::new(
            net,
            &self.page,
            &start_place,
            statement,
            virt_memory,
        )?);
        Ok(())
    }
    pub fn start_place(&self) -> &NodeRef {
        &self.start_place
    }
    pub fn end_place(&self) -> &NodeRef {
        &self.end_place
    }
}

impl Statement {
    pub fn new<'net>(
        net: &'net mut PetriNet,
        page: &PageRef,
        start_place: &NodeRef,
        statement: &mir::Statement<'_>,
        virt_memory: &VirtualMemory,
    ) -> Result<Self> {
        let end_place = net.add_place(page)?;
        // the statement transition is its important part
        // it "executes" the effect of the statement
        let stmt_transition = net.add_transition(page)?;
        //stmt_transition.name(net, "");
        net.add_arc(page, &start_place, &stmt_transition)?;
        net.add_arc(page, &stmt_transition, &end_place)?;
        let stmt = Statement {
            start_place: start_place.clone(),
            stmt_transition,
            end_place,
        };
        stmt.build(net, page, statement, virt_memory)?;
        Ok(stmt)
    }
    pub fn start_place(&self) -> &NodeRef {
        &self.start_place
    }
    pub fn end_place(&self) -> &NodeRef {
        &self.end_place
    }

    fn build<'net>(
        &self,
        net: &'net mut PetriNet,
        page: &PageRef,
        statement: &mir::Statement<'_>,
        virt_memory: &VirtualMemory,
    ) -> Result<()> {
        use mir::StatementKind;
        match &statement.kind {
            StatementKind::Assign(lvalue, rvalue) => {
                self.build_assign(net, page, virt_memory, lvalue, rvalue.as_ref())?
            }
            StatementKind::StorageLive(local) => {
                let local = virt_memory.locals.get(&local).expect("local not found");
                net.add_arc(page, &local.prenatal_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &local.live_place)?;
            }
            StatementKind::StorageDead(local) => {
                let local = virt_memory.locals.get(&local).expect("local not found");
                net.add_arc(page, &local.live_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &local.dead_place)?;
            }
            StatementKind::FakeRead(_, _)
            | StatementKind::SetDiscriminant { .. }
            | StatementKind::InlineAsm(_)
            | StatementKind::Retag(_, _)
            | StatementKind::AscribeUserType(_, _, _) => panic!("statementKind not supported"),
            StatementKind::Nop => {}
        }
        Ok(())
    }

    fn build_assign<'net>(
        &self,
        net: &'net mut PetriNet,
        page: &PageRef,
        virt_memory: &VirtualMemory,
        lvalue: &mir::Place<'_>,
        rvalue: &mir::Rvalue<'_>,
    ) -> Result<()> {
        use mir::Rvalue;
        const nolocal: &str = "Unable to get local";
        let llocal = virt_memory
            .locals
            .get(&place_to_local(lvalue))
            .expect(nolocal);
        match rvalue {
            Rvalue::Use(ref operand)
            | Rvalue::Repeat(ref operand, _)
            | Rvalue::UnaryOp(_, ref operand) => {
                let op_place = match op_to_local(operand) {
                    LocalKey::MirLocal(local) => {
                        &virt_memory.locals.get(&local).expect(nolocal).live_place
                    }
                    LocalKey::Constant => &virt_memory.constants,
                };
                net.add_arc(page, &op_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &op_place)?;
                net.add_arc(page, &self.stmt_transition, &llocal.live_place)?;
            }
            Rvalue::Ref(_, _, ref place) | Rvalue::Len(ref place) => {
                let place_local = virt_memory
                    .locals
                    .get(&place_to_local(place))
                    .expect(nolocal);
                net.add_arc(page, &place_local.live_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &place_local.live_place)?;
                net.add_arc(page, &self.stmt_transition, &llocal.live_place)?;
            }
            Rvalue::Cast(ref _kind, ref _operand, ref _typ) => {}
            Rvalue::BinaryOp(ref _operator, ref loperand, ref roperand)
            | Rvalue::CheckedBinaryOp(ref _operator, ref loperand, ref roperand) => {
                let lop_place = match op_to_local(loperand) {
                    LocalKey::MirLocal(local) => {
                        &virt_memory.locals.get(&local).expect(nolocal).live_place
                    }
                    LocalKey::Constant => &virt_memory.constants,
                };
                let rop_place = match op_to_local(roperand) {
                    LocalKey::MirLocal(local) => {
                        &virt_memory.locals.get(&local).expect(nolocal).live_place
                    }
                    LocalKey::Constant => &virt_memory.constants,
                };
                net.add_arc(page, &lop_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &lop_place)?;
                net.add_arc(page, &rop_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &rop_place)?;
                net.add_arc(page, &self.stmt_transition, &llocal.live_place)?;
            }
            Rvalue::NullaryOp(ref operator, ref _typ) => match operator {
                // these are essentially a lookup of the type size in the static space
                mir::NullOp::SizeOf | mir::NullOp::Box => {
                    net.add_arc(page, &virt_memory.constants, &self.stmt_transition)?;
                    net.add_arc(page, &self.stmt_transition, &virt_memory.constants)?;
                }
            },
            Rvalue::Discriminant(ref _place) => panic!("discriminant"),
            Rvalue::Aggregate(ref kind, ref operands) => panic!("aggregate"),
        }
        Ok(())
    }
}

fn op_to_local(operand: &mir::Operand<'_>) -> LocalKey {
    match operand {
        mir::Operand::Copy(place) | mir::Operand::Move(place) => {
            LocalKey::MirLocal(place_to_local(place))
        }
        // Constants are always valid reads
        // until using a high level petri net the value is not important and can be ignored
        // The static space can be seen as one petri net place that is accessed
        mir::Operand::Constant(_) => LocalKey::Constant,
    }
}

fn place_to_local(place: &mir::Place<'_>) -> mir::Local {
    place
        .local_or_deref_local()
        //FIXME: is it valid to just use the outermost local if nothing better was found?
        .or_else(|| match &place.base {
            mir::PlaceBase::Local(local) => Some(local.clone()),
            mir::PlaceBase::Static(_statik) => panic!("cannot convert static to local"),
        })
        .expect("cannot convert place to local")
}

use crate::petri_net::function::{Data, Local, LocalKey, VirtualMemory};
use pnml;
use pnml::{NodeRef, PageRef, PetriNet, Result};
use rustc::mir;
use std::clone::Clone;
use std::collections::HashMap;

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
        let statements = Vec::new();
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
            StatementKind::Assign(box (lvalue, rvalue)) => {
                self.build_assign(net, page, virt_memory, lvalue, rvalue)?
            }
            StatementKind::StorageLive(local) => {
                let local = virt_memory.get_local(&local).expect("local not found");
                net.add_arc(page, &local.prenatal_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &local.live_place)?;
            }
            StatementKind::StorageDead(local) => {
                let local = virt_memory.get_local(&local).expect("local not found");
                net.add_arc(page, &local.live_place, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, &local.dead_place)?;
            }
            StatementKind::SetDiscriminant { place, .. } => {
                let place_node = place_to_data_node(place, virt_memory);
                net.add_arc(page, place_node, &self.stmt_transition)?;
                net.add_arc(page, &self.stmt_transition, place_node)?;
            }
            StatementKind::FakeRead(_, _)
            | StatementKind::InlineAsm(_)
            | StatementKind::Retag(_, _)
            | StatementKind::AscribeUserType(box (_, _), _) => {
                panic!("statementKind not supported: {:?}", statement.kind)
            }
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
        let llocal = place_to_data_node(lvalue, virt_memory);
        add_node_to_statement(net, page, llocal, &self.stmt_transition, virt_memory)?;
        match rvalue {
            Rvalue::Use(ref operand)
            | Rvalue::Repeat(ref operand, _)
            | Rvalue::UnaryOp(_, ref operand) => {
                let op_place = op_to_data_node(operand, virt_memory);
                add_node_to_statement(net, page, op_place, &self.stmt_transition, virt_memory)?;
            }
            Rvalue::Ref(_, _, ref place) | Rvalue::Len(ref place) => {
                let place_local = place_to_data_node(place, virt_memory);
                add_node_to_statement(net, page, place_local, &self.stmt_transition, virt_memory)?;
            }
            Rvalue::Cast(ref _kind, ref operand, ref _typ) => {
                let op_place = op_to_data_node(operand, virt_memory);
                add_node_to_statement(net, page, op_place, &self.stmt_transition, virt_memory)?;
            }
            Rvalue::BinaryOp(ref _operator, ref loperand, ref roperand)
            | Rvalue::CheckedBinaryOp(ref _operator, ref loperand, ref roperand) => {
                let lop_place = op_to_data_node(loperand, virt_memory);
                let rop_place = op_to_data_node(roperand, virt_memory);
                add_node_to_statement(net, page, lop_place, &self.stmt_transition, virt_memory)?;
                add_node_to_statement(net, page, rop_place, &self.stmt_transition, virt_memory)?;
            }
            Rvalue::NullaryOp(ref operator, ref _typ) => match operator {
                // these are essentially a lookup of the type size in the static space
                mir::NullOp::SizeOf | mir::NullOp::Box => {
                    net.add_arc(page, &virt_memory.get_constant(), &self.stmt_transition)?;
                    net.add_arc(page, &self.stmt_transition, &virt_memory.get_constant())?;
                }
            },
            Rvalue::Discriminant(ref place) => {
                let op_place = place_to_data_node(place, virt_memory);
                add_node_to_statement(net, page, op_place, &self.stmt_transition, virt_memory)?;
            }
            Rvalue::Aggregate(ref _kind, ref operands) => {
                //FIXME: does the kind matter?
                for operand in operands {
                    let op_place = op_to_data_node(operand, virt_memory);
                    add_node_to_statement(net, page, op_place, &self.stmt_transition, virt_memory)?;
                }
            }
        }
        Ok(())
    }
}

fn add_node_to_statement(
    net: &mut PetriNet,
    page: &PageRef,
    place_node: &NodeRef,
    statement_transition: &NodeRef,
    memory: &VirtualMemory,
) -> Result<()> {
    net.add_arc(page, &place_node, statement_transition)?;
    net.add_arc(page, statement_transition, &place_node)?;
    Ok(())
}

fn op_to_data_node<'a>(operand: &mir::Operand<'_>, memory: &'a VirtualMemory) -> &'a NodeRef {
    match operand {
        mir::Operand::Copy(place) | mir::Operand::Move(place) => place_to_data_node(place, memory),
        // Constants are always valid reads
        // until using a high level petri net the value is not important and can be ignored
        // Constants can be seen as one petri net place that is accessed
        mir::Operand::Constant(_) => memory.get_constant(),
    }
}

fn place_to_data_node<'a>(place: &mir::Place<'_>, memory: &'a VirtualMemory) -> &'a NodeRef {
    let local = place.local_or_deref_local();
    match local {
        Some(local) => {
            &memory
                .get_local(&local)
                .expect("local not found")
                .live_place
        }
        //FIXME: is it valid to just use the outermost local if nothing better was found?
        // maybe this functions helps?
        // https://doc.rust-lang.org/nightly/nightly-rustc/rustc/ty/context/struct.TyCtxt.html#method.intern_place_elems
        // https://doc.rust-lang.org/nightly/nightly-rustc/rustc/ty/context/struct.TyCtxt.html#method.mk_place_elems
        None => match &place.base {
            mir::PlaceBase::Local(local) => {
                &memory.get_local(local).expect("local not found").live_place
            }
            // https://doc.rust-lang.org/nightly/nightly-rustc/rustc/ty/context/struct.TyCtxt.html#method.promoted_mir
            mir::PlaceBase::Static(statik) => match statik.kind {
                mir::StaticKind::Static => panic!("staticKind::Static -> cannot convert"),
                mir::StaticKind::Promoted(promoted, _) => &memory
                    .get_static(&promoted)
                    .expect("promoted statik not found"),
            },
        },
    }
}

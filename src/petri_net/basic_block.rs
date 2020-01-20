use crate::petri_net::function::{op_to_data_node, place_to_data_node, VirtualMemory};
use petri_to_star::{NodeRef, PetriNet, Result};
use rustc::mir;
use std::clone::Clone;

#[derive(Debug)]
pub struct BasicBlock {
    start_place: NodeRef,
    end_place: NodeRef,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct Statement {
    start_place: NodeRef,
    stmt_transition: NodeRef,
}

impl BasicBlock {
    pub fn new<'net>(net: &'net mut PetriNet, start_place: NodeRef) -> Result<Self> {
        let end_place = net.add_place();
        let statements = Vec::new();
        Ok(BasicBlock {
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
        // otherwise the end place of the last statement is the start place of the new one
        let start_place = {
            if let Some(statement) = self.statements.last() {
                let place = net.add_place();
                net.add_arc(statement.stmt_transition, place)?;
                place
            } else {
                self.start_place()
            }
        };

        self.statements
            .push(Statement::new(net, start_place, statement, virt_memory)?);
        Ok(())
    }

    pub fn finish_statement_block(&self, net: &mut PetriNet) -> Result<()> {
        if let Some(statement) = self.statements.last() {
            net.add_arc(statement.stmt_transition, self.end_place)?;
        } else {
            // if there is only a terminator (no statement) we have to connect start and end place of the block
            let t = net.add_transition();
            t.name(net, "NOP".into())?;
            net.add_arc(self.start_place, t)?;
            net.add_arc(t, self.end_place)?;
        }
        Ok(())
    }

    pub fn start_place(&self) -> NodeRef {
        self.start_place
    }
    pub fn end_place(&self) -> NodeRef {
        self.end_place
    }
}

impl Statement {
    pub fn new<'net>(
        net: &'net mut PetriNet,
        start_place: NodeRef,
        statement: &mir::Statement<'_>,
        virt_memory: &VirtualMemory,
    ) -> Result<Self> {
        // the statement transition is its important part
        // it "executes" the effect of the statement
        let stmt_transition = net.add_transition();
        stmt_transition.name(net, format!("{:?}", statement.kind))?;
        //stmt_transition.name(net, "");
        net.add_arc(start_place, stmt_transition)?;
        let stmt = Statement {
            start_place: start_place.clone(),
            stmt_transition,
        };
        stmt.build(net, statement, virt_memory)?;
        Ok(stmt)
    }
    pub fn start_place(&self) -> &NodeRef {
        &self.start_place
    }

    fn build<'net>(
        &self,
        net: &'net mut PetriNet,
        statement: &mir::Statement<'_>,
        virt_memory: &VirtualMemory,
    ) -> Result<()> {
        use mir::StatementKind;
        match &statement.kind {
            StatementKind::Assign(box (lvalue, rvalue)) => {
                self.build_assign(net, virt_memory, lvalue, rvalue)?
            }
            StatementKind::StorageLive(local) => {
                let local = virt_memory.get_local(&local).expect("local not found");
                net.add_arc(
                    local.prenatal_place.expect("no uninitialized place"),
                    self.stmt_transition,
                )?;
                net.add_arc(self.stmt_transition, local.live_place)?;
            }
            StatementKind::StorageDead(local) => {
                let local = virt_memory.get_local(&local).expect("local not found");
                net.add_arc(local.live_place, self.stmt_transition)?;
                net.add_arc(
                    self.stmt_transition,
                    local.dead_place.expect("no dead place"),
                )?;
            }
            StatementKind::SetDiscriminant { place, .. } => {
                let place_node = place_to_data_node(place, virt_memory);
                net.add_arc(place_node, self.stmt_transition)?;
                net.add_arc(self.stmt_transition, place_node)?;
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
        virt_memory: &VirtualMemory,
        lvalue: &mir::Place<'_>,
        rvalue: &mir::Rvalue<'_>,
    ) -> Result<()> {
        use mir::Rvalue;
        let llocal = place_to_data_node(lvalue, virt_memory);
        add_node_to_statement(net, llocal, self.stmt_transition)?;
        match rvalue {
            Rvalue::Use(ref operand)
            | Rvalue::Repeat(ref operand, _)
            | Rvalue::UnaryOp(_, ref operand) => {
                let op_place = op_to_data_node(operand, virt_memory);
                add_node_to_statement(net, op_place, self.stmt_transition)?;
            }
            Rvalue::Ref(_, _, ref place) | Rvalue::Len(ref place) => {
                let place_local = place_to_data_node(place, virt_memory);
                add_node_to_statement(net, place_local, self.stmt_transition)?;
            }
            Rvalue::Cast(ref _kind, ref operand, ref _typ) => {
                let op_place = op_to_data_node(operand, virt_memory);
                add_node_to_statement(net, op_place, self.stmt_transition)?;
            }
            Rvalue::BinaryOp(ref _operator, ref loperand, ref roperand)
            | Rvalue::CheckedBinaryOp(ref _operator, ref loperand, ref roperand) => {
                let lop_place = op_to_data_node(loperand, virt_memory);
                let rop_place = op_to_data_node(roperand, virt_memory);
                add_node_to_statement(net, lop_place, self.stmt_transition)?;
                add_node_to_statement(net, rop_place, self.stmt_transition)?;
            }
            Rvalue::NullaryOp(ref operator, ref _typ) => match operator {
                // these are essentially a lookup of the type size in the static space
                mir::NullOp::SizeOf | mir::NullOp::Box => {
                    net.add_arc(virt_memory.get_constant(), self.stmt_transition)?;
                    net.add_arc(self.stmt_transition, virt_memory.get_constant())?;
                }
            },
            Rvalue::Discriminant(ref place) => {
                let op_place = place_to_data_node(place, virt_memory);
                add_node_to_statement(net, op_place, self.stmt_transition)?;
            }
            Rvalue::Aggregate(ref _kind, ref operands) => {
                //FIXME: does the kind matter?
                for operand in operands {
                    let op_place = op_to_data_node(operand, virt_memory);
                    add_node_to_statement(net, op_place, self.stmt_transition)?;
                }
            }
            Rvalue::AddressOf(_, place) => {
                let place_local = place_to_data_node(place, virt_memory);
                add_node_to_statement(net, place_local, self.stmt_transition)?;
            }
        }
        Ok(())
    }
}

fn add_node_to_statement(
    net: &mut PetriNet,
    place_node: NodeRef,
    statement_transition: NodeRef,
) -> Result<()> {
    net.add_arc(place_node, statement_transition)?;
    net.add_arc(statement_transition, place_node)?;
    Ok(())
}

use crate::petri_net::basic_block::BasicBlock;
use pnml;
use pnml::{NodeRef, PageRef, PetriNet, Result};
use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc_data_structures::indexed_vec::IndexVec;
use std::collections::HashMap;

macro_rules! active_block {
    ($function:ident) => {
        $function
            .basic_blocks
            .get(
                &$function
                    .active_block
                    .expect("activeBlock was not initialized"),
            )
            .expect("unable to find active basic block in function");
    };
}

macro_rules! active_block_mut {
    ($function:ident) => {
        $function
            .basic_blocks
            .get_mut(
                &$function
                    .active_block
                    .expect("activeBlock was not initialized"),
            )
            .expect("unable to find active basic block in function");
    };
}

#[derive(Debug)]
pub struct Function<'mir> {
    pub mir_id: DefId,
    pub mir_body: &'mir mir::Body<'mir>,
    basic_blocks: HashMap<mir::BasicBlock, BasicBlock>,
    locals: HashMap<mir::Local, Local>,
    args: HashMap<mir::Local, Local>,
    destination: Option<(mir::Local, Local)>,
    active_block: Option<mir::BasicBlock>,
    page: PageRef,
    start_place: NodeRef,
    end_place: NodeRef,
}

#[derive(Debug, Clone)]
pub struct Local {
    prenatal_place: NodeRef,
    live_place: NodeRef,
    dead_place: NodeRef,
}

impl Local {
    pub fn new<'net>(net: &'net mut PetriNet, page: &PageRef) -> Result<Self> {
        let mut prenatal_place = net.add_place(page)?;
        prenatal_place.initial_marking(net, 1)?;
        let live_place = net.add_place(page)?;
        let dead_place = net.add_place(page)?;
        Ok(Local {
            prenatal_place,
            live_place,
            dead_place,
        })
    }
    pub fn spawn(&mut self) {}
    pub fn eliminate(&mut self) {}
}

impl<'mir, 'a> Function<'mir> {
    pub fn new<'net>(
        mir_id: DefId,
        mir_body: &'mir mir::Body<'mir>,
        net: &'net mut PetriNet,
        args: HashMap<mir::Local, Local>,
        destination: Option<(mir::Local, Local)>, //TODO: also request return basicBlock?
        start_place: NodeRef,
        name: &str,
    ) -> Result<Self> {
        let page = net.add_page(Some(name));
        let end_place = net.add_place(&page)?;

        Ok(Function {
            mir_id,
            mir_body,
            basic_blocks: HashMap::new(),
            locals: HashMap::new(),
            args,
            destination,
            active_block: None,
            page,
            start_place,
            end_place,
        })
    }

    pub fn get_local(&self, mir_local: &mir::Local) -> Result<&Local> {
        // search in the args the list of locals and in the destination
        match self.args.get(mir_local) {
            Some(local) => Ok(&local),
            None => match self.locals.get(mir_local) {
                Some(local) => Ok(&local),
                None => match &self.destination {
                    Some((_, local)) => Ok(&local),
                    None => panic!("local not found"),
                },
            },
        }
    }

    pub fn add_statement<'net>(&mut self, net: &'net mut PetriNet) -> Result<()> {
        active_block_mut!(self).add_statement(net);
        Ok(())
    }

    pub fn goto<'net>(&mut self, net: &'net mut PetriNet, to: &mir::BasicBlock) -> Result<()> {
        let page = self.page.clone();
        let t = net.add_transition(&page)?;
        net.add_arc(&self.page, active_block!(self).end_place(), &t)?;
        let to = match self.basic_blocks.get(to) {
            Some(block) => block.start_place(),
            None => self.add_basic_block(net, to)?.start_place(),
        };
        net.add_arc(&page, &t, to)?;
        Ok(())
    }

    pub fn retorn<'net>(&mut self, net: &'net mut PetriNet) -> Result<()> {
        let source = {
            // check if we got trolled by an empty function
            if let Some(mir_block) = self.active_block {
                if let Some(_) = self.basic_blocks.get(&mir_block) {
                    active_block!(self).end_place()
                } else {
                    &self.start_place
                }
            } else {
                &self.start_place
            }
        };
        let t = net.add_transition(&self.page)?;
        net.add_arc(&self.page, source, &t)?;
        net.add_arc(&self.page, &t, &self.end_place)?;
        Ok(())
    }

    pub fn activate_block<'net>(
        &mut self,
        net: &'net mut PetriNet,
        block: &mir::BasicBlock,
    ) -> Result<()> {
        match self.basic_blocks.get(block) {
            Some(_) => {}
            None => {
                self.add_basic_block(net, block)?;
            }
        };
        self.active_block = Some(*block);
        Ok(())
    }

    pub fn function_call_start_place(&self) -> Result<&NodeRef> {
        let block = active_block!(self);
        Ok(block.end_place())
    }

    fn add_basic_block<'net>(
        &mut self,
        net: &'net mut PetriNet,
        block: &mir::BasicBlock,
    ) -> Result<&BasicBlock> {
        let start_place = net.add_place(&self.page)?;
        let bb = BasicBlock::new(net, &self.page, &start_place)?;
        self.basic_blocks
            .insert(*block, bb)
            .expect_none("this should not happen");
        Ok(self
            .basic_blocks
            .get(block)
            .expect("this also should not happen"))
    }

    pub fn add_locals<'net, 'tcx>(
        &mut self,
        net: &'net mut PetriNet,
        locals: &IndexVec<mir::Local, mir::LocalDecl<'tcx>>,
    ) -> Result<()> {
        // a lot of locals here:
        // mir_local: mir::Local => index for local decls in mir data structure
        // decl: mir::LocalDecl => data of a local in mir data structure
        // local: crate:: .. ::Local => petri net representation of a local
        for (mir_local, _decl) in locals.iter_enumerated() {
            let local = Local::new(net, &self.page)?;
            self.locals.insert(mir_local, local);
        }
        Ok(())
    }
}

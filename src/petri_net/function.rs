use crate::petri_net::basic_block::BasicBlock;
use pnml;
use pnml::{NodeRef, PageRef, PetriNet, Result};
use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc_index::vec::IndexVec;
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

//TODO: Statics and constants are handled inefficiently. They are just copied by every
// push of a new stack frame. But they can be just referenced from a root owner (the
// ownership should be clear on a stack). Maybe it is also possible to handle them per
// function (or per module) as their scope is limited.
#[derive(Debug)]
pub struct VirtualMemory {
    locals: HashMap<mir::Local, Data>,
    //FIXME: this is an oversimplification of statics
    // the DefId can be of an entire function and
    // inlining may split the same static into different DefIds
    statics: HashMap<mir::Promoted, Data>,
    // constants currently don't need special data and can be represented all with the same node
    constants: Data,
}

#[derive(Debug)]
pub struct Function<'mir> {
    pub mir_id: DefId,
    pub mir_body: &'mir mir::Body<'mir>,
    basic_blocks: HashMap<mir::BasicBlock, BasicBlock>,
    virt_memory: VirtualMemory,
    pub active_block: Option<mir::BasicBlock>,
    page: PageRef,
    start_place: NodeRef,
    end_place: NodeRef,
}

#[derive(Debug, Clone)]
pub enum Data {
    Local(Local),
    Static(NodeRef),
    Constant(NodeRef),
}

#[derive(Debug, Clone)]
pub struct Local {
    pub(crate) prenatal_place: NodeRef,
    pub(crate) live_place: NodeRef,
    pub(crate) dead_place: NodeRef,
}

#[derive(Debug, Clone, Hash)]
pub enum LocalKey {
    MirLocal(mir::Local),
    MirStatic(mir::Promoted),
    Constant,
}

impl Local {
    pub fn new<'net>(net: &'net mut PetriNet, page: &PageRef, name: &str) -> Result<Self> {
        let mut prenatal_place = net.add_place(page)?;
        prenatal_place.initial_marking(net, 1)?;
        let mut live_place = net.add_place(page)?;
        let mut dead_place = net.add_place(page)?;
        prenatal_place.name(net, &format!("local_{}_uninitialized", name))?;
        live_place.name(net, &format!("local_{}_live", name))?;
        dead_place.name(net, &format!("local_{}_dead", name))?;
        Ok(Local {
            prenatal_place,
            live_place,
            dead_place,
        })
    }
}

impl VirtualMemory {
    pub fn get(&self, key: &LocalKey) -> Option<&Data> {
        match key {
            LocalKey::MirLocal(local) => self.locals.get(local),
            LocalKey::MirStatic(statik) => self.statics.get(statik),
            LocalKey::Constant => Some(&self.constants),
        }
    }

    pub fn get_local(&self, local: &mir::Local) -> Option<&Local> {
        match self.locals.get(local) {
            Some(Data::Local(local)) => Some(local),
            None => None,
            Some(_) => panic!("Non local stored in locals space"),
        }
    }

    pub fn get_static(&self, statik: &mir::Promoted) -> Option<&NodeRef> {
        match self.statics.get(statik) {
            Some(Data::Static(statik)) => Some(statik),
            None => None,
            Some(_) => panic!("Non static stored in statics space"),
        }
    }

    pub fn get_constant(&self) -> &NodeRef {
        match &self.constants {
            Data::Constant(constant) => constant,
            _ => panic!("Non constant stored in constant space"),
        }
    }

    // fn add_static(&mut self, statik: &DefId) -> &NodeRef {
    //     self.statics.push()
    // }
}

impl<'mir> Function<'mir> {
    pub fn new<'net>(
        mir_id: DefId,
        mir_body: &'mir mir::Body<'mir>,
        net: &'net mut PetriNet,
        start_place: NodeRef,
        constant_memory: &Data,
        static_memory: &HashMap<mir::Promoted, Data>,
        name: &str,
    ) -> Result<Self> {
        let page = net.add_page(Some(name));
        let end_place = net.add_place(&page)?;
        let mut function = Function {
            mir_id,
            mir_body,
            basic_blocks: HashMap::new(),
            //FIXME: unnessecary cloning of statics and constants
            virt_memory: VirtualMemory {
                locals: HashMap::new(),
                constants: constant_memory.clone(),
                statics: static_memory.clone(),
            },
            active_block: None,
            page,
            start_place,
            end_place,
        };
        function.add_locals(net, &function.mir_body.local_decls)?;
        Ok(function)
    }

    pub fn add_statement<'net>(
        &mut self,
        net: &'net mut PetriNet,
        statement: &mir::Statement<'_>,
    ) -> Result<()> {
        active_block_mut!(self).add_statement(net, statement, &self.virt_memory)?;
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

    pub fn switch_int<'net>(
        &mut self,
        net: &'net mut PetriNet,
        targets: &Vec<mir::BasicBlock>,
    ) -> Result<()> {
        for bb in targets {
            if !self.basic_blocks.contains_key(bb) {
                self.add_basic_block(net, bb)?;
            };
            let source_end = active_block!(self).end_place();
            let target_start = self.basic_blocks.get(bb).unwrap().start_place();
            let mut connection_transition = net.add_transition(&self.page)?;
            connection_transition.name(net, &format!("switch int{}", bb.index()))?;
            net.add_arc(&self.page, &source_end, &connection_transition)?;
            net.add_arc(&self.page, &connection_transition, &target_start)?;
        }
        Ok(())
    }

    pub fn resume<'net>(
        &mut self,
        net: &'net mut PetriNet,
        unwind_place: &NodeRef,
        program_end_place: &NodeRef,
    ) -> Result<()> {
        // TODO: make the unwind and resume semantic clear

        let source_place = active_block!(self).end_place();
        let mut t = net.add_transition(&self.page)?;
        t.name(net, "unwind")?;
        net.add_arc(&self.page, source_place, &t)?;
        net.add_arc(&self.page, &t, unwind_place)?;
        net.add_arc(&self.page, &t, program_end_place)?;
        Ok(())
    }

    pub fn drop<'net>(
        &mut self,
        net: &'net mut PetriNet,
        target: &mir::BasicBlock,
        unwind: &Option<mir::BasicBlock>,
    ) -> Result<()> {
        let source = active_block!(self).end_place();
        let target_start = self.basic_blocks.get(target).unwrap().start_place();
        let mut t = net.add_transition(&self.page)?;
        t.name(net, "drop")?;
        net.add_arc(&self.page, source, &t)?;
        net.add_arc(&self.page, &t, target_start)?;

        if let Some(unwind) = unwind {
            let unwind_start = self.basic_blocks.get(unwind).unwrap().start_place();
            let mut t_unwind = net.add_transition(&self.page)?;
            t.name(net, "drop_unwind")?;
            net.add_arc(&self.page, source, &t_unwind)?;
            net.add_arc(&self.page, &t_unwind, unwind_start)?;
        };
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

    pub fn constants(&self) -> &Data {
        &self.virt_memory.constants
    }

    pub fn statics(&self) -> &HashMap<mir::Promoted, Data> {
        &self.virt_memory.statics
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
        for (mir_local, decl) in locals.iter_enumerated() {
            let name = match decl.name {
                Some(name) => format!("{}: {}", name, decl.ty),
                None => format!("_: {}", decl.ty),
            };
            let local = Local::new(net, &self.page, &name)?;
            self.virt_memory
                .locals
                .insert(mir_local, Data::Local(local));
        }
        Ok(())
    }
}

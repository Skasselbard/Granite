use crate::intrinsics::*;
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

macro_rules! block_to_start_place {
    ($function:ident, $net:ident, $block:expr) => {
        match $function.basic_blocks.get($block) {
            Some(block) => block.start_place().clone(),
            None => $function
                .add_basic_block($net, $block)?
                .start_place()
                .clone(),
        }
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
    pub page: PageRef,
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
        let to = block_to_start_place!(self, net, to);
        net.add_arc(&page, &t, &to)?;
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
        let page = self.page.clone();
        let target_start = block_to_start_place!(self, net, target);
        let source = active_block!(self).end_place().clone();
        let mut t = net.add_transition(&page)?;
        t.name(net, "drop")?;
        net.add_arc(&page, &source, &t)?;
        net.add_arc(&page, &t, &target_start)?;

        if let Some(unwind) = unwind {
            let unwind_start = block_to_start_place!(self, net, unwind);
            let mut t_unwind = net.add_transition(&self.page)?;
            t.name(net, "drop_unwind")?;
            net.add_arc(&self.page, &source, &t_unwind)?;
            net.add_arc(&self.page, &t_unwind, &unwind_start)?;
        };
        Ok(())
    }

    pub fn emulate_foreign(
        &mut self,
        net: &mut PetriNet,
        intrinsic_name: &str,
        args: &Vec<mir::Operand<'_>>,
        destination: &(mir::Place<'_>, mir::BasicBlock),
        cleanup: &Option<mir::BasicBlock>,
    ) -> Result<()> {
        let (destination_node, destination_block) = {
            let node = place_to_data_node(&destination.0, &self.virt_memory).clone();
            let block = block_to_start_place!(self, net, &destination.1);
            (node, block)
        };
        let cleanup = match cleanup {
            Some(block) => Some(block_to_start_place!(self, net, &block)),
            None => None,
        };
        let source = active_block!(self).end_place().clone();
        let mut arg_nodes = Vec::new();
        for operand in args {
            arg_nodes.push(op_to_data_node(operand, &self.virt_memory));
        }
        match intrinsic_name {
            name if name.contains("std::ops::DerefMut::deref_mut")
                | name.contains("std::convert::Into::into")
                | name.contains("std::ops::FnOnce::call_once")
                | name.contains("std::ops::Deref::deref")
                | name.contains("std::intrinsics::transmute") =>
            {
                generic_foreign(
                    net,
                    &self.page,
                    &arg_nodes,
                    &source,
                    &destination_node,
                    &destination_block,
                    cleanup,
                )?
            }
            _ => {
                error!("unimplemented intrinsic: {}", intrinsic_name);
            }
        }
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

pub(crate) fn op_to_data_node<'a>(
    operand: &mir::Operand<'_>,
    memory: &'a VirtualMemory,
) -> &'a NodeRef {
    match operand {
        mir::Operand::Copy(place) | mir::Operand::Move(place) => place_to_data_node(place, memory),
        // Constants are always valid reads
        // until using a high level petri net the value is not important and can be ignored
        // Constants can be seen as one petri net place that is accessed
        mir::Operand::Constant(_) => memory.get_constant(),
    }
}

pub(crate) fn place_to_data_node<'a>(
    place: &mir::Place<'_>,
    memory: &'a VirtualMemory,
) -> &'a NodeRef {
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

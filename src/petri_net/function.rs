use crate::intrinsics::*;
use crate::petri_net::basic_block::BasicBlock;
use petri_to_star::{NodeRef, PetriNet, PlaceRef, Result};
use rustc::hir::def_id::DefId;
use rustc::mir;
use rustc_index::vec::IndexVec;
use std::collections::HashMap;
use std::convert::TryFrom;

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
        match $function.basic_blocks.get(&$block) {
            Some(block) => block.start_place(),
            None => $function.add_basic_block($net, $block)?.start_place(),
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
    pub name: String,
    pub mir_body: &'mir mir::Body<'mir>,
    basic_blocks: HashMap<mir::BasicBlock, BasicBlock>,
    virt_memory: VirtualMemory,
    pub active_block: Option<mir::BasicBlock>,
    start_place: NodeRef,
    return_flow: NodeRef,
}

#[derive(Debug, Clone)]
pub enum Data {
    Local(Local),
    Static(NodeRef),
    Constant(NodeRef),
}

#[derive(Debug, Clone, Copy)]
pub struct Local {
    pub(crate) prenatal_place: Option<NodeRef>, // function arguments can be constant
    pub(crate) live_place: NodeRef,
    pub(crate) dead_place: Option<NodeRef>,
}

#[derive(Debug, Clone, Hash)]
pub enum LocalKey {
    MirLocal(mir::Local),
    MirStatic(mir::Promoted),
    Constant,
}

impl Local {
    pub fn new<'net>(net: &'net mut PetriNet, name: &str) -> Result<Self> {
        let prenatal_place = Some(net.add_place());
        PlaceRef::try_from(prenatal_place.unwrap())?.marking(net, 1)?;
        let live_place = net.add_place();
        let dead_place = Some(net.add_place());
        prenatal_place
            .unwrap()
            .name(net, format!("{} uninitialized", name))?;
        live_place.name(net, format!("{} live", name))?;
        dead_place.unwrap().name(net, format!("{} dead", name))?;
        Ok(Local {
            prenatal_place,
            live_place,
            dead_place,
        })
    }
    pub fn new_constant(live_place: NodeRef) -> Self {
        Self {
            prenatal_place: None,
            live_place,
            dead_place: None,
        }
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

    pub fn get_static(&self, statik: &mir::Promoted) -> Option<NodeRef> {
        match self.statics.get(statik) {
            Some(Data::Static(statik)) => Some(*statik),
            None => None,
            Some(_) => panic!("Non static stored in statics space"),
        }
    }

    pub fn get_constant(&self) -> NodeRef {
        match &self.constants {
            Data::Constant(constant) => *constant,
            _ => panic!("Non constant stored in constant space"),
        }
    }
}

impl<'mir> Function<'mir> {
    pub fn new<'net>(
        name: String,
        mir_body: &'mir mir::Body<'mir>,
        net: &'net mut PetriNet,
        mut args: Vec<Local>, // data that is used from the previous stack frame
        data_return: Local,   // node which stores the return value
        start_place: NodeRef, // where to start from
        constant_memory: &Data,
        static_memory: &HashMap<mir::Promoted, Data>,
        return_flow: NodeRef, // where to continue after the call
    ) -> Result<Self> {
        let mut function = Function {
            name,
            mir_body,
            basic_blocks: HashMap::new(),
            //FIXME: unnessecary cloning of statics and constants
            virt_memory: VirtualMemory {
                locals: HashMap::new(),
                constants: constant_memory.clone(),
                statics: static_memory.clone(),
            },
            active_block: None,
            start_place,
            return_flow,
        };
        // add the locals but remember the locals from the previous stack frame
        // index zero is the return local followed by the function arguments
        args.insert(0, data_return);
        function.add_locals(net, &function.mir_body.local_decls, args)?;
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

    pub fn finish_basic_block(&self, net: &mut PetriNet) -> Result<()> {
        active_block!(self).finish_statement_block(net)
    }

    pub fn goto<'net>(&mut self, net: &'net mut PetriNet, to: mir::BasicBlock) -> Result<()> {
        let t = net.add_transition();
        t.name(net, "Goto".into())?;
        net.add_arc(active_block!(self).end_place(), t)?;
        let to = block_to_start_place!(self, net, to);
        net.add_arc(t, to)?;
        Ok(())
    }

    pub fn retorn<'net>(&mut self, net: &'net mut PetriNet) -> Result<()> {
        let source = {
            // check if we got trolled by an empty function
            if let Some(mir_block) = self.active_block {
                if let Some(_) = self.basic_blocks.get(&mir_block) {
                    active_block!(self).end_place()
                } else {
                    self.start_place
                }
            } else {
                self.start_place
            }
        };
        let t = net.add_transition();
        t.name(net, "Return".into())?;
        net.add_arc(source, t)?;
        net.add_arc(t, self.return_flow)?;
        Ok(())
    }

    pub fn switch_int<'net>(
        &mut self,
        net: &'net mut PetriNet,
        targets: &Vec<mir::BasicBlock>,
    ) -> Result<()> {
        for bb in targets {
            if !self.basic_blocks.contains_key(bb) {
                self.add_basic_block(net, *bb)?;
            };
            let source_end = active_block!(self).end_place();
            let target_start = self.basic_blocks.get(bb).unwrap().start_place();
            let connection_transition = net.add_transition();
            connection_transition.name(net, format!("switch int{}", bb.index()))?;
            net.add_arc(source_end, connection_transition)?;
            net.add_arc(connection_transition, target_start)?;
        }
        Ok(())
    }

    pub fn resume<'net>(
        &mut self,
        net: &'net mut PetriNet,
        unwind_place: NodeRef,
        program_end_place: NodeRef,
    ) -> Result<()> {
        // TODO: make the unwind and resume semantic clear
        let source_place = active_block!(self).end_place();
        let t = net.add_transition();
        t.name(net, "unwind".into())?;
        net.add_arc(source_place, t)?;
        net.add_arc(t, unwind_place)?;
        net.add_arc(t, program_end_place)?;
        Ok(())
    }

    pub fn drop<'net>(
        &mut self,
        net: &'net mut PetriNet,
        target: mir::BasicBlock,
        unwind: Option<mir::BasicBlock>,
    ) -> Result<()> {
        let target_start = block_to_start_place!(self, net, target);
        let source = active_block!(self).end_place().clone();
        let t = net.add_transition();
        t.name(net, "drop".into())?;
        net.add_arc(source, t)?;
        net.add_arc(t, target_start)?;

        if let Some(unwind) = unwind {
            let unwind_start = block_to_start_place!(self, net, unwind);
            let t_unwind = net.add_transition();
            t_unwind.name(net, "drop_unwind".into())?;
            net.add_arc(source, t_unwind)?;
            net.add_arc(t_unwind, unwind_start)?;
        };
        Ok(())
    }

    pub fn assert(
        &mut self,
        net: &mut PetriNet,
        condition: &mir::Operand<'_>,
        _expected: bool,
        target: mir::BasicBlock,
        cleanup: Option<mir::BasicBlock>,
    ) -> Result<()> {
        let target_start = block_to_start_place!(self, net, target);
        let source = active_block!(self).end_place().clone();
        let op_place = op_to_data_node(condition, &self.virt_memory);
        let t = net.add_transition();
        t.name(net, "drop".into())?;
        net.add_arc(source, t)?;
        net.add_arc(t, target_start)?;
        net.add_arc(op_place, t)?;
        net.add_arc(t, op_place)?;
        // represents reading the condition
        net.add_arc(t, self.virt_memory.get_constant())?;
        net.add_arc(self.virt_memory.get_constant(), t)?;
        if let Some(unwind) = cleanup {
            let unwind_start = block_to_start_place!(self, net, unwind);
            let t_unwind = net.add_transition();
            t_unwind.name(net, "assert_unwind".into())?;
            net.add_arc(source, t_unwind)?;
            net.add_arc(t_unwind, unwind_start)?;
        };
        Ok(())
    }

    pub fn emulate_foreign(
        &mut self,
        net: &mut PetriNet,
        intrinsic_name: &str,
        //TODO: check arguments -> are noderefs needed?
        args: &Vec<mir::Operand<'_>>,
        destination: &Option<(mir::Place<'_>, mir::BasicBlock)>,
        cleanup: Option<mir::BasicBlock>,
        panic_place: NodeRef,
    ) -> Result<()> {
        if let Some((destination_node, destination_block)) = destination {
            let node = place_to_data_node(destination_node, &self.virt_memory).clone();
            let block = block_to_start_place!(self, net, *destination_block);

            let cleanup = match cleanup {
                Some(block) => Some(block_to_start_place!(self, net, block)),
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
                | name.contains("std::panicking::panicking")
                //TODO: deallocation might be deadlock relevant
                | name.contains("alloc::alloc::__rust_dealloc")
                | name.contains("std::intrinsics::min_align_of_val")
                | name.contains("std::intrinsics::caller_location")
                | name.contains("std::intrinsics::size_of_val")
                //TODO: atomic functions need to be explained
                | name.contains("std::intrinsics::atomic_load_acq")
                | name.contains("std::intrinsics::atomic_load_relaxed")
                | name.contains("std::intrinsics::atomic_load")
                | name.contains("std::intrinsics::transmute") =>
                {
                    generic_foreign(net, &arg_nodes, source, node, block, cleanup)?
                }
                name if name.contains("libc::unix::pthread_mutexattr_init")
                    | name.contains("libc::unix::pthread_mutex_init")
                    | name.contains("libc::unix::pthread_mutexattr_settype")
                    | name.contains("libc::unix::pthread_mutexattr_destro")
                    | name.contains("libc::unix::pthread_mutex_lock") =>
                {
                    warn!("mutex intrinsic {}", name);
                    generic_foreign(net, &arg_nodes, source, node, block, cleanup)?
                }
                _ => {
                    warn!(
                        "unchecked intrinsic: {} args:{:?} dest:{:?}, cleanup:{:?}",
                        intrinsic_name, args, destination, cleanup
                    );
                    generic_foreign(net, &arg_nodes, source, node, block, cleanup)?
                }
            }
        } else {
            // diverging function (destination = none)
            match intrinsic_name {
                name if name.contains("std::alloc::handle_alloc_error")
                    | name.contains("alloc::raw_vec::capacity_overflow")
                    | name.contains("std::result::unwrap_failed") =>
                {
                    self.handle_panic(net, panic_place)?
                }
                _ => panic!("unhandled diverging foreign: {}", intrinsic_name),
            }
        }
        Ok(())
    }

    pub fn handle_panic(&mut self, net: &mut PetriNet, panic_place: NodeRef) -> Result<()> {
        let source = active_block!(self).end_place().clone();
        let t = net.add_transition();
        t.name(net, "panic".into())?;
        net.add_arc(source, t)?;
        net.add_arc(t, panic_place)?;
        Ok(())
    }

    pub fn activate_block<'net>(
        &mut self,
        net: &'net mut PetriNet,
        block: mir::BasicBlock,
    ) -> Result<()> {
        match self.basic_blocks.get(&block) {
            Some(_) => {}
            None => {
                self.add_basic_block(net, block)?;
            }
        };
        self.active_block = Some(block);
        Ok(())
    }

    pub fn function_call_start_place(&self) -> Result<NodeRef> {
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
        block: mir::BasicBlock,
    ) -> Result<&BasicBlock> {
        let start_place = if self.basic_blocks.is_empty() {
            self.start_place
        } else {
            let place = net.add_place();
            place.name(net, format!("bb {}", block.index()))?;
            place
        };
        let bb = BasicBlock::new(net, start_place)?;
        self.basic_blocks
            .insert(block, bb)
            .expect_none("this should not happen");
        Ok(self
            .basic_blocks
            .get(&block)
            .expect("this also should not happen"))
    }

    pub fn get_basic_block_start(
        &mut self,
        net: &mut PetriNet,
        block: mir::BasicBlock,
    ) -> Result<NodeRef> {
        Ok(block_to_start_place!(self, net, block))
    }

    fn add_locals<'net, 'tcx>(
        &mut self,
        net: &'net mut PetriNet,
        locals: &IndexVec<mir::Local, mir::LocalDecl<'tcx>>,
        known_locals: Vec<Local>,
    ) -> Result<()> {
        // a lot of locals here:
        // mir_local: mir::Local => index for local decls in mir data structure
        // decl: mir::LocalDecl => data of a local in mir data structure
        // local: crate:: .. ::Local => petri net representation of a local
        for (mir_local, decl) in locals.iter_enumerated() {
            let name = format!("{}_{}: {}", self.name, mir_local.index(), decl.ty);
            let local = if let Some(local) = known_locals.get(mir_local.index()) {
                *local
            } else {
                Local::new(net, &name)?
            };
            self.virt_memory
                .locals
                .insert(mir_local, Data::Local(local));
        }
        Ok(())
    }

    pub fn get_local(&self, local: &mir::Local) -> Option<&Local> {
        self.virt_memory.get_local(local)
    }

    pub fn get_promoted(&self, statik: &mir::Promoted) -> Option<Local> {
        match self.virt_memory.get_static(statik) {
            Some(node) => Some(Local::new_constant(node)),
            None => None,
        }
    }
}

pub(crate) fn op_to_data_node(operand: &mir::Operand<'_>, memory: &VirtualMemory) -> NodeRef {
    match operand {
        mir::Operand::Copy(place) | mir::Operand::Move(place) => place_to_data_node(place, memory),
        // Constants are always valid reads
        // until using a high level petri net the value is not important and can be ignored
        // Constants can be seen as one petri net place that is accessed
        mir::Operand::Constant(_) => memory.get_constant(),
    }
}

pub(crate) fn place_to_data_node(place: &mir::Place<'_>, memory: &VirtualMemory) -> NodeRef {
    let local = place.local_or_deref_local();
    match local {
        Some(local) => {
            memory
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
                memory.get_local(local).expect("local not found").live_place
            }
            // https://doc.rust-lang.org/nightly/nightly-rustc/rustc/ty/context/struct.TyCtxt.html#method.promoted_mir
            mir::PlaceBase::Static(statik) => match statik.kind {
                mir::StaticKind::Static => panic!("staticKind::Static -> cannot convert"),
                mir::StaticKind::Promoted(promoted, _) => memory
                    .get_static(&promoted)
                    .expect("promoted statik not found"),
            },
        },
    }
}

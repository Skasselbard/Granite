use crate::petri_net::function::{Data, Function, Local};
use crate::petri_net::unique_functions::MutexList;
use petri_to_star::{NodeRef, PetriNet, PlaceRef, Result};
use rustc::mir::visit::Visitor;
use rustc::mir::visit::*;
use rustc::mir::{self, *};
use rustc::ty::{self, Ty, TyCtxt};
use rustc_hir::def_id::DefId;
use rustc_mir::util::write_mir_pretty;
use std::collections::HashSet;
use std::convert::TryFrom;

struct CallStack<T> {
    stack: Vec<T>,
}

impl<T> CallStack<T> {
    pub fn new() -> Self {
        CallStack { stack: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        self.stack.push(item)
    }

    pub fn pop(&mut self) -> Option<T> {
        self.stack.pop()
    }

    pub fn peek(&self) -> Option<&T> {
        if self.stack.is_empty() {
            None
        } else {
            Some(&self.stack[self.stack.len() - 1])
        }
    }

    pub fn peek_mut(&mut self) -> Option<&mut T> {
        if self.stack.is_empty() {
            None
        } else {
            let len = self.stack.len();
            Some(&mut self.stack[len - 1])
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

pub struct Translator<'tcx> {
    tcx: TyCtxt<'tcx>,
    call_stack: CallStack<Function<'tcx>>,
    visited: HashSet<DefId>,
    net: PetriNet,
    mutex_list: MutexList,
    unwind_abort_place: NodeRef,
    program_end_place: Option<NodeRef>,
    mir_dump: Option<std::fs::File>,
}

macro_rules! net {
    ($translator:ident) => {
        &mut $translator.net
    };
}

macro_rules! function {
    ($translator:ident) => {
        $translator.call_stack.peek_mut().expect("empty call stack")
    };
}

impl<'tcx> Translator<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, mir_dump: Option<std::fs::File>) -> Result<Self> {
        let mut net = PetriNet::new();
        let unwind_abort_place = net.add_place();
        unwind_abort_place.name(&mut net, "unwind_abort".into())?;
        Ok(Translator {
            tcx,
            call_stack: CallStack::new(),
            visited: HashSet::new(),
            net,
            mutex_list: MutexList::new(),
            unwind_abort_place,
            program_end_place: None,
            mir_dump,
        })
    }

    pub fn petrify(&mut self, main_fn: DefId) -> Result<&PetriNet> {
        let start_place = {
            let net = net!(self);
            let place = net.add_place();
            PlaceRef::try_from(place)?.marking(net, 1)?;
            place
        };
        self.program_end_place = {
            let net = net!(self);
            let place = net.add_place();
            place.name(net, "program end".into())?;
            Some(place)
        };
        let data_return = Local::new(net!(self), "main_return")?;
        self.translate(
            main_fn,
            Vec::new(), //TODO: Arguments would be important for HiLvl Nets
            data_return,
            start_place,
            self.program_end_place
                .expect("no program end place defined"),
        )?;
        Ok(&self.net)
    }

    fn translate<'a>(
        &mut self,
        function: DefId,
        args: Vec<Local>,
        data_return: Local,
        start_place: NodeRef,
        return_flow: NodeRef,
    ) -> Result<()> {
        let fn_name = self.tcx.def_path_str(function);
        start_place.name(&mut self.net, fn_name.clone())?;
        if Self::is_unique(&fn_name) {
            self.translate_unique(
                function,
                args,
                data_return,
                start_place,
                return_flow,
                fn_name,
            )?
        } else {
            self.translate_default(
                function,
                args,
                data_return,
                start_place,
                return_flow,
                fn_name,
            )?
        }
        Ok(())
    }

    fn is_panic(tcx: TyCtxt<'_>, function: DefId) -> bool {
        match tcx.def_path_str(function) {
            // panic functions of libstd
            name if name.contains("std::rt::begin_panic_fmt")
                | name.contains("std::rt::begin_panic")
                // panic functions of libcore
                | name.contains("core::panicking::panic")
                | name.contains("core::panicking::panic_fmt") =>
            {
                true
            }
            _ => false,
        }
    }

    pub fn is_unique(name: &str) -> bool {
        match name {
            name if name.contains("std::sync::Mutex::<T>::new")
                | name.contains("std::sync::Mutex::<T>::lock")
                | name.contains("std::sync::Mutex::<T>::try_lock") =>
            {
                true
            }
            _ => false,
        }
    }

    fn translate_default(
        &mut self,
        function: DefId,
        args: Vec<Local>,
        data_return: Local,
        start_place: NodeRef,
        return_flow: NodeRef,
        fn_name: String,
    ) -> Result<()> {
        info!("\n\nENTERING function: {:?}", fn_name);
        if let Some(file) = &mut self.mir_dump {
            if !self.visited.contains(&function) {
                write_mir_pretty(self.tcx, Some(function), file).unwrap();
            }
        };
        self.visited.insert(function);
        let body = self.tcx.optimized_mir(function);
        let (const_memory, mut static_memory) = if self.call_stack.is_empty() {
            let constants = net!(self).add_place();
            constants.name(net!(self), "CONSTANTS".into())?;
            PlaceRef::try_from(constants)?.marking(&mut self.net, 1)?;
            (Data::Constant(constants), std::collections::HashMap::new())
        } else {
            (
                function!(self).constants().clone(),
                function!(self).statics().clone(),
            )
        };
        // add missing promoted statics
        for (promoted, _) in self.tcx.promoted_mir(function).iter_enumerated() {
            if static_memory.get(&promoted).is_none() {
                let promoted_node = net!(self).add_place();
                promoted_node.name(
                    net!(self),
                    format!("Promoted_{} {}", promoted.index(), fn_name),
                )?;
                static_memory.insert(promoted, Data::Static(promoted_node));
            } else {
                warn!("duplicate of promoted static");
            }
        }
        let petri_function = Function::new(
            fn_name.clone(),
            body,
            net!(self),
            args,
            data_return,
            start_place,
            &const_memory,
            &static_memory,
            return_flow,
            &mut self.mutex_list,
            self.tcx,
        )?;
        self.call_stack.push(petri_function);
        self.visit_body(body.unwrap_read_only());
        self.call_stack.pop();
        info!("\nLEAVING function: {:?}\n", fn_name);
        Ok(())
    }

    fn translate_unique(
        &mut self,
        function: DefId,
        args: Vec<Local>,
        data_return: Local,
        start_place: NodeRef,
        return_flow: NodeRef,
        fn_name: String,
    ) -> Result<()> {
        let net = &mut self.net;

        // bridge the call
        let t = net.add_transition();
        t.name(net, fn_name.clone())?;
        net.add_arc(start_place, t)?;
        net.add_arc(t, return_flow)?;

        match fn_name {
            name if name.contains("std::sync::Mutex::<T>::new") => {
                let mutex = *self
                    .mutex_list
                    .get_linked(data_return)
                    .expect("mutex not found");
                net.add_arc(mutex.uninitialized(&self.mutex_list), t)?;
                net.add_arc(t, mutex.unlocked(&self.mutex_list))?;
            }
            name if name.contains("std::sync::Mutex::<T>::lock") => {
                let mutex = *self
                    .mutex_list
                    .get_linked(*args.get(0).expect("no mutex lock arg found"))
                    .expect("mutex not found");
                self.mutex_list.add_guard(data_return, mutex);
                net.add_arc(mutex.unlocked(&self.mutex_list), t)?;
                net.add_arc(t, mutex.locked(&self.mutex_list))?;
            }
            name if name.contains("std::sync::Mutex::<T>::try_lock") => unimplemented!(),
            _ => panic!("unhandled unique function"),
        };
        Ok(())
    }
}

impl<'tcx> Visitor<'tcx> for Translator<'tcx> {
    fn visit_body(&mut self, body: ReadOnlyBodyAndCache<'_, 'tcx>) {
        match body.phase {
            MirPhase::Optimized => {}
            _ => error!("tried to translate unoptimized MIR"),
        }
        self.super_body(body);
    }

    fn visit_basic_block_data(&mut self, block: BasicBlock, data: &BasicBlockData<'tcx>) {
        trace!("---BasicBlock {:?}---", block);
        function!(self)
            .activate_block(net!(self), block)
            .expect("unable to activate basic");
        self.super_basic_block_data(block, data)
    }

    fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
        let function = function!(self);

        let mut locals = Vec::new();
        match rvalue {
            Rvalue::Use(operand) | Rvalue::Repeat(operand, _) | Rvalue::Cast(_, operand, _) => {
                locals.push(function.op_to_local(operand))
            }
            Rvalue::Ref(_, _, place) => locals.push(function.place_to_local(place)),
            Rvalue::Discriminant(place) => locals.push(function.place_to_local(place)),
            Rvalue::AddressOf(_, _) => locals.push(function.place_to_local(place)),
            Rvalue::Aggregate(_, _) => unimplemented!(),
            _ => {}
        }

        for local in locals {
            if let Some(mutex) = self.mutex_list.is_linked(local) {
                debug!("link '{:?}' to mutex '{:?}'", place, mutex);
                self.mutex_list.link(function.place_to_local(place), *mutex)
            }
        }
        self.super_assign(place, rvalue, location);
    }

    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        trace!("{:?}: ", statement.kind);
        function!(self)
            .add_statement(net!(self), statement)
            .expect("unable to add statement");
        self.super_statement(statement, location);
    }

    fn visit_terminator_kind(&mut self, kind: &TerminatorKind<'tcx>, location: Location) {
        trace!("{:?}", kind);

        // check mutex links
        match kind {
            Call {
                func: _,
                ref args,
                ref destination,
                ..
            } => {
                for arg in args {
                    let local = function!(self).op_to_local(arg);
                    if let Some((place, _)) = destination {
                        if let Some(mutex) = self.mutex_list.is_linked(local) {
                            debug!("link '{:?}' to mutex '{:?}'", place, mutex);
                            self.mutex_list
                                .link(function!(self).place_to_local(place), *mutex)
                        }
                    }
                }
            }
            _ => {}
        }

        use rustc::mir::TerminatorKind::*;
        let net = net!(self);
        function!(self)
            .finish_basic_block(net)
            .expect("cannot end statement block");
        match kind {
            Return => {
                // trace!("Return");
                function!(self).retorn(net).expect("return failed");
            }

            Goto { target } => {
                // trace!("Goto");
                function!(self)
                    .goto(net, *target)
                    .expect("Goto Block failed");
            }

            SwitchInt {
                discr: _,
                switch_ty: _,
                values: _,
                targets,
            } => function!(self)
                .switch_int(net, targets)
                .expect("switch int failed"),

            Call {
                ref func,
                ref args,
                ref destination,
                ref cleanup,
                ..
            } => {
                // info!(
                //     "functionCall\nfunc: {:?}\nargs: {:?}\ndest: {:?}",
                //     func, args, destination
                // );
                let sty = {
                    match func {
                        Operand::Copy(ref place) | Operand::Move(ref place) => {
                            let function = self.call_stack.peek().expect("peeked empty stack");
                            let decls = function.mir_body.local_decls();
                            let place_ty: &mir::tcx::PlaceTy<'tcx> = &place.base.ty(decls);
                            place_ty.ty
                        }
                        Operand::Constant(ref constant) => &constant.literal.ty,
                    }
                };
                let function = match sty.kind {
                    ty::FnPtr(_) => {
                        error!("Function pointers are not supported");
                        panic!("")
                    }
                    ty::FnDef(def_id, _) => def_id,
                    _ => {
                        error!("Expected function definition or pointer but got: {:?}", sty);
                        panic!("")
                    }
                };
                if !Self::is_panic(self.tcx, function) {
                    if self.tcx.is_foreign_item(function) || !self.tcx.is_mir_available(function) {
                        info!("emulating mir-less item {:?}", function);
                        function!(self)
                            .emulate_foreign(
                                net,
                                &self.tcx.def_path_str(function),
                                args,
                                destination,
                                *cleanup,
                                self.unwind_abort_place,
                            )
                            .expect("unknown foreign item");
                    } else {
                        let start_place = function!(self)
                            .function_call_start_place()
                            .expect("Unable to infer start place of function call")
                            .clone();
                        let (return_place, return_block) = destination.as_ref().expect(&format!(
                            "diverging function: {}",
                            self.tcx.def_path_str(function),
                        ));
                        let data_return = *function!(self)
                            .get_local(
                                &return_place
                                    .local_or_deref_local()
                                    .expect("deref return place failed"),
                            )
                            .expect("return local not found");
                        let stack_top = function!(self); // needed in the closure
                        let args = args
                            .iter()
                            .map(|operand| stack_top.op_to_local(operand))
                            .collect();
                        let return_place = function!(self)
                            .get_basic_block_start(net, *return_block)
                            .expect("cannot find return block");
                        self.translate(function, args, data_return, start_place, return_place)
                            .expect("translation error");
                    }
                } else {
                    function!(self)
                        .handle_panic(net, self.unwind_abort_place)
                        .expect("panic handling error");
                }
            }

            Drop {
                location: _,
                target,
                unwind,
            } => function!(self)
                .drop(net, *target, *unwind)
                .expect("drop failed"),

            Assert {
                ref cond,
                ref expected,
                msg: _,
                ref target,
                ref cleanup,
            } => function!(self)
                .assert(net, cond, *expected, *target, *cleanup)
                .expect("assert failed"),

            Yield { .. } => panic!("Yield"),
            GeneratorDrop => panic!("GeneratorDrop"),
            DropAndReplace { .. } => panic!("DropAndReplace"),
            Resume => {
                function!(self)
                    .resume(net, self.unwind_abort_place)
                    .expect("resume failed");
            }
            Abort => function!(self)
                .handle_panic(net, self.unwind_abort_place)
                .expect("panic handling error"),
            FalseEdges { .. } => bug!(
                "should have been eliminated by\
                 `simplify_branches` mir pass"
            ),
            FalseUnwind { .. } => bug!(
                "should have been eliminated by\
                 `simplify_branches` mir pass"
            ),
            Unreachable => debug!("unreachable"),
        }
        self.super_terminator_kind(kind, location);
    }
}

use crate::petri_net::function::{Data, Function, Local};
use petri_to_star::{NodeRef, PetriNet, PlaceRef, Result};
use rustc::hir::def_id::DefId;
use rustc::mir::visit::Visitor;
use rustc::mir::visit::*;
use rustc::mir::{self, *};
use rustc::ty::{self, Ty, TyCtxt};
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
    net: PetriNet,
    unwind_abort_place: NodeRef,
    program_end_place: Option<NodeRef>,
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
    pub fn new(tcx: TyCtxt<'tcx>) -> Result<Self> {
        let mut net = PetriNet::new();
        let unwind_abort_place = net.add_place();
        unwind_abort_place.name(&mut net, "unwind_abort".into())?;
        Ok(Translator {
            tcx,
            call_stack: CallStack::new(),
            net,
            unwind_abort_place,
            program_end_place: None,
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
        let fn_name = function.describe_as_module(self.tcx);
        start_place.name(&mut self.net, fn_name.clone())?;
        info!("\n\nENTERING function: {:?}", fn_name);
        let body = self.tcx.optimized_mir(function);
        let (const_memory, mut static_memory) = if self.call_stack.is_empty() {
            let constants = net!(self).add_place();
            constants.name(net!(self), "CONSTANTS".into())?;
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
        )?;
        self.call_stack.push(petri_function);
        self.visit_body(body);
        self.call_stack.pop();
        info!("\nLEAVING function: {:?}\n", fn_name);
        Ok(())
    }

    fn is_panic(tcx: TyCtxt<'_>, function: DefId) -> bool {
        match function.describe_as_module(tcx) {
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
}

impl<'tcx> Visitor<'tcx> for Translator<'tcx> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        match body.phase {
            MirPhase::Optimized => {
                // trace!("source scopes: {:?}", body.source_scopes);
                // trace!(
                //     "source scopes local data: {:?}",
                //     body.source_scope_local_data
                // );
                //trace!("promoted: {:?}", entry_body.promoted);
                //trace!("return type: {:?}", body.return_ty());
                //trace!("yield type: {:?}", body.yield_ty);
                //trace!("generator drop: {:?}", body.generator_drop);
                //trace!("local declarations: {:?}", body.local_decls());
            }
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

    fn visit_source_scope_data(&mut self, scope_data: &SourceScopeData) {
        self.super_source_scope_data(scope_data);
    }

    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        trace!("{:?}: ", statement.kind);
        function!(self)
            .add_statement(net!(self), statement)
            .expect("unable to add statement");
        self.super_statement(statement, location);
    }

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        //trace!("{:?}", terminator);
        //warn!("Successors: {:?}", terminator.successors());
        self.super_terminator(terminator, location);
    }
    // fn visit_place_base(
    //     &mut self,
    //     base: &PlaceBase<'tcx>,
    //     context: PlaceContext,
    //     location: Location,
    // ) {
    //     warn!("placeBase: {:?}", base);
    //     self.super_place_base(base, context, location);
    // }
    // fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
    //     warn!("place: {:?}", place);
    //     self.super_place(place, context, location);
    // }

    fn visit_terminator_kind(&mut self, kind: &TerminatorKind<'tcx>, location: Location) {
        trace!("{:?}", kind);
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
                                &function.describe_as_module(self.tcx),
                                args,
                                destination.as_ref().expect("diverging foreign function"),
                                *cleanup,
                            )
                            .expect("unknown foreign item");
                    } else {
                        let start_place = function!(self)
                            .function_call_start_place()
                            .expect("Unable to infer start place of function call")
                            .clone();
                        let (return_place, return_block) =
                            destination.as_ref().expect("diverging function");
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
                            .map(|operand| {
                                let mir_local = match operand {
                                    mir::Operand::Copy(place) | mir::Operand::Move(place) => {
                                        place.local_or_deref_local().expect("deref argument failed")
                                    }
                                    Operand::Constant(_) => panic!("unexpected constant argument"),
                                };
                                *stack_top.get_local(&mir_local).expect("argument not found")
                            })
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
                    //error!("skipped {}", function.describe_as_module(self.tcx));
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
                ref msg,
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
                    .resume(
                        net,
                        self.unwind_abort_place,
                        self.program_end_place.expect("missing program end place"),
                    )
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

    fn visit_assert_message(&mut self, msg: &AssertMessage<'tcx>, location: Location) {
        self.super_assert_message(msg, location);
    }

    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        // trace!("Constant: {:?}", constant);
        self.super_constant(constant, location);
    }

    // fn visit_span(&mut self,
    //               span: &Span) {
    //     self.super_span(span);
    // }

    fn visit_source_info(&mut self, source_info: &SourceInfo) {
        self.super_source_info(source_info);
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>, _: TyContext) {
        self.super_ty(ty);
    }

    fn visit_user_type_projection(&mut self, ty: &UserTypeProjection) {
        self.super_user_type_projection(ty);
    }

    // fn visit_user_type_annotation(
    //     &mut self,
    //     index: UserTypeAnnotationIndex,
    //     ty: &CanonicalUserTypeAnnotation<'tcx>,
    // ) {
    //     self.super_user_type_annotation(index, ty);
    // }

    fn visit_region(&mut self, region: &ty::Region<'tcx>, _: Location) {
        self.super_region(region);
    }

    fn visit_const(&mut self, constant: &&'tcx ty::Const<'tcx>, _: Location) {
        // trace!("Const: {:?}", constant);
        self.super_const(constant);
    }

    // fn visit_substs(&mut self,
    //                 substs: &SubstsRef<'tcx>,
    //                 _: Location) {
    //     self.super_substs(substs);
    // }

    fn visit_local_decl(&mut self, local: mir::Local, local_decl: &LocalDecl<'tcx>) {
        self.super_local_decl(local, local_decl);
    }

    fn visit_source_scope(&mut self, scope: &SourceScope) {
        self.super_source_scope(scope);
    }
}

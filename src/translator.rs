use crate::petri_net::function::{Data, Function};
use pnml::{NodeRef, PNMLDocument, PageRef, PetriNetRef, Result};
use rustc::hir::def_id::DefId;
use rustc::mir::visit::Visitor;
use rustc::mir::visit::*;
use rustc::mir::{self, *};
use rustc::ty::{self, Ty, TyCtxt};

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
    pnml_doc: PNMLDocument,
    net_ref: PetriNetRef,
    root_page: PageRef,
    unwind_abort_place: NodeRef,
    program_end_place: Option<NodeRef>,
}

macro_rules! net {
    ($translator:ident) => {
        $translator
            .pnml_doc
            .petri_net_data($translator.net_ref)
            .expect("corrupted net reference")
    };
}

macro_rules! function {
    ($translator:ident) => {
        $translator.call_stack.peek_mut().expect("empty call stack")
    };
}

impl<'tcx> Translator<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Result<Self> {
        let mut pnml_doc = PNMLDocument::new();
        //TODO: find a descriptive name e.g. name of the program
        let net_ref = pnml_doc.add_petri_net(None);
        let net = pnml_doc
            .petri_net_data(net_ref)
            .expect("corrupted net reference");
        let root_page = net.add_page(Some("entry"));
        let mut unwind_abort_place = net.add_place(&root_page)?;
        unwind_abort_place.name(net, "unwind_abort")?;
        Ok(Translator {
            tcx,
            call_stack: CallStack::new(),
            pnml_doc,
            net_ref,
            root_page,
            unwind_abort_place,
            program_end_place: None,
        })
    }

    pub fn petrify(&mut self, main_fn: DefId) -> Result<String> {
        let start_place = {
            let net = net!(self);
            let mut place = net.add_place(&self.root_page)?;
            place.name(net, "program_start")?;
            place.initial_marking(net, 1)?;
            place
        };
        self.program_end_place = {
            let net = net!(self);
            let mut place = net.add_place(&self.root_page)?;
            place.name(net, "program_end")?;
            place.initial_marking(net, 1)?;
            Some(place)
        };
        self.translate(main_fn, start_place)?;
        Ok(self.pnml_doc.to_xml()?)
    }

    fn translate<'a>(&mut self, function: DefId, start_place: NodeRef) -> Result<()> {
        let fn_name = function.describe_as_module(self.tcx);
        info!("\n\nENTERING function: {:?}", fn_name);
        let body = self.tcx.optimized_mir(function);
        let (const_memory, mut static_memory) = if self.call_stack.is_empty() {
            (
                Data::Constant(net!(self).add_place(&self.root_page)?),
                std::collections::HashMap::new(),
            )
        } else {
            (
                function!(self).constants().clone(),
                function!(self).statics().clone(),
            )
        };
        // add missing promoted statics
        for (promoted, _) in self.tcx.promoted_mir(function).iter_enumerated() {
            if static_memory.get(&promoted).is_none() {
                let promoted_node = net!(self).add_place(&self.root_page)?;
                static_memory.insert(promoted, Data::Static(promoted_node));
            } else {
                warn!("duplicate of promoted static");
            }
        }
        let petri_function = Function::new(
            function,
            body,
            net!(self),
            start_place,
            &const_memory,
            &static_memory,
            &fn_name,
        )?;
        self.call_stack.push(petri_function);
        self.visit_body(body);
        self.call_stack.pop();
        info!("\nLEAVING function: {:?}\n", fn_name);
        Ok(())
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
        function!(self)
            .add_locals(net!(self), &body.local_decls)
            .expect("cannot add locals to petri net function");
        self.super_body(body);
    }

    fn visit_basic_block_data(&mut self, block: BasicBlock, data: &BasicBlockData<'tcx>) {
        trace!("---BasicBlock {:?}---", block);
        function!(self)
            .activate_block(net!(self), &block)
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
        match kind {
            Return => {
                // trace!("Return");
                function!(self).retorn(net).expect("return failed");
            }

            Goto { target } => {
                // trace!("Goto");
                function!(self)
                    .goto(net, target)
                    .expect("Goto Block failed");
            }

            SwitchInt {
                discr,
                switch_ty,
                values,
                targets,
            } => function!(self)
                .switch_int(net, targets)
                .expect("switch int failed"),

            Call {
                ref func,
                //args,
                //destination,
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
                if self.tcx.is_foreign_item(function) {
                    error!("found foreign item: {:?}", function);
                } else {
                    if !skip_function(self.tcx, function) {
                        if !self.tcx.is_mir_available(function) {
                            error!("Could not find mir: {:?}", function);
                        } else {
                            let start_place = function!(self)
                                .function_call_start_place()
                                .expect("Unable to infer start place of function call")
                                .clone();
                            self.translate(function, start_place)
                                .expect("translation error");
                        }
                    }
                }
            }

            Drop {
                location,
                target,
                unwind,
            } => function!(self)
                .drop(net, target, unwind)
                .expect("drop failed"),

            Assert { .. } => panic!("assert"),

            Yield { .. } => panic!("Yield"),
            GeneratorDrop => panic!("GeneratorDrop"),
            DropAndReplace { .. } => warn!("DropAndReplace"),
            Resume => {
                function!(self)
                    .resume(
                        net,
                        &self.unwind_abort_place,
                        self.program_end_place
                            .as_ref()
                            .expect("missing program end place"),
                    )
                    .expect("resume failed");
            }
            Abort => panic!("Abort"),
            FalseEdges { .. } => bug!(
                "should have been eliminated by\
                 `simplify_branches` mir pass"
            ),
            FalseUnwind { .. } => bug!(
                "should have been eliminated by\
                 `simplify_branches` mir pass"
            ),
            Unreachable => error!("unreachable"),
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

    fn visit_local_decl(&mut self, local: Local, local_decl: &LocalDecl<'tcx>) {
        self.super_local_decl(local, local_decl);
    }

    fn visit_source_scope(&mut self, scope: &SourceScope) {
        self.super_source_scope(scope);
    }
}

fn skip_function<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> bool {
    //FIXME: check if a call for a panic always result in a panic (it might be caught later)
    if tcx.lang_items().items().contains(&Some(def_id)) {
        debug!("LangItem: {:?}", def_id);
    };
    if Some(def_id) == tcx.lang_items().panic_fn() {
        trace!("panic");
        return true;
    }
    let description = def_id.describe_as_module(tcx);
    if description.contains("std::rt::begin_panic_fmt") {
        true
    } else if description.contains("std::panicking::panicking") {
        true
    } else {
        false
    }
}

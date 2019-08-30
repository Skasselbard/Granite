use rustc::hir::def_id::DefId;
use rustc::mir::visit::Visitor;
use rustc::mir::visit::*;
use rustc::mir::{self, *};
use rustc::ty::{self, ClosureSubsts, GeneratorSubsts, Ty, TyCtxt};

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
}

pub struct Translator<'tcx> {
    tcx: TyCtxt<'tcx>,
    call_stack: CallStack<(DefId, &'tcx Body<'tcx>)>,
}

impl<'tcx> Translator<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Translator {
            tcx,
            call_stack: CallStack::new(),
        }
    }
    pub fn translate<'a>(&mut self, function: DefId) {
        info!(
            "ENTERING function: {:?}",
            function.describe_as_module(self.tcx)
        );
        let body = self.tcx.optimized_mir(function);
        self.call_stack.push((function, body));
        self.visit_body(body);
        self.call_stack.pop();
        info!(
            "LEAVING function: {:?}",
            function.describe_as_module(self.tcx)
        );
    }
}

impl<'tcx> Visitor<'tcx> for Translator<'tcx> {
    fn visit_body(&mut self, body: &Body<'tcx>) {
        match body.phase {
            MirPhase::Optimized => {
                trace!("source scopes: {:?}", body.source_scopes);
                trace!(
                    "source scopes local data: {:?}",
                    body.source_scope_local_data
                );
                //trace!("promoted: {:?}", entry_body.promoted);
                trace!("return type: {:?}", body.return_ty());
                trace!("yield type: {:?}", body.yield_ty);
                trace!("generator drop: {:?}", body.generator_drop);
                //trace!("local declarations: {:?}", body.local_decls());
            }
            _ => error!("tried to translate unoptimized MIR"),
        }
        self.super_body(body);
    }

    fn visit_basic_block_data(&mut self, block: BasicBlock, data: &BasicBlockData<'tcx>) {
        trace!("\n---BasicBlock {:?}---", block);
        self.super_basic_block_data(block, data)
    }
    fn visit_source_scope_data(&mut self, scope_data: &SourceScopeData) {
        self.super_source_scope_data(scope_data);
    }

    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        trace!("{:?}: ", statement.kind);
        self.super_statement(statement, location);
    }

    //Begin statement visits//
    fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
        //trace!("{:?} = {:?}", place, rvalue);
        self.super_assign(place, rvalue, location);
    }

    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
        self.super_place(place, context, location);
    }

    fn visit_local(&mut self, _local: &Local, _context: PlaceContext, _location: Location) {}

    fn visit_retag(&mut self, kind: &RetagKind, place: &Place<'tcx>, location: Location) {
        trace!("{:?}@{:?}", kind, place);
        self.super_retag(kind, place, location);
    }

    fn visit_ascribe_user_ty(
        &mut self,
        place: &Place<'tcx>,
        variance: &ty::Variance,
        user_ty: &UserTypeProjection,
        location: Location,
    ) {
        self.super_ascribe_user_ty(place, variance, user_ty, location);
    }
    //End statement visists

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        //trace!("{:?}", terminator);
        self.super_terminator(terminator, location);
    }

    fn visit_terminator_kind(&mut self, kind: &TerminatorKind<'tcx>, location: Location) {
        trace!("{:?}", kind);
        use rustc::mir::TerminatorKind::*;
        match kind {
            Return => trace!("Return"),

            Goto { .. } => trace!("Goto"),

            SwitchInt { .. } => trace!("SwitchInt"),

            Call { ref func, .. } => {
                // info!(
                //     "functionCall\nfunc: {:?}\nargs: {:?}\ndest: {:?}",
                //     func, args, destination
                // );
                let sty = {
                    match func {
                        Operand::Copy(ref place) | Operand::Move(ref place) => {
                            let (_, body) = self.call_stack.peek().expect("peeked empty stack");
                            let decls = body.local_decls();
                            let place_ty: &mir::tcx::PlaceTy<'tcx> = &place.base.ty(decls);
                            place_ty.ty
                        }
                        Operand::Constant(ref constant) => &constant.ty,
                    }
                };
                let function = match sty.sty {
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
                    warn!("found foreign item: {:?}", function);
                //panic!("");
                //emulate_foreign_item(ecx, instance, args, dest, ret);
                } else {
                    if !skip_function(self.tcx, function) {
                        if !self.tcx.is_mir_available(function) {
                            warn!("Could not find mir: {:?}", function);
                        } else {
                            self.translate(function);
                        }
                    }
                }
            }

            Drop { .. } => {}

            Assert { .. } => warn!("assert"),

            Yield { .. } => warn!("Yield"),
            GeneratorDrop => warn!("GeneratorDrop"),
            DropAndReplace { .. } => warn!("DropAndReplace"),
            Resume => warn!("Resume"),
            Abort => warn!("Abort"),
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

    fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
        self.super_rvalue(rvalue, location);
    }

    fn visit_operand(&mut self, operand: &Operand<'tcx>, location: Location) {
        self.super_operand(operand, location);
    }

    fn visit_place_base(
        &mut self,
        place_base: &PlaceBase<'tcx>,
        context: PlaceContext,
        location: Location,
    ) {
        self.super_place_base(place_base, context, location);
    }

    // fn visit_projection(&mut self,
    //                     place: &Projection<'tcx>,
    //                     context: PlaceContext,
    //                     location: Location) {
    //     self.super_projection(place, context, location);
    // }

    fn visit_constant(&mut self, constant: &Constant<'tcx>, location: Location) {
        trace!("Constant: {:?}", constant);
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
        trace!("Const: {:?}", constant);
        self.super_const(constant);
    }

    // fn visit_substs(&mut self,
    //                 substs: &SubstsRef<'tcx>,
    //                 _: Location) {
    //     self.super_substs(substs);
    // }

    fn visit_closure_substs(&mut self, substs: &ClosureSubsts<'tcx>, _: Location) {
        self.super_closure_substs(substs);
    }

    fn visit_generator_substs(&mut self, substs: &GeneratorSubsts<'tcx>, _: Location) {
        self.super_generator_substs(substs);
    }

    fn visit_local_decl(&mut self, local: Local, local_decl: &LocalDecl<'tcx>) {
        self.super_local_decl(local, local_decl);
    }

    fn visit_source_scope(&mut self, scope: &SourceScope) {
        self.super_source_scope(scope);
    }
}

fn skip_function<'tcx>(tcx: TyCtxt<'tcx>, def_id: DefId) -> bool {
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

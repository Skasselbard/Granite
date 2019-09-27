// compiler crates
use rustc::mir::{self, interpret::InterpResult};
use rustc::ty::layout::{self, Align, LayoutOf, Size};
use rustc::ty::{self, TyCtxt};
use rustc_mir::interpret::{
    Immediate, InterpCx, InterpError, MPlaceTy, Machine, MemoryKind, OpTy, PlaceTy,
    PointerArithmetic, Scalar, StackPopCleanup,
};
use syntax::attr;
use syntax::symbol::sym;

fn log_unverified(name: &str) {
    warn!("entered unverified foreign function: {}", name);
}

pub fn call_intrinsic<'tcx, 'mir>(
    ecx: &mut InterpCx<'mir, 'tcx, PetriTranslator>,
    instance: ty::Instance<'tcx>,
    args: &[OpTy<'tcx, PointerTag>],
    dest: PlaceTy<'tcx, PointerTag>,
) -> InterpResult<'tcx> {
    // skeleton copied from miri https://github.com/rust-lang/miri/blob/master/src/shims/intrinsics.rs
    let substs = instance.substs;
    let intrinsic_name = ecx.tcx.item_name(instance.def_id()).as_str();
    let tcx = ecx.tcx;
    match intrinsic_name.get() {
        "panic_if_uninhabited" => {
            let ty = substs.type_at(0);
            let layout = ecx.layout_of(ty)?;
            if layout.abi.is_uninhabited() {
                throw_ub_format!("Trying to instantiate uninhabited type {}", ty)
            }
        }
        "init" => {
            // Check fast path: we don't want to force an allocation in case the destination is a simple value,
            // but we also do not want to create a new allocation with 0s and then copy that over.
            // FIXME: We do not properly validate in case of ZSTs and when doing it in memory!
            // However, this only affects direct calls of the intrinsic; calls to the stable
            // functions wrapping them do get their validation.
            // FIXME: should we check that the destination pointer is aligned even for ZSTs?
            if !dest.layout.is_zst() {
                match dest.layout.abi {
                    layout::Abi::Scalar(ref s) => {
                        let x = Scalar::from_int(0, s.value.size(ecx));
                        ecx.write_scalar(x, dest)?;
                    }
                    layout::Abi::ScalarPair(ref s1, ref s2) => {
                        let x = Scalar::from_int(0, s1.value.size(ecx));
                        let y = Scalar::from_int(0, s2.value.size(ecx));
                        ecx.write_immediate(Immediate::ScalarPair(x.into(), y.into()), dest)?;
                    }
                    _ => {
                        // Do it in memory
                        let mplace = ecx.force_allocation(dest)?;
                        assert!(mplace.meta.is_none());
                        // not a zst, must be valid pointer
                        let ptr = mplace.ptr.to_ptr()?;
                        ecx.memory_mut().get_mut(ptr.alloc_id)?.write_repeat(
                            &tcx.tcx,
                            ptr,
                            0,
                            dest.layout.size,
                        )?;
                    }
                }
            }
        }
        "arith_offset"
        | "assume"
        | "volatile_load"
        | "volatile_store"
        | "atomic_load"
        | "atomic_load_relaxed"
        | "atomic_load_acq"
        | "atomic_store"
        | "atomic_store_relaxed"
        | "atomic_store_rel"
        | "atomic_fence_acq"
        | "atomic_or"
        | "atomic_or_acq"
        | "atomic_or_rel"
        | "atomic_or_acqrel"
        | "atomic_or_relaxed"
        | "atomic_xor"
        | "atomic_xor_acq"
        | "atomic_xor_rel"
        | "atomic_xor_acqrel"
        | "atomic_xor_relaxed"
        | "atomic_and"
        | "atomic_and_acq"
        | "atomic_and_rel"
        | "atomic_and_acqrel"
        | "atomic_and_relaxed"
        | "atomic_nand"
        | "atomic_nand_acq"
        | "atomic_nand_rel"
        | "atomic_nand_acqrel"
        | "atomic_nand_relaxed"
        | "atomic_xadd"
        | "atomic_xadd_acq"
        | "atomic_xadd_rel"
        | "atomic_xadd_acqrel"
        | "atomic_xadd_relaxed"
        | "atomic_xsub"
        | "atomic_xsub_acq"
        | "atomic_xsub_rel"
        | "atomic_xsub_acqrel"
        | "atomic_xsub_relaxed"
        | "breakpoint"
        | "copy"
        | "copy_nonoverlapping"
        | "discriminant_value"
        | "sinf32"
        | "fabsf32"
        | "cosf32"
        | "sqrtf32"
        | "expf32"
        | "exp2f32"
        | "logf32"
        | "log10f32"
        | "log2f32"
        | "floorf32"
        | "ceilf32"
        | "truncf32"
        | "roundf32"
        | "sinf64"
        | "fabsf64"
        | "cosf64"
        | "sqrtf64"
        | "expf64"
        | "exp2f64"
        | "logf64"
        | "log10f64"
        | "log2f64"
        | "floorf64"
        | "ceilf64"
        | "truncf64"
        | "roundf64"
        | "fadd_fast"
        | "fsub_fast"
        | "fmul_fast"
        | "fdiv_fast"
        | "frem_fast"
        | "minnumf32"
        | "maxnumf32"
        | "minnumf64"
        | "maxnumf64"
        | "exact_div"
        | "forget"
        | "likely"
        | "unlikely"
        | "pref_align_of"
        | "move_val_init"
        | "offset"
        | "powf32"
        | "powf64"
        | "fmaf32"
        | "fmaf64"
        | "powif32"
        | "powif64"
        | "size_of_val"
        | "min_align_of_val"
        | "align_of_val"
        | "unchecked_div"
        | "unchecked_rem"
        | "unchecked_add"
        | "unchecked_sub"
        | "unchecked_mul"
        | "uninit"
        | "write_bytes"
        | _ => {
            error!("unimplemented intrinsic: {}", intrinsic_name);
            panic!("");
        }
    }
    Ok(())
}

/// Emulates calling a foreign item, failing if the item is not supported.
/// skeleton from miri:
/// https://github.com/rust-lang/miri/blob/master/src/shims/foreign_items.rs
pub fn emulate_foreign_item<'mir, 'tcx>(
    ecx: &mut InterpCx<'mir, 'tcx, PetriTranslator>,
    instance: ty::Instance<'tcx>,
    args: &[OpTy<'tcx, PointerTag>],
    dest: Option<PlaceTy<'tcx, PointerTag>>,
    ret: Option<mir::BasicBlock>,
) -> InterpResult<'tcx> {
    let tcx = ecx.tcx;
    let attrs = tcx.get_attrs(instance.def_id());
    let link_name = match attr::first_attr_value_str_by_name(&attrs, sym::link_name) {
        Some(name) => name.as_str(),
        None => tcx.item_name(instance.def_id()).as_str(),
    };
    // Strip linker suffixes (seen on 32-bit macOS).
    let link_name = link_name.get().trim_end_matches("$UNIX2003");
    let tcx = &tcx;

    // First: functions that diverge.
    match link_name {
        "__rust_start_panic" | "panic_impl" => {
            throw_unsup_format!("the evaluated program panicked");
        }
        "exit" | "ExitProcess" => {
            // it's really u32 for ExitProcess, but we have to put it into the `Exit` error variant anyway
            let code = ecx.read_scalar(args[0])?.to_i32()?;
            return Err(InterpError::Exit(code).into());
        }
        _ => {
            if dest.is_none() {
                throw_unsup_format!("can't call (diverging) foreign function: {}", link_name);
            }
        }
    }

    // Next: functions that assume a ret and dest.
    let dest = dest.expect("we already checked for a dest");
    let ret = ret.expect("dest is `Some` but ret is `None`");
    match link_name {
        "malloc" => {
            let size = ecx.read_scalar(args[0])?.to_usize(ecx)?;
            let res = malloc(
                ecx,
                size,
                /*zero_init:*/ false,
                PetriMemoryKind::Dynamic.into(),
            );
            ecx.write_scalar(res, dest)?;
        }
        "calloc" => {
            let items = ecx.read_scalar(args[0])?.to_usize(ecx)?;
            let len = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            let size = items
                .checked_mul(len)
                .ok_or_else(|| err_panic!(Overflow(mir::BinOp::Mul)))?;
            let res = malloc(
                ecx,
                size,
                /*zero_init:*/ true,
                PetriMemoryKind::Dynamic.into(),
            );
            ecx.write_scalar(res, dest)?;
        }
        "posix_memalign" => {
            error!("foreign posix_memalign is not implemented");
            panic!("");
            // let ret = ecx.deref_operand(args[0])?;
            // let align = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            // let size = ecx.read_scalar(args[2])?.to_usize(ecx)?;
            // // Align must be power of 2, and also at least ptr-sized (POSIX rules).
            // if !align.is_power_of_two() {
            //     throw_unsup!(HeapAllocNonPowerOfTwoAlignment(align));
            // }
            // if align < ecx.pointer_size().bytes() {
            //     throw_ub_format!(
            //         "posix_memalign: alignment must be at least the size of a pointer, but is {}",
            //         align,
            //     );
            // }

            // if size == 0 {
            //     ecx.write_null(ret.into())?;
            // } else {
            //     let ptr = ecx.memory_mut().allocate(
            //         Size::from_bytes(size),
            //         Align::from_bytes(align).unwrap(),
            //         PetriMemoryKind::Dynamic.into(),
            //     );
            //     ecx.write_scalar(Scalar::Ptr(ptr), ret.into())?;
            // }
            // ecx.write_null(dest)?;
        }
        "free" => {
            let ptr = ecx.read_scalar(args[0])?.not_undef()?;
            free(ecx, ptr, PetriMemoryKind::Dynamic.into())?;
        }
        "realloc" => {
            let old_ptr = ecx.read_scalar(args[0])?.not_undef()?;
            let new_size = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            let res = realloc(ecx, old_ptr, new_size, PetriMemoryKind::Dynamic.into())?;
            ecx.write_scalar(res, dest)?;
        }

        "__rust_alloc" => {
            let size = ecx.read_scalar(args[0])?.to_usize(ecx)?;
            let align = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            if size == 0 {
                throw_unsup!(HeapAllocZeroBytes);
            }
            if !align.is_power_of_two() {
                throw_unsup!(HeapAllocNonPowerOfTwoAlignment(align));
            }
            let ptr = ecx.memory_mut().allocate(
                Size::from_bytes(size),
                Align::from_bytes(align).unwrap(),
                PetriMemoryKind::Dynamic.into(),
            );
            ecx.write_scalar(Scalar::Ptr(ptr), dest)?;
        }
        "__rust_alloc_zeroed" => {
            let size = ecx.read_scalar(args[0])?.to_usize(ecx)?;
            let align = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            if size == 0 {
                throw_unsup!(HeapAllocZeroBytes);
            }
            if !align.is_power_of_two() {
                throw_unsup!(HeapAllocNonPowerOfTwoAlignment(align));
            }
            let ptr = ecx.memory_mut().allocate(
                Size::from_bytes(size),
                Align::from_bytes(align).unwrap(),
                PetriMemoryKind::Dynamic.into(),
            );
            // We just allocated ecx, the access cannot fail
            let x = ecx
                .memory_mut()
                .get_mut(ptr.alloc_id)
                .unwrap()
                .write_repeat(&tcx.tcx, ptr, 0, Size::from_bytes(size))
                .unwrap();
            ecx.write_scalar(Scalar::Ptr(ptr), dest)?;
        }
        "__rust_dealloc" => {
            let ptr = ecx.read_scalar(args[0])?.not_undef()?;
            let old_size = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            let align = ecx.read_scalar(args[2])?.to_usize(ecx)?;
            if old_size == 0 {
                throw_unsup!(HeapAllocZeroBytes);
            }
            if !align.is_power_of_two() {
                throw_unsup!(HeapAllocNonPowerOfTwoAlignment(align));
            }
            let ptr = ecx.force_ptr(ptr)?;
            ecx.memory_mut().deallocate(
                ptr,
                Some((
                    Size::from_bytes(old_size),
                    Align::from_bytes(align).unwrap(),
                )),
                PetriMemoryKind::Dynamic.into(),
            )?;
        }
        "__rust_realloc" => {
            let ptr = ecx.read_scalar(args[0])?.to_ptr()?;
            let old_size = ecx.read_scalar(args[1])?.to_usize(ecx)?;
            let align = ecx.read_scalar(args[2])?.to_usize(ecx)?;
            let new_size = ecx.read_scalar(args[3])?.to_usize(ecx)?;
            if old_size == 0 || new_size == 0 {
                throw_unsup!(HeapAllocZeroBytes);
            }
            if !align.is_power_of_two() {
                throw_unsup!(HeapAllocNonPowerOfTwoAlignment(align));
            }
            let align = Align::from_bytes(align).unwrap();
            let new_ptr = ecx.memory_mut().reallocate(
                ptr,
                Some((Size::from_bytes(old_size), align)),
                Size::from_bytes(new_size),
                align,
                PetriMemoryKind::Dynamic.into(),
            )?;
            ecx.write_scalar(Scalar::Ptr(new_ptr), dest)?;
        }

        "__rust_maybe_catch_panic" => {
            // fn __rust_maybe_catch_panic(
            //     f: fn(*mut u8),
            //     data: *mut u8,
            //     data_ptr: *mut usize,
            //     vtable_ptr: *mut usize,
            // ) -> u32
            // We abort on panic, so not much is going on here, but we still have to call the closure.
            let f = ecx.read_scalar(args[0])?.not_undef()?;
            let data = ecx.read_scalar(args[1])?.not_undef()?;
            let f_instance = ecx.memory().get_fn(f)?.as_instance()?;
            ecx.write_scalar(Scalar::from_int(0, dest.layout.size), dest)?;
            trace!("__rust_maybe_catch_panic: {:?}", f_instance);

            // Now we make a function call.
            // TODO: consider making ecx reusable? `InterpCx::step` does something similar
            // for the TLS destructors, and of course `eval_main`.
            let mir = ecx.load_mir(f_instance.def)?;
            let ret_place = MPlaceTy::dangling(ecx.layout_of(ecx.tcx.mk_unit())?, ecx).into();
            ecx.push_stack_frame(
                f_instance,
                mir.span,
                mir,
                Some(ret_place),
                // Directly return to caller.
                StackPopCleanup::Goto(Some(ret)),
            )?;
            let mut args = ecx.frame().body.args_iter();

            let arg_local = args
                .next()
                .expect("Argument to __rust_maybe_catch_panic does not take enough arguments.");
            let arg_dest = local_place(ecx, arg_local)?;
            ecx.write_scalar(data, arg_dest)?;

            assert!(
                args.next().is_none(),
                "__rust_maybe_catch_panic argument has more arguments than expected"
            );

            // We ourselves will return `0`, eventually (because we will not return if we paniced).
            ecx.write_scalar(Scalar::from_int(0, dest.layout.size), dest)?;

            // Don't fall through, we do *not* want to `goto_block`!
            return Ok(());
        }

        "signal" | "sigaction" | "sigaltstack" => {
            log_unverified(link_name);
            ecx.write_scalar(Scalar::from_int(0, dest.layout.size), dest)?;
        }

        "sysconf" => {
            let r = unsafe { libc::sysconf(ecx.read_scalar(args[0])?.to_i32()?) };
            let r = Scalar::from_int(r, dest.layout.size);
            ecx.write_scalar(r, dest)?;
        }

        // http://man7.org/linux/man-pages/man3/pthread_attr_init.3.html#ATTRIBUTES
        // The pthread_attr_init() function initializes the thread attributes
        // object pointed to by attr with default attribute values.
        "pthread_attr_init"
        | "pthread_attr_destroy"
        // http://man7.org/linux/man-pages/man3/pthread_self.3.html
        // The pthread_self() function returns the ID of the calling thread.
        // This is the same value that is returned in *thread in the
        // pthread_create(3) call that created this thread.
        | "pthread_self"
        | "pthread_attr_setstacksize" => {
            log_unverified(link_name);
            ecx.write_scalar(Scalar::from_int(0, dest.layout.size), dest)?;
        }

        "pthread_attr_getstack" => {
            log_unverified(link_name);
            warn!("STACK_SIZE and STACK_ADDR are still hardcoded");
            const STACK_ADDR: u64 = 0;
            const STACK_SIZE: u64 = 1024*1024*100;//100mb

            let addr_place = ecx.deref_operand(args[1])?;
            let size_place = ecx.deref_operand(args[2])?;

            ecx.write_scalar(
                Scalar::from_uint(STACK_ADDR, addr_place.layout.size),
                addr_place.into(),
            )?;
            ecx.write_scalar(
                Scalar::from_uint(STACK_SIZE, size_place.layout.size),
                size_place.into(),
            )?;

            // Return success (`0`).
            ecx.write_scalar(Scalar::from_int(0, dest.layout.size), dest)?;
        }

        "pthread_attr_get_np" | "pthread_getattr_np" => {
            log_unverified(link_name);
            ecx.write_scalar(Scalar::from_int(0, dest.layout.size), dest)?;
        }
        // "pthread_get_stackaddr_np" => {
        //     log_unverified(link_name);
        //     let stack_addr = Scalar::from_uint(STACK_ADDR, dest.layout.size);
        //     ecx.write_scalar(stack_addr, dest)?;
        // }
        // "pthread_get_stacksize_np" => {
        //     log_unverified(link_name);
        //     let stack_size = Scalar::from_uint(STACK_SIZE, dest.layout.size);
        //     ecx.write_scalar(stack_size, dest)?;
        // }

        "syscall"
        | "getrandom"
        | "dlsym"
        | "memcmp"
        | "memrchr"
        | "memchr"
        | "getenv"
        | "unsetenv"
        | "setenv"
        | "write"
        | "strlen"
        | "cbrtf"
        | "coshf"
        | "sinhf"
        | "tanf"
        | "_hypotf"
        | "hypotf"
        | "atan2f"
        | "cbrt"
        | "cosh"
        | "sinh"
        | "tan"
        | "_hypot"
        | "hypot"
        | "atan2"
        | "_ldexp"
        | "ldexp"
        | "scalbn"
        | "sched_getaffinity"
        | "isatty"
        | "pthread_key_create"
        | "pthread_key_delete"
        | "pthread_getspecific"
        | "pthread_setspecific"
        | "pthread_attr_getstack"
        | "pthread_create"
        | "CreateThread"
        | "pthread_mutexattr_init"
        | "pthread_mutexattr_settype"
        | "pthread_mutex_init"
        | "pthread_mutexattr_destroy"
        | "pthread_mutex_lock"
        | "pthread_mutex_unlock"
        | "pthread_mutex_destroy"
        | "pthread_rwlock_rdlock"
        | "pthread_rwlock_unlock"
        | "pthread_rwlock_wrlock"
        | "pthread_rwlock_destroy"
        | "pthread_condattr_init"
        | "pthread_condattr_setclock"
        | "pthread_cond_init"
        | "pthread_condattr_destroy"
        | "pthread_cond_destroy"
        | "pthread_atfork"
        | "mmap"
        | "mprotect"
        | "pthread_attr_get_np"
        | "pthread_getattr_np"
        | "pthread_get_stackaddr_np"
        | "pthread_get_stacksize_np"
        | "_tlv_atexit"
        | "_NSGetArgc"
        | "_NSGetArgv"
        | "SecRandomCopyBytes"
        | "GetProcessHeap"
        | "HeapAlloc"
        | "HeapFree"
        | "HeapReAlloc"
        | "SetLastError"
        | "GetLastError"
        | "AddVectoredExceptionHandler"
        | "InitializeCriticalSection"
        | "EnterCriticalSection"
        | "LeaveCriticalSection"
        | "DeleteCriticalSection"
        | "GetModuleHandleW"
        | "GetProcAddress"
        | "TryEnterCriticalSection"
        | "GetConsoleScreenBufferInfo"
        | "SetConsoleTextAttribute"
        | "GetSystemInfo"
        | "TlsAlloc"
        | "TlsGetValue"
        | "TlsSetValue"
        | "GetStdHandle"
        | "WriteFile"
        | "GetConsoleMode"
        | "GetEnvironmentVariableW"
        | "GetCommandLineW"
        | "SystemFunction036" => {
            error!(
                "not implemented foreign but miri had an entry: {:?}",
                link_name
            );
            panic!("")
        }
        _ => {
            error!("not implemented foreign: {:?}", link_name);
            panic!("")
        }
    }

    ecx.goto_block(Some(ret))?;
    ecx.dump_place(*dest);
    Ok(())
}

fn min_align<'mir, 'tcx, M: Machine<'mir, 'tcx>>(
    ecx: &InterpCx<'mir, 'tcx, M>,
    size: u64,
    kind: MemoryKind<M::MemoryKinds>,
) -> Align {
    // List taken from `libstd/sys_common/alloc.rs`.
    let min_align = match ecx.tcx.tcx.sess.target.target.arch.as_str() {
        "x86" | "arm" | "mips" | "powerpc" | "powerpc64" | "asmjs" | "wasm32" => 8,
        "x86_64" | "aarch64" | "mips64" | "s390x" | "sparc64" => 16,
        arch => bug!("Unsupported target architecture: {}", arch),
    };
    // Windows always aligns, even small allocations.
    // Source: <https://support.microsoft.com/en-us/help/286470/how-to-use-pageheap-exe-in-windows-xp-windows-2000-and-windows-server>
    // But jemalloc does not, so for the C heap we only align if the allocation is sufficiently big.
    if size >= min_align {
        return Align::from_bytes(min_align).unwrap();
    }
    // We have `size < min_align`. Round `size` *down* to the next power of two and use that.
    fn prev_power_of_two(x: u64) -> u64 {
        let next_pow2 = x.next_power_of_two();
        if next_pow2 == x {
            // x *is* a power of two, just use that.
            x
        } else {
            // x is between two powers, so next = 2*prev.
            next_pow2 / 2
        }
    }
    Align::from_bytes(prev_power_of_two(size)).unwrap()
}

fn malloc<'mir, 'tcx, M: Machine<'mir, 'tcx>>(
    ecx: &mut InterpCx<'mir, 'tcx, M>,
    size: u64,
    zero_init: bool,
    kind: MemoryKind<M::MemoryKinds>,
) -> Scalar<M::PointerTag> {
    let tcx = &{ ecx.tcx.tcx };
    if size == 0 {
        Scalar::from_int(0, ecx.pointer_size())
    } else {
        let align = min_align(&ecx, size, kind);
        let ptr = ecx
            .memory_mut()
            .allocate(Size::from_bytes(size), align, kind.into());
        if zero_init {
            // We just allocated ecx, the access cannot fail
            ecx.memory_mut()
                .get_mut(ptr.alloc_id)
                .unwrap()
                .write_repeat(tcx, ptr, 0, Size::from_bytes(size))
                .unwrap();
        }
        Scalar::Ptr(ptr)
    }
}

fn free<'mir, 'tcx, M: Machine<'mir, 'tcx>>(
    ecx: &mut InterpCx<'mir, 'tcx, M>,
    ptr: Scalar<M::PointerTag>,
    kind: MemoryKind<M::MemoryKinds>,
) -> InterpResult<'tcx> {
    if !is_null(ecx, ptr)? {
        let ptr = ecx.force_ptr(ptr)?;
        ecx.memory_mut().deallocate(ptr, None, kind.into())?;
    }
    Ok(())
}

fn realloc<'mir, 'tcx, M: Machine<'mir, 'tcx>>(
    ecx: &mut InterpCx<'mir, 'tcx, M>,
    old_ptr: Scalar<M::PointerTag>,
    new_size: u64,
    kind: MemoryKind<M::MemoryKinds>,
) -> InterpResult<'tcx, Scalar<M::PointerTag>> {
    let new_align = min_align(ecx, new_size, kind);
    if is_null(ecx, old_ptr)? {
        if new_size == 0 {
            Ok(Scalar::from_int(0, ecx.pointer_size()))
        } else {
            let new_ptr =
                ecx.memory_mut()
                    .allocate(Size::from_bytes(new_size), new_align, kind.into());
            Ok(Scalar::Ptr(new_ptr))
        }
    } else {
        let old_ptr = ecx.force_ptr(old_ptr)?;
        let memory = ecx.memory_mut();
        if new_size == 0 {
            memory.deallocate(old_ptr, None, kind.into())?;
            Ok(Scalar::from_int(0, ecx.pointer_size()))
        } else {
            let new_ptr = memory.reallocate(
                old_ptr,
                None,
                Size::from_bytes(new_size),
                new_align,
                kind.into(),
            )?;
            Ok(Scalar::Ptr(new_ptr))
        }
    }
}

fn is_null<'mir, 'tcx, M: Machine<'mir, 'tcx>>(
    ecx: &InterpCx<'mir, 'tcx, M>,
    val: Scalar<M::PointerTag>,
) -> InterpResult<'tcx, bool> {
    let null = Scalar::from_int(0, ecx.memory().pointer_size());
    let size = ecx.pointer_size();
    let left = ecx.force_bits(val, size)?;
    let right = ecx.force_bits(null, size)?;
    Ok(left == right)
}

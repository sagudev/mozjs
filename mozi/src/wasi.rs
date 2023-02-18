use std::sync::Mutex;

use mozjs::jsapi::{CallArgs, JSContext, JS_DefineFunction, Value};
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use once_cell::sync::OnceCell;
use wasi_common::snapshots::preview_1::wasi_snapshot_preview1::WasiSnapshotPreview1;
use wasi_common::I32Exit;
use wasi_common::WasiCtx;
use wasmtime_wasi;
use wiggle::{run_in_dummy_executor, GuestPtr};

static WASI_CTX: OnceCell<Mutex<WasiCtx>> = OnceCell::new();

// setter
// TODO: one time there should be real otions passing here
pub fn init_global_wasi_cx() -> bool {
    let b = wasmtime_wasi::WasiCtxBuilder::new()
        .inherit_stderr()
        .inherit_args()
        .unwrap()
        .inherit_env()
        .unwrap()
        .inherit_stdio();
    WASI_CTX.set(Mutex::new(b.build())).is_ok()
}

/// sexy getter
fn wasi_ctx() -> &'static std::sync::Mutex<wasi_common::WasiCtx> {
    WASI_CTX
        .get()
        .expect("WASI_CTX should be initialized by now.")
}

/*
    (import "wasi_snapshot_preview1" "fd_write" (func $_ZN4wasi13lib_generated22wasi_snapshot_preview18fd_write17h10967bea88bd2a6fE (type $t8)))
  (import "wasi_snapshot_preview1" "environ_get" (func $__imported_wasi_snapshot_preview1_environ_get (type $t5)))
  (import "wasi_snapshot_preview1" "environ_sizes_get" (func $__imported_wasi_snapshot_preview1_environ_sizes_get (type $t5)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $__imported_wasi_snapshot_preview1_proc_exit (type $t1)))

(type $t1 (func (param i32)))
(type $t5 (func (param i32 i32) (result i32)))
(type $t8 (func (param i32 i32 i32 i32) (result i32)))
  */

unsafe extern "C" fn proc_exit(_cx: *mut JSContext, argc: u32, vp: *mut Value) -> bool {
    // parse inputs
    let args = CallArgs::from_vp(vp, argc);

    // arg0 is exitcode
    let arg0 = mozjs::rust::Handle::from_raw(args.get(0));
    debug_assert!(arg0.is_int32());
    let arg0 = arg0.to_int32() as u32;

    // call wasi_common function
    let wasi_ctx = &mut *wasi_ctx().lock().unwrap();
    let res = run_in_dummy_executor(wasi_ctx.proc_exit(arg0)).unwrap();

    // this command is only excpetion where we need aditional work over the impl

    // from wasmtime:
    // If a specific WASI error code was requested then that's
    // forwarded through to the process here without printing any
    // extra error information.
    if let Some(exit) = res.downcast_ref::<I32Exit>() {
        // Print the error message in the usual way.
        // On Windows, exit status 3 indicates an abort (see below),
        // so return 1 indicating a non-zero status to avoid ambiguity.
        if cfg!(windows) && exit.0 >= 3 {
            std::process::exit(1);
        }
        std::process::exit(exit.0);
    } else {
        panic!("Drowning failed!")
    }

    // set return type
    //args.rval().set(UndefinedValue());
    //true
}

unsafe extern "C" fn environ_get(cx: *mut JSContext, argc: u32, vp: *mut Value) -> bool {
    // parse inputs
    let args = CallArgs::from_vp(vp, argc);

    //
    let arg0 = mozjs::rust::Handle::from_raw(args.get(0));
    debug_assert!(arg0.is_int32());
    let arg0 = arg0.to_int32();

    //
    let arg1 = mozjs::rust::Handle::from_raw(args.get(1));
    debug_assert!(arg1.is_int32());
    let arg1 = arg1.to_int32();

    // call wasi
    let wasi_ctx = &mut *wasi_ctx().lock().unwrap();
    // here we need acces to wasm module instance to get acces to mem
    // maybe if I create them as a class specs there would be no problemm

    todo!("environ_get");
    /*let res = run_in_dummy_executor(wasi_ctx.environ_get(
        &GuestPtr::new(wasi_ctx.env, GuestPtr::new(wasi_ctx.env, 0)),
        &GuestPtr::<u8>::new(memory, arg0 as u32),
    ))
    .unwrap();*/

    // set return type
    args.rval().set(UndefinedValue());
    true
}

unsafe extern "C" fn environ_sizes_get(cx: *mut JSContext, argc: u32, vp: *mut Value) -> bool {
    // parse inputs
    let args = CallArgs::from_vp(vp, argc);

    //
    let arg0 = mozjs::rust::Handle::from_raw(args.get(0));
    debug_assert!(arg0.is_int32());
    let arg0 = arg0.to_int32();

    //
    let arg1 = mozjs::rust::Handle::from_raw(args.get(1));
    debug_assert!(arg1.is_int32());
    let arg1 = arg1.to_int32();

    //todo!("environ_sizes_get");

    // call wasi_common function
    let wasi_ctx = &mut *wasi_ctx().lock().unwrap();
    let res = run_in_dummy_executor(wasi_ctx.environ_sizes_get()).unwrap();

    rooted!(in(cx) let rval = 5);
    // set return type
    args.rval().set(UndefinedValue());
    true
}

unsafe extern "C" fn fd_write(cx: *mut JSContext, argc: u32, vp: *mut Value) -> bool {
    // parse inputs
    let args = CallArgs::from_vp(vp, argc);
    let arg0 = mozjs::rust::Handle::from_raw(args.get(0));
    debug_assert!(arg0.is_int32());
    let arg0 = arg0.to_int32();
    let arg1 = mozjs::rust::Handle::from_raw(args.get(1));
    debug_assert!(arg1.is_int32());
    let arg1 = arg1.to_int32();
    // call wasi_common function
    //wasi_common::snapshots::preview_1::wasi_snapshot_preview1::proc_exit(ctx, memory, arg0);
    //let wasi_ctx = C.get().unwrap().lock().unwrap();
    //let res = (&mut wasi_ctx).proc_exit(arg0 as u32);
    todo!("fd_write");
    // set return type
    args.rval().set(UndefinedValue());
    true
}

pub unsafe fn populate_wasi_snap(
    cx: *mut JSContext,
    wasi_snapshot_preview1_obj: &mut mozjs::rust::RootedGuard<'_, *mut mozjs::jsapi::JSObject>,
) {
    // populate wasi_snapshot_preview1 object
    assert!(!JS_DefineFunction(
        cx,
        wasi_snapshot_preview1_obj.handle().into(),
        b"fd_write\0".as_ptr() as *const libc::c_char,
        Some(fd_write),
        4,
        0,
    )
    .is_null());
    assert!(!JS_DefineFunction(
        cx,
        wasi_snapshot_preview1_obj.handle().into(),
        b"environ_get\0".as_ptr() as *const libc::c_char,
        Some(environ_get),
        2,
        0,
    )
    .is_null());
    assert!(!JS_DefineFunction(
        cx,
        wasi_snapshot_preview1_obj.handle().into(),
        b"environ_sizes_get\0".as_ptr() as *const libc::c_char,
        Some(environ_sizes_get),
        2,
        0,
    )
    .is_null());
    assert!(!JS_DefineFunction(
        cx,
        wasi_snapshot_preview1_obj.handle().into(),
        b"proc_exit\0".as_ptr() as *const libc::c_char,
        Some(proc_exit),
        1,
        0,
    )
    .is_null());
}

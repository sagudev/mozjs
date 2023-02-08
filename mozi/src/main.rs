//! This example illustrates usage of WebAssembly JS API
use core::ptr;
use core::ptr::null_mut;

use mozjs::jsapi::*;
use mozjs::jsval::ObjectValue;
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::jsapi_wrapped::{Construct1, JS_GetProperty, JS_SetProperty};
use mozjs::rust::SIMPLE_GLOBAL_CLASS;
use mozjs::rust::{JSEngine, RealmOptions, Runtime};
use once_cell::sync::OnceCell;
use wasi_common::WasiCtx;

use crate::error::handle_exception;
use crate::wasi::populate_wasi_snap;

mod error;
mod wasi;

/// ```rust
/// fn main() {
///     println!("Hello, world!");
/// }
///```
const HELLO_WASI: [u8; 2132663] = *include_bytes!("../hello.wasm");

use ::std::sync::Mutex;

static C: OnceCell<Mutex<WasiCtx>> = OnceCell::new();

fn main() {
    let b = wasmtime_wasi::WasiCtxBuilder::new()
        .inherit_stderr()
        .inherit_args()
        .unwrap()
        .inherit_env()
        .unwrap()
        .inherit_stdio();
    assert!(C.set(Mutex::new(b.build())).is_ok());
    // init engine
    let engine = JSEngine::init().expect("failed to initalize JS engine");
    let rt = Runtime::new(engine.handle());
    assert!(!rt.cx().is_null(), "failed to create JSContext");
    let options = RealmOptions::default();
    rooted!(in(rt.cx()) let global = unsafe {
        JS_NewGlobalObject(rt.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                           OnNewGlobalHookOption::FireOnNewGlobalHook,
                           &*options)
    });
    let _ac = JSAutoRealm::new(rt.cx(), global.get());

    // Get WebAssembly.Module and WebAssembly.Instance constructors.
    rooted!(in(rt.cx()) let mut wasm = UndefinedValue());
    rooted!(in(rt.cx()) let mut wasm_module = UndefinedValue());
    rooted!(in(rt.cx()) let mut wasm_instance = UndefinedValue());

    unsafe {
        assert!(JS_GetProperty(
            rt.cx(),
            global.handle(),
            b"WebAssembly\0".as_ptr() as *const libc::c_char,
            &mut wasm.handle_mut()
        ));
        rooted!(in(rt.cx()) let mut wasm_obj = wasm.to_object());
        assert!(JS_GetProperty(
            rt.cx(),
            wasm_obj.handle(),
            b"Module\0".as_ptr() as *const libc::c_char,
            &mut wasm_module.handle_mut()
        ));
        assert!(JS_GetProperty(
            rt.cx(),
            wasm_obj.handle(),
            b"Instance\0".as_ptr() as *const libc::c_char,
            &mut wasm_instance.handle_mut()
        ));
    }
    // Construct Wasm module from bytes.
    rooted!(in(rt.cx()) let mut module = null_mut::<JSObject>());
    unsafe {
        let array_buffer = JS::NewArrayBufferWithUserOwnedContents(
            rt.cx(),
            HELLO_WASI.len(),
            HELLO_WASI.as_ptr() as _,
        );
        assert!(!array_buffer.is_null());

        let args = HandleValueArray::from_rooted_slice(&[ObjectValue(array_buffer)]);

        assert!(Construct1(
            rt.cx(),
            wasm_module.handle(),
            &args,
            &mut module.handle_mut(),
        ));
    }

    // Construct Wasm module instance with WASI.
    rooted!(in(rt.cx()) let mut instance = null_mut::<JSObject>());
    unsafe {
        // build wasi_snapshot_preview1 object
        rooted!(in(rt.cx()) let mut wasi_snapshot_preview1_obj = JS_NewPlainObject(rt.cx()));
        assert!(!wasi_snapshot_preview1_obj.is_null());
        // populate it
        populate_wasi_snap(rt.cx(), &mut wasi_snapshot_preview1_obj);

        rooted!(in(rt.cx()) let mut wasi_snapshot_preview1 = ObjectValue(wasi_snapshot_preview1_obj.get()));
        // Build imports bag.
        rooted!(in(rt.cx()) let mut imports = JS_NewPlainObject(rt.cx()));
        assert!(!imports.is_null());
        assert!(JS_SetProperty(
            rt.cx(),
            imports.handle(),
            b"wasi_snapshot_preview1\0".as_ptr() as *const libc::c_char,
            wasi_snapshot_preview1.handle()
        ));

        let args = HandleValueArray::from_rooted_slice(&[
            ObjectValue(module.get()),
            ObjectValue(imports.get()),
        ]);

        assert!(Construct1(
            rt.cx(),
            wasm_instance.handle(),
            &args,
            &mut instance.handle_mut(),
        ));
        assert!(!handle_exception(rt.cx()));
    }

    // get exports
    rooted!(in(rt.cx()) let mut exports = UndefinedValue());
    assert!(unsafe {
        JS_GetProperty(
            rt.cx(),
            instance.handle(),
            b"exports\0".as_ptr() as *const libc::c_char,
            &mut exports.handle_mut(),
        )
    });
    rooted!(in(rt.cx()) let mut exports_obj = exports.to_object());

    // get main (entrypoint is named _start in WASI)
    rooted!(in(rt.cx()) let mut main = UndefinedValue());
    assert!(unsafe {
        JS_GetProperty(
            rt.cx(),
            exports_obj.handle(),
            b"_start\0".as_ptr() as *const libc::c_char,
            &mut main.handle_mut(),
        )
    });

    // call main (result is not used as it is a unit `()`)
    rooted!(in(rt.cx()) let mut rval = UndefinedValue());
    assert!(unsafe {
        Call(
            rt.cx(),
            JS::UndefinedHandleValue,
            main.handle().into(),
            &HandleValueArray::new(),
            rval.handle_mut().into(),
        )
    });
}

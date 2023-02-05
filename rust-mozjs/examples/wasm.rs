//! This example illustrates usage of WebAssembly JS API
//! as showcased in [spidermonkey-embedding-examples/examples/wasm.cpp](https://github.com/mozilla-spidermonkey/spidermonkey-embedding-examples/blob/esr102/examples/wasm.cpp)
//! It does no error handling and simply exits if something goes wrong.
//!
//! To use the WebAssembly JIT you need to create a context and a global object,
//! and do some setup on both of these. You also need to enter a "realm"
//! (environment within one global object) before you can execute code.

use core::ptr;
use core::ptr::null_mut;

use mozjs::jsapi::*;
use mozjs::jsval::ObjectValue;
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::jsapi_wrapped::{Construct1, JS_GetProperty, JS_SetProperty};
use mozjs::rust::SIMPLE_GLOBAL_CLASS;
use mozjs::rust::{JSEngine, RealmOptions, Runtime};

/// hi.wat:
/// ```
/// (module
///  (import "env" "bar" (func $bar (param i32) (result i32)))
///  (func (export "foo") (result i32)
///    i32.const 42
///    call $bar
///  ))
///```
const HI_WASM: [libc::c_uchar; 56] = [
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0a, 0x02, 0x60, 0x01, 0x7f, 0x01, 0x7f,
    0x60, 0x00, 0x01, 0x7f, 0x02, 0x0b, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x03, 0x62, 0x61, 0x72, 0x00,
    0x00, 0x03, 0x02, 0x01, 0x01, 0x07, 0x07, 0x01, 0x03, 0x66, 0x6f, 0x6f, 0x00, 0x01, 0x0a, 0x08,
    0x01, 0x06, 0x00, 0x41, 0x2a, 0x10, 0x00, 0x0b,
];

unsafe extern "C" fn bar(_cx: *mut JSContext, argc: u32, vp: *mut Value) -> bool {
    let args = CallArgs::from_vp(vp, argc);
    args.rval().set(args.get(0).get());
    true
}

fn run(rt: Runtime) {
    let options = RealmOptions::default();
    rooted!(in(rt.cx()) let global = unsafe {
        JS_NewGlobalObject(rt.cx(), &SIMPLE_GLOBAL_CLASS, ptr::null_mut(),
                           OnNewGlobalHookOption::FireOnNewGlobalHook,
                           &*options)
    });

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

        // Construct Wasm module from bytes.
        rooted!(in(rt.cx()) let mut module = null_mut::<JSObject>());
        {
            let array_buffer = JS::NewArrayBufferWithUserOwnedContents(
                rt.cx(),
                HI_WASM.len(),
                HI_WASM.as_ptr() as _,
            );
            assert!(!array_buffer.is_null());

            let args = HandleValueArray::from_rooted_slice(&[ObjectValue(array_buffer)]);

            assert!(Construct1(
                rt.cx(),
                wasm_module.handle(),
                &args,
                &mut module.handle_mut()
            ))
        }

        // Construct Wasm module instance with required imports.
        rooted!(in(rt.cx()) let mut instance = null_mut::<JSObject>());
        {
            // Build "env" imports object.
            rooted!(in(rt.cx()) let mut env_import_obj = JS_NewPlainObject(rt.cx()));
            assert!(!env_import_obj.is_null());
            let function = JS_DefineFunction(
                rt.cx(),
                env_import_obj.handle().into(),
                b"bar\0".as_ptr() as *const libc::c_char,
                Some(bar),
                1,
                0,
            );
            assert!(!function.is_null());
            rooted!(in(rt.cx()) let mut env_import = ObjectValue(env_import_obj.get()));
            // Build imports bag.
            rooted!(in(rt.cx()) let mut imports = JS_NewPlainObject(rt.cx()));
            assert!(!imports.is_null());
            assert!(JS_SetProperty(
                rt.cx(),
                imports.handle(),
                b"env\0".as_ptr() as *const libc::c_char,
                env_import.handle()
            ));

            let args = HandleValueArray::from_rooted_slice(&[
                ObjectValue(module.get()),
                ObjectValue(imports.get()),
            ]);

            assert!(Construct1(
                rt.cx(),
                wasm_instance.handle(),
                &args,
                &mut instance.handle_mut()
            ));
        }

        // Find `foo` method in exports.
        rooted!(in(rt.cx()) let mut exports = UndefinedValue());

        assert!(JS_GetProperty(
            rt.cx(),
            instance.handle(),
            b"exports\0".as_ptr() as *const libc::c_char,
            &mut exports.handle_mut()
        ));

        rooted!(in(rt.cx()) let mut exports_obj = exports.to_object());
        rooted!(in(rt.cx()) let mut foo = UndefinedValue());
        assert!(JS_GetProperty(
            rt.cx(),
            exports_obj.handle(),
            b"foo\0".as_ptr() as *const libc::c_char,
            &mut foo.handle_mut()
        ));

        // call foo ang get its result
        rooted!(in(rt.cx()) let mut rval = UndefinedValue());
        assert!(Call(
            rt.cx(),
            JS::UndefinedHandleValue,
            foo.handle().into(),
            &HandleValueArray::new(),
            rval.handle_mut().into()
        ));

        // check if results are correct
        assert!(rval.get().is_int32());
        assert_eq!(rval.get().to_int32(), 42);
    }
}

fn main() {
    let engine = JSEngine::init().expect("failed to initalize JS engine");
    let runtime = Runtime::new(engine.handle());
    assert!(!runtime.cx().is_null(), "failed to create JSContext");
    run(runtime);
}

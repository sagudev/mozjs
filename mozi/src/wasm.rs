use std::cell::UnsafeCell;

use mozjs::jsapi::{JSContext, JSObject};
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::jsapi_wrapped::JS_GetProperty;
use mozjs::rust::RootedGuard;
use once_cell::sync::OnceCell;
use wiggle::borrow::BorrowChecker;
use wiggle::GuestMemory;

static WASM_CTX: OnceCell<WasmCtx<'static>> = OnceCell::new();

/// sexy getter
pub fn wasm_ctx() -> &'static WasmCtx<'static> {
    WASM_CTX
        .get()
        .expect("WASM_CTX should be initialized by now.")
}

pub struct WasmCtx<'a> {
    /// SAFETY: we have the borrow checker bellow
    mem: &'a [UnsafeCell<u8>],
    /// Borrow checker for memory
    bc: BorrowChecker,
}

// These need to be reapplied due to the usage of `UnsafeCell` internally.
unsafe impl Send for WasmCtx<'_> {}
unsafe impl Sync for WasmCtx<'_> {}

unsafe impl GuestMemory for WasmCtx<'_> {
    fn base(&self) -> &[std::cell::UnsafeCell<u8>] {
        self.mem
    }

    fn has_outstanding_borrows(&self) -> bool {
        self.bc.has_outstanding_borrows()
    }

    fn is_mut_borrowed(&self, r: wiggle::Region) -> bool {
        self.bc.is_mut_borrowed(r)
    }

    fn is_shared_borrowed(&self, r: wiggle::Region) -> bool {
        self.bc.is_shared_borrowed(r)
    }

    fn mut_borrow(
        &self,
        r: wiggle::Region,
    ) -> wiggle::anyhow::Result<wiggle::BorrowHandle, wiggle::GuestError> {
        self.bc.mut_borrow(r)
    }

    fn shared_borrow(
        &self,
        r: wiggle::Region,
    ) -> wiggle::anyhow::Result<wiggle::BorrowHandle, wiggle::GuestError> {
        self.bc.shared_borrow(r)
    }

    fn mut_unborrow(&self, h: wiggle::BorrowHandle) {
        self.bc.mut_unborrow(h)
    }

    fn shared_unborrow(&self, h: wiggle::BorrowHandle) {
        self.bc.shared_unborrow(h)
    }
}

pub fn init_global_wasm_ctx(cx: *mut JSContext, exports_obj: &RootedGuard<*mut JSObject>) {
    rooted!(in(cx) let mut memory = UndefinedValue());
    assert!(
        unsafe {
            JS_GetProperty(
                cx,
                exports_obj.handle(),
                b"memory\0".as_ptr() as *const libc::c_char,
                &mut memory.handle_mut(),
            )
        },
        "Cannot find memory"
    );
    rooted!(in(cx) let mut memory_obj = memory.to_object());
    // access memory buffer
    rooted!(in(cx) let mut memory_buffer = UndefinedValue());
    assert!(unsafe {
        JS_GetProperty(
            cx,
            memory_obj.handle(),
            b"buffer\0".as_ptr() as *const libc::c_char,
            &mut memory_buffer.handle_mut(),
        )
    });
    rooted!(in(cx) let mut memory_buffer_obj = memory_buffer.to_object());
    debug_assert!(unsafe { mozjs::jsapi::IsArrayBufferObject(memory_buffer_obj.get()) });
    //let size = unsafe { mozjs::jsapi::JS::GetArrayBufferByteLength(memory_buffer_obj.get()) };

    let mut len: usize = 0;
    unsafe {
        // https://stackoverflow.com/questions/58530104/what-is-the-proper-way-to-use-a-pointer-to-pointer-in-a-foreign-function-in-rust
        let mut mem: *mut u8 = std::ptr::null_mut();
        let mem_ptr: *mut *mut u8 = &mut mem;
        assert!(!mozjs::jsapi::JS::GetObjectAsArrayBuffer(
            memory_buffer_obj.get(),
            &mut len,
            mem_ptr,
        )
        .is_null());

        assert!(
            WASM_CTX
                .set(WasmCtx {
                    mem: std::slice::from_raw_parts((mem as *const u8).cast(), len),
                    bc: BorrowChecker::new(),
                })
                .is_ok(),
            "WASM_CTX is full"
        );
    }

    // TODO: if we would ever need other stuff like table,global,...
    // we should create wasm_ctx struct (&wasi_ctx) and use it like
}

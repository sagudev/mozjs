use crate::jsapi::{Heap, JSObject, JSTracer};
use crate::rust::{Runtime, Stencil};
use mozjs_sys::trace::Traceable;
use std::cell::UnsafeCell;
use std::ffi::c_void;

use crate::typedarray::{TypedArray, TypedArrayElement};

unsafe impl<T: TypedArrayElement> Traceable for TypedArray<T, Box<Heap<*mut JSObject>>> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        self.underlying_object().trace(trc);
    }
}

unsafe impl Traceable for Runtime {
    #[inline]
    unsafe fn trace(&self, _: *mut JSTracer) {}
}

unsafe impl Traceable for Stencil {
    #[inline]
    unsafe fn trace(&self, _: *mut JSTracer) {}
}

/// Holds a list of JSTraceables that need to be rooted
///
/// Generally, rooting happens in stack (LIFO) order, but that is not a requirement, but it does bring performance benefits.
pub struct RootCollection(UnsafeCell<Vec<*const dyn Traceable>>);

impl RootCollection {
    pub fn new() -> Self {
        RootCollection(UnsafeCell::new(Vec::new()))
    }

    pub fn add(&self, traceable: *const dyn Traceable) {
        unsafe { (*self.0.get()).push(traceable) }
    }

    pub fn remove(&self, traceable: *const dyn Traceable) {
        let traceables = unsafe { &mut *self.0.get() };
        let idx = match traceables
            .iter()
            .rposition(|x| *x as *const () == traceable as *const ())
        {
            Some(idx) => idx,
            None => return,
        };
        traceables.remove(idx);
    }

    unsafe fn trace(&self, trc: *mut JSTracer) {
        let traceables = unsafe { &*self.0.get() };
        for traceable in traceables {
            unsafe { (**traceable).trace(trc) }
        }
    }
}

thread_local!(
    pub(crate) static ROOTED_TRACEABLES: RootCollection =
        RootCollection(UnsafeCell::new(Vec::new()));
);

pub unsafe extern "C" fn trace_traceables(trc: *mut JSTracer, _: *mut c_void) {
    ROOTED_TRACEABLES.with(|traceables| {
        traceables.trace(trc);
    });
}

/// Holds a list of JSTraceables that need to be rooted
pub struct RootedTraceableSet;

impl RootedTraceableSet {
    pub unsafe fn add(traceable: *const dyn Traceable) {
        ROOTED_TRACEABLES.with(|traceables| {
            traceables.add(traceable);
        });
    }

    pub unsafe fn remove(traceable: *const dyn Traceable) {
        ROOTED_TRACEABLES.with(|traceables| {
            traceables.remove(traceable);
        });
    }
}

/// `StableTraceObject` represents values that can be rooted through a stable address that will
/// not change for their whole lifetime.
/// It is an unsafe trait that requires implementors to ensure certain safety guarantees.
///
/// # Safety
///
/// Implementors of this trait must ensure that the `trace` method correctly accounts for all
/// owned and referenced objects, so that the garbage collector can accurately determine which
/// objects are still in use. Failing to adhere to this contract may result in undefined behavior,
/// such as use-after-free errors.
pub unsafe trait StableTraceObject {
    /// Returns a stable trace object which address won't change for the whole
    /// lifetime of the value.
    fn stable_trace_object(&self) -> *const dyn Traceable;
}

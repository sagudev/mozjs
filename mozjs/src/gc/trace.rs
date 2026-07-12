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

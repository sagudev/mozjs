/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Utilities for tracing JS-managed values.
//!
//! The lifetime of DOM objects is managed by the SpiderMonkey Garbage
//! Collector. A rooted DOM object implementing the interface `Foo` is traced
//! as follows:
//!
//! 1. The GC calls `_trace` defined in `FooBinding` during the marking
//!    phase. (This happens through `JSClass.trace` for non-proxy bindings, and
//!    through `ProxyTraps.trace` otherwise.)
//! 2. `_trace` calls `Foo::trace()` (an implementation of `JSTraceable`).
//!    This is typically derived via a `#[dom_struct]`
//!    (implies `#[derive(JSTraceable)]`) annotation.
//!    Non-JS-managed types have an empty inline `trace()` method,
//!    achieved via `unsafe_no_jsmanaged_fields!` or similar.
//! 3. For all fields, `Foo::trace()`
//!    calls `trace()` on the field.
//!    For example, for fields of type `Dom<T>`, `Dom<T>::trace()` calls
//!    `trace_reflector()`.
//! 4. `trace_reflector()` calls `Dom::TraceEdge()` with a
//!    pointer to the `JSObject` for the reflector. This notifies the GC, which
//!    will add the object to the graph, and will trace that object as well.
//! 5. When the GC finishes tracing, it [`finalizes`](../index.html#destruction)
//!    any reflectors that were not reachable.
//!
//! The `unsafe_no_jsmanaged_fields!()` macro adds an empty implementation of
//! `JSTraceable` to a datatype.

use crate::glue::{CallObjectTracer, CallScriptTracer, CallStringTracer, CallValueTracer};
use crate::jsapi::{
    GCTraceKindToAscii, Heap, JSObject, JSScript, JSString, JSTracer, JobQueue, TraceKind,
};
use crate::jsval::JSVal;
use crate::rust::{GCMethods, Handle, Runtime, Stencil};
use crate::typedarray::TypedArray;
use crate::typedarray::TypedArrayElement;
use std::borrow::Cow;
use std::cell::{Cell, RefCell, UnsafeCell};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::{BuildHasher, Hash};
use std::num::NonZeroU64;
use std::ops::{Deref, DerefMut, Range};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime};

pub use jstraceable_derive::JSTraceable;

/// A trait to allow tracing (only) DOM objects.
pub unsafe trait JSTraceable {
    /// Trace `self`.
    unsafe fn trace(&self, trc: *mut JSTracer);
}

/// Holds a set of JSTraceables that need to be rooted
struct RootedTraceableSet {
    set: Vec<*const dyn JSTraceable>,
}

thread_local!(
    /// TLV Holds a set of JSTraceables that need to be rooted
    static ROOTED_TRACEABLES: RefCell<RootedTraceableSet> = RefCell::new(RootedTraceableSet::new());
);

impl RootedTraceableSet {
    fn new() -> RootedTraceableSet {
        RootedTraceableSet { set: vec![] }
    }

    unsafe fn remove(traceable: *const dyn JSTraceable) {
        ROOTED_TRACEABLES.with(|ref traceables| {
            let mut traceables = traceables.borrow_mut();
            let idx = match traceables
                .set
                .iter()
                .rposition(|x| *x as *const () == traceable as *const ())
            {
                Some(idx) => idx,
                None => unreachable!(),
            };
            traceables.set.remove(idx);
        });
    }

    unsafe fn add(traceable: *const dyn JSTraceable) {
        ROOTED_TRACEABLES.with(|ref traceables| {
            traceables.borrow_mut().set.push(traceable);
        })
    }

    unsafe fn trace(&self, tracer: *mut JSTracer) {
        for traceable in &self.set {
            (**traceable).trace(tracer);
        }
    }
}

/// Roots any JSTraceable thing
///
/// If you have a valid DomObject, use DomRoot.
/// If you have GC things like *mut JSObject or JSVal, use rooted!.
/// If you have an arbitrary number of DomObjects to root, use rooted_vec!.
/// If you know what you're doing, use this.
pub struct RootedTraceableBox<T: 'static + JSTraceable> {
    ptr: *mut T,
}

unsafe impl<T: JSTraceable + 'static> JSTraceable for RootedTraceableBox<T> {
    unsafe fn trace(&self, tracer: *mut JSTracer) {
        (*self.ptr).trace(tracer);
    }
}

impl<T: JSTraceable + 'static> RootedTraceableBox<T> {
    /// DomRoot a JSTraceable thing for the life of this RootedTraceableBox
    pub fn new(traceable: T) -> RootedTraceableBox<T> {
        Self::from_box(Box::new(traceable))
    }

    /// Consumes a boxed JSTraceable and roots it for the life of this RootedTraceableBox.
    pub fn from_box(boxed_traceable: Box<T>) -> RootedTraceableBox<T> {
        let traceable = Box::into_raw(boxed_traceable);
        unsafe {
            RootedTraceableSet::add(traceable);
        }
        RootedTraceableBox { ptr: traceable }
    }
}

impl<T> RootedTraceableBox<Heap<T>>
where
    Heap<T>: JSTraceable + 'static,
    T: GCMethods + Copy,
{
    pub fn handle(&self) -> Handle<T> {
        unsafe { Handle::from_raw((*self.ptr).handle()) }
    }
}

impl<T: JSTraceable + Default> Default for RootedTraceableBox<T> {
    fn default() -> RootedTraceableBox<T> {
        RootedTraceableBox::new(T::default())
    }
}

impl<T: JSTraceable> Deref for RootedTraceableBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<T: JSTraceable> DerefMut for RootedTraceableBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<T: JSTraceable + 'static> Drop for RootedTraceableBox<T> {
    fn drop(&mut self) {
        unsafe {
            RootedTraceableSet::remove(self.ptr);
            let _ = Box::from_raw(self.ptr);
        }
    }
}

/// A vector of items to be rooted with `RootedVec`.
/// Guaranteed to be empty when not rooted.
/// Usage: `rooted_vec!(let mut v);` or if you have an
/// iterator of `DomRoot`s, `rooted_vec!(let v <- iterator);`.
#[derive(JSTraceable)]
pub struct RootableVec<T: JSTraceable> {
    v: Vec<T>,
}

impl<T: JSTraceable> RootableVec<T> {
    /// Create a vector of items of type T that can be rooted later.
    pub fn new_unrooted() -> RootableVec<T> {
        RootableVec { v: vec![] }
    }
}

/// A vector of items that are rooted for the lifetime 'a.
pub struct RootedVec<'a, T: 'static + JSTraceable> {
    root: &'a mut RootableVec<T>,
}

impl<'a, T: 'static + JSTraceable> RootedVec<'a, T> {
    /// Create a vector of items of type T that is rooted for
    /// the lifetime of this struct
    pub fn new(root: &'a mut RootableVec<T>) -> Self {
        unsafe {
            RootedTraceableSet::add(root);
        }
        RootedVec { root: root }
    }
}

impl<'a, T: JSTraceable + 'static> Drop for RootedVec<'a, T> {
    fn drop(&mut self) {
        self.clear();
        unsafe {
            RootedTraceableSet::remove(self.root);
        }
    }
}

impl<'a, T: JSTraceable> Deref for RootedVec<'a, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.root.v
    }
}

impl<'a, T: JSTraceable> DerefMut for RootedVec<'a, T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.root.v
    }
}

/// SM Callback that traces the rooted traceables
pub unsafe fn trace_traceables(tracer: *mut JSTracer) {
    trace!("tracing stack-rooted traceables");
    ROOTED_TRACEABLES.with(|ref traceables| {
        let traceables = traceables.borrow();
        traceables.trace(tracer);
    });
}

/// For use on non-jsmanaged types
/// Use #[derive(JSTraceable)] on JS managed types
#[macro_export]
macro_rules! unsafe_no_jsmanaged_fields(
    ($($ty:ty),+) => (
        $(
            #[allow(unsafe_code)]
            unsafe impl $crate::trace::JSTraceable for $ty {
                #[inline]
                unsafe fn trace(&self, _: *mut $crate::jsapi::JSTracer) {
                    // Do nothing
                }
            }
        )+
    );
);

#[macro_export]
macro_rules! jsmanaged_array(
    ($count:expr) => (
        #[allow(unsafe_code)]
        unsafe impl<T> $crate::trace::JSTraceable for [T; $count]
            where T: $crate::trace::JSTraceable
        {
            #[inline]
            unsafe fn trace(&self, tracer: *mut ::js::jsapi::JSTracer) {
                for v in self.iter() {
                    v.trace(tracer);
                }
            }
        }
    );
);

#[macro_export]
macro_rules! rooted_vec {
    (let mut $name:ident) => {
        let mut root = $crate::dom::bindings::trace::RootableVec::new_unrooted();
        let mut $name = $crate::dom::bindings::trace::RootedVec::new(&mut root);
    };
    (let $name:ident <- $iter:expr) => {
        let mut root = $crate::dom::bindings::trace::RootableVec::new_unrooted();
        let $name = $crate::dom::bindings::trace::RootedVec::from_iter(&mut root, $iter);
    };
    (let mut $name:ident <- $iter:expr) => {
        let mut root = $crate::dom::bindings::trace::RootableVec::new_unrooted();
        let mut $name = $crate::dom::bindings::trace::RootedVec::from_iter(&mut root, $iter);
    };
}

unsafe_no_jsmanaged_fields!(*mut JobQueue);
unsafe_no_jsmanaged_fields!(JoinHandle<()>);
unsafe_no_jsmanaged_fields!(Cow<'static, str>);

/// Trace a `JSScript`.
pub fn trace_script(tracer: *mut JSTracer, description: &str, script: &Heap<*mut JSScript>) {
    unsafe {
        trace!("tracing {}", description);
        CallScriptTracer(
            tracer,
            script.ptr.get() as *mut _,
            GCTraceKindToAscii(TraceKind::Script),
        );
    }
}

/// Trace a `JSVal`.
pub fn trace_jsval(tracer: *mut JSTracer, description: &str, val: &Heap<JSVal>) {
    unsafe {
        if !val.get().is_markable() {
            return;
        }

        trace!("tracing value {}", description);
        CallValueTracer(
            tracer,
            val.ptr.get() as *mut _,
            GCTraceKindToAscii(val.get().trace_kind()),
        );
    }
}

/// Trace a `JSObject`.
pub fn trace_object(tracer: *mut JSTracer, description: &str, obj: &Heap<*mut JSObject>) {
    unsafe {
        trace!("tracing {}", description);
        CallObjectTracer(
            tracer,
            obj.ptr.get() as *mut _,
            GCTraceKindToAscii(TraceKind::Object),
        );
    }
}

/// Trace a `JSString`.
pub fn trace_string(tracer: *mut JSTracer, description: &str, s: &Heap<*mut JSString>) {
    unsafe {
        trace!("tracing {}", description);
        CallStringTracer(
            tracer,
            s.ptr.get() as *mut _,
            GCTraceKindToAscii(TraceKind::String),
        );
    }
}

unsafe impl<T: JSTraceable> JSTraceable for Rc<T> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        (**self).trace(trc)
    }
}

unsafe impl<T: JSTraceable> JSTraceable for Arc<T> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        (**self).trace(trc)
    }
}

unsafe impl<T: JSTraceable + ?Sized> JSTraceable for Box<T> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        (**self).trace(trc)
    }
}

unsafe impl<T: JSTraceable> JSTraceable for [T] {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        for e in self.iter() {
            e.trace(trc);
        }
    }
}

unsafe impl<T: JSTraceable + Copy> JSTraceable for Cell<T> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        self.get().trace(trc)
    }
}

unsafe impl<T: JSTraceable> JSTraceable for UnsafeCell<T> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        (*self.get()).trace(trc)
    }
}

unsafe impl<T: JSTraceable> JSTraceable for RefCell<T> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        (*self).borrow().trace(trc)
    }
}

unsafe impl JSTraceable for Heap<*mut JSScript> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        if self.get().is_null() {
            return;
        }
        trace_script(trc, "heap script", self);
    }
}

unsafe impl JSTraceable for Heap<*mut JSObject> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        if self.get().is_null() {
            return;
        }
        trace_object(trc, "heap object", self);
    }
}

unsafe impl JSTraceable for Heap<*mut JSString> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        if self.get().is_null() {
            return;
        }
        trace_string(trc, "heap string", self);
    }
}

unsafe impl JSTraceable for Heap<JSVal> {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        trace_jsval(trc, "heap value", self);
    }
}

// XXXManishearth Check if the following three are optimized to no-ops
// if e.trace() is a no-op (e.g it is an unsafe_no_jsmanaged_fields type)
unsafe impl<T: JSTraceable> JSTraceable for Vec<T> {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        for e in &*self {
            e.trace(trc);
        }
    }
}

unsafe impl<T: JSTraceable> JSTraceable for VecDeque<T> {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        for e in &*self {
            e.trace(trc);
        }
    }
}

unsafe impl<A, B, C, D> JSTraceable for (A, B, C, D)
where
    A: JSTraceable,
    B: JSTraceable,
    C: JSTraceable,
    D: JSTraceable,
{
    unsafe fn trace(&self, trc: *mut JSTracer) {
        self.0.trace(trc);
        self.1.trace(trc);
        self.2.trace(trc);
        self.3.trace(trc);
    }
}

unsafe impl<T: JSTraceable> JSTraceable for Option<T> {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        self.as_ref().map(|e| e.trace(trc));
    }
}

unsafe impl<T: JSTraceable, U: JSTraceable> JSTraceable for Result<T, U> {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        match *self {
            Ok(ref inner) => inner.trace(trc),
            Err(ref inner) => inner.trace(trc),
        }
    }
}

unsafe impl<K, V, S> JSTraceable for HashMap<K, V, S>
where
    K: Hash + Eq + JSTraceable,
    V: JSTraceable,
    S: BuildHasher,
{
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        for (k, v) in &*self {
            k.trace(trc);
            v.trace(trc);
        }
    }
}

unsafe impl<T, S> JSTraceable for HashSet<T, S>
where
    T: Hash + Eq + JSTraceable,
    S: BuildHasher,
{
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        for v in &*self {
            v.trace(trc);
        }
    }
}

unsafe impl<K: Ord + JSTraceable, V: JSTraceable> JSTraceable for BTreeMap<K, V> {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        for (k, v) in self {
            k.trace(trc);
            v.trace(trc);
        }
    }
}

unsafe impl<A: JSTraceable, B: JSTraceable> JSTraceable for (A, B) {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        let (ref a, ref b) = *self;
        a.trace(trc);
        b.trace(trc);
    }
}

unsafe impl<A: JSTraceable, B: JSTraceable, C: JSTraceable> JSTraceable for (A, B, C) {
    #[inline]
    unsafe fn trace(&self, trc: *mut JSTracer) {
        let (ref a, ref b, ref c) = *self;
        a.trace(trc);
        b.trace(trc);
        c.trace(trc);
    }
}

unsafe_no_jsmanaged_fields!(bool, f32, f64, String, AtomicBool, AtomicUsize, char);
unsafe_no_jsmanaged_fields!(usize, u8, u16, u32, u64);
unsafe_no_jsmanaged_fields!(isize, i8, i16, i32, i64);
unsafe_no_jsmanaged_fields!(NonZeroU64);
// These three are interdependent, if you plan to put jsmanaged data
// in one of these make sure it is propagated properly to containing structs

unsafe_no_jsmanaged_fields!(Runtime);
unsafe_no_jsmanaged_fields!(SystemTime);
unsafe_no_jsmanaged_fields!(Instant);
unsafe_no_jsmanaged_fields!(PathBuf);
unsafe_no_jsmanaged_fields!(Range<u64>);
unsafe_no_jsmanaged_fields!(Stencil);

unsafe impl<'a> JSTraceable for &'a str {
    #[inline]
    unsafe fn trace(&self, _: *mut JSTracer) {
        // Do nothing
    }
}

unsafe impl<A, B> JSTraceable for fn(A) -> B {
    #[inline]
    unsafe fn trace(&self, _: *mut JSTracer) {
        // Do nothing
    }
}

unsafe impl JSTraceable for () {
    #[inline]
    unsafe fn trace(&self, _: *mut JSTracer) {
        // Do nothing
    }
}

unsafe impl<T> JSTraceable for TypedArray<T, Box<Heap<*mut JSObject>>>
where
    T: TypedArrayElement,
{
    unsafe fn trace(&self, trc: *mut JSTracer) {
        self.underlying_object().trace(trc);
    }
}

#![allow(clippy::cast_ptr_alignment)] // clippy is too strict here

use super::arc_wake::{ArcWake, clone_arc_raw, wake_arc_raw};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::task::{Waker, RawWaker, RawWakerVTable};

/// A [`Waker`](::std::task::Waker) that is only valid for a given lifetime.
///
/// Note: this type implements [`Deref<Target = Waker>`](::std::ops::Deref),
/// so it can be used to get a `&Waker`.
#[derive(Debug)]
pub struct WakerRef<'a> {
    waker: Waker,
    _marker: PhantomData<&'a ()>,
}

impl<'a> WakerRef<'a> {
    /// Create a new [`WakerRef`] from a [`Waker`].
    ///
    /// Note: this function is safe, but it is generally only used
    /// from `unsafe` contexts that need to create a `Waker`
    /// that is guaranteed not to outlive a particular lifetime.
    pub fn new(waker: Waker) -> Self {
        WakerRef {
            waker,
            _marker: PhantomData,
        }
    }
}

impl<'a> Deref for WakerRef<'a> {
    type Target = Waker;

    fn deref(&self) -> &Waker {
        &self.waker
    }
}

#[inline]
unsafe fn noop(_data: *const ()) {}

/// Creates a reference to a [`Waker`](::std::task::Waker)
/// from a local [`wake`](::std::task::Wake).
///
/// The resulting [`Waker`](::std::task::Waker) will call
/// [`wake.wake()`](::std::task::Wake::wake) if awoken.
#[inline]
pub fn waker_ref<W>(wake: &Arc<W>) -> WakerRef<'_>
where
    W: ArcWake
{
    // This uses the same mechanism as Arc::into_raw, without needing a reference.
    // This is potentially not stable
    let ptr = &*wake as &W as *const W as *const();

    // Similar to `waker_vtable`, but with a no-op `drop` function.
    // Clones of the resulting `RawWaker` will still be dropped normally.
    let vtable = &RawWakerVTable {
        clone: clone_arc_raw::<W>,
        drop: noop,
        wake: wake_arc_raw::<W>,
    };

    let waker = unsafe {
        Waker::new_unchecked(RawWaker::new(ptr, vtable))
    };
    WakerRef::new(waker)
}

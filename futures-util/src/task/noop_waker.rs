//! Utilities for creating zero-cost wakers that don't do anything.
use futures_core::task::{RawWaker, RawWakerVTable, Waker};
use core::ptr::null;
#[cfg(feature = "std")]
use core::cell::UnsafeCell;

unsafe fn noop_clone(_data: *const()) -> RawWaker {
    noop_raw_waker()
}

unsafe fn noop(_data: *const()) {
}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable {
    clone: noop_clone,
    drop: noop,
    wake: noop,
};

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(null(), &NOOP_WAKER_VTABLE)
}

/// Create a new [`Waker`](futures_core::task::Waker) which does
/// nothing when `wake()` is called on it. The [`Waker`] can be converted
/// into a [`Waker`] which will behave the same way.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures::task::noop_waker;
/// let lw = noop_waker();
/// lw.wake();
/// ```
#[inline]
pub fn noop_waker() -> Waker {
    unsafe {
        Waker::new_unchecked(noop_raw_waker())
    }
}

/// Get a thread local reference to a
/// [`Waker`](futures_core::task::Waker) referencing a singleton
/// instance of a [`Waker`] which panics when woken.
///
/// # Examples
///
/// ```
/// #![feature(futures_api)]
/// use futures::task::noop_waker_ref;
/// let lw = noop_waker_ref();
/// lw.wake();
/// ```
#[inline]
#[cfg(feature = "std")]
pub fn noop_waker_ref() -> &'static Waker {
    thread_local! {
        static NOOP_WAKER_INSTANCE: UnsafeCell<Waker> =
            UnsafeCell::new(noop_waker());
    }
    NOOP_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}


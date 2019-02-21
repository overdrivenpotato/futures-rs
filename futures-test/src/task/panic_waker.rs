use futures_core::task::{Waker, RawWaker, RawWakerVTable};
use core::cell::UnsafeCell;
use core::ptr::null;

unsafe fn clone_panic_waker(_data: *const()) -> RawWaker {
    raw_panic_waker()
}

unsafe fn noop(_data: *const()) {
}

unsafe fn wake_panic(_data: *const()) {
    panic!("should not be woken");
}

const PANIC_WAKER_VTABLE: RawWakerVTable = RawWakerVTable {
    clone: clone_panic_waker,
    drop: noop,
    wake: wake_panic,
};

fn raw_panic_waker() -> RawWaker {
    RawWaker::new(null(), &PANIC_WAKER_VTABLE)
}

/// Create a new [`Waker`](futures_core::task::Waker) which will
/// panic when `wake()` is called on it. The [`Waker`] can be converted
/// into a [`Waker`] which will behave the same way.
///
/// # Examples
///
/// ```should_panic
/// #![feature(futures_api)]
/// use futures_test::task::panic_waker;
///
/// let lw = panic_waker();
/// lw.wake(); // Will panic
/// ```
pub fn panic_waker() -> Waker {
    unsafe { Waker::new_unchecked(raw_panic_waker()) }
}

/// Get a global reference to a
/// [`Waker`](futures_core::task::Waker) referencing a singleton
/// instance of a [`Waker`] which panics when woken.
///
/// # Examples
///
/// ```should_panic
/// #![feature(async_await, futures_api)]
/// use futures::task;
/// use futures_test::task::panic_waker_ref;
///
/// let lw = panic_waker_ref();
/// lw.wake(); // Will panic
/// ```
pub fn panic_waker_ref() -> &'static Waker {
    thread_local! {
        static PANIC_WAKER_INSTANCE: UnsafeCell<Waker> =
            UnsafeCell::new(panic_waker());
    }
    PANIC_WAKER_INSTANCE.with(|l| unsafe { &*l.get() })
}

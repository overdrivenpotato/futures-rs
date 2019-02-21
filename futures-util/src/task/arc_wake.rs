use std::mem;
use std::sync::Arc;
use std::task::{Waker, RawWaker, RawWakerVTable};

/// A way of waking up a specific task.
///
/// By implementing this trait, types that are expected to be wrapped in an `Arc`
/// can be converted into `Waker` objects.
/// Those Wakers can be used to signal executors that a task it owns
/// is ready to be `poll`ed again.
pub trait ArcWake {
    /// Indicates that the associated task is ready to make progress and should
    /// be `poll`ed.
    ///
    /// This function can be called from an arbitrary thread, including threads which
    /// did not create the `ArcWake` based `Waker`.
    ///
    /// Executors generally maintain a queue of "ready" tasks; `wake` should place
    /// the associated task onto this queue.
    fn wake(arc_self: &Arc<Self>);

    /// Creates a `Waker` from an Arc<T>, if T implements `ArcWake`.
    ///
    /// If `wake()` is called on the returned `Waker`,
    /// the `wake()` function that is defined inside this trait will get called.
    fn into_waker(self: Arc<Self>) -> Waker where Self: Sized
    {
        let ptr = Arc::into_raw(self) as *const();

        unsafe {
            Waker::new_unchecked(RawWaker::new(ptr, waker_vtable!(Self)))
        }
    }
}

// FIXME: panics on Arc::clone / refcount changes could wreak havoc on the
// code here. We should guard against this by aborting.

unsafe fn increase_refcount<T: ArcWake>(data: *const()) {
    // Retain Arc by creating a copy
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    let arc_clone = arc.clone();
    // Forget the Arcs again, so that the refcount isn't decrased
    mem::forget(arc);
    mem::forget(arc_clone);
}

// used by `waker_ref`
pub(super) unsafe fn clone_arc_raw<T: ArcWake>(data: *const()) -> RawWaker {
    increase_refcount::<T>(data);
    RawWaker::new(data, waker_vtable!(T))
}

unsafe fn drop_arc_raw<T: ArcWake>(data: *const()) {
    drop(Arc::<T>::from_raw(data as *const T))
}

// used by `waker_ref`
pub(super) unsafe fn wake_arc_raw<T: ArcWake>(data: *const()) {
    let arc: Arc<T> = Arc::from_raw(data as *const T);
    ArcWake::wake(&arc);
    mem::forget(arc);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct CountingWaker {
        nr_wake: Mutex<i32>,
    }

    impl CountingWaker {
        fn new() -> CountingWaker {
            CountingWaker {
                nr_wake: Mutex::new(0),
            }
        }

        pub fn wakes(&self) -> i32 {
            *self.nr_wake.lock().unwrap()
        }
    }

    impl ArcWake for CountingWaker {
        fn wake(arc_self: &Arc<Self>) {
            let mut lock = arc_self.nr_wake.lock().unwrap();
            *lock += 1;
        }
    }

    #[test]
    fn create_waker_from_arc() {
        let some_w = Arc::new(CountingWaker::new());

        let w1: Waker = ArcWake::into_waker(some_w.clone());
        assert_eq!(2, Arc::strong_count(&some_w));
        w1.wake();
        assert_eq!(1, some_w.wakes());

        let w2 = w1.clone();
        assert_eq!(3, Arc::strong_count(&some_w));

        w2.wake();
        assert_eq!(2, some_w.wakes());

        drop(w2);
        assert_eq!(2, Arc::strong_count(&some_w));
        drop(w1);
        assert_eq!(1, Arc::strong_count(&some_w));
    }
}

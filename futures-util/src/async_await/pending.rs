use core::pin::Pin;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};

/// A macro which yields to the event loop once.
///
/// This is equivalent to returning [`Poll::Pending`](futures_core::task::Poll)
/// from a [`Future::poll`](futures_core::future::Future::poll) implementation.
/// Similarly, when using this macro, it must be ensured that [`wake`](std::task::Waker::wake)
/// is called somewhere when further progress can be made.
///
/// This macro is only usable inside of async functions, closures, and blocks.
#[macro_export]
macro_rules! pending {
    () => {
        await!($crate::async_await::pending_once())
    }
}

#[doc(hidden)]
pub fn pending_once() -> PendingOnce {
    PendingOnce { is_ready: false }
}

#[allow(missing_debug_implementations)]
#[doc(hidden)]
pub struct PendingOnce {
    is_ready: bool,
}

impl Future for PendingOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &Waker) -> Poll<Self::Output> {
        if self.is_ready {
            Poll::Ready(())
        } else {
            self.is_ready = true;
            Poll::Pending
        }
    }
}

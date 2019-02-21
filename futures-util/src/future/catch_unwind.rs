use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use pin_utils::unsafe_pinned;
use std::any::Any;
use std::pin::Pin;
use std::panic::{catch_unwind, UnwindSafe, AssertUnwindSafe};
use std::prelude::v1::*;

/// Future for the `catch_unwind` combinator.
///
/// This is created by the `Future::catch_unwind` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct CatchUnwind<Fut> where Fut: Future {
    future: Fut,
}

impl<Fut> CatchUnwind<Fut> where Fut: Future + UnwindSafe {
    unsafe_pinned!(future: Fut);

    pub(super) fn new(future: Fut) -> CatchUnwind<Fut> {
        CatchUnwind { future }
    }
}

impl<Fut> Future for CatchUnwind<Fut>
    where Fut: Future + UnwindSafe,
{
    type Output = Result<Fut::Output, Box<dyn Any + Send>>;

    fn poll(self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        match catch_unwind(AssertUnwindSafe(|| self.future().poll(waker))) {
            Ok(res) => res.map(Ok),
            Err(e) => Poll::Ready(Err(e))
        }
    }
}

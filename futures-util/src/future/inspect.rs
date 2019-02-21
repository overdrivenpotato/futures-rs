use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Do something with the item of a future, passing it on.
///
/// This is created by the [`super::FutureExt::inspect`] method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Inspect<Fut, F> where Fut: Future {
    future: Fut,
    f: Option<F>,
}

impl<Fut: Future, F: FnOnce(&Fut::Output)> Inspect<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(f: Option<F>);

    pub(super) fn new(future: Fut, f: F) -> Inspect<Fut, F> {
        Inspect {
            future,
            f: Some(f),
        }
    }
}

impl<Fut: Future + Unpin, F> Unpin for Inspect<Fut, F> {}

impl<Fut: Future + FusedFuture, F> FusedFuture for Inspect<Fut, F> {
    fn is_terminated(&self) -> bool { self.future.is_terminated() }
}

impl<Fut, F> Future for Inspect<Fut, F>
    where Fut: Future,
          F: FnOnce(&Fut::Output),
{
    type Output = Fut::Output;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Fut::Output> {
        let e = match self.as_mut().future().poll(waker) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(e) => e,
        };

        let f = self.as_mut().f().take().expect("cannot poll Inspect twice");
        f(&e);
        Poll::Ready(e)
    }
}

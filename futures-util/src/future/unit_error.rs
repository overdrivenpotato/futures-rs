use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Waker, Poll};
use pin_utils::unsafe_pinned;

/// Future for the `unit_error` combinator, turning a `Future` into a `TryFuture`.
///
/// This is created by the `FutureExt::unit_error` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct UnitError<Fut> {
    future: Fut,
}

impl<Fut> UnitError<Fut> {
    unsafe_pinned!(future: Fut);

    /// Creates a new UnitError.
    pub(super) fn new(future: Fut) -> UnitError<Fut> {
        UnitError { future }
    }
}

impl<Fut: Unpin> Unpin for UnitError<Fut> {}

impl<Fut: FusedFuture> FusedFuture for UnitError<Fut> {
    fn is_terminated(&self) -> bool { self.future.is_terminated() }
}

impl<Fut, T> Future for UnitError<Fut>
    where Fut: Future<Output = T>,
{
    type Output = Result<T, ()>;

    fn poll(self: Pin<&mut Self>, waker: &Waker) -> Poll<Result<T, ()>> {
        self.future().poll(waker).map(Ok)
    }
}

use core::pin::Pin;
use futures_core::future::{FusedFuture, Future, TryFuture};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// Future for the [`map_err`](super::TryFutureExt::map_err) combinator.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct MapErr<Fut, F> {
    future: Fut,
    f: Option<F>,
}

impl<Fut, F> MapErr<Fut, F> {
    unsafe_pinned!(future: Fut);
    unsafe_unpinned!(f: Option<F>);

    /// Creates a new MapErr.
    pub(super) fn new(future: Fut, f: F) -> MapErr<Fut, F> {
        MapErr { future, f: Some(f) }
    }
}

impl<Fut: Unpin, F> Unpin for MapErr<Fut, F> {}

impl<Fut, F> FusedFuture for MapErr<Fut, F> {
    fn is_terminated(&self) -> bool { self.f.is_none() }
}

impl<Fut, F, E> Future for MapErr<Fut, F>
    where Fut: TryFuture,
          F: FnOnce(Fut::Error) -> E,
{
    type Output = Result<Fut::Ok, E>;

    fn poll(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Self::Output> {
        match self.as_mut().future().try_poll(waker) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                let f = self.as_mut().f().take()
                    .expect("MapErr must not be polled after it returned `Poll::Ready`");
                Poll::Ready(result.map_err(f))
            }
        }
    }
}

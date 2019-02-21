use super::Chain;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::task::{Waker, Poll};
use pin_utils::unsafe_pinned;

/// Future for the `then` combinator, chaining computations on the end of
/// another future regardless of its outcome.
///
/// This is created by the `Future::then` method.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Then<Fut1, Fut2, F> {
    chain: Chain<Fut1, Fut2, F>,
}

impl<Fut1, Fut2, F> Then<Fut1, Fut2, F>
    where Fut1: Future,
          Fut2: Future,
{
    unsafe_pinned!(chain: Chain<Fut1, Fut2, F>);

    /// Creates a new `Then`.
    pub(super) fn new(future: Fut1, f: F) -> Then<Fut1, Fut2, F> {
        Then {
            chain: Chain::new(future, f),
        }
    }
}

impl<Fut1, Fut2, F> FusedFuture for Then<Fut1, Fut2, F> {
    fn is_terminated(&self) -> bool { self.chain.is_terminated() }
}

impl<Fut1, Fut2, F> Future for Then<Fut1, Fut2, F>
    where Fut1: Future,
          Fut2: Future,
          F: FnOnce(Fut1::Output) -> Fut2,
{
    type Output = Fut2::Output;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Fut2::Output> {
        self.as_mut().chain().poll(waker, |output, f| f(output))
    }
}

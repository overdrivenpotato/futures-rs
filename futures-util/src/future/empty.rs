use core::marker;
use core::pin::Pin;
use futures_core::future::{Future, FusedFuture};
use futures_core::task::{Waker, Poll};

/// A future which is never resolved.
///
/// This future can be created with the [`empty()`] function.
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Empty<T> {
    _data: marker::PhantomData<T>,
}

impl<T> FusedFuture for Empty<T> {
    fn is_terminated(&self) -> bool { false }
}

/// Creates a future which never resolves, representing a computation that never
/// finishes.
///
/// The returned future will forever return [`Poll::Pending`].
///
/// # Examples
///
/// ```ignore
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future;
///
/// let future = future::empty();
/// let () = await!(future);
/// unreachable!();
/// # });
/// ```
pub fn empty<T>() -> Empty<T> {
    Empty { _data: marker::PhantomData }
}

impl<T> Future for Empty<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _: &Waker) -> Poll<T> {
        Poll::Pending
    }
}

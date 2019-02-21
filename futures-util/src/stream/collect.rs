use core::mem;
use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A future which collects all of the values of a stream into a vector.
///
/// This future is created by the `Stream::collect` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Collect<St, C> {
    stream: St,
    collection: C,
}

impl<St: Unpin + Stream, C> Unpin for Collect<St, C> {}

impl<St: Stream, C: Default> Collect<St, C> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(collection: C);

    fn finish(mut self: Pin<&mut Self>) -> C {
        mem::replace(self.as_mut().collection(), Default::default())
    }

    pub(super) fn new(stream: St) -> Collect<St, C> {
        Collect {
            stream,
            collection: Default::default(),
        }
    }
}

impl<St: FusedStream, C> FusedFuture for Collect<St, C> {
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

impl<St, C> Future for Collect<St, C>
where St: Stream,
      C: Default + Extend<St:: Item>
{
    type Output = C;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<C> {
        loop {
            match ready!(self.as_mut().stream().poll_next(waker)) {
                Some(e) => self.as_mut().collection().extend(Some(e)),
                None => return Poll::Ready(self.as_mut().finish()),
            }
        }
    }
}

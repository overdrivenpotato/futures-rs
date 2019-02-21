use crate::stream::{StreamExt, Fuse};
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A `Stream` that implements a `peek` method.
///
/// The `peek` method can be used to retrieve a reference
/// to the next `Stream::Item` if available. A subsequent
/// call to `poll` will return the owned item.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Peekable<St: Stream> {
    stream: Fuse<St>,
    peeked: Option<St::Item>,
}

impl<St: Stream + Unpin> Unpin for Peekable<St> {}

impl<St: Stream> Peekable<St> {
    unsafe_pinned!(stream: Fuse<St>);
    unsafe_unpinned!(peeked: Option<St::Item>);

    pub(super) fn new(stream: St) -> Peekable<St> {
        Peekable {
            stream: stream.fuse(),
            peeked: None
        }
    }

    /// Peek retrieves a reference to the next item in the stream.
    ///
    /// This method polls the underlying stream and return either a reference
    /// to the next item if the stream is ready or passes through any errors.
    pub fn peek<'a>(
        mut self: Pin<&'a mut Self>,
        waker: &Waker,
    ) -> Poll<Option<&'a St::Item>> {
        if self.peeked.is_some() {
            let this: &Self = self.into_ref().get_ref();
            return Poll::Ready(this.peeked.as_ref())
        }
        match ready!(self.as_mut().stream().poll_next(waker)) {
            None => Poll::Ready(None),
            Some(item) => {
                *self.as_mut().peeked() = Some(item);
                let this: &Self = self.into_ref().get_ref();
                Poll::Ready(this.peeked.as_ref())
            }
        }
    }
}

impl<St: Stream> FusedStream for Peekable<St> {
    fn is_terminated(&self) -> bool {
        self.peeked.is_none() && self.stream.is_terminated()
    }
}

impl<S: Stream> Stream for Peekable<S> {
    type Item = S::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Option<Self::Item>> {
        if let Some(item) = self.as_mut().peeked().take() {
            return Poll::Ready(Some(item))
        }
        self.as_mut().stream().poll_next(waker)
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S> Sink for Peekable<S>
    where S: Sink + Stream
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

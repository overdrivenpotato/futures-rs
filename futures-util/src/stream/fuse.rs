use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream which "fuse"s a stream once it's terminated.
///
/// Normally streams can behave unpredictably when used after they have already
/// finished, but `Fuse` continues to return `None` from `poll` forever when
/// finished.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Fuse<St> {
    stream: St,
    done: bool,
}

impl<St: Unpin> Unpin for Fuse<St> {}

impl<St: Stream> Fuse<St> {
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(done: bool);

    pub(super) fn new(stream: St) -> Fuse<St> {
        Fuse { stream, done: false }
    }

    /// Returns whether the underlying stream has finished or not.
    ///
    /// If this method returns `true`, then all future calls to poll are
    /// guaranteed to return `None`. If this returns `false`, then the
    /// underlying stream is still in use.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Acquires a reference to the underlying stream that this combinator is
    /// pulling from.
    pub fn get_ref(&self) -> &St {
        &self.stream
    }

    /// Acquires a mutable reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    pub fn get_mut(&mut self) -> &mut St {
        &mut self.stream
    }

    /// Acquires a mutable pinned reference to the underlying stream that this
    /// combinator is pulling from.
    ///
    /// Note that care must be taken to avoid tampering with the state of the
    /// stream which may otherwise confuse this combinator.
    #[allow(clippy::needless_lifetimes)] // https://github.com/rust-lang/rust/issues/52675
    pub fn get_pin_mut<'a>(self: Pin<&'a mut Self>) -> Pin<&'a mut St> {
        unsafe { Pin::map_unchecked_mut(self, |x| x.get_mut()) }
    }

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<S> FusedStream for Fuse<S> {
    fn is_terminated(&self) -> bool {
        self.done
    }
}

impl<S: Stream> Stream for Fuse<S> {
    type Item = S::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Option<S::Item>> {
        if *self.as_mut().done() {
            return Poll::Ready(None);
        }

        let item = ready!(self.as_mut().stream().poll_next(waker));
        if item.is_none() {
            *self.as_mut().done() = true;
        }
        Poll::Ready(item)
    }
}

// Forwarding impl of Sink from the underlying stream
impl<S: Stream + Sink> Sink for Fuse<S> {
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}

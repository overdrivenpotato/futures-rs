use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which chains a computation onto each item produced by a
/// stream.
///
/// This structure is produced by the `Stream::then` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Then<St, Fut, F> {
    stream: St,
    future: Option<Fut>,
    f: F,
}

impl<St: Unpin, Fut: Unpin, F> Unpin for Then<St, Fut, F> {}

impl<St, Fut, F> Then<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
{
    unsafe_pinned!(stream: St);
    unsafe_pinned!(future: Option<Fut>);
    unsafe_unpinned!(f: F);

    pub(super) fn new(stream: St, f: F) -> Then<St, Fut, F> {
        Then {
            stream,
            future: None,
            f,
        }
    }
}

impl<St: FusedStream, Fut, F> FusedStream for Then<St, Fut, F> {
    fn is_terminated(&self) -> bool {
        self.future.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for Then<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future,
{
    type Item = Fut::Output;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Option<Fut::Output>> {
        if self.as_mut().future().as_pin_mut().is_none() {
            let item = match ready!(self.as_mut().stream().poll_next(waker)) {
                None => return Poll::Ready(None),
                Some(e) => e,
            };
            let fut = (self.as_mut().f())(item);
            self.as_mut().future().set(Some(fut));
        }

        let e = ready!(self.as_mut().future().as_pin_mut().unwrap().poll(waker));
        self.as_mut().future().set(None);
        Poll::Ready(Some(e))
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, U, F> Sink for Then<S, U, F>
    where S: Sink, U: IntoFuture,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
 */

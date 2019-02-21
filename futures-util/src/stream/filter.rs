use core::pin::Pin;
use futures_core::future::Future;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator used to filter the results of a stream and only yield
/// some values.
///
/// This structure is produced by the `Stream::filter` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Filter<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    stream: St,
    f: F,
    pending_fut: Option<Fut>,
    pending_item: Option<St::Item>,
}

impl<St, Fut, F> Filter<St, Fut, F>
where St: Stream,
      F: FnMut(&St::Item) -> Fut,
      Fut: Future<Output = bool>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending_fut: Option<Fut>);
    unsafe_unpinned!(pending_item: Option<St::Item>);

    pub(super) fn new(stream: St, f: F) -> Filter<St, Fut, F> {
        Filter {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
        }
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

    /// Consumes this combinator, returning the underlying stream.
    ///
    /// Note that this may discard intermediate state of this combinator, so
    /// care should be taken to avoid losing resources when this is called.
    pub fn into_inner(self) -> St {
        self.stream
    }
}

impl<St, Fut, F> Unpin for Filter<St, Fut, F>
    where St: Stream + Unpin,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool> + Unpin,
{}

impl<St, Fut, F> FusedStream for Filter<St, Fut, F>
    where St: Stream + FusedStream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    fn is_terminated(&self) -> bool {
        self.pending_fut.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Stream for Filter<St, Fut, F>
    where St: Stream,
          F: FnMut(&St::Item) -> Fut,
          Fut: Future<Output = bool>,
{
    type Item = St::Item;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Option<St::Item>> {
        loop {
            if self.as_mut().pending_fut().as_pin_mut().is_none() {
                let item = match ready!(self.as_mut().stream().poll_next(waker)) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.as_mut().f())(&item);
                self.as_mut().pending_fut().set(Some(fut));
                *self.as_mut().pending_item() = Some(item);
            }

            let yield_item = ready!(self.as_mut().pending_fut().as_pin_mut().unwrap().poll(waker));
            self.as_mut().pending_fut().set(None);
            let item = self.as_mut().pending_item().take().unwrap();

            if yield_item {
                return Poll::Ready(Some(item));
            }
        }
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, R, P> Sink for Filter<S, R, P>
    where S: Stream,
          P: FnMut(&S::Item) -> R,
          R: Future<Item = bool>,
          S: Sink,
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

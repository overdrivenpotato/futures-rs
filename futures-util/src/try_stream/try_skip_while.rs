use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::stream::{Stream, TryStream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which skips elements of a stream while a predicate
/// holds.
///
/// This structure is produced by the
/// [`TryStreamExt::try_skip_while`](super::TryStreamExt::try_skip_while)
/// method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct TrySkipWhile<St, Fut, F> where St: TryStream {
    stream: St,
    f: F,
    pending_fut: Option<Fut>,
    pending_item: Option<St::Ok>,
    done_skipping: bool,
}

impl<St: Unpin + TryStream, Fut: Unpin, F> Unpin for TrySkipWhile<St, Fut, F> {}

impl<St, Fut, F> TrySkipWhile<St, Fut, F>
    where St: TryStream,
          F: FnMut(&St::Ok) -> Fut,
          Fut: TryFuture<Ok = bool, Error = St::Error>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(pending_fut: Option<Fut>);
    unsafe_unpinned!(pending_item: Option<St::Ok>);
    unsafe_unpinned!(done_skipping: bool);

    pub(super) fn new(stream: St, f: F) -> TrySkipWhile<St, Fut, F> {
        TrySkipWhile {
            stream,
            f,
            pending_fut: None,
            pending_item: None,
            done_skipping: false,
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

impl<St, Fut, F> Stream for TrySkipWhile<St, Fut, F>
    where St: TryStream,
          F: FnMut(&St::Ok) -> Fut,
          Fut: TryFuture<Ok = bool, Error = St::Error>,
{
    type Item = Result<St::Ok, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Option<Self::Item>> {
        if self.done_skipping {
            return self.as_mut().stream().try_poll_next(waker);
        }

        loop {
            if self.pending_item.is_none() {
                let item = match ready!(self.as_mut().stream().try_poll_next(waker)?) {
                    Some(e) => e,
                    None => return Poll::Ready(None),
                };
                let fut = (self.as_mut().f())(&item);
                self.as_mut().pending_fut().set(Some(fut));
                *self.as_mut().pending_item() = Some(item);
            }

            let skipped = ready!(self.as_mut().pending_fut().as_pin_mut().unwrap().try_poll(waker)?);
            let item = self.as_mut().pending_item().take().unwrap();
            self.as_mut().pending_fut().set(None);

            if !skipped {
                *self.as_mut().done_skipping() = true;
                return Poll::Ready(Some(Ok(item)))
            }
        }
    }
}

/* TODO
// Forwarding impl of Sink from the underlying stream
impl<S, R, P> Sink for TrySkipWhile<S, R, P>
    where S: Sink + Stream, R: IntoFuture
{
    type SinkItem = S::SinkItem;
    type SinkError = S::SinkError;

    delegate_sink!(stream);
}
*/

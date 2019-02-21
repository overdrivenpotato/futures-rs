use crate::stream::{FuturesUnordered, StreamExt};
use core::pin::Pin;
use core::num::NonZeroUsize;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::Stream;
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which executes a unit closure over each item on a
/// stream concurrently.
///
/// This structure is returned by the
/// [`StreamExt::for_each_concurrent`](super::StreamExt::for_each_concurrent)
/// method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEachConcurrent<St, Fut, F> {
    stream: Option<St>,
    f: F,
    futures: FuturesUnordered<Fut>,
    limit: Option<NonZeroUsize>,
}

impl<St, Fut, F> Unpin for ForEachConcurrent<St, Fut, F>
where St: Unpin,
      Fut: Unpin,
{}

impl<St, Fut, F> ForEachConcurrent<St, Fut, F>
where St: Stream,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()>,
{
    unsafe_pinned!(stream: Option<St>);
    unsafe_unpinned!(f: F);
    unsafe_unpinned!(futures: FuturesUnordered<Fut>);
    unsafe_unpinned!(limit: Option<NonZeroUsize>);

    pub(super) fn new(stream: St, limit: Option<usize>, f: F) -> ForEachConcurrent<St, Fut, F> {
        ForEachConcurrent {
            stream: Some(stream),
            // Note: `limit` = 0 gets ignored.
            limit: limit.and_then(NonZeroUsize::new),
            f,
            futures: FuturesUnordered::new(),
        }
    }
}

impl<St, Fut, F> FusedFuture for ForEachConcurrent<St, Fut, F> {
    fn is_terminated(&self) -> bool {
        self.stream.is_none() && self.futures.is_empty()
    }
}

impl<St, Fut, F> Future for ForEachConcurrent<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<()> {
        loop {
            let mut made_progress_this_iter = false;

            // Try and pull an item from the stream
            let current_len = self.futures.len();
            // Check if we've already created a number of futures greater than `limit`
            if self.limit.map(|limit| limit.get() > current_len).unwrap_or(true) {
                let mut stream_completed = false;
                let elem = if let Some(stream) = self.as_mut().stream().as_pin_mut() {
                    match stream.poll_next(waker) {
                        Poll::Ready(Some(elem)) => {
                            made_progress_this_iter = true;
                            Some(elem)
                        },
                        Poll::Ready(None) => {
                            stream_completed = true;
                            None
                        }
                        Poll::Pending => None,
                    }
                } else {
                    None
                };
                if stream_completed {
                    self.as_mut().stream().set(None);
                }
                if let Some(elem) = elem {
                    let next_future = (self.as_mut().f())(elem);
                    self.as_mut().futures().push(next_future);
                }
            }

            match self.as_mut().futures().poll_next_unpin(waker) {
                Poll::Ready(Some(())) => made_progress_this_iter = true,
                Poll::Ready(None) => {
                    if self.as_mut().stream().is_none() {
                        return Poll::Ready(())
                    }
                },
                Poll::Pending => {}
            }

            if !made_progress_this_iter {
                return Poll::Pending;
            }
        }
    }
}

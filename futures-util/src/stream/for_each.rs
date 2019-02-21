use core::pin::Pin;
use futures_core::future::{FusedFuture, Future};
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

/// A stream combinator which executes a unit closure over each item on a
/// stream.
///
/// This structure is returned by the `Stream::for_each` method.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct ForEach<St, Fut, F> {
    stream: St,
    f: F,
    future: Option<Fut>,
}

impl<St, Fut, F> Unpin for ForEach<St, Fut, F>
where St: Stream + Unpin,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()> + Unpin,
{}

impl<St, Fut, F> ForEach<St, Fut, F>
where St: Stream,
      F: FnMut(St::Item) -> Fut,
      Fut: Future<Output = ()>,
{
    unsafe_pinned!(stream: St);
    unsafe_unpinned!(f: F);
    unsafe_pinned!(future: Option<Fut>);

    pub(super) fn new(stream: St, f: F) -> ForEach<St, Fut, F> {
        ForEach {
            stream,
            f,
            future: None,
        }
    }
}

impl<St: FusedStream, Fut, F> FusedFuture for ForEach<St, Fut, F> {
    fn is_terminated(&self) -> bool {
        self.future.is_none() && self.stream.is_terminated()
    }
}

impl<St, Fut, F> Future for ForEach<St, Fut, F>
    where St: Stream,
          F: FnMut(St::Item) -> Fut,
          Fut: Future<Output = ()>,
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<()> {
        loop {
            if let Some(future) = self.as_mut().future().as_pin_mut() {
                ready!(future.poll(waker));
            }
            self.as_mut().future().as_mut().set(None);

            match ready!(self.as_mut().stream().poll_next(waker)) {
                Some(e) => {
                    let future = (self.as_mut().f())(e);
                    self.as_mut().future().set(Some(future));
                }
                None => {
                    return Poll::Ready(());
                }
            }
        }
    }
}

use crate::stream::{StreamExt, Fuse};
use core::pin::Pin;
use futures_core::stream::{FusedStream, Stream};
use futures_core::task::{Waker, Poll};

/// An adapter for merging the output of two streams.
///
/// The merged stream will attempt to pull items from both input streams. Each
/// stream will be polled in a round-robin fashion, and whenever a stream is
/// ready to yield an item that item is yielded.
///
/// After one of the two input stream completes, the remaining one will be
/// polled exclusively. The returned stream completes when both input
/// streams have completed.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Select<St1, St2> {
    stream1: Fuse<St1>,
    stream2: Fuse<St2>,
    flag: bool,
}

impl<St1: Unpin, St2: Unpin> Unpin for Select<St1, St2> {}

impl<St1, St2> Select<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    pub(super) fn new(stream1: St1, stream2: St2) -> Select<St1, St2> {
        Select {
            stream1: stream1.fuse(),
            stream2: stream2.fuse(),
            flag: false,
        }
    }
}

impl<St1, St2> FusedStream for Select<St1, St2> {
    fn is_terminated(&self) -> bool {
        self.stream1.is_terminated() && self.stream2.is_terminated()
    }
}

impl<St1, St2> Stream for Select<St1, St2>
    where St1: Stream,
          St2: Stream<Item = St1::Item>
{
    type Item = St1::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Option<St1::Item>> {
        let Select { flag, stream1, stream2 } =
            unsafe { Pin::get_unchecked_mut(self) };
        let stream1 = unsafe { Pin::new_unchecked(stream1) };
        let stream2 = unsafe { Pin::new_unchecked(stream2) };

        if !*flag {
            poll_inner(flag, stream1, stream2, waker)
        } else {
            poll_inner(flag, stream2, stream1, waker)
        }
    }
}

fn poll_inner<St1, St2>(
    flag: &mut bool,
    a: Pin<&mut St1>,
    b: Pin<&mut St2>,
    waker: &Waker
) -> Poll<Option<St1::Item>>
    where St1: Stream, St2: Stream<Item = St1::Item>
{
    let a_done = match a.poll_next(waker) {
        Poll::Ready(Some(item)) => {
            // give the other stream a chance to go first next time
            *flag = !*flag;
            return Poll::Ready(Some(item))
        },
        Poll::Ready(None) => true,
        Poll::Pending => false,
    };

    match b.poll_next(waker) {
        Poll::Ready(Some(item)) => {
            Poll::Ready(Some(item))
        }
        Poll::Ready(None) if a_done => Poll::Ready(None),
        Poll::Ready(None) | Poll::Pending => Poll::Pending,
    }
}

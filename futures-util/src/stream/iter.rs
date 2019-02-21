use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Waker, Poll};

/// A stream which is just a shim over an underlying instance of `Iterator`.
///
/// This stream will never block and is always ready.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Iter<I> {
    iter: I,
}

impl<I> Unpin for Iter<I> {}

/// Converts an `Iterator` into a `Stream` which is always ready
/// to yield the next value.
///
/// Iterators in Rust don't express the ability to block, so this adapter
/// simply always calls `iter.next()` and returns that.
///
/// ```
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let mut stream = stream::iter(vec![17, 19]);
/// assert_eq!(vec![17, 19], block_on(stream.collect::<Vec<i32>>()));
/// ```
pub fn iter<I>(i: I) -> Iter<I::IntoIter>
    where I: IntoIterator,
{
    Iter {
        iter: i.into_iter(),
    }
}

impl<I> Stream for Iter<I>
    where I: Iterator,
{
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _: &Waker) -> Poll<Option<I::Item>> {
        Poll::Ready(self.iter.next())
    }
}

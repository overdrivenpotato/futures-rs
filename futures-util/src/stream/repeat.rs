use core::pin::Pin;
use futures_core::stream::Stream;
use futures_core::task::{Waker, Poll};

/// Stream that produces the same element repeatedly.
///
/// This structure is created by the `stream::repeat` function.
#[derive(Debug)]
#[must_use = "streams do nothing unless polled"]
pub struct Repeat<T> {
    item: T,
}

/// Create a stream which produces the same item repeatedly.
///
/// The stream never terminates. Note that you likely want to avoid
/// usage of `collect` or such on the returned stream as it will exhaust
/// available memory as it tries to just fill up all RAM.
///
/// ```
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let mut stream = stream::repeat(9);
/// assert_eq!(vec![9, 9, 9], block_on(stream.take(3).collect::<Vec<i32>>()));
/// ```
pub fn repeat<T>(item: T) -> Repeat<T>
    where T: Clone
{
    Repeat { item }
}

impl<T> Unpin for Repeat<T> {}

impl<T> Stream for Repeat<T>
    where T: Clone
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &Waker) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(self.item.clone()))
    }
}

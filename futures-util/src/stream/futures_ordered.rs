use crate::stream::FuturesUnordered;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_core::task::{Waker, Poll};
use pin_utils::unsafe_pinned;
use std::cmp::{Eq, PartialEq, PartialOrd, Ord, Ordering};
use std::collections::binary_heap::{BinaryHeap, PeekMut};
use std::fmt::{self, Debug};
use std::iter::FromIterator;
use std::pin::Pin;

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
struct OrderWrapper<T> {
    data: T, // A future or a future's output
    index: usize,
}

impl<T> PartialEq for OrderWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for OrderWrapper<T> {}

impl<T> PartialOrd for OrderWrapper<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for OrderWrapper<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max heap, so compare backwards here.
        other.index.cmp(&self.index)
    }
}

impl<T> OrderWrapper<T> {
    unsafe_pinned!(data: T);
}

impl<T> Future for OrderWrapper<T>
    where T: Future
{
    type Output = OrderWrapper<T::Output>;

    fn poll(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Self::Output> {
        self.as_mut().data().as_mut().poll(waker)
            .map(|output| OrderWrapper { data: output, index: self.index })
    }
}

/// An unbounded queue of futures.
///
/// This "combinator" is similar to `FuturesUnordered`, but it imposes an order
/// on top of the set of futures. While futures in the set will race to
/// completion in parallel, results will only be returned in the order their
/// originating futures were added to the queue.
///
/// Futures are pushed into this queue and their realized values are yielded in
/// order. This structure is optimized to manage a large number of futures.
/// Futures managed by `FuturesOrdered` will only be polled when they generate
/// notifications. This reduces the required amount of work needed to coordinate
/// large numbers of futures.
///
/// When a `FuturesOrdered` is first created, it does not contain any futures.
/// Calling `poll` in this state will result in `Poll::Ready(None))` to be
/// returned. Futures are submitted to the queue using `push`; however, the
/// future will **not** be polled at this point. `FuturesOrdered` will only
/// poll managed futures when `FuturesOrdered::poll` is called. As such, it
/// is important to call `poll` after pushing new futures.
///
/// If `FuturesOrdered::poll` returns `Poll::Ready(None)` this means that
/// the queue is currently not managing any futures. A future may be submitted
/// to the queue at a later time. At that point, a call to
/// `FuturesOrdered::poll` will either return the future's resolved value
/// **or** `Poll::Pending` if the future has not yet completed. When
/// multiple futures are submitted to the queue, `FuturesOrdered::poll` will
/// return `Poll::Pending` until the first future completes, even if
/// some of the later futures have already completed.
///
/// Note that you can create a ready-made `FuturesOrdered` via the
/// `futures_ordered` function in the `stream` module, or you can start with an
/// empty queue with the `FuturesOrdered::new` constructor.
#[must_use = "streams do nothing unless polled"]
pub struct FuturesOrdered<T: Future> {
    in_progress_queue: FuturesUnordered<OrderWrapper<T>>,
    queued_outputs: BinaryHeap<OrderWrapper<T::Output>>,
    next_incoming_index: usize,
    next_outgoing_index: usize,
}

impl<T: Future> Unpin for FuturesOrdered<T> {}

/// Converts a list of futures into a `Stream` of results from the futures.
///
/// This function will take a list of futures (e.g. a vector, an iterator,
/// etc), and return a stream. The stream will yield items as they become
/// available on the futures internally, in the order that their originating
/// futures were submitted to the queue. If the futures complete out of order,
/// items will be stored internally within `FuturesOrdered` until all preceding
/// items have been yielded.
///
/// Note that the returned queue can also be used to dynamically push more
/// futures into the queue as they become available.
pub fn futures_ordered<I>(futures: I) -> FuturesOrdered<I::Item>
where
    I: IntoIterator,
    I::Item: Future,
{
    futures.into_iter().collect()
}

impl<Fut: Future> FuturesOrdered<Fut> {
    /// Constructs a new, empty `FuturesOrdered`
    ///
    /// The returned `FuturesOrdered` does not contain any futures and, in this
    /// state, `FuturesOrdered::poll` will return `Ok(Async::Ready(None))`.
    pub fn new() -> FuturesOrdered<Fut> {
        FuturesOrdered {
            in_progress_queue: FuturesUnordered::new(),
            queued_outputs: BinaryHeap::new(),
            next_incoming_index: 0,
            next_outgoing_index: 0,
        }
    }

    /// Returns the number of futures contained in the queue.
    ///
    /// This represents the total number of in-flight futures, both
    /// those currently processing and those that have completed but
    /// which are waiting for earlier futures to complete.
    pub fn len(&self) -> usize {
        self.in_progress_queue.len() + self.queued_outputs.len()
    }

    /// Returns `true` if the queue contains no futures
    pub fn is_empty(&self) -> bool {
        self.in_progress_queue.is_empty() && self.queued_outputs.is_empty()
    }

    /// Push a future into the queue.
    ///
    /// This function submits the given future to the internal set for managing.
    /// This function will not call `poll` on the submitted future. The caller
    /// must ensure that `FuturesOrdered::poll` is called in order to receive
    /// task notifications.
    pub fn push(&mut self, future: Fut) {
        let wrapped = OrderWrapper {
            data: future,
            index: self.next_incoming_index,
        };
        self.next_incoming_index += 1;
        self.in_progress_queue.push(wrapped);
    }
}

impl<Fut: Future> Default for FuturesOrdered<Fut> {
    fn default() -> FuturesOrdered<Fut> {
        FuturesOrdered::new()
    }
}

impl<Fut: Future> Stream for FuturesOrdered<Fut> {
    type Item = Fut::Output;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        // Check to see if we've already received the next value
        if let Some(next_output) = this.queued_outputs.peek_mut() {
            if next_output.index == this.next_outgoing_index {
                this.next_outgoing_index += 1;
                return Poll::Ready(Some(PeekMut::pop(next_output).data));
            }
        }

        loop {
            match Pin::new(&mut this.in_progress_queue).poll_next(waker) {
                Poll::Ready(Some(output)) => {
                    if output.index == this.next_outgoing_index {
                        this.next_outgoing_index += 1;
                        return Poll::Ready(Some(output.data));
                    } else {
                        this.queued_outputs.push(output)
                    }
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<Fut: Future> Debug for FuturesOrdered<Fut> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "FuturesOrdered {{ ... }}")
    }
}

impl<Fut: Future> FromIterator<Fut> for FuturesOrdered<Fut> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Fut>,
    {
        let acc = FuturesOrdered::new();
        iter.into_iter().fold(acc, |mut acc, item| { acc.push(item); acc })
    }
}

use futures_core::stream::Stream;
use futures_core::task::{Waker, Poll};
use futures_sink::Sink;
use pin_utils::{unsafe_pinned, unsafe_unpinned};
use std::collections::VecDeque;
use std::pin::Pin;

/// Sink for the `Sink::buffer` combinator, which buffers up to some fixed
/// number of values when the underlying sink is unable to accept them.
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Buffer<Si: Sink> {
    sink: Si,
    buf: VecDeque<Si::SinkItem>,

    // Track capacity separately from the `VecDeque`, which may be rounded up
    capacity: usize,
}

impl<Si: Sink + Unpin> Unpin for Buffer<Si> {}

impl<Si: Sink> Buffer<Si> {
    unsafe_pinned!(sink: Si);
    unsafe_unpinned!(buf: VecDeque<Si::SinkItem>);
    unsafe_unpinned!(capacity: usize);


    pub(super) fn new(sink: Si, capacity: usize) -> Buffer<Si> {
        Buffer {
            sink,
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Get a shared reference to the inner sink.
    pub fn get_ref(&self) -> &Si {
        &self.sink
    }

    fn try_empty_buffer(
        mut self: Pin<&mut Self>,
        waker: &Waker
    ) -> Poll<Result<(), Si::SinkError>> {
        try_ready!(self.as_mut().sink().poll_ready(waker));
        while let Some(item) = self.as_mut().buf().pop_front() {
            if let Err(e) = self.as_mut().sink().start_send(item) {
                return Poll::Ready(Err(e));
            }
            if !self.buf.is_empty() {
                try_ready!(self.as_mut().sink().poll_ready(waker));
            }
        }
        Poll::Ready(Ok(()))
    }
}

// Forwarding impl of Stream from the underlying sink
impl<S> Stream for Buffer<S> where S: Sink + Stream {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, waker: &Waker) -> Poll<Option<S::Item>> {
        self.sink().poll_next(waker)
    }
}

impl<Si: Sink> Sink for Buffer<Si> {
    type SinkItem = Si::SinkItem;
    type SinkError = Si::SinkError;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Result<(), Self::SinkError>> {
        if self.capacity == 0 {
            return self.as_mut().sink().poll_ready(waker);
        }

        if let Poll::Ready(Err(e)) = self.as_mut().try_empty_buffer(waker) {
            return Poll::Ready(Err(e));
        }

        if self.buf.len() >= self.capacity {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        if self.capacity == 0 {
            self.as_mut().sink().start_send(item)
        } else {
            self.as_mut().buf().push_back(item);
            Ok(())
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.as_mut().try_empty_buffer(waker));
        debug_assert!(self.as_mut().buf().is_empty());
        self.as_mut().sink().poll_flush(waker)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Result<(), Self::SinkError>> {
        try_ready!(self.as_mut().try_empty_buffer(waker));
        debug_assert!(self.as_mut().buf().is_empty());
        self.as_mut().sink().poll_close(waker)
    }
}

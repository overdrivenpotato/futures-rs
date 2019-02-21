use crate::io::AsyncRead;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use std::io;
use std::pin::Pin;

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
#[derive(Debug)]
pub struct Read<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut [u8],
}

// Pinning is never projected to fields
impl<R: ?Sized> Unpin for Read<'_, R> {}

impl<'a, R: AsyncRead + ?Sized> Read<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut [u8]) -> Self {
        Read { reader, buf }
    }
}

impl<R: AsyncRead + ?Sized> Future for Read<'_, R> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        let this = &mut *self;
        this.reader.poll_read(waker, this.buf)
    }
}

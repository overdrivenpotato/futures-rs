use futures_01::executor::{
    spawn as spawn01, Notify as Notify01, NotifyHandle as NotifyHandle01,
    Spawn as Spawn01, UnsafeNotify as UnsafeNotify01,
};
use futures_01::{
    Async as Async01, AsyncSink as AsyncSink01, Future as Future01,
    Sink as Sink01, Stream as Stream01,
};
use futures_core::{task as task03, Future as Future03, Stream as Stream03};
use std::pin::Pin;
use futures_sink::Sink as Sink03;
use std::task::Waker;

/// Converts a futures 0.1 Future, Stream, AsyncRead, or AsyncWrite
/// object to a futures 0.3-compatible version,
#[derive(Debug)]
#[must_use = "futures do nothing unless polled"]
pub struct Compat01As03<T> {
    pub(crate) inner: Spawn01<T>,
}

impl<T> Unpin for Compat01As03<T> {}

impl<T> Compat01As03<T> {
    /// Wraps a futures 0.1 Future, Stream, AsyncRead, or AsyncWrite
    /// object in a futures 0.3-compatible wrapper.
    pub fn new(object: T) -> Compat01As03<T> {
        Compat01As03 {
            inner: spawn01(object),
        }
    }

    fn in_notify<R>(&mut self, waker: &Waker, f: impl FnOnce(&mut T) -> R) -> R {
        let notify = &WakerToHandle(waker);
        self.inner.poll_fn_notify(notify, 0, f)
    }
}

/// Extension trait for futures 0.1 [`Future`](futures::future::Future)
pub trait Future01CompatExt: Future01 {
    /// Converts a futures 0.1
    /// [`Future<Item = T, Error = E>`](futures::future::Future)
    /// into a futures 0.3
    /// [`Future<Output = Result<T, E>>`](futures_core::future::Future).
    fn compat(self) -> Compat01As03<Self>
    where
        Self: Sized,
    {
        Compat01As03::new(self)
    }
}
impl<Fut: Future01> Future01CompatExt for Fut {}

/// Extension trait for futures 0.1 [`Stream`](futures::stream::Stream)
pub trait Stream01CompatExt: Stream01 {
    /// Converts a futures 0.1
    /// [`Stream<Item = T, Error = E>`](futures::stream::Stream)
    /// into a futures 0.3
    /// [`Stream<Item = Result<T, E>>`](futures_core::stream::Stream).
    fn compat(self) -> Compat01As03<Self>
    where
        Self: Sized,
    {
        Compat01As03::new(self)
    }
}
impl<St: Stream01> Stream01CompatExt for St {}

/// Extension trait for futures 0.1 [`Sink`](futures::sink::Sink)
pub trait Sink01CompatExt: Sink01 {
    /// Converts a futures 0.1
    /// [`Sink<SinkItem = T, SinkError = E>`](futures::sink::Sink)
    /// into a futures 0.3
    /// [`Sink<SinkItem = T, SinkError = E>`](futures_sink::sink::Sink).
    fn sink_compat(self) -> Compat01As03Sink<Self, Self::SinkItem>
    where
        Self: Sized,
    {
        Compat01As03Sink::new(self)
    }
}
impl<Si: Sink01> Sink01CompatExt for Si {}

fn poll_01_to_03<T, E>(x: Result<Async01<T>, E>) -> task03::Poll<Result<T, E>> {
    match x {
        Ok(Async01::Ready(t)) => task03::Poll::Ready(Ok(t)),
        Ok(Async01::NotReady) => task03::Poll::Pending,
        Err(e) => task03::Poll::Ready(Err(e)),
    }
}

impl<Fut: Future01> Future03 for Compat01As03<Fut> {
    type Output = Result<Fut::Item, Fut::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> task03::Poll<Self::Output> {
        poll_01_to_03(self.in_notify(waker, Future01::poll))
    }
}

impl<St: Stream01> Stream03 for Compat01As03<St> {
    type Item = Result<St::Item, St::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> task03::Poll<Option<Self::Item>> {
        match self.in_notify(waker, Stream01::poll) {
            Ok(Async01::Ready(Some(t))) => task03::Poll::Ready(Some(Ok(t))),
            Ok(Async01::Ready(None)) => task03::Poll::Ready(None),
            Ok(Async01::NotReady) => task03::Poll::Pending,
            Err(e) => task03::Poll::Ready(Some(Err(e))),
        }
    }
}

/// Converts a futures 0.1 Sink object to a futures 0.3-compatible version
#[derive(Debug)]
#[must_use = "sinks do nothing unless polled"]
pub struct Compat01As03Sink<S, SinkItem> {
    pub(crate) inner: Spawn01<S>,
    pub(crate) buffer: Option<SinkItem>,
    pub(crate) close_started: bool,
}

impl<S, SinkItem> Unpin for Compat01As03Sink<S, SinkItem> {}

impl<S, SinkItem> Compat01As03Sink<S, SinkItem> {
    /// Wraps a futures 0.1 Sink object in a futures 0.3-compatible wrapper.
    pub fn new(inner: S) -> Compat01As03Sink<S, SinkItem> {
        Compat01As03Sink {
            inner: spawn01(inner),
            buffer: None,
            close_started: false
        }
    }

    fn in_notify<R>(
        &mut self,
        waker: &Waker,
        f: impl FnOnce(&mut S) -> R,
    ) -> R {
        let notify = &WakerToHandle(waker);
        self.inner.poll_fn_notify(notify, 0, f)
    }
}

impl<S, SinkItem> Stream03 for Compat01As03Sink<S, SinkItem>
where
    S: Stream01,
{
    type Item = Result<S::Item, S::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> task03::Poll<Option<Self::Item>> {
        match self.in_notify(waker, |f| f.poll()) {
            Ok(Async01::Ready(Some(t))) => task03::Poll::Ready(Some(Ok(t))),
            Ok(Async01::Ready(None)) => task03::Poll::Ready(None),
            Ok(Async01::NotReady) => task03::Poll::Pending,
            Err(e) => task03::Poll::Ready(Some(Err(e))),
        }
    }
}

impl<S, SinkItem> Sink03 for Compat01As03Sink<S, SinkItem>
where
    S: Sink01<SinkItem = SinkItem>,
{
    type SinkItem = SinkItem;
    type SinkError = S::SinkError;

    fn start_send(
        mut self: Pin<&mut Self>,
        item: Self::SinkItem,
    ) -> Result<(), Self::SinkError> {
        debug_assert!(self.buffer.is_none());
        self.buffer = Some(item);
        Ok(())
    }

    fn poll_ready(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> task03::Poll<Result<(), Self::SinkError>> {
        match self.buffer.take() {
            Some(item) => match self.in_notify(waker, |f| f.start_send(item)) {
                Ok(AsyncSink01::Ready) => task03::Poll::Ready(Ok(())),
                Ok(AsyncSink01::NotReady(i)) => {
                    self.buffer = Some(i);
                    task03::Poll::Pending
                }
                Err(e) => task03::Poll::Ready(Err(e)),
            },
            None => task03::Poll::Ready(Ok(())),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> task03::Poll<Result<(), Self::SinkError>> {
        let item = self.buffer.take();
        match self.in_notify(waker, |f| match item {
            Some(i) => match f.start_send(i) {
                Ok(AsyncSink01::Ready) => f.poll_complete().map(|i| (i, None)),
                Ok(AsyncSink01::NotReady(t)) => {
                    Ok((Async01::NotReady, Some(t)))
                }
                Err(e) => Err(e),
            },
            None => f.poll_complete().map(|i| (i, None)),
        }) {
            Ok((Async01::Ready(_), _)) => task03::Poll::Ready(Ok(())),
            Ok((Async01::NotReady, item)) => {
                self.buffer = item;
                task03::Poll::Pending
            }
            Err(e) => task03::Poll::Ready(Err(e)),
        }
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        waker: &Waker,
    ) -> task03::Poll<Result<(), Self::SinkError>> {
        let item = self.buffer.take();
        let close_started = self.close_started;

        let result = self.in_notify(waker, |f| {
            if !close_started {
                if let Some(item) = item {
                    if let AsyncSink01::NotReady(item) = f.start_send(item)? {
                        return Ok((Async01::NotReady, Some(item), false));
                    }
                }

                if let Async01::NotReady = f.poll_complete()? {
                    return Ok((Async01::NotReady, None, false));
                }
            }

            Ok((<S as Sink01>::close(f)?, None, true))
        });

        match result {
            Ok((Async01::Ready(_), _, _)) => task03::Poll::Ready(Ok(())),
            Ok((Async01::NotReady, item, close_started)) => {
                self.buffer = item;
                self.close_started = close_started;
                task03::Poll::Pending
            }
            Err(e) => task03::Poll::Ready(Err(e)),
        }
    }
}

struct NotifyWaker(task03::Waker);

#[derive(Clone)]
struct WakerToHandle<'a>(&'a task03::Waker);

impl<'a> From<WakerToHandle<'a>> for NotifyHandle01 {
    fn from(handle: WakerToHandle<'a>) -> NotifyHandle01 {
        let ptr = Box::new(NotifyWaker(handle.0.clone()));

        unsafe { NotifyHandle01::new(Box::into_raw(ptr)) }
    }
}

impl Notify01 for NotifyWaker {
    fn notify(&self, _: usize) {
        self.0.wake();
    }
}

unsafe impl UnsafeNotify01 for NotifyWaker {
    unsafe fn clone_raw(&self) -> NotifyHandle01 {
        WakerToHandle(&self.0).into()
    }

    unsafe fn drop_raw(&self) {
        let ptr: *const dyn UnsafeNotify01 = self;
        drop(Box::from_raw(ptr as *mut dyn UnsafeNotify01));
    }
}

#[cfg(feature = "io-compat")]
mod io {
    use super::*;
    use futures_io::{
        AsyncRead as AsyncRead03, AsyncWrite as AsyncWrite03, Initializer,
    };
    use std::io::Error;
    use tokio_io::{AsyncRead as AsyncRead01, AsyncWrite as AsyncWrite01};

    impl<R: AsyncRead01> AsyncRead03 for Compat01As03<R> {
        unsafe fn initializer(&self) -> Initializer {
            // check if `prepare_uninitialized_buffer` needs zeroing
            if self.inner.get_ref().prepare_uninitialized_buffer(&mut [1]) {
                Initializer::zeroing()
            } else {
                Initializer::nop()
            }
        }

        fn poll_read(&mut self, waker: &task03::Waker, buf: &mut [u8])
            -> task03::Poll<Result<usize, Error>>
        {
            poll_01_to_03(self.in_notify(waker, |x| x.poll_read(buf)))
        }
    }

    impl<W: AsyncWrite01> AsyncWrite03 for Compat01As03<W> {
        fn poll_write(&mut self, waker: &task03::Waker, buf: &[u8])
            -> task03::Poll<Result<usize, Error>>
        {
            poll_01_to_03(self.in_notify(waker, |x| x.poll_write(buf)))
        }

        fn poll_flush(&mut self, waker: &task03::Waker)
            -> task03::Poll<Result<(), Error>>
        {
            poll_01_to_03(self.in_notify(waker, AsyncWrite01::poll_flush))
        }

        fn poll_close(&mut self, waker: &task03::Waker)
            -> task03::Poll<Result<(), Error>>
        {
            poll_01_to_03(self.in_notify(waker, AsyncWrite01::shutdown))
        }
    }
}

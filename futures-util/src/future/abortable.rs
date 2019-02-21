use crate::task::AtomicWaker;
use futures_core::future::Future;
use futures_core::task::{Waker, Poll};
use pin_utils::unsafe_pinned;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A future which can be remotely short-circuited using an `AbortHandle`.
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless polled"]
pub struct Abortable<Fut> {
    future: Fut,
    inner: Arc<AbortInner>,
}

impl<Fut: Unpin> Unpin for Abortable<Fut> {}

impl<Fut> Abortable<Fut> where Fut: Future {
    unsafe_pinned!(future: Fut);

    /// Creates a new `Abortable` future using an existing `AbortRegistration`.
    /// `AbortRegistration`s can be acquired through `AbortHandle::new`.
    ///
    /// When `abort` is called on the handle tied to `reg` or if `abort` has
    /// already been called, the future will complete immediately without making
    /// any further progress.
    ///
    /// Example:
    ///
    /// ```
    /// use futures::future::{ready, Abortable, AbortHandle, Aborted};
    /// use futures::executor::block_on;
    ///
    /// let (abort_handle, abort_registration) = AbortHandle::new_pair();
    /// let future = Abortable::new(ready(2), abort_registration);
    /// abort_handle.abort();
    /// assert_eq!(block_on(future), Err(Aborted));
    /// ```
    pub fn new(future: Fut, reg: AbortRegistration) -> Self {
        Abortable {
            future,
            inner: reg.inner,
        }
    }
}

/// A registration handle for a `Abortable` future.
/// Values of this type can be acquired from `AbortHandle::new` and are used
/// in calls to `Abortable::new`.
#[derive(Debug)]
pub struct AbortRegistration {
    inner: Arc<AbortInner>,
}

/// A handle to a `Abortable` future.
#[derive(Debug, Clone)]
pub struct AbortHandle {
    inner: Arc<AbortInner>,
}

impl AbortHandle {
    /// Creates an (`AbortHandle`, `AbortRegistration`) pair which can be used
    /// to abort a running future.
    ///
    /// This function is usually paired with a call to `Abortable::new`.
    ///
    /// Example:
    ///
    /// ```
    /// use futures::future::{ready, Abortable, AbortHandle, Aborted};
    /// use futures::executor::block_on;
    ///
    /// let (abort_handle, abort_registration) = AbortHandle::new_pair();
    /// let future = Abortable::new(ready(2), abort_registration);
    /// abort_handle.abort();
    /// assert_eq!(block_on(future), Err(Aborted));
    pub fn new_pair() -> (Self, AbortRegistration) {
        let inner = Arc::new(AbortInner {
            waker: AtomicWaker::new(),
            cancel: AtomicBool::new(false),
        });

        (
            AbortHandle {
                inner: inner.clone(),
            },
            AbortRegistration {
                inner,
            },
        )
    }
}

// Inner type storing the waker to awaken and a bool indicating that it
// should be cancelled.
#[derive(Debug)]
struct AbortInner {
    waker: AtomicWaker,
    cancel: AtomicBool,
}

/// Creates a new `Abortable` future and a `AbortHandle` which can be used to stop it.
///
/// This function is a convenient (but less flexible) alternative to calling
/// `AbortHandle::new` and `Abortable::new` manually.
pub fn abortable<Fut>(future: Fut) -> (Abortable<Fut>, AbortHandle)
    where Fut: Future
{
    let (handle, reg) = AbortHandle::new_pair();
    (
        Abortable::new(future, reg),
        handle,
    )
}

/// Indicator that the `Abortable` future was aborted.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Aborted;

impl<Fut> Future for Abortable<Fut> where Fut: Future {
    type Output = Result<Fut::Output, Aborted>;

    fn poll(mut self: Pin<&mut Self>, waker: &Waker) -> Poll<Self::Output> {
        // Check if the future has been aborted
        if self.inner.cancel.load(Ordering::Relaxed) {
            return Poll::Ready(Err(Aborted))
        }

        // attempt to complete the future
        if let Poll::Ready(x) = self.as_mut().future().poll(waker) {
            return Poll::Ready(Ok(x))
        }

        // Register to receive a wakeup if the future is aborted in the... future
        self.inner.waker.register(waker);

        // Check to see if the future was aborted between the first check and
        // registration.
        // Checking with `Relaxed` is sufficient because `register` introduces an
        // `AcqRel` barrier.
        if self.inner.cancel.load(Ordering::Relaxed) {
            return Poll::Ready(Err(Aborted))
        }

        Poll::Pending
    }
}

impl AbortHandle {
    /// Abort the `Abortable` future associated with this handle.
    ///
    /// Notifies the Abortable future associated with this handle that it
    /// should abort. Note that if the future is currently being polled on
    /// another thread, it will not immediately stop running. Instead, it will
    /// continue to run until its poll method returns.
    pub fn abort(&self) {
        self.inner.cancel.store(true, Ordering::Relaxed);
        self.inner.waker.wake();
    }
}

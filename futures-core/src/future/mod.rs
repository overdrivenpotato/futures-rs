//! Futures.

use crate::task::{Waker, Poll};
use core::pin::Pin;

pub use core::future::Future;

mod future_obj;
pub use self::future_obj::{FutureObj, LocalFutureObj, UnsafeFutureObj};

/// A `Future` or `TryFuture` which tracks whether or not the underlying future
/// should no longer be polled.
///
/// `is_terminated` will return `true` if a future should no longer be polled.
/// Usually, this state occurs after `poll` (or `try_poll`) returned
/// `Poll::Ready`. However, `is_terminated` may also return `true` if a future
/// has become inactive and can no longer make progress and should be ignored
/// or dropped rather than being `poll`ed again.
pub trait FusedFuture {
    /// Returns `true` if the underlying future should no longer be polled.
    fn is_terminated(&self) -> bool;
}

impl<F: FusedFuture> FusedFuture for Pin<&mut F> {
    fn is_terminated(&self) -> bool {
        <F as FusedFuture>::is_terminated(&**self)
    }
}

impl<F: FusedFuture + ?Sized> FusedFuture for &mut F {
    fn is_terminated(&self) -> bool {
        <F as FusedFuture>::is_terminated(&**self)
    }
}

#[cfg(feature = "std")]
mod if_std {
    use super::*;
    impl<F: FusedFuture + ?Sized> FusedFuture for Box<F> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }

    impl<F: FusedFuture + ?Sized> FusedFuture for Pin<Box<F>> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }

    impl<F: FusedFuture> FusedFuture for std::panic::AssertUnwindSafe<F> {
        fn is_terminated(&self) -> bool {
            <F as FusedFuture>::is_terminated(&**self)
        }
    }
}

/// A convenience for futures that return `Result` values that includes
/// a variety of adapters tailored to such futures.
pub trait TryFuture {
    /// The type of successful values yielded by this future
    type Ok;

    /// The type of failures yielded by this future
    type Error;

    /// Poll this `TryFuture` as if it were a `Future`.
    ///
    /// This method is a stopgap for a compiler limitation that prevents us from
    /// directly inheriting from the `Future` trait; in the future it won't be
    /// needed.
    fn try_poll(
        self: Pin<&mut Self>,
        waker: &Waker,
    ) -> Poll<Result<Self::Ok, Self::Error>>;
}

impl<F, T, E> TryFuture for F
    where F: Future<Output = Result<T, E>>
{
    type Ok = T;
    type Error = E;

    #[inline]
    fn try_poll(self: Pin<&mut Self>, waker: &Waker) -> Poll<F::Output> {
        self.poll(waker)
    }
}

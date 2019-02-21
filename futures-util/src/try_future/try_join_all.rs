//! Definition of the `TryJoinAll` combinator, waiting for all of a list of
//! futures to finish with either success or error.

use std::fmt;
use std::future::Future;
use std::iter::FromIterator;
use std::mem;
use std::pin::Pin;
use std::prelude::v1::*;
use std::task::Poll;

use super::TryFuture;

#[derive(Debug)]
enum ElemState<F>
where
    F: TryFuture,
{
    Pending(F),
    Done(Option<F::Ok>),
}

impl<F> ElemState<F>
where
    F: TryFuture,
{
    fn pending_pin_mut<'a>(self: Pin<&'a mut Self>) -> Option<Pin<&'a mut F>> {
        // Safety: Basic enum pin projection, no drop + optionally Unpin based
        // on the type of this variant
        match unsafe { self.get_unchecked_mut() } {
            ElemState::Pending(f) => Some(unsafe { Pin::new_unchecked(f) }),
            ElemState::Done(_) => None,
        }
    }

    fn take_done(self: Pin<&mut Self>) -> Option<F::Ok> {
        // Safety: Going from pin to a variant we never pin-project
        match unsafe { self.get_unchecked_mut() } {
            ElemState::Pending(_) => None,
            ElemState::Done(output) => output.take(),
        }
    }
}

impl<F> Unpin for ElemState<F> where F: TryFuture + Unpin {}

fn iter_pin_mut<T>(slice: Pin<&mut [T]>) -> impl Iterator<Item = Pin<&mut T>> {
    // Safety: `std` _could_ make this unsound if it were to decide Pin's
    // invariants aren't required to transmit through slices. Otherwise this has
    // the same safety as a normal field pin projection.
    unsafe { slice.get_unchecked_mut() }
        .iter_mut()
        .map(|t| unsafe { Pin::new_unchecked(t) })
}

enum FinalState<E = ()> {
    Pending,
    AllDone,
    Error(E)
}

/// A future which takes a list of futures and resolves with a vector of the
/// completed values or an error.
///
/// This future is created with the `try_join_all` function.
#[must_use = "futures do nothing unless polled"]
pub struct TryJoinAll<F>
where
    F: TryFuture,
{
    elems: Pin<Box<[ElemState<F>]>>,
}

impl<F> fmt::Debug for TryJoinAll<F>
where
    F: TryFuture + fmt::Debug,
    F::Ok: fmt::Debug,
    F::Error: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("TryJoinAll")
            .field("elems", &self.elems)
            .finish()
    }
}

/// Creates a future which represents either a collection of the results of the
/// futures given or an error.
///
/// The returned future will drive execution for all of its underlying futures,
/// collecting the results into a destination `Vec<T>` in the same order as they
/// were provided.
///
/// If any future returns an error then all other futures will be canceled and
/// an error will be returned immediately. If all futures complete successfully,
/// however, then the returned future will succeed with a `Vec` of all the
/// successful results.
///
/// # Examples
///
/// ```
/// #![feature(async_await, await_macro, futures_api)]
/// # futures::executor::block_on(async {
/// use futures::future::{self, try_join_all};
///
/// let futures = vec![
///     future::ok::<u32, u32>(1),
///     future::ok::<u32, u32>(2),
///     future::ok::<u32, u32>(3),
/// ];
///
/// assert_eq!(await!(try_join_all(futures)), Ok(vec![1, 2, 3]));
///
/// let futures = vec![
///     future::ok::<u32, u32>(1),
///     future::err::<u32, u32>(2),
///     future::ok::<u32, u32>(3),
/// ];
///
/// assert_eq!(await!(try_join_all(futures)), Err(2));
/// # });
/// ```
pub fn try_join_all<I>(i: I) -> TryJoinAll<I::Item>
where
    I: IntoIterator,
    I::Item: TryFuture,
{
    let elems: Box<[_]> = i.into_iter().map(ElemState::Pending).collect();
    TryJoinAll {
        elems: Box::into_pin(elems),
    }
}

impl<F> Future for TryJoinAll<F>
where
    F: TryFuture,
{
    type Output = Result<Vec<F::Ok>, F::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        waker: &::std::task::Waker,
    ) -> Poll<Self::Output> {
        let mut state = FinalState::AllDone;

        for mut elem in iter_pin_mut(self.elems.as_mut()) {
            if let Some(pending) = elem.as_mut().pending_pin_mut() {
                match pending.try_poll(waker) {
                    Poll::Pending => state = FinalState::Pending,
                    Poll::Ready(output) => match output {
                        Ok(item) => elem.set(ElemState::Done(Some(item))),
                        Err(e) => {
                            state = FinalState::Error(e);
                            break;
                        }
                    }
                }
            }
        }

        match state {
            FinalState::Pending => Poll::Pending,
            FinalState::AllDone => {
                let mut elems = mem::replace(&mut self.elems, Box::pin([]));
                let results = iter_pin_mut(elems.as_mut())
                    .map(|e| e.take_done().unwrap())
                    .collect();
                Poll::Ready(Ok(results))
            },
            FinalState::Error(e) => {
                let _ = mem::replace(&mut self.elems, Box::pin([]));
                Poll::Ready(Err(e))
            },
        }
    }
}

impl<F: TryFuture> FromIterator<F> for TryJoinAll<F> {
    fn from_iter<T: IntoIterator<Item = F>>(iter: T) -> Self {
        try_join_all(iter)
    }
}

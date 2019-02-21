use core::pin::Pin;
use futures_core::future::TryFuture;
use futures_core::task::{Waker, Poll};

#[must_use = "futures do nothing unless polled"]
#[derive(Debug)]
pub(crate) enum TryChain<Fut1, Fut2, Data> {
    First(Fut1, Option<Data>),
    Second(Fut2),
    Empty,
}

pub(crate) enum TryChainAction<Fut2>
    where Fut2: TryFuture,
{
    Future(Fut2),
    Output(Result<Fut2::Ok, Fut2::Error>),
}

impl<Fut1, Fut2, Data> TryChain<Fut1, Fut2, Data>
    where Fut1: TryFuture,
          Fut2: TryFuture,
{
    pub(crate) fn new(fut1: Fut1, data: Data) -> TryChain<Fut1, Fut2, Data> {
        TryChain::First(fut1, Some(data))
    }

    pub(crate) fn is_terminated(&self) -> bool {
        match self {
            TryChain::First(..) | TryChain::Second(_) => true,
            TryChain::Empty => false,
        }
    }

    pub(crate) fn poll<F>(
        self: Pin<&mut Self>,
        waker: &Waker,
        f: F,
    ) -> Poll<Result<Fut2::Ok, Fut2::Error>>
        where F: FnOnce(Result<Fut1::Ok, Fut1::Error>, Data) -> TryChainAction<Fut2>,
    {
        let mut f = Some(f);

        // Safe to call `get_unchecked_mut` because we won't move the futures.
        let this = unsafe { Pin::get_unchecked_mut(self) };

        loop {
            let (output, data) = match this {
                TryChain::First(fut1, data) => {
                    // Poll the first future
                    match unsafe { Pin::new_unchecked(fut1) }.try_poll(waker) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(output) => (output, data.take().unwrap()),
                    }
                }
                TryChain::Second(fut2) => {
                    // Poll the second future
                    return unsafe { Pin::new_unchecked(fut2) }.try_poll(waker)
                }
                TryChain::Empty => {
                    panic!("future must not be polled after it returned `Poll::Ready`");
                }
            };

            *this = TryChain::Empty; // Drop fut1
            let f = f.take().unwrap();
            match f(output, data) {
                TryChainAction::Future(fut2) => *this = TryChain::Second(fut2),
                TryChainAction::Output(output) => return Poll::Ready(output),
            }
        }
    }
}

#![feature(futures_api)]

use futures::channel::oneshot;
use futures::executor::{block_on, block_on_stream};
use futures::future::{self, FutureExt, FutureObj};
use futures::stream::{StreamExt, futures_unordered, FuturesUnordered};
use futures::task::Poll;
use futures_test::{assert_stream_done, assert_stream_next};
use futures_test::future::FutureTestExt;
use futures_test::task::noop_waker_ref;
use std::boxed::Box;

#[test]
fn works_1() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut iter = block_on_stream(futures_unordered(vec![a_rx, b_rx, c_rx]));

    b_tx.send(99).unwrap();
    assert_eq!(Some(Ok(99)), iter.next());

    a_tx.send(33).unwrap();
    c_tx.send(33).unwrap();
    assert_eq!(Some(Ok(33)), iter.next());
    assert_eq!(Some(Ok(33)), iter.next());
    assert_eq!(None, iter.next());
}

#[test]
fn works_2() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = futures_unordered(vec![
        FutureObj::new(Box::new(a_rx)),
        FutureObj::new(Box::new(b_rx.join(c_rx).map(|(a, b)| Ok(a? + b?)))),
    ]);

    a_tx.send(9).unwrap();
    b_tx.send(10).unwrap();

    let lw = &noop_waker_ref();
    assert_eq!(stream.poll_next_unpin(lw), Poll::Ready(Some(Ok(9))));
    c_tx.send(20).unwrap();
    assert_eq!(stream.poll_next_unpin(lw), Poll::Ready(Some(Ok(30))));
    assert_eq!(stream.poll_next_unpin(lw), Poll::Ready(None));
}

#[test]
fn from_iterator() {
    let stream = vec![
        future::ready::<i32>(1),
        future::ready::<i32>(2),
        future::ready::<i32>(3)
    ].into_iter().collect::<FuturesUnordered<_>>();
    assert_eq!(stream.len(), 3);
    assert_eq!(block_on(stream.collect::<Vec<_>>()), vec![1,2,3]);
}

/* ToDo: This requires FutureExt::select to be implemented
#[test]
fn finished_future() {
    let (_a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = futures_unordered(vec![
        FutureObj::new(Box::new(a_rx)),
        //FutureObj::new(Box::new(b_rx.select(c_rx))),
    ]);

    support::with_noop_waker_context(f)(|lw| {
        for _ in 0..10 {
            assert!(stream.poll_next_unpin(lw).is_pending());
        }

        b_tx.send(12).unwrap();
        assert!(stream.poll_next_unpin(lw).is_ready());
        c_tx.send(3).unwrap();
        assert!(stream.poll_next_unpin(lw).is_pending());
        assert!(stream.poll_next_unpin(lw).is_pending());
    })
}*/

#[test]
fn iter_mut_cancel() {
    let (a_tx, a_rx) = oneshot::channel::<i32>();
    let (b_tx, b_rx) = oneshot::channel::<i32>();
    let (c_tx, c_rx) = oneshot::channel::<i32>();

    let mut stream = futures_unordered(vec![a_rx, b_rx, c_rx]);

    for rx in stream.iter_mut() {
        rx.close();
    }

    let mut iter = block_on_stream(stream);

    assert!(a_tx.is_canceled());
    assert!(b_tx.is_canceled());
    assert!(c_tx.is_canceled());

    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), Some(Err(futures::channel::oneshot::Canceled)));
    assert_eq!(iter.next(), None);
}

#[test]
fn iter_mut_len() {
    let mut stream = futures_unordered(vec![
        future::empty::<()>(),
        future::empty::<()>(),
        future::empty::<()>()
    ]);

    let mut iter_mut = stream.iter_mut();
    assert_eq!(iter_mut.len(), 3);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 2);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 1);
    assert!(iter_mut.next().is_some());
    assert_eq!(iter_mut.len(), 0);
    assert!(iter_mut.next().is_none());
}

#[test]
fn futures_not_moved_after_poll() {
    // Future that will be ready after being polled twice,
    // asserting that it does not move.
    let fut = future::ready(()).pending_once().assert_unmoved();
    let mut stream = futures_unordered(vec![fut; 3]);
    assert_stream_next!(stream, ());
    assert_stream_next!(stream, ());
    assert_stream_next!(stream, ());
    assert_stream_done!(stream);
}

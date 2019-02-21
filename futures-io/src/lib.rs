//! Asynchronous I/O
//!
//! This crate contains the `AsyncRead` and `AsyncWrite` traits, the
//! asynchronous analogs to `std::io::{Read, Write}`. The primary difference is
//! that these traits integrate with the asynchronous task system.

#![cfg_attr(not(feature = "std"), no_std)]

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

#![doc(html_root_url = "https://rust-lang-nursery.github.io/futures-api-docs/0.3.0-alpha.13/futures_io")]

#![feature(futures_api)]

#[cfg(feature = "std")]
mod if_std {
    use futures_core::task::{Waker, Poll};
    use std::boxed::Box;
    use std::cmp;
    use std::io as StdIo;
    use std::ptr;

    // Re-export IoVec for convenience
    pub use iovec::IoVec;

    // Re-export io::Error so that users don't have to deal
    // with conflicts when `use`ing `futures::io` and `std::io`.
    pub use self::StdIo::Error as Error;
    pub use self::StdIo::ErrorKind as ErrorKind;
    pub use self::StdIo::Result as Result;

    /// A type used to conditionally initialize buffers passed to `AsyncRead`
    /// methods, modeled after `std`.
    #[derive(Debug)]
    pub struct Initializer(bool);

    impl Initializer {
        /// Returns a new `Initializer` which will zero out buffers.
        #[inline]
        pub fn zeroing() -> Initializer {
            Initializer(true)
        }

        /// Returns a new `Initializer` which will not zero out buffers.
        ///
        /// # Safety
        ///
        /// This method may only be called by `AsyncRead`ers which guarantee
        /// that they will not read from the buffers passed to `AsyncRead`
        /// methods, and that the return value of the method accurately reflects
        /// the number of bytes that have been written to the head of the buffer.
        #[inline]
        pub unsafe fn nop() -> Initializer {
            Initializer(false)
        }

        /// Indicates if a buffer should be initialized.
        #[inline]
        pub fn should_initialize(&self) -> bool {
            self.0
        }

        /// Initializes a buffer if necessary.
        #[inline]
        pub fn initialize(&self, buf: &mut [u8]) {
            if self.should_initialize() {
                unsafe { ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len()) }
            }
        }
    }

    /// Read bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Read` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_read`
    /// method, unlike `Read::read`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncRead {
        /// Determines if this `AsyncRead`er can work with buffers of
        /// uninitialized memory.
        ///
        /// The default implementation returns an initializer which will zero
        /// buffers.
        ///
        /// # Safety
        ///
        /// This method is `unsafe` because and `AsyncRead`er could otherwise
        /// return a non-zeroing `Initializer` from another `AsyncRead` type
        /// without an `unsafe` block.
        #[inline]
        unsafe fn initializer(&self) -> Initializer {
            Initializer::zeroing()
        }

        /// Attempt to read from the `AsyncRead` into `buf`.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `waker.wake()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_read(&mut self, waker: &Waker, buf: &mut [u8])
            -> Poll<Result<usize>>;

        /// Attempt to read from the `AsyncRead` into `vec` using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_read`, but allows data to be read
        /// into multiple buffers using a single operation.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `waker.wake()`) to receive a notification when the object becomes
        /// readable or is closed.
        /// By default, this method delegates to using `poll_read` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_vectored_read(&mut self, waker: &Waker, vec: &mut [&mut IoVec])
            -> Poll<Result<usize>>
        {
            if let Some(ref mut first_iovec) = vec.get_mut(0) {
                self.poll_read(waker, first_iovec)
            } else {
                // `vec` is empty.
                Poll::Ready(Ok(0))
            }
        }
    }

    /// Write bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Write` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_write`
    /// method, unlike `Write::write`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncWrite {
        /// Attempt to write bytes from `buf` into the object.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `waker.wake()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_write(&mut self, waker: &Waker, buf: &[u8])
            -> Poll<Result<usize>>;

        /// Attempt to write bytes from `vec` into the object using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_write`, but allows data from multiple buffers to be written
        /// using a single operation.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `waker.wake()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// By default, this method delegates to using `poll_write` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_vectored_write(&mut self, waker: &Waker, vec: &[&IoVec])
            -> Poll<Result<usize>>
        {
            if let Some(ref first_iovec) = vec.get(0) {
                self.poll_write(waker, &*first_iovec)
            } else {
                // `vec` is empty.
                Poll::Ready(Ok(0))
            }
        }

        /// Attempt to flush the object, ensuring that any buffered data reach
        /// their destination.
        ///
        /// On success, returns `Ok(Async::Ready(()))`.
        ///
        /// If flushing cannot immediately complete, this method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `waker.wake()`) to receive a notification when the object can make
        /// progress towards flushing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_flush(&mut self, waker: &Waker) -> Poll<Result<()>>;

        /// Attempt to close the object.
        ///
        /// On success, returns `Ok(Async::Ready(()))`.
        ///
        /// If closing cannot immediately complete, this function returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `waker.wake()`) to receive a notification when the object can make
        /// progress towards closing.
        ///
        /// # Implementation
        ///
        /// This function may not return errors of kind `WouldBlock` or
        /// `Interrupted`.  Implementations must convert `WouldBlock` into
        /// `Async::Pending` and either internally retry or convert
        /// `Interrupted` into another error kind.
        fn poll_close(&mut self, waker: &Waker) -> Poll<Result<()>>;
    }

    macro_rules! deref_async_read {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                (**self).initializer()
            }

            fn poll_read(&mut self, waker: &Waker, buf: &mut [u8])
                -> Poll<Result<usize>>
            {
                (**self).poll_read(waker, buf)
            }

            fn poll_vectored_read(&mut self, waker: &Waker, vec: &mut [&mut IoVec])
                -> Poll<Result<usize>>
            {
                (**self).poll_vectored_read(waker, vec)
            }
        }
    }

    impl<T: ?Sized + AsyncRead> AsyncRead for Box<T> {
        deref_async_read!();
    }

    impl<'a, T: ?Sized + AsyncRead> AsyncRead for &'a mut T {
        deref_async_read!();
    }

    /// `unsafe` because the `StdIo::Read` type must not access the buffer
    /// before reading data into it.
    macro_rules! unsafe_delegate_async_read_to_stdio {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                Initializer::nop()
            }

            fn poll_read(&mut self, _: &Waker, buf: &mut [u8])
                -> Poll<Result<usize>>
            {
                Poll::Ready(StdIo::Read::read(self, buf))
            }
        }
    }

    impl<'a> AsyncRead for &'a [u8] {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl AsyncRead for StdIo::Repeat {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl<T: AsRef<[u8]>> AsyncRead for StdIo::Cursor<T> {
        unsafe_delegate_async_read_to_stdio!();
    }

    macro_rules! deref_async_write {
        () => {
            fn poll_write(&mut self, waker: &Waker, buf: &[u8])
                -> Poll<Result<usize>>
            {
                (**self).poll_write(waker, buf)
            }

            fn poll_vectored_write(&mut self, waker: &Waker, vec: &[&IoVec])
                -> Poll<Result<usize>>
            {
                (**self).poll_vectored_write(waker, vec)
            }

            fn poll_flush(&mut self, waker: &Waker) -> Poll<Result<()>> {
                (**self).poll_flush(waker)
            }

            fn poll_close(&mut self, waker: &Waker) -> Poll<Result<()>> {
                (**self).poll_close(waker)
            }
        }
    }

    impl<T: ?Sized + AsyncWrite> AsyncWrite for Box<T> {
        deref_async_write!();
    }

    impl<'a, T: ?Sized + AsyncWrite> AsyncWrite for &'a mut T {
        deref_async_write!();
    }

    macro_rules! delegate_async_write_to_stdio {
        () => {
            fn poll_write(&mut self, _: &Waker, buf: &[u8])
                -> Poll<Result<usize>>
            {
                Poll::Ready(StdIo::Write::write(self, buf))
            }

            fn poll_flush(&mut self, _: &Waker) -> Poll<Result<()>> {
                Poll::Ready(StdIo::Write::flush(self))
            }

            fn poll_close(&mut self, waker: &Waker) -> Poll<Result<()>> {
                self.poll_flush(waker)
            }
        }
    }

    impl<T: AsMut<[u8]>> AsyncWrite for StdIo::Cursor<T> {
        fn poll_write(
            &mut self,
            _: &Waker,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            let position = self.position();
            let result = {
                let out = self.get_mut().as_mut();
                let pos = cmp::min(out.len() as u64, position) as usize;
                StdIo::Write::write(&mut &mut out[pos..], buf)
            };
            if let Ok(offset) = result {
                self.set_position(position + offset as u64);
            }
            Poll::Ready(result)
        }

        fn poll_flush(&mut self, _: &Waker) -> Poll<Result<()>> {
            Poll::Ready(StdIo::Write::flush(&mut self.get_mut().as_mut()))
        }

        fn poll_close(&mut self, waker: &Waker) -> Poll<Result<()>> {
            self.poll_flush(waker)
        }
    }

    impl AsyncWrite for StdIo::Sink {
        delegate_async_write_to_stdio!();
    }
}

#[cfg(feature = "std")]
pub use self::if_std::*;

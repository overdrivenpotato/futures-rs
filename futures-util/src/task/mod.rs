//! Task notification

/// A macro for creating a `RawWaker` vtable for a type that implements
/// the `ArcWake` trait.
macro_rules! waker_vtable {
    ($ty:ident) => {
        &RawWakerVTable {
            clone: clone_arc_raw::<$ty>,
            drop: drop_arc_raw::<$ty>,
            wake: wake_arc_raw::<$ty>,
        }
    };
}

#[cfg(feature = "std")]
mod arc_wake;
#[cfg(feature = "std")]
pub use self::arc_wake::ArcWake;

mod noop_waker;
pub use self::noop_waker::noop_waker;
#[cfg(feature = "std")]
pub use self::noop_waker::noop_waker_ref;

mod spawn;
pub use self::spawn::{SpawnExt, LocalSpawnExt};

#[cfg(feature = "std")]
mod waker_ref;
#[cfg(feature = "std")]
pub use self::waker_ref::{waker_ref, WakerRef};

#[cfg_attr(
    feature = "cfg-target-has-atomic",
    cfg(all(target_has_atomic = "cas", target_has_atomic = "ptr"))
)]
pub use futures_core::task::__internal::AtomicWaker;

// re-export for `select!`
#[doc(hidden)]
pub use futures_core::task::{Waker, Poll};

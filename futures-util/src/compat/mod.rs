//! Futures 0.1 / 0.3 shims

#![allow(missing_debug_implementations)]

mod executor;
pub use self::executor::{Executor01CompatExt, Executor01Future, Executor01As03};

mod compat01as03;
pub use self::compat01as03::{Compat01As03, Future01CompatExt, Stream01CompatExt, Sink01CompatExt};

mod compat03as01;
pub use self::compat03as01::Compat;

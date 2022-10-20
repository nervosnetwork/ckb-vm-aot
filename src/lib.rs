#[cfg(has_aot)]
#[macro_use]
extern crate derive_more;

#[cfg(has_aot)]
mod emitter;
#[cfg(has_aot)]
mod error;
#[cfg(has_aot)]
mod label_gather;
#[cfg(has_aot)]
pub use error::AotError;
#[cfg(has_aot)]
pub use label_gather::{AotCode, AotCompilingMachine, AotMachine};

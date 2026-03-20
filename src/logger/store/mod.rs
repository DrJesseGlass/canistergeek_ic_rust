//! Logger storage module
//!
//! This module exposes exactly one storage implementation per build.

#[cfg(not(feature = "stable-memory"))]
mod base;
#[cfg(not(feature = "stable-memory"))]
mod generic;
#[cfg(feature = "stable-memory")]
mod stable;

#[cfg(not(feature = "stable-memory"))]
pub use generic::Storage;
#[cfg(feature = "stable-memory")]
pub use stable::{Memory, Storage};

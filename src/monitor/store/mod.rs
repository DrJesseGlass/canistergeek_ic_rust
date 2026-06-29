//! Monitor storage module
//!
//! This module exposes exactly one storage implementation per build:
//! - `generic::Storage` for default builds
//! - `stable::Storage` when the `stable-memory` feature is enabled

mod base;
#[cfg(not(feature = "stable-memory"))]
mod generic;

#[cfg(feature = "stable-memory")]
mod stable;

pub use crate::monitor::data_type::{DayData, DayDataReader, DayDataStorage};

// Re-export day_id type
pub use base::DayId;

#[cfg(not(feature = "stable-memory"))]
pub use generic::{DayDataTable, Storage};

#[cfg(feature = "stable-memory")]
pub use stable::{Memory, Storage};

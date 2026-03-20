//! Monitor data types

use candid::{CandidType, Deserialize};
use serde::Serialize;

// number of update calls in each time interval for a specific day.
pub type DayUpdateCallsCountData = Vec<u64>;

// canister heap memory size in each time interval for a specific day.
pub type DayCanisterHeapMemorySizeData = Vec<u64>;

// canister memory size in each time interval for a specific day.
pub type DayCanisterMemorySizeData = Vec<u64>;

// canister available cycles in each time interval for a specific day.
pub type DayCanisterCyclesData = Vec<u64>;

/// Specific day data with all necessary metrics
#[derive(Clone, Debug, CandidType, Deserialize, Serialize)]
pub struct DayData {
    update_calls_data: DayUpdateCallsCountData,
    canister_heap_memory_size_data: DayCanisterHeapMemorySizeData,
    canister_memory_size_data: DayCanisterMemorySizeData,
    canister_cycles_data: DayCanisterCyclesData,
}

impl DayData {
    pub fn new(cell_count: &usize) -> Self {
        Self {
            update_calls_data: create_empty_vector(cell_count),
            canister_heap_memory_size_data: create_empty_vector(cell_count),
            canister_memory_size_data: create_empty_vector(cell_count),
            canister_cycles_data: create_empty_vector(cell_count),
        }
    }

    #[cfg(feature = "stable-memory")]
    pub(crate) fn from_vecs(
        update_calls_data: Vec<u64>,
        canister_heap_memory_size_data: Vec<u64>,
        canister_memory_size_data: Vec<u64>,
        canister_cycles_data: Vec<u64>,
    ) -> Self {
        Self {
            update_calls_data,
            canister_heap_memory_size_data,
            canister_memory_size_data,
            canister_cycles_data,
        }
    }

    pub fn store(
        &mut self,
        cell: &usize,
        update_calls: u64,
        canister_heap_memory_size: u64,
        canister_memory_size: u64,
        canister_cycles: u64,
    ) {
        self.update_calls_data[*cell] = update_calls;
        self.set_canister_info(
            cell,
            canister_heap_memory_size,
            canister_memory_size,
            canister_cycles,
        );
    }

    pub fn increment_update_calls(&mut self, cell: &usize) {
        self.update_calls_data[*cell] += 1;
    }

    pub fn set_canister_info(
        &mut self,
        cell: &usize,
        canister_heap_memory_size: u64,
        canister_memory_size: u64,
        canister_cycles: u64,
    ) {
        self.canister_heap_memory_size_data[*cell] = canister_heap_memory_size;
        self.canister_memory_size_data[*cell] = canister_memory_size;
        self.canister_cycles_data[*cell] = canister_cycles;
    }

    pub fn get_update_calls_data(&self) -> &DayUpdateCallsCountData {
        &self.update_calls_data
    }

    pub fn get_canister_heap_memory_size_data(&self) -> &DayCanisterHeapMemorySizeData {
        &self.canister_heap_memory_size_data
    }

    pub fn get_canister_memory_size_data(&self) -> &DayCanisterMemorySizeData {
        &self.canister_memory_size_data
    }

    pub fn get_canister_cycles_data(&self) -> &DayCanisterCyclesData {
        &self.canister_cycles_data
    }
}

fn create_empty_vector(cell_count: &usize) -> Vec<u64> {
    vec![0_u64; *cell_count]
}

/// Trait for reading day data
pub trait DayDataReader {
    fn get_day_data(&self, year: &i32, month: &u32, day: &u32) -> Option<DayData>;
}

/// Trait for storing day data
pub trait DayDataStorage: DayDataReader {
    fn store_day_data(&mut self, year: &i32, month: &u32, day: &u32, day_data: DayData);
}

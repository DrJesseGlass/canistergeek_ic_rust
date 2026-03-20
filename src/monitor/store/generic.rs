//! Generic in-memory storage implementation for monitor data using BTreeMap

use std::collections::BTreeMap;

use crate::monitor::data_type::{DayData, DayDataReader, DayDataStorage};
use crate::monitor::store::base::{to_day_id, DayId};

pub type DayDataTable = BTreeMap<DayId, DayData>;

/// Generic in-memory storage using BTreeMap
#[derive(Default)]
pub struct Storage {
    day_data_table: DayDataTable,
}

impl Storage {
    /// Initialize storage with existing data (for deserialization)
    pub fn init(day_data_table: DayDataTable) -> Self {
        Self { day_data_table }
    }

    /// Get the day data table for serialization
    pub fn get_day_data_table(&self) -> &DayDataTable {
        &self.day_data_table
    }
}

impl DayDataReader for Storage {
    fn get_day_data(&self, year: &i32, month: &u32, day: &u32) -> Option<DayData> {
        let day_id = to_day_id(year, month, day).ok()?;
        self.day_data_table.get(&day_id).cloned()
    }
}

impl DayDataStorage for Storage {
    fn store_day_data(&mut self, year: &i32, month: &u32, day: &u32, day_data: DayData) {
        if let Ok(day_id) = to_day_id(year, month, day) {
            self.day_data_table.insert(day_id, day_data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_storage() {
        let mut storage = Storage::default();

        let time_nanos = Utc
            .with_ymd_and_hms(2022, 1, 28, 13, 0, 0)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap() as u64;
        let data = Utc.timestamp_nanos(time_nanos as i64);

        let day_data = DayData::new(&288);
        storage.store_day_data(&data.year(), &data.month(), &data.day(), day_data);

        let retrieved = storage.get_day_data(&data.year(), &data.month(), &data.day());
        assert!(retrieved.is_some());
    }
}

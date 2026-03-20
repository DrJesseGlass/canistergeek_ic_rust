use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, Storable};
use std::borrow::Cow;

use crate::monitor::data_type::{DayData, DayDataReader, DayDataStorage};
use crate::monitor::store::base::{to_day_id, DayId};

const MAX_DAY_DATA_SIZE: u32 = 32_768;

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

pub struct Storage {
    map: StableBTreeMap<DayId, DayData, Memory>,
}

// Binary format: [4 bytes LE u32: cell_count]
//   [cell_count × 8 bytes LE u64: update_calls]
//   [cell_count × 8 bytes LE u64: heap_memory_size]
//   [cell_count × 8 bytes LE u64: memory_size]
//   [cell_count × 8 bytes LE u64: cycles]
impl Storable for DayData {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let count = self.get_update_calls_data().len() as u32;
        let mut buf = Vec::with_capacity(4 + (count as usize) * 4 * 8);
        buf.extend_from_slice(&count.to_le_bytes());
        for v in [
            self.get_update_calls_data(),
            self.get_canister_heap_memory_size_data(),
            self.get_canister_memory_size_data(),
            self.get_canister_cycles_data(),
        ] {
            for &val in v {
                buf.extend_from_slice(&val.to_le_bytes());
            }
        }
        Cow::Owned(buf)
    }

    fn into_bytes(self) -> Vec<u8> {
        self.to_bytes().into_owned()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let mut offset = 4;
        let mut read_vec = || -> Vec<u64> {
            let mut v = Vec::with_capacity(count);
            for _ in 0..count {
                v.push(u64::from_le_bytes(
                    bytes[offset..offset + 8].try_into().unwrap(),
                ));
                offset += 8;
            }
            v
        };
        DayData::from_vecs(read_vec(), read_vec(), read_vec(), read_vec())
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_DAY_DATA_SIZE,
        is_fixed_size: false,
    };
}

impl Storage {
    pub fn new(memory: Memory) -> Self {
        let map = StableBTreeMap::init(memory);
        Self { map }
    }
}

impl DayDataReader for Storage {
    fn get_day_data(&self, year: &i32, month: &u32, day: &u32) -> Option<DayData> {
        let day_id = to_day_id(year, month, day).ok()?;
        self.map.get(&day_id)
    }
}

impl DayDataStorage for Storage {
    fn store_day_data(&mut self, year: &i32, month: &u32, day: &u32, day_data: DayData) {
        if let Ok(day_id) = to_day_id(year, month, day) {
            self.map.insert(day_id, day_data);
        }
    }
}

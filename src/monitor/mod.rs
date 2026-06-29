pub mod calculator;
pub mod collector;
pub mod data_type;
pub mod store;

use std::cell::RefCell;

use super::api_type::{CanisterMetrics, GetMetricsParameters};
use super::ic_util;
use crate::api_type::{StatusRequest, StatusResponse};
use collector::CanisterInfo;

#[cfg(not(feature = "stable-memory"))]
pub type PreUpgradeStableData = (u8, store::DayDataTable);
#[cfg(not(feature = "stable-memory"))]
pub type PostUpgradeStableData = (u8, store::DayDataTable);

#[cfg(not(feature = "stable-memory"))]
const VERSION: u8 = 1;

thread_local! {
    static STORAGE: RefCell<Option<store::Storage>> = const { RefCell::new(None) };
}

#[cfg(feature = "stable-memory")]
pub fn init_stable_storage(memory: store::Memory) {
    STORAGE.with(|storage| {
        *storage.borrow_mut() = Some(store::Storage::new(memory));
    });
}

fn with_storage<R>(f: impl FnOnce(&mut store::Storage) -> R) -> Option<R> {
    STORAGE.with(|storage| {
        let mut storage = storage.borrow_mut();
        #[cfg(not(feature = "stable-memory"))]
        if storage.is_none() {
            *storage = Some(store::Storage::default());
        }

        match storage.as_mut() {
            Some(s) => Some(f(s)),
            None => {
                #[cfg(not(test))]
                ic_cdk::api::debug_print(
                    "WARNING: monitor stable storage is not initialized - metrics collection skipped",
                );
                #[cfg(test)]
                eprintln!("WARNING: monitor stable storage is not initialized - metrics collection skipped");
                None
            }
        }
    })
}

#[cfg(not(feature = "stable-memory"))]
pub fn pre_upgrade_stable_data() -> PreUpgradeStableData {
    STORAGE.with(|storage| {
        let mut storage = storage.borrow_mut();
        if storage.is_none() {
            *storage = Some(store::Storage::default());
        }

        (
            VERSION,
            storage
                .as_ref()
                .expect("monitor storage must be initialized")
                .get_day_data_table()
                .clone(),
        )
    })
}

#[cfg(not(feature = "stable-memory"))]
pub fn post_upgrade_stable_data((version, upgrade_data): PostUpgradeStableData) {
    if version != VERSION {
        ic_cdk::api::debug_print(std::format!(
            "Can not upgrade stable data. Unsupported version {}",
            version
        ));
    } else {
        STORAGE.with(|storage| {
            *storage.borrow_mut() = Some(store::Storage::init(upgrade_data));
        });
    }
}

pub fn collect_metrics() {
    collect_metrics_int(false);
}

pub fn get_metrics(parameters: &GetMetricsParameters) -> Option<CanisterMetrics> {
    with_storage(|s| match calculator::get_canister_metrics(parameters, s) {
        Ok(data) => Some(CanisterMetrics { data }),
        Err(_) => None,
    })
    .flatten()
}

pub(crate) fn collect_metrics_int(force_set_info: bool) {
    with_storage(|s| {
        collector::collect_canister_metrics(
            s,
            ic_util::get_ic_time_nanos(),
            force_set_info,
            || CanisterInfo {
                heap_memory_size: get_current_heap_memory_size(),
                memory_size: get_current_memory_size(),
                cycles: get_current_cycles(),
            },
        );
    });
}

pub(crate) fn get_status(request: StatusRequest) -> StatusResponse {
    let cycles = obtain_value(request.cycles, get_current_cycles);
    let memory_size = obtain_value(request.memory_size, get_current_memory_size);
    let heap_memory_size = obtain_value(request.heap_memory_size, get_current_heap_memory_size);

    StatusResponse {
        cycles,
        memory_size,
        heap_memory_size,
    }
}

fn obtain_value<T, F>(need: bool, supplier: F) -> Option<T>
where
    F: Fn() -> T,
{
    if need {
        Some(supplier())
    } else {
        None
    }
}

fn get_current_cycles() -> u64 {
    ic_util::get_cycles()
}

fn get_current_memory_size() -> u64 {
    ic_util::get_stable_memory_size() + ic_util::get_heap_memory_size()
}

fn get_current_heap_memory_size() -> u64 {
    ic_util::get_heap_memory_size()
}

#[cfg(test)]
mod tests {
    use super::calculator;
    use super::collector;
    use crate::api_type::CanisterMetricsData;
    use candid::Nat;
    use chrono::prelude::*;

    #[cfg(feature = "stable-memory")]
    use ic_stable_structures::memory_manager::{MemoryId, MemoryManager};
    #[cfg(feature = "stable-memory")]
    use ic_stable_structures::DefaultMemoryImpl;

    #[test]
    fn test_metrics() {
        #[cfg(not(feature = "stable-memory"))]
        let mut storage = super::store::Storage::default();
        #[cfg(feature = "stable-memory")]
        let mut storage = {
            let memory_manager = MemoryManager::init(DefaultMemoryImpl::default());
            super::store::Storage::new(memory_manager.get(MemoryId::new(0)))
        };

        let time_nanos = Utc
            .with_ymd_and_hms(2022, 01, 28, 13, 0, 0)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap() as u64;

        collector::collect_canister_metrics(&mut storage, time_nanos, false, || {
            let heap_memory_size = 234000;
            let memory_size = 345000;
            let cycles = 8787;
            collector::CanisterInfo {
                heap_memory_size,
                memory_size,
                cycles,
            }
        });

        let time_nanos = Utc
            .with_ymd_and_hms(2022, 01, 28, 9, 0, 0)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap() as u64;

        collector::collect_canister_metrics(&mut storage, time_nanos, false, || {
            let heap_memory_size = 1234000;
            let memory_size = 1345000;
            let cycles = 18787;
            collector::CanisterInfo {
                heap_memory_size,
                memory_size,
                cycles,
            }
        });

        let params = crate::api_type::GetMetricsParameters {
            granularity: crate::api_type::MetricsGranularity::hourly,
            dateFromMillis: Nat::from(
                Utc.with_ymd_and_hms(2022, 01, 28, 11, 11, 11)
                    .unwrap()
                    .timestamp_millis() as u64,
            ),
            dateToMillis: Nat::from(
                Utc.with_ymd_and_hms(2022, 01, 28, 11, 11, 11)
                    .unwrap()
                    .timestamp_millis() as u64,
            ),
        };

        let result = calculator::get_canister_metrics(&params, &storage);
        dbg!(&result);

        let vector = match result.unwrap() {
            CanisterMetricsData::hourly(vector) => vector,
            _ => panic!(),
        };

        assert_eq!(vector.len(), 1);
        let hourly_data = vector.get(0).unwrap();
        assert_eq!(hourly_data.timeMillis, candid::Int::from(1643328000000_i64));

        let cell_count = 288;
        let cell_9_hour = 9 * 3600 / 300;
        let cell_13_hour = 13 * 3600 / 300;

        assert_eq!(hourly_data.canisterCycles.len(), cell_count);
        assert_eq!(
            hourly_data.canisterCycles.get(cell_9_hour).unwrap(),
            &18787_u64
        );
        assert_eq!(
            hourly_data.canisterCycles.get(cell_13_hour).unwrap(),
            &8787_u64
        );

        assert_eq!(hourly_data.canisterHeapMemorySize.len(), cell_count);
        assert_eq!(
            hourly_data.canisterHeapMemorySize.get(cell_9_hour).unwrap(),
            &1234000_u64
        );
        assert_eq!(
            hourly_data
                .canisterHeapMemorySize
                .get(cell_13_hour)
                .unwrap(),
            &234000_u64
        );

        assert_eq!(hourly_data.canisterMemorySize.len(), cell_count);
        assert_eq!(
            hourly_data.canisterMemorySize.get(cell_9_hour).unwrap(),
            &1345000_u64
        );
        assert_eq!(
            hourly_data.canisterMemorySize.get(cell_13_hour).unwrap(),
            &345000_u64
        );

        assert_eq!(hourly_data.updateCalls.len(), cell_count);
        assert_eq!(hourly_data.updateCalls.get(cell_9_hour).unwrap(), &1_u64);
        assert_eq!(hourly_data.updateCalls.get(cell_13_hour).unwrap(), &1_u64);

        for i in 0..cell_count {
            if i != cell_9_hour && i != cell_13_hour {
                assert_eq!(hourly_data.canisterCycles.get(i).unwrap(), &0_u64);
                assert_eq!(hourly_data.canisterHeapMemorySize.get(i).unwrap(), &0_u64);
                assert_eq!(hourly_data.canisterMemorySize.get(i).unwrap(), &0_u64);
                assert_eq!(hourly_data.updateCalls.get(i).unwrap(), &0_u64);
            }
        }
    }

    #[test]
    #[cfg(feature = "stable-memory")]
    fn test_collect_metrics_without_init_does_not_panic() {
        // Test that calling collect_metrics before init_stable_storage()
        // does not panic and just logs a warning
        super::collect_metrics();
        // If we reach here, no panic occurred
    }
}

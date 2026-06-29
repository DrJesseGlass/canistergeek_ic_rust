mod calculator;
mod collector;
mod data_type;
mod store;

use std::cell::RefCell;

use super::api_type::{
    CanisterLogFeature, CanisterLogMessagesInfo, CanisterLogRequest, CanisterLogResponse,
};
use data_type::LogMessagesStorage;

pub type LogMessageStorage = store::Storage;

#[cfg(not(feature = "stable-memory"))]
pub type PreUpgradeStableData = (u8, LogMessageStorage);
#[cfg(not(feature = "stable-memory"))]
pub type PostUpgradeStableData = (u8, LogMessageStorage);

#[cfg(not(feature = "stable-memory"))]
const VERSION: u8 = 1;

#[allow(dead_code)]
const DEFAULT_MAX_LOG_MESSAGES_COUNT: usize = 10_000;
const DEFAULT_MAX_LOG_MESSAGE_LENGTH: usize = 4096;

thread_local! {
    static STORAGE: RefCell<Option<LogMessageStorage>> = const { RefCell::new(None) };
}

#[cfg(not(feature = "stable-memory"))]
fn create_storage() -> LogMessageStorage {
    LogMessageStorage::new(DEFAULT_MAX_LOG_MESSAGES_COUNT)
}

#[cfg(feature = "stable-memory")]
pub fn init_stable_storage(
    messages_memory: store::Memory,
    metadata_memory: store::Memory,
    max_messages_count: usize,
) {
    STORAGE.with(|storage| {
        *storage.borrow_mut() = Some(LogMessageStorage::restore(
            messages_memory,
            metadata_memory,
            max_messages_count,
        ));
    });
}

fn with_storage<R>(f: impl FnOnce(&mut LogMessageStorage) -> R) -> Option<R> {
    STORAGE.with(|storage| {
        let mut storage = storage.borrow_mut();
        #[cfg(not(feature = "stable-memory"))]
        if storage.is_none() {
            *storage = Some(create_storage());
        }
        match storage.as_mut() {
            Some(s) => Some(f(s)),
            None => {
                #[cfg(not(test))]
                ic_cdk::api::debug_print(
                    "WARNING: logger stable storage is not initialized - logging skipped",
                );
                #[cfg(test)]
                eprintln!("WARNING: logger stable storage is not initialized - logging skipped");
                None
            }
        }
    })
}

// API

#[cfg(not(feature = "stable-memory"))]
pub fn pre_upgrade_stable_data() -> PreUpgradeStableData {
    STORAGE.with(|storage| {
        let mut storage = storage.borrow_mut();
        if storage.is_none() {
            *storage = Some(create_storage());
        }

        (
            VERSION,
            storage
                .as_ref()
                .expect("logger storage must be initialized")
                .clone(),
        )
    })
}

#[cfg(not(feature = "stable-memory"))]
pub fn post_upgrade_stable_data(data: PostUpgradeStableData) {
    match data {
        (VERSION, log_message_storage) => {
            STORAGE.with(|storage| {
                *storage.borrow_mut() = Some(log_message_storage);
            });
        }
        _ => {
            ic_cdk::api::debug_print(std::format!(
                "Can not upgrade stable log messages data. Unsupported version {}",
                data.0
            ));
        }
    }
}

pub fn set_max_messages_count(limit: u32) {
    assert!(limit > 0);
    with_storage(|s| s.set_max_messages_count(limit as usize));
}

pub fn log_message(message: String) {
    with_storage(|s| collector::store_log_message(s, message, &DEFAULT_MAX_LOG_MESSAGE_LENGTH));
}

pub fn get_canister_log(request: Option<CanisterLogRequest>) -> Option<CanisterLogResponse> {
    with_storage(|storage| {
        match request {
            Some(CanisterLogRequest::getMessagesInfo) => {
                let info = calculator::get_log_messages_info(storage);
                let features = vec![
                    Some(CanisterLogFeature::filterMessageByContains),
                    // Some(CanisterLogFeature::filterMessageByRegex),
                ];

                Some(CanisterLogResponse::messagesInfo(CanisterLogMessagesInfo {
                    features,
                    ..info
                }))
            }
            Some(CanisterLogRequest::getMessages(parameters)) => {
                match calculator::get_log_messages(storage, parameters) {
                    Err(_) => None,
                    Ok(messages) => Some(CanisterLogResponse::messages(messages)),
                }
            }
            Some(CanisterLogRequest::getLatestMessages(parameters)) => {
                match calculator::get_latest_log_messages(storage, parameters) {
                    Err(_) => None,
                    Ok(messages) => Some(CanisterLogResponse::messages(messages)),
                }
            }
            None => None,
        }
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::super::api_type::{
        GetLatestLogMessagesParameters, GetLogMessagesFilter, GetLogMessagesParameters,
    };
    use super::super::logger::calculator;
    use super::super::logger::collector;
    use super::super::logger::data_type::LogMessagesInfo;
    use super::super::logger::store::Storage;
    #[cfg(feature = "stable-memory")]
    use ic_stable_structures::memory_manager::{MemoryId, MemoryManager};
    #[cfg(feature = "stable-memory")]
    use ic_stable_structures::DefaultMemoryImpl;

    #[cfg(not(feature = "stable-memory"))]
    fn make_storage(limit: usize) -> Storage {
        Storage::new(limit)
    }

    #[cfg(feature = "stable-memory")]
    fn make_storage(limit: usize) -> Storage {
        let memory_manager = MemoryManager::init(DefaultMemoryImpl::default());
        Storage::new(
            memory_manager.get(MemoryId::new(10)),
            memory_manager.get(MemoryId::new(11)),
            limit,
        )
    }

    #[test]
    fn test_empty_log_messages() {
        let storage = make_storage(4);

        let params = GetLatestLogMessagesParameters {
            count: 10,
            filter: None,
            upToTimeNanos: None,
        };

        let result = calculator::get_latest_log_messages(&storage, params);
        let messages = result.expect("must zero elements");
        assert_eq!(messages.data.len(), 0);
    }

    #[test]
    fn test_chunk_log_messages() {
        let mut storage = make_storage(4);

        collector::store_log_message(&mut storage, String::from("1 message"), &10);
        collector::store_log_message(&mut storage, String::from("2 message"), &10);
        collector::store_log_message(&mut storage, String::from("3 message"), &10);
        collector::store_log_message(&mut storage, String::from("4 message"), &3);

        let params = GetLatestLogMessagesParameters {
            count: 10,
            filter: None,
            upToTimeNanos: None,
        };

        let result = calculator::get_latest_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 4);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            messages.get(3).unwrap().timeNanos
        );

        assert_eq!(messages.get(0).unwrap().message, "4 m");
        assert_eq!(messages.get(1).unwrap().message, "3 message");
        assert_eq!(messages.get(2).unwrap().message, "2 message");
        assert_eq!(messages.get(3).unwrap().message, "1 message");

        let params = GetLatestLogMessagesParameters {
            count: 2,
            filter: None,
            upToTimeNanos: None,
        };

        let result = calculator::get_latest_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 2);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            messages.get(1).unwrap().timeNanos
        );

        assert_eq!(messages.get(0).unwrap().message, "4 m");
        assert_eq!(messages.get(1).unwrap().message, "3 message");

        let params = GetLatestLogMessagesParameters {
            count: 1,
            filter: None,
            upToTimeNanos: Some(messages.get(1).unwrap().timeNanos),
        };

        let result = calculator::get_latest_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 1);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            messages.get(0).unwrap().timeNanos
        );

        assert_eq!(messages.get(0).unwrap().message, "2 message");
    }

    // #[test]
    // fn test_filter_log_messages_by_regex() {
    //     let mut storage = make_storage(4);
    //
    //     collector::store_log_message(&mut storage, String::from("message 1"), &1024);
    //     collector::store_log_message(&mut storage, String::from("сообщение abc 2 "), &1024);
    //     collector::store_log_message(&mut storage, String::from("message abc 3"), &1024);
    //     collector::store_log_message(&mut storage, String::from("message 4"), &10);
    //
    //     let params = GetLogMessagesParameters {
    //         count: 4,
    //         filter: None,
    //         fromTimeNanos: None,
    //     };
    //
    //     let result = calculator::get_log_messages(&storage, params).unwrap();
    //     let messages = result.data;
    //     assert_eq!(messages.len(), 4);
    //     assert_eq!(
    //         result.lastAnalyzedMessageTimeNanos.unwrap(),
    //         messages.get(3).unwrap().timeNanos
    //     );
    //
    //     let message1 = messages.get(0).unwrap();
    //     let message2 = messages.get(1).unwrap();
    //     let message3 = messages.get(2).unwrap();
    //     let message4 = messages.get(3).unwrap();
    //     assert_eq!(message1.message, "message 1");
    //     assert_eq!(message2.message, "сообщение abc 2 ");
    //     assert_eq!(message3.message, "message abc 3");
    //     assert_eq!(message4.message, "message 4");
    //
    //     let params = GetLatestLogMessagesParameters {
    //         count: 1,
    //         filter: Some(GetLogMessagesFilter {
    //             messageRegex: Some(String::from("abc")),
    //             messageContains: None,
    //             analyzeCount: 10,
    //         }),
    //         upToTimeNanos: None,
    //     };
    //
    //     let result = calculator::get_latest_log_messages(&storage, params).unwrap();
    //     let messages = result.data;
    //     assert_eq!(messages.len(), 1);
    //     assert_eq!(
    //         result.lastAnalyzedMessageTimeNanos.unwrap(),
    //         messages.get(0).unwrap().timeNanos
    //     );
    //
    //     assert_eq!(messages.get(0).unwrap().message, "message abc 3");
    //
    //     let params = GetLatestLogMessagesParameters {
    //         count: 10,
    //         filter: Some(GetLogMessagesFilter {
    //             messageRegex: Some(String::from("abc")),
    //             messageContains: None,
    //             analyzeCount: 10,
    //         }),
    //         upToTimeNanos: None,
    //     };
    //
    //     let result = calculator::get_latest_log_messages(&storage, params).unwrap();
    //     let messages = result.data;
    //     assert_eq!(messages.len(), 2);
    //     assert_eq!(
    //         result.lastAnalyzedMessageTimeNanos.unwrap(),
    //         message1.timeNanos
    //     );
    //
    //     assert_eq!(messages.get(0).unwrap().message, "message abc 3");
    //     assert_eq!(messages.get(1).unwrap().message, "сообщение abc 2 ");
    //
    //     let params = GetLatestLogMessagesParameters {
    //         count: 10,
    //         filter: Some(GetLogMessagesFilter {
    //             messageRegex: Some(String::from("abc")),
    //             messageContains: None,
    //             analyzeCount: 10,
    //         }),
    //         upToTimeNanos: Some(messages.get(0).unwrap().timeNanos),
    //     };
    //
    //     let result = calculator::get_latest_log_messages(&storage, params).unwrap();
    //     let messages = result.data;
    //     assert_eq!(messages.len(), 1);
    //     assert_eq!(
    //         result.lastAnalyzedMessageTimeNanos.unwrap(),
    //         message1.timeNanos
    //     );
    //
    //     assert_eq!(messages.get(0).unwrap().message, "сообщение abc 2 ");
    //
    //     let params = GetLatestLogMessagesParameters {
    //         count: 10,
    //         filter: Some(GetLogMessagesFilter {
    //             messageRegex: Some(String::from("mess.*")),
    //             messageContains: Some(String::from("abC")),
    //             analyzeCount: 3,
    //         }),
    //         upToTimeNanos: None,
    //     };
    //
    //     let result = calculator::get_latest_log_messages(&storage, params).unwrap();
    //     let messages = result.data;
    //     assert_eq!(messages.len(), 2);
    //     assert_eq!(
    //         result.lastAnalyzedMessageTimeNanos.unwrap(),
    //         message2.timeNanos
    //     );
    //
    //     assert_eq!(messages.get(0).unwrap().message, "message 4");
    //     assert_eq!(messages.get(1).unwrap().message, "message abc 3");
    //
    //     let params = GetLatestLogMessagesParameters {
    //         count: 10,
    //         filter: Some(GetLogMessagesFilter {
    //             messageRegex: None,
    //             messageContains: None,
    //             analyzeCount: 3,
    //         }),
    //         upToTimeNanos: None,
    //     };
    //
    //     let result = calculator::get_latest_log_messages(&storage, params);
    //     assert_eq!(result.is_err(), true);
    // }

    #[test]
    fn test_filter_log_messages_by_contains() {
        let mut storage = make_storage(4);

        collector::store_log_message(&mut storage, String::from("meSSage 1"), &1024);
        collector::store_log_message(&mut storage, String::from("сообщение Abc 2 "), &1024);
        collector::store_log_message(&mut storage, String::from("MEssage aBc 3"), &1024);
        collector::store_log_message(&mut storage, String::from("messaGE 4"), &10);

        let params = GetLogMessagesParameters {
            count: 4,
            filter: None,
            fromTimeNanos: None,
        };

        let result = calculator::get_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 4);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            messages.get(3).unwrap().timeNanos
        );

        let message1 = messages.get(0).unwrap();
        let message2 = messages.get(1).unwrap();
        let message3 = messages.get(2).unwrap();
        let message4 = messages.get(3).unwrap();
        assert_eq!(message1.message, "meSSage 1");
        assert_eq!(message2.message, "сообщение Abc 2 ");
        assert_eq!(message3.message, "MEssage aBc 3");
        assert_eq!(message4.message, "messaGE 4");

        let params = GetLatestLogMessagesParameters {
            count: 1,
            filter: Some(GetLogMessagesFilter {
                messageRegex: None,
                messageContains: Some(String::from("abC")),
                analyzeCount: 10,
            }),
            upToTimeNanos: None,
        };

        let result = calculator::get_latest_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 1);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            messages.get(0).unwrap().timeNanos
        );

        assert_eq!(messages.get(0).unwrap().message, message3.message);

        let params = GetLatestLogMessagesParameters {
            count: 10,
            filter: Some(GetLogMessagesFilter {
                messageRegex: None,
                messageContains: Some(String::from("abC")),
                analyzeCount: 10,
            }),
            upToTimeNanos: None,
        };

        let result = calculator::get_latest_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 2);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            message1.timeNanos
        );

        assert_eq!(messages.get(0).unwrap().message, message3.message);
        assert_eq!(messages.get(1).unwrap().message, message2.message);

        let params = GetLatestLogMessagesParameters {
            count: 10,
            filter: Some(GetLogMessagesFilter {
                messageRegex: None,
                messageContains: Some(String::from("abC")),
                analyzeCount: 10,
            }),
            upToTimeNanos: Some(messages.get(0).unwrap().timeNanos),
        };

        let result = calculator::get_latest_log_messages(&storage, params).unwrap();
        let messages = result.data;
        assert_eq!(messages.len(), 1);
        assert_eq!(
            result.lastAnalyzedMessageTimeNanos.unwrap(),
            message1.timeNanos
        );

        assert_eq!(messages.get(0).unwrap().message, message2.message);
    }

    #[test]
    fn test_log_messages_info() {
        let mut storage = make_storage(4);
        assert_eq!(storage.get_log_messages_count(), 0);

        collector::store_log_message(&mut storage, String::from("message 1"), &1024);
        assert_eq!(storage.get_log_messages_count(), 1);

        collector::store_log_message(&mut storage, String::from("сообщение abc 2 "), &1024);
        collector::store_log_message(&mut storage, String::from("message abc 3"), &1024);
        collector::store_log_message(&mut storage, String::from("message 4"), &10);
        collector::store_log_message(&mut storage, String::from("message 5"), &10);
        assert_eq!(storage.get_log_messages_count(), 4);
    }

    #[test]
    #[cfg(feature = "stable-memory")]
    fn test_log_without_init_does_not_panic() {
        // Test that calling log_message before init_stable_storage()
        // does not panic and just logs a warning
        super::log_message(String::from("test message"));
        // If we reach here, no panic occurred
    }
}

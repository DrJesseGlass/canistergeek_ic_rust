use candid::CandidType;
use serde::{Deserialize, Serialize};

use crate::logger::data_type::{
    LogMessage, LogMessagesInfo, LogMessagesStorage, LogMessagesSupplier,
};
use crate::logger::store::base::{get_count, get_first_index, LogMessageRefIterator};

/// Generic in-memory storage implementation using Vec
#[derive(Clone, Debug, CandidType, Deserialize, Serialize)]
pub struct Storage {
    queue: Vec<LogMessage>,
    max_count: usize,
    next: usize,
    full: bool,
}

impl Storage {
    /// Create a new storage with the specified maximum message count
    pub fn new(max_messages_count: usize) -> Self {
        Self {
            queue: Vec::new(),
            max_count: max_messages_count,
            next: 0,
            full: false,
        }
    }

    /// Initialize storage with existing data (for deserialization)
    pub fn init(queue: Vec<LogMessage>, max_count: usize, next: usize, full: bool) -> Self {
        Self {
            queue,
            max_count,
            next,
            full,
        }
    }

    /// Get the internal queue for serialization
    pub fn get_queue(&self) -> &Vec<LogMessage> {
        &self.queue
    }
}

impl LogMessagesInfo for Storage {
    fn get_log_messages_count(&self) -> u32 {
        self.queue.len() as u32
    }

    fn get_first_log_message_time(&self) -> Option<u64> {
        if self.queue.is_empty() {
            None
        } else {
            let index = get_first_index(self.full, self.next);
            Some(self.queue[index].timeNanos)
        }
    }

    fn get_last_log_message_time(&self) -> Option<u64> {
        if self.queue.is_empty() {
            None
        } else {
            let count = get_count(self.full, self.next, self.max_count);
            let first_index = get_first_index(self.full, self.next);
            let last_index = (first_index + count - 1) % self.max_count;
            Some(self.queue[last_index].timeNanos)
        }
    }
}

impl LogMessagesStorage for Storage {
    fn store_log_message(&mut self, log_message: LogMessage) {
        if self.full {
            self.queue[self.next] = log_message;
        } else {
            self.queue.push(log_message);
        }

        self.next += 1;

        if self.next == self.max_count {
            self.full = true;
            self.next = 0;
        }
    }

    fn set_max_messages_count(&mut self, new_max_messages_count: usize) {
        if self.max_count == new_max_messages_count {
            return;
        }

        let mut new_storage = Storage::new(new_max_messages_count);

        let count = get_count(self.full, self.next, self.max_count);
        let first_index = get_first_index(self.full, self.next);

        let range = if new_max_messages_count >= count {
            first_index..(first_index + count)
        } else {
            let start = first_index + count - new_max_messages_count;
            start..(start + new_max_messages_count)
        };

        for i in range {
            new_storage.store_log_message(self.queue.get(i % self.max_count).unwrap().clone());
        }

        self.queue = new_storage.queue;
        self.max_count = new_storage.max_count;
        self.next = new_storage.next;
        self.full = new_storage.full;
    }
}

impl LogMessagesSupplier for Storage {
    fn get_log_messages(
        &self,
        from_time_nanos: &Option<u64>,
    ) -> Box<dyn Iterator<Item = LogMessage> + '_> {
        Box::new(
            LogMessageRefIterator::create(&self.queue, from_time_nanos, self.next, self.full)
                .cloned(),
        )
    }

    fn get_log_messages_reverse(
        &self,
        up_to_time_nanos: &Option<u64>,
    ) -> Box<dyn Iterator<Item = LogMessage> + '_> {
        Box::new(
            LogMessageRefIterator::create_reverse(
                &self.queue,
                up_to_time_nanos,
                self.next,
                self.full,
            )
            .cloned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let storage = Storage::new(4);

        let mut iterator_box = storage.get_log_messages(&None);
        let iterator = iterator_box.as_mut();
        assert_eq!(iterator.next().is_none(), true);

        let mut iterator_box = storage.get_log_messages_reverse(&None);
        let iterator = iterator_box.as_mut();
        assert_eq!(iterator.next().is_none(), true);
    }

    #[test]
    fn test_cyclic() {
        let mut storage = Storage::new(4);

        storage.store_log_message(LogMessage {
            timeNanos: 10,
            message: String::from("time 10"),
        });

        let iterator_box = storage.get_log_messages(&None);
        let messages: Vec<_> = iterator_box.collect();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].timeNanos, 10);

        storage.store_log_message(LogMessage {
            timeNanos: 20,
            message: String::from("time 20"),
        });
        storage.store_log_message(LogMessage {
            timeNanos: 30,
            message: String::from("time 30"),
        });

        let iterator_box = storage.get_log_messages(&None);
        let messages: Vec<_> = iterator_box.collect();
        assert_eq!(messages.len(), 3);

        storage.store_log_message(LogMessage {
            timeNanos: 40,
            message: String::from("time 40"),
        });
        storage.store_log_message(LogMessage {
            timeNanos: 50,
            message: String::from("time 50"),
        });

        let iterator_box = storage.get_log_messages(&None);
        let messages: Vec<_> = iterator_box.collect();
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].message, "time 20");
        assert_eq!(messages[3].message, "time 50");
    }

    #[test]
    fn test_info() {
        let mut storage = Storage::new(2);
        assert_eq!(storage.get_log_messages_count(), 0);
        assert_eq!(storage.get_first_log_message_time().is_none(), true);
        assert_eq!(storage.get_last_log_message_time().is_none(), true);

        storage.store_log_message(LogMessage {
            timeNanos: 10,
            message: String::from("time 10"),
        });
        assert_eq!(storage.get_log_messages_count(), 1);
        assert_eq!(storage.get_first_log_message_time().unwrap(), 10_u64);
        assert_eq!(storage.get_last_log_message_time().unwrap(), 10_u64);

        storage.store_log_message(LogMessage {
            timeNanos: 20,
            message: String::from("time 20"),
        });
        assert_eq!(storage.get_log_messages_count(), 2);
        assert_eq!(storage.get_first_log_message_time().unwrap(), 10_u64);
        assert_eq!(storage.get_last_log_message_time().unwrap(), 20_u64);

        storage.store_log_message(LogMessage {
            timeNanos: 30,
            message: String::from("time 30"),
        });
        assert_eq!(storage.get_log_messages_count(), 2);
        assert_eq!(storage.get_first_log_message_time().unwrap(), 20_u64);
        assert_eq!(storage.get_last_log_message_time().unwrap(), 30_u64);
    }
}

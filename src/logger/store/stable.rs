use ic_stable_structures::memory_manager::VirtualMemory;
use ic_stable_structures::storable::Bound;
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, StableCell, Storable};
use std::borrow::Cow;

use crate::logger::data_type::{
    LogMessage, LogMessagesInfo, LogMessagesStorage, LogMessagesSupplier,
};

/// Maximum encoded size of a single log message: 8 bytes timestamp + up to 16376 bytes UTF-8 text.
const LOG_MESSAGE_MAX_SIZE: u32 = 16_384;

pub type Memory = VirtualMemory<DefaultMemoryImpl>;

// -- Binary Storable for LogMessage: [8 bytes LE u64 timeNanos][UTF-8 message bytes] --

impl Storable for LogMessage {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let msg = self.message.as_bytes();
        let mut buf = Vec::with_capacity(8 + msg.len());
        buf.extend_from_slice(&self.timeNanos.to_le_bytes());
        buf.extend_from_slice(msg);
        Cow::Owned(buf)
    }

    fn into_bytes(self) -> Vec<u8> {
        let msg = self.message.into_bytes();
        let mut buf = Vec::with_capacity(8 + msg.len());
        buf.extend_from_slice(&self.timeNanos.to_le_bytes());
        buf.extend_from_slice(&msg);
        buf
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        let time_nanos = u64::from_le_bytes(
            bytes[..8]
                .try_into()
                .expect("LogMessage: invalid timestamp bytes"),
        );
        let message =
            String::from_utf8(bytes[8..].to_vec()).expect("LogMessage: invalid UTF-8 message");
        Self {
            timeNanos: time_nanos,
            message,
        }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: LOG_MESSAGE_MAX_SIZE,
        is_fixed_size: false,
    };
}

// -- Binary Storable for LogMetadata: 3×u64 = 24 bytes, fixed-size --

#[derive(Clone, Debug)]
struct LogMetadata {
    max_count: u64,
    next_key: u64,
    oldest_key: u64,
}

impl LogMetadata {
    fn new(max_count: usize) -> Self {
        assert!(max_count > 0);
        Self {
            max_count: max_count as u64,
            next_key: 0,
            oldest_key: 0,
        }
    }

    fn len(&self) -> u64 {
        self.next_key - self.oldest_key
    }
}

impl Storable for LogMetadata {
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        let mut buf = [0u8; 24];
        buf[0..8].copy_from_slice(&self.max_count.to_le_bytes());
        buf[8..16].copy_from_slice(&self.next_key.to_le_bytes());
        buf[16..24].copy_from_slice(&self.oldest_key.to_le_bytes());
        Cow::Owned(buf.to_vec())
    }

    fn into_bytes(self) -> Vec<u8> {
        self.to_bytes().into_owned()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Self {
            max_count: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            next_key: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            oldest_key: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        }
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 24,
        is_fixed_size: true,
    };
}

// -- Storage --

pub struct Storage {
    messages: StableBTreeMap<u64, LogMessage, Memory>,
    metadata: StableCell<LogMetadata, Memory>,
    metadata_cache: LogMetadata,
}

impl Storage {
    /// Create **fresh** storage, overwriting any previous data in the memory regions.
    pub fn new(
        messages_memory: Memory,
        metadata_memory: Memory,
        max_messages_count: usize,
    ) -> Self {
        let mut messages = StableBTreeMap::init(messages_memory);
        messages.clear_new();
        let metadata = StableCell::new(metadata_memory, LogMetadata::new(max_messages_count));
        let metadata_cache = metadata.get().clone();
        Self {
            messages,
            metadata,
            metadata_cache,
        }
    }

    /// Restore storage from existing stable memory, preserving data across upgrades.
    /// `max_messages_count` is used only as a fallback if the memory region is blank.
    pub fn restore(
        messages_memory: Memory,
        metadata_memory: Memory,
        max_messages_count: usize,
    ) -> Self {
        let messages = StableBTreeMap::init(messages_memory);
        let metadata = StableCell::init(metadata_memory, LogMetadata::new(max_messages_count));
        let metadata_cache = metadata.get().clone();
        Self {
            messages,
            metadata,
            metadata_cache,
        }
    }

    fn metadata(&self) -> &LogMetadata {
        &self.metadata_cache
    }

    fn set_metadata(&mut self, m: LogMetadata) {
        self.metadata.set(m.clone());
        self.metadata_cache = m;
    }
}

impl LogMessagesInfo for Storage {
    fn get_log_messages_count(&self) -> u32 {
        self.metadata().len() as u32
    }

    fn get_first_log_message_time(&self) -> Option<u64> {
        let m = self.metadata();
        if m.len() == 0 {
            return None;
        }
        self.messages.get(&m.oldest_key).map(|msg| msg.timeNanos)
    }

    fn get_last_log_message_time(&self) -> Option<u64> {
        let m = self.metadata();
        if m.len() == 0 {
            return None;
        }
        self.messages
            .get(&(m.next_key - 1))
            .map(|msg| msg.timeNanos)
    }
}

impl LogMessagesStorage for Storage {
    fn store_log_message(&mut self, log_message: LogMessage) {
        let mut m = self.metadata().clone();

        // Evict the oldest entry when at capacity
        if m.len() >= m.max_count {
            self.messages.remove(&m.oldest_key);
            m.oldest_key += 1;
        }

        self.messages.insert(m.next_key, log_message);
        m.next_key += 1;

        self.set_metadata(m);
    }

    fn set_max_messages_count(&mut self, new_max_messages_count: usize) {
        let mut m = self.metadata().clone();
        let new_max = new_max_messages_count as u64;
        if m.max_count == new_max {
            return;
        }

        m.max_count = new_max;

        // Remove excess oldest entries
        while m.len() > m.max_count {
            self.messages.remove(&m.oldest_key);
            m.oldest_key += 1;
        }

        self.set_metadata(m);
    }
}

impl LogMessagesSupplier for Storage {
    /// Lazy forward iterator over messages in stable memory.
    fn get_log_messages(
        &self,
        from_time_nanos: &Option<u64>,
    ) -> Box<dyn Iterator<Item = LogMessage> + '_> {
        let from_time_nanos = *from_time_nanos;
        let m = self.metadata();
        Box::new(
            self.messages
                .range(m.oldest_key..m.next_key)
                .map(|entry| entry.value())
                .filter(move |msg| match from_time_nanos {
                    Some(t) => msg.timeNanos > t,
                    None => true,
                }),
        )
    }

    /// Lazy reverse iterator over messages in stable memory.
    fn get_log_messages_reverse(
        &self,
        up_to_time_nanos: &Option<u64>,
    ) -> Box<dyn Iterator<Item = LogMessage> + '_> {
        let up_to_time_nanos = *up_to_time_nanos;
        let m = self.metadata();
        Box::new(
            self.messages
                .range(m.oldest_key..m.next_key)
                .rev()
                .map(|entry| entry.value())
                .filter(move |msg| match up_to_time_nanos {
                    Some(t) => msg.timeNanos < t,
                    None => true,
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_type::LogMessageData;
    use ic_stable_structures::memory_manager::{MemoryId, MemoryManager};
    use ic_stable_structures::DefaultMemoryImpl;

    fn storage(max_messages_count: usize) -> Storage {
        let memory_manager = MemoryManager::init(DefaultMemoryImpl::default());
        Storage::new(
            memory_manager.get(MemoryId::new(10)),
            memory_manager.get(MemoryId::new(11)),
            max_messages_count,
        )
    }

    #[test]
    fn test_cyclic_behavior() {
        let mut storage = storage(2);

        storage.store_log_message(LogMessageData {
            timeNanos: 10,
            message: String::from("10"),
        });
        storage.store_log_message(LogMessageData {
            timeNanos: 20,
            message: String::from("20"),
        });
        storage.store_log_message(LogMessageData {
            timeNanos: 30,
            message: String::from("30"),
        });

        let messages: Vec<_> = storage.get_log_messages(&None).collect();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].timeNanos, 20);
        assert_eq!(messages[1].timeNanos, 30);
    }

    #[test]
    fn test_resize_keeps_latest_messages() {
        let mut storage = storage(4);

        for time_nanos in [10, 20, 30, 40] {
            storage.store_log_message(LogMessageData {
                timeNanos: time_nanos,
                message: time_nanos.to_string(),
            });
        }

        storage.set_max_messages_count(2);

        let messages: Vec<_> = storage.get_log_messages(&None).collect();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].timeNanos, 30);
        assert_eq!(messages[1].timeNanos, 40);
    }

    #[test]
    fn test_restore_preserves_data() {
        let memory_manager = MemoryManager::init(DefaultMemoryImpl::default());
        let msg_mem = memory_manager.get(MemoryId::new(10));
        let meta_mem = memory_manager.get(MemoryId::new(11));

        {
            let mut s = Storage::new(msg_mem.clone(), meta_mem.clone(), 4);
            s.store_log_message(LogMessageData {
                timeNanos: 100,
                message: "hello".into(),
            });
            s.store_log_message(LogMessageData {
                timeNanos: 200,
                message: "world".into(),
            });
        }

        // Simulate canister upgrade — restore from same memory
        let s = Storage::restore(msg_mem, meta_mem, 4);
        let messages: Vec<_> = s.get_log_messages(&None).collect();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].timeNanos, 100);
        assert_eq!(messages[1].timeNanos, 200);
    }
}

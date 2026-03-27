//! Logger data types

use crate::api_type::LogMessageData;
use crate::api_type::Nanos;

/// Log message type alias
pub type LogMessage = LogMessageData;

/// Trait for getting log messages information
pub trait LogMessagesInfo {
    fn get_log_messages_count(&self) -> u32;
    fn get_first_log_message_time(&self) -> Option<Nanos>;
    fn get_last_log_message_time(&self) -> Option<Nanos>;
}

/// Trait for supplying log messages (iterators)
pub trait LogMessagesSupplier: LogMessagesInfo {
    fn get_log_messages(
        &self,
        from_time_nanos: &Option<Nanos>,
    ) -> Box<dyn Iterator<Item = LogMessage> + '_>;

    fn get_log_messages_reverse(
        &self,
        up_to_time_nanos: &Option<Nanos>,
    ) -> Box<dyn Iterator<Item = LogMessage> + '_>;
}

/// Trait for storing log messages
pub trait LogMessagesStorage: LogMessagesInfo {
    fn store_log_message(&mut self, log_message: LogMessage);
    fn set_max_messages_count(&mut self, new_max_messages_count: usize);
}

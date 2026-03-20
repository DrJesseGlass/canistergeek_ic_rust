//! Logger storage base helpers

use crate::logger::data_type::LogMessage;

/// Helper struct for iterating over log messages in a circular buffer (reference version)
pub struct LogMessageRefIterator<'a> {
    pub queue: &'a Vec<LogMessage>,
    pub index: usize,
    pub delta: i32,
    pub next_count: usize,
    pub max_count: usize,
}

impl<'a> LogMessageRefIterator<'a> {
    /// Create a new iterator for forward iteration
    pub fn create(
        queue: &'a Vec<LogMessage>,
        from_time_nanos: &Option<u64>,
        next: usize,
        full: bool,
    ) -> Self {
        let len = queue.len();
        if len == 0 {
            return LogMessageRefIterator {
                queue,
                index: 0,
                delta: 0,
                next_count: 0,
                max_count: len,
            };
        }

        let start_index = if full { next } else { 0 };
        let count = if full { queue.len() } else { next };

        let mut iterator = LogMessageRefIterator {
            queue,
            index: start_index,
            delta: 1,
            next_count: count,
            max_count: queue.len(),
        };

        if from_time_nanos.is_some() {
            let from_time_nanos = from_time_nanos.unwrap();
            while !iterator.is_done()
                && iterator.get_current_message().unwrap().timeNanos <= from_time_nanos
            {
                iterator.shift_to_next();
            }
        }

        iterator
    }

    /// Create a new iterator for reverse iteration
    pub fn create_reverse(
        queue: &'a Vec<LogMessage>,
        up_to_time_nanos: &Option<u64>,
        next: usize,
        full: bool,
    ) -> Self {
        let len = queue.len();
        if len == 0 {
            return LogMessageRefIterator {
                queue,
                index: 0,
                delta: 0,
                next_count: 0,
                max_count: len,
            };
        }

        let count = if full { len } else { next };
        let start_index = if full {
            (next + len - 1) % len
        } else {
            next - 1
        };

        let mut iterator = LogMessageRefIterator {
            queue,
            index: start_index,
            delta: -1,
            next_count: count,
            max_count: queue.len(),
        };

        if up_to_time_nanos.is_some() {
            let up_to_time_nanos = up_to_time_nanos.unwrap();
            while !iterator.is_done()
                && iterator.get_current_message().unwrap().timeNanos >= up_to_time_nanos
            {
                iterator.shift_to_next();
            }
        }

        iterator
    }

    /// Get the current message at the iterator position
    pub fn get_current_message(&self) -> Option<&'a LogMessage> {
        self.queue.get(self.index)
    }

    /// Check if the iterator is done
    pub fn is_done(&self) -> bool {
        self.next_count == 0
    }

    /// Shift to the next position
    pub fn shift_to_next(&mut self) {
        if self.delta > 0 {
            self.index = (self.index + 1) % self.max_count;
        } else {
            self.index = if self.index == 0 {
                self.max_count - 1
            } else {
                self.index - 1
            };
        }
        self.next_count -= 1;
    }
}

impl<'a> Iterator for LogMessageRefIterator<'a> {
    type Item = &'a LogMessage;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            None
        } else {
            let message = self.get_current_message();
            self.shift_to_next();
            message
        }
    }
}

/// Helper functions for circular buffer index management
pub fn get_count(full: bool, next: usize, max_count: usize) -> usize {
    if full {
        max_count
    } else {
        next
    }
}

pub fn get_first_index(full: bool, next: usize) -> usize {
    if full {
        next
    } else {
        0
    }
}

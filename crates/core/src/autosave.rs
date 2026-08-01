use std::collections::HashMap;
use std::time::{Duration, Instant};

// Frontend owns autosave debounce; this scheduler is retained for future Rust-side flush hooks.
pub struct AutosaveScheduler {
    interval: Duration,
    pending_writes: HashMap<String, Instant>,
}

impl AutosaveScheduler {
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval: Duration::from_millis(interval_ms),
            pending_writes: HashMap::new(),
        }
    }

    pub fn queue_write(&mut self, doc_id: String) {
        self.pending_writes.insert(doc_id, Instant::now() + self.interval);
    }

    pub fn get_ready_flushes(&mut self) -> Vec<String> {
        let now = Instant::now();
        let mut ready = Vec::new();
        for (doc_id, time) in &self.pending_writes {
            if now >= *time {
                ready.push(doc_id.clone());
            }
        }
        for doc_id in &ready {
            self.pending_writes.remove(doc_id);
        }
        ready
    }

    pub fn remove_flush(&mut self, doc_id: &str) {
        self.pending_writes.remove(doc_id);
    }

    pub fn has_pending(&self, doc_id: &str) -> bool {
        self.pending_writes.contains_key(doc_id)
    }
}

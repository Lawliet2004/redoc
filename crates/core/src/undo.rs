use serde::{Deserialize, Serialize};

const MAX_DEPTH: usize = 20;
const MAX_MEMORY_BYTES: usize = 25 * 1024 * 1024; // 25 MB

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UndoOp {
    pub op_type: String,
    pub payload: serde_json::Value,
    pub inverse_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoStack {
    undo_list: Vec<UndoOp>,
    redo_list: Vec<UndoOp>,
    max_depth: usize,
    max_memory_bytes: usize,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            undo_list: Vec::new(),
            redo_list: Vec::new(),
            max_depth: MAX_DEPTH,
            max_memory_bytes: MAX_MEMORY_BYTES,
        }
    }

    pub fn with_limits(max_depth: usize, max_memory_bytes: usize) -> Self {
        Self {
            undo_list: Vec::new(),
            redo_list: Vec::new(),
            max_depth,
            max_memory_bytes,
        }
    }

    fn current_memory_usage(&self) -> usize {
        let mut size = 0;
        for op in &self.undo_list {
            size += serde_json::to_vec(op).unwrap_or_default().len();
        }
        for op in &self.redo_list {
            size += serde_json::to_vec(op).unwrap_or_default().len();
        }
        size
    }

    fn enforce_limits(&mut self) {
        while self.undo_list.len() > self.max_depth {
            self.undo_list.remove(0);
        }
        while self.current_memory_usage() > self.max_memory_bytes && !self.undo_list.is_empty() {
            self.undo_list.remove(0);
        }
    }

    pub fn push(&mut self, op: UndoOp) {
        self.undo_list.push(op);
        self.redo_list.clear();
        self.enforce_limits();
    }

    pub fn undo(&mut self) -> Option<UndoOp> {
        let op = self.undo_list.pop()?;
        self.redo_list.push(op.clone());
        self.enforce_limits();
        Some(op)
    }

    pub fn redo(&mut self) -> Option<UndoOp> {
        let op = self.redo_list.pop()?;
        self.undo_list.push(op.clone());
        self.enforce_limits();
        Some(op)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_list.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_list.is_empty()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_op(data: &str) -> UndoOp {
        UndoOp {
            op_type: "insert".to_string(),
            payload: serde_json::json!({ "data": data }),
            inverse_payload: serde_json::json!({ "data": data }),
        }
    }

    #[test]
    fn test_max_depth() {
        let mut stack = UndoStack::with_limits(3, 1024 * 1024);
        stack.push(dummy_op("1"));
        stack.push(dummy_op("2"));
        stack.push(dummy_op("3"));
        stack.push(dummy_op("4"));

        assert_eq!(stack.undo_list.len(), 3);
        assert_eq!(stack.undo_list[0].payload["data"], "2");
        assert_eq!(stack.undo_list[2].payload["data"], "4");
    }

    #[test]
    fn test_memory_bounding() {
        // Very small memory limit to force eviction
        let mut stack = UndoStack::with_limits(10, 150); 
        stack.push(dummy_op("A"));

        // Push enough to exceed 150 bytes. Each op is around 80-90 bytes.
        stack.push(dummy_op("B"));
        stack.push(dummy_op("C"));
        
        assert!(stack.current_memory_usage() <= 150);
        assert!(stack.undo_list.len() < 3);
        let last = stack.undo_list.last().unwrap();
        assert_eq!(last.payload["data"], "C");
    }

    #[test]
    fn test_serialization() {
        let mut stack = UndoStack::new();
        stack.push(dummy_op("test"));
        
        let json = stack.to_json().unwrap();
        let restored = UndoStack::from_json(&json).unwrap();
        
        assert_eq!(restored.undo_list.len(), 1);
        assert_eq!(restored.undo_list[0].payload["data"], "test");
        assert_eq!(restored.max_depth, MAX_DEPTH);
        assert_eq!(restored.max_memory_bytes, MAX_MEMORY_BYTES);
    }
}

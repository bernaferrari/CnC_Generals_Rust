//! Buffer pool for efficient memory management

/// Buffer pool
pub struct BufferPool;

impl BufferPool {
    /// Create new pool
    pub fn new() -> Self {
        Self
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new()
    }
}
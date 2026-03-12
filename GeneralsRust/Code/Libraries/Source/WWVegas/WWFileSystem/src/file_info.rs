/// FileInfo structure matching the C++ implementation
/// Stores file metadata including size and timestamp
#[derive(Debug, Clone, Default)]
pub struct FileInfo {
    pub size_high: i32,
    pub size_low: i32,
    pub timestamp_high: i32,
    pub timestamp_low: i32,
}

impl FileInfo {
    pub fn new() -> Self {
        Self {
            size_high: 0,
            size_low: 0,
            timestamp_high: 0,
            timestamp_low: 0,
        }
    }

    pub fn clear(&mut self) {
        self.size_high = 0;
        self.size_low = 0;
        self.timestamp_high = 0;
        self.timestamp_low = 0;
    }
}

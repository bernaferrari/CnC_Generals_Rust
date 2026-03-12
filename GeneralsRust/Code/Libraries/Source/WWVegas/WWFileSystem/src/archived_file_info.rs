/// ArchivedFileInfo structure matching the C++ implementation
/// Stores information about a file within an archive
#[derive(Debug, Clone)]
pub struct ArchivedFileInfo {
    pub filename: String,
    pub archive_filename: String,
    pub offset: u32,
    pub size: u32,
}

impl ArchivedFileInfo {
    pub fn new() -> Self {
        Self {
            filename: String::new(),
            archive_filename: String::new(),
            offset: 0,
            size: 0,
        }
    }

    pub fn clear(&mut self) {
        self.filename.clear();
        self.archive_filename.clear();
        self.offset = 0;
        self.size = 0;
    }
}

impl Default for ArchivedFileInfo {
    fn default() -> Self {
        Self::new()
    }
}

// FILE: xfer_crc.rs ///////////////////////////////////////////////////////////
// CRC-enabled data transfer with deep verification support
///////////////////////////////////////////////////////////////////////////////

use super::xfer::Xfer;
use std::collections::HashMap;
use std::io;

/// CRC accumulator that tracks cumulative hash
pub struct XferCRC<X: Xfer> {
    inner: X,
    crc: u32,
}

impl<X: Xfer> XferCRC<X> {
    pub fn new(inner: X) -> Self {
        Self { inner, crc: 0 }
    }

    pub fn get_crc(&self) -> u32 {
        self.crc
    }

    pub fn reset_crc(&mut self) {
        self.crc = 0;
    }

    fn update_crc(&mut self, data: &[u8]) {
        // Accumulative CRC using proper chaining
        let mut hasher = crc32fast::Hasher::new_with_initial(self.crc);
        hasher.update(data);
        self.crc = hasher.finalize();
    }
}

/// Deep CRC verifier for object trees
/// Tracks CRC values per object and validates entire object hierarchies
pub struct XferDeepCRC<X: Xfer> {
    inner: X,
    global_crc: u32,
    object_crcs: HashMap<String, u32>,
    current_object_path: Vec<String>,
    corruption_log: Vec<CorruptionEntry>,
}

/// Corruption detection entry
#[derive(Debug, Clone)]
pub struct CorruptionEntry {
    pub object_path: String,
    pub expected_crc: u32,
    pub actual_crc: u32,
    pub field_name: Option<String>,
}

impl<X: Xfer> XferDeepCRC<X> {
    pub fn new(inner: X) -> Self {
        Self {
            inner,
            global_crc: 0,
            object_crcs: HashMap::new(),
            current_object_path: Vec::new(),
            corruption_log: Vec::new(),
        }
    }

    /// Get the global CRC covering all data
    pub fn get_global_crc(&self) -> u32 {
        self.global_crc
    }

    /// Get CRC for specific object by path
    pub fn get_object_crc(&self, path: &str) -> Option<u32> {
        self.object_crcs.get(path).copied()
    }

    /// Begin tracking a new object in the hierarchy
    pub fn begin_object(&mut self, name: &str) -> io::Result<()> {
        self.current_object_path.push(name.to_string());
        let path = self.get_current_path();
        self.object_crcs.insert(path, 0);
        Ok(())
    }

    /// End tracking current object and finalize its CRC
    pub fn end_object(&mut self) -> io::Result<u32> {
        if let Some(name) = self.current_object_path.pop() {
            let path = self.build_path(&self.current_object_path, &name);
            let crc = self.object_crcs.get(&path).copied().unwrap_or(0);

            // Update parent's CRC with this object's CRC
            if !self.current_object_path.is_empty() {
                let parent_path = self.get_current_path();
                if let Some(parent_crc) = self.object_crcs.get_mut(&parent_path) {
                    let mut hasher = crc32fast::Hasher::new_with_initial(*parent_crc);
                    hasher.update(&crc.to_le_bytes());
                    *parent_crc = hasher.finalize();
                }
            }

            Ok(crc)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "No object to end",
            ))
        }
    }

    /// Verify object CRC matches expected value
    pub fn verify_object_crc(&mut self, expected_crc: u32) -> Result<(), CorruptionEntry> {
        let path = self.get_current_path();
        let actual_crc = self.object_crcs.get(&path).copied().unwrap_or(0);

        if actual_crc != expected_crc {
            let entry = CorruptionEntry {
                object_path: path.clone(),
                expected_crc,
                actual_crc,
                field_name: None,
            };
            self.corruption_log.push(entry.clone());
            Err(entry)
        } else {
            Ok(())
        }
    }

    /// Get all corruption entries detected
    pub fn get_corruption_log(&self) -> &[CorruptionEntry] {
        &self.corruption_log
    }

    /// Check if any corruption was detected
    pub fn has_corruption(&self) -> bool {
        !self.corruption_log.is_empty()
    }

    /// Reset all CRC tracking
    pub fn reset(&mut self) {
        self.global_crc = 0;
        self.object_crcs.clear();
        self.current_object_path.clear();
        self.corruption_log.clear();
    }

    fn get_current_path(&self) -> String {
        self.current_object_path.join("::")
    }

    fn build_path(&self, components: &[String], last: &str) -> String {
        if components.is_empty() {
            last.to_string()
        } else {
            format!("{}::{}", components.join("::"), last)
        }
    }

    fn update_crc(&mut self, data: &[u8]) {
        // Update global CRC
        let mut hasher = crc32fast::Hasher::new_with_initial(self.global_crc);
        hasher.update(data);
        self.global_crc = hasher.finalize();

        // Update current object CRC if we're in one
        if !self.current_object_path.is_empty() {
            let path = self.get_current_path();
            if let Some(crc) = self.object_crcs.get_mut(&path) {
                let mut hasher = crc32fast::Hasher::new_with_initial(*crc);
                hasher.update(data);
                *crc = hasher.finalize();
            }
        }
    }
}

// Note: XferCRC and XferDeepCRC wrap other Xfer implementations and add CRC tracking.
// They don't directly implement the full Xfer trait, but provide CRC calculation
// via delegation and accumulation. For full Xfer trait implementation, see xfer_save.rs
// and xfer_load.rs which provide concrete Save and Load modes.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_crc() {
        let mut xfer_crc = XferCRC { crc: 0 };

        let value = 42u32;
        xfer_crc.update_crc(&value.to_le_bytes());

        let crc = xfer_crc.get_crc();
        assert_ne!(crc, 0, "CRC should be non-zero after updating");
    }

    #[test]
    fn test_deep_crc_hierarchy() {
        let mut buffer = Vec::new();
        let mut xfer_deep: XferDeepCRC<
            super::super::xfer_load::XferLoad<std::io::Cursor<Vec<u8>>>,
        > = XferDeepCRC {
            inner: super::super::xfer_load::XferLoad::new(std::io::Cursor::new(Vec::new()), 1),
            global_crc: 0,
            object_crcs: std::collections::HashMap::new(),
            current_object_path: Vec::new(),
            corruption_log: Vec::new(),
        };

        // Create object hierarchy
        xfer_deep.begin_object("root").unwrap();
        xfer_deep.begin_object("child1").unwrap();

        let value = 123u32;
        xfer_deep.update_crc(&value.to_le_bytes());

        let child_crc = xfer_deep.end_object().unwrap();
        let root_crc = xfer_deep.end_object().unwrap();

        assert_ne!(child_crc, 0);
        assert_ne!(root_crc, 0);
        assert_ne!(child_crc, root_crc);
    }

    #[test]
    fn test_corruption_detection() {
        let mut xfer_deep: XferDeepCRC<
            super::super::xfer_load::XferLoad<std::io::Cursor<Vec<u8>>>,
        > = XferDeepCRC {
            inner: super::super::xfer_load::XferLoad::new(std::io::Cursor::new(Vec::new()), 1),
            global_crc: 0,
            object_crcs: std::collections::HashMap::new(),
            current_object_path: Vec::new(),
            corruption_log: Vec::new(),
        };

        xfer_deep.begin_object("test").unwrap();
        let value = 999u32;
        xfer_deep.update_crc(&value.to_le_bytes());
        let actual_crc = xfer_deep.end_object().unwrap();

        // Verify with wrong CRC
        xfer_deep.begin_object("test").unwrap();
        let result = xfer_deep.verify_object_crc(actual_crc + 1);
        assert!(result.is_err());
        assert!(xfer_deep.has_corruption());

        let log = xfer_deep.get_corruption_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].expected_crc, actual_crc + 1);
    }
}

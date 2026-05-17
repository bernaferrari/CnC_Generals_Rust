// FILE: xfer_crc.rs ///////////////////////////////////////////////////////////
// CRC-enabled data transfer with deep verification support
///////////////////////////////////////////////////////////////////////////////

use super::snapshot::Snapshot;
use super::xfer::{Xfer, XferBlockSize, XferMode, XferStatus};
use std::collections::BTreeMap;
use std::io;

fn add_crc_word(crc: u32, word: u32) -> u32 {
    let word = word.to_be();
    let hibit = u32::from((crc & 0x8000_0000) != 0);
    crc.wrapping_shl(1).wrapping_add(word).wrapping_add(hibit)
}

fn fold_crc_bytes(mut crc: u32, data: &[u8]) -> u32 {
    let full_words = data.len() / 4;
    for chunk in data[..full_words * 4].chunks_exact(4) {
        crc = add_crc_word(crc, u32::from_ne_bytes(chunk.try_into().unwrap()));
    }

    let leftover = &data[full_words * 4..];
    if !leftover.is_empty() {
        let mut word = 0u32;
        for (index, byte) in leftover.iter().enumerate() {
            word = word.wrapping_add((*byte as u32) << (index * 8));
        }
        crc = add_crc_word(crc, word.to_be());
    }

    crc
}

fn utf16_le_bytes(data: &str) -> Vec<u8> {
    data.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>()
}

/// CRC accumulator that tracks cumulative hash
pub struct XferCRC<X: Xfer> {
    #[allow(dead_code)] // C++ parity: inner Xfer is stored for potential future delegation
    inner: X,
    crc: u32,
}

impl<X: Xfer> XferCRC<X> {
    pub fn new(inner: X) -> Self {
        Self { inner, crc: 0 }
    }

    pub fn get_crc(&self) -> u32 {
        self.crc.to_be()
    }

    pub fn reset_crc(&mut self) {
        self.crc = 0;
    }

    fn update_crc(&mut self, data: &[u8]) {
        self.crc = fold_crc_bytes(self.crc, data);
    }
}

/// Deep CRC verifier for object trees
/// Tracks CRC values per object and validates entire object hierarchies
pub struct XferDeepCRC<X: Xfer> {
    #[allow(dead_code)] // C++ parity: inner Xfer is stored for potential future delegation
    inner: X,
    global_crc: u32,
    object_crcs: BTreeMap<String, u32>,
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
            object_crcs: BTreeMap::new(),
            current_object_path: Vec::new(),
            corruption_log: Vec::new(),
        }
    }

    /// Get the global CRC covering all data
    pub fn get_global_crc(&self) -> u32 {
        self.global_crc.to_be()
    }

    /// Get CRC for specific object by path
    pub fn get_object_crc(&self, path: &str) -> Option<u32> {
        self.object_crcs.get(path).copied().map(u32::to_be)
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
                    *parent_crc = add_crc_word(*parent_crc, crc);
                }
            }

            Ok(crc.to_be())
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
        let actual_crc = self.object_crcs.get(&path).copied().unwrap_or(0).to_be();

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
        // Update global CRC using the same accumulation rule as the C++ XferCRC.
        self.global_crc = fold_crc_bytes(self.global_crc, data);

        // Update current object CRC if we're in one
        if !self.current_object_path.is_empty() {
            let path = self.get_current_path();
            if let Some(crc) = self.object_crcs.get_mut(&path) {
                *crc = fold_crc_bytes(*crc, data);
            }
        }
    }
}

// Note: XferCRC and XferDeepCRC wrap other Xfer implementations and add CRC tracking.

impl<X: Xfer> Xfer for XferCRC<X> {
    fn get_xfer_mode(&self) -> XferMode {
        XferMode::Crc
    }

    fn get_identifier(&self) -> &str {
        self.inner.get_identifier()
    }

    fn set_options(&mut self, options: u32) {
        self.inner.set_options(options)
    }

    fn clear_options(&mut self, options: u32) {
        self.inner.clear_options(options)
    }

    fn get_options(&self) -> u32 {
        self.inner.get_options()
    }

    fn open(&mut self, identifier: &str) -> Result<(), XferStatus> {
        self.inner.open(identifier)
    }

    fn close(&mut self) -> Result<(), XferStatus> {
        self.inner.close()
    }

    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        self.inner.begin_block()
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        self.inner.end_block()
    }

    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
        self.inner.skip(data_size)
    }

    fn xfer_snapshot(&mut self, snapshot: &mut Snapshot) -> Result<(), XferStatus> {
        snapshot.crc(self).map_err(|_| XferStatus::InvalidData)
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()> {
        self.update_crc(ascii_string_data.as_bytes());
        Ok(())
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()> {
        self.update_crc(&utf16_le_bytes(unicode_string_data));
        Ok(())
    }

    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        if data_size > 0 && !data.is_null() {
            let slice = std::slice::from_raw_parts(data, data_size);
            self.update_crc(slice);
        }
        Ok(())
    }
}

impl<X: Xfer> Xfer for XferDeepCRC<X> {
    fn get_xfer_mode(&self) -> XferMode {
        self.inner.get_xfer_mode()
    }

    fn get_identifier(&self) -> &str {
        self.inner.get_identifier()
    }

    fn set_options(&mut self, options: u32) {
        self.inner.set_options(options)
    }

    fn clear_options(&mut self, options: u32) {
        self.inner.clear_options(options)
    }

    fn get_options(&self) -> u32 {
        self.inner.get_options()
    }

    fn open(&mut self, identifier: &str) -> Result<(), XferStatus> {
        self.inner.open(identifier)
    }

    fn close(&mut self) -> Result<(), XferStatus> {
        self.inner.close()
    }

    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus> {
        self.inner.begin_block()
    }

    fn end_block(&mut self) -> Result<(), XferStatus> {
        self.inner.end_block()
    }

    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus> {
        self.inner.skip(data_size)
    }

    fn xfer_snapshot(&mut self, snapshot: &mut Snapshot) -> Result<(), XferStatus> {
        self.inner.xfer_snapshot(snapshot)
    }

    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()> {
        if ascii_string_data.len() > 16385 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "XferDeepCRC ascii string too long (max 16385)",
            ));
        }
        let mut len = ascii_string_data.len() as u16;
        self.xfer_unsigned_short(&mut len)?;
        if len > 0 {
            let bytes = ascii_string_data.as_bytes();
            unsafe {
                self.xfer_implementation(bytes.as_ptr() as *mut u8, bytes.len())?;
            }
        }
        Ok(())
    }

    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()> {
        let utf16 = utf16_le_bytes(unicode_string_data);
        let units = utf16.len() / 2;
        if units > 255 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "XferDeepCRC unicode string too long (max 255)",
            ));
        }
        let mut len = units as u8;
        self.xfer_unsigned_byte(&mut len)?;
        if !utf16.is_empty() {
            unsafe {
                self.xfer_implementation(utf16.as_ptr() as *mut u8, utf16.len())?;
            }
        }
        Ok(())
    }

    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        let result = unsafe { self.inner.xfer_implementation(data, data_size) };
        if result.is_ok() {
            let slice = std::slice::from_raw_parts(data, data_size);
            self.update_crc(slice);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_basic_crc() {
        let mut xfer_crc = XferCRC::new(super::super::xfer_load::XferLoad::new(
            Cursor::new(Vec::new()),
            1,
        ));

        let value = 42u32;
        xfer_crc.update_crc(&value.to_le_bytes());

        let crc = xfer_crc.get_crc();
        assert_ne!(crc, 0, "CRC should be non-zero after updating");
    }

    #[test]
    fn test_deep_crc_hierarchy() {
        let mut xfer_deep = XferDeepCRC::new(super::super::xfer_load::XferLoad::new(
            Cursor::new(Vec::new()),
            1,
        ));

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
        let mut xfer_deep = XferDeepCRC::new(super::super::xfer_load::XferLoad::new(
            Cursor::new(Vec::new()),
            1,
        ));

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

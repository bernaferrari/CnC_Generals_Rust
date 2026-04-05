////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: snapshot.rs ///////////////////////////////////////////////////////////
// Game state snapshot functionality
///////////////////////////////////////////////////////////////////////////////

use super::xfer::Xfer;
use std::collections::HashMap;
use std::io::{self, Read, Write};

/// Snapshot trait for objects that can be serialized and have their state saved/loaded
pub trait Snapshotable {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String>;
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String>;
    fn load_post_process(&mut self) -> Result<(), String>;
}

/// Snapshot data storage
#[derive(Debug, Clone)]
pub struct Snapshot {
    frame_number: u32,
    timestamp: f64,
    data: Vec<u8>,
    metadata: HashMap<String, String>,
}

impl Snapshot {
    pub fn new(frame_number: u32, timestamp: f64) -> Self {
        Self {
            frame_number,
            timestamp,
            data: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn add_data(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    pub fn frame_number(&self) -> u32 {
        self.frame_number
    }

    pub fn timestamp(&self) -> f64 {
        self.timestamp
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.metadata.clear();
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) -> io::Result<()> {
        // TODO: In C++, Snapshot is an abstract class with pure virtual crc().
        // Each subclass implements crc() by calling xfer methods on its own fields.
        // This generic implementation CRCs the raw data bytes for compatibility.
        let mut frame = self.frame_number;
        xfer.xfer_unsigned_int(&mut frame)?;
        let mut ts_bytes = self.timestamp.to_le_bytes();
        for b in ts_bytes.iter_mut() {
            let mut byte = *b;
            xfer.xfer_unsigned_byte(&mut byte)?;
        }
        if !self.data.is_empty() {
            unsafe {
                xfer.xfer_user(self.data.as_ptr() as *mut u8, self.data.len())?;
            }
        }
        Ok(())
    }

    pub fn save_to_writer<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        // Write frame number
        writer.write_all(&self.frame_number.to_le_bytes())?;

        // Write timestamp
        writer.write_all(&self.timestamp.to_le_bytes())?;

        // Write data size and data
        writer.write_all(&(self.data.len() as u32).to_le_bytes())?;
        writer.write_all(&self.data)?;

        // Write metadata
        writer.write_all(&(self.metadata.len() as u32).to_le_bytes())?;
        for (key, value) in &self.metadata {
            writer.write_all(&(key.len() as u32).to_le_bytes())?;
            writer.write_all(key.as_bytes())?;
            writer.write_all(&(value.len() as u32).to_le_bytes())?;
            writer.write_all(value.as_bytes())?;
        }

        Ok(())
    }

    pub fn load_from_reader<R: Read>(reader: &mut R) -> io::Result<Self> {
        // Read frame number
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        let frame_number = u32::from_le_bytes(buffer);

        // Read timestamp
        let mut buffer = [0u8; 8];
        reader.read_exact(&mut buffer)?;
        let timestamp = f64::from_le_bytes(buffer);

        // Read data
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        let data_size = u32::from_le_bytes(buffer) as usize;
        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;

        // Read metadata
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        let metadata_count = u32::from_le_bytes(buffer);
        let mut metadata = HashMap::new();

        for _ in 0..metadata_count {
            // Read key
            let mut buffer = [0u8; 4];
            reader.read_exact(&mut buffer)?;
            let key_len = u32::from_le_bytes(buffer) as usize;
            let mut key_bytes = vec![0u8; key_len];
            reader.read_exact(&mut key_bytes)?;
            let key = String::from_utf8(key_bytes)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in key"))?;

            // Read value
            let mut buffer = [0u8; 4];
            reader.read_exact(&mut buffer)?;
            let value_len = u32::from_le_bytes(buffer) as usize;
            let mut value_bytes = vec![0u8; value_len];
            reader.read_exact(&mut value_bytes)?;
            let value = String::from_utf8(value_bytes).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 in value")
            })?;

            metadata.insert(key, value);
        }

        Ok(Self {
            frame_number,
            timestamp,
            data,
            metadata,
        })
    }
}

/// Snapshot manager
pub struct SnapshotManager {
    snapshots: Vec<Snapshot>,
    max_snapshots: usize,
}

impl SnapshotManager {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    pub fn add_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshots.push(snapshot);

        // Keep only the most recent snapshots
        if self.snapshots.len() > self.max_snapshots {
            self.snapshots
                .drain(0..self.snapshots.len() - self.max_snapshots);
        }
    }

    pub fn get_snapshot(&self, frame_number: u32) -> Option<&Snapshot> {
        self.snapshots
            .iter()
            .find(|s| s.frame_number == frame_number)
    }

    pub fn get_latest_snapshot(&self) -> Option<&Snapshot> {
        self.snapshots.last()
    }

    pub fn clear(&mut self) {
        self.snapshots.clear();
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_snapshot_creation() {
        let mut snapshot = Snapshot::new(100, 1.5);
        assert_eq!(snapshot.frame_number(), 100);
        assert_eq!(snapshot.timestamp(), 1.5);

        snapshot.add_data(b"test data");
        assert_eq!(snapshot.data_size(), 9);

        snapshot.set_metadata("version".to_string(), "1.0".to_string());
        assert_eq!(snapshot.get_metadata("version"), Some(&"1.0".to_string()));
    }

    #[test]
    fn test_snapshot_serialization() {
        let mut snapshot = Snapshot::new(42, 3.14);
        snapshot.add_data(b"hello");
        snapshot.set_metadata("test".to_string(), "value".to_string());

        let mut buffer = Vec::new();
        snapshot.save_to_writer(&mut buffer).unwrap();

        let mut cursor = Cursor::new(buffer);
        let loaded_snapshot = Snapshot::load_from_reader(&mut cursor).unwrap();

        assert_eq!(loaded_snapshot.frame_number(), 42);
        assert_eq!(loaded_snapshot.timestamp(), 3.14);
        assert_eq!(loaded_snapshot.data(), b"hello");
        assert_eq!(
            loaded_snapshot.get_metadata("test"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_snapshot_manager() {
        let mut manager = SnapshotManager::new(3);

        for i in 0..5 {
            let snapshot = Snapshot::new(i, i as f64);
            manager.add_snapshot(snapshot);
        }

        // Should only keep the last 3 snapshots
        assert_eq!(manager.snapshot_count(), 3);
        assert!(manager.get_snapshot(0).is_none());
        assert!(manager.get_snapshot(1).is_none());
        assert!(manager.get_snapshot(2).is_some());
        assert!(manager.get_snapshot(3).is_some());
        assert!(manager.get_snapshot(4).is_some());
    }
}

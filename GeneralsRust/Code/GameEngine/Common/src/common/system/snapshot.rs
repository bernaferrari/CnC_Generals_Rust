////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: snapshot.rs ///////////////////////////////////////////////////////////
// C++ Snapshot is an abstract interface (Snapshot.h:20-46).
///////////////////////////////////////////////////////////////////////////////

use super::xfer::Xfer;

/// Snapshot trait for objects that can be serialized and have their state saved/loaded.
///
/// C++ `Snapshot` (`Snapshot.h:20-46`) is abstract with `crc` / `xfer` /
/// `loadPostProcess`. This trait is the live Rust equivalent.
pub trait Snapshotable {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String>;
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String>;
    fn load_post_process(&mut self) -> Result<(), String>;
}

/// Invented byte-buffer helper. Not a C++ type. Test-only — live
/// `Xfer::xfer_snapshot` takes `dyn Snapshotable` (C++ `Snapshot*`).
#[cfg(test)]
#[derive(Debug, Clone)]
pub struct Snapshot {
    frame_number: u32,
    timestamp: f64,
    data: Vec<u8>,
    metadata: std::collections::HashMap<String, String>,
}

#[cfg(test)]
impl Snapshot {
    pub fn new(frame_number: u32, timestamp: f64) -> Self {
        Self {
            frame_number,
            timestamp,
            data: Vec::new(),
            metadata: std::collections::HashMap::new(),
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

    /// C++ Snapshot::xfer visitor — mutate this object through `xfer`, never
    /// replace it. `XferLoad.cpp:161` calls `snapshot->xfer(this)`.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> std::io::Result<()> {
        xfer.xfer_unsigned_int(&mut self.frame_number)?;
        let mut ts_bytes = self.timestamp.to_le_bytes();
        for byte in &mut ts_bytes {
            xfer.xfer_unsigned_byte(byte)?;
        }
        if xfer.is_reading() {
            self.timestamp = f64::from_le_bytes(ts_bytes);
        }
        let mut data_len = self.data.len() as u32;
        xfer.xfer_unsigned_int(&mut data_len)?;
        if xfer.is_reading() {
            self.data.resize(data_len as usize, 0);
        }
        if data_len > 0 {
            // SAFETY: `data` has exactly `data_len` bytes.
            unsafe {
                xfer.xfer_user(self.data.as_mut_ptr(), data_len as usize)?;
            }
        }
        let mut metadata_count = self.metadata.len() as u32;
        xfer.xfer_unsigned_int(&mut metadata_count)?;
        if xfer.is_writing() {
            let mut entries: Vec<(String, String)> = self
                .metadata
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (mut key, mut value) in entries {
                xfer.xfer_ascii_string(&mut key)?;
                xfer.xfer_ascii_string(&mut value)?;
            }
        } else {
            self.metadata.clear();
            for _ in 0..metadata_count {
                let mut key = String::new();
                let mut value = String::new();
                xfer.xfer_ascii_string(&mut key)?;
                xfer.xfer_ascii_string(&mut value)?;
                self.metadata.insert(key, value);
            }
        }
        Ok(())
    }

    /// C++ Snapshot::loadPostProcess — empty on the generic blob.
    pub fn load_post_process(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) -> std::io::Result<()> {
        let mut frame = self.frame_number;
        xfer.xfer_unsigned_int(&mut frame)?;
        let mut ts_bytes = self.timestamp.to_le_bytes();
        for b in ts_bytes.iter_mut() {
            let mut byte = *b;
            xfer.xfer_unsigned_byte(&mut byte)?;
        }
        if !self.data.is_empty() {
                        // SAFETY: self.data is an owned Vec<u8>; casting to *mut u8 is
                        // fine because Crc mode never writes through xfer_user.
            unsafe {
                xfer.xfer_user(self.data.as_ptr() as *mut u8, self.data.len())?;
            }
        }
        Ok(())
    }

    pub fn save_to_writer<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.frame_number.to_le_bytes())?;
        writer.write_all(&self.timestamp.to_le_bytes())?;
        writer.write_all(&(self.data.len() as u32).to_le_bytes())?;
        writer.write_all(&self.data)?;
        writer.write_all(&(self.metadata.len() as u32).to_le_bytes())?;
        for (key, value) in &self.metadata {
            writer.write_all(&(key.len() as u32).to_le_bytes())?;
            writer.write_all(key.as_bytes())?;
            writer.write_all(&(value.len() as u32).to_le_bytes())?;
            writer.write_all(value.as_bytes())?;
        }
        Ok(())
    }

    pub fn load_from_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        let frame_number = u32::from_le_bytes(buffer);

        let mut buffer = [0u8; 8];
        reader.read_exact(&mut buffer)?;
        let timestamp = f64::from_le_bytes(buffer);

        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        let data_size = u32::from_le_bytes(buffer) as usize;
        let mut data = vec![0u8; data_size];
        reader.read_exact(&mut data)?;

        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        let metadata_count = u32::from_le_bytes(buffer);
        let mut metadata = std::collections::HashMap::new();

        for _ in 0..metadata_count {
            let mut buffer = [0u8; 4];
            reader.read_exact(&mut buffer)?;
            let key_len = u32::from_le_bytes(buffer) as usize;
            let mut key_bytes = vec![0u8; key_len];
            reader.read_exact(&mut key_bytes)?;
            let key = String::from_utf8(key_bytes).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 in key")
            })?;

            let mut buffer = [0u8; 4];
            reader.read_exact(&mut buffer)?;
            let value_len = u32::from_le_bytes(buffer) as usize;
            let mut value_bytes = vec![0u8; value_len];
            reader.read_exact(&mut value_bytes)?;
            let value = String::from_utf8(value_bytes).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8 in value")
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

#[cfg(test)]
impl Snapshotable for Snapshot {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshot::crc(self, xfer).map_err(|e| e.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshot::xfer(self, xfer).map_err(|e| e.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Invented ring buffer. C++ has no SnapshotManager.
#[cfg(test)]
pub struct SnapshotManager {
    snapshots: Vec<Snapshot>,
    max_snapshots: usize,
}

#[cfg(test)]
impl SnapshotManager {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    pub fn add_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshots.push(snapshot);
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
    use super::super::xfer::Xfer;
    use super::super::xfer_load::XferLoad;
    use super::super::xfer_save::XferSave;
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

        assert_eq!(manager.snapshot_count(), 3);
        assert!(manager.get_snapshot(0).is_none());
        assert!(manager.get_snapshot(1).is_none());
        assert!(manager.get_snapshot(2).is_some());
        assert!(manager.get_snapshot(3).is_some());
        assert!(manager.get_snapshot(4).is_some());
    }

    /// C++ `Xfer::xferSnapshot(Snapshot*)` (`Xfer.h:108`) takes the abstract
    /// Snapshot interface (`Snapshot.h:20-46`), not a byte-buffer container.
    /// Pre-fix Rust required the invented `Snapshot` struct as the payload type.
    #[test]
    fn xfer_snapshot_dispatches_through_snapshotable() {
        struct Tiny {
            value: u32,
        }
        impl Snapshotable for Tiny {
            fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
                Ok(())
            }
            fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
                xfer.xfer_unsigned_int(&mut self.value)
                    .map_err(|e| e.to_string())
            }
            fn load_post_process(&mut self) -> Result<(), String> {
                Ok(())
            }
        }

        let mut saved = Tiny { value: 0xA11C_E5 };
        let mut encoded = Vec::new();
        {
            let mut save = XferSave::new(Cursor::new(&mut encoded), 1);
            save.xfer_snapshot(&mut saved).expect("save Snapshotable");
        }

        let mut loaded = Tiny { value: 0 };
        {
            let mut load = XferLoad::new(Cursor::new(&encoded), 1);
            load.xfer_snapshot(&mut loaded).expect("load Snapshotable");
        }
        assert_eq!(loaded.value, 0xA11C_E5);
    }
}

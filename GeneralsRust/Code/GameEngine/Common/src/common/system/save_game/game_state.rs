// FILE: save_game/game_state.rs ///////////////////////////////////////////////
// Game state management for save/load functionality
///////////////////////////////////////////////////////////////////////////////

use crate::common::system::xfer::Xfer;
use std::collections::HashMap;
use std::io;

#[derive(Debug, Clone)]
pub struct GameState {
    pub version: u32,
    pub timestamp: u64,
    pub map_name: String,
    pub game_mode: String,
    pub player_count: u32,
    pub current_frame: u32,
    pub elapsed_time: f32,
    pub metadata: HashMap<String, String>,
    pub data: Vec<u8>,
}

impl GameState {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            timestamp: 0,
            map_name: String::new(),
            game_mode: String::new(),
            player_count: 0,
            current_frame: 0,
            elapsed_time: 0.0,
            metadata: HashMap::new(),
            data: Vec::new(),
        }
    }

    pub fn set_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    pub fn clear_data(&mut self) {
        self.data.clear();
    }

    pub fn append_data(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    pub fn xfer<X: Xfer>(&mut self, xfer: &mut X) -> io::Result<()> {
        // C++ uses UnsignedByte (u8) for version - matches C++ parity
        // GameState version stored internally as u32 but serialized as u8
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        self.version = version as u32;

        xfer.xfer_u64(&mut self.timestamp)?;

        xfer.xfer_ascii_string(&mut self.map_name)?;
        xfer.xfer_ascii_string(&mut self.game_mode)?;
        xfer.xfer_unsigned_int(&mut self.player_count)?;
        xfer.xfer_unsigned_int(&mut self.current_frame)?;
        xfer.xfer_real(&mut self.elapsed_time)?;

        // Transfer metadata with explicit read/write behavior and deterministic save ordering.
        let mut metadata_count = self.metadata.len() as u32;
        xfer.xfer_unsigned_int(&mut metadata_count)?;
        if xfer.is_writing() {
            let mut entries: Vec<(String, String)> = self
                .metadata
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));

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

        // Transfer data - write length then bytes
        let mut data_len = self.data.len() as u32;
        xfer.xfer_unsigned_int(&mut data_len)?;

        // Transfer the actual bytes.
        if xfer.is_reading() {
            self.data.resize(data_len as usize, 0);
        }

        if data_len > 0 {
            for byte in &mut self.data {
                xfer.xfer_unsigned_byte(byte)?;
            }
        }

        Ok(())
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new(1)
    }
}

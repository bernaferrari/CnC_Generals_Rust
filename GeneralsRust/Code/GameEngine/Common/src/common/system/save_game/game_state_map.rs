// FILE: save_game/game_state_map.rs ///////////////////////////////////////////
// Game state mapping functionality for save/load operations
///////////////////////////////////////////////////////////////////////////////

use super::game_state::GameState;
use crate::common::system::xfer::{Xfer, XferMode, XferStatus};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct GameStateMap {
    states: HashMap<String, GameState>,
    save_directory: PathBuf,
}

impl GameStateMap {
    pub fn new(save_directory: PathBuf) -> Self {
        Self {
            states: HashMap::new(),
            save_directory,
        }
    }

    pub fn add_state(&mut self, name: String, state: GameState) {
        self.states.insert(name, state);
    }

    pub fn get_state(&self, name: &str) -> Option<&GameState> {
        self.states.get(name)
    }

    pub fn get_state_mut(&mut self, name: &str) -> Option<&mut GameState> {
        self.states.get_mut(name)
    }

    pub fn remove_state(&mut self, name: &str) -> Option<GameState> {
        self.states.remove(name)
    }

    pub fn list_states(&self) -> Vec<&String> {
        self.states.keys().collect()
    }

    pub fn clear(&mut self) {
        self.states.clear();
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn save_directory(&self) -> &PathBuf {
        &self.save_directory
    }

    pub fn set_save_directory(&mut self, directory: PathBuf) {
        self.save_directory = directory;
    }

    /// Delete temporary `.map` scratch files from the save directory.
    /// Mirrors C++ `GameStateMap::clearScratchPadMaps` behavior.
    pub fn clear_scratch_pad_maps(&self) -> io::Result<usize> {
        let mut removed = 0usize;
        let entries = match fs::read_dir(&self.save_directory) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(err),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_map = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("map"))
                .unwrap_or(false);
            if is_map {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// Embed a pristine map file into the active xfer stream as a block payload.
    pub fn embed_pristine_map<X: Xfer>(&self, map_path: &Path, xfer: &mut X) -> io::Result<()> {
        let mut bytes = fs::read(map_path)?;
        xfer.begin_block().map_err(map_xfer_error)?;
        // SAFETY: bytes was just allocated with valid data
        unsafe { xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())? };
        xfer.end_block().map_err(map_xfer_error)?;
        Ok(())
    }

    /// Embed an already extracted in-use map into the active xfer stream.
    pub fn embed_in_use_map<X: Xfer>(&self, map_path: &Path, xfer: &mut X) -> io::Result<()> {
        self.embed_pristine_map(map_path, xfer)
    }

    /// Extract an embedded map block from the xfer stream and save it to disk.
    pub fn extract_and_save_map<X: Xfer>(&self, map_path: &Path, xfer: &mut X) -> io::Result<()> {
        let size = xfer.begin_block().map_err(map_xfer_error)?;
        if size < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "negative map block size",
            ));
        }

        let mut bytes = vec![0u8; size as usize];
        if !bytes.is_empty() {
            // SAFETY: bytes was just allocated with size elements
            unsafe { xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())? };
        }
        xfer.end_block().map_err(map_xfer_error)?;

        if let Some(parent) = map_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(map_path, bytes)?;
        Ok(())
    }

    /// Transfer a map file payload according to xfer mode.
    pub fn xfer_map_file<X: Xfer>(&self, map_path: &Path, xfer: &mut X) -> io::Result<()> {
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => self.embed_in_use_map(map_path, xfer),
            XferMode::Load => self.extract_and_save_map(map_path, xfer),
            XferMode::Invalid => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid xfer mode",
            )),
        }
    }
}

impl Default for GameStateMap {
    fn default() -> Self {
        Self::new(PathBuf::from("saves"))
    }
}

impl Drop for GameStateMap {
    fn drop(&mut self) {
        let _ = self.clear_scratch_pad_maps();
    }
}

fn map_xfer_error(status: XferStatus) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("xfer error: {status:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::system::xfer_load::XferLoad;
    use crate::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn clear_scratch_pad_maps_only_removes_map_files() {
        let dir = tempdir().expect("temp dir");
        let map_path = dir.path().join("scratch.map");
        let txt_path = dir.path().join("keep.txt");
        fs::write(&map_path, b"map").expect("write map");
        fs::write(&txt_path, b"text").expect("write txt");

        let gsm = GameStateMap::new(dir.path().to_path_buf());
        let removed = gsm.clear_scratch_pad_maps().expect("clear scratch maps");

        assert_eq!(removed, 1);
        assert!(!map_path.exists());
        assert!(txt_path.exists());
    }

    #[test]
    fn map_embed_extract_round_trip() {
        let dir = tempdir().expect("temp dir");
        let src_map = dir.path().join("source.map");
        let out_map = dir.path().join("out").join("loaded.map");
        let payload = b"test-map-payload-12345";
        fs::write(&src_map, payload).expect("write source map");

        let gsm = GameStateMap::new(dir.path().to_path_buf());
        let mut save_bytes = Vec::new();
        {
            let writer = Cursor::new(&mut save_bytes);
            let mut saver = XferSave::new(writer, 1);
            gsm.embed_pristine_map(&src_map, &mut saver)
                .expect("embed map");
        }

        {
            let reader = Cursor::new(&save_bytes);
            let mut loader = XferLoad::new(reader, 1);
            gsm.extract_and_save_map(&out_map, &mut loader)
                .expect("extract map");
        }

        let loaded = fs::read(out_map).expect("read extracted map");
        assert_eq!(loaded, payload);
    }
}

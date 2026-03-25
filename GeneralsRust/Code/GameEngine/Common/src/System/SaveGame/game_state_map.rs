// FILE: game_state_map.rs
// Author: Ported from C++ (Colin Day, October 2002)
// Desc: Chunk in the save game file that will hold a pristine version of the map file

use super::super::xfer::*;
use super::game_state::SaveCode;
use super::{
    get_game_state, get_runtime_drawable_id_counter, get_runtime_object_id_counter,
    notify_begin_load, notify_end_load, notify_get_game_mode, notify_get_skirmish_payload,
    notify_post_load_refresh, notify_set_game_mode, notify_set_loading_save,
    notify_set_skirmish_payload, notify_start_new_game_from_save,
    set_runtime_drawable_id_counter, set_runtime_object_id_counter,
};
use crate::common::ini::ini_game_data::get_global_data;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const GAME_SKIRMISH_MODE: i32 = 2;

// ------------------------------------------------------------------------------------------------
// GameStateMap - Manages map embedding in save files
// ------------------------------------------------------------------------------------------------
pub struct GameStateMap {
    save_directory: PathBuf,
}

impl GameStateMap {
    /// Create a new GameStateMap instance
    pub fn new(save_directory: PathBuf) -> Self {
        Self { save_directory }
    }

    /// Initialize
    pub fn init(&mut self) {
        // Nothing to initialize
    }

    /// Reset
    pub fn reset(&mut self) {
        // Nothing to reset
    }

    /// Update (no-op)
    pub fn update(&mut self) {
        // Nothing to update
    }

    /// Clear scratch pad maps from save directory
    pub fn clear_scratch_pad_maps(&self) -> Result<(), std::io::Error> {
        // Iterate directory and delete .map files
        let entries = std::fs::read_dir(&self.save_directory)?;

        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "map" {
                            std::fs::remove_file(path)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Embed a pristine map into the xfer stream
    fn embed_pristine_map(&self, map_path: &str, xfer: &mut dyn Xfer) -> Result<(), SaveCode> {
        // Open the map file
        let mut file = File::open(map_path).map_err(|_| {
            eprintln!(
                "embedPristineMap - Error opening source file '{}'",
                map_path
            );
            SaveCode::InvalidData
        })?;

        // Get file size
        let file_size = file.seek(SeekFrom::End(0)).map_err(|_| {
            eprintln!("embedPristineMap - Error seeking file '{}'", map_path);
            SaveCode::InvalidData
        })? as usize;

        // Rewind to beginning
        file.seek(SeekFrom::Start(0)).map_err(|_| {
            eprintln!("embedPristineMap - Error rewinding file '{}'", map_path);
            SaveCode::InvalidData
        })?;

        // Allocate buffer
        let mut buffer = vec![0u8; file_size];

        // Read entire file
        file.read_exact(&mut buffer).map_err(|_| {
            eprintln!("embedPristineMap - Error reading from file '{}'", map_path);
            SaveCode::InvalidData
        })?;

        // Write to xfer stream
        xfer.begin_block().map_err(|_| SaveCode::Error)?;
        // SAFETY: buffer was allocated with file_size bytes
        unsafe { xfer.xfer_user(buffer.as_mut_ptr(), file_size) }.map_err(|_| SaveCode::Error)?;
        xfer.end_block().map_err(|_| SaveCode::Error)?;

        Ok(())
    }

    /// Embed an "in use" map (already extracted from save) into xfer stream
    fn embed_in_use_map(&self, map_path: &str, xfer: &mut dyn Xfer) -> Result<(), SaveCode> {
        // Open the map file
        let mut file = File::open(map_path).map_err(|_| {
            eprintln!("embedInUseMap - Unable to open file '{}'", map_path);
            SaveCode::InvalidData
        })?;

        // Get file size
        let file_size = file.seek(SeekFrom::End(0)).map_err(|_| {
            eprintln!("embedInUseMap - Error seeking file '{}'", map_path);
            SaveCode::InvalidData
        })? as usize;

        // Rewind to beginning
        file.seek(SeekFrom::Start(0)).map_err(|_| {
            eprintln!("embedInUseMap - Error rewinding file '{}'", map_path);
            SaveCode::InvalidData
        })?;

        // Allocate buffer
        let mut buffer = vec![0u8; file_size];

        // Read entire file
        file.read_exact(&mut buffer).map_err(|_| {
            eprintln!("embedInUseMap - Error reading from file '{}'", map_path);
            SaveCode::InvalidData
        })?;

        // Embed into xfer stream
        xfer.begin_block().map_err(|_| SaveCode::Error)?;
        // SAFETY: buffer was allocated with file_size bytes
        unsafe { xfer.xfer_user(buffer.as_mut_ptr(), file_size) }.map_err(|_| SaveCode::Error)?;
        xfer.end_block().map_err(|_| SaveCode::Error)?;

        Ok(())
    }

    /// Extract map from xfer stream and save as file
    fn extract_and_save_map(&self, map_to_save: &str, xfer: &mut dyn Xfer) -> Result<(), SaveCode> {
        // Open output file
        let mut file = File::create(map_to_save).map_err(|_| {
            eprintln!("extractAndSaveMap - Unable to open file '{}'", map_to_save);
            SaveCode::InvalidData
        })?;

        // Read data size from file
        let data_size = xfer.begin_block().map_err(|_| SaveCode::Error)? as usize;

        // Allocate buffer
        let mut buffer = vec![0u8; data_size];

        // Read map file
        // SAFETY: buffer was allocated with data_size bytes
        unsafe { xfer.xfer_user(buffer.as_mut_ptr(), data_size) }.map_err(|_| SaveCode::Error)?;

        // Write to new file
        file.write_all(&buffer).map_err(|_| {
            eprintln!(
                "extractAndSaveMap - Error writing to file '{}'",
                map_to_save
            );
            SaveCode::InvalidData
        })?;

        // End block
        xfer.end_block().map_err(|_| SaveCode::Error)?;

        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshot implementation for GameStateMap
// ------------------------------------------------------------------------------------------------
impl Snapshot for GameStateMap {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        // Empty implementation
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let save_code_to_xfer = |code: SaveCode| match code {
            SaveCode::InvalidData => XferStatus::InvalidData,
            SaveCode::FileNotFound => XferStatus::FileNotFound,
            SaveCode::UnableToOpenFile => XferStatus::FileNotOpen,
            _ => XferStatus::ErrorUnknown,
        };
        let is_load = xfer.get_xfer_mode() == XferMode::Load;
        if is_load {
            let _ = self.clear_scratch_pad_maps();
            notify_begin_load();
            notify_set_loading_save(true);
        }

        let transfer_result = (|| {
            // Version
            let current_version: XferVersion = 2;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;

            let mut effective_game_mode = notify_get_game_mode().unwrap_or(0);
            let mut first_save = false;
            match xfer.get_xfer_mode() {
                XferMode::Save => {
                    let mut state = get_game_state();
                    let global = get_global_data()
                        .map(|data| data.read().map_name.clone())
                        .unwrap_or_default();

                    let map_leaf = state.get_map_leaf_name(&global);
                    let save_game_map_name = state
                        .get_file_path_in_save_directory(&map_leaf)
                        .to_string_lossy()
                        .to_string();
                    let mut portable = state.real_map_path_to_portable_map_path(&save_game_map_name);
                    xfer.xfer_ascii_string(&mut portable)?;

                    let mut pristine_map_name = String::new();
                    if !state.is_in_save_directory(Path::new(&global)) && !global.is_empty() {
                        pristine_map_name = global.clone();
                        first_save = true;
                    }
                    let mut pristine_portable =
                        state.real_map_path_to_portable_map_path(&pristine_map_name);
                    xfer.xfer_ascii_string(&mut pristine_portable)?;

                    {
                        let save_info = state.get_save_game_info_mut();
                        save_info.save_game_map_name = save_game_map_name.clone();
                        save_info.pristine_map_name = pristine_map_name.clone();
                    }

                    if version >= 2 {
                        // Game mode
                        let mut game_mode: i32 = notify_get_game_mode().unwrap_or(effective_game_mode);
                        xfer.xfer_int(&mut game_mode)?;
                        effective_game_mode = game_mode;
                    }

                    if first_save {
                        self.embed_pristine_map(&pristine_map_name, xfer)
                            .map_err(save_code_to_xfer)?;
                    } else {
                        self.embed_in_use_map(&save_game_map_name, xfer)
                            .map_err(save_code_to_xfer)?;
                    }
                }
                XferMode::Load => {
                    // Read save game map name
                    let mut save_game_map_name = String::new();
                    xfer.xfer_ascii_string(&mut save_game_map_name)?;

                    // Read pristine map filename
                    let mut pristine_map_name = String::new();
                    xfer.xfer_ascii_string(&mut pristine_map_name)?;

                    {
                        let mut state = get_game_state();
                        let real_save = state.portable_map_path_to_real_map_path(&save_game_map_name);
                        let real_pristine =
                            state.portable_map_path_to_real_map_path(&pristine_map_name);
                        let save_game_map_name = real_save.clone();
                        {
                            let save_info = state.get_save_game_info_mut();
                            save_info.save_game_map_name = save_game_map_name.clone();
                            save_info.pristine_map_name = real_pristine;
                        }

                        if !state.is_in_save_directory(Path::new(&save_game_map_name)) {
                            eprintln!(
                                "GameStateMap::xfer - The map filename read from the file '{}' is not in the SAVE directory, but should be",
                                save_game_map_name
                            );
                            return Err(XferStatus::InvalidData);
                        }

                        if let Some(global) = get_global_data() {
                            global.write().map_name = save_game_map_name.clone();
                        }
                    }

                    if version >= 2 {
                        // Game mode
                        let mut game_mode: i32 = 0;
                        xfer.xfer_int(&mut game_mode)?;
                        effective_game_mode = game_mode;
                        notify_set_game_mode(game_mode);
                    }

                    let save_map_path = {
                        let state = get_game_state();
                        state.get_save_game_info().save_game_map_name.clone()
                    };
                    self.extract_and_save_map(&save_map_path, xfer)
                        .map_err(save_code_to_xfer)?;
                }
                _ => {
                    return Err(XferStatus::ModeUnknown);
                }
            }

            // Object ID counter
            let mut high_object_id: ObjectID = if xfer.get_xfer_mode() == XferMode::Save {
                get_runtime_object_id_counter().unwrap_or(1)
            } else {
                1
            };
            xfer.xfer_object_id(&mut high_object_id)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                set_runtime_object_id_counter(high_object_id);
            }

            // Drawable ID counter
            let mut high_drawable_id: DrawableID = if xfer.get_xfer_mode() == XferMode::Save {
                get_runtime_drawable_id_counter().unwrap_or(1)
            } else {
                1
            };
            xfer.xfer_drawable_id(&mut high_drawable_id)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                set_runtime_drawable_id_counter(high_drawable_id);
                notify_start_new_game_from_save();
                notify_post_load_refresh();
            }

            if effective_game_mode == GAME_SKIRMISH_MODE {
                let mut payload = if xfer.get_xfer_mode() == XferMode::Save {
                    notify_get_skirmish_payload().unwrap_or_default()
                } else {
                    Vec::new()
                };
                let mut payload_len = payload.len() as u32;
                xfer.xfer_unsigned_int(&mut payload_len)?;
                if xfer.get_xfer_mode() == XferMode::Load {
                    payload.resize(payload_len as usize, 0);
                }
                if payload_len > 0 {
                    // SAFETY: payload buffer is allocated with at least `payload_len` bytes.
                    unsafe { xfer.xfer_user(payload.as_mut_ptr(), payload_len as usize)? };
                }
                if xfer.get_xfer_mode() == XferMode::Load {
                    notify_set_skirmish_payload(Some(payload));
                }
            } else if xfer.get_xfer_mode() == XferMode::Load {
                notify_set_skirmish_payload(None);
            }
            Ok(())
        })();

        if is_load {
            notify_set_loading_save(false);
            notify_end_load();
        }

        transfer_result
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        // Empty implementation
        Ok(())
    }
}

impl Drop for GameStateMap {
    fn drop(&mut self) {
        // Clear scratch pad maps on destruction
        let _ = self.clear_scratch_pad_maps();
    }
}

// ------------------------------------------------------------------------------------------------
// Helper functions for map path manipulation
// ------------------------------------------------------------------------------------------------

/// Get map leaf and directory name
fn get_map_leaf_and_dir_name(path: &str) -> String {
    let path_obj = Path::new(path);

    // Get parent and file name
    if let (Some(parent), Some(filename)) = (path_obj.parent(), path_obj.file_name()) {
        if let Some(_grandparent) = parent.parent() {
            // Have something like: maps\foo\foo.map
            let parent_name = parent.file_name().unwrap_or_default();
            format!(
                "{}\\{}",
                parent_name.to_str().unwrap_or(""),
                filename.to_str().unwrap_or("")
            )
        } else {
            // Have something like: save\foo.map
            path.to_string()
        }
    } else {
        path.to_string()
    }
}

/// Remove extension from filename
fn remove_extension(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(path)
        .to_string()
}

/// Constants for portable paths
pub const PORTABLE_SAVE: &str = "Save\\";
pub const PORTABLE_MAPS: &str = "Maps\\";
pub const PORTABLE_USER_MAPS: &str = "UserData\\Maps\\";

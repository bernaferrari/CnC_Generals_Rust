// FILE: game_state_map.rs
// Author: Ported from C++ (Colin Day, October 2002)
// Desc: Chunk in the save game file that will hold a pristine version of the map file

use super::super::xfer::*;
use super::super::xfer_load::XferLoad;
use super::super::xfer_save::XferSave;
use super::game_state::SaveCode;
use super::{
    get_game_state, get_runtime_drawable_id_counter, get_runtime_object_id_counter,
    notify_begin_load, notify_end_load, notify_get_game_mode, notify_get_skirmish_payload,
    notify_post_load_refresh, notify_set_game_mode, notify_set_loading_save,
    notify_set_skirmish_payload, notify_start_new_game_from_save, set_runtime_drawable_id_counter,
    set_runtime_object_id_counter,
};
use crate::common::ini::ini_game_data::get_global_data;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const GAME_SKIRMISH_MODE: i32 = 2;

/// C++ `GameInfo.h` / `NetworkDefs.h`: `MAX_SLOTS = MAX_PLAYER+1 = 8`.
const SKIRMISH_MAX_SLOTS: i32 = 8;
const SKIRMISH_GAME_INFO_VERSION: XferVersion = 4;

/// C++ `SkirmishGameInfo::xfer` (GameInfo.cpp:1488-1588) field layout.
/// Host/UI hooks still exchange the same snapshot bytes; we no longer wrap
/// them in a Rust-only `u32` length prefix.
#[derive(Clone)]
struct SkirmishGameInfoSnapshot {
    preorder_mask: i32,
    crc_interval: i32,
    in_game: bool,
    in_progress: bool,
    surrendered: bool,
    game_id: i32,
    slots: [SkirmishSlotSnapshot; SKIRMISH_MAX_SLOTS as usize],
    local_ip: u32,
    map_name: String,
    map_crc: u32,
    map_size: u32,
    map_mask: i32,
    seed: i32,
    superweapon_restriction: u16,
    starting_cash: u32,
}

#[derive(Clone, Default)]
struct SkirmishSlotSnapshot {
    state: i32,
    name: String,
    is_accepted: bool,
    is_muted: bool,
    color: i32,
    start_pos: i32,
    player_template: i32,
    team_number: i32,
    orig_color: i32,
    orig_start_pos: i32,
    orig_player_template: i32,
}

impl Default for SkirmishGameInfoSnapshot {
    fn default() -> Self {
        Self {
            preorder_mask: 0,
            crc_interval: 0,
            in_game: false,
            in_progress: false,
            surrendered: false,
            game_id: 0,
            slots: Default::default(),
            local_ip: 0,
            map_name: String::new(),
            map_crc: 0,
            map_size: 0,
            map_mask: 0,
            seed: 0,
            superweapon_restriction: 0,
            starting_cash: 0,
        }
    }
}

impl Snapshot for SkirmishGameInfoSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        // GameInfo.cpp:1490-1492
        let mut version = SKIRMISH_GAME_INFO_VERSION;
        xfer.xfer_version(&mut version, SKIRMISH_GAME_INFO_VERSION)?;

        xfer.xfer_int(&mut self.preorder_mask)?;
        xfer.xfer_int(&mut self.crc_interval)?;
        xfer.xfer_bool(&mut self.in_game)?;
        xfer.xfer_bool(&mut self.in_progress)?;
        xfer.xfer_bool(&mut self.surrendered)?;
        xfer.xfer_int(&mut self.game_id)?;

        let mut slot_count = SKIRMISH_MAX_SLOTS;
        xfer.xfer_int(&mut slot_count)?;
        let slots_to_xfer = slot_count.clamp(0, SKIRMISH_MAX_SLOTS) as usize;
        for slot in self.slots.iter_mut().take(slots_to_xfer) {
            xfer.xfer_int(&mut slot.state)?;
            if version >= 2 {
                xfer.xfer_unicode_string(&mut slot.name)?;
            }
            xfer.xfer_bool(&mut slot.is_accepted)?;
            xfer.xfer_bool(&mut slot.is_muted)?;
            xfer.xfer_int(&mut slot.color)?;
            xfer.xfer_int(&mut slot.start_pos)?;
            xfer.xfer_int(&mut slot.player_template)?;
            xfer.xfer_int(&mut slot.team_number)?;
            xfer.xfer_int(&mut slot.orig_color)?;
            xfer.xfer_int(&mut slot.orig_start_pos)?;
            xfer.xfer_int(&mut slot.orig_player_template)?;
        }

        xfer.xfer_unsigned_int(&mut self.local_ip)?;
        // System Xfer has no xfer_map_name; C++ xferMapName is ascii + portable
        // conversion, which GameState already applied to hook payloads.
        xfer.xfer_ascii_string(&mut self.map_name)?;
        xfer.xfer_unsigned_int(&mut self.map_crc)?;
        xfer.xfer_unsigned_int(&mut self.map_size)?;
        xfer.xfer_int(&mut self.map_mask)?;
        xfer.xfer_int(&mut self.seed)?;

        if version >= 3 {
            xfer.xfer_unsigned_short(&mut self.superweapon_restriction)?;
            if version == 3 {
                let mut obsolete = false;
                xfer.xfer_bool(&mut obsolete)?;
            }
            // C++ `xfer->xferSnapshot(&m_startingCash)` → Money.cpp v1 + u32.
            let mut money_version: XferVersion = 1;
            xfer.xfer_version(&mut money_version, 1)?;
            xfer.xfer_unsigned_int(&mut self.starting_cash)?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.superweapon_restriction = 0;
            self.starting_cash = 0;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

fn unique_skirmish_scratch_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "skirmish_xfer_{}_{}_{}.bin",
        label,
        std::process::id(),
        stamp
    ))
}

fn encode_skirmish_snapshot(info: &SkirmishGameInfoSnapshot) -> Vec<u8> {
    let path = unique_skirmish_scratch_path("enc");
    let mut copy = info.clone();
    {
        let mut xfer = XferSave::new();
        if xfer.open(path.to_string_lossy().into_owned()).is_err() {
            return Vec::new();
        }
        if copy.xfer(&mut xfer).is_err() {
            let _ = xfer.close();
            let _ = std::fs::remove_file(&path);
            return Vec::new();
        }
        let _ = xfer.close();
    }
    let bytes = std::fs::read(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    bytes
}

fn try_decode_skirmish_snapshot(bytes: &[u8]) -> Option<SkirmishGameInfoSnapshot> {
    // Only accept a C++ SkirmishGameInfo stream (GameInfo.cpp:1490, version 4).
    // Legacy hook blobs that started with a u32 length must not be parsed as v1.
    if bytes.first().copied() != Some(SKIRMISH_GAME_INFO_VERSION) || bytes.len() < 16 {
        return None;
    }
    let path = unique_skirmish_scratch_path("dec");
    std::fs::write(&path, bytes).ok()?;
    let mut info = SkirmishGameInfoSnapshot::default();
    let decoded = {
        let mut xfer = XferLoad::new();
        if xfer.open(path.to_string_lossy().into_owned()).is_err() {
            None
        } else {
            let ok = info.xfer(&mut xfer).is_ok();
            let _ = xfer.close();
            ok.then_some(info)
        }
    };
    let _ = std::fs::remove_file(&path);
    decoded
}

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
                    let mut portable =
                        state.real_map_path_to_portable_map_path(&save_game_map_name);
                    xfer.xfer_ascii_string(&mut portable)?;

                    // C++ GameStateMap.cpp:260-283 — keep the previously stored pristine
                    // path unless the current map is outside the save directory.
                    let mut pristine_map_name = {
                        let save_info = state.get_save_game_info();
                        save_info.pristine_map_name.clone()
                    };
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
                        let mut game_mode: i32 =
                            notify_get_game_mode().unwrap_or(effective_game_mode);
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
                        let real_save =
                            state.portable_map_path_to_real_map_path(&save_game_map_name);
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
            }

            if effective_game_mode == GAME_SKIRMISH_MODE {
                // C++ GameStateMap.cpp:396-406 — `xfer->xferSnapshot(TheSkirmishGameInfo)`.
                let mut info = SkirmishGameInfoSnapshot::default();
                if xfer.get_xfer_mode() == XferMode::Save {
                    if let Some(payload) = notify_get_skirmish_payload() {
                        if let Some(decoded) = try_decode_skirmish_snapshot(&payload) {
                            info = decoded;
                        }
                    }
                }
                xfer.xfer_snapshot(&mut info)?;
                if xfer.get_xfer_mode() == XferMode::Load {
                    notify_set_skirmish_payload(Some(encode_skirmish_snapshot(&info)));
                }
            } else {
                // C++ GameStateMap.cpp:408-414 — delete TheSkirmishGameInfo whenever
                // the mode is not skirmish, on both save and load.
                notify_set_skirmish_payload(None);
            }

            if xfer.get_xfer_mode() == XferMode::Load {
                notify_start_new_game_from_save();
                notify_post_load_refresh();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::System::SaveGame::{
        init_game_state, register_drawable_id_counter_hooks, register_object_id_counter_hooks,
        register_save_load_lifecycle_hooks, register_save_load_skirmish_hooks,
    };
    use crate::System::{XferLoad, XferSave};
    use crate::common::ini::ini_game_data::{get_global_data, init_global_data};
    use std::fs;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_guard() -> &'static Mutex<()> {
        static TEST_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_GUARD.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_save_dir(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before UNIX_EPOCH")
            .as_nanos();
        path.push(format!(
            "game_state_map_test_{}_{}_{}",
            label,
            std::process::id(),
            stamp
        ));
        fs::create_dir_all(&path).expect("create temp save dir");
        path
    }

    fn register_noop_id_hooks() {
        register_object_id_counter_hooks(Some(Arc::new(|| 1)), Some(Arc::new(|_| {})));
        register_drawable_id_counter_hooks(Some(Arc::new(|| 1)), Some(Arc::new(|_| {})));
    }

    fn push_event(log: &Arc<Mutex<Vec<String>>>, event: &str) {
        log.lock().expect("event log lock").push(event.to_string());
    }

    #[test]
    fn load_refreshes_new_game_after_skirmish_payload() {
        let _guard = test_guard().lock().expect("test lock");
        let save_dir = unique_temp_save_dir("load_refresh");
        let map_path = save_dir.join("FrozenValley.map");
        fs::write(&map_path, b"dummy map payload").expect("write dummy map");

        init_global_data();
        if let Some(global) = get_global_data() {
            global.write().map_name = map_path.to_string_lossy().to_string();
        }
        init_game_state(save_dir.clone());

        let event_log = Arc::new(Mutex::new(Vec::<String>::new()));

        register_save_load_lifecycle_hooks(
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move || push_event(&event_log, "begin_load")
            })),
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move || push_event(&event_log, "end_load")
            })),
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move |loading| {
                    push_event(
                        &event_log,
                        if loading {
                            "loading_save_true"
                        } else {
                            "loading_save_false"
                        },
                    )
                }
            })),
            Some(Arc::new(|| GAME_SKIRMISH_MODE)),
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move |mode| {
                    push_event(
                        &event_log,
                        if mode == GAME_SKIRMISH_MODE {
                            "set_game_mode_skirmish"
                        } else {
                            "set_game_mode_other"
                        },
                    )
                }
            })),
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move || push_event(&event_log, "start_new_game")
            })),
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move || push_event(&event_log, "post_load_refresh")
            })),
        );

        register_save_load_skirmish_hooks(
            Some(Arc::new(|| Some(vec![1, 2, 3, 4]))),
            Some(Arc::new({
                let event_log = Arc::clone(&event_log);
                move |_| push_event(&event_log, "set_skirmish_payload")
            })),
        );

        register_noop_id_hooks();

        let mut game_state_map = GameStateMap::new(save_dir.clone());
        let save_path = save_dir.join("00000001.sav");

        {
            let mut xfer_save = XferSave::new();
            xfer_save
                .open(save_path.to_string_lossy().into_owned())
                .expect("open save file");
            game_state_map
                .xfer(&mut xfer_save)
                .expect("save game state map");
            xfer_save.close().expect("close save file");
        }

        event_log.lock().expect("event log lock").clear();

        {
            let mut xfer_load = XferLoad::new();
            xfer_load
                .open(save_path.to_string_lossy().into_owned())
                .expect("open load file");
            game_state_map
                .xfer(&mut xfer_load)
                .expect("load game state map");
            xfer_load.close().expect("close load file");
        }

        let events = event_log.lock().expect("event log lock").clone();
        let payload_idx = events
            .iter()
            .position(|event| event == "set_skirmish_payload")
            .expect("skirmish payload event");
        let start_idx = events
            .iter()
            .position(|event| event == "start_new_game")
            .expect("start_new_game event");
        let refresh_idx = events
            .iter()
            .position(|event| event == "post_load_refresh")
            .expect("post_load_refresh event");

        assert!(payload_idx < start_idx);
        assert!(start_idx < refresh_idx);

        let _ = fs::remove_dir_all(save_dir);
    }

    #[test]
    fn skirmish_branch_writes_versioned_snapshot_not_u32_len_prefix() {
        // C++ GameStateMap.cpp:396-406 xfers TheSkirmishGameInfo as a Snapshot
        // (GameInfo.cpp:1488 starts with XferVersion=4). Pre-fix Rust wrote
        // u32 payload_len + raw bytes, which C++ cannot parse.
        let mut info = SkirmishGameInfoSnapshot::default();
        info.seed = 0x11;
        info.map_name = "AlpineAssault.map".to_string();
        let encoded = encode_skirmish_snapshot(&info);
        assert_eq!(
            encoded.first().copied(),
            Some(SKIRMISH_GAME_INFO_VERSION),
            "C++ SkirmishGameInfo::xfer starts with version byte 4"
        );
        assert!(
            encoded.len() > 4,
            "snapshot must be the versioned field stream, not u32-len + 0 bytes"
        );
        let decoded = try_decode_skirmish_snapshot(&encoded).expect("decode versioned snapshot");
        assert_eq!(decoded.seed, 0x11);
        assert_eq!(decoded.map_name, "AlpineAssault.map");
        assert!(try_decode_skirmish_snapshot(&[1, 2, 3, 4]).is_none());
    }

    #[test]
    fn production_hook_v4_bytes_populate_skirmish_snapshot() {
        // hq-6rwbw: hook payload must be GameInfo.cpp:1488 v4 xfer, not bincode.
        let mut live = crate::System::ChallengeGameInfoXfer::default();
        live.seed = 0x5EED;
        live.map_name = "UserData\\Maps\\Custom\\Custom.map".to_string();
        live.starting_cash = 12_000;
        live.slots[0].name = "Host".to_string();
        live.slots[0].state = 5;
        let hook_bytes = live.encode_xfer_bytes();
        assert_eq!(
            hook_bytes.first().copied(),
            Some(SKIRMISH_GAME_INFO_VERSION)
        );
        let decoded = try_decode_skirmish_snapshot(&hook_bytes)
            .expect("GameStateMap must accept the same v4 bytes the live hook emits");
        assert_eq!(decoded.seed, 0x5EED);
        assert_eq!(decoded.map_name, "UserData\\Maps\\Custom\\Custom.map");
        assert_eq!(decoded.starting_cash, 12_000);
        assert_eq!(decoded.slots[0].name, "Host");
        assert_eq!(decoded.slots[0].state, 5);
        assert!(
            try_decode_skirmish_snapshot(&bincode_like_blob()).is_none(),
            "bincode hook blobs must not decode as SkirmishGameInfo v4"
        );
    }

    fn bincode_like_blob() -> Vec<u8> {
        // bincode structs typically do not start with xfer version byte 4
        // plus a valid GameInfo field stream.
        vec![0x00, 0x01, 0x02, 0x03, 0x10, 0x00, 0x00, 0x00]
    }
}

// ------------------------------------------------------------------------------------------------
// Helper functions for map path manipulation
// ------------------------------------------------------------------------------------------------

/// Get map leaf and directory name
#[allow(dead_code)]
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
#[allow(dead_code)]
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

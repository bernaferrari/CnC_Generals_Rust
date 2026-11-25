// FILE: game_state_map.rs
// Author: Ported from C++ (Colin Day, October 2002)
// Desc: Chunk in the save game file that will hold a pristine version of the map file

use super::super::xfer::*;
use super::super::xfer_load::XferLoad;
use super::super::xfer_save::XferSave;
use super::game_state::{GameState, SaveCode};
use super::{
    get_game_state, get_runtime_drawable_id_counter, get_runtime_object_id_counter,
    notify_begin_load, notify_end_load, notify_get_game_mode, notify_get_skirmish_payload,
    notify_post_load_refresh, notify_set_game_mode, notify_set_loading_save,
    notify_set_skirmish_payload, notify_start_new_game_from_save, set_runtime_drawable_id_counter,
    set_runtime_object_id_counter,
};
use crate::common::ini::ini_game_data::get_global_data;
use std::path::{Path, PathBuf};

const GAME_SKIRMISH_MODE: i32 = 2;

const SKIRMISH_MAX_SLOTS: i32 = 8;
const SKIRMISH_GAME_INFO_VERSION: XferVersion = 4;
use super::campaign_xfer::{
    ChallengeGameInfoXfer as SkirmishGameInfoSnapshot, ChallengeSlotXfer as SkirmishSlotSnapshot,
};
impl Snapshot for SkirmishGameInfoSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        SkirmishGameInfoSnapshot::xfer(self, xfer)
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
/// The map fields which must be installed before the world starts from a save.
pub struct SavedMapState {
    pub map_name: String,
    pub game_mode: i32,
    pub next_object_id: ObjectID,
    pub next_drawable_id: DrawableID,
    pub skirmish: Option<SkirmishGameInfoSnapshot>,
}

/// GameStateMap.cpp:371–430. Map IO produces these values and the driving
/// match applies them before starting the extracted map.
pub trait SavedMapRuntime {
    fn capture_saved_map(&mut self) -> Result<SavedMapState, XferStatus>;
    fn begin_saved_map_load(&mut self);
    fn start_saved_map(&mut self, saved: SavedMapState) -> Result<(), XferStatus>;
    fn end_saved_map_load(&mut self);

    fn save_lock_ghost_objects(&mut self, enable: bool);
    fn finish_saved_game_load(&mut self) -> Result<(), XferStatus>;
    fn abort_saved_game_load(&mut self);
}

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
        Self::clear_scratch_pad_maps_in(&self.save_directory)
    }

    fn clear_scratch_pad_maps_in(directory: &Path) -> Result<(), std::io::Error> {
        let entries = std::fs::read_dir(directory)?;

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

    fn embed_map(map_path: &str, xfer: &mut dyn Xfer) -> Result<(), SaveCode> {
        let mut buffer = std::fs::read(map_path).map_err(|error| {
            eprintln!("Unable to embed map {map_path:?}: {error}");
            SaveCode::InvalidData
        })?;
        xfer.begin_block().map_err(|_| SaveCode::Error)?;
        xfer.xfer_user(&mut buffer).map_err(|_| SaveCode::Error)?;
        xfer.end_block().map_err(|_| SaveCode::Error)?;
        Ok(())
    }

    /// Extract map from xfer stream and save as file
    fn extract_and_save_map(map_to_save: &str, xfer: &mut dyn Xfer) -> Result<(), SaveCode> {
        let data_size = xfer.begin_block().map_err(|_| SaveCode::Error)?;
        let data_size = usize::try_from(data_size).map_err(|_| SaveCode::InvalidData)?;
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(data_size).map_err(|_| SaveCode::InvalidData)?;
        buffer.resize(data_size, 0);
        xfer.xfer_user(&mut buffer).map_err(|_| SaveCode::Error)?;
        xfer.end_block().map_err(|_| SaveCode::Error)?;
        std::fs::write(map_to_save, buffer).map_err(|error| {
            eprintln!("Unable to extract map {map_to_save:?}: {error}");
            SaveCode::InvalidData
        })
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
        // Only safe outside the GameState save/load block loop: the loop holds
        // THE_GAME_STATE's mutex and dispatches xfer_with_state instead.
        let mut state = get_game_state();
        self.xfer_with_state(xfer, &mut state)
    }

    fn xfer_with_state(
        &mut self,
        xfer: &mut dyn Xfer,
        state: &mut GameState,
    ) -> Result<(), XferStatus> {
        Self::xfer_with_runtime(xfer, state, &mut ActiveMapHooks)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        // Empty implementation
        Ok(())
    }
}

impl GameStateMap {
    pub(super) fn xfer_with_runtime(
        xfer: &mut dyn Xfer,
        state: &mut GameState,
        runtime: &mut dyn SavedMapRuntime,
    ) -> Result<(), XferStatus> {
        let is_load = xfer.get_xfer_mode() == XferMode::Load;
        if is_load {
            let _ = Self::clear_scratch_pad_maps_in(state.get_save_directory());
            runtime.begin_saved_map_load();
        }

        let result = Self::transfer_map(xfer, state, runtime);
        if is_load {
            runtime.end_saved_map_load();
        }
        result
    }

    fn transfer_map(
        xfer: &mut dyn Xfer,
        state: &mut GameState,
        runtime: &mut dyn SavedMapRuntime,
    ) -> Result<(), XferStatus> {
        let current_version = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut saved = match xfer.get_xfer_mode() {
            XferMode::Save => {
                let mut saved = runtime.capture_saved_map()?;
                let map_leaf = state.get_map_leaf_name(&saved.map_name);
                let saved_path = state.get_file_path_in_save_directory(&map_leaf);
                let saved_path = saved_path.to_string_lossy();
                let first_save = !saved.map_name.is_empty()
                    && !state.is_in_save_directory(Path::new(&saved.map_name));
                let pristine_path = if first_save {
                    saved.map_name.clone()
                } else {
                    state.get_save_game_info().pristine_map_name.clone()
                };
                xfer.xfer_ascii_string(&mut state.real_map_path_to_portable_map_path(&saved_path))?;
                xfer.xfer_ascii_string(
                    &mut state.real_map_path_to_portable_map_path(&pristine_path),
                )?;
                let info = state.get_save_game_info_mut();
                info.save_game_map_name = saved_path.to_string();
                info.pristine_map_name = pristine_path;

                xfer.xfer_int(&mut saved.game_mode)?;
                let input = if first_save { &saved.map_name } else { saved_path.as_ref() };
                Self::embed_map(input, xfer).map_err(Self::map_xfer_error)?;
                saved
            }
            XferMode::Load => {
                let mut saved_path = String::new();
                let mut pristine_path = String::new();
                xfer.xfer_ascii_string(&mut saved_path)?;
                xfer.xfer_ascii_string(&mut pristine_path)?;
                let map_name = state.portable_map_path_to_real_map_path(&saved_path);
                let pristine_path = state.portable_map_path_to_real_map_path(&pristine_path);
                if !state.is_in_save_directory(Path::new(&map_name)) {
                    eprintln!("Embedded map path {map_name:?} is outside the save directory");
                    return Err(XferStatus::InvalidData);
                }
                let info = state.get_save_game_info_mut();
                info.save_game_map_name.clone_from(&map_name);
                info.pristine_map_name = pristine_path;
                let mut game_mode = 0;
                xfer.xfer_int(&mut game_mode)?;
                Self::extract_and_save_map(&map_name, xfer).map_err(Self::map_xfer_error)?;
                SavedMapState {
                    map_name, game_mode, next_object_id: 1, next_drawable_id: 1, skirmish: None,
                }
            }
            _ => return Err(XferStatus::ModeUnknown),
        };

        xfer.xfer_object_id(&mut saved.next_object_id)?;
        xfer.xfer_drawable_id(&mut saved.next_drawable_id)?;
        if saved.game_mode == GAME_SKIRMISH_MODE {
            let skirmish = saved.skirmish.get_or_insert_with(Default::default);
            xfer.xfer_snapshot(skirmish)?;
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            runtime.start_saved_map(saved)?;
        }
        Ok(())
    }

    fn map_xfer_error(code: SaveCode) -> XferStatus {
        match code {
            SaveCode::InvalidData => XferStatus::InvalidData,
            SaveCode::FileNotFound => XferStatus::FileNotFound,
            SaveCode::UnableToOpenFile => XferStatus::FileNotOpen,
            _ => XferStatus::ErrorUnknown,
        }
    }
}

struct ActiveMapHooks;

impl SavedMapRuntime for ActiveMapHooks {
    fn capture_saved_map(&mut self) -> Result<SavedMapState, XferStatus> {
        let game_mode = notify_get_game_mode().unwrap_or(0);
        let skirmish = if game_mode == GAME_SKIRMISH_MODE {
            notify_get_skirmish_payload().as_deref().and_then(try_decode_skirmish_snapshot)
        } else {
            notify_set_skirmish_payload(None);
            None
        };
        Ok(SavedMapState {
            map_name: get_global_data().map(|data| data.read().map_name.clone()).unwrap_or_default(),
            game_mode,
            next_object_id: get_runtime_object_id_counter().unwrap_or(1),
            next_drawable_id: get_runtime_drawable_id_counter().unwrap_or(1),
            skirmish,
        })
    }

    fn begin_saved_map_load(&mut self) {
        notify_begin_load();
        notify_set_loading_save(true);
    }

    fn start_saved_map(&mut self, saved: SavedMapState) -> Result<(), XferStatus> {
        if let Some(data) = get_global_data() {
            data.write().map_name = saved.map_name;
        }
        notify_set_game_mode(saved.game_mode);
        set_runtime_object_id_counter(saved.next_object_id);
        set_runtime_drawable_id_counter(saved.next_drawable_id);
        notify_set_skirmish_payload(saved.skirmish.as_ref().map(encode_skirmish_snapshot));
        notify_start_new_game_from_save();
        notify_post_load_refresh();
        Ok(())
    }

    fn end_saved_map_load(&mut self) {
        notify_set_loading_save(false);
        notify_end_load();
    }

    fn save_lock_ghost_objects(&mut self, enable: bool) {
        super::notify_save_lock_ghost_objects(enable);
    }

    fn finish_saved_game_load(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }

    fn abort_saved_game_load(&mut self) {}
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
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    // THREAD: test-local guard; the repo convention runs tests with
    // `--test-threads=1`, so a per-thread cell replaces the old process mutex.
    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        struct TestGuardSlot(std::sync::Mutex<()>);
        static TEST_GUARD: std::sync::LazyLock<TestGuardSlot> =
            std::sync::LazyLock::new(|| TestGuardSlot(std::sync::Mutex::new(())));
        TEST_GUARD.0.lock().unwrap_or_else(|p| p.into_inner())
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
        let _guard = test_guard();
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

        // Build a stored-v1 fixture from the production v2 writer, changing only
        // GameStateMap's leading version byte. C++ gates its mode field on the
        // current implementation version (2), not the stored version, so v1
        // saves still contain this field before the embedded map block.
        let mut v1_bytes = fs::read(&save_path).expect("read production v2 save");
        assert_eq!(v1_bytes.first(), Some(&2), "writer stores GameStateMap v2");
        v1_bytes[0] = 1;
        let v1_save_path = save_dir.join("stored_v1.sav");
        fs::write(&v1_save_path, v1_bytes).expect("write stored-v1 fixture");

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

        // The current v2 stream above remains the normal round-trip control.
        // Loading the stored-v1 stream through the same production xfer path
        // must consume game mode before extracting the following map block.
        event_log.lock().expect("event log lock").clear();
        {
            let mut xfer_load = XferLoad::new();
            xfer_load
                .open(v1_save_path.to_string_lossy().into_owned())
                .expect("open stored-v1 load file");
            game_state_map
                .xfer(&mut xfer_load)
                .expect("load stored-v1 game state map");
            xfer_load.close().expect("close stored-v1 load file");
        }
        assert_eq!(
            fs::read(&map_path).expect("read extracted map"),
            b"dummy map payload"
        );
        assert!(
            event_log
                .lock()
                .expect("event log lock")
                .iter()
                .any(|event| event == "set_game_mode_skirmish")
        );

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

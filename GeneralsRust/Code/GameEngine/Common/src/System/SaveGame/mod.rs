// FILE: mod.rs (SaveGame module)
// Author: Ported from C++
// Desc: SaveGame subsystem module exports

use super::xfer::{DrawableID, ObjectID};
use crate::common::ini::ini_game_data::get_global_data;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

pub mod game_state;
pub mod game_state_map;

pub use game_state::{
    AvailableGameInfo, GameState, SaveCode, SaveDate, SaveFileType, SaveGameInfo,
    SaveLoadLayoutType, SnapshotType,
};

pub use game_state_map::{GameStateMap, PORTABLE_MAPS, PORTABLE_SAVE, PORTABLE_USER_MAPS};

static THE_GAME_STATE: OnceLock<Mutex<GameState>> = OnceLock::new();
type ObjectIdCounterGetter = Arc<dyn Fn() -> ObjectID + Send + Sync>;
type ObjectIdCounterSetter = Arc<dyn Fn(ObjectID) + Send + Sync>;
type DrawableIdCounterGetter = Arc<dyn Fn() -> DrawableID + Send + Sync>;
type DrawableIdCounterSetter = Arc<dyn Fn(DrawableID) + Send + Sync>;
type BeginLoadHook = Arc<dyn Fn() + Send + Sync>;
type EndLoadHook = Arc<dyn Fn() + Send + Sync>;
type SetLoadingSaveHook = Arc<dyn Fn(bool) + Send + Sync>;
type GetGameModeHook = Arc<dyn Fn() -> i32 + Send + Sync>;
type SetGameModeHook = Arc<dyn Fn(i32) + Send + Sync>;
type StartNewGameFromSaveHook = Arc<dyn Fn() + Send + Sync>;
type PostLoadRefreshHook = Arc<dyn Fn() + Send + Sync>;
type GetSkirmishPayloadHook = Arc<dyn Fn() -> Option<Vec<u8>> + Send + Sync>;
type SetSkirmishPayloadHook = Arc<dyn Fn(Option<Vec<u8>>) + Send + Sync>;
type ClearGameDataHook = Arc<dyn Fn() + Send + Sync>;
type MissionStartArgsHook = Arc<dyn Fn() -> (i32, i32) + Send + Sync>;

#[derive(Default)]
struct RuntimeIdCounterHooks {
    object_getter: Option<ObjectIdCounterGetter>,
    object_setter: Option<ObjectIdCounterSetter>,
    drawable_getter: Option<DrawableIdCounterGetter>,
    drawable_setter: Option<DrawableIdCounterSetter>,
}

static RUNTIME_ID_COUNTER_HOOKS: OnceLock<Mutex<RuntimeIdCounterHooks>> = OnceLock::new();

#[derive(Default)]
struct SaveLoadLifecycleHooks {
    begin_load: Option<BeginLoadHook>,
    end_load: Option<EndLoadHook>,
    set_loading_save: Option<SetLoadingSaveHook>,
    get_game_mode: Option<GetGameModeHook>,
    set_game_mode: Option<SetGameModeHook>,
    start_new_game_from_save: Option<StartNewGameFromSaveHook>,
    post_load_refresh: Option<PostLoadRefreshHook>,
    get_skirmish_payload: Option<GetSkirmishPayloadHook>,
    set_skirmish_payload: Option<SetSkirmishPayloadHook>,
    clear_game_data: Option<ClearGameDataHook>,
    mission_start_args: Option<MissionStartArgsHook>,
}

static SAVE_LOAD_LIFECYCLE_HOOKS: OnceLock<Mutex<SaveLoadLifecycleHooks>> = OnceLock::new();

fn id_counter_hooks() -> &'static Mutex<RuntimeIdCounterHooks> {
    RUNTIME_ID_COUNTER_HOOKS.get_or_init(|| Mutex::new(RuntimeIdCounterHooks::default()))
}

fn save_load_hooks() -> &'static Mutex<SaveLoadLifecycleHooks> {
    SAVE_LOAD_LIFECYCLE_HOOKS.get_or_init(|| Mutex::new(SaveLoadLifecycleHooks::default()))
}

fn default_save_directory() -> PathBuf {
    let user_dir = get_global_data()
        .map(|data| data.read().get_path_user_data().to_string())
        .unwrap_or_default();
    let mut base = if user_dir.is_empty() {
        "UserData/".to_string()
    } else {
        user_dir
    };
    if !base.ends_with('/') && !base.ends_with('\\') {
        base.push('/');
    }
    base.push_str("Save");
    PathBuf::from(base)
}

pub fn get_game_state() -> std::sync::MutexGuard<'static, GameState> {
    let lock = THE_GAME_STATE.get_or_init(|| {
        let mut state = GameState::new(default_save_directory());
        state.init();
        Mutex::new(state)
    });
    lock.lock().expect("TheGameState mutex poisoned")
}

pub fn init_game_state(save_directory: PathBuf) {
    let lock = THE_GAME_STATE.get_or_init(|| Mutex::new(GameState::new(save_directory.clone())));
    let mut state = lock.lock().expect("TheGameState mutex poisoned");
    state.reset_for_init(save_directory);
    state.init();
}

pub fn register_object_id_counter_hooks(
    getter: Option<ObjectIdCounterGetter>,
    setter: Option<ObjectIdCounterSetter>,
) {
    if let Ok(mut hooks) = id_counter_hooks().lock() {
        hooks.object_getter = getter;
        hooks.object_setter = setter;
    }
}

pub fn register_drawable_id_counter_hooks(
    getter: Option<DrawableIdCounterGetter>,
    setter: Option<DrawableIdCounterSetter>,
) {
    if let Ok(mut hooks) = id_counter_hooks().lock() {
        hooks.drawable_getter = getter;
        hooks.drawable_setter = setter;
    }
}

pub fn register_save_load_lifecycle_hooks(
    begin_load: Option<BeginLoadHook>,
    end_load: Option<EndLoadHook>,
    set_loading_save: Option<SetLoadingSaveHook>,
    get_game_mode: Option<GetGameModeHook>,
    set_game_mode: Option<SetGameModeHook>,
    start_new_game_from_save: Option<StartNewGameFromSaveHook>,
    post_load_refresh: Option<PostLoadRefreshHook>,
) {
    if let Ok(mut hooks) = save_load_hooks().lock() {
        hooks.begin_load = begin_load;
        hooks.end_load = end_load;
        hooks.set_loading_save = set_loading_save;
        hooks.get_game_mode = get_game_mode;
        hooks.set_game_mode = set_game_mode;
        hooks.start_new_game_from_save = start_new_game_from_save;
        hooks.post_load_refresh = post_load_refresh;
    }
}

pub fn register_save_load_mission_hooks(
    clear_game_data: Option<ClearGameDataHook>,
    mission_start_args: Option<MissionStartArgsHook>,
) {
    if let Ok(mut hooks) = save_load_hooks().lock() {
        hooks.clear_game_data = clear_game_data;
        hooks.mission_start_args = mission_start_args;
    }
}

pub fn register_save_load_skirmish_hooks(
    get_skirmish_payload: Option<GetSkirmishPayloadHook>,
    set_skirmish_payload: Option<SetSkirmishPayloadHook>,
) {
    if let Ok(mut hooks) = save_load_hooks().lock() {
        hooks.get_skirmish_payload = get_skirmish_payload;
        hooks.set_skirmish_payload = set_skirmish_payload;
    }
}

pub(crate) fn notify_begin_load() {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.begin_load.as_ref() {
            callback();
        }
    }
}

pub(crate) fn notify_end_load() {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.end_load.as_ref() {
            callback();
        }
    }
}

pub(crate) fn notify_set_loading_save(loading: bool) {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.set_loading_save.as_ref() {
            callback(loading);
        }
    }
}

pub(crate) fn notify_get_game_mode() -> Option<i32> {
    let hooks = save_load_hooks().lock().ok()?;
    hooks.get_game_mode.as_ref().map(|callback| callback())
}

pub(crate) fn notify_set_game_mode(game_mode: i32) {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.set_game_mode.as_ref() {
            callback(game_mode);
        }
    }
}

pub(crate) fn notify_start_new_game_from_save() {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.start_new_game_from_save.as_ref() {
            callback();
        }
    }
}

pub(crate) fn notify_post_load_refresh() {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.post_load_refresh.as_ref() {
            callback();
        }
    }
}

pub(crate) fn notify_clear_game_data() {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.clear_game_data.as_ref() {
            callback();
        }
    }
}

pub(crate) fn notify_get_mission_start_args() -> Option<(i32, i32)> {
    let hooks = save_load_hooks().lock().ok()?;
    hooks
        .mission_start_args
        .as_ref()
        .map(|callback| callback())
}

pub(crate) fn notify_get_skirmish_payload() -> Option<Vec<u8>> {
    let hooks = save_load_hooks().lock().ok()?;
    hooks
        .get_skirmish_payload
        .as_ref()
        .and_then(|callback| callback())
}

pub(crate) fn notify_set_skirmish_payload(payload: Option<Vec<u8>>) {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.set_skirmish_payload.as_ref() {
            callback(payload);
        }
    }
}

pub fn get_runtime_object_id_counter() -> Option<ObjectID> {
    let hooks = id_counter_hooks().lock().ok()?;
    hooks.object_getter.as_ref().map(|getter| getter())
}

pub fn set_runtime_object_id_counter(counter: ObjectID) {
    if let Ok(hooks) = id_counter_hooks().lock() {
        if let Some(setter) = hooks.object_setter.as_ref() {
            setter(counter);
        }
    }
}

pub fn get_runtime_drawable_id_counter() -> Option<DrawableID> {
    let hooks = id_counter_hooks().lock().ok()?;
    hooks.drawable_getter.as_ref().map(|getter| getter())
}

pub fn set_runtime_drawable_id_counter(counter: DrawableID) {
    if let Ok(hooks) = id_counter_hooks().lock() {
        if let Some(setter) = hooks.drawable_setter.as_ref() {
            setter(counter);
        }
    }
}

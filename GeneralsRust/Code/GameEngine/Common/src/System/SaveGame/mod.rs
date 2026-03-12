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

#[derive(Default)]
struct RuntimeIdCounterHooks {
    object_getter: Option<ObjectIdCounterGetter>,
    object_setter: Option<ObjectIdCounterSetter>,
    drawable_getter: Option<DrawableIdCounterGetter>,
    drawable_setter: Option<DrawableIdCounterSetter>,
}

static RUNTIME_ID_COUNTER_HOOKS: OnceLock<Mutex<RuntimeIdCounterHooks>> = OnceLock::new();

fn id_counter_hooks() -> &'static Mutex<RuntimeIdCounterHooks> {
    RUNTIME_ID_COUNTER_HOOKS.get_or_init(|| Mutex::new(RuntimeIdCounterHooks::default()))
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
    *state = GameState::new(save_directory);
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

// FILE: mod.rs (SaveGame module)
// Author: Ported from C++
// Desc: SaveGame subsystem module exports

use super::xfer::{DrawableID, ObjectID};
use crate::common::ini::ini_game_data::get_global_data;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod campaign_xfer;
pub mod game_state;
pub mod game_state_map;

pub use game_state::{
    AvailableGameInfo, BorrowedSnapshotBlock, GameState, SAVELOAD_BLOCK_NAMES, SaveCode, SaveDate,
    SaveFileType, SaveGameInfo, SaveLoadLayoutType, SnapshotType, format_mission_save_description,
    register_partition_manager_update, register_world_dict_map_name,
};

pub use campaign_xfer::{
    CHALLENGE_MAX_SLOTS, CampaignManagerXferState, ChallengeGameInfoXfer, ChallengeSlotXfer,
};
pub use game_state_map::{GameStateMap, PORTABLE_MAPS, PORTABLE_SAVE, PORTABLE_USER_MAPS};

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
type SaveLockGhostObjectsHook = Arc<dyn Fn(bool) + Send + Sync>;
/// (campaign_name_label key, mission number, current campaign map). `None` = no campaign.
type CampaignSnapshotHook = Arc<dyn Fn() -> Option<(String, i32, String)> + Send + Sync>;
type FetchGameTextHook = Arc<dyn Fn(&str) -> String + Send + Sync>;
type PostUiMessageHook = Arc<dyn Fn(&str) + Send + Sync>;
type MessageBoxOkHook = Arc<dyn Fn(&str, &str) + Send + Sync>;

#[derive(Default)]
pub(crate) struct RuntimeIdCounterHooks {
    object_getter: Option<ObjectIdCounterGetter>,
    object_setter: Option<ObjectIdCounterSetter>,
    drawable_getter: Option<DrawableIdCounterGetter>,
    drawable_setter: Option<DrawableIdCounterSetter>,
}

#[derive(Default)]
pub(crate) struct SaveLoadLifecycleHooks {
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
    save_lock_ghost_objects: Option<SaveLockGhostObjectsHook>,
    get_campaign_snapshot: Option<CampaignSnapshotHook>,
    fetch_game_text: Option<FetchGameTextHook>,
    post_ui_message: Option<PostUiMessageHook>,
    message_box_ok: Option<MessageBoxOkHook>,
}

fn id_counter_hooks() -> Arc<Mutex<RuntimeIdCounterHooks>> {
    Arc::clone(
        &crate::common::common_services::common_services()
            .system
            .save_id_counter_hooks,
    )
}

fn save_load_hooks() -> Arc<Mutex<SaveLoadLifecycleHooks>> {
    Arc::clone(
        &crate::common::common_services::common_services()
            .system
            .save_load_lifecycle_hooks,
    )
}

pub(crate) fn default_save_directory() -> PathBuf {
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

pub fn get_game_state() -> crate::common::owned_lock::OwnedMutexGuard<GameState> {
    let services = crate::common::common_services::common_services();
    services.system.game_state.lock_arc()
}

pub fn init_game_state(save_directory: PathBuf) {
    let services = crate::common::common_services::common_services();
    let mut state = services.system.game_state.lock();
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

pub fn register_save_lock_ghost_objects_hook(
    save_lock_ghost_objects: Option<SaveLockGhostObjectsHook>,
) {
    if let Ok(mut hooks) = save_load_hooks().lock() {
        hooks.save_lock_ghost_objects = save_lock_ghost_objects;
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
pub fn register_save_load_campaign_hooks(get_campaign_snapshot: Option<CampaignSnapshotHook>) {
    if let Ok(mut hooks) = save_load_hooks().lock() {
        hooks.get_campaign_snapshot = get_campaign_snapshot;
    }
}

/// Wire GameText + InGameUI/MessageBox paths used by `GameState::save_game` /
/// `mission_save` (C++ GameState.cpp:548-596, 616-618).
pub fn register_save_game_ui_hooks(
    fetch_game_text: Option<FetchGameTextHook>,
    post_ui_message: Option<PostUiMessageHook>,
    message_box_ok: Option<MessageBoxOkHook>,
) {
    if let Ok(mut hooks) = save_load_hooks().lock() {
        hooks.fetch_game_text = fetch_game_text;
        hooks.post_ui_message = post_ui_message;
        hooks.message_box_ok = message_box_ok;
    }
}

pub(crate) fn notify_fetch_game_text(key: &str) -> String {
    let hooks_arc = save_load_hooks();
    if let Ok(hooks) = hooks_arc.lock() {
        if let Some(fetch) = hooks.fetch_game_text.as_ref() {
            return fetch(key);
        }
    }
    // Fall back to the map-cache GameText provider when UI hooks are unset.
    crate::common::ini::ini_map_cache::lookup_game_text(key).unwrap_or_else(|| key.to_string())
}

pub(crate) fn notify_post_ui_message(text_or_label: &str) {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(post) = hooks.post_ui_message.as_ref() {
            post(text_or_label);
        }
    }
}

pub(crate) fn notify_message_box_ok(title: &str, body: &str) {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(mbox) = hooks.message_box_ok.as_ref() {
            mbox(title, body);
        }
    }
}

pub(crate) fn notify_get_campaign_snapshot() -> Option<(String, i32, String)> {
    let hooks_arc = save_load_hooks();
    let hooks = hooks_arc.lock().ok()?;
    hooks
        .get_campaign_snapshot
        .as_ref()
        .and_then(|callback| callback())
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
    let hooks_arc = save_load_hooks();
    let hooks = hooks_arc.lock().ok()?;
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
    let hooks_arc = save_load_hooks();
    let hooks = hooks_arc.lock().ok()?;
    hooks.mission_start_args.as_ref().map(|callback| callback())
}

pub(crate) fn notify_save_lock_ghost_objects(enable: bool) {
    if let Ok(hooks) = save_load_hooks().lock() {
        if let Some(callback) = hooks.save_lock_ghost_objects.as_ref() {
            callback(enable);
        }
    }
}

pub(crate) fn notify_get_skirmish_payload() -> Option<Vec<u8>> {
    let hooks_arc = save_load_hooks();
    let hooks = hooks_arc.lock().ok()?;
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
    let hooks_arc = id_counter_hooks();
    let hooks = hooks_arc.lock().ok()?;
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
    let hooks_arc = id_counter_hooks();
    let hooks = hooks_arc.lock().ok()?;
    hooks.drawable_getter.as_ref().map(|getter| getter())
}

pub fn set_runtime_drawable_id_counter(counter: DrawableID) {
    if let Ok(hooks) = id_counter_hooks().lock() {
        if let Some(setter) = hooks.drawable_setter.as_ref() {
            setter(counter);
        }
    }
}

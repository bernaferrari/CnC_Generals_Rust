//! C++ `TheChallengeGameInfo` (`ChallengeMenu.cpp`).
//!
//! Separate `SkirmishGameInfo` from `TheSkirmishGameInfo`. Challenge init/play/save
//! and GameLogic bind for challenge campaigns use this pointer, not skirmish setup.

use game_network::{GameSlot, SkirmishGameInfo, SlotState};
use std::sync::{Mutex, MutexGuard, OnceLock};

static CHALLENGE_GAME_INFO: OnceLock<Mutex<Option<SkirmishGameInfo>>> = OnceLock::new();

fn store() -> &'static Mutex<Option<SkirmishGameInfo>> {
    CHALLENGE_GAME_INFO.get_or_init(|| Mutex::new(None))
}

fn lock_store() -> MutexGuard<'static, Option<SkirmishGameInfo>> {
    store().lock().unwrap_or_else(|e| e.into_inner())
}

/// C++ `TheChallengeGameInfo == NULL`.
pub fn challenge_game_info_exists() -> bool {
    lock_store().is_some()
}

/// Allocate + init like C++ ChallengeMenuInit / ScoreScreen next-mission.
pub fn ensure_challenge_game_info() -> MutexGuard<'static, Option<SkirmishGameInfo>> {
    let mut guard = lock_store();
    if guard.is_none() {
        let mut info = SkirmishGameInfo::new();
        info.game_info_mut().init();
        info.game_info_mut().clear_slot_list();
        info.game_info_mut().reset();
        info.game_info_mut().enter_game();
        *guard = Some(info);
    }
    guard
}

/// C++ delete TheChallengeGameInfo.
pub fn clear_challenge_game_info() {
    *lock_store() = None;
}

pub fn with_challenge_game_info_mut<R>(f: impl FnOnce(&mut SkirmishGameInfo) -> R) -> Option<R> {
    let mut guard = lock_store();
    guard.as_mut().map(f)
}

pub fn with_challenge_game_info<R>(f: impl FnOnce(&SkirmishGameInfo) -> R) -> Option<R> {
    let guard = lock_store();
    guard.as_ref().map(f)
}

/// Init empty challenge session (ChallengeMenuInit).
pub fn init_challenge_game_info() {
    let mut info = SkirmishGameInfo::new();
    info.game_info_mut().init();
    info.game_info_mut().clear_slot_list();
    info.game_info_mut().reset();
    info.game_info_mut().enter_game();
    *lock_store() = Some(info);
}

pub fn set_challenge_slot0_and_map(map: String, display_name: String, template_num: i32) {
    let mut guard = ensure_challenge_game_info();
    if let Some(info) = guard.as_mut() {
        let gi = info.game_info_mut();
        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, display_name, 0);
        slot.set_player_template(template_num);
        gi.set_slot(0, slot);
        gi.set_map(map);
    }
}

pub fn snapshot_map_and_template() -> (String, i32) {
    with_challenge_game_info(|info| {
        let gi = info.game_info();
        let map = gi.get_map().to_string();
        let template = gi
            .get_slot(0)
            .map(|s| s.get_player_template())
            .unwrap_or(-1);
        (map, template)
    })
    .unwrap_or_default()
}

pub fn restore_map_and_template(map: String, template: i32) {
    if map.is_empty() && template < 0 {
        clear_challenge_game_info();
        return;
    }
    set_challenge_slot0_and_map(map, String::new(), template);
}

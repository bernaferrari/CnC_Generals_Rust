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

/// C++ `xferSnapshot(TheChallengeGameInfo)` capture.
pub fn snapshot_challenge_game_info() -> game_engine::System::ChallengeGameInfoXfer {
    use game_engine::System::{CHALLENGE_MAX_SLOTS, ChallengeGameInfoXfer, ChallengeSlotXfer};
    with_challenge_game_info(|info| {
        let gi = info.game_info();
        let mut slots = std::array::from_fn(|_| ChallengeSlotXfer::default());
        for i in 0..CHALLENGE_MAX_SLOTS {
            if let Some(slot) = gi.get_slot(i) {
                slots[i] = ChallengeSlotXfer {
                    state: slot.get_state() as i32,
                    name: slot.get_name().to_string(),
                    is_accepted: slot.is_accepted(),
                    is_muted: slot.is_muted(),
                    color: slot.get_color(),
                    start_pos: slot.get_start_pos(),
                    player_template: slot.get_player_template(),
                    team_number: slot.get_team_number(),
                    orig_color: slot.get_original_color(),
                    orig_start_pos: slot.get_original_start_pos(),
                    orig_player_template: slot.get_original_player_template(),
                };
            }
        }
        let mut preorder_mask = 0;
        for i in 0..CHALLENGE_MAX_SLOTS {
            if gi.is_player_preorder(i) {
                preorder_mask |= 1 << i;
            }
        }
        ChallengeGameInfoXfer {
            preorder_mask,
            crc_interval: gi.get_crc_interval(),
            in_game: gi.is_in_game(),
            in_progress: gi.is_game_in_progress(),
            surrendered: gi.have_we_surrendered(),
            game_id: gi.get_game_id(),
            slots,
            local_ip: gi.get_local_ip(),
            map_name: gi.get_map().to_string(),
            map_crc: gi.get_map_crc(),
            map_size: gi.get_map_size(),
            map_mask: gi.get_map_contents_mask(),
            seed: gi.get_seed(),
            superweapon_restriction: gi.get_superweapon_restriction(),
            starting_cash: gi.get_starting_cash().count_money(),
        }
    })
    .unwrap_or_default()
}

/// C++ load path: allocate TheChallengeGameInfo then apply SkirmishGameInfo::xfer.
pub fn restore_challenge_game_info(snapshot: game_engine::System::ChallengeGameInfoXfer) {
    use game_engine::System::CHALLENGE_MAX_SLOTS;
    use game_network::{GameSlot, Money, SlotState};
    let mut guard = ensure_challenge_game_info();
    let Some(info) = guard.as_mut() else {
        return;
    };
    let gi = info.game_info_mut();
    gi.reset();
    gi.set_crc_interval(snapshot.crc_interval);
    if snapshot.in_game {
        gi.set_in_game();
    }
    gi.set_game_in_progress(snapshot.in_progress);
    if snapshot.surrendered {
        gi.mark_as_surrendered();
    }
    if snapshot.in_progress {
        gi.start_game(snapshot.game_id);
    }
    gi.set_local_ip(snapshot.local_ip);
    gi.set_map(snapshot.map_name);
    gi.set_map_crc(snapshot.map_crc);
    gi.set_map_size(snapshot.map_size);
    gi.set_map_contents_mask(snapshot.map_mask);
    gi.set_seed(snapshot.seed);
    gi.set_superweapon_restriction(snapshot.superweapon_restriction);
    let mut cash = Money::new(0);
    cash.init();
    cash.deposit(snapshot.starting_cash);
    gi.set_starting_cash(cash);
    for i in 0..CHALLENGE_MAX_SLOTS {
        if snapshot.preorder_mask & (1 << i) != 0 {
            gi.mark_player_as_preorder(i);
        }
        let src = &snapshot.slots[i];
        let state = match src.state {
            0 => SlotState::Open,
            1 => SlotState::Closed,
            2 => SlotState::EasyAI,
            3 => SlotState::MedAI,
            4 => SlotState::BrutalAI,
            5 => SlotState::Player,
            _ => SlotState::Closed,
        };
        let mut slot = GameSlot::new();
        slot.set_state(state, src.name.clone(), 0);
        if src.is_accepted {
            slot.set_accept();
        }
        slot.mute(src.is_muted);
        slot.set_player_template(src.orig_player_template);
        slot.set_start_pos(src.orig_start_pos);
        slot.set_color(src.orig_color);
        slot.save_off_original_info();
        slot.set_team_number(src.team_number);
        slot.set_color(src.color);
        slot.set_start_pos(src.start_pos);
        slot.set_player_template(src.player_template);
        gi.set_slot(i, slot);
    }
}

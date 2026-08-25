//! Shared skirmish setup state for menu coordination.

use game_engine::System::{CHALLENGE_MAX_SLOTS, ChallengeGameInfoXfer, ChallengeSlotXfer};
use game_network::{GameSlot, Money, SkirmishGameInfo, SlotState};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Default)]
pub struct SkirmishSetup {
    selected_map: String,
    use_system_maps: bool,
    game_info: SkirmishGameInfo,
}

impl SkirmishSetup {
    pub fn selected_map(&self) -> &str {
        &self.selected_map
    }

    pub fn set_selected_map(&mut self, map: String) {
        self.selected_map = map;
    }

    pub fn use_system_maps(&self) -> bool {
        self.use_system_maps
    }

    pub fn set_use_system_maps(&mut self, value: bool) {
        self.use_system_maps = value;
    }

    pub fn game_info(&self) -> &SkirmishGameInfo {
        &self.game_info
    }

    pub fn game_info_mut(&mut self) -> &mut SkirmishGameInfo {
        &mut self.game_info
    }
}

static SKIRMISH_SETUP: OnceLock<Mutex<SkirmishSetup>> = OnceLock::new();

pub fn get_skirmish_setup() -> std::sync::MutexGuard<'static, SkirmishSetup> {
    SKIRMISH_SETUP
        .get_or_init(|| Mutex::new(SkirmishSetup::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Live lobby → C++ `SkirmishGameInfo::xfer` v4 fields (GameInfo.cpp:1488).
pub fn snapshot_skirmish_lobby() -> ChallengeGameInfoXfer {
    let setup = get_skirmish_setup();
    let gi = setup.game_info().game_info();
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
}

/// C++ GameStateMap.cpp:406 `xferSnapshot(TheSkirmishGameInfo)` restore.
pub fn restore_skirmish_lobby(payload: Option<Vec<u8>>) {
    let mut setup = get_skirmish_setup();
    if let Some(bytes) = payload {
        let Some(snapshot) = ChallengeGameInfoXfer::decode_xfer_bytes(&bytes) else {
            return;
        };
        let gi = setup.game_info_mut().game_info_mut();
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
    } else {
        *setup.game_info_mut() = SkirmishGameInfo::default();
    }
}

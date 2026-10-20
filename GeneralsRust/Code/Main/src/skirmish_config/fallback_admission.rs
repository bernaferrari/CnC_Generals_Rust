//! Receiving fallback GameInfo lifecycle. Existing GameInfo's TLS publication
//! is unsuitable here; the plain original v4 record remains the sole save wire.
use super::*;
use game_engine::common::system::skirmish_game_info::{SkirmishGameInfo, SkirmishSlot};
mod map_metadata;

struct FallbackGameInfo {
    record: SkirmishGameInfo,
    local_slot: i32,
}
impl FallbackGameInfo {
    fn enter(config: &SkirmishMatchConfig) -> Self {
        // CPP GameInfo.cpp279–308/reset,370–376/enterGame. These flags are
        // transitions of this owner, not reconstructed facts about another match.
        let mut record = SkirmishGameInfo::default();
        record.crc_interval = 100; // CPP NET_CRC_INTERVAL, GameInfo::reset.
        record.in_game = true;
        record.map_name = config.map.clone();
        record.seed = config.random_seed as i32;
        record.starting_cash = config.rules.starting_cash.max(0) as u32;
        record.superweapon_restriction = u16::from(config.rules.limit_superweapons);
        record.slots = std::array::from_fn(|_| observe_slot(&game_client::GameSlot::new()));
        Self {
            record,
            local_slot: -1,
        }
    }
    fn start(mut self) -> (SkirmishGameInfo, i32) {
        // CPP384/startGame: default offline gameID0, close open slots, progress.
        for slot in &mut self.record.slots {
            if slot.state == 0 {
                slot.state = 1;
            }
        }
        self.record.in_progress = true;
        (self.record, self.local_slot)
    }
}
fn observe_slot(slot: &game_client::GameSlot) -> SkirmishSlot {
    SkirmishSlot {
        state: slot.get_state() as i32,
        name: slot.get_name().into(),
        is_accepted: slot.is_accepted(),
        is_muted: slot.is_muted(),
        color: slot.get_color(),
        start_pos: slot.get_start_pos(),
        player_template: slot.get_player_template(),
        team_number: slot.get_team_number(),
        orig_color: slot.get_original_color(),
        orig_start_pos: slot.get_original_start_pos(),
        orig_player_template: slot.get_original_player_template(),
    }
}
pub(super) fn observe_start(
    config: &SkirmishMatchConfig,
    menu_slots: &[GameSlot],
) -> Option<(SkirmishGameInfo, i32)> {
    // Direct legacy fixtures have no authored selection; this is not an RGB inverse.
    for slot in config.slots.iter().filter(|slot| slot.is_active) {
        menu_slots
            .iter()
            .find(|s| s.slot_index == slot.slot_index)?
            .color
            .authored_index()?;
    }
    let mut owner = FallbackGameInfo::enter(config);
    let metadata = {
        let cache = game_client::map_util::get_map_cache_manager();
        let cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.find_map(&config.map)
    };
    if let Some(metadata) = metadata.as_ref() {
        owner.record.map_crc = metadata.crc;
        owner.record.map_size = metadata.filesize;
    }
    owner.record.map_mask = map_metadata::observe_contents(&config.map, metadata.is_some());
    for slot in &config.slots {
        let authored = menu_slots
            .iter()
            .find(|s| s.slot_index == slot.slot_index)?;
        let mut destination = game_client::GameSlot::new();
        let state = if !slot.is_active {
            game_client::SlotState::Closed
        } else if slot.is_human {
            game_client::SlotState::Player
        } else {
            match slot.ai_difficulty.as_deref() {
                Some("Easy") => game_client::SlotState::EasyAI,
                Some("Hard" | "Brutal") => game_client::SlotState::BrutalAI,
                _ => game_client::SlotState::MedAI,
            }
        };
        // Offline slot IP remains the actual default0; no network address is invented.
        destination.set_state(state, slot.player_name.clone(), 0);
        if let Some(index) = authored.color.authored_index() {
            destination.set_color(index);
        }
        destination.set_start_pos(slot.start_position);
        destination.set_team_number(slot.team);
        destination.set_player_template(match slot.player_template {
            SkirmishPlayerTemplateSelection::Exact { template_index, .. } => template_index,
            SkirmishPlayerTemplateSelection::Observer => -2,
            SkirmishPlayerTemplateSelection::Random => -1,
        });
        destination.save_off_original_info();
        *owner.record.slots.get_mut(slot.slot_index)? = observe_slot(&destination);
        if slot.is_active && slot.is_human && owner.local_slot < 0 {
            owner.local_slot = slot.slot_index as i32;
        }
    }
    Some(owner.start())
}

//! C++ `TheChallengeGameInfo` (`ChallengeMenu.cpp`).
//!
//! The mutable value is owned by the receiving `CampaignManager`; this module
//! contains only operations over an explicitly borrowed value.

use game_network::{GameSlot, SkirmishGameInfo, SlotState};

pub fn challenge_game_info_exists(info: &Option<SkirmishGameInfo>) -> bool {
    info.is_some()
}

fn new_challenge_game_info() -> SkirmishGameInfo {
    let mut info = SkirmishGameInfo::new();
    info.game_info_mut().init();
    info.game_info_mut().clear_slot_list();
    info.game_info_mut().reset();
    info.game_info_mut().enter_game();
    info
}

pub fn ensure_challenge_game_info(info: &mut Option<SkirmishGameInfo>) -> &mut SkirmishGameInfo {
    info.get_or_insert_with(new_challenge_game_info)
}

pub fn clear_challenge_game_info(info: &mut Option<SkirmishGameInfo>) {
    *info = None;
}

pub fn with_challenge_game_info_mut<R>(
    info: &mut Option<SkirmishGameInfo>,
    f: impl FnOnce(&mut SkirmishGameInfo) -> R,
) -> Option<R> {
    info.as_mut().map(f)
}

pub fn with_challenge_game_info<R>(
    info: Option<&SkirmishGameInfo>,
    f: impl FnOnce(&SkirmishGameInfo) -> R,
) -> Option<R> {
    info.map(f)
}

pub fn init_challenge_game_info(info: &mut Option<SkirmishGameInfo>) {
    *info = Some(new_challenge_game_info());
}

pub fn set_challenge_slot0_and_map(
    info: &mut Option<SkirmishGameInfo>,
    map: String,
    display_name: String,
    template_num: i32,
) {
    let game = ensure_challenge_game_info(info);
    let gi = game.game_info_mut();
    let mut slot = GameSlot::new();
    slot.set_state(SlotState::Player, display_name, 0);
    slot.set_player_template(template_num);
    gi.set_slot(0, slot);
    gi.set_map(map);
}

pub fn snapshot_map_and_template(info: Option<&SkirmishGameInfo>) -> (String, i32) {
    with_challenge_game_info(info, |info| {
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

pub fn restore_map_and_template(info: &mut Option<SkirmishGameInfo>, map: String, template: i32) {
    if map.is_empty() && template < 0 {
        clear_challenge_game_info(info);
    } else {
        set_challenge_slot0_and_map(info, map, String::new(), template);
    }
}

/// C++ `xferSnapshot(TheChallengeGameInfo)` capture.
pub fn snapshot_challenge_game_info(
    info: Option<&SkirmishGameInfo>,
) -> game_engine::System::ChallengeGameInfoXfer {
    use game_engine::System::{CHALLENGE_MAX_SLOTS, ChallengeGameInfoXfer, ChallengeSlotXfer};
    let Some(info) = info else {
        return ChallengeGameInfoXfer::default();
    };
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
}

/// C++ load path: allocate receiving TheChallengeGameInfo then apply Xfer state.
pub fn restore_challenge_game_info(
    info: &mut Option<SkirmishGameInfo>,
    snapshot: game_engine::System::ChallengeGameInfoXfer,
) {
    use game_engine::System::CHALLENGE_MAX_SLOTS;
    use game_network::Money;
    let game = ensure_challenge_game_info(info);
    let gi = game.game_info_mut();
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

#[cfg(test)]
mod tests {
    #[test]
    fn challenge_game_info_is_owned_by_each_campaign_manager() {
        let mut first = crate::gui::campaign_manager::CampaignManager::new();
        let mut second = crate::gui::campaign_manager::CampaignManager::new();
        first.set_challenge_slot0_and_map("maps/first.map".into(), "First".into(), 2);
        second.set_challenge_slot0_and_map("maps/second.map".into(), "Second".into(), 5);

        assert_eq!(
            first.challenge_game_info().unwrap().game_info().get_map(),
            "maps/first.map"
        );
        assert_eq!(
            second.challenge_game_info().unwrap().game_info().get_map(),
            "maps/second.map"
        );
        first.clear_challenge_game_info();
        assert!(first.challenge_game_info().is_none());
        assert!(second.challenge_game_info().is_some());
    }
}

//! GameSpy game state helpers (GameInfo + SetGameOptions sync).

use std::sync::{Mutex, OnceLock};

use game_engine::common::ascii_string::AsciiString;
use game_network::gamespy::peer_defs::{get_gamespy_info, GameSpyStagingRoom};
use game_network::gamespy::peer_thread::{get_peer_message_queue, PeerRequest, PeerRequestType};
use game_network::{
    game_info_to_ascii_string, GameInfo, SlotState, MAX_SLOTS, PLAYERTEMPLATE_OBSERVER,
};

static GAMESPY_GAME_INFO: OnceLock<Mutex<GameInfo>> = OnceLock::new();

fn gamespy_game_info() -> &'static Mutex<GameInfo> {
    GAMESPY_GAME_INFO.get_or_init(|| Mutex::new(GameInfo::new()))
}

pub fn with_gamespy_game_info<R>(f: impl FnOnce(&GameInfo) -> R) -> R {
    let guard = gamespy_game_info()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    f(&guard)
}

pub fn with_gamespy_game_info_mut<R>(f: impl FnOnce(&mut GameInfo) -> R) -> R {
    let mut guard = gamespy_game_info()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

pub fn push_gamespy_game_options() -> bool {
    let (options, map_name, player_names, use_stats, slot_states, slot_templates, slot_colors) =
        with_gamespy_game_info(|info| {
            let options = game_info_to_ascii_string(info);
            let map_name = info.get_map().to_string().replace('\\', "/");
            let use_stats = info.get_use_stats() != 0;
            let mut names = std::array::from_fn(|_| String::new());
            let mut states = [SlotState::Closed; MAX_SLOTS];
            let mut templates = [0; MAX_SLOTS];
            let mut colors = [0; MAX_SLOTS];
            for i in 0..MAX_SLOTS {
                if let Some(slot) = info.get_slot(i) {
                    states[i] = slot.get_state();
                    templates[i] = slot.get_player_template();
                    colors[i] = slot.get_color();
                    names[i] = match slot.get_state() {
                        SlotState::EasyAI => "CE".to_string(),
                        SlotState::MedAI => "CM".to_string(),
                        SlotState::BrutalAI => "CH".to_string(),
                        SlotState::Player => slot.get_name().to_string(),
                        _ => String::new(),
                    };
                }
            }
            (
                options, map_name, names, use_stats, states, templates, colors,
            )
        });

    let player_info_map = get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| guard.get_player_info_map().clone())
        })
        .unwrap_or_default();

    if let Some(info) = get_gamespy_info().and_then(|info| info.lock().ok()) {
        if let Some(room) = info.get_current_staging_room().cloned() {
            let mut updated = room;
            updated.map_name = AsciiString::from(&map_name);
            updated.use_stats = use_stats;
            updated.num_players = 0;
            updated.num_observers = 0;
            updated.max_players = 0;
            for i in 0..MAX_SLOTS {
                updated.player_names[i] = AsciiString::from(&player_names[i]);
                updated.slot_profiles[i] = 0;
                updated.slot_wins[i] = 0;
                updated.slot_losses[i] = 0;
                updated.slot_faction[i] = slot_templates[i];
                updated.slot_color[i] = slot_colors[i];
                match slot_states[i] {
                    SlotState::Open => {
                        updated.max_players += 1;
                    }
                    SlotState::Player => {
                        let key = player_names[i].to_lowercase();
                        if let Some(player) = player_info_map.get(&key) {
                            updated.slot_profiles[i] = player.profile_id;
                            updated.slot_wins[i] = player.wins;
                            updated.slot_losses[i] = player.losses;
                        }
                        if slot_templates[i] == PLAYERTEMPLATE_OBSERVER {
                            updated.num_observers += 1;
                        } else {
                            updated.num_players += 1;
                        }
                        updated.max_players += 1;
                    }
                    SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI => {
                        updated.num_players += 1;
                        updated.max_players += 1;
                        updated.slot_profiles[i] = slot_states[i] as i32;
                    }
                    SlotState::Closed => {}
                }
            }
            info.update_staging_room(updated);
        }
    }

    let Some(queue) = get_peer_message_queue() else {
        return false;
    };

    if let Ok(mut queue) = queue.lock() {
        let mut req = PeerRequest::default();
        req.request_type = PeerRequestType::SetGameOptions;
        req.options = options;
        req.game_opts_map_name = map_name;
        req.game_opts_player_names = player_names;
        req.use_stats = use_stats;
        req.num_players = 0;
        req.num_observers = 0;
        let mut num_open_slots = 0;

        for i in 0..MAX_SLOTS {
            match slot_states[i] {
                SlotState::Open => {
                    num_open_slots += 1;
                }
                SlotState::Player => {
                    let name_key = player_names[i].to_lowercase();
                    let (wins, losses, profile_id) = player_info_map
                        .get(&name_key)
                        .map(|p| (p.wins, p.losses, p.profile_id))
                        .unwrap_or((0, 0, 0));
                    let idx = (req.num_observers + req.num_players) as usize;
                    if idx < MAX_SLOTS {
                        req.wins[idx] = wins;
                        req.losses[idx] = losses;
                        req.profiles[idx] = profile_id;
                        req.faction[idx] = slot_templates[i];
                        req.color[idx] = slot_colors[i];
                    }
                    if slot_templates[i] == PLAYERTEMPLATE_OBSERVER {
                        req.num_observers += 1;
                    } else {
                        req.num_players += 1;
                    }
                }
                SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI => {
                    let idx = (req.num_observers + req.num_players) as usize;
                    if idx < MAX_SLOTS {
                        req.wins[idx] = 0;
                        req.losses[idx] = 0;
                        req.profiles[idx] = slot_states[i] as i32;
                        req.faction[idx] = slot_templates[i];
                        req.color[idx] = slot_colors[i];
                    }
                    req.num_players += 1;
                }
                SlotState::Closed => {}
            }
        }
        req.max_players = num_open_slots + req.num_players + req.num_observers;
        queue.add_request(req);
        return true;
    }

    false
}

//! Full skirmish match configuration propagated from UI to match start.
//!
//! Previously the skirmish menu only emitted mode/faction/map. This type carries
//! all eight slots, difficulties, colors, teams, starting positions, and rules.

use crate::ai::AIDifficulty;
use crate::game_logic::{GameLogic, GameMode, Player, Team};
use crate::ui::skirmish_menu::{Faction, GameRules, GameSlot, PlayerType, MAX_SLOTS};
use serde::{Deserialize, Serialize};

/// One configured skirmish slot as pure data (no UI types required by GameLogic).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkirmishSlotConfig {
    pub slot_index: usize,
    pub is_human: bool,
    pub is_active: bool,
    pub faction: String,
    pub color_rgb: (u8, u8, u8),
    pub team: i32,
    pub start_position: i32,
    pub player_name: String,
    pub ai_difficulty: Option<String>,
}

/// Complete skirmish start configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkirmishMatchConfig {
    pub map: String,
    pub rules: GameRulesSnapshot,
    pub slots: Vec<SkirmishSlotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameRulesSnapshot {
    pub starting_cash: i32,
    pub game_speed: f32,
    pub limit_superweapons: bool,
    pub allow_tech_buildings: bool,
    pub crates_enabled: bool,
    pub fog_of_war: bool,
}

impl From<&GameRules> for GameRulesSnapshot {
    fn from(r: &GameRules) -> Self {
        Self {
            starting_cash: r.starting_cash,
            game_speed: r.game_speed,
            limit_superweapons: r.limit_superweapons,
            allow_tech_buildings: r.allow_tech_buildings,
            crates_enabled: r.crates_enabled,
            fog_of_war: r.fog_of_war,
        }
    }
}

impl GameRulesSnapshot {
    pub fn default_rules() -> Self {
        Self::from(&GameRules::default())
    }
}

fn faction_to_team(faction: &str) -> Team {
    match faction.to_ascii_uppercase().as_str() {
        "USA" | "AMERICA" => Team::USA,
        "CHINA" => Team::China,
        "GLA" => Team::GLA,
        _ => Team::USA,
    }
}

fn ui_faction_name(f: Faction) -> String {
    match f {
        Faction::USA => "USA".into(),
        Faction::China => "China".into(),
        Faction::GLA => "GLA".into(),
        Faction::Random => "Random".into(),
    }
}

fn resolve_random_faction(slot_index: usize) -> Team {
    match slot_index % 3 {
        0 => Team::USA,
        1 => Team::China,
        _ => Team::GLA,
    }
}

fn difficulty_from_player_type(t: PlayerType) -> Option<AIDifficulty> {
    match t {
        PlayerType::EasyAI => Some(AIDifficulty::Easy),
        PlayerType::MediumAI => Some(AIDifficulty::Medium),
        PlayerType::HardAI => Some(AIDifficulty::Hard),
        PlayerType::BrutalAI => Some(AIDifficulty::Brutal),
        _ => None,
    }
}

fn difficulty_name(d: AIDifficulty) -> String {
    match d {
        AIDifficulty::Easy => "Easy".into(),
        AIDifficulty::Medium => "Medium".into(),
        AIDifficulty::Hard => "Hard".into(),
        AIDifficulty::Brutal => "Brutal".into(),
    }
}

fn parse_difficulty(s: &str) -> AIDifficulty {
    match s.to_ascii_lowercase().as_str() {
        "easy" => AIDifficulty::Easy,
        "hard" => AIDifficulty::Hard,
        "brutal" => AIDifficulty::Brutal,
        _ => AIDifficulty::Medium,
    }
}

/// Build match config from live skirmish menu state (shipped UI path).
pub fn config_from_skirmish_menu(
    map: &str,
    rules: &GameRules,
    slots: &[GameSlot],
) -> SkirmishMatchConfig {
    let mut out_slots = Vec::with_capacity(MAX_SLOTS);
    for slot in slots.iter().take(MAX_SLOTS) {
        let is_active = !matches!(slot.player_type, PlayerType::Open | PlayerType::Closed);
        let is_human = matches!(slot.player_type, PlayerType::Human);
        let ai = difficulty_from_player_type(slot.player_type);
        out_slots.push(SkirmishSlotConfig {
            slot_index: slot.slot_index,
            is_human,
            is_active,
            faction: ui_faction_name(slot.faction),
            color_rgb: slot.color.to_rgb(),
            team: slot.team,
            start_position: slot.start_position,
            player_name: slot.player_name.clone(),
            ai_difficulty: ai.map(difficulty_name),
        });
    }
    SkirmishMatchConfig {
        map: map.to_string(),
        rules: GameRulesSnapshot::from(rules),
        slots: out_slots,
    }
}

/// Golden skirmish: USA human vs Medium GLA AI on a fixed map name.
pub fn golden_skirmish_config(map: &str) -> SkirmishMatchConfig {
    SkirmishMatchConfig {
        map: map.to_string(),
        rules: GameRulesSnapshot {
            starting_cash: 10_000,
            game_speed: 1.0,
            limit_superweapons: false,
            allow_tech_buildings: true,
            crates_enabled: true,
            fog_of_war: true,
        },
        slots: vec![
            SkirmishSlotConfig {
                slot_index: 0,
                is_human: true,
                is_active: true,
                faction: "USA".into(),
                color_rgb: (0, 0, 200),
                team: 0,
                start_position: 0,
                player_name: "Player".into(),
                ai_difficulty: None,
            },
            SkirmishSlotConfig {
                slot_index: 1,
                is_human: false,
                is_active: true,
                faction: "GLA".into(),
                color_rgb: (200, 0, 0),
                team: 1,
                start_position: 1,
                player_name: "GLA AI".into(),
                ai_difficulty: Some("Medium".into()),
            },
        ],
    }
}

/// Apply full skirmish configuration to the authoritative Main GameLogic.
///
/// This is the shipped match-start path for slots/rules (not hard-coded difficulty-by-id).
pub fn apply_skirmish_config(
    logic: &mut GameLogic,
    config: &SkirmishMatchConfig,
) -> Result<(), String> {
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();

    let cash = config.rules.starting_cash.max(0) as u32;
    let mut human_id: Option<u32> = None;

    for slot in config.slots.iter().filter(|s| s.is_active) {
        let team = if slot.faction.eq_ignore_ascii_case("random") {
            resolve_random_faction(slot.slot_index)
        } else {
            faction_to_team(&slot.faction)
        };
        let player_id = slot.slot_index as u32;
        let mut player = Player::new(player_id, team, &slot.player_name, slot.is_human);
        player.resources.supplies = cash;
        player.color_rgb = slot.color_rgb;
        player.start_position = slot.start_position;
        player.alliance_team = slot.team;
        // C++ Player::resetSciences residual: IntrinsicSciences + Rank1 SPP.
        player.apply_faction_intrinsic_sciences();
        logic.add_player(player);

        if slot.is_human && human_id.is_none() {
            human_id = Some(player_id);
        }

        if !slot.is_human {
            let difficulty = slot
                .ai_difficulty
                .as_deref()
                .map(parse_difficulty)
                .unwrap_or(AIDifficulty::Medium);
            logic.add_ai_opponent(player_id, team, difficulty);
            logic.set_ai_difficulty(player_id, difficulty);
        }
    }

    if logic.get_players().is_empty() {
        return Err("skirmish config produced no active players".into());
    }

    // Apply skirmish game rules that the host currently models.
    // FOW: enable/disable shroud evaluation path on GameLogic when supported.
    logic.set_skirmish_rules(
        config.rules.fog_of_war,
        config.rules.crates_enabled,
        config.rules.limit_superweapons,
        config.rules.allow_tech_buildings,
        config.rules.game_speed,
    );

    let _ = human_id;
    Ok(())
}

/// Local human faction string from config (first human slot).
pub fn local_faction_from_config(config: &SkirmishMatchConfig) -> String {
    config
        .slots
        .iter()
        .find(|s| s.is_human && s.is_active)
        .map(|s| s.faction.clone())
        .unwrap_or_else(|| "USA".into())
}

/// Build match config from the live GameClient skirmish setup (WND Start path).
///
/// SkirmishGameOptionsMenu writes slots/map/cash into `get_skirmish_setup()` and
/// queues `GameMessageType::NewGame`. Main must convert that setup into
/// `SkirmishMatchConfig` so `start_game_from_ui` applies the same authority path
/// as the headless shell smoke / SkirmishMenu.
#[cfg(feature = "game_client")]
pub fn config_from_client_skirmish_setup(
    map_override: Option<&str>,
) -> Option<SkirmishMatchConfig> {
    use game_client::gui::get_skirmish_setup;
    use game_client::{SlotState, MAX_SLOTS};
    use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
    use game_engine::common::rts::player_template::get_player_template_store;

    let setup = get_skirmish_setup();
    let info = setup.game_info().game_info();

    let map = map_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            let selected = setup.selected_map().trim();
            if !selected.is_empty() {
                Some(selected.to_string())
            } else {
                None
            }
        })
        .or_else(|| {
            let m = info.get_map().trim();
            if !m.is_empty() && m != "NOMAP" {
                Some(m.to_string())
            } else {
                None
            }
        })?;

    let store = get_player_template_store();
    let mut slots = Vec::with_capacity(MAX_SLOTS);
    for i in 0..MAX_SLOTS {
        let Some(slot) = info.get_slot(i) else {
            continue;
        };
        let state = slot.get_state();
        let is_active = matches!(
            state,
            SlotState::Player | SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI
        );
        if !is_active {
            // Keep slot indices stable for apply_skirmish_config (slot_index = player id).
            slots.push(SkirmishSlotConfig {
                slot_index: i,
                is_human: false,
                is_active: false,
                faction: "USA".into(),
                color_rgb: (128, 128, 128),
                team: -1,
                start_position: -1,
                player_name: String::new(),
                ai_difficulty: None,
            });
            continue;
        }

        let is_human = matches!(state, SlotState::Player);
        let ai_difficulty = match state {
            SlotState::EasyAI => Some("Easy".into()),
            SlotState::MedAI => Some("Medium".into()),
            SlotState::BrutalAI => Some("Hard".into()),
            _ => None,
        };

        let faction = {
            let tpl = slot.get_player_template();
            if tpl >= 0 {
                store
                    .get_nth_player_template(tpl as usize)
                    .map(|t| {
                        let side = t.get_side().trim();
                        if side.is_empty() {
                            "USA".to_string()
                        } else if side.eq_ignore_ascii_case("America") {
                            "USA".to_string()
                        } else {
                            side.to_string()
                        }
                    })
                    .unwrap_or_else(|| "USA".into())
            } else {
                // PLAYERTEMPLATE_RANDOM / unset — host resolves per slot index.
                "Random".into()
            }
        };

        let color_rgb = color_rgb_from_multiplayer_index(slot.get_color(), i);

        slots.push(SkirmishSlotConfig {
            slot_index: i,
            is_human,
            is_active: true,
            faction,
            color_rgb,
            team: slot.get_team_number(),
            start_position: slot.get_start_pos(),
            player_name: {
                let n = slot.get_name().trim();
                if n.is_empty() {
                    if is_human {
                        "Player".into()
                    } else {
                        format!("AI {}", i)
                    }
                } else {
                    n.to_string()
                }
            },
            ai_difficulty,
        });
    }

    if !slots.iter().any(|s| s.is_active) {
        return None;
    }

    let starting_cash = info.get_starting_cash().count_money() as i32;
    let limit_sw = info.get_superweapon_restriction() != 0;
    let fog = with_multiplayer_settings(|s| s.is_shroud_in_multiplayer);
    Some(SkirmishMatchConfig {
        map,
        rules: GameRulesSnapshot {
            starting_cash: if starting_cash > 0 {
                starting_cash
            } else {
                10_000
            },
            game_speed: 1.0,
            limit_superweapons: limit_sw,
            allow_tech_buildings: true,
            crates_enabled: true,
            fog_of_war: fog,
        },
        slots,
    })
}

fn color_rgb_from_multiplayer_index(color_idx: i32, slot_index: usize) -> (u8, u8, u8) {
    // Fallback palette when MultiplayerSettings colors are not loaded yet.
    const FALLBACK: [(u8, u8, u8); 8] = [
        (0, 0, 200),
        (200, 0, 0),
        (0, 180, 0),
        (200, 200, 0),
        (0, 200, 200),
        (180, 0, 180),
        (220, 120, 0),
        (220, 220, 220),
    ];

    #[cfg(feature = "game_client")]
    {
        use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
        if color_idx >= 0 {
            if let Some(packed) = with_multiplayer_settings(|s| s.get_color_value(color_idx)) {
                let r = ((packed >> 16) & 0xFF) as u8;
                let g = ((packed >> 8) & 0xFF) as u8;
                let b = (packed & 0xFF) as u8;
                return (r, g, b);
            }
        }
    }
    let _ = color_idx;
    FALLBACK[slot_index % FALLBACK.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::skirmish_menu::{GameSlot, PlayerType};

    #[test]
    fn menu_config_propagates_slot_difficulties_and_cash() {
        let rules = GameRules {
            starting_cash: 12_500,
            ..GameRules::default()
        };
        let mut slots = vec![GameSlot::new(0), GameSlot::new(1)];
        slots[0].player_type = PlayerType::Human;
        slots[0].faction = Faction::USA;
        slots[1].player_type = PlayerType::HardAI;
        slots[1].faction = Faction::GLA;

        let cfg = config_from_skirmish_menu("Maps/Test/Test.map", &rules, &slots);
        assert_eq!(cfg.rules.starting_cash, 12_500);
        assert!(cfg.slots[0].is_human);
        assert_eq!(cfg.slots[1].ai_difficulty.as_deref(), Some("Hard"));
        assert_eq!(cfg.slots[1].faction, "GLA");
    }

    #[test]
    fn skirmish_config_grants_faction_intrinsic_sciences() {
        use crate::game_logic::GameLogic;
        let mut logic = GameLogic::new();
        let config = SkirmishMatchConfig {
            map: "Lone Eagle".into(),
            rules: GameRulesSnapshot::default_rules(),
            slots: vec![
                SkirmishSlotConfig {
                    slot_index: 0,
                    is_human: true,
                    is_active: true,
                    faction: "USA".into(),
                    color_rgb: (0, 0, 255),
                    team: 0,
                    start_position: 0,
                    player_name: "Human".into(),
                    ai_difficulty: None,
                },
                SkirmishSlotConfig {
                    slot_index: 1,
                    is_human: false,
                    is_active: true,
                    faction: "China".into(),
                    color_rgb: (255, 0, 0),
                    team: 1,
                    start_position: 1,
                    player_name: "AI".into(),
                    ai_difficulty: Some("Medium".into()),
                },
            ],
        };
        apply_skirmish_config(&mut logic, &config).expect("cfg");
        let usa = logic.get_player(0).expect("usa");
        assert!(usa.has_unlocked_science("SCIENCE_AMERICA"));
        assert!(usa.has_unlocked_science("SCIENCE_Rank1"));
        assert!(usa.science_purchase_points >= 1);
        let china = logic.get_player(1).expect("china");
        assert!(china.has_unlocked_science("SCIENCE_CHINA"));
        assert!(china.has_unlocked_science("SCIENCE_Rank1"));
        assert!(usa.is_capable_of_purchasing_science("SCIENCE_DaisyCutter"));
    }

    #[test]
    fn apply_skirmish_config_sets_cash_and_ai_from_slots() {
        let cfg = golden_skirmish_config("SmokeTestMap");
        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &cfg).expect("apply");
        let p0 = logic.get_player(0).expect("human");
        assert!(p0.is_local || p0.resources.supplies == 10_000);
        assert_eq!(p0.resources.supplies, 10_000);
        let p1 = logic.get_player(1).expect("ai");
        assert_eq!(p1.resources.supplies, 10_000);
        assert_eq!(logic.get_players().len(), 2);
        // Rules from config must be applied onto the authoritative world.
        assert!(logic.skirmish_rules().fog_of_war);
        assert!(logic.skirmish_rules().crates_enabled);
        assert!((logic.skirmish_rules().game_speed - 1.0).abs() < f32::EPSILON);
        // Slot color / start position / alliance team must land on players.
        let p0 = logic.get_player(0).expect("human after rules");
        assert_eq!(p0.color_rgb, (0, 0, 200));
        assert_eq!(p0.start_position, 0);
        assert_eq!(p0.alliance_team, 0);
        assert_eq!(p1.color_rgb, (200, 0, 0));
        assert_eq!(p1.start_position, 1);
        assert_eq!(p1.alliance_team, 1);
        assert!(logic.host_ai_player_count() >= 1);
    }

    #[test]
    fn golden_config_is_usa_vs_medium_gla() {
        let cfg = golden_skirmish_config("Maps/Lone Eagle/Lone Eagle.map");
        assert_eq!(local_faction_from_config(&cfg), "USA");
        assert_eq!(cfg.slots[1].faction, "GLA");
        assert_eq!(cfg.slots[1].ai_difficulty.as_deref(), Some("Medium"));
    }

    /// Host skirmish residual: apply config → load_map must keep players 0/1 cash,
    /// Medium GLA AI registration/difficulty/active, GLA_* templates, and allow
    /// set_ai_active + a non-panicking AI update. Prefer retail Lone Eagle; if the
    /// map is missing, still prove rebind on the synthetic host world.
    #[test]
    fn skirmish_players_and_ai_survive_load_map_preserve_path() {
        const MAP_CANDIDATES: &[&str] = &[
            "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "Maps/Lone Eagle/Lone Eagle.map",
            "Lone Eagle",
        ];
        let map_identity = MAP_CANDIDATES
            .iter()
            .find(|p| {
                std::path::Path::new(p).is_file()
                    || crate::game_logic::script_loader::find_map_file(p).is_some()
            })
            .copied()
            .unwrap_or("Lone Eagle");

        let cfg = golden_skirmish_config(map_identity);
        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &cfg).expect("apply skirmish");
        logic.ensure_ai_faction_templates(Team::USA);
        logic.ensure_ai_faction_templates(Team::GLA);

        assert_eq!(
            logic.get_player(0).map(|p| p.resources.supplies),
            Some(10_000)
        );
        assert_eq!(
            logic.get_player(1).map(|p| p.resources.supplies),
            Some(10_000)
        );
        assert_eq!(
            logic.host_ai_difficulty(1),
            Some(crate::ai::AIDifficulty::Medium)
        );
        assert!(logic.is_host_ai_active(1));
        assert!(logic.templates.contains_key("GLA_CommandCenter"));
        assert!(logic.templates.contains_key("GLA_Barracks"));
        assert!(logic.templates.contains_key("GLA_Soldier"));

        // Stale object_id on the AI build queue (map wipe residual) without spending cash.
        {
            // Touch AI queue via public relocate (re-seeds layout) then rebind will clear refs.
            logic.relocate_host_ai_base(1, glam::Vec3::new(120.0, 0.0, 120.0));
        }

        // Snapshot immediately before load — preserve path must not rewrite cash/slots.
        let cash0 = logic
            .get_player(0)
            .map(|p| p.resources.supplies)
            .expect("human cash before load");
        let cash1 = logic
            .get_player(1)
            .map(|p| p.resources.supplies)
            .expect("ai cash before load");
        let players_before = logic.get_players().len();
        let ai_before = logic.host_ai_player_count();

        let loaded = logic.load_map(map_identity);
        if !loaded {
            // Map missing in this workspace: still exercise explicit rebind residual.
            logic.rebind_host_ai_after_map_load();
        }

        assert!(
            logic.get_player(0).is_some() && logic.get_player(1).is_some(),
            "players 0 and 1 must survive load_map preserve / rebind"
        );
        assert!(
            logic.get_players().len() >= players_before,
            "host player slots must not shrink on load_map"
        );
        assert!(
            logic.host_ai_player_count() >= ai_before,
            "host AI count must not shrink on load_map"
        );
        let cash0_after = logic
            .get_player(0)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        let cash1_after = logic
            .get_player(1)
            .map(|p| p.resources.supplies)
            .unwrap_or(0);
        assert_eq!(
            cash0_after, cash0,
            "human cash must be unchanged across load_map preserve (before={cash0} after={cash0_after})"
        );
        assert_eq!(
            cash1_after, cash1,
            "AI cash must be unchanged across load_map preserve (before={cash1} after={cash1_after})"
        );
        // Slot identity proves preserve (map wipe path would rename to PlayerN defaults).
        assert_eq!(logic.get_player(0).map(|p| p.name.as_str()), Some("Player"));
        assert_eq!(logic.get_player(1).map(|p| p.name.as_str()), Some("GLA AI"));
        assert_eq!(logic.get_player(0).map(|p| p.color_rgb), Some((0, 0, 200)));
        assert_eq!(logic.get_player(1).map(|p| p.color_rgb), Some((200, 0, 0)));
        assert!(
            logic.host_ai_player_count() >= 1,
            "host AI registration must survive load_map"
        );
        assert_eq!(
            logic.host_ai_difficulty(1),
            Some(crate::ai::AIDifficulty::Medium),
            "Medium difficulty must be retained across load_map"
        );
        assert!(
            logic.is_host_ai_active(1),
            "AI is_active must remain true after rebind"
        );
        // set_ai_active must still work (toggle off then on).
        logic.set_ai_active(1, false);
        assert!(!logic.is_host_ai_active(1));
        logic.set_ai_active(1, true);
        assert!(logic.is_host_ai_active(1));

        // Templates required by host AI rebuild soup must still be present.
        for name in [
            "GLA_CommandCenter",
            "GLA_SupplyStash",
            "GLA_ArmsDealer",
            "GLA_Barracks",
            "GLA_Soldier",
            "GLA_Technical",
        ] {
            assert!(
                logic.templates.contains_key(name),
                "AI template {name} must survive load_map / rebind"
            );
        }

        // Non-panicking AI update after rebind (rebuild soup path).
        for _ in 0..15 {
            logic.update();
        }
        // Fail-closed: do not require retail AI parity — only that update ran and
        // AI is still registered/active with cashed players.
        assert!(logic.is_host_ai_active(1));
        assert!(logic.get_player(0).is_some());
        assert!(logic.get_player(1).is_some());
        let _ = loaded; // true when Lone Eagle (or other candidate) resolved on disk
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn config_from_client_skirmish_setup_reads_slots_and_map() {
        use game_client::gui::get_skirmish_setup;
        use game_client::{Money, SlotState};

        {
            let mut setup = get_skirmish_setup();
            setup.set_selected_map(String::new());
            let info = setup.game_info_mut().game_info_mut();
            info.reset();
            info.set_map("Maps/Lone Eagle/Lone Eagle.map".into());
            info.set_starting_cash(Money::new(15_000));
            if let Some(slot) = info.get_slot_mut(0) {
                slot.set_state(SlotState::Player, "Commander".into(), 1);
                slot.set_player_template(-1); // Random → host resolves
                slot.set_team_number(0);
                slot.set_start_pos(0);
                slot.set_color(0);
            }
            if let Some(slot) = info.get_slot_mut(1) {
                slot.set_state(SlotState::MedAI, "GLA AI".into(), 0);
                slot.set_player_template(-1);
                slot.set_team_number(1);
                slot.set_start_pos(1);
                slot.set_color(1);
            }
        }

        let cfg = config_from_client_skirmish_setup(None).expect("config from setup");
        assert!(
            cfg.map.contains("Lone Eagle"),
            "map from GameInfo: {}",
            cfg.map
        );
        assert_eq!(cfg.rules.starting_cash, 15_000);
        assert!(cfg.slots[0].is_human && cfg.slots[0].is_active);
        assert!(!cfg.slots[1].is_human && cfg.slots[1].is_active);
        assert_eq!(cfg.slots[1].ai_difficulty.as_deref(), Some("Medium"));
        assert_eq!(cfg.slots[0].player_name, "Commander");
    }

    /// WND Start residual composition (no window): client skirmish setup →
    /// SkirmishMatchConfig → apply_skirmish_config → PresentationFrame world_env.
    /// Proves menu Start data reaches host authority without a GPU window.
    #[cfg(feature = "game_client")]
    #[test]
    fn new_game_client_setup_applies_to_host_authority() {
        use crate::presentation_frame::PresentationFrame;
        use game_client::gui::get_skirmish_setup;
        use game_client::{Money, SlotState};

        {
            let mut setup = get_skirmish_setup();
            setup.set_selected_map("Maps/Lone Eagle/Lone Eagle.map".into());
            let info = setup.game_info_mut().game_info_mut();
            info.reset();
            info.set_map("Maps/Lone Eagle/Lone Eagle.map".into());
            info.set_starting_cash(Money::new(20_000));
            if let Some(slot) = info.get_slot_mut(0) {
                slot.set_state(SlotState::Player, "Human".into(), 1);
                slot.set_player_template(-1);
                slot.set_team_number(0);
                slot.set_start_pos(0);
            }
            if let Some(slot) = info.get_slot_mut(1) {
                slot.set_state(SlotState::MedAI, "Enemy".into(), 0);
                slot.set_player_template(-1);
                slot.set_team_number(1);
                slot.set_start_pos(1);
            }
        }

        let cfg = config_from_client_skirmish_setup(None).expect("client setup config");
        assert!(cfg.map.contains("Lone Eagle"), "{}", cfg.map);
        assert_eq!(cfg.rules.starting_cash, 20_000);
        assert_eq!(cfg.slots.iter().filter(|s| s.is_active).count(), 2);
        assert!(!local_faction_from_config(&cfg).is_empty());

        let mut logic = GameLogic::new();
        apply_skirmish_config(&mut logic, &cfg).expect("apply");
        assert_eq!(
            logic.get_player(0).map(|p| p.resources.supplies),
            Some(20_000)
        );
        assert!(logic.host_ai_player_count() >= 1);

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        let (a, b) = logic.world_bounds();
        assert_eq!(snap.world_env.world_min, [a.x, a.y, a.z]);
        assert_eq!(snap.world_env.world_max, [b.x, b.y, b.z]);
        assert_eq!(snap.local_player_id, 0);
    }
}

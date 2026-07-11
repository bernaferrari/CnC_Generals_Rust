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
}

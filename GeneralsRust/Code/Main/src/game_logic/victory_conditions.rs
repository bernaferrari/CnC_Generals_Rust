/*
** Command & Conquer Generals Zero Hour(tm) - Victory Conditions
** Mirrors GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/VictoryConditions.cpp
*/

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bitflags::bitflags;
use log::{debug, warn};
use std::sync::OnceLock;

use crate::config::{ConfigValue, IniParser, LoadMode};

use super::{
    game_logic::Player, object::Object, victory::VictoryCondition, KindOf, ObjectId, Team,
};

bitflags! {
    /// Multiplayer victory condition bitflags (see C++ VictoryType enum).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct VictoryType: u32 {
        const NO_BUILDINGS = 1 << 0;
        const NO_UNITS = 1 << 1;
    }
}

impl Default for VictoryType {
    fn default() -> Self {
        Self::NO_BUILDINGS | Self::NO_UNITS
    }
}

impl VictoryType {
    pub fn from_requirements(require_units: bool, require_buildings: bool) -> Self {
        let mut flags = VictoryType::empty();
        if require_units {
            flags |= VictoryType::NO_UNITS;
        }
        if require_buildings {
            flags |= VictoryType::NO_BUILDINGS;
        }
        flags
    }

    pub fn requires_units(self) -> bool {
        self.contains(VictoryType::NO_UNITS)
    }

    pub fn requires_buildings(self) -> bool {
        self.contains(VictoryType::NO_BUILDINGS)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct PlayerArmyState {
    has_any_objects: bool,
    has_units: bool,
    has_structures: bool,
}

impl PlayerArmyState {
    fn from_objects(objects: &HashMap<ObjectId, Object>, team: Team) -> Self {
        let mut state = Self::default();

        for obj in objects.values() {
            if obj.team != team || !obj.is_alive() {
                continue;
            }

            state.has_any_objects = true;

            if obj.is_kind_of(KindOf::Structure) {
                state.has_structures = true;
            } else if counts_as_unit(obj) {
                state.has_units = true;
            }

            if state.has_structures && state.has_units {
                break;
            }
        }

        state
    }
}

fn counts_as_unit(obj: &Object) -> bool {
    obj.is_kind_of(KindOf::Infantry)
        || obj.is_kind_of(KindOf::Vehicle)
        || obj.is_kind_of(KindOf::Aircraft)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllianceState {
    Active,
    AlliedVictory,
    AlliedDefeat,
}

#[derive(Debug, Clone, Copy)]
pub struct AllianceNotification {
    pub player_id: u32,
    pub state: AllianceState,
}

#[derive(Debug)]
pub struct VictoryConditions {
    config: VictoryType,
    defeated_players: HashSet<u32>,
    end_frame: Option<u32>,
    defeat_events: Vec<u32>,
    alliance_states: HashMap<u32, AllianceState>,
    alliance_events: Vec<AllianceNotification>,
    winning_team: Option<Team>,
}

impl VictoryConditions {
    pub fn new() -> Self {
        Self {
            config: VictoryType::default(),
            defeated_players: HashSet::new(),
            end_frame: None,
            defeat_events: Vec::new(),
            alliance_states: HashMap::new(),
            alliance_events: Vec::new(),
            winning_team: None,
        }
    }

    pub fn reset(&mut self) {
        self.defeated_players.clear();
        self.end_frame = None;
        self.config = VictoryType::default();
        self.defeat_events.clear();
        self.alliance_states.clear();
        self.alliance_events.clear();
        self.winning_team = None;
    }

    pub fn set_victory_conditions(&mut self, config: VictoryType) {
        self.config = config;
    }

    pub fn end_frame(&self) -> Option<u32> {
        self.end_frame
    }

    pub fn evaluate(
        &mut self,
        players: &HashMap<u32, Player>,
        objects: &HashMap<ObjectId, Object>,
        frame: u32,
    ) -> Option<VictoryCondition> {
        if players.is_empty() {
            return None;
        }

        let mut living_players = Vec::new();
        let mut active_alliances: HashMap<Team, Vec<u32>> = HashMap::new();

        for (&player_id, player) in players {
            if player.team == Team::Neutral {
                continue;
            }

            if self.defeated_players.contains(&player_id) {
                continue;
            }

            let state = PlayerArmyState::from_objects(objects, player.team);
            if self.is_defeated(state) {
                if self.defeated_players.insert(player_id) {
                    self.defeat_events.push(player_id);
                }
                continue;
            }

            living_players.push(player_id);
            active_alliances
                .entry(player.team)
                .or_default()
                .push(player_id);
        }

        if living_players.is_empty() {
            self.end_frame.get_or_insert(frame);
            return Some(VictoryCondition::Draw);
        }

        let mut non_neutral_alliances: Vec<(Team, Vec<u32>)> = active_alliances
            .into_iter()
            .filter(|(team, members)| *team != Team::Neutral && !members.is_empty())
            .collect();

        if non_neutral_alliances.is_empty() {
            self.end_frame.get_or_insert(frame);
            return Some(VictoryCondition::Draw);
        }
        let winning_entry = if non_neutral_alliances.len() == 1 {
            Some(non_neutral_alliances.remove(0))
        } else {
            None
        };
        self.winning_team = winning_entry.as_ref().map(|(team, _)| *team);
        self.refresh_alliance_states(players);

        if let Some((_, members)) = winning_entry {
            if let Some(winner_id) = members.first().copied() {
                self.end_frame.get_or_insert(frame);
                return Some(VictoryCondition::Winner(winner_id));
            }
        }

        None
    }

    fn is_defeated(&self, state: PlayerArmyState) -> bool {
        match (
            self.config.contains(VictoryType::NO_UNITS),
            self.config.contains(VictoryType::NO_BUILDINGS),
        ) {
            (true, true) => !state.has_any_objects,
            (true, false) => !state.has_units,
            (false, true) => !state.has_structures,
            (false, false) => !state.has_any_objects,
        }
    }

    pub fn take_defeat_events(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.defeat_events)
    }

    pub fn take_alliance_events(&mut self) -> Vec<AllianceNotification> {
        std::mem::take(&mut self.alliance_events)
    }

    fn refresh_alliance_states(&mut self, players: &HashMap<u32, Player>) {
        for (&player_id, player) in players {
            if player.team == Team::Neutral {
                continue;
            }
            let new_state = if self.defeated_players.contains(&player_id) {
                AllianceState::AlliedDefeat
            } else if self
                .winning_team
                .map(|team| team == player.team)
                .unwrap_or(false)
            {
                AllianceState::AlliedVictory
            } else {
                AllianceState::Active
            };
            let previous = self
                .alliance_states
                .insert(player_id, new_state)
                .unwrap_or(AllianceState::Active);
            if previous != new_state {
                self.alliance_events.push(AllianceNotification {
                    player_id,
                    state: new_state,
                });
            }
        }
    }
}

pub fn victory_rules_for_map(map_name: &str) -> VictoryType {
    let rules = MAP_VICTORY_RULES
        .get_or_init(MapVictoryRules::load)
        .victory_for(map_name);

    if rules != VictoryType::default() {
        return rules;
    }

    campaign_victory_override(map_name).unwrap_or(rules)
}

struct MapVictoryRules {
    default: VictoryType,
    overrides: HashMap<String, VictoryType>,
}

impl MapVictoryRules {
    fn load() -> Self {
        let mut parser = IniParser::new();
        let mut loaded = false;
        const SEARCH_PATHS: [&str; 2] = ["Data/INI/MapVictoryRules.ini", "INI/MapVictoryRules.ini"];
        for path in SEARCH_PATHS {
            let path_ref = Path::new(path);
            if !path_ref.exists() {
                continue;
            }
            match parser.load_file(path_ref, LoadMode::MultiFile) {
                Ok(_) => {
                    debug!("Loaded map victory rules from {}", path_ref.display());
                    loaded = true;
                }
                Err(err) => warn!(
                    "Failed to load map victory rules {}: {err}",
                    path_ref.display()
                ),
            }
        }

        if !loaded {
            return Self {
                default: VictoryType::default(),
                overrides: HashMap::new(),
            };
        }

        let mut default_rules = VictoryType::default();
        let mut overrides = HashMap::new();

        for (section, values) in parser.get_config() {
            if values.is_empty() {
                continue;
            }
            let require_units = read_bool(values.get("requireunits"), true);
            let require_buildings = read_bool(values.get("requirebuildings"), true);
            let mut rules = VictoryType::from_requirements(require_units, require_buildings);

            if let Some(configured) = values
                .get("victory")
                .and_then(|value| parse_victory_string(value))
            {
                rules = configured;
            }

            if section == "default" {
                default_rules = rules;
            } else {
                overrides.insert(section.to_lowercase(), rules);
            }
        }

        Self {
            default: default_rules,
            overrides,
        }
    }

    fn victory_for(&self, map_name: &str) -> VictoryType {
        if map_name.is_empty() {
            return self.default;
        }
        let normalized = map_name.to_lowercase();
        self.overrides
            .get(&normalized)
            .copied()
            .unwrap_or(self.default)
    }
}

fn read_bool(value: Option<&ConfigValue>, default: bool) -> bool {
    match value {
        Some(ConfigValue::Boolean(b)) => *b,
        Some(ConfigValue::Integer(i)) => *i != 0,
        Some(ConfigValue::Float(f)) => *f != 0.0,
        Some(ConfigValue::String(s)) => match s.trim().to_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => true,
            "false" | "no" | "off" | "0" => false,
            _ => default,
        },
        _ => default,
    }
}

fn parse_victory_string(value: &ConfigValue) -> Option<VictoryType> {
    match value {
        ConfigValue::String(text) => {
            let tokens = text
                .split(|c| c == ',' || c == '|' || c == '+' || c == ';')
                .map(|token| token.trim().to_lowercase())
                .filter(|token| !token.is_empty());

            let mut rules = VictoryType::empty();
            let mut saw_token = false;
            for token in tokens {
                saw_token = true;
                match token.as_str() {
                    "annihilation" | "standard" | "default" => {
                        rules = VictoryType::NO_BUILDINGS | VictoryType::NO_UNITS;
                        break;
                    }
                    "nobuildings" | "structures" => rules |= VictoryType::NO_BUILDINGS,
                    "nounits" | "armies" => rules |= VictoryType::NO_UNITS,
                    _ => {}
                }
            }

            if saw_token {
                if rules.is_empty() {
                    Some(VictoryType::NO_BUILDINGS | VictoryType::NO_UNITS)
                } else {
                    Some(rules)
                }
            } else {
                None
            }
        }
        ConfigValue::Boolean(_) | ConfigValue::Integer(_) | ConfigValue::Float(_) => {
            Some(VictoryType::from_requirements(
                read_bool(Some(value), true),
                read_bool(Some(value), true),
            ))
        }
        _ => None,
    }
}

static MAP_VICTORY_RULES: OnceLock<MapVictoryRules> = OnceLock::new();

fn campaign_victory_override(map_name: &str) -> Option<VictoryType> {
    if map_name.is_empty() {
        return None;
    }

    let manager_arc = crate::save_load::game_state::global_campaign_manager().ok()?;
    let manager = manager_arc.try_lock().ok()?;
    for mission in manager.iter_missions() {
        if mission.map_name.eq_ignore_ascii_case(map_name) {
            if let Some(rule) = mission
                .victory_rule
                .as_deref()
                .and_then(parse_victory_keyword)
            {
                return Some(rule);
            }
        }
    }
    None
}

fn parse_victory_keyword(keyword: &str) -> Option<VictoryType> {
    match keyword.trim().to_lowercase().as_str() {
        "annihilation" | "default" | "standard" => {
            Some(VictoryType::NO_BUILDINGS | VictoryType::NO_UNITS)
        }
        "nounits" | "units" | "armies" => Some(VictoryType::NO_UNITS),
        "nobuildings" | "structures" => Some(VictoryType::NO_BUILDINGS),
        "none" | "custom" => Some(VictoryType::empty()),
        _ => None,
    }
}

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
    game_logic::{GameMode, Player},
    object::Object,
    victory::VictoryCondition,
    KindOf, ObjectId, Team,
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

/// C++ `VictoryConditions::areAllies` / slot team: `Player.alliance_team`,
/// not faction [`Team`]. Unset (`< 0`) means the player is a lone alliance.
fn alliance_key(player: &Player) -> i32 {
    if player.alliance_team >= 0 {
        player.alliance_team
    } else {
        // Unique negative so two USA slots without a team number never merge.
        -1 - (player.id as i32)
    }
}

/// C++ `TheRecorder->isMultiplayer()`: skirmish / network / replay of those.
/// Campaign (`GAME_SINGLE_PLAYER`) and shell never run this evaluator.
pub fn is_multiplayer_or_skirmish_victory(mode: GameMode) -> bool {
    matches!(
        mode,
        GameMode::Skirmish
            | GameMode::Multiplayer
            | GameMode::Lan
            | GameMode::Internet
            | GameMode::Replay
    )
}

fn unique_faction_owner(players: &HashMap<u32, Player>, team: Team) -> Option<u32> {
    if team == Team::Neutral {
        return None;
    }
    let mut found = None;
    for player in players.values() {
        if player.team != team || !is_playable_victory_player(player) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(player.id);
    }
    found
}

fn object_belongs_to_player(
    obj: &Object,
    player: &Player,
    unique_team_owner: Option<u32>,
) -> bool {
    match obj.owner_player_id {
        Some(owner) => owner == player.id,
        None => unique_team_owner == Some(player.id) && obj.team == player.team,
    }
}

fn is_playable_victory_player(player: &Player) -> bool {
    if player.team == Team::Neutral || player.is_observer {
        return false;
    }
    let name = player.name.to_ascii_lowercase();
    if name.contains("observer") || name.contains("civilian") {
        return false;
    }
    !leftover_player_is_observer(player.id)
}

fn leftover_player_is_observer(player_id: u32) -> bool {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return false;
    };
    let named = format!("player{player_id}");
    let player = list
        .find_player_by_name(&named)
        .or_else(|| list.get_player(player_id as gamelogic::player::PlayerIndex).cloned());
    drop(list);
    player
        .and_then(|p| p.read().ok().map(|g| g.is_player_observer()))
        .unwrap_or(false)
}

/// C++ `Team::hasAnyBuildings(KINDOF_MP_COUNT_FOR_VICTORY)` — STRUCTURE is
/// forced on the mask; walls without the victory bit do not stall a match.
fn counts_as_victory_building(obj: &Object) -> bool {
    obj.is_kind_of(KindOf::Structure) && obj.is_kind_of(KindOf::MpCountForVictory)
}

/// C++ `Team::hasAnyUnits`: not structure, not projectile, not mine.
fn counts_as_unit(obj: &Object) -> bool {
    !obj.is_kind_of(KindOf::Structure)
        && !obj.is_kind_of(KindOf::Projectile)
        && !obj.is_kind_of(KindOf::Mine)
}

/// C++ `Team::hasAnyObjects`: skip projectiles, mines (and inert, not hosted).
fn counts_as_any_object(obj: &Object) -> bool {
    !obj.is_kind_of(KindOf::Projectile) && !obj.is_kind_of(KindOf::Mine)
}

impl PlayerArmyState {
    fn from_objects(
        objects: &HashMap<ObjectId, Object>,
        player: &Player,
        unique_team_owner: Option<u32>,
    ) -> Self {
        let mut state = Self::default();

        for obj in objects.values() {
            if !object_belongs_to_player(obj, player, unique_team_owner) || !obj.is_alive() {
                continue;
            }

            if counts_as_any_object(obj) {
                state.has_any_objects = true;
            }

            if counts_as_victory_building(obj) {
                state.has_structures = true;
            } else if counts_as_unit(obj) {
                state.has_units = true;
            }

            if state.has_structures && state.has_units && state.has_any_objects {
                break;
            }
        }

        state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AllianceState {
    Active,
    AlliedVictory,
    AlliedDefeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    winning_alliance: Option<i32>,
    pending_kills: Vec<u32>,
}

impl Default for VictoryConditions {
    fn default() -> Self {
        Self::new()
    }
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
            winning_alliance: None,
            pending_kills: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.defeated_players.clear();
        self.end_frame = None;
        self.config = VictoryType::default();
        self.defeat_events.clear();
        self.alliance_states.clear();
        self.alliance_events.clear();
        self.winning_alliance = None;
        self.pending_kills.clear();
        // C++ VictoryConditions::reset clears m_singleAllianceRemaining.
        gamelogic::helpers::TheVictoryConditions::set_local_allied_victory(false);
    }

    pub fn set_victory_conditions(&mut self, config: VictoryType) {
        self.config = config;
    }

    pub fn victory_type(&self) -> VictoryType {
        self.config
    }

    pub fn end_frame(&self) -> Option<u32> {
        self.end_frame
    }

    pub fn evaluate(
        &mut self,
        players: &HashMap<u32, Player>,
        objects: &HashMap<ObjectId, Object>,
        frame: u32,
        game_mode: GameMode,
    ) -> Option<VictoryCondition> {
        // C++ VictoryConditions.cpp:125 — early-return unless isMultiplayer.
        if !is_multiplayer_or_skirmish_victory(game_mode) {
            return None;
        }
        if players.is_empty() {
            return None;
        }

        let mut living_players = Vec::new();
        let mut active_alliances: HashMap<i32, Vec<u32>> = HashMap::new();

        for (&player_id, player) in players {
            if !is_playable_victory_player(player) {
                continue;
            }

            if self.defeated_players.contains(&player_id) {
                continue;
            }

            let unique_owner = unique_faction_owner(players, player.team);
            let state = PlayerArmyState::from_objects(objects, player, unique_owner);
            if self.is_defeated(state) {
                if self.defeated_players.insert(player_id) {
                    self.defeat_events.push(player_id);
                    self.pending_kills.push(player_id);
                }
                continue;
            }

            living_players.push(player_id);
            active_alliances
                .entry(alliance_key(player))
                .or_default()
                .push(player_id);
        }

        if living_players.is_empty() {
            self.end_frame.get_or_insert(frame);
            self.winning_alliance = None;
            self.refresh_alliance_states(players);
            self.sync_leftover_local_allied_victory(players);
            return Some(VictoryCondition::Draw);
        }

        let mut non_neutral_alliances: Vec<(i32, Vec<u32>)> = active_alliances
            .into_iter()
            .filter(|(_, members)| !members.is_empty())
            .collect();

        if non_neutral_alliances.is_empty() {
            self.end_frame.get_or_insert(frame);
            self.winning_alliance = None;
            self.refresh_alliance_states(players);
            self.sync_leftover_local_allied_victory(players);
            return Some(VictoryCondition::Draw);
        }
        let winning_entry = if non_neutral_alliances.len() == 1 {
            Some(non_neutral_alliances.remove(0))
        } else {
            None
        };
        self.winning_alliance = winning_entry.as_ref().map(|(key, _)| *key);
        self.refresh_alliance_states(players);
        self.sync_leftover_local_allied_victory(players);

        if let Some((_, members)) = winning_entry {
            if let Some(winner_id) = members.first().copied() {
                self.end_frame.get_or_insert(frame);
                return Some(VictoryCondition::Winner(winner_id));
            }
        }

        None
    }

    /// C++ `VictoryConditions::isLocalAlliedVictory`: live-computed from
    /// `m_singleAllianceRemaining` + local player still allied with a living
    /// member. Leftover MultiplayerScripts.scb reads a single atomic.
    pub fn is_local_allied_victory(&self, players: &HashMap<u32, Player>) -> bool {
        let Some(key) = self.winning_alliance else {
            return false;
        };
        players.values().any(|player| {
            player.is_local
                && is_playable_victory_player(player)
                && alliance_key(player) == key
        })
    }

    fn sync_leftover_local_allied_victory(&self, players: &HashMap<u32, Player>) {
        gamelogic::helpers::TheVictoryConditions::set_local_allied_victory(
            self.is_local_allied_victory(players),
        );
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

    pub fn peek_defeat_events(&self) -> &[u32] {
        &self.defeat_events
    }

    pub fn take_defeat_events(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.defeat_events)
    }

    pub fn take_pending_kills(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.pending_kills)
    }

    pub fn peek_alliance_events(&self) -> &[AllianceNotification] {
        &self.alliance_events
    }

    pub fn take_alliance_events(&mut self) -> Vec<AllianceNotification> {
        std::mem::take(&mut self.alliance_events)
    }

    fn refresh_alliance_states(&mut self, players: &HashMap<u32, Player>) {
        for (&player_id, player) in players {
            if !is_playable_victory_player(player) {
                continue;
            }
            let new_state = if self.defeated_players.contains(&player_id) {
                AllianceState::AlliedDefeat
            } else if self
                .winning_alliance
                .map(|key| key == alliance_key(player))
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

            if let Some(configured) = values.get("victory").and_then(parse_victory_string) {
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
                .split([',', '|', '+', ';'])
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
    // Prefer stem/path-aware mission match so full map paths resolve Campaign.ini
    // residual table entries (MD_USA01, GC_*, etc.).
    if let Some(mission) = manager.find_mission_for_map(map_name) {
        if let Some(rule) = mission
            .victory_rule
            .as_deref()
            .and_then(parse_victory_keyword)
        {
            return Some(rule);
        }
    }
    for mission in manager.iter_missions() {
        if crate::save_load::campaign::map_name_matches_mission(map_name, &mission.map_name) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ThingTemplate;

    fn player(id: u32, team: Team, alliance: i32) -> Player {
        let mut p = Player::new(id, team, &format!("P{id}"), id == 0);
        p.alliance_team = alliance;
        p.is_alive = true;
        p.resources.supplies = 5_000;
        p
    }

    fn obj(
        id: u32,
        owner: u32,
        team: Team,
        kinds: &[KindOf],
    ) -> (ObjectId, Object) {
        let mut tpl = ThingTemplate::new(format!("T{id}"));
        tpl.set_health(100.0);
        for k in kinds {
            tpl.add_kind_of(*k);
        }
        let oid = ObjectId(id);
        let mut o = Object::new(tpl, oid, team);
        o.owner_player_id = Some(owner);
        (oid, o)
    }

    #[test]
    fn alliance_uses_slot_team_not_faction() {
        let mut vc = VictoryConditions::new();
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::USA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(
            1,
            0,
            Team::USA,
            &[KindOf::Infantry],
        );
        let (b, ob) = obj(
            2,
            1,
            Team::USA,
            &[KindOf::Infantry],
        );
        objects.insert(a, oa);
        objects.insert(b, ob);
        assert!(
            vc.evaluate(&players, &objects, 10, GameMode::Skirmish)
                .is_none(),
            "USA-vs-USA with different alliance_team must continue"
        );

        let mut mixed = HashMap::new();
        mixed.insert(0, player(0, Team::USA, 7));
        mixed.insert(1, player(1, Team::China, 7));
        mixed.insert(2, player(2, Team::GLA, 8));
        let mut objs = HashMap::new();
        let (c, oc) = obj(3, 0, Team::USA, &[KindOf::Infantry]);
        let (d, od) = obj(4, 1, Team::China, &[KindOf::Infantry]);
        objs.insert(c, oc);
        objs.insert(d, od);
        // GLA has no army → defeated; USA+China share alliance 7 → win.
        let outcome = vc.evaluate(&mixed, &objs, 11, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(_))),
            "mixed-faction 2v1 with shared alliance_team must end, got {outcome:?}"
        );
        assert!(
            vc.is_local_allied_victory(&mixed),
            "C++ isLocalAlliedVictory is true for the living local alliance"
        );
        assert!(gamelogic::helpers::TheVictoryConditions::is_local_allied_victory());
        vc.reset();
    }

    #[test]
    fn campaign_and_shell_do_not_evaluate() {
        let mut vc = VictoryConditions::new();
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        objects.insert(a, oa);
        assert!(vc
            .evaluate(&players, &objects, 3, GameMode::SinglePlayer)
            .is_none());
        assert!(vc
            .evaluate(&players, &objects, 3, GameMode::Shell)
            .is_none());
        assert!(vc.peek_defeat_events().is_empty());
    }

    #[test]
    fn building_loss_requires_mp_count_for_victory() {
        let mut vc = VictoryConditions::new();
        vc.set_victory_conditions(VictoryType::NO_BUILDINGS);
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (cc, occ) = obj(
            1,
            0,
            Team::USA,
            &[KindOf::Structure, KindOf::MpCountForVictory],
        );
        let (wall, owall) = obj(2, 1, Team::GLA, &[KindOf::Structure]);
        objects.insert(cc, occ);
        objects.insert(wall, owall);
        let outcome = vc.evaluate(&players, &objects, 4, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(0))),
            "wall-only STRUCTURE without MP_COUNT must not stall, got {outcome:?}"
        );
        assert!(vc.take_pending_kills().contains(&1));
        assert!(gamelogic::helpers::TheVictoryConditions::is_local_allied_victory());
        vc.reset();
    }
}

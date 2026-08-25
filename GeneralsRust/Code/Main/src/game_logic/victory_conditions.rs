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
    KindOf, ObjectId, Team,
    game_logic::{GameMode, Player, PlayerTemplateIdentity},
    object::Object,
    victory::VictoryCondition,
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

/// C++ `Player::getRelationship(const Team*)` (`Player.cpp:542-572`).
/// Team-id override (`PLAYER_SET_OVERRIDE_RELATION_TO_TEAM`) wins first,
/// then leftover `m_teamRelations`, then player->player map, then lobby
/// `alliance_team` as the initial slot-team default.
fn live_player_relationship(source: &Player, target: &Player) -> gamelogic::common::Relationship {
    use gamelogic::common::Relationship;
    if source.id == target.id {
        return Relationship::Allies;
    }
    if let Some(rel) = leftover_relationship_to_default_team(source, target) {
        return rel;
    }
    for name in default_team_instance_names(target) {
        if let Some(rel) = leftover_player_team_relationship_override(source, &name) {
            return rel;
        }
        if let Some(rel) = source.team_relationship_override(&name) {
            return rel;
        }
    }
    if let Some(rel) = source.map_relationship(target.id) {
        return rel;
    }
    if source.alliance_team >= 0 && source.alliance_team == target.alliance_team {
        Relationship::Allies
    } else if source.alliance_team >= 0 && target.alliance_team >= 0 {
        Relationship::Enemies
    } else {
        Relationship::Neutral
    }
}

/// C++ `"team" + playerName` default team plus leftover / `teamplayerN` aliases.
fn default_team_instance_names(player: &Player) -> Vec<String> {
    let mut names = Vec::new();
    let push = |names: &mut Vec<String>, raw: String| {
        let name = raw.trim();
        if name.is_empty() {
            return;
        }
        if !names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(name))
        {
            names.push(name.to_string());
        }
    };
    if let Some(name) = leftover_default_team_instance_name(player) {
        push(&mut names, name);
    }
    let host_name = player.name.trim();
    if !host_name.is_empty() {
        push(&mut names, format!("team{host_name}"));
    }
    let map_name = player.map_side.map_player_name.trim();
    if !map_name.is_empty() {
        push(&mut names, format!("team{map_name}"));
    }
    push(&mut names, format!("teamplayer{}", player.id));
    names
}

fn leftover_default_team_instance_name(player: &Player) -> Option<String> {
    let arc = leftover_player_arc_for_host(player.id, &player.name, false)?;
    let team = {
        let guard = arc.read().ok()?;
        guard.get_default_team()?
    };
    let name = team.read().ok()?.get_name().to_string();
    if name.trim().is_empty() {
        None
    } else {
        Some(name)
    }
}

/// C++ `p1->getRelationship(p2->getDefaultTeam())` leftover `m_teamRelations` tier.
fn leftover_relationship_to_default_team(
    source: &Player,
    target: &Player,
) -> Option<gamelogic::common::Relationship> {
    let source_arc = leftover_player_arc_for_host(source.id, &source.name, false)?;
    let target_arc = leftover_player_arc_for_host(target.id, &target.name, false)?;
    let team = {
        let target_guard = target_arc.read().ok()?;
        target_guard.get_default_team()?
    };
    let team_guard = team.read().ok()?;
    let source_guard = source_arc.read().ok()?;
    source_guard.override_relationship_for_team(&team_guard)
}

/// C++ leftover `Player::m_teamRelations` keyed by named team instance.
fn leftover_player_team_relationship_override(
    source: &Player,
    team_name: &str,
) -> Option<gamelogic::common::Relationship> {
    if team_name.trim().is_empty() {
        return None;
    }
    let team = {
        let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
            return None;
        };
        factory.find_team_instances(team_name).into_iter().next()?
    };
    let player_arc = leftover_player_arc_for_host(source.id, &source.name, false)?;
    let Ok(player) = player_arc.read() else {
        return None;
    };
    let Ok(team_guard) = team.read() else {
        return None;
    };
    player.override_relationship_for_team(&team_guard)
}

/// C++ `VictoryConditions::areAllies`.
/// Mutual `Player::getRelationship(getDefaultTeam())` ALLIES and not the same player.
/// Team-id overrides (`PLAYER_SET_OVERRIDE_RELATION_TO_TEAM`) win first;
/// live host `map_relationship` (PLAYER_RELATES / map playerAllies) next;
/// lobby `alliance_team` is the initial slot-team default when neither map is set.
fn live_players_are_allies(a: &Player, b: &Player) -> bool {
    use gamelogic::common::Relationship;
    if a.id == b.id {
        return false;
    }
    live_player_relationship(a, b) == Relationship::Allies
        && live_player_relationship(b, a) == Relationship::Allies
}

fn player_shares_winning_alliance(
    players: &HashMap<u32, Player>,
    player: &Player,
    winning_rep: i32,
) -> bool {
    let Some(rep) = players.get(&(winning_rep as u32)) else {
        return false;
    };
    player.id == rep.id || live_players_are_allies(player, rep)
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

fn unique_faction_owner(
    players: &HashMap<u32, Player>,
    team: Team,
    identities: &HashMap<u32, PlayerTemplateIdentity>,
) -> Option<u32> {
    if team == Team::Neutral {
        return None;
    }
    let mut found = None;
    for player in players.values() {
        if player.team != team || !is_playable_victory_player(player, identities) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(player.id);
    }
    found
}

fn object_belongs_to_player(obj: &Object, player: &Player, unique_team_owner: Option<u32>) -> bool {
    match obj.owner_player_id {
        Some(owner) => owner == player.id,
        None => unique_team_owner == Some(player.id) && obj.team == player.team,
    }
}

/// C++ `VictoryConditions::cachePlayerPtrs`: Neutral pointer, template-less
/// after a failed resolve, `FactionCivilian` template identity, and
/// `isPlayerObserver()`. Slot display names are never consulted.
fn is_playable_victory_player(
    player: &Player,
    identities: &HashMap<u32, PlayerTemplateIdentity>,
) -> bool {
    if player.team == Team::Neutral || player.is_observer {
        return false;
    }
    if leftover_player_is_observer(player.id) || leftover_player_is_faction_civilian(player.id) {
        return false;
    }
    if let Some(ident) = identities.get(&player.id) {
        match ident.resolve() {
            Some(template) => {
                if template.is_observer()
                    || template.get_name().eq_ignore_ascii_case("FactionCivilian")
                {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

fn leftover_player_is_faction_civilian(player_id: u32) -> bool {
    leftover_player_arc_for_host(player_id, "", false)
        .and_then(|player| {
            player.read().ok().map(|guard| {
                guard.get_player_template().is_some_and(|template| {
                    template.get_name().eq_ignore_ascii_case("FactionCivilian")
                })
            })
        })
        .unwrap_or(false)
}

fn leftover_player_is_markable(player: &gamelogic::player::Player) -> bool {
    !player.is_player_observer()
        && player.get_player_type() != gamelogic::player::PlayerType::Neutral
}

fn leftover_player_arc_for_host(
    player_id: u32,
    host_name: &str,
    markable_only: bool,
) -> Option<std::sync::Arc<std::sync::RwLock<gamelogic::player::Player>>> {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return None;
    };
    let named = format!("player{player_id}");
    if let Some(player) = list.find_player_by_name(&named) {
        return Some(player);
    }
    if !host_name.is_empty() {
        if let Some(player) = list.find_player_by_name(host_name) {
            return Some(player);
        }
    }
    if let Some(player) = list.get_player(player_id as gamelogic::player::PlayerIndex) {
        if !markable_only
            || player
                .read()
                .ok()
                .is_some_and(|guard| leftover_player_is_markable(&guard))
        {
            return Some(std::sync::Arc::clone(player));
        }
    }
    for arc in list.iter() {
        if arc.read().ok().is_some_and(|guard| {
            guard.get_player_index() as u32 == player_id
                && (!markable_only || leftover_player_is_markable(&guard))
        }) {
            return Some(std::sync::Arc::clone(arc));
        }
    }
    None
}

fn leftover_player_is_observer(player_id: u32) -> bool {
    leftover_player_arc_for_host(player_id, "", false)
        .and_then(|player| player.read().ok().map(|guard| guard.is_player_observer()))
        .unwrap_or(false)
}

fn mark_leftover_player_defeated(player_id: u32, host: &Player) {
    let Some(arc) = leftover_player_arc_for_host(player_id, &host.name, true) else {
        return;
    };
    if let Ok(mut guard) = arc.write() {
        guard.set_defeated(true);
        guard.set_player_dead(true);
    }
    if host.is_local {
        gamelogic::helpers::TheVictoryConditions::set_local_player_defeated(true);
    }
}

fn leftover_local_is_observer(players: &HashMap<u32, Player>) -> bool {
    let Some(local) = players.values().find(|player| player.is_local) else {
        // C++ cachePlayerPtrs: no local slot → observer.
        return true;
    };
    local.is_observer || leftover_player_is_observer(local.id)
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

/// C++ `Team::hasAnyObjects`: skip projectiles, mines, and inert
/// (radiation fields stay living so ambulances can attack them).
fn counts_as_any_object(obj: &Object) -> bool {
    !obj.is_kind_of(KindOf::Projectile)
        && !obj.is_kind_of(KindOf::Mine)
        && !obj.is_kind_of(KindOf::Inert)
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
    player_templates: HashMap<u32, PlayerTemplateIdentity>,
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
            player_templates: HashMap::new(),
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
        self.player_templates.clear();
        // C++ VictoryConditions::reset clears m_singleAllianceRemaining.
        gamelogic::helpers::TheVictoryConditions::set_local_allied_victory(false);
        gamelogic::helpers::TheVictoryConditions::set_local_allied_defeat(false);
        gamelogic::helpers::TheVictoryConditions::set_single_alliance_remaining(false);
        gamelogic::helpers::TheVictoryConditions::set_victory_flags_from_live(false);
        gamelogic::helpers::TheVictoryConditions::set_local_player_defeated(false);
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
        self.evaluate_with_templates(players, objects, frame, game_mode, &HashMap::new())
    }

    /// C++ `cachePlayerPtrs` template identity: live host bindings, never names.
    pub fn evaluate_with_templates(
        &mut self,
        players: &HashMap<u32, Player>,
        objects: &HashMap<ObjectId, Object>,
        frame: u32,
        game_mode: GameMode,
        identities: &HashMap<u32, PlayerTemplateIdentity>,
    ) -> Option<VictoryCondition> {
        self.player_templates.clear();
        self.player_templates
            .extend(identities.iter().map(|(&id, ident)| (id, ident.clone())));
        // C++ VictoryConditions.cpp:125 — early-return unless isMultiplayer.
        if !is_multiplayer_or_skirmish_victory(game_mode) {
            return None;
        }
        if players.is_empty() {
            return None;
        }

        let mut living_players = Vec::new();

        for (&player_id, player) in players {
            if !is_playable_victory_player(player, &self.player_templates) {
                continue;
            }

            if self.defeated_players.contains(&player_id) {
                continue;
            }

            let unique_owner = unique_faction_owner(players, player.team, &self.player_templates);
            let state = PlayerArmyState::from_objects(objects, player, unique_owner);
            if self.is_defeated(state) {
                if self.defeated_players.insert(player_id) {
                    // C++ VictoryConditions.cpp:168-196 — mark m_isDefeated and
                    // killPlayer even on frames 0-1. Reveal / GUI message /
                    // GUIMessageReceived only when TheGameLogic->getFrame() > 1
                    // so army-less start sides stay silent (leftover
                    // impl_update.rs:1474).
                    if frame > 1 {
                        self.defeat_events.push(player_id);
                    }
                    self.pending_kills.push(player_id);
                    mark_leftover_player_defeated(player_id, player);
                }
                continue;
            }

            living_players.push(player_id);
        }

        living_players.sort_unstable();

        if living_players.is_empty() {
            self.end_frame.get_or_insert(frame);
            self.winning_alliance = None;
            self.refresh_alliance_states(players);
            self.sync_leftover_victory_flags(players);

            return Some(VictoryCondition::Draw);
        }

        // C++ update: first living player vs every other living player via areAllies.
        let single_alliance = players.get(&living_players[0]).is_some_and(|first| {
            living_players[1..].iter().all(|id| {
                players
                    .get(id)
                    .is_some_and(|other| live_players_are_allies(first, other))
            })
        });

        self.winning_alliance = if single_alliance {
            Some(living_players[0] as i32)
        } else {
            None
        };
        if single_alliance {
            // C++ sets m_endFrame with m_singleAllianceRemaining before scripts read it.
            self.end_frame.get_or_insert(frame);
        }
        self.refresh_alliance_states(players);
        self.sync_leftover_victory_flags(players);

        if single_alliance {
            return Some(VictoryCondition::Winner(living_players[0]));
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
                && is_playable_victory_player(player, &self.player_templates)
                && player_shares_winning_alliance(players, player, key)
        })
    }

    /// C++ `m_singleAllianceRemaining`: 0 or 1 living alliances.
    fn is_single_alliance_remaining(&self) -> bool {
        self.winning_alliance.is_some() || self.end_frame.is_some()
    }

    /// C++ `VictoryConditions::isLocalAlliedDefeat`.
    pub fn is_local_allied_defeat(&self, players: &HashMap<u32, Player>) -> bool {
        if !self.is_single_alliance_remaining() {
            return false;
        }
        if leftover_local_is_observer(players) {
            return true;
        }
        !self.is_local_allied_victory(players)
    }

    fn sync_leftover_victory_flags(&self, players: &HashMap<u32, Player>) {
        gamelogic::helpers::TheVictoryConditions::set_local_allied_victory(
            self.is_local_allied_victory(players),
        );
        gamelogic::helpers::TheVictoryConditions::set_local_allied_defeat(
            self.is_local_allied_defeat(players),
        );
        gamelogic::helpers::TheVictoryConditions::set_single_alliance_remaining(
            self.is_single_alliance_remaining(),
        );
        let local_defeated = players
            .values()
            .any(|player| player.is_local && self.defeated_players.contains(&player.id));
        gamelogic::helpers::TheVictoryConditions::set_local_player_defeated(local_defeated);
        gamelogic::helpers::TheVictoryConditions::set_victory_flags_from_live(true);
    }

    fn is_defeated(&self, state: PlayerArmyState) -> bool {
        match (
            self.config.contains(VictoryType::NO_UNITS),
            self.config.contains(VictoryType::NO_BUILDINGS),
        ) {
            (true, true) => !state.has_any_objects,
            (true, false) => !state.has_units,
            (false, true) => !state.has_structures,
            (false, false) => false,
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
            if !is_playable_victory_player(player, &self.player_templates) {
                continue;
            }
            let new_state = if self.defeated_players.contains(&player_id) {
                AllianceState::AlliedDefeat
            } else if self
                .winning_alliance
                .map(|key| player_shares_winning_alliance(players, player, key))
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

    fn obj(id: u32, owner: u32, team: Team, kinds: &[KindOf]) -> (ObjectId, Object) {
        let mut tpl = ThingTemplate::new(&format!("T{id}"));
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
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        let (b, ob) = obj(2, 1, Team::USA, &[KindOf::Infantry]);
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
    fn scripted_get_relationship_overrides_slot_team() {
        use gamelogic::common::Relationship;
        let mut vc = VictoryConditions::new();
        let mut allies = HashMap::new();
        let mut usa = player(0, Team::USA, 1);
        let mut gla = player(1, Team::GLA, 2);
        usa.set_map_relationship(1, Relationship::Allies);
        gla.set_map_relationship(0, Relationship::Allies);
        allies.insert(0, usa);
        allies.insert(1, gla);
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        let (b, ob) = obj(2, 1, Team::GLA, &[KindOf::Infantry]);
        objects.insert(a, oa);
        objects.insert(b, ob);
        let outcome = vc.evaluate(&allies, &objects, 12, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(_))),
            "mutual PLAYER_RELATES Allies must be one alliance, got {outcome:?}"
        );
        assert!(vc.is_local_allied_victory(&allies));
        vc.reset();

        let mut ffa = HashMap::new();
        let mut usa = player(0, Team::USA, 7);
        let mut china = player(1, Team::China, 7);
        usa.set_map_relationship(1, Relationship::Enemies);
        china.set_map_relationship(0, Relationship::Enemies);
        ffa.insert(0, usa);
        ffa.insert(1, china);
        let mut objs = HashMap::new();
        let (c, oc) = obj(3, 0, Team::USA, &[KindOf::Infantry]);
        let (d, od) = obj(4, 1, Team::China, &[KindOf::Infantry]);
        objs.insert(c, oc);
        objs.insert(d, od);
        assert!(
            vc.evaluate(&ffa, &objs, 13, GameMode::Skirmish).is_none(),
            "scripted Enemies on the same slot team must not share victory"
        );
        vc.reset();
    }

    #[test]
    fn scripted_team_override_allies_counts_for_victory() {
        use gamelogic::common::Relationship;
        let mut vc = VictoryConditions::new();
        let mut allies = HashMap::new();
        let mut usa = player(0, Team::USA, 1);
        let mut gla = player(1, Team::GLA, 2);
        usa.set_map_relationship(1, Relationship::Enemies);
        gla.set_map_relationship(0, Relationship::Enemies);
        usa.set_team_relationship_override("teamP1", Relationship::Allies);
        gla.set_team_relationship_override("teamP0", Relationship::Allies);
        allies.insert(0, usa);
        allies.insert(1, gla);
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        let (b, ob) = obj(2, 1, Team::GLA, &[KindOf::Infantry]);
        objects.insert(a, oa);
        objects.insert(b, ob);
        let outcome = vc.evaluate(&allies, &objects, 14, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(_))),
            "mutual PLAYER_SET_OVERRIDE_RELATION_TO_TEAM Allies must be one alliance, got {outcome:?}"
        );
        assert!(vc.is_local_allied_victory(&allies));
        vc.reset();

        let mut one_way = HashMap::new();
        let mut usa = player(0, Team::USA, 1);
        let gla = player(1, Team::GLA, 2);
        usa.set_team_relationship_override("teamP1", Relationship::Allies);
        one_way.insert(0, usa);
        one_way.insert(1, gla);
        let mut objs = HashMap::new();
        let (c, oc) = obj(3, 0, Team::USA, &[KindOf::Infantry]);
        let (d, od) = obj(4, 1, Team::GLA, &[KindOf::Infantry]);
        objs.insert(c, oc);
        objs.insert(d, od);
        assert!(
            vc.evaluate(&one_way, &objs, 15, GameMode::Skirmish)
                .is_none(),
            "one-way PLAYER_SET_OVERRIDE_RELATION_TO_TEAM must not share victory"
        );
        vc.reset();

        let mut teamplayer = HashMap::new();
        let mut usa = player(0, Team::USA, 1);
        let mut gla = player(1, Team::GLA, 2);
        usa.set_team_relationship_override("teamplayer1", Relationship::Allies);
        gla.set_team_relationship_override("teamplayer0", Relationship::Allies);
        teamplayer.insert(0, usa);
        teamplayer.insert(1, gla);
        let mut objs = HashMap::new();
        let (e, oe) = obj(5, 0, Team::USA, &[KindOf::Infantry]);
        let (f, of) = obj(6, 1, Team::GLA, &[KindOf::Infantry]);
        objs.insert(e, oe);
        objs.insert(f, of);
        let outcome = vc.evaluate(&teamplayer, &objs, 16, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(_))),
            "scripted teamplayerN default-team ALLIES must share victory, got {outcome:?}"
        );
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
        assert!(
            vc.evaluate(&players, &objects, 3, GameMode::SinglePlayer)
                .is_none()
        );
        assert!(
            vc.evaluate(&players, &objects, 3, GameMode::Shell)
                .is_none()
        );
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

    #[test]
    fn inert_objects_do_not_count_as_any_objects() {
        let mut vc = VictoryConditions::new();
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        let (rad, orad) = obj(2, 1, Team::GLA, &[KindOf::Inert]);
        objects.insert(a, oa);
        objects.insert(rad, orad);
        let outcome = vc.evaluate(&players, &objects, 5, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(0))),
            "KINDOF_INERT radiation must not stall annihilation, got {outcome:?}"
        );
        vc.reset();
    }

    fn leftover_named(id: u32) -> std::sync::Arc<std::sync::RwLock<gamelogic::player::Player>> {
        let mut leftover = gamelogic::player::Player::new(id as gamelogic::player::PlayerIndex);
        leftover.set_display_name(format!("player{id}"));
        std::sync::Arc::new(std::sync::RwLock::new(leftover))
    }

    #[test]
    fn evaluate_marks_leftover_player_list_defeated() {
        static PLAYER_LIST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = PLAYER_LIST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        {
            let mut list = gamelogic::player::ThePlayerList()
                .write()
                .unwrap_or_else(|e| e.into_inner());
            list.clear();
            list.add_player(leftover_named(0));
            list.add_player(leftover_named(1));
            list.set_local_player_index(0);
        }

        let mut vc = VictoryConditions::new();
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        objects.insert(a, oa);

        let outcome = vc.evaluate(&players, &objects, 8, GameMode::Skirmish);
        assert!(
            matches!(outcome, Some(VictoryCondition::Winner(0))),
            "local should win when the other army is gone, got {outcome:?}"
        );
        let leftover_defeated = leftover_player_arc_for_host(1, "P1", true)
            .and_then(|arc| arc.read().ok().map(|g| g.is_defeated()))
            .unwrap_or(false);
        assert!(
            leftover_defeated,
            "C++ VictoryConditions::update writes Player::setDefeated"
        );
        assert!(gamelogic::helpers::TheVictoryConditions::is_local_allied_victory());
        assert!(!gamelogic::helpers::TheVictoryConditions::is_local_allied_defeat());

        vc.reset();
        if let Ok(mut list) = gamelogic::player::ThePlayerList().write() {
            list.clear();
        }
    }

    #[test]
    fn allied_defeat_requires_last_alliance_standing() {
        let mut vc = VictoryConditions::new();
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::USA, 1));
        players.insert(2, player(2, Team::China, 2));
        players.insert(3, player(3, Team::GLA, 3));
        let mut objects = HashMap::new();
        let (c, oc) = obj(3, 2, Team::China, &[KindOf::Infantry]);
        let (d, od) = obj(4, 3, Team::GLA, &[KindOf::Infantry]);
        objects.insert(c, oc);
        objects.insert(d, od);

        let outcome = vc.evaluate(&players, &objects, 9, GameMode::Skirmish);
        assert!(
            outcome.is_none(),
            "two enemy alliances still fighting must not end the match, got {outcome:?}"
        );
        assert!(
            !vc.is_local_allied_defeat(&players),
            "C++ isLocalAlliedDefeat is false while multiple alliances remain"
        );
        assert!(!gamelogic::helpers::TheVictoryConditions::is_local_allied_defeat());
        vc.reset();
    }

    #[test]
    fn playable_census_ignores_observer_civilian_slot_names() {
        let mut vc = VictoryConditions::new();
        let mut named = player(0, Team::USA, 1);
        named.name = "CivilianCrusherObserver".into();
        let mut players = HashMap::new();
        players.insert(0, named);
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        let (b, ob) = obj(2, 1, Team::GLA, &[KindOf::Infantry]);
        objects.insert(a, oa);
        objects.insert(b, ob);
        assert!(
            vc.evaluate(&players, &objects, 12, GameMode::Skirmish)
                .is_none(),
            "a living army named Civilian/Observer still counts in cachePlayerPtrs"
        );

        let mut civilian_idents = HashMap::new();
        if let Some(ident) = PlayerTemplateIdentity::from_exact_name("FactionCivilian") {
            civilian_idents.insert(0, ident);
        }
        let mut named_only = HashMap::new();
        named_only.insert(0, {
            let mut p = player(0, Team::USA, 1);
            p.name = "CivilianCrusherObserver".into();
            p
        });
        named_only.insert(1, player(1, Team::GLA, 2));
        let outcome = vc.evaluate_with_templates(
            &named_only,
            &objects,
            13,
            GameMode::Skirmish,
            &civilian_idents,
        );
        if !civilian_idents.is_empty() {
            assert!(
                matches!(outcome, Some(VictoryCondition::Winner(1))),
                "FactionCivilian template identity is excluded, got {outcome:?}"
            );
        }
        vc.reset();
    }

    #[test]
    fn empty_victory_flags_never_auto_defeat() {
        let mut vc = VictoryConditions::new();
        vc.set_victory_conditions(VictoryType::empty());
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        objects.insert(a, oa);
        // GLA has no army; empty flags must leave victory to scripts.
        let outcome = vc.evaluate(&players, &objects, 20, GameMode::Skirmish);
        assert!(
            outcome.is_none(),
            "C++ hasSinglePlayerBeenDefeated is false with no flags, got {outcome:?}"
        );
        assert!(vc.peek_defeat_events().is_empty());
        assert!(vc.take_pending_kills().is_empty());
        vc.reset();
    }

    #[test]
    fn early_frame_defeat_marks_and_kills_without_announce() {
        // C++ VictoryConditions.cpp:168-196 — frames 0-1 still set
        // m_isDefeated + killPlayer; HUD/reveal/audio wait for frame > 1.
        let mut players = HashMap::new();
        players.insert(0, player(0, Team::USA, 1));
        players.insert(1, player(1, Team::GLA, 2));
        let mut objects = HashMap::new();
        let (a, oa) = obj(1, 0, Team::USA, &[KindOf::Infantry]);
        objects.insert(a, oa);

        for frame in [0u32, 1] {
            let mut vc = VictoryConditions::new();
            let outcome = vc.evaluate(&players, &objects, frame, GameMode::Skirmish);
            assert!(
                matches!(outcome, Some(VictoryCondition::Winner(0))),
                "frame {frame} still ends the match, got {outcome:?}"
            );
            assert!(
                vc.peek_defeat_events().is_empty(),
                "frame {frame} must not emit GUI:PlayerHasBeenDefeated"
            );
            assert!(
                vc.take_pending_kills().contains(&1),
                "frame {frame} still killPlayer()s the army-less side"
            );
            vc.reset();
        }

        let mut late = VictoryConditions::new();
        let late_outcome = late.evaluate(&players, &objects, 2, GameMode::Skirmish);
        assert!(
            matches!(late_outcome, Some(VictoryCondition::Winner(0))),
            "frame 2 still ends the match, got {late_outcome:?}"
        );
        assert!(
            late.peek_defeat_events().contains(&1),
            "frame > 1 announces the newly defeated side"
        );
        assert!(late.take_pending_kills().contains(&1));
        late.reset();

        // Frame-0 mark sticks; later ticks must not announce a start-empty side.
        let mut sticky = VictoryConditions::new();
        sticky.evaluate(&players, &objects, 0, GameMode::Skirmish);
        assert!(sticky.peek_defeat_events().is_empty());
        let _ = sticky.take_pending_kills();
        sticky.evaluate(&players, &objects, 8, GameMode::Skirmish);
        assert!(
            sticky.peek_defeat_events().is_empty(),
            "already-marked start-empty side stays silent after frame > 1"
        );
        sticky.reset();
    }
}

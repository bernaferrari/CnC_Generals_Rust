//! Team system - Complete Rust conversion of C++ Team class
//!
//! Teams manage groups of objects that work together, handle ownership,
//! relationships, and provide team-level functionality. This includes both
//! individual Team instances and TeamPrototypes for creating new teams.

use crate::ai::AIGroup;
use crate::common::CoordOrigin;
use crate::common::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{ThePartitionManager, TheThingFactory};
use crate::locomotor::core::{LocomotorSurfaceTypeMask, SURFACE_GROUND};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::{player_list, PLAYER_INDEX_INVALID};
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::core::Script;
use crate::scripting::engine::{get_area_tracker, get_script_engine};
use crate::scripting::evaluator::ScriptEvaluator;
use crate::waypoint::WaypointId;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::{Xfer, XferMode, XferVersion};
use game_engine::common::well_known_keys::{
    key_team_aggressiveness, key_team_all_clear_script, key_team_attack_common_target,
    key_team_auto_reinforce, key_team_avoid_threats, key_team_destroyed_threshold,
    key_team_enemy_sighted_script, key_team_executes_actions_on_create,
    key_team_generic_script_hook, key_team_initial_idle_frames, key_team_is_ai_recruitable,
    key_team_is_base_defense, key_team_is_perimeter_defense, key_team_max_instances, key_team_name,
    key_team_on_create_script, key_team_on_destroyed_script, key_team_on_idle_script,
    key_team_on_unit_destroyed_script, key_team_owner, key_team_production_condition,
    key_team_production_priority, key_team_production_priority_failure_decrease,
    key_team_production_priority_success_increase, key_team_reinforcement_origin,
    key_team_starts_full, key_team_transport, key_team_transports_exit, key_team_transports_return,
    key_team_unit_max_count1, key_team_unit_max_count2, key_team_unit_max_count3,
    key_team_unit_max_count4, key_team_unit_max_count5, key_team_unit_max_count6,
    key_team_unit_max_count7, key_team_unit_min_count1, key_team_unit_min_count2,
    key_team_unit_min_count3, key_team_unit_min_count4, key_team_unit_min_count5,
    key_team_unit_min_count6, key_team_unit_min_count7, key_team_unit_type1, key_team_unit_type2,
    key_team_unit_type3, key_team_unit_type4, key_team_unit_type5, key_team_unit_type6,
    key_team_unit_type7,
};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, LockResult, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, Weak};

/// Team identifier type (matching C++ TeamID)
pub type TeamID = UnsignedInt;
pub const TEAM_ID_INVALID: TeamID = 0;

/// Team prototype identifier (matching C++ TeamPrototypeID)
pub type TeamPrototypeID = UnsignedInt;
pub const TEAM_PROTOTYPE_ID_INVALID: TeamPrototypeID = 0;

/// Attitude type for AI teams (matching C++ AttitudeType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttitudeType {
    Sleep = -2,
    Passive = -1,
    Normal = 0,
    Alert = 1,
    Aggressive = 2,
    Invalid = 3,
}

impl AttitudeType {
    fn from_ini(value: Int) -> Self {
        match value {
            -2 => Self::Sleep,
            -1 => Self::Passive,
            1 => Self::Alert,
            2 => Self::Aggressive,
            3 => Self::Invalid,
            _ => Self::Normal,
        }
    }
}

/// Team behavior types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamBehavior {
    Normal = 0,
    IgnoreDistractions = 1,
    DealAggressively = 2,
}

/// Maximum number of unit types in a team template
pub const MAX_UNIT_TYPES: usize = 7;

/// Maximum generic scripts
pub const MAX_GENERIC_SCRIPTS: usize = 16;

#[derive(Debug, Clone)]
struct PendingTeamScriptEvent {
    team_name: String,
    script_name: String,
}

#[derive(Debug, Clone)]
struct PendingTeamGenericScriptEval {
    team: Arc<RwLock<Team>>,
    prototype: Arc<TeamPrototype>,
    team_name: String,
    script_name: String,
    script_index: usize,
    current_player_name: Option<String>,
}

static PENDING_TEAM_SCRIPT_EVENTS: OnceLock<Mutex<Vec<PendingTeamScriptEvent>>> = OnceLock::new();

fn pending_team_script_events() -> &'static Mutex<Vec<PendingTeamScriptEvent>> {
    PENDING_TEAM_SCRIPT_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn queue_team_script_event(team_name: &str, script_name: &str) {
    if team_name.is_empty() || script_name.is_empty() {
        return;
    }

    if let Ok(mut pending) = pending_team_script_events().lock() {
        pending.push(PendingTeamScriptEvent {
            team_name: team_name.to_string(),
            script_name: script_name.to_string(),
        });
    }
}

fn drain_pending_team_script_events() -> Vec<PendingTeamScriptEvent> {
    pending_team_script_events()
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default()
}

pub fn flush_pending_team_script_events() {
    let pending = drain_pending_team_script_events();
    if pending.is_empty() {
        return;
    }

    // C++ Team::updateState: TheScriptEngine->runScript(scriptName, this).
    let script_engine = get_script_engine();
    let Ok(mut engine_guard) = script_engine.write() else {
        return;
    };
    let Some(engine) = engine_guard.as_mut() else {
        return;
    };

    for event in pending {
        engine.run_script(&event.script_name, Some(event.team_name.as_str()));
    }
}

/// Unit creation info (matching C++ TCreateUnitsInfo)
#[derive(Debug, Clone, Copy)]
pub struct CreateUnitsInfo {
    pub min_units: Int,
    pub max_units: Int,
    pub unit_thing_name: &'static str, // Simplified for now
}

impl CreateUnitsInfo {
    pub const fn new() -> Self {
        Self {
            min_units: 0,
            max_units: 0,
            unit_thing_name: "",
        }
    }
}

impl Default for CreateUnitsInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Team relation map type (matching C++ TeamRelationMapType)
pub type TeamRelationMapType = HashMap<TeamID, Relationship>;

/// Team relation map (matching C++ TeamRelationMap)
#[derive(Debug, Clone)]
pub struct TeamRelationMap {
    pub map: TeamRelationMapType,
}

impl TeamRelationMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

/// Team class (matching C++ Team structure and functionality)
#[derive(Debug)]
pub struct Team {
    // Core identity
    id: TeamID,
    name: AsciiString,

    // Team members (using ObjectID for now)
    members: Vec<ObjectID>,

    // Player control
    controlling_player_id: Option<UnsignedInt>,

    // AI state
    state: AsciiString,

    // Status flags
    entered_or_exited: Bool,
    active: Bool,
    created: Bool,
    recruitable: Bool,
    recruitability_set: Bool,

    // Enemy sighting and awareness
    check_enemy_sighted: Bool,
    see_enemy: Bool,
    prev_see_enemy: Bool,

    // Idle detection
    was_idle: Bool,

    // Destruction tracking
    destroy_threshold: Int,
    cur_units: Int,
    destroyed_threshold_ratio: Real,

    // Script hooks copied from TeamTemplateInfo.
    script_on_create: AsciiString,
    script_on_idle: AsciiString,
    script_on_enemy_sighted: AsciiString,
    script_on_all_clear: AsciiString,
    script_on_destroyed: AsciiString,
    script_on_unit_destroyed: AsciiString,

    // Common attack target
    common_attack_target: ObjectID,

    // Current waypoint for group pathing (matches C++ Team::setCurrentWaypoint)
    current_waypoint_id: Option<WaypointId>,

    // Generic script hooks runtime state (C++ Team::m_shouldAttemptGenericScript)
    should_attempt_generic_script: [Bool; MAX_GENERIC_SCRIPTS],

    // Relationship overrides
    team_relations: Option<TeamRelationMap>,
    player_relations: Option<HashMap<Int, Relationship>>,

    // Singleton flag (from TeamPrototype at creation time)
    is_singleton: Bool,
}

impl Team {
    /// Create a new team with the given ID and name
    pub fn new(name: AsciiString, id: TeamID) -> Self {
        Self {
            id,
            name,
            members: Vec::new(),
            controlling_player_id: None,
            state: String::new().into(),
            entered_or_exited: false,
            active: false,
            created: false,
            recruitable: false,
            recruitability_set: false,
            check_enemy_sighted: false,
            see_enemy: false,
            prev_see_enemy: false,
            was_idle: false,
            destroy_threshold: 0,
            cur_units: 0,
            destroyed_threshold_ratio: 0.0,
            script_on_create: String::new().into(),
            script_on_idle: String::new().into(),
            script_on_enemy_sighted: String::new().into(),
            script_on_all_clear: String::new().into(),
            script_on_destroyed: String::new().into(),
            script_on_unit_destroyed: String::new().into(),
            common_attack_target: INVALID_ID,
            current_waypoint_id: None,
            should_attempt_generic_script: [true; MAX_GENERIC_SCRIPTS],
            team_relations: None,
            player_relations: None,
            is_singleton: false,
        }
    }

    /// Get team ID
    pub fn get_id(&self) -> TeamID {
        self.id
    }

    /// Set team ID
    pub fn set_id(&mut self, id: TeamID) {
        self.id = id;
    }

    /// Get team name
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    /// Set team name  
    pub fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    /// Get controlling player ID
    pub fn get_controlling_player_id(&self) -> Option<UnsignedInt> {
        self.controlling_player_id
    }

    /// Set controlling player
    pub fn set_controlling_player_id(&mut self, player_id: Option<UnsignedInt>) {
        let changed = self.controlling_player_id != player_id;
        self.controlling_player_id = player_id;
        if !changed {
            return;
        }

        // C++ parity (Team::setControllingPlayer): refresh partition/shroud state of all members
        // when team control changes.
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                object_guard.handle_partition_cell_maintenance();
            });
        }
    }

    /// Get team state
    pub fn get_state(&self) -> &AsciiString {
        &self.state
    }

    /// Set team state
    pub fn set_state(&mut self, state: AsciiString) {
        self.state = state;
    }

    /// Set current waypoint ID for this team (matches C++ Team::setCurrentWaypoint).
    pub fn set_current_waypoint_id(&mut self, waypoint_id: Option<WaypointId>) {
        self.current_waypoint_id = waypoint_id;
    }

    /// Get current waypoint ID for this team.
    pub fn get_current_waypoint_id(&self) -> Option<WaypointId> {
        self.current_waypoint_id
    }

    /// Get count of targetable (alive or building) objects
    pub fn get_targetable_count(&self) -> Int {
        if OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let mut count: Int = 0;

        // C++ parity (Team::getTargetableCount):
        // count alive members that either have AI or are structures.
        for &object_id in &self.members {
            let Some(countable) = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_effectively_dead() {
                    return false;
                }
                if object_guard.get_ai_update_interface().is_none()
                    && !object_guard.is_kind_of(KindOf::Structure)
                {
                    return false;
                }
                true
            }) else {
                continue;
            };
            if countable {
                count += 1;
            }
        }

        count
    }

    /// Set team target object
    pub fn set_team_target_object(&mut self, target: ObjectID) {
        if target == INVALID_ID {
            self.common_attack_target = INVALID_ID;
            return;
        }

        // C++ parity: only AI teams set common attack targets, and not on easy difficulty.
        let Some(controller_id) = self.controlling_player_id else {
            return;
        };
        let Some(controller_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(controller_id as Int).cloned())
        else {
            return;
        };
        let Ok(controller) = controller_arc.read() else {
            return;
        };
        if controller.get_player_type() != crate::player::PlayerType::Computer {
            return;
        }
        if controller.get_player_difficulty() == crate::player::GameDifficulty::Easy {
            return;
        }

        self.common_attack_target = target;
    }

    /// Get team target object
    pub fn get_team_target_object(&self) -> ObjectID {
        let target_id = self.common_attack_target;
        if target_id == INVALID_ID {
            return INVALID_ID;
        }

        let Some(valid) = OBJECT_REGISTRY.with_object(target_id, |target| {
            let target_status = target.get_status_bits();
            if target_status.contains(ObjectStatusMaskType::STEALTHED)
                && !target_status.contains(ObjectStatusMaskType::DETECTED)
                && !target_status.contains(ObjectStatusMaskType::DISGUISED)
            {
                return false;
            }

            if target.is_effectively_dead() || target.get_contained_by().is_some() {
                return false;
            }

            if target.is_kind_of(KindOf::Aircraft) {
                return false;
            }

            true
        }) else {
            return INVALID_ID;
        };
        if valid {
            target_id
        } else {
            INVALID_ID
        }
    }

    /// Whether this team should share a common attack target.
    pub fn attack_common_target(&self) -> Bool {
        let team_name = self.get_name().to_string();
        let Ok(factory) = get_team_factory().lock() else {
            return true;
        };
        factory
            .find_team_prototype(&team_name)
            .map(|prototype| prototype.attack_common_target())
            .unwrap_or(true)
    }

    /// Access team member list (read-only).
    /// Set team as active
    pub fn set_active(&mut self) {
        if !self.active {
            self.created = true;
            self.active = true;
        }
    }

    /// Check if team is active
    pub fn is_active(&self) -> Bool {
        self.active
    }

    /// Check if this team can be recruited by AI/team-building logic.
    pub fn is_recruitable(&self) -> Bool {
        self.recruitable
    }

    /// Returns true if this team has an explicit runtime recruitability override.
    /// Mirrors C++ Team::m_isRecruitablitySet semantics.
    pub fn is_recruitability_set(&self) -> Bool {
        self.recruitability_set
    }

    /// Seed recruitability from prototype defaults without marking a runtime override.
    fn set_prototype_recruitable(&mut self, recruitable: Bool) {
        self.recruitable = recruitable;
        self.recruitability_set = false;
    }

    /// Set whether this team can be recruited.
    pub fn set_recruitable(&mut self, recruitable: Bool) {
        self.recruitable = recruitable;
        self.recruitability_set = true;
    }

    /// Copy script hook fields from the team template.
    pub fn apply_template_script_hooks(&mut self, prototype: &TeamPrototype) {
        self.script_on_create = prototype.get_script_on_create().clone();
        self.script_on_idle = prototype.get_script_on_idle().clone();
        self.script_on_enemy_sighted = prototype.get_script_on_enemy_sighted().clone();
        self.script_on_all_clear = prototype.get_script_on_all_clear().clone();
        self.script_on_destroyed = prototype.get_script_on_destroyed().clone();
        self.script_on_unit_destroyed = prototype.get_script_on_unit_destroyed().clone();
        self.destroyed_threshold_ratio = prototype.get_destroyed_threshold();
        self.check_enemy_sighted =
            !self.script_on_enemy_sighted.is_empty() || !self.script_on_all_clear.is_empty();
    }

    pub fn should_attempt_generic_script(&self, index: usize) -> Bool {
        self.should_attempt_generic_script
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    pub fn disable_generic_script_attempt(&mut self, index: usize) {
        if let Some(should_attempt) = self.should_attempt_generic_script.get_mut(index) {
            *should_attempt = false;
        }
    }

    fn compute_enemy_sighted_state(&self) -> (Bool, Bool) {
        let Some(partition) = ThePartitionManager::get() else {
            return (false, false);
        };

        let mut any_alive_in_team = false;
        for &object_id in &self.members {
            let Some(found_enemy) = OBJECT_REGISTRY
                .with_object(object_id, |source| {
                    if source.is_effectively_dead() {
                        return None;
                    }
                    any_alive_in_team = true;

                    let source_pos = *source.get_position();
                    let vision_range = source.get_vision_range();
                    let source_off_map = source.is_off_map();

                    for candidate_id in partition.get_objects_in_range(&source_pos, vision_range) {
                        if candidate_id == object_id {
                            continue;
                        }
                        let is_enemy = OBJECT_REGISTRY
                            .with_object(candidate_id, |candidate| {
                                if candidate.is_effectively_dead() {
                                    return false;
                                }
                                if candidate.is_off_map() != source_off_map {
                                    return false;
                                }

                                let status = candidate.get_status_bits();
                                if status.contains(ObjectStatusMaskType::STEALTHED)
                                    && !status.contains(ObjectStatusMaskType::DETECTED)
                                    && !status.contains(ObjectStatusMaskType::DISGUISED)
                                {
                                    return false;
                                }

                                source.relationship_to(candidate) == Relationship::Enemies
                            })
                            .unwrap_or(false);
                        if is_enemy {
                            return Some(true);
                        }
                    }
                    Some(false)
                })
                .flatten()
            else {
                continue;
            };
            if found_enemy {
                return (true, true);
            }
        }

        (false, any_alive_in_team)
    }

    /// Check if team was just created
    pub fn is_created(&self) -> Bool {
        self.created
    }

    /// Note that a team member entered/exited trigger area
    pub fn set_entered_exited(&mut self) {
        self.entered_or_exited = true;
    }

    /// Check if member entered/exited trigger area
    pub fn did_enter_or_exit(&self) -> Bool {
        self.entered_or_exited
    }

    /// Update team state (called each frame)
    pub fn update_state(&mut self) {
        self.entered_or_exited = false;
        if !self.active {
            return;
        }

        if self.created {
            self.created = false;

            if !self.script_on_create.is_empty() {
                queue_team_script_event(self.name.as_str(), self.script_on_create.as_str());
            }

            if !self.script_on_destroyed.is_empty() {
                self.cur_units = self.members.len() as Int;
                self.destroy_threshold = self.cur_units
                    - (self.cur_units as Real * self.destroyed_threshold_ratio) as Int;

                if self.destroy_threshold > self.cur_units - 1 {
                    self.destroy_threshold = self.cur_units - 1;
                }
                if self.destroy_threshold < 0 {
                    self.destroy_threshold = 0;
                }
            }
        }

        if self.check_enemy_sighted {
            self.prev_see_enemy = self.see_enemy;
            let (see_enemy_now, any_alive_in_team) = self.compute_enemy_sighted_state();
            self.see_enemy = see_enemy_now;

            if any_alive_in_team && self.prev_see_enemy != self.see_enemy {
                if self.see_enemy {
                    queue_team_script_event(
                        self.name.as_str(),
                        self.script_on_enemy_sighted.as_str(),
                    );
                } else {
                    queue_team_script_event(self.name.as_str(), self.script_on_all_clear.as_str());
                }
            }
        }

        if !self.script_on_destroyed.is_empty() {
            let prev_units = self.cur_units;
            self.cur_units = 0;

            for &object_id in &self.members {
                let alive = OBJECT_REGISTRY
                    .with_object(object_id, |object_guard| {
                        !object_guard.is_effectively_dead()
                    })
                    .unwrap_or(false);
                if alive {
                    self.cur_units += 1;
                }
            }

            if self.cur_units != prev_units && self.cur_units <= self.destroy_threshold {
                queue_team_script_event(self.name.as_str(), self.script_on_destroyed.as_str());
                self.destroy_threshold = -1;
            }
        }

        if !self.script_on_idle.is_empty() {
            let mut is_idle = true;
            let mut any_alive_in_team = false;

            for &object_id in &self.members {
                let Some(idle) = OBJECT_REGISTRY
                    .with_object(object_id, |object_guard| {
                        if object_guard.is_effectively_dead() {
                            return None;
                        }
                        if object_guard.get_ai_update_interface().is_none() {
                            return None;
                        }
                        Some(object_guard.is_idle())
                    })
                    .flatten()
                else {
                    continue;
                };

                any_alive_in_team = true;
                if !idle {
                    is_idle = false;
                }
            }

            if any_alive_in_team && is_idle && self.was_idle {
                queue_team_script_event(self.name.as_str(), self.script_on_idle.as_str());
            }
            self.was_idle = is_idle;
        }
    }

    /// Notify team of object death
    pub fn notify_team_of_object_death(&mut self) {
        if self.script_on_unit_destroyed.is_empty() {
            return;
        }

        queue_team_script_event(self.name.as_str(), self.script_on_unit_destroyed.as_str());
    }

    /// Get relationship with another team
    /// Matches C++ Team.cpp:1447 Team::getRelationship
    pub fn get_relationship(&self, that_team: &Team) -> Relationship {
        if self.get_id() == that_team.get_id() {
            return Relationship::Allies;
        }

        // Check for team-specific relationship override first
        if let Some(ref relations) = self.team_relations {
            if let Some(&relationship) = relations.map.get(&that_team.get_id()) {
                return relationship;
            }
        }

        // Check for player-specific override
        if let Some(ref player_relations) = self.player_relations {
            if let Some(that_player_id) = that_team.get_controlling_player_id() {
                if let Some(&relationship) = player_relations.get(&(that_player_id as Int)) {
                    return relationship;
                }
            }
        }

        // Fall back to controlling player's relationship with that team.
        if let Some(my_player_id) = self.get_controlling_player_id() {
            if let Ok(players) = player_list().read() {
                if let Some(my_player_arc) = players.get_player(my_player_id as Int).cloned() {
                    if let Ok(my_player) = my_player_arc.read() {
                        return my_player.get_relationship_with_team(that_team);
                    }
                }
            }
        }

        Relationship::Neutral
    }

    /// Get relationship between this team and a player
    pub fn get_relationship_with_player(&self, player_index: Int) -> Relationship {
        // Check for player-specific override
        if let Some(ref player_relations) = self.player_relations {
            if let Some(&relationship) = player_relations.get(&player_index) {
                return relationship;
            }
        }

        if let Some(my_player_id) = self.get_controlling_player_id() {
            if let Ok(players) = player_list().read() {
                if let (Some(my_player_arc), Some(that_player_arc)) = (
                    players.get_player(my_player_id as Int).cloned(),
                    players.get_player(player_index).cloned(),
                ) {
                    if let (Ok(my_player), Ok(that_player)) =
                        (my_player_arc.read(), that_player_arc.read())
                    {
                        return my_player.get_relationship(&that_player);
                    }
                }
            }
        }

        Relationship::Neutral
    }

    /// Set override team relationship
    pub fn set_override_team_relationship(&mut self, team_id: TeamID, relationship: Relationship) {
        if team_id == TEAM_ID_INVALID {
            return;
        }
        if self.team_relations.is_none() {
            self.team_relations = Some(TeamRelationMap::new());
        }
        if let Some(ref mut relations) = self.team_relations {
            relations.map.insert(team_id, relationship);
        }
    }

    /// Remove override team relationship
    pub fn remove_override_team_relationship(&mut self, team_id: TeamID) -> Bool {
        if let Some(ref mut relations) = self.team_relations {
            if relations.map.is_empty() {
                return false;
            }
            if team_id == TEAM_ID_INVALID {
                relations.map.clear();
                return true;
            }
            relations.map.remove(&team_id).is_some()
        } else {
            false
        }
    }

    /// Clear all team-to-team relationship overrides.
    /// Matches C++ Team::removeOverrideTeamRelationship(NULL) behavior.
    pub fn clear_override_team_relationships(&mut self) {
        if let Some(ref mut relations) = self.team_relations {
            relations.map.clear();
        }
    }

    /// Set override player relationship
    pub fn set_override_player_relationship(
        &mut self,
        player_index: Int,
        relationship: Relationship,
    ) {
        if player_index == PLAYER_INDEX_INVALID {
            return;
        }
        if self.player_relations.is_none() {
            self.player_relations = Some(HashMap::new());
        }
        if let Some(ref mut relations) = self.player_relations {
            relations.insert(player_index, relationship);
        }
    }

    /// Remove override player relationship
    pub fn remove_override_player_relationship(&mut self, player_index: Int) -> Bool {
        if let Some(ref mut relations) = self.player_relations {
            if relations.is_empty() {
                return false;
            }
            if player_index == PLAYER_INDEX_INVALID {
                relations.clear();
                return true;
            }
            relations.remove(&player_index).is_some()
        } else {
            false
        }
    }

    /// Clear all team-to-player relationship overrides.
    /// Matches C++ Team::removeOverridePlayerRelationship(NULL) behavior.
    pub fn clear_override_player_relationships(&mut self) {
        if let Some(ref mut relations) = self.player_relations {
            relations.clear();
        }
    }

    /// Count buildings in team
    pub fn count_buildings(&self) -> Int {
        if OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let mut count = 0;
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_kind_of(KindOf::Structure) {
                    count += 1;
                }
            });
        }
        count
    }

    /// Count team members matching each template entry.
    /// Matches C++ Team::countObjectsByThingTemplate.
    pub fn count_objects_by_thing_template(
        &self,
        templates: &[Arc<dyn ThingTemplate>],
        ignore_dead: Bool,
        ignore_under_construction: Bool,
        counts: &mut [Int],
    ) {
        if OBJECT_REGISTRY.is_empty() {
            counts.fill(0);
            return;
        }
        counts.fill(0);
        let max_templates = templates.len().min(counts.len());
        if max_templates == 0 {
            return;
        }

        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if ignore_dead && object_guard.is_effectively_dead() {
                    return;
                }
                if ignore_under_construction
                    && object_guard.test_status(ObjectStatusTypes::UnderConstruction)
                {
                    return;
                }

                let obj_template = object_guard.get_template();
                for i in 0..max_templates {
                    if !obj_template.is_equivalent_to(templates[i].as_ref()) {
                        continue;
                    }
                    counts[i] += 1;
                    break;
                }
            });
        }
    }

    /// Heal all team members completely.
    /// Matches C++ Team::healAllObjects.
    pub fn heal_all_objects(&mut self) {
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                let _ = object_guard.heal_completely();
            });
        }
    }

    /// Iterate all live team member objects and invoke callback for each.
    /// Matches C++ Team::iterateObjects.
    /// Host/presentation path: OBJECT_REGISTRY empty → no dual-world members to visit.
    fn for_each_live_member<F>(&self, mut func: F)
    where
        F: FnMut(Arc<RwLock<crate::object::Object>>),
    {
        if OBJECT_REGISTRY.is_empty() || self.members.is_empty() {
            return;
        }
        for &object_id in &self.members {
            let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            func(object_arc);
        }
    }

    pub fn iterate_objects<F>(&self, mut func: F)
    where
        F: FnMut(Arc<RwLock<crate::object::Object>>),
    {
        self.for_each_live_member(func);
    }

    /// Add this team's members to an AIGroup.
    /// Matches C++ Team::getTeamAsAIGroup.
    pub fn get_team_as_ai_group(&self, ai_group: &mut AIGroup) {
        self.iterate_objects(|object_arc| {
            let _ = ai_group.add(object_arc);
        });
    }

    /// Try to recruit a matching unit from other teams of this controller.
    /// Matches C++ Team::tryToRecruit.
    pub fn try_to_recruit(
        &self,
        template: &Arc<dyn ThingTemplate>,
        team_home: &Coord3D,
        max_dist: Real,
    ) -> Option<Arc<RwLock<crate::object::Object>>> {
        let controller_id = self.controlling_player_id?;
        let default_team_id = player_list()
            .read()
            .ok()
            .and_then(|players| players.get_player(controller_id as Int).cloned())
            .and_then(|player_arc| {
                player_arc
                    .read()
                    .ok()
                    .and_then(|player| player.get_default_team())
            })
            .and_then(|team_arc| team_arc.read().ok().map(|team| team.get_id()));

        let my_priority = get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| {
                factory
                    .find_team_prototype(self.name.as_str())
                    .map(|prototype| prototype.get_production_priority())
            })
            .unwrap_or(Int::MAX);

        let mut dist_sqr = max_dist * max_dist;
        let mut recruit: Option<Arc<RwLock<crate::object::Object>>> = None;
        let mut current = crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .and_then(|logic| logic.get_first_object());

        while let Some(object_arc) = current.clone() {
            let (next, obj_template, obj_player, obj_team, is_dead, held, pos, candidate_name) = {
                let Ok(object_guard) = object_arc.read() else {
                    break;
                };
                (
                    object_guard.get_next_object(),
                    object_guard.get_template().clone(),
                    object_guard.get_controlling_player_id(),
                    object_guard.get_team(),
                    object_guard.is_effectively_dead(),
                    object_guard.is_disabled_by_type(DisabledType::Held),
                    *object_guard.get_position(),
                    object_guard.get_template().get_name().to_string(),
                )
            };
            current = next;

            let template_matches = obj_template.is_equivalent_to(template.as_ref())
                || TheThingFactory::has_build_variation_name(template, &candidate_name);
            if !template_matches {
                continue;
            }
            if obj_player != Some(controller_id) {
                continue;
            }
            if is_dead || held {
                continue;
            }

            let Some(source_team_arc) = obj_team else {
                continue;
            };
            let Ok(source_team_guard) = source_team_arc.read() else {
                continue;
            };
            if !source_team_guard.is_active() {
                continue;
            }

            let source_priority = get_team_factory()
                .lock()
                .ok()
                .and_then(|factory| {
                    factory
                        .find_team_prototype(source_team_guard.get_name().as_str())
                        .map(|prototype| prototype.get_production_priority())
                })
                .unwrap_or(Int::MAX);
            if source_priority >= my_priority {
                continue;
            }

            let is_default_team = default_team_id == Some(source_team_guard.get_id());
            let mut team_is_recruitable = is_default_team;
            if source_team_guard.is_recruitable() {
                team_is_recruitable = true;
            }
            if source_team_guard.is_recruitability_set() {
                team_is_recruitable = source_team_guard.is_recruitable();
            }
            if !team_is_recruitable {
                continue;
            }

            // C++ also checks AIUpdateInterface::isRecruitable(). This runtime does not expose a
            // getter on the trait yet, so we currently mirror all available checks.
            let dx = team_home.x - pos.x;
            let dy = team_home.y - pos.y;
            let this_dist_sqr = dx * dx + dy * dy;

            if is_default_team && recruit.is_none() {
                recruit = Some(object_arc.clone());
                dist_sqr = this_dist_sqr;
            }

            if this_dist_sqr > dist_sqr {
                continue;
            }

            dist_sqr = this_dist_sqr;
            recruit = Some(object_arc);
        }

        recruit
    }

    /// Count objects with specific kind flags
    pub fn count_objects(&self, set_mask: u32, clear_mask: u32) -> Int {
        if OBJECT_REGISTRY.is_empty() {
            return 0;
        }
        let required = set_mask as KindOfMaskType;
        let forbidden = clear_mask as KindOfMaskType;
        let mut count = 0;

        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_kind_of_multi(required, forbidden) {
                    count += 1;
                }
            });
        }

        count
    }

    /// Check if team has any buildings
    pub fn has_any_buildings(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    object_guard.is_kind_of(KindOf::Structure)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has buildings of specific kind
    pub fn has_any_buildings_of_kind(&self, kind_of: u32) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        let mask = (kind_of as KindOfMaskType) | (1u64 << (KindOf::Structure as u32));
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    object_guard.is_kind_of_multi(mask, crate::common::KIND_OF_MASK_NONE)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has any units
    pub fn has_any_units(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                        return false;
                    }
                    if object_guard.is_kind_of(KindOf::Structure)
                        || object_guard.is_kind_of(KindOf::Projectile)
                        || object_guard.is_kind_of(KindOf::Mine)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has any objects
    pub fn has_any_objects(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead()
                        || object_guard.is_destroyed()
                        || object_guard.is_kind_of(KindOf::Projectile)
                        || object_guard.is_kind_of(KindOf::Inert)
                        || object_guard.is_kind_of(KindOf::Mine)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if all team units are idle
    pub fn is_idle(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            let Some(idle) = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                if object_guard.is_effectively_dead() {
                    return true; // skip dead
                }
                let Some(ai_arc) = object_guard.get_ai_update_interface() else {
                    return true; // skip non-AI
                };
                let Ok(ai_guard) = ai_arc.lock() else {
                    return false; // lock fail => not idle
                };
                ai_guard.is_idle()
            }) else {
                continue;
            };
            if !idle {
                return false;
            }
        }
        true
    }

    /// Returns true when any live team member is inside the trigger area.
    /// Matches C++ Team::unitsEntered.
    pub fn units_entered(&self, trigger: &PolygonTrigger) -> Bool {
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_effectively_dead() {
                        return false;
                    }
                    Self::object_in_trigger(object_guard, trigger)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Check if team has any build facilities
    pub fn has_any_build_facility(&self) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    object_guard.get_template().is_build_facility()
                })
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Move team to destination
    pub fn move_team_to(&mut self, _destination: Coord3D) {
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        // C++ Team::moveTeamTo currently performs no command issue.
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object(object_id, |object_guard| {
                let _ = object_guard.is_effectively_dead() || object_guard.is_destroyed();
            });
        }
    }

    /// Damage all team members
    pub fn damage_team_members(&mut self, amount: Real) -> Bool {
        if OBJECT_REGISTRY.is_empty() {
            return false;
        }
        for &object_id in &self.members {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                if object_guard.is_effectively_dead() || object_guard.is_destroyed() {
                    return;
                }

                if amount < 0.0 {
                    object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                } else {
                    let mut damage_info = DamageInfo::with_simple(
                        amount,
                        INVALID_ID,
                        DamageType::Unresistable,
                        DeathType::Normal,
                    );
                    let _ = object_guard.attempt_damage(&mut damage_info);
                }
            });
        }

        // C++ Team::damageTeamMembers returns FALSE.
        false
    }

    /// Delete team (mark for destruction)
    pub fn delete_team(&mut self, ignore_dead: Bool) {
        if self.is_default_team_for_controller() {
            self.evacuate_team_containers();
        }

        let members = self.members.clone();
        if let Ok(mut manager) = get_object_manager().write() {
            for object_id in members {
                let should_destroy = OBJECT_REGISTRY
                    .with_object(object_id, |object_guard| {
                        !(ignore_dead && object_guard.is_effectively_dead())
                    })
                    .unwrap_or(false);
                if !should_destroy {
                    continue;
                }
                manager.destroy_object(object_id);
            }
        }
    }

    /// Get estimated team position (first member position)
    pub fn get_estimate_team_position(&self) -> Option<Coord3D> {
        let object_id = *self.members.first()?;
        OBJECT_REGISTRY.with_object(object_id, |object_guard| *object_guard.get_position())
    }

    fn locomotor_surface_matches(
        ai: Option<Arc<Mutex<dyn crate::modules::AIUpdateInterface>>>,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> bool {
        // C++ parity (Team.cpp static locoSetMatches):
        // script condition bits are remapped before comparing against locomotor surface bits.
        let mut surface_bits = which_to_consider as UnsignedInt;
        surface_bits = (surface_bits & 0x01) | ((surface_bits & 0x02) << 2);
        let considered = surface_bits as LocomotorSurfaceTypeMask;

        if let Some(ai_arc) = ai {
            if let Ok(ai_guard) = ai_arc.lock() {
                if let Some(loco_arc) = ai_guard.get_cur_locomotor() {
                    if let Ok(loco_guard) = loco_arc.lock() {
                        return (loco_guard.get_legal_surfaces() & considered) != 0;
                    }
                }
            }
        }

        (SURFACE_GROUND & considered) != 0
    }

    fn object_in_trigger(object: &crate::object::Object, trigger: &PolygonTrigger) -> bool {
        let pos = object.get_position();
        let point = ICoord3D::new(pos.x as Int, pos.y as Int, pos.z as Int);
        trigger.point_in_trigger_int(&point)
    }

    fn object_did_enter(object_id: ObjectID, trigger: &PolygonTrigger) -> bool {
        let area_tracker = get_area_tracker();
        let area_name = trigger.get_trigger_name().str();
        let current_frame = crate::helpers::TheGameLogic::get_frame() as u32;
        area_tracker
            .get_last_enter_frame(area_name, object_id)
            .map(|frame| frame == current_frame || frame + 1 == current_frame)
            .unwrap_or(false)
    }

    fn object_did_exit(object_id: ObjectID, trigger: &PolygonTrigger) -> bool {
        let area_tracker = get_area_tracker();
        let area_name = trigger.get_trigger_name().str();
        let current_frame = crate::helpers::TheGameLogic::get_frame() as u32;
        area_tracker
            .get_last_exit_frame(area_name, object_id)
            .map(|frame| frame == current_frame || frame + 1 == current_frame)
            .unwrap_or(false)
    }

    pub fn did_all_enter(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        if !self.entered_or_exited {
            return false;
        }

        let mut entered = false;
        let mut outside = false;

        for &object_id in &self.members {
            let Some((did_enter, is_outside)) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }

                    let did_enter = Self::object_did_enter(object_id, trigger);
                    let is_outside = !did_enter && !Self::object_in_trigger(object_guard, trigger);
                    Some((did_enter, is_outside))
                })
                .flatten()
            else {
                continue;
            };

            if did_enter {
                entered = true;
            } else if is_outside {
                outside = true;
            }
        }

        entered && !outside
    }

    pub fn did_partial_enter(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        if !self.entered_or_exited {
            return false;
        }

        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return false;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return false;
                    }
                    Self::object_did_enter(object_id, trigger)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    pub fn did_partial_exit(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        if !self.entered_or_exited {
            return false;
        }

        for &object_id in &self.members {
            if OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return false;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return false;
                    }
                    Self::object_did_exit(object_id, trigger)
                })
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    pub fn did_all_exit(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        if !self.entered_or_exited {
            return false;
        }

        let mut exited = false;
        let mut inside = false;
        let mut any_considered = false;

        for &object_id in &self.members {
            let Some((did_exit, is_inside)) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }

                    let did_exit = Self::object_did_exit(object_id, trigger);
                    let is_inside = !did_exit && Self::object_in_trigger(object_guard, trigger);
                    Some((did_exit, is_inside))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if did_exit {
                exited = true;
            } else if is_inside {
                inside = true;
            }
        }

        any_considered && exited && !inside
    }

    pub fn all_inside(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        if !self.has_any_objects() {
            return false;
        }

        let mut any_considered = false;
        let mut any_outside = false;

        for &object_id in &self.members {
            let Some(is_outside) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }
                    Some(!Self::object_in_trigger(object_guard, trigger))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if is_outside {
                any_outside = true;
                break;
            }
        }

        any_considered && !any_outside
    }

    pub fn none_inside(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        let mut any_considered = false;
        let mut any_inside = false;

        for &object_id in &self.members {
            let Some(is_inside) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }
                    Some(Self::object_in_trigger(object_guard, trigger))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if is_inside {
                any_inside = true;
            }
        }

        any_considered && !any_inside
    }

    pub fn some_inside_some_outside(
        &self,
        trigger: &PolygonTrigger,
        which_to_consider: LocomotorSurfaceTypeMask,
    ) -> Bool {
        let mut any_considered = false;
        let mut any_inside = false;
        let mut any_outside = false;

        for &object_id in &self.members {
            let Some(is_inside) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if !Self::locomotor_surface_matches(
                        object_guard.get_ai_update_interface(),
                        which_to_consider,
                    ) {
                        return None;
                    }
                    if object_guard.is_effectively_dead() || object_guard.is_kind_of(KindOf::Inert)
                    {
                        return None;
                    }
                    Some(Self::object_in_trigger(object_guard, trigger))
                })
                .flatten()
            else {
                continue;
            };

            any_considered = true;
            if is_inside {
                any_inside = true;
            } else {
                any_outside = true;
            }
        }

        any_considered && any_inside && any_outside
    }

    /// Transfer all units to another team
    pub fn transfer_units_to(&mut self, new_team: &mut Team) {
        if self.id == new_team.id {
            return;
        }

        let members = self.members.clone();
        for object_id in members {
            new_team.add_member(object_id);
            self.remove_member(object_id);
        }
    }

    /// Kill all team members
    pub fn kill_team(&mut self) {
        self.evacuate_team_containers();

        let neutral_default_team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
            .and_then(|player_arc| {
                player_arc
                    .read()
                    .ok()
                    .and_then(|player| player.get_default_team())
            });

        // C++ parity (Team::killTeam): effectively-dead beacon objects are still processed.
        let beacon_template = self
            .controlling_player_id
            .and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as Int).cloned())
            })
            .and_then(|player_arc| {
                player_arc.read().ok().and_then(|player| {
                    player
                        .get_player_template()
                        .map(|template| template.beacon_name.clone())
                })
            })
            .and_then(|beacon_name| {
                if beacon_name.is_empty() {
                    None
                } else {
                    TheThingFactory::find_template(beacon_name.as_str())
                }
            });

        let members = self.members.clone();
        let mut moved_to_neutral = Vec::new();
        for object_id in members {
            let Some((_is_beacon, is_tech_building)) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    let is_beacon = beacon_template.as_ref().is_some_and(|template| {
                        object_guard
                            .get_template()
                            .is_equivalent_to(template.as_ref())
                    });
                    let destroyed = object_guard.is_destroyed();
                    let effectively_dead = object_guard.is_effectively_dead();
                    let same_team = object_guard.get_team_id() == Some(self.id);
                    if destroyed || (effectively_dead && !is_beacon) || !same_team {
                        return None;
                    }
                    Some((is_beacon, object_guard.is_kind_of(KindOf::TechBuilding)))
                })
                .flatten()
            else {
                continue;
            };

            if is_tech_building {
                if let Some(neutral_team) = neutral_default_team.clone() {
                    let moved = OBJECT_REGISTRY
                        .with_object_mut(object_id, |object_guard| {
                            let _ = object_guard.set_team(Some(neutral_team));
                        })
                        .is_some();
                    if moved {
                        moved_to_neutral.push(object_id);
                    }
                } else {
                    let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                        object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                    });
                }
            } else {
                let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                    object_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Normal));
                });
            }
        }

        if !moved_to_neutral.is_empty() {
            let moved_set: HashSet<ObjectID> = moved_to_neutral.iter().copied().collect();
            self.members.retain(|id| !moved_set.contains(id));
            self.cur_units = self.members.len() as Int;

            if let Some(neutral_team) = neutral_default_team {
                if let Ok(mut neutral_guard) = neutral_team.write() {
                    for object_id in moved_to_neutral {
                        neutral_guard.add_member(object_id);
                    }
                }
            }
        }
    }

    fn is_default_team_for_controller(&self) -> bool {
        let Some(controller_id) = self.controlling_player_id else {
            return false;
        };
        let Ok(players) = player_list().read() else {
            return false;
        };
        let Some(player_arc) = players.get_player(controller_id as Int).cloned() else {
            return false;
        };
        let Ok(player) = player_arc.read() else {
            return false;
        };
        let Some(default_team) = player.get_default_team() else {
            return false;
        };
        let Ok(default_team_guard) = default_team.read() else {
            return false;
        };
        default_team_guard.get_id() == self.id
    }

    fn evacuate_team_containers(&self) {
        if OBJECT_REGISTRY.is_empty() {
            return;
        }
        let members = self.members.clone();
        for object_id in members {
            let Some(contain_arc) = OBJECT_REGISTRY
                .with_object(object_id, |object_guard| {
                    if object_guard.is_destroyed() || object_guard.is_effectively_dead() {
                        return None;
                    }
                    object_guard.get_contain()
                })
                .flatten()
            else {
                continue;
            };
            let Ok(mut contain_guard) = contain_arc.lock() else {
                continue;
            };
            if contain_guard.get_contain_count() > 0 {
                let _ = contain_guard.remove_all_contained(false);
            }
        }
    }

    // Member management
    pub fn add_member(&mut self, object_id: ObjectID) {
        if !self.members.contains(&object_id) {
            self.members.push(object_id);
            self.cur_units = self.members.len() as Int;
        }
    }

    pub fn remove_member(&mut self, object_id: ObjectID) {
        self.members.retain(|&id| id != object_id);
        self.cur_units = self.members.len() as Int;
    }

    pub fn get_members(&self) -> &[ObjectID] {
        &self.members
    }

    pub fn get_member_count(&self) -> usize {
        self.members.len()
    }

    pub fn has_member(&self, object_id: ObjectID) -> bool {
        self.members.contains(&object_id)
    }

    /// Check if this team is allied with another team
    pub fn is_allied_with(&self, that_team: &Team) -> Bool {
        matches!(self.get_relationship(that_team), Relationship::Allies)
    }

    /// Check if this team is enemy with another team
    pub fn is_enemy_with(&self, that_team: &Team) -> Bool {
        matches!(self.get_relationship(that_team), Relationship::Enemies)
    }

    /// Check if this team is neutral with another team
    pub fn is_neutral_with(&self, that_team: &Team) -> Bool {
        matches!(self.get_relationship(that_team), Relationship::Neutral)
    }

    /// Get all team members (for iteration/AI)
    pub fn iterate_members<F>(&self, mut func: F)
    where
        F: FnMut(ObjectID),
    {
        for &member_id in &self.members {
            func(member_id);
        }
    }

    /// Check if team can target another team (enemy relationship)
    pub fn can_target_team(&self, that_team: &Team) -> Bool {
        self.is_enemy_with(that_team)
    }

    /// Get vision shared teams (all allied teams)
    pub fn get_vision_shared_teams(&self) -> Vec<TeamID> {
        let Ok(factory) = get_team_factory().lock() else {
            return Vec::new();
        };

        let mut shared = Vec::new();
        for team_arc in factory.get_all_teams() {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            if team_guard.get_id() == self.id {
                continue;
            }
            if self.is_allied_with(&team_guard) {
                shared.push(team_guard.get_id());
            }
        }
        shared
    }

    /// Check if this team shares vision with another team
    pub fn shares_vision_with(&self, that_team: &Team) -> Bool {
        self.is_allied_with(that_team)
    }

    /// Check if this team shares radar with another team
    pub fn shares_radar_with(&self, that_team: &Team) -> Bool {
        // Teams share radar if they are allied
        // Full implementation would also check if controlling players have radar
        self.is_allied_with(that_team)
    }

    /// Check if this team is a singleton (only one instance allowed)
    /// Reference: C++ Team::GetIsSingleton()
    pub fn is_singleton(&self) -> Bool {
        self.is_singleton
    }

    /// Set singleton flag on team instance
    /// Reference: C++ Team::SetIsSingleton()
    pub fn set_singleton(&mut self, singleton: Bool) {
        self.is_singleton = singleton;
    }
}

/// Save/load support for Team.
/// Matches C++ Team::xfer (Team.cpp:2547, version 1).
impl Snapshotable for Team {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let mut team_id = self.id;
        xfer.xfer_u32(&mut team_id).map_err(|e| e.to_string())?;

        let mut member_count = self.members.len() as u16;
        xfer.xfer_unsigned_short(&mut member_count)
            .map_err(|e| e.to_string())?;
        for &obj_id in &self.members {
            let mut id = obj_id;
            xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
        }

        let mut state = self.state.to_string();
        xfer.xfer_ascii_string(&mut state)
            .map_err(|e| e.to_string())?;

        let mut entered_or_exited = self.entered_or_exited;
        xfer.xfer_bool(&mut entered_or_exited)
            .map_err(|e| e.to_string())?;
        let mut active = self.active;
        xfer.xfer_bool(&mut active).map_err(|e| e.to_string())?;
        let mut created = self.created;
        xfer.xfer_bool(&mut created).map_err(|e| e.to_string())?;
        let mut check_enemy_sighted = self.check_enemy_sighted;
        xfer.xfer_bool(&mut check_enemy_sighted)
            .map_err(|e| e.to_string())?;
        let mut see_enemy = self.see_enemy;
        xfer.xfer_bool(&mut see_enemy).map_err(|e| e.to_string())?;
        let mut prev_see_enemy = self.prev_see_enemy;
        xfer.xfer_bool(&mut prev_see_enemy)
            .map_err(|e| e.to_string())?;
        let mut was_idle = self.was_idle;
        xfer.xfer_bool(&mut was_idle).map_err(|e| e.to_string())?;
        let mut destroy_threshold = self.destroy_threshold;
        xfer.xfer_int(&mut destroy_threshold)
            .map_err(|e| e.to_string())?;
        let mut cur_units = self.cur_units;
        xfer.xfer_int(&mut cur_units).map_err(|e| e.to_string())?;

        let mut waypoint_id = self.current_waypoint_id.unwrap_or(0);
        xfer.xfer_u32(&mut waypoint_id).map_err(|e| e.to_string())?;

        let mut generic_count = MAX_GENERIC_SCRIPTS as u16;
        xfer.xfer_unsigned_short(&mut generic_count)
            .map_err(|e| e.to_string())?;
        for i in 0..MAX_GENERIC_SCRIPTS {
            let mut val = self.should_attempt_generic_script[i];
            xfer.xfer_bool(&mut val).map_err(|e| e.to_string())?;
        }

        let mut recruitability_set = self.recruitability_set;
        xfer.xfer_bool(&mut recruitability_set)
            .map_err(|e| e.to_string())?;
        let mut recruitable = self.recruitable;
        xfer.xfer_bool(&mut recruitable)
            .map_err(|e| e.to_string())?;

        let mut target = self.common_attack_target;
        xfer.xfer_object_id(&mut target)
            .map_err(|e| e.to_string())?;

        // team_relations (inline, matching C++ TeamRelationMap::xfer pattern)
        let team_rel_count = self
            .team_relations
            .as_ref()
            .map(|r| r.map.len() as u16)
            .unwrap_or(0);
        let mut team_rel_count_xfer = team_rel_count;
        xfer.xfer_unsigned_short(&mut team_rel_count_xfer)
            .map_err(|e| e.to_string())?;
        if let Some(ref relations) = self.team_relations {
            for (&tid, &rel) in &relations.map {
                let mut team_id_val = tid;
                let mut rel_raw = rel as i32;
                xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
            }
        }

        // player_relations (inline, matching C++ PlayerRelationMap::xfer pattern)
        let player_rel_count = self
            .player_relations
            .as_ref()
            .map(|r| r.len() as u16)
            .unwrap_or(0);
        let mut player_rel_count_xfer = player_rel_count;
        xfer.xfer_unsigned_short(&mut player_rel_count_xfer)
            .map_err(|e| e.to_string())?;
        if let Some(ref relations) = self.player_relations {
            for (&pidx, &rel) in relations {
                let mut player_idx = pidx;
                let mut rel_raw = rel as i32;
                xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Team::xfer version 1
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // Team ID sanity check
        let mut team_id = self.id;
        xfer.xfer_u32(&mut team_id).map_err(|e| e.to_string())?;
        if team_id != self.id {
            return Err(format!(
                "Team::xfer - TeamID mismatch. Xfered '{}' but should be '{}'",
                team_id, self.id
            ));
        }

        // Member list
        let mut member_count = self.members.len() as u16;
        xfer.xfer_unsigned_short(&mut member_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for &obj_id in &self.members {
                let mut id = obj_id;
                xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
            }
        } else {
            // Load: store member IDs for post-process reconnection
            self.members.clear();
            for _ in 0..member_count {
                let mut id = INVALID_ID;
                xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
                self.members.push(id);
            }
        }

        // State
        let mut state = self.state.to_string();
        xfer.xfer_ascii_string(&mut state)
            .map_err(|e| e.to_string())?;

        // Status flags
        xfer.xfer_bool(&mut self.entered_or_exited)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.active)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.created)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.check_enemy_sighted)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.see_enemy)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.prev_see_enemy)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.was_idle)
            .map_err(|e| e.to_string())?;

        // Destruction tracking
        xfer.xfer_int(&mut self.destroy_threshold)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.cur_units)
            .map_err(|e| e.to_string())?;

        // Current waypoint ID
        let mut waypoint_id: UnsignedInt = self.current_waypoint_id.unwrap_or(0);
        xfer.xfer_u32(&mut waypoint_id).map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.current_waypoint_id = if waypoint_id == 0 {
                None
            } else {
                Some(waypoint_id)
            };
        }

        // Generic script attempt flags
        let mut generic_count = MAX_GENERIC_SCRIPTS as u16;
        xfer.xfer_unsigned_short(&mut generic_count)
            .map_err(|e| e.to_string())?;
        if generic_count as usize != MAX_GENERIC_SCRIPTS {
            return Err(
                "Team::xfer - The number of allowable Generic scripts has changed, and this chunk needs to be versioned."
                    .to_string(),
            );
        }
        for i in 0..MAX_GENERIC_SCRIPTS {
            xfer.xfer_bool(&mut self.should_attempt_generic_script[i])
                .map_err(|e| e.to_string())?;
        }

        // Recruitability
        xfer.xfer_bool(&mut self.recruitability_set)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.recruitable)
            .map_err(|e| e.to_string())?;

        // Common attack target
        xfer.xfer_object_id(&mut self.common_attack_target)
            .map_err(|e| e.to_string())?;

        // Team relations (inline, matching C++ TeamRelationMap::xfer)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;

            let mut rel_count = self
                .team_relations
                .as_ref()
                .map(|r| r.map.len() as u16)
                .unwrap_or(0);
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Save {
                if let Some(ref relations) = self.team_relations {
                    for (&tid, &rel) in &relations.map {
                        let mut team_id_val = tid;
                        let mut rel_raw = rel as Int;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                self.team_relations = None;
                if rel_count > 0 {
                    let mut map = TeamRelationMap::new();
                    for _ in 0..rel_count {
                        let mut team_id_val: UnsignedInt = 0;
                        let mut rel_raw: Int = 0;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                        let rel = match rel_raw {
                            0 => Relationship::Enemies,
                            1 => Relationship::Neutral,
                            2 => Relationship::Allies,
                            _ => Relationship::Neutral,
                        };
                        map.map.insert(team_id_val, rel);
                    }
                    self.team_relations = Some(map);
                }
            }
        }

        // Player relations (inline, matching C++ PlayerRelationMap::xfer)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;

            let mut rel_count = self
                .player_relations
                .as_ref()
                .map(|r| r.len() as u16)
                .unwrap_or(0);
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Save {
                if let Some(ref relations) = self.player_relations {
                    for (&pidx, &rel) in relations {
                        let mut player_idx = pidx;
                        let mut rel_raw = rel as Int;
                        xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                self.player_relations = None;
                if rel_count > 0 {
                    let mut map = HashMap::new();
                    for _ in 0..rel_count {
                        let mut player_idx: Int = 0;
                        let mut rel_raw: Int = 0;
                        xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                        let rel = match rel_raw {
                            0 => Relationship::Enemies,
                            1 => Relationship::Neutral,
                            2 => Relationship::Allies,
                            _ => Relationship::Neutral,
                        };
                        map.insert(player_idx, rel);
                    }
                    self.player_relations = Some(map);
                }
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl game_engine::common::thing::thing_factory::Team for Team {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn team_id(&self) -> Option<u32> {
        Some(self.get_id())
    }
}

/// Team prototype (matching C++ TeamPrototype functionality)
/// Runtime state for C++ `TeamPrototype::evaluateProductionCondition`.
#[derive(Debug, Default)]
struct ProductionConditionRuntime {
    always_false: bool,
    script: Option<Script>,
}

#[derive(Debug, Clone)]
pub struct TeamPrototype {
    // Identity
    id: TeamPrototypeID,
    name: AsciiString,
    owner_name: AsciiString,
    is_singleton: Bool,

    // Base settings
    is_ai_recruitable: Bool,
    is_base_defense: Bool,
    is_perimeter_defense: Bool,
    automatically_reinforce: Bool,
    initial_team_attitude: AttitudeType,
    transports_return: Bool,
    avoid_threats: Bool,
    attack_common_target: Bool,
    max_instances: Int,
    script_on_create: AsciiString,
    script_on_idle: AsciiString,
    initial_idle_frames: Int,
    script_on_enemy_sighted: AsciiString,
    script_on_all_clear: AsciiString,
    script_on_destroyed: AsciiString,
    destroyed_threshold: Real,
    script_on_unit_destroyed: AsciiString,
    production_priority: Int,
    production_priority_success_increase: Int,
    production_priority_failure_decrease: Int,
    production_condition: AsciiString,
    /// C++ m_productionConditionAlwaysFalse + m_productionConditionScript (shared runtime).
    production_condition_runtime: Arc<Mutex<ProductionConditionRuntime>>,
    execute_actions_on_create: Bool,
    team_generic_scripts: [AsciiString; MAX_GENERIC_SCRIPTS],
    generic_script_runtime: Arc<Mutex<Vec<Option<Script>>>>,

    // Attack priority
    attack_priority_name: AsciiString,

    // Unit creation info (matches TeamTemplateInfo::m_unitsInfo)
    units_info: [CreateUnitsInfo; MAX_UNIT_TYPES],
    num_units_info: usize,

    // Reinforcement-specific settings (matches TeamTemplateInfo reinforcement fields)
    transport_unit_type: AsciiString,
    start_reinforce_waypoint: AsciiString,
    team_starts_full: Bool,
    transports_exit: Bool,

    // C++ TeamTemplateInfo::m_homeLocation / m_hasHomeLocation
    home_location: Coord3D,
    has_home_location: Bool,
}

impl TeamPrototype {
    /// Create new team prototype
    pub fn new(name: AsciiString) -> Self {
        Self {
            id: TEAM_PROTOTYPE_ID_INVALID,
            name,
            owner_name: String::new().into(),
            is_singleton: false,
            is_ai_recruitable: false,
            is_base_defense: false,
            is_perimeter_defense: false,
            automatically_reinforce: false,
            initial_team_attitude: AttitudeType::Normal,
            transports_return: false,
            avoid_threats: false,
            attack_common_target: false,
            max_instances: 1,
            script_on_create: String::new().into(),
            script_on_idle: String::new().into(),
            initial_idle_frames: 0,
            script_on_enemy_sighted: String::new().into(),
            script_on_all_clear: String::new().into(),
            script_on_destroyed: String::new().into(),
            destroyed_threshold: 0.0,
            script_on_unit_destroyed: String::new().into(),
            production_priority: 0,
            production_priority_success_increase: 0,
            production_priority_failure_decrease: 0,
            production_condition: String::new().into(),
            production_condition_runtime: Arc::new(Mutex::new(
                ProductionConditionRuntime::default(),
            )),
            execute_actions_on_create: false,
            team_generic_scripts: std::array::from_fn(|_| String::new().into()),
            generic_script_runtime: Arc::new(Mutex::new(vec![None; MAX_GENERIC_SCRIPTS])),
            attack_priority_name: String::new().into(),
            units_info: [CreateUnitsInfo::new(); MAX_UNIT_TYPES],
            num_units_info: 0,
            transport_unit_type: String::new().into(),
            start_reinforce_waypoint: String::new().into(),
            team_starts_full: false,
            transports_exit: false,
            home_location: Coord3D::new(0.0, 0.0, 0.0),
            has_home_location: false,
        }
    }

    /// C++ TeamTemplateInfo::m_hasHomeLocation
    pub fn has_home_location(&self) -> bool {
        self.has_home_location
    }

    /// C++ TeamTemplateInfo::m_homeLocation
    pub fn home_location(&self) -> Coord3D {
        self.home_location
    }

    pub fn set_home_location(&mut self, loc: Coord3D) {
        self.home_location = loc;
        self.has_home_location = true;
    }

    pub fn clear_home_location(&mut self) {
        self.has_home_location = false;
        self.home_location = Coord3D::new(0.0, 0.0, 0.0);
    }

    /// Get prototype ID
    pub fn get_id(&self) -> TeamPrototypeID {
        self.id
    }

    /// Set prototype ID
    pub fn set_id(&mut self, id: TeamPrototypeID) {
        self.id = id;
    }

    /// Get prototype name
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    /// Get controlling owner/player name from team definition.
    pub fn get_owner_name(&self) -> &AsciiString {
        &self.owner_name
    }

    /// Set controlling owner/player name from team definition.
    pub fn set_owner_name(&mut self, owner_name: AsciiString) {
        self.owner_name = owner_name;
    }

    /// Check if prototype is singleton
    pub fn is_singleton(&self) -> Bool {
        self.is_singleton
    }

    /// Set singleton flag
    pub fn set_singleton(&mut self, singleton: Bool) {
        self.is_singleton = singleton;
    }

    /// Check if AI recruitable
    pub fn is_ai_recruitable(&self) -> Bool {
        self.is_ai_recruitable
    }

    /// Set AI recruitable
    pub fn set_ai_recruitable(&mut self, recruitable: Bool) {
        self.is_ai_recruitable = recruitable;
    }

    /// Check if base defense
    pub fn is_base_defense(&self) -> Bool {
        self.is_base_defense
    }

    /// Set base defense
    pub fn set_base_defense(&mut self, base_defense: Bool) {
        self.is_base_defense = base_defense;
    }

    pub fn is_perimeter_defense(&self) -> Bool {
        self.is_perimeter_defense
    }

    pub fn set_perimeter_defense(&mut self, perimeter_defense: Bool) {
        self.is_perimeter_defense = perimeter_defense;
    }

    pub fn automatically_reinforce(&self) -> Bool {
        self.automatically_reinforce
    }

    pub fn set_automatically_reinforce(&mut self, automatically_reinforce: Bool) {
        self.automatically_reinforce = automatically_reinforce;
    }

    pub fn get_initial_team_attitude(&self) -> AttitudeType {
        self.initial_team_attitude
    }

    pub fn set_initial_team_attitude(&mut self, attitude: AttitudeType) {
        self.initial_team_attitude = attitude;
    }

    pub fn transports_return(&self) -> Bool {
        self.transports_return
    }

    pub fn set_transports_return(&mut self, transports_return: Bool) {
        self.transports_return = transports_return;
    }

    pub fn avoid_threats(&self) -> Bool {
        self.avoid_threats
    }

    pub fn set_avoid_threats(&mut self, avoid_threats: Bool) {
        self.avoid_threats = avoid_threats;
    }

    pub fn attack_common_target(&self) -> Bool {
        self.attack_common_target
    }

    pub fn set_attack_common_target(&mut self, attack_common_target: Bool) {
        self.attack_common_target = attack_common_target;
    }

    /// Get max instances
    pub fn get_max_instances(&self) -> Int {
        self.max_instances
    }

    /// Set max instances
    pub fn set_max_instances(&mut self, max_instances: Int) {
        self.max_instances = max_instances;
    }

    pub fn get_script_on_create(&self) -> &AsciiString {
        &self.script_on_create
    }

    pub fn set_script_on_create(&mut self, script_name: AsciiString) {
        self.script_on_create = script_name;
    }

    pub fn get_script_on_idle(&self) -> &AsciiString {
        &self.script_on_idle
    }

    pub fn set_script_on_idle(&mut self, script_name: AsciiString) {
        self.script_on_idle = script_name;
    }

    pub fn get_initial_idle_frames(&self) -> Int {
        self.initial_idle_frames
    }

    pub fn set_initial_idle_frames(&mut self, frames: Int) {
        self.initial_idle_frames = frames;
    }

    pub fn get_script_on_enemy_sighted(&self) -> &AsciiString {
        &self.script_on_enemy_sighted
    }

    pub fn set_script_on_enemy_sighted(&mut self, script_name: AsciiString) {
        self.script_on_enemy_sighted = script_name;
    }

    pub fn get_script_on_all_clear(&self) -> &AsciiString {
        &self.script_on_all_clear
    }

    pub fn set_script_on_all_clear(&mut self, script_name: AsciiString) {
        self.script_on_all_clear = script_name;
    }

    pub fn get_script_on_destroyed(&self) -> &AsciiString {
        &self.script_on_destroyed
    }

    pub fn set_script_on_destroyed(&mut self, script_name: AsciiString) {
        self.script_on_destroyed = script_name;
    }

    pub fn get_destroyed_threshold(&self) -> Real {
        self.destroyed_threshold
    }

    pub fn set_destroyed_threshold(&mut self, destroyed_threshold: Real) {
        self.destroyed_threshold = destroyed_threshold;
    }

    pub fn get_script_on_unit_destroyed(&self) -> &AsciiString {
        &self.script_on_unit_destroyed
    }

    pub fn set_script_on_unit_destroyed(&mut self, script_name: AsciiString) {
        self.script_on_unit_destroyed = script_name;
    }

    /// Get production priority
    pub fn get_production_priority(&self) -> Int {
        self.production_priority
    }

    /// Set production priority
    pub fn set_production_priority(&mut self, priority: Int) {
        self.production_priority = priority;
    }

    pub fn get_production_priority_success_increase(&self) -> Int {
        self.production_priority_success_increase
    }

    pub fn set_production_priority_success_increase(&mut self, increase: Int) {
        self.production_priority_success_increase = increase;
    }

    pub fn get_production_priority_failure_decrease(&self) -> Int {
        self.production_priority_failure_decrease
    }

    pub fn set_production_priority_failure_decrease(&mut self, decrease: Int) {
        self.production_priority_failure_decrease = decrease;
    }

    pub fn get_production_condition(&self) -> &AsciiString {
        &self.production_condition
    }

    pub fn set_production_condition(&mut self, production_condition: AsciiString) {
        self.production_condition = production_condition;
        // Changing the named condition invalidates any cached always-false / script copy.
        if let Ok(mut rt) = self.production_condition_runtime.lock() {
            rt.always_false = false;
            rt.script = None;
        }
    }

    /// C++ `TeamPrototype::evaluateProductionCondition` (Team.cpp).
    ///
    /// Loads/caches the productionCondition script, gates on player difficulty flags,
    /// honors delay-eval frame, and evaluates conditions for the controlling player.
    pub fn evaluate_production_condition(&self) -> Bool {
        let Ok(mut rt) = self.production_condition_runtime.lock() else {
            return false;
        };
        if rt.always_false {
            return false;
        }

        // Already have a local script copy — periodic / immediate eval.
        if rt.script.is_some() {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            if let Some(script) = rt.script.as_mut() {
                if current_frame < script.frame_to_evaluate_at {
                    return false;
                }
                let delay_seconds = script.delay_evaluation_seconds;
                if delay_seconds > 0 {
                    script.frame_to_evaluate_at = current_frame.saturating_add(
                        (delay_seconds as u32).saturating_mul(LOGICFRAMES_PER_SECOND as u32),
                    );
                }
            }
            let player_name = self.controlling_player_name();
            let script_engine = get_script_engine();
            let Ok(mut eng) = script_engine.write() else {
                return false;
            };
            let Some(engine) = eng.as_mut() else {
                return false;
            };
            if let Some(script) = rt.script.as_mut() {
                return engine.evaluate_conditions(script, None, player_name.as_deref());
            }
            return false;
        }

        // No script yet — resolve from name.
        let cond_name = self.production_condition.to_string();
        if cond_name.is_empty() {
            rt.always_false = true;
            return false;
        }

        let script_engine = get_script_engine();
        let Ok(mut eng) = script_engine.write() else {
            return false;
        };
        let Some(engine) = eng.as_mut() else {
            return false;
        };
        let Some(mut script) = engine.find_script_clone_by_name(&cond_name) else {
            rt.always_false = true;
            return false;
        };

        // Difficulty gate (C++ isEasy/isNormal/isHard on script).
        let difficulty = self.controlling_player_difficulty();
        let ok_for_diff = match difficulty {
            crate::player::GameDifficulty::Easy => script.easy,
            crate::player::GameDifficulty::Normal => script.normal,
            crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal => {
                script.hard
            }
        };
        if !ok_for_diff {
            rt.always_false = true;
            return false;
        }

        let player_name = self.controlling_player_name();
        let result = engine.evaluate_conditions(&mut script, None, player_name.as_deref());
        rt.script = Some(script);
        result
    }

    fn controlling_player_name(&self) -> Option<String> {
        let owner = self.owner_name.to_string();
        if owner.is_empty() {
            return None;
        }
        Some(owner)
    }

    fn controlling_player_difficulty(&self) -> crate::player::GameDifficulty {
        let owner = self.owner_name.to_string();
        if owner.is_empty() {
            return crate::player::GameDifficulty::Normal;
        }
        player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&owner))
            .and_then(|p| p.read().ok().map(|g| g.get_player_difficulty()))
            .unwrap_or(crate::player::GameDifficulty::Normal)
    }

    pub fn get_execute_actions_on_create(&self) -> Bool {
        self.execute_actions_on_create
    }

    pub fn set_execute_actions_on_create(&mut self, execute_actions_on_create: Bool) {
        self.execute_actions_on_create = execute_actions_on_create;
    }

    pub fn get_generic_script(&self, index: usize) -> Option<&AsciiString> {
        self.team_generic_scripts.get(index)
    }

    pub fn set_generic_script(&mut self, index: usize, script_name: AsciiString) {
        if let Some(slot) = self.team_generic_scripts.get_mut(index) {
            *slot = script_name;
        }
    }

    fn take_or_load_generic_script_runtime(&self, index: usize) -> Option<Script> {
        let script_name = self.get_generic_script(index)?.to_string();
        if script_name.is_empty() {
            return None;
        }

        let mut runtime = self.generic_script_runtime.lock().ok()?;
        if index >= runtime.len() {
            return None;
        }

        if runtime[index].is_none() {
            let script_engine = get_script_engine();
            let script = script_engine.read().ok().and_then(|engine_guard| {
                engine_guard
                    .as_ref()
                    .and_then(|engine| engine.find_script_clone_by_name(&script_name))
            })?;
            runtime[index] = Some(script);
        }

        runtime[index].take()
    }

    fn store_generic_script_runtime(&self, index: usize, script: Option<Script>) {
        let Ok(mut runtime) = self.generic_script_runtime.lock() else {
            return;
        };
        if let Some(slot) = runtime.get_mut(index) {
            *slot = script;
        }
    }

    pub fn set_units_info(&mut self, index: usize, info: CreateUnitsInfo) {
        if index >= MAX_UNIT_TYPES {
            return;
        }
        self.units_info[index] = info;
        if self.num_units_info <= index {
            self.num_units_info = index + 1;
        }
    }

    pub fn units_info(&self) -> &[CreateUnitsInfo] {
        &self.units_info[..self.num_units_info]
    }

    pub fn get_transport_unit_type(&self) -> &AsciiString {
        &self.transport_unit_type
    }

    pub fn set_transport_unit_type(&mut self, unit_type: AsciiString) {
        self.transport_unit_type = unit_type;
    }

    pub fn get_start_reinforce_waypoint(&self) -> &AsciiString {
        &self.start_reinforce_waypoint
    }

    pub fn set_start_reinforce_waypoint(&mut self, waypoint_name: AsciiString) {
        self.start_reinforce_waypoint = waypoint_name;
    }

    pub fn get_team_starts_full(&self) -> Bool {
        self.team_starts_full
    }

    pub fn set_team_starts_full(&mut self, starts_full: Bool) {
        self.team_starts_full = starts_full;
    }

    pub fn get_transports_exit(&self) -> Bool {
        self.transports_exit
    }

    pub fn set_transports_exit(&mut self, transports_exit: Bool) {
        self.transports_exit = transports_exit;
    }

    /// Set attack priority name
    pub fn set_attack_priority_name(&mut self, name: AsciiString) {
        self.attack_priority_name = name;
    }

    /// Get attack priority name
    pub fn get_attack_priority_name(&self) -> &AsciiString {
        &self.attack_priority_name
    }
}

/// Team factory for managing team prototypes and instances (matching C++ TeamFactory)
#[derive(Debug)]
pub struct TeamFactory {
    prototypes: HashMap<String, Arc<TeamPrototype>>,
    teams: HashMap<TeamID, Arc<RwLock<Team>>>,
    unique_team_prototype_id: TeamPrototypeID,
    unique_team_id: TeamID,
    pending_create_action_scripts: Vec<String>,
    pending_generic_script_evals: Vec<PendingTeamGenericScriptEval>,
}

impl TeamFactory {
    /// Create new team factory
    pub fn new() -> Self {
        Self {
            prototypes: HashMap::new(),
            teams: HashMap::new(),
            unique_team_prototype_id: 1,
            unique_team_id: 1,
            pending_create_action_scripts: Vec::new(),
            pending_generic_script_evals: Vec::new(),
        }
    }

    /// Initialize team factory
    pub fn init(&mut self) {
        self.prototypes.clear();
        self.teams.clear();
        self.unique_team_prototype_id = 1;
        self.unique_team_id = 1;
        self.pending_create_action_scripts.clear();
        self.pending_generic_script_evals.clear();
    }

    /// Reset team factory
    pub fn reset(&mut self) {
        self.prototypes.clear();
        self.teams.clear();
        self.unique_team_prototype_id = 1;
        self.unique_team_id = 1;
        self.pending_create_action_scripts.clear();
        self.pending_generic_script_evals.clear();
    }

    /// Update team factory (called each frame)
    pub fn update(&mut self) {
        // Update all teams
        for team_arc in self.teams.values() {
            if let Ok(mut team) = team_arc.write() {
                team.update_state();
            }
        }

        // Queue generic script evaluations (executed after factory unlock in guard drop).
        for team_arc in self.teams.values() {
            let (team_name, controlling_player_id) = match team_arc.read() {
                Ok(team_guard) => (
                    team_guard.get_name().to_string(),
                    team_guard.get_controlling_player_id(),
                ),
                Err(_) => continue,
            };

            let Some(prototype) = self.prototypes.get(&team_name).cloned() else {
                continue;
            };

            let current_player_name = controlling_player_id.and_then(|player_id| {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as Int).cloned())
                    .and_then(|player_arc| {
                        player_arc
                            .read()
                            .ok()
                            .map(|player| player.get_player_name_key())
                    })
                    .and_then(NameKeyGenerator::key_to_name)
            });

            let Ok(mut team_guard) = team_arc.write() else {
                continue;
            };
            for idx in 0..MAX_GENERIC_SCRIPTS {
                if !team_guard.should_attempt_generic_script(idx) {
                    continue;
                }

                let script_name = prototype
                    .get_generic_script(idx)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if script_name.is_empty() {
                    team_guard.disable_generic_script_attempt(idx);
                    continue;
                }

                self.pending_generic_script_evals
                    .push(PendingTeamGenericScriptEval {
                        team: team_arc.clone(),
                        prototype: prototype.clone(),
                        team_name: team_name.clone(),
                        script_name,
                        script_index: idx,
                        current_player_name: current_player_name.clone(),
                    });
            }
        }

        // C++ parity: remove empty active non-singleton teams that are not default teams.
        let mut teams_to_remove = Vec::new();
        for (team_id, team_arc) in &self.teams {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            if !team_guard.get_members().is_empty() {
                continue;
            }
            if !team_guard.is_active() || team_guard.is_default_team_for_controller() {
                continue;
            }

            let team_name = team_guard.get_name().to_string();
            if self
                .prototypes
                .get(&team_name)
                .is_some_and(|prototype| prototype.is_singleton())
            {
                continue;
            }
            teams_to_remove.push(*team_id);
        }

        for team_id in teams_to_remove {
            self.team_about_to_be_deleted(team_id);
        }
    }

    /// Initialize a team from configuration
    pub fn init_team(
        &mut self,
        name: AsciiString,
        owner: AsciiString,
        is_singleton: Bool,
        dict: Option<&Dict>,
    ) -> Option<Arc<TeamPrototype>> {
        let mut prototype = TeamPrototype::new(name.clone());
        prototype.set_id(self.unique_team_prototype_id);
        prototype.set_owner_name(owner);
        prototype.set_singleton(is_singleton);

        if let Some(dict) = dict {
            let unit_type_keys = [
                key_team_unit_type1(),
                key_team_unit_type2(),
                key_team_unit_type3(),
                key_team_unit_type4(),
                key_team_unit_type5(),
                key_team_unit_type6(),
                key_team_unit_type7(),
            ];
            let unit_min_keys = [
                key_team_unit_min_count1(),
                key_team_unit_min_count2(),
                key_team_unit_min_count3(),
                key_team_unit_min_count4(),
                key_team_unit_min_count5(),
                key_team_unit_min_count6(),
                key_team_unit_min_count7(),
            ];
            let unit_max_keys = [
                key_team_unit_max_count1(),
                key_team_unit_max_count2(),
                key_team_unit_max_count3(),
                key_team_unit_max_count4(),
                key_team_unit_max_count5(),
                key_team_unit_max_count6(),
                key_team_unit_max_count7(),
            ];

            for idx in 0..MAX_UNIT_TYPES {
                let type_key = unit_type_keys[idx];
                let min_key = unit_min_keys[idx];
                let max_key = unit_max_keys[idx];

                if dict.get_type(type_key).is_none() || dict.get_type(max_key).is_none() {
                    continue;
                }

                let max_units = dict.get_int(max_key);
                if max_units <= 0 {
                    continue;
                }
                let min_units = dict.get_int(min_key);
                let template_name = dict.get_ascii_string(type_key);
                if template_name.is_empty() {
                    continue;
                }

                let leaked_name: &'static str = Box::leak(template_name.into_boxed_str());
                prototype.set_units_info(
                    idx,
                    CreateUnitsInfo {
                        min_units,
                        max_units,
                        unit_thing_name: leaked_name,
                    },
                );
            }

            if dict.get_type(key_team_max_instances()).is_some() {
                prototype.set_max_instances(dict.get_int(key_team_max_instances()));
            }

            if dict.get_type(key_team_production_priority()).is_some() {
                prototype.set_production_priority(dict.get_int(key_team_production_priority()));
            }

            if dict
                .get_type(key_team_production_priority_success_increase())
                .is_some()
            {
                prototype.set_production_priority_success_increase(
                    dict.get_int(key_team_production_priority_success_increase()),
                );
            }

            if dict
                .get_type(key_team_production_priority_failure_decrease())
                .is_some()
            {
                prototype.set_production_priority_failure_decrease(
                    dict.get_int(key_team_production_priority_failure_decrease()),
                );
            }

            if dict.get_type(key_team_is_ai_recruitable()).is_some() {
                prototype.set_ai_recruitable(dict.get_bool(key_team_is_ai_recruitable()));
            }

            if dict.get_type(key_team_is_base_defense()).is_some() {
                prototype.set_base_defense(dict.get_bool(key_team_is_base_defense()));
            }

            if dict.get_type(key_team_is_perimeter_defense()).is_some() {
                prototype.set_perimeter_defense(dict.get_bool(key_team_is_perimeter_defense()));
            }

            if dict.get_type(key_team_auto_reinforce()).is_some() {
                prototype.set_automatically_reinforce(dict.get_bool(key_team_auto_reinforce()));
            }

            if dict.get_type(key_team_aggressiveness()).is_some() {
                prototype.set_initial_team_attitude(AttitudeType::from_ini(
                    dict.get_int(key_team_aggressiveness()),
                ));
            }

            if dict.get_type(key_team_transports_return()).is_some() {
                prototype.set_transports_return(dict.get_bool(key_team_transports_return()));
            }

            if dict.get_type(key_team_avoid_threats()).is_some() {
                prototype.set_avoid_threats(dict.get_bool(key_team_avoid_threats()));
            }

            if dict.get_type(key_team_attack_common_target()).is_some() {
                prototype.set_attack_common_target(dict.get_bool(key_team_attack_common_target()));
            }

            if dict.get_type(key_team_on_create_script()).is_some() {
                prototype.set_script_on_create(
                    dict.get_ascii_string(key_team_on_create_script()).into(),
                );
            }

            if dict.get_type(key_team_on_idle_script()).is_some() {
                prototype
                    .set_script_on_idle(dict.get_ascii_string(key_team_on_idle_script()).into());
            }

            if dict.get_type(key_team_initial_idle_frames()).is_some() {
                prototype.set_initial_idle_frames(dict.get_int(key_team_initial_idle_frames()));
            }

            if dict.get_type(key_team_enemy_sighted_script()).is_some() {
                prototype.set_script_on_enemy_sighted(
                    dict.get_ascii_string(key_team_enemy_sighted_script())
                        .into(),
                );
            }

            if dict.get_type(key_team_all_clear_script()).is_some() {
                prototype.set_script_on_all_clear(
                    dict.get_ascii_string(key_team_all_clear_script()).into(),
                );
            }

            if dict.get_type(key_team_on_destroyed_script()).is_some() {
                prototype.set_script_on_destroyed(
                    dict.get_ascii_string(key_team_on_destroyed_script()).into(),
                );
            }

            if dict.get_type(key_team_destroyed_threshold()).is_some() {
                prototype.set_destroyed_threshold(dict.get_real(key_team_destroyed_threshold()));
            }

            if dict.get_type(key_team_on_unit_destroyed_script()).is_some() {
                prototype.set_script_on_unit_destroyed(
                    dict.get_ascii_string(key_team_on_unit_destroyed_script())
                        .into(),
                );
            }

            if dict.get_type(key_team_production_condition()).is_some() {
                prototype.set_production_condition(
                    dict.get_ascii_string(key_team_production_condition())
                        .into(),
                );
            }

            if dict
                .get_type(key_team_executes_actions_on_create())
                .is_some()
            {
                prototype.set_execute_actions_on_create(
                    dict.get_bool(key_team_executes_actions_on_create()),
                );
            }

            let generic_base =
                NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap_or_default();
            for idx in 0..MAX_GENERIC_SCRIPTS {
                let key_name = format!("{}{}", generic_base, idx);
                let key = NameKeyGenerator::name_to_key(&key_name);
                if dict.get_type(key).is_some() {
                    prototype.set_generic_script(idx, dict.get_ascii_string(key).into());
                } else {
                    prototype.set_generic_script(idx, String::new().into());
                }
            }

            if dict.get_type(key_team_transport()).is_some() {
                prototype
                    .set_transport_unit_type(dict.get_ascii_string(key_team_transport()).into());
            }

            if dict.get_type(key_team_reinforcement_origin()).is_some() {
                prototype.set_start_reinforce_waypoint(
                    dict.get_ascii_string(key_team_reinforcement_origin())
                        .into(),
                );
            }

            if dict.get_type(key_team_starts_full()).is_some() {
                prototype.set_team_starts_full(dict.get_bool(key_team_starts_full()));
            }

            if dict.get_type(key_team_transports_exit()).is_some() {
                prototype.set_transports_exit(dict.get_bool(key_team_transports_exit()));
            }
        }

        self.unique_team_prototype_id += 1;

        let prototype = Arc::new(prototype);
        self.prototypes.insert(name.to_string(), prototype.clone());
        if is_singleton {
            let _ = self.create_inactive_team(name.as_str());
        }
        Some(prototype)
    }

    /// Find team prototype by name
    pub fn find_team_prototype(&self, name: &str) -> Option<Arc<TeamPrototype>> {
        self.prototypes.get(name).cloned()
    }

    /// Find team prototype by ID
    pub fn find_team_prototype_by_id(&self, id: TeamPrototypeID) -> Option<Arc<TeamPrototype>> {
        for prototype in self.prototypes.values() {
            if prototype.get_id() == id {
                return Some(prototype.clone());
            }
        }
        None
    }

    pub fn list_team_prototypes(&self) -> Vec<Arc<TeamPrototype>> {
        self.prototypes.values().cloned().collect()
    }

    /// Snapshot hook: next team instance ID allocator value.
    pub fn get_next_team_id(&self) -> TeamID {
        self.unique_team_id
    }

    /// Snapshot hook: next team prototype ID allocator value.
    pub fn get_next_team_prototype_id(&self) -> TeamPrototypeID {
        self.unique_team_prototype_id
    }

    /// Snapshot hook: restore allocator state from save data.
    pub fn set_next_team_ids(
        &mut self,
        next_team_id: TeamID,
        next_team_prototype_id: TeamPrototypeID,
    ) {
        self.unique_team_id = next_team_id.max(1);
        self.unique_team_prototype_id = next_team_prototype_id.max(1);
    }

    /// Find team by ID
    pub fn find_team_by_id(&self, team_id: TeamID) -> Option<Arc<RwLock<Team>>> {
        self.teams.get(&team_id).cloned()
    }

    fn find_existing_team_by_name(&self, name: &str) -> Option<Arc<RwLock<Team>>> {
        for team in self.teams.values() {
            if let Ok(team_ref) = team.read() {
                if team_ref.get_name() == name {
                    return Some(team.clone());
                }
            }
        }
        None
    }

    fn queue_create_actions_for_prototype(&mut self, prototype: &TeamPrototype) {
        if !prototype.get_execute_actions_on_create() {
            return;
        }
        let production_condition = prototype.get_production_condition();
        if production_condition.is_empty() {
            return;
        }
        self.pending_create_action_scripts
            .push(production_condition.to_string());
    }

    /// Create team from prototype name
    pub fn create_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let team = self.create_inactive_team(name)?;
        team.write().ok()?.set_active();
        Some(team)
    }

    /// Create inactive team
    pub fn create_inactive_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let prototype = self.find_team_prototype(name);
        if prototype.is_none() {
            return None;
        }
        if prototype.as_ref().is_some_and(|p| p.is_singleton()) {
            if let Some(existing) = self.find_existing_team_by_name(name) {
                if let Some(prototype) = prototype.as_deref() {
                    self.queue_create_actions_for_prototype(prototype);
                }
                return Some(existing);
            }
        }

        let team_id = self.unique_team_id;
        self.unique_team_id += 1;

        let team = Arc::new(RwLock::new(Team::new(name.to_string().into(), team_id)));
        if let Some(ref prototype) = prototype {
            if let Ok(mut team_guard) = team.write() {
                team_guard.set_prototype_recruitable(prototype.is_ai_recruitable());
                team_guard.apply_template_script_hooks(prototype);
            }
            let owner_name = prototype.get_owner_name().to_string();
            let owner_player = player_list().read().ok().and_then(|list| {
                if owner_name.is_empty() {
                    None
                } else {
                    list.find_player_by_name(&owner_name)
                }
                .or_else(|| list.get_neutral_player())
            });
            if let Some(owner_player) = owner_player {
                if let (Ok(owner_guard), Ok(mut team_guard)) = (owner_player.read(), team.write()) {
                    team_guard
                        .set_controlling_player_id(Some(owner_guard.get_player_index() as u32));
                }
            }
        }

        self.teams.insert(team_id, team.clone());
        if let Some(prototype) = prototype.as_deref() {
            self.queue_create_actions_for_prototype(prototype);
        }
        Some(team)
    }

    /// Find team by name
    pub fn find_team(&mut self, name: &str) -> Option<Arc<RwLock<Team>>> {
        let prototype = self.find_team_prototype(name)?;
        if let Some(team) = self.find_existing_team_by_name(name) {
            return Some(team);
        }
        if !prototype.is_singleton() {
            return self.create_inactive_team(name);
        }
        None
    }

    /// Find all team instances that were created from the same prototype name.
    ///
    /// C++ Reference: `TeamPrototype::iterate_TeamInstanceList()` used by
    /// `ScriptEngine::executeScript()` when `conditionTeamName` is set.
    pub fn find_team_instances(&self, prototype_name: &str) -> Vec<Arc<RwLock<Team>>> {
        self.teams
            .values()
            .filter_map(|team| {
                let guard = team.read().ok()?;
                (guard.get_name() == prototype_name).then_some(team.clone())
            })
            .collect()
    }

    /// Return all live team instances.
    pub fn get_all_teams(&self) -> Vec<Arc<RwLock<Team>>> {
        self.teams.values().cloned().collect()
    }

    /// Adjust production priority for a team prototype at runtime.
    ///
    /// C++ parity: `TeamPrototype::increaseAIPriorityForSuccess` /
    /// `TeamPrototype::decreaseAIPriorityForFailure` mutate template runtime state.
    pub fn adjust_team_prototype_priority(
        &mut self,
        prototype_name: &str,
        delta: Int,
    ) -> Option<Int> {
        let prototype = self.prototypes.get(prototype_name)?.clone();
        let mut updated = (*prototype).clone();
        let next = updated.get_production_priority().saturating_add(delta);
        updated.set_production_priority(next);
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        Some(next)
    }

    pub fn increase_team_prototype_priority_for_success(
        &mut self,
        prototype_name: &str,
    ) -> Option<Int> {
        let prototype = self.prototypes.get(prototype_name)?.clone();
        let mut updated = (*prototype).clone();
        let delta = updated.get_production_priority_success_increase();
        let next = updated.get_production_priority().saturating_add(delta);
        updated.set_production_priority(next);
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        Some(next)
    }

    pub fn decrease_team_prototype_priority_for_failure(
        &mut self,
        prototype_name: &str,
    ) -> Option<Int> {
        let prototype = self.prototypes.get(prototype_name)?.clone();
        let mut updated = (*prototype).clone();
        let delta = updated.get_production_priority_failure_decrease();
        let next = updated.get_production_priority().saturating_sub(delta);
        updated.set_production_priority(next);
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        Some(next)
    }

    /// Set runtime attack-priority set name for a team prototype.
    pub fn set_team_prototype_attack_priority_name(
        &mut self,
        prototype_name: &str,
        attack_priority_name: &str,
    ) -> bool {
        let Some(prototype) = self.prototypes.get(prototype_name).cloned() else {
            return false;
        };
        let mut updated = (*prototype).clone();
        updated.set_attack_priority_name(AsciiString::from(attack_priority_name));
        self.prototypes
            .insert(prototype_name.to_string(), Arc::new(updated));
        true
    }

    /// Notify that team is about to be deleted
    pub fn team_about_to_be_deleted(&mut self, team_id: TeamID) {
        self.teams.remove(&team_id);
    }

    fn drain_pending_create_action_scripts(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_create_action_scripts)
    }

    fn drain_pending_generic_script_evals(&mut self) -> Vec<PendingTeamGenericScriptEval> {
        std::mem::take(&mut self.pending_generic_script_evals)
    }
}

fn execute_pending_team_create_action_scripts(script_names: Vec<String>) {
    if script_names.is_empty() {
        return;
    }

    // C++ createInactiveTeam: friend_executeAction(action) with NULL team.
    let script_engine = get_script_engine();
    for script_name in script_names {
        let action = {
            let Ok(engine_guard) = script_engine.read() else {
                continue;
            };
            engine_guard
                .as_ref()
                .and_then(|engine| engine.find_script_clone_by_name(&script_name))
                .and_then(|script| script.get_action().cloned())
        };
        let Some(action) = action else {
            continue;
        };
        if let Ok(mut eng) = script_engine.write() {
            if let Some(e) = eng.as_mut() {
                e.friend_execute_action(&action, None);
            }
        }
    }
}

fn evaluate_generic_script_conditions(
    script: &mut Script,
    evaluator: &ScriptEvaluator,
    current_player_name: Option<&str>,
) -> Result<bool, String> {
    if !script.is_active() {
        return Ok(false);
    }

    let difficulty = current_player_name
        .and_then(|player_name| {
            player_list()
                .read()
                .ok()
                .and_then(|list| list.find_player_by_name(player_name))
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|player| player.get_player_difficulty())
                })
        })
        .unwrap_or(crate::player::GameDifficulty::Normal);

    match difficulty {
        crate::player::GameDifficulty::Easy if !script.easy => return Ok(false),
        crate::player::GameDifficulty::Normal if !script.normal => return Ok(false),
        crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal
            if !script.hard =>
        {
            return Ok(false);
        }
        _ => {}
    }

    let current_frame = crate::helpers::TheGameLogic::get_frame();
    if current_frame < script.frame_to_evaluate_at {
        return Ok(false);
    }

    if script.delay_evaluation_seconds > 0 {
        script.frame_to_evaluate_at = current_frame
            + (script.delay_evaluation_seconds as u32) * (LOGICFRAMES_PER_SECOND as u32);
    }

    let Some(or_condition) = script.condition.as_deref_mut() else {
        return Ok(false);
    };

    evaluator
        .evaluate_or_condition(or_condition)
        .map_err(|err| err.to_string())
}

fn execute_pending_team_generic_script_evals(script_evals: Vec<PendingTeamGenericScriptEval>) {
    if script_evals.is_empty() {
        return;
    }

    let script_engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(script_engine.clone());

    for pending in script_evals {
        let Some(mut script) = pending
            .prototype
            .take_or_load_generic_script_runtime(pending.script_index)
        else {
            if let Ok(mut team_guard) = pending.team.write() {
                team_guard.disable_generic_script_attempt(pending.script_index);
            }
            continue;
        };

        let saved_context = match script_engine.write() {
            Ok(mut engine_guard) => engine_guard.as_mut().map(|engine| {
                engine.set_external_eval_context(
                    pending.current_player_name.clone(),
                    Some(pending.team_name.clone()),
                )
            }),
            Err(_) => None,
        };

        let Some(saved_context) = saved_context else {
            pending
                .prototype
                .store_generic_script_runtime(pending.script_index, Some(script));
            continue;
        };

        let eval_result = evaluate_generic_script_conditions(
            &mut script,
            &evaluator,
            pending.current_player_name.as_deref(),
        );

        if let Ok(mut engine_guard) = script_engine.write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.restore_external_eval_context(saved_context);
            }
        }

        match eval_result {
            Ok(condition_true) => {
                if condition_true {
                    if let Some(action) = script.get_action().cloned() {
                        // C++ friend_executeAction(action, this) — team-scoped.
                        if let Ok(mut eng) = script_engine.write() {
                            if let Some(e) = eng.as_mut() {
                                e.friend_execute_action(&action, Some(pending.team_name.as_str()));
                            }
                        }
                    }

                    if script.is_one_shot() {
                        if let Ok(mut team_guard) = pending.team.write() {
                            team_guard.disable_generic_script_attempt(pending.script_index);
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!(
                    "Team generic script '{}' evaluation failed for team '{}': {}",
                    pending.script_name,
                    pending.team_name,
                    err
                );
            }
        }

        pending
            .prototype
            .store_generic_script_runtime(pending.script_index, Some(script));
    }
}

/// RAII guard for TeamFactory that flushes pending create-actions after unlock.
pub struct TeamFactoryGuard<'a> {
    inner: Option<MutexGuard<'a, TeamFactory>>,
}

impl<'a> TeamFactoryGuard<'a> {
    fn new(inner: MutexGuard<'a, TeamFactory>) -> Self {
        Self { inner: Some(inner) }
    }
}

impl Deref for TeamFactoryGuard<'_> {
    type Target = TeamFactory;

    fn deref(&self) -> &Self::Target {
        self.inner
            .as_deref()
            .expect("TeamFactoryGuard missing inner")
    }
}

impl DerefMut for TeamFactoryGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner
            .as_deref_mut()
            .expect("TeamFactoryGuard missing inner")
    }
}

impl Drop for TeamFactoryGuard<'_> {
    fn drop(&mut self) {
        let Some(mut guard) = self.inner.take() else {
            return;
        };
        let pending = guard.drain_pending_create_action_scripts();
        let pending_generic = guard.drain_pending_generic_script_evals();
        drop(guard);
        execute_pending_team_create_action_scripts(pending);
        execute_pending_team_generic_script_evals(pending_generic);
    }
}

/// Mutex wrapper that returns TeamFactoryGuard with post-unlock flush semantics.
pub struct TeamFactoryMutex {
    inner: Mutex<TeamFactory>,
}

impl TeamFactoryMutex {
    fn new() -> Self {
        Self {
            inner: Mutex::new(TeamFactory::new()),
        }
    }

    pub fn lock(&self) -> LockResult<TeamFactoryGuard<'_>> {
        match self.inner.lock() {
            Ok(guard) => Ok(TeamFactoryGuard::new(guard)),
            Err(poisoned) => Err(PoisonError::new(TeamFactoryGuard::new(
                poisoned.into_inner(),
            ))),
        }
    }
}

/// Global team factory instance (matching C++ TheTeamFactory)
static THE_TEAM_FACTORY: OnceLock<TeamFactoryMutex> = OnceLock::new();

/// Get global team factory instance
pub fn get_team_factory() -> &'static TeamFactoryMutex {
    THE_TEAM_FACTORY.get_or_init(TeamFactoryMutex::new)
}

/// Convenience alias for C++ compatibility
pub use get_team_factory as TheTeamFactory;

/// Extension trait for Arc<RwLock<Team>> to provide helper methods
pub trait TeamArcExt {
    fn get_relationship(&self, that_team: &Team) -> Relationship;
}

impl TeamArcExt for Arc<RwLock<Team>> {
    /// Get relationship between this team and another team
    fn get_relationship(&self, that_team: &Team) -> Relationship {
        if let Ok(guard) = self.read() {
            guard.get_relationship(that_team)
        } else {
            Relationship::Neutral
        }
    }
}

// Note: rhai::Locked<T> is an alias for RwLock<T>, so the impl above covers both

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_team_parses_unit_and_reinforcement_fields_from_dict() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();

        dict.set_ascii_string(key_team_unit_type1(), "AmericaInfantryRanger");
        dict.set_int(key_team_unit_min_count1(), 1);
        dict.set_int(key_team_unit_max_count1(), 3);

        dict.set_ascii_string(key_team_unit_type2(), "AmericaVehicleHumvee");
        dict.set_int(key_team_unit_min_count2(), 2);
        dict.set_int(key_team_unit_max_count2(), 4);

        dict.set_int(key_team_max_instances(), 5);
        dict.set_ascii_string(key_team_transport(), "AmericaJetCargoPlane");
        dict.set_ascii_string(key_team_reinforcement_origin(), "ReinforceStart01");
        dict.set_bool(key_team_starts_full(), true);
        dict.set_bool(key_team_transports_exit(), false);

        let prototype = factory
            .init_team(
                AsciiString::from("TestTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let units = prototype.units_info();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].unit_thing_name, "AmericaInfantryRanger");
        assert_eq!(units[0].min_units, 1);
        assert_eq!(units[0].max_units, 3);
        assert_eq!(units[1].unit_thing_name, "AmericaVehicleHumvee");
        assert_eq!(units[1].min_units, 2);
        assert_eq!(units[1].max_units, 4);

        assert_eq!(prototype.get_max_instances(), 5);
        assert_eq!(
            prototype.get_transport_unit_type().as_str(),
            "AmericaJetCargoPlane"
        );
        assert_eq!(
            prototype.get_start_reinforce_waypoint().as_str(),
            "ReinforceStart01"
        );
        assert!(prototype.get_team_starts_full());
        assert!(!prototype.get_transports_exit());
    }

    #[test]
    fn create_inactive_team_requires_existing_prototype() {
        let mut factory = TeamFactory::new();
        assert!(factory.create_inactive_team("MissingTeam").is_none());
        assert!(factory.create_team("MissingTeam").is_none());
    }

    #[test]
    fn find_team_creates_missing_non_singleton_instance() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "AutoCreateTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let _ = factory
            .init_team(
                AsciiString::from("AutoCreateTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert!(factory.get_all_teams().is_empty());

        let team = factory
            .find_team("AutoCreateTeam")
            .expect("find_team should auto-create for non-singleton prototype");
        let team_name = team.read().expect("team read lock").get_name().to_string();

        assert_eq!(team_name, "AutoCreateTeam");
        assert_eq!(factory.get_all_teams().len(), 1);
    }

    #[test]
    fn create_team_marks_instance_active() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ActiveTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let _ = factory
            .init_team(
                AsciiString::from("ActiveTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let team = factory
            .create_team("ActiveTeam")
            .expect("create_team should create from prototype");
        assert!(team.read().expect("team read lock").is_active());
    }

    #[test]
    fn init_team_parses_and_defaults_ai_recruitable_flags() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "RecruitableTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_bool(key_team_is_ai_recruitable(), true);
        dict.set_bool(key_team_is_base_defense(), true);

        let prototype = factory
            .init_team(
                AsciiString::from("RecruitableTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert!(prototype.is_ai_recruitable());
        assert!(prototype.is_base_defense());

        let mut default_dict = Dict::new();
        default_dict.set_ascii_string(key_team_name(), "DefaultFlagsTeam");
        default_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        let default_prototype = factory
            .init_team(
                AsciiString::from("DefaultFlagsTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&default_dict),
            )
            .expect("default prototype should be created");
        assert!(!default_prototype.is_ai_recruitable());
        assert!(!default_prototype.is_base_defense());
    }

    #[test]
    fn init_team_parses_create_action_production_fields() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ActionTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_production_condition(), "ScriptCreateTeam");
        dict.set_bool(key_team_executes_actions_on_create(), true);

        let prototype = factory
            .init_team(
                AsciiString::from("ActionTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");
        assert_eq!(
            prototype.get_production_condition().as_str(),
            "ScriptCreateTeam"
        );
        assert!(prototype.get_execute_actions_on_create());

        let mut default_dict = Dict::new();
        default_dict.set_ascii_string(key_team_name(), "ActionDefaultsTeam");
        default_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        let default_prototype = factory
            .init_team(
                AsciiString::from("ActionDefaultsTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&default_dict),
            )
            .expect("default prototype should be created");
        assert!(default_prototype.get_production_condition().is_empty());
        assert!(!default_prototype.get_execute_actions_on_create());
    }

    #[test]
    fn evaluate_production_condition_empty_is_false() {
        let proto = TeamPrototype::new("NoCondTeam".into());
        // C++: empty productionCondition → always false thereafter.
        assert!(!proto.evaluate_production_condition());
        assert!(!proto.evaluate_production_condition());
    }

    #[test]
    fn evaluate_production_condition_missing_script_is_false() {
        let mut proto = TeamPrototype::new("MissingScriptTeam".into());
        proto.set_production_condition("DoesNotExist_ScriptXYZ".into());
        assert!(!proto.evaluate_production_condition());
    }

    #[test]
    fn flush_team_scripts_use_run_script_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/team.rs"));
        let i = src
            .find("pub fn flush_pending_team_script_events")
            .expect("flush");
        let w = &src[i..src.len().min(i + 900)];
        assert!(
            w.contains("run_script")
                && w.contains("Some(event.team_name.as_str())")
                && !w.contains("append_sequential_script"),
            "Team event flush must runScript(name, team) like C++ updateState"
        );
    }

    #[test]
    fn team_scripts_use_friend_execute_like_cpp() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/team.rs"));
        let create = src
            .find("fn execute_pending_team_create_action_scripts")
            .expect("create scripts");
        let create_w = &src[create..src.len().min(create + 1200)];
        assert!(
            create_w.contains("friend_execute_action")
                && create_w.contains("None")
                && !create_w.contains("ScriptEvaluator::new"),
            "createInactiveTeam path must friend_executeAction with NULL team"
        );
        let gen = src
            .find("fn execute_pending_team_generic_script_evals")
            .expect("generic scripts");
        let gen_w = &src[gen..src.len().min(gen + 3500)];
        assert!(
            gen_w.contains("friend_execute_action")
                && gen_w.contains("pending.team_name")
                && !gen_w.contains("evaluator.execute_action_sequence"),
            "updateGenericScripts path must friend_executeAction with team"
        );
    }

    #[test]
    fn create_inactive_team_queues_create_action_script() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "QueueActionTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_production_condition(), "ScriptQueueActionTeam");
        dict.set_bool(key_team_executes_actions_on_create(), true);

        let _ = factory
            .init_team(
                AsciiString::from("QueueActionTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let _ = factory
            .create_inactive_team("QueueActionTeam")
            .expect("team should be created");

        let queued = factory.drain_pending_create_action_scripts();
        assert_eq!(queued, vec!["ScriptQueueActionTeam".to_string()]);
    }

    #[test]
    fn team_priority_success_and_failure_use_template_deltas() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "PriorityTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_int(key_team_production_priority(), 10);
        dict.set_int(key_team_production_priority_success_increase(), 3);
        dict.set_int(key_team_production_priority_failure_decrease(), 2);

        let prototype = factory
            .init_team(
                AsciiString::from("PriorityTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");
        assert_eq!(prototype.get_production_priority(), 10);
        assert_eq!(prototype.get_production_priority_success_increase(), 3);
        assert_eq!(prototype.get_production_priority_failure_decrease(), 2);

        let increased = factory
            .increase_team_prototype_priority_for_success("PriorityTeam")
            .expect("prototype should exist");
        assert_eq!(increased, 13);

        let decreased = factory
            .decrease_team_prototype_priority_for_failure("PriorityTeam")
            .expect("prototype should exist");
        assert_eq!(decreased, 11);
    }

    #[test]
    fn created_team_inherits_prototype_recruitable_flag() {
        let mut factory = TeamFactory::new();

        let mut default_dict = Dict::new();
        default_dict.set_ascii_string(key_team_name(), "DefaultRecruitableTeam");
        default_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        let _ = factory
            .init_team(
                AsciiString::from("DefaultRecruitableTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&default_dict),
            )
            .expect("default prototype should be created");
        let default_team = factory
            .create_inactive_team("DefaultRecruitableTeam")
            .expect("default team should be created");
        assert!(!default_team
            .read()
            .expect("team read lock")
            .is_recruitable());

        let mut recruitable_dict = Dict::new();
        recruitable_dict.set_ascii_string(key_team_name(), "RecruitableTeamTrue");
        recruitable_dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        recruitable_dict.set_bool(key_team_is_ai_recruitable(), true);
        let _ = factory
            .init_team(
                AsciiString::from("RecruitableTeamTrue"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&recruitable_dict),
            )
            .expect("recruitable prototype should be created");
        let recruitable_team = factory
            .create_inactive_team("RecruitableTeamTrue")
            .expect("recruitable team should be created");
        assert!(recruitable_team
            .read()
            .expect("team read lock")
            .is_recruitable());
    }

    #[test]
    fn init_team_parses_extended_template_behavior_and_scripts() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ExtendedTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_bool(key_team_is_perimeter_defense(), true);
        dict.set_bool(key_team_auto_reinforce(), true);
        dict.set_int(key_team_aggressiveness(), 2);
        dict.set_bool(key_team_transports_return(), true);
        dict.set_bool(key_team_avoid_threats(), true);
        dict.set_bool(key_team_attack_common_target(), true);
        dict.set_ascii_string(key_team_on_create_script(), "TeamCreateHook");
        dict.set_ascii_string(key_team_on_idle_script(), "TeamIdleHook");
        dict.set_int(key_team_initial_idle_frames(), 45);
        dict.set_ascii_string(key_team_enemy_sighted_script(), "TeamEnemySightedHook");
        dict.set_ascii_string(key_team_all_clear_script(), "TeamAllClearHook");
        dict.set_ascii_string(key_team_on_destroyed_script(), "TeamDestroyedHook");
        dict.set_real(key_team_destroyed_threshold(), 0.5);
        dict.set_ascii_string(key_team_on_unit_destroyed_script(), "TeamUnitDestroyedHook");

        let prototype = factory
            .init_team(
                AsciiString::from("ExtendedTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert!(prototype.is_perimeter_defense());
        assert!(prototype.automatically_reinforce());
        assert_eq!(
            prototype.get_initial_team_attitude(),
            AttitudeType::Aggressive
        );
        assert!(prototype.transports_return());
        assert!(prototype.avoid_threats());
        assert!(prototype.attack_common_target());
        assert_eq!(prototype.get_script_on_create().as_str(), "TeamCreateHook");
        assert_eq!(prototype.get_script_on_idle().as_str(), "TeamIdleHook");
        assert_eq!(prototype.get_initial_idle_frames(), 45);
        assert_eq!(
            prototype.get_script_on_enemy_sighted().as_str(),
            "TeamEnemySightedHook"
        );
        assert_eq!(
            prototype.get_script_on_all_clear().as_str(),
            "TeamAllClearHook"
        );
        assert_eq!(
            prototype.get_script_on_destroyed().as_str(),
            "TeamDestroyedHook"
        );
        assert!((prototype.get_destroyed_threshold() - 0.5).abs() < f32::EPSILON);
        assert_eq!(
            prototype.get_script_on_unit_destroyed().as_str(),
            "TeamUnitDestroyedHook"
        );
    }

    #[test]
    fn attitude_from_ini_matches_cpp_values() {
        assert_eq!(AttitudeType::from_ini(-2), AttitudeType::Sleep);
        assert_eq!(AttitudeType::from_ini(-1), AttitudeType::Passive);
        assert_eq!(AttitudeType::from_ini(0), AttitudeType::Normal);
        assert_eq!(AttitudeType::from_ini(1), AttitudeType::Alert);
        assert_eq!(AttitudeType::from_ini(2), AttitudeType::Aggressive);
        assert_eq!(AttitudeType::from_ini(3), AttitudeType::Invalid);
        assert_eq!(AttitudeType::from_ini(99), AttitudeType::Normal);
    }

    #[test]
    fn team_state_and_death_queue_team_script_events() {
        let _ = drain_pending_team_script_events();

        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "ScriptHookTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");
        dict.set_ascii_string(key_team_on_create_script(), "OnCreateTeamScript");
        dict.set_ascii_string(
            key_team_on_unit_destroyed_script(),
            "OnUnitDestroyedTeamScript",
        );

        let _ = factory
            .init_team(
                AsciiString::from("ScriptHookTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let team = factory
            .create_team("ScriptHookTeam")
            .expect("team should be created");

        {
            let mut guard = team.write().expect("team write lock");
            guard.update_state();
            guard.notify_team_of_object_death();
        }

        let queued = drain_pending_team_script_events();
        assert_eq!(queued.len(), 2);
        assert_eq!(queued[0].team_name, "ScriptHookTeam");
        assert_eq!(queued[0].script_name, "OnCreateTeamScript");
        assert_eq!(queued[1].team_name, "ScriptHookTeam");
        assert_eq!(queued[1].script_name, "OnUnitDestroyedTeamScript");
    }

    #[test]
    fn update_removes_empty_active_non_singleton_teams() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "TempTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let _ = factory
            .init_team(
                AsciiString::from("TempTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        let _team = factory
            .create_team("TempTeam")
            .expect("team should be created");
        assert_eq!(factory.get_all_teams().len(), 1);

        factory.update();
        assert!(factory.get_all_teams().is_empty());
    }

    #[test]
    fn init_team_parses_generic_script_hook_slots() {
        let mut factory = TeamFactory::new();
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), "GenericHookTeam");
        dict.set_ascii_string(key_team_owner(), "PlyrCivilian");

        let base =
            NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap_or_default();
        let hook0 = NameKeyGenerator::name_to_key(&format!("{}0", base));
        let hook3 = NameKeyGenerator::name_to_key(&format!("{}3", base));
        dict.set_ascii_string(hook0, "GenericHookScript0");
        dict.set_ascii_string(hook3, "GenericHookScript3");

        let prototype = factory
            .init_team(
                AsciiString::from("GenericHookTeam"),
                AsciiString::from("PlyrCivilian"),
                false,
                Some(&dict),
            )
            .expect("prototype should be created");

        assert_eq!(
            prototype
                .get_generic_script(0)
                .map(|s| s.as_str())
                .unwrap_or_default(),
            "GenericHookScript0"
        );
        assert_eq!(
            prototype
                .get_generic_script(3)
                .map(|s| s.as_str())
                .unwrap_or_default(),
            "GenericHookScript3"
        );
        assert!(prototype
            .get_generic_script(1)
            .map(|s| s.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn generic_script_eval_disables_slot_when_script_missing() {
        let team = Arc::new(RwLock::new(Team::new(
            AsciiString::from("GenericEvalTeam"),
            1,
        )));
        let mut prototype = TeamPrototype::new(AsciiString::from("GenericEvalTeam"));
        prototype.set_generic_script(0, AsciiString::from("DefinitelyMissingScript"));
        let prototype = Arc::new(prototype);

        assert!(team
            .read()
            .expect("team read lock")
            .should_attempt_generic_script(0));

        execute_pending_team_generic_script_evals(vec![PendingTeamGenericScriptEval {
            team: team.clone(),
            prototype,
            team_name: "GenericEvalTeam".to_string(),
            script_name: "DefinitelyMissingScript".to_string(),
            script_index: 0,
            current_player_name: None,
        }]);

        assert!(!team
            .read()
            .expect("team read lock")
            .should_attempt_generic_script(0));
    }

    #[test]
    fn set_team_target_object_requires_ai_controller() {
        let mut team = Team::new(AsciiString::from("TargetTeam"), 1);
        team.set_team_target_object(42);
        assert_eq!(team.get_team_target_object(), INVALID_ID);
    }

    #[test]
    fn get_team_target_object_rejects_missing_object() {
        let mut team = Team::new(AsciiString::from("TargetTeam"), 1);
        team.common_attack_target = 999_999;
        assert_eq!(team.get_team_target_object(), INVALID_ID);
    }

    #[test]
    fn get_targetable_count_ignores_missing_member_entries() {
        let mut team = Team::new(AsciiString::from("TargetableTeam"), 1);
        team.add_member(111_111);
        team.add_member(222_222);
        assert_eq!(team.get_targetable_count(), 0);
    }

    #[test]
    fn set_override_team_relationship_ignores_invalid_id() {
        let mut team = Team::new(AsciiString::from("RelationsTeam"), 1);
        team.set_override_team_relationship(TEAM_ID_INVALID, Relationship::Enemies);
        assert!(team.team_relations.is_none());
    }

    #[test]
    fn remove_override_team_relationship_invalid_id_clears_overrides() {
        let mut team = Team::new(AsciiString::from("RelationsTeam"), 1);
        team.set_override_team_relationship(2, Relationship::Enemies);
        team.set_override_team_relationship(3, Relationship::Allies);
        assert!(team.remove_override_team_relationship(TEAM_ID_INVALID));
        assert!(team
            .team_relations
            .as_ref()
            .map(|m| m.map.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn remove_override_player_relationship_invalid_id_clears_overrides() {
        let mut team = Team::new(AsciiString::from("RelationsTeam"), 1);
        team.set_override_player_relationship(0, Relationship::Enemies);
        team.set_override_player_relationship(1, Relationship::Allies);
        assert!(team.remove_override_player_relationship(crate::player::PLAYER_INDEX_INVALID));
        assert!(team
            .player_relations
            .as_ref()
            .map(|m| m.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn delete_team_does_not_deactivate_team() {
        let mut team = Team::new(AsciiString::from("DeleteTeam"), 1);
        team.set_active();
        assert!(team.is_active());
        team.delete_team(false);
        assert!(team.is_active());
    }

    #[test]
    fn team_area_events_remain_visible_on_next_script_frame() {
        use crate::polygon_trigger::PolygonTrigger;
        use crate::scripting::engine::{get_area_tracker, get_event_manager};
        use crate::scripting::events::TriggerArea;
        use crate::system::game_logic::get_game_logic;

        let area_name = "TeamAreaPreviousFrameParity";
        let object_id = 0x00FE_DCBA;
        let tracker = get_area_tracker();
        let _ = tracker.unregister_area(area_name);
        tracker
            .register_area(TriggerArea::new_rectangular(
                area_name.to_string(),
                [0.0, 0.0, 0.0],
                0.0,
                0.0,
                10.0,
                10.0,
            ))
            .expect("register test area");

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(41);
        tracker
            .update_object_position_sync(object_id, [5.0, 5.0, 0.0], &get_event_manager())
            .expect("record enter");

        let trigger = PolygonTrigger::new(1, AsciiString::from(area_name), Vec::new());
        assert!(Team::object_did_enter(object_id, &trigger));

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(42);
        assert!(Team::object_did_enter(object_id, &trigger));

        tracker
            .update_object_position_sync(object_id, [20.0, 20.0, 0.0], &get_event_manager())
            .expect("record exit");
        assert!(Team::object_did_exit(object_id, &trigger));

        get_game_logic()
            .lock()
            .expect("game logic lock")
            .set_current_frame(43);
        assert!(Team::object_did_exit(object_id, &trigger));

        let _ = tracker.unregister_area(area_name);
    }
}

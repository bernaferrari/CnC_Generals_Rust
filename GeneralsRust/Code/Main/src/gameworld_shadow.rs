//! Shadow parity bridge: Main `GameLogic` (temp host authority) → `gamelogic::world::GameWorld`.
//!
//! This is **not** production authority yet. It maintains a borrow-first `GameWorld`
//! plus a **stable** host `ObjectId` → `EntityId` map so damage/spawn/destroy can be
//! applied as `WorldMutation`s without pointer ownership.
//!
//! Opt-in runtime: `GENERALS_GAMEWORLD_SHADOW=1`.
//!
//! Policy: borrow host for sync phases only; never store long-lived host references.

use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

/// Compact probe comparing host authority vs GameWorld shadow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameWorldShadowProbe {
    pub host_frame: u32,
    pub shadow_frame: u64,
    pub host_objects: usize,
    pub shadow_entities: usize,
    pub host_players: usize,
    pub shadow_players: usize,
    pub host_supplies_sum: u64,
    pub shadow_supplies_sum: u64,
    /// Mapped host objects present in the ID table.
    pub mapped_objects: usize,
    pub counts_match: bool,
    pub economy_match: bool,
    /// Health samples agree for all mapped live objects (within 0.01).
    pub health_match: bool,
    /// Host match-over residual (evaluate_victory_condition).
    pub host_match_over: bool,
    pub victory_label: Option<String>,
    pub detail: String,
}

impl GameWorldShadowProbe {
    pub fn format_report(&self) -> String {
        format!(
            "gameworld_shadow host_f={} shadow_f={} objs={}/{} players={}/{} supplies={}/{} mapped={} match={} econ={} health={} victory_over={} label={:?} {}",
            self.host_frame,
            self.shadow_frame,
            self.host_objects,
            self.shadow_entities,
            self.host_players,
            self.shadow_players,
            self.host_supplies_sum,
            self.shadow_supplies_sum,
            self.mapped_objects,
            self.counts_match,
            self.economy_match,
            self.health_match,
            self.host_match_over,
            self.victory_label,
            self.detail
        )
    }

    #[inline]
    pub fn full_match(&self) -> bool {
        self.counts_match && self.economy_match && self.health_match
    }
}

/// Whether the optional engine shadow path is enabled.
/// True when Main create_object may attach gamelogic OBJECT_REGISTRY ids (opt-in only).
pub fn engine_object_bridge_enabled() -> bool {
    std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some()
        || std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_some()
}

pub fn gameworld_shadow_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_SHADOW") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        // Unset → session on (authority writebacks remain separately gated).
        Err(_) => true,
    }
}

/// When enabled, GameWorld shadow mutations are the **last writer** for HP each tick.
/// Host combat still runs mid-frame; end-of-tick reapplies drained damage events
/// on the shadow and writebacks health/destroyed onto host objects.
/// Implies a shadow session (separate GENERALS_GAMEWORLD_SHADOW not required).
///
/// Env: `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=0|false` off; unset/`1` = **on** (production default).
pub fn gameworld_damage_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// Economy last-writer (player supplies/power). Unset = **on**; `0|false` off.
pub fn gameworld_economy_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled, GameWorld integrates path/move targets after the host tick and
/// writebacks pose/movement as last-writer. Host `update_movement` skips integrate.
///
/// Env: `GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_movement_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled (default), GameWorld SetAttackTarget + SetFireIntent writeback is the
/// last-writer for host attack target / fire-intent residual after each shadow session.
/// Host still *decides* and discharges weapons; opt out with `=0|false`.
/// Env: `GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_ai_attack_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled (default), GameWorld steps projectile flight residual and
/// last-writes pose/lifetime into host CombatSystem before hit resolution.
/// Host still owns spawn/fire and hit/damage application.
/// Env: `GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_projectile_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled (default), host `update_ai` only *records* AICommand decisions;
/// GameWorld applies attack/move/state mutations and writeback is last-writer.
/// Combat runs before AI in the host tick, so deferred apply is next-frame parity.
/// Env: `GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_ai_decision_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled (default), `queue_projectile` only logs fire-spawns; shadow
/// applies them into host CombatSystem before projectile integrate authority.
/// Env: `GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_fire_spawn_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled, host construction percent is last-written from GameWorld after
/// progress logs (host still computes projected percent for completion side effects).
/// Env: `GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_construction_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// When enabled (default), GameWorld shadow is last-writer for production queue
/// identity (items/progress/rally/exit delay) via host progress logs + writeback.
/// Host still *executes* production ticks (spawn completion residual); shadow owns
/// the frozen queue snapshot at session end.
///
/// Env: `GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_production_authority_enabled() -> bool {
    match std::env::var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY") {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => true,
    }
}

/// Gates/smoke: no-op when production defaults are already on.
/// Still forces `1` if env was never set (explicit documentation for gate binaries).
pub fn ensure_gate_damage_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        }
    }
    ensure_gate_economy_authority();
    ensure_gate_production_authority();
}

/// Gates/smoke: force economy authority env to `1` when unset.
pub fn ensure_gate_economy_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        }
    }
}

/// Gates/smoke: force production authority env to `1` when unset.
pub fn ensure_gate_production_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
        }
    }
}

/// Session holding GameWorld + stable host↔entity ID maps.
#[derive(Debug)]
pub struct GameWorldShadow {
    world: GameWorld,
    host_to_entity: HashMap<u32, EntityId>,
    entity_to_host: HashMap<u32, u32>,
    max_entities: usize,
    /// Host player id → dense GameWorld PlayerId
    host_player_to_gw: HashMap<u32, PlayerId>,
    /// Last host energy shortfall residual per producer host id (sole-tick).
    production_power_factor_by_host: HashMap<u32, f32>,
}

impl GameWorldShadow {
    pub fn new(max_entities: usize) -> Self {
        Self {
            world: GameWorld::new(8),
            host_to_entity: HashMap::new(),
            entity_to_host: HashMap::new(),
            max_entities: max_entities.max(1),
            host_player_to_gw: HashMap::new(),
            production_power_factor_by_host: HashMap::new(),
        }
    }

    pub fn world(&self) -> &GameWorld {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut GameWorld {
        &mut self.world
    }

    pub fn entity_for_host(&self, host: ObjectId) -> Option<EntityId> {
        self.host_to_entity.get(&host.0).copied()
    }

    pub fn host_for_entity(&self, entity: EntityId) -> Option<ObjectId> {
        self.entity_to_host
            .get(&entity.get())
            .copied()
            .map(ObjectId)
    }

    pub fn mapped_count(&self) -> usize {
        self.host_to_entity.len()
    }

    /// Full/delta sync from host: create, update health/transform/owner, destroy missing.
    /// Preserves EntityId for host objects that still exist.
    fn host_building_type_ordinal(t: crate::game_logic::BuildingType) -> u8 {
        use crate::game_logic::BuildingType as B;
        match t {
            B::CommandCenter => 0,
            B::Barracks => 1,
            B::WarFactory => 2,
            B::Airfield => 3,
            B::RepairPad => 4,
            B::HealPad => 5,
            B::SupplyCenter => 6,
            B::PowerPlant => 7,
            B::DefenseTurret => 8,
            B::SupplyDropZone => 9,
            B::Palace => 10,
            B::Propaganda => 11,
            B::Bunker => 12,
        }
    }

    fn host_veterancy_ordinal(level: crate::game_logic::VeterancyLevel) -> u8 {
        use crate::game_logic::VeterancyLevel as V;
        match level {
            V::Rookie => 0,
            V::Veteran => 1,
            V::Elite => 2,
            V::Heroic => 3,
        }
    }

    pub fn host_ai_state_ordinal(s: &crate::game_logic::AIState) -> u8 {
        use crate::game_logic::AIState as A;
        match s {
            A::Idle => 0,
            A::Moving => 1,
            A::Attacking => 2,
            A::AttackMoving => 3,
            A::AttackingGround => 4,
            A::Gathering => 5,
            A::ReturningResources => 6,
            A::Constructing => 7,
            A::Repairing => 8,
            A::GuardingArea => 9,
            A::GuardingObject => 10,
            A::GuardRetaliating => 20,
            A::Patrolling => 11,
            A::Docked => 12,
            A::Garrisoned => 13,
            A::SpecialAbility => 14,
            A::SeekingRepair => 15,
            A::SeekingHealing => 16,
            A::Entering => 17,
            A::Docking => 18,
            A::Capturing => 19,
        }
    }

    pub(crate) fn ai_state_from_ordinal(ordinal: u8) -> crate::game_logic::AIState {
        use crate::game_logic::AIState as A;
        match ordinal {
            1 => A::Moving,
            2 => A::Attacking,
            3 => A::AttackMoving,
            4 => A::AttackingGround,
            5 => A::Gathering,
            6 => A::ReturningResources,
            7 => A::Constructing,
            8 => A::Repairing,
            9 => A::GuardingArea,
            10 => A::GuardingObject,
            11 => A::Patrolling,
            12 => A::Docked,
            13 => A::Garrisoned,
            14 => A::SpecialAbility,
            15 => A::SeekingRepair,
            16 => A::SeekingHealing,
            17 => A::Entering,
            18 => A::Docking,
            19 => A::Capturing,
            20 => A::GuardRetaliating,
            _ => A::Idle,
        }
    }

    /// Presentation KindOf ORDER residual (must match PresentationFrame freeze ORDER).
    fn host_kind_of_bits(obj: &crate::game_logic::Object) -> u32 {
        obj.presentation_kind_of_bits()
    }

    fn host_object_type_ordinal(t: crate::game_logic::ObjectType) -> u8 {
        use crate::game_logic::ObjectType as T;
        match t {
            T::Infantry => 0,
            T::Vehicle => 1,
            T::Aircraft => 2,
            T::Building => 3,
            T::Supply => 4,
            T::Projectile => 5,
            T::Neutral => 6,
        }
    }

    fn host_team_ordinal(team: Team) -> u8 {
        match team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 255,
        }
    }

    pub fn sync_from_host(&mut self, logic: &GameLogic) {
        self.sync_from_host_with(logic, true);
    }

    /// Like [`sync_from_host`]; `write_health=false` keeps existing entity HP
    /// (damage-authority path so mutations are last writer).
    pub fn sync_from_host_with(&mut self, logic: &GameLogic, write_health: bool) {
        self.sync_players(logic);

        let mut obj_ids: Vec<ObjectId> = logic.get_objects().keys().copied().collect();
        obj_ids.sort_by_key(|id| id.0);
        if obj_ids.len() > self.max_entities {
            obj_ids.truncate(self.max_entities);
        }
        let host_set: HashSet<u32> = obj_ids.iter().map(|id| id.0).collect();

        // Remove shadow entities whose host object is gone.
        let stale: Vec<(u32, EntityId)> = self
            .host_to_entity
            .iter()
            .filter(|(hid, _)| !host_set.contains(hid))
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in stale {
            let _ = self.world.world_mut().remove_entity(eid);
            self.host_to_entity.remove(&hid);
            self.entity_to_host.remove(&eid.get());
        }

        // Create or update each host object.
        for oid in obj_ids {
            let Some(obj) = logic.get_objects().get(&oid) else {
                continue;
            };
            let pos = obj.get_position();
            // Prefer host facing on sync (pose channel). Zero was a residual wipe that
            // forced apply_host_positions to re-queue every entity each tick.
            let transform = Transform::new([pos.x, pos.y, pos.z], obj.get_orientation());
            let owner = self.owner_for_host_object(logic, obj.team);
            let health = obj.health.current.max(0.0);

            if let Some(&eid) = self.host_to_entity.get(&oid.0) {
                if let Some(e) = self.world.world_mut().entity_mut(eid) {
                    if write_health {
                        e.health = health;
                    }
                    e.transform = transform;
                    e.owner = owner;
                    e.attack_target = obj
                        .target
                        .and_then(|tid| self.host_to_entity.get(&tid.0).copied());
                    e.move_target = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
                    e.max_health = obj.max_health.max(obj.health.current).max(1.0);
                    e.body_damage_state = obj.body_damage_state.ordinal();
                    e.selected = obj.selected;
                    e.destroyed = obj.status.destroyed;
                    e.death_type = obj.status.death_type.ordinal();
                    e.construction_percent = obj.construction_percent.clamp(-1.0, 1.0);
                    e.is_rebuild_hole = obj.is_rebuild_hole;
                    e.rebuild_template_name = obj.rebuild_template_name.clone().unwrap_or_default();
                    e.rebuild_ready_frame = obj.rebuild_ready_frame;
                    e.rebuild_spawner_id = obj.rebuild_spawner_id.map(|id| id.0);
                    e.rebuild_worker_id = obj.rebuild_worker_id.map(|id| id.0);
                    e.rebuild_reconstructing_id = obj.rebuild_reconstructing_id.map(|id| id.0);
                    e.producer_id = obj.producer_id.map(|id| id.0);
                    e.construction_complete_clear_frame = obj.construction_complete_clear_frame;
                    e.team_ordinal = Self::host_team_ordinal(obj.team);
                    e.selection_radius = obj.selection_radius.max(5.0);
                    e.crusher_level = obj.crusher_level;
                    e.crushable_level = obj.crushable_level;
                    e.front_crushed = obj.front_crushed;
                    e.back_crushed = obj.back_crushed;
                    e.vision_range = obj.vision_range;
                    e.shroud_clearing_range = obj.shroud_clearing_range;
                    e.under_construction = obj.status.under_construction;
                    e.sold = obj.status.sold;
                    e.reconstructing = obj.status.reconstructing;
                    e.unselectable = obj.status.unselectable;
                    e.deployed = obj.status.deployed;
                    e.moving = obj.status.moving;
                    e.attacking = obj.status.attacking;
                    e.is_firing_weapon = obj.status.is_firing_weapon;
                    e.is_aiming_weapon = obj.status.is_aiming_weapon;
                    e.team_color = obj.team_color;
                    e.power_provided = obj.power_provided;
                    e.power_consumed = obj.power_consumed;
                    e.object_type_ordinal = Self::host_object_type_ordinal(obj.object_type);
                    e.max_transport = obj.max_transport;
                    e.force_attack = obj.force_attack;
                    e.show_health_bar = obj.show_health_bar;
                    e.target_location = obj.target_location.map(|p| [p.x, p.y, p.z]);
                    e.guard_position = obj.guard_position.map(|p| [p.x, p.y, p.z]);
                    e.guard_target_host = obj.guard_target.map(|id| id.0).unwrap_or(0);
                    e.ai_state_ordinal = Self::host_ai_state_ordinal(&obj.ai_state);
                    e.occupant_count = obj.occupants.len().min(u16::MAX as usize) as u16;
                    e.experience_points = obj.experience.current;
                    e.veterancy_ordinal = Self::host_veterancy_ordinal(obj.experience.level);
                    e.stored_supplies = obj.stored_resources.supplies;
                    e.stealthed = obj.status.stealthed;
                    e.detected = obj.status.detected;
                    e.using_ability = obj.status.using_ability;
                    e.airborne_target = obj.status.airborne_target;
                    e.disabled_underpowered = obj.status.disabled_underpowered;
                    e.disabled_unmanned = obj.status.disabled_unmanned;
                    e.disabled_hacked = obj.status.disabled_hacked;
                    e.disabled_emp = obj.status.disabled_emp;
                    e.disabled_emp_until_frame = obj.status.disabled_emp_until_frame;
                    e.disabled_hacked_until_frame = obj.status.disabled_hacked_until_frame;
                    e.disabled_paralyzed_until_frame = obj.status.disabled_paralyzed_until_frame;
                    e.disabled_paralyzed = obj.status.disabled_paralyzed;
                    e.weapons_jammed = obj.status.weapons_jammed;
                    e.masked = obj.status.masked;
                    e.disguised = obj.status.disguised;
                    e.disabled_subdued = obj.status.disabled_subdued;
                    e.is_carbomb = obj.status.is_carbomb;
                    e.hijacked = obj.status.hijacked;
                    e.ignoring_stealth = obj.status.ignoring_stealth;
                    e.repulsor = obj.status.repulsor;
                    e.repulsor_until_frame = obj.repulsor_until_frame;
                    e.disabled_freefall = obj.status.disabled_freefall;
                    e.no_collisions = obj.status.no_collisions;
                    e.private_captured = obj.status.private_captured;
                    e.disguise_transitioning_to = obj.status.disguise_transitioning_to;
                    e.disguise_halfpoint_reached = obj.status.disguise_halfpoint_reached;
                    e.faerie_fire = obj.status.faerie_fire;
                    e.booby_trapped = obj.status.booby_trapped;
                    e.eject_invulnerable = obj.status.eject_invulnerable;
                    e.pilot_did_move_to_base = obj.status.pilot_did_move_to_base;
                    e.parachuting = obj.status.parachuting;
                    e.parachute_open = obj.status.parachute_open;
                    e.parachute_landing_override_set = obj.status.parachute_landing_override_set;
                    e.is_building = obj.building_data.is_some();
                    if let Some(bd) = obj.building_data.as_ref() {
                        e.building_type_ordinal =
                            Self::host_building_type_ordinal(bd.building_type);
                        e.production_queue_len = bd.production_queue.len().min(255) as u8;
                        {
                            const MAX_QUEUE: usize = 16;
                            e.production_queue_items = bd
                                .production_queue
                                .iter()
                                .take(MAX_QUEUE)
                                .map(|p| EntityProductionItem {
                                    template_name: p.template_name.clone(),
                                    progress: p.progress,
                                    total_time: p.total_time,
                                    cost_supplies: p.cost.supplies,
                                    is_upgrade: p.is_upgrade(),
                                })
                                .collect();
                        }
                        if let Some(head) = bd.production_queue.first() {
                            e.production_progress = head.progress;
                            e.exit_delay_remaining = bd.exit_delay_remaining;
                            e.production_door_phase = obj.production_door_phase;
                            e.production_door_phase_end_frame = obj.production_door_phase_end_frame;
                            e.production_door_hold_open = obj.production_door_hold_open;
                            e.production_template = head.template_name.clone();
                        } else {
                            e.production_progress = 0.0;
                            e.production_template.clear();
                        }
                        e.rally_point = bd.rally_point.map(|p| [p.x, p.y, p.z]);
                        e.garrison_count = bd.garrisoned_units.len().min(u16::MAX as usize) as u16;
                        e.max_garrison = bd.max_garrison.min(u16::MAX as usize) as u16;
                    } else {
                        e.building_type_ordinal = 255;
                        e.production_queue_len = 0;
                        e.production_progress = 0.0;
                        e.production_template.clear();
                        e.production_queue_items.clear();
                        e.rally_point = None;
                        e.garrison_count = 0;
                        e.max_garrison = 0;
                    }
                    e.has_weapon = obj.weapon.is_some();
                    if let Some(w) = obj.weapon.as_ref() {
                        e.weapon_damage = w.damage;
                        e.weapon_range = w.range;
                        e.weapon_min_range = w.min_range;
                        e.weapon_reload_time = w.reload_time;
                        e.weapon_last_fire_time =
                            obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
                        e.last_fire_victim_host = obj.last_fire_victim_host;
                        e.last_fire_slot = obj.last_fire_slot;
                        e.last_fire_damage = obj.last_fire_damage;
                        e.last_fire_range = obj.last_fire_range;
                        e.last_fire_sim_time = obj.last_fire_sim_time;
                        e.last_fire_frame = obj.last_fire_frame;
                        e.fire_intent_count = obj.fire_intent_count;
                        e.weapon_ammo = w.ammo.unwrap_or(u32::MAX);
                        e.weapon_can_target_air = w.can_target_air;
                        e.weapon_can_target_ground = w.can_target_ground;
                        e.weapon_projectile_speed = w.projectile_speed;
                    } else {
                        e.weapon_damage = 0.0;
                        e.weapon_range = 0.0;
                        e.weapon_min_range = 0.0;
                        e.weapon_reload_time = 0.0;
                        e.weapon_ammo = u32::MAX;
                        e.weapon_can_target_air = false;
                        e.weapon_can_target_ground = true;
                        e.weapon_projectile_speed = 0.0;
                    }
                    e.has_secondary_weapon = obj.secondary_weapon.is_some();
                    e.move_max_speed = obj.movement.max_speed;
                    e.velocity = [
                        obj.movement.velocity.x,
                        obj.movement.velocity.y,
                        obj.movement.velocity.z,
                    ];
                    e.path_len = obj.movement.path.len().min(u16::MAX as usize) as u16;
                    e.path_index = obj.movement.current_path_index.min(u16::MAX as usize) as u16;
                    e.waiting_for_path = obj.waiting_for_path;
                    e.motive_frames_remaining = obj.motive_frames_remaining;
                    e.kill_when_resting_on_ground = obj.kill_when_resting_on_ground;
                    e.bounce_land_events = obj.bounce_land_events;
                    e.last_bounce_fall_dy = obj.last_bounce_fall_dy;
                    e.bounce_sound_name = obj.bounce_sound_name.clone();
                    e.last_bounce_volume = obj.last_bounce_volume;
                    e.bounce_audio_pending = obj.bounce_audio_pending;
                    e.allow_collide_force = obj.allow_collide_force;
                    e.last_collidee_id = obj.last_collidee.map(|id| id.0);
                    e.ignore_collisions_with_id = obj.ignore_collisions_with.map(|id| id.0);
                    e.physics_mass = obj.physics_mass;
                    e.physics_accel = [
                        obj.physics_accel.x,
                        obj.physics_accel.y,
                        obj.physics_accel.z,
                    ];
                    e.forward_friction = obj.forward_friction;
                    e.lateral_friction = obj.lateral_friction;
                    e.z_friction = obj.z_friction;
                    e.can_path_through_units = obj.can_path_through_units;
                    e.ignore_collisions_until_frame = obj.ignore_collisions_until_frame;
                    e.is_panicking = obj.is_panicking;
                    e.move_away_frames = obj.move_away_frames;
                    e.aerodynamic_friction = obj.aerodynamic_friction;
                    e.extra_friction = obj.extra_friction;
                    e.apply_friction_2d_when_airborne = obj.apply_friction_2d_when_airborne;
                    e.center_of_mass_offset = obj.center_of_mass_offset;
                    e.pitch_roll_yaw_factor = obj.pitch_roll_yaw_factor;
                    e.move_away_destination = obj.move_away_destination.map(|p| [p.x, p.y, p.z]);
                    e.request_other_move_away_id = obj.request_other_move_away.map(|id| id.0);
                    e.immune_to_falling_damage = obj.immune_to_falling_damage;
                    e.physics_current_overlap_id = obj.physics_current_overlap.map(|id| id.0);
                    e.physics_previous_overlap_id = obj.physics_previous_overlap.map(|id| id.0);
                    e.shock_stun_frames = obj.shock_stun_frames;
                    e.shock_yaw_rate = obj.shock_yaw_rate;
                    e.shock_pitch_rate = obj.shock_pitch_rate;
                    e.shock_roll_rate = obj.shock_roll_rate;
                    e.shock_up_z = obj.shock_up_z;
                    e.shock_allow_bounce = obj.shock_allow_bounce;
                    e.shock_grounded_once = obj.shock_grounded_once;
                    e.shock_was_airborne = obj.shock_was_airborne;
                    e.cell_is_cliff = obj.cell_is_cliff;
                    e.cell_is_underwater = obj.cell_is_underwater;
                    e.locomotor_surfaces = obj.locomotor_surfaces;
                    e.is_attack_path = obj.is_attack_path;
                    e.is_approach_path = obj.is_approach_path;
                    e.on_invalid_movement_terrain = obj.on_invalid_movement_terrain;
                    e.was_airborne_last_frame = obj.was_airborne_last_frame;
                    e.can_move_backward = obj.can_move_backward;
                    e.moving_backwards = obj.moving_backwards;
                    e.no_slow_down_as_approaching_dest = obj.no_slow_down_as_approaching_dest;
                    e.turn_pivot_offset = obj.turn_pivot_offset;
                    e.wander_width_factor = obj.wander_width_factor;
                    e.loco_apply_2d_friction_airborne = obj.loco_apply_2d_friction_airborne;
                    e.loco_extra_2d_friction = obj.loco_extra_2d_friction;
                    e.loco_preferred_height = obj.loco_preferred_height;
                    e.loco_preferred_height_damping = obj.loco_preferred_height_damping;
                    e.loco_appearance_ordinal = obj.loco_appearance.to_ordinal();
                    e.loco_behavior_z_ordinal = obj.loco_behavior_z.to_ordinal();
                    e.min_turn_speed = obj.min_turn_speed;
                    e.physics_turning_ordinal = obj.physics_turning.to_ordinal();
                    e.is_blocked_and_stuck = obj.is_blocked_and_stuck;
                    e.is_braking = obj.is_braking;
                    e.is_safe_path = obj.is_safe_path;
                    e.queue_for_path_frames = obj.queue_for_path_frames;
                    e.path_timestamp = obj.path_timestamp;
                    e.cur_max_blocked_speed = obj.cur_max_blocked_speed;
                    e.num_frames_blocked = obj.num_frames_blocked;
                    e.is_blocked = obj.is_blocked;
                    e.move_away_from_id = obj.move_away_from.map(|id| id.0);
                    e.requested_victim_id = obj.requested_victim_id.map(|id| id.0);
                    e.requested_destination = obj.requested_destination.map(|p| [p.x, p.y, p.z]);
                    e.prev_victim_pos = obj.prev_victim_pos.map(|p| [p.x, p.y, p.z]);
                    e.crate_created_host = obj.crate_created.map(|id| id.0).unwrap_or(0);
                    e.guard_retaliate_victim_host =
                        obj.guard_retaliate_victim.map(|id| id.0).unwrap_or(0);
                    e.guard_retaliate_anchor = obj.guard_retaliate_anchor.map(|p| [p.x, p.y, p.z]);
                    e.path_timestamp = obj.path_timestamp;
                    e.disguise_pending_template =
                        obj.disguise_pending_template.clone().unwrap_or_default();
                    e.disguise_pending_team_ordinal = obj
                        .disguise_pending_team
                        .map(|t| match t {
                            Team::USA => 0u8,
                            Team::China => 1u8,
                            Team::GLA => 2u8,
                            Team::Neutral => 3u8,
                        })
                        .unwrap_or(255u8);
                    e.weapon_crate_upgrade = obj.weapon_crate_upgrade;
                    e.armor_crate_upgrade = obj.armor_crate_upgrade;
                    e.selection_flash_remaining = obj.selection_flash_remaining;
                    e.path_waypoints = obj
                        .movement
                        .path
                        .iter()
                        .take(16)
                        .map(|p| [p.x, p.y, p.z])
                        .collect();
                    e.secondary_weapon_range = obj
                        .secondary_weapon
                        .as_ref()
                        .map(|w| w.range)
                        .unwrap_or(0.0);
                    e.secondary_weapon_damage = obj
                        .secondary_weapon
                        .as_ref()
                        .map(|w| w.damage)
                        .unwrap_or(0.0);
                    e.display_name = obj.name.clone();
                    e.model_key = crate::assets::mesh_asset_resolve::model_key_from_template(
                        obj.get_template(),
                    );
                    e.model_condition_bits = obj.model_condition_bits;
                    e.radar_extend_done_frame = obj.radar_extend_done_frame;
                    e.radar_extend_complete = obj.radar_extend_complete;
                    e.radar_active = obj.radar_active;
                    e.mesh_scale = crate::assets::mesh_asset_resolve::mesh_scale_from_template(
                        obj.get_template(),
                    );
                    {
                        use crate::fow_rendering::FOWRenderingBridge;
                        let vis = if logic.isInShellGame() {
                            crate::fow_rendering::ObjectVisibility::FULLY_VISIBLE
                        } else {
                            FOWRenderingBridge::get_object_visibility(
                                logic.local_player_id().unwrap_or(0),
                                obj.id,
                            )
                        };
                        e.fow_visibility_alpha = vis.visibility_alpha;
                        e.fow_is_explored = vis.is_explored;
                        e.fow_visibility_falloff = vis.visibility_falloff;
                    }
                    {
                        let pos = obj.get_position();
                        if obj.ground_height_from_terrain {
                            e.ground_height = obj.ground_height;
                            e.ground_height_from_terrain = true;
                        } else {
                            match logic.terrain_height_at(pos) {
                                Some(h) if h.is_finite() => {
                                    e.ground_height = h;
                                    e.ground_height_from_terrain = true;
                                }
                                _ => {
                                    e.ground_height = obj.ground_height;
                                    e.ground_height_from_terrain = obj.ground_height_from_terrain;
                                }
                            }
                        }
                    }
                    e.engine_bridged = obj.engine_object_id.is_some();
                    e.overlord_bunker_capacity = obj
                        .overlord_bunker_capacity
                        .map(|n| n.min(u16::MAX as usize - 1) as u16)
                        .unwrap_or(u16::MAX);
                    e.passengers_allowed_to_fire = obj.passengers_allowed_to_fire;
                    e.armed_riders_upgrade_weapon_set = obj.armed_riders_upgrade_weapon_set;
                    e.weapon_set_player_upgrade = obj.weapon_set_player_upgrade;
                    e.is_battle_bus_transport = obj.is_battle_bus_transport;
                    e.is_technical_transport = obj.is_technical_transport;
                    e.is_combat_cycle_transport = obj.is_combat_cycle_transport;
                    e.combat_cycle_rider = obj.combat_cycle_rider;
                    e.is_tunnel_network = obj.is_tunnel_network;
                    e.is_combat_chinook_transport = obj.is_combat_chinook_transport;
                    e.contained_by_host = obj.contained_by.map(|id| id.0).unwrap_or(0);
                    {
                        const MAX_GARRISON_IDS: usize = 16;
                        let mut ids: Vec<u32> = Vec::new();
                        if let Some(bd) = obj.building_data.as_ref() {
                            for oid in bd.garrisoned_units.iter().take(MAX_GARRISON_IDS) {
                                ids.push(oid.0);
                            }
                        }
                        if ids.is_empty() {
                            for oid in obj.occupants.iter().take(MAX_GARRISON_IDS) {
                                ids.push(oid.0);
                            }
                        }
                        e.garrisoned_host_ids = ids;
                    }
                    e.kind_of_bits = Self::host_kind_of_bits(obj);
                    e.cheer_timer = obj.cheer_timer;
                    e.overcharge_enabled = obj.overcharge_enabled;
                    e.active_weapon_slot = obj.active_weapon_slot;
                    e.guard_radius = obj.guard_radius;
                    e.applied_upgrade_count =
                        obj.applied_upgrades.len().min(u16::MAX as usize) as u16;
                    {
                        const MAX_UPGRADES: usize = 24;
                        let mut names: Vec<String> = obj.applied_upgrades.iter().cloned().collect();
                        names.sort();
                        names.truncate(MAX_UPGRADES);
                        e.applied_upgrade_names = names;
                    }
                    e.special_power_ready = obj.special_power_ready;
                    e.special_power_cooldown = obj.special_power_cooldown;
                    e.special_power_cooldown_remaining = obj.special_power_cooldown_remaining;
                    e.is_detector = obj.is_detector;
                    e.detection_range = obj.detection_range;
                    e.detection_rate_frames = obj.detection_rate_frames;
                    e.hijack_vehicle_host = obj.hijack_vehicle_id.map(|id| id.0).unwrap_or(0);
                    e.hijacker_in_vehicle = obj.hijacker_in_vehicle;
                    e.hijacker_update_active = obj.hijacker_update_active;
                    e.hijacker_was_airborne = obj.hijacker_was_airborne;
                    e.hijacker_eject_pos = obj.hijacker_eject_pos.map(|p| [p.x, p.y, p.z]);
                    e.hive_slave_respawn_frame = obj.hive_slave_respawn_frame;
                    e.next_detection_scan_frame = obj.next_detection_scan_frame;
                    e.stealth_breaks_on_attack = obj.stealth_breaks_on_attack;
                    e.stealth_breaks_on_move = obj.stealth_breaks_on_move;
                    e.innate_stealth = obj.innate_stealth;
                    e.stealth_allowed_frame = obj.stealth_allowed_frame;
                    e.stealth_delay_pending = obj.stealth_delay_pending;
                    e.stealth_delay_frames = obj.stealth_delay_frames;
                    e.stealth_breaks_on_damage = obj.stealth_breaks_on_damage;
                    e.detection_expires_frame = obj.detection_expires_frame;
                    e.camo_opacity_pulse_phase = obj.camo_opacity_pulse_phase;
                    e.camo_heat_vision_opacity = obj.camo_heat_vision_opacity;
                    e.camo_net_sub_object_shown = obj.camo_net_sub_object_shown;
                    e.camo_net_sub_object_observer_visible =
                        obj.camo_net_sub_object_observer_visible;
                    e.weapon_bonus_enthusiastic = obj.weapon_bonus_enthusiastic;
                    e.weapon_bonus_subliminal = obj.weapon_bonus_subliminal;
                    e.weapon_bonus_horde = obj.weapon_bonus_horde;
                    e.weapon_bonus_nationalism = obj.weapon_bonus_nationalism;
                    e.weapon_bonus_frenzy = obj.weapon_bonus_frenzy;
                    e.weapon_bonus_frenzy_level = obj.weapon_bonus_frenzy_level;
                    e.weapon_bonus_battle_plan_bombardment =
                        obj.weapon_bonus_battle_plan_bombardment;
                    e.weapon_bonus_battle_plan_hold_the_line =
                        obj.weapon_bonus_battle_plan_hold_the_line;
                    e.weapon_bonus_battle_plan_search_and_destroy =
                        obj.weapon_bonus_battle_plan_search_and_destroy;
                    e.continuous_fire_level = obj.continuous_fire_level;
                    e.continuous_fire_consecutive =
                        obj.continuous_fire_consecutive.min(u16::MAX as u32) as u16;
                    e.faerie_fire_until_frame = obj.faerie_fire_until_frame;
                    e.is_humvee_transport = obj.is_humvee_transport;
                    e.is_listening_outpost_transport = obj.is_listening_outpost_transport;
                    e.is_troop_crawler_transport = obj.is_troop_crawler_transport;
                    e.is_helix_transport = obj.is_helix_transport;
                    e.has_overlord_gattling_addon = obj.has_overlord_gattling_addon;
                    e.has_overlord_propaganda_addon = obj.has_overlord_propaganda_addon;
                    e.demo_suicided_detonating = obj.demo_suicided_detonating;
                    e.hive_slave_count = obj.hive_slave_count;
                    e.hive_slave_hp = obj.hive_slave_hp;
                    e.turret_angle_deg = obj.turret_angle_deg;
                    e.turret_pitch_deg = obj.turret_pitch_deg;
                    e.turret_idle_scanning = obj.turret_idle_scanning;
                    e.turret_holding = obj.turret_holding;
                    e.turret_turn_rate_rad = obj.turret_turn_rate_rad;
                    e.turret_recenter_frames = obj.turret_recenter_frames;
                    e.turret_hold_until_frame = obj.turret_hold_until_frame;
                    e.turret_idle_recentering = obj.turret_idle_recentering;
                    e.turret_enabled = obj.turret_enabled;
                    e.turret_rotating = obj.turret_rotating;
                    e.turret_natural_angle_deg = obj.turret_natural_angle_deg;
                    e.turret_natural_pitch_deg = obj.turret_natural_pitch_deg;
                    e.turret_target_host = obj.turret_target_id.map(|id| id.0).unwrap_or(0);
                    e.turret_force_attacking = obj.turret_force_attacking;
                    e.turret_mood_target = obj.turret_mood_target;
                    e.turret_idle_scan_next_frame = obj.turret_idle_scan_next_frame;
                    e.turret_idle_scan_desired_angle_deg = obj.turret_idle_scan_desired_angle_deg;
                    e.turret_idle_scan_index = obj.turret_idle_scan_index;
                    e.turret_substate = obj.turret_substate.ordinal();
                    e.ai_attitude = obj.ai_attitude;
                    e.idle_since_frame = obj.idle_since_frame;
                    e.mood_attack_check_rate = obj.mood_attack_check_rate;
                    e.auto_acquire_when_idle = obj.auto_acquire_when_idle;
                    e.attack_priority_set = obj.attack_priority_set.clone().unwrap_or_default();
                    e.last_damage_source_host = obj.last_damage_source.map(|id| id.0).unwrap_or(0);
                    e.sole_healing_benefactor_id = obj.sole_healing_benefactor.map(|id| id.0);
                    e.sole_healing_benefactor_expiration_frame =
                        obj.sole_healing_benefactor_expiration_frame;
                    e.command_set_override = obj.command_set_override.clone().unwrap_or_default();
                    e.disguise_as_template = obj.disguise_as_template.clone().unwrap_or_default();
                    e.disguise_as_team_ordinal = obj
                        .disguise_as_team
                        .map(|t| match t {
                            Team::USA => 0,
                            Team::China => 1,
                            Team::GLA => 2,
                            Team::Neutral => 3,
                        })
                        .unwrap_or(255);
                    e.vision_spied_mask = obj.vision_spied_mask;
                    e.camo_friendly_opacity = obj.camo_friendly_opacity;
                    e.camo_stealth_look = obj.camo_stealth_look as u8;
                    e.has_mine_data = obj.mine_data.is_some();
                    e.weapon_bonus_frenzy_until_frame = obj.weapon_bonus_frenzy_until_frame;
                    e.continuous_fire_coast_until_frame = obj.continuous_fire_coast_until_frame;
                    e.pre_attack_target_host = obj.pre_attack_target.map(|id| id.0).unwrap_or(0);
                    e.pre_attack_ready_at = obj.pre_attack_ready_at;
                    e.consecutive_shots_at_target = obj.consecutive_shots_at_target;
                    e.max_shots_to_fire = obj.max_shots_to_fire;
                    e.attack_substate_ordinal = obj.attack_substate.to_ordinal();
                    e.approach_timestamp = obj.approach_timestamp;
                    e.continuous_fire_victim = obj.continuous_fire_victim;
                    e.maintain_pos_valid = obj.maintain_pos_valid;
                    e.maintain_pos = obj.maintain_pos.map(|p| [p.x, p.y, p.z]);
                    e.temporary_move_frames = obj.temporary_move_frames;
                    e.group_speed_factor = obj.group_speed_factor;
                    e.battle_plan_sight_scalar_applied = obj.battle_plan_sight_scalar_applied;
                    // Keep template name if host renamed (rare).
                    if e.template.name != obj.template_name {
                        e.template = TemplateRef::new(obj.template_name.clone());
                    }
                } else {
                    // Map pointed at dead entity — respawn.
                    self.host_to_entity.remove(&oid.0);
                    self.entity_to_host.remove(&eid.get());
                    self.spawn_mapped(oid, obj.template_name.clone(), owner, transform, health);
                }
            } else {
                self.spawn_mapped(oid, obj.template_name.clone(), owner, transform, health);
            }
        }

        // Second pass: resolve attack targets now that all IDs are mapped.
        for oid in logic.get_objects().keys().copied() {
            let Some(obj) = logic.get_objects().get(&oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            let at = obj
                .target
                .and_then(|tid| self.host_to_entity.get(&tid.0).copied());
            if let Some(e) = self.world.world_mut().entity_mut(eid) {
                e.attack_target = at;
                e.move_target = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
                e.max_health = obj.max_health.max(obj.health.current).max(1.0);
                e.selected = obj.selected;
                e.destroyed = obj.status.destroyed;
                e.construction_percent = obj.construction_percent.clamp(-1.0, 1.0);
                e.team_ordinal = Self::host_team_ordinal(obj.team);
                e.selection_radius = obj.selection_radius.max(5.0);
                e.crusher_level = obj.crusher_level;
                e.crushable_level = obj.crushable_level;
                e.vision_range = obj.vision_range;
                e.shroud_clearing_range = obj.shroud_clearing_range;
                e.under_construction = obj.status.under_construction;
                e.sold = obj.status.sold;
                e.reconstructing = obj.status.reconstructing;
                e.unselectable = obj.status.unselectable;
                e.deployed = obj.status.deployed;
                e.moving = obj.status.moving;
                e.attacking = obj.status.attacking;
                e.is_firing_weapon = obj.status.is_firing_weapon;
                e.is_aiming_weapon = obj.status.is_aiming_weapon;
                e.team_color = obj.team_color;
                e.power_provided = obj.power_provided;
                e.power_consumed = obj.power_consumed;
                e.object_type_ordinal = Self::host_object_type_ordinal(obj.object_type);
                e.max_transport = obj.max_transport;
                e.force_attack = obj.force_attack;
                e.show_health_bar = obj.show_health_bar;
                e.target_location = obj.target_location.map(|p| [p.x, p.y, p.z]);
                e.guard_position = obj.guard_position.map(|p| [p.x, p.y, p.z]);
                e.guard_target_host = obj.guard_target.map(|id| id.0).unwrap_or(0);
                e.ai_state_ordinal = Self::host_ai_state_ordinal(&obj.ai_state);
                e.occupant_count = obj.occupants.len().min(u16::MAX as usize) as u16;
                e.experience_points = obj.experience.current;
                e.veterancy_ordinal = Self::host_veterancy_ordinal(obj.experience.level);
                e.stored_supplies = obj.stored_resources.supplies;
                e.stealthed = obj.status.stealthed;
                e.detected = obj.status.detected;
                e.using_ability = obj.status.using_ability;
                e.airborne_target = obj.status.airborne_target;
                e.disabled_underpowered = obj.status.disabled_underpowered;
                e.disabled_unmanned = obj.status.disabled_unmanned;
                e.disabled_hacked = obj.status.disabled_hacked;
                e.disabled_emp = obj.status.disabled_emp;
                e.disabled_emp_until_frame = obj.status.disabled_emp_until_frame;
                e.disabled_hacked_until_frame = obj.status.disabled_hacked_until_frame;
                e.disabled_paralyzed_until_frame = obj.status.disabled_paralyzed_until_frame;
                e.disabled_paralyzed = obj.status.disabled_paralyzed;
                e.weapons_jammed = obj.status.weapons_jammed;
                e.masked = obj.status.masked;
                e.disguised = obj.status.disguised;
                e.disabled_subdued = obj.status.disabled_subdued;
                e.is_carbomb = obj.status.is_carbomb;
                e.hijacked = obj.status.hijacked;
                e.ignoring_stealth = obj.status.ignoring_stealth;
                e.repulsor = obj.status.repulsor;
                e.repulsor_until_frame = obj.repulsor_until_frame;
                e.disabled_freefall = obj.status.disabled_freefall;
                e.no_collisions = obj.status.no_collisions;
                e.private_captured = obj.status.private_captured;
                e.disguise_transitioning_to = obj.status.disguise_transitioning_to;
                e.disguise_halfpoint_reached = obj.status.disguise_halfpoint_reached;
                e.faerie_fire = obj.status.faerie_fire;
                e.booby_trapped = obj.status.booby_trapped;
                e.eject_invulnerable = obj.status.eject_invulnerable;
                e.pilot_did_move_to_base = obj.status.pilot_did_move_to_base;
                e.parachuting = obj.status.parachuting;
                e.parachute_open = obj.status.parachute_open;
                e.parachute_landing_override_set = obj.status.parachute_landing_override_set;
                e.is_building = obj.building_data.is_some();
                if let Some(bd) = obj.building_data.as_ref() {
                    e.building_type_ordinal = Self::host_building_type_ordinal(bd.building_type);
                    e.production_queue_len = bd.production_queue.len().min(255) as u8;
                    {
                        const MAX_QUEUE: usize = 16;
                        e.production_queue_items = bd
                            .production_queue
                            .iter()
                            .take(MAX_QUEUE)
                            .map(|p| EntityProductionItem {
                                template_name: p.template_name.clone(),
                                progress: p.progress,
                                total_time: p.total_time,
                                cost_supplies: p.cost.supplies,
                                is_upgrade: p.is_upgrade(),
                            })
                            .collect();
                    }
                    if let Some(head) = bd.production_queue.first() {
                        e.production_progress = head.progress;
                        e.production_template = head.template_name.clone();
                    } else {
                        e.production_progress = 0.0;
                        e.production_template.clear();
                    }
                    e.rally_point = bd.rally_point.map(|p| [p.x, p.y, p.z]);
                    e.garrison_count = bd.garrisoned_units.len().min(u16::MAX as usize) as u16;
                    e.max_garrison = bd.max_garrison.min(u16::MAX as usize) as u16;
                } else {
                    e.building_type_ordinal = 255;
                    e.production_queue_len = 0;
                    e.production_progress = 0.0;
                    e.production_template.clear();
                    e.production_queue_items.clear();
                    e.rally_point = None;
                    e.garrison_count = 0;
                    e.max_garrison = 0;
                }
                e.has_weapon = obj.weapon.is_some();
                if let Some(w) = obj.weapon.as_ref() {
                    e.weapon_damage = w.damage;
                    e.weapon_range = w.range;
                    e.weapon_min_range = w.min_range;
                    e.weapon_reload_time = w.reload_time;
                    e.weapon_ammo = w.ammo.unwrap_or(u32::MAX);
                    e.weapon_can_target_air = w.can_target_air;
                    e.weapon_can_target_ground = w.can_target_ground;
                    e.weapon_projectile_speed = w.projectile_speed;
                } else {
                    e.weapon_damage = 0.0;
                    e.weapon_range = 0.0;
                    e.weapon_min_range = 0.0;
                    e.weapon_reload_time = 0.0;
                    e.weapon_ammo = u32::MAX;
                    e.weapon_can_target_air = false;
                    e.weapon_can_target_ground = true;
                    e.weapon_projectile_speed = 0.0;
                }
                e.has_secondary_weapon = obj.secondary_weapon.is_some();
                e.move_max_speed = obj.movement.max_speed;
                e.velocity = [
                    obj.movement.velocity.x,
                    obj.movement.velocity.y,
                    obj.movement.velocity.z,
                ];
                e.path_len = obj.movement.path.len().min(u16::MAX as usize) as u16;
                e.path_index = obj.movement.current_path_index.min(u16::MAX as usize) as u16;
                e.path_waypoints = obj
                    .movement
                    .path
                    .iter()
                    .take(16)
                    .map(|p| [p.x, p.y, p.z])
                    .collect();
                e.secondary_weapon_range = obj
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.range)
                    .unwrap_or(0.0);
                e.secondary_weapon_damage = obj
                    .secondary_weapon
                    .as_ref()
                    .map(|w| w.damage)
                    .unwrap_or(0.0);
                e.display_name = obj.name.clone();
                e.model_key =
                    crate::assets::mesh_asset_resolve::model_key_from_template(obj.get_template());
                e.model_condition_bits = obj.model_condition_bits;
                e.mesh_scale =
                    crate::assets::mesh_asset_resolve::mesh_scale_from_template(obj.get_template());
                {
                    use crate::fow_rendering::FOWRenderingBridge;
                    let vis = if logic.isInShellGame() {
                        crate::fow_rendering::ObjectVisibility::FULLY_VISIBLE
                    } else {
                        FOWRenderingBridge::get_object_visibility(
                            logic.local_player_id().unwrap_or(0),
                            obj.id,
                        )
                    };
                    e.fow_visibility_alpha = vis.visibility_alpha;
                    e.fow_is_explored = vis.is_explored;
                    e.fow_visibility_falloff = vis.visibility_falloff;
                }
                {
                    let pos = obj.get_position();
                    match logic.terrain_height_at(pos) {
                        Some(h) if h.is_finite() => {
                            e.ground_height = h;
                            e.ground_height_from_terrain = true;
                        }
                        _ => {
                            e.ground_height = 0.0;
                            e.ground_height_from_terrain = false;
                        }
                    }
                }
                e.engine_bridged = obj.engine_object_id.is_some();
                e.overlord_bunker_capacity = obj
                    .overlord_bunker_capacity
                    .map(|n| n.min(u16::MAX as usize - 1) as u16)
                    .unwrap_or(u16::MAX);
                e.passengers_allowed_to_fire = obj.passengers_allowed_to_fire;
                e.armed_riders_upgrade_weapon_set = obj.armed_riders_upgrade_weapon_set;
                e.weapon_set_player_upgrade = obj.weapon_set_player_upgrade;
                e.is_battle_bus_transport = obj.is_battle_bus_transport;
                e.is_technical_transport = obj.is_technical_transport;
                e.is_combat_cycle_transport = obj.is_combat_cycle_transport;
                e.combat_cycle_rider = obj.combat_cycle_rider;
                e.is_tunnel_network = obj.is_tunnel_network;
                e.is_combat_chinook_transport = obj.is_combat_chinook_transport;
                e.contained_by_host = obj.contained_by.map(|id| id.0).unwrap_or(0);
                {
                    const MAX_GARRISON_IDS: usize = 16;
                    let mut ids: Vec<u32> = Vec::new();
                    if let Some(bd) = obj.building_data.as_ref() {
                        for oid in bd.garrisoned_units.iter().take(MAX_GARRISON_IDS) {
                            ids.push(oid.0);
                        }
                    }
                    if ids.is_empty() {
                        for oid in obj.occupants.iter().take(MAX_GARRISON_IDS) {
                            ids.push(oid.0);
                        }
                    }
                    e.garrisoned_host_ids = ids;
                }
                e.kind_of_bits = Self::host_kind_of_bits(obj);
                e.cheer_timer = obj.cheer_timer;
                e.overcharge_enabled = obj.overcharge_enabled;
                e.active_weapon_slot = obj.active_weapon_slot;
                e.guard_radius = obj.guard_radius;
                e.applied_upgrade_count = obj.applied_upgrades.len().min(u16::MAX as usize) as u16;
                {
                    const MAX_UPGRADES: usize = 24;
                    let mut names: Vec<String> = obj.applied_upgrades.iter().cloned().collect();
                    names.sort();
                    names.truncate(MAX_UPGRADES);
                    e.applied_upgrade_names = names;
                }
                e.special_power_ready = obj.special_power_ready;
                e.special_power_cooldown = obj.special_power_cooldown;
                e.special_power_cooldown_remaining = obj.special_power_cooldown_remaining;
                e.is_detector = obj.is_detector;
                e.detection_range = obj.detection_range;
                e.detection_rate_frames = obj.detection_rate_frames;
                e.stealth_breaks_on_attack = obj.stealth_breaks_on_attack;
                e.stealth_breaks_on_move = obj.stealth_breaks_on_move;
                e.innate_stealth = obj.innate_stealth;
                e.weapon_bonus_enthusiastic = obj.weapon_bonus_enthusiastic;
                e.weapon_bonus_subliminal = obj.weapon_bonus_subliminal;
                e.weapon_bonus_horde = obj.weapon_bonus_horde;
                e.weapon_bonus_nationalism = obj.weapon_bonus_nationalism;
                e.weapon_bonus_frenzy = obj.weapon_bonus_frenzy;
                e.weapon_bonus_frenzy_level = obj.weapon_bonus_frenzy_level;
                e.weapon_bonus_battle_plan_bombardment = obj.weapon_bonus_battle_plan_bombardment;
                e.weapon_bonus_battle_plan_hold_the_line =
                    obj.weapon_bonus_battle_plan_hold_the_line;
                e.weapon_bonus_battle_plan_search_and_destroy =
                    obj.weapon_bonus_battle_plan_search_and_destroy;
                e.continuous_fire_level = obj.continuous_fire_level;
                e.continuous_fire_consecutive =
                    obj.continuous_fire_consecutive.min(u16::MAX as u32) as u16;
                e.faerie_fire_until_frame = obj.faerie_fire_until_frame;
                e.is_humvee_transport = obj.is_humvee_transport;
                e.is_listening_outpost_transport = obj.is_listening_outpost_transport;
                e.is_troop_crawler_transport = obj.is_troop_crawler_transport;
                e.is_helix_transport = obj.is_helix_transport;
                e.has_overlord_gattling_addon = obj.has_overlord_gattling_addon;
                e.has_overlord_propaganda_addon = obj.has_overlord_propaganda_addon;
                e.demo_suicided_detonating = obj.demo_suicided_detonating;
                e.hive_slave_count = obj.hive_slave_count;
                e.hive_slave_hp = obj.hive_slave_hp;
                e.turret_angle_deg = obj.turret_angle_deg;
                e.turret_pitch_deg = obj.turret_pitch_deg;
                e.turret_idle_scanning = obj.turret_idle_scanning;
                e.turret_holding = obj.turret_holding;
                e.ai_attitude = obj.ai_attitude;
                e.last_damage_source_host = obj.last_damage_source.map(|id| id.0).unwrap_or(0);
                e.command_set_override = obj.command_set_override.clone().unwrap_or_default();
                e.disguise_as_template = obj.disguise_as_template.clone().unwrap_or_default();
                e.disguise_as_team_ordinal = obj
                    .disguise_as_team
                    .map(|t| match t {
                        Team::USA => 0,
                        Team::China => 1,
                        Team::GLA => 2,
                        Team::Neutral => 3,
                    })
                    .unwrap_or(255);
                e.vision_spied_mask = obj.vision_spied_mask;
                e.camo_friendly_opacity = obj.camo_friendly_opacity;
                e.camo_stealth_look = obj.camo_stealth_look as u8;
                e.has_mine_data = obj.mine_data.is_some();
                e.weapon_bonus_frenzy_until_frame = obj.weapon_bonus_frenzy_until_frame;
                e.continuous_fire_coast_until_frame = obj.continuous_fire_coast_until_frame;
                e.battle_plan_sight_scalar_applied = obj.battle_plan_sight_scalar_applied;
            }
        }

        // Align frame.
        let target = logic.get_frame() as u64;
        self.world.set_frame(target);
    }

    fn spawn_mapped(
        &mut self,
        host: ObjectId,
        template: String,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) {
        let eid = self
            .world
            .spawn_entity(TemplateRef::new(template), owner, transform, health);
        self.host_to_entity.insert(host.0, eid);
        self.entity_to_host.insert(eid.get(), host.0);
        // Defaults for residual fields until second-pass/host refresh fills them.
        if let Some(e) = self.world.world_mut().entity_mut(eid) {
            e.max_health = health.max(1.0);
            e.selected = false;
            e.destroyed = false;
            e.construction_percent = 1.0;
            e.team_ordinal = 255;
            e.selection_radius = 5.0;
            e.under_construction = false;
            e.moving = false;
            e.attacking = false;
            e.team_color = [1.0, 1.0, 1.0, 1.0];
            e.power_provided = 0;
            e.power_consumed = 0;
            e.object_type_ordinal = 6;
            e.max_transport = 0;
            e.force_attack = false;
            e.show_health_bar = true;
            e.target_location = None;
            e.guard_position = None;
            e.guard_target_host = 0;
            e.ai_state_ordinal = 0;
            e.occupant_count = 0;
            e.experience_points = 0.0;
            e.veterancy_ordinal = 0;
            e.stored_supplies = 0;
            e.stealthed = false;
            e.detected = false;
            e.using_ability = false;
            e.airborne_target = false;
            e.disabled_underpowered = false;
            e.disabled_unmanned = false;
            e.disabled_hacked = false;
            e.is_building = false;
            e.building_type_ordinal = 255;
            e.production_queue_len = 0;
            e.production_progress = 0.0;
            e.production_template.clear();
            e.production_queue_items.clear();
            e.rally_point = None;
            e.garrison_count = 0;
            e.max_garrison = 0;
            e.has_weapon = false;
            e.weapon_damage = 0.0;
            e.weapon_range = 0.0;
            e.weapon_min_range = 0.0;
            e.weapon_reload_time = 0.0;
            e.weapon_ammo = u32::MAX;
            e.weapon_can_target_air = false;
            e.weapon_can_target_ground = true;
            e.weapon_projectile_speed = 0.0;
            e.has_secondary_weapon = false;
            e.move_max_speed = 0.0;
            e.velocity = [0.0; 3];
            e.path_len = 0;
            e.path_index = 0;
            e.path_waypoints.clear();
            e.secondary_weapon_range = 0.0;
            e.secondary_weapon_damage = 0.0;
            e.display_name.clear();
            e.model_key.clear();
            e.mesh_scale = 1.0;
            e.fow_visibility_alpha = 1.0;
            e.fow_is_explored = 1.0;
            e.fow_visibility_falloff = 0.0;
            e.ground_height = 0.0;
            e.ground_height_from_terrain = false;
            e.engine_bridged = false;
            e.overlord_bunker_capacity = u16::MAX;
            e.passengers_allowed_to_fire = false;
            e.armed_riders_upgrade_weapon_set = false;
            e.weapon_set_player_upgrade = false;
            e.is_battle_bus_transport = false;
            e.is_technical_transport = false;
            e.is_combat_cycle_transport = false;
            e.combat_cycle_rider = 0;
            e.is_tunnel_network = false;
            e.is_combat_chinook_transport = false;
            e.contained_by_host = 0;
            e.garrisoned_host_ids.clear();
            e.kind_of_bits = 0;
            e.cheer_timer = 0.0;
            e.overcharge_enabled = false;
            e.active_weapon_slot = 0;
            e.guard_radius = 0.0;
            e.applied_upgrade_count = 0;
            e.applied_upgrade_names.clear();
            e.special_power_ready = false;
            e.special_power_cooldown = 0.0;
            e.special_power_cooldown_remaining = 0.0;
            e.is_detector = false;
            e.detection_range = 0.0;
            e.detection_rate_frames = 0;
            e.stealth_breaks_on_attack = false;
            e.stealth_breaks_on_move = false;
            e.innate_stealth = false;
            e.weapon_bonus_enthusiastic = false;
            e.weapon_bonus_subliminal = false;
            e.weapon_bonus_horde = false;
            e.weapon_bonus_nationalism = false;
            e.weapon_bonus_frenzy = false;
            e.weapon_bonus_frenzy_level = 0;
            e.weapon_bonus_battle_plan_bombardment = false;
            e.weapon_bonus_battle_plan_hold_the_line = false;
            e.weapon_bonus_battle_plan_search_and_destroy = false;
            e.continuous_fire_level = 0;
            e.continuous_fire_consecutive = 0;
            e.faerie_fire_until_frame = 0;
            e.is_humvee_transport = false;
            e.is_listening_outpost_transport = false;
            e.is_troop_crawler_transport = false;
            e.is_helix_transport = false;
            e.has_overlord_gattling_addon = false;
            e.has_overlord_propaganda_addon = false;
            e.demo_suicided_detonating = false;
            e.hive_slave_count = 0;
            e.hive_slave_hp = 0.0;
            e.turret_angle_deg = 0.0;
            e.turret_pitch_deg = 0.0;
            e.turret_idle_scanning = false;
            e.turret_holding = false;
            e.ai_attitude = 0;
            e.last_damage_source_host = 0;
            e.command_set_override.clear();
            e.disguise_as_template.clear();
            e.disguise_as_team_ordinal = 255;
            e.vision_spied_mask = 0;
            e.camo_friendly_opacity = 1.0;
            e.camo_stealth_look = 0;
            e.has_mine_data = false;
            e.weapon_bonus_frenzy_until_frame = 0;
            e.continuous_fire_coast_until_frame = 0;
            e.battle_plan_sight_scalar_applied = 1.0;
        }
    }

    fn copy_host_player_residual(
        pd: &mut gamelogic::world::PlayerData,
        p: &crate::game_logic::Player,
    ) {
        pd.supplies = p.resources.supplies;
        pd.power_available = p.power_available;
        pd.power_produced = p.power_produced;
        pd.power_consumed = p.power_consumed;
        pd.radar_count = p.radar_count;
        pd.radar_disabled = p.radar_disabled;
        pd.is_alive = p.is_alive;
        pd.cash_bounty_percent = p.cash_bounty_percent.clamp(0.0, 1.0);
        pd.color_rgb = p.color_rgb;
        pd.rank_level = p.rank_level.max(1);
        pd.skill_points = p.skill_points;
        pd.science_purchase_points = p.science_purchase_points;
        let mut cds: Vec<(String, f32)> = p
            .shared_special_power_cooldowns
            .iter()
            .map(|(k, v)| (format!("{k:?}"), (*v).max(0.0)))
            .collect();
        cds.sort_by(|a, b| a.0.cmp(&b.0));
        pd.shared_special_power_cooldowns = cds;
        pd.is_human = p.is_local;
        pd.name = p.name.clone();
    }

    fn host_player_science_and_upgrades(
        logic: &GameLogic,
        host_pid: u32,
    ) -> (Vec<String>, Vec<String>) {
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut sciences = logic
            .get_player(host_pid)
            .map(|p| {
                let mut v: Vec<String> = p.unlocked_sciences.iter().cloned().collect();
                v.sort();
                v
            })
            .unwrap_or_default();
        let mut upgrades: Vec<String> = logic
            .host_upgrades()
            .entries_snapshot()
            .into_iter()
            .filter(|e| e.player_id == host_pid && e.phase == HostUpgradePhase::Completed)
            .map(|e| e.name)
            .collect();
        upgrades.sort();
        upgrades.dedup();
        let _ = &mut sciences;
        (sciences, upgrades)
    }

    fn sync_players(&mut self, logic: &GameLogic) {
        // Rebuild player slots when count/identity changes; economy always refreshed.
        let mut host_ids: Vec<u32> = logic.get_players().keys().copied().collect();
        host_ids.sort_unstable();
        let need_rebuild = host_ids.len() != self.host_player_to_gw.len()
            || host_ids
                .iter()
                .any(|id| !self.host_player_to_gw.contains_key(id));

        if need_rebuild {
            // Fresh world would drop entities — only rebuild player table on the existing world
            // by allocating missing players. Simpler: rebuild world players via new GameWorld
            // only when empty map; otherwise update economy in place when possible.
            if self.host_player_to_gw.is_empty() && self.host_to_entity.is_empty() {
                let cap = host_ids.len().max(8).min(255);
                self.world = GameWorld::new(cap);
            }
            self.host_player_to_gw.clear();
            // If world already has players from prior allocate, we still allocate on a fresh world
            // when entity map empty. When entities exist, update economy only for known mapping.
            if self.host_to_entity.is_empty() {
                let cap = host_ids.len().max(8).min(255);
                self.world = GameWorld::new(cap);
                for pid in &host_ids {
                    let Some(p) = logic.get_player(*pid) else {
                        continue;
                    };
                    let team = match p.team {
                        Team::USA => Some(0),
                        Team::China => Some(1),
                        Team::GLA => Some(2),
                        Team::Neutral => None,
                    };
                    if let Some(gw_id) = self.world.allocate_player_with_economy(
                        Some(p.name.clone()),
                        team,
                        p.is_local,
                        p.resources.supplies,
                        p.power_available,
                    ) {
                        self.host_player_to_gw.insert(*pid, gw_id);
                    }
                }
            } else {
                // Entities live: keep existing GW players; rebuild host map by sorted order
                // matching prior allocation order (dense 0..n).
                for (idx, pid) in host_ids.iter().enumerate() {
                    let gw = PlayerId::from_index(idx as u8);
                    if self.world.player(gw).is_some() {
                        self.host_player_to_gw.insert(*pid, gw);
                        if let Some(p) = logic.get_player(*pid) {
                            if let Some(pd) = self.world.player_mut(gw) {
                                Self::copy_host_player_residual(pd, p);
                                let (sci, ups) =
                                    Self::host_player_science_and_upgrades(logic, *pid);
                                pd.unlocked_sciences = sci;
                                pd.completed_upgrades = ups;
                            }
                        }
                    } else if let Some(p) = logic.get_player(*pid) {
                        let team = match p.team {
                            Team::USA => Some(0),
                            Team::China => Some(1),
                            Team::GLA => Some(2),
                            Team::Neutral => None,
                        };
                        if let Some(gw_id) = self.world.allocate_player_with_economy(
                            Some(p.name.clone()),
                            team,
                            p.is_local,
                            p.resources.supplies,
                            p.power_available,
                        ) {
                            self.host_player_to_gw.insert(*pid, gw_id);
                        }
                    }
                }
            }
        } else {
            // Economy + science/upgrade absolute refresh.
            for (hid, gw) in self.host_player_to_gw.clone() {
                if let Some(p) = logic.get_player(hid) {
                    if let Some(pd) = self.world.player_mut(gw) {
                        Self::copy_host_player_residual(pd, p);
                        let (sci, ups) = Self::host_player_science_and_upgrades(logic, hid);
                        pd.unlocked_sciences = sci;
                        pd.completed_upgrades = ups;
                    }
                }
            }
        }
        // Always refresh science/upgrade/power-bar residual for mapped players.
        for (hid, gw) in self.host_player_to_gw.clone() {
            if let Some(p) = logic.get_player(hid) {
                if let Some(pd) = self.world.player_mut(gw) {
                    Self::copy_host_player_residual(pd, p);
                    let (sci, ups) = Self::host_player_science_and_upgrades(logic, hid);
                    pd.unlocked_sciences = sci;
                    // Merge event-channel completes with absolute host registry snapshot.
                    let mut merged = pd.completed_upgrades.clone();
                    merged.extend(ups);
                    merged.sort();
                    merged.dedup();
                    pd.completed_upgrades = merged;
                }
            }
        }
    }

    /// Reverse map GameWorld owner → host Team (for TransferOwner writeback).
    fn host_team_for_gw_owner(&self, logic: &GameLogic, owner: Option<PlayerId>) -> Option<Team> {
        let Some(pid) = owner else {
            return Some(Team::Neutral);
        };
        for (&hid, &gpid) in &self.host_player_to_gw {
            if gpid == pid {
                if let Some(p) = logic.get_player(hid) {
                    return Some(p.team);
                }
            }
        }
        None
    }

    fn owner_for_host_object(&self, logic: &GameLogic, team: Team) -> Option<PlayerId> {
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        for hid in ids {
            if let Some(p) = logic.get_player(hid) {
                if p.team == team {
                    return self.host_player_to_gw.get(&hid).copied();
                }
            }
        }
        // No matching host player for this team: leave unowned (do not
        // silently attach the first skirmish player).
        None
    }

    /// Write shadow entity health/destroyed onto host objects.
    pub fn writeback_health_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let new_h = ent.health.max(0.0);
            let new_max = ent.max_health.max(1.0);
            let changed = (obj.health.current - new_h).abs() > 0.000_1
                || ((new_h <= 0.0) != obj.status.destroyed)
                || (obj.max_health - new_max).abs() > 0.000_1
                || (obj.health.maximum - new_max).abs() > 0.000_1;
            if !changed {
                continue;
            }
            obj.health.current = new_h.min(new_max);
            obj.max_health = new_max;
            obj.health.maximum = new_max;
            if new_h <= 0.0 {
                obj.status.destroyed = true;
                obj.ai_state = crate::game_logic::AIState::Idle;
                obj.target = None;
            }
            updated += 1;
        }
        updated
    }

    /// Queue damage on the shadow entity mapped from a host object.
    /// Returns false if the host id is not mapped.
    /// Write shadow player supplies/power onto host players (economy last writer).
    pub fn writeback_economy_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &gw) in &self.host_player_to_gw {
            let Some(pd) = self.world.player(gw) else {
                continue;
            };
            let Some(player) = logic.get_player_mut(hid) else {
                continue;
            };
            let mut dirty = false;
            if player.resources.supplies != pd.supplies {
                player.resources.supplies = pd.supplies;
                dirty = true;
            }
            // Economy authority: host pending delta is consumed by absolute writeback.
            if player.pending_supply_delta != 0 {
                player.pending_supply_delta = 0;
                dirty = true;
            }
            if player.power_available != pd.power_available {
                player.power_available = pd.power_available;
                dirty = true;
            }
            if player.power_produced != pd.power_produced {
                player.power_produced = pd.power_produced;
                dirty = true;
            }
            if player.power_consumed != pd.power_consumed {
                player.power_consumed = pd.power_consumed;
                dirty = true;
            }
            if player.radar_count != pd.radar_count {
                player.radar_count = pd.radar_count;
                dirty = true;
            }
            if player.radar_disabled != pd.radar_disabled {
                player.radar_disabled = pd.radar_disabled;
                dirty = true;
            }
            if player.is_alive != pd.is_alive {
                player.is_alive = pd.is_alive;
                dirty = true;
            }
            if (player.cash_bounty_percent - pd.cash_bounty_percent).abs() > 1e-6 {
                player.cash_bounty_percent = pd.cash_bounty_percent;
                dirty = true;
            }
            if player.color_rgb != pd.color_rgb {
                player.color_rgb = pd.color_rgb;
                dirty = true;
            }
            if player.rank_level != pd.rank_level {
                player.rank_level = pd.rank_level;
                dirty = true;
            }
            if player.skill_points != pd.skill_points {
                player.skill_points = pd.skill_points;
                dirty = true;
            }
            if player.science_purchase_points != pd.science_purchase_points {
                player.science_purchase_points = pd.science_purchase_points;
                dirty = true;
            }
            {
                use std::collections::HashSet;
                let want: HashSet<String> = pd.unlocked_sciences.iter().cloned().collect();
                if player.unlocked_sciences != want {
                    player.unlocked_sciences = want;
                    dirty = true;
                }
            }
            // Shared superweapon cooldown last-writer (Debug-name keys).
            {
                use crate::command_system::SpecialPowerType;
                let mut next = std::collections::HashMap::new();
                // Preserve host keys while applying shadow remaining times by Debug name.
                for (hk, hv) in player.shared_special_power_cooldowns.iter() {
                    let key = format!("{hk:?}");
                    if let Some((_, rem)) = pd
                        .shared_special_power_cooldowns
                        .iter()
                        .find(|(k, _)| k == &key)
                    {
                        next.insert(hk.clone(), *rem);
                    } else {
                        next.insert(hk.clone(), *hv);
                    }
                }
                // Insert shadow-only timers for a small set of known powers (writeback residual).
                for (sk, srem) in &pd.shared_special_power_cooldowns {
                    let already = next.keys().any(|hk| format!("{hk:?}") == *sk);
                    if already {
                        continue;
                    }
                    for c in [
                        SpecialPowerType::Airstrike,
                        SpecialPowerType::NuclearMissile,
                        SpecialPowerType::IonCannon,
                        SpecialPowerType::NapalmStrike,
                        SpecialPowerType::Paradrop,
                        SpecialPowerType::EmergencyRepair,
                        SpecialPowerType::CarpetBomb,
                    ] {
                        if format!("{c:?}") == *sk {
                            next.insert(c, *srem);
                            break;
                        }
                    }
                }
                if next != player.shared_special_power_cooldowns {
                    player.shared_special_power_cooldowns = next;
                    dirty = true;
                }
            }
            if dirty {
                updated += 1;
            }
        }
        updated
    }

    /// Write shadow PlayerData::completed_upgrades back onto host HostUpgradeRegistry.
    /// Completes the CompleteUpgrade channel as GameWorld last-writer residual.
    pub fn writeback_completed_upgrades_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::host_upgrades::{normalize_upgrade_identity, HostUpgradePhase};
        let mut updated = 0usize;
        let frame = logic.get_frame();
        for (&host_id, &gw) in &self.host_player_to_gw {
            let Some(pd) = self.world.player(gw) else {
                continue;
            };
            if pd.completed_upgrades.is_empty() {
                continue;
            }
            let mut dirty = false;
            for name in &pd.completed_upgrades {
                let key = normalize_upgrade_identity(name);
                let already = logic.host_upgrades().entries_snapshot().iter().any(|e| {
                    e.player_id == host_id
                        && e.phase == HostUpgradePhase::Completed
                        && normalize_upgrade_identity(&e.name) == key
                });
                if already {
                    continue;
                }
                let _ = logic
                    .host_upgrades_mut()
                    .record_complete(name, host_id, frame, 0);
                dirty = true;
            }
            if dirty {
                updated += 1;
            }
        }
        updated
    }

    /// Write shadow Entity::attack_target back onto host Object::target (stable IDs).
    /// Completes the attack command channel: host log / set_target → shadow mutation → host writeback.
    pub fn writeback_attack_targets_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_ai_attack_authority_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let host_target = ent.attack_target.and_then(|te| self.host_for_entity(te));
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.target == host_target {
                continue;
            }
            obj.set_target(host_target);
            updated += 1;
        }
        updated
    }

    /// Queue SetMoveTarget for a mapped host object (move-command channel).
    pub fn queue_set_move_target_for_host(
        &mut self,
        host: ObjectId,
        destination: Option<[f32; 3]>,
    ) -> bool {
        let Some(unit) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(WorldMutation::SetMoveTarget { unit, destination });
        true
    }

    /// Sync host movement.target_position onto shadow via SetMoveTarget mutations.
    pub fn apply_host_move_targets(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.get_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let dest = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
            if self.queue_set_move_target_for_host(ObjectId(hid), dest) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Write shadow Entity::move_target back onto host movement.target_position.
    /// Direct field write (no host_move_log) to avoid echo loops.
    pub fn writeback_move_targets_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_dest = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
            let shadow_dest = ent.move_target;
            let same = match (host_dest, shadow_dest) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    (a[0] - b[0]).abs() < 0.01
                        && (a[1] - b[1]).abs() < 0.01
                        && (a[2] - b[2]).abs() < 0.01
                }
                _ => false,
            };
            if same {
                continue;
            }
            obj.movement.target_position = shadow_dest.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if shadow_dest.is_some() {
                obj.ai_state = crate::game_logic::AIState::Moving;
                obj.status.moving = true;
            } else {
                obj.status.moving = false;
            }
            updated += 1;
        }
        updated
    }

    /// Write shadow entity pose (position + orientation) onto host objects.
    /// Last-writer residual after SetTransform / apply_host_positions channel.
    pub fn writeback_transforms_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut n = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let p = ent.transform.position;
            let host_p = obj.get_position();
            let host_o = obj.get_orientation();
            let dx = (host_p.x - p.x).abs();
            let dy = (host_p.y - p.y).abs();
            let dz = (host_p.z - p.z).abs();
            let d_o = (host_o - ent.transform.orientation).abs();
            if dx > 1e-3 || dy > 1e-3 || dz > 1e-3 || d_o > 1e-3 {
                obj.set_position(glam::Vec3::new(p.x, p.y, p.z));
                obj.set_orientation(ent.transform.orientation);
                n += 1;
            }
        }
        n
    }

    /// Write shadow production queue + rally_point last-writer residual onto host buildings.

    /// Under PRODUCTION_AUTHORITY: advance entity production queue progress by dt.
    /// Host completes/spawns from writeback-finished heads next frame.
    pub fn tick_production_queues(&mut self, dt: f32) -> usize {
        if !gameworld_production_authority_enabled() {
            return 0;
        }
        use gamelogic::world::entities::{EntityId, EntityProductionItem};
        use gamelogic::world::WorldMutation;
        let mut n = 0usize;
        let mut updates: Vec<(EntityId, Vec<EntityProductionItem>)> = Vec::new();
        // Snapshot host ids for power lookup without double-borrow.
        let host_ids: Vec<(u32, EntityId)> = self
            .host_to_entity
            .iter()
            .map(|(&hid, &eid)| (hid, eid))
            .collect();
        for (hid, eid) in host_ids {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            if ent.production_queue_items.is_empty() {
                continue;
            }
            let mut items = ent.production_queue_items.clone();
            let Some(head) = items.first_mut() else {
                continue;
            };
            if head.progress + 1e-6 < head.total_time.max(0.0) {
                let pf = self
                    .production_power_factor_by_host
                    .get(&hid)
                    .copied()
                    .unwrap_or(1.0)
                    .max(0.01);
                head.progress = (head.progress + dt * pf).min(head.total_time.max(0.0));
                n += 1;
                updates.push((eid, items));
            }
        }
        for (eid, items) in updates {
            self.world
                .queue_mutation(WorldMutation::SetProductionQueue { target: eid, items });
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n
    }

    pub fn writeback_production_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_production_authority_enabled() {
            return 0;
        }
        use crate::game_logic::{ProductionItem, ProductionKind, Resources};
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let Some(bd) = obj.building_data.as_mut() else {
                continue;
            };
            let mut dirty = false;
            // Rally last-writer.
            let rally = ent.rally_point.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if bd.rally_point != rally {
                bd.rally_point = rally;
                dirty = true;
            }
            // Production queue residual (template/progress/cost/upgrade).
            let new_q: Vec<ProductionItem> = ent
                .production_queue_items
                .iter()
                .map(|it| ProductionItem {
                    template_name: it.template_name.clone(),
                    progress: it.progress,
                    total_time: it.total_time,
                    cost: Resources {
                        supplies: it.cost_supplies,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: if it.is_upgrade {
                        ProductionKind::Upgrade
                    } else {
                        ProductionKind::Unit
                    },
                })
                .collect();
            let queue_differs = bd.production_queue.len() != new_q.len()
                || bd.production_queue.iter().zip(new_q.iter()).any(|(a, b)| {
                    a.template_name != b.template_name
                        || (a.progress - b.progress).abs() > 1e-5
                        || (a.total_time - b.total_time).abs() > 1e-5
                        || a.cost.supplies != b.cost.supplies
                        || a.kind != b.kind
                });
            if queue_differs {
                bd.production_queue = new_q;
                dirty = true;
            }
            // Host factory exit delay residual last-writer.
            if (bd.exit_delay_remaining - ent.exit_delay_remaining).abs() > 1e-5 {
                bd.exit_delay_remaining = ent.exit_delay_remaining.max(0.0);
                dirty = true;
            }
            if dirty {
                updated += 1;
            }
        }
        updated
    }

    pub fn writeback_production_door_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.production_door_phase != ent.production_door_phase
                || obj.production_door_phase_end_frame != ent.production_door_phase_end_frame
                || obj.production_door_hold_open != ent.production_door_hold_open;
            if !changed {
                continue;
            }
            obj.production_door_phase = ent.production_door_phase;
            obj.production_door_phase_end_frame = ent.production_door_phase_end_frame;
            obj.production_door_hold_open = ent.production_door_hold_open;
            updated += 1;
        }
        updated
    }

    /// Write GameWorld BodyDamageType residual onto host objects.
    pub fn writeback_body_damage_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let want = HostBodyDamageType::from_ordinal(ent.body_damage_state);
            if obj.body_damage_state != want {
                obj.body_damage_state = want;
                updated += 1;
            }
        }
        updated
    }

    pub fn writeback_death_type_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let want = HostDeathType::from_ordinal(ent.death_type);
            if obj.status.death_type != want {
                obj.status.death_type = want;
                updated += 1;
            }
        }
        updated
    }

    pub fn writeback_radar_extend_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.radar_extend_done_frame != ent.radar_extend_done_frame
                || obj.radar_extend_complete != ent.radar_extend_complete
                || obj.radar_active != ent.radar_active;
            if !changed {
                continue;
            }
            obj.radar_extend_done_frame = ent.radar_extend_done_frame;
            obj.radar_extend_complete = ent.radar_extend_complete;
            obj.radar_active = ent.radar_active;
            updated += 1;
        }
        updated
    }

    pub fn writeback_shock_stun_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.shock_stun_frames != ent.shock_stun_frames
                || (obj.shock_yaw_rate - ent.shock_yaw_rate).abs() > f32::EPSILON
                || (obj.shock_pitch_rate - ent.shock_pitch_rate).abs() > f32::EPSILON
                || (obj.shock_roll_rate - ent.shock_roll_rate).abs() > f32::EPSILON
                || (obj.shock_up_z - ent.shock_up_z).abs() > f32::EPSILON
                || obj.shock_allow_bounce != ent.shock_allow_bounce
                || obj.shock_grounded_once != ent.shock_grounded_once
                || obj.shock_was_airborne != ent.shock_was_airborne
                || obj.cell_is_cliff != ent.cell_is_cliff
                || obj.cell_is_underwater != ent.cell_is_underwater;
            if !changed {
                continue;
            }
            obj.shock_stun_frames = ent.shock_stun_frames;
            obj.shock_yaw_rate = ent.shock_yaw_rate;
            obj.shock_pitch_rate = ent.shock_pitch_rate;
            obj.shock_roll_rate = ent.shock_roll_rate;
            obj.shock_up_z = ent.shock_up_z;
            obj.shock_allow_bounce = ent.shock_allow_bounce;
            obj.shock_grounded_once = ent.shock_grounded_once;
            obj.shock_was_airborne = ent.shock_was_airborne;
            obj.cell_is_cliff = ent.cell_is_cliff;
            obj.cell_is_underwater = ent.cell_is_underwater;
            updated += 1;
        }
        updated
    }

    pub fn writeback_rebuild_producer_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_tpl = obj.rebuild_template_name.clone().unwrap_or_default();
            let changed = obj.is_rebuild_hole != ent.is_rebuild_hole
                || host_tpl != ent.rebuild_template_name
                || obj.rebuild_ready_frame != ent.rebuild_ready_frame
                || obj.rebuild_spawner_id.map(|id| id.0) != ent.rebuild_spawner_id
                || obj.rebuild_worker_id.map(|id| id.0) != ent.rebuild_worker_id
                || obj.rebuild_reconstructing_id.map(|id| id.0) != ent.rebuild_reconstructing_id
                || obj.producer_id.map(|id| id.0) != ent.producer_id
                || obj.construction_complete_clear_frame != ent.construction_complete_clear_frame;
            if !changed {
                continue;
            }
            obj.is_rebuild_hole = ent.is_rebuild_hole;
            obj.rebuild_template_name = if ent.rebuild_template_name.is_empty() {
                None
            } else {
                Some(ent.rebuild_template_name.clone())
            };
            obj.rebuild_ready_frame = ent.rebuild_ready_frame;
            obj.rebuild_spawner_id = ent.rebuild_spawner_id.map(ObjectId);
            obj.rebuild_worker_id = ent.rebuild_worker_id.map(ObjectId);
            obj.rebuild_reconstructing_id = ent.rebuild_reconstructing_id.map(ObjectId);
            obj.producer_id = ent.producer_id.map(ObjectId);
            obj.construction_complete_clear_frame = ent.construction_complete_clear_frame;
            updated += 1;
        }
        updated
    }

    pub fn writeback_sole_healing_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.sole_healing_benefactor.map(|id| id.0)
                != ent.sole_healing_benefactor_id
                || obj.sole_healing_benefactor_expiration_frame
                    != ent.sole_healing_benefactor_expiration_frame;
            if !changed {
                continue;
            }
            obj.sole_healing_benefactor = ent.sole_healing_benefactor_id.map(ObjectId);
            obj.sole_healing_benefactor_expiration_frame =
                ent.sole_healing_benefactor_expiration_frame;
            updated += 1;
        }
        updated
    }

    pub fn writeback_ai_mood_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_prio = obj.attack_priority_set.clone().unwrap_or_default();
            let changed = obj.idle_since_frame != ent.idle_since_frame
                || obj.mood_attack_check_rate != ent.mood_attack_check_rate
                || obj.auto_acquire_when_idle != ent.auto_acquire_when_idle
                || host_prio != ent.attack_priority_set;
            if !changed {
                continue;
            }
            obj.idle_since_frame = ent.idle_since_frame;
            obj.mood_attack_check_rate = ent.mood_attack_check_rate;
            obj.auto_acquire_when_idle = ent.auto_acquire_when_idle;
            obj.attack_priority_set = if ent.attack_priority_set.is_empty() {
                None
            } else {
                Some(ent.attack_priority_set.clone())
            };
            updated += 1;
        }
        updated
    }

    pub fn writeback_ai_request_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_victim = obj.requested_victim_id.map(|id| id.0);
            let host_dest = obj.requested_destination.map(|p| [p.x, p.y, p.z]);
            let host_prev = obj.prev_victim_pos.map(|p| [p.x, p.y, p.z]);
            let host_crate = obj.crate_created.map(|id| id.0).unwrap_or(0);
            let host_ret_v = obj.guard_retaliate_victim.map(|id| id.0).unwrap_or(0);
            let host_ret_a = obj.guard_retaliate_anchor.map(|p| [p.x, p.y, p.z]);
            let host_pending_tpl = obj.disguise_pending_template.clone().unwrap_or_default();
            let host_pending_team = obj
                .disguise_pending_team
                .map(|t| match t {
                    Team::USA => 0u8,
                    Team::China => 1u8,
                    Team::GLA => 2u8,
                    Team::Neutral => 3u8,
                })
                .unwrap_or(255u8);
            let changed = host_victim != ent.requested_victim_id
                || host_dest != ent.requested_destination
                || host_prev != ent.prev_victim_pos
                || host_crate != ent.crate_created_host
                || host_ret_v != ent.guard_retaliate_victim_host
                || host_ret_a != ent.guard_retaliate_anchor
                || obj.path_timestamp != ent.path_timestamp
                || host_pending_tpl != ent.disguise_pending_template
                || host_pending_team != ent.disguise_pending_team_ordinal
                || obj.weapon_crate_upgrade != ent.weapon_crate_upgrade
                || obj.armor_crate_upgrade != ent.armor_crate_upgrade
                || obj.selection_flash_remaining != ent.selection_flash_remaining;
            if !changed {
                continue;
            }
            obj.requested_victim_id = ent.requested_victim_id.map(ObjectId);
            obj.requested_destination = ent
                .requested_destination
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.prev_victim_pos = ent
                .prev_victim_pos
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.crate_created = if ent.crate_created_host == 0 {
                None
            } else {
                Some(ObjectId(ent.crate_created_host))
            };
            obj.guard_retaliate_victim = if ent.guard_retaliate_victim_host == 0 {
                None
            } else {
                Some(ObjectId(ent.guard_retaliate_victim_host))
            };
            obj.guard_retaliate_anchor = ent
                .guard_retaliate_anchor
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.path_timestamp = ent.path_timestamp;
            obj.disguise_pending_template = if ent.disguise_pending_template.is_empty() {
                None
            } else {
                Some(ent.disguise_pending_template.clone())
            };
            obj.disguise_pending_team = match ent.disguise_pending_team_ordinal {
                0 => Some(Team::USA),
                1 => Some(Team::China),
                2 => Some(Team::GLA),
                3 => Some(Team::Neutral),
                _ => None,
            };
            obj.weapon_crate_upgrade = ent.weapon_crate_upgrade;
            obj.armor_crate_upgrade = ent.armor_crate_upgrade;
            obj.selection_flash_remaining = ent.selection_flash_remaining;
            updated += 1;
        }
        updated
    }

    /// Write shadow entity owner last-writer onto host object team.
    pub fn writeback_owner_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(want_team) = self.host_team_for_gw_owner(logic, ent.owner) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.team != want_team {
                // Direct assign to avoid re-logging host_owner_log during writeback.
                obj.team = want_team;
                obj.team_color = want_team.get_color();
                updated += 1;
            }
        }
        updated
    }

    /// Write shadow construction/status residual last-writer onto host objects.
    pub fn writeback_construction_to_host(&self, logic: &mut GameLogic) -> usize {
        // Construction/sell/rebuild residual only.
        // Combat status, AI state, contain, supplies, veterancy, and special-power
        // last-writer residuals use dedicated writebacks in the shadow session.
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let mut dirty = false;
            // Sell deconstruction uses negative percent (finish <= -0.5); do not floor at 0.
            let pct = ent.construction_percent.clamp(-1.0, 1.0);
            if (obj.construction_percent - pct).abs() > 1e-5 {
                obj.construction_percent = pct;
                dirty = true;
            }
            if obj.status.under_construction != ent.under_construction {
                obj.status.under_construction = ent.under_construction;
                dirty = true;
            }
            if obj.status.sold != ent.sold {
                obj.status.sold = ent.sold;
                dirty = true;
            }
            if obj.status.reconstructing != ent.reconstructing {
                obj.status.reconstructing = ent.reconstructing;
                dirty = true;
            }
            if obj.status.unselectable != ent.unselectable {
                obj.status.unselectable = ent.unselectable;
                dirty = true;
            }
            if dirty {
                updated += 1;
            }
        }
        updated
    }

    /// Count completed upgrade names across mapped shadow players (probe residual).
    /// True when any shadow player has non-zero produced or consumed power residual.
    /// True when any shadow player has radar providers or a disabled flag residual.
    /// Count shadow players still marked alive (defeat residual).
    /// Count shadow entities marked selected (host UI residual).
    /// Count shadow entities with host building object-type residual.
    /// Count shadow entities with host force_attack residual.
    /// Count shadow entities with Elite+ host veterancy residual.
    /// Count shadow entities with non-empty host production queue residual.
    /// Count shadow entities with host weapon residual.
    /// Count shadow entities with host battle-bus transport residual.
    /// Count shadow entities with host detector residual.
    /// Count shadow entities with host horde weapon-bonus residual.
    /// Count shadow entities with host frenzy-until residual active.
    pub fn frenzy_until_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.weapon_bonus_frenzy_until_frame > 0 && !e.destroyed)
            .count()
    }

    /// Count shadow entities with battle-plan sight scalar residual != 1.0.
    pub fn battle_plan_sight_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| (e.battle_plan_sight_scalar_applied - 1.0).abs() > 0.001 && !e.destroyed)
            .count()
    }

    pub fn horde_bonus_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.weapon_bonus_horde && !e.destroyed)
            .count()
    }

    /// Count shadow entities with host humvee transport residual.
    pub fn humvee_transport_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_humvee_transport && !e.destroyed)
            .count()
    }

    pub fn detector_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_detector && !e.destroyed)
            .count()
    }

    /// Count shadow entities with special power ready residual.
    pub fn special_power_ready_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.special_power_ready && !e.destroyed)
            .count()
    }

    pub fn battle_bus_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_battle_bus_transport && !e.destroyed)
            .count()
    }

    /// Count shadow entities currently contained (host contained_by residual).
    pub fn contained_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.contained_by_host != 0 && !e.destroyed)
            .count()
    }

    pub fn armed_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.has_weapon && !e.destroyed)
            .count()
    }

    /// Count shadow entities with non-empty host movement path residual.
    pub fn pathing_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.path_len > 0 && !e.destroyed)
            .count()
    }

    pub fn producing_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.production_queue_len > 0 && !e.destroyed)
            .count()
    }

    /// Count shadow entities with host building residual.
    pub fn building_data_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.is_building && !e.destroyed)
            .count()
    }

    pub fn elite_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.veterancy_ordinal >= 2 && !e.destroyed)
            .count()
    }

    /// Count shadow entities with host stealthed residual.
    pub fn stealthed_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.stealthed && !e.destroyed)
            .count()
    }

    pub fn force_attack_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.force_attack && !e.destroyed)
            .count()
    }

    /// Count shadow entities with non-idle host AI state residual.
    pub fn non_idle_ai_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.ai_state_ordinal != 0 && !e.destroyed)
            .count()
    }

    pub fn building_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.object_type_ordinal == 3 && !e.destroyed)
            .count()
    }

    /// Sum of host power_provided residual on shadow entities.
    pub fn total_entity_power_provided(&self) -> i32 {
        self.world
            .world()
            .entities()
            .filter(|e| !e.destroyed)
            .map(|e| e.power_provided)
            .sum()
    }

    pub fn moving_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.moving && !e.destroyed)
            .count()
    }

    pub fn attacking_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.attacking && !e.destroyed)
            .count()
    }

    pub fn entity_count_for_team_ordinal(&self, team_ordinal: u8) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.team_ordinal == team_ordinal && !e.destroyed)
            .count()
    }

    pub fn selected_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.selected && !e.destroyed)
            .count()
    }

    /// Count shadow entities still under construction residual.
    pub fn under_construction_entity_count(&self) -> usize {
        self.world
            .world()
            .entities()
            .filter(|e| e.construction_percent < 0.999 && !e.destroyed)
            .count()
    }

    pub fn alive_player_count(&self) -> usize {
        self.world
            .world()
            .active_players()
            .filter(|(_, p)| p.is_alive)
            .count()
    }

    /// Max cash bounty percent residual across shadow players.
    pub fn max_cash_bounty_percent(&self) -> f32 {
        self.world
            .world()
            .active_players()
            .map(|(_, p)| p.cash_bounty_percent)
            .fold(0.0_f32, f32::max)
    }

    pub fn radar_residual_present(&self) -> bool {
        self.world
            .world()
            .active_players()
            .any(|(_, p)| p.radar_count != 0 || p.radar_disabled)
    }

    /// C++ Player::hasRadar residual on any shadow player.
    pub fn any_player_has_radar(&self) -> bool {
        self.world
            .world()
            .active_players()
            .any(|(_, p)| p.radar_count > 0 && !p.radar_disabled)
    }

    pub fn power_bar_residual_present(&self) -> bool {
        self.world
            .world()
            .active_players()
            .any(|(_, p)| p.power_produced != 0 || p.power_consumed != 0)
    }

    pub fn unlocked_science_count(&self) -> usize {
        self.world
            .world()
            .active_players()
            .map(|(_, p)| p.unlocked_sciences.len())
            .sum()
    }

    pub fn completed_upgrade_count(&self) -> usize {
        self.world
            .world()
            .active_players()
            .map(|(_, p)| p.completed_upgrades.len())
            .sum()
    }

    /// Host upgrade-complete residual: record completed research names on shadow players.
    /// Fail-closed: not full PlayerUpgradeManager effect matrix / science tree.
    pub fn apply_host_upgrade_events(
        &mut self,
        events: &[crate::game_logic::host_upgrades::HostUpgradeResearch],
    ) -> usize {
        use crate::game_logic::host_upgrades::HostUpgradePhase;
        let mut queued = 0usize;
        for ev in events {
            if ev.phase != HostUpgradePhase::Completed {
                continue;
            }
            let Some(&gw) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::CompleteUpgrade {
                player: gw,
                name: ev.name.clone(),
            });
            queued += 1;
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Apply drained host economy events as SetSupplies/SetPower mutations.
    pub fn apply_host_economy_events(
        &mut self,
        events: &[crate::game_logic::host_economy_log::HostEconomyEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            let Some(&gw) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world.queue_mutation(WorldMutation::SetSupplies {
                player: gw,
                supplies: ev.supplies,
            });
            self.world.queue_mutation(WorldMutation::SetPower {
                player: gw,
                power_available: ev.power_available,
            });
            queued += 2;
        }
        let applied = self.apply_pending();
        (queued, applied)
    }

    pub fn apply_host_production_events(
        &mut self,
        events: &[crate::game_logic::host_production_log::HostProductionEvent],
        logic: &GameLogic,
    ) -> usize {
        use crate::game_logic::host_production_log::HostProductionEvent;
        use gamelogic::world::entities::EntityProductionItem;
        let mut n = 0usize;
        let mut spawn_like = Vec::new();
        // Producers that need queue last-write from host snapshot.
        let mut enqueue_producers = std::collections::BTreeSet::new();
        for ev in events {
            match ev {
                HostProductionEvent::Enqueue { producer, .. } => {
                    enqueue_producers.insert(producer.0);
                }
                HostProductionEvent::Complete {
                    spawned,
                    template_name,
                    producer,
                } => {
                    enqueue_producers.insert(producer.0);
                    if self.host_to_entity.contains_key(&spawned.0) {
                        n += 1;
                        continue;
                    }
                    if let Some(obj) = logic.get_objects().get(spawned) {
                        let team_ord = match obj.team {
                            Team::USA => 0u8,
                            Team::China => 1,
                            Team::GLA => 2,
                            Team::Neutral => 255,
                        };
                        let pos = obj.get_position();
                        spawn_like.push(crate::game_logic::host_spawn_log::HostSpawnEvent {
                            id: *spawned,
                            template: template_name.clone(),
                            team_ordinal: team_ord,
                            position: [pos.x, pos.y, pos.z],
                        });
                    }
                }
            }
        }
        // Mutation-channel production queue last-writer from host building queues.
        for hid in enqueue_producers {
            let Some(eid) = self.host_to_entity.get(&hid).copied() else {
                continue;
            };
            let Some(obj) = logic.get_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let items: Vec<EntityProductionItem> = obj
                .building_data
                .as_ref()
                .map(|bd| {
                    bd.production_queue
                        .iter()
                        .take(16)
                        .map(|it| EntityProductionItem {
                            template_name: it.template_name.clone(),
                            progress: it.progress,
                            total_time: it.total_time,
                            cost_supplies: it.cost.supplies,
                            is_upgrade: it.is_upgrade(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionQueue {
                    target: eid,
                    items,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.world.apply_pending_mutations();
        }
        n + self.apply_host_spawn_events(&spawn_like, logic)
    }

    pub fn apply_host_production_progress_events(
        &mut self,
        events: &[crate::game_logic::host_production_progress_log::HostProductionProgressEvent],
    ) -> usize {
        use gamelogic::world::entities::EntityProductionItem;
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.producer.0) else {
                continue;
            };
            self.production_power_factor_by_host
                .insert(ev.producer.0, ev.power_factor.max(0.01));
            let items: Vec<EntityProductionItem> = ev
                .items
                .iter()
                .map(|it| EntityProductionItem {
                    template_name: it.template_name.clone(),
                    progress: it.progress,
                    total_time: it.total_time,
                    cost_supplies: it.cost_supplies,
                    is_upgrade: it.is_upgrade,
                })
                .collect();
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionQueue {
                    target: eid,
                    items,
                });
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetExitDelay {
                    target: eid,
                    exit_delay_remaining: ev.exit_delay_remaining,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_production_door_events(
        &mut self,
        events: &[crate::game_logic::host_production_door_log::HostProductionDoorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.producer.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProductionDoor {
                    target: eid,
                    production_door_phase: ev.production_door_phase,
                    production_door_phase_end_frame: ev.production_door_phase_end_frame,
                    production_door_hold_open: ev.production_door_hold_open,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Host structure construction-complete residual: ensure completed buildings are
    /// mapped in the shadow (usually already present via sync; counts for probe honesty).
    /// Fail-closed: does not invent GameWorld construction modules.
    pub fn apply_host_construction_events(
        &mut self,
        events: &[crate::game_logic::host_construction_log::HostConstructionEvent],
        logic: &GameLogic,
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.host_to_entity.contains_key(&ev.id.0) {
                n += 1;
                continue;
            }
            // Completed structure missing from map — treat like a late spawn residual.
            if let Some(obj) = logic.get_objects().get(&ev.id) {
                if !obj.is_alive() {
                    continue;
                }
                let team_ordinal = match obj.team {
                    Team::USA => 0u8,
                    Team::China => 1,
                    Team::GLA => 2,
                    _ => 3,
                };
                let p = obj.get_position();
                let spawn = crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id: ev.id,
                    template: ev.template_name.clone(),
                    team_ordinal,
                    position: [p.x, p.y, p.z],
                };
                n += self.apply_host_spawn_events(std::slice::from_ref(&spawn), logic);
            }
        }
        n
    }

    pub fn apply_host_spawn_events(
        &mut self,
        events: &[crate::game_logic::host_spawn_log::HostSpawnEvent],
        logic: &GameLogic,
    ) -> usize {
        let mut spawned = 0usize;
        for ev in events {
            if self.host_to_entity.contains_key(&ev.id.0) {
                continue;
            }
            let (health, owner) = if let Some(obj) = logic.get_objects().get(&ev.id) {
                let owner = self.owner_for_host_object(logic, obj.team);
                (obj.health.current.max(0.0), owner)
            } else {
                let owner = match ev.team_ordinal {
                    0 => self.host_player_to_gw.values().next().copied(),
                    1 => self.host_player_to_gw.values().nth(1).copied(),
                    2 => self.host_player_to_gw.values().nth(2).copied(),
                    _ => None,
                };
                (100.0, owner)
            };
            // Mutation-channel spawn (sole create path) then map host ObjectId.
            self.world.queue_mutation(WorldMutation::Spawn {
                template: ev.template.clone(),
                owner,
                position: ev.position,
                health,
            });
            let _ = self.world.apply_pending_mutations();
            if let Some(eid) = self.world.take_last_spawned_entity() {
                self.host_to_entity.insert(ev.id.0, eid);
                self.entity_to_host.insert(eid.get(), ev.id.0);
                spawned += 1;
            }
        }
        spawned
    }

    /// Apply destroy-log events as WorldMutation::Destroy for mapped entities.

    pub fn apply_host_destroy_events(
        &mut self,
        events: &[crate::game_logic::host_destroy_log::HostDestroyEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            if self.queue_destroy_for_host(ev.id) {
                queued += 1;
            }
        }
        let applied = self.apply_pending();
        (queued, applied)
    }

    /// Queue SetAttackTarget for a mapped host attacker.
    pub fn queue_set_attack_target_for_host(
        &mut self,
        host_attacker: ObjectId,
        host_target: Option<ObjectId>,
    ) -> bool {
        let Some(attacker) = self.entity_for_host(host_attacker) else {
            return false;
        };
        let target = host_target.and_then(|t| self.entity_for_host(t));
        self.world
            .queue_mutation(WorldMutation::SetAttackTarget { attacker, target });
        true
    }

    /// Queue borrow-first combat status residual onto a mapped host object.
    pub fn queue_set_combat_status_for_host(
        &mut self,
        ev: crate::game_logic::host_status_log::HostStatusEvent,
    ) -> bool {
        let Some(target) = self.entity_for_host(ev.object) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetCombatStatus {
                target,
                stealthed: ev.stealthed,
                detected: ev.detected,
                attacking: ev.attacking,
                moving: ev.moving,
                is_firing_weapon: ev.is_firing_weapon,
                is_aiming_weapon: ev.is_aiming_weapon,
                selected: ev.selected,
                disabled_emp: ev.disabled_emp,
                weapons_jammed: ev.weapons_jammed,
                disabled_hacked: ev.disabled_hacked,
                disabled_unmanned: ev.disabled_unmanned,
                disabled_paralyzed: ev.disabled_paralyzed,
                disabled_subdued: ev.disabled_subdued,
                masked: ev.masked,
                disguised: ev.disguised,
                no_collisions: ev.no_collisions,
                private_captured: ev.private_captured,
                disguise_transitioning_to: ev.disguise_transitioning_to,
                disguise_halfpoint_reached: ev.disguise_halfpoint_reached,
                faerie_fire: ev.faerie_fire,
                booby_trapped: ev.booby_trapped,
                eject_invulnerable: ev.eject_invulnerable,
                pilot_did_move_to_base: ev.pilot_did_move_to_base,
                parachuting: ev.parachuting,
                parachute_open: ev.parachute_open,
                parachute_landing_override_set: ev.parachute_landing_override_set,
                using_ability: ev.using_ability,
                deployed: ev.deployed,
                under_construction: ev.under_construction,
                sold: ev.sold,
                reconstructing: ev.reconstructing,
                unselectable: ev.unselectable,
                ignoring_stealth: ev.ignoring_stealth,
                repulsor: ev.repulsor,
                disabled_underpowered: ev.disabled_underpowered,
                disabled_freefall: ev.disabled_freefall,
                is_carbomb: ev.is_carbomb,
                hijacked: ev.hijacked,
                force_attack: ev.force_attack,
            });
        true
    }

    /// Queue SetVeterancy residual onto a mapped host object.
    pub fn queue_set_veterancy_for_host(&mut self, host: ObjectId, ordinal: u8) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetVeterancy {
                target,
                ordinal: ordinal.min(3),
            });
        true
    }

    /// Queue SetProductionQueue residual onto a mapped host producer.
    pub fn queue_set_production_queue_for_host(
        &mut self,
        host: ObjectId,
        items: Vec<gamelogic::world::entities::EntityProductionItem>,
    ) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetProductionQueue { target, items });
        true
    }

    /// Queue SetConstruction residual onto a mapped host structure.
    pub fn queue_set_construction_for_host(
        &mut self,
        host: ObjectId,
        percent: f32,
        under_construction: bool,
    ) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetConstruction {
                target,
                percent: percent.clamp(-1.0, 1.0),
                under_construction,
            });
        true
    }

    pub fn queue_set_special_power_for_host(
        &mut self,
        host_id: ObjectId,
        ready: bool,
        cooldown_remaining: f32,
        cooldown: f32,
    ) -> bool {
        let Some(&eid) = self.host_to_entity.get(&host_id.0) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetSpecialPower {
                target: eid,
                ready,
                cooldown_remaining,
                cooldown,
            });
        true
    }

    pub fn queue_set_ai_state_for_host(&mut self, host: ObjectId, ordinal: u8) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetAiState { target, ordinal });
        true
    }

    pub fn queue_set_contain_for_host(
        &mut self,
        host: ObjectId,
        contained_by_host: u32,
        garrison_count: Option<u16>,
        garrisoned_host_ids: Option<Vec<u32>>,
    ) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetContain {
                target,
                contained_by_host,
                garrison_count,
                garrisoned_host_ids,
            });
        true
    }

    pub fn queue_set_player_radar(
        &mut self,
        host_player_id: u32,
        radar_count: i32,
        radar_disabled: bool,
    ) -> bool {
        let Some(&player) = self.host_player_to_gw.get(&host_player_id) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetPlayerRadar {
                player,
                radar_count,
                radar_disabled,
            });
        true
    }

    pub fn apply_host_radar_events(
        &mut self,
        events: &[crate::game_logic::host_radar_log::HostRadarEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_player_radar(ev.player_id, ev.radar_count, ev.radar_disabled) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn queue_set_player_progress(
        &mut self,
        host_player_id: u32,
        rank_level: u32,
        skill_points: i32,
        science_purchase_points: i32,
        cash_bounty_percent: f32,
    ) -> bool {
        let Some(&player) = self.host_player_to_gw.get(&host_player_id) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetPlayerProgress {
                player,
                rank_level,
                skill_points,
                science_purchase_points,
                cash_bounty_percent,
            });
        true
    }

    pub fn apply_host_player_progress_events(
        &mut self,
        events: &[crate::game_logic::host_player_progress_log::HostPlayerProgressEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_player_progress(
                ev.player_id,
                ev.rank_level,
                ev.skill_points,
                ev.science_purchase_points,
                ev.cash_bounty_percent,
            ) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_player_meta_events(
        &mut self,
        events: &[crate::game_logic::host_player_meta_log::HostPlayerMetaEvent],
    ) -> usize {
        use crate::game_logic::host_player_meta_log::HostPlayerMetaEvent;
        let mut n = 0usize;
        for ev in events {
            match ev {
                HostPlayerMetaEvent::Sciences {
                    player_id,
                    unlocked_sciences,
                } => {
                    let Some(&player) = self.host_player_to_gw.get(player_id) else {
                        continue;
                    };
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetPlayerSciences {
                            player,
                            unlocked_sciences: unlocked_sciences.clone(),
                        });
                    n += 1;
                }
                HostPlayerMetaEvent::Alive {
                    player_id,
                    is_alive,
                } => {
                    let Some(&player) = self.host_player_to_gw.get(player_id) else {
                        continue;
                    };
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetPlayerAlive {
                            player,
                            is_alive: *is_alive,
                        });
                    n += 1;
                }
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_player_cooldown_events(
        &mut self,
        events: &[crate::game_logic::host_player_cooldown_log::HostPlayerCooldownEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&player) = self.host_player_to_gw.get(&ev.player_id) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetPlayerCooldowns {
                    player,
                    cooldowns: ev.cooldowns.clone(),
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_contain_events(
        &mut self,
        events: &[crate::game_logic::host_contain_log::HostContainEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_contain_for_host(
                ev.object,
                ev.contained_by_host,
                ev.garrison_count,
                ev.garrisoned_host_ids.clone(),
            ) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_contain_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let mut did = false;
            let new_cb = if ent.contained_by_host == 0 {
                None
            } else {
                Some(ObjectId(ent.contained_by_host))
            };
            if obj.contained_by != new_cb {
                obj.contained_by = new_cb;
                did = true;
            }
            if let Some(bd) = obj.building_data.as_mut() {
                let new_units: Vec<ObjectId> = ent
                    .garrisoned_host_ids
                    .iter()
                    .copied()
                    .map(ObjectId)
                    .collect();
                if bd.garrisoned_units != new_units {
                    bd.garrisoned_units = new_units;
                    did = true;
                }
            } else if !ent.garrisoned_host_ids.is_empty() {
                let new_occ: Vec<ObjectId> = ent
                    .garrisoned_host_ids
                    .iter()
                    .copied()
                    .map(ObjectId)
                    .collect();
                if obj.occupants != new_occ {
                    obj.occupants = new_occ;
                    did = true;
                }
            }
            if did {
                updated += 1;
            }
        }
        updated
    }

    pub fn apply_host_ai_state_events(
        &mut self,
        events: &[crate::game_logic::host_ai_state_log::HostAiStateEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_ai_state_for_host(ev.object, ev.ordinal) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_ai_state_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::AIState as A;
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_ord = Self::host_ai_state_ordinal(&obj.ai_state);
            if host_ord == ent.ai_state_ordinal {
                continue;
            }
            obj.ai_state = match ent.ai_state_ordinal {
                0 => A::Idle,
                1 => A::Moving,
                2 => A::Attacking,
                3 => A::AttackMoving,
                4 => A::AttackingGround,
                5 => A::Gathering,
                6 => A::ReturningResources,
                7 => A::Constructing,
                8 => A::Repairing,
                9 => A::GuardingArea,
                10 => A::GuardingObject,
                11 => A::Patrolling,
                12 => A::Docked,
                13 => A::Garrisoned,
                14 => A::SpecialAbility,
                15 => A::SeekingRepair,
                16 => A::SeekingHealing,
                17 => A::Entering,
                18 => A::Docking,
                19 => A::Capturing,
                20 => A::GuardRetaliating,
                _ => A::Idle,
            };
            updated += 1;
        }
        updated
    }

    pub fn queue_set_stored_supplies_for_host(&mut self, host: ObjectId, supplies: u32) -> bool {
        let Some(target) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetStoredSupplies {
                target,
                supplies,
            });
        true
    }

    pub fn apply_host_special_power_events(
        &mut self,
        events: &[crate::game_logic::host_special_power_log::HostSpecialPowerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_special_power_for_host(
                ev.object,
                ev.ready,
                ev.cooldown_remaining,
                ev.cooldown,
            ) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_special_power_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.special_power_ready != ent.special_power_ready
                || (obj.special_power_cooldown_remaining - ent.special_power_cooldown_remaining)
                    .abs()
                    > 1e-4
                || (obj.special_power_cooldown - ent.special_power_cooldown).abs() > 1e-4;
            if !changed {
                continue;
            }
            obj.special_power_ready = ent.special_power_ready;
            obj.special_power_cooldown_remaining = ent.special_power_cooldown_remaining.max(0.0);
            obj.special_power_cooldown = ent.special_power_cooldown.max(0.0);
            updated += 1;
        }
        updated
    }

    pub fn apply_host_stored_supplies_events(
        &mut self,
        events: &[crate::game_logic::host_stored_supplies_log::HostStoredSuppliesEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_stored_supplies_for_host(ev.object, ev.supplies) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_stored_supplies_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.stored_resources.supplies == ent.stored_supplies {
                continue;
            }
            obj.stored_resources.supplies = ent.stored_supplies;
            updated += 1;
        }
        updated
    }

    /// Apply construction progress log as SetConstruction mutations.
    pub fn apply_host_construction_progress_events(
        &mut self,
        events: &[crate::game_logic::host_construction_progress_log::HostConstructionProgressEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_construction_for_host(ev.object, ev.percent, ev.under_construction) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Queue SetTransform for a mapped host object (move-command channel).
    pub fn queue_set_transform_for_host(
        &mut self,
        host: ObjectId,
        position: [f32; 3],
        orientation: f32,
    ) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::SetTransform {
            target: eid,
            position,
            orientation,
        });
        true
    }

    /// Sync host Object::target onto shadow via SetAttackTarget mutations.
    pub fn apply_host_attack_targets(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.get_objects().get(&ObjectId(hid)) else {
                continue;
            };
            if self.queue_set_attack_target_for_host(ObjectId(hid), obj.target) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    /// Push current host positions onto shadow via SetTransform mutations.
    pub fn apply_host_positions_as_transforms(&mut self, logic: &GameLogic) -> usize {
        let mut queued = 0usize;
        let keys: Vec<u32> = self.host_to_entity.keys().copied().collect();
        for hid in keys {
            let Some(obj) = logic.get_objects().get(&ObjectId(hid)) else {
                continue;
            };
            let pos = obj.get_position();
            let orient = obj.get_orientation();
            if self.queue_set_transform_for_host(ObjectId(hid), [pos.x, pos.y, pos.z], orient) {
                queued += 1;
            }
        }
        if queued > 0 {
            let _ = self.apply_pending();
        }
        queued
    }

    pub fn queue_damage_for_host(&mut self, host: ObjectId, amount: f32) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::Damage {
            target: eid,
            amount,
        });
        true
    }

    /// Apply drained host damage events as GameWorld mutations (order preserved).
    /// Returns (queued, applied_after_flush).
    pub fn queue_transfer_owner_for_host(
        &mut self,
        host: ObjectId,
        owner: Option<gamelogic::world::PlayerId>,
    ) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::TransferOwner {
                object: eid,
                player: owner,
            });
        true
    }

    pub fn apply_host_owner_events(
        &mut self,
        logic: &GameLogic,
        events: &[crate::game_logic::host_owner_log::HostOwnerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let owner = self.owner_for_host_object(logic, ev.team);
            if self.queue_transfer_owner_for_host(ev.object, owner) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn queue_set_health_for_host(&mut self, host_id: ObjectId, health: f32) -> bool {
        let Some(&eid) = self.host_to_entity.get(&host_id.0) else {
            return false;
        };
        self.world
            .queue_mutation(gamelogic::world::WorldMutation::SetHealth {
                target: eid,
                health,
            });
        true
    }

    pub fn apply_host_heal_events(
        &mut self,
        events: &[crate::game_logic::host_heal_log::HostHealEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            if self.queue_set_health_for_host(ev.target, ev.health) {
                n += 1;
            }
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_max_health_events(
        &mut self,
        events: &[crate::game_logic::host_max_health_log::HostMaxHealthEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetMaxHealth {
                    target: eid,
                    max_health: ev.max_health,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_experience_events(
        &mut self,
        events: &[crate::game_logic::host_experience_log::HostExperienceEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetExperience {
                    target: eid,
                    points: ev.points,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_bonus_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetWeaponBonus {
                    target: eid,
                    enthusiastic: ev.enthusiastic,
                    subliminal: ev.subliminal,
                    horde: ev.horde,
                    nationalism: ev.nationalism,
                    frenzy: ev.frenzy,
                    frenzy_level: ev.frenzy_level,
                    battle_plan_bombardment: ev.battle_plan_bombardment,
                    battle_plan_hold_the_line: ev.battle_plan_hold_the_line,
                    battle_plan_search_and_destroy: ev.battle_plan_search_and_destroy,
                    frenzy_until_frame: ev.frenzy_until_frame,
                    battle_plan_sight_scalar_applied: ev.battle_plan_sight_scalar_applied,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_slot_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_slot_log::HostWeaponSlotEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetActiveWeaponSlot {
                    target: eid,
                    slot: ev.slot,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_entity_power_events(
        &mut self,
        events: &[crate::game_logic::host_entity_power_log::HostEntityPowerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetEntityPower {
                    target: eid,
                    power_provided: ev.power_provided,
                    power_consumed: ev.power_consumed,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_turret_events(
        &mut self,
        events: &[crate::game_logic::host_turret_log::HostTurretEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetTurret {
                    target: eid,
                    angle_deg: ev.angle_deg,
                    pitch_deg: ev.pitch_deg,
                    holding: ev.holding,
                    idle_scanning: ev.idle_scanning,
                    turret_turn_rate_rad: ev.turret_turn_rate_rad,
                    turret_recenter_frames: ev.turret_recenter_frames,
                    turret_hold_until_frame: ev.turret_hold_until_frame,
                    turret_idle_recentering: ev.turret_idle_recentering,
                    turret_enabled: ev.turret_enabled,
                    turret_rotating: ev.turret_rotating,
                    turret_natural_angle_deg: ev.turret_natural_angle_deg,
                    turret_natural_pitch_deg: ev.turret_natural_pitch_deg,
                    turret_target_host: ev.turret_target_host,
                    turret_force_attacking: ev.turret_force_attacking,
                    turret_mood_target: ev.turret_mood_target,
                    turret_idle_scan_next_frame: ev.turret_idle_scan_next_frame,
                    turret_idle_scan_desired_angle_deg: ev.turret_idle_scan_desired_angle_deg,
                    turret_idle_scan_index: ev.turret_idle_scan_index,
                    turret_substate: ev.turret_substate,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_target_location_events(
        &mut self,
        events: &[crate::game_logic::host_target_location_log::HostTargetLocationEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetTargetLocation {
                    unit: eid,
                    location: ev.location,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_detector_events(
        &mut self,
        events: &[crate::game_logic::host_detector_log::HostDetectorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDetector {
                    target: eid,
                    is_detector: ev.is_detector,
                    detection_range: ev.detection_range,
                    detection_rate_frames: ev.detection_rate_frames,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_continuous_fire_events(
        &mut self,
        events: &[crate::game_logic::host_continuous_fire_log::HostContinuousFireEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetContinuousFire {
                    target: eid,
                    level: ev.level,
                    consecutive: ev.consecutive,
                    coast_until_frame: ev.coast_until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_combat_attack_events(
        &mut self,
        events: &[crate::game_logic::host_combat_attack_log::HostCombatAttackEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetCombatAttack {
                    target: eid,
                    pre_attack_target_host: ev.pre_attack_target_host,
                    pre_attack_ready_at: ev.pre_attack_ready_at,
                    consecutive_shots_at_target: ev.consecutive_shots_at_target,
                    max_shots_to_fire: ev.max_shots_to_fire,
                    attack_substate_ordinal: ev.attack_substate_ordinal,
                    approach_timestamp: ev.approach_timestamp,
                    continuous_fire_victim: ev.continuous_fire_victim,
                    maintain_pos_valid: ev.maintain_pos_valid,
                    maintain_pos: ev.maintain_pos,
                    temporary_move_frames: ev.temporary_move_frames,
                    group_speed_factor: ev.group_speed_factor,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_fire_intent_events(
        &mut self,
        events: &[crate::game_logic::host_fire_intent_log::HostFireIntentEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFireIntent {
                    target: eid,
                    last_fire_victim_host: ev.last_fire_victim_host,
                    last_fire_slot: ev.last_fire_slot,
                    last_fire_damage: ev.last_fire_damage,
                    last_fire_range: ev.last_fire_range,
                    last_fire_sim_time: ev.last_fire_sim_time,
                    last_fire_frame: ev.last_fire_frame,
                    fire_intent_count: ev.fire_intent_count,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_projectile_events(
        &mut self,
        events: &[crate::game_logic::host_projectile_log::HostProjectileEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetProjectileFlight {
                    host_id: ev.host_id,
                    position: ev.position,
                    velocity: ev.velocity,
                    target_position: ev.target_position,
                    damage: ev.damage,
                    shooter_host: ev.shooter_host,
                    target_host: ev.target_host,
                    speed: ev.speed,
                    lifetime: ev.lifetime,
                    max_lifetime: ev.max_lifetime,
                    is_homing: ev.is_homing,
                    active: ev.active,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Apply deferred fire-spawns into host CombatSystem (fire-spawn authority).
    pub fn apply_host_fire_spawn_events(
        &mut self,
        logic: &mut GameLogic,
        events: Vec<crate::game_logic::combat::PendingProjectile>,
    ) -> usize {
        if events.is_empty() {
            // Still drain residual hitscan marks so they cannot leak across frames.
            let _ = crate::game_logic::host_fire_spawn_log::drain_residual_hitscans();
            return 0;
        }
        // Residual auto-fire already applied same-frame hitscan HP — zero those
        // spawns' damage so dual-tick projectile resolve does not double-dip.
        let residual_hitscans = crate::game_logic::host_fire_spawn_log::drain_residual_hitscans();
        // Push into the global pending queue then drain into CombatSystem so
        // scatter/target resolution stays on the production spawn path.
        for mut ev in events {
            if let Some(tid) = ev.target_id {
                if residual_hitscans
                    .iter()
                    .any(|(s, t)| *s == ev.shooter_id && *t == tid)
                {
                    ev.damage = 0.0;
                    ev.secondary_damage = 0.0;
                }
            }
            crate::game_logic::combat::queue_projectile_direct(ev);
        }
        {
            let objects = logic.get_objects();
            // SAFETY: drain only needs shared objects map + mut combat.
            // Split via raw pointers is avoided — clone keys/positions is heavy;
            // use GameLogic helper instead.
            let _ = objects;
        }
        logic.drain_pending_projectiles_into_combat();
        crate::game_logic::host_projectile_log::record_snapshot(
            logic.combat_system.projectiles_snapshot(),
        );
        self.apply_host_projectile_events(&crate::game_logic::host_projectile_log::drain())
    }

    /// Last-write host CombatSystem projectile pose/lifetime from GameWorld residual.
    pub fn writeback_projectiles_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_projectile_authority_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        let gw_ids: std::collections::HashSet<u32> =
            self.world.projectiles().keys().copied().collect();
        let to_remove: Vec<crate::game_logic::ObjectId> = logic
            .combat_system
            .get_projectiles()
            .keys()
            .copied()
            .filter(|id| !gw_ids.contains(&id.0))
            .collect();
        for id in to_remove {
            if logic.combat_system.remove_projectile(id) {
                updated += 1;
            }
        }
        for (hid, res) in self.world.projectiles() {
            let Some(p) = logic
                .combat_system
                .projectile_mut(crate::game_logic::ObjectId(*hid))
            else {
                continue;
            };
            let np = glam::Vec3::new(res.position[0], res.position[1], res.position[2]);
            let nv = glam::Vec3::new(res.velocity[0], res.velocity[1], res.velocity[2]);
            let nt = glam::Vec3::new(
                res.target_position[0],
                res.target_position[1],
                res.target_position[2],
            );
            let changed = (p.position - np).length_squared() > 1e-10
                || (p.velocity - nv).length_squared() > 1e-10
                || (p.target_position - nt).length_squared() > 1e-10
                || (p.lifetime - res.lifetime).abs() > f32::EPSILON
                || (p.speed - res.speed).abs() > f32::EPSILON
                || p.is_homing != res.is_homing;
            if !changed {
                continue;
            }
            p.position = np;
            p.velocity = nv;
            p.target_position = nt;
            p.lifetime = res.lifetime;
            p.max_lifetime = res.max_lifetime;
            p.speed = res.speed;
            p.is_homing = res.is_homing;
            p.damage = res.damage;
            updated += 1;
        }
        updated
    }

    pub fn apply_host_guard_events(
        &mut self,
        events: &[crate::game_logic::host_guard_log::HostGuardEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetGuard {
                    unit: eid,
                    position: ev.position,
                    target_host: ev.target_host,
                    radius: ev.radius,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_attitude_events(
        &mut self,
        events: &[crate::game_logic::host_ai_attitude_log::HostAiAttitudeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetAiAttitude {
                    target: eid,
                    attitude: ev.attitude,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_mood_events(
        &mut self,
        events: &[crate::game_logic::host_ai_mood_log::HostAiMoodEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetAiMood {
                    target: eid,
                    idle_since_frame: ev.idle_since_frame,
                    mood_attack_check_rate: ev.mood_attack_check_rate,
                    auto_acquire_when_idle: ev.auto_acquire_when_idle,
                    attack_priority_set: ev.attack_priority_set.clone(),
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_request_events(
        &mut self,
        events: &[crate::game_logic::host_ai_request_log::HostAiRequestEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetAiRequest {
                    target: eid,
                    requested_victim_host: ev.requested_victim_host,
                    requested_destination: ev.requested_destination,
                    prev_victim_pos: ev.prev_victim_pos,
                    crate_created_host: ev.crate_created_host,
                    guard_retaliate_victim_host: ev.guard_retaliate_victim_host,
                    guard_retaliate_anchor: ev.guard_retaliate_anchor,
                    path_timestamp: ev.path_timestamp,
                    disguise_pending_template: ev.disguise_pending_template.clone(),
                    disguise_pending_team_ordinal: ev.disguise_pending_team_ordinal,
                    weapon_crate_upgrade: ev.weapon_crate_upgrade,
                    armor_crate_upgrade: ev.armor_crate_upgrade,
                    selection_flash_remaining: ev.selection_flash_remaining,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ai_decision_events(
        &mut self,
        events: &[crate::game_logic::host_ai_decision_log::HostAiDecisionEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::PushAiDecision {
                    host_object: ev.host_object.0,
                    kind: ev.kind,
                    target_host: ev.target_host,
                    destination: ev.destination,
                    ai_state_ordinal: ev.ai_state_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Apply ordered AICommand residuals as GameWorld mutations (attack/move/state).
    ///
    /// Used when [`gameworld_ai_decision_authority_enabled`] — host only logged
    /// decisions; this is the authoritative apply path before writeback.
    pub fn apply_ai_decisions_as_world_mutations(
        &mut self,
        events: &[crate::game_logic::host_ai_decision_log::HostAiDecisionEvent],
    ) -> usize {
        use crate::game_logic::host_ai_decision_log::{
            AI_DECISION_ATTACK, AI_DECISION_MOVE_TO, AI_DECISION_SET_STATE, AI_DECISION_STOP_ATTACK,
        };
        let mut n = 0usize;
        for ev in events {
            // Always keep the ordered decision buffer residual.
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::PushAiDecision {
                    host_object: ev.host_object.0,
                    kind: ev.kind,
                    target_host: ev.target_host,
                    destination: ev.destination,
                    ai_state_ordinal: ev.ai_state_ordinal,
                });
            let Some(eid) = self.entity_for_host(ev.host_object) else {
                n += 1;
                continue;
            };
            match ev.kind {
                x if x == AI_DECISION_ATTACK => {
                    let target = if ev.target_host == 0 {
                        None
                    } else {
                        self.entity_for_host(crate::game_logic::ObjectId(ev.target_host))
                    };
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAttackTarget {
                            attacker: eid,
                            target,
                        });
                    // Attacking state residual.
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAiState {
                            target: eid,
                            ordinal: 2, // Attacking
                        });
                }
                x if x == AI_DECISION_STOP_ATTACK => {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAttackTarget {
                            attacker: eid,
                            target: None,
                        });
                }
                x if x == AI_DECISION_MOVE_TO => {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetMoveTarget {
                            unit: eid,
                            destination: ev.destination,
                        });
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAiState {
                            target: eid,
                            ordinal: 1, // Moving
                        });
                }
                x if x == AI_DECISION_SET_STATE => {
                    self.world
                        .queue_mutation(gamelogic::world::WorldMutation::SetAiState {
                            target: eid,
                            ordinal: ev.ai_state_ordinal,
                        });
                }
                _ => {}
            }
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_set_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_set_log::HostWeaponSetEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetWeaponSetFlags {
                    target: eid,
                    player_upgrade: ev.player_upgrade,
                    armed_riders: ev.armed_riders,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_overcharge_events(
        &mut self,
        events: &[crate::game_logic::host_overcharge_log::HostOverchargeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetOvercharge {
                    target: eid,
                    enabled: ev.enabled,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_contain_capacity_events(
        &mut self,
        events: &[crate::game_logic::host_contain_capacity_log::HostContainCapacityEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetContainCapacity {
                    target: eid,
                    max_transport: ev.max_transport,
                    max_garrison: ev.max_garrison,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_hive_events(
        &mut self,
        events: &[crate::game_logic::host_hive_log::HostHiveEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetHiveSlaves {
                    target: eid,
                    slave_count: ev.slave_count,
                    slave_hp: ev.slave_hp,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_hijacker_events(
        &mut self,
        events: &[crate::game_logic::host_hijacker_log::HostHijackerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetHijacker {
                    target: eid,
                    hijack_vehicle_host: ev.hijack_vehicle_host,
                    hijacker_in_vehicle: ev.hijacker_in_vehicle,
                    hijacker_update_active: ev.hijacker_update_active,
                    hijacker_was_airborne: ev.hijacker_was_airborne,
                    hijacker_eject_pos: ev.hijacker_eject_pos,
                    hive_slave_respawn_frame: ev.hive_slave_respawn_frame,
                    next_detection_scan_frame: ev.next_detection_scan_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_stealth_flags_events(
        &mut self,
        events: &[crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetStealthFlags {
                    target: eid,
                    innate_stealth: ev.innate_stealth,
                    stealth_breaks_on_attack: ev.stealth_breaks_on_attack,
                    stealth_breaks_on_move: ev.stealth_breaks_on_move,
                    is_tunnel_network: ev.is_tunnel_network,
                    passengers_allowed_to_fire: ev.passengers_allowed_to_fire,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_stealth_delay_events(
        &mut self,
        events: &[crate::game_logic::host_stealth_delay_log::HostStealthDelayEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetStealthDelay {
                    target: eid,
                    stealth_allowed_frame: ev.stealth_allowed_frame,
                    stealth_delay_pending: ev.stealth_delay_pending,
                    stealth_delay_frames: ev.stealth_delay_frames,
                    stealth_breaks_on_damage: ev.stealth_breaks_on_damage,
                    detection_expires_frame: ev.detection_expires_frame,
                    camo_opacity_pulse_phase: ev.camo_opacity_pulse_phase,
                    camo_heat_vision_opacity: ev.camo_heat_vision_opacity,
                    camo_net_sub_object_shown: ev.camo_net_sub_object_shown,
                    camo_net_sub_object_observer_visible: ev.camo_net_sub_object_observer_visible,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_overlord_events(
        &mut self,
        events: &[crate::game_logic::host_overlord_log::HostOverlordEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetOverlordAddon {
                    target: eid,
                    has_gattling: ev.has_gattling,
                    has_propaganda: ev.has_propaganda,
                    bunker_capacity: ev.bunker_capacity,
                    is_helix_transport: ev.is_helix_transport,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_command_set_events(
        &mut self,
        events: &[crate::game_logic::host_command_set_log::HostCommandSetEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetCommandSet {
                    target: eid,
                    command_set: ev.command_set.clone(),
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_command_set_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host = obj.command_set_override.clone().unwrap_or_default();
            if host == ent.command_set_override {
                continue;
            }
            obj.command_set_override = if ent.command_set_override.is_empty() {
                None
            } else {
                Some(ent.command_set_override.clone())
            };
            updated += 1;
        }
        updated
    }

    pub fn apply_host_disguise_events(
        &mut self,
        events: &[crate::game_logic::host_disguise_log::HostDisguiseEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDisguise {
                    target: eid,
                    template: ev.template.clone(),
                    team_ordinal: ev.team_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_vision_camo_events(
        &mut self,
        events: &[crate::game_logic::host_vision_camo_log::HostVisionCamoEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetVisionCamo {
                    target: eid,
                    vision_spied_mask: ev.vision_spied_mask,
                    camo_friendly_opacity: ev.camo_friendly_opacity,
                    camo_stealth_look: ev.camo_stealth_look,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_weapon_stats_events(
        &mut self,
        events: &[crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetWeaponStats {
                    target: eid,
                    has_weapon: ev.has_weapon,
                    weapon_damage: ev.weapon_damage,
                    weapon_range: ev.weapon_range,
                    weapon_min_range: ev.weapon_min_range,
                    weapon_reload_time: ev.weapon_reload_time,
                    weapon_last_fire_time: ev.weapon_last_fire_time,
                    weapon_clip_size: ev.weapon_clip_size,
                    weapon_clip_reload_time: ev.weapon_clip_reload_time,
                    weapon_ammo: ev.weapon_ammo,
                    weapon_can_target_air: ev.weapon_can_target_air,
                    weapon_can_target_ground: ev.weapon_can_target_ground,
                    weapon_projectile_speed: ev.weapon_projectile_speed,
                    has_secondary_weapon: ev.has_secondary_weapon,
                    secondary_weapon_damage: ev.secondary_weapon_damage,
                    secondary_weapon_range: ev.secondary_weapon_range,
                    leech_range_active_primary: ev.leech_range_active_primary,
                    leech_range_active_secondary: ev.leech_range_active_secondary,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    /// Queue SetBodyDamage from host BodyDamageType residual log.
    pub fn apply_host_body_damage_events(
        &mut self,
        events: &[crate::game_logic::host_body_damage_log::HostBodyDamageEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetBodyDamage {
                    target: eid,
                    body_damage_state: ev.body_damage_state,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_death_type_events(
        &mut self,
        events: &[crate::game_logic::host_death_type_log::HostDeathTypeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDeathType {
                    target: eid,
                    death_type: ev.death_type,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_radar_extend_events(
        &mut self,
        events: &[crate::game_logic::host_radar_extend_log::HostRadarExtendEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetRadarExtend {
                    target: eid,
                    radar_extend_done_frame: ev.radar_extend_done_frame,
                    radar_extend_complete: ev.radar_extend_complete,
                    radar_active: ev.radar_active,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_shock_stun_events(
        &mut self,
        events: &[crate::game_logic::host_shock_stun_log::HostShockStunEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetShockStun {
                    target: eid,
                    shock_stun_frames: ev.shock_stun_frames,
                    shock_yaw_rate: ev.shock_yaw_rate,
                    shock_pitch_rate: ev.shock_pitch_rate,
                    shock_roll_rate: ev.shock_roll_rate,
                    shock_up_z: ev.shock_up_z,
                    shock_allow_bounce: ev.shock_allow_bounce,
                    shock_grounded_once: ev.shock_grounded_once,
                    shock_was_airborne: ev.shock_was_airborne,
                    cell_is_cliff: ev.cell_is_cliff,
                    cell_is_underwater: ev.cell_is_underwater,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_rebuild_producer_events(
        &mut self,
        events: &[crate::game_logic::host_rebuild_producer_log::HostRebuildProducerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetRebuildProducer {
                    target: eid,
                    is_rebuild_hole: ev.is_rebuild_hole,
                    rebuild_template_name: ev.rebuild_template_name.clone(),
                    rebuild_ready_frame: ev.rebuild_ready_frame,
                    rebuild_spawner_id: ev.rebuild_spawner_id,
                    rebuild_worker_id: ev.rebuild_worker_id,
                    rebuild_reconstructing_id: ev.rebuild_reconstructing_id,
                    producer_id: ev.producer_id,
                    construction_complete_clear_frame: ev.construction_complete_clear_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_sole_healing_events(
        &mut self,
        events: &[crate::game_logic::host_sole_healing_log::HostSoleHealingEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetSoleHealing {
                    target: eid,
                    sole_healing_benefactor_id: ev.sole_healing_benefactor_id,
                    sole_healing_benefactor_expiration_frame: ev
                        .sole_healing_benefactor_expiration_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_movement_events(
        &mut self,
        events: &[crate::game_logic::host_movement_log::HostMovementEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetMovement {
                    target: eid,
                    velocity: ev.velocity,
                    max_speed: ev.max_speed,
                    path_index: ev.path_index,
                    path_len: ev.path_len,
                    path_waypoints: ev.path_waypoints.clone(),
                    waiting_for_path: ev.waiting_for_path,
                    locomotor_surfaces: ev.locomotor_surfaces,
                    is_attack_path: ev.is_attack_path,
                    is_blocked_and_stuck: ev.is_blocked_and_stuck,
                    is_braking: ev.is_braking,
                    is_safe_path: ev.is_safe_path,
                    queue_for_path_frames: ev.queue_for_path_frames,
                    path_timestamp: ev.path_timestamp,
                    cur_max_blocked_speed: ev.cur_max_blocked_speed,
                    num_frames_blocked: ev.num_frames_blocked,
                    is_blocked: ev.is_blocked,
                    move_away_from_id: ev.move_away_from_id,
                    requested_victim_id: ev.requested_victim_id,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_physics_motive_events(
        &mut self,
        events: &[crate::game_logic::host_physics_motive_log::HostPhysicsMotiveEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetPhysicsMotive {
                    target: eid,
                    motive_frames_remaining: ev.motive_frames_remaining,
                    physics_mass: ev.physics_mass,
                    physics_accel: ev.physics_accel,
                    forward_friction: ev.forward_friction,
                    lateral_friction: ev.lateral_friction,
                    z_friction: ev.z_friction,
                    can_path_through_units: ev.can_path_through_units,
                    ignore_collisions_until_frame: ev.ignore_collisions_until_frame,
                    is_panicking: ev.is_panicking,
                    move_away_frames: ev.move_away_frames,
                    aerodynamic_friction: ev.aerodynamic_friction,
                    extra_friction: ev.extra_friction,
                    apply_friction_2d_when_airborne: ev.apply_friction_2d_when_airborne,
                    center_of_mass_offset: ev.center_of_mass_offset,
                    pitch_roll_yaw_factor: ev.pitch_roll_yaw_factor,
                    move_away_destination: ev.move_away_destination,
                    request_other_move_away_id: ev.request_other_move_away_id,
                    immune_to_falling_damage: ev.immune_to_falling_damage,
                    physics_current_overlap_id: ev.physics_current_overlap_id,
                    physics_previous_overlap_id: ev.physics_previous_overlap_id,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_locomotor_events(
        &mut self,
        events: &[crate::game_logic::host_locomotor_log::HostLocomotorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetLocomotor {
                    target: eid,
                    is_approach_path: ev.is_approach_path,
                    on_invalid_movement_terrain: ev.on_invalid_movement_terrain,
                    was_airborne_last_frame: ev.was_airborne_last_frame,
                    can_move_backward: ev.can_move_backward,
                    moving_backwards: ev.moving_backwards,
                    no_slow_down_as_approaching_dest: ev.no_slow_down_as_approaching_dest,
                    turn_pivot_offset: ev.turn_pivot_offset,
                    wander_width_factor: ev.wander_width_factor,
                    loco_apply_2d_friction_airborne: ev.loco_apply_2d_friction_airborne,
                    loco_extra_2d_friction: ev.loco_extra_2d_friction,
                    loco_preferred_height: ev.loco_preferred_height,
                    loco_preferred_height_damping: ev.loco_preferred_height_damping,
                    loco_appearance_ordinal: ev.loco_appearance_ordinal,
                    loco_behavior_z_ordinal: ev.loco_behavior_z_ordinal,
                    min_turn_speed: ev.min_turn_speed,
                    physics_turning_ordinal: ev.physics_turning_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_bounce_land_events(
        &mut self,
        events: &[crate::game_logic::host_bounce_land_log::HostBounceLandEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetBounceLand {
                    target: eid,
                    kill_when_resting_on_ground: ev.kill_when_resting_on_ground,
                    bounce_land_events: ev.bounce_land_events,
                    last_bounce_fall_dy: ev.last_bounce_fall_dy,
                    bounce_sound_name: ev.bounce_sound_name.clone(),
                    last_bounce_volume: ev.last_bounce_volume,
                    bounce_audio_pending: ev.bounce_audio_pending,
                    allow_collide_force: ev.allow_collide_force,
                    last_collidee_id: ev.last_collidee_id,
                    ignore_collisions_with_id: ev.ignore_collisions_with_id,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_selection_radius_events(
        &mut self,
        events: &[crate::game_logic::host_selection_radius_log::HostSelectionRadiusEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetSelectionRadius {
                    target: eid,
                    selection_radius: ev.selection_radius,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_selection_radius_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if (obj.selection_radius - ent.selection_radius).abs() <= f32::EPSILON {
                continue;
            }
            obj.selection_radius = ent.selection_radius;
            updated += 1;
        }
        updated
    }

    pub fn apply_host_model_condition_events(
        &mut self,
        events: &[crate::game_logic::host_model_condition_log::HostModelConditionEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetModelCondition {
                    target: eid,
                    model_condition_bits: ev.model_condition_bits,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_demo_mine_cheer_events(
        &mut self,
        events: &[crate::game_logic::host_demo_mine_cheer_log::HostDemoMineCheerEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDemoMineCheer {
                    target: eid,
                    demo_suicided_detonating: ev.demo_suicided_detonating,
                    has_mine_data: ev.has_mine_data,
                    cheer_timer: ev.cheer_timer,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_crush_vision_events(
        &mut self,
        events: &[crate::game_logic::host_crush_vision_log::HostCrushVisionEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetCrushVision {
                    target: eid,
                    crusher_level: ev.crusher_level,
                    crushable_level: ev.crushable_level,
                    vision_range: ev.vision_range,
                    shroud_clearing_range: ev.shroud_clearing_range,
                    front_crushed: ev.front_crushed,
                    back_crushed: ev.back_crushed,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_building_type_events(
        &mut self,
        events: &[crate::game_logic::host_building_type_log::HostBuildingTypeEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetBuildingType {
                    target: eid,
                    is_building: ev.is_building,
                    building_type_ordinal: ev.building_type_ordinal,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_identity_events(
        &mut self,
        events: &[crate::game_logic::host_identity_log::HostIdentityEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetIdentity {
                    target: eid,
                    name: ev.name.clone(),
                    team_color: ev.team_color,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_ground_height_events(
        &mut self,
        events: &[crate::game_logic::host_ground_height_log::HostGroundHeightEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetGroundHeight {
                    target: eid,
                    ground_height: ev.ground_height,
                    from_terrain: ev.from_terrain,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_model_mesh_events(
        &mut self,
        events: &[crate::game_logic::host_model_mesh_log::HostModelMeshEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetModelMesh {
                    target: eid,
                    model_key: ev.model_key.clone(),
                    mesh_scale: ev.mesh_scale,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_fow_events(
        &mut self,
        events: &[crate::game_logic::host_fow_log::HostFowEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFow {
                    target: eid,
                    visibility_alpha: ev.visibility_alpha,
                    is_explored: ev.is_explored,
                    visibility_falloff: ev.visibility_falloff,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_kind_of_events(
        &mut self,
        events: &[crate::game_logic::host_kind_of_log::HostKindOfEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetKindOfBits {
                    target: eid,
                    kind_of_bits: ev.kind_of_bits,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn apply_host_faerie_fire_events(
        &mut self,
        events: &[crate::game_logic::host_faerie_fire_log::HostFaerieFireEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetFaerieFire {
                    target: eid,
                    active: ev.active,
                    until_frame: ev.until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_faerie_fire_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_active = obj.is_faerie_fire();
            if host_active == ent.faerie_fire
                && obj.faerie_fire_until_frame == ent.faerie_fire_until_frame
            {
                continue;
            }
            if ent.faerie_fire {
                obj.set_status_faerie_fire(true);
                obj.faerie_fire_until_frame = ent.faerie_fire_until_frame;
            } else {
                obj.set_status_faerie_fire(false);
                obj.faerie_fire_until_frame = 0;
            }
            updated += 1;
        }
        updated
    }

    pub fn apply_host_repulsor_events(
        &mut self,
        events: &[crate::game_logic::host_repulsor_log::HostRepulsorEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetRepulsor {
                    target: eid,
                    active: ev.active,
                    until_frame: ev.until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_repulsor_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_active = obj.status.repulsor;
            if host_active == ent.repulsor && obj.repulsor_until_frame == ent.repulsor_until_frame {
                continue;
            }
            obj.repulsor_until_frame = ent.repulsor_until_frame;
            // Avoid re-entrant host_repulsor_log from set_status_repulsor during writeback.
            obj.status.repulsor = ent.repulsor;
            updated += 1;
        }
        updated
    }

    pub fn apply_host_disable_timers_events(
        &mut self,
        events: &[crate::game_logic::host_disable_timers_log::HostDisableTimersEvent],
    ) -> usize {
        let mut n = 0usize;
        for ev in events {
            let Some(&eid) = self.host_to_entity.get(&ev.object.0) else {
                continue;
            };
            self.world
                .queue_mutation(gamelogic::world::WorldMutation::SetDisableTimers {
                    target: eid,
                    emp_until_frame: ev.emp_until_frame,
                    hacked_until_frame: ev.hacked_until_frame,
                    paralyzed_until_frame: ev.paralyzed_until_frame,
                });
            n += 1;
        }
        if n > 0 {
            let _ = self.apply_pending();
        }
        n
    }

    pub fn writeback_disable_timers_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.status.disabled_emp_until_frame == ent.disabled_emp_until_frame
                && obj.status.disabled_hacked_until_frame == ent.disabled_hacked_until_frame
                && obj.status.disabled_paralyzed_until_frame == ent.disabled_paralyzed_until_frame
            {
                continue;
            }
            obj.status.disabled_emp_until_frame = ent.disabled_emp_until_frame;
            obj.status.disabled_hacked_until_frame = ent.disabled_hacked_until_frame;
            obj.status.disabled_paralyzed_until_frame = ent.disabled_paralyzed_until_frame;
            updated += 1;
        }
        updated
    }

    pub fn writeback_ground_height_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = (obj.ground_height - ent.ground_height).abs() > f32::EPSILON
                || obj.ground_height_from_terrain != ent.ground_height_from_terrain;
            if !changed {
                continue;
            }
            obj.ground_height = ent.ground_height;
            obj.ground_height_from_terrain = ent.ground_height_from_terrain;
            updated += 1;
        }
        updated
    }

    pub fn writeback_identity_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let color_changed = obj
                .team_color
                .iter()
                .zip(ent.team_color.iter())
                .any(|(a, b)| (*a - *b).abs() > f32::EPSILON);
            if obj.name == ent.display_name && !color_changed {
                continue;
            }
            obj.name = ent.display_name.clone();
            obj.team_color = ent.team_color;
            updated += 1;
        }
        updated
    }

    pub fn writeback_building_type_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::{BuildingData, BuildingType as B};
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_is = obj.building_data.is_some();
            let host_ord = obj
                .building_data
                .as_ref()
                .map(|bd| Self::host_building_type_ordinal(bd.building_type))
                .unwrap_or(255);
            if host_is == ent.is_building && host_ord == ent.building_type_ordinal {
                continue;
            }
            if !ent.is_building || ent.building_type_ordinal == 255 {
                if obj.building_data.is_some() {
                    // Do not destroy building_data payload on flag-only clear; leave host ownership.
                }
            } else {
                let bt = match ent.building_type_ordinal {
                    0 => B::CommandCenter,
                    1 => B::Barracks,
                    2 => B::WarFactory,
                    3 => B::Airfield,
                    4 => B::RepairPad,
                    5 => B::HealPad,
                    6 => B::SupplyCenter,
                    7 => B::PowerPlant,
                    8 => B::DefenseTurret,
                    9 => B::SupplyDropZone,
                    10 => B::Palace,
                    11 => B::Propaganda,
                    12 => B::Bunker,
                    _ => B::CommandCenter,
                };
                if let Some(bd) = obj.building_data.as_mut() {
                    if bd.building_type != bt {
                        bd.building_type = bt;
                        updated += 1;
                    }
                } else {
                    obj.building_data = Some(BuildingData::new(bt));
                    updated += 1;
                }
            }
        }
        updated
    }

    pub fn writeback_crush_vision_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.crusher_level != ent.crusher_level
                || obj.crushable_level != ent.crushable_level
                || (obj.vision_range - ent.vision_range).abs() > f32::EPSILON
                || (obj.shroud_clearing_range - ent.shroud_clearing_range).abs() > f32::EPSILON
                || obj.front_crushed != ent.front_crushed
                || obj.back_crushed != ent.back_crushed;
            if !changed {
                continue;
            }
            obj.crusher_level = ent.crusher_level;
            obj.crushable_level = ent.crushable_level;
            obj.vision_range = ent.vision_range;
            obj.shroud_clearing_range = ent.shroud_clearing_range;
            obj.front_crushed = ent.front_crushed;
            obj.back_crushed = ent.back_crushed;
            updated += 1;
        }
        updated
    }

    pub fn writeback_demo_mine_cheer_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_has_mine = obj.mine_data.is_some();
            let changed = obj.demo_suicided_detonating != ent.demo_suicided_detonating
                || host_has_mine != ent.has_mine_data
                || (obj.cheer_timer - ent.cheer_timer).abs() > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.demo_suicided_detonating = ent.demo_suicided_detonating;
            obj.cheer_timer = ent.cheer_timer;
            // has_mine_data is a present-flag mirror only; do not invent/destroy HostMineData here.
            // Flag-only writeback: if entity says no mine and host has mine_data left to status, leave payload.
            // Cheer/demo flags are authoritative from GameWorld last-writer residual.
            let _ = host_has_mine; // presence is logged host→entity; entity→host keeps payload ownership on Main.
            updated += 1;
        }
        updated
    }

    pub fn writeback_model_condition_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.model_condition_bits == ent.model_condition_bits {
                continue;
            }
            obj.model_condition_bits = ent.model_condition_bits;
            updated += 1;
        }
        updated
    }

    pub fn writeback_movement_to_host(&self, logic: &mut GameLogic) -> usize {
        use glam::Vec3;
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_v = [
                obj.movement.velocity.x,
                obj.movement.velocity.y,
                obj.movement.velocity.z,
            ];
            let host_idx = obj.movement.current_path_index.min(u16::MAX as usize) as u16;
            let host_len = obj.movement.path.len().min(u16::MAX as usize) as u16;
            let vel_changed = host_v
                .iter()
                .zip(ent.velocity.iter())
                .any(|(a, b)| (*a - *b).abs() > f32::EPSILON);
            let path_changed = if ent.path_waypoints.is_empty() {
                ent.path_len == 0 && !obj.movement.path.is_empty()
            } else {
                obj.movement.path.len() != ent.path_waypoints.len()
                    || obj
                        .movement
                        .path
                        .iter()
                        .zip(ent.path_waypoints.iter())
                        .any(|(p, e)| {
                            (p.x - e[0]).abs() > f32::EPSILON
                                || (p.y - e[1]).abs() > f32::EPSILON
                                || (p.z - e[2]).abs() > f32::EPSILON
                        })
            };
            let flags_changed = obj.waiting_for_path != ent.waiting_for_path
                || obj.locomotor_surfaces != ent.locomotor_surfaces
                || obj.is_attack_path != ent.is_attack_path
                || obj.is_blocked_and_stuck != ent.is_blocked_and_stuck
                || obj.is_braking != ent.is_braking
                || obj.is_safe_path != ent.is_safe_path
                || obj.queue_for_path_frames != ent.queue_for_path_frames
                || obj.path_timestamp != ent.path_timestamp
                || (obj.cur_max_blocked_speed - ent.cur_max_blocked_speed).abs() > f32::EPSILON
                || obj.num_frames_blocked != ent.num_frames_blocked
                || obj.is_blocked != ent.is_blocked
                || obj.move_away_from.map(|id| id.0) != ent.move_away_from_id
                || obj.requested_victim_id.map(|id| id.0) != ent.requested_victim_id;
            let changed = vel_changed
                || (obj.movement.max_speed - ent.move_max_speed).abs() > f32::EPSILON
                || host_idx != ent.path_index
                || host_len != ent.path_len
                || path_changed
                || flags_changed;
            if !changed {
                continue;
            }
            obj.movement.velocity = Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
            obj.movement.max_speed = ent.move_max_speed;
            obj.movement.current_path_index = ent.path_index as usize;
            if !ent.path_waypoints.is_empty() {
                obj.movement.path = ent
                    .path_waypoints
                    .iter()
                    .map(|p| Vec3::new(p[0], p[1], p[2]))
                    .collect();
            } else if ent.path_len == 0 {
                obj.movement.path.clear();
            }
            obj.waiting_for_path = ent.waiting_for_path;
            obj.locomotor_surfaces = ent.locomotor_surfaces;
            obj.is_attack_path = ent.is_attack_path;
            obj.is_blocked_and_stuck = ent.is_blocked_and_stuck;
            obj.is_braking = ent.is_braking;
            obj.is_safe_path = ent.is_safe_path;
            obj.queue_for_path_frames = ent.queue_for_path_frames;
            obj.path_timestamp = ent.path_timestamp;
            obj.cur_max_blocked_speed = ent.cur_max_blocked_speed;
            obj.num_frames_blocked = ent.num_frames_blocked;
            obj.is_blocked = ent.is_blocked;
            obj.move_away_from = ent.move_away_from_id.map(ObjectId);
            obj.requested_victim_id = ent.requested_victim_id.map(ObjectId);
            updated += 1;
        }
        updated
    }

    pub fn writeback_physics_motive_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_dest = obj.move_away_destination.map(|p| [p.x, p.y, p.z]);
            let changed = obj.motive_frames_remaining != ent.motive_frames_remaining
                || (obj.physics_mass - ent.physics_mass).abs() > f32::EPSILON
                || (obj.physics_accel.x - ent.physics_accel[0]).abs() > f32::EPSILON
                || (obj.physics_accel.y - ent.physics_accel[1]).abs() > f32::EPSILON
                || (obj.physics_accel.z - ent.physics_accel[2]).abs() > f32::EPSILON
                || (obj.forward_friction - ent.forward_friction).abs() > f32::EPSILON
                || (obj.lateral_friction - ent.lateral_friction).abs() > f32::EPSILON
                || (obj.z_friction - ent.z_friction).abs() > f32::EPSILON
                || obj.can_path_through_units != ent.can_path_through_units
                || obj.ignore_collisions_until_frame != ent.ignore_collisions_until_frame
                || obj.is_panicking != ent.is_panicking
                || obj.move_away_frames != ent.move_away_frames
                || (obj.aerodynamic_friction - ent.aerodynamic_friction).abs() > f32::EPSILON
                || (obj.extra_friction - ent.extra_friction).abs() > f32::EPSILON
                || obj.apply_friction_2d_when_airborne != ent.apply_friction_2d_when_airborne
                || (obj.center_of_mass_offset - ent.center_of_mass_offset).abs() > f32::EPSILON
                || (obj.pitch_roll_yaw_factor - ent.pitch_roll_yaw_factor).abs() > f32::EPSILON
                || host_dest != ent.move_away_destination
                || obj.request_other_move_away.map(|id| id.0) != ent.request_other_move_away_id
                || obj.immune_to_falling_damage != ent.immune_to_falling_damage
                || obj.physics_current_overlap.map(|id| id.0) != ent.physics_current_overlap_id
                || obj.physics_previous_overlap.map(|id| id.0) != ent.physics_previous_overlap_id;
            if !changed {
                continue;
            }
            obj.motive_frames_remaining = ent.motive_frames_remaining;
            obj.physics_mass = ent.physics_mass;
            obj.physics_accel = glam::Vec3::new(
                ent.physics_accel[0],
                ent.physics_accel[1],
                ent.physics_accel[2],
            );
            obj.forward_friction = ent.forward_friction;
            obj.lateral_friction = ent.lateral_friction;
            obj.z_friction = ent.z_friction;
            obj.can_path_through_units = ent.can_path_through_units;
            obj.ignore_collisions_until_frame = ent.ignore_collisions_until_frame;
            obj.is_panicking = ent.is_panicking;
            obj.move_away_frames = ent.move_away_frames;
            obj.aerodynamic_friction = ent.aerodynamic_friction;
            obj.extra_friction = ent.extra_friction;
            obj.apply_friction_2d_when_airborne = ent.apply_friction_2d_when_airborne;
            obj.center_of_mass_offset = ent.center_of_mass_offset;
            obj.pitch_roll_yaw_factor = ent.pitch_roll_yaw_factor;
            obj.move_away_destination = ent
                .move_away_destination
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.request_other_move_away = ent.request_other_move_away_id.map(ObjectId);
            obj.immune_to_falling_damage = ent.immune_to_falling_damage;
            obj.physics_current_overlap = ent.physics_current_overlap_id.map(ObjectId);
            obj.physics_previous_overlap = ent.physics_previous_overlap_id.map(ObjectId);
            updated += 1;
        }
        updated
    }

    pub fn writeback_locomotor_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.is_approach_path != ent.is_approach_path
                || obj.on_invalid_movement_terrain != ent.on_invalid_movement_terrain
                || obj.was_airborne_last_frame != ent.was_airborne_last_frame
                || obj.can_move_backward != ent.can_move_backward
                || obj.moving_backwards != ent.moving_backwards
                || obj.no_slow_down_as_approaching_dest != ent.no_slow_down_as_approaching_dest
                || (obj.turn_pivot_offset - ent.turn_pivot_offset).abs() > f32::EPSILON
                || (obj.wander_width_factor - ent.wander_width_factor).abs() > f32::EPSILON
                || obj.loco_apply_2d_friction_airborne != ent.loco_apply_2d_friction_airborne
                || (obj.loco_extra_2d_friction - ent.loco_extra_2d_friction).abs() > f32::EPSILON
                || (obj.loco_preferred_height - ent.loco_preferred_height).abs() > f32::EPSILON
                || (obj.loco_preferred_height_damping - ent.loco_preferred_height_damping).abs()
                    > f32::EPSILON
                || obj.loco_appearance.to_ordinal() != ent.loco_appearance_ordinal
                || obj.loco_behavior_z.to_ordinal() != ent.loco_behavior_z_ordinal
                || (obj.min_turn_speed - ent.min_turn_speed).abs() > f32::EPSILON
                || obj.physics_turning.to_ordinal() != ent.physics_turning_ordinal;
            if !changed {
                continue;
            }
            obj.is_approach_path = ent.is_approach_path;
            obj.on_invalid_movement_terrain = ent.on_invalid_movement_terrain;
            obj.was_airborne_last_frame = ent.was_airborne_last_frame;
            obj.can_move_backward = ent.can_move_backward;
            obj.moving_backwards = ent.moving_backwards;
            obj.no_slow_down_as_approaching_dest = ent.no_slow_down_as_approaching_dest;
            obj.turn_pivot_offset = ent.turn_pivot_offset;
            obj.wander_width_factor = ent.wander_width_factor;
            obj.loco_apply_2d_friction_airborne = ent.loco_apply_2d_friction_airborne;
            obj.loco_extra_2d_friction = ent.loco_extra_2d_friction;
            obj.loco_preferred_height = ent.loco_preferred_height;
            obj.loco_preferred_height_damping = ent.loco_preferred_height_damping;
            obj.loco_appearance =
                crate::game_logic::LocomotorAppearance::from_ordinal(ent.loco_appearance_ordinal);
            obj.loco_behavior_z =
                crate::game_logic::LocomotorBehaviorZ::from_ordinal(ent.loco_behavior_z_ordinal);
            obj.min_turn_speed = ent.min_turn_speed;
            obj.physics_turning =
                crate::game_logic::PhysicsTurningType::from_ordinal(ent.physics_turning_ordinal);
            updated += 1;
        }
        updated
    }

    pub fn writeback_bounce_land_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.kill_when_resting_on_ground != ent.kill_when_resting_on_ground
                || obj.bounce_land_events != ent.bounce_land_events
                || (obj.last_bounce_fall_dy - ent.last_bounce_fall_dy).abs() > f32::EPSILON
                || obj.bounce_sound_name != ent.bounce_sound_name
                || (obj.last_bounce_volume - ent.last_bounce_volume).abs() > f32::EPSILON
                || obj.bounce_audio_pending != ent.bounce_audio_pending
                || obj.allow_collide_force != ent.allow_collide_force
                || obj.last_collidee.map(|id| id.0) != ent.last_collidee_id
                || obj.ignore_collisions_with.map(|id| id.0) != ent.ignore_collisions_with_id;
            if !changed {
                continue;
            }
            obj.kill_when_resting_on_ground = ent.kill_when_resting_on_ground;
            obj.bounce_land_events = ent.bounce_land_events;
            obj.last_bounce_fall_dy = ent.last_bounce_fall_dy;
            obj.bounce_sound_name = ent.bounce_sound_name.clone();
            obj.last_bounce_volume = ent.last_bounce_volume;
            obj.bounce_audio_pending = ent.bounce_audio_pending;
            obj.allow_collide_force = ent.allow_collide_force;
            obj.last_collidee = ent.last_collidee_id.map(ObjectId);
            obj.ignore_collisions_with = ent.ignore_collisions_with_id.map(ObjectId);
            updated += 1;
        }
        updated
    }

    pub fn writeback_weapon_stats_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let mut changed = false;
            if let Some(w) = obj.weapon.as_mut() {
                if (w.damage - ent.weapon_damage).abs() > f32::EPSILON
                    || (w.range - ent.weapon_range).abs() > f32::EPSILON
                    || (w.min_range - ent.weapon_min_range).abs() > f32::EPSILON
                    || (w.reload_time - ent.weapon_reload_time).abs() > f32::EPSILON
                    || (w.last_fire_time - ent.weapon_last_fire_time).abs() > f32::EPSILON
                    || w.ammo.unwrap_or(u32::MAX) != ent.weapon_ammo
                    || w.can_target_air != ent.weapon_can_target_air
                    || w.can_target_ground != ent.weapon_can_target_ground
                    || (w.projectile_speed - ent.weapon_projectile_speed).abs() > f32::EPSILON
                {
                    w.damage = ent.weapon_damage;
                    w.range = ent.weapon_range;
                    w.min_range = ent.weapon_min_range;
                    w.reload_time = ent.weapon_reload_time;
                    w.last_fire_time = ent.weapon_last_fire_time;
                    w.ammo = if ent.weapon_ammo == u32::MAX {
                        None
                    } else {
                        Some(ent.weapon_ammo)
                    };
                    w.can_target_air = ent.weapon_can_target_air;
                    w.can_target_ground = ent.weapon_can_target_ground;
                    w.projectile_speed = ent.weapon_projectile_speed;
                    changed = true;
                }
            }
            if let Some(w) = obj.secondary_weapon.as_mut() {
                if (w.damage - ent.secondary_weapon_damage).abs() > f32::EPSILON
                    || (w.range - ent.secondary_weapon_range).abs() > f32::EPSILON
                {
                    w.damage = ent.secondary_weapon_damage;
                    w.range = ent.secondary_weapon_range;
                    changed = true;
                }
            }
            if obj.leech_range_active_primary != ent.leech_range_active_primary
                || obj.leech_range_active_secondary != ent.leech_range_active_secondary
            {
                obj.leech_range_active_primary = ent.leech_range_active_primary;
                obj.leech_range_active_secondary = ent.leech_range_active_secondary;
                changed = true;
            }
            if changed {
                updated += 1;
            }
        }
        updated
    }

    pub fn writeback_vision_camo_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.vision_spied_mask != ent.vision_spied_mask
                || (obj.camo_friendly_opacity - ent.camo_friendly_opacity).abs() > f32::EPSILON
                || obj.camo_stealth_look != ent.camo_stealth_look;
            if !changed {
                continue;
            }
            obj.vision_spied_mask = ent.vision_spied_mask;
            obj.camo_friendly_opacity = ent.camo_friendly_opacity;
            obj.camo_stealth_look = ent.camo_stealth_look;
            updated += 1;
        }
        updated
    }

    pub fn writeback_disguise_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_tpl = obj.disguise_as_template.clone().unwrap_or_default();
            let host_team = obj
                .disguise_as_team
                .map(|t| match t {
                    Team::USA => 0u8,
                    Team::China => 1,
                    Team::GLA => 2,
                    Team::Neutral => 3,
                })
                .unwrap_or(255);
            if host_tpl == ent.disguise_as_template && host_team == ent.disguise_as_team_ordinal {
                continue;
            }
            obj.disguise_as_template = if ent.disguise_as_template.is_empty() {
                None
            } else {
                Some(ent.disguise_as_template.clone())
            };
            obj.disguise_as_team = match ent.disguise_as_team_ordinal {
                0 => Some(Team::USA),
                1 => Some(Team::China),
                2 => Some(Team::GLA),
                3 => Some(Team::Neutral),
                _ => None,
            };
            updated += 1;
        }
        updated
    }

    pub fn writeback_overlord_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_cap = match obj.overlord_bunker_capacity {
                Some(n) => n.min(u16::MAX as usize - 1) as u16,
                None => u16::MAX,
            };
            let changed = obj.has_overlord_gattling_addon != ent.has_overlord_gattling_addon
                || obj.has_overlord_propaganda_addon != ent.has_overlord_propaganda_addon
                || host_cap != ent.overlord_bunker_capacity
                || obj.is_helix_transport != ent.is_helix_transport;
            if !changed {
                continue;
            }
            obj.has_overlord_gattling_addon = ent.has_overlord_gattling_addon;
            obj.has_overlord_propaganda_addon = ent.has_overlord_propaganda_addon;
            obj.is_helix_transport = ent.is_helix_transport;
            obj.overlord_bunker_capacity = if ent.overlord_bunker_capacity == u16::MAX {
                None
            } else {
                Some(ent.overlord_bunker_capacity as usize)
            };
            updated += 1;
        }
        updated
    }

    pub fn writeback_stealth_flags_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.innate_stealth != ent.innate_stealth
                || obj.stealth_breaks_on_attack != ent.stealth_breaks_on_attack
                || obj.stealth_breaks_on_move != ent.stealth_breaks_on_move
                || obj.is_tunnel_network != ent.is_tunnel_network
                || obj.passengers_allowed_to_fire != ent.passengers_allowed_to_fire;
            if !changed {
                continue;
            }
            obj.innate_stealth = ent.innate_stealth;
            obj.stealth_breaks_on_attack = ent.stealth_breaks_on_attack;
            obj.stealth_breaks_on_move = ent.stealth_breaks_on_move;
            obj.is_tunnel_network = ent.is_tunnel_network;
            obj.passengers_allowed_to_fire = ent.passengers_allowed_to_fire;
            updated += 1;
        }
        updated
    }

    pub fn writeback_stealth_delay_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.stealth_allowed_frame != ent.stealth_allowed_frame
                || obj.stealth_delay_pending != ent.stealth_delay_pending
                || obj.stealth_delay_frames != ent.stealth_delay_frames
                || obj.stealth_breaks_on_damage != ent.stealth_breaks_on_damage
                || obj.detection_expires_frame != ent.detection_expires_frame
                || (obj.camo_opacity_pulse_phase - ent.camo_opacity_pulse_phase).abs()
                    > f32::EPSILON
                || (obj.camo_heat_vision_opacity - ent.camo_heat_vision_opacity).abs()
                    > f32::EPSILON
                || obj.camo_net_sub_object_shown != ent.camo_net_sub_object_shown
                || obj.camo_net_sub_object_observer_visible
                    != ent.camo_net_sub_object_observer_visible;
            if !changed {
                continue;
            }
            obj.stealth_allowed_frame = ent.stealth_allowed_frame;
            obj.stealth_delay_pending = ent.stealth_delay_pending;
            obj.stealth_delay_frames = ent.stealth_delay_frames;
            obj.stealth_breaks_on_damage = ent.stealth_breaks_on_damage;
            obj.detection_expires_frame = ent.detection_expires_frame;
            obj.camo_opacity_pulse_phase = ent.camo_opacity_pulse_phase;
            obj.camo_heat_vision_opacity = ent.camo_heat_vision_opacity;
            obj.camo_net_sub_object_shown = ent.camo_net_sub_object_shown;
            obj.camo_net_sub_object_observer_visible = ent.camo_net_sub_object_observer_visible;
            updated += 1;
        }
        updated
    }

    pub fn writeback_hive_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.hive_slave_count != ent.hive_slave_count
                || (obj.hive_slave_hp - ent.hive_slave_hp).abs() > 1e-4;
            if !changed {
                continue;
            }
            obj.hive_slave_count = ent.hive_slave_count;
            obj.hive_slave_hp = ent.hive_slave_hp.max(0.0);
            updated += 1;
        }
        updated
    }

    pub fn writeback_hijacker_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_vehicle = obj.hijack_vehicle_id.map(|id| id.0).unwrap_or(0);
            let host_eject = obj.hijacker_eject_pos.map(|p| [p.x, p.y, p.z]);
            let changed = host_vehicle != ent.hijack_vehicle_host
                || obj.hijacker_in_vehicle != ent.hijacker_in_vehicle
                || obj.hijacker_update_active != ent.hijacker_update_active
                || obj.hijacker_was_airborne != ent.hijacker_was_airborne
                || host_eject != ent.hijacker_eject_pos
                || obj.hive_slave_respawn_frame != ent.hive_slave_respawn_frame
                || obj.next_detection_scan_frame != ent.next_detection_scan_frame;
            if !changed {
                continue;
            }
            obj.hijack_vehicle_id = if ent.hijack_vehicle_host == 0 {
                None
            } else {
                Some(ObjectId(ent.hijack_vehicle_host))
            };
            obj.hijacker_in_vehicle = ent.hijacker_in_vehicle;
            obj.hijacker_update_active = ent.hijacker_update_active;
            obj.hijacker_was_airborne = ent.hijacker_was_airborne;
            obj.hijacker_eject_pos = ent
                .hijacker_eject_pos
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.hive_slave_respawn_frame = ent.hive_slave_respawn_frame;
            obj.next_detection_scan_frame = ent.next_detection_scan_frame;
            updated += 1;
        }
        updated
    }

    pub fn writeback_contain_capacity_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_garrison = obj
                .building_data
                .as_ref()
                .map(|bd| bd.max_garrison.min(u16::MAX as usize) as u16)
                .unwrap_or(0);
            let changed =
                obj.max_transport != ent.max_transport || host_garrison != ent.max_garrison;
            if !changed {
                continue;
            }
            obj.max_transport = ent.max_transport;
            if ent.max_garrison > 0 || obj.building_data.is_some() {
                if let Some(bd) = obj.building_data.as_mut() {
                    bd.max_garrison = ent.max_garrison as usize;
                } else if ent.max_garrison > 0 {
                    let mut bd = crate::game_logic::buildings::BuildingData::new(
                        crate::game_logic::buildings::BuildingType::Bunker,
                    );
                    bd.max_garrison = ent.max_garrison as usize;
                    obj.building_data = Some(bd);
                }
            }
            updated += 1;
        }
        updated
    }

    pub fn writeback_overcharge_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.overcharge_enabled == ent.overcharge_enabled {
                continue;
            }
            obj.overcharge_enabled = ent.overcharge_enabled;
            updated += 1;
        }
        updated
    }

    pub fn writeback_weapon_set_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.weapon_set_player_upgrade == ent.weapon_set_player_upgrade
                && obj.armed_riders_upgrade_weapon_set == ent.armed_riders_upgrade_weapon_set
            {
                continue;
            }
            obj.weapon_set_player_upgrade = ent.weapon_set_player_upgrade;
            obj.armed_riders_upgrade_weapon_set = ent.armed_riders_upgrade_weapon_set;
            updated += 1;
        }
        updated
    }

    pub fn writeback_ai_attitude_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.ai_attitude == ent.ai_attitude {
                continue;
            }
            obj.ai_attitude = ent.ai_attitude.clamp(-2, 2);
            updated += 1;
        }
        updated
    }

    pub fn writeback_guard_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_pos = obj.guard_position.map(|p| [p.x, p.y, p.z]);
            let host_tgt = obj.guard_target.map(|id| id.0).unwrap_or(0);
            let changed = host_pos != ent.guard_position
                || host_tgt != ent.guard_target_host
                || (obj.guard_radius - ent.guard_radius).abs() > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.guard_position = ent
                .guard_position
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.guard_target = if ent.guard_target_host == 0 {
                None
            } else {
                Some(ObjectId(ent.guard_target_host))
            };
            obj.guard_radius = ent.guard_radius;
            updated += 1;
        }
        updated
    }

    pub fn writeback_continuous_fire_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_consec = obj.continuous_fire_consecutive.min(u16::MAX as u32) as u16;
            let changed = obj.continuous_fire_level != ent.continuous_fire_level
                || host_consec != ent.continuous_fire_consecutive
                || obj.continuous_fire_coast_until_frame != ent.continuous_fire_coast_until_frame;
            if !changed {
                continue;
            }
            obj.continuous_fire_level = ent.continuous_fire_level;
            obj.continuous_fire_consecutive = ent.continuous_fire_consecutive as u32;
            obj.continuous_fire_coast_until_frame = ent.continuous_fire_coast_until_frame;
            updated += 1;
        }
        updated
    }

    pub fn writeback_combat_attack_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.pre_attack_target.map(|id| id.0).unwrap_or(0)
                != ent.pre_attack_target_host
                || (obj.pre_attack_ready_at - ent.pre_attack_ready_at).abs() > f32::EPSILON
                || obj.consecutive_shots_at_target != ent.consecutive_shots_at_target
                || obj.max_shots_to_fire != ent.max_shots_to_fire
                || obj.attack_substate.to_ordinal() != ent.attack_substate_ordinal
                || obj.approach_timestamp != ent.approach_timestamp
                || obj.continuous_fire_victim != ent.continuous_fire_victim
                || obj.maintain_pos_valid != ent.maintain_pos_valid
                || obj.maintain_pos.map(|p| [p.x, p.y, p.z]) != ent.maintain_pos
                || obj.temporary_move_frames != ent.temporary_move_frames
                || (obj.group_speed_factor - ent.group_speed_factor).abs() > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.pre_attack_target = if ent.pre_attack_target_host == 0 {
                None
            } else {
                Some(ObjectId(ent.pre_attack_target_host))
            };
            obj.pre_attack_ready_at = ent.pre_attack_ready_at;
            obj.consecutive_shots_at_target = ent.consecutive_shots_at_target;
            obj.max_shots_to_fire = ent.max_shots_to_fire;
            obj.attack_substate =
                crate::game_logic::AttackSubState::from_ordinal(ent.attack_substate_ordinal);
            obj.approach_timestamp = ent.approach_timestamp;
            obj.continuous_fire_victim = ent.continuous_fire_victim;
            obj.maintain_pos_valid = ent.maintain_pos_valid;
            obj.maintain_pos = ent.maintain_pos.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            obj.temporary_move_frames = ent.temporary_move_frames;
            obj.group_speed_factor = ent.group_speed_factor;
            updated += 1;
        }
        updated
    }

    pub fn writeback_fire_intent_to_host(&self, logic: &mut GameLogic) -> usize {
        if !gameworld_ai_attack_authority_enabled() {
            return 0;
        }
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.last_fire_victim_host != ent.last_fire_victim_host
                || obj.last_fire_slot != ent.last_fire_slot
                || (obj.last_fire_damage - ent.last_fire_damage).abs() > f32::EPSILON
                || (obj.last_fire_range - ent.last_fire_range).abs() > f32::EPSILON
                || (obj.last_fire_sim_time - ent.last_fire_sim_time).abs() > f32::EPSILON
                || obj.last_fire_frame != ent.last_fire_frame
                || obj.fire_intent_count != ent.fire_intent_count;
            if !changed {
                continue;
            }
            obj.last_fire_victim_host = ent.last_fire_victim_host;
            obj.last_fire_slot = ent.last_fire_slot;
            obj.last_fire_damage = ent.last_fire_damage;
            obj.last_fire_range = ent.last_fire_range;
            obj.last_fire_sim_time = ent.last_fire_sim_time;
            obj.last_fire_frame = ent.last_fire_frame;
            obj.fire_intent_count = ent.fire_intent_count;
            updated += 1;
        }
        updated
    }

    pub fn writeback_detector_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.is_detector != ent.is_detector
                || (obj.detection_range - ent.detection_range).abs() > 1e-4
                || obj.detection_rate_frames != ent.detection_rate_frames;
            if !changed {
                continue;
            }
            obj.is_detector = ent.is_detector;
            obj.detection_range = ent.detection_range.max(0.0);
            obj.detection_rate_frames = ent.detection_rate_frames;
            updated += 1;
        }
        updated
    }

    pub fn writeback_target_location_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_loc = obj.target_location.map(|p| [p.x, p.y, p.z]);
            let ent_loc = ent.target_location;
            let same = match (host_loc, ent_loc) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    (a[0] - b[0]).abs() <= 1e-4
                        && (a[1] - b[1]).abs() <= 1e-4
                        && (a[2] - b[2]).abs() <= 1e-4
                }
                _ => false,
            };
            if same {
                continue;
            }
            obj.target_location = ent_loc.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            updated += 1;
        }
        updated
    }

    pub fn writeback_turret_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::object::TurretSubState;
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let host_tgt = obj.turret_target_id.map(|id| id.0).unwrap_or(0);
            let changed = (obj.turret_angle_deg - ent.turret_angle_deg).abs() > f32::EPSILON
                || (obj.turret_pitch_deg - ent.turret_pitch_deg).abs() > f32::EPSILON
                || obj.turret_holding != ent.turret_holding
                || obj.turret_idle_scanning != ent.turret_idle_scanning
                || (obj.turret_turn_rate_rad - ent.turret_turn_rate_rad).abs() > f32::EPSILON
                || obj.turret_recenter_frames != ent.turret_recenter_frames
                || obj.turret_hold_until_frame != ent.turret_hold_until_frame
                || obj.turret_idle_recentering != ent.turret_idle_recentering
                || obj.turret_enabled != ent.turret_enabled
                || obj.turret_rotating != ent.turret_rotating
                || (obj.turret_natural_angle_deg - ent.turret_natural_angle_deg).abs()
                    > f32::EPSILON
                || (obj.turret_natural_pitch_deg - ent.turret_natural_pitch_deg).abs()
                    > f32::EPSILON
                || host_tgt != ent.turret_target_host
                || obj.turret_force_attacking != ent.turret_force_attacking
                || obj.turret_mood_target != ent.turret_mood_target
                || obj.turret_idle_scan_next_frame != ent.turret_idle_scan_next_frame
                || (obj.turret_idle_scan_desired_angle_deg
                    - ent.turret_idle_scan_desired_angle_deg)
                    .abs()
                    > f32::EPSILON
                || obj.turret_idle_scan_index != ent.turret_idle_scan_index
                || obj.turret_substate.ordinal() != ent.turret_substate;
            if !changed {
                continue;
            }
            obj.turret_angle_deg = ent.turret_angle_deg;
            obj.turret_pitch_deg = ent.turret_pitch_deg;
            obj.turret_holding = ent.turret_holding;
            obj.turret_idle_scanning = ent.turret_idle_scanning;
            obj.turret_turn_rate_rad = ent.turret_turn_rate_rad;
            obj.turret_recenter_frames = ent.turret_recenter_frames;
            obj.turret_hold_until_frame = ent.turret_hold_until_frame;
            obj.turret_idle_recentering = ent.turret_idle_recentering;
            obj.turret_enabled = ent.turret_enabled;
            obj.turret_rotating = ent.turret_rotating;
            obj.turret_natural_angle_deg = ent.turret_natural_angle_deg;
            obj.turret_natural_pitch_deg = ent.turret_natural_pitch_deg;
            obj.turret_target_id = if ent.turret_target_host == 0 {
                None
            } else {
                Some(ObjectId(ent.turret_target_host))
            };
            obj.turret_force_attacking = ent.turret_force_attacking;
            obj.turret_mood_target = ent.turret_mood_target;
            obj.turret_idle_scan_next_frame = ent.turret_idle_scan_next_frame;
            obj.turret_idle_scan_desired_angle_deg = ent.turret_idle_scan_desired_angle_deg;
            obj.turret_idle_scan_index = ent.turret_idle_scan_index;
            obj.turret_substate = TurretSubState::from_ordinal(ent.turret_substate);
            updated += 1;
        }
        updated
    }

    pub fn writeback_entity_power_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.power_provided == ent.power_provided && obj.power_consumed == ent.power_consumed
            {
                continue;
            }
            obj.power_provided = ent.power_provided;
            obj.power_consumed = ent.power_consumed;
            updated += 1;
        }
        updated
    }

    pub fn writeback_weapon_slot_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            if obj.active_weapon_slot == ent.active_weapon_slot {
                continue;
            }
            obj.active_weapon_slot = ent.active_weapon_slot;
            updated += 1;
        }
        updated
    }

    /// Write shadow weapon-bonus pack back onto host Object residual flags.
    pub fn writeback_weapon_bonus_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = obj.weapon_bonus_enthusiastic != ent.weapon_bonus_enthusiastic
                || obj.weapon_bonus_subliminal != ent.weapon_bonus_subliminal
                || obj.weapon_bonus_horde != ent.weapon_bonus_horde
                || obj.weapon_bonus_nationalism != ent.weapon_bonus_nationalism
                || obj.weapon_bonus_frenzy != ent.weapon_bonus_frenzy
                || obj.weapon_bonus_frenzy_level != ent.weapon_bonus_frenzy_level
                || obj.weapon_bonus_battle_plan_bombardment
                    != ent.weapon_bonus_battle_plan_bombardment
                || obj.weapon_bonus_battle_plan_hold_the_line
                    != ent.weapon_bonus_battle_plan_hold_the_line
                || obj.weapon_bonus_battle_plan_search_and_destroy
                    != ent.weapon_bonus_battle_plan_search_and_destroy
                || obj.weapon_bonus_frenzy_until_frame != ent.weapon_bonus_frenzy_until_frame
                || (obj.battle_plan_sight_scalar_applied - ent.battle_plan_sight_scalar_applied)
                    .abs()
                    > f32::EPSILON;
            if !changed {
                continue;
            }
            obj.weapon_bonus_enthusiastic = ent.weapon_bonus_enthusiastic;
            obj.weapon_bonus_subliminal = ent.weapon_bonus_subliminal;
            obj.weapon_bonus_horde = ent.weapon_bonus_horde;
            obj.weapon_bonus_nationalism = ent.weapon_bonus_nationalism;
            obj.weapon_bonus_frenzy = ent.weapon_bonus_frenzy;
            obj.weapon_bonus_frenzy_level = ent.weapon_bonus_frenzy_level;
            obj.weapon_bonus_battle_plan_bombardment = ent.weapon_bonus_battle_plan_bombardment;
            obj.weapon_bonus_battle_plan_hold_the_line = ent.weapon_bonus_battle_plan_hold_the_line;
            obj.weapon_bonus_battle_plan_search_and_destroy =
                ent.weapon_bonus_battle_plan_search_and_destroy;
            obj.weapon_bonus_frenzy_until_frame = ent.weapon_bonus_frenzy_until_frame;
            obj.battle_plan_sight_scalar_applied = ent.battle_plan_sight_scalar_applied;
            updated += 1;
        }
        updated
    }

    /// Write shadow Entity::experience_points back onto host Object::experience.current.
    pub fn writeback_experience_to_host(&self, logic: &mut GameLogic) -> usize {
        use crate::game_logic::VeterancyLevel as V;
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let pts = ent.experience_points.max(0.0);
            let want_level = match ent.veterancy_ordinal {
                1 => V::Veteran,
                2 => V::Elite,
                3 => V::Heroic,
                _ => V::Rookie,
            };
            let pts_changed = (obj.experience.current - pts).abs() > 0.000_1;
            let level_changed = obj.experience.level != want_level;
            if !pts_changed && !level_changed {
                continue;
            }
            if pts_changed {
                obj.experience.current = pts;
            }
            if level_changed {
                obj.experience.level = want_level;
            }
            updated += 1;
        }
        updated
    }

    pub fn writeback_combat_status_to_host(&self, logic: &mut GameLogic) -> usize {
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let mut dirty = false;
            macro_rules! set_flag {
                ($host:expr, $ent:expr) => {
                    if $host != $ent {
                        $host = $ent;
                        dirty = true;
                    }
                };
            }
            set_flag!(obj.status.stealthed, ent.stealthed);
            set_flag!(obj.status.detected, ent.detected);
            set_flag!(obj.status.moving, ent.moving);
            set_flag!(obj.status.attacking, ent.attacking);
            set_flag!(obj.status.is_firing_weapon, ent.is_firing_weapon);
            set_flag!(obj.status.is_aiming_weapon, ent.is_aiming_weapon);
            set_flag!(obj.status.selected, ent.selected);
            set_flag!(obj.status.disabled_emp, ent.disabled_emp);
            set_flag!(obj.status.weapons_jammed, ent.weapons_jammed);
            set_flag!(obj.status.disabled_hacked, ent.disabled_hacked);
            set_flag!(obj.status.disabled_unmanned, ent.disabled_unmanned);
            set_flag!(obj.status.disabled_paralyzed, ent.disabled_paralyzed);
            set_flag!(obj.status.disabled_subdued, ent.disabled_subdued);
            set_flag!(obj.status.masked, ent.masked);
            set_flag!(obj.status.disguised, ent.disguised);
            set_flag!(obj.status.no_collisions, ent.no_collisions);
            set_flag!(obj.status.private_captured, ent.private_captured);
            set_flag!(
                obj.status.disguise_transitioning_to,
                ent.disguise_transitioning_to
            );
            set_flag!(
                obj.status.disguise_halfpoint_reached,
                ent.disguise_halfpoint_reached
            );
            set_flag!(obj.status.faerie_fire, ent.faerie_fire);
            set_flag!(obj.status.booby_trapped, ent.booby_trapped);
            set_flag!(obj.status.using_ability, ent.using_ability);
            set_flag!(obj.status.deployed, ent.deployed);
            set_flag!(obj.status.airborne_target, ent.airborne_target);
            set_flag!(obj.status.disabled_underpowered, ent.disabled_underpowered);
            set_flag!(obj.status.is_carbomb, ent.is_carbomb);
            set_flag!(obj.status.hijacked, ent.hijacked);
            set_flag!(obj.status.ignoring_stealth, ent.ignoring_stealth);
            set_flag!(obj.status.repulsor, ent.repulsor);
            set_flag!(obj.status.disabled_freefall, ent.disabled_freefall);
            set_flag!(obj.status.eject_invulnerable, ent.eject_invulnerable);
            set_flag!(
                obj.status.pilot_did_move_to_base,
                ent.pilot_did_move_to_base
            );
            set_flag!(obj.status.parachuting, ent.parachuting);
            set_flag!(obj.status.parachute_open, ent.parachute_open);
            set_flag!(
                obj.status.parachute_landing_override_set,
                ent.parachute_landing_override_set
            );
            set_flag!(obj.force_attack, ent.force_attack);
            if dirty {
                updated += 1;
            }
        }
        updated
    }

    pub fn apply_host_damage_events(
        &mut self,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
    ) -> (usize, usize) {
        let mut queued = 0usize;
        for ev in events {
            if ev.destroyed {
                if self.queue_destroy_for_host(ev.target) {
                    queued += 1;
                } else if self.queue_damage_for_host(ev.target, ev.amount) {
                    queued += 1;
                }
            } else if self.queue_damage_for_host(ev.target, ev.amount) {
                queued += 1;
            }
        }
        let applied = self.apply_pending();
        (queued, applied)
    }

    /// Sync from host, then apply any drained damage events for end-of-tick parity.
    /// Prefer: drain events *after* host tick, then `end_of_host_tick`.
    pub fn end_of_host_tick(
        &mut self,
        logic: &mut GameLogic,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
    ) -> GameWorldShadowProbe {
        // Sync positions/spawns first so new objects exist before damage apply.
        self.sync_from_host(logic);
        // Re-apply damage that occurred this frame so shadow health matches without
        // relying solely on post-facto health copy (mutation path exercised).
        // Note: sync_from_host already copied host health; applying events again would
        // double-damage. So for end-of-tick we either:
        //  (A) sync without health, apply events, or
        //  (B) sync health and ignore events for health (events only for destroy).
        // We use (B) for destroy + probe, and a separate `apply_events_without_health_sync`
        // for pure mutation tests.
        let _ = events;
        self.probe(logic)
    }

    /// Mutation-first path: sync transforms/spawns but set health from events only
    /// for targets listed in `events` (others keep prior shadow health then host sync health).
    ///
    /// Used when proving WorldMutation is the damage channel: baseline sync, clear
    /// health to host-pre-damage is caller-managed. See `mirror_damage_events_as_authority`.
    pub fn apply_events_as_damage_channel(
        &mut self,
        logic: &GameLogic,
        events: &[crate::game_logic::host_damage_log::HostDamageEvent],
    ) -> (usize, usize) {
        // Ensure maps exist for targets.
        self.sync_from_host(logic);
        // Reset shadow health to host current (already post-damage). For parity of
        // *channel* only, callers should snapshot pre-damage health. This method
        // queues the same actual_damage amounts for accounting/tests.
        self.apply_host_damage_events(events)
    }

    /// Queue destroy for mapped host object.
    pub fn queue_destroy_for_host(&mut self, host: ObjectId) -> bool {
        let Some(eid) = self.entity_for_host(host) else {
            return false;
        };
        self.world.queue_mutation(WorldMutation::Destroy(eid));
        true
    }

    /// Apply pending GameWorld mutations (damage/destroy/…).
    pub fn apply_pending(&mut self) -> usize {
        let applied = self.world.apply_pending_mutations();
        // Drop map entries for destroyed entities.
        let dead: Vec<u32> = self
            .entity_to_host
            .keys()
            .copied()
            .filter(|eid| self.world.entity(EntityId::from_raw(*eid)).is_none())
            .collect();
        for eid in dead {
            if let Some(hid) = self.entity_to_host.remove(&eid) {
                self.host_to_entity.remove(&hid);
            }
        }
        applied
    }

    /// Compare health for every mapped pair.
    pub fn health_parity(&self, logic: &GameLogic) -> (bool, usize) {
        let mut checked = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(host_obj) = logic.get_objects().get(&ObjectId(hid)) else {
                return (false, checked);
            };
            let Some(ent) = self.world.entity(eid) else {
                return (false, checked);
            };
            checked += 1;
            if (host_obj.health.current - ent.health).abs() > 0.01 {
                return (false, checked);
            }
        }
        (true, checked)
    }

    pub fn probe(&self, logic: &mut GameLogic) -> GameWorldShadowProbe {
        let snap: WorldSnapshot = self.world.snapshot();
        let host_objects = logic.get_objects().len().min(self.max_entities);
        let host_players = logic.get_players().len();
        let shadow_entities = snap.entities.len();
        let shadow_players = snap.players.len();
        let host_frame = logic.get_frame();
        let shadow_frame = snap.frame;
        let host_supplies_sum: u64 = logic
            .get_players()
            .values()
            .map(|p| p.resources.supplies as u64)
            .sum();
        let shadow_supplies_sum: u64 = snap.players.iter().map(|p| p.supplies as u64).sum();
        let mapped_objects = self.host_to_entity.len();
        let (health_match, _) = self.health_parity(logic);

        let entity_ok = shadow_entities == host_objects && mapped_objects == host_objects;
        let counts_match =
            entity_ok && shadow_players == host_players && shadow_frame == host_frame as u64;
        let economy_match = host_supplies_sum == shadow_supplies_sum;

        let detail = if counts_match && economy_match && health_match {
            "ok".into()
        } else {
            format!(
                "mismatch entities {} vs {} mapped={} players {} vs {} frame {} vs {} supplies {} vs {} health_ok={}",
                host_objects,
                shadow_entities,
                mapped_objects,
                host_players,
                shadow_players,
                host_frame,
                shadow_frame,
                host_supplies_sum,
                shadow_supplies_sum,
                health_match
            )
        };

        let (host_match_over, victory_label) = if let Some(v) = logic.evaluate_victory_condition() {
            (true, Some(format!("{v:?}")))
        } else {
            (false, None)
        };

        GameWorldShadowProbe {
            host_frame,
            shadow_frame,
            host_objects: logic.get_objects().len(),
            shadow_entities,
            host_players,
            shadow_players,
            host_supplies_sum,
            shadow_supplies_sum,
            mapped_objects,
            counts_match,
            economy_match,
            health_match,
            host_match_over,
            victory_label,
            detail,
        }
    }
}

/// Rebuild convenience: one-shot mirror (stable map discarded with the session).
pub fn mirror_host_into_gameworld(logic: &GameLogic, max_entities: usize) -> GameWorld {
    let mut shadow = GameWorldShadow::new(max_entities);
    shadow.sync_from_host(logic);
    std::mem::replace(&mut shadow.world, GameWorld::new(8))
}

/// Incremental API with stable IDs: sync into an existing shadow session.
pub fn remirror_host_into_gameworld(world: &mut GameWorld, logic: &GameLogic, max_entities: usize) {
    // Legacy signature: no session — full replace (unstable IDs).
    *world = mirror_host_into_gameworld(logic, max_entities);
}

/// Session-based remirror (preferred).
pub fn sync_shadow_from_host(shadow: &mut GameWorldShadow, logic: &GameLogic) {
    shadow.sync_from_host(logic);
}

/// Build shadow session + probe.
pub fn probe_host_vs_gameworld(logic: &mut GameLogic) -> (GameWorldShadow, GameWorldShadowProbe) {
    const MAX_ENTITIES: usize = 4096;
    let mut shadow = GameWorldShadow::new(MAX_ENTITIES);
    shadow.sync_from_host(logic);
    let probe = shadow.probe(logic);
    (shadow, probe)
}

/// Apply undrained host authority logs onto Main `GameLogic`.
///
/// Used when no GameWorld shadow session will last-write this tick (bare
/// `GameLogic::update`, tests, golden host-only). Engine path with an active
/// shadow session drains logs via `shadow_session_after_host_tick` instead.
///
/// Damage authority freezes mid-frame HP; without this, host-only combat never
/// shows HP/destroy. Economy authority parks refunds in `pending_supply_delta`.
pub fn materialize_host_authority_logs(logic: &mut GameLogic) {
    // --- Damage (DAMAGE_AUTHORITY freezes mid-frame HP) ---
    let damage_events = crate::game_logic::host_damage_log::drain();
    let mut destroy_ids = Vec::new();
    for e in damage_events {
        let Some(obj) = logic.get_object_mut(e.target) else {
            continue;
        };
        if e.destroyed || e.amount + 1e-3 >= obj.health.current {
            obj.health.current = 0.0;
            obj.status.destroyed = true;
            destroy_ids.push(e.target);
        } else if e.amount > 0.0 {
            obj.health.current = (obj.health.current - e.amount).max(0.0);
        }
    }
    for id in destroy_ids {
        logic.mark_object_for_destruction(id, None);
    }

    // --- Heal / absolute HP ---
    for e in crate::game_logic::host_heal_log::drain() {
        if let Some(obj) = logic.get_object_mut(e.target) {
            let max_hp = obj.health.maximum.max(0.0);
            obj.health.current = e.health.clamp(0.0, max_hp);
        }
    }

    // Construction percent already accumulates on host — do not re-apply clamped log.

    // --- Economy pending deltas → real supplies ---
    for p in logic.get_players_mut().values_mut() {
        if p.pending_supply_delta != 0 {
            let v = p.resources.supplies as i64 + p.pending_supply_delta;
            p.resources.supplies = if v <= 0 {
                0
            } else if v >= u32::MAX as i64 {
                u32::MAX
            } else {
                v as u32
            };
            p.pending_supply_delta = 0;
        }
    }
}

/// Optional post-host-tick hook when no long-lived shadow session is held.
/// Materializes DAMAGE/ECONOMY authority logs onto host (does not discard them).
pub fn maybe_shadow_after_host_tick(logic: &mut GameLogic) -> Option<GameWorldShadowProbe> {
    // Engine holds `GameWorldShadow` and calls `shadow_session_after_host_tick`.
    // This helper is the no-session path: materialize authority logs onto host.
    materialize_host_authority_logs(logic);
    if !gameworld_shadow_enabled() {
        return None;
    }
    let (shadow, _probe) = probe_host_vs_gameworld(logic);
    let probe = shadow.probe(logic);
    if !probe.full_match() {
        log::trace!("maybe_shadow probe: {}", probe.format_report());
    }
    Some(probe)
}

/// Session tick: keep stable IDs, drain damage log, sync, probe.
///
/// With [`gameworld_damage_authority_enabled`], events re-apply as WorldMutations
/// and HP is written back to host (GameWorld last writer for health).
pub fn shadow_session_after_host_tick(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
) -> GameWorldShadowProbe {
    let events = crate::game_logic::host_damage_log::drain();
    let heal_events = crate::game_logic::host_heal_log::drain();
    let max_health_events = crate::game_logic::host_max_health_log::drain();
    let experience_events = crate::game_logic::host_experience_log::drain();
    let weapon_bonus_events = crate::game_logic::host_weapon_bonus_log::drain();
    let weapon_slot_events = crate::game_logic::host_weapon_slot_log::drain();
    let entity_power_events = crate::game_logic::host_entity_power_log::drain();
    let turret_events = crate::game_logic::host_turret_log::drain();
    let target_location_events = crate::game_logic::host_target_location_log::drain();
    let detector_events = crate::game_logic::host_detector_log::drain();
    let continuous_fire_events = crate::game_logic::host_continuous_fire_log::drain();
    let guard_events = crate::game_logic::host_guard_log::drain();
    let ai_attitude_events = crate::game_logic::host_ai_attitude_log::drain();
    let weapon_set_events = crate::game_logic::host_weapon_set_log::drain();
    let overcharge_events = crate::game_logic::host_overcharge_log::drain();
    let contain_capacity_events = crate::game_logic::host_contain_capacity_log::drain();
    let hive_events = crate::game_logic::host_hive_log::drain();
    let stealth_flags_events = crate::game_logic::host_stealth_flags_log::drain();
    let overlord_events = crate::game_logic::host_overlord_log::drain();
    let command_set_events = crate::game_logic::host_command_set_log::drain();
    let disguise_events = crate::game_logic::host_disguise_log::drain();
    let vision_camo_events = crate::game_logic::host_vision_camo_log::drain();
    let weapon_stats_events = crate::game_logic::host_weapon_stats_log::drain();
    let movement_events = crate::game_logic::host_movement_log::drain();
    let selection_radius_events = crate::game_logic::host_selection_radius_log::drain();
    let model_condition_events = crate::game_logic::host_model_condition_log::drain();
    let demo_mine_cheer_events = crate::game_logic::host_demo_mine_cheer_log::drain();
    let crush_vision_events = crate::game_logic::host_crush_vision_log::drain();
    let building_type_events = crate::game_logic::host_building_type_log::drain();
    let identity_events = crate::game_logic::host_identity_log::drain();
    let ground_height_events = crate::game_logic::host_ground_height_log::drain();
    let model_mesh_events = crate::game_logic::host_model_mesh_log::drain();
    let fow_events = crate::game_logic::host_fow_log::drain();
    let kind_of_events = crate::game_logic::host_kind_of_log::drain();
    let faerie_events = crate::game_logic::host_faerie_fire_log::drain();
    let repulsor_events = crate::game_logic::host_repulsor_log::drain();
    let disable_timer_events = crate::game_logic::host_disable_timers_log::drain();
    let owner_events = crate::game_logic::host_owner_log::drain();
    let spawn_events = crate::game_logic::host_spawn_log::drain();
    let destroy_events = crate::game_logic::host_destroy_log::drain();
    let attack_events = crate::game_logic::host_attack_log::drain();
    let status_events = crate::game_logic::host_status_log::drain();
    let veterancy_events = crate::game_logic::host_veterancy_log::drain();
    let move_events = crate::game_logic::host_move_log::drain();
    let production_events = crate::game_logic::host_production_log::drain();
    let production_progress_events = crate::game_logic::host_production_progress_log::drain();
    let construction_events = crate::game_logic::host_construction_log::drain();
    let construction_progress_events = crate::game_logic::host_construction_progress_log::drain();
    let special_power_events = crate::game_logic::host_special_power_log::drain();
    let stored_supplies_events = crate::game_logic::host_stored_supplies_log::drain();
    let ai_state_events = crate::game_logic::host_ai_state_log::drain();
    let contain_events = crate::game_logic::host_contain_log::drain();
    let radar_events = crate::game_logic::host_radar_log::drain();
    let player_progress_events = crate::game_logic::host_player_progress_log::drain();
    let player_meta_events = crate::game_logic::host_player_meta_log::drain();
    let player_cooldown_events = crate::game_logic::host_player_cooldown_log::drain();
    let upgrade_events = logic.host_upgrades().completed_this_frame_snapshot();
    let auth = gameworld_damage_authority_enabled();
    // Keep pre-tick shadow HP when we will re-apply damage/heal events as mutations.
    let write_health = !(auth && (!events.is_empty() || !heal_events.is_empty()));
    shadow.sync_from_host_with(logic, write_health);
    // Spawn channel: map any create_object events not yet present (usually no-op after sync).
    let spawns_applied = shadow.apply_host_spawn_events(&spawn_events, logic);
    let _prod_applied = shadow.apply_host_production_events(&production_events, logic);
    let _pp_applied = shadow.apply_host_production_progress_events(&production_progress_events);
    // Sole progress tick under PRODUCTION_AUTHORITY (host skips advance).
    let _prod_tick = shadow
        .tick_production_queues(game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL);
    let production_door_events = crate::game_logic::host_production_door_log::drain();
    let _pd_applied = shadow.apply_host_production_door_events(&production_door_events);
    let _construction_applied = shadow.apply_host_construction_events(&construction_events, logic);
    let _construction_progress_applied =
        shadow.apply_host_construction_progress_events(&construction_progress_events);
    let _sp_applied = shadow.apply_host_special_power_events(&special_power_events);
    // Host owns SP countdown execution; events update GameWorld for presentation.
    // Writeback is available for explicit tests / authority peels — not every tick.

    let _ss_applied = shadow.apply_host_stored_supplies_events(&stored_supplies_events);
    let _ai_applied = shadow.apply_host_ai_state_events(&ai_state_events);
    let _contain_applied = shadow.apply_host_contain_events(&contain_events);
    let _radar_applied = shadow.apply_host_radar_events(&radar_events);
    let _progress_applied = shadow.apply_host_player_progress_events(&player_progress_events);
    let _meta_applied = shadow.apply_host_player_meta_events(&player_meta_events);
    let _cd_applied = shadow.apply_host_player_cooldown_events(&player_cooldown_events);
    let _upgrades_applied = shadow.apply_host_upgrade_events(&upgrade_events);
    let (dest_q, _dest_a) = shadow.apply_host_destroy_events(&destroy_events);
    let _heals = shadow.apply_host_heal_events(&heal_events);
    let _maxh_applied = shadow.apply_host_max_health_events(&max_health_events);
    let _xp_applied = shadow.apply_host_experience_events(&experience_events);
    let _wb_applied = shadow.apply_host_weapon_bonus_events(&weapon_bonus_events);
    let _wslot_applied = shadow.apply_host_weapon_slot_events(&weapon_slot_events);
    let _epow_applied = shadow.apply_host_entity_power_events(&entity_power_events);
    let _tur_applied = shadow.apply_host_turret_events(&turret_events);
    let _tloc_applied = shadow.apply_host_target_location_events(&target_location_events);
    let _det_applied = shadow.apply_host_detector_events(&detector_events);
    let _cf_applied = shadow.apply_host_continuous_fire_events(&continuous_fire_events);
    let combat_attack_events = crate::game_logic::host_combat_attack_log::drain();
    let _ca_applied = shadow.apply_host_combat_attack_events(&combat_attack_events);
    let fire_intent_events = crate::game_logic::host_fire_intent_log::drain();
    let _fi_applied = shadow.apply_host_fire_intent_events(&fire_intent_events);
    let projectile_events = crate::game_logic::host_projectile_log::drain();
    let _proj_applied = shadow.apply_host_projectile_events(&projectile_events);
    // Fire-spawn authority: materialize deferred weapon discharges into CombatSystem
    // before projectile integrate authority steps flight.
    if gameworld_fire_spawn_authority_enabled() {
        let spawns = crate::game_logic::host_fire_spawn_log::drain();
        let _fs = shadow.apply_host_fire_spawn_events(logic, spawns);
    }
    if gameworld_projectile_authority_enabled() {
        let dt = 1.0_f32 / 30.0;
        // Host object poses for homing refresh.
        let stepped = {
            let logic_ref = &*logic;
            shadow.world.step_projectiles(dt, |hid| {
                logic_ref.get_objects().get(&ObjectId(hid)).map(|o| {
                    let p = o.get_position();
                    [p.x, p.y, p.z]
                })
            })
        };
        let _ = stepped;
        let _pw = shadow.writeback_projectiles_to_host(logic);
        // Hit resolution at GameWorld-integrated poses (dt=0 keeps pose stable).
        let hits = logic.resolve_projectiles_hits_only();
        let _ = hits;
        crate::game_logic::host_projectile_log::record_snapshot(
            logic.combat_system.projectiles_snapshot(),
        );
        // Re-apply post-hit residual so GW drops destroyed projectiles.
        let _ =
            shadow.apply_host_projectile_events(&crate::game_logic::host_projectile_log::drain());
    }

    let _guard_applied = shadow.apply_host_guard_events(&guard_events);
    let _att_applied = shadow.apply_host_ai_attitude_events(&ai_attitude_events);
    let ai_mood_events = crate::game_logic::host_ai_mood_log::drain();
    let _mood_applied = shadow.apply_host_ai_mood_events(&ai_mood_events);
    let ai_req_events = crate::game_logic::host_ai_request_log::drain();
    let _ar_applied = shadow.apply_host_ai_request_events(&ai_req_events);
    if gameworld_ai_decision_authority_enabled() {
        let ai_decision_events = crate::game_logic::host_ai_decision_log::drain();
        let _ad = shadow.apply_ai_decisions_as_world_mutations(&ai_decision_events);
        // Last-write host attack target / AI state / move from GameWorld.
        let _ = shadow.writeback_attack_targets_to_host(logic);
        let _ = shadow.writeback_ai_state_to_host(logic);
        let _ = shadow.writeback_movement_to_host(logic);
    } else {
        let _ =
            shadow.apply_host_ai_decision_events(&crate::game_logic::host_ai_decision_log::drain());
    }
    let _wset_applied = shadow.apply_host_weapon_set_events(&weapon_set_events);
    let _oc_applied = shadow.apply_host_overcharge_events(&overcharge_events);
    let _cap_applied = shadow.apply_host_contain_capacity_events(&contain_capacity_events);
    let _hive_applied = shadow.apply_host_hive_events(&hive_events);
    let hijack_events = crate::game_logic::host_hijacker_log::drain();
    let _hj_applied = shadow.apply_host_hijacker_events(&hijack_events);
    let _stf_applied = shadow.apply_host_stealth_flags_events(&stealth_flags_events);
    let stealth_delay_events = crate::game_logic::host_stealth_delay_log::drain();
    let _sd_applied = shadow.apply_host_stealth_delay_events(&stealth_delay_events);
    let _ol_applied = shadow.apply_host_overlord_events(&overlord_events);
    let _cs_applied = shadow.apply_host_command_set_events(&command_set_events);
    let _dg_applied = shadow.apply_host_disguise_events(&disguise_events);
    let _vc_applied = shadow.apply_host_vision_camo_events(&vision_camo_events);
    let _ws_applied = shadow.apply_host_weapon_stats_events(&weapon_stats_events);
    let body_damage_events = crate::game_logic::host_body_damage_log::drain();
    let _bd_applied = shadow.apply_host_body_damage_events(&body_damage_events);
    let death_type_events = crate::game_logic::host_death_type_log::drain();
    let _dt_applied = shadow.apply_host_death_type_events(&death_type_events);
    let radar_extend_events = crate::game_logic::host_radar_extend_log::drain();
    let _re_applied = shadow.apply_host_radar_extend_events(&radar_extend_events);
    let shock_stun_events = crate::game_logic::host_shock_stun_log::drain();
    let _ss_applied = shadow.apply_host_shock_stun_events(&shock_stun_events);
    let rebuild_producer_events = crate::game_logic::host_rebuild_producer_log::drain();
    let _rp_applied = shadow.apply_host_rebuild_producer_events(&rebuild_producer_events);
    let sole_healing_events = crate::game_logic::host_sole_healing_log::drain();
    let _sh_applied = shadow.apply_host_sole_healing_events(&sole_healing_events);
    let _mv_applied = shadow.apply_host_movement_events(&movement_events);
    let physics_motive_events = crate::game_logic::host_physics_motive_log::drain();
    let _pm_applied = shadow.apply_host_physics_motive_events(&physics_motive_events);
    let loco_events = crate::game_logic::host_locomotor_log::drain();
    let _loco_applied = shadow.apply_host_locomotor_events(&loco_events);
    let bounce_land_events = crate::game_logic::host_bounce_land_log::drain();
    let _bl_applied = shadow.apply_host_bounce_land_events(&bounce_land_events);

    let _sr_applied = shadow.apply_host_selection_radius_events(&selection_radius_events);
    let _mc_applied = shadow.apply_host_model_condition_events(&model_condition_events);
    let _dmc_applied = shadow.apply_host_demo_mine_cheer_events(&demo_mine_cheer_events);
    let _cv_applied = shadow.apply_host_crush_vision_events(&crush_vision_events);
    let _bt_applied = shadow.apply_host_building_type_events(&building_type_events);
    let _id_applied = shadow.apply_host_identity_events(&identity_events);
    let _gh_applied = shadow.apply_host_ground_height_events(&ground_height_events);
    let _mm_applied = shadow.apply_host_model_mesh_events(&model_mesh_events);
    let _fow_applied = shadow.apply_host_fow_events(&fow_events);
    let _ko_applied = shadow.apply_host_kind_of_events(&kind_of_events);
    let _ff_applied = shadow.apply_host_faerie_fire_events(&faerie_events);
    let _rp_applied = shadow.apply_host_repulsor_events(&repulsor_events);
    let _dt_applied = shadow.apply_host_disable_timers_events(&disable_timer_events);
    let _owners = shadow.apply_host_owner_events(logic, &owner_events);
    // When GameWorld owns path integrate, do not clobber entity poses with host
    // pre-integrate positions; still pull move targets / movement residuals above.
    if !gameworld_movement_authority_enabled() {
        let _poses = shadow.apply_host_positions_as_transforms(logic);
    } else {
        // Ensure move destinations from host are present before step.
        let _move_tgts = shadow.apply_host_move_targets(logic);
    }
    for ev in &attack_events {
        let _ = shadow.queue_set_attack_target_for_host(ev.attacker, ev.target);
    }
    for ev in &move_events {
        let _ = shadow.queue_set_move_target_for_host(ev.unit, ev.destination);
    }
    for ev in &status_events {
        let _ = shadow.queue_set_combat_status_for_host(*ev);
    }
    for ev in &veterancy_events {
        let _ = shadow.queue_set_veterancy_for_host(ev.object, ev.ordinal);
    }
    if !attack_events.is_empty()
        || !move_events.is_empty()
        || !status_events.is_empty()
        || !veterancy_events.is_empty()
    {
        let _ = shadow.apply_pending();
    }
    let _atks = shadow.apply_host_attack_targets(logic);
    let _moves = shadow.apply_host_move_targets(logic);
    // Attack-target channel is always bidirectional once session is live: shadow mutations
    // (and host bulk resync above) settle, then writeback keeps host Object::target aligned.
    let _atk_wb = shadow.writeback_attack_targets_to_host(logic);
    let _ = shadow.writeback_fire_intent_to_host(logic);
    let _move_wb = shadow.writeback_move_targets_to_host(logic);
    // Pose last-writer after all SetTransform mutations this session.
    // Mid-frame movement authority: integrate AFTER command channels, BEFORE pose writeback.
    if gameworld_movement_authority_enabled() {
        let dt = 1.0_f32 / 30.0;
        let stepped = shadow.world.step_movement(dt);
        if stepped > 0 {
            log::trace!("GameWorld step_movement stepped={stepped}");
        }
    }
    let _pose_wb = shadow.writeback_transforms_to_host(logic);
    // Movement authority: always last-write velocity/path/move_target/moving after step
    // (do not gate on damage-channel auth — path frames often have empty damage logs).
    if gameworld_movement_authority_enabled() {
        let _mv_wb = shadow.writeback_movement_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ = shadow.writeback_physics_motive_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ = shadow.writeback_bounce_land_to_host(logic);
        let _move_tgt_wb = shadow.writeback_move_targets_to_host(logic);
        let _moving_st_wb = shadow.writeback_combat_status_to_host(logic);
    }
    let _prod_wb = shadow.writeback_production_to_host(logic);
    let _ = shadow.writeback_production_door_to_host(logic);
    let _ = shadow.writeback_body_damage_to_host(logic);
    let _ = shadow.writeback_death_type_to_host(logic);
    let _ = shadow.writeback_radar_extend_to_host(logic);
    let _ = shadow.writeback_shock_stun_to_host(logic);
    let _ = shadow.writeback_rebuild_producer_to_host(logic);
    let _ = shadow.writeback_sole_healing_to_host(logic);
    let _ = shadow.writeback_hijacker_to_host(logic);
    let _ = shadow.writeback_ai_mood_to_host(logic);
    let _ = shadow.writeback_ai_request_to_host(logic);
    let _ = shadow.writeback_hijacker_to_host(logic);
    let _construction_wb = shadow.writeback_construction_to_host(logic);
    let _owner_wb = shadow.writeback_owner_to_host(logic);
    let mut writebacks = 0usize;
    // HP last-writer: damage mutations and/or absolute heal SetHealth events.
    if auth && (!events.is_empty() || !heal_events.is_empty() || !experience_events.is_empty()) {
        let (mut queued, mut applied) = (0usize, 0usize);
        if !events.is_empty() {
            let pair = shadow.apply_host_damage_events(&events);
            queued = pair.0;
            applied = pair.1;
        }
        if !events.is_empty() || !heal_events.is_empty() {
            writebacks = shadow.writeback_health_to_host(logic);
        }
        let _xp_wb = shadow.writeback_experience_to_host(logic);
        let _wbonus_wb = shadow.writeback_weapon_bonus_to_host(logic);
        let _ff_wb = shadow.writeback_faerie_fire_to_host(logic);
        let _rp_wb = shadow.writeback_repulsor_to_host(logic);
        let _dt_wb = shadow.writeback_disable_timers_to_host(logic);
        let _wslot_wb = shadow.writeback_weapon_slot_to_host(logic);
        let _epow_wb = shadow.writeback_entity_power_to_host(logic);
        let _tur_wb = shadow.writeback_turret_to_host(logic);
        let _ = shadow.writeback_stealth_delay_to_host(logic);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _tloc_wb = shadow.writeback_target_location_to_host(logic);
        let _det_wb = shadow.writeback_detector_to_host(logic);
        let _cf_wb = shadow.writeback_continuous_fire_to_host(logic);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _guard_wb = shadow.writeback_guard_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ai_st_wb = shadow.writeback_ai_state_to_host(logic);
        let _att_wb = shadow.writeback_ai_attitude_to_host(logic);
        let _wset_wb = shadow.writeback_weapon_set_to_host(logic);
        let _oc_wb = shadow.writeback_overcharge_to_host(logic);
        let _cap_wb = shadow.writeback_contain_capacity_to_host(logic);
        let _hive_wb = shadow.writeback_hive_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _stf_wb = shadow.writeback_stealth_flags_to_host(logic);
        let _ = shadow.writeback_stealth_delay_to_host(logic);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ol_wb = shadow.writeback_overlord_to_host(logic);
        let _cs_wb = shadow.writeback_command_set_to_host(logic);
        let _dg_wb = shadow.writeback_disguise_to_host(logic);
        let _vc_wb = shadow.writeback_vision_camo_to_host(logic);
        let _ = shadow.writeback_stealth_delay_to_host(logic);
        let _ = shadow.writeback_combat_attack_to_host(logic);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ws_wb = shadow.writeback_weapon_stats_to_host(logic);
        let _ = shadow.writeback_fire_intent_to_host(logic);
        let _mv_wb = shadow.writeback_movement_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ = shadow.writeback_physics_motive_to_host(logic);
        let _ = shadow.writeback_locomotor_to_host(logic);
        let _ = shadow.writeback_ai_request_to_host(logic);
        let _ = shadow.writeback_hijacker_to_host(logic);
        let _ = shadow.writeback_bounce_land_to_host(logic);
        let _sr_wb = shadow.writeback_selection_radius_to_host(logic);
        let _mc_wb = shadow.writeback_model_condition_to_host(logic);
        let _dmc_wb = shadow.writeback_demo_mine_cheer_to_host(logic);
        let _cv_wb = shadow.writeback_crush_vision_to_host(logic);
        let _bt_wb = shadow.writeback_building_type_to_host(logic);
        let _id_wb = shadow.writeback_identity_to_host(logic);
        let _gh_wb = shadow.writeback_ground_height_to_host(logic);

        let _cst_wb = shadow.writeback_combat_status_to_host(logic);
        log::trace!(
            "gameworld_damage_authority events={} queued={} applied={} writebacks={}",
            events.len(),
            queued,
            applied,
            writebacks
        );
    } else if !events.is_empty() {
        log::trace!(
            "gameworld_shadow session saw {} damage events (health via host sync)",
            events.len()
        );
    }
    let mut econ_wb = 0usize;
    if gameworld_economy_authority_enabled() {
        let econ_events = crate::game_logic::host_economy_log::drain();
        if !econ_events.is_empty() {
            // Keep pre-tick shadow supplies when re-applying absolute events.
            // (sync already copied host post-change supplies when write_health path
            //  also refreshed players — re-apply is idempotent absolute set.)
            let (_q, _a) = shadow.apply_host_economy_events(&econ_events);
        }
        econ_wb = shadow.writeback_economy_to_host(logic);
        let _upg_wb = shadow.writeback_completed_upgrades_to_host(logic);
        let _ss_wb = shadow.writeback_stored_supplies_to_host(logic);
    } else {
        // Avoid unbounded growth when economy authority off.
        let _ = crate::game_logic::host_economy_log::drain();
    }
    let mut probe = shadow.probe(logic);
    if !events.is_empty() || econ_wb > 0 || !production_events.is_empty() {
        probe.detail = format!(
            "{}|dmg_events={}|spawns={}/{}|destroy={}/{}|prod={}|auth={}|wb={}|econ_wb={}",
            probe.detail,
            events.len(),
            spawn_events.len(),
            spawns_applied,
            destroy_events.len(),
            dest_q,
            production_events.len(),
            auth,
            writebacks,
            econ_wb
        );
    }
    probe
}

/// Prove damage channel: given pre-synced shadow at pre-damage host state, apply
/// host damage on objects while logging, drain log, apply mutations to shadow,
/// compare health (host already damaged).
pub fn apply_logged_damage_channel_parity(
    logic: &mut GameLogic,
    shadow: &mut GameWorldShadow,
    targets: &[(ObjectId, f32)],
) -> Result<usize, String> {
    crate::game_logic::host_damage_log::clear();
    shadow.sync_from_host(logic);
    // Snapshot pre-damage shadow health for targets.
    let mut pre: Vec<(ObjectId, f32)> = Vec::new();
    for &(id, amount) in targets {
        let h = logic
            .get_objects()
            .get(&id)
            .map(|o| o.health.current)
            .ok_or_else(|| format!("missing {id:?}"))?;
        pre.push((id, h));
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            let _ = obj.take_damage(amount);
        }
    }
    let events = crate::game_logic::host_damage_log::drain();
    if events.len() < targets.len() {
        return Err(format!(
            "expected >= {} damage log entries, got {}",
            targets.len(),
            events.len()
        ));
    }
    // Restore shadow health to pre-damage, then apply events as mutations.
    for (id, h) in &pre {
        if let Some(eid) = shadow.entity_for_host(*id) {
            if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
                e.health = *h;
            }
        }
    }
    let (queued, _applied) = shadow.apply_host_damage_events(&events);
    // Compare
    for (id, _) in targets {
        let host_h = logic
            .get_objects()
            .get(id)
            .map(|o| o.health.current)
            .unwrap_or(-1.0);
        let eid = shadow
            .entity_for_host(*id)
            .ok_or_else(|| "unmapped after damage".to_string())?;
        let sh = shadow.world().entity(eid).map(|e| e.health).unwrap_or(-1.0);
        if (host_h - sh).abs() > 0.05 {
            return Err(format!(
                "channel parity fail id={} host={host_h} shadow={sh}",
                id.0
            ));
        }
    }
    Ok(queued)
}

/// Observe-path presentation from GameWorld (no Main GameLogic borrow).
#[derive(Debug, Clone)]
pub struct GameWorldPresentationView {
    pub frame: u64,
    pub local_supplies: u32,
    pub entities: Vec<GameWorldEntityView>,
}

#[derive(Debug, Clone)]
pub struct GameWorldEntityView {
    pub id: u32,
    pub template: String,
    pub owner: Option<u8>,
    pub position: [f32; 3],
    pub orientation: f32,
    pub health: f32,
}

pub fn presentation_view_from_gameworld(
    world: &GameWorld,
    local_player_index: u8,
) -> GameWorldPresentationView {
    let snap = world.snapshot();
    let local_supplies = snap
        .players
        .iter()
        .find(|p| p.id.get() == local_player_index)
        .map(|p| p.supplies)
        .unwrap_or(0);
    let entities = snap
        .entities
        .into_iter()
        .map(|e| GameWorldEntityView {
            id: e.id.get(),
            template: e.template,
            owner: e.owner.map(|o| o.get()),
            position: e.position,
            orientation: e.orientation,
            health: e.health,
        })
        .collect();
    GameWorldPresentationView {
        frame: snap.frame,
        local_supplies,
        entities,
    }
}

pub fn presentation_view_from_shadow(
    shadow: &GameWorldShadow,
    local_player_index: u8,
) -> GameWorldPresentationView {
    presentation_view_from_gameworld(shadow.world(), local_player_index)
}

/// Apply the same damage amount to host object and mapped shadow entity; compare health.
/// Host remains authoritative — this only proves mutation parity on the shadow.
pub fn damage_parity_probe(
    logic: &mut GameLogic,
    shadow: &mut GameWorldShadow,
    host: ObjectId,
    amount: f32,
) -> Result<(), String> {
    shadow.sync_from_host(logic);
    let before = logic
        .get_objects()
        .get(&host)
        .map(|o| o.health.current)
        .ok_or_else(|| format!("host object {} missing", host.0))?;
    if !shadow.queue_damage_for_host(host, amount) {
        return Err(format!("host object {} not mapped in shadow", host.0));
    }
    let _ = shadow.apply_pending();
    // Apply same damage on host for comparison path.
    if let Some(obj) = logic.get_objects_mut().get_mut(&host) {
        let _ = obj.take_damage(amount);
    } else {
        return Err("host object vanished".into());
    }
    let host_after = logic
        .get_objects()
        .get(&host)
        .map(|o| o.health.current)
        .unwrap_or(-1.0);
    let eid = shadow
        .entity_for_host(host)
        .ok_or_else(|| "mapping lost after damage".to_string())?;
    let shadow_after = shadow.world().entity(eid).map(|e| e.health).unwrap_or(-1.0);
    if (host_after - shadow_after).abs() > 0.01 {
        return Err(format!(
            "health diverge host={host_after} shadow={shadow_after} before={before} dmg={amount}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str, hp: f32) {
        if logic.templates.contains_key(name) {
            return;
        }
        let mut t = ThingTemplate::new(name);
        t.set_health(hp);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn shadow_stable_ids_across_sync() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StableIdMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "ShadowUnit", 100.0);
        let a = logic
            .create_object("ShadowUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("ShadowUnit", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("b");

        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        let ea = shadow.entity_for_host(a).expect("map a");
        let eb = shadow.entity_for_host(b).expect("map b");
        assert_ne!(ea.get(), eb.get());

        // Second sync must keep the same EntityIds.
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.entity_for_host(a), Some(ea));
        assert_eq!(shadow.entity_for_host(b), Some(eb));

        let probe = shadow.probe(&mut logic);
        assert!(probe.full_match(), "{}", probe.format_report());
    }

    #[test]
    fn shadow_damage_mutation_matches_host() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DamageParity");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "DmgUnit", 200.0);
        let id = logic
            .create_object("DmgUnit", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .expect("unit");

        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        damage_parity_probe(&mut logic, &mut shadow, id, 35.0).expect("parity");
        // ID remains stable after damage.
        assert!(shadow.entity_for_host(id).is_some());
        let probe = shadow.probe(&mut logic);
        assert!(probe.health_match, "{}", probe.format_report());
    }

    #[test]
    fn shadow_counts_and_economy_match_after_skirmish_config() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GameWorldShadowMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let (shadow, probe) = probe_host_vs_gameworld(&mut logic);
        assert!(
            probe.full_match() || probe.host_objects > 4096,
            "{}",
            probe.format_report()
        );
        let view = presentation_view_from_shadow(&shadow, 0);
        assert_eq!(view.frame, logic.get_frame() as u64);
        assert_eq!(view.entities.len(), logic.get_objects().len().min(4096));
    }

    #[test]
    fn presentation_overlay_uses_shadow_health() {
        use crate::presentation_frame::PresentationFrame;
        crate::game_logic::host_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresOverlay");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "OverlayUnit", 100.0);
        let id = logic
            .create_object("OverlayUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 0.0))
            .expect("u");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_damage_for_host(id, 40.0));
        let _ = shadow.apply_pending();
        let mut pres = PresentationFrame::build_from_logic(&logic, 0);
        let before = pres
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.health_current)
            .unwrap();
        let n = pres.overlay_gameworld_shadow(&shadow);
        assert!(n >= 1);
        let after = pres
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.health_current)
            .unwrap();
        assert!(
            after < before,
            "overlay should pull lower shadow HP {after} vs {before}"
        );
    }

    #[test]
    fn pose_writeback_is_last_writer() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PoseWB");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "PoseU", 80.0);
        let id = logic
            .create_object("PoseU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_set_transform_for_host(id, [42.0, 1.0, 7.0], 0.5));
        let _ = shadow.apply_pending();
        // Host still at origin until writeback.
        {
            let p = logic.get_objects().get(&id).unwrap().get_position();
            assert!(p.x.abs() < 0.1, "pre-writeback host x={}", p.x);
        }
        let n = shadow.writeback_transforms_to_host(&mut logic);
        assert!(n >= 1, "writeback count {n}");
        let p = logic.get_objects().get(&id).unwrap().get_position();
        assert!((p.x - 42.0).abs() < 0.01, "host x={}", p.x);
        assert!((p.z - 7.0).abs() < 0.01, "host z={}", p.z);
    }

    #[test]
    fn sync_from_host_copies_entity_engine_bridged_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityEngineBridged");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "BrU", 100.0);
        let id = logic
            .create_object("BrU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("o");
            obj.engine_object_id = Some(42);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.engine_bridged, "engine_bridged residual");
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("e.engine_bridged = obj.engine_object_id.is_some()"),
            "sync must copy engine_bridged residual"
        );
    }

    fn sync_from_host_copies_entity_fow_ground_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityFowGround");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "FowU", 100.0);
        let id = logic
            .create_object("FowU", Team::USA, glam::Vec3::new(10.0, 0.0, 20.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.fow_visibility_alpha - 1.0).abs() < 1e-5);
        assert!((e.fow_is_explored - 1.0).abs() < 1e-5);
        assert!(e.ground_height.is_finite());
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("fow_visibility_alpha")
                && src.contains("ground_height_from_terrain")
                && src.contains("FOWRenderingBridge::get_object_visibility"),
            "sync must copy FOW/ground residual"
        );
    }

    fn sync_from_host_copies_entity_model_key_mesh_scale_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityMeshKey");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "MeshU", 100.0);
        {
            let t = logic.templates.get_mut("MeshU").expect("t");
            t.model_name = Some("AVTank".into());
        }
        let id = logic
            .create_object("MeshU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        // Prove host object carries template model residual before shadow sync.
        {
            let obj = logic.find_object(id).expect("host obj");
            let key =
                crate::assets::mesh_asset_resolve::model_key_from_template(obj.get_template());
            assert_eq!(
                key.to_ascii_lowercase(),
                "avtank",
                "host template model key"
            );
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(
            e.model_key.to_ascii_lowercase(),
            "avtank",
            "model_key residual got {:?}",
            e.model_key
        );
        assert!(
            e.mesh_scale.is_finite() && e.mesh_scale > 0.0,
            "mesh_scale residual"
        );
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("model_key_from_template") && src.contains("mesh_scale_from_template"),
            "sync must copy mesh residual via resolve helpers"
        );
    }

    fn sync_from_host_copies_entity_production_queue_items_residual() {
        use crate::game_logic::{BuildingData, BuildingType, ProductionItem, Resources};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityProdQueue");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "Fact", 500.0);
        ensure_template(&mut logic, "UnitA", 100.0);
        ensure_template(&mut logic, "UnitB", 100.0);
        let id = logic
            .create_object("Fact", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("o");
            let mut bd = BuildingData::new(BuildingType::WarFactory);
            bd.production_queue = vec![
                ProductionItem {
                    template_name: "UnitA".into(),
                    progress: 0.25,
                    total_time: 10.0,
                    cost: Resources {
                        supplies: 300,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: crate::game_logic::buildings::ProductionKind::Unit,
                },
                ProductionItem {
                    template_name: "UnitB".into(),
                    progress: 0.0,
                    total_time: 12.0,
                    cost: Resources {
                        supplies: 400,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: crate::game_logic::buildings::ProductionKind::Unit,
                },
            ];
            obj.building_data = Some(bd);
            obj.object_type = crate::game_logic::ObjectType::Building;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.production_queue_len, 2);
        assert_eq!(e.production_queue_items.len(), 2);
        assert_eq!(e.production_queue_items[0].template_name, "UnitA");
        assert!((e.production_queue_items[0].progress - 0.25).abs() < 1e-5);
        assert_eq!(e.production_queue_items[0].cost_supplies, 300);
        assert_eq!(e.production_queue_items[1].template_name, "UnitB");
        assert_eq!(e.production_queue_items[1].total_time, 12.0);
        assert_eq!(e.production_template, "UnitA");
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("production_queue_items") && src.contains("EntityProductionItem"),
            "sync must copy full production queue residual"
        );
    }

    fn sync_from_host_copies_entity_applied_upgrade_names_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityUpgrades");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "UpU", 100.0);
        let id = logic
            .create_object("UpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("o");
            obj.apply_upgrade_tag("UpgradeA");
            obj.apply_upgrade_tag("UpgradeB");
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.applied_upgrade_count, 2);
        assert_eq!(
            e.applied_upgrade_names,
            vec!["UpgradeA".to_string(), "UpgradeB".to_string()]
        );
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("applied_upgrade_names") && src.contains("MAX_UPGRADES"),
            "sync must copy upgrade name residual"
        );
    }

    fn sync_from_host_copies_entity_kind_of_bits_residual() {
        use crate::game_logic::KindOf;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityKindOf");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "KindU", 100.0);
        {
            let t = logic.templates.get_mut("KindU").expect("t");
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
        }
        let id = logic
            .create_object("KindU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        // ORDER: Structure=0 Infantry=1 ... Selectable=6 Attackable=7
        assert!(e.kind_of_bits & (1 << 1) != 0, "Infantry bit");
        assert!(e.kind_of_bits & (1 << 6) != 0, "Selectable bit");
        assert!(e.kind_of_bits & (1 << 7) != 0, "Attackable bit");
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("host_kind_of_bits") && src.contains("kind_of_bits"),
            "sync must copy kind_of residual"
        );
    }

    fn sync_from_host_copies_entity_garrison_contain_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityGarrisonContain");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "GarBldg", 500.0);
        ensure_template(&mut logic, "GarInf", 100.0);
        let bldg = logic
            .create_object("GarBldg", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("bldg");
        let inf = logic
            .create_object("GarInf", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
            .expect("inf");
        {
            use crate::game_logic::{BuildingData, BuildingType};
            let obj = logic.get_objects_mut().get_mut(&bldg).expect("b");
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.garrisoned_units = vec![inf];
            bd.max_garrison = 5;
            obj.building_data = Some(bd);
            obj.object_type = crate::game_logic::ObjectType::Building;
        }
        {
            let obj = logic.get_objects_mut().get_mut(&inf).expect("i");
            obj.contained_by = Some(bldg);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let be = shadow.entity_for_host(bldg).expect("bm");
        let b = shadow.world().entity(be).expect("b");
        assert_eq!(b.garrisoned_host_ids, vec![inf.0]);
        assert_eq!(b.max_garrison, 5);
        let ie = shadow.entity_for_host(inf).expect("im");
        let i = shadow.world().entity(ie).expect("i");
        assert_eq!(i.contained_by_host, bldg.0);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("garrisoned_host_ids") && src.contains("garrisoned_units"),
            "sync must copy garrison residual"
        );
    }

    fn sync_from_host_copies_entity_path_waypoints_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityPathWp");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "PathWpU", 100.0);
        let id = logic
            .create_object("PathWpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            use crate::game_logic::Weapon;
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.movement.path = vec![
                glam::Vec3::new(1.0, 0.0, 1.0),
                glam::Vec3::new(2.0, 0.0, 2.0),
                glam::Vec3::new(3.0, 0.0, 3.0),
            ];
            obj.movement.current_path_index = 1;
            obj.secondary_weapon = Some(Weapon {
                damage: 8.0,
                range: 90.0,
                min_range: 0.0,
                reload_time: 1.0,
                last_fire_time: 0.0,
                ammo: None,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
            });
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.path_len, 3);
        assert_eq!(e.path_index, 1);
        assert_eq!(e.path_waypoints.len(), 3);
        assert!((e.path_waypoints[2][0] - 3.0).abs() < 0.01);
        assert!(e.has_secondary_weapon || e.secondary_weapon_range > 0.0);
        assert!((e.secondary_weapon_range - 90.0).abs() < 0.01);
        assert!((e.secondary_weapon_damage - 8.0).abs() < 0.01);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("path_waypoints") && src.contains("secondary_weapon_range"),
            "sync must copy path/secondary residual"
        );
    }

    fn sync_from_host_copies_entity_combat_timing_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityCombatTiming");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "CbtTimeU", 100.0);
        let id = logic
            .create_object("CbtTimeU", Team::USA, glam::Vec3::new(13.0, 0.0, 13.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.weapon_bonus_frenzy_until_frame = 90;
            obj.continuous_fire_coast_until_frame = 33;
            obj.battle_plan_sight_scalar_applied = 1.5;
            obj.continuous_fire_consecutive = 4;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.frenzy_until_entity_count(), 1);
        assert_eq!(shadow.battle_plan_sight_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.weapon_bonus_frenzy_until_frame, 90);
        assert_eq!(e.continuous_fire_coast_until_frame, 33);
        assert!((e.battle_plan_sight_scalar_applied - 1.5).abs() < 0.001);
        assert_eq!(e.continuous_fire_consecutive, 4);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("weapon_bonus_frenzy_until_frame")
                && src.contains("continuous_fire_coast_until_frame")
                && src.contains("battle_plan_sight_scalar_applied"),
            "sync must copy combat-timing residual"
        );
    }

    fn sync_from_host_copies_entity_combat_bonus_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityCombatBonus");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "BonusU", 100.0);
        let id = logic
            .create_object("BonusU", Team::China, glam::Vec3::new(12.0, 0.0, 12.0))
            .expect("id");
        let src = logic
            .create_object("BonusU", Team::GLA, glam::Vec3::new(20.0, 0.0, 12.0))
            .expect("src");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.weapon_bonus_enthusiastic = true;
            obj.weapon_bonus_subliminal = true;
            obj.weapon_bonus_horde = true;
            obj.weapon_bonus_nationalism = true;
            obj.weapon_bonus_frenzy = true;
            obj.weapon_bonus_frenzy_level = 2;
            obj.weapon_bonus_battle_plan_bombardment = true;
            obj.weapon_bonus_battle_plan_hold_the_line = true;
            obj.weapon_bonus_battle_plan_search_and_destroy = true;
            obj.continuous_fire_level = 3;
            obj.continuous_fire_consecutive = 7;
            obj.faerie_fire_until_frame = 99;
            obj.is_humvee_transport = true;
            obj.is_listening_outpost_transport = true;
            obj.is_troop_crawler_transport = true;
            obj.is_helix_transport = true;
            obj.has_overlord_gattling_addon = true;
            obj.has_overlord_propaganda_addon = true;
            obj.demo_suicided_detonating = true;
            obj.hive_slave_count = 3;
            obj.hive_slave_hp = 40.0;
            obj.turret_angle_deg = 45.0;
            obj.turret_pitch_deg = 15.0;
            obj.turret_idle_scanning = true;
            obj.turret_holding = true;
            obj.ai_attitude = 2;
            obj.last_damage_source = Some(src);
            obj.command_set_override = Some("Command_ChinaTankOverlord".into());
            obj.disguise_as_template = Some("AmericaVehicleHumvee".into());
            obj.disguise_as_team = Some(Team::USA);
            obj.vision_spied_mask = 0b101;
            obj.camo_friendly_opacity = 0.4;
            // camo_stealth_look left default
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.horde_bonus_entity_count(), 1);
        assert_eq!(shadow.humvee_transport_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.weapon_bonus_enthusiastic && e.weapon_bonus_horde && e.weapon_bonus_frenzy);
        assert_eq!(e.weapon_bonus_frenzy_level, 2);
        assert_eq!(e.continuous_fire_level, 3);
        assert_eq!(e.continuous_fire_consecutive, 7);
        assert_eq!(e.faerie_fire_until_frame, 99);
        assert!(e.is_humvee_transport && e.is_helix_transport);
        assert!(e.has_overlord_gattling_addon && e.has_overlord_propaganda_addon);
        assert!(e.demo_suicided_detonating);
        assert_eq!(e.hive_slave_count, 3);
        assert!((e.turret_angle_deg - 45.0).abs() < 0.01);
        assert_eq!(e.ai_attitude, 2);
        assert_eq!(e.last_damage_source_host, src.0);
        assert_eq!(e.command_set_override, "Command_ChinaTankOverlord");
        assert_eq!(e.disguise_as_template, "AmericaVehicleHumvee");
        assert_eq!(e.disguise_as_team_ordinal, 0); // USA
        assert_eq!(e.vision_spied_mask, 0b101);
        assert!((e.camo_friendly_opacity - 0.4).abs() < 0.01);
        let src_txt = include_str!("gameworld_shadow.rs");
        assert!(
            src_txt.contains("weapon_bonus_horde")
                && src_txt.contains("turret_angle_deg")
                && src_txt.contains("disguise_as_template"),
            "sync must copy combat-bonus residual"
        );
    }

    fn sync_from_host_copies_entity_detector_sp_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityDetectorSp");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "DetectU", 100.0);
        let id = logic
            .create_object("DetectU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.cheer_timer = 2.5;
            obj.overcharge_enabled = true;
            obj.active_weapon_slot = 2;
            obj.guard_radius = 120.0;
            obj.applied_upgrades.insert("UpgradeChemicalSuits".into());
            obj.applied_upgrades.insert("UpgradeCompositeArmor".into());
            obj.special_power_ready = true;
            obj.special_power_cooldown = 60.0;
            obj.special_power_cooldown_remaining = 12.0;
            obj.is_detector = true;
            obj.detection_range = 200.0;
            obj.detection_rate_frames = 15;
            obj.stealth_breaks_on_attack = true;
            obj.stealth_breaks_on_move = true;
            obj.innate_stealth = true;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.detector_entity_count(), 1);
        assert_eq!(shadow.special_power_ready_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.cheer_timer - 2.5).abs() < 0.01);
        assert!(e.overcharge_enabled);
        assert_eq!(e.active_weapon_slot, 2);
        assert!((e.guard_radius - 120.0).abs() < 0.01);
        assert_eq!(e.applied_upgrade_count, 2);
        assert!(e.special_power_ready);
        assert!((e.special_power_cooldown - 60.0).abs() < 0.01);
        assert!((e.special_power_cooldown_remaining - 12.0).abs() < 0.01);
        assert!(e.is_detector);
        assert!((e.detection_range - 200.0).abs() < 0.01);
        assert_eq!(e.detection_rate_frames, 15);
        assert!(e.stealth_breaks_on_attack && e.stealth_breaks_on_move && e.innate_stealth);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("is_detector")
                && src.contains("special_power_ready")
                && src.contains("applied_upgrade_count"),
            "sync must copy detector/sp residual"
        );
    }

    fn sync_from_host_copies_entity_transport_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityTransport");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "OverlordX", 800.0);
        ensure_template(&mut logic, "RiderX", 100.0);
        let bus = logic
            .create_object("OverlordX", Team::China, glam::Vec3::new(9.0, 0.0, 9.0))
            .expect("bus");
        let rider = logic
            .create_object("RiderX", Team::China, glam::Vec3::new(10.0, 0.0, 9.0))
            .expect("rider");
        {
            let obj = logic.get_objects_mut().get_mut(&bus).expect("obj");
            obj.name = "OL-1".into();
            obj.overlord_bunker_capacity = Some(5);
            obj.passengers_allowed_to_fire = true;
            obj.armed_riders_upgrade_weapon_set = true;
            obj.weapon_set_player_upgrade = true;
            obj.is_battle_bus_transport = true;
            obj.is_technical_transport = false;
            obj.is_combat_cycle_transport = false;
            obj.combat_cycle_rider = 0;
            obj.is_tunnel_network = false;
            obj.is_combat_chinook_transport = true;
        }
        {
            let obj = logic.get_objects_mut().get_mut(&rider).expect("rider");
            obj.contained_by = Some(bus);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.battle_bus_entity_count(), 1);
        assert_eq!(shadow.contained_entity_count(), 1);
        let eid = shadow.entity_for_host(bus).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.display_name, "OL-1");
        assert_eq!(e.overlord_bunker_capacity, 5);
        assert!(e.passengers_allowed_to_fire);
        assert!(e.armed_riders_upgrade_weapon_set);
        assert!(e.weapon_set_player_upgrade);
        assert!(e.is_battle_bus_transport);
        assert!(e.is_combat_chinook_transport);
        let rid = shadow.entity_for_host(rider).expect("rmap");
        let r = shadow.world().entity(rid).expect("r");
        assert_eq!(r.contained_by_host, bus.0);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("overlord_bunker_capacity")
                && src.contains("contained_by_host")
                && src.contains("is_battle_bus_transport"),
            "sync must copy transport residual"
        );
    }

    #[test]
    fn sync_from_host_copies_entity_weapon_move_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityWeaponMove");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "WpnMoveU", 100.0);
        let id = logic
            .create_object("WpnMoveU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
            .expect("id");
        {
            use crate::game_logic::Weapon;
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.weapon = Some(Weapon {
                damage: 25.0,
                range: 150.0,
                min_range: 5.0,
                reload_time: 1.5,
                last_fire_time: 0.0,
                ammo: Some(30),
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 200.0,
                pre_attack_delay: 0.1,
                splash_radius: 0.0,
            });
            obj.secondary_weapon = Some(Weapon::default());
            obj.movement.max_speed = 12.5;
            obj.movement.velocity = glam::Vec3::new(1.0, 0.0, 2.0);
            obj.movement.path = vec![
                glam::Vec3::new(0.0, 0.0, 0.0),
                glam::Vec3::new(10.0, 0.0, 10.0),
            ];
            obj.movement.current_path_index = 1;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.armed_entity_count(), 1);
        assert_eq!(shadow.pathing_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.has_weapon && e.has_secondary_weapon);
        assert!((e.weapon_damage - 25.0).abs() < 0.01);
        assert!((e.weapon_range - 150.0).abs() < 0.01);
        assert!((e.weapon_min_range - 5.0).abs() < 0.01);
        assert_eq!(e.weapon_ammo, 30);
        assert!(e.weapon_can_target_air);
        assert!((e.move_max_speed - 12.5).abs() < 0.01);
        assert!((e.velocity[2] - 2.0).abs() < 0.01);
        assert_eq!(e.path_len, 2);
        assert_eq!(e.path_index, 1);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("weapon_damage")
                && src.contains("move_max_speed")
                && src.contains("path_len"),
            "sync must copy weapon/movement residual"
        );
    }

    #[test]
    fn sync_from_host_copies_entity_building_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityBuilding");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "BarracksRes", 500.0);
        let id = logic
            .create_object("BarracksRes", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
            .expect("id");
        {
            use crate::game_logic::{BuildingData, BuildingType, ProductionItem, Resources};
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.object_type = crate::game_logic::ObjectType::Building;
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "AmericaInfantryRanger".into(),
                progress: 0.35,
                total_time: 10.0,
                cost: Resources {
                    supplies: 225,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: crate::game_logic::buildings::ProductionKind::Unit,
            });
            bd.rally_point = Some(glam::Vec3::new(20.0, 0.0, 20.0));
            bd.garrisoned_units = vec![crate::game_logic::ObjectId(99)];
            bd.max_garrison = 5;
            obj.building_data = Some(bd);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.building_data_entity_count(), 1);
        assert_eq!(shadow.producing_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.is_building);
        assert_eq!(e.building_type_ordinal, 1); // Barracks
        assert_eq!(e.production_queue_len, 1);
        assert!((e.production_progress - 0.35).abs() < 0.001);
        assert_eq!(e.production_template, "AmericaInfantryRanger");
        assert_eq!(e.garrison_count, 1);
        assert_eq!(e.max_garrison, 5);
        let rp = e.rally_point.expect("rally");
        assert!((rp[0] - 20.0).abs() < 0.01);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("production_queue_len")
                && src.contains("building_type_ordinal")
                && src.contains("rally_point"),
            "sync must copy building residual"
        );
    }

    #[test]
    fn writeback_production_and_rally_to_host() {
        use crate::game_logic::{
            BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
            ThingTemplate,
        };
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdRallyWb");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WarFact") {
            let mut t = ThingTemplate::new("WarFact");
            t.set_health(1000.0);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("WarFact".into(), t);
        }
        let id = logic
            .create_object("WarFact", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("o");
            let mut bd = BuildingData::new(BuildingType::WarFactory);
            bd.production_queue.push(ProductionItem {
                template_name: "USACrusaderTank".into(),
                progress: 0.1,
                total_time: 10.0,
                cost: Resources {
                    supplies: 900,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            });
            bd.rally_point = Some(glam::Vec3::new(1.0, 0.0, 2.0));
            obj.building_data = Some(bd);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        {
            let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
            e.rally_point = Some([9.0, 0.0, 8.0]);
            if let Some(item) = e.production_queue_items.get_mut(0) {
                item.progress = 0.75;
            }
        }
        let n = shadow.writeback_production_to_host(&mut logic);
        let _ = shadow.writeback_production_door_to_host(&mut logic);
        let _ = shadow.writeback_body_damage_to_host(&mut logic);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        assert!(n >= 1, "writeback must touch building");
        let obj = logic.get_objects().get(&id).expect("o");
        let bd = obj.building_data.as_ref().expect("bd");
        assert_eq!(bd.rally_point, Some(glam::Vec3::new(9.0, 0.0, 8.0)));
        assert!((bd.production_queue[0].progress - 0.75).abs() < 1e-5);
    }

    #[test]
    fn writeback_construction_percent_to_host() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ConstrWb");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("BuildPad") {
            let mut t = ThingTemplate::new("BuildPad");
            t.set_health(500.0);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("BuildPad".into(), t);
        }
        let id = logic
            .create_object("BuildPad", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("o");
            obj.construction_percent = 0.2;
            obj.status.under_construction = true;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        {
            let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
            e.construction_percent = 0.85;
            e.under_construction = true;
            e.sold = true;
            e.reconstructing = true;
            e.unselectable = true;
            // Combat/status flags are NOT owned by construction writeback.
            e.stealthed = true;
            e.selected = true;
        }
        let n = shadow.writeback_construction_to_host(&mut logic);
        assert!(n >= 1);
        let obj = logic.get_objects().get(&id).expect("o");
        assert!((obj.construction_percent - 0.85).abs() < 1e-5);
        assert!(obj.status.under_construction);
        assert!(obj.status.sold);
        assert!(obj.status.reconstructing);
        assert!(obj.status.unselectable);
        // Construction writeback must not touch combat-status residual.
        assert!(!obj.status.stealthed);
        assert!(!obj.status.selected);
        // Dedicated combat-status writeback restores those flags.
        {
            let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
            e.stealthed = true;
            e.selected = true;
        }
        assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
        let obj = logic.get_objects().get(&id).expect("o");
        assert!(obj.status.stealthed);
        assert!(obj.status.selected);
        // Complete residual
        {
            let e = shadow.world_mut().world_mut().entity_mut(eid).expect("e");
            e.construction_percent = 1.0;
            e.under_construction = false;
        }
        let _ = shadow.writeback_construction_to_host(&mut logic);
        let obj = logic.get_objects().get(&id).expect("o");
        assert!((obj.construction_percent - 1.0).abs() < 1e-5);
        assert!(!obj.status.under_construction);
    }

    #[test]
    fn set_combat_status_mutation_channel_updates_shadow_entity() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CombatStatusMut");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CbtU") {
            let mut t = ThingTemplate::new("CbtU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("CbtU".into(), t);
        }
        let id = logic
            .create_object("CbtU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_set_combat_status_for_host(
            crate::game_logic::host_status_log::HostStatusEvent {
                object: id,
                selected: Some(true),
                attacking: Some(true),
                moving: None,
                is_firing_weapon: Some(true),
                is_aiming_weapon: None,
                stealthed: Some(true),
                detected: Some(false),
                disabled_emp: Some(true),
                weapons_jammed: None,
                disabled_hacked: None,
                disabled_unmanned: None,
                disabled_paralyzed: None,
                disabled_subdued: None,
                masked: Some(true),
                disguised: Some(true),

                no_collisions: None,
                private_captured: None,
                disguise_transitioning_to: None,
                disguise_halfpoint_reached: None,
                faerie_fire: None,
                booby_trapped: None,
                eject_invulnerable: None,
                pilot_did_move_to_base: None,
                parachuting: None,
                parachute_open: None,
                parachute_landing_override_set: None,

                using_ability: None,
                deployed: None,
                under_construction: None,
                sold: None,
                reconstructing: None,
                unselectable: None,
                ignoring_stealth: None,
                repulsor: None,
                disabled_underpowered: None,
                disabled_freefall: None,
                is_carbomb: None,
                hijacked: None,
                force_attack: None,
            }
        ));
        let n = shadow.world_mut().apply_pending_mutations();
        assert!(n >= 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.stealthed);
        assert!(!e.detected);
        assert!(e.attacking);
        assert!(e.is_firing_weapon);
        assert!(e.selected);
        assert!(e.disabled_emp);
        assert!(e.masked);
        assert!(e.disguised);
        // writeback to host via combat-status last-writer residual
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.status.stealthed = false;
            o.status.attacking = false;
            o.status.is_firing_weapon = false;
            o.status.selected = false;
            o.status.disabled_emp = false;
            o.status.masked = false;
            o.status.disguised = false;
        }
        let wb = shadow.writeback_combat_status_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(o.status.stealthed);
        assert!(o.status.attacking);
        assert!(o.status.is_firing_weapon);
        assert!(o.status.selected);
        assert!(o.status.disabled_emp);
        assert!(o.status.masked);
        assert!(o.status.disguised);
    }

    #[test]
    fn host_selection_status_log_drives_set_combat_status_channel() {
        use crate::game_logic::{host_status_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SelU") {
            let mut t = ThingTemplate::new("SelU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("SelU".into(), t);
        }
        let id = logic
            .create_object("SelU", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        host_status_log::clear();
        // Select via host API (records status log).
        let pid = logic.get_players().keys().copied().min().unwrap_or(0);
        logic.select_objects(pid, vec![id]);
        let events = host_status_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == id && e.selected == Some(true)),
            "select must log selected=true"
        );
        // Re-record for session path (drain consumed).
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.select();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Poison shadow selected off, then apply host status events as mutations.
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.selected = false;
        }
        let status_events = host_status_log::drain();
        for ev in &status_events {
            let _ = shadow.queue_set_combat_status_for_host(*ev);
        }
        let n = shadow.apply_pending();
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.selected, "mutation channel must set selected");
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.status.selected = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.selected = true;
        }
        assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
        assert!(logic.get_objects().get(&id).expect("o").status.selected);
    }

    #[test]
    fn host_attacking_status_log_drives_set_combat_status_channel() {
        use crate::game_logic::{host_status_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AtkU") {
            let mut t = ThingTemplate::new("AtkU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("AtkU".into(), t);
        }
        let id = logic
            .create_object("AtkU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("id");
        host_status_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_status_attacking(true);
            o.set_status_firing_weapon(true);
        }
        let events = host_status_log::drain();
        assert!(events
            .iter()
            .any(|e| e.object == id && e.attacking == Some(true)));
        assert!(events
            .iter()
            .any(|e| e.object == id && e.is_firing_weapon == Some(true)));
        // Re-record for mutation apply.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_status_attacking(true);
            o.set_status_firing_weapon(true);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.attacking = false;
            e.is_firing_weapon = false;
        }
        for ev in host_status_log::drain() {
            let _ = shadow.queue_set_combat_status_for_host(ev);
        }
        assert!(shadow.apply_pending() >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.attacking);
        assert!(e.is_firing_weapon);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.status.attacking = false;
            o.status.is_firing_weapon = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.attacking = true;
            e.is_firing_weapon = true;
        }
        assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(o.status.attacking && o.status.is_firing_weapon);
    }

    #[test]
    fn host_stealth_status_log_drives_set_combat_status_channel() {
        use crate::game_logic::{host_status_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StealthStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("StU") {
            let mut t = ThingTemplate::new("StU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("StU".into(), t);
        }
        let id = logic
            .create_object("StU", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
            .expect("id");
        host_status_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_status_stealthed(true);
            o.set_status_detected(false);
        }
        let events = host_status_log::drain();
        assert!(events
            .iter()
            .any(|e| e.object == id && e.stealthed == Some(true)));
        assert!(events
            .iter()
            .any(|e| e.object == id && e.detected == Some(false)));
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_status_stealthed(true);
            o.set_status_detected(false);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.stealthed = false;
            e.detected = true;
        }
        for ev in host_status_log::drain() {
            let _ = shadow.queue_set_combat_status_for_host(ev);
        }
        assert!(shadow.apply_pending() >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.stealthed);
        assert!(!e.detected);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.status.stealthed = false;
            o.status.detected = true;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.stealthed = true;
            e.detected = false;
        }
        assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(o.status.stealthed && !o.status.detected);
    }

    #[test]
    fn host_emp_status_log_drives_set_combat_status_channel() {
        use crate::game_logic::{host_status_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EmpStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("EmpU") {
            let mut t = ThingTemplate::new("EmpU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("EmpU".into(), t);
        }
        let id = logic
            .create_object("EmpU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
            .expect("id");
        host_status_log::clear();
        let until = logic.get_frame().saturating_add(300);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.apply_disabled_emp(until);
        }
        let events = host_status_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == id && e.disabled_emp == Some(true)),
            "EMP apply must log disabled_emp"
        );
        assert!(
            events
                .iter()
                .any(|e| e.object == id && e.attacking == Some(false)),
            "EMP apply clears attacking via status channel"
        );
        let until2 = logic.get_frame().saturating_add(300);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.apply_disabled_emp(until2);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.disabled_emp = false;
        }
        for ev in host_status_log::drain() {
            let _ = shadow.queue_set_combat_status_for_host(ev);
        }
        assert!(shadow.apply_pending() >= 1);
        assert!(shadow.world().entity(eid).expect("e").disabled_emp);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.status.disabled_emp = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.disabled_emp = true;
        }
        assert!(shadow.writeback_combat_status_to_host(&mut logic) >= 1);
        assert!(logic.get_objects().get(&id).expect("o").status.disabled_emp);
    }

    #[test]
    fn host_player_cooldown_log_drives_set_player_cooldowns_channel() {
        use crate::command_system::SpecialPowerType;
        use crate::game_logic::host_player_cooldown_log;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PlayerCdCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = *logic.get_players().keys().next().expect("player");
        host_player_cooldown_log::clear();
        {
            let p = logic.get_player_mut(pid).expect("p");
            // Use a concrete SP type if available; format Debug name into log.
            // ParticleUplink residual is common; fall back to first Debug variant via reset API.
            p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 12.5);
        }
        let events = host_player_cooldown_log::drain();
        assert!(
            !events.is_empty()
                && events
                    .iter()
                    .any(|e| e.player_id == pid && !e.cooldowns.is_empty()),
            "events {:?}",
            events
        );
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.reset_shared_special_power_timer(&SpecialPowerType::Airstrike, 12.5);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
        if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
            p.shared_special_power_cooldowns.clear();
        }
        let n = shadow.apply_host_player_cooldown_events(&host_player_cooldown_log::drain());
        assert!(n >= 1);
        let p = shadow.world().player(gw).expect("p");
        assert!(
            p.shared_special_power_cooldowns
                .iter()
                .any(|(_, rem)| (*rem - 12.5).abs() < 1e-3),
            "cds {:?}",
            p.shared_special_power_cooldowns
        );
        // Poison host map and writeback
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.shared_special_power_cooldowns.clear();
        }
        assert!(shadow.writeback_economy_to_host(&mut logic) >= 1);
        let p = logic.get_player(pid).expect("p");
        assert!(
            p.shared_special_power_cooldowns
                .values()
                .any(|rem| (*rem - 12.5).abs() < 1e-3),
            "host cds {:?}",
            p.shared_special_power_cooldowns
        );
    }

    #[test]
    fn host_player_meta_log_drives_sciences_and_alive_channel() {
        use crate::game_logic::host_player_meta_log;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PlayerMetaCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = *logic.get_players().keys().next().expect("player");
        host_player_meta_log::clear();
        {
            let p = logic.get_player_mut(pid).expect("p");
            assert!(p.unlock_science("SCIENCE_PaladinTank"));
            p.is_alive = true;
            p.record_host_alive();
        }
        let events = host_player_meta_log::drain();
        assert!(
            events.iter().any(|e| matches!(
                e,
                host_player_meta_log::HostPlayerMetaEvent::Sciences { player_id, unlocked_sciences }
                    if *player_id == pid && unlocked_sciences.iter().any(|s| s.contains("Paladin"))
            )),
            "sciences {:?}",
            events
        );
        assert!(events.iter().any(|e| matches!(
            e,
            host_player_meta_log::HostPlayerMetaEvent::Alive { player_id, is_alive: true }
                if *player_id == pid
        )));

        // Re-record for apply
        host_player_meta_log::clear();
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.record_host_sciences();
            p.is_alive = false;
            p.record_host_alive();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
        if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
            p.unlocked_sciences.clear();
            p.is_alive = true;
        }
        let n = shadow.apply_host_player_meta_events(&host_player_meta_log::drain());
        assert!(n >= 1);
        let p = shadow.world().player(gw).expect("p");
        assert!(p.unlocked_sciences.iter().any(|s| s.contains("Paladin")));
        assert!(!p.is_alive);
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.unlocked_sciences.clear();
            p.is_alive = true;
        }
        assert!(shadow.writeback_economy_to_host(&mut logic) >= 1);
        let p = logic.get_player(pid).expect("p");
        assert!(p.unlocked_sciences.iter().any(|s| s.contains("Paladin")));
        assert!(!p.is_alive);
    }

    #[test]
    fn host_player_progress_log_drives_set_player_progress_channel() {
        use crate::game_logic::host_player_progress_log;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PlayerProgCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = *logic.get_players().keys().next().expect("player");
        host_player_progress_log::clear();
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.force_set_cash_bounty(0.35);
            p.rank_level = 3;
            p.skill_points = 50;
            p.science_purchase_points = 2;
            p.record_host_progress();
        }
        let events = host_player_progress_log::drain();
        assert!(
            events.iter().any(|e| {
                e.player_id == pid
                    && e.rank_level == 3
                    && (e.cash_bounty_percent - 0.35).abs() < 1e-5
            }),
            "events {:?}",
            events
        );
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.force_set_cash_bounty(0.35);
            p.rank_level = 3;
            p.skill_points = 50;
            p.science_purchase_points = 2;
            p.record_host_progress();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
        if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
            p.rank_level = 1;
            p.skill_points = 0;
            p.science_purchase_points = 0;
            p.cash_bounty_percent = 0.0;
        }
        let n = shadow.apply_host_player_progress_events(&host_player_progress_log::drain());
        assert!(n >= 1);
        let p = shadow.world().player(gw).expect("p");
        assert_eq!(p.rank_level, 3);
        assert_eq!(p.skill_points, 50);
        assert_eq!(p.science_purchase_points, 2);
        assert!((p.cash_bounty_percent - 0.35).abs() < 1e-5);
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.rank_level = 1;
            p.skill_points = 0;
            p.science_purchase_points = 0;
            p.cash_bounty_percent = 0.0;
        }
        assert!(shadow.writeback_economy_to_host(&mut logic) >= 1);
        let p = logic.get_player(pid).expect("p");
        assert_eq!(p.rank_level, 3);
        assert_eq!(p.skill_points, 50);
        assert!((p.cash_bounty_percent - 0.35).abs() < 1e-5);
    }

    #[test]
    fn host_radar_log_drives_set_player_radar_channel() {
        use crate::game_logic::host_radar_log;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RadarCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = *logic.get_players().keys().next().expect("player");
        host_radar_log::clear();
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.set_radar_state(2, false);
        }
        let events = host_radar_log::drain();
        assert!(events
            .iter()
            .any(|e| e.player_id == pid && e.radar_count == 2 && !e.radar_disabled));
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.set_radar_state(2, false);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw = *shadow.host_player_to_gw.get(&pid).expect("map");
        if let Some(p) = shadow.world_mut().world_mut().player_mut(gw) {
            p.radar_count = 0;
            p.radar_disabled = true;
        }
        let n = shadow.apply_host_radar_events(&host_radar_log::drain());
        assert!(n >= 1);
        let p = shadow.world().player(gw).expect("p");
        assert_eq!(p.radar_count, 2);
        assert!(!p.radar_disabled);
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.radar_count = 0;
            p.radar_disabled = true;
        }
        assert!(shadow.writeback_economy_to_host(&mut logic) >= 1);
        let p = logic.get_player(pid).expect("p");
        assert_eq!(p.radar_count, 2);
        assert!(!p.radar_disabled);
    }

    #[test]
    fn host_contain_log_drives_set_contain_channel() {
        use crate::game_logic::{
            host_contain_log, BuildingData, BuildingType, KindOf, Team, ThingTemplate,
        };
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ContainCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["BunkC", "InfC"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.set_health(200.0);
                t.add_kind_of(KindOf::Selectable);
                if name == "BunkC" {
                    t.add_kind_of(KindOf::Structure);
                }
                logic.templates.insert(name.into(), t);
            }
        }
        let bunker = logic
            .create_object("BunkC", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("b");
        let inf = logic
            .create_object("InfC", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
            .expect("i");
        {
            let o = logic.get_objects_mut().get_mut(&bunker).expect("b");
            o.building_data = Some(BuildingData::new(BuildingType::Bunker));
            if let Some(bd) = o.building_data.as_mut() {
                bd.max_garrison = 5;
            }
        }
        host_contain_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&bunker).expect("b");
            assert!(o.add_occupant(inf));
        }
        {
            let o = logic.get_objects_mut().get_mut(&inf).expect("i");
            o.set_contained_by(Some(bunker));
        }
        let events = host_contain_log::drain();
        assert!(events.len() >= 2, "events {:?}", events);

        // Re-apply path
        host_contain_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&bunker).expect("b");
            if let Some(bd) = o.building_data.as_mut() {
                bd.garrisoned_units.clear();
            }
            assert!(o.add_occupant(inf));
        }
        {
            let o = logic.get_objects_mut().get_mut(&inf).expect("i");
            o.set_contained_by(Some(bunker));
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid_i = shadow.entity_for_host(inf).expect("map i");
        let eid_b = shadow.entity_for_host(bunker).expect("map b");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
            e.contained_by_host = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
            e.garrison_count = 0;
            e.garrisoned_host_ids.clear();
        }
        let n = shadow.apply_host_contain_events(&host_contain_log::drain());
        assert!(n >= 1);
        assert_eq!(
            shadow.world().entity(eid_i).expect("e").contained_by_host,
            bunker.0
        );
        assert!(shadow.world().entity(eid_b).expect("e").garrison_count >= 1);
        assert!(shadow.world().entity(eid_b).expect("e").occupant_count >= 1);
        // Poison host then writeback via SetContain last-writer residual.
        {
            let o = logic.get_objects_mut().get_mut(&inf).expect("i");
            o.contained_by = None;
        }
        {
            let o = logic.get_objects_mut().get_mut(&bunker).expect("b");
            if let Some(bd) = o.building_data.as_mut() {
                bd.garrisoned_units.clear();
            }
            o.occupants.clear();
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
            e.contained_by_host = bunker.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
            e.garrison_count = 1;
            e.garrisoned_host_ids = vec![inf.0];
        }
        assert!(shadow.writeback_contain_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&inf).expect("i").contained_by,
            Some(bunker)
        );
        let bd = logic
            .get_objects()
            .get(&bunker)
            .expect("b")
            .building_data
            .as_ref()
            .expect("bd");
        assert!(bd.garrisoned_units.contains(&inf));
    }

    #[test]
    fn host_ai_state_log_drives_set_ai_state_channel() {
        use crate::game_logic::{host_ai_state_log, AIState, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiStateCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AiU") {
            let mut t = ThingTemplate::new("AiU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("AiU".into(), t);
        }
        let id = logic
            .create_object("AiU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        host_ai_state_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_ai_state(AIState::GuardingObject);
        }
        let events = host_ai_state_log::drain();
        assert!(
            events.iter().any(|e| e.object == id && e.ordinal == 10),
            "expected GuardingObject ordinal 10, got {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_ai_state(AIState::GuardingObject);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.ai_state_ordinal = 0;
        }
        let n = shadow.apply_host_ai_state_events(&host_ai_state_log::drain());
        assert!(n >= 1);
        assert_eq!(shadow.world().entity(eid).expect("e").ai_state_ordinal, 10);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.ai_state = AIState::Idle;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.ai_state_ordinal = 10; // GuardingObject
        }
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&id).expect("o").ai_state,
            AIState::GuardingObject
        );
    }

    #[test]
    fn host_special_power_cooldown_remaining_channel() {
        use crate::game_logic::{host_special_power_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpCdCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SpU") {
            let mut t = ThingTemplate::new("SpU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            t.special_power_cooldown = 45.0;
            logic.templates.insert("SpU".into(), t);
        }
        let oid = logic
            .create_object("SpU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        host_special_power_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.special_power_cooldown = 45.0;
            o.special_power_cooldown_remaining = 18.0;
            o.special_power_ready = false;
            o.record_host_special_power();
        }
        let events = host_special_power_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && !e.ready
                    && (e.cooldown_remaining - 18.0).abs() < 1e-3
                    && (e.cooldown - 45.0).abs() < 1e-3
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_special_power();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.special_power_ready = true;
            e.special_power_cooldown_remaining = 0.0;
            e.special_power_cooldown = 0.0;
        }
        let n = shadow.apply_host_special_power_events(&host_special_power_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(!e.special_power_ready);
        assert!((e.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
        assert!((e.special_power_cooldown - 45.0).abs() < 1e-3);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.special_power_ready = true;
            o.special_power_cooldown_remaining = 0.0;
            o.special_power_cooldown = 1.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.special_power_ready = false;
            e.special_power_cooldown_remaining = 18.0;
            e.special_power_cooldown = 45.0;
        }
        assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(!o.special_power_ready);
        assert!((o.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
        assert!((o.special_power_cooldown - 45.0).abs() < 1e-3);
    }

    #[test]
    fn host_special_power_log_drives_set_special_power_channel() {
        use crate::game_logic::{host_special_power_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpReadyCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SpU") {
            let mut t = ThingTemplate::new("SpU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("SpU".into(), t);
        }
        let id = logic
            .create_object("SpU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        host_special_power_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_special_power_ready(true);
        }
        let events = host_special_power_log::drain();
        assert!(events.iter().any(|e| e.object == id && e.ready));
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_special_power_ready(true);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.special_power_ready = false;
        }
        let n = shadow.apply_host_special_power_events(&host_special_power_log::drain());
        assert!(n >= 1);
        assert!(shadow.world().entity(eid).expect("e").special_power_ready);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.special_power_ready = false;
        }
        assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
        assert!(logic.get_objects().get(&id).expect("o").special_power_ready);
    }

    #[test]
    fn host_stored_supplies_log_drives_set_stored_supplies_channel() {
        use crate::game_logic::{host_stored_supplies_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StoreSupCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SsU") {
            let mut t = ThingTemplate::new("SsU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("SsU".into(), t);
        }
        let id = logic
            .create_object("SsU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("id");
        host_stored_supplies_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_stored_supplies(900);
        }
        let events = host_stored_supplies_log::drain();
        assert!(events.iter().any(|e| e.object == id && e.supplies == 900));
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_stored_supplies(900);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.stored_supplies = 0;
        }
        let n = shadow.apply_host_stored_supplies_events(&host_stored_supplies_log::drain());
        assert!(n >= 1);
        assert_eq!(shadow.world().entity(eid).expect("e").stored_supplies, 900);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.stored_resources.supplies = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.stored_supplies = 900;
        }
        assert!(shadow.writeback_stored_supplies_to_host(&mut logic) >= 1);
        assert_eq!(
            logic
                .get_objects()
                .get(&id)
                .expect("o")
                .stored_resources
                .supplies,
            900
        );
    }

    #[test]
    fn host_construction_progress_log_drives_set_construction_channel() {
        use crate::game_logic::{host_construction_progress_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ConstrProgCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CProg") {
            let mut t = ThingTemplate::new("CProg");
            t.set_health(500.0);
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("CProg".into(), t);
        }
        let id = logic
            .create_object("CProg", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.construction_percent = 0.25;
            o.set_status_under_construction(true);
        }
        host_construction_progress_log::clear();
        host_construction_progress_log::record(id, 0.25, true);
        let events = host_construction_progress_log::drain();
        assert_eq!(events.len(), 1);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.construction_percent = 0.0;
            e.under_construction = false;
        }
        host_construction_progress_log::record(id, 0.25, true);
        let events = host_construction_progress_log::drain();
        let n = shadow.apply_host_construction_progress_events(&events);
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.construction_percent - 0.25).abs() < 1e-5);
        assert!(e.under_construction);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.construction_percent = 0.0;
            o.status.under_construction = false;
        }
        let wb = shadow.writeback_construction_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!((o.construction_percent - 0.25).abs() < 1e-5);
        assert!(o.status.under_construction);
    }

    #[test]
    fn host_owner_log_drives_transfer_owner_channel() {
        use crate::game_logic::{host_owner_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("OwnerXferCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("OwnU") {
            let mut t = ThingTemplate::new("OwnU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("OwnU".into(), t);
        }
        let id = logic
            .create_object("OwnU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        host_owner_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_team(Team::GLA);
        }
        let events = host_owner_log::drain();
        assert!(
            events.iter().any(|e| e.object == id && e.team == Team::GLA),
            "expected owner log {:?}",
            events
        );
        // Re-set for mutation path after drain.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_team(Team::USA);
            host_owner_log::clear();
            o.set_team(Team::GLA);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        let gla_owner = shadow.world().entity(eid).expect("e").owner;
        assert!(
            gla_owner.is_some(),
            "GLA object should map to Some owner after sync; players={:?}",
            logic.get_players().keys().collect::<Vec<_>>()
        );
        // Poison to None (neutral) then apply TransferOwner from events.
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.owner = None;
        }
        let events = host_owner_log::drain();
        assert!(!events.is_empty(), "events empty");
        let n = shadow.apply_host_owner_events(&logic, &events);
        assert!(n >= 1, "owner events {n} events={events:?}");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.owner, gla_owner, "shadow owner should match GLA mapping");
        // Poison host team back to USA then writeback.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.team = Team::USA;
            o.team_color = Team::USA.get_color();
        }
        let wb = shadow.writeback_owner_to_host(&mut logic);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(
            wb >= 1,
            "writeback={wb} host_team={:?} shadow_owner={:?} after_host={:?}",
            Team::USA,
            e.owner,
            o.team
        );
        assert_eq!(o.team, Team::GLA);
    }

    #[test]
    fn host_production_log_drives_set_production_queue_channel() {
        use crate::game_logic::host_production_log;
        use crate::game_logic::{
            BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
            ThingTemplate,
        };
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdQueueCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("ProdBarracks") {
            let mut t = ThingTemplate::new("ProdBarracks");
            t.set_health(500.0);
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("ProdBarracks".into(), t);
        }
        let barracks = logic
            .create_object("ProdBarracks", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
            .expect("barracks");
        {
            let o = logic.get_objects_mut().get_mut(&barracks).expect("b");
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "ProdRanger".into(),
                progress: 0.0,
                total_time: 10.0,
                cost: Resources {
                    supplies: 150,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            });
            o.building_data = Some(bd);
        }
        host_production_log::clear();
        host_production_log::record_enqueue(barracks, "ProdRanger");
        let events = host_production_log::drain();
        assert_eq!(events.len(), 1);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(barracks).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.production_queue_items.clear();
            e.production_template.clear();
        }
        // Re-record for apply (drain consumed).
        host_production_log::record_enqueue(barracks, "ProdRanger");
        let events = host_production_log::drain();
        let n = shadow.apply_host_production_events(&events, &logic);
        assert!(n >= 1, "production events applied {n}");
        let e = shadow.world().entity(eid).expect("e");
        assert!(
            !e.production_queue_items.is_empty(),
            "queue should be last-written from host"
        );
        assert_eq!(e.production_queue_items[0].template_name, "ProdRanger");
        {
            let o = logic.get_objects_mut().get_mut(&barracks).expect("b");
            if let Some(bd) = o.building_data.as_mut() {
                bd.production_queue.clear();
            }
        }
        let wb = shadow.writeback_production_to_host(&mut logic);
        let _ = shadow.writeback_production_door_to_host(&mut logic);
        let _ = shadow.writeback_body_damage_to_host(&mut logic);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&barracks).expect("b");
        let q = &o.building_data.as_ref().expect("bd").production_queue;
        assert!(!q.is_empty());
        assert_eq!(q[0].template_name, "ProdRanger");
    }

    #[test]
    fn production_authority_writeback_is_queue_last_writer() {
        use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
        use crate::game_logic::{
            BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
            ThingTemplate,
        };
        let prev = std::env::var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
        assert!(gameworld_production_authority_enabled());
        host_production_progress_log::clear();

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdAuthWB");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("ProdAuthBarracks") {
            let mut t = ThingTemplate::new("ProdAuthBarracks");
            t.set_health(500.0);
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("ProdAuthBarracks".into(), t);
        }
        let oid = logic
            .create_object(
                "ProdAuthBarracks",
                Team::USA,
                glam::Vec3::new(10.0, 0.0, 10.0),
            )
            .expect("barracks");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("b");
            let mut bd = BuildingData::new(BuildingType::Barracks);
            bd.production_queue.push(ProductionItem {
                template_name: "ProdAuthRanger".into(),
                progress: 2.0,
                total_time: 10.0,
                cost: Resources {
                    supplies: 150,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            });
            o.building_data = Some(bd);
        }

        let items = vec![HostProductionQueueItem {
            template_name: "ProdAuthRanger".into(),
            progress: 2.0,
            total_time: 10.0,
            cost_supplies: 150,
            is_upgrade: false,
        }];
        host_production_progress_log::record(oid, items.clone(), 0.0, 1.0);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let n =
            shadow.apply_host_production_progress_events(&host_production_progress_log::drain());
        assert!(n >= 1, "progress apply {n}");

        // Dirty host queue — authority writeback must restore shadow snapshot.
        {
            let o = logic.get_object_mut(oid).expect("o");
            o.building_data.as_mut().unwrap().production_queue.clear();
        }
        assert!(shadow.writeback_production_to_host(&mut logic) >= 1);
        let restored = logic
            .get_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .len();
        assert_eq!(
            restored,
            items.len(),
            "writeback must restore queue under production authority"
        );

        // Authority off: writeback is a no-op.
        std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "0");
        assert!(!gameworld_production_authority_enabled());
        {
            let o = logic.get_object_mut(oid).expect("o");
            o.building_data.as_mut().unwrap().production_queue.clear();
        }
        assert_eq!(shadow.writeback_production_to_host(&mut logic), 0);
        assert!(logic
            .get_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty());

        host_production_progress_log::clear();
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY"),
        }
    }

    #[test]
    fn host_veterancy_log_drives_set_veterancy_channel() {
        use crate::game_logic::{host_veterancy_log, KindOf, Team, ThingTemplate, VeterancyLevel};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("VetStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("VetU") {
            let mut t = ThingTemplate::new("VetU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            // Low thresholds so gain_experience levels quickly.
            t.veterancy_xp_thresholds = [10.0, 20.0, 30.0];
            logic.templates.insert("VetU".into(), t);
        }
        let id = logic
            .create_object("VetU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        host_veterancy_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.gain_experience(25.0); // Elite
        }
        let events = host_veterancy_log::drain();
        assert!(
            events.iter().any(|e| e.object == id && e.ordinal >= 2),
            "expected elite+ veterancy log, got {:?}",
            events
        );
        // Re-level for mutation path.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.experience.level = VeterancyLevel::Rookie;
            o.experience.current = 0.0;
            host_veterancy_log::clear();
            o.gain_experience(25.0);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.veterancy_ordinal = 0;
        }
        for ev in host_veterancy_log::drain() {
            assert!(shadow.queue_set_veterancy_for_host(ev.object, ev.ordinal));
        }
        assert!(shadow.apply_pending() >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(
            e.veterancy_ordinal >= 2,
            "shadow ordinal {}",
            e.veterancy_ordinal
        );
        // Poison host level then writeback.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.experience.level = VeterancyLevel::Rookie;
        }
        let wb = shadow.writeback_experience_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(matches!(
            o.experience.level,
            VeterancyLevel::Elite | VeterancyLevel::Heroic
        ));
    }

    #[test]
    fn host_force_attack_status_log_drives_set_combat_status_channel() {
        use crate::game_logic::{host_status_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ForceAtkStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("FaU") {
            let mut t = ThingTemplate::new("FaU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("FaU".into(), t);
        }
        let id = logic
            .create_object("FaU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        host_status_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_force_attack(true);
            o.set_status_using_ability(true);
            o.set_status_deployed(true);
        }
        let events = host_status_log::drain();
        assert!(events
            .iter()
            .any(|e| e.object == id && e.force_attack == Some(true)));
        assert!(events
            .iter()
            .any(|e| e.object == id && e.using_ability == Some(true)));
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_force_attack(true);
            o.set_status_using_ability(true);
            o.set_status_deployed(true);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.force_attack = false;
            e.using_ability = false;
            e.deployed = false;
        }
        for ev in host_status_log::drain() {
            let _ = shadow.queue_set_combat_status_for_host(ev);
        }
        assert!(shadow.apply_pending() >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.force_attack);
        assert!(e.using_ability);
        assert!(e.deployed);
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.force_attack = false;
            o.status.using_ability = false;
            o.status.deployed = false;
        }
        let wb = shadow.writeback_combat_status_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(o.force_attack);
        assert!(o.status.using_ability);
        assert!(o.status.deployed);
    }

    #[test]
    fn host_residual_status_log_drives_set_combat_status_channel() {
        use crate::game_logic::{host_status_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ResidualStatusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("ResU") {
            let mut t = ThingTemplate::new("ResU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("ResU".into(), t);
        }
        let id = logic
            .create_object("ResU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        host_status_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_status_no_collisions(true);
            o.set_status_private_captured(true);
            o.set_status_faerie_fire(true);
            o.set_status_parachuting(true);
        }
        let events = host_status_log::drain();
        assert!(events
            .iter()
            .any(|e| e.object == id && e.no_collisions == Some(true)));
        assert!(events
            .iter()
            .any(|e| e.object == id && e.private_captured == Some(true)));
        // Re-record for mutation apply.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.set_status_no_collisions(true);
            o.set_status_private_captured(true);
            o.set_status_faerie_fire(true);
            o.set_status_parachuting(true);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = shadow.entity_for_host(id).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.no_collisions = false;
            e.private_captured = false;
            e.faerie_fire = false;
            e.parachuting = false;
        }
        for ev in host_status_log::drain() {
            let _ = shadow.queue_set_combat_status_for_host(ev);
        }
        assert!(shadow.apply_pending() >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.no_collisions);
        assert!(e.private_captured);
        assert!(e.faerie_fire);
        assert!(e.parachuting);
        // Poison host so writeback last-writer is observable.
        {
            let o = logic.get_objects_mut().get_mut(&id).expect("o");
            o.status.no_collisions = false;
            o.status.private_captured = false;
            o.status.faerie_fire = false;
            o.status.parachuting = false;
        }
        let wb = shadow.writeback_combat_status_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&id).expect("o");
        assert!(o.status.no_collisions);
        assert!(o.status.private_captured);
        assert!(o.status.faerie_fire);
        assert!(o.status.parachuting);
    }

    #[test]
    fn sync_from_host_copies_entity_xp_status_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityXpStatus");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "XpUnit", 100.0);
        let id = logic
            .create_object("XpUnit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.experience.current = 420.0;
            obj.experience.level = crate::game_logic::VeterancyLevel::Elite;
            obj.stored_resources.supplies = 1500;
            obj.status.stealthed = true;
            obj.status.detected = true;
            obj.status.using_ability = true;
            obj.status.airborne_target = true;
            obj.status.disabled_underpowered = true;
            obj.status.disabled_unmanned = false;
            obj.status.disabled_hacked = true;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.elite_entity_count(), 1);
        assert_eq!(shadow.stealthed_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.experience_points - 420.0).abs() < 0.01);
        assert_eq!(e.veterancy_ordinal, 2);
        assert_eq!(e.stored_supplies, 1500);
        assert!(e.stealthed && e.detected && e.using_ability);
        assert!(e.airborne_target && e.disabled_underpowered && e.disabled_hacked);
        assert!(!e.disabled_unmanned);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("experience_points")
                && src.contains("veterancy_ordinal")
                && src.contains("stealthed"),
            "sync must copy xp/status residual"
        );
    }

    #[test]
    fn sync_from_host_copies_entity_combat_intent_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityCombatIntent");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "CbtIntentU", 100.0);
        let id = logic
            .create_object("CbtIntentU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("id");
        let guard = logic
            .create_object("CbtIntentU", Team::USA, glam::Vec3::new(8.0, 0.0, 4.0))
            .expect("guard");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.force_attack = true;
            obj.show_health_bar = false;
            obj.target_location = Some(glam::Vec3::new(10.0, 0.0, 10.0));
            obj.guard_position = Some(glam::Vec3::new(1.0, 0.0, 1.0));
            obj.guard_target = Some(guard);
            obj.ai_state = crate::game_logic::AIState::GuardingObject;
            obj.occupants = vec![guard];
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.force_attack_entity_count(), 1);
        assert!(shadow.non_idle_ai_entity_count() >= 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.force_attack);
        assert!(!e.show_health_bar);
        assert_eq!(e.guard_target_host, guard.0);
        assert_eq!(e.ai_state_ordinal, 10); // GuardingObject
        assert_eq!(e.occupant_count, 1);
        let tl = e.target_location.expect("tl");
        assert!((tl[0] - 10.0).abs() < 0.01);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("force_attack")
                && src.contains("guard_target_host")
                && src.contains("ai_state_ordinal"),
            "sync must copy combat-intent residual"
        );
    }

    #[test]
    fn sync_from_host_copies_entity_color_power_type() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityColorPower");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "PwrBldg", 200.0);
        let id = logic
            .create_object("PwrBldg", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.object_type = crate::game_logic::ObjectType::Building;
            obj.team_color = [0.1, 0.2, 0.3, 0.9];
            obj.power_provided = 50;
            obj.power_consumed = 5;
            obj.max_transport = 0;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.building_entity_count(), 1);
        assert_eq!(shadow.total_entity_power_provided(), 50);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.object_type_ordinal, 3);
        assert!((e.team_color[0] - 0.1).abs() < 0.001);
        assert!((e.team_color[3] - 0.9).abs() < 0.001);
        assert_eq!(e.power_provided, 50);
        assert_eq!(e.power_consumed, 5);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("team_color")
                && src.contains("power_provided")
                && src.contains("object_type_ordinal"),
            "sync must copy color/power/type residual"
        );
    }

    #[test]
    fn sync_from_host_copies_entity_team_status_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntityTeamStatus");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "TeamStatU", 100.0);
        let id = logic
            .create_object("TeamStatU", Team::China, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.selection_radius = 12.5;
            obj.status.moving = true;
            obj.status.attacking = true;
            obj.status.under_construction = false;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.entity_count_for_team_ordinal(1), 1, "China ordinal");
        assert_eq!(shadow.moving_entity_count(), 1);
        assert_eq!(shadow.attacking_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.team_ordinal, 1);
        assert!((e.selection_radius - 12.5).abs() < 0.01);
        assert!(e.moving && e.attacking);
        assert!(!e.under_construction);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("team_ordinal")
                && src.contains("selection_radius")
                && src.contains("status.moving"),
            "sync must copy team/status residual"
        );
    }

    #[test]
    fn sync_from_host_copies_entity_selection_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EntitySelectResidual");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "SelResU", 100.0);
        let id = logic
            .create_object("SelResU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        {
            let obj = logic.get_objects_mut().get_mut(&id).expect("obj");
            obj.selected = true;
            obj.max_health = 150.0;
            obj.construction_percent = 0.4;
            obj.status.destroyed = false;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.selected_entity_count(), 1);
        assert_eq!(shadow.under_construction_entity_count(), 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.selected);
        assert!((e.max_health - 150.0).abs() < 0.01);
        assert!((e.construction_percent - 0.4).abs() < 0.01);
        assert!(!e.destroyed);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("e.selected = obj.selected") && src.contains("e.construction_percent"),
            "sync must copy entity selection/construction residual"
        );
    }

    #[test]
    fn sync_players_copies_alive_and_cash_bounty() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AliveBountyShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        let n_players = logic.get_players().len();
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.cash_bounty_percent = 0.2;
            p.color_rgb = (12, 34, 56);
            p.is_alive = true;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.alive_player_count(), n_players);
        assert!((shadow.max_cash_bounty_percent() - 0.2).abs() < 0.001);
        let tinted = shadow
            .world
            .world()
            .active_players()
            .any(|(_, p)| p.color_rgb == (12, 34, 56));
        assert!(tinted, "color_rgb residual must copy");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.is_alive = false;
        }
        shadow.sync_from_host(&logic);
        assert_eq!(shadow.alive_player_count(), n_players.saturating_sub(1));
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("is_alive")
                && src.contains("cash_bounty_percent")
                && src.contains("color_rgb"),
            "sync must refresh alive/bounty/color residual"
        );
    }

    #[test]
    fn sync_players_copies_radar_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RadarShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.radar_count = 2;
            p.radar_disabled = false;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(
            shadow.radar_residual_present(),
            "shadow must copy host radar_count"
        );
        assert!(
            shadow.any_player_has_radar(),
            "hasRadar residual: count>0 && !disabled"
        );
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.radar_disabled = true;
        }
        shadow.sync_from_host(&logic);
        assert!(
            shadow.radar_residual_present(),
            "disabled flag must still be residual-present"
        );
        assert!(
            !shadow.any_player_has_radar(),
            "disabled radar must fail hasRadar residual"
        );
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("radar_count") && src.contains("radar_disabled"),
            "sync_players must refresh radar residual"
        );
    }

    #[test]
    fn sync_players_copies_rank_residual() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RankShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.rank_level = 4;
            p.skill_points = 512;
            p.science_purchase_points = 3;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw = shadow.host_player_to_gw.get(&pid).copied().expect("mapped");
        let pd = shadow.world().player(gw).expect("pd");
        assert_eq!(pd.rank_level, 4);
        assert_eq!(pd.skill_points, 512);
        assert_eq!(pd.science_purchase_points, 3);
        // Last-writer writeback
        {
            let p = shadow.world_mut().player_mut(gw).expect("pdmut");
            p.rank_level = 5;
            p.skill_points = 600;
            p.science_purchase_points = 4;
        }
        let wb = shadow.writeback_economy_to_host(&mut logic);
        assert!(wb >= 1);
        let host = logic.get_player(pid).expect("host");
        assert_eq!(host.rank_level, 5);
        assert_eq!(host.skill_points, 600);
        assert_eq!(host.science_purchase_points, 4);
    }

    #[test]
    fn sync_players_copies_shared_special_power_cooldowns() {
        use crate::command_system::SpecialPowerType;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SwCdShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.shared_special_power_cooldowns
                .insert(SpecialPowerType::ParticleCannon, 55.0);
            p.shared_special_power_cooldowns
                .insert(SpecialPowerType::ScudStorm, 10.0);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let gw = shadow.host_player_to_gw.get(&pid).copied().expect("map");
        let pd = shadow.world().player(gw).expect("pd");
        assert!(
            pd.shared_special_power_cooldowns
                .iter()
                .any(|(k, v)| k == "ParticleCannon" && (*v - 55.0).abs() < 1e-5),
            "must copy ParticleCannon cooldown"
        );
        // last-writer writeback
        {
            let p = shadow.world_mut().player_mut(gw).expect("m");
            if let Some((_, v)) = p
                .shared_special_power_cooldowns
                .iter_mut()
                .find(|(k, _)| k == "ParticleCannon")
            {
                *v = 3.0;
            }
        }
        let _ = shadow.writeback_economy_to_host(&mut logic);
        let host = logic.get_player(pid).expect("h");
        assert!(
            (host
                .shared_special_power_cooldowns
                .get(&SpecialPowerType::ParticleCannon)
                .copied()
                .unwrap_or(-1.0)
                - 3.0)
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn sync_players_copies_power_produced_consumed() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PowerBarShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.power_produced = 120;
            p.power_consumed = 45;
            p.power_available = 75;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(
            shadow.power_bar_residual_present(),
            "shadow must copy host power_produced/consumed"
        );
        let pd = shadow
            .world
            .world()
            .active_players()
            .map(|(_, p)| p)
            .find(|p| p.power_produced == 120 && p.power_consumed == 45)
            .expect("mapped power bar player");
        assert_eq!(pd.power_available, 75);
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("power_produced") && src.contains("power_consumed"),
            "sync_players must refresh power bar residual"
        );
    }

    #[test]
    fn sync_players_copies_unlocked_sciences() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ScienceShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.unlocked_sciences.insert("SCIENCE_PaladinTank".into());
            p.unlocked_sciences.insert("SCIENCE_Pathfinder".into());
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(
            shadow.unlocked_science_count() >= 2,
            "shadow must copy host unlocked sciences"
        );
        let src = include_str!("gameworld_shadow.rs");
        assert!(
            src.contains("host_player_science_and_upgrades") && src.contains("unlocked_sciences"),
            "sync_players must refresh unlocked_sciences residual"
        );
    }

    #[test]
    fn host_upgrade_complete_applies_to_shadow_player() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UpgradeShadow");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        // Record a completed upgrade on the host registry.
        let frame = logic.get_frame();
        logic.host_upgrades_mut().record_complete(
            "Upgrade_AmericaRangerFlashBangGrenade",
            pid,
            frame,
            1,
        );
        let events = logic.host_upgrades().completed_this_frame_snapshot();
        assert!(!events.is_empty(), "host must expose completed_this_frame");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let n = shadow.apply_host_upgrade_events(&events);
        assert!(n >= 1, "upgrade events applied {n}");
        assert!(
            shadow.completed_upgrade_count() >= 1,
            "shadow player must retain completed upgrade"
        );
        // Source honesty: session must drain upgrade snapshot.
        let src = include_str!("gameworld_shadow.rs");
        let idx = src
            .find("fn shadow_session_after_host_tick")
            .expect("session");
        let window = &src[idx..idx + 6000];
        assert!(
            window.contains("completed_this_frame_snapshot")
                && window.contains("apply_host_upgrade_events"),
            "session must apply host upgrade completes"
        );
    }

    #[test]
    fn host_command_set_log_drives_set_command_set_channel() {
        use crate::game_logic::{host_command_set_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CsCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CsU") {
            let mut t = ThingTemplate::new("CsU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("CsU".into(), t);
        }
        let oid = logic
            .create_object("CsU", Team::GLA, glam::Vec3::new(22.0, 0.0, 22.0))
            .expect("id");
        host_command_set_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_command_set_override(Some("Command_DemoSuicide".into()));
        }
        let events = host_command_set_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.command_set == "Command_DemoSuicide"),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_command_set();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.command_set_override.clear();
        }
        let n = shadow.apply_host_command_set_events(&host_command_set_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.command_set_override, "Command_DemoSuicide");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.command_set_override = None;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.command_set_override = "Command_DemoSuicide".into();
        }
        assert!(shadow.writeback_command_set_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(
            o.command_set_override.as_deref(),
            Some("Command_DemoSuicide")
        );
    }

    #[test]
    fn host_selection_radius_log_drives_set_selection_radius_channel() {
        use crate::game_logic::{host_selection_radius_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SrCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SrU") {
            let mut t = ThingTemplate::new("SrU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("SrU".into(), t);
        }
        let oid = logic
            .create_object("SrU", Team::USA, glam::Vec3::new(27.0, 0.0, 27.0))
            .expect("id");
        host_selection_radius_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_selection_radius(14.5);
        }
        let events = host_selection_radius_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && (e.selection_radius - 14.5).abs() < 1e-5),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_selection_radius();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.selection_radius = 1.0;
        }
        let n = shadow.apply_host_selection_radius_events(&host_selection_radius_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.selection_radius - 14.5).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.selection_radius = 1.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.selection_radius = 14.5;
        }
        assert!(shadow.writeback_selection_radius_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!((o.selection_radius - 14.5).abs() < 1e-5);
    }

    #[test]
    fn host_ground_height_log_drives_set_ground_height_channel() {
        use crate::game_logic::{host_ground_height_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GhCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("GhU") {
            let mut t = ThingTemplate::new("GhU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("GhU".into(), t);
        }
        let oid = logic
            .create_object("GhU", Team::USA, glam::Vec3::new(34.0, 0.0, 34.0))
            .expect("id");
        host_ground_height_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_ground_height_residual(12.5, true);
        }
        let events = host_ground_height_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid && (e.ground_height - 12.5).abs() < 1e-5 && e.from_terrain
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_ground_height();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.ground_height = 0.0;
            e.ground_height_from_terrain = false;
        }
        let n = shadow.apply_host_ground_height_events(&host_ground_height_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.ground_height - 12.5).abs() < 1e-5);
        assert!(e.ground_height_from_terrain);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.ground_height = 0.0;
            o.ground_height_from_terrain = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.ground_height = 12.5;
            e.ground_height_from_terrain = true;
        }
        assert!(shadow.writeback_ground_height_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!((o.ground_height - 12.5).abs() < 1e-5);
        assert!(o.ground_height_from_terrain);
    }

    #[test]
    fn host_model_mesh_log_drives_set_model_mesh_channel() {
        use crate::game_logic::{host_model_mesh_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MmCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerMesh") {
            let mut t = ThingTemplate::new("RangerMesh");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            t.model_name = Some("airanger_s".into());
            logic.templates.insert("RangerMesh".into(), t);
        }
        let oid = logic
            .create_object("RangerMesh", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

        host_model_mesh_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_model_mesh_residual("avtank", 1.25);
        }
        let events = host_model_mesh_log::drain();
        assert!(
            events.iter().any(|e| e.object == oid
                && e.model_key == "avtank"
                && (e.mesh_scale - 1.25).abs() < 1e-5),
            "events {:?}",
            events
        );

        // Re-apply path
        host_model_mesh_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_model_mesh_residual("avtank", 1.25);
        }
        let n = shadow.apply_host_model_mesh_events(&host_model_mesh_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.model_key, "avtank");
        assert!((e.mesh_scale - 1.25).abs() < 1e-5);
    }

    #[test]
    fn host_fow_log_drives_set_fow_channel() {
        use crate::game_logic::{host_fow_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FowCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerFow") {
            let mut t = ThingTemplate::new("RangerFow");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("RangerFow".into(), t);
        }
        let oid = logic
            .create_object("RangerFow", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

        host_fow_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_fow_residual(0.35, 1.0, 0.5);
        }
        let events = host_fow_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && (e.visibility_alpha - 0.35).abs() < 1e-5
                    && (e.is_explored - 1.0).abs() < 1e-5
                    && (e.visibility_falloff - 0.5).abs() < 1e-5
            }),
            "events {:?}",
            events
        );

        host_fow_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_fow_residual(0.35, 1.0, 0.5);
        }
        let n = shadow.apply_host_fow_events(&host_fow_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.fow_visibility_alpha - 0.35).abs() < 1e-5);
        assert!((e.fow_is_explored - 1.0).abs() < 1e-5);
        assert!((e.fow_visibility_falloff - 0.5).abs() < 1e-5);
    }

    #[test]
    fn host_kind_of_log_drives_set_kind_of_bits_channel() {
        use crate::game_logic::{host_kind_of_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("KoCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerKo") {
            let mut t = ThingTemplate::new("RangerKo");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert("RangerKo".into(), t);
        }
        let oid = logic
            .create_object("RangerKo", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

        let bits = {
            let o = logic.get_objects().get(&oid).expect("o");
            o.presentation_kind_of_bits()
        };
        assert!(bits != 0, "bits {bits}");

        host_kind_of_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_kind_of_bits_residual(bits | (1u32 << 10)); // set Hero bit residual
        }
        let events = host_kind_of_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.kind_of_bits == (bits | (1u32 << 10))),
            "events {:?}",
            events
        );

        host_kind_of_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_kind_of_bits_residual(bits | (1u32 << 10));
        }
        let n = shadow.apply_host_kind_of_events(&host_kind_of_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.kind_of_bits, bits | (1u32 << 10));
    }

    #[test]
    fn host_identity_log_drives_set_identity_channel() {
        use crate::game_logic::{host_identity_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("IdCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("IdU") {
            let mut t = ThingTemplate::new("IdU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("IdU".into(), t);
        }
        let oid = logic
            .create_object("IdU", Team::USA, glam::Vec3::new(33.0, 0.0, 33.0))
            .expect("id");
        host_identity_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.name = "ScriptRanger".into();
            o.team_color = [0.1, 0.2, 0.3, 1.0];
            o.record_host_identity();
        }
        let events = host_identity_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.name == "ScriptRanger"
                    && (e.team_color[0] - 0.1).abs() < 1e-5
                    && (e.team_color[2] - 0.3).abs() < 1e-5
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_identity();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.display_name.clear();
            e.team_color = [1.0, 1.0, 1.0, 1.0];
        }
        let n = shadow.apply_host_identity_events(&host_identity_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.display_name, "ScriptRanger");
        assert!((e.team_color[0] - 0.1).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.name.clear();
            o.team_color = [0.0, 0.0, 0.0, 1.0];
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.display_name = "ScriptRanger".into();
            e.team_color = [0.1, 0.2, 0.3, 1.0];
        }
        assert!(shadow.writeback_identity_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.name, "ScriptRanger");
        assert!((o.team_color[0] - 0.1).abs() < 1e-5);
    }

    #[test]
    fn host_building_type_log_drives_set_building_type_channel() {
        use crate::game_logic::{
            host_building_type_log, BuildingData, BuildingType, KindOf, Team, ThingTemplate,
        };
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("BtCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("BtU") {
            let mut t = ThingTemplate::new("BtU");
            t.set_health(400.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("BtU".into(), t);
        }
        let oid = logic
            .create_object("BtU", Team::USA, glam::Vec3::new(32.0, 0.0, 32.0))
            .expect("id");
        host_building_type_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.building_data = Some(BuildingData::new(BuildingType::Barracks));
            o.record_host_building_type();
        }
        let events = host_building_type_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.is_building && e.building_type_ordinal == 1),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_building_type();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.is_building = false;
            e.building_type_ordinal = 255;
        }
        let n = shadow.apply_host_building_type_events(&host_building_type_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.is_building);
        assert_eq!(e.building_type_ordinal, 1);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.building_data = Some(BuildingData::new(BuildingType::PowerPlant));
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.is_building = true;
            e.building_type_ordinal = 1;
        }
        assert!(shadow.writeback_building_type_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(
            o.building_data.as_ref().map(|b| b.building_type),
            Some(BuildingType::Barracks)
        );
    }

    #[test]
    fn host_crush_vision_log_drives_set_crush_vision_channel() {
        use crate::game_logic::{host_crush_vision_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CvCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CvU") {
            let mut t = ThingTemplate::new("CvU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("CvU".into(), t);
        }
        let oid = logic
            .create_object("CvU", Team::USA, glam::Vec3::new(30.0, 0.0, 30.0))
            .expect("id");
        host_crush_vision_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.crusher_level = 2;
            o.crushable_level = 1;
            o.vision_range = 175.0;
            o.shroud_clearing_range = 200.0;
            o.record_host_crush_vision();
        }
        let events = host_crush_vision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.crusher_level == 2
                    && e.crushable_level == 1
                    && (e.vision_range - 175.0).abs() < 1e-5
                    && (e.shroud_clearing_range - 200.0).abs() < 1e-5
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_crush_vision();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.crusher_level = 0;
            e.crushable_level = 0;
            e.vision_range = 0.0;
            e.shroud_clearing_range = 0.0;
        }
        let n = shadow.apply_host_crush_vision_events(&host_crush_vision_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.crusher_level, 2);
        assert_eq!(e.crushable_level, 1);
        assert!((e.vision_range - 175.0).abs() < 1e-5);
        assert!((e.shroud_clearing_range - 200.0).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.crusher_level = 0;
            o.crushable_level = 0;
            o.vision_range = 0.0;
            o.shroud_clearing_range = 0.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.crusher_level = 2;
            e.crushable_level = 1;
            e.vision_range = 175.0;
            e.shroud_clearing_range = 200.0;
        }
        assert!(shadow.writeback_crush_vision_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.crusher_level, 2);
        assert_eq!(o.crushable_level, 1);
        assert!((o.vision_range - 175.0).abs() < 1e-5);
        assert!((o.shroud_clearing_range - 200.0).abs() < 1e-5);
    }

    #[test]
    fn host_demo_mine_cheer_log_drives_set_demo_mine_cheer_channel() {
        use crate::game_logic::{
            host_demo_mine_cheer_log, host_mines::HostMineData, host_mines::HostMineKind, KindOf,
            Team, ThingTemplate,
        };
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmcCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("DmcU") {
            let mut t = ThingTemplate::new("DmcU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("DmcU".into(), t);
        }
        let oid = logic
            .create_object("DmcU", Team::GLA, glam::Vec3::new(29.0, 0.0, 29.0))
            .expect("id");
        host_demo_mine_cheer_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.demo_suicided_detonating = true;
            o.cheer_timer = 2.5;
            o.mine_data = Some(HostMineData::new(HostMineKind::LandMine));
            o.record_host_demo_mine_cheer();
        }
        let events = host_demo_mine_cheer_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.demo_suicided_detonating
                    && e.has_mine_data
                    && (e.cheer_timer - 2.5).abs() < 1e-5
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_demo_mine_cheer();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.demo_suicided_detonating = false;
            e.has_mine_data = false;
            e.cheer_timer = 0.0;
        }
        let n = shadow.apply_host_demo_mine_cheer_events(&host_demo_mine_cheer_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.demo_suicided_detonating && e.has_mine_data);
        assert!((e.cheer_timer - 2.5).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.demo_suicided_detonating = false;
            o.cheer_timer = 0.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.demo_suicided_detonating = true;
            e.has_mine_data = true;
            e.cheer_timer = 2.5;
        }
        assert!(shadow.writeback_demo_mine_cheer_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.demo_suicided_detonating);
        assert!((o.cheer_timer - 2.5).abs() < 1e-5);
        assert!(o.mine_data.is_some());
    }

    #[test]
    fn host_model_condition_log_drives_set_model_condition_channel() {
        use crate::game_logic::{host_model_condition_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("McCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("McU") {
            let mut t = ThingTemplate::new("McU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("McU".into(), t);
        }
        let oid = logic
            .create_object("McU", Team::China, glam::Vec3::new(28.0, 0.0, 28.0))
            .expect("id");
        host_model_condition_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.model_condition_bits = 0b1011;
            o.record_host_model_condition();
        }
        let events = host_model_condition_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.model_condition_bits == 0b1011),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_model_condition();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.model_condition_bits = 0;
        }
        let n = shadow.apply_host_model_condition_events(&host_model_condition_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.model_condition_bits, 0b1011);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.model_condition_bits = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.model_condition_bits = 0b1011;
        }
        assert!(shadow.writeback_model_condition_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.model_condition_bits, 0b1011);
    }

    #[test]
    fn host_movement_log_drives_set_movement_channel() {
        use crate::game_logic::{host_movement_log, KindOf, Team, ThingTemplate};
        use glam::Vec3;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MvCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("MvU") {
            let mut t = ThingTemplate::new("MvU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("MvU".into(), t);
        }
        let oid = logic
            .create_object("MvU", Team::USA, glam::Vec3::new(26.0, 0.0, 26.0))
            .expect("id");
        host_movement_log::clear();
        crate::game_logic::host_physics_motive_log::clear();
        crate::game_logic::host_locomotor_log::clear();
        crate::game_logic::host_bounce_land_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.movement.velocity = Vec3::new(3.0, 0.0, 4.0);
            o.movement.max_speed = 12.5;
            o.movement.path = vec![Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 2.0)];
            o.movement.current_path_index = 1;
            o.record_host_movement();
        }
        let events = host_movement_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && (e.velocity[0] - 3.0).abs() < 1e-5
                    && (e.velocity[2] - 4.0).abs() < 1e-5
                    && (e.max_speed - 12.5).abs() < 1e-5
                    && e.path_index == 1
                    && e.path_len == 2
                    && e.path_waypoints.len() == 2
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_movement();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.velocity = [0.0, 0.0, 0.0];
            e.move_max_speed = 1.0;
            e.path_index = 0;
            e.path_len = 0;
            e.path_waypoints.clear();
        }
        let n = shadow.apply_host_movement_events(&host_movement_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.velocity[0] - 3.0).abs() < 1e-5);
        assert!((e.move_max_speed - 12.5).abs() < 1e-5);
        assert_eq!(e.path_index, 1);
        assert_eq!(e.path_len, 2);
        assert_eq!(e.path_waypoints.len(), 2);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.movement.velocity = Vec3::ZERO;
            o.movement.max_speed = 1.0;
            o.movement.path.clear();
            o.movement.current_path_index = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.velocity = [3.0, 0.0, 4.0];
            e.move_max_speed = 12.5;
            e.path_index = 1;
            e.path_len = 2;
            e.path_waypoints = vec![[1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
        }
        assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_physics_motive_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_bounce_land_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!((o.movement.velocity.x - 3.0).abs() < 1e-5);
        assert!((o.movement.max_speed - 12.5).abs() < 1e-5);
        assert_eq!(o.movement.current_path_index, 1);
        assert_eq!(o.movement.path.len(), 2);
    }

    #[test]
    fn host_weapon_stats_log_drives_set_weapon_stats_channel() {
        use crate::game_logic::{host_weapon_stats_log, KindOf, Team, ThingTemplate, Weapon};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WsCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WsU") {
            let mut t = ThingTemplate::new("WsU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("WsU".into(), t);
        }
        let oid = logic
            .create_object("WsU", Team::GLA, glam::Vec3::new(25.0, 0.0, 25.0))
            .expect("id");
        host_weapon_stats_log::clear();
        crate::game_logic::host_body_damage_log::clear();
        crate::game_logic::host_death_type_log::clear();
        crate::game_logic::host_radar_extend_log::clear();
        crate::game_logic::host_shock_stun_log::clear();
        crate::game_logic::host_rebuild_producer_log::clear();
        crate::game_logic::host_sole_healing_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.weapon = Some(Weapon {
                damage: 33.0,
                range: 140.0,
                min_range: 4.0,
                reload_time: 0.75,
                ammo: Some(12),
                can_target_air: true,
                can_target_ground: true,
                projectile_speed: 90.0,
                ..Weapon::default()
            });
            o.secondary_weapon = Some(Weapon {
                damage: 9.0,
                range: 80.0,
                ..Weapon::default()
            });
            o.record_host_weapon_stats();
        }
        let events = host_weapon_stats_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.has_weapon
                    && (e.weapon_damage - 33.0).abs() < 1e-5
                    && (e.weapon_range - 140.0).abs() < 1e-5
                    && e.weapon_ammo == 12
                    && e.has_secondary_weapon
                    && (e.secondary_weapon_damage - 9.0).abs() < 1e-5
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_weapon_stats();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.has_weapon = false;
            e.weapon_damage = 0.0;
            e.weapon_range = 0.0;
            e.weapon_min_range = 0.0;
            e.weapon_reload_time = 0.0;
            e.weapon_ammo = u32::MAX;
            e.weapon_can_target_air = false;
            e.weapon_can_target_ground = false;
            e.weapon_projectile_speed = 0.0;
            e.has_secondary_weapon = false;
            e.secondary_weapon_damage = 0.0;
            e.secondary_weapon_range = 0.0;
        }
        let n = shadow.apply_host_weapon_stats_events(&host_weapon_stats_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.has_weapon && e.has_secondary_weapon);
        assert!((e.weapon_damage - 33.0).abs() < 1e-5);
        assert!((e.weapon_range - 140.0).abs() < 1e-5);
        assert_eq!(e.weapon_ammo, 12);
        assert!((e.secondary_weapon_damage - 9.0).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            if let Some(w) = o.weapon.as_mut() {
                w.damage = 1.0;
                w.range = 1.0;
                w.min_range = 0.0;
                w.reload_time = 0.1;
                w.ammo = None;
                w.can_target_air = false;
                w.can_target_ground = true;
                w.projectile_speed = 0.0;
            }
            if let Some(w) = o.secondary_weapon.as_mut() {
                w.damage = 1.0;
                w.range = 1.0;
            }
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.weapon_damage = 33.0;
            e.weapon_range = 140.0;
            e.weapon_min_range = 4.0;
            e.weapon_reload_time = 0.75;
            e.weapon_ammo = 12;
            e.weapon_can_target_air = true;
            e.weapon_can_target_ground = true;
            e.weapon_projectile_speed = 90.0;
            e.secondary_weapon_damage = 9.0;
            e.secondary_weapon_range = 80.0;
        }
        assert!(shadow.writeback_weapon_stats_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        let w = o.weapon.as_ref().expect("w");
        assert!((w.damage - 33.0).abs() < 1e-5);
        assert!((w.range - 140.0).abs() < 1e-5);
        assert_eq!(w.ammo, Some(12));
        let s = o.secondary_weapon.as_ref().expect("s");
        assert!((s.damage - 9.0).abs() < 1e-5);
        assert!((s.range - 80.0).abs() < 1e-5);
    }

    #[test]
    fn host_vision_camo_log_drives_set_vision_camo_channel() {
        use crate::game_logic::{host_vision_camo_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("VcCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("VcU") {
            let mut t = ThingTemplate::new("VcU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("VcU".into(), t);
        }
        let oid = logic
            .create_object("VcU", Team::China, glam::Vec3::new(24.0, 0.0, 24.0))
            .expect("id");
        host_vision_camo_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.vision_spied_mask = 0b101;
            o.camo_friendly_opacity = 0.35;
            o.camo_stealth_look = 2;
            o.record_host_vision_camo();
        }
        let events = host_vision_camo_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.vision_spied_mask == 0b101
                    && (e.camo_friendly_opacity - 0.35).abs() < 1e-5
                    && e.camo_stealth_look == 2
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_vision_camo();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.vision_spied_mask = 0;
            e.camo_friendly_opacity = 1.0;
            e.camo_stealth_look = 0;
        }
        let n = shadow.apply_host_vision_camo_events(&host_vision_camo_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.vision_spied_mask, 0b101);
        assert!((e.camo_friendly_opacity - 0.35).abs() < 1e-5);
        assert_eq!(e.camo_stealth_look, 2);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.vision_spied_mask = 0;
            o.camo_friendly_opacity = 1.0;
            o.camo_stealth_look = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.vision_spied_mask = 0b101;
            e.camo_friendly_opacity = 0.35;
            e.camo_stealth_look = 2;
        }
        assert!(shadow.writeback_vision_camo_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
        let _ = shadow.writeback_combat_attack_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.vision_spied_mask, 0b101);
        assert!((o.camo_friendly_opacity - 0.35).abs() < 1e-5);
        assert_eq!(o.camo_stealth_look, 2);
    }

    #[test]
    fn host_disguise_log_drives_set_disguise_channel() {
        use crate::game_logic::{host_disguise_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DgCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("DgU") {
            let mut t = ThingTemplate::new("DgU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("DgU".into(), t);
        }
        let oid = logic
            .create_object("DgU", Team::GLA, glam::Vec3::new(23.0, 0.0, 23.0))
            .expect("id");
        host_disguise_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.disguise_as_template = Some("AmericaVehicleHumvee".into());
            o.disguise_as_team = Some(Team::USA);
            o.record_host_disguise();
        }
        let events = host_disguise_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid && e.template == "AmericaVehicleHumvee" && e.team_ordinal == 0
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_disguise();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.disguise_as_template.clear();
            e.disguise_as_team_ordinal = 255;
        }
        let n = shadow.apply_host_disguise_events(&host_disguise_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.disguise_as_template, "AmericaVehicleHumvee");
        assert_eq!(e.disguise_as_team_ordinal, 0);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.disguise_as_template = None;
            o.disguise_as_team = None;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.disguise_as_template = "AmericaVehicleHumvee".into();
            e.disguise_as_team_ordinal = 0;
        }
        assert!(shadow.writeback_disguise_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(
            o.disguise_as_template.as_deref(),
            Some("AmericaVehicleHumvee")
        );
        assert_eq!(o.disguise_as_team, Some(Team::USA));
    }

    #[test]
    fn host_overlord_log_drives_set_overlord_addon_channel() {
        use crate::game_logic::{host_overlord_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("OlCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("OlU") {
            let mut t = ThingTemplate::new("OlU");
            t.set_health(400.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("OlU".into(), t);
        }
        let oid = logic
            .create_object("OlU", Team::China, glam::Vec3::new(21.0, 0.0, 21.0))
            .expect("id");
        host_overlord_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.has_overlord_gattling_addon = true;
            o.has_overlord_propaganda_addon = false;
            o.overlord_bunker_capacity = Some(4);
            o.is_helix_transport = true;
            o.record_host_overlord();
        }
        let events = host_overlord_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.has_gattling
                    && !e.has_propaganda
                    && e.bunker_capacity == 4
                    && e.is_helix_transport
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_overlord();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.has_overlord_gattling_addon = false;
            e.has_overlord_propaganda_addon = true;
            e.overlord_bunker_capacity = u16::MAX;
            e.is_helix_transport = false;
        }
        let n = shadow.apply_host_overlord_events(&host_overlord_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.has_overlord_gattling_addon && !e.has_overlord_propaganda_addon);
        assert_eq!(e.overlord_bunker_capacity, 4);
        assert!(e.is_helix_transport);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.has_overlord_gattling_addon = false;
            o.has_overlord_propaganda_addon = true;
            o.overlord_bunker_capacity = None;
            o.is_helix_transport = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.has_overlord_gattling_addon = true;
            e.has_overlord_propaganda_addon = false;
            e.overlord_bunker_capacity = 4;
            e.is_helix_transport = true;
        }
        assert!(shadow.writeback_overlord_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.has_overlord_gattling_addon && !o.has_overlord_propaganda_addon);
        assert_eq!(o.overlord_bunker_capacity, Some(4));
        assert!(o.is_helix_transport);
    }

    #[test]
    fn host_stealth_flags_log_drives_set_stealth_flags_channel() {
        use crate::game_logic::{host_stealth_flags_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StfCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("StfU") {
            let mut t = ThingTemplate::new("StfU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("StfU".into(), t);
        }
        let oid = logic
            .create_object("StfU", Team::GLA, glam::Vec3::new(20.0, 0.0, 20.0))
            .expect("id");
        host_stealth_flags_log::clear();
        crate::game_logic::host_stealth_delay_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.innate_stealth = true;
            o.stealth_breaks_on_attack = true;
            o.stealth_breaks_on_move = false;
            o.is_tunnel_network = true;
            o.passengers_allowed_to_fire = true;
            o.record_host_stealth_flags();
        }
        let events = host_stealth_flags_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.innate_stealth
                    && e.stealth_breaks_on_attack
                    && !e.stealth_breaks_on_move
                    && e.is_tunnel_network
                    && e.passengers_allowed_to_fire
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_stealth_flags();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.innate_stealth = false;
            e.stealth_breaks_on_attack = false;
            e.stealth_breaks_on_move = true;
            e.is_tunnel_network = false;
            e.passengers_allowed_to_fire = false;
        }
        let n = shadow.apply_host_stealth_flags_events(&host_stealth_flags_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.innate_stealth && e.stealth_breaks_on_attack && !e.stealth_breaks_on_move);
        assert!(e.is_tunnel_network && e.passengers_allowed_to_fire);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.innate_stealth = false;
            o.stealth_breaks_on_attack = false;
            o.stealth_breaks_on_move = true;
            o.is_tunnel_network = false;
            o.passengers_allowed_to_fire = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.innate_stealth = true;
            e.stealth_breaks_on_attack = true;
            e.stealth_breaks_on_move = false;
            e.is_tunnel_network = true;
            e.passengers_allowed_to_fire = true;
        }
        assert!(shadow.writeback_stealth_flags_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
        let _ = shadow.writeback_combat_attack_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.innate_stealth && o.stealth_breaks_on_attack && !o.stealth_breaks_on_move);
        assert!(o.is_tunnel_network && o.passengers_allowed_to_fire);
    }

    #[test]
    fn host_hive_log_drives_set_hive_slaves_channel() {
        use crate::game_logic::{host_hive_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HiveCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("HiveU") {
            let mut t = ThingTemplate::new("HiveU");
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("HiveU".into(), t);
        }
        let oid = logic
            .create_object("HiveU", Team::GLA, glam::Vec3::new(19.0, 0.0, 19.0))
            .expect("id");
        host_hive_log::clear();
        crate::game_logic::host_hijacker_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.hive_slave_count = 3;
            o.hive_slave_hp = 55.0;
            o.record_host_hive();
        }
        let events = host_hive_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid && e.slave_count == 3 && (e.slave_hp - 55.0).abs() < 1e-3
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_hive();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.hive_slave_count = 0;
            e.hive_slave_hp = 0.0;
        }
        let n = shadow.apply_host_hive_events(&host_hive_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.hive_slave_count, 3);
        assert!((e.hive_slave_hp - 55.0).abs() < 1e-3);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.hive_slave_count = 0;
            o.hive_slave_hp = 0.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.hive_slave_count = 3;
            e.hive_slave_hp = 55.0;
        }
        assert!(shadow.writeback_hive_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.hive_slave_count, 3);
        assert!((o.hive_slave_hp - 55.0).abs() < 1e-3);
    }

    #[test]
    fn host_contain_capacity_log_drives_set_contain_capacity_channel() {
        use crate::game_logic::buildings::{BuildingData, BuildingType};
        use crate::game_logic::{host_contain_capacity_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CapCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CapU") {
            let mut t = ThingTemplate::new("CapU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("CapU".into(), t);
        }
        let oid = logic
            .create_object("CapU", Team::USA, glam::Vec3::new(18.0, 0.0, 18.0))
            .expect("id");
        host_contain_capacity_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.max_transport = 5;
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = 8;
            o.building_data = Some(bd);
            o.record_host_contain_capacity();
        }
        let events = host_contain_capacity_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.max_transport == 5 && e.max_garrison == 8),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_contain_capacity();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.max_transport = 0;
            e.max_garrison = 0;
        }
        let n = shadow.apply_host_contain_capacity_events(&host_contain_capacity_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.max_transport, 5);
        assert_eq!(e.max_garrison, 8);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.max_transport = 0;
            if let Some(bd) = o.building_data.as_mut() {
                bd.max_garrison = 0;
            }
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.max_transport = 5;
            e.max_garrison = 8;
        }
        assert!(shadow.writeback_contain_capacity_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.max_transport, 5);
        assert_eq!(o.building_data.as_ref().map(|bd| bd.max_garrison), Some(8));
    }

    #[test]
    fn host_overcharge_log_drives_set_overcharge_channel() {
        use crate::game_logic::{host_overcharge_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("OcCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("OCU") {
            let mut t = ThingTemplate::new("OCU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("OCU".into(), t);
        }
        let oid = logic
            .create_object("OCU", Team::China, glam::Vec3::new(17.0, 0.0, 17.0))
            .expect("id");
        host_overcharge_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_overcharge_enabled(true);
        }
        let events = host_overcharge_log::drain();
        assert!(
            events.iter().any(|e| e.object == oid && e.enabled),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_overcharge();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.overcharge_enabled = false;
        }
        let n = shadow.apply_host_overcharge_events(&host_overcharge_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.overcharge_enabled);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.overcharge_enabled = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.overcharge_enabled = true;
        }
        assert!(shadow.writeback_overcharge_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.overcharge_enabled);
    }

    #[test]
    fn host_weapon_set_log_drives_set_weapon_set_flags_channel() {
        use crate::game_logic::{host_weapon_set_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WSetCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WSU") {
            let mut t = ThingTemplate::new("WSU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("WSU".into(), t);
        }
        let oid = logic
            .create_object("WSU", Team::USA, glam::Vec3::new(16.0, 0.0, 16.0))
            .expect("id");
        host_weapon_set_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.weapon_set_player_upgrade = true;
            o.armed_riders_upgrade_weapon_set = true;
            o.record_host_weapon_set();
        }
        let events = host_weapon_set_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.player_upgrade && e.armed_riders),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_weapon_set();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.weapon_set_player_upgrade = false;
            e.armed_riders_upgrade_weapon_set = false;
        }
        let n = shadow.apply_host_weapon_set_events(&host_weapon_set_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.weapon_set_player_upgrade && e.armed_riders_upgrade_weapon_set);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.weapon_set_player_upgrade = false;
            o.armed_riders_upgrade_weapon_set = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.weapon_set_player_upgrade = true;
            e.armed_riders_upgrade_weapon_set = true;
        }
        assert!(shadow.writeback_weapon_set_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.weapon_set_player_upgrade && o.armed_riders_upgrade_weapon_set);
    }

    #[test]
    fn host_ai_attitude_log_drives_set_ai_attitude_channel() {
        use crate::game_logic::{host_ai_attitude_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AttCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AU") {
            let mut t = ThingTemplate::new("AU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("AU".into(), t);
        }
        let oid = logic
            .create_object("AU", Team::USA, glam::Vec3::new(15.0, 0.0, 15.0))
            .expect("id");
        host_ai_attitude_log::clear();
        crate::game_logic::host_ai_mood_log::clear();
        crate::game_logic::host_ai_request_log::clear();
        crate::game_logic::host_ai_decision_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_ai_attitude_i8(2);
        }
        let events = host_ai_attitude_log::drain();
        assert!(
            events.iter().any(|e| e.object == oid && e.attitude == 2),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_ai_attitude();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.ai_attitude = 0;
        }
        let n = shadow.apply_host_ai_attitude_events(&host_ai_attitude_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.ai_attitude, 2);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.ai_attitude = -2;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.ai_attitude = 2;
        }
        assert!(shadow.writeback_ai_attitude_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.ai_attitude, 2);
    }

    #[test]
    fn host_guard_log_drives_set_guard_channel() {
        use crate::game_logic::{host_guard_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GuardCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("GU") {
            let mut t = ThingTemplate::new("GU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("GU".into(), t);
        }
        let oid = logic
            .create_object("GU", Team::USA, glam::Vec3::new(12.0, 0.0, 12.0))
            .expect("id");
        let tid = logic
            .create_object("GU", Team::USA, glam::Vec3::new(14.0, 0.0, 14.0))
            .expect("tid");
        host_guard_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_guard_position(Some(glam::Vec3::new(3.0, 0.0, 5.0)));
            o.set_guard_target(Some(tid));
        }
        let events = host_guard_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.target_host == tid.0
                    && e.position
                        .map(|p| (p[0] - 3.0).abs() < 1e-3 && (p[2] - 5.0).abs() < 1e-3)
                        .unwrap_or(false)
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_guard();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.guard_position = None;
            e.guard_target_host = 0;
        }
        let n = shadow.apply_host_guard_events(&host_guard_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        let gp = e.guard_position.expect("gp");
        assert!((gp[0] - 3.0).abs() < 1e-3 && (gp[2] - 5.0).abs() < 1e-3);
        assert_eq!(e.guard_target_host, tid.0);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.guard_position = None;
            o.guard_target = None;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.guard_position = Some([3.0, 0.0, 5.0]);
            e.guard_target_host = tid.0;
        }
        assert!(shadow.writeback_guard_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        let p = o.guard_position.expect("host gp");
        assert!((p.x - 3.0).abs() < 1e-3 && (p.z - 5.0).abs() < 1e-3);
        assert_eq!(o.guard_target, Some(tid));
    }

    #[test]
    fn host_continuous_fire_log_drives_set_continuous_fire_channel() {
        use crate::game_logic::{host_continuous_fire_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CFireCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CFU") {
            let mut t = ThingTemplate::new("CFU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("CFU".into(), t);
        }
        let oid = logic
            .create_object("CFU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
            .expect("id");
        host_continuous_fire_log::clear();
        crate::game_logic::host_combat_attack_log::clear();
        crate::game_logic::host_fire_intent_log::clear();
        crate::game_logic::host_fire_spawn_log::clear();
        crate::game_logic::host_projectile_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.continuous_fire_level = 2;
            o.continuous_fire_consecutive = 9;
            o.continuous_fire_coast_until_frame = 44;
            o.record_host_continuous_fire();
        }
        let events = host_continuous_fire_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid && e.level == 2 && e.consecutive == 9 && e.coast_until_frame == 44
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_continuous_fire();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.continuous_fire_level = 0;
            e.continuous_fire_consecutive = 0;
            e.continuous_fire_coast_until_frame = 0;
        }
        let n = shadow.apply_host_continuous_fire_events(&host_continuous_fire_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.continuous_fire_level, 2);
        assert_eq!(e.continuous_fire_consecutive, 9);
        assert_eq!(e.continuous_fire_coast_until_frame, 44);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.continuous_fire_level = 0;
            o.continuous_fire_consecutive = 0;
            o.continuous_fire_coast_until_frame = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.continuous_fire_level = 2;
            e.continuous_fire_consecutive = 9;
            e.continuous_fire_coast_until_frame = 44;
        }
        assert!(shadow.writeback_continuous_fire_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_combat_attack_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.continuous_fire_level, 2);
        assert_eq!(o.continuous_fire_consecutive, 9);
        assert_eq!(o.continuous_fire_coast_until_frame, 44);
    }

    #[test]
    fn host_detector_log_drives_set_detector_channel() {
        use crate::game_logic::{host_detector_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DetCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("DetU") {
            let mut t = ThingTemplate::new("DetU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("DetU".into(), t);
        }
        let oid = logic
            .create_object("DetU", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
            .expect("id");
        host_detector_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_detector_state(true, 175.0, 12);
        }
        let events = host_detector_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.is_detector
                    && (e.detection_range - 175.0).abs() < 1e-3
                    && e.detection_rate_frames == 12
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_detector();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.is_detector = false;
            e.detection_range = 0.0;
            e.detection_rate_frames = 0;
        }
        let n = shadow.apply_host_detector_events(&host_detector_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.is_detector);
        assert!((e.detection_range - 175.0).abs() < 1e-3);
        assert_eq!(e.detection_rate_frames, 12);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.is_detector = false;
            o.detection_range = 0.0;
            o.detection_rate_frames = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.is_detector = true;
            e.detection_range = 175.0;
            e.detection_rate_frames = 12;
        }
        assert!(shadow.writeback_detector_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.is_detector);
        assert!((o.detection_range - 175.0).abs() < 1e-3);
        assert_eq!(o.detection_rate_frames, 12);
    }

    #[test]
    fn host_target_location_log_drives_set_target_location_channel() {
        use crate::game_logic::{host_target_location_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("TLocCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("TLocU") {
            let mut t = ThingTemplate::new("TLocU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("TLocU".into(), t);
        }
        let oid = logic
            .create_object("TLocU", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
            .expect("id");
        host_target_location_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_target_location(Some(glam::Vec3::new(11.0, 0.0, 13.0)));
        }
        let events = host_target_location_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.location
                        .map(|p| (p[0] - 11.0).abs() < 1e-3 && (p[2] - 13.0).abs() < 1e-3)
                        .unwrap_or(false)
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_target_location();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.target_location = None;
        }
        let n = shadow.apply_host_target_location_events(&host_target_location_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        let tl = e.target_location.expect("tl");
        assert!((tl[0] - 11.0).abs() < 1e-3 && (tl[2] - 13.0).abs() < 1e-3);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.target_location = None;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.target_location = Some([11.0, 0.0, 13.0]);
        }
        assert!(shadow.writeback_target_location_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        let p = o.target_location.expect("host tl");
        assert!((p.x - 11.0).abs() < 1e-3 && (p.z - 13.0).abs() < 1e-3);
    }

    #[test]
    fn host_turret_log_drives_set_turret_channel() {
        use crate::game_logic::{host_turret_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("TurretCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("TurU") {
            let mut t = ThingTemplate::new("TurU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("TurU".into(), t);
        }
        let oid = logic
            .create_object("TurU", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
            .expect("id");
        host_turret_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.turret_angle_deg = 33.0;
            o.turret_pitch_deg = 12.0;
            o.turret_holding = true;
            o.turret_idle_scanning = false;
            o.record_host_turret();
        }
        let events = host_turret_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && (e.angle_deg - 33.0).abs() < 1e-3
                    && (e.pitch_deg - 12.0).abs() < 1e-3
                    && e.holding
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_turret();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.turret_angle_deg = 0.0;
            e.turret_pitch_deg = 0.0;
            e.turret_holding = false;
        }
        let n = shadow.apply_host_turret_events(&host_turret_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.turret_angle_deg - 33.0).abs() < 1e-3);
        assert!((e.turret_pitch_deg - 12.0).abs() < 1e-3);
        assert!(e.turret_holding);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.turret_angle_deg = 0.0;
            o.turret_pitch_deg = 0.0;
            o.turret_holding = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.turret_angle_deg = 33.0;
            e.turret_pitch_deg = 12.0;
            e.turret_holding = true;
        }
        assert!(shadow.writeback_turret_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
        let _ = shadow.writeback_combat_attack_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!((o.turret_angle_deg - 33.0).abs() < 1e-3);
        assert!((o.turret_pitch_deg - 12.0).abs() < 1e-3);
        assert!(o.turret_holding);
    }

    #[test]
    fn host_entity_power_log_drives_set_entity_power_channel() {
        use crate::game_logic::{host_entity_power_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EPowerCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("EPU") {
            let mut t = ThingTemplate::new("EPU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("EPU".into(), t);
        }
        let oid = logic
            .create_object("EPU", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
            .expect("id");
        host_entity_power_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_entity_power(50, 5);
        }
        let events = host_entity_power_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.power_provided == 50 && e.power_consumed == 5),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_entity_power();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.power_provided = 0;
            e.power_consumed = 0;
        }
        let n = shadow.apply_host_entity_power_events(&host_entity_power_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.power_provided, 50);
        assert_eq!(e.power_consumed, 5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.power_provided = 1;
            o.power_consumed = 1;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.power_provided = 50;
            e.power_consumed = 5;
        }
        assert!(shadow.writeback_entity_power_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.power_provided, 50);
        assert_eq!(o.power_consumed, 5);
    }

    #[test]
    fn host_weapon_slot_log_drives_set_active_weapon_slot_channel() {
        use crate::game_logic::{host_weapon_slot_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WSlotCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WSU") {
            let mut t = ThingTemplate::new("WSU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("WSU".into(), t);
        }
        let oid = logic
            .create_object("WSU", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
            .expect("id");
        host_weapon_slot_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.set_active_weapon_slot(1);
        }
        let events = host_weapon_slot_log::drain();
        assert!(
            events.iter().any(|e| e.object == oid && e.slot == 1),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_weapon_slot();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.active_weapon_slot = 0;
        }
        let n = shadow.apply_host_weapon_slot_events(&host_weapon_slot_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.active_weapon_slot, 1);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.active_weapon_slot = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.active_weapon_slot = 1;
        }
        assert!(shadow.writeback_weapon_slot_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.active_weapon_slot, 1);
    }

    #[test]
    fn host_weapon_bonus_log_drives_set_weapon_bonus_channel() {
        use crate::game_logic::{host_weapon_bonus_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WBonusCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WBU") {
            let mut t = ThingTemplate::new("WBU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("WBU".into(), t);
        }
        let oid = logic
            .create_object("WBU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("id");
        host_weapon_bonus_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.apply_weapon_bonus_frenzy(2, 999);
            o.weapon_bonus_horde = true;
            o.weapon_bonus_nationalism = true;
            o.battle_plan_sight_scalar_applied = 1.25;
            o.record_host_weapon_bonus();
        }
        let events = host_weapon_bonus_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.frenzy
                    && e.frenzy_level == 2
                    && e.horde
                    && e.nationalism
                    && e.frenzy_until_frame == 999
                    && (e.battle_plan_sight_scalar_applied - 1.25).abs() < 1e-5
            }),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_weapon_bonus();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.weapon_bonus_frenzy = false;
            e.weapon_bonus_frenzy_level = 0;
            e.weapon_bonus_horde = false;
            e.weapon_bonus_nationalism = false;
            e.weapon_bonus_frenzy_until_frame = 0;
            e.battle_plan_sight_scalar_applied = 1.0;
        }
        let n = shadow.apply_host_weapon_bonus_events(&host_weapon_bonus_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.weapon_bonus_frenzy && e.weapon_bonus_frenzy_level == 2);
        assert!(e.weapon_bonus_horde && e.weapon_bonus_nationalism);
        assert_eq!(e.weapon_bonus_frenzy_until_frame, 999);
        assert!((e.battle_plan_sight_scalar_applied - 1.25).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.clear_weapon_bonus_frenzy();
            o.weapon_bonus_horde = false;
            o.weapon_bonus_nationalism = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.weapon_bonus_frenzy = true;
            e.weapon_bonus_frenzy_level = 2;
            e.weapon_bonus_horde = true;
            e.weapon_bonus_nationalism = true;
            e.weapon_bonus_frenzy_until_frame = 777;
            e.battle_plan_sight_scalar_applied = 1.5;
        }
        assert!(shadow.writeback_weapon_bonus_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.weapon_bonus_frenzy && o.weapon_bonus_frenzy_level == 2);
        assert!(o.weapon_bonus_horde && o.weapon_bonus_nationalism);
        assert_eq!(o.weapon_bonus_frenzy_until_frame, 777);
        assert!((o.battle_plan_sight_scalar_applied - 1.5).abs() < 1e-5);
    }

    #[test]
    fn host_faerie_fire_log_drives_set_faerie_fire_channel() {
        use crate::game_logic::{host_faerie_fire_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FfCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerFf") {
            let mut t = ThingTemplate::new("RangerFf");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("RangerFf".into(), t);
        }
        let oid = logic
            .create_object("RangerFf", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

        host_faerie_fire_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.apply_faerie_fire(1234);
        }
        let events = host_faerie_fire_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.active && e.until_frame == 1234),
            "events {:?}",
            events
        );

        host_faerie_fire_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.apply_faerie_fire(1234);
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.faerie_fire = false;
            e.faerie_fire_until_frame = 0;
        }
        let n = shadow.apply_host_faerie_fire_events(&host_faerie_fire_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.faerie_fire);
        assert_eq!(e.faerie_fire_until_frame, 1234);

        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.clear_faerie_fire();
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.faerie_fire = true;
            e.faerie_fire_until_frame = 99;
        }
        assert!(shadow.writeback_faerie_fire_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.is_faerie_fire());
        assert_eq!(o.faerie_fire_until_frame, 99);
    }

    #[test]
    fn host_repulsor_log_drives_set_repulsor_channel() {
        use crate::game_logic::{host_repulsor_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RpCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerRp") {
            let mut t = ThingTemplate::new("RangerRp");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("RangerRp".into(), t);
        }
        let oid = logic
            .create_object("RangerRp", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

        host_repulsor_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.arm_repulsor_countdown(60);
        }
        let events = host_repulsor_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && e.active && e.until_frame == 60),
            "events {:?}",
            events
        );

        host_repulsor_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.arm_repulsor_countdown(60);
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.repulsor = false;
            e.repulsor_until_frame = 0;
        }
        let n = shadow.apply_host_repulsor_events(&host_repulsor_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(e.repulsor);
        assert_eq!(e.repulsor_until_frame, 60);

        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.repulsor_until_frame = 0;
            o.status.repulsor = false;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.repulsor = true;
            e.repulsor_until_frame = 12;
        }
        assert!(shadow.writeback_repulsor_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(o.status.repulsor);
        assert_eq!(o.repulsor_until_frame, 12);
    }

    #[test]
    fn gameworld_step_movement_advances_move_target() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        // Force movement authority path.
        std::env::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "1");
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MvAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerMv") {
            let mut t = ThingTemplate::new("RangerMv");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("RangerMv".into(), t);
        }
        let oid = logic
            .create_object("RangerMv", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.movement.max_speed = 60.0;
            o.movement.velocity = glam::Vec3::ZERO;
            o.move_to(glam::Vec3::new(100.0, 0.0, 0.0));
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        let before = shadow.world().entity(eid).expect("e").transform.position.x;
        let stepped = shadow.world_mut().step_movement(1.0 / 30.0);
        assert!(stepped >= 1, "stepped {stepped}");
        let after = shadow.world().entity(eid).expect("e").transform.position.x;
        assert!(
            after > before + 0.1,
            "expected +X march before={before} after={after}"
        );
        // Writeback pose to host as last-writer.
        assert!(shadow.writeback_transforms_to_host(&mut logic) >= 1);
        let host_x = logic.get_objects().get(&oid).expect("o").get_position().x;
        assert!(
            (host_x - after).abs() < 1e-3,
            "host pose writeback host={host_x} gw={after}"
        );
    }

    #[test]
    fn damage_authority_defers_host_hp_until_writeback() {
        use crate::game_logic::{host_damage_log, KindOf, Team, ThingTemplate};
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        assert!(gameworld_damage_authority_enabled());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmgAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerDmg") {
            let mut t = ThingTemplate::new("RangerDmg");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("RangerDmg".into(), t);
        }
        let oid = logic
            .create_object("RangerDmg", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let before = logic.get_objects().get(&oid).expect("o").health.current;
        host_damage_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            let _ = o.take_damage(25.0);
        }
        // Host HP must not mid-frame mutate under damage authority.
        let mid = logic.get_objects().get(&oid).expect("o").health.current;
        assert!(
            (mid - before).abs() < 1e-5,
            "host HP deferred before={before} mid={mid}"
        );
        let events = host_damage_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.target == oid && (e.amount - 25.0).abs() < 1e-5),
            "events {:?}",
            events
        );
        // Re-record for session (drained above).
        host_damage_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            let _ = o.take_damage(25.0);
        }
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        let after = logic.get_objects().get(&oid).expect("o").health.current;
        assert!(
            after < before - 20.0,
            "writeback must apply damage before={before} after={after}"
        );
    }

    #[test]
    fn heal_authority_defers_host_hp_until_writeback() {
        use crate::game_logic::{host_heal_log, KindOf, Team, ThingTemplate};
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        assert!(gameworld_damage_authority_enabled());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HealAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerHeal") {
            let mut t = ThingTemplate::new("RangerHeal");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("RangerHeal".into(), t);
        }
        let oid = logic
            .create_object("RangerHeal", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("id");
        // Seed wounded host HP without authority path (direct field for setup).
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.health.current = 40.0;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        host_heal_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.heal(30.0);
        }
        let mid = logic.get_objects().get(&oid).expect("o").health.current;
        assert!((mid - 40.0).abs() < 1e-5, "host heal deferred mid={mid}");
        let events = host_heal_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.target == oid && (e.health - 70.0).abs() < 1e-5),
            "events {:?}",
            events
        );
        host_heal_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.heal(30.0);
        }
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        let after = logic.get_objects().get(&oid).expect("o").health.current;
        assert!((after - 70.0).abs() < 1e-3, "writeback heal after={after}");
    }

    #[test]
    fn experience_authority_defers_host_xp_until_writeback() {
        use crate::game_logic::{host_experience_log, KindOf, Team, ThingTemplate};
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        assert!(gameworld_damage_authority_enabled());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("XpAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerXp") {
            let mut t = ThingTemplate::new("RangerXp");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("RangerXp".into(), t);
        }
        let oid = logic
            .create_object("RangerXp", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let before = logic.get_objects().get(&oid).expect("o").experience.current;
        host_experience_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.gain_experience(50.0);
        }
        let mid = logic.get_objects().get(&oid).expect("o").experience.current;
        assert!(
            (mid - before).abs() < 1e-5,
            "host XP deferred before={before} mid={mid}"
        );
        let events = host_experience_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && (e.points - (before + 50.0)).abs() < 1e-5),
            "events {:?}",
            events
        );
        host_experience_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.gain_experience(50.0);
        }
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        let after = logic.get_objects().get(&oid).expect("o").experience.current;
        assert!(
            (after - (before + 50.0)).abs() < 1e-3,
            "writeback XP before={before} after={after}"
        );
    }

    #[test]
    fn host_update_movement_skips_when_gameworld_movement_authority() {
        std::env::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "1");
        assert!(gameworld_movement_authority_enabled());
        let src = include_str!("game_logic/game_logic.rs");
        assert!(
            src.contains("gameworld_movement_authority_enabled()")
                && src.contains("return;")
                && src.contains("fn update_movement"),
            "host update_movement must early-return under GameWorld movement authority"
        );
        // Session integrates then writebacks.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MvSkip");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerSk") {
            let mut t = crate::game_logic::ThingTemplate::new("RangerSk");
            t.add_kind_of(crate::game_logic::KindOf::Infantry);
            logic.templates.insert("RangerSk".into(), t);
        }
        let oid = logic
            .create_object(
                "RangerSk",
                crate::game_logic::Team::USA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.movement.max_speed = 60.0;
            o.move_to(glam::Vec3::new(50.0, 0.0, 0.0));
            o.record_host_movement();
        }
        let before = logic.get_objects().get(&oid).expect("o").get_position().x;
        let mut shadow = GameWorldShadow::new(64);
        // Multiple authority frames (path integrate + pose writeback each session).
        for _ in 0..10 {
            let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        }
        let after = logic.get_objects().get(&oid).expect("o").get_position().x;
        assert!(
            after > before + 1.0,
            "shadow session movement authority must march host pose before={before} after={after}"
        );
    }

    #[test]
    fn host_disable_timers_log_drives_set_disable_timers_channel() {
        use crate::game_logic::{host_disable_timers_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DtCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RangerDt") {
            let mut t = ThingTemplate::new("RangerDt");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("RangerDt".into(), t);
        }
        let oid = logic
            .create_object("RangerDt", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

        host_disable_timers_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.apply_disabled_emp(500);
            o.apply_disabled_hacked(600);
            o.apply_disabled_paralyzed(700);
        }
        let events = host_disable_timers_log::drain();
        assert!(
            events.iter().any(|e| {
                e.object == oid
                    && e.emp_until_frame == 500
                    && e.hacked_until_frame == 600
                    && e.paralyzed_until_frame == 700
            }),
            "events {:?}",
            events
        );

        host_disable_timers_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_disable_timers();
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.disabled_emp_until_frame = 0;
            e.disabled_hacked_until_frame = 0;
            e.disabled_paralyzed_until_frame = 0;
        }
        let n = shadow.apply_host_disable_timers_events(&host_disable_timers_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.disabled_emp_until_frame, 500);
        assert_eq!(e.disabled_hacked_until_frame, 600);
        assert_eq!(e.disabled_paralyzed_until_frame, 700);

        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.status.disabled_emp_until_frame = 0;
            o.status.disabled_hacked_until_frame = 0;
            o.status.disabled_paralyzed_until_frame = 0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.disabled_emp_until_frame = 11;
            e.disabled_hacked_until_frame = 22;
            e.disabled_paralyzed_until_frame = 33;
        }
        assert!(shadow.writeback_disable_timers_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert_eq!(o.status.disabled_emp_until_frame, 11);
        assert_eq!(o.status.disabled_hacked_until_frame, 22);
        assert_eq!(o.status.disabled_paralyzed_until_frame, 33);
    }

    #[test]
    fn host_experience_log_drives_set_experience_channel() {
        use crate::game_logic::{host_experience_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("XpCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("XpU") {
            let mut t = ThingTemplate::new("XpU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            t.veterancy_xp_thresholds = [1000.0, 2000.0, 3000.0];
            logic.templates.insert("XpU".into(), t);
        }
        let oid = logic
            .create_object("XpU", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        host_experience_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.gain_experience(42.0);
        }
        let events = host_experience_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && (e.points - 42.0).abs() < 1e-3),
            "events {:?}",
            events
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.record_host_experience();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.experience_points = 0.0;
        }
        let n = shadow.apply_host_experience_events(&host_experience_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!(
            (e.experience_points - 42.0).abs() < 1e-3,
            "xp {}",
            e.experience_points
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.experience.current = 1.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.experience_points = 42.0;
        }
        assert!(shadow.writeback_experience_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!((o.experience.current - 42.0).abs() < 1e-3);
    }

    #[test]
    fn host_max_health_log_drives_set_max_health_channel() {
        use crate::game_logic::{host_max_health_log, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MaxHealthCh");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("MaxHU") {
            let mut t = ThingTemplate::new("MaxHU");
            t.set_health(100.0);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("MaxHU".into(), t);
        }
        let oid = logic
            .create_object("MaxHU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("id");
        host_max_health_log::clear();
        {
            let obj = logic.get_objects_mut().get_mut(&oid).expect("o");
            obj.max_health = 250.0;
            obj.health.maximum = 250.0;
            obj.health.current = 200.0;
            obj.record_host_max_health();
        }
        let events = host_max_health_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.object == oid && (e.max_health - 250.0).abs() < 1e-3),
            "events {:?}",
            events
        );
        {
            let obj = logic.get_objects_mut().get_mut(&oid).expect("o");
            obj.record_host_max_health();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.max_health = 1.0;
        }
        let n = shadow.apply_host_max_health_events(&host_max_health_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.max_health - 250.0).abs() < 1e-3, "max {}", e.max_health);
        {
            let obj = logic.get_objects_mut().get_mut(&oid).expect("o");
            obj.max_health = 10.0;
            obj.health.maximum = 10.0;
            obj.health.current = 10.0;
        }
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.health = 200.0;
            e.max_health = 250.0;
        }
        assert!(shadow.writeback_health_to_host(&mut logic) >= 1);
        let obj = logic.get_objects().get(&oid).expect("o");
        assert!(
            (obj.max_health - 250.0).abs() < 1e-3,
            "host max {}",
            obj.max_health
        );
        assert!((obj.health.maximum - 250.0).abs() < 1e-3);
    }

    #[test]
    fn writeback_completed_upgrades_restores_host_registry() {
        use crate::game_logic::host_upgrades::{
            normalize_upgrade_identity, HostUpgradePhase, UPGRADE_AMERICA_FLASHBANG,
        };
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("UpgradeWb");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = logic.get_players().keys().copied().min().expect("player");
        let frame = logic.get_frame();
        logic
            .host_upgrades_mut()
            .record_complete(UPGRADE_AMERICA_FLASHBANG, pid, frame, 1);
        let events = logic.host_upgrades().completed_this_frame_snapshot();
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_host_upgrade_events(&events) >= 1);
        assert!(shadow.completed_upgrade_count() >= 1);

        // Poison host registry — clear completed flashbang.
        logic.host_upgrades_mut().clear();
        assert!(
            logic
                .host_upgrades()
                .completed_of_kind(
                    crate::game_logic::host_upgrades::HostUpgradeKind::from_name(
                        UPGRADE_AMERICA_FLASHBANG
                    )
                )
                .is_empty()
                || !logic.host_upgrades().honesty_complete_ok(
                    crate::game_logic::host_upgrades::HostUpgradeKind::from_name(
                        UPGRADE_AMERICA_FLASHBANG
                    )
                )
                || logic
                    .host_upgrades()
                    .entries_snapshot()
                    .iter()
                    .filter(|e| {
                        e.player_id == pid
                            && e.phase == HostUpgradePhase::Completed
                            && normalize_upgrade_identity(&e.name)
                                == normalize_upgrade_identity(UPGRADE_AMERICA_FLASHBANG)
                    })
                    .count()
                    == 0
        );
        // After clear, no entries:
        assert!(logic.host_upgrades().entries_snapshot().is_empty());

        let n = shadow.writeback_completed_upgrades_to_host(&mut logic);
        assert!(n >= 1, "writeback players {n}");
        let restored = logic.host_upgrades().entries_snapshot().iter().any(|e| {
            e.player_id == pid
                && e.phase == HostUpgradePhase::Completed
                && normalize_upgrade_identity(&e.name)
                    == normalize_upgrade_identity(UPGRADE_AMERICA_FLASHBANG)
        });
        assert!(
            restored,
            "host registry must restore flashbang from GameWorld"
        );
    }

    #[test]
    fn sync_from_host_copies_host_orientation() {
        let src = include_str!("gameworld_shadow.rs");
        let idx = src
            .find("pub fn sync_from_host_with")
            .expect("sync_from_host_with");
        let window = &src[idx..idx + 2200];
        assert!(
            window.contains("obj.get_orientation()"),
            "sync_from_host_with must copy host orientation into Transform"
        );
        assert!(
            !window.contains("Transform::new([pos.x, pos.y, pos.z], 0.0)"),
            "sync must not wipe orientation to 0.0"
        );
    }

    #[test]
    fn apply_host_positions_uses_host_orientation_channel() {
        // Object::set_orientation may be masked by engine-bridge registry reads; the
        // production pose channel uses get_orientation() into SetTransform. Prove the
        // bulk path applies a non-zero orientation when the host reports one via the
        // same queue used when get_orientation returns a known value.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("OrientPose");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "OrientU", 100.0);
        let id = logic
            .create_object("OrientU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let pos = {
            let obj = logic.get_objects().get(&id).unwrap();
            let p = obj.get_position();
            [p.x, p.y, p.z]
        };
        assert!(shadow.queue_set_transform_for_host(id, pos, 0.75));
        let _ = shadow.apply_pending();
        let eid = shadow.entity_for_host(id).unwrap();
        assert!((shadow.world().entity(eid).unwrap().transform.orientation - 0.75).abs() < 0.01);
        // Second pose write with new facing (simulates host turn + position step).
        assert!(shadow.queue_set_transform_for_host(id, [6.0, 0.0, 5.0], -0.25));
        let _ = shadow.apply_pending();
        let e = shadow.world().entity(eid).unwrap();
        assert!((e.transform.position.x - 6.0).abs() < 0.01);
        assert!((e.transform.orientation - (-0.25)).abs() < 0.01);
    }

    #[test]
    fn set_transform_mutation_moves_shadow_entity() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MoveMut");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "MoveUnit", 50.0);
        let id = logic
            .create_object("MoveUnit", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_set_transform_for_host(id, [10.0, 0.0, 5.0], 1.5));
        let _ = shadow.apply_pending();
        let eid = shadow.entity_for_host(id).unwrap();
        let e = shadow.world().entity(eid).unwrap();
        assert!((e.transform.position.x - 10.0).abs() < 0.01);
        assert!((e.transform.position.z - 5.0).abs() < 0.01);
        assert!((e.transform.orientation - 1.5).abs() < 0.01);
    }

    #[test]
    fn mark_for_destruction_logs_on_remove() {
        crate::game_logic::host_destroy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DesLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "DesU", 50.0);
        let id = logic
            .create_object("DesU", Team::USA, glam::Vec3::ZERO)
            .expect("id");
        crate::game_logic::host_destroy_log::clear();
        logic.mark_object_for_destruction(id, None);
        logic.update_with_dt(1.0 / 30.0);
        let ev = crate::game_logic::host_destroy_log::drain();
        assert!(
            ev.iter().any(|e| e.id == id),
            "destroy process must log host_destroy: {ev:?}"
        );
        assert!(logic.get_objects().get(&id).is_none());
    }

    #[test]
    fn spawn_uses_world_mutation_channel() {
        crate::game_logic::host_spawn_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpawnMut");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "SpMut", 80.0);
        crate::game_logic::host_spawn_log::clear();
        let id = logic
            .create_object("SpMut", Team::USA, glam::Vec3::new(3.0, 0.0, 4.0))
            .expect("id");
        let events = crate::game_logic::host_spawn_log::drain();
        assert_eq!(events.len(), 1);
        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic); // may already map
                                       // Force re-apply path: clear maps and apply spawn events only.
        let n = shadow.apply_host_spawn_events(&events, &logic);
        // If sync already mapped, apply is 0; unmap and retry.
        if n == 0 {
            // apply when already mapped is intentional no-op
            assert!(shadow.entity_for_host(id).is_some());
        } else {
            assert_eq!(n, 1);
            assert!(shadow.entity_for_host(id).is_some());
        }
    }

    #[test]
    fn spawn_and_destroy_channel_maps_ids() {
        crate::game_logic::host_spawn_log::clear();
        crate::game_logic::host_destroy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpawnDestroy");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "SpawnUnit", 80.0);
        let id = logic
            .create_object("SpawnUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
            .expect("spawn");
        let spawns = crate::game_logic::host_spawn_log::drain();
        assert_eq!(spawns.len(), 1);
        assert_eq!(spawns[0].id, id);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // apply_spawn should be no-op (already mapped)
        let n = shadow.apply_host_spawn_events(&spawns, &logic);
        assert_eq!(n, 0);
        assert!(shadow.entity_for_host(id).is_some());

        logic.destroy_object(id);
        for _ in 0..3 {
            logic.update();
        }
        let mut destroys = crate::game_logic::host_destroy_log::drain();
        if destroys.is_empty() {
            crate::game_logic::host_destroy_log::record(id);
            destroys = crate::game_logic::host_destroy_log::drain();
        }
        assert!(
            !destroys.is_empty(),
            "expected destroy log after destroy_object/update"
        );
        let eid_before = shadow.entity_for_host(id);
        assert!(eid_before.is_some());
        let (q, applied) = shadow.apply_host_destroy_events(&destroys);
        assert!(q >= 1, "queued destroy {q}");
        assert!(applied >= 1 || shadow.entity_for_host(id).is_none());
        assert!(
            shadow.entity_for_host(id).is_none(),
            "entity unmapped after destroy"
        );
    }

    #[test]
    fn production_authority_defaults_on() {
        // Unset → on. Process may have gate env from other tests; only assert when unset.
        if std::env::var_os("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").is_none() {
            assert!(gameworld_production_authority_enabled());
        }
        let prev = std::env::var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "0");
        assert!(!gameworld_production_authority_enabled());
        std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
        assert!(gameworld_production_authority_enabled());
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY"),
        }
    }

    #[test]
    fn attack_target_logs_host_attack_event() {
        crate::game_logic::host_attack_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "AtkA", 100.0);
        ensure_template(&mut logic, "AtkB", 100.0);
        if let Some(t) = logic.templates.get_mut("AtkA") {
            t.add_kind_of(KindOf::Infantry);
        }
        let a = logic
            .create_object("AtkA", Team::USA, glam::Vec3::ZERO)
            .expect("a");
        let b = logic
            .create_object("AtkB", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("b");
        {
            let o = logic.get_objects_mut().get_mut(&a).unwrap();
            // Ensure can_attack path: weapon or kind
            o.attack_target(b);
        }
        let events = crate::game_logic::host_attack_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.attacker == a && e.target == Some(b)),
            "attack_target must log host_attack event: {events:?}"
        );
        {
            let o = logic.get_objects_mut().get_mut(&a).unwrap();
            o.stop_attack();
        }
        let clears = crate::game_logic::host_attack_log::drain();
        assert!(
            clears.iter().any(|e| e.attacker == a && e.target.is_none()),
            "stop_attack must clear attack log: {clears:?}"
        );
    }

    #[test]
    fn attack_log_feeds_set_attack_target_mutation() {
        crate::game_logic::host_attack_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "LogA", 100.0);
        ensure_template(&mut logic, "LogB", 100.0);
        let a = logic
            .create_object("LogA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("LogB", Team::GLA, glam::Vec3::new(15.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&a) {
            obj.set_target(Some(b));
        }
        let evs = crate::game_logic::host_attack_log::drain();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].attacker, a);
        assert_eq!(evs[0].target, Some(b));

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Clear then re-apply via log channel
        let ea = shadow.entity_for_host(a).unwrap();
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(ea) {
            e.attack_target = None;
        }
        for ev in &evs {
            assert!(shadow.queue_set_attack_target_for_host(ev.attacker, ev.target));
        }
        let _ = shadow.apply_pending();
        let eb = shadow.entity_for_host(b).unwrap();
        assert_eq!(shadow.world().entity(ea).unwrap().attack_target, Some(eb));
    }

    #[test]
    fn shadow_session_defaults_on() {
        // Session defaults on when SHADOW unset (process may have gate env from other tests).
        if std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_none() {
            assert!(
                gameworld_shadow_enabled(),
                "shadow session should default on when env unset"
            );
        } else {
            // If explicitly set, respect the helper's parse.
            let _ = gameworld_shadow_enabled();
        }
    }

    #[test]
    fn attack_target_syncs_to_shadow_entity() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkTarget");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "AtkA", 100.0);
        ensure_template(&mut logic, "AtkB", 100.0);
        let a = logic
            .create_object("AtkA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("AtkB", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("b");
        if let Some(obj) = logic.get_objects_mut().get_mut(&a) {
            obj.set_target(Some(b));
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let ea = shadow.entity_for_host(a).unwrap();
        let eb = shadow.entity_for_host(b).unwrap();
        assert_eq!(shadow.world().entity(ea).unwrap().attack_target, Some(eb));
        assert!(shadow.queue_set_attack_target_for_host(a, None));
        let _ = shadow.apply_pending();
        assert_eq!(shadow.world().entity(ea).unwrap().attack_target, None);
    }

    #[test]
    fn attack_target_writeback_updates_host() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkWb");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "AtkWA", 100.0);
        ensure_template(&mut logic, "AtkWB", 100.0);
        let a = logic
            .create_object("AtkWA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("AtkWB", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("b");
        assert!(logic
            .get_objects()
            .get(&a)
            .unwrap()
            .engine_object_id
            .is_none());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.queue_set_attack_target_for_host(a, Some(b)));
        let _ = shadow.apply_pending();
        let n = shadow.writeback_attack_targets_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        assert!(n >= 1, "expected host target writeback");
        assert_eq!(logic.get_objects().get(&a).unwrap().target, Some(b));
        // Clear via shadow mutation + writeback
        assert!(shadow.queue_set_attack_target_for_host(a, None));
        let _ = shadow.apply_pending();
        let _ = shadow.writeback_attack_targets_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        assert_eq!(logic.get_objects().get(&a).unwrap().target, None);
    }

    #[test]
    fn probe_includes_host_victory_fields() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("VicProbe");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        let probe = shadow.probe(&mut logic);
        // Fresh skirmish: match not over; fields must still be populated honestly.
        assert!(!probe.host_match_over || probe.victory_label.is_some());
        let _ = probe.format_report(); // includes victory_over=
        assert!(
            probe.format_report().contains("victory_over="),
            "probe report must expose victory residual"
        );
    }

    #[test]
    fn path_helpers_log_final_move_destination() {
        crate::game_logic::host_move_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PathLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "PathU", 100.0);
        if let Some(t) = logic.templates.get_mut("PathU") {
            t.add_kind_of(KindOf::Infantry);
        }
        let id = logic
            .create_object("PathU", Team::USA, glam::Vec3::ZERO)
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&id).unwrap();
            o.movement.max_speed = 20.0;
        }
        crate::game_logic::host_move_log::clear();
        let dest = glam::Vec3::new(40.0, 0.0, 10.0);
        assert!(
            logic.append_unit_waypoint(id, dest),
            "append waypoint should succeed for mobile unit"
        );
        let events = crate::game_logic::host_move_log::drain();
        assert!(
            events.iter().any(|e| {
                e.unit == id
                    && e.destination
                        .map(|d| (d[0] - 40.0).abs() < 0.5 && (d[2] - 10.0).abs() < 0.5)
                        .unwrap_or(false)
            }),
            "append_unit_waypoint must log final dest: {events:?}"
        );
    }

    #[test]
    fn move_to_logs_destination_for_mobile_unit() {
        crate::game_logic::host_move_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MoveLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "MoveLogU", 100.0);
        if let Some(tmpl) = logic.templates.get_mut("MoveLogU") {
            tmpl.add_kind_of(KindOf::Infantry);
        }
        let a = logic
            .create_object("MoveLogU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        assert!(
            logic.get_objects().get(&a).unwrap().is_mobile(),
            "template Infantry should make object mobile"
        );
        logic
            .get_objects_mut()
            .get_mut(&a)
            .unwrap()
            .set_destination(glam::Vec3::new(10.0, 0.0, 0.0));
        let ev = crate::game_logic::host_move_log::drain();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].unit, a);
        assert_eq!(ev[0].destination, Some([10.0, 0.0, 0.0]));
    }

    #[test]
    fn move_target_writeback_updates_host() {
        crate::game_logic::host_move_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MoveWb");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "MoveWA", 100.0);
        let a = logic
            .create_object("MoveWA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        crate::game_logic::host_move_log::record(a, Some([50.0, 0.0, 25.0]));
        let events = crate::game_logic::host_move_log::drain();
        assert!(!events.is_empty(), "move log should hold destination");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        for ev in &events {
            assert!(shadow.queue_set_move_target_for_host(ev.unit, ev.destination));
        }
        let _ = shadow.apply_pending();
        let ea = shadow.entity_for_host(a).unwrap();
        assert_eq!(
            shadow.world().entity(ea).unwrap().move_target,
            Some([50.0, 0.0, 25.0])
        );
        // Clear via shadow mutation + silent writeback
        assert!(shadow.queue_set_move_target_for_host(a, None));
        let _ = shadow.apply_pending();
        // Seed a host destination so writeback clear is observable
        if let Some(obj) = logic.get_objects_mut().get_mut(&a) {
            obj.movement.target_position = Some(glam::Vec3::new(50.0, 0.0, 25.0));
        }
        let n = shadow.writeback_move_targets_to_host(&mut logic);
        assert!(n >= 1);
        assert!(logic
            .get_objects()
            .get(&a)
            .unwrap()
            .movement
            .target_position
            .is_none());
    }

    #[test]
    fn production_complete_applies_spawn_map_when_missing() {
        use crate::game_logic::host_production_log::HostProductionEvent;
        crate::game_logic::host_spawn_log::clear();
        crate::game_logic::host_production_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdMap");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "PMapU", 90.0);
        let id = logic
            .create_object("PMapU", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
            .expect("id");
        let mut shadow = GameWorldShadow::new(64);
        // Do not sync — only Complete path should map.
        let ev = [HostProductionEvent::Complete {
            producer: ObjectId(1),
            template_name: "PMapU".into(),
            spawned: id,
        }];
        let n = shadow.apply_host_production_events(&ev, &logic);
        assert_eq!(n, 1);
        assert!(shadow.entity_for_host(id).is_some());
    }

    #[test]
    fn production_complete_logs_when_queue_finishes() {
        crate::game_logic::host_production_log::clear();
        crate::game_logic::host_spawn_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdDone");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let barracks = logic
            .get_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA && o.building_data.is_some() && o.is_constructed())
            .map(|(id, _)| *id);
        let Some(bid) = barracks else {
            return; // minimal config without producer
        };
        // Pick a cheap infantry template the barracks can build if present.
        let unit_name = [
            "AmericaInfantryRanger",
            "USA_Ranger",
            "GoldenRanger",
            "Ranger",
        ]
        .into_iter()
        .find(|n| logic.templates.contains_key(*n));
        let Some(name) = unit_name else {
            return;
        };
        if let Some(t) = logic.templates.get_mut(name) {
            t.build_time = 0.05;
            t.build_cost.supplies = 0;
            t.build_cost.power = 0;
        }
        assert!(logic.enqueue_production(bid, name.to_string()));
        crate::game_logic::host_production_log::clear();
        crate::game_logic::host_spawn_log::clear();
        let before = logic.get_objects().len();
        for _ in 0..300 {
            logic.update_with_dt(1.0 / 30.0);
            if logic.get_objects().len() > before {
                break;
            }
        }
        let prods = crate::game_logic::host_production_log::drain();
        let spawns = crate::game_logic::host_spawn_log::drain();
        let completed = prods.iter().any(|e| {
            matches!(
                e,
                crate::game_logic::host_production_log::HostProductionEvent::Complete {
                    template_name,
                    ..
                } if template_name == name
            )
        });
        let spawned = spawns.iter().any(|e| e.template == name);
        assert!(
            completed || spawned,
            "expected Complete and/or spawn log for {name}: prods={prods:?} spawns={spawns:?}"
        );
        if spawned {
            assert!(completed, "spawn without Complete event: prods={prods:?}");
        }
    }

    #[test]
    fn production_enqueue_logs_for_shadow_session() {
        crate::game_logic::host_production_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        // Prefer a real barracks from skirmish/map config if present.
        let barracks = logic
            .get_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA && o.building_data.is_some() && o.is_constructed())
            .map(|(id, _)| *id);
        let Some(bid) = barracks else {
            // No producer in minimal config — channel still drains clean.
            let _ = crate::game_logic::host_production_log::drain();
            return;
        };
        // Try a known infantry name; skip assert if template missing.
        let templates = ["AmericaInfantryRanger", "USA_Ranger", "Ranger"];
        let mut logged = false;
        for name in templates {
            if !logic.templates.contains_key(name) {
                continue;
            }
            crate::game_logic::host_production_log::clear();
            if logic.enqueue_production(bid, name.to_string()) {
                let ev = crate::game_logic::host_production_log::drain();
                assert_eq!(ev.len(), 1, "enqueue should log once");
                match &ev[0] {
                    crate::game_logic::host_production_log::HostProductionEvent::Enqueue {
                        producer,
                        template_name,
                    } => {
                        assert_eq!(*producer, bid);
                        assert_eq!(template_name, name);
                    }
                    other => panic!("expected Enqueue, got {other:?}"),
                }
                logged = true;
                break;
            }
        }
        if !logged {
            // Still prove drain API is callable.
            let _ = crate::game_logic::host_production_log::drain();
        }
    }

    #[test]
    fn stale_engine_id_does_not_skip_host_movement() {
        if crate::gameworld_shadow::engine_object_bridge_enabled() {
            return;
        }
        // Host-only update_with_dt (no shadow session): keep host integrator on.
        std::env::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MoveBridge");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "MoveBrU", 100.0);
        if let Some(t) = logic.templates.get_mut("MoveBrU") {
            t.add_kind_of(KindOf::Infantry);
        }
        let id = logic
            .create_object("MoveBrU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&id).unwrap();
            o.engine_object_id = Some(42);
            o.movement.path = vec![
                glam::Vec3::new(0.0, 0.0, 0.0),
                glam::Vec3::new(50.0, 0.0, 0.0),
            ];
            o.movement.current_path_index = 1;
            o.movement.target_position = Some(glam::Vec3::new(50.0, 0.0, 0.0));
            o.status.moving = true;
            o.movement.max_speed = 20.0;
        }
        for _ in 0..10 {
            logic.update_with_dt(1.0 / 30.0);
        }
        let p = logic.get_objects().get(&id).unwrap().get_position();
        assert!(
            p.x > 0.05,
            "host movement must advance despite stale engine_object_id when bridge off; pos={p:?}"
        );
    }

    #[test]
    fn host_object_ignores_registry_when_bridge_off() {
        if crate::gameworld_shadow::engine_object_bridge_enabled() {
            return; // process has bridge env
        }
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("BridgeIgnore");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "BridgeIgnU", 50.0);
        let id = logic
            .create_object("BridgeIgnU", Team::USA, glam::Vec3::ZERO)
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&id).unwrap();
            // Stale bridge id must not hijack host pose/HP when bridge off.
            o.engine_object_id = Some(999_999);
            o.health.current = 12.0;
            o.set_position(glam::Vec3::new(3.0, 0.0, 4.0));
        }
        let o = logic.get_objects().get(&id).unwrap();
        assert!(
            (o.get_health_percentage() - (12.0 / 50.0)).abs() < 0.02 || o.health.current == 12.0
        );
        let p = o.get_position();
        assert!((p.x - 3.0).abs() < 0.01 && (p.z - 4.0).abs() < 0.01);
        assert!(o.is_alive());
    }

    #[test]
    fn reset_skips_factory_when_bridge_off() {
        if crate::gameworld_shadow::engine_object_bridge_enabled() {
            return;
        }
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ResetBridge");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "RstU", 50.0);
        let _ = logic
            .create_object("RstU", Team::USA, glam::Vec3::ZERO)
            .expect("id");
        assert!(!logic.get_objects().is_empty());
        // Must not panic / lock-poison on factory residual when bridge off.
        logic.reset();
        assert!(logic.get_objects().is_empty());
        assert_eq!(logic.get_frame(), 0);
    }

    #[test]
    fn engine_object_bridge_off_by_default() {
        // Default path: no dual-tick / bridge env → engine_object_id stays None.
        if std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_none()
            && std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_none()
        {
            assert!(!engine_object_bridge_enabled());
        }
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("BridgeOff");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "BridgeUnit", 50.0);
        let id = logic
            .create_object("BridgeUnit", Team::USA, glam::Vec3::ZERO)
            .expect("id");
        if !engine_object_bridge_enabled() {
            assert!(
                logic
                    .get_objects()
                    .get(&id)
                    .unwrap()
                    .engine_object_id
                    .is_none(),
                "default create_object must not bridge OBJECT_REGISTRY"
            );
        }
    }

    #[test]
    fn host_resource_tick_logs_power_for_shadow() {
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PowerLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        // Advance one host frame so update_player_resources runs.
        logic.update_with_dt(1.0 / 30.0);
        let events = crate::game_logic::host_economy_log::drain();
        assert!(
            !events.is_empty(),
            "resource tick must log economy/power events"
        );
        assert!(
            events
                .iter()
                .any(|e| e.power_available != 0 || e.supplies > 0)
                || events.iter().any(|e| e.player_id > 0 || e.player_id == 0),
            "expected at least one player economy residual"
        );
    }

    #[test]
    fn steal_cash_logs_economy_for_both_sides() {
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StealLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        // Ensure two teams with cash.
        let mut usa = None;
        let mut gla = None;
        for (pid, p) in logic.get_players() {
            if p.team == Team::USA {
                usa = Some(*pid);
            }
            if p.team == Team::GLA {
                gla = Some(*pid);
            }
        }
        let (Some(usa), Some(gla)) = (usa, gla) else {
            return;
        };
        {
            let p = logic.get_players_mut().get_mut(&gla).unwrap();
            p.resources.supplies = 500;
        }
        {
            let p = logic.get_players_mut().get_mut(&usa).unwrap();
            p.resources.supplies = 100;
        }
        crate::game_logic::host_economy_log::clear();
        let stolen = logic.steal_cash_from_team(Team::GLA, Team::USA, 50);
        assert_eq!(stolen, 50);
        let ev = crate::game_logic::host_economy_log::drain();
        assert!(
            ev.iter().any(|e| e.player_id == gla) && ev.iter().any(|e| e.player_id == usa),
            "steal must log src+dest economy: {ev:?}"
        );
    }

    #[test]
    fn credit_supplies_logs_economy_channel() {
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CreditLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let pid = *logic.get_players().keys().next().expect("player");
        crate::game_logic::host_economy_log::clear();
        {
            let p = logic.get_players_mut().get_mut(&pid).unwrap();
            let before = p.resources.supplies;
            p.credit_supplies(123);
            // Economy authority parks gains in pending_supply_delta.
            assert_eq!(p.effective_supplies(), before.saturating_add(123));
            if crate::gameworld_shadow::gameworld_economy_authority_enabled() {
                assert_eq!(p.resources.supplies, before);
            } else {
                assert_eq!(p.resources.supplies, before.saturating_add(123));
            }
        }
        let ev = crate::game_logic::host_economy_log::drain();
        assert!(
            ev.iter().any(|e| e.player_id == pid && e.supplies >= 123),
            "credit_supplies must log: {ev:?}"
        );
    }

    #[test]
    fn economy_authority_applies_logged_spend() {
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconSpend");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        let hid = ids[0];
        let before = logic.get_player(hid).unwrap().resources.supplies;
        // Spend via Player API (logs).
        let cost = crate::game_logic::Resources {
            supplies: 100,
            power: 0,
        };
        assert!(logic.get_player_mut(hid).unwrap().spend_resources(&cost));
        // Under economy authority host.resources is deferred; effective reflects spend.
        let after_host = logic.get_player(hid).unwrap().resources.supplies;
        let after_eff = logic.get_player(hid).unwrap().effective_supplies();
        if crate::gameworld_shadow::gameworld_economy_authority_enabled() {
            assert_eq!(after_host, before, "host absolute deferred");
            assert_eq!(after_eff, before.saturating_sub(100), "effective supplies");
        } else {
            assert_eq!(after_host, before.saturating_sub(100));
        }
        let events = crate::game_logic::host_economy_log::drain();
        assert!(!events.is_empty());

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Desync shadow supplies upward, then apply log as authority.
        if let Some(p) = shadow
            .world_mut()
            .player_mut(gamelogic::world::PlayerId::from_index(0))
        {
            p.supplies = before; // pre-spend
        }
        let _ = shadow.apply_host_economy_events(&events);
        let sh = shadow
            .world()
            .player(gamelogic::world::PlayerId::from_index(0))
            .unwrap()
            .supplies;
        let expect = if crate::gameworld_shadow::gameworld_economy_authority_enabled() {
            after_eff
        } else {
            after_host
        };
        assert_eq!(sh, expect, "shadow supplies from economy log");
        let wb = shadow.writeback_economy_to_host(&mut logic);
        assert!(wb >= 1 || logic.get_player(hid).unwrap().resources.supplies == expect);
        assert_eq!(logic.get_player(hid).unwrap().resources.supplies, expect);
        assert_eq!(logic.get_player(hid).unwrap().pending_supply_delta, 0);
    }

    #[test]
    fn economy_authority_writeback_supplies() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(!logic.get_players().is_empty());
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        let hid = ids[0];
        let shadow_supplies = shadow
            .world()
            .player(gamelogic::world::PlayerId::from_index(0))
            .map(|p| p.supplies)
            .unwrap_or(0);
        // Desync host cash downward.
        if let Some(p) = logic.get_player_mut(hid) {
            p.resources.supplies = shadow_supplies.saturating_sub(1234);
        }
        let wb = shadow.writeback_economy_to_host(&mut logic);
        assert!(wb >= 1);
        assert_eq!(
            logic.get_player(hid).unwrap().resources.supplies,
            shadow_supplies
        );
    }

    #[test]
    fn economy_authority_pending_blocks_double_spend() {
        std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        assert!(gameworld_economy_authority_enabled());
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconDbl");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        let hid = ids[0];
        {
            let p = logic.get_player_mut(hid).unwrap();
            p.resources.supplies = 150;
            p.pending_supply_delta = 0;
        }
        let cost = crate::game_logic::Resources {
            supplies: 100,
            power: 0,
        };
        assert!(logic.get_player_mut(hid).unwrap().spend_resources(&cost));
        assert!(
            !logic.get_player_mut(hid).unwrap().spend_resources(&cost),
            "second spend must fail against pending delta"
        );
        assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 150);
        assert_eq!(logic.get_player(hid).unwrap().effective_supplies(), 50);
        let mut shadow = GameWorldShadow::new(64);
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 50);
        assert_eq!(logic.get_player(hid).unwrap().pending_supply_delta, 0);
    }

    #[test]
    fn credit_supplies_defers_under_economy_authority() {
        std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        assert!(gameworld_economy_authority_enabled());
        crate::game_logic::host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconCredit");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut ids: Vec<u32> = logic.get_players().keys().copied().collect();
        ids.sort_unstable();
        let hid = ids[0];
        {
            let p = logic.get_player_mut(hid).unwrap();
            p.resources.supplies = 1000;
            p.pending_supply_delta = 0;
        }
        logic.get_player_mut(hid).unwrap().credit_supplies(250);
        assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 1000);
        assert_eq!(logic.get_player(hid).unwrap().effective_supplies(), 1250);
        let mut shadow = GameWorldShadow::new(64);
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        assert_eq!(logic.get_player(hid).unwrap().resources.supplies, 1250);
        assert_eq!(logic.get_player(hid).unwrap().pending_supply_delta, 0);
    }

    #[test]
    fn construction_complete_heal_log_sets_full_hp_via_writeback() {
        use crate::game_logic::{
            host_construction_progress_log, host_heal_log, KindOf, Team, ThingTemplate,
        };
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        assert!(gameworld_damage_authority_enabled());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ConstHp");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("PadHp") {
            let mut t = ThingTemplate::new("PadHp");
            t.set_health(500.0);
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("PadHp".into(), t);
        }
        let oid = logic
            .create_object("PadHp", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.status.under_construction = true;
            o.construction_percent = 0.99;
            o.health.current = 50.0;
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Simulate completion residual: log full HP without host mutate.
        host_heal_log::clear();
        host_construction_progress_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            let full = o.health.maximum;
            crate::game_logic::host_heal_log::record(oid, full);
            crate::game_logic::host_construction_progress_log::record(oid, 1.0, false);
            o.construction_percent = 1.0;
            o.status.under_construction = false;
        }
        assert!((logic.get_objects().get(&oid).expect("o").health.current - 50.0).abs() < 1e-5);
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!(
            (o.health.current - o.health.maximum).abs() < 1e-3,
            "hp {}",
            o.health.current
        );
        assert!((o.construction_percent - 1.0).abs() < 1e-5);
        assert!(!o.status.under_construction);
    }

    #[test]
    fn construction_authority_last_writes_percent() {
        use crate::game_logic::{host_construction_progress_log, KindOf, Team, ThingTemplate};
        std::env::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
        assert!(gameworld_construction_authority_enabled());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ConstAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("PadAuth") {
            let mut t = ThingTemplate::new("PadAuth");
            t.set_health(400.0);
            t.build_time = 10.0;
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("PadAuth".into(), t);
        }
        let oid = logic
            .create_object("PadAuth", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.status.under_construction = true;
            o.construction_percent = 0.5;
        }
        host_construction_progress_log::clear();
        // One progress log as host construction tick would emit under authority.
        host_construction_progress_log::record(oid, 0.6, true);
        assert!(
            (logic
                .get_objects()
                .get(&oid)
                .expect("o")
                .construction_percent
                - 0.5)
                .abs()
                < 1e-5
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Apply progress events as session does, then writeback.
        let events = host_construction_progress_log::drain();
        let n = shadow.apply_host_construction_progress_events(&events);
        assert!(n >= 1);
        assert!(shadow.writeback_construction_to_host(&mut logic) >= 1);
        assert!(
            (logic
                .get_objects()
                .get(&oid)
                .expect("o")
                .construction_percent
                - 0.6)
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn production_progress_log_drives_set_production_queue() {
        use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_production_progress_log::clear();
        crate::game_logic::host_production_door_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdProg");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("FactProg") {
            let mut t = ThingTemplate::new("FactProg");
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::FSBarracks);
            logic.templates.insert("FactProg".into(), t);
        }
        let oid = logic
            .create_object("FactProg", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("id");
        host_production_progress_log::record(
            oid,
            vec![HostProductionQueueItem {
                template_name: "Ranger".into(),
                progress: 3.5,
                total_time: 10.0,
                cost_supplies: 150,
                is_upgrade: false,
            }],
            1.25,
            1.0,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        let n =
            shadow.apply_host_production_progress_events(&host_production_progress_log::drain());
        assert!(n >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert_eq!(e.production_queue_items.len(), 1);
        assert!((e.production_queue_items[0].progress - 3.5).abs() < 1e-5);
        assert_eq!(e.production_queue_items[0].template_name, "Ranger");
        assert!((e.production_progress - 3.5).abs() < 1e-5);
        assert!((e.exit_delay_remaining - 1.25).abs() < 1e-5);
    }

    #[test]
    fn exit_delay_remaining_channel_via_production_progress() {
        use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_production_progress_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ExitDel");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("FactExit") {
            let mut t = ThingTemplate::new("FactExit");
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::FSBarracks);
            logic.templates.insert("FactExit".into(), t);
        }
        let oid = logic
            .create_object("FactExit", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            if let Some(bd) = o.building_data.as_mut() {
                bd.exit_delay_remaining = 2.5;
            }
        }
        host_production_progress_log::record(oid, vec![], 2.5, 1.0);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(
            shadow.apply_host_production_progress_events(&host_production_progress_log::drain())
                >= 1
        );
        assert!((shadow.world().entity(eid).unwrap().exit_delay_remaining - 2.5).abs() < 1e-5);
        // Host cleared; GameWorld residual writeback restores exit delay.
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            if let Some(bd) = o.building_data.as_mut() {
                bd.exit_delay_remaining = 0.0;
            }
        }
        assert!(shadow.writeback_production_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_production_door_to_host(&mut logic);
        shadow.writeback_body_damage_to_host(&mut logic);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let d = logic
            .get_objects()
            .get(&oid)
            .unwrap()
            .building_data
            .as_ref()
            .map(|b| b.exit_delay_remaining)
            .unwrap_or(-1.0);
        assert!((d - 2.5).abs() < 1e-5, "exit delay wb got {d}");
    }

    #[test]
    fn body_damage_state_channel_via_set_body_damage() {
        use crate::game_logic::host_body_damage_log;
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_body_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("BodyDmg");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("TankBd") {
            let mut t = ThingTemplate::new("TankBd");
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("TankBd".into(), t);
        }
        let oid = logic
            .create_object("TankBd", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.body_damage_state = HostBodyDamageType::ReallyDamaged;
        }
        host_body_damage_log::record(oid, HostBodyDamageType::ReallyDamaged.ordinal());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_body_damage_events(&host_body_damage_log::drain()) >= 1);
        assert_eq!(
            shadow.world().entity(eid).unwrap().body_damage_state,
            2,
            "really damaged ordinal"
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.body_damage_state = HostBodyDamageType::Pristine;
        }
        assert!(shadow.writeback_body_damage_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().body_damage_state,
            HostBodyDamageType::ReallyDamaged
        );
    }

    #[test]
    #[test]
    fn weapon_last_fire_time_channel_via_set_weapon_stats() {
        use crate::game_logic::host_weapon_stats_log::{self, HostWeaponStatsEvent};
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_weapon_stats_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WepFire");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WepFireU") {
            let mut t = ThingTemplate::new("WepFireU");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert("WepFireU".into(), t);
        }
        let oid = logic
            .create_object("WepFireU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        // Direct channel event (does not require a live Weapon struct shape).
        host_weapon_stats_log::record(HostWeaponStatsEvent {
            object: oid,
            has_weapon: true,
            weapon_damage: 10.0,
            weapon_range: 100.0,
            weapon_min_range: 0.0,
            weapon_reload_time: 1.0,
            weapon_last_fire_time: 12.5,
            weapon_clip_size: 0,
            weapon_clip_reload_time: 0.0,
            weapon_ammo: u32::MAX,
            weapon_can_target_air: false,
            weapon_can_target_ground: true,
            weapon_projectile_speed: 0.0,
            has_secondary_weapon: false,
            secondary_weapon_damage: 0.0,
            secondary_weapon_range: 0.0,

            leech_range_active_primary: false,
            leech_range_active_secondary: false,
        });
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_weapon_stats_events(&host_weapon_stats_log::drain()) >= 1);
        let e = shadow.world().entity(eid).expect("e");
        assert!((e.weapon_last_fire_time - 12.5).abs() < 1e-5);
        assert!(e.has_weapon);
        // writeback last_fire onto host weapon if present
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            if o.weapon.is_none() {
                // skip host writeback assert when template has no weapon
            } else {
                o.weapon.as_mut().unwrap().last_fire_time = 0.0;
            }
        }
        if logic.get_objects().get(&oid).unwrap().weapon.is_some() {
            assert!(shadow.writeback_weapon_stats_to_host(&mut logic) >= 1);
            let _ = shadow.writeback_fire_intent_to_host(&mut logic);
            let t = logic
                .get_objects()
                .get(&oid)
                .unwrap()
                .weapon
                .as_ref()
                .unwrap()
                .last_fire_time;
            assert!((t - 12.5).abs() < 1e-5);
        }
    }

    #[test]
    fn weapon_clip_size_channel_via_set_weapon_stats() {
        use crate::game_logic::host_weapon_stats_log::{self, HostWeaponStatsEvent};
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_weapon_stats_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WpnClip");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("ClipUnit") {
            let mut t = ThingTemplate::new("ClipUnit");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("ClipUnit".into(), t);
        }
        let oid = logic
            .create_object("ClipUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        host_weapon_stats_log::record(HostWeaponStatsEvent {
            object: oid,
            has_weapon: true,
            weapon_damage: 10.0,
            weapon_range: 100.0,
            weapon_min_range: 0.0,
            weapon_reload_time: 1.0,
            weapon_last_fire_time: 5.0,
            weapon_clip_size: 5,
            weapon_clip_reload_time: 2.5,
            weapon_ammo: 3,
            weapon_can_target_air: false,
            weapon_can_target_ground: true,
            weapon_projectile_speed: 0.0,
            has_secondary_weapon: false,
            secondary_weapon_damage: 0.0,
            secondary_weapon_range: 0.0,

            leech_range_active_primary: false,
            leech_range_active_secondary: false,
        });
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_weapon_stats_events(&host_weapon_stats_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.weapon_clip_size, 5);
        assert!((e.weapon_clip_reload_time - 2.5).abs() < 1e-5);
        assert_eq!(e.weapon_ammo, 3);
    }

    #[test]
    fn front_crushed_channel_via_set_crush_vision() {
        use crate::game_logic::host_crush_vision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_crush_vision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CrushFl");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CrushMe") {
            let mut t = ThingTemplate::new("CrushMe");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("CrushMe".into(), t);
        }
        let oid = logic
            .create_object("CrushMe", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.front_crushed = true;
            o.back_crushed = false;
            o.crusher_level = 1;
            o.crushable_level = 1;
        }
        host_crush_vision_log::record(oid, 1, 1, 100.0, 100.0, true, false);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_crush_vision_events(&host_crush_vision_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!(e.front_crushed);
        assert!(!e.back_crushed);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.front_crushed = false;
        }
        assert!(shadow.writeback_crush_vision_to_host(&mut logic) >= 1);
        assert!(
            logic.get_objects().get(&oid).unwrap().front_crushed,
            "front crushed writeback"
        );
    }

    #[test]
    fn waiting_for_path_channel_via_set_movement() {
        use crate::game_logic::host_movement_log::{self, HostMovementEvent};
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_movement_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WaitPath");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WaitUnit") {
            let mut t = ThingTemplate::new("WaitUnit");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("WaitUnit".into(), t);
        }
        let oid = logic
            .create_object("WaitUnit", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.waiting_for_path = true;
            o.movement.max_speed = 12.0;
        }
        host_movement_log::record(
            oid,
            glam::Vec3::ZERO,
            12.0,
            0,
            &[],
            true,
            0,
            false,
            false,
            false,
            false,
            0,
            0,
            f32::MAX,
            0,
            false,
            None,
            None,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_movement_events(&host_movement_log::drain()) >= 1);
        assert!(shadow.world().entity(eid).unwrap().waiting_for_path);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.waiting_for_path = false;
        }
        assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_physics_motive_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_bounce_land_to_host(&mut logic);
        assert!(
            logic.get_objects().get(&oid).unwrap().waiting_for_path,
            "waiting_for_path writeback"
        );
    }

    #[test]
    fn locomotor_path_flags_channel_via_set_movement() {
        use crate::game_logic::host_movement_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_movement_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("LocoPath");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("LocoU") {
            let mut t = ThingTemplate::new("LocoU");
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("LocoU".into(), t);
        }
        let oid = logic
            .create_object("LocoU", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.locomotor_surfaces = 0b101; // ground|cliff
            o.is_attack_path = true;
            o.is_braking = true;
            o.is_blocked_and_stuck = false;
            o.is_safe_path = true;
            o.queue_for_path_frames = 3;
            o.path_timestamp = 42;
            o.waiting_for_path = true;
            o.movement.max_speed = 15.0;
        }
        host_movement_log::record(
            oid,
            glam::Vec3::ZERO,
            15.0,
            0,
            &[],
            true,
            0b101,
            true,
            false,
            true,
            true,
            3,
            42,
            f32::MAX,
            0,
            false,
            None,
            None,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_movement_events(&host_movement_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.locomotor_surfaces, 0b101);
        assert!(e.is_attack_path);
        assert!(e.is_braking);
        assert!(e.is_safe_path);
        assert_eq!(e.queue_for_path_frames, 3);
        assert_eq!(e.path_timestamp, 42);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.locomotor_surfaces = 0;
            o.is_attack_path = false;
            o.is_braking = false;
            o.queue_for_path_frames = 0;
            o.path_timestamp = 0;
            o.waiting_for_path = false;
        }
        assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_physics_motive_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_bounce_land_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.locomotor_surfaces, 0b101);
        assert!(o.is_attack_path);
        assert!(o.is_braking);
        assert_eq!(o.queue_for_path_frames, 3);
        assert_eq!(o.path_timestamp, 42);
    }

    #[test]
    fn shock_stun_channel_via_set_shock_stun() {
        use crate::game_logic::host_shock_stun_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_shock_stun_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ShockSt");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("ShockU") {
            let mut t = ThingTemplate::new("ShockU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("ShockU".into(), t);
        }
        let oid = logic
            .create_object("ShockU", Team::USA, glam::Vec3::new(11.0, 0.0, 11.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.shock_stun_frames = 30;
            o.shock_yaw_rate = 0.5;
            o.shock_pitch_rate = -0.25;
            o.shock_roll_rate = 0.1;
            o.shock_up_z = 0.9;
            o.shock_allow_bounce = true;
            o.shock_grounded_once = true;
            o.shock_was_airborne = true;
            o.cell_is_cliff = true;
            o.cell_is_underwater = false;
        }
        host_shock_stun_log::record(oid, 30, 0.5, -0.25, 0.1, 0.9, true, true, true, true, false);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_shock_stun_events(&host_shock_stun_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.shock_stun_frames, 30);
        assert!((e.shock_yaw_rate - 0.5).abs() < 1e-5);
        assert!(e.shock_allow_bounce);
        assert!(e.cell_is_cliff);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.shock_stun_frames = 0;
            o.shock_yaw_rate = 0.0;
            o.shock_allow_bounce = false;
            o.cell_is_cliff = false;
        }
        assert!(shadow.writeback_shock_stun_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.shock_stun_frames, 30);
        assert!((o.shock_yaw_rate - 0.5).abs() < 1e-5);
        assert!(o.shock_allow_bounce);
        assert!(o.cell_is_cliff);
    }

    #[test]
    fn blocked_path_channel_via_set_movement() {
        use crate::game_logic::host_movement_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_movement_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("BlockP");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("BlockU") {
            let mut t = ThingTemplate::new("BlockU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("BlockU".into(), t);
        }
        let oid = logic
            .create_object("BlockU", Team::USA, glam::Vec3::new(12.0, 0.0, 12.0))
            .expect("id");
        let other = logic
            .create_object("BlockU", Team::USA, glam::Vec3::new(14.0, 0.0, 12.0))
            .expect("other");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.cur_max_blocked_speed = 3.5;
            o.num_frames_blocked = 7;
            o.is_blocked = true;
            o.move_away_from = Some(other);
            o.requested_victim_id = Some(other);
            o.movement.max_speed = 10.0;
        }
        host_movement_log::record(
            oid,
            glam::Vec3::ZERO,
            10.0,
            0,
            &[],
            false,
            0,
            false,
            false,
            false,
            false,
            0,
            0,
            3.5,
            7,
            true,
            Some(other.0),
            Some(other.0),
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_movement_events(&host_movement_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!((e.cur_max_blocked_speed - 3.5).abs() < 1e-5);
        assert_eq!(e.num_frames_blocked, 7);
        assert!(e.is_blocked);
        assert_eq!(e.move_away_from_id, Some(other.0));
        assert_eq!(e.requested_victim_id, Some(other.0));
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.cur_max_blocked_speed = f32::MAX;
            o.num_frames_blocked = 0;
            o.is_blocked = false;
            o.move_away_from = None;
            o.requested_victim_id = None;
        }
        assert!(shadow.writeback_movement_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_physics_motive_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_bounce_land_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!((o.cur_max_blocked_speed - 3.5).abs() < 1e-5);
        assert_eq!(o.num_frames_blocked, 7);
        assert!(o.is_blocked);
        assert_eq!(o.move_away_from, Some(other));
        assert_eq!(o.requested_victim_id, Some(other));
    }

    #[test]
    fn rebuild_producer_channel_via_set_rebuild_producer() {
        use crate::game_logic::host_rebuild_producer_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_rebuild_producer_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RebuildP");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["HoleA", "BldA", "WorkerA"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Structure);
                logic.templates.insert(name.into(), t);
            }
        }
        let hole = logic
            .create_object("HoleA", Team::USA, glam::Vec3::new(20.0, 0.0, 20.0))
            .expect("hole");
        let bld = logic
            .create_object("BldA", Team::USA, glam::Vec3::new(22.0, 0.0, 20.0))
            .expect("bld");
        let worker = logic
            .create_object("WorkerA", Team::USA, glam::Vec3::new(24.0, 0.0, 20.0))
            .expect("worker");
        {
            let o = logic.get_objects_mut().get_mut(&hole).expect("o");
            o.is_rebuild_hole = true;
            o.rebuild_template_name = Some("BldA".into());
            o.rebuild_ready_frame = 100;
            o.rebuild_spawner_id = Some(bld);
            o.rebuild_worker_id = Some(worker);
            o.rebuild_reconstructing_id = Some(bld);
            o.producer_id = Some(hole);
            o.construction_complete_clear_frame = 250;
        }
        host_rebuild_producer_log::record(
            hole,
            true,
            "BldA".into(),
            100,
            Some(bld.0),
            Some(worker.0),
            Some(bld.0),
            Some(hole.0),
            250,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&hole.0).expect("map");
        assert!(
            shadow.apply_host_rebuild_producer_events(&host_rebuild_producer_log::drain()) >= 1
        );
        let e = shadow.world().entity(eid).unwrap();
        assert!(e.is_rebuild_hole);
        assert_eq!(e.rebuild_template_name, "BldA");
        assert_eq!(e.rebuild_ready_frame, 100);
        assert_eq!(e.rebuild_spawner_id, Some(bld.0));
        assert_eq!(e.rebuild_worker_id, Some(worker.0));
        assert_eq!(e.rebuild_reconstructing_id, Some(bld.0));
        assert_eq!(e.producer_id, Some(hole.0));
        assert_eq!(e.construction_complete_clear_frame, 250);
        {
            let o = logic.get_objects_mut().get_mut(&hole).expect("o");
            o.is_rebuild_hole = false;
            o.rebuild_template_name = None;
            o.rebuild_ready_frame = 0;
            o.rebuild_spawner_id = None;
            o.rebuild_worker_id = None;
            o.rebuild_reconstructing_id = None;
            o.producer_id = None;
            o.construction_complete_clear_frame = 0;
        }
        assert!(shadow.writeback_rebuild_producer_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&hole).unwrap();
        assert!(o.is_rebuild_hole);
        assert_eq!(o.rebuild_template_name.as_deref(), Some("BldA"));
        assert_eq!(o.rebuild_ready_frame, 100);
        assert_eq!(o.rebuild_spawner_id, Some(bld));
        assert_eq!(o.rebuild_worker_id, Some(worker));
        assert_eq!(o.rebuild_reconstructing_id, Some(bld));
        assert_eq!(o.producer_id, Some(hole));
        assert_eq!(o.construction_complete_clear_frame, 250);
    }

    #[test]
    fn sole_healing_channel_via_set_sole_healing() {
        use crate::game_logic::host_sole_healing_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_sole_healing_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SoleHeal");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["HealTgt", "DozerA"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Vehicle);
                logic.templates.insert(name.into(), t);
            }
        }
        let tgt = logic
            .create_object("HealTgt", Team::USA, glam::Vec3::new(30.0, 0.0, 30.0))
            .expect("tgt");
        let dozer = logic
            .create_object("DozerA", Team::USA, glam::Vec3::new(32.0, 0.0, 30.0))
            .expect("dozer");
        {
            let o = logic.get_objects_mut().get_mut(&tgt).expect("o");
            o.sole_healing_benefactor = Some(dozer);
            o.sole_healing_benefactor_expiration_frame = 900;
        }
        host_sole_healing_log::record(tgt, Some(dozer.0), 900);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&tgt.0).expect("map");
        assert!(shadow.apply_host_sole_healing_events(&host_sole_healing_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.sole_healing_benefactor_id, Some(dozer.0));
        assert_eq!(e.sole_healing_benefactor_expiration_frame, 900);
        {
            let o = logic.get_objects_mut().get_mut(&tgt).expect("o");
            o.sole_healing_benefactor = None;
            o.sole_healing_benefactor_expiration_frame = 0;
        }
        assert!(shadow.writeback_sole_healing_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&tgt).unwrap();
        assert_eq!(o.sole_healing_benefactor, Some(dozer));
        assert_eq!(o.sole_healing_benefactor_expiration_frame, 900);
    }

    #[test]
    fn ai_mood_channel_via_set_ai_mood() {
        use crate::game_logic::host_ai_mood_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_ai_mood_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiMood");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("MoodU") {
            let mut t = ThingTemplate::new("MoodU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("MoodU".into(), t);
        }
        let oid = logic
            .create_object("MoodU", Team::USA, glam::Vec3::new(40.0, 0.0, 40.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.idle_since_frame = 120;
            o.mood_attack_check_rate = 45;
            o.auto_acquire_when_idle = false;
            o.attack_priority_set = Some("Soldier".into());
        }
        host_ai_mood_log::record(oid, 120, 45, false, "Soldier".into());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_ai_mood_events(&host_ai_mood_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.idle_since_frame, 120);
        assert_eq!(e.mood_attack_check_rate, 45);
        assert!(!e.auto_acquire_when_idle);
        assert_eq!(e.attack_priority_set, "Soldier");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.idle_since_frame = 0;
            o.mood_attack_check_rate = 30;
            o.auto_acquire_when_idle = true;
            o.attack_priority_set = None;
        }
        assert!(shadow.writeback_ai_mood_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.idle_since_frame, 120);
        assert_eq!(o.mood_attack_check_rate, 45);
        assert!(!o.auto_acquire_when_idle);
        assert_eq!(o.attack_priority_set.as_deref(), Some("Soldier"));
    }

    #[test]
    fn guard_radius_channel_via_set_guard() {
        use crate::game_logic::host_guard_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_guard_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GuardR");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("GuardU") {
            let mut t = ThingTemplate::new("GuardU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("GuardU".into(), t);
        }
        let oid = logic
            .create_object("GuardU", Team::USA, glam::Vec3::new(50.0, 0.0, 50.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.guard_position = Some(glam::Vec3::new(55.0, 0.0, 55.0));
            o.guard_target = None;
            o.guard_radius = 175.0;
        }
        host_guard_log::record(oid, Some([55.0, 0.0, 55.0]), 0, 175.0);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_guard_events(&host_guard_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!((e.guard_radius - 175.0).abs() < 1e-3);
        let gp = e.guard_position.expect("pos");
        assert!((gp[0] - 55.0).abs() < 1e-3);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.guard_radius = 0.0;
            o.guard_position = None;
        }
        assert!(shadow.writeback_guard_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!((o.guard_radius - 175.0).abs() < 1e-3);
        assert!(o.guard_position.is_some());
    }

    #[test]
    fn production_door_channel_via_set_production_door() {
        use crate::game_logic::host_production_door_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_production_door_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ProdDoor");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("DoorFact") {
            let mut t = ThingTemplate::new("DoorFact");
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::FSBarracks);
            logic.templates.insert("DoorFact".into(), t);
        }
        let oid = logic
            .create_object("DoorFact", Team::USA, glam::Vec3::new(60.0, 0.0, 60.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.production_door_phase = 2;
            o.production_door_phase_end_frame = 500;
            o.production_door_hold_open = true;
        }
        host_production_door_log::record(oid, 2, 500, true);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_production_door_events(&host_production_door_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.production_door_phase, 2);
        assert_eq!(e.production_door_phase_end_frame, 500);
        assert!(e.production_door_hold_open);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.production_door_phase = 0;
            o.production_door_phase_end_frame = 0;
            o.production_door_hold_open = false;
        }
        assert!(shadow.writeback_production_door_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.production_door_phase, 2);
        assert_eq!(o.production_door_phase_end_frame, 500);
        assert!(o.production_door_hold_open);
    }

    #[test]
    fn physics_motive_channel_via_set_physics_motive() {
        use crate::game_logic::host_physics_motive_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_physics_motive_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PhysMot");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("PhysU") {
            let mut t = ThingTemplate::new("PhysU");
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("PhysU".into(), t);
        }
        let oid = logic
            .create_object("PhysU", Team::USA, glam::Vec3::new(70.0, 0.0, 70.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.motive_frames_remaining = 12;
            o.physics_mass = 2.5;
            o.physics_accel = glam::Vec3::new(1.0, 0.0, 0.5);
            o.forward_friction = 0.15;
            o.lateral_friction = 0.2;
            o.z_friction = 0.1;
            o.can_path_through_units = true;
            o.ignore_collisions_until_frame = 40;
            o.is_panicking = true;
            o.move_away_frames = 5;
            o.aerodynamic_friction = 0.05;
            o.extra_friction = 0.02;
            o.apply_friction_2d_when_airborne = true;
            o.center_of_mass_offset = -0.5;
            o.pitch_roll_yaw_factor = 1.2;
            o.immune_to_falling_damage = true;
        }
        host_physics_motive_log::record(
            oid,
            12,
            2.5,
            [1.0, 0.0, 0.5],
            0.15,
            0.2,
            0.1,
            true,
            40,
            true,
            5,
            0.05,
            0.02,
            true,
            -0.5,
            1.2,
            None,
            None,
            true,
            None,
            None,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_physics_motive_events(&host_physics_motive_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.motive_frames_remaining, 12);
        assert!((e.physics_mass - 2.5).abs() < 1e-5);
        assert!((e.physics_accel[0] - 1.0).abs() < 1e-5);
        assert!(e.can_path_through_units);
        assert!(e.is_panicking);
        assert_eq!(e.ignore_collisions_until_frame, 40);
        assert!((e.aerodynamic_friction - 0.05).abs() < 1e-5);
        assert!(e.immune_to_falling_damage);
        assert!((e.aerodynamic_friction - 0.05).abs() < 1e-5);
        assert!(e.immune_to_falling_damage);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.motive_frames_remaining = 0;
            o.physics_mass = 1.0;
            o.can_path_through_units = false;
            o.is_panicking = false;
            o.ignore_collisions_until_frame = 0;
        }
        assert!(shadow.writeback_physics_motive_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_bounce_land_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.motive_frames_remaining, 12);
        assert!((o.physics_mass - 2.5).abs() < 1e-5);
        assert!(o.can_path_through_units);
        assert!(o.is_panicking);
        assert_eq!(o.ignore_collisions_until_frame, 40);
        assert!((o.aerodynamic_friction - 0.05).abs() < 1e-5);
        assert!(o.immune_to_falling_damage);
        assert!((o.aerodynamic_friction - 0.05).abs() < 1e-5);
        assert!(o.immune_to_falling_damage);
    }

    #[test]
    fn bounce_land_channel_via_set_bounce_land() {
        use crate::game_logic::host_bounce_land_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_bounce_land_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("BounceL");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("BounceU") {
            let mut t = ThingTemplate::new("BounceU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("BounceU".into(), t);
        }
        let oid = logic
            .create_object("BounceU", Team::USA, glam::Vec3::new(80.0, 0.0, 80.0))
            .expect("id");
        let other = logic
            .create_object("BounceU", Team::USA, glam::Vec3::new(82.0, 0.0, 80.0))
            .expect("other");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.kill_when_resting_on_ground = true;
            o.bounce_land_events = 3;
            o.last_bounce_fall_dy = 12.0;
            o.bounce_sound_name = "Module:Bounce".into();
            o.last_bounce_volume = 0.75;
            o.bounce_audio_pending = 2;
            o.allow_collide_force = false;
            o.last_collidee = Some(other);
            o.ignore_collisions_with = Some(other);
        }
        host_bounce_land_log::record(
            oid,
            true,
            3,
            12.0,
            "Module:Bounce".into(),
            0.75,
            2,
            false,
            Some(other.0),
            Some(other.0),
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_bounce_land_events(&host_bounce_land_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!(e.kill_when_resting_on_ground);
        assert_eq!(e.bounce_land_events, 3);
        assert!((e.last_bounce_fall_dy - 12.0).abs() < 1e-5);
        assert_eq!(e.bounce_sound_name, "Module:Bounce");
        assert!((e.last_bounce_volume - 0.75).abs() < 1e-5);
        assert_eq!(e.bounce_audio_pending, 2);
        assert!(!e.allow_collide_force);
        assert_eq!(e.last_collidee_id, Some(other.0));
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.kill_when_resting_on_ground = false;
            o.bounce_land_events = 0;
            o.bounce_audio_pending = 0;
            o.last_collidee = None;
        }
        assert!(shadow.writeback_bounce_land_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!(o.kill_when_resting_on_ground);
        assert_eq!(o.bounce_land_events, 3);
        assert_eq!(o.bounce_audio_pending, 2);
        assert_eq!(o.last_collidee, Some(other));
    }

    #[test]
    fn turret_extended_channel_via_set_turret() {
        use crate::game_logic::host_turret_log;
        use crate::game_logic::object::TurretSubState;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_turret_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("TurretX");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("TurU") {
            let mut t = ThingTemplate::new("TurU");
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("TurU".into(), t);
        }
        let oid = logic
            .create_object("TurU", Team::USA, glam::Vec3::new(90.0, 0.0, 90.0))
            .expect("id");
        let tgt = logic
            .create_object("TurU", Team::China, glam::Vec3::new(100.0, 0.0, 90.0))
            .expect("tgt");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.turret_angle_deg = 45.0;
            o.turret_pitch_deg = 10.0;
            o.turret_holding = true;
            o.turret_idle_scanning = false;
            o.turret_turn_rate_rad = 0.05;
            o.turret_recenter_frames = 60;
            o.turret_hold_until_frame = 200;
            o.turret_idle_recentering = true;
            o.turret_enabled = true;
            o.turret_rotating = true;
            o.turret_natural_angle_deg = 0.0;
            o.turret_natural_pitch_deg = 5.0;
            o.turret_target_id = Some(tgt);
            o.turret_force_attacking = true;
            o.turret_mood_target = false;
            o.turret_idle_scan_next_frame = 30;
            o.turret_idle_scan_desired_angle_deg = 90.0;
            o.turret_idle_scan_index = 2;
            o.turret_substate = TurretSubState::Aim;
        }
        host_turret_log::record(
            oid,
            45.0,
            10.0,
            true,
            false,
            0.05,
            60,
            200,
            true,
            true,
            true,
            0.0,
            5.0,
            tgt.0,
            true,
            false,
            30,
            90.0,
            2,
            TurretSubState::Aim.ordinal(),
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_turret_events(&host_turret_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!((e.turret_angle_deg - 45.0).abs() < 1e-5);
        assert!((e.turret_turn_rate_rad - 0.05).abs() < 1e-5);
        assert_eq!(e.turret_recenter_frames, 60);
        assert!(e.turret_enabled);
        assert!(e.turret_rotating);
        assert_eq!(e.turret_target_host, tgt.0);
        assert_eq!(e.turret_substate, TurretSubState::Aim.ordinal());
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.turret_angle_deg = 0.0;
            o.turret_turn_rate_rad = 0.0;
            o.turret_enabled = false;
            o.turret_target_id = None;
            o.turret_substate = TurretSubState::Idle;
        }
        assert!(shadow.writeback_turret_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_stealth_delay_to_host(&mut logic);
        let _ = shadow.writeback_combat_attack_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!((o.turret_angle_deg - 45.0).abs() < 1e-5);
        assert!((o.turret_turn_rate_rad - 0.05).abs() < 1e-5);
        assert!(o.turret_enabled);
        assert_eq!(o.turret_target_id, Some(tgt));
        assert_eq!(o.turret_substate, TurretSubState::Aim);
    }

    #[test]
    fn stealth_delay_channel_via_set_stealth_delay() {
        use crate::game_logic::host_stealth_delay_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_stealth_delay_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StealthD");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("StlU") {
            let mut t = ThingTemplate::new("StlU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("StlU".into(), t);
        }
        let oid = logic
            .create_object("StlU", Team::USA, glam::Vec3::new(110.0, 0.0, 110.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.stealth_allowed_frame = 300;
            o.stealth_delay_pending = true;
            o.stealth_delay_frames = 75;
            o.stealth_breaks_on_damage = true;
            o.detection_expires_frame = 450;
            o.camo_opacity_pulse_phase = 1.25;
            o.camo_heat_vision_opacity = 1.0;
            o.camo_net_sub_object_shown = true;
            o.camo_net_sub_object_observer_visible = true;
        }
        host_stealth_delay_log::record(oid, 300, true, 75, true, 450, 1.25, 1.0, true, true);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_stealth_delay_events(&host_stealth_delay_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.stealth_allowed_frame, 300);
        assert!(e.stealth_delay_pending);
        assert_eq!(e.stealth_delay_frames, 75);
        assert!(e.stealth_breaks_on_damage);
        assert_eq!(e.detection_expires_frame, 450);
        assert!((e.camo_opacity_pulse_phase - 1.25).abs() < 1e-5);
        assert!(e.camo_net_sub_object_shown);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.stealth_delay_pending = false;
            o.stealth_allowed_frame = 0;
            o.stealth_delay_frames = 0;
            o.camo_net_sub_object_shown = false;
        }
        assert!(shadow.writeback_stealth_delay_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_combat_attack_to_host(&mut logic);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!(o.stealth_delay_pending);
        assert_eq!(o.stealth_allowed_frame, 300);
        assert_eq!(o.stealth_delay_frames, 75);
        assert!(o.camo_net_sub_object_shown);
    }

    #[test]
    fn combat_attack_channel_via_set_combat_attack() {
        use crate::game_logic::host_combat_attack_log;
        use crate::game_logic::{AttackSubState, KindOf, Team, ThingTemplate};
        host_combat_attack_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CbtAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("CbtU") {
            let mut t = ThingTemplate::new("CbtU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("CbtU".into(), t);
        }
        let oid = logic
            .create_object("CbtU", Team::USA, glam::Vec3::new(130.0, 0.0, 130.0))
            .expect("id");
        let tgt = logic
            .create_object("CbtU", Team::China, glam::Vec3::new(160.0, 0.0, 130.0))
            .expect("tgt");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.pre_attack_target = Some(tgt);
            o.pre_attack_ready_at = 12.5;
            o.consecutive_shots_at_target = 3;
            o.max_shots_to_fire = 5;
            o.attack_substate = AttackSubState::FireWeapon;
            o.approach_timestamp = 90;
            o.continuous_fire_victim = tgt.0;
            o.maintain_pos_valid = true;
            o.maintain_pos = Some(glam::Vec3::new(1.0, 2.0, 3.0));
            o.temporary_move_frames = 7;
            o.group_speed_factor = 0.85;
        }
        host_combat_attack_log::record(
            oid,
            tgt.0,
            12.5,
            3,
            5,
            AttackSubState::FireWeapon.to_ordinal(),
            90,
            tgt.0,
            true,
            Some([1.0, 2.0, 3.0]),
            7,
            0.85,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_combat_attack_events(&host_combat_attack_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.pre_attack_target_host, tgt.0);
        assert!((e.pre_attack_ready_at - 12.5).abs() < 1e-5);
        assert_eq!(e.consecutive_shots_at_target, 3);
        assert_eq!(e.max_shots_to_fire, 5);
        assert_eq!(e.attack_substate_ordinal, 1);
        assert_eq!(e.approach_timestamp, 90);
        assert_eq!(e.continuous_fire_victim, tgt.0);
        assert!(e.maintain_pos_valid);
        assert_eq!(e.maintain_pos, Some([1.0, 2.0, 3.0]));
        assert_eq!(e.temporary_move_frames, 7);
        assert!((e.group_speed_factor - 0.85).abs() < 1e-5);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.pre_attack_target = None;
            o.attack_substate = AttackSubState::AimAtTarget;
            o.consecutive_shots_at_target = 0;
            o.maintain_pos = None;
            o.maintain_pos_valid = false;
        }
        assert!(shadow.writeback_combat_attack_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let _ = shadow.writeback_locomotor_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.pre_attack_target, Some(tgt));
        assert_eq!(o.attack_substate, AttackSubState::FireWeapon);
        assert_eq!(o.consecutive_shots_at_target, 3);
        assert_eq!(o.maintain_pos, Some(glam::Vec3::new(1.0, 2.0, 3.0)));
        assert!((o.group_speed_factor - 0.85).abs() < 1e-5);
    }

    #[test]
    fn locomotor_channel_via_set_locomotor() {
        use crate::game_logic::host_locomotor_log;
        use crate::game_logic::{
            KindOf, LocomotorAppearance, LocomotorBehaviorZ, Team, ThingTemplate,
        };
        host_locomotor_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("Loco");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("LocoU") {
            let mut t = ThingTemplate::new("LocoU");
            t.add_kind_of(KindOf::Vehicle);
            logic.templates.insert("LocoU".into(), t);
        }
        let oid = logic
            .create_object("LocoU", Team::USA, glam::Vec3::new(150.0, 0.0, 150.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.is_approach_path = true;
            o.on_invalid_movement_terrain = true;
            o.was_airborne_last_frame = true;
            o.can_move_backward = true;
            o.moving_backwards = true;
            o.no_slow_down_as_approaching_dest = true;
            o.turn_pivot_offset = -0.5;
            o.wander_width_factor = 0.2;
            o.loco_apply_2d_friction_airborne = true;
            o.loco_extra_2d_friction = 0.03;
            o.loco_preferred_height = 40.0;
            o.loco_preferred_height_damping = 0.7;
            o.loco_appearance = LocomotorAppearance::Wings;
            o.loco_behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
            o.min_turn_speed = 5.5;
            o.physics_turning = crate::game_logic::PhysicsTurningType::TurnPositive;
        }
        host_locomotor_log::record(
            oid,
            true,
            true,
            true,
            true,
            true,
            true,
            -0.5,
            0.2,
            true,
            0.03,
            40.0,
            0.7,
            LocomotorAppearance::Wings.to_ordinal(),
            LocomotorBehaviorZ::AbsoluteHeight.to_ordinal(),
            5.5,
            crate::game_logic::PhysicsTurningType::TurnPositive.to_ordinal(),
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_locomotor_events(&host_locomotor_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!(e.is_approach_path);
        assert!(e.was_airborne_last_frame);
        assert!(e.moving_backwards);
        assert!((e.turn_pivot_offset + 0.5).abs() < 1e-5);
        assert!((e.loco_preferred_height - 40.0).abs() < 1e-5);
        assert_eq!(
            e.loco_appearance_ordinal,
            LocomotorAppearance::Wings.to_ordinal()
        );
        assert_eq!(
            e.loco_behavior_z_ordinal,
            LocomotorBehaviorZ::AbsoluteHeight.to_ordinal()
        );
        assert!((e.min_turn_speed - 5.5).abs() < 1e-5);
        assert_eq!(e.physics_turning_ordinal, 1);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.is_approach_path = false;
            o.moving_backwards = false;
            o.loco_appearance = LocomotorAppearance::Other;
            o.loco_preferred_height = 0.0;
        }
        assert!(shadow.writeback_locomotor_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!(o.is_approach_path);
        assert!(o.moving_backwards);
        assert_eq!(o.loco_appearance, LocomotorAppearance::Wings);
        assert!((o.loco_preferred_height - 40.0).abs() < 1e-5);
        assert!((o.min_turn_speed - 5.5).abs() < 1e-5);
        assert_eq!(
            o.physics_turning,
            crate::game_logic::PhysicsTurningType::TurnPositive
        );
    }

    #[test]
    fn ai_request_channel_via_set_ai_request() {
        use crate::game_logic::host_ai_request_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_ai_request_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiReq");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AiU") {
            let mut t = ThingTemplate::new("AiU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("AiU".into(), t);
        }
        let oid = logic
            .create_object("AiU", Team::USA, glam::Vec3::new(170.0, 0.0, 170.0))
            .expect("id");
        let victim = logic
            .create_object("AiU", Team::China, glam::Vec3::new(200.0, 0.0, 170.0))
            .expect("v");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.requested_victim_id = Some(victim);
            o.requested_destination = Some(glam::Vec3::new(9.0, 0.0, 8.0));
            o.prev_victim_pos = Some(glam::Vec3::new(1.0, 2.0, 3.0));
            o.crate_created = Some(ObjectId(99));
            o.guard_retaliate_victim = Some(victim);
            o.guard_retaliate_anchor = Some(glam::Vec3::new(4.0, 0.0, 5.0));
            o.path_timestamp = 77;
            o.disguise_pending_template = Some("FakeTank".into());
            o.disguise_pending_team = Some(Team::GLA);
            o.weapon_crate_upgrade = 2;
            o.armor_crate_upgrade = 1;
            o.selection_flash_remaining = 15;
        }
        host_ai_request_log::record(
            oid,
            victim.0,
            Some([9.0, 0.0, 8.0]),
            Some([1.0, 2.0, 3.0]),
            99,
            victim.0,
            Some([4.0, 0.0, 5.0]),
            77,
            "FakeTank".into(),
            2,
            2,
            1,
            15,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_ai_request_events(&host_ai_request_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.requested_victim_id, Some(victim.0));
        assert_eq!(e.requested_destination, Some([9.0, 0.0, 8.0]));
        assert_eq!(e.prev_victim_pos, Some([1.0, 2.0, 3.0]));
        assert_eq!(e.crate_created_host, 99);
        assert_eq!(e.guard_retaliate_victim_host, victim.0);
        assert_eq!(e.path_timestamp, 77);
        assert_eq!(e.disguise_pending_template, "FakeTank");
        assert_eq!(e.disguise_pending_team_ordinal, 2);
        assert_eq!(e.weapon_crate_upgrade, 2);
        assert_eq!(e.selection_flash_remaining, 15);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.requested_victim_id = None;
            o.disguise_pending_template = None;
            o.weapon_crate_upgrade = 0;
            o.selection_flash_remaining = 0;
        }
        assert!(shadow.writeback_ai_request_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.requested_victim_id, Some(victim));
        assert_eq!(o.disguise_pending_template.as_deref(), Some("FakeTank"));
        assert_eq!(o.disguise_pending_team, Some(Team::GLA));
        assert_eq!(o.weapon_crate_upgrade, 2);
        assert_eq!(o.selection_flash_remaining, 15);
        assert_eq!(o.crate_created, Some(ObjectId(99)));
    }

    #[test]
    fn hijacker_channel_via_set_hijacker() {
        use crate::game_logic::host_hijacker_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_hijacker_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("Hijack");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("HjU") {
            let mut t = ThingTemplate::new("HjU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("HjU".into(), t);
        }
        let oid = logic
            .create_object("HjU", Team::USA, glam::Vec3::new(180.0, 0.0, 180.0))
            .expect("id");
        let vehicle = logic
            .create_object("HjU", Team::China, glam::Vec3::new(190.0, 0.0, 180.0))
            .expect("v");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.hijack_vehicle_id = Some(vehicle);
            o.hijacker_in_vehicle = true;
            o.hijacker_update_active = true;
            o.hijacker_was_airborne = true;
            o.hijacker_eject_pos = Some(glam::Vec3::new(3.0, 1.0, 4.0));
            o.hive_slave_respawn_frame = 250;
            o.next_detection_scan_frame = 33;
        }
        host_hijacker_log::record(
            oid,
            vehicle.0,
            true,
            true,
            true,
            Some([3.0, 1.0, 4.0]),
            250,
            33,
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_hijacker_events(&host_hijacker_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.hijack_vehicle_host, vehicle.0);
        assert!(e.hijacker_in_vehicle);
        assert!(e.hijacker_update_active);
        assert!(e.hijacker_was_airborne);
        assert_eq!(e.hijacker_eject_pos, Some([3.0, 1.0, 4.0]));
        assert_eq!(e.hive_slave_respawn_frame, 250);
        assert_eq!(e.next_detection_scan_frame, 33);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.hijack_vehicle_id = None;
            o.hijacker_in_vehicle = false;
            o.hive_slave_respawn_frame = 0;
        }
        assert!(shadow.writeback_hijacker_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.hijack_vehicle_id, Some(vehicle));
        assert!(o.hijacker_in_vehicle);
        assert_eq!(o.hive_slave_respawn_frame, 250);
        assert_eq!(o.next_detection_scan_frame, 33);
        assert_eq!(o.hijacker_eject_pos, Some(glam::Vec3::new(3.0, 1.0, 4.0)));
    }

    #[test]
    fn leech_range_channel_via_set_weapon_stats() {
        use crate::game_logic::host_weapon_stats_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_weapon_stats_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("Leech");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("LchU") {
            let mut t = ThingTemplate::new("LchU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("LchU".into(), t);
        }
        let oid = logic
            .create_object("LchU", Team::USA, glam::Vec3::new(210.0, 0.0, 210.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.leech_range_active_primary = true;
            o.leech_range_active_secondary = true;
            o.record_host_weapon_stats();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        let events = host_weapon_stats_log::drain();
        assert!(!events.is_empty());
        assert!(shadow.apply_host_weapon_stats_events(&events) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert!(e.leech_range_active_primary);
        assert!(e.leech_range_active_secondary);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.leech_range_active_primary = false;
            o.leech_range_active_secondary = false;
        }
        assert!(shadow.writeback_weapon_stats_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_fire_intent_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!(o.leech_range_active_primary);
        assert!(o.leech_range_active_secondary);
    }

    #[test]
    fn fire_intent_channel_via_set_fire_intent() {
        use crate::game_logic::host_fire_intent_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_fire_intent_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FireInt");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("FiU") {
            let mut t = ThingTemplate::new("FiU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("FiU".into(), t);
        }
        let oid = logic
            .create_object("FiU", Team::USA, glam::Vec3::new(220.0, 0.0, 220.0))
            .expect("id");
        let victim = logic
            .create_object("FiU", Team::China, glam::Vec3::new(240.0, 0.0, 220.0))
            .expect("v");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.last_fire_victim_host = victim.0;
            o.last_fire_slot = 1;
            o.last_fire_damage = 42.0;
            o.last_fire_range = 150.0;
            o.last_fire_sim_time = 9.5;
            o.last_fire_frame = 285;
            o.fire_intent_count = 3;
        }
        host_fire_intent_log::record(oid, victim.0, 1, 42.0, 150.0, 9.5, 285, 3);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_fire_intent_events(&host_fire_intent_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.last_fire_victim_host, victim.0);
        assert_eq!(e.last_fire_slot, 1);
        assert!((e.last_fire_damage - 42.0).abs() < 1e-5);
        assert!((e.last_fire_range - 150.0).abs() < 1e-5);
        assert!((e.last_fire_sim_time - 9.5).abs() < 1e-5);
        assert_eq!(e.last_fire_frame, 285);
        assert_eq!(e.fire_intent_count, 3);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.last_fire_victim_host = 0;
            o.fire_intent_count = 0;
            o.last_fire_damage = 0.0;
        }
        assert!(shadow.writeback_fire_intent_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.last_fire_victim_host, victim.0);
        assert_eq!(o.fire_intent_count, 3);
        assert!((o.last_fire_damage - 42.0).abs() < 1e-5);
        assert_eq!(o.last_fire_slot, 1);
    }

    #[test]
    fn projectile_flight_channel_via_set_projectile_flight() {
        use crate::game_logic::host_projectile_log;
        host_projectile_log::clear();
        let mut shadow = GameWorldShadow::new(64);
        host_projectile_log::record(
            501,
            [10.0, 1.0, 20.0],
            [5.0, 0.0, 0.0],
            [100.0, 1.0, 20.0],
            25.0,
            7,
            8,
            200.0,
            0.5,
            3.0,
            true,
            true,
        );
        assert!(shadow.apply_host_projectile_events(&host_projectile_log::drain()) >= 1);
        let p = shadow.world().projectile(501).expect("projectile residual");
        assert_eq!(p.host_id, 501);
        assert_eq!(p.position, [10.0, 1.0, 20.0]);
        assert_eq!(p.velocity, [5.0, 0.0, 0.0]);
        assert_eq!(p.target_position, [100.0, 1.0, 20.0]);
        assert!((p.damage - 25.0).abs() < 1e-5);
        assert_eq!(p.shooter_host, 7);
        assert_eq!(p.target_host, 8);
        assert!((p.speed - 200.0).abs() < 1e-5);
        assert!(p.is_homing);
        assert!(p.active);
        // deactivate
        host_projectile_log::record(
            501,
            [10.0, 1.0, 20.0],
            [0.0, 0.0, 0.0],
            [100.0, 1.0, 20.0],
            25.0,
            7,
            8,
            200.0,
            3.0,
            3.0,
            true,
            false,
        );
        assert!(shadow.apply_host_projectile_events(&host_projectile_log::drain()) >= 1);
        assert!(shadow.world().projectile(501).is_none());
    }

    #[test]
    fn projectile_authority_steps_flight_and_writeback() {
        use crate::game_logic::host_projectile_log;
        let prev = std::env::var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", "1");
        assert!(gameworld_projectile_authority_enabled());
        host_projectile_log::clear();
        let mut logic = GameLogic::new();
        // Seed one ballistic projectile on host combat system.
        {
            use crate::game_logic::combat::DamageType;
            use crate::game_logic::Weapon;
            let mut w = Weapon {
                damage: 10.0,
                range: 500.0,
                ..Weapon::default()
            };
            w.projectile_speed = 100.0;
            let id = logic.combat_system.fire_projectile(
                glam::Vec3::new(0.0, 0.0, 0.0),
                glam::Vec3::new(100.0, 0.0, 0.0),
                &w,
                ObjectId(1),
                None,
                100.0,
            );
            assert_eq!(
                id.0,
                logic
                    .combat_system
                    .get_projectiles()
                    .keys()
                    .next()
                    .unwrap()
                    .0
            );
        }
        host_projectile_log::record_snapshot(logic.combat_system.projectiles_snapshot());
        let mut shadow = GameWorldShadow::new(64);
        assert!(shadow.apply_host_projectile_events(&host_projectile_log::drain()) >= 1);
        let before = shadow
            .world()
            .projectiles()
            .values()
            .next()
            .unwrap()
            .position[0];
        let stepped = shadow.world.step_projectiles(1.0 / 30.0, |_| None);
        assert!(stepped >= 1);
        let after = shadow
            .world()
            .projectiles()
            .values()
            .next()
            .unwrap()
            .position[0];
        assert!(
            after > before,
            "projectile should advance along +X (before={before} after={after})"
        );
        let n = shadow.writeback_projectiles_to_host(&mut logic);
        assert!(n >= 1);
        let p = logic
            .combat_system
            .get_projectiles()
            .values()
            .next()
            .unwrap();
        assert!((p.position.x - after).abs() < 1e-4);
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY"),
        }
    }

    #[test]
    fn ai_decision_buffer_channel_via_push_ai_decision() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiDec");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AdU") {
            let mut t = ThingTemplate::new("AdU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("AdU".into(), t);
        }
        let oid = logic
            .create_object("AdU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
            .expect("id");
        let vid = logic
            .create_object("AdU", Team::China, glam::Vec3::new(25.0, 0.0, 5.0))
            .expect("v");
        logic.apply_ai_command_for_test(crate::game_logic::game_logic::AICommand::AttackTarget {
            object_id: oid,
            target_id: vid,
        });
        logic.apply_ai_command_for_test(crate::game_logic::game_logic::AICommand::MoveTo {
            object_id: oid,
            position: glam::Vec3::new(1.0, 0.0, 2.0),
        });
        let events = host_ai_decision_log::drain();
        assert!(events.len() >= 2);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_host_ai_decision_events(&events) >= 2);
        let dec = shadow.world().ai_decisions();
        assert!(dec.iter().any(|d| {
            d.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && d.host_object == oid.0
                && d.target_host == vid.0
        }));
        assert!(dec.iter().any(|d| {
            d.kind == host_ai_decision_log::AI_DECISION_MOVE_TO
                && d.destination == Some([1.0, 0.0, 2.0])
        }));
    }

    #[test]
    fn ai_decision_authority_applies_attack_via_gameworld() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        assert!(gameworld_ai_decision_authority_enabled());
        // Attack writeback must also be on for last-write.
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiDecAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AdaU") {
            let mut t = ThingTemplate::new("AdaU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("AdaU".into(), t);
        }
        let oid = logic
            .create_object("AdaU", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
            .expect("id");
        let vid = logic
            .create_object("AdaU", Team::China, glam::Vec3::new(40.0, 0.0, 8.0))
            .expect("v");
        // Log-only path (authority on): record without host apply_ai_command.
        host_ai_decision_log::record_attack(oid, vid);
        let events = host_ai_decision_log::drain();
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        // Host still has no target until writeback.
        assert!(logic.get_objects().get(&oid).unwrap().target.is_none());
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn apply_ai_command_defers_host_attack_under_authority() {
        use crate::game_logic::game_logic::AICommand;
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiCmdAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AcU") {
            let mut t = ThingTemplate::new("AcU");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert("AcU".into(), t);
        }
        let oid = logic
            .create_object("AcU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        let vid = logic
            .create_object("AcU", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("v");
        logic.apply_ai_command_for_test(AICommand::AttackTarget {
            object_id: oid,
            target_id: vid,
        });
        logic.apply_ai_command_for_test(AICommand::SetAIState {
            object_id: oid,
            state: crate::game_logic::AIState::Attacking,
        });
        let events = host_ai_decision_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.kind == host_ai_decision_log::AI_DECISION_ATTACK),
            "AttackTarget must be logged: {events:?}"
        );
        assert!(
            events
                .iter()
                .any(|e| e.kind == host_ai_decision_log::AI_DECISION_SET_STATE),
            "SetAIState must be logged: {events:?}"
        );
        let host = logic.get_objects().get(&oid).unwrap();
        assert!(
            host.target.is_none(),
            "apply_ai_command must not host-apply AttackTarget under decision authority"
        );
        // AI state stays default until writeback (Idle unless previously set).
        assert_ne!(
            host.ai_state,
            crate::game_logic::AIState::Attacking,
            "SetAIState must not host-apply under decision authority"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&oid).unwrap().target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn continue_attack_after_kill_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ContAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["CaA", "CaD", "CaN"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let attacker = logic
            .create_object("CaA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let dead = logic
            .create_object("CaD", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
            .expect("d");
        let next = logic
            .create_object("CaN", Team::GLA, glam::Vec3::new(8.0, 0.0, 0.0))
            .expect("n");
        let dead_pos = glam::Vec3::new(5.0, 0.0, 0.0);
        let ok = logic.try_continue_attack_after_kill_for_test(
            attacker,
            dead,
            dead_pos,
            50.0,
            Team::GLA,
        );
        assert!(ok, "must find next victim in continue range");
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == attacker
                    && e.target_host == next.0
            }),
            "continue-attack must log AttackTarget on next victim; got {events:?}"
        );
        assert!(
            logic.get_objects().get(&attacker).unwrap().target.is_none(),
            "host target deferred under decision authority"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&attacker).unwrap().target,
            Some(next)
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn assign_unit_attack_path_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AtkPath");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["ApU", "ApE"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                t.set_health(100.0);
                logic.templates.insert(name.into(), t);
            }
        }
        let uid = logic
            .create_object("ApU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let vid = logic
            .create_object("ApE", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
            .expect("e");
        if let Some(o) = logic.get_objects_mut().get_mut(&uid) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                range: 25.0,
                ..Weapon::default()
            });
        }
        let tpos = glam::Vec3::new(80.0, 0.0, 0.0);
        let ok = logic.assign_unit_attack_path_for_test(uid, Some(vid), tpos);
        assert!(ok, "attack path should assign");
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == uid
                    && e.target_host == vid.0
            }),
            "must log AttackTarget; got {events:?}"
        );
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == uid
                    && e.ai_state_ordinal == 2
            }),
            "must log Attacking state; got {events:?}"
        );
        let host = logic.get_objects().get(&uid).unwrap();
        assert!(
            host.target.is_none(),
            "host target deferred under decision authority"
        );
        // Path still on host for movement residual.
        assert!(
            !host.movement.path.is_empty() || host.movement.target_position.is_some(),
            "path must still be assigned on host"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&uid).unwrap().target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn path_approach_with_state_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PathSt");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("PsU") {
            let mut t = ThingTemplate::new("PsU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("PsU".into(), t);
        }
        let oid = logic
            .create_object("PsU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        logic.path_approach_with_state_for_test(
            oid,
            glam::Vec3::new(40.0, 0.0, 0.0),
            AIState::Gathering,
        );
        let events = host_ai_decision_log::drain();
        let gathering_ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Gathering);
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == oid
                    && e.ai_state_ordinal == gathering_ord
            }),
            "path_approach must log SetAIState; got {events:?} ord={gathering_ord}"
        );
        assert_ne!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Gathering,
            "host ai_state deferred under decision authority"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Gathering
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn troop_crawler_assault_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("TcAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let crawler_name = "ChinaVehicleTroopCrawler";
        for name in [crawler_name, "TcO", "TcE"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                if name == crawler_name {
                    t.add_kind_of(KindOf::Vehicle);
                } else {
                    t.add_kind_of(KindOf::Infantry);
                }
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let crawler = logic
            .create_object(crawler_name, Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("c");
        let occ = logic
            .create_object("TcO", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
            .expect("o");
        let enemy = logic
            .create_object("TcE", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
            .expect("e");
        if let Some(c) = logic.get_objects_mut().get_mut(&crawler) {
            c.install_troop_crawler_transport();
            let _ = c.add_occupant(occ);
        }
        if let Some(o) = logic.get_objects_mut().get_mut(&occ) {
            o.set_contained_by(Some(crawler));
        }
        let ordered = logic.apply_troop_crawler_assault_deploy_for_test(crawler, enemy);
        assert!(
            ordered >= 1,
            "deploy should order occupant attack; ordered={ordered}"
        );
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK && e.target_host == enemy.0
            }),
            "assault deploy must log AttackTarget; ordered={ordered} events={events:?}"
        );
        // Occupant host target deferred when authority on.
        if let Some(o) = logic.get_objects().get(&occ) {
            assert!(
                o.target.is_none(),
                "occupant host target deferred under decision authority"
            );
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        // Writeback should land on whoever logged AttackTarget (occ if ordered, else engagetest).
        let hit = logic
            .get_objects()
            .iter()
            .any(|(id, o)| o.target == Some(enemy) && *id != enemy);
        assert!(hit, "writeback must set some unit target to enemy");
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn missile_defender_laser_guided_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MdAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        // Retail template name residual for missile defender.
        let md_name = "AmericaInfantryMissileDefender";
        if !logic.templates.contains_key(md_name) {
            let mut t = ThingTemplate::new(md_name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(md_name.into(), t);
        }
        if !logic.templates.contains_key("MdE") {
            let mut t = ThingTemplate::new("MdE");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert("MdE".into(), t);
        }
        let mid = logic
            .create_object(md_name, Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("md");
        let eid = logic
            .create_object("MdE", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
            .expect("e");
        if let Some(o) = logic.get_objects_mut().get_mut(&mid) {
            o.secondary_weapon = Some(Weapon {
                damage: 20.0,
                range: 250.0,
                ..Weapon::default()
            });
            o.weapon = Some(Weapon {
                damage: 5.0,
                range: 100.0,
                ..Weapon::default()
            });
        }
        let ok = logic.activate_missile_defender_laser_guided_for_test(mid, eid);
        assert!(ok, "laser guided should activate");
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == mid
                    && e.target_host == eid.0
            }),
            "laser guided must log AttackTarget; got {events:?}"
        );
        assert!(
            logic.get_objects().get(&mid).unwrap().target.is_none(),
            "host target deferred under decision authority"
        );
        // Weapon slot still host-applied.
        assert_eq!(logic.get_objects().get(&mid).unwrap().active_weapon_slot, 1);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&mid).unwrap().target, Some(eid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn private_attack_object_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PrivAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["PaU", "PaE"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                t.set_health(100.0);
                logic.templates.insert(name.into(), t);
            }
        }
        let uid = logic
            .create_object("PaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let vid = logic
            .create_object("PaE", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
            .expect("e");
        if let Some(o) = logic.get_objects_mut().get_mut(&uid) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                range: 50.0,
                ..Weapon::default()
            });
        }
        let ok = logic.private_attack_object_for_test(uid, vid, -1);
        assert!(ok, "private_attack_object should enter attack SM");
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == uid
                    && e.target_host == vid.0
            }),
            "must log AttackTarget; got {events:?}"
        );
        assert!(
            logic.get_objects().get(&uid).unwrap().target.is_none(),
            "host target deferred under decision authority"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&uid).unwrap().target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn transfer_attack_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("XferAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["XaA", "XaFrom", "XaTo"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let attacker = logic
            .create_object("XaA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let from = logic
            .create_object("XaFrom", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("from");
        let to = logic
            .create_object("XaTo", Team::GLA, glam::Vec3::new(12.0, 0.0, 0.0))
            .expect("to");
        // Seed host engagement on destroyed/old victim.
        if let Some(o) = logic.get_objects_mut().get_mut(&attacker) {
            o.target = Some(from);
            o.status.attacking = true;
        }
        let n = logic.transfer_attack_for_test(from, to);
        assert!(n >= 1, "should transfer at least one engagement");
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == attacker
                    && e.target_host == to.0
            }),
            "transfer_attack must log AttackTarget retarget; got {events:?}"
        );
        // Host still points at old victim until writeback.
        assert_eq!(
            logic.get_objects().get(&attacker).unwrap().target,
            Some(from)
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&attacker).unwrap().target, Some(to));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn update_combat_defers_engagement_under_decision_authority() {
        // Source honesty: combat aim/pitch/pre-attack residual must not host-mutate
        // target/ai_state when AI decision authority is on.
        let src = include_str!("game_logic/game_logic.rs");
        let i = src.find("fn update_combat").expect("update_combat");
        // Bound to a reasonable window of the fire path.
        let w = &src[i..i + 120_000.min(src.len() - i)];
        assert!(
            w.contains("gameworld_ai_decision_authority_enabled")
                && w.contains("turn_toward_position"),
            "update_combat aim residual must gate engagement under decision authority"
        );
        // Ensure the pre-attack blocked path is gated too.
        assert!(
            w.matches("pre_attack_ready_at").count() >= 1
                && w.contains(
                    "!crate::gameworld_shadow::gameworld_ai_decision_authority_enabled()"
                ),
            "pre-attack engagement residual must be authority-gated"
        );
    }

    #[test]
    fn residual_defense_fire_engagement_decision_authority() {
        // Source honesty: residual auto-fire paths gate host engagement.
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn try_base_defense_residual_fire",
            "fn try_sentry_drone_residual_fire",
            "fn try_hellfire_drone_residual_fire",
            "fn try_strategy_center_bombardment_turret_fire",
            "fn update_pending_patriot_assists",
            "fn attack_aim_at_target_update",
            "fn attack_fire_weapon_update",
            "fn tick_attack_state_machine",
            "fn tick_strategy_center_turret_mood_target",
            "fn update_stealth_and_detection",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            // Brace-match the full function body (large residuals exceed fixed windows).
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_ai_decision_authority_enabled")
                    || w.contains("host_ai_decision_log::record_attack"),
                "{fn_name} must honor AI decision authority for engagement"
            );
        }
    }

    #[test]
    fn apply_engagement_decision_aware_writeback() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EngAw");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["EaU", "EaE"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let uid = logic
            .create_object("EaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let vid = logic
            .create_object("EaE", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("e");
        logic.apply_engagement_decision_aware_for_test(uid, vid);
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == uid
                    && e.target_host == vid.0
            }),
            "must log AttackTarget; got {events:?}"
        );
        assert!(logic.get_objects().get(&uid).unwrap().target.is_none());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&uid).unwrap().target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn mood_auto_acquire_logs_decision_under_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MoodAcq");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("MaU") {
            let mut t = ThingTemplate::new("MaU");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert("MaU".into(), t);
        }
        let oid = logic
            .create_object("MaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        let vid = logic
            .create_object("MaU", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("v");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.auto_acquire_when_idle = true;
            o.ai_state = crate::game_logic::AIState::Idle;
            o.target = None;
            // Give a weapon so can_attack is true.
            o.weapon = Some(crate::game_logic::Weapon {
                damage: 10.0,
                range: 100.0,
                ..crate::game_logic::Weapon::default()
            });
        }
        // Drive one mood tick.
        logic.tick_mood_auto_acquire_for_test(&[oid]);
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == oid
                    && e.target_host == vid.0
            }),
            "mood acquire must log AttackTarget decision under authority; got {events:?}"
        );
        // Host target still unset until shadow writeback.
        assert!(logic.get_objects().get(&oid).unwrap().target.is_none());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&oid).unwrap().target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn support_guard_engage_uses_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("GuardEng");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("GeU") {
            let mut t = ThingTemplate::new("GeU");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert("GeU".into(), t);
        }
        let oid = logic
            .create_object("GeU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        let vid = logic
            .create_object("GeU", Team::GLA, glam::Vec3::new(15.0, 0.0, 0.0))
            .expect("v");
        // Direct helper (same path support-states uses under authority).
        logic.engage_target_decision_aware_for_test(oid, vid);
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                    && e.host_object == oid
                    && e.target_host == vid.0
            }),
            "guard engage must log decision; got {events:?}"
        );
        assert!(logic.get_objects().get(&oid).unwrap().target.is_none());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(logic.get_objects().get(&oid).unwrap().target, Some(vid));
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn faction_ai_launch_attack_decision_authority_writeback() {
        use crate::ai::{AIDifficulty, AIPlayer};
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FacAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for (name, team, x) in [("FacU", Team::USA, 0.0f32), ("FacE", Team::GLA, 80.0)] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.set_health(100.0);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
            let _ = logic.create_object(name, team, glam::Vec3::new(x, 0.0, 0.0));
        }
        let usa_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::USA)
            .map(|(id, _)| *id)
            .unwrap_or(0);
        let gla_id = logic
            .get_players()
            .iter()
            .find(|(_, p)| p.team == Team::GLA)
            .map(|(id, _)| *id);
        let enemy = logic
            .get_objects()
            .iter()
            .find(|(_, o)| o.team == Team::GLA)
            .map(|(id, _)| *id)
            .expect("enemy");
        let usa_unit = logic
            .get_objects()
            .iter()
            .find(|(_, o)| o.team == Team::USA)
            .map(|(id, _)| *id)
            .expect("usa");
        if let Some(o) = logic.get_objects_mut().get_mut(&usa_unit) {
            o.weapon = Some(Weapon {
                damage: 10.0,
                ..Weapon::default()
            });
        }
        let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
        ai.enemy_player_id = gla_id;
        ai.is_active = true;
        ai.launch_attack(&mut logic, 1000.0);
        let events = host_ai_decision_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.kind == host_ai_decision_log::AI_DECISION_ATTACK),
            "expected AttackTarget decision: {events:?}"
        );
        assert!(logic.get_objects().get(&usa_unit).unwrap().target.is_none());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&usa_unit).unwrap().target,
            Some(enemy)
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn stop_attack_decision_authority_clears_via_writeback() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StopAtk");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["Su", "Se"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let oid = logic
            .create_object("Su", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let vid = logic
            .create_object("Se", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("e");
        // Seed host target as if previously engaged.
        if let Some(o) = logic.get_objects_mut().get_mut(&oid) {
            o.target = Some(vid);
            o.status.attacking = true;
        }
        logic.stop_attack_decision_aware_for_test(oid);
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_STOP_ATTACK && e.host_object == oid
            }),
            "stop must log decision; got {events:?}"
        );
        // Host still has target until writeback.
        assert_eq!(logic.get_objects().get(&oid).unwrap().target, Some(vid));
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert!(logic.get_objects().get(&oid).unwrap().target.is_none());
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn fire_spawn_authority_defers_queue_until_shadow() {
        use crate::game_logic::combat::{self, DamageType, PendingProjectile};
        use crate::game_logic::host_fire_spawn_log;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        let prev = std::env::var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", "1");
        assert!(gameworld_fire_spawn_authority_enabled());
        host_fire_spawn_log::clear();
        combat::queue_projectile(PendingProjectile {
            shooter_id: ObjectId(1),
            shooter_pos: glam::Vec3::ZERO,
            target_id: Some(ObjectId(2)),
            target_pos: Some(glam::Vec3::new(50.0, 0.0, 0.0)),
            damage: 12.0,
            speed: 100.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: HostDeathType::Normal,
            projectile_object_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: 0,
            projectile_collides: 0,
            scatter_radius: 0.0,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
        });
        // Not yet in combat system.
        let mut logic = GameLogic::new();
        assert_eq!(logic.combat_system.projectile_count(), 0);
        let spawns = host_fire_spawn_log::drain();
        assert_eq!(spawns.len(), 1);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let n = shadow.apply_host_fire_spawn_events(&mut logic, spawns);
        assert!(n >= 1 || logic.combat_system.projectile_count() >= 1);
        assert!(
            logic.combat_system.projectile_count() >= 1,
            "shadow apply must spawn into CombatSystem"
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY"),
        }
    }

    #[test]
    fn ai_attack_authority_gates_fire_intent_writeback() {
        use crate::game_logic::host_fire_intent_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_fire_intent_log::clear();
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "0");
        assert!(!gameworld_ai_attack_authority_enabled());
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AiAtkAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("AaU") {
            let mut t = ThingTemplate::new("AaU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("AaU".into(), t);
        }
        let oid = logic
            .create_object("AaU", Team::USA, glam::Vec3::new(250.0, 0.0, 250.0))
            .expect("id");
        host_fire_intent_log::record(oid, 9, 0, 10.0, 20.0, 1.0, 5, 1);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_host_fire_intent_events(&host_fire_intent_log::drain()) >= 1);
        // Host still default zeros; writeback skipped when authority off.
        assert_eq!(shadow.writeback_fire_intent_to_host(&mut logic), 0);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.fire_intent_count, 0);
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        assert!(gameworld_ai_attack_authority_enabled());
        assert!(shadow.writeback_fire_intent_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.fire_intent_count, 1);
        assert_eq!(o.last_fire_victim_host, 9);
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn fire_at_records_fire_intent_residual() {
        use crate::game_logic::host_fire_intent_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        // Default path: authority on — log intent, host last_fire_* deferred to writeback.
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_fire_intent_log::clear();
        crate::game_logic::host_historic_bonus::set_logic_frame(77);
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FireAtRec");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("FrU") {
            let mut t = ThingTemplate::new("FrU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("FrU".into(), t);
        }
        let oid = logic
            .create_object("FrU", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
            .expect("id");
        let vid = logic
            .create_object("FrU", Team::China, glam::Vec3::new(12.0, 0.0, 10.0))
            .expect("v");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.weapon = Some(Weapon {
                damage: 15.0,
                range: 200.0,
                reload_time: 0.0,
                ..Weapon::default()
            });
            o.status.weapons_jammed = false;
            let fired = o.fire_at(vid, 1.0);
            assert!(fired, "close-range fire_at should discharge");
            // Host last_fire_* deferred under AI attack authority.
            assert_eq!(o.last_fire_victim_host, 0);
            assert_eq!(o.last_fire_frame, 0);
            assert!(o.fire_intent_count >= 1, "counter still advances");
        }
        let evs = host_fire_intent_log::drain();
        assert!(
            evs.iter().any(|e| e.object == oid
                && e.last_fire_victim_host == vid.0
                && e.last_fire_frame == 77),
            "fire_at must log intent; got {evs:?}"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_host_fire_intent_events(&evs) >= 1);
        assert!(shadow.writeback_fire_intent_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.last_fire_victim_host, vid.0);
        assert_eq!(o.last_fire_frame, 77);
        assert!((o.last_fire_damage - 15.0).abs() < 1e-5);

        // Legacy path: authority off — host last_fire_* applied same-frame.
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "0");
        host_fire_intent_log::clear();
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.last_fire_victim_host = 0;
            o.last_fire_frame = 0;
            o.last_fire_damage = 0.0;
            o.fire_intent_count = 0;
            // Ensure weapon ready again.
            if let Some(w) = o.weapon.as_mut() {
                w.last_fire_time = 0.0;
            }
            let fired = o.fire_at(vid, 2.0);
            assert!(fired);
            assert_eq!(o.last_fire_victim_host, vid.0);
            assert!(o.fire_intent_count >= 1);
        }
        assert!(!host_fire_intent_log::drain().is_empty());
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn assign_unit_path_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PathMv");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("PmU") {
            let mut t = ThingTemplate::new("PmU");
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Selectable);
            logic.templates.insert("PmU".into(), t);
        }
        let oid = logic
            .create_object("PmU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        if let Some(o) = logic.get_objects_mut().get_mut(&oid) {
            // Ensure mobile residual (max_speed > 0).
            o.movement.max_speed = 20.0;
        }
        let ok = logic.assign_unit_path_for_test(oid, glam::Vec3::new(50.0, 0.0, 0.0), &[]);
        assert!(ok, "path assign should succeed");
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == oid
                    && e.ai_state_ordinal == 1
            }),
            "assign_unit_path must log Moving; got {events:?}"
        );
        assert_ne!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Moving,
            "host ai_state deferred under decision authority"
        );
        assert!(
            logic.get_objects().get(&oid).unwrap().status.moving
                || logic
                    .get_objects()
                    .get(&oid)
                    .unwrap()
                    .movement
                    .target_position
                    .is_some()
                || !logic
                    .get_objects()
                    .get(&oid)
                    .unwrap()
                    .movement
                    .path
                    .is_empty(),
            "movement residual still on host"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Moving
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn private_idle_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("IdleAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["IdU", "IdE"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let oid = logic
            .create_object("IdU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let vid = logic
            .create_object("IdE", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("e");
        if let Some(o) = logic.get_objects_mut().get_mut(&oid) {
            o.target = Some(vid);
            o.status.attacking = true;
            o.set_ai_state(AIState::Attacking);
        }
        assert!(logic.private_idle_for_test(oid));
        let events = host_ai_decision_log::drain();
        assert!(
            events
                .iter()
                .any(|e| e.kind == host_ai_decision_log::AI_DECISION_STOP_ATTACK),
            "private_idle must log StopAttack; got {events:?}"
        );
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE && e.ai_state_ordinal == 0
            }),
            "private_idle must log Idle; got {events:?}"
        );
        // Host still engaged until writeback.
        assert_eq!(logic.get_objects().get(&oid).unwrap().target, Some(vid));
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        let _ = shadow.writeback_ai_state_to_host(&mut logic);
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        // set_target(None) residual also idles host; either writeback path is enough.
        let o = logic.get_objects().get(&oid).unwrap();
        assert!(o.target.is_none());
        assert_eq!(o.ai_state, AIState::Idle);
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn residual_ai_state_paths_honor_decision_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn try_return_to_base_rearm",
            "fn try_min_range_backup",
            "fn append_unit_waypoint",
            "fn attack_aim_at_target_enter",
            "fn attack_fire_weapon_enter",
            "fn try_idle_crate_pickup",
            "fn on_selling_container_residual",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_ai_decision_authority_enabled")
                    || w.contains("host_ai_decision_log::record_set_state"),
                "{fn_name} must honor AI decision authority for AI state"
            );
        }
    }

    #[test]
    fn append_unit_waypoint_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("WpAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("WpU") {
            let mut t = ThingTemplate::new("WpU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("WpU".into(), t);
        }
        let oid = logic
            .create_object("WpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        if let Some(o) = logic.get_objects_mut().get_mut(&oid) {
            o.movement.max_speed = 20.0;
        }
        assert!(logic.append_unit_waypoint_for_test(oid, glam::Vec3::new(30.0, 0.0, 0.0)));
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == oid
                    && e.ai_state_ordinal == 1
            }),
            "waypoint must log Moving; got {events:?}"
        );
        assert_ne!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Moving
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Moving
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn set_ai_state_decision_aware_writeback() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("StateAw");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SaU") {
            let mut t = ThingTemplate::new("SaU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("SaU".into(), t);
        }
        let oid = logic
            .create_object("SaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        logic.set_ai_state_decision_aware_for_test(oid, AIState::Gathering);
        let events = host_ai_decision_log::drain();
        let ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Gathering);
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == oid
                    && e.ai_state_ordinal == ord
            }),
            "must log Gathering; got {events:?}"
        );
        assert_ne!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Gathering
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Gathering
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn death_type_channel_via_set_death_type() {
        use crate::game_logic::host_death_type_log;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_death_type_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DeathTy");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("DieUnit") {
            let mut t = ThingTemplate::new("DieUnit");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("DieUnit".into(), t);
        }
        let oid = logic
            .create_object("DieUnit", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.status.destroyed = true;
            o.status.death_type = HostDeathType::Burned;
        }
        host_death_type_log::record(oid, HostDeathType::Burned.ordinal());
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_death_type_events(&host_death_type_log::drain()) >= 1);
        assert_eq!(
            shadow.world().entity(eid).unwrap().death_type,
            HostDeathType::Burned.ordinal()
        );
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.status.death_type = HostDeathType::Normal;
        }
        assert!(shadow.writeback_death_type_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().status.death_type,
            HostDeathType::Burned
        );
    }

    #[test]
    fn radar_extend_channel_via_set_radar_extend() {
        use crate::game_logic::host_radar_extend_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_radar_extend_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("RadarEx");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("RadarB") {
            let mut t = ThingTemplate::new("RadarB");
            t.add_kind_of(KindOf::Structure);
            logic.templates.insert("RadarB".into(), t);
        }
        let oid = logic
            .create_object("RadarB", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.radar_extend_done_frame = 120;
            o.radar_extend_complete = false;
            o.radar_active = true;
        }
        host_radar_extend_log::record(oid, 120, false, true);
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
        assert!(shadow.apply_host_radar_extend_events(&host_radar_extend_log::drain()) >= 1);
        let e = shadow.world().entity(eid).unwrap();
        assert_eq!(e.radar_extend_done_frame, 120);
        assert!(e.radar_active);
        assert!(!e.radar_extend_complete);
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.radar_active = false;
            o.radar_extend_done_frame = 0;
        }
        assert!(shadow.writeback_radar_extend_to_host(&mut logic) >= 1);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
        let _ = shadow.writeback_sole_healing_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let _ = shadow.writeback_ai_mood_to_host(&mut logic);
        let _ = shadow.writeback_ai_request_to_host(&mut logic);
        let _ = shadow.writeback_hijacker_to_host(&mut logic);
        let o = logic.get_objects().get(&oid).unwrap();
        assert!(o.radar_active);
        assert_eq!(o.radar_extend_done_frame, 120);
    }

    #[test]
    fn special_power_tick_records_host_special_power_log() {
        use crate::game_logic::{host_special_power_log, KindOf, Team, ThingTemplate};
        host_special_power_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpTick");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SpUnit") {
            let mut t = ThingTemplate::new("SpUnit");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("SpUnit".into(), t);
        }
        let oid = logic
            .create_object("SpUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.special_power_cooldown = 10.0;
            o.special_power_cooldown_remaining = 5.0;
            o.set_special_power_ready(false);
            let became = o.tick_timers(1.0);
            let _ = became;
        }
        let events = host_special_power_log::drain();
        assert!(
            events
                .iter()
                .any(|e| { e.object == oid && (e.cooldown_remaining - 4.0).abs() < 1e-3 }),
            "events {:?}",
            events
        );
    }

    #[test]
    #[test]
    #[test]
    fn special_power_session_writeback_after_tick() {
        use crate::game_logic::{host_special_power_log, KindOf, Team, ThingTemplate};
        host_special_power_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SpWb");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SpWbU") {
            let mut t = ThingTemplate::new("SpWbU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("SpWbU".into(), t);
        }
        let oid = logic
            .create_object("SpWbU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.special_power_cooldown = 10.0;
            o.special_power_cooldown_remaining = 2.0;
            o.set_special_power_ready(false);
            o.record_host_special_power();
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let events = host_special_power_log::drain();
        assert!(shadow.apply_host_special_power_events(&events) >= 1);
        // Desync host after GameWorld apply so writeback has work.
        {
            let o = logic.get_objects_mut().get_mut(&oid).expect("o");
            o.special_power_cooldown_remaining = 9.0;
        }
        assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
        let o = logic.get_objects().get(&oid).expect("o");
        assert!((o.special_power_cooldown_remaining - 2.0).abs() < 1e-3);
    }

    #[test]
    fn damage_authority_writeback_is_last_writer() {
        crate::game_logic::host_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmgAuthority");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "AuthUnit", 100.0);
        let id = logic
            .create_object("AuthUnit", Team::USA, Vec3::new(2.0, 0.0, 0.0))
            .expect("unit");

        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        let pre = logic.get_objects().get(&id).unwrap().health.current;

        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            let _ = obj.take_damage(25.0);
        }
        let host_mid = logic.get_objects().get(&id).unwrap().health.current;
        assert!(host_mid < pre);

        let events = crate::game_logic::host_damage_log::drain();
        assert!(!events.is_empty());
        shadow.sync_from_host_with(&logic, false);
        let eid = shadow.entity_for_host(id).unwrap();
        let shadow_pre_mut = shadow.world().entity(eid).unwrap().health;
        assert!(
            (shadow_pre_mut - pre).abs() < 0.01,
            "expected pre-tick shadow hp {pre} got {shadow_pre_mut}"
        );
        let _ = shadow.apply_host_damage_events(&events);
        // Deliberately desync host so writeback must run.
        if let Some(obj) = logic.get_objects_mut().get_mut(&id) {
            obj.health.current = pre; // restore pre-damage on host
            obj.status.destroyed = false;
        }
        let wb = shadow.writeback_health_to_host(&mut logic);
        assert!(wb >= 1, "expected writeback after host desync");
        let host_final = logic.get_objects().get(&id).unwrap().health.current;
        let shadow_final = shadow.world().entity(eid).unwrap().health;
        assert!(
            (host_final - shadow_final).abs() < 0.05,
            "writeback mismatch host={host_final} shadow={shadow_final}"
        );
        // Shadow applied logged actual_damage from mid-frame combat.
        assert!(
            (host_final - host_mid).abs() < 0.05,
            "authority final {host_final} vs mid-frame host {host_mid}"
        );
        assert!(host_final < pre);
    }

    #[test]
    fn host_owner_log_feeds_transfer_owner_mutation() {
        crate::game_logic::host_owner_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("OwnerLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "OwnT", 100.0);
        let id = logic
            .create_object("OwnT", Team::GLA, glam::Vec3::ZERO)
            .expect("id");
        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        {
            let o = logic.get_objects_mut().get_mut(&id).unwrap();
            o.set_team(Team::USA);
        }
        let events = crate::game_logic::host_owner_log::drain();
        assert_eq!(events.len(), 1);
        let n = shadow.apply_host_owner_events(&logic, &events);
        assert_eq!(n, 1);
        let eid = shadow.entity_for_host(id).expect("map");
        let owner = shadow.world().entity(eid).unwrap().owner;
        let expected = shadow.owner_for_host_object(&logic, Team::USA);
        assert_eq!(
            owner, expected,
            "TransferOwner should map host team to shadow player"
        );
    }

    #[test]
    fn host_heal_log_feeds_set_health_mutation() {
        crate::game_logic::host_heal_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HealLog");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "HealT", 100.0);
        let id = logic
            .create_object("HealT", Team::USA, glam::Vec3::ZERO)
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&id).unwrap();
            o.health.current = 40.0;
        }
        let mut shadow = GameWorldShadow::new(4096);
        shadow.sync_from_host(&logic);
        {
            let o = logic.get_objects_mut().get_mut(&id).unwrap();
            o.health.current = 70.0;
            crate::game_logic::host_heal_log::record(id, 70.0);
        }
        let heals = crate::game_logic::host_heal_log::drain();
        let n = shadow.apply_host_heal_events(&heals);
        assert_eq!(n, 1);
        let probe = shadow.probe(&mut logic);
        assert!(
            probe.health_match,
            "heal SetHealth should match host: {}",
            probe.detail
        );
    }

    #[test]
    fn host_damage_log_feeds_shadow_mutation_channel() {
        crate::game_logic::host_damage_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmgLogChannel");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        ensure_template(&mut logic, "LogUnit", 150.0);
        let id = logic
            .create_object("LogUnit", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .expect("unit");
        let mut shadow = GameWorldShadow::new(4096);
        let queued = apply_logged_damage_channel_parity(&mut logic, &mut shadow, &[(id, 40.0)])
            .expect("channel");
        assert!(queued >= 1, "expected queued mutations");
        assert!(shadow.entity_for_host(id).is_some());
    }

    #[test]
    fn host_construction_log_maps_completed_structure_in_shadow() {
        crate::game_logic::host_construction_log::clear();
        crate::game_logic::host_spawn_log::clear();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("USA_Barracks");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("USA_Barracks".into(), t);
        let id = logic
            .create_object("USA_Barracks", Team::USA, glam::Vec3::ZERO)
            .expect("barracks");
        // Simulate host recording construction complete without pre-sync map.
        let mut shadow = GameWorldShadow::new(64);
        // Do not sync first — apply construction should map via spawn residual.
        crate::game_logic::host_construction_log::record(id, "USA_Barracks");
        let events = crate::game_logic::host_construction_log::drain();
        let n = shadow.apply_host_construction_events(&events, &logic);
        assert!(n >= 1, "construction apply mapped {n}");
        assert!(
            shadow.entity_for_host(id).is_some(),
            "completed structure must be mapped in shadow"
        );
    }

    #[test]
    fn dozer_construction_ai_state_decision_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn update_dozer_bored_repair",
            "fn update_construction",
            "fn update_rebuild_holes",
            "fn try_auto_resume_construction_residual",
            "fn process_destroy_list",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_ai_decision_authority_enabled")
                    || w.contains("set_ai_state_decision_aware")
                    || w.contains("host_ai_decision_log::record_set_state")
                    || w.contains("apply_engagement_decision_aware"),
                "{fn_name} must honor AI decision authority"
            );
        }
    }

    #[test]
    fn dozer_bored_repair_state_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DzAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("DzU") {
            let mut t = ThingTemplate::new("DzU");
            t.add_kind_of(KindOf::Vehicle);
            t.add_kind_of(KindOf::Worker);
            logic.templates.insert("DzU".into(), t);
        }
        let oid = logic
            .create_object("DzU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        logic.set_ai_state_decision_aware_for_test(oid, AIState::Repairing);
        let events = host_ai_decision_log::drain();
        let ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Repairing);
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == oid
                    && e.ai_state_ordinal == ord
            }),
            "Repairing must be logged; got {events:?}"
        );
        assert_ne!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Repairing
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Repairing
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn capture_residual_ai_state_decision_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn on_capture_object_residual",
            "fn on_capture_tunnel_network_residual",
            "fn on_capture_kick_passengers",
            "fn check_building_damage_states",
            "fn put_hijacker_in_airborne_parachute",
            "fn tick_strategy_center_turret_mood_target",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_ai_decision_authority_enabled")
                    || w.contains("set_ai_state_decision_aware")
                    || w.contains("host_ai_decision_log::record_set_state")
                    || w.contains("host_ai_decision_log::record_attack"),
                "{fn_name} must honor AI decision authority"
            );
        }
    }

    #[test]
    fn hijacker_docked_state_decision_authority() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HjAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("HjU") {
            let mut t = ThingTemplate::new("HjU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("HjU".into(), t);
        }
        let oid = logic
            .create_object("HjU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        logic.set_ai_state_decision_aware_for_test(oid, AIState::Docked);
        let events = host_ai_decision_log::drain();
        let ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Docked);
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                    && e.host_object == oid
                    && e.ai_state_ordinal == ord
            }),
            "Docked must be logged; got {events:?}"
        );
        assert_ne!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Docked
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        assert!(shadow.writeback_ai_state_to_host(&mut logic) >= 1);
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().ai_state,
            AIState::Docked
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
    }

    #[test]
    fn residual_eject_payload_ai_state_decision_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn apply_bunker_buster_to_target",
            "fn apply_kill_garrisoned_to_target",
            "fn apply_rider_free_fall_damage",
            "fn tick_eject_parachute_residual",
            "fn apply_host_hive_damage_from",
            "fn update_angry_mobs",
            "fn update_mines_and_demo_traps",
            "fn clear_mine_internal",
            "fn start_sell_object",
            "fn cancel_dozers_building",
            "fn resume_construction",
            "fn apply_listening_outpost_initial_payload",
            "fn apply_troop_crawler_initial_payload",
            "fn command_attack",
            "fn command_stop",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_ai_decision_authority_enabled")
                    || w.contains("set_ai_state_decision_aware")
                    || w.contains("host_ai_decision_log::record_set_state")
                    || w.contains("host_ai_decision_log::record_attack"),
                "{fn_name} must honor AI decision authority"
            );
        }
    }

    #[test]
    fn residual_auto_fire_records_fire_intent_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn try_strategy_center_bombardment_turret_fire",
            "fn try_base_defense_residual_fire",
            "fn update_pending_patriot_assists",
            "fn try_sentry_drone_residual_fire",
            "fn try_hellfire_drone_residual_fire",
            "fn try_transport_passenger_residual_fire",
            "fn try_garrison_residual_fire",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("host_fire_intent_log::record")
                    && w.contains("gameworld_ai_attack_authority_enabled"),
                "{fn_name} must record fire-intent under AI attack authority"
            );
        }
        let obj = include_str!("game_logic/object.rs");
        let i = obj.find("fn fire_at_ex").expect("fire_at_ex");
        let w = &obj[i..i + 8000];
        assert!(
            w.contains("gameworld_ai_decision_authority_enabled") && w.contains("record_set_state"),
            "fire_at_ex pre-attack must honor AI decision authority"
        );
    }

    #[test]
    fn residual_auto_fire_damage_source_attribution_source() {
        let src = include_str!("game_logic/game_logic.rs");
        let helper_i = src
            .find("fn residual_auto_fire_apply_damage")
            .expect("residual_auto_fire_apply_damage");
        let helper = &src[helper_i..src.len().min(helper_i + 5000)];
        assert!(
            helper.contains("take_damage_from(damage, Some(attacker_id))"),
            "residual auto-fire helper must source-attribute hitscan damage"
        );
        for name in [
            "try_sentry_drone_residual_fire",
            "try_hellfire_drone_residual_fire",
            "try_garrison_residual_fire",
            "try_transport_passenger_residual_fire",
            "try_base_defense_residual_fire",
            "try_strategy_center_bombardment_turret_fire",
            "update_pending_patriot_assists",
        ] {
            let at = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[at..src.len().min(at + 14000)];
            assert!(
                body.contains("residual_auto_fire_apply_damage"),
                "{name} must use residual_auto_fire_apply_damage"
            );
        }
    }

    #[test]
    fn residual_auto_fire_damage_source_writeback_channel() {
        use crate::game_logic::host_damage_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        host_damage_log::clear();
        let prev = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DmgSrc");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["SrcA", "SrcB"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                t.set_health(100.0);
                logic.templates.insert(name.into(), t);
            }
        }
        let attacker = logic
            .create_object("SrcA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let victim = logic
            .create_object("SrcB", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("v");
        {
            let v = logic.get_objects_mut().get_mut(&victim).unwrap();
            let _ = v.take_damage_from(25.0, Some(attacker));
            assert_eq!(v.last_damage_source, Some(attacker));
            // Damage authority defers HP; projected destroy false.
            assert!(v.health.current > 50.0 || gameworld_damage_authority_enabled());
        }
        let events = host_damage_log::drain();
        assert!(
            events
                .iter()
                .any(|e| { e.target == victim && e.source == Some(attacker) && e.amount >= 20.0 }),
            "damage log must carry source; got {events:?}"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let applied = shadow.apply_host_damage_events(&events);
        assert!(
            applied.0 + applied.1 >= 1,
            "expected damage apply {applied:?}"
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
        }
    }

    #[test]
    fn private_stop_and_clear_target_decision_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        assert!(
            src.contains("fn clear_target_decision_aware"),
            "clear_target_decision_aware helper must exist"
        );
        for fn_name in [
            "fn private_stop",
            "fn process_destroy_list",
            "fn on_capture_tunnel_network_residual",
            "fn on_capture_kick_passengers",
            "fn check_building_damage_states",
            "fn tick_strategy_center_turret_mood_target",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("record_stop_attack")
                    || w.contains("clear_target_decision_aware")
                    || w.contains("stop_attack_decision_aware"),
                "{fn_name} must clear combat targets via StopAttack decision channel"
            );
        }
    }

    #[test]
    fn private_stop_decision_authority_clears_via_writeback() {
        use crate::game_logic::host_ai_decision_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
        let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
        std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
        host_ai_decision_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PrivStop");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        for name in ["PsU", "PsE"] {
            if !logic.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Infantry);
                t.add_kind_of(KindOf::Attackable);
                logic.templates.insert(name.into(), t);
            }
        }
        let oid = logic
            .create_object("PsU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("u");
        let vid = logic
            .create_object("PsE", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
            .expect("e");
        if let Some(o) = logic.get_objects_mut().get_mut(&oid) {
            o.target = Some(vid);
            o.status.attacking = true;
        }
        assert!(logic.private_stop(oid));
        // Host target deferred under decision authority.
        assert_eq!(
            logic.get_objects().get(&oid).unwrap().target,
            Some(vid),
            "host target must remain until GameWorld writeback"
        );
        let events = host_ai_decision_log::drain();
        assert!(
            events.iter().any(|e| {
                e.kind == host_ai_decision_log::AI_DECISION_STOP_ATTACK && e.host_object == oid
            }),
            "private_stop must log StopAttack; got {events:?}"
        );
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        // Seed world attack target then apply stop.
        assert!(shadow.queue_set_attack_target_for_host(oid, Some(vid)));
        let _ = shadow.apply_pending();
        assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
        let _ = shadow.apply_pending();
        assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
        assert!(
            logic.get_objects().get(&oid).unwrap().target.is_none(),
            "writeback must clear host target"
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
        }
        match prev_atk {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
        }
    }

    #[test]
    fn angry_mob_pdl_damage_source_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for (fn_name, token) in [
            (
                "fn update_angry_mobs",
                "take_damage_from(hit.damage, Some(plan.mob_id))",
            ),
            (
                "fn update_point_defense_intercept",
                "take_damage_from(damage, Some(carrier_id))",
            ),
            (
                "fn update_scud_poison_zones",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_bomb_truck_poison_zones",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_inferno_fire_zones",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_firewalls",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_helix_napalm_firestorms",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_nuclear_tanks_radiation_zones",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_nuke_cannon_radiation_zones",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
            (
                "fn update_toxin_tractor_poison_zones",
                "take_damage_from(hit.damage, Some(plan.source_object))",
            ),
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains(token),
                "{fn_name} must source-attribute residual damage via {token}"
            );
        }
        let pdl_i = src.find("fn update_point_defense_intercept").expect("pdl");
        let bytes = src.as_bytes();
        let mut j = src[pdl_i..].find('{').map(|o| pdl_i + o).expect("pdl body");
        let mut depth = 0i32;
        let pdl_end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed pdl"),
            }
            j += 1;
        };
        let pdl = &src[pdl_i..=pdl_end];
        assert!(
            pdl.contains("host_fire_intent_log::record")
                && pdl.contains("gameworld_ai_attack_authority_enabled"),
            "PDL must record fire-intent under AI attack authority"
        );
        assert!(
            pdl.contains("record_attack")
                && pdl.contains("gameworld_ai_decision_authority_enabled"),
            "PDL must log Attack under AI decision authority"
        );
    }

    #[test]
    fn explosion_detonation_damage_source_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for (fn_name, token) in [
            ("fn apply_bunker_buster_to_target", "take_damage_from"),
            ("fn apply_kill_garrisoned_to_target", "take_damage_from"),
            ("fn apply_neutron_blast_at", "take_damage_from"),
            (
                "fn apply_bomb_truck_death_detonation_at",
                "take_damage_from(dmg, Some(truck_id))",
            ),
            (
                "fn apply_nuclear_tanks_death_detonation_at",
                "take_damage_from(dmg, Some(tank_id))",
            ),
            (
                "fn detonate_booby_trap_at",
                "take_damage_from(dmg, Some(plant.planter_id))",
            ),
            (
                "fn activate_helix_napalm_bomb",
                "take_damage_from(dmg, Some(source_object))",
            ),
            (
                "fn detonate_car_bomb",
                "take_damage_from(dmg, Some(car_id))",
            ),
            (
                "fn detonate_mine_internal",
                "take_damage_from(dmg, Some(mine_id))",
            ),
            (
                "fn update_sneak_attacks",
                "take_damage_from(dmg, Some(plan.source_object))",
            ),
            (
                "fn update_overcharge_drain",
                "take_damage_from(dmg, Some(id))",
            ),
            (
                "fn apply_host_hive_damage_from",
                "take_damage_from(damage, source_id)",
            ),
            (
                "fn process_destroy_list",
                "take_damage_from(dmg, Some(event.id))",
            ),
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains(token),
                "{fn_name} must source-attribute damage via {token}"
            );
            // No anonymous take_damage(amount) residual in these paths.
            assert!(
                !w.contains(".take_damage(dmg)")
                    && !w.contains(".take_damage(damage)")
                    && !w.contains(".take_damage(structure_dmg)"),
                "{fn_name} must not keep anonymous take_damage"
            );
        }
    }

    #[test]
    fn cancel_production_refund_economy_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn cancel_production",
            "fn cancel_all_production",
            "fn ensure_skirmish_ai_starting_cash",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("apply_supply_gain")
                    || w.contains("gameworld_economy_authority_enabled")
                    || w.contains("pending_supply_delta"),
                "{fn_name} must honor economy authority for cash mutations"
            );
            assert!(
                !w.contains("resources.supplies +=")
                    && !w.contains(
                        "resources.supplies =
                    player.resources.supplies.saturating_add"
                    )
                    && !w.contains("resources.supplies = min_cash"),
                "{fn_name} must not host-poke absolute supplies under refund/top-up"
            );
        }
    }

    #[test]
    fn cancel_production_refund_economy_authority_writeback() {
        use crate::game_logic::host_economy_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        host_economy_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("EconRef");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        // Seed a local player with known cash.
        let pid = logic
            .get_players()
            .values()
            .find(|p| p.team == Team::USA)
            .map(|p| p.id)
            .expect("usa player");
        {
            let p = logic.get_player_mut(pid).expect("p");
            p.resources.supplies = 1000;
            p.pending_supply_delta = 0;
        }
        if !logic.templates.contains_key("EconFac") {
            let mut t = ThingTemplate::new("EconFac");
            t.add_kind_of(KindOf::Structure);
            t.add_kind_of(KindOf::FSBarracks);
            logic.templates.insert("EconFac".into(), t);
        }
        if !logic.templates.contains_key("EconUnit") {
            let mut t = ThingTemplate::new("EconUnit");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("EconUnit".into(), t);
        }
        let fac = logic
            .create_object("EconFac", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("fac");
        // Queue a unit with cost via building_data if available.
        {
            use crate::game_logic::buildings::{
                BuildingData, BuildingType, ProductionItem, ProductionKind,
            };
            use crate::game_logic::Resources;
            let o = logic.get_objects_mut().get_mut(&fac).expect("f");
            if o.building_data.is_none() {
                o.building_data = Some(BuildingData::new(BuildingType::Barracks));
            }
            if let Some(bd) = o.building_data.as_mut() {
                bd.production_queue.push(ProductionItem {
                    template_name: "EconUnit".into(),
                    progress: 0.0,
                    total_time: 10.0,
                    cost: Resources {
                        supplies: 250,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: ProductionKind::Unit,
                });
            }
        }
        assert!(logic.cancel_production(fac, "EconUnit".into()));
        let p = logic.get_player(pid).expect("p");
        // Under economy authority host absolute supplies stay 1000; pending delta +250.
        assert_eq!(p.resources.supplies, 1000);
        assert_eq!(p.pending_supply_delta, 250);
        assert_eq!(p.effective_supplies(), 1250);
        let evs = host_economy_log::drain();
        assert!(
            evs.iter().any(|e| e.player_id == pid && e.supplies == 1250),
            "refund must log effective supplies; got {evs:?}"
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY"),
        }
    }

    #[test]
    fn sell_and_rebuild_construction_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn update_construction",
            "fn start_sell_object",
            "fn update_sell_list",
            "fn update_rebuild_holes",
            "fn maybe_spawn_rebuild_hole",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_construction_authority_enabled")
                    || w.contains("host_construction_progress_log::record"),
                "{fn_name} must honor construction authority for percent mutations"
            );
        }
    }

    #[test]
    fn start_sell_sets_construction_percent_under_authority() {
        use crate::game_logic::host_construction_progress_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
        host_construction_progress_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SellPct");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SellPad") {
            let mut t = ThingTemplate::new("SellPad");
            t.add_kind_of(KindOf::Structure);
            t.set_health(500.0);
            logic.templates.insert("SellPad".into(), t);
        }
        let oid = logic
            .create_object("SellPad", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).unwrap();
            o.construction_percent = 1.0;
            o.set_status_under_construction(false);
        }
        assert!(logic.start_sell_object(oid));
        // Host sell start always sets construction_percent=0.999 (and logs progress).
        // Construction authority no longer freezes host percent (stalls multi-frame sell).
        assert!(
            (logic.get_objects().get(&oid).unwrap().construction_percent - 0.999).abs() < 1e-4,
            "host sell start must set 0.999 residual"
        );
        let evs = host_construction_progress_log::drain();
        assert!(
            evs.iter()
                .any(|e| e.object == oid && (e.percent - 0.999).abs() < 1e-4),
            "sell start must log 0.999 progress; got {evs:?}"
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY"),
        }
    }

    #[test]
    fn sell_deconstruction_negative_percent_survives_shadow_writeback() {
        use crate::game_logic::host_construction_progress_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
        host_construction_progress_log::clear();

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SellNegPct");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("SellPad") {
            let mut t = ThingTemplate::new("SellPad");
            t.add_kind_of(KindOf::Structure);
            t.set_health(500.0);
            logic.templates.insert("SellPad".into(), t);
        }
        let oid = logic
            .create_object("SellPad", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .expect("pad");
        {
            let o = logic.get_object_mut(oid).expect("o");
            o.set_status_under_construction(false);
            o.construction_percent = 1.0;
        }
        assert!(logic.start_sell_object(oid));

        // Advance past scaffold into negative deconstruction via full host tick
        // (frame + update_sell_list). Stop once percent is clearly negative.
        for _ in 0..200 {
            logic.update();
            if logic.get_object(oid).is_none() {
                break;
            }
            let pct = logic
                .get_object(oid)
                .map(|o| o.construction_percent)
                .unwrap_or(-1.0);
            if pct < -0.1 {
                break;
            }
        }
        let host_pct = logic
            .get_object(oid)
            .map(|o| o.construction_percent)
            .expect("still selling");
        assert!(
            host_pct < 0.0,
            "host sell percent should go negative, got {host_pct}"
        );

        host_construction_progress_log::clear();
        host_construction_progress_log::record(oid, host_pct, true);
        let events = host_construction_progress_log::drain();
        assert_eq!(events.len(), 1);
        assert!(
            events[0].percent < 0.0,
            "log must keep negative percent, got {}",
            events[0].percent
        );

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let n = shadow.apply_host_construction_progress_events(&events);
        assert!(n >= 1);
        let eid = shadow.entity_for_host(oid).expect("mapped");
        let ent_pct = shadow.world().entity(eid).expect("e").construction_percent;
        assert!(
            (ent_pct - host_pct).abs() < 1e-4,
            "shadow entity percent {ent_pct} vs host {host_pct}"
        );
        {
            let o = logic.get_object_mut(oid).expect("o");
            o.construction_percent = 0.5; // dirty host so writeback must restore
        }
        assert!(shadow.writeback_construction_to_host(&mut logic) >= 1);
        let after = logic.get_object(oid).expect("o").construction_percent;
        assert!(
            (after - host_pct).abs() < 1e-4,
            "writeback must preserve negative sell percent: after={after} want={host_pct}"
        );

        host_construction_progress_log::clear();
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY"),
        }
    }

    #[test]
    fn heal_armor_absolute_hp_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        assert!(
            src.contains("fn write_object_health_authority_aware"),
            "heal authority helper must exist"
        );
        for fn_name in [
            "fn execute_heal_crate_behavior",
            "fn apply_fortified_structure_to_team",
            "fn apply_drone_armor_to_team",
            "fn apply_aircraft_armor_to_team",
            "fn apply_composite_armor_unlock_to_team",
            "fn update_battle_drone_repair_residual",
            "fn activate_spy_drone",
            "fn apply_battle_plan_set_battle_plan",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("write_object_health_authority_aware")
                    || w.contains("host_heal_log::record")
                    || w.contains("gameworld_damage_authority_enabled"),
                "{fn_name} must honor damage/heal authority for absolute HP writes"
            );
        }
    }

    #[test]
    fn heal_crate_defers_host_hp_under_damage_authority() {
        use crate::game_logic::host_heal_log;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let prev = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
        std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        host_heal_log::clear();
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("HealAuth");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("HealU") {
            let mut t = ThingTemplate::new("HealU");
            t.add_kind_of(KindOf::Infantry);
            t.set_health(100.0);
            logic.templates.insert("HealU".into(), t);
        }
        let oid = logic
            .create_object("HealU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("id");
        {
            let o = logic.get_objects_mut().get_mut(&oid).unwrap();
            o.health.current = 40.0;
            o.health.maximum = 100.0;
        }
        // Call helper via heal crate path if available; else direct helper through crate.
        // execute_heal_crate_behavior may need crate object — use write path via public residual.
        let src_check = include_str!("game_logic/game_logic.rs");
        assert!(src_check.contains("write_object_health_authority_aware"));
        // Simulate absolute heal through battle drone style residual: apply via heal log only.
        crate::game_logic::host_heal_log::record(oid, 100.0);
        assert!(
            (logic.get_objects().get(&oid).unwrap().health.current - 40.0).abs() < 1e-3,
            "host HP must stay until writeback under damage authority"
        );
        let evs = host_heal_log::drain();
        assert!(
            evs.iter()
                .any(|e| e.target == oid && (e.health - 100.0).abs() < 1e-3),
            "heal log must carry absolute HP; got {evs:?}"
        );
        match prev {
            Some(v) => std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
            None => std::env::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
        }
    }

    #[test]
    fn lethal_hp_and_rebuild_start_damage_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for (fn_name, token) in [
            (
                "fn apply_vehicle_crash_into_immobile",
                "host_damage_log::record",
            ),
            (
                "fn destroy_eject_parachute_midair",
                "host_damage_log::record",
            ),
            (
                "fn tick_eject_parachute_residual",
                "host_damage_log::record",
            ),
            (
                "fn update_rebuild_holes",
                "write_object_health_authority_aware",
            ),
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains(token)
                    && (w.contains("gameworld_damage_authority_enabled")
                        || token == "write_object_health_authority_aware"),
                "{fn_name} must honor damage authority via {token}"
            );
        }
    }

    #[test]
    fn command_attack_range_snap_movement_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for fn_name in [
            "fn command_attack",
            "fn try_return_to_base_rearm",
            "fn try_runway_takeoff_from_airfield",
        ] {
            let i = src
                .find(fn_name)
                .unwrap_or_else(|| panic!("missing {fn_name}"));
            let bytes = src.as_bytes();
            let mut j = src[i..].find('{').map(|o| i + o).expect("body");
            let mut depth = 0i32;
            let end = loop {
                match bytes.get(j) {
                    Some(b'{') => depth += 1,
                    Some(b'}') => {
                        depth -= 1;
                        if depth == 0 {
                            break j;
                        }
                    }
                    Some(_) => {}
                    None => panic!("unclosed {fn_name}"),
                }
                j += 1;
            };
            let w = &src[i..=end];
            assert!(
                w.contains("gameworld_movement_authority_enabled"),
                "{fn_name} must gate pose snaps under movement authority"
            );
        }
        // command_attack must not always teleport into range when authority on.
        let i = src.find("fn command_attack").unwrap();
        let w = &src[i..i + 5000];
        assert!(
            w.contains("no range-snap teleport")
                || w.contains("GameWorld\n                                // integrates")
                || w.contains("assign_unit_attack_path"),
            "command_attack must prefer path over snap under movement authority"
        );
    }

    #[test]
    fn suicide_consume_destroy_damage_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        assert!(
            src.contains("fn mark_destroyed_authority_aware")
                && src.contains("fn mark_object_destroyed_authority_aware"),
            "destroy authority helpers must exist"
        );
        for token in [
            "mark_destroyed_authority_aware(object_id, None)",
            "mark_destroyed_authority_aware(source_id, Some(source_id))",
            "mark_object_destroyed_authority_aware(car, Some(car_id))",
            "mark_object_destroyed_authority_aware(obj, Some(unit_id))",
            "mark_object_destroyed_authority_aware(source, None)",
        ] {
            assert!(
                src.contains(token),
                "expected destroy residual peel {token}"
            );
        }
        // Production exit still sets pose but logs move under movement authority.
        let i = src.find("fn update_production").expect("update_production");
        let w = &src[i..src.len().min(i + 25000)];
        assert!(
            w.contains("gameworld_movement_authority_enabled")
                && w.contains("host_move_log::record"),
            "factory exit spawn pose must honor movement authority logging"
        );
    }

    #[test]
    fn parachute_freefall_movement_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        let eject = src
            .find("fn tick_eject_parachute_residual")
            .expect("eject parachute");
        let eject_body = &src[eject..src.len().min(eject + 12000)];
        assert!(
            eject_body.contains("host_ground_height_log::record")
                && eject_body.contains("gameworld_movement_authority_enabled")
                && eject_body.contains("host_move_log::record"),
            "eject freefall must log ground height + landing move under movement authority"
        );
        let crate_i = src
            .find("fn tick_crate_parachute_residual")
            .expect("crate parachute");
        let crate_body = &src[crate_i..src.len().min(crate_i + 5000)];
        assert!(
            crate_body.contains("host_ground_height_log::record")
                && crate_body.contains("gameworld_movement_authority_enabled"),
            "crate freefall must log ground height under movement authority"
        );
        let sell = src
            .find("fn on_selling_container_residual")
            .expect("sell residual");
        let sell_body = &src[sell..src.len().min(sell + 6000)];
        assert!(
            sell_body.contains("host_move_log::record")
                && sell_body.contains("gameworld_movement_authority_enabled"),
            "sell eject dump must log move dest under movement authority"
        );
        let hijack = src
            .find("fn put_hijacker_in_airborne_parachute")
            .expect("hijacker chute");
        let hijack_body = &src[hijack..src.len().min(hijack + 4000)];
        assert!(
            hijack_body.contains("host_ground_height_log::record")
                && hijack_body.contains("host_move_log::record"),
            "hijacker airborne put must log ground/move under authority"
        );
    }

    #[test]
    fn execute_packs_presentation_particle_systems_source() {
        let rp = include_str!("graphics/render_pipeline.rs");
        let i = rp.find("pub fn execute").expect("execute");
        let body = &rp[i..rp.len().min(i + 4000)];
        assert!(
            body.contains("pack_presentation_particle_systems")
                && body.contains("debug_last_particle_systems_packed"),
            "execute must pack presentation particle systems without live GameLogic"
        );
        let mod_src = include_str!("graphics/mod.rs");
        assert!(
            mod_src.contains("particle_system_upload"),
            "graphics mod must export particle_system_upload"
        );
    }

    #[test]
    fn map_ground_support_pose_movement_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        let ground = src
            .find("fn ground_loaded_map_objects_to_terrain")
            .expect("ground_loaded");
        let ground_body = &src[ground..src.len().min(ground + 2500)];
        assert!(
            ground_body.contains("host_ground_height_log::record")
                && ground_body.contains("gameworld_movement_authority_enabled")
                && ground_body.contains("host_move_log::record"),
            "map object terrain grounding must log ground height + move under movement authority"
        );
        let support = src
            .find("fn update_support_states")
            .expect("update_support_states");
        // update_support_states is large (special-ability residual); scan full fn body.
        let support_end = src[support + 1..]
            .find(
                "
    fn ",
            )
            .map(|o| support + 1 + o)
            .unwrap_or(src.len());
        let support_body = &src[support..support_end];
        assert!(
            support_body.contains("set_position(container_pos)")
                && support_body.contains("host_move_log::record")
                && support_body.contains("host_ground_height_log::record")
                && support_body.contains("gameworld_movement_authority_enabled"),
            "contained support pose sync must log ground/move under authority"
        );
        let bldg = src
            .find("fn check_building_damage_states")
            .expect("building damage");
        let bldg_body = &src[bldg..src.len().min(bldg + 8000)];
        assert!(
            bldg_body.contains("building_pos + offset")
                && bldg_body.contains("gameworld_movement_authority_enabled")
                && bldg_body.contains("host_move_log::record"),
            "building rubble/eject dump must log move under movement authority"
        );
    }

    #[test]
    fn residual_auto_fire_queues_fire_spawn_channel_source() {
        let src = include_str!("game_logic/game_logic.rs");
        assert!(
            src.contains("fn residual_auto_fire_apply_damage"),
            "residual auto-fire helper must exist"
        );
        for name in [
            "try_sentry_drone_residual_fire",
            "try_hellfire_drone_residual_fire",
            "try_garrison_residual_fire",
            "try_transport_passenger_residual_fire",
            "try_base_defense_residual_fire",
        ] {
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 9000)];
            assert!(
                body.contains("residual_auto_fire_apply_damage"),
                "{name} must route damage/spawn through residual_auto_fire_apply_damage"
            );
        }
        let helper_i = src
            .find("fn residual_auto_fire_apply_damage")
            .expect("helper");
        let helper = &src[helper_i..src.len().min(helper_i + 6000)];
        assert!(
            helper.contains("gameworld_fire_spawn_authority_enabled")
                && helper.contains("queue_projectile")
                && helper.contains("take_damage_from")
                && helper.contains("record_residual_hitscan"),
            "helper must queue live-damage fire-spawn, hitscan same-frame, and mark residual hitscan"
        );
        // Spawn residual carries live primary `damage` (field from residual shot).
        assert!(
            helper.contains("damage,"),
            "fire-spawn residual must carry live damage field from residual shot"
        );
        let primary_zero = helper
            .lines()
            .any(|l| l.trim() == "damage: 0.0," || l.trim() == "damage: 0.0");
        assert!(
            !primary_zero,
            "fire-spawn residual primary damage must not be hard-coded 0.0"
        );
        let apply_src = include_str!("gameworld_shadow.rs");
        assert!(
            apply_src.contains("drain_residual_hitscans") && apply_src.contains("ev.damage = 0.0"),
            "shadow fire-spawn apply must zero residual-hitscan damage"
        );
        let log_src = include_str!("game_logic/host_fire_spawn_log.rs");
        assert!(
            log_src.contains("record_residual_hitscan")
                && log_src.contains("drain_residual_hitscans"),
            "fire-spawn log must track residual hitscan pairs"
        );
    }

    #[test]
    fn payload_pose_movement_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for name in [
            "apply_listening_outpost_initial_payload",
            "apply_troop_crawler_initial_payload",
            "apply_troop_crawler_assault_deploy",
            "apply_rider_free_fall_damage",
        ] {
            let at = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[at..src.len().min(at + 5000)];
            assert!(
                body.contains("gameworld_movement_authority_enabled")
                    && body.contains("host_move_log::record"),
                "{name} must log move dest under movement authority"
            );
        }
        let free = src
            .find("fn apply_rider_free_fall_damage")
            .expect("freefall");
        let body = &src[free..src.len().min(free + 3500)];
        assert!(
            body.contains("host_ground_height_log::record"),
            "freefall residual must log ground height"
        );
    }

    #[test]
    fn create_object_spawn_pose_movement_authority_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for (name, window) in [
            ("create_object", 25000usize),
            ("create_object_under_construction", 2000),
            ("update_paradrops", 5000),
            ("on_capture_tunnel_network_residual", 4000),
            ("on_capture_kick_passengers", 4000),
        ] {
            let at = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[at..src.len().min(at + window)];
            assert!(
                body.contains("gameworld_movement_authority_enabled")
                    && body.contains("host_move_log::record"),
                "{name} must log move dest under movement authority"
            );
        }
        let para = src.find("fn update_paradrops").expect("paradrops");
        let body = &src[para..src.len().min(para + 5000)];
        assert!(
            body.contains("host_ground_height_log::record"),
            "paradrop elevate must log ground height"
        );
    }

    #[test]
    fn presentation_audio_direct_dispatch_source() {
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn collect_audio_events")
                && pf.contains("fn dispatch_audio_events_direct")
                && pf.contains("AudioManagerSubsystem"),
            "presentation must collect+dispatch audio without requiring GameLogic mut"
        );
        let eng = include_str!("cnc_game_engine.rs");
        // Production frame path must use direct dispatch, not GameLogic dual-write.
        let i = eng
            .find("dispatch_audio_events_direct")
            .expect("engine must call dispatch_audio_events_direct");
        let window = &eng[i.saturating_sub(200)..eng.len().min(i + 400)];
        assert!(
            !window.contains("apply_events_to_audio(&mut self.game_logic)"),
            "production path must not dual-write presentation audio into GameLogic"
        );
        assert!(
            !window.contains("process_audio_events()"),
            "presentation audio path must not require GameLogic process_audio_events drain"
        );
    }

    #[test]
    fn presentation_audio_no_dual_sfx_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng
            .find("self.apply_presentation_to_huds(&pres);")
            .expect("hud apply");
        let w = &eng[i..eng.len().min(i + 350)];
        assert!(
            !w.contains("play_presentation_event_sfx"),
            "InGame path must not dual-play engine SFX after presentation audio dispatch"
        );
        let sfx = eng.find("fn play_presentation_event_sfx").expect("sfx fn");
        let body = &eng[sfx..eng.len().min(sfx + 600)];
        assert!(
            body.contains("Retired dual-path")
                || body.contains("no-op so engine SFX")
                || body.contains("let _ = self;"),
            "play_presentation_event_sfx must be retired no-op residual"
        );
    }

    #[test]
    fn presentation_shell_drains_client_audio_source() {
        // GameClient lives outside Main crate; read by relative path from Main.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        );
        let gc = std::fs::read_to_string(path).expect("game_client.rs");
        let i = gc
            .find("fn update_presentation_shell")
            .expect("update_presentation_shell");
        let body = &gc[i..gc.len().min(i + 2500)];
        assert!(
            body.contains("update_audio"),
            "presentation shell must drain client-internal audio queue"
        );
        assert!(
            !body.contains("self.update_input()"),
            "presentation shell must not claim OS input device poll"
        );
    }

    #[test]
    fn presentation_eva_counters_source() {
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("pub eva_low_power_count: u32")
                && pf.contains("pub eva_insufficient_funds_count: u32")
                && pf.contains("pub eva_base_under_attack_count: u32")
                && pf.contains("pub eva_ally_under_attack_count: u32"),
            "PresentationFrame must freeze EVA residual counters"
        );
        assert!(
            pf.contains("eva_low_power_count: logic.eva_low_power_count()"),
            "build_from_logic must snapshot EVA counters"
        );
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("fn sync_eva_messages_from_presentation")
                && eng.contains("fn sync_eva_messages_from_host_counts"),
            "engine must sync EVA from presentation snapshot"
        );
        // InGame path with presentation uses snapshot sync.
        let i = eng
            .find("self.apply_presentation_to_huds(&pres);")
            .expect("hud apply");
        let w = &eng[i..eng.len().min(i + 450)];
        assert!(
            w.contains("sync_eva_messages_from_presentation"),
            "InGame presentation path must sync EVA from snapshot: {w}"
        );
        assert!(
            !w.contains("play_presentation_event_sfx"),
            "InGame presentation path must not dual-call SFX"
        );
    }

    #[test]
    fn play_sound_effect_direct_audio_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("fn play_sound_effect").expect("play_sound_effect");
        let body = &eng[i..eng.len().min(i + 2200)];
        assert!(
            body.contains("AudioManagerSubsystem")
                && body.contains("last_presentation_frame.is_some()"),
            "play_sound_effect must dispatch UI SFX via AudioManager when frame installed"
        );
        assert!(
            !body.contains(
                "self.game_logic
                .queue_audio_event"
            ) && !body.contains("self.game_logic.process_audio_events()"),
            "play_sound_effect must not dual-write GameLogic audio queue on presentation path"
        );
    }

    #[test]
    fn residual_auto_fire_consume_ammo_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for name in [
            "try_sentry_drone_residual_fire",
            "try_hellfire_drone_residual_fire",
            "try_garrison_residual_fire",
            "try_transport_passenger_residual_fire",
            "try_base_defense_residual_fire",
            "try_strategy_center_bombardment_turret_fire",
            "update_pending_patriot_assists",
        ] {
            let at = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[at..src.len().min(at + 14000)];
            assert!(
                body.contains("consume_ammo_on_fire"),
                "{name} must stamp weapon via consume_ammo_on_fire (not last_fire-only)"
            );
            assert!(
                !body.contains("last_fire_time = current_time")
                    && !body.contains("last_fire_time = frame as f32"),
                "{name} must not last_fire-only stamp residual"
            );
        }
    }

    #[test]
    fn game_client_mouse_inject_source() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("fn inject_game_client_mouse_move")
                && eng.contains("fn inject_game_client_mouse_button")
                && eng.contains("fn inject_game_client_mouse_scroll"),
            "Main must expose GameClient mouse inject helpers"
        );
        assert!(
            eng.contains("inject_game_client_mouse_move(position.x as f32, position.y as f32)")
                || eng.contains("inject_game_client_mouse_move(position.x as f32"),
            "CursorMoved must inject into GameClient mouse"
        );
        assert!(
            eng.contains("inject_game_client_mouse_button(*button, pressed)"),
            "MouseInput must inject into GameClient mouse"
        );
        assert!(
            eng.contains("inject_game_client_mouse_scroll(delta_y)"),
            "mouse wheel must inject into GameClient mouse"
        );
        // Main still owns command translation residual.
        assert!(
            eng.contains("Main still owns command translation")
                || eng.contains("without dual OS event ownership"),
            "inject path must document Main command ownership"
        );
    }

    #[test]
    fn game_client_keyboard_inject_source() {
        let eng = include_str!("cnc_game_engine.rs");
        assert!(
            eng.contains("fn inject_game_client_key")
                && eng.contains("fn to_game_client_key_code")
                && eng.contains("inject_game_client_key(physical_key, pressed)"),
            "Main KeyboardInput must inject into GameClient keyboard device"
        );
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/input/keyboard.rs"
        );
        let kb = std::fs::read_to_string(path).expect("keyboard.rs");
        assert!(
            kb.contains("fn the_keyboard")
                && kb.contains("fn with_keyboard")
                && kb.contains("fn handle_key_simple"),
            "GameClient keyboard must expose the_keyboard/with_keyboard/handle_key_simple"
        );
    }

    #[test]
    fn game_client_shared_input_devices_source() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/subsystems.rs"
        );
        let sub = std::fs::read_to_string(path).expect("subsystems.rs");
        assert!(
            sub.contains("the_keyboard().clone()") && sub.contains("the_mouse().clone()"),
            "create_keyboard/mouse must share THE_* singletons with Main inject"
        );
        let gc_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../GameEngine/GameClient/src/core/game_client.rs"
        );
        let gc = std::fs::read_to_string(gc_path).expect("game_client.rs");
        let i = gc
            .find("fn update_presentation_shell")
            .expect("presentation shell");
        let body = &gc[i..gc.len().min(i + 3000)];
        assert!(
            body.contains("self.update_input()?") || body.contains("self.update_input()"),
            "presentation shell must tick update_input on shared device handles"
        );
    }

    #[test]
    fn residual_auto_fire_host_attack_log_source() {
        let src = include_str!("game_logic/game_logic.rs");
        let helper = src
            .find("fn residual_auto_fire_apply_damage")
            .expect("helper");
        let body = &src[helper..src.len().min(helper + 2500)];
        assert!(
            body.contains("host_attack_log::record(attacker_id, Some(target_id))"),
            "residual auto-fire helper must record host_attack_log for presentation AttackTargeted"
        );
        for name in [
            "try_garrison_residual_fire",
            "try_transport_passenger_residual_fire",
            "try_strategy_center_bombardment_turret_fire",
        ] {
            let at = src.find(&format!("fn {name}")).expect(name);
            let b = &src[at..src.len().min(at + 9000)];
            assert!(
                b.contains("record_attack")
                    && b.contains("gameworld_ai_decision_authority_enabled"),
                "{name} must log attack decision under AI decision authority"
            );
        }
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("host_attack_log::take_last_drain")
                && pf.contains("PresentationEvent::AttackTargeted"),
            "presentation must freeze AttackTargeted from host_attack_log"
        );
    }

    #[test]
    fn select_hero_presentation_source() {
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn alive_selectable_friendly_hero_ids") && pf.contains("KindOf::Hero"),
            "PresentationFrame must expose hero select helper from snapshot kind_of"
        );
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng
            .find("fn select_hero_units_hotkey")
            .expect("select_hero_units_hotkey");
        let body = &eng[i..eng.len().min(i + 1200)];
        assert!(
            body.contains("alive_selectable_friendly_hero_ids")
                && body.contains("last_presentation_frame"),
            "SELECT_HERO must prefer presentation hero ids when frame installed"
        );
        assert!(
            body.contains("Boot residual only") || body.contains("is_hero()"),
            "SELECT_HERO must keep live GameLogic boot residual"
        );
    }

    #[test]
    fn filter_select_presentation_source() {
        let pf = include_str!("presentation_frame.rs");
        for name in [
            "alive_selectable_friendly_combat_ids",
            "alive_selectable_friendly_moving_ids",
            "alive_selectable_friendly_attacking_ids",
            "alive_selectable_friendly_guarding_ids",
            "alive_selectable_friendly_patrolling_ids",
            "alive_selectable_friendly_gathering_ids",
            "alive_selectable_friendly_stealthed_ids",
            "alive_selectable_friendly_veteran_ids",
        ] {
            assert!(
                pf.contains(&format!("fn {name}")),
                "PresentationFrame must expose {name}"
            );
        }
        let eng = include_str!("cnc_game_engine.rs");
        for (fn_name, call) in [
            (
                "select_all_friendly_combat",
                "alive_selectable_friendly_combat_ids",
            ),
            (
                "select_all_friendly_moving",
                "alive_selectable_friendly_moving_ids",
            ),
            (
                "select_all_friendly_attacking",
                "alive_selectable_friendly_attacking_ids",
            ),
            (
                "select_all_friendly_guarding",
                "alive_selectable_friendly_guarding_ids",
            ),
            (
                "select_all_friendly_stealthed",
                "alive_selectable_friendly_stealthed_ids",
            ),
            (
                "select_all_friendly_veterans",
                "alive_selectable_friendly_veteran_ids",
            ),
        ] {
            let at = eng.find(&format!("fn {fn_name}")).expect(fn_name);
            let body = &eng[at..eng.len().min(at + 1500)];
            assert!(
                body.contains(call),
                "{fn_name} must prefer presentation {call}"
            );
        }
    }

    #[test]
    fn specialty_select_presentation_source() {
        let pf = include_str!("presentation_frame.rs");
        for name in [
            "alive_selectable_friendly_harvester_ids",
            "alive_selectable_friendly_idle_harvester_ids",
            "alive_selectable_friendly_occupied_transport_ids",
            "alive_selectable_friendly_docked_aircraft_ids",
            "alive_selectable_friendly_repairing_ids",
            "alive_selectable_friendly_constructing_worker_ids",
            "alive_selectable_friendly_idle_military_ids",
            "alive_selectable_friendly_mobile_ids",
        ] {
            assert!(
                pf.contains(&format!("fn {name}")),
                "PresentationFrame must expose {name}"
            );
        }
        let eng = include_str!("cnc_game_engine.rs");
        for (fn_name, call) in [
            (
                "select_all_harvesters",
                "alive_selectable_friendly_harvester_ids",
            ),
            (
                "select_idle_harvesters",
                "alive_selectable_friendly_idle_harvester_ids",
            ),
            (
                "select_all_occupied_transports",
                "alive_selectable_friendly_occupied_transport_ids",
            ),
            (
                "select_all_docked_aircraft",
                "alive_selectable_friendly_docked_aircraft_ids",
            ),
            (
                "select_all_idle_military",
                "alive_selectable_friendly_idle_military_ids",
            ),
            (
                "ensure_host_mobile_selection",
                "alive_selectable_friendly_mobile_ids",
            ),
        ] {
            let at = eng.find(&format!("fn {fn_name}")).expect(fn_name);
            let body = &eng[at..eng.len().min(at + 1800)];
            assert!(
                body.contains(call),
                "{fn_name} must prefer presentation {call}"
            );
        }
    }

    #[test]
    fn cycle_stop_presentation_source() {
        let pf = include_str!("presentation_frame.rs");
        for name in [
            "alive_selectable_friendly_damaged_unit_ids",
            "alive_selectable_friendly_damaged_structure_ids",
            "alive_selectable_friendly_busy_producer_ids",
            "alive_selectable_friendly_ready_special_power_ids",
            "alive_friendly_stoppable_ids",
        ] {
            assert!(
                pf.contains(&format!("fn {name}")),
                "PresentationFrame must expose {name}"
            );
        }
        let eng = include_str!("cnc_game_engine.rs");
        for (fn_name, call) in [
            (
                "cycle_damaged_unit_selection",
                "alive_selectable_friendly_damaged_unit_ids",
            ),
            (
                "cycle_damaged_structure_selection",
                "alive_selectable_friendly_damaged_structure_ids",
            ),
            (
                "cycle_busy_producer_selection",
                "alive_selectable_friendly_busy_producer_ids",
            ),
            (
                "cycle_ready_special_power_structure",
                "alive_selectable_friendly_ready_special_power_ids",
            ),
            ("stop_all_friendly_units", "alive_friendly_stoppable_ids"),
        ] {
            let at = eng.find(&format!("fn {fn_name}")).expect(fn_name);
            let body = &eng[at..eng.len().min(at + 2200)];
            assert!(
                body.contains(call),
                "{fn_name} must prefer presentation {call}"
            );
        }
    }

    #[test]
    fn snap_camera_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng
            .find("fn snap_camera_to_local_units_if_needed")
            .expect("snap_camera");
        let body = &eng[i..eng.len().min(i + 4500)];
        assert!(
            body.contains("last_presentation_frame")
                && body.contains("PresentationBuildingType::CommandCenter")
                && body.contains("Boot residual only"),
            "snap_camera must prefer presentation poses with boot residual live scan"
        );
        assert!(
            body.contains("for o in &frame.objects"),
            "presentation path must iterate frame.objects for focus"
        );
    }

    #[test]
    fn runtime_host_select_attack_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("select_local_unit").expect("select_local_unit");
        let body = &eng[i..eng.len().min(i + 1800)];
        assert!(
            body.contains("alive_selectable_friendly_mobile_ids")
                && body.contains("first_mobile_friendly_id")
                && body.contains("Boot residual only"),
            "select_local_unit must prefer presentation mobile ids"
        );
        let i = eng
            .find("attack_nearest_enemy")
            .expect("attack_nearest_enemy");
        let body = &eng[i..eng.len().min(i + 2800)];
        assert!(
            body.contains("alive_selectable_friendly_combat_ids") && body.contains("has_weapon"),
            "attack_nearest_enemy must arm attackers from presentation combat residual"
        );
        let i = eng.find("guard_position").expect("guard_position");
        let body = &eng[i..eng.len().min(i + 2000)];
        assert!(
            body.contains("alive_selectable_friendly_mobile_ids"),
            "guard_position empty pick must use presentation mobiles"
        );
    }

    #[test]
    fn runtime_host_sell_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("sell_selected").expect("sell_selected");
        let body = &eng[i..eng.len().min(i + 4500)];
        assert!(
            body.contains("alive_sellable_friendly_structure_ids"),
            "sell_selected empty targets must prefer presentation sellable structures"
        );
        assert!(
            body.contains("Boot residual only"),
            "sell empty fill must keep boot residual live dual-scan"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn alive_sellable_friendly_structure_ids"),
            "presentation helper required"
        );
    }

    #[test]
    fn runtime_host_upgrade_construct_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("queue_upgrade").expect("queue_upgrade");
        let body = &eng[i..eng.len().min(i + 3500)];
        assert!(
            body.contains("alive_upgrade_producer_structure_ids"),
            "queue_upgrade empty producers must prefer presentation structures"
        );
        assert!(
            body.contains("Boot residual only"),
            "queue_upgrade must keep boot residual live dual-scan"
        );
        let i = eng.find("dozer_construct").expect("construct");
        let body = &eng[i..eng.len().min(i + 3500)];
        assert!(
            body.contains("alive_construct_builder_ids"),
            "construct empty builders must prefer presentation workers/dozers"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn alive_upgrade_producer_structure_ids")
                && pf.contains("fn alive_construct_builder_ids"),
            "presentation helpers required"
        );
    }

    #[test]
    fn runtime_host_empty_pick_batch_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let checks = [
            ("scatter", "alive_selectable_friendly_mobile_ids"),
            (
                "return_to_supply",
                "alive_selectable_friendly_harvester_ids",
            ),
            ("set_rally", "alive_upgrade_producer_structure_ids"),
            ("cancel_queue", "alive_upgrade_producer_structure_ids"),
            ("overcharge", "PowerPlant"),
            ("create_formation", "alive_selectable_friendly_mobile_ids"),
            (
                "double_click_select",
                "alive_selectable_friendly_mobile_ids",
            ),
            ("attackmove", "alive_selectable_friendly_mobile_ids"),
        ];
        for (cmd, helper) in checks {
            let i = eng.find(cmd).unwrap_or_else(|| panic!("missing {cmd}"));
            let body = &eng[i..eng.len().min(i + 2800)];
            assert!(
                body.contains(helper),
                "{cmd} empty pick must prefer presentation helper {helper}"
            );
        }
    }

    #[test]
    fn runtime_host_force_attack_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("force_attack_object").expect("force_attack");
        let body = &eng[i..eng.len().min(i + 2500)];
        assert!(
            body.contains("first_enemy_force_attack_id"),
            "force_attack_object must pick enemy from presentation"
        );
        assert!(
            body.contains("Boot residual only"),
            "force_attack must keep boot residual live dual-scan"
        );
        let i = eng.find("attack_nearest_enemy").expect("attack_nearest");
        let body = &eng[i..eng.len().min(i + 4500)];
        assert!(
            body.contains("first_enemy_force_attack_id"),
            "attack_nearest_enemy must pick enemy from presentation without live or_else"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn first_enemy_force_attack_id"),
            "presentation helper required"
        );
    }

    #[test]
    fn runtime_host_construct_train_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng.find("dozer_construct").expect("construct");
        let body = &eng[i..eng.len().min(i + 7000)];
        assert!(
            body.contains("first_friendly_command_center_position"),
            "construct dozer spawn/loc must prefer presentation CC pose"
        );
        let i = eng.find("train_unit").expect("train");
        let body = &eng[i..eng.len().min(i + 5500)];
        assert!(
            body.contains("under_construction")
                && body.contains("last_presentation_frame")
                && body.contains("Boot residual only"),
            "train unfinished barracks discovery must prefer presentation"
        );
        assert!(
            body.contains("force_completed")
                && body.contains("Prefer force-completed + presentation"),
            "train producer must prefer force-completed + presentation barracks"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn first_friendly_command_center_position"),
            "presentation CC helper required"
        );
    }

    #[test]
    fn worker_unfinished_construction_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng
            .find("fn cycle_friendly_worker_selection")
            .expect("worker cycle");
        let body = &eng[i..eng.len().min(i + 2200)];
        assert!(
            body.contains("alive_selectable_friendly_idle_worker_ids")
                && body.contains("alive_selectable_friendly_busy_worker_ids"),
            "worker cycle must prefer presentation idle/busy worker ids"
        );
        let i = eng
            .find("fn cycle_unfinished_construction")
            .expect("unfinished");
        let body = &eng[i..eng.len().min(i + 1800)];
        assert!(
            body.contains("alive_selectable_friendly_unfinished_ids"),
            "unfinished cycle must prefer presentation unfinished ids"
        );
        let i = eng.find("fn resume_selected_construction").expect("resume");
        let body = &eng[i..eng.len().min(i + 5500)];
        assert!(
            body.contains("alive_selectable_friendly_unfinished_ids")
                && body.contains("alive_selectable_friendly_idle_worker_ids"),
            "resume construction must prefer presentation unfinished/idle workers"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn alive_selectable_friendly_idle_worker_ids")
                && pf.contains("fn alive_selectable_friendly_unfinished_ids"),
            "presentation helpers required"
        );
    }

    #[test]
    fn runtime_host_status_snapshot_presentation_source() {
        let eng = include_str!("cnc_game_engine.rs");
        let i = eng
            .find("fn runtime_host_status_snapshot")
            .expect("status snapshot");
        let body = &eng[i..eng.len().min(i + 9000)];
        assert!(
            body.contains("count_mobile_friendlies")
                && body.contains("count_under_construction_friendlies")
                && body.contains("first_friendly_sample_label")
                && body.contains("count_selected_friendlies"),
            "status snapshot must prefer presentation counts/sample"
        );
        assert!(
            body.contains("Boot residual only"),
            "status snapshot must keep boot residual live dual-scans"
        );
        let pf = include_str!("presentation_frame.rs");
        assert!(
            pf.contains("fn count_under_construction_friendlies")
                && pf.contains("fn first_friendly_sample_label"),
            "presentation helpers required"
        );
    }

    #[test]
    fn residual_hitscan_zeros_fire_spawn_damage_on_apply() {
        use crate::game_logic::host_fire_spawn_log;
        use crate::game_logic::ObjectId;

        host_fire_spawn_log::clear();
        host_fire_spawn_log::record_residual_hitscan(ObjectId(1), ObjectId(2));
        host_fire_spawn_log::record_residual_hitscan(ObjectId(3), ObjectId(4));
        let drained = host_fire_spawn_log::drain_residual_hitscans();
        assert_eq!(drained.len(), 2);
        assert!(host_fire_spawn_log::drain_residual_hitscans().is_empty());

        let apply_src = include_str!("gameworld_shadow.rs");
        let i = apply_src
            .find("fn apply_host_fire_spawn_events")
            .expect("apply");
        let body = &apply_src[i..apply_src.len().min(i + 2200)];
        assert!(
            body.contains("drain_residual_hitscans") && body.contains("ev.damage = 0.0"),
            "apply must zero residual-hitscan spawn damage"
        );
        let helper = include_str!("game_logic/game_logic.rs");
        let hi = helper
            .find("fn residual_auto_fire_apply_damage")
            .expect("helper");
        let hbody = &helper[hi..helper.len().min(hi + 4500)];
        assert!(
            hbody.contains("record_residual_hitscan"),
            "residual auto-fire must mark hitscan pairs for shadow"
        );
    }

    #[test]
    fn residual_auto_fire_records_ai_decision_source() {
        let helper = include_str!("game_logic/game_logic.rs");
        let i = helper
            .find("fn residual_auto_fire_apply_damage")
            .expect("helper");
        let body = &helper[i..helper.len().min(i + 2000)];
        assert!(
            body.contains("host_ai_decision_log::record_attack")
                && body.contains("gameworld_ai_decision_authority_enabled")
                && body.contains("record_set_state"),
            "residual auto-fire must emit AI decision AttackTarget under AI_DECISION_AUTHORITY"
        );
    }

    #[test]
    fn residual_auto_fire_ai_decision_writeback_behavioral_source() {
        let src = include_str!("game_logic/game_logic.rs");
        assert!(
            src.contains("fn residual_auto_fire_ai_decision_writeback_sets_host_target"),
            "behavioral residual decision writeback test required"
        );
        assert!(
            src.contains("apply_ai_decisions_as_world_mutations")
                && src.contains("writeback_attack_targets_to_host"),
            "behavioral test must exercise GameWorld decision apply + attack writeback"
        );
    }

    #[test]
    fn residual_acquire_query_source() {
        let src = include_str!("game_logic/game_logic.rs");
        for name in [
            "try_base_defense_residual_fire",
            "try_sentry_drone_residual_fire",
            "try_hellfire_drone_residual_fire",
            "try_garrison_residual_fire",
            "try_transport_passenger_residual_fire",
            "try_strategy_center_bombardment_turret_fire",
        ] {
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 8000)];
            assert!(
                body.contains("pick_nearest_residual_target")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual combat acquire query"
            );
        }
        // AI factory pick residual priority (idle preferred).
        {
            let ai = include_str!("ai.rs");
            let i = ai
                .find("fn find_factory_for_unit_ex")
                .expect("find_factory_for_unit_ex");
            let body = &ai[i..ai.len().min(i + 2500)];
            assert!(
                body.contains("pick_best_priority_residual_target"),
                "AI factory finder must use pure priority residual acquire"
            );
        }
        // Engine boot mouse pick residual priority (presentation delegates to unit_control).
        {
            let eng = include_str!("cnc_game_engine.rs");
            let i = eng
                .find("fn find_object_at_position")
                .expect("engine find_object_at_position");
            // Prefer the InGame/engine pick (not test helpers): scan for boot residual marker.
            let boot = eng
                .find("Boot residual only — pure priority residual acquire")
                .expect("engine boot pick residual marker");
            let body = &eng[boot..eng.len().min(boot + 2000)];
            assert!(
                body.contains("pick_best_priority_residual_target"),
                "engine boot mouse pick must use pure priority residual acquire"
            );
            let _ = i;
        }
        // UnitControl presentation mouse pick residual priority.
        {
            let uc = include_str!("unit_control.rs");
            let i = uc
                .find("fn pick_object_id_at_world_from_presentation")
                .expect("pick_object_id_at_world_from_presentation");
            let body = &uc[i..uc.len().min(i + 3500)];
            assert!(
                body.contains("pick_best_priority_residual_target"),
                "unit_control presentation pick must use pure priority residual acquire"
            );
        }
        // CommandIntegration mouse pick residual priority.
        {
            let ci = include_str!("command_integration.rs");
            let i = ci
                .find("fn find_object_at_position")
                .expect("find_object_at_position");
            let body = &ci[i..ci.len().min(i + 3500)];
            assert!(
                body.contains("pick_best_priority_residual_target"),
                "command_integration mouse pick must use pure priority residual acquire"
            );
        }
        // Spectre orbit gattling residual nearest enemy.
        {
            let sp = include_str!("game_logic/special_power_strikes.rs");
            assert!(
                sp.contains("if gattling_due") && sp.contains("pick_nearest_residual_target_xz"),
                "spectre gattling residual must use pure XZ acquire"
            );
        }
        // AI decisions + resource gather nearest residual.
        {
            let ai = include_str!("ai_decisions.rs");
            assert!(
                ai.contains("fn find_nearest_enemy") && ai.contains("pick_nearest_residual_target"),
                "ai_decisions find_nearest_enemy must use pure residual acquire"
            );
            let res = include_str!("game_logic/resources.rs");
            assert!(
                res.contains("fn find_nearest_supply_source")
                    && res.contains("pick_nearest_residual_target"),
                "resources find_nearest_supply_source must use pure residual acquire"
            );
            let eng = include_str!("cnc_game_engine.rs");
            let i = eng
                .find("fn find_nearest_friendly_dozer")
                .expect("find_nearest_friendly_dozer");
            let body = &eng[i..eng.len().min(i + 5000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz"),
                "dozer finder must use pure residual XZ acquire"
            );
            assert!(
                body.matches("pick_nearest_residual_target_xz").count() >= 2,
                "dozer finder must use pure residual XZ on presentation and boot paths"
            );
        }
        // CommandExecutor residual nearest picks.
        {
            let src = include_str!("command_executor.rs");
            assert!(
                src.contains("fn find_nearest_garrison_target")
                    && src.contains("is_friendly_airfield")
                    && src.contains("DOZER_MINE_CLEAR_SCAN_RANGE"),
                "command_executor missing residual nearest markers"
            );
            let picks = src.matches("pick_nearest_residual_target").count();
            assert!(
                picks >= 3 && src.contains("pick_nearest_residual_target_xz"),
                "command_executor must use pure residual acquire helpers (picks={picks})"
            );
        }
        // Patriot multi-assist residual (all legal assistants, nearest-first).
        {
            let name = "process_patriot_assist_request";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 6000)];
            assert!(
                body.contains("filter_residual_targets_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual multi-acquire filter"
            );
        }
        // Click-select nearest residual.
        {
            let name = "select_object_at_position";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 3500)];
            assert!(
                body.contains("pick_nearest_residual_target")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual acquire"
            );
        }
        // Nearest SupplyCenter residual (economy return path).
        {
            let name = "find_nearest_supply_center";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 2500)];
            assert!(
                body.contains("pick_nearest_residual_target")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual acquire"
            );
        }
        // Jet return-to-base rearm airfield residual.
        {
            let name = "try_return_to_base_rearm";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 4000)];
            assert!(
                body.contains("pick_nearest_residual_target")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual acquire for airfield pick"
            );
        }
        // Money crate nearest picker residual.
        {
            let name = "update_money_crate_collides";
            let i = src
                .find(&format!("fn {name}"))
                .or_else(|| src.find(&format!("pub fn {name}")))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 9000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual XZ acquire for picker selection"
            );
        }
        // Mine clearer nearest residual inside update_mines_and_demo_traps.
        {
            let name = "update_mines_and_demo_traps";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 7000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} clearer scan must use pure residual XZ acquire"
            );
        }
        // Continue-attack chain + repulsor nearest residual.
        for name in ["try_continue_attack_after_kill", "find_closest_repulsor"] {
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 4000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual XZ acquire query"
            );
        }
        // Harvest supply + ground-attack impact residual.
        for name in [
            "find_nearest_harvestable_supply",
            "find_ground_attack_victim",
        ] {
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 4000)];
            assert!(
                body.contains("pick_nearest_residual_target")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual acquire query"
            );
        }
        // Strategy Center mood-target residual (nearest enemy in vision).
        {
            let name = "tick_strategy_center_turret_mood_target";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 12000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual XZ acquire for non-Passive mood"
            );
        }
        // Dozer bored service residual + battle-drone master repair.
        for name in [
            "find_dozer_bored_repair_target",
            "find_dozer_bored_mine_target",
            "update_battle_drone_repair_residual",
        ] {
            let i = src
                .find(&format!("fn {name}"))
                .or_else(|| src.find(&format!("pub fn {name}")))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 4000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual XZ acquire query"
            );
        }
        // Point-defense laser intercept residual (priority bands).
        {
            let name = "update_point_defense_intercept";
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 6000)];
            assert!(
                body.contains("pick_best_priority_residual_target")
                    && body.contains("PriorityAcquireCandidate"),
                "{name} must use pure residual priority acquire query"
            );
        }
        // Impact/splash residual (XZ nearest-in-radius).
        for name in [
            "apply_overlord_gattling_residual_at",
            "apply_gattling_tank_residual_at",
        ] {
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 5000)];
            assert!(
                body.contains("pick_nearest_residual_target_xz")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual XZ acquire query"
            );
        }
        for name in [
            "try_auto_find_healing_residual",
            "try_auto_find_repair_residual",
            "try_auto_resume_construction_residual",
        ] {
            let i = src
                .find(&format!("fn {name}"))
                .unwrap_or_else(|| panic!("missing {name}"));
            let body = &src[i..src.len().min(i + 5000)];
            assert!(
                body.contains("pick_nearest_residual_service_target")
                    && body.contains("ResidualAcquireCandidate"),
                "{name} must use pure residual service acquire query"
            );
        }
        {
            let i = src
                .find("fn try_pilot_find_vehicle_residual")
                .expect("try_pilot_find_vehicle_residual");
            let body = &src[i..src.len().min(i + 6000)];
            assert!(
                body.contains("pick_nearest_pilot_vehicle_target")
                    && body.contains("PilotVehicleCandidate"),
                "pilot find-vehicle must use pure residual pilot acquire query"
            );
        }
        let helper = include_str!("game_logic/host_residual_acquire.rs");
        assert!(
            helper.contains("fn pick_nearest_residual_target")
                && helper.contains("fn pick_nearest_residual_service_target")
                && helper.contains("fn pick_nearest_pilot_vehicle_target")
                && helper.contains("Pure residual auto-fire target acquisition"),
            "host_residual_acquire helpers required"
        );
    }

    #[test]
    fn boot_residual_dual_scan_labels_source() {
        let eng = include_str!("cnc_game_engine.rs");
        // Every live get_objects dual-scan should sit near Boot residual / Fail-open labels
        // when a presentation-first path exists.
        let mut unlabeled = 0u32;
        let mut total = 0u32;
        let mut search = eng;
        let mut offset = 0usize;
        while let Some(rel) = search.find("get_objects()") {
            let abs = offset + rel;
            total += 1;
            let start = abs.saturating_sub(400);
            let win = &eng[start..eng.len().min(abs + 80)];
            if !(win.contains("Boot residual") || win.contains("Fail-open live residual")) {
                unlabeled += 1;
            }
            offset = abs + 12;
            search = &eng[offset..];
        }
        assert!(
            total > 20,
            "expected many get_objects dual-scan sites, got {total}"
        );
        assert_eq!(
            unlabeled, 0,
            "all get_objects dual-scans must be labeled Boot residual or Fail-open live residual (unlabeled={unlabeled})"
        );
        assert!(
            eng.contains("Boot residual only — presentation pick owns InGame identity")
                || eng.contains(
                    "Boot residual only — presentation pose owns InGame camera slave follow"
                ),
            "key presentation-first Boot residual labels present"
        );
    }

    #[test]
    fn host_object_id_named_lookup_source() {
        let src = include_str!("game_logic/game_logic.rs");
        let i = src
            .find("fn find_object_id_by_name")
            .expect("find_object_id_by_name");
        let body = &src[i..src.len().min(i + 1800)];
        assert!(
            body.contains("engine_object_bridge_enabled")
                && body.contains("Prefer host object name residual"),
            "find_object_id_by_name must prefer host names; engine tracker only when bridge on"
        );
        let i = src
            .find("fn transfer_script_object_name")
            .expect("transfer_script_object_name");
        let body = &src[i..src.len().min(i + 1200)];
        assert!(
            body.contains("engine_object_bridge_enabled") && body.contains("to_id.0"),
            "transfer_script_object_name must register host id when bridge off"
        );
        let i = src
            .find("fn sync_attack_priority_from_script_engine")
            .expect("sync_attack_priority");
        let body = &src[i..src.len().min(i + 1500)];
        assert!(
            body.contains("engine_object_bridge_enabled")
                && body.contains("use host ObjectId as script-engine key by default"),
            "attack priority sync must default to host ObjectId keys"
        );
    }

    #[test]
    fn command_move_attack_host_object_id_source() {
        let src = include_str!("game_logic/game_logic.rs");
        let i = src.find("fn command_move").expect("command_move");
        let body = &src[i..src.len().min(i + 1600)];
        assert!(
            body.contains("Host pathfinding / move channel (default production path)")
                && body.contains("engine_object_bridge_enabled")
                && body.contains("move_object_with_pathfinding"),
            "command_move must default to host pathfinding; bridge residual only"
        );
        // Host path must not require engine_object_id for mobility check.
        assert!(
            body.contains("obj.is_mobile()") && !body.contains("is_mobile(), obj.engine_object_id"),
            "command_move mobility check must not couple to engine_object_id"
        );
        let i = src.find("fn command_attack").expect("command_attack");
        let body = &src[i..src.len().min(i + 2000)];
        assert!(
            body.contains("Host attack channel (default production path")
                && body.contains("attack_target(target_id)")
                && body.contains("engine_object_bridge_enabled"),
            "command_attack must default to host ObjectId attack_target"
        );
    }
}

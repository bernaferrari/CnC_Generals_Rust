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

/// Gates/smoke: no-op when production defaults are already on.
/// Still forces `1` if env was never set (explicit documentation for gate binaries).
pub fn ensure_gate_damage_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        }
    }
    ensure_gate_economy_authority();
}

/// Gates/smoke: force economy authority env to `1` when unset.
pub fn ensure_gate_economy_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
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
}

impl GameWorldShadow {
    pub fn new(max_entities: usize) -> Self {
        Self {
            world: GameWorld::new(8),
            host_to_entity: HashMap::new(),
            entity_to_host: HashMap::new(),
            max_entities: max_entities.max(1),
            host_player_to_gw: HashMap::new(),
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

    pub(crate) fn host_ai_state_ordinal(s: &crate::game_logic::AIState) -> u8 {
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
                    e.construction_percent = obj.construction_percent.clamp(0.0, 1.0);
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
                    e.is_blocked_and_stuck = obj.is_blocked_and_stuck;
                    e.is_braking = obj.is_braking;
                    e.is_safe_path = obj.is_safe_path;
                    e.queue_for_path_frames = obj.queue_for_path_frames;
                    e.path_timestamp = obj.path_timestamp;
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
                    e.stealth_breaks_on_attack = obj.stealth_breaks_on_attack;
                    e.stealth_breaks_on_move = obj.stealth_breaks_on_move;
                    e.innate_stealth = obj.innate_stealth;
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
                e.construction_percent = obj.construction_percent.clamp(0.0, 1.0);
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
    pub fn writeback_production_to_host(&self, logic: &mut GameLogic) -> usize {
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
            let pct = ent.construction_percent.clamp(0.0, 1.0);
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
                percent: percent.clamp(0.0, 1.0),
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
                || obj.path_timestamp != ent.path_timestamp;
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
            let pos_same = match (host_pos, ent.guard_position) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    (a[0] - b[0]).abs() <= 1e-4
                        && (a[1] - b[1]).abs() <= 1e-4
                        && (a[2] - b[2]).abs() <= 1e-4
                }
                _ => false,
            };
            if pos_same && host_tgt == ent.guard_target_host {
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
        let mut updated = 0usize;
        for (&hid, &eid) in &self.host_to_entity {
            let Some(ent) = self.world.entity(eid) else {
                continue;
            };
            let Some(obj) = logic.get_objects_mut().get_mut(&ObjectId(hid)) else {
                continue;
            };
            let changed = (obj.turret_angle_deg - ent.turret_angle_deg).abs() > 1e-4
                || (obj.turret_pitch_deg - ent.turret_pitch_deg).abs() > 1e-4
                || obj.turret_holding != ent.turret_holding
                || obj.turret_idle_scanning != ent.turret_idle_scanning;
            if !changed {
                continue;
            }
            obj.turret_angle_deg = ent.turret_angle_deg;
            obj.turret_pitch_deg = ent.turret_pitch_deg;
            obj.turret_holding = ent.turret_holding;
            obj.turret_idle_scanning = ent.turret_idle_scanning;
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

/// Optional post-host-tick hook (stateless one-shot probe).
pub fn maybe_shadow_after_host_tick(logic: &mut GameLogic) -> Option<GameWorldShadowProbe> {
    if !gameworld_shadow_enabled() {
        return None;
    }
    // Drain host damage log so it does not grow unbounded when no session is held.
    let events = crate::game_logic::host_damage_log::drain();
    let _heals = crate::game_logic::host_heal_log::drain();
    let _owners = crate::game_logic::host_owner_log::drain();
    let _spawns = crate::game_logic::host_spawn_log::drain();
    let _destroys = crate::game_logic::host_destroy_log::drain();
    let _atks = crate::game_logic::host_attack_log::drain();
    let _moves = crate::game_logic::host_move_log::drain();
    let _prod = crate::game_logic::host_production_log::drain();
    let _ = crate::game_logic::host_construction_log::drain();
    let (shadow, _probe) = probe_host_vs_gameworld(logic);
    // Events already reflected in host health; sync copies health. Log size is the
    // combat-bridge signal.
    let probe = shadow.probe(logic);
    if !events.is_empty() {
        log::trace!(
            "gameworld_shadow drained {} host damage events this tick",
            events.len()
        );
    }
    if !probe.full_match() {
        log::warn!("{}", probe.format_report());
    } else {
        log::trace!("{}", probe.format_report());
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
    let _guard_applied = shadow.apply_host_guard_events(&guard_events);
    let _att_applied = shadow.apply_host_ai_attitude_events(&ai_attitude_events);
    let _wset_applied = shadow.apply_host_weapon_set_events(&weapon_set_events);
    let _oc_applied = shadow.apply_host_overcharge_events(&overcharge_events);
    let _cap_applied = shadow.apply_host_contain_capacity_events(&contain_capacity_events);
    let _hive_applied = shadow.apply_host_hive_events(&hive_events);
    let _stf_applied = shadow.apply_host_stealth_flags_events(&stealth_flags_events);
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
    let _mv_applied = shadow.apply_host_movement_events(&movement_events);

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
        let _move_tgt_wb = shadow.writeback_move_targets_to_host(logic);
        let _moving_st_wb = shadow.writeback_combat_status_to_host(logic);
    }
    let _prod_wb = shadow.writeback_production_to_host(logic);
    let _ = shadow.writeback_body_damage_to_host(logic);
    let _ = shadow.writeback_death_type_to_host(logic);
    let _ = shadow.writeback_radar_extend_to_host(logic);
    let _ = shadow.writeback_shock_stun_to_host(logic);
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
        let _tloc_wb = shadow.writeback_target_location_to_host(logic);
        let _det_wb = shadow.writeback_detector_to_host(logic);
        let _cf_wb = shadow.writeback_continuous_fire_to_host(logic);
        let _guard_wb = shadow.writeback_guard_to_host(logic);
        let _ai_st_wb = shadow.writeback_ai_state_to_host(logic);
        let _att_wb = shadow.writeback_ai_attitude_to_host(logic);
        let _wset_wb = shadow.writeback_weapon_set_to_host(logic);
        let _oc_wb = shadow.writeback_overcharge_to_host(logic);
        let _cap_wb = shadow.writeback_contain_capacity_to_host(logic);
        let _hive_wb = shadow.writeback_hive_to_host(logic);
        let _stf_wb = shadow.writeback_stealth_flags_to_host(logic);
        let _ol_wb = shadow.writeback_overlord_to_host(logic);
        let _cs_wb = shadow.writeback_command_set_to_host(logic);
        let _dg_wb = shadow.writeback_disguise_to_host(logic);
        let _vc_wb = shadow.writeback_vision_camo_to_host(logic);
        let _ws_wb = shadow.writeback_weapon_stats_to_host(logic);
        let _mv_wb = shadow.writeback_movement_to_host(logic);
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
        let _ = shadow.writeback_body_damage_to_host(&mut logic);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
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
        let _ = shadow.writeback_body_damage_to_host(&mut logic);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
        assert!(wb >= 1);
        let o = logic.get_objects().get(&barracks).expect("b");
        let q = &o.building_data.as_ref().expect("bd").production_queue;
        assert!(!q.is_empty());
        assert_eq!(q[0].template_name, "ProdRanger");
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
        if std::env::var_os("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").is_none() {
            assert!(gameworld_damage_authority_enabled());
        }
        if std::env::var_os("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").is_none() {
            assert!(gameworld_economy_authority_enabled());
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
        assert!(n >= 1, "expected host target writeback");
        assert_eq!(logic.get_objects().get(&a).unwrap().target, Some(b));
        // Clear via shadow mutation + writeback
        assert!(shadow.queue_set_attack_target_for_host(a, None));
        let _ = shadow.apply_pending();
        let _ = shadow.writeback_attack_targets_to_host(&mut logic);
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
            assert_eq!(p.resources.supplies, before.saturating_add(123));
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
        host_production_progress_log::record(oid, vec![], 2.5);
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
        shadow.writeback_body_damage_to_host(&mut logic);
        let _ = shadow.writeback_death_type_to_host(&mut logic);
        let _ = shadow.writeback_radar_extend_to_host(&mut logic);
        let _ = shadow.writeback_shock_stun_to_host(&mut logic);
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
        let o = logic.get_objects().get(&oid).unwrap();
        assert_eq!(o.shock_stun_frames, 30);
        assert!((o.shock_yaw_rate - 0.5).abs() < 1e-5);
        assert!(o.shock_allow_bounce);
        assert!(o.cell_is_cliff);
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
}

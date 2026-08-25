//! Construct, ID maps, ordinal helpers, and host sync.

use super::types::HordePlayerRel;
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

impl GameWorldShadow {
    fn host_ready_structure_special_power_template(
        logic: &GameLogic,
        obj: &crate::game_logic::Object,
    ) -> Option<(String, u32)> {
        use crate::command_system::SpecialPowerType as P;

        obj.thing
            .template
            .special_power_modules
            .iter()
            .find(|module| {
                module.command_power.as_ref().is_some_and(|power| {
                    matches!(
                        power,
                        &P::ParticleCannon
                            | &P::SuperweaponParticleCannon
                            | &P::LaserCannon
                            | &P::ScudStorm
                            | &P::NuclearMissile
                            | &P::NukeNeutronMissile
                            | &P::SuperweaponNeutronMissile
                    ) && logic.is_special_power_ready_for(obj.id, power)
                })
            })
            .map(|module| {
                (
                    module.special_power_template.clone(),
                    module.special_power_template_id,
                )
            })
    }

    pub fn new(max_entities: usize) -> Self {
        Self {
            world: GameWorld::new(8),
            host_to_entity: HashMap::new(),
            entity_to_host: HashMap::new(),
            max_entities: max_entities.max(1),
            host_player_to_gw: HashMap::new(),
            horde_player_rel: HashMap::new(),

            production_power_factor_by_host: HashMap::new(),
            construction_rate_by_host: HashMap::new(),
            special_power_frozen_by_host: HashMap::new(),
            map_min_x: 0.0,
            map_min_z: 0.0,
            map_max_x: 0.0,
            map_max_z: 0.0,
            a10_pending_drops: Vec::new(),
            artillery_pending_drops: Vec::new(),
            carpet_pending_drops: Vec::new(),
        }
    }

    /// Discard all shadow state at a host-world replacement boundary.
    ///
    /// Host `ObjectId`s are allocator-local and may be reused by a reset,
    /// map load, or staged save restore.  Keeping the old `GameWorld` and its
    /// per-host residual queues would therefore alias entities from the
    /// previous match into the new authoritative world.  Reconstructing the
    /// shadow also clears the GameWorld's pending mutations/projectile and AI
    /// residuals, while retaining the configured entity capacity.
    pub fn reset_for_world_boundary(&mut self) {
        let max_entities = self.max_entities;
        *self = Self::new(max_entities);
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

    pub(super) fn invalidate_dead_entity_maps(&mut self) {
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
    }

    /// Full/delta sync from host: create, update health/transform/owner, destroy missing.
    /// Preserves EntityId for host objects that still exist.
    pub(super) fn host_building_type_ordinal(t: crate::game_logic::BuildingType) -> u8 {
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

    pub(super) fn host_veterancy_ordinal(level: crate::game_logic::VeterancyLevel) -> u8 {
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
            A::FacingObject => 21,
            A::FacingPosition => 22,
        }
    }

    pub fn ai_state_from_ordinal(ordinal: u8) -> crate::game_logic::AIState {
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
            21 => A::FacingObject,
            22 => A::FacingPosition,
            _ => A::Idle,
        }
    }

    /// Presentation KindOf ORDER residual (must match PresentationFrame freeze ORDER).
    pub(super) fn host_kind_of_bits(obj: &crate::game_logic::Object) -> u32 {
        obj.presentation_kind_of_bits()
    }

    pub(super) fn host_object_type_ordinal(t: crate::game_logic::ObjectType) -> u8 {
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

    pub(super) fn host_team_ordinal(team: Team) -> u8 {
        match team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 255,
        }
    }

    pub(super) fn entity_team_from_ordinal(ord: u8) -> Team {
        match ord {
            0 => Team::USA,
            1 => Team::China,
            2 => Team::GLA,
            _ => Team::Neutral,
        }
    }

    pub fn sync_from_host(&mut self, logic: &GameLogic) {
        self.sync_from_host_with(logic, true);
    }

    /// Like [`sync_from_host`]; `write_health=false` keeps existing entity HP
    /// (damage-authority path so mutations are last writer).
    pub fn sync_from_host_with(&mut self, logic: &GameLogic, write_health: bool) {
        self.sync_players(logic);

        let mut obj_ids: Vec<ObjectId> = logic.host_objects().keys().copied().collect();
        obj_ids.sort_by_key(|id| id.0);
        // C++ GameLogic grows its ObjectID lookup vector when a newly
        // allocated ID exceeds the current size (GameLogic.cpp:3827-3841).
        // `max_entities` is only the initial/reset capacity hint; silently
        // truncating this host set would leave later objects without a
        // shadow lifecycle mapping and bypass every coupled authority channel.
        if obj_ids.len() > self.max_entities {
            self.max_entities = obj_ids.len();
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
            let Some(obj) = logic.host_objects().get(&oid) else {
                continue;
            };
            let pos = obj.get_position();
            // Prefer host facing on sync (pose channel). Zero was a residual wipe that
            // forced apply_host_positions to re-queue every entity each tick.
            let transform = Transform::new([pos.x, pos.y, pos.z], obj.get_orientation());
            let owner = self.owner_for_host_object(logic, obj);
            let health = obj.health.current.max(0.0);

            if let Some(&eid) = self.host_to_entity.get(&oid.0) {
                if let Some(e) = self.world.world_mut().entity_mut(eid) {
                    let skip_sole_channels =
                        crate::gameworld_shadow::gameworld_construction_sole_tick_enabled()
                            || crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
                            || crate::gameworld_shadow::gameworld_damage_authority_live();
                    if write_health && !skip_sole_channels {
                        e.health = health;
                    }
                    e.transform = transform;
                    e.owner = owner;
                    if !skip_sole_channels {
                        e.attack_target = obj
                            .target
                            .and_then(|tid| self.host_to_entity.get(&tid.0).copied());
                    }
                    e.move_target = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
                    e.max_health = obj.max_health.max(obj.health.current).max(1.0);
                    e.body_damage_state = obj.body_damage_state.ordinal();
                    e.selected = obj.selected;
                    e.destroyed = obj.status.destroyed;
                    e.death_type = obj.status.death_type.ordinal();
                    if !skip_sole_channels {
                        e.construction_percent = obj.construction_percent.clamp(-1.0, 1.0);
                    }
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
                    e.user_1 = (obj.model_condition_bits
                        & (1u128
                            << crate::game_logic::host_enum_table_residual::user_1_model_bit()))
                        != 0;
                    e.user_2 = (obj.model_condition_bits
                        & (1u128
                            << crate::game_logic::host_enum_table_residual::user_2_model_bit()))
                        != 0;
                    e.back_crushed = obj.back_crushed;
                    e.vision_range = obj.vision_range;
                    e.shroud_clearing_range = obj.shroud_clearing_range;
                    if !skip_sole_channels {
                        e.under_construction = obj.status.under_construction;
                    }
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
                    e.disabled_script_disabled = obj.status.disabled_script_disabled;
                    e.disabled_script_underpowered = obj.status.disabled_script_underpowered;
                    e.disabled_held = obj.status.disabled_held;
                    e.weapons_jammed = obj.status.weapons_jammed;
                    e.masked = obj.status.masked;
                    e.unattackable = obj.is_kind_of(crate::game_logic::KindOf::Unattackable);
                    e.dock_kind = obj.thing.template.dock_kind as u8;
                    e.capturable = obj.thing.template.capturable;
                    e.immune_to_capture = obj.thing.template.immune_to_capture;
                    e.capture_garrisonable = obj.thing.template.garrison_contain_max.is_some();
                    e.capture_power = obj.thing.template.capture_power as u8;
                    e.capture_power_ready = obj
                        .thing
                        .template
                        .capture_power
                        .special_power_type()
                        .is_some_and(|power| logic.is_special_power_ready_for(oid, &power));
                    e.hacker_disable_building_capable = obj
                        .thing
                        .template
                        .hacker_disable_building
                        .as_ref()
                        .is_some_and(|metadata| metadata.is_hacker_command());
                    e.hacker_disable_building_ready = logic.is_hacker_disable_building_ready(oid);
                    if let Some((template_name, template_id)) =
                        Self::host_ready_structure_special_power_template(logic, obj)
                    {
                        e.special_power_ready_template_name = template_name;
                        e.special_power_ready_template_id = template_id;
                    } else {
                        e.special_power_ready_template_name.clear();
                        e.special_power_ready_template_id = 0;
                    }
                    e.disguised = obj.status.disguised;
                    e.disabled_subdued = obj.status.disabled_subdued;
                    e.subdual_damage = obj.subdual_damage;
                    e.subdual_heal_amount = obj.subdual_heal_amount;
                    e.subdual_heal_rate_frames = obj.subdual_heal_rate_frames;
                    e.subdual_heal_countdown = obj.subdual_heal_countdown;
                    if let Some(d) = obj.defection_helper.as_ref() {
                        e.defection_undetected = d.undetected_defector;
                        e.defection_detection_end = d.detection_end;
                        e.defection_detection_start = d.detection_start;
                        e.defection_flash_phase = d.flash_phase;
                        e.defection_do_fx = d.do_defector_fx;
                        e.defection_flash_this_frame = d.flash_this_frame;
                        e.defection_final_white_flash = d.final_white_flash;
                    } else {
                        e.defection_undetected = false;
                        e.defection_detection_end = 0;
                        e.defection_detection_start = 0;
                        e.defection_flash_phase = 0.0;
                        e.defection_do_fx = false;
                        e.defection_flash_this_frame = false;
                        e.defection_final_white_flash = false;
                    }
                    e.fire_sound_loop_until_frame = obj.fire_sound_loop_until_frame;
                    e.fire_sound_loop_name = obj.fire_sound_loop_name.clone();
                    if let Some(l) = obj.lifetime_update.as_ref() {
                        e.lifetime_expire_at_frame = l.expire_at_frame;
                        e.lifetime_active = l.active;
                    } else {
                        e.lifetime_expire_at_frame = 0;
                        e.lifetime_active = false;
                    }
                    if let Some(p) = obj.poisoned_behavior.as_ref() {
                        e.poison_damage_frame = p.poison_damage_frame;
                        e.poison_overall_stop_frame = p.poison_overall_stop_frame;
                        e.poison_damage_amount = p.poison_damage_amount;
                        e.poison_death_type = 5; // HostDeathType::Poisoned residual ordinal
                        e.poison_tint = p.tint_poisoned;
                    } else {
                        e.poison_damage_frame = 0;
                        e.poison_overall_stop_frame = 0;
                        e.poison_damage_amount = 0.0;
                        e.poison_death_type = 0;
                        e.poison_tint = false;
                    }
                    if let Some(td) = obj.topple_data.as_ref() {
                        e.topple_active = true;
                        e.topple_state = td.state as u8;
                        e.topple_dir_x = td.dir_x;
                        e.topple_dir_y = td.dir_y;
                        e.topple_angular_velocity = td.angular_velocity;
                        e.topple_angular_acceleration = td.angular_acceleration;
                        e.topple_angular_accumulation = td.angular_accumulation;
                        e.topple_options = td.options;
                        e.topple_kill_when_toppled = td.kill_when_toppled;
                        e.topple_lean_radians = td.lean_radians;
                    } else {
                        e.topple_active = false;
                        e.topple_state = 0;
                        e.topple_dir_x = 0.0;
                        e.topple_dir_y = 0.0;
                        e.topple_angular_velocity = 0.0;
                        e.topple_angular_acceleration = 0.0;
                        e.topple_angular_accumulation = 0.0;
                        e.topple_options = 0;
                        e.topple_kill_when_toppled = false;
                        e.topple_lean_radians = 0.0;
                    }
                    if let Some(hd) = obj.height_die.as_ref() {
                        e.height_die_active = hd.active;
                        e.height_die_target_hat = hd.target_height_above_terrain;
                        e.height_die_only_when_descending = hd.only_when_descending;
                        e.height_die_earliest_frame = hd.earliest_death_frame;
                        e.height_die_last_height = hd.last_height;
                        e.height_die_has_died = hd.has_died;
                    } else {
                        e.height_die_active = false;
                        e.height_die_target_hat = 0.0;
                        e.height_die_only_when_descending = true;
                        e.height_die_earliest_frame = 0;
                        e.height_die_last_height = f32::MAX;
                        e.height_die_has_died = false;
                    }
                    if let Some(j) = obj.jet_slow_death.as_ref() {
                        e.jet_slow_death_active = j.active;
                        e.jet_slow_death_started_on_ground = j.started_on_ground;
                        e.jet_slow_death_hit_ground = j.hit_ground;
                        e.jet_slow_death_hit_ground_frame = j.hit_ground_frame;
                        e.jet_slow_death_roll_rate = j.roll_rate;
                        e.jet_slow_death_roll_rate_delta = j.roll_rate_delta;
                        e.jet_slow_death_fall_how_fast = j.fall_how_fast;
                        e.jet_slow_death_vertical_velocity = j.vertical_velocity;
                        e.jet_slow_death_roll_accum = j.roll_accum;
                        e.jet_slow_death_done = j.done;
                    } else {
                        e.jet_slow_death_active = false;
                        e.jet_slow_death_started_on_ground = false;
                        e.jet_slow_death_hit_ground = false;
                        e.jet_slow_death_hit_ground_frame = 0;
                        e.jet_slow_death_roll_rate = 0.2;
                        e.jet_slow_death_roll_rate_delta = 1.0;
                        e.jet_slow_death_fall_how_fast = 1.10;
                        e.jet_slow_death_vertical_velocity = 0.0;
                        e.jet_slow_death_roll_accum = 0.0;
                        e.jet_slow_death_done = false;
                    }
                    if let Some(h) = obj.helicopter_slow_death.as_ref() {
                        e.heli_slow_death_active = h.active;
                        e.heli_slow_death_hit_ground = h.hit_ground;
                        e.heli_slow_death_hit_ground_frame = h.hit_ground_frame;
                        e.heli_slow_death_activate_frame = h.activate_frame;
                        e.heli_slow_death_orbit_angle = h.orbit_angle;
                        e.heli_slow_death_self_spin = h.self_spin;
                        e.heli_slow_death_self_spin_dir = h.self_spin_dir;
                        e.heli_slow_death_frames_since_spin_update = h.frames_since_spin_update;
                        e.heli_slow_death_forward_speed = h.forward_speed;
                        e.heli_slow_death_vertical_velocity = h.vertical_velocity;
                        e.heli_slow_death_orientation_delta = h.orientation_delta;
                        e.heli_slow_death_blade_flew_off = h.blade_flew_off;
                        e.heli_slow_death_done = h.done;
                    } else {
                        e.heli_slow_death_active = false;
                        e.heli_slow_death_hit_ground = false;
                        e.heli_slow_death_hit_ground_frame = 0;
                        e.heli_slow_death_activate_frame = 0;
                        e.heli_slow_death_orbit_angle = 0.0;
                        e.heli_slow_death_self_spin = 0.0;
                        e.heli_slow_death_self_spin_dir = 1.0;
                        e.heli_slow_death_frames_since_spin_update = 0;
                        e.heli_slow_death_forward_speed = 0.0;
                        e.heli_slow_death_vertical_velocity = 0.0;
                        e.heli_slow_death_orientation_delta = 0.0;
                        e.heli_slow_death_blade_flew_off = false;
                        e.heli_slow_death_done = false;
                    }
                    if let Some(sd) = obj.slow_death.as_ref() {
                        e.slow_death_phase = sd.phase as u8;
                        e.slow_death_begin_frame = sd.begin_frame;
                        e.slow_death_sink_at_frame = sd.sink_at_frame;
                        e.slow_death_destroy_at_frame = sd.destroy_at_frame;
                        e.slow_death_sink_rate_per_frame = sd.sink_rate_per_frame;
                        e.slow_death_sink_offset = sd.sink_offset;
                        e.slow_death_destruction_altitude = sd.destruction_altitude;
                        e.slow_death_fling_vx = sd.fling_vx;
                        e.slow_death_fling_vz = sd.fling_vz;
                        e.slow_death_fling_vy = sd.fling_vy;
                        e.slow_death_fling_applied = sd.fling_applied;
                    } else {
                        e.slow_death_phase = 0;
                        e.slow_death_begin_frame = 0;
                        e.slow_death_sink_at_frame = 0;
                        e.slow_death_destroy_at_frame = 0;
                        e.slow_death_sink_rate_per_frame = 0.0;
                        e.slow_death_sink_offset = 0.0;
                        e.slow_death_destruction_altitude = -10.0;
                        e.slow_death_fling_vx = 0.0;
                        e.slow_death_fling_vz = 0.0;
                        e.slow_death_fling_vy = 0.0;
                        e.slow_death_fling_applied = false;
                    }
                    if let Some(sc) = obj.structure_collapse_data.as_ref() {
                        e.structure_collapse_state = sc.state as u8;
                        e.structure_collapse_start_frame = sc.collapse_start_frame;
                        e.structure_collapse_velocity = sc.collapse_velocity;
                        e.structure_collapse_current_height = sc.current_height;
                        e.structure_collapse_damping = sc.collapse_damping;
                        e.structure_collapse_max_shudder = sc.max_shudder;
                        e.structure_collapse_building_height = sc.building_height;
                        e.structure_collapse_shudder_x = sc.shudder_x;
                        e.structure_collapse_shudder_z = sc.shudder_z;
                    } else {
                        e.structure_collapse_state = 0;
                        e.structure_collapse_start_frame = 0;
                        e.structure_collapse_velocity = 0.0;
                        e.structure_collapse_current_height = 0.0;
                        e.structure_collapse_damping = 0.0;
                        e.structure_collapse_max_shudder = 0.6;
                        e.structure_collapse_building_height = 35.0;
                        e.structure_collapse_shudder_x = 0.0;
                        e.structure_collapse_shudder_z = 0.0;
                    }
                    if let Some(st) = obj.structure_topple_data.as_ref() {
                        e.structure_topple_state = st.state as u8;
                        e.structure_topple_start_frame = st.topple_start_frame;
                        e.structure_topple_dir_x = st.dir_x;
                        e.structure_topple_dir_y = st.dir_y;
                        e.structure_topple_velocity = st.topple_velocity;
                        e.structure_topple_accumulated_angle = st.accumulated_angle;
                        e.structure_topple_structural_integrity = st.structural_integrity;
                        e.structure_topple_structural_decay = st.structural_decay;
                        e.structure_topple_done_frame = st.done_frame;
                        e.structure_topple_lean_radians = st.lean_radians;
                        e.structure_topple_last_crushed_location = st.last_crushed_location;
                        e.structure_topple_building_height = st.building_height;
                        e.structure_topple_facing_width = st.facing_width;
                    } else {
                        e.structure_topple_state = 0;
                        e.structure_topple_start_frame = 0;
                        e.structure_topple_dir_x = 0.0;
                        e.structure_topple_dir_y = 1.0;
                        e.structure_topple_velocity = 0.0;
                        e.structure_topple_accumulated_angle = 0.0;
                        e.structure_topple_structural_integrity = 0.5;
                        e.structure_topple_structural_decay = 0.1;
                        e.structure_topple_done_frame = 0;
                        e.structure_topple_lean_radians = 0.0;
                        e.structure_topple_last_crushed_location = 0.0;
                        e.structure_topple_building_height = 40.0;
                        e.structure_topple_facing_width = 20.0;
                    }
                    if let Some(fw) = obj.fire_weapon_when_damaged.as_ref() {
                        e.fwwd_active = fw.active;
                        e.fwwd_last_continuous_frame = fw.last_continuous_frame;
                        e.fwwd_continuous_reload_frames = fw.continuous_reload_frames;
                        e.fwwd_continuous_pristine =
                            fw.continuous_pristine.clone().unwrap_or_default();
                        e.fwwd_continuous_damaged =
                            fw.continuous_damaged.clone().unwrap_or_default();
                        e.fwwd_continuous_really_damaged =
                            fw.continuous_really_damaged.clone().unwrap_or_default();
                        e.fwwd_continuous_rubble = fw.continuous_rubble.clone().unwrap_or_default();
                        e.fwwd_damage_amount = fw.damage_amount;
                        e.fwwd_last_reaction_frame = fw.last_reaction_frame;
                        e.fwwd_reaction_pristine = fw.reaction_pristine.clone().unwrap_or_default();
                        e.fwwd_reaction_damaged = fw.reaction_damaged.clone().unwrap_or_default();
                        e.fwwd_reaction_really_damaged =
                            fw.reaction_really_damaged.clone().unwrap_or_default();
                        e.fwwd_reaction_rubble = fw.reaction_rubble.clone().unwrap_or_default();
                    } else {
                        e.fwwd_active = false;
                        e.fwwd_last_continuous_frame = 0;
                        e.fwwd_continuous_reload_frames = 30;
                        e.fwwd_continuous_pristine.clear();
                        e.fwwd_continuous_damaged.clear();
                        e.fwwd_continuous_really_damaged.clear();
                        e.fwwd_continuous_rubble.clear();
                        e.fwwd_damage_amount = 1.0;
                        e.fwwd_last_reaction_frame = 0;
                        e.fwwd_reaction_pristine.clear();
                        e.fwwd_reaction_damaged.clear();
                        e.fwwd_reaction_really_damaged.clear();
                        e.fwwd_reaction_rubble.clear();
                    }
                    if let Some(br) = obj.base_regenerate.as_ref() {
                        e.base_regen_active = br.active;
                        e.base_regen_wake_frame = br.wake_frame;
                        e.base_regen_done_sold = br.done_sold;
                        e.base_regen_pending_damage = br.pending_damage;
                    } else {
                        e.base_regen_active = false;
                        e.base_regen_wake_frame = 0;
                        e.base_regen_done_sold = false;
                        e.base_regen_pending_damage = false;
                    }
                    if let Some(en) = obj.enemy_near.as_ref() {
                        e.enemy_near_active = true;
                        e.enemy_near = en.enemy_near || en.model_enemy_near;
                        e.enemy_near_scan_delay = en.scan_delay;
                        e.enemy_near_scan_delay_time = en.scan_delay_time;
                        e.enemy_near_model = en.model_enemy_near;
                        e.enemy_near_vision_range = en.vision_range.max(obj.vision_range);
                    } else {
                        e.enemy_near_active = false;
                        e.enemy_near = false;
                        e.enemy_near_scan_delay = 0;
                        e.enemy_near_scan_delay_time = 30;
                        e.enemy_near_model = false;
                        e.enemy_near_vision_range = 150.0;
                    }
                    if let Some(pu) = obj.prone_update.as_ref() {
                        e.prone_active = true;
                        e.prone_frames = pu.prone_frames;
                        e.prone_damage_to_frames_ratio = pu.damage_to_frames_ratio;
                        e.prone_model = pu.model_prone;
                        e.prone_no_attack = pu.no_attack;
                    } else {
                        e.prone_active = false;
                        e.prone_frames = 0;
                        e.prone_damage_to_frames_ratio = 1.0;
                        e.prone_model = false;
                        e.prone_no_attack = false;
                    }
                    if let Some(fu) = obj.float_update.as_ref() {
                        e.float_update_active = true;
                        e.float_update_enabled = fu.enabled;
                        e.float_yaw = fu.yaw;
                        e.float_pitch = fu.pitch;
                    } else {
                        e.float_update_active = false;
                        e.float_update_enabled = false;
                        e.float_yaw = 0.0;
                        e.float_pitch = 0.0;
                    }

                    e.is_carbomb = obj.status.is_carbomb;
                    e.hijacked = obj.status.hijacked;
                    e.ignoring_stealth = obj.status.ignoring_stealth;
                    e.repulsor = obj.status.repulsor;
                    e.repulsor_until_frame = obj.repulsor_until_frame;
                    e.disabled_freefall = obj.status.disabled_freefall;
                    e.no_collisions = obj.status.no_collisions;
                    e.private_captured = obj.status.private_captured;
                    e.is_surrendered = obj.is_surrendered;
                    e.emoticon_name = obj.emoticon_name.clone();
                    e.emoticon_frames_left = obj.emoticon_frames_left;
                    e.damage_fx_name = obj
                        .pending_transition_damage_fx
                        .last()
                        .and_then(|e| e.fx_name.clone());
                    e.bone_fx_name = obj.bone_fx_damage.as_ref().and_then(|b| b.last_fx.clone());
                    e.death_fx_name = obj.pending_death_fx.clone();
                    e.formation_id = obj.formation_id;
                    e.formation_offset = [obj.formation_offset.x, obj.formation_offset.y];
                    e.disguise_transitioning_to = obj.status.disguise_transitioning_to;
                    e.disguise_halfpoint_reached = obj.status.disguise_halfpoint_reached;
                    e.faerie_fire = obj.status.faerie_fire;
                    e.booby_trapped = obj.status.booby_trapped;
                    e.eject_invulnerable = obj.status.eject_invulnerable;
                    e.eject_invulnerable_until_frame = obj.status.eject_invulnerable_until_frame;
                    e.pilot_did_move_to_base = obj.status.pilot_did_move_to_base;
                    e.parachuting = obj.status.parachuting;
                    e.parachute_open = obj.status.parachute_open;
                    e.parachute_landing_override_set = obj.status.parachute_landing_override_set;
                    e.is_building = obj.building_data.is_some();
                    if let Some(bd) = obj.building_data.as_ref() {
                        e.building_type_ordinal =
                            Self::host_building_type_ordinal(bd.building_type);
                        let skip_production =
                            crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
                        // A sole-tick shadow normally owns queue progress and
                        // must not be overwritten by the host every frame.
                        // A fresh shadow (including a post-load rebuild),
                        // however, has no queue at all.  Bootstrap exactly
                        // once from the host so a completed saved factory can
                        // still enter the C++ door/ready sequence instead of
                        // becoming an inert empty GameWorld entity.
                        let bootstrap_production = skip_production
                            && e.production_queue_items.is_empty()
                            && !bd.production_queue.is_empty();
                        if !skip_production || bootstrap_production {
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
                                        construction_frames: p.construction_frames,
                                        cost_supplies: p.cost.supplies,
                                        is_upgrade: p.is_upgrade(),
                                        quantity_total: p.quantity_total.max(1),
                                        quantity_produced: p.quantity_produced,
                                    })
                                    .collect();
                                e.production_paused = bd.production_paused;
                            }
                            if let Some(head) = bd.production_queue.first() {
                                e.production_progress = head.progress;
                                e.exit_delay_remaining = bd.exit_delay_remaining;
                                e.production_template = head.template_name.clone();
                            } else {
                                e.production_progress = 0.0;
                                e.production_template.clear();
                            }
                        }
                        // QueueProductionExitUpdate owns these per-object
                        // counters even after the head is gone.  Mirror the
                        // exact state separately from (and without stomping)
                        // sole-tick queue progress.
                        e.exit_delay_remaining = bd.exit_delay_remaining;
                        e.exit_delay_remaining_frames = bd.exit_delay_remaining_frames;
                        e.exit_burst_remaining = bd.exit_burst_remaining;
                        e.queue_exit_state_initialized = bd.queue_exit_state_initialized;
                        // Sole-tick GameWorld owns door phase after bootstrap.
                        // Copy host only when the shadow door is still idle so a
                        // host-started cycle is not lost, then never stomp GW.
                        if !skip_production || e.production_door_phase == 0 {
                            e.production_door_phase = obj.production_door_phase;
                            e.production_door_phase_end_frame = obj.production_door_phase_end_frame;
                            e.production_door_hold_open = obj.production_door_hold_open;
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
                        e.production_paused = false;
                        e.exit_delay_remaining = 0.0;
                        e.exit_delay_remaining_frames = 0;
                        e.exit_burst_remaining = 0;
                        e.queue_exit_state_initialized = false;
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
                        e.weapon_clip_size = w.clip_size;
                        e.weapon_can_target_air = w.can_target_air;
                        e.weapon_can_target_ground = w.can_target_ground;
                        e.weapon_projectile_speed = w.projectile_speed;
                    } else {
                        e.weapon_damage = 0.0;
                        e.weapon_range = 0.0;
                        e.weapon_min_range = 0.0;
                        e.weapon_reload_time = 0.0;
                        e.weapon_ammo = u32::MAX;
                        e.weapon_clip_size = 0;
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
                    e.shock_allow_bounce = obj.shock_allow_bounce;
                    e.shock_grounded_once = obj.shock_grounded_once;
                    e.shock_stun_frames = obj.shock_stun_frames;
                    e.power_plant_rods_extended = obj.power_plant_rods_extended;
                    e.power_plant_rods_done_frame = obj.power_plant_rods_done_frame;
                    e.jet_slow_death_active = obj.jet_slow_death.is_some();
                    if let Some(s) = obj.animation_steering.as_ref() {
                        e.anim_steer_active = true;
                        e.anim_steer_turn = s.current_turn_anim as u8;
                        e.anim_steer_next_transition_frame = s.next_transition_frame;
                        e.anim_steer_transition_frames = s.transition_frames;
                        e.anim_steer_has_condition = s.active_condition.is_some();
                    } else {
                        e.anim_steer_active = false;
                        e.anim_steer_turn = 0;
                        e.anim_steer_next_transition_frame = 0;
                        e.anim_steer_transition_frames = 9;
                        e.anim_steer_has_condition = false;
                    }
                    if let Some(rd) = obj.radius_decal_update.as_ref() {
                        e.radius_decal_awake = rd.awake;
                        e.radius_decal_kill_when_idle = rd.kill_when_no_longer_attacking;
                        e.radius_decal_empty = rd.delivery_decal.empty;
                        e.radius_decal_pos_x = rd.delivery_decal.position.x;
                        e.radius_decal_pos_y = rd.delivery_decal.position.y;
                        e.radius_decal_pos_z = rd.delivery_decal.position.z;
                        e.radius_decal_radius = rd.delivery_decal.radius;
                        e.radius_decal_opacity = rd.delivery_decal.opacity;
                        e.radius_decal_birth_frame = rd.delivery_decal.birth_frame;
                        if let Some(tmpl) = rd.delivery_decal.template.as_ref() {
                            e.radius_decal_opacity_min = tmpl.opacity_min;
                            e.radius_decal_opacity_max = tmpl.opacity_max;
                            e.radius_decal_throb_frames = tmpl.throb_frames;
                        }
                    } else {
                        e.radius_decal_awake = false;
                        e.radius_decal_kill_when_idle = false;
                        e.radius_decal_empty = true;
                        e.radius_decal_opacity = 0.0;
                        e.radius_decal_birth_frame = 0;
                    }
                    if let Some(cp) = obj.checkpoint_update.as_ref() {
                        e.checkpoint_active = true;
                        e.checkpoint_enemy_near = cp.enemy_near;
                        e.checkpoint_ally_near = cp.ally_near;
                        e.checkpoint_scan_delay = cp.scan_delay;
                        e.checkpoint_scan_delay_time = cp.scan_delay_time;
                        e.checkpoint_max_minor_radius = cp.max_minor_radius;
                        e.checkpoint_path_radius = cp.path_radius;
                        e.checkpoint_door_anim = cp.door_anim as u8;
                        e.checkpoint_open = cp.open;
                        e.checkpoint_vision_range = cp.vision_range.max(obj.vision_range);
                    } else {
                        e.checkpoint_active = false;
                        e.checkpoint_enemy_near = false;
                        e.checkpoint_ally_near = false;
                        e.checkpoint_scan_delay = 0;
                        e.checkpoint_scan_delay_time = 30;
                        e.checkpoint_max_minor_radius = 10.0;
                        e.checkpoint_path_radius = 10.0;
                        e.checkpoint_door_anim = 0;
                        e.checkpoint_open = false;
                        e.checkpoint_vision_range = 150.0;
                    }
                    if let Some(h) = obj.smart_bomb_target_homing.as_ref() {
                        e.smart_bomb_homing_active = true;
                        e.smart_bomb_target_received = h.target_received;
                        e.smart_bomb_course_scalar = h.course_correction_scalar;
                        e.smart_bomb_target_x = h.target.x;
                        e.smart_bomb_target_y = h.target.y;
                        e.smart_bomb_target_z = h.target.z;
                    } else {
                        e.smart_bomb_homing_active = false;
                        e.smart_bomb_target_received = false;
                        e.smart_bomb_course_scalar = 0.99;
                        e.smart_bomb_target_x = 0.0;
                        e.smart_bomb_target_y = 0.0;
                        e.smart_bomb_target_z = 0.0;
                    }
                    if let Some(d) = obj.daisy_cutter_transport.as_ref() {
                        e.daisy_transport_active = true;
                        e.daisy_transport_tier = match d.tier {
                            crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier::Moab => 1,
                            _ => 0,
                        };
                        e.daisy_transport_target_x = d.target.x;
                        e.daisy_transport_target_y = d.target.y;
                        e.daisy_transport_target_z = d.target.z;
                        e.daisy_transport_launch_x = d.launch.x;
                        e.daisy_transport_launch_y = d.launch.y;
                        e.daisy_transport_launch_z = d.launch.z;
                    } else {
                        e.daisy_transport_active = false;
                    }
                    e.daisy_cutter_bomb = obj.daisy_cutter_bomb;
                    e.daisy_bomb_vel_y = if obj.daisy_cutter_bomb {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    if let Some(d) = obj.anthrax_bomb_transport.as_ref() {
                        e.anthrax_transport_active = true;
                        e.anthrax_transport_tier = match d.tier {
                            crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier::Gamma => 1,
                            _ => 0,
                        };
                        e.anthrax_transport_target_x = d.target.x;
                        e.anthrax_transport_target_y = d.target.y;
                        e.anthrax_transport_target_z = d.target.z;
                        e.anthrax_transport_launch_x = d.launch.x;
                        e.anthrax_transport_launch_y = d.launch.y;
                        e.anthrax_transport_launch_z = d.launch.z;
                        e.anthrax_delivery_complete = d.delivery_complete;
                    } else {
                        e.anthrax_transport_active = false;
                    }
                    e.anthrax_bomb_payload = obj.anthrax_bomb_payload;
                    e.anthrax_bomb_vel_y = if obj.anthrax_bomb_payload {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    if let Some(d) = obj.cluster_mines_transport.as_ref() {
                        e.cluster_mines_transport_active = true;
                        e.cluster_mines_transport_target_x = d.target.x;
                        e.cluster_mines_transport_target_y = d.target.y;
                        e.cluster_mines_transport_target_z = d.target.z;
                        e.cluster_mines_transport_launch_x = d.launch.x;
                        e.cluster_mines_transport_launch_y = d.launch.y;
                        e.cluster_mines_transport_launch_z = d.launch.z;
                    } else {
                        e.cluster_mines_transport_active = false;
                    }
                    e.cluster_mines_bomb = obj.cluster_mines_bomb;
                    e.cluster_mines_bomb_vel_y = if obj.cluster_mines_bomb {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    if let Some(d) = obj.emp_pulse_transport.as_ref() {
                        e.emp_pulse_transport_active = true;
                        e.emp_pulse_transport_player_id = d.player_id;
                        e.emp_pulse_transport_caster_id = d.caster_id;
                        e.emp_pulse_transport_target_x = d.target.x;
                        e.emp_pulse_transport_target_y = d.target.y;
                        e.emp_pulse_transport_target_z = d.target.z;
                        e.emp_pulse_transport_launch_x = d.launch.x;
                        e.emp_pulse_transport_launch_y = d.launch.y;
                        e.emp_pulse_transport_launch_z = d.launch.z;
                    } else {
                        e.emp_pulse_transport_active = false;
                    }
                    e.emp_pulse_bomb = obj.emp_pulse_bomb;
                    e.emp_pulse_bomb_vel_y = if obj.emp_pulse_bomb {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    e.emp_pulse_spheroid = obj.emp_pulse_spheroid;
                    e.emp_pulse_spheroid_expires_frame =
                        obj.emp_pulse_spheroid_expires_frame.unwrap_or(0);
                    if let Some(d) = obj.a10_strike_transport.as_ref() {
                        e.a10_strike_transport_active = true;
                        e.a10_strike_transport_tier = match d.tier {
                            crate::game_logic::special_power_strikes::A10StrikeScienceTier::Level2 => 1,
                            crate::game_logic::special_power_strikes::A10StrikeScienceTier::Level3 => 2,
                            _ => 0,
                        };
                        e.a10_strike_transport_target_x = d.target.x;
                        e.a10_strike_transport_target_y = d.target.y;
                        e.a10_strike_transport_target_z = d.target.z;
                        e.a10_strike_transport_launch_x = d.launch.x;
                        e.a10_strike_transport_launch_y = d.launch.y;
                        e.a10_strike_transport_launch_z = d.launch.z;
                        e.a10_strike_dive_state = d.dive_state;
                        e.a10_strike_last_vulcan_frame = d.last_vulcan_frame;
                    } else {
                        e.a10_strike_transport_active = false;
                        e.a10_strike_dive_state = 0;
                        e.a10_strike_last_vulcan_frame = 0;
                    }
                    e.a10_strike_missile = obj.a10_strike_missile;
                    if obj.a10_strike_missile {
                        e.a10_strike_missile_vel_y = obj.movement.velocity.y;
                        e.a10_strike_transport_launch_x = obj.movement.velocity.x;
                        e.a10_strike_transport_launch_z = obj.movement.velocity.z;
                    } else {
                        e.a10_strike_missile_vel_y = 0.0;
                    }
                    if let Some(d) = obj.artillery_barrage_transport.as_ref() {
                        e.artillery_barrage_transport_active = true;
                        e.artillery_barrage_transport_tier = match d.tier {
                            crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier::Level2 => 1,
                            crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier::Level3 => 2,
                            _ => 0,
                        };
                        e.artillery_barrage_transport_target_x = d.target.x;
                        e.artillery_barrage_transport_target_y = d.target.y;
                        e.artillery_barrage_transport_target_z = d.target.z;
                        e.artillery_barrage_transport_launch_x = d.launch.x;
                        e.artillery_barrage_transport_launch_y = d.launch.y;
                        e.artillery_barrage_transport_launch_z = d.launch.z;
                    } else {
                        e.artillery_barrage_transport_active = false;
                    }
                    e.artillery_barrage_shell = obj.artillery_barrage_shell;
                    e.artillery_barrage_shell_vel_y = if obj.artillery_barrage_shell {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    if let Some(d) = obj.carpet_bomb_transport.as_ref() {
                        e.carpet_bomb_transport_active = true;
                        e.carpet_bomb_transport_tier = match d.tier {
                            crate::game_logic::special_power_strikes::CarpetBombFactionTier::AirForce => 1,
                            crate::game_logic::special_power_strikes::CarpetBombFactionTier::China => 2,
                            _ => 0,
                        };
                        e.carpet_bomb_transport_target_x = d.target.x;
                        e.carpet_bomb_transport_target_y = d.target.y;
                        e.carpet_bomb_transport_target_z = d.target.z;
                        e.carpet_bomb_transport_launch_x = d.launch.x;
                        e.carpet_bomb_transport_launch_y = d.launch.y;
                        e.carpet_bomb_transport_launch_z = d.launch.z;
                    } else {
                        e.carpet_bomb_transport_active = false;
                    }
                    e.carpet_bomb_payload = obj.carpet_bomb_payload;
                    e.carpet_bomb_payload_vel_y = if obj.carpet_bomb_payload {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    if let Some(t) = obj.leaflet_transport_target {
                        e.leaflet_transport_active = true;
                        e.leaflet_transport_target_x = t.x;
                        e.leaflet_transport_target_y = t.y;
                        e.leaflet_transport_target_z = t.z;
                    } else {
                        e.leaflet_transport_active = false;
                    }
                    e.leaflet_container = obj.leaflet_container;
                    e.leaflet_container_vel_y = if obj.leaflet_container {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    if let Some(t) = obj.paradrop_transport_target {
                        e.paradrop_transport_active = true;
                        e.paradrop_transport_target_x = t.x;
                        e.paradrop_transport_target_y = t.y;
                        e.paradrop_transport_target_z = t.z;
                    } else {
                        e.paradrop_transport_active = false;
                    }
                    e.paradrop_parachute = obj.paradrop_parachute;
                    e.paradrop_parachute_vel_y = if obj.paradrop_parachute {
                        obj.movement.velocity.y
                    } else {
                        0.0
                    };
                    e.aurora_bomb_projectile = obj.aurora_bomb_projectile;
                    if let Some(a) = obj.aurora_bomb_aim {
                        e.aurora_bomb_has_aim = true;
                        e.aurora_bomb_aim_x = a[0];
                        e.aurora_bomb_aim_y = a[1];
                        e.aurora_bomb_aim_z = a[2];
                    } else {
                        e.aurora_bomb_has_aim = false;
                    }
                    e.aurora_bomb_mission_id = obj.aurora_bomb_mission_id.unwrap_or(0);
                    e.aurora_bomb_mission_live = obj
                        .aurora_bomb_mission_id
                        .map(|mid| logic.aurora_bombs.has_mission(mid))
                        .unwrap_or(false);
                    e.toxin_stream_projectile = obj.toxin_stream_projectile;
                    if let Some(a) = obj.toxin_stream_aim {
                        e.toxin_stream_has_aim = true;
                        e.toxin_stream_aim_x = a[0];
                        e.toxin_stream_aim_y = a[1];
                        e.toxin_stream_aim_z = a[2];
                    } else {
                        e.toxin_stream_has_aim = false;
                    }
                    if let Some(i) = obj.toxin_stream_intended {
                        e.toxin_stream_has_intended = true;
                        e.toxin_stream_intended = i;
                    } else {
                        e.toxin_stream_has_intended = false;
                    }
                    e.toxin_stream_travelled = obj.toxin_stream_travelled;
                    if let Some(f) = obj.toxin_stream_fuel_expires_frame {
                        e.toxin_stream_has_fuel = true;
                        e.toxin_stream_fuel_expires_frame = f;
                    } else {
                        e.toxin_stream_has_fuel = false;
                    }
                    if let Some(f) = obj.toxin_stream_ignition_frame {
                        e.toxin_stream_has_ignition = true;
                        e.toxin_stream_ignition_frame = f;
                    } else {
                        e.toxin_stream_has_ignition = false;
                    }
                    if let Some(s) = obj.toxin_stream_shooter {
                        e.toxin_stream_has_shooter = true;
                        e.toxin_stream_shooter = s;
                    } else {
                        e.toxin_stream_has_shooter = false;
                    }
                    e.angry_mob_projectile = obj.angry_mob_projectile;
                    e.angry_mob_projectile_kind = obj.angry_mob_projectile_kind;
                    if let Some(a) = obj.angry_mob_projectile_from {
                        e.angry_mob_projectile_has_from = true;
                        e.angry_mob_projectile_from_x = a[0];
                        e.angry_mob_projectile_from_y = a[1];
                        e.angry_mob_projectile_from_z = a[2];
                    } else {
                        e.angry_mob_projectile_has_from = false;
                    }
                    if let Some(a) = obj.angry_mob_projectile_aim {
                        e.angry_mob_projectile_has_aim = true;
                        e.angry_mob_projectile_aim_x = a[0];
                        e.angry_mob_projectile_aim_y = a[1];
                        e.angry_mob_projectile_aim_z = a[2];
                    } else {
                        e.angry_mob_projectile_has_aim = false;
                    }
                    e.angry_mob_projectile_launch_frame =
                        obj.angry_mob_projectile_launch_frame.unwrap_or(0);
                    e.angry_mob_projectile_flight_frames = obj.angry_mob_projectile_flight_frames;
                    if let Some(i) = obj.angry_mob_projectile_intended {
                        e.angry_mob_projectile_has_intended = true;
                        e.angry_mob_projectile_intended = i;
                    } else {
                        e.angry_mob_projectile_has_intended = false;
                    }
                    e.scud_launcher_missile_projectile = obj.scud_launcher_missile_projectile;
                    e.scud_launcher_missile_toxin = obj.scud_launcher_missile_toxin;
                    if let Some(a) = obj.scud_launcher_missile_aim {
                        e.scud_launcher_missile_has_aim = true;
                        e.scud_launcher_missile_aim_x = a[0];
                        e.scud_launcher_missile_aim_y = a[1];
                        e.scud_launcher_missile_aim_z = a[2];
                    } else {
                        e.scud_launcher_missile_has_aim = false;
                    }
                    e.scud_launcher_missile_travelled = obj.scud_launcher_missile_travelled;
                    if let Some(f) = obj.scud_launcher_missile_fuel_expires_frame {
                        e.scud_launcher_missile_has_fuel = true;
                        e.scud_launcher_missile_fuel_expires_frame = f;
                    } else {
                        e.scud_launcher_missile_has_fuel = false;
                    }
                    e.neutron_cannon_shell_projectile = obj.neutron_cannon_shell_projectile;
                    if let Some(a) = obj.neutron_shell_from {
                        e.neutron_shell_has_from = true;
                        e.neutron_shell_from_x = a[0];
                        e.neutron_shell_from_y = a[1];
                        e.neutron_shell_from_z = a[2];
                    } else {
                        e.neutron_shell_has_from = false;
                    }
                    if let Some(a) = obj.neutron_shell_aim {
                        e.neutron_shell_has_aim = true;
                        e.neutron_shell_aim_x = a[0];
                        e.neutron_shell_aim_y = a[1];
                        e.neutron_shell_aim_z = a[2];
                    } else {
                        e.neutron_shell_has_aim = false;
                    }
                    e.neutron_shell_launch_frame = obj.neutron_shell_launch_frame.unwrap_or(0);
                    e.neutron_shell_flight_frames = obj.neutron_shell_flight_frames;
                    e.nuke_cannon_shell_projectile = obj.nuke_cannon_shell_projectile;
                    if let Some(a) = obj.nuke_shell_from {
                        e.nuke_shell_has_from = true;
                        e.nuke_shell_from_x = a[0];
                        e.nuke_shell_from_y = a[1];
                        e.nuke_shell_from_z = a[2];
                    } else {
                        e.nuke_shell_has_from = false;
                    }
                    if let Some(a) = obj.nuke_shell_aim {
                        e.nuke_shell_has_aim = true;
                        e.nuke_shell_aim_x = a[0];
                        e.nuke_shell_aim_y = a[1];
                        e.nuke_shell_aim_z = a[2];
                    } else {
                        e.nuke_shell_has_aim = false;
                    }
                    e.nuke_shell_launch_frame = obj.nuke_shell_launch_frame.unwrap_or(0);
                    e.nuke_shell_flight_frames = obj.nuke_shell_flight_frames;
                    e.angry_mob_member = obj.angry_mob_member;
                    if let Some(n) = obj.angry_mob_nexus_id {
                        e.angry_mob_has_nexus = true;
                        e.angry_mob_nexus_id = n.0;
                    } else {
                        e.angry_mob_has_nexus = false;
                        e.angry_mob_nexus_id = 0;
                    }
                    e.nuke_radiation_field = obj.nuke_radiation_field;
                    e.nuke_radiation_field_expires_frame =
                        obj.nuke_radiation_field_expires_frame.unwrap_or(0);
                    e.anthrax_toxin_field = obj.anthrax_toxin_field;
                    e.anthrax_toxin_field_expires_frame =
                        obj.anthrax_toxin_field_expires_frame.unwrap_or(0);
                    e.inferno_fire_field = obj.inferno_fire_field;
                    e.inferno_fire_field_expires_frame =
                        obj.inferno_fire_field_expires_frame.unwrap_or(0);
                    e.inferno_shell_projectile = obj.inferno_shell_projectile;
                    if let Some(a) = obj.inferno_shell_from {
                        e.inferno_shell_has_from = true;
                        e.inferno_shell_from_x = a[0];
                        e.inferno_shell_from_y = a[1];
                        e.inferno_shell_from_z = a[2];
                    } else {
                        e.inferno_shell_has_from = false;
                    }
                    if let Some(a) = obj.inferno_shell_aim {
                        e.inferno_shell_has_aim = true;
                        e.inferno_shell_aim_x = a[0];
                        e.inferno_shell_aim_y = a[1];
                        e.inferno_shell_aim_z = a[2];
                    } else {
                        e.inferno_shell_has_aim = false;
                    }
                    e.inferno_shell_launch_frame = obj.inferno_shell_launch_frame.unwrap_or(0);
                    e.inferno_shell_flight_frames = obj.inferno_shell_flight_frames;
                    if let Some(i) = obj.inferno_shell_intended {
                        e.inferno_shell_has_intended = true;
                        e.inferno_shell_intended = i;
                    } else {
                        e.inferno_shell_has_intended = false;
                    }
                    e.inferno_shell_upgraded = obj.inferno_shell_upgraded;
                    e.spy_satellite_ping = obj.spy_satellite_ping;
                    e.spy_satellite_ping_expires_frame =
                        obj.spy_satellite_ping_expires_frame.unwrap_or(0);
                    e.flashbang_grenade_projectile = obj.flashbang_grenade_projectile;
                    if let Some(a) = obj.flashbang_grenade_from {
                        e.flashbang_grenade_has_from = true;
                        e.flashbang_grenade_from_x = a[0];
                        e.flashbang_grenade_from_y = a[1];
                        e.flashbang_grenade_from_z = a[2];
                    } else {
                        e.flashbang_grenade_has_from = false;
                    }
                    if let Some(a) = obj.flashbang_grenade_aim {
                        e.flashbang_grenade_has_aim = true;
                        e.flashbang_grenade_aim_x = a[0];
                        e.flashbang_grenade_aim_y = a[1];
                        e.flashbang_grenade_aim_z = a[2];
                    } else {
                        e.flashbang_grenade_has_aim = false;
                    }
                    e.flashbang_grenade_launch_frame =
                        obj.flashbang_grenade_launch_frame.unwrap_or(0);
                    e.flashbang_grenade_flight_frames = obj.flashbang_grenade_flight_frames;
                    if let Some(i) = obj.flashbang_grenade_intended {
                        e.flashbang_grenade_has_intended = true;
                        e.flashbang_grenade_intended = i;
                    } else {
                        e.flashbang_grenade_has_intended = false;
                    }
                    e.comanche_rocket_pod_projectile = obj.comanche_rocket_pod_projectile;
                    e.comanche_rocket_pod_projectile_expires_frame = obj
                        .comanche_rocket_pod_projectile_expires_frame
                        .unwrap_or(0);
                    e.helix_napalm_bomb_projectile = obj.helix_napalm_bomb_projectile;
                    e.scorpion_missile_projectile = obj.scorpion_missile_projectile;
                    if let Some(a) = obj.scorpion_missile_aim {
                        e.scorpion_missile_has_aim = true;
                        e.scorpion_missile_aim_x = a[0];
                        e.scorpion_missile_aim_y = a[1];
                        e.scorpion_missile_aim_z = a[2];
                    } else {
                        e.scorpion_missile_has_aim = false;
                    }
                    if let Some(i) = obj.scorpion_missile_intended {
                        e.scorpion_missile_has_intended = true;
                        e.scorpion_missile_intended = i;
                    } else {
                        e.scorpion_missile_has_intended = false;
                    }
                    e.scorpion_missile_travelled = obj.scorpion_missile_travelled;
                    e.scorpion_missile_fuel_expires_frame =
                        obj.scorpion_missile_fuel_expires_frame.unwrap_or(0);
                    e.scorpion_missile_slot = obj.scorpion_missile_slot;
                    e.spectre_howitzer_shell = obj.spectre_howitzer_shell;
                    e.spectre_howitzer_shell_expires_frame =
                        obj.spectre_howitzer_shell_expires_frame.unwrap_or(0);
                    e.countermeasure_flare = obj.countermeasure_flare;
                    e.countermeasure_flare_expires_frame =
                        obj.countermeasure_flare_expires_frame.unwrap_or(0);
                    e.point_defense_laser_beam = obj.point_defense_laser_beam;
                    e.point_defense_laser_beam_expires_frame =
                        obj.point_defense_laser_beam_expires_frame.unwrap_or(0);
                    e.weapon_laser_beam = obj.weapon_laser_beam;
                    e.weapon_laser_beam_expires_frame =
                        obj.weapon_laser_beam_expires_frame.unwrap_or(0);
                    {
                        use crate::game_logic::host_mines::HostMineKind;
                        if let Some(md) = obj.mine_data.as_ref() {
                            let sticky = matches!(
                                md.kind,
                                HostMineKind::TimedDemoCharge | HostMineKind::RemoteDemoCharge
                            ) && md.attached_to.is_some()
                                && md.is_active();
                            e.sticky_bomb_attached = sticky;
                            e.sticky_bomb_attached_to = md.attached_to.map(|id| id.0).unwrap_or(0);
                            e.sticky_bomb_mine_kind = match md.kind {
                                HostMineKind::TimedDemoCharge => 2,
                                HostMineKind::RemoteDemoCharge => 3,
                                HostMineKind::LandMine => 0,
                                HostMineKind::DemoTrap => 1,
                            };
                        } else {
                            e.sticky_bomb_attached = false;
                            e.sticky_bomb_attached_to = 0;
                            e.sticky_bomb_mine_kind = 0;
                        }
                    }
                    e.booby_trap_special = obj.booby_trap_special;
                    if let Some(a) = obj.booby_trap_attached_to {
                        e.booby_trap_has_attached = true;
                        e.booby_trap_attached_to = a.0;
                    } else {
                        e.booby_trap_has_attached = false;
                        e.booby_trap_attached_to = 0;
                    }
                    e.particle_trail_remnant = obj.particle_trail_remnant;
                    e.particle_trail_remnant_expires_frame =
                        obj.particle_trail_remnant_expires_frame.unwrap_or(0);
                    e.particle_orbital_laser = obj.particle_orbital_laser;
                    e.particle_orbital_laser_expires_frame =
                        obj.particle_orbital_laser_expires_frame.unwrap_or(0);
                    e.particle_connector_laser = obj.particle_connector_laser;
                    e.particle_connector_laser_expires_frame =
                        obj.particle_connector_laser_expires_frame.unwrap_or(0);
                    e.firewall_segment = obj.firewall_segment;
                    e.firewall_segment_expires_frame =
                        obj.firewall_segment_expires_frame.unwrap_or(0);
                    if let Some(wid) = obj.firewall_segment_wall_id {
                        e.firewall_segment_has_wall_id = true;
                        e.firewall_segment_wall_id = wid;
                    } else {
                        e.firewall_segment_has_wall_id = false;
                        e.firewall_segment_wall_id = 0;
                    }
                    if let Some(d) = obj.firewall_segment_dir {
                        e.firewall_segment_has_dir = true;
                        e.firewall_segment_dir_x = d[0];
                        e.firewall_segment_dir_z = d[1];
                    } else {
                        e.firewall_segment_has_dir = false;
                        e.firewall_segment_dir_x = 1.0;
                        e.firewall_segment_dir_z = 0.0;
                    }
                    e.radar_van_ping = obj.radar_van_ping;
                    e.radar_van_ping_expires_frame = obj.radar_van_ping_expires_frame.unwrap_or(0);
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
                    e.allow_motive_force_while_airborne = obj.allow_motive_force_while_airborne;
                    e.locomotor_works_when_dead = obj.locomotor_works_when_dead;
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
                    e.enemy_near = obj
                        .enemy_near
                        .as_ref()
                        .map(|d| d.model_enemy_near || d.enemy_near)
                        .unwrap_or(false);
                    e.armed = obj.armed_riders_upgrade_weapon_set
                        || (!obj.occupants.is_empty() && obj.passengers_allowed_to_fire);
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
                    e.engine_bridged = false;
                    e.overlord_bunker_capacity = obj
                        .overlord_bunker_capacity
                        .map(|n| n.min(u16::MAX as usize - 1) as u16)
                        .unwrap_or(u16::MAX);
                    e.passengers_allowed_to_fire = obj.passengers_allowed_to_fire;
                    e.armed_riders_upgrade_weapon_set = obj.armed_riders_upgrade_weapon_set;
                    e.weapon_set_player_upgrade = obj.weapon_set_player_upgrade;
                    e.second_life = obj.armor_set_second_life;
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
                    e.weapon_fire_status = obj.weapon_fire_status as u8;
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
                    e.weapon_bonus_fanaticism = obj.weapon_bonus_fanaticism;
                    e.last_horde_refresh_frame = obj.last_horde_refresh_frame;
                    e.horde_next_wake_frame = obj.horde_next_wake_frame;
                    e.horde_wake_initialized = obj.horde_wake_initialized;

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
                    e.hive_slaves_alive = [
                        obj.hive_slaves[0].alive,
                        obj.hive_slaves[1].alive,
                        obj.hive_slaves[2].alive,
                    ];
                    e.hive_slaves_hp = [
                        obj.hive_slaves[0].hp,
                        obj.hive_slaves[1].hp,
                        obj.hive_slaves[2].hp,
                    ];
                    if let Some(ce) = logic.host_money_crates.get(oid) {
                        e.money_crate = true;
                        e.money_crate_expires_frame = ce.expires_frame;
                    } else {
                        e.money_crate = false;
                        e.money_crate_expires_frame = 0;
                    }
                    if let Some(fs) = obj.fire_spread.as_ref() {
                        e.fire_spread_active = fs.active;
                        e.fire_spread_state = match fs.state {
                            crate::game_logic::host_fire_spread::HostFlammableState::Normal => 0,
                            crate::game_logic::host_fire_spread::HostFlammableState::Aflame => 1,
                            crate::game_logic::host_fire_spread::HostFlammableState::Burned => 2,
                        };
                        e.fire_spread_aflame_end_frame = fs.aflame_end_frame;
                        e.fire_spread_burned_end_frame = fs.burned_end_frame;
                        e.fire_spread_next_spread_frame = fs.next_spread_frame;
                        e.fire_spread_min_delay = fs.min_spread_delay;
                        e.fire_spread_max_delay = fs.max_spread_delay;
                        e.fire_spread_try_range = fs.spread_try_range;
                        e.fire_spread_aflame_duration = fs.aflame_duration;
                        e.fire_spread_burned_delay = fs.burned_delay;
                        e.fire_spread_enabled = fs.spread_enabled;
                        e.fire_spread_flame_damage_accum = fs.flame_damage_accum;
                        e.fire_spread_flame_damage_limit = fs.flame_damage_limit;
                    } else {
                        e.fire_spread_active = false;
                        e.fire_spread_state = 0;
                    }
                    {
                        use crate::game_logic::host_oil_derrick::is_oil_derrick_template;
                        use crate::game_logic::is_black_market_template;
                        let name = obj.template_name.as_str();
                        let is_bm = is_black_market_template(name)
                            || name.to_ascii_lowercase().contains("blackmarket");
                        let is_fake = name.to_ascii_lowercase().contains("fake");
                        if is_bm && !is_fake {
                            e.black_market_building = true;
                            e.black_market_next_deposit_frame =
                                logic.black_markets.peek_next_deposit(oid).unwrap_or(0);
                        } else {
                            e.black_market_building = false;
                            e.black_market_next_deposit_frame = 0;
                        }
                        if is_oil_derrick_template(name) {
                            e.oil_derrick_building = true;
                            e.oil_derrick_next_deposit_frame =
                                logic.oil_derricks.peek_next_deposit(oid).unwrap_or(0);
                        } else {
                            e.oil_derrick_building = false;
                            e.oil_derrick_next_deposit_frame = 0;
                        }
                    }
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
                    self.spawn_mapped(oid, obj, owner, transform, health);
                }
            } else {
                self.spawn_mapped(oid, obj, owner, transform, health);
            }
        }

        // Second pass: resolve attack targets now that all IDs are mapped.
        for oid in logic.host_objects().keys().copied() {
            let Some(obj) = logic.host_objects().get(&oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            let at = obj
                .target
                .and_then(|tid| self.host_to_entity.get(&tid.0).copied());
            if let Some(e) = self.world.world_mut().entity_mut(eid) {
                let skip_construction =
                    crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
                let skip_production =
                    crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
                e.attack_target = at;
                e.move_target = obj.movement.target_position.map(|p| [p.x, p.y, p.z]);
                e.max_health = obj.max_health.max(obj.health.current).max(1.0);
                e.selected = obj.selected;
                e.destroyed = obj.status.destroyed;
                if !skip_construction {
                    e.construction_percent = obj.construction_percent.clamp(-1.0, 1.0);
                    e.under_construction = obj.status.under_construction;
                }
                e.team_ordinal = Self::host_team_ordinal(obj.team);
                e.selection_radius = obj.selection_radius.max(5.0);
                e.crusher_level = obj.crusher_level;
                e.crushable_level = obj.crushable_level;
                e.vision_range = obj.vision_range;
                e.shroud_clearing_range = obj.shroud_clearing_range;
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
                e.disabled_script_disabled = obj.status.disabled_script_disabled;
                e.disabled_script_underpowered = obj.status.disabled_script_underpowered;
                e.disabled_held = obj.status.disabled_held;
                e.weapons_jammed = obj.status.weapons_jammed;
                e.masked = obj.status.masked;
                e.unattackable = obj.is_kind_of(crate::game_logic::KindOf::Unattackable);
                e.dock_kind = obj.thing.template.dock_kind as u8;
                e.capturable = obj.thing.template.capturable;
                e.immune_to_capture = obj.thing.template.immune_to_capture;
                e.capture_garrisonable = obj.thing.template.garrison_contain_max.is_some();
                e.capture_power = obj.thing.template.capture_power as u8;
                e.capture_power_ready = obj
                    .thing
                    .template
                    .capture_power
                    .special_power_type()
                    .is_some_and(|power| logic.is_special_power_ready_for(oid, &power));
                e.hacker_disable_building_capable = obj
                    .thing
                    .template
                    .hacker_disable_building
                    .as_ref()
                    .is_some_and(|metadata| metadata.is_hacker_command());
                e.hacker_disable_building_ready = logic.is_hacker_disable_building_ready(oid);
                if let Some((template_name, template_id)) =
                    Self::host_ready_structure_special_power_template(logic, obj)
                {
                    e.special_power_ready_template_name = template_name;
                    e.special_power_ready_template_id = template_id;
                } else {
                    e.special_power_ready_template_name.clear();
                    e.special_power_ready_template_id = 0;
                }
                e.disguised = obj.status.disguised;
                e.disabled_subdued = obj.status.disabled_subdued;
                e.subdual_damage = obj.subdual_damage;
                e.subdual_heal_amount = obj.subdual_heal_amount;
                e.subdual_heal_rate_frames = obj.subdual_heal_rate_frames;
                e.subdual_heal_countdown = obj.subdual_heal_countdown;
                if let Some(d) = obj.defection_helper.as_ref() {
                    e.defection_undetected = d.undetected_defector;
                    e.defection_detection_end = d.detection_end;
                    e.defection_detection_start = d.detection_start;
                    e.defection_flash_phase = d.flash_phase;
                    e.defection_do_fx = d.do_defector_fx;
                    e.defection_flash_this_frame = d.flash_this_frame;
                    e.defection_final_white_flash = d.final_white_flash;
                } else {
                    e.defection_undetected = false;
                    e.defection_detection_end = 0;
                    e.defection_detection_start = 0;
                    e.defection_flash_phase = 0.0;
                    e.defection_do_fx = false;
                    e.defection_flash_this_frame = false;
                    e.defection_final_white_flash = false;
                }
                e.fire_sound_loop_until_frame = obj.fire_sound_loop_until_frame;
                e.fire_sound_loop_name = obj.fire_sound_loop_name.clone();
                if let Some(l) = obj.lifetime_update.as_ref() {
                    e.lifetime_expire_at_frame = l.expire_at_frame;
                    e.lifetime_active = l.active;
                } else {
                    e.lifetime_expire_at_frame = 0;
                    e.lifetime_active = false;
                }
                if let Some(p) = obj.poisoned_behavior.as_ref() {
                    e.poison_damage_frame = p.poison_damage_frame;
                    e.poison_overall_stop_frame = p.poison_overall_stop_frame;
                    e.poison_damage_amount = p.poison_damage_amount;
                    e.poison_death_type = 5 /* Poisoned */;
                    e.poison_tint = p.tint_poisoned;
                } else {
                    e.poison_damage_frame = 0;
                    e.poison_overall_stop_frame = 0;
                    e.poison_damage_amount = 0.0;
                    e.poison_death_type = 0;
                    e.poison_tint = false;
                }

                e.is_carbomb = obj.status.is_carbomb;
                e.hijacked = obj.status.hijacked;
                e.ignoring_stealth = obj.status.ignoring_stealth;
                e.repulsor = obj.status.repulsor;
                e.repulsor_until_frame = obj.repulsor_until_frame;
                e.disabled_freefall = obj.status.disabled_freefall;
                e.no_collisions = obj.status.no_collisions;
                e.private_captured = obj.status.private_captured;
                e.is_surrendered = obj.is_surrendered;
                e.emoticon_name = obj.emoticon_name.clone();
                e.emoticon_frames_left = obj.emoticon_frames_left;
                e.damage_fx_name = obj
                    .pending_transition_damage_fx
                    .last()
                    .and_then(|e| e.fx_name.clone());
                e.bone_fx_name = obj.bone_fx_damage.as_ref().and_then(|b| b.last_fx.clone());
                e.death_fx_name = obj.pending_death_fx.clone();
                e.formation_id = obj.formation_id;
                e.formation_offset = [obj.formation_offset.x, obj.formation_offset.y];
                e.disguise_transitioning_to = obj.status.disguise_transitioning_to;
                e.disguise_halfpoint_reached = obj.status.disguise_halfpoint_reached;
                e.faerie_fire = obj.status.faerie_fire;
                e.booby_trapped = obj.status.booby_trapped;
                e.eject_invulnerable = obj.status.eject_invulnerable;
                e.eject_invulnerable_until_frame = obj.status.eject_invulnerable_until_frame;
                e.pilot_did_move_to_base = obj.status.pilot_did_move_to_base;
                e.parachuting = obj.status.parachuting;
                e.parachute_open = obj.status.parachute_open;
                e.parachute_landing_override_set = obj.status.parachute_landing_override_set;
                e.is_building = obj.building_data.is_some();
                if let Some(bd) = obj.building_data.as_ref() {
                    e.building_type_ordinal = Self::host_building_type_ordinal(bd.building_type);
                    // See the primary sync: sole ownership means do not
                    // stomp a live GameWorld queue, except when this entity
                    // has just been shadowed and needs its saved host queue
                    // to bootstrap the first production tick.
                    let bootstrap_production = skip_production
                        && e.production_queue_items.is_empty()
                        && !bd.production_queue.is_empty();
                    if !skip_production || bootstrap_production {
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
                                    construction_frames: p.construction_frames,
                                    cost_supplies: p.cost.supplies,
                                    is_upgrade: p.is_upgrade(),
                                    quantity_total: p.quantity_total.max(1),
                                    quantity_produced: p.quantity_produced,
                                })
                                .collect();
                            e.production_paused = bd.production_paused;
                        }
                        if let Some(head) = bd.production_queue.first() {
                            e.production_progress = head.progress;
                            e.exit_delay_remaining = bd.exit_delay_remaining;
                            e.production_template = head.template_name.clone();
                        } else {
                            e.production_progress = 0.0;
                            e.production_template.clear();
                        }
                    }
                    e.exit_delay_remaining = bd.exit_delay_remaining;
                    e.exit_delay_remaining_frames = bd.exit_delay_remaining_frames;
                    e.exit_burst_remaining = bd.exit_burst_remaining;
                    e.queue_exit_state_initialized = bd.queue_exit_state_initialized;
                    let skip_door =
                        crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
                            && e.production_door_phase != 0;
                    if !skip_door {
                        e.production_door_phase = obj.production_door_phase;
                        e.production_door_phase_end_frame = obj.production_door_phase_end_frame;
                        e.production_door_hold_open = obj.production_door_hold_open;
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
                    e.production_paused = false;
                    e.exit_delay_remaining = 0.0;
                    e.exit_delay_remaining_frames = 0;
                    e.exit_burst_remaining = 0;
                    e.queue_exit_state_initialized = false;
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
                    e.weapon_clip_size = w.clip_size;
                    e.weapon_can_target_air = w.can_target_air;
                    e.weapon_can_target_ground = w.can_target_ground;
                    e.weapon_projectile_speed = w.projectile_speed;
                } else {
                    e.weapon_damage = 0.0;
                    e.weapon_range = 0.0;
                    e.weapon_min_range = 0.0;
                    e.weapon_reload_time = 0.0;
                    e.weapon_ammo = u32::MAX;
                    e.weapon_clip_size = 0;
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
                e.engine_bridged = false;
                e.overlord_bunker_capacity = obj
                    .overlord_bunker_capacity
                    .map(|n| n.min(u16::MAX as usize - 1) as u16)
                    .unwrap_or(u16::MAX);
                e.passengers_allowed_to_fire = obj.passengers_allowed_to_fire;
                e.armed_riders_upgrade_weapon_set = obj.armed_riders_upgrade_weapon_set;
                e.weapon_set_player_upgrade = obj.weapon_set_player_upgrade;
                e.second_life = obj.armor_set_second_life;
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
                e.weapon_fire_status = obj.weapon_fire_status as u8;
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
                e.weapon_bonus_fanaticism = obj.weapon_bonus_fanaticism;
                e.last_horde_refresh_frame = obj.last_horde_refresh_frame;
                e.horde_next_wake_frame = obj.horde_next_wake_frame;
                e.horde_wake_initialized = obj.horde_wake_initialized;

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
                e.hive_slaves_alive = [
                    obj.hive_slaves[0].alive,
                    obj.hive_slaves[1].alive,
                    obj.hive_slaves[2].alive,
                ];
                e.hive_slaves_hp = [
                    obj.hive_slaves[0].hp,
                    obj.hive_slaves[1].hp,
                    obj.hive_slaves[2].hp,
                ];
                if let Some(ce) = logic.host_money_crates.get(oid) {
                    e.money_crate = true;
                    e.money_crate_expires_frame = ce.expires_frame;
                } else {
                    e.money_crate = false;
                    e.money_crate_expires_frame = 0;
                }
                if let Some(fs) = obj.fire_spread.as_ref() {
                    e.fire_spread_active = fs.active;
                    e.fire_spread_state = match fs.state {
                        crate::game_logic::host_fire_spread::HostFlammableState::Normal => 0,
                        crate::game_logic::host_fire_spread::HostFlammableState::Aflame => 1,
                        crate::game_logic::host_fire_spread::HostFlammableState::Burned => 2,
                    };
                    e.fire_spread_aflame_end_frame = fs.aflame_end_frame;
                    e.fire_spread_burned_end_frame = fs.burned_end_frame;
                    e.fire_spread_next_spread_frame = fs.next_spread_frame;
                    e.fire_spread_min_delay = fs.min_spread_delay;
                    e.fire_spread_max_delay = fs.max_spread_delay;
                    e.fire_spread_try_range = fs.spread_try_range;
                    e.fire_spread_aflame_duration = fs.aflame_duration;
                    e.fire_spread_burned_delay = fs.burned_delay;
                    e.fire_spread_enabled = fs.spread_enabled;
                    e.fire_spread_flame_damage_accum = fs.flame_damage_accum;
                    e.fire_spread_flame_damage_limit = fs.flame_damage_limit;
                } else {
                    e.fire_spread_active = false;
                    e.fire_spread_state = 0;
                }
                {
                    use crate::game_logic::host_oil_derrick::is_oil_derrick_template;
                    use crate::game_logic::is_black_market_template;
                    let name = obj.template_name.as_str();
                    let is_bm = is_black_market_template(name)
                        || name.to_ascii_lowercase().contains("blackmarket");
                    let is_fake = name.to_ascii_lowercase().contains("fake");
                    if is_bm && !is_fake {
                        e.black_market_building = true;
                        e.black_market_next_deposit_frame =
                            logic.black_markets.peek_next_deposit(oid).unwrap_or(0);
                    } else {
                        e.black_market_building = false;
                        e.black_market_next_deposit_frame = 0;
                    }
                    if is_oil_derrick_template(name) {
                        e.oil_derrick_building = true;
                        e.oil_derrick_next_deposit_frame =
                            logic.oil_derricks.peek_next_deposit(oid).unwrap_or(0);
                    } else {
                        e.oil_derrick_building = false;
                        e.oil_derrick_next_deposit_frame = 0;
                    }
                }
                {
                    let metadata = obj.thing.template.hack_internet_ai_update;
                    if let Some(metadata) = metadata {
                        // `InternetHackContain::onContaining` starts the AI
                        // command.  A faction-building KindOf or an
                        // InternetCenter-looking basename is not enough.
                        let in_ic = obj
                            .contained_by
                            .and_then(|cid| logic.objects.get(&cid))
                            .is_some_and(|container| {
                                container.is_alive()
                                    && container.is_constructed()
                                    && !container.status.under_construction
                                    && !container.status.sold
                                    && container.thing.template.contain_module.kind
                                        == crate::game_logic::ContainModuleKind::InternetHack
                                    && container.thing.template.contain_module.admission
                                        == crate::game_logic::ContainAdmission::MoneyHackerOnly
                                    && container.contained_units().contains(&oid)
                                    && logic.normal_enter_controller_matches(obj, container)
                            });
                        e.hacker_unit = true;
                        // During a coupled tick the GameWorld is the income
                        // scheduler.  Preserve its first contained schedule
                        // across host remirrors until the first writeback
                        // records the authoritative active state.
                        // C++ HackInternetAIUpdate::aiDoCommand
                        // (HackInternetAIUpdate.cpp:105) PACKING on evacuate/exit:
                        // leftover registry state must not remirror as field
                        // hacking after leaving InternetHackContain.
                        let was_in_ic = e.hacker_in_internet_center;
                        e.hacker_hacking = if in_ic {
                            true
                        } else if was_in_ic {
                            false
                        } else {
                            logic.hacker_income.is_hacking(oid)
                        };
                        if !e.hacker_hacking {
                            e.hacker_next_deposit_frame = 0;
                        } else if let Some(next) = logic.hacker_income.peek_next_deposit(oid) {
                            e.hacker_next_deposit_frame = next;
                        } else if e.hacker_next_deposit_frame == 0 {
                            // C++ UNPACKING then CashUpdateDelay before first ping.
                            let delay = if in_ic {
                                metadata.cash_update_delay_fast_frames
                            } else {
                                metadata.cash_update_delay_frames
                            };
                            e.hacker_next_deposit_frame = logic
                                .frame
                                .saturating_add(metadata.unpack_time_frames)
                                .saturating_add(delay)
                                .saturating_add(1);
                        }
                        e.hacker_in_internet_center = in_ic;
                        e.hacker_cash_update_delay_frames = metadata.cash_update_delay_frames;
                        e.hacker_cash_update_delay_fast_frames =
                            metadata.cash_update_delay_fast_frames;
                        e.hacker_regular_cash_amount = metadata.regular_cash_amount;
                        e.hacker_veteran_cash_amount = metadata.veteran_cash_amount;
                        e.hacker_elite_cash_amount = metadata.elite_cash_amount;
                        e.hacker_heroic_cash_amount = metadata.heroic_cash_amount;
                        e.hacker_xp_per_cash_update = metadata.xp_per_cash_update;
                    } else {
                        e.hacker_unit = false;
                        e.hacker_hacking = false;
                        e.hacker_in_internet_center = false;
                        e.hacker_next_deposit_frame = 0;
                        e.hacker_cash_update_delay_frames = 0;
                        e.hacker_cash_update_delay_fast_frames = 0;
                        e.hacker_regular_cash_amount = 0;
                        e.hacker_veteran_cash_amount = 0;
                        e.hacker_elite_cash_amount = 0;
                        e.hacker_heroic_cash_amount = 0;
                        e.hacker_xp_per_cash_update = 0.0;
                    }
                }
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

        let mapped: Vec<ObjectId> = self.host_to_entity.keys().copied().map(ObjectId).collect();
        for oid in mapped {
            let Some(obj) = logic.host_objects().get(&oid) else {
                continue;
            };
            let Some(&eid) = self.host_to_entity.get(&oid.0) else {
                continue;
            };
            self.sync_weapon_slots_from_host(eid, obj);
        }

        let (map_min, map_max) = logic.world_bounds();
        self.map_min_x = map_min.x;
        self.map_min_z = map_min.z;
        self.map_max_x = map_max.x;
        self.map_max_z = map_max.z;
        // Wave 792: mirror A10 pending missile drops for GW sole-tick.
        self.a10_pending_drops = logic.a10_strike_flight_reg.pending_drops.clone();
        self.artillery_pending_drops = logic.artillery_barrage_flight_reg.pending_drops.clone();
        self.carpet_pending_drops = logic.carpet_bomb_flight_reg.pending_drops.clone();

        // Align frame.
        let target = logic.get_frame() as u64;
        self.world.set_frame(target);
    }

    pub(super) fn spawn_mapped(
        &mut self,
        host: ObjectId,
        obj: &crate::game_logic::Object,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) {
        let eid = self.world.spawn_entity(
            TemplateRef::new(obj.template_name.clone()),
            owner,
            transform,
            health,
        );
        self.host_to_entity.insert(host.0, eid);
        self.entity_to_host.insert(eid.get(), host.0);
        if crate::gameworld_shadow::gameworld_entity_modules_enabled() {
            let spec = entity_module_spec_from_host(obj);
            let _ = self.world.install_entity_modules(eid, &spec);
        }
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
            e.production_paused = false;
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
            e.weapon_bonus_fanaticism = false;
            e.last_horde_refresh_frame = 0;
            e.horde_next_wake_frame = 0;
            e.horde_wake_initialized = false;

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

    pub(super) fn copy_host_player_residual(
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

    pub(super) fn host_player_science_and_upgrades(
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

    pub(super) fn sync_players(&mut self, logic: &GameLogic) {
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
        self.sync_horde_player_rel(logic);
    }

    /// Reverse map GameWorld owner → host Team (for TransferOwner writeback).
    pub(super) fn host_team_for_gw_owner(
        &self,
        logic: &GameLogic,
        owner: Option<PlayerId>,
    ) -> Option<Team> {
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

    /// Reverse-map a GameWorld owner to its host player identity.
    pub(super) fn host_player_for_gw_owner(&self, owner: Option<PlayerId>) -> Option<u32> {
        let owner = owner?;
        self.host_player_to_gw
            .iter()
            .find_map(|(&host_player_id, &gw_player_id)| {
                (gw_player_id == owner).then_some(host_player_id)
            })
    }

    /// Snapshot host `Player::getRelationship` facts used by GW AlliesOnly horde.
    pub(super) fn sync_horde_player_rel(&mut self, logic: &GameLogic) {
        self.horde_player_rel.clear();
        for (&id, p) in logic.get_players() {
            self.horde_player_rel.insert(
                id,
                HordePlayerRel {
                    alliance_team: p.alliance_team,
                    is_alive: p.is_alive,
                    map_relations: p.map_side.relations.clone(),
                },
            );
        }
    }

    /// C++ `Player::getRelationship` / `GameLogic::player_relationship_from_map`.
    /// Same controlling player is always ALLIES (even before the roster snapshot).
    fn horde_player_relationship(
        &self,
        source_player_id: u32,
        target_player_id: u32,
    ) -> gamelogic::common::Relationship {
        use gamelogic::common::Relationship;
        if source_player_id == target_player_id {
            return Relationship::Allies;
        }
        let Some(source) = self.horde_player_rel.get(&source_player_id) else {
            return Relationship::Neutral;
        };
        let Some(target) = self.horde_player_rel.get(&target_player_id) else {
            return Relationship::Neutral;
        };
        if !source.is_alive || !target.is_alive {
            return Relationship::Neutral;
        }
        if let Some(rel) = source.map_relations.get(&target_player_id) {
            return *rel;
        }
        if source.alliance_team >= 0 && source.alliance_team == target.alliance_team {
            Relationship::Allies
        } else if source.alliance_team >= 0 && target.alliance_team >= 0 {
            Relationship::Enemies
        } else {
            Relationship::Neutral
        }
    }

    /// C++ `PartitionFilterHordeMember::allow` AlliesOnly (`HordeUpdate.cpp:77-79`).
    pub(super) fn horde_allies_only(
        &self,
        a_owner: Option<u32>,
        a_team: u8,
        b_owner: Option<u32>,
        b_team: u8,
    ) -> bool {
        use gamelogic::common::Relationship;
        match (a_owner, b_owner) {
            (Some(a), Some(b)) => self.horde_player_relationship(a, b) == Relationship::Allies,
            (None, None) => a_team == b_team,
            _ => false,
        }
    }

    /// Resolve a host object through its explicit controlling player. Objects
    /// without a player remain neutral in GameWorld; choosing the first player
    /// with the same faction breaks USA-vs-USA skirmishes.
    pub(super) fn owner_for_host_object(
        &self,
        _logic: &GameLogic,
        object: &crate::game_logic::Object,
    ) -> Option<PlayerId> {
        object
            .owner_player_id
            .and_then(|player_id| self.host_player_to_gw.get(&player_id).copied())
    }
}

fn entity_module_spec_from_host(
    obj: &crate::game_logic::Object,
) -> gamelogic::world::EntityModuleInstallSpec {
    let template = obj.thing.get_template();
    let mut template_module_tags = Vec::new();
    if template.garrison_contain_max.is_some() {
        template_module_tags.push("GarrisonContain".to_string());
    }
    if obj.building_data.is_some() {
        template_module_tags.push("ProductionUpdate".to_string());
    }
    gamelogic::world::EntityModuleInstallSpec {
        template_module_tags,
        inactive_body: false,
        shrubbery: false,
        can_be_repulsed: obj
            .thing
            .is_kind_of(crate::game_logic::KindOf::CanBeRepulsed),
        has_weapons: obj.weapon.is_some(),
    }
}

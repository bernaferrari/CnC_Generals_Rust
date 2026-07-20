//! Entity storage and helpers mirroring the legacy object/thing system.
//!
//! The original engine routes almost everything through the global
//! `ObjectManager`.  Here we provide a modern, owned representation that still
//! uses familiar terminology (entity, template, owner) so porting code can stay
//! close to the C++ layout while benefiting from Rust's safety.

use crate::world::PlayerId;
use nalgebra::Point3;
use std::collections::HashMap;

/// Shadow residual of one host BuildingData::production_queue entry.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityProductionItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
    pub cost_supplies: u32,
    /// Host PRODUCTION_UPGRADE residual.
    pub is_upgrade: bool,
}

/// Identifier assigned to entities/things in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(u32);

impl EntityId {
    /// First valid entity identifier.
    pub const FIRST: EntityId = EntityId(1);

    /// Construct from a raw numeric id (shadow ID maps / diagnostics).
    pub fn from_raw(raw: u32) -> Self {
        EntityId(raw)
    }

    /// Raw numeric accessor.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Runtime description of a template. In the legacy engine this maps to
/// `ThingTemplate`.  We keep the fields intentionally small until the
/// higher-level systems are ported.
#[derive(Debug, Clone)]
pub struct TemplateRef {
    /// Stable name (matches C++ `ThingTemplate::GetName()`).
    pub name: String,
    /// Optional path to the definition file.
    pub source: Option<String>,
}

impl TemplateRef {
    /// Create a new template reference.
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            source: None,
        }
    }
}

/// Minimal spatial information for an entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// World-space position (X/Y/Z).
    pub position: Point3<f32>,
    /// Facing angle in radians.
    pub orientation: f32,
}

impl Transform {
    /// Create a new transform.
    pub fn new(position: [f32; 3], orientation: f32) -> Self {
        Self {
            position: Point3::from(position),
            orientation,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Point3::new(0.0, 0.0, 0.0),
            orientation: 0.0,
        }
    }
}

/// Core runtime data for an entity.
#[derive(Debug, Clone)]
pub struct Entity {
    /// Unique identifier.
    pub id: EntityId,
    /// Template metadata.
    pub template: TemplateRef,
    /// Owning player (if any).
    pub owner: Option<PlayerId>,
    /// Spatial state.
    pub transform: Transform,
    /// Current hitpoints.
    pub health: f32,
    /// Attack/command target (shadow of host Object::target).
    pub attack_target: Option<EntityId>,
    /// Move destination (shadow of host movement.target_position).
    pub move_target: Option<[f32; 3]>,
    /// Host Object::max_health residual.
    pub max_health: f32,
    /// C++ BodyDamageType residual (0 pristine .. 3 rubble).
    pub body_damage_state: u8,
    /// Host Object::selected residual (UI selection).
    pub selected: bool,
    /// Host Object::status.destroyed residual.
    pub destroyed: bool,
    /// Host Object::construction_percent residual (0..1).
    pub construction_percent: f32,
    /// Host Object::team residual as ordinal: 0 USA, 1 China, 2 GLA, 255 Neutral.
    pub team_ordinal: u8,
    /// Host Object::selection_radius residual.
    pub selection_radius: f32,
    /// Host Object::crusher_level residual.
    pub crusher_level: u8,
    /// Host Object::crushable_level residual.
    pub crushable_level: u8,
    /// Host Object::vision_range residual.
    pub vision_range: f32,
    /// Host Object::shroud_clearing_range residual.
    pub shroud_clearing_range: f32,
    /// Host Object::status.under_construction residual.
    pub under_construction: bool,
    /// Host Object::status.sold residual.
    pub sold: bool,
    /// Host Object::status.reconstructing residual.
    pub reconstructing: bool,
    /// Host Object::status.unselectable residual.
    pub unselectable: bool,
    /// Host Object::status.deployed residual.
    pub deployed: bool,
    /// Host Object::status.moving residual.
    pub moving: bool,
    /// Host Object::status.attacking residual.
    pub attacking: bool,
    /// Host Object::status.is_firing_weapon residual.
    pub is_firing_weapon: bool,
    /// Host Object::status.is_aiming_weapon residual.
    pub is_aiming_weapon: bool,
    /// Host Object::team_color residual (RGBA 0..1).
    pub team_color: [f32; 4],
    /// Host Object::power_provided residual.
    pub power_provided: i32,
    /// Host Object::power_consumed residual.
    pub power_consumed: i32,
    /// Host Object::object_type residual ordinal:
    /// 0 Infantry, 1 Vehicle, 2 Aircraft, 3 Building, 4 Supply, 5 Projectile, 6 Neutral.
    pub object_type_ordinal: u8,
    /// Host Object::max_transport residual (0 = heuristic default).
    pub max_transport: usize,
    /// Host Object::force_attack residual.
    pub force_attack: bool,
    /// Host Object::show_health_bar residual.
    pub show_health_bar: bool,
    /// Host Object::target_location residual (ground attack).
    pub target_location: Option<[f32; 3]>,
    /// Host Object::guard_position residual.
    pub guard_position: Option<[f32; 3]>,
    /// Host Object::guard_target residual as host object id (0 = none).
    pub guard_target_host: u32,
    /// Host Object::ai_state residual ordinal (see host_ai_state_ordinal).
    pub ai_state_ordinal: u8,
    /// Host Object::occupants.len residual (transport/garrison count).
    pub occupant_count: u16,
    /// Host Object::experience.current residual.
    pub experience_points: f32,
    /// Host Object::experience.level residual: 0 Rookie, 1 Veteran, 2 Elite, 3 Heroic.
    pub veterancy_ordinal: u8,
    /// Host Object::stored_resources.supplies residual.
    pub stored_supplies: u32,
    /// Host Object::status.stealthed residual.
    pub stealthed: bool,
    /// Host Object::status.detected residual.
    pub detected: bool,
    /// Host Object::status.using_ability residual.
    pub using_ability: bool,
    /// Host Object::status.airborne_target residual.
    pub airborne_target: bool,
    /// Host Object::status.disabled_underpowered residual.
    pub disabled_underpowered: bool,
    /// Host Object::status.disabled_unmanned residual.
    pub disabled_unmanned: bool,
    /// Host Object::status.disabled_hacked residual.
    pub disabled_hacked: bool,
    /// Host Object::status.disabled_emp residual.
    pub disabled_emp: bool,
    /// Host status.disabled_emp_until_frame residual.
    pub disabled_emp_until_frame: u32,
    /// Host status.disabled_hacked_until_frame residual.
    pub disabled_hacked_until_frame: u32,
    /// Host status.disabled_paralyzed_until_frame residual.
    pub disabled_paralyzed_until_frame: u32,
    /// Host Object::status.disabled_paralyzed residual.
    pub disabled_paralyzed: bool,
    /// Host Object::status.weapons_jammed residual.
    pub weapons_jammed: bool,
    /// Host Object::status.masked residual.
    pub masked: bool,
    /// Host Object::status.disguised residual.
    pub disguised: bool,
    /// Host Object::status.disabled_subdued residual.
    pub disabled_subdued: bool,
    /// Host Object::status.is_carbomb residual.
    pub is_carbomb: bool,
    /// Host Object::status.hijacked residual.
    pub hijacked: bool,
    /// Host Object::status.ignoring_stealth residual.
    pub ignoring_stealth: bool,
    /// Host Object::status.repulsor residual.
    pub repulsor: bool,
    /// Host Object::repulsor_until_frame residual (countdown frames; 0 = permanent/none).
    pub repulsor_until_frame: u32,
    /// Host Object::status.disabled_freefall residual.
    pub disabled_freefall: bool,
    /// Host Object::status.no_collisions residual.
    pub no_collisions: bool,
    /// Host Object::status.private_captured residual.
    pub private_captured: bool,
    /// Host Object::status.disguise_transitioning_to residual.
    pub disguise_transitioning_to: bool,
    /// Host Object::status.disguise_halfpoint_reached residual.
    pub disguise_halfpoint_reached: bool,
    /// Host Object::status.faerie_fire residual.
    pub faerie_fire: bool,
    /// Host Object::status.booby_trapped residual.
    pub booby_trapped: bool,
    /// Host Object::status.eject_invulnerable residual.
    pub eject_invulnerable: bool,
    /// Host Object::status.pilot_did_move_to_base residual.
    pub pilot_did_move_to_base: bool,
    /// Host Object::status.parachuting residual.
    pub parachuting: bool,
    /// Host Object::status.parachute_open residual.
    pub parachute_open: bool,
    /// Host Object::status.parachute_landing_override_set residual.
    pub parachute_landing_override_set: bool,
    /// Host Object::building_data present residual.
    pub is_building: bool,
    /// Host BuildingType residual ordinal (0..12; 255 = not a building).
    pub building_type_ordinal: u8,
    /// Host BuildingData::production_queue.len residual.
    pub production_queue_len: u8,
    /// Head of production queue progress residual (0..1-ish).
    pub production_progress: f32,
    /// Head of production queue template name residual (empty if none).
    pub production_template: String,
    /// Full production queue residual (capped).
    pub production_queue_items: Vec<EntityProductionItem>,
    /// Host BuildingData::exit_delay_remaining residual (seconds).
    pub exit_delay_remaining: f32,
    /// Host BuildingData::rally_point residual.
    pub rally_point: Option<[f32; 3]>,
    /// Host BuildingData::garrisoned_units.len residual.
    pub garrison_count: u16,
    /// Host BuildingData::max_garrison residual.
    pub max_garrison: u16,
    /// Host Object::weapon present residual.
    pub has_weapon: bool,
    /// Host Weapon::damage residual.
    pub weapon_damage: f32,
    /// Host Weapon::range residual.
    pub weapon_range: f32,
    /// Host Weapon::min_range residual.
    pub weapon_min_range: f32,
    /// Host Weapon::reload_time residual (seconds).
    pub weapon_reload_time: f32,
    /// Host Weapon::last_fire_time residual (seconds, sim clock).
    pub weapon_last_fire_time: f32,
    /// Host Weapon::clip_size residual (0 = unlimited).
    pub weapon_clip_size: u32,
    /// Host Weapon::clip_reload_time residual (seconds; 0 = use reload_time).
    pub weapon_clip_reload_time: f32,
    /// Host Weapon::ammo residual (`u32::MAX` = unlimited/None).
    pub weapon_ammo: u32,
    /// Host Weapon::can_target_air residual.
    pub weapon_can_target_air: bool,
    /// Host Weapon::can_target_ground residual.
    pub weapon_can_target_ground: bool,
    /// Host Weapon::projectile_speed residual.
    pub weapon_projectile_speed: f32,
    /// Host secondary_weapon present residual.
    pub has_secondary_weapon: bool,
    /// Host Movement::max_speed residual.
    pub move_max_speed: f32,
    /// Host Movement::velocity residual.
    pub velocity: [f32; 3],
    /// Host Movement::path.len residual.
    pub path_len: u16,
    /// Host Movement::current_path_index residual.
    pub path_index: u16,
    /// Host Movement::path waypoints residual (capped for presentation line pack).
    pub path_waypoints: Vec<[f32; 3]>,
    /// Host secondary weapon range residual.
    pub secondary_weapon_range: f32,
    /// Host secondary weapon damage residual.
    pub secondary_weapon_damage: f32,
    /// Host Object::name residual (display/script name; empty if unset).
    pub display_name: String,
    /// Host ThingTemplate model key residual (mesh resolve; empty if unset).
    pub model_key: String,
    /// Host Object::model_condition_bits residual.
    pub model_condition_bits: u128,
    /// Host ThingTemplate mesh scale residual (retail combat often 1.0).
    pub mesh_scale: f32,
    /// Host FOW visibility residual (alpha / explored / falloff).
    pub fow_visibility_alpha: f32,
    pub fow_is_explored: f32,
    pub fow_visibility_falloff: f32,
    /// Host terrain ground height residual at object XY.
    pub ground_height: f32,
    /// True when ground_height came from terrain sample (not default-0).
    pub ground_height_from_terrain: bool,
    /// Host Object::engine_object_id.is_some residual (bridged factory id).
    pub engine_bridged: bool,
    /// Host Object::overlord_bunker_capacity residual:
    /// `u16::MAX` = None (not overlord-style).
    pub overlord_bunker_capacity: u16,
    /// Host Object::passengers_allowed_to_fire residual.
    pub passengers_allowed_to_fire: bool,
    /// Host Object::armed_riders_upgrade_weapon_set residual.
    pub armed_riders_upgrade_weapon_set: bool,
    /// Host Object::weapon_set_player_upgrade residual.
    pub weapon_set_player_upgrade: bool,
    /// Host Object::is_battle_bus_transport residual.
    pub is_battle_bus_transport: bool,
    /// Host Object::is_technical_transport residual.
    pub is_technical_transport: bool,
    /// Host Object::is_combat_cycle_transport residual.
    pub is_combat_cycle_transport: bool,
    /// Host Object::combat_cycle_rider residual.
    pub combat_cycle_rider: u8,
    /// Host Object::is_tunnel_network residual.
    pub is_tunnel_network: bool,
    /// Host Object::is_combat_chinook_transport residual.
    pub is_combat_chinook_transport: bool,
    /// Host Object::contained_by residual as host object id (0 = free).
    pub contained_by_host: u32,
    /// Host building_data.garrisoned_units / occupants host ids residual (capped).
    pub garrisoned_host_ids: Vec<u32>,
    /// Host ThingTemplate kind_of residual as bitset (presentation ORDER bits).
    pub kind_of_bits: u32,
    /// Host Object::cheer_timer residual.
    pub cheer_timer: f32,
    /// Host Object::overcharge_enabled residual.
    pub overcharge_enabled: bool,
    /// Host Object::active_weapon_slot residual.
    pub active_weapon_slot: u8,
    /// Host Object::guard_radius residual.
    pub guard_radius: f32,
    /// Host Object::applied_upgrades.len residual.
    pub applied_upgrade_count: u16,
    /// Host Object::applied_upgrades name residual (capped, sorted for determinism).
    pub applied_upgrade_names: Vec<String>,
    /// Host Object::special_power_ready residual.
    pub special_power_ready: bool,
    /// Host Object::special_power_cooldown residual (full duration seconds).
    pub special_power_cooldown: f32,
    /// Host Object::special_power_cooldown_remaining residual.
    pub special_power_cooldown_remaining: f32,
    /// Host Object::is_detector residual.
    pub is_detector: bool,
    /// Host Object::detection_range residual.
    pub detection_range: f32,
    /// Host Object::detection_rate_frames residual.
    pub detection_rate_frames: u32,
    /// Host Object::stealth_breaks_on_attack residual.
    pub stealth_breaks_on_attack: bool,
    /// Host Object::stealth_breaks_on_move residual.
    pub stealth_breaks_on_move: bool,
    /// Host Object::innate_stealth residual.
    pub innate_stealth: bool,
    /// Host weapon-bonus flags residual.
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,
    pub weapon_bonus_horde: bool,
    pub weapon_bonus_nationalism: bool,
    pub weapon_bonus_frenzy: bool,
    pub weapon_bonus_frenzy_level: u8,
    pub weapon_bonus_battle_plan_bombardment: bool,
    pub weapon_bonus_battle_plan_hold_the_line: bool,
    pub weapon_bonus_battle_plan_search_and_destroy: bool,
    /// Host continuous-fire residual.
    pub continuous_fire_level: u8,
    pub continuous_fire_consecutive: u16,
    /// Host faerie_fire_until_frame residual.
    pub faerie_fire_until_frame: u32,
    /// Extra transport-kind markers.
    pub is_humvee_transport: bool,
    pub is_listening_outpost_transport: bool,
    pub is_troop_crawler_transport: bool,
    pub is_helix_transport: bool,
    pub has_overlord_gattling_addon: bool,
    pub has_overlord_propaganda_addon: bool,
    /// Host demo/hive residual.
    pub demo_suicided_detonating: bool,
    pub hive_slave_count: u8,
    pub hive_slave_hp: f32,
    /// Host turret residual.
    pub turret_angle_deg: f32,
    pub turret_pitch_deg: f32,
    pub turret_idle_scanning: bool,
    pub turret_holding: bool,
    /// Host AI attitude residual (-1..n as host i8).
    pub ai_attitude: i8,
    /// Host last_damage_source as host object id (0 = none).
    pub last_damage_source_host: u32,
    /// Host command_set_override residual (empty = none).
    pub command_set_override: String,
    /// Host disguise residual (empty template = none).
    pub disguise_as_template: String,
    /// Host disguise team ordinal (255 = none).
    pub disguise_as_team_ordinal: u8,
    /// Host vision_spied_mask residual.
    pub vision_spied_mask: u32,
    /// Host camo residual.
    pub camo_friendly_opacity: f32,
    pub camo_stealth_look: u8,
    /// Host mine residual present flag.
    pub has_mine_data: bool,
    /// Host weapon_bonus_frenzy_until_frame residual.
    pub weapon_bonus_frenzy_until_frame: u32,
    /// Host continuous_fire_coast_until_frame residual.
    pub continuous_fire_coast_until_frame: u32,
    /// Host battle_plan_sight_scalar_applied residual (1.0 = none).
    pub battle_plan_sight_scalar_applied: f32,
}

impl Entity {
    /// Convenience accessor for the template name.
    pub fn template_name(&self) -> &str {
        &self.template.name
    }
}

/// Store responsible for allocating and tracking entities.
#[derive(Debug, Default, Clone)]
pub struct EntityStore {
    next_id: u32,
    alive: HashMap<EntityId, Entity>,
}

impl EntityStore {
    /// Remove every entity and reset id allocation.
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Create a new store.
    pub fn new() -> Self {
        Self {
            next_id: EntityId::FIRST.get(),
            alive: HashMap::new(),
        }
    }

    /// Number of living entities.
    pub fn len(&self) -> usize {
        self.alive.len()
    }

    /// Returns true if no entities are alive.
    pub fn is_empty(&self) -> bool {
        self.alive.is_empty()
    }

    /// Iterate over entities.
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.alive.values()
    }

    /// Living entity ids (stable snapshot for mid-frame step loops).
    pub fn ids(&self) -> Vec<EntityId> {
        self.alive.keys().copied().collect()
    }

    /// Get a specific entity.
    pub fn get(&self, id: EntityId) -> Option<&Entity> {
        self.alive.get(&id)
    }

    /// Mutable accessor.
    pub fn get_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.alive.get_mut(&id)
    }

    /// Spawn a new entity using the provided template and initial state.
    pub fn spawn(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> EntityId {
        let id = EntityId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1).max(EntityId::FIRST.get());

        let entity = Entity {
            id,
            template,
            owner,
            transform,
            health,
            attack_target: None,
            move_target: None,

            max_health: health.max(1.0),
            body_damage_state: 0,
            selected: false,
            destroyed: false,
            construction_percent: 1.0,
            team_ordinal: 255,
            selection_radius: 5.0,
            crusher_level: 0,
            crushable_level: 0,
            vision_range: 0.0,
            shroud_clearing_range: 0.0,
            under_construction: false,
            sold: false,
            reconstructing: false,
            unselectable: false,
            deployed: false,
            moving: false,
            attacking: false,
            is_firing_weapon: false,
            is_aiming_weapon: false,
            team_color: [1.0, 1.0, 1.0, 1.0],
            power_provided: 0,
            power_consumed: 0,
            object_type_ordinal: 6,
            max_transport: 0,
            force_attack: false,
            show_health_bar: true,
            target_location: None,
            guard_position: None,
            guard_target_host: 0,
            ai_state_ordinal: 0,
            occupant_count: 0,
            experience_points: 0.0,
            veterancy_ordinal: 0,
            stored_supplies: 0,
            stealthed: false,
            detected: false,
            using_ability: false,
            airborne_target: false,
            disabled_underpowered: false,
            disabled_unmanned: false,
            disabled_hacked: false,
            disabled_emp: false,
            disabled_emp_until_frame: 0,
            disabled_hacked_until_frame: 0,
            disabled_paralyzed_until_frame: 0,
            disabled_paralyzed: false,
            weapons_jammed: false,
            masked: false,
            disguised: false,
            disabled_subdued: false,
            is_carbomb: false,
            hijacked: false,
            ignoring_stealth: false,
            repulsor: false,
            repulsor_until_frame: 0,
            disabled_freefall: false,
            no_collisions: false,
            private_captured: false,
            disguise_transitioning_to: false,
            disguise_halfpoint_reached: false,
            faerie_fire: false,
            booby_trapped: false,
            eject_invulnerable: false,
            pilot_did_move_to_base: false,
            parachuting: false,
            parachute_open: false,
            parachute_landing_override_set: false,
            is_building: false,
            building_type_ordinal: 255,
            production_queue_len: 0,
            production_progress: 0.0,
            production_template: String::new(),
            production_queue_items: Vec::new(),
            exit_delay_remaining: 0.0,
            rally_point: None,
            garrison_count: 0,
            max_garrison: 0,
            has_weapon: false,
            weapon_damage: 0.0,
            weapon_range: 0.0,
            weapon_min_range: 0.0,

            weapon_reload_time: 0.0,
            weapon_last_fire_time: 0.0,
            weapon_clip_size: 0,
            weapon_clip_reload_time: 0.0,
            weapon_ammo: u32::MAX,
            weapon_can_target_air: false,
            weapon_can_target_ground: true,
            weapon_projectile_speed: 0.0,
            has_secondary_weapon: false,
            move_max_speed: 0.0,
            velocity: [0.0; 3],
            path_len: 0,
            path_index: 0,
            path_waypoints: Vec::new(),
            secondary_weapon_range: 0.0,
            secondary_weapon_damage: 0.0,
            display_name: String::new(),
            model_key: String::new(),
            model_condition_bits: 0,
            mesh_scale: 1.0,
            fow_visibility_alpha: 1.0,
            fow_is_explored: 1.0,
            fow_visibility_falloff: 0.0,
            ground_height: 0.0,
            ground_height_from_terrain: false,
            engine_bridged: false,
            overlord_bunker_capacity: u16::MAX,
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            is_battle_bus_transport: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            is_tunnel_network: false,
            is_combat_chinook_transport: false,
            contained_by_host: 0,
            garrisoned_host_ids: Vec::new(),
            kind_of_bits: 0,
            cheer_timer: 0.0,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            guard_radius: 0.0,
            applied_upgrade_count: 0,
            applied_upgrade_names: Vec::new(),
            special_power_ready: false,
            special_power_cooldown: 0.0,
            special_power_cooldown_remaining: 0.0,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            stealth_breaks_on_attack: false,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            continuous_fire_level: 0,
            continuous_fire_consecutive: 0,
            faerie_fire_until_frame: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_troop_crawler_transport: false,
            is_helix_transport: false,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            turret_angle_deg: 0.0,
            turret_pitch_deg: 0.0,
            turret_idle_scanning: false,
            turret_holding: false,
            ai_attitude: 0,
            last_damage_source_host: 0,
            command_set_override: String::new(),
            disguise_as_template: String::new(),
            disguise_as_team_ordinal: 255,
            vision_spied_mask: 0,
            camo_friendly_opacity: 1.0,
            camo_stealth_look: 0,
            has_mine_data: false,
            weapon_bonus_frenzy_until_frame: 0,
            continuous_fire_coast_until_frame: 0,
            battle_plan_sight_scalar_applied: 1.0,
        };

        self.alive.insert(id, entity);
        id
    }

    /// Remove an entity. Returns the removed entity if it was alive.
    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        self.alive.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_and_remove_entity() {
        let mut store = EntityStore::new();
        assert_eq!(store.len(), 0);

        let id = store.spawn(
            TemplateRef::new("GLAInfantryRebel"),
            Some(PlayerId::FIRST),
            Transform::new([10.0, 5.0, 0.0], 1.57),
            100.0,
        );

        let entity = store.get(id).expect("entity spawned");
        assert_eq!(entity.template_name(), "GLAInfantryRebel");
        assert_eq!(entity.owner, Some(PlayerId::FIRST));

        let removed = store.remove(id).expect("removed entity");
        assert_eq!(removed.id, id);
        assert!(store.is_empty());
    }
}

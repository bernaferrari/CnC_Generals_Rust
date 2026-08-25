//! Entity storage and helpers mirroring the legacy object/thing system.
//!
//! The original engine routes almost everything through the global
//! `ObjectManager`.  Here we provide a modern, owned representation that still
//! uses familiar terminology (entity, template, owner) so porting code can stay
//! close to the C++ layout while benefiting from Rust's safety.

use crate::world::PlayerId;
use nalgebra::Point3;
use std::collections::HashMap;

#[path = "../entity_lifecycle.rs"]
pub mod entity_lifecycle;
pub use entity_lifecycle::{
    ENTITY_LIFECYCLE_ENVELOPE_VERSION, EntityLifecycleCodecError, EntityLifecycleEnvelope,
    EntityModuleState,
};

/// One installed module participant (tag + crate handle name).
/// Lifecycle/serialization only — modules do not tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityModuleRecord {
    pub tag: String,
    pub handle: String,
}

/// Ordered module-graph scaffolding stored on the Entity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityInstalledModules {
    pub records: Vec<EntityModuleRecord>,
    pub on_created: bool,
    pub on_delete_order: Vec<String>,
    pub live_instances: usize,
}

/// Shadow residual of one host BuildingData::production_queue entry.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityProductionItem {
    pub template_name: String,
    pub progress: f32,
    pub total_time: f32,
    /// C++ ProductionEntry::m_framesUnderConstruction.  Production advances
    /// once per logic update, not by repeatedly accumulating a floating-point
    /// duration.  Keep this in the GameWorld mirror so sole-tick authority
    /// cannot disagree with the host queue after save/writeback.
    pub construction_frames: u32,
    pub cost_supplies: u32,
    /// Host PRODUCTION_UPGRADE residual.
    pub is_upgrade: bool,
    /// C++ ProductionEntry::m_productionQuantityTotal residual (Wave 463).
    pub quantity_total: u32,
    /// C++ ProductionEntry::m_productionQuantityProduced residual (Wave 463).
    pub quantity_produced: u32,
}

/// C++ `LOGICFRAMES_PER_SECOND`, used by `ProductionUpdate::update` and both
/// `ThingTemplate::calcTimeToBuild` / `UpgradeTemplate::calcTimeToBuild`.
pub const PRODUCTION_LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

/// C++ `ProductionUpdate` recomputes this integer threshold on every update.
///
/// Units use `ThingTemplate::calcTimeToBuild`, which divides the integer base
/// frame count by the current low-power penalty rate and truncates.  Upgrades
/// use `UpgradeTemplate::calcTimeToBuild`, whose retail implementation ignores
/// power.  A zero authored duration remains immediately complete; positive
/// sub-frame durations use one update so the queue cannot divide by zero.
pub fn production_total_logic_frames(total_time: f32, is_upgrade: bool, power_factor: f32) -> u32 {
    if !total_time.is_finite() || total_time <= 0.0 {
        return 0;
    }

    // C++ assigns the product into an Int before applying the later modifiers.
    let base_frames = (total_time * PRODUCTION_LOGIC_FRAMES_PER_SECOND)
        .trunc()
        .clamp(1.0, u32::MAX as f32) as u32;
    if is_upgrade {
        return base_frames;
    }

    // ThingTemplate::calcTimeToBuild clamps the penalty rate to at least .01
    // and C++ `Int /= Real` truncates the resulting threshold.
    let rate = power_factor.clamp(0.01, 1.0);
    ((base_frames as f32 / rate)
        .trunc()
        .clamp(1.0, u32::MAX as f32)) as u32
}

/// Recover an integer counter from old float-only snapshots exactly once.
/// New queue entries have both zero progress and zero construction frames, so
/// this cannot turn a newly queued item into a partially complete one.
pub fn production_frames_from_legacy_progress(
    progress: f32,
    total_time: f32,
    total_frames: u32,
) -> u32 {
    if total_frames == 0 || !progress.is_finite() || !total_time.is_finite() || total_time <= 0.0 {
        return 0;
    }
    if progress >= total_time {
        return total_frames;
    }
    ((progress.max(0.0) / total_time).clamp(0.0, 1.0) * total_frames as f32).floor() as u32
}

/// Presentation/UI seconds retained for compatibility with existing queue
/// consumers.  Authority always uses the integer counter and threshold above.
pub fn production_progress_from_logic_frames(
    construction_frames: u32,
    total_frames: u32,
    total_time: f32,
) -> f32 {
    if !total_time.is_finite() || total_time <= 0.0 || total_frames == 0 {
        return total_time.max(0.0);
    }
    total_time * (construction_frames.min(total_frames) as f32 / total_frames as f32)
}

impl EntityProductionItem {
    pub fn total_construction_frames(&self, power_factor: f32) -> u32 {
        production_total_logic_frames(self.total_time, self.is_upgrade, power_factor)
    }

    pub fn migrate_legacy_construction_frames(&mut self, power_factor: f32) {
        if self.construction_frames == 0 && self.progress > 0.0 {
            self.construction_frames = production_frames_from_legacy_progress(
                self.progress,
                self.total_time,
                self.total_construction_frames(power_factor),
            );
        }
    }

    pub fn advance_one_construction_frame(&mut self, power_factor: f32) {
        self.migrate_legacy_construction_frames(power_factor);
        let total_frames = self.total_construction_frames(power_factor);
        if total_frames == 0 {
            self.progress = self.total_time.max(0.0);
            return;
        }
        self.construction_frames = self.construction_frames.saturating_add(1);
        self.progress = production_progress_from_logic_frames(
            self.construction_frames,
            total_frames,
            self.total_time,
        );
    }

    pub fn is_complete_at_power(&mut self, power_factor: f32) -> bool {
        self.migrate_legacy_construction_frames(power_factor);
        let total_frames = self.total_construction_frames(power_factor);
        total_frames == 0 || self.construction_frames >= total_frames
    }
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
    /// Logic frame when destroyed was set. Entity stays resolvable until process_destroy_list.
    pub destroyed_at_frame: u32,
    /// C++ DeathType residual ordinal (HostDeathType).
    pub death_type: u8,
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
    /// Host Object::front_crushed residual.
    pub front_crushed: bool,
    /// Host USER_1 model residual.
    pub user_1: bool,
    /// Host USER_2 model residual.
    pub user_2: bool,
    /// Host Object::back_crushed residual.
    pub back_crushed: bool,
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
    /// Host Object::rebuild_ready_frame residual.
    /// Host Object::is_rebuild_hole residual.
    pub is_rebuild_hole: bool,
    /// Host Object::rebuild_template_name residual.
    pub rebuild_template_name: String,
    pub rebuild_ready_frame: u32,
    /// Host Object::rebuild_spawner_id residual.
    pub rebuild_spawner_id: Option<u32>,
    /// Host Object::rebuild_worker_id residual.
    pub rebuild_worker_id: Option<u32>,
    /// Host Object::rebuild_reconstructing_id residual.
    pub rebuild_reconstructing_id: Option<u32>,
    /// Host Object::producer_id residual.
    pub producer_id: Option<u32>,
    /// Host Object::construction_complete_clear_frame residual.
    pub construction_complete_clear_frame: u32,
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
    /// Host Object::status.disabled_script_disabled residual.
    pub disabled_script_disabled: bool,
    /// Host Object::status.disabled_script_underpowered residual.
    pub disabled_script_underpowered: bool,
    /// Host Object::status.disabled_held residual.
    pub disabled_held: bool,
    /// Host Object::status.weapons_jammed residual.
    pub weapons_jammed: bool,
    /// Host Object::status.masked residual.
    pub masked: bool,
    /// C++ KINDOF_UNATTACKABLE carried separately from the compact 32-bit
    /// presentation KindOf bank.  It is an unconditional WeaponSet victim
    /// override, so the GameWorld-primary presentation/input path must retain
    /// it even though the legacy bank has no spare bit.
    pub unattackable: bool,
    /// Main host `DockKind` ordinal.  This is a compact presentation/shadow
    /// channel because the engine crate cannot depend on the Main host enum.
    pub dock_kind: u8,
    /// Main host C++ `KINDOF_CAPTURABLE` semantic, outside the legacy compact
    /// KindOf bank so the engine crate remains independent of host enums.
    pub capturable: bool,
    /// Main host C++ `KINDOF_IMMUNE_TO_CAPTURE` semantic.
    pub immune_to_capture: bool,
    /// Exact host `GarrisonContain` presence for capture legality.
    pub capture_garrisonable: bool,
    /// Main host CapturePowerKind ordinal (0 = none).
    pub capture_power: u8,
    /// Snapshot/authority readiness for that exact capture power.
    pub capture_power_ready: bool,
    /// Exact paired Object INI Hacker Disable Building capability.  Kept
    /// separate from capture and the compact KindOf bank so GameWorld cannot
    /// infer it from an infantry/template name.
    pub hacker_disable_building_capable: bool,
    /// Frozen source readiness for the paired Hacker Disable Building module.
    pub hacker_disable_building_ready: bool,
    /// Canonical parsed SpecialPowerTemplate name for a ready structure
    /// module.  Kept as text because GameLogic cannot depend on Main's host
    /// command enum; an empty value represents no ready source module.
    pub special_power_ready_template_name: String,
    /// Parsed SpecialPowerTemplate ID paired with the canonical name above.
    pub special_power_ready_template_id: u32,
    /// Host Object::status.disguised residual.
    pub disguised: bool,
    /// Host Object::status.disabled_subdued residual.
    pub disabled_subdued: bool,
    /// Host Object::subdual_damage residual.
    pub subdual_damage: f32,
    /// Host Object::subdual_heal_amount residual.
    pub subdual_heal_amount: f32,
    /// Host Object::subdual_heal_rate_frames residual.
    pub subdual_heal_rate_frames: u32,
    /// Host Object::subdual_heal_countdown residual.
    pub subdual_heal_countdown: u32,
    /// Host ObjectDefectionHelper residual: undetected defector active.
    pub defection_undetected: bool,
    /// Host detection timer end frame (0 = none).
    pub defection_detection_end: u32,
    /// Host detection timer start frame.
    pub defection_detection_start: u32,
    /// Host flash phase residual.
    pub defection_flash_phase: f32,
    /// Host doDefectorFX residual.
    pub defection_do_fx: bool,
    /// Presentation: selection flash this frame.
    pub defection_flash_this_frame: bool,
    /// Presentation: final white flash on timer expire.
    pub defection_final_white_flash: bool,
    /// Host Object::fire_sound_loop_until_frame residual (0 = none).
    pub fire_sound_loop_until_frame: u32,
    /// Host Object::fire_sound_loop_name residual (empty = none).
    pub fire_sound_loop_name: String,
    /// Host LifetimeUpdate expire_at_frame residual (0 = none).
    pub lifetime_expire_at_frame: u32,
    /// Host LifetimeUpdate active residual.
    pub lifetime_active: bool,
    /// Host PoisonedBehavior next DoT frame (0 = inactive).
    pub poison_damage_frame: u32,
    /// Host PoisonedBehavior overall stop frame (0 = inactive).
    pub poison_overall_stop_frame: u32,
    /// Host PoisonedBehavior retake damage amount.
    pub poison_damage_amount: f32,
    /// Host PoisonedBehavior death type ordinal residual.
    pub poison_death_type: u8,
    /// Host PoisonedBehavior tint residual.
    pub poison_tint: bool,
    /// Host ToppleUpdate state residual (0 upright, 1 falling, 2 down).
    pub topple_state: u8,
    pub topple_dir_x: f32,
    pub topple_dir_y: f32,
    pub topple_angular_velocity: f32,
    pub topple_angular_acceleration: f32,
    pub topple_angular_accumulation: f32,
    pub topple_options: u32,
    pub topple_kill_when_toppled: bool,
    pub topple_lean_radians: f32,
    /// True when entity has active topple residual data.
    pub topple_active: bool,
    /// Host HeightDieUpdate residual active.
    pub height_die_active: bool,
    pub height_die_target_hat: f32,
    pub height_die_only_when_descending: bool,
    pub height_die_earliest_frame: u32,
    pub height_die_last_height: f32,
    pub height_die_has_died: bool,
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
    /// Host Object::status.eject_invulnerable_until_frame residual (0 = none).
    pub eject_invulnerable_until_frame: u32,
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
    /// Wave 990: host BuildingData::production_paused residual.
    pub production_paused: bool,
    /// Host BuildingData::exit_delay_remaining residual (seconds).
    pub exit_delay_remaining: f32,
    /// Source-backed QueueProductionExitUpdate::m_currentDelay (logic frames).
    /// The seconds field above is only a presentation/legacy compatibility
    /// mirror once this state has been initialized.
    pub exit_delay_remaining_frames: u32,
    /// Source-backed QueueProductionExitUpdate::m_currentBurstCount.
    pub exit_burst_remaining: u32,
    /// Whether the frame/burst values are authoritative parsed Queue state.
    pub queue_exit_state_initialized: bool,
    /// Host Object::production_door_phase residual.
    pub production_door_phase: u8,
    /// Host Object::production_door_phase_end_frame residual.
    pub production_door_phase_end_frame: u32,
    /// Host Object::production_door_hold_open residual.
    pub production_door_hold_open: bool,
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
    /// Host combat fire-intent residual `last_fire_victim_host`.
    pub last_fire_victim_host: u32,
    /// Host combat fire-intent residual `last_fire_slot`.
    pub last_fire_slot: u8,
    /// Host combat fire-intent residual `last_fire_damage`.
    pub last_fire_damage: f32,
    /// Host combat fire-intent residual `last_fire_range`.
    pub last_fire_range: f32,
    /// Host combat fire-intent residual `last_fire_sim_time`.
    pub last_fire_sim_time: f32,
    /// Host combat fire-intent residual `last_fire_frame`.
    pub last_fire_frame: u32,
    /// Host combat fire-intent residual `fire_intent_count`.
    pub fire_intent_count: u32,
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
    /// Host Movement/Object::waiting_for_path residual.
    pub waiting_for_path: bool,
    /// Host Object::motive_frames_remaining residual.
    pub motive_frames_remaining: u32,
    /// Host Object::kill_when_resting_on_ground residual.
    pub kill_when_resting_on_ground: bool,
    /// Host Object::bounce_land_events residual.
    pub bounce_land_events: u32,
    /// Host Object::last_bounce_fall_dy residual.
    pub last_bounce_fall_dy: f32,
    /// Host Object::bounce_sound_name residual.
    pub bounce_sound_name: String,
    /// Host Object::last_bounce_volume residual.
    pub last_bounce_volume: f32,
    /// Host Object::bounce_audio_pending residual.
    pub bounce_audio_pending: u32,
    /// Host Object::allow_collide_force residual.
    pub allow_collide_force: bool,
    /// Host Object::last_collidee_id residual.
    pub last_collidee_id: Option<u32>,
    /// Host Object::ignore_collisions_with_id residual.
    pub ignore_collisions_with_id: Option<u32>,
    /// Host Object::physics_mass residual.
    pub physics_mass: f32,
    /// Host Object::physics_accel residual.
    pub physics_accel: [f32; 3],
    /// Host Object::forward_friction residual.
    pub forward_friction: f32,
    /// Host Object::lateral_friction residual.
    pub lateral_friction: f32,
    /// Host Object::z_friction residual.
    pub z_friction: f32,
    /// Host Object::can_path_through_units residual.
    pub can_path_through_units: bool,
    /// Host Object::ignore_collisions_until_frame residual.
    pub ignore_collisions_until_frame: u32,
    /// Host Object::is_panicking residual.
    pub is_panicking: bool,
    /// Host Object::move_away_frames residual.
    pub move_away_frames: u32,
    /// Host Object::aerodynamic_friction residual.
    pub aerodynamic_friction: f32,
    /// Host Object::extra_friction residual.
    pub extra_friction: f32,
    /// Host Object::apply_friction_2d_when_airborne residual.
    pub apply_friction_2d_when_airborne: bool,
    /// Host Object::center_of_mass_offset residual.
    pub center_of_mass_offset: f32,
    /// Host Object::pitch_roll_yaw_factor residual.
    pub pitch_roll_yaw_factor: f32,
    /// Host Object::move_away_destination residual.
    pub move_away_destination: Option<[f32; 3]>,
    /// Host Object::request_other_move_away_id residual.
    pub request_other_move_away_id: Option<u32>,
    /// Host Object::immune_to_falling_damage residual.
    pub immune_to_falling_damage: bool,
    /// Host Object::physics_current_overlap_id residual.
    pub physics_current_overlap_id: Option<u32>,
    /// Host Object::physics_previous_overlap_id residual.
    pub physics_previous_overlap_id: Option<u32>,
    /// Host Object::locomotor_surfaces residual.
    pub locomotor_surfaces: u32,
    /// Host Object::is_attack_path residual.
    pub is_attack_path: bool,
    /// Host locomotor residual `is_approach_path`.
    pub is_approach_path: bool,
    /// Host locomotor residual `on_invalid_movement_terrain`.
    pub on_invalid_movement_terrain: bool,
    /// Host locomotor residual `was_airborne_last_frame`.
    pub was_airborne_last_frame: bool,
    /// Host locomotor residual `can_move_backward`.
    pub can_move_backward: bool,
    /// Host locomotor residual `moving_backwards`.
    pub moving_backwards: bool,
    /// Host locomotor residual `no_slow_down_as_approaching_dest`.
    pub no_slow_down_as_approaching_dest: bool,
    /// Host locomotor residual `turn_pivot_offset`.
    pub turn_pivot_offset: f32,
    /// Host locomotor residual `wander_width_factor`.
    pub wander_width_factor: f32,
    /// Host locomotor residual `loco_apply_2d_friction_airborne`.
    pub loco_apply_2d_friction_airborne: bool,
    /// Host locomotor residual `allow_motive_force_while_airborne`.
    pub allow_motive_force_while_airborne: bool,
    /// Host locomotor residual `locomotor_works_when_dead`.
    pub locomotor_works_when_dead: bool,
    /// Host locomotor residual `loco_extra_2d_friction`.
    pub loco_extra_2d_friction: f32,
    /// Host locomotor residual `loco_preferred_height`.
    pub loco_preferred_height: f32,
    /// Host locomotor residual `loco_preferred_height_damping`.
    pub loco_preferred_height_damping: f32,
    /// Host locomotor residual `loco_appearance_ordinal`.
    pub loco_appearance_ordinal: u8,
    /// Host locomotor residual `loco_behavior_z_ordinal`.
    pub loco_behavior_z_ordinal: u8,
    /// Host locomotor residual `min_turn_speed`.
    pub min_turn_speed: f32,
    /// Host PhysicsTurningType residual (-1/0/1 as i8).
    pub physics_turning_ordinal: i8,
    /// Host Object::is_blocked_and_stuck residual.
    pub is_blocked_and_stuck: bool,
    /// Host Object::is_braking residual.
    pub is_braking: bool,
    /// Host Object::is_safe_path residual.
    pub is_safe_path: bool,
    /// Host Object::queue_for_path_frames residual.
    pub queue_for_path_frames: u32,
    /// Host Object::path_timestamp residual.
    pub path_timestamp: u32,
    /// Host Object::cur_max_blocked_speed residual.
    pub cur_max_blocked_speed: f32,
    /// Host Object::num_frames_blocked residual.
    pub num_frames_blocked: u32,
    /// Host Object::is_blocked residual.
    pub is_blocked: bool,
    /// Host Object::move_away_from_id residual.
    pub move_away_from_id: Option<u32>,
    /// Host Object::requested_victim_id residual.
    pub requested_victim_id: Option<u32>,
    /// Host AI request residual `requested_destination`.
    pub requested_destination: Option<[f32; 3]>,
    /// Host AI request residual `prev_victim_pos`.
    pub prev_victim_pos: Option<[f32; 3]>,
    /// Host AI request residual `crate_created_host`.
    pub crate_created_host: u32,
    /// Host AI request residual `guard_retaliate_victim_host`.
    pub guard_retaliate_victim_host: u32,
    /// Host AI request residual `guard_retaliate_anchor`.
    pub guard_retaliate_anchor: Option<[f32; 3]>,
    /// Host AI request residual `disguise_pending_template`.
    pub disguise_pending_template: String,
    /// Host AI request residual `disguise_pending_team_ordinal`.
    pub disguise_pending_team_ordinal: u8,
    /// Host AI request residual `weapon_crate_upgrade`.
    pub weapon_crate_upgrade: u8,
    /// Host AI request residual `armor_crate_upgrade`.
    pub armor_crate_upgrade: u8,
    /// C++ EnemyNearUpdate model residual.
    pub enemy_near: bool,
    /// Host EnemyNearUpdate module active (walls/props).
    pub enemy_near_active: bool,
    pub enemy_near_scan_delay: u32,
    pub enemy_near_scan_delay_time: u32,
    pub enemy_near_model: bool,
    pub enemy_near_vision_range: f32,
    /// Host ProneUpdate residual.
    pub prone_active: bool,
    pub prone_frames: i32,
    pub prone_damage_to_frames_ratio: f32,
    pub prone_model: bool,
    pub prone_no_attack: bool,
    /// Host FloatUpdate residual (boat sway / water snap).
    pub float_update_active: bool,
    pub float_update_enabled: bool,
    pub float_yaw: f32,
    pub float_pitch: f32,
    /// C++ ARMED model residual.
    pub armed: bool,
    /// Host AI request residual `selection_flash_remaining`.
    pub selection_flash_remaining: u32,
    /// Host Object::shock_stun_frames residual.
    pub shock_stun_frames: u32,
    /// C++ PowerPlantUpdate m_extended residual.
    pub power_plant_rods_extended: bool,
    /// Power plant rods done frame residual.
    pub power_plant_rods_done_frame: u32,
    /// Jet slow-death residual active.
    pub jet_slow_death_active: bool,
    pub jet_slow_death_started_on_ground: bool,
    pub jet_slow_death_hit_ground: bool,
    pub jet_slow_death_hit_ground_frame: u32,
    pub jet_slow_death_roll_rate: f32,
    pub jet_slow_death_roll_rate_delta: f32,
    pub jet_slow_death_fall_how_fast: f32,
    pub jet_slow_death_vertical_velocity: f32,
    pub jet_slow_death_roll_accum: f32,
    pub jet_slow_death_done: bool,
    /// Host HelicopterSlowDeathBehavior residual.
    pub heli_slow_death_active: bool,
    pub heli_slow_death_hit_ground: bool,
    pub heli_slow_death_hit_ground_frame: u32,
    pub heli_slow_death_activate_frame: u32,
    pub heli_slow_death_orbit_angle: f32,
    pub heli_slow_death_self_spin: f32,
    pub heli_slow_death_self_spin_dir: f32,
    pub heli_slow_death_frames_since_spin_update: u32,
    pub heli_slow_death_forward_speed: f32,
    pub heli_slow_death_vertical_velocity: f32,
    pub heli_slow_death_orientation_delta: f32,
    pub heli_slow_death_blade_flew_off: bool,
    pub heli_slow_death_done: bool,
    /// Host SlowDeathBehavior residual phase (0 Inactive..4 Done).
    pub slow_death_phase: u8,
    pub slow_death_begin_frame: u32,
    pub slow_death_sink_at_frame: u32,
    pub slow_death_destroy_at_frame: u32,
    pub slow_death_sink_rate_per_frame: f32,
    pub slow_death_sink_offset: f32,
    pub slow_death_destruction_altitude: f32,
    pub slow_death_fling_vx: f32,
    pub slow_death_fling_vz: f32,
    pub slow_death_fling_vy: f32,
    pub slow_death_fling_applied: bool,
    /// Host StructureCollapseUpdate residual state (0 Standing..3 Done).
    pub structure_collapse_state: u8,
    pub structure_collapse_start_frame: u32,
    pub structure_collapse_velocity: f32,
    pub structure_collapse_current_height: f32,
    pub structure_collapse_damping: f32,
    pub structure_collapse_max_shudder: f32,
    pub structure_collapse_building_height: f32,
    pub structure_collapse_shudder_x: f32,
    pub structure_collapse_shudder_z: f32,
    /// Host StructureToppleUpdate residual state (0 Standing..4 Done).
    pub structure_topple_state: u8,
    pub structure_topple_start_frame: u32,
    pub structure_topple_dir_x: f32,
    pub structure_topple_dir_y: f32,
    pub structure_topple_velocity: f32,
    pub structure_topple_accumulated_angle: f32,
    pub structure_topple_structural_integrity: f32,
    pub structure_topple_structural_decay: f32,
    pub structure_topple_done_frame: u32,
    pub structure_topple_lean_radians: f32,
    pub structure_topple_last_crushed_location: f32,
    pub structure_topple_building_height: f32,
    pub structure_topple_facing_width: f32,
    /// Host FireWeaponWhenDamagedBehavior continuous residual.
    pub fwwd_active: bool,
    pub fwwd_last_continuous_frame: u32,
    pub fwwd_continuous_reload_frames: u32,
    pub fwwd_continuous_pristine: String,
    pub fwwd_continuous_damaged: String,
    pub fwwd_continuous_really_damaged: String,
    pub fwwd_continuous_rubble: String,
    /// Host FireWeaponWhenDamagedBehavior reaction residual.
    pub fwwd_damage_amount: f32,
    pub fwwd_last_reaction_frame: u32,
    pub fwwd_reaction_pristine: String,
    pub fwwd_reaction_damaged: String,
    pub fwwd_reaction_really_damaged: String,
    pub fwwd_reaction_rubble: String,
    /// Host BaseRegenerateUpdate residual.
    pub base_regen_active: bool,
    pub base_regen_wake_frame: u32,
    pub base_regen_done_sold: bool,
    pub base_regen_pending_damage: bool,
    /// C++ AnimationSteeringUpdate turn anim ordinal residual.
    pub anim_steer_turn: u8,
    /// Host AnimationSteeringUpdate module active.
    pub anim_steer_active: bool,
    pub anim_steer_next_transition_frame: u32,
    pub anim_steer_transition_frames: u32,
    /// 0 empty, else short residual name key via length+hash not needed — store enum only.
    pub anim_steer_has_condition: bool,
    /// Host RadiusDecalUpdate residual.
    pub radius_decal_awake: bool,
    pub radius_decal_kill_when_idle: bool,
    pub radius_decal_empty: bool,
    pub radius_decal_pos_x: f32,
    pub radius_decal_pos_y: f32,
    pub radius_decal_pos_z: f32,
    pub radius_decal_radius: f32,
    pub radius_decal_opacity: f32,
    pub radius_decal_opacity_min: f32,
    pub radius_decal_opacity_max: f32,
    pub radius_decal_throb_frames: u32,
    pub radius_decal_birth_frame: u32,
    /// Host CheckpointUpdate residual.
    pub checkpoint_active: bool,
    pub checkpoint_enemy_near: bool,
    pub checkpoint_ally_near: bool,
    pub checkpoint_scan_delay: u32,
    pub checkpoint_scan_delay_time: u32,
    pub checkpoint_max_minor_radius: f32,
    pub checkpoint_path_radius: f32,
    pub checkpoint_door_anim: u8,
    pub checkpoint_open: bool,
    pub checkpoint_vision_range: f32,
    /// Host SmartBombTargetHomingUpdate residual.
    pub smart_bomb_homing_active: bool,
    pub smart_bomb_target_received: bool,
    pub smart_bomb_course_scalar: f32,
    pub smart_bomb_target_x: f32,
    pub smart_bomb_target_y: f32,
    pub smart_bomb_target_z: f32,
    /// Host DaisyCutter/MOAB transport residual.
    pub daisy_transport_active: bool,
    pub daisy_transport_tier: u8,
    pub daisy_transport_target_x: f32,
    pub daisy_transport_target_y: f32,
    pub daisy_transport_target_z: f32,
    pub daisy_transport_launch_x: f32,
    pub daisy_transport_launch_y: f32,
    pub daisy_transport_launch_z: f32,
    /// Host DaisyCutterBomb / MOAB payload residual.
    pub daisy_cutter_bomb: bool,
    pub daisy_bomb_is_moab: bool,
    pub daisy_bomb_vel_y: f32,
    /// Host AnthraxBomb transport residual.
    pub anthrax_transport_active: bool,
    pub anthrax_transport_tier: u8,
    pub anthrax_transport_target_x: f32,
    pub anthrax_transport_target_y: f32,
    pub anthrax_transport_target_z: f32,
    pub anthrax_transport_launch_x: f32,
    pub anthrax_transport_launch_y: f32,
    pub anthrax_transport_launch_z: f32,
    /// C++ DeliveringState finished → HeadOffMapState.
    pub anthrax_delivery_complete: bool,
    /// Host AnthraxBomb payload residual.
    pub anthrax_bomb_payload: bool,
    pub anthrax_bomb_vel_y: f32,
    /// Host ClusterMines transport residual.
    pub cluster_mines_transport_active: bool,
    pub cluster_mines_transport_target_x: f32,
    pub cluster_mines_transport_target_y: f32,
    pub cluster_mines_transport_target_z: f32,
    pub cluster_mines_transport_launch_x: f32,
    pub cluster_mines_transport_launch_y: f32,
    pub cluster_mines_transport_launch_z: f32,
    /// Host ClusterMinesBomb residual.
    pub cluster_mines_bomb: bool,
    pub cluster_mines_bomb_vel_y: f32,
    /// Host EMP Pulse transport residual.
    pub emp_pulse_transport_active: bool,
    pub emp_pulse_transport_player_id: u32,
    pub emp_pulse_transport_caster_id: u32,
    pub emp_pulse_transport_target_x: f32,
    pub emp_pulse_transport_target_y: f32,
    pub emp_pulse_transport_target_z: f32,
    pub emp_pulse_transport_launch_x: f32,
    pub emp_pulse_transport_launch_y: f32,
    pub emp_pulse_transport_launch_z: f32,
    /// Host EMPPulseBomb residual.
    pub emp_pulse_bomb: bool,
    pub emp_pulse_bomb_vel_y: f32,
    /// Host EMPPulseEffectSpheroid residual.
    pub emp_pulse_spheroid: bool,
    pub emp_pulse_spheroid_expires_frame: u32,
    /// Host A10 Thunderbolt transport residual.
    pub a10_strike_transport_active: bool,
    pub a10_strike_transport_tier: u8,
    pub a10_strike_transport_target_x: f32,
    pub a10_strike_transport_target_y: f32,
    pub a10_strike_transport_target_z: f32,
    pub a10_strike_transport_launch_x: f32,
    pub a10_strike_transport_launch_y: f32,
    pub a10_strike_transport_launch_z: f32,
    /// C++ DeliverPayloadAIUpdate `m_diveState` (0 predive, 1 diving, 2 postdive).
    pub a10_strike_dive_state: u8,
    /// Last frame A10ThunderboltVulcan residual fired (DelayBetweenShots 60ms).
    pub a10_strike_last_vulcan_frame: u32,
    /// Host A10ThunderboltMissile residual.
    pub a10_strike_missile: bool,
    pub a10_strike_missile_vel_y: f32,
    /// Host ArtilleryBarrage transport residual.
    pub artillery_barrage_transport_active: bool,
    pub artillery_barrage_transport_tier: u8,
    pub artillery_barrage_transport_target_x: f32,
    pub artillery_barrage_transport_target_y: f32,
    pub artillery_barrage_transport_target_z: f32,
    pub artillery_barrage_transport_launch_x: f32,
    pub artillery_barrage_transport_launch_y: f32,
    pub artillery_barrage_transport_launch_z: f32,
    /// Host ArtilleryBarrage shell residual.
    pub artillery_barrage_shell: bool,
    pub artillery_barrage_shell_vel_y: f32,
    /// Host CarpetBomb transport residual.
    pub carpet_bomb_transport_active: bool,
    pub carpet_bomb_transport_tier: u8,
    pub carpet_bomb_transport_target_x: f32,
    pub carpet_bomb_transport_target_y: f32,
    pub carpet_bomb_transport_target_z: f32,
    pub carpet_bomb_transport_launch_x: f32,
    pub carpet_bomb_transport_launch_y: f32,
    pub carpet_bomb_transport_launch_z: f32,
    /// Host CarpetBomb payload residual.
    pub carpet_bomb_payload: bool,
    pub carpet_bomb_payload_vel_y: f32,
    /// Host Leaflet B52 transport residual.
    pub leaflet_transport_active: bool,
    pub leaflet_transport_target_x: f32,
    pub leaflet_transport_target_y: f32,
    pub leaflet_transport_target_z: f32,
    /// Host LeafletContainer residual.
    pub leaflet_container: bool,
    pub leaflet_container_vel_y: f32,
    /// Host Paradrop cargo transport residual.
    pub paradrop_transport_active: bool,
    pub paradrop_transport_target_x: f32,
    pub paradrop_transport_target_y: f32,
    pub paradrop_transport_target_z: f32,
    /// Host AmericaParachute residual.
    pub paradrop_parachute: bool,
    pub paradrop_parachute_vel_y: f32,
    /// Host AuroraBomb projectile residual.
    pub aurora_bomb_projectile: bool,
    pub aurora_bomb_aim_x: f32,
    pub aurora_bomb_aim_y: f32,
    pub aurora_bomb_aim_z: f32,
    pub aurora_bomb_has_aim: bool,
    pub aurora_bomb_mission_id: u32,
    pub aurora_bomb_mission_live: bool,
    /// Host ToxinStream projectile residual.
    pub toxin_stream_projectile: bool,
    pub toxin_stream_aim_x: f32,
    pub toxin_stream_aim_y: f32,
    pub toxin_stream_aim_z: f32,
    pub toxin_stream_has_aim: bool,
    pub toxin_stream_intended: u32,
    pub toxin_stream_has_intended: bool,
    pub toxin_stream_travelled: f32,
    pub toxin_stream_fuel_expires_frame: u32,
    pub toxin_stream_has_fuel: bool,
    pub toxin_stream_ignition_frame: u32,
    pub toxin_stream_has_ignition: bool,
    pub toxin_stream_shooter: u32,
    pub toxin_stream_has_shooter: bool,
    /// Host AngryMob projectile residual.
    pub angry_mob_projectile: bool,
    pub angry_mob_projectile_kind: u8,
    pub angry_mob_projectile_from_x: f32,
    pub angry_mob_projectile_from_y: f32,
    pub angry_mob_projectile_from_z: f32,
    pub angry_mob_projectile_aim_x: f32,
    pub angry_mob_projectile_aim_y: f32,
    pub angry_mob_projectile_aim_z: f32,
    pub angry_mob_projectile_has_from: bool,
    pub angry_mob_projectile_has_aim: bool,
    pub angry_mob_projectile_launch_frame: u32,
    pub angry_mob_projectile_flight_frames: u32,
    pub angry_mob_projectile_intended: u32,
    pub angry_mob_projectile_has_intended: bool,
    /// Host SCUD launcher missile residual.
    pub scud_launcher_missile_projectile: bool,
    pub scud_launcher_missile_toxin: bool,
    pub scud_launcher_missile_aim_x: f32,
    pub scud_launcher_missile_aim_y: f32,
    pub scud_launcher_missile_aim_z: f32,
    pub scud_launcher_missile_has_aim: bool,
    pub scud_launcher_missile_travelled: f32,
    pub scud_launcher_missile_fuel_expires_frame: u32,
    pub scud_launcher_missile_has_fuel: bool,
    /// Host Neutron cannon shell residual.
    pub neutron_cannon_shell_projectile: bool,
    pub neutron_shell_from_x: f32,
    pub neutron_shell_from_y: f32,
    pub neutron_shell_from_z: f32,
    pub neutron_shell_aim_x: f32,
    pub neutron_shell_aim_y: f32,
    pub neutron_shell_aim_z: f32,
    pub neutron_shell_has_from: bool,
    pub neutron_shell_has_aim: bool,
    pub neutron_shell_launch_frame: u32,
    pub neutron_shell_flight_frames: u32,
    /// Host Nuke cannon shell residual.
    pub nuke_cannon_shell_projectile: bool,
    pub nuke_shell_from_x: f32,
    pub nuke_shell_from_y: f32,
    pub nuke_shell_from_z: f32,
    pub nuke_shell_aim_x: f32,
    pub nuke_shell_aim_y: f32,
    pub nuke_shell_aim_z: f32,
    pub nuke_shell_has_from: bool,
    pub nuke_shell_has_aim: bool,
    pub nuke_shell_launch_frame: u32,
    pub nuke_shell_flight_frames: u32,
    /// Host AngryMob member residual.
    pub angry_mob_member: bool,
    pub angry_mob_nexus_id: u32,
    pub angry_mob_has_nexus: bool,
    /// Host NukeRadiationField object residual.
    pub nuke_radiation_field: bool,
    pub nuke_radiation_field_expires_frame: u32,
    /// Host AnthraxToxinField object residual.
    pub anthrax_toxin_field: bool,
    pub anthrax_toxin_field_expires_frame: u32,
    /// Host InfernoFireField object residual.
    pub inferno_fire_field: bool,
    pub inferno_fire_field_expires_frame: u32,
    /// Host InfernoCannon shell residual.
    pub inferno_shell_projectile: bool,
    pub inferno_shell_from_x: f32,
    pub inferno_shell_from_y: f32,
    pub inferno_shell_from_z: f32,
    pub inferno_shell_aim_x: f32,
    pub inferno_shell_aim_y: f32,
    pub inferno_shell_aim_z: f32,
    pub inferno_shell_has_from: bool,
    pub inferno_shell_has_aim: bool,
    pub inferno_shell_launch_frame: u32,
    pub inferno_shell_flight_frames: u32,
    pub inferno_shell_intended: u32,
    pub inferno_shell_has_intended: bool,
    pub inferno_shell_upgraded: bool,
    /// Host SpySatellite ping object residual.
    pub spy_satellite_ping: bool,
    pub spy_satellite_ping_expires_frame: u32,
    /// Host Flashbang grenade residual.
    pub flashbang_grenade_projectile: bool,
    pub flashbang_grenade_from_x: f32,
    pub flashbang_grenade_from_y: f32,
    pub flashbang_grenade_from_z: f32,
    pub flashbang_grenade_aim_x: f32,
    pub flashbang_grenade_aim_y: f32,
    pub flashbang_grenade_aim_z: f32,
    pub flashbang_grenade_has_from: bool,
    pub flashbang_grenade_has_aim: bool,
    pub flashbang_grenade_launch_frame: u32,
    pub flashbang_grenade_flight_frames: u32,
    pub flashbang_grenade_intended: u32,
    pub flashbang_grenade_has_intended: bool,
    /// Host Comanche rocket-pod residual.
    pub comanche_rocket_pod_projectile: bool,
    pub comanche_rocket_pod_projectile_expires_frame: u32,
    /// Host Helix napalm bomb residual.
    pub helix_napalm_bomb_projectile: bool,
    /// Host Scorpion tank missile residual.
    pub scorpion_missile_projectile: bool,
    pub scorpion_missile_aim_x: f32,
    pub scorpion_missile_aim_y: f32,
    pub scorpion_missile_aim_z: f32,
    pub scorpion_missile_has_aim: bool,
    pub scorpion_missile_intended: u32,
    pub scorpion_missile_has_intended: bool,
    pub scorpion_missile_travelled: f32,
    pub scorpion_missile_fuel_expires_frame: u32,
    pub scorpion_missile_slot: u8,
    /// Host Spectre gunship howitzer shell residual.
    pub spectre_howitzer_shell: bool,
    pub spectre_howitzer_shell_expires_frame: u32,
    /// Host countermeasure flare residual.
    pub countermeasure_flare: bool,
    pub countermeasure_flare_expires_frame: u32,
    /// Host point-defense laser beam residual.
    pub point_defense_laser_beam: bool,
    pub point_defense_laser_beam_expires_frame: u32,
    /// Host weapon laser beam residual.
    pub weapon_laser_beam: bool,
    pub weapon_laser_beam_expires_frame: u32,
    /// Host sticky Timed/Remote demo charge attach residual.
    pub sticky_bomb_attached: bool,
    pub sticky_bomb_attached_to: u32,
    pub sticky_bomb_mine_kind: u8, // 2=TimedDemoCharge, 3=RemoteDemoCharge
    /// Host booby-trap special object attach residual.
    pub booby_trap_special: bool,
    pub booby_trap_attached_to: u32,
    pub booby_trap_has_attached: bool,
    /// Host ParticleUplink residual objects.
    pub particle_trail_remnant: bool,
    pub particle_trail_remnant_expires_frame: u32,
    pub particle_orbital_laser: bool,
    pub particle_orbital_laser_expires_frame: u32,
    pub particle_connector_laser: bool,
    pub particle_connector_laser_expires_frame: u32,
    /// Host FireWall segment residual.
    pub firewall_segment: bool,
    pub firewall_segment_expires_frame: u32,
    pub firewall_segment_wall_id: u32,
    pub firewall_segment_has_wall_id: bool,
    pub firewall_segment_dir_x: f32,
    pub firewall_segment_dir_z: f32,
    pub firewall_segment_has_dir: bool,
    /// Host RadarVanPing residual.
    pub radar_van_ping: bool,
    pub radar_van_ping_expires_frame: u32,
    /// Host Object::shock_yaw_rate residual.
    pub shock_yaw_rate: f32,
    /// Host Object::shock_pitch_rate residual.
    pub shock_pitch_rate: f32,
    /// Host Object::shock_roll_rate residual.
    pub shock_roll_rate: f32,
    /// Host Object::shock_up_z residual.
    pub shock_up_z: f32,
    /// Host Object::shock_allow_bounce residual.
    pub shock_allow_bounce: bool,
    /// Host Object::shock_grounded_once residual.
    pub shock_grounded_once: bool,
    /// Host Object::shock_was_airborne residual.
    pub shock_was_airborne: bool,
    /// Host Object::cell_is_cliff residual.
    pub cell_is_cliff: bool,
    /// Host Object::cell_is_underwater residual.
    pub cell_is_underwater: bool,
    /// Host Movement::path waypoints residual (capped for presentation line pack).
    pub path_waypoints: Vec<[f32; 3]>,
    /// Host secondary weapon range residual.
    pub secondary_weapon_range: f32,
    /// Host Weapon leech residual `leech_range_active_primary`.
    pub leech_range_active_primary: bool,
    /// Host Weapon leech residual `leech_range_active_secondary`.
    pub leech_range_active_secondary: bool,
    /// Host secondary weapon damage residual.
    pub secondary_weapon_damage: f32,
    /// Host Object::name residual (display/script name; empty if unset).
    pub display_name: String,
    /// Host ThingTemplate model key residual (mesh resolve; empty if unset).
    pub model_key: String,
    /// Host Object::model_condition_bits residual.
    pub model_condition_bits: u128,
    /// C++ RadarUpdate m_extendDoneFrame residual (0 = inactive).
    pub radar_extend_done_frame: u32,
    /// C++ RadarUpdate m_extendComplete residual.
    pub radar_extend_complete: bool,
    /// C++ RadarUpdate m_radarActive residual.
    pub radar_active: bool,
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
    /// C++ ARMORSET_SECOND_LIFE residual.
    pub second_life: bool,
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
    /// Host Object::formation_id residual (0 = no formation).
    pub formation_id: u32,
    /// Host Object::formation_offset residual (XZ as Vec2).
    pub formation_offset: [f32; 2],
    /// Wave 999: host Object::is_surrendered residual.
    pub is_surrendered: bool,
    /// Wave 999: host Object::emoticon_name residual.
    pub emoticon_name: String,
    /// Wave 999: host Object::emoticon_frames_left residual.
    pub emoticon_frames_left: i32,
    /// Wave 1001: host pending transition damage FX residual name.
    pub damage_fx_name: Option<String>,
    /// Wave 1001: host BoneFXDamage last FX residual name.
    pub bone_fx_name: Option<String>,
    /// Wave 1001: host pending death FX residual name.
    pub death_fx_name: Option<String>,
    /// Host Object::overcharge_enabled residual.
    pub overcharge_enabled: bool,
    /// Host Object::active_weapon_slot residual.
    pub active_weapon_slot: u8,
    /// C++ WeaponFireStatus ordinal residual (Ready/OutOfAmmo/Between/Reload/PreAttack).
    pub weapon_fire_status: u8,
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
    /// Host hijacker residual `hijack_vehicle_host`.
    pub hijack_vehicle_host: u32,
    /// Host hijacker residual `hijacker_in_vehicle`.
    pub hijacker_in_vehicle: bool,
    /// Host hijacker residual `hijacker_update_active`.
    pub hijacker_update_active: bool,
    /// Host hijacker residual `hijacker_was_airborne`.
    pub hijacker_was_airborne: bool,
    /// Host hijacker residual `hijacker_eject_pos`.
    pub hijacker_eject_pos: Option<[f32; 3]>,
    /// Host hijacker residual `hive_slave_respawn_frame`.
    pub hive_slave_respawn_frame: u32,
    /// Host ResidualHiveSlave alive flags (3 slots).
    pub hive_slaves_alive: [bool; 3],
    /// Host ResidualHiveSlave HP (3 slots).
    pub hive_slaves_hp: [f32; 3],
    /// Host money/salvage crate DeletionUpdate residual.
    pub money_crate: bool,
    pub money_crate_expires_frame: u32,
    /// Host FireSpread/Flammable residual.
    pub fire_spread_active: bool,
    pub fire_spread_state: u8, // 0 Normal, 1 Aflame, 2 Burned
    pub fire_spread_aflame_end_frame: u32,
    pub fire_spread_burned_end_frame: u32,
    pub fire_spread_next_spread_frame: u32,
    pub fire_spread_min_delay: u32,
    pub fire_spread_max_delay: u32,
    pub fire_spread_try_range: f32,
    pub fire_spread_aflame_duration: u32,
    pub fire_spread_burned_delay: u32,
    pub fire_spread_enabled: bool,
    pub fire_spread_flame_damage_accum: f32,
    pub fire_spread_flame_damage_limit: f32,
    /// Host black market AutoDeposit next frame residual.
    pub black_market_building: bool,
    pub black_market_next_deposit_frame: u32,
    /// Host oil derrick AutoDeposit next frame residual.
    pub oil_derrick_building: bool,
    pub oil_derrick_next_deposit_frame: u32,
    /// Host China Hacker HackInternet residual.
    pub hacker_unit: bool,
    pub hacker_hacking: bool,
    pub hacker_in_internet_center: bool,
    pub hacker_next_deposit_frame: u32,
    /// Exact parsed `HackInternetAIUpdate` cash schedule/data mirrored from
    /// Main.  Zeros remain authored C++ defaults; they never fall back to a
    /// retail Hacker template constant in the GameWorld sole-tick path.
    pub hacker_cash_update_delay_frames: u32,
    pub hacker_cash_update_delay_fast_frames: u32,
    pub hacker_regular_cash_amount: u32,
    pub hacker_veteran_cash_amount: u32,
    pub hacker_elite_cash_amount: u32,
    pub hacker_heroic_cash_amount: u32,
    pub hacker_xp_per_cash_update: f32,
    /// Host hijacker residual `next_detection_scan_frame`.
    pub next_detection_scan_frame: u32,
    /// Host Object::stealth_allowed_frame residual.
    pub stealth_allowed_frame: u32,
    /// Host Object::stealth_delay_pending residual.
    pub stealth_delay_pending: bool,
    /// Host Object::stealth_delay_frames residual.
    pub stealth_delay_frames: u32,
    /// Host Object::stealth_breaks_on_damage residual.
    pub stealth_breaks_on_damage: bool,
    /// Host Object::detection_expires_frame residual.
    pub detection_expires_frame: u32,
    /// Host Object::camo_opacity_pulse_phase residual.
    pub camo_opacity_pulse_phase: f32,
    /// Host Object::camo_heat_vision_opacity residual.
    pub camo_heat_vision_opacity: f32,
    /// Host Object::camo_net_sub_object_shown residual.
    pub camo_net_sub_object_shown: bool,
    /// Host Object::camo_net_sub_object_observer_visible residual.
    pub camo_net_sub_object_observer_visible: bool,
    /// Host weapon-bonus flags residual.
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,
    pub weapon_bonus_horde: bool,
    pub weapon_bonus_nationalism: bool,
    pub weapon_bonus_fanaticism: bool,
    pub last_horde_refresh_frame: u32,
    pub horde_next_wake_frame: u32,
    pub horde_wake_initialized: bool,

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
    /// Host Object::turret_turn_rate_rad residual.
    pub turret_turn_rate_rad: f32,
    /// Host Object::turret_recenter_frames residual.
    pub turret_recenter_frames: u32,
    /// Host Object::turret_hold_until_frame residual.
    pub turret_hold_until_frame: u32,
    /// Host Object::turret_idle_recentering residual.
    pub turret_idle_recentering: bool,
    /// Host Object::turret_enabled residual.
    pub turret_enabled: bool,
    /// Host Object::turret_rotating residual.
    pub turret_rotating: bool,
    /// Host Object::turret_natural_angle_deg residual.
    pub turret_natural_angle_deg: f32,
    /// Host Object::turret_natural_pitch_deg residual.
    pub turret_natural_pitch_deg: f32,
    /// Host Object::turret_target_host residual.
    pub turret_target_host: u32,
    /// Host Object::turret_force_attacking residual.
    pub turret_force_attacking: bool,
    /// Host Object::turret_mood_target residual.
    pub turret_mood_target: bool,
    /// Host Object::turret_idle_scan_next_frame residual.
    pub turret_idle_scan_next_frame: u32,
    /// Host Object::turret_idle_scan_desired_angle_deg residual.
    pub turret_idle_scan_desired_angle_deg: f32,
    /// Host Object::turret_idle_scan_index residual.
    pub turret_idle_scan_index: u32,
    /// Host Object::turret_substate residual.
    pub turret_substate: u8,
    /// Host AI attitude residual (-1..n as host i8).
    pub ai_attitude: i8,
    /// Host Object::idle_since_frame residual.
    pub idle_since_frame: u32,
    /// Host Object::mood_attack_check_rate residual.
    pub mood_attack_check_rate: u32,
    /// Host Object::auto_acquire_when_idle residual.
    pub auto_acquire_when_idle: bool,
    /// Host Object::attack_priority_set residual.
    pub attack_priority_set: String,
    /// Host last_damage_source as host object id (0 = none).
    pub last_damage_source_host: u32,
    /// Host Object::sole_healing_benefactor_id residual.
    pub sole_healing_benefactor_id: Option<u32>,
    /// Host Object::sole_healing_benefactor_expiration_frame residual.
    pub sole_healing_benefactor_expiration_frame: u32,
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
    /// Host Object::frame_to_force_reload residual (0 = none).
    pub frame_to_force_reload: u32,
    /// Host combat attack residual `pre_attack_target_host`.
    pub pre_attack_target_host: u32,
    /// Host combat attack residual `pre_attack_ready_at`.
    pub pre_attack_ready_at: f32,
    /// Host combat attack residual `consecutive_shots_at_target`.
    pub consecutive_shots_at_target: u32,
    /// Host combat attack residual `max_shots_to_fire`.
    pub max_shots_to_fire: i32,
    /// Host combat attack residual `attack_substate_ordinal`.
    pub attack_substate_ordinal: u8,
    /// Host combat attack residual `approach_timestamp`.
    pub approach_timestamp: u32,
    /// Host combat attack residual `continuous_fire_victim`.
    pub continuous_fire_victim: u32,
    /// Host combat attack residual `maintain_pos_valid`.
    pub maintain_pos_valid: bool,
    /// Host combat attack residual `maintain_pos`.
    pub maintain_pos: Option<[f32; 3]>,
    /// Host combat attack residual `temporary_move_frames`.
    pub temporary_move_frames: u32,
    /// Host combat attack residual `group_speed_factor`.
    pub group_speed_factor: f32,
    /// Host battle_plan_sight_scalar_applied residual (1.0 = none).
    pub battle_plan_sight_scalar_applied: f32,
    /// Envelope schema version (default 1). Entity stores payloads verbatim.
    pub envelope_version: u8,
    /// Attached lifecycle envelope. Not interpreted until authority cutover.
    pub lifecycle_envelope: Option<EntityLifecycleEnvelope>,
    /// Preview module-graph participants. None unless ENTITY_MODULES is on.
    pub entity_modules: Option<EntityInstalledModules>,
}

impl Entity {
    /// Convenience accessor for the template name.
    pub fn template_name(&self) -> &str {
        &self.template.name
    }

    /// Targeting/AI queries skip marked-destroyed entities; find-by-id still works.
    pub fn is_eligible_for_targeting(&self) -> bool {
        !self.destroyed && self.health > 0.0
    }

    /// C++ Object::isDisabled residual used by GameWorld movement authority.
    /// weapons_jammed is fire-only and is intentionally omitted.
    pub fn is_disabled(&self) -> bool {
        self.disabled_underpowered
            || self.disabled_unmanned
            || self.disabled_hacked
            || self.disabled_emp
            || self.disabled_paralyzed
            || self.disabled_subdued
            || self.disabled_freefall
            || self.disabled_script_disabled
            || self.disabled_script_underpowered
            || self.disabled_held
            || self.under_construction
    }

    /// Store the envelope verbatim. Header destroyed timing stays aligned with
    /// the deferred-destroy mark frame (`GameLogic.cpp:3932-3967`).
    pub fn attach_envelope(&mut self, envelope: EntityLifecycleEnvelope) {
        self.envelope_version = envelope.version;
        self.destroyed = envelope.destroyed;
        self.destroyed_at_frame = envelope.destroyed_at_frame;
        self.lifecycle_envelope = Some(envelope);
    }

    /// Detach the stored envelope for save/load.
    pub fn take_envelope(&mut self) -> Option<EntityLifecycleEnvelope> {
        self.lifecycle_envelope.take()
    }
}

/// Store responsible for allocating and tracking entities.
#[derive(Debug, Clone)]
pub struct EntityStore {
    pub(in crate::world) next_id: u32,
    pub(in crate::world) alive: HashMap<EntityId, Entity>,
    pub(in crate::world) generations: HashMap<u32, u32>,
}

impl Default for EntityStore {
    fn default() -> Self {
        Self::new()
    }
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
            generations: HashMap::new(),
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
        self.finish_spawn(id, template, owner, transform, health);
        id
    }

    pub(in crate::world) fn finish_spawn(
        &mut self,
        id: EntityId,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) {
        self.allocate_live_generation(id);
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
            destroyed_at_frame: 0,
            death_type: 0,
            construction_percent: 1.0,
            team_ordinal: 255,
            selection_radius: 5.0,
            crusher_level: 0,
            crushable_level: 0,
            front_crushed: false,
            back_crushed: false,
            user_1: false,
            user_2: false,
            vision_range: 0.0,
            shroud_clearing_range: 0.0,
            under_construction: false,
            sold: false,
            reconstructing: false,
            is_rebuild_hole: false,
            rebuild_template_name: String::new(),
            rebuild_ready_frame: 0,
            rebuild_spawner_id: None,
            rebuild_worker_id: None,
            rebuild_reconstructing_id: None,
            producer_id: None,
            construction_complete_clear_frame: 0,
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
            disabled_script_disabled: false,
            disabled_script_underpowered: false,
            disabled_held: false,
            weapons_jammed: false,
            masked: false,
            unattackable: false,
            dock_kind: 0,
            capturable: false,
            immune_to_capture: false,
            capture_garrisonable: false,
            capture_power: 0,
            capture_power_ready: false,
            hacker_disable_building_capable: false,
            hacker_disable_building_ready: false,
            special_power_ready_template_name: String::new(),
            special_power_ready_template_id: 0,
            disguised: false,
            disabled_subdued: false,
            subdual_damage: 0.0,
            subdual_heal_amount: 0.0,
            subdual_heal_rate_frames: 0,
            subdual_heal_countdown: 0,
            defection_undetected: false,
            defection_detection_end: 0,
            defection_detection_start: 0,
            defection_flash_phase: 0.0,
            defection_do_fx: false,
            defection_flash_this_frame: false,
            defection_final_white_flash: false,
            fire_sound_loop_until_frame: 0,
            fire_sound_loop_name: String::new(),
            lifetime_expire_at_frame: 0,
            lifetime_active: false,
            poison_damage_frame: 0,
            poison_overall_stop_frame: 0,
            poison_damage_amount: 0.0,
            poison_death_type: 0,
            poison_tint: false,
            topple_state: 0,
            topple_dir_x: 0.0,
            topple_dir_y: 0.0,
            topple_angular_velocity: 0.0,
            topple_angular_acceleration: 0.0,
            topple_angular_accumulation: 0.0,
            topple_options: 0,
            topple_kill_when_toppled: false,
            topple_lean_radians: 0.0,
            topple_active: false,
            height_die_active: false,
            height_die_target_hat: 0.0,
            height_die_only_when_descending: true,
            height_die_earliest_frame: 0,
            height_die_last_height: f32::MAX,
            height_die_has_died: false,
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
            eject_invulnerable_until_frame: 0,
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
            production_paused: false,
            exit_delay_remaining: 0.0,
            exit_delay_remaining_frames: 0,
            exit_burst_remaining: 0,
            queue_exit_state_initialized: false,
            production_door_phase: 0,
            production_door_phase_end_frame: 0,
            production_door_hold_open: false,
            rally_point: None,
            garrison_count: 0,
            max_garrison: 0,
            has_weapon: false,
            weapon_damage: 0.0,
            weapon_range: 0.0,
            weapon_min_range: 0.0,

            weapon_reload_time: 0.0,
            weapon_last_fire_time: 0.0,
            last_fire_victim_host: 0,
            last_fire_slot: 0,
            last_fire_damage: 0.0,
            last_fire_range: 0.0,
            last_fire_sim_time: 0.0,
            last_fire_frame: 0,
            fire_intent_count: 0,
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
            waiting_for_path: false,
            motive_frames_remaining: 0,
            kill_when_resting_on_ground: false,
            bounce_land_events: 0,
            last_bounce_fall_dy: 0.0,
            bounce_sound_name: String::new(),
            last_bounce_volume: 0.0,
            bounce_audio_pending: 0,
            allow_collide_force: true,
            last_collidee_id: None,
            ignore_collisions_with_id: None,
            physics_mass: 1.0,
            physics_accel: [0.0; 3],
            forward_friction: 0.0,
            lateral_friction: 0.0,
            z_friction: 0.0,
            can_path_through_units: false,
            ignore_collisions_until_frame: 0,
            is_panicking: false,
            move_away_frames: 0,
            aerodynamic_friction: 0.0,
            extra_friction: 0.0,
            apply_friction_2d_when_airborne: false,
            center_of_mass_offset: 0.0,
            pitch_roll_yaw_factor: 2.0,
            move_away_destination: None,
            request_other_move_away_id: None,
            immune_to_falling_damage: false,
            physics_current_overlap_id: None,
            physics_previous_overlap_id: None,
            locomotor_surfaces: 0,
            is_attack_path: false,
            is_approach_path: false,
            on_invalid_movement_terrain: false,
            was_airborne_last_frame: false,
            can_move_backward: false,
            moving_backwards: false,
            no_slow_down_as_approaching_dest: false,
            turn_pivot_offset: 0.0,
            allow_motive_force_while_airborne: false,
            locomotor_works_when_dead: false,
            wander_width_factor: 0.0,
            loco_apply_2d_friction_airborne: false,
            loco_extra_2d_friction: 0.0,
            loco_preferred_height: 0.0,
            loco_preferred_height_damping: 1.0,
            loco_appearance_ordinal: 0,
            loco_behavior_z_ordinal: 0,
            min_turn_speed: 0.0,
            physics_turning_ordinal: 0,
            is_blocked_and_stuck: false,
            is_braking: false,
            is_safe_path: false,
            queue_for_path_frames: 0,
            path_timestamp: 0,
            cur_max_blocked_speed: f32::MAX,
            num_frames_blocked: 0,
            is_blocked: false,
            move_away_from_id: None,
            requested_victim_id: None,
            requested_destination: None,
            prev_victim_pos: None,
            crate_created_host: 0,
            guard_retaliate_victim_host: 0,
            guard_retaliate_anchor: None,
            disguise_pending_template: String::new(),
            disguise_pending_team_ordinal: 255,
            weapon_crate_upgrade: 0,
            armor_crate_upgrade: 0,
            enemy_near: false,
            enemy_near_active: false,
            enemy_near_scan_delay: 0,
            enemy_near_scan_delay_time: 30,
            enemy_near_model: false,
            enemy_near_vision_range: 150.0,
            prone_active: false,
            prone_frames: 0,
            prone_damage_to_frames_ratio: 1.0,
            prone_model: false,
            prone_no_attack: false,
            float_update_active: false,
            float_update_enabled: false,
            float_yaw: 0.0,
            float_pitch: 0.0,
            armed: false,
            selection_flash_remaining: 0,
            shock_stun_frames: 0,
            power_plant_rods_extended: false,
            power_plant_rods_done_frame: 0,
            jet_slow_death_active: false,
            jet_slow_death_started_on_ground: false,
            jet_slow_death_hit_ground: false,
            jet_slow_death_hit_ground_frame: 0,
            jet_slow_death_roll_rate: 0.2,
            jet_slow_death_roll_rate_delta: 1.0,
            jet_slow_death_fall_how_fast: 1.10,
            jet_slow_death_vertical_velocity: 0.0,
            jet_slow_death_roll_accum: 0.0,
            jet_slow_death_done: false,
            heli_slow_death_active: false,
            heli_slow_death_hit_ground: false,
            heli_slow_death_hit_ground_frame: 0,
            heli_slow_death_activate_frame: 0,
            heli_slow_death_orbit_angle: 0.0,
            heli_slow_death_self_spin: 0.0,
            heli_slow_death_self_spin_dir: 1.0,
            heli_slow_death_frames_since_spin_update: 0,
            heli_slow_death_forward_speed: 0.0,
            heli_slow_death_vertical_velocity: 0.0,
            heli_slow_death_orientation_delta: 0.0,
            heli_slow_death_blade_flew_off: false,
            heli_slow_death_done: false,
            slow_death_phase: 0,
            slow_death_begin_frame: 0,
            slow_death_sink_at_frame: 0,
            slow_death_destroy_at_frame: 0,
            slow_death_sink_rate_per_frame: 0.0,
            slow_death_sink_offset: 0.0,
            slow_death_destruction_altitude: -10.0,
            slow_death_fling_vx: 0.0,
            slow_death_fling_vz: 0.0,
            slow_death_fling_vy: 0.0,
            slow_death_fling_applied: false,
            structure_collapse_state: 0,
            structure_collapse_start_frame: 0,
            structure_collapse_velocity: 0.0,
            structure_collapse_current_height: 0.0,
            structure_collapse_damping: 0.0,
            structure_collapse_max_shudder: 0.6,
            structure_collapse_building_height: 35.0,
            structure_collapse_shudder_x: 0.0,
            structure_collapse_shudder_z: 0.0,
            structure_topple_state: 0,
            structure_topple_start_frame: 0,
            structure_topple_dir_x: 0.0,
            structure_topple_dir_y: 1.0,
            structure_topple_velocity: 0.0,
            structure_topple_accumulated_angle: 0.0,
            structure_topple_structural_integrity: 0.5,
            structure_topple_structural_decay: 0.1,
            structure_topple_done_frame: 0,
            structure_topple_lean_radians: 0.0,
            structure_topple_last_crushed_location: 0.0,
            structure_topple_building_height: 40.0,
            structure_topple_facing_width: 20.0,
            fwwd_active: false,
            fwwd_last_continuous_frame: 0,
            fwwd_continuous_reload_frames: 30,
            fwwd_continuous_pristine: String::new(),
            fwwd_continuous_damaged: String::new(),
            fwwd_continuous_really_damaged: String::new(),
            fwwd_continuous_rubble: String::new(),
            fwwd_damage_amount: 1.0,
            fwwd_last_reaction_frame: 0,
            fwwd_reaction_pristine: String::new(),
            fwwd_reaction_damaged: String::new(),
            fwwd_reaction_really_damaged: String::new(),
            fwwd_reaction_rubble: String::new(),
            base_regen_active: false,
            base_regen_wake_frame: 0,
            base_regen_done_sold: false,
            base_regen_pending_damage: false,
            anim_steer_turn: 0,
            anim_steer_active: false,
            anim_steer_next_transition_frame: 0,
            anim_steer_transition_frames: 9,
            anim_steer_has_condition: false,
            radius_decal_awake: false,
            radius_decal_kill_when_idle: false,
            radius_decal_empty: true,
            radius_decal_pos_x: 0.0,
            radius_decal_pos_y: 0.0,
            radius_decal_pos_z: 0.0,
            radius_decal_radius: 0.0,
            radius_decal_opacity: 0.0,
            radius_decal_opacity_min: 0.25,
            radius_decal_opacity_max: 0.5,
            radius_decal_throb_frames: 15,
            radius_decal_birth_frame: 0,
            checkpoint_active: false,
            checkpoint_enemy_near: false,
            checkpoint_ally_near: false,
            checkpoint_scan_delay: 0,
            checkpoint_scan_delay_time: 30,
            checkpoint_max_minor_radius: 10.0,
            checkpoint_path_radius: 10.0,
            checkpoint_door_anim: 0,
            checkpoint_open: false,
            checkpoint_vision_range: 150.0,
            smart_bomb_homing_active: false,
            smart_bomb_target_received: false,
            smart_bomb_course_scalar: 0.99,
            smart_bomb_target_x: 0.0,
            smart_bomb_target_y: 0.0,
            smart_bomb_target_z: 0.0,
            daisy_transport_active: false,
            daisy_transport_tier: 0,
            daisy_transport_target_x: 0.0,
            daisy_transport_target_y: 0.0,
            daisy_transport_target_z: 0.0,
            daisy_transport_launch_x: 0.0,
            daisy_transport_launch_y: 0.0,
            daisy_transport_launch_z: 0.0,
            daisy_cutter_bomb: false,
            daisy_bomb_is_moab: false,
            daisy_bomb_vel_y: 0.0,
            anthrax_transport_active: false,
            anthrax_transport_tier: 0,
            anthrax_transport_target_x: 0.0,
            anthrax_transport_target_y: 0.0,
            anthrax_transport_target_z: 0.0,
            anthrax_transport_launch_x: 0.0,
            anthrax_transport_launch_y: 0.0,
            anthrax_transport_launch_z: 0.0,
            anthrax_delivery_complete: false,
            anthrax_bomb_payload: false,
            anthrax_bomb_vel_y: 0.0,
            cluster_mines_transport_active: false,
            cluster_mines_transport_target_x: 0.0,
            cluster_mines_transport_target_y: 0.0,
            cluster_mines_transport_target_z: 0.0,
            cluster_mines_transport_launch_x: 0.0,
            cluster_mines_transport_launch_y: 0.0,
            cluster_mines_transport_launch_z: 0.0,
            cluster_mines_bomb: false,
            cluster_mines_bomb_vel_y: 0.0,
            emp_pulse_transport_active: false,
            emp_pulse_transport_player_id: 0,
            emp_pulse_transport_caster_id: 0,
            emp_pulse_transport_target_x: 0.0,
            emp_pulse_transport_target_y: 0.0,
            emp_pulse_transport_target_z: 0.0,
            emp_pulse_transport_launch_x: 0.0,
            emp_pulse_transport_launch_y: 0.0,
            emp_pulse_transport_launch_z: 0.0,
            emp_pulse_bomb: false,
            emp_pulse_bomb_vel_y: 0.0,
            emp_pulse_spheroid: false,
            emp_pulse_spheroid_expires_frame: 0,
            a10_strike_transport_active: false,
            a10_strike_transport_tier: 0,
            a10_strike_transport_target_x: 0.0,
            a10_strike_transport_target_y: 0.0,
            a10_strike_transport_target_z: 0.0,
            a10_strike_transport_launch_x: 0.0,
            a10_strike_transport_launch_y: 0.0,
            a10_strike_transport_launch_z: 0.0,
            a10_strike_dive_state: 0,
            a10_strike_last_vulcan_frame: 0,
            a10_strike_missile: false,
            a10_strike_missile_vel_y: 0.0,
            artillery_barrage_transport_active: false,
            artillery_barrage_transport_tier: 0,
            artillery_barrage_transport_target_x: 0.0,
            artillery_barrage_transport_target_y: 0.0,
            artillery_barrage_transport_target_z: 0.0,
            artillery_barrage_transport_launch_x: 0.0,
            artillery_barrage_transport_launch_y: 0.0,
            artillery_barrage_transport_launch_z: 0.0,
            artillery_barrage_shell: false,
            artillery_barrage_shell_vel_y: 0.0,
            carpet_bomb_transport_active: false,
            carpet_bomb_transport_tier: 0,
            carpet_bomb_transport_target_x: 0.0,
            carpet_bomb_transport_target_y: 0.0,
            carpet_bomb_transport_target_z: 0.0,
            carpet_bomb_transport_launch_x: 0.0,
            carpet_bomb_transport_launch_y: 0.0,
            carpet_bomb_transport_launch_z: 0.0,
            carpet_bomb_payload: false,
            carpet_bomb_payload_vel_y: 0.0,
            leaflet_transport_active: false,
            leaflet_transport_target_x: 0.0,
            leaflet_transport_target_y: 0.0,
            leaflet_transport_target_z: 0.0,
            leaflet_container: false,
            leaflet_container_vel_y: 0.0,
            paradrop_transport_active: false,
            paradrop_transport_target_x: 0.0,
            paradrop_transport_target_y: 0.0,
            paradrop_transport_target_z: 0.0,
            paradrop_parachute: false,
            paradrop_parachute_vel_y: 0.0,
            aurora_bomb_projectile: false,
            aurora_bomb_aim_x: 0.0,
            aurora_bomb_aim_y: 0.0,
            aurora_bomb_aim_z: 0.0,
            aurora_bomb_has_aim: false,
            aurora_bomb_mission_id: 0,
            aurora_bomb_mission_live: false,
            toxin_stream_projectile: false,
            toxin_stream_aim_x: 0.0,
            toxin_stream_aim_y: 0.0,
            toxin_stream_aim_z: 0.0,
            toxin_stream_has_aim: false,
            toxin_stream_intended: 0,
            toxin_stream_has_intended: false,
            toxin_stream_travelled: 0.0,
            toxin_stream_fuel_expires_frame: 0,
            toxin_stream_has_fuel: false,
            toxin_stream_ignition_frame: 0,
            toxin_stream_has_ignition: false,
            toxin_stream_shooter: 0,
            toxin_stream_has_shooter: false,
            angry_mob_projectile: false,
            angry_mob_projectile_kind: 0,
            angry_mob_projectile_from_x: 0.0,
            angry_mob_projectile_from_y: 0.0,
            angry_mob_projectile_from_z: 0.0,
            angry_mob_projectile_aim_x: 0.0,
            angry_mob_projectile_aim_y: 0.0,
            angry_mob_projectile_aim_z: 0.0,
            angry_mob_projectile_has_from: false,
            angry_mob_projectile_has_aim: false,
            angry_mob_projectile_launch_frame: 0,
            angry_mob_projectile_flight_frames: 0,
            angry_mob_projectile_intended: 0,
            angry_mob_projectile_has_intended: false,
            scud_launcher_missile_projectile: false,
            scud_launcher_missile_toxin: false,
            scud_launcher_missile_aim_x: 0.0,
            scud_launcher_missile_aim_y: 0.0,
            scud_launcher_missile_aim_z: 0.0,
            scud_launcher_missile_has_aim: false,
            scud_launcher_missile_travelled: 0.0,
            scud_launcher_missile_fuel_expires_frame: 0,
            scud_launcher_missile_has_fuel: false,
            neutron_cannon_shell_projectile: false,
            neutron_shell_from_x: 0.0,
            neutron_shell_from_y: 0.0,
            neutron_shell_from_z: 0.0,
            neutron_shell_aim_x: 0.0,
            neutron_shell_aim_y: 0.0,
            neutron_shell_aim_z: 0.0,
            neutron_shell_has_from: false,
            neutron_shell_has_aim: false,
            neutron_shell_launch_frame: 0,
            neutron_shell_flight_frames: 0,
            nuke_cannon_shell_projectile: false,
            nuke_shell_from_x: 0.0,
            nuke_shell_from_y: 0.0,
            nuke_shell_from_z: 0.0,
            nuke_shell_aim_x: 0.0,
            nuke_shell_aim_y: 0.0,
            nuke_shell_aim_z: 0.0,
            nuke_shell_has_from: false,
            nuke_shell_has_aim: false,
            nuke_shell_launch_frame: 0,
            nuke_shell_flight_frames: 0,
            angry_mob_member: false,
            angry_mob_nexus_id: 0,
            angry_mob_has_nexus: false,
            nuke_radiation_field: false,
            nuke_radiation_field_expires_frame: 0,
            anthrax_toxin_field: false,
            anthrax_toxin_field_expires_frame: 0,
            inferno_fire_field: false,
            inferno_fire_field_expires_frame: 0,
            inferno_shell_projectile: false,
            inferno_shell_from_x: 0.0,
            inferno_shell_from_y: 0.0,
            inferno_shell_from_z: 0.0,
            inferno_shell_aim_x: 0.0,
            inferno_shell_aim_y: 0.0,
            inferno_shell_aim_z: 0.0,
            inferno_shell_has_from: false,
            inferno_shell_has_aim: false,
            inferno_shell_launch_frame: 0,
            inferno_shell_flight_frames: 0,
            inferno_shell_intended: 0,
            inferno_shell_has_intended: false,
            inferno_shell_upgraded: false,
            spy_satellite_ping: false,
            spy_satellite_ping_expires_frame: 0,
            flashbang_grenade_projectile: false,
            flashbang_grenade_from_x: 0.0,
            flashbang_grenade_from_y: 0.0,
            flashbang_grenade_from_z: 0.0,
            flashbang_grenade_aim_x: 0.0,
            flashbang_grenade_aim_y: 0.0,
            flashbang_grenade_aim_z: 0.0,
            flashbang_grenade_has_from: false,
            flashbang_grenade_has_aim: false,
            flashbang_grenade_launch_frame: 0,
            flashbang_grenade_flight_frames: 0,
            flashbang_grenade_intended: 0,
            flashbang_grenade_has_intended: false,
            comanche_rocket_pod_projectile: false,
            comanche_rocket_pod_projectile_expires_frame: 0,
            helix_napalm_bomb_projectile: false,
            scorpion_missile_projectile: false,
            scorpion_missile_aim_x: 0.0,
            scorpion_missile_aim_y: 0.0,
            scorpion_missile_aim_z: 0.0,
            scorpion_missile_has_aim: false,
            scorpion_missile_intended: 0,
            scorpion_missile_has_intended: false,
            scorpion_missile_travelled: 0.0,
            scorpion_missile_fuel_expires_frame: 0,
            scorpion_missile_slot: 0,
            spectre_howitzer_shell: false,
            spectre_howitzer_shell_expires_frame: 0,
            countermeasure_flare: false,
            countermeasure_flare_expires_frame: 0,
            point_defense_laser_beam: false,
            point_defense_laser_beam_expires_frame: 0,
            weapon_laser_beam: false,
            weapon_laser_beam_expires_frame: 0,
            sticky_bomb_attached: false,
            sticky_bomb_attached_to: 0,
            sticky_bomb_mine_kind: 0,
            booby_trap_special: false,
            booby_trap_attached_to: 0,
            booby_trap_has_attached: false,
            particle_trail_remnant: false,
            particle_trail_remnant_expires_frame: 0,
            particle_orbital_laser: false,
            particle_orbital_laser_expires_frame: 0,
            particle_connector_laser: false,
            particle_connector_laser_expires_frame: 0,
            firewall_segment: false,
            firewall_segment_expires_frame: 0,
            firewall_segment_wall_id: 0,
            firewall_segment_has_wall_id: false,
            firewall_segment_dir_x: 1.0,
            firewall_segment_dir_z: 0.0,
            firewall_segment_has_dir: false,
            radar_van_ping: false,
            radar_van_ping_expires_frame: 0,
            shock_yaw_rate: 0.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_up_z: 1.0,
            shock_allow_bounce: false,
            shock_grounded_once: false,
            shock_was_airborne: false,
            cell_is_cliff: false,
            cell_is_underwater: false,
            path_waypoints: Vec::new(),
            secondary_weapon_range: 0.0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            secondary_weapon_damage: 0.0,
            display_name: String::new(),
            model_key: String::new(),
            model_condition_bits: 0,
            radar_extend_done_frame: 0,
            radar_extend_complete: false,
            radar_active: false,
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
            second_life: false,
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
            formation_id: 0,
            formation_offset: [0.0, 0.0],
            is_surrendered: false,
            emoticon_name: String::new(),
            emoticon_frames_left: 0,
            damage_fx_name: None,
            bone_fx_name: None,
            death_fx_name: None,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            weapon_fire_status: 0,
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
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
            detection_expires_frame: 0,
            camo_opacity_pulse_phase: 0.0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_fanaticism: false,
            last_horde_refresh_frame: 0,
            horde_next_wake_frame: 0,
            horde_wake_initialized: false,

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
            hijack_vehicle_host: 0,
            hijacker_in_vehicle: false,
            hijacker_update_active: false,
            hijacker_was_airborne: false,
            hijacker_eject_pos: None,
            hive_slave_respawn_frame: 0,
            hive_slaves_alive: [false; 3],
            hive_slaves_hp: [0.0; 3],
            money_crate: false,
            money_crate_expires_frame: 0,
            fire_spread_active: false,
            fire_spread_state: 0,
            fire_spread_aflame_end_frame: 0,
            fire_spread_burned_end_frame: 0,
            fire_spread_next_spread_frame: 0,
            fire_spread_min_delay: 0,
            fire_spread_max_delay: 0,
            fire_spread_try_range: 0.0,
            fire_spread_aflame_duration: 0,
            fire_spread_burned_delay: 0,
            fire_spread_enabled: false,
            fire_spread_flame_damage_accum: 0.0,
            fire_spread_flame_damage_limit: 0.0,
            black_market_building: false,
            black_market_next_deposit_frame: 0,
            oil_derrick_building: false,
            oil_derrick_next_deposit_frame: 0,
            hacker_unit: false,
            hacker_hacking: false,
            hacker_in_internet_center: false,
            hacker_next_deposit_frame: 0,
            hacker_cash_update_delay_frames: 0,
            hacker_cash_update_delay_fast_frames: 0,
            hacker_regular_cash_amount: 0,
            hacker_veteran_cash_amount: 0,
            hacker_elite_cash_amount: 0,
            hacker_heroic_cash_amount: 0,
            hacker_xp_per_cash_update: 0.0,
            next_detection_scan_frame: 0,
            turret_angle_deg: 0.0,
            turret_pitch_deg: 0.0,
            turret_idle_scanning: false,
            turret_holding: false,
            turret_turn_rate_rad: 0.0,
            turret_recenter_frames: 0,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_enabled: false,
            turret_rotating: false,
            turret_natural_angle_deg: 0.0,
            turret_natural_pitch_deg: 0.0,
            turret_target_host: 0,
            turret_force_attacking: false,
            turret_mood_target: false,
            turret_idle_scan_next_frame: 0,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_substate: 0,
            ai_attitude: 0,
            idle_since_frame: 0,
            mood_attack_check_rate: 30,
            auto_acquire_when_idle: true,
            attack_priority_set: String::new(),
            last_damage_source_host: 0,
            sole_healing_benefactor_id: None,
            sole_healing_benefactor_expiration_frame: 0,
            command_set_override: String::new(),
            disguise_as_template: String::new(),
            disguise_as_team_ordinal: 255,
            vision_spied_mask: 0,
            camo_friendly_opacity: 1.0,
            camo_stealth_look: 0,
            has_mine_data: false,
            weapon_bonus_frenzy_until_frame: 0,
            continuous_fire_coast_until_frame: 0,
            frame_to_force_reload: 0,
            pre_attack_target_host: 0,
            pre_attack_ready_at: 0.0,
            consecutive_shots_at_target: 0,
            max_shots_to_fire: -1,
            attack_substate_ordinal: 0,
            approach_timestamp: 0,
            continuous_fire_victim: 0,
            maintain_pos_valid: false,
            maintain_pos: None,
            temporary_move_frames: 0,
            group_speed_factor: 1.0,
            battle_plan_sight_scalar_applied: 1.0,
            envelope_version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            lifecycle_envelope: None,
            entity_modules: None,
        };

        self.alive.insert(id, entity);
    }

    /// Remove an entity. Returns the removed entity if it was alive.
    pub fn remove(&mut self, id: EntityId) -> Option<Entity> {
        let removed = self.alive.remove(&id)?;
        self.bump_generation(id);
        Some(removed)
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

    #[test]
    fn attach_envelope_copies_deferred_destroy_timing() {
        let mut store = EntityStore::new();
        let id = store.spawn(TemplateRef::new("Test"), None, Transform::default(), 10.0);
        let entity = store.get_mut(id).expect("spawned");
        assert_eq!(entity.envelope_version, ENTITY_LIFECYCLE_ENVELOPE_VERSION);
        entity.attach_envelope(EntityLifecycleEnvelope {
            version: ENTITY_LIFECYCLE_ENVELOPE_VERSION,
            entity_id: id.get(),
            destroyed: true,
            destroyed_at_frame: 33,
            module_states: vec![EntityModuleState {
                tag: "UnknownFuture".to_string(),
                payload: vec![1],
            }],
        });
        assert!(entity.destroyed);
        assert_eq!(entity.destroyed_at_frame, 33);
        let taken = entity.take_envelope().expect("attached");
        assert_eq!(taken.destroyed_at_frame, 33);
        assert_eq!(taken.module_states[0].tag, "UnknownFuture");
        assert!(entity.take_envelope().is_none());
    }
}

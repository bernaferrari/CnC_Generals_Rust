//! Authored definitions shared by the world and legacy weapon adapters.

use std::sync::Arc;

use crate::damage::{DamageType, DeathType};
use crate::object::{VeterancyLevel, WeaponDamageStatus};

use super::{
    WeaponAffectsMask, WeaponAntiMask, WeaponBonus, WeaponBonusSet, WeaponCollideMask,
    WeaponPrefireType, WeaponReloadType,
};

/// 2D coordinate for scatter targets
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Name of the audio event sent by a weapon.
#[derive(Debug, Clone)]
pub struct WeaponSound {
    event_name: String,
}

impl WeaponSound {
    pub fn new(event_name: String) -> Self {
        Self { event_name }
    }

    pub fn is_empty(&self) -> bool {
        self.event_name.is_empty()
    }

    pub fn name(&self) -> &str {
        &self.event_name
    }
}

/// An immutable reference to an authored FX list.
#[derive(Debug, Clone)]
pub struct WeaponFxName(Arc<str>);

impl WeaponFxName {
    pub fn new(name: &str) -> Self {
        Self(Arc::from(name))
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Authored weapon properties. References carry their names across the simulation boundary.
#[derive(Debug, Clone)]
pub struct WeaponDefinition {
    /// Basic properties
    pub name: String,

    /// Damage properties
    pub primary_damage: f32,
    pub primary_damage_radius: f32,
    pub secondary_damage: f32,
    pub secondary_damage_radius: f32,
    pub shock_wave_amount: f32,
    pub shock_wave_radius: f32,
    pub shock_wave_taper_off: f32,

    /// Range and targeting
    pub attack_range: f32,
    pub minimum_attack_range: f32,
    pub request_assist_range: f32,
    pub aim_delta: f32,
    pub scatter_radius: f32,
    pub scatter_target_scalar: f32,
    pub scatter_targets: Vec<Coord2D>,

    /// Timing and reload
    pub min_delay_between_shots: i32,
    pub max_delay_between_shots: i32,
    pub clip_size: i32,
    pub clip_reload_time: i32,
    pub pre_attack_delay: i32,
    pub auto_reload_when_idle_frames: u32,
    pub suspend_fx_delay: u32,

    /// Weapon behavior
    pub weapon_speed: f32,
    pub min_weapon_speed: f32,
    pub is_scale_weapon_speed: bool,
    pub weapon_recoil: f32,
    pub min_target_pitch: f32,
    pub max_target_pitch: f32,
    pub radius_damage_angle: f32,

    /// Projectile
    pub projectile_name: String,
    pub projectile_stream_name: String,
    pub laser_name: String,
    pub laser_bone_name: String,

    /// Damage and death types
    pub damage_type: DamageType,
    pub damage_status_type: WeaponDamageStatus,
    pub death_type: DeathType,

    /// Masks and flags
    pub anti_mask: WeaponAntiMask,
    pub affects_mask: WeaponAffectsMask,
    pub collide_mask: WeaponCollideMask,

    /// Weapon type properties
    pub damage_dealt_at_self_position: bool,
    pub reload_type: WeaponReloadType,
    pub prefire_type: WeaponPrefireType,
    pub leech_range_weapon: bool,
    pub capable_of_following_waypoint: bool,
    pub is_shows_ammo_pips: bool,
    pub allow_attack_garrisoned_bldgs: bool,
    pub play_fx_when_stealthed: bool,
    pub die_on_detonate: bool,
    pub must_travel_pfx: bool,

    /// Continuous fire
    pub continuous_fire_one_shots_needed: i32,
    pub continuous_fire_two_shots_needed: i32,
    pub continuous_fire_coast_frames: u32,

    /// Special targeting
    pub continue_attack_range: f32,
    pub infantry_inaccuracy_dist: f32,

    /// Barrel management
    pub shots_per_barrel: i32,

    /// Historic bonus
    pub historic_bonus_time: u32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_count: i32,
    /// Weapon to fire when the historic shot count reaches its threshold.
    pub historic_bonus_weapon_name: String,

    /// Audio
    pub fire_sound: WeaponSound,
    pub fire_sound_loop_time: u32,

    /// Per-veterancy level effects (Regular, Veteran, Elite, Heroic)
    pub fire_fx: [Option<WeaponFxName>; 4],
    pub projectile_detonate_fx: [Option<WeaponFxName>; 4],
    /// Raw C++ `FireOCL` / `VeterancyFireOCL` names, one per veterancy level.
    pub fire_ocl_names: [Option<String>; 4],
    /// Raw C++ `ProjectileDetonationOCL` names, one per veterancy level.
    pub projectile_detonation_ocl_names: [Option<String>; 4],
    /// Raw C++ `ProjectileExhaust` names, resolved lazily once ParticleSystem
    /// definitions are available. This avoids inventing a default template.
    pub projectile_exhaust_names: [Option<String>; 4],

    /// Bonuses
    pub extra_bonus: Option<WeaponBonusSet>,
}

impl WeaponDefinition {
    /// Reject non-finite values at the boundary where worlds admit a catalog.
    pub(super) fn has_finite_simulation_values(&self) -> bool {
        [
            self.primary_damage,
            self.primary_damage_radius,
            self.secondary_damage,
            self.secondary_damage_radius,
            self.weapon_speed,
            self.min_weapon_speed,
            self.attack_range,
            self.minimum_attack_range,
            self.request_assist_range,
            self.scatter_radius,
            self.infantry_inaccuracy_dist,
            self.scatter_target_scalar,
            self.shock_wave_amount,
            self.shock_wave_radius,
            self.shock_wave_taper_off,
            self.aim_delta,
            self.min_target_pitch,
            self.max_target_pitch,
            self.weapon_recoil,
            self.radius_damage_angle,
            self.historic_bonus_radius,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self
                .scatter_targets
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
    }

    pub fn new(name: String) -> Self {
        Self {
            name,
            primary_damage: 0.0,
            primary_damage_radius: 0.0,
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            attack_range: 0.0,
            minimum_attack_range: 0.0,
            request_assist_range: 0.0,
            aim_delta: 0.0,
            scatter_radius: 0.0,
            scatter_target_scalar: 0.0,
            scatter_targets: Vec::new(),
            min_delay_between_shots: 0,
            max_delay_between_shots: 0,
            clip_size: 0,
            clip_reload_time: 0,
            pre_attack_delay: 0,
            auto_reload_when_idle_frames: 0,
            suspend_fx_delay: 0,
            weapon_speed: 999999.0,
            min_weapon_speed: 999999.0,
            is_scale_weapon_speed: false,
            weapon_recoil: 0.0,
            min_target_pitch: -std::f32::consts::PI,
            max_target_pitch: std::f32::consts::PI,
            radius_damage_angle: std::f32::consts::PI,
            projectile_name: String::new(),
            projectile_stream_name: String::new(),
            laser_name: String::new(),
            laser_bone_name: String::new(),
            damage_type: DamageType::Explosion,
            damage_status_type: WeaponDamageStatus::new(WeaponDamageStatus::NONE),
            death_type: DeathType::Normal,
            anti_mask: WeaponAntiMask::new(WeaponAntiMask::GROUND),
            affects_mask: WeaponAffectsMask::new(WeaponAffectsMask::DEFAULT),
            collide_mask: WeaponCollideMask::new(WeaponCollideMask::STRUCTURES),
            damage_dealt_at_self_position: false,
            reload_type: WeaponReloadType::AutoReload,
            prefire_type: WeaponPrefireType::PrefirePerShot,
            leech_range_weapon: false,
            capable_of_following_waypoint: false,
            is_shows_ammo_pips: false,
            allow_attack_garrisoned_bldgs: false,
            play_fx_when_stealthed: false,
            die_on_detonate: false,
            must_travel_pfx: false,
            continuous_fire_one_shots_needed: i32::MAX,
            continuous_fire_two_shots_needed: i32::MAX,
            continuous_fire_coast_frames: 0,
            continue_attack_range: 0.0,
            infantry_inaccuracy_dist: 0.0,
            shots_per_barrel: 1,
            historic_bonus_time: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_count: 0,
            historic_bonus_weapon_name: String::new(),
            fire_sound: WeaponSound::new(String::new()),
            fire_sound_loop_time: 0,
            fire_fx: [None, None, None, None],
            projectile_detonate_fx: [None, None, None, None],
            fire_ocl_names: [None, None, None, None],
            projectile_detonation_ocl_names: [None, None, None, None],
            projectile_exhaust_names: [None, None, None, None],
            extra_bonus: None,
        }
    }

    /// Effective shot delay with the receiving world's draw function.
    pub fn delay_between_shots(
        &self,
        bonus: &WeaponBonus,
        random: impl FnOnce(i32, i32) -> i32,
    ) -> i32 {
        bonus.delay_between_shots(
            self.min_delay_between_shots,
            self.max_delay_between_shots,
            random,
        )
    }

    /// Get attack range with bonus applied.
    pub fn get_attack_range(&self, bonus: &WeaponBonus) -> f32 {
        bonus.attack_range(self.attack_range)
    }

    /// Get unmodified attack range
    pub fn get_unmodified_attack_range(&self) -> f32 {
        self.attack_range
    }

    pub fn get_request_assist_range(&self) -> f32 {
        self.request_assist_range
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_min_target_pitch(&self) -> f32 {
        self.min_target_pitch
    }

    pub fn get_max_target_pitch(&self) -> f32 {
        self.max_target_pitch
    }

    pub fn get_shots_per_barrel(&self) -> i32 {
        self.shots_per_barrel
    }

    pub fn get_clip_size(&self) -> i32 {
        self.clip_size
    }

    pub fn get_scatter_targets_count(&self) -> usize {
        self.scatter_targets.len()
    }

    pub fn get_scatter_targets_vector(&self) -> &[Coord2D] {
        &self.scatter_targets
    }

    pub fn get_scatter_target_scalar(&self) -> f32 {
        self.scatter_target_scalar
    }

    pub fn is_leech_range_weapon(&self) -> bool {
        self.leech_range_weapon
    }

    pub fn get_anti_mask(&self) -> u32 {
        self.anti_mask.bits()
    }

    pub fn get_extra_bonus(&self) -> Option<&WeaponBonusSet> {
        self.extra_bonus.as_ref()
    }

    /// Get minimum attack range
    pub fn get_minimum_attack_range(&self) -> f32 {
        super::minimum_attack_range(self.minimum_attack_range)
    }

    /// Get clip reload time with bonus applied
    pub fn get_clip_reload_time(&self, bonus: &WeaponBonus) -> i32 {
        bonus.reload_frames(self.clip_reload_time)
    }

    /// Get pre-attack delay with bonus applied
    pub fn get_pre_attack_delay(&self, bonus: &WeaponBonus) -> i32 {
        bonus.pre_attack_frames(self.pre_attack_delay)
    }

    /// Get primary damage with bonus applied
    pub fn get_primary_damage(&self, bonus: &WeaponBonus) -> f32 {
        bonus.damage(self.primary_damage)
    }

    /// Get primary damage radius with bonus applied
    pub fn get_primary_damage_radius(&self, bonus: &WeaponBonus) -> f32 {
        bonus.damage_radius(self.primary_damage_radius)
    }

    /// Get secondary damage with bonus applied
    pub fn get_secondary_damage(&self, bonus: &WeaponBonus) -> f32 {
        bonus.damage(self.secondary_damage)
    }

    /// Get secondary damage radius with bonus applied
    pub fn get_secondary_damage_radius(&self, bonus: &WeaponBonus) -> f32 {
        bonus.damage_radius(self.secondary_damage_radius)
    }

    /// Check if this is a contact weapon (requires collision with target)
    ///
    /// Matches C++ WeaponTemplate::isContactWeapon() from Weapon.cpp lines 531-543
    /// A weapon is a contact weapon if its attack range (minus undersize) is less than
    /// one pathfind cell size. This ensures weapons that require close proximity
    /// (melee, collision-based) are correctly identified.
    pub fn is_contact_weapon(&self) -> bool {
        super::is_contact_weapon(self.attack_range)
    }

    /// Check if this weapon automatically reloads
    pub fn get_auto_reloads_clip(&self) -> bool {
        matches!(self.reload_type, WeaponReloadType::AutoReload)
    }

    /// Check if this is a laser weapon
    pub fn is_laser(&self) -> bool {
        !self.laser_name.is_empty()
    }

    pub fn get_damage_type(&self) -> DamageType {
        self.damage_type
    }

    pub fn get_fire_fx(&self, level: VeterancyLevel) -> Option<&WeaponFxName> {
        self.fire_fx.get(level as usize).and_then(|fx| fx.as_ref())
    }

    pub fn get_projectile_detonate_fx(&self, level: VeterancyLevel) -> Option<&WeaponFxName> {
        self.projectile_detonate_fx
            .get(level as usize)
            .and_then(|fx| fx.as_ref())
    }

    /// Raw `FireOCL` name, retained for host-side presentation paths that use
    /// names rather than direct ObjectCreationList handles.
    pub fn get_fire_ocl_name(&self, level: VeterancyLevel) -> Option<&str> {
        self.fire_ocl_names
            .get(level as usize)
            .and_then(|name| name.as_deref())
    }

    /// Raw `ProjectileDetonationOCL` name, retained until it can be resolved
    /// against the live ObjectCreationList store.
    pub fn get_projectile_detonation_ocl_name(&self, level: VeterancyLevel) -> Option<&str> {
        self.projectile_detonation_ocl_names
            .get(level as usize)
            .and_then(|name| name.as_deref())
    }

    /// Raw `ProjectileExhaust` name. This is intentionally separate from
    /// `get_projectile_exhaust`: presentation can wait for the client template
    /// loader, while simulation stays fail-closed when the definition is gone.
    pub fn get_projectile_exhaust_name(&self, level: VeterancyLevel) -> Option<&str> {
        self.projectile_exhaust_names
            .get(level as usize)
            .and_then(|name| name.as_deref())
    }
}

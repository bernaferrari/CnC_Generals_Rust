////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_weapon.rs
//! Author: Colin Day, November 2001 (Converted to Rust)
//! Desc:   Parsing Weapon INI entries

use crate::common::ascii_string::AsciiString;
use crate::common::ini::INILoadType;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Result type for weapon parsing operations
pub type WeaponResult<T> = Result<T, WeaponError>;

/// Errors that can occur during weapon parsing
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponError {
    InvalidName,
    InvalidType,
    ParseError(String),
    StoreError(String),
    NotFound,
    AlreadyExists,
}

const CPP_WEAPON_TEMPLATE_FIELDS: &[&str] = &[
    "PrimaryDamage",
    "PrimaryDamageRadius",
    "SecondaryDamage",
    "SecondaryDamageRadius",
    "ShockWaveAmount",
    "ShockWaveRadius",
    "ShockWaveTaperOff",
    "AttackRange",
    "MinimumAttackRange",
    "RequestAssistRange",
    "AcceptableAimDelta",
    "ScatterRadius",
    "ScatterTargetScalar",
    "ScatterRadiusVsInfantry",
    "DamageType",
    "DamageStatusType",
    "DeathType",
    "WeaponSpeed",
    "MinWeaponSpeed",
    "ScaleWeaponSpeed",
    "WeaponRecoil",
    "MinTargetPitch",
    "MaxTargetPitch",
    "RadiusDamageAngle",
    "ProjectileObject",
    "FireSound",
    "FireSoundLoopTime",
    "FireFX",
    "ProjectileDetonationFX",
    "FireOCL",
    "ProjectileDetonationOCL",
    "ProjectileExhaust",
    "VeterancyFireFX",
    "VeterancyProjectileDetonationFX",
    "VeterancyFireOCL",
    "VeterancyProjectileDetonationOCL",
    "VeterancyProjectileExhaust",
    "ClipSize",
    "ContinuousFireOne",
    "ContinuousFireTwo",
    "ContinuousFireCoast",
    "AutoReloadWhenIdle",
    "ClipReloadTime",
    "DelayBetweenShots",
    "ShotsPerBarrel",
    "DamageDealtAtSelfPosition",
    "RadiusDamageAffects",
    "ProjectileCollidesWith",
    "AntiAirborneVehicle",
    "AntiGround",
    "AntiProjectile",
    "AntiSmallMissile",
    "AntiMine",
    "AntiParachute",
    "AntiAirborneInfantry",
    "AntiBallisticMissile",
    "AutoReloadsClip",
    "ProjectileStreamName",
    "LaserName",
    "LaserBoneName",
    "WeaponBonus",
    "HistoricBonusTime",
    "HistoricBonusRadius",
    "HistoricBonusCount",
    "HistoricBonusWeapon",
    "LeechRangeWeapon",
    "ScatterTarget",
    "CapableOfFollowingWaypoints",
    "ShowsAmmoPips",
    "AllowAttackGarrisonedBldgs",
    "PlayFXWhenStealthed",
    "PreAttackDelay",
    "PreAttackType",
    "ContinueAttackRange",
    "SuspendFXDelay",
    "MissileCallsOnDie",
];

fn is_cpp_weapon_template_field(key: &str) -> bool {
    CPP_WEAPON_TEMPLATE_FIELDS
        .iter()
        .any(|field| field.eq_ignore_ascii_case(key))
}

fn parse_cpp_weapon_field_for_table(value: &str) -> Result<Box<dyn std::any::Any>, String> {
    Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
}

impl std::fmt::Display for WeaponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeaponError::InvalidName => write!(f, "Invalid weapon name"),
            WeaponError::InvalidType => write!(f, "Invalid weapon type"),
            WeaponError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WeaponError::StoreError(msg) => write!(f, "Weapon store error: {}", msg),
            WeaponError::NotFound => write!(f, "Weapon not found"),
            WeaponError::AlreadyExists => write!(f, "Weapon already exists"),
        }
    }
}

impl std::error::Error for WeaponError {}

/// Weapon damage types
#[derive(Debug, Clone, PartialEq)]
pub enum DamageType {
    Physical,
    Explosive,
    Fire,
    Chemical,
    Electrical,
    Radiation,
    Laser,
    Plasma,
    Kinetic,
    Armor,
    Structure,
    Custom(String),
}

impl DamageType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "physical" => Self::Physical,
            "explosive" => Self::Explosive,
            "fire" => Self::Fire,
            "chemical" => Self::Chemical,
            "electrical" => Self::Electrical,
            "radiation" => Self::Radiation,
            "laser" => Self::Laser,
            "plasma" => Self::Plasma,
            "kinetic" => Self::Kinetic,
            "armor" => Self::Armor,
            "structure" => Self::Structure,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Physical => "Physical",
            Self::Explosive => "Explosive",
            Self::Fire => "Fire",
            Self::Chemical => "Chemical",
            Self::Electrical => "Electrical",
            Self::Radiation => "Radiation",
            Self::Laser => "Laser",
            Self::Plasma => "Plasma",
            Self::Kinetic => "Kinetic",
            Self::Armor => "Armor",
            Self::Structure => "Structure",
            Self::Custom(name) => name,
        }
    }
}

/// Weapon attack types
#[derive(Debug, Clone, PartialEq)]
pub enum AttackType {
    Direct,
    Area,
    Projectile,
    Beam,
    Hitscan,
    Guided,
    Ballistic,
    Custom(String),
}

impl AttackType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "direct" => Self::Direct,
            "area" => Self::Area,
            "projectile" => Self::Projectile,
            "beam" => Self::Beam,
            "hitscan" => Self::Hitscan,
            "guided" => Self::Guided,
            "ballistic" => Self::Ballistic,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Direct => "Direct",
            Self::Area => "Area",
            Self::Projectile => "Projectile",
            Self::Beam => "Beam",
            Self::Hitscan => "Hitscan",
            Self::Guided => "Guided",
            Self::Ballistic => "Ballistic",
            Self::Custom(name) => name,
        }
    }
}

/// Weapon firing effects
#[derive(Debug, Clone)]
pub struct FiringEffects {
    pub muzzle_flash: AsciiString,
    pub projectile_object: AsciiString,
    pub hit_effect: AsciiString,
    pub miss_effect: AsciiString,
    pub sound_effect: AsciiString,
    pub tracer_effect: AsciiString,
}

impl Default for FiringEffects {
    fn default() -> Self {
        Self {
            muzzle_flash: AsciiString::from(""),
            projectile_object: AsciiString::from(""),
            hit_effect: AsciiString::from(""),
            miss_effect: AsciiString::from(""),
            sound_effect: AsciiString::from(""),
            tracer_effect: AsciiString::from(""),
        }
    }
}

/// Weapon template definition
#[derive(Debug, Clone)]
pub struct WeaponTemplate {
    pub name: AsciiString,
    pub display_name: AsciiString,
    pub damage_type: DamageType,
    pub attack_type: AttackType,
    pub primary_damage: f32,
    pub secondary_damage: f32,
    pub damage_radius: f32,
    pub range: f32,
    pub min_range: f32,
    pub rate_of_fire: f32,
    pub reload_time: f32,
    pub accuracy: f32,
    pub projectile_speed: f32,
    pub acceptable_aim_delta: f32,
    pub min_weapon_speed: f32,
    pub scale_weapon_speed: bool,
    pub weapon_recoil: f32,
    pub min_target_pitch: f32,
    pub max_target_pitch: f32,
    pub radius_damage_angle: f32,
    pub fire_sound_loop_time: u32,
    pub continuous_fire_coast: u32,
    pub clip_reload_time: u32,
    pub auto_reload_when_idle: u32,
    /// C++ `m_antiMask`. Starts as `WEAPON_ANTI_GROUND` (0x02).
    pub anti_mask: u32,
    pub fire_fx: [Option<String>; 4],
    pub projectile_detonate_fx: [Option<String>; 4],
    pub fire_ocl: [Option<String>; 4],
    pub projectile_detonate_ocl: [Option<String>; 4],
    pub projectile_exhaust: [Option<String>; 4],
    pub clip_size: i32,
    pub continuous_fire_one: i32,
    pub continuous_fire_two: i32,
    pub shots_per_barrel: i32,
    pub historic_bonus_time: u32,
    pub historic_bonus_radius: f32,
    pub historic_bonus_count: i32,
    pub pre_attack_delay: u32,
    pub continue_attack_range: f32,
    pub suspend_fx_delay: u32,
    pub min_delay_between_shots: i32,
    pub max_delay_between_shots: i32,
    pub damage_dealt_at_self_position: bool,
    pub leech_range_weapon: bool,
    pub play_fx_when_stealthed: bool,
    pub die_on_detonate: bool,
    pub capable_of_following_waypoints: bool,
    pub shows_ammo_pips: bool,
    pub allow_attack_garrisoned_bldgs: bool,
    pub reload_type: i32,
    pub prefire_type: i32,
    pub damage_type_index: i32,
    pub damage_status_type: i32,
    pub death_type_index: i32,
    pub affects_mask: u32,
    pub collide_mask: u32,
    pub shockwave_amount: f32,
    pub shockwave_radius: f32,
    pub shockwave_taper_off: f32,
    pub projectile_stream_name: String,
    pub laser_name: String,
    pub laser_bone_name: String,
    pub historic_bonus_weapon: Option<String>,
    pub secondary_damage_radius: f32,
    pub request_assist_range: f32,
    pub scatter_radius: f32,
    pub scatter_target_scalar: f32,
    pub infantry_inaccuracy_dist: f32,
    pub projectile_count: u32,
    pub ammo_capacity: u32,
    pub penetration: f32,
    pub armor_piercing: f32,
    pub can_target_air: bool,
    pub can_target_ground: bool,
    pub can_target_water: bool,
    pub can_target_stealth: bool,
    pub can_fire_while_moving: bool,
    pub requires_los: bool, // Line of sight
    pub effects: FiringEffects,
    pub projectile_template: AsciiString,
    pub damage_fx_template: AsciiString,
    pub prerequisites: Vec<AsciiString>,
    pub properties: HashMap<String, String>,
    /// C++ `m_bonus[condition][field]`. Unset cells are `1.0`.
    pub weapon_bonus: [[f32; 5]; 27],
    /// Repeated `ScatterTarget = x y` lines (C++ appends).
    pub scatter_targets: Vec<(f32, f32)>,
}

impl WeaponTemplate {
    pub fn new(name: AsciiString) -> Self {
        // C++ WeaponTemplate::WeaponTemplate (Weapon.cpp:231-303)
        Self {
            name,
            display_name: AsciiString::from(""),
            damage_type: DamageType::Explosive,
            attack_type: AttackType::Direct,
            primary_damage: 0.0,
            secondary_damage: 0.0,
            damage_radius: 0.0,
            range: 0.0,
            min_range: 0.0,
            rate_of_fire: 0.0,
            reload_time: 0.0,
            accuracy: 1.0,
            projectile_speed: 999999.0,
            acceptable_aim_delta: 0.0,
            min_weapon_speed: 999999.0,
            scale_weapon_speed: false,
            weapon_recoil: 0.0,
            min_target_pitch: -std::f32::consts::PI,
            max_target_pitch: std::f32::consts::PI,
            radius_damage_angle: std::f32::consts::PI,
            fire_sound_loop_time: 0,
            continuous_fire_coast: 0,
            clip_reload_time: 0,
            anti_mask: 0x02,
            fire_fx: [None, None, None, None],
            projectile_detonate_fx: [None, None, None, None],
            fire_ocl: [None, None, None, None],
            projectile_detonate_ocl: [None, None, None, None],
            projectile_exhaust: [None, None, None, None],
            clip_size: 0,
            continuous_fire_one: i32::MAX,
            continuous_fire_two: i32::MAX,
            shots_per_barrel: 1,
            historic_bonus_time: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_count: 0,
            damage_type_index: 0,
            damage_status_type: 0,
            death_type_index: 0,
            pre_attack_delay: 0,
            continue_attack_range: 0.0,
            suspend_fx_delay: 0,
            min_delay_between_shots: 0,
            max_delay_between_shots: 0,
            damage_dealt_at_self_position: false,
            leech_range_weapon: false,
            play_fx_when_stealthed: false,
            die_on_detonate: false,
            capable_of_following_waypoints: false,
            shows_ammo_pips: false,
            allow_attack_garrisoned_bldgs: false,
            reload_type: 0,
            prefire_type: 0,
            affects_mask: 0x0E,
            collide_mask: 0x04,
            shockwave_amount: 0.0,
            shockwave_radius: 0.0,
            shockwave_taper_off: 0.0,
            projectile_stream_name: String::new(),
            laser_name: String::new(),
            laser_bone_name: String::new(),
            historic_bonus_weapon: None,
            secondary_damage_radius: 0.0,
            request_assist_range: 0.0,
            scatter_radius: 0.0,
            scatter_target_scalar: 0.0,
            infantry_inaccuracy_dist: 0.0,
            auto_reload_when_idle: 0,
            projectile_count: 1,
            ammo_capacity: 0,
            penetration: 0.0,
            armor_piercing: 1.0,
            can_target_air: false,
            can_target_ground: true,
            can_target_water: false,
            can_target_stealth: false,
            can_fire_while_moving: false,
            requires_los: true,
            effects: FiringEffects::default(),
            projectile_template: AsciiString::from(""),
            damage_fx_template: AsciiString::from(""),
            prerequisites: Vec::new(),
            properties: HashMap::new(),
            weapon_bonus: [[1.0; 5]; 27],
            scatter_targets: Vec::new(),
        }
    }

    /// Get the field parse table for this template
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        CPP_WEAPON_TEMPLATE_FIELDS
            .iter()
            .map(|field| {
                (
                    *field,
                    parse_cpp_weapon_field_for_table
                        as fn(&str) -> Result<Box<dyn std::any::Any>, String>,
                )
            })
            .collect()
    }

    /// Update template from properties
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> WeaponResult<()> {
        let mut entries: Vec<(&String, &String)> = properties.iter().collect();
        entries.sort_by(|a, b| weapon_property_order(a.0).cmp(&weapon_property_order(b.0)));
        for (key, value) in entries {
            let base_key = if let Some((base, repeat)) = key.rsplit_once('#') {
                if repeat.parse::<usize>().is_ok() {
                    base
                } else {
                    key.as_str()
                }
            } else {
                key.as_str()
            };
            match base_key {
                "DamageType" => {
                    self.damage_type_index = scan_index(value, DAMAGE_TYPE_NAMES, "DamageType")?;
                }
                "DamageStatusType" => {
                    self.damage_status_type =
                        scan_index(value, OBJECT_STATUS_NAMES, "DamageStatusType")?;
                }
                "DeathType" => {
                    self.death_type_index = scan_index(value, DEATH_TYPE_NAMES, "DeathType")?;
                }
                "PrimaryDamage" => {
                    self.primary_damage = parse_f32_field(base_key, value)?;
                }
                "SecondaryDamage" => {
                    self.secondary_damage = parse_f32_field(base_key, value)?;
                }
                "PrimaryDamageRadius" => {
                    self.damage_radius = parse_f32_field(base_key, value)?;
                }
                "AttackRange" => {
                    self.range = parse_f32_field(base_key, value)?;
                }
                "MinimumAttackRange" => {
                    self.min_range = parse_f32_field(base_key, value)?;
                }
                "WeaponSpeed" => {
                    // C++ Weapon.cpp:163 `INI::parseVelocityReal` stores dist/frame
                    // (`ConvertVelocityInSecsToFrames`, divide by 30). The ctor
                    // default 999999 is already in that unit and is not scaled.
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.projectile_speed = super::INI::parse_velocity_real(token).map_err(|_| {
                        WeaponError::ParseError(format!(
                            "Invalid {} value '{}': expected distance per second",
                            base_key, value
                        ))
                    })?;
                }
                "AcceptableAimDelta" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.acceptable_aim_delta = super::INI::parse_angle_real(token).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid AcceptableAimDelta '{}'", value))
                    })?;
                }
                "AntiAirborneVehicle"
                | "AntiGround"
                | "AntiProjectile"
                | "AntiSmallMissile"
                | "AntiMine"
                | "AntiAirborneInfantry"
                | "AntiBallisticMissile"
                | "AntiParachute" => {
                    apply_anti_mask_bit(&mut self.anti_mask, base_key, value)?;
                }
                "MinWeaponSpeed" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.min_weapon_speed = super::INI::parse_velocity_real(token).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid MinWeaponSpeed '{}'", value))
                    })?;
                }
                "ScaleWeaponSpeed" => {
                    self.scale_weapon_speed = parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "WeaponRecoil" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.weapon_recoil = super::INI::parse_angle_real(token).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid WeaponRecoil '{}'", value))
                    })?;
                }
                "MinTargetPitch" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.min_target_pitch = super::INI::parse_angle_real(token).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid MinTargetPitch '{}'", value))
                    })?;
                }
                "MaxTargetPitch" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.max_target_pitch = super::INI::parse_angle_real(token).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid MaxTargetPitch '{}'", value))
                    })?;
                }
                "RadiusDamageAngle" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.radius_damage_angle = super::INI::parse_angle_real(token).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid RadiusDamageAngle '{}'", value))
                    })?;
                }
                "FireSoundLoopTime" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.fire_sound_loop_time = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!("Invalid FireSoundLoopTime '{}'", value))
                        })?;
                }
                "ContinuousFireCoast" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.continuous_fire_coast = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!(
                                "Invalid ContinuousFireCoast '{}'",
                                value
                            ))
                        })?;
                }
                "AutoReloadWhenIdle" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.auto_reload_when_idle = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!(
                                "Invalid AutoReloadWhenIdle '{}'",
                                value
                            ))
                        })?;
                }
                "ClipReloadTime" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.clip_reload_time = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!("Invalid ClipReloadTime '{}'", value))
                        })?;
                }
                "FireFX" => {
                    let name = fx_list_name(value)?;
                    for slot in &mut self.fire_fx {
                        *slot = Some(name.clone());
                    }
                }
                "ProjectileDetonationFX" => {
                    let name = fx_list_name(value)?;
                    for slot in &mut self.projectile_detonate_fx {
                        *slot = Some(name.clone());
                    }
                }
                "VeterancyFireFX" => {
                    let (level, name) = vet_fx_list(value)?;
                    self.fire_fx[level] = Some(name);
                }
                "VeterancyProjectileDetonationFX" => {
                    let (level, name) = vet_fx_list(value)?;
                    self.projectile_detonate_fx[level] = Some(name);
                }
                "FireOCL" => fill_all_names(&mut self.fire_ocl, value)?,
                "ProjectileDetonationOCL" => {
                    fill_all_names(&mut self.projectile_detonate_ocl, value)?
                }
                "ProjectileExhaust" => fill_all_names(&mut self.projectile_exhaust, value)?,
                "VeterancyFireOCL" => {
                    let (level, name) = vet_fx_list(value)?;
                    self.fire_ocl[level] = Some(name);
                }
                "VeterancyProjectileDetonationOCL" => {
                    let (level, name) = vet_fx_list(value)?;
                    self.projectile_detonate_ocl[level] = Some(name);
                }
                "VeterancyProjectileExhaust" => {
                    let (level, name) = vet_fx_list(value)?;
                    self.projectile_exhaust[level] = Some(name);
                }
                "ClipSize" => {
                    self.clip_size = parse_i32_field(base_key, value)?;
                }
                "ContinuousFireOne" => {
                    self.continuous_fire_one = parse_i32_field(base_key, value)?;
                }
                "ContinuousFireTwo" => {
                    self.continuous_fire_two = parse_i32_field(base_key, value)?;
                }
                "ShotsPerBarrel" => {
                    self.shots_per_barrel = parse_i32_field(base_key, value)?;
                }
                "HistoricBonusTime" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.historic_bonus_time = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!("Invalid HistoricBonusTime '{}'", value))
                        })?;
                }
                "HistoricBonusRadius" => {
                    self.historic_bonus_radius = parse_f32_field(base_key, value)?;
                }
                "HistoricBonusCount" => {
                    self.historic_bonus_count = parse_i32_field(base_key, value)?;
                }
                "PreAttackDelay" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.pre_attack_delay = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!("Invalid PreAttackDelay '{}'", value))
                        })?;
                }
                "ContinueAttackRange" => {
                    self.continue_attack_range = parse_f32_field(base_key, value)?;
                }
                "SuspendFXDelay" => {
                    let token = value.split_whitespace().next().unwrap_or(value);
                    self.suspend_fx_delay = super::INI::parse_duration_unsigned_int(token)
                        .map_err(|_| {
                            WeaponError::ParseError(format!("Invalid SuspendFXDelay '{}'", value))
                        })?;
                }
                "DelayBetweenShots" => {
                    let (min_delay, max_delay) = parse_shot_delay(value)?;
                    self.min_delay_between_shots = min_delay;
                    self.max_delay_between_shots = max_delay;
                }
                "DamageDealtAtSelfPosition" => {
                    self.damage_dealt_at_self_position =
                        parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "LeechRangeWeapon" => {
                    self.leech_range_weapon = parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "PlayFXWhenStealthed" => {
                    self.play_fx_when_stealthed =
                        parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "MissileCallsOnDie" => {
                    self.die_on_detonate = parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "CapableOfFollowingWaypoints" => {
                    self.capable_of_following_waypoints =
                        parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "ShowsAmmoPips" => {
                    self.shows_ammo_pips = parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "AllowAttackGarrisonedBldgs" => {
                    self.allow_attack_garrisoned_bldgs =
                        parse_bool(value).map_err(WeaponError::ParseError)?;
                }
                "AutoReloadsClip" => {
                    self.reload_type = scan_index(
                        value,
                        &["YES", "NO", "RETURN_TO_BASE"],
                        "AutoReloadsClip",
                    )?;
                }
                "RadiusDamageAffects" => {
                    self.affects_mask = parse_bit_string(
                        value,
                        &[
                            "SELF",
                            "ALLIES",
                            "ENEMIES",
                            "NEUTRALS",
                            "SUICIDE",
                            "NOT_SIMILAR",
                            "NOT_AIRBORNE",
                        ],
                        self.affects_mask,
                        "RadiusDamageAffects",
                    )?;
                }
                "ProjectileCollidesWith" => {
                    self.collide_mask = parse_bit_string(
                        value,
                        &[
                            "ALLIES",
                            "ENEMIES",
                            "STRUCTURES",
                            "SHRUBBERY",
                            "PROJECTILES",
                            "WALLS",
                            "SMALL_MISSILES",
                            "BALLISTIC_MISSILES",
                            "CONTROLLED_STRUCTURES",
                        ],
                        self.collide_mask,
                        "ProjectileCollidesWith",
                    )?;
                }
                "PreAttackType" => {
                    self.prefire_type = scan_index(
                        value,
                        &["PER_SHOT", "PER_ATTACK", "PER_CLIP"],
                        "PreAttackType",
                    )?;
                }
                "ShockWaveAmount" => {
                    self.shockwave_amount = parse_f32_field(base_key, value)?;
                }
                "ShockWaveRadius" => {
                    self.shockwave_radius = parse_f32_field(base_key, value)?;
                }
                "ShockWaveTaperOff" => {
                    self.shockwave_taper_off = parse_f32_field(base_key, value)?;
                }
                "ProjectileStreamName" => {
                    self.projectile_stream_name = value.split_whitespace().next().unwrap_or("").to_string();
                }
                "LaserName" => {
                    self.laser_name = value.split_whitespace().next().unwrap_or("").to_string();
                }
                "LaserBoneName" => {
                    self.laser_bone_name = value.split_whitespace().next().unwrap_or("").to_string();
                }
                "HistoricBonusWeapon" => {
                    let name = value.split_whitespace().next().unwrap_or("");
                    if name.is_empty() {
                        return Err(WeaponError::ParseError(
                            "Invalid HistoricBonusWeapon: missing token".to_string(),
                        ));
                    }
                    self.historic_bonus_weapon = Some(name.to_string());
                }
                "SecondaryDamageRadius" => {
                    self.secondary_damage_radius = parse_f32_field(base_key, value)?;
                }
                "RequestAssistRange" => {
                    self.request_assist_range = parse_f32_field(base_key, value)?;
                }
                "ScatterRadius" => {
                    self.scatter_radius = parse_f32_field(base_key, value)?;
                }
                "ScatterTargetScalar" => {
                    self.scatter_target_scalar = parse_f32_field(base_key, value)?;
                }
                "ScatterRadiusVsInfantry" => {
                    self.infantry_inaccuracy_dist = parse_f32_field(base_key, value)?;
                }
                "ProjectileObject" => {
                    self.effects.projectile_object = AsciiString::from(value);
                }
                "FireSound" => {
                    self.effects.sound_effect = AsciiString::from(value);
                }
                "WeaponBonus" => {
                    let mut tokens = value.split_whitespace();
                    let condition = tokens.next().unwrap_or("");
                    let field = tokens.next().unwrap_or("");
                    let percent = tokens.next().unwrap_or("");
                    let condition = if condition.eq_ignore_ascii_case("DEMORALIZED") {
                        "DEMORALIZED_OBSOLETE"
                    } else {
                        condition
                    };
                    let condition = scan_index(condition, WEAPON_BONUS_CONDITIONS, "WeaponBonus")?;
                    let field = scan_index(field, WEAPON_BONUS_FIELDS, "WeaponBonus")?;
                    let percent = super::INI::parse_percent_to_real(percent).map_err(|_| {
                        WeaponError::ParseError(format!("Invalid WeaponBonus percent '{}'", value))
                    })?;
                    self.weapon_bonus[condition as usize][field as usize] = percent;
                }
                "ScatterTarget" => {
                    self.scatter_targets.push(parse_scatter_coord(value)?);
                }
                _ => {
                    if is_cpp_weapon_template_field(base_key) {
                        validate_unmodeled_cpp_weapon_field(base_key, value)?;
                        self.properties.insert(base_key.to_string(), value.clone());
                    } else {
                        return Err(WeaponError::ParseError(format!(
                            "Unknown weapon field '{}'",
                            key
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn is_valid(&self) -> bool {
        // C++ accepts utility/FX weapons with zero damage (for example
        // `ScorpionTankGunFXWeapon`) and data-only weapons with no conventional
        // attack range. Field parsers validate malformed values; the template
        // itself only requires a name.
        !self.name.is_empty()
    }

    pub fn is_area_weapon(&self) -> bool {
        self.damage_radius > 0.0 || self.attack_type == AttackType::Area
    }

    pub fn is_anti_air(&self) -> bool {
        self.can_target_air && !self.can_target_ground
    }

    pub fn is_anti_ground(&self) -> bool {
        self.can_target_ground && !self.can_target_air
    }

    pub fn is_dual_purpose(&self) -> bool {
        self.can_target_air && self.can_target_ground
    }

    pub fn can_target(&self, target_type: &str) -> bool {
        match target_type.to_lowercase().as_str() {
            "air" | "aircraft" => self.can_target_air,
            "ground" | "land" => self.can_target_ground,
            "water" | "naval" => self.can_target_water,
            "stealth" => self.can_target_stealth,
            _ => false,
        }
    }

    pub fn get_effective_damage(&self, armor: f32) -> f32 {
        let base_damage = self.primary_damage * self.armor_piercing;
        (base_damage - armor).max(0.0)
    }

    pub fn get_dps(&self) -> f32 {
        if self.rate_of_fire > 0.0 {
            self.primary_damage * self.rate_of_fire
        } else {
            0.0
        }
    }
}

/// Weapon store - manages all weapon templates
#[derive(Debug)]
pub struct WeaponStore {
    templates: HashMap<String, WeaponTemplate>,
    template_order: Vec<String>,
}

impl WeaponStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            template_order: Vec::new(),
        }
    }

    /// Find a template by name
    pub fn find_template(&self, name: &AsciiString) -> Option<&WeaponTemplate> {
        self.templates.get(name.as_str())
    }

    pub fn iter_templates(&self) -> impl Iterator<Item = &WeaponTemplate> {
        self.templates.values()
    }

    /// Find a mutable template by name
    pub fn find_template_mut(&mut self, name: &AsciiString) -> Option<&mut WeaponTemplate> {
        self.templates.get_mut(name.as_str())
    }

    /// Create a new template
    pub fn new_template(&mut self, name: AsciiString) -> &mut WeaponTemplate {
        let template = WeaponTemplate::new(name.clone());
        let key = name.as_str().to_string();
        if !self.templates.contains_key(&key) {
            self.template_order.push(key.clone());
        }
        self.templates.insert(key, template);
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Get or create a template
    pub fn get_or_create_template(&mut self, name: &AsciiString) -> &mut WeaponTemplate {
        if !self.templates.contains_key(name.as_str()) {
            self.new_template(name.clone());
        }
        self.templates.get_mut(name.as_str()).unwrap()
    }

    /// Register a template
    pub fn register_template(&mut self, template: WeaponTemplate) {
        let name = template.name.as_str().to_string();
        if !self.templates.contains_key(&name) {
            self.template_order.push(name.clone());
        }
        self.templates.insert(name, template);
    }

    pub fn register_definition(
        &mut self,
        name: AsciiString,
        properties: &HashMap<String, String>,
        load_type: INILoadType,
    ) -> WeaponResult<()> {
        let mut template = if let Some(existing) = self.find_template(&name).cloned() {
            if load_type != INILoadType::CreateOverrides {
                return Err(WeaponError::AlreadyExists);
            }
            existing
        } else {
            WeaponTemplate::new(name)
        };

        template.update_from_properties(properties)?;
        if !template.is_valid() {
            return Err(WeaponError::ParseError(
                "Invalid weapon template configuration".to_string(),
            ));
        }

        self.register_template(template);
        Ok(())
    }

    /// Get all template names
    pub fn get_template_names(&self) -> Vec<&String> {
        self.template_order
            .iter()
            .filter(|name| self.templates.contains_key(name.as_str()))
            .collect()
    }

    /// Get templates by damage type
    pub fn get_templates_by_damage_type(&self, damage_type: &DamageType) -> Vec<&WeaponTemplate> {
        self.template_order
            .iter()
            .filter_map(|name| self.templates.get(name.as_str()))
            .filter(|t| &t.damage_type == damage_type)
            .collect()
    }

    /// Get templates by attack type
    pub fn get_templates_by_attack_type(&self, attack_type: &AttackType) -> Vec<&WeaponTemplate> {
        self.template_order
            .iter()
            .filter_map(|name| self.templates.get(name.as_str()))
            .filter(|t| &t.attack_type == attack_type)
            .collect()
    }

    /// Remove a template
    pub fn remove_template(&mut self, name: &AsciiString) -> bool {
        let removed = self.templates.remove(name.as_str()).is_some();
        if removed {
            self.template_order
                .retain(|template_name| template_name != name.as_str());
        }
        removed
    }

    /// Clear all templates
    pub fn clear(&mut self) {
        self.templates.clear();
        self.template_order.clear();
    }

    /// Get template count
    pub fn get_template_count(&self) -> usize {
        self.templates.len()
    }

    /// Parse weapon template definition - equivalent to original parseWeaponTemplateDefinition
    pub fn parse_weapon_template_definition(name: AsciiString) -> WeaponResult<()> {
        // In the original C++, this would delegate to WeaponStore::parseWeaponTemplateDefinition
        println!("Parsing weapon template definition for: {}", name.as_str());
        Ok(())
    }
}

impl Default for WeaponStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global weapon store instance
static WEAPON_STORE: OnceCell<RwLock<WeaponStore>> = OnceCell::new();

/// Initialize the global weapon store
pub fn initialize_weapon_store() {
    WEAPON_STORE.get_or_init(|| RwLock::new(WeaponStore::new()));
}

/// Explicitly clear registered weapon definitions.
///
/// Initialization must be idempotent because every parsed `Weapon` calls it;
/// resetting there previously discarded the preceding retail definition and
/// left the store containing only the final weapon.
pub fn reset_weapon_store() {
    initialize_weapon_store();
    if let Some(store) = WEAPON_STORE.get() {
        if let Ok(mut guard) = store.write() {
            guard.clear();
        }
    }
}

/// Get a reference to the global weapon store
pub fn get_weapon_store() -> Option<RwLockWriteGuard<'static, WeaponStore>> {
    WEAPON_STORE
        .get()
        .map(|store| store.write().expect("WeaponStore poisoned"))
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    // C++ token parsers consume the first value token. This also preserves
    // compatibility with the retail Humvee weapon line whose explanatory text
    // accidentally lacks a leading semicolon.
    let value = value.split_whitespace().next().unwrap_or(value);
    match value.trim().to_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

fn scan_index(value: &str, names: &[&str], field: &str) -> WeaponResult<i32> {
    let token = value.split_whitespace().next().unwrap_or(value);
    let upper = token.to_ascii_uppercase();
    for (index, name) in names.iter().enumerate() {
        if upper == *name {
            return Ok(index as i32);
        }
    }
    Err(WeaponError::ParseError(format!(
        "Invalid {} value '{}'",
        field, token
    )))
}

fn delay_frames(token: &str) -> WeaponResult<i32> {
    let msecs: i32 = token.parse().map_err(|_| {
        WeaponError::ParseError(format!("Invalid DelayBetweenShots '{}'", token))
    })?;
    Ok(super::INI::convert_duration_msecs_to_frames(msecs as f32).ceil() as i32)
}

fn parse_shot_delay(value: &str) -> WeaponResult<(i32, i32)> {
    let tokens: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ':')
        .filter(|token| !token.is_empty())
        .collect();
    let Some(first) = tokens.first().copied() else {
        return Err(WeaponError::ParseError(
            "Invalid DelayBetweenShots: missing token".to_string(),
        ));
    };
    if first.eq_ignore_ascii_case("Min") {
        let Some(min_token) = tokens.get(1) else {
            return Err(WeaponError::ParseError(
                "Invalid DelayBetweenShots: missing Min".to_string(),
            ));
        };
        let min_delay = delay_frames(min_token)?;
        if tokens.get(2).is_some_and(|token| token.eq_ignore_ascii_case("Max")) {
            let Some(max_token) = tokens.get(3) else {
                return Err(WeaponError::ParseError(
                    "Invalid DelayBetweenShots: missing Max".to_string(),
                ));
            };
            Ok((min_delay, delay_frames(max_token)?))
        } else {
            Ok((min_delay, min_delay))
        }
    } else {
        let frames = delay_frames(first)?;
        Ok((frames, frames))
    }
}

const WEAPON_BONUS_CONDITIONS: &[&str] = &[
    "GARRISONED", "HORDE", "CONTINUOUS_FIRE_MEAN", "CONTINUOUS_FIRE_FAST", "NATIONALISM",
    "PLAYER_UPGRADE", "DRONE_SPOTTING", "DEMORALIZED_OBSOLETE", "ENTHUSIASTIC", "VETERAN",
    "ELITE", "HERO", "BATTLEPLAN_BOMBARDMENT", "BATTLEPLAN_HOLDTHELINE",
    "BATTLEPLAN_SEARCHANDDESTROY", "SUBLIMINAL", "SOLO_HUMAN_EASY", "SOLO_HUMAN_NORMAL",
    "SOLO_HUMAN_HARD", "SOLO_AI_EASY", "SOLO_AI_NORMAL", "SOLO_AI_HARD", "TARGET_FAERIE_FIRE",
    "FANATICISM", "FRENZY_ONE", "FRENZY_TWO", "FRENZY_THREE",
];
const WEAPON_BONUS_FIELDS: &[&str] = &["DAMAGE", "RADIUS", "RANGE", "RATE_OF_FIRE", "PRE_ATTACK"];

fn parse_scatter_coord(value: &str) -> WeaponResult<(f32, f32)> {
    let mut x = None;
    let mut y = None;
    for token in value.split_whitespace() {
        let Some((label, number)) = token.split_once(':') else {
            return Err(WeaponError::ParseError(format!(
                "Invalid ScatterTarget '{}'",
                value
            )));
        };
        let number = number.parse::<f32>().map_err(|_| {
            WeaponError::ParseError(format!("Invalid ScatterTarget '{}'", value))
        })?;
        match label.to_ascii_uppercase().as_str() {
            "X" => x = Some(number),
            "Y" => y = Some(number),
            _ => {
                return Err(WeaponError::ParseError(format!(
                    "Invalid ScatterTarget '{}'",
                    value
                )))
            }
        }
    }
    match (x, y) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(WeaponError::ParseError(format!(
            "Invalid ScatterTarget '{}': expected X: and Y:",
            value
        ))),
    }
}

const DAMAGE_TYPE_NAMES: &[&str] = &[
    "EXPLOSION", "CRUSH", "ARMOR_PIERCING", "SMALL_ARMS", "GATTLING", "RADIATION", "FLAME",
    "LASER", "SNIPER", "POISON", "HEALING", "UNRESISTABLE", "WATER", "DEPLOY", "SURRENDER",
    "HACK", "KILL_PILOT", "PENALTY", "FALLING", "MELEE", "DISARM", "HAZARD_CLEANUP",
    "PARTICLE_BEAM", "TOPPLING", "INFANTRY_MISSILE", "AURORA_BOMB", "LAND_MINE", "JET_MISSILES",
    "STEALTHJET_MISSILES", "MOLOTOV_COCKTAIL", "COMANCHE_VULCAN", "SUBDUAL_MISSILE",
    "SUBDUAL_VEHICLE", "SUBDUAL_BUILDING", "SUBDUAL_UNRESISTABLE", "MICROWAVE", "KILL_GARRISONED",
    "STATUS",
];
const DEATH_TYPE_NAMES: &[&str] = &[
    "NORMAL", "NONE", "CRUSHED", "BURNED", "EXPLODED", "POISONED", "TOPPLED", "FLOODED",
    "SUICIDED", "LASERED", "DETONATED", "SPLATTED", "POISONED_BETA", "EXTRA_2", "EXTRA_3",
    "EXTRA_4", "EXTRA_5", "EXTRA_6", "EXTRA_7", "EXTRA_8", "POISONED_GAMMA",
];
const OBJECT_STATUS_NAMES: &[&str] = &[
    "NONE", "DESTROYED", "CAN_ATTACK", "UNDER_CONSTRUCTION", "UNSELECTABLE", "NO_COLLISIONS",
    "NO_ATTACK", "AIRBORNE_TARGET", "PARACHUTING", "REPULSOR", "HIJACKED", "AFLAME", "BURNED",
    "WET", "IS_FIRING_WEAPON", "IS_BRAKING", "STEALTHED", "DETECTED", "CAN_STEALTH", "SOLD",
    "UNDERGOING_REPAIR", "RECONSTRUCTING", "MASKED", "IS_ATTACKING", "USING_ABILITY",
    "IS_AIMING_WEAPON", "NO_ATTACK_FROM_AI", "IGNORING_STEALTH", "IS_CARBOMB", "DECK_HEIGHT_OFFSET",
    "STATUS_RIDER1", "STATUS_RIDER2", "STATUS_RIDER3", "STATUS_RIDER4", "STATUS_RIDER5",
    "STATUS_RIDER6", "STATUS_RIDER7", "STATUS_RIDER8", "FAERIE_FIRE", "KILLING_SELF",
    "REASSIGN_PARKING", "BOOBY_TRAPPED", "IMMOBILE", "DISGUISED", "DEPLOYED",
];

fn parse_bit_string(
    value: &str,
    names: &[&str],
    mut bits: u32,
    field: &str,
) -> WeaponResult<u32> {
    let mut found_normal = false;
    let mut found_add_or_sub = false;
    for token in value.split_whitespace() {
        if token.eq_ignore_ascii_case("NONE") {
            if found_normal || found_add_or_sub {
                return Err(WeaponError::ParseError(format!(
                    "Invalid {} value: mixed NONE",
                    field
                )));
            }
            return Ok(0);
        }
        let (op, name) = if let Some(rest) = token.strip_prefix('+') {
            if found_normal {
                return Err(WeaponError::ParseError(format!(
                    "Invalid {} value: mixed +/-",
                    field
                )));
            }
            found_add_or_sub = true;
            (1i32, rest)
        } else if let Some(rest) = token.strip_prefix('-') {
            if found_normal {
                return Err(WeaponError::ParseError(format!(
                    "Invalid {} value: mixed +/-",
                    field
                )));
            }
            found_add_or_sub = true;
            (-1, rest)
        } else {
            if found_add_or_sub {
                return Err(WeaponError::ParseError(format!(
                    "Invalid {} value: mixed +/-",
                    field
                )));
            }
            if !found_normal {
                bits = 0;
                found_normal = true;
            }
            (1, token)
        };
        let upper = name.to_ascii_uppercase();
        let Some(index) = names.iter().position(|candidate| *candidate == upper) else {
            return Err(WeaponError::ParseError(format!(
                "Invalid {} value '{}'",
                field, token
            )));
        };
        let bit = 1u32 << index;
        if op < 0 {
            bits &= !bit;
        } else {
            bits |= bit;
        }
    }
    Ok(bits)
}

fn parse_f32_field(field_name: &str, value: &str) -> WeaponResult<f32> {
    let token = value.split_whitespace().next().unwrap_or(value);
    token.parse::<f32>().map_err(|e| {
        WeaponError::ParseError(format!("Invalid {} value '{}': {}", field_name, value, e))
    })
}

fn parse_u32_field(field_name: &str, value: &str) -> WeaponResult<u32> {
    let token = value.split_whitespace().next().unwrap_or(value);
    token.parse::<u32>().map_err(|e| {
        WeaponError::ParseError(format!("Invalid {} value '{}': {}", field_name, value, e))
    })
}

fn parse_i32_field(field_name: &str, value: &str) -> WeaponResult<i32> {
    let token = value.split_whitespace().next().unwrap_or(value);
    token.parse::<i32>().map_err(|e| {
        WeaponError::ParseError(format!("Invalid {} value '{}': {}", field_name, value, e))
    })
}

fn validate_unmodeled_cpp_weapon_field(field_name: &str, value: &str) -> WeaponResult<()> {
    if value.trim().is_empty() {
        return Err(WeaponError::ParseError(format!(
            "Invalid {} value: missing token",
            field_name
        )));
    }
    Ok(())
}

fn apply_anti_mask_bit(mask: &mut u32, field: &str, value: &str) -> WeaponResult<()> {
    let bit = match field {
        "AntiAirborneVehicle" => 0x01,
        "AntiGround" => 0x02,
        "AntiProjectile" => 0x04,
        "AntiSmallMissile" => 0x08,
        "AntiMine" => 0x10,
        "AntiAirborneInfantry" => 0x20,
        "AntiBallisticMissile" => 0x40,
        "AntiParachute" => 0x80,
        _ => {
            return Err(WeaponError::ParseError(format!(
                "Unknown anti mask '{}'",
                field
            )))
        }
    };
    let token = value.split_whitespace().next().unwrap_or(value);
    match token {
        "Yes" | "yes" | "YES" => *mask |= bit,
        "No" | "no" | "NO" => *mask &= !bit,
        _ => {
            return Err(WeaponError::ParseError(format!(
                "Invalid {} value '{}': expected Yes or No",
                field, value
            )))
        }
    }
    Ok(())
}

fn fx_list_name(value: &str) -> WeaponResult<String> {
    let name = value.split_whitespace().next().unwrap_or("").to_string();
    if name.is_empty() {
        return Err(WeaponError::ParseError(
            "Invalid FX list: missing token".to_string(),
        ));
    }
    Ok(name)
}

fn fill_all_names(slots: &mut [Option<String>; 4], value: &str) -> WeaponResult<()> {
    let name = fx_list_name(value)?;
    for slot in slots {
        *slot = Some(name.clone());
    }
    Ok(())
}

fn vet_fx_list(value: &str) -> WeaponResult<(usize, String)> {
    let mut tokens = value.split_whitespace();
    let level = tokens.next().unwrap_or("");
    let name = tokens.next().unwrap_or("").to_string();
    let index = match level.to_ascii_uppercase().as_str() {
        "REGULAR" => 0,
        "VETERAN" => 1,
        "ELITE" => 2,
        "HEROIC" => 3,
        _ => {
            return Err(WeaponError::ParseError(format!(
                "Invalid veterancy '{}'",
                level
            )))
        }
    };
    if name.is_empty() {
        return Err(WeaponError::ParseError(
            "Invalid veterancy FX list: missing name".to_string(),
        ));
    }
    Ok((index, name))
}

fn weapon_property_order(key: &str) -> (u32, &str) {
    let (base, index) = if let Some((base, repeat)) = key.rsplit_once('#') {
        if let Ok(index) = repeat.parse::<u32>() {
            (base, index)
        } else {
            (key, 0)
        }
    } else {
        (key, 0)
    };
    let slot = matches!(
        base,
        "FireFX"
            | "ProjectileDetonationFX"
            | "FireOCL"
            | "ProjectileDetonationOCL"
            | "ProjectileExhaust"
            | "VeterancyFireFX"
            | "VeterancyProjectileDetonationFX"
            | "VeterancyFireOCL"
            | "VeterancyProjectileDetonationOCL"
            | "VeterancyProjectileExhaust"
    );
    if slot {
        (index, "")
    } else {
        (index, key)
    }
}


/// INI parsing functions for weapons
pub struct IniWeapon;

impl IniWeapon {
    /// Parse weapon template definition - equivalent to INI::parseWeaponTemplateDefinition
    pub fn parse_weapon_template_definition(name: AsciiString) -> WeaponResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(WeaponError::InvalidName);
        }

        // Initialize weapon store if needed
        initialize_weapon_store();

        // Delegate to WeaponStore
        WeaponStore::parse_weapon_template_definition(name)
    }

    /// Parse a complete weapon template block from INI data
    pub fn parse_weapon_template_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> WeaponResult<WeaponTemplate> {
        // Validate name
        if name.is_empty() {
            return Err(WeaponError::InvalidName);
        }

        // Create template
        let mut template = WeaponTemplate::new(name);

        // Update template from properties
        template.update_from_properties(&properties)?;

        // Validate template
        if !template.is_valid() {
            return Err(WeaponError::ParseError(
                "Invalid weapon template configuration".to_string(),
            ));
        }

        Ok(template)
    }

    /// Register a weapon template
    pub fn register_template(template: WeaponTemplate) -> WeaponResult<()> {
        initialize_weapon_store();

        let mut store = get_weapon_store()
            .ok_or_else(|| WeaponError::StoreError("Store not initialized".to_string()))?;

        store.register_template(template);
        Ok(())
    }

    pub fn register_definition(
        name: AsciiString,
        properties: HashMap<String, String>,
        load_type: INILoadType,
    ) -> WeaponResult<()> {
        if name.is_empty() {
            return Err(WeaponError::InvalidName);
        }

        initialize_weapon_store();

        let mut store = get_weapon_store()
            .ok_or_else(|| WeaponError::StoreError("Store not initialized".to_string()))?;
        store.register_definition(name, &properties, load_type)
    }

    /// Find a weapon template by name
    pub fn find_template_by_name(name: &AsciiString) -> Option<WeaponTemplate> {
        if let Some(store) = get_weapon_store() {
            store.find_template(name).cloned()
        } else {
            None
        }
    }

    /// Validate weapon name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_type_parsing() {
        assert_eq!(DamageType::from_string("explosive"), DamageType::Explosive);
        assert_eq!(DamageType::from_string("LASER"), DamageType::Laser);
        assert_eq!(
            DamageType::from_string("CustomDamage"),
            DamageType::Custom("CustomDamage".to_string())
        );
    }

    #[test]
    fn test_attack_type_parsing() {
        assert_eq!(AttackType::from_string("area"), AttackType::Area);
        assert_eq!(
            AttackType::from_string("PROJECTILE"),
            AttackType::Projectile
        );
        assert_eq!(
            AttackType::from_string("CustomAttack"),
            AttackType::Custom("CustomAttack".to_string())
        );
    }

    #[test]
    fn test_weapon_template_creation() {
        // C++ WeaponTemplate::WeaponTemplate (Weapon.cpp:231-303)
        let name = AsciiString::from("TestWeapon");
        let template = WeaponTemplate::new(name.clone());

        assert_eq!(template.name, name);
        assert_eq!(template.primary_damage, 0.0);
        assert_eq!(template.range, 0.0);
        assert_eq!(template.projectile_speed, 999999.0);
        assert!(matches!(template.damage_type, DamageType::Explosive));
        assert!(!template.can_target_air);
        assert!(template.can_target_ground);
        assert!(template.is_valid());
    }

    #[test]
    fn test_weapon_store() {
        let mut store = WeaponStore::new();
        let name = AsciiString::from("TestWeapon");

        // Create new template
        let template = store.new_template(name.clone());
        template.damage_type = DamageType::Explosive;
        template.primary_damage = 50.0;
        template.damage_radius = 20.0;

        // Find template
        let found = store.find_template(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().primary_damage, 50.0);
        assert!(matches!(found.unwrap().damage_type, DamageType::Explosive));
        assert!(found.unwrap().is_area_weapon());

        // Count templates
        assert_eq!(store.get_template_count(), 1);
    }

    #[test]
    fn weapon_store_enumerates_in_registration_order() {
        let mut store = WeaponStore::new();

        let mut first = WeaponTemplate::new(AsciiString::from("FirstWeapon"));
        first.damage_type = DamageType::Explosive;
        first.attack_type = AttackType::Projectile;
        let mut second = WeaponTemplate::new(AsciiString::from("SecondWeapon"));
        second.damage_type = DamageType::Laser;
        second.attack_type = AttackType::Projectile;
        let mut third = WeaponTemplate::new(AsciiString::from("ThirdWeapon"));
        third.damage_type = DamageType::Explosive;
        third.attack_type = AttackType::Beam;

        store.register_template(first);
        store.register_template(second);
        store.register_template(third);

        let names: Vec<&str> = store
            .get_template_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["FirstWeapon", "SecondWeapon", "ThirdWeapon"]);

        let explosive_names: Vec<&str> = store
            .get_templates_by_damage_type(&DamageType::Explosive)
            .into_iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(explosive_names, vec!["FirstWeapon", "ThirdWeapon"]);

        let projectile_names: Vec<&str> = store
            .get_templates_by_attack_type(&AttackType::Projectile)
            .into_iter()
            .map(|template| template.name.as_str())
            .collect();
        assert_eq!(projectile_names, vec!["FirstWeapon", "SecondWeapon"]);
    }

    #[test]
    fn weapon_definition_rejects_duplicate_without_override_load() {
        let mut store = WeaponStore::new();
        let name = AsciiString::from("ExistingWeapon");
        let mut properties = HashMap::new();
        properties.insert("PrimaryDamage".to_string(), "25".to_string());

        store
            .register_definition(name.clone(), &properties, INILoadType::Overwrite)
            .unwrap();

        let result = store.register_definition(name, &properties, INILoadType::Overwrite);
        assert_eq!(result, Err(WeaponError::AlreadyExists));
        assert_eq!(store.get_template_count(), 1);
    }

    #[test]
    fn weapon_override_preserves_existing_order_and_fields() {
        let mut store = WeaponStore::new();

        let first_name = AsciiString::from("FirstWeapon");
        let second_name = AsciiString::from("SecondWeapon");
        let mut first_properties = HashMap::new();
        first_properties.insert("PrimaryDamage".to_string(), "25".to_string());
        first_properties.insert("AttackRange".to_string(), "100".to_string());
        let mut second_properties = HashMap::new();
        second_properties.insert("PrimaryDamage".to_string(), "50".to_string());
        second_properties.insert("AttackRange".to_string(), "150".to_string());

        store
            .register_definition(
                first_name.clone(),
                &first_properties,
                INILoadType::Overwrite,
            )
            .unwrap();
        store
            .register_definition(
                second_name.clone(),
                &second_properties,
                INILoadType::Overwrite,
            )
            .unwrap();

        let mut override_properties = HashMap::new();
        override_properties.insert("PrimaryDamage".to_string(), "77".to_string());
        store
            .register_definition(
                first_name.clone(),
                &override_properties,
                INILoadType::CreateOverrides,
            )
            .unwrap();

        let first = store.find_template(&first_name).unwrap();
        assert_eq!(first.primary_damage, 77.0);
        assert_eq!(first.range, 100.0);
        assert_eq!(
            store
                .get_template_names()
                .into_iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["FirstWeapon", "SecondWeapon"]
        );
        assert_eq!(store.get_template_count(), 2);
    }

    #[test]
    fn test_weapon_capabilities() {
        let mut template = WeaponTemplate::new(AsciiString::from("TestWeapon"));
        template.can_target_air = true;
        template.can_target_ground = false;
        template.damage_radius = 15.0;
        template.primary_damage = 25.0;
        template.rate_of_fire = 2.0;

        assert!(template.is_anti_air());
        assert!(!template.is_anti_ground());
        assert!(!template.is_dual_purpose());
        assert!(template.is_area_weapon());
        assert!(template.can_target("air"));
        assert!(!template.can_target("ground"));
        assert_eq!(template.get_dps(), 50.0);
    }

    #[test]
    fn repeated_weapon_bonus_and_scatter_lines_are_kept() {
        let mut properties = HashMap::new();
        properties.insert(
            "WeaponBonus".to_string(),
            "DRONE_SPOTTING RATE_OF_FIRE 200%".to_string(),
        );
        properties.insert(
            "WeaponBonus#1".to_string(),
            "DRONE_SPOTTING RANGE 200%".to_string(),
        );
        properties.insert(
            "WeaponBonus#2".to_string(),
            "DRONE_SPOTTING DAMAGE 200%".to_string(),
        );
        properties.insert("ScatterTarget".to_string(), "X:0.0 Y:0.0".to_string());
        properties.insert("ScatterTarget#1".to_string(), "X:1.0 Y:0.5".to_string());

        let mut template = WeaponTemplate::new(AsciiString::from("RangerACR"));
        template
            .update_from_properties(&properties)
            .expect("weapon fields");
        assert_eq!(template.weapon_bonus[6][3], 2.0);
        assert_eq!(template.weapon_bonus[6][2], 2.0);
        assert_eq!(template.weapon_bonus[6][0], 2.0);
        assert_eq!(template.weapon_bonus[0][0], 1.0);
        assert_eq!(template.scatter_targets, vec![(0.0, 0.0), (1.0, 0.5)]);
    }

    #[test]
    fn test_effective_damage_calculation() {
        let mut template = WeaponTemplate::new(AsciiString::from("TestWeapon"));
        template.primary_damage = 100.0;
        template.armor_piercing = 0.8;

        let damage_vs_light_armor = template.get_effective_damage(10.0);
        let damage_vs_heavy_armor = template.get_effective_damage(90.0);
        let damage_vs_super_armor = template.get_effective_damage(200.0);

        assert_eq!(damage_vs_light_armor, 70.0); // 100 * 0.8 - 10
        assert_eq!(damage_vs_heavy_armor, 0.0); // Max of (80 - 90, 0)
        assert_eq!(damage_vs_super_armor, 0.0); // Max of (80 - 200, 0)
    }

    #[test]
    fn test_template_properties_update() {
        let mut template = WeaponTemplate::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("DamageType".to_string(), "FLAME".to_string());
        properties.insert("PrimaryDamage".to_string(), "75.0".to_string());
        properties.insert("AttackRange".to_string(), "200.0".to_string());
        properties.insert("WeaponSpeed".to_string(), "400.0".to_string());
        properties.insert("ProjectileObject".to_string(), "TestProjectile".to_string());
        properties.insert("FireSound".to_string(), "WeaponFire".to_string());

        template.update_from_properties(&properties).unwrap();

        assert_eq!(template.damage_type_index, 6);
        assert_eq!(template.primary_damage, 75.0);
        assert_eq!(template.range, 200.0);
        assert_eq!(template.projectile_speed, 400.0 / 30.0);
        assert_eq!(
            template.effects.projectile_object.as_str(),
            "TestProjectile"
        );
        assert_eq!(template.effects.sound_effect.as_str(), "WeaponFire");
    }

    #[test]
    fn weapon_template_accepts_cpp_weapon_field_names() {
        let mut properties = HashMap::new();
        properties.insert("PrimaryDamage".to_string(), "125.0".to_string());
        properties.insert("PrimaryDamageRadius".to_string(), "20.5".to_string());
        properties.insert("AttackRange".to_string(), "260.0".to_string());
        properties.insert("MinimumAttackRange".to_string(), "35.0".to_string());
        properties.insert("WeaponSpeed".to_string(), "999.0".to_string());
        properties.insert("ProjectileObject".to_string(), "TestProjectile".to_string());
        properties.insert("FireSound".to_string(), "TestWeaponFire".to_string());
        properties.insert("RequestAssistRange".to_string(), "300.0".to_string());
        properties.insert("SecondaryDamageRadius".to_string(), "12.0".to_string());

        let template =
            IniWeapon::parse_weapon_template_block(AsciiString::from("CxxWeapon"), properties)
                .unwrap();

        assert_eq!(template.primary_damage, 125.0);
        assert_eq!(template.damage_radius, 20.5);
        assert_eq!(template.range, 260.0);
        assert_eq!(template.min_range, 35.0);
        assert_eq!(template.projectile_speed, 999.0 / 30.0);
        assert_eq!(
            template.effects.projectile_object.as_str(),
            "TestProjectile"
        );
        assert_eq!(template.effects.sound_effect.as_str(), "TestWeaponFire");
        assert!(!template.properties.contains_key("PrimaryDamageRadius"));
        assert!(!template.properties.contains_key("AttackRange"));
        assert!(!template.properties.contains_key("MinimumAttackRange"));
        assert!(!template.properties.contains_key("WeaponSpeed"));
        assert_eq!(template.request_assist_range, 300.0);
        assert_eq!(template.secondary_damage_radius, 12.0);
        assert!(!template.properties.contains_key("RequestAssistRange"));
        assert!(!template.properties.contains_key("SecondaryDamageRadius"));
    }

    #[test]
    fn weapon_block_rejects_invalid_parsed_field_values() {
        let mut properties = HashMap::new();
        properties.insert("PrimaryDamage".to_string(), "heavy".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(AsciiString::from("BadDamage"), properties)
                .is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("WeaponSpeed".to_string(), "fast".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(AsciiString::from("BadWeaponSpeed"), properties)
                .is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("RequestAssistRange".to_string(), "far".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(
                AsciiString::from("BadRequestAssistRange"),
                properties
            )
            .is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("ScaleWeaponSpeed".to_string(), "sometimes".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(
                AsciiString::from("BadScaleWeaponSpeed"),
                properties
            )
            .is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("ClipSize".to_string(), "many".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(AsciiString::from("BadClipSize"), properties)
                .is_err()
        );
    }

    #[test]
    fn weapon_block_rejects_fields_outside_cpp_parse_table() {
        let mut properties = HashMap::new();
        properties.insert("Range".to_string(), "200.0".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(AsciiString::from("RustRange"), properties)
                .is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("CanTargetAir".to_string(), "false".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(
                AsciiString::from("RustCanTargetAir"),
                properties
            )
            .is_err()
        );

        let mut properties = HashMap::new();
        properties.insert("TotallyUnknown".to_string(), "value".to_string());
        assert!(
            IniWeapon::parse_weapon_template_block(
                AsciiString::from("UnknownWeaponField"),
                properties
            )
            .is_err()
        );
    }

    #[test]
    fn test_firing_effects() {
        let mut template = WeaponTemplate::new(AsciiString::from("TestWeapon"));
        template.effects.muzzle_flash = AsciiString::from("MuzzleFlash01");
        template.effects.hit_effect = AsciiString::from("ExplosionSmall");
        template.effects.sound_effect = AsciiString::from("WeaponFire");

        assert_eq!(template.effects.muzzle_flash.as_str(), "MuzzleFlash01");
        assert_eq!(template.effects.hit_effect.as_str(), "ExplosionSmall");
        assert_eq!(template.effects.sound_effect.as_str(), "WeaponFire");
    }

    #[test]
    fn test_parse_bool() {
        // C++ INI.cpp:584-626 accepts only Yes/No, ignoring case.
        for value in ["yes", "YES", "Yes"] {
            assert_eq!(parse_bool(value), Ok(true));
        }
        for value in ["no", "NO", "No"] {
            assert_eq!(parse_bool(value), Ok(false));
        }
        assert_eq!(parse_bool("Yes trailing text"), Ok(true));
        assert_eq!(parse_bool("No trailing text"), Ok(false));
        for invalid in ["true", "TRUE", "1", "false", "FALSE", "0", "invalid", ""] {
            assert!(parse_bool(invalid).is_err());
        }
    }

    #[test]
    fn test_validate_name() {
        assert!(IniWeapon::validate_name(&AsciiString::from("ValidName")));
        assert!(!IniWeapon::validate_name(&AsciiString::from("")));
    }
}

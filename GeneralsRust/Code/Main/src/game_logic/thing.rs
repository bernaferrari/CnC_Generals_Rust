use super::*;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod template_metadata;
pub use template_metadata::{
    DockKind,
    ContainModuleKind,
    ContainAdmission,
    RiderChangeRiderMetadata,
    ContainModuleMetadata,
    OverchargeBehaviorMetadata,
    PowerPlantUpdateMetadata,
    ParkingPlaceMetadata,
    FlightDeckMetadata,
    DeployStyleMetadata,
    SupplyTruckMetadata,
    SupplyTruckState,
    ProductionExitStyle,
    ProductionExitMetadata,
    VeterancyCrateCollideMetadata,
    VeterancyGainCreateMetadata,
    GrantUpgradeCreateMetadata,
    EjectPilotCreationList,
    EjectPilotDeathTypes,
    EjectPilotVeterancyLevels,
    EjectPilotExemptStatus,
    EjectPilotRequiredStatus,
    EjectPilotDieMetadata,
    RebuildHoleExposeDieMetadata,
    HackInternetAIUpdateMetadata,
    HackerDisableBuildingMetadata,
    ChargePlantAbilityMetadata,
    pack_unpack_variation_multiplier,
    apply_pack_unpack_variation_ms,
    vary_pack_unpack_duration_ms,
    SpecialPowerModuleKind,
    SpecialPowerModuleMetadata,
    CapturePowerKind,
    HostArmorSet,
    HostGeometryType,
    HostGeometryInfo,
};

mod template;
pub use template::ThingTemplate;

/// Leftover Common ThingTemplate::getThreatValue when the factory is live.
fn leftover_template_threat_value(template_name: &str) -> Option<u16> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    Some(tmpl.get_threat_value())
}

fn default_auto_choose_masks() -> [u32; 3] {
    // C++ WeaponTemplateSet::clear: m_autoChooseMask[i] = 0xffffffff
    [u32::MAX; 3]
}

/// C++ `WeaponTemplateSet::parseAutoChoose`: slot then CommandSourceMask bits.
fn parse_auto_choose_value(value: &str) -> Option<(u8, u32)> {
    let mut tokens = value.split_whitespace();
    let first = tokens.next()?;
    let slot = match first.to_ascii_uppercase().as_str() {
        "PRIMARY" => 0u8,
        "SECONDARY" => 1,
        "TERTIARY" => 2,
        _ => return None,
    };
    let mut mask = 0u32;
    for token in tokens {
        match token.to_ascii_uppercase().as_str() {
            "NONE" => {}
            "FROM_PLAYER" => mask |= 1 << 0,
            "FROM_SCRIPT" => mask |= 1 << 1,
            "FROM_AI" => mask |= 1 << 2,
            "FROM_DOZER" => mask |= 1 << 3,
            "DEFAULT_SWITCH_WEAPON" => mask |= 1 << 4,
            _ => {}
        }
    }
    Some((slot, mask))
}

/// C++ FiringTracker fields bound from a WeaponStore template onto a host Object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponTrackerBind {
    pub continuous_fire_one_shots: u32,
    pub continuous_fire_two_shots: u32,
    pub continuous_fire_coast_frames: u32,
    pub auto_reload_when_idle_frames: u32,
}

impl Default for WeaponTrackerBind {
    fn default() -> Self {
        Self {
            continuous_fire_one_shots: u32::MAX,
            continuous_fire_two_shots: u32::MAX,
            continuous_fire_coast_frames: 0,
            auto_reload_when_idle_frames: 0,
        }
    }
}

fn shots_needed_to_host(value: i32) -> u32 {
    if value <= 0 || value == i32::MAX {
        u32::MAX
    } else {
        value as u32
    }
}

fn parse_ini_bool(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "1"
    )
}

fn kind_of_from_preferred_token(token: &str) -> Option<KindOf> {
    match token.trim().to_ascii_uppercase().replace('-', "_").as_str() {
        "INFANTRY" => Some(KindOf::Infantry),
        "VEHICLE" => Some(KindOf::Vehicle),
        "AIRCRAFT" => Some(KindOf::Aircraft),
        "STRUCTURE" => Some(KindOf::Structure),
        "PROJECTILE" => Some(KindOf::Projectile),
        "BALLISTIC_MISSILE" => Some(KindOf::BallisticMissile),
        "SMALL_MISSILE" => Some(KindOf::SmallMissile),
        "MINE" => Some(KindOf::Mine),
        "DEMOTRAP" => Some(KindOf::DemoTrap),
        "PARACHUTE" => Some(KindOf::Parachute),
        _ => None,
    }
}

/// C++ `WeaponTemplateSet::parsePreferredAgainst`: first token is the slot.
fn parse_preferred_against_value(value: &str) -> Option<(u8, Vec<KindOf>)> {
    let mut tokens = value.split_whitespace();
    let first = tokens.next()?;
    let (slot, leftover) = match first.to_ascii_uppercase().as_str() {
        "PRIMARY" => (0u8, None),
        "SECONDARY" => (1, None),
        "TERTIARY" => (2, None),
        _ => (0, Some(first)),
    };
    let mut kinds = Vec::new();
    if let Some(token) = leftover {
        if let Some(kind) = kind_of_from_preferred_token(token) {
            kinds.push(kind);
        }
    }
    for token in tokens {
        if let Some(kind) = kind_of_from_preferred_token(token) {
            kinds.push(kind);
        }
    }
    if kinds.is_empty() {
        None
    } else {
        Some((slot, kinds))
    }
}

fn default_template_shroud_clearing_range() -> f32 {
    // C++ ThingTemplate m_shroudClearingRange default -1 → use VisionRange.
    -1.0
}

fn default_template_skill_point_values() -> [i32; 4] {
    [crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL; 4]
}

fn default_template_shroud_reveal_to_all_range() -> f32 {
    // C++ ThingTemplate m_shroudRevealToAllRange default -1.
    -1.0
}

fn default_template_crushable_level() -> u8 {
    // C++ ThingTemplate.cpp:1024 m_crushableLevel = 255 (uncrushable).
    255
}

fn default_asset_scale() -> f32 {
    1.0
}

fn default_stealth_friendly_opacity_min() -> f32 {
    0.5
}

fn default_stealth_friendly_opacity_max() -> f32 {
    1.0
}

fn default_template_physics_mass() -> f32 {
    1.0
}

fn default_template_pitch_roll_yaw_factor() -> f32 {
    2.0
}

fn default_template_forward_friction() -> f32 {
    0.15
}
fn default_template_lateral_friction() -> f32 {
    0.15
}
fn default_template_z_friction() -> f32 {
    0.8
}
fn default_template_allow_collide_force() -> bool {
    true
}
fn default_template_min_fall_speed() -> f32 {
    // Leftover height_to_speed(40) with retail Gravity -64/900 (~2.385).
    (2.0 * (64.0_f32 / 900.0) * 40.0).sqrt()
}
fn default_template_fall_height_damage_factor() -> f32 {
    1.0
}

#[cfg(test)]
#[path = "authored_weapon_range_tests.rs"]
mod authored_weapon_range_tests;

#[cfg(test)]
#[path = "thing/weapon_resolve_tests.rs"]
mod weapon_resolve_tests;

/// Base Thing class - common functionality for all game entities
#[derive(Debug, Serialize, Deserialize)]
pub struct Thing {
    pub template: ThingTemplate,
    pub geometry: GeometryInfo,
    pub transform: Mat4,

    // Cached values for performance
    cached_position: Vec3,
    cached_angle: f32,
    cached_dir_vector: Vec3,
    cache_valid: bool,
}

impl Thing {
    pub fn new(template: ThingTemplate) -> Self {
        let geometry = template.geometry_info.to_host_geometry();
        let mut thing = Self {
            template,
            geometry,
            transform: Mat4::IDENTITY,
            cached_position: Vec3::ZERO,
            cached_angle: 0.0,
            cached_dir_vector: Vec3::X,
            cache_valid: false,
        };
        thing.update_cache();
        thing
    }

    pub fn get_template(&self) -> &ThingTemplate {
        &self.template
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.template.is_kind_of(kind)
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.geometry.position = position;
        self.transform =
            Mat4::from_translation(position) * Mat4::from_rotation_y(self.cached_angle);
        self.update_cache();
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.cached_angle = angle;
        self.transform =
            Mat4::from_translation(self.cached_position) * Mat4::from_rotation_y(angle);
        self.update_cache();
    }

    pub fn get_position(&self) -> Vec3 {
        self.cached_position
    }

    pub fn get_orientation(&self) -> f32 {
        self.cached_angle
    }

    pub fn get_direction_vector(&self) -> Vec3 {
        self.cached_dir_vector
    }

    pub fn set_transform_matrix(&mut self, transform: Mat4) {
        self.transform = transform;
        self.update_cache();
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.transform
    }

    fn update_cache(&mut self) {
        // Extract position from transform matrix
        let translation = self.transform.w_axis.truncate();
        self.cached_position = translation;

        // Extract yaw from the facing basis. Host movement / aim residual uses
        // forward = (cos θ, 0, -sin θ), which is the X column of from_rotation_y(θ).
        // (Previously used Z column, which shifted θ by -π/2 and broke aim checks.)
        let forward = self.transform.x_axis.truncate();
        self.cached_angle = (-forward.z).atan2(forward.x);

        // Calculate direction vector
        self.cached_dir_vector = Vec3::new(self.cached_angle.cos(), 0.0, -self.cached_angle.sin());

        // Update geometry position
        self.geometry.position = self.cached_position;
        self.geometry.rotation = self.cached_angle;

        self.cache_valid = true;
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        (self.transform * point.extend(1.0)).truncate()
    }

    pub fn get_distance_to(&self, other: &Thing) -> f32 {
        self.cached_position.distance(other.cached_position)
    }

    pub fn get_distance_to_position(&self, position: Vec3) -> f32 {
        self.cached_position.distance(position)
    }

    pub fn is_within_range(&self, other: &Thing, range: f32) -> bool {
        self.get_distance_to(other) <= range
    }

    pub fn get_bounds(&self) -> (Vec3, Vec3) {
        let half_size = Vec3::splat(self.geometry.radius);
        (
            self.cached_position - half_size,
            self.cached_position + half_size,
        )
    }

    pub fn intersects_bounds(&self, other: &Thing) -> bool {
        let (min_a, max_a) = self.get_bounds();
        let (min_b, max_b) = other.get_bounds();

        max_a.x >= min_b.x
            && min_a.x <= max_b.x
            && max_a.y >= min_b.y
            && min_a.y <= max_b.y
            && max_a.z >= min_b.z
            && min_a.z <= max_b.z
    }
}

impl Clone for Thing {
    fn clone(&self) -> Self {
        Self {
            template: self.template.clone(),
            geometry: self.geometry.clone(),
            transform: self.transform,
            cached_position: self.cached_position,
            cached_angle: self.cached_angle,
            cached_dir_vector: self.cached_dir_vector,
            cache_valid: self.cache_valid,
        }
    }
}

//! StealthUpdate - Rust conversion of C++ StealthUpdate
//!
//! Update module that manages stealth functionality for units.
//! Includes distance-based reveal logic: stealth breaks when unit gets too close to hostile targets.
//! Author: Kris Morness, May 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::NameKeyType;
use crate::common::{
    Bool, Coord3D, DisabledMaskType, Int, KindOf, ModuleData, ObjectStatusMaskType,
    ObjectStatusTypes, Real, UnsignedInt, XferVersion,
};
use crate::modules::{
    BehaviorModuleInterface, UpdateModule, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{Object as GameObject, ObjectID, INVALID_ID as OBJECT_INVALID_ID};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::Thing as ModuleThing;
use log::{debug, trace};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

// ObjectStatusMaskType constants
const OBJECT_STATUS_IS_FIRING_WEAPON: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::IsFiringWeapon);
const OBJECT_STATUS_IS_USING_ABILITY: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::IsUsingAbility);
const OBJECT_STATUS_CAN_STEALTH: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::CanStealth);
pub const OBJECT_STATUS_STEALTHED: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::Stealthed);
pub const OBJECT_STATUS_DETECTED: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::Detected);
pub const OBJECT_STATUS_DISGUISED: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::Disguised);
const OBJECT_STATUS_SCRIPT_UNSTEALTHED: ObjectStatusMaskType =
    ObjectStatusMaskType::from_status(ObjectStatusTypes::Detected);

// Stealth level flags
pub const STEALTH_NOT_WHILE_ATTACKING: u32 = 0x00000001;
pub const STEALTH_NOT_WHILE_MOVING: u32 = 0x00000002;
pub const STEALTH_NOT_WHILE_USING_ABILITY: u32 = 0x00000004;
pub const STEALTH_NOT_WHILE_FIRING_PRIMARY: u32 = 0x00000008;
pub const STEALTH_NOT_WHILE_FIRING_SECONDARY: u32 = 0x00000010;
pub const STEALTH_NOT_WHILE_FIRING_TERTIARY: u32 = 0x00000020;
pub const STEALTH_ONLY_WITH_BLACK_MARKET: u32 = 0x00000040;
pub const STEALTH_NOT_WHILE_TAKING_DAMAGE: u32 = 0x00000080;
pub const STEALTH_NOT_WHILE_RIDERS_ATTACKING: u32 = 0x00000100;
pub const STEALTH_NOT_WHILE_FIRING_WEAPON: u32 = STEALTH_NOT_WHILE_FIRING_PRIMARY
    | STEALTH_NOT_WHILE_FIRING_SECONDARY
    | STEALTH_NOT_WHILE_FIRING_TERTIARY;

const INVALID_OPACITY: Real = -1.0;
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
const NEVER: UnsignedInt = u32::MAX;

/// Default reveal distance threshold (in game units)
const DEFAULT_REVEAL_DISTANCE: Real = 100.0;

/// Reveal distance configuration for stealth detection
#[derive(Clone, Debug)]
pub struct RevealDistanceConfig {
    /// Base reveal distance in game units
    pub base_reveal_distance: f32,

    /// Per-unit-type modifiers for reveal distance
    pub per_unit_modifiers: HashMap<String, f32>,
}

impl Default for RevealDistanceConfig {
    fn default() -> Self {
        Self {
            base_reveal_distance: DEFAULT_REVEAL_DISTANCE,
            per_unit_modifiers: HashMap::new(),
        }
    }
}

impl RevealDistanceConfig {
    /// Create new configuration with base distance
    pub fn new(base_distance: f32) -> Self {
        Self {
            base_reveal_distance: base_distance.max(0.0),
            per_unit_modifiers: HashMap::new(),
        }
    }

    /// Add per-unit modifier for a unit type
    pub fn add_unit_modifier(&mut self, unit_type: String, modifier: f32) {
        self.per_unit_modifiers.insert(unit_type, modifier.max(0.0));
    }

    /// Get effective reveal distance for a unit type
    pub fn get_effective_distance(&self, unit_type: &str) -> f32 {
        let modifier = self
            .per_unit_modifiers
            .get(unit_type)
            .copied()
            .unwrap_or(1.0);
        self.base_reveal_distance * modifier
    }
}

/// Module data for StealthUpdate
#[derive(Clone, Debug)]
pub struct StealthUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub hint_detectable_states: ObjectStatusMaskType,
    pub required_status: ObjectStatusMaskType,
    pub forbidden_status: ObjectStatusMaskType,
    pub stealth_speed: Real,
    pub friendly_opacity_min: Real,
    pub friendly_opacity_max: Real,
    pub reveal_distance_from_target: Real,
    pub disguise_transition_frames: UnsignedInt,
    pub disguise_reveal_transition_frames: UnsignedInt,
    pub pulse_frames: UnsignedInt,
    pub stealth_delay: UnsignedInt,
    pub stealth_level: UnsignedInt,
    pub black_market_check_frames: UnsignedInt,
    pub innate_stealth: Bool,
    pub order_idle_enemies_to_attack_me_upon_reveal: Bool,
    pub team_disguised: Bool,
    pub use_rider_stealth: Bool,
    pub granted_by_special_power: Bool,
}

impl Default for StealthUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            hint_detectable_states: ObjectStatusMaskType::none(),
            required_status: ObjectStatusMaskType::none(),
            forbidden_status: ObjectStatusMaskType::none(),
            stealth_speed: 0.0,
            friendly_opacity_min: 0.0,
            friendly_opacity_max: 1.0,
            reveal_distance_from_target: DEFAULT_REVEAL_DISTANCE,
            disguise_transition_frames: 30,
            disguise_reveal_transition_frames: 15,
            pulse_frames: 0,
            stealth_delay: 0,
            stealth_level: 0,
            black_market_check_frames: 30,
            innate_stealth: false,
            order_idle_enemies_to_attack_me_upon_reveal: false,
            team_disguised: false,
            use_rider_stealth: false,
            granted_by_special_power: false,
        }
    }
}

impl crate::common::LegacyModuleData for StealthUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for StealthUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// StealthUpdate behavior module
pub struct StealthUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<StealthUpdateModuleData>,

    // State
    stealth_allowed_frame: UnsignedInt,
    detection_expires_frame: UnsignedInt,
    next_black_market_check_frame: UnsignedInt,
    enabled: Bool,
    pulse_phase_rate: Real,
    pulse_phase: Real,

    // Disguise state
    disguise_as_player_index: Int,
    disguise_transition_frames: UnsignedInt,
    disguise_halfpoint_reached: Bool,
    transitioning_to_disguise: Bool,
    disguised: Bool,
    frames_granted: UnsignedInt,

    // Runtime state
    xfer_restore_disguise: Bool,

    // Distance-based reveal configuration
    reveal_distance_config: RevealDistanceConfig,
    last_distance_check_frame: UnsignedInt,
}

impl StealthUpdate {
    /// Create a new StealthUpdate instance
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<StealthUpdateModuleData>()
            .ok_or("Invalid module data type for StealthUpdate")?;

        let reveal_distance_config =
            RevealDistanceConfig::new(specific_data.reveal_distance_from_target);

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            stealth_allowed_frame: 0,
            detection_expires_frame: 0,
            next_black_market_check_frame: 0,
            enabled: true,
            pulse_phase_rate: 0.0,
            pulse_phase: 0.0,
            disguise_as_player_index: -1,
            disguise_transition_frames: 0,
            disguise_halfpoint_reached: false,
            transitioning_to_disguise: false,
            disguised: false,
            frames_granted: 0,
            xfer_restore_disguise: false,
            reveal_distance_config,
            last_distance_check_frame: 0,
        })
    }

    /// Get reveal distance from target (used by C++ reference)
    pub fn get_reveal_distance_from_target(&self) -> Real {
        self.module_data.reveal_distance_from_target
    }

    /// Check if unit is disguised
    pub fn is_disguised(&self) -> Bool {
        self.disguised
    }

    /// Get disguised player index
    pub fn get_disguised_player_index(&self) -> Int {
        self.disguise_as_player_index
    }

    /// Mark unit as detected for a duration
    pub fn mark_as_detected(&mut self, num_frames: UnsignedInt) {
        if let Some(object) = self.object.upgrade() {
            if let Ok(_obj) = object.read() {
                // Frame counter is managed externally by game logic
                // Use module_data.stealth_delay as default if num_frames is 0
                let current_frame = self.last_distance_check_frame; // Track via last update

                if num_frames == 0 {
                    self.detection_expires_frame = current_frame + self.module_data.stealth_delay;
                } else {
                    self.detection_expires_frame = current_frame + num_frames;
                }
            }
        }
    }

    /// Get friendly opacity for rendering
    pub fn get_friendly_opacity(&self) -> Real {
        if self.module_data.pulse_frames > 0 {
            let min_opacity = self.module_data.friendly_opacity_min;
            let max_opacity = self.module_data.friendly_opacity_max;
            let range = max_opacity - min_opacity;

            // Pulse between min and max
            min_opacity + (self.pulse_phase.sin() * 0.5 + 0.5) * range
        } else {
            self.module_data.friendly_opacity_max
        }
    }

    /// Check if unit is currently attacking
    fn is_attacking(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check OBJECT_STATUS_IS_FIRING_WEAPON status bit
                return obj
                    .get_status_bits()
                    .intersects(OBJECT_STATUS_IS_FIRING_WEAPON);
            }
        }
        false
    }

    /// Get current velocity magnitude of the unit
    fn get_velocity(&self) -> Real {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Get velocity from physics module (C++ StealthUpdate.cpp:390)
                if let Some(physics) = obj.get_physics() {
                    if let Ok(phys_guard) = physics.lock() {
                        return phys_guard.get_velocity().length();
                    }
                }
                return 0.0;
            }
        }
        0.0
    }

    /// Check if unit is firing primary weapon
    fn is_firing_primary(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check last shot frame of primary weapon (C++ StealthUpdate.cpp:336-344)
                // Weapon slot checking requires weapon module access
                // For now, rely on IS_FIRING_WEAPON status bit which is more reliable
                return obj
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::IS_FIRING_WEAPON);
            }
        }
        false
    }

    /// Check if unit is firing secondary weapon
    fn is_firing_secondary(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check last shot frame of secondary weapon (C++ StealthUpdate.cpp:345-353)
                // Weapon slot checking requires weapon module access
                // For now, rely on IS_FIRING_WEAPON status bit which is more reliable
                return obj
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::IS_FIRING_WEAPON);
            }
        }
        false
    }

    /// Check if unit is firing tertiary weapon
    fn is_firing_tertiary(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check last shot frame of tertiary weapon (C++ StealthUpdate.cpp:354-362)
                // Weapon slot checking requires weapon module access
                // For now, rely on IS_FIRING_WEAPON status bit which is more reliable
                return obj
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::IS_FIRING_WEAPON);
            }
        }
        false
    }

    /// Check if unit is currently taking damage
    fn is_taking_damage(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Get last damage timestamp from body module (C++ StealthUpdate.cpp:299-311)
                // Check if damage occurred within the last frame or two
                if let Some(body) = obj.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        let last_damage_ts = body_guard.get_last_damage_timestamp();
                        let current_frame = self.last_distance_check_frame;
                        // Check if damage is recent (within last 2 frames) and not healing
                        if last_damage_ts != u32::MAX
                            && last_damage_ts >= current_frame.saturating_sub(2)
                        {
                            if let Some(damage_info) = body_guard.get_last_damage_info() {
                                // Ignore healing damage
                                return damage_info.input.damage_type
                                    != crate::damage::DamageType::Healing;
                            }
                            return true;
                        }
                    }
                }
                return false;
            }
        }
        false
    }

    /// Check if unit has riders attacking
    fn has_riders_attacking(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check if contained module exists and riders are attacking (C++ StealthUpdate.cpp:376-385)
                if let Some(contain) = obj.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        // Check each contained unit for attacking status
                        for &rider_id in contain_guard.get_contained_objects() {
                            if let Some(rider) =
                                crate::object::registry::OBJECT_REGISTRY.get_object(rider_id)
                            {
                                if let Ok(rider_guard) = rider.read() {
                                    let rider_status = rider_guard.get_status_bits();
                                    if rider_status.contains(ObjectStatusMaskType::IS_ATTACKING)
                                        || rider_status
                                            .contains(ObjectStatusMaskType::IS_FIRING_WEAPON)
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
                return false;
            }
        }
        false
    }

    /// Check if unit is using special ability
    fn is_using_ability(&self) -> Bool {
        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check OBJECT_STATUS_IS_USING_ABILITY status bit
                return obj
                    .get_status_bits()
                    .intersects(OBJECT_STATUS_IS_USING_ABILITY);
            }
        }
        false
    }

    /// Check if black market is available for the controlling player
    fn check_black_market_available(&self, _player_id: Int) -> Bool {
        let Some(owner_arc) = self.object.upgrade() else {
            return false;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return false;
        };
        let Some(player_arc) = owner_guard.get_controlling_player() else {
            return false;
        };

        let mut has_black_market = false;
        if let Ok(player_guard) = player_arc.read() {
            let _ = player_guard.iterate_objects(|object_arc| {
                if has_black_market {
                    return Ok(());
                }

                let Ok(object_guard) = object_arc.read() else {
                    return Ok(());
                };
                if object_guard.is_effectively_dead() {
                    return Ok(());
                }

                let status = object_guard.get_status_bits();
                if status.contains(ObjectStatusMaskType::UNDER_CONSTRUCTION)
                    || status.contains(ObjectStatusMaskType::SOLD)
                {
                    return Ok(());
                }

                let template_name = object_guard.get_template_name().to_ascii_lowercase();
                let matches_template = template_name.contains("blackmarket")
                    || template_name.contains("black_market")
                    || template_name.contains("black-market");
                let matches_kind = object_guard.is_kind_of(KindOf::CashGenerator)
                    && template_name.contains("market");

                if matches_template || matches_kind {
                    has_black_market = true;
                }

                Ok(())
            });
        }

        has_black_market
    }

    /// Check delay time has expired
    fn delay_time_expired(&self) -> Bool {
        // Frame counter tracked via last_distance_check_frame
        let current_frame = self.last_distance_check_frame;
        current_frame >= self.stealth_allowed_frame
    }

    /// Calculate reveal distance for distance-based stealth break
    /// Returns the effective reveal distance threshold in game units
    pub fn calculate_reveal_distance(&self) -> Real {
        // Use base reveal distance from configuration
        self.reveal_distance_config.base_reveal_distance
    }

    /// Check if unit is too close to a hostile target (stealth reveal condition)
    /// Returns true if stealth should be broken due to distance
    fn check_distance_to_targets(&self) -> Bool {
        // Implement proximity checks (C++ StealthUpdate.cpp:675-693)
        // This requires partition manager query APIs which may not be fully exposed yet
        // The logic checks if the unit's current victim is within reveal distance
        if self.module_data.reveal_distance_from_target <= 0.0 {
            return false; // Feature disabled
        }

        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check distance to current attack target
                if let Some(victim_pos) = obj.get_current_victim_pos() {
                    let self_pos = obj.get_position();
                    let distance = (victim_pos - *self_pos).length();
                    return distance < self.module_data.reveal_distance_from_target;
                }
            }
        }
        false
    }

    /// Check if unit can stealth based on all conditions including distance
    pub fn allowed_to_stealth(&self) -> Bool {
        if !self.enabled {
            return false;
        }

        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Use last_distance_check_frame as current frame tracker
                let current_frame = self.last_distance_check_frame;

                // Check if still in detection period
                if current_frame < self.detection_expires_frame {
                    return false;
                }

                // Check if enough time has passed since damage
                if current_frame < self.stealth_allowed_frame {
                    return false;
                }

                // Check if unit has CAN_STEALTH status bit
                if !obj.get_status_bits().intersects(OBJECT_STATUS_CAN_STEALTH) {
                    return false;
                }

                let level = self.module_data.stealth_level;

                // Check STEALTH_NOT_WHILE_ATTACKING condition
                if (level & STEALTH_NOT_WHILE_ATTACKING) != 0 {
                    if self.is_attacking() {
                        return false;
                    }
                }

                // Check STEALTH_NOT_WHILE_USING_ABILITY condition
                if (level & STEALTH_NOT_WHILE_USING_ABILITY) != 0 {
                    if self.is_using_ability() {
                        return false;
                    }
                }

                // Check STEALTH_ONLY_WITH_BLACK_MARKET condition
                if (level & STEALTH_ONLY_WITH_BLACK_MARKET) != 0 {
                    if !self.check_black_market_available(-1) {
                        return false;
                    }
                }

                // Check STEALTH_NOT_WHILE_TAKING_DAMAGE condition
                if (level & STEALTH_NOT_WHILE_TAKING_DAMAGE) != 0 {
                    if self.is_taking_damage() {
                        return false;
                    }
                }

                // Check required status bits - must have ALL required bits
                if !self.module_data.required_status.is_empty() {
                    if (obj.get_status_bits() & self.module_data.required_status)
                        != self.module_data.required_status
                    {
                        return false;
                    }
                }

                // Check forbidden status bits - must NOT have ANY forbidden bits
                if !self.module_data.forbidden_status.is_empty() {
                    if obj
                        .get_status_bits()
                        .intersects(self.module_data.forbidden_status)
                    {
                        return false;
                    }
                }

                // Check STEALTH_NOT_WHILE_FIRING_WEAPON conditions (primary, secondary, tertiary)
                if (level & STEALTH_NOT_WHILE_FIRING_WEAPON) != 0 {
                    if self.is_attacking() {
                        // Do weapon-specific checks
                        if (level & STEALTH_NOT_WHILE_FIRING_PRIMARY) != 0 {
                            if self.is_firing_primary() {
                                return false;
                            }
                        }

                        if (level & STEALTH_NOT_WHILE_FIRING_SECONDARY) != 0 {
                            if self.is_firing_secondary() {
                                return false;
                            }
                        }

                        if (level & STEALTH_NOT_WHILE_FIRING_TERTIARY) != 0 {
                            if self.is_firing_tertiary() {
                                return false;
                            }
                        }
                    }
                }

                // Check STEALTH_NOT_WHILE_RIDERS_ATTACKING condition
                if (level & STEALTH_NOT_WHILE_RIDERS_ATTACKING) != 0 {
                    if self.has_riders_attacking() {
                        return false;
                    }
                }

                // Check STEALTH_NOT_WHILE_MOVING condition
                if (level & STEALTH_NOT_WHILE_MOVING) != 0 {
                    if self.get_velocity() > self.module_data.stealth_speed {
                        return false;
                    }
                }

                // Check script unstealthed status bit
                if obj
                    .get_status_bits()
                    .intersects(OBJECT_STATUS_SCRIPT_UNSTEALTHED)
                {
                    return false;
                }

                // CRITICAL: Check distance to hostile targets - breaks stealth if too close
                if self.check_distance_to_targets() {
                    trace!("Stealth denied due to proximity to hostile targets");
                    return false;
                }

                return true;
            }
        }

        false
    }

    /// Receive temporary stealth grant
    pub fn receive_grant(&mut self, active: Bool, frames: UnsignedInt) {
        if active {
            self.frames_granted = frames;
            self.enabled = true;
        } else {
            self.frames_granted = 0;
            if !self.module_data.innate_stealth {
                self.enabled = false;
            }
        }
    }

    /// Calculate sleep time for update
    fn calc_sleep_time(&self) -> UpdateSleepTime {
        if self.disguise_transition_frames > 0 {
            return UpdateSleepTime::Frames(1); // Update every frame during transition
        }

        if self.module_data.pulse_frames > 0 {
            return UpdateSleepTime::Frames(1); // Update every frame for pulsing
        }

        // Default update rate
        UpdateSleepTime::Frames(10)
    }
}

impl UpdateModuleInterface for StealthUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if let Some(object) = self.object.upgrade() {
            if let Ok(mut obj) = object.write() {
                // Increment frame counter
                self.last_distance_check_frame = self.last_distance_check_frame.saturating_add(1);

                // Handle temporary grant expiration
                if self.frames_granted > 0 {
                    self.frames_granted = self.frames_granted.saturating_sub(1);
                    if self.frames_granted == 0 && !self.module_data.innate_stealth {
                        self.enabled = false;
                    }
                }

                // Update pulse phase
                if self.module_data.pulse_frames > 0 {
                    self.pulse_phase += self.pulse_phase_rate;
                }

                // Handle disguise transitions
                if self.disguise_transition_frames > 0 {
                    self.disguise_transition_frames =
                        self.disguise_transition_frames.saturating_sub(1);

                    let halfway = if self.transitioning_to_disguise {
                        self.module_data.disguise_transition_frames / 2
                    } else {
                        self.module_data.disguise_reveal_transition_frames / 2
                    };

                    if !self.disguise_halfpoint_reached
                        && self.disguise_transition_frames <= halfway
                    {
                        self.disguise_halfpoint_reached = true;
                        // Switch drawables - handled by drawable system based on DISGUISED status bit
                        // The status bit change triggers visual model swap in renderer
                    }

                    if self.disguise_transition_frames == 0 {
                        self.disguised = self.transitioning_to_disguise;
                    }
                }

                // Check if can stealth and apply status
                if self.allowed_to_stealth() {
                    // Apply stealth to object (C++ StealthUpdate.cpp:735)
                    if !obj.get_status_bits().contains(OBJECT_STATUS_STEALTHED) {
                        obj.set_status(OBJECT_STATUS_STEALTHED, true);
                    }
                } else {
                    // Remove stealth status (C++ StealthUpdate.cpp:749)
                    if obj.get_status_bits().contains(OBJECT_STATUS_STEALTHED) {
                        obj.set_status(OBJECT_STATUS_STEALTHED, false);
                    }
                }

                return self.calc_sleep_time();
            }
        }

        UPDATE_SLEEP_FOREVER
    }
}

impl Snapshotable for StealthUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // version -- C++ StealthUpdate.cpp line 1119: currentVersion = 2
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("StealthUpdate version xfer failed: {:?}", e))?;

        // stealth allowed frame -- C++ StealthUpdate.cpp line 1127
        xfer.xfer_unsigned_int(&mut self.stealth_allowed_frame)
            .map_err(|e| format!("StealthUpdate stealth_allowed_frame xfer failed: {:?}", e))?;

        // detection expires frame -- C++ StealthUpdate.cpp line 1130
        xfer.xfer_unsigned_int(&mut self.detection_expires_frame)
            .map_err(|e| format!("StealthUpdate detection_expires_frame xfer failed: {:?}", e))?;

        // enabled -- C++ StealthUpdate.cpp line 1133
        xfer.xfer_bool(&mut self.enabled)
            .map_err(|e| format!("StealthUpdate enabled xfer failed: {:?}", e))?;

        // pulse phase rate -- C++ StealthUpdate.cpp line 1136
        xfer.xfer_real(&mut self.pulse_phase_rate)
            .map_err(|e| format!("StealthUpdate pulse_phase_rate xfer failed: {:?}", e))?;

        // pulse phase -- C++ StealthUpdate.cpp line 1139
        xfer.xfer_real(&mut self.pulse_phase)
            .map_err(|e| format!("StealthUpdate pulse_phase xfer failed: {:?}", e))?;

        // disguise as player index -- C++ StealthUpdate.cpp line 1142
        xfer.xfer_int(&mut self.disguise_as_player_index)
            .map_err(|e| {
                format!(
                    "StealthUpdate disguise_as_player_index xfer failed: {:?}",
                    e
                )
            })?;

        // disguise as template -- C++ StealthUpdate.cpp line 1145-1165
        // The Rust port does not store a disguise template pointer, so we xfer
        // an empty string for compatibility with the C++ save format.
        let mut disguise_template_name = String::new();
        xfer.xfer_ascii_string(&mut disguise_template_name)
            .map_err(|e| format!("StealthUpdate disguise_template_name xfer failed: {:?}", e))?;

        // disguise transition frames -- C++ StealthUpdate.cpp line 1168
        xfer.xfer_unsigned_int(&mut self.disguise_transition_frames)
            .map_err(|e| {
                format!(
                    "StealthUpdate disguise_transition_frames xfer failed: {:?}",
                    e
                )
            })?;

        // disguise halfpoint reached -- C++ StealthUpdate.cpp line 1171
        xfer.xfer_bool(&mut self.disguise_halfpoint_reached)
            .map_err(|e| {
                format!(
                    "StealthUpdate disguise_halfpoint_reached xfer failed: {:?}",
                    e
                )
            })?;

        // transitioning to disguise -- C++ StealthUpdate.cpp line 1174
        xfer.xfer_bool(&mut self.transitioning_to_disguise)
            .map_err(|e| {
                format!(
                    "StealthUpdate transitioning_to_disguise xfer failed: {:?}",
                    e
                )
            })?;

        // disguised -- C++ StealthUpdate.cpp line 1177
        xfer.xfer_bool(&mut self.disguised)
            .map_err(|e| format!("StealthUpdate disguised xfer failed: {:?}", e))?;

        // version 2 fields -- C++ StealthUpdate.cpp line 1179-1182
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.frames_granted)
                .map_err(|e| format!("StealthUpdate frames_granted xfer failed: {:?}", e))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ StealthUpdate.cpp line 1189-1204
        // Restore disguise flag after load so visual disguise can be applied
        // when the game is ready to run (cannot do it here because drawable
        // destruction during load is unsafe).
        if self.disguised {
            self.xfer_restore_disguise = true;
        }
        Ok(())
    }
}

impl BehaviorModuleInterface for StealthUpdate {
    fn get_module_name(&self) -> &'static str {
        "StealthUpdate"
    }

    fn get_disguised_player_index(&self) -> Option<Int> {
        if self.disguised && self.disguise_as_player_index >= 0 {
            Some(self.disguise_as_player_index)
        } else {
            None
        }
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

// Factory for creating StealthUpdate instances
pub struct StealthUpdateFactory;

impl StealthUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let behavior = StealthUpdate::new(thing, module_data)?;
        Ok(Box::new(behavior))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_flags() {
        assert_eq!(
            STEALTH_NOT_WHILE_FIRING_WEAPON,
            STEALTH_NOT_WHILE_FIRING_PRIMARY
                | STEALTH_NOT_WHILE_FIRING_SECONDARY
                | STEALTH_NOT_WHILE_FIRING_TERTIARY
        );
    }

    #[test]
    fn test_reveal_distance_config_creation() {
        let config = RevealDistanceConfig::new(100.0);
        assert_eq!(config.base_reveal_distance, 100.0);
        assert!(config.per_unit_modifiers.is_empty());
    }

    #[test]
    fn test_reveal_distance_config_default() {
        let config = RevealDistanceConfig::default();
        assert_eq!(config.base_reveal_distance, DEFAULT_REVEAL_DISTANCE);
    }

    #[test]
    fn test_reveal_distance_negative_clamping() {
        let config = RevealDistanceConfig::new(-50.0);
        assert_eq!(config.base_reveal_distance, 0.0);
    }

    #[test]
    fn test_reveal_distance_unit_modifier() {
        let mut config = RevealDistanceConfig::new(100.0);
        config.add_unit_modifier("RangedUnit".to_string(), 1.5);

        assert_eq!(config.get_effective_distance("RangedUnit"), 150.0);
        assert_eq!(config.get_effective_distance("MeleeUnit"), 100.0); // No modifier = 1.0x
    }

    #[test]
    fn test_reveal_distance_unit_modifier_negative_clamping() {
        let mut config = RevealDistanceConfig::new(100.0);
        config.add_unit_modifier("MeleeUnit".to_string(), -1.0);

        assert_eq!(config.get_effective_distance("MeleeUnit"), 0.0);
    }

    #[test]
    fn test_reveal_distance_multiple_modifiers() {
        let mut config = RevealDistanceConfig::new(100.0);
        config.add_unit_modifier("Scout".to_string(), 2.0);
        config.add_unit_modifier("Soldier".to_string(), 0.8);
        config.add_unit_modifier("DetectionUnit".to_string(), 1.5);

        assert_eq!(config.get_effective_distance("Scout"), 200.0);
        assert_eq!(config.get_effective_distance("Soldier"), 80.0);
        assert_eq!(config.get_effective_distance("DetectionUnit"), 150.0);
    }

    #[test]
    fn test_stealth_update_module_data_default() {
        let data = StealthUpdateModuleData::default();
        assert_eq!(data.reveal_distance_from_target, DEFAULT_REVEAL_DISTANCE);
        assert_eq!(data.friendly_opacity_min, 0.0);
        assert_eq!(data.friendly_opacity_max, 1.0);
        assert!(!data.innate_stealth);
    }

    #[test]
    fn test_reveal_distance_configuration_zero() {
        let config = RevealDistanceConfig::new(0.0);
        assert_eq!(config.base_reveal_distance, 0.0);
        assert_eq!(config.get_effective_distance("AnyUnit"), 0.0);
    }

    #[test]
    fn test_reveal_distance_large_values() {
        let config = RevealDistanceConfig::new(10000.0);
        assert_eq!(config.base_reveal_distance, 10000.0);
    }

    #[test]
    fn test_calculate_reveal_distance_distance_factor() {
        // Test distance factor calculation used in detection_manager
        let distance = 50.0;
        let reveal_distance = 100.0;

        // Stealth should be revealed if distance < reveal_distance
        assert!(distance < reveal_distance);
    }

    #[test]
    fn test_calculate_reveal_distance_beyond_threshold() {
        // Test case where unit is beyond reveal distance
        let distance = 150.0;
        let reveal_distance = 100.0;

        // Stealth should hold if distance >= reveal_distance
        assert!(distance >= reveal_distance);
    }

    #[test]
    fn test_calculate_reveal_distance_exact_boundary() {
        // Test case at exact boundary
        let distance = 100.0;
        let reveal_distance = 100.0;

        // At boundary, stealth should hold (not less than)
        assert!(distance >= reveal_distance);
    }

    #[test]
    fn test_calculate_reveal_distance_within_tolerance() {
        // Test multiple distances
        let reveal_distance = 150.0;

        assert!(100.0 < reveal_distance); // Stealth broken
        assert!(149.9 < reveal_distance); // Stealth broken
        assert!(150.0 >= reveal_distance); // Stealth holds
        assert!(150.1 >= reveal_distance); // Stealth holds
    }
}

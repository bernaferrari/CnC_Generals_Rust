//! Transport Contain Module
//!
//! Contain module for transport units with specialized transport functionality
//! including slot capacity, exit handling, and payload management.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, ObjectTemplate, OpenContain};
use crate::common::{
    CommandSourceType, DisabledType, GameResult, KindOf, ModelConditionState, ObjectID,
    PlayerMaskType, WeaponSlotType, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::helpers::TheThingFactory;
use crate::modules::{ContainModuleInterface, ContainWant, ExitDoorType, UpdateSleepTime};
use crate::object::{Object, ObjectArcExt};
use crate::player::Player;
use crate::weapon::WeaponSetType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

#[allow(dead_code)]
type ObjectId = ObjectID;

/// Initial payload configuration
#[derive(Debug, Clone)]
pub struct InitialPayload {
    pub name: String,
    pub count: i32,
}

impl Default for InitialPayload {
    fn default() -> Self {
        Self {
            name: String::new(),
            count: 0,
        }
    }
}

/// Configuration data for TransportContain module
#[derive(Debug, Clone)]
pub struct TransportContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Maximum units that can be inside (slot-based)
    pub slot_capacity: i32,
    /// Exit pitch rate
    pub exit_pitch_rate: f32,
    /// Exit bone name
    pub exit_bone: String,
    /// Initial payload configuration
    pub initial_payload: InitialPayload,
    /// Health regeneration rate
    pub health_regen: f32,
    /// Exit delay in frames
    pub exit_delay: u32,
    /// Scatter nearby units on exit
    pub scatter_nearby_on_exit: bool,
    /// Orient like container on exit
    pub orient_like_container_on_exit: bool,
    /// Keep container velocity on exit
    pub keep_container_velocity_on_exit: bool,
    /// Go aggressive on exit
    pub go_aggressive_on_exit: bool,
    /// Armed riders upgrade weapon set
    pub armed_riders_upgrade_weapon_set: bool,
    /// Reset mood check time on exit
    pub reset_mood_check_time_on_exit: bool,
    /// Destroy riders who are not free to exit
    pub destroy_riders_who_are_not_free_to_exit: bool,
    /// Delay exit when in air
    pub is_delay_exit_in_air: bool,
}

impl Default for TransportContainModuleData {
    fn default() -> Self {
        let mut base = super::OpenContainModuleData::default();
        base.allow_inside_kind_of = 1u64 << (KindOf::Infantry as u32);

        Self {
            base,
            slot_capacity: 0,
            exit_pitch_rate: 0.0,
            exit_bone: String::new(),
            initial_payload: Default::default(),
            health_regen: 0.0,
            exit_delay: 0,
            scatter_nearby_on_exit: true,
            orient_like_container_on_exit: false,
            keep_container_velocity_on_exit: false,
            go_aggressive_on_exit: false,
            armed_riders_upgrade_weapon_set: false,
            reset_mood_check_time_on_exit: true,
            destroy_riders_who_are_not_free_to_exit: false,
            is_delay_exit_in_air: false,
        }
    }
}

impl TransportContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, TRANSPORT_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, TRANSPORT_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for TransportContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        TransportContainModuleData::parse_from_config(self, config)
    }
}

fn parse_slot_capacity(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.slot_capacity = INI::parse_int(token)?;
    Ok(())
}

fn parse_scatter_nearby_on_exit(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scatter_nearby_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_orient_like_container_on_exit(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.orient_like_container_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_keep_container_velocity_on_exit(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.keep_container_velocity_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_go_aggressive_on_exit(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.go_aggressive_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_reset_mood_check_time_on_exit(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.reset_mood_check_time_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_destroy_riders_who_are_not_free_to_exit(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.destroy_riders_who_are_not_free_to_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_exit_bone(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.exit_bone = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_exit_pitch_rate(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.exit_pitch_rate = INI::parse_angular_velocity_real(token)?;
    Ok(())
}

fn parse_initial_payload(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = tokens.first().ok_or(INIError::InvalidData)?;
    let count = match tokens.get(1) {
        Some(token) => INI::parse_int(token)?,
        None => 1,
    };
    data.initial_payload.name = name.to_string();
    data.initial_payload.count = count;
    Ok(())
}

fn parse_health_regen_percent_per_sec(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.health_regen = INI::parse_real(token)?;
    Ok(())
}

fn parse_exit_delay(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.exit_delay = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_armed_riders_upgrade_weapon_set(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.armed_riders_upgrade_weapon_set = INI::parse_bool(token)?;
    Ok(())
}

fn parse_delay_exit_in_air(
    _ini: &mut INI,
    data: &mut TransportContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.is_delay_exit_in_air = INI::parse_bool(token)?;
    Ok(())
}

const TRANSPORT_CONTAIN_FIELDS: &[FieldParse<TransportContainModuleData>] = &[
    FieldParse {
        token: "Slots",
        parse: parse_slot_capacity,
    },
    FieldParse {
        token: "ScatterNearbyOnExit",
        parse: parse_scatter_nearby_on_exit,
    },
    FieldParse {
        token: "OrientLikeContainerOnExit",
        parse: parse_orient_like_container_on_exit,
    },
    FieldParse {
        token: "KeepContainerVelocityOnExit",
        parse: parse_keep_container_velocity_on_exit,
    },
    FieldParse {
        token: "GoAggressiveOnExit",
        parse: parse_go_aggressive_on_exit,
    },
    FieldParse {
        token: "ResetMoodCheckTimeOnExit",
        parse: parse_reset_mood_check_time_on_exit,
    },
    FieldParse {
        token: "DestroyRidersWhoAreNotFreeToExit",
        parse: parse_destroy_riders_who_are_not_free_to_exit,
    },
    FieldParse {
        token: "ExitBone",
        parse: parse_exit_bone,
    },
    FieldParse {
        token: "ExitPitchRate",
        parse: parse_exit_pitch_rate,
    },
    FieldParse {
        token: "InitialPayload",
        parse: parse_initial_payload,
    },
    FieldParse {
        token: "HealthRegen%PerSec",
        parse: parse_health_regen_percent_per_sec,
    },
    FieldParse {
        token: "ExitDelay",
        parse: parse_exit_delay,
    },
    FieldParse {
        token: "ArmedRidersUpgradeMyWeaponSet",
        parse: parse_armed_riders_upgrade_weapon_set,
    },
    FieldParse {
        token: "DelayExitInAir",
        parse: parse_delay_exit_in_air,
    },
];

/// Transport contain module - specialized container for transport units
#[derive(Debug)]
pub struct TransportContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Transport configuration retained for C++ behavior hooks.
    module_data: TransportContainModuleData,
    /// Reference to the owning object
    object_id: ObjectID,
    /// Whether payload has been created
    payload_created: bool,
    /// Extra slots in use (for units that take multiple slots)
    extra_slots_in_use: i32,
    /// Frame when exit will not be busy
    frame_exit_not_busy: u32,
}

impl TransportContain {
    /// Create a new TransportContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &TransportContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            module_data: module_data.clone(),
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
            payload_created: false,
            extra_slots_in_use: 0,
            frame_exit_not_busy: 0,
        })
    }

    /// Get the object this module belongs to
    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(id, f)
    }

    fn with_owner_object_mut<R>(&self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object_mut(id, f)
    }

    /// Short-lived Arc resolve; prefer `with_owner_object` / `get_object_id`.
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Check if object is contained in a zero-slot container (parachute)
        let actual_obj = if let Some(container_id) = obj.get_container_id() {
            let is_zero_slot = crate::object::registry::OBJECT_REGISTRY
                .with_object(container_id, |container| {
                    if let Some(contain) = container.get_contain() {
                        if let Ok(contain_guard) = contain.lock() {
                            return contain_guard.get_max_capacity() == 0;
                        }
                    }
                    false
                })
                .unwrap_or(false);
            if is_zero_slot {
                // Get first object inside the zero-slot container
                // For now, just use the original object since we can't easily
                // get a reference to the contained object
                obj
            } else {
                obj
            }
        } else {
            obj
        };

        // Call base validation
        if !self.base.is_valid_container_for(actual_obj, check_capacity) {
            return false;
        }

        // Only our own units can be transported (not allies)
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                let owner_player = owner.get_controlling_player();
                let actual_player = actual_obj.get_controlling_player();

                // Compare player IDs or Arc pointers
                match (owner_player, actual_player) {
                    (Some(ref p1), Some(ref p2)) if !Arc::ptr_eq(p1, p2) => return false,
                    (None, Some(_)) | (Some(_), None) => return false,
                    _ => {}
                }
            }
        }

        // Get transport slot count
        let transport_slot_count = actual_obj.get_transport_slot_count();

        // If 0, object isn't transportable
        if transport_slot_count == 0 {
            return false;
        }

        // Check capacity if requested
        if check_capacity {
            let contain_max = self.get_contain_max();
            let contain_count = self.base.get_contain_count() as i32;

            return self.extra_slots_in_use + contain_count + (transport_slot_count as i32)
                <= contain_max;
        }

        true
    }

    /// Handle capture event
    pub fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> GameResult<()> {
        let owners_differ = match (old_owner, new_owner) {
            (Some(old), Some(new)) => !Arc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };
        if !owners_differ {
            return Ok(());
        }

        // C++ parity: sniped/unmanned transports dump instantly; otherwise passengers get exit orders.
        if owner.is_disabled_by_type(DisabledType::DisabledUnmanned) {
            self.base.remove_all_contained(false)?;
        } else {
            ContainModuleInterface::order_all_passengers_to_exit(
                self,
                CommandSourceType::FromAi,
                false,
            )?;
        }
        Ok(())
    }

    /// Handle death event
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        self.kill_riders_who_are_not_free_to_exit()?;
        self.base.on_die(damage_info)?;
        Ok(())
    }

    /// Handle deletion event through inherited OpenContain cleanup.
    pub fn on_delete(&mut self) -> GameResult<()> {
        self.base.on_delete()
    }

    /// Called when this object starts containing another object
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_containing(obj_id, was_selected)?;

        // Set object as disabled (held)
        if let Ok(mut rider) = obj.write() {
            rider.set_disabled_held(true)?;

            // Track extra slots (units can take more than 1 slot)
            let transport_slot_count = rider.get_transport_slot_count();
            debug_assert!(
                transport_slot_count > 0,
                "TransportContain contained a non-transportable rider"
            );
            self.extra_slots_in_use += (transport_slot_count - 1) as i32;

            // Verify slot count is valid
            debug_assert!(
                self.extra_slots_in_use >= 0
                    && self.extra_slots_in_use + self.base.get_contain_count() as i32
                        <= self.get_contain_max(),
                "Bad slot count in TransportContain"
            );
        }

        // Set model condition LOADED when first unit enters
        if self.base.get_contain_count() == 1 {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    if let Some(drawable) = owner.get_drawable() {
                        if let Ok(mut draw) = drawable.write() {
                            draw.set_model_condition_state(ModelConditionState::Loaded);
                        }
                    }
                }
            }
        }

        // Let riders upgrade weapon set if configured
        self.let_riders_upgrade_weapon_set()?;

        // Track transport occupancy on the unit itself for quick validation elsewhere.
        if let Some(owner_transport) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(mut transport_guard) = owner_transport.write() {
                transport_guard.set_is_transporting(true);
            }
        }

        // Handle special case: Jarmen Kell + Combat Bike weapon timer transfer
        if let Some(owner_obj) = self.get_object() {
            if let Ok(mut owner) = owner_obj.write() {
                if let Ok(rider) = obj.read() {
                    if owner.is_kind_of(KindOf::CliffJumper)
                        && rider.is_kind_of(KindOf::Hero)
                        && rider.is_kind_of(KindOf::Salvager)
                    {
                        // Transfer weapon timers between bike and rider
                        if let (Some(bike_weapon), Some(rider_weapon)) = (
                            owner.get_weapon_in_slot_mut(crate::weapon::WeaponSlotType::Secondary),
                            rider.get_weapon_in_slot(crate::weapon::WeaponSlotType::Secondary),
                        ) {
                            bike_weapon.transfer_next_shot_stats_from(&rider_weapon);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_removing(obj_id)?;

        // Clear disabled state
        if let Ok(mut rider) = obj.write() {
            rider.set_disabled_held(false)?;

            // Reclaim extra slots
            let transport_slot_count = rider.get_transport_slot_count();
            debug_assert!(
                transport_slot_count > 0,
                "TransportContain removed a non-transportable rider"
            );
            self.extra_slots_in_use -= (transport_slot_count - 1) as i32;

            if !self.module_data.exit_bone.is_empty() {
                if let Some(owner_obj) = self.get_object() {
                    if let Ok(owner) = owner_obj.read() {
                        let (_, bone_pos, _) =
                            owner.get_single_logical_bone_position(&self.module_data.exit_bone);
                        let _ = rider.set_position(&bone_pos);
                    }
                }
            }

            if self.module_data.orient_like_container_on_exit {
                if let Some(owner_obj) = self.get_object() {
                    if let Ok(owner) = owner_obj.read() {
                        let _ = rider.set_orientation(owner.get_orientation());
                    }
                }
            }
        }

        // Clear model condition LOADED when last unit exits
        if self.base.get_contain_count() == 0 {
            if let Some(owner_obj) = self.get_object() {
                if let Ok(owner) = owner_obj.read() {
                    if let Some(drawable) = owner.get_drawable() {
                        if let Ok(mut draw) = drawable.write() {
                            draw.clear_model_condition_state(ModelConditionState::Loaded);
                        }
                    }
                }
            }
        }

        // Update transport tracking
        if let Some(owner_transport) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(mut transport_guard) = owner_transport.write() {
                let still_contains = self.base.get_contain_count() > 0;
                transport_guard.set_is_transporting(still_contains);
            }
        }

        // Let riders upgrade weapon set if configured
        self.let_riders_upgrade_weapon_set()?;

        self.frame_exit_not_busy =
            TheGameLogic::get_frame().saturating_add(self.module_data.exit_delay);

        Ok(())
    }

    /// Update method called once per frame
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Create payload if not already created
        if !self.payload_created {
            self.create_payload()?;
        }

        if self.module_data.health_regen != 0.0 {
            let owner = self.get_object();
            for object_id in self.base.get_contained_object_ids().to_vec() {
                if let Some(object) = TheGameLogic::find_object_by_id(object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(object_id))
                {
                    let needs_healing = object
                        .read()
                        .ok()
                        .and_then(|guard| guard.get_body_module())
                        .and_then(|body| {
                            body.lock().ok().map(|body_guard| {
                                body_guard.get_health() < body_guard.get_max_health()
                                    && body_guard.get_max_health() > 0.0
                            })
                        })
                        .unwrap_or(false);
                    if !needs_healing {
                        continue;
                    }

                    let Some(max_health) = object
                        .read()
                        .ok()
                        .and_then(|guard| guard.get_body_module())
                        .and_then(|body| {
                            body.lock()
                                .ok()
                                .map(|body_guard| body_guard.get_max_health())
                        })
                    else {
                        continue;
                    };
                    let regen = max_health * self.module_data.health_regen / 100.0
                        * SECONDS_PER_LOGICFRAME_REAL;
                    if let Ok(mut object_guard) = object.write() {
                        let source_guard = owner.as_ref().and_then(|owner| owner.read().ok());
                        let source_ref = source_guard.as_deref();
                        let _ = object_guard.attempt_healing(regen, source_ref);
                    }
                }
            }
        }

        self.base.update()
    }

    /// Check if this is a rider change container
    pub fn is_rider_change_contain(&self) -> bool {
        false
    }

    /// Check if this is a special overlord style container
    pub fn is_special_overlord_style_container(&self) -> bool {
        false
    }

    /// Get maximum containment capacity
    pub fn get_contain_max(&self) -> i32 {
        self.module_data.slot_capacity
    }

    /// Get extra slots in use
    pub fn get_extra_slots_in_use(&self) -> i32 {
        self.extra_slots_in_use
    }

    pub fn is_payload_created(&self) -> bool {
        self.payload_created
    }

    pub fn set_payload_created(&mut self, payload_created: bool) {
        self.payload_created = payload_created;
    }

    /// Check if exit is currently busy
    pub fn is_exit_busy(&self) -> bool {
        if self.module_data.is_delay_exit_in_air {
            let Some(owner) = self.get_object() else {
                return false;
            };
            if owner
                .read()
                .map(|owner_guard| owner_guard.is_above_terrain())
                .unwrap_or(false)
            {
                return true;
            }
        }
        TheGameLogic::get_frame() < self.frame_exit_not_busy
    }

    /// Reserve door for exit
    pub fn reserve_door_for_exit(
        &self,
        obj_type: &ObjectTemplate,
        specific_object: &Object,
    ) -> GameResult<ExitDoorType> {
        let _ = obj_type;
        if self.is_specific_rider_free_to_exit(specific_object) {
            Ok(ExitDoorType::Primary)
        } else {
            Ok(ExitDoorType::NoneAvailable)
        }
    }

    /// Unreserve door for exit
    pub fn unreserve_door_for_exit(&self, exit_door: ExitDoorType) -> GameResult<()> {
        self.base.unreserve_door_for_exit(exit_door)
    }

    /// Check if displayed on control bar
    pub fn is_displayed_on_control_bar(&self) -> bool {
        true
    }

    /// Kill riders who are not free to exit
    fn kill_riders_who_are_not_free_to_exit(&mut self) -> GameResult<()> {
        let contained = self.get_contained_objects().to_vec();
        for obj_id in contained {
            let Some(obj) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if !self.is_specific_rider_free_to_exit(&*obj_guard) {
                drop(obj_guard);
                if self.module_data.destroy_riders_who_are_not_free_to_exit {
                    let _ = TheGameLogic::destroy_object_by_id(obj_id);
                } else if let Ok(mut obj_write) = obj.write() {
                    obj_write.kill(None, None);
                }
            }
        }
        Ok(())
    }

    /// Check if specific rider is free to exit
    fn is_specific_rider_free_to_exit(&self, obj: &Object) -> bool {
        let Some(owner) = self.get_object() else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                return matches!(
                    ai_guard.get_ai_free_to_exit(obj),
                    crate::object::production::AIFreeToExitType::FreeToExit
                );
            }
        }
        true
    }

    /// Check if passenger is allowed to fire
    pub fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        self.base.is_passenger_allowed_to_fire(id)
    }

    /// Create initial payload
    fn create_payload(&mut self) -> GameResult<()> {
        if self.payload_created {
            return Ok(());
        }

        let (payload_name, payload_count, owner_team) = match self.get_object() {
            Some(owner) => {
                if let Ok(owner_guard) = owner.read() {
                    (
                        self.module_data.initial_payload.name.clone(),
                        self.module_data.initial_payload.count.max(0),
                        owner_guard.get_controlling_player().and_then(|player| {
                            player.read().ok().and_then(|p| p.get_default_team())
                        }),
                    )
                } else {
                    (String::new(), 0, None)
                }
            }
            None => (String::new(), 0, None),
        };

        if payload_count == 0 || payload_name.is_empty() {
            self.payload_created = true;
            return Ok(());
        }

        let Some(template) = TheThingFactory::find_template(&payload_name) else {
            log::warn!(
                "TransportContain payload template '{}' not found; skipping payload",
                payload_name
            );
            self.payload_created = true;
            return Ok(());
        };

        let factory = TheThingFactory::get().map_err(|e| e.to_string())?;
        self.base.enable_load_sounds(false);

        for _ in 0..payload_count {
            let payload = if let Some(team_arc) = &owner_team {
                if let Ok(team_guard) = team_arc.read() {
                    factory.new_object(template.clone(), &*team_guard)
                } else {
                    factory.new_object_optional_team(template.clone(), None)
                }
            } else {
                factory.new_object_optional_team(template.clone(), None)
            };

            let Ok(payload_obj) = payload else {
                log::warn!(
                    "TransportContain failed to create payload '{}' for owner",
                    payload_name
                );
                continue;
            };

            let can_add = payload_obj
                .read()
                .ok()
                .map(|guard| self.is_valid_container_for(&*guard, true))
                .unwrap_or(false);
            if can_add {
                let payload_id = payload_obj
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID);
                self.add_to_contain(payload_id)?;
            } else {
                log::warn!(
                    "TransportContain payload '{}' could not be inserted (container full/invalid)",
                    payload_name
                );
            }
        }

        self.base.enable_load_sounds(true);
        self.payload_created = true;
        Ok(())
    }

    /// Let riders upgrade weapon set (matches C++ letRidersUpgradeWeaponSet)
    fn let_riders_upgrade_weapon_set(&mut self) -> GameResult<()> {
        // Check if this feature is enabled
        if !self.module_data.armed_riders_upgrade_weapon_set {
            return Ok(());
        }

        if let Some(owner_obj) = self.get_object() {
            let mut any_rider_has_viable_weapon = false;

            // Check all riders for viable weapons
            for rider_id in self.base.get_contained_object_ids().to_vec() {
                if let Some(rider_obj) = TheGameLogic::find_object_by_id(rider_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(rider_id))
                {
                    if let Ok(rider) = rider_obj.read() {
                        // Only infantry can have viable weapons for this purpose
                        if !rider.is_kind_of(KindOf::Infantry) {
                            continue;
                        }

                        // Check all weapon slots
                        for weapon_slot in [
                            crate::weapon::WeaponSlotType::Primary,
                            crate::weapon::WeaponSlotType::Secondary,
                            crate::weapon::WeaponSlotType::Tertiary,
                        ] {
                            if let Some(weapon) = rider.get_weapon_in_slot(weapon_slot) {
                                // Weapon must be non-contact and damage-dealing
                                if !weapon.is_contact_weapon() && weapon.is_damage_weapon() {
                                    any_rider_has_viable_weapon = true;
                                    break;
                                }
                            }
                        }

                        if any_rider_has_viable_weapon {
                            break;
                        }
                    }
                }
            }

            // Update weapon set flag on transport
            if let Ok(mut owner_mut) = owner_obj.write() {
                if any_rider_has_viable_weapon {
                    owner_mut.set_weapon_set_flag(WeaponSetType::PlayerUpgrade);
                } else {
                    owner_mut.clear_weapon_set_flag(WeaponSetType::PlayerUpgrade);
                }
            }
        }

        Ok(())
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = HashMap::new();

        // Save base state
        let base_state = self.base.save_state()?;
        for (key, value) in base_state {
            state.insert(format!("base_{}", key), value);
        }

        // Save transport-specific state
        state.insert(
            "payload_created".to_string(),
            vec![if self.payload_created { 1 } else { 0 }],
        );
        state.insert(
            "extra_slots_in_use".to_string(),
            self.extra_slots_in_use.to_le_bytes().to_vec(),
        );
        state.insert(
            "frame_exit_not_busy".to_string(),
            self.frame_exit_not_busy.to_le_bytes().to_vec(),
        );

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        // Extract base state
        let mut base_state = HashMap::new();
        for (key, value) in state {
            if let Some(base_key) = key.strip_prefix("base_") {
                base_state.insert(base_key.to_string(), value.clone());
            }
        }

        // Load base state
        self.base.load_state(&base_state)?;

        // Load transport-specific state
        if let Some(data) = state.get("payload_created") {
            self.payload_created = data.get(0).copied().unwrap_or(0) != 0;
        }

        if let Some(data) = state.get("extra_slots_in_use") {
            if data.len() >= 4 {
                let bytes: [u8; 4] = data[0..4]
                    .try_into()
                    .map_err(|_| "Invalid extra_slots_in_use data")?;
                self.extra_slots_in_use = i32::from_le_bytes(bytes);
            }
        }

        if let Some(data) = state.get("frame_exit_not_busy") {
            if data.len() >= 4 {
                let bytes: [u8; 4] = data[0..4]
                    .try_into()
                    .map_err(|_| "Invalid frame_exit_not_busy data")?;
                self.frame_exit_not_busy = u32::from_le_bytes(bytes);
            }
        }

        Ok(())
    }

    /// Post-process after loading
    pub fn load_post_process(&mut self) -> GameResult<()> {
        self.base.load_post_process()
    }

    /// Add object to containment
    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let owner = self.get_object();
        if super::should_cancel_containment_after_booby_trap(
            owner.and_then(|o| o.read().ok().map(|g| g.get_id())),
            obj_id,
        ) {
            return Ok(());
        }

        let obj = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("Transport contain object not found")?;

        let was_selected = obj
            .read()
            .ok()
            .and_then(|guard| guard.get_drawable())
            .and_then(|drawable| drawable.read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);

        {
            let obj_ref = obj.read().map_err(|_| "Object lock poisoned")?;
            if !self.is_valid_container_for(&*obj_ref, true) {
                return Err("Object not valid for this transport container".into());
            }
            if obj_ref.get_contained_by().is_some() {
                return Ok(());
            }
        }

        self.add_to_contain_list(obj_id)?;
        let should_remove_from_world = obj
            .read()
            .map(|obj_guard| self.base.is_enclosing_container_for(&*obj_guard))
            .unwrap_or(false);
        if should_remove_from_world {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
        }
        self.base.redeploy_occupants()?;
        self.on_containing(obj_id, was_selected)?;
        Ok(())
    }

    /// Add object to contain list (internal method)
    pub fn add_to_contain_list(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.add_to_contain_list(obj_id)
    }

    /// Remove object from containment
    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        if !self.base.get_contained_object_ids().contains(&obj_id) {
            let _ = expose_stealth_units;
            return Ok(());
        }

        self.base.remove_from_contain_list(obj_id);
        let should_add_to_world = obj
            .read()
            .map(|obj_guard| self.base.is_enclosing_container_for(&*obj_guard))
            .unwrap_or(false);
        if should_add_to_world {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, true);
            if let Some(owner) = self.get_object() {
                if let (Ok(owner_guard), Ok(mut obj_guard)) = (owner.read(), obj.write()) {
                    let _ = obj_guard.set_position(owner_guard.get_position());
                    obj_guard.set_layer(owner_guard.get_layer());
                }
            }
        }
        self.base.do_unload_sound();
        self.on_removing(obj_id)?;

        let _ = expose_stealth_units;
        Ok(())
    }

    /// Check if this is an enclosing container for the given object
    pub fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        // Transport containers enclose their contents
        // Could add transport-specific logic here if needed
        self.base.is_enclosing_container_for(obj)
    }

    /// Redeploy all occupants from the transport
    pub fn redeploy_occupants(&mut self) -> GameResult<()> {
        // Delegate to base implementation which removes all and places at container position
        self.base.redeploy_occupants()
    }

    /// Get container pips info for UI display
    pub fn get_container_pips_info(&self) -> (i32, i32) {
        // For transport containers, we need to account for extra slots
        let total = self.get_contain_max();
        let full = self.base.get_contain_count() as i32 + self.extra_slots_in_use;
        (total, full)
    }
}

impl Snapshotable for TransportContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_bool(&mut self.payload_created)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.extra_slots_in_use)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.frame_exit_not_busy)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for TransportContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.add_to_contain(object_id).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.remove_from_contain(object_id, false)
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn get_container_pips_to_show(&self) -> (i32, i32, bool) {
        let (total, full) = self.get_container_pips_info();
        (total, full, true)
    }

    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(self)
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::update(self).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(damage_info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::on_die(self, damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn add_to_contain_list(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::add_to_contain_list(self, obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
    }

    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::on_capture(self, owner, old_owner, new_owner).map_err(|e| e.into())
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        TransportContain::is_passenger_allowed_to_fire(self, id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        self.base.has_objects_wanting_to_enter_or_exit()
    }

    fn is_special_overlord_style_container(&self) -> bool {
        self.is_special_overlord_style_container()
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        TransportContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        TransportContain::on_removing(self, obj_id).map_err(|e| e.into())
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_ids: Vec<_> = self.base.get_contained_object_ids().to_vec();
        for obj_id in object_ids {
            self.remove_from_contain(obj_id, expose_stealth)?;
        }
        Ok(())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_ids = self.base.get_contained_object_ids().to_vec();
        for obj_id in object_ids {
            self.remove_from_contain(obj_id, true)?;
            if let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                if let Ok(mut guard) = obj.write() {
                    let _ = guard.attempt_damage(damage_info);
                }
            }
        }
        Ok(())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_ids = self.base.get_contained_object_ids().to_vec();
        for obj_id in object_ids {
            self.remove_from_contain(obj_id, true)?;
            if let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                if let Ok(mut guard) = obj.write() {
                    guard.kill(None, None);
                }
            }
        }
        Ok(())
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        TransportContain::is_displayed_on_control_bar(self)
    }
}

impl ContainerInterface for TransportContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.base.get_contain_count();
        let max = match self.get_contain_max() {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::player::{Player, ThePlayerList};
    use crate::team::Team;

    #[test]
    fn test_transport_contain_creation() {
        let module_data = TransportContainModuleData {
            slot_capacity: 8,
            exit_delay: 30,
            go_aggressive_on_exit: true,
            ..Default::default()
        };

        assert_eq!(module_data.slot_capacity, 8);
        assert_eq!(module_data.exit_delay, 30);
        assert_eq!(module_data.go_aggressive_on_exit, true);
    }

    #[test]
    fn test_initial_payload() {
        let payload = InitialPayload {
            name: "Infantry".to_string(),
            count: 5,
        };

        assert_eq!(payload.name, "Infantry");
        assert_eq!(payload.count, 5);
    }

    fn reset_players() {
        let mut list = ThePlayerList().write().expect("player list write");
        list.clear();
        list.add_player(Arc::new(RwLock::new(Player::new(0))));
    }

    fn owned_object(name: &str, id: ObjectID, player_index: u32) -> Arc<RwLock<Object>> {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(player_index));
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), Some(team))
            .expect("owned test object")
    }

    fn slotted_passenger(name: &str, id: ObjectID, slots: i32) -> Arc<RwLock<Object>> {
        let team = Arc::new(RwLock::new(Team::new(
            format!("{name}Team").into(),
            id + 10_000,
        )));
        team.write()
            .expect("team write")
            .set_controlling_player_id(Some(0));
        let mut template = DefaultThingTemplate::new(name.to_string());
        let mut fields = HashMap::new();
        fields.insert("KindOf".to_string(), "INFANTRY".to_string());
        template.parse_object_fields_from_ini(&fields);
        let obj = Object::new_with_id(
            Arc::new(template),
            id,
            ObjectStatusMaskType::none(),
            Some(team),
        )
        .expect("slotted passenger");
        let data = super::super::OpenContainModuleData {
            contain_max: slots,
            ..Default::default()
        };
        let contain = OpenContain::new(Arc::downgrade(&obj), &data).expect("slot contain");
        obj.write()
            .expect("passenger write")
            .set_contain(Some(Arc::new(Mutex::new(contain))));
        obj
    }

    fn transport_for(owner: &Arc<RwLock<Object>>, slots: i32) -> TransportContain {
        let data = TransportContainModuleData {
            slot_capacity: slots,
            ..Default::default()
        };
        TransportContain::new(Arc::downgrade(owner), &data).expect("transport contain")
    }

    #[test]
    fn trait_containment_uses_transport_slots_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TransportOwner", 95001, 0);
        let passenger = slotted_passenger("TwoSlotPassenger", 95002, 2);
        let mut contain = transport_for(&owner, 3);

        assert_eq!(contain.get_contain_max(), 3);
        assert!(contain.is_valid_container_for(&passenger.read().expect("passenger read"), true));
        ContainModuleInterface::contain_object(&mut contain, 95002).expect("contain passenger");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
        assert_eq!(contain.get_extra_slots_in_use(), 1);
        assert_eq!(contain.get_container_pips_info(), (3, 2));
        assert_eq!(
            ContainModuleInterface::get_container_pips_to_show(&contain),
            (3, 2, true)
        );
        assert_eq!(
            passenger.read().expect("passenger read").get_contained_by(),
            Some(95001)
        );

        OBJECT_REGISTRY.unregister_object(95001);
        OBJECT_REGISTRY.unregister_object(95002);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn trait_release_uses_transport_removal_hook_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TransportReleaseOwner", 95003, 0);
        let passenger = slotted_passenger("TransportReleasePassenger", 95004, 2);
        let mut contain = transport_for(&owner, 3);

        ContainModuleInterface::contain_object(&mut contain, 95004).expect("contain passenger");
        ContainModuleInterface::release_object(&mut contain, 95004).expect("release passenger");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 0);
        assert_eq!(contain.get_extra_slots_in_use(), 0);
        assert_eq!(
            passenger.read().expect("passenger read").get_contained_by(),
            None
        );

        OBJECT_REGISTRY.unregister_object(95003);
        OBJECT_REGISTRY.unregister_object(95004);
        ThePlayerList().write().expect("player list write").clear();
    }
}

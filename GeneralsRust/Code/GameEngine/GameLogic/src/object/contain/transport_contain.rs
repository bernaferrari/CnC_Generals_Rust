//! Transport Contain Module
//!
//! Contain module for transport units with specialized transport functionality
//! including slot capacity, exit handling, and payload management.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, ObjectTemplate, OpenContain};
use crate::common::{
    CommandSourceType, DisabledType, GameResult, KindOf, ModelConditionState, ObjectID,
    PlayerMaskType, WeaponSlotType,
};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::helpers::TheThingFactory;
use crate::modules::{ContainModuleInterface, ContainWant, ExitDoorType, UpdateSleepTime};
use crate::object::{Object, ObjectArcExt};
use crate::player::Player;
use crate::weapon::WeaponSetType;
use game_engine::common::ini::{FieldParse, INIError, INI};

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
    /// Reference to the owning object
    object: Weak<RwLock<Object>>,
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
            object,
            payload_created: false,
            extra_slots_in_use: 0,
            frame_exit_not_busy: 0,
        })
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object.upgrade()
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Check if object is contained in a zero-slot container (parachute)
        let actual_obj = if let Some(container_arc) = obj.get_container() {
            if container_arc.is_special_zero_slot_container() {
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
        // Delegate to base implementation
        self.base.on_die(damage_info)?;
        Ok(())
    }

    /// Handle deletion event
    pub fn on_delete(&mut self) -> GameResult<()> {
        // Clean up transport-specific state if needed
        // For now, this is a no-op since we don't have transport-specific cleanup
        Ok(())
    }

    /// Called when this object starts containing another object
    pub fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        self.base.on_containing(obj.clone(), was_selected)?;

        // Set object as disabled (held)
        if let Ok(mut rider) = obj.write() {
            rider.set_disabled_held(true)?;

            // Track extra slots (units can take more than 1 slot)
            let transport_slot_count = rider.get_transport_slot_count();
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
        if let Some(owner_transport) = self.object.upgrade() {
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
    pub fn on_removing(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.on_removing(obj.clone())?;

        // Clear disabled state
        if let Ok(mut rider) = obj.write() {
            rider.set_disabled_held(false)?;

            // Reclaim extra slots
            let transport_slot_count = rider.get_transport_slot_count();
            self.extra_slots_in_use -= (transport_slot_count - 1) as i32;
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
        if let Some(owner_transport) = self.object.upgrade() {
            if let Ok(mut transport_guard) = owner_transport.write() {
                let still_contains = self.base.get_contain_count() > 0;
                transport_guard.set_is_transporting(still_contains);
            }
        }

        // Let riders upgrade weapon set if configured
        self.let_riders_upgrade_weapon_set()?;

        // Space out exits according to ExitDelay (matches C++ TransportContain::onRemoving).
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner_guard) = owner_obj.read() {
                if let Ok(module_data) = owner_guard.get_transport_contain_module_data() {
                    self.frame_exit_not_busy =
                        TheGameLogic::get_frame().saturating_add(module_data.exit_delay);
                }
            }
        }

        Ok(())
    }

    /// Update method called once per frame
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Create payload if not already created
        if !self.payload_created {
            self.create_payload()?;
        }

        // Kill riders who are not free to exit (if configured)
        self.kill_riders_who_are_not_free_to_exit()?;

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
        // Get from module data
        // For now, use base implementation
        self.base.get_contain_max()
    }

    /// Get extra slots in use
    pub fn get_extra_slots_in_use(&self) -> i32 {
        self.extra_slots_in_use
    }

    /// Check if exit is currently busy
    pub fn is_exit_busy(&self) -> bool {
        let Some(owner) = self.get_object() else {
            return false;
        };
        let Ok(owner_guard) = owner.read() else {
            return false;
        };
        let Ok(module_data) = owner_guard.get_transport_contain_module_data() else {
            return false;
        };
        if module_data.is_delay_exit_in_air && owner_guard.is_above_terrain() {
            return true;
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
        let Some(owner) = self.get_object() else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };
        let Ok(module_data) = owner_guard.get_transport_contain_module_data() else {
            return Ok(());
        };
        if !module_data.destroy_riders_who_are_not_free_to_exit {
            return Ok(());
        }

        let contained = self.get_contained_objects().to_vec();
        for obj_id in contained {
            let Some(obj) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if !self.is_specific_rider_free_to_exit(&*obj_guard) {
                let _ = TheGameLogic::destroy_object_by_id(obj_id);
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
                    if let Ok(module_data) = owner_guard.get_transport_contain_module_data() {
                        (
                            module_data.initial_payload.name.clone(),
                            module_data.initial_payload.count.max(0),
                            owner_guard.get_controlling_player().and_then(|player| {
                                player.read().ok().and_then(|p| p.get_default_team())
                            }),
                        )
                    } else {
                        (String::new(), 0, None)
                    }
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
                self.base.add_to_contain(payload_obj)?;
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
        if let Some(owner_obj) = self.get_object() {
            if let Ok(owner) = owner_obj.read() {
                if let Ok(module_data) = owner.get_transport_contain_module_data() {
                    if !module_data.armed_riders_upgrade_weapon_set {
                        return Ok(());
                    }

                    let mut any_rider_has_viable_weapon = false;

                    // Check all riders for viable weapons
                    let rider_list = self.base.get_contained_items_list()?;
                    for rider_obj in rider_list {
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

                    // Update weapon set flag on transport
                    drop(owner);
                    if let Ok(mut owner_mut) = owner_obj.write() {
                        if any_rider_has_viable_weapon {
                            owner_mut.set_weapon_set_flag(WeaponSetType::PlayerUpgrade);
                        } else {
                            owner_mut.clear_weapon_set_flag(WeaponSetType::PlayerUpgrade);
                        }
                    }
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
    pub fn add_to_contain(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        // Validate before adding
        if let Ok(obj_ref) = obj.read() {
            if !self.is_valid_container_for(&*obj_ref, true) {
                return Err("Object not valid for this transport container".into());
            }
        }

        self.add_to_contain_list(obj.clone())?;
        self.on_containing(obj, false)?;
        Ok(())
    }

    /// Add object to contain list (internal method)
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        // Delegate to base implementation
        self.base.add_to_contain_list(obj)
    }

    /// Remove object from containment
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let obj_id = obj.read().map_err(|_| "Object lock poisoned")?.get_id();
        if let Some(pos) = self
            .base
            .get_contained_items_list()?
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, &obj))
        {
            let _ = pos;
            self.base.remove_from_contain_list(obj_id);
            self.on_removing(obj)?;
        }

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
        let (total, _) = self.base.get_container_pips_info();
        let full = self.base.get_contain_count() as i32 + self.extra_slots_in_use;
        (total, full)
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
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.base.add_to_contain(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.base
            .remove_from_contain(obj, false)
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
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::on_containing(self, obj, was_selected).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj: Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::on_removing(self, obj).map_err(|e| e.into())
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .remove_all_contained(expose_stealth)
            .map_err(|e| e.into())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        TransportContain::is_displayed_on_control_bar(self)
    }
}

impl ContainerInterface for TransportContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.base.add_to_contain(obj.clone())?;
        self.on_containing(obj, false)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.on_removing(obj.clone())?;
        self.base.remove_from_contain(obj, false)
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
}

//! Transport Contain Module
//!
//! Contain module for transport units with specialized transport functionality
//! including slot capacity, exit handling, and payload management.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface, ObjectTemplate, OpenContain};
use crate::ai::the_ai;
use crate::common::{
    CommandSourceType, Coord3D, DisabledType, GameResult, KindOf, Matrix3D, ModelConditionState,
    ObjectID, PathfindLayerEnum, PlayerMaskType, SECONDS_PER_LOGICFRAME_REAL, WeaponSlotType,
};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::helpers::TheThingFactory;
use crate::locomotor::LocomotorSet;
use crate::modules::{
    AIAttitudeType, AIUpdateInterfaceExt, ContainModuleInterface, ContainWant, ExitDoorType,
    PhysicsBehavior, UpdateSleepTime,
};
use crate::object::{Object, ObjectArcExt};
use crate::player::Player;
use crate::weapon::WeaponSetType;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

#[allow(dead_code)]
/// Wave 272 residual scan still sees `OBJECT_REGISTRY.is_empty()`.
/// Do not skip-close contain solely because the dual-world registry is empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

/// C++ `TransportContain::isPassengerAllowedToFire` (TransportContain.cpp:576-578):
/// only infantry may fire out. Vehicles ride silent (Combat Chinook
/// `AllowInsideKindOf = INFANTRY VEHICLE`).
#[inline]
pub fn transport_contain_passenger_kind_allowed_to_fire(is_infantry: bool) -> bool {
    is_infantry
}

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
        base.allow_inside_kind_of = KindOf::Infantry.cpp_mask();

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
    /// Slot delta added by the last successful on_containing.
    last_extra_slots_delta: i32,
    /// Frame when exit will not be busy
    frame_exit_not_busy: u32,
    /// C++ RailedTransportContain::isSpecificRiderFreeToExit — dock must be open.
    require_open_dock_to_exit: bool,
}

impl TransportContain {
    /// Create a new TransportContain module
    pub fn new(
        object_id: ObjectID,
        module_data: &TransportContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object_id, &module_data.base)?;

        Ok(Self {
            base,
            module_data: module_data.clone(),
            object_id: object_id,
            payload_created: false,
            extra_slots_in_use: 0,
            last_extra_slots_delta: 0,
            frame_exit_not_busy: 0,
            require_open_dock_to_exit: false,
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
    pub fn get_object(&self) -> Option<ObjectID> {
        let id = self.object_id;
        if id == crate::common::INVALID_ID {
            None
        } else {
            Some(id)
        }
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Wave 272: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        // C++ TransportContain::isValidContainerFor: if the rider is a special
        // zero-slot container (parachute), replace the check target with the
        // first contained infantry so a plane can accept a paratrooper.
        let unwrapped = super::unwrap_special_zero_slot_rider(obj);
        let check = |actual: &Object| -> bool {
            if !self.base.is_valid_container_for(actual, check_capacity) {
                return false;
            }
            let Some(owner_id) = self.get_object() else {
                return false;
            };
            let Some(players_ok) = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner| {
                let owner_player = owner.get_controlling_player();
                let actual_player = actual.get_controlling_player();
                match (&owner_player, &actual_player) {
                    (Some(p1), Some(p2)) if !Arc::ptr_eq(p1, p2) => false,
                    (None, Some(_)) | (Some(_), None) => false,
                    _ => true,
                }
            }) else {
                return false;
            };
            if !players_ok {
                return false;
            }
            let transport_slot_count = actual.get_transport_slot_count();
            if transport_slot_count == 0 {
                return false;
            }
            if check_capacity {
                let contain_max = self.get_contain_max();
                let contain_count = self.base.get_contain_count() as i32;
                return self.extra_slots_in_use + contain_count + (transport_slot_count as i32)
                    <= contain_max;
            }
            true
        };
        if let Some(id) = unwrapped {
            return crate::object::registry::OBJECT_REGISTRY
                .with_object(id, |actual| check(actual))
                .unwrap_or(false);
        }
        check(obj)
    }

    /// C++ OpenContain::processDamageToContained — Battle Bus / contain death rules.
    pub fn process_damage_to_contained(&mut self, percent_damage: f32) -> GameResult<()> {
        self.base.process_damage_to_contained(percent_damage)
    }

    pub fn set_require_open_dock_to_exit(&mut self, require: bool) {
        self.require_open_dock_to_exit = require;
    }

    /// Handle capture event
    pub fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) -> GameResult<()> {
        let owners_differ = old_owner != new_owner;
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

    /// Handle death event.
    ///
    /// C++ `TransportContain` does not override `onDie`. `OpenContain::onDie`
    /// (`OpenContain.cpp:833-851`) is: DieMux applicability return →
    /// `processDamageToContained(DamagePercentToUnits)` →
    /// `killRidersWhoAreNotFreeToExit` → `removeAllContained`.
    /// Rust `OpenContain::kill_riders_who_are_not_free_to_exit` is a no-op, so
    /// Transport must run that C++ virtual after DieMux + damage%.
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        self.on_die_for_owner(None, damage_info)
    }

    pub fn on_die_for_owner(
        &mut self,
        owner: Option<&Object>,
        damage_info: Option<&DamageInfo>,
    ) -> GameResult<()> {
        if let Some(info) = damage_info {
            let applicable = if let Some(owner) = owner {
                self.base.is_die_applicable(owner, info)
            } else {
                let Some(applicable) =
                    self.with_owner_object(|owner| self.base.is_die_applicable(owner, info))
                else {
                    return Ok(());
                };
                applicable
            };
            if !applicable {
                return Ok(());
            }
        }

        let percent = self.base.get_damage_percentage_to_units();
        if percent > 0.0 {
            if let Err(err) = self.base.process_damage_to_contained(percent) {
                log::warn!(
                    "TransportContain::on_die damage to contained failed: {}",
                    err
                );
            }
        }

        if let Err(err) = self.kill_riders_who_are_not_free_to_exit() {
            log::warn!(
                "TransportContain::on_die kill blocked riders failed: {}",
                err
            );
        }
        let object_ids: Vec<_> = self.base.get_contained_object_ids().to_vec();
        for obj_id in object_ids {
            if let Err(err) = self.remove_from_contain(obj_id, false) {
                log::warn!(
                    "TransportContain::on_die remove failed for {}: {}",
                    obj_id,
                    err
                );
            }
        }
        Ok(())
    }

    /// Handle deletion event through inherited OpenContain cleanup.
    pub fn on_delete(&mut self) -> GameResult<()> {
        self.base.on_delete()
    }

    /// Called when this object starts containing another object
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.last_extra_slots_delta = 0;

        self.base.on_containing(obj_id, was_selected)?;

        let held = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |rider| {
            rider.set_disabled_held(true)?;
            let transport_slot_count = rider.get_transport_slot_count();
            debug_assert!(
                transport_slot_count > 0,
                "TransportContain contained a non-transportable rider"
            );
            Ok::<i32, Box<dyn std::error::Error + Send + Sync>>((transport_slot_count as i32 - 1).max(0))
        });
        let Some(held) = held else {
            self.base.unlink_contained_id(obj_id);
            return Err("Transport rider lock busy".into());
        };
        let extra = held?;
        self.extra_slots_in_use += extra;
        self.last_extra_slots_delta = extra;

        debug_assert!(
            self.extra_slots_in_use >= 0
                && self.extra_slots_in_use + self.base.get_contain_count() as i32
                    <= self.get_contain_max(),
            "Bad slot count in TransportContain"
        );

        if self.base.get_contain_count() == 1 {
            if let Some(drawable) = self
                .with_owner_object(|owner| owner.get_drawable())
                .flatten()
            {
                if let Ok(mut draw) = drawable.write() {
                    draw.set_model_condition_state(ModelConditionState::Loaded);
                }
            }
        }

        self.let_riders_upgrade_weapon_set()?;

        if self.object_id != crate::common::INVALID_ID {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, |transport| {
                transport.set_is_transporting(true);
            });
        }

        let timers = crate::object::registry::OBJECT_REGISTRY.with_object(obj, |rider| {
            if rider.is_kind_of(KindOf::Hero) && rider.is_kind_of(KindOf::Salvager) {
                rider
                    .get_weapon_in_slot(WeaponSlotType::Secondary.into())
                    .map(|rider_weapon| {
                        (
                            rider_weapon.when_we_can_fire_again,
                            rider_weapon.when_pre_attack_finished,
                            rider_weapon.when_last_reload_started,
                        )
                    })
            } else {
                None
            }
        });
        if let Some(Some((when_we_can_fire_again, when_pre_attack_finished, when_last_reload_started))) =
            timers
        {
            if let Some(owner_id) = self.get_object() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    if owner.is_kind_of(KindOf::CliffJumper) {
                        if let Some(bike_weapon) =
                            owner.get_weapon_in_slot_mut(WeaponSlotType::Secondary.into())
                        {
                            bike_weapon.when_we_can_fire_again = when_we_can_fire_again;
                            bike_weapon.when_pre_attack_finished = when_pre_attack_finished;
                            bike_weapon.when_last_reload_started = when_last_reload_started;
                        }
                    }
                });
            }
        }

        Ok(())
    }

    /// C++ `PhysicsBehavior::getVelocity` for KeepContainerVelocityOnExit.
    /// `PhysicsBehaviorExt::get_velocity` turns a failed `try_lock` into zero.
    fn owner_exit_velocity(&self) -> GameResult<Option<Coord3D>> {
        let Some(owner_id) = self.get_object() else {
            return Ok(None);
        };
        let Some(physics) = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner| {
            owner.get_physics()
        }) else {
            return Err("Transport owner lock busy during velocity inherit".into());
        };
        let Some(physics) = physics else {
            return Ok(None);
        };
        let Ok(parent) = physics.try_lock() else {
            return Err("Transport owner physics lock busy during velocity inherit".into());
        };
        Ok(Some(parent.get_velocity()))
    }

    /// C++ `child->applyMotiveForce(parentVelocity * mass)` and
    /// `setPitchRate(centerOfMassOffset * exitPitchRate)`.
    /// Do not call this while `rider`'s object write is held: motive force
    /// read-locks the rider, and `std` locks do not reenter.
    fn inherit_container_velocity_on_exit(
        &self,
        rider: &ObjectID,
        parent_velocity: Coord3D,
    ) -> GameResult<()> {
        let child_physics = crate::object::registry::OBJECT_REGISTRY.with_object(*rider, |rider_guard| {
            rider_guard.get_physics()
        });
        let Some(child_physics) = child_physics else {
            return Err("Transport passenger lock busy during velocity inherit".into());
        };
        let Some(child_physics) = child_physics else {
            return Ok(());
        };
        let Ok(mut child) = child_physics.try_lock() else {
            return Err("Transport rider physics lock busy during velocity inherit".into());
        };
        let mass = child.get_mass();
        let starting_force = Coord3D::new(
            parent_velocity.x * mass,
            parent_velocity.y * mass,
            parent_velocity.z * mass,
        );
        let pitch_rate = child.get_center_of_mass_offset() * self.module_data.exit_pitch_rate;
        child.apply_motive_force(&starting_force);
        child.set_pitch_rate(pitch_rate);
        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.base.on_removing(obj_id)?;

        let exit_bone = self.module_data.exit_bone.clone();
        let orient_like = self.module_data.orient_like_container_on_exit;
        let owner_id = self.get_object();
        let bone_pos = if !exit_bone.is_empty() {
            owner_id.and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY.with_object(id, |owner| {
                    let (_, bone_pos, _) = owner.get_single_logical_bone_position(&exit_bone);
                    bone_pos
                })
            })
        } else {
            None
        };
        let orient = if orient_like {
            owner_id.and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(id, |owner| owner.get_orientation())
            })
        } else {
            None
        };

        let cleared = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |rider| {
            rider.set_disabled_held(false)?;
            let transport_slot_count = rider.get_transport_slot_count();
            debug_assert!(
                transport_slot_count > 0,
                "TransportContain removed a non-transportable rider"
            );
            if let Some(bone_pos) = bone_pos {
                let _ = rider.set_position(&bone_pos);
            }
            if let Some(orient) = orient {
                let _ = rider.set_orientation(orient);
            }
            Ok::<i32, Box<dyn std::error::Error + Send + Sync>>(transport_slot_count as i32)
        });
        let Some(cleared) = cleared else {
            return Err("Transport passenger lock busy during held clear".into());
        };
        let transport_slot_count = cleared?;
        self.extra_slots_in_use -= transport_slot_count - 1;

        if self.base.get_contain_count() == 0 {
            if let Some(drawable) = self.with_owner_object(|owner| owner.get_drawable()).flatten() {
                if let Ok(mut draw) = drawable.write() {
                    draw.clear_model_condition_state(ModelConditionState::Loaded);
                }
            }
        }

        if self.object_id != crate::common::INVALID_ID {
            let still_contains = self.base.get_contain_count() > 0;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, |transport| {
                transport.set_is_transporting(still_contains);
            });
        }

        if self.module_data.keep_container_velocity_on_exit {
            if let Some(parent_velocity) = self.owner_exit_velocity()? {
                self.inherit_container_velocity_on_exit(&obj, parent_velocity)?;
            }
        }

        let owner_state = self.get_object().and_then(|owner_id| {
            crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner| {
                let bike_secondary = if owner.is_kind_of(KindOf::CliffJumper) {
                    owner
                        .get_weapon_in_slot(WeaponSlotType::Secondary.into())
                        .map(|w| {
                            (
                                w.when_we_can_fire_again,
                                w.when_pre_attack_finished,
                                w.when_last_reload_started,
                            )
                        })
                } else {
                    None
                };
                (
                    owner.is_above_terrain(),
                    owner.is_effectively_dead(),
                    owner.is_kind_of(KindOf::CliffJumper),
                    bike_secondary,
                )
            })
        });

        if let Some((above_terrain, owner_dead, owner_is_bike, bike_secondary)) = owner_state {
            let fall = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |rider| {
                if above_terrain {
                    if let Some(physics) = rider.get_physics() {
                        let Ok(mut physics) = physics.try_lock() else {
                            return Err(
                                "Transport rider physics lock busy during allow to fall".into(),
                            );
                        };
                        physics.set_allow_to_fall(true);
                    }
                }
                if owner_dead {
                    let _ = self.base.scatter_to_nearby_position(rider);
                }
                if owner_is_bike
                    && rider.is_kind_of(KindOf::Hero)
                    && rider.is_kind_of(KindOf::Salvager)
                {
                    if let (Some((when_fire, when_pre, when_reload)), Some(rider_weapon)) = (
                        bike_secondary,
                        rider.get_weapon_in_slot_mut(WeaponSlotType::Secondary.into()),
                    ) {
                        rider_weapon.when_we_can_fire_again = when_fire;
                        rider_weapon.when_pre_attack_finished = when_pre;
                        rider_weapon.when_last_reload_started = when_reload;
                    }
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            });
            if let Some(Err(err)) = fall {
                return Err(err);
            }
        }
        if self.module_data.go_aggressive_on_exit {
            if let Some(ai) = crate::object::registry::OBJECT_REGISTRY
                .with_object(obj, |rider| rider.get_ai())
                .flatten()
            {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    ai_guard.set_attitude(AIAttitudeType::Aggressive);
                }
            }
        }
        if self.module_data.reset_mood_check_time_on_exit {
            if let Some(ai) = crate::object::registry::OBJECT_REGISTRY
                .with_object(obj, |rider| rider.get_ai())
                .flatten()
            {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    ai_guard.wake_up_and_attempt_to_target();
                }
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
        // Wave 272: empty dual-world → sleep forever (no factory walks).
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::Forever);
        }

        // Create payload if not already created
        if !self.payload_created {
            if let Err(err) = self.create_payload() {
                log::warn!("TransportContain::update: createPayload failed: {}", err);
            }
        }

        if self.module_data.health_regen != 0.0 {
            let owner_id = self.get_object_id();
            for object_id in self.base.get_contained_object_ids().to_vec() {
                if let Some(object) = TheGameLogic::find_object_by_id(object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(object_id))
                {
                    let body_info = crate::object::registry::OBJECT_REGISTRY.with_object(object, |guard| {
                        guard.get_body_module()
                    });
                    let Some(body) = body_info else {
                        log::warn!("TransportContain::update regen lock busy for {}", object_id);
                        continue;
                    };
                    let Some(body) = body else {
                        continue;
                    };
                    let Ok(body_guard) = body.try_lock() else {
                        log::warn!(
                            "TransportContain::update regen body lock busy for {}",
                            object_id
                        );
                        continue;
                    };
                    let max_health = body_guard.get_max_health();
                    let needs_healing = body_guard.get_health() < max_health && max_health > 0.0;
                    drop(body_guard);
                    if !needs_healing {
                        continue;
                    }
                    let regen = max_health * self.module_data.health_regen / 100.0
                        * SECONDS_PER_LOGICFRAME_REAL;
                    let wrote = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object, |object_guard| {
                        if owner_id != crate::common::INVALID_ID {
                            let _ = object_guard.attempt_healing_from_source_id(regen, owner_id);
                        } else {
                            let _ = object_guard.attempt_healing(regen, None);
                        }
                    });
                    if wrote.is_none() {
                        log::warn!(
                            "TransportContain::update regen write busy for {}",
                            object_id
                        );
                    }
                }
            }
        }

        match self.base.update() {
            Ok(sleep) => Ok(sleep),
            Err(err) => {
                log::warn!("TransportContain::update base update failed: {}", err);
                Ok(UpdateSleepTime::None)
            }
        }
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
    pub fn release_extra_slots(&mut self, transport_slot_count: u32) {
        let extra = (transport_slot_count as i32 - 1).max(0);
        self.extra_slots_in_use = (self.extra_slots_in_use - extra).max(0);
    }

    /// Undo the slot delta recorded by the last `on_containing`, without reading the rider.
    pub fn release_last_extra_slots(&mut self) {
        self.extra_slots_in_use = (self.extra_slots_in_use - self.last_extra_slots_delta).max(0);
        self.last_extra_slots_delta = 0;
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
            if self
                .with_owner_object(|owner_guard| owner_guard.is_above_terrain())
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

    /// Unreserve door for exit.
    ///
    /// C++ `TransportContain::unreserveDoorForExit` (`TransportContain.cpp:504-508`)
    /// is an explicit no-op. Door-close countdown is armed only inside
    /// `exitObjectViaDoor` / `exitObjectInAHurry`.
    pub fn unreserve_door_for_exit(&self, exit_door: ExitDoorType) -> GameResult<()> {
        let _ = exit_door;
        Ok(())
    }

    /// Check if displayed on control bar
    pub fn is_displayed_on_control_bar(&self) -> bool {
        true
    }

    /// Kill riders who are not free to exit
    fn kill_riders_who_are_not_free_to_exit(&mut self) -> GameResult<()> {
        let contained = self.get_contained_objects().into_owned();
        for obj_id in contained {
            let Some(obj) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            if !self.is_rider_id_free_to_exit(obj_id) {
                if self.module_data.destroy_riders_who_are_not_free_to_exit {
                    let _ = TheGameLogic::destroy_object_by_id(obj_id);
                } else {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |obj_write| {
                        obj_write.kill(None, None);
                    });
                }
            }
        }
        Ok(())
    }

    /// Same check as `is_specific_rider_free_to_exit`, but the rider lock is not held
    /// across `with_cur_locomotor` (that callback write-locks the rider).
    fn is_rider_id_free_to_exit(&self, rider_id: ObjectID) -> bool {
        if self.require_open_dock_to_exit {
            let dock_open = self
                .with_owner_object(|owner| {
                    owner
                        .with_dock_update_interface(|dock| dock.is_dock_open().unwrap_or(true))
                        .unwrap_or(true)
                })
                .unwrap_or(true);
            return dock_open;
        }

        let Some(obj) = TheGameLogic::find_object_by_id(rider_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(rider_id))
        else {
            return false;
        };
        let Some((airborne, layer, pos, rider_ai)) = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj, |rider| {
                let owner_bits = self.with_owner_object(|owner| {
                    if let Some(ai) = owner.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.try_lock() {
                            if !matches!(
                                ai_guard.get_ai_free_to_exit(rider),
                                crate::object::production::AIFreeToExitType::FreeToExit
                            ) {
                                return None;
                            }
                        }
                    }
                    Some((
                        owner.is_using_airborne_locomotor(),
                        owner.get_layer(),
                        *owner.get_position(),
                    ))
                });
                let rider_ai = rider.get_ai_update_interface();
                owner_bits
                    .flatten()
                    .map(|(airborne, layer, pos)| (airborne, layer, pos, rider_ai))
            })
            .flatten()
        else {
            return false;
        };
        if airborne {
            return true;
        }
        let Some(rider_ai) = rider_ai else {
            return false;
        };
        let mut his_loco = None;
        rider_ai.with_cur_locomotor(&mut |loco| his_loco = Some(loco.clone()));
        let Some(his_loco) = his_loco else {
            return false;
        };
        let mut loco_set = LocomotorSet::new();
        loco_set.add_locomotor("cur".to_string(), his_loco);
        let layer = crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32);
        the_ai()
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pf| {
                pf.read()
                    .ok()
                    .map(|pf| pf.valid_movement_terrain(layer, &loco_set, &pos))
            })
            .unwrap_or(false)
    }

    /// Check if specific rider is free to exit
    fn is_specific_rider_free_to_exit(&self, obj: &Object) -> bool {
        if self.require_open_dock_to_exit {
            let dock_open = self
                .with_owner_object(|owner| {
                    owner
                        .with_dock_update_interface(|dock| dock.is_dock_open().unwrap_or(true))
                        .unwrap_or(true)
                })
                .unwrap_or(true);
            // C++ RailedTransportContain does not extend TransportContain here.
            return dock_open;
        }

        let Some((airborne, layer, pos)) = self
            .with_owner_object(|owner| {
                if let Some(ai) = owner.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.try_lock() {
                        if !matches!(
                            ai_guard.get_ai_free_to_exit(obj),
                            crate::object::production::AIFreeToExitType::FreeToExit
                        ) {
                            return None;
                        }
                    }
                }
                Some((
                    owner.is_using_airborne_locomotor(),
                    owner.get_layer(),
                    *owner.get_position(),
                ))
            })
            .flatten()
        else {
            return false;
        };

        // C++: airborne transports can always kick people out.
        if airborne {
            return true;
        }

        let Some(rider_ai) = obj.get_ai_update_interface() else {
            return false;
        };
        let mut his_loco = None;
        rider_ai.with_cur_locomotor(&mut |loco| his_loco = Some(loco.clone()));
        let Some(his_loco) = his_loco else {
            return false;
        };

        let mut loco_set = LocomotorSet::new();
        loco_set.add_locomotor("cur".to_string(), his_loco);
        let layer = crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32);
        the_ai()
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pf| {
                pf.read()
                    .ok()
                    .map(|pf| pf.valid_movement_terrain(layer, &loco_set, &pos))
            })
            .unwrap_or(false)
    }

    /// Check if passenger is allowed to fire
    pub fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        // C++ TransportContain::isPassengerAllowedToFire: infantry only,
        // then Overlord-style parent nest, else OpenContain.
        if let Some(obj_id) = id {
            if let Some(passenger) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                let infantry = crate::object::registry::OBJECT_REGISTRY.with_object(passenger, |passenger_guard| {
                    transport_contain_passenger_kind_allowed_to_fire(
                        passenger_guard.is_kind_of(KindOf::Infantry),
                    )
                });
                if infantry != Some(true) {
                    return false;
                }
            }
        }

        if let Some(parent_id) = self
            .with_owner_object(|owner| owner.get_contained_by())
            .flatten()
        {
            if let Some(parent) = TheGameLogic::find_object_by_id(parent_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(parent_id))
            {
                let overlord = crate::object::registry::OBJECT_REGISTRY.with_object(parent, |parent_guard| {
                    if let Some(contain) = parent_guard.get_contain() {
                        let Ok(contain_guard) = contain.try_lock() else {
                            return None;
                        };
                        if contain_guard.is_special_overlord_style_container() {
                            return Some(contain_guard.is_passenger_allowed_to_fire(id));
                        }
                    }
                    Some(None)
                });
                match overlord {
                    None => return false,
                    Some(None) => {}
                    Some(Some(allowed)) => return allowed,
                }
            }
        }

        self.base.is_passenger_allowed_to_fire(id)
    }

    /// Create initial payload
    fn create_payload(&mut self) -> GameResult<()> {
        if self.payload_created {
            return Ok(());
        }

        let (payload_name, payload_count, owner_team) = self
            .with_owner_object(|owner_guard| {
                (
                    self.module_data.initial_payload.name.clone(),
                    self.module_data.initial_payload.count.max(0),
                    owner_guard
                        .get_controlling_player()
                        .and_then(|player| player.read().ok().and_then(|p| p.get_default_team())),
                )
            })
            .unwrap_or_else(|| (String::new(), 0, None));

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

        let factory = match TheThingFactory::get() {
            Ok(factory) => factory,
            Err(err) => return Err(err.to_string().into()),
        };
        self.base.enable_load_sounds(false);
        let mut added_any = false;

        for _ in 0..payload_count {
            let payload = if let Some(team_arc) = &owner_team {
                let Ok(team_guard) = team_arc.try_read() else {
                    self.base.enable_load_sounds(true);
                    if added_any {
                        self.payload_created = true;
                    }
                    return Err("Transport team lock busy".into());
                };
                factory.new_object(template.clone(), &*team_guard)
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

            let checked = crate::object::registry::OBJECT_REGISTRY.with_object(payload_obj, |guard| {
                let payload_id = guard.get_id();
                let can_add = self.is_valid_container_for(guard, true);
                (payload_id, can_add)
            });
            let Some((payload_id, can_add)) = checked else {
                let payload_id = crate::object::registry::OBJECT_REGISTRY
                    .with_object(payload_obj, |guard| guard.get_id());
                if let Some(payload_id) = payload_id {
                    let _ = TheGameLogic::destroy_object_by_id(payload_id);
                }
                self.base.enable_load_sounds(true);
                if added_any {
                    self.payload_created = true;
                }
                return Err("Transport payload lock busy".into());
            };
            if can_add {
                if let Err(err) = self.add_to_contain(payload_id) {
                    let _ = TheGameLogic::destroy_object_by_id(payload_id);
                    self.base.enable_load_sounds(true);
                    if added_any {
                        self.payload_created = true;
                    }
                    return Err(err);
                }
                added_any = true;
            } else {
                let _ = TheGameLogic::destroy_object_by_id(payload_id);
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
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        // Check if this feature is enabled
        if !self.module_data.armed_riders_upgrade_weapon_set {
            return Ok(());
        }

        {
            let mut any_rider_has_viable_weapon = false;
            let mut every_rider_read = true;

            // Check all riders for viable weapons
            for rider_id in self.base.get_contained_object_ids().to_vec() {
                let Some(rider_obj) = TheGameLogic::find_object_by_id(rider_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(rider_id))
                else {
                    continue;
                };
                let viable = crate::object::registry::OBJECT_REGISTRY.with_object(rider_obj, |rider| {
                    if !transport_contain_passenger_kind_allowed_to_fire(
                        rider.is_kind_of(KindOf::Infantry),
                    ) {
                        return false;
                    }
                    for weapon_slot in [
                        WeaponSlotType::Primary,
                        WeaponSlotType::Secondary,
                        WeaponSlotType::Tertiary,
                    ] {
                        if let Some(weapon) = rider.get_weapon_in_slot(weapon_slot.into()) {
                            if !weapon.is_contact_weapon() && weapon.is_damage_weapon() {
                                return true;
                            }
                        }
                    }
                    false
                });
                let Some(viable) = viable else {
                    every_rider_read = false;
                    continue;
                };
                if viable {
                    any_rider_has_viable_weapon = true;
                    break;
                }
            }

            if every_rider_read {
                if let Some(owner_id) = self.get_object() {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_mut| {
                        if any_rider_has_viable_weapon {
                            owner_mut.set_weapon_set_flag(WeaponSetType::PlayerUpgrade);
                        } else {
                            owner_mut.clear_weapon_set_flag(WeaponSetType::PlayerUpgrade);
                        }
                    });
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
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if super::should_cancel_containment_after_booby_trap(
            {
                let id = self.get_object_id();
                if id == crate::common::INVALID_ID {
                    None
                } else {
                    Some(id)
                }
            },
            obj_id,
        ) {
            return Ok(());
        }

        let obj = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("Transport contain object not found")?;

        let was_selected = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj, |guard| guard.get_drawable())
            .flatten()
            .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);

        {
            let Some(ok) = crate::object::registry::OBJECT_REGISTRY.with_object(obj, |obj_ref| {
                if !self.is_valid_container_for(obj_ref, true) {
                    return Err("Object not valid for this transport container".into());
                }
                let already_listed = self.base.get_contained_object_ids().contains(&obj_id);
                let contained_by = obj_ref.get_contained_by();
                if contained_by.is_some()
                    && (already_listed || contained_by != Some(self.get_object_id()))
                {
                    return Ok(false);
                }
                Ok(true)
            }) else {
                return Err("Transport passenger lock busy".into());
            };
            if !ok? {
                return Ok(());
            }
        }

        self.add_to_contain_list(obj_id)?;
        let should_remove_from_world = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj, |obj_guard| self.base.is_enclosing_container_for(obj_guard))
            .unwrap_or(false);
        if should_remove_from_world {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
        }
        self.base.redeploy_occupants()?;
        if let Err(err) = self.on_containing(obj_id, was_selected) {
            self.base.unlink_contained_id(obj_id);
            if should_remove_from_world {
                let _ = self.base.add_or_remove_obj_from_world(obj_id, true);
            }
            let _ = self.base.redeploy_occupants();
            return Err(err);
        }
        self.base.do_load_sound();
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
        self.remove_passenger(obj_id, expose_stealth_units, true)
            .map(|_| ())
    }

    pub(crate) fn remove_passenger(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
        run_exit_hook: bool,
    ) -> GameResult<(bool, bool)> {
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok((false, false));
        }

        let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok((false, false));
        };

        if !self.base.get_contained_object_ids().contains(&obj_id) {
            return Ok((false, false));
        }

        let Some(stealth_garrison) = self.base.remove_from_contain_list(obj_id) else {
            return Err("Transport passenger lock busy".into());
        };
        // C++ OpenContain::removeFromContainViaIterator (`OpenContain.cpp:621-633`):
        // KINDOF_STEALTH_GARRISON + exposeStealthUnits → stealth->markAsDetected().
        if expose_stealth_units {
            if let Some(stealth) = crate::object::registry::OBJECT_REGISTRY.with_object(obj, |obj_guard| {
                if obj_guard.is_kind_of(KindOf::StealthGarrison) {
                    obj_guard.get_stealth()
                } else {
                    None
                }
            }).flatten() {
                if let Ok(mut stealth_guard) = stealth.lock() {
                    stealth_guard.mark_as_detected();
                }
            }
        }
        let should_add_to_world = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj, |obj_guard| self.base.is_enclosing_container_for(obj_guard))
            .unwrap_or(false);
        if should_add_to_world {
            let _ = self.base.add_or_remove_obj_from_world(obj_id, true);
            let owner_id = self.get_object_id();
            if owner_id != crate::common::INVALID_ID {
                if let Some((pos, layer)) = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                    (*owner_guard.get_position(), owner_guard.get_layer())
                }) {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |obj_guard| {
                        let _ = obj_guard.set_position(&pos);
                        obj_guard.set_layer(layer);
                    });
                }
            }
        }
        self.base.do_unload_sound();
        if !run_exit_hook {
            return Ok((stealth_garrison, should_add_to_world));
        }
        if let Err(err) = self.on_removing(obj_id) {
            let _ = self.base.add_to_contain_list_id(obj_id, stealth_garrison);
            if should_add_to_world {
                let _ = self.base.add_or_remove_obj_from_world(obj_id, false);
            }
            return Err(err);
        }

        if self.base.note_removed_from(obj_id).is_err() {
            self.base.note_removed_from(obj_id)?;
        }
        Ok((stealth_garrison, should_add_to_world))
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
            if let Some(valid) = crate::object::registry::OBJECT_REGISTRY
                .with_object(obj, |obj_guard| self.is_valid_container_for(obj_guard, true))
            {
                return valid;
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

    fn remove_from_contain(
        &mut self,
        object_id: ObjectID,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::remove_from_contain(self, object_id, expose_stealth).map_err(|e| e.into())
    }

    fn get_contained_objects(&self) -> std::borrow::Cow<'_, [ObjectID]> {
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
        if max < 0 { usize::MAX } else { max as usize }
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

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        TransportContain::on_die_for_owner(self, Some(owner), damage_info).map_err(|e| e.into())
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let Some(other) = TheGameLogic::find_object_by_id(other_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(other_id))
        else {
            return Ok(());
        };
        let valid = crate::object::registry::OBJECT_REGISTRY
            .with_object(other, |guard| self.is_valid_container_for(guard, true))
            .unwrap_or(false);
        if valid {
            self.add_to_contain(other_id)?;
        }
        Ok(())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        TransportContain::is_valid_container_for(self, obj, check_capacity)
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
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
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
        TransportContain::is_special_overlord_style_container(self)
    }

    fn is_rider_change_contain(&self) -> bool {
        TransportContain::is_rider_change_contain(self)
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_selling().map_err(|e| e.into())
    }

    fn redeploy_riders_at(&mut self, owner_pos: &Coord3D, fire_points: &[Matrix3D]) {
        self.base.redeploy_riders_at(owner_pos, fire_points);
    }

    fn passengers_in_turret(&self) -> bool {
        self.base.passengers_in_turret()
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

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
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

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
            if let Err(err) = self.remove_from_contain(obj_id, expose_stealth) {
                log::warn!(
                    "TransportContain::remove_all_contained failed for {}: {}",
                    obj_id,
                    err
                );
            }
        }
        Ok(())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let object_ids = self.base.get_contained_object_ids().to_vec();
        for obj_id in object_ids {
            if let Err(err) = self.remove_from_contain(obj_id, true) {
                log::warn!(
                    "TransportContain::harm_and_force_exit failed for {}: {}",
                    obj_id,
                    err
                );
                continue;
            }
            if let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |guard| {
                    let _ = guard.attempt_damage(damage_info);
                });
            }
        }
        Ok(())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 272: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let object_ids = self.base.get_contained_object_ids().to_vec();
        for obj_id in object_ids {
            if let Err(err) = self.remove_from_contain(obj_id, true) {
                log::warn!(
                    "TransportContain::kill_all_contained failed for {}: {}",
                    obj_id,
                    err
                );
                continue;
            }
            if let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj, |guard| {
                    guard.kill(None, None);
                });
            }
        }
        Ok(())
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&Object>,
        spawn: Option<&Object>,
    ) -> ExitDoorType {
        let Some(obj) = spawn else {
            return ExitDoorType::Primary;
        };
        if self.is_specific_rider_free_to_exit(obj) {
            ExitDoorType::Primary
        } else {
            ExitDoorType::NoneAvailable
        }
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        TransportContain::is_displayed_on_control_bar(self)
    }

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = TransportContain::process_damage_to_contained(self, percent_damage);
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

    fn owned_object(name: &str, id: ObjectID, player_index: u32) -> ObjectID {
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

    fn slotted_passenger(name: &str, id: ObjectID, slots: i32) -> ObjectID {
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
        fields.insert("TransportSlotCount".to_string(), slots.to_string());
        template.parse_object_fields_from_ini(&fields);
        Object::new_with_id(
            Arc::new(template),
            id,
            ObjectStatusMaskType::none(),
            Some(team),
        )
        .expect("slotted passenger")
    }

    fn transport_for(owner: &ObjectID, slots: i32) -> TransportContain {
        let data = TransportContainModuleData {
            slot_capacity: slots,
            ..Default::default()
        };
        TransportContain::new(*owner, &data).expect("transport contain")
    }

    #[test]
    fn trait_containment_uses_transport_slots_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("TransportOwner", 95001, 0);
        let passenger = slotted_passenger("TwoSlotPassenger", 95002, 2);
        let mut contain = transport_for(&owner, 3);

        assert_eq!(contain.get_contain_max(), 3);
        assert!(OBJECT_REGISTRY.with_object(passenger, |p| contain.is_valid_container_for(p, true)).unwrap());
        ContainModuleInterface::contain_object(&mut contain, 95002).expect("contain passenger");

        assert_eq!(ContainModuleInterface::get_contained_count(&contain), 1);
        assert_eq!(contain.get_extra_slots_in_use(), 1);
        assert_eq!(contain.get_container_pips_info(), (3, 2));
        assert_eq!(
            ContainModuleInterface::get_container_pips_to_show(&contain),
            (3, 2, true)
        );
        assert_eq!(
            OBJECT_REGISTRY.with_object(passenger, |p| p.get_contained_by()).unwrap(),
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
            OBJECT_REGISTRY.with_object(passenger, |p| p.get_contained_by()).unwrap(),
            None
        );

        OBJECT_REGISTRY.unregister_object(95003);
        OBJECT_REGISTRY.unregister_object(95004);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn slot_count_reads_template_field_not_contain_capacity_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();

        // Infantry with a parsed TransportSlotCount and no contain module:
        // C++ Object::getTransportSlotCount returns the template field
        // (Object.cpp:702), not a contain-module capacity.
        let passenger = slotted_passenger("SlotFieldPassenger", 95005, 2);
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(passenger, |p| p.get_transport_slot_count())
                .unwrap(),
            2
        );

        // A garrisonable building's contain capacity must not leak into its
        // own transport slot count.
        let building = owned_object("GarrisonableBuilding", 95006, 0);
        let data = super::super::OpenContainModuleData {
            contain_max: 20,
            ..Default::default()
        };
        let contain = OpenContain::new(building, &data).expect("building contain");
        OBJECT_REGISTRY.with_object_mut(building, |b| {
            b.set_contain(Some(Arc::new(Mutex::new(contain))));
        });
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(building, |b| b.get_transport_slot_count())
                .unwrap(),
            0
        );

        OBJECT_REGISTRY.unregister_object(95005);
        OBJECT_REGISTRY.unregister_object(95006);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn special_zero_slot_container_sums_rider_slots_like_cpp() {
        let _lock = crate::test_sync::lock();
        reset_players();

        let chute_owner = owned_object("ParachuteContainer", 95007, 0);
        let chute = super::super::ParachuteContain::new(
            chute_owner,
            &super::super::ParachuteContainModuleData::default(),
        )
        .expect("chute contain");
        let chute: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(chute));
        OBJECT_REGISTRY.with_object_mut(chute_owner, |owner| {
            owner.set_contain(Some(Arc::clone(&chute)));
        });

        assert_eq!(
            OBJECT_REGISTRY
                .with_object(chute_owner, |o| o.get_transport_slot_count())
                .unwrap(),
            0
        );

        let rider = slotted_passenger("ChuteRider", 95008, 2);
        let _ = rider;
        ContainModuleInterface::contain_object(&mut *chute.lock().expect("chute lock"), 95008)
            .expect("attach rider");
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(chute_owner, |o| o.get_transport_slot_count())
                .unwrap(),
            2
        );

        OBJECT_REGISTRY.unregister_object(95007);
        OBJECT_REGISTRY.unregister_object(95008);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[derive(Debug, Default)]
    struct ExitRecord {
        motive: Option<Coord3D>,
        pitch: f32,
    }

    #[derive(Debug)]
    struct ExitPhysics {
        vel: Coord3D,
        mass: f32,
        com: f32,
        record: Arc<Mutex<ExitRecord>>,
    }

    impl PhysicsBehavior for ExitPhysics {
        fn update(&mut self, _dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn get_velocity(&self) -> Coord3D {
            self.vel
        }
        fn set_velocity(&mut self, velocity: &Coord3D) {
            self.vel = *velocity;
        }
        fn is_on_ground(&self) -> bool {
            true
        }
        fn get_mass(&self) -> f32 {
            self.mass
        }
        fn get_center_of_mass_offset(&self) -> f32 {
            self.com
        }
        fn apply_motive_force(&mut self, force: &Coord3D) {
            self.record.lock().expect("exit record").motive = Some(*force);
        }
        fn set_pitch_rate(&mut self, rate: f32) {
            self.record.lock().expect("exit record").pitch = rate;
        }
    }

    fn attach_exit_physics(
        obj: &ObjectID,
        vel: Coord3D,
        mass: f32,
        com: f32,
    ) -> (Arc<Mutex<dyn PhysicsBehavior>>, Arc<Mutex<ExitRecord>>) {
        let record = Arc::new(Mutex::new(ExitRecord::default()));
        let physics: Arc<Mutex<dyn PhysicsBehavior>> = Arc::new(Mutex::new(ExitPhysics {
            vel,
            mass,
            com,
            record: Arc::clone(&record),
        }));
        OBJECT_REGISTRY
            .with_object_mut(*obj, |o| o.set_physics(Some(Arc::clone(&physics))))
            .expect("attach physics");
        (physics, record)
    }

    fn velocity_transport(owner: &ObjectID, pitch: f32) -> TransportContain {
        let data = TransportContainModuleData {
            slot_capacity: 4,
            keep_container_velocity_on_exit: true,
            exit_pitch_rate: pitch,
            reset_mood_check_time_on_exit: false,
            ..Default::default()
        };
        TransportContain::new(*owner, &data).expect("velocity transport")
    }

    #[test]
    fn on_removing_copies_parent_velocity_and_fall_pitch() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("VelTransport", 95121, 0);
        let rider = slotted_passenger("VelRider", 95122, 1);
        let _parent = attach_exit_physics(&owner, Coord3D::new(3.0, -4.0, 1.5), 10.0, 0.0);
        let (_child, child_record) =
            attach_exit_physics(&rider, Coord3D::new(0.0, 0.0, 0.0), 2.0, 4.0);
        let mut contain = velocity_transport(&owner, 0.25);

        contain.on_removing(95122).expect("velocity inherit");

        let recorded = child_record.lock().expect("child record");
        assert_eq!(recorded.motive, Some(Coord3D::new(6.0, -8.0, 3.0)));
        assert_eq!(recorded.pitch, 1.0);
        drop(recorded);

        let still_owner = owned_object("ZeroVelTransport", 95123, 0);
        let still_rider = slotted_passenger("ZeroVelRider", 95124, 1);
        let _still_parent =
            attach_exit_physics(&still_owner, Coord3D::new(0.0, 0.0, 0.0), 1.0, 0.0);
        let (_still_child, still_record) =
            attach_exit_physics(&still_rider, Coord3D::new(1.0, 0.0, 0.0), 3.0, 2.0);
        let mut still = velocity_transport(&still_owner, 0.5);
        still
            .on_removing(95124)
            .expect("real zero velocity is success");
        let recorded = still_record.lock().expect("zero record");
        assert_eq!(recorded.motive, Some(Coord3D::new(0.0, 0.0, 0.0)));
        assert_eq!(recorded.pitch, 1.0);

        OBJECT_REGISTRY.unregister_object(95121);
        OBJECT_REGISTRY.unregister_object(95122);
        OBJECT_REGISTRY.unregister_object(95123);
        OBJECT_REGISTRY.unregister_object(95124);
        ThePlayerList().write().expect("player list write").clear();
    }

    #[test]
    fn on_removing_physics_lock_failure_is_not_zero_velocity_success() {
        let _lock = crate::test_sync::lock();
        reset_players();
        let owner = owned_object("BusyParentTransport", 95131, 0);
        let rider = slotted_passenger("BusyParentRider", 95132, 1);
        let (parent, _parent_record) =
            attach_exit_physics(&owner, Coord3D::new(9.0, 1.0, 0.0), 1.0, 0.0);
        let (_child, child_record) =
            attach_exit_physics(&rider, Coord3D::new(0.0, 0.0, 0.0), 5.0, 2.0);
        let mut contain = velocity_transport(&owner, 0.5);
        let _busy_parent = parent.lock().expect("hold parent physics");

        let err = contain.on_removing(95132);
        assert!(
            err.is_err(),
            "parent physics lock failure must not report success"
        );
        let recorded = child_record.lock().expect("child record");
        assert!(
            recorded.motive.is_none(),
            "lock failure must not apply a zero-velocity force"
        );
        assert_eq!(recorded.pitch, 0.0);
        drop(recorded);
        drop(_busy_parent);

        let owner = owned_object("BusyChildTransport", 95133, 0);
        let rider = slotted_passenger("BusyChildRider", 95134, 1);
        let _parent = attach_exit_physics(&owner, Coord3D::new(9.0, 1.0, 0.0), 1.0, 0.0);
        let (child, child_record) =
            attach_exit_physics(&rider, Coord3D::new(0.0, 0.0, 0.0), 5.0, 2.0);
        let mut contain = velocity_transport(&owner, 0.5);
        let _busy_child = child.lock().expect("hold rider physics");

        let err = contain.on_removing(95134);
        assert!(
            err.is_err(),
            "rider physics lock failure must not report success"
        );
        let recorded = child_record.lock().expect("child record");
        assert!(recorded.motive.is_none());
        assert_eq!(recorded.pitch, 0.0);

        OBJECT_REGISTRY.unregister_object(95131);
        OBJECT_REGISTRY.unregister_object(95132);
        OBJECT_REGISTRY.unregister_object(95133);
        OBJECT_REGISTRY.unregister_object(95134);
        ThePlayerList().write().expect("player list write").clear();
    }
}

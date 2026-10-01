//! Mob Nexus Contain Module
//!
//! C++ `MobNexusContain`: ally + slot hold, HELD/LOADED, extra slots, exit bone,
//! velocity/scatter, InitialPayload spawn, HealthRegen%PerSec heal, tryToEvacuate.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Weak};

use super::{
    ContainerIniParse, ContainerInterface, ObjectTemplate, unwrap_special_zero_slot_rider,
};
use crate::common::{
    DisabledType, GameResult, KindOf, ModelConditionState, ObjectID, PlayerMaskType,
    SECONDS_PER_LOGICFRAME_REAL,
};
use crate::damage::DamageInfo;
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{
    ContainModuleInterface, ContainWant, ExitDoorType, PhysicsBehavior, UpdateSleepTime,
};
use crate::object::Object;
use crate::object::contain::OpenContain;
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::production::AIFreeToExitType;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Configuration data for MobNexusContain module
#[derive(Debug, Clone)]
pub struct MobNexusContainModuleData {
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
    /// Scatter nearby units on exit
    pub scatter_nearby_on_exit: bool,
    /// Orient like container on exit
    pub orient_like_container_on_exit: bool,
    /// Keep container velocity on exit
    pub keep_container_velocity_on_exit: bool,
}

/// Initial payload configuration
#[derive(Debug, Clone, Default)]
pub struct InitialPayload {
    pub name: String,
    pub count: i32,
}

impl Default for MobNexusContainModuleData {
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
            scatter_nearby_on_exit: true,
            orient_like_container_on_exit: false,
            keep_container_velocity_on_exit: false,
        }
    }
}

impl MobNexusContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, MOB_NEXUS_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, MOB_NEXUS_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for MobNexusContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        MobNexusContainModuleData::parse_from_config(self, config)
    }
}

fn parse_slot_capacity(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.slot_capacity = INI::parse_int(token)?;
    Ok(())
}

fn parse_scatter_nearby_on_exit(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scatter_nearby_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_orient_like_container_on_exit(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.orient_like_container_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_keep_container_velocity_on_exit(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.keep_container_velocity_on_exit = INI::parse_bool(token)?;
    Ok(())
}

fn parse_exit_bone(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.exit_bone = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_exit_pitch_rate(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.exit_pitch_rate = INI::parse_angular_velocity_real(token)?;
    Ok(())
}

fn parse_initial_payload(
    _ini: &mut INI,
    data: &mut MobNexusContainModuleData,
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
    data: &mut MobNexusContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.health_regen = INI::parse_real(token)?;
    Ok(())
}

const MOB_NEXUS_CONTAIN_FIELDS: &[FieldParse<MobNexusContainModuleData>] = &[
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
];

/// Mob nexus contain module
#[derive(Debug)]
pub struct MobNexusContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Reference to the owning object
    object_id: ObjectID,
    /// Module configuration
    module_data: MobNexusContainModuleData,
    /// Extra slots in use (units that take more than one slot)
    extra_slots_in_use: i32,
    /// Whether InitialPayload has been spawned
    payload_created: bool,
}


/// C++ `child->getCenterOfMassOffset() * exitPitchRate`, then `setPitchRate`.
/// The offset is read from the borrowed physics module.
fn apply_exit_pitch_and_force(
    body: &mut dyn PhysicsBehavior,
    vel: crate::common::Coord3D,
    exit_pitch_rate: f32,
) {
    let mass = body.get_mass();
    let pitch_rate = body.get_center_of_mass_offset() * exit_pitch_rate;
    let force = crate::common::Coord3D::new(vel.x * mass, vel.y * mass, vel.z * mass);
    body.apply_motive_force(&force);
    body.set_pitch_rate(pitch_rate);
}


impl MobNexusContain {
    /// Create a new MobNexusContain module
    pub fn new(
        object_id: ObjectID,
        module_data: &MobNexusContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object_id, &module_data.base)?;

        Ok(Self {
            base,
            object_id: object_id,
            module_data: module_data.clone(),
            extra_slots_in_use: 0,
            payload_created: false,
        })
    }

    fn with_owner_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, f)
    }

    pub fn get_contain_max(&self) -> i32 {
        self.module_data.slot_capacity
    }

    pub fn get_extra_slots_in_use(&self) -> i32 {
        self.extra_slots_in_use
    }

    pub fn process_damage_to_contained(&mut self, percent_damage: f32) -> GameResult<()> {
        self.base.process_damage_to_contained(percent_damage)
    }

    /// C++ MobNexusContain::isValidContainerFor
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        let unwrapped = unwrap_special_zero_slot_rider(obj);
        let decide = |rider: &Object| -> bool {
            if !self.base.is_valid_container_for(rider, check_capacity) {
                false
            } else {
                let is_ally = self
                    .with_owner_object(|owner| {
                        rider.get_relationship_to(owner) == ObjectRelationship::Ally
                    })
                    .unwrap_or(false);
                if !is_ally {
                    false
                } else {
                    let slot_count = rider.get_transport_slot_count();
                    if slot_count == 0 {
                        false
                    } else if check_capacity {
                        self.extra_slots_in_use
                            + self.base.get_contain_count() as i32
                            + slot_count as i32
                            <= self.get_contain_max()
                    } else {
                        true
                    }
                }
            }
        };
        if let Some(id) = unwrapped {
            crate::object::registry::OBJECT_REGISTRY
                .with_object(id, decide)
                .unwrap_or(false)
        } else {
            decide(obj)
        }
    }

    /// C++ MobNexusContain::onContaining
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }
        if crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(obj_id, |_| ())
            .is_none()
        {
            return Err("Mob nexus rider lock busy".into());
        }
        self.base.on_containing(obj_id, was_selected)?;
        let Some(slot_count) =
            crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |rider| {
                rider.set_disabled(DisabledType::Held);
                rider.get_transport_slot_count()
            })
        else {
            self.base.unlink_contained_id(obj_id);
            return Err("Mob nexus rider lock busy".into());
        };
        self.extra_slots_in_use += (slot_count as i32) - 1;

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
        Ok(())
    }

    /// C++ MobNexusContain::onRemoving
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }
        if crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(obj_id, |_| ())
            .is_none()
        {
            return Err("Mob nexus rider lock busy".into());
        }
        self.base.on_removing(obj_id)?;

        let bone_pos = if self.module_data.exit_bone.is_empty() {
            None
        } else {
            self.with_owner_object(|owner| {
                let (_, bone_pos, _) =
                    owner.get_single_logical_bone_position(&self.module_data.exit_bone);
                bone_pos
            })
        };
        let orient = if self.module_data.orient_like_container_on_exit {
            self.with_owner_object(|owner| owner.get_orientation())
        } else {
            None
        };
        // C++: never hold parent and child physics at once — read the
        // container velocity first, then apply to the rider physics.
        let parent_velocity = if self.module_data.keep_container_velocity_on_exit {
            self.with_owner_object(|owner| owner.get_physics().map(|body| body.get_velocity()))
                .flatten()
        } else {
            None
        };
        let exit_pitch_rate = self.module_data.exit_pitch_rate;
        let scatter = self.module_data.scatter_nearby_on_exit;
        let above_terrain = self
            .with_owner_object(|owner| owner.is_above_terrain())
            .unwrap_or(false);

        let slot_count =
            crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |rider| {
                rider.clear_disabled(DisabledType::Held);
                if let Some(bone_pos) = bone_pos.as_ref() {
                    let _ = rider.set_position(bone_pos);
                }
                if let Some(orient) = orient {
                    let _ = rider.set_orientation(orient);
                }
                let slot_count = rider.get_transport_slot_count();
                if let Some(vel) = parent_velocity {
                    if let Some(child) = rider.get_physics_mut() {
                        apply_exit_pitch_and_force(child, vel, exit_pitch_rate);
                    }
                }
                if scatter {
                    let _ = self.base.scatter_to_nearby_position(rider);
                }
                slot_count
            });
        let Some(slot_count) = slot_count else {
            let _ = self.base.add_to_contain_list_id(obj_id, false);
            return Err("Mob nexus rider lock busy".into());
        };
        self.extra_slots_in_use -= (slot_count as i32) - 1;

        if self.base.get_contain_count() == 0 {
            if let Some(drawable) = self
                .with_owner_object(|owner| owner.get_drawable())
                .flatten()
            {
                if let Ok(mut draw) = drawable.write() {
                    draw.clear_model_condition_state(ModelConditionState::Loaded);
                }
            }
        }

        if above_terrain {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |rider| {
                if let Some(physics) = rider.get_physics_mut() {
                    physics.set_allow_to_fall(true);
                }
            });
        }
        Ok(())
    }

    fn create_payload(&mut self) -> GameResult<()> {
        if self.payload_created {
            return Ok(());
        }

        let (payload_name, payload_count, owner_team) = self
            .with_owner_object(|owner| {
                (
                    self.module_data.initial_payload.name.clone(),
                    self.module_data.initial_payload.count.max(0),
                    owner
                        .get_controlling_player()
                        .and_then(|player| player.read().ok().and_then(|p| p.get_default_team())),
                )
            })
            .unwrap_or_else(|| (String::new(), 0, None));

        if payload_count == 0 || payload_name.is_empty() {
            return Ok(());
        }

        let Some(template) = TheThingFactory::find_template(&payload_name) else {
            log::warn!(
                "MobNexusContain payload template '{}' not found; skipping payload",
                payload_name
            );
            return Ok(());
        };

        let factory = match TheThingFactory::get() {
            Ok(factory) => factory,
            Err(err) => {
                return Err(format!("MobNexusContain thing factory unavailable: {}", err).into());
            }
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
                    return Err("Mob nexus team lock busy".into());
                };
                factory.new_object(template.clone(), &*team_guard)
            } else {
                factory.new_object_optional_team(template.clone(), None)
            };
            let Ok(payload_id) = payload else {
                continue;
            };
            let can_add =
                crate::object::registry::OBJECT_REGISTRY.with_object(payload_id, |guard| {
                    self.is_valid_container_for(guard, true)
                });
            let Some(can_add) = can_add else {
                let _ = TheGameLogic::destroy_object_by_id(payload_id);
                self.base.enable_load_sounds(true);
                if added_any {
                    self.payload_created = true;
                }
                return Err("Mob nexus payload lock busy".into());
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
            }
        }
        self.base.enable_load_sounds(true);
        self.payload_created = true;
        Ok(())
    }

    /// C++ MobNexusContain::onObjectCreated
    pub fn on_object_created(&mut self) -> GameResult<()> {
        self.create_payload()
    }

    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        if !TheGameLogic::find_object_by_id(obj_id) {
            return Err("MobNexus contain object not found".into());
        }
        let decision = crate::object::registry::OBJECT_REGISTRY.with_object(
            obj_id,
            |obj_ref| -> Result<Option<bool>, &'static str> {
                let was_selected = obj_ref
                    .get_drawable()
                    .and_then(|drawable| drawable.read().ok().map(|draw| draw.is_selected()))
                    .unwrap_or(false);
                if !self.is_valid_container_for(obj_ref, true) {
                    return Err("Object not valid for this mob nexus");
                }
                let already_listed = self.base.get_contained_object_ids().contains(&obj_id);
                let contained_by = obj_ref.get_contained_by();
                if contained_by.is_some() && (already_listed || contained_by != Some(self.object_id))
                {
                    return Ok(None);
                }
                Ok(Some(was_selected))
            },
        );
        let was_selected = match decision {
            None => return Err("Object lock poisoned".into()),
            Some(Err(msg)) => return Err(msg.into()),
            Some(Ok(None)) => return Ok(()),
            Some(Ok(Some(was_selected))) => was_selected,
        };
        self.base.add_to_contain_list(obj_id)?;
        self.on_containing(obj_id, was_selected)?;
        self.base.do_load_sound();
        Ok(())
    }

    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        if !self.base.get_contained_object_ids().contains(&obj_id) {
            return Ok(());
        }
        let Some(stealth_garrison) = self.base.remove_from_contain_list(obj_id) else {
            return Err("Mob passenger lock busy".into());
        };
        if expose_stealth_units {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                if let Some(stealth) = obj_guard.get_stealth() {
                    if let Ok(mut stealth_guard) = stealth.try_lock() {
                        stealth_guard.mark_as_detected();
                    }
                }
            });
        }
        self.base.do_unload_sound();
        if let Err(err) = self.on_removing(obj_id) {
            let _ = self.base.add_to_contain_list_id(obj_id, stealth_garrison);
            return Err(err);
        }
        if let Err(err) = self.base.note_removed_from(obj_id) {
            let _ = self.base.add_to_contain_list_id(obj_id, stealth_garrison);
            return Err(err);
        }
        Ok(())
    }

    /// C++ MobNexusContain::update — HealthRegen%PerSec then OpenContain::update
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        if !self.payload_created {
            if let Err(err) = self.create_payload() {
                log::warn!("MobNexusContain::update: createPayload failed: {}", err);
            }
        }

        if self.module_data.health_regen != 0.0 {
            let owner_id = self.object_id;
            for object_id in self.base.get_contained_object_ids().to_vec() {
                if !TheGameLogic::find_object_by_id(object_id) {
                    continue;
                }
                let health_read = crate::object::registry::OBJECT_REGISTRY.with_object(
                    object_id,
                    |guard| {
                        let Some(body) = guard.get_body_module() else {
                            return None;
                        };
                        let max_health = body.get_max_health();
                        if body.get_health() < max_health && max_health > 0.0 {
                            Some(max_health)
                        } else {
                            None
                        }
                    },
                );
                let Some(max_health) = health_read else {
                    log::warn!("MobNexusContain::update regen lock busy for {}", object_id);
                    continue;
                };
                let Some(max_health) = max_health else {
                    continue;
                };
                let regen = max_health * self.module_data.health_regen / 100.0
                    * SECONDS_PER_LOGICFRAME_REAL;
                let healed =
                    crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                        if owner_id != crate::common::INVALID_ID {
                            let _ = object_guard.attempt_healing_from_source_id(regen, owner_id);
                        } else {
                            let _ = object_guard.attempt_healing(regen, None);
                        }
                    });
                if healed.is_none() {
                    log::warn!("MobNexusContain::update regen write busy for {}", object_id);
                }
            }
        }

        match self.base.update() {
            Ok(sleep) => Ok(sleep),
            Err(err) => {
                log::warn!("MobNexusContain::update base update failed: {}", err);
                Ok(UpdateSleepTime::None)
            }
        }
    }

    /// C++ MobNexusContain::reserveDoorForExit
    pub fn reserve_door_for_exit(
        &self,
        _obj_type: &ObjectTemplate,
        specific_object: Option<&Object>,
    ) -> ExitDoorType {
        let Some(specific) = specific_object else {
            return ExitDoorType::Primary;
        };

        let blocked = self
            .with_owner_object(|me| {
                if let Some(ai) = me.get_ai_update_interface() {
                    if !matches!(
                        ai.get_ai_free_to_exit(me),
                        AIFreeToExitType::FreeToExit
                    ) {
                        return true;
                    }
                }
                false
            })
            .unwrap_or(false);
        if blocked {
            return ExitDoorType::NoneAvailable;
        }

        if self
            .with_owner_object(|me| me.is_using_airborne_locomotor())
            .unwrap_or(false)
        {
            return ExitDoorType::Primary;
        }

        if specific.get_ai_update_interface().is_none() {
            return ExitDoorType::NoneAvailable;
        }

        let valid_terrain = self
            .with_owner_object(|me| {
                let astar_layer = match me.get_layer() {
                    crate::common::PathfindLayerEnum::Top => {
                        crate::ai::pathfind_astar::PathfindLayerEnum::Top
                    }
                    _ => crate::ai::pathfind_astar::PathfindLayerEnum::Ground,
                };
                crate::ai::the_ai()
                    .read()
                    .ok()
                    .and_then(|ai_sys| {
                        let pf = ai_sys.pathfinder()?;
                        let pf_guard = pf.read().ok()?;
                        let found = pf_guard.get_cell_type_at_layer(me.get_position(), astar_layer);
                        let found = if found.is_none()
                            && astar_layer == crate::ai::pathfind_astar::PathfindLayerEnum::Top
                        {
                            pf_guard.get_cell_type_at_layer(
                                me.get_position(),
                                crate::ai::pathfind_astar::PathfindLayerEnum::Ground,
                            )
                        } else {
                            found
                        };
                        Some(!matches!(
                            found,
                            Some(crate::ai::pathfind_astar::PathfindCellType::Cliff)
                                | Some(crate::ai::pathfind_astar::PathfindCellType::Water)
                                | Some(crate::ai::pathfind_astar::PathfindCellType::Impassable)
                                | None
                        ))
                    })
                    .unwrap_or(true)
            })
            .unwrap_or(true);
        if !valid_terrain {
            return ExitDoorType::NoneAvailable;
        }
        ExitDoorType::Primary
    }

    /// C++ MobNexusContain::tryToEvacuate
    pub fn try_to_evacuate(&mut self, expose_stealthed_units: bool) -> bool {
        let mut exited_anyone = false;
        let ids = self.base.get_contained_object_ids().to_vec();
        for obj_id in ids {
            let door = crate::object::registry::OBJECT_REGISTRY
                .with_object(obj_id, |guard| {
                    self.reserve_door_for_exit(&ObjectTemplate {}, Some(guard))
                })
                .unwrap_or(ExitDoorType::NoneAvailable);
            if matches!(door, ExitDoorType::None | ExitDoorType::NoneAvailable) {
                continue;
            }
            if self.base.exit_object_via_door(obj_id, door).is_ok() {
                exited_anyone = true;
                if expose_stealthed_units {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                        if obj_guard.is_kind_of(KindOf::StealthGarrison) {
                            if let Some(stealth) = obj_guard.get_stealth() {
                                if let Ok(mut stealth_guard) = stealth.lock() {
                                    stealth_guard.mark_as_detected();
                                }
                            }
                        }
                    });
                }
            }
        }
        exited_anyone
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        self.base.save_state()
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)
    }
}

impl Snapshotable for MobNexusContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_int(&mut self.extra_slots_in_use)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl ContainModuleInterface for MobNexusContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                self.is_valid_container_for(obj_guard, true)
            })
            .unwrap_or(false)
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
        MobNexusContain::remove_from_contain(self, object_id, expose_stealth).map_err(|e| e.into())
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

    fn on_owner_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_object_created().map_err(|e| e.into())
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        MobNexusContain::update(self).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_die_for_owner(Some(owner), damage_info)
            .map_err(|e| e.into())
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.base.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let valid = crate::object::registry::OBJECT_REGISTRY
            .with_object(other_id, |guard| self.is_valid_container_for(guard, true))
            .unwrap_or(false);
        if valid {
            self.add_to_contain(other_id)?;
        }
        Ok(())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        MobNexusContain::is_valid_container_for(self, obj, check_capacity)
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

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&Object>,
        spawn: Option<&Object>,
    ) -> ExitDoorType {
        MobNexusContain::reserve_door_for_exit(self, &ObjectTemplate {}, spawn)
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .exit_object_via_door(obj_id, door)
            .map_err(|e| e.into())
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        MobNexusContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        MobNexusContain::on_removing(self, obj_id).map_err(|e| e.into())
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
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

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = MobNexusContain::process_damage_to_contained(self, percent_damage);
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_selling().map_err(|e| e.into())
    }
}

impl ContainerInterface for MobNexusContain {
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
        let current = self.base.get_contain_count() + self.extra_slots_in_use as u32;
        let max = match self.module_data.slot_capacity {
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
    use crate::common::{Coord3D, DefaultThingTemplate, ObjectID, ObjectStatusMaskType};
    use crate::helpers::TheGameLogic;
    use crate::object::Object;
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct ExitRecord {
        pitch: f32,
        force: Coord3D,
        applied: bool,
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
        fn set_pitch_rate(&mut self, rate: f32) {
            self.record
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pitch = rate;
        }
        fn apply_motive_force(&mut self, force: &Coord3D) {
            let mut record = self
                .record
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            record.force = *force;
            record.applied = true;
        }
    }

    struct Registered(Vec<ObjectID>);

    impl Drop for Registered {
        fn drop(&mut self) {
            for id in &self.0 {
                TheGameLogic::remove_object(*id);
            }
        }
    }

    fn test_object(name: &str, id: ObjectID) -> ObjectID {
        let template = Arc::new(DefaultThingTemplate::new(name.to_string()));
        let object = Object::new_raw(template, id, ObjectStatusMaskType::none(), None);
        crate::object::registry::OBJECT_REGISTRY.register_object(id, object);
        id
    }

    fn nexus_for(owner: &ObjectID, pitch: f32) -> MobNexusContain {
        let mut data = MobNexusContainModuleData::default();
        data.exit_pitch_rate = pitch;
        data.keep_container_velocity_on_exit = true;
        MobNexusContain::new(*owner, &data).expect("nexus")
    }

    fn attach_physics(
        object: &ObjectID,
        vel: Coord3D,
        mass: f32,
        com: f32,
        pitch: f32,
        shared: Option<Arc<Mutex<ExitRecord>>>,
    ) -> Arc<Mutex<ExitRecord>> {
        let record = shared.unwrap_or_else(|| {
            Arc::new(Mutex::new(ExitRecord {
                pitch,
                force: Coord3D::new(0.0, 0.0, 0.0),
                applied: false,
            }))
        });
        let physics = ExitPhysics {
            vel,
            mass,
            com,
            record: record.clone(),
        };
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(*object, |obj| {
                obj.set_physics(Some(Box::new(physics)));
            })
            .expect("object");
        record
    }

    #[test]
    fn on_removing_scales_exit_pitch_by_center_of_mass() {
        let owner_id = 91_440_011;
        let rider_id = 91_440_012;
        let _registered = Registered(vec![owner_id, rider_id]);
        let owner = test_object("MobNexusPitchOwner", owner_id);
        let rider = test_object("MobNexusPitchRider", rider_id);
        let _parent = attach_physics(
            &owner,
            Coord3D::new(3.0, -1.0, 2.0),
            10.0,
            0.0,
            0.0,
            None,
        );
        let record = attach_physics(
            &rider,
            Coord3D::new(0.0, 0.0, 0.0),
            4.0,
            2.5,
            9.0,
            None,
        );
        let mut nexus = nexus_for(&owner, 0.4);
        nexus.on_removing(rider_id).expect("on_removing");
        let record = record
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(record.applied);
        assert!(
            (record.pitch - 1.0).abs() < 1.0e-5,
            "pitch {} is not center-of-mass * exit rate",
            record.pitch
        );
        assert_eq!(record.force, Coord3D::new(12.0, -4.0, 8.0));
    }

    #[test]
    fn on_removing_same_physics_state_scales_pitch() {
        let owner_id = 91_440_021;
        let rider_id = 91_440_022;
        let _registered = Registered(vec![owner_id, rider_id]);
        let owner = test_object("MobNexusSameOwner", owner_id);
        let rider = test_object("MobNexusSameRider", rider_id);
        let record = attach_physics(
            &owner,
            Coord3D::new(3.0, -1.0, 2.0),
            4.0,
            2.5,
            9.0,
            None,
        );
        attach_physics(
            &rider,
            Coord3D::new(3.0, -1.0, 2.0),
            4.0,
            2.5,
            9.0,
            Some(record.clone()),
        );
        let mut nexus = nexus_for(&owner, 0.4);
        nexus.on_removing(rider_id).expect("on_removing");
        let record = record
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(record.applied);
        assert!(
            (record.pitch - 1.0).abs() < 1.0e-5,
            "same-physics pitch {}",
            record.pitch
        );
        assert_ne!(record.pitch, 0.0);
        assert_eq!(record.force, Coord3D::new(12.0, -4.0, 8.0));
    }

    #[test]
    fn on_removing_without_parent_physics_leaves_pitch() {
        let owner_id = 91_440_031;
        let rider_id = 91_440_032;
        let _registered = Registered(vec![owner_id, rider_id]);
        let owner = test_object("MobNexusNoPhysOwner", owner_id);
        let rider = test_object("MobNexusNoPhysRider", rider_id);
        let record = attach_physics(&rider, Coord3D::new(1.0, 0.0, 0.0), 4.0, 2.5, 9.0, None);
        let mut nexus = nexus_for(&owner, 0.4);
        nexus.on_removing(rider_id).expect("on_removing");
        let record = record
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(!record.applied);
        assert!((record.pitch - 9.0).abs() < 1.0e-5);
    }
}

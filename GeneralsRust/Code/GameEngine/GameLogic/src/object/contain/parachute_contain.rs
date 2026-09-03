//! Parachute Contain Module
//!
//! C++ `ParachuteContain`: open after `paraOpenDist`, landing AI, sway, empty-chute
//! kill, water/impassable rider death, airborne free-fall damage, ground dump.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::ai::the_ai;
use crate::ai::pathfind_astar::PathfindCellType;
use crate::common::audio::AudioEventRts;
use crate::common::{
    CommandSourceType, Coord3D, DisabledType, GameResult, KindOf, LocomotorSetType, Matrix3D,
    ModelConditionFlags, ObjectID, ObjectStatusMaskType, PathfindLayerEnum, PlayerMaskType,
};
use crate::damage::{DamageInfo, DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::helpers::{
    TheAudio, TheGameLogic, ThePartitionManager, TheTerrainLogic, get_game_logic_random_value_real,
};
use crate::modules::{
    AIUpdateInterfaceExt, ContainModuleInterface, ContainWant, PhysicsBehaviorExt, UpdateSleepTime,
};
use crate::object::Object;
use crate::object::contain::OpenContain;
use crate::object::drawable::DrawableArcExt;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

const NO_START_Z: f32 = 1e10;
const ALTITUDE_DAMP_START: f32 = 20.0;

/// Configuration data for ParachuteContain module
#[derive(Debug, Clone)]
pub struct ParachuteContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Max pitch rate
    pub pitch_rate_max: f32,
    /// Max roll rate
    pub roll_rate_max: f32,
    /// Low altitude damping
    pub low_altitude_damping: f32,
    /// Deploy the parachute when we have traveled this far
    pub para_open_dist: f32,
    /// Free fall damage percent
    pub free_fall_damage_percent: f32,
    /// Kill when landing in water slop threshold
    pub kill_when_landing_in_water_slop: f32,
    /// Parachute open sound
    pub parachute_open_sound: Option<AudioEventRts>,
}

impl Default for ParachuteContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            pitch_rate_max: 0.0,
            roll_rate_max: 0.0,
            low_altitude_damping: 0.2,
            para_open_dist: 0.0,
            free_fall_damage_percent: 0.5,
            kill_when_landing_in_water_slop: 10.0,
            parachute_open_sound: None,
        }
    }
}

impl ParachuteContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, PARACHUTE_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, PARACHUTE_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for ParachuteContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        ParachuteContainModuleData::parse_from_config(self, config)
    }
}

fn parse_pitch_rate_max(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.pitch_rate_max = INI::parse_angular_velocity_real(token)?;
    Ok(())
}

fn parse_roll_rate_max(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.roll_rate_max = INI::parse_angular_velocity_real(token)?;
    Ok(())
}

fn parse_low_altitude_damping(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.low_altitude_damping = INI::parse_real(token)?;
    Ok(())
}

fn parse_parachute_open_dist(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.para_open_dist = INI::parse_real(token)?;
    Ok(())
}

fn parse_kill_when_landing_in_water_slop(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.kill_when_landing_in_water_slop = INI::parse_real(token)?;
    Ok(())
}

fn parse_free_fall_damage_percent(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.free_fall_damage_percent = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_parachute_open_sound(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.parachute_open_sound = None;
    } else {
        data.parachute_open_sound = Some(AudioEventRts::new(*token));
    }
    Ok(())
}

const PARACHUTE_CONTAIN_FIELDS: &[FieldParse<ParachuteContainModuleData>] = &[
    FieldParse {
        token: "PitchRateMax",
        parse: parse_pitch_rate_max,
    },
    FieldParse {
        token: "RollRateMax",
        parse: parse_roll_rate_max,
    },
    FieldParse {
        token: "LowAltitudeDamping",
        parse: parse_low_altitude_damping,
    },
    FieldParse {
        token: "ParachuteOpenDist",
        parse: parse_parachute_open_dist,
    },
    FieldParse {
        token: "KillWhenLandingInWaterSlop",
        parse: parse_kill_when_landing_in_water_slop,
    },
    FieldParse {
        token: "FreeFallDamagePercent",
        parse: parse_free_fall_damage_percent,
    },
    FieldParse {
        token: "ParachuteOpenSound",
        parse: parse_parachute_open_sound,
    },
];

/// Parachute contain module - for airborne deployment
#[derive(Debug)]
pub struct ParachuteContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    object_id: ObjectID,
    module_data: ParachuteContainModuleData,
    pitch: f32,
    roll: f32,
    pitch_rate: f32,
    roll_rate: f32,
    start_z: f32,
    is_landing_override_set: bool,
    landing_override: Coord3D,
    rider_attach_bone: Coord3D,
    rider_sway_bone: Coord3D,
    para_attach_bone: Coord3D,
    para_sway_bone: Coord3D,
    rider_attach_offset: Coord3D,
    rider_sway_offset: Coord3D,
    para_attach_offset: Coord3D,
    para_sway_offset: Coord3D,
    need_to_update_rider_bones: bool,
    need_to_update_para_bones: bool,
    opened: bool,
}

impl ParachuteContain {
    /// Create a new ParachuteContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &ParachuteContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;
        let object_id = object
            .upgrade()
            .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
            .unwrap_or(crate::common::INVALID_ID);

        if object_id != crate::common::INVALID_ID {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(object_id, |owner| {
                owner.set_status(ObjectStatusMaskType::PARACHUTING, true);
            });
        }

        Ok(Self {
            base,
            object_id,
            module_data: module_data.clone(),
            pitch: 0.0,
            roll: 0.0,
            pitch_rate: get_game_logic_random_value_real(
                -module_data.pitch_rate_max,
                module_data.pitch_rate_max,
            ),
            roll_rate: get_game_logic_random_value_real(
                -module_data.roll_rate_max,
                module_data.roll_rate_max,
            ),
            start_z: NO_START_Z,
            is_landing_override_set: false,
            landing_override: Coord3D::new(0.0, 0.0, 0.0),
            rider_attach_bone: Coord3D::new(0.0, 0.0, 0.0),
            rider_sway_bone: Coord3D::new(0.0, 0.0, 0.0),
            para_attach_bone: Coord3D::new(0.0, 0.0, 0.0),
            para_sway_bone: Coord3D::new(0.0, 0.0, 0.0),
            rider_attach_offset: Coord3D::new(0.0, 0.0, 0.0),
            rider_sway_offset: Coord3D::new(0.0, 0.0, 0.0),
            para_attach_offset: Coord3D::new(0.0, 0.0, 0.0),
            para_sway_offset: Coord3D::new(0.0, 0.0, 0.0),
            need_to_update_rider_bones: true,
            need_to_update_para_bones: true,
            opened: false,
        })
    }

    fn with_owner<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, f)
    }

    fn with_owner_mut<R>(&self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, f)
    }

    fn first_rider_id(&self) -> Option<ObjectID> {
        self.base.get_contained_object_ids().first().copied()
    }

    fn resolve_rider(&self) -> Option<Arc<RwLock<Object>>> {
        let id = self.first_rider_id()?;
        TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    pub fn process_damage_to_contained(&mut self, percent_damage: f32) -> GameResult<()> {
        self.base.process_damage_to_contained(percent_damage)
    }

    /// C++ ParachuteContain::isValidContainerFor
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        if !self.base.is_valid_container_for(obj, check_capacity) {
            return false;
        }
        let transport_slot_count = obj.get_transport_slot_count();
        if transport_slot_count == 0
            && !obj.is_kind_of(KindOf::Infantry)
            && !obj.is_kind_of(KindOf::Parachutable)
        {
            return false;
        }
        if self.base.get_contain_count() > 0 {
            return false;
        }
        true
    }

    fn hide_until_open(&self) {
        if let Some(drawable) = self.with_owner(|owner| owner.get_drawable()).flatten() {
            drawable.set_drawable_hidden(!self.opened);
        }
    }

    fn calc_sway_mtx(&self, offset: &Coord3D) -> Matrix3D {
        // C++: Translate(offset) * RotX(roll) * RotY(pitch) * Translate(-offset)
        let t = glam::Vec3::new(offset.x, offset.y, offset.z);
        Matrix3D::from_translation(t)
            * Matrix3D::from_rotation_x(self.roll)
            * Matrix3D::from_rotation_y(self.pitch)
            * Matrix3D::from_translation(-t)
    }

    fn update_bone_positions(&mut self, rider: Option<&Object>) {
        if self.need_to_update_para_bones {
            self.need_to_update_para_bones = false;
            if let Some(drawable) = self.with_owner(|owner| owner.get_drawable()).flatten() {
                if let Ok(draw) = drawable.read() {
                    if let Some(pos) = draw.get_pristine_bone_positions("PARA_COG", 0, 1).first() {
                        self.para_sway_bone = *pos;
                    } else {
                        self.para_sway_bone = Coord3D::new(0.0, 0.0, 0.0);
                    }
                    if let Some(pos) = draw.get_pristine_bone_positions("PARA_ATTCH", 0, 1).first()
                    {
                        self.para_attach_bone = *pos;
                    } else {
                        self.para_attach_bone = Coord3D::new(0.0, 0.0, 0.0);
                    }
                }
            }
        }

        if self.need_to_update_rider_bones {
            // Callers holding the rider's write lock (position_rider) pass the
            // rider reference directly; re-locking the same RwLock here would
            // self-deadlock. Lock-free callers resolve the rider first and pass
            // the guard as `rider`.
            if let Some(rider_guard) = rider {
                if let Some(drawable) = rider_guard.get_drawable() {
                    if let Ok(draw) = drawable.read() {
                        if let Some(pos) =
                            draw.get_pristine_bone_positions("PARA_MAN", 0, 1).first()
                        {
                            self.rider_attach_bone = *pos;
                        } else {
                            let height = rider_guard
                                .get_geometry_info()
                                .get_max_height_above_position();
                            self.rider_attach_bone = Coord3D::new(0.0, 0.0, height);
                        }
                    }
                }
            }
        }
    }

    fn update_offsets_from_bones(&mut self, rider: Option<&Object>) {
        let Some(obj_pos) = self.with_owner(|owner| *owner.get_position()) else {
            return;
        };

        if let Some(world) = self.with_owner(|owner| {
            let mtx = owner.convert_bone_pos_to_world_pos(Some(&self.para_sway_bone), None);
            let (_, _, translation) = mtx.to_scale_rotation_translation();
            translation
        }) {
            self.para_sway_offset = Coord3D::new(
                world.x - obj_pos.x,
                world.y - obj_pos.y,
                world.z - obj_pos.z,
            );
        }
        if let Some(world) = self.with_owner(|owner| {
            let mtx = owner.convert_bone_pos_to_world_pos(Some(&self.para_attach_bone), None);
            let (_, _, translation) = mtx.to_scale_rotation_translation();
            translation
        }) {
            self.para_attach_offset = Coord3D::new(
                world.x - obj_pos.x,
                world.y - obj_pos.y,
                world.z - obj_pos.z,
            );
        }

        // Same re-entry rule as update_bone_positions: callers holding the
        // rider's write lock pass the reference; lock-free callers resolve the
        // rider first and pass the guard as `rider`.
        if let Some(rider_guard) = rider {
            let rider_pos = *rider_guard.get_position();
            let mtx =
                rider_guard.convert_bone_pos_to_world_pos(Some(&self.rider_attach_bone), None);
            let (_, _, world) = mtx.to_scale_rotation_translation();
            self.rider_attach_offset = Coord3D::new(
                world.x - rider_pos.x,
                world.y - rider_pos.y,
                world.z - rider_pos.z,
            );
            self.rider_attach_offset.x = self.para_attach_offset.x - self.rider_attach_offset.x;
            self.rider_attach_offset.y = self.para_attach_offset.y - self.rider_attach_offset.y;
            self.rider_attach_offset.z = self.para_attach_offset.z - self.rider_attach_offset.z;
            self.rider_sway_offset.x = self.para_sway_offset.x - self.rider_attach_offset.x;
            self.rider_sway_offset.y = self.para_sway_offset.y - self.rider_attach_offset.y;
            self.rider_sway_offset.z = self.para_sway_offset.z - self.rider_attach_offset.z;
        }
    }

    fn position_rider(&mut self, rider: &mut Object) {
        self.update_bone_positions(Some(rider));
        self.update_offsets_from_bones(Some(rider));

        let Some(mut pos) = self.with_owner(|owner| *owner.get_position()) else {
            return;
        };
        pos.x += self.rider_attach_offset.x;
        pos.y += self.rider_attach_offset.y;
        pos.z += self.rider_attach_offset.z;
        let _ = rider.set_position(&pos);

        let alt = rider.get_height_above_terrain();
        if alt < 0.0 {
            pos.z -= alt;
            let _ = rider.set_position(&pos);
        }

        if let Some(orient) = self.with_owner(|owner| owner.get_orientation()) {
            let _ = rider.set_orientation(orient);
        }

        if let Some(drawable) = rider.get_drawable() {
            if rider.is_disabled_by_type(DisabledType::Held) {
                let mtx = self.calc_sway_mtx(&self.rider_sway_offset);
                drawable.set_instance_matrix(Some(&mtx));
            } else {
                drawable.set_instance_matrix(None);
            }
        }
    }

    fn position_contained_objects(&mut self) {
        let ids = self.base.get_contained_object_ids().to_vec();
        for id in ids {
            if let Some(obj) = TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
            {
                if let Ok(mut rider) = obj.write() {
                    self.position_rider(&mut rider);
                }
            }
        }
    }

    /// C++ ParachuteContain::setOverrideDestination
    pub fn set_override_destination(&mut self, pos: &Coord3D) {
        self.landing_override = *pos;
        self.is_landing_override_set = true;
    }

    /// C++ ParachuteContain::onContaining
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        self.base.on_containing(obj_id, was_selected)?;
        let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };
        if let Ok(mut rider) = obj.write() {
            rider.set_disabled(DisabledType::Held);
            rider.set_status(ObjectStatusMaskType::PARACHUTING, true);
            let _ = rider.clear_and_set_model_condition_flags(
                ModelConditionFlags::PARACHUTING,
                ModelConditionFlags::FREEFALL,
            );
            self.need_to_update_rider_bones = true;
            self.position_rider(&mut rider);
        }
        Ok(())
    }

    /// C++ ParachuteContain::onRemoving
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.on_removing(obj_id)?;
        let Some(obj) = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        else {
            return Ok(());
        };

        self.with_owner_mut(|owner| {
            owner.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
        });

        if let Ok(mut rider) = obj.write() {
            rider.clear_disabled(DisabledType::Held);
            rider.set_status(ObjectStatusMaskType::PARACHUTING, false);
            self.position_rider(&mut rider);
            let _ = rider.clear_and_set_model_condition_flags(
                ModelConditionFlags::FREEFALL | ModelConditionFlags::PARACHUTING,
                ModelConditionFlags::empty(),
            );
            self.need_to_update_rider_bones = true;

            if let Some(physics) = rider.get_physics() {
                physics.set_allow_to_fall(true);
                physics.apply_force(&crate::common::Coord3D::new(0.0, 0.0, 0.0));
            }

            if let Some(ai) = rider.get_ai() {
                let is_skirmish = rider
                    .get_controlling_player()
                    .and_then(|p| p.read().ok().map(|g| g.is_skirmish_ai_player()))
                    .unwrap_or(false);
                if is_skirmish {
                    ai.ai_hunt(CommandSourceType::FromAi);
                } else {
                    let mut has_rally = false;
                    let producer_id = rider.get_producer_id();
                    if producer_id != crate::common::INVALID_ID {
                        if let Some(transport) = TheGameLogic::find_object_by_id(producer_id) {
                            if let Ok(transport_guard) = transport.read() {
                                let building_id = transport_guard.get_producer_id();
                                if building_id != crate::common::INVALID_ID {
                                    if let Some(building) =
                                        TheGameLogic::find_object_by_id(building_id)
                                    {
                                        if let Ok(building_guard) = building.read() {
                                            if let Some(exit) =
                                                building_guard.get_object_exit_interface()
                                            {
                                                if let Ok(mut exit_guard) = exit.lock() {
                                                    if exit_guard.use_spawn_rally_point() {
                                                        exit_guard.exit_object_via_door(
                                                            rider.get_id(),
                                                            crate::modules::ExitDoorType::Primary,
                                                        );
                                                        has_rally = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !has_rally {
                        ai.ai_idle(CommandSourceType::FromAi);
                    }
                }
            }

            let rider_pos = *rider.get_position();
            let mut water_z = 0.0;
            let mut terrain_z = 0.0;
            if let Some(terrain) = TheTerrainLogic::get() {
                if terrain.is_underwater(
                    rider_pos.x,
                    rider_pos.y,
                    Some(&mut water_z),
                    Some(&mut terrain_z),
                ) && rider_pos.z <= water_z + self.module_data.kill_when_landing_in_water_slop
                    && rider.get_layer() == PathfindLayerEnum::Ground
                {
                    let mut damage_info = DamageInfo::with_simple(
                        HUGE_DAMAGE_AMOUNT,
                        crate::common::INVALID_ID,
                        DamageType::Water,
                        DeathType::Flooded,
                    );
                    let _ = rider.attempt_damage(&mut damage_info);
                }
            }

            let layer = rider.get_layer();
            let ai_store = the_ai();let cell_type = ai_store.read().ok().and_then(|ai| {
                ai.pathfinder().and_then(|pf| {
                    pf.read().ok().and_then(|pf_guard| {
                        let astar_layer = match layer {
                            PathfindLayerEnum::Top => {
                                crate::ai::pathfind_astar::PathfindLayerEnum::Top
                            }
                            _ => crate::ai::pathfind_astar::PathfindLayerEnum::Ground,
                        };
                        pf_guard.get_cell_type_at_layer(rider.get_position(), astar_layer)
                    })
                })
            });
            let bad_cell = matches!(
                cell_type,
                Some(PathfindCellType::Cliff)
                    | Some(PathfindCellType::Water)
                    | Some(PathfindCellType::Impassable)
                    | None
            );
            if rider.is_off_map() || bad_cell {
                rider.kill(None, None);
            }
        }
        Ok(())
    }

    /// C++ ParachuteContain::onDie
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        let airborne = self
            .with_owner(|owner| owner.is_significantly_above_terrain())
            .unwrap_or(false);
        if airborne {
            if let Some(rider_arc) = self.resolve_rider() {
                let _ = self.base.remove_all_contained(false);
                if let Ok(mut rider) = rider_arc.write() {
                    if self.module_data.free_fall_damage_percent > 0.0 {
                        let max_health = rider
                            .get_body_module()
                            .and_then(|body| {
                                body.lock()
                                    .ok()
                                    .map(|body_guard| body_guard.get_max_health())
                            })
                            .unwrap_or(0.0);
                        let source = damage_info
                            .map(|info| info.input.source_id)
                            .unwrap_or(crate::common::INVALID_ID);
                        let mut extra = DamageInfo::with_simple(
                            max_health * self.module_data.free_fall_damage_percent,
                            source,
                            DamageType::Falling,
                            DeathType::Splatted,
                        );
                        let _ = rider.attempt_damage(&mut extra);
                    }
                    if let Some(physics) = rider.get_physics() {
                        physics.set_allow_to_fall(true);
                        physics.set_is_in_freefall(true);
                        physics.apply_force(&Coord3D::new(0.0, 0.0, 0.0));
                    }
                }
            }
        }
        self.base.on_die(damage_info)
    }

    /// C++ ParachuteContain::onCollide — other == null means ground.
    pub fn on_collide(&mut self, other: Option<ObjectID>) -> GameResult<()> {
        if other.is_some() {
            return Ok(());
        }
        let contained_by = self.with_owner(|owner| owner.get_contained_by()).flatten();
        if contained_by.is_some() {
            return Ok(());
        }
        self.base.remove_all_contained(false)?;
        self.with_owner_mut(|owner| owner.kill(None, None));
        Ok(())
    }

    /// C++ ParachuteContain::update
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        self.base.update()?;

        let held = self
            .with_owner(|owner| owner.is_disabled_by_type(DisabledType::Held))
            .unwrap_or(false);
        if held {
            return Ok(UpdateSleepTime::None);
        }

        if self.start_z == NO_START_Z {
            if let Some((pos, ground)) = self.with_owner(|owner| {
                let pos = *owner.get_position();
                let ground = TheTerrainLogic::get()
                    .map(|t| t.get_ground_height(pos.x, pos.y, None))
                    .unwrap_or(0.0);
                (pos, ground)
            }) {
                self.start_z = pos.z;
                if self.start_z - ground < 2.0 * self.module_data.para_open_dist {
                    self.start_z = ground + 2.0 * self.module_data.para_open_dist;
                }
            }
        }

        let pos_z = self
            .with_owner(|owner| owner.get_position().z)
            .unwrap_or(0.0);

        if !self.opened {
            if (self.start_z - pos_z).abs() >= self.module_data.para_open_dist {
                self.opened = true;
                self.with_owner_mut(|owner| {
                    let _ = owner.clear_and_set_model_condition_flags(
                        ModelConditionFlags::FREEFALL,
                        ModelConditionFlags::PARACHUTING,
                    );
                });
                self.need_to_update_para_bones = true;
                if let Some(rider) = self.resolve_rider() {
                    if let Ok(mut rider_guard) = rider.write() {
                        let _ = rider_guard.clear_and_set_model_condition_flags(
                            ModelConditionFlags::FREEFALL,
                            ModelConditionFlags::PARACHUTING,
                        );
                        self.need_to_update_rider_bones = true;
                        if let Some(sound) = &self.module_data.parachute_open_sound {
                            let mut event = sound.clone();
                            event.set_object_id(rider_guard.get_id());
                            if let Some(audio) = TheAudio::get() {
                                audio.add_audio_event(&event);
                            }
                        }
                    }
                }

                if let Some(ai) = self
                    .with_owner(|owner| owner.get_ai_update_interface())
                    .flatten()
                {
                    let mut target = self
                        .with_owner(|owner| *owner.get_position())
                        .unwrap_or(Coord3D::new(0.0, 0.0, 0.0));
                    if self.is_landing_override_set {
                        target = self.landing_override;
                        if let Some(loco) = ai.get_cur_locomotor() {
                            if let Ok(mut loco_guard) = loco.lock() {
                                loco_guard.set_ultra_accurate(true);
                            }
                        }
                    } else if let Some(partition) = ThePartitionManager::get() {
                        let mut found = target;
                        if partition.find_position_around(&target, 0.0, 100.0, &mut found) {
                            target = found;
                        }
                    }
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.ai_move_to_position(&target);
                    }
                }
            } else if let Some(rider) = self.resolve_rider() {
                if let Ok(mut rider_guard) = rider.write() {
                    let _ = rider_guard.clear_and_set_model_condition_flags(
                        ModelConditionFlags::PARACHUTING,
                        ModelConditionFlags::FREEFALL,
                    );
                }
            }
        }

        self.hide_until_open();

        let nonempty_open = self.opened && self.base.get_contain_count() > 0;
        self.with_owner_mut(|owner| {
            owner.set_status(ObjectStatusMaskType::NO_COLLISIONS, !nonempty_open);
        });
        if let Some(rider) = self.resolve_rider() {
            if let Ok(mut rider_guard) = rider.write() {
                rider_guard.set_status(ObjectStatusMaskType::NO_COLLISIONS, !nonempty_open);
            }
        }

        let dead = self
            .with_owner(|owner| owner.is_effectively_dead())
            .unwrap_or(false);
        if !dead {
            if let Some(ai) = self
                .with_owner(|owner| owner.get_ai_update_interface())
                .flatten()
            {
                let set = if self.opened {
                    LocomotorSetType::Normal
                } else {
                    LocomotorSetType::Freefall
                };
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.choose_locomotor_set(set);
                }

                if self.opened {
                    let mut altitude_damping = 0.0;
                    if let Some(rider) = self.resolve_rider() {
                        if let Ok(rider_guard) = rider.read() {
                            if rider_guard.get_height_above_terrain() <= ALTITUDE_DAMP_START {
                                altitude_damping = self.module_data.low_altitude_damping;
                            }
                        }
                    }
                    // C++ ParachuteContain::update: locomotor pitch/roll spring+damper.
                    let (pitch_stiffness, roll_stiffness, pitch_damping, roll_damping) =
                        if let Some(loco) = ai.get_cur_locomotor() {
                            if let Ok(loco_guard) = loco.lock() {
                                (
                                    loco_guard.template.pitch_stiffness,
                                    loco_guard.template.roll_stiffness,
                                    loco_guard.template.pitch_damping + altitude_damping,
                                    loco_guard.template.roll_damping + altitude_damping,
                                )
                            } else {
                                (0.1, 0.1, 0.1 + altitude_damping, 0.1 + altitude_damping)
                            }
                        } else {
                            (0.1, 0.1, 0.1 + altitude_damping, 0.1 + altitude_damping)
                        };
                    self.pitch_rate +=
                        (-pitch_stiffness * self.pitch) + (-pitch_damping * self.pitch_rate);
                    self.roll_rate +=
                        (-roll_stiffness * self.roll) + (-roll_damping * self.roll_rate);
                    self.pitch += self.pitch_rate;
                    self.roll += self.roll_rate;

                    if self.is_landing_override_set {
                        if let Some(loco) = ai.get_cur_locomotor() {
                            if let Ok(mut loco_guard) = loco.lock() {
                                loco_guard.set_close_enough_dist(10.0);
                            }
                        }
                    }
                }

                if let Some(drawable) = self.with_owner(|owner| owner.get_drawable()).flatten() {
                    self.update_bone_positions(None);
                    self.update_offsets_from_bones(None);
                    let mtx = self.calc_sway_mtx(&self.para_sway_offset);
                    drawable.set_instance_matrix(Some(&mtx));
                }
                self.position_contained_objects();
            }
        }

        if let Some(para_pos) = self.with_owner(|owner| *owner.get_position()) {
            if let Some(terrain) = TheTerrainLogic::get() {
                let new_layer = terrain.get_highest_layer_for_destination(&para_pos);
                self.with_owner_mut(|owner| owner.set_layer(new_layer));
                if let Some(rider) = self.resolve_rider() {
                    if let Ok(mut rider_guard) = rider.write() {
                        self.position_rider(&mut rider_guard);
                    }
                }

                if self.base.get_contain_count() == 0 {
                    self.with_owner_mut(|owner| owner.kill(None, None));
                } else {
                    let dead = self
                        .with_owner(|owner| owner.is_effectively_dead())
                        .unwrap_or(false);
                    let layer = self
                        .with_owner(|owner| owner.get_layer())
                        .unwrap_or(PathfindLayerEnum::Ground);
                    let mut water_z = 0.0;
                    if !dead
                        && layer == PathfindLayerEnum::Ground
                        && terrain.is_underwater(para_pos.x, para_pos.y, Some(&mut water_z), None)
                        && (para_pos.z - water_z) < self.module_data.kill_when_landing_in_water_slop
                    {
                        self.with_owner_mut(|owner| owner.kill(None, None));
                    }
                }
            }
        }

        Ok(UpdateSleepTime::None)
    }

    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        let obj = TheGameLogic::find_object_by_id(obj_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            .ok_or("Parachute contain object not found")?;
        let was_selected = obj
            .read()
            .ok()
            .and_then(|guard| guard.get_drawable())
            .and_then(|drawable| drawable.read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);
        {
            let obj_ref = obj.read().map_err(|_| "Object lock poisoned")?;
            if !self.is_valid_container_for(&*obj_ref, true) {
                return Err("Object not valid for this parachute".into());
            }
        }
        self.base.add_to_contain_list(obj_id)?;
        self.on_containing(obj_id, was_selected)?;
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
        self.base.remove_from_contain_list(obj_id);
        let _ = expose_stealth_units;
        self.on_removing(obj_id)?;
        Ok(())
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

impl Snapshotable for ParachuteContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_real(&mut self.pitch).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.roll).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.pitch_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.roll_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.start_z)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_landing_override_set)
            .map_err(|e| e.to_string())?;
        OpenContainXfer::coord(xfer, &mut self.landing_override)?;
        OpenContainXfer::coord(xfer, &mut self.rider_attach_bone)?;
        OpenContainXfer::coord(xfer, &mut self.rider_sway_bone)?;
        OpenContainXfer::coord(xfer, &mut self.para_attach_bone)?;
        OpenContainXfer::coord(xfer, &mut self.para_sway_bone)?;
        OpenContainXfer::coord(xfer, &mut self.rider_attach_offset)?;
        OpenContainXfer::coord(xfer, &mut self.rider_sway_offset)?;
        OpenContainXfer::coord(xfer, &mut self.para_attach_offset)?;
        OpenContainXfer::coord(xfer, &mut self.para_sway_offset)?;
        xfer.xfer_bool(&mut self.need_to_update_rider_bones)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.need_to_update_para_bones)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.opened)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

struct OpenContainXfer;
impl OpenContainXfer {
    fn coord(xfer: &mut dyn Xfer, coord: &mut Coord3D) -> Result<(), String> {
        xfer.xfer_real(&mut coord.x).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut coord.y).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut coord.z).map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl ContainModuleInterface for ParachuteContain {
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
        // C++ isSpecialZeroSlotContainer — transport unwraps this container.
        0
    }

    fn is_special_zero_slot_container(&self) -> bool {
        true
    }

    fn is_enclosing_container_for(&self, _obj: &Object) -> bool {
        false
    }

    fn is_displayed_on_control_bar(&self) -> bool {
        false
    }

    fn set_override_destination(&mut self, pos: &Coord3D) {
        ParachuteContain::set_override_destination(self, pos);
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        ParachuteContain::update(self).map_err(|e| e.into())
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
        ParachuteContain::on_die(self, damage_info).map_err(|e| e.into())
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

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ParachuteContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ParachuteContain::on_removing(self, obj_id).map_err(|e| e.into())
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
        let _ = ParachuteContain::process_damage_to_contained(self, percent_damage);
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_selling().map_err(|e| e.into())
    }
}

impl ContainerInterface for ParachuteContain {
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
        (self.base.get_contain_count(), 1)
    }
}

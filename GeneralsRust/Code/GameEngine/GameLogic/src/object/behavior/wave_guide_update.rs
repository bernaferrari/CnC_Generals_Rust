//! WaveGuideUpdate - Flooding wave behavior for waveguide objects
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::audio::AudioEventRts;
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord2D, Coord3D, GameLogicRandomValue, Matrix3D, ModelConditionFlags, ModuleData,
    Real, UnsignedInt, XferVersion, LOGICFRAMES_PER_SECOND,
};
use crate::damage::{DamageInfo, DamageInfoInput, DamageType, DeathType};
use crate::helpers::{
    TheAudio, TheGameLogic, ThePartitionManager, TheRadar, TheTerrainLogic, TheTerrainVisual,
    TheThingFactory,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::DrawableArcExt;
use crate::object::Object as GameObject;
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use glam::Vec4;
use std::sync::{Arc, RwLock, Weak};

const MAX_WAVEGUIDE_SHAPE_POINTS: usize = 64;
const MAX_SHAPE_EFFECTS: usize = 3;
const INVALID_PARTICLE_SYSTEM_ID: u32 = 0;
const PATH_EXTRA_DISTANCE: Real = 10.0 * PATHFIND_CELL_SIZE_F;

#[derive(Clone, Debug)]
pub struct WaveGuideUpdateModuleData {
    pub base: BehaviorModuleData,
    pub wave_delay: Real,
    pub y_size: Real,
    pub linear_wave_spacing: Real,
    pub wave_bend_magnitude: Real,
    pub water_velocity: Real,
    pub preferred_height: Real,
    pub shoreline_effect_distance: Real,
    pub damage_radius: Real,
    pub damage_amount: Real,
    pub topple_force: Real,
    pub random_splash_sound: AudioEventRts,
    pub random_splash_sound_frequency: i32,
    pub bridge_particle: Option<AsciiString>,
    pub bridge_particle_angle_fudge: Real,
    pub looping_sound: AudioEventRts,
}

impl Default for WaveGuideUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            wave_delay: 0.0,
            y_size: 0.0,
            linear_wave_spacing: 0.0,
            wave_bend_magnitude: 0.0,
            water_velocity: 0.0,
            preferred_height: 0.0,
            shoreline_effect_distance: 0.0,
            damage_radius: 0.0,
            damage_amount: 0.0,
            topple_force: 0.0,
            random_splash_sound: AudioEventRts::default(),
            random_splash_sound_frequency: 0,
            bridge_particle: None,
            bridge_particle_angle_fudge: 0.0,
            looping_sound: AudioEventRts::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(WaveGuideUpdateModuleData, base);

impl WaveGuideUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, WAVE_GUIDE_UPDATE_FIELDS)
    }
}

fn parse_duration_real(tokens: &[&str]) -> Result<Real, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    INI::parse_duration_real(tokens[0])
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    INI::parse_real(tokens[0])
}

fn parse_int(tokens: &[&str]) -> Result<i32, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    tokens[0].parse().map_err(|_| INIError::InvalidData)
}

fn parse_wave_delay(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.wave_delay = parse_duration_real(tokens)?;
    Ok(())
}

fn parse_y_size(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.y_size = parse_real(tokens)?;
    Ok(())
}

fn parse_linear_wave_spacing(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.linear_wave_spacing = parse_real(tokens)?;
    Ok(())
}

fn parse_wave_bend_magnitude(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.wave_bend_magnitude = parse_real(tokens)?;
    Ok(())
}

fn parse_water_velocity(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = parse_real(tokens)?;
    data.water_velocity = INI::convert_velocity_secs_to_frames(value);
    Ok(())
}

fn parse_preferred_height(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.preferred_height = parse_real(tokens)?;
    Ok(())
}

fn parse_shoreline_effect_distance(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.shoreline_effect_distance = parse_real(tokens)?;
    Ok(())
}

fn parse_damage_radius(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_radius = parse_real(tokens)?;
    Ok(())
}

fn parse_damage_amount(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_amount = parse_real(tokens)?;
    Ok(())
}

fn parse_topple_force(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.topple_force = parse_real(tokens)?;
    Ok(())
}

fn parse_random_splash_sound(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens[0].eq_ignore_ascii_case("NONE") {
        data.random_splash_sound = AudioEventRts::default();
    } else {
        data.random_splash_sound = AudioEventRts::new(tokens[0]);
    }
    Ok(())
}

fn parse_random_splash_sound_frequency(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.random_splash_sound_frequency = parse_int(tokens)?;
    Ok(())
}

fn parse_bridge_particle(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens[0].eq_ignore_ascii_case("NONE") {
        data.bridge_particle = None;
    } else {
        data.bridge_particle = Some(AsciiString::from(tokens[0]));
    }
    Ok(())
}

fn parse_bridge_particle_angle_fudge(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.bridge_particle_angle_fudge = INI::parse_angle_real(tokens[0])?;
    Ok(())
}

fn parse_looping_sound(
    _ini: &mut INI,
    data: &mut WaveGuideUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens[0].eq_ignore_ascii_case("NONE") {
        data.looping_sound = AudioEventRts::default();
    } else {
        data.looping_sound = AudioEventRts::new(tokens[0]);
    }
    Ok(())
}

const WAVE_GUIDE_UPDATE_FIELDS: &[FieldParse<WaveGuideUpdateModuleData>] = &[
    FieldParse {
        token: "WaveDelay",
        parse: parse_wave_delay,
    },
    FieldParse {
        token: "YSize",
        parse: parse_y_size,
    },
    FieldParse {
        token: "LinearWaveSpacing",
        parse: parse_linear_wave_spacing,
    },
    FieldParse {
        token: "WaveBendMagnitude",
        parse: parse_wave_bend_magnitude,
    },
    FieldParse {
        token: "WaterVelocity",
        parse: parse_water_velocity,
    },
    FieldParse {
        token: "PreferredHeight",
        parse: parse_preferred_height,
    },
    FieldParse {
        token: "ShorelineEffectDistance",
        parse: parse_shoreline_effect_distance,
    },
    FieldParse {
        token: "DamageRadius",
        parse: parse_damage_radius,
    },
    FieldParse {
        token: "DamageAmount",
        parse: parse_damage_amount,
    },
    FieldParse {
        token: "ToppleForce",
        parse: parse_topple_force,
    },
    FieldParse {
        token: "RandomSplashSound",
        parse: parse_random_splash_sound,
    },
    FieldParse {
        token: "RandomSplashSoundFrequency",
        parse: parse_random_splash_sound_frequency,
    },
    FieldParse {
        token: "BridgeParticle",
        parse: parse_bridge_particle,
    },
    FieldParse {
        token: "BridgeParticleAngleFudge",
        parse: parse_bridge_particle_angle_fudge,
    },
    FieldParse {
        token: "LoopingSound",
        parse: parse_looping_sound,
    },
];

pub struct WaveGuideUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<WaveGuideUpdateModuleData>,
    active_frame: UnsignedInt,
    need_disable: bool,
    initialized: bool,
    shape_points: [Coord3D; MAX_WAVEGUIDE_SHAPE_POINTS],
    transformed_shape_points: [Coord3D; MAX_WAVEGUIDE_SHAPE_POINTS],
    shape_effects: [[u32; MAX_SHAPE_EFFECTS]; MAX_WAVEGUIDE_SHAPE_POINTS],
    shape_point_count: usize,
    splash_sound_frame: UnsignedInt,
    final_destination: Coord3D,
}

impl WaveGuideUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<WaveGuideUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(data.clone()),
            active_frame: 0,
            need_disable: true,
            initialized: false,
            shape_points: [Coord3D::ZERO; MAX_WAVEGUIDE_SHAPE_POINTS],
            transformed_shape_points: [Coord3D::ZERO; MAX_WAVEGUIDE_SHAPE_POINTS],
            shape_effects: [[INVALID_PARTICLE_SYSTEM_ID; MAX_SHAPE_EFFECTS];
                MAX_WAVEGUIDE_SHAPE_POINTS],
            shape_point_count: 0,
            splash_sound_frame: 0,
            final_destination: Coord3D::ZERO,
        })
    }

    fn start_moving(&mut self) -> bool {
        let Some(object_arc) = self.object.upgrade() else {
            return false;
        };
        let Ok(mut waveguide) = object_arc.write() else {
            return false;
        };

        if let Some(audio) = TheAudio::get() {
            let mut event = self.module_data.looping_sound.clone();
            event.set_object_id(waveguide.get_id());
            audio.add_audio_event(&event);
        }

        let terrain = crate::terrain::get_terrain_logic();
        let Ok(terrain_guard) = terrain.read() else {
            return false;
        };
        let waypoint_name = AsciiString::from("WaveGuide1");
        let Some(waypoint) = terrain_guard.get_waypoint_by_name(&waypoint_name) else {
            return true;
        };

        let mut verify = Some(waypoint);
        while let Some(node) = verify {
            if node.get_num_links() > 1 {
                return false;
            }
            self.final_destination = *node.get_location();
            verify = node
                .get_link(0)
                .and_then(|id| terrain_guard.get_waypoint_by_id(id));
        }

        let Some(next_id) = waypoint.get_link(0) else {
            return false;
        };
        let Some(next) = terrain_guard.get_waypoint_by_id(next_id) else {
            return false;
        };

        let v = Coord2D::new(
            next.get_location().x - waypoint.get_location().x,
            next.get_location().y - waypoint.get_location().y,
        );
        let angle = v.y.atan2(v.x);
        let _ = waveguide.set_orientation(angle);

        if let Some(ai) = waveguide.get_ai_update_interface() {
            if let Ok(mut guard) = ai.lock() {
                let mut pos = *waypoint.get_location();
                if let Some(terrain_logic) = TheTerrainLogic::get() {
                    pos.z = terrain_logic.get_ground_height(pos.x, pos.y, None);
                }
                let _ = waveguide.set_position(&pos);

                let mut params = AiCommandParams::new(
                    AiCommandType::FollowWaypointPath,
                    CommandSourceType::FromAi,
                );
                params.waypoint = Some(waypoint.get_id());
                let _ = guard.execute_command(&params);
                let _ = guard.set_path_extra_distance(PATH_EXTRA_DISTANCE);
            }
        }

        true
    }

    fn init_waveguide(&mut self) -> bool {
        if !self.start_moving() {
            return false;
        }

        self.compute_wave_shape_points();

        let Some(object_arc) = self.object.upgrade() else {
            return false;
        };
        let Ok(waveguide) = object_arc.read() else {
            return false;
        };
        let waveguide_id = waveguide.get_id();
        if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
            for i in 0..self.shape_point_count {
                if let Some(id) = ps_manager.create_particle_system(Some("WaveSpray01")) {
                    ps_manager.set_particle_system_position(id, &self.shape_points[i]);
                    ps_manager.attach_particle_system_to_object(id, waveguide_id);
                    self.shape_effects[i][0] = id;
                }
                if let Some(id) = ps_manager.create_particle_system(Some("WaveSpray02")) {
                    ps_manager.set_particle_system_position(id, &self.shape_points[i]);
                    ps_manager.attach_particle_system_to_object(id, waveguide_id);
                    self.shape_effects[i][1] = id;
                }
                if i % 5 == 0 {
                    if let Some(id) = ps_manager.create_particle_system(Some("WaveSpray03")) {
                        ps_manager.set_particle_system_position(id, &self.shape_points[i]);
                        ps_manager.attach_particle_system_to_object(id, waveguide_id);
                        self.shape_effects[i][2] = id;
                    }
                }
            }
        }

        true
    }

    fn compute_wave_shape_points(&mut self) {
        self.shape_point_count = 0;
        let step = self.module_data.linear_wave_spacing as i32;
        if step == 0 {
            return;
        }

        let half_y = (self.module_data.y_size / 2.0) as i32;
        let mut y = -half_y;
        while y < half_y && self.shape_point_count < MAX_WAVEGUIDE_SHAPE_POINTS {
            let y_f = y as Real;
            let x = if self.module_data.wave_bend_magnitude != 0.0 {
                -(y_f * y_f) / self.module_data.wave_bend_magnitude
            } else {
                0.0
            };
            self.shape_points[self.shape_point_count] = Coord3D::new(x, y_f, 0.0);
            self.shape_point_count += 1;
            y += step;
        }
    }

    fn transform_wave_shape(&mut self) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(waveguide) = object_arc.read() else {
            return;
        };
        let transform = waveguide.get_transform_matrix();

        for i in 0..self.shape_point_count {
            let local = self.shape_points[i];
            let transformed = transform.transform_point3(local);
            let mut world = Coord3D::new(transformed.x, transformed.y, transformed.z);
            if let Some(terrain_logic) = TheTerrainLogic::get() {
                world.z = terrain_logic.get_ground_height(world.x, world.y, None);
            }
            self.transformed_shape_points[i] = world;
        }
    }

    fn do_shape_effects(&self) {
        if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
            for i in 0..self.shape_point_count {
                let world = self.transformed_shape_points[i];
                if world.z >= self.module_data.preferred_height {
                    continue;
                }
                for effect_id in self.shape_effects[i] {
                    if effect_id == INVALID_PARTICLE_SYSTEM_ID {
                        continue;
                    }
                    if let Some(mut current) = ps_manager.get_particle_system_position(effect_id) {
                        current.z = world.z;
                        ps_manager.set_particle_system_position(effect_id, &current);
                    }
                }
            }
        }
    }

    fn do_water_motion(&self) {
        let Some(terrain_visual) = TheTerrainVisual::get() else {
            return;
        };
        for i in 0..self.shape_point_count {
            let point = self.transformed_shape_points[i];
            terrain_visual.add_water_velocity(
                point.x,
                point.y,
                self.module_data.water_velocity,
                self.module_data.preferred_height,
            );
        }
    }

    fn do_shore_effects(&self) {
        if TheGameLogic::get_frame() & 0x1 != 0 {
            return;
        }

        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(waveguide) = object_arc.read() else {
            return;
        };

        let mut effect_points = [Coord3D::ZERO; MAX_WAVEGUIDE_SHAPE_POINTS];
        for i in 0..self.shape_point_count {
            let mut pt = self.shape_points[i];
            pt.x -= self.module_data.shoreline_effect_distance;
            let transformed = waveguide.get_transform_matrix().transform_point3(pt);
            effect_points[i] = Coord3D::new(transformed.x, transformed.y, transformed.z);
        }

        let mut under_water = true;
        for i in 0..self.shape_point_count {
            let point = effect_points[i];
            let terrain_z = TheTerrainLogic::get()
                .map(|terrain| terrain.get_ground_height(point.x, point.y, None))
                .unwrap_or(0.0);
            if terrain_z > self.module_data.preferred_height {
                if under_water && i != 0 {
                    if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
                        if let Some(id) =
                            ps_manager.create_particle_system(Some("WaveSplashRight01"))
                        {
                            ps_manager.set_particle_system_position(id, &effect_points[i - 1]);
                        }
                    }
                }
                under_water = false;
            } else {
                if !under_water && i != 0 {
                    if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
                        if let Some(id) =
                            ps_manager.create_particle_system(Some("WaveSplashLeft01"))
                        {
                            ps_manager.set_particle_system_position(id, &effect_points[i]);
                        }
                    }
                }
                under_water = true;
            }
        }
    }

    fn do_damage(&self) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(waveguide) = object_arc.read() else {
            return;
        };

        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        for i in 0..self.shape_point_count {
            let center = self.transformed_shape_points[i];
            let local_point = self.shape_points[i];
            for id in partition.get_objects_in_range(&center, self.module_data.damage_radius) {
                if id == waveguide.get_id() {
                    continue;
                }
                let Some(obj_arc) = TheGameLogic::find_object_by_id(id) else {
                    continue;
                };
                let Ok(mut obj) = obj_arc.write() else {
                    continue;
                };

                let obj_pos = *obj.get_position();
                if obj.is_kind_of(crate::common::KindOf::WaveGuide)
                    || obj.is_kind_of(crate::common::KindOf::BridgeTower)
                {
                    continue;
                }

                if obj_pos.z > self.module_data.preferred_height
                    && !obj.is_kind_of(crate::common::KindOf::Bridge)
                {
                    continue;
                }

                let mut v = Coord3D::new(obj_pos.x - center.x, obj_pos.y - center.y, 0.0);
                if v.length_squared() > 0.0 {
                    v = v.normalize();
                }
                let angle = v.x * center.x + v.y * center.y + v.z * center.z;
                if angle >= 0.0 {
                    continue;
                }

                if !obj
                    .get_status_bits()
                    .test(crate::common::ObjectStatusTypes::Wet)
                {
                    if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
                        if let Some(id) = ps_manager.create_particle_system(Some("WaveHit01")) {
                            let pos = Coord3D::new(local_point.x, local_point.y, obj_pos.z);
                            ps_manager.set_particle_system_position(id, &pos);
                            ps_manager.attach_particle_system_to_object(id, waveguide.get_id());
                        }
                    }

                    obj.set_status(crate::common::ObjectStatusMaskType::WET, true);

                    let topple_vec = Coord3D::new(obj_pos.x - center.x, obj_pos.y - center.y, 0.0);
                    obj.topple(
                        &topple_vec,
                        self.module_data.topple_force,
                        crate::object::behavior::topple_update::TOPPLE_OPTIONS_NO_BOUNCE
                            | crate::object::behavior::topple_update::TOPPLE_OPTIONS_NO_FX,
                    );

                    let mut damage = DamageInfo::new();
                    damage.input = DamageInfoInput {
                        amount: self.module_data.damage_amount,
                        source_id: waveguide.get_id(),
                        damage_type: DamageType::Water,
                        death_type: DeathType::Flooded,
                        ..DamageInfoInput::default()
                    };
                    damage.sync_from_input();
                    let _ = obj.attempt_damage_with_return(&mut damage);

                    if let Some(drawable) = obj.get_drawable() {
                        drawable.set_model_condition_state(ModelConditionFlags::FLOODED);
                        drawable.set_shadows_enabled(false);
                    }
                    if obj.is_kind_of(crate::common::KindOf::Bridge) {
                        if let Ok(factory) = TheThingFactory::get() {
                            if let Some(template) =
                                TheThingFactory::find_template("WaterWaveBridge")
                            {
                                if let Ok(new_bridge_arc) =
                                    factory.new_object_optional_team(template, None)
                                {
                                    if let Ok(mut new_bridge) = new_bridge_arc.write() {
                                        let _ = new_bridge.set_position(&obj_pos);
                                        let mut angle = 0.0;
                                        if let Ok(terrain_guard) =
                                            crate::terrain::get_terrain_logic().read()
                                        {
                                            if let Some(bridge) =
                                                terrain_guard.find_bridge_at(&obj_pos)
                                            {
                                                let bridge_info = bridge.get_bridge_info();
                                                let v = Coord2D::new(
                                                    bridge_info.to.x - bridge_info.from.x,
                                                    bridge_info.to.y - bridge_info.from.y,
                                                );
                                                angle = v.y.atan2(v.x);
                                            }
                                        }
                                        let _ = new_bridge.set_orientation(angle);

                                        if let Some(bridge_fx) =
                                            self.module_data.bridge_particle.as_ref()
                                        {
                                            if let Some(ps_manager) =
                                                crate::helpers::TheParticleSystemManager::get()
                                            {
                                                if let Some(id) = ps_manager.create_particle_system(
                                                    Some(bridge_fx.as_str()),
                                                ) {
                                                    let fudge = self
                                                        .module_data
                                                        .bridge_particle_angle_fudge;
                                                    let u = Coord3D::new(
                                                        (angle + fudge).cos(),
                                                        (angle + fudge).sin(),
                                                        0.0,
                                                    );
                                                    let z = Coord3D::new(0.0, 0.0, 1.0);
                                                    let y = z.cross(u);
                                                    let x = y.cross(z);
                                                    let transform = Matrix3D::from_cols(
                                                        Vec4::new(x.x, x.y, x.z, 0.0),
                                                        Vec4::new(y.x, y.y, y.z, 0.0),
                                                        Vec4::new(z.x, z.y, z.z, 0.0),
                                                        Vec4::new(
                                                            obj_pos.x, obj_pos.y, obj_pos.z, 1.0,
                                                        ),
                                                    );
                                                    ps_manager.set_particle_system_transform(
                                                        id, &transform,
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    if let Ok(mut terrain_guard) =
                                        crate::terrain::get_terrain_logic().write()
                                    {
                                        let _ = terrain_guard.delete_bridge_at(&obj_pos);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl UpdateModuleInterface for WaveGuideUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::None;
        };
        let Ok(mut waveguide) = object_arc.write() else {
            return UpdateSleepTime::None;
        };

        if self.need_disable {
            self.need_disable = false;
            waveguide.set_disabled(crate::common::DisabledType::DisabledDefault);
            return UpdateSleepTime::None;
        }

        if waveguide.is_disabled() {
            return UpdateSleepTime::None;
        }

        if self.active_frame == 0 {
            self.active_frame = TheGameLogic::get_frame();
        }

        let frames_since_active = TheGameLogic::get_frame().saturating_sub(self.active_frame);
        if (frames_since_active as f32) < self.module_data.wave_delay {
            return UpdateSleepTime::None;
        }

        if !self.initialized {
            if !self.init_waveguide() {
                let _ = TheGameLogic::destroy_object(&*waveguide);
                return UpdateSleepTime::None;
            }
            self.initialized = true;
        }

        if (TheGameLogic::get_frame().saturating_sub(self.splash_sound_frame) as f32)
            > (crate::common::LOGICFRAMES_PER_SECOND as f32 / 2.0)
        {
            self.splash_sound_frame = TheGameLogic::get_frame();
            if GameLogicRandomValue(1, 100) > self.module_data.random_splash_sound_frequency {
                if let Some(audio) = TheAudio::get() {
                    let mut event = self.module_data.random_splash_sound.clone();
                    event.set_object_id(waveguide.get_id());
                    audio.add_audio_event(&event);
                }
            }
        }

        self.transform_wave_shape();

        let pos = *waveguide.get_position();
        let v = Coord2D::new(
            self.final_destination.x - pos.x,
            self.final_destination.y - pos.y,
        );
        if v.x * v.x + v.y * v.y <= PATH_EXTRA_DISTANCE * PATH_EXTRA_DISTANCE {
            if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
                if let Some(id) = ps_manager.create_particle_system(Some("WaveSplash01")) {
                    let transform = waveguide.get_transform_matrix();
                    ps_manager.set_particle_system_transform(id, &transform);
                }
            }
            let _ = TheGameLogic::destroy_object(&*waveguide);
            if let Some(radar) = TheRadar::get() {
                radar.refresh_terrain();
            }
            return UpdateSleepTime::None;
        }

        self.do_shape_effects();
        self.do_water_motion();
        self.do_shore_effects();
        self.do_damage();

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for WaveGuideUpdate {
    fn get_module_name(&self) -> &'static str {
        "WaveGuideUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for WaveGuideUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_unsigned_int(&mut self.active_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.need_disable)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.initialized)
            .map_err(|e| e.to_string())?;

        for point in &mut self.shape_points {
            xfer.xfer_coord3d(point);
        }

        for point in &mut self.transformed_shape_points {
            xfer.xfer_coord3d(point);
        }

        for effects in &mut self.shape_effects {
            for effect in effects {
                xfer.xfer_particle_system_id(effect);
            }
        }

        let mut shape_point_count = self.shape_point_count as i32;
        xfer.xfer_i32(&mut shape_point_count)
            .map_err(|e| e.to_string())?;
        self.shape_point_count =
            shape_point_count.clamp(0, MAX_WAVEGUIDE_SHAPE_POINTS as i32) as usize;

        xfer.xfer_unsigned_int(&mut self.splash_sound_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_coord3d(&mut self.final_destination);
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct WaveGuideUpdateFactory;
impl WaveGuideUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(WaveGuideUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_real_accepts_suffixes() {
        assert!((parse_duration_real(&["1.5s"]).expect("duration") - 45.0).abs() < 0.001);
        assert!((parse_duration_real(&["500ms"]).expect("duration") - 15.0).abs() < 0.001);
    }
}

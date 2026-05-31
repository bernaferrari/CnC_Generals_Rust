//! ToppleUpdate - Toppling behavior for objects (trees, props, etc.)
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::ai::integration::with_ai_integration_mut;
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, ICoord2D, Matrix3D, ModuleData, Real, UnsignedInt, XferVersion,
    INVALID_ID,
};
use crate::damage::{DamageInfo, DamageInfoInput, DamageType, DeathType, HUGE_DAMAGE_AMOUNT};
use crate::effects::FXList;
use crate::helpers::{TheFXListStore, TheGameLogic, TheThingFactory};
use crate::modules::UpdateSleepTime;
use crate::modules::{
    BehaviorModuleInterface, CollideModuleInterface, PhysicsBehaviorExt, ToppleControlInterface,
    UpdateModuleInterface,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::DrawableArcExt;
use crate::object::Object as GameObject;
use crate::path::{grid_to_world, world_to_grid, PathfindLayerEnum, PATHFIND_CELL_SIZE_F};
use crate::scripting::engine::get_script_engine;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

const ANGULAR_LIMIT: Real = std::f32::consts::PI / 2.0 - std::f32::consts::PI / 64.0;
const VELOCITY_BOUNCE_LIMIT: Real = 0.01;
const VELOCITY_BOUNCE_SOUND_LIMIT: Real = 0.03;

pub const TOPPLE_OPTIONS_NONE: u32 = 0x0000_0000;
pub const TOPPLE_OPTIONS_NO_BOUNCE: u32 = 0x0000_0001;
pub const TOPPLE_OPTIONS_NO_FX: u32 = 0x0000_0002;

#[derive(Clone, Debug)]
pub struct ToppleUpdateModuleData {
    pub base: BehaviorModuleData,
    pub topple_fx: Option<Arc<FXList>>,
    pub bounce_fx: Option<Arc<FXList>>,
    pub stump_name: AsciiString,
    pub initial_velocity_percent: Real,
    pub initial_accel_percent: Real,
    pub bounce_velocity_percent: Real,
    pub kill_when_toppled: bool,
    pub kill_when_start_toppled: bool,
    pub kill_stump_when_toppled: bool,
    pub topple_left_or_right_only: bool,
    pub reorient_toppled_rubble: bool,
}

impl Default for ToppleUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            topple_fx: None,
            bounce_fx: None,
            stump_name: AsciiString::new(),
            initial_velocity_percent: 0.2,
            initial_accel_percent: 0.01,
            bounce_velocity_percent: 0.3,
            kill_when_toppled: true,
            kill_when_start_toppled: false,
            kill_stump_when_toppled: false,
            topple_left_or_right_only: false,
            reorient_toppled_rubble: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(ToppleUpdateModuleData, base);

impl ToppleUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, TOPPLE_UPDATE_FIELDS)
    }
}

fn parse_fx_list(data_field: &mut Option<Arc<FXList>>, tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    if token.eq_ignore_ascii_case("NONE") {
        *data_field = None;
        return Ok(());
    }
    *data_field = TheFXListStore::find_fx_list(token);
    Ok(())
}

fn parse_topple_fx(
    _ini: &mut INI,
    data: &mut ToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.topple_fx, tokens)
}

fn parse_bounce_fx(
    _ini: &mut INI,
    data: &mut ToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_list(&mut data.bounce_fx, tokens)
}

fn parse_stump_name(
    _ini: &mut INI,
    data: &mut ToppleUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.stump_name = AsciiString::from(token);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_bool_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(bool),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_percent_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_percent_to_real(token)?);
    Ok(())
}

const TOPPLE_UPDATE_FIELDS: &[FieldParse<ToppleUpdateModuleData>] = &[
    FieldParse {
        token: "ToppleFX",
        parse: parse_topple_fx,
    },
    FieldParse {
        token: "BounceFX",
        parse: parse_bounce_fx,
    },
    FieldParse {
        token: "StumpName",
        parse: parse_stump_name,
    },
    FieldParse {
        token: "KillWhenStartToppling",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |value| data.kill_when_start_toppled = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "KillWhenFinishedToppling",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |value| data.kill_when_toppled = value, tokens)
        },
    },
    FieldParse {
        token: "KillStumpWhenToppled",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |value| data.kill_stump_when_toppled = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "ToppleLeftOrRightOnly",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |value| data.topple_left_or_right_only = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "ReorientToppledRubble",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |value| data.reorient_toppled_rubble = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "InitialVelocityPercent",
        parse: |ini, data, tokens| {
            parse_percent_field(
                ini,
                &mut |value| data.initial_velocity_percent = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "InitialAccelPercent",
        parse: |ini, data, tokens| {
            parse_percent_field(ini, &mut |value| data.initial_accel_percent = value, tokens)
        },
    },
    FieldParse {
        token: "BounceVelocityPercent",
        parse: |ini, data, tokens| {
            parse_percent_field(
                ini,
                &mut |value| data.bounce_velocity_percent = value,
                tokens,
            )
        },
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToppleState {
    Upright,
    Falling,
    Down,
}

pub struct ToppleUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<ToppleUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    angular_velocity: Real,
    angular_acceleration: Real,
    topple_direction: Coord3D,
    topple_state: ToppleState,
    angular_accumulation: Real,
    angle_delta_x: Real,
    num_angle_delta_x: i32,
    do_bounce_fx: bool,
    options: u32,
    stump_id: crate::common::ObjectID,
}

impl ToppleUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<ToppleUpdateModuleData>()
            .ok_or("Invalid module data")?;

        if let Ok(obj) = object.read() {
            TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Coord3D::ZERO,
            topple_state: ToppleState::Upright,
            angular_accumulation: 0.0,
            angle_delta_x: 0.0,
            num_angle_delta_x: 0,
            do_bounce_fx: false,
            options: TOPPLE_OPTIONS_NONE,
            stump_id: crate::common::INVALID_ID,
        })
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<ToppleUpdateModuleData>,
    ) -> Self {
        Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Coord3D::ZERO,
            topple_state: ToppleState::Upright,
            angular_accumulation: 0.0,
            angle_delta_x: 0.0,
            num_angle_delta_x: 0,
            do_bounce_fx: false,
            options: TOPPLE_OPTIONS_NONE,
            stump_id: crate::common::INVALID_ID,
        }
    }

    fn normalize_angle(mut angle: Real) -> Real {
        let two_pi = std::f32::consts::PI * 2.0;
        while angle > std::f32::consts::PI {
            angle -= two_pi;
        }
        while angle < -std::f32::consts::PI {
            angle += two_pi;
        }
        angle
    }

    fn std_angle_diff(angle1: Real, angle2: Real) -> Real {
        Self::normalize_angle(angle1 - angle2)
    }

    fn angle_closest_to(a1: Real, a2: Real, desired: Real) -> Real {
        let a1 = Self::normalize_angle(a1);
        let a2 = Self::normalize_angle(a2);
        if Self::std_angle_diff(desired, a1).abs() < Self::std_angle_diff(desired, a2).abs() {
            a1
        } else {
            a2
        }
    }

    fn death_by_toppling(obj: &mut GameObject) {
        let mut info = DamageInfo::new();
        info.input = DamageInfoInput {
            amount: HUGE_DAMAGE_AMOUNT,
            source_id: crate::common::INVALID_ID,
            damage_type: DamageType::Unresistable,
            death_type: DeathType::Toppled,
            ..DamageInfoInput::default()
        };
        info.sync_from_input();
        let _ = obj.attempt_damage_with_return(&mut info);
    }

    fn pathfinding_footprint_positions(obj: &GameObject) -> Vec<Coord3D> {
        let pos = *obj.get_position();
        let radius = obj.get_geometry_info().get_bounding_circle_radius();
        let center = world_to_grid(&pos);
        let cell_radius = (radius / PATHFIND_CELL_SIZE_F).ceil() as i32;
        let mut positions = Vec::new();

        for dy in -cell_radius..=cell_radius {
            for dx in -cell_radius..=cell_radius {
                let cell = ICoord2D::new(center.x + dx, center.y + dy);
                let world = grid_to_world(&cell, PathfindLayerEnum::Ground);
                let delta = Coord3D::new(world.x - pos.x, world.y - pos.y, 0.0);
                if delta.length_squared() <= radius * radius {
                    positions.push(world);
                }
            }
        }

        if positions.is_empty() {
            positions.push(pos);
        }

        positions
    }

    pub fn apply_toppling_force(
        &mut self,
        topple_direction: &Coord3D,
        topple_speed: Real,
        options: u32,
    ) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(mut obj) = object_arc.write() else {
            return;
        };
        self.apply_toppling_force_with_object(
            &mut obj,
            &object_arc,
            topple_direction,
            topple_speed,
            options,
        );
    }

    pub fn apply_toppling_force_with_object(
        &mut self,
        obj: &mut GameObject,
        object_arc: &Arc<RwLock<GameObject>>,
        topple_direction: &Coord3D,
        topple_speed: Real,
        options: u32,
    ) {
        if obj.is_effectively_dead() {
            return;
        }

        TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::None);

        if self.module_data.kill_when_start_toppled {
            TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
            obj.kill(None, None);
            return;
        }

        self.topple_direction = *topple_direction;
        if self.topple_direction.length_squared() > 0.0 {
            self.topple_direction = self.topple_direction.normalize();
        }

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                engine.adjust_topple_direction(obj, &mut self.topple_direction);
            }
        }

        self.angular_velocity = topple_speed * self.module_data.initial_velocity_percent;
        self.angular_acceleration = topple_speed * self.module_data.initial_accel_percent;
        self.topple_state = ToppleState::Falling;
        self.options = options;

        if let Some(drawable) = obj.get_drawable() {
            drawable.set_swaying_enabled(false);
        }

        let cur_angle_x = Self::normalize_angle(obj.get_orientation());
        let mut topple_angle =
            Self::normalize_angle(self.topple_direction.y.atan2(self.topple_direction.x));

        if self.module_data.topple_left_or_right_only {
            topple_angle = Self::angle_closest_to(
                cur_angle_x + std::f32::consts::PI / 2.0,
                cur_angle_x - std::f32::consts::PI / 2.0,
                topple_angle,
            );
            self.topple_direction.x = topple_angle.cos();
            self.topple_direction.y = topple_angle.sin();
            let positions = Self::pathfinding_footprint_positions(obj);
            let object_id = obj.get_id();
            let _ = with_ai_integration_mut(|manager| {
                manager.remove_pathfinding_obstacle(object_id, &positions)
            });
        }

        let desired_angle_x = Self::angle_closest_to(
            topple_angle + std::f32::consts::PI / 2.0,
            topple_angle - std::f32::consts::PI / 2.0,
            cur_angle_x,
        );
        if self.angular_velocity == 0.0 {
            self.num_angle_delta_x = 1;
        } else {
            self.num_angle_delta_x = (ANGULAR_LIMIT / (self.angular_velocity * 2.0)).floor() as i32;
        }
        if self.num_angle_delta_x < 1 {
            self.num_angle_delta_x = 1;
        }
        self.angle_delta_x = (desired_angle_x - cur_angle_x) / self.num_angle_delta_x as Real;

        obj.set_model_condition_state(crate::common::ModelConditionFlags::TOPPLED);
        if let Some(fx) = &self.module_data.topple_fx {
            let _ = fx.do_fx_obj(object_arc, None);
        }

        if !self.module_data.stump_name.is_empty() {
            if let Ok(factory) = TheThingFactory::get() {
                if let Some(template) =
                    TheThingFactory::find_template(self.module_data.stump_name.as_str())
                {
                    if let Ok(stump_arc) = factory.new_object_optional_team(template, None) {
                        if let Ok(mut stump) = stump_arc.write() {
                            let _ = stump.set_position(obj.get_position());
                            let _ = stump.set_orientation(obj.get_orientation());
                            self.stump_id = stump.get_id();
                            if let Some(src_draw) = obj.get_drawable() {
                                let flags = src_draw.get_model_condition_flags();
                                if let Some(stump_draw) = stump.get_drawable() {
                                    if flags.contains(crate::common::ModelConditionFlags::BURNED) {
                                        stump_draw.set_model_condition_state(
                                            crate::common::ModelConditionFlags::BURNED,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn is_able_to_be_toppled(&self) -> bool {
        self.topple_state == ToppleState::Upright
    }
}

impl UpdateModuleInterface for ToppleUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if matches!(self.topple_state, ToppleState::Upright | ToppleState::Down) {
            return UpdateSleepTime::Forever;
        }

        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(mut obj) = object_arc.write() else {
            return UpdateSleepTime::Forever;
        };

        if self.num_angle_delta_x > 0 {
            let mut xfrm = obj.get_transform_matrix();
            let rot = Matrix3D::from_rotation_z(self.angle_delta_x);
            xfrm = rot * xfrm;
            obj.set_transform_matrix(&xfrm);
            self.num_angle_delta_x -= 1;
        }

        let mut cur_vel_to_use = self.angular_velocity;
        if self.angular_accumulation + cur_vel_to_use > ANGULAR_LIMIT {
            cur_vel_to_use = ANGULAR_LIMIT - self.angular_accumulation;
        }

        let mut xfrm = obj.get_transform_matrix();
        let rot_x = Matrix3D::from_rotation_x(-cur_vel_to_use * self.topple_direction.y);
        let rot_y = Matrix3D::from_rotation_y(cur_vel_to_use * self.topple_direction.x);
        xfrm = rot_y * rot_x * xfrm;
        obj.set_transform_matrix(&xfrm);

        self.angular_accumulation += cur_vel_to_use;
        if self.angular_accumulation >= ANGULAR_LIMIT && self.angular_velocity > 0.0 {
            self.angular_velocity *= -self.module_data.bounce_velocity_percent;

            let no_bounce = (self.options & TOPPLE_OPTIONS_NO_BOUNCE) != 0;
            if no_bounce || self.angular_velocity.abs() < VELOCITY_BOUNCE_LIMIT {
                self.angular_velocity = 0.0;
                self.topple_state = ToppleState::Down;

                if self.module_data.kill_when_toppled {
                    Self::death_by_toppling(&mut obj);
                    if self.module_data.reorient_toppled_rubble {
                        let mut pos = Coord3D::new(
                            0.0,
                            0.0,
                            obj.get_geometry_info().get_max_height_above_position(),
                        );
                        let xfrm = obj.get_transform_matrix();
                        let transformed = xfrm.transform_vector3(pos);
                        pos = Coord3D::new(transformed.x, transformed.y, transformed.z);
                        let _ = obj.set_position(&pos);
                        let orientation = obj.get_orientation();
                        let _ = obj.set_orientation(orientation);
                    }
                }

                if self.module_data.kill_stump_when_toppled {
                    if let Some(stump_arc) = TheGameLogic::find_object_by_id(self.stump_id) {
                        if let Ok(mut stump) = stump_arc.write() {
                            Self::death_by_toppling(&mut stump);
                        }
                    }
                }
            } else if self.angular_velocity.abs() >= VELOCITY_BOUNCE_SOUND_LIMIT {
                let no_fx = (self.options & TOPPLE_OPTIONS_NO_FX) != 0;
                if !no_fx {
                    if let Some(fx) = &self.module_data.bounce_fx {
                        let _ = fx.do_fx_obj(&object_arc, None);
                    }
                }
            }
        } else {
            self.angular_velocity += self.angular_acceleration;
        }

        if let Some(draw) = obj.get_drawable() {
            draw.set_shadows_enabled(false);
        }

        UpdateSleepTime::None
    }
}

impl CollideModuleInterface for ToppleUpdate {
    fn on_collision(
        &mut self,
        _object_id: crate::common::ObjectID,
        other_id: crate::common::ObjectID,
    ) {
        if other_id == crate::common::INVALID_ID {
            return;
        }
        let Some(other_arc) = TheGameLogic::find_object_by_id(other_id) else {
            return;
        };
        let Ok(other) = other_arc.read() else {
            return;
        };

        if other.get_crusher_level() <= 1 {
            return;
        }

        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj) = object_arc.read() else {
            return;
        };

        let mut topple_vec = *obj.get_position() - *other.get_position();
        topple_vec.z = 0.0;

        let speed = other
            .get_physics()
            .map(|phys| phys.get_velocity())
            .map(|v| v.length())
            .unwrap_or(0.0);

        drop(obj);
        self.apply_toppling_force(&topple_vec, speed, TOPPLE_OPTIONS_NONE);
    }
}

impl BehaviorModuleInterface for ToppleUpdate {
    fn get_module_name(&self) -> &'static str {
        "ToppleUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_collide(&mut self) -> Option<&mut dyn CollideModuleInterface> {
        Some(self)
    }

    fn get_topple_control_interface(&mut self) -> Option<&mut dyn ToppleControlInterface> {
        Some(self)
    }
}

impl ToppleControlInterface for ToppleUpdate {
    fn is_able_to_be_toppled(&self) -> bool {
        ToppleUpdate::is_able_to_be_toppled(self)
    }

    fn apply_toppling_force(
        &mut self,
        topple_direction: &Coord3D,
        topple_speed: Real,
        options: u32,
    ) {
        ToppleUpdate::apply_toppling_force(self, topple_direction, topple_speed, options);
    }

    fn apply_toppling_force_with_object(
        &mut self,
        obj: &mut GameObject,
        object_arc: &Arc<RwLock<GameObject>>,
        topple_direction: &Coord3D,
        topple_speed: Real,
        options: u32,
    ) {
        ToppleUpdate::apply_toppling_force_with_object(
            self,
            obj,
            object_arc,
            topple_direction,
            topple_speed,
            options,
        );
    }
}

/// Glue that exposes ToppleUpdate through the common Module trait.
pub struct ToppleUpdateModule {
    behavior: ToppleUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<ToppleUpdateModuleData>,
}

impl ToppleUpdateModule {
    pub fn new(
        behavior: ToppleUpdate,
        module_name: &AsciiString,
        module_data: Arc<ToppleUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut ToppleUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for ToppleUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for ToppleUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn get_topple_control_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::module::ToppleControlInterface> {
        Some(self)
    }
}

impl game_engine::common::thing::module::ToppleControlInterface for ToppleUpdateModule {
    fn apply_toppling_force(&mut self, x: f32, y: f32, z: f32, topple_speed: f32, options: u32) {
        let direction = Coord3D::new(x, y, z);
        self.behavior
            .apply_toppling_force(&direction, topple_speed, options);
    }
}

pub fn topple_update_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = ToppleUpdateModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse ToppleUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

pub fn topple_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<ToppleUpdateModuleData>()
        .expect("ToppleUpdateModuleData expected");

    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("ToppleUpdate requires a valid object");
    let behavior = ToppleUpdate::new_from_object_handle(object, Arc::clone(&module_data_arc));

    let module_name = AsciiString::from("ToppleUpdate");
    Box::new(ToppleUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

impl Snapshotable for ToppleUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_real(&mut self.angular_velocity)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angular_acceleration)
            .map_err(|e| e.to_string())?;
        xfer.xfer_coord3d(&mut self.topple_direction);

        let mut topple_state = self.topple_state as i32;
        xfer.xfer_i32(&mut topple_state)
            .map_err(|e| e.to_string())?;
        self.topple_state = match topple_state {
            1 => ToppleState::Falling,
            2 => ToppleState::Down,
            _ => ToppleState::Upright,
        };

        xfer.xfer_real(&mut self.angular_accumulation)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angle_delta_x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_i32(&mut self.num_angle_delta_x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.do_bounce_fx)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.options)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.stump_id)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topple_update_defaults_match_cpp_constructor() {
        let data = ToppleUpdateModuleData::default();

        assert!(data.topple_fx.is_none());
        assert!(data.bounce_fx.is_none());
        assert!(data.stump_name.is_empty());
        assert_eq!(data.initial_velocity_percent, 0.2);
        assert_eq!(data.initial_accel_percent, 0.01);
        assert_eq!(data.bounce_velocity_percent, 0.3);
        assert!(data.kill_when_toppled);
        assert!(!data.kill_when_start_toppled);
        assert!(!data.kill_stump_when_toppled);
        assert!(!data.topple_left_or_right_only);
        assert!(!data.reorient_toppled_rubble);
    }

    #[test]
    fn topple_update_fields_use_cpp_ini_token_handling() {
        let mut ini = INI::new();
        let mut data = ToppleUpdateModuleData::default();

        parse_stump_name(&mut ini, &mut data, &["=", "TreeStump"]).unwrap();
        parse_bool_field(
            &mut ini,
            &mut |value| data.kill_when_start_toppled = value,
            &["=", "yes"],
        )
        .unwrap();
        parse_percent_field(
            &mut ini,
            &mut |value| data.initial_velocity_percent = value,
            &["=", "55%"],
        )
        .unwrap();
        parse_fx_list(&mut data.bounce_fx, &["=", "NONE"]).unwrap();

        assert_eq!(data.stump_name.as_str(), "TreeStump");
        assert!(data.kill_when_start_toppled);
        assert!((data.initial_velocity_percent - 0.55).abs() < Real::EPSILON);
        assert!(data.bounce_fx.is_none());
    }

    #[test]
    fn topple_update_rejects_missing_values_like_cpp_parsers() {
        let mut ini = INI::new();
        let mut value = false;
        let mut percent = 0.0;
        let mut fx = None;

        assert!(matches!(
            parse_bool_field(&mut ini, &mut |parsed| value = parsed, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_percent_field(&mut ini, &mut |parsed| percent = parsed, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_fx_list(&mut fx, &["="]),
            Err(INIError::InvalidData)
        ));

        assert!(!value);
        assert_eq!(percent, 0.0);
    }
}

pub struct ToppleUpdateFactory;
impl ToppleUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(ToppleUpdate::new(thing, module_data)?))
    }
}

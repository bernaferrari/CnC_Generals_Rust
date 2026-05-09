// NeutronMissileUpdate - Implementation of missile behavior
// Author: Michael S. Booth, December 2001
// Ported to Rust

use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::drawable::DrawableArcExt;
use crate::player::ThePlayerList;
use crate::prelude::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};

const STRAIGHT_DOWN_SLOW_FACTOR: f32 = 0.5;

#[derive(Debug, Clone)]
pub struct NeutronMissileUpdateModuleData {
    pub base: BehaviorModuleData,
    pub initial_dist: f32,
    pub max_turn_rate: f32,
    pub forward_damping: f32,
    pub relative_speed: f32,
    pub target_from_directly_above: f32,
    pub ignition_fx: Option<FXListId>,
    pub launch_fx: Option<FXListId>,
    pub special_accel_factor: f32,
    pub special_speed_time: u32,
    pub special_speed_height: f32,
    pub special_jitter_distance: f32,
    pub delivery_decal_radius: f32,
    pub delivery_decal_template: RadiusDecalTemplate,
}

impl Default for NeutronMissileUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            initial_dist: 0.0,
            max_turn_rate: 999.0,
            forward_damping: 0.0,
            relative_speed: 1.0,
            target_from_directly_above: 0.0,
            ignition_fx: None,
            launch_fx: None,
            special_accel_factor: 1.0,
            special_speed_time: 0,
            special_speed_height: 0.0,
            special_jitter_distance: 0.0,
            delivery_decal_radius: 0.0,
            delivery_decal_template: RadiusDecalTemplate::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(NeutronMissileUpdateModuleData, base);

impl NeutronMissileUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, NEUTRON_MISSILE_UPDATE_FIELDS)
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_angular_velocity(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_angular_velocity_real(token)
}

fn parse_fx_list_id(tokens: &[&str]) -> Result<Option<FXListId>, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        return Ok(None);
    }
    Ok(Some(name_key_generate(token) as FXListId))
}

fn parse_radius_decal_texture(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.texture_name = AsciiString::from(*token);
    Ok(())
}

fn parse_radius_decal_style(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.shadow_type = INI::parse_bit_string_32(tokens, &SHADOW_NAMES)?;
    Ok(())
}

fn parse_radius_decal_opacity_min(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.min_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.min_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_max(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.max_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.max_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_throb_time(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.opacity_throb_time = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_radius_decal_color(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens.len() == 1 {
        if let Ok(value) = tokens[0].parse::<u32>() {
            data.color = value;
            return Ok(());
        }
    }

    let mut r: u8 = 0;
    let mut g: u8 = 0;
    let mut b: u8 = 0;
    let mut a: u8 = 255;

    for token in tokens {
        let (key, value) = match token.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => ("", token.trim()),
        };
        let parsed = value.parse::<u8>().map_err(|_| INIError::InvalidData)?;
        match key.to_ascii_uppercase().as_str() {
            "R" => r = parsed,
            "G" => g = parsed,
            "B" => b = parsed,
            "A" => a = parsed,
            _ => {}
        }
    }

    data.color = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
    Ok(())
}

fn parse_radius_decal_only_visible(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.only_visible_to_owning_player = INI::parse_bool(token)?;
    Ok(())
}

const RADIUS_DECAL_TEMPLATE_FIELDS: &[FieldParse<RadiusDecalTemplate>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_radius_decal_texture,
    },
    FieldParse {
        token: "Style",
        parse: parse_radius_decal_style,
    },
    FieldParse {
        token: "OpacityMin",
        parse: parse_radius_decal_opacity_min,
    },
    FieldParse {
        token: "OpacityMax",
        parse: parse_radius_decal_opacity_max,
    },
    FieldParse {
        token: "OpacityThrobTime",
        parse: parse_radius_decal_opacity_throb_time,
    },
    FieldParse {
        token: "Color",
        parse: parse_radius_decal_color,
    },
    FieldParse {
        token: "OnlyVisibleToOwningPlayer",
        parse: parse_radius_decal_only_visible,
    },
];

fn parse_delivery_decal(
    ini: &mut INI,
    data: &mut NeutronMissileUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(
        &mut data.delivery_decal_template,
        RADIUS_DECAL_TEMPLATE_FIELDS,
    )
}

const NEUTRON_MISSILE_UPDATE_FIELDS: &[FieldParse<NeutronMissileUpdateModuleData>] = &[
    FieldParse {
        token: "DistanceToTravelBeforeTurning",
        parse: |_, data, tokens| {
            data.initial_dist = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MaxTurnRate",
        parse: |_, data, tokens| {
            data.max_turn_rate = parse_angular_velocity(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ForwardDamping",
        parse: |_, data, tokens| {
            data.forward_damping = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "RelativeSpeed",
        parse: |_, data, tokens| {
            data.relative_speed = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TargetFromDirectlyAbove",
        parse: |_, data, tokens| {
            data.target_from_directly_above = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "LaunchFX",
        parse: |_, data, tokens| {
            data.launch_fx = parse_fx_list_id(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SpecialSpeedTime",
        parse: |_, data, tokens| {
            data.special_speed_time = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SpecialSpeedHeight",
        parse: |_, data, tokens| {
            data.special_speed_height = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SpecialAccelFactor",
        parse: |_, data, tokens| {
            data.special_accel_factor = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SpecialJitterDistance",
        parse: |_, data, tokens| {
            data.special_jitter_distance = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "IgnitionFX",
        parse: |_, data, tokens| {
            data.ignition_fx = parse_fx_list_id(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DeliveryDecal",
        parse: parse_delivery_decal,
    },
    FieldParse {
        token: "DeliveryDecalRadius",
        parse: |_, data, tokens| {
            data.delivery_decal_radius = parse_real(tokens)?;
            Ok(())
        },
    },
];

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissileStateType {
    PreLaunch,
    Launch,
    Attack,
    Dead,
}

#[derive(Debug, Clone)]
pub struct NeutronMissileUpdate {
    thing: ThingId,
    module_data: NeutronMissileUpdateModuleData,
    module_name_key: NameKeyType,
    state: MissileStateType,
    next_call_frame_and_phase: UnsignedInt,
    target_pos: Coord3D,
    intermed_pos: Coord3D,
    launcher_id: ObjectId,
    attach_wslot: WeaponSlotType,
    attach_specific_barrel_to_use: i32,
    accel: Coord3D,
    vel: Coord3D,
    state_timestamp: u32,
    is_launched: bool,
    is_armed: bool,
    no_turn_dist_left: f32,
    reached_intermediate_pos: bool,
    frame_at_launch: u32,
    height_at_launch: f32,
    exhaust_sys_tmpl: Option<AsciiString>,
    delivery_decal: RadiusDecal,
}

impl NeutronMissileUpdate {
    pub fn new(
        thing: ThingId,
        module_data: NeutronMissileUpdateModuleData,
        module_name: &AsciiString,
    ) -> Self {
        Self {
            thing,
            module_name_key: NameKeyGenerator::name_to_key(module_name.as_str()),
            no_turn_dist_left: module_data.initial_dist,
            reached_intermediate_pos: true,
            module_data,
            target_pos: Coord3D::origin(),
            intermed_pos: Coord3D::origin(),
            accel: Coord3D::origin(),
            vel: Coord3D::origin(),
            state: MissileStateType::PreLaunch,
            next_call_frame_and_phase: 0,
            state_timestamp: TheGameLogic::get_frame(),
            is_armed: false,
            is_launched: false,
            launcher_id: INVALID_OBJECT_ID,
            attach_wslot: WeaponSlotType::Primary,
            attach_specific_barrel_to_use: 0,
            height_at_launch: 0.0,
            frame_at_launch: 0,
            exhaust_sys_tmpl: None,
            delivery_decal: RadiusDecal::new(Coord3D::origin(), 0.0),
        }
    }

    pub fn projectile_launch_at_object_or_position(
        &mut self,
        victim: Option<&Object>,
        victim_pos: Option<&Coord3D>,
        launcher: Option<&Object>,
        wslot: WeaponSlotType,
        specific_barrel_to_use: i32,
        det_weap: Option<&WeaponTemplate>,
        exhaust_sys_override: Option<AsciiString>,
    ) {
        debug_assert!(specific_barrel_to_use >= 0);

        self.launcher_id = launcher.map(|l| l.id()).unwrap_or(INVALID_OBJECT_ID);
        self.attach_wslot = wslot;
        self.attach_specific_barrel_to_use = specific_barrel_to_use;

        self.vel = Coord3D::origin();
        if let Some(launcher_obj) = launcher {
            if let Some(phys) = launcher_obj.get_physics() {
                self.vel = phys.get_velocity();
            }
        }

        self.projectile_fire_at_object_or_position(
            victim,
            victim_pos,
            det_weap,
            exhaust_sys_override,
        );
    }

    fn projectile_fire_at_object_or_position(
        &mut self,
        victim: Option<&Object>,
        victim_pos: Option<&Coord3D>,
        _det_weap: Option<&WeaponTemplate>,
        exhaust_sys_override: Option<AsciiString>,
    ) {
        self.exhaust_sys_tmpl = exhaust_sys_override;

        self.state = MissileStateType::Launch;
        self.state_timestamp = TheGameLogic::get_frame();

        if let Some(victim_obj) = victim {
            self.target_pos = *victim_obj.get_position();
            self.intermed_pos = self.target_pos;
            self.intermed_pos.z += self.module_data.target_from_directly_above;
        } else if let Some(pos) = victim_pos {
            self.target_pos = *pos;
            self.intermed_pos = self.target_pos;
            self.intermed_pos.z += self.module_data.target_from_directly_above;
        }

        // Create delivery decal (radius override matches C++)
        self.delivery_decal.clear();
        let owner_index = TheGameLogic::find_object_by_id(self.thing).and_then(|obj_arc| {
            let obj = obj_arc.read().ok()?;
            let player = obj.get_controlling_player()?;
            let player_guard = player.read().ok()?;
            Some(player_guard.get_player_index())
        });
        let local_index = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index());
        let allow_decal = if self
            .module_data
            .delivery_decal_template
            .only_visible_to_owning_player
        {
            matches!((local_index, owner_index), (Some(local), Some(owner)) if local == owner)
        } else {
            true
        };
        if allow_decal {
            let mut decal = self
                .module_data
                .delivery_decal_template
                .create_radius_decal_with_radius(
                    self.target_pos,
                    self.module_data.delivery_decal_radius,
                );
            if !decal.is_empty() {
                if self.module_data.delivery_decal_template.color == 0 {
                    if let (Some(owner), Ok(list)) = (owner_index, ThePlayerList().read()) {
                        if let Some(player) = list.get_player(owner).and_then(|p| p.read().ok()) {
                            decal.color = player.get_player_color().to_argb_u32();
                        }
                    }
                }
                self.delivery_decal = decal;
            }
        }
    }

    fn do_launch(&mut self, ctx: &mut UpdateContext<'_>) {
        if !self.is_launched {
            let Some(launcher) = ctx.game_logic.find_object(self.launcher_id) else {
                // If our launch vehicle is gone, destroy ourselves
                self.launcher_id = INVALID_OBJECT_ID;
                if let Some(object) = ctx.game_logic.find_object(self.thing) {
                    ctx.game_logic.destroy_object(object.id());
                }
                return;
            };

            // Get launch offset from drawable
            let attach_transform = if let Some(drawable) = launcher.get_drawable() {
                let launch = drawable.get_projectile_launch_offset(
                    self.attach_wslot,
                    self.attach_specific_barrel_to_use,
                    TurretType::Invalid,
                );
                if launch.is_none() {
                    log::warn!(
                        "ProjectileLaunchPos {:?} {} not found for launcher {}",
                        self.attach_wslot,
                        self.attach_specific_barrel_to_use,
                        launcher.get_id()
                    );
                    debug_assert!(
                        false,
                        "ProjectileLaunchPos {:?} {} not found for launcher {}",
                        self.attach_wslot,
                        self.attach_specific_barrel_to_use,
                        launcher.get_id()
                    );
                }
                launch
                    .map(|launch| launch.transform)
                    .unwrap_or(Matrix3D::IDENTITY)
            } else {
                Matrix3D::IDENTITY
            };

            let world_transform =
                launcher.convert_bone_pos_to_world_pos(None, Some(&attach_transform));

            let translation = world_transform.w_axis;
            let world_pos = Coord3D {
                x: translation.x,
                y: translation.y,
                z: translation.z,
            };

            // Rotate the missile on end (45 degrees adjustment)
            let adjusted_transform =
                world_transform * Matrix3D::from_rotation_x(std::f32::consts::PI / 2.0);

            if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
                if let Some(drawable) = object.get_drawable() {
                    drawable.set_drawable_hidden(false);
                }
                object.set_transform_matrix(&adjusted_transform);
                if let Err(err) = object.set_position(&world_pos) {
                    log::debug!("NeutronMissileUpdate::do_launch set_position failed: {err}");
                }

                if let Some(tracker) = object.get_experience_tracker() {
                    tracker.set_experience_sink(self.launcher_id);
                }
            }

            self.is_launched = true;

            if self.module_data.target_from_directly_above > 0.0 {
                self.reached_intermediate_pos = false;
            }

            // Do launch FX
            if let Some(fx) = self.module_data.launch_fx {
                if let Some(fx_list) = ctx.fx_list {
                    fx_list.do_fx_obj(fx, self.thing);
                }
            }

            if let Some(object) = ctx.game_logic.find_object(self.thing) {
                self.height_at_launch = object.get_position().z;
                self.frame_at_launch = ctx.game_logic.get_frame();
            }
        }

        // Fall
        if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
            let mut pos = *object.get_position();
            pos.x += self.vel.x;
            pos.y += self.vel.y;
            pos.z += self.vel.z;
            if let Err(err) = object.set_position(&pos) {
                log::debug!("NeutronMissileUpdate::do_launch drift set_position failed: {err}");
            }
        }

        // Do ignition FX
        if let Some(fx) = self.module_data.ignition_fx {
            if let Some(fx_list) = ctx.fx_list {
                fx_list.do_fx_obj(fx, self.thing);
            }
        }

        if let Some(exhaust_name) = self.exhaust_sys_tmpl.as_ref() {
            if let Some(mgr) = ctx.particle_system_manager.as_mut() {
                if let Some(template_id) = mgr.find_template(exhaust_name.as_str()) {
                    mgr.create_attached_particle_system_id(template_id, self.thing);
                }
            }
        }

        self.state = MissileStateType::Attack;
        self.state_timestamp = ctx.game_logic.get_frame();

        // Arm the missile's warhead
        self.is_armed = true;
    }

    fn do_attack(&mut self, ctx: &mut UpdateContext<'_>) {
        // Get frame now before mutable borrow
        let now = ctx.game_logic.get_frame();

        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        let mut speed = self.module_data.relative_speed;

        if self.module_data.target_from_directly_above > 0.0 && self.reached_intermediate_pos {
            speed *= STRAIGHT_DOWN_SLOW_FACTOR;
        }

        // Orient toward destination
        let target_pos = if self.reached_intermediate_pos {
            &self.target_pos
        } else {
            &self.intermed_pos
        };

        let mx = if self.no_turn_dist_left > 0.0 {
            object.get_transform_matrix()
        } else {
            calc_transform(object, target_pos, self.module_data.max_turn_rate)
        };

        // Get true forward direction of missile
        let true_dir = mx.x_axis.truncate().normalize();

        // Move forward along forward direction
        let damping = self.module_data.forward_damping;
        self.accel.x = speed * true_dir.x - damping * self.vel.x;
        self.accel.y = speed * true_dir.y - damping * self.vel.y;
        self.accel.z = speed * true_dir.z - damping * self.vel.z;

        self.vel.x += self.accel.x;
        self.vel.y += self.accel.y;
        self.vel.z += self.accel.z;

        let mut pos = *object.get_position();

        // Handle special speed/height logic
        if self.module_data.special_speed_time > 0
            && now <= self.frame_at_launch + self.module_data.special_speed_time
        {
            if let Some(drawable) = object.get_drawable() {
                drawable.set_instance_matrix(None);
            }

            let elapsed = now - self.frame_at_launch;
            if elapsed < self.module_data.special_speed_time {
                let time_frac = elapsed as f32 / self.module_data.special_speed_time as f32;
                let mut accel_factor = self.module_data.special_accel_factor;
                if accel_factor < 0.01 {
                    accel_factor = 0.01;
                }

                let mut new_pos = pos;
                new_pos.z = self.height_at_launch
                    + (accel_factor * time_frac.powi(2)) * self.module_data.special_speed_height;

                self.vel.x = new_pos.x - pos.x;
                self.vel.y = new_pos.y - pos.y;
                self.vel.z = new_pos.z - pos.z;

                // Handle jitter
                if self.module_data.special_jitter_distance > 0.0 {
                    let amplitude = (1.0 - time_frac) * self.module_data.special_jitter_distance;
                    let jitter = Vector3 {
                        x: 0.0,
                        y: GameLogicRandomValueReal(-1.0, 1.0) * amplitude,
                        z: GameLogicRandomValueReal(-1.0, 1.0) * amplitude,
                    };
                    let rotated_jitter = mx.transform_vector3(jitter);
                    let jitter_matrix = Matrix3D::from_translation(rotated_jitter);

                    if let Some(drawable) = object.get_drawable() {
                        drawable.set_instance_matrix(Some(&jitter_matrix));
                    }
                }
            }
        }

        pos.x += self.vel.x;
        pos.y += self.vel.y;
        pos.z += self.vel.z;

        object.set_transform_matrix(&mx);
        if let Err(err) = object.set_position(&pos) {
            log::debug!("NeutronMissileUpdate::do_attack set_position failed: {err}");
        }
    }

    pub fn projectile_handle_collision(
        &mut self,
        other: Option<ObjectId>,
        ctx: &mut UpdateContext<'_>,
    ) -> bool {
        // Check if warhead is armed
        if !self.projectile_is_armed() {
            return true;
        }

        // Don't hit your own launcher
        if let Some(other_id) = other {
            if other_id == self.launcher_id {
                return true;
            }
        }

        // Collided - blow up!
        self.detonate(ctx);

        // Mark as no collisions (since we might still exist in slow death mode)
        if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
            object.set_status(ObjectStatus::NoCollisions.into(), true);
        }

        true
    }

    pub fn on_die(&mut self, _damage_info: &DamageInfo) {
        self.delivery_decal.clear();
    }

    fn detonate(&mut self, ctx: &mut UpdateContext<'_>) {
        self.delivery_decal.clear();

        if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
            object.kill(None, None);
            self.state = MissileStateType::Dead;

            if let Some(drawable) = object.get_drawable() {
                drawable.set_drawable_hidden(true);
            }
        }
    }

    fn projectile_is_armed(&self) -> bool {
        self.is_armed
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        self.delivery_decal.update();

        if !self.reached_intermediate_pos {
            if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
                let dist_sqr = ctx.partition_manager.get_distance_squared_to_pos(
                    &object,
                    &self.intermed_pos,
                    PartitionDistanceType::Center3D,
                );
                let bound_sqr = object
                    .get_geometry_info()
                    .get_bounding_sphere_radius()
                    .powi(2);

                if dist_sqr <= bound_sqr {
                    self.reached_intermediate_pos = true;
                    if let Err(err) = object.set_position(&self.intermed_pos) {
                        log::debug!(
                            "NeutronMissileUpdate::update intermediate set_position failed: {err}"
                        );
                    }

                    let vel_len = self.vel.length();
                    self.vel.x = 0.0;
                    self.vel.y = 0.0;
                    self.vel.z = -vel_len * STRAIGHT_DOWN_SLOW_FACTOR;
                }
            }
        }

        let old_pos = if let Some(object) = ctx.game_logic.find_object(self.thing) {
            *object.get_position()
        } else {
            Coord3D::origin()
        };

        let old_pos_valid = self.state == MissileStateType::Attack;

        match self.state {
            MissileStateType::PreLaunch => {}
            MissileStateType::Launch => self.do_launch(ctx),
            MissileStateType::Attack => self.do_attack(ctx),
            MissileStateType::Dead => {}
        }

        if self.no_turn_dist_left > 0.0 && old_pos_valid {
            if let Some(object) = ctx.game_logic.find_object(self.thing) {
                let new_pos = object.get_position();
                let dist_this_turn = ((new_pos.x - old_pos.x).powi(2)
                    + (new_pos.y - old_pos.y).powi(2)
                    + (new_pos.z - old_pos.z).powi(2))
                .sqrt();
                self.no_turn_dist_left -= dist_this_turn;
            }
        }

        // Check if hit terrain
        if self.state != MissileStateType::PreLaunch
            && self.state != MissileStateType::Dead
            && !self.is_above_terrain(ctx)
        {
            let normal = Coord3D {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            };

            if let Some(object) = ctx.game_logic.find_object_mut(self.thing) {
                let pos = *object.get_position();
                object.on_collide(None, &pos, &normal);
            }
        }

        UpdateSleepTime::None
    }

    fn is_above_terrain(&self, ctx: &UpdateContext<'_>) -> bool {
        if let Some(object) = ctx.game_logic.find_object(self.thing) {
            object.is_above_terrain()
        } else {
            true
        }
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("NeutronMissileUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .expect("NeutronMissileUpdate::save failed to xfer UpdateModule base state");
        let mut state_val = self.state as u32;
        xfer_io(xfer.xfer_u32(&mut state_val), "state");
        let mut target_pos = self.target_pos;
        xfer.xfer_coord3d(&mut target_pos);
        let mut intermed_pos = self.intermed_pos;
        xfer.xfer_coord3d(&mut intermed_pos);
        let mut launcher_id = self.launcher_id;
        xfer_io(xfer.xfer_object_id(&mut launcher_id), "launcher_id");
        let mut wslot_val = self.attach_wslot as u32;
        xfer_io(xfer.xfer_u32(&mut wslot_val), "attach_wslot");
        let mut barrel = self.attach_specific_barrel_to_use;
        xfer_io(xfer.xfer_i32(&mut barrel), "attach_specific_barrel_to_use");
        let mut accel = self.accel;
        xfer.xfer_coord3d(&mut accel);
        let mut vel = self.vel;
        xfer.xfer_coord3d(&mut vel);
        let mut state_timestamp = self.state_timestamp;
        xfer_io(xfer.xfer_u32(&mut state_timestamp), "state_timestamp");
        let mut is_launched = self.is_launched;
        xfer_io(xfer.xfer_bool(&mut is_launched), "is_launched");
        let mut is_armed = self.is_armed;
        xfer_io(xfer.xfer_bool(&mut is_armed), "is_armed");
        let mut no_turn = self.no_turn_dist_left;
        xfer_io(xfer.xfer_f32(&mut no_turn), "no_turn_dist_left");
        let mut reached_intermediate = self.reached_intermediate_pos;
        xfer_io(
            xfer.xfer_bool(&mut reached_intermediate),
            "reached_intermediate_pos",
        );
        let mut frame_at_launch = self.frame_at_launch;
        xfer_io(xfer.xfer_u32(&mut frame_at_launch), "frame_at_launch");
        let mut height_at_launch = self.height_at_launch;
        xfer_io(xfer.xfer_f32(&mut height_at_launch), "height_at_launch");

        xfer.xfer_radius_decal(&self.delivery_decal);

        // Exhaust system template name
        let mut name = self
            .exhaust_sys_tmpl
            .as_ref()
            .map(|tmpl| tmpl.to_string())
            .unwrap_or_default();
        xfer_io(xfer.xfer_string(&mut name), "exhaust_sys_tmpl");
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("NeutronMissileUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
                .expect("NeutronMissileUpdate::load failed to xfer UpdateModule base state");

            let mut state_val = 0u32;
            xfer_io(xfer.xfer_u32(&mut state_val), "state");
            self.state = match state_val {
                0 => MissileStateType::PreLaunch,
                1 => MissileStateType::Launch,
                2 => MissileStateType::Attack,
                3 => MissileStateType::Dead,
                _ => MissileStateType::PreLaunch,
            };

            xfer.xfer_coord3d(&mut self.target_pos);
            xfer.xfer_coord3d(&mut self.intermed_pos);
            xfer_io(xfer.xfer_object_id(&mut self.launcher_id), "launcher_id");

            let mut wslot_val = 0u32;
            xfer_io(xfer.xfer_u32(&mut wslot_val), "attach_wslot");
            self.attach_wslot =
                WeaponSlotType::from_u32(wslot_val).unwrap_or(WeaponSlotType::Primary);

            xfer_io(
                xfer.xfer_i32(&mut self.attach_specific_barrel_to_use),
                "attach_specific_barrel_to_use",
            );
            xfer.xfer_coord3d(&mut self.accel);
            xfer.xfer_coord3d(&mut self.vel);
            xfer_io(xfer.xfer_u32(&mut self.state_timestamp), "state_timestamp");
            xfer_io(xfer.xfer_bool(&mut self.is_launched), "is_launched");
            xfer_io(xfer.xfer_bool(&mut self.is_armed), "is_armed");
            xfer_io(
                xfer.xfer_f32(&mut self.no_turn_dist_left),
                "no_turn_dist_left",
            );
            xfer_io(
                xfer.xfer_bool(&mut self.reached_intermediate_pos),
                "reached_intermediate_pos",
            );
            xfer_io(xfer.xfer_u32(&mut self.frame_at_launch), "frame_at_launch");
            xfer_io(
                xfer.xfer_f32(&mut self.height_at_launch),
                "height_at_launch",
            );

            xfer.xfer_radius_decal_mut(&mut self.delivery_decal);

            // Load exhaust system template name
            let mut name = String::new();
            xfer_io(xfer.xfer_string(&mut name), "exhaust_sys_tmpl");
            self.exhaust_sys_tmpl = if name.is_empty() {
                None
            } else {
                Some(AsciiString::from(name.as_str()))
            };
        }
    }
}

impl Snapshotable for NeutronMissileUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("NeutronMissileUpdate xfer version: {e:?}"))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        let mut state_val = self.state as u32;
        xfer.xfer_u32(&mut state_val)
            .map_err(|e| format!("NeutronMissileUpdate xfer state: {e:?}"))?;
        self.state = match state_val {
            0 => MissileStateType::PreLaunch,
            1 => MissileStateType::Launch,
            2 => MissileStateType::Attack,
            3 => MissileStateType::Dead,
            _ => MissileStateType::PreLaunch,
        };

        xfer.xfer_coord3d(&mut self.target_pos);
        xfer.xfer_coord3d(&mut self.intermed_pos);
        xfer.xfer_object_id(&mut self.launcher_id)
            .map_err(|e| format!("NeutronMissileUpdate xfer launcher id: {e:?}"))?;

        let mut wslot_val = self.attach_wslot as u32;
        xfer.xfer_u32(&mut wslot_val)
            .map_err(|e| format!("NeutronMissileUpdate xfer weapon slot: {e:?}"))?;
        self.attach_wslot = WeaponSlotType::from_u32(wslot_val).unwrap_or(WeaponSlotType::Primary);

        xfer.xfer_i32(&mut self.attach_specific_barrel_to_use)
            .map_err(|e| format!("NeutronMissileUpdate xfer barrel: {e:?}"))?;
        xfer.xfer_coord3d(&mut self.accel);
        xfer.xfer_coord3d(&mut self.vel);
        xfer.xfer_u32(&mut self.state_timestamp)
            .map_err(|e| format!("NeutronMissileUpdate xfer state timestamp: {e:?}"))?;
        xfer.xfer_bool(&mut self.is_launched)
            .map_err(|e| format!("NeutronMissileUpdate xfer launched flag: {e:?}"))?;
        xfer.xfer_bool(&mut self.is_armed)
            .map_err(|e| format!("NeutronMissileUpdate xfer armed flag: {e:?}"))?;
        xfer.xfer_f32(&mut self.no_turn_dist_left)
            .map_err(|e| format!("NeutronMissileUpdate xfer no-turn distance: {e:?}"))?;
        xfer.xfer_bool(&mut self.reached_intermediate_pos)
            .map_err(|e| format!("NeutronMissileUpdate xfer intermediate flag: {e:?}"))?;
        xfer.xfer_u32(&mut self.frame_at_launch)
            .map_err(|e| format!("NeutronMissileUpdate xfer launch frame: {e:?}"))?;
        xfer.xfer_f32(&mut self.height_at_launch)
            .map_err(|e| format!("NeutronMissileUpdate xfer launch height: {e:?}"))?;
        xfer.xfer_radius_decal_mut(&mut self.delivery_decal);

        let mut name = self
            .exhaust_sys_tmpl
            .as_ref()
            .map(|tmpl| tmpl.to_string())
            .unwrap_or_default();
        xfer.xfer_string(&mut name)
            .map_err(|e| format!("NeutronMissileUpdate xfer exhaust template: {e:?}"))?;
        self.exhaust_sys_tmpl = if name.is_empty() {
            None
        } else {
            Some(AsciiString::from(name.as_str()))
        };

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for NeutronMissileUpdate {
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
        EngineModuleData::get_module_tag_name_key(&self.module_data)
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        &self.module_data
    }
}

fn calc_transform(obj: &Object, pos: &Coord3D, max_turn_rate: f32) -> Matrix3D {
    // Convert to Vector3
    let obj_pos = Vector3 {
        x: obj.get_position().x,
        y: obj.get_position().y,
        z: obj.get_position().z,
    };
    let other_pos = Vector3 {
        x: pos.x,
        y: pos.y,
        z: pos.z,
    };

    let obj_dir = obj.get_transform_matrix().transform_vector3(Vector3::X);
    let mut other_dir = other_pos - obj_pos;
    other_dir = other_dir.normalize();

    // Dot of two unit vectors is cos of angle between them
    let c = obj_dir.dot(other_dir).clamp(-1.0, 1.0);
    let angle = c.acos();

    let new_dir = if angle.abs() < max_turn_rate {
        other_dir
    } else {
        // Turn as much as we can
        let obj_cross_other = obj_dir.cross(other_dir).normalize();
        let rot_mtx = Matrix3D::from_axis_angle(obj_cross_other, max_turn_rate);
        rot_mtx.transform_vector3(obj_dir)
    };

    // Build transform matrix from position and direction
    // Create basis vectors: X (forward), Y (up), Z (right)
    let x_axis = new_dir.normalize();

    // Choose an up vector that's not parallel to the forward direction
    let up = if x_axis.y.abs() < 0.999 {
        Vector3::Y
    } else {
        Vector3::Z
    };

    // Construct orthonormal basis
    let z_axis = x_axis.cross(up).normalize();
    let y_axis = z_axis.cross(x_axis).normalize();

    Matrix3D::from_cols(
        x_axis.extend(0.0),
        y_axis.extend(0.0),
        z_axis.extend(0.0),
        obj_pos.extend(1.0),
    )
}

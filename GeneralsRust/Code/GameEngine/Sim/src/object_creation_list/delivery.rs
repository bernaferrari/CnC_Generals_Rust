use crate::weapon::WeaponSlotType;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::thing::radius_decal_data::RadiusDecalTemplate;
use glam::Vec3;
use std::f32::consts::PI;

/// Delivery data passed to DeliverPayloadAIUpdate.
#[derive(Debug, Clone)]
pub struct DeliverPayloadData<Fx = String, Weapon = String> {
    pub visible_drop_bone_name: AsciiString,
    pub visible_sub_object_name: AsciiString,
    pub visible_payload_template_name: AsciiString,
    pub dist_to_target: f32,
    pub pre_open_distance: f32,
    pub max_attempts: i32,
    pub drop_offset: Vec3,
    pub drop_variance: Vec3,
    pub drop_delay: u32,
    pub fire_weapon: bool,
    pub self_destruct_object: bool,
    pub visible_num_bones: i32,
    pub dive_start_distance: f32,
    pub dive_end_distance: f32,
    pub strafing_weapon_slot: Option<WeaponSlotType>,
    pub visible_items_dropped_per_interval: i32,
    pub inherit_transport_velocity: bool,
    pub is_parachute_directly: bool,
    pub exit_pitch_rate: f32,
    pub strafe_fx: Option<Fx>,
    pub strafe_length: f32,
    pub visible_payload_weapon_template: Option<Weapon>,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: f32,
}

impl<Fx, Weapon> DeliverPayloadData<Fx, Weapon> {
    /// Bind authored references for a runtime that uses resolved asset handles.
    pub fn bind_assets<BoundFx, BoundWeapon>(
        self,
        resolve_fx: impl FnOnce(Fx) -> Option<BoundFx>,
        resolve_weapon: impl FnOnce(Weapon) -> Option<BoundWeapon>,
    ) -> DeliverPayloadData<BoundFx, BoundWeapon> {
        DeliverPayloadData {
            visible_drop_bone_name: self.visible_drop_bone_name,
            visible_sub_object_name: self.visible_sub_object_name,
            visible_payload_template_name: self.visible_payload_template_name,
            dist_to_target: self.dist_to_target,
            pre_open_distance: self.pre_open_distance,
            max_attempts: self.max_attempts,
            drop_offset: self.drop_offset,
            drop_variance: self.drop_variance,
            drop_delay: self.drop_delay,
            fire_weapon: self.fire_weapon,
            self_destruct_object: self.self_destruct_object,
            visible_num_bones: self.visible_num_bones,
            dive_start_distance: self.dive_start_distance,
            dive_end_distance: self.dive_end_distance,
            strafing_weapon_slot: self.strafing_weapon_slot,
            visible_items_dropped_per_interval: self.visible_items_dropped_per_interval,
            inherit_transport_velocity: self.inherit_transport_velocity,
            is_parachute_directly: self.is_parachute_directly,
            exit_pitch_rate: self.exit_pitch_rate,
            strafe_fx: self.strafe_fx.and_then(resolve_fx),
            strafe_length: self.strafe_length,
            visible_payload_weapon_template: self
                .visible_payload_weapon_template
                .and_then(resolve_weapon),
            delivery_decal_template: self.delivery_decal_template,
            delivery_decal_radius: self.delivery_decal_radius,
        }
    }

    /// C++ drop world pose: current item pos + variance sample + dropOffset.
    /// `variance_sample` is already in [-drop_variance, +drop_variance] per axis
    /// (or zero when that axis has no variance).
    pub fn drop_world_pos(&self, base: Vec3, variance_sample: Vec3) -> Vec3 {
        let mut pos = base;
        if self.drop_variance.x > 0.0 {
            pos.x += variance_sample.x;
        }
        if self.drop_variance.y > 0.0 {
            pos.y += variance_sample.y;
        }
        if self.drop_variance.z > 0.0 {
            pos.z += variance_sample.z;
        }
        pos.x += self.drop_offset.x;
        pos.y += self.drop_offset.y;
        pos.z += self.drop_offset.z;
        pos
    }
}

impl<Fx, Weapon> Default for DeliverPayloadData<Fx, Weapon> {
    fn default() -> Self {
        Self {
            visible_drop_bone_name: AsciiString::new(),
            visible_sub_object_name: AsciiString::new(),
            visible_payload_template_name: AsciiString::new(),
            dist_to_target: 0.0,
            pre_open_distance: 0.0,
            max_attempts: 1,
            drop_offset: Vec3::ZERO,
            drop_variance: Vec3::ZERO,
            drop_delay: 0,
            fire_weapon: false,
            self_destruct_object: false,
            visible_num_bones: 0,
            dive_start_distance: 0.0,
            dive_end_distance: 0.0,
            strafing_weapon_slot: None,
            visible_items_dropped_per_interval: 0,
            inherit_transport_velocity: false,
            is_parachute_directly: false,
            exit_pitch_rate: 0.0,
            strafe_fx: None,
            strafe_length: 0.0,
            visible_payload_weapon_template: None,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

/// Payload information for delivery
/// Matches C++ Payload struct (ObjectCreationList.cpp:551-555)
#[derive(Debug, Clone)]
pub struct Payload {
    pub payload_name: String,
    pub payload_count: i32,
}

pub type DeliverPayloadNugget = DeliveryNugget<DeliverPayloadData>;

/// DeliverPayloadNugget - spawns transport aircraft with payload
/// Matches C++ DeliverPayloadNugget (ObjectCreationList.cpp:225-572)
///
/// Used for:
/// - A-10 Thunderbolt strikes
/// - Carpet bombing runs
/// - Paratroop drops
/// - Napalm strikes
/// - Fuel Air Bomb delivery
#[derive(Debug, Clone)]
pub struct DeliveryNugget<Data> {
    // Transport that carries payload
    pub transport_name: String,
    pub start_at_preferred_height: bool,
    pub start_at_max_speed: bool,

    // Formation parameters (for multiple transports)
    pub formation_size: u32,
    pub formation_spacing: f32,
    pub convergence_factor: f32, // 0.0 = spread, 1.0 = converge to same point
    pub error_radius: f32,       // Random targeting error
    pub delay_delivery_frames_max: u32,

    // Payload objects to deliver
    pub payload: Vec<Payload>,
    pub put_in_container_name: String,

    // AI delivery parameters
    pub data: Data,
}

impl<Data: Default> Default for DeliveryNugget<Data> {
    fn default() -> Self {
        Self {
            transport_name: String::new(),
            start_at_preferred_height: true,
            start_at_max_speed: false,
            formation_size: 1,
            formation_spacing: 25.0,
            convergence_factor: 0.0,
            error_radius: 0.0,
            delay_delivery_frames_max: 0,
            payload: Vec::new(),
            put_in_container_name: String::new(),
            data: Data::default(),
        }
    }
}

/// C++ DeliverPayloadNugget formation flight poses (start / moveTo / target / yaw).
#[derive(Debug, Clone, PartialEq)]
pub struct DeliverPayloadFormationPose {
    pub start_pos: Vec3,
    pub move_to_pos: Vec3,
    pub target_pos: Vec3,
    pub orient: f32,
}

impl<Data> DeliveryNugget<Data> {
    /// Calculate formation offset vectors (CCW and CW perpendicular to approach)
    /// Matches C++ ObjectCreationList.cpp:271-298
    pub fn calculate_formation_vectors(primary: &Vec3, secondary: &Vec3) -> (f32, f32, f32, f32) {
        let dx = primary.x - secondary.x;
        let dy = primary.y - secondary.y;

        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.001 {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let dx_norm = dx / length;
        let dy_norm = dy / length;

        // Rotate 90 degrees CCW
        let radians = 90.0 * PI / 180.0;
        let s = radians.sin();
        let c = radians.cos();
        let ccw_x = dx_norm * c + dy_norm * -s + dx_norm;
        let ccw_y = dx_norm * s + dy_norm * c + dy_norm;

        // Rotate 90 degrees CW
        let s = (-radians).sin();
        let c = (-radians).cos();
        let cw_x = dx_norm * c + dy_norm * -s + dx_norm;
        let cw_y = dx_norm * s + dy_norm * c + dy_norm;

        (ccw_x, ccw_y, cw_x, cw_y)
    }

    /// Calculate offset for formation member
    /// Matches C++ ObjectCreationList.cpp:303-319
    pub fn calculate_formation_offset(
        formation_index: i32,
        formation_size: i32,
        formation_spacing: f32,
        ccw_x: f32,
        ccw_y: f32,
        cw_x: f32,
        cw_y: f32,
    ) -> Vec3 {
        if formation_size <= 1 {
            return Vec3::new(0.0, 0.0, 0.0);
        }

        // ObjectCreationList.cpp assigns the product to i32 after the
        // member pair index is multiplied by authored spacing.
        let offset_multiplier =
            (((formation_index + 1) / 2) as f32 * formation_spacing) as i32 as f32;

        if formation_index % 2 == 1 {
            // Odd - use CCW
            Vec3::new(ccw_x * offset_multiplier, ccw_y * offset_multiplier, 0.0)
        } else {
            // Even - use CW
            Vec3::new(cw_x * offset_multiplier, cw_y * offset_multiplier, 0.0)
        }
    }

    /// Full C++ formation flight matrix for one transport:
    /// offset → start/moveTo/target, optional error radius, distToTarget slop.
    pub fn formation_flight_pose(
        primary: &Vec3,
        secondary: &Vec3,
        formation_index: i32,
        formation_size: i32,
        formation_spacing: f32,
        convergence_factor: f32,
        dist_to_target: f32,
        error_sample: Option<(f32, f32)>,
    ) -> DeliverPayloadFormationPose {
        let (ccw_x, ccw_y, cw_x, cw_y) = if formation_size > 1 {
            Self::calculate_formation_vectors(primary, secondary)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        let offset = Self::calculate_formation_offset(
            formation_index,
            formation_size,
            formation_spacing,
            ccw_x,
            ccw_y,
            cw_x,
            cw_y,
        );

        let mut start_pos = *primary;
        start_pos.x += offset.x;
        start_pos.y += offset.y;

        let mut move_to_pos = *secondary;
        move_to_pos.x += offset.x;
        move_to_pos.y += offset.y;

        let mut target_pos = *secondary;
        target_pos.x += offset.x * (1.0 - convergence_factor);
        target_pos.y += offset.y * (1.0 - convergence_factor);

        // First guy is always spot-on; later members may scatter by error radius.
        if formation_index > 0 {
            if let Some((random_radius, random_angle)) = error_sample {
                target_pos.x += random_radius * random_angle.cos();
                target_pos.y += random_radius * random_angle.sin();
            }
        }

        let orient = (move_to_pos.y - start_pos.y).atan2(move_to_pos.x - start_pos.x);
        if dist_to_target > 0.0 {
            const SLOP: f32 = 1.5;
            start_pos.x -= orient.cos() * dist_to_target * SLOP;
            start_pos.y -= orient.sin() * dist_to_target * SLOP;
        }

        DeliverPayloadFormationPose {
            start_pos,
            move_to_pos,
            target_pos,
            orient,
        }
    }

    pub fn map_data<Bound>(self, bind: impl FnOnce(Data) -> Bound) -> DeliveryNugget<Bound> {
        DeliveryNugget {
            transport_name: self.transport_name,
            start_at_preferred_height: self.start_at_preferred_height,
            start_at_max_speed: self.start_at_max_speed,
            formation_size: self.formation_size,
            formation_spacing: self.formation_spacing,
            convergence_factor: self.convergence_factor,
            error_radius: self.error_radius,
            delay_delivery_frames_max: self.delay_delivery_frames_max,
            payload: self.payload,
            put_in_container_name: self.put_in_container_name,
            data: bind(self.data),
        }
    }
}

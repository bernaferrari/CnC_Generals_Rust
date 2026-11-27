use super::calc_random_force_with;
use crate::debris::DebrisDisposition;
use game_engine::common::ini::StaticGameLODLevel;
use glam::Vec3;
use std::f32::consts::PI;

/// Animation set for debris (initial, flying, final)
/// Matches C++ AnimSet struct (ObjectCreationList.cpp:1423-1428)
#[derive(Debug, Clone)]
pub struct AnimSet {
    pub anim_initial: String,
    pub anim_flying: String,
    pub anim_final: String,
}

/// Generic object/debris creation nugget
/// Matches C++ GenericObjectCreationNugget (ObjectCreationList.cpp:711-1475)
#[derive(Debug, Clone)]
pub struct GenericObjectCreationNugget {
    // Object names or model names to create
    pub names: Vec<String>,
    pub name_are_objects: bool, // true = object templates, false = debris models

    // Basic creation parameters
    pub debris_to_generate: i32,
    pub offset: Vec3,
    pub disposition: DebrisDisposition,
    pub disposition_intensity: f32,

    // Physics parameters (for debris)
    pub mass: f32,
    pub extra_bounciness: f32,
    pub extra_friction: f32,

    // Force parameters (for RANDOM_FORCE disposition)
    pub min_mag: f32,
    pub max_mag: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,

    // Spin rates (angular velocity)
    pub spin_rate: f32,  // -1.0 = calculate from intensity
    pub yaw_rate: f32,   // -1.0 = use spin_rate
    pub roll_rate: f32,  // -1.0 = use spin_rate
    pub pitch_rate: f32, // -1.0 = use spin_rate

    // Lifetime
    pub min_frames: u32,
    pub max_frames: u32,

    // Health range
    pub min_health: f32, // 0.0-1.0 (percentage)
    pub max_health: f32,

    // Advanced options
    pub inherit_veterancy: bool,
    pub ignore_primary_obstacle: bool,
    pub skip_if_significantly_airborne: bool,
    pub invulnerable_time: u32,
    pub contain_inside_source_object: bool,
    pub dies_on_bad_land: bool,
    pub requires_live_player: bool,
    pub preserve_layer: bool,
    pub orient_in_force_direction: bool,

    // Spread formation
    pub spread_formation: bool,
    pub min_distance_a_formation: f32,
    pub min_distance_b_formation: f32,
    pub max_distance_formation: f32,

    // Fade in/out
    pub fade_in: bool,
    pub fade_out: bool,
    pub fade_frames: u32,
    pub fade_sound_name: String,

    // Container
    pub put_in_container: String,

    // Particle system attachment
    pub particle_sys_name: String,

    // Debris-specific (when name_are_objects = false)
    pub anim_sets: Vec<AnimSet>,
    pub fx_final: Option<String>,
    pub ok_to_change_model_color: bool,
    pub min_lod_required: StaticGameLODLevel,
    /// C++ ShadowType bitfield (`TheShadowNames` / SHADOW_* flags).
    pub shadow_type: u32,
    pub bounce_sound: String,
}

impl Default for GenericObjectCreationNugget {
    fn default() -> Self {
        Self {
            names: Vec::new(),
            name_are_objects: true,
            debris_to_generate: 1,
            offset: Vec3::new(0.0, 0.0, 0.0),
            disposition: DebrisDisposition::new(DebrisDisposition::ON_GROUND_ALIGNED),
            disposition_intensity: 0.0,
            mass: 0.0,
            extra_bounciness: 0.0,
            extra_friction: 0.0,
            min_mag: 0.0,
            max_mag: 0.0,
            min_pitch: 0.0,
            max_pitch: 0.0,
            spin_rate: -1.0,
            yaw_rate: -1.0,
            roll_rate: -1.0,
            pitch_rate: -1.0,
            min_frames: 0,
            max_frames: 0,
            min_health: 1.0,
            max_health: 1.0,
            inherit_veterancy: false,
            ignore_primary_obstacle: false,
            skip_if_significantly_airborne: false,
            invulnerable_time: 0,
            contain_inside_source_object: false,
            dies_on_bad_land: false,
            requires_live_player: false,
            preserve_layer: true,
            orient_in_force_direction: false,
            spread_formation: false,
            min_distance_a_formation: 0.0,
            min_distance_b_formation: 0.0,
            max_distance_formation: 0.0,
            fade_in: false,
            fade_out: false,
            fade_frames: 0,
            fade_sound_name: String::new(),
            put_in_container: String::new(),
            particle_sys_name: String::new(),
            anim_sets: Vec::new(),
            fx_final: None,
            ok_to_change_model_color: false,
            min_lod_required: StaticGameLODLevel::Low,
            shadow_type: 0,
            bounce_sound: String::new(),
        }
    }
}

impl GenericObjectCreationNugget {
    /// Draw yaw, roll, and pitch rates, then the launch force
    /// (`ObjectCreationList.cpp:1129-1175`).
    pub fn draw_flight(&self, mut draw: impl FnMut(f32, f32) -> f32) -> (Vec3, [f32; 3]) {
        // Calculate spin rates
        let spin_rate = if self.spin_rate >= 0.0 {
            self.spin_rate
        } else {
            (PI / 32.0) * self.disposition_intensity
        };

        let yaw_rate = if self.yaw_rate >= 0.0 {
            self.yaw_rate
        } else {
            spin_rate
        };
        let roll_rate = if self.roll_rate >= 0.0 {
            self.roll_rate
        } else {
            spin_rate
        };
        let pitch_rate = if self.pitch_rate >= 0.0 {
            self.pitch_rate
        } else {
            spin_rate
        };

        let yaw = draw(-yaw_rate, yaw_rate);
        let roll = draw(-roll_rate, roll_rate);
        let pitch = draw(-pitch_rate, pitch_rate);

        // Calculate force based on disposition
        let force = if self.disposition.has(DebrisDisposition::SEND_IT_FLYING) {
            let horiz_force = 4.0 * self.disposition_intensity;
            let vert_force = 3.0 * self.disposition_intensity;
            Vec3::new(
                draw(-horiz_force, horiz_force),
                draw(-horiz_force, horiz_force),
                draw(vert_force * 0.33, vert_force),
            )
        } else if self.disposition.has(DebrisDisposition::SEND_IT_UP) {
            let horiz_force = 2.0 * self.disposition_intensity;
            let vert_force = 4.0 * self.disposition_intensity;
            Vec3::new(
                draw(-horiz_force, horiz_force),
                draw(-horiz_force, horiz_force),
                draw(vert_force * 0.75, vert_force),
            )
        } else {
            calc_random_force_with(
                draw,
                self.min_mag,
                self.max_mag,
                self.min_pitch,
                self.max_pitch,
            )
        };

        (force, [yaw, roll, pitch])
    }
}

//! Existing source camera-shaker state and constructor, owned by SessionCamera.
use glam::Vec3;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScriptCameraShaker {
    pub(crate) epicenter: Vec3,
    pub(crate) radius: f32,
    pub(crate) duration_seconds: f32,
    pub(crate) elapsed_seconds: f32,
    /// C++ `Add_Camera_Shake` power*PI/180 intensity (radians).
    pub(crate) intensity: f32,
    /// Per-axis start omega in [12.5, 15] revolutions/s (radians).
    pub(crate) omega: Vec3,
    pub(crate) phi: Vec3,
    pub(crate) rng_seed: u32,
}

impl ScriptCameraShaker {
    pub(crate) fn new(
        epicenter: Vec3,
        radius: f32,
        duration_seconds: f32,
        amplitude_degrees: f32,
    ) -> Self {
        // Deterministic stand-in for C++ WWMath::Random_Float(MIN_OMEGA, MAX_OMEGA)
        // and Random_Float(0, 360deg). Frequency stays in the 12.5-15Hz band.
        let seed = (epicenter.x.to_bits())
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(epicenter.y.to_bits().rotate_left(7))
            .wrapping_add(epicenter.z.to_bits().rotate_left(13))
            .wrapping_add(amplitude_degrees.to_bits());
        let unit = |lane: u32| -> f32 {
            let mut x = seed ^ lane.wrapping_mul(0x85EB_CA6B);
            x ^= x >> 16;
            x = x.wrapping_mul(0x7FEB_352D);
            x ^= x >> 15;
            x = x.wrapping_mul(0x846C_A68B);
            x ^= x >> 16;
            (x >> 8) as f32 / ((1u32 << 24) as f32)
        };
        let min_omega = 12.5 * std::f32::consts::TAU;
        let max_omega = 15.0 * std::f32::consts::TAU;
        let omega_at = |lane: u32| min_omega + unit(lane) * (max_omega - min_omega);
        Self {
            epicenter,
            radius: radius.max(0.01),
            duration_seconds: duration_seconds.max(0.01),
            elapsed_seconds: 0.0,
            intensity: amplitude_degrees.abs() * std::f32::consts::PI / 180.0,
            omega: Vec3::new(omega_at(1), omega_at(2), omega_at(3)),
            phi: Vec3::new(
                unit(4) * std::f32::consts::TAU,
                unit(5) * std::f32::consts::TAU,
                unit(6) * std::f32::consts::TAU,
            ),
            rng_seed: seed,
        }
    }
}

pub(crate) const SHAKE_AXIS_PITCH: f32 = 7.5 * std::f32::consts::PI / 180.;
pub(crate) const SHAKE_AXIS_YAW: f32 = 15. * std::f32::consts::PI / 180.;
pub(crate) const SHAKE_AXIS_ROLL: f32 = 5. * std::f32::consts::PI / 180.;
const SHAKE_END_OMEGA: f32 = std::f32::consts::TAU;
fn shaker_hash_signed(seed: u32, elapsed_bits: u32, axis: u32, pass: u32) -> f32 {
    let mut x = seed
        ^ elapsed_bits.wrapping_mul(0x9E37_79B9)
        ^ axis.wrapping_mul(0x85EB_CA6B)
        ^ pass.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    let unit = (x >> 8) as f32 / ((1u32 << 24) as f32);
    unit * 2.0 - 1.0
}

pub(crate) fn script_camera_shaker_rotations(
    shaker: &ScriptCameraShaker,
    camera_position: Vec3,
) -> Vec3 {
    // C++ CameraShakerClass::Compute_Rotations: 3D eye-to-epicenter falloff,
    // omega(t)=omega+(END_OMEGA-omega)*elapsed, plus ±0.5*intensity fudge
    // accumulated three times (once per axis loop).
    let offset = camera_position - shaker.epicenter;
    let dist_sq = offset.length_squared();
    if dist_sq > shaker.radius * shaker.radius {
        return Vec3::ZERO;
    }
    let dist = dist_sq.sqrt();
    let life = (1.0 - shaker.elapsed_seconds / shaker.duration_seconds).clamp(0.0, 1.0);
    let intensity = shaker.intensity * (1.0 - dist / shaker.radius) * life;
    if intensity <= f32::EPSILON {
        return Vec3::ZERO;
    }
    let t = shaker.elapsed_seconds.max(0.0);
    let elapsed_bits = t.to_bits();
    let axis = [SHAKE_AXIS_PITCH, SHAKE_AXIS_YAW, SHAKE_AXIS_ROLL];
    let mut angles = Vec3::ZERO;
    for i in 0..3 {
        let omega = shaker.omega[i] + (SHAKE_END_OMEGA - shaker.omega[i]) * t;
        angles[i] += axis[i] * intensity * (omega * t + shaker.phi[i]).sin();
        let minor = intensity * 0.5;
        angles.x += shaker_hash_signed(shaker.rng_seed, elapsed_bits, i as u32, 0) * minor;
        angles.y += shaker_hash_signed(shaker.rng_seed, elapsed_bits, i as u32, 1) * minor;
        angles.z += shaker_hash_signed(shaker.rng_seed, elapsed_bits, i as u32, 2) * minor;
    }
    angles
}

pub(crate) fn step_transform_shakers(shakers: &mut Vec<ScriptCameraShaker>, commit: bool) {
    if !commit {
        return;
    }
    const SCRIPT_CAMERA_SHAKER_STEP: f32 = 1.0 / 30.0;
    for shaker in shakers.iter_mut() {
        shaker.elapsed_seconds += SCRIPT_CAMERA_SHAKER_STEP;
    }
    shakers.retain(|shaker| shaker.elapsed_seconds < shaker.duration_seconds);
}

impl super::SessionCamera {
    pub(crate) fn advance_transform_shakers(&mut self, target: Vec3, impulse: Vec3) {
        step_transform_shakers(&mut self.script_camera_shakers, true);
        let eye = super::pose::camera_orbit_pose(
            &self.native_camera,
            target,
            impulse,
            self.camera_zoom,
            self.camera_pitch_radians,
            self.camera_yaw_radians,
            self.camera_fx_pitch,
            self.constraint_bounds(),
        )
        .0;
        let mut rotation = Vec3::ZERO;
        for shaker in &self.script_camera_shakers {
            rotation += script_camera_shaker_rotations(shaker, eye);
        }
        rotation.x = rotation.x.clamp(-SHAKE_AXIS_PITCH, SHAKE_AXIS_PITCH);
        rotation.y = rotation.y.clamp(-SHAKE_AXIS_YAW, SHAKE_AXIS_YAW);
        rotation.z = rotation.z.clamp(-SHAKE_AXIS_ROLL, SHAKE_AXIS_ROLL);
        self.camera_shake_rotation = rotation;
    }
}

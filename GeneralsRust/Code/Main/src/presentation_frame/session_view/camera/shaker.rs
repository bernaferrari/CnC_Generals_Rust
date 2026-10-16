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

//! Complete 3D audio positioning and attenuation system
//!
//! Matches C++ MILES 3D audio from Sound3DClass and implements:
//! - Distance-based attenuation with falloff curves
//! - Doppler effect for moving sources
//! - 3D panning and HRTF-ready output
//! - Occlusion and obstruction
//! - Environmental effects

use crate::{
    error::Result,
    math::{Matrix3D, Vector3},
    mixer::{VoiceHandle, VoiceParams, VoiceSpatialMode, VoiceSpatialParams},
};
use std::f32::consts::PI;

/// Speed of sound in meters per second (C++ constant from AudibleSound.cpp)
pub const SPEED_OF_SOUND_M_S: f32 = 343.0;

/// Minimum distance where volume starts to attenuate (C++ MIN_DISTANCE)
pub const DEFAULT_MIN_DISTANCE: f32 = 1.0;

/// Maximum distance where sound is completely silent (C++ MAX_DISTANCE)
pub const DEFAULT_MAX_DISTANCE: f32 = 1000.0;

/// Distance rolloff factor (C++ ROLLOFF_FACTOR)
pub const DEFAULT_ROLLOFF_FACTOR: f32 = 1.0;

/// Attenuation model matching C++ MILES providers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttenuationModel {
    /// No distance attenuation
    None,
    /// Inverse distance attenuation: gain = min_distance / (min_distance + rolloff * (distance - min_distance))
    InverseDistance,
    /// Inverse distance clamped
    InverseDistanceClamped,
    /// Linear attenuation: gain = (1 - rolloff * (distance - min_distance) / (max_distance - min_distance))
    Linear,
    /// Linear attenuation clamped
    LinearClamped,
    /// Exponential attenuation: gain = (distance / min_distance)^(-rolloff)
    Exponential,
    /// Exponential clamped
    ExponentialClamped,
}

impl Default for AttenuationModel {
    fn default() -> Self {
        Self::InverseDistanceClamped
    }
}

/// 3D audio source configuration - matches C++ Sound3DClass properties
#[derive(Debug, Clone)]
pub struct Audio3DConfig {
    /// Position in 3D space
    pub position: Vector3,
    /// Velocity for Doppler effect
    pub velocity: Vector3,
    /// Minimum distance (reference distance)
    pub min_distance: f32,
    /// Maximum distance (cutoff distance)
    pub max_distance: f32,
    /// Rolloff factor for distance attenuation
    pub rolloff_factor: f32,
    /// Attenuation model
    pub attenuation_model: AttenuationModel,
    /// Enable Doppler effect
    pub doppler_enabled: bool,
    /// Doppler factor multiplier (0.0 = disabled, 1.0 = realistic)
    pub doppler_factor: f32,
    /// Occlusion factor (0.0 = none, 1.0 = fully occluded)
    pub occlusion: f32,
    /// Obstruction factor (0.0 = none, 1.0 = fully obstructed)
    pub obstruction: f32,
    /// Enable cone-based directional audio
    pub cone_enabled: bool,
    /// Inner cone angle in degrees
    pub cone_inner_angle: f32,
    /// Outer cone angle in degrees
    pub cone_outer_angle: f32,
    /// Volume outside cone
    pub cone_outer_volume: f32,
    /// Source orientation (for cone)
    pub orientation: Vector3,
}

impl Default for Audio3DConfig {
    fn default() -> Self {
        Self {
            position: Vector3::ZERO,
            velocity: Vector3::ZERO,
            min_distance: DEFAULT_MIN_DISTANCE,
            max_distance: DEFAULT_MAX_DISTANCE,
            rolloff_factor: DEFAULT_ROLLOFF_FACTOR,
            attenuation_model: AttenuationModel::default(),
            doppler_enabled: true,
            doppler_factor: 1.0,
            occlusion: 0.0,
            obstruction: 0.0,
            cone_enabled: false,
            cone_inner_angle: 360.0,
            cone_outer_angle: 360.0,
            cone_outer_volume: 0.0,
            orientation: Vector3::new(0.0, 0.0, 1.0),
        }
    }
}

/// 3D listener configuration - matches C++ Listener3DClass
#[derive(Debug, Clone)]
pub struct Listener3DConfig {
    /// Position in 3D space
    pub position: Vector3,
    /// Velocity for Doppler effect
    pub velocity: Vector3,
    /// Orientation matrix
    pub transform: Matrix3D,
    /// Master gain multiplier
    pub gain: f32,
}

impl Default for Listener3DConfig {
    fn default() -> Self {
        Self {
            position: Vector3::ZERO,
            velocity: Vector3::ZERO,
            transform: Matrix3D::IDENTITY,
            gain: 1.0,
        }
    }
}

/// Complete 3D audio calculation result
#[derive(Debug, Clone, Copy)]
pub struct Audio3DResult {
    /// Final gain after distance attenuation
    pub gain: f32,
    /// Stereo pan value (-1.0 = left, 0.0 = center, 1.0 = right)
    pub pan: f32,
    /// Pitch scale for Doppler effect (1.0 = normal)
    pub pitch_scale: f32,
    /// Left channel HRTF delay in samples
    pub hrtf_delay_left: f32,
    /// Right channel HRTF delay in samples
    pub hrtf_delay_right: f32,
}

impl Default for Audio3DResult {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            pitch_scale: 1.0,
            hrtf_delay_left: 0.0,
            hrtf_delay_right: 0.0,
        }
    }
}

/// 3D audio processor - implements MILES-compatible 3D audio
pub struct Audio3DProcessor {
    listener: Listener3DConfig,
    sample_rate: u32,
}

impl Audio3DProcessor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            listener: Listener3DConfig::default(),
            sample_rate,
        }
    }

    /// Update listener configuration
    pub fn set_listener(&mut self, listener: Listener3DConfig) {
        self.listener = listener;
    }

    /// Get current listener configuration
    pub fn listener(&self) -> &Listener3DConfig {
        &self.listener
    }

    /// Calculate complete 3D audio parameters
    /// Matches C++ Sound3DClass::Calculate() and AIL_set_3D_* functions
    pub fn calculate(&self, config: &Audio3DConfig) -> Audio3DResult {
        let mut result = Audio3DResult::default();

        // Calculate relative position
        let relative = config.position - self.listener.position;
        let distance = relative.length();

        // Early exit if at listener position
        if distance < f32::EPSILON {
            result.gain = 1.0;
            result.pan = 0.0;
            return result;
        }

        // Calculate distance attenuation
        result.gain = self.calculate_distance_attenuation(
            distance,
            config.min_distance,
            config.max_distance,
            config.rolloff_factor,
            config.attenuation_model,
        ) * self.listener.gain;

        // Apply occlusion and obstruction
        if config.occlusion > 0.0 {
            result.gain *= 1.0 - (config.occlusion * 0.8); // Occlusion reduces volume by up to 80%
        }
        if config.obstruction > 0.0 {
            result.gain *= 1.0 - (config.obstruction * 0.5); // Obstruction reduces volume by up to 50%
        }

        // Calculate pan from listener's right vector
        let right = self.listener.transform.right_vector().normalize();
        let direction = relative.normalize();
        result.pan = direction.dot(right).clamp(-1.0, 1.0);

        // Calculate cone attenuation if enabled
        if config.cone_enabled {
            let cone_gain = self.calculate_cone_attenuation(
                &config.position,
                &config.orientation,
                &self.listener.position,
                config.cone_inner_angle,
                config.cone_outer_angle,
                config.cone_outer_volume,
            );
            result.gain *= cone_gain;
        }

        // Calculate Doppler effect if enabled
        if config.doppler_enabled && config.doppler_factor > 0.0 {
            result.pitch_scale = self.calculate_doppler(
                &relative,
                distance,
                &config.velocity,
                &self.listener.velocity,
                config.doppler_factor,
            );
        }

        // Calculate HRTF delays for improved spatialization
        result.hrtf_delay_left = self.calculate_hrtf_delay(&relative, true);
        result.hrtf_delay_right = self.calculate_hrtf_delay(&relative, false);

        result
    }

    /// Convert Audio3DConfig and result into VoiceSpatialParams for mixer
    pub fn create_spatial_params(&self, config: &Audio3DConfig) -> VoiceSpatialParams {
        VoiceSpatialParams {
            mode: VoiceSpatialMode::Full3D,
            position: config.position,
            velocity: config.velocity,
            listener_position: self.listener.position,
            listener_velocity: self.listener.velocity,
            listener_transform: self.listener.transform,
            min_distance: config.min_distance,
            max_distance: config.max_distance,
        }
    }

    /// Calculate distance-based attenuation
    /// Matches C++ AIL_set_3D_distance_factor and rolloff calculations
    fn calculate_distance_attenuation(
        &self,
        distance: f32,
        min_distance: f32,
        max_distance: f32,
        rolloff: f32,
        model: AttenuationModel,
    ) -> f32 {
        match model {
            AttenuationModel::None => 1.0,

            AttenuationModel::InverseDistance => {
                min_distance / (min_distance + rolloff * (distance - min_distance).max(0.0))
            }

            AttenuationModel::InverseDistanceClamped => {
                let clamped_distance = distance.clamp(min_distance, max_distance);
                min_distance / (min_distance + rolloff * (clamped_distance - min_distance).max(0.0))
            }

            AttenuationModel::Linear => {
                let range = max_distance - min_distance;
                if range < f32::EPSILON {
                    return 1.0;
                }
                (1.0 - rolloff * (distance - min_distance) / range).max(0.0)
            }

            AttenuationModel::LinearClamped => {
                let range = max_distance - min_distance;
                if range < f32::EPSILON {
                    return 1.0;
                }
                let clamped_distance = distance.clamp(min_distance, max_distance);
                (1.0 - rolloff * (clamped_distance - min_distance) / range).max(0.0)
            }

            AttenuationModel::Exponential => {
                if distance < f32::EPSILON {
                    return 1.0;
                }
                (distance / min_distance).powf(-rolloff).max(0.0)
            }

            AttenuationModel::ExponentialClamped => {
                let clamped_distance = distance.clamp(min_distance, max_distance);
                (clamped_distance / min_distance).powf(-rolloff).max(0.0)
            }
        }
    }

    /// Calculate cone-based directional attenuation
    /// Matches C++ AIL_set_3D_orientation_cone
    fn calculate_cone_attenuation(
        &self,
        source_pos: &Vector3,
        source_orientation: &Vector3,
        listener_pos: &Vector3,
        inner_angle: f32,
        outer_angle: f32,
        outer_volume: f32,
    ) -> f32 {
        let to_listener = (*listener_pos - *source_pos).normalize();
        let forward = source_orientation.normalize();

        let dot = to_listener.dot(forward);
        let angle = dot.acos() * 180.0 / PI;

        if angle <= inner_angle / 2.0 {
            1.0
        } else if angle >= outer_angle / 2.0 {
            outer_volume
        } else {
            // Linear interpolation between inner and outer
            let t = (angle - inner_angle / 2.0) / ((outer_angle - inner_angle) / 2.0);
            1.0 + t * (outer_volume - 1.0)
        }
    }

    /// Calculate Doppler pitch shift
    /// Matches C++ AIL_set_3D_doppler_effects
    fn calculate_doppler(
        &self,
        relative: &Vector3,
        distance: f32,
        source_velocity: &Vector3,
        listener_velocity: &Vector3,
        doppler_factor: f32,
    ) -> f32 {
        if distance < f32::EPSILON {
            return 1.0;
        }

        let direction = relative.normalize();

        // Project velocities onto the line between source and listener
        let source_speed = source_velocity.dot(direction);
        let listener_speed = listener_velocity.dot(direction);

        // Calculate Doppler shift: f' = f * (c - vl) / (c - vs)
        let numerator = SPEED_OF_SOUND_M_S - listener_speed;
        let denominator = SPEED_OF_SOUND_M_S - source_speed;

        if denominator.abs() < f32::EPSILON {
            return 1.0;
        }

        let raw_scale = numerator / denominator;

        // Apply doppler factor and clamp to reasonable range
        let factor_scale = 1.0 + (raw_scale - 1.0) * doppler_factor;
        factor_scale.clamp(0.25, 4.0)
    }

    /// Calculate HRTF delay for improved spatialization
    /// Simplified head-related transfer function delay
    fn calculate_hrtf_delay(&self, relative: &Vector3, left_ear: bool) -> f32 {
        const HEAD_RADIUS_M: f32 = 0.0875; // Average head radius ~8.75cm

        let right = self.listener.transform.right_vector();
        let direction = relative.normalize();

        let azimuth = direction.dot(right);

        // Calculate inter-aural time difference
        let itd_samples = if left_ear {
            (azimuth * HEAD_RADIUS_M / SPEED_OF_SOUND_M_S * self.sample_rate as f32).max(0.0)
        } else {
            (-azimuth * HEAD_RADIUS_M / SPEED_OF_SOUND_M_S * self.sample_rate as f32).max(0.0)
        };

        itd_samples.clamp(0.0, 0.001 * self.sample_rate as f32) // Max ~1ms delay
    }
}

/// Utility for batch updating 3D audio sources
pub struct Audio3DBatchProcessor {
    processor: Audio3DProcessor,
    configs: Vec<(VoiceHandle, Audio3DConfig)>,
}

impl Audio3DBatchProcessor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            processor: Audio3DProcessor::new(sample_rate),
            configs: Vec::new(),
        }
    }

    pub fn set_listener(&mut self, listener: Listener3DConfig) {
        self.processor.set_listener(listener);
    }

    pub fn add_source(&mut self, handle: VoiceHandle, config: Audio3DConfig) {
        self.configs.push((handle, config));
    }

    pub fn clear_sources(&mut self) {
        self.configs.clear();
    }

    /// Calculate all 3D audio and return updated voice params
    pub fn process_all(&self) -> Vec<(VoiceHandle, VoiceParams)> {
        self.configs
            .iter()
            .map(|(handle, config)| {
                let result = self.processor.calculate(config);
                let spatial = self.processor.create_spatial_params(config);

                let params = VoiceParams {
                    gain: result.gain,
                    pan: result.pan,
                    playback_rate: (44100.0 * result.pitch_scale) as u32,
                    loop_count: 1,
                    start_frame: 0,
                    is_culled: false,
                    spatial,
                };

                (*handle, params)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_attenuation_inverse() {
        let processor = Audio3DProcessor::new(44100);

        // At min distance, gain should be 1.0
        let gain = processor.calculate_distance_attenuation(
            1.0,
            1.0,
            100.0,
            1.0,
            AttenuationModel::InverseDistance,
        );
        assert!((gain - 1.0).abs() < 0.01);

        // At 2x min distance with rolloff 1.0, gain should be 0.5
        let gain = processor.calculate_distance_attenuation(
            2.0,
            1.0,
            100.0,
            1.0,
            AttenuationModel::InverseDistance,
        );
        assert!((gain - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_distance_attenuation_linear() {
        let processor = Audio3DProcessor::new(44100);

        let gain = processor.calculate_distance_attenuation(
            50.0,
            0.0,
            100.0,
            1.0,
            AttenuationModel::Linear,
        );
        assert!((gain - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_doppler_effect() {
        let processor = Audio3DProcessor::new(44100);
        let relative = Vector3::new(100.0, 0.0, 0.0);
        let source_velocity = Vector3::new(34.3, 0.0, 0.0); // Moving toward listener at 10% speed of sound
        let listener_velocity = Vector3::ZERO;

        let pitch = processor.calculate_doppler(
            &relative,
            100.0,
            &source_velocity,
            &listener_velocity,
            1.0,
        );

        // Moving toward listener should increase pitch
        assert!(pitch > 1.0);
    }

    #[test]
    fn test_cone_attenuation() {
        let processor = Audio3DProcessor::new(44100);

        // Listener directly in front (inside inner cone)
        let gain = processor.calculate_cone_attenuation(
            &Vector3::ZERO,
            &Vector3::new(0.0, 0.0, 1.0),
            &Vector3::new(0.0, 0.0, 10.0),
            90.0,
            180.0,
            0.0,
        );
        assert!((gain - 1.0).abs() < 0.01);

        // Listener directly behind (outside outer cone)
        let gain = processor.calculate_cone_attenuation(
            &Vector3::ZERO,
            &Vector3::new(0.0, 0.0, 1.0),
            &Vector3::new(0.0, 0.0, -10.0),
            90.0,
            180.0,
            0.0,
        );
        assert!(gain < 0.1);
    }
}

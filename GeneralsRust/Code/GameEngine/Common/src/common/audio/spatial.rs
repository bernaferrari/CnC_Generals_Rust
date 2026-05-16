//! 3D Spatial Audio System with HRTF Support
//!
//! This module implements advanced 3D spatial audio processing including:
//! - Head-Related Transfer Function (HRTF) processing
//! - Distance-based attenuation
//! - Doppler effect simulation
//! - Environmental occlusion and reverb
//! - Multi-listener support for split-screen scenarios

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(feature = "audio")]
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

use crate::common::audio::{AudioHandle, Coord3D, Real, TimeOfDay};

/// Maximum distance for 3D audio processing (beyond this, sound is muted)
pub const MAX_3D_DISTANCE: f32 = 1000.0;

/// Minimum distance for 3D audio (closer than this uses constant volume)
pub const MIN_3D_DISTANCE: f32 = 1.0;

/// Speed of sound in units per second (for Doppler effect)
pub const SPEED_OF_SOUND: f32 = 343.0;

/// HRTF sample rate
pub const HRTF_SAMPLE_RATE: u32 = 44100;

/// HRTF impulse response length
pub const HRTF_LENGTH: usize = 256;

/// 3D audio position in world space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Position3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn from_coord3d(coord: &Coord3D) -> Self {
        Self::new(coord.x, coord.y, coord.z)
    }

    pub fn distance_to(&self, other: &Position3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn direction_to(&self, other: &Position3D) -> Direction3D {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        let magnitude = (dx * dx + dy * dy + dz * dz).sqrt();

        if magnitude > 0.0 {
            Direction3D {
                x: dx / magnitude,
                y: dy / magnitude,
                z: dz / magnitude,
            }
        } else {
            Direction3D {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            }
        }
    }

    pub fn lerp(&self, other: &Position3D, t: f32) -> Position3D {
        Position3D {
            x: self.x + t * (other.x - self.x),
            y: self.y + t * (other.y - self.y),
            z: self.z + t * (other.z - self.z),
        }
    }
}

impl Default for Position3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl From<[f32; 3]> for Position3D {
    fn from(array: [f32; 3]) -> Self {
        Self::new(array[0], array[1], array[2])
    }
}

impl From<Position3D> for [f32; 3] {
    fn from(pos: Position3D) -> Self {
        [pos.x, pos.y, pos.z]
    }
}

/// 3D direction vector (normalized)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Direction3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Direction3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        let magnitude = (x * x + y * y + z * z).sqrt();
        if magnitude > 0.0 {
            Self {
                x: x / magnitude,
                y: y / magnitude,
                z: z / magnitude,
            }
        } else {
            Self {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            }
        }
    }

    pub fn dot(&self, other: &Direction3D) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: &Direction3D) -> Direction3D {
        Direction3D::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn angle_between(&self, other: &Direction3D) -> f32 {
        self.dot(other).clamp(-1.0, 1.0).acos()
    }
}

impl Default for Direction3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        } // Forward direction
    }
}

impl From<[f32; 3]> for Direction3D {
    fn from(array: [f32; 3]) -> Self {
        Self::new(array[0], array[1], array[2])
    }
}

impl From<Direction3D> for [f32; 3] {
    fn from(dir: Direction3D) -> [f32; 3] {
        [dir.x, dir.y, dir.z]
    }
}

/// Velocity in 3D space (for Doppler effect)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Velocity3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Velocity3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn dot(&self, direction: &Direction3D) -> f32 {
        self.x * direction.x + self.y * direction.y + self.z * direction.z
    }
}

/// Audio listener with full 3D orientation
#[derive(Debug, Clone)]
pub struct SpatialListener {
    /// Position in 3D space
    pub position: Position3D,
    /// Forward direction (where the listener is looking)
    pub forward: Direction3D,
    /// Up direction (orientation of the listener's head)
    pub up: Direction3D,
    /// Velocity for Doppler effect
    pub velocity: Velocity3D,
    /// Hearing sensitivity (0.0 = deaf, 1.0 = normal)
    pub hearing_sensitivity: f32,
    /// HRTF profile (different head sizes/shapes)
    pub hrtf_profile: HRTFProfile,
}

impl SpatialListener {
    pub fn new() -> Self {
        Self {
            position: Position3D::default(),
            forward: Direction3D::default(),
            up: Direction3D::new(0.0, 1.0, 0.0),
            velocity: Velocity3D::default(),
            hearing_sensitivity: 1.0,
            hrtf_profile: HRTFProfile::Generic,
        }
    }

    pub fn set_position(&mut self, position: Position3D) {
        self.position = position;
    }

    pub fn set_orientation(&mut self, forward: Direction3D, up: Direction3D) {
        self.forward = forward;
        self.up = up;
    }

    pub fn get_right(&self) -> Direction3D {
        self.forward.cross(&self.up)
    }
}

impl Default for SpatialListener {
    fn default() -> Self {
        Self::new()
    }
}

/// HRTF profiles for different head types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HRTFProfile {
    /// Generic profile suitable for most listeners
    Generic,
    /// Small head profile
    SmallHead,
    /// Large head profile
    LargeHead,
    /// Custom profile with specific parameters
    Custom { head_radius: f32, ear_distance: f32 },
}

impl Default for HRTFProfile {
    fn default() -> Self {
        HRTFProfile::Generic
    }
}

impl std::hash::Hash for HRTFProfile {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            HRTFProfile::Generic => 0.hash(state),
            HRTFProfile::SmallHead => 1.hash(state),
            HRTFProfile::LargeHead => 2.hash(state),
            HRTFProfile::Custom {
                head_radius,
                ear_distance,
            } => {
                3.hash(state);
                // Hash floats as their bit representation
                head_radius.to_bits().hash(state);
                ear_distance.to_bits().hash(state);
            }
        }
    }
}

impl Eq for HRTFProfile {}

/// HRTF impulse response data
#[derive(Debug, Clone)]
pub struct HRTFResponse {
    /// Left ear impulse response
    pub left: Vec<f32>,
    /// Right ear impulse response
    pub right: Vec<f32>,
    /// Azimuth angle (degrees, -180 to 180)
    pub azimuth: f32,
    /// Elevation angle (degrees, -90 to 90)
    pub elevation: f32,
}

impl HRTFResponse {
    pub fn new(azimuth: f32, elevation: f32) -> Self {
        Self {
            left: vec![0.0; HRTF_LENGTH],
            right: vec![0.0; HRTF_LENGTH],
            azimuth,
            elevation,
        }
    }
}

/// HRTF processor for spatial audio
pub struct HRTFProcessor {
    /// HRTF impulse responses indexed by angle
    responses: HashMap<(i32, i32), HRTFResponse>,
    /// Current HRTF profile
    profile: HRTFProfile,
    /// FFT planner for efficient convolution
    #[cfg(feature = "audio")]
    fft_planner: Arc<Mutex<RealFftPlanner<f32>>>,
}

impl HRTFProcessor {
    pub fn new(profile: HRTFProfile) -> Self {
        let mut processor = Self {
            responses: HashMap::new(),
            profile,
            #[cfg(feature = "audio")]
            fft_planner: Arc::new(Mutex::new(RealFftPlanner::new())),
        };

        processor.generate_hrtf_responses();
        processor
    }

    /// Generate HRTF responses for the current profile
    fn generate_hrtf_responses(&mut self) {
        // Generate HRTF responses for common angles
        // In a real implementation, these would be loaded from measured HRTF data
        for azimuth in (-180..=180).step_by(15) {
            for elevation in (-90..=90).step_by(15) {
                let mut response = HRTFResponse::new(azimuth as f32, elevation as f32);
                self.generate_synthetic_hrtf(&mut response);
                self.responses.insert((azimuth, elevation), response);
            }
        }
    }

    /// Generate synthetic HRTF response (simplified model)
    fn generate_synthetic_hrtf(&self, response: &mut HRTFResponse) {
        let azimuth_rad = response.azimuth.to_radians();
        let elevation_rad = response.elevation.to_radians();

        // Simple synthetic HRTF based on geometric acoustics
        let (head_radius, ear_distance) = match self.profile {
            HRTFProfile::Generic => (0.0875, 0.14), // 8.75cm head radius, 14cm ear distance
            HRTFProfile::SmallHead => (0.08, 0.12),
            HRTFProfile::LargeHead => (0.095, 0.16),
            HRTFProfile::Custom {
                head_radius,
                ear_distance,
            } => (head_radius, ear_distance),
        };

        // Generate impulse responses using simplified head shadow model
        for i in 0..HRTF_LENGTH {
            let t = i as f32 / HRTF_SAMPLE_RATE as f32;

            // Left ear response
            let left_delay = self.calculate_ear_delay(
                azimuth_rad,
                elevation_rad,
                head_radius,
                ear_distance,
                true,
            );
            let left_attenuation = self.calculate_ear_attenuation(azimuth_rad, elevation_rad, true);
            response.left[i] = self.generate_impulse_sample(t, left_delay, left_attenuation);

            // Right ear response
            let right_delay = self.calculate_ear_delay(
                azimuth_rad,
                elevation_rad,
                head_radius,
                ear_distance,
                false,
            );
            let right_attenuation =
                self.calculate_ear_attenuation(azimuth_rad, elevation_rad, false);
            response.right[i] = self.generate_impulse_sample(t, right_delay, right_attenuation);
        }
    }

    /// Calculate delay to ear based on head geometry
    fn calculate_ear_delay(
        &self,
        azimuth: f32,
        elevation: f32,
        head_radius: f32,
        ear_distance: f32,
        left_ear: bool,
    ) -> f32 {
        let ear_angle = if left_ear {
            azimuth + std::f32::consts::PI / 2.0
        } else {
            azimuth - std::f32::consts::PI / 2.0
        };
        let path_difference =
            head_radius * (1.0 - ear_angle.cos()) + ear_distance * azimuth.sin().abs();
        path_difference / SPEED_OF_SOUND
    }

    /// Calculate attenuation due to head shadow
    fn calculate_ear_attenuation(&self, azimuth: f32, elevation: f32, left_ear: bool) -> f32 {
        let ear_angle = if left_ear {
            azimuth + std::f32::consts::PI / 2.0
        } else {
            azimuth - std::f32::consts::PI / 2.0
        };
        let shadow_factor = (1.0 + ear_angle.cos()) / 2.0; // Simple head shadow model
        0.5 + 0.5 * shadow_factor // Avoid complete attenuation
    }

    /// Generate a single sample of the impulse response
    fn generate_impulse_sample(&self, time: f32, delay: f32, attenuation: f32) -> f32 {
        if time < delay {
            0.0
        } else {
            let t = time - delay;
            // Simple exponentially decaying impulse
            attenuation * (-t * 1000.0).exp() * (t * 2000.0 * std::f32::consts::PI).sin()
        }
    }

    /// Get HRTF response for a specific direction
    pub fn get_hrtf_response(&self, azimuth: f32, elevation: f32) -> Option<&HRTFResponse> {
        let azimuth_quantized = (azimuth / 15.0).round() as i32 * 15;
        let elevation_quantized = (elevation / 15.0).round() as i32 * 15;
        self.responses
            .get(&(azimuth_quantized, elevation_quantized))
    }

    /// Apply HRTF processing to audio samples
    pub fn process_audio(
        &self,
        samples: &[f32],
        azimuth: f32,
        elevation: f32,
    ) -> (Vec<f32>, Vec<f32>) {
        if let Some(response) = self.get_hrtf_response(azimuth, elevation) {
            self.convolve_with_hrtf(samples, response)
        } else {
            // Fallback to simple panning if no HRTF response available
            self.simple_pan(samples, azimuth)
        }
    }

    /// Convolve audio with HRTF impulse response
    fn convolve_with_hrtf(&self, samples: &[f32], hrtf: &HRTFResponse) -> (Vec<f32>, Vec<f32>) {
        let output_length = samples.len() + HRTF_LENGTH - 1;
        let mut left_output = vec![0.0; output_length];
        let mut right_output = vec![0.0; output_length];

        // Simple time-domain convolution (could be optimized with FFT)
        for (i, &sample) in samples.iter().enumerate() {
            for (j, &hrtf_sample) in hrtf.left.iter().enumerate() {
                if i + j < left_output.len() {
                    left_output[i + j] += sample * hrtf_sample;
                }
            }
            for (j, &hrtf_sample) in hrtf.right.iter().enumerate() {
                if i + j < right_output.len() {
                    right_output[i + j] += sample * hrtf_sample;
                }
            }
        }

        (left_output, right_output)
    }

    /// Simple stereo panning fallback
    fn simple_pan(&self, samples: &[f32], azimuth: f32) -> (Vec<f32>, Vec<f32>) {
        let pan = (azimuth / std::f32::consts::PI).clamp(-1.0, 1.0);
        let left_gain = (1.0 - pan) * 0.5;
        let right_gain = (1.0 + pan) * 0.5;

        let left_output: Vec<f32> = samples.iter().map(|&s| s * left_gain).collect();
        let right_output: Vec<f32> = samples.iter().map(|&s| s * right_gain).collect();

        (left_output, right_output)
    }
}

/// Spatial audio source
#[derive(Debug, Clone)]
pub struct SpatialSource {
    /// Unique identifier
    pub handle: AudioHandle,
    /// Position in 3D space
    pub position: Position3D,
    /// Velocity for Doppler effect
    pub velocity: Velocity3D,
    /// Base volume (before 3D processing)
    pub base_volume: f32,
    /// Current processed volume
    pub processed_volume: f32,
    /// Attenuation parameters
    pub min_distance: f32,
    pub max_distance: f32,
    pub rolloff_factor: f32,
    /// Doppler effect parameters
    pub doppler_factor: f32,
    /// Occlusion amount (0.0 = no occlusion, 1.0 = fully occluded)
    pub occlusion: f32,
    /// Environmental reverb amount
    pub reverb_amount: f32,
}

impl SpatialSource {
    pub fn new(handle: AudioHandle, position: Position3D) -> Self {
        Self {
            handle,
            position,
            velocity: Velocity3D::default(),
            base_volume: 1.0,
            processed_volume: 1.0,
            min_distance: MIN_3D_DISTANCE,
            max_distance: MAX_3D_DISTANCE,
            rolloff_factor: 1.0,
            doppler_factor: 1.0,
            occlusion: 0.0,
            reverb_amount: 0.0,
        }
    }

    /// Calculate distance-based attenuation
    pub fn calculate_distance_attenuation(&self, listener_position: &Position3D) -> f32 {
        let distance = self.position.distance_to(listener_position);

        if distance <= self.min_distance {
            1.0
        } else if distance >= self.max_distance {
            0.0
        } else {
            let normalized_distance =
                (distance - self.min_distance) / (self.max_distance - self.min_distance);
            (1.0 - normalized_distance.powf(self.rolloff_factor)).max(0.0)
        }
    }

    /// Calculate Doppler shift factor
    pub fn calculate_doppler_shift(&self, listener: &SpatialListener) -> f32 {
        let direction_to_listener = self.position.direction_to(&listener.position);

        let source_velocity_component = self.velocity.dot(&direction_to_listener);
        let listener_velocity_component = listener.velocity.dot(&direction_to_listener);

        let relative_velocity = listener_velocity_component - source_velocity_component;

        // Doppler shift formula: f' = f * (c + vr) / (c + vs)
        let doppler_shift = (SPEED_OF_SOUND + relative_velocity) / SPEED_OF_SOUND;

        // Clamp to reasonable values to avoid artifacts
        doppler_shift.clamp(0.5, 2.0)
    }

    /// Get spherical coordinates relative to listener
    pub fn get_spherical_coords(&self, listener: &SpatialListener) -> (f32, f32, f32) {
        let direction = listener.position.direction_to(&self.position);
        let distance = listener.position.distance_to(&self.position);

        // Transform to listener's local coordinate system
        let right = listener.forward.cross(&listener.up);
        let local_x = direction.dot(&right);
        let local_y = direction.dot(&listener.up);
        let local_z = direction.dot(&listener.forward);

        // Calculate spherical coordinates
        let azimuth = local_x.atan2(-local_z); // Azimuth in listener's coordinate system
        let elevation = local_y.asin(); // Elevation angle

        (azimuth, elevation, distance)
    }
}

/// Spatial audio processor
pub struct SpatialAudioProcessor {
    /// HRTF processor for each profile
    hrtf_processors: HashMap<HRTFProfile, HRTFProcessor>,
    /// Active spatial sources
    sources: Arc<RwLock<HashMap<AudioHandle, SpatialSource>>>,
    /// Audio listener
    listener: Arc<RwLock<SpatialListener>>,
    /// Environmental parameters
    environment: Arc<RwLock<EnvironmentalAudio>>,
}

impl SpatialAudioProcessor {
    pub fn new() -> Self {
        let mut hrtf_processors = HashMap::new();
        hrtf_processors.insert(
            HRTFProfile::Generic,
            HRTFProcessor::new(HRTFProfile::Generic),
        );
        hrtf_processors.insert(
            HRTFProfile::SmallHead,
            HRTFProcessor::new(HRTFProfile::SmallHead),
        );
        hrtf_processors.insert(
            HRTFProfile::LargeHead,
            HRTFProcessor::new(HRTFProfile::LargeHead),
        );

        Self {
            hrtf_processors,
            sources: Arc::new(RwLock::new(HashMap::new())),
            listener: Arc::new(RwLock::new(SpatialListener::new())),
            environment: Arc::new(RwLock::new(EnvironmentalAudio::new())),
        }
    }

    /// Add or update a spatial source
    pub fn set_source(&self, source: SpatialSource) {
        self.sources.write().insert(source.handle, source);
    }

    /// Remove a spatial source
    pub fn remove_source(&self, handle: AudioHandle) {
        self.sources.write().remove(&handle);
    }

    /// Update listener properties
    pub fn update_listener(&self, listener: SpatialListener) {
        *self.listener.write() = listener;
    }

    /// Process spatial audio for a source
    pub fn process_source_audio(
        &self,
        handle: AudioHandle,
        samples: &[f32],
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        let sources = self.sources.read();
        let listener = self.listener.read();

        if let Some(source) = sources.get(&handle) {
            let (azimuth, elevation, distance) = source.get_spherical_coords(&listener);

            // Calculate distance attenuation
            let distance_attenuation = source.calculate_distance_attenuation(&listener.position);

            // Calculate Doppler shift
            let doppler_shift = source.calculate_doppler_shift(&listener);

            // Apply occlusion
            let occlusion_attenuation = 1.0 - source.occlusion * 0.8; // Max 80% attenuation

            // Get HRTF processor for listener's profile
            if let Some(hrtf_processor) = self.hrtf_processors.get(&listener.hrtf_profile) {
                // Apply HRTF processing
                let (mut left, mut right) =
                    hrtf_processor.process_audio(samples, azimuth, elevation);

                // Apply distance and occlusion attenuation
                let total_attenuation = distance_attenuation
                    * occlusion_attenuation
                    * source.base_volume
                    * listener.hearing_sensitivity;

                for sample in &mut left {
                    *sample *= total_attenuation;
                }
                for sample in &mut right {
                    *sample *= total_attenuation;
                }

                Some((left, right))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Update all sources based on current listener position
    pub fn update_all_sources(&self) {
        let listener = self.listener.read();
        let mut sources = self.sources.write();

        for source in sources.values_mut() {
            source.processed_volume = source.calculate_distance_attenuation(&listener.position);
        }
    }
}

/// Environmental audio parameters
#[derive(Debug, Clone)]
pub struct EnvironmentalAudio {
    /// Ambient reverb amount
    pub reverb_amount: f32,
    /// Air absorption coefficient
    pub air_absorption: f32,
    /// Environmental occlusion map
    pub occlusion_map: HashMap<Position3D, f32>,
    /// Time of day for environmental changes
    pub time_of_day: TimeOfDay,
}

impl EnvironmentalAudio {
    pub fn new() -> Self {
        Self {
            reverb_amount: 0.2,
            air_absorption: 0.01,
            occlusion_map: HashMap::new(),
            time_of_day: TimeOfDay::Day,
        }
    }

    /// Calculate occlusion between two points
    pub fn calculate_occlusion(&self, from: &Position3D, to: &Position3D) -> f32 {
        // Simplified occlusion calculation
        // In a real implementation, this would use ray tracing or a precomputed occlusion map
        0.0
    }

    /// Apply environmental effects to audio
    pub fn apply_environmental_effects(&self, samples: &mut [f32], distance: f32) {
        // Apply air absorption (high frequencies are absorbed more)
        let absorption_factor = (-self.air_absorption * distance).exp();
        for sample in samples {
            *sample *= absorption_factor;
        }
    }
}

impl Default for EnvironmentalAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_3d_operations() {
        let pos1 = Position3D::new(0.0, 0.0, 0.0);
        let pos2 = Position3D::new(3.0, 4.0, 0.0);
        assert_eq!(pos1.distance_to(&pos2), 5.0);

        let direction = pos1.direction_to(&pos2);
        assert_eq!(direction.x, 0.6);
        assert_eq!(direction.y, 0.8);
        assert_eq!(direction.z, 0.0);
    }

    #[test]
    fn test_direction_3d_operations() {
        let dir1 = Direction3D::new(1.0, 0.0, 0.0);
        let dir2 = Direction3D::new(0.0, 1.0, 0.0);

        assert_eq!(dir1.dot(&dir2), 0.0);

        let cross = dir1.cross(&dir2);
        assert_eq!(cross.z, 1.0);

        let angle = dir1.angle_between(&dir2);
        assert!((angle - std::f32::consts::PI / 2.0).abs() < 0.001);
    }

    #[test]
    fn test_spatial_source_attenuation() {
        let source = SpatialSource::new(1, Position3D::new(0.0, 0.0, 0.0));
        let listener_pos = Position3D::new(50.0, 0.0, 0.0);

        let attenuation = source.calculate_distance_attenuation(&listener_pos);
        assert!(attenuation > 0.0 && attenuation < 1.0);
    }

    #[test]
    fn test_hrtf_processor_creation() {
        let processor = HRTFProcessor::new(HRTFProfile::Generic);
        assert!(!processor.responses.is_empty());

        let response = processor.get_hrtf_response(0.0, 0.0);
        assert!(response.is_some());
    }

    #[test]
    fn test_spatial_audio_processor() {
        let processor = SpatialAudioProcessor::new();

        let source = SpatialSource::new(1, Position3D::new(10.0, 0.0, 0.0));
        processor.set_source(source);

        assert!(processor.sources.read().contains_key(&1));

        processor.remove_source(1);
        assert!(!processor.sources.read().contains_key(&1));
    }
}

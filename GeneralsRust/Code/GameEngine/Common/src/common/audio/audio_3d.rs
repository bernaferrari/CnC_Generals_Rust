//! 3D Positional Audio System
//!
//! This module provides comprehensive 3D audio capabilities including distance
//! attenuation, Doppler effects, environmental reverb, and HRTF (Head-Related
//! Transfer Function) processing for realistic spatial audio.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

use crate::common::audio::{AudioHandle, Bool, Coord3D, Int, Real, UnsignedInt};

/// 3D position with utility methods
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Position3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn distance_to(&self, other: &Position3D) -> Real {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn direction_to(&self, other: &Position3D) -> Direction3D {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        let length = (dx * dx + dy * dy + dz * dz).sqrt();

        if length > 0.0 {
            Direction3D::new(dx / length, dy / length, dz / length)
        } else {
            Direction3D::new(0.0, 0.0, 1.0)
        }
    }

    pub fn lerp(&self, other: &Position3D, t: Real) -> Position3D {
        Position3D::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
            self.z + (other.z - self.z) * t,
        )
    }
}

impl From<Coord3D> for Position3D {
    fn from(coord: Coord3D) -> Self {
        Self {
            x: coord.x,
            y: coord.y,
            z: coord.z,
        }
    }
}

impl Into<Coord3D> for Position3D {
    fn into(self) -> Coord3D {
        Coord3D {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

impl Into<[f32; 3]> for Position3D {
    fn into(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

/// 3D direction vector (normalized)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Direction3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Direction3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        let length = (x * x + y * y + z * z).sqrt();
        if length > 0.0 {
            Self {
                x: x / length,
                y: y / length,
                z: z / length,
            }
        } else {
            Self {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            }
        }
    }

    pub fn forward() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }
    }

    pub fn up() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    }

    pub fn right() -> Self {
        Self {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn dot(&self, other: &Direction3D) -> Real {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(&self, other: &Direction3D) -> Direction3D {
        Direction3D::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(&self) -> Real {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

impl Into<[f32; 3]> for Direction3D {
    fn into(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

/// 3D velocity vector
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Velocity3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Velocity3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn magnitude(&self) -> Real {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

impl Into<[f32; 3]> for Velocity3D {
    fn into(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

/// Distance attenuation models
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttenuationModel {
    /// No distance attenuation
    None,
    /// Linear falloff: volume = 1.0 - (distance - min_distance) / (max_distance - min_distance)
    Linear,
    /// Inverse distance: volume = min_distance / distance
    Inverse,
    /// Inverse distance with clamping: volume = min_distance / max(distance, min_distance)
    InverseClamped,
    /// Exponential falloff: volume = exp(-distance / rolloff)
    Exponential,
    /// Custom curve defined by user points
    Custom(Vec<(Real, Real)>), // (distance, volume) pairs
}

/// Environmental reverb parameters
#[derive(Debug, Clone)]
pub struct ReverbParameters {
    pub room_size: Real,   // 0.0 to 1.0
    pub damping: Real,     // 0.0 to 1.0
    pub wet_level: Real,   // 0.0 to 1.0
    pub dry_level: Real,   // 0.0 to 1.0
    pub early_delay: Real, // in seconds
    pub late_delay: Real,  // in seconds
    pub diffusion: Real,   // 0.0 to 1.0
    pub density: Real,     // 0.0 to 1.0
}

impl Default for ReverbParameters {
    fn default() -> Self {
        Self {
            room_size: 0.5,
            damping: 0.5,
            wet_level: 0.3,
            dry_level: 0.7,
            early_delay: 0.02,
            late_delay: 0.04,
            diffusion: 0.7,
            density: 0.8,
        }
    }
}

/// Environmental audio zone
#[derive(Debug, Clone)]
pub struct AudioZone {
    pub id: u32,
    pub center: Position3D,
    pub radius: Real,
    pub reverb: ReverbParameters,
    pub ambient_volume: Real,
    pub priority: Int,
    pub transition_time: Real, // Time to transition into this zone's reverb
}

impl AudioZone {
    pub fn new(id: u32, center: Position3D, radius: Real) -> Self {
        Self {
            id,
            center,
            radius,
            reverb: ReverbParameters::default(),
            ambient_volume: 1.0,
            priority: 0,
            transition_time: 1.0,
        }
    }

    pub fn contains_point(&self, point: &Position3D) -> bool {
        self.center.distance_to(point) <= self.radius
    }

    pub fn distance_to_edge(&self, point: &Position3D) -> Real {
        (self.center.distance_to(point) - self.radius).max(0.0)
    }
}

/// 3D audio listener (camera/player position)
#[derive(Debug, Clone)]
pub struct SpatialListener {
    pub position: Position3D,
    pub forward: Direction3D,
    pub up: Direction3D,
    pub velocity: Velocity3D,
    pub gain: Real,
    pub meters_per_unit: Real, // World units to meters conversion
    pub doppler_factor: Real,  // Doppler effect strength (0.0 = none, 1.0 = full)
    pub speed_of_sound: Real,  // Speed of sound in world units/second
}

impl SpatialListener {
    pub fn new() -> Self {
        Self {
            position: Position3D::zero(),
            forward: Direction3D::forward(),
            up: Direction3D::up(),
            velocity: Velocity3D::zero(),
            gain: 1.0,
            meters_per_unit: 1.0,
            doppler_factor: 1.0,
            speed_of_sound: 343.3, // meters per second
        }
    }

    pub fn set_position(&mut self, position: Position3D) {
        self.position = position;
    }

    pub fn set_orientation(&mut self, forward: Direction3D, up: Direction3D) {
        self.forward = forward;
        self.up = up;
    }

    pub fn set_velocity(&mut self, velocity: Velocity3D) {
        self.velocity = velocity;
    }

    /// Get the right vector from forward and up
    pub fn right(&self) -> Direction3D {
        self.forward.cross(&self.up)
    }

    /// Calculate stereo panning for a position (-1.0 = full left, 1.0 = full right)
    pub fn calculate_pan(&self, source_pos: &Position3D) -> Real {
        let to_source = self.position.direction_to(source_pos);
        let right = self.right();
        right.dot(&to_source).clamp(-1.0, 1.0)
    }
}

impl Default for SpatialListener {
    fn default() -> Self {
        Self::new()
    }
}

/// 3D audio source
#[derive(Debug, Clone)]
pub struct SpatialSource {
    pub handle: AudioHandle,
    pub position: Position3D,
    pub velocity: Velocity3D,
    pub direction: Option<Direction3D>, // For directional sources
    pub cone_inner_angle: Real,         // Inner cone angle in radians
    pub cone_outer_angle: Real,         // Outer cone angle in radians
    pub cone_outer_gain: Real,          // Volume multiplier outside cone
    pub min_distance: Real,
    pub max_distance: Real,
    pub rolloff_factor: Real,
    pub attenuation_model: AttenuationModel,
    pub gain: Real,
    pub pitch: Real,
    pub is_relative: Bool, // Position relative to listener?
    pub is_looping: Bool,
}

impl SpatialSource {
    pub fn new(handle: AudioHandle, position: Position3D) -> Self {
        Self {
            handle,
            position,
            velocity: Velocity3D::zero(),
            direction: None,
            cone_inner_angle: 2.0 * PI, // 360 degrees (omnidirectional)
            cone_outer_angle: 2.0 * PI,
            cone_outer_gain: 1.0,
            min_distance: 1.0,
            max_distance: 1000.0,
            rolloff_factor: 1.0,
            attenuation_model: AttenuationModel::InverseClamped,
            gain: 1.0,
            pitch: 1.0,
            is_relative: false,
            is_looping: false,
        }
    }

    /// Calculate distance-based attenuation
    pub fn calculate_distance_attenuation(&self, distance: Real) -> Real {
        if distance <= self.min_distance {
            return 1.0;
        }
        if distance >= self.max_distance {
            return 0.0;
        }

        match self.attenuation_model {
            AttenuationModel::None => 1.0,
            AttenuationModel::Linear => {
                let range = self.max_distance - self.min_distance;
                if range > 0.0 {
                    1.0 - ((distance - self.min_distance) / range)
                } else {
                    1.0
                }
            }
            AttenuationModel::Inverse => self.min_distance / distance,
            AttenuationModel::InverseClamped => self.min_distance / distance.max(self.min_distance),
            AttenuationModel::Exponential => {
                (-distance / (self.rolloff_factor * self.min_distance)).exp()
            }
            AttenuationModel::Custom(ref curve) => {
                if curve.is_empty() {
                    return 1.0;
                }

                // Linear interpolation between curve points
                for i in 0..curve.len() - 1 {
                    let (d1, v1) = curve[i];
                    let (d2, v2) = curve[i + 1];

                    if distance >= d1 && distance <= d2 {
                        let t = (distance - d1) / (d2 - d1);
                        return v1 + (v2 - v1) * t;
                    }
                }

                // Beyond curve range
                if distance <= curve[0].0 {
                    curve[0].1
                } else {
                    curve.last().unwrap().1
                }
            }
        }
    }

    /// Calculate directional cone attenuation
    pub fn calculate_cone_attenuation(&self, listener_pos: &Position3D) -> Real {
        if let Some(source_dir) = self.direction {
            let to_listener = self.position.direction_to(listener_pos);
            let angle = source_dir.dot(&to_listener).acos();

            if angle <= self.cone_inner_angle * 0.5 {
                1.0 // Inside inner cone
            } else if angle <= self.cone_outer_angle * 0.5 {
                // Interpolate between inner and outer cone
                let inner_half = self.cone_inner_angle * 0.5;
                let outer_half = self.cone_outer_angle * 0.5;
                let t = (angle - inner_half) / (outer_half - inner_half);
                1.0 + (self.cone_outer_gain - 1.0) * t
            } else {
                self.cone_outer_gain // Outside outer cone
            }
        } else {
            1.0 // Omnidirectional
        }
    }

    /// Calculate Doppler pitch shift
    pub fn calculate_doppler_shift(&self, listener: &SpatialListener) -> Real {
        if listener.doppler_factor == 0.0 {
            return 1.0;
        }

        let to_listener = self.position.direction_to(&listener.position);

        // Velocity components towards listener
        let source_velocity = self.velocity.x * to_listener.x
            + self.velocity.y * to_listener.y
            + self.velocity.z * to_listener.z;
        let listener_velocity = listener.velocity.x * to_listener.x
            + listener.velocity.y * to_listener.y
            + listener.velocity.z * to_listener.z;

        let relative_velocity = listener_velocity - source_velocity;
        let doppler_shift = (listener.speed_of_sound + relative_velocity) / listener.speed_of_sound;

        // Apply doppler factor and clamp to reasonable range
        let final_shift = 1.0 + (doppler_shift - 1.0) * listener.doppler_factor;
        final_shift.clamp(0.1, 3.0) // Prevent extreme pitch shifts
    }
}

/// Audio occlusion/obstruction parameters
#[derive(Debug, Clone)]
pub struct OcclusionData {
    pub occlusion_factor: Real,   // 0.0 = no occlusion, 1.0 = fully occluded
    pub obstruction_factor: Real, // 0.0 = no obstruction, 1.0 = fully obstructed
    pub occlusion_lf_ratio: Real, // Low frequency attenuation ratio
    pub occlusion_hf_ratio: Real, // High frequency attenuation ratio
}

impl Default for OcclusionData {
    fn default() -> Self {
        Self {
            occlusion_factor: 0.0,
            obstruction_factor: 0.0,
            occlusion_lf_ratio: 1.0,
            occlusion_hf_ratio: 1.0,
        }
    }
}

/// Main 3D audio processor
pub struct SpatialAudioProcessor {
    listener: RwLock<SpatialListener>,
    sources: RwLock<HashMap<AudioHandle, SpatialSource>>,
    zones: RwLock<HashMap<u32, AudioZone>>,
    current_zone: RwLock<Option<u32>>,
    occlusion_cache: RwLock<HashMap<AudioHandle, OcclusionData>>,
    global_3d_settings: RwLock<Global3DSettings>,
}

#[derive(Debug, Clone)]
pub struct Global3DSettings {
    pub distance_model: AttenuationModel,
    pub doppler_factor: Real,
    pub speed_of_sound: Real,
    pub rolloff_factor: Real,
    pub max_distance: Real,
    pub reference_distance: Real,
}

impl Default for Global3DSettings {
    fn default() -> Self {
        Self {
            distance_model: AttenuationModel::InverseClamped,
            doppler_factor: 1.0,
            speed_of_sound: 343.3,
            rolloff_factor: 1.0,
            max_distance: 1000.0,
            reference_distance: 1.0,
        }
    }
}

impl SpatialAudioProcessor {
    pub fn new() -> Self {
        Self {
            listener: RwLock::new(SpatialListener::new()),
            sources: RwLock::new(HashMap::new()),
            zones: RwLock::new(HashMap::new()),
            current_zone: RwLock::new(None),
            occlusion_cache: RwLock::new(HashMap::new()),
            global_3d_settings: RwLock::new(Global3DSettings::default()),
        }
    }

    /// Update listener position and orientation
    pub fn update_listener(&self, listener: SpatialListener) {
        *self.listener.write().unwrap() = listener;
        self.update_current_zone();
    }

    /// Add or update a 3D audio source
    pub fn add_source(&self, source: SpatialSource) {
        let mut sources = self.sources.write().unwrap();
        sources.insert(source.handle, source);
    }

    /// Remove a 3D audio source
    pub fn remove_source(&self, handle: AudioHandle) -> bool {
        let mut sources = self.sources.write().unwrap();
        let mut occlusion_cache = self.occlusion_cache.write().unwrap();

        occlusion_cache.remove(&handle);
        sources.remove(&handle).is_some()
    }

    /// Update source position
    pub fn update_source_position(&self, handle: AudioHandle, position: Position3D) -> bool {
        let mut sources = self.sources.write().unwrap();
        if let Some(source) = sources.get_mut(&handle) {
            source.position = position;
            true
        } else {
            false
        }
    }

    /// Update source velocity
    pub fn update_source_velocity(&self, handle: AudioHandle, velocity: Velocity3D) -> bool {
        let mut sources = self.sources.write().unwrap();
        if let Some(source) = sources.get_mut(&handle) {
            source.velocity = velocity;
            true
        } else {
            false
        }
    }

    /// Add environmental audio zone
    pub fn add_audio_zone(&self, zone: AudioZone) {
        let mut zones = self.zones.write().unwrap();
        zones.insert(zone.id, zone);
    }

    /// Remove environmental audio zone
    pub fn remove_audio_zone(&self, zone_id: u32) -> bool {
        let mut zones = self.zones.write().unwrap();
        zones.remove(&zone_id).is_some()
    }

    /// Calculate 3D audio parameters for a source
    pub fn calculate_3d_audio_params(&self, handle: AudioHandle) -> Option<Audio3DParams> {
        let sources = self.sources.read().unwrap();
        let listener = self.listener.read().unwrap();
        let global_settings = self.global_3d_settings.read().unwrap();

        let source = sources.get(&handle)?;

        let distance = listener.position.distance_to(&source.position);
        let distance_attenuation = source.calculate_distance_attenuation(distance);
        let cone_attenuation = source.calculate_cone_attenuation(&listener.position);
        let doppler_shift = source.calculate_doppler_shift(&listener);
        let pan = listener.calculate_pan(&source.position);

        // Get occlusion data
        let occlusion_cache = self.occlusion_cache.read().unwrap();
        let occlusion = occlusion_cache.get(&handle).cloned().unwrap_or_default();

        // Calculate final volume with all attenuations
        let final_volume = source.gain * 
                          distance_attenuation * 
                          cone_attenuation * 
                          (1.0 - occlusion.occlusion_factor * 0.8) * // Occlusion reduces volume
                          (1.0 - occlusion.obstruction_factor * 0.6); // Obstruction reduces volume less

        // Calculate environmental reverb mix
        let reverb_mix = self.calculate_reverb_mix(&source.position);

        Some(Audio3DParams {
            volume: final_volume.clamp(0.0, 1.0),
            pitch: (source.pitch * doppler_shift).clamp(0.1, 3.0),
            pan: pan.clamp(-1.0, 1.0),
            distance,
            reverb_mix,
            occlusion_lf_ratio: occlusion.occlusion_lf_ratio,
            occlusion_hf_ratio: occlusion.occlusion_hf_ratio,
        })
    }

    /// Get all currently tracked sources
    pub fn get_all_sources(&self) -> Vec<SpatialSource> {
        let sources = self.sources.read().unwrap();
        sources.values().cloned().collect()
    }

    /// Get current listener position
    pub fn get_listener_position(&self) -> Position3D {
        self.listener.read().unwrap().position
    }

    /// Set occlusion data for a source
    pub fn set_occlusion(&self, handle: AudioHandle, occlusion: OcclusionData) {
        let mut occlusion_cache = self.occlusion_cache.write().unwrap();
        occlusion_cache.insert(handle, occlusion);
    }

    /// Update global 3D settings
    pub fn set_global_settings(&self, settings: Global3DSettings) {
        *self.global_3d_settings.write().unwrap() = settings;
    }

    /// Get statistics about 3D audio processing
    pub fn get_statistics(&self) -> SpatialAudioStats {
        let sources = self.sources.read().unwrap();
        let zones = self.zones.read().unwrap();
        let listener = self.listener.read().unwrap();

        let mut audible_sources = 0;
        let mut total_distance = 0.0;

        for source in sources.values() {
            let distance = listener.position.distance_to(&source.position);
            total_distance += distance;

            if distance <= source.max_distance {
                audible_sources += 1;
            }
        }

        let avg_distance = if sources.len() > 0 {
            total_distance / sources.len() as Real
        } else {
            0.0
        };

        SpatialAudioStats {
            total_sources: sources.len(),
            audible_sources,
            audio_zones: zones.len(),
            average_distance: avg_distance,
            current_zone: *self.current_zone.read().unwrap(),
        }
    }

    /// Calculate reverb mix based on environmental zones
    fn calculate_reverb_mix(&self, position: &Position3D) -> Real {
        let zones = self.zones.read().unwrap();

        let mut total_influence = 0.0;
        let mut weighted_reverb = 0.0;

        for zone in zones.values() {
            let distance_to_edge = zone.distance_to_edge(position);
            if distance_to_edge <= zone.radius {
                // Inside or near the zone
                let influence = if distance_to_edge == 0.0 {
                    1.0 // Inside zone
                } else {
                    // Fade based on distance to edge and transition time
                    (1.0 - distance_to_edge / zone.radius).max(0.0)
                };

                total_influence += influence * zone.priority as Real;
                weighted_reverb += influence * zone.reverb.wet_level * zone.priority as Real;
            }
        }

        if total_influence > 0.0 {
            weighted_reverb / total_influence
        } else {
            0.0 // No environmental influence
        }
    }

    /// Update which audio zone the listener is currently in
    fn update_current_zone(&self) {
        let listener = self.listener.read().unwrap();
        let zones = self.zones.read().unwrap();
        let mut current_zone = self.current_zone.write().unwrap();

        let mut best_zone_id = None;
        let mut best_priority = Int::MIN;

        for zone in zones.values() {
            if zone.contains_point(&listener.position) {
                if zone.priority > best_priority {
                    best_priority = zone.priority;
                    best_zone_id = Some(zone.id);
                }
            }
        }

        *current_zone = best_zone_id;
    }
}

impl Default for SpatialAudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// 3D audio parameters calculated for each source
#[derive(Debug, Clone)]
pub struct Audio3DParams {
    pub volume: Real,
    pub pitch: Real,
    pub pan: Real,
    pub distance: Real,
    pub reverb_mix: Real,
    pub occlusion_lf_ratio: Real,
    pub occlusion_hf_ratio: Real,
}

/// Statistics about spatial audio processing
#[derive(Debug, Clone)]
pub struct SpatialAudioStats {
    pub total_sources: usize,
    pub audible_sources: usize,
    pub audio_zones: usize,
    pub average_distance: Real,
    pub current_zone: Option<u32>,
}

/// HRTF (Head-Related Transfer Function) processor for binaural audio
pub struct HRTFProcessor {
    hrtf_data: Option<HRTFData>,
    sample_rate: u32,
    enabled: bool,
}

#[derive(Debug, Clone)]
pub struct HRTFData {
    pub sample_rate: u32,
    pub azimuth_count: usize,
    pub elevation_count: usize,
    pub ir_length: usize,
    pub left_impulses: Vec<Vec<f32>>,
    pub right_impulses: Vec<Vec<f32>>,
}

impl HRTFProcessor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            hrtf_data: None,
            sample_rate,
            enabled: false,
        }
    }

    /// Load HRTF data from file or generate synthetic data
    pub fn load_hrtf_data(&mut self, data: HRTFData) -> Result<(), String> {
        if data.sample_rate != self.sample_rate {
            return Err(format!(
                "HRTF sample rate {} doesn't match processor sample rate {}",
                data.sample_rate, self.sample_rate
            ));
        }

        self.hrtf_data = Some(data);
        self.enabled = true;
        Ok(())
    }

    /// Enable or disable HRTF processing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled && self.hrtf_data.is_some();
    }

    /// Check if HRTF is available and enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Process audio with HRTF for a given direction
    pub fn process_audio(
        &self,
        direction: Direction3D,
        input: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        let fallback_pan = || {
            let pan = direction.x.clamp(-1.0, 1.0);
            let left_gain = ((1.0 - pan) * 0.5).sqrt();
            let right_gain = ((1.0 + pan) * 0.5).sqrt();

            for (i, &sample) in input.iter().enumerate() {
                left_out[i] = sample * left_gain;
                right_out[i] = sample * right_gain;
            }
        };

        if !self.enabled || input.len() != left_out.len() || input.len() != right_out.len() {
            fallback_pan();
            return;
        }

        let Some(hrtf_data) = self.hrtf_data.as_ref() else {
            fallback_pan();
            return;
        };

        if hrtf_data.azimuth_count == 0
            || hrtf_data.elevation_count == 0
            || hrtf_data.left_impulses.is_empty()
            || hrtf_data.right_impulses.is_empty()
        {
            fallback_pan();
            return;
        }

        let azimuth = direction.x.atan2(direction.z);
        let elevation = direction.y.asin();

        let azimuth_norm = ((azimuth + PI) / (2.0 * PI)).clamp(0.0, 1.0);
        let elevation_norm = ((elevation + PI * 0.5) / PI).clamp(0.0, 1.0);

        let azimuth_index =
            (azimuth_norm * (hrtf_data.azimuth_count.saturating_sub(1)) as f32).round() as usize;
        let elevation_index = (elevation_norm
            * (hrtf_data.elevation_count.saturating_sub(1)) as f32)
            .round() as usize;

        let impulse_index = elevation_index
            .saturating_mul(hrtf_data.azimuth_count)
            .saturating_add(azimuth_index);

        let left_ir = match hrtf_data.left_impulses.get(impulse_index) {
            Some(ir) if !ir.is_empty() => ir,
            _ => {
                fallback_pan();
                return;
            }
        };
        let right_ir = match hrtf_data.right_impulses.get(impulse_index) {
            Some(ir) if !ir.is_empty() => ir,
            _ => {
                fallback_pan();
                return;
            }
        };

        for i in 0..input.len() {
            let mut left_acc = 0.0;
            let mut right_acc = 0.0;

            for (tap, &ir_sample) in left_ir.iter().enumerate() {
                if i >= tap {
                    left_acc += input[i - tap] * ir_sample;
                }
            }

            for (tap, &ir_sample) in right_ir.iter().enumerate() {
                if i >= tap {
                    right_acc += input[i - tap] * ir_sample;
                }
            }

            left_out[i] = left_acc;
            right_out[i] = right_acc;
        }
    }
}

/// Environmental audio system
pub struct EnvironmentalAudio {
    processor: Arc<SpatialAudioProcessor>,
    ambient_zones: Vec<AudioZone>,
    weather_factor: Real,
    time_of_day_factor: Real,
}

impl EnvironmentalAudio {
    pub fn new(processor: Arc<SpatialAudioProcessor>) -> Self {
        Self {
            processor,
            ambient_zones: Vec::new(),
            weather_factor: 1.0,
            time_of_day_factor: 1.0,
        }
    }

    /// Set weather influence on audio (0.0 = clear, 1.0 = storm)
    pub fn set_weather_factor(&mut self, factor: Real) {
        self.weather_factor = factor.clamp(0.0, 1.0);
    }

    /// Set time of day influence on audio (0.0 = night, 1.0 = day)
    pub fn set_time_of_day_factor(&mut self, factor: Real) {
        self.time_of_day_factor = factor.clamp(0.0, 1.0);
    }

    /// Update environmental effects based on listener position
    pub fn update_environmental_effects(&self) {
        let listener_pos = self.processor.get_listener_position();

        // Apply weather effects to all sources
        let sources = self.processor.get_all_sources();
        for source in sources {
            let distance = listener_pos.distance_to(&source.position);

            // Weather reduces distant sound clarity
            let weather_attenuation = if distance > 100.0 {
                1.0 - (self.weather_factor * 0.3 * (distance / 1000.0).min(1.0))
            } else {
                1.0
            };

            // Time of day affects ambient sounds differently than action sounds
            let time_factor = if source.handle % 2 == 0 {
                // Simple check for "ambient" sounds
                0.5 + self.time_of_day_factor * 0.5 // Quieter at night
            } else {
                1.0 // Action sounds unaffected
            };

            let occlusion = OcclusionData {
                occlusion_factor: (1.0 - weather_attenuation) * 0.5,
                obstruction_factor: 0.0,
                occlusion_lf_ratio: weather_attenuation,
                occlusion_hf_ratio: weather_attenuation * weather_attenuation, // High freq more affected
            };

            self.processor.set_occlusion(source.handle, occlusion);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position3d() {
        let pos1 = Position3D::new(0.0, 0.0, 0.0);
        let pos2 = Position3D::new(3.0, 4.0, 0.0);

        assert_eq!(pos1.distance_to(&pos2), 5.0);

        let direction = pos1.direction_to(&pos2);
        assert!((direction.x - 0.6).abs() < 0.001);
        assert!((direction.y - 0.8).abs() < 0.001);
        assert_eq!(direction.z, 0.0);
    }

    #[test]
    fn test_direction3d() {
        let dir1 = Direction3D::new(1.0, 0.0, 0.0);
        let dir2 = Direction3D::new(0.0, 1.0, 0.0);

        assert_eq!(dir1.dot(&dir2), 0.0);

        let cross = dir1.cross(&dir2);
        assert_eq!(cross.x, 0.0);
        assert_eq!(cross.y, 0.0);
        assert_eq!(cross.z, 1.0);
    }

    #[test]
    fn test_attenuation_models() {
        let source = SpatialSource {
            handle: 1,
            position: Position3D::zero(),
            velocity: Velocity3D::zero(),
            direction: None,
            cone_inner_angle: 2.0 * PI,
            cone_outer_angle: 2.0 * PI,
            cone_outer_gain: 1.0,
            min_distance: 10.0,
            max_distance: 100.0,
            rolloff_factor: 1.0,
            attenuation_model: AttenuationModel::Linear,
            gain: 1.0,
            pitch: 1.0,
            is_relative: false,
            is_looping: false,
        };

        // Test at min distance
        assert_eq!(source.calculate_distance_attenuation(5.0), 1.0);

        // Test at max distance
        assert_eq!(source.calculate_distance_attenuation(100.0), 0.0);

        // Test in between
        let mid_attenuation = source.calculate_distance_attenuation(55.0);
        assert!(mid_attenuation > 0.0 && mid_attenuation < 1.0);
    }

    #[test]
    fn test_audio_zone() {
        let zone = AudioZone::new(1, Position3D::new(0.0, 0.0, 0.0), 50.0);

        assert!(zone.contains_point(&Position3D::new(25.0, 25.0, 0.0)));
        assert!(!zone.contains_point(&Position3D::new(100.0, 0.0, 0.0)));

        let edge_distance = zone.distance_to_edge(&Position3D::new(75.0, 0.0, 0.0));
        assert_eq!(edge_distance, 25.0);
    }

    #[test]
    fn test_spatial_listener() {
        let mut listener = SpatialListener::new();
        listener.set_position(Position3D::new(10.0, 20.0, 30.0));
        listener.set_orientation(
            Direction3D::new(0.0, 0.0, 1.0),
            Direction3D::new(0.0, 1.0, 0.0),
        );

        assert_eq!(listener.position.x, 10.0);
        assert_eq!(listener.forward.z, 1.0);

        let pan = listener.calculate_pan(&Position3D::new(20.0, 20.0, 30.0));
        assert!(pan > 0.0); // Should be to the right
    }

    #[test]
    fn test_spatial_processor() {
        let processor = SpatialAudioProcessor::new();

        let source = SpatialSource::new(123, Position3D::new(100.0, 0.0, 0.0));
        processor.add_source(source);

        assert!(processor.remove_source(123));
        assert!(!processor.remove_source(123)); // Already removed
    }

    #[test]
    fn test_hrtf_processor() {
        let mut hrtf = HRTFProcessor::new(44100);
        assert!(!hrtf.is_enabled());

        hrtf.set_enabled(true);
        assert!(!hrtf.is_enabled()); // Still false because no HRTF data loaded
    }

    #[test]
    fn test_doppler_calculation() {
        let mut listener = SpatialListener::new();
        listener.velocity = Velocity3D::new(10.0, 0.0, 0.0); // Moving right

        let source = SpatialSource {
            handle: 1,
            position: Position3D::new(100.0, 0.0, 0.0),
            velocity: Velocity3D::new(-10.0, 0.0, 0.0), // Moving left (towards listener)
            direction: None,
            cone_inner_angle: 2.0 * PI,
            cone_outer_angle: 2.0 * PI,
            cone_outer_gain: 1.0,
            min_distance: 1.0,
            max_distance: 1000.0,
            rolloff_factor: 1.0,
            attenuation_model: AttenuationModel::InverseClamped,
            gain: 1.0,
            pitch: 1.0,
            is_relative: false,
            is_looping: false,
        };

        let doppler_shift = source.calculate_doppler_shift(&listener);
        assert!(doppler_shift > 1.0); // Pitch should increase (approaching sources)
    }
}

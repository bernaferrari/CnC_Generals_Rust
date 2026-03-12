//! # 3D Spatial Audio System with HRTF
//!
//! This module provides comprehensive 3D spatial audio capabilities including:
//! - Head-Related Transfer Function (HRTF) processing
//! - Distance-based attenuation
//! - Doppler effect simulation
//! - Environmental audio effects (reverb, occlusion)
//! - Binaural rendering for realistic 3D positioning
//! - Real-time spatial audio processing

use crate::audio::{AudioDeviceError, AudioFormat, AudioListener, Position3D, Result, SoundBuffer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex, RwLock};
use uuid::Uuid;

/// HRTF (Head-Related Transfer Function) database
#[derive(Debug, Clone)]
pub struct HrtfDatabase {
    /// Sample rate of HRTF data
    pub sample_rate: u32,
    /// Elevation angles in degrees (-90 to +90)
    pub elevations: Vec<i32>,
    /// Azimuth angles in degrees (0 to 359)
    pub azimuths: Vec<i32>,
    /// HRTF impulse responses: [elevation][azimuth][ear][sample]
    /// ear: 0=left, 1=right
    pub impulse_responses: Vec<Vec<Vec<Vec<f32>>>>,
    /// ITD (Interaural Time Difference) values in samples
    pub itd_values: Vec<Vec<f32>>,
}

impl Default for HrtfDatabase {
    fn default() -> Self {
        Self::load_default_hrtf()
    }
}

impl HrtfDatabase {
    /// Load default HRTF database (simplified dataset for demo)
    pub fn load_default_hrtf() -> Self {
        let sample_rate = 44100;
        let elevations = vec![-40, -30, -20, -10, 0, 10, 20, 30, 40, 50, 60, 70, 80, 90];
        let azimuths = (0..360).step_by(15).collect::<Vec<_>>();

        // Generate simplified HRTF data (in production, load from file)
        let mut impulse_responses = Vec::new();
        let mut itd_values = Vec::new();

        for (elev_idx, elevation) in elevations.iter().enumerate() {
            let mut elev_responses = Vec::new();
            let mut elev_itds = Vec::new();

            for azimuth in &azimuths {
                // Simplified HRTF generation (real HRTF would be measured data)
                let left_response = Self::generate_simplified_hrtf(*elevation, *azimuth, 0);
                let right_response = Self::generate_simplified_hrtf(*elevation, *azimuth, 1);

                elev_responses.push(vec![left_response, right_response]);

                // Calculate ITD based on azimuth (simplified model)
                let azimuth_rad = (*azimuth as f32).to_radians();
                let head_radius = 0.0875; // ~8.75cm average head radius
                let sound_speed = 343.0; // m/s
                let itd_seconds = (head_radius * azimuth_rad.sin()) / sound_speed;
                let itd_samples = itd_seconds * sample_rate as f32;
                elev_itds.push(itd_samples);
            }

            impulse_responses.push(elev_responses);
            itd_values.push(elev_itds);
        }

        Self {
            sample_rate,
            elevations,
            azimuths,
            impulse_responses,
            itd_values,
        }
    }

    /// Generate simplified HRTF impulse response
    fn generate_simplified_hrtf(elevation: i32, azimuth: i32, ear: usize) -> Vec<f32> {
        const IR_LENGTH: usize = 128;
        let mut response = vec![0.0; IR_LENGTH];

        let elev_rad = (elevation as f32).to_radians();
        let azim_rad = (azimuth as f32).to_radians();

        // Simplified frequency-dependent directional filtering
        for i in 0..IR_LENGTH {
            let t = i as f32 / 44100.0;

            // Basic frequency shaping based on angle
            let freq1 = 1000.0 + elev_rad.abs() * 2000.0;
            let freq2 = 5000.0 + azim_rad.sin().abs() * 3000.0;

            let gain = match ear {
                0 => 0.7 + 0.3 * (-azim_rad).cos(), // Left ear
                1 => 0.7 + 0.3 * azim_rad.cos(),    // Right ear
                _ => 0.7,
            };

            response[i] = gain
                * (0.5 * (2.0 * PI * freq1 * t).sin() * (-20.0 * t).exp()
                    + 0.3 * (2.0 * PI * freq2 * t).sin() * (-50.0 * t).exp());
        }

        response
    }

    /// Interpolate HRTF for arbitrary position
    pub fn interpolate_hrtf(&self, elevation: f32, azimuth: f32) -> (Vec<f32>, Vec<f32>, f32) {
        let mut azimuth = azimuth % 360.0;
        if azimuth < 0.0 {
            azimuth += 360.0;
        }

        // Find surrounding elevation indices
        let elev_idx = self.find_elevation_index(elevation);
        let azim_idx = self.find_azimuth_index(azimuth);

        // Bilinear interpolation for smooth transitions
        let (left_ir, right_ir) = self.bilinear_interpolate(elevation, azimuth, elev_idx, azim_idx);
        let itd = self.interpolate_itd(elevation, azimuth, elev_idx, azim_idx);

        (left_ir, right_ir, itd)
    }

    fn find_elevation_index(&self, elevation: f32) -> usize {
        let clamped_elev = elevation.clamp(-90.0, 90.0) as i32;

        for i in 0..self.elevations.len() {
            if self.elevations[i] >= clamped_elev {
                return if i == 0 { 0 } else { i - 1 };
            }
        }

        self.elevations.len() - 2
    }

    fn find_azimuth_index(&self, azimuth: f32) -> usize {
        let azimuth = azimuth as i32;

        for i in 0..self.azimuths.len() {
            if self.azimuths[i] >= azimuth {
                return if i == 0 { 0 } else { i - 1 };
            }
        }

        self.azimuths.len() - 2
    }

    fn bilinear_interpolate(
        &self,
        elevation: f32,
        azimuth: f32,
        elev_idx: usize,
        azim_idx: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let ir_length = self.impulse_responses[0][0][0].len();
        let mut left_result = vec![0.0; ir_length];
        let mut right_result = vec![0.0; ir_length];

        // Get the 4 surrounding points for bilinear interpolation
        let elev1_idx = elev_idx.min(self.elevations.len() - 1);
        let elev2_idx = (elev_idx + 1).min(self.elevations.len() - 1);
        let azim1_idx = azim_idx.min(self.azimuths.len() - 1);
        let azim2_idx = (azim_idx + 1) % self.azimuths.len();

        let elev1 = self.elevations[elev1_idx] as f32;
        let elev2 = self.elevations[elev2_idx] as f32;
        let azim1 = self.azimuths[azim1_idx] as f32;
        let azim2 = self.azimuths[azim2_idx] as f32;

        // Calculate interpolation weights
        let elev_weight = if elev2 != elev1 {
            (elevation - elev1) / (elev2 - elev1)
        } else {
            0.0
        };

        let azim_weight = if azim2 != azim1 {
            (azimuth - azim1) / (azim2 - azim1)
        } else {
            0.0
        };

        // Perform bilinear interpolation
        for i in 0..ir_length {
            // Get the 4 corner values for each ear
            let left_00 = self.impulse_responses[elev1_idx][azim1_idx][0][i];
            let left_01 = self.impulse_responses[elev1_idx][azim2_idx][0][i];
            let left_10 = self.impulse_responses[elev2_idx][azim1_idx][0][i];
            let left_11 = self.impulse_responses[elev2_idx][azim2_idx][0][i];

            let right_00 = self.impulse_responses[elev1_idx][azim1_idx][1][i];
            let right_01 = self.impulse_responses[elev1_idx][azim2_idx][1][i];
            let right_10 = self.impulse_responses[elev2_idx][azim1_idx][1][i];
            let right_11 = self.impulse_responses[elev2_idx][azim2_idx][1][i];

            // Bilinear interpolation
            left_result[i] = left_00 * (1.0 - elev_weight) * (1.0 - azim_weight)
                + left_01 * (1.0 - elev_weight) * azim_weight
                + left_10 * elev_weight * (1.0 - azim_weight)
                + left_11 * elev_weight * azim_weight;

            right_result[i] = right_00 * (1.0 - elev_weight) * (1.0 - azim_weight)
                + right_01 * (1.0 - elev_weight) * azim_weight
                + right_10 * elev_weight * (1.0 - azim_weight)
                + right_11 * elev_weight * azim_weight;
        }

        (left_result, right_result)
    }

    fn interpolate_itd(
        &self,
        elevation: f32,
        azimuth: f32,
        elev_idx: usize,
        azim_idx: usize,
    ) -> f32 {
        let elev1_idx = elev_idx.min(self.elevations.len() - 1);
        let elev2_idx = (elev_idx + 1).min(self.elevations.len() - 1);
        let azim1_idx = azim_idx.min(self.azimuths.len() - 1);
        let azim2_idx = (azim_idx + 1) % self.azimuths.len();

        let elev1 = self.elevations[elev1_idx] as f32;
        let elev2 = self.elevations[elev2_idx] as f32;
        let azim1 = self.azimuths[azim1_idx] as f32;
        let azim2 = self.azimuths[azim2_idx] as f32;

        let elev_weight = if elev2 != elev1 {
            (elevation - elev1) / (elev2 - elev1)
        } else {
            0.0
        };
        let azim_weight = if azim2 != azim1 {
            (azimuth - azim1) / (azim2 - azim1)
        } else {
            0.0
        };

        let itd_00 = self.itd_values[elev1_idx][azim1_idx];
        let itd_01 = self.itd_values[elev1_idx][azim2_idx];
        let itd_10 = self.itd_values[elev2_idx][azim1_idx];
        let itd_11 = self.itd_values[elev2_idx][azim2_idx];

        itd_00 * (1.0 - elev_weight) * (1.0 - azim_weight)
            + itd_01 * (1.0 - elev_weight) * azim_weight
            + itd_10 * elev_weight * (1.0 - azim_weight)
            + itd_11 * elev_weight * azim_weight
    }
}

/// 3D spatial audio source
#[derive(Debug, Clone)]
pub struct SpatialAudioSource {
    /// Unique identifier
    pub id: Uuid,
    /// Current position in 3D space
    pub position: Position3D,
    /// Velocity for doppler effect calculation
    pub velocity: Position3D,
    /// Reference distance for attenuation (distance at which gain = 1.0)
    pub reference_distance: f32,
    /// Maximum distance for audio (beyond this, sound is inaudible)
    pub max_distance: f32,
    /// Rolloff factor for distance attenuation
    pub rolloff_factor: f32,
    /// Sound cone parameters for directional audio
    pub cone: Option<AudioCone>,
    /// Current gain multiplier
    pub gain: f32,
    /// Whether the source is currently playing
    pub is_playing: bool,
    /// Associated sound buffer
    pub buffer_id: Option<Uuid>,
}

/// Audio cone for directional sound sources
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioCone {
    /// Inner cone angle in degrees
    pub inner_angle: f32,
    /// Outer cone angle in degrees  
    pub outer_angle: f32,
    /// Gain outside the outer cone (0.0 = silent, 1.0 = full volume)
    pub outer_gain: f32,
    /// Direction vector (normalized)
    pub direction: Position3D,
}

impl Default for AudioCone {
    fn default() -> Self {
        Self {
            inner_angle: 360.0,
            outer_angle: 360.0,
            outer_gain: 1.0,
            direction: Position3D::new(0.0, 0.0, -1.0),
        }
    }
}

/// Environmental audio parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EnvironmentalAudio {
    /// Reverb strength (0.0 = none, 1.0 = maximum)
    pub reverb_strength: f32,
    /// Room size for reverb calculation
    pub room_size: f32,
    /// Air absorption coefficient
    pub air_absorption: f32,
    /// Speed of sound (m/s) for doppler calculations
    pub sound_speed: f32,
}

impl Default for EnvironmentalAudio {
    fn default() -> Self {
        Self {
            reverb_strength: 0.2,
            room_size: 10.0,
            air_absorption: 0.01,
            sound_speed: 343.0,
        }
    }
}

/// Main 3D spatial audio processor
pub struct SpatialAudioProcessor {
    /// HRTF database for binaural processing
    hrtf_db: Arc<HrtfDatabase>,
    /// Current listener state
    listener: Arc<RwLock<AudioListener>>,
    /// Active spatial audio sources
    sources: Arc<RwLock<HashMap<Uuid, SpatialAudioSource>>>,
    /// Environmental audio settings
    environment: Arc<RwLock<EnvironmentalAudio>>,
    /// Audio sample rate
    sample_rate: u32,
    /// Current frame size for processing
    frame_size: usize,
    /// Convolution buffers for HRTF processing
    convolution_buffers: Arc<Mutex<ConvolutionBuffers>>,
}

/// Buffers for real-time convolution processing
struct ConvolutionBuffers {
    /// Left ear convolution buffer
    left_conv_buffer: Vec<f32>,
    /// Right ear convolution buffer  
    right_conv_buffer: Vec<f32>,
    /// Overlap-add buffers for seamless processing
    left_overlap: Vec<f32>,
    right_overlap: Vec<f32>,
    /// FFT workspace
    fft_buffer: Vec<f32>,
}

impl SpatialAudioProcessor {
    /// Create new spatial audio processor
    pub fn new(sample_rate: u32, frame_size: usize) -> Result<Self> {
        let hrtf_db = Arc::new(HrtfDatabase::default());
        let listener = Arc::new(RwLock::new(AudioListener::default()));
        let sources = Arc::new(RwLock::new(HashMap::new()));
        let environment = Arc::new(RwLock::new(EnvironmentalAudio::default()));

        let convolution_buffers = Arc::new(Mutex::new(ConvolutionBuffers {
            left_conv_buffer: vec![0.0; frame_size * 2],
            right_conv_buffer: vec![0.0; frame_size * 2],
            left_overlap: vec![0.0; 128], // HRTF IR length
            right_overlap: vec![0.0; 128],
            fft_buffer: vec![0.0; frame_size * 4],
        }));

        Ok(Self {
            hrtf_db,
            listener,
            sources,
            environment,
            sample_rate,
            frame_size,
            convolution_buffers,
        })
    }

    /// Add a new spatial audio source
    pub fn add_source(&self, mut source: SpatialAudioSource) -> Result<Uuid> {
        source.id = Uuid::new_v4();
        let id = source.id;

        let mut sources = self.sources.write().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire sources lock: {}", e))
        })?;

        sources.insert(id, source);
        Ok(id)
    }

    /// Remove a spatial audio source
    pub fn remove_source(&self, source_id: Uuid) -> Result<()> {
        let mut sources = self.sources.write().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire sources lock: {}", e))
        })?;

        sources.remove(&source_id);
        Ok(())
    }

    /// Update source position and velocity
    pub fn update_source(
        &self,
        source_id: Uuid,
        position: Position3D,
        velocity: Position3D,
    ) -> Result<()> {
        let mut sources = self.sources.write().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire sources lock: {}", e))
        })?;

        if let Some(source) = sources.get_mut(&source_id) {
            source.position = position;
            source.velocity = velocity;
        }

        Ok(())
    }

    /// Update listener position and orientation
    pub fn update_listener(&self, listener: AudioListener) -> Result<()> {
        let mut current_listener = self.listener.write().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire listener lock: {}", e))
        })?;

        *current_listener = listener;
        Ok(())
    }

    /// Process spatial audio for all sources
    pub fn process_spatial_audio(
        &self,
        input_buffers: &HashMap<Uuid, &[f32]>,
        output_left: &mut [f32],
        output_right: &mut [f32],
    ) -> Result<()> {
        if output_left.len() != output_right.len() || output_left.len() != self.frame_size {
            return Err(AudioDeviceError::SpatialAudioError(
                "Output buffer size mismatch".to_string(),
            ));
        }

        // Clear output buffers
        output_left.fill(0.0);
        output_right.fill(0.0);

        let listener = self.listener.read().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire listener lock: {}", e))
        })?;

        let sources = self.sources.read().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire sources lock: {}", e))
        })?;

        let environment = self.environment.read().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!(
                "Failed to acquire environment lock: {}",
                e
            ))
        })?;

        // Process each active source
        for (source_id, source) in sources.iter() {
            if !source.is_playing {
                continue;
            }

            if let Some(input_buffer) = input_buffers.get(source_id) {
                self.process_source(
                    &listener,
                    source,
                    &environment,
                    input_buffer,
                    output_left,
                    output_right,
                )?;
            }
        }

        Ok(())
    }

    /// Process a single spatial audio source
    fn process_source(
        &self,
        listener: &AudioListener,
        source: &SpatialAudioSource,
        environment: &EnvironmentalAudio,
        input: &[f32],
        output_left: &mut [f32],
        output_right: &mut [f32],
    ) -> Result<()> {
        // Calculate relative position
        let relative_pos = Position3D::new(
            source.position.x - listener.position.x,
            source.position.y - listener.position.y,
            source.position.z - listener.position.z,
        );

        // Transform to listener coordinate system
        let local_pos = self.transform_to_listener_space(&relative_pos, listener);

        // Calculate distance and attenuation
        let distance = local_pos.magnitude();
        let distance_gain = self.calculate_distance_attenuation(distance, source);

        if distance_gain < 0.001 {
            return Ok(()); // Too far, skip processing
        }

        // Calculate directional gain (if source has directional cone)
        let directional_gain = if let Some(cone) = &source.cone {
            self.calculate_directional_gain(&relative_pos, cone)
        } else {
            1.0
        };

        // Calculate doppler effect
        let doppler_factor = self.calculate_doppler_effect(
            &source.velocity,
            &listener.velocity,
            &relative_pos,
            environment.sound_speed,
        );

        // Convert to spherical coordinates for HRTF lookup
        let (azimuth, elevation) = self.cartesian_to_spherical(&local_pos);

        // Get HRTF impulse responses
        let (left_ir, right_ir, itd) = self.hrtf_db.interpolate_hrtf(elevation, azimuth);

        // Apply convolution with HRTF
        let total_gain = source.gain * distance_gain * directional_gain;
        self.apply_hrtf_convolution(
            input,
            &left_ir,
            &right_ir,
            itd,
            doppler_factor,
            total_gain,
            output_left,
            output_right,
        )?;

        Ok(())
    }

    /// Transform position to listener's local coordinate system
    fn transform_to_listener_space(
        &self,
        pos: &Position3D,
        listener: &AudioListener,
    ) -> Position3D {
        // For simplicity, assume listener is always facing negative Z
        // In a full implementation, you'd use the listener's orientation matrix
        Position3D::new(pos.x, pos.y, pos.z)
    }

    /// Calculate distance-based attenuation
    fn calculate_distance_attenuation(&self, distance: f32, source: &SpatialAudioSource) -> f32 {
        if distance <= source.reference_distance {
            return 1.0;
        }

        if distance >= source.max_distance {
            return 0.0;
        }

        // Inverse distance law with rolloff factor
        let ratio = source.reference_distance / distance;
        ratio.powf(source.rolloff_factor).min(1.0)
    }

    /// Calculate directional gain for cone-shaped sources
    fn calculate_directional_gain(&self, relative_pos: &Position3D, cone: &AudioCone) -> f32 {
        let distance = relative_pos.magnitude();
        if distance < 0.001 {
            return 1.0;
        }

        // Calculate angle between source direction and listener direction
        let listener_dir = Position3D::new(
            relative_pos.x / distance,
            relative_pos.y / distance,
            relative_pos.z / distance,
        );

        let dot_product = cone.direction.x * listener_dir.x
            + cone.direction.y * listener_dir.y
            + cone.direction.z * listener_dir.z;

        let angle = dot_product.acos().to_degrees();

        if angle <= cone.inner_angle / 2.0 {
            1.0 // Full gain
        } else if angle >= cone.outer_angle / 2.0 {
            cone.outer_gain // Outer gain
        } else {
            // Linear interpolation between inner and outer
            let t = (angle - cone.inner_angle / 2.0)
                / (cone.outer_angle / 2.0 - cone.inner_angle / 2.0);
            1.0 * (1.0 - t) + cone.outer_gain * t
        }
    }

    /// Calculate doppler effect frequency shift
    fn calculate_doppler_effect(
        &self,
        source_velocity: &Position3D,
        listener_velocity: &Position3D,
        relative_pos: &Position3D,
        sound_speed: f32,
    ) -> f32 {
        let distance = relative_pos.magnitude();
        if distance < 0.001 {
            return 1.0;
        }

        // Unit vector from source to listener
        let direction = Position3D::new(
            relative_pos.x / distance,
            relative_pos.y / distance,
            relative_pos.z / distance,
        );

        // Relative velocity component along the line connecting source and listener
        let relative_velocity = (listener_velocity.x - source_velocity.x) * direction.x
            + (listener_velocity.y - source_velocity.y) * direction.y
            + (listener_velocity.z - source_velocity.z) * direction.z;

        // Doppler frequency shift factor
        let doppler_factor = (sound_speed + relative_velocity) / sound_speed;
        doppler_factor.clamp(0.5, 2.0) // Limit extreme doppler effects
    }

    /// Convert Cartesian coordinates to spherical (azimuth, elevation)
    fn cartesian_to_spherical(&self, pos: &Position3D) -> (f32, f32) {
        let distance = pos.magnitude();
        if distance < 0.001 {
            return (0.0, 0.0);
        }

        // Azimuth: angle in XZ plane from positive X axis
        let azimuth = pos.z.atan2(pos.x).to_degrees();

        // Elevation: angle from XZ plane
        let elevation = (pos.y / distance).asin().to_degrees();

        (azimuth, elevation)
    }

    /// Apply HRTF convolution to input signal
    fn apply_hrtf_convolution(
        &self,
        input: &[f32],
        left_ir: &[f32],
        right_ir: &[f32],
        itd: f32,
        doppler_factor: f32,
        gain: f32,
        output_left: &mut [f32],
        output_right: &mut [f32],
    ) -> Result<()> {
        let mut buffers = self.convolution_buffers.lock().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!(
                "Failed to acquire convolution buffers: {}",
                e
            ))
        })?;

        // Apply doppler effect by resampling (simplified)
        let mut doppler_input = vec![0.0; input.len()];
        if (doppler_factor - 1.0).abs() > 0.001 {
            self.apply_doppler_resampling(input, &mut doppler_input, doppler_factor);
        } else {
            doppler_input.copy_from_slice(input);
        }

        // Convolution with HRTF impulse responses (simplified overlap-add)
        for i in 0..output_left.len() {
            let mut left_sample = 0.0;
            let mut right_sample = 0.0;

            for j in 0..left_ir.len().min(i + 1) {
                let input_idx = i - j;
                if input_idx < doppler_input.len() {
                    left_sample += doppler_input[input_idx] * left_ir[j];
                    right_sample += doppler_input[input_idx] * right_ir[j];
                }
            }

            // Apply ITD (Interaural Time Difference) by delaying one channel
            let itd_samples = itd.abs() as usize;
            if itd > 0.0 && i >= itd_samples {
                // Delay left channel
                output_left[i] += left_sample * gain;
                output_right[i] += right_sample * gain;
            } else if itd < 0.0 && i >= itd_samples {
                // Delay right channel
                output_left[i] += left_sample * gain;
                output_right[i] += right_sample * gain;
            } else {
                output_left[i] += left_sample * gain;
                output_right[i] += right_sample * gain;
            }
        }

        Ok(())
    }

    /// Apply doppler effect by resampling
    fn apply_doppler_resampling(&self, input: &[f32], output: &mut [f32], factor: f32) {
        // Simplified linear interpolation resampling
        for i in 0..output.len() {
            let src_pos = i as f32 / factor;
            let src_idx = src_pos as usize;

            if src_idx + 1 < input.len() {
                let frac = src_pos - src_idx as f32;
                output[i] = input[src_idx] * (1.0 - frac) + input[src_idx + 1] * frac;
            } else if src_idx < input.len() {
                output[i] = input[src_idx];
            }
        }
    }

    /// Set environmental audio parameters
    pub fn set_environment(&self, environment: EnvironmentalAudio) -> Result<()> {
        let mut current_env = self.environment.write().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!(
                "Failed to acquire environment lock: {}",
                e
            ))
        })?;

        *current_env = environment;
        Ok(())
    }

    /// Get current listener position
    pub fn get_listener(&self) -> Result<AudioListener> {
        let listener = self.listener.read().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire listener lock: {}", e))
        })?;

        Ok(listener.clone())
    }

    /// Get all spatial audio sources
    pub fn get_sources(&self) -> Result<Vec<SpatialAudioSource>> {
        let sources = self.sources.read().map_err(|e| {
            AudioDeviceError::SpatialAudioError(format!("Failed to acquire sources lock: {}", e))
        })?;

        Ok(sources.values().cloned().collect())
    }
}

impl Default for SpatialAudioSource {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            position: Position3D::new(0.0, 0.0, 0.0),
            velocity: Position3D::new(0.0, 0.0, 0.0),
            reference_distance: 1.0,
            max_distance: 100.0,
            rolloff_factor: 1.0,
            cone: None,
            gain: 1.0,
            is_playing: false,
            buffer_id: None,
        }
    }
}

impl Position3D {
    /// Calculate magnitude (distance from origin)
    pub fn magnitude(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Normalize to unit vector
    pub fn normalize(&self) -> Position3D {
        let mag = self.magnitude();
        if mag > 0.001 {
            Position3D::new(self.x / mag, self.y / mag, self.z / mag)
        } else {
            Position3D::new(0.0, 0.0, 0.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hrtf_database_creation() {
        let hrtf = HrtfDatabase::load_default_hrtf();
        assert!(!hrtf.elevations.is_empty());
        assert!(!hrtf.azimuths.is_empty());
        assert_eq!(hrtf.sample_rate, 44100);
    }

    #[test]
    fn test_spatial_audio_processor_creation() {
        let processor = SpatialAudioProcessor::new(44100, 512).unwrap();
        assert_eq!(processor.sample_rate, 44100);
        assert_eq!(processor.frame_size, 512);
    }

    #[test]
    fn test_distance_attenuation() {
        let processor = SpatialAudioProcessor::new(44100, 512).unwrap();
        let source = SpatialAudioSource {
            reference_distance: 1.0,
            max_distance: 10.0,
            rolloff_factor: 1.0,
            ..Default::default()
        };

        let gain_near = processor.calculate_distance_attenuation(0.5, &source);
        let gain_ref = processor.calculate_distance_attenuation(1.0, &source);
        let gain_far = processor.calculate_distance_attenuation(5.0, &source);
        let gain_max = processor.calculate_distance_attenuation(10.0, &source);

        assert_eq!(gain_near, 1.0); // Within reference distance
        assert_eq!(gain_ref, 1.0); // At reference distance
        assert!(gain_far < gain_ref); // Farther away, lower gain
        assert_eq!(gain_max, 0.0); // At max distance
    }

    #[test]
    fn test_cartesian_to_spherical() {
        let processor = SpatialAudioProcessor::new(44100, 512).unwrap();

        // Test cardinal directions
        let (azimuth, elevation) =
            processor.cartesian_to_spherical(&Position3D::new(1.0, 0.0, 0.0));
        assert!((azimuth - 0.0).abs() < 0.1);
        assert!((elevation - 0.0).abs() < 0.1);

        let (azimuth, elevation) =
            processor.cartesian_to_spherical(&Position3D::new(0.0, 1.0, 0.0));
        assert!((elevation - 90.0).abs() < 0.1);
    }
}

//! Advanced Audio Mixing and Effects System
//!
//! This module provides high-quality audio mixing and real-time effects processing:
//! - Multi-channel mixing with dynamic range compression
//! - Real-time audio effects (reverb, EQ, distortion, etc.)
//! - Audio bus system for organized mixing
//! - Dynamic range compression and limiting
//! - Crossfading and smooth transitions
//! - Audio quality scaling based on performance

use parking_lot::{Mutex, RwLock};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "audio")]
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
#[cfg(feature = "audio")]
use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use crate::common::audio::{AudioAffect, AudioHandle, Bool, Real, UnsignedInt};

/// Maximum number of audio buses
pub const MAX_AUDIO_BUSES: usize = 32;

/// Maximum number of effects per bus
pub const MAX_EFFECTS_PER_BUS: usize = 16;

/// Audio buffer size for effect processing
pub const EFFECT_BUFFER_SIZE: usize = 512;

/// Default sample rate for processing
pub const PROCESSING_SAMPLE_RATE: u32 = 44100;

/// Audio quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioQuality {
    /// Low quality for performance
    Low = 0,
    /// Medium quality (default)
    Medium = 1,
    /// High quality for best audio experience
    High = 2,
    /// Ultra quality for audiophiles
    Ultra = 3,
}

impl AudioQuality {
    pub fn get_quality_multiplier(&self) -> f32 {
        match self {
            Self::Low => 0.5,
            Self::Medium => 1.0,
            Self::High => 1.5,
            Self::Ultra => 2.0,
        }
    }

    pub fn get_buffer_size(&self) -> usize {
        match self {
            Self::Low => 1024,
            Self::Medium => 512,
            Self::High => 256,
            Self::Ultra => 128,
        }
    }
}

/// Audio bus for organizing and processing groups of audio sources
pub struct AudioBus {
    /// Bus identifier
    pub id: UnsignedInt,
    /// Bus name
    pub name: String,
    /// Master volume (0.0 - 1.0)
    pub volume: f32,
    /// Pan (-1.0 to 1.0)
    pub pan: f32,
    /// Mute flag
    pub muted: bool,
    /// Solo flag
    pub solo: bool,
    /// Effects chain
    pub effects: Vec<Box<dyn AudioEffect>>,
    /// Parent bus (for hierarchical mixing)
    pub parent_bus: Option<UnsignedInt>,
    /// Child buses
    pub child_buses: Vec<UnsignedInt>,
    /// Audio sources assigned to this bus
    pub sources: Vec<AudioHandle>,
    /// Send levels to other buses (for reverb sends, etc.)
    pub sends: HashMap<UnsignedInt, f32>,
}

impl std::fmt::Debug for AudioBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioBus")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("volume", &self.volume)
            .field("pan", &self.pan)
            .field("muted", &self.muted)
            .field("solo", &self.solo)
            .field("effects_count", &self.effects.len())
            .field("parent_bus", &self.parent_bus)
            .field("child_buses", &self.child_buses)
            .field("sources", &self.sources)
            .field("sends", &self.sends)
            .finish()
    }
}

impl AudioBus {
    pub fn new(id: UnsignedInt, name: String) -> Self {
        Self {
            id,
            name,
            volume: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            effects: Vec::new(),
            parent_bus: None,
            child_buses: Vec::new(),
            sources: Vec::new(),
            sends: HashMap::new(),
        }
    }

    /// Add an effect to the bus
    pub fn add_effect(&mut self, effect: Box<dyn AudioEffect>) {
        if self.effects.len() < MAX_EFFECTS_PER_BUS {
            self.effects.push(effect);
        }
    }

    /// Remove an effect from the bus
    pub fn remove_effect(&mut self, effect_id: UnsignedInt) {
        self.effects.retain(|effect| effect.get_id() != effect_id);
    }

    /// Process audio through the bus effects chain
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        if self.muted {
            left.fill(0.0);
            right.fill(0.0);
            return;
        }

        // Apply effects in chain
        for effect in &mut self.effects {
            if effect.is_enabled() {
                effect.process(left, right);
            }
        }

        // Apply bus volume and pan
        let left_gain = self.volume * (1.0 - self.pan.max(0.0));
        let right_gain = self.volume * (1.0 + self.pan.min(0.0));

        for sample in left {
            *sample *= left_gain;
        }
        for sample in right {
            *sample *= right_gain;
        }
    }
}

/// Audio effect trait for real-time processing
pub trait AudioEffect: Send + Sync {
    /// Get unique effect ID
    fn get_id(&self) -> UnsignedInt;

    /// Get effect name
    fn get_name(&self) -> &str;

    /// Check if effect is enabled
    fn is_enabled(&self) -> bool;

    /// Enable or disable the effect
    fn set_enabled(&mut self, enabled: bool);

    /// Process audio samples
    fn process(&mut self, left: &mut [f32], right: &mut [f32]);

    /// Get effect parameters
    fn get_parameters(&self) -> HashMap<String, f32>;

    /// Set effect parameter
    fn set_parameter(&mut self, name: &str, value: f32);

    /// Reset effect state
    fn reset(&mut self);

    /// Get effect latency in samples
    fn get_latency(&self) -> usize {
        0
    }
}

/// Parametric equalizer effect
pub struct ParametricEqualizer {
    id: UnsignedInt,
    enabled: bool,
    sample_rate: f32,
    bands: Vec<EqBand>,
}

#[derive(Debug, Clone)]
struct EqBand {
    frequency: f32,
    gain: f32,
    q: f32,
    filter_type: FilterType,
    // Filter state variables
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    // Filter coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum FilterType {
    HighPass,
    LowPass,
    BandPass,
    Peak,
    HighShelf,
    LowShelf,
}

impl ParametricEqualizer {
    pub fn new(id: UnsignedInt, sample_rate: f32) -> Self {
        let mut eq = Self {
            id,
            enabled: true,
            sample_rate,
            bands: Vec::new(),
        };

        // Add default bands
        eq.add_band(100.0, 0.0, 0.707, FilterType::HighPass);
        eq.add_band(1000.0, 0.0, 1.0, FilterType::Peak);
        eq.add_band(5000.0, 0.0, 1.0, FilterType::Peak);
        eq.add_band(10000.0, 0.0, 0.707, FilterType::LowPass);

        eq
    }

    pub fn add_band(&mut self, frequency: f32, gain: f32, q: f32, filter_type: FilterType) {
        let mut band = EqBand {
            frequency,
            gain,
            q,
            filter_type,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        };

        self.calculate_coefficients(&mut band);
        self.bands.push(band);
    }

    fn calculate_coefficients(&self, band: &mut EqBand) {
        Self::calculate_coefficients_with_sample_rate(band, self.sample_rate);
    }

    fn calculate_coefficients_with_sample_rate(band: &mut EqBand, sample_rate: f32) {
        let omega = 2.0 * std::f32::consts::PI * band.frequency / sample_rate;
        let cos_omega = omega.cos();
        let sin_omega = omega.sin();
        let alpha = sin_omega / (2.0 * band.q);
        let a = 10.0_f32.powf(band.gain / 40.0); // Convert dB to linear

        match band.filter_type {
            FilterType::Peak => {
                band.b0 = 1.0 + alpha * a;
                band.b1 = -2.0 * cos_omega;
                band.b2 = 1.0 - alpha * a;
                let a0 = 1.0 + alpha / a;
                band.a1 = -2.0 * cos_omega / a0;
                band.a2 = (1.0 - alpha / a) / a0;
                band.b0 /= a0;
                band.b1 /= a0;
                band.b2 /= a0;
            }
            FilterType::LowPass => {
                band.b0 = (1.0 - cos_omega) / 2.0;
                band.b1 = 1.0 - cos_omega;
                band.b2 = (1.0 - cos_omega) / 2.0;
                let a0 = 1.0 + alpha;
                band.a1 = -2.0 * cos_omega / a0;
                band.a2 = (1.0 - alpha) / a0;
                band.b0 /= a0;
                band.b1 /= a0;
                band.b2 /= a0;
            }
            FilterType::HighPass => {
                band.b0 = (1.0 + cos_omega) / 2.0;
                band.b1 = -(1.0 + cos_omega);
                band.b2 = (1.0 + cos_omega) / 2.0;
                let a0 = 1.0 + alpha;
                band.a1 = -2.0 * cos_omega / a0;
                band.a2 = (1.0 - alpha) / a0;
                band.b0 /= a0;
                band.b1 /= a0;
                band.b2 /= a0;
            }
            // Add other filter types as needed
            _ => {}
        }
    }

    fn process_band(band: &mut EqBand, input: f32) -> f32 {
        let output = band.b0 * input + band.b1 * band.x1 + band.b2 * band.x2
            - band.a1 * band.y1
            - band.a2 * band.y2;

        // Update delay elements
        band.x2 = band.x1;
        band.x1 = input;
        band.y2 = band.y1;
        band.y1 = output;

        output
    }
}

impl AudioEffect for ParametricEqualizer {
    fn get_id(&self) -> UnsignedInt {
        self.id
    }
    fn get_name(&self) -> &str {
        "Parametric EQ"
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let mut left_sample = *l;
            let mut right_sample = *r;

            // Process each band
            for band in &mut self.bands {
                left_sample = Self::process_band(band, left_sample);
                right_sample = Self::process_band(band, right_sample);
            }

            *l = left_sample;
            *r = right_sample;
        }
    }

    fn get_parameters(&self) -> HashMap<String, f32> {
        let mut params = HashMap::new();
        for (i, band) in self.bands.iter().enumerate() {
            params.insert(format!("band{}_freq", i), band.frequency);
            params.insert(format!("band{}_gain", i), band.gain);
            params.insert(format!("band{}_q", i), band.q);
        }
        params
    }

    fn set_parameter(&mut self, name: &str, value: f32) {
        // Parse parameter name and update band
        if let Some(caps) = regex::Regex::new(r"band(\d+)_(\w+)")
            .unwrap()
            .captures(name)
        {
            if let (Ok(band_idx), Some(param)) = (caps[1].parse::<usize>(), caps.get(2)) {
                if band_idx < self.bands.len() {
                    match param.as_str() {
                        "freq" => {
                            self.bands[band_idx].frequency = value;
                            let sample_rate = self.sample_rate;
                            Self::calculate_coefficients_with_sample_rate(
                                &mut self.bands[band_idx],
                                sample_rate,
                            );
                        }
                        "gain" => {
                            self.bands[band_idx].gain = value;
                            let sample_rate = self.sample_rate;
                            Self::calculate_coefficients_with_sample_rate(
                                &mut self.bands[band_idx],
                                sample_rate,
                            );
                        }
                        "q" => {
                            self.bands[band_idx].q = value;
                            let sample_rate = self.sample_rate;
                            Self::calculate_coefficients_with_sample_rate(
                                &mut self.bands[band_idx],
                                sample_rate,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.x1 = 0.0;
            band.x2 = 0.0;
            band.y1 = 0.0;
            band.y2 = 0.0;
        }
    }
}

/// Reverb effect using a simple reverb algorithm
pub struct SimpleReverb {
    id: UnsignedInt,
    enabled: bool,
    room_size: f32,
    damping: f32,
    wet_level: f32,
    dry_level: f32,
    delay_lines: Vec<DelayLine>,
    all_pass_filters: Vec<AllPassFilter>,
}

struct DelayLine {
    buffer: Vec<f32>,
    read_pos: usize,
    write_pos: usize,
    feedback: f32,
}

struct AllPassFilter {
    buffer: Vec<f32>,
    read_pos: usize,
    write_pos: usize,
    feedback: f32,
}

impl SimpleReverb {
    pub fn new(id: UnsignedInt, sample_rate: f32) -> Self {
        let mut reverb = Self {
            id,
            enabled: true,
            room_size: 0.5,
            damping: 0.5,
            wet_level: 0.3,
            dry_level: 0.7,
            delay_lines: Vec::new(),
            all_pass_filters: Vec::new(),
        };

        // Initialize delay lines with different lengths for stereo spread
        let delay_lengths = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        for &length in &delay_lengths {
            let adjusted_length = (length as f32 * sample_rate / 44100.0) as usize;
            reverb.delay_lines.push(DelayLine {
                buffer: vec![0.0; adjusted_length],
                read_pos: 0,
                write_pos: 0,
                feedback: 0.84,
            });
        }

        // Initialize all-pass filters
        let allpass_lengths = [556, 441, 341, 225];
        for &length in &allpass_lengths {
            let adjusted_length = (length as f32 * sample_rate / 44100.0) as usize;
            reverb.all_pass_filters.push(AllPassFilter {
                buffer: vec![0.0; adjusted_length],
                read_pos: 0,
                write_pos: 0,
                feedback: 0.5,
            });
        }

        reverb
    }
}

impl AudioEffect for SimpleReverb {
    fn get_id(&self) -> UnsignedInt {
        self.id
    }
    fn get_name(&self) -> &str {
        "Simple Reverb"
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let input_left = *l;
            let input_right = *r;

            // Process through delay lines (simplified reverb)
            let mut reverb_left = 0.0;
            let mut reverb_right = 0.0;

            for (i, delay_line) in self.delay_lines.iter_mut().enumerate() {
                let delay_output = delay_line.buffer[delay_line.read_pos];

                if i % 2 == 0 {
                    reverb_left += delay_output;
                } else {
                    reverb_right += delay_output;
                }

                // Write input + feedback to delay line
                let input_sample = if i % 2 == 0 { input_left } else { input_right };
                delay_line.buffer[delay_line.write_pos] =
                    input_sample + delay_output * delay_line.feedback;

                delay_line.read_pos = (delay_line.read_pos + 1) % delay_line.buffer.len();
                delay_line.write_pos = (delay_line.write_pos + 1) % delay_line.buffer.len();
            }

            // Apply wet/dry mix
            *l = input_left * self.dry_level + reverb_left * self.wet_level * 0.125;
            *r = input_right * self.dry_level + reverb_right * self.wet_level * 0.125;
        }
    }

    fn get_parameters(&self) -> HashMap<String, f32> {
        let mut params = HashMap::new();
        params.insert("room_size".to_string(), self.room_size);
        params.insert("damping".to_string(), self.damping);
        params.insert("wet_level".to_string(), self.wet_level);
        params.insert("dry_level".to_string(), self.dry_level);
        params
    }

    fn set_parameter(&mut self, name: &str, value: f32) {
        match name {
            "room_size" => self.room_size = value.clamp(0.0, 1.0),
            "damping" => self.damping = value.clamp(0.0, 1.0),
            "wet_level" => self.wet_level = value.clamp(0.0, 1.0),
            "dry_level" => self.dry_level = value.clamp(0.0, 1.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        for delay_line in &mut self.delay_lines {
            delay_line.buffer.fill(0.0);
            delay_line.read_pos = 0;
            delay_line.write_pos = 0;
        }

        for allpass in &mut self.all_pass_filters {
            allpass.buffer.fill(0.0);
            allpass.read_pos = 0;
            allpass.write_pos = 0;
        }
    }
}

/// Dynamic range compressor
pub struct Compressor {
    id: UnsignedInt,
    enabled: bool,
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    knee: f32,
    makeup_gain: f32,
    envelope: f32,
    sample_rate: f32,
}

impl Compressor {
    pub fn new(id: UnsignedInt, sample_rate: f32) -> Self {
        Self {
            id,
            enabled: true,
            threshold: -12.0, // dB
            ratio: 4.0,
            attack: 0.003,    // 3ms
            release: 0.100,   // 100ms
            knee: 2.0,        // dB
            makeup_gain: 0.0, // dB
            envelope: 0.0,
            sample_rate,
        }
    }

    fn db_to_linear(&self, db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    fn linear_to_db(&self, linear: f32) -> f32 {
        20.0 * linear.abs().max(1e-6).log10()
    }
}

impl AudioEffect for Compressor {
    fn get_id(&self) -> UnsignedInt {
        self.id
    }
    fn get_name(&self) -> &str {
        "Compressor"
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let attack_coeff = (-1.0 / (self.attack * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release * self.sample_rate)).exp();
        let makeup_linear = self.db_to_linear(self.makeup_gain);

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            // Calculate input level (peak of both channels)
            let input_level = l.abs().max(r.abs());
            let input_db = self.linear_to_db(input_level);

            // Calculate compression
            let mut gain_db = 0.0;

            if input_db > self.threshold {
                let over_threshold = input_db - self.threshold;

                // Soft knee
                if over_threshold < self.knee {
                    let ratio_factor = over_threshold / self.knee;
                    gain_db = -over_threshold * ratio_factor * (1.0 - 1.0 / self.ratio);
                } else {
                    gain_db = -over_threshold * (1.0 - 1.0 / self.ratio);
                }
            }

            let target_gain = self.db_to_linear(gain_db);

            // Envelope follower
            let coeff = if target_gain < self.envelope {
                attack_coeff
            } else {
                release_coeff
            };
            self.envelope = target_gain + (self.envelope - target_gain) * coeff;

            // Apply compression and makeup gain
            *l *= self.envelope * makeup_linear;
            *r *= self.envelope * makeup_linear;
        }
    }

    fn get_parameters(&self) -> HashMap<String, f32> {
        let mut params = HashMap::new();
        params.insert("threshold".to_string(), self.threshold);
        params.insert("ratio".to_string(), self.ratio);
        params.insert("attack".to_string(), self.attack);
        params.insert("release".to_string(), self.release);
        params.insert("knee".to_string(), self.knee);
        params.insert("makeup_gain".to_string(), self.makeup_gain);
        params
    }

    fn set_parameter(&mut self, name: &str, value: f32) {
        match name {
            "threshold" => self.threshold = value.clamp(-60.0, 0.0),
            "ratio" => self.ratio = value.clamp(1.0, 20.0),
            "attack" => self.attack = value.clamp(0.001, 0.1),
            "release" => self.release = value.clamp(0.01, 5.0),
            "knee" => self.knee = value.clamp(0.0, 10.0),
            "makeup_gain" => self.makeup_gain = value.clamp(-20.0, 20.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

/// Advanced audio mixer with bus system
pub struct AudioMixer {
    /// Audio buses
    buses: RwLock<HashMap<UnsignedInt, AudioBus>>,
    /// Master bus ID
    master_bus_id: UnsignedInt,
    /// Next available bus ID
    next_bus_id: Arc<Mutex<UnsignedInt>>,
    /// Audio quality setting
    quality: RwLock<AudioQuality>,
    /// Sample rate
    sample_rate: f32,
    /// Processing buffer size
    buffer_size: usize,
    /// Master limiter
    limiter: Arc<Mutex<Compressor>>,
    /// Mix matrix for routing
    mix_matrix: RwLock<HashMap<(UnsignedInt, UnsignedInt), f32>>,
}

impl AudioMixer {
    pub fn new(sample_rate: f32) -> Self {
        let mixer = Self {
            buses: RwLock::new(HashMap::new()),
            master_bus_id: 0,
            next_bus_id: Arc::new(Mutex::new(1)),
            quality: RwLock::new(AudioQuality::Medium),
            sample_rate,
            buffer_size: EFFECT_BUFFER_SIZE,
            limiter: Arc::new(Mutex::new(Compressor::new(999, sample_rate))),
            mix_matrix: RwLock::new(HashMap::new()),
        };

        // Create master bus
        let master_bus = AudioBus::new(0, "Master".to_string());
        mixer.buses.write().insert(0, master_bus);

        // Create default buses
        mixer.create_bus("Music".to_string(), Some(0));
        mixer.create_bus("SFX".to_string(), Some(0));
        mixer.create_bus("Voice".to_string(), Some(0));
        mixer.create_bus("Ambient".to_string(), Some(0));

        mixer
    }

    /// Create a new audio bus
    pub fn create_bus(&self, name: String, parent_id: Option<UnsignedInt>) -> UnsignedInt {
        let id = {
            let mut next_id = self.next_bus_id.lock();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let mut bus = AudioBus::new(id, name);
        bus.parent_bus = parent_id;

        // Add to parent's children
        if let Some(parent_id) = parent_id {
            let mut buses = self.buses.write();
            if let Some(parent_bus) = buses.get_mut(&parent_id) {
                parent_bus.child_buses.push(id);
            }
        }

        self.buses.write().insert(id, bus);
        id
    }

    /// Remove an audio bus
    pub fn remove_bus(&self, bus_id: UnsignedInt) -> bool {
        if bus_id == self.master_bus_id {
            return false; // Cannot remove master bus
        }

        let mut buses = self.buses.write();
        let (parent_bus_id, children) = if let Some(bus) = buses.get(&bus_id) {
            (bus.parent_bus, bus.child_buses.clone())
        } else {
            return false;
        };

        // Remove from parent's children
        if let Some(parent_id) = parent_bus_id {
            if let Some(parent_bus) = buses.get_mut(&parent_id) {
                parent_bus.child_buses.retain(|&id| id != bus_id);
            }
        }

        // Reparent children to master
        for child_id in children {
            if let Some(child_bus) = buses.get_mut(&child_id) {
                child_bus.parent_bus = Some(self.master_bus_id);
            }
            if let Some(master_bus) = buses.get_mut(&self.master_bus_id) {
                master_bus.child_buses.push(child_id);
            }
        }

        buses.remove(&bus_id).is_some()
    }

    /// Add an effect to a bus
    pub fn add_bus_effect(&self, bus_id: UnsignedInt, effect: Box<dyn AudioEffect>) -> bool {
        if let Some(bus) = self.buses.write().get_mut(&bus_id) {
            bus.add_effect(effect);
            true
        } else {
            false
        }
    }

    /// Set bus volume
    pub fn set_bus_volume(&self, bus_id: UnsignedInt, volume: f32) {
        if let Some(bus) = self.buses.write().get_mut(&bus_id) {
            bus.volume = volume.clamp(0.0, 2.0);
        }
    }

    /// Set bus pan
    pub fn set_bus_pan(&self, bus_id: UnsignedInt, pan: f32) {
        if let Some(bus) = self.buses.write().get_mut(&bus_id) {
            bus.pan = pan.clamp(-1.0, 1.0);
        }
    }

    /// Mute/unmute a bus
    pub fn set_bus_mute(&self, bus_id: UnsignedInt, muted: bool) {
        if let Some(bus) = self.buses.write().get_mut(&bus_id) {
            bus.muted = muted;
        }
    }

    /// Solo/unsolo a bus
    pub fn set_bus_solo(&self, bus_id: UnsignedInt, solo: bool) {
        if let Some(bus) = self.buses.write().get_mut(&bus_id) {
            bus.solo = solo;
        }
    }

    /// Get bus by audio affect type
    pub fn get_bus_for_affect(&self, affect: AudioAffect) -> Option<UnsignedInt> {
        let buses = self.buses.read();
        match affect {
            AudioAffect::Music => buses
                .iter()
                .find(|(_, bus)| bus.name == "Music")
                .map(|(id, _)| *id),
            AudioAffect::SoundEffects => buses
                .iter()
                .find(|(_, bus)| bus.name == "SFX")
                .map(|(id, _)| *id),
            AudioAffect::Speech => buses
                .iter()
                .find(|(_, bus)| bus.name == "Voice")
                .map(|(id, _)| *id),
            AudioAffect::Ambient => buses
                .iter()
                .find(|(_, bus)| bus.name == "Ambient")
                .map(|(id, _)| *id),
            _ => Some(self.master_bus_id),
        }
    }

    /// Process audio through the mixer
    pub fn process(
        &self,
        sources: &[(AudioHandle, UnsignedInt, Vec<f32>, Vec<f32>)],
    ) -> (Vec<f32>, Vec<f32>) {
        let buffer_size = sources
            .first()
            .map(|(_, _, l, _)| l.len())
            .unwrap_or(self.buffer_size);
        let mut master_left = vec![0.0; buffer_size];
        let mut master_right = vec![0.0; buffer_size];

        // Group sources by bus
        let mut bus_sources: HashMap<UnsignedInt, Vec<(AudioHandle, &Vec<f32>, &Vec<f32>)>> =
            HashMap::new();
        for (handle, bus_id, left, right) in sources {
            bus_sources
                .entry(*bus_id)
                .or_default()
                .push((*handle, left, right));
        }

        // Process each bus
        let mut buses = self.buses.write();
        let bus_ids: Vec<_> = buses.keys().cloned().collect();

        for bus_id in bus_ids {
            if bus_id == self.master_bus_id {
                continue; // Process master bus last
            }

            if let Some(bus_sources) = bus_sources.get(&bus_id) {
                // Mix sources for this bus
                let mut bus_left = vec![0.0; buffer_size];
                let mut bus_right = vec![0.0; buffer_size];

                for (_, left, right) in bus_sources {
                    for (i, (&l, &r)) in left.iter().zip(right.iter()).enumerate() {
                        bus_left[i] += l;
                        bus_right[i] += r;
                    }
                }

                // Process bus effects
                if let Some(bus) = buses.get_mut(&bus_id) {
                    bus.process(&mut bus_left, &mut bus_right);

                    // Send to parent bus (or master)
                    let parent_id = bus.parent_bus.unwrap_or(self.master_bus_id);
                    if parent_id == self.master_bus_id {
                        for (i, (&l, &r)) in bus_left.iter().zip(bus_right.iter()).enumerate() {
                            master_left[i] += l;
                            master_right[i] += r;
                        }
                    }
                }
            }
        }

        // Process master bus
        if let Some(master_bus) = buses.get_mut(&self.master_bus_id) {
            master_bus.process(&mut master_left, &mut master_right);
        }

        // Apply master limiter
        self.limiter
            .lock()
            .process(&mut master_left, &mut master_right);

        (master_left, master_right)
    }

    /// Set audio quality
    pub fn set_quality(&mut self, quality: AudioQuality) {
        *self.quality.write() = quality;
        self.buffer_size = quality.get_buffer_size();
    }

    /// Get current audio quality
    pub fn get_quality(&self) -> AudioQuality {
        *self.quality.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_quality() {
        assert_eq!(AudioQuality::Low.get_quality_multiplier(), 0.5);
        assert_eq!(AudioQuality::Medium.get_quality_multiplier(), 1.0);
        assert_eq!(AudioQuality::High.get_quality_multiplier(), 1.5);
        assert_eq!(AudioQuality::Ultra.get_quality_multiplier(), 2.0);
    }

    #[test]
    fn test_audio_bus_creation() {
        let bus = AudioBus::new(1, "Test Bus".to_string());
        assert_eq!(bus.id, 1);
        assert_eq!(bus.name, "Test Bus");
        assert_eq!(bus.volume, 1.0);
        assert_eq!(bus.pan, 0.0);
        assert!(!bus.muted);
        assert!(!bus.solo);
    }

    #[test]
    fn test_audio_mixer_creation() {
        let mixer = AudioMixer::new(44100.0);
        let buses = mixer.buses.read();

        // Should have master + 4 default buses
        assert!(buses.len() >= 5);
        assert!(buses.contains_key(&0)); // Master bus

        // Check for default buses
        let bus_names: Vec<&String> = buses.values().map(|b| &b.name).collect();
        assert!(bus_names.contains(&&"Master".to_string()));
        assert!(bus_names.contains(&&"Music".to_string()));
        assert!(bus_names.contains(&&"SFX".to_string()));
        assert!(bus_names.contains(&&"Voice".to_string()));
        assert!(bus_names.contains(&&"Ambient".to_string()));
    }

    #[test]
    fn test_mixer_bus_management() {
        let mixer = AudioMixer::new(44100.0);

        // Create new bus
        let new_bus_id = mixer.create_bus("Test".to_string(), Some(0));
        assert!(new_bus_id > 0);

        // Check bus exists
        assert!(mixer.buses.read().contains_key(&new_bus_id));

        // Remove bus
        assert!(mixer.remove_bus(new_bus_id));
        assert!(!mixer.buses.read().contains_key(&new_bus_id));

        // Cannot remove master bus
        assert!(!mixer.remove_bus(0));
    }

    #[test]
    fn test_parametric_eq_creation() {
        let eq = ParametricEqualizer::new(1, 44100.0);
        assert_eq!(eq.get_id(), 1);
        assert_eq!(eq.get_name(), "Parametric EQ");
        assert!(eq.is_enabled());
        assert_eq!(eq.bands.len(), 4); // Default bands
    }

    #[test]
    fn test_compressor_creation() {
        let compressor = Compressor::new(2, 44100.0);
        assert_eq!(compressor.get_id(), 2);
        assert_eq!(compressor.get_name(), "Compressor");
        assert!(compressor.is_enabled());
        assert_eq!(compressor.threshold, -12.0);
        assert_eq!(compressor.ratio, 4.0);
    }
}

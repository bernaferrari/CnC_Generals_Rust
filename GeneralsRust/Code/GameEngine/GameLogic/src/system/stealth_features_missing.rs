//! Missing C++ Stealth & Detection System Features
//!
//! This module provides the missing components from the C++ implementation:
//! - Particle effect system hooks for IR detection visualization
//! - Animation framework for disguise transitions
//! - Reveal distance tracking system
//! - Configuration INI loading support
//! - FX (effects) system integration framework

use crate::common::{Coord3D, ObjectID, Real, UnsignedInt};
use log::{debug, trace, warn};
use std::collections::HashMap;

// ============================================================================
// PARTICLE EFFECT FRAMEWORK
// ============================================================================

/// Particle effect types for stealth system visualization
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticleEffect {
    /// IR Beacon effect - continuous detection beacon
    IRBeacon,
    /// IR Ping effect - single detection pulse
    IRPing,
    /// IR Grid effect - area detection grid visualization
    IRGrid,
    /// IR Bright effect - bright detection indicator
    IRBright,
    /// Disguise FX - effect when unit enters disguise
    DisguiseFX,
    /// Reveal FX - effect when unit is revealed
    RevealFX,
}

impl ParticleEffect {
    /// Get the string name for the effect type
    pub fn name(&self) -> &'static str {
        match self {
            ParticleEffect::IRBeacon => "IR_BEACON",
            ParticleEffect::IRPing => "IR_PING",
            ParticleEffect::IRGrid => "IR_GRID",
            ParticleEffect::IRBright => "IR_BRIGHT",
            ParticleEffect::DisguiseFX => "DISGUISE_FX",
            ParticleEffect::RevealFX => "REVEAL_FX",
        }
    }
}

/// Particle effect instance with position and parameters
#[derive(Debug, Clone)]
pub struct ParticleEffectFramework {
    /// Effect type
    pub effect_type: ParticleEffect,
    /// World position of effect
    pub position: Coord3D,
    /// Duration in frames (0 = infinite)
    pub duration_frames: UnsignedInt,
    /// Intensity/strength of effect (0.0-1.0)
    pub intensity: f32,
    /// Optional scale multiplier
    pub scale: f32,
}

impl ParticleEffectFramework {
    /// Create new particle effect
    pub fn new(
        effect_type: ParticleEffect,
        position: Coord3D,
        duration_frames: UnsignedInt,
    ) -> Self {
        Self {
            effect_type,
            position,
            duration_frames,
            intensity: 1.0,
            scale: 1.0,
        }
    }

    /// Set effect intensity
    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0).min(1.0);
        self
    }

    /// Set effect scale
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale.max(0.1).min(10.0);
        self
    }

    /// Format effect for particle system integration
    /// Returns formatted string ready for game engine particle system
    pub fn format_for_particle_system(&self) -> String {
        format!(
            "PARTICLE_EFFECT|{}|pos:({:.2},{:.2},{:.2})|duration:{}|intensity:{:.2}|scale:{:.2}",
            self.effect_type.name(),
            self.position.x,
            self.position.y,
            self.position.z,
            self.duration_frames,
            self.intensity,
            self.scale
        )
    }
}

// ============================================================================
// ANIMATION FRAMEWORK
// ============================================================================

/// Transition animation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// Animation not started
    Idle,
    /// Animation in progress
    Playing,
    /// Animation completed
    Completed,
}

/// Disguise transition animation
#[derive(Debug, Clone)]
pub struct TransitionAnimation {
    /// Current animation state
    pub state: AnimationState,
    /// Total frames in animation
    pub total_frames: UnsignedInt,
    /// Current frame number
    pub current_frame: UnsignedInt,
    /// Progress as percentage (0.0-1.0)
    pub progress: f32,
}

impl TransitionAnimation {
    /// Create new transition animation
    pub fn new(total_frames: UnsignedInt) -> Self {
        Self {
            state: AnimationState::Idle,
            total_frames: total_frames.max(1),
            current_frame: 0,
            progress: 0.0,
        }
    }

    /// Start animation
    pub fn start(&mut self) {
        self.state = AnimationState::Playing;
        self.current_frame = 0;
        self.progress = 0.0;
    }

    /// Update animation frame
    pub fn update(&mut self) {
        if self.state != AnimationState::Playing {
            return;
        }

        self.current_frame += 1;
        self.progress = (self.current_frame as f32) / (self.total_frames as f32);

        if self.current_frame >= self.total_frames {
            self.state = AnimationState::Completed;
            self.progress = 1.0;
        }
    }

    /// Check if animation is complete
    pub fn is_complete(&self) -> bool {
        self.state == AnimationState::Completed
    }

    /// Reset animation
    pub fn reset(&mut self) {
        self.state = AnimationState::Idle;
        self.current_frame = 0;
        self.progress = 0.0;
    }
}

/// Disguise animation with morph support
#[derive(Debug, Clone)]
pub struct DisguiseAnimation {
    /// Source template name
    pub source_template: String,
    /// Target template name (disguise as)
    pub target_template: String,
    /// Transition animation
    pub transition: TransitionAnimation,
    /// Reveal transition animation
    pub reveal_transition: TransitionAnimation,
    /// Frame to swap drawable (when to change visual)
    pub drawable_swap_frame: UnsignedInt,
    /// Whether morphing is enabled during transition
    pub morph_enabled: bool,
    /// Morph progress (0.0-1.0)
    pub morph_progress: f32,
}

impl DisguiseAnimation {
    /// Create new disguise animation
    pub fn new(
        source: String,
        target: String,
        transition_frames: UnsignedInt,
        drawable_swap_frame: UnsignedInt,
    ) -> Self {
        Self {
            source_template: source,
            target_template: target,
            transition: TransitionAnimation::new(transition_frames),
            reveal_transition: TransitionAnimation::new(transition_frames),
            drawable_swap_frame: drawable_swap_frame.min(transition_frames),
            morph_enabled: false,
            morph_progress: 0.0,
        }
    }

    /// Enable morphing during transition
    pub fn with_morph(mut self) -> Self {
        self.morph_enabled = true;
        self
    }

    /// Start disguise animation
    pub fn start_disguise(&mut self) {
        self.transition.start();
        self.reveal_transition.reset();
        if self.morph_enabled {
            self.morph_progress = 0.0;
        }
    }

    /// Start reveal animation
    pub fn start_reveal(&mut self) {
        self.reveal_transition.start();
        self.transition.reset();
    }

    /// Update animations
    pub fn update(&mut self) {
        self.transition.update();
        self.reveal_transition.update();

        // Update morph progress based on transition
        if self.morph_enabled && self.transition.state == AnimationState::Playing {
            self.morph_progress = self.transition.progress;
        }
    }

    /// Check if should swap drawable
    pub fn should_swap_drawable(&self) -> bool {
        self.transition.current_frame >= self.drawable_swap_frame
            && self.transition.current_frame < self.drawable_swap_frame + 1
    }

    /// Check if animation active
    pub fn is_active(&self) -> bool {
        self.transition.state == AnimationState::Playing
            || self.reveal_transition.state == AnimationState::Playing
    }
}

// ============================================================================
// REVEAL DISTANCE SYSTEM
// ============================================================================

/// Reveal distance thresholds for detection
#[derive(Debug, Clone, Copy)]
pub struct RevealDistanceThresholds {
    /// Outer threshold where stealth becomes weak (hints detection)
    pub break_distance: Real,
    /// Inner threshold where stealth actually breaks (revealed)
    pub hint_distance: Real,
    /// Minimum distance to maintain stealth (safety buffer)
    pub minimum_distance: Real,
}

impl RevealDistanceThresholds {
    /// Create with custom thresholds
    pub fn new(break_distance: Real, hint_distance: Real, minimum_distance: Real) -> Self {
        Self {
            break_distance: break_distance.max(1.0),
            hint_distance: hint_distance.max(0.1).min(break_distance),
            minimum_distance: minimum_distance.max(0.0).min(hint_distance),
        }
    }

    /// Standard thresholds for typical stealth units
    pub fn standard() -> Self {
        Self::new(200.0, 100.0, 0.0)
    }

    /// Close-range detection thresholds
    pub fn close_range() -> Self {
        Self::new(100.0, 50.0, 0.0)
    }

    /// Long-range detection thresholds
    pub fn long_range() -> Self {
        Self::new(500.0, 300.0, 100.0)
    }
}

/// Per-object reveal distance tracking
#[derive(Debug, Clone)]
pub struct RevealDistance {
    /// Object ID being tracked
    pub object_id: ObjectID,
    /// Distance thresholds
    pub thresholds: RevealDistanceThresholds,
    /// Current distance to nearest observer (cache)
    pub current_distance: Real,
    /// Is stealth broken at this distance
    pub is_broken: bool,
    /// Is stealth hinted (weak) at this distance
    pub is_hinted: bool,
}

impl RevealDistance {
    /// Create new reveal distance tracker
    pub fn new(object_id: ObjectID, thresholds: RevealDistanceThresholds) -> Self {
        Self {
            object_id,
            thresholds,
            current_distance: f32::MAX,
            is_broken: false,
            is_hinted: false,
        }
    }

    /// Update distance and check reveal status
    pub fn update_distance(&mut self, distance: Real) {
        self.current_distance = distance;
        self.is_broken = distance < self.thresholds.hint_distance;
        self.is_hinted = !self.is_broken && distance < self.thresholds.break_distance;
    }

    /// Check if stealth is broken at current distance
    pub fn check_break(&self) -> bool {
        self.is_broken
    }

    /// Check if stealth is hinted at current distance
    pub fn check_hint(&self) -> bool {
        self.is_hinted
    }

    /// Get distance ratio (0.0 at break point, 1.0 at minimum distance)
    pub fn distance_ratio(&self) -> f32 {
        if self.current_distance >= self.thresholds.break_distance {
            0.0
        } else if self.current_distance <= self.thresholds.minimum_distance {
            1.0
        } else {
            let range = self.thresholds.break_distance - self.thresholds.minimum_distance;
            (self.thresholds.break_distance - self.current_distance) / range
        }
    }

    /// Get threat level (0.0-1.0)
    pub fn threat_level(&self) -> f32 {
        if self.is_broken {
            1.0
        } else if self.is_hinted {
            0.5
        } else {
            0.0
        }
    }
}

// ============================================================================
// CONFIGURATION FRAMEWORK
// ============================================================================

/// Stealth behavior configuration
#[derive(Debug, Clone)]
pub struct StealthConfig {
    /// Base stealth delay in frames before stealth activates
    pub stealth_delay_frames: UnsignedInt,
    /// Speed threshold - moving faster than this breaks stealth
    pub move_threshold_speed: Real,
    /// Stealth level mask for condition checks
    pub stealth_level_mask: u32,
    /// Whether this unit has innate stealth
    pub innate_stealth: bool,
    /// Whether stealth is granted by special power
    pub granted_by_special_power: bool,
    /// FX effect name for disguise entry
    pub disguise_fx_name: String,
    /// FX effect name for disguise reveal
    pub disguise_reveal_fx_name: String,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            stealth_delay_frames: 30,
            move_threshold_speed: 0.0,
            stealth_level_mask: 0,
            innate_stealth: false,
            granted_by_special_power: false,
            disguise_fx_name: String::new(),
            disguise_reveal_fx_name: String::new(),
        }
    }
}

impl StealthConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.move_threshold_speed < 0.0 {
            return Err("move_threshold_speed cannot be negative".to_string());
        }
        Ok(())
    }

    /// Create configuration for standard GLA stealth
    pub fn gla_stealth() -> Self {
        Self {
            stealth_delay_frames: 20,
            move_threshold_speed: 0.0,
            stealth_level_mask: 0x1,
            innate_stealth: false,
            granted_by_special_power: true,
            disguise_fx_name: "GLA_DISGUISE".to_string(),
            disguise_reveal_fx_name: "GLA_REVEAL".to_string(),
        }
    }

    /// Create configuration for innate stealth
    pub fn innate_stealth() -> Self {
        Self {
            stealth_delay_frames: 0,
            move_threshold_speed: 1.0,
            stealth_level_mask: 0,
            innate_stealth: true,
            granted_by_special_power: false,
            disguise_fx_name: String::new(),
            disguise_reveal_fx_name: String::new(),
        }
    }
}

/// Detection range configuration
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Base detection range in game units
    pub base_range: Real,
    /// Detection range multiplier for units with radar
    pub radar_multiplier: f32,
    /// Detection range multiplier for infantry
    pub infantry_multiplier: f32,
    /// Detection range multiplier for vehicles
    pub vehicle_multiplier: f32,
    /// Minimum detection range (can't be lower)
    pub minimum_range: Real,
    /// Maximum detection range (can't be higher)
    pub maximum_range: Real,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            base_range: 200.0,
            radar_multiplier: 1.5,
            infantry_multiplier: 0.8,
            vehicle_multiplier: 1.2,
            minimum_range: 50.0,
            maximum_range: 500.0,
        }
    }
}

impl DetectionConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.base_range < self.minimum_range {
            return Err(format!(
                "base_range ({}) must be >= minimum_range ({})",
                self.base_range, self.minimum_range
            ));
        }
        if self.base_range > self.maximum_range {
            return Err(format!(
                "base_range ({}) must be <= maximum_range ({})",
                self.base_range, self.maximum_range
            ));
        }
        if self.minimum_range > self.maximum_range {
            return Err(format!(
                "minimum_range ({}) must be <= maximum_range ({})",
                self.minimum_range, self.maximum_range
            ));
        }
        if self.radar_multiplier <= 0.0
            || self.infantry_multiplier <= 0.0
            || self.vehicle_multiplier <= 0.0
        {
            return Err("all multipliers must be positive".to_string());
        }
        Ok(())
    }

    /// Get effective detection range with multiplier
    pub fn effective_range(&self, multiplier: f32) -> Real {
        let range = self.base_range * (multiplier as f32);
        range.max(self.minimum_range).min(self.maximum_range)
    }
}

/// Disguise system configuration
#[derive(Debug, Clone)]
pub struct DisguiseConfig {
    /// Disguise transition duration in frames
    pub transition_frames: UnsignedInt,
    /// Reveal transition duration in frames
    pub reveal_transition_frames: UnsignedInt,
    /// Whether to use morph animation during transition
    pub use_morph_animation: bool,
    /// Friendly unit opacity minimum (0.0-1.0)
    pub friendly_opacity_min: Real,
    /// Friendly unit opacity maximum (0.0-1.0)
    pub friendly_opacity_max: Real,
    /// Whether team-wide disguise is enabled
    pub team_wide_disguise_enabled: bool,
    /// Pulse frequency for effect updates (frames)
    pub pulse_frequency_frames: UnsignedInt,
}

impl Default for DisguiseConfig {
    fn default() -> Self {
        Self {
            transition_frames: 30,
            reveal_transition_frames: 20,
            use_morph_animation: true,
            friendly_opacity_min: 0.0,
            friendly_opacity_max: 1.0,
            team_wide_disguise_enabled: true,
            pulse_frequency_frames: 10,
        }
    }
}

impl DisguiseConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.friendly_opacity_min < 0.0 || self.friendly_opacity_min > 1.0 {
            return Err("friendly_opacity_min must be between 0.0 and 1.0".to_string());
        }
        if self.friendly_opacity_max < 0.0 || self.friendly_opacity_max > 1.0 {
            return Err("friendly_opacity_max must be between 0.0 and 1.0".to_string());
        }
        if self.friendly_opacity_min > self.friendly_opacity_max {
            return Err("friendly_opacity_min must be <= friendly_opacity_max".to_string());
        }
        if self.transition_frames == 0 || self.reveal_transition_frames == 0 {
            return Err("transition frame counts cannot be zero".to_string());
        }
        Ok(())
    }
}

/// Complete stealth configuration loaded from INI
#[derive(Debug, Clone)]
pub struct ConfigurationFramework {
    /// Stealth behavior configuration
    pub stealth: StealthConfig,
    /// Detection range configuration
    pub detection: DetectionConfig,
    /// Disguise animation configuration
    pub disguise: DisguiseConfig,
    /// Is configuration loaded and valid
    pub is_loaded: bool,
}

impl Default for ConfigurationFramework {
    fn default() -> Self {
        Self {
            stealth: StealthConfig::default(),
            detection: DetectionConfig::default(),
            disguise: DisguiseConfig::default(),
            is_loaded: false,
        }
    }
}

impl ConfigurationFramework {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from INI (lightweight parser for stealth settings)
    pub fn load_from_ini(&mut self, _ini_path: &str) -> Result<(), String> {
        let contents = std::fs::read_to_string(_ini_path)
            .map_err(|e| format!("Failed to read INI {}: {}", _ini_path, e))?;

        fn parse_bool_value(value: &str) -> bool {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "true" | "yes" | "1" | "on"
            )
        }

        let mut section = String::new();
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();

            match section.as_str() {
                "stealth" => match key.as_str() {
                    "stealth_delay_frames" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.stealth.stealth_delay_frames = v;
                        }
                    }
                    "move_threshold_speed" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.stealth.move_threshold_speed = v;
                        }
                    }
                    "stealth_level_mask" => {
                        if let Ok(v) = u32::from_str_radix(value.trim_start_matches("0x"), 16) {
                            self.stealth.stealth_level_mask = v;
                        } else if let Ok(v) = value.parse::<u32>() {
                            self.stealth.stealth_level_mask = v;
                        }
                    }
                    "innate_stealth" => {
                        self.stealth.innate_stealth = parse_bool_value(value);
                    }
                    "granted_by_special_power" => {
                        self.stealth.granted_by_special_power = parse_bool_value(value);
                    }
                    "disguise_fx_name" => {
                        self.stealth.disguise_fx_name = value.to_string();
                    }
                    "disguise_reveal_fx_name" => {
                        self.stealth.disguise_reveal_fx_name = value.to_string();
                    }
                    _ => {}
                },
                "detection" => match key.as_str() {
                    "base_range" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.detection.base_range = v;
                        }
                    }
                    "radar_multiplier" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.detection.radar_multiplier = v;
                        }
                    }
                    "infantry_multiplier" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.detection.infantry_multiplier = v;
                        }
                    }
                    "vehicle_multiplier" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.detection.vehicle_multiplier = v;
                        }
                    }
                    "minimum_range" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.detection.minimum_range = v;
                        }
                    }
                    "maximum_range" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.detection.maximum_range = v;
                        }
                    }
                    _ => {}
                },
                "disguise" => match key.as_str() {
                    "transition_frames" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.disguise.transition_frames = v;
                        }
                    }
                    "reveal_transition_frames" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.disguise.reveal_transition_frames = v;
                        }
                    }
                    "use_morph_animation" => {
                        self.disguise.use_morph_animation = parse_bool_value(value);
                    }
                    "friendly_opacity_min" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.disguise.friendly_opacity_min = v;
                        }
                    }
                    "friendly_opacity_max" => {
                        if let Ok(v) = value.parse::<f32>() {
                            self.disguise.friendly_opacity_max = v;
                        }
                    }
                    "team_wide_disguise_enabled" => {
                        self.disguise.team_wide_disguise_enabled = parse_bool_value(value);
                    }
                    "pulse_frequency_frames" => {
                        if let Ok(v) = value.parse::<u32>() {
                            self.disguise.pulse_frequency_frames = v;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        self.stealth.validate()?;
        self.detection.validate()?;
        self.disguise.validate()?;
        self.is_loaded = true;
        debug!("Configuration loaded from {}", _ini_path);
        Ok(())
    }

    /// Load with custom stealth config
    pub fn with_stealth_config(mut self, config: StealthConfig) -> Self {
        self.stealth = config;
        self
    }

    /// Load with custom detection config
    pub fn with_detection_config(mut self, config: DetectionConfig) -> Self {
        self.detection = config;
        self
    }

    /// Load with custom disguise config
    pub fn with_disguise_config(mut self, config: DisguiseConfig) -> Self {
        self.disguise = config;
        self
    }

    /// Validate entire configuration
    pub fn validate(&self) -> Result<(), String> {
        self.stealth.validate()?;
        self.detection.validate()?;
        self.disguise.validate()?;
        Ok(())
    }

    /// Mark as loaded (for testing)
    pub fn mark_loaded(&mut self) {
        self.is_loaded = true;
    }
}

// ============================================================================
// FX SYSTEM INTEGRATION FRAMEWORK
// ============================================================================

/// FX (Effects) system integration framework
#[derive(Debug, Clone)]
pub struct FXSystemFramework {
    /// Registered FX effect names
    pub registered_effects: HashMap<String, String>,
    /// Is FX system ready for integration
    pub is_ready: bool,
}

impl Default for FXSystemFramework {
    fn default() -> Self {
        Self::new()
    }
}

impl FXSystemFramework {
    /// Create new FX system framework
    pub fn new() -> Self {
        let mut framework = Self {
            registered_effects: HashMap::new(),
            is_ready: false,
        };
        framework.register_default_effects();
        framework
    }

    /// Register default stealth FX effects
    fn register_default_effects(&mut self) {
        self.registered_effects.insert(
            "IR_BEACON".to_string(),
            "ParticleSystem_IRBeacon".to_string(),
        );
        self.registered_effects
            .insert("IR_PING".to_string(), "ParticleSystem_IRPing".to_string());
        self.registered_effects
            .insert("IR_GRID".to_string(), "ParticleSystem_IRGrid".to_string());
        self.registered_effects.insert(
            "IR_BRIGHT".to_string(),
            "ParticleSystem_IRBright".to_string(),
        );
        self.registered_effects.insert(
            "DISGUISE_FX".to_string(),
            "ParticleSystem_DisguiseFX".to_string(),
        );
        self.registered_effects.insert(
            "REVEAL_FX".to_string(),
            "ParticleSystem_RevealFX".to_string(),
        );
    }

    /// Register custom FX effect
    pub fn register_effect(&mut self, name: String, system_name: String) {
        self.registered_effects.insert(name, system_name);
    }

    /// Get FX system name for effect
    pub fn get_effect_system(&self, effect_name: &str) -> Option<String> {
        self.registered_effects.get(effect_name).cloned()
    }

    /// Mark FX system as ready for integration
    pub fn set_ready(&mut self, ready: bool) {
        self.is_ready = ready;
        if ready {
            debug!("FX system framework ready for integration");
        }
    }

    /// Format FX effect for game engine
    pub fn format_for_engine(
        &self,
        effect_name: &str,
        object_id: ObjectID,
        position: &Coord3D,
    ) -> Option<String> {
        self.get_effect_system(effect_name).map(|system_name| {
            format!(
                "FX_REQUEST|system:{}|object_id:{}|position:({:.2},{:.2},{:.2})",
                system_name, object_id, position.x, position.y, position.z
            )
        })
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ParticleEffectFramework tests
    #[test]
    fn particle_effect_framework_creates_beacon() {
        let pos = Coord3D::new(100.0, 200.0, 0.0);
        let effect = ParticleEffectFramework::new(ParticleEffect::IRBeacon, pos, 60);

        assert_eq!(effect.effect_type, ParticleEffect::IRBeacon);
        assert_eq!(effect.duration_frames, 60);
        assert_eq!(effect.intensity, 1.0);
        assert_eq!(effect.scale, 1.0);
    }

    #[test]
    fn particle_effect_framework_with_intensity() {
        let pos = Coord3D::new(0.0, 0.0, 0.0);
        let effect =
            ParticleEffectFramework::new(ParticleEffect::IRPing, pos, 30).with_intensity(0.5);

        assert_eq!(effect.intensity, 0.5);
    }

    #[test]
    fn particle_effect_framework_formats_correctly() {
        let pos = Coord3D::new(100.0, 200.0, 50.0);
        let effect = ParticleEffectFramework::new(ParticleEffect::DisguiseFX, pos, 100)
            .with_intensity(0.8)
            .with_scale(1.5);

        let formatted = effect.format_for_particle_system();
        assert!(formatted.contains("DISGUISE_FX"));
        assert!(formatted.contains("100.00"));
        assert!(formatted.contains("200.00"));
        assert!(formatted.contains("50.00"));
        assert!(formatted.contains("intensity:0.80"));
        assert!(formatted.contains("scale:1.50"));
    }

    #[test]
    fn particle_effect_names_are_unique() {
        let effects = vec![
            ParticleEffect::IRBeacon,
            ParticleEffect::IRPing,
            ParticleEffect::IRGrid,
            ParticleEffect::IRBright,
            ParticleEffect::DisguiseFX,
            ParticleEffect::RevealFX,
        ];

        let mut names = std::collections::HashSet::new();
        for effect in effects {
            assert!(names.insert(effect.name()));
        }
    }

    // TransitionAnimation tests
    #[test]
    fn transition_animation_creates_with_idle_state() {
        let anim = TransitionAnimation::new(100);
        assert_eq!(anim.state, AnimationState::Idle);
        assert_eq!(anim.current_frame, 0);
        assert_eq!(anim.progress, 0.0);
        assert_eq!(anim.total_frames, 100);
    }

    #[test]
    fn transition_animation_starts_playing() {
        let mut anim = TransitionAnimation::new(100);
        anim.start();
        assert_eq!(anim.state, AnimationState::Playing);
    }

    #[test]
    fn transition_animation_updates_progress() {
        let mut anim = TransitionAnimation::new(100);
        anim.start();

        for i in 1..=100 {
            anim.update();
            assert!(anim.progress > 0.0);
            assert!(anim.progress <= 1.0);
        }

        assert_eq!(anim.state, AnimationState::Completed);
        assert_eq!(anim.progress, 1.0);
    }

    #[test]
    fn transition_animation_completes() {
        let mut anim = TransitionAnimation::new(10);
        anim.start();

        for _ in 0..10 {
            anim.update();
        }

        assert!(anim.is_complete());
    }

    #[test]
    fn transition_animation_resets() {
        let mut anim = TransitionAnimation::new(50);
        anim.start();

        for _ in 0..25 {
            anim.update();
        }

        anim.reset();
        assert_eq!(anim.state, AnimationState::Idle);
        assert_eq!(anim.current_frame, 0);
        assert_eq!(anim.progress, 0.0);
    }

    // DisguiseAnimation tests
    #[test]
    fn disguise_animation_creates_with_templates() {
        let anim = DisguiseAnimation::new("Ranger".to_string(), "GLA_Infantry".to_string(), 60, 30);

        assert_eq!(anim.source_template, "Ranger");
        assert_eq!(anim.target_template, "GLA_Infantry");
        assert_eq!(anim.transition.total_frames, 60);
        assert_eq!(anim.drawable_swap_frame, 30);
    }

    #[test]
    fn disguise_animation_starts_disguise() {
        let mut anim = DisguiseAnimation::new("A".to_string(), "B".to_string(), 60, 30);

        anim.start_disguise();
        assert_eq!(anim.transition.state, AnimationState::Playing);
    }

    #[test]
    fn disguise_animation_detects_drawable_swap() {
        let mut anim = DisguiseAnimation::new("A".to_string(), "B".to_string(), 60, 30);

        anim.start_disguise();
        for _ in 0..29 {
            anim.update();
            assert!(!anim.should_swap_drawable());
        }

        anim.update();
        assert!(anim.should_swap_drawable());
    }

    #[test]
    fn disguise_animation_morph_progress() {
        let mut anim =
            DisguiseAnimation::new("A".to_string(), "B".to_string(), 100, 50).with_morph();

        anim.start_disguise();
        for _ in 0..50 {
            anim.update();
        }

        assert!(anim.morph_progress > 0.0);
        assert!(anim.morph_progress <= 1.0);
    }

    // RevealDistance tests
    #[test]
    fn reveal_distance_creates_with_standard_thresholds() {
        let reveal = RevealDistance::new(1, RevealDistanceThresholds::standard());
        assert_eq!(reveal.object_id, 1);
        assert_eq!(reveal.thresholds.break_distance, 200.0);
    }

    #[test]
    fn reveal_distance_updates_and_checks_break() {
        let mut reveal = RevealDistance::new(1, RevealDistanceThresholds::standard());

        reveal.update_distance(50.0);
        assert!(reveal.check_break());

        reveal.update_distance(300.0);
        assert!(!reveal.check_break());
    }

    #[test]
    fn reveal_distance_checks_hint() {
        let mut reveal = RevealDistance::new(1, RevealDistanceThresholds::standard());

        reveal.update_distance(150.0);
        assert!(reveal.check_hint());
        assert!(!reveal.check_break());
    }

    #[test]
    fn reveal_distance_calculates_ratio() {
        let mut reveal = RevealDistance::new(1, RevealDistanceThresholds::new(200.0, 100.0, 0.0));

        reveal.update_distance(0.0);
        assert_eq!(reveal.distance_ratio(), 1.0);

        reveal.update_distance(200.0);
        assert_eq!(reveal.distance_ratio(), 0.0);

        reveal.update_distance(100.0);
        assert!(reveal.distance_ratio() > 0.0 && reveal.distance_ratio() < 1.0);
    }

    #[test]
    fn reveal_distance_threat_levels() {
        let mut reveal = RevealDistance::new(1, RevealDistanceThresholds::standard());

        reveal.update_distance(50.0);
        assert_eq!(reveal.threat_level(), 1.0);

        reveal.update_distance(150.0);
        assert_eq!(reveal.threat_level(), 0.5);

        reveal.update_distance(300.0);
        assert_eq!(reveal.threat_level(), 0.0);
    }

    // StealthConfig tests
    #[test]
    fn stealth_config_default_creates_valid() {
        let config = StealthConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn stealth_config_gla_stealth_valid() {
        let config = StealthConfig::gla_stealth();
        assert_eq!(config.stealth_delay_frames, 20);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn stealth_config_innate_stealth_valid() {
        let config = StealthConfig::innate_stealth();
        assert!(config.innate_stealth);
        assert!(config.validate().is_ok());
    }

    // DetectionConfig tests
    #[test]
    fn detection_config_default_valid() {
        let config = DetectionConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn detection_config_validates_ranges() {
        let config = DetectionConfig {
            base_range: 600.0,
            radar_multiplier: 1.5,
            infantry_multiplier: 0.8,
            vehicle_multiplier: 1.2,
            minimum_range: 50.0,
            maximum_range: 500.0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn detection_config_effective_range() {
        let config = DetectionConfig::default();
        let range = config.effective_range(2.0);
        assert!(range >= config.minimum_range);
        assert!(range <= config.maximum_range);
    }

    // DisguiseConfig tests
    #[test]
    fn disguise_config_default_valid() {
        let config = DisguiseConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn disguise_config_validates_opacity() {
        let config = DisguiseConfig {
            friendly_opacity_min: 1.5,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn disguise_config_validates_frame_counts() {
        let config = DisguiseConfig {
            transition_frames: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    // ConfigurationFramework tests
    #[test]
    fn configuration_framework_creates_default() {
        let config = ConfigurationFramework::default();
        assert!(!config.is_loaded);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn configuration_framework_loads_from_ini() {
        let ini_contents = r#"
[Stealth]
stealth_delay_frames = 45
move_threshold_speed = 1.5
stealth_level_mask = 0x2
innate_stealth = true

[Detection]
base_range = 250
minimum_range = 50
maximum_range = 500

[Disguise]
transition_frames = 40
reveal_transition_frames = 25
use_morph_animation = false
"#;
        let path = std::env::temp_dir().join(format!(
            "stealth_features_missing_{}_{}.ini",
            std::process::id(),
            crate::helpers::TheGameLogic::get_frame()
        ));
        std::fs::write(&path, ini_contents).expect("failed to create temp ini");

        let mut config = ConfigurationFramework::default();
        let load_result = config.load_from_ini(path.to_string_lossy().as_ref());
        let _ = std::fs::remove_file(&path);

        assert!(load_result.is_ok());
        assert!(config.is_loaded);
        assert_eq!(config.stealth.stealth_delay_frames, 45);
        assert!((config.stealth.move_threshold_speed - 1.5).abs() < 0.001);
        assert_eq!(config.stealth.stealth_level_mask, 0x2);
        assert!(config.stealth.innate_stealth);
        assert_eq!(config.detection.base_range, 250.0);
        assert_eq!(config.disguise.transition_frames, 40);
        assert_eq!(config.disguise.reveal_transition_frames, 25);
        assert!(!config.disguise.use_morph_animation);
    }

    #[test]
    fn configuration_framework_with_stealth_config() {
        let stealth = StealthConfig::gla_stealth();
        let config = ConfigurationFramework::default().with_stealth_config(stealth);

        assert_eq!(config.stealth.stealth_delay_frames, 20);
    }

    #[test]
    fn configuration_framework_validates_all_components() {
        let config = ConfigurationFramework::default();
        assert!(config.validate().is_ok());
    }

    // FXSystemFramework tests
    #[test]
    fn fx_system_framework_registers_defaults() {
        let framework = FXSystemFramework::new();
        assert!(framework.get_effect_system("IR_BEACON").is_some());
        assert!(framework.get_effect_system("DISGUISE_FX").is_some());
    }

    #[test]
    fn fx_system_framework_registers_custom_effect() {
        let mut framework = FXSystemFramework::new();
        framework.register_effect("CUSTOM_FX".to_string(), "CustomSystem".to_string());
        assert_eq!(
            framework.get_effect_system("CUSTOM_FX"),
            Some("CustomSystem".to_string())
        );
    }

    #[test]
    fn fx_system_framework_ready_state() {
        let mut framework = FXSystemFramework::new();
        assert!(!framework.is_ready);
        framework.set_ready(true);
        assert!(framework.is_ready);
    }

    #[test]
    fn fx_system_framework_formats_for_engine() {
        let framework = FXSystemFramework::new();
        let pos = Coord3D::new(100.0, 200.0, 0.0);
        let formatted = framework.format_for_engine("IR_BEACON", 42, &pos);

        assert!(formatted.is_some());
        let formatted = formatted.unwrap();
        assert!(formatted.contains("object_id:42"));
        assert!(formatted.contains("100.00"));
        assert!(formatted.contains("200.00"));
    }

    #[test]
    fn configuration_framework_integration() {
        let mut config = ConfigurationFramework::default()
            .with_stealth_config(StealthConfig::gla_stealth())
            .with_detection_config(DetectionConfig::default())
            .with_disguise_config(DisguiseConfig::default());

        config.mark_loaded();
        assert!(config.is_loaded);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn all_frameworks_work_together() {
        let mut config = ConfigurationFramework::new();
        let mut reveal = RevealDistance::new(1, RevealDistanceThresholds::standard());
        let mut disguise = DisguiseAnimation::new(
            "Source".to_string(),
            "Target".to_string(),
            config.disguise.transition_frames,
            30,
        );
        let fx = FXSystemFramework::new();

        config.mark_loaded();
        reveal.update_distance(150.0);
        disguise.start_disguise();

        assert!(config.is_loaded);
        assert!(reveal.check_hint());
        assert!(disguise.is_active());
        assert!(fx.get_effect_system("DISGUISE_FX").is_some());
    }
}

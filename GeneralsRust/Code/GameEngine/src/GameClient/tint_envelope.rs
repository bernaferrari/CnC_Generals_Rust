// FILE: tint_envelope.rs
// TintEnvelope handles the fading of tint color (ADSR envelope)
// Ported from C++ Drawable.h TintEnvelope class
// Author: Original C++ implementation in Drawable.h

use crate::Common::game_type::{Real, Bool, UnsignedInt};
use crate::GameClient::draw_module::RGBColor;
use crate::WWMath::vector3::Vector3;

/// Default tint color fade rate
pub const DEFAULT_TINT_COLOR_FADE_RATE: Real = 0.6;

/// Default attack frames
pub const DEF_ATTACK_FRAMES: UnsignedInt = 1;

/// Default sustain frames
pub const DEF_SUSTAIN_FRAMES: UnsignedInt = 1;

/// Default decay frames
pub const DEF_DECAY_FRAMES: UnsignedInt = 4;

/// Sustain indefinitely constant
pub const SUSTAIN_INDEFINITELY: UnsignedInt = 0xfffffffe;

/// Fade rate epsilon for determining when we're close enough
const FADE_RATE_EPSILON: Real = 0.001;

/// ADSR envelope states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopeState {
    Rest,
    Attack,
    Decay,
    Sustain,
}

/// TintEnvelope handles the fading of the tint color up, down, stable etc.
/// Assumes that (0,0,0) is the color for the AT REST state, used as decay target.
/// Works like an ADSR envelope, except that SUSTAIN and RELEASE are randomly
/// (or never) triggered externally.
#[derive(Debug, Clone)]
pub struct TintEnvelope {
    /// Step amount to make tint turn on slow or fast
    attack_rate: Vector3,

    /// Step amount to make tint turn off slow or fast
    decay_rate: Vector3,

    /// The peak color, what color we are headed toward during attack
    peak_color: Vector3,

    /// The current color, how we are colored now
    current_color: Vector3,

    /// Sustain counter
    sustain_counter: UnsignedInt,

    /// Current envelope state
    env_state: EnvelopeState,

    /// Set TRUE if this has any effect (has a non 0,0,0 color)
    affect: Bool,
}

impl TintEnvelope {
    /// Create a new TintEnvelope at rest
    pub fn new() -> Self {
        Self {
            attack_rate: Vector3::new(0.0, 0.0, 0.0),
            decay_rate: Vector3::new(0.0, 0.0, 0.0),
            peak_color: Vector3::new(0.0, 0.0, 0.0),
            current_color: Vector3::new(0.0, 0.0, 0.0),
            sustain_counter: 0,
            env_state: EnvelopeState::Rest,
            affect: false,
        }
    }

    /// Play the tint envelope with specified parameters
    pub fn play(
        &mut self,
        peak: &RGBColor,
        attack_frames: UnsignedInt,
        decay_frames: UnsignedInt,
        sustain_at_peak: UnsignedInt,
    ) {
        self.set_peak_color(peak);
        self.set_attack_frames(attack_frames);
        self.set_decay_frames(decay_frames);

        self.env_state = EnvelopeState::Attack;
        self.sustain_counter = sustain_at_peak;
        self.affect = true;

        // Check if we're already at the peak color
        let delta = self.current_color - self.peak_color;
        if delta.length() <= FADE_RATE_EPSILON {
            self.env_state = EnvelopeState::Sustain;
        }
    }

    /// Update the envelope (called each frame)
    pub fn update(&mut self) {
        match self.env_state {
            EnvelopeState::Rest => {
                // Most likely case
                self.current_color = Vector3::new(0.0, 0.0, 0.0);
                self.affect = false;
            }

            EnvelopeState::Decay => {
                // Much more likely than attack
                if self.decay_rate.length() > self.current_color.length()
                    || self.current_color.length() <= FADE_RATE_EPSILON
                {
                    // We are at rest
                    self.env_state = EnvelopeState::Rest;
                    self.affect = false;
                } else {
                    // Add the decay rate to the current color
                    self.current_color = self.current_color + self.decay_rate;
                    self.affect = true;
                }
            }

            EnvelopeState::Attack => {
                let delta = self.current_color - self.peak_color;

                if self.attack_rate.length() > delta.length()
                    || delta.length() <= FADE_RATE_EPSILON
                {
                    // We are at the peak
                    if self.sustain_counter > 0 {
                        self.env_state = EnvelopeState::Sustain;
                    } else {
                        self.env_state = EnvelopeState::Decay;
                    }
                } else {
                    // Add the attack rate to the current color
                    self.current_color = self.current_color + self.attack_rate;
                    self.affect = true;
                }
            }

            EnvelopeState::Sustain => {
                if self.sustain_counter > 0 && self.sustain_counter != SUSTAIN_INDEFINITELY {
                    self.sustain_counter -= 1;
                    if self.sustain_counter == 0 {
                        self.release();
                    }
                }
                // Otherwise sustain until externally triggered to release
            }
        }
    }

    /// Manually sustain (switch to sustain state)
    pub fn sustain(&mut self) {
        self.env_state = EnvelopeState::Sustain;
    }

    /// Release (switch to decay state)
    pub fn release(&mut self) {
        self.env_state = EnvelopeState::Decay;
    }

    /// Go to rest state immediately
    pub fn rest(&mut self) {
        self.env_state = EnvelopeState::Rest;
    }

    /// Check if this envelope is effective (has a non-zero color)
    pub fn is_effective(&self) -> bool {
        self.affect
    }

    /// Get the current color
    pub fn get_color(&self) -> &Vector3 {
        &self.current_color
    }

    /// Set attack frames
    fn set_attack_frames(&mut self, frames: UnsignedInt) {
        let frames = frames.max(1);
        let recip_frames = 1.0 / frames as Real;

        // Calculate attack rate from current color to peak color
        self.attack_rate = self.peak_color - self.current_color;
        self.attack_rate = self.attack_rate.scale(recip_frames);
    }

    /// Set decay frames
    fn set_decay_frames(&mut self, frames: UnsignedInt) {
        let frames = frames.max(1);
        let recip_frames = -1.0 / frames as Real;

        // Calculate decay rate from peak color back to zero
        self.decay_rate = self.peak_color.scale(recip_frames);
    }

    /// Set peak color from RGBColor
    fn set_peak_color(&mut self, peak: &RGBColor) {
        self.peak_color = Vector3::new(peak.red, peak.green, peak.blue);
    }
}

impl Default for TintEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tint_envelope_creation() {
        let envelope = TintEnvelope::new();
        assert!(!envelope.is_effective());
        assert_eq!(envelope.env_state, EnvelopeState::Rest);
    }

    #[test]
    fn test_tint_envelope_play() {
        let mut envelope = TintEnvelope::new();
        let white = RGBColor::new(1.0, 1.0, 1.0);

        envelope.play(&white, 10, 10, 5);
        assert!(envelope.is_effective());
        assert_eq!(envelope.env_state, EnvelopeState::Attack);
    }

    #[test]
    fn test_tint_envelope_update_attack() {
        let mut envelope = TintEnvelope::new();
        let white = RGBColor::new(1.0, 1.0, 1.0);

        envelope.play(&white, 10, 10, 0);

        // Simulate several update frames during attack
        for _ in 0..5 {
            envelope.update();
            assert!(envelope.is_effective());
        }
    }

    #[test]
    fn test_tint_envelope_sustain_release() {
        let mut envelope = TintEnvelope::new();
        let white = RGBColor::new(1.0, 1.0, 1.0);

        envelope.play(&white, 1, 10, 5);
        envelope.update(); // Attack
        envelope.update(); // Should be in sustain now

        envelope.release();
        assert_eq!(envelope.env_state, EnvelopeState::Decay);
    }

    #[test]
    fn test_tint_envelope_rest() {
        let mut envelope = TintEnvelope::new();
        let white = RGBColor::new(1.0, 1.0, 1.0);

        envelope.play(&white, 1, 1, 0);
        envelope.rest();

        assert_eq!(envelope.env_state, EnvelopeState::Rest);
        assert!(!envelope.is_effective());
    }
}

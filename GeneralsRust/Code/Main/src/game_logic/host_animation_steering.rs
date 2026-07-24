//! Host AnimationSteeringUpdate residual (turn anim conditions from physics).
//!
//! C++: `AnimationSteeringUpdate::update` maps `PhysicsBehavior::getTurning()` to
//! model conditions:
//! - straight → CENTER_TO_RIGHT / CENTER_TO_LEFT on turn start
//! - turning → RIGHT_TO_CENTER / LEFT_TO_CENTER when turn ends
//! - recenter → clear when TURN_NONE
//!
//! Retail peel (`GLAVehicleBattleBus` and variants):
//! - `MinTransitionTime = 300` ms → **9** frames
//!
//! Fail-closed: not full Drawable clearAndSet multi-flag mask scrub /
//! random physics spike / client-only anim blend fidelity.

use crate::game_logic::object::PhysicsTurningType;
use serde::{Deserialize, Serialize};

pub const ANIM_STEER_LOGIC_FPS: f32 = 30.0;
/// Retail MinTransitionTime 300ms.
pub const BATTLE_BUS_MIN_TRANSITION_MS: u32 = 300;
pub const BATTLE_BUS_MIN_TRANSITION_FRAMES: u32 = 9;

pub fn anim_steer_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * ANIM_STEER_LOGIC_FPS / 1000.0).round() as u32
}

/// Current turn animation residual (C++ ModelConditionFlagType subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostAnimSteerTurnAnim {
    #[default]
    Invalid,
    CenterToRight,
    CenterToLeft,
    LeftToCenter,
    RightToCenter,
}

impl HostAnimSteerTurnAnim {
    pub fn model_condition_name(self) -> Option<&'static str> {
        match self {
            Self::Invalid => None,
            Self::CenterToRight => Some("CENTER_TO_RIGHT"),
            Self::CenterToLeft => Some("CENTER_TO_LEFT"),
            Self::LeftToCenter => Some("LEFT_TO_CENTER"),
            Self::RightToCenter => Some("RIGHT_TO_CENTER"),
        }
    }
}

/// Per-object AnimationSteeringUpdate residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostAnimationSteeringData {
    pub current_turn_anim: HostAnimSteerTurnAnim,
    pub next_transition_frame: u32,
    pub transition_frames: u32,
    /// Last applied model condition name residual (for honesty / client).
    pub active_condition: Option<String>,
}

impl Default for HostAnimationSteeringData {
    fn default() -> Self {
        Self {
            current_turn_anim: HostAnimSteerTurnAnim::Invalid,
            next_transition_frame: 0,
            transition_frames: BATTLE_BUS_MIN_TRANSITION_FRAMES,
            active_condition: None,
        }
    }
}

impl HostAnimationSteeringData {
    pub fn battle_bus_default() -> Self {
        Self::default()
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_animation_steering_template(template_name) {
            Some(Self::battle_bus_default())
        } else {
            None
        }
    }

    /// One frame residual. Returns newly set condition name if changed.
    pub fn tick(
        &mut self,
        now: u32,
        turning: PhysicsTurningType,
    ) -> Option<&'static str> {
        if now < self.next_transition_frame {
            return None;
        }
        let mut changed: Option<&'static str> = None;
        match self.current_turn_anim {
            HostAnimSteerTurnAnim::Invalid => {
                if turning == PhysicsTurningType::TurnNegative {
                    self.current_turn_anim = HostAnimSteerTurnAnim::CenterToRight;
                    self.next_transition_frame = now.saturating_add(self.transition_frames);
                    changed = Some("CENTER_TO_RIGHT");
                } else if turning == PhysicsTurningType::TurnPositive {
                    self.current_turn_anim = HostAnimSteerTurnAnim::CenterToLeft;
                    self.next_transition_frame = now.saturating_add(self.transition_frames);
                    changed = Some("CENTER_TO_LEFT");
                }
            }
            HostAnimSteerTurnAnim::CenterToRight => {
                if turning != PhysicsTurningType::TurnNegative {
                    self.current_turn_anim = HostAnimSteerTurnAnim::RightToCenter;
                    self.next_transition_frame = now.saturating_add(self.transition_frames);
                    changed = Some("RIGHT_TO_CENTER");
                }
            }
            HostAnimSteerTurnAnim::CenterToLeft => {
                if turning != PhysicsTurningType::TurnPositive {
                    self.current_turn_anim = HostAnimSteerTurnAnim::LeftToCenter;
                    self.next_transition_frame = now.saturating_add(self.transition_frames);
                    changed = Some("LEFT_TO_CENTER");
                }
            }
            HostAnimSteerTurnAnim::LeftToCenter | HostAnimSteerTurnAnim::RightToCenter => {
                if turning == PhysicsTurningType::TurnNone {
                    self.current_turn_anim = HostAnimSteerTurnAnim::Invalid;
                    self.next_transition_frame = now;
                    self.active_condition = None;
                    return None;
                }
            }
        }
        if let Some(name) = changed {
            self.active_condition = Some(name.to_string());
        }
        changed
    }
}

/// Battle Bus family templates with AnimationSteeringUpdate.
pub fn is_animation_steering_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("battlebus") || n.contains("battle_bus")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostAnimationSteeringRegistry {
    pub installed: u32,
    pub transitions: u32,
    pub left_turns: u32,
    pub right_turns: u32,
}

impl HostAnimationSteeringRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_transition(&mut self, cond: &str) {
        self.transitions = self.transitions.saturating_add(1);
        if cond.contains("LEFT") {
            self.left_turns = self.left_turns.saturating_add(1);
        }
        if cond.contains("RIGHT") {
            self.right_turns = self.right_turns.saturating_add(1);
        }
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.transitions > 0
    }
}

pub fn honesty_animation_steering_residual_ok() -> bool {
    anim_steer_ms_to_frames(BATTLE_BUS_MIN_TRANSITION_MS) == BATTLE_BUS_MIN_TRANSITION_FRAMES
        && is_animation_steering_template("GLAVehicleBattleBus")
        && is_animation_steering_template("Chem_GLAVehicleBattleBus")
        && !is_animation_steering_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack() {
        assert!(honesty_animation_steering_residual_ok());
    }

    #[test]
    fn turn_right_then_recenter() {
        let mut d = HostAnimationSteeringData::battle_bus_default();
        let c = d.tick(0, PhysicsTurningType::TurnNegative);
        assert_eq!(c, Some("CENTER_TO_RIGHT"));
        // During transition, ignore.
        assert!(d.tick(1, PhysicsTurningType::TurnNone).is_none());
        // After transition frames, recenter path.
        let c = d.tick(BATTLE_BUS_MIN_TRANSITION_FRAMES, PhysicsTurningType::TurnNone);
        assert_eq!(c, Some("RIGHT_TO_CENTER"));
        let c = d.tick(
            BATTLE_BUS_MIN_TRANSITION_FRAMES * 2,
            PhysicsTurningType::TurnNone,
        );
        assert!(c.is_none());
        assert_eq!(d.current_turn_anim, HostAnimSteerTurnAnim::Invalid);
    }

    #[test]
    fn turn_left() {
        let mut d = HostAnimationSteeringData::battle_bus_default();
        assert_eq!(
            d.tick(0, PhysicsTurningType::TurnPositive),
            Some("CENTER_TO_LEFT")
        );
    }
}

//! Receiving Object state for the authored AnimationSteeringUpdate behavior.

use crate::game_logic::ThingTemplate;
use crate::game_logic::object::PhysicsTurningType;
use gamelogic::object::behavior::AnimationSteeringUpdateModuleData;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub const BATTLE_BUS_MIN_TRANSITION_MS: u32 = 300;
#[cfg(test)]
pub const BATTLE_BUS_MIN_TRANSITION_FRAMES: u32 = 9;

/// Current turn animation residual (C++ ModelConditionFlagType subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum HostAnimSteerTurnAnim {
    #[default]
    Invalid,
    CenterToRight,
    CenterToLeft,
    LeftToCenter,
    RightToCenter,
}

impl HostAnimSteerTurnAnim {
    pub(crate) fn from_ordinal(value: u8) -> Self {
        match value {
            1 => Self::CenterToRight,
            2 => Self::CenterToLeft,
            3 => Self::LeftToCenter,
            4 => Self::RightToCenter,
            _ => Self::Invalid,
        }
    }

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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostAnimationSteeringData {
    pub current_turn_anim: HostAnimSteerTurnAnim,
    pub next_transition_frame: u32,
    pub transition_frames: u32,
    /// Last applied model condition name residual (for honesty / client).
    pub active_condition: Option<String>,
}

impl HostAnimationSteeringData {
    /// C++ installs the behavior from the resolved ThingTemplate and uses
    /// the module data's frame duration, including a zero-duration default.
    pub(crate) fn from_authored(template: &ThingTemplate) -> Option<Self> {
        let data = template
            .authored_module_data::<AnimationSteeringUpdateModuleData>("AnimationSteeringUpdate")?;
        Some(Self {
            transition_frames: data.transition_frames,
            ..Self::default()
        })
    }

    /// C++ AnimationSteeringUpdate.cpp:55–108. Clearing a completed turn
    /// returns no new animation.
    pub(crate) fn tick(
        &mut self,
        now: u32,
        turning: PhysicsTurningType,
    ) -> Option<HostAnimSteerTurnAnim> {
        use HostAnimSteerTurnAnim::{
            CenterToLeft, CenterToRight, Invalid, LeftToCenter, RightToCenter,
        };
        use PhysicsTurningType::{TurnNegative, TurnNone, TurnPositive};

        if now < self.next_transition_frame {
            return None;
        }
        let next = match (self.current_turn_anim, turning) {
            (Invalid, TurnNegative) => CenterToRight,
            (Invalid, TurnPositive) => CenterToLeft,
            (CenterToRight, TurnNone | TurnPositive) => RightToCenter,
            (CenterToLeft, TurnNone | TurnNegative) => LeftToCenter,
            (LeftToCenter | RightToCenter, TurnNone) => Invalid,
            _ => return None,
        };
        self.current_turn_anim = next;
        let Some(condition) = next.model_condition_name() else {
            self.next_transition_frame = now;
            self.active_condition = None;
            return None;
        };
        self.next_transition_frame = now.wrapping_add(self.transition_frames);
        self.active_condition = Some(condition.to_owned());
        Some(next)
    }
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
    pub(crate) fn record_transition(&mut self, transition: HostAnimSteerTurnAnim) {
        use HostAnimSteerTurnAnim::{
            CenterToLeft, CenterToRight, Invalid, LeftToCenter, RightToCenter,
        };
        match transition {
            CenterToLeft | LeftToCenter => self.left_turns = self.left_turns.saturating_add(1),
            CenterToRight | RightToCenter => {
                self.right_turns = self.right_turns.saturating_add(1);
            }
            Invalid => return,
        }
        self.transitions = self.transitions.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.transitions > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battle_bus_timing() -> HostAnimationSteeringData {
        HostAnimationSteeringData {
            transition_frames: BATTLE_BUS_MIN_TRANSITION_FRAMES,
            ..Default::default()
        }
    }

    #[test]
    fn transition_deadline_wraps_like_the_original_frame_counter() {
        let mut data = HostAnimationSteeringData {
            transition_frames: 2,
            ..Default::default()
        };
        assert_eq!(
            data.tick(u32::MAX, PhysicsTurningType::TurnNegative),
            Some(HostAnimSteerTurnAnim::CenterToRight)
        );
        assert_eq!(data.next_transition_frame, 1);
        assert!(data.tick(0, PhysicsTurningType::TurnNone).is_none());
        assert_eq!(
            data.tick(1, PhysicsTurningType::TurnNone),
            Some(HostAnimSteerTurnAnim::RightToCenter)
        );
    }

    #[test]
    fn turn_right_then_recenter() {
        let mut d = battle_bus_timing();
        let c = d.tick(0, PhysicsTurningType::TurnNegative);
        assert_eq!(c, Some(HostAnimSteerTurnAnim::CenterToRight));
        // During transition, ignore.
        assert!(d.tick(1, PhysicsTurningType::TurnNone).is_none());
        // After transition frames, recenter path.
        let c = d.tick(
            BATTLE_BUS_MIN_TRANSITION_FRAMES,
            PhysicsTurningType::TurnNone,
        );
        assert_eq!(c, Some(HostAnimSteerTurnAnim::RightToCenter));
        let c = d.tick(
            BATTLE_BUS_MIN_TRANSITION_FRAMES * 2,
            PhysicsTurningType::TurnNone,
        );
        assert!(c.is_none());
        assert_eq!(d.current_turn_anim, HostAnimSteerTurnAnim::Invalid);
    }

    #[test]
    fn turn_left() {
        let mut d = battle_bus_timing();
        assert_eq!(
            d.tick(0, PhysicsTurningType::TurnPositive),
            Some(HostAnimSteerTurnAnim::CenterToLeft)
        );
    }
}

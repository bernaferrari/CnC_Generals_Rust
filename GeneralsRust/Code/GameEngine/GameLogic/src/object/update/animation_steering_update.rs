// AnimationSteeringUpdate - Uses animation states to handle steering
// Author: Kris Morness, May 2003
// Ported to Rust

use crate::object::drawable::DrawableArcExt;
use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct AnimationSteeringUpdateModuleData {
    pub transition_frames: u32,
}

impl Default for AnimationSteeringUpdateModuleData {
    fn default() -> Self {
        Self {
            transition_frames: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnimationSteeringUpdate {
    thing: ThingId,
    module_data: AnimationSteeringUpdateModuleData,
    current_turn_anim: ModelConditionFlag,
    next_transition_frame: u32,
}

impl AnimationSteeringUpdate {
    pub fn new(thing: ThingId, module_data: AnimationSteeringUpdateModuleData) -> Self {
        Self {
            thing,
            module_data,
            current_turn_anim: ModelConditionFlag::Invalid,
            next_transition_frame: 0,
        }
    }

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return UpdateSleepTime::Forever;
        };

        let Some(physics) = object.get_physics() else {
            return UpdateSleepTime::None;
        };

        let Some(drawable) = object.get_drawable() else {
            return UpdateSleepTime::None;
        };

        let now = ctx.game_logic.get_frame();

        if now < self.next_transition_frame {
            return UpdateSleepTime::None;
        }

        let current_turn = physics.get_turning();

        let current_turn_type = if current_turn < 0.0 {
            PhysicsTurningType::Negative
        } else if current_turn > 0.0 {
            PhysicsTurningType::Positive
        } else {
            PhysicsTurningType::None
        };

        match self.current_turn_anim {
            ModelConditionFlag::Invalid => {
                // We're currently going straight. Check if we want to turn.
                match current_turn_type {
                    PhysicsTurningType::Negative => {
                        // Initiate a right turn
                        drawable.set_model_condition_state(ModelConditionFlag::CenterToRight);
                        self.next_transition_frame = now + self.module_data.transition_frames;
                        self.current_turn_anim = ModelConditionFlag::CenterToRight;
                    }
                    PhysicsTurningType::Positive => {
                        // Initiate a left turn
                        drawable.set_model_condition_state(ModelConditionFlag::CenterToLeft);
                        self.next_transition_frame = now + self.module_data.transition_frames;
                        self.current_turn_anim = ModelConditionFlag::CenterToLeft;
                    }
                    PhysicsTurningType::None => {}
                }
            }

            ModelConditionFlag::CenterToRight => {
                // We're currently initiating a turn to the right.
                // We can go back to center or maintain the turn.
                if current_turn_type != PhysicsTurningType::Negative {
                    // Recenter!
                    drawable.clear_and_set_model_condition_state(
                        ModelConditionFlag::CenterToRight,
                        ModelConditionFlag::RightToCenter,
                    );
                    self.next_transition_frame = now + self.module_data.transition_frames;
                    self.current_turn_anim = ModelConditionFlag::RightToCenter;
                }
            }

            ModelConditionFlag::CenterToLeft => {
                // We're currently initiating a turn to the left.
                // We can go back to center or maintain the turn.
                if current_turn_type != PhysicsTurningType::Positive {
                    // Recenter!
                    drawable.clear_and_set_model_condition_state(
                        ModelConditionFlag::CenterToLeft,
                        ModelConditionFlag::LeftToCenter,
                    );
                    self.next_transition_frame = now + self.module_data.transition_frames;
                    self.current_turn_anim = ModelConditionFlag::LeftToCenter;
                }
            }

            ModelConditionFlag::LeftToCenter | ModelConditionFlag::RightToCenter => {
                if current_turn_type == PhysicsTurningType::None {
                    // Finish the turn
                    drawable.clear_model_condition_flags(
                        ModelConditionFlag::LeftToCenter | ModelConditionFlag::RightToCenter,
                    );
                    self.next_transition_frame = now;
                    self.current_turn_anim = ModelConditionFlag::Invalid;
                }
            }

            _ => {}
        }

        UpdateSleepTime::None
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("AnimationSteeringUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut anim_bits = self.current_turn_anim.bits();
        xfer_io(xfer.xfer_u128(&mut anim_bits), "current_turn_anim");
        let mut next_frame = self.next_transition_frame;
        xfer_io(xfer.xfer_u32(&mut next_frame), "next_transition_frame");
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("AnimationSteeringUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            let mut anim_bits = 0u128;
            xfer_io(xfer.xfer_u128(&mut anim_bits), "current_turn_anim");
            self.current_turn_anim = ModelConditionFlag::from_bits_truncate(anim_bits);
            xfer_io(
                xfer.xfer_u32(&mut self.next_transition_frame),
                "next_transition_frame",
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsTurningType {
    None,
    Positive,
    Negative,
}

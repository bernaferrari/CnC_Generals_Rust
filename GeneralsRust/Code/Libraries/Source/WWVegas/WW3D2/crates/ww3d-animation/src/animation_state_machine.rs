//! Animation State Machine
//!
//! Provides a state machine for managing character animations with automatic
//! transitions between states like idle, walk, run, attack, death, etc.
//!
//! Reference: Common game animation state machine patterns used in RTS games
use crate::hanim::{AnimationMode, HAnimClass};
use crate::skeletal_animation::AnimatedModel;
use glam::Mat4;
use std::collections::HashMap;

/// Standard animation states for RTS units
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimationState {
    /// Idle/standing animation
    Idle,
    /// Walking animation
    Walk,
    /// Running animation (faster movement)
    Run,
    /// Attack animation
    Attack,
    /// Taking damage animation
    Hit,
    /// Death animation
    Death,
    /// Victory/celebration animation
    Victory,
    /// Building/construction animation
    Build,
    /// Special ability animation
    Special,
    /// Custom state (for mod support)
    Custom(u32),
}

impl AnimationState {
    /// Get the default animation name for this state
    pub fn default_name(&self) -> &str {
        match self {
            AnimationState::Idle => "IDLE",
            AnimationState::Walk => "WALK",
            AnimationState::Run => "RUN",
            AnimationState::Attack => "ATTACK",
            AnimationState::Hit => "HIT",
            AnimationState::Death => "DEATH",
            AnimationState::Victory => "VICTORY",
            AnimationState::Build => "BUILD",
            AnimationState::Special => "SPECIAL",
            AnimationState::Custom(_) => "CUSTOM",
        }
    }

    /// Check if this state should loop
    pub fn is_looping(&self) -> bool {
        matches!(
            self,
            AnimationState::Idle
                | AnimationState::Walk
                | AnimationState::Run
                | AnimationState::Build
        )
    }

    /// Check if this state can be interrupted
    pub fn is_interruptible(&self) -> bool {
        !matches!(self, AnimationState::Death)
    }
}

/// Transition condition between animation states
#[derive(Debug, Clone)]
pub enum TransitionCondition {
    /// Transition immediately
    Immediate,
    /// Transition when current animation completes
    OnComplete,
    /// Transition after a time delay (seconds)
    AfterDelay(f32),
    /// Custom condition based on frame number
    OnFrame(u32),
}

/// Animation state transition
#[derive(Debug, Clone)]
pub struct StateTransition {
    /// Source state
    pub from: AnimationState,
    /// Target state
    pub to: AnimationState,
    /// Condition for triggering transition
    pub condition: TransitionCondition,
    /// Blend duration in seconds
    pub blend_duration: f32,
}

/// Animation state machine
pub struct AnimationStateMachine {
    /// Current animation state
    current_state: AnimationState,
    /// Previous state (for debugging)
    previous_state: AnimationState,
    /// Animated model
    model: AnimatedModel,
    /// Animation mappings (state -> animation name)
    animation_map: HashMap<AnimationState, String>,
    /// Available animations (name -> animation)
    animations: HashMap<String, HAnimClass>,
    /// Registered transitions
    transitions: Vec<StateTransition>,
    /// Time in current state
    state_time: f32,
    /// Pending transition (if any)
    pending_transition: Option<PendingTransition>,
}

#[derive(Debug, Clone)]
struct PendingTransition {
    target_state: AnimationState,
    blend_duration: f32,
    condition: TransitionCondition,
    elapsed: f32,
}

impl AnimationStateMachine {
    /// Create a new animation state machine
    pub fn new(model: AnimatedModel) -> Self {
        Self {
            current_state: AnimationState::Idle,
            previous_state: AnimationState::Idle,
            model,
            animation_map: HashMap::new(),
            animations: HashMap::new(),
            transitions: Vec::new(),
            state_time: 0.0,
            pending_transition: None,
        }
    }

    /// Register an animation for a state
    pub fn register_animation(
        &mut self,
        state: AnimationState,
        name: impl Into<String>,
        animation: HAnimClass,
    ) {
        let name = name.into();
        self.animation_map.insert(state, name.clone());
        self.animations.insert(name, animation);
    }

    /// Register a state transition
    pub fn register_transition(&mut self, transition: StateTransition) {
        self.transitions.push(transition);
    }

    /// Register standard transitions for common state pairs
    pub fn register_standard_transitions(&mut self) {
        // Idle can transition to most states immediately
        self.register_transition(StateTransition {
            from: AnimationState::Idle,
            to: AnimationState::Walk,
            condition: TransitionCondition::Immediate,
            blend_duration: 0.2,
        });

        self.register_transition(StateTransition {
            from: AnimationState::Idle,
            to: AnimationState::Attack,
            condition: TransitionCondition::Immediate,
            blend_duration: 0.1,
        });

        // Walk can transition to idle or run
        self.register_transition(StateTransition {
            from: AnimationState::Walk,
            to: AnimationState::Idle,
            condition: TransitionCondition::Immediate,
            blend_duration: 0.2,
        });

        self.register_transition(StateTransition {
            from: AnimationState::Walk,
            to: AnimationState::Run,
            condition: TransitionCondition::Immediate,
            blend_duration: 0.15,
        });

        // Attack returns to idle when complete
        self.register_transition(StateTransition {
            from: AnimationState::Attack,
            to: AnimationState::Idle,
            condition: TransitionCondition::OnComplete,
            blend_duration: 0.1,
        });

        // Any state can transition to death
        for state in [
            AnimationState::Idle,
            AnimationState::Walk,
            AnimationState::Run,
            AnimationState::Attack,
        ] {
            self.register_transition(StateTransition {
                from: state,
                to: AnimationState::Death,
                condition: TransitionCondition::Immediate,
                blend_duration: 0.0,
            });
        }
    }

    /// Request a state change
    pub fn request_state(&mut self, new_state: AnimationState) -> bool {
        if !self.current_state.is_interruptible() {
            return false;
        }

        // Find matching transition
        let transition = self
            .transitions
            .iter()
            .find(|t| t.from == self.current_state && t.to == new_state);

        if let Some(transition) = transition {
            match transition.condition {
                TransitionCondition::Immediate => {
                    self.execute_transition(new_state, transition.blend_duration);
                    true
                }
                TransitionCondition::OnComplete => {
                    self.pending_transition = Some(PendingTransition {
                        target_state: new_state,
                        blend_duration: transition.blend_duration,
                        condition: transition.condition.clone(),
                        elapsed: 0.0,
                    });
                    true
                }
                TransitionCondition::AfterDelay(_delay) => {
                    self.pending_transition = Some(PendingTransition {
                        target_state: new_state,
                        blend_duration: transition.blend_duration,
                        condition: transition.condition.clone(),
                        elapsed: 0.0,
                    });
                    true
                }
                TransitionCondition::OnFrame(_) => {
                    self.pending_transition = Some(PendingTransition {
                        target_state: new_state,
                        blend_duration: transition.blend_duration,
                        condition: transition.condition.clone(),
                        elapsed: 0.0,
                    });
                    true
                }
            }
        } else {
            // No transition defined, allow direct change
            self.execute_transition(new_state, 0.2);
            true
        }
    }

    /// Execute state transition
    fn execute_transition(&mut self, new_state: AnimationState, blend_duration: f32) {
        self.previous_state = self.current_state;
        self.current_state = new_state;
        self.state_time = 0.0;

        // Get animation for new state
        if let Some(anim_name) = self.animation_map.get(&new_state) {
            if let Some(animation) = self.animations.get(anim_name).cloned() {
                // Set animation mode based on state
                let mode = if new_state.is_looping() {
                    AnimationMode::Loop
                } else {
                    AnimationMode::Once
                };

                // Transition to animation
                self.model.set_animation_mode(mode);
                if blend_duration > 0.0 {
                    self.model.transition_to(animation, blend_duration);
                } else {
                    self.model.set_animation(animation);
                }
            }
        }
    }

    /// Update state machine
    pub fn update(&mut self, delta_time: f32, root_transform: Mat4) {
        self.state_time += delta_time;

        // Check for automatic transitions
        if let Some(pending) = self.pending_transition.as_mut() {
            pending.elapsed += delta_time;
            let should_transition = match pending.condition {
                TransitionCondition::OnComplete => {
                    // Check if animation is complete
                    if let Some(anim_name) = self.animation_map.get(&self.current_state) {
                        if let Some(animation) = self.animations.get(anim_name) {
                            let current_frame = self.model.get_current_frame();
                            let num_frames = animation.get_num_frames() as f32;
                            current_frame >= num_frames - 1.0
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                TransitionCondition::AfterDelay(delay) => pending.elapsed >= delay,
                TransitionCondition::OnFrame(frame) => {
                    let current_frame = self.model.get_current_frame() as u32;
                    current_frame >= frame
                }
                _ => false,
            };

            if should_transition {
                let pending = self.pending_transition.take().unwrap();
                self.execute_transition(pending.target_state, pending.blend_duration);
            }
        }

        // Update animated model
        self.model.update(delta_time, root_transform);
    }

    /// Get current state
    pub fn current_state(&self) -> AnimationState {
        self.current_state
    }

    /// Get previous state
    pub fn previous_state(&self) -> AnimationState {
        self.previous_state
    }

    /// Get time in current state
    pub fn state_time(&self) -> f32 {
        self.state_time
    }

    /// Get reference to animated model
    pub fn model(&self) -> &AnimatedModel {
        &self.model
    }

    /// Get mutable reference to animated model
    pub fn model_mut(&mut self) -> &mut AnimatedModel {
        &mut self.model
    }

    /// Get skinning matrices for rendering
    pub fn get_skinning_matrices(&self) -> Vec<Mat4> {
        self.model.get_skinning_matrices()
    }

    /// Get skinning matrices as flat array
    pub fn get_skinning_matrices_flat(&self) -> Vec<f32> {
        self.model.get_skinning_matrices_flat()
    }
}

/// Builder pattern for creating animation state machines
pub struct AnimationStateMachineBuilder {
    machine: AnimationStateMachine,
}

impl AnimationStateMachineBuilder {
    /// Create a new builder
    pub fn new(model: AnimatedModel) -> Self {
        Self {
            machine: AnimationStateMachine::new(model),
        }
    }

    /// Add an animation
    pub fn with_animation(
        mut self,
        state: AnimationState,
        name: impl Into<String>,
        animation: HAnimClass,
    ) -> Self {
        self.machine.register_animation(state, name, animation);
        self
    }

    /// Add a transition
    pub fn with_transition(mut self, transition: StateTransition) -> Self {
        self.machine.register_transition(transition);
        self
    }

    /// Add standard transitions
    pub fn with_standard_transitions(mut self) -> Self {
        self.machine.register_standard_transitions();
        self
    }

    /// Set initial state
    pub fn with_initial_state(mut self, state: AnimationState) -> Self {
        self.machine.current_state = state;
        self
    }

    /// Build the state machine
    pub fn build(self) -> AnimationStateMachine {
        self.machine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htree::HTreeClass;

    #[test]
    fn test_animation_state() {
        assert_eq!(AnimationState::Idle.default_name(), "IDLE");
        assert!(AnimationState::Idle.is_looping());
        assert!(!AnimationState::Attack.is_looping());
        assert!(AnimationState::Idle.is_interruptible());
        assert!(!AnimationState::Death.is_interruptible());
    }

    #[test]
    fn test_state_machine_creation() {
        let mut htree = HTreeClass::new();
        htree.init_default();
        let model = AnimatedModel::new(htree);
        let machine = AnimationStateMachine::new(model);
        assert_eq!(machine.current_state(), AnimationState::Idle);
    }

    #[test]
    fn test_builder() {
        let mut htree = HTreeClass::new();
        htree.init_default();
        let model = AnimatedModel::new(htree);
        let machine = AnimationStateMachineBuilder::new(model)
            .with_standard_transitions()
            .with_initial_state(AnimationState::Idle)
            .build();
        assert_eq!(machine.current_state(), AnimationState::Idle);
    }
}

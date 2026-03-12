//! Deploy Style AI Update Module - Unit deployment behavior
//!
//! Handles AI for units that can deploy/pack (e.g., artillery, mobile AA)
//! including:
//! - Deployment decision making
//! - Packing/unpacking
//! - Position selection
//! - Mobility vs firepower trade-off

use super::{
    AIModulePriority, AIModuleState, AIModuleType, AIUpdateContext, AIUpdateModuleTrait,
    AIUpdateResult,
};
use crate::ai::AiError;
use crate::common::{Coord3D, ObjectID, Real};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployState {
    Packed,
    Deploying,
    Deployed,
    Packing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployMode {
    Manual,     // Only deploy on explicit command
    Auto,       // Auto-deploy when stopped
    AutoCombat, // Auto-deploy when combat detected
}

#[derive(Debug)]
pub struct DeployStyleAIUpdate {
    state: AIModuleState,
    deploy_state: DeployState,
    deploy_mode: DeployMode,

    deploy_time: Real,
    pack_time: Real,
    transition_timer: Real,

    deployed_position: Option<Coord3D>,

    auto_pack_when_threatened: bool,
    threat_threshold: f32,

    last_deployment: u32,
    min_deploy_duration: u32, // Minimum frames to stay deployed
}

impl DeployStyleAIUpdate {
    pub fn new() -> Self {
        Self {
            state: AIModuleState::Idle,
            deploy_state: DeployState::Packed,
            deploy_mode: DeployMode::Auto,
            deploy_time: 3.0,
            pack_time: 2.0,
            transition_timer: 0.0,
            deployed_position: None,
            auto_pack_when_threatened: true,
            threat_threshold: 0.7,
            last_deployment: 0,
            min_deploy_duration: 150, // 5 seconds minimum
        }
    }

    pub fn set_deploy_mode(&mut self, mode: DeployMode) {
        self.deploy_mode = mode;
    }

    pub fn deploy(&mut self) {
        if matches!(self.deploy_state, DeployState::Packed) {
            self.deploy_state = DeployState::Deploying;
            self.transition_timer = 0.0;
        }
    }

    pub fn pack(&mut self) {
        if matches!(self.deploy_state, DeployState::Deployed) {
            self.deploy_state = DeployState::Packing;
            self.transition_timer = 0.0;
        }
    }

    pub fn is_deployed(&self) -> bool {
        matches!(self.deploy_state, DeployState::Deployed)
    }

    pub fn is_packed(&self) -> bool {
        matches!(self.deploy_state, DeployState::Packed)
    }

    fn should_auto_deploy(&self, context: &AIUpdateContext) -> bool {
        match self.deploy_mode {
            DeployMode::Manual => false,
            DeployMode::Auto => !context.is_moving,
            DeployMode::AutoCombat => !context.is_moving && context.current_target.is_some(),
        }
    }

    fn should_auto_pack(&self, context: &AIUpdateContext) -> bool {
        // Pack if we need to move or under threat
        context.is_moving
            || (self.auto_pack_when_threatened && context.health_percentage < self.threat_threshold)
    }

    fn update_deployment(&mut self, delta_time: Real) -> AIUpdateResult<()> {
        self.transition_timer += delta_time;

        if self.transition_timer >= self.deploy_time {
            self.deploy_state = DeployState::Deployed;
            self.transition_timer = 0.0;
        }

        Ok(())
    }

    fn update_packing(&mut self, delta_time: Real) -> AIUpdateResult<()> {
        self.transition_timer += delta_time;

        if self.transition_timer >= self.pack_time {
            self.deploy_state = DeployState::Packed;
            self.transition_timer = 0.0;
            self.deployed_position = None;
        }

        Ok(())
    }
}

impl AIUpdateModuleTrait for DeployStyleAIUpdate {
    fn get_module_type(&self) -> AIModuleType {
        AIModuleType::DeployStyle
    }

    fn get_priority(&self) -> AIModulePriority {
        AIModulePriority::Normal
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, _context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        self.deploy_state = DeployState::Packed;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.deploy_state = DeployState::Packed;
        self.deployed_position = None;
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        match self.deploy_state {
            DeployState::Packed => {
                if self.should_auto_deploy(context) {
                    self.deploy();
                    self.deployed_position = Some(context.position);
                    self.last_deployment = context.current_frame;
                }
            }
            DeployState::Deploying => {
                self.update_deployment(context.delta_time)?;
            }
            DeployState::Deployed => {
                // Check if should pack
                if self.should_auto_pack(context) {
                    let time_deployed = context.current_frame - self.last_deployment;
                    if time_deployed >= self.min_deploy_duration {
                        self.pack();
                    }
                }
            }
            DeployState::Packing => {
                self.update_packing(context.delta_time)?;
            }
        }

        Ok(())
    }

    fn should_update(&self, _context: &AIUpdateContext) -> bool {
        true
    }

    fn on_damage_received(
        &mut self,
        _damage_amount: f32,
        _attacker: Option<ObjectID>,
    ) -> AIUpdateResult<()> {
        // Consider packing if taking heavy damage
        if self.auto_pack_when_threatened && self.is_deployed() {
            self.pack();
        }
        Ok(())
    }
}

impl Default for DeployStyleAIUpdate {
    fn default() -> Self {
        Self::new()
    }
}

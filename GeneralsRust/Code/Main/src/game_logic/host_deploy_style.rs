//! Host DeployStyleAIUpdate residual (pack/unpack before move/attack).
//!
//! C++: units that must unpack to attack and pack before moving
//! (`DeployStyleAIUpdate::update` state machine).
//!
//! Timing and policy come from the exact parsed Object INI
//! `DeployStyleAIUpdate` module on each template.  No template-name list is
//! used to create deploy authority.
//!
//! States (simplified host residual):
//! - ReadyToMove: undeployed, may path
//! - Deploying: unpacking timer → ReadyToAttack
//! - ReadyToAttack: deployed, may fire
//! - Undeploying: packing timer → ReadyToMove
//!
//! Fail-closed: this compact logic state does not fabricate per-turret
//! alignment/reset or manual Drawable animation-frame behavior.  Those source
//! flags remain on `DeployStyleMetadata` for snapshots and later rendering
//! work instead of becoming a guessed delay or visual.

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const DEPLOY_STYLE_LOGIC_FPS: f32 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostDeployStyleState {
    #[default]
    ReadyToMove,
    Deploying,
    ReadyToAttack,
    Undeploying,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostDeployStyleData {
    pub state: HostDeployStyleState,
    /// Frame when current pack/unpack completes (0 = inactive).
    pub ready_frame: u32,
    pub pack_frames: u32,
    pub unpack_frames: u32,
}

impl HostDeployStyleData {
    /// Construct live deploy state from the Object INI module data carried by
    /// the object's template.  `DeployStyleMetadata` already stores C++
    /// `parseDurationUnsignedInt` values in logic frames.
    pub fn from_metadata(metadata: &crate::game_logic::DeployStyleMetadata) -> Self {
        Self {
            state: HostDeployStyleState::ReadyToMove,
            ready_frame: 0,
            pack_frames: metadata.pack_time_frames,
            unpack_frames: metadata.unpack_time_frames,
        }
    }

    pub fn is_ready_to_attack(&self) -> bool {
        matches!(self.state, HostDeployStyleState::ReadyToAttack)
    }

    pub fn is_ready_to_move(&self) -> bool {
        matches!(self.state, HostDeployStyleState::ReadyToMove)
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self.state,
            HostDeployStyleState::Deploying | HostDeployStyleState::Undeploying
        )
    }

    /// Begin unpack when attack in range while undeployed.
    /// Returns true if transition started.
    pub fn begin_deploy(&mut self, current_frame: u32) -> bool {
        match self.state {
            HostDeployStyleState::ReadyToMove => {
                self.state = HostDeployStyleState::Deploying;
                self.ready_frame = current_frame.saturating_add(self.unpack_frames.max(1));
                true
            }
            HostDeployStyleState::Undeploying => {
                // C++ reverse undeploy at current progress residual → start deploy.
                self.state = HostDeployStyleState::Deploying;
                self.ready_frame = current_frame.saturating_add(self.unpack_frames.max(1));
                true
            }
            HostDeployStyleState::Deploying | HostDeployStyleState::ReadyToAttack => false,
        }
    }

    /// Begin pack when ordered to move while deployed/attacking.
    pub fn begin_undeploy(&mut self, current_frame: u32) -> bool {
        match self.state {
            HostDeployStyleState::ReadyToAttack => {
                self.state = HostDeployStyleState::Undeploying;
                self.ready_frame = current_frame.saturating_add(self.pack_frames.max(1));
                true
            }
            HostDeployStyleState::Deploying => {
                // Reverse deploy → undeploy.
                self.state = HostDeployStyleState::Undeploying;
                self.ready_frame = current_frame.saturating_add(self.pack_frames.max(1));
                true
            }
            HostDeployStyleState::Undeploying | HostDeployStyleState::ReadyToMove => false,
        }
    }

    /// Advance timers; returns (became_ready_to_attack, became_ready_to_move).
    pub fn tick(&mut self, current_frame: u32) -> (bool, bool) {
        if self.ready_frame == 0 || current_frame < self.ready_frame {
            return (false, false);
        }
        match self.state {
            HostDeployStyleState::Deploying => {
                self.state = HostDeployStyleState::ReadyToAttack;
                self.ready_frame = 0;
                (true, false)
            }
            HostDeployStyleState::Undeploying => {
                self.state = HostDeployStyleState::ReadyToMove;
                self.ready_frame = 0;
                (false, true)
            }
            _ => {
                self.ready_frame = 0;
                (false, false)
            }
        }
    }
}

/// Honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostDeployStyleRegistry {
    pub deploys: u32,
    pub undeploys: u32,
    pub blocked_fires: u32,
    pub blocked_moves: u32,
}

impl HostDeployStyleRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_deploy(&mut self) {
        self.deploys = self.deploys.saturating_add(1);
    }
    pub fn record_undeploy(&mut self) {
        self.undeploys = self.undeploys.saturating_add(1);
    }
    pub fn record_blocked_fire(&mut self) {
        self.blocked_fires = self.blocked_fires.saturating_add(1);
    }
    pub fn record_blocked_move(&mut self) {
        self.blocked_moves = self.blocked_moves.saturating_add(1);
    }
    pub fn honesty_deploy_ok(&self) -> bool {
        self.deploys > 0
    }
    pub fn honesty_undeploy_ok(&self) -> bool {
        self.undeploys > 0
    }
}

pub fn deploy_style_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    // C++ INI::parseDurationUnsignedInt: ceil(msec * logic_fps / 1000).
    // Use integer math to retain exact authored boundaries without f32 residue.
    ((u64::from(ms) * 30 + 999) / 1_000) as u32
}

pub fn honesty_deploy_style_residual_ok() -> bool {
    deploy_style_ms_to_frames(1_000) == 30
        && deploy_style_ms_to_frames(3_333) == 100
        && deploy_style_ms_to_frames(34) == 2
        && (DEPLOY_STYLE_LOGIC_FPS - 30.0).abs() < f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_metadata_drives_pack_unpack_state_machine() {
        assert!(honesty_deploy_style_residual_ok());
        let metadata = crate::game_logic::DeployStyleMetadata {
            pack_time_frames: 30,
            unpack_time_frames: 30,
            ..Default::default()
        };
        let mut d = HostDeployStyleData::from_metadata(&metadata);
        assert!(d.is_ready_to_move());
        assert!(d.begin_deploy(0));
        assert!(d.is_busy());
        assert!(!d.is_ready_to_attack());
        let (atk, mv) = d.tick(29);
        assert!(!atk && !mv);
        let (atk, mv) = d.tick(30);
        assert!(atk && !mv);
        assert!(d.is_ready_to_attack());
        assert!(d.begin_undeploy(40));
        let (atk, mv) = d.tick(70);
        assert!(!atk && mv);
        assert!(d.is_ready_to_move());
    }

    #[test]
    fn metadata_retains_longer_authored_pack() {
        let metadata = crate::game_logic::DeployStyleMetadata {
            pack_time_frames: 100,
            unpack_time_frames: 100,
            turrets_function_only_when_deployed: true,
            turrets_must_center_before_packing: true,
            manual_deploy_animations: true,
            ..Default::default()
        };
        let d = HostDeployStyleData::from_metadata(&metadata);
        assert_eq!(d.pack_frames, 100);
        assert_eq!(d.unpack_frames, 100);
    }
}

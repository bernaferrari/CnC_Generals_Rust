//! Host DeployStyleAIUpdate residual (pack/unpack before move/attack).
//!
//! C++: units that must unpack to attack and pack before moving
//! (`DeployStyleAIUpdate::update` state machine).
//!
//! Retail peels:
//! - AmericaVehicleSentryDrone: PackTime/UnpackTime **1000**ms → **30**f
//! - ChinaVehicleNukeLauncher: PackTime/UnpackTime **3333**ms → **100**f
//!
//! States (simplified host residual):
//! - ReadyToMove: undeployed, may path
//! - Deploying: unpacking timer → ReadyToAttack
//! - ReadyToAttack: deployed, may fire
//! - Undeploying: packing timer → ReadyToMove
//!
//! Fail-closed: not turret align-before-pack / manual anim scrub / guard-idle auto-deploy.

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const DEPLOY_STYLE_LOGIC_FPS: f32 = 30.0;

/// Sentry drone pack/unpack residual (msec).
pub const SENTRY_DRONE_PACK_MS: u32 = 1_000;
pub const SENTRY_DRONE_UNPACK_MS: u32 = 1_000;
/// 1000ms → 30 frames @ 30 FPS.
pub const SENTRY_DRONE_PACK_FRAMES: u32 = 30;
pub const SENTRY_DRONE_UNPACK_FRAMES: u32 = 30;

/// Nuke cannon pack/unpack residual (msec).
pub const NUKE_LAUNCHER_PACK_MS: u32 = 3_333;
pub const NUKE_LAUNCHER_UNPACK_MS: u32 = 3_333;
/// 3333ms → 100 frames @ 30 FPS.
pub const NUKE_LAUNCHER_PACK_FRAMES: u32 = 100;
pub const NUKE_LAUNCHER_UNPACK_FRAMES: u32 = 100;

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
    pub fn for_template(template_name: &str) -> Option<Self> {
        let (pack, unpack) = pack_unpack_frames_for_template(template_name)?;
        Some(Self {
            state: HostDeployStyleState::ReadyToMove,
            ready_frame: 0,
            pack_frames: pack,
            unpack_frames: unpack,
        })
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
    ((ms as f32) * DEPLOY_STYLE_LOGIC_FPS / 1000.0).round() as u32
}

pub fn is_deploy_style_template(name: &str) -> bool {
    pack_unpack_frames_for_template(name).is_some()
}

pub fn pack_unpack_frames_for_template(name: &str) -> Option<(u32, u32)> {
    let n = name.to_ascii_lowercase();
    if n.contains("sentrydrone") || n.contains("sentry_drone") {
        return Some((
            deploy_style_ms_to_frames(SENTRY_DRONE_PACK_MS),
            deploy_style_ms_to_frames(SENTRY_DRONE_UNPACK_MS),
        ));
    }
    if n.contains("nukelauncher") || n.contains("nuke_launcher") || n.contains("nukecannon") {
        return Some((
            deploy_style_ms_to_frames(NUKE_LAUNCHER_PACK_MS),
            deploy_style_ms_to_frames(NUKE_LAUNCHER_UNPACK_MS),
        ));
    }
    None
}

pub fn honesty_deploy_style_residual_ok() -> bool {
    deploy_style_ms_to_frames(1_000) == 30
        && deploy_style_ms_to_frames(3_333) == 100
        && SENTRY_DRONE_PACK_FRAMES == 30
        && NUKE_LAUNCHER_PACK_FRAMES == 100
        && is_deploy_style_template("AmericaVehicleSentryDrone")
        && is_deploy_style_template("ChinaVehicleNukeLauncher")
        && !is_deploy_style_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack_peels_and_state_machine() {
        assert!(honesty_deploy_style_residual_ok());
        let mut d = HostDeployStyleData::for_template("AmericaVehicleSentryDrone").unwrap();
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
    fn nuke_launcher_longer_pack() {
        let d = HostDeployStyleData::for_template("ChinaVehicleNukeLauncher").unwrap();
        assert_eq!(d.pack_frames, 100);
        assert_eq!(d.unpack_frames, 100);
    }
}

//! Host DeployStyleAIUpdate residual (pack/unpack before move/attack).
//!
//! C++: units that must unpack to attack and pack before moving
//! (`DeployStyleAIUpdate::update` state machine).
//!
//! Timing and policy come from the exact parsed Object INI
//! `DeployStyleAIUpdate` module on each template.  No template-name list is
//! used to create deploy authority.
//!
//! States:
//! - ReadyToMove: undeployed, may path
//! - Deploying: unpacking timer → ReadyToAttack
//! - ReadyToAttack: deployed, may fire
//! - AligningTurrets: C++ `ALIGNING_TURRETS` — recenter then pack
//! - Undeploying: packing timer → ReadyToMove
//!
//! `TurretsMustCenterBeforePacking` waits for the host turret to reach its
//! authored natural yaw/pitch (C++ `isTurretInNaturalPosition`) instead of
//! inventing a pack delay.

use serde::{Deserialize, Serialize};

/// Logic FPS residual.
pub const DEPLOY_STYLE_LOGIC_FPS: f32 = 30.0;

/// C++ DeployStyleAIUpdate PerUnitSound slots (resolve before queue).
pub const DEPLOY_STYLE_DEPLOY_AUDIO: &str = "Deploy";
pub const DEPLOY_STYLE_UNDEPLOY_AUDIO: &str = "Undeploy";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostDeployStyleState {
    #[default]
    ReadyToMove,
    Deploying,
    ReadyToAttack,
    AligningTurrets,
    Undeploying,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostDeployStyleData {
    pub state: HostDeployStyleState,
    /// Frame when current pack/unpack completes (0 = inactive).
    /// While `AligningTurrets`, this is the frame the align state was entered.
    pub ready_frame: u32,
    pub pack_frames: u32,
    pub unpack_frames: u32,
    /// C++ `m_turretsMustCenterBeforePacking`.
    #[serde(default)]
    pub turrets_must_center_before_packing: bool,
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
            turrets_must_center_before_packing: metadata.turrets_must_center_before_packing,
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
            HostDeployStyleState::Deploying
                | HostDeployStyleState::Undeploying
                | HostDeployStyleState::AligningTurrets
        )
    }

    pub fn is_aligning_turrets(&self) -> bool {
        matches!(self.state, HostDeployStyleState::AligningTurrets)
    }

    /// C++ `setMyState(..., reverseDeploy)` leftover: `now + unpackTime - framesLeft`.
    fn reverse_ready_frame(&self, current_frame: u32) -> u32 {
        let frames_left = self.ready_frame.saturating_sub(current_frame);
        current_frame.saturating_add(self.unpack_frames.saturating_sub(frames_left))
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
                // C++ setMyState(DEPLOY, TRUE): leftover wait is mirrored
                // onto unpackTime, not a full restart.
                self.state = HostDeployStyleState::Deploying;
                self.ready_frame = self.reverse_ready_frame(current_frame);
                true
            }
            HostDeployStyleState::AligningTurrets => {
                // C++ ALIGNING_TURRETS + in-range/guard-idle → READY_TO_ATTACK.
                self.state = HostDeployStyleState::ReadyToAttack;
                self.ready_frame = 0;
                true
            }
            HostDeployStyleState::Deploying | HostDeployStyleState::ReadyToAttack => false,
        }
    }

    /// Begin pack when ordered to move while deployed/attacking.
    ///
    /// Convenience for units whose current weapon owns a turret that is not
    /// yet known to be natural. Leftover `getWhichTurretForCurWeapon` +
    /// `isTurretInNaturalPosition` belong on `begin_undeploy_with_weapon_turret`.
    pub fn begin_undeploy(&mut self, current_frame: u32) -> bool {
        self.begin_undeploy_with_weapon_turret(current_frame, true, false)
    }

    /// C++ READY_TO_ATTACK + move: turret + TurretsMustCenterBeforePacking
    /// → ALIGNING_TURRETS (recenter); otherwise UNDEPLOY.
    /// C++ ALIGNING_TURRETS + leftover `isTurretInNaturalPosition` → UNDEPLOY.
    pub fn begin_undeploy_with_weapon_turret(
        &mut self,
        current_frame: u32,
        has_weapon_turret: bool,
        turret_in_natural_position: bool,
    ) -> bool {
        match self.state {
            HostDeployStyleState::ReadyToAttack => {
                if has_weapon_turret && self.turrets_must_center_before_packing {
                    self.state = HostDeployStyleState::AligningTurrets;
                    self.ready_frame = current_frame;
                    return true;
                }
                self.enter_undeploying(current_frame);
                true
            }
            HostDeployStyleState::Deploying => {
                // C++ setMyState(UNDEPLOY, TRUE): leftover unpack wait
                // becomes remaining pack (totalFrames = unpackTime).
                self.state = HostDeployStyleState::Undeploying;
                self.ready_frame = self.reverse_ready_frame(current_frame);
                true
            }
            HostDeployStyleState::AligningTurrets => {
                self.finish_aligning_turrets(current_frame, turret_in_natural_position)
            }
            HostDeployStyleState::Undeploying | HostDeployStyleState::ReadyToMove => false,
        }
    }

    /// C++ ALIGNING_TURRETS + leftover `isTurretInNaturalPosition` → UNDEPLOY.
    /// Same-update enter stays ALIGNING (leftover match does not fall through).
    pub fn finish_aligning_turrets(
        &mut self,
        current_frame: u32,
        turret_in_natural_position: bool,
    ) -> bool {
        if self.state != HostDeployStyleState::AligningTurrets {
            return false;
        }
        if current_frame <= self.ready_frame || !turret_in_natural_position {
            return false;
        }
        self.enter_undeploying(current_frame);
        true
    }

    fn enter_undeploying(&mut self, current_frame: u32) {
        self.state = HostDeployStyleState::Undeploying;
        self.ready_frame = current_frame.saturating_add(self.pack_frames.max(1));
    }

    /// Advance timers; returns (became_ready_to_attack, became_ready_to_move).
    pub fn tick(&mut self, current_frame: u32) -> (bool, bool) {
        if matches!(self.state, HostDeployStyleState::AligningTurrets) {
            return (false, false);
        }
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

/// C++ `DeployStyleAIUpdate::setMyState` model-condition matrix.
///
/// DEPLOY: clear PACKING, set UNPACKING.
/// UNDEPLOY: clear UNPACKING|DEPLOYED, set PACKING.
/// READY_TO_ATTACK: clear UNPACKING, set DEPLOYED.
/// READY_TO_MOVE: clear PACKING.
pub fn leftover_stamp_deploy_style_conditions(bits: &mut u128, state: HostDeployStyleState) {
    use crate::game_logic::host_enum_table_residual::{
        deployed_model_bit, packing_model_bit, unpacking_model_bit,
    };
    let packing = 1u128 << packing_model_bit();
    let unpacking = 1u128 << unpacking_model_bit();
    let deployed = 1u128 << deployed_model_bit();
    *bits &= !(packing | unpacking | deployed);
    match state {
        HostDeployStyleState::ReadyToMove => {}
        HostDeployStyleState::Deploying => *bits |= unpacking,
        HostDeployStyleState::ReadyToAttack | HostDeployStyleState::AligningTurrets => {
            *bits |= deployed
        }
        HostDeployStyleState::Undeploying => *bits |= packing,
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
    let mut bits = 0u128;
    leftover_stamp_deploy_style_conditions(&mut bits, HostDeployStyleState::Deploying);
    let unpacking = 1u128 << crate::game_logic::host_enum_table_residual::unpacking_model_bit();
    deploy_style_ms_to_frames(1_000) == 30
        && deploy_style_ms_to_frames(3_333) == 100
        && deploy_style_ms_to_frames(34) == 2
        && (DEPLOY_STYLE_LOGIC_FPS - 30.0).abs() < f32::EPSILON
        && (bits & unpacking) != 0
}

/// C++ `TurretAI::isTurretInNaturalPosition` (under-construction is natural;
/// leftover eps is 0.0001 rad on current vs authored natural angles).
pub fn leftover_host_turret_is_in_natural_position(
    under_construction: bool,
    angle_deg: f32,
    pitch_deg: f32,
    natural_angle_deg: f32,
    natural_pitch_deg: f32,
) -> bool {
    if under_construction {
        return true;
    }
    (angle_deg.to_radians() - natural_angle_deg.to_radians()).abs() < 0.0001
        && (pitch_deg.to_radians() - natural_pitch_deg.to_radians()).abs() < 0.0001
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
        assert!(d.turrets_must_center_before_packing);
    }

    #[test]
    fn aligning_turrets_waits_for_natural_before_pack() {
        let metadata = crate::game_logic::DeployStyleMetadata {
            pack_time_frames: 30,
            unpack_time_frames: 30,
            turrets_must_center_before_packing: true,
            ..Default::default()
        };
        let mut d = HostDeployStyleData::from_metadata(&metadata);
        assert!(d.begin_deploy(0));
        let _ = d.tick(30);
        assert!(d.is_ready_to_attack());
        assert!(d.begin_undeploy(40));
        assert!(d.is_aligning_turrets());
        assert!(d.is_busy());
        assert!(
            !d.finish_aligning_turrets(40, true),
            "same-frame stay ALIGNING"
        );
        assert!(d.is_aligning_turrets());
        assert!(
            !d.finish_aligning_turrets(41, false),
            "leftover off-natural stays ALIGNING"
        );
        assert!(d.is_aligning_turrets());
        let (atk, mv) = d.tick(40);
        assert!(!atk && !mv);
        assert!(d.begin_undeploy_with_weapon_turret(41, true, true));
        assert!(!d.is_aligning_turrets());
        assert!(d.is_busy());
        let (atk, mv) = d.tick(71);
        assert!(!atk && mv);
        assert!(d.is_ready_to_move());
    }

    #[test]
    fn invalid_weapon_turret_packs_without_align() {
        let metadata = crate::game_logic::DeployStyleMetadata {
            pack_time_frames: 30,
            unpack_time_frames: 30,
            turrets_must_center_before_packing: true,
            ..Default::default()
        };
        let mut d = HostDeployStyleData::from_metadata(&metadata);
        assert!(d.begin_deploy(0));
        let _ = d.tick(30);
        assert!(d.begin_undeploy_with_weapon_turret(40, false, false));
        assert!(!d.is_aligning_turrets());
        assert!(d.is_busy());
        let (atk, mv) = d.tick(70);
        assert!(!atk && mv);
        assert!(d.is_ready_to_move());
    }

    #[test]
    fn leftover_host_turret_natural_matches_cpp_eps() {
        assert!(leftover_host_turret_is_in_natural_position(
            false, 0.0, 0.0, 0.0, 0.0
        ));
        assert!(leftover_host_turret_is_in_natural_position(
            true, 45.0, 10.0, 0.0, 0.0
        ));
        assert!(!leftover_host_turret_is_in_natural_position(
            false, 45.0, 0.0, 0.0, 0.0
        ));
    }
}

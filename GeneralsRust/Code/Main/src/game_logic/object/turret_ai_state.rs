//! C++ `TurretAI` pose / state-machine residual (hq-2qedt.2.3.1).
//!
//! Formerly inline on `Object`; extracted so the AltTurret slot can own a
//! second copy later (hq-2qedt.2.3.2). Primary-only for now: `Object` holds
//! exactly one `TurretAiState`. Field names and serde attributes are kept
//! verbatim so bincode saves stay byte-identical.

use serde::{Deserialize, Serialize};

use super::TurretSubState;
use crate::game_logic::ObjectId;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurretAiState {
    /// Host residual: TurretAI yaw (deg). Defaults **0**; Strategy Center
    /// authors NaturalTurretAngle **-90**.
    #[serde(default)]
    pub turret_angle_deg: f32,
    /// Host residual: TurretAI pitch (deg). Defaults **0**; Strategy Center
    /// authors NaturalTurretPitch **45**.
    #[serde(default)]
    pub turret_pitch_deg: f32,
    /// TurretAI idle-scan residual: absolute frame when next idle scan may start.
    /// 0 = not scheduled (or just completed without reschedule).
    #[serde(default)]
    pub turret_idle_scan_next_frame: u32,
    /// TurretAI idle-scan residual: true while rotating toward desired angle.
    #[serde(default)]
    pub turret_idle_scanning: bool,
    /// TurretAI idle-scan residual: desired absolute yaw while scanning.
    #[serde(default)]
    pub turret_idle_scan_desired_angle_deg: f32,
    /// TurretAI idle-scan residual: deterministic scan index (interval/offset seed).
    #[serde(default)]
    pub turret_idle_scan_index: u32,
    /// TurretAI HoldTurret residual: true while holding after idle-scan complete.
    #[serde(default)]
    pub turret_holding: bool,
    /// TurretAI HoldTurret residual: absolute frame when hold ends (0 = none).
    #[serde(default)]
    pub turret_hold_until_frame: u32,
    /// TurretAI idle-recenter residual: true while recentering after Hold (not pack).
    #[serde(default)]
    pub turret_idle_recentering: bool,
    /// TurretAI idle mood-target residual: target was set by friend_checkForIdleMoodTarget.
    /// Cleared when mood target leaves range / dies (C++ m_targetWasSetByIdleMood).
    #[serde(default)]
    pub turret_mood_target: bool,
    /// C++ TurretAI goal object residual.
    #[serde(default)]
    pub turret_target_id: Option<ObjectId>,
    /// C++ TurretAI m_target forceAttacking residual.
    #[serde(default)]
    pub turret_force_attacking: bool,
    /// C++ TurretAI enabled residual (false until unit has a turret slot).
    #[serde(default)]
    pub turret_enabled: bool,
    /// C++ TurretAIData::m_turnRate residual (radians per logic frame).
    #[serde(default = "super::default_turret_turn_rate")]
    pub turret_turn_rate_rad: f32,
    /// C++ TurretAI state machine residual.
    #[serde(default)]
    pub turret_substate: TurretSubState,
    /// C++ MODELCONDITION_TURRET_ROTATE residual.
    #[serde(default)]
    pub turret_rotating: bool,
    /// C++ TurretAIData NaturalTurretAngle residual (deg).
    #[serde(default)]
    pub turret_natural_angle_deg: f32,
    /// C++ TurretAIData NaturalTurretPitch residual (deg).
    #[serde(default)]
    pub turret_natural_pitch_deg: f32,
    /// C++ TurretAIData::m_recenterTime residual (logic frames).
    #[serde(default = "super::default_turret_recenter_frames")]
    pub turret_recenter_frames: u32,
}

impl TurretAiState {
    /// Seed a fresh TurretAI pose/SM from a spawn spec (primary or AltTurret).
    pub(crate) fn from_spawn(turret: &super::TurretSpawnSpec) -> Self {
        Self {
            turret_angle_deg: turret.angle_deg,
            turret_pitch_deg: turret.pitch_deg,
            turret_enabled: turret.enabled,
            turret_turn_rate_rad: turret.turn_rate_rad,
            turret_substate: TurretSubState::Idle,
            turret_natural_angle_deg: turret.natural_angle_deg,
            turret_natural_pitch_deg: turret.natural_pitch_deg,
            turret_recenter_frames: turret.recenter_frames,
            ..Self::default()
        }
    }
}

impl Default for TurretAiState {
    fn default() -> Self {
        Self {
            turret_angle_deg: Default::default(),
            turret_pitch_deg: Default::default(),
            turret_idle_scan_next_frame: Default::default(),
            turret_idle_scanning: Default::default(),
            turret_idle_scan_desired_angle_deg: Default::default(),
            turret_idle_scan_index: Default::default(),
            turret_holding: Default::default(),
            turret_hold_until_frame: Default::default(),
            turret_idle_recentering: Default::default(),
            turret_mood_target: Default::default(),
            turret_target_id: Default::default(),
            turret_force_attacking: Default::default(),
            turret_enabled: Default::default(),
            turret_turn_rate_rad: super::default_turret_turn_rate(),
            turret_substate: Default::default(),
            turret_rotating: Default::default(),
            turret_natural_angle_deg: Default::default(),
            turret_natural_pitch_deg: Default::default(),
            turret_recenter_frames: super::default_turret_recenter_frames(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::object::TurretSpawnSpec;

    /// hq-2qedt.2.3.2: AltTurret slot owns an independent TurretAiState
    /// seeded from its own spawn spec (C++ m_turretAI[TURRET_ALT]).
    #[test]
    fn alt_slot_owns_independent_turret_ai_state() {
        let mut primary = crate::game_logic::object::turret_spawn::turret_spawn_for_template("AmericaTankCrusader");
        primary.turn_rate_rad = 0.1;
        primary.natural_angle_deg = -90.0;
        let mut alt = primary.clone();
        alt.turn_rate_rad = 0.25;
        alt.natural_angle_deg = 30.0;
        alt.natural_pitch_deg = 10.0;

        let p = TurretAiState::from_spawn(&primary);
        let a = TurretAiState::from_spawn(&alt);
        assert_eq!(p.turret_turn_rate_rad, 0.1);
        assert_eq!(a.turret_turn_rate_rad, 0.25);
        assert_eq!(a.turret_natural_angle_deg, 30.0);
        assert_eq!(a.turret_natural_pitch_deg, 10.0);
        assert_eq!(a.turret_substate, TurretSubState::Idle);
        assert_ne!(p, a, "alt pose/SM memory is independent of primary");
    }
}

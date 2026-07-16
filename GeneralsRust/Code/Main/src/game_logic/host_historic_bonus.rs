//! Host residual for C++ WeaponTemplate HistoricBonus multi-hit firestorm.
//!
//! When `HistoricBonusCount` impacts of the same weapon template land within
//! `HistoricBonusTime` frames and `HistoricBonusRadius`, fire the bonus weapon
//! (typically FirestormSmallCreationWeapon → OCL firestorm DoT residual).
//!
//! Fail-closed: not full WeaponStore::createAndFireTempWeapon OCL matrix;
//! firestorm DoT reuses HostHelixFirestormZone via GameLogic drain.

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

/// One historic damage sample (C++ HistoricWeaponDamageInfo).
#[derive(Debug, Clone, Copy)]
struct HistoricSample {
    frame: u32,
    pos: Vec3,
}

/// Pending firestorm spawn from a historic bonus trigger.
#[derive(Debug, Clone)]
pub struct PendingHistoricFirestorm {
    pub source_id: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub black_napalm: bool,
    pub bonus_weapon: String,
    pub trigger_frame: u32,
}

#[derive(Debug, Default)]
struct HistoricState {
    /// Per-weapon-template damage samples.
    samples: HashMap<String, Vec<HistoricSample>>,
    /// Logic frame advanced by combat/GameLogic.
    frame: u32,
    pending: Vec<PendingHistoricFirestorm>,
    /// Honesty counters.
    impacts_recorded: u32,
    bonuses_triggered: u32,
}

static STATE: Mutex<Option<HistoricState>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut HistoricState) -> R) -> R {
    let mut guard = STATE.lock().expect("historic bonus lock");
    if guard.is_none() {
        *guard = Some(HistoricState::default());
    }
    f(guard.as_mut().unwrap())
}

/// Advance residual frame (call once per logic tick).
pub fn set_logic_frame(frame: u32) {
    with_state(|s| s.frame = frame);
}

pub fn logic_frame() -> u32 {
    with_state(|s| s.frame)
}

/// Record an impact and maybe trigger HistoricBonus weapon.
///
/// Returns true if a bonus firestorm was queued this call.
pub fn record_impact(
    weapon_key: &str,
    peel: &crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel,
    pos: Vec3,
    source_id: ObjectId,
    source_team: super::Team,
) -> bool {
    if !peel.is_active() || weapon_key.is_empty() {
        return false;
    }
    with_state(|s| {
        s.impacts_recorded = s.impacts_recorded.saturating_add(1);
        let frame = s.frame;
        let rad = peel.radius;
        let rad_sqr = rad * rad;
        let oldest = frame.saturating_sub(peel.time_frames);

        // Trim + count without holding entry borrow across other s fields.
        let mut list = s.samples.remove(weapon_key).unwrap_or_default();
        list.retain(|h| h.frame >= oldest);

        let mut count = 0i32;
        for h in list.iter() {
            let dx = h.pos.x - pos.x;
            let dz = h.pos.z - pos.z;
            if dx * dx + dz * dz <= rad_sqr {
                count += 1;
            }
        }

        // C++: count >= historicBonusCount - 1 (self included implicitly)
        if count >= peel.count - 1 {
            s.pending.push(PendingHistoricFirestorm {
                source_id,
                source_team,
                position: pos,
                black_napalm: peel.is_black_napalm_bonus(),
                bonus_weapon: peel.bonus_weapon.clone(),
                trigger_frame: frame,
            });
            s.bonuses_triggered = s.bonuses_triggered.saturating_add(1);
            // C++ E3 plug: clear list on success (do not reinsert).
            true
        } else {
            list.push(HistoricSample { frame, pos });
            s.samples.insert(weapon_key.to_string(), list);
            false
        }
    })
}

/// Drain pending historic firestorm spawns.
pub fn drain_pending_firestorms() -> Vec<PendingHistoricFirestorm> {
    with_state(|s| std::mem::take(&mut s.pending))
}

pub fn honesty_impacts() -> u32 {
    with_state(|s| s.impacts_recorded)
}

pub fn honesty_bonuses() -> u32 {
    with_state(|s| s.bonuses_triggered)
}

/// Test/reset helper.
pub fn reset_for_tests() {
    with_state(|s| *s = HistoricState::default());
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostHistoricBonusHonesty {
    pub impacts_recorded: u32,
    pub bonuses_triggered: u32,
}

pub fn honesty_snapshot() -> HostHistoricBonusHonesty {
    HostHistoricBonusHonesty {
        impacts_recorded: honesty_impacts(),
        bonuses_triggered: honesty_bonuses(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel;
    use crate::game_logic::Team;

    #[test]
    fn historic_bonus_triggers_on_third_close_impact() {
        reset_for_tests();
        set_logic_frame(100);
        let peel = HostHistoricBonusPeel {
            time_frames: 90,
            count: 3,
            radius: 20.0,
            bonus_weapon: "FirestormSmallCreationWeapon".into(),
        };
        let key = "InfernoCannonGun";
        let pos = Vec3::ZERO;
        assert!(!record_impact(key, &peel, pos, ObjectId(1), Team::China));
        assert!(!record_impact(key, &peel, pos, ObjectId(1), Team::China));
        assert!(record_impact(key, &peel, pos, ObjectId(1), Team::China));
        let pending = drain_pending_firestorms();
        assert_eq!(pending.len(), 1);
        assert!(!pending[0].black_napalm);
        assert_eq!(honesty_bonuses(), 1);
    }

    #[test]
    fn historic_bonus_ignores_far_impacts() {
        reset_for_tests();
        set_logic_frame(50);
        let peel = HostHistoricBonusPeel {
            time_frames: 90,
            count: 3,
            radius: 20.0,
            bonus_weapon: "FirestormSmallCreationWeapon".into(),
        };
        assert!(!record_impact(
            "InfernoCannonGun",
            &peel,
            Vec3::ZERO,
            ObjectId(1),
            Team::China
        ));
        assert!(!record_impact(
            "InfernoCannonGun",
            &peel,
            Vec3::new(100.0, 0.0, 0.0),
            ObjectId(1),
            Team::China
        ));
        assert!(!record_impact(
            "InfernoCannonGun",
            &peel,
            Vec3::new(200.0, 0.0, 0.0),
            ObjectId(1),
            Team::China
        ));
        assert!(drain_pending_firestorms().is_empty());
    }
}

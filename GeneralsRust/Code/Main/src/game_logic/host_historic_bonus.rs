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
use std::collections::{HashMap, VecDeque};
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

/// C++ WeaponTemplate history is shared by all users of that template in one
/// game. The host's CombatSystem owns it; immutable authored rules stay outside.
#[derive(Debug, Default)]
pub(crate) struct HostHistoricBonus {
    samples: HashMap<String, VecDeque<HistoricSample>>,
    pending: Vec<PendingHistoricFirestorm>,
    impacts_recorded: u32,
    bonuses_triggered: u32,
}

// Temporary compatibility clock for callers outside the owned impact path.
// hq-vrkh8 tracks migration of those consumers to their driving game/frame.
// This clock owns no history, pending firestorms, or counters.
static LOGIC_FRAME: Mutex<u32> = Mutex::new(0);

pub fn set_logic_frame(frame: u32) {
    *LOGIC_FRAME.lock().expect("logic frame lock") = frame;
}

pub fn logic_frame() -> u32 {
    *LOGIC_FRAME.lock().expect("logic frame lock")
}

impl HostHistoricBonus {
    /// C++ Weapon.cpp:1169-1251: trim chronological history, count previous
    /// same-template impacts, then dispatch/clear or append the current hit.
    /// Frame and immutable retention rules come from the driving operation.
    pub(crate) fn record_impact(
        &mut self,
        frame: u32,
        historic_damage_limit: u32,
        weapon_key: &str,
        peel: &crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel,
        pos: Vec3,
        source_id: ObjectId,
        source_team: super::Team,
    ) -> bool {
        if !peel.is_active() || weapon_key.is_empty() {
            return false;
        }
        self.impacts_recorded = self.impacts_recorded.saturating_add(1);
        let list = self.samples.entry(weapon_key.to_string()).or_default();
        // C++ UnsignedInt subtraction wraps, including early logic frames.
        let expiration = frame.wrapping_sub(historic_damage_limit);
        while list
            .front()
            .is_some_and(|sample| sample.frame <= expiration)
        {
            list.pop_front();
        }
        let oldest = frame.wrapping_sub(peel.time_frames);
        let rad_sqr = peel.radius * peel.radius;
        let count = list
            .iter()
            .filter(|sample| {
                let dx = sample.pos.x - pos.x;
                let dz = sample.pos.z - pos.z;
                sample.frame >= oldest && dx * dx + dz * dz <= rad_sqr
            })
            .count() as i32;

        // C++ includes the current impact implicitly; it is not appended
        // before checking the threshold. Host bonus dispatch remains the
        // existing deferred FirestormSmall/OCL residual.
        if count >= peel.count - 1 {
            self.pending.push(PendingHistoricFirestorm {
                source_id,
                source_team,
                position: pos,
                black_napalm: peel.is_black_napalm_bonus(),
                bonus_weapon: peel.bonus_weapon.clone(),
                trigger_frame: frame,
            });
            self.bonuses_triggered = self.bonuses_triggered.saturating_add(1);
            list.clear();
            true
        } else {
            list.push_back(HistoricSample { frame, pos });
            false
        }
    }

    pub(crate) fn drain_pending_firestorms(&mut self) -> Vec<PendingHistoricFirestorm> {
        std::mem::take(&mut self.pending)
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn honesty_snapshot(&self) -> HostHistoricBonusHonesty {
        HostHistoricBonusHonesty {
            impacts_recorded: self.impacts_recorded,
            bonuses_triggered: self.bonuses_triggered,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostHistoricBonusHonesty {
    pub impacts_recorded: u32,
    pub bonuses_triggered: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;
    use crate::game_logic::weapon_bootstrap::HostHistoricBonusPeel;

    #[test]
    fn historic_bonus_triggers_on_third_close_impact() {
        let mut history = HostHistoricBonus::default();
        let peel = HostHistoricBonusPeel {
            time_frames: 90,
            count: 3,
            radius: 20.0,
            bonus_weapon: "FirestormSmallCreationWeapon".into(),
        };
        let key = "InfernoCannonGun";
        let pos = Vec3::ZERO;
        assert!(!history.record_impact(100, 90, key, &peel, pos, ObjectId(1), Team::China));
        assert!(!history.record_impact(100, 90, key, &peel, pos, ObjectId(1), Team::China));
        assert!(history.record_impact(100, 90, key, &peel, pos, ObjectId(1), Team::China));
        let pending = history.drain_pending_firestorms();
        assert_eq!(pending.len(), 1);
        assert!(!pending[0].black_napalm);
        assert_eq!(history.honesty_snapshot().bonuses_triggered, 1);
    }

    #[test]
    fn historic_bonus_ignores_far_impacts() {
        let mut history = HostHistoricBonus::default();
        let peel = HostHistoricBonusPeel {
            time_frames: 90,
            count: 3,
            radius: 20.0,
            bonus_weapon: "FirestormSmallCreationWeapon".into(),
        };
        assert!(!history.record_impact(
            100,
            90,
            "InfernoCannonGun",
            &peel,
            Vec3::ZERO,
            ObjectId(1),
            Team::China
        ));
        assert!(!history.record_impact(
            100,
            90,
            "InfernoCannonGun",
            &peel,
            Vec3::new(100.0, 0.0, 0.0),
            ObjectId(1),
            Team::China
        ));
        assert!(!history.record_impact(
            100,
            90,
            "InfernoCannonGun",
            &peel,
            Vec3::new(200.0, 0.0, 0.0),
            ObjectId(1),
            Team::China
        ));
        assert!(history.drain_pending_firestorms().is_empty());
    }
    #[test]
    fn historic_bonus_unsigned_thresholds_match_cpp() {
        // Weapon.cpp:1171 expirationDate and :1221 oldestThatWillCount
        // subtract UnsignedInt. Expiry is <=; qualifying time is >=.
        let cases = [
            (100, 108, 8, 8, false),
            (1, 1, 8, 1, false),
            (1, 1, 1, 8, false),
            (u32::MAX - 1, 1, 8, 8, true),
            (100, 180, 90, 80, true),
            (100, 181, 90, 80, false),
        ];
        for (first_frame, next_frame, limit, time_frames, expected) in cases {
            let mut history = HostHistoricBonus::default();
            let peel = HostHistoricBonusPeel {
                time_frames,
                count: 2,
                radius: 20.0,
                bonus_weapon: "FirestormSmallCreationWeapon".into(),
            };
            assert!(!history.record_impact(
                first_frame,
                limit,
                "WrapGun",
                &peel,
                Vec3::ZERO,
                ObjectId(1),
                Team::China,
            ));
            assert_eq!(
                history.record_impact(
                    next_frame,
                    limit,
                    "WrapGun",
                    &peel,
                    Vec3::ZERO,
                    ObjectId(2),
                    Team::China,
                ),
                expected,
                "first={first_frame}, next={next_frame}, limit={limit}, window={time_frames}",
            );
        }
    }
}

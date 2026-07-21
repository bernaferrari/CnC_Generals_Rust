//! Pure residual auto-fire target acquisition (query phase).
//!
//! Host residual scanners (base defense / sentry / hellfire / …) choose targets
//! via these helpers so fire *decision* is a pure function of snapshot inputs.
//! Apply phase (fire-spawn / hitscan / AI decision log) stays on the caller.
//!
//! Fail-closed: not full PartitionManager / AcquirePlayerTargets / stealth
//! detector matrix — residual legality hooks stay caller-supplied.

use crate::game_logic::{ObjectId, Team};
use glam::Vec3;

/// Snapshot fields required for residual nearest-in-range acquire.
#[derive(Debug, Clone, Copy)]
pub struct ResidualAcquireCandidate {
    pub id: ObjectId,
    pub team: Team,
    pub position: Vec3,
    pub is_alive: bool,
    pub is_neutral: bool,
    pub under_construction: bool,
    /// Attackable / structure / infantry / vehicle / aircraft residual union.
    pub combat_kind: bool,
    pub effectively_stealthed: bool,
    pub is_air: bool,
}

/// Nearest legal candidate in range. `range_for_air` returns max engagement range
/// for air vs ground (dual-slot defenses use different ranges).
pub fn pick_nearest_residual_target(
    self_id: ObjectId,
    self_team: Team,
    fire_pos: Vec3,
    candidates: impl IntoIterator<Item = ResidualAcquireCandidate>,
    mut range_for_air: impl FnMut(bool) -> f32,
    mut is_legal: impl FnMut(&ResidualAcquireCandidate) -> bool,
) -> Option<(ObjectId, f32, bool)> {
    let mut best: Option<(ObjectId, f32, bool)> = None;
    for c in candidates {
        if c.id == self_id {
            continue;
        }
        if !is_legal(&c) {
            continue;
        }
        // Stealthed + undetected residual: skip enemy stealth (parity with update_combat).
        if c.effectively_stealthed && c.team != self_team {
            continue;
        }
        let range = range_for_air(c.is_air);
        if range <= 0.0 {
            continue;
        }
        let dist = fire_pos.distance(c.position);
        if dist <= range && best.map(|(_, d, _)| dist < d).unwrap_or(true) {
            best = Some((c.id, dist, c.is_air));
        }
    }
    best
}

/// Standard combat-kind residual union used by base-defense / drone residual scanners.
pub fn residual_combat_kind(
    attackable: bool,
    structure: bool,
    infantry: bool,
    vehicle: bool,
    aircraft: bool,
) -> bool {
    attackable || structure || infantry || vehicle || aircraft
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::ObjectId;

    fn cand(id: u32, team: Team, pos: Vec3, air: bool) -> ResidualAcquireCandidate {
        ResidualAcquireCandidate {
            id: ObjectId(id),
            team,
            position: pos,
            is_alive: true,
            is_neutral: false,
            under_construction: false,
            combat_kind: true,
            effectively_stealthed: false,
            is_air: air,
        }
    }

    #[test]
    fn picks_nearest_enemy_in_range() {
        let self_id = ObjectId(1);
        let fire = Vec3::ZERO;
        let list = [
            cand(2, Team::GLA, Vec3::new(50.0, 0.0, 0.0), false),
            cand(3, Team::GLA, Vec3::new(20.0, 0.0, 0.0), false),
            cand(4, Team::GLA, Vec3::new(80.0, 0.0, 0.0), false),
        ];
        let best = pick_nearest_residual_target(
            self_id,
            Team::USA,
            fire,
            list,
            |_| 60.0,
            |c| c.is_alive && c.team != Team::USA && c.combat_kind,
        );
        assert_eq!(best.map(|(id, _, _)| id), Some(ObjectId(3)));
    }

    #[test]
    fn skips_out_of_range_and_friendly() {
        let best = pick_nearest_residual_target(
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            [
                cand(2, Team::USA, Vec3::new(10.0, 0.0, 0.0), false),
                cand(3, Team::GLA, Vec3::new(100.0, 0.0, 0.0), false),
            ],
            |_| 50.0,
            |c| c.is_alive && c.team != Team::USA,
        );
        assert!(best.is_none());
    }

    #[test]
    fn dual_slot_air_uses_air_range() {
        let best = pick_nearest_residual_target(
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            [cand(9, Team::GLA, Vec3::new(70.0, 0.0, 0.0), true)],
            |air| if air { 100.0 } else { 40.0 },
            |c| c.team != Team::USA,
        );
        assert_eq!(
            best.map(|(id, _, air)| (id, air)),
            Some((ObjectId(9), true))
        );
    }
}

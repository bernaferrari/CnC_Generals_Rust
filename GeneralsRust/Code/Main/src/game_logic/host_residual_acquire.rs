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
    /// C++ InvulnerableTime eject residual (enemies treat as allies).
    pub eject_invulnerable: bool,
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

/// Nearest legal service target (heal pad / repair pad / unfinished structure).
/// Unlike combat acquire, does **not** auto-skip stealthed candidates — callers
/// encode pad/structure legality only.
/// Nearest residual target by XZ distance from an impact/origin point.
/// Used by splash / impact residual (gattling) where height is ignored.
/// `exclude` skips the firing source when present. No automatic enemy-team filter —
/// legality is entirely `is_legal`.
/// Candidate for priority-band residual acquire (PDL intercept, etc.).
#[derive(Debug, Clone)]
pub struct PriorityAcquireCandidate {
    pub id: ObjectId,
    pub position: Vec3,
    pub is_alive: bool,
    /// Lower is better. `None` = ineligible.
    pub priority: Option<u8>,
}

/// Best residual target by priority band (lower better), XZ range gate, 3D distance tiebreak.
/// Mirrors C++ PointDefenseLaserUpdate-style selection.
pub fn pick_best_priority_residual_target(
    exclude: ObjectId,
    origin: Vec3,
    origin_xz: (f32, f32),
    max_range_2d: f32,
    candidates: impl IntoIterator<Item = PriorityAcquireCandidate>,
) -> Option<(ObjectId, u8, f32)> {
    let mut best: Option<(ObjectId, u8, f32)> = None;
    let range_sq = max_range_2d * max_range_2d;
    for c in candidates {
        if c.id == exclude || !c.is_alive {
            continue;
        }
        let Some(prio) = c.priority else {
            continue;
        };
        let dx = origin_xz.0 - c.position.x;
        let dz = origin_xz.1 - c.position.z;
        if dx * dx + dz * dz > range_sq {
            continue;
        }
        let dist = origin.distance(c.position);
        let better = match best {
            None => true,
            Some((_, bp, bd)) => prio < bp || (prio == bp && dist < bd),
        };
        if better {
            best = Some((c.id, prio, dist));
        }
    }
    best
}

pub fn pick_nearest_residual_target_xz(
    exclude: Option<ObjectId>,
    origin_xz: (f32, f32),
    candidates: impl IntoIterator<Item = ResidualAcquireCandidate>,
    max_range: f32,
    is_legal: impl Fn(&ResidualAcquireCandidate) -> bool,
) -> Option<(ObjectId, f32, bool)> {
    let mut best: Option<(ObjectId, f32, bool)> = None;
    for c in candidates {
        if exclude == Some(c.id) {
            continue;
        }
        if !c.is_alive || !is_legal(&c) {
            continue;
        }
        let dx = origin_xz.0 - c.position.x;
        let dz = origin_xz.1 - c.position.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist > max_range {
            continue;
        }
        if best.map(|(_, d, _)| dist < d).unwrap_or(true) {
            best = Some((c.id, dist, c.is_air));
        }
    }
    best
}

pub fn pick_nearest_residual_service_target(
    self_id: ObjectId,
    origin: Vec3,
    candidates: impl IntoIterator<Item = ResidualAcquireCandidate>,
    max_range: f32,
    mut is_legal: impl FnMut(&ResidualAcquireCandidate) -> bool,
) -> Option<(ObjectId, f32, Vec3)> {
    if max_range <= 0.0 {
        return None;
    }
    let mut best: Option<(ObjectId, f32, Vec3)> = None;
    for c in candidates {
        if c.id == self_id {
            continue;
        }
        if !is_legal(&c) {
            continue;
        }
        let dist = origin.distance(c.position);
        if dist <= max_range && best.map(|(_, d, _)| dist < d).unwrap_or(true) {
            best = Some((c.id, dist, c.position));
        }
    }
    best
}

/// PilotFindVehicle residual candidate (recrew unmanned vehicle).
#[derive(Debug, Clone, Copy)]
pub struct PilotVehicleCandidate {
    pub id: ObjectId,
    pub position: Vec3,
    pub recrewable: bool,
    pub health_ok: bool,
    pub same_player_ok: bool,
    pub collide_ok: bool,
}

/// Nearest pilot recrew target plus PartitionFilterPlayer / CollideModule reject counts.
///
/// Reject counters match host residual honesty: counted only when recrewable +
/// MinHealth + in-range hold, but player or collide gate fails.
pub fn pick_nearest_pilot_vehicle_target(
    self_id: ObjectId,
    origin: Vec3,
    candidates: impl IntoIterator<Item = PilotVehicleCandidate>,
    max_range: f32,
) -> (Option<(ObjectId, f32, Vec3)>, u32, u32) {
    let mut best: Option<(ObjectId, f32, Vec3)> = None;
    let mut player_rejects = 0u32;
    let mut collide_rejects = 0u32;
    if max_range <= 0.0 {
        return (None, 0, 0);
    }
    for c in candidates {
        if c.id == self_id {
            continue;
        }
        let dist = ((origin.x - c.position.x).powi(2) + (origin.z - c.position.z).powi(2)).sqrt();
        let in_range = dist <= max_range;
        if c.recrewable && c.health_ok && in_range && !c.same_player_ok {
            player_rejects = player_rejects.saturating_add(1);
            continue;
        }
        if c.recrewable && c.health_ok && in_range && c.same_player_ok && !c.collide_ok {
            collide_rejects = collide_rejects.saturating_add(1);
            continue;
        }
        // Final gate: recrewable + health + range + same player + collide.
        if !(c.same_player_ok && c.recrewable && c.health_ok && in_range && c.collide_ok) {
            continue;
        }
        if best.map(|(_, d, _)| dist < d).unwrap_or(true) {
            best = Some((c.id, dist, c.position));
        }
    }
    (best, player_rejects, collide_rejects)
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
            eject_invulnerable: false,
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

    #[test]
    fn service_target_picks_nearest_pad() {
        let origin = Vec3::ZERO;
        let list = [
            ResidualAcquireCandidate {
                id: ObjectId(2),
                team: Team::USA,
                position: Vec3::new(100.0, 0.0, 0.0),
                is_alive: true,
                is_neutral: false,
                under_construction: false,
                combat_kind: false,
                effectively_stealthed: false,
                is_air: false,
                eject_invulnerable: false,
            },
            ResidualAcquireCandidate {
                id: ObjectId(3),
                team: Team::USA,
                position: Vec3::new(30.0, 0.0, 0.0),
                is_alive: true,
                is_neutral: false,
                under_construction: false,
                combat_kind: false,
                effectively_stealthed: false,
                is_air: false,
                eject_invulnerable: false,
            },
        ];
        let best = pick_nearest_residual_service_target(ObjectId(1), origin, list, 200.0, |c| {
            c.is_alive && !c.under_construction
        });
        assert_eq!(best.map(|(id, _, _)| id), Some(ObjectId(3)));
    }

    #[test]
    fn pilot_vehicle_picks_nearest_and_counts_rejects() {
        let origin = Vec3::ZERO;
        let list = [
            PilotVehicleCandidate {
                id: ObjectId(2),
                position: Vec3::new(40.0, 0.0, 0.0),
                recrewable: true,
                health_ok: true,
                same_player_ok: false,
                collide_ok: true,
            },
            PilotVehicleCandidate {
                id: ObjectId(3),
                position: Vec3::new(20.0, 0.0, 0.0),
                recrewable: true,
                health_ok: true,
                same_player_ok: true,
                collide_ok: true,
            },
            PilotVehicleCandidate {
                id: ObjectId(4),
                position: Vec3::new(30.0, 0.0, 0.0),
                recrewable: true,
                health_ok: true,
                same_player_ok: true,
                collide_ok: false,
            },
        ];
        let (best, player_rej, collide_rej) =
            pick_nearest_pilot_vehicle_target(ObjectId(1), origin, list, 100.0);
        assert_eq!(best.map(|(id, _, _)| id), Some(ObjectId(3)));
        assert_eq!(player_rej, 1);
        assert_eq!(collide_rej, 1);
    }

    #[test]
    fn xz_picks_nearest_in_radius() {
        let origin = (0.0_f32, 0.0_f32);
        let cands = [
            cand(1, Team::GLA, Vec3::new(10.0, 50.0, 0.0), false), // 3D far Y ignored
            cand(2, Team::GLA, Vec3::new(5.0, 0.0, 0.0), false),
            cand(3, Team::GLA, Vec3::new(20.0, 0.0, 0.0), false), // out of 12
        ];
        let pick = pick_nearest_residual_target_xz(Some(ObjectId(99)), origin, cands, 12.0, |c| {
            c.combat_kind
        });
        assert_eq!(pick.map(|(id, _, _)| id), Some(ObjectId(2)));
    }

    #[test]
    fn priority_picks_primary_over_secondary() {
        let origin = Vec3::ZERO;
        let cands = [
            PriorityAcquireCandidate {
                id: ObjectId(1),
                position: Vec3::new(5.0, 0.0, 0.0),
                is_alive: true,
                priority: Some(1), // secondary closer
            },
            PriorityAcquireCandidate {
                id: ObjectId(2),
                position: Vec3::new(8.0, 0.0, 0.0),
                is_alive: true,
                priority: Some(0), // primary farther
            },
            PriorityAcquireCandidate {
                id: ObjectId(3),
                position: Vec3::new(50.0, 0.0, 0.0),
                is_alive: true,
                priority: Some(0), // primary out of range
            },
        ];
        let pick =
            pick_best_priority_residual_target(ObjectId(99), origin, (0.0, 0.0), 12.0, cands);
        assert_eq!(pick.map(|(id, p, _)| (id, p)), Some((ObjectId(2), 0)));
    }
}

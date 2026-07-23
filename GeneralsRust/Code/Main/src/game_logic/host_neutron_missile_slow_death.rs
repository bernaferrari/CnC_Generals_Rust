//! Host NeutronMissileSlowDeathBehavior residual (multi-blast nuke death waves).
//!
//! C++: extends SlowDeath; on activation plays FX_Nuke, then scheduled blasts:
//! topple/push rings (1–5), damage blast 6 (3500/300 falloff), scorch rings
//! set MODELCONDITION_BURNED, DestructionDelay 3501ms, OCL radiation midpoint.
//!
//! Residual playability slice:
//! - Retail blast schedule (ms → frames @ 30 FPS)
//! - Radial damage falloff for Blast6
//! - Scorch waves mark BURNED + scorch size residual
//! - Topple impulse requests for trees/props
//!
//! Fail-closed: not full partition iterators, FlammableUpdate ignite, shrubbery
//! shadow disable, or SlowDeath base sink phases on the missile drawable.

use serde::{Deserialize, Serialize};

pub const NEUTRON_LOGIC_FPS: f32 = 30.0;
pub const NEUTRON_SCORCH_MARK_SIZE: f32 = 320.0;
pub const NEUTRON_DESTRUCTION_DELAY_MS: u32 = 3501;
pub const NEUTRON_FX_LIST: &str = "FX_Nuke";
pub const NEUTRON_RADIATION_OCL: &str = "OCL_NukeRadiationField";
/// C++ MODELCONDITION_BURNED bit residual index.
pub const MC_BIT_BURNED: u32 = 63;
/// C++ MODELCONDITION_FRONTCRUSHED / BACKCRUSHED residual indices.
pub const MC_BIT_FRONTCRUSHED: u32 = 1;
pub const MC_BIT_BACKCRUSHED: u32 = 2;

#[inline]
pub fn ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * NEUTRON_LOGIC_FPS / 1000.0).round().max(0.0) as u32
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NeutronBlastInfo {
    pub enabled: bool,
    pub delay_frames: u32,
    pub scorch_delay_frames: u32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub max_damage: f32,
    pub min_damage: f32,
    pub topple_speed: f32,
    pub push_force: f32,
}

impl NeutronBlastInfo {
    pub const fn empty() -> Self {
        Self {
            enabled: false,
            delay_frames: 0,
            scorch_delay_frames: 0,
            inner_radius: 0.0,
            outer_radius: 0.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.0,
            push_force: 0.0,
        }
    }
}

/// Retail Superweapon NeutronMissile / NuclearMissile SlowDeath blast table.
pub fn retail_neutron_blasts() -> [NeutronBlastInfo; 9] {
    [
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(580),
            scorch_delay_frames: ms_to_frames(100),
            inner_radius: 60.0,
            outer_radius: 60.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.5,
            push_force: 10.0,
        },
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(660),
            scorch_delay_frames: ms_to_frames(180),
            inner_radius: 90.0,
            outer_radius: 90.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.45,
            push_force: 8.0,
        },
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(720),
            scorch_delay_frames: ms_to_frames(260),
            inner_radius: 120.0,
            outer_radius: 120.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.42,
            push_force: 6.0,
        },
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(850),
            scorch_delay_frames: ms_to_frames(340),
            inner_radius: 150.0,
            outer_radius: 150.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.40,
            push_force: 6.0,
        },
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(1000),
            scorch_delay_frames: ms_to_frames(420),
            inner_radius: 180.0,
            outer_radius: 180.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.38,
            push_force: 6.0,
        },
        // Blast6: the real damage wave.
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(1180),
            scorch_delay_frames: ms_to_frames(500),
            inner_radius: 60.0,
            outer_radius: 210.0,
            max_damage: 3500.0,
            min_damage: 300.0,
            topple_speed: 0.35,
            push_force: 4.0,
        },
        // Scorch-only rings (delay 999999 = skip damage wave).
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(999_999),
            scorch_delay_frames: ms_to_frames(620),
            inner_radius: 0.0,
            outer_radius: 210.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.0,
            push_force: 0.0,
        },
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(999_999),
            scorch_delay_frames: ms_to_frames(700),
            inner_radius: 0.0,
            outer_radius: 210.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.0,
            push_force: 0.0,
        },
        NeutronBlastInfo {
            enabled: true,
            delay_frames: ms_to_frames(999_999),
            scorch_delay_frames: ms_to_frames(800),
            inner_radius: 0.0,
            outer_radius: 210.0,
            max_damage: 0.0,
            min_damage: 0.0,
            topple_speed: 0.0,
            push_force: 0.0,
        },
    ]
}

/// C++ damage falloff residual for one target at 2D distance.
pub fn blast_damage_at_distance(info: &NeutronBlastInfo, dist: f32) -> f32 {
    if !info.enabled || info.outer_radius <= 0.0 {
        return 0.0;
    }
    if info.max_damage <= 0.0 && info.min_damage <= 0.0 {
        return 0.0;
    }
    if dist > info.outer_radius {
        return 0.0;
    }
    if dist <= info.inner_radius {
        return info.max_damage.max(info.min_damage);
    }
    let span = (info.outer_radius - info.inner_radius) + 0.01;
    let percent = 1.0 - ((dist - info.inner_radius) / span);
    let amount = info.max_damage * percent;
    amount.max(info.min_damage)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostNeutronMissileSlowDeathData {
    pub activation_frame: u32,
    /// False until `begin` is called (allows activation_frame == 0).
    pub activated: bool,
    pub completed_blasts: [bool; 9],
    pub completed_scorch: [bool; 9],
    pub scorch_placed: bool,
    pub fx_played: bool,
    pub done: bool,
    pub total_damage_applied: f32,
    pub damage_hits: u32,
    pub scorch_waves: u32,
    pub topple_requests: u32,
}

impl Default for HostNeutronMissileSlowDeathData {
    fn default() -> Self {
        Self {
            activation_frame: 0, // inactive until begin()
            activated: false,
            completed_blasts: [false; 9],
            completed_scorch: [false; 9],
            scorch_placed: false,
            fx_played: false,
            done: false,
            total_damage_applied: 0.0,
            damage_hits: 0,
            scorch_waves: 0,
            topple_requests: 0,
        }
    }
}

impl HostNeutronMissileSlowDeathData {
    pub fn begin(activation_frame: u32) -> Self {
        let mut s = Self::default();
        s.activation_frame = activation_frame;
        s.activated = true;
        s.fx_played = true; // FX_Nuke at ground residual — caller emits presentation
        s
    }

    pub fn is_active(&self) -> bool {
        self.activated && !self.done
    }

    pub fn destruction_frame(&self) -> u32 {
        self.activation_frame
            .saturating_add(ms_to_frames(NEUTRON_DESTRUCTION_DELAY_MS))
    }
}

/// One damage application planned for this frame.
#[derive(Debug, Clone)]
pub struct NeutronDamageHit {
    pub target_index: usize,
    pub damage: f32,
    pub topple_dx: f32,
    pub topple_dz: f32,
    pub topple_speed: f32,
    pub push_force: f32,
    pub set_burned: bool,
}

/// Plan residual effects for one logic frame.
/// `epicenter` is (x, z); `object_xz` positions match `hits.target_index`.
pub fn plan_neutron_frame(
    state: &mut HostNeutronMissileSlowDeathData,
    current_frame: u32,
    epicenter: (f32, f32),
    object_xz: &[(f32, f32)],
) -> (
    Vec<NeutronDamageHit>,
    bool, /*place_scorch*/
    bool, /*sequence_done*/
) {
    let mut hits = Vec::new();
    let mut place_scorch = false;
    if !state.is_active() {
        return (hits, false, true);
    }
    let elapsed = current_frame.saturating_sub(state.activation_frame);
    let blasts = retail_neutron_blasts();

    // First scorch mark size residual once.
    if !state.scorch_placed && elapsed >= ms_to_frames(100) {
        state.scorch_placed = true;
        place_scorch = true;
    }

    for (i, info) in blasts.iter().enumerate() {
        if !info.enabled {
            continue;
        }
        // Damage / topple wave
        if !state.completed_blasts[i]
            && elapsed >= info.delay_frames
            && info.delay_frames < ms_to_frames(900_000)
        {
            state.completed_blasts[i] = true;
            for (idx, &(ox, oz)) in object_xz.iter().enumerate() {
                let dx = ox - epicenter.0;
                let dz = oz - epicenter.1;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist > info.outer_radius && info.outer_radius > 0.0 {
                    continue;
                }
                if info.outer_radius <= 0.0 {
                    continue;
                }
                let dmg = blast_damage_at_distance(info, dist);
                let len = dist.max(0.001);
                hits.push(NeutronDamageHit {
                    target_index: idx,
                    damage: dmg,
                    topple_dx: dx / len,
                    topple_dz: dz / len,
                    topple_speed: info.topple_speed,
                    push_force: info.push_force,
                    set_burned: false,
                });
                if dmg > 0.0 {
                    state.total_damage_applied += dmg;
                    state.damage_hits = state.damage_hits.saturating_add(1);
                }
                if info.topple_speed > 0.0 {
                    state.topple_requests = state.topple_requests.saturating_add(1);
                }
            }
        }
        // Scorch / burned wave
        if !state.completed_scorch[i] && elapsed >= info.scorch_delay_frames {
            state.completed_scorch[i] = true;
            state.scorch_waves = state.scorch_waves.saturating_add(1);
            for (idx, &(ox, oz)) in object_xz.iter().enumerate() {
                let dx = ox - epicenter.0;
                let dz = oz - epicenter.1;
                let dist = (dx * dx + dz * dz).sqrt();
                if info.outer_radius > 0.0 && dist <= info.outer_radius {
                    hits.push(NeutronDamageHit {
                        target_index: idx,
                        damage: 0.0,
                        topple_dx: 0.0,
                        topple_dz: 0.0,
                        topple_speed: 0.0,
                        push_force: 0.0,
                        set_burned: true,
                    });
                }
            }
        }
    }

    if current_frame >= state.destruction_frame() {
        state.done = true;
    }
    let done = state.done;
    (hits, place_scorch, done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blast6_full_damage_inside_inner() {
        let b = retail_neutron_blasts()[5];
        assert!((blast_damage_at_distance(&b, 10.0) - 3500.0).abs() < 0.1);
        assert!(blast_damage_at_distance(&b, 200.0) >= 300.0);
        assert_eq!(blast_damage_at_distance(&b, 300.0), 0.0);
    }

    #[test]
    fn sequence_fires_damage_after_blast6_delay() {
        let mut s = HostNeutronMissileSlowDeathData::begin(0);
        let objs = [(0.0_f32, 0.0_f32)];
        // Before blast6
        let early = ms_to_frames(500);
        let (h0, _, _) = plan_neutron_frame(&mut s, early, (0.0, 0.0), &objs);
        assert!(h0.iter().all(|h| h.damage == 0.0));
        // At blast6
        let t = ms_to_frames(1180);
        let (h1, _, _) = plan_neutron_frame(&mut s, t, (0.0, 0.0), &objs);
        assert!(h1.iter().any(|h| h.damage >= 3500.0));
    }

    #[test]
    fn destruction_delay_completes() {
        let mut s = HostNeutronMissileSlowDeathData::begin(10);
        let end = s.destruction_frame();
        let (_, _, done) = plan_neutron_frame(&mut s, end, (0.0, 0.0), &[]);
        assert!(done);
    }
}

//! Host A10 Thunderbolt DeliverPayload residual (jet + missiles).
//!
//! C++: `SUPERWEAPON_A10ThunderboltMissileStrike1/2/3` DeliverPayload
//! Transport=`AmericaJetA10Thunderbolt`, Payload=`A10ThunderboltMissile`,
//! FormationSize 1/2/3, FormationSpacing 35, DropDelay 500ms,
//! VisibleItemsDroppedPerInterval 2, VisibleNumBones 6.
//!
//! Residual playability slice:
//! - Spawn jet transport residual toward target
//! - Schedule missile drops along formation line with DropDelay stagger
//! - Missiles dive and apply A10ThunderboltMissileWeapon residual
//!
//! Fail-closed: not full AmericaJetA10 pathfinder / preferred-height locomotor.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    A10StrikeScienceTier, A10_FORMATIONION_SPACING, A10_MISSILE_PRIMARY_DAMAGE,
    A10_MISSILE_PRIMARY_RADIUS, A10_PAYLOAD_TEMPLATE, A10_STRIKE_IMPACT_DELAY_FRAMES,
    A10_TRANSPORT,
};

/// Retail DropDelay residual (ms) for A10 payload sets.
pub const A10_DROP_DELAY_MS: u32 = 500;
/// DropDelay frames @ 30 FPS.
pub const A10_DROP_DELAY_FRAMES: u32 = 15;
/// Retail VisibleItemsDroppedPerInterval residual.
pub const A10_ITEMS_PER_DROP: u32 = 2;
/// Retail VisibleNumBones residual (max missiles per jet).
pub const A10_NUM_BONES: u32 = 6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingA10MissileDrop {
    pub drop_frame: u32,
    pub target: Vec3,
    pub source_id: u32,
    pub missile_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostA10StrikeFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: A10StrikeScienceTier,
    pub transport_alive: bool,
}

impl HostA10StrikeFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: A10StrikeScienceTier) -> Self {
        Self {
            target,
            launch,
            tier,
            transport_alive: true,
        }
    }

    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        let dest = self.target;
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 22.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(140.0);
        if dist < 5.0 {
            return (new_pos, Vec3::ZERO, true);
        }
        let step = speed.min(dist);
        new_pos.x += dx / dist * step;
        new_pos.z += dz / dist * step;
        let vel = new_pos - pos;
        let over = dist <= 60.0;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostA10StrikeFlightRegistry {
    pub transports_spawned: u32,
    pub missiles_scheduled: u32,
    pub missiles_dropped: u32,
    pub impacts: u32,
    pub pending_drops: Vec<PendingA10MissileDrop>,
}

impl HostA10StrikeFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Build formation targets: FormationSize jets × bones with spacing residual.
    pub fn formation_targets(target: Vec3, tier: A10StrikeScienceTier) -> Vec<Vec3> {
        let jets = tier.formation_size().max(1);
        let mut out = Vec::new();
        // Per jet: drop up to VisibleNumBones missiles in pairs along run.
        let half = (jets as f32 - 1.0) * 0.5;
        for j in 0..jets {
            let lateral = (j as f32 - half) * A10_FORMATIONION_SPACING;
            // 3 drop pairs × 2 missiles = 6 bones residual.
            let pairs = (A10_NUM_BONES / A10_ITEMS_PER_DROP).max(1);
            for p in 0..pairs {
                let along = (p as f32 - (pairs as f32 - 1.0) * 0.5) * 20.0;
                // two missiles per interval residual
                for k in 0..A10_ITEMS_PER_DROP {
                    let side = if k == 0 { -6.0 } else { 6.0 };
                    out.push(Vec3::new(
                        target.x + along,
                        0.0,
                        target.z + lateral + side,
                    ));
                }
            }
        }
        out
    }

    pub fn schedule_drops(
        &mut self,
        activate_frame: u32,
        source_id: u32,
        target: Vec3,
        tier: A10StrikeScienceTier,
    ) {
        let points = Self::formation_targets(target, tier);
        for (i, pt) in points.into_iter().enumerate() {
            // DropDelay residual between pairs (every 2 missiles).
            let pair = (i as u32) / A10_ITEMS_PER_DROP;
            let drop_frame = activate_frame
                .saturating_add(A10_STRIKE_IMPACT_DELAY_FRAMES)
                .saturating_add(pair.saturating_mul(A10_DROP_DELAY_FRAMES));
            // Fall residual: drop a few frames before impact.
            let drop_frame = drop_frame.saturating_sub(10);
            self.pending_drops.push(PendingA10MissileDrop {
                drop_frame,
                target: pt,
                source_id,
                missile_index: i as u32,
            });
            self.missiles_scheduled = self.missiles_scheduled.saturating_add(1);
        }
    }

    pub fn take_due_drops(&mut self, frame: u32) -> Vec<PendingA10MissileDrop> {
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for p in self.pending_drops.drain(..) {
            if p.drop_frame <= frame {
                due.push(p);
            } else {
                keep.push(p);
            }
        }
        self.pending_drops = keep;
        due
    }

    pub fn record_transport(&mut self) {
        self.transports_spawned = self.transports_spawned.saturating_add(1);
    }

    pub fn record_drop(&mut self) {
        self.missiles_dropped = self.missiles_dropped.saturating_add(1);
    }

    pub fn record_impact(&mut self) {
        self.impacts = self.impacts.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 || self.missiles_scheduled > 0 || self.impacts > 0
    }
}

pub fn honesty_a10_strike_flight_residual_ok() -> bool {
    A10_TRANSPORT == "AmericaJetA10Thunderbolt"
        && A10_PAYLOAD_TEMPLATE == "A10ThunderboltMissile"
        && (A10_MISSILE_PRIMARY_DAMAGE - 200.0).abs() < 0.1
        && (A10_MISSILE_PRIMARY_RADIUS - 50.0).abs() < 0.1
        && A10_DROP_DELAY_FRAMES == 15
        && A10_ITEMS_PER_DROP == 2
        && A10_NUM_BONES == 6
        && HostA10StrikeFlightRegistry::formation_targets(
            Vec3::ZERO,
            A10StrikeScienceTier::Level1,
        )
        .len()
            == 6
        && {
            let mut reg = HostA10StrikeFlightRegistry::new();
            reg.schedule_drops(
                0,
                1,
                Vec3::new(100.0, 0.0, 0.0),
                A10StrikeScienceTier::Level1,
            );
            reg.missiles_scheduled == 6
                && reg.pending_drops.first().map(|p| p.drop_frame).unwrap_or(0)
                    < reg.pending_drops.last().map(|p| p.drop_frame).unwrap_or(0)
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_staggered_l1_missiles() {
        assert!(honesty_a10_strike_flight_residual_ok());
    }
}

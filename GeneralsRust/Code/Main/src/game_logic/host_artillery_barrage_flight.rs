//! Host ArtilleryBarrage OCL DeliverPayload residual (cannon + shells).
//!
//! C++: `SUPERWEAPON_ArtilleryBarrage1/2/3` DeliverPayload
//! Transport=`ChinaArtilleryCannon`, Payload=`ChinaArtilleryBarrageShell`,
//! FormationSize 12/24/36, DelayDelivery 0–3000ms, WeaponErrorRadius 100.
//!
//! Residual playability slice:
//! - Spawn transport residual near target approach
//! - Schedule shell drops via `artillery_barrage_points` + DelayDelivery stagger
//! - Spawn shell objects that fall and detonate
//! - Impact damage residual (`ArtilleryBarrageDamageWeapon` 105/r50)
//!
//! Fail-closed: not full ChinaArtilleryCannon locomotor / preferred-height path.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    artillery_barrage_points_for_tier, artillery_shell_impact_frame, ArtilleryBarrageScienceTier,
    ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_PREFERRED_HEIGHT, ARTILLERY_BARRAGE_RADIUS,
    ARTILLERY_BARRAGE_SHELL_OBJECT, ARTILLERY_BARRAGE_TRANSPORT,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingArtilleryShellDrop {
    pub drop_frame: u32,
    pub target: Vec3,
    pub source_id: u32,
    pub shell_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: ArtilleryBarrageScienceTier,
    pub transport_alive: bool,
}

impl HostArtilleryBarrageFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: ArtilleryBarrageScienceTier) -> Self {
        Self {
            target,
            launch,
            tier,
            transport_alive: true,
        }
    }

    /// Advance cannon transport toward target residual.
    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        let dest = self.target;
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 14.0_f32;
        let mut new_pos = pos;
        new_pos.y = ARTILLERY_BARRAGE_PREFERRED_HEIGHT.max(120.0);
        if dist < 5.0 {
            return (new_pos, Vec3::ZERO, true);
        }
        let step = speed.min(dist);
        new_pos.x += dx / dist * step;
        new_pos.z += dz / dist * step;
        let vel = new_pos - pos;
        let over = dist <= 80.0;
        (new_pos, vel, over)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightRegistry {
    pub transports_spawned: u32,
    pub shells_scheduled: u32,
    pub shells_dropped: u32,
    pub impacts: u32,
    pub pending_drops: Vec<PendingArtilleryShellDrop>,
}

impl HostArtilleryBarrageFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn schedule_drops(
        &mut self,
        activate_frame: u32,
        source_id: u32,
        target: Vec3,
        tier: ArtilleryBarrageScienceTier,
    ) {
        let points = artillery_barrage_points_for_tier(target, tier);
        for (i, pt) in points.into_iter().enumerate() {
            let impact = artillery_shell_impact_frame(activate_frame, i as u32);
            // Drop a few frames before residual impact (fall residual).
            let drop_frame = impact.saturating_sub(8);
            self.pending_drops.push(PendingArtilleryShellDrop {
                drop_frame,
                target: pt,
                source_id,
                shell_index: i as u32,
            });
            self.shells_scheduled = self.shells_scheduled.saturating_add(1);
        }
    }

    pub fn take_due_drops(&mut self, frame: u32) -> Vec<PendingArtilleryShellDrop> {
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
        self.shells_dropped = self.shells_dropped.saturating_add(1);
    }

    pub fn record_impact(&mut self) {
        self.impacts = self.impacts.saturating_add(1);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 || self.shells_scheduled > 0 || self.impacts > 0
    }
}

pub fn honesty_artillery_barrage_flight_residual_ok() -> bool {
    ARTILLERY_BARRAGE_TRANSPORT == "ChinaArtilleryCannon"
        && ARTILLERY_BARRAGE_SHELL_OBJECT == "ChinaArtilleryBarrageShell"
        && (ARTILLERY_BARRAGE_DAMAGE - 105.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_RADIUS - 50.0).abs() < 0.1
        && artillery_barrage_points_for_tier(Vec3::ZERO, ArtilleryBarrageScienceTier::Level1).len()
            == 12
        && {
            let mut reg = HostArtilleryBarrageFlightRegistry::new();
            reg.schedule_drops(
                0,
                1,
                Vec3::new(100.0, 0.0, 0.0),
                ArtilleryBarrageScienceTier::Level1,
            );
            reg.shells_scheduled == 12
                && reg.pending_drops.first().map(|p| p.drop_frame).unwrap_or(0)
                    < reg.pending_drops.last().map(|p| p.drop_frame).unwrap_or(0)
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_staggered_l1_formation() {
        assert!(honesty_artillery_barrage_flight_residual_ok());
    }
}

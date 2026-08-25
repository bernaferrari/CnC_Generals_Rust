//! Host CarpetBomb OCL DeliverPayload residual (B52 + line drops).
//!
//! C++: `SUPERWEAPON_CarpetBomb` DeliverPayload Transport=AmericaJetB52,
//! Payload=CarpetBomb ×15, DropDelay 300ms, DropVariance, DeliveryDistance 400.
//!
//! Residual playability slice:
//! - Spawn transport at edge residual toward target line
//! - Schedule bomb drop points along launch→target (C++ flight path) + DropDelay stagger
//! - Spawn CarpetBomb payload objects that HeightDie / explode
//! - Impact damage residual at each drop
//!
//! Fail-closed: not full AmericaJetB52 pathfinder / preferred-height locomotor.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    CARPET_BOMB_DAMAGE, CARPET_BOMB_DROP_VARIANCE_X, CARPET_BOMB_PAYLOAD_OBJECT,
    CARPET_BOMB_RADIUS, CARPET_BOMB_TRANSPORT, CarpetBombFactionTier,
    carpet_bomb_impact_frame_for_tier, carpet_bomb_points_for_tier,
    carpet_bomb_points_for_tier_along,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCarpetBombDrop {
    pub drop_frame: u32,
    pub target: Vec3,
    pub source_id: u32,
    pub bomb_index: u32,
    /// Transport that still carries this payload (C++ contain). 0 = unbound.
    #[serde(default)]
    pub transport_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCarpetBombFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: CarpetBombFactionTier,
    pub transport_alive: bool,
    /// C++ DeliveringState finished → HeadOffMapState.
    #[serde(default)]
    pub delivery_complete: bool,
    /// C++ HeadOffMapState: dest is HUGE_DIST after delivery.
    #[serde(default)]
    pub passed_target: bool,
    /// Map extent for C++ `isOffMap` / HeadOffMap HUGE_DIST (world_min/max).
    #[serde(default)]
    pub map_min: Vec3,
    #[serde(default)]
    pub map_max: Vec3,
}

impl HostCarpetBombFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: CarpetBombFactionTier) -> Self {
        Self {
            target,
            launch,
            tier,
            transport_alive: true,
            delivery_complete: false,
            passed_target: false,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
        }
    }

    fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ Approach/Delivering toward moveToPos, then HeadOffMap HUGE_DIST.
    /// Returns (new_pos, vel, off_map / CLEAN_UP).
    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        use crate::game_logic::host_deliver_payload::{
            RESIDUAL_MAP_EXTENT_MAX_X, RESIDUAL_MAP_EXTENT_MAX_Z, RESIDUAL_MAP_EXTENT_MIN_X,
            RESIDUAL_MAP_EXTENT_MIN_Z, head_off_map_exit_point_residual, is_off_map_residual,
        };
        let hx = self.target.x - self.launch.x;
        let hz = self.target.z - self.launch.z;
        if self.delivery_complete && !self.passed_target {
            self.passed_target = true;
        }
        let (min_x, min_z, max_x, max_z) = if self.map_extent_ok() {
            (
                self.map_min.x,
                self.map_min.z,
                self.map_max.x,
                self.map_max.z,
            )
        } else {
            (
                RESIDUAL_MAP_EXTENT_MIN_X,
                RESIDUAL_MAP_EXTENT_MIN_Z,
                RESIDUAL_MAP_EXTENT_MAX_X,
                RESIDUAL_MAP_EXTENT_MAX_Z,
            )
        };
        let dest = if self.delivery_complete {
            head_off_map_exit_point_residual(pos, hx, hz, min_x, min_z, max_x, max_z)
        } else {
            self.target
        };
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 18.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(120.0);
        let vel = if dist >= 1.0 {
            let step = speed.min(dist);
            new_pos.x += dx / dist * step;
            new_pos.z += dz / dist * step;
            new_pos - pos
        } else {
            Vec3::ZERO
        };
        let at_exit =
            self.delivery_complete && is_off_map_residual(new_pos, min_x, min_z, max_x, max_z);
        (new_pos, vel, at_exit)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCarpetBombFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_scheduled: u32,
    pub bombs_dropped: u32,
    pub impacts: u32,
    pub pending_drops: Vec<PendingCarpetBombDrop>,
}

impl HostCarpetBombFlightRegistry {
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
        launch: Vec3,
        tier: CarpetBombFactionTier,
        transport_id: u32,
    ) {
        let points = carpet_bomb_points_for_tier_along(target, tier, launch);
        for (i, pt) in points.into_iter().enumerate() {
            let drop_frame = carpet_bomb_impact_frame_for_tier(activate_frame, i as u32, tier);
            // Drop slightly before strike residual impact (approach residual).
            let drop_frame = drop_frame.saturating_sub(tier.drop_delay_frames().min(3));
            self.pending_drops.push(PendingCarpetBombDrop {
                drop_frame,
                target: pt,
                source_id,
                bomb_index: i as u32,
                transport_id,
            });
            self.bombs_scheduled = self.bombs_scheduled.saturating_add(1);
        }
    }
    /// C++ payload is contained on the transport. Dead bomber cancels remaining drops.
    pub fn take_due_drops(
        &mut self,
        frame: u32,
        alive_transports: &[u32],
    ) -> Vec<PendingCarpetBombDrop> {
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for p in self.pending_drops.drain(..) {
            let transport_alive = if p.transport_id == 0 {
                !alive_transports.is_empty()
            } else {
                alive_transports.contains(&p.transport_id)
            };
            if !transport_alive {
                continue;
            }
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
        self.bombs_dropped = self.bombs_dropped.saturating_add(1);
    }
    pub fn record_impact(&mut self) {
        self.impacts = self.impacts.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 || self.bombs_scheduled > 0 || self.impacts > 0
    }
}

pub fn honesty_carpet_bomb_flight_residual_ok() -> bool {
    CARPET_BOMB_TRANSPORT == "AmericaJetB52"
        && CARPET_BOMB_PAYLOAD_OBJECT == "CarpetBomb"
        && (CARPET_BOMB_DAMAGE - 300.0).abs() < 0.1
        && (CARPET_BOMB_RADIUS - 50.0).abs() < 0.1
        && carpet_bomb_points_for_tier(Vec3::ZERO, CarpetBombFactionTier::America).len() == 15
        && {
            let mut reg = HostCarpetBombFlightRegistry::new();
            reg.schedule_drops(
                0,
                1,
                Vec3::new(100.0, 0.0, 0.0),
                Vec3::ZERO,
                CarpetBombFactionTier::America,
                42,
            );
            let scheduled_ok = reg.bombs_scheduled == 15
                && reg.pending_drops[0].drop_frame < reg.pending_drops[14].drop_frame;
            let along_z = carpet_bomb_points_for_tier_along(
                Vec3::ZERO,
                CarpetBombFactionTier::America,
                Vec3::new(0.0, 0.0, -1.0),
            );
            let axis_ok = along_z[0].z < along_z[14].z
                && along_z[0].x.abs() <= CARPET_BOMB_DROP_VARIANCE_X + 0.1;
            let killed = reg.take_due_drops(u32::MAX, &[]);
            scheduled_ok && axis_ok && killed.is_empty() && reg.pending_drops.is_empty()
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_staggered_usa_payload() {
        assert!(honesty_carpet_bomb_flight_residual_ok());
    }

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        // C++ HeadOffMapState + CleanUpState: fly HUGE_DIST, destroy when isOffMap.
        let mut data = HostCarpetBombFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            CarpetBombFactionTier::America,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        data.delivery_complete = true;
        let mut pos = Vec3::new(98.0, 150.0, 0.0);
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, vel, at_exit) = data.tick_transport(pos);
            assert!(
                vel.x > 0.0 || at_exit,
                "must keep flying past the target, pos={next:?}"
            );
            pos = next;
            if pos.x > 100.0 {
                left = true;
            }
            if at_exit {
                destroyed = true;
                break;
            }
        }
        assert!(left, "bomber must leave the target, pos={pos:?}");
        assert!(
            destroyed,
            "C++ isOffMap must destroy the transport, pos={pos:?}"
        );
        assert!(pos.x > 200.0 || pos.x < 0.0 || pos.z < 0.0 || pos.z > 200.0);
    }

    #[test]
    fn waits_at_target_until_delivery_then_heads_off() {
        let mut data = HostCarpetBombFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            CarpetBombFactionTier::America,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        let mut pos = Vec3::new(98.0, 150.0, 0.0);
        for _ in 0..8 {
            let (next, _, at_exit) = data.tick_transport(pos);
            pos = next;
            assert!(!at_exit, "must not HeadOffMap during DeliveringState");
        }
        assert!((pos.x - 100.0).abs() < 2.0, "holds moveToPos, pos={pos:?}");
        data.delivery_complete = true;
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, _, at_exit) = data.tick_transport(pos);
            pos = next;
            if pos.x > 100.0 {
                left = true;
            }
            if at_exit {
                destroyed = true;
                break;
            }
        }
        assert!(
            left && destroyed,
            "C++ HeadOffMap+CleanUp after delivery, pos={pos:?}"
        );
    }
}

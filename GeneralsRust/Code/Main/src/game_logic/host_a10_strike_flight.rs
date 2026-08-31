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

use crate::game_logic::host_deliver_payload::is_close_enough_to_target_squared_residual;
use crate::game_logic::special_power_strikes::{
    A10_DELIVERY_DISTANCE, A10_DIVE_END_DISTANCE, A10_DIVE_START_DISTANCE,
    A10_FORMATIONION_SPACING, A10_MISSILE_PRIMARY_DAMAGE, A10_MISSILE_PRIMARY_RADIUS,
    A10_PAYLOAD_TEMPLATE, A10_PRE_OPEN_DISTANCE, A10_TRANSPORT, A10StrikeScienceTier,
};

/// C++ DeliverPayloadAIUpdate `DIVESTATE_PREDIVE`.
pub const A10_DIVE_PREDIVE: u8 = 0;
/// C++ DeliverPayloadAIUpdate `DIVESTATE_DIVING`.
pub const A10_DIVE_DIVING: u8 = 1;
/// C++ DeliverPayloadAIUpdate `DIVESTATE_POSTDIVE`.
pub const A10_DIVE_POSTDIVE: u8 = 2;
/// Host residual preferred-height cruise (precise-Z off).
pub const A10_CRUISE_HEIGHT: f32 = 140.0;
/// C++ `ThingTemplate::getPerUnitSound("StartDive")`.
pub const A10_START_DIVE_SOUND: &str = "StartDive";
/// C++ A10ThunderboltVulcan DelayBetweenShots 60ms → 2 frames @ 30 FPS.
pub const A10_VULCAN_DELAY_FRAMES: u32 = 2;

/// One tick of C++ DeliverPayloadAIUpdate dive + StrafingWeaponSlot (cpp:155-220).
#[derive(Debug, Clone, Copy)]
pub struct A10DiveTick {
    pub new_y: f32,
    pub start_dive: bool,
    pub should_strafe: bool,
    pub strafe_point: Vec3,
}

/// Live Y-up residual of leftover `DeliverPayloadAIUpdate::update_dive_logic`.
/// C++ `velocity.z` (up) is `vel.y` here. PREDIVE uses 2D start 500; DIVING
/// ends on 3D 300; vulcan fires while diving and `vel.y < 5`.
pub fn tick_a10_dive(
    dive_state: &mut u8,
    pos: Vec3,
    target: Vec3,
    vel: Vec3,
    speed: f32,
) -> A10DiveTick {
    let mut out = A10DiveTick {
        new_y: pos.y,
        start_dive: false,
        should_strafe: false,
        strafe_point: Vec3::new(target.x, 0.0, target.z),
    };
    if *dive_state == A10_DIVE_POSTDIVE {
        if pos.y < A10_CRUISE_HEIGHT {
            out.new_y = (pos.y + speed).min(A10_CRUISE_HEIGHT);
        } else {
            out.new_y = pos.y.max(A10_CRUISE_HEIGHT);
        }
        return out;
    }
    let dx = pos.x - target.x;
    let dz = pos.z - target.z;
    let dy = pos.y - target.y;
    let dist2_sq = dx * dx + dz * dz;
    if *dive_state == A10_DIVE_PREDIVE {
        out.new_y = pos.y.max(A10_CRUISE_HEIGHT);
        if dist2_sq <= A10_DIVE_START_DISTANCE * A10_DIVE_START_DISTANCE {
            *dive_state = A10_DIVE_DIVING;
            out.start_dive = true;
        }
        return out;
    }
    let dist3_sq = dist2_sq + dy * dy;
    if dist3_sq <= A10_DIVE_END_DISTANCE * A10_DIVE_END_DISTANCE {
        *dive_state = A10_DIVE_POSTDIVE;
    } else {
        let dist3 = dist3_sq.sqrt();
        if dist3 > 1e-4 {
            let step = speed.min(dist3);
            out.new_y = pos.y + (target.y - pos.y) / dist3 * step;
        }
    }
    if vel.y < 5.0 {
        let current = dist3_sq.sqrt();
        let denom = (A10_DIVE_START_DISTANCE - A10_DIVE_END_DISTANCE).max(0.001);
        let dive_ratio = (A10_DIVE_START_DISTANCE - current) / denom;
        let mut v = Vec3::new(vel.x, 0.0, vel.z);
        let len = v.length();
        if len > 1e-6 {
            v /= len;
        }
        v *= dive_ratio * 100.0;
        let backwards = v * 0.33;
        let mut strafe = target - backwards + v;
        strafe.y = 0.0;
        out.strafe_point = strafe;
        out.should_strafe = true;
    }
    out
}

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
    #[serde(default)]
    pub exit: Vec3,
    pub tier: A10StrikeScienceTier,
    pub transport_alive: bool,
    #[serde(default)]
    pub passed_target: bool,
    #[serde(default)]
    pub last_vulcan_frame: u32,
    /// C++ DeliverPayloadAIUpdate `m_diveState`.
    #[serde(default)]
    pub dive_state: u8,
    /// C++ DeliverPayload visible bones still to drop (VisibleNumBones 6).
    #[serde(default)]
    pub missiles_remaining: u32,
    /// Next frame a visible-payload pair may drop (DropDelay 15).
    #[serde(default)]
    pub next_drop_frame: u32,
    /// Previous 2D dist² for inbound DeliverPayload::isCloseEnoughToTarget.
    #[serde(default = "host_a10_prev_dist_sq_default")]
    pub prev_dist_sq: f32,
    /// Map extent for C++ `isOffMap` / HeadOffMap HUGE_DIST (world_min/max).
    #[serde(default)]
    pub map_min: Vec3,
    #[serde(default)]
    pub map_max: Vec3,
}

fn host_a10_prev_dist_sq_default() -> f32 {
    f32::MAX
}

impl HostA10StrikeFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: A10StrikeScienceTier) -> Self {
        Self::start_with_exit(launch, target, Vec3::ZERO, tier)
    }

    pub fn start_with_exit(
        launch: Vec3,
        target: Vec3,
        exit: Vec3,
        tier: A10StrikeScienceTier,
    ) -> Self {
        Self {
            target,
            launch,
            exit,
            tier,
            transport_alive: true,
            passed_target: false,
            last_vulcan_frame: 0,
            dive_state: A10_DIVE_PREDIVE,
            missiles_remaining: A10_NUM_BONES,
            next_drop_frame: 0,
            prev_dist_sq: f32::MAX,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
        }
    }

    fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ HeadOffMapState: after the target, dest is HUGE_DIST ahead.
    /// Returns (new_pos, vel, off_map / CLEAN_UP).
    pub fn tick_transport(&mut self, pos: Vec3) -> (Vec3, Vec3, bool) {
        use crate::game_logic::host_deliver_payload::{
            head_off_map_exit_point_residual, is_off_map_residual,
        };
        let hx = self.target.x - self.launch.x;
        let hz = self.target.z - self.launch.z;
        if !self.passed_target {
            let dx = self.target.x - pos.x;
            let dz = self.target.z - pos.z;
            if dx * dx + dz * dz <= 25.0
                || (pos.x - self.target.x) * hx + (pos.z - self.target.z) * hz > 0.0
            {
                self.passed_target = true;
            }
        }
        let dest = if self.passed_target {
            if self.map_extent_ok() {
                head_off_map_exit_point_residual(
                    pos,
                    hx,
                    hz,
                    self.map_min.x,
                    self.map_min.z,
                    self.map_max.x,
                    self.map_max.z,
                )
            } else if self.exit.length_squared() > 1.0 {
                self.exit
            } else {
                self.target
            }
        } else {
            self.target
        };
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let speed = 22.0_f32;
        let mut new_pos = pos;
        new_pos.y = new_pos.y.max(140.0);
        let vel = if dist >= 1.0 {
            let step = speed.min(dist);
            new_pos.x += dx / dist * step;
            new_pos.z += dz / dist * step;
            new_pos - pos
        } else {
            Vec3::ZERO
        };
        let at_exit = self.passed_target
            && if self.map_extent_ok() {
                is_off_map_residual(
                    new_pos,
                    self.map_min.x,
                    self.map_min.z,
                    self.map_max.x,
                    self.map_max.z,
                )
            } else {
                self.exit.length_squared() > 1.0
                    && (new_pos.x - self.exit.x).abs() < 8.0
                    && (new_pos.z - self.exit.z).abs() < 8.0
            };
        (new_pos, vel, at_exit)
    }

    /// C++ `DeliverPayloadAIUpdate::isCloseEnoughToTarget` (DeliveryDistance 450,
    /// PreOpenDistance 0): inbound expands the allowed band to the sum.
    pub fn is_close_enough_to_target(&mut self, pos: Vec3) -> bool {
        let dx = pos.x - self.target.x;
        let dz = pos.z - self.target.z;
        let current = dx * dx + dz * dz;
        let previous = self.prev_dist_sq;
        self.prev_dist_sq = current;
        is_close_enough_to_target_squared_residual(
            current,
            previous,
            A10_DELIVERY_DISTANCE,
            A10_PRE_OPEN_DISTANCE,
        )
    }

    /// C++ `DeliveringState::update` VisibleItemsDroppedPerInterval while close.
    pub fn take_visible_payload_drops(&mut self, now: u32) -> u32 {
        if self.missiles_remaining == 0 {
            return 0;
        }
        if self.next_drop_frame != 0 && now < self.next_drop_frame {
            return 0;
        }
        let n = self.missiles_remaining.min(A10_ITEMS_PER_DROP);
        self.missiles_remaining = self.missiles_remaining.saturating_sub(n);
        self.next_drop_frame = now.saturating_add(A10_DROP_DELAY_FRAMES);
        n
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
                    out.push(Vec3::new(target.x + along, 0.0, target.z + lateral + side));
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
            let drop_frame =
                activate_frame.saturating_add(pair.saturating_mul(A10_DROP_DELAY_FRAMES));
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
        && HostA10StrikeFlightRegistry::formation_targets(Vec3::ZERO, A10StrikeScienceTier::Level1)
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

    #[test]
    fn close_enough_matches_cpp_delivery_distance() {
        // C++ DeliverPayloadAIUpdate.cpp:348-368 — 2D DeliveryDistance 450.
        let mut data = HostA10StrikeFlightData::start(
            Vec3::new(0.0, 160.0, 0.0),
            Vec3::new(1000.0, 0.0, 0.0),
            A10StrikeScienceTier::Level1,
        );
        assert!(
            !data.is_close_enough_to_target(Vec3::new(0.0, 160.0, 0.0)),
            "1000wu from target must not deliver"
        );
        assert!(
            data.is_close_enough_to_target(Vec3::new(600.0, 160.0, 0.0)),
            "400wu inbound must deliver"
        );
        // C++ strict boundary: exactly DeliveryDistance is outside the band.
        assert!(
            !data.is_close_enough_to_target(Vec3::new(550.0, 160.0, 0.0)),
            "exactly 450wu is outside the strict C++ band"
        );
        assert_eq!(data.take_visible_payload_drops(0), 2);
        assert_eq!(data.take_visible_payload_drops(1), 0);
        assert_eq!(data.take_visible_payload_drops(A10_DROP_DELAY_FRAMES), 2);
    }

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        // C++ HeadOffMapState + CleanUpState: fly HUGE_DIST, destroy when isOffMap.
        let mut data = HostA10StrikeFlightData::start_with_exit(
            Vec3::new(0.0, 160.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::ZERO,
            A10StrikeScienceTier::Level1,
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        let mut pos = Vec3::new(98.0, 160.0, 0.0);
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
        assert!(left, "jet must leave the target, pos={pos:?}");
        assert!(
            destroyed,
            "C++ isOffMap must destroy the transport, pos={pos:?}"
        );
        assert!(pos.x > 200.0 || pos.x < 0.0 || pos.z < 0.0 || pos.z > 200.0);
    }
}

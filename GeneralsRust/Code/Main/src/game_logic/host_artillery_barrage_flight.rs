//! Host ArtilleryBarrage OCL DeliverPayload residual (cannon + shells).
//!
//! C++: `SUPERWEAPON_ArtilleryBarrage1/2/3` DeliverPayload
//! Transport=`ChinaArtilleryCannon`, Payload=`ChinaArtilleryBarrageShell`,
//! FormationSize 12/24/36, DelayDelivery 0–3000ms, WeaponErrorRadius 100.
//!
//! Residual playability slice:
//! - Spawn FormationSize 12/24/36 ChinaArtilleryCannon at CREATE_AT_EDGE_FARTHEST_FROM_TARGET
//!   (z += 300) with C++ CW/CCW FormationSpacing offsets, DelayDeliveryMax stagger,
//!   and WeaponErrorRadius on non-lead targets. Each transport HeadOffMaps after firing.
//! - Schedule shell drops via `artillery_barrage_points` + DelayDelivery stagger
//! - Spawn shell objects that fall and detonate
//! - Impact damage residual (`ArtilleryBarrageDamageWeapon` 105/r50)
//!
//! Fail-closed: not full ChinaArtilleryCannon locomotor / preferred-height path.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_deliver_payload::is_close_enough_to_target_squared_residual;
use crate::game_logic::special_power_strikes::{
    ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
    ARTILLERY_BARRAGE_FORMATION_SPACING, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
    ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE, ARTILLERY_BARRAGE_PREFERRED_HEIGHT,
    ARTILLERY_BARRAGE_RADIUS, ARTILLERY_BARRAGE_SHELL_OBJECT, ARTILLERY_BARRAGE_TRANSPORT,
    ArtilleryBarrageScienceTier, artillery_barrage_points_for_tier,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingArtilleryShellDrop {
    pub drop_frame: u32,
    pub target: Vec3,
    pub source_id: u32,
    pub shell_index: u32,
}

/// C++ DeliverPayloadNugget formation pose (host Y-up: C++ Y → Z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArtilleryFormationPose {
    pub start: Vec3,
    pub move_to: Vec3,
    pub target: Vec3,
    pub orient: f32,
}

/// C++ ObjectCreationList.cpp:271-298 formation CW/CCW vectors (XZ).
pub fn artillery_formation_vectors(primary: Vec3, secondary: Vec3) -> (f32, f32, f32, f32) {
    let dx = primary.x - secondary.x;
    let dz = primary.z - secondary.z;
    let length = (dx * dx + dz * dz).sqrt();
    if length < 0.001 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let dx = dx / length;
    let dz = dz / length;
    let radians = std::f32::consts::FRAC_PI_2;
    let (s, c) = (radians.sin(), radians.cos());
    let ccw_x = dx * c + dz * -s + dx;
    let ccw_z = dx * s + dz * c + dz;
    let (s, c) = ((-radians).sin(), (-radians).cos());
    let cw_x = dx * c + dz * -s + dx;
    let cw_z = dx * s + dz * c + dz;
    (ccw_x, ccw_z, cw_x, cw_z)
}

/// C++ ObjectCreationList.cpp:303-319 formation member offset.
pub fn artillery_formation_offset(
    formation_index: u32,
    formation_size: u32,
    formation_spacing: f32,
    ccw_x: f32,
    ccw_z: f32,
    cw_x: f32,
    cw_z: f32,
) -> Vec3 {
    if formation_size <= 1 {
        return Vec3::ZERO;
    }
    let offset_multiplier = ((formation_index + 1) / 2) as f32 * formation_spacing;
    if formation_index % 2 == 1 {
        Vec3::new(ccw_x * offset_multiplier, 0.0, ccw_z * offset_multiplier)
    } else {
        Vec3::new(cw_x * offset_multiplier, 0.0, cw_z * offset_multiplier)
    }
}

/// C++ DeliverPayloadNugget::create formation start / moveTo / target + DeliveryDistance slop.
pub fn artillery_formation_pose(
    primary: Vec3,
    secondary: Vec3,
    formation_index: u32,
    formation_size: u32,
    formation_spacing: f32,
    convergence_factor: f32,
    dist_to_target: f32,
    error_sample: Option<(f32, f32)>,
) -> ArtilleryFormationPose {
    let (ccw_x, ccw_z, cw_x, cw_z) = if formation_size > 1 {
        artillery_formation_vectors(primary, secondary)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    let offset = artillery_formation_offset(
        formation_index,
        formation_size,
        formation_spacing,
        ccw_x,
        ccw_z,
        cw_x,
        cw_z,
    );
    let mut start = primary + offset;
    let mut move_to = secondary + offset;
    let mut target = secondary;
    target.x += offset.x * (1.0 - convergence_factor);
    target.z += offset.z * (1.0 - convergence_factor);
    if formation_index > 0 {
        if let Some((random_radius, random_angle)) = error_sample {
            target.x += random_radius * random_angle.cos();
            target.z += random_radius * random_angle.sin();
        }
    }
    let orient = (move_to.z - start.z).atan2(move_to.x - start.x);
    if dist_to_target > 0.0 {
        const SLOP: f32 = 1.5;
        start.x -= orient.cos() * dist_to_target * SLOP;
        start.z -= orient.sin() * dist_to_target * SLOP;
    }
    ArtilleryFormationPose {
        start,
        move_to,
        target,
        orient,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: ArtilleryBarrageScienceTier,
    pub transport_alive: bool,
    /// C++ DeliveringState finished → HeadOffMapState.
    #[serde(default)]
    pub delivery_complete: bool,
    /// C++ HeadOffMapState: dest is HUGE_DIST after delivery.
    #[serde(default)]
    pub passed_target: bool,
    /// Previous 2D dist² for C++ isCloseEnoughToTarget inbound tracking
    /// (DeliverPayloadAIUpdate.cpp:99,356-357; starts at 0 like m_previousDistanceSqr).
    #[serde(default)]
    pub prev_dist_sq: f32,
    /// Map extent for C++ `isOffMap` / HeadOffMap HUGE_DIST (world_min/max).
    #[serde(default)]
    pub map_min: Vec3,
    #[serde(default)]
    pub map_max: Vec3,
    /// C++ setDisabledUntil(DISABLED_DEFAULT, now + GameLogicRandomValue(0, DelayDeliveryMax)).
    #[serde(default)]
    pub delay_until_frame: u32,
}

impl HostArtilleryBarrageFlightData {
    pub fn start(launch: Vec3, target: Vec3, tier: ArtilleryBarrageScienceTier) -> Self {
        Self {
            target,
            launch,
            tier,
            transport_alive: true,
            delivery_complete: false,
            passed_target: false,
            prev_dist_sq: 0.0,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
            delay_until_frame: 0,
        }
    }

    /// C++ DISABLED_DEFAULT hold until DelayDeliveryMax elapses.
    pub fn is_hold_for_delay(&self, frame: u32) -> bool {
        self.delay_until_frame > frame
    }

    pub fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ DeliveringState `isCloseEnoughToTarget` residual (live band):
    /// authored DeliveryDistance 250, PreOpenDistance 0; inbound expands the
    /// allowed band to the sum.
    pub fn in_delivery_band(&mut self, pos: Vec3) -> bool {
        let dx = self.target.x - pos.x;
        let dz = self.target.z - pos.z;
        let current = dx * dx + dz * dz;
        let previous = self.prev_dist_sq;
        self.prev_dist_sq = current;
        is_close_enough_to_target_squared_residual(
            current,
            previous,
            ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
            ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE,
        )
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
        let speed = 14.0_f32;
        let mut new_pos = pos;
        new_pos.y = ARTILLERY_BARRAGE_PREFERRED_HEIGHT.max(120.0);
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

    /// Shell `i` is the payload of transport `i` (ObjectCreationList.cpp:375-378):
    /// every transport — lead included — rolls rand(0, DelayDeliveryMax) and
    /// cannot deliver while disabled, so the schedule adds the same delay.
    pub fn schedule_drops(
        &mut self,
        activate_frame: u32,
        source_id: u32,
        target: Vec3,
        tier: ArtilleryBarrageScienceTier,
        transport_delays: &[u32],
    ) {
        let points = artillery_barrage_points_for_tier(target, tier);
        for (i, pt) in points.into_iter().enumerate() {
            let transport_delay = transport_delays.get(i).copied().unwrap_or(0);
            let impact = activate_frame
                .saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES)
                .saturating_add(transport_delay);
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
            let delays: Vec<u32> = (0..12u32).map(|i| i.saturating_mul(3)).collect();
            reg.schedule_drops(
                0,
                1,
                Vec3::new(100.0, 0.0, 0.0),
                ArtilleryBarrageScienceTier::Level1,
                &delays,
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

    #[test]
    fn delivery_band_is_authored_250_not_residual_80() {
        // C++ DeliveringState isCloseEnoughToTarget uses the authored
        // DeliveryDistance 250 (SUPERWEAPON_ArtilleryBarrage1), not the
        // mis-ported 80; exactly 250 is outside the strict band.
        let mut data = HostArtilleryBarrageFlightData::start(
            Vec3::new(0.0, 130.0, 0.0),
            Vec3::new(500.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level1,
        );
        assert!(
            data.in_delivery_band(Vec3::new(400.0, 130.0, 0.0)),
            "100wu inside the authored 250 DeliveryDistance band must deliver"
        );
        assert!(
            data.in_delivery_band(Vec3::new(251.0, 130.0, 0.0)),
            "249wu inside the authored 250 DeliveryDistance band must deliver"
        );
        assert!(
            !data.in_delivery_band(Vec3::new(500.0 - 251.0, 130.0, 0.0)),
            "251wu outside the authored 250 DeliveryDistance band must not deliver"
        );
    }

    #[test]
    fn shell_schedule_honors_transport_delay_delivery() {
        // ObjectCreationList.cpp:375-378 — each transport (lead included) rolls
        // rand(0, DelayDeliveryMax); its shell cannot land before its own
        // transport is enabled again, so the schedule adds the same delay.
        let mut reg = HostArtilleryBarrageFlightRegistry::new();
        let delays: Vec<u32> = (0..12u32).map(|i| i * 7 + 13).collect();
        reg.schedule_drops(
            10,
            1,
            Vec3::new(100.0, 0.0, 0.0),
            ArtilleryBarrageScienceTier::Level1,
            &delays,
        );
        for p in &reg.pending_drops {
            let impact =
                10 + ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES + delays[p.shell_index as usize];
            assert_eq!(
                p.drop_frame,
                impact - 8,
                "shell {} must honor transport delay",
                p.shell_index
            );
        }
        assert_ne!(
            reg.pending_drops[0].drop_frame,
            10 + ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES - 8,
            "lead shell must honor its transport's rolled DelayDelivery"
        );
    }

    #[test]
    fn formation_lead_is_unoffset_wings_use_cw_ccw() {
        let edge = Vec3::new(-256.0, 300.0, 0.0);
        let target = Vec3::new(200.0, 0.0, 0.0);
        let lead = artillery_formation_pose(
            edge,
            target,
            0,
            12,
            ARTILLERY_BARRAGE_FORMATION_SPACING,
            0.0,
            ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
            None,
        );
        let wing = artillery_formation_pose(
            edge,
            target,
            1,
            12,
            ARTILLERY_BARRAGE_FORMATION_SPACING,
            0.0,
            0.0,
            None,
        );
        // DeliveryDistance * 1.5 slop pushes the lead further off the far rim.
        assert!(
            lead.start.x < edge.x - 300.0,
            "lead must spawn behind farthest edge, start={:?}",
            lead.start
        );
        assert!(
            (wing.start.z - edge.z).abs() > 0.0 || (wing.start.x - edge.x).abs() > 0.0,
            "wing must take CW/CCW FormationSpacing offset"
        );
        assert_eq!(ArtilleryBarrageScienceTier::Level1.formation_size(), 12);
        assert_eq!(ArtilleryBarrageScienceTier::Level2.formation_size(), 24);
        assert_eq!(ArtilleryBarrageScienceTier::Level3.formation_size(), 36);
    }
}

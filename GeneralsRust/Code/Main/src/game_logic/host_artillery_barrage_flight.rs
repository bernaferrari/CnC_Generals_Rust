//! Per-carrier continuation for the artillery DeliverPayload order.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_deliver_payload::is_close_enough_to_target_squared_residual;
use crate::game_logic::special_power_strikes::{
    ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
    ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE, ARTILLERY_BARRAGE_RADIUS, ARTILLERY_BARRAGE_SHELL_OBJECT,
    ARTILLERY_BARRAGE_TRANSPORT, ArtilleryBarrageScienceTier, artillery_barrage_points_for_tier,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VisibleArtilleryPayload {
    pub object: String,
    pub weapon: String,
    pub bone: String,
    pub subobject: String,
    pub inherit_velocity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ArtilleryPayload {
    Visible(VisibleArtilleryPayload),
    TemplateOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub tier: ArtilleryBarrageScienceTier,
    pub(crate) aim: Vec3,
    /// Present while the carrier is approaching with its payload.
    pub(crate) payload: Option<ArtilleryPayload>,
    pub(crate) strike_id: Option<u32>,
    pub(crate) delivery_distance: f32,
    pub(crate) pre_open_distance: f32,
    pub(crate) speed_per_frame: f32,
    pub(crate) self_destruct_after_release: bool,
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
            aim: target,
            payload: Some(ArtilleryPayload::TemplateOnly),
            strike_id: None,
            delivery_distance: ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
            pre_open_distance: ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE,
            speed_per_frame: 14.0,
            self_destruct_after_release: false,
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
        let dx = self.aim.x - pos.x;
        let dz = self.aim.z - pos.z;
        let current = dx * dx + dz * dz;
        let previous = self.prev_dist_sq;
        self.prev_dist_sq = current;
        is_close_enough_to_target_squared_residual(
            current,
            previous,
            self.delivery_distance,
            self.pre_open_distance,
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
        let dest = if self.payload.is_none() {
            head_off_map_exit_point_residual(pos, hx, hz, min_x, min_z, max_x, max_z)
        } else {
            self.target
        };
        let dx = dest.x - pos.x;
        let dz = dest.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        let mut new_pos = pos;

        let vel = if dist >= 1.0 {
            let step = self.speed_per_frame.min(dist);
            new_pos.x += dx / dist * step;
            new_pos.z += dz / dist * step;
            new_pos - pos
        } else {
            Vec3::ZERO
        };
        let at_exit =
            self.payload.is_none() && is_off_map_residual(new_pos, min_x, min_z, max_x, max_z);
        (new_pos, vel, at_exit)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostArtilleryBarrageFlightRegistry {
    pub transports_spawned: u32,
    pub shells_dropped: u32,
    pub impacts: u32,
}

impl HostArtilleryBarrageFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
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
        self.transports_spawned > 0 || self.impacts > 0
    }
}

pub fn honesty_artillery_barrage_flight_residual_ok() -> bool {
    ARTILLERY_BARRAGE_TRANSPORT == "ChinaArtilleryCannon"
        && ARTILLERY_BARRAGE_SHELL_OBJECT == "ChinaArtilleryBarrageShell"
        && (ARTILLERY_BARRAGE_DAMAGE - 105.0).abs() < 0.1
        && (ARTILLERY_BARRAGE_RADIUS - 50.0).abs() < 0.1
        && artillery_barrage_points_for_tier(Vec3::ZERO, ArtilleryBarrageScienceTier::Level1).len()
            == 12
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

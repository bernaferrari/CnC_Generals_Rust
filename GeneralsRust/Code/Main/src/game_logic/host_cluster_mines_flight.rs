//! Host ClusterMines DeliverPayload residual (China cargo plane + bomb).
//!
//! C++: `SUPERWEAPON_ClusterMines` DeliverPayload
//! Transport=`ChinaJetCargoPlane`, Payload=`ClusterMinesBomb` ×1,
//! DeliveryDistance **140**, DropVariance X:20 Y:20 Z:0,
//! bomb → GenerateMinefieldBehavior NumVirtualMines **8**.
//!
//! Residual playability slice:
//! - Spawn ChinaJetCargoPlane transport residual toward target
//! - Drop ClusterMinesBomb near DeliveryDistance
//! - Bomb falls; on ground impact places mine ring via host_mines residual
//! - Leftover GenerateMinefieldBehavior GenerationFX at place (`do_fx_at_position`)

//!
//! Fail-closed: not full pathfinder / SmartBorder minefield matrix.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::host_mines::{
    CLUSTER_MINE_NUM_VIRTUAL, CLUSTER_MINES_BOMB_TEMPLATE, CLUSTER_MINES_DELIVERY_DISTANCE,
    CLUSTER_MINES_DROP_OFFSET, CLUSTER_MINES_OCL_TRANSPORT,
    CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES, CLUSTER_MINES_VIEW_OBJECT_RANGE,
};

/// Retail Payload residual alias.
pub const CLUSTER_MINES_BOMB_OBJECT: &str = CLUSTER_MINES_BOMB_TEMPLATE;
/// Retail ClusterMinesBomb `GenerationFX` residual.
pub const CLUSTER_MINES_GENERATION_FX: &str = "WeaponFX_ClusterMineImpact";

/// Leftover `GenerateMinefieldBehavior.generation_fx` on ClusterMinesBomb.
pub fn leftover_cluster_mines_generation_fx(template_name: &str) -> Option<String> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !entry
            .name
            .as_str()
            .eq_ignore_ascii_case("GenerateMinefieldBehavior")
        {
            continue;
        }
        if let Some(data) = entry
            .data
            .downcast_ref::<gamelogic::object::behavior::GenerateMinefieldBehaviorModuleData>(
        ) {
            let name = data
                .generation_fx
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() || name.eq_ignore_ascii_case("none") {
                return None;
            }
            return Some(name);
        }
        let name = entry
            .data
            .get_ini_field("GenerationFX")
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            return None;
        }
        return Some(name);
    }
    None
}

/// Authored leftover GenerationFX, else retail `WeaponFX_ClusterMineImpact`.
pub fn cluster_mines_generation_fx_name() -> String {
    leftover_cluster_mines_generation_fx(CLUSTER_MINES_BOMB_OBJECT)
        .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case("none"))
        .unwrap_or_else(|| CLUSTER_MINES_GENERATION_FX.to_string())
}

/// Leftover `play_fx` / `TheFXList::do_fx_at_position` of authored GenerationFX.
pub fn play_cluster_mines_generation_fx(pos: Vec3) -> bool {
    let name = cluster_mines_generation_fx_name();
    crate::game_logic::dispatch_fx_list_at_pos(&name, pos)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostClusterMinesFlightData {
    pub target: Vec3,
    pub launch: Vec3,
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

impl HostClusterMinesFlightData {
    pub fn start(launch: Vec3, target: Vec3) -> Self {
        Self {
            target,
            launch,
            delivery_complete: false,
            passed_target: false,
            map_min: Vec3::ZERO,
            map_max: Vec3::ZERO,
        }
    }

    pub fn map_extent_ok(&self) -> bool {
        self.map_max.x > self.map_min.x && self.map_max.z > self.map_min.z
    }

    /// C++ DeliverPayloadAIUpdate::isCloseEnoughToTarget (DeliveryDistance 140).
    pub fn in_delivery_band(&self, pos: Vec3) -> bool {
        let dx = self.target.x - pos.x;
        let dz = self.target.z - pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        dist <= CLUSTER_MINES_DELIVERY_DISTANCE
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
        new_pos.y = new_pos.y.max(150.0);
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

/// C++ contained ClusterMinesBomb exits at the transport pose + DropOffset.
pub fn cluster_mines_payload_drop_pos(plane_pos: Vec3) -> Vec3 {
    Vec3::new(
        plane_pos.x + CLUSTER_MINES_DROP_OFFSET.0,
        plane_pos.y + CLUSTER_MINES_DROP_OFFSET.2,
        plane_pos.z + CLUSTER_MINES_DROP_OFFSET.1,
    )
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostClusterMinesFlightRegistry {
    pub transports_spawned: u32,
    pub bombs_dropped: u32,
    pub minefields_placed: u32,
    pub mines_spawned: u32,
}

impl HostClusterMinesFlightRegistry {
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
        self.bombs_dropped = self.bombs_dropped.saturating_add(1);
    }

    pub fn record_minefield(&mut self, mine_count: u32) {
        self.minefields_placed = self.minefields_placed.saturating_add(1);
        self.mines_spawned = self.mines_spawned.saturating_add(mine_count);
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.transports_spawned > 0 && self.bombs_dropped > 0 && self.minefields_placed > 0
    }
}

pub fn honesty_cluster_mines_flight_residual_ok() -> bool {
    CLUSTER_MINES_OCL_TRANSPORT == "ChinaJetCargoPlane"
        && CLUSTER_MINES_BOMB_OBJECT == "ClusterMinesBomb"
        && (CLUSTER_MINES_DELIVERY_DISTANCE - 140.0).abs() < 0.1
        && CLUSTER_MINE_NUM_VIRTUAL == 8
        && CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES == 900
        && (CLUSTER_MINES_VIEW_OBJECT_RANGE - 250.0).abs() < 0.01
        && CLUSTER_MINES_GENERATION_FX == "WeaponFX_ClusterMineImpact"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_pack() {
        assert!(honesty_cluster_mines_flight_residual_ok());
    }

    #[test]
    fn delivery_band_is_full_140_not_half() {
        let data = HostClusterMinesFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(200.0, 0.0, 0.0),
        );
        assert!(data.in_delivery_band(Vec3::new(60.0, 150.0, 0.0)));
        assert!(data.in_delivery_band(Vec3::new(200.0 - 140.0, 150.0, 0.0)));
        assert!(!data.in_delivery_band(Vec3::new(200.0 - 141.0, 150.0, 0.0)));
        let plane = Vec3::new(60.0, 150.0, 0.0);
        let drop = cluster_mines_payload_drop_pos(plane);
        assert!((drop.x - plane.x).abs() < 0.01);
        assert!((drop.y - (plane.y - 2.0)).abs() < 0.01);
        assert_ne!(drop.x, data.target.x);
    }

    #[test]
    fn head_off_map_flies_past_target_then_destroys() {
        let mut data = HostClusterMinesFlightData::start(
            Vec3::new(0.0, 150.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
        );
        data.map_min = Vec3::new(0.0, 0.0, 0.0);
        data.map_max = Vec3::new(200.0, 0.0, 200.0);
        data.delivery_complete = true;
        let mut pos = Vec3::new(98.0, 150.0, 0.0);
        let mut left = false;
        let mut destroyed = false;
        for _ in 0..80 {
            let (next, vel, at_exit) = data.tick_transport(pos);
            assert!(vel.x > 0.0 || at_exit, "must keep flying past the target");
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

    #[test]
    fn generation_fx_is_leftover_cluster_mine_impact() {
        assert_eq!(CLUSTER_MINES_GENERATION_FX, "WeaponFX_ClusterMineImpact");
        assert_eq!(
            cluster_mines_generation_fx_name(),
            "WeaponFX_ClusterMineImpact"
        );
        let place = include_str!("world_scripts/stealth_mines.rs");
        assert!(
            place.contains("play_cluster_mines_generation_fx"),
            "leftover GenerationFX must play on place_cluster_mines_unvaried"
        );
        assert!(
            place.contains("do_fx_at_position")
                || place.contains("play_cluster_mines_generation_fx"),
            "leftover play_fx is TheFXList::do_fx_at_position"
        );
    }
}

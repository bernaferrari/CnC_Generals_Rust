//! Live TheRadar hooks: Object ctor addObject color + map-load newMap.
//!
//! C++ `Object` ctor (`Object.cpp:473`) always `TheRadar->addObject(this)`.
//! `Radar::addObject` (`Radar.cpp:376-443`) colors disguise / garrison /
//! player. `TerrainLogic` (`TerrainLogic.cpp:2589`) calls `TheRadar->newMap`.
//!
//! Stay off sides_list / polygon — this only binds radar extent + blips.

#![allow(unused_imports, non_snake_case)]
use super::super::*;

use game_engine::common::system::radar::{
    get_radar_system, register_radar_map_source, register_radar_object_provider,
    resolve_radar_object_color, Coord3D, RadarMapSource, RadarObject, RadarObjectInsert,
    RadarObjectProvider, RadarPriorityType,
};
use gamelogic::system::shroud_manager::get_shroud_manager;
use std::sync::{Arc, LazyLock, Mutex};

struct HostRadarMapState {
    min: Coord3D,
    max: Coord3D,
    ready: bool,
    local_player_id: u32,
}

impl HostRadarMapState {
    const fn empty() -> Self {
        Self {
            min: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            max: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            ready: false,
            local_player_id: 0,
        }
    }
}

static HOST_RADAR_MAP: Mutex<HostRadarMapState> = Mutex::new(HostRadarMapState::empty());

struct HostRadarMapSource;

static HOST_RADAR_MAP_REGISTERED: LazyLock<()> = LazyLock::new(|| {
    let _ = register_radar_map_source(Arc::new(HostRadarMapSource));
});

static HOST_RADAR_OBJECTS: Mutex<Vec<RadarObjectInsert>> = Mutex::new(Vec::new());

struct HostRadarObjectProvider;

static HOST_RADAR_PROVIDER_REGISTERED: LazyLock<()> = LazyLock::new(|| {
    let _ = register_radar_object_provider(Arc::new(HostRadarObjectProvider));
});

impl RadarObjectProvider for HostRadarObjectProvider {
    fn collect_objects(&self) -> Vec<RadarObjectInsert> {
        HOST_RADAR_OBJECTS
            .lock()
            .ok()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

fn ensure_radar_hooks_registered() {
    ensure_radar_map_source_registered();
    LazyLock::force(&HOST_RADAR_PROVIDER_REGISTERED);
}

/// Push leftover ShroudManager cells onto TheRadar for the last local player.
pub fn host_refresh_radar_shroud() {
    let pid = HOST_RADAR_MAP
        .lock()
        .ok()
        .map(|g| g.local_player_id)
        .unwrap_or(0);
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.refresh_radar_shroud_for_player(pid);
    }
}

impl RadarMapSource for HostRadarMapSource {
    fn map_extent(&self) -> Option<(Coord3D, Coord3D)> {
        let guard = HOST_RADAR_MAP.lock().ok()?;
        if !guard.ready {
            return None;
        }
        Some((guard.min, guard.max))
    }

    fn sample_cell(&self, _world_x: f32, _world_y: f32) -> Option<(f32, bool)> {
        Some((0.0, false))
    }
}

fn pack_player_color_argb(rgb: (u8, u8, u8)) -> u32 {
    0xFF00_0000 | ((rgb.0 as u32) << 16) | ((rgb.1 as u32) << 8) | (rgb.2 as u32)
}

fn host_to_radar_coord(pos: glam::Vec3) -> Coord3D {
    // C++ radar plane is XY; host world plane is XZ (Y-up).
    Coord3D::new(pos.x, pos.z, pos.y)
}

fn ensure_radar_map_source_registered() {
    LazyLock::force(&HOST_RADAR_MAP_REGISTERED);
    LazyLock::force(&HOST_RADAR_PROVIDER_REGISTERED);
}

fn store_radar_map_extent(min: glam::Vec3, max: glam::Vec3) -> Option<(Coord3D, Coord3D)> {
    let lo = host_to_radar_coord(min);
    let hi = host_to_radar_coord(max);
    if (hi.x - lo.x).abs() <= f32::EPSILON || (hi.y - lo.y).abs() <= f32::EPSILON {
        return None;
    }
    if let Ok(mut guard) = HOST_RADAR_MAP.lock() {
        guard.min = lo;
        guard.max = hi;
        guard.ready = true;
    }
    Some((lo, hi))
}

fn radar_priority_for_object(obj: &Object) -> RadarPriorityType {
    if obj.is_kind_of(KindOf::Structure) {
        RadarPriorityType::Structure
    } else if obj.is_kind_of(KindOf::Infantry)
        || obj.is_kind_of(KindOf::Vehicle)
        || obj.is_kind_of(KindOf::Aircraft)
        || obj.is_kind_of(KindOf::Hero)
    {
        RadarPriorityType::Unit
    } else {
        RadarPriorityType::Invalid
    }
}

impl GameLogic {
    fn host_player_color(&self, player_id: Option<u32>, team: Team) -> u32 {
        if let Some(id) = player_id {
            if let Some(player) = self.players.get(&id) {
                return pack_player_color_argb(player.color_rgb);
            }
        }
        if let Some(player) = self.players.values().find(|p| p.team == team) {
            return pack_player_color_argb(player.color_rgb);
        }
        0xFFC8_C8C8
    }

    fn host_player_index(&self, player_id: Option<u32>, team: Team) -> i32 {
        if let Some(id) = player_id {
            return id as i32;
        }
        self.players
            .values()
            .find(|p| p.team == team)
            .map(|p| p.id as i32)
            .unwrap_or(-1)
    }

    fn host_local_player(&self) -> Option<&Player> {
        self.players.values().find(|p| p.is_local)
    }

    fn host_owner_is_ally_of_local(&self, owner_team: Team, owner_id: Option<u32>) -> bool {
        let Some(local) = self.host_local_player() else {
            return true;
        };
        if owner_team == local.team {
            return true;
        }
        if let Some(id) = owner_id {
            if let Some(owner) = self.players.get(&id) {
                if owner.alliance_team >= 0 && owner.alliance_team == local.alliance_team {
                    return true;
                }
            }
        }
        false
    }

    fn host_radar_insert_spec(&self, obj: &Object) -> RadarObjectInsert {
        let owner_id = obj.owner_player_id;
        let owner_color = self.host_player_color(owner_id, obj.team);
        let owner_index = self.host_player_index(owner_id, obj.team);
        let local = self.host_local_player();
        let local_index = local.map(|p| p.id as i32).unwrap_or(-1);
        let local_active = local.map(|p| p.is_alive).unwrap_or(true);
        let is_local = owner_id
            .and_then(|id| self.players.get(&id).map(|p| p.is_local))
            .unwrap_or(false);

        let is_disguiser = obj.is_kind_of(KindOf::Disguiser);
        let disguised = obj.status.disguised;
        let (disguised_index, disguised_color) = if disguised {
            if let Some(team) = obj.disguise_as_team {
                let idx = self.host_player_index(None, team);
                (idx, self.host_player_color(None, team))
            } else {
                (-1, owner_color)
            }
        } else {
            (-1, owner_color)
        };

        let occupant_owner = obj.contained_units().into_iter().next().and_then(|uid| {
            self.objects.get(&uid).and_then(|occupant| {
                occupant.owner_player_id.or_else(|| {
                    self.players
                        .values()
                        .find(|p| p.team == occupant.team)
                        .map(|p| p.id)
                })
            })
        });
        let (contain_apparent_player_index, contain_apparent_color) =
            if let Some(pid) = occupant_owner {
                let color = self
                    .players
                    .get(&pid)
                    .map(|p| pack_player_color_argb(p.color_rgb))
                    .unwrap_or(owner_color);
                (Some(pid as i32), Some(color))
            } else {
                (None, None)
            };

        let pos = obj.get_position();
        let mut radar_obj = RadarObject::new(obj.id.0);
        radar_obj.world_pos = host_to_radar_coord(pos);
        radar_obj.priority = radar_priority_for_object(obj);
        radar_obj.is_local = is_local;
        radar_obj.is_stealth = obj.status.stealthed;
        radar_obj.is_detected = obj.status.detected;
        radar_obj.is_disguised = disguised;
        radar_obj.is_enemy = !self.host_owner_is_ally_of_local(obj.team, owner_id);
        radar_obj.is_hero = obj.is_kind_of(KindOf::Hero);
        // C++ StealthDetectorUpdate DetectionRange (or VisionRange fallback).
        // RadarSystem::update_stealth_detection only reveals when these are set.
        radar_obj.can_detect_stealth = obj.is_detector;
        radar_obj.radar_range = if obj.is_detector {
            obj.effective_detection_range()
        } else {
            0.0
        };
        // OBJECT_STATUS_DETECTED at insert → not STEALTHLOOK_INVISIBLE.
        radar_obj.stealth_revealed = radar_obj.is_detected || radar_obj.is_disguised;

        radar_obj.color = owner_color;

        RadarObjectInsert {
            object: radar_obj,
            is_disguiser,
            disguised,
            disguised_player_index: disguised_index,
            owner_player_index: owner_index,
            local_player_index: local_index,
            owner_is_ally_of_local: self.host_owner_is_ally_of_local(obj.team, owner_id),
            local_player_active: local_active,
            contain_apparent_player_index,
            contain_apparent_color,
            indicator_color: owner_color,
            owner_player_color: owner_color,
            disguised_player_color: disguised_color,
        }
    }

    /// Bind terrain extent into TheRadar (`Radar::newMap`) without touching
    /// sides_list / polygon. Force-reset so a reload does not keep stale blips.
    pub(in super::super) fn host_radar_on_map_loaded(&mut self) {
        ensure_radar_map_source_registered();
        let Some((lo, hi)) = store_radar_map_extent(self.world_min, self.world_max) else {
            return;
        };
        if let Ok(mut radar) = get_radar_system().write() {
            radar.new_map(lo, hi, &[]);
            let _ = radar.try_new_map_from_source();
        }
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            self.host_radar_add_object(id);
        }
    }

    /// C++ `TheRadar->addObject` from Object ctor / mid-game spawn.
    pub(in super::super) fn host_radar_add_object(&mut self, id: ObjectId) {
        ensure_radar_map_source_registered();
        let needs_extent = get_radar_system()
            .read()
            .ok()
            .is_some_and(|radar| !radar.has_map_extent());
        if needs_extent {
            if let Some((lo, hi)) = store_radar_map_extent(self.world_min, self.world_max) {
                if let Ok(mut radar) = get_radar_system().write() {
                    let _ = radar.try_new_map_from_source();
                    if !radar.has_map_extent() {
                        radar.new_map(lo, hi, &[]);
                    }
                }
            }
        }
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let spec = self.host_radar_insert_spec(obj);
        let _ = resolve_radar_object_color(&spec);
        if let Ok(mut radar) = get_radar_system().write() {
            radar.add_live_object(spec);
        }
    }

    /// C++ `TheRadar->removeObject` from Object dtor / destroy list.
    pub(in super::super) fn host_radar_remove_object(&mut self, id: ObjectId) {
        if let Ok(mut radar) = get_radar_system().write() {
            radar.remove_object(id.0);
        }
    }

    /// C++ `RadarObject::isTemporarilyHidden` + overlay draw use live Object
    /// pose/stealth each frame. Re-stamp every host object so cloaking,
    /// detection, and movement update the blip (hq-sn4a0).
    pub(in super::super) fn host_radar_sync_live_objects(&mut self) {
        ensure_radar_hooks_registered();
        if let Some(local) = self.host_local_player() {
            if let Ok(mut guard) = HOST_RADAR_MAP.lock() {
                guard.local_player_id = local.id;
            }
        }
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut specs = Vec::new();
        for id in &ids {
            if let Some(obj) = self.objects.get(id) {
                if obj.is_alive() {
                    specs.push(self.host_radar_insert_spec(obj));
                }
            }
        }
        if let Ok(mut store) = HOST_RADAR_OBJECTS.lock() {
            *store = specs.clone();
        }
        if let Some(local) = self.host_local_player() {
            if let Ok(mut shroud) = get_shroud_manager().lock() {
                shroud.refresh_radar_shroud_for_player(local.id);
            }
        }
        let live: std::collections::HashSet<u32> = ids.iter().map(|id| id.0).collect();
        if let Ok(mut radar) = get_radar_system().write() {
            let stale: Vec<u32> = radar
                .get_all_objects()
                .filter(|obj| !live.contains(&obj.object_id))
                .map(|obj| obj.object_id)
                .collect();
            for id in stale {
                radar.remove_object(id);
            }
        }
        for id in ids {
            let alive = self.objects.get(&id).is_some_and(|obj| obj.is_alive());
            if alive {
                self.host_radar_add_object(id);
            } else {
                self.host_radar_remove_object(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{Player, ThingTemplate};
    use game_engine::common::system::radar::get_radar_system;
    use glam::Vec3;

    /// C++ Radar.cpp:118-125 queries live Drawable stealth/pose each update.
    #[test]
    fn host_radar_sync_updates_position_and_stealth() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.add_player(Player::new(2, Team::China, "China", false));

        let mut tpl = ThingTemplate::new("RadarInfantry");
        tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("RadarInfantry".into(), tpl);

        let id = logic
            .create_object_for_player("RadarInfantry", 2, Vec3::new(10.0, 0.0, 20.0))
            .expect("spawn");
        logic.host_radar_add_object(id);

        {
            let radar = get_radar_system().read().expect("radar");
            let blip = radar
                .get_all_objects()
                .find(|o| o.object_id == id.0)
                .expect("blip");
            assert!((blip.world_pos.x - 10.0).abs() < 0.01);
            assert!(!blip.is_stealth);
            assert!(!blip.is_temporarily_hidden());
        }

        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(Vec3::new(80.0, 0.0, 90.0));
            obj.status.stealthed = true;
            obj.status.detected = false;
        }
        logic.host_radar_sync_live_objects();

        let radar = get_radar_system().read().expect("radar");
        let blip = radar
            .get_all_objects()
            .find(|o| o.object_id == id.0)
            .expect("moved blip");
        assert!(
            (blip.world_pos.x - 80.0).abs() < 0.01,
            "blip must track live pose, got {}",
            blip.world_pos.x
        );
        assert!(
            (blip.world_pos.y - 90.0).abs() < 0.01,
            "host XZ maps to radar XY"
        );
        assert!(blip.is_stealth);
        assert!(
            blip.is_temporarily_hidden(),
            "enemy cloak after spawn must hide the blip"
        );
    }
}

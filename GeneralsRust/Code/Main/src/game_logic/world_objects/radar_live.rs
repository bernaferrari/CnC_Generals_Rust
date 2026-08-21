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
    samples: Vec<(f32, bool)>,
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
            samples: Vec::new(),
        }
    }

    fn sample(&self, world_x: f32, world_y: f32) -> Option<(f32, bool)> {
        const W: usize = 128;
        const H: usize = 128;
        if self.samples.len() != W * H {
            return None;
        }
        let span_x = self.max.x - self.min.x;
        let span_y = self.max.y - self.min.y;
        if span_x <= f32::EPSILON || span_y <= f32::EPSILON {
            return None;
        }
        let x = ((world_x - self.min.x) / span_x * W as f32).floor() as i32;
        let y = ((world_y - self.min.y) / span_y * H as f32).floor() as i32;
        let x = x.clamp(0, W as i32 - 1) as usize;
        let y = y.clamp(0, H as i32 - 1) as usize;
        self.samples.get(y * W + x).copied()
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

    fn sample_cell(&self, world_x: f32, world_y: f32) -> Option<(f32, bool)> {
        if let Ok(guard) = HOST_RADAR_MAP.lock() {
            if let Some(sample) = guard.sample(world_x, world_y) {
                return Some(sample);
            }
        }
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().try_read() {
            let height = tl.get_ground_height(world_x, world_y, None);
            let water = tl.is_underwater(world_x, world_y, None, None);
            return Some((height, water));
        }
        Some((0.0, false))
    }
}

fn pack_player_color_argb(rgb: (u8, u8, u8)) -> u32 {
    crate::game_logic::host_radar::pack_player_color_argb(rgb)
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

fn leftover_authored_radar_priority(template_name: &str) -> Option<RadarPriorityType> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    match tmpl.get_radar_priority() {
        game_engine::common::thing::thing_template::RadarPriorityType::NotOnRadar => {
            Some(RadarPriorityType::NotOnRadar)
        }
        game_engine::common::thing::thing_template::RadarPriorityType::Structure => {
            Some(RadarPriorityType::Structure)
        }
        game_engine::common::thing::thing_template::RadarPriorityType::Unit => {
            Some(RadarPriorityType::Unit)
        }
        game_engine::common::thing::thing_template::RadarPriorityType::LocalUnitOnly => {
            Some(RadarPriorityType::LocalUnitOnly)
        }
        game_engine::common::thing::thing_template::RadarPriorityType::Invalid => None,
    }
}

fn radar_priority_for_object(obj: &Object) -> RadarPriorityType {
    let mut priority = match obj.thing.template.radar_priority {
        1 => RadarPriorityType::NotOnRadar,
        2 => RadarPriorityType::Structure,
        3 => RadarPriorityType::Unit,
        4 => RadarPriorityType::LocalUnitOnly,
        _ => leftover_authored_radar_priority(&obj.template_name)
            .unwrap_or(RadarPriorityType::Invalid),
    };
    // C++ Object.cpp:6254-6267 infer only when template is INVALID.
    if priority == RadarPriorityType::Invalid {
        if obj.thing.template.garrison_contain_max.is_some() || obj.thing.template.capturable {
            priority = RadarPriorityType::Structure;
        }
    }
    // C++ Object.cpp:6270 IS_CARBOMB forces UNIT after infer.
    if obj.is_car_bomb() {
        priority = RadarPriorityType::Unit;
    }
    // Unparsed live templates have no leftover INI; keep KindOf residual.
    if priority == RadarPriorityType::Invalid {
        if obj.is_kind_of(KindOf::Structure) {
            priority = RadarPriorityType::Structure;
        } else if obj.is_kind_of(KindOf::Infantry)
            || obj.is_kind_of(KindOf::Vehicle)
            || obj.is_kind_of(KindOf::Aircraft)
            || obj.is_kind_of(KindOf::Hero)
        {
            priority = RadarPriorityType::Unit;
        }
    }
    priority
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
        radar_obj.drawable_hidden = obj.drawable_hidden || obj.hijacker_in_vehicle;
        radar_obj.hidden_by_stealth = matches!(
            crate::game_logic::host_upgrades::HostCamoStealthLook::from_u8(obj.camo_stealth_look),
            crate::game_logic::host_upgrades::HostCamoStealthLook::Invisible
        );

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
        self.host_radar_rescan_terrain();
        if let Ok(mut radar) = get_radar_system().write() {
            if !radar.try_new_map_from_source() {
                radar.new_map(lo, hi, &[]);
                let _ = radar.try_new_map_from_source();
            }
        }
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            self.host_radar_add_object(id);
        }
    }

    /// Sample leftover TerrainLogic + live height into the 128x128 radar cache.
    pub(in super::super) fn host_radar_rescan_terrain(&mut self) {
        let (lo, hi, ready) = match HOST_RADAR_MAP.lock() {
            Ok(guard) => (guard.min, guard.max, guard.ready),
            Err(_) => return,
        };
        if !ready {
            return;
        }
        let span_x = hi.x - lo.x;
        let span_y = hi.y - lo.y;
        if span_x <= f32::EPSILON || span_y <= f32::EPSILON {
            return;
        }
        const W: u32 = 128;
        const H: u32 = 128;
        let x_sample = span_x / W as f32;
        let y_sample = span_y / H as f32;
        let mut samples = Vec::with_capacity((W * H) as usize);
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        for y in 0..H {
            for x in 0..W {
                let wx = lo.x + x as f32 * x_sample;
                let wy = lo.y + y as f32 * y_sample;
                let world = glam::Vec3::new(wx, 0.0, wy);
                let mut height = self.terrain_height_at(world).unwrap_or(0.0);
                let mut water = self
                    .terrain
                    .as_ref()
                    .is_some_and(|t| t.is_underwater_at_world(world));
                if let Ok(tl) = gamelogic::terrain::get_terrain_logic().try_read() {
                    let leftover_h = tl.get_ground_height(wx, wy, None);
                    if self.terrain.is_none() {
                        height = leftover_h;
                    }
                    water = water || tl.is_underwater(wx, wy, None, None);
                }
                min_z = min_z.min(height);
                max_z = max_z.max(height);
                samples.push((height, water));
            }
        }
        if let Ok(mut guard) = HOST_RADAR_MAP.lock() {
            if min_z.is_finite() && max_z.is_finite() && max_z > min_z {
                guard.min.z = min_z;
                guard.max.z = max_z;
            }
            guard.samples = samples;
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

    #[test]
    fn host_radar_authored_not_on_radar_is_dropped() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        let mut tpl = ThingTemplate::new("Decoy");
        tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
        tpl.radar_priority = 1; // NOT_ON_RADAR
        logic.templates.insert("Decoy".into(), tpl);
        let id = logic
            .create_object_for_player("Decoy", 1, Vec3::new(2.0, 0.0, 2.0))
            .expect("spawn");
        logic.host_radar_add_object(id);
        let radar = get_radar_system().read().expect("radar");
        assert!(
            radar.get_all_objects().all(|o| o.object_id != id.0),
            "authored NOT_ON_RADAR must not leak a KindOf infantry blip"
        );
    }

    #[test]
    fn host_radar_hidden_drawable_skips_blip() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.add_player(Player::new(2, Team::China, "China", false));
        let mut tpl = ThingTemplate::new("HiddenHijacker");
        tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("HiddenHijacker".into(), tpl);
        let id = logic
            .create_object_for_player("HiddenHijacker", 2, Vec3::new(5.0, 0.0, 5.0))
            .expect("spawn");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.drawable_hidden = true;
        }
        logic.host_radar_add_object(id);
        let radar = get_radar_system().read().expect("radar");
        let blip = radar
            .get_all_objects()
            .find(|o| o.object_id == id.0)
            .expect("blip");
        assert!(
            blip.drawable_hidden && blip.is_temporarily_hidden(),
            "hijacker/script-hidden drawable must drop off the radar"
        );
    }

    #[test]
    fn host_radar_carbomb_forces_unit_priority() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        let mut tpl = ThingTemplate::new("CivCar");
        tpl.add_kind_of(KindOf::Vehicle).set_health(100.0);
        logic.templates.insert("CivCar".into(), tpl);
        let id = logic
            .create_object_for_player("CivCar", 1, Vec3::new(1.0, 0.0, 1.0))
            .expect("spawn");
        if let Some(obj) = logic.host_object_mut(id) {
            obj.apply_convert_to_car_bomb();
        }
        logic.host_radar_add_object(id);
        let radar = get_radar_system().read().expect("radar");
        let blip = radar
            .get_all_objects()
            .find(|o| o.object_id == id.0)
            .expect("blip");
        assert_eq!(blip.priority, RadarPriorityType::Unit);
    }

    #[test]
    fn host_create_radar_event_sets_last_event() {
        use crate::game_logic::host_radar::host_create_radar_event;
        use game_engine::common::system::radar::RadarEventType;
        host_create_radar_event(Vec3::new(40.0, 0.0, 80.0), RadarEventType::Construction);
        let radar = get_radar_system().read().expect("radar");
        let loc = radar.get_last_event_loc().expect("last event");
        assert!((loc.x - 40.0).abs() < 0.01);
        assert!((loc.y - 80.0).abs() < 0.01);
    }
}

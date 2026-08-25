//! Host `RailedTransportAIUpdate` residual (C++ `RailedTransportAIUpdate.cpp`).
//!
//! Leftover crate `RailedTransportAIUpdate::update` / `private_execute_railed_transport`
//! already match C++ but early-out when `OBJECT_REGISTRY` is empty (host-only).
//! This is the live path: authored `PathPrefixName` StartNN/EndNN pairs, transit
//! dock close, and `aiFollowWaypointPath` along the start-waypoint link chain.

use super::ObjectId;
use crate::game_logic::host_railroad::{HostWaypointSnap, snapshot_terrain_waypoints};
use crate::game_logic::{AIState, DockKind, GameLogic};
use glam::Vec3;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// C++ `RailedTransportAIUpdate::MAX_WAYPOINT_PATHS`.
pub const RAILED_MAX_WAYPOINT_PATHS: usize = 32;
/// C++ `INVALID_PATH`.
pub const RAILED_INVALID_PATH: i32 = -1;
/// C++ arrival threshold in `RailedTransportAIUpdate::update`.
pub const RAILED_ARRIVE_DISTANCE: f32 = 5.0;

thread_local! {
    static RAILED_WAYPOINT_OVERLAY: RefCell<Vec<HostWaypointSnap>> =
        RefCell::new(Vec::new());
}

pub fn railed_waypoint_overlay_reset() {
    RAILED_WAYPOINT_OVERLAY.with(|overlay| overlay.borrow_mut().clear());
}

/// Test / mapless inject of a named ferry waypoint (host XZ plane).
pub fn inject_railed_waypoint(snap: HostWaypointSnap) {
    RAILED_WAYPOINT_OVERLAY.with(|overlay| {
        let mut extra = overlay.borrow_mut();
        if let Some(existing) = extra
            .iter_mut()
            .find(|wp| wp.name.eq_ignore_ascii_case(&snap.name))
        {
            *existing = snap;
        } else {
            extra.push(snap);
        }
    });
}

fn all_railed_waypoints() -> Vec<HostWaypointSnap> {
    let mut snaps = snapshot_terrain_waypoints();
    RAILED_WAYPOINT_OVERLAY.with(|overlay| {
        for extra in overlay.borrow().iter() {
            if let Some(existing) = snaps
                .iter_mut()
                .find(|wp| wp.name.eq_ignore_ascii_case(&extra.name))
            {
                *existing = extra.clone();
            } else {
                snaps.push(extra.clone());
            }
        }
    });
    snaps
}

fn waypoint_name_eq(name: &str, expected: &str) -> bool {
    name.eq_ignore_ascii_case(expected)
}

fn walk_link0_positions(snaps: &[HostWaypointSnap], start_id: u32) -> Vec<Vec3> {
    let by_id: HashMap<u32, &HostWaypointSnap> = snaps.iter().map(|wp| (wp.id, wp)).collect();
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    let mut current = Some(start_id);
    while let Some(id) = current {
        if out.len() >= gamelogic::terrain::WAYPOINT_PATH_LIMIT || !visited.insert(id) {
            break;
        }
        let Some(wp) = by_id.get(&id) else {
            break;
        };
        out.push(wp.position);
        current = wp.link0;
    }
    out
}

fn load_prefix_paths(prefix: &str) -> Vec<(u32, u32)> {
    let snaps = all_railed_waypoints();
    let mut paths = vec![(0u32, 0u32); RAILED_MAX_WAYPOINT_PATHS];
    let mut num_paths = 0usize;
    for i in 0..RAILED_MAX_WAYPOINT_PATHS {
        let start_name = format!("{prefix}Start{:02}", i + 1);
        let end_name = format!("{prefix}End{:02}", i + 1);
        let start = snaps
            .iter()
            .find(|wp| waypoint_name_eq(&wp.name, &start_name));
        let end = snaps
            .iter()
            .find(|wp| waypoint_name_eq(&wp.name, &end_name));
        if let (Some(start), Some(end)) = (start, end) {
            paths[i] = (start.id, end.id);
            num_paths += 1;
        }
    }
    paths.truncate(num_paths);
    paths
}

pub fn default_railed_current_path() -> i32 {
    RAILED_INVALID_PATH
}

impl GameLogic {
    fn ensure_railed_waypoint_data(
        &self,
        prefix: &str,
        loaded: bool,
        paths: &[(u32, u32)],
    ) -> (bool, Vec<(u32, u32)>) {
        if loaded {
            (true, paths.to_vec())
        } else {
            (true, load_prefix_paths(prefix))
        }
    }

    fn set_railed_in_transit(&mut self, id: ObjectId, in_transit: bool) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_in_transit = in_transit;
        }
    }

    fn follow_railed_waypoint_id(&mut self, id: ObjectId, waypoint_id: u32) -> bool {
        let snaps = all_railed_waypoints();
        let points = walk_link0_positions(&snaps, waypoint_id);
        if points.is_empty() {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.waiting_for_path = false;
        unit.movement.current_path_index = 0;
        unit.movement.path = points;
        unit.movement.target_position = unit.movement.path.first().copied();
        unit.is_exact_path = true;
        unit.set_ultra_accurate(true);
        unit.set_ai_state(AIState::Moving);
        unit.set_status_moving(true);
        true
    }

    fn pick_and_move_to_initial_railed_location(&mut self, id: ObjectId) -> bool {
        let (pos, paths) = {
            let Some(obj) = self.objects.get(&id) else {
                return false;
            };
            (obj.get_position(), obj.railed_paths.clone())
        };
        if paths.is_empty() {
            return false;
        }
        let snaps = all_railed_waypoints();
        let mut closest_path = RAILED_INVALID_PATH;
        let mut closest_dist = f32::MAX;
        let mut closest_end_id = None;
        for (i, (_start, end_id)) in paths.iter().enumerate() {
            let Some(end) = snaps.iter().find(|wp| wp.id == *end_id) else {
                continue;
            };
            let dist = (end.position - pos).length();
            if dist < closest_dist {
                closest_dist = dist;
                closest_path = i as i32;
                closest_end_id = Some(*end_id);
            }
        }
        let Some(end_id) = closest_end_id else {
            return false;
        };
        if !self.follow_railed_waypoint_id(id, end_id) {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_current_path = closest_path;
        }
        self.set_railed_in_transit(id, true);
        true
    }

    /// C++ `RailedTransportAIUpdate::privateExecuteRailedTransport`.
    pub fn execute_railed_transport_for(&mut self, id: ObjectId) -> bool {
        let (prefix, dock_kind, loading, loaded, current, paths) = {
            let Some(obj) = self.objects.get(&id) else {
                return false;
            };
            if !obj.is_alive() {
                return false;
            }
            (
                obj.thing.template.railed_path_prefix_name.clone(),
                obj.thing.template.dock_kind,
                obj.dock_active_docker.is_some(),
                obj.railed_waypoint_data_loaded,
                obj.railed_current_path,
                obj.railed_paths.clone(),
            )
        };
        if prefix.is_empty() || dock_kind != DockKind::RailedTransport {
            return false;
        }
        if loading {
            return false;
        }
        let (loaded, paths) = self.ensure_railed_waypoint_data(&prefix, loaded, &paths);
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_waypoint_data_loaded = loaded;
            obj.railed_paths = paths.clone();
        }
        if paths.is_empty() {
            return false;
        }
        let mut next = current + 1;
        if next >= paths.len() as i32 {
            next = 0;
        }
        let start_id = paths[next as usize].0;
        if !self.follow_railed_waypoint_id(id, start_id) {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.railed_current_path = next;
        }
        self.set_railed_in_transit(id, true);
        true
    }

    /// C++ `RailedTransportAIUpdate::update`.
    pub fn update_railed_transports(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                if obj.thing.template.railed_path_prefix_name.is_empty() {
                    return None;
                }
                if obj.thing.template.dock_kind != DockKind::RailedTransport {
                    return None;
                }
                Some(*id)
            })
            .collect();

        for id in ids {
            let (prefix, loaded, paths, current, in_transit, pos, idle) = {
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                obj.set_ultra_accurate(true);
                (
                    obj.thing.template.railed_path_prefix_name.clone(),
                    obj.railed_waypoint_data_loaded,
                    obj.railed_paths.clone(),
                    obj.railed_current_path,
                    obj.railed_in_transit,
                    obj.get_position(),
                    matches!(obj.ai_state, AIState::Idle)
                        || obj.movement.path.is_empty()
                        || obj.movement.current_path_index >= obj.movement.path.len(),
                )
            };
            let (loaded, paths) = self.ensure_railed_waypoint_data(&prefix, loaded, &paths);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.railed_waypoint_data_loaded = loaded;
                obj.railed_paths = paths.clone();
            }
            if current == RAILED_INVALID_PATH && !paths.is_empty() {
                let _ = self.pick_and_move_to_initial_railed_location(id);
                continue;
            }
            if !in_transit || current < 0 {
                continue;
            }
            let Some((_, end_id)) = paths.get(current as usize) else {
                continue;
            };
            let snaps = all_railed_waypoints();
            let Some(end) = snaps.iter().find(|wp| wp.id == *end_id) else {
                continue;
            };
            let dist = (end.position - pos).length();
            if dist <= RAILED_ARRIVE_DISTANCE || idle {
                self.set_railed_in_transit(id, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{
        ContainModuleKind, ContainModuleMetadata, KindOf, Team, ThingTemplate,
    };

    fn ferry_template(name: &str, prefix: &str) -> ThingTemplate {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Transport)
            .set_health(400.0);
        t.dock_kind = DockKind::RailedTransport;
        t.railed_path_prefix_name = prefix.to_string();
        t.railed_transport_slots = Some(2);
        t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::RailedTransport,
            slots: Some(2),
            ..Default::default()
        };
        t
    }

    #[test]
    fn execute_railed_transport_follows_start_end_and_closes_dock() {
        railed_waypoint_overlay_reset();
        inject_railed_waypoint(HostWaypointSnap {
            id: 101,
            name: "ProbeFerryStart01".into(),
            position: Vec3::new(100.0, 0.0, 0.0),
            link0: Some(102),
        });
        inject_railed_waypoint(HostWaypointSnap {
            id: 102,
            name: "ProbeFerryEnd01".into(),
            position: Vec3::new(200.0, 0.0, 0.0),
            link0: None,
        });

        let mut logic = GameLogic::new();
        logic.templates.insert(
            "ProbeFerry".into(),
            ferry_template("ProbeFerry", "ProbeFerry"),
        );
        let id = logic
            .create_object("ProbeFerry", Team::USA, Vec3::ZERO)
            .expect("ferry");

        assert!(logic.execute_railed_transport_for(id));

        let ferry = logic.host_object(id).expect("ferry after execute");
        assert!(ferry.railed_in_transit);
        assert_eq!(ferry.railed_current_path, 0);
        assert_eq!(ferry.ai_state, AIState::Moving);
        assert_eq!(
            ferry.movement.path,
            vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0)]
        );
        assert!(ferry.ultra_accurate);
        railed_waypoint_overlay_reset();
    }

    #[test]
    fn railed_update_parks_at_nearest_end_then_opens_on_arrival() {
        railed_waypoint_overlay_reset();
        inject_railed_waypoint(HostWaypointSnap {
            id: 201,
            name: "ParkFerryStart01".into(),
            position: Vec3::new(0.0, 0.0, 0.0),
            link0: Some(202),
        });
        inject_railed_waypoint(HostWaypointSnap {
            id: 202,
            name: "ParkFerryEnd01".into(),
            position: Vec3::new(10.0, 0.0, 0.0),
            link0: None,
        });

        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("ParkFerry".into(), ferry_template("ParkFerry", "ParkFerry"));
        let id = logic
            .create_object("ParkFerry", Team::USA, Vec3::new(11.0, 0.0, 0.0))
            .expect("ferry");

        logic.update_railed_transports();
        {
            let ferry = logic.host_object(id).expect("parked");
            assert!(ferry.railed_in_transit);
            assert_eq!(ferry.railed_current_path, 0);
            assert_eq!(ferry.movement.path, vec![Vec3::new(10.0, 0.0, 0.0)]);
        }

        {
            let ferry = logic.host_object_mut(id).expect("arrive");
            ferry.set_position(Vec3::new(10.0, 0.0, 0.0));
            ferry.movement.path.clear();
            ferry.set_ai_state(AIState::Idle);
        }
        logic.update_railed_transports();
        let ferry = logic.host_object(id).expect("dock open");
        assert!(!ferry.railed_in_transit);
        railed_waypoint_overlay_reset();
    }
}

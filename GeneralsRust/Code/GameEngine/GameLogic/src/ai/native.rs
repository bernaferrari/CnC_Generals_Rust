//! Native Rust AI helpers: enum-based state machine + serde snapshots.
//!
//! This module is intentionally parallel to the C++-faithful AI code.
//! It is *not* wired into gameplay; it exists for comparison and future refactors.

use crate::common::{Coord3D, ObjectID};
use crate::terrain::TerrainLogic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use petgraph::graph::{Graph, NodeIndex};
use petgraph::Undirected;

pub type WaypointId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaypointNode {
    pub id: WaypointId,
    pub name: String,
    pub position: Coord3D,
    pub links: Vec<WaypointId>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WaypointGraph {
    nodes: Vec<WaypointNode>,
    by_name: HashMap<String, WaypointId>,
    by_id: HashMap<WaypointId, usize>,
    #[serde(skip)]
    petgraph_cache: Option<Graph<WaypointId, (), Undirected>>,
}

impl WaypointGraph {
    pub fn from_terrain(terrain: &TerrainLogic) -> Self {
        let mut graph = WaypointGraph::default();
        let mut current = terrain.get_first_waypoint();

        while let Some(waypoint) = current {
            let links: Vec<_> = (0..waypoint.get_num_links())
                .filter_map(|i| waypoint.get_link(i))
                .collect();

            graph.add_waypoint(WaypointNode {
                id: waypoint.get_id(),
                name: waypoint.get_name().as_str().to_string(),
                position: *waypoint.get_location(),
                links,
            });

            current = waypoint.get_next();
        }

        graph
    }

    pub fn add_waypoint(&mut self, node: WaypointNode) {
        self.by_name.insert(node.name.clone(), node.id);
        self.by_id.insert(node.id, self.nodes.len());
        self.nodes.push(node);
        self.petgraph_cache = None;
    }

    pub fn get_by_id(&self, id: WaypointId) -> Option<&WaypointNode> {
        self.by_id.get(&id).and_then(|idx| self.nodes.get(*idx))
    }

    pub fn get_by_name(&self, name: &str) -> Option<&WaypointNode> {
        self.by_name.get(name).and_then(|id| self.get_by_id(*id))
    }

    /// Link two waypoints by id (no-op if either id is missing).
    pub fn link_waypoints(&mut self, from: WaypointId, to: WaypointId) {
        let (Some(&from_idx), Some(&to_idx)) = (self.by_id.get(&from), self.by_id.get(&to)) else {
            return;
        };

        if let Some(node) = self.nodes.get_mut(from_idx) {
            if !node.links.contains(&to) {
                node.links.push(to);
            }
        }

        if let Some(node) = self.nodes.get_mut(to_idx) {
            if !node.links.contains(&from) {
                node.links.push(from);
            }
        }

        self.petgraph_cache = None;
    }

    /// Optional on-demand petgraph view for algorithms.
    /// Keeps serde storage simple while allowing graph traversal.
    pub fn build_petgraph(&self) -> Graph<WaypointId, (), Undirected> {
        Self::build_petgraph_from_nodes(&self.nodes)
    }

    /// Cached petgraph build.
    pub fn build_petgraph_cached(&mut self) -> &Graph<WaypointId, (), Undirected> {
        if self.petgraph_cache.is_none() {
            let graph = Self::build_petgraph_from_nodes(&self.nodes);
            self.petgraph_cache = Some(graph);
        }
        self.petgraph_cache
            .as_ref()
            .expect("petgraph cache should be populated")
    }

    fn build_petgraph_from_nodes(nodes: &[WaypointNode]) -> Graph<WaypointId, (), Undirected> {
        let mut graph = Graph::<WaypointId, (), Undirected>::new_undirected();
        let mut index_map: HashMap<WaypointId, NodeIndex> = HashMap::new();

        for node in nodes {
            let idx = graph.add_node(node.id);
            index_map.insert(node.id, idx);
        }

        for node in nodes {
            let Some(&src) = index_map.get(&node.id) else {
                continue;
            };
            for link in &node.links {
                if let Some(&dst) = index_map.get(link) {
                    graph.update_edge(src, dst, ());
                }
            }
        }

        graph
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeGoalContext {
    pub goal_object: Option<ObjectID>,
    pub goal_position: Option<Coord3D>,
    pub goal_waypoint: Option<WaypointId>,
    pub goal_polygon: Option<crate::polygon_trigger::PolygonTriggerId>,
    pub int_value: i32,
}

impl Default for NativeGoalContext {
    fn default() -> Self {
        Self {
            goal_object: None,
            goal_position: None,
            goal_waypoint: None,
            goal_polygon: None,
            int_value: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NativeState {
    Idle,
    MoveTo { destination: Coord3D },
    AttackMoveTo { destination: Coord3D },
    AttackObject { target_id: ObjectID },
    AttackAndFollowObject { target_id: ObjectID },
    AttackPosition { target: Coord3D },
    FollowWaypointPath { path_id: WaypointId, index: usize },
    FollowPath { path_len: usize, index: usize },
    Guard { position: Coord3D },
    Hunt,
    Wander,
    WanderInPlace { center: Coord3D },
    Panic,
    Custom(String),
}

impl NativeState {
    pub fn name(&self) -> &str {
        match self {
            NativeState::Idle => "Idle",
            NativeState::MoveTo { .. } => "MoveTo",
            NativeState::AttackMoveTo { .. } => "AttackMoveTo",
            NativeState::AttackObject { .. } => "AttackObject",
            NativeState::AttackAndFollowObject { .. } => "AttackAndFollowObject",
            NativeState::AttackPosition { .. } => "AttackPosition",
            NativeState::FollowWaypointPath { .. } => "FollowWaypointPath",
            NativeState::FollowPath { .. } => "FollowPath",
            NativeState::Guard { .. } => "Guard",
            NativeState::Hunt => "Hunt",
            NativeState::Wander => "Wander",
            NativeState::WanderInPlace { .. } => "WanderInPlace",
            NativeState::Panic => "Panic",
            NativeState::Custom(name) => name.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeTempState {
    pub state: NativeState,
    pub frame_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeStateMachine {
    pub owner_id: ObjectID,
    pub current_state: NativeState,
    pub temp_state: Option<NativeTempState>,
    pub goals: NativeGoalContext,
}

impl NativeStateMachine {
    pub fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            current_state: NativeState::Idle,
            temp_state: None,
            goals: NativeGoalContext::default(),
        }
    }

    pub fn set_state(&mut self, state: NativeState) {
        self.current_state = state;
    }

    pub fn set_temporary_state(
        &mut self,
        state: NativeState,
        frame_limit: u32,
        current_frame: u32,
    ) {
        const FRAME_COUNT_MAX: u32 = 60 * 30;
        let capped_limit = frame_limit.min(FRAME_COUNT_MAX);
        self.temp_state = Some(NativeTempState {
            state,
            frame_end: current_frame.saturating_add(capped_limit),
        });
    }

    pub fn clear_temporary_state(&mut self) {
        self.temp_state = None;
    }

    pub fn get_current_state_name(&self) -> String {
        let mut name = self.current_state.name().to_string();
        if let Some(temp) = &self.temp_state {
            name.push_str(" /T/");
            name.push_str(temp.state.name());
        }
        name
    }

    /// Minimal update loop for illustration; returns the active state name.
    pub fn update_state_machine(&mut self, current_frame: u32) -> &str {
        let use_temp = self
            .temp_state
            .as_ref()
            .map_or(false, |temp| current_frame < temp.frame_end);
        if use_temp {
            return self.temp_state.as_ref().unwrap().state.name();
        }
        self.clear_temporary_state();
        self.current_state.name()
    }

    pub fn set_goal_object(&mut self, object_id: Option<ObjectID>) {
        self.goals.goal_object = object_id;
    }

    pub fn set_goal_position(&mut self, position: Option<Coord3D>) {
        self.goals.goal_position = position;
    }

    pub fn set_goal_waypoint(&mut self, waypoint_id: Option<WaypointId>) {
        self.goals.goal_waypoint = waypoint_id;
    }

    pub fn set_goal_polygon(
        &mut self,
        polygon_id: Option<crate::polygon_trigger::PolygonTriggerId>,
    ) {
        self.goals.goal_polygon = polygon_id;
    }
}

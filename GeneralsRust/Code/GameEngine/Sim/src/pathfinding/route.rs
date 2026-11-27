//! C++ Path and PathNode state after a search accepts or composes a route.
use super::PathfindLayerEnum;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// One owned C++ PathNode. Raw predecessor/successor are adjacent in `nodes`;
/// optimized links skip raw nodes without discarding them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathRouteNode {
    pub position: Vec3,
    pub layer: Option<u8>,
    pub can_optimize: bool,
    pub next_optimized: Option<usize>,
}

/// One movement cursor on the optimized chain, using raw-node indices.
/// `current == usize::MAX` denotes no current node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PathCursor {
    previous: Option<usize>,
    current: usize,
}

impl Default for PathCursor {
    fn default() -> Self {
        Self {
            previous: None,
            current: usize::MAX,
        }
    }
}

impl PathCursor {
    pub fn previous(&self) -> Option<usize> {
        self.previous
    }
    pub fn current(&self) -> Option<usize> {
        (self.current != usize::MAX).then_some(self.current)
    }
    pub fn advance(&mut self, path: &PathRoute) {
        if let Some(current) = self.current() {
            self.previous = Some(current);
            self.current = path
                .node(current)
                .and_then(|node| node.next_optimized)
                .unwrap_or(usize::MAX);
        } else {
            self.current = match self.previous {
                Some(previous) => path.node(previous).and_then(|node| node.next_optimized),
                None => (!path.is_empty()).then_some(0),
            }
            .unwrap_or(usize::MAX);
        }
    }
}

/// A single owner for the raw path and its optimized traversal.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PathRoute {
    nodes: Vec<PathRouteNode>,
    pub blocked_by_ally: bool,
    #[serde(skip)]
    optimized_count: usize,
}

impl<'de> Deserialize<'de> for PathRoute {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct StoredPath {
            nodes: Vec<PathRouteNode>,
            blocked_by_ally: bool,
        }
        let stored = StoredPath::deserialize(deserializer)?;
        Self::from_nodes(stored.nodes, stored.blocked_by_ally)
            .ok_or_else(|| serde::de::Error::custom("invalid optimized path links"))
    }
}

impl PathRoute {
    pub fn from_coords(points: Vec<Vec3>) -> Self {
        Self::from_points(points, |_| None)
    }

    /// Build the route with the receiving grid's layer for each point.
    pub fn from_points_with_layer(points: Vec<Vec3>, mut layer: impl FnMut(Vec3) -> u8) -> Self {
        Self::from_points(points, |point| Some(layer(point)))
    }

    fn from_points(points: Vec<Vec3>, mut layer: impl FnMut(Vec3) -> Option<u8>) -> Self {
        let len = points.len();
        Self {
            nodes: points
                .into_iter()
                .enumerate()
                .map(|(i, position)| PathRouteNode {
                    position,
                    layer: layer(position),
                    can_optimize: false,
                    next_optimized: (i + 1 < len).then_some(i + 1),
                })
                .collect(),
            blocked_by_ally: false,
            optimized_count: len,
        }
    }

    pub fn from_layered(points: Vec<Vec3>, layers: Vec<u8>) -> Self {
        assert_eq!(
            points.len(),
            layers.len(),
            "each raw path node needs its layer"
        );
        let mut path = Self::from_coords(points);
        for (node, layer) in path.nodes.iter_mut().zip(layers) {
            node.layer = Some(layer);
        }
        path
    }

    /// Raw node index for an ordinal along the optimized route.
    pub fn raw_index(&self, ordinal: usize) -> Option<usize> {
        let mut index = (!self.nodes.is_empty()).then_some(0)?;
        for _ in 0..ordinal {
            index = self.nodes.get(index)?.next_optimized?;
        }
        self.nodes.get(index).map(|_| index)
    }

    pub fn len(&self) -> usize {
        self.optimized_count
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&Vec3> {
        self.raw_index(index).map(|i| &self.nodes[i].position)
    }
    pub fn first(&self) -> Option<&Vec3> {
        self.get(0)
    }
    pub fn last(&self) -> Option<&Vec3> {
        self.nodes.last().map(|node| &node.position)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Vec3> {
        std::iter::successors((!self.nodes.is_empty()).then_some(0), |&i| {
            self.nodes.get(i).and_then(|node| node.next_optimized)
        })
        .filter_map(|i| self.nodes.get(i).map(|node| &node.position))
    }
    pub fn coords(&self) -> Vec<Vec3> {
        self.iter().copied().collect()
    }
    pub fn optimized_layer(&self, ordinal: usize) -> Option<u8> {
        self.raw_index(ordinal).and_then(|i| self.nodes[i].layer)
    }
    pub fn raw_predecessor(&self, ordinal: usize) -> Option<&PathRouteNode> {
        self.raw_index(ordinal)
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| self.nodes.get(i))
    }
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.optimized_count = 0;
        self.blocked_by_ally = false;
    }
    pub fn push(&mut self, position: Vec3) {
        let index = self.nodes.len();
        if let Some(last) = self.nodes.last_mut() {
            last.next_optimized = Some(index);
        }
        self.nodes.push(PathRouteNode {
            position,
            layer: None,
            can_optimize: false,
            next_optimized: None,
        });
        self.optimized_count += 1;
    }

    pub fn nodes(&self) -> &[PathRouteNode] {
        &self.nodes
    }

    pub fn node(&self, index: usize) -> Option<&PathRouteNode> {
        self.nodes.get(index)
    }

    pub fn iter_from_raw(
        &self,
        start: Option<usize>,
    ) -> impl Iterator<Item = (usize, &PathRouteNode)> {
        std::iter::successors(start, |&index| {
            self.nodes.get(index).and_then(|node| node.next_optimized)
        })
        .filter_map(|index| self.nodes.get(index).map(|node| (index, node)))
    }

    pub fn cursor_start(&self) -> PathCursor {
        PathCursor {
            previous: None,
            current: if self.is_empty() { usize::MAX } else { 0 },
        }
    }

    pub fn cursor_at_ordinal(&self, ordinal: usize) -> PathCursor {
        let mut cursor = self.cursor_start();
        for _ in 0..ordinal {
            if cursor.current().is_none() {
                break;
            }
            cursor.advance(self);
        }
        cursor
    }

    /// C++ Path::computeFlightDistToGoal selects the first unpassed node
    /// and sums the forward projections from the saved start of the leg.
    /// Past nodes before that first forward segment can be skipped next tick.
    pub fn flight_course(&self, position: Vec3, cursor: &mut PathCursor) -> Option<(Vec3, f32)> {
        let start = cursor
            .previous()
            .or(cursor.current())
            .or_else(|| (!self.is_empty()).then_some(0));
        let mut nodes = self.iter_from_raw(start);
        let (_, first) = nodes.next()?;
        let mut goal = first.position;
        let mut previous = first.position;
        let mut remaining = 0.0;
        let mut found_forward_leg = false;
        for (index, node) in nodes {
            if !found_forward_leg {
                goal = node.position;
            }
            let direction = (node.position - previous).with_y(0.0).normalize_or_zero();
            let projection = (node.position - position).with_y(0.0).dot(direction);
            if projection >= 0.0 {
                found_forward_leg = true;
                remaining += projection;
            } else if !found_forward_leg {
                *cursor = PathCursor {
                    previous: Some(index),
                    current: node.next_optimized.unwrap_or(usize::MAX),
                };
            }
            previous = node.position;
        }
        Some((goal, remaining))
    }

    /// Validate an untrusted persisted cursor against this route.
    pub fn cursor_from_raw(
        &self,
        previous: Option<usize>,
        current: Option<usize>,
    ) -> Option<PathCursor> {
        let mut preceding = None;
        for (index, _) in self.iter_from_raw((!self.is_empty()).then_some(0)) {
            if current == Some(index) {
                return (previous == preceding).then_some(PathCursor {
                    previous,
                    current: index,
                });
            }
            if current.is_none() && previous == Some(index) {
                return Some(PathCursor {
                    previous,
                    current: usize::MAX,
                });
            }
            preceding = Some(index);
        }
        (current.is_none() && previous.is_none()).then_some(PathCursor {
            previous: None,
            current: usize::MAX,
        })
    }

    pub fn cursor_ordinal(&self, cursor: &PathCursor) -> usize {
        match cursor.current() {
            Some(current) => self
                .iter_from_raw((!self.is_empty()).then_some(0))
                .position(|(index, _)| index == current)
                .unwrap_or(self.len()),
            None => cursor.previous.map_or(0, |previous| {
                self.iter_from_raw((!self.is_empty()).then_some(0))
                    .position(|(index, _)| index == previous)
                    .map_or(self.len(), |ordinal| ordinal + 1)
            }),
        }
    }

    /// Rebuild and validate the derived optimized-node count from the links.
    pub fn from_nodes(nodes: Vec<PathRouteNode>, blocked_by_ally: bool) -> Option<Self> {
        if nodes.iter().enumerate().any(|(i, node)| {
            node.next_optimized
                .is_some_and(|next| next <= i || next >= nodes.len())
        }) {
            return None;
        }
        let mut count = 0;
        if !nodes.is_empty() {
            let mut index = 0;
            loop {
                count += 1;
                match nodes[index].next_optimized {
                    Some(next) => index = next,
                    None if index == nodes.len() - 1 => break,
                    None => return None,
                }
            }
        }
        Some(Self {
            nodes,
            blocked_by_ally,
            optimized_count: count,
        })
    }

    pub fn set_node_position(&mut self, index: usize, position: Vec3) {
        self.nodes[index].position = position;
    }
    pub fn set_node_layer(&mut self, index: usize, layer: Option<u8>) {
        self.nodes[index].layer = layer;
    }
    pub fn set_node_can_optimize(&mut self, index: usize, can_optimize: bool) {
        self.nodes[index].can_optimize = can_optimize;
    }

    /// C++ Path::optimize and Path::optimizeGroundPath. The receiving grid
    /// supplies node layers and decides whether each candidate shortcut is clear.
    /// Only the ground variant takes a grid size for its short-segment pass.
    pub fn optimize_grid_route(
        points: impl ExactSizeIterator<Item = (Vec3, u8, bool)>,
        ground_grid_size: Option<f32>,
        respect_anchors: bool,
        mut can_shortcut: impl FnMut(usize, usize, PathfindLayerEnum, bool) -> bool,
    ) -> Self {
        let len = points.len();
        let ground_path = ground_grid_size.is_some();
        let mut nodes: Vec<_> = points
            .enumerate()
            .map(|(i, (position, layer, can_optimize))| PathRouteNode {
                position,
                layer: Some(layer),
                can_optimize,
                next_optimized: (len <= 2 && i + 1 < len).then_some(i + 1),
            })
            .collect();
        if len <= 2 {
            return Self {
                nodes,
                blocked_by_ally: false,
                optimized_count: len,
            };
        }
        // C++ Path::optimize ALLOWED_STEPS = 3 past a layer change.
        const ALLOWED_STEPS: usize = 3;
        let mut optimized_count = 1;
        let mut anchor = 0usize;
        while anchor < len - 1 {
            let mut layer = PathfindLayerEnum::from_u32(nodes[anchor].layer.unwrap() as u32);
            let mut cur_layer = layer;
            let mut count = 0usize;
            let mut node_idx = anchor + 1;
            while node_idx + 1 < len {
                count += 1;
                if cur_layer == PathfindLayerEnum::Ground {
                    let node_layer =
                        PathfindLayerEnum::from_u32(nodes[node_idx].layer.unwrap() as u32);
                    if node_layer != cur_layer {
                        layer = node_layer;
                        cur_layer = layer;
                        if count > ALLOWED_STEPS {
                            break;
                        }
                    }
                } else if nodes[node_idx + 1].layer != Some(cur_layer as u8)
                    && count > ALLOWED_STEPS
                {
                    break;
                }
                cur_layer = PathfindLayerEnum::from_u32(nodes[node_idx].layer.unwrap() as u32);
                if !ground_path && respect_anchors && !nodes[node_idx].can_optimize {
                    break;
                }
                node_idx += 1;
            }
            // Path::optimize pins its first visibility check to the starting
            // layer; optimizeGroundPath deliberately does not.
            if !ground_path && anchor == 0 {
                layer = PathfindLayerEnum::from_u32(nodes[0].layer.unwrap() as u32);
            }
            let mut found = false;
            let mut far = node_idx;
            while far > anchor {
                // Uniformity uses the layers assigned to the original nodes.
                let layers_uniform = (anchor..=far).all(|i| nodes[i].layer == Some(layer as u8));
                if can_shortcut(anchor, far, layer, layers_uniform) {
                    if far == anchor {
                        break;
                    }
                    nodes[anchor].next_optimized = Some(far);
                    optimized_count += 1;
                    anchor = far;
                    found = true;
                    break;
                }
                if far == 0 {
                    break;
                }
                far -= 1;
            }
            if !found {
                nodes[anchor].next_optimized = Some(anchor + 1);
                optimized_count += 1;
                anchor += 1;
            }
        }
        if let Some(grid_size) = ground_grid_size {
            // optimizeGroundPath alone performs the jig/jog pass.
            let thresh = grid_size * grid_size * 3.9;
            let mut i = 0;
            while let Some(next) = nodes[i].next_optimized {
                let Some(after) = nodes[next].next_optimized else {
                    break;
                };
                let dx = nodes[next].position.x - nodes[i].position.x;
                let dz = nodes[next].position.z - nodes[i].position.z;
                if dx * dx + dz * dz < thresh {
                    nodes[i].next_optimized = Some(after);
                    optimized_count -= 1;
                }
                i = nodes[i].next_optimized.unwrap();
            }
        }
        Self {
            nodes,
            blocked_by_ally: false,
            optimized_count,
        }
    }

    /// Append a complete route, retaining all raw nodes and optimized links.
    pub fn append_route(&mut self, mut segment: Self) {
        if segment.nodes.is_empty() {
            return;
        }
        if self.nodes.is_empty() {
            *self = segment;
            return;
        }
        self.blocked_by_ally |= segment.blocked_by_ally;
        let duplicate = self
            .last()
            .is_some_and(|last| last.distance_squared(segment.nodes[0].position) < 0.0001);
        let skip = usize::from(duplicate);
        if duplicate && segment.nodes.len() == 1 {
            return;
        }
        let offset = self.nodes.len();
        let next = if duplicate {
            segment.nodes[0].next_optimized.map(|i| offset + i - 1)
        } else {
            Some(offset)
        };
        self.nodes.last_mut().unwrap().next_optimized = next;
        self.optimized_count += segment.optimized_count - skip;
        self.nodes.reserve(segment.nodes.len() - skip);
        for mut node in segment.nodes.drain(skip..) {
            node.next_optimized = node.next_optimized.map(|i| i + offset - skip);
            self.nodes.push(node);
        }
    }
}

impl std::ops::Index<usize> for PathRoute {
    type Output = Vec3;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("optimized path index out of bounds")
    }
}

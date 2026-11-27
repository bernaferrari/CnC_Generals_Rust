//! Authored terrain bridge geometry, ordered spans, and receiving damage state.

use crate::pathfinding::PATHFIND_CELL_SIZE_F;
use generals_game_domain::body::BodyDamageType;
use glam::{Vec2, Vec3};

/// Bridge tower type (matching C++ BridgeTowerType / TerrainRoads.h)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeTowerType {
    FromLeft = 0,
    FromRight = 1,
    ToLeft = 2,
    ToRight = 3,
}

impl BridgeTowerType {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::FromLeft),
            1 => Some(Self::FromRight),
            2 => Some(Self::ToLeft),
            3 => Some(Self::ToRight),
            _ => None,
        }
    }
}

/// Maximum number of bridge towers (C++ BRIDGE_MAX_TOWERS)
pub const BRIDGE_MAX_TOWERS: usize = 4;

/// 2D region (matching C++ Region2D)
#[derive(Debug, Clone, Copy)]
pub struct Region2D {
    pub lo: Vec2,
    pub hi: Vec2,
}

impl Default for Region2D {
    fn default() -> Self {
        Self {
            lo: Vec2::ZERO,
            hi: Vec2::ZERO,
        }
    }
}

impl Region2D {
    pub fn new(lo: Vec2, hi: Vec2) -> Self {
        Self { lo, hi }
    }

    /// C++ `Region2D::width`.
    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    /// C++ `Region2D::height`.
    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
    }
}

/// Bridge information structure
#[derive(Debug, Clone)]
pub struct BridgeInfo {
    /// The points that the bridge was drawn using
    pub from: Vec3,
    pub to: Vec3,
    /// Width of the bridge
    pub bridge_width: f32,
    /// The 4 corners of the rectangle that the bridge covers
    pub from_left: Vec3,
    pub from_right: Vec3,
    pub to_left: Vec3,
    pub to_right: Vec3,
    /// The index to the drawable bridges
    pub bridge_index: i32,
    /// Original placed-map-object index for landmark bridges, if map-authored.
    pub source_map_object_index: Option<usize>,
    /// Current damage state
    pub cur_damage_state: BodyDamageType,
    /// Associated object IDs
    pub bridge_object_id: u32,
    pub tower_object_id: [u32; BRIDGE_MAX_TOWERS],
    /// Whether damage state changed
    pub damage_state_changed: bool,
}

impl BridgeInfo {
    pub fn new() -> Self {
        Self {
            from: Vec3::ZERO,
            to: Vec3::ZERO,
            bridge_width: 0.0,
            from_left: Vec3::ZERO,
            from_right: Vec3::ZERO,
            to_left: Vec3::ZERO,
            to_right: Vec3::ZERO,
            bridge_index: -1,
            source_map_object_index: None,
            cur_damage_state: BodyDamageType::Pristine,
            bridge_object_id: 0,
            tower_object_id: [0; BRIDGE_MAX_TOWERS],
            damage_state_changed: false,
        }
    }

    pub fn from_landmark(
        position: Vec3,
        angle: f32,
        halfsize_x: f32,
        halfsize_y: f32,
        bridge_object_id: u32,
    ) -> Self {
        let c = angle.cos();
        let s = angle.sin();

        let from_left = Vec3::new(
            position.x - halfsize_x * c - halfsize_y * s,
            position.y + halfsize_y * c - halfsize_x * s,
            position.z,
        );
        let to_left = Vec3::new(
            position.x + halfsize_x * c - halfsize_y * s,
            position.y + halfsize_y * c + halfsize_x * s,
            position.z,
        );
        let from_right = Vec3::new(
            position.x - halfsize_x * c + halfsize_y * s,
            position.y - halfsize_y * c - halfsize_x * s,
            position.z,
        );
        let to_right = Vec3::new(
            position.x + halfsize_x * c + halfsize_y * s,
            position.y - halfsize_y * c + halfsize_x * s,
            position.z,
        );

        let mut bridge_info = Self::new();
        bridge_info.from_left = from_left;
        bridge_info.from_right = from_right;
        bridge_info.to_left = to_left;
        bridge_info.to_right = to_right;
        bridge_info.from = Vec3::new(
            (from_left.x + from_right.x) * 0.5,
            (from_left.y + from_right.y) * 0.5,
            (from_left.z + from_right.z) * 0.5,
        );
        bridge_info.to = Vec3::new(
            (to_left.x + to_right.x) * 0.5,
            (to_left.y + to_right.y) * 0.5,
            (to_left.z + to_right.z) * 0.5,
        );
        bridge_info.bridge_width = halfsize_y * 2.0;
        bridge_info.bridge_object_id = bridge_object_id;
        bridge_info
    }

    /// Geometry produced from the four authored drawable bridge corners.
    pub fn from_authored(
        from: Vec3,
        to: Vec3,
        width: f32,
        polygon: [Vec2; 4],
        index: i32,
    ) -> Option<Self> {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx * dx + dy * dy <= f32::EPSILON {
            return None;
        }
        let mut info = Self::new();
        info.from = from;
        info.to = to;
        info.bridge_width = width;
        info.from_left = polygon[0].extend(from.z);
        info.from_right = polygon[1].extend(from.z);
        info.to_right = polygon[2].extend(to.z);
        info.to_left = polygon[3].extend(to.z);
        info.bridge_index = index;
        Some(info)
    }

    /// Update the body state and identify entry to or exit from rubble.
    /// Callers clear the previous event at their existing frame boundary.
    pub fn set_damage_state(&mut self, state: BodyDamageType) -> Option<BodyDamageType> {
        let previous = self.cur_damage_state;
        if previous == state {
            return None;
        }
        self.cur_damage_state = state;
        self.damage_state_changed =
            previous == BodyDamageType::Rubble || state == BodyDamageType::Rubble;
        Some(previous)
    }

    /// C++ getBridgeAttackPoints offsets by half the deck width at both ends.
    pub fn attack_points(&self, attack_info: &mut BridgeAttackInfo) {
        let info = self;
        // Calculate direction vector from 'from' to 'to' (normalized)
        let mut delta = Vec3::new(
            info.to.x - info.from.x,
            info.to.y - info.from.y,
            info.to.z - info.from.z,
        );
        let delta_len = delta.length();
        if delta_len > f32::EPSILON {
            delta.x /= delta_len;
            delta.y /= delta_len;
            delta.z /= delta_len;
        }

        // Calculate width vector to get half-width offset
        let width = Vec3::new(
            info.from_right.x - info.from_left.x,
            info.from_right.y - info.from_left.y,
            info.from_right.z - info.from_left.z,
        );
        let half_width = width.length() / 2.0;

        // Attack point 1: at 'from' end, offset by half-width along bridge direction
        attack_info.attack_point1.x = info.from.x + delta.x * half_width;
        attack_info.attack_point1.y = info.from.y + delta.y * half_width;
        attack_info.attack_point1.z = info.from.z + delta.z * half_width;

        // Attack point 2: at 'to' end, offset by half-width back along bridge direction
        attack_info.attack_point2.x = info.to.x - delta.x * half_width;
        attack_info.attack_point2.y = info.to.y - delta.y * half_width;
        attack_info.attack_point2.z = info.to.z - delta.z * half_width;
    }
}

/// Bridge attack information
#[derive(Debug, Clone)]
pub struct BridgeAttackInfo {
    /// Points that can be attacked
    pub attack_point1: Vec3,
    pub attack_point2: Vec3,
}

impl BridgeAttackInfo {
    pub fn new() -> Self {
        Self {
            attack_point1: Vec3::ZERO,
            attack_point2: Vec3::ZERO,
        }
    }
}

/// Bridge class for terrain logic
#[derive(Debug)]
pub struct Bridge<Name, Layer> {
    /// Link for traversing all bridges
    next: Option<Box<Bridge<Name, Layer>>>,
    /// Bridge template name
    template_name: Name,
    /// Bridge information
    bridge_info: BridgeInfo,
    /// 2D bounds for quick screening
    bounds: Region2D,
    /// Pathfind layer for this bridge
    layer: Layer,
}

impl<Name: Clone, Layer: Copy> Clone for Bridge<Name, Layer> {
    fn clone(&self) -> Self {
        Self {
            next: None,
            template_name: self.template_name.clone(),
            bridge_info: self.bridge_info.clone(),
            bounds: self.bounds.clone(),
            layer: self.layer,
        }
    }
}

impl<Name, Layer: Copy> Bridge<Name, Layer> {
    pub fn new(bridge_info: BridgeInfo, template_name: Name, layer: Layer) -> Self {
        // Calculate bounds from bridge info
        let min_x = bridge_info
            .from_left
            .x
            .min(bridge_info.from_right.x)
            .min(bridge_info.to_left.x)
            .min(bridge_info.to_right.x);
        let max_x = bridge_info
            .from_left
            .x
            .max(bridge_info.from_right.x)
            .max(bridge_info.to_left.x)
            .max(bridge_info.to_right.x);
        let min_y = bridge_info
            .from_left
            .y
            .min(bridge_info.from_right.y)
            .min(bridge_info.to_left.y)
            .min(bridge_info.to_right.y);
        let max_y = bridge_info
            .from_left
            .y
            .max(bridge_info.from_right.y)
            .max(bridge_info.to_left.y)
            .max(bridge_info.to_right.y);

        let bounds = Region2D::new(Vec2::new(min_x, min_y), Vec2::new(max_x, max_y));

        Self {
            next: None,
            template_name,
            bridge_info,
            bounds,
            layer,
        }
    }

    /// Get bridge template name
    pub fn get_bridge_template_name(&self) -> &Name {
        &self.template_name
    }

    /// Get next bridge in list
    pub fn get_next(&self) -> Option<&Self> {
        self.next.as_ref().map(|b| b.as_ref())
    }

    /// Get height for an object on bridge
    pub fn get_bridge_height(&self, location: &Vec3, normal: Option<&mut Vec3>) -> f32 {
        let p1 = self.bridge_info.from_left;
        let p2 = self.bridge_info.from_right;
        let p3 = self.bridge_info.to_left;

        let v1 = p2 - p1;
        let v2 = p3 - p1;
        let mut n = v1.cross(v2);
        let n_len = n.length();
        if n_len <= f32::EPSILON {
            if let Some(out) = normal {
                *out = Vec3::new(0.0, 0.0, 1.0);
            }
            return p1.z;
        }

        n /= n_len;

        let z = if n.z.abs() > f32::EPSILON {
            p1.z - (n.x * (location.x - p1.x) + n.y * (location.y - p1.y)) / n.z
        } else {
            p1.z
        };

        if let Some(out) = normal {
            *out = n;
        }

        z
    }

    /// Get bridge logical info
    pub fn get_bridge_info(&self) -> &BridgeInfo {
        &self.bridge_info
    }
    pub fn bridge_info_mut(&mut self) -> &mut BridgeInfo {
        &mut self.bridge_info
    }

    /// Check if point is on bridge
    pub fn is_point_on_bridge(&self, location: &Vec3) -> bool {
        // Simple bounds check first
        if location.x < self.bounds.lo.x
            || location.x > self.bounds.hi.x
            || location.y < self.bounds.lo.y
            || location.y > self.bounds.hi.y
        {
            return false;
        }

        let p = Vec2::new(location.x, location.y);
        let quad = [
            Vec2::new(self.bridge_info.from_left.x, self.bridge_info.from_left.y),
            Vec2::new(self.bridge_info.from_right.x, self.bridge_info.from_right.y),
            Vec2::new(self.bridge_info.to_right.x, self.bridge_info.to_right.y),
            Vec2::new(self.bridge_info.to_left.x, self.bridge_info.to_left.y),
        ];

        point_in_convex_quad(&p, &quad)
    }

    /// Check if a cell region lies on the end of the bridge.
    pub fn is_cell_on_end(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.bridge_info.from_right - self.bridge_info.from_left;
        let len = end_vector.length();
        if len <= f32::EPSILON {
            return false;
        }
        end_vector /= len;
        end_vector *= PATHFIND_CELL_SIZE_F;

        let mut from_left = self.bridge_info.from_left;
        from_left.x += end_vector.x;
        from_left.y += end_vector.y;
        let mut from_right = self.bridge_info.from_right;
        from_right.x -= end_vector.x;
        from_right.y -= end_vector.y;

        let mut to_left = self.bridge_info.to_left;
        to_left.x += end_vector.x;
        to_left.y += end_vector.y;
        let mut to_right = self.bridge_info.to_right;
        to_right.x -= end_vector.x;
        to_right.y -= end_vector.y;

        let line1 = Vec2::new(from_left.x, from_left.y);
        let line2 = Vec2::new(from_right.x, from_right.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        let line1 = Vec2::new(to_left.x, to_left.y);
        let line2 = Vec2::new(to_right.x, to_right.y);
        line_in_region(&line1, &line2, cell)
    }

    /// Get layer
    pub fn get_layer(&self) -> Layer {
        self.layer
    }

    /// Set layer
    pub fn set_layer(&mut self, layer: Layer) {
        self.layer = layer;
    }

    /// Get bounds
    pub fn get_bounds(&self) -> &Region2D {
        &self.bounds
    }

    /// Set bridge object ID
    pub fn set_bridge_object_id(&mut self, id: u32) {
        self.bridge_info.bridge_object_id = id;
    }

    /// Set tower object ID
    /// C++ `Bridge::setToweru32` indexes `toweru32[which]`.
    pub fn set_tower_object_id(&mut self, id: u32, which: BridgeTowerType) {
        let index = which as usize;
        if index < self.bridge_info.tower_object_id.len() {
            self.bridge_info.tower_object_id[index] = id;
        }
    }

    /// Check if a cell region lies on the side of the bridge.
    /// Reference: C++ TerrainLogic.cpp Bridge::isCellOnSide()
    ///
    /// This is used to determine if a pathfinding cell touches the sides
    /// of a bridge, which affects pathfinding calculations.
    pub fn is_cell_on_side(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.bridge_info.from_right - self.bridge_info.from_left;
        let len = end_vector.length();
        if len <= f32::EPSILON {
            return false;
        }
        end_vector /= len;
        // Offset by 0.51 pathfind cells for side detection
        end_vector *= PATHFIND_CELL_SIZE_F * 0.51;

        let mut from_left = self.bridge_info.from_left;
        from_left.x -= end_vector.x;
        from_left.y -= end_vector.y;

        let mut from_right = self.bridge_info.from_right;
        from_right.x += end_vector.x;
        from_right.y += end_vector.y;

        let mut to_left = self.bridge_info.to_left;
        to_left.x -= end_vector.x;
        to_left.y -= end_vector.y;

        let mut to_right = self.bridge_info.to_right;
        to_right.x += end_vector.x;
        to_right.y += end_vector.y;

        // Check left side of bridge
        let line1 = Vec2::new(from_left.x, from_left.y);
        let line2 = Vec2::new(to_left.x, to_left.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        // Check right side of bridge
        let line1 = Vec2::new(from_right.x, from_right.y);
        let line2 = Vec2::new(to_right.x, to_right.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        // Check with additional offset for wider detection
        from_left.x -= end_vector.x;
        from_left.y -= end_vector.y;
        from_right.x += end_vector.x;
        from_right.y += end_vector.y;
        to_left.x -= end_vector.x;
        to_left.y -= end_vector.y;
        to_right.x += end_vector.x;
        to_right.y += end_vector.y;

        let line1 = Vec2::new(from_left.x, from_left.y);
        let line2 = Vec2::new(to_left.x, to_left.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        let line1 = Vec2::new(from_right.x, from_right.y);
        let line2 = Vec2::new(to_right.x, to_right.y);
        line_in_region(&line1, &line2, cell)
    }

    /// Check if a pathfind cell is an entry point to the bridge.
    /// Reference: C++ TerrainLogic.cpp Bridge::isCellEntryPoint()
    ///
    /// Entry points are the areas at either end of the bridge where
    /// units can transition onto the bridge surface.
    pub fn is_cell_entry_point(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.bridge_info.from_right - self.bridge_info.from_left;
        let len = end_vector.length();
        if len <= f32::EPSILON {
            return false;
        }
        end_vector /= len;
        // Offset by 1 pathfind cell
        end_vector *= PATHFIND_CELL_SIZE_F;

        let mut bridge_vector = self.bridge_info.to - self.bridge_info.from;
        let bridge_len = bridge_vector.length();
        if bridge_len <= f32::EPSILON {
            return false;
        }
        bridge_vector /= bridge_len;
        // Offset by half a pathfind cell along bridge direction
        bridge_vector *= PATHFIND_CELL_SIZE_F * 0.5;

        // Calculate entry point at 'from' end
        let mut from_left = self.bridge_info.from_left;
        from_left.x -= bridge_vector.x;
        from_left.y -= bridge_vector.y;
        from_left.x += end_vector.x;
        from_left.y += end_vector.y;

        let mut from_right = self.bridge_info.from_right;
        from_right.x -= bridge_vector.x;
        from_right.y -= bridge_vector.y;
        from_right.x -= end_vector.x;
        from_right.y -= end_vector.y;

        // Check 'from' entry point
        let line1 = Vec2::new(from_left.x, from_left.y);
        let line2 = Vec2::new(from_right.x, from_right.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        // Calculate entry point at 'to' end
        let mut to_left = self.bridge_info.to_left;
        to_left.x += bridge_vector.x;
        to_left.y += bridge_vector.y;
        to_left.x += end_vector.x;
        to_left.y += end_vector.y;

        let mut to_right = self.bridge_info.to_right;
        to_right.x += bridge_vector.x;
        to_right.y += bridge_vector.y;
        to_right.x -= end_vector.x;
        to_right.y -= end_vector.y;

        // Check 'to' entry point
        let line1 = Vec2::new(to_left.x, to_left.y);
        let line2 = Vec2::new(to_right.x, to_right.y);
        line_in_region(&line1, &line2, cell)
    }
}

fn point_in_convex_quad(point: &Vec2, quad: &[Vec2; 4]) -> bool {
    let mut has_positive = false;
    let mut has_negative = false;

    for edge in 0..4 {
        let a = quad[edge];
        let b = quad[(edge + 1) % 4];
        let cross = cross_2d(&a, &b, point);
        if cross > 1.0e-5 {
            has_positive = true;
        } else if cross < -1.0e-5 {
            has_negative = true;
        }

        if has_positive && has_negative {
            return false;
        }
    }

    true
}

fn cross_2d(a: &Vec2, b: &Vec2, p: &Vec2) -> f32 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = p.x - a.x;
    let ap_y = p.y - a.y;
    ab_x * ap_y - ab_y * ap_x
}

fn line_in_region(line1: &Vec2, line2: &Vec2, region: &Region2D) -> bool {
    // Liang-Barsky clipping for axis-aligned rectangle.
    let x0 = line1.x;
    let y0 = line1.y;
    let x1 = line2.x;
    let y1 = line2.y;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;

    let clip = |p: f32, q: f32, t0: &mut f32, t1: &mut f32| -> bool {
        if p.abs() <= f32::EPSILON {
            return q >= 0.0;
        }
        let r = q / p;
        if p < 0.0 {
            if r > *t1 {
                return false;
            }
            if r > *t0 {
                *t0 = r;
            }
        } else {
            if r < *t0 {
                return false;
            }
            if r < *t1 {
                *t1 = r;
            }
        }
        true
    };

    let left = region.lo.x;
    let right = region.hi.x;
    let top = region.lo.y;
    let bottom = region.hi.y;

    if !clip(-dx, x0 - left, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dx, right - x0, &mut t0, &mut t1) {
        return false;
    }
    if !clip(-dy, y0 - top, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dy, bottom - y0, &mut t0, &mut t1) {
        return false;
    }

    t0 <= t1
}

/// The receiving terrain's bridge list. A detached span does not retain its list tail.
#[derive(Debug)]
pub struct BridgeList<Name, Layer> {
    head: Option<Box<Bridge<Name, Layer>>>,
    damage_states_changed: bool,
}

impl<Name, Layer> Default for BridgeList<Name, Layer> {
    fn default() -> Self {
        Self {
            head: None,
            damage_states_changed: false,
        }
    }
}

impl<Name, Layer: Copy> BridgeList<Name, Layer> {
    pub fn clear(&mut self) {
        self.head = None;
    }

    pub fn first(&self) -> Option<&Bridge<Name, Layer>> {
        self.head.as_deref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Bridge<Name, Layer>> {
        std::iter::successors(self.first(), |bridge| bridge.get_next())
    }

    pub fn for_each_mut(&mut self, mut visit: impl FnMut(&mut Bridge<Name, Layer>)) {
        let mut current = self.head.as_deref_mut();
        while let Some(bridge) = current {
            visit(bridge);
            current = bridge.next.as_deref_mut();
        }
    }

    pub fn find_mut(
        &mut self,
        mut matches: impl FnMut(&Bridge<Name, Layer>) -> bool,
    ) -> Option<&mut Bridge<Name, Layer>> {
        let mut current = self.head.as_deref_mut();
        while let Some(bridge) = current {
            if matches(bridge) {
                return Some(bridge);
            }
            current = bridge.next.as_deref_mut();
        }
        None
    }

    pub fn prepend(&mut self, mut bridge: Box<Bridge<Name, Layer>>, layer: Layer) {
        bridge.set_layer(layer);
        bridge.next = self.head.take();
        self.head = Some(bridge);
    }

    pub fn detach_first(
        &mut self,
        mut matches: impl FnMut(&Bridge<Name, Layer>) -> bool,
    ) -> Option<Box<Bridge<Name, Layer>>> {
        let mut link = &mut self.head;
        loop {
            if matches(link.as_deref()?) {
                let mut removed = link.take()?;
                *link = removed.next.take();
                self.damage_states_changed = true;
                return Some(removed);
            }
            link = &mut link.as_mut()?.next;
        }
    }

    /// C++ TerrainLogic::loadPostProcess removes spans whose actor no longer exists.
    pub fn remove_orphans(&mut self, mut has_object: impl FnMut(u32) -> bool) -> Vec<Layer> {
        let mut layers = Vec::new();
        let mut link = &mut self.head;
        while let Some(bridge) = link.as_ref() {
            if has_object(bridge.get_bridge_info().bridge_object_id) {
                link = &mut link.as_mut().expect("bridge present").next;
            } else {
                layers.push(bridge.get_layer());
                *link = link.take().expect("bridge present").next.take();
            }
        }
        self.damage_states_changed |= !layers.is_empty();
        layers
    }

    pub fn damage_states_changed(&self) -> bool {
        self.damage_states_changed
    }

    pub fn begin_damage_frame(&mut self) {
        self.damage_states_changed = false;
    }

    /// The completed host damage scan publishes its result even if unchanged.
    pub fn finish_damage_scan(&mut self) {
        self.damage_states_changed = true;
    }

    pub fn write_damage_state(&mut self, object_id: u32, state: BodyDamageType) -> bool {
        if object_id == 0 {
            return false;
        }
        let mut changed = false;
        self.for_each_mut(|bridge| {
            let info = bridge.bridge_info_mut();
            if info.bridge_object_id == object_id {
                changed |= info.set_damage_state(state).is_some();
            }
        });
        self.damage_states_changed |= changed;
        changed
    }

    /// One receiving actor observation. The driving world applies its effects
    /// before advancing to the next bridge.
    pub fn observe_body(
        &mut self,
        object_id: u32,
        state: Option<BodyDamageType>,
    ) -> Option<(u32, BodyDamageType, BodyDamageType)> {
        let mut transition = None;
        self.for_each_mut(|bridge| {
            let info = bridge.bridge_info_mut();
            if info.bridge_object_id != object_id {
                return;
            }
            info.damage_state_changed = false;
            if info.bridge_object_id == 0 {
                return;
            }
            let Some(state) = state else {
                info.bridge_object_id = 0;
                return;
            };
            if let Some(old) = info.set_damage_state(state) {
                transition = Some((info.bridge_object_id, old, state));
            }
        });
        transition
    }

    /// Resume between effects. The cloned receipt contains only the visited span.
    pub fn next_damage<Effect>(
        &mut self,
        visited: &mut usize,
        mut observe: impl FnMut(&mut Bridge<Name, Layer>) -> Option<Effect>,
    ) -> Option<(Effect, Bridge<Name, Layer>)>
    where
        Name: Clone,
    {
        let mut current = self.head.as_deref_mut();
        for _ in 0..*visited {
            current = current.and_then(|bridge| bridge.next.as_deref_mut());
        }
        while let Some(bridge) = current {
            *visited += 1;
            let effect = observe(bridge);
            self.damage_states_changed |= bridge.get_bridge_info().damage_state_changed;
            if let Some(effect) = effect {
                return Some((effect, bridge.clone()));
            }
            current = bridge.next.as_deref_mut();
        }
        None
    }
}

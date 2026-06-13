//! CPU-side parity port for C++ `W3DDevice/GameClient/W3DTreeBuffer.cpp`.

use glam::{Mat4, Vec2, Vec3};

pub const MAX_TREE_VERTEX: usize = 30_000;
pub const MAX_TREE_INDEX: usize = 60_000;
pub const MAX_TREES: usize = 4000;
pub const MAX_TYPES: usize = 64;
pub const MAX_TILES: usize = 512;
pub const NUM_SWAY_ENTRIES: usize = 100;
pub const MAX_SWAY_TYPES: usize = 10;
pub const MAX_BUFFERS: usize = 1;
pub const SORT_ITERATIONS_PER_FRAME: usize = 10;
pub const PARTITION_WIDTH_HEIGHT: usize = 100;
pub const END_OF_PARTITION: i16 = -1;
pub const DELETED_TREE_TYPE: i32 = -2;
pub const TREE_RADIUS_APPROX: f32 = 7.0;
pub const CONSTRUCTION_TREE_COLLISION_RADIUS: f32 = 2.0 * TREE_RADIUS_APPROX;
pub const W3D_TOPPLE_OPTIONS_NONE: u32 = 0x0000_0000;
pub const W3D_TOPPLE_OPTIONS_NO_BOUNCE: u32 = 0x0000_0001;
pub const W3D_TOPPLE_OPTIONS_NO_FX: u32 = 0x0000_0002;
pub const ANGULAR_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - std::f32::consts::PI / 64.0;

/// C++ `W3DToppleState`, including the typoed `TOPPPLE_SHROUDED` slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum W3DToppleState {
    Upright = 0,
    Falling = 1,
    Fogged = 2,
    Shrouded = 3,
    Down = 4,
}

/// Minimal C++ `Region2D` equivalent used by the tree partition grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeRegion2D {
    pub lo: Vec2,
    pub hi: Vec2,
}

impl Default for TreeRegion2D {
    fn default() -> Self {
        Self {
            lo: Vec2::ZERO,
            hi: Vec2::ONE,
        }
    }
}

impl TreeRegion2D {
    pub fn new(lo: Vec2, hi: Vec2) -> Self {
        Self { lo, hi }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Default for TreeSphere {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 1.0,
        }
    }
}

/// C++ `W3DTreeDrawModuleData` subset consumed by `W3DTreeBuffer`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeModuleData {
    pub model_name: String,
    pub texture_name: String,
    pub frames_to_move_outward: u32,
    pub frames_to_move_inward: u32,
    pub max_outward_movement: f32,
    pub darkening: f32,
    pub initial_velocity_percent: f32,
    pub initial_accel_percent: f32,
    pub bounce_velocity_percent: f32,
    pub minimum_topple_speed: f32,
    pub kill_when_toppled: bool,
    pub do_topple: bool,
    pub sink_frames: u32,
    pub sink_distance: f32,
    pub do_shadow: bool,
}

impl Default for TreeModuleData {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            texture_name: String::new(),
            frames_to_move_outward: 1,
            frames_to_move_inward: 1,
            max_outward_movement: 1.0,
            darkening: 0.0,
            initial_velocity_percent: 0.2,
            initial_accel_percent: 0.01,
            bounce_velocity_percent: 0.3,
            minimum_topple_speed: 0.5,
            kill_when_toppled: true,
            do_topple: false,
            sink_frames: 10 * 30,
            sink_distance: 20.0,
            do_shadow: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeTypeInfo {
    pub data: TreeModuleData,
    pub bounds: TreeSphere,
    pub texture_origin: (i32, i32),
    pub num_tiles: i32,
    pub first_tile: i32,
    pub tile_width: i32,
    pub half_tile: bool,
    pub offset: Vec3,
    pub shadow_size: f32,
    pub do_shadow: bool,
}

impl TreeTypeInfo {
    fn from_module(data: TreeModuleData, bounds: TreeSphere) -> Self {
        Self {
            shadow_size: bounds.radius * 2.0,
            do_shadow: data.do_shadow,
            data,
            bounds,
            texture_origin: (0, 0),
            num_tiles: 0,
            first_tile: 0,
            tile_width: 0,
            half_tile: false,
            offset: Vec3::ZERO,
        }
    }
}

/// C++ `TTree`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub location: Vec3,
    pub scale: f32,
    pub sin: f32,
    pub cos: f32,
    pub tree_type: i32,
    pub visible: bool,
    pub bounds: TreeSphere,
    pub sort_key: f32,
    pub drawable_id: u32,
    pub push_aside: f32,
    pub push_aside_delta: f32,
    pub push_aside_sin: f32,
    pub push_aside_cos: f32,
    pub push_aside_source: u32,
    pub last_frame_updated: u32,
    pub next_in_partition: i16,
    pub sway_type: i32,
    pub first_index: i32,
    pub buffer_ndx: i32,
    pub angular_velocity: f32,
    pub angular_acceleration: f32,
    pub topple_direction: Vec3,
    pub topple_state: W3DToppleState,
    pub angular_accumulation: f32,
    pub options: u32,
    pub matrix: Mat4,
    pub sink_frames_left: u32,
}

impl Default for TreeEntry {
    fn default() -> Self {
        Self {
            location: Vec3::ZERO,
            scale: 1.0,
            sin: 0.0,
            cos: 1.0,
            tree_type: DELETED_TREE_TYPE,
            visible: false,
            bounds: TreeSphere::default(),
            sort_key: 0.0,
            drawable_id: 0,
            push_aside: 0.0,
            push_aside_delta: 0.0,
            push_aside_sin: 1.0,
            push_aside_cos: 1.0,
            push_aside_source: u32::MAX,
            last_frame_updated: 0,
            next_in_partition: END_OF_PARTITION,
            sway_type: 0,
            first_index: 0,
            buffer_ndx: -1,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Vec3::ZERO,
            topple_state: W3DToppleState::Upright,
            angular_accumulation: 0.0,
            options: W3D_TOPPLE_OPTIONS_NONE,
            matrix: Mat4::IDENTITY,
            sink_frames_left: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeShroudStatus {
    Clear,
    Fogged,
    Shrouded,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreezeInfo {
    pub breeze_version: i32,
    pub lean: f32,
    pub intensity: f32,
    pub direction_vec: Vec2,
    pub randomness: f32,
    pub breeze_period: i32,
}

impl Default for BreezeInfo {
    fn default() -> Self {
        Self {
            breeze_version: 0,
            lean: 0.0,
            intensity: 0.0,
            direction_vec: Vec2::X,
            randomness: 0.0,
            breeze_period: NUM_SWAY_ENTRIES as i32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TreeGeometryType {
    Cylinder,
    Box,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeCollisionUnit {
    pub object_id: u32,
    pub position: Vec3,
    pub direction_2d: Vec2,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub geometry_type: TreeGeometryType,
    pub crusher_level: i32,
    pub immobile: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeConstructionGeometry {
    pub position: Vec3,
    pub major_radius: f32,
    pub minor_radius: f32,
    pub geometry_type: TreeGeometryType,
    pub angle: f32,
}

impl TreeConstructionGeometry {
    fn collides_with_tree_cylinder(&self, tree_position: Vec3) -> bool {
        let dx = tree_position.x - self.position.x;
        let dy = tree_position.y - self.position.y;
        if self.geometry_type == TreeGeometryType::Box {
            let (sin, cos) = self.angle.sin_cos();
            let local_x = dx * cos + dy * sin;
            let local_y = -dx * sin + dy * cos;
            let half_y = self.minor_radius;
            let closest_x = local_x.clamp(-self.major_radius, self.major_radius);
            let closest_y = local_y.clamp(-half_y, half_y);
            let delta_x = local_x - closest_x;
            let delta_y = local_y - closest_y;
            delta_x * delta_x + delta_y * delta_y
                <= CONSTRUCTION_TREE_COLLISION_RADIUS * CONSTRUCTION_TREE_COLLISION_RADIUS
        } else {
            let radius = self.major_radius + CONSTRUCTION_TREE_COLLISION_RADIUS;
            dx * dx + dy * dy <= radius * radius
        }
    }
}

/// Snapshot record order used by C++ `W3DTreeBuffer::xfer`.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSaveRecord {
    pub model_name: String,
    pub model_texture: String,
    pub location: Vec3,
    pub scale: f32,
    pub sin: f32,
    pub cos: f32,
    pub drawable_id: u32,
    pub angular_velocity: f32,
    pub angular_acceleration: f32,
    pub topple_direction: Vec3,
    pub topple_state: W3DToppleState,
    pub angular_accumulation: f32,
    pub options: u32,
    pub matrix: Mat4,
    pub sink_frames_left: u32,
}

#[derive(Debug, Clone)]
pub struct W3DTreeBuffer {
    area_partition: Vec<i16>,
    bounds: TreeRegion2D,
    trees: Vec<TreeEntry>,
    tree_types: Vec<TreeTypeInfo>,
    anything_changed: bool,
    any_push_changed: bool,
    update_all_keys: bool,
    initialized: bool,
    is_terrain_pass: bool,
    need_to_update_texture: bool,
    num_tiles: i32,
    camera_look_at_vector: Vec3,
    sway_offsets: [Vec3; NUM_SWAY_ENTRIES],
    cur_sway_version: i32,
    cur_sway_offset: [f32; MAX_SWAY_TYPES],
    cur_sway_step: [f32; MAX_SWAY_TYPES],
    cur_sway_factor: [f32; MAX_SWAY_TYPES],
    cur_num_tree_vertices: [i32; MAX_BUFFERS],
    cur_num_tree_indices: [i32; MAX_BUFFERS],
}

impl Default for W3DTreeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DTreeBuffer {
    pub fn new() -> Self {
        let mut buffer = Self {
            area_partition: vec![END_OF_PARTITION; PARTITION_WIDTH_HEIGHT * PARTITION_WIDTH_HEIGHT],
            bounds: TreeRegion2D::default(),
            trees: Vec::new(),
            tree_types: Vec::new(),
            anything_changed: false,
            any_push_changed: false,
            update_all_keys: false,
            initialized: false,
            is_terrain_pass: false,
            need_to_update_texture: false,
            num_tiles: 0,
            camera_look_at_vector: Vec3::ZERO,
            sway_offsets: [Vec3::ZERO; NUM_SWAY_ENTRIES],
            cur_sway_version: -1,
            cur_sway_offset: [0.0; MAX_SWAY_TYPES],
            cur_sway_step: [0.0; MAX_SWAY_TYPES],
            cur_sway_factor: [0.0; MAX_SWAY_TYPES],
            cur_num_tree_vertices: [0; MAX_BUFFERS],
            cur_num_tree_indices: [0; MAX_BUFFERS],
        };
        buffer.clear_all_trees();
        buffer.initialized = true;
        buffer.cur_sway_version = -1;
        buffer
    }

    pub fn trees(&self) -> &[TreeEntry] {
        &self.trees
    }

    pub fn tree_mut(&mut self, index: usize) -> Option<&mut TreeEntry> {
        self.trees.get_mut(index)
    }

    pub fn tree_types(&self) -> &[TreeTypeInfo] {
        &self.tree_types
    }

    pub fn area_partition(&self) -> &[i16] {
        &self.area_partition
    }

    pub fn bounds(&self) -> TreeRegion2D {
        self.bounds
    }

    pub fn anything_changed(&self) -> bool {
        self.anything_changed
    }

    pub fn any_push_changed(&self) -> bool {
        self.any_push_changed
    }

    pub fn update_all_keys(&self) -> bool {
        self.update_all_keys
    }

    pub fn camera_look_at_vector(&self) -> Vec3 {
        self.camera_look_at_vector
    }

    pub fn need_to_update_texture(&self) -> bool {
        self.need_to_update_texture
    }

    pub fn set_bounds(&mut self, bounds: TreeRegion2D) {
        self.bounds = bounds;
    }

    pub fn set_is_terrain(&mut self) {
        self.is_terrain_pass = true;
    }

    pub fn need_to_draw(&self) -> bool {
        self.is_terrain_pass
    }

    pub fn do_full_update(&mut self) {
        self.update_all_keys = true;
    }

    pub fn cull_trees(
        &mut self,
        camera_look_at_vector: Vec3,
        mut is_visible: impl FnMut(&TreeSphere) -> bool,
    ) {
        self.camera_look_at_vector = camera_look_at_vector;
        for tree in &mut self.trees {
            let mut update_key = false;
            let visible = is_visible(&tree.bounds);
            if visible != tree.visible {
                tree.visible = visible;
                self.anything_changed = true;
                if visible {
                    update_key = true;
                }
            }
            if update_key || (visible && self.update_all_keys) {
                tree.sort_key = tree.location.dot(self.camera_look_at_vector);
            }
        }
        self.update_all_keys = false;
    }

    pub fn clear_all_trees(&mut self) {
        self.trees.clear();
        self.bounds = TreeRegion2D::default();
        self.cur_num_tree_indices[0] = 0;
        self.anything_changed = true;
        self.area_partition.fill(END_OF_PARTITION);
        self.tree_types.clear();
        self.num_tiles = 0;
        self.need_to_update_texture = false;
    }

    pub fn add_tree_type(&mut self, data: TreeModuleData, bounds: TreeSphere) -> Option<usize> {
        if self.tree_types.len() >= MAX_TYPES {
            return None;
        }
        self.need_to_update_texture = true;
        self.tree_types
            .push(TreeTypeInfo::from_module(data, bounds));
        Some(self.tree_types.len() - 1)
    }

    pub fn add_tree(
        &mut self,
        drawable_id: u32,
        location: Vec3,
        scale: f32,
        angle: f32,
        random_scale_amount: f32,
        data: TreeModuleData,
        base_bounds: TreeSphere,
    ) -> Option<usize> {
        let mut rng = DeterministicTreeRandom;
        self.add_tree_randomized(
            drawable_id,
            location,
            scale,
            angle,
            random_scale_amount,
            data,
            base_bounds,
            &mut rng,
        )
    }

    pub fn add_tree_randomized(
        &mut self,
        drawable_id: u32,
        location: Vec3,
        scale: f32,
        angle: f32,
        random_scale_amount: f32,
        data: TreeModuleData,
        base_bounds: TreeSphere,
        rng: &mut impl TreeRandom,
    ) -> Option<usize> {
        if self.trees.len() >= MAX_TREES || !self.initialized {
            return None;
        }

        let tree_type = self
            .tree_types
            .iter()
            .position(|existing| {
                existing
                    .data
                    .model_name
                    .eq_ignore_ascii_case(&data.model_name)
                    && existing
                        .data
                        .texture_name
                        .eq_ignore_ascii_case(&data.texture_name)
            })
            .or_else(|| self.add_tree_type(data.clone(), base_bounds))?;

        let random_scale = rng.real_range(1.0 - random_scale_amount, 1.0 + random_scale_amount);
        let final_scale = if random_scale_amount > 0.0 {
            scale * random_scale
        } else {
            scale
        };
        let mut entry = TreeEntry {
            location,
            scale: final_scale,
            sin: angle.sin(),
            cos: angle.cos(),
            tree_type: tree_type as i32,
            visible: false,
            drawable_id,
            first_index: 0,
            buffer_ndx: -1,
            sway_type: rng.int_range(0, MAX_SWAY_TYPES as i32 - 1),
            push_aside: 0.0,
            last_frame_updated: 0,
            push_aside_source: u32::MAX,
            push_aside_delta: 0.0,
            push_aside_cos: 1.0,
            push_aside_sin: 1.0,
            topple_state: W3DToppleState::Upright,
            ..TreeEntry::default()
        };
        entry.bounds = self.scaled_bounds(tree_type, location, entry.scale);

        if data.frames_to_move_outward > 2 || data.do_topple {
            let bucket = self.get_partition_bucket(location) as usize;
            entry.next_in_partition = self.area_partition[bucket];
            self.area_partition[bucket] = self.trees.len() as i16;
        }

        self.trees.push(entry);
        Some(self.trees.len() - 1)
    }

    pub fn update_tree_position(&mut self, drawable_id: u32, location: Vec3, angle: f32) -> bool {
        for i in 0..self.trees.len() {
            if self.trees[i].drawable_id == drawable_id {
                self.trees[i].location = location;
                self.trees[i].sin = angle.sin();
                self.trees[i].cos = angle.cos();
                let tree_type = self.trees[i].tree_type as usize;
                self.trees[i].bounds = self.scaled_bounds(tree_type, location, self.trees[i].scale);
                self.anything_changed = true;
                return true;
            }
        }
        false
    }

    pub fn remove_tree(&mut self, drawable_id: u32) {
        for tree in &mut self.trees {
            if tree.drawable_id == drawable_id {
                tree.location = Vec3::ZERO;
                tree.tree_type = DELETED_TREE_TYPE;
                tree.bounds = TreeSphere::default();
                self.anything_changed = true;
            }
        }
    }

    pub fn remove_trees_for_construction(&mut self, geom: TreeConstructionGeometry) {
        for tree in &mut self.trees {
            if tree.tree_type < 0 {
                continue;
            }
            if geom.collides_with_tree_cylinder(tree.location) {
                tree.tree_type = DELETED_TREE_TYPE;
                self.anything_changed = true;
            }
        }
    }

    pub fn get_partition_bucket(&self, pos: Vec3) -> i32 {
        let mut x = pos.x;
        let mut y = pos.y;
        if x < self.bounds.lo.x {
            x = self.bounds.lo.x;
        }
        if y < self.bounds.lo.y {
            y = self.bounds.lo.y;
        }
        if x > self.bounds.hi.x {
            x = self.bounds.hi.x;
        }
        if y > self.bounds.hi.y {
            y = self.bounds.hi.y;
        }
        let x_index = ((x / (self.bounds.hi.x - self.bounds.lo.x))
            * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
            .floor() as i32;
        let y_index = ((y / (self.bounds.hi.y - self.bounds.lo.y))
            * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
            .floor() as i32;
        y_index * PARTITION_WIDTH_HEIGHT as i32 + x_index
    }

    pub fn push_aside_tree(
        &mut self,
        drawable_id: u32,
        pusher_pos: Vec3,
        pusher_direction: Vec2,
        pusher_id: u32,
        frame: u32,
    ) {
        for tree in &mut self.trees {
            if tree.drawable_id != drawable_id {
                continue;
            }
            let last_frame = tree.last_frame_updated;
            tree.last_frame_updated = frame;
            if tree.push_aside_source == pusher_id && tree.last_frame_updated - last_frame < 3 {
                return;
            }
            if tree.push_aside != 0.0 {
                return;
            }
            tree.push_aside_source = pusher_id;
            let delta = tree.location - pusher_pos;
            if pusher_direction.x * delta.y - pusher_direction.y * delta.x > 0.0 {
                tree.push_aside_cos = -pusher_direction.y;
                tree.push_aside_sin = pusher_direction.x;
            } else {
                tree.push_aside_cos = pusher_direction.y;
                tree.push_aside_sin = -pusher_direction.x;
            }
            self.any_push_changed = true;
            let tree_type = tree.tree_type as usize;
            let outward = self.tree_types[tree_type]
                .data
                .frames_to_move_outward
                .max(1);
            tree.push_aside_delta = 1.0 / outward as f32;
        }
    }

    pub fn unit_moved(&mut self, unit: TreeCollisionUnit, frame: u32) {
        if unit.immobile {
            return;
        }
        let mut radius = unit.major_radius;
        if unit.geometry_type == TreeGeometryType::Box && radius > unit.minor_radius {
            radius = unit.minor_radius;
        }
        radius += TREE_RADIUS_APPROX;

        let (x_min, y_min) = self.partition_min_indices(unit.position, radius);
        let (x_max, y_max) = self.partition_max_indices(unit.position, radius);
        for x in x_min..x_max {
            for y in y_min..y_max {
                let bucket = x + PARTITION_WIDTH_HEIGHT as i32 * y;
                let mut tree_ndx = self.area_partition[bucket as usize];
                while tree_ndx != END_OF_PARTITION {
                    let index = tree_ndx as usize;
                    if index >= self.trees.len() {
                        break;
                    }
                    tree_ndx = self.trees[index].next_in_partition;
                    if self.trees[index].tree_type < 0 {
                        continue;
                    }
                    let delta = self.trees[index].location - unit.position;
                    if radius * radius <= delta.length_squared() {
                        continue;
                    }
                    let tree_type = self.trees[index].tree_type as usize;
                    if unit.crusher_level > 1 && self.tree_types[tree_type].data.do_topple {
                        let topple_vector = Vec3::new(
                            self.trees[index].location.x - unit.position.x,
                            self.trees[index].location.y - unit.position.y,
                            0.0,
                        );
                        self.apply_toppling_force_by_index(
                            index,
                            topple_vector,
                            0.0,
                            W3D_TOPPLE_OPTIONS_NONE,
                        );
                    } else if self.tree_types[tree_type].data.frames_to_move_outward > 1 {
                        let drawable_id = self.trees[index].drawable_id;
                        self.push_aside_tree(
                            drawable_id,
                            unit.position,
                            unit.direction_2d,
                            unit.object_id,
                            frame,
                        );
                    }
                }
            }
        }
    }

    pub fn apply_toppling_force(
        &mut self,
        drawable_id: u32,
        topple_direction: Vec3,
        topple_speed: f32,
        options: u32,
    ) -> bool {
        let Some(index) = self
            .trees
            .iter()
            .position(|tree| tree.drawable_id == drawable_id)
        else {
            return false;
        };
        self.apply_toppling_force_by_index(index, topple_direction, topple_speed, options)
    }

    pub fn update_toppling_tree(&mut self, index: usize, shroud: TreeShroudStatus) {
        if index >= self.trees.len()
            || self.trees[index].topple_state == W3DToppleState::Upright
            || self.trees[index].topple_state == W3DToppleState::Down
        {
            return;
        }

        let data = self.tree_types[self.trees[index].tree_type as usize]
            .data
            .clone();
        if shroud == TreeShroudStatus::Fogged {
            self.trees[index].topple_state = W3DToppleState::Fogged;
            return;
        }
        if self.trees[index].topple_state == W3DToppleState::Fogged {
            self.trees[index].angular_velocity = 0.0;
            self.trees[index].topple_state = W3DToppleState::Down;
            pre_rotate_topple_matrix(&mut self.trees[index], ANGULAR_LIMIT);
            self.trees[index].angular_accumulation = ANGULAR_LIMIT;
            if data.kill_when_toppled {
                self.trees[index].sink_frames_left = 0;
            }
            return;
        }

        const VELOCITY_BOUNCE_LIMIT: f32 = 0.01;
        let mut cur_vel_to_use = self.trees[index].angular_velocity;
        if self.trees[index].angular_accumulation + cur_vel_to_use > ANGULAR_LIMIT {
            cur_vel_to_use = ANGULAR_LIMIT - self.trees[index].angular_accumulation;
        }
        pre_rotate_topple_matrix(&mut self.trees[index], cur_vel_to_use);
        self.trees[index].angular_accumulation += cur_vel_to_use;
        if self.trees[index].angular_accumulation >= ANGULAR_LIMIT
            && self.trees[index].angular_velocity > 0.0
        {
            self.trees[index].angular_velocity *= -data.bounce_velocity_percent;
            if self.trees[index].options & W3D_TOPPLE_OPTIONS_NO_BOUNCE != 0
                || self.trees[index].angular_velocity.abs() < VELOCITY_BOUNCE_LIMIT
            {
                self.trees[index].angular_velocity = 0.0;
                self.trees[index].topple_state = W3DToppleState::Down;
                if data.kill_when_toppled {
                    self.trees[index].sink_frames_left = data.sink_frames;
                }
            }
        } else {
            self.trees[index].angular_velocity += self.trees[index].angular_acceleration;
        }
    }

    pub fn tick_cpu(&mut self, pause: bool, shroud: impl Fn(&TreeEntry) -> TreeShroudStatus) {
        self.is_terrain_pass = false;
        if pause {
            return;
        }
        for index in 0..self.trees.len() {
            let tree_type = self.trees[index].tree_type;
            if tree_type < 0 {
                continue;
            }
            match self.trees[index].topple_state {
                W3DToppleState::Falling | W3DToppleState::Fogged => {
                    let status = shroud(&self.trees[index]);
                    self.update_toppling_tree(index, status);
                }
                W3DToppleState::Down => {
                    let data = &self.tree_types[tree_type as usize].data;
                    if data.kill_when_toppled {
                        if self.trees[index].sink_frames_left == 0 {
                            self.trees[index].tree_type = DELETED_TREE_TYPE;
                            self.anything_changed = true;
                        }
                        self.trees[index].sink_frames_left =
                            self.trees[index].sink_frames_left.wrapping_sub(1);
                        self.trees[index].location.z -=
                            data.sink_distance / data.sink_frames.max(1) as f32;
                        set_matrix_translation(&mut self.trees[index]);
                    }
                }
                _ if self.trees[index].push_aside_delta != 0.0 => {
                    self.trees[index].push_aside += self.trees[index].push_aside_delta;
                    let data = &self.tree_types[tree_type as usize].data;
                    if self.trees[index].push_aside >= 1.0 {
                        self.trees[index].push_aside_delta =
                            -1.0 / data.frames_to_move_inward.max(1) as f32;
                    } else if self.trees[index].push_aside <= 0.0 {
                        self.trees[index].push_aside_delta = 0.0;
                        self.trees[index].push_aside = 0.0;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn update_sway(&mut self, info: BreezeInfo, rng: &mut impl TreeRandom) {
        for i in 0..NUM_SWAY_ENTRIES {
            let factor =
                (i as f32 * 2.0 * std::f32::consts::PI / (NUM_SWAY_ENTRIES as f32 + 1.0)).cos();
            let angle = info.lean + info.intensity * factor;
            let s = angle.sin();
            let c = angle.cos();
            self.sway_offsets[i] =
                Vec3::new(info.direction_vec.x * s, info.direction_vec.y * s, c - 1.0);
        }

        let delta = info.randomness * 0.5;
        for tree in &mut self.trees {
            tree.sway_type = 1 + rng.int_range(0, MAX_SWAY_TYPES as i32 - 1);
        }
        for i in 0..MAX_SWAY_TYPES {
            self.cur_sway_step[i] = NUM_SWAY_ENTRIES as f32 / info.breeze_period as f32;
            self.cur_sway_step[i] *= rng.real_range(1.0 - delta, 1.0 + delta);
            if self.cur_sway_step[i] < 0.0 {
                self.cur_sway_step[i] = 0.0;
            }
            self.cur_sway_offset[i] = 0.0;
            self.cur_sway_factor[i] = rng.real_range(1.0 - delta, 1.0 + delta);
        }
        self.cur_sway_version = info.breeze_version;
    }

    pub fn save_records(&self) -> Vec<TreeSaveRecord> {
        self.trees
            .iter()
            .map(|tree| {
                let (model_name, model_texture) = if tree.tree_type != DELETED_TREE_TYPE {
                    let tree_type = &self.tree_types[tree.tree_type as usize];
                    (
                        tree_type.data.model_name.clone(),
                        tree_type.data.texture_name.clone(),
                    )
                } else {
                    (String::new(), String::new())
                };
                TreeSaveRecord {
                    model_name,
                    model_texture,
                    location: tree.location,
                    scale: tree.scale,
                    sin: tree.sin,
                    cos: tree.cos,
                    drawable_id: tree.drawable_id,
                    angular_velocity: tree.angular_velocity,
                    angular_acceleration: tree.angular_acceleration,
                    topple_direction: tree.topple_direction,
                    topple_state: tree.topple_state,
                    angular_accumulation: tree.angular_accumulation,
                    options: tree.options,
                    matrix: tree.matrix,
                    sink_frames_left: tree.sink_frames_left,
                }
            })
            .collect()
    }

    pub fn load_records(&mut self, records: &[TreeSaveRecord]) {
        self.trees.clear();
        self.area_partition.fill(END_OF_PARTITION);
        self.cur_num_tree_vertices = [0; MAX_BUFFERS];
        self.cur_num_tree_indices = [0; MAX_BUFFERS];

        for record in records {
            let Some(tree_type) = self.tree_types.iter().position(|existing| {
                existing
                    .data
                    .model_name
                    .eq_ignore_ascii_case(&record.model_name)
                    && existing
                        .data
                        .texture_name
                        .eq_ignore_ascii_case(&record.model_texture)
            }) else {
                continue;
            };
            let data = self.tree_types[tree_type].data.clone();
            let base_bounds = self.tree_types[tree_type].bounds;
            let Some(index) = self.add_tree(
                record.drawable_id,
                record.location,
                record.scale,
                0.0,
                0.0,
                data,
                base_bounds,
            ) else {
                continue;
            };
            let tree = &mut self.trees[index];
            tree.angular_acceleration = record.angular_acceleration;
            tree.angular_velocity = record.angular_velocity;
            tree.topple_direction = record.topple_direction;
            tree.topple_state = record.topple_state;
            tree.options = record.options;
            tree.matrix = record.matrix;
            tree.sink_frames_left = record.sink_frames_left;
        }
        self.anything_changed = true;
    }

    fn scaled_bounds(&self, tree_type: usize, location: Vec3, scale: f32) -> TreeSphere {
        let base = self.tree_types[tree_type].bounds;
        TreeSphere {
            center: base.center * scale + location,
            radius: base.radius * scale,
        }
    }

    fn apply_toppling_force_by_index(
        &mut self,
        index: usize,
        topple_direction: Vec3,
        mut topple_speed: f32,
        options: u32,
    ) -> bool {
        if self.trees[index].topple_state != W3DToppleState::Upright {
            return false;
        }
        let tree_type = self.trees[index].tree_type as usize;
        let data = &self.tree_types[tree_type].data;
        if topple_speed < data.minimum_topple_speed {
            topple_speed = data.minimum_topple_speed;
        }
        let direction = topple_direction.normalize_or_zero();
        self.trees[index].topple_direction = direction;
        self.trees[index].angular_accumulation = 0.0;
        self.trees[index].angular_velocity = topple_speed * data.initial_velocity_percent;
        self.trees[index].angular_acceleration = topple_speed * data.initial_accel_percent;
        self.trees[index].topple_state = W3DToppleState::Falling;
        self.trees[index].options = options;
        self.any_push_changed = true;
        self.trees[index].matrix = Mat4::from_translation(self.trees[index].location);
        true
    }

    fn partition_min_indices(&self, pos: Vec3, radius: f32) -> (i32, i32) {
        let mut x = (pos.x - radius).clamp(self.bounds.lo.x, self.bounds.hi.x);
        let mut y = (pos.y - radius).clamp(self.bounds.lo.y, self.bounds.hi.y);
        if x.is_nan() {
            x = self.bounds.lo.x;
        }
        if y.is_nan() {
            y = self.bounds.lo.y;
        }
        (
            ((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .floor() as i32,
            ((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .floor() as i32,
        )
    }

    fn partition_max_indices(&self, pos: Vec3, radius: f32) -> (i32, i32) {
        let mut x = (pos.x + radius).clamp(self.bounds.lo.x, self.bounds.hi.x);
        let mut y = (pos.y + radius).clamp(self.bounds.lo.y, self.bounds.hi.y);
        if x.is_nan() {
            x = self.bounds.hi.x;
        }
        if y.is_nan() {
            y = self.bounds.hi.y;
        }
        (
            ((x / (self.bounds.hi.x - self.bounds.lo.x)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .ceil() as i32,
            ((y / (self.bounds.hi.y - self.bounds.lo.y)) * (PARTITION_WIDTH_HEIGHT as f32 - 0.1))
                .ceil() as i32,
        )
    }
}

fn pre_rotate_topple_matrix(tree: &mut TreeEntry, angle: f32) {
    tree.matrix = Mat4::from_rotation_x(-angle * tree.topple_direction.y) * tree.matrix;
    tree.matrix = Mat4::from_rotation_y(angle * tree.topple_direction.x) * tree.matrix;
}

fn set_matrix_translation(tree: &mut TreeEntry) {
    tree.matrix.w_axis = tree.location.extend(1.0);
}

pub trait TreeRandom {
    fn int_range(&mut self, min: i32, max: i32) -> i32;
    fn real_range(&mut self, min: f32, max: f32) -> f32;
}

struct DeterministicTreeRandom;

impl TreeRandom for DeterministicTreeRandom {
    fn int_range(&mut self, min: i32, _max: i32) -> i32 {
        min
    }

    fn real_range(&mut self, min: f32, max: f32) -> f32 {
        (min + max) * 0.5
    }
}

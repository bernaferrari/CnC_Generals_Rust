// Partition manager object queries and find-position helpers
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

pub struct ThePartitionManager;

/// FindPosition options for ThePartitionManager::find_position_around_with_options.
#[derive(Debug, Clone)]
pub struct FindPositionOptions {
    pub min_radius: Real,
    pub max_radius: Real,
    pub start_angle: Option<Real>,
    pub max_z_delta: Real,
    pub flags: u32,
    pub relationship_object_id: Option<ObjectID>,
    pub ignore_object_id: Option<ObjectID>,
    pub source_to_path_to_dest_id: Option<ObjectID>,
}

impl Default for FindPositionOptions {
    fn default() -> Self {
        Self {
            min_radius: 0.0,
            max_radius: 0.0,
            start_angle: None,
            max_z_delta: 99999.0,
            flags: 0,
            relationship_object_id: None,
            ignore_object_id: None,
            source_to_path_to_dest_id: None,
        }
    }
}

pub const FPF_NONE: u32 = 0x00;
pub const FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS: u32 = 0x01;
pub const FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES: u32 = 0x02;
pub const FPF_IGNORE_ENEMY_UNITS: u32 = 0x04;
pub const FPF_IGNORE_ENEMY_STRUCTURES: u32 = 0x08;
pub const FPF_IGNORE_ALL_OBJECTS: u32 = 0x10;
pub const FPF_IGNORE_WATER: u32 = 0x20;
pub const FPF_WATER_ONLY: u32 = 0x40;
pub const FPF_CLEAR_CELLS_ONLY: u32 = 0x80;
pub const FPF_USE_HIGHEST_LAYER: u32 = 0x100;

#[derive(Debug, Default)]
pub struct ThePartitionManagerBridge;

impl ThePartitionManager {
    pub fn get() -> Option<&'static Self> {
        static PARTITION: OnceLock<ThePartitionManager> = OnceLock::new();
        Some(PARTITION.get_or_init(|| ThePartitionManager))
    }

    /// Get objects in range.
    ///
    /// C++ uses `ThePartitionManager->iterateObjectsInRange(...)` for most radius queries.
    /// The Rust port does not yet have a single unified partition system, so we bridge through
    /// `ObjectManager`'s spatial partition and then validate against live objects in the registry.
    pub fn get_objects_in_range(
        &self,
        pos: &Coord3D,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        // Wave 281: empty dual-world → no objects.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let candidate_ids = if let Ok(logic) = crate::system::game_logic::get_game_logic().lock() {
            logic
                .partition_manager()
                .find_objects_in_radius(*pos, radius)
        } else {
            let manager_ref = crate::object_manager::get_object_manager();
            let Ok(manager) = manager_ref.read() else {
                return Vec::new();
            };
            manager.find_objects_in_radius(*pos, radius)
        };

        let radius_sqr = radius * radius;
        candidate_ids
            .into_iter()
            .filter_map(|id| {
                OBJECT_REGISTRY
                    .with_object(id, |obj_guard| {
                        let obj_pos = obj_guard.get_position();
                        let dx = obj_pos.x - pos.x;
                        let dy = obj_pos.y - pos.y;
                        if dx * dx + dy * dy <= radius_sqr {
                            Some(id)
                        } else {
                            None
                        }
                    })
                    .flatten()
            })
            .collect()
    }

    /// C++ `PartitionManager::iteratePotentialCollisions`.
    /// Uses the query geometry's 2D bounding circle (not the 3D sphere).
    pub fn iterate_potential_collisions(
        &self,
        pos: &Coord3D,
        geometry: &GeometryInfo,
        _orientation: Real,
    ) -> Vec<crate::common::ObjectID> {
        let radius = geometry.get_bounding_circle_radius().max(1.0);
        self.get_objects_in_range(pos, radius)
    }

    /// Find a legal position around a point (matching C++ PartitionManager::findPositionAround).
    pub fn find_position_around(
        &self,
        center: &Coord3D,
        min_radius: Real,
        max_radius: Real,
        result: &mut Coord3D,
    ) -> bool {
        let mut options = FindPositionOptions::default();
        options.min_radius = min_radius;
        options.max_radius = max_radius;
        self.find_position_around_with_options(center, &options, result)
    }

    /// Full FindPositionAround implementation with options (closer to C++).
    pub fn find_position_around_with_options(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
        result: &mut Coord3D,
    ) -> bool {
        // Wave 281: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        const RING_SPACING: Real = 5.0;
        const TWO_PI: Real = std::f32::consts::PI * 2.0;

        fn try_position<F>(
            center: &Coord3D,
            dist: Real,
            angle: Real,
            terrain: Option<&TheTerrainLogic>,
            in_extent: &F,
            options: &FindPositionOptions,
            result: &mut Coord3D,
        ) -> bool
        where
            F: Fn(&Coord3D) -> bool,
        {
            let mut pos = Coord3D::new(
                center.x + dist * angle.cos(),
                center.y + dist * angle.sin(),
                center.z,
            );

            if !in_extent(&pos) {
                return false;
            }

            if let Some(terrain) = terrain {
                let mut layer = PathfindLayerEnum::Ground;
                if (options.flags & FPF_USE_HIGHEST_LAYER) != 0 {
                    pos.z = 99999.0;
                    layer = terrain.get_highest_layer_for_destination(&pos);
                    pos.z = terrain.get_layer_height(pos.x, pos.y, layer);
                    if layer != PathfindLayerEnum::Ground {
                        pos.z += 1.0;
                    }
                } else {
                    pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                }

                if (pos.z - center.z).abs() > options.max_z_delta {
                    return false;
                }

                if terrain.is_cliff_cell(pos.x, pos.y) {
                    return false;
                }

                if (options.flags & FPF_IGNORE_WATER) == 0 {
                    let underwater = terrain.is_underwater(pos.x, pos.y, None, None);
                    if (options.flags & FPF_WATER_ONLY) != 0 {
                        if !underwater {
                            return false;
                        }
                    } else if underwater {
                        return false;
                    }
                }

                if (options.flags & FPF_CLEAR_CELLS_ONLY) != 0 {
                    let ai_store = crate::ai::the_ai(); if let Ok(ai) = ai_store.read() {
                        if let Some(ps) = ai.pathfinding_system() {
                            if let Ok(ps_guard) = ps.read() {
                                if !ps_guard.is_cell_clear_at(&pos, layer) {
                                    return false;
                                }
                            }
                        }
                    }
                }
            }

            if (options.flags & FPF_IGNORE_ALL_OBJECTS) == 0 {
                let relation_id = options.relationship_object_id;

                // Host path: empty dual-world registry — no object residual for path find.
                if !OBJECT_REGISTRY.is_empty() {
                    for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
                        let obj_arc = match OBJECT_REGISTRY.get_object(obj_id) {
                            Some(v) => v,
                            None => continue,
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            continue;
                        };
                        let obj_id = obj_guard.get_id();

                        if options.ignore_object_id == Some(obj_id) {
                            continue;
                        }
                        if options.source_to_path_to_dest_id == Some(obj_id) {
                            continue;
                        }

                        if let Some(rel_id) = relation_id {
                            let should_skip = OBJECT_REGISTRY
                                .with_object(rel_id, |rel_guard| {
                                    let relation = rel_guard.relationship_to(&obj_guard);
                                    let is_unit = obj_guard.is_kind_of(KindOf::Infantry)
                                        || obj_guard.is_kind_of(KindOf::Vehicle);
                                    let is_structure = obj_guard.is_kind_of(KindOf::Structure);

                                    if (options.flags & FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS) != 0
                                        && relation != Relationship::Enemies
                                        && is_unit
                                    {
                                        return true;
                                    }
                                    if (options.flags & FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES) != 0
                                        && relation != Relationship::Enemies
                                        && is_structure
                                    {
                                        return true;
                                    }
                                    if (options.flags & FPF_IGNORE_ENEMY_UNITS) != 0
                                        && relation == Relationship::Enemies
                                        && is_unit
                                    {
                                        return true;
                                    }
                                    if (options.flags & FPF_IGNORE_ENEMY_STRUCTURES) != 0
                                        && relation == Relationship::Enemies
                                        && is_structure
                                    {
                                        return true;
                                    }
                                    false
                                })
                                .unwrap_or(false);
                            if should_skip {
                                continue;
                            }
                        }

                        let obj_pos = obj_guard.get_position();
                        let dx = obj_pos.x - pos.x;
                        let dy = obj_pos.y - pos.y;
                        let radius =
                            obj_guard.get_geometry_info().get_bounding_circle_radius() + 5.0;
                        if dx * dx + dy * dy <= radius * radius {
                            return false;
                        }
                    }
                }
            }

            if let Some(source_id) = options.source_to_path_to_dest_id {
                if let Some(source_pos) = OBJECT_REGISTRY
                    .with_object(source_id, |source_guard| *source_guard.get_position())
                {
                    if let Some(terrain) = terrain {
                        if !terrain.is_clear_line_of_sight(&source_pos, &pos) {
                            return false;
                        }
                    }
                }
            }

            *result = pos;
            true
        }

        let terrain = TheTerrainLogic::get();
        let extent = terrain
            .map(|t| t.get_maximum_pathfind_extent())
            .unwrap_or_else(|| crate::common::Region3D::new(*center, *center));

        let in_extent = |pos: &Coord3D| {
            pos.x >= extent.lo.x
                && pos.x <= extent.hi.x
                && pos.y >= extent.lo.y
                && pos.y <= extent.hi.y
        };

        if !in_extent(center) {
            *result = *center;
            return true;
        }

        if (options.flags & FPF_IGNORE_WATER) != 0 && (options.flags & FPF_WATER_ONLY) != 0 {
            return false;
        }

        let max_radius = if options.max_radius < options.min_radius {
            options.min_radius
        } else {
            options.max_radius
        };
        let start_angle = options
            .start_angle
            .unwrap_or_else(|| GameLogicRandomValueReal!(0.0, TWO_PI));

        let mut dist = options.min_radius;
        while dist <= max_radius {
            let angle_spacing = if dist == options.min_radius {
                TWO_PI
            } else {
                (RING_SPACING / (dist + 1.0)) * (TWO_PI / 6.0)
            };

            let samples = ((TWO_PI / angle_spacing) / 2.0).ceil() as i32;
            for i in 0..samples {
                let angle_offset = angle_spacing * i as f32;
                if try_position(
                    center,
                    dist,
                    start_angle + angle_offset,
                    terrain,
                    &in_extent,
                    options,
                    result,
                ) {
                    return true;
                }
                if i != 0
                    && try_position(
                        center,
                        dist,
                        start_angle - angle_offset,
                        terrain,
                        &in_extent,
                        options,
                        result,
                    )
                {
                    return true;
                }
            }

            dist += RING_SPACING;
        }

        false
    }

    /// Get objects in range using boundary-to-boundary distance in 2D.
    ///
    /// Mirrors C++ `FROM_BOUNDINGSPHERE_2D` distance calculation when the query
    /// is a position (no source object).
    pub fn get_objects_in_range_boundary_2d(
        &self,
        pos: &Coord3D,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        // Wave 281: empty dual-world → no objects.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let radius_sqr = radius * radius;
        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj = obj_arc.read().ok()?;
                let obj_pos = obj.get_position();
                let dx = obj_pos.x - pos.x;
                let dy = obj_pos.y - pos.y;
                let center_dist = (dx * dx + dy * dy).sqrt();
                let obj_radius = obj.get_geometry_info().get_bounding_circle_radius();
                let boundary_dist = if center_dist <= obj_radius {
                    0.0
                } else {
                    center_dist - obj_radius
                };
                if boundary_dist * boundary_dist <= radius_sqr {
                    Some(obj.get_id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get objects in range using boundary-to-boundary distance in 3D.
    ///
    /// Mirrors C++ `FROM_BOUNDINGSPHERE_3D` distance calculation when the query
    /// is a position (no source object).
    pub fn get_objects_in_range_boundary_3d(
        &self,
        pos: &Coord3D,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        // Wave 281: empty dual-world → no objects.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let radius_sqr = radius * radius;
        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj = obj_arc.read().ok()?;
                let obj_pos = obj.get_position();
                let geom = obj.get_geometry_info();
                let center_z_delta = (geom.bounds.min.z + geom.bounds.max.z) * 0.5;
                let dx = obj_pos.x - pos.x;
                let dy = obj_pos.y - pos.y;
                let dz = (obj_pos.z + center_z_delta) - pos.z;
                let center_dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let obj_radius = geom.get_bounding_sphere_radius();
                let boundary_dist = if center_dist <= obj_radius {
                    0.0
                } else {
                    center_dist - obj_radius
                };
                if boundary_dist * boundary_dist <= radius_sqr {
                    Some(obj.get_id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get objects in range using boundary-to-boundary distance in 3D from a source object.
    ///
    /// Mirrors C++ `iterateObjectsInRange(source, radius, FROM_BOUNDINGSPHERE_3D, ...)`.
    pub fn get_objects_in_range_boundary_3d_from_object(
        &self,
        source: &crate::object::Object,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        // Wave 281: empty dual-world → no objects.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let radius_sqr = radius * radius;
        let source_pos = source.get_position();
        let source_geom = source.get_geometry_info();
        let source_center_z = (source_geom.bounds.min.z + source_geom.bounds.max.z) * 0.5;
        let source_radius = source_geom.get_bounding_sphere_radius();

        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj = obj_arc.read().ok()?;
                let obj_pos = obj.get_position();
                let geom = obj.get_geometry_info();
                let center_z_delta = (geom.bounds.min.z + geom.bounds.max.z) * 0.5;
                let dx = obj_pos.x - source_pos.x;
                let dy = obj_pos.y - source_pos.y;
                let dz = (obj_pos.z + center_z_delta) - (source_pos.z + source_center_z);
                let center_dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let obj_radius = geom.get_bounding_sphere_radius();
                let combined_radius = source_radius + obj_radius;
                let boundary_dist = if center_dist <= combined_radius {
                    0.0
                } else {
                    center_dist - combined_radius
                };
                if boundary_dist * boundary_dist <= radius_sqr {
                    Some(obj.get_id())
                } else {
                    None
                }
            })
            .collect()
    }
    /// Get the closest object in range that satisfies a filter
    /// C++ Reference: PartitionManager::getClosestObject
    pub fn get_closest_object<F>(
        &self,
        pos: &Coord3D,
        radius: Real,
        mut filter: F,
    ) -> Option<ObjectID>
    where
        F: FnMut(&crate::object::Object) -> bool,
    {
        let candidate_ids = self.get_objects_in_range(pos, radius);
        let mut closest_id = None;
        let mut min_dist_sqr = radius * radius + 1.0; // Plus 1 to ensure we pick up objects exactly on the radius if needed

        for id in candidate_ids {
            let _ = OBJECT_REGISTRY.with_object(id, |obj| {
                if filter(obj) {
                    let obj_pos = obj.get_position();
                    let dist_sqr = pos.distance_squared(*obj_pos);
                    if dist_sqr < min_dist_sqr {
                        min_dist_sqr = dist_sqr;
                        closest_id = Some(id);
                    }
                }
            });
        }
        closest_id
    }

    /// Get the closest object in range using 2D distance that satisfies a filter.
    /// Mirrors C++ `FROM_CENTER_2D` selection for closest-object queries.
    pub fn get_closest_object_2d<F>(
        &self,
        pos: &Coord3D,
        radius: Real,
        mut filter: F,
    ) -> Option<ObjectID>
    where
        F: FnMut(&crate::object::Object) -> bool,
    {
        let candidate_ids = self.get_objects_in_range(pos, radius);
        let mut closest_id = None;
        let mut min_dist_sqr = radius * radius + 1.0;

        for id in candidate_ids {
            let _ = OBJECT_REGISTRY.with_object(id, |obj| {
                if filter(obj) {
                    let obj_pos = obj.get_position();
                    let dx = obj_pos.x - pos.x;
                    let dy = obj_pos.y - pos.y;
                    let dist_sqr = dx * dx + dy * dy;
                    if dist_sqr < min_dist_sqr {
                        min_dist_sqr = dist_sqr;
                        closest_id = Some(id);
                    }
                }
            });
        }
        closest_id
    }

    /// Get distance squared between two objects
    /// Matches C++ PartitionManager::getDistanceSquared
    pub fn get_distance_squared(
        obj1: &crate::object::Object,
        obj2: &crate::object::Object,
        flags: DistanceType,
    ) -> Real {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_3D, FROM_EDGE_2D};

        let pos1 = obj1.get_position();
        let pos2 = obj2.get_position();

        if flags == FROM_CENTER_3D {
            return pos1.distance_squared(*pos2);
        }

        let dx = pos1.x - pos2.x;
        let dy = pos1.y - pos2.y;
        let center_dist = (dx * dx + dy * dy).sqrt();

        if flags == FROM_BOUNDING_SPHERE_2D || flags == FROM_EDGE_2D {
            let radius_sum = obj1.get_geometry_info().get_bounding_circle_radius()
                + obj2.get_geometry_info().get_bounding_circle_radius();
            let boundary_dist = if center_dist <= radius_sum {
                0.0
            } else {
                center_dist - radius_sum
            };
            boundary_dist * boundary_dist
        } else {
            dx * dx + dy * dy
        }
    }

    /// Get distance squared between object and position
    /// Matches C++ PartitionManager::getDistanceSquared (position variant)
    pub fn get_distance_squared_to_pos(
        obj: &crate::object::Object,
        pos: &Coord3D,
        flags: DistanceType,
    ) -> Real {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_3D, FROM_EDGE_2D};

        let obj_pos = obj.get_position();

        if flags == FROM_CENTER_3D {
            return obj_pos.distance_squared(*pos);
        }

        let dx = obj_pos.x - pos.x;
        let dy = obj_pos.y - pos.y;
        let center_dist = (dx * dx + dy * dy).sqrt();

        if flags == FROM_BOUNDING_SPHERE_2D || flags == FROM_EDGE_2D {
            let radius = obj.get_geometry_info().get_bounding_circle_radius();
            let boundary_dist = if center_dist <= radius {
                0.0
            } else {
                center_dist - radius
            };
            boundary_dist * boundary_dist
        } else {
            dx * dx + dy * dy
        }
    }

    /// Estimate terrain extremes along a line (matching C++ PartitionManager::estimateTerrainExtremesAlongLine).
    /// Returns false if the line travels off-map.
    pub fn estimate_terrain_extremes_along_line(
        &self,
        start: Coord3D,
        end: Coord3D,
        highest: &mut Real,
    ) -> bool {
        let Some(terrain) = TheTerrainLogic::get() else {
            return true;
        };

        let extent = terrain.get_maximum_pathfind_extent();
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let step = crate::common::MAP_XY_FACTOR.max(1.0);
        let steps = ((dist / step).ceil() as i32).max(1);
        let mut max_height = f32::MIN;

        for i in 0..=steps {
            let t = (i as f32) / (steps as f32);
            let x = start.x + dx * t;
            let y = start.y + dy * t;
            if x < extent.lo.x || x > extent.hi.x || y < extent.lo.y || y > extent.hi.y {
                return false;
            }
            let z = terrain.get_ground_height(x, y, None);
            if z > max_height {
                max_height = z;
            }
        }

        *highest = if max_height.is_finite() {
            max_height
        } else {
            0.0
        };
        true
    }

    /// Mirrors C++ PartitionManager::getShroudStatusForPlayer(playerIndex, loc).
    /// Returns `CELLSHROUD_SHROUDED` for invalid players or unknown cells.
    pub fn get_shroud_status_for_player(
        &self,
        player_index: Int,
        loc: &Coord3D,
    ) -> game_engine::common::system::radar::CellShroudStatus {
        use crate::system::shroud_manager::ShroudState;
        use game_engine::common::system::radar::CellShroudStatus;

        // C++: if (playerIndex < 0) return CELLSHROUD_SHROUDED;
        if player_index < 0 {
            return CellShroudStatus::Shrouded;
        }

        let Ok(shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return CellShroudStatus::Shrouded;
        };
        match shroud.get_shroud_state(player_index as u32, loc) {
            ShroudState::Visible => CellShroudStatus::Clear,
            ShroudState::Explored => CellShroudStatus::Fogged,
            ShroudState::Hidden => CellShroudStatus::Shrouded,
        }
    }

    /// Mirrors C++ ThePartitionManager->doShroudReveal().
    pub fn do_shroud_reveal(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_shroud_reveal(center, radius, player_mask.bits());
        drop(shroud);
        crate::object::stamp_partition_cell_lookers(center, radius, player_mask.bits(), true);
    }

    /// Mirrors C++ ThePartitionManager->undoShroudReveal().
    pub fn undo_shroud_reveal(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_shroud_reveal(center, radius, player_mask.bits());
        drop(shroud);
        crate::object::stamp_partition_cell_lookers(center, radius, player_mask.bits(), false);
    }

    /// Mirrors C++ ThePartitionManager->queueUndoShroudReveal().
    pub fn queue_undo_shroud_reveal(
        &self,
        center: &Coord3D,
        radius: Real,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        let persist_frames = game_engine::common::global_data::read_safe()
            .map(|data| data.unlook_persist_duration)
            .unwrap_or(0);
        let current_frame = TheGameLogic::get_frame();
        shroud.queue_undo_shroud_reveal(
            center,
            radius,
            player_mask.bits(),
            persist_frames,
            current_frame,
        );
        drop(shroud);
        // 40wu SWITCH_BORDER grid shares lookers. Persist delay stays on
        // ShroudManager; the partition cell undo is applied immediately so
        // storeFoggedCells cannot keep a looker that already unlooked.
        crate::object::stamp_partition_cell_lookers(center, radius, player_mask.bits(), false);
    }

    /// Mirrors C++ ThePartitionManager->doShroudCover().
    pub fn do_shroud_cover(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_shroud_cover(center, radius, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->undoShroudCover().
    pub fn undo_shroud_cover(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_shroud_cover(center, radius, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->doThreatAffect().
    pub fn do_threat_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        threat_value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_threat_affect(center, radius, threat_value, player_mask.bits());
        drop(shroud);
        Self::affect_partition_value_cells(center, radius, threat_value, player_mask, true, true);
    }

    /// Mirrors C++ ThePartitionManager->undoThreatAffect().
    pub fn undo_threat_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        threat_value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_threat_affect(center, radius, threat_value, player_mask.bits());
        drop(shroud);
        Self::affect_partition_value_cells(center, radius, threat_value, player_mask, true, false);
    }

    /// Mirrors C++ ThePartitionManager->doValueAffect().
    pub fn do_value_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_value_affect(center, radius, value, player_mask.bits());
        drop(shroud);
        Self::affect_partition_value_cells(center, radius, value, player_mask, false, true);
    }

    /// Mirrors C++ ThePartitionManager->undoValueAffect().
    pub fn undo_value_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_value_affect(center, radius, value, player_mask.bits());
        drop(shroud);
        Self::affect_partition_value_cells(center, radius, value, player_mask, false, false);
    }

    /// C++ has one store: doValueAffect / doThreatAffect write the same
    /// PartitionCell cash/threat arrays that getMostValuableLocation and
    /// getNearestGroupWithValue read.
    fn affect_partition_value_cells(
        center: &Coord3D,
        radius: Real,
        value: u32,
        player_mask: PlayerMaskType,
        is_threat: bool,
        add: bool,
    ) {
        let Ok(mut pm) = crate::object::collide::partition_manager::PARTITION_MANAGER.write() else {
            return;
        };
        let bits = player_mask.bits();
        for player_idx in 0..16 {
            if (bits & (1u32 << player_idx)) == 0 {
                continue;
            }
            if add {
                if is_threat {
                    pm.do_threat_affect(center.x, center.y, radius, player_idx, value);
                } else {
                    pm.do_value_affect(center.x, center.y, radius, player_idx, value);
                }
            } else if is_threat {
                pm.remove_threat_affect(center.x, center.y, radius, player_idx, value);
            } else {
                pm.remove_value_affect(center.x, center.y, radius, player_idx, value);
            }
        }
    }

    /// C++ `ThePartitionManager->getNearestGroupWithValue`.
    pub fn get_nearest_group_with_value(
        &self,
        player_index: i32,
        allowed_player_mask: u32,
        val_type: crate::object::collide::partition_manager::ValueOrThreat,
        source_pos: &Coord3D,
        value_required: i32,
        greater_than: bool,
    ) -> Option<Coord3D> {
        let collide_pos =
            crate::object::collide::Coord3D::new(source_pos.x, source_pos.y, source_pos.z);
        crate::object::collide::partition_manager::PARTITION_MANAGER
            .read()
            .ok()
            .and_then(|pm| {
                pm.get_nearest_group_with_value(
                    player_index,
                    allowed_player_mask,
                    val_type,
                    &collide_pos,
                    value_required,
                    greater_than,
                )
            })
            .map(|p| Coord3D::new(p.x, p.y, p.z))
    }

    /// C++ `ThePartitionManager->getMostValuableLocation`.
    pub fn get_most_valuable_location(
        &self,
        player_index: i32,
        allowed_player_mask: u32,
        val_type: crate::object::collide::partition_manager::ValueOrThreat,
    ) -> Option<Coord3D> {
        crate::object::collide::partition_manager::PARTITION_MANAGER
            .read()
            .ok()
            .and_then(|pm| {
                pm.get_most_valuable_location(player_index, allowed_player_mask, val_type)
            })
            .map(|p| Coord3D::new(p.x, p.y, p.z))
    }

    /// C++ `ThePartitionManager->unRegisterObject`.
    pub fn unregister_object(&self, object_id: crate::common::ObjectID) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().try_lock() {
            logic.partition_manager_mut().remove_object(object_id);
        }
    }

    /// C++ `ThePartitionManager->registerObject`.
    pub fn register_object_at(&self, object_id: crate::common::ObjectID, pos: Coord3D) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().try_lock() {
            logic
                .partition_manager_mut()
                .add_object(object_id, (pos.x, pos.y, pos.z));
        }
    }
}

impl crate::common::types::PartitionManagerInterface for ThePartitionManagerBridge {
    fn get_distance_squared(
        &self,
        a: &crate::object::Object,
        b: &crate::object::Object,
        distance_type: crate::common::types::PartitionDistanceType,
    ) -> f32 {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_2D, FROM_CENTER_3D};

        let flags = match distance_type {
            crate::common::types::PartitionDistanceType::Center2D => FROM_CENTER_2D,
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                FROM_BOUNDING_SPHERE_2D
            }
            crate::common::types::PartitionDistanceType::Center3D => FROM_CENTER_3D,
        };
        ThePartitionManager::get_distance_squared(a, b, flags)
    }

    fn get_distance_squared_to_pos(
        &self,
        obj: &crate::object::Object,
        pos: &Coord3D,
        distance_type: crate::common::types::PartitionDistanceType,
    ) -> f32 {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_2D, FROM_CENTER_3D};

        let flags = match distance_type {
            crate::common::types::PartitionDistanceType::Center2D => FROM_CENTER_2D,
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                FROM_BOUNDING_SPHERE_2D
            }
            crate::common::types::PartitionDistanceType::Center3D => FROM_CENTER_3D,
        };
        ThePartitionManager::get_distance_squared_to_pos(obj, pos, flags)
    }

    fn get_closest_object(
        &self,
        from: &crate::object::Object,
        max_range: f32,
        distance_type: crate::common::types::PartitionDistanceType,
        filters: &[crate::common::types::PartitionFilter],
    ) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
        // Wave 281: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let partition = ThePartitionManager::get()?;
        let from_pos = from.get_position();
        let candidates = match distance_type {
            crate::common::types::PartitionDistanceType::Center3D => {
                partition.get_objects_in_range_boundary_3d(from_pos, max_range)
            }
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                partition.get_objects_in_range_boundary_2d(from_pos, max_range)
            }
            crate::common::types::PartitionDistanceType::Center2D => {
                partition.get_objects_in_range(from_pos, max_range)
            }
        };
        let mut best: Option<(f32, ObjectID)> = None;

        for id in candidates {
            let Some(dist) = crate::object::registry::OBJECT_REGISTRY
                .with_object(id, |obj| {
                    if !partition_filter_allows(from, obj, filters) {
                        return None;
                    }
                    Some(self.get_distance_squared(from, obj, distance_type))
                })
                .flatten()
            else {
                continue;
            };
            if dist <= max_range * max_range {
                if best.map_or(true, |(best_dist, _)| dist < best_dist) {
                    best = Some((dist, id));
                }
            }
        }

        // Closest-object API still returns Arc for callers that need a handle.
        best.and_then(|(_, id)| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }
}

impl ThePartitionManagerBridge {
    /// Closest object ID matching filters without retaining an Arc at the call site.
    pub fn get_closest_object_id(
        &self,
        from: &crate::object::Object,
        max_range: f32,
        distance_type: crate::common::types::PartitionDistanceType,
        filters: &[crate::common::types::PartitionFilter],
    ) -> Option<ObjectID> {
        // Wave 281: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let partition = ThePartitionManager::get()?;
        let from_pos = from.get_position();
        let candidates = match distance_type {
            crate::common::types::PartitionDistanceType::Center3D => {
                partition.get_objects_in_range_boundary_3d(from_pos, max_range)
            }
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                partition.get_objects_in_range_boundary_2d(from_pos, max_range)
            }
            crate::common::types::PartitionDistanceType::Center2D => {
                partition.get_objects_in_range(from_pos, max_range)
            }
        };
        let mut best: Option<(f32, ObjectID)> = None;

        for id in candidates {
            let Some(dist) = crate::object::registry::OBJECT_REGISTRY
                .with_object(id, |obj| {
                    if !partition_filter_allows(from, obj, filters) {
                        return None;
                    }
                    Some(
                        crate::common::types::PartitionManagerInterface::get_distance_squared(
                            self,
                            from,
                            obj,
                            distance_type,
                        ),
                    )
                })
                .flatten()
            else {
                continue;
            };
            if dist <= max_range * max_range {
                match best {
                    Some((best_dist, _)) if dist >= best_dist => {}
                    _ => best = Some((dist, id)),
                }
            }
        }

        best.map(|(_, id)| id)
    }
}

fn partition_filter_allows(
    from: &crate::object::Object,
    candidate: &crate::object::Object,
    filters: &[crate::common::types::PartitionFilter],
) -> bool {
    use crate::common::types::PartitionFilter;
    use crate::common::Relationship;
    use crate::object::ObjectScriptStatusBit;

    for filter in filters {
        let allowed = match *filter {
            PartitionFilter::Flammable => candidate.find_update_module("FlammableUpdate").is_some(),
            PartitionFilter::Enemy => {
                matches!(from.relationship_to(candidate), Relationship::Enemies)
            }
            PartitionFilter::Friendly => {
                matches!(from.relationship_to(candidate), Relationship::Allies)
            }
            PartitionFilter::Neutral => {
                matches!(from.relationship_to(candidate), Relationship::Neutral)
            }
            PartitionFilter::Targetable => {
                if candidate.is_effectively_dead() {
                    false
                } else if candidate.test_script_status_bit(ObjectScriptStatusBit::ScriptTargetable)
                {
                    true
                } else {
                    !candidate.test_script_status_bit(ObjectScriptStatusBit::ScriptDisabled)
                        && !candidate.is_off_map()
                }
            }
            PartitionFilter::Attackable => {
                !candidate.is_effectively_dead()
                    && !candidate.is_off_map()
                    && !candidate.test_script_status_bit(ObjectScriptStatusBit::ScriptDisabled)
            }
            PartitionFilter::CanHeal => object_can_heal(candidate),
            PartitionFilter::CanRepair => object_can_repair(candidate),
            PartitionFilter::KindOf(kind) => candidate.is_kind_of(kind),
        };
        if !allowed {
            return false;
        }
    }
    true
}

fn object_can_heal(candidate: &crate::object::Object) -> bool {
    if candidate.is_kind_of(crate::common::KindOf::HealPad) {
        return true;
    }
    for module_handle in candidate.behavior_modules() {
        let mut matched = false;
        module_handle.with_module(|module| {
            if module
                .as_any()
                .is::<crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule>()
            {
                matched = true;
            }
        });
        if matched {
            return true;
        }
    }
    false
}

fn object_can_repair(candidate: &crate::object::Object) -> bool {
    if candidate.is_kind_of(crate::common::KindOf::RepairPad) {
        return true;
    }
    if candidate.with_dock_update_interface(|_| true).is_some() {
        return true;
    }
    false
}

impl crate::special_power_module::integration::PartitionManagerInterface
    for ThePartitionManagerBridge
{
    fn find_objects_in_radius(
        &self,
        center: &Coord3D,
        radius: Real,
        filter: Option<crate::special_power_module::integration::ObjectFilter>,
    ) -> Vec<ObjectID> {
        // Wave 281: empty dual-world → no objects.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let partition = ThePartitionManager::get();
        let Some(partition) = partition else {
            return Vec::new();
        };
        let mut results = partition.get_objects_in_range(center, radius);
        if let Some(filter) = filter {
            let local_team = crate::player::player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()));
            results.retain(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(*id, |obj| match filter {
                        crate::special_power_module::integration::ObjectFilter::All => true,
                        crate::special_power_module::integration::ObjectFilter::Infantry => {
                            obj.is_kind_of(crate::common::KindOf::Infantry)
                        }
                        crate::special_power_module::integration::ObjectFilter::Vehicles => {
                            obj.is_kind_of(crate::common::KindOf::Vehicle)
                        }
                        crate::special_power_module::integration::ObjectFilter::Structures => {
                            obj.is_kind_of(crate::common::KindOf::Structure)
                                || obj.is_kind_of(crate::common::KindOf::Building)
                        }
                        crate::special_power_module::integration::ObjectFilter::Aircraft => {
                            obj.is_kind_of(crate::common::KindOf::Aircraft)
                        }
                        crate::special_power_module::integration::ObjectFilter::Enemy => {
                            let Some(team_arc) = local_team.as_ref() else {
                                return true;
                            };
                            let Ok(team_guard) = team_arc.read() else {
                                return true;
                            };
                            let Some(obj_team) = obj.get_team() else {
                                return false;
                            };
                            let Ok(obj_team_guard) = obj_team.read() else {
                                return false;
                            };
                            team_guard.get_relationship(&obj_team_guard)
                                == crate::common::Relationship::Enemies
                        }
                        crate::special_power_module::integration::ObjectFilter::Friendly => {
                            let Some(team_arc) = local_team.as_ref() else {
                                return true;
                            };
                            let Ok(team_guard) = team_arc.read() else {
                                return true;
                            };
                            let Some(obj_team) = obj.get_team() else {
                                return false;
                            };
                            let Ok(obj_team_guard) = obj_team.read() else {
                                return false;
                            };
                            matches!(
                                team_guard.get_relationship(&obj_team_guard),
                                crate::common::Relationship::Allies
                            )
                        }
                    })
                    .unwrap_or(false)
            });
        }
        results
    }

    fn find_position_around(
        &self,
        location: &Coord3D,
        max_radius: Real,
        flags: crate::special_power_module::integration::FindPositionFlags,
    ) -> Option<Coord3D> {
        let partition = ThePartitionManager::get()?;
        let mut options = FindPositionOptions::default();
        options.max_radius = max_radius;
        if flags
            .contains(crate::special_power_module::integration::FindPositionFlags::CLEAR_CELLS_ONLY)
        {
            options.flags |= FPF_CLEAR_CELLS_ONLY;
        }
        if flags.contains(crate::special_power_module::integration::FindPositionFlags::NO_WATER) {
            options.flags |= FPF_IGNORE_WATER;
        }
        if flags.contains(crate::special_power_module::integration::FindPositionFlags::PASSABLE) {
            options.flags |= FPF_CLEAR_CELLS_ONLY;
        }
        let mut result = Coord3D::new(0.0, 0.0, 0.0);
        if partition.find_position_around_with_options(location, &options, &mut result) {
            Some(result)
        } else {
            None
        }
    }
}

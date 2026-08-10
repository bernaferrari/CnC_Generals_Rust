use super::*;

impl PathfindingSystem {
    /// C++ `segmentIntersectsBuildingCallback`: first AIRCRAFT_PATH_AROUND
    /// obstacle along a ground Bresenham line (via cell obstacle ID).
    pub(crate) fn find_tall_building_along_segment(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        ignore_building: ObjectID,
    ) -> Option<(ObjectID, Coord3D, f32)> {
        // Wave 262: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let mut found = None;
        let _ = self.iterate_cells_along_line_world(
            from,
            to,
            PathfindLayerEnum::Ground,
            |_f, to_c, _x, _y| {
                // C++: to->getType()==OBSTACLE then findObjectByID(to->getObstacleID()).
                let Ok(pf) = self.pathfinder.lock() else {
                    return 0;
                };
                if pf.get_cell_type(to_c) != Some(PathfindCellType::Obstacle) {
                    return 0;
                }
                let Some(oid) = pf.get_cell_obstacle_id(to_c) else {
                    return 0;
                };
                drop(pf);
                if oid == ignore_building || oid == INVALID_ID {
                    return 0;
                }
                let Some((p, r)) = OBJECT_REGISTRY
                    .with_object(oid, |g| {
                        if !g.is_kind_of(KindOf::AircraftPathAround) {
                            return None;
                        }
                        let p = *g.get_position();
                        let r = g.get_geometry_info().get_bounding_circle_radius()
                            + 2.0 * PATHFIND_CELL_SIZE_F;
                        Some((p, r))
                    })
                    .flatten()
                else {
                    return 0;
                };
                found = Some((oid, p, r));
                1 // stop like C++ callback return 1
            },
        );
        found
    }

    /// C++ `Pathfinder::segmentIntersectsTallBuilding` (AIPathfind.cpp:9464-9519).
    ///
    /// If the ground segment hits a tall building, write three radial offset
    /// insert positions and return true. May nudge `to` outward if it lies
    /// inside the building radius.
    pub fn segment_intersects_tall_building(
        &self,
        from: &Coord3D,
        to: &mut Coord3D,
        ignore_building: ObjectID,
        insert1: &mut Coord3D,
        insert2: &mut Coord3D,
        insert3: &mut Coord3D,
    ) -> bool {
        let mut from_pos = *from;
        let mut to_pos = *to;
        for _ in 0..2 {
            let Some((_id, bldg_pos, radius)) =
                self.find_tall_building_along_segment(&from_pos, &to_pos, ignore_building)
            else {
                return false;
            };

            // If toPos inside radius, push it out (C++ nextNode->setPosition).
            let mut delta_x = to_pos.x - bldg_pos.x;
            let mut delta_y = to_pos.y - bldg_pos.y;
            let mut len = (delta_x * delta_x + delta_y * delta_y).sqrt();
            if len <= radius * 0.98 {
                if len < 0.1 {
                    delta_x = 1.0;
                    delta_y = 0.0;
                    len = 1.0;
                }
                delta_x = delta_x / len * radius;
                delta_y = delta_y / len * radius;
                to_pos.x = bldg_pos.x + delta_x;
                to_pos.y = bldg_pos.y + delta_y;
                *to = to_pos;
                continue; // retry loop like C++
            }

            // If fromPos inside radius, push from out.
            delta_x = from_pos.x - bldg_pos.x;
            delta_y = from_pos.y - bldg_pos.y;
            len = (delta_x * delta_x + delta_y * delta_y).sqrt();
            if len <= radius * 0.98 {
                if len < 0.1 {
                    delta_x = 1.0;
                    delta_y = 0.0;
                    len = 1.0;
                }
                delta_x = delta_x / len * radius;
                delta_y = delta_y / len * radius;
                from_pos.x = bldg_pos.x + delta_x;
                from_pos.y = bldg_pos.y + delta_y;
            }

            Self::compute_normal_radial_offset(&from_pos, insert2, &to_pos, &bldg_pos, radius);
            Self::compute_normal_radial_offset(&from_pos, insert1, insert2, &bldg_pos, radius);
            Self::compute_normal_radial_offset(insert2, insert3, &to_pos, &bldg_pos, radius);
            return true;
        }
        false
    }

    /// C++ `Pathfinder::circleClipsTallBuilding` (AIPathfind.cpp:9522-9539).
    ///
    /// If a KINDOF_AIRCRAFT_PATH_AROUND building is within circleRadius of `to`,
    /// offset `adjust_to` around it. Optionally adjust for a second nearby tall building.
    pub fn circle_clips_tall_building(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        circle_radius: f32,
        ignore_building: ObjectID,
        adjust_to: &mut Coord3D,
    ) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let Some(partition) = ThePartitionManager::get() else {
            return false;
        };
        let mut tall_id = None;
        let mut tall_pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut tall_radius = 0.0_f32;
        let mut best_dist = f32::MAX;
        for oid in partition.get_objects_in_range(to, circle_radius) {
            if oid == ignore_building || oid == INVALID_ID {
                continue;
            }
            let Some((p, radius)) = OBJECT_REGISTRY
                .with_object(oid, |g| {
                    if !g.is_kind_of(KindOf::AircraftPathAround) {
                        return None;
                    }
                    let p = *g.get_position();
                    let radius = g.get_geometry_info().get_bounding_circle_radius()
                        + 2.0 * PATHFIND_CELL_SIZE_F;
                    Some((p, radius))
                })
                .flatten()
            else {
                continue;
            };
            let dx = p.x - to.x;
            let dy = p.y - to.y;
            let d = (dx * dx + dy * dy).sqrt();
            if d < best_dist {
                best_dist = d;
                tall_id = Some(oid);
                tall_pos = p;
                tall_radius = radius;
            }
        }
        let Some(tall_id) = tall_id else {
            return false;
        };
        Self::compute_normal_radial_offset(
            from,
            adjust_to,
            to,
            &tall_pos,
            circle_radius + tall_radius,
        );

        // Second tall building near adjust_to.
        let mut other_pos = None;
        let mut other_radius = 0.0_f32;
        best_dist = f32::MAX;
        for oid in partition.get_objects_in_range(adjust_to, circle_radius) {
            if oid == ignore_building || oid == tall_id || oid == INVALID_ID {
                continue;
            }
            let Some((p, radius)) = OBJECT_REGISTRY
                .with_object(oid, |g| {
                    if !g.is_kind_of(KindOf::AircraftPathAround) {
                        return None;
                    }
                    let p = *g.get_position();
                    let radius = g.get_geometry_info().get_bounding_circle_radius()
                        + 2.0 * PATHFIND_CELL_SIZE_F;
                    Some((p, radius))
                })
                .flatten()
            else {
                continue;
            };
            let dx = p.x - adjust_to.x;
            let dy = p.y - adjust_to.y;
            let d = (dx * dx + dy * dy).sqrt();
            if d < best_dist {
                best_dist = d;
                other_pos = Some(p);
                other_radius = radius;
            }
        }
        if let Some(op) = other_pos {
            let tmp = *adjust_to;
            Self::compute_normal_radial_offset(
                from,
                adjust_to,
                &tmp,
                &op,
                circle_radius + other_radius,
            );
        }
        true
    }

    /// C++ `Pathfinder::clearCellForDiameter` (AIPathfind.cpp:6700-6759).
    ///
    /// Returns clear diameter (even) if the footprint is clear; 0 if blocked;
    /// recursively tries pathDiameter-2 when blocked and diameter >= 2.
    pub fn clear_cell_for_diameter(
        &self,
        crusher: bool,
        cell_x: i32,
        cell_y: i32,
        layer: PathfindLayerEnum,
        path_diameter: i32,
    ) -> i32 {
        // Grid cell types / fence flags work with an empty registry.
        // Missing pos-unit object: treat as non-crushable (that cell only).

        if path_diameter <= 0 {
            return 0;
        }
        let radius = path_diameter / 2;
        let mut num_cells_above = radius;
        if radius == 0 {
            num_cells_above += 1;
        }
        let cut_corners = radius > 1;
        let mut clear = true;

        let goals = self.goal_cells.lock().ok();

        'outer: for i in (cell_x - radius)..(cell_x + num_cells_above) {
            let x_min_or_max = i == cell_x - radius || i == cell_x + num_cells_above - 1;
            for j in (cell_y - radius)..(cell_y + num_cells_above) {
                let y_min_or_max = j == cell_y - radius || j == cell_y + num_cells_above - 1;
                if x_min_or_max && y_min_or_max && cut_corners {
                    continue; // outside corner cut
                }
                let coord = GridCoord::new(i, j);
                if !self.is_valid_coord(coord) {
                    return 0; // off the map
                }
                let world = coord.to_world(layer);
                let Some(ctype) = self.get_cell_type(&world) else {
                    return 0;
                };
                if ctype != PathfindCellType::Clear {
                    if ctype == PathfindCellType::Obstacle {
                        // C++: fence obstacles block only non-crushers; solid obstacles always block.
                        let is_fence = self
                            .pathfinder
                            .lock()
                            .map(|pf| pf.is_obstacle_fence(coord))
                            .unwrap_or(false);
                        if is_fence {
                            if !crusher {
                                clear = false;
                            }
                        } else {
                            clear = false;
                        }
                    } else {
                        clear = false;
                    }
                }
                // C++ UNIT_PRESENT_FIXED via getPosUnit when pathDiameter >= 2.
                if path_diameter >= 2 {
                    if let Some(ref goals) = goals {
                        if let Some(row) = goals.get(coord.x as usize) {
                            if let Some(gc) = row.get(coord.y as usize) {
                                let pos_unit = gc.get_pos_unit(layer);
                                if pos_unit != INVALID_ID {
                                    let _ = OBJECT_REGISTRY.with_object(pos_unit, |og| {
                                        let crushable = og.get_crushable_level();
                                        if crusher {
                                            if crushable > 1 {
                                                clear = false;
                                            }
                                        } else if crushable > 0 {
                                            clear = false;
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
                if !clear {
                    break 'outer;
                }
            }
        }
        drop(goals);

        if clear {
            if radius == 0 {
                return 1;
            }
            return 2 * radius;
        }
        if path_diameter < 2 {
            return 0;
        }
        self.clear_cell_for_diameter(crusher, cell_x, cell_y, layer, path_diameter - 2)
    }

    /// C++ `Pathfinder::iterateCellsAlongLine` Bresenham (AIPathfind.cpp:9092-9200).
    ///
    /// Calls `proc(from_cell, to_cell, x, y)` for each cell. Returns first non-zero
    /// proc result, or 0 if the line completed.
    pub fn iterate_cells_along_line<F>(
        &self,
        start: GridCoord,
        end: GridCoord,
        _layer: PathfindLayerEnum,
        mut proc: F,
    ) -> i32
    where
        F: FnMut(Option<GridCoord>, GridCoord, i32, i32) -> i32,
    {
        let delta_x = (end.x - start.x).abs();
        let delta_y = (end.y - start.y).abs();
        let mut x = start.x;
        let mut y = start.y;

        let (mut xinc1, mut xinc2) = if end.x >= start.x { (1, 1) } else { (-1, -1) };
        let (mut yinc1, mut yinc2) = if end.y >= start.y { (1, 1) } else { (-1, -1) };

        let (den, mut num, numadd, numpixels);
        if delta_x >= delta_y {
            xinc1 = 0;
            yinc2 = 0;
            den = delta_x;
            num = delta_x / 2;
            numadd = delta_y;
            numpixels = delta_x;
        } else {
            xinc2 = 0;
            yinc1 = 0;
            den = delta_y;
            num = delta_y / 2;
            numadd = delta_x;
            numpixels = delta_y;
        }

        let mut from: Option<GridCoord> = None;
        for _ in 0..=numpixels {
            let to = GridCoord::new(x, y);
            if !self.is_valid_coord(to) {
                return 0;
            }
            let ret = proc(from, to, x, y);
            if ret != 0 {
                return ret;
            }
            num += numadd;
            if num >= den {
                num -= den;
                x += xinc1;
                y += yinc1;
                from = Some(to);
                let to2 = GridCoord::new(x, y);
                if !self.is_valid_coord(to2) {
                    return 0;
                }
                let ret = proc(from, to2, x, y);
                if ret != 0 {
                    return ret;
                }
                from = Some(to2);
            } else {
                from = Some(to);
            }
            x += xinc2;
            y += yinc2;
        }
        0
    }

    /// World-space entry for `iterateCellsAlongLine`.
    pub fn iterate_cells_along_line_world<F>(
        &self,
        start_world: &Coord3D,
        end_world: &Coord3D,
        layer: PathfindLayerEnum,
        proc: F,
    ) -> i32
    where
        F: FnMut(Option<GridCoord>, GridCoord, i32, i32) -> i32,
    {
        let start = GridCoord::from_world(start_world);
        let end = GridCoord::from_world(end_world);
        self.iterate_cells_along_line(start, end, layer, proc)
    }

    /// C++ `Pathfinder::validLocomotorSurfacesForCellType` (AIPathfind.cpp:4734-4758).
    pub fn valid_locomotor_surfaces_for_cell_type(
        cell_type: PathfindCellType,
    ) -> LocomotorSurfaceTypeMask {
        match cell_type {
            PathfindCellType::Obstacle
            | PathfindCellType::Impassable
            | PathfindCellType::BridgeImpassable => SURFACE_AIR,
            PathfindCellType::Clear => SURFACE_GROUND | SURFACE_AIR,
            PathfindCellType::Water => SURFACE_WATER | SURFACE_AIR,
            PathfindCellType::Rubble => SURFACE_RUBBLE | SURFACE_AIR,
            PathfindCellType::Cliff => SURFACE_CLIFF | SURFACE_AIR,
            _ => 0,
        }
    }

    /// C++ `Pathfinder::validMovementTerrain` (AIPathfind.cpp:4763-4783).
    ///
    /// Obstacle/Impassable return true (terrain present); otherwise require
    /// locomotor surfaces ∩ cell surfaces.
    pub fn valid_movement_terrain(
        &self,
        layer: PathfindLayerEnum,
        surfaces: LocomotorSurfaceTypeMask,
        pos: &Coord3D,
    ) -> bool {
        let coord = GridCoord::from_world(pos);
        if !self.is_valid_coord(coord) {
            return false;
        }
        let Some(cell_type) = self.get_cell_type(pos) else {
            return false;
        };
        // C++: OBSTACLE / IMPASSABLE → true
        if matches!(
            cell_type,
            PathfindCellType::Obstacle | PathfindCellType::Impassable
        ) {
            return true;
        }
        // C++ validMovementTerrain: non-ground CLEAR cells always pass
        if layer != PathfindLayerEnum::Ground && cell_type == PathfindCellType::Clear {
            return true;
        }
        let cell_surfaces = Self::valid_locomotor_surfaces_for_cell_type(cell_type);
        (surfaces & cell_surfaces) != 0
    }

    /// Quick validity check for a locomotor position (C++ validMovementPosition usage).
    pub fn valid_movement_position(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        pos: &Coord3D,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let coord = GridCoord::from_world(pos);
        self.valid_movement_cell(surfaces, is_crusher, coord, ignore_obstacle_id)
    }

    /// Quick validity check for a locomotor grid cell.
    pub(crate) fn valid_movement_cell(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        coord: GridCoord,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        if !self.is_valid_coord(coord) {
            return false;
        }
        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        let pathfinder = self.pathfinder.lock().unwrap();
        pathfinder.is_passable_with_ignore(coord, surfaces, is_crusher, ignore_cells.as_ref())
    }
}

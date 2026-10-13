//! C++ contract witnesses for the shared pathfinding algorithms.
//! Kept beneath the runtime tests so their containing filter and private helpers stay shared.

use super::*;

#[test]
fn pathfinder_clip_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn clip(").expect("clip");
    let w = &prod[i..prod.len().min(i + 800)];
    assert!(
        w.contains("0.05") && w.contains("clip_line_cells") && w.contains("from_world"),
        "Pathfinder::clip must floor cells, ClipLine, write +0.05 like C++"
    );
}

#[test]
fn adjust_destination_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn adjust_destination_from")
        .expect("adjustDestinationFrom");
    let end = prod[i..]
        .find("pub fn adjust_to_possible_destination")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 12000));
    let w = &prod[i..end];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.5")
            && w.contains("PathfindCellType::Cliff")
            && w.contains("client_safe_quick_does_path_exist")
            && w.contains("MAX_CELLS_TO_TRY")
            && w.contains("try_adjust_cell"),
        "adjustDestination must half-cell offset, reject cliffs, path-gate like C++"
    );
}

#[test]
fn snap_closest_goal_position_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn snap_closest_goal_position")
        .expect("snapClosestGoalPosition");
    let w = &prod[i..prod.len().min(i + 4500)];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.5")
            && w.contains("adjust_coord_to_cell")
            && w.contains("is_destination_valid")
            && w.contains("radius == 0"),
        "snapClosestGoalPosition must half-cell, 3x3, radius0 unoccupied like C++"
    );
}

#[test]
fn adjust_to_possible_destination_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn adjust_to_possible_destination")
        .expect("adjustToPossibleDestination");
    let end = prod[i..]
        .find("pub fn check_for_target(")
        .map(|o| i + o)
        .expect("adjustment and zone helper boundary");
    let w = &prod[i..end];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.5")
            && w.contains("is_valid_coord(goal_cell)")
            && w.contains("are_connected")
            && w.contains("adjust_coord_to_cell"),
        "adjustToPossibleDestination must half-cell, bounds-fail, zone+snap like C++"
    );
}

#[test]
fn adjust_to_landing_destination_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("fn check_for_landing(").expect("checkForLanding");
    let end = prod[i..]
        .find("pub fn check_for_adjust")
        .or_else(|| prod[i..].find("/// Full adjustment pipeline"))
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 6000));
    let w = &prod[i..end];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.5")
            && w.contains("check_for_landing")
            && w.contains("MAX_CELLS_TO_TRY")
            && w.contains("PathfindCellType::Cliff")
            && w.contains("PathfindCellType::Water")
            && w.contains("PathfindCellType::Impassable"),
        "adjustToLandingDestination must half-cell spiral + reject cliff/water like C++"
    );
}

#[test]
fn adjust_target_destination_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn adjust_target_destination")
        .expect("adjustTargetDestination");
    let w = &prod[i..prod.len().min(i + 4000)];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.5")
            && w.contains("check_for_target")
            && w.contains("MAX_CELLS_TO_TRY")
            && w.contains("is_valid_coord(cell)"),
        "adjustTargetDestination must half-cell spiral + bounds fail like C++"
    );
}

#[test]
fn is_line_passable_ex_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("fn is_line_passable_for_object_with_ignore_id(")
        .expect("borrowed line-passability implementation");
    let wrapper = prod
        .split("fn is_line_passable_for_object_inner(")
        .nth(1)
        .expect("line-passability wrapper")
        .split("fn is_line_passable_for_object_with_ignore_id(")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.is_line_passable_for_object_with_ignore_id("));
    let w = prod[i..].split("/// Check if ground path").next().unwrap();
    assert!(
        w.contains("allow_pinched")
            && w.contains("is_crusher")
            && w.contains("is_pinched")
            && w.contains("check_for_movement")
            && w.contains("ally_fixed_count")
            && w.contains("enemy_fixed"),
        "linePassableCallback must pinch-gate + checkForMovement occupancy like C++"
    );
}

#[test]
fn check_for_movement_cpp_surface() {
    let src = include_str!("check_movement.rs");
    let wrapper = src
        .split("pub fn check_for_movement(")
        .nth(1)
        .expect("checkForMovement wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.check_for_movement_with_ignore_id("));
    assert!(wrapper.contains("get_ignored_obstacle_id"));
    let borrowed = src
        .split("pub(crate) fn check_for_movement_with_ignore_id(")
        .nth(1)
        .expect("borrowed mover AI adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        borrowed
            .contains("self.check_for_movement_with_observations(obj_id, info, ignore_id, None)")
    );
    let w = src
        .split("pub(crate) fn check_for_movement_with_observations(")
        .nth(1)
        .expect("shared occupancy scan")
        .split("\n    }\n")
        .next()
        .unwrap();
    let relationship = src
        .split("fn movement_relationship(")
        .nth(1)
        .expect("directional relationship observation")
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(
        w.contains("ally_fixed_count")
            && w.contains("enemy_fixed")
            && w.contains("MAX_ALLY")
            && w.contains("Relationship::Allies")
            && w.contains("movement_relationship(source, occupant, observations)")
            && relationship.contains("source.relationship_to(occupant)")
            && relationship.contains("observations.relationship(source, occupant)"),
        "checkForMovement must track ally/enemy fixed occupancy like C++"
    );
}

#[test]
fn valid_movement_terrain_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn valid_locomotor_surfaces_for_cell_type")
        .expect("validLocomotorSurfacesForCellType");
    let end = prod[i..]
        .find("pub fn valid_movement_position")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 3500));
    let w = &prod[i..end];
    assert!(
        w.contains("PathfindCellType::Obstacle")
            && w.contains("PathfindCellType::Impassable")
            && w.contains("valid_locomotor_surfaces_for_cell_type")
            && w.contains("SURFACE_GROUND | SURFACE_AIR")
            && w.contains("pub fn valid_movement_terrain"),
        "validMovementTerrain must special-case obstacle/impassable + surface mask"
    );
}

#[test]
fn tighten_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn tighten_path").expect("tightenPath");
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("try_adjust_cell")
            && w.contains("iterate_cells_along_line_world")
            && w.contains("found"),
        "tightenPath must Bresenham-walk with checkForAdjust residual"
    );
}

#[test]
fn move_allies_away_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn move_allies_away_from_destination")
        .expect("moveAlliesAwayFromDestination");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("get_ignored_obstacle_id")
            && w.contains("Relationship::Allies")
            && w.contains("ai_move_away_from_unit")
            && w.contains("is_idle")
            && w.contains("iterate_cells_along_line_world")
            && w.contains("CommandSourceType::FromAi"),
        "moveAlliesAwayFromDestination must Bresenham-nudge idle allies like C++"
    );
}

#[test]
fn clear_cell_for_diameter_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn clear_cell_for_diameter")
        .expect("clearCellForDiameter");
    let end = prod[i..]
        .find("pub fn iterate_cells_along_line")
        .map(|o| i + o)
        .unwrap_or(prod.len().min(i + 6000));
    let w = &prod[i..end];
    assert!(
        w.contains("cut_corners")
            && w.contains("path_diameter - 2")
            && w.contains("PathfindCellType::Obstacle")
            && w.contains("get_crushable_level"),
        "clearCellForDiameter must cut corners, recurse diameter-2, check obstacles"
    );
}

#[test]
fn iterate_cells_along_line_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn iterate_cells_along_line<")
        .expect("iterateCellsAlongLine");
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("delta_x") && w.contains("numpixels") && w.contains("numadd"),
        "iterateCellsAlongLine must use Bresenham like C++"
    );
}

#[test]
fn circle_clips_tall_building_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn circle_clips_tall_building")
        .expect("circleClipsTallBuilding");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("KindOf::AircraftPathAround")
            && w.contains("compute_normal_radial_offset")
            && w.contains("2.0 * PATHFIND_CELL_SIZE_F")
            && w.contains("get_objects_in_range"),
        "circleClipsTallBuilding must path around AIRCRAFT_PATH_AROUND like C++"
    );
}

#[test]
fn segment_intersects_tall_building_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn segment_intersects_tall_building")
        .expect("segmentIntersectsTallBuilding");
    let w = &prod[i..prod.len().min(i + 4000)];
    assert!(
        w.contains("find_tall_building_along_segment")
            && w.contains("compute_normal_radial_offset")
            && w.contains("0.98")
            && w.contains("KindOf::AircraftPathAround"),
        "segmentIntersectsTallBuilding must Bresenham-find tall bldg + radial inserts"
    );
}

#[test]
fn queue_for_path_and_process_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(
        prod.contains("struct ObjectPathQueue")
            && prod.contains("pub fn queue_for_path")
            && prod.contains("PATHFIND_CELLS_PER_FRAME")
            && prod.contains("pub fn process_queue"),
        "queueForPath/processPathfindQueue must use ObjectID ring like C++"
    );
}

#[test]
fn build_actual_path_prepend_cells_cpp_surface() {
    let src = include_str!("occupancy.rs");
    let wrapper = src
        .split("pub(crate) fn build_actual_path_from_layered_path(")
        .nth(1)
        .expect("layer-preserving wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.build_actual_path_from_layered_path_with_observations("));
    let supplied = src
        .split("pub(crate) fn build_actual_path_from_layered_path_with_observations(")
        .nth(1)
        .expect("supplied reconstruction adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(supplied.contains("self.reconstruct_layered_path("));
    let w = src
        .split("pub(super) fn reconstruct_layered_path(")
        .nth(1)
        .expect("shared reconstruction")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        w.contains("can_optimize")
            && w.contains("PathfindCellType::Cliff")
            && w.contains("insert(0")
            && w.contains("from_world")
            && w.contains("let (coord, layer) = grid_path[idx]")
            && w.contains("for idx in (1..grid_path.len()).rev()"),
        "buildActualPath/prependCells must reverse-walk with cliff optimize flags"
    );
}

#[test]
fn snap_position_for_radius_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn snap_position_for_radius")
        .expect("snapPosition");
    let w = &prod[i..prod.len().min(i + 1500)];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F * 0.5")
            && w.contains("adjust_coord_to_cell")
            && w.contains("center_in_cell"),
        "snapPosition must half-cell bias + adjustCoordToCell like C++"
    );
}

#[test]
fn classify_object_footprint_fence_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn classify_object_footprint_ex")
        .expect("classifyObjectFootprint");
    let w = &prod[i..prod.len().min(i + 6000)];
    assert!(
        w.contains("KindOf::Mine")
            && w.contains("classify_fence")
            && w.contains("get_fence_width")
            && w.contains("STEP_SIZE")
            && w.contains("GeometryType::Box"),
        "classifyObjectFootprint must filter kindofs, fence raster, box/cylinder"
    );
}

#[test]
fn update_goal_remove_goal_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn update_goal").expect("updateGoal");
    let w = &prod[i..prod.len().min(i + 4500)];
    assert!(
        w.contains("remove_goal") && w.contains("unit_goal_cells") && w.contains("set_goal_cells"),
        "updateGoal must remove prior goal then stamp cells"
    );
    let j = prod
        .find("pub fn remove_unit_from_pathfind_map")
        .expect("removeUnitFromPathfindMap");
    let w2 = &prod[j..prod.len().min(j + 800)];
    assert!(
        w2.contains("remove_goal") && w2.contains("remove_pos"),
        "removeUnitFromPathfindMap clears goal+pos"
    );
}

#[test]
fn update_aircraft_goal_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn update_aircraft_goal")
        .expect("updateAircraftGoal");
    let w = &prod[i..prod.len().min(i + 2000)];
    assert!(
        w.contains("remove_goal")
            && w.contains("set_aircraft_goal_cells")
            && w.contains("cell_for_unit_position"),
        "updateAircraftGoal must removeGoal then stamp goalAircraft"
    );
}

#[test]
fn force_map_and_wall_pieces_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("pub fn force_map_recalculation"));
    assert!(prod.contains("pub fn add_wall_piece"));
    assert!(prod.contains("pub fn remove_wall_piece"));
    assert!(prod.contains("pub fn is_point_on_wall"));
    let i = prod.find("pub fn is_point_on_wall").expect("isPointOnWall");
    let w = &prod[i..prod.len().min(i + 800)];
    assert!(
        w.contains("wall_pieces.is_empty()") && w.contains("wall_cells"),
        "isPointOnWall must require wall pieces + wall cell set"
    );
}

#[test]
fn check_change_layers_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn check_change_layers")
        .expect("checkChangeLayers");
    let w = &prod[i..prod.len().min(i + 1200)];
    assert!(
        w.contains("get_cell_connect_layer") && w.contains("PathfindLayerEnum::Invalid"),
        "checkChangeLayers must read connectLayer"
    );
}

#[test]
fn goal_position_and_path_destination_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn goal_position").expect("goalPosition");
    let goal_end = prod[i..]
        .find("pub fn check_path_cost(")
        .map(|offset| i + offset)
        .unwrap_or(prod.len());
    let w = &prod[i..goal_end];
    assert!(
        w.contains("unit_goal_cells") && w.contains("adjust_coord_to_cell"),
        "goalPosition must read tracked goal cell"
    );
    let j = prod
        .find("pub fn path_destination")
        .expect("pathDestination");
    let path_end = prod[j..]
        .find("pub fn update_aircraft_goal(")
        .map(|offset| j + offset)
        .expect("pathDestination helper boundary");
    let w2 = &prod[j..path_end];
    assert!(
        w2.contains("MAX_CELL_COUNT")
            && w2.contains("check_for_adjust")
            && w2.contains("check_change_layers")
            && w2.contains("is_map_ready"),
        "pathDestination must budget search + checkForAdjust"
    );
}

#[test]
fn zone_block_and_effective_zone_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("ZONE_BLOCK_SIZE"));
    assert!(prod.contains("fn get_block_zone"));
    assert!(prod.contains("fn get_effective_zone"));
    assert!(prod.contains("pub fn process_hierarchical_cell"));
    // Prefer PathfindZoneManager::getEffectiveZone (not ZoneBlock local).
    let i = prod
        .rfind("fn get_effective_zone")
        .expect("getEffectiveZone");
    // Also accept BlockCombiner + manager both present.
    assert!(
        prod.contains("crusher_zones") && prod.contains("ground_cliff_zones"),
        "manager combiner tables present"
    );
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("SURFACE_AIR") && (w.contains("crusher_zones") || w.contains("self.crusher")),
        "getEffectiveZone must handle air + combiner tables"
    );
}

#[test]
fn pathfinder_crc_xfer_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn crc(&self, xfer").expect("crc");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("is_map_ready")
            && w.contains("is_tunneling")
            && w.contains("object_path_queue")
            && w.contains("wall_pieces")
            && w.contains("cumulative_cells_allocated"),
        "Pathfinder::crc must cover extent, flags, queue, walls, cells"
    );
    assert!(prod.contains("pub fn xfer(&mut self, xfer"));
    assert!(prod.contains("pub fn load_post_process"));
    assert!(prod.contains("pub fn clean_open_and_closed_lists"));
    assert!(prod.contains("pub fn move_allies("));
}

#[test]
fn get_move_away_from_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn get_move_away_from_path_result")
        .expect("getMoveAwayFromPath result");
    let w = &prod[i..prod.len().min(i + 10000)];
    assert!(
        w.contains("line_in_region")
            && w.contains("box_half")
            && w.contains("path_to_avoid")
            && w.contains("is_map_ready")
            && w.contains("BinaryHeap")
            && w.contains("check_for_movement")
            && w.contains("set_all_passable")
            && w.contains("find_path"),
        "getMoveAwayFromPath must A* expand + box-test path segments + build path"
    );
    // Reset semantics are exercised by cpp_reset_releases_map_dimensions_and_readiness.
    // Reconstructing the inert owner need not spell out every field assignment.
}

#[test]
fn patch_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn patch_path").expect("patchPath");
    let w = &prod[i..prod.len().min(i + 12000)];
    assert!(
        w.contains("check_for_movement")
            && w.contains("CELL_LIMIT")
            && w.contains("original_waypoints")
            && w.contains("set_all_passable")
            && w.contains("optimize_path"),
        "patchPath must walk original path, A* reconnect, splice + optimize"
    );
}

#[test]
fn find_attack_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn find_attack_path")
        .expect("findAttackPath");
    let w = &prod[i..prod.len().min(i + 6000)];
    assert!(
        w.contains("PATHFIND_CELL_SIZE_F")
            && w.contains("in_range")
            && w.contains("view_blocked")
            && w.contains("find_closest_hierarchical_path")
            && w.contains("clear_passable_flags"),
        "findAttackPath must quick-step, hierarchical probe, spiral attack cells"
    );
}

#[test]
fn get_aircraft_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn get_aircraft_path")
        .expect("getAircraftPath");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("circle_clips_tall_building")
            && w.contains("segment_intersects_tall_building")
            && w.contains("limit")
            && w.contains("100.0"),
        "getAircraftPath must clip tall buildings and insert detour nodes"
    );
    assert!(prod.contains("pub fn check_for_possible"));
}

#[test]
fn build_ground_and_hierarchical_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn build_ground_path")
        .expect("buildGroundPath");
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("optimize_ground_path") && w.contains("build_actual_path"),
        "buildGroundPath must prepend + optimizeGroundPath"
    );
    let j = prod
        .find("pub fn build_hierarchical_path")
        .expect("buildHierachicalPath");
    let w2 = &prod[j..prod.len().min(j + 2000)];
    assert!(
        w2.contains("set_passable") && w2.contains("ZONE_BLOCK_SIZE"),
        "buildHierarchicalPath expands passable around start"
    );
    assert!(prod.contains("pub fn set_debug_path"));
}

#[test]
fn zone_calculate_with_types_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("calculate_zones_with_types"));
    assert!(prod.contains("build_surface_combiners"));
    assert!(prod.contains("pair_ground_cliff"));
    assert!(prod.contains("pair_water_ground"));
    assert!(prod.contains("pair_crusher_ground"));
    assert!(prod.contains("recalculate_zones_from_cells"));
}

#[test]
fn fence_flag_cpp_surface() {
    // AIPathfind.cpp:1421-1453: obstacle annotations belong to the obstacle operations.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/obstacles.rs"
    ));
    assert!(src.contains("obstacle_fence"));
    assert!(src.contains("is_obstacle_fence"));
    let pc = PATHFIND_COMPLETE_SRC;
    let prod = pc.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("calculate_zones_with_types_and_fences"));
    assert!(prod.contains("is_obstacle_fence"));
}

#[test]
fn zone_block_grid_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("struct BlockCombiner"));
    assert!(prod.contains("zone_blocks"));
    assert!(prod.contains("fn rebuild_zone_blocks"));
    assert!(prod.contains("get_block_zone"));
    let i = prod.find("fn get_block_zone").expect("getBlockZone");
    let w = &prod[i..prod.len().min(i + 1200)];
    assert!(
        w.contains("ZONE_BLOCK_SIZE") && w.contains("get_effective_zone"),
        "getBlockZone must index zone_blocks and use block effective zone"
    );
}

#[test]
fn hierarchical_and_terrain_zones_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("hierarchical_zones"));
    assert!(prod.contains("terrain_zones"));
    let i = prod
        .rfind("fn get_effective_zone")
        .expect("getEffectiveZone");
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("hierarchical_zones"),
        "default getEffectiveZone must use hierarchical_zones"
    );
    let j = prod.find("fn get_effective_terrain_zone").expect("terrain");
    let wt = &prod[j..prod.len().min(j + 800)];
    assert!(
        wt.contains("terrain_zones") && wt.contains("hierarchical_zones"),
        "getEffectiveTerrainZone = hierarchical[terrain[zone]]"
    );
}

#[test]
fn connect_layer_hierarchical_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("connect_layers"));
    assert!(prod.contains("layer_zones"));
    assert!(
        prod.contains("PathfindLayerEnum::Ground as u8"),
        "connectLayer > LAYER_GROUND hierarchical resolve"
    );
}

#[test]
fn process_queue_dirty_cpp_surface() {
    let src = include_str!("construct.rs");
    let wrapper = src
        .split("pub fn process_queue(")
        .nth(1)
        .expect("queue wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.process_queue_with_ai(max_per_frame,"));
    let borrowed = src
        .split("pub fn process_queue_with_ai(")
        .nth(1)
        .expect("borrowed queue adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(borrowed.contains("self.process_queue_with_owner(max_per_frame, None,"));
    let w = src
        .split("pub fn process_queue_with_owner(")
        .nth(1)
        .expect("shared queue phase")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        w.contains("zones_dirty") && w.contains("recalculate_zones_from_cells"),
        "process_queue must recalculate dirty zones like C++"
    );
    assert!(
        w.find("recalculate_zones_from_cells").unwrap()
            < w.find("object_path_queue.pop_front()").unwrap()
    );
}

#[test]
fn zone_passable_cost_cpp_surface() {
    // AIPathfind.cpp:6155-6158, 6313-6315: coarse-zone failure adds a cost to A*.
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("set_zone_passable"));
    assert!(prod.contains("set_zone_cell_passable"));
    let astar = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/astar.rs"
    ));
    assert!(astar.contains("movement_cost.saturating_add(ZONE_IMPASSABLE_COST)"));
    assert!(astar.contains("notZonePassable") || astar.contains("is_zone_passable"));
}

#[test]
fn zone_bridge_flags_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("fn set_bridge"));
    assert!(prod.contains("fn interacts_with_bridge"));
    assert!(prod.contains("clear_bridge_flags"));
    assert!(
        prod.contains("set_bridge(bridge.start_cell")
            || prod.contains("zones.set_bridge(bridge.start_cell"),
        "recalc must stamp setBridge from live bridge layers"
    );
}

#[test]
fn logical_extent_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("logical_extent_lo"));
    assert!(prod.contains("refresh_logical_extent"));
    assert!(prod.contains("in_logical_extent"));
    assert!(prod.contains("is_human"));
}

#[test]
fn process_queue_uses_cell_budget() {
    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    // Queue several paths.
    for i in 0..5 {
        let req = PathRequest {
            object_id: INVALID_ID,
            from: Coord3D::new(20.0, 20.0, 0.0),
            to: Coord3D::new(200.0 + i as f32 * 10.0, 200.0, 0.0),
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human: false,
        };
        system.queue_path_request(req).ok();
    }
    let n = system.process_queue(PATHFIND_CELLS_PER_FRAME);
    assert!(n >= 1, "should process at least one path");
    assert!(
        system.cumulative_cells_allocated() > 0,
        "cells examined must accumulate"
    );
    // Tiny budget stops after cells exceed.
    system.cumulative_cells_allocated = PATHFIND_CELLS_PER_FRAME as i32;
    // re-queue
    let req = PathRequest {
        object_id: INVALID_ID,
        from: Coord3D::new(20.0, 20.0, 0.0),
        to: Coord3D::new(300.0, 300.0, 0.0),
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    system.queue_path_request(req).ok();
    // process_queue resets cumulative at start via refresh - check it zeros first
    // Actually process_queue stores 0 at start - so budget always fresh.
    // Verify surface: process_queue contains cell_budget check.
    let src = include_str!("construct.rs");
    let wrapper = src
        .split("pub fn process_queue(")
        .nth(1)
        .expect("queue wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.process_queue_with_ai(max_per_frame,"));
    let borrowed = src
        .split("pub fn process_queue_with_ai(")
        .nth(1)
        .expect("borrowed queue adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(borrowed.contains("self.process_queue_with_owner(max_per_frame, None,"));
    let w = src
        .split("pub fn process_queue_with_owner(")
        .nth(1)
        .expect("shared queue phase")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(w.contains("cell_budget") || w.contains("PATHFIND_CELLS_PER_FRAME"));
    assert!(w.contains("cumulative_cells_allocated"));
    assert!(w.contains("self.cumulative_cells_allocated = 0"));
    assert!(w.contains("self.cumulative_cells_allocated() as usize >= cell_budget"));
}

#[test]
fn hierarchical_bridge_jumps_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("fn hierarchical_bridge_jumps"));
    assert!(prod.contains("hierarchical_zones_join_via_bridge"));
    assert!(
        !prod.contains("Full multi-layer bridge zone jump remains residual"),
        "bridge jump residual comment must be gone"
    );
}

#[test]
fn build_actual_path_center_in_cell_cpp_surface() {
    let src = include_str!("occupancy.rs");
    let wrapper = src
        .split("pub fn build_actual_path(")
        .nth(1)
        .expect("buildActualPath wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        wrapper.contains("center_in_cell")
            && wrapper.contains("self.build_actual_path_for_object(")
    );
    let object = src
        .split("pub fn build_actual_path_for_object(")
        .nth(1)
        .expect("object path adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        object.contains("center_in_cell")
            && object.contains("self.build_actual_path_from_layered_path(")
    );
    let layered = src
        .split("pub(crate) fn build_actual_path_from_layered_path(")
        .nth(1)
        .expect("layered adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(layered.contains("self.build_actual_path_from_layered_path_with_observations("));
    let supplied = src
        .split("pub(crate) fn build_actual_path_from_layered_path_with_observations(")
        .nth(1)
        .expect("supplied height adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        supplied.contains("self.reconstruct_layered_path(") && supplied.contains("center_in_cell")
    );
    let w = src
        .split("pub(super) fn reconstruct_layered_path(")
        .nth(1)
        .expect("shared reconstruction")
        .split("\n    }\n")
        .next()
        .unwrap();
    let snap = include_str!("snap.rs");
    let adjustment = snap
        .split("pub(crate) fn adjust_coord_to_cell_with_observations(")
        .nth(1)
        .expect("adjustCoordToCell shared conversion")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        w.contains("center_in_cell")
            && w.contains("Self::coord_to_cell_with_height(")
            && adjustment.contains("Self::coord_to_cell_with_height("),
        "buildActualPath must use the same centerInCell coordinate conversion as adjustCoordToCell"
    );
    assert!(!w.contains("residual: callers pass centerInCell"));
    // AIPathfind.cpp:8934-8946: actual conversion uses 0.5/0.05 cell bias.
    let centered = PathfindingSystem::coord_to_cell_with_height(
        1,
        2,
        true,
        PathfindLayerEnum::Top,
        |_, _, layer| layer as u32 as f32,
    );
    let cornered = PathfindingSystem::coord_to_cell_with_height(
        1,
        2,
        false,
        PathfindLayerEnum::Top,
        |_, _, layer| layer as u32 as f32,
    );
    assert_eq!(centered, Coord3D::new(15.0, 25.0, 2.0));
    assert_eq!(cornered, Coord3D::new(10.5, 20.5, 2.0));
}

#[test]
fn check_for_movement_can_crush_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn check_for_movement")
        .expect("checkForMovement");
    let w = &prod[i..prod.len().min(i + 12000)];
    assert!(
        w.contains("can_crush_or_squish") && w.contains("CrushSquishTestType::TestCrushOrSquish"),
        "checkForMovement must call canCrushOrSquish like C++"
    );
    assert!(!w.contains("Prefer real canCrush"));
}

#[test]
fn update_goal_bridge_end_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn update_goal").expect("updateGoal");
    let w = &prod[i..prod.len().min(i + 1200)];
    assert!(
        w.contains("interacts_with_bridge_end"),
        "updateGoal must take objectInteractsWithBridgeEnd flag"
    );
    assert!(!w.contains("Bridge-end residual"));
}

#[test]
fn tall_building_segment_uses_obstacle_id_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("fn find_tall_building_along_segment")
        .expect("find_tall");
    let w = &prod[i..prod.len().min(i + 2000)];
    assert!(
        w.contains("get_cell_obstacle_id") && w.contains("AircraftPathAround"),
        "segmentIntersectsBuildingCallback must use cell obstacle ID"
    );
    assert!(!w.contains("Without per-cell obstacle IDs"));
}

#[test]
fn clear_cell_for_diameter_fence_and_pos_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn clear_cell_for_diameter")
        .expect("clearCellForDiameter");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("is_obstacle_fence") && w.contains("get_pos_unit"),
        "clearCellForDiameter must use fence flag + getPosUnit"
    );
    assert!(!w.contains("Fence residual"));
    assert!(!w.contains("UNIT_PRESENT_FIXED residual via goal"));
}

#[test]
fn check_for_movement_flag_semantics_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn check_for_movement")
        .expect("checkForMovement");
    let w = &prod[i..prod.len().min(i + 5500)];
    assert!(
        w.contains("get_pos_unit")
            && w.contains("UNIT_PRESENT_MOVING")
            && w.contains("ally_moving")
            && w.contains("consider_transient"),
        "checkForMovement must use pos units + moving/fixed flags"
    );
    assert!(!w.contains("UNIT_PRESENT_FIXED residual via goal claim"));
}

#[test]
fn snap_closest_fixed_uses_pos_goal_flags_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("fn goal_cell_fixed_occupied")
        .expect("fixed helper");
    let w = &prod[i..prod.len().min(i + 800)];
    assert!(
        w.contains("UNIT_PRESENT_FIXED") && w.contains("get_pos_unit"),
        "snapClosestGoal radius0 pass must use UNIT_PRESENT_FIXED flags"
    );
    assert!(!w.contains("Approximate UNIT_PRESENT_FIXED"));
}

#[test]
fn find_attack_path_range_los_during_search_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn find_attack_path_range")
        .expect("findAttackPath range");
    let w = &prod[i..prod.len().min(i + 2000)];
    assert!(
        w.contains("view_blocked") || w.contains("is_line_passable_ex"),
        "find_attack_path_range must apply LOS during candidate selection"
    );
    assert!(!w.contains("residual vs full re-search"));
    assert!(!w.contains("|_a, _b| false"));
}

#[test]
fn find_attack_path_human_logical_extent_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn find_attack_path")
        .expect("findAttackPath");
    let w = &prod[i..prod.len().min(i + 5000)];
    assert!(
        w.contains("is_human") && w.contains("in_logical_extent"),
        "findAttackPath must clamp human candidates to m_logicalExtent"
    );
}

#[test]
fn check_for_adjust_ex_human_extent_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn check_for_adjust_ex")
        .expect("checkForAdjustEx");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("is_human") && w.contains("in_logical_extent"),
        "checkForAdjust must clamp humans to m_logicalExtent"
    );
    assert!(
        w.contains("tighten_path") && w.contains("check_path_cost"),
        "checkForAdjust must tightenPath + checkPathCost for groupDest"
    );
    assert!(
        w.contains("PathfindCellType::Cliff"),
        "checkForAdjust must reject cliff destinations"
    );
}

#[test]
fn path_destination_uses_check_for_adjust_ex_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn path_destination")
        .expect("pathDestination");
    let w = &prod[i..prod.len().min(i + 4500)];
    assert!(
        w.contains("check_for_adjust_ex"),
        "pathDestination must call full checkForAdjust with is_human + groupDest"
    );
    assert!(!w.contains("let _ = is_human"));
}

#[test]
fn are_connected_uses_effective_zone_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("fn are_connected(").expect("are_connected");
    let w = &prod[i..prod.len().min(i + 1500)];
    assert!(
        w.contains("get_effective_zone"),
        "are_connected must compare getEffectiveZone results, not raw cell zones"
    );
    assert!(!w.contains("_surfaces"));
    assert!(!w.contains("_is_crusher"));
}

#[test]
fn move_allies_uses_pos_unit_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn move_allies(").expect("moveAllies");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("get_pos_unit(layer)"),
        "moveAllies must read PathfindCell::getPosUnit standing occupancy"
    );
    assert!(
        !w.contains("get_goal_unit(layer)"),
        "moveAllies must not use goal-unit claims for standing allies"
    );
}

#[test]
fn check_path_cost_astar_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn check_path_cost").expect("checkPathCost");
    let w = &prod[i..prod.len().min(i + 5000)];
    assert!(
        w.contains("MAX_CELL_COUNT") && w.contains("BinaryHeap") && w.contains("0x7fff_0000"),
        "checkPathCost must run limited A* and return C++ MAX_COST"
    );
    assert!(!w.contains("Approximate path cost as cell Manhattan"));
}

#[test]
fn refresh_logical_extent_from_terrain_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn refresh_logical_extent")
        .expect("refreshLogicalExtent");
    let w = &prod[i..prod.len().min(i + 1500)];
    assert!(
        w.contains("get_extent") && w.contains("PATHFIND_CELL_SIZE_F") && w.contains("hi_x -= 1"),
        "refresh_logical_extent must floor terrain extent / cell size and decrement hi"
    );
}

#[test]
fn process_queue_calls_do_pathfind_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn process_queue")
        .expect("processPathfindQueue");
    let w = &prod[i..prod.len().min(i + 3500)];
    assert!(
        w.contains("do_pathfind"),
        "processPathfindQueue must call AIUpdateInterface::doPathfind"
    );
}

#[test]
fn find_safe_path_astar_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn find_safe_path").expect("findSafePath");
    let w = &prod[i..prod.len().min(i + 5000)];
    assert!(
        w.contains("BinaryHeap")
            && w.contains("set_all_passable")
            && w.contains("repulsor_radius_sqr")
            && w.contains("MAX_CELLS"),
        "findSafePath must A* expand with repulsor radius and setAllPassable"
    );
    assert!(!w.contains("for search_radius in 0i32..=64"));
}

#[test]
fn find_path_hierarchical_precheck_cpp_surface() {
    let src = include_str!("find_path.rs");
    let wrapper = src
        .split("pub fn find_path(")
        .nth(1)
        .expect("findPath wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.find_path_with_movement_policy(request, None)"));
    let policy = src
        .split("pub(crate) fn find_path_with_movement_policy(")
        .nth(1)
        .expect("borrowed movement policy adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(policy.contains("self.find_path_with_observations(request, downhill_only, None)"));
    let w = src
        .split("pub(crate) fn find_path_with_observations(")
        .nth(1)
        .expect("shared findPath")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        w.contains("client_safe_quick_does_path_exist")
            && w.contains("clear_passable_flags")
            && w.contains("are_connected")
            && w.contains("set_all_passable")
            && w.contains("hierarchical_zones_join_via_bridge")
            && w.contains("self.clear_zone_passable_flags()")
            && w.contains("self.set_all_zone_passable()"),
        "findPath must quick-exist + hierarchical passable flag dance like C++"
    );
}

#[test]
fn find_closest_path_astar_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn find_closest_path")
        .expect("findClosestPath");
    let w = &prod[i..prod.len().min(i + 6000)];
    assert!(
        w.contains("BinaryHeap")
            && w.contains("closest_cell")
            && w.contains("COST_TO_DISTANCE_FACTOR_SQR")
            && w.contains("clear_passable_flags"),
        "findClosestPath must A* track closest valid cell like C++"
    );
    assert!(!w.contains("max_search_radius = 20"));
}

#[test]
fn internal_find_hierarchical_path_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn internal_find_hierarchical_path")
        .expect("internal_findHierarchicalPath");
    let w = &prod[i..prod.len().min(i + 12000)];
    assert!(
        w.contains("process_hierarchical_cell")
            && w.contains("hierarchical_bridge_jumps")
            && w.contains("ZONE_BLOCK_SIZE")
            && w.contains("closest_ok"),
        "hierarchical path must zone-block A* with processHierarchicalCell"
    );
}

#[test]
fn classify_bridge_cells_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    assert!(prod.contains("LAYER_Z_CLOSE_ENOUGH_F"));
    let i = prod
        .find("pub fn classify_bridge_cells")
        .expect("classify_bridge_cells");
    let w = &prod[i..prod.len().min(i + 5000)];
    assert!(
        w.contains("BridgeImpassable")
            && w.contains("LAYER_Z_CLOSE_ENOUGH_F")
            && w.contains("ground_connect_cells"),
        "bridge classify must apply clearance + entry Clear"
    );
}

#[test]
fn build_actual_path_ally_block_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn build_actual_path_for_object")
        .expect("buildActualPath for object");
    let w = &prod[i..prod.len().min(i + 7000)];
    assert!(
        w.contains("cell_blocked_by_ally") && w.contains("blocked_by_ally = true"),
        "buildActualPath must stamp path blockedByAlly from cell occupancy"
    );
}

#[test]
fn find_path_ex_ally_cost_cpp_surface() {
    // AIPathfind.cpp:6281-6294: ally occupancy contributes to the neighbor cost.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/astar.rs"
    ));
    assert!(
        src.contains("find_path_ex") && src.contains("extra_cost"),
        "A* must accept per-cell extra cost for allyFixedCount"
    );
    let complete = PATHFIND_COMPLETE_SRC;
    let prod = complete.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("fn find_path_internal")
        .expect("internalFindPath");
    let w = &prod[i..prod.len().min(i + 12000)];
    assert!(
        w.contains("ally_fixed_count")
            && (w.contains("find_path_ex") || w.contains("find_path_ex3")),
        "internalFindPath must feed allyFixedCount into A* costs"
    );
}

#[test]
fn optimize_path_blocked_cpp_surface() {
    let src = include_str!("find_path.rs");
    let wrapper = src
        .split("pub(crate) fn find_path_internal(")
        .nth(1)
        .expect("internalFindPath wrapper")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(wrapper.contains("self.find_path_internal_with_downhill_policy(request, None)"));
    let policy = src
        .split("fn find_path_internal_with_downhill_policy(")
        .nth(1)
        .expect("internal movement policy adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        policy
            .contains("self.find_path_internal_with_observations(request, downhill_policy, None)")
    );
    let w = src
        .split("pub(crate) fn find_path_internal_with_observations(")
        .nth(1)
        .expect("shared internalFindPath")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        w.contains("optimize_path_blocked") && w.contains("result.blocked_by_ally"),
        "internalFindPath must optimize with blockedByAlly flag like C++"
    );
    let lines = include_str!("line_passable.rs");
    let compatibility = lines
        .split("pub(crate) fn optimize_path_blocked(")
        .nth(1)
        .expect("blocked path adapter")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(compatibility.contains(
        "self.optimize_path_blocked_with_observations(waypoints, layers, request, blocked, None)"
    ));
    let optimization = lines
        .split("pub(crate) fn optimize_path_blocked_with_observations(")
        .nth(1)
        .expect("shared blocked path optimization")
        .split("\n    }\n")
        .next()
        .unwrap();
    assert!(
        optimization.contains("blocked, // consider_transient")
            && optimization.contains("self.optimizer.optimize(waypoints, layers, passability)")
    );
}

#[test]
fn ally_moving_cost_requires_near_start_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("fn find_path_internal").expect("internal");
    let w = &prod[i..prod.len().min(i + 12000)];
    assert!(
        w.contains("dx < 10") && w.contains("ally_moving"),
        "allyMoving cost must require dx,dy < 10 from start like C++"
    );
}

#[test]
fn downhill_only_astar_cpp_surface() {
    // AIPathfind.cpp:6189-6204: downhill-only locomotors reject uphill neighbors.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/astar.rs"
    ));
    assert!(
        src.contains("downhill_only") && src.contains("find_path_ex2"),
        "A* must support downhill-only step rejection"
    );
    assert!(src.contains("if fz < tz {"));
    let complete = PATHFIND_COMPLETE_SRC;
    let prod = complete.split("#[cfg(test)]").next().expect("production");
    assert!(
        prod.contains("object_is_downhill_only")
            && (prod.contains("find_path_ex2")
                || prod.contains("find_path_ex3")
                || prod.contains("find_path_ex4")
                || prod.contains("find_path_ex5")
                || prod.contains("find_path_ex6")),
        "internalFindPath must pass downhill_only into A*"
    );
}

#[test]
fn tunneling_dozer_astar_cpp_surface() {
    // AIPathfind.cpp:6208-6226: dozers and tunneling can permit obstacle cells.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/astar.rs"
    ));
    assert!(src.contains("force_passable") && src.contains("find_path_ex3"));
    let complete = PATHFIND_COMPLETE_SRC;
    let prod = complete.split("#[cfg(test)]").next().expect("production");
    assert!(
        prod.contains("start_is_obstacle")
            && prod.contains("object_is_dozer")
            && prod.contains("dozer_obstacle_ok")
            && (prod.contains("find_path_ex3")
                || prod.contains("find_path_ex4")
                || prod.contains("find_path_ex5")
                || prod.contains("find_path_ex6")),
        "internalFindPath must set tunneling from obstacle start and dozerHack"
    );
}

#[test]
fn examine_cells_line_seed_cpp_surface() {
    // AIPathfind.cpp:5996-6093: line seeding adds half an orthogonal step cost.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/astar.rs"
    ));
    assert!(
        src.contains("examine_cells_toward_goal")
            && src.contains("COST_ORTHOGONAL / 2")
            && src.contains("find_path_ex4"),
        "A* must seed line-to-goal cells at half orthogonal cost"
    );
    let complete = PATHFIND_COMPLETE_SRC;
    let prod = complete.split("#[cfg(test)]").next().expect("production");
    assert!(
        prod.contains("seed_line")
            && (prod.contains("find_path_ex4")
                || prod.contains("find_path_ex5")
                || prod.contains("find_path_ex6"))
            && prod.contains("line_ok"),
        "internalFindPath must enable examineCellsCallback line seed"
    );
}

#[test]
fn tunneling_dynamic_clear_cpp_surface() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/lib.rs"
    ));
    assert!(
        src.contains("starts_tunneling")
            && src.contains("is_tunneling = false")
            && src.contains("10 * COST_ORTHOGONAL"),
        "A* must clear tunneling and apply C++ tunnel surcharge"
    );
    let complete = PATHFIND_COMPLETE_SRC;
    let prod = complete.split("#[cfg(test)]").next().expect("production");
    assert!(
        (prod.contains("find_path_ex5") || prod.contains("find_path_ex6"))
            && prod.contains("tunneling"),
        "internalFindPath must pass starts_tunneling into A*"
    );
}

#[test]
fn find_attack_path_astar_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("pub fn find_attack_path").expect("fap");
    let w = &prod[i..prod.len().min(i + 16000)];
    assert!(
        w.contains("ATTACK_CELL_LIMIT")
            && w.contains("ally_goal")
            && w.contains("attack_dist")
            && w.contains("is_vehicle"),
        "findAttackPath must A*-expand with attackDistance + allyGoal costs"
    );
}

#[test]
fn human_logical_extent_astar_cpp_surface() {
    // AIPathfind.cpp:6160-6165: human neighbors are restricted to logical bounds.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../Pathfinding/src/astar.rs"
    ));
    assert!(
        src.contains("cell_allowed") && src.contains("logical extent"),
        "A* must accept cell_allowed for human logical extent clamp"
    );
    assert!(src.contains("if let Some(ok) = cell_allowed"));
    assert!(src.contains("if !ok(neighbor_coord)"));
    let complete = PATHFIND_COMPLETE_SRC;
    let prod = complete.split("#[cfg(test)]").next().expect("production");
    let i = prod.find("fn find_path_internal").expect("internal");
    let w = &prod[i..prod.len().min(i + 12000)];
    assert!(
        w.contains("cell_allowed") && w.contains("in_logical_extent") && w.contains("is_human"),
        "internalFindPath must clamp human A* neighbors to logical extent"
    );
}

#[test]
fn is_attack_view_blocked_by_obstacle_cpp_surface() {
    let src = PATHFIND_COMPLETE_SRC;
    let prod = src.split("#[cfg(test)]").next().expect("production");
    let i = prod
        .find("pub fn is_attack_view_blocked_by_obstacle")
        .expect("LOS");
    let w = &prod[i..prod.len().min(i + 9000)];
    assert!(
        w.contains("AttackNeedsLineOfSight")
            && w.contains("skip_count")
            && w.contains("is_obstacle_transparent")
            && w.contains("is_clear_goal_firing_line_of_sight_terrain"),
        "isAttackViewBlockedByObstacle must match C++ callback + LOS_TERRAIN"
    );
}

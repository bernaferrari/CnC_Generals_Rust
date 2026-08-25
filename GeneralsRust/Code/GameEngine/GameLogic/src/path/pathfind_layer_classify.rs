//! C++ `PathfindLayer::classifyLayerMapCell` / `classifyWallMapCell`.
//!
//! Kept out of `pathfind_layer.rs` so the layer object stays a cell matrix
//! owner; classification is the corner-count rules from AIPathfind.cpp.

use super::{PATHFIND_CELL_SIZE_F, PathfindCell, PathfindCellType, PathfindLayerEnum};
use crate::common::{Coord3D, ICoord2D, ObjectID};
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::system::geometry::GeometryType;

#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// C++ `PathfindLayer::isPointOnWall` — oriented rectangle (sphere uses major×major).
pub fn is_point_on_wall(wall_pieces: &[ObjectID], pt: &Coord3D) -> bool {
    if dual_world_registry_unavailable() {
        return false;
    }
    for &wall_id in wall_pieces {
        let on = OBJECT_REGISTRY
            .with_object(wall_id, |wall| {
                let pos = wall.get_position();
                let geom = wall.get_geometry_info();
                let major = geom.get_major_radius();
                let minor = if geom.get_geometry_type() == GeometryType::Sphere {
                    major
                } else {
                    geom.get_minor_radius()
                };
                let ori = -wall.get_orientation();
                let (s, c) = ori.sin_cos();
                let ptx = pt.x - pos.x;
                let pty = pt.y - pos.y;
                let ptx_new = (ptx * c - pty * s).abs();
                let pty_new = (ptx * s + pty * c).abs();
                ptx_new <= major && pty_new <= minor
            })
            .unwrap_or(false);
        if on {
            return true;
        }
    }
    false
}

/// World-space corners of pathfind cell `(i, j)` — C++ classify*MapCell.
#[inline]
pub fn cell_corners(i: i32, j: i32) -> [Coord3D; 4] {
    let x0 = i as f32 * PATHFIND_CELL_SIZE_F;
    let y0 = j as f32 * PATHFIND_CELL_SIZE_F;
    let x1 = x0 + PATHFIND_CELL_SIZE_F;
    let y1 = y0 + PATHFIND_CELL_SIZE_F;
    [
        Coord3D::new(x0, y0, 0.0),
        Coord3D::new(x0, y1, 0.0),
        Coord3D::new(x1, y1, 0.0),
        Coord3D::new(x1, y0, 0.0),
    ]
}

/// How many of the 4 cell corners sit on the wall pieces.
pub fn wall_corner_count(i: i32, j: i32, wall_pieces: &[ObjectID]) -> u32 {
    cell_corners(i, j)
        .iter()
        .filter(|pt| is_point_on_wall(wall_pieces, pt))
        .count() as u32
}

/// Approximate C++ 4-corner bridge-polygon count when only the layer AABB exists.
/// Interior cells → 4 (deck). Edge → 2. Corner → 1. Outside → 0.
pub fn aabb_deck_corner_count(cell_x: i32, cell_y: i32, lo: ICoord2D, hi: ICoord2D) -> u32 {
    if cell_x < lo.x || cell_x > hi.x || cell_y < lo.y || cell_y > hi.y {
        return 0;
    }
    let edge_x = cell_x == lo.x || cell_x == hi.x;
    let edge_y = cell_y == lo.y || cell_y == hi.y;
    match (edge_x, edge_y) {
        (false, false) => 4,
        (true, true) => 1,
        _ => 2,
    }
}

/// C++ `classifyWallMapCell`: reset, LAYER_WALL, default IMPASSABLE;
/// 4 corners → CLEAR; 1–3 → BRIDGE_IMPASSABLE; 0 stays IMPASSABLE.
pub fn classify_wall_map_cell(
    global_x: i32,
    global_y: i32,
    cell: &mut PathfindCell,
    wall_pieces: &[ObjectID],
) {
    cell.reset();
    cell.set_layer(PathfindLayerEnum::Wall);
    cell.set_type(PathfindCellType::Impassable);
    let count = wall_corner_count(global_x, global_y, wall_pieces);
    if count == 4 {
        cell.set_type(PathfindCellType::Clear);
    } else if count != 0 {
        cell.set_type(PathfindCellType::BridgeImpassable);
    }
}

/// C++ `classifyLayerMapCell` without a live `Bridge` polygon:
/// corner-count against the allocated AABB, then end/entry override.
pub fn classify_layer_map_cell(
    layer: PathfindLayerEnum,
    global_x: i32,
    global_y: i32,
    cell: &mut PathfindCell,
    extent_lo: ICoord2D,
    extent_hi: ICoord2D,
    start_cell: ICoord2D,
    end_cell: ICoord2D,
    destroyed: bool,
) {
    cell.reset();
    cell.set_layer(layer);
    cell.set_connect_layer(PathfindLayerEnum::Invalid);
    cell.set_type(PathfindCellType::Impassable);

    if destroyed {
        cell.set_type(PathfindCellType::BridgeImpassable);
        return;
    }

    if layer == PathfindLayerEnum::Wall {
        cell.set_type(PathfindCellType::Impassable);
        return;
    }

    let count = aabb_deck_corner_count(global_x, global_y, extent_lo, extent_hi);
    let is_entry = (global_x == start_cell.x && global_y == start_cell.y)
        || (global_x == end_cell.x && global_y == end_cell.y);

    if count == 4 {
        cell.set_type(PathfindCellType::Clear);
    } else if count != 0 {
        if is_entry {
            cell.set_type(PathfindCellType::Clear);
            cell.set_connect_layer(PathfindLayerEnum::Ground);
        } else {
            cell.set_type(PathfindCellType::BridgeImpassable);
        }
    }
}

/// Per-cell type for a bridge AABB (pathfind_complete `BridgeLayer` matrix).
pub fn classify_bridge_aabb_cell(
    cell_x: i32,
    cell_y: i32,
    lo: ICoord2D,
    hi: ICoord2D,
    start: ICoord2D,
    end: ICoord2D,
    destroyed: bool,
) -> Option<PathfindCellType> {
    if destroyed {
        return Some(PathfindCellType::BridgeImpassable);
    }
    let count = aabb_deck_corner_count(cell_x, cell_y, lo, hi);
    let is_entry = (cell_x == start.x && cell_y == start.y) || (cell_x == end.x && cell_y == end.y);
    match count {
        4 => Some(PathfindCellType::Clear),
        0 => None,
        _ if is_entry => Some(PathfindCellType::Clear),
        _ => Some(PathfindCellType::BridgeImpassable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_zero_corners_stays_impassable() {
        let mut cell = PathfindCell::new();
        classify_wall_map_cell(0, 0, &mut cell, &[]);
        assert_eq!(cell.get_type(), PathfindCellType::Impassable);
        assert_eq!(cell.get_layer(), PathfindLayerEnum::Wall);
    }

    #[test]
    fn bridge_interior_is_clear_edge_is_impassable() {
        let mut deck = PathfindCell::new();
        classify_layer_map_cell(
            PathfindLayerEnum::Bridge1,
            3,
            3,
            &mut deck,
            ICoord2D::new(2, 2),
            ICoord2D::new(6, 6),
            ICoord2D::new(2, 4),
            ICoord2D::new(6, 4),
            false,
        );
        assert_eq!(deck.get_type(), PathfindCellType::Clear);

        let mut edge = PathfindCell::new();
        classify_layer_map_cell(
            PathfindLayerEnum::Bridge1,
            2,
            3,
            &mut edge,
            ICoord2D::new(2, 2),
            ICoord2D::new(6, 6),
            ICoord2D::new(2, 4),
            ICoord2D::new(6, 4),
            false,
        );
        assert_eq!(edge.get_type(), PathfindCellType::BridgeImpassable);

        let mut entry = PathfindCell::new();
        classify_layer_map_cell(
            PathfindLayerEnum::Bridge1,
            2,
            4,
            &mut entry,
            ICoord2D::new(2, 2),
            ICoord2D::new(6, 6),
            ICoord2D::new(2, 4),
            ICoord2D::new(6, 4),
            false,
        );
        assert_eq!(entry.get_type(), PathfindCellType::Clear);
        assert_eq!(entry.get_connect_layer(), PathfindLayerEnum::Ground);
    }

    #[test]
    fn destroyed_bridge_cells_are_bridge_impassable() {
        let mut cell = PathfindCell::new();
        classify_layer_map_cell(
            PathfindLayerEnum::Bridge1,
            3,
            3,
            &mut cell,
            ICoord2D::new(2, 2),
            ICoord2D::new(6, 6),
            ICoord2D::new(2, 4),
            ICoord2D::new(6, 4),
            true,
        );
        assert_eq!(cell.get_type(), PathfindCellType::BridgeImpassable);
    }
}

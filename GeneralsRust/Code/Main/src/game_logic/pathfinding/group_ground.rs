//! Owned `Pathfinder::findGroundPath` group request (AIPathfind.cpp:6980).
use super::*;

#[path = "group_ground_clip.rs"]
mod clip;
#[path = "group_ground_optimize.rs"]
mod optimize;
#[path = "group_ground_search.rs"]
mod search;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Cell {
    pos: GridPos,
    layer: u8,
}

impl PathfindingGrid {
    /// The hierarchical corridor is prepared by the receiving pathfinder;
    /// it is deliberately an explicit input rather than an ambient active finder.
    pub(super) fn find_group_ground_path_owned(
        &self,
        from: Vec3,
        to: Vec3,
        objects: &HashMap<ObjectId, Object>,
        passable: impl Fn(GridPos) -> bool,
        cleanup_cells: &mut usize,
    ) -> Option<Vec<Vec3>> {
        if !from.is_finite()
            || !to.is_finite()
            || (to.x == 0.0 && to.z == 0.0)
            || self.width <= 0
            || self.height <= 0
        {
            return None;
        }
        let start_layer = self.layer_for_destination(from) as u8;
        let (clipped_from_pos, clipped_to_pos) = clip::clip(self, from, to);
        // C++ worldToCell clamps after the segment clip, including when the
        // segment lies wholly outside the extent (AIPathfind.h:935-945).
        let clipped_from = self.clamp_pos(self.world_to_grid(clipped_from_pos));
        let mut goal = Cell {
            pos: self.clamp_pos(self.world_to_grid(clipped_to_pos)),
            layer: self.layer_for_destination(clipped_to_pos) as u8,
        };
        if self.group_clearance(goal, 6, objects) != 6 {
            let original = goal.pos;
            let mut found = None;
            'rings: for radius in 1..8 {
                for (dx, dy) in [
                    (1, 0),
                    (1, 1),
                    (0, 1),
                    (-1, 1),
                    (-1, 0),
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                ] {
                    let candidate = Cell {
                        pos: GridPos::new(original.x + radius * dx, original.y + radius * dy),
                        ..goal
                    };
                    if self.group_clearance(candidate, 6, objects) == 6 {
                        found = Some(candidate);
                        break 'rings;
                    }
                }
            }
            goal = found?;
        }
        if !self.is_valid_pos(clipped_from) {
            return None;
        }
        goal = self.group_cell(goal.pos, goal.layer);
        let start = self.group_cell(clipped_from, start_layer);
        if !self.zones_connected(start.pos, goal.pos, SURFACE_GROUND, false) {
            return None;
        }
        let (cells, _) = search::find(self, start, goal, objects, &passable, cleanup_cells)?;
        Some(optimize::build(self, from, &cells, objects))
    }

    #[cfg(test)]
    pub(super) fn group_ground_raw_trace(
        &self,
        start: GridPos,
        goal: GridPos,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<(Vec<GridPos>, u32)> {
        let (cells, cost) = self.group_ground_layer_trace(start, goal, objects)?;
        Some((cells.into_iter().map(|(pos, _)| pos).collect(), cost))
    }

    #[cfg(test)]
    pub(super) fn group_ground_layer_trace(
        &self,
        start: GridPos,
        goal: GridPos,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<(Vec<(GridPos, u8)>, u32)> {
        let (cells, cost) = search::find(
            self,
            Cell {
                pos: start,
                layer: PathfindLayerEnum::Ground as u8,
            },
            Cell {
                pos: goal,
                layer: PathfindLayerEnum::Ground as u8,
            },
            objects,
            &|_| true,
            &mut 0,
        )?;
        Some((cells.into_iter().map(|c| (c.pos, c.layer)).collect(), cost))
    }

    fn group_cell(&self, pos: GridPos, layer: u8) -> Cell {
        let layer = if layer > PathfindLayerEnum::Ground as u8
            && self
                .layer_cell_type(layer, pos)
                .is_some_and(|ty| ty != PathfindCellType::Impassable)
        {
            layer
        } else {
            PathfindLayerEnum::Ground as u8
        };
        Cell { pos, layer }
    }

    fn group_clearance(
        &self,
        cell: Cell,
        diameter: i32,
        objects: &HashMap<ObjectId, Object>,
    ) -> i32 {
        let radius = diameter / 2;
        let above = if radius == 0 { 1 } else { radius };
        let mut clear = true;
        for x in cell.pos.x - radius..cell.pos.x + above {
            for y in cell.pos.y - radius..cell.pos.y + above {
                if radius > 1
                    && (x == cell.pos.x - radius || x == cell.pos.x + above - 1)
                    && (y == cell.pos.y - radius || y == cell.pos.y + above - 1)
                {
                    continue;
                }
                let p = GridPos::new(x, y);
                if !self.is_valid_pos(p) {
                    return 0;
                }
                let actual = self.group_cell(p, cell.layer);
                let layer = PathfindLayerEnum::from_u32(actual.layer as u32);
                if self.resolved_cell_type(layer, p) != PathfindCellType::Clear {
                    clear = false;
                }
                let occ = self.occupancy_bits(p, layer);
                // Original single posUnit lookup, not aggregate player/crush masks.
                if diameter >= 2
                    && occ.pos_unit != 0
                    && !occ.pos_moving
                    && objects
                        .get(&ObjectId(occ.pos_unit))
                        .is_some_and(|o| o.crushable_level > 0)
                {
                    clear = false;
                }
                if !clear {
                    break;
                }
            }
        }
        if clear {
            if radius == 0 { 1 } else { 2 * radius }
        } else if diameter < 2 {
            0
        } else {
            self.group_clearance(cell, diameter - 2, objects)
        }
    }
}

/// Original line iterator includes the intermediate orthogonal cell at every
/// numerator crossing, including the final crossing (AIPathfind.cpp:9097).
fn line_cells(start: GridPos, end: GridPos) -> Vec<GridPos> {
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let sx = if end.x >= start.x { 1 } else { -1 };
    let sy = if end.y >= start.y { 1 } else { -1 };
    let (x1, y1, x2, y2, den, add) = if dx >= dy {
        (0, sy, sx, 0, dx, dy)
    } else {
        (sx, 0, 0, sy, dy, dx)
    };
    let mut num = den / 2;
    let mut p = start;
    let mut out = Vec::new();
    for _ in 0..=den {
        out.push(p);
        num += add;
        if num >= den {
            num -= den;
            p.x += x1;
            p.y += y1;
            out.push(p);
        }
        p.x += x2;
        p.y += y2;
    }
    out
}

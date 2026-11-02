use super::*;
/// Integer `ClipLine2D` (GameClient/Line2D.cpp:33), with widened products
/// so malformed coordinates cannot overflow otherwise safe Rust arithmetic.
pub(super) fn clip(grid: &PathfindingGrid, from: Vec3, to: Vec3) -> (Vec3, Vec3) {
    let cell = |p: Vec3| {
        GridPos::new(
            ((p.x - grid.origin.x) / grid.grid_size).floor() as i32,
            ((p.z - grid.origin.z) / grid.grid_size).floor() as i32,
        )
    };
    let a = cell(from);
    let b = cell(to);
    let Some((ca, cb)) = clip_cells(a, b, grid.width - 1, grid.height - 1) else {
        return (from, to);
    };
    let world = |original: Vec3, before: GridPos, after: GridPos| {
        if before == after {
            original
        } else {
            Vec3::new(
                grid.origin.x + after.x as f32 * grid.grid_size + 0.05,
                original.y,
                grid.origin.z + after.y as f32 * grid.grid_size + 0.05,
            )
        }
    };
    (world(from, a, ca), world(to, b, cb))
}
fn clip_cells(a: GridPos, b: GridPos, right: i32, bottom: i32) -> Option<(GridPos, GridPos)> {
    let code = |p: GridPos| {
        (if p.x < 0 {
            1
        } else if p.x > right {
            2
        } else {
            0
        }) | (if p.y < 0 {
            8
        } else if p.y > bottom {
            4
        } else {
            0
        })
    };
    let c1 = code(a);
    let c2 = code(b);
    if c1 | c2 == 0 {
        return Some((a, b));
    }
    if c1 & c2 != 0 {
        return None;
    }
    let (mut x1, mut y1, mut x2, mut y2) = (
        i128::from(a.x),
        i128::from(a.y),
        i128::from(b.x),
        i128::from(b.y),
    );
    let (right, bottom) = (i128::from(right), i128::from(bottom));
    if c1 != 0 {
        if c1 & 8 != 0 {
            if y2 == y1 {
                return None;
            }
            x1 += (x2 - x1) * (0 - y1) / (y2 - y1);
            y1 = 0;
        } else if c1 & 4 != 0 {
            if y2 == y1 {
                return None;
            }
            x1 += (x2 - x1) * (bottom - y1) / (y2 - y1);
            y1 = bottom;
        }
        if x1 > right {
            if x2 == x1 {
                return None;
            }
            y1 += (y2 - y1) * (right - x1) / (x2 - x1);
            x1 = right;
        } else if x1 < 0 {
            if x2 == x1 {
                return None;
            }
            y1 += (y2 - y1) * (0 - x1) / (x2 - x1);
            x1 = 0;
        }
    }
    if c2 != 0 {
        if c2 & 8 != 0 {
            if y2 == y1 {
                return None;
            }
            x2 += (x2 - x1) * (0 - y2) / (y2 - y1);
            y2 = 0;
        } else if c2 & 4 != 0 {
            if y2 == y1 {
                return None;
            }
            x2 += (x2 - x1) * (bottom - y2) / (y2 - y1);
            y2 = bottom;
        }
        if x2 > right {
            if x2 == x1 {
                return None;
            }
            y2 += (y2 - y1) * (right - x2) / (x2 - x1);
            x2 = right;
        } else if x2 < 0 {
            if x2 == x1 {
                return None;
            }
            y2 += (y2 - y1) * (0 - x2) / (x2 - x1);
            x2 = 0;
        }
    }
    Some((
        GridPos::new(x1 as i32, y1 as i32),
        GridPos::new(x2 as i32, y2 as i32),
    ))
}

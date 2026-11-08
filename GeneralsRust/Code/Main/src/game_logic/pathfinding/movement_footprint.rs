use super::*;

/// Per-search facts used when examining every cell covered by a mover.
pub(super) struct MovementFootprint {
    pub(super) mover: ObjectId,
    pub(super) start: GridPos,
    pub(super) radius: i32,
    pub(super) center_in_cell: bool,
    pub(super) is_infantry: bool,
}

/// C++ `TCheckMovementInfo` results for one footprint (AIPathfind.cpp:4971-5075),
/// with `considerTransient == false` as every caller here uses it.
struct MovementCheck {
    ally_fixed: bool,
    ally_moving: bool,
}

impl MovementFootprint {
    /// C++ `checkForMovement`: `None` when off the map or `enemyFixed`.
    fn check(
        &self,
        grid: &PathfindingGrid,
        position: GridPos,
        layer: PathfindLayerEnum,
    ) -> Option<MovementCheck> {
        let mut result = MovementCheck {
            ally_fixed: false,
            ally_moving: false,
        };
        let above = self.radius + i32::from(self.center_in_cell);
        for x in position.x - self.radius..position.x + above {
            for y in position.y - self.radius..position.y + above {
                let cell = GridPos::new(x, y);
                if !grid.is_valid_pos(cell) {
                    return None;
                }
                let bits = grid.occupancy_bits(cell, layer);
                // NO_UNITS, our own cell, the ignored unit (already filtered) or
                // an occupant no longer in this world (`unit == NULL`).
                if bits.pos_unit == 0 || bits.pos_unit == self.mover.0 || bits.pos_player == 0 {
                    continue;
                }
                if bits.pos_moving {
                    result.ally_moving |= bits.pos_player == OCC_ALLY;
                    continue;
                }
                if self.is_infantry && bits.pos_infantry {
                    continue; // INFANTRY_MOVES_THROUGH_INFANTRY
                }
                if bits.pos_player == OCC_ALLY {
                    result.ally_fixed = true;
                } else if bits.pos_crushable != 0 {
                    return None; // enemyFixed
                }
            }
        }
        Some(result)
    }

    /// `examineNeighboringCells` occupancy cost in C++ cost units
    /// (AIPathfind.cpp:6241-6291): `allyMoving` within 10 cells of the start
    /// and any `allyFixedCount` each add `3*COST_DIAGONAL`; `enemyFixed`
    /// skips the cell.
    pub(super) fn cost(
        &self,
        grid: &PathfindingGrid,
        position: GridPos,
        layer: PathfindLayerEnum,
    ) -> Option<i32> {
        let check = self.check(grid, position, layer)?;
        let near = (position.x - self.start.x).abs() < 10 && (position.y - self.start.y).abs() < 10;
        let mut cost = 0;
        if check.ally_moving && near {
            cost += 3 * COST_DIAGONAL as i32;
        }
        if check.ally_fixed {
            cost += 3 * COST_DIAGONAL as i32;
        }
        Some(cost)
    }

    /// The line-seed callback aborts on `enemyFixed` or any `allyFixedCount`
    /// (AIPathfind.cpp:6034-6050).
    pub(super) fn line_seed_clear(
        &self,
        grid: &PathfindingGrid,
        position: GridPos,
        layer: PathfindLayerEnum,
    ) -> bool {
        self.check(grid, position, layer)
            .is_some_and(|check| !check.ally_fixed)
    }
}

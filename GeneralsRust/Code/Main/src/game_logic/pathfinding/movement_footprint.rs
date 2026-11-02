use super::*;

/// Per-search facts used when examining every cell covered by a mover.
pub(super) struct MovementFootprint {
    pub(super) mover: ObjectId,
    pub(super) start: GridPos,
    pub(super) radius: i32,
    pub(super) center_in_cell: bool,
    pub(super) seeker_player: Option<u32>,
    pub(super) is_infantry: bool,
    pub(super) crusher_level: u8,
    pub(super) ally_mask: u16,
}

impl MovementFootprint {
    pub(super) fn cost(
        &self,
        grid: &PathfindingGrid,
        position: GridPos,
        layer: PathfindLayerEnum,
    ) -> Option<f32> {
        let above = self.radius + i32::from(self.center_in_cell);
        let mut cost = 0.0f32;
        for x in position.x - self.radius..position.x + above {
            for y in position.y - self.radius..position.y + above {
                let cell = GridPos::new(x, y);
                if !grid.is_valid_pos(cell) {
                    return None;
                }
                if grid.occupancy_bits(cell, layer).pos_unit == self.mover.0 {
                    continue;
                }
                let cell_cost = grid.occupancy_cost_on_layer(
                    cell,
                    self.seeker_player,
                    self.is_infantry,
                    self.crusher_level,
                    self.ally_mask,
                    Some(self.start),
                    layer,
                )?;
                cost = cost.max(cell_cost);
            }
        }
        Some(cost)
    }

    /// The line-seed callback rejects fixed allies and non-crushable fixed
    /// enemies; moving units and the mover itself do not block the seed.
    pub(super) fn line_seed_clear(
        &self,
        grid: &PathfindingGrid,
        position: GridPos,
        layer: PathfindLayerEnum,
        objects: HostObjectRead<'_>,
    ) -> bool {
        let above = self.radius + i32::from(self.center_in_cell);
        for x in position.x - self.radius..position.x + above {
            for y in position.y - self.radius..position.y + above {
                let cell = GridPos::new(x, y);
                if !grid.is_valid_pos(cell) {
                    return false;
                }
                let bits = grid.occupancy_bits(cell, layer);
                if bits.pos_unit == 0
                    || bits.pos_unit == self.mover.0
                    || bits.pos_moving
                    || (self.is_infantry && bits.pos_infantry)
                {
                    continue;
                }
                let Some(other) = objects.lookup(&ObjectId(bits.pos_unit)) else {
                    continue;
                };
                let (Some(player), Some(mover)) = (self.seeker_player, objects.lookup(&self.mover))
                else {
                    return false;
                };
                let friends = (1u16 << player.min(15)) | self.ally_mask;
                if bits.pos_player & friends != 0 || !mover.can_crush_or_squish(other, false) {
                    return false;
                }
            }
        }
        true
    }
}

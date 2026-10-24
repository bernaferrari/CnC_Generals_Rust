//! Read the existing receiving scalar cells, never a Core partition mirror.
use super::*;
use crate::object::collide::partition_manager::ValueOrThreat;
use std::collections::VecDeque;

impl ShroudManager {
    pub fn host_scalar_value_at(
        &self,
        position: &Coord3D,
        player: usize,
        value_type: ValueOrThreat,
    ) -> Option<u32> {
        let grid = self.shroud_grid.as_ref()?;
        let (x, y) = grid.world_to_grid(position)?;
        let cell = grid.get_cell(x, y)?;
        Some(match value_type {
            ValueOrThreat::CashValue => cell.get_cash_value(player),
            ValueOrThreat::ThreatValue => cell.get_threat_value(player),
        })
    }

    /// Original iterateCellsBreadthFirst expands the previous curX/curY before
    /// popping the next head. Preserve that observation order, including the
    /// delayed final expansion. Original bottom OOB access has no defined result;
    /// receiving bounds reject it without consulting any other map.
    pub fn nearest_host_scalar_group(
        &self,
        source: &Coord3D,
        mask: u32,
        required: i32,
        value_type: ValueOrThreat,
    ) -> Option<Coord3D> {
        if mask == 0 {
            return None;
        }
        let grid = self.shroud_grid.as_ref()?;
        let start = grid.world_to_grid(source)?;
        let mut previous = start;
        let mut visited = vec![false; grid.width.checked_mul(grid.height)?];
        visited[start.1 * grid.width + start.0] = true;
        let mut queue = VecDeque::from([start]);
        while !queue.is_empty() {
            let (x, y) = (previous.0 as i32, previous.1 as i32);
            for (nx, ny) in [(x - 1, y), (x, y - 1), (x + 1, y), (x, y + 1)] {
                if nx < 0 || ny < 0 || nx as usize >= grid.width || ny as usize >= grid.height {
                    continue;
                }
                let next = (nx as usize, ny as usize);
                let index = next.1 * grid.width + next.0;
                if !visited[index] {
                    visited[index] = true;
                    queue.push_back(next);
                }
            }
            let current = queue.pop_front().expect("nonempty original cell queue");
            previous = current;
            let cell = grid.get_cell(current.0, current.1)?;
            let mut value = 0u32;
            for player in 0..MAX_PLAYER_COUNT {
                if mask & (1u32 << player) != 0 {
                    value = value.wrapping_add(match value_type {
                        ValueOrThreat::CashValue => cell.get_cash_value(player),
                        ValueOrThreat::ThreatValue => cell.get_threat_value(player),
                    });
                }
            }
            // C++ parms.greaterThan = valueRequired, including unsigned comparison.
            let passes = if required != 0 {
                value > required as u32
            } else {
                value < required as u32
            };
            if passes {
                return Some(Coord3D::new(
                    current.0 as f32 * grid.cell_size,
                    current.1 as f32 * grid.cell_size,
                    0.0,
                ));
            }
        }
        None
    }
}

/// Native save DTO of the single receiving grid; not a mutable runtime store.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HostScalarGridSnapshot {
    width: u32,
    height: u32,
    cells: Vec<(u32, [u32; MAX_PLAYER_COUNT], [u32; MAX_PLAYER_COUNT])>,
}
impl HostScalarGridSnapshot {
    pub fn validate_dimensions(&self, expected: Option<[u32; 2]>) -> Result<(), String> {
        if expected != Some([self.width, self.height]) || self.width == 0 || self.height == 0 {
            return Err("native scalar grid does not match the receiving visibility grid".into());
        }
        let count = self
            .width
            .checked_mul(self.height)
            .ok_or("native scalar grid dimensions overflow")?;
        let mut unique = std::collections::HashSet::new();
        for (index, cash, threat) in &self.cells {
            if *index >= count
                || !unique.insert(*index)
                || cash.iter().chain(threat.iter()).all(|v| *v == 0)
            {
                return Err("native scalar grid has invalid/duplicate/redundant cell".into());
            }
        }
        Ok(())
    }
}
impl ShroudManager {
    pub fn capture_host_scalar_grid(&self) -> Result<Option<HostScalarGridSnapshot>, String> {
        let Some(grid) = self.shroud_grid.as_ref() else {
            return Ok(None);
        };
        let width =
            u32::try_from(grid.width).map_err(|_| "scalar width exceeds native snapshot range")?;
        let height = u32::try_from(grid.height)
            .map_err(|_| "scalar height exceeds native snapshot range")?;
        width
            .checked_mul(height)
            .ok_or("scalar cell count exceeds native snapshot range")?;
        let cells = grid
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| {
                if cell
                    .cash_values
                    .iter()
                    .chain(cell.threat_values.iter())
                    .all(|v| *v == 0)
                {
                    None
                } else {
                    Some((index as u32, cell.cash_values, cell.threat_values))
                }
            })
            .collect();
        Ok(Some(HostScalarGridSnapshot {
            width,
            height,
            cells,
        }))
    }
    pub fn clear_host_scalar_grid(&mut self) {
        if let Some(grid) = self.shroud_grid.as_mut() {
            for cell in &mut grid.cells {
                cell.cash_values.fill(0);
                cell.threat_values.fill(0);
            }
        }
    }
    pub fn restore_host_scalar_grid(
        &mut self,
        snapshot: &HostScalarGridSnapshot,
    ) -> Result<(), String> {
        let dimensions = self
            .shroud_grid
            .as_ref()
            .map(|grid| [grid.width as u32, grid.height as u32]);
        snapshot.validate_dimensions(dimensions)?;
        self.clear_host_scalar_grid();
        let grid = self
            .shroud_grid
            .as_mut()
            .expect("validated receiving scalar grid");
        for (index, cash, threat) in &snapshot.cells {
            let cell = &mut grid.cells[*index as usize];
            cell.cash_values = *cash;
            cell.threat_values = *threat;
        }
        Ok(())
    }
}

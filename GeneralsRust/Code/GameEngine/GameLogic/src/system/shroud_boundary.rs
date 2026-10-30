//! C++ PartitionManager::storeFoggedCells / restoreFoggedCells phases.
use super::{MAX_PLAYER_COUNT, ShroudManager, ShroudState};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum StoredCell {
    #[default]
    Untouched,
    Fogged,
    PermanentlyRevealed,
}

/// Owned temporary state for one boundary change. Its dimensions refer to the
/// old grid; restoration clips against the new grid as the original does.
pub struct BoundaryShroudStore {
    width: usize,
    height: usize,
    cells: Vec<[StoredCell; MAX_PLAYER_COUNT]>,
}

impl ShroudManager {
    /// First pass, before objects and their lookers are detached.
    pub fn store_boundary_fogged_cells(&self) -> Result<BoundaryShroudStore, String> {
        let grid = self
            .shroud_grid
            .as_ref()
            .ok_or("boundary change requires a shroud grid")?;
        let mut cells = vec![[StoredCell::Untouched; MAX_PLAYER_COUNT]; grid.cells.len()];
        for (saved, cell) in cells.iter_mut().zip(&grid.cells) {
            for (player, level) in cell.shroud_levels.iter().enumerate() {
                if level.get_shroud_status() == ShroudState::Explored {
                    saved[player] = StoredCell::Fogged;
                }
            }
        }
        Ok(BoundaryShroudStore {
            width: grid.width,
            height: grid.height,
            cells,
        })
    }

    /// Second pass, after object lookers have been detached. A changed grid
    /// here indicates a broken boundary transaction, not an empty result.
    pub fn store_boundary_permanently_revealed_cells(
        &self,
        store: &mut BoundaryShroudStore,
    ) -> Result<(), String> {
        let grid = self
            .shroud_grid
            .as_ref()
            .ok_or("boundary change requires a shroud grid")?;
        if grid.width != store.width || grid.height != store.height {
            return Err("shroud grid changed during boundary capture".into());
        }
        for (saved, cell) in store.cells.iter_mut().zip(&grid.cells) {
            for (player, level) in cell.shroud_levels.iter().enumerate() {
                if level.get_shroud_status() == ShroudState::Visible {
                    saved[player] = StoredCell::PermanentlyRevealed;
                }
            }
        }
        Ok(())
    }

    /// Restore the saved permanent reveal pass before objects are reattached.
    pub fn restore_boundary_permanently_revealed_cells(
        &mut self,
        store: &BoundaryShroudStore,
    ) -> Result<(), String> {
        self.restore_boundary_cells(store, StoredCell::PermanentlyRevealed)
    }

    /// Restore saved fog after objects have re-established their lookers.
    pub fn restore_boundary_fogged_cells(
        &mut self,
        store: &BoundaryShroudStore,
    ) -> Result<(), String> {
        self.restore_boundary_cells(store, StoredCell::Fogged)
    }

    fn restore_boundary_cells(
        &mut self,
        store: &BoundaryShroudStore,
        kind: StoredCell,
    ) -> Result<(), String> {
        let grid = self
            .shroud_grid
            .as_mut()
            .ok_or("boundary change requires a shroud grid")?;
        for player in 0..MAX_PLAYER_COUNT {
            for y in 0..store.height.min(grid.height) {
                for x in 0..store.width.min(grid.width) {
                    let saved = &store.cells[y * store.width + x];
                    let cell = &mut grid.cells[y * grid.width + x];
                    if saved[player] == kind {
                        cell.add_looker(player);
                        if kind == StoredCell::Fogged {
                            cell.remove_looker(player);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_capture_and_restore_clips_without_touching_other_players() {
        let mut shroud = ShroudManager::new();
        shroud
            .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 120.0, 80.0, 40.0)
            .unwrap();
        let grid = shroud.shroud_grid.as_mut().unwrap();
        grid.cells[0].add_looker(0);
        grid.cells[0].remove_looker(0); // fogged before detachment
        grid.cells[1].add_looker(0); // a permanent looker survives detachment
        grid.cells[2].add_looker(1); // clipped by the smaller grid
        let mut saved = shroud.store_boundary_fogged_cells().unwrap();
        shroud
            .store_boundary_permanently_revealed_cells(&mut saved)
            .unwrap();
        shroud
            .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 80.0, 40.0, 40.0)
            .unwrap();
        shroud
            .restore_boundary_permanently_revealed_cells(&saved)
            .unwrap();
        assert_eq!(
            shroud.shroud_grid.as_ref().unwrap().cells[1].shroud_levels[0].get_shroud_status(),
            ShroudState::Visible
        );
        shroud.restore_boundary_fogged_cells(&saved).unwrap();
        let cells = &shroud.shroud_grid.as_ref().unwrap().cells;
        assert_eq!(
            cells[0].shroud_levels[0].get_shroud_status(),
            ShroudState::Explored
        );
        assert_eq!(
            cells[0].shroud_levels[1].get_shroud_status(),
            ShroudState::Hidden
        );
        assert_eq!(
            cells[1].shroud_levels[0].get_shroud_status(),
            ShroudState::Visible
        );
    }
}

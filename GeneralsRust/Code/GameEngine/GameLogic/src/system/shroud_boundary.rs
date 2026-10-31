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
    /// Allocate capture storage before the boundary operation changes its owner.
    /// The actual fog values are read later, after pending unlooks are flushed.
    pub fn prepare_boundary_shroud_store(&self) -> Result<BoundaryShroudStore, String> {
        let grid = self
            .shroud_grid
            .as_ref()
            .ok_or("boundary change requires a shroud grid")?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(grid.cells.len())
            .map_err(|_| "cannot allocate boundary shroud capture".to_string())?;
        cells.resize(grid.cells.len(), [StoredCell::Untouched; MAX_PLAYER_COUNT]);
        Ok(BoundaryShroudStore {
            width: grid.width,
            height: grid.height,
            cells,
        })
    }

    /// First pass, after pending unlooks and before objects are detached.
    pub fn capture_boundary_fogged_cells(
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
            *saved = [StoredCell::Untouched; MAX_PLAYER_COUNT];
            for (player, level) in cell.shroud_levels.iter().enumerate() {
                if level.get_shroud_status() == ShroudState::Explored {
                    saved[player] = StoredCell::Fogged;
                }
            }
        }
        Ok(())
    }

    /// Second pass, after objects are detached and their look receipts reset.
    /// The original does not remove those existing lookers at this point, so
    /// currently clear cells are captured even if an object made them clear.
    /// A changed grid indicates a broken transaction, not an empty result.
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
    fn preparation_is_inert_and_fog_is_captured_only_at_the_ordered_phase() {
        let mut owner = ShroudManager::new();
        owner.reveal_map_for_player_permanently(1).unwrap();
        let pending = owner.snapshot_state();
        assert!(owner.prepare_boundary_shroud_store().is_err());
        assert_eq!(owner.snapshot_state(), pending);

        owner
            .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 80.0, 40.0, 40.0)
            .unwrap();
        owner.undo_reveal_map_for_player_permanently(1).unwrap();
        let before = owner.snapshot_state();
        let mut store = owner.prepare_boundary_shroud_store().unwrap();
        assert_eq!(owner.snapshot_state(), before);
        let cell = &mut owner.shroud_grid.as_mut().unwrap().cells[0];
        cell.add_looker(0);
        cell.remove_looker(0);
        owner.capture_boundary_fogged_cells(&mut store).unwrap();
        assert!(store.cells[0][0] == StoredCell::Fogged);
        let captured = store.cells.clone();
        owner
            .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 40.0, 40.0, 40.0)
            .unwrap();
        assert!(owner.capture_boundary_fogged_cells(&mut store).is_err());
        assert!(
            store.cells == captured,
            "shape rejection must not alter the capture"
        );
    }

    #[test]
    fn ordered_capture_and_restore_clips_without_touching_other_players() {
        let mut shroud = ShroudManager::new();
        shroud
            .init_shroud_grid_at_origin_with_cell_size([0.0, 0.0], 120.0, 80.0, 40.0)
            .unwrap();
        let grid = shroud.shroud_grid.as_mut().unwrap();
        grid.cells[0].add_looker(0);
        grid.cells[0].remove_looker(0); // fogged before detachment
        // An object looker is still in the grid when its receipt is reset.
        // C++ stores this clear cell as permanently revealed on the second pass.
        grid.cells[1].add_looker(0);
        grid.cells[2].add_looker(1); // clipped by the smaller grid
        let mut saved = shroud.prepare_boundary_shroud_store().unwrap();
        shroud.capture_boundary_fogged_cells(&mut saved).unwrap();
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

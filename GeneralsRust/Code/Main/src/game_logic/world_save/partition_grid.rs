//! Receiving PartitionManager::init phase; no process configuration selection.
use super::*;

impl GameLogic {
    /// The existing private GameData policy captures startup and ordered map
    /// overrides. PM alone clamps it; the camera retains the raw source rule.
    pub(crate) fn initialize_receiving_partition_grid(&mut self) -> Result<(), String> {
        let cell_size = self
            .session_view
            .camera
            .movement_policy
            .current()
            .partition_cell_size;
        let mut shroud = self
            .world_services
            .shroud()
            .lock()
            .map_err(|_| "receiving shroud owner is poisoned".to_string())?;
        shroud.init_shroud_grid_at_origin_with_cell_size(
            [self.world_min.x, self.world_min.z],
            self.world_width,
            self.world_height,
            cell_size,
        )?;
        self.partition_manager.configure_grid_from_shroud(&shroud);
        Ok(())
    }

    pub(crate) fn restore_receiving_partition_grid(
        &mut self,
        snapshot: &gamelogic::system::shroud_manager::ShroudSnapshot,
        frame: u32,
    ) -> Result<(), String> {
        let mut shroud = self
            .world_services
            .shroud()
            .lock()
            .map_err(|_| "receiving shroud owner is poisoned".to_string())?;
        shroud.replace_state_owned(snapshot, frame)?;
        self.partition_manager.configure_grid_from_shroud(&shroud);
        Ok(())
    }
}

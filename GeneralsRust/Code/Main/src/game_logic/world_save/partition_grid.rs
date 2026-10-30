//! Receiving PartitionManager::init phase; no process configuration selection.
use super::*;

impl GameLogic {
    /// C++ Object::friend_notifyOfNewMapBoundary refreshes every live object.
    pub(crate) fn refresh_receiving_object_map_status(&mut self) {
        self.partition_manager
            .set_map_extent(self.world_min, self.world_max);
        for (_, object) in self.objects.registered_mut() {
            self.partition_manager.refresh_object_map_status(object);
        }
    }
    /// The existing private GameData policy captures startup and ordered map
    /// overrides. PM alone clamps it; the camera retains the raw source rule.
    pub(crate) fn initialize_receiving_partition_grid(&mut self) -> Result<(), String> {
        self.objects
            .retire_owned_scalar_stamps(&mut self.partition_manager);
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
        drop(shroud);
        self.refresh_receiving_object_map_status();
        Ok(())
    }

    pub(crate) fn restore_receiving_partition_grid(
        &mut self,
        snapshot: &gamelogic::system::shroud_manager::ShroudSnapshot,
        frame: u32,
    ) -> Result<(), String> {
        self.objects
            .retire_owned_scalar_stamps(&mut self.partition_manager);
        let mut shroud = self
            .world_services
            .shroud()
            .lock()
            .map_err(|_| "receiving shroud owner is poisoned".to_string())?;
        shroud.replace_state_owned(snapshot, frame)?;
        self.partition_manager.configure_grid_from_shroud(&shroud);
        drop(shroud);
        self.partition_manager
            .set_map_extent(self.world_min, self.world_max);
        Ok(())
    }
}

//! Receiving PartitionManager::init phase; no process configuration selection.
use super::*;

impl GameLogic {
    /// The active terrain boundary is distinct from the full map or path grid.
    pub(crate) fn sync_receiving_map_extent(&mut self) {
        let extent = {
            let terrain = self.world_services.terrain();
            let terrain = terrain.read().unwrap_or_else(|error| error.into_inner());
            terrain.has_height_map().then(|| terrain.get_extent())
        };
        let (min, max) = extent.map_or((self.world_min, self.world_max), |extent| {
            (
                Vec3::new(extent.lo.x, extent.lo.z, extent.lo.y),
                Vec3::new(extent.hi.x, extent.hi.z, extent.hi.y),
            )
        });
        self.partition_manager.set_map_extent(min, max);
    }

    /// C++ Object::friend_notifyOfNewMapBoundary refreshes every live object.
    pub(crate) fn refresh_receiving_object_map_status(&mut self) {
        self.sync_receiving_map_extent();
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
        self.sync_receiving_map_extent();
        Ok(())
    }
}

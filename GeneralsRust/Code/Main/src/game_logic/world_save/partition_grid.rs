//! Receiving PartitionManager::init phase; no process configuration selection.
use super::*;

impl GameLogic {
    /// TerrainLogic::setActiveBoundary, driven by this session's owners.
    pub(crate) fn switch_owned_map_boundary(
        &mut self,
        boundary: i32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        let terrain_owner = self.world_services.terrain().clone();
        let target_extent = {
            let terrain = terrain_owner
                .read()
                .map_err(|_| "receiving terrain owner is poisoned")?;
            (boundary != terrain.get_active_boundary())
                .then(|| terrain.extent_for_saved_boundary(boundary))
                .flatten()
        };
        // Allocate before undoing observer reveal, selecting the boundary or
        // detaching any objects. A failed allocation must leave this owner intact.
        let prepared_grid = if let Some(extent) = target_extent {
            let width = extent.hi.x - extent.lo.x;
            let height = extent.hi.y - extent.lo.y;
            if width.abs() <= f32::EPSILON || height.abs() <= f32::EPSILON {
                return Err("boundary has invalid radar extent".into());
            }
            let cell_size = self
                .session_view
                .camera
                .movement_policy
                .current()
                .partition_cell_size;
            Some(
                gamelogic::system::shroud_manager::ShroudManager::prepare_shroud_grid(
                    [extent.lo.x, extent.lo.y],
                    width,
                    height,
                    cell_size,
                )?,
            )
        } else {
            None
        };
        let shroud_owner = self.world_services.shroud().clone();
        let prepared = if let Some(grid) = prepared_grid {
            let store = shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned")?
                .prepare_boundary_shroud_store()?;
            Some((grid, store))
        } else {
            None
        };
        let observer = self
            .players
            .values()
            .find(|player| player.name == "ReplayObserver")
            .map(|player| player.id);
        if let Some(observer) = observer {
            shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned")?
                .undo_reveal_map_for_player_permanently(observer)?;
        }
        if let Some((prepared_grid, mut saved)) = prepared {
            {
                let mut shroud = shroud_owner
                    .lock()
                    .map_err(|_| "receiving shroud owner is poisoned")?;
                shroud.process_entire_pending_undo_shroud_reveal_queue();
                shroud.capture_boundary_fogged_cells(&mut saved)?;
            }
            terrain_owner
                .write()
                .map_err(|_| "receiving terrain owner is poisoned")?
                .restore_active_boundary_from_snapshot(boundary);
            self.w3d_ghosts.release_partition_data();
            let ids = self.objects.ordered_ids().to_vec();
            for id in &ids {
                self.host_radar_remove_object(*id);
                self.partition_manager.unregister_object(id.0);
                self.objects
                    .get_mut(id)
                    .expect("registered boundary object")
                    .forget_retired_partition_scalar_receipt();
            }
            self.vision_last_looks.clear();
            self.vision_last_reveal_all.clear();
            self.vision_last_shroud.clear();
            shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned")?
                .store_boundary_permanently_revealed_cells(&mut saved)?;
            self.install_receiving_partition_grid(prepared_grid, false)?;
            let (min, max) = self.receiving_partition_extent();
            if !self.host_radar_new_map_with_extent(min, max) {
                return Err("boundary has invalid radar extent".into());
            }
            shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned")?
                .restore_boundary_permanently_revealed_cells(&saved)?;
            self.w3d_ghosts.set_lock_ghost_objects(true);
            let mut notification_result = Ok(());
            for id in &ids {
                let object = self
                    .objects
                    .get_mut(id)
                    .expect("registered boundary object");
                self.partition_manager.attach_receiving_object(object);
                self.host_radar_add_object_with_effects(*id, effects);
                if let Err(error) = self.readmit_boundary_pathfinder_object(*id) {
                    notification_result = Err(error);
                    break;
                }
                if let Err(error) = self.relook_boundary_object(*id) {
                    notification_result = Err(error);
                    break;
                }
                let object = self
                    .objects
                    .get_mut(id)
                    .expect("registered boundary object");
                object.handle_partition_cell_maintenance_with_partition_owner(
                    &mut self.partition_manager,
                );
                self.partition_manager.refresh_object_map_status(object);
            }
            if notification_result.is_ok() {
                self.update_main_crate_vision();
            }
            let fog_result = shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned".to_owned())
                .and_then(|mut shroud| shroud.restore_boundary_fogged_cells(&saved));
            self.w3d_ghosts.restore_partition_data();
            self.w3d_ghosts.set_lock_ghost_objects(false);
            notification_result?;
            fog_result?;
            self.session_view.camera.constraints.invalidate();
        }
        if let Some(observer) = observer {
            shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned")?
                .reveal_map_for_player_permanently(observer)?;
        }
        if let Some(player) = self.local_player_id() {
            let mut shroud = shroud_owner
                .lock()
                .map_err(|_| "receiving shroud owner is poisoned")?;
            shroud.refresh_radar_shroud_for_player_into(player, self.frame, &mut self.radar);
        }
        Ok(())
    }
    /// Object::friend_notifyOfNewMapBoundary re-adds the object to this
    /// session's existing pathfinder; preparation deliberately did not remove it.
    fn readmit_boundary_pathfinder_object(&mut self, id: ObjectId) -> Result<(), String> {
        let Some(object) = self.objects.get(&id) else {
            return Err("boundary pathfinder object disappeared".into());
        };
        let terrain = self
            .world_services
            .terrain()
            .read()
            .map_err(|_| "receiving terrain owner is poisoned")?;
        let ground = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
            &terrain,
            self.map_loaded,
            self.terrain.as_ref(),
            self.pathfinding_height_samples.as_ref(),
            &self.pathfinding_system.grid,
        );
        self.pathfinding_system
            .classify_and_pinch_object(object, &ground);
        Ok(())
    }

    /// Validate against the saved boundary without changing the receiving world.
    pub(crate) fn validate_saved_partition_geometry(
        &self,
        snapshot: &gamelogic::system::shroud_manager::ShroudSnapshot,
        boundary: i32,
    ) -> Result<(), String> {
        let terrain = self.world_services.terrain();
        let terrain = terrain
            .read()
            .map_err(|_| "receiving terrain owner is poisoned")?;
        let shroud = self.world_services.shroud();
        let shroud = shroud
            .lock()
            .map_err(|_| "receiving shroud owner is poisoned")?;
        if !terrain.has_height_map() || snapshot.grid.is_none() {
            return shroud.validate_snapshot_geometry(snapshot);
        }
        let extent = terrain
            .extent_for_saved_boundary(boundary)
            .ok_or("saved terrain boundary does not exist")?;
        let cell_size = shroud.grid_dimensions().map(|(_, _, size)| size).unwrap_or(
            self.session_view
                .camera
                .movement_policy
                .current()
                .partition_cell_size,
        );
        shroud.validate_snapshot_for_extent(
            snapshot,
            [extent.lo.x, extent.lo.y],
            extent.hi.x - extent.lo.x,
            extent.hi.y - extent.lo.y,
            cell_size,
        )
    }

    /// Prepare the saved boundary's grid after preflight and before shroud xfer.
    pub(crate) fn prepare_saved_partition_boundary(&mut self, boundary: i32) -> Result<(), String> {
        let terrain = self.world_services.terrain().clone();
        {
            let mut terrain = terrain
                .write()
                .map_err(|_| "receiving terrain owner is poisoned")?;
            if !terrain.has_height_map() || terrain.get_active_boundary() == boundary {
                return Ok(());
            }
            terrain
                .extent_for_saved_boundary(boundary)
                .ok_or("saved terrain boundary does not exist")?;
            terrain.restore_active_boundary_from_snapshot(boundary);
        }
        self.initialize_receiving_partition_grid()
    }
    /// The active terrain boundary is distinct from the full map or path grid.
    fn receiving_partition_extent(&self) -> (Vec3, Vec3) {
        let extent = {
            let terrain = self.world_services.terrain();
            let terrain = terrain.read().unwrap_or_else(|error| error.into_inner());
            terrain.has_height_map().then(|| terrain.get_extent())
        };
        extent.map_or((self.world_min, self.world_max), |extent| {
            (
                Vec3::new(extent.lo.x, extent.lo.z, extent.lo.y),
                Vec3::new(extent.hi.x, extent.hi.z, extent.hi.y),
            )
        })
    }

    pub(crate) fn sync_receiving_map_extent(&mut self) {
        let (min, max) = self.receiving_partition_extent();
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
        let cell_size = self
            .session_view
            .camera
            .movement_policy
            .current()
            .partition_cell_size;
        let (min, max) = self.receiving_partition_extent();
        let prepared = gamelogic::system::shroud_manager::ShroudManager::prepare_shroud_grid(
            [min.x, min.z],
            max.x - min.x,
            max.z - min.z,
            cell_size,
        )?;
        self.install_receiving_partition_grid(prepared, true)
    }

    fn install_receiving_partition_grid(
        &mut self,
        prepared: gamelogic::system::shroud_manager::PreparedShroudGrid,
        refresh_objects: bool,
    ) -> Result<(), String> {
        let mut shroud = self
            .world_services
            .shroud()
            .lock()
            .map_err(|_| "receiving shroud owner is poisoned".to_string())?;
        self.objects
            .retire_owned_scalar_stamps(&mut self.partition_manager);
        shroud.install_prepared_shroud_grid(prepared);
        self.partition_manager.configure_grid_from_shroud(&shroud);
        drop(shroud);
        if refresh_objects {
            self.refresh_receiving_object_map_status();
        } else {
            self.sync_receiving_map_extent();
        }
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

use game_engine::System::SaveGame::{SavedMapRuntime, SavedMapState};

impl SavedMapRuntime for GameLogic {
    fn capture_saved_map(&mut self) -> Result<SavedMapState, XferStatus> {
        let world = self
            .owning_world_services()
            .ok_or(XferStatus::InvalidData)?;
        let common = world.common();
        let map_name = common
            .system
            .global_data
            .lock()
            .map_err(|_| XferStatus::InvalidData)?
            .as_ref()
            .map(|data| data.read().map_name.clone())
            .unwrap_or_default();
        let next_drawable_id =
            game_engine::System::SaveGame::get_runtime_drawable_id_counter_in(common)
                .unwrap_or_else(|| {
                    world
                        .session_flags()
                        .next_drawable_id
                        .load(Ordering::Relaxed)
                });
        Ok(SavedMapState {
            map_name,
            game_mode: self.get_game_mode(),
            next_object_id: self.get_object_id_counter(),
            next_drawable_id,
            skirmish: self.saved_skirmish_info.clone(),
        })
    }

    fn begin_saved_map_load(&mut self) {
        self.clear_all_objects();
        self.rebuild_spatial_index_from_live_objects();
        self.set_loading_map(true);
        self.set_loading_save(true);
    }

    fn start_saved_map(&mut self, saved: SavedMapState) -> Result<(), XferStatus> {
        use crate::system::map_loader::MapLoader;
        use crate::terrain::TerrainLogic;

        let world = self
            .owning_world_services()
            .ok_or(XferStatus::InvalidData)?;
        let common = world.common();
        let path = std::path::Path::new(&saved.map_name);
        let geometry = MapLoader::load(path).map_err(|error| {
            warn!("Saved map {}: {error:?}", path.display());
            XferStatus::InvalidData
        })?;
        TerrainLogic::validate_map_geometry(&geometry).map_err(|error| {
            warn!("Saved map geometry: {error}");
            XferStatus::InvalidData
        })?;

        // GameStateMap.cpp:379–426 installs both counters before map startup.
        self.set_game_mode(saved.game_mode);
        self.set_object_id_counter(saved.next_object_id);
        world
            .session_flags()
            .next_drawable_id
            .store(saved.next_drawable_id, Ordering::Relaxed);
        game_engine::System::SaveGame::set_runtime_drawable_id_counter_in(
            common,
            saved.next_drawable_id,
        );
        self.saved_skirmish_info = saved.skirmish;
        let definitions = common
            .system
            .global_data
            .lock()
            .map_err(|_| XferStatus::InvalidData)?
            .clone();
        if let Some(data) = definitions.as_ref() {
            data.write().map_name.clone_from(&saved.map_name);
        }

        {
            let mut terrain = world
                .terrain()
                .write()
                .map_err(|_| XferStatus::InvalidData)?;
            terrain.reset();
            terrain.set_source_filename(saved.map_name.as_str().into());
            let bridges = terrain.try_load_map_geometry(geometry).map_err(|error| {
                warn!("Saved map admission: {error}");
                XferStatus::InvalidData
            })?;
            let pathfinder = self
                .pathfinding_owner
                .as_ref()
                .and_then(|owner| owner.ai().read().ok()?.pathfinder());
            let mut pathfinder = pathfinder
                .as_ref()
                .map(|pathfinder| pathfinder.write().map_err(|_| XferStatus::InvalidData))
                .transpose()?;
            // A newly admitted map owns fresh layers even if the previous
            // map happened to have the same dimensions.
            if let Some(pathfinder) = pathfinder.as_deref_mut() {
                pathfinder.reset();
            }
            for (bridge, template) in bridges {
                let layer = pathfinder
                    .as_deref_mut()
                    .map(|pathfinder| TerrainLogic::register_bridge_in(pathfinder, &bridge))
                    .unwrap_or(crate::path::PathfindLayerEnum::Ground);
                terrain.prepend_bridge_on_layer(bridge, template, layer);
            }
            let definitions = definitions.as_ref().map(|data| data.read());
            terrain.new_map_with_water_configuration(
                true,
                &saved.map_name,
                definitions.as_deref(),
                None,
            );
        }

        self.rebuild_selection_cache();
        if self.pathfinding_owner.is_some() {
            pathfinder_new_map_after_polygon_load(self)?;
        }
        Ok(())
    }

    fn end_saved_map_load(&mut self) {
        self.set_loading_save(false);
        self.set_loading_map(false);
    }

    fn save_lock_ghost_objects(&mut self, enable: bool) {
        if let Some(world) = self.owning_world_services() {
            if let Ok(mut ghosts) = world.ghost_objects().write() {
                ghosts.save_lock_ghost_objects(enable);
            }
            if let Ok(mut ghosts) = world.w3d_ghost_objects().write() {
                ghosts.set_save_lock_ghost_objects(enable);
            }
        }
    }

    fn finish_saved_game_load(&mut self) -> Result<(), XferStatus> {
        let mut partition = self
            .partition
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        partition.update_dirty_geometry();
        partition.refresh_host_positions(self.live_object_positions());
        Ok(())
    }

    fn abort_saved_game_load(&mut self) {
        self.clear_all_objects();
        self.end_saved_map_load();
    }
}

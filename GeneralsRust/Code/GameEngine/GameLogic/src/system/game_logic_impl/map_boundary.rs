//! Original TerrainLogic::setActiveBoundary, driven by the receiving match.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptWorldRequest};
use crate::system::engine_stores::WorldServices;
use crate::system::shroud_manager::ShroudManager;
use crate::system::world_ctx::ModuleCtx;
use game_engine::common::system::radar::{
    CellShroudStatus, Coord3D as RadarCoord, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, RadarSystem,
    sample_radar_map_grid,
};

impl GameLogic {
    /// TerrainLogic.cpp:2545–2615. The largest pathfinder grid stays resident.
    pub(super) fn switch_active_map_boundary(&mut self, boundary: Int) -> Result<(), String> {
        let world = self
            .owning_world_services()
            .ok_or("boundary change requires a world")?;
        let (previous, next) = {
            let terrain = world.terrain().read().map_err(|e| e.to_string())?;
            if boundary == terrain.get_active_boundary() {
                return Ok(());
            }
            let Some(next) = terrain.extent_for_saved_boundary(boundary) else {
                return Ok(());
            };
            (terrain.get_extent(), next)
        };
        let width = next.hi.x - next.lo.x;
        let height = next.hi.y - next.lo.y;
        let old_width = previous.hi.x - previous.lo.x;
        let old_height = previous.hi.y - previous.lo.y;

        let ids = self.get_all_object_ids();
        let pathfinder = if ids.is_empty() {
            None
        } else {
            let core = self
                .pathfinding_owner
                .as_ref()
                .ok_or("boundary actors require a Core AI")?;
            let ai = core
                .ai()
                .try_read()
                .map_err(|e| format!("boundary AI: {e}"))?;
            Some(
                ai.pathfinder()
                    .ok_or("boundary actors require a pathfinder")?,
            )
        };
        let mut pathfinder = pathfinder
            .as_ref()
            .map(|pathfinder| {
                pathfinder
                    .try_write()
                    .map_err(|e| format!("boundary pathfinder: {e}"))
            })
            .transpose()?;
        let cell_size = world
            .common()
            .system
            .global_data
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .map(|data| data.read().partition_cell_size.max(1.0))
            .unwrap_or_else(PartitionManager::partition_cell_size);
        let replacement =
            ShroudManager::prepare_shroud_grid([next.lo.x, next.lo.y], width, height, cell_size)?;
        let min = RadarCoord::new(next.lo.x, next.lo.y, next.lo.z);
        let max = RadarCoord::new(next.hi.x, next.hi.y, next.hi.z);
        let heights = {
            let terrain = world.terrain().read().map_err(|e| e.to_string())?;
            sample_radar_map_grid(
                min,
                width / RADAR_CELL_WIDTH as f32,
                height / RADAR_CELL_HEIGHT as f32,
                |x, y| {
                    let mut z = 0.0;
                    let water = terrain.is_underwater(x, y, None, Some(&mut z));
                    Some((z, water))
                },
            )
        };
        let radar = boundary_radar(&world);
        let paint = world
            .common()
            .system
            .radar_paint_source
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let local_player = world
            .players()
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .get_local_player()
            .cloned();

        let mut saved = {
            let mut shroud = world.shroud().lock().map_err(|e| e.to_string())?;
            let mut saved = shroud
                .has_shroud_grid()
                .then(|| shroud.prepare_boundary_shroud_store())
                .transpose()?;
            shroud.process_entire_pending_undo_shroud_reveal_queue();
            if let Some(saved) = &mut saved {
                shroud.capture_boundary_fogged_cells(saved)?;
            }
            saved
        };
        let fogged = self.partition_manager().capture_boundary_shroud(
            old_width,
            old_height,
            CellShroudStatus::Fogged,
        );
        world
            .terrain()
            .write()
            .map_err(|e| e.to_string())?
            .restore_active_boundary_from_snapshot(boundary);

        let mut ghosts = world
            .ghost_objects()
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let mut w3d_ghosts = world
            .w3d_ghost_objects()
            .write()
            .unwrap_or_else(|error| error.into_inner());
        ghosts.release_partition_data();
        w3d_ghosts.release_partition_data();
        for id in ids {
            self.objects[id]
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .detach_map_boundary_observations();
        }

        if let Some(saved) = &mut saved {
            world
                .shroud()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .store_boundary_permanently_revealed_cells(saved)?;
        }
        let revealed = {
            let mut partition = self
                .partition
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let revealed =
                partition.capture_boundary_shroud(old_width, old_height, CellShroudStatus::Clear);
            partition.reset_host_boundary_index();
            revealed
        };
        world
            .shroud()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .install_prepared_shroud_grid(replacement);
        radar
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .new_map_owned_with_paint(min, max, &heights, paint.as_deref());
        if let Some(saved) = &saved {
            world
                .shroud()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .restore_boundary_permanently_revealed_cells(saved)?;
        }
        self.partition
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .restore_boundary_shroud(revealed, width, height, false);

        ghosts.lock_ghost_objects(true);
        w3d_ghosts.set_lock_ghost_objects(true);
        let ctx = ModuleCtx {
            partition: &self.partition,
            shroud: world.shroud(),
            objects: world.objects(),
            players: world.players(),
            frame: self.frame,
            unlook_persist_frames: world
                .common()
                .system
                .runtime_global_data
                .read()
                .unlook_persist_duration,
        };
        let mut notification = Ok(());
        for id in ids {
            let mut object = self.objects[id]
                .write()
                .unwrap_or_else(|error| error.into_inner());
            let pos = *object.get_position();
            self.partition
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .add_object(*id, (pos.x, pos.y, pos.z));
            object.restore_map_boundary_radar(
                &mut radar.write().unwrap_or_else(|error| error.into_inner()),
                local_player.as_ref(),
            );
            if let Some(pathfinder) = pathfinder.as_deref_mut() {
                let result = pathfinder.add_object_to_pathfind_map(&object);
                notification = notification.and(result);
            }
            ctx.maintain_partition_cell(&mut object);
            object.refresh_off_map_status(&next);
        }
        drop(pathfinder);

        let restored_fog = if let Some(saved) = &saved {
            world
                .shroud()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .restore_boundary_fogged_cells(saved)
        } else {
            Ok(())
        };
        self.partition_manager_mut()
            .restore_boundary_shroud(fogged, width, height, true);
        ghosts.restore_partition_data();
        ghosts.lock_ghost_objects(false);
        w3d_ghosts.restore_partition_data();
        w3d_ghosts.set_lock_ghost_objects(false);
        drop(ghosts);
        drop(w3d_ghosts);

        if let Some(recalc) = world.client_visuals().recalc_camera_constraints.get() {
            recalc();
        }
        notification.and(restored_fog)
    }

    /// ScriptActions.cpp:4909–4937 brackets the resize with observer vision.
    pub(crate) fn switch_script_map_boundary(
        &mut self,
        boundary: Int,
    ) -> crate::GameLogicResult<()> {
        let world = self.owning_world_services().ok_or_else(|| {
            crate::GameLogicError::Configuration("boundary script requires a world".into())
        })?;
        let (observer, local) = {
            let players = world
                .players()
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let observer = players.find_player_by_name("ReplayObserver").map(|player| {
                player
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_player_index()
            });
            (observer, players.get_local_player_index())
        };
        let shroud = world.shroud();
        if let Some(observer) = observer {
            shroud
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .undo_reveal_map_for_player_permanently(observer as u32)
                .map_err(crate::GameLogicError::Configuration)?;
        }
        let change = self.switch_active_map_boundary(boundary);
        let mut shroud = shroud.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(observer) = observer {
            shroud
                .reveal_map_for_player_permanently(observer as u32)
                .map_err(crate::GameLogicError::Configuration)?;
        }
        if local != crate::player::PLAYER_INDEX_INVALID {
            let radar = boundary_radar(&world);
            let mut radar = radar.write().unwrap_or_else(|error| error.into_inner());
            shroud.refresh_radar_shroud_for_player_into(local as u32, self.frame, &mut radar);
        }
        change.map_err(crate::GameLogicError::Configuration)
    }
}

fn boundary_radar(world: &WorldServices) -> Arc<RwLock<RadarSystem>> {
    let mut slot = world
        .common()
        .system
        .radar_system
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    Arc::clone(slot.get_or_insert_with(|| Arc::new(RwLock::new(RadarSystem::new()))))
}

/// The regular Core script phase already has the only mutable GameLogic loan.
pub(super) struct CoreBoundaryScriptDriver<'a>(pub(super) &'a mut GameLogic);

impl ScriptExecutionDriver<crate::scripting::engine::ScriptEngine> for CoreBoundaryScriptDriver<'_> {
    fn world(&mut self, request: ScriptWorldRequest<'_>) -> Option<crate::GameLogicResult<()>> {
        match request {
            ScriptWorldRequest::MapSwitchBoundary(boundary) => {
                Some(self.0.switch_script_map_boundary(boundary))
            }
            _ => None,
        }
    }

    fn set_show_behind_building_markers(
        &mut self,
        enabled: bool,
    ) -> Option<crate::GameLogicResult<()>> {
        self.0.set_show_behind_building_markers(enabled);
        Some(Ok(()))
    }

    fn after_action(&mut self) -> crate::GameLogicResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

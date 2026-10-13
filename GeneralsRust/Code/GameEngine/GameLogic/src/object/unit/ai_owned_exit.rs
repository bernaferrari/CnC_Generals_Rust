//! Original ordinary/quick contain-exit path effects on this installed AI.
use super::ai_core::{UnitAIUpdate, UnitAiRuntime};
use super::imports::*;

impl UnitAIUpdate {
    pub(super) fn validate_removal_identity_owned(
        &self,
        owner: &Arc<RwLock<Object>>,
    ) -> Result<(), String> {
        let actual = self
            .runtime
            .native_owner()
            .ok_or("removal AI owner missing")?;
        if !Arc::ptr_eq(&actual, owner) || self.ai_state_machine.is_none() {
            return Err("removal AI does not belong to this installed owner".into());
        }
        Ok(())
    }
    pub(super) fn validate_removed_parent_owned(&self) -> Result<(), String> {
        if self
            .runtime
            .native_owner()
            .ok_or("removed owner absent")?
            .read()
            .map_err(|_| "removed owner poisoned")?
            .is_effectively_dead()
        {
            return Ok(()); // Original aiMoveToPosition will ignore the command.
        }
        let machine = self
            .ai_state_machine
            .as_ref()
            .ok_or("removed movement machine missing")?;
        if machine.get_temporary_state().is_some()
            || !matches!(machine.get_current_state_id(), Some(id) if id == AIStateType::Idle as u32 || id == AIStateType::Enter as u32 || id == AIStateType::Busy as u32)
        {
            return Err("removed move previous state not admitted".into());
        }
        Ok(())
    }
    pub(super) fn validate_exit_occupancy_owned(
        &self,
        world: &dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        self.validate_world_owner(world)
            .map_err(|error| error.to_string())?;
        let owner = self.runtime.native_owner().ok_or("exit vehicle absent")?;
        let owner = owner.read().map_err(|_| "exit vehicle poisoned")?;
        if !self
            .runtime
            .is_doing_ground_movement_for_owner_with_world(&owner, Some(world))?
        {
            return Err("vehicle exit refresh requires supplied ground locomotor".into());
        }
        world.layer_for_destination(owner.get_position())?;
        Ok(())
    }
    pub(super) fn refresh_exit_occupancy_owned(
        &mut self,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        self.validate_world_owner(world)
            .map_err(|error| error.to_string())?;
        let owner = self.runtime.native_owner().ok_or("exit vehicle absent")?;
        let (id, position, layer, radius, ground, immobile) = {
            let owner = owner.read().map_err(|_| "exit vehicle poisoned")?;
            (
                owner.get_id(),
                *owner.get_position(),
                owner.get_layer(),
                owner.get_geometry_info().get_bounding_circle_radius(),
                self.runtime
                    .is_doing_ground_movement_for_owner_with_world(&owner, Some(world))?,
                owner.is_kind_of(KindOf::Immobile),
            )
        };
        if !ground {
            return Err("vehicle exit refresh requires supplied ground locomotor".into());
        }
        if immobile {
            return Ok(());
        }
        let goal_layer = world.layer_for_destination(&position)?;
        let (radius_cells, center) =
            crate::ai::pathfind_complete::PathfindingSystem::compute_radius_and_center(radius);
        let cell = UnitAiRuntime::compute_goal_cell(&position, center);
        let size = crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F;
        let mut diameter = 2.0 * radius;
        if diameter > size && diameter < 2.0 * size {
            diameter = 2.0 * size;
        }
        let diameter = ((diameter / size + 0.3).floor() as i32).max(1);
        let mut goal_center = diameter & 1 != 0;
        let mut goal_radius = diameter / 2;
        if goal_radius > 2 {
            goal_radius = 2;
            goal_center = true;
        }
        let goal_cell = UnitAiRuntime::compute_goal_cell(&position, goal_center);
        let bridge = if goal_layer != crate::common::PathfindLayerEnum::Ground {
            let owner = owner.read().map_err(|_| "exit vehicle poisoned")?;
            world.bridge_layer_interaction(&owner, goal_layer)?
        } else {
            false
        };
        let pathfinder = world.pathfinder().ok_or("exit vehicle pathfinder absent")?;
        let path_layer = crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32);
        pathfinder.remove_unit_from_pathfind_map(id, radius_cells, center, path_layer);
        self.runtime.data.pathfind_cur_cell = ICoord2D::new(-1, -1);
        self.runtime.data.pathfind_goal_cell = ICoord2D::new(-1, -1);
        pathfinder.update_pos_cells(
            crate::ai::pathfind_astar::GridCoord::new(cell.x, cell.y),
            id,
            path_layer,
            radius_cells,
            center,
            bridge,
        );
        if pathfinder.is_map_ready() {
            self.runtime.data.pathfind_cur_cell = cell;
        }
        owner
            .write()
            .map_err(|_| "exit vehicle poisoned")?
            .set_destination_layer(goal_layer);
        self.runtime.update_ground_goal_cells(
            pathfinder,
            id,
            goal_cell,
            crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(goal_layer as u32),
            goal_radius,
            goal_center,
            bridge,
        );
        Ok(())
    }
    pub(super) fn finish_contain_exit_path_owned(
        &mut self,
        follow_owner: Option<ObjectID>,
        mut end: Coord3D,
        rally: Option<Coord3D>,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        self.validate_world_owner(world)
            .map_err(|error| error.to_string())?;
        let original_end = end;
        self.runtime.data.ignore_obstacle_id = INVALID_ID;
        self.runtime.data.ignore_collisions_until =
            world.frame().wrapping_add(LOGICFRAMES_PER_SECOND);
        {
            let owner = self.runtime.native_owner().ok_or("exit actor absent")?;
            let object = owner.read().map_err(|_| "exit actor poisoned")?;
            world.adjust_exit_destination(&object, &self.runtime.data.locomotor_set, &mut end)?;
        }
        // Hurry records its first node before destination adjustment; ordinary
        // exit duplicates the adjusted node (OpenContain.cpp997/1122).
        let mut path = vec![
            if follow_owner.is_some() {
                end
            } else {
                original_end
            },
            end,
        ];
        if let Some(rally) = rally {
            path.push(rally);
        }
        let owner = self.runtime.native_owner().ok_or("exit actor absent")?;
        let physics = owner
            .read()
            .map_err(|_| "exit actor poisoned")?
            .get_physics();
        let previous = if follow_owner.is_some() {
            physics.as_ref().map(|physics| {
                let mut physics = physics.access().expect("admitted exit Physics poisoned");
                let previous = physics.get_allow_to_fall();
                physics.set_allow_to_fall(false);
                previous
            })
        } else {
            None
        };
        let result = if let Some(ignore) = follow_owner {
            let mut command = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::FollowPath,
                crate::ai::CommandSourceType::FromAi,
            );
            command.obj = Some(ignore);
            command.coords = path;
            self.execute_command_with_world(&command, world)
                .map_err(|error| error.to_string())
        } else {
            let machine = self
                .ai_state_machine
                .as_mut()
                .ok_or("quick exit machine absent")?;
            let locked = machine.is_locked();
            machine.unlock();
            machine.set_goal_path(&path);
            machine.install_follow_exit_path(&path);
            let mut runtime = super::UnitAiStateRuntime {
                runtime: &mut self.runtime,
                parent_exists: true,
                parent_state_id: None,
                world: Some(world),
            };
            let result = machine.driver().enter_temporary_with_ai(
                AIStateType::FollowExitProductionPath as u32,
                10 * LOGICFRAMES_PER_SECOND,
                &mut runtime,
            );
            if locked {
                machine.lock();
            }
            if result == crate::state_machine::StateReturnType::Continue {
                Ok(())
            } else {
                Err("quick exit path entry failed".into())
            }
        }
        .and_then(|_| {
            let layer = world.layer_for_destination(&end)?;
            let mut runtime = super::UnitAiStateRuntime {
                runtime: &mut self.runtime,
                parent_exists: true,
                parent_state_id: None,
                world: Some(world),
            };
            crate::modules::ai_state_runtime::AiStateRuntime::update_goal_position(
                &mut runtime,
                &end,
                layer,
            )
        });
        if let (Some(physics), Some(previous)) = (physics, previous) {
            physics
                .access()
                .expect("admitted exit Physics poisoned")
                .set_allow_to_fall(previous);
        }
        result
    }
}

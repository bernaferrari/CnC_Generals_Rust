//! Complete scheduled UnitAI phases through the receiving instance.
//!
//! C++ AIUpdate.cpp:989–1129 fixes the order: FSM, completed movement,
//! delayed path request, turret/death handling, locomotor, waypoint sleep override.
use super::ai_core::UnitAIUpdate;
use super::imports::*;
use crate::modules::ai_world::AiWorld;
use game_engine::common::thing::update_module::UpdateSleepTime;

impl UnitAIUpdate {
    /// Reject unsupported callback families before resetting any frame metadata.
    pub(super) fn validate_scheduled_tick_owned(&self, world: &dyn AiWorld) -> Result<(), String> {
        if !world.has_pathfinder() {
            return Err("scheduled UnitAI requires the receiving pathfinder".into());
        }
        let machine = self
            .ai_state_machine
            .as_ref()
            .ok_or("scheduled AI has no machine")?;
        let state = machine.get_current_state_id();
        let temporary = machine.get_temporary_state();
        let path = temporary == Some(AIStateType::FollowExitProductionPath as u32)
            || (temporary.is_none() && state == Some(AIStateType::FollowPath as u32));
        let idle = temporary.is_none() && state == Some(AIStateType::Idle as u32);
        let dozer = self.owned_fortify_phase_is_admitted();
        let move_to = dozer
            && (temporary == Some(AIStateType::MoveTo as u32)
                || (temporary.is_none() && state == Some(AIStateType::MoveTo as u32)));
        let waypoint = temporary.is_none()
            && matches!(state,
            Some(id) if id == AIStateType::FollowWaypointPathAsIndividuals as u32
                || id == AIStateType::FollowWaypointPathAsIndividualsExact as u32
                || id == AIStateType::FollowWaypointPathAsTeam as u32
                || id == AIStateType::FollowWaypointPathAsTeamExact as u32);
        let enter = temporary.is_none() && state == Some(AIStateType::Enter as u32);
        let exit = temporary.is_none()
            && matches!(state,
            Some(id) if id == AIStateType::Exit as u32 || id == AIStateType::ExitInstantly as u32);
        let dead = temporary.is_none() && state == Some(AIStateType::Dead as u32);
        let held_enter_completion = if enter {
            self.validate_owned_enter_completion_with_world(world)
                .map_err(|error| error.to_string())?
        } else {
            self.validate_owned_components_with_world(
                world,
                self.runtime.data.locomotor_goal_type == 0,
                dozer,
            )
            .map_err(|error| error.to_string())?;
            false
        };
        if !path && !idle && !waypoint && !enter && !exit && !dead && !move_to {
            return Err("scheduled UnitAI state requires its supplied callback domain".into());
        }
        let owner = self
            .runtime
            .native_owner()
            .ok_or("scheduled AI owner retired")?;
        if dead || self.runtime.data.ai_dead {
            world.validate_dead_ai(&owner)?;
        }
        let immobile = owner
            .try_read()
            .map_err(|_| "scheduled AI owner borrowed")?
            .is_kind_of(KindOf::Immobile);
        if !immobile {
            let selected = self.selected_locomotor_owned(world)?;
            let active = self
                .runtime
                .data
                .locomotor_set
                .get_locomotor(&selected)
                .ok_or("scheduled selected locomotor absent")?;
            if !matches!(self.runtime.data.locomotor_goal_type, 0 | 1 | 2) {
                return Err("scheduled AI goal requires an owned locomotor family".into());
            }
            // Enter's actual FSM callback clears the old PATH goal before
            // doLocomotor. Only its exact already-held roster completion may
            // preflight that future NONE maintenance instead of movement.
            if self.runtime.data.locomotor_goal_type == 0 || held_enter_completion {
                world.validate_owned_maintenance_step(&owner, active)?;
            } else {
                world.validate_owned_locomotor_step(&owner, active)?;
            }
            if self.runtime.data.locomotor_set.active_name() != Some(selected.as_str()) {
                if let Some(group) = owner
                    .read()
                    .map_err(|_| "scheduled AI owner poisoned")?
                    .get_group_id()
                {
                    world.validate_group_speed_recompute(group)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn update_scheduled_tick_owned(
        &mut self,
        world: &mut dyn AiWorld,
    ) -> Result<UpdateSleepTime, String> {
        self.validate_scheduled_tick_owned(&*world)
            .map_err(|error| {
                format!(
                    "scheduled preflight state={:?} goal={}: {error}",
                    self.ai_state_machine
                        .as_ref()
                        .and_then(|machine| machine.get_current_state_id()),
                    self.runtime.data.locomotor_goal_type
                )
            })?;
        self.runtime.data.completed_waypoint_id = None;
        let machine = self
            .ai_state_machine
            .as_ref()
            .ok_or("scheduled AI has no machine")?;
        let state = machine.get_current_state_id();
        let temporary = machine.get_temporary_state();
        let mut sleep = if state == Some(AIStateType::Dead as u32) {
            let machine = self
                .ai_state_machine
                .as_mut()
                .ok_or("scheduled AI machine disappeared")?;
            let mut runtime = super::UnitAiStateRuntime {
                runtime: &mut self.runtime,
                parent_exists: true,
                parent_state_id: None,
                world: Some(world),
            };
            machine
                .update_with_synchronous_commands(&mut runtime, |driver, ai, terminal| {
                    ai.execute_command_native(terminal.params(), driver)
                })
                .map(Self::state_machine_sleep_owned)
                .map_err(|error| error.to_string())?
        } else if temporary == Some(AIStateType::FollowExitProductionPath as u32)
            || state == Some(AIStateType::FollowPath as u32)
        {
            <Self as AIUpdateInterface>::update_follow_path_phase_sleep_with_world(self, world)?
        } else if state == Some(AIStateType::Enter as u32) {
            self.update_enter_phase_sleep_with_world(world)
                .map_err(|error| format!("scheduled Enter FSM: {error}"))?
        } else if matches!(state, Some(id) if id == AIStateType::Exit as u32 || id == AIStateType::ExitInstantly as u32)
        {
            <Self as AIUpdateInterface>::update_exit_phase_sleep_with_world(self, world)?
        } else if self.owned_fortify_phase_is_admitted()
            && (temporary == Some(AIStateType::MoveTo as u32)
                || state == Some(AIStateType::MoveTo as u32))
        {
            let machine = self
                .ai_state_machine
                .as_mut()
                .ok_or("Dozer movement machine absent")?;
            let mut runtime = super::UnitAiStateRuntime {
                runtime: &mut self.runtime,
                parent_exists: true,
                parent_state_id: None,
                world: Some(world),
            };
            machine
                .update_with_synchronous_commands(&mut runtime, |driver, ai, terminal| {
                    ai.execute_command_native(terminal.params(), driver)
                })
                .map(Self::state_machine_sleep_owned)
                .map_err(|error| error.to_string())?
        } else if state == Some(AIStateType::Idle as u32) {
            self.update_idle_phase_sleep_with_world(world)
                .map_err(|error| error.to_string())?
        } else {
            self.update_waypoint_phase_sleep_with_world(world)
                .map_err(|error| error.to_string())?
        };
        self.finish_completed_movement_owned(world)
            .map_err(|error| format!("scheduled movement completion: {error}"))?;
        let now = world.frame();
        let deadline = self.runtime.data.queue_for_path_frame;
        if deadline != 0 {
            if now >= deadline {
                let request = self
                    .runtime
                    .build_classic_path_request(self.runtime.data.requested_destination, false)?;
                world
                    .pathfinder()
                    .ok_or("scheduled AI pathfinder disappeared")?
                    .queue_for_path_request(request)?;
                self.runtime.data.queue_for_path_frame = 0;
            } else {
                sleep = minimum_sleep(sleep, UpdateSleepTime::Frames(deadline - now));
            }
        }
        // Turrets/specialized actors are rejected by the readonly admission.
        // Do not invoke their ambient update through the general legacy driver.
        if self.runtime.data.ai_dead
            && self.ai_state_machine.as_ref().is_some_and(|machine| {
                machine.get_current_state_id() != Some(AIStateType::Dead as u32)
            })
        {
            let owner = self.runtime.native_owner().ok_or("Dead AI owner retired")?;
            world.validate_dead_ai(&owner)?;
            let machine = self
                .ai_state_machine
                .as_mut()
                .ok_or("Dead AI machine disappeared")?;
            let mut runtime = super::UnitAiStateRuntime {
                runtime: &mut self.runtime,
                parent_exists: true,
                parent_state_id: None,
                world: Some(world),
            };
            machine.driver().clear_with_ai(&mut runtime);
            let entered = machine
                .driver()
                .set_state_with_ai(AIStateType::Dead as u32, &mut runtime);
            if entered == crate::state_machine::StateReturnType::Failure {
                return Err("supplied Dead state entry failed".into());
            }
            machine.lock();
            sleep = UpdateSleepTime::None;
        }
        let owner = self
            .runtime
            .native_owner()
            .ok_or("scheduled AI owner retired")?;
        let immobile = owner
            .read()
            .map_err(|_| "scheduled AI owner poisoned")?
            .is_kind_of(KindOf::Immobile);
        if !immobile {
            self.choose_locomotor_owned(world)?;
            self.runtime.data.blocked_frames = if self.runtime.data.is_blocked {
                self.runtime.data.blocked_frames.wrapping_add(1)
            } else {
                0
            };
            self.runtime.data.is_blocked = false;
            sleep = minimum_sleep(
                sleep,
                self.update_locomotor_phase_with_world(world)
                    .map_err(|error| {
                        format!(
                            "scheduled locomotor after FSM state={:?} goal={}: {error}",
                            self.ai_state_machine
                                .as_ref()
                                .and_then(|machine| machine.get_current_state_id()),
                            self.runtime.data.locomotor_goal_type
                        )
                    })?,
            );
        }
        // CPP DozerAIUpdate.cpp1545–1600: inherited phases finish first,
        // then the separate primary machine observes their resulting state.
        if self.owned_fortify_phase_is_admitted() {
            let owner = self.runtime.native_owner().ok_or("Dozer actor retired")?;
            if !owner
                .try_read()
                .map_err(|_| "Dozer actor borrowed")?
                .is_effectively_dead()
            {
                self.update_dozer_primary_with_world(world)?;
            }
            // CPP1578 also returns UPDATE_SLEEP_NONE for effectively dead
            // Dozers; only the derived primary update is skipped above.
            sleep = UpdateSleepTime::None;
        }
        Ok(if self.runtime.data.completed_waypoint_id.is_some() {
            UpdateSleepTime::None
        } else {
            sleep
        })
    }

    fn selected_locomotor_owned(&self, world: &dyn AiWorld) -> Result<String, String> {
        let owner = self
            .runtime
            .native_owner()
            .ok_or("locomotor owner retired")?;
        let (position, layer) = {
            let object = owner.try_read().map_err(|_| "locomotor owner borrowed")?;
            (*object.get_position(), object.get_layer())
        };
        let surfaces = world.locomotor_surfaces_at(layer, &position)?;
        let set = &self.runtime.data.locomotor_set;
        // CPP keeps the prior member on unsupported terrain, or chooses a
        // ground member for a newly created object lacking a previous member.
        set.find_locomotor(surfaces)
            .or_else(|| set.get_active())
            .or_else(|| set.find_locomotor(crate::locomotor::SURFACE_GROUND))
            .map(|loco| loco.get_template_name().to_string())
            .ok_or("receiving locomotor set has no compatible or fallback member".into())
    }

    fn choose_locomotor_owned(&mut self, world: &mut dyn AiWorld) -> Result<(), String> {
        let selected = self.selected_locomotor_owned(&*world)?;
        if self.runtime.data.locomotor_set.active_name() == Some(selected.as_str()) {
            return Ok(());
        }
        let owner = self
            .runtime
            .native_owner()
            .ok_or("locomotor owner retired")?;
        let group = owner
            .read()
            .map_err(|_| "locomotor owner poisoned")?
            .get_group_id();
        if let Some(group) = group {
            world.validate_group_speed_recompute(group)?;
        }
        if !self.runtime.data.locomotor_set.set_active(&selected) {
            return Err("selected locomotor vanished".into());
        }
        if let Some(group) = group {
            world.recompute_group_speed(group)?;
        }
        let active = self
            .runtime
            .data
            .locomotor_set
            .get_active_mut()
            .ok_or("selected locomotor vanished")?;
        active.set_precise_z_pos(false);
        active.set_no_slow_down(false);
        active.set_ultra_accurate(false);
        Ok(())
    }

    fn finish_completed_movement_owned(&mut self, world: &mut dyn AiWorld) -> Result<(), String> {
        if !self.runtime.data.movement_complete {
            return Ok(());
        }
        self.runtime.data.queue_for_path_frame = 0;
        self.runtime.destroy_path();
        let owner = self
            .runtime
            .native_owner()
            .ok_or("completed movement owner retired")?;
        let (position, radius, layer) = {
            let mut object = owner
                .write()
                .map_err(|_| "completed movement owner poisoned")?;
            object.clear_model_condition_state(ModelConditionFlags::MOVING);
            (
                *object.get_position(),
                object.get_geometry_info().get_bounding_circle_radius(),
                object.get_layer(),
            )
        };
        let mut goal = Coord3D::new(0.0, 0.0, 0.0);
        let pathfinder = world
            .pathfinder()
            .ok_or("completed movement pathfinder missing")?;
        if pathfinder.goal_position_for_unit(self.runtime.unit_id, radius, &mut goal) {
            let dx = goal.x - position.x;
            let dy = goal.y - position.y;
            let cell = crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F;
            if dx * dx + dy * dy >= cell * cell {
                goal = pathfinder.snap_position_for_radius(&position, radius);
            }
            self.runtime.set_final_position(&goal);
            self.runtime.update_goal_position_with_world(
                &goal,
                crate::common::PathfindLayerEnum::from_u32(layer as u32),
                world,
            )?;
        }
        self.runtime.data.movement_complete = false;
        self.runtime.data.ignore_obstacle_id = INVALID_ID;
        Ok(())
    }
}

/// Compare actual wake delays, keeping UPDATE_SLEEP_NONE's one-frame meaning.
fn minimum_sleep(first: UpdateSleepTime, second: UpdateSleepTime) -> UpdateSleepTime {
    fn frames(sleep: UpdateSleepTime) -> u32 {
        match sleep {
            UpdateSleepTime::None => 1,
            UpdateSleepTime::Frames(n) => n.max(1),
            UpdateSleepTime::Forever => 0x3fff_ffff,
        }
    }
    if frames(second) < frames(first) {
        second
    } else {
        first
    }
}

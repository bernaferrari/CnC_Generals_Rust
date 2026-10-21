use super::*;

impl GameLogic {
    /// C++ GarrisonContain::onRemoving Patch 1.01 — Fire Base (non-enclosing)
    /// occupants snap host Y (C++ Z) to ground so they do not leave at STATION
    /// bone altitude. Terrain sample wins; else the container's ground Y.
    pub(super) fn snap_garrison_exit_occupant_to_ground(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(pos) = self.objects.get(&unit_id).map(|p| p.get_position()) else {
            return;
        };
        let Some(ground_y) = self
            .terrain_height_at(pos)
            .or_else(|| self.objects.get(&container_id).map(|c| c.get_position().y))
        else {
            return;
        };
        if let Some(p) = self.objects.get_mut(&unit_id) {
            let mut next = p.get_position();
            next.y = ground_y;
            p.set_position_with_effects(next, self.frame, presentation_effects);
        }
    }

    /// C++ GarrisonContain::exitObjectViaDoor — EVAC_TO_LEFT / RIGHT / BURST.
    /// Occupant should already be removed from the container roster.
    pub fn garrison_exit_occupant_via_door(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
    ) -> bool {
        self.garrison_exit_occupant_via_door_with_effects(
            unit_id,
            container_id,
            &mut Default::default(),
        )
    }

    pub(crate) fn garrison_exit_occupant_via_door_with_effects(
        &mut self,
        unit_id: ObjectId,
        container_id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(container) = self.objects.get(&container_id) else {
            return false;
        };
        if !container.is_garrison_contain() {
            return false;
        }
        let evac = container.garrison_evac_disposition();
        let enclosing = container.is_enclosing_garrison_container();
        let geom = container.thing().template.geometry_info;
        let major = if geom.authored {
            geom.major_radius.max(1.0)
        } else {
            20.0
        };
        let minor = if geom.authored {
            geom.minor_radius.max(1.0)
        } else {
            20.0
        };
        let yaw = container.get_orientation();
        let building_pos = container.get_position();
        let seed = unit_id.0.wrapping_add(self.frame);

        if evac == 1 || evac == 2 {
            let (start, mut end) =
                garrison_evac_side_points(building_pos, yaw, major, minor, evac, seed);
            let Some(p) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            p.set_contained_by(None);
            p.target = None;
            p.set_position_with_effects(start, self.frame, presentation_effects);
            p.set_orientation(yaw);
            drop(p);
            let _ = self.adjust_to_possible_destination(unit_id, &mut end);
            let Some(p) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            p.movement.path.clear();
            p.movement.path.push(end);
            p.movement.current_path_index = 0;
            p.is_attack_path = false;
            p.can_path_through_units = false;
            p.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
            p.refresh_follow_path_extra_distance();
            p.adjust_destinations = !p.ultra_accurate;
            p.set_destination(end, &mut self.movement_events.writer(self.frame));
            p.set_ai_state(AIState::Moving);
            p.status.moving = true;
        } else {
            let mut start =
                garrison_evac_cliff_start(building_pos, yaw, major, &self.pathfinding_system.grid);
            if enclosing {
                if let Some(gh) = self.terrain_height_at(start) {
                    start.y = gh;
                }
            }
            let end = {
                let owner = self.world_services.terrain();
                let loan = owner.read().unwrap_or_else(|error| error.into_inner());
                let ground = PathfindingGround::for_terrain(
                    &loan,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                garrison_evac_burst_end(start, major, &self.pathfinding_system.grid, &ground)
            };
            let Some(p) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            p.set_contained_by(None);
            p.target = None;
            if enclosing {
                p.set_position_with_effects(start, self.frame, presentation_effects);
            }
            p.set_orientation(yaw);
            p.movement.path.clear();
            p.movement.path.push(end);
            p.movement.current_path_index = 0;
            p.is_attack_path = false;
            p.can_path_through_units = false;
            p.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
            p.refresh_follow_path_extra_distance();
            p.adjust_destinations = !p.ultra_accurate;
            p.set_destination(end, &mut self.movement_events.writer(self.frame));
            p.set_ai_state(AIState::Moving);
            p.status.moving = true;
        }
        if let Some(goal) = self
            .objects
            .get(&unit_id)
            .and_then(|p| p.movement.target_position)
        {
            self.register_ground_path_goal(unit_id, goal);
        }
        if !enclosing {
            // C++ GarrisonContain::onRemoving Patch 1.01: Fire Base station
            // occupants snap Z to ground so sell/evac does not leave them
            // floating at STATION bone height.
            self.snap_garrison_exit_occupant_to_ground(unit_id, container_id, presentation_effects);
        }
        // C++ GarrisonContain::onRemoving setSafeOcclusionFrame(frame + OcclusionDelay).
        if let Some(p) = self.objects.get_mut(&unit_id) {
            p.stamp_safe_occlusion_frame(self.frame);
        }

        // set_destination → move_to already records the goal.
        // C++ OpenContain::removeFromContain doUnloadSound — leftover TheAudio.
        self.play_container_removing_template_sounds(container_id, unit_id, presentation_effects);
        // C++ GarrisonContain::exitObjectViaDoor ends in recalcApparentControllingPlayer.
        self.recalc_garrison_apparent_controller(container_id);
        true
    }
}

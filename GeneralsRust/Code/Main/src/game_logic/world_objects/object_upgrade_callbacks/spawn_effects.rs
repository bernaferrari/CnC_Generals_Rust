//! ObjectCreationUpgrade and ReplaceObjectUpgrade effects on the receiving world.
use super::*;

impl GameLogic {
    pub(super) fn replace_object_with_effects(
        &mut self,
        object_id: ObjectId,
        object_birth: &std::sync::Weak<()>,
        target: &str,
        legacy_fallback: bool,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let Some((source, team, owner, team_name, team_id, transform, position)) =
            self.host_object(object_id).map(|object| {
                (
                    object.template_name.clone(),
                    object.team,
                    object.owner_player_id,
                    object.team_instance_name.clone(),
                    object.team_instance_id,
                    object.get_transform_matrix(),
                    object.get_position(),
                )
            })
        else {
            return Ok(());
        };
        // ReplaceObjectUpgrade.cpp:55–62 must resolve the exact target
        // before touching the old object's pose claims or life state.
        if !self.ensure_host_spawn_template(target) {
            log::error!("ReplaceObjectUpgrade on '{source}' has no target template '{target}'");
            return Ok(());
        }

        let exact_team = owner.zip(team_id);
        if !legacy_fallback && exact_team.is_none() {
            return Err(generals_sim::GameLogicError::Configuration(format!(
                "ReplaceObjectUpgrade on '{source}' has no exact receiving Team"
            )));
        }

        self.release_mobile_path_footprint(object_id);
        if !legacy_fallback {
            // ReplaceObjectUpgrade.cpp:65 removes the source footprint while
            // the source is still in lookup. AIPathfind.cpp:4125–4153 releases
            // mobile claims before visiting live Wall-layer occupants.
            self.apply_live_wall_footprint_removal(object_id, interpreter, effects)?;
            if !self.object_birth_matches(object_id, object_birth) {
                return Ok(());
            }
            self.unclassify_object_path(object_id);
        }
        self.destroy_object_with_effects(object_id, effects)?;
        // A queued object is still the original. A reentrant teardown must
        // not use a replacement that merely reused the old numeric ID.
        if !self.object_birth_matches(object_id, object_birth) {
            return Ok(());
        }
        let created = match (exact_team, legacy_fallback) {
            (Some((controller, team_id)), false) => self
                .create_replacement_on_exact_team_with_effects_checked(
                    target,
                    controller,
                    &team_name,
                    team_id,
                    transform,
                    interpreter,
                    effects,
                )?,
            (Some((controller, team_id)), true) => self
                .create_object_on_exact_script_team_with_effects_checked(
                    target,
                    controller,
                    &team_name,
                    team_id,
                    position,
                    true,
                    interpreter,
                    effects,
                )?,
            (None, _) => self.create_object_for_owner_or_team_with_effects_checked(
                target,
                team,
                owner,
                position,
                interpreter,
                effects,
            )?,
        };
        let Some(new_id) = created else {
            return Ok(());
        };
        if legacy_fallback {
            self.set_owned_object_transform_with_effects(new_id, transform, self.frame, effects);
        } else {
            let replacement_birth = self
                .host_object(new_id)
                .map(|replacement| {
                    std::sync::Arc::downgrade(&replacement.visual_draw_identity_token)
                })
                .ok_or(generals_sim::GameLogicError::InvalidObject(new_id.0))?;
            self.classify_object_path(new_id);
            self.with_own_services_mut(|world| {
                world.apply_create_modules_on_build_complete_with_effects_checked(
                    new_id,
                    interpreter,
                    effects,
                )?;
                if !world.object_birth_matches(new_id, &replacement_birth) {
                    return Err(generals_sim::GameLogicError::InvalidObject(new_id.0));
                }
                if let Some(controller) = world
                    .host_object(new_id)
                    .and_then(|replacement| world.player_owner_for_host_object(replacement))
                {
                    world.complete_owned_structure_player_with_effects(
                        new_id,
                        controller,
                        false,
                        interpreter,
                        effects,
                    )?;
                }
                Ok(())
            })?;
        }
        self.replace_grant_command_upgrades
            .record_replace(&source, target);
        Ok(())
    }

    pub(in crate::game_logic) fn release_mobile_path_footprint(&mut self, id: ObjectId) {
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        // AIPathfind.cpp:4093–4125 filters these before removeUnitFromPathfindMap.
        if object.is_kind_of(KindOf::Mine)
            || object.is_kind_of(KindOf::Projectile)
            || object.is_kind_of(KindOf::BridgeTower)
            || object.is_kind_of(KindOf::BlastCrater)
            || (object.get_template().fence_width > 0.0
                && !object.is_kind_of(KindOf::DefensiveWall))
            || object.is_kind_of(KindOf::Immobile)
            || !object.has_ai_update_interface()
        {
            return;
        }
        if self.pathfinding_system.map_ready {
            self.pathfinding_system.grid.clear_path_position_square(
                id.0,
                object.geometry_info().bounding_circle_radius(),
                object.pathfind_cur_cell,
                object.pathfind_layer,
            );
            object.pathfind_cur_cell = (-1, -1);
        }
        self.remove_registered_path_goal(id);
    }

    pub(super) fn apply_object_creation_upgrade_ocl_with_interpreter_and_effects(
        &mut self,
        object_id: ObjectId,
        ocl_name: &str,
        kind: Option<crate::game_logic::host_slave_drones::SlaveDroneKind>,
        master_birth: &std::sync::Weak<()>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let ctx = self
            .objects
            .get(&object_id)
            .filter(|obj| {
                master_birth.ptr_eq(&std::sync::Arc::downgrade(&obj.visual_draw_identity_token))
            })
            .map(|obj| {
                (
                    obj.team,
                    obj.get_position(),
                    obj.get_orientation(),
                    obj.experience.level,
                    obj.movement.velocity,
                )
            });
        let Some((team, pos, orient, vet, vel)) = ctx else {
            return Err(generals_sim::GameLogicError::InvalidObject(object_id.0));
        };
        let outcome = self.execute_parsed_weapon_ocl_with_births(
            ocl_name,
            Some(object_id),
            team,
            vet,
            orient,
            vel,
            pos,
            interpreter,
            presentation_effects,
        );
        let ocl_error = outcome.error;
        let spawned = outcome.created;
        if spawned.is_empty() {
            if let Some(error) = ocl_error {
                return Err(error);
            }
            if !self.object_birth_matches(object_id, master_birth) {
                return Err(generals_sim::GameLogicError::InvalidObject(object_id.0));
            }
            return Ok(());
        }
        let Some(kind) = kind else {
            if let Some(error) = ocl_error {
                return Err(error);
            }
            return Ok(());
        };
        if !self.object_birth_matches(object_id, master_birth) {
            let mut cleanup_error = None;
            for (drone_id, (generation, epoch)) in spawned {
                if self.spawn_identity_matches(drone_id, generation, epoch) {
                    if let Err(error) =
                        self.destroy_object_with_effects(drone_id, presentation_effects)
                    {
                        cleanup_error.get_or_insert(error);
                    }
                }
            }
            if let Some(error) = cleanup_error {
                return Err(error);
            }
            if let Some(error) = ocl_error {
                return Err(error);
            }
            return Err(generals_sim::GameLogicError::InvalidObject(object_id.0));
        }
        // C++ SlavedUpdate startSlavedEffects + UpgradeDie on the OCL spawn.
        for (drone_id, (generation, epoch)) in spawned {
            if !self.spawn_identity_matches(drone_id, generation, epoch) {
                continue;
            }
            if let Some(drone) = self.objects.get_mut(&drone_id) {
                drone.producer_id = Some(object_id);
                drone.set_status_unselectable(true);
            }
        }
        if let Some(error) = ocl_error {
            return Err(error);
        }
        match kind {
            crate::game_logic::host_slave_drones::SlaveDroneKind::Scout => {
                self.scout_drone_residual_attaches =
                    self.scout_drone_residual_attaches.saturating_add(1);
            }
            crate::game_logic::host_slave_drones::SlaveDroneKind::Hellfire => {
                self.hellfire_drone_residual_attaches =
                    self.hellfire_drone_residual_attaches.saturating_add(1);
            }
            crate::game_logic::host_slave_drones::SlaveDroneKind::Battle => {
                self.battle_drone_residual_attaches =
                    self.battle_drone_residual_attaches.saturating_add(1);
            }
        }
        Ok(())
    }
}

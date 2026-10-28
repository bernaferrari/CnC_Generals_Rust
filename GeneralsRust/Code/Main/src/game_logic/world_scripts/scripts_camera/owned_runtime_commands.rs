//! Concrete ScriptActions effect delivery through one borrowed Main owner.
use super::*;
impl GameLogic {
    pub(super) fn resolve_owned_player_default_team(
        &self,
        player_id: u32,
    ) -> gamelogic::GameLogicResult<Option<(u32, Vec<u32>)>> {
        let Some(identity) = self.host_default_team_id(player_id) else {
            return Ok(None);
        };
        let factory = self
            .team_factory
            .lock()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        let Some(team) = factory.find_team_by_id(identity) else {
            return Ok(None);
        };
        let team = team
            .read()
            .map_err(|error| gamelogic::GameLogicError::Threading(error.to_string()))?;
        if let Some(reason) = team.creation_failure() {
            return Err(gamelogic::GameLogicError::TeamCreationFailed(reason.into()));
        }
        Ok(Some((identity, team.get_members().to_vec())))
    }

    /// Apply the current live TeamMove behavior through the receiving world.
    /// Full C++ AIGroup formation/path parity remains separate work.
    pub(super) fn apply_owned_team_move_to_waypoint(
        &mut self,
        team: &str,
        waypoint: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        let Some(destination) = self.host_script_waypoint_position(waypoint) else {
            return Ok(());
        };
        for member in members {
            self.unit_command_move_to(ObjectId(member), destination);
        }
        Ok(())
    }

    pub(super) fn apply_owned_team_guard_object(
        &mut self,
        team: &str,
        target: &str,
        this_object: Option<ObjectId>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(target) = self.owned_named_script_object(target, this_object) else {
            return Ok(());
        };
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        for member in members {
            let id = ObjectId(member);
            if self.host_script_unit_can_guard(id) {
                self.unit_command_script_guard_object(id, target);
            }
        }
        Ok(())
    }
    pub(super) fn apply_owned_team_damage(
        &mut self,
        team: &str,
        amount: f32,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        self.apply_owned_team_damage_with_effects(
            team,
            amount,
            calling,
            condition,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(super) fn apply_owned_team_damage_with_effects(
        &mut self,
        team: &str,
        amount: f32,
        calling: Option<u32>,
        condition: Option<u32>,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        for member in members {
            let id = ObjectId(member);
            if !self.host_object(id).is_some_and(|object| {
                object.is_alive() && !object.status.effectively_dead && !object.status.destroyed
            }) {
                continue;
            }
            self.host_script_damage_with_interpreter_with_effects(
                id,
                amount,
                amount < 0.0,
                interpreter,
                effects,
            );
        }
        Ok(())
    }
    pub(super) fn apply_owned_team_attack_team(
        &mut self,
        source: &str,
        target: &str,
        calling: Option<u32>,
        condition: Option<u32>,
        force_normal: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((target_id, _)) = self.resolve_owned_script_team(target, calling, condition)?
        else {
            return Ok(());
        };
        let Some((_, members)) = self.resolve_owned_script_team(source, calling, condition)? else {
            return Ok(());
        };
        for member in members {
            let id = ObjectId(member);
            if self
                .host_object(id)
                .is_some_and(|object| object.has_ai_update_interface() && !object.is_contained())
            {
                // CPP groupAttackTeam does not choose Normal or leaveGroup.
                self.apply_owned_attack_team_member(id, target_id, force_normal, effects)?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_owned_named_waypoint(
        &mut self,
        name: &str,
        waypoint: &str,
        follow: Option<bool>,
        this_object: Option<ObjectId>,
    ) {
        let Some(id) = self.owned_named_script_object(name, this_object) else {
            return;
        };
        let Some(object) = self
            .host_object(id)
            .filter(|object| object.has_ai_update_interface())
        else {
            return;
        };
        let position = object.get_position();
        if let Some(exact) = follow {
            let Some(path) = self.host_script_waypoint_route_from(waypoint, position) else {
                return;
            };
            self.host_object_mut(id)
                .unwrap()
                .set_formation(0, glam::Vec2::ZERO);
            self.apply_unit_locomotor_set(id, "normal");
            self.host_object_mut(id).unwrap().last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            self.host_script_issue_follow_waypoint_path(
                &[id],
                &path.points,
                exact,
                false,
                path.completion(exact),
            );
        } else {
            let Some(destination) = self.host_script_waypoint_position(waypoint) else {
                return;
            };
            // CPP433 clears the queue before Normal and aiMoveToPosition.
            let object = self.host_object_mut(id).unwrap();
            object.set_formation(0, glam::Vec2::ZERO);
            object.clear_pending_waypoint_labels();
            self.clear_unit_movement_path(id);
            self.apply_unit_locomotor_set(id, "normal");
            self.unit_command_script_move_to(id, destination);
        }
    }

    pub(super) fn apply_owned_team_hunt_button(
        &mut self,
        team: &str,
        button: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, members)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        if !Self::leftover_command_button_is_hunt_capable(button) {
            return Ok(());
        }
        for member in members {
            let id = ObjectId(member);
            if self.unit_can_team_hunt_with_command_button(id, Some(button)) {
                self.start_command_button_hunt_named(id, Some(button))?;
            }
        }
        Ok(())
    }

    pub(super) fn apply_owned_team_transfer(
        &mut self,
        team: &str,
        player: u32,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((team_id, _)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        self.apply_owned_team_transfer_to_id(team_id, player)
    }

    pub(super) fn apply_owned_team_transfer_to_id(
        &mut self,
        team_id: u32,
        player: u32,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(side) = self.players.get(&player).map(|player| player.team) else {
            return Ok(());
        };
        let Some(transfer) = self
            .team_factory
            .transfer_prototype_controller_without_flushing(team_id, player)
            .map_err(gamelogic::GameLogicError::Threading)?
        else {
            return Ok(());
        };
        // CPP changes the controlling player, retaining the concrete team and
        // roster. Every sibling object's cached controlling-player/faction
        // observation follows the shared prototype. C++ partition maintenance
        // and ScriptActions::updateTeamAndPlayerStuff remain selected-Team-only.
        // The latter's upgrade-module and Drawable-indicator refreshes do not
        // yet have a corresponding host operation in this code path.
        for member in transfer.all_member_ids {
            if let Some(object) = self.host_object_mut(ObjectId(member)) {
                object.owner_player_id = Some(player);
                object.team = side;
            }
        }
        for member in transfer.selected_member_ids {
            if let Some((object, partition)) = self.host_object_and_partition_mut(ObjectId(member))
            {
                object.handle_partition_cell_maintenance_with_partition_owner(partition);
            }
        }
        Ok(())
    }

    pub(super) fn apply_owned_sell_everything(&mut self, player: u32) {
        self.apply_owned_sell_everything_with_effects(player, &mut Default::default())
    }

    pub(super) fn apply_owned_sell_everything_with_effects(
        &mut self,
        player: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let ids: Vec<ObjectId> = self
            .host_objects()
            .registered_values()
            .filter(|object| {
                object.owner_player_id == Some(player)
                    && object.is_alive()
                    && !object.status.effectively_dead
                    && (object.is_faction_structure()
                        || object.is_kind_of(KindOf::CommandCenter)
                        || object.is_kind_of(KindOf::FSPower))
            })
            .map(|object| object.id)
            .collect();
        for id in ids {
            self.start_sell_object_with_effects(id, effects);
        }
    }
}

impl GameLogic {
    /// CPP ScriptActions1767–1865 resolves the driving roster and owned path
    /// before issuing the immediate normal/exact/as-team group order.
    pub(super) fn apply_owned_team_waypoint_path(
        &mut self,
        team: &str,
        path_label: &str,
        as_team: bool,
        exact: bool,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some((_, roster)) = self.resolve_owned_script_team(team, calling, condition)? else {
            return Ok(());
        };
        let members: Vec<_> = roster.into_iter().map(ObjectId).collect();
        let mut center = Vec3::ZERO;
        let mut count = 0;
        for member in &members {
            if let Some(object) = self.host_object(*member) {
                center += object.get_position();
                count += 1;
            }
        }
        if count == 0 {
            return Ok(());
        }
        center /= count as f32;
        let Some(path) = self.host_script_waypoint_route_from(path_label, center) else {
            return Ok(());
        };
        self.host_script_issue_follow_waypoint_path(
            &members,
            &path.points,
            exact,
            as_team,
            path.completion(exact),
        );
        Ok(())
    }
}

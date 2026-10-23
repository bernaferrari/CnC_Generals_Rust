//! Concrete ScriptActions effect delivery through one borrowed Main owner.
use super::*;
impl GameLogic {
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
                self.apply_owned_attack_team_member(id, target_id, force_normal);
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
                self.start_command_button_hunt_named(id, Some(button));
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
        let Some(side) = self.players.get(&player).map(|player| player.team) else {
            return Ok(());
        };
        let members = {
            let factory = self.team_factory.lock().expect("owned controller transfer");
            let Some(team) = factory.find_team_by_id(team_id) else {
                return Ok(());
            };
            let mut team = team.write().expect("owned controller");
            team.set_controlling_player_id_for_owned_members(Some(player))
        };
        // CPP changes the controlling player, retaining the concrete team and
        // roster. There is no capture command or capture behavior callback.
        for member in members {
            if let Some(object) = self.host_object_mut(ObjectId(member)) {
                object.owner_player_id = Some(player);
                object.team = side;
                object.handle_partition_cell_maintenance();
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

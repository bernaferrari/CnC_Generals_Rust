//! Named ScriptActions operate synchronously on the driving Main world.
use super::*;
use gamelogic::scripting::engine::ScriptNamedCommand;

impl GameLogic {
    fn owned_named_script_object(
        &self,
        name: &str,
        this_object: Option<ObjectId>,
    ) -> Option<ObjectId> {
        // CPP ScriptEngine::getUnitNamed: exact authored name, or the calling
        // object (before condition object) supplied by the borrowed engine.
        if name.is_empty() {
            return None;
        }
        if name == gamelogic::scripting::core::THIS_OBJECT {
            return this_object.filter(|id| self.host_object(*id).is_some());
        }
        self.host_objects()
            .values()
            .find(|o| o.name == name)
            .map(|o| o.id)
    }

    /// CPP ScriptActions.cpp1966 chooses NORMAL even if AI rejects the order;
    /// privateHunt.cpp3611 clears and reenters Hunt without leaving the group.
    pub(super) fn apply_owned_named_hunt(&mut self, name: &str, this_object: Option<ObjectId>) {
        let Some(id) = self.owned_named_script_object(name, this_object) else {
            return;
        };
        if !self
            .host_object(id)
            .is_some_and(|o| o.has_ai_update_interface())
        {
            return;
        }
        self.apply_unit_locomotor_set(id, "normal");
        if !self.owned_named_ai_command_admitted(id) {
            return;
        }

        self.drop_jet_targeters_on_attack_exit(id);
        self.clear_unit_movement_path(id);
        // The existing Rust stream is owned by this session. Initialize at
        // onEnter, rather than adopting an ambient stream on the next tick.
        let deadline = self.frame.wrapping_add(self.logic_random.next_u32() % 31);
        let unit = self.host_object_mut(id).expect("admitted Hunt unit");
        Self::clear_owned_named_ai_goal(unit);
        unit.last_command_source =
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        unit.auto_acquire_when_idle = true;
        unit.hunting = true;
        unit.set_ai_state(AIState::Patrolling);
        unit.mark_jet_command_for_reload_interrupt(true);
        unit.set_status_moving(false);
        unit.unit_ai_runtime.set_hunt_scan_deadline(Some(deadline));
    }

    /// CPP AIUpdate.cpp2572/4014: SCRIPT admission uses the simulation
    /// controller and Object::isMobile, independently of display locality.
    fn owned_named_ai_command_admitted(&self, id: ObjectId) -> bool {
        let Some(unit) = self.host_object(id) else {
            return false;
        };
        let sleeping_ai = unit.ai_attitude()
            == crate::game_logic::host_strategy_center::HostAiAttitude::Sleep
            && unit
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .is_none_or(|p| !p.is_human);
        unit.is_alive()
            && !unit.status.effectively_dead
            && !sleeping_ai
            && unit.is_mobile_for_ai_command()
            && !unit.is_kind_of(KindOf::Projectile)
    }

    /// The bounded onExit phase shared by commands that clear the current
    /// AI machine. It borrows only the canonical object, with no registry lookup.
    fn clear_owned_named_ai_goal(unit: &mut Object) {
        if unit.hunting || matches!(unit.ai_state, AIState::Patrolling) {
            unit.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }
        // Deleting the parent also exits its active attack submachine
        // (CPP AIAttackState::onExit5668), including Hunt -> Hunt reentry.
        if matches!(
            unit.ai_state,
            AIState::Attacking
                | AIState::AttackingGround
                | AIState::AttackMoving
                | AIState::GuardRetaliating
        ) {
            unit.set_status_attacking(false);
            unit.set_status_firing_weapon(false);
            unit.set_status_aiming_weapon(false);
            unit.set_status_ignoring_stealth(false);
            unit.model_condition_bits &=
                !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_ATTACKING);
            unit.record_host_model_condition();
            unit.clear_leech_range_mode_for_all_weapons();
            unit.set_turret_target_object(None, false);
        }
        unit.clear_pending_waypoint_labels();
        unit.requested_destination = None;
        unit.waiting_for_path = false;
        unit.pending_move = None;
        unit.set_target(None);
        unit.set_target_location(None);
        unit.set_force_attack(false);
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        unit.end_guard_retaliate();
    }

    /// SCRIPT groupIdle keeps group/locomotor preparation intact. Ordinary
    /// player Stop has additional effects and cannot stand in for this command.
    pub(super) fn apply_owned_script_idle(&mut self, id: ObjectId) {
        self.apply_owned_script_idle_member(id, &mut std::collections::HashSet::new());
    }

    fn apply_owned_script_idle_member(
        &mut self,
        id: ObjectId,
        visited: &mut std::collections::HashSet<ObjectId>,
    ) {
        // Malformed/stale containment links cannot reenter an already visited
        // object. Effects remain depth first at the original synchronous point.
        if !visited.insert(id) {
            return;
        }
        let Some(unit) = self.host_object(id) else {
            return;
        };
        let occupants = unit.contained_units();
        let has_ai = unit.has_ai_update_interface();
        if has_ai {
            // CPP2572 general command admission and3067 privateIdle: Idle
            // accepts immobile AI. The mobile check belongs to Guard/Hunt.
            let sleeping_ai = unit.ai_attitude()
                == crate::game_logic::host_strategy_center::HostAiAttitude::Sleep
                && unit
                    .owner_player_id
                    .and_then(|pid| self.players.get(&pid))
                    .is_none_or(|player| !player.is_human);
            if !unit.is_alive()
                || unit.status.effectively_dead
                || sleeping_ai
                || unit.is_kind_of(KindOf::Projectile)
                || unit.is_surrendered
            {
                return;
            }
            self.drop_jet_targeters_on_attack_exit(id);
            self.clear_unit_movement_path(id);
            let unit = self.host_object_mut(id).expect("admitted SCRIPT Idle unit");
            Self::clear_owned_named_ai_goal(unit);
            unit.stop();
            unit.clear_guard_chase();
            unit.unit_ai_runtime.clear_guard();
            unit.unit_ai_runtime.clear_hunt();
            unit.hunting = false;
            unit.set_ai_state(AIState::Idle);
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            unit.mark_jet_command_for_reload_interrupt(true);
        }
        // CPP privateIdle3094 and AIGroup2069 also idle garrisoned AI.
        for occupant in occupants {
            if occupant != id {
                self.apply_owned_script_idle_member(occupant, visited);
            }
        }
    }

    /// CPP ScriptActions.cpp1861: leaveGroup, capture position, NORMAL,
    /// then SCRIPT GuardPosition. Even a rejected order keeps the first effects.
    pub(super) fn apply_owned_named_guard(&mut self, name: &str, this_object: Option<ObjectId>) {
        let Some(id) = self.owned_named_script_object(name, this_object) else {
            return;
        };
        let unit = self.host_object_mut(id).expect("resolved Guard unit");
        if !unit.has_ai_update_interface() {
            return;
        }
        unit.set_formation(0, glam::Vec2::ZERO);
        let position = unit.get_position();
        self.apply_unit_locomotor_set(id, "normal");
        self.apply_owned_script_guard_position(id, position);
    }

    /// Shared private Guard entry, after each action's distinct preparation.
    /// TeamGuard does not leave its group or select a new locomotor set.
    pub(super) fn apply_owned_script_guard_position(&mut self, id: ObjectId, position: Vec3) {
        if !self
            .host_object(id)
            .is_some_and(|unit| unit.has_ai_update_interface())
            || !self.owned_named_ai_command_admitted(id)
        {
            return;
        }
        let clear_team_target = self
            .host_object(id)
            .is_some_and(|unit| matches!(unit.guard_chase_phase, 1 | 3));
        self.drop_jet_targeters_on_attack_exit(id);
        if clear_team_target {
            self.set_host_team_common_target(id, None);
        }
        self.clear_unit_movement_path(id);
        let unit = self.host_object_mut(id).expect("admitted Guard unit");
        Self::clear_owned_named_ai_goal(unit);
        unit.clear_guard_chase();
        unit.unit_ai_runtime.clear_guard();
        unit.unit_ai_runtime.clear_hunt();
        unit.hunting = false;
        unit.set_guard_mode(GuardMode::Normal);
        unit.set_guard_position(Some(position));
        unit.last_command_source =
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        unit.mark_jet_command_for_reload_interrupt(true);
        self.return_guard_to_post(id);
    }

    pub(super) fn apply_owned_named_script_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<ObjectId>,
    ) {
        match request {
            ScriptNamedCommand::Hunt { unit } => {
                self.apply_owned_named_hunt(unit, this_object);
                return;
            }
            ScriptNamedCommand::Guard { unit } => {
                self.apply_owned_named_guard(unit, this_object);
                return;
            }
            _ => {}
        }
        let (unit, target) = match request {
            ScriptNamedCommand::ForceAttack { unit, target }
            | ScriptNamedCommand::FaceObject { unit, target } => (unit, target),
            ScriptNamedCommand::Hunt { .. } | ScriptNamedCommand::Guard { .. } => {
                unreachable!("single-unit command handled above")
            }
        };
        let Some(id) = self.owned_named_script_object(unit, this_object) else {
            return;
        };
        let Some(target_id) = self.owned_named_script_object(target, this_object) else {
            return;
        };
        if !self
            .host_object(id)
            .is_some_and(|o| o.has_ai_update_interface())
        {
            return;
        }
        if matches!(request, ScriptNamedCommand::FaceObject { .. }) {
            // Main's AddWaypoint appends to this object's movement route. Clear
            // that route and its pending destination without issuing player
            // Stop (which also idles passengers and invokes other behaviors).
            self.clear_unit_movement_path(id);
            let unit = self.host_object_mut(id).expect("resolved script unit");
            unit.clear_pending_waypoint_labels();
            unit.requested_destination = None;
            unit.waiting_for_path = false;
            unit.pending_move = None;
        }
        // CPP ScriptActions.cpp1042/6081: leaveGroup precedes NORMAL selection.
        // The compatibility leave-group helper also touches OBJECT_REGISTRY;
        // the selected owner's command must affect only its own formation.
        self.host_object_mut(id)
            .expect("resolved script unit")
            .set_formation(0, glam::Vec2::ZERO);
        self.apply_unit_locomotor_set(id, "normal");
        match request {
            ScriptNamedCommand::Hunt { .. } | ScriptNamedCommand::Guard { .. } => {
                unreachable!("single-unit command handled above")
            }
            ScriptNamedCommand::ForceAttack { .. } => {
                self.host_object_mut(id)
                    .expect("resolved script unit")
                    .last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
                if self.unit_command_force_attack(id, target_id) {
                    // CPP privateForceAttackObject applies the limit after
                    // entering the state because onEnter resets the weapon.
                    let unit = self.host_object_mut(id).expect("resolved script unit");
                    unit.pending_move = None;
                    unit.set_max_shots_to_fire(-1);
                }
            }
            ScriptNamedCommand::FaceObject { .. } => {
                let unit = self.host_object_mut(id).expect("resolved script unit");
                if !unit.can_move() {
                    return;
                }
                unit.last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
                unit.num_frames_blocked = 0;
                self.private_face_object(id, target_id);
            }
        }
    }
}

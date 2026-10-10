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
        let unit = self.host_object(id).expect("resolved Hunt unit");
        let sleeping_ai = unit.ai_attitude()
            == crate::game_logic::host_strategy_center::HostAiAttitude::Sleep
            && unit
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .is_none_or(|p| !p.is_human);
        if !unit.is_alive()
            || unit.status.effectively_dead
            || sleeping_ai
            || !unit.is_mobile_for_ai_command()
            || unit.is_kind_of(KindOf::Projectile)
        {
            return;
        }

        self.drop_jet_targeters_on_attack_exit(id);
        self.clear_unit_movement_path(id);
        // The existing Rust stream is owned by this session. Initialize at
        // onEnter, rather than adopting an ambient stream on the next tick.
        let deadline = self.frame.wrapping_add(self.logic_random.next_u32() % 31);
        let unit = self.host_object_mut(id).expect("admitted Hunt unit");
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
        unit.last_command_source =
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        unit.auto_acquire_when_idle = true;
        unit.hunting = true;
        unit.set_ai_state(AIState::Patrolling);
        unit.mark_jet_command_for_reload_interrupt(true);
        unit.set_status_moving(false);
        unit.unit_ai_runtime.set_hunt_scan_deadline(Some(deadline));
    }

    pub(super) fn apply_owned_named_script_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<ObjectId>,
    ) {
        if let ScriptNamedCommand::Hunt { unit } = request {
            self.apply_owned_named_hunt(unit, this_object);
            return;
        }
        let (unit, target) = match request {
            ScriptNamedCommand::ForceAttack { unit, target }
            | ScriptNamedCommand::FaceObject { unit, target } => (unit, target),
            ScriptNamedCommand::Hunt { .. } => unreachable!("Hunt handled above"),
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
            ScriptNamedCommand::Hunt { .. } => unreachable!("Hunt handled above"),
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

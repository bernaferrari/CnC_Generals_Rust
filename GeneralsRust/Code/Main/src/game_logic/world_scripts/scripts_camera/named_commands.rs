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

    pub(super) fn apply_owned_named_script_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<ObjectId>,
    ) {
        let (unit, target) = match request {
            ScriptNamedCommand::ForceAttack { unit, target }
            | ScriptNamedCommand::FaceObject { unit, target } => (unit, target),
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

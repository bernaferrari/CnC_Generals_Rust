//! C++ ScriptActions5136-5308 and AIMoveToState: one owned checked object goal.
use super::*;
use gamelogic::scripting::engine::{ScriptNearestObjectRequest, ScriptNearestRecipient};

impl GameLogic {
    pub(super) fn apply_owned_nearest_object_move(
        &mut self,
        request: ScriptNearestObjectRequest<'_>,
        trigger_name: &str,
    ) -> gamelogic::GameLogicResult<()> {
        let members = match request.recipient {
            ScriptNearestRecipient::Named(name) => {
                let Some(id) =
                    self.owned_named_script_object(name, request.this_object.map(ObjectId))
                else {
                    return Ok(());
                };
                vec![id]
            }
            ScriptNearestRecipient::Team(name) => {
                let Some((_, members)) = self.resolve_owned_script_team(
                    name,
                    request.calling_team,
                    request.condition_team,
                )?
                else {
                    return Ok(());
                };
                members.into_iter().map(ObjectId).collect()
            }
        };
        let source = match request.recipient {
            ScriptNearestRecipient::Named(_) => members.first().copied(),
            ScriptNearestRecipient::Team(_) => members.iter().copied().find(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|object| object.has_ai_update_interface())
            }),
        };
        let Some(source) = source.filter(|id| {
            self.objects
                .get(id)
                .is_some_and(|object| object.has_ai_update_interface())
        }) else {
            return Ok(());
        };
        // Team::getEstimateTeamPosition returns the first member's position,
        // whereas same-map filtering uses the first AI-capable member.
        let estimate = match request.recipient {
            ScriptNearestRecipient::Named(_) => self.objects[&source].get_position(),
            ScriptNearestRecipient::Team(_) => {
                let Some(first) = members.first().and_then(|id| self.objects.get(id)) else {
                    return Ok(());
                };
                first.get_position()
            }
        };
        let Some(target) = self.owned_nearest_object_in_trigger(
            source,
            estimate,
            request.object_type,
            request.object_types,
            trigger_name,
        ) else {
            return Ok(());
        };
        for id in members {
            // Original team walk returns at the first missing/non-AI member;
            // effects on earlier members remain observable.
            if !self
                .objects
                .get(&id)
                .is_some_and(|object| object.has_ai_update_interface())
            {
                return Ok(());
            }
            let _ = self.apply_unit_locomotor_set(id, "normal");
            let _ = self.script_move_to_object(id, target);
        }
        Ok(())
    }

    fn owned_nearest_object_in_trigger(
        &self,
        source: ObjectId,
        estimate: glam::Vec3,
        object_type: &str,
        object_types: &[String],
        trigger_name: &str,
    ) -> Option<ObjectId> {
        let terrain = self
            .world_services
            .terrain()
            .read()
            .expect("owned nearest-object terrain");
        let trigger = terrain.get_trigger_area_by_name(trigger_name)?;
        let names = if self.templates.contains_key(object_type) {
            vec![object_type.to_owned()]
        } else {
            object_types.to_vec()
        };
        let off_map = |position: glam::Vec3| {
            position.x < self.world_min.x
                || position.x > self.world_max.x
                || position.z < self.world_min.z
                || position.z > self.world_max.z
        };
        let source_off_map = off_map(self.objects.get(&source)?.get_position());
        let mut best: Option<(ObjectId, f32)> = None;
        // C++ uses the Coord3D overload, so no receiver is excluded. These
        // filters also omit PartitionFilterAlive: retained dead/queued bodies
        // remain candidates until their physical partition removal.
        // Keep the current production visitation order and strict tie rule;
        // exact equivalence/partition ordering is tracked in hq-llakh.
        for (&id, object) in self.objects.iter() {
            if off_map(object.get_position()) != source_off_map
                || !names.iter().any(|name| object.template_name == *name)
            {
                continue;
            }
            let position = object.get_position();
            if !trigger.point_in_trigger_int(&gamelogic::common::ICoord3D::new(
                position.x as i32,
                position.z as i32,
                0,
            )) {
                continue;
            }
            let distance = (position.x - estimate.x).powi(2) + (position.z - estimate.z).powi(2);
            if best.is_none_or(|(_, current)| distance < current) {
                best = Some((id, distance));
            }
        }
        best.map(|(id, _)| id)
    }

    pub(crate) fn script_move_to_object(&mut self, id: ObjectId, target: ObjectId) -> bool {
        if !self.unit_can_move(id)
            || !self
                .objects
                .get(&id)
                .is_some_and(|object| object.has_ai_update_interface())
        {
            return false;
        }
        let Some(destination) = self.objects.get(&target).map(Object::get_position) else {
            return false;
        };
        self.stop_attack_clearing_jet_targeter(id);
        if let Some(object) = self.objects.get_mut(&id) {
            // privateMoveToObject clears/replaces the parent state and stores
            // the checked identity before entering AI_MOVE_TO.
            if object.hunting || matches!(object.ai_state, AIState::Patrolling) {
                object.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }
            object.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            object.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            object.set_movement_goal_object(Some(target));
        }
        // Script goals do not use CMD_FROM_PLAYER map-edge clipping.
        let admitted = self.assign_unit_path(id, destination, &[]);
        if let Some(object) = self.objects.get_mut(&id) {
            object.set_ai_state(
                AIState::Moving,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            object.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            object.hunting = false;
            object.set_movement_goal_object(Some(target));
        }
        self.clear_unit_hunt_scan(id);
        admitted
    }

    pub(crate) fn refresh_movement_goal_objects(&mut self, object_ids: &[ObjectId]) {
        let updates: Vec<_> = object_ids
            .iter()
            .filter_map(|id| {
                let object = self.objects.get(id)?;
                if object.is_disabled()
                    || object.host_skip_dead_locomotor()
                    || object.temporary_move_frames > 0
                    || !matches!(object.ai_state, AIState::Moving)
                {
                    return None;
                }
                let target = object.movement_goal_object()?;
                // AIStates.cpp:2080 retains its last goal position when a goal
                // object lookup fails. Absence is not permission to reacquire.
                let position = self.objects.get(&target)?.get_position();
                Some((*id, position))
            })
            .collect();
        for (id, position) in updates {
            if let Some(object) = self.objects.get_mut(&id) {
                object.requested_destination = Some(position);
            }
        }
        // The existing repath phase preserves the original >10-frame timing,
        // pending search and prior path; refreshing a goal does not reinit AI.
    }
}

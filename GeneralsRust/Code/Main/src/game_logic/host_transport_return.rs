//! AIStates.cpp:683,3429–3480,3698–3749: move/evacuate → return/delete.
use crate::game_logic::{AIState, GameLogic, ObjectId};
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TransportReturnState {
    MoveAndEvacuate {
        origin: Vec3,
    },
    MoveAndDelete {
        origin: Vec3,
        append_goal: bool,
    },
    TemporaryMove {
        origin: Vec3,
        returning: bool,
        append_goal: bool,
        parent_goal: Vec3,
    },
}

impl GameLogic {
    /// CPP clear/setGoal/setState ignore external replacement while locked;
    /// the intervening ordinary command bookkeeping still takes effect.
    pub(crate) fn apply_locked_transport_command(&mut self, id: ObjectId) -> bool {
        let Some(object) = self.host_object_mut(id) else {
            return false;
        };
        if object.transport_return_state.is_none() {
            return false;
        }
        object.num_frames_blocked = 0;
        object.is_blocked = false;
        object.is_blocked_and_stuck = false;
        true
    }

    /// The shared AI temporary-move entry suspends, rather than exits, its parent.
    pub(crate) fn suspend_transport_for_temporary_move(&mut self, id: ObjectId) -> bool {
        let Some(object) = self.host_object_mut(id) else {
            return false;
        };
        let Some(state) = object.transport_return_state else {
            return false;
        };
        let (origin, returning, append_goal, parent_goal) = match state {
            TransportReturnState::MoveAndEvacuate { origin } => (
                origin,
                false,
                false,
                object.requested_destination.unwrap_or(origin),
            ),
            TransportReturnState::MoveAndDelete {
                origin,
                append_goal,
            } => (origin, true, append_goal, origin),
            TransportReturnState::TemporaryMove {
                origin,
                returning,
                append_goal,
                parent_goal,
                ..
            } => (origin, returning, append_goal, parent_goal),
        };
        object.transport_return_state = Some(TransportReturnState::TemporaryMove {
            origin,
            returning,
            append_goal,
            parent_goal,
        });
        true
    }

    fn resume_transport_after_temporary_move(&mut self, id: ObjectId) {
        let Some((origin, returning, append_goal, parent_goal)) =
            self.host_object(id).and_then(|object| {
                if object.has_temporary_ai_move() {
                    return None;
                }
                match object.transport_return_state {
                    Some(TransportReturnState::TemporaryMove {
                        origin,
                        returning,
                        append_goal,
                        parent_goal,
                        ..
                    }) => Some((origin, returning, append_goal, parent_goal)),
                    _ => None,
                }
            })
        else {
            return;
        };
        if let Some(object) = self.host_object_mut(id) {
            object.transport_return_state = Some(if returning {
                TransportReturnState::MoveAndDelete {
                    origin,
                    append_goal,
                }
            } else {
                TransportReturnState::MoveAndEvacuate { origin }
            });
        }
        // AIInternalMoveTo::onExit ends the temporary route. The resumed
        // parent sees a null path and recomputes its own saved goal.
        self.assign_unit_path(id, parent_goal, &[]);
    }

    /// Executed before this object's locomotor loan, after real path delivery.
    pub(crate) fn prepare_transport_return_path(&mut self, id: ObjectId) {
        let Some(TransportReturnState::MoveAndDelete {
            origin,
            append_goal,
        }) = self
            .host_object(id)
            .and_then(|object| object.transport_return_state)
        else {
            return;
        };
        let ready = self.host_object(id).is_some_and(|object| {
            object.is_alive() && !object.waiting_for_path && !object.movement.path.is_empty()
        });
        if let Some(object) = self.host_object_mut(id) {
            object.allow_invalid_position = true;
        }
        if !append_goal || !ready {
            return;
        }
        let height = self.terrain_height_at(origin).unwrap_or_else(|| {
            self.host_object(id)
                .map_or(origin.y, |object| object.ground_height)
        });
        let goal = Vec3::new(origin.x, height, origin.z);
        if let Some(object) = self.host_object_mut(id) {
            // C++ appendNode is unconditional, including an identical final node.
            object.movement.path.push(goal);
            object.refresh_follow_path_extra_distance();
            object.transport_return_state = Some(TransportReturnState::MoveAndDelete {
                origin,
                append_goal: false,
            });
        }
    }

    /// Bounded owned continuation after this object's real motion/arrival phase.
    pub(crate) fn finish_transport_return_phase(&mut self, id: ObjectId) {
        self.finish_transport_return_phase_with_effects(id, &mut Default::default())
    }

    pub(crate) fn finish_transport_return_phase_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if self.host_object(id).is_some_and(|object| {
            matches!(
                object.transport_return_state,
                Some(TransportReturnState::TemporaryMove { .. })
            )
        }) {
            let continuing = self.host_object(id).is_some_and(|object| {
                object
                    .unit_ai_runtime
                    .temporary_move()
                    .is_some_and(|temporary| {
                        let arrived = !object.waiting_for_path
                            && object.movement.path.is_empty()
                            && object.movement.target_position.is_none();
                        !arrived && !(temporary.deadline < self.frame)
                    })
            });
            if continuing {
                return;
            }
            if let Some((object, audio_events)) = self.host_object_and_audio_events_mut(id) {
                object.end_temporary_move_overlay(&mut crate::game_logic::ObjectAudio::queue(
                    audio_events,
                ));
            }
            self.resume_transport_after_temporary_move(id);
        }
        let Some((state, alive, terminal)) = self.host_object(id).and_then(|object| {
            object.transport_return_state.map(|state| {
                (
                    state,
                    object.is_alive(),
                    !object.waiting_for_path
                        && object.queue_for_path_frames == 0
                        && object.movement.path.is_empty()
                        && !object.status.moving,
                )
            })
        }) else {
            return;
        };
        if !alive {
            if let Some(object) = self.host_object_mut(id) {
                object.transport_return_state = None;
            }
            return;
        }
        if !terminal {
            return;
        }
        match state {
            TransportReturnState::TemporaryMove { .. } => return,
            TransportReturnState::MoveAndEvacuate { origin } => {
                // CPP orders riders to exit, activates the team and returns
                // immediately; delayed rider exits do not hold the hull here.
                self.evacuate_container_now_with_effects(id, false, effects);
                self.activate_leftover_team_for_host_object_with_effects(id, effects);
                self.begin_transport_return(id, origin);
            }
            TransportReturnState::MoveAndDelete { .. } => {
                if let Some(object) = self.host_object_mut(id) {
                    object.transport_return_state = None;
                    object.pending_exit_after_evacuate = false;
                }
                // Actual GameLogic destruction at the completed state callback,
                // never a fabricated health/death flag or end-frame substitute.
                self.destroy_object_with_effects(id, effects);
            }
        }
    }

    fn begin_transport_return(&mut self, id: ObjectId, origin: Vec3) {
        self.note_move_to_request_path(id);
        if let Some(object) = self.host_object_mut(id) {
            object.set_adjust_destinations(false);
            object.is_final_goal = false;
            object.allow_invalid_position = true;
            object.pending_exit_after_evacuate = false;
            object.transport_return_state = Some(TransportReturnState::MoveAndDelete {
                origin,
                append_goal: true,
            });
        }
        if self.assign_unit_path(id, origin, &[]) {
            if let Some((object, audio_events)) = self.host_object_and_audio_events_mut(id) {
                object.set_ai_state(
                    AIState::Moving,
                    &mut crate::game_logic::ObjectAudio::queue(audio_events),
                );
            }
        } else {
            // AIInternalMoveTo::onEnter failure follows the state's failure
            // transition; it never executes MoveAndDelete::update's destroy.
            if let Some((object, audio_events)) = self.host_object_and_audio_events_mut(id) {
                object.transport_return_state = None;
                object.stop_moving(&mut crate::game_logic::ObjectAudio::queue(audio_events));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    fn carrier() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("ReturnCarrier");
        template.set_health(200.0);
        template.add_kind_of(KindOf::Vehicle);
        template.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
        template.contain_module.slots = Some(3);
        logic.templates.insert("ReturnCarrier".into(), template);
        let id = logic
            .create_object("ReturnCarrier", Team::USA, Vec3::new(10.0, 0.0, 20.0))
            .unwrap();
        (logic, id)
    }

    #[test]
    fn admitted_return_appends_real_goal_once_and_deletes_without_damage() {
        let (mut logic, id) = carrier();
        let (peer, peer_id) = carrier();
        assert_eq!(id, peer_id);
        let origin = logic.host_object(id).unwrap().get_position();
        {
            let object = logic.host_object_mut(id).unwrap();
            object.transport_return_state = Some(TransportReturnState::MoveAndDelete {
                origin,
                append_goal: true,
            });
            object.movement.path = vec![origin, origin + Vec3::X];
            object.status.moving = true;
        }
        logic.prepare_transport_return_path(id);
        assert_eq!(logic.host_object(id).unwrap().movement.path.len(), 3);
        logic.prepare_transport_return_path(id);
        assert_eq!(logic.host_object(id).unwrap().movement.path.len(), 3);
        assert!(logic.host_object(id).unwrap().allow_invalid_position);
        {
            let (object, output) = logic.host_object_and_audio_events_mut(id).unwrap();
            object.stop_moving(&mut crate::game_logic::ObjectAudio::record(output));
        }
        logic.finish_transport_return_phase(id);
        let owner = logic.host_object(id).unwrap();
        assert!(owner.status.destroyed);
        assert_eq!(owner.health.current, 200.0);
        assert!(!peer.host_object(peer_id).unwrap().status.destroyed);
        logic.process_destroy_list();
        assert!(logic.host_object(id).is_none());
        assert!(peer.host_object(peer_id).is_some());
    }

    #[test]
    fn actual_evac_exit_command_moves_out_returns_and_removes_only_after_arrival() {
        use crate::command_executor::CommandExecutor;
        use crate::command_system::CommandResult;
        let (mut logic, id) = carrier();
        let mut rider_template = ThingTemplate::new("ReturnRider");
        rider_template.add_kind_of(KindOf::Infantry);
        logic.templates.insert("ReturnRider".into(), rider_template);
        let rider = logic
            .create_object("ReturnRider", Team::USA, Vec3::new(11.0, 0.0, 20.0))
            .unwrap();
        assert!(logic.host_object_mut(id).unwrap().add_occupant(rider));
        logic.set_fixture_object_contained_by(rider, Some(id));
        let origin = logic.host_object(id).unwrap().get_position();
        let destination = origin + Vec3::new(40.0, 0.0, 0.0);
        assert_eq!(
            CommandExecutor::new(&mut logic, 0).execute_move_to_and_evacuate(
                &[id],
                destination,
                true
            ),
            CommandResult::Success
        );
        let mut saw_return = false;
        for _ in 0..3000 {
            logic.update_movement_for_test(&[id, rider], 1.0 / 30.0);
            let object = logic.host_object(id).unwrap();
            if matches!(
                object.transport_return_state,
                Some(TransportReturnState::MoveAndDelete { .. })
            ) {
                saw_return = true;
                assert_eq!(object.health.current, 200.0);
                assert!(!object.status.destroyed);
                assert!(logic.host_object(rider).unwrap().contained_by.is_none());
            }
            if object.status.destroyed {
                assert!(
                    saw_return,
                    "the return locomotor phase must execute before deletion"
                );
                assert_eq!(object.health.current, 200.0);
                break;
            }
        }
        assert!(
            logic.host_object(id).unwrap().status.destroyed,
            "actual return path must reach direct deletion"
        );
        logic.process_destroy_list();
        assert!(logic.host_object(id).is_none());
        assert!(logic.host_object(rider).is_some());
    }

    #[test]
    fn ai_temporary_move_retains_locked_goal_and_resumes_parent_after_strict_deadline() {
        let (mut logic, id) = carrier();
        let origin = logic.host_object(id).unwrap().get_position();
        let goal = origin + Vec3::new(40.0, 0.0, 0.0);
        assert!(logic.unit_command_set_pending_evacuate(id, true, true, true));
        assert!(logic.assign_unit_path(id, goal, &[]));
        {
            let (owner, output) = logic.host_object_and_audio_events_mut(id).unwrap();
            owner.set_ai_state(
                AIState::Moving,
                &mut crate::game_logic::ObjectAudio::record(output),
            );
            owner.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
        }
        assert!(logic.private_move_to_position(id, Vec3::new(900.0, 0.0, 900.0)));
        let owner = logic.host_object(id).unwrap();
        assert_eq!(owner.temporary_ai_move_remaining(logic.frame), 601);
        let suspended = owner.transport_return_state.unwrap();
        assert_eq!(
            serde_json::from_slice::<TransportReturnState>(
                &serde_json::to_vec(&suspended).unwrap()
            )
            .unwrap(),
            suspended
        );
        assert!(
            matches!(owner.transport_return_state, Some(TransportReturnState::TemporaryMove { origin: saved, returning: false, .. }) if saved == origin)
        );
        assert_ne!(
            owner.movement.path.last().copied(),
            Some(Vec3::new(900.0, 0.0, 900.0))
        );
        logic.finish_transport_return_phase(id);
        assert!(!logic.host_object(id).unwrap().status.destroyed);
        for _ in 0..600 {
            let (object, output) = logic.host_object_and_audio_events_mut(id).unwrap();
            assert!(
                !object.tick_path_queue(0, &mut crate::game_logic::ObjectAudio::queue(output)),
                "transport owns the absolute temporary deadline; observing the path queue does not expire it"
            );
        }
        // Repeated observations in one logic frame never consume the saved deadline.
        assert_eq!(
            logic
                .host_object(id)
                .unwrap()
                .temporary_ai_move_remaining(logic.frame),
            601
        );
        logic.frame = 600;
        logic.finish_transport_return_phase(id);
        assert!(matches!(
            logic.host_object(id).unwrap().transport_return_state,
            Some(TransportReturnState::TemporaryMove { .. })
        ));
        logic.frame = 900; // A receiving-world jump crosses the absolute deadline.
        logic.finish_transport_return_phase(id);
        let owner = logic.host_object(id).unwrap();
        assert_eq!(
            owner.transport_return_state,
            Some(TransportReturnState::MoveAndEvacuate { origin })
        );
        assert!(
            !owner.movement.path.is_empty(),
            "resumed parent performs its real null-path recompute"
        );
        assert!(!owner.status.destroyed);
    }

    #[test]
    fn encoded_world_save_restores_return_and_absolute_temporary_continuations() {
        use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};
        for temporary in [false, true] {
            let (mut source, id) = carrier();
            let origin = source.host_object(id).unwrap().get_position();
            let goal = origin + Vec3::new(50.0, 0.0, 0.0);
            source.frame = 80;
            assert!(source.unit_command_set_pending_evacuate(id, true, true, true));
            assert!(source.assign_unit_path(id, goal, &[]));
            {
                let (object, output) = source.host_object_and_audio_events_mut(id).unwrap();
                object.set_ai_state(
                    AIState::Moving,
                    &mut crate::game_logic::ObjectAudio::record(output),
                );
            }
            if temporary {
                source.host_object_mut(id).unwrap().last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                assert!(source.private_move_to_position(id, Vec3::splat(999.0)));
            } else {
                source.host_object_mut(id).unwrap().transport_return_state =
                    Some(TransportReturnState::MoveAndDelete {
                        origin,
                        append_goal: true,
                    });
            }
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
            let (mut receiving, receiving_id) = carrier();
            let (peer, peer_id) = carrier();
            assert_eq!(id, receiving_id);
            assert_eq!(id, peer_id);
            builder
                .restore_from_snapshot(&decoded, &mut receiving)
                .unwrap();
            assert_eq!(
                receiving.host_object(id).unwrap().transport_return_state,
                source.host_object(id).unwrap().transport_return_state
            );
            assert_eq!(
                receiving
                    .host_object(id)
                    .unwrap()
                    .temporary_ai_move_remaining(receiving.frame),
                source
                    .host_object(id)
                    .unwrap()
                    .temporary_ai_move_remaining(source.frame)
            );
            assert!(receiving.host_object(id).unwrap().pending_evacuate_on_stop);
            if temporary {
                receiving.frame = 680;
                receiving.finish_transport_return_phase(id);
                assert!(matches!(
                    receiving.host_object(id).unwrap().transport_return_state,
                    Some(TransportReturnState::TemporaryMove { .. })
                ));
                receiving.frame = 681;
                receiving.finish_transport_return_phase(id);
                assert!(matches!(
                    receiving.host_object(id).unwrap().transport_return_state,
                    Some(TransportReturnState::MoveAndEvacuate { .. })
                ));
                assert!(!receiving.host_object(id).unwrap().movement.path.is_empty());
            } else {
                receiving.prepare_transport_return_path(id);
                assert!(matches!(
                    receiving.host_object(id).unwrap().transport_return_state,
                    Some(TransportReturnState::MoveAndDelete {
                        append_goal: false,
                        ..
                    })
                ));
                {
                    let (object, output) = receiving.host_object_and_audio_events_mut(id).unwrap();
                    object.stop_moving(&mut crate::game_logic::ObjectAudio::record(output));
                }
                receiving.finish_transport_return_phase(id);
                assert!(receiving.host_object(id).unwrap().status.destroyed);
                assert_eq!(receiving.host_object(id).unwrap().health.current, 200.0);
                receiving.process_destroy_list();
                assert!(receiving.host_object(id).is_none());
            }
            assert!(
                peer.host_object(id)
                    .unwrap()
                    .transport_return_state
                    .is_none()
            );
            assert!(!peer.host_object(id).unwrap().status.destroyed);
        }
    }

    #[test]
    fn return_continuation_roundtrips_and_preserves_locked_move_effects() {
        let (mut logic, id) = carrier();
        assert!(logic.unit_command_set_pending_evacuate(id, true, true, true));
        let state = logic
            .host_object(id)
            .unwrap()
            .transport_return_state
            .unwrap();
        let bytes = serde_json::to_vec(&state).unwrap();
        let restored: TransportReturnState = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(state, restored);
        {
            let owner = logic.host_object_mut(id).unwrap();
            owner.num_frames_blocked = 9;
            owner.is_blocked = true;
            owner.is_blocked_and_stuck = true;
        }
        let path = logic.host_object(id).unwrap().movement.path.clone();
        assert!(logic.unit_command_move_to(id, Vec3::new(90.0, 0.0, 90.0)));
        let owner = logic.host_object(id).unwrap();
        assert_eq!(owner.num_frames_blocked, 0);
        assert!(!owner.is_blocked);
        assert!(!owner.is_blocked_and_stuck);
        assert_eq!(
            owner.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER
        );
        assert_eq!(owner.movement.path, path);
        assert!(logic.unit_command_set_pending_evacuate(id, true, true, true));
        assert_eq!(
            crate::command_executor::CommandExecutor::new(&mut logic, 0)
                .execute_move_to_and_evacuate(&[id], Vec3::new(70.0, 0.0, 70.0), true),
            crate::command_system::CommandResult::Success
        );
        assert_eq!(logic.host_object(id).unwrap().movement.path, path);
        assert_eq!(
            logic.host_object(id).unwrap().transport_return_state,
            Some(state)
        );
    }
}

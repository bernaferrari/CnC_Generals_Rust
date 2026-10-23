//! Receiving transport state and temporary movement continuation in OXOB v15.
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct TransportMovementPersist {
    object_id: u32,
    state: Option<crate::game_logic::host_transport_return::TransportReturnState>,
    temporary_move_frames: u32,
    temporary_move_saved_dest: Option<glam::Vec3>,
    temporary_move_saved_path_goal: Option<glam::Vec3>,
    pending_evacuate: bool,
    pending_exit: bool,
    pending_stream: bool,
}

pub(super) fn capture(game_logic: &GameLogic) -> Vec<TransportMovementPersist> {
    game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| TransportMovementPersist {
            object_id: id.0,
            state: object.transport_return_state,
            temporary_move_frames: object.temporary_move_frames,
            temporary_move_saved_dest: object.temporary_move_saved_dest,
            temporary_move_saved_path_goal: object.temporary_move_saved_path_goal,
            pending_evacuate: object.pending_evacuate_on_stop,
            pending_exit: object.pending_exit_after_evacuate,
            pending_stream: object.pending_stream_exit,
        })
        .collect()
}

pub(super) fn validate(
    game_logic: &GameLogic,
    records: &[TransportMovementPersist],
) -> SaveLoadResult<()> {
    use crate::game_logic::host_transport_return::TransportReturnState;
    let mut ids = std::collections::HashSet::new();
    for entry in records {
        let finite_state = match entry.state {
            Some(TransportReturnState::MoveAndEvacuate { origin })
            | Some(TransportReturnState::MoveAndDelete { origin, .. }) => origin.is_finite(),
            Some(TransportReturnState::TemporaryMove {
                origin,
                parent_goal,
                ..
            }) => origin.is_finite() && parent_goal.is_finite(),
            None => true,
        };
        let temporary_consistent = match entry.state {
            Some(TransportReturnState::TemporaryMove { .. }) => {
                if entry.temporary_move_frames > 0 {
                    entry.temporary_move_frames <= 601 && entry.temporary_move_saved_dest.is_some()
                } else {
                    // Real end_temporary_move_overlay clears both saved goals;
                    // a phase-boundary save may precede the parent's callback.
                    entry.temporary_move_saved_dest.is_none()
                        && entry.temporary_move_saved_path_goal.is_none()
                }
            }
            Some(_) => entry.temporary_move_frames == 0,
            None => true, // Ordinary nontransport temporary movement is independent.
        };
        if !ids.insert(entry.object_id)
            || game_logic.host_object(ObjectId(entry.object_id)).is_none()
            || !finite_state
            || !temporary_consistent
            || entry
                .temporary_move_saved_dest
                .is_some_and(|value| !value.is_finite())
            || entry
                .temporary_move_saved_path_goal
                .is_some_and(|value| !value.is_finite())
        {
            return Err(SaveLoadError::Corrupted(
                "invalid saved transport movement owner/state".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn reset(game_logic: &mut GameLogic) {
    for object in game_logic.host_objects_mut().registered_values_mut() {
        object.transport_return_state = None;
        object.temporary_move_frames = 0;
        object.temporary_move_saved_dest = None;
        object.temporary_move_saved_path_goal = None;
        object.pending_evacuate_on_stop = false;
        object.pending_exit_after_evacuate = false;
        object.pending_stream_exit = false;
    }
}

pub(super) fn apply(game_logic: &mut GameLogic, records: Vec<TransportMovementPersist>) {
    for entry in records {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.transport_return_state = entry.state;
            object.temporary_move_frames = entry.temporary_move_frames;
            object.temporary_move_saved_dest = entry.temporary_move_saved_dest;
            object.temporary_move_saved_path_goal = entry.temporary_move_saved_path_goal;
            object.pending_evacuate_on_stop = entry.pending_evacuate;
            object.pending_exit_after_evacuate = entry.pending_exit;
            object.pending_stream_exit = entry.pending_stream;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_transport_return::TransportReturnState;
    use crate::game_logic::{Team, ThingTemplate};

    #[test]
    fn temporary_record_requires_actual_marker_and_completed_exit_shape() {
        let mut world = GameLogic::new();
        world.templates.insert(
            "TransportCodecOwner".into(),
            ThingTemplate::new("TransportCodecOwner"),
        );
        let id = world
            .create_object("TransportCodecOwner", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let mut records = capture(&world);
        records[0].state = Some(TransportReturnState::TemporaryMove {
            origin: glam::Vec3::ZERO,
            returning: false,
            append_goal: false,
            parent_goal: glam::Vec3::X,
            expires_frame: 600,
        });
        records[0].temporary_move_frames = 601;
        assert!(
            validate(&world, &records).is_err(),
            "active temporary state without saved parent goal is corrupt"
        );
        records[0].temporary_move_saved_dest = Some(glam::Vec3::X);
        assert!(validate(&world, &records).is_ok());
        records[0].temporary_move_frames = 0;
        assert!(
            validate(&world, &records).is_err(),
            "zero marker cannot retain an unexecuted exit's saved goal"
        );
        records[0].temporary_move_saved_dest = None;
        assert!(
            validate(&world, &records).is_ok(),
            "real completed exit may await its parent callback"
        );
        records[0].state = Some(TransportReturnState::MoveAndDelete {
            origin: glam::Vec3::ZERO,
            append_goal: false,
        });
        records[0].temporary_move_frames = 1;
        assert!(validate(&world, &records).is_err());
        let mut payload = super::super::capture(&world);
        payload.transport = records;
        let encoded = bincode_legacy::serialize(&payload).unwrap();
        let mut bytes = super::super::OXOB_MAGIC.to_vec();
        super::super::append_u32(&mut bytes, super::super::OXOB_VERSION);
        super::super::append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        assert!(
            super::super::apply_from_lifecycle_tail(&bytes, &mut world).is_err(),
            "actual decoder rejects the malformed continuation before receiving mutation"
        );
        assert!(
            world
                .host_object(id)
                .unwrap()
                .transport_return_state
                .is_none()
        );
    }
    #[test]
    fn exact_v14_payload_defaults_transport_without_inventing_resume() {
        let mut world = GameLogic::new();
        world.templates.insert(
            "OldTransportCodec".into(),
            ThingTemplate::new("OldTransportCodec"),
        );
        let id = world
            .create_object("OldTransportCodec", Team::USA, glam::Vec3::ZERO)
            .unwrap();
        let current = super::super::capture(&world);
        let old = super::super::ObjectXferPersistPayloadV14 {
            objects: current.objects,
            script_status: current.script_status,
            structures: current.structures,
            damage_info: current.damage_info,
        };
        world.host_object_mut(id).unwrap().transport_return_state =
            Some(TransportReturnState::MoveAndDelete {
                origin: glam::Vec3::ZERO,
                append_goal: true,
            });
        world
            .host_object_mut(id)
            .unwrap()
            .pending_exit_after_evacuate = true;
        let encoded = bincode_legacy::serialize(&old).unwrap();
        let mut bytes = super::super::OXOB_MAGIC.to_vec();
        super::super::append_u32(&mut bytes, 14);
        super::super::append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        super::super::apply_from_lifecycle_tail(&bytes, &mut world).unwrap();
        let owner = world.host_object(id).unwrap();
        assert!(owner.transport_return_state.is_none());
        assert!(!owner.pending_exit_after_evacuate);
        assert!(!owner.status.destroyed);
    }
}

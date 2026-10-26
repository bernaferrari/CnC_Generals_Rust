//! Completed C++ Object disabled-state audio observations.
//! Storage belongs to the receiving match's concrete body output owner.
use super::ObjectId;

#[derive(Debug, Clone, PartialEq)]
pub struct HostDisableAudioEvent {
    pub object: ObjectId,
    pub position: [f32; 3],
    pub event_name: String,
}

#[cfg(test)]
mod owned_audio_tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use glam::Vec3;

    fn world(drone: bool) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::USA, "BodyAudioOwner", true));
        let mut template = ThingTemplate::new("BodyAudioVehicle");
        template.add_kind_of(KindOf::Vehicle).set_health(200.0);
        if drone {
            template.add_kind_of(KindOf::Drone);
        }
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object_for_player("BodyAudioVehicle", 0, Vec3::new(2.0, 3.0, 4.0))
            .unwrap();
        (world, id)
    }
    fn cue(token: &str) -> String {
        crate::game_logic::host_economy_log::resolve_misc_audio_event(token)
    }

    #[test]
    fn real_disable_edges_freeze_position_and_fifo_without_peer_or_health_drains() {
        let (mut owner, id) = world(false);
        let (mut peer, peer_id) = world(false);
        assert_eq!(id, peer_id);
        {
            let (object, outputs) = owner.host_object_and_health_events_mut(id).unwrap();
            object.apply_disabled_emp(
                10,
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            object.apply_disabled_emp(
                11,
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            ); // refresh is not another power edge
            object.apply_disabled_hacked(
                20,
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            object.set_position(Vec3::new(8.0, 9.0, 10.0));
            object.tick_disabled_emp(11, outputs); // another power disable remains
            assert!(!object.status.disabled_emp);
            assert!(object.status.disabled_hacked);
            object.tick_disabled_hacked(20, outputs);
            assert!(!object.is_power_style_disabled());
        }
        // A health boundary must not consume audio produced by those bodies.
        owner.health_events.drain_for_host_boundary();
        let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
        assert!(peer_frame.disable_audio_receipts.is_empty());
        peer.reset();
        let frame = PresentationFrame::build_from_logic(&mut owner, 0);
        assert_eq!(
            frame.disable_audio_receipts,
            vec![
                HostDisableAudioEvent {
                    object: id,
                    position: [2.0, 3.0, 4.0],
                    event_name: cue("VehicleDisabled")
                },
                HostDisableAudioEvent {
                    object: id,
                    position: [8.0, 9.0, 10.0],
                    event_name: cue("VehicleReenabled")
                },
            ]
        );
        let audio_values = |frame: &PresentationFrame| {
            frame
                .collect_audio_events()
                .into_iter()
                .map(|event| {
                    (
                        event.event_type,
                        event.object_id,
                        event.position,
                        event.priority,
                        event.is_looping,
                        event.stop,
                        event.player_index,
                    )
                })
                .collect::<Vec<_>>()
        };
        let expected = audio_values(&frame);
        let (mut peer, peer_id) = world(false);
        let (object, outputs) = peer.host_object_and_health_events_mut(peer_id).unwrap();
        object.apply_disabled_emp(
            100,
            outputs,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        let _ = PresentationFrame::build_from_logic(&mut peer, 0);
        assert_eq!(
            audio_values(&frame),
            expected,
            "a frozen frame never drains foreign sounds"
        );
        assert!(
            PresentationFrame::build_from_logic(&mut owner, 0)
                .disable_audio_receipts
                .is_empty()
        );
    }

    #[test]
    fn real_body_subdual_and_unmanned_outputs_follow_receiver_and_drone_rule() {
        let (mut owner, id) = world(false);
        {
            let (object, outputs) = owner.host_object_and_health_events_mut(id).unwrap();
            object.subdual_damage_cap = 200.0;
            object.subdual_heal_rate_frames = 1;
            object.subdual_heal_amount = 200.0;
            object.apply_subdual_damage(
                200.0,
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            assert!(object.status.disabled_subdued);
            object.tick_subdual_damage(
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            object.tick_subdual_damage(
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            assert!(!object.status.disabled_subdued);
            object.apply_kill_pilot_unmanned(
                outputs,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            assert!(object.is_unmanned());
        }
        let frame = PresentationFrame::build_from_logic(&mut owner, 0);
        assert_eq!(
            frame
                .disable_audio_receipts
                .iter()
                .map(|e| e.event_name.clone())
                .collect::<Vec<_>>(),
            vec![
                cue("VehicleDisabled"),
                cue("VehicleReenabled"),
                cue("SplatterVehiclePilotsBrain")
            ]
        );
        let (mut drone, id) = world(true);
        let (object, outputs) = drone.host_object_and_health_events_mut(id).unwrap();
        object.apply_kill_pilot_unmanned(
            outputs,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert!(object.is_unmanned());
        assert!(
            PresentationFrame::build_from_logic(&mut drone, 0)
                .disable_audio_receipts
                .is_empty()
        );
    }

    #[test]
    fn saved_disable_future_clears_only_committed_receivers_transient_cues() {
        let (mut owner, id) = world(false);
        let (object, outputs) = owner.host_object_and_health_events_mut(id).unwrap();
        object.apply_disabled_emp(
            30,
            outputs,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&owner).unwrap();
        let (mut peer, peer_id) = world(false);
        let (object, outputs) = peer.host_object_and_health_events_mut(peer_id).unwrap();
        object.apply_disabled_hacked(
            99,
            outputs,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
        assert!(
            PresentationFrame::build_from_logic(&mut peer, 0)
                .disable_audio_receipts
                .is_empty()
        );
        assert_eq!(
            peer.host_object(id)
                .unwrap()
                .status
                .disabled_emp_until_frame,
            30
        );
        let (object, outputs) = peer.host_object_and_health_events_mut(id).unwrap();
        object.tick_disabled_emp(30, outputs);
        let restored_frame = PresentationFrame::build_from_logic(&mut peer, 0);
        assert_eq!(restored_frame.disable_audio_receipts.len(), 1);
        assert_eq!(
            restored_frame.disable_audio_receipts[0].event_name,
            cue("VehicleReenabled")
        );
        assert!(owner.host_object(id).unwrap().status.disabled_emp);
        assert_eq!(
            PresentationFrame::build_from_logic(&mut owner, 0)
                .disable_audio_receipts
                .len(),
            1
        );
    }
}

//! Real factory -> receiving PresentationFrame -> actual follow history gate.
use super::*;

#[test]
fn actual_follow_producer_preserves_receiving_debug_history_and_equal_id_peer() {
    use crate::presentation_frame::follow::FollowAdmission;
    let (mut owner, _, id) = world(32, None);
    let (mut peer, _, peer_id) = world(80, None);
    assert_eq!(id, peer_id);
    owner
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    peer.owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(false);
    owner.session_view.target = Vec3::new(100., 0., 110.);
    peer.session_view.target = Vec3::new(200., 0., 210.);
    let owner_target = owner.session_view.target;
    let peer_target = peer.session_view.target;
    let owner_prior = Vec3::new(-31., 0., -37.);
    let peer_prior = Vec3::new(-41., 0., -43.);
    owner.session_view.camera.native_filter_previous_target = owner_prior;
    peer.session_view.camera.native_filter_previous_target = peer_prior;
    let owner_frame = PresentationFrame::build_from_logic(&mut owner, 0);
    let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
    assert_eq!(owner_frame.camera_follow_object_id, Some(id));
    assert_eq!(peer_frame.camera_follow_object_id, Some(peer_id));
    assert_eq!(
        owner_frame.camera_follow_facts.unwrap().position,
        owner.host_object(id).unwrap().get_position()
    );
    assert_eq!(
        peer_frame.camera_follow_facts.unwrap().position,
        peer.host_object(peer_id).unwrap().get_position()
    );
    assert!(
        owner_frame
            .camera_follow_facts
            .unwrap()
            .airborne_orientation
            .is_some()
    );
    assert!(
        peer_frame
            .camera_follow_facts
            .unwrap()
            .airborne_orientation
            .is_none()
    );
    let peer_before = peer.capture_camera_continuation().unwrap();
    assert_eq!(
        owner
            .admit_receiving_camera_follow(
                owner_frame.camera_follow_object_id,
                owner_frame.camera_follow_facts,
                10.,
                false,
                None
            )
            .unwrap(),
        FollowAdmission::Followed
    );
    assert_ne!(owner.session_view.target, owner_target);
    assert_eq!(
        owner.session_view.camera.native_filter_previous_target, owner_prior,
        "C++ history is frozen for this receiving debugger while follow still moves"
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    let owner_before = owner.capture_camera_continuation().unwrap();
    assert_eq!(
        peer.admit_receiving_camera_follow(
            peer_frame.camera_follow_object_id,
            peer_frame.camera_follow_facts,
            10.,
            false,
            None
        )
        .unwrap(),
        FollowAdmission::Followed
    );
    assert_ne!(peer.session_view.target, peer_target);
    assert_eq!(
        peer.session_view.camera.native_filter_previous_target,
        peer_target
    );
    assert_eq!(owner.capture_camera_continuation().unwrap(), owner_before);
    owner.force_script_debug_continue(None).unwrap();
    let owner_previous = owner.session_view.target;
    let peer_before = peer.capture_camera_continuation().unwrap();
    owner
        .admit_receiving_camera_follow(
            owner_frame.camera_follow_object_id,
            owner_frame.camera_follow_facts,
            10.,
            false,
            None,
        )
        .unwrap();
    assert_eq!(
        owner.session_view.camera.native_filter_previous_target,
        owner_previous
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

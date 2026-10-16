use super::*;
use crate::{
    presentation_frame::camera::continuation::SessionCameraState,
    save_load::camera_codec::CameraCodec,
};

fn world(ground: f32) -> GameLogic {
    let mut world = GameLogic::new();
    world.override_world_size(100., 100.);
    let (width, height, _) = world.snapshot_pathfinding_passability();
    assert!(world.restore_terrain_heights_from_grid(
        width,
        height,
        &vec![ground; (width * height) as usize]
    ));
    world.session_view.target = Vec3::new(0., 30., 0.);
    world
        .session_view
        .camera
        .native_camera
        .initialize_map(ground, 400., 37.5, 12.);
    world
        .session_view
        .camera
        .native_camera
        .set_height(650., 10., 1000.);
    world.session_view.camera.camera_yaw_radians = 0.75;
    world.session_view.camera.camera_fx_pitch = 1.25;
    world.session_view.impulse.offset = Vec3::new(3., 7., -5.);
    world.session_view.commit_orbit_pose();
    world
}

#[test]
fn completed_audio_observation_uses_receiving_hag_fx_heading_and_raw_ground() {
    let receiving = world(17.);
    let mut peer = world(91.);
    let codec = CameraCodec::<{ 1024 * 1024 }>;
    let before = codec
        .serialize(&SessionCameraState::capture(&receiving.session_view.camera))
        .unwrap();
    let completed_eye = receiving.session_view.camera.camera_position;
    for frame in 0..120 {
        peer.session_view.camera.camera_yaw_radians = frame as f32;
        peer.session_view.commit_orbit_pose();
        let peer_view = CompletedAudioView::capture(&peer);
        assert_eq!(peer_view.listener.ground_height, 91.);
        let view = CompletedAudioView::capture(&receiving);
        assert_eq!(view.listener.ground_height, 17.);
        assert_eq!(
            (
                view.listener.ground_position.x,
                view.listener.ground_position.y,
                view.listener.ground_position.z
            ),
            (0., 0., 30.)
        );
        assert_eq!(
            (
                view.listener.camera_position.x,
                view.listener.camera_position.y,
                view.listener.camera_position.z
            ),
            (completed_eye.x, completed_eye.z, completed_eye.y)
        );
        assert_eq!(
            view.listener.heading, 0.75,
            "view angle is independent of shake, FX and map basis yaw"
        );
        let mut audio = game_engine::common::audio::game_audio::AudioManager::new();
        audio.update_with_view(view.listener, &mut |position| view.is_on_screen(position));
        assert!(audio.get_listener_position().x.is_finite());
    }
    assert_eq!(
        codec
            .serialize(&SessionCameraState::capture(&receiving.session_view.camera))
            .unwrap(),
        before,
        "observation cannot advance HAG, shake or saved camera state"
    );
}

#[test]
fn audio_visibility_uses_full_receiving_frustum_with_inclusive_edges() {
    let mut observation = CompletedAudioView::capture(&world(0.));
    observation.view = Mat4::IDENTITY;
    // Exactly representable D3D projection: near=10, far=20, half-FOV=45°.
    observation.projection = Mat4::from_cols(
        glam::Vec4::X,
        glam::Vec4::Y,
        glam::Vec4::new(0., 0., -2., -1.),
        glam::Vec4::new(0., 0., -20., 0.),
    );
    let records: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("audio_frustum_cpp_records.json")).unwrap();
    assert_eq!(records.len(), 12);
    for record in records {
        let p = record["position"].as_array().unwrap();
        let position = Coord3D {
            x: p[0].as_f64().unwrap() as f32,
            y: p[1].as_f64().unwrap() as f32,
            z: p[2].as_f64().unwrap() as f32,
        };
        assert_eq!(
            observation.is_on_screen(&position),
            record["on_screen"].as_bool().unwrap(),
            "{record}"
        );
    }
    assert!(!observation.is_on_screen(&Coord3D {
        x: f32::NAN,
        y: -15.,
        z: 0.
    }));
}

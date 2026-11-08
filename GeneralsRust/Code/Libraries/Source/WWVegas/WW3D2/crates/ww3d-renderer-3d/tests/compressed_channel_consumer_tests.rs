use glam::{Quat, Vec3};
use ww3d_animation::HTreeClass;
use ww3d_renderer_3d::w3d_animation_loader::{AnimationPlayback, W3DAnimationLoader};

mod fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../ww3d-animation/tests/support/compressed_fixture.rs"));
}

fn hierarchy() -> HTreeClass {
    let mut tree = HTreeClass::with_name("TEST");
    tree.init_default();
    tree.add_pivot("BONE", 0, Vec3::ZERO, Quat::IDENTITY);
    tree
}

#[test]
fn canonical_compressed_renderer_adaptive_random_seeks_and_independent_players() {
    let asset = W3DAnimationLoader::load_animation_from_buffer(
        &fixture::adaptive_animation(), "TEST.CLIP", &hierarchy(),
    ).unwrap();
    let mut first = AnimationPlayback::new(asset);
    let mut second = AnimationPlayback {
        animation: first.animation.clone(), current_frame: 0.0,
        mode: first.mode, is_playing: true, playback_direction: 1.0,
    };
    // Same immutable asset, independently changing receiving playback positions.
    assert!(std::sync::Arc::ptr_eq(&first.animation, &second.animation));
    for frame in [0.0, 1.0, 2.0, 15.5, 16.0, 31.5, 32.0, 1.5, 17.5, 0.5] {
        first.current_frame = frame;
        second.current_frame = 32.0 - frame;
        for player in [&first, &second] {
            let frame = player.current_frame;
            let expected = fixture::adaptive_expected(frame);
            fixture::assert_pose(player.get_bone_transform(1), expected, frame);
            assert_eq!(player.get_bone_translation(1), expected.0);
            assert!((player.get_bone_rotation(1) - expected.1).length() < 0.000002);
            assert_eq!(player.get_bone_visibility(1), fixture::visible(frame));
        }
        assert_eq!(first.current_frame, frame);
    }
    // Sequential and random access are bit-identical, including packet ends.
    for frame in 0..33 {
        first.current_frame = frame as f32;
        let sequential = first.get_bone_rotation(1).to_array().map(f32::to_bits);
        second.current_frame = 32.0;
        second.get_bone_rotation(1);
        second.current_frame = frame as f32;
        assert_eq!(second.get_bone_rotation(1).to_array().map(f32::to_bits), sequential);
    }
}

#[test]
fn canonical_compressed_renderer_steps_fractional_antipodal_and_bits() {
    let asset = W3DAnimationLoader::load_animation_from_buffer(
        &fixture::timecoded_animation(), "TEST.CLIP", &hierarchy(),
    ).unwrap();
    let mut player = AnimationPlayback::new(asset);
    for frame in [0.0, 0.5, 2.0, 2.999, 3.0, 3.999, 4.0, 4.5, 5.0, 7.5, 8.0, 20.0] {
        player.current_frame = frame;
        fixture::assert_pose(player.get_bone_transform(1), fixture::timecoded_expected(frame), frame);
        assert_eq!(player.get_bone_visibility(1), fixture::visible(frame));
    }
    player.current_frame = 0.5;
    assert_eq!(player.get_current_frame(), 0, "integer reporting must not quantize sampling");
    assert!(player.get_bone_rotation(1).x > 0.09);
}

#[test]
fn canonical_compressed_renderer_rejects_malformed_loads() {
    for (index, bytes) in fixture::malformed_animations().iter().enumerate() {
        assert!(W3DAnimationLoader::load_animation_from_buffer(bytes, "invalid", &hierarchy()).is_err(),
            "malformed fixture {index}");
    }
}

#[test]
fn canonical_compressed_renderer_single_frame_needs_no_delta_packet() {
    let asset = W3DAnimationLoader::load_animation_from_buffer(
        &fixture::single_frame_animation(), "TEST.CLIP", &hierarchy(),
    ).unwrap();
    let mut player = AnimationPlayback::new(asset);
    for frame in [0.0, 0.5, 20.0] {
        player.current_frame = frame;
        assert_eq!(player.get_bone_translation(1), Vec3::new(2.5, 0.0, 0.0));
        assert_eq!(player.get_bone_rotation(1), Quat::IDENTITY);
        assert!(player.get_bone_visibility(1));
    }
}

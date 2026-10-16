use super::*;
use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
use crate::presentation_frame::camera::clip::CameraClipRules;
fn map_file(dir: &std::path::Path, draw: bool) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join("Clip.map");
    let mut data = game_engine::common::system::DataChunkOutput::new();
    data.open_data_chunk("GlobalLighting", 1);
    data.write_int(2);
    for _ in 0..4 {
        for _ in 0..2 {
            for n in [0.1, 0.2, 0.3, 0.8, 0.8, 0.8, 0., 0., -1.] {
                data.write_real(n);
            }
        }
    }
    data.close_data_chunk();
    std::fs::write(&path, data.into_ckmp_bytes()).unwrap();
    std::fs::write(
        dir.join("Map.ini"),
        format!(
            "GameData\n DrawEntireTerrain = {}\n ShakeNormalIntensity = 11\n End\n",
            if draw { "No" } else { "Yes" }
        ),
    )
    .unwrap();
    // Solo.ini follows Map.ini in the actual receiving definition loader.
    std::fs::write(
        dir.join("Solo.ini"),
        format!(
            "GameData\n DrawEntireTerrain = {}\n ShakeNormalIntensity = {}\n End\n",
            if draw { "Yes" } else { "No" },
            if draw { 7 } else { 3 }
        ),
    )
    .unwrap();
    path.to_str().unwrap().into()
}
fn setup(world: &mut GameLogic) {
    world.session_view.target = Vec3::new(1000., 0., 1000.);
    world
        .session_view
        .camera
        .native_camera
        .initialize_map(0., 300., 37.5, 0.);
    world.add_object(Object::new(
        ThingTemplate::new("ClipOwner"),
        ObjectId(42),
        Team::USA,
    ));
}
fn far_is_extended(world: &GameLogic) -> bool {
    let p = world.session_view.camera.projection_matrix * glam::Vec4::new(0., 0., -2000., 1.);
    p.z < p.w
}
#[test]
fn actual_receiving_map_clip_rules_ignore_foreign_map_and_ambient_definition() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_receiving_map_clip_rules_ignore_foreign_map_and_ambient_definition",
        || {
            let temp = tempfile::tempdir().unwrap();
            let a_path = map_file(&temp.path().join("A"), false);
            let b_path = map_file(&temp.path().join("B"), true);
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            a.admit_camera_application_clip(CameraClipRules {
                draw_entire_terrain: true,
            });
            b.admit_camera_application_clip(CameraClipRules::default());
            assert!(a.load_map(&a_path));
            assert!(b.load_map(&b_path));
            assert_eq!(
                a.session_view
                    .camera
                    .shake_policy
                    .current()
                    .shake_normal_intensity,
                3.
            );
            assert_eq!(
                b.session_view
                    .camera
                    .shake_policy
                    .current()
                    .shake_normal_intensity,
                7.
            );
            setup(&mut a);
            setup(&mut b);
            assert!(
                !a.session_view
                    .camera
                    .clip_policy
                    .current()
                    .draw_entire_terrain
            );
            assert!(
                b.session_view
                    .camera
                    .clip_policy
                    .current()
                    .draw_entire_terrain
            );
            // A real foreign global differs deliberately; production kernel must not read it.
            let old = game_engine::common::global_data::read().draw_entire_terrain;
            game_engine::common::global_data::write().draw_entire_terrain = true;
            let b_before = b.capture_camera_continuation().unwrap();
            a.commit_native_camera_with_effects(&mut Default::default())
                .unwrap();
            assert!(!far_is_extended(&a));
            assert_eq!(b_before, b.capture_camera_continuation().unwrap());
            b.commit_native_camera_with_effects(&mut Default::default())
                .unwrap();
            assert!(far_is_extended(&b));
            let a_before = a.capture_camera_continuation().unwrap();
            b.reset();
            assert_eq!(
                b.session_view
                    .camera
                    .shake_policy
                    .current()
                    .shake_normal_intensity,
                1.
            );
            assert!(
                !b.session_view
                    .camera
                    .clip_policy
                    .current()
                    .draw_entire_terrain
            );
            assert_eq!(a_before, a.capture_camera_continuation().unwrap());
            a.reset();
            assert_eq!(
                a.session_view
                    .camera
                    .shake_policy
                    .current()
                    .shake_normal_intensity,
                1.
            );
            assert!(
                a.session_view
                    .camera
                    .clip_policy
                    .current()
                    .draw_entire_terrain
            );
            game_engine::common::global_data::write().draw_entire_terrain = old;
        },
    );
}
#[test]
fn v6_preserves_pending_clip_inputs_v5_is_frozen_and_uses_receiving_policy() {
    let mut source = GameLogic::new();
    setup(&mut source);
    source
        .commit_native_camera_with_effects(&mut Default::default())
        .unwrap();
    assert!(!far_is_extended(&source));
    source.admit_camera_map_clip(CameraClipRules {
        draw_entire_terrain: true,
    });
    source
        .admit_camera_map_shake(game_engine::common::ini::ini_game_data::CameraShakeRules {
            max_shake_range: 543.,
            ..Default::default()
        })
        .unwrap();
    let saved_shake = source.session_view.camera.shake_policy;
    let installed = source.session_view.camera.projection_matrix;
    let bytes = source.capture_camera_continuation().unwrap();
    assert_eq!(&bytes[4..8], &6u32.to_le_bytes());
    let current: EnvelopeV6 = codec().deserialize(&bytes[8..]).unwrap();
    let mut old = MAGIC.to_vec();
    old.extend_from_slice(&5u32.to_le_bytes());
    old.extend(codec().serialize(&current.base).unwrap());
    let old_decoded = GameLogic::decode_camera_continuation(&old).unwrap();
    assert_eq!(old_decoded.owned_clip_policy(), None);
    assert_eq!(old_decoded.owned_shake_policy(), None);
    let peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&bytes).unwrap())
        .unwrap();
    assert_eq!(restored.session_view.camera.projection_matrix, installed);
    assert_eq!(restored.session_view.camera.shake_policy, saved_shake);
    assert!(
        restored
            .session_view
            .camera
            .clip_policy
            .current()
            .draw_entire_terrain
    );
    restored
        .commit_native_camera_with_effects(&mut Default::default())
        .unwrap();
    assert!(far_is_extended(&restored));
    let mut legacy = GameLogic::new();
    let legacy_shake = legacy.session_view.camera.shake_policy;
    legacy.admit_camera_application_clip(CameraClipRules::default());
    legacy.admit_camera_continuation(old_decoded).unwrap();
    assert_eq!(legacy.session_view.camera.shake_policy, legacy_shake);
    assert_eq!(legacy.session_view.camera.projection_matrix, installed);
    legacy
        .commit_native_camera_with_effects(&mut Default::default())
        .unwrap();
    assert!(!far_is_extended(&legacy));
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
    let mut bad = bytes.clone();
    let shake_len = codec().serialize(&current.shake_policy).unwrap().len();
    let clip_end = bad.len() - shake_len;
    bad[clip_end - 1] = 2;
    let mut bad_shake = bytes.clone();
    let tail = bad_shake.len();
    bad_shake[tail - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(GameLogic::decode_camera_continuation(&bad_shake).is_err());
    let before = restored.capture_camera_continuation().unwrap();
    assert!(GameLogic::decode_camera_continuation(&bad).is_err());
    assert_eq!(before, restored.capture_camera_continuation().unwrap());
}

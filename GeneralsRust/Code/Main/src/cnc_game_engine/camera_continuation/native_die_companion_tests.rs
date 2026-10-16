//! Actual archive/native codec and borrowed owner admission. This does not
//! construct CnCGameEngine, a window, or a GPU staged-restore boundary.
use super::*;
use crate::game_logic::{GameLogic, Player, Team};
use crate::save_load::snapshot::{ClientDrawableWorldSnapshot, ClientSaveState, SnapshotBuilder};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use game_engine::common::ini::ini_fx_list as fx;

struct Observe(std::sync::mpsc::Sender<()>);
impl game_client::fx_list::FXNugget for Observe {
    fn do_fx_pos(
        &self,
        _: Option<&gamelogic::common::Coord3D>,
        _: Option<&gamelogic::common::Matrix3D>,
        _: f32,
        _: Option<&gamelogic::common::Coord3D>,
        _: f32,
    ) {
        panic!("authored Die keeps object primary dispatch");
    }
    fn do_fx_obj_host(
        &self,
        _: &gamelogic::helpers::HostFxObjectPose,
        _: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        self.0.send(()).unwrap();
    }
}
fn world() -> (GameLogic, crate::game_logic::ObjectId) {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(
        "Object NativeCompanionDie\nKindOf = INERT IMMOBILE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = FXListDie Shake\nDeathFX = FX_NativeCompanionDie\nEnd\nBehavior = KeepObjectDie Retain\nEnd\nEnd",
        "native-companion-die.ini",
    ).unwrap();
    let template = GameLogic::build_template_from_object_definition(
        "NativeCompanionDie",
        parser.get_definition("NativeCompanionDie").unwrap(),
        None,
    );
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("NativeCompanionDie", Team::USA, Vec3::ZERO)
        .unwrap();
    world.objects.get_mut(&id).unwrap().owner_player_id = Some(1);
    {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.init_shroud_grid(128., 128.);
        shroud.reveal_map_for_player_permanently(1).unwrap();
        shroud.set_host_object_shroud_status(1, id.0, gamelogic::common::ObjectShroudStatus::Clear);
    }
    (world, id)
}
#[test]
#[cfg(feature = "game_client")]
fn native_die_camera_companion_archive_restores_view_height_and_receipts_without_replay() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_die_camera_companion_archive_restores_view_height_and_receipts_without_replay",
        || {
            let mut definition = fx::FXList::new("FX_NativeCompanionDie".into());
            definition.add_nugget(fx::FXNugget::ViewShake {
                shake_type: fx::CameraShakeType::Normal,
            });
            let (sender, events) = std::sync::mpsc::channel();
            let mut list = game_client::fx_list::FXList::from_definition(&definition);
            list.add_fx_nugget(Box::new(Observe(sender)));
            game_client::fx_list::get_fx_list_store_mut()
                .add_fx_list("FX_NativeCompanionDie".into(), list);
            let (mut source, id) = world();
            let (mut receiving, same_id) = world();
            let (mut peer, peer_id) = world();
            assert_eq!(id, same_id);
            assert_eq!(id, peer_id);
            peer.session_view.target = Vec3::splat(77.);
            let peer_view = peer.session_view.capture();
            source
                .apply_owned_damage(
                    id,
                    100.,
                    None,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &crate::game_logic::object::DamageHitContext::default(),
                )
                .unwrap();
            assert_eq!(events.try_iter().count(), 1);
            let view = source.session_view.capture();
            assert!(
                view.intensity > 0.,
                "actual receiving death ViewShake must execute"
            );
            let receipts = source
                .host_object(id)
                .unwrap()
                .native_die_runtime
                .completed
                .clone();
            assert_eq!(receipts.len(), 2);
            let mut state = fixture();
            state.camera_target = view.target;
            state.screen_shake_intensity = view.intensity;
            state.screen_shake_angle_cos = view.cosine;
            state.screen_shake_angle_sin = view.sine;
            state.camera_shake_offset = view.offset;
            fixture_on_owner(&mut state, &mut source);
            let expected_camera =
                crate::presentation_frame::camera::continuation::SessionCameraState::capture(
                    &source.session_view.camera,
                );
            let expected_height = codec().serialize(&state.height).unwrap();
            let expected_view = view;
            let directory = tempfile::tempdir().unwrap();
            let mut saves = SaveFileManager::with_save_directory(directory.path());
            let info = SaveGameInfo {
                filename: "native_die".into(),
                display_name: "native_die".into(),
                description: "native_die".into(),
                map_name: "native_die".into(),
                pristine_map_name: None,
                campaign_side: None,
                mission_number: None,
                save_date: std::time::UNIX_EPOCH,
                game_version: "test".into(),
                play_time: Duration::ZERO,
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            let mut client = game_client::core::game_client::GameClient::new().unwrap();
            saves
                .save_game_with_client_save_state(
                    "native_die",
                    &source,
                    ClientDrawableWorldSnapshot::default(),
                    &mut client,
                    &info,
                    ClientSaveState {
                        camera: None,
                        camera_continuation: Some(codec().serialize(&state).unwrap()),
                        control_groups: None,
                    },
                )
                .unwrap();
            let (snapshot, _) = saves.load_game_snapshot("native_die").unwrap();
            let native = crate::save_load::snapshot::read_native_camera_continuation(
                &snapshot.lifecycle_tail,
            )
            .unwrap()
            .unwrap();
            SnapshotBuilder::new()
                .restore_from_snapshot(&snapshot, &mut receiving)
                .unwrap();
            assert_eq!(
                codec().serialize(&crate::presentation_frame::camera::continuation::SessionCameraState::capture(
                    &receiving.session_view.camera)).unwrap(),
                codec().serialize(&expected_camera).unwrap(),
                "the real world snapshot restores its complete owned camera without a native window"
            );
            assert_eq!(
                codec()
                    .serialize(
                        &receiving
                            .session_view
                            .camera
                            .native_camera
                            .capture_continuation()
                    )
                    .unwrap(),
                expected_height
            );
            // Same explicit owner admissions as the Engine native contract;
            // no claim that the full Engine wrapper was constructed here.
            let saved = native.0;
            let mut height = CameraHeight::new(900., 20., 0.);
            height.admit_continuation(saved.height);
            receiving
                .session_view
                .admit(crate::presentation_frame::SessionViewState {
                    target: saved.camera_target,
                    intensity: saved.screen_shake_intensity,
                    cosine: saved.screen_shake_angle_cos,
                    sine: saved.screen_shake_angle_sin,
                    offset: saved.camera_shake_offset,
                })
                .unwrap();
            assert_eq!(
                codec().serialize(&height.capture_continuation()).unwrap(),
                expected_height
            );
            assert_eq!(receiving.session_view.capture(), expected_view);
            assert_eq!(
                receiving
                    .host_object(id)
                    .unwrap()
                    .native_die_runtime
                    .completed,
                receipts
            );
            receiving
                .apply_owned_damage(
                    id,
                    1.,
                    None,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                    None,
                    &crate::game_logic::object::DamageHitContext::default(),
                )
                .unwrap();
            assert_eq!(
                receiving
                    .host_object(id)
                    .unwrap()
                    .native_die_runtime
                    .completed,
                receipts
            );
            assert_eq!(receiving.session_view.capture(), expected_view);
            assert_eq!(peer.session_view.capture(), peer_view);
            assert!(
                peer.host_object(id)
                    .unwrap()
                    .native_die_runtime
                    .completed
                    .is_empty()
            );
            assert_eq!(
                events.try_iter().count(),
                0,
                "archive restoration and repeated death must not replay Die FX"
            );
        },
    );
}

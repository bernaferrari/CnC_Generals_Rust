//! Actual authored interpreter actions -> checked native frame fade sampling.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use crate::presentation_frame::{PresentationCameraFade, PresentationFrame};
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn fade_action(kind: ScriptActionType, min: f32, max: f32) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    for value in [min, max] {
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, value))
            .unwrap();
    }
    for frames in [4, 1, 2] {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, frames))
            .unwrap();
    }
    action
}
fn author(world: &mut GameLogic, kind: ScriptActionType, min: f32, max: f32) {
    world
        .with_owned_script_engine(|engine, world| {
            engine.friend_execute_action_with_driver(
                &fade_action(kind, min, max),
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::with_interpreter(world, engine),
            );
        })
        .unwrap();
}
fn advance(world: &mut GameLogic) {
    world
        .with_owned_script_engine(|engine, world| {
            engine.update_with_driver(
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::with_interpreter(world, engine),
            )
        })
        .unwrap()
        .unwrap();
}
fn freeze(world: &mut GameLogic) -> PresentationCameraFade {
    PresentationFrame::try_build_from_logic(world, 0)
        .unwrap()
        .camera_fade
}

#[test]
fn native_fade_freezes_actual_authored_receiver_without_global_selection_or_clock_advance() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_fade_freezes_actual_authored_receiver_without_global_selection_or_clock_advance",
        || {
            let mut first = GameLogic::new();
            let mut peer = GameLogic::new();
            let mut no_fade = GameLogic::new();
            author(&mut first, ScriptActionType::CameraFadeAdd, 0.25, 0.75);
            author(&mut peer, ScriptActionType::CameraFadeMultiply, 0.75, 1.0);
            let scripts = gamelogic::scripting::get_script_engine();
            {
                let mut foreign = scripts.write().unwrap();
                *foreign = Some(gamelogic::scripting::engine::ScriptEngine::new().unwrap());
                foreign.as_ref().unwrap().set_fade_parameters(
                    gamelogic::scripting::TFade::Subtract,
                    0.1,
                    0.2,
                    4,
                    1,
                    2,
                );
            }
            let first_tail = first
                .owned_script_engine
                .as_ref()
                .unwrap()
                .snapshot_xfer_tail();
            let peer_tail = peer
                .owned_script_engine
                .as_ref()
                .unwrap()
                .snapshot_xfer_tail();
            let hag = first.session_view.capture();
            let a = freeze(&mut first);
            assert_eq!(
                a,
                PresentationCameraFade {
                    fade: 2,
                    intensity: 0.25,
                    diffuse: 0xff3f3f3f
                }
            );
            assert_eq!(
                freeze(&mut peer),
                PresentationCameraFade {
                    fade: 4,
                    intensity: 0.75,
                    diffuse: 0xffbfbfbf
                }
            );
            let foreign = scripts.write().unwrap();
            assert_eq!(
                freeze(&mut no_fade),
                PresentationCameraFade::default(),
                "an actual receiving no-fade interpreter cannot inherit the foreign fade"
            );
            assert_eq!(
                freeze(&mut first),
                a,
                "render frequency cannot advance the receiving fade clock or acquire a foreign interpreter"
            );
            assert_eq!(
                first
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .snapshot_xfer_tail(),
                first_tail
            );
            assert_eq!(
                peer.owned_script_engine
                    .as_ref()
                    .unwrap()
                    .snapshot_xfer_tail(),
                peer_tail
            );
            assert_eq!(
                first.session_view.capture(),
                hag,
                "sampling cannot advance HAG/camera state"
            );
            drop(foreign);
            advance(&mut first);
            let before_save = freeze(&mut first);
            assert!(before_save.intensity > a.intensity);
            assert_eq!(freeze(&mut peer).intensity, 0.75);
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&first).unwrap();
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded =
                crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
            // The positional legacy body remains unchanged; its framed SESC
            // continuation now retains the original ScriptEngine state too.
            // The actual .sav still carries CHUNK_ScriptEngine separately.
            assert_eq!(
                decoded
                    .persist_v18
                    .script_engine_tail
                    .as_ref()
                    .expect("encoded receiving ScriptEngine continuation")
                    .cur_fade_value,
                before_save.intensity
            );
            let saved_tail = snapshot.persist_v18.script_engine_tail.as_ref().unwrap();
            assert_eq!(saved_tail.cur_fade_value, before_save.intensity);
            let saves = tempfile::TempDir::new().unwrap();
            let mut manager = crate::save_load::SaveFileManager::with_save_directory(saves.path());
            manager.init().unwrap();
            let info = crate::save_load::SaveGameInfo {
                filename: "native_fade_owner".into(),
                display_name: "Native fade owner".into(),
                description: "Actual ScriptEngine archive tail continuation".into(),
                map_name: first.get_current_map_name().to_string(),
                pristine_map_name: None,
                campaign_side: None,
                mission_number: None,
                save_date: std::time::SystemTime::now(),
                game_version: env!("CARGO_PKG_VERSION").into(),
                play_time: std::time::Duration::ZERO,
                difficulty: crate::save_load::GameDifficulty::Medium,
                save_type: crate::save_load::SaveFileType::Normal,
            };
            manager
                .save_game("native_fade_owner", &first, &info)
                .unwrap();
            let archived = manager.load_game_snapshot("native_fade_owner").unwrap();
            assert_eq!(
                archived.snapshot.persist_v18.script_engine_tail.as_ref(),
                Some(saved_tail),
                "the actual archive must restore the exact original ScriptEngine tail"
            );
            let mut restored = GameLogic::new();
            manager
                .restore_game_snapshot(&archived.snapshot, &mut restored)
                .unwrap();
            assert_eq!(freeze(&mut restored), before_save);
            for _ in 0..9 {
                advance(&mut first);
                advance(&mut restored);
                assert_eq!(
                    freeze(&mut first),
                    freeze(&mut restored),
                    "saved actual interpreter fade preserves the future"
                );
            }
            peer.reset();
            assert_eq!(freeze(&mut first), freeze(&mut restored));
        },
    );
}

#[test]
fn native_fade_rejects_actual_interpreter_loan_before_frame_publication() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_fade_rejects_actual_interpreter_loan_before_frame_publication",
        || {
            let mut world = GameLogic::new();
            author(&mut world, ScriptActionType::CameraFadeAdd, 0.25, 0.75);
            let receipt = crate::game_logic::host_weapon_discharge_log::HostWeaponDischargeEvent {
                source: ObjectId(901),
                weapon_slot: 0,
                fired_barrel: 0,
                sequence: 1,
                logic_frame: 0,
                visual_plan: None,
            };
            world.weapon_discharge_log.record(receipt.clone());
            world.health_events.record_applied_damage(
                ObjectId(901),
                5.0,
                None,
                false,
                0,
                crate::game_logic::host_damage_log::BodyHealthObservation {
                    current: 35.0,
                    maximum: 40.0,
                },
            );
            let completed = world.health_events.drain_for_host_boundary();
            assert_eq!(
                completed.len(),
                1,
                "complete the actual health observation boundary before frame delivery"
            );
            let before = world
                .owned_script_engine
                .as_ref()
                .unwrap()
                .snapshot_xfer_tail();
            world.with_owned_script_engine(|engine, world| {
        assert!(world.owned_script_engine.is_none());
        let query = PresentationFrame::try_build_from_logic(world, 0);
        assert!(matches!(query, Err(ref error) if error.contains("receiving interpreter is unavailable or lent")), "a loaned native query must reject before constructing a frame");
        let publication = PresentationFrame::try_publish_for_engine_with_runtime_heightmap(world, 0, None, None, true);
        assert!(matches!(publication, Err(ref error) if error.contains("receiving interpreter is unavailable or lent")),
            "a loaned native interpreter must be a checked error, never a no-fade frame");
        assert_eq!(engine.snapshot_xfer_tail(), before);
    }).unwrap();
            assert_eq!(
                world
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .snapshot_xfer_tail(),
                before
            );
            assert_eq!(
                world.take_weapon_discharges_for_presentation(),
                vec![receipt],
                "failed native publication must retain the actual owned discharge transport"
            );
            let damage = world.health_events.take_last_damage();
            assert_eq!(
                damage.len(),
                1,
                "failed native publication must retain owned health receipts"
            );
            assert_eq!(damage[0].amount, 5.0);
            assert_eq!(freeze(&mut world).intensity, 0.25);
        },
    );
}

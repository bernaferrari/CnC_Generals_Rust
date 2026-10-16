//! Actual native staging admission, without constructing a GPU engine.
use crate::game_logic::GameLogic;
use gamelogic::runtime_world_transaction::HostRuntimeWorldStage;

#[test]
fn native_team_preflight_empty_guard_batches_finish_without_global_script_discovery() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_team_preflight_empty_guard_batches_finish_without_global_script_discovery",
        || {
            let mut owner = GameLogic::new();
            let peer = GameLogic::new();
            owner.session_view.target = glam::Vec3::new(11.0, 22.0, 33.0);
            let expected = owner.session_view.target;
            let peer_target = peer.session_view.target;
            let stage = HostRuntimeWorldStage::begin(&owner.world_services);
            // Real guard drop defers a receipt even though no callback is pending.
            drop(gamelogic::team::get_team_factory().lock().unwrap());
            drop(gamelogic::team::get_team_factory().lock().unwrap());
            let candidate = stage.finish_and_restore_live(&owner.world_services);
            let checked = candidate.preflight_native().unwrap();
            checked.install_globals().finish_after_logic_commit();
            assert_eq!(owner.session_view.target, expected);
            assert_eq!(peer.session_view.target, peer_target);
            assert!(
                gamelogic::scripting::engine::get_script_engine()
                    .read()
                    .unwrap()
                    .is_none()
            );
        },
    );
}

#[test]
fn native_team_preflight_unexpected_create_rejects_before_live_session_or_camera_changes() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_team_preflight_unexpected_create_rejects_before_live_session_or_camera_changes",
        || {
            use game_engine::common::dict::Dict;
            use game_engine::common::well_known_keys::{
                key_team_executes_actions_on_create, key_team_production_condition,
            };
            use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            owner.session_view.target = glam::Vec3::new(17.0, 23.0, 31.0);
            peer.session_view.target = glam::Vec3::new(-5.0, 9.0, 80.0);
            let owner_target = owner.session_view.target;
            let peer_target = peer.session_view.target;
            let foreign = ScriptEngine::new().unwrap();
            foreign.set_counter("ForeignCallback", 41).unwrap();
            *get_script_engine().write().unwrap() = Some(foreign);
            let stage = HostRuntimeWorldStage::begin(&owner.world_services);
            let mut definition = Dict::new();
            definition.set_bool(key_team_executes_actions_on_create(), true);
            definition.set_ascii_string(key_team_production_condition(), "ForeignCallback");
            {
                let mut factory = gamelogic::team::get_team_factory().lock().unwrap();
                assert!(
                    factory
                        .init_team(
                            "UnexpectedCoreTeam".into(),
                            "".into(),
                            true,
                            Some(&definition)
                        )
                        .is_some()
                );
            }
            let candidate = stage.finish_and_restore_live(&owner.world_services);
            let error = match candidate.preflight_native() {
                Ok(_) => panic!("unexpected Core payload must fail before native installation"),
                Err(error) => error,
            };
            assert!(error.contains("1 create, 0 generic"), "{error}");
            assert_eq!(owner.session_view.target, owner_target);
            assert_eq!(peer.session_view.target, peer_target);
            let handle = get_script_engine();
            let interpreter = handle.read().unwrap();
            assert_eq!(
                interpreter
                    .as_ref()
                    .unwrap()
                    .get_counter("ForeignCallback")
                    .unwrap()
                    .value,
                41
            );
        },
    );
}

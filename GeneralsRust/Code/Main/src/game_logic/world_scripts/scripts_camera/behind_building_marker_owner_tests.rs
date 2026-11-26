use super::*;
use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
use gamelogic::helpers::TheGameLogic;
use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn authored_mode(mode: i32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::OptionsSetOcclusionMode);
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, mode))
        .unwrap();
    action
}

fn dispatch(world: &mut GameLogic, action: &ScriptAction) -> generals_sim::GameLogicResult<()> {
    world
        .with_owned_script_engine(|engine, world| {
            world.execute_checked_factory_actions_for_test(engine, action)
        })
        .expect("receiving world owns its script engine")
}

#[test]
fn behind_building_marker_script_and_save_use_the_receiving_world() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "behind_building_marker_script_and_save_use_the_receiving_world",
        || {
            let mut source = GameLogic::new();
            let mut restored = GameLogic::new();
            let peer = GameLogic::new();
            let builder = SnapshotBuilder::new();

            source.with_own_services(|| TheGameLogic::set_show_behind_building_markers(true));
            restored.with_own_services(|| TheGameLogic::set_show_behind_building_markers(true));
            peer.with_own_services(|| {
                TheGameLogic::set_show_behind_building_markers(true);
                dispatch(&mut source, &authored_mode(0)).unwrap();
                assert!(
                    TheGameLogic::get_show_behind_building_markers(),
                    "the script must leave the published peer's Core policy alone"
                );
            });
            assert!(
                source.with_own_services(TheGameLogic::get_show_behind_building_markers),
                "Main must not use its legacy Core adapter for the marker policy"
            );

            let saved = builder.create_world_snapshot(&source).unwrap();
            assert!(!saved.persist_v18.show_behind_building_markers);
            let encoded = bincode_legacy::serialize(&saved).unwrap();
            let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
            peer.with_own_services(|| {
                builder
                    .restore_from_snapshot(&decoded, &mut restored)
                    .unwrap();
            });
            assert!(
                restored.with_own_services(TheGameLogic::get_show_behind_building_markers),
                "restoring Main must leave the legacy Core adapter unchanged"
            );
            assert!(peer.with_own_services(TheGameLogic::get_show_behind_building_markers));

            let saved_flag = |world: &GameLogic| {
                crate::save_load::snapshot::persist_v18::capture_persist_v18(world)
                    .show_behind_building_markers
            };
            assert!(!saved_flag(&restored));
            assert!(saved_flag(&peer));
            let missing = ScriptAction::new(ScriptActionType::OptionsSetOcclusionMode);
            assert!(dispatch(&mut restored, &missing).is_err());
            assert!(!saved_flag(&restored));
            dispatch(&mut restored, &authored_mode(-2)).unwrap();
            assert!(saved_flag(&restored), "C++ enables every nonzero mode");
            assert!(!saved_flag(&source));
            source.reset();
            assert!(
                saved_flag(&source),
                "C++ reset restores the enabled default"
            );
        },
    );
}

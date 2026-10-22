//! Real primary script admission writes its receiving match before Core fallback.
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn action(mode: i32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::OptionsSetDrawiconUiMode);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, mode))
        .unwrap();
    action
}
#[test]
fn draw_icon_script_is_instance_owned_and_reset_does_not_touch_peer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "draw_icon_script_is_instance_owned_and_reset_does_not_touch_peer",
        || {
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
            assert!(a.get_draw_icon_ui() && b.get_draw_icon_ui());
            gamelogic::helpers::TheGameLogic::set_draw_icon_ui(true);
            a.execute_checked_factory_actions_for_test(&engine, &action(0))
                .unwrap();
            assert!(!a.get_draw_icon_ui());
            assert!(b.get_draw_icon_ui());
            assert!(gamelogic::helpers::TheGameLogic::get_draw_icon_ui());
            b.execute_checked_factory_actions_for_test(&engine, &action(0))
                .unwrap();
            a.execute_checked_factory_actions_for_test(&engine, &action(-2))
                .unwrap();
            assert!(
                a.get_draw_icon_ui(),
                "C++ treats every nonzero Int mode as true"
            );
            assert!(!b.get_draw_icon_ui());
            let invalid = ScriptAction::new(ScriptActionType::OptionsSetDrawiconUiMode);
            assert!(
                b.execute_checked_factory_actions_for_test(&engine, &invalid)
                    .is_err()
            );
            assert!(!b.get_draw_icon_ui());
            a.reset();
            assert!(a.get_draw_icon_ui());
            assert!(!b.get_draw_icon_ui());
            b.reset();
            assert!(b.get_draw_icon_ui());
        },
    );
}

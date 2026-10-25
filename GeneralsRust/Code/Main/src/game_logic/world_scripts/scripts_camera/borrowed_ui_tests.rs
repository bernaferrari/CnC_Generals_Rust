//! Real WND visibility is observable by the next authored camera action.
use super::super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use super::{action, engine, script, world};
use game_client::gui::callbacks::control_bar_callbacks::ControlBarCallbacks;
use game_client::gui::control_bar::ControlBar;
use game_client::gui::{GameWindow, window_manager::WindowManager};
use gamelogic::scripting::core::{
    Parameter, ParameterType, ScriptAction, ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptExecutionDriver;
use std::{cell::RefCell, rc::Rc};

struct ReceivingUi {
    bar: ControlBar,
    callbacks: ControlBarCallbacks,
    parent: Rc<RefCell<GameWindow>>,
    delivered: Vec<bool>,
}
impl crate::game_logic::ScriptUiEffects for ReceivingUi {
    fn weather_visible(&mut self, _visible: bool) {}
    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
        _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
        game_ending: bool,
    ) {
        if enabled {
            self.callbacks.hide_control_bar_owned(
                &mut self.bar,
                camera,
                Some(self.parent.clone()),
                true,
                true,
            );
        } else {
            self.callbacks.show_control_bar_receiving(
                &mut self.bar,
                camera,
                Some(self.parent.clone()),
                replay,
                false,
                _upgrade_source,
                game_ending,
            );
        }
        self.delivered.push(enabled);
    }
}
struct ObserveNextAction<'a> {
    actual: HostScriptExecutionDriver<'a>,
    parent: Rc<RefCell<GameWindow>>,
    observations: usize,
}
impl ScriptExecutionDriver for ObserveNextAction<'_> {
    fn camera(
        &mut self,
        request: gamelogic::scripting::engine::ScriptCameraRequest<'_>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        self.actual.camera(request)
    }
    fn camera_waypoint_position(
        &mut self,
        name: &str,
    ) -> Option<gamelogic::GameLogicResult<Option<gamelogic::common::Coord3D>>> {
        assert_eq!(
            self.parent.borrow().is_hidden(),
            name == "Hidden",
            "the next action must see actual WND visibility before a native drain"
        );
        self.observations += 1;
        Some(Ok(None))
    }
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        self.actual.after_action()
    }
}
fn probe(name: &str) -> Box<ScriptAction> {
    action(
        ScriptActionType::CameraMotionBlurJump,
        vec![Parameter::with_string(ParameterType::Waypoint, name.into())],
    )
}

#[test]
fn authored_letterbox_updates_real_parent_before_next_action_and_subroutine_return() {
    let (mut owner, _, _) = world(120.);
    let (peer, _, _) = world(320.);
    let mut windows = WindowManager::new();
    let parent = windows.create_window(None, 0, 380, 640, 100).unwrap();
    let foreign = windows.create_window(None, 0, 380, 640, 100).unwrap();
    let mut ui = ReceivingUi {
        bar: ControlBar::new(),
        callbacks: ControlBarCallbacks::new(),
        parent: parent.clone(),
        delivered: Vec::new(),
    };
    let mut list = ScriptList::new();
    let mut sub = script(
        "Sub",
        vec![
            action(ScriptActionType::CameraLetterboxEnd, vec![]),
            probe("Shown"),
        ],
        true,
    );
    sub.is_subroutine = true;
    list.append_script(sub);
    list.append_script(script(
        "Caller",
        vec![
            action(ScriptActionType::CameraLetterboxBegin, vec![]),
            probe("Hidden"),
            action(
                ScriptActionType::CallSubroutine,
                vec![Parameter::with_string(ParameterType::Script, "Sub".into())],
            ),
            probe("Shown"),
            action(ScriptActionType::CameraLetterboxEnd, vec![]),
            probe("Shown"),
        ],
        true,
    ));
    let interpreter = engine(peer.mission_scripts.clone(), list);
    let mut driver = ObserveNextAction {
        actual: HostScriptExecutionDriver::with_ui(&mut owner, &interpreter, Some(&mut ui)),
        parent: parent.clone(),
        observations: 0,
    };
    interpreter
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::new(),
            &mut driver,
        )
        .unwrap();
    assert_eq!(driver.observations, 4);
    drop(driver);
    assert_eq!(ui.delivered, [true, false, false]);
    assert!(
        owner.session_view.pending_letterbox_visibility.is_empty(),
        "native presentation must not replay already-delivered actions"
    );
    assert!(!owner.cinematic_letterbox);
    assert!(!parent.borrow().is_hidden());
    assert!(!foreign.borrow().is_hidden());
    assert!(peer.session_view.pending_letterbox_visibility.is_empty());
}

#[test]
fn fixed_step_receives_ui_only_for_admitted_script_execution() {
    let (mut owner, _, _) = world(120.);
    let mut windows = WindowManager::new();
    let parent = windows.create_window(None, 0, 380, 640, 100).unwrap();
    let mut ui = ReceivingUi {
        bar: ControlBar::new(),
        callbacks: ControlBarCallbacks::new(),
        parent: parent.clone(),
        delivered: Vec::new(),
    };
    let mut list = ScriptList::new();
    list.append_script(script(
        "Step",
        vec![action(ScriptActionType::CameraLetterboxBegin, vec![])],
        true,
    ));
    owner.install_owned_script_engine(engine(owner.mission_scripts.clone(), list));
    owner.scripts_loaded = true;
    owner.tick_logic_frame_with_script_ui(0., None, Some(1), Some(&mut ui), |_| {}, |_| {});
    assert!(ui.delivered.is_empty());
    assert!(!parent.borrow().is_hidden());
    owner.tick_logic_frame_with_script_ui(1. / 30., None, Some(1), Some(&mut ui), |_| {}, |_| {});
    assert_eq!(ui.delivered, [true]);
    assert!(parent.borrow().is_hidden());
    assert!(owner.session_view.pending_letterbox_visibility.is_empty());
    owner.tick_logic_frame_with_script_ui(0., None, Some(1), Some(&mut ui), |_| {}, |_| {});
    assert_eq!(ui.delivered, [true]);
}

#[test]
fn earlier_pending_visibility_precedes_borrowed_actions_and_is_not_replayed() {
    let (mut owner, _, _) = world(120.);
    let (peer, _, _) = world(320.);
    let mut windows = WindowManager::new();
    let parent = windows.create_window(None, 0, 380, 640, 100).unwrap();
    let mut ui = ReceivingUi {
        bar: ControlBar::new(),
        callbacks: ControlBarCallbacks::new(),
        parent: parent.clone(),
        delivered: Vec::new(),
    };
    owner
        .mission_scripts
        .apply_camera_request(gamelogic::scripting::engine::ScriptCameraRequest::LetterboxBegin)
        .unwrap();
    owner.apply_script_action_requests();
    assert_eq!(owner.session_view.pending_letterbox_visibility, [true]);
    let mut list = ScriptList::new();
    list.append_script(script(
        "ContinueAfterCallback",
        vec![
            probe("Hidden"),
            action(ScriptActionType::CameraLetterboxEnd, vec![]),
            probe("Shown"),
        ],
        true,
    ));
    let interpreter = engine(peer.mission_scripts.clone(), list);
    let mut driver = ObserveNextAction {
        actual: HostScriptExecutionDriver::with_ui(&mut owner, &interpreter, Some(&mut ui)),
        parent: parent.clone(),
        observations: 0,
    };
    interpreter
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::new(),
            &mut driver,
        )
        .unwrap();
    assert_eq!(driver.observations, 2);
    drop(driver);
    assert_eq!(ui.delivered, [true, false]);
    assert!(!parent.borrow().is_hidden());
    let (pending, delivered) = owner.session_view.take_letterbox_visibility_output();
    assert!(pending.is_empty());
    assert!(
        delivered,
        "presentation must suppress its UI transition fallback"
    );
    assert_eq!(
        owner.session_view.take_letterbox_visibility_output(),
        (vec![], false)
    );
}

#[test]
fn authored_letterbox_zoom_policy_changes_at_action_and_only_on_transition() {
    let (mut owner, _, _) = world(120.);
    let (peer, _, _) = world(320.);
    let mut windows = WindowManager::new();
    let parent = windows.create_window(None, 0, 380, 640, 100).unwrap();
    let mut ui = ReceivingUi {
        bar: ControlBar::new(),
        callbacks: ControlBarCallbacks::new(),
        parent,
        delivered: Vec::new(),
    };
    for (enabled, expected_limit) in [(true, false), (true, true), (false, true), (false, false)] {
        let mut list = ScriptList::new();
        list.append_script(script(
            "ZoomPolicy",
            vec![action(
                if enabled {
                    ScriptActionType::CameraLetterboxBegin
                } else {
                    ScriptActionType::CameraLetterboxEnd
                },
                vec![],
            )],
            true,
        ));
        let interpreter = engine(peer.mission_scripts.clone(), list);
        interpreter
            .update_with_driver(
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::with_ui(&mut owner, &interpreter, Some(&mut ui)),
            )
            .unwrap();
        assert_eq!(
            owner.session_view.camera.native_camera.zoom_limited(),
            expected_limit,
            "CPP W3DDisplay::enableLetterBox changes zoom policy only at a flag transition"
        );
        owner
            .session_view
            .camera
            .native_camera
            .set_zoom_limited(!expected_limit);
        let (_, delivered) = owner.session_view.take_letterbox_visibility_output();
        assert!(delivered);
        assert_eq!(
            owner.session_view.camera.native_camera.zoom_limited(),
            !expected_limit
        );
    }
}

#[test]
fn authored_letterbox_receives_current_upgrade_observation_at_each_action() {
    struct ObserveUpgrades(Vec<bool>);
    impl crate::game_logic::ScriptUiEffects for ObserveUpgrades {
        fn weather_visible(&mut self, _: bool) {}
        fn letterbox(
            &mut self,
            _: &mut crate::presentation_frame::camera::SessionCamera,
            _: bool,
            _: bool,
            source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<'_>,
            _game_ending: bool,
        ) {
            match source {
                game_engine::common::thing::receiving_module_content::UpgradeTooltipSource::Receiving(observe) => {
                    let observation = observe("", "Upgrade_LetterboxObservation");
                    assert!(observation.template.is_some());
                    self.0.push(observation.completed);
                }
                game_engine::common::thing::receiving_module_content::UpgradeTooltipSource::Standalone => panic!("authored native action must supply its receiving observation"),
            }
        }
    }
    let (mut owner, _, _) = world(120.);
    let (peer, _, _) = world(320.);
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut owner,
        "Upgrade_LetterboxObservation",
        "PLAYER",
        1,
        1,
    );
    let player = owner
        .get_players()
        .values()
        .find(|player| player.is_local)
        .unwrap()
        .id;
    let mut ui = ObserveUpgrades(Vec::new());
    for completed in [false, true] {
        if completed {
            owner
                .get_player_mut(player)
                .unwrap()
                .add_completed_upgrade("Upgrade_LetterboxObservation");
        }
        let mut list = ScriptList::new();
        list.append_script(script(
            "CurrentUpgradeLetterbox",
            vec![action(ScriptActionType::CameraLetterboxEnd, vec![])],
            true,
        ));
        let interpreter = engine(peer.mission_scripts.clone(), list);
        interpreter
            .update_with_driver(
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::with_ui(&mut owner, &interpreter, Some(&mut ui)),
            )
            .unwrap();
    }
    assert_eq!(ui.0, [false, true]);
    assert!(
        !peer
            .get_player(player)
            .unwrap()
            .has_unlocked_upgrade("Upgrade_LetterboxObservation")
    );
}

#[test]
fn borrowed_letterbox_uses_current_interpreter_ending_state_with_same_id_peer() {
    struct ObserveEnding(Vec<bool>);
    impl crate::game_logic::ScriptUiEffects for ObserveEnding {
        fn weather_visible(&mut self, _: bool) {}
        fn letterbox(
            &mut self,
            _: &mut crate::presentation_frame::camera::SessionCamera,
            _: bool,
            _: bool,
            _: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<'_>,
            game_ending: bool,
        ) {
            self.0.push(game_ending);
        }
    }
    let (mut owner, unit, _) = world(120.);
    let (mut peer, peer_unit, _) = world(320.);
    assert_eq!(unit, peer_unit);
    let mut ui = ObserveEnding(Vec::new());
    let mut peer_ui = ObserveEnding(Vec::new());
    owner
        .with_owned_script_engine(|interpreter, world| {
            assert!(world.owned_script_engine.is_none());
            interpreter.start_end_game_timer();
            let mut driver = HostScriptExecutionDriver::with_ui(world, interpreter, Some(&mut ui));
            interpreter
                .friend_execute_action_with_driver_checked(
                    &action(ScriptActionType::CameraLetterboxEnd, vec![]),
                    None,
                    gamelogic::scripting::executor::ScriptContext::new(),
                    &mut driver,
                )
                .unwrap();
        })
        .unwrap();
    peer.with_owned_script_engine(|interpreter, world| {
        assert!(world.owned_script_engine.is_none());
        let mut driver = HostScriptExecutionDriver::with_ui(world, interpreter, Some(&mut peer_ui));
        interpreter
            .friend_execute_action_with_driver_checked(
                &action(ScriptActionType::CameraLetterboxEnd, vec![]),
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut driver,
            )
            .unwrap();
    })
    .unwrap();
    assert_eq!(ui.0, [true]);
    assert_eq!(peer_ui.0, [false]);
    assert!(owner.owned_script_game_ending());
    assert!(!peer.owned_script_game_ending());
}

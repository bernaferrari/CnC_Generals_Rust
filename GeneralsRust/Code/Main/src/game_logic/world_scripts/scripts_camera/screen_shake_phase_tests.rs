//! Actual borrowed ScriptEngine actions and authored native onDie FX.
use super::*;
use crate::game_logic::{Player, Team};
use crate::presentation_frame::camera::shake_rules::CameraShakeRules;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use std::sync::Arc;

fn world() -> (GameLogic, [ObjectId; 2]) {
    let source = r#"Object ImmediateShakeVictim
KindOf = INERT IMMOBILE
Body = ActiveBody Body
MaxHealth = 100
InitialHealth = 100
End
Behavior = FXListDie Shake
DeathFX = FX_ImmediateShakeDeath
End
Behavior = KeepObjectDie Retain
End
End
Object AmericaCommandCenter
KindOf = STRUCTURE COMMANDCENTER
Body = ActiveBody Body
MaxHealth = 100
InitialHealth = 100
End
End
"#;
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "immediate-shake.ini".to_owned(),
                source.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world
        .admit_camera_application_shake(CameraShakeRules {
            shake_normal_intensity: 4.,
            shake_cine_extreme_intensity: 8.,
            max_shake_intensity: 10.,
            max_shake_range: 100.,
            ..Default::default()
        })
        .unwrap();
    world.set_current_frame(713);
    let ids = ["VictimA", "VictimB"].map(|name| {
        let id = world
            .create_object_for_player("ImmediateShakeVictim", 1, Vec3::ZERO)
            .unwrap();
        world.host_object_mut(id).unwrap().name = name.into();
        id
    });
    let mut shroud = world.world_services.shroud().lock().unwrap();
    shroud.init_shroud_grid(128., 128.);
    shroud.reveal_map_for_player_permanently(1).unwrap();
    for id in ids {
        shroud.set_host_object_shroud_status(1, id.0, gamelogic::common::ObjectShroudStatus::Clear);
    }
    drop(shroud);
    (world, ids)
}
fn install_death_fx() {
    use game_engine::common::ini::ini_fx_list::{CameraShakeType, FXList, FXNugget};
    let mut definition = FXList::new("FX_ImmediateShakeDeath".into());
    definition.add_nugget(FXNugget::ViewShake {
        shake_type: CameraShakeType::Normal,
    });
    game_client::fx_list::get_fx_list_store_mut().add_fx_list(
        "FX_ImmediateShakeDeath".into(),
        game_client::fx_list::FXList::from_definition(&definition),
    );
}
fn action(kind: ScriptActionType, parameter: Parameter) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action.add_parameter(parameter).unwrap();
    Box::new(action)
}
fn actions() -> Vec<Box<ScriptAction>> {
    vec![
        action(
            ScriptActionType::ScreenShake,
            Parameter::with_int(ParameterType::Int, 4),
        ),
        action(
            ScriptActionType::NamedKill,
            Parameter::with_string(ParameterType::Unit, "VictimA".into()),
        ),
        action(
            ScriptActionType::NamedKill,
            Parameter::with_string(ParameterType::Unit, "VictimB".into()),
        ),
    ]
}
fn chain(mut actions: Vec<Box<ScriptAction>>) -> Option<Box<ScriptAction>> {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(action);
    }
    next
}

#[test]
fn actual_same_walk_screen_shake_precedes_native_death_fx_and_never_replays_frozen_output() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_same_walk_screen_shake_precedes_native_death_fx_and_never_replays_frozen_output",
        || {
            install_death_fx();
            let (mut source, ids) = world();
            let (mut peer, peer_ids) = world();
            assert_eq!(ids, peer_ids);
            let peer_before = peer.capture_camera_continuation().unwrap();
            let mut engine = ScriptEngine::new().unwrap();
            // A mistaken retained-handler fallback would mutate the actual peer.
            engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(
                peer.mission_scripts.clone(),
            ))));
            let mut trace = Vec::new();
            for action in actions() {
                let frame = source.get_current_frame();
                engine.friend_execute_action_with_driver(
                    &action,
                    None,
                    gamelogic::scripting::executor::ScriptContext::at_frame(frame as u32),
                    &mut HostScriptExecutionDriver::with_interpreter(&mut source, &engine),
                );
                trace.push(source.session_view.impulse.intensity);
            }
            // W3DView3224-3280:8,8+4->fixed3,3+4=7. Deferring the first
            // authored action yields4,8,8+8->3, an observable different result.
            assert_eq!(trace, [8., 3., 7.]);
            assert!(ids
                .into_iter()
                .all(|id| !source.host_object(id).unwrap().is_alive()));
            assert!(peer_ids
                .into_iter()
                .all(|id| peer.host_object(id).unwrap().is_alive()));
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            let source_before = source.session_view.capture();
            let frozen =
                crate::presentation_frame::PresentationFrame::build_from_logic(&mut source, 0);
            assert!(
                frozen.screen_shakes.is_empty(),
                "current script execution produces no delayed shake intent"
            );
            assert_eq!(
                source
                    .acknowledge_frozen_camera_requests(&frozen)
                    .screen_shakes,
                0
            );
            assert_eq!(
                source
                    .acknowledge_frozen_camera_requests(&frozen)
                    .screen_shakes,
                0
            );
            assert_eq!(source.session_view.capture(), source_before);

            // The ordinary Main evaluator uses the same full authored chain.
            let (mut ordinary, _) = world();
            let mut script = Script::new();
            script.script_name = "ImmediateShakeChain".into();
            script.is_one_shot = true;
            let mut condition = OrCondition::new();
            condition.set_first_and_condition(Some(Box::new(Condition::new(
                ConditionType::ConditionTrue,
            ))));
            script.condition = Some(Box::new(condition));
            script.action = chain(actions());
            let mut list = ScriptList::new();
            list.append_script(Box::new(script));
            engine
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            ordinary.install_owned_script_engine(engine);
            ordinary.scripts_loaded = true;
            ordinary.evaluate_and_execute_scripts(0.);
            assert_eq!(ordinary.session_view.impulse.intensity, 7.);
            assert!(ordinary.pending_screen_shakes.is_empty());
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
        },
    );
}

#[test]
fn fallback_hooks_and_direct_driver_share_logic_frame_seed_without_double_admission() {
    let (mut source, _) = world();
    let (peer, _) = world();
    let peer_before = peer.capture_camera_continuation().unwrap();
    source
        .mission_scripts
        .push_screen_shake(ScreenShakeRequest { intensity: 4 });
    HostScriptExecutionDriver::new(&mut source)
        .after_action()
        .unwrap();
    let first = source.session_view.capture();
    assert_eq!(first.intensity, 8.);
    HostScriptExecutionDriver::new(&mut source)
        .after_action()
        .unwrap();
    assert_eq!(source.session_view.capture(), first);
    let mut direct = GameLogic::new();
    direct.set_current_frame(713);
    direct
        .admit_camera_application_shake(source.session_view.camera.shake_policy.current())
        .unwrap();
    HostScriptExecutionDriver::new(&mut direct)
        .camera(ScriptCameraRequest::ScreenShake { intensity: 4 })
        .unwrap()
        .unwrap();
    assert_eq!(
        direct.session_view.capture(),
        first,
        "same Logic frame and source operation, no presentation seed"
    );
    assert!(direct
        .mission_scripts
        .drain_screen_shake_requests()
        .is_empty());
    assert!(direct.pending_screen_shakes.is_empty());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

#[test]
fn authored_shake_then_home_observes_active_impulse_and_rule_changes_only_next_admission() {
    let (mut owner, _) = world();
    let (peer, _) = world();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.boundaries = vec![gamelogic::common::ICoord2D::new(64, 64)];
    map.heightmap = vec![0; 64 * 64];
    owner
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    // The receiving catalog authors COMMANDCENTER, so Home remains
    // unambiguous among this owner's other immobile zero-cost objects.
    owner
        .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(310., 0., 330.))
        .unwrap();
    assert_eq!(owner.local_player_id(), Some(1));
    assert_eq!(
        owner.command_center_position(Team::USA),
        Some(Vec3::new(310., 0., 330.))
    );
    owner
        .session_view
        .camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(640., 0., 640.), 0.)
        .unwrap();
    owner.session_view.camera.install_zoom(2., 3., 0., 0.);
    let zoom_frame = owner.session_view.camera.camera_zoom_frame;
    let hag_before = serde_json::to_vec(
        &owner
            .session_view
            .camera
            .native_camera
            .capture_continuation(),
    )
    .unwrap();
    let engine = ScriptEngine::new().unwrap();
    let context = || gamelogic::scripting::executor::ScriptContext::at_frame(713);
    engine.friend_execute_action_with_driver(
        &actions().remove(0),
        None,
        context(),
        &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
    );
    assert_eq!(owner.session_view.impulse.intensity, 8.);
    // Source View::shake only installs strength/direction; LookAt does not
    // perform the later view-update impulse step. Existing offset is retained.
    let active = owner.session_view.capture();
    let before_view = owner.session_view.camera.view_matrix;
    engine.friend_execute_action_with_driver(
        &ScriptAction::new(ScriptActionType::CameraMoveHome),
        None,
        context(),
        &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
    );
    assert_eq!(owner.session_view.target, Vec3::new(310., 0., 330.));
    assert_ne!(owner.session_view.camera.view_matrix, before_view);
    assert_eq!(owner.session_view.impulse.intensity, active.intensity);
    assert_eq!(owner.session_view.impulse.cosine, active.cosine);
    assert_eq!(owner.session_view.impulse.sine, active.sine);
    assert_eq!(owner.session_view.impulse.offset, active.offset);
    assert_eq!(owner.session_view.camera.camera_zoom_frame, zoom_frame);
    assert_eq!(
        serde_json::to_vec(
            &owner
                .session_view
                .camera
                .native_camera
                .capture_continuation()
        )
        .unwrap(),
        hag_before
    );
    let before_rules = owner.session_view.capture();
    let mut next = owner.session_view.camera.shake_policy.current();
    next.shake_cine_extreme_intensity = 2.;
    owner.admit_camera_map_shake(next).unwrap();
    assert_eq!(owner.session_view.capture(), before_rules);
    engine.friend_execute_action_with_driver(
        &actions().remove(0),
        None,
        context(),
        &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
    );
    assert_eq!(
        owner.session_view.impulse.intensity, 10.,
        "strict source > cap permits equality"
    );
    assert!(owner.peek_pending_screen_shakes().is_empty());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

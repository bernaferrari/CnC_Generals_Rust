//! Actual authored actions, eligibility, native frame, and original map Xfer.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use crate::game_logic::host_production_buildable_command_residual::{
    CANMAKE_NO_PREREQ, CANMAKE_OK,
};
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
const SET: &str = "OwnedOverrideProducerSet";
const PRODUCER: &str = "OwnedOverrideBarracks";
const UNIT: &str = "OwnedOverrideInfantry";
const BUILD: &str = "Command_OwnedOverrideBuild";
const GUARD: &str = "Command_OwnedOverrideGuard";
fn definitions() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        use game_engine::common::ini::{ini_command_button as buttons, ini_command_set as sets};
        buttons::initialize_control_bar();
        sets::initialize_command_set_manager();
        {
            let mut bar = buttons::get_control_bar_mut().unwrap();
            let build = bar.new_command_button(BUILD.into());
            build.command = "UNIT_BUILD".into();
            build.object = UNIT.into();
            bar.new_command_button(GUARD.into()).command = "GUARD".into();
        }
        let mut manager = sets::get_command_set_manager_mut().unwrap();
        let set = manager.new_command_set(SET.into());
        set.set_button_at_position(0, BUILD.into()).unwrap();
        set.set_button_at_position(1, GUARD.into()).unwrap();
    });
}
fn fixture() -> (GameLogic, ObjectId) {
    definitions();
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(
        Player::new(1, Team::USA, "OverrideOwner", true).with_script_identity("OverrideOwner"),
    );
    let mut producer = ThingTemplate::new(PRODUCER);
    producer
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    producer.command_set_name = SET.into();
    world.templates.insert(PRODUCER.into(), producer);
    let mut unit = ThingTemplate::new(UNIT);
    unit.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_cost(100, 0);
    world.templates.insert(UNIT.into(), unit);
    let id = world
        .create_object_for_player(PRODUCER, 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    world.host_object_mut(id).unwrap().selected = true;
    world.get_player_mut(1).unwrap().selected_objects = vec![id];
    (world, id)
}
fn action(
    world: &mut GameLogic,
    kind: ScriptActionType,
    first: &str,
    second: Option<&str>,
    value: Option<i32>,
) {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            first.into(),
        ))
        .unwrap();
    if let Some(second) = second {
        action
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                second.into(),
            ))
            .unwrap();
    }
    if let Some(value) = value {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    world
        .with_owned_script_engine(|engine, receiving| {
            let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
            engine.friend_execute_action_with_driver(
                &action,
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut driver,
            );
        })
        .unwrap();
}
fn frame(world: &mut GameLogic) -> crate::presentation_frame::PresentationFrame {
    crate::presentation_frame::PresentationFrame::build_from_logic(world, 1)
}
#[test]
fn same_template_overrides_follow_actions_eligibility_frame_reset_and_encoded_restore() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, id) = fixture();
    let (mut peer, peer_id) = fixture();
    assert_eq!(id, peer_id);
    assert_eq!(owner.can_make_unit(id, UNIT), CANMAKE_OK);
    assert_eq!(peer.can_make_unit(id, UNIT), CANMAKE_OK);
    let frozen = frame(&mut owner);
    assert_eq!(frozen.unit_command_buttons()[0].command_name, BUILD);
    action(
        &mut owner,
        ScriptActionType::TechtreeModifyBuildabilityObject,
        UNIT,
        None,
        Some(2),
    );
    assert_eq!(owner.can_make_unit(id, UNIT), CANMAKE_NO_PREREQ);
    assert_eq!(peer.can_make_unit(id, UNIT), CANMAKE_OK);
    assert_eq!(
        owner.templates.get(UNIT).unwrap().buildable_status,
        0,
        "script override does not rewrite immutable definition default"
    );
    assert!(
        frame(&mut owner).unit_command_buttons()[0]
            .command_name
            .is_empty()
    );
    assert_eq!(
        frame(&mut peer).unit_command_buttons()[0].command_name,
        BUILD
    );
    action(
        &mut owner,
        ScriptActionType::CommandbarAddButtonObjecttypeSlot,
        GUARD,
        Some(PRODUCER),
        Some(1),
    );
    assert_eq!(owner.find_control_bar_override(SET, 0), Some(Some(GUARD)));
    assert_eq!(
        frame(&mut owner).unit_command_buttons()[0].command_name,
        GUARD
    );
    action(
        &mut owner,
        ScriptActionType::CommandbarRemoveButtonObjecttype,
        GUARD,
        Some(PRODUCER),
        None,
    );
    assert_eq!(owner.find_control_bar_override(SET, 0), Some(None));
    assert_eq!(
        owner.find_control_bar_override(SET, 1),
        None,
        "first effective matching slot is removed"
    );
    assert_eq!(peer.find_control_bar_override(SET, 0), None);
    assert!(
        frame(&mut owner).unit_command_buttons()[0]
            .command_name
            .is_empty()
    );
    assert_eq!(
        frozen.unit_command_buttons()[0].command_name,
        BUILD,
        "old immutable frame retains its original map"
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    assert!(
        snapshot
            .persist_v18
            .buildable_overrides
            .contains(&(UNIT.into(), 2))
    );
    assert!(
        snapshot
            .persist_v18
            .control_bar_overrides
            .contains(&(format!("0{SET}"), String::new()))
    );
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _) = fixture();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.find_control_bar_override(SET, 0), Some(None));
    assert_eq!(restored.find_control_bar_override(SET, 1), None);
    assert_eq!(restored.can_make_unit(id, UNIT), CANMAKE_NO_PREREQ);
    peer.reset();
    assert_eq!(restored.find_buildable_status_override(UNIT), Some(2));
    assert!(
        frame(&mut restored).unit_command_buttons()[0]
            .command_name
            .is_empty()
    );
    action(
        &mut restored,
        ScriptActionType::TechtreeModifyBuildabilityObject,
        UNIT,
        None,
        Some(0),
    );
    assert_eq!(
        restored.can_make_unit(id, UNIT),
        CANMAKE_NO_PREREQ,
        "explicit null still denies production"
    );
    action(
        &mut restored,
        ScriptActionType::CommandbarAddButtonObjecttypeSlot,
        BUILD,
        Some(PRODUCER),
        Some(1),
    );
    assert_eq!(restored.can_make_unit(id, UNIT), CANMAKE_OK);
    assert_eq!(
        frame(&mut restored).unit_command_buttons()[0].command_name,
        BUILD
    );
    restored.reset();
    assert_eq!(restored.find_buildable_status_override(UNIT), None);
    assert_eq!(restored.find_control_bar_override(SET, 0), None);
    assert_eq!(owner.find_control_bar_override(SET, 0), Some(None));
}

//! CPP ScriptConditions2354, Player2880, ProductionPrerequisite123.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use game_engine::common::rts::ProductionPrerequisite;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

const PLAYER: &str = "OwnedPrerequisitePlayer";
const CANDIDATE: &str = "OwnedPrerequisiteTank";
const FACILITY: &str = "OwnedPrerequisiteFactory";
const LIST: &str = "OwnedPrerequisiteList";

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(Player::new(1, Team::USA, PLAYER, true));
    let mut template = ThingTemplate::new(CANDIDATE);
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    let mut prerequisite = ProductionPrerequisite::new();
    prerequisite.add_unit_prereq(FACILITY.into(), false);
    template.set_production_prerequisites(vec![prerequisite]);
    world.templates.insert(CANDIDATE.into(), template);
    let mut facility = ThingTemplate::new(FACILITY);
    facility.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert(FACILITY.into(), facility);
    world
}

fn condition(player: &str, object_type: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::SkirmishPlayerHasPrerequisiteToBuild);
    for (kind, value) in [
        (ParameterType::Side, player),
        (ParameterType::ObjectType, object_type),
    ] {
        condition
            .add_parameter(Parameter::with_string(kind, value.into()))
            .unwrap();
    }
    condition
}

fn increment() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "CanBuild".into(),
        ))
        .unwrap();
    action
}

fn script(condition: Condition, action: ScriptAction, name: &str) -> Script {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.script_name = name.into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    script
}

fn engine_with_scripts(scripts: Vec<Script>) -> ScriptEngine {
    let mut list = ScriptList::new();
    for script in scripts {
        list.append_script(Box::new(script));
    }
    let engine = ScriptEngine::new().unwrap();
    engine.set_counter("CanBuild", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn engine(player: &str, types: &str) -> ScriptEngine {
    engine_with_scripts(vec![script(
        condition(player, types),
        increment(),
        "ObservePrerequisite",
    )])
}

fn tick(engine: &ScriptEngine, world: &mut GameLogic) -> i32 {
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
    world.frame += 1;
    engine.get_counter("CanBuild").unwrap().value
}

fn set_list(engine: &ScriptEngine, names: &[&str]) {
    let mut list = gamelogic::object::object_types::ObjectTypes::new();
    for name in names {
        list.add_object_type(gamelogic::common::AsciiString::from(*name));
    }
    engine.set_object_types(LIST.into(), list);
}

#[test]
fn prerequisite_action_walk_calibration() {
    let mut world = world();
    let engine = engine_with_scripts(vec![script(
        Condition::new(ConditionType::ConditionTrue),
        increment(),
        "Calibration",
    )]);
    assert_eq!(tick(&engine, &mut world), 1);
}

#[test]
fn live_prerequisites_observe_own_facility_before_next_frame() {
    let mut world = world();
    let engine = engine(PLAYER, CANDIDATE);
    assert_eq!(tick(&engine, &mut world), 0);
    let facility = world
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(tick(&engine, &mut world), 1);
    world
        .host_object_mut(facility)
        .unwrap()
        .status
        .under_construction = true;
    assert_eq!(tick(&engine, &mut world), 1);
    // CPP counts dead-but-not-removed objects for prerequisites (ignoreDead=false).
    let object = world.host_object_mut(facility).unwrap();
    object.status.under_construction = false;
    object.status.effectively_dead = true;
    assert_eq!(tick(&engine, &mut world), 2);
    world.host_objects_mut().remove(&facility);
    assert_eq!(tick(&engine, &mut world), 2);
}

#[test]
fn registered_empty_list_and_mixed_candidates_use_current_engine() {
    let mut world = world();
    world
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    // A buildable template sharing the list name must not replace an empty list.
    world
        .templates
        .insert(LIST.into(), ThingTemplate::new(LIST));
    let engine = engine(PLAYER, LIST);
    set_list(&engine, &[]);
    assert_eq!(tick(&engine, &mut world), 0);
    set_list(&engine, &["MissingPrerequisiteTemplate", CANDIDATE]);
    assert_eq!(tick(&engine, &mut world), 1);
    set_list(&engine, &[]);
    assert_eq!(tick(&engine, &mut world), 1);
}

#[test]
fn same_ids_different_worlds_and_foreign_reset_keep_prerequisites_separate() {
    let mut a = world();
    let mut b = world();
    let a_facility = a.create_object_for_player(FACILITY, 1, Vec3::ZERO).unwrap();
    let b_facility = b.create_object_for_player(FACILITY, 1, Vec3::ZERO).unwrap();
    assert_eq!(a_facility, b_facility);
    b.host_object_mut(b_facility)
        .unwrap()
        .status
        .under_construction = true;
    let a_engine = engine(PLAYER, CANDIDATE);
    let b_engine = engine(PLAYER, CANDIDATE);
    assert_eq!(tick(&a_engine, &mut a), 1);
    assert_eq!(tick(&b_engine, &mut b), 0);
    b.reset();
    assert_eq!(tick(&a_engine, &mut a), 2);
    b.players.clear();
    assert_eq!(tick(&b_engine, &mut b), 0);
    drop(b);
    assert_eq!(tick(&a_engine, &mut a), 3);
}

#[test]
fn missing_player_is_false_without_script_failure() {
    let mut world = world();
    let engine = engine("MissingPrerequisitePlayer", CANDIDATE);
    assert_eq!(tick(&engine, &mut world), 0);
    assert_eq!(tick(&engine, &mut world), 0);
}

#[test]
fn object_list_actions_are_visible_to_the_next_script_in_the_same_walk() {
    let mut world = world();
    world
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    for (action_kind, expected) in [
        (ScriptActionType::ObjectlistAddobjecttype, 1),
        (ScriptActionType::ObjectlistRemoveobjecttype, 0),
    ] {
        let mut edit = ScriptAction::new(action_kind);
        edit.add_parameter(Parameter::with_string(
            ParameterType::ObjectTypeList,
            LIST.into(),
        ))
        .unwrap();
        edit.add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            CANDIDATE.into(),
        ))
        .unwrap();
        let engine = engine_with_scripts(vec![
            script(
                Condition::new(ConditionType::ConditionTrue),
                edit,
                "EditList",
            ),
            script(condition(PLAYER, LIST), increment(), "ObserveEditedList"),
        ]);
        set_list(&engine, if expected == 0 { &[CANDIDATE] } else { &[] });
        assert_eq!(tick(&engine, &mut world), expected);
    }
}

#[test]
fn object_list_xfer_tail_preserves_future_query() {
    let mut world = world();
    world
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    let original = engine(PLAYER, LIST);
    set_list(&original, &[CANDIDATE]);
    let tail = original.snapshot_xfer_tail();
    set_list(&original, &[]);
    assert_eq!(tick(&original, &mut world), 0);
    let restored = engine(PLAYER, LIST);
    restored.restore_xfer_tail(&tail);
    assert_eq!(tick(&restored, &mut world), 1);
    assert_eq!(tick(&original, &mut world), 0);
}

#[test]
fn player_permission_and_build_status_precede_prerequisites_and_caps() {
    use crate::game_logic::host_production_buildable_command_residual::*;
    let mut world = world();
    let engine = engine(PLAYER, CANDIDATE);
    world
        .templates
        .get_mut(CANDIDATE)
        .unwrap()
        .max_simultaneous_of_type = 1;
    world
        .create_object_for_player(CANDIDATE, 1, Vec3::ZERO)
        .unwrap();
    world.templates.get_mut(CANDIDATE).unwrap().buildable_status = BSTATUS_IGNORE_PREREQUISITES;
    assert_eq!(
        tick(&engine, &mut world),
        1,
        "ignore-prereqs returns before both prerequisites and cap"
    );
    world.players.get_mut(&1).unwrap().can_build_units = false;
    assert_eq!(
        tick(&engine, &mut world),
        1,
        "permission precedes ignore-prereqs"
    );
    world.players.get_mut(&1).unwrap().can_build_units = true;
    world.templates.get_mut(CANDIDATE).unwrap().buildable_status = BSTATUS_NO;
    assert_eq!(tick(&engine, &mut world), 1);
    world.templates.get_mut(CANDIDATE).unwrap().buildable_status = BSTATUS_ONLY_BY_AI;
    assert_eq!(tick(&engine, &mut world), 1);
    world.players.get_mut(&1).unwrap().is_human = false;
    world
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(tick(&engine, &mut world), 1, "ordinary AI still obeys cap");
    world
        .templates
        .get_mut(CANDIDATE)
        .unwrap()
        .max_simultaneous_of_type = 0;
    assert_eq!(tick(&engine, &mut world), 2);
}

// Tests below additionally exercise admitted metadata and the new capability.
#[test]
fn admitted_science_names_belong_to_the_driving_player() {
    use crate::game_logic::thing::PrerequisiteDefinitions;
    let mut a = world();
    let mut b = world();
    for world in [&mut a, &mut b] {
        let mut prerequisite = ProductionPrerequisite::new();
        prerequisite.add_science_prereq(701);
        let template = world.templates.get_mut(CANDIDATE).unwrap();
        template.set_production_prerequisites(vec![prerequisite]);
        template.prerequisite_definitions = PrerequisiteDefinitions::resolve(
            &template.production_prerequisites,
            |_| None,
            |_| Some("OwnedScience".into()),
        );
    }
    a.players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_OwnedScience".into());
    let a_engine = engine(PLAYER, CANDIDATE);
    let b_engine = engine(PLAYER, CANDIDATE);
    assert_eq!(tick(&a_engine, &mut a), 1);
    assert_eq!(tick(&b_engine, &mut b), 0);
    b.players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert("OwnedScience".into());
    a.players.get_mut(&1).unwrap().unlocked_sciences.clear();
    assert_eq!(tick(&a_engine, &mut a), 1);
    assert_eq!(tick(&b_engine, &mut b), 1);
}

#[test]
fn prerequisite_or_groups_and_first_equivalent_match_follow_cpp() {
    let mut world = world();
    let mut alias = ThingTemplate::new("OwnedPrerequisiteFactorySkin");
    alias.reskinned_from = Some(FACILITY.into());
    alias.add_kind_of(KindOf::Structure);
    world.templates.insert(alias.name.clone(), alias);
    world
        .create_object_for_player("OwnedPrerequisiteFactorySkin", 1, Vec3::ZERO)
        .unwrap();
    let mut prerequisite = ProductionPrerequisite::new();
    prerequisite.add_unit_prereq("MissingFacility".into(), false);
    prerequisite.add_unit_prereq(FACILITY.into(), true);
    world
        .templates
        .get_mut(CANDIDATE)
        .unwrap()
        .set_production_prerequisites(vec![prerequisite]);
    let engine = engine(PLAYER, CANDIDATE);
    assert_eq!(tick(&engine, &mut world), 1);
    let mut duplicates = ProductionPrerequisite::new();
    duplicates.add_unit_prereq(FACILITY.into(), false);
    duplicates.add_unit_prereq("OwnedPrerequisiteFactorySkin".into(), false);
    world
        .templates
        .get_mut(CANDIDATE)
        .unwrap()
        .set_production_prerequisites(vec![duplicates]);
    assert_eq!(
        tick(&engine, &mut world),
        1,
        "Team counts only first equivalent entry"
    );
}

#[test]
fn prepared_bindings_and_player_tokens_are_explicit() {
    use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
    let mut world = world();
    world
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    let candidates = vec![CANDIDATE.into()];
    let driver = HostScriptExecutionDriver::new(&mut world);
    assert_eq!(
        driver.skirmish_player_can_build_any(
            gamelogic::scripting::core::THIS_PLAYER,
            &candidates,
            Some(PLAYER)
        ),
        ScriptOwnerQuery::Present(true)
    );
    assert_eq!(
        driver.skirmish_player_can_build_any(
            gamelogic::scripting::core::LOCAL_PLAYER,
            &candidates,
            None
        ),
        ScriptOwnerQuery::Present(true)
    );
    assert_eq!(
        driver.skirmish_player_can_build_any("ownedprerequisiteplayer", &candidates, None),
        ScriptOwnerQuery::Missing
    );
    world.game_mode = GameMode::Skirmish;
    assert_eq!(
        HostScriptExecutionDriver::new(&mut world).skirmish_player_can_build_any(
            PLAYER,
            &candidates,
            None
        ),
        ScriptOwnerQuery::Unavailable
    );
}

#[test]
fn world_snapshot_preserves_future_prerequisite_queries() {
    use crate::game_logic::thing::PrerequisiteDefinitions;
    let mut source = world();
    let facility = source
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    let mut prerequisite = ProductionPrerequisite::new();
    prerequisite.add_unit_prereq(FACILITY.into(), false);
    prerequisite.add_science_prereq(702);
    let template = source.templates.get_mut(CANDIDATE).unwrap();
    template.set_production_prerequisites(vec![prerequisite]);
    template.prerequisite_definitions = PrerequisiteDefinitions::resolve(
        &template.production_prerequisites,
        |_| None,
        |_| Some("SnapshotScience".into()),
    );
    template.buildable_status =
        crate::game_logic::host_production_buildable_command_residual::BSTATUS_ONLY_BY_AI;
    let player = source.players.get_mut(&1).unwrap();
    player.is_human = false;
    player
        .unlocked_sciences
        .insert("SCIENCE_SnapshotScience".into());
    player.can_build_units = false;
    let source_engine = engine(PLAYER, CANDIDATE);
    assert_eq!(tick(&source_engine, &mut source), 0);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let mut restored = world();
    // Save records carry catalog identities; reload uses the admitted content.
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let restored_engine = engine(PLAYER, CANDIDATE);
    let mut foreign = world();
    foreign
        .create_object_for_player(FACILITY, 1, Vec3::ZERO)
        .unwrap();
    foreign.reset();
    foreign.players.clear();
    assert!(!restored.players[&1].is_human);
    assert!(!restored.players[&1].can_build_units);
    assert!(
        restored.players[&1]
            .unlocked_sciences
            .contains("SCIENCE_SnapshotScience")
    );
    for (engine, world) in [
        (&source_engine, &mut source),
        (&restored_engine, &mut restored),
    ] {
        assert_eq!(tick(engine, world), 0);
        world.players.get_mut(&1).unwrap().can_build_units = true;
        assert_eq!(tick(engine, world), 1);
        world
            .host_object_mut(facility)
            .unwrap()
            .status
            .under_construction = true;
        assert_eq!(tick(engine, world), 1);
        world
            .host_object_mut(facility)
            .unwrap()
            .status
            .under_construction = false;
        world.players.get_mut(&1).unwrap().unlocked_sciences.clear();
        assert_eq!(tick(engine, world), 1);
    }
}

#[test]
fn unresolved_science_and_reskin_variations_do_not_guess_other_worlds() {
    let mut world = world();
    let mut prerequisite = ProductionPrerequisite::new();
    prerequisite.add_science_prereq(703);
    world
        .templates
        .get_mut(CANDIDATE)
        .unwrap()
        .set_production_prerequisites(vec![prerequisite]);
    // Unrelated purchased sciences cannot satisfy an unbound definition.
    world
        .players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert("UnrelatedScience".into());
    assert_eq!(tick(&engine(PLAYER, CANDIDATE), &mut world), 0);
    let mut base = ThingTemplate::new("OwnedVariationBase");
    base.build_variations.push("OwnedVariationChild".into());
    let mut child = ThingTemplate::new("OwnedVariationChild");
    assert!(base.is_equivalent_to(&child));
    assert!(child.is_equivalent_to(&base));
    let mut sibling = ThingTemplate::new("OwnedVariationSibling");
    sibling.reskinned_from = Some(base.name.clone());
    child.reskinned_from = Some(base.name.clone());
    assert!(child.is_equivalent_to(&sibling));
    assert!(sibling.is_equivalent_to(&child));
    assert!(!base.is_equivalent_to(&ThingTemplate::new("UnrelatedVariation")));
}

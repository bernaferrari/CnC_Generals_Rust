//! TransportContain.cpp:42,84,389–393 and AIUpdate.cpp:4428–4438.
use super::*;
use crate::assets::{BehaviorModuleDefinition, ObjectDefinition};

fn authored_container(flag: Option<bool>) -> ThingTemplate {
    let mut template = ThingTemplate::new("MoodTransport");
    template.add_kind_of(KindOf::Vehicle).set_health(200.0);
    let mut definition = ObjectDefinition::new("MoodTransport".into());
    let mut attributes = std::collections::HashMap::new();
    attributes.insert("Slots".into(), "5".into());
    if let Some(flag) = flag {
        attributes.insert(
            "ResetMoodCheckTimeOnExit".into(),
            if flag { "Yes" } else { "No" }.into(),
        );
    }
    definition.behavior_modules.push(BehaviorModuleDefinition {
        class_name: "TransportContain".into(),
        module_tag: Some("ModuleTag_Contain".into()),
        attributes,
    });
    GameLogic::apply_authored_dock_and_contain_modules(&mut template, &definition, None);
    template
}

fn setup_rider(
    flag: Option<bool>,
    state: AIState,
    jitter: bool,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = 77;
    world
        .templates
        .insert("MoodTransport".into(), authored_container(flag));
    let mut passenger = ThingTemplate::new("MoodPassenger");
    passenger.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("MoodPassenger".into(), passenger);
    let transport = world
        .create_object("MoodTransport", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let rider = world
        .create_object("MoodPassenger", Team::USA, glam::Vec3::X)
        .unwrap();
    let unit = world.host_object_mut(rider).unwrap();
    unit.set_contained_by(Some(transport));
    unit.set_ai_state(state);
    unit.next_mood_check_time = 9999;
    unit.randomly_offset_mood_check = jitter;
    assert!(
        world
            .host_object_mut(transport)
            .unwrap()
            .add_occupant(rider)
    );
    (world, transport, rider)
}

#[test]
fn authored_transport_exit_mood_defaults_true_and_preserves_overrides() {
    assert!(
        authored_container(None)
            .contain_module
            .reset_mood_check_time_on_exit
    );
    assert!(
        authored_container(Some(true))
            .contain_module
            .reset_mood_check_time_on_exit
    );
    assert!(
        !authored_container(Some(false))
            .contain_module
            .reset_mood_check_time_on_exit
    );
}

#[test]
fn idle_transport_exit_wakes_and_requests_jitter_before_follow_path() {
    for authored in [None, Some(true)] {
        let (mut world, transport, rider) = setup_rider(authored, AIState::Idle, false);
        world.walk_unit_via_open_contain_exit(rider, transport);
        let unit = world.host_object(rider).unwrap();
        assert_eq!(unit.ai_state, AIState::Moving);
        assert_eq!(unit.next_mood_check_time, 77);
        assert!(unit.randomly_offset_mood_check);
        assert!(unit.contained_by.is_none());
        assert!(
            !world
                .host_object(transport)
                .unwrap()
                .contained_units()
                .contains(&rider)
        );
    }
}

#[test]
fn authored_false_transport_exit_preserves_both_mood_fields() {
    for jitter in [false, true] {
        let (mut world, transport, rider) = setup_rider(Some(false), AIState::Idle, jitter);
        world.walk_unit_via_open_contain_exit(rider, transport);
        let unit = world.host_object(rider).unwrap();
        assert_eq!(unit.ai_state, AIState::Moving);
        assert_eq!(unit.next_mood_check_time, 9999);
        assert_eq!(unit.randomly_offset_mood_check, jitter);
    }
}

#[test]
fn active_transport_exit_preserves_both_mood_fields() {
    for state in [AIState::Moving, AIState::Attacking] {
        for jitter in [false, true] {
            let (mut world, transport, rider) = setup_rider(Some(true), state.clone(), jitter);
            world.walk_unit_via_open_contain_exit(rider, transport);
            let unit = world.host_object(rider).unwrap();
            assert_eq!(unit.next_mood_check_time, 9999);
            assert_eq!(unit.randomly_offset_mood_check, jitter);
        }
    }
}

#[test]
fn garrison_exit_does_not_inherit_transport_mood_wake() {
    let (mut world, container, rider) = setup_rider(Some(true), AIState::Idle, false);
    world
        .host_object_mut(container)
        .unwrap()
        .thing
        .template
        .contain_module
        .kind = ContainModuleKind::Garrison;
    world.walk_unit_via_open_contain_exit(rider, container);
    let unit = world.host_object(rider).unwrap();
    assert_eq!(unit.next_mood_check_time, 9999);
    assert!(!unit.randomly_offset_mood_check);
}

#[test]
fn exit_mood_metadata_round_trip_and_missing_field_keep_cpp_default() {
    let authored = authored_container(Some(false)).contain_module;
    let restored: ContainModuleMetadata =
        serde_json::from_slice(&serde_json::to_vec(&authored).unwrap()).unwrap();
    assert_eq!(restored, authored);
    assert!(!restored.reset_mood_check_time_on_exit);

    let mut missing = serde_json::to_value(&ContainModuleMetadata::default()).unwrap();
    missing
        .as_object_mut()
        .unwrap()
        .remove("reset_mood_check_time_on_exit");
    let restored: ContainModuleMetadata = serde_json::from_value(missing).unwrap();
    assert!(restored.reset_mood_check_time_on_exit);
}

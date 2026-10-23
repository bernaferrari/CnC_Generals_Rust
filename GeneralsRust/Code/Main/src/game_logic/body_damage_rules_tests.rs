//! Real receiving parser/factory/Body routes with two same-ID worlds.
use super::*;
use crate::game_logic::host_enum_table_residual::HostBodyDamageType as State;
use crate::game_logic::{GameLogic, Object, ObjectId, ObjectType, Player, Team};

fn receiving(damaged: f32, really: f32) -> GameLogic {
    let mut world = GameLogic::new();
    let rules = BodyDamageRules::default().overlay_ini_text(&format!(
        "GameData\nUnitDamagedThreshold = {damaged}\nUnitReallyDamagedThreshold = {really}\nEnd\n"
    )).unwrap();
    world.admit_body_damage_baseline(BodyDamagePolicy::Receiving(rules));
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "receiving-threshold-body.ini".into(),
        "Object ReceivingThresholdBody\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 60\nEnd\nEnd\n".into()
    )]).unwrap()).unwrap();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world
}
fn born(world: &mut GameLogic) -> ObjectId {
    world
        .create_object_for_player("ReceivingThresholdBody", 1, glam::Vec3::ZERO)
        .unwrap()
}
fn damage(world: &mut GameLogic, id: ObjectId, amount: f32) {
    world
        .apply_owned_damage_with_effects(
            id,
            amount,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
            &mut Default::default(),
        )
        .unwrap();
}
#[test]
fn same_id_factory_initial_damage_heal_and_direct_maxhealth_share_receiving_policy() {
    let mut owner = receiving(0.8, 0.4);
    let mut peer = receiving(0.5, 0.2);
    let id = born(&mut owner);
    let peer_id = born(&mut peer);
    assert_eq!(id, peer_id);
    assert_eq!(
        owner.host_object(id).unwrap().body_damage_state,
        State::Damaged
    );
    assert_eq!(
        peer.host_object(id).unwrap().body_damage_state,
        State::Pristine
    );
    damage(&mut owner, id, 25.0);
    damage(&mut peer, peer_id, 25.0);
    assert_eq!(
        owner.host_object(id).unwrap().body_damage_state,
        State::ReallyDamaged
    );
    assert_eq!(
        peer.host_object(id).unwrap().body_damage_state,
        State::Damaged
    );
    owner
        .apply_owned_healing_with_effects(id, 20.0, None, &mut Default::default())
        .unwrap();
    assert_eq!(
        owner.host_object(id).unwrap().body_damage_state,
        State::Damaged
    );
    assert_eq!(peer.host_object(id).unwrap().health.current, 35.0);
    // Direct health and setMaxHealth must not silently return to retail0.7/0.35.
    let actor = owner.objects.get_mut(&id).unwrap();
    actor.set_body_max_health_with_mode(
        200.0,
        crate::game_logic::object::MaxHealthChange::PreserveRatio,
    );
    assert_eq!(actor.health.current, 110.0);
    assert_eq!(actor.body_damage_state, State::Damaged);
    actor.internal_change_health(-40.0);
    assert_eq!(actor.body_damage_state, State::ReallyDamaged);
    actor.apply_overlord_addon_set_damage_state(State::Damaged);
    assert_eq!(
        actor.health.current, 159.0,
        "setDamageState uses receiving0.8 not retail0.7"
    );
    assert_eq!(actor.body_damage_state, State::Damaged);
    actor.internal_change_health(-159.0);
    assert_eq!(actor.body_damage_state, State::Rubble);
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 35.0);
}
#[test]
fn standalone_provenance_is_explicit_and_real_external_admission_rebinds_before_callbacks() {
    let mut world = receiving(0.5, 0.2);
    let mut actor = Object::new_simple(ObjectId(77), ObjectType::Vehicle, "ToolBody".into());
    assert_eq!(
        actor.body_damage_policy(),
        BodyDamagePolicy::StandaloneRetail
    );
    actor.internal_change_health(-40.0);
    assert_eq!(actor.body_damage_state, State::Damaged);
    actor.team = Team::USA;
    actor.owner_player_id = Some(1);
    world.try_add_object(actor).unwrap();
    let admitted = world.host_object(ObjectId(77)).unwrap();
    assert!(matches!(
        admitted.body_damage_policy(),
        BodyDamagePolicy::Receiving(_)
    ));
    assert_eq!(admitted.health.current, 60.0);
    assert_eq!(admitted.body_damage_state, State::Pristine);
}
#[test]
fn parser_order_failure_and_reset_are_receiving_and_do_not_rewrite_live_actor_definitions() {
    let mut owner = receiving(0.8, 0.4);
    let mut peer = receiving(0.5, 0.2);
    owner
        .admit_body_damage_companion_sources(&[
            "GameData\nUnitDamagedThreshold = 0.9\nEnd".into(),
            "GameData\nUnitReallyDamagedThreshold = 0.7\nEnd".into(),
        ])
        .unwrap();
    let effective = owner.body_damage_policy();
    assert!(
        owner
            .admit_body_damage_companion_sources(&[
                "GameData\nUnitDamagedThreshold = broken\nEnd".into()
            ])
            .is_err()
    );
    assert_eq!(owner.body_damage_policy(), effective);
    let a = born(&mut owner);
    let b = born(&mut peer);
    assert_eq!(a, b);
    assert_eq!(
        owner.host_object(a).unwrap().body_damage_state,
        State::ReallyDamaged
    );
    assert_eq!(
        peer.host_object(b).unwrap().body_damage_state,
        State::Pristine
    );
    assert!(
        owner.admit_body_damage_companion_sources(&[]).is_err(),
        "no live content replacement with stale actor policies"
    );
    assert_eq!(owner.body_damage_policy(), effective);
    owner.reset();
    assert_eq!(owner.body_damage_policy(), owner.body_damage_baseline());
    assert_eq!(peer.host_object(b).unwrap().health.current, 60.0);
}
#[test]
fn exact_classifier_trace_handles_strict_custom_edges_zero_and_reversed_authored_order() {
    let rules = BodyDamageRules::default()
        .overlay_ini_text(
            "GameData\nUnitDamagedThreshold = 0.8\nUnitReallyDamagedThreshold = 0.4\nEnd",
        )
        .unwrap();
    let policy = BodyDamagePolicy::Receiving(rules);
    for (health, max, expected) in [
        (81.0, 100.0, State::Pristine),
        (80.0, 100.0, State::Damaged),
        (41.0, 100.0, State::Damaged),
        (40.0, 100.0, State::ReallyDamaged),
        (0.01, 100.0, State::ReallyDamaged),
        (0.0, 100.0, State::Rubble),
        (0.0, 0.0, State::Rubble),
        (1.0, 0.0, State::Pristine),
    ] {
        assert_eq!(policy.classify(health, max), expected);
    }
    let reversed = BodyDamagePolicy::Receiving(BodyDamageRules::from_thresholds(0.2, 0.8).unwrap());
    assert_eq!(
        reversed.classify(30.0, 100.0),
        State::Pristine,
        "original branch order, no sorting/clamping"
    );
}

#[test]
fn game_data_default_is_original_point_five_point_one_then_retail_override_is_explicit() {
    assert_eq!(BodyDamageRules::default().thresholds(), (0.5, 0.1));
    let retail = BodyDamageRules::default()
        .overlay_ini_text(
            "GameData\nUnitDamagedThreshold = 0.7\nUnitReallyDamagedThreshold = 0.35\nEnd",
        )
        .unwrap();
    assert_eq!(retail.thresholds(), (0.7, 0.35));
    assert_eq!(
        BodyDamagePolicy::Receiving(BodyDamageRules::default()).classify(20.0, 100.0),
        State::Damaged
    );
    assert_eq!(
        BodyDamagePolicy::Receiving(retail).classify(20.0, 100.0),
        State::ReallyDamaged
    );
}

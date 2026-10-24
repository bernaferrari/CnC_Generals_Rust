//! CPP PhysicsUpdate receiving rules, actor definition identity and saved future.
use super::*;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
use glam::Vec3;

fn rules_world(gravity: f32, ground: f32, structure: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_physics_rules_base(PhysicsContentRules {
            gravity,
            ground_stiffness: ground,
            structure_stiffness: structure,
        })
        .unwrap();
    let mut template = ThingTemplate::new("OwnedPhysicsRuleVehicle");
    template.add_kind_of(KindOf::Vehicle);
    template.set_health(1000.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object(
            "OwnedPhysicsRuleVehicle",
            Team::USA,
            Vec3::new(0.0, 10.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        &world.objects[&id].physics_rules,
        &world.physics_definitions.effective
    );
    let object = world.objects.get_mut(&id).unwrap();
    object.shock_stun_frames = 20;
    object.shock_allow_bounce = true;
    object.movement.velocity = Vec3::new(0.0, -1.0, 0.0);
    object.was_airborne_last_frame = true;
    (world, id)
}

#[test]
fn same_id_physics_rules_keep_authored_values_and_independent_motion() {
    let (mut first, id) = rules_world(-1.0, 0.5, 0.5);
    let (mut second, other) = rules_world(-0.25, 0.25, 0.9);
    assert_eq!(id, other);
    assert_eq!(
        first.objects[&id].physics_gravity(),
        -1.0,
        "valid authored -1 is not an unparsed/default guess"
    );
    assert_eq!(first.objects[&id].physics_ground_stiffness(), 0.5);
    assert_eq!(first.objects[&id].physics_structure_stiffness(), 0.5);
    assert!((first.objects[&id].min_fall_speed_for_damage - 80.0f32.sqrt()).abs() < 0.0001);
    let first_before = first.objects[&id].get_position();
    second.frame = 900;
    second.tick_shock_stun_all();
    assert_eq!(first.objects[&id].get_position(), first_before);
    first.frame = 500;
    first.tick_shock_stun_all();
    assert!((first.objects[&id].movement.velocity.y + 2.0).abs() < 0.0001);
    assert!((second.objects[&id].movement.velocity.y + 1.25).abs() < 0.0001);
    assert_ne!(
        first.objects[&id].get_position(),
        second.objects[&id].get_position()
    );
    assert_ne!(
        first.objects[&id].height_treats_as_airborne(3.0),
        second.objects[&id].height_treats_as_airborne(3.0)
    );
}

#[test]
fn receiving_rules_preserve_bounce_and_normal_saved_continuation() {
    let (mut source, id) = rules_world(-0.1, 0.5, 0.5);
    let (mut peer, _) = rules_world(-0.1, 0.2, 0.8);
    for world in [&mut source, &mut peer] {
        let (object, fixture_partition_object) = world.host_object_and_partition_mut(id).unwrap();
        object.set_position_with_partition_owner(
            Vec3::new(0.0, 1.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_object,
        );
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        object.shock_stun_frames = 20;
    }
    source.frame = 31;
    peer.frame = 31;
    source.tick_shock_stun_all();
    peer.tick_shock_stun_all();
    assert_ne!(
        source.objects[&id].movement.velocity.y, peer.objects[&id].movement.velocity.y,
        "real grounded bounce consumes the receiving authored stiffness"
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, restored_id) = rules_world(-0.1, 0.5, 0.5);
    assert_eq!(restored_id, id);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        &restored.objects[&id].physics_rules,
        &restored.physics_definitions.effective
    );
    for frame in 32..36 {
        source.frame = frame;
        restored.frame = frame;
        peer.frame = frame + 1000;
        peer.tick_shock_stun_all();
        source.tick_shock_stun_all();
        restored.tick_shock_stun_all();
        assert_eq!(
            source.objects[&id].get_position(),
            restored.objects[&id].get_position()
        );
        assert_eq!(
            source.objects[&id].movement.velocity,
            restored.objects[&id].movement.velocity
        );
        assert_eq!(
            source.objects[&id].health.current,
            restored.objects[&id].health.current
        );
        assert_eq!(
            source.objects[&id].shock_stun_frames,
            restored.objects[&id].shock_stun_frames
        );
    }
}

#[test]
fn physics_rule_admission_rejects_invalid_or_live_replacement_without_mutation() {
    let mut empty = GameLogic::new();
    let previous = empty.physics_definitions.effective;
    assert!(
        empty
            .admit_physics_rules_base(PhysicsContentRules {
                gravity: f32::NAN,
                ground_stiffness: 0.5,
                structure_stiffness: 0.5
            })
            .is_err()
    );
    assert_eq!(&previous, &empty.physics_definitions.effective);
    let (mut live, id) = rules_world(-1.0, 0.5, 0.5);
    let previous = live.objects[&id].physics_rules;
    assert!(
        live.admit_physics_rules_base(PhysicsContentRules {
            gravity: -0.1,
            ground_stiffness: 0.2,
            structure_stiffness: 0.3
        })
        .is_err()
    );
    assert_eq!(&previous, &live.objects[&id].physics_rules);
    assert_eq!(&previous, &live.physics_definitions.effective);
}

#[test]
fn owned_gamedata_parser_preserves_acceleration_units_and_overlay_values() {
    let rules = PhysicsContentRules::from_global_data(
        &game_engine::common::global_data::GlobalData::default(),
    );
    assert_eq!(rules.gravity, -1.0);
    let rules = rules
        .overlay_ini_text("GameData\nGravity=-900\nGroundStiffness=.5\nStructureStiffness=.5\nEnd")
        .unwrap();
    // CPP GameCommon.h64 multiplies by the squared f32 frame duration.
    let parsed_gravity = -900.0f32 * ((1.0f32 / 30.0) * (1.0f32 / 30.0));
    assert_eq!(rules.gravity, parsed_gravity);
    assert_eq!(rules.ground_stiffness, 0.5);
    assert_eq!(rules.structure_stiffness, 0.5);
    let map = rules
        .overlay_ini_text("GameData\nGravity=-225\nGroundStiffness=.2\nEnd")
        .unwrap();
    assert_eq!(map.gravity, -225.0f32 * ((1.0f32 / 30.0) * (1.0f32 / 30.0)));
    assert_eq!(map.ground_stiffness, 0.2);
    assert_eq!(map.structure_stiffness, 0.5);
    assert_eq!(rules.gravity, parsed_gravity);
}

#[test]
fn authored_stiffness_stays_raw_while_real_bounce_applies_cpp_limits() {
    for (authored, consumed) in [(0.0, 0.01), (1.0, 0.99), (-3.0, 0.01), (3.0, 0.99)] {
        let (mut world, id) = rules_world(-1.0, authored, authored);
        let object = world.objects.get_mut(&id).unwrap();
        assert_eq!(object.physics_rules.ground_stiffness, authored);
        assert_eq!(object.physics_rules.structure_stiffness, authored);
        object.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
        let force = object.compute_ground_bounce_force(1.0, -3.0, 0.0).unwrap();
        assert!((force.y - 4.0 * object.physics_mass * consumed).abs() < 0.0001);
        assert_eq!(object.physics_rules.ground_stiffness, authored);
        assert_eq!(object.physics_structure_stiffness(), consumed);
    }
}

#[test]
fn authored_fall_height_sign_and_zero_follow_cpp_fabs_expression() {
    for height in [-40.0, 0.0, 40.0] {
        let mut world = GameLogic::new();
        let rules = PhysicsContentRules {
            gravity: -1.0,
            ground_stiffness: 0.5,
            structure_stiffness: 0.5,
        };
        world.admit_physics_rules_base(rules).unwrap();
        let mut template = ThingTemplate::new("SignedFallHeightActor");
        template.add_kind_of(KindOf::Vehicle);
        template.min_fall_height_for_damage = Some(height);
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object("SignedFallHeightActor", Team::USA, Vec3::ZERO)
            .unwrap();
        assert_eq!(
            world.objects[&id].min_fall_speed_for_damage,
            (2.0 * rules.gravity * height).abs().sqrt()
        );
    }
}

#[test]
fn cloned_and_replaced_actor_admission_rebinds_rules_without_rewriting_resolved_scalar() {
    let (source, id) = rules_world(-1.0, 0.5, 0.5);
    let (mut receiver, receiver_id) = rules_world(-0.25, 0.2, 0.9);
    assert_eq!(id, receiver_id);
    let mut detached = source.objects[&id].clone();
    assert_eq!(
        &detached.physics_rules,
        &source.physics_definitions.effective
    );
    detached.min_fall_speed_for_damage = 17.0;
    detached.movement.velocity = Vec3::new(3.25, -7.5, 1.0);
    let previous_generation = receiver.objects[&id].visual_object_generation;
    let previous_velocity = receiver.objects[&id].movement.velocity;
    assert!(receiver.try_add_object(detached.clone()).is_err());
    assert_eq!(
        receiver.objects[&id].visual_object_generation,
        previous_generation
    );
    assert_eq!(receiver.objects[&id].movement.velocity, previous_velocity);
    receiver.destroy_object(id);
    receiver.process_destroy_list();
    receiver.add_object(detached);
    assert_eq!(
        &receiver.objects[&id].physics_rules,
        &receiver.physics_definitions.effective
    );
    assert_eq!(receiver.objects[&id].physics_gravity(), -0.25);
    assert_eq!(
        receiver.objects[&id].min_fall_speed_for_damage, 17.0,
        "admission cannot silently rewrite transferred module data"
    );
    assert_eq!(source.objects[&id].physics_gravity(), -1.0);
    assert_eq!(
        &source.objects[&id].physics_rules,
        &source.physics_definitions.effective
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&receiver).unwrap();
    let (mut restored, _) = rules_world(-0.25, 0.2, 0.9);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        &restored.objects[&id].physics_rules,
        &restored.physics_definitions.effective
    );
    assert_eq!(
        restored.objects[&id].movement.velocity,
        Vec3::new(3.25, -7.5, 1.0),
        "normal save load retains actual Physics continuation while binding receiving rules"
    );
    assert_eq!(
        restored.objects[&id].min_fall_speed_for_damage,
        20.0f32.sqrt(),
        "CPP module-data thresholds reconstruct from receiving definitions; they are not runtime Xfer fields"
    );
    assert_eq!(restored.objects[&id].physics_gravity(), -0.25);
}

//! Creation, retirement, pose, and receiving-world ownership.

use super::*;

#[test]
fn position_and_orientation_publish_after_each_write() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OrientPose");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object OrientU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "OrientU", 100.0);
    let id = logic
        .create_object("OrientU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    let initial = publish(&mut logic);
    let pos = {
        let obj = logic.host_objects().lookup(&id).unwrap();
        let p = obj.get_position();
        [p.x, p.y, p.z]
    };
    {
        let (o, fixture_partition_o) = logic.host_object_and_partition_mut(id).unwrap();
        o.set_position_with_partition_owner(
            glam::Vec3::from_array(pos),
            0,
            &mut Default::default(),
            fixture_partition_o,
        );
        o.set_orientation_with_partition_owner(0.75, fixture_partition_o);
    }
    let turned = publish(&mut logic);
    assert!((actor(&turned, id).orientation - 0.75).abs() < 0.01);
    // Second pose write with new facing (simulates host turn + position step).
    {
        let (o, fixture_partition_o) = logic.host_object_and_partition_mut(id).unwrap();
        o.set_position_with_partition_owner(
            glam::Vec3::from_array([6.0, 0.0, 5.0]),
            0,
            &mut Default::default(),
            fixture_partition_o,
        );
        o.set_orientation_with_partition_owner(-0.25, fixture_partition_o);
    }
    let moved = publish(&mut logic);
    assert_eq!(actor(&moved, id).position, Vec3::new(6.0, 0.0, 5.0));
    assert!((actor(&moved, id).orientation + 0.25).abs() < 0.01);
    assert_eq!(actor(&initial, id).position, Vec3::new(5.0, 0.0, 5.0));
    assert!((actor(&turned, id).orientation - 0.75).abs() < 0.01);
}

#[test]
fn queued_destruction_retires_at_the_fixed_update() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DesLog");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("DesU")
            .kind_of("SELECTABLE")
            .health(50.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let id = logic
        .create_object("DesU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    let before = publish(&mut logic);
    assert_eq!(actor(&before, id).id, id);
    logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::MarkForDestruction {
        id: id,
        team: None,
    });
    assert!(
        logic.host_objects().lookup(&id).is_some(),
        "marking only queues the destroy"
    );
    logic.update_with_dt(1.0 / 30.0);
    // C++ GameLogic::update processes the destroy list at the end of the
    // frame; the frame boundary consumes the transient destroy record, so the
    // durable evidence is the object's absence from the receiving owner.
    assert!(logic.host_objects().lookup(&id).is_none());
    assert!(publish(&mut logic).objects.iter().all(|row| row.id != id));
    assert_eq!(actor(&before, id).id, id);
}

#[test]
fn creation_and_physical_retirement_publish_the_original_id() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpawnDestroy");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object SpawnUnit\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "SpawnUnit", 80.0);
    let id = logic
        .create_object("SpawnUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
        .expect("spawn");
    let object = logic
        .host_object(id)
        .expect("successful factory creation is immediately observable");
    assert_eq!(object.template_name, "SpawnUnit");
    assert_eq!(object.team, Team::USA);
    assert_eq!(object.get_position(), glam::Vec3::new(3.0, 0.0, 0.0));
    let initial = publish(&mut logic);
    assert_eq!(actor(&initial, id).template_name, "SpawnUnit");
    assert_eq!(actor(&initial, id).position, Vec3::new(3.0, 0.0, 0.0));
    logic.destroy_object(id).unwrap();
    for _ in 0..3 {
        logic.update();
    }
    assert!(logic.host_object(id).is_none());
    assert!(publish(&mut logic).objects.iter().all(|row| row.id != id));
    assert_eq!(actor(&initial, id).id, id);
}

#[test]
fn pose_and_health_queries_read_the_same_live_object() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HostSolePose");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object HostSoleU\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "HostSoleU", 80.0);
    let id = logic
        .create_object("HostSoleU", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("id");
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(id).unwrap();
        o.health.current = 33.0;
        o.health.maximum = 80.0;
        o.set_position_with_partition_owner(
            glam::Vec3::new(7.0, 0.0, 9.0),
            0,
            &mut Default::default(),
            fixture_partition,
        );
        o.set_orientation_with_partition_owner(1.25, fixture_partition);
    }
    let o = logic.host_objects().lookup(&id).unwrap();
    assert_eq!(o.get_position(), glam::Vec3::new(7.0, 0.0, 9.0));
    assert!((o.get_orientation() - 1.25).abs() < 1e-5);
    assert!((o.get_health_percentage() - (33.0 / 80.0)).abs() < 1e-5);
    assert!(o.is_alive());
    let frame = publish(&mut logic);
    let row = actor(&frame, id);
    assert_eq!(row.position, Vec3::new(7.0, 0.0, 9.0));
    assert_eq!((row.health_current, row.health_max), (33.0, 80.0));
}

#[test]
fn reset_clears_the_world_and_its_current_publication() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ResetBridge");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object RstU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "RstU", 50.0);
    let id = logic
        .create_object("RstU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    assert!(!logic.host_objects().registered_is_empty());
    let prior = publish(&mut logic);
    logic.reset();
    assert!(logic.host_objects().registered_is_empty());
    assert_eq!(logic.get_frame(), 0);
    assert!(publish(&mut logic).objects.is_empty());
    assert_eq!(actor(&prior, id).id, id);
}

#[test]
fn body_writes_appear_only_in_later_frames() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("GwStore");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object GwStoreU\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    insert_fixture_template(&mut logic, "GwStoreU", 100.0);
    let oid = logic
        .create_object("GwStoreU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");

    let before = publish(&mut logic);
    assert_eq!(actor(&before, oid).health_current, 100.0);
    {
        logic.with_host_object_mut(oid, |o| {
            o.health.current = (o.health.current - 25.0).max(0.0);
        });
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        o.move_to(
            glam::Vec3::new(40.0, 0.0, 0.0),
            0,
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    };
    assert_eq!(logic.host_authoritative_health(oid), Some(75.0));
    let current = publish(&mut logic);
    assert_eq!(actor(&current, oid).health_current, 75.0);
    assert_eq!(
        actor(&current, oid).position.to_array(),
        logic.host_authoritative_pose(oid).unwrap()
    );
    assert_eq!(actor(&before, oid).health_current, 100.0);
}

#[test]
fn ordinary_mutable_object_lookup_borrows_the_receiving_body() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GwView");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("GwViewU")
            .kind_of("SELECTABLE")
            .health(80.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("GwViewU", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("id");
    let prior = publish(&mut logic);
    let object = logic.host_object_mut(oid).unwrap();
    assert_eq!(object.health.current, 80.0);
    object.health.current = 33.0;
    assert_eq!(logic.host_authoritative_health(oid), Some(33.0));
    let current = publish(&mut logic);
    assert_eq!(actor(&current, oid).health_current, 33.0);
    assert_eq!(actor(&prior, oid).health_current, 80.0);
}

#[test]
fn repeated_step_borrows_share_the_receiving_pose_and_target() {
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object OwnerPhaseUnit\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &golden_skirmish_config("OwnerPhase")).expect("config");
    insert_fixture_template(&mut logic, "OwnerPhaseUnit", 80.0);

    let source = logic
        .create_object("OwnerPhaseUnit", Team::USA, Vec3::new(1.0, 0.0, 2.0))
        .expect("source");
    let target = logic
        .create_object("OwnerPhaseUnit", Team::USA, Vec3::new(5.0, 0.0, 6.0))
        .expect("target");
    let initial = publish(&mut logic);
    let owner_position = [17.0, 0.0, 19.0];
    let final_owner_position = [23.0, 0.0, 29.0];

    {
        assert_eq!(logic.host_authoritative_target(source), None);
        assert_eq!(actor(&initial, source).position, Vec3::new(1.0, 0.0, 2.0));

        logic
            .with_host_logic_step(|owner| {
                {
                    let (object, attack_events) = owner
                        .host_object_and_attack_events_mut(source)
                        .expect("first owner mutation");
                    object.set_order_target(
                        Some(target),
                        attack_events,
                        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
                    );
                }

                owner.set_fixture_object_position(source, glam::Vec3::from_array(owner_position));

                owner
                    .with_host_object_mut(source, |object| {
                        assert_eq!(object.target, Some(target));
                        assert_eq!(
                            object.get_position().to_array(),
                            owner_position,
                            "the first host mutation must survive the second borrow"
                        );
                    })
                    .expect("second owner mutation");
                owner.set_fixture_object_position(
                    source,
                    glam::Vec3::from_array(final_owner_position),
                );

                assert_eq!(
                    owner.host_authoritative_target(source),
                    Some(target),
                    "in-phase target reads must use the same live Object"
                );
                assert_eq!(
                    owner.host_authoritative_pose(source),
                    Some(final_owner_position),
                    "in-phase pose reads must use the same live Object"
                );
                assert_eq!(
                    owner.host_authoritative_pose(target),
                    Some([5.0, 0.0, 6.0]),
                    "unmodified owner data remains intact"
                );

                // A nested phase cannot replace/reset the active guard. It is
                // rejected while the original owner scope remains active.
                assert!(owner.with_host_logic_step(|_| ()).is_none());
                assert_eq!(owner.host_authoritative_target(source), Some(target));
            })
            .expect("enter host-owned post-sync phase");

        // Leaving the reentry guard never changes the source of ordinary reads.
        assert_eq!(logic.host_authoritative_target(source), Some(target));
        assert_eq!(
            logic.host_authoritative_pose(source),
            Some(final_owner_position)
        );
        let latest = publish(&mut logic);
        assert_eq!(
            actor(&latest, source).position.to_array(),
            final_owner_position
        );
        assert_eq!(actor(&latest, source).attack_target, Some(target));
        assert_eq!(actor(&initial, source).position, Vec3::new(1.0, 0.0, 2.0));
    }
}

#[test]
fn logic_step_guard_restores_on_return_and_unwind() {
    let mut logic = GameLogic::new();
    logic
        .with_host_logic_step(|_| ())
        .expect("first phase entry");
    assert!(logic.with_host_logic_step(|_| ()).is_some());

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = logic.with_host_logic_step(|_| panic!("exercise guard drop"));
    }));
    assert!(unwind.is_err());
    assert!(logic.with_host_logic_step(|_| ()).is_some());
}

#[test]
fn same_id_spawns_keep_their_poses_when_the_other_world_moves_and_drops() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    insert_fixture_template(&mut a, "OwnedSpawnA", 80.0);
    insert_fixture_template(&mut b, "OwnedSpawnB", 90.0);
    let a_id = a
        .create_object("OwnedSpawnA", Team::USA, Vec3::new(11.0, 0.0, 12.0))
        .unwrap();
    let b_id = b
        .create_object("OwnedSpawnB", Team::China, Vec3::new(21.0, 0.0, 22.0))
        .unwrap();
    assert_eq!(
        a_id, b_id,
        "fixture deliberately overlaps session-local identities"
    );
    let owner = publish(&mut a);
    let other = publish(&mut b);
    assert_eq!(actor(&owner, a_id).template_name, "OwnedSpawnA");
    assert_eq!(actor(&other, b_id).template_name, "OwnedSpawnB");
    assert_eq!(actor(&owner, a_id).position, Vec3::new(11.0, 0.0, 12.0));
    assert_eq!(actor(&other, b_id).position, Vec3::new(21.0, 0.0, 22.0));
    b.set_fixture_object_position(b_id, Vec3::new(31.0, 0.0, 32.0));
    assert_eq!(
        actor(&publish(&mut b), b_id).position,
        Vec3::new(31.0, 0.0, 32.0)
    );
    drop(b);
    let current = publish(&mut a);
    assert_eq!(actor(&current, a_id).position, Vec3::new(11.0, 0.0, 12.0));
    assert_eq!(
        a.host_object(a_id).unwrap().get_position(),
        actor(&current, a_id).position
    );
    assert_eq!(actor(&other, b_id).position, Vec3::new(21.0, 0.0, 22.0));
}

#[test]
fn published_destroyed_body_carries_its_death_type() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DeathTy");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object DieUnit\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DieUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("DieUnit".into(), t);
    }
    let oid = logic
        .create_object("DieUnit", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.destroyed = true;
        o.status.death_type = HostDeathType::Burned;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(row.destroyed);
    assert_eq!(row.death_type_name, "BURNED");
    assert_eq!(
        logic.host_object(oid).unwrap().status.death_type,
        HostDeathType::Burned
    );
}

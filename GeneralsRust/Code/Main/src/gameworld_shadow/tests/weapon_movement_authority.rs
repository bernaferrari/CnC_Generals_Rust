//! Phase 2 weapon-slot authority + Phase 3 movement last-writer.

use super::*;
use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
use gamelogic::world::{WEAPON_SLOT_MINE_CLEAR, WEAPON_SLOT_PRIMARY, WEAPON_SLOT_TERTIARY};

#[test]
fn weapon_slots_tertiary_never_aliases_primary_and_ammo_writeback() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("WeapSlot");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object WeapU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("WeapU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("WeapU".into(), t);
    }
    let oid = logic
        .create_object("WeapU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(oid).expect("map");
    assert!(shadow.world().weapon_slots().slots(eid).is_none());
    {
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        o.weapon = Some(Weapon {
            clip_size: 10,
            ammo: Some(7),
            reload_time: 1.0,
            last_fire_time: 0.2,
            ..Weapon::default()
        });
        o.tertiary_weapon = Some(Weapon {
            clip_size: 4,
            ammo: Some(3),
            reload_time: 2.5,
            last_fire_time: 9.0,
            ..Weapon::default()
        });
    }

    shadow.sync_from_host(&logic);
    let primary = shadow
        .world()
        .weapon_slots()
        .slot(eid, WEAPON_SLOT_PRIMARY)
        .expect("primary");
    let tertiary = shadow
        .world()
        .weapon_slots()
        .slot(eid, WEAPON_SLOT_TERTIARY)
        .expect("tertiary");
    let mine = shadow
        .world()
        .weapon_slots()
        .slot(eid, WEAPON_SLOT_MINE_CLEAR)
        .expect("mine");
    assert_eq!(primary.clip_size, 10);
    assert_eq!(primary.ammo, 7);
    assert_eq!(tertiary.clip_size, 4);
    assert_eq!(tertiary.ammo, 3);
    assert_ne!(tertiary.clip_size, primary.clip_size);
    assert_ne!(tertiary.ammo, primary.ammo);
    assert!(
        !mine.present,
        "the selected C++ inventory has exactly three slots"
    );

    shadow
        .world_mut()
        .queue_mutation(gamelogic::world::WorldMutation::SetWeaponSlot {
            target: eid,
            slot: WEAPON_SLOT_PRIMARY,
            facts: gamelogic::world::WeaponSlotFacts {
                present: true,
                clip_size: 10,
                ammo: 6,
                reload_time: 1.0,
                last_fire_time: 0.2,
                barrel_cursor: 0,
                barrel_count: 1,
                lock_type: 0,
            },
        });
    let _ = shadow.world_mut().apply_pending_mutations();
    assert_eq!(
        shadow
            .world()
            .weapon_slots()
            .slot(eid, WEAPON_SLOT_PRIMARY)
            .map(|f| f.ammo),
        Some(6)
    );
    assert_eq!(
        shadow
            .world()
            .weapon_slots()
            .slot(eid, WEAPON_SLOT_TERTIARY)
            .map(|f| f.ammo),
        Some(3)
    );

    if let Some(o) = logic.host_object_mut(oid) {
        if let Some(w) = o.weapon.as_mut() {
            w.ammo = Some(6);
        }
    }
    shadow.sync_from_host(&logic);
    let host = logic.host_object(oid).expect("host");
    assert_eq!(host.weapon.as_ref().and_then(|w| w.ammo), Some(6));
    assert_eq!(host.tertiary_weapon.as_ref().and_then(|w| w.ammo), Some(3));
    let probe = shadow.probe(&mut logic);
    assert!(probe.weapon_match, "{}", probe.format_report());

    // A single removed slot clears while the related bank remains present.
    logic.host_object_mut(oid).expect("host").tertiary_weapon = None;
    shadow.sync_from_host(&logic);
    let slots = shadow.world().weapon_slots().slots(eid).expect("prior row");
    assert!(slots[usize::from(WEAPON_SLOT_PRIMARY)].present);
    assert_eq!(slots[usize::from(WEAPON_SLOT_PRIMARY)].ammo, 6);
    assert!(!slots[usize::from(WEAPON_SLOT_TERTIARY)].present);

    logic.host_object_mut(oid).expect("host").weapon = None;
    shadow.sync_from_host(&logic);
    let slots = shadow.world().weapon_slots().slots(eid).expect("prior row");
    assert!(slots.iter().all(|facts| !facts.present));
}

#[test]
fn movement_authority_path_follow_matches_host_only_golden() {
    let frames = 45u32;
    let start = glam::Vec3::new(0.0, 0.0, 0.0);
    let dest = glam::Vec3::new(80.0, 0.0, 0.0);
    let speed = 30.0;

    let golden = {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MvGold");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("MvGoldU").kind_of("INFANTRY").health(100.0).walker(),
                crate::fixture_content::FixtureObject::new("MvGwU").kind_of("INFANTRY").health(100.0).walker(),
            ],
        );
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let oid = logic
            .create_object("MvGoldU", Team::USA, start)
            .expect("id");
        {
            let (o, mut movement_events) =
                logic.host_object_and_movement_events_mut(oid).expect("o");
            o.movement.max_speed = speed;
            o.move_to(
                dest,
                0,
                &mut movement_events,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        for _ in 0..frames {
            logic.update_movement_for_test(&[oid], 1.0 / 30.0);
            shadow.sync_from_host(&logic);
        }
        let eid = shadow.entity_for_host(oid).expect("gmap");
        let p = shadow.world().entity(eid).expect("ge").transform.position;
        glam::Vec3::new(p[0], p[1], p[2])
    };

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("MvGw");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("MvGwU").kind_of("INFANTRY").health(100.0).walker()],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic.create_object("MvGwU", Team::USA, start).expect("id");
    {
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        o.movement.max_speed = speed;
        o.move_to(
            dest,
            0,
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    for _ in 0..frames {
        logic.update_movement_for_test(&[oid], 1.0 / 30.0);
        shadow.sync_from_host(&logic);
    }
    let eid = shadow.entity_for_host(oid).expect("map");
    let gw = shadow.world().entity(eid).expect("e").transform.position;
    assert!(
        (gw[0] - golden.x).abs() < 1e-3 && (gw[2] - golden.z).abs() < 1e-3,
        "GW pose {:?} vs host-only golden {golden}",
        gw
    );
    assert!(shadow.probe(&logic).pose_match);
    assert!(
        logic.host_object(oid).unwrap().get_position().x > start.x,
        "actual Main owner must move"
    );
}

#[test]
fn same_frame_object_observation_reuse_preserves_live_context_and_owner() {
    use gamelogic::common::ObjectShroudStatus;
    use gamelogic::world::{WeaponSlotFacts, WorldMutation};

    let make_world = |ammo| {
        let mut world = GameLogic::new();
        crate::fixture_content::admit_fixture_objects(
            &mut world,
            &[crate::fixture_content::FixtureObject::new("ObservedWeapon")
                .kind_of("INFANTRY SELECTABLE")],
        );
        world.add_player(crate::game_logic::Player::new(0, Team::USA, "Viewer", true));
        world.add_player(crate::game_logic::Player::new(1, Team::China, "Enemy", false));
        let id = world
            .create_object("ObservedWeapon", Team::China, glam::Vec3::ZERO)
            .unwrap();
        let actor = world.host_object_mut(id).unwrap();
        actor.weapon = Some(Weapon {
            clip_size: 10,
            ammo: Some(ammo),
            ..Weapon::default()
        });
        actor.subdual_damage = ammo as f32;
        (world, id)
    };

    let (mut world, object) = make_world(7);
    let (peer, peer_object) = make_world(8);
    assert_eq!(object, peer_object);
    let frame = world.get_frame();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&world);
    let entity = shadow.entity_for_host(object).unwrap();
    let ammo = |shadow: &GameWorldShadow| {
        shadow
            .world()
            .weapon_slots()
            .slot(entity, WEAPON_SLOT_PRIMARY)
            .unwrap()
            .ammo
    };
    let owning_revision = shadow.observed_object_revision.as_ref().unwrap().host_revision;
    assert_eq!(ammo(&shadow), 7);

    // Shroud belongs to another receiving service and remains live even on a
    // clean Object-owned revision. Ordinary external writes still resync.
    {
        let mut shroud = world.world_services.shroud().lock().unwrap();
        shroud.mark_host_vision_ready();
        shroud.mark_host_object_explored(0, object.0);
        shroud.set_host_object_shroud_status(0, object.0, ObjectShroudStatus::Fogged);
    }
    shadow.sync_from_host(&world);
    assert_eq!(shadow.observed_object_revision.as_ref().unwrap().host_revision, owning_revision);
    let cached = shadow.world().entity(entity).unwrap();
    assert_eq!(cached.fow_visibility_alpha, crate::fow_rendering::ObjectVisibility::FOGGED.visibility_alpha);
    let cached = format!("{cached:?}");
    shadow.observed_object_revision = None;
    shadow.sync_from_host(&world);
    assert_eq!(format!("{:?}", shadow.world().entity(entity).unwrap()), cached);

    // Queue a receiving-only write that applies after the existing main copy.
    // The next clean read must restore the actual Object values at this same frame.
    shadow.world_mut().queue_mutation(WorldMutation::SetWeaponSlot {
        target: entity,
        slot: WEAPON_SLOT_PRIMARY,
        facts: WeaponSlotFacts {
            present: true,
            clip_size: 10,
            ammo: 1,
            ..Default::default()
        },
    });
    shadow.sync_from_host(&world);
    assert_eq!(ammo(&shadow), 1);
    assert!(shadow.observed_object_revision.is_none());
    shadow.sync_from_host(&world);
    assert_eq!(ammo(&shadow), 7);

    let actor = world.host_object_mut(object).unwrap();
    actor.weapon.as_mut().unwrap().ammo = Some(5);
    actor.subdual_damage = 11.0;
    let revised = world.host_objects().observation_revision().unwrap().1;
    assert_ne!(revised, owning_revision);
    shadow.sync_from_host(&world);
    assert_eq!(ammo(&shadow), 5);
    assert_eq!(shadow.world().entity(entity).unwrap().subdual_damage, 11.0);

    shadow.sync_from_host(&peer);
    assert_eq!(ammo(&shadow), 8);
    assert_eq!(shadow.world().entity(entity).unwrap().subdual_damage, 8.0);
    shadow.sync_from_host(&world);
    assert_eq!(ammo(&shadow), 5);
    assert_eq!(world.get_frame(), frame);
    assert_eq!(peer.get_frame(), frame);
}

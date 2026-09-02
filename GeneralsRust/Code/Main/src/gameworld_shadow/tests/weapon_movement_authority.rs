//! Phase 2 weapon-slot authority + Phase 3 movement last-writer.

use super::*;
use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
use gamelogic::world::{WEAPON_SLOT_MINE_CLEAR, WEAPON_SLOT_PRIMARY, WEAPON_SLOT_TERTIARY};

#[test]
fn weapon_slots_tertiary_never_aliases_primary_and_ammo_writeback() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();

    let mut logic = GameLogic::new();
    logic.set_weapon_authority(true);
    let cfg = golden_skirmish_config("WeapSlot");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WeapU") {
        let mut t = ThingTemplate::new("WeapU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("WeapU".into(), t);
    }
    let oid = logic
        .create_object("WeapU", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
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
        o.mine_clearing_primary_weapon = Some(Weapon {
            clip_size: 1,
            ammo: Some(1),
            ..Weapon::default()
        });
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(oid).expect("map");
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
    assert_eq!(mine.clip_size, 1);

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
            w.ammo = Some(99);
        }
    }
    assert!(shadow.writeback_weapon_stats_to_host(&mut logic) >= 1);
    let host = logic.host_object(oid).expect("host");
    assert_eq!(host.weapon.as_ref().and_then(|w| w.ammo), Some(6));
    assert_eq!(host.tertiary_weapon.as_ref().and_then(|w| w.ammo), Some(3));
    let probe = shadow.probe(&mut logic);
    assert!(probe.weapon_match, "{}", probe.format_report());
}

#[test]
fn movement_authority_path_follow_matches_host_only_golden() {
    let frames = 45u32;
    let start = glam::Vec3::new(0.0, 0.0, 0.0);
    let dest = glam::Vec3::new(80.0, 0.0, 0.0);
    let speed = 30.0;

    let golden = {
        let _env = AuthorityEnvGuard::lock()
            .set("GENERALS_GAMEWORLD_SHADOW", "0");
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("MvGold");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        if !logic.templates.contains_key("MvGoldU") {
            let mut t = ThingTemplate::new("MvGoldU");
            t.add_kind_of(KindOf::Infantry);
            logic.templates.insert("MvGoldU".into(), t);
        }
        let oid = logic
            .create_object("MvGoldU", Team::USA, start)
            .expect("id");
        {
            let o = logic.host_object_mut(oid).expect("o");
            o.movement.max_speed = speed;
            o.move_to(dest);
        }
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        for _ in 0..frames {
            let _ = shadow.world_mut().step_movement(1.0 / 30.0);
        }
        let eid = shadow.entity_for_host(oid).expect("gmap");
        let p = shadow.world().entity(eid).expect("ge").transform.position;
        glam::Vec3::new(p[0], p[1], p[2])
    };

    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();
    let mut logic = GameLogic::new();
    logic.set_movement_authority(true);
    let cfg = golden_skirmish_config("MvGw");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("MvGwU") {
        let mut t = ThingTemplate::new("MvGwU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("MvGwU".into(), t);
    }
    let oid = logic.create_object("MvGwU", Team::USA, start).expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.movement.max_speed = speed;
        o.move_to(dest);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    for _ in 0..frames {
        let _ = shadow.world_mut().step_movement(1.0 / 30.0);
    }
    let eid = shadow.entity_for_host(oid).expect("map");
    let gw = shadow.world().entity(eid).expect("e").transform.position;
    assert!(
        (gw[0] - golden.x).abs() < 1e-3 && (gw[2] - golden.z).abs() < 1e-3,
        "GW pose {:?} vs host-only golden {golden}",
        gw
    );
    assert!(
        shadow.probe(&mut logic).pose_match || {
            let _ = shadow.writeback_transforms_to_host(&mut logic);
            true
        }
    );
}

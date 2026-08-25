//! Phase 4–5: factory/construction authority and contain roster.

use super::*;
use crate::game_logic::{
    BuildingData, BuildingType, KindOf, Team, ThingTemplate, host_construction_progress_log,
    host_production_progress_log,
};
use gamelogic::world::WorldMutation;

#[test]
fn production_queue_advances_one_frame_per_logic_update_and_writeback_matches() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1")
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("P4Fact");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("P4Fact") {
        let mut t = ThingTemplate::new("P4Fact");
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::FSBarracks);
        logic.templates.insert("P4Fact".into(), t);
    }
    let oid = logic
        .create_object("P4Fact", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    host_production_progress_log::record(
        oid,
        vec![host_production_progress_log::HostProductionQueueItem {
            template_name: "Ranger".into(),
            progress: 0.0,
            total_time: 1.0,
            construction_frames: 0,
            cost_supplies: 100,
            is_upgrade: false,
            quantity_total: 1,
            quantity_produced: 0,
        }],
        0.0,
        1.0,
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let _ = shadow.apply_host_production_progress_events(&host_production_progress_log::drain());

    const TICKS: u32 = 5;
    for _ in 0..TICKS {
        let n = shadow.tick_production_queues(1.0 / 30.0);
        assert!(
            n >= 1,
            "C++ ProductionUpdate.cpp:687 increments once per update"
        );
    }
    let eid = shadow.entity_for_host(oid).expect("map");
    let head = shadow
        .world()
        .entity(eid)
        .expect("e")
        .production_queue_items
        .first()
        .expect("head");
    assert_eq!(head.construction_frames, TICKS);
    assert!(shadow.writeback_production_to_host(&mut logic) >= 1);
    let host_frames = logic
        .host_object(oid)
        .and_then(|o| o.building_data.as_ref())
        .and_then(|bd| bd.production_queue.first())
        .map(|h| h.construction_frames);
    assert_eq!(host_frames, Some(TICKS));
    let probe = shadow.probe(&mut logic);
    assert!(probe.production_match, "{}", probe.format_report());
}

#[test]
fn production_door_advances_opening_to_waiting_open() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1")
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("P4Door");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("P4Door") {
        let mut t = ThingTemplate::new("P4Door");
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("P4Door".into(), t);
    }
    let oid = logic
        .create_object("P4Door", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.production_door_phase = 1;
        o.production_door_phase_end_frame = 10;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let n = shadow.tick_production_doors(10);
    assert!(n >= 1);
    let eid = shadow.entity_for_host(oid).expect("map");
    assert_eq!(
        shadow.world().entity(eid).expect("e").production_door_phase,
        2
    );
}

#[test]
fn construction_percent_completes_same_frame_and_writeback_matches() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1")
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("P4Ctor");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("P4Ctor") {
        let mut t = ThingTemplate::new("P4Ctor");
        t.set_health(400.0);
        t.build_time = 10.0;
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("P4Ctor".into(), t);
    }
    let oid = logic
        .create_object("P4Ctor", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.under_construction = true;
        o.construction_percent = 0.99;
    }
    host_construction_progress_log::clear();
    host_construction_progress_log::record_rate_only(oid, true, 1.0);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.queue_set_construction_for_host(oid, 0.99, true));
    let _ = shadow.apply_pending();
    let _ =
        shadow.apply_host_construction_progress_events(&host_construction_progress_log::drain());
    let n = shadow.tick_construction_progress(1.0 / 30.0);
    assert!(n >= 1);
    let eid = shadow.entity_for_host(oid).expect("map");
    let pct = shadow.world().entity(eid).expect("e").construction_percent;
    assert!(
        (pct - 1.0).abs() < 1e-5,
        "same-frame completion expected, got {pct}"
    );
    assert!(shadow.writeback_construction_to_host(&mut logic) >= 1);
    let host_pct = logic.host_object(oid).expect("h").construction_percent;
    assert!((host_pct - 1.0).abs() < 1e-5);
}

#[test]
fn contain_enter_exit_roster_and_container_destroy_ejects_before_remove() {
    let _env = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .set("GENERALS_GAMEWORLD_DEFERRED_DESTROY", "1")
        .couple();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("P5Cont");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["P5Bunker", "P5Inf"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            if name == "P5Bunker" {
                t.add_kind_of(KindOf::Structure);
            }
            logic.templates.insert(name.into(), t);
        }
    }
    let bunker = logic
        .create_object("P5Bunker", Team::USA, glam::Vec3::ZERO)
        .expect("b");
    let inf = logic
        .create_object("P5Inf", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("i");
    {
        let o = logic.host_object_mut(bunker).expect("b");
        o.building_data = Some(BuildingData::new(BuildingType::Bunker));
        assert!(o.add_occupant(inf));
    }
    {
        let o = logic.host_object_mut(inf).expect("i");
        o.set_contained_by(Some(bunker));
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid_b = shadow.entity_for_host(bunker).expect("b");
    let eid_i = shadow.entity_for_host(inf).expect("i");
    shadow
        .world_mut()
        .queue_mutation(WorldMutation::ContainEnter {
            container: eid_b,
            occupant: eid_i,
        });
    assert!(shadow.apply_pending() >= 1);
    assert_eq!(shadow.world().contain_roster().occupants(eid_b), &[eid_i]);
    assert_eq!(
        shadow.world().contain_roster().contained_by(eid_i),
        Some(eid_b)
    );
    let probe = shadow.probe(&mut logic);
    assert!(probe.contain_match, "{}", probe.format_report());

    shadow
        .world_mut()
        .queue_mutation(WorldMutation::Destroy(eid_b));
    let _ = shadow.apply_pending();
    assert!(shadow.world().entity(eid_b).expect("marked").destroyed);
    assert_eq!(shadow.world_mut().process_destroy_list(), 1);
    assert!(shadow.world().entity(eid_b).is_none());
    assert!(shadow.world().entity(eid_i).is_some());
    assert!(
        shadow
            .world()
            .contain_roster()
            .contained_by(eid_i)
            .is_none()
    );
    assert_eq!(
        shadow
            .world()
            .entity(eid_i)
            .expect("rider")
            .contained_by_host,
        0
    );
}

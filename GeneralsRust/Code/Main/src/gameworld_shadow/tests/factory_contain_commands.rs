//! Phase 4–5: factory/construction authority and contain roster.

use super::*;
use crate::game_logic::{
    BuildingData, BuildingType, KindOf, Team, ThingTemplate, host_production_progress_log,
};
use gamelogic::world::WorldMutation;

#[test]
fn production_queue_advances_one_frame_per_logic_update_and_writeback_matches() {
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
    {
        use crate::game_logic::{ProductionItem, ProductionKind, Resources};
        let mut data = BuildingData::new(BuildingType::Barracks);
        data.production_queue.push(ProductionItem {
            template_name: "Ranger".into(),
            progress: 0.0,
            total_time: 1.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        logic.host_object_mut(oid).unwrap().building_data = Some(data);
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    const TICKS: u32 = 5;
    for _ in 0..TICKS {
        logic.update_production(1.0 / 30.0);
    }
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(oid).expect("map");
    let head = shadow
        .world()
        .entity(eid)
        .expect("e")
        .production_queue_items
        .first()
        .expect("head");
    assert_eq!(head.construction_frames, TICKS);
    shadow.sync_from_host(&logic);
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
    // CPP advances after the deadline; bool reports all doors closed.
    assert!(!logic.host_object_mut(oid).unwrap().tick_production_door(10));
    assert_eq!(logic.host_object(oid).unwrap().production_door_phase, 1);
    assert!(!logic.host_object_mut(oid).unwrap().tick_production_door(11));
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(oid).expect("map");
    assert_eq!(
        shadow.world().entity(eid).expect("e").production_door_phase,
        2
    );
}

#[test]
fn construction_observation_does_not_complete_host_or_projection() {
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

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    let _ = shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(oid).expect("map");
    let pct = shadow.world().entity(eid).expect("e").construction_percent;
    assert!(
        (pct - 0.99).abs() < 1e-5,
        "shadow must not complete construction, got {pct}"
    );
    assert_eq!(shadow.compare_construction_to_host(&logic), 0);
    let host_pct = logic.host_object(oid).expect("h").construction_percent;
    assert!((host_pct - 0.99).abs() < 1e-5);
    assert!(logic.host_object(oid).unwrap().status.under_construction);
}

#[test]
fn contain_enter_exit_roster_and_container_destroy_ejects_before_remove() {
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
    assert_eq!(shadow.world().contain_roster().occupants(eid_b), &[eid_i]);
    assert_eq!(
        shadow.world().contain_roster().contained_by(eid_i),
        Some(eid_b)
    );
    let probe = shadow.probe(&mut logic);
    assert!(probe.contain_match, "{}", probe.format_report());

    assert!(logic.evacuate_container_now(bunker, true));
    assert!(logic.host_object(bunker).unwrap().occupants.is_empty());
    assert!(logic.host_object(inf).unwrap().contained_by.is_none());
    logic.destroy_object(bunker);
    assert!(logic.host_object(bunker).unwrap().status.destroyed);
    logic.process_destroy_list();
    shadow.sync_from_host(&logic);
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

//! Combat-status, player, contain, AI, special-power, and production log channels.

use super::*;

#[test]
fn canonical_containment_projects_without_writeback() {
    use crate::game_logic::{BuildingData, BuildingType, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ContainCh");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("BunkC").kind_of("SELECTABLE STRUCTURE").health(200.0),
            crate::fixture_content::FixtureObject::new("InfC").kind_of("SELECTABLE STRUCTURE").health(200.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["BunkC", "InfC"] {
        {
            let mut t = ThingTemplate::new(name);
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            if name == "BunkC" {
                t.add_kind_of(KindOf::Structure);
            }
            logic.templates.insert(name.into(), t);
        }
    }
    let bunker = logic
        .create_object("BunkC", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    let inf = logic
        .create_object("InfC", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("i");
    {
        let o = logic.host_object_mut(bunker).expect("b");
        o.building_data = Some(BuildingData::new(BuildingType::Bunker));
        if let Some(bd) = o.building_data.as_mut() {
            bd.max_garrison = 5;
        }
    }

    {
        let o = logic.host_object_mut(bunker).expect("b");
        assert!(o.add_occupant(inf));
    }
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(inf).expect("i");
        o.set_contained_by_with_partition_owner(Some(bunker), true, fixture_partition);
    }

    // Re-apply path

    {
        let o = logic.host_object_mut(bunker).expect("b");
        if let Some(bd) = o.building_data.as_mut() {
            bd.garrisoned_units.clear();
        }
        assert!(o.add_occupant(inf));
    }
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(inf).expect("i");
        o.set_contained_by_with_partition_owner(Some(bunker), true, fixture_partition);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid_i = shadow.entity_for_host(inf).expect("map i");
    let eid_b = shadow.entity_for_host(bunker).expect("map b");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
        e.contained_by_host = 0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
        e.garrison_count = 0;
        e.garrisoned_host_ids.clear();
    }

    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow.world().entity(eid_i).expect("e").contained_by_host,
        bunker.0
    );
    assert!(shadow.world().entity(eid_b).expect("e").garrison_count >= 1);
    assert!(shadow.world().entity(eid_b).expect("e").occupant_count >= 1);
    // Poison host then writeback via SetContain last-writer residual.

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_i) {
        e.contained_by_host = bunker.0;
    }
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid_b) {
        e.garrison_count = 1;
        e.garrisoned_host_ids = vec![inf.0];
    }
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&inf).expect("i").contained_by,
        Some(bunker)
    );
    let bd = logic
        .host_objects()
        .lookup(&bunker)
        .expect("b")
        .building_data
        .as_ref()
        .expect("bd");
    assert!(bd.garrisoned_units.contains(&inf));
}

#[test]
fn host_special_power_cooldown_remaining_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpCdCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.special_power_cooldown = 45.0;
        logic.templates.insert("SpU".into(), t);
    }
    let oid = logic
        .create_object("SpU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 45.0;
        o.special_power_cooldown_remaining = 18.0;
        o.special_power_ready = false;
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = true;
        e.special_power_cooldown_remaining = 0.0;
        e.special_power_cooldown = 0.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(!e.special_power_ready);
    assert!((e.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
    assert!((e.special_power_cooldown - 45.0).abs() < 1e-3);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = false;
        e.special_power_cooldown_remaining = 18.0;
        e.special_power_cooldown = 45.0;
    }
    shadow.sync_from_host(&logic);
    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!(!o.special_power_ready);
    assert!((o.special_power_cooldown_remaining - 18.0).abs() < 1e-3);
    assert!((o.special_power_cooldown - 45.0).abs() < 1e-3);
}

#[test]
fn canonical_special_power_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpReadyCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SpU".into(), t);
    }
    let id = logic
        .create_object("SpU", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_special_power_ready(true);
    }

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_special_power_ready(true);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = false;
    }

    shadow.sync_from_host(&logic);
    assert!(shadow.world().entity(eid).expect("e").special_power_ready);

    shadow.sync_from_host(&logic);
    assert!(
        logic
            .host_objects()
            .lookup(&id)
            .expect("o")
            .special_power_ready
    );
}

#[test]
fn canonical_stored_supplies_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StoreSupCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SsU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SsU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SsU".into(), t);
    }
    let id = logic
        .create_object("SsU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_stored_supplies(900);
    }

    {
        let o = logic.host_object_mut(id).expect("o");
        o.set_stored_supplies(900);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stored_supplies = 0;
    }

    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(eid).expect("e").stored_supplies, 900);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.stored_supplies = 900;
    }
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic
            .host_objects()
            .lookup(&id)
            .expect("o")
            .stored_resources
            .supplies,
        900
    );
}

#[test]
fn production_authority_writeback_is_queue_last_writer() {
    use crate::game_logic::host_production_progress_log::{self, HostProductionQueueItem};
    use crate::game_logic::{
        BuildingData, BuildingType, KindOf, ProductionItem, ProductionKind, Resources, Team,
        ThingTemplate,
    };
    let mut logic = GameLogic::new();

    host_production_progress_log::clear();

    let cfg = golden_skirmish_config("ProdAuthWB");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object ProdAuthBarracks\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("ProdAuthBarracks");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ProdAuthBarracks".into(), t);
    }
    let oid = logic
        .create_object(
            "ProdAuthBarracks",
            Team::USA,
            glam::Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("barracks");
    {
        let o = logic.host_object_mut(oid).expect("b");
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            runtime_entry_identity: 0,
            template_name: "ProdAuthRanger".into(),
            progress: 2.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 150,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        o.building_data = Some(bd);
    }

    let items = vec![HostProductionQueueItem {
        template_name: "ProdAuthRanger".into(),
        progress: 2.0,
        total_time: 10.0,
        construction_frames: 0,
        cost_supplies: 150,
        is_upgrade: false,
        quantity_total: 1,
        quantity_produced: 0,
    }];
    host_production_progress_log::record(oid, items.clone(), 0.0, 1.0);

    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .production_queue_items
        .clear();
    assert_eq!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .len(),
        items.len(),
        "poisoned observation cannot remove an actual queue item"
    );
    shadow.sync_from_host(&logic);
    assert_eq!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .production_queue_items
            .len(),
        items.len()
    );
    assert!(logic.cancel_production_at_index(oid, 0));
    assert!(
        logic
            .host_object(oid)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    shadow.sync_from_host(&logic);
    assert!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .production_queue_items
            .is_empty()
    );

    host_production_progress_log::clear();
}

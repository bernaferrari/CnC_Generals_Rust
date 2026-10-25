//! Executable command and lifecycle regressions.

fn evacuate_contain_log() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EvacContain201");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("EvacBunker201") {
        let mut t = ThingTemplate::new("EvacBunker201");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("EvacBunker201".into(), t);
    }
    if !logic.templates.contains_key("EvacInf201") {
        let mut t = ThingTemplate::new("EvacInf201");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("EvacInf201".into(), t);
    }
    let Some(bunker) = logic.create_object("EvacBunker201", Team::USA, Vec3::new(10.0, 0.0, 10.0))
    else {
        return false;
    };
    let Some(inf) = logic.create_object("EvacInf201", Team::USA, Vec3::new(12.0, 0.0, 12.0)) else {
        return false;
    };
    // Force bunker capacity + load passenger.
    if let Some(obj) = logic.host_object_mut(bunker) {
        if obj.building_data.is_none() {
            obj.building_data = Some(crate::game_logic::buildings::BuildingData::new(
                crate::game_logic::buildings::BuildingType::Bunker,
            ));
        } else if let Some(bd) = obj.building_data.as_mut() {
            bd.max_garrison = bd.max_garrison.max(5);
        }
    }
    // Contain via set_contained_by + add occupant if API exists
    if let Some((obj, partition)) = logic.host_object_and_partition_mut(inf) {
        obj.set_contained_by_with_partition_owner(Some(bunker), true, partition);
    }
    if let Some(obj) = logic.host_object_mut(bunker) {
        if let Some(bd) = obj.building_data.as_mut() {
            if !bd.garrisoned_units.contains(&inf) {
                bd.garrisoned_units.push(inf);
            }
        }
    }

    if !logic.evacuate_container_now(bunker, false) {
        return false;
    }
    if logic
        .host_object(inf)
        .is_none_or(|rider| rider.contained_by.is_some())
    {
        return false;
    }
    if logic.host_object(bunker).is_none_or(|owner| {
        owner
            .building_data
            .as_ref()
            .is_none_or(|building| building.garrisoned_units.contains(&inf))
    }) {
        return false;
    }
    let mut observer = crate::gameworld_shadow::GameWorldShadow::new(64);
    observer.sync_from_host(&logic);
    let Some(rider) = observer
        .entity_for_host(inf)
        .and_then(|id| observer.world().entity(id))
    else {
        return false;
    };
    let Some(owner) = observer
        .entity_for_host(bunker)
        .and_then(|id| observer.world().entity(id))
    else {
        return false;
    };
    rider.contained_by_host == 0 && !owner.garrisoned_host_ids.contains(&inf.0)
}

#[test]
fn evacuate_contain_log_runs() {
    assert!(evacuate_contain_log());
}

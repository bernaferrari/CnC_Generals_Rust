//! Executable command and lifecycle regressions.

fn gameworld_entity_view_deepen() -> bool {
    use crate::game_logic::Team;
    use crate::gameworld_shadow::{GameWorldShadow, presentation_view_from_shadow};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame; units are authored content.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("ViewDeep191")
            .kind_of("VEHICLE SELECTABLE")
            .health(180.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("GwEntityView191"),
    );
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let Some(oid) = logic.create_object("ViewDeep191", Team::USA, Vec3::new(14.0, 0.0, 15.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };
    // Author the real Main owner; the observer only copies these completed facts.
    let Some(object) = logic.host_object_mut(oid) else {
        return false;
    };
    object.movement.velocity = Vec3::new(1.5, 0.0, 2.5);
    object.movement.max_speed = 22.0;
    object.selected = true;
    object.team_color = [0.2, 0.4, 0.6, 1.0];
    object.body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged;
    object.health.current = 99.0;
    object.health.maximum = 180.0;
    object.max_health = 180.0;
    shadow.sync_from_host(&logic);

    let view = presentation_view_from_shadow(&shadow, 0);
    // View id is EntityId.get() (shadow store), not host ObjectId.
    let Some(ev) = view
        .entities
        .iter()
        .find(|e| e.template == "ViewDeep191" || e.id == eid.get())
    else {
        return false;
    };
    if (ev.velocity[0] - 1.5).abs() > 1e-3 || (ev.velocity[2] - 2.5).abs() > 1e-3 {
        return false;
    }
    if (ev.move_max_speed - 22.0).abs() > 1e-3 {
        return false;
    }
    if !ev.selected || ev.team_color != [0.2, 0.4, 0.6, 1.0] {
        return false;
    }
    if ev.body_damage_state != 1 || (ev.health - 99.0).abs() > 1e-3 {
        return false;
    }
    if (ev.max_health - 180.0).abs() > 1e-3 {
        return false;
    }

    true
}

#[test]
fn gameworld_entity_view_deepen_runs() {
    assert!(gameworld_entity_view_deepen());
}

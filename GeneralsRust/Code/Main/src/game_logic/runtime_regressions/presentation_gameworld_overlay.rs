//! Executable command and lifecycle regressions.

fn presentation_gameworld_overlay() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresGwOverlay187");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("OverlayU187") {
        let mut t = ThingTemplate::new("OverlayU187");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("OverlayU187".into(), t);
    }
    let Some(oid) = logic.create_object("OverlayU187", Team::USA, Vec3::new(10.0, 0.0, 10.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };

    // Desync shadow health as last-writer residual (pose via Transform if available).
    let Some((object, partition)) = logic.host_object_and_partition_mut(oid) else {
        return false;
    };
    object.health.current = 77.0;
    object.set_position_with_partition_owner(
        Vec3::new(42.0, 0.0, 43.0),
        0,
        &mut Default::default(),
        partition,
    );
    object.set_orientation_with_partition_owner(1.25, partition);
    shadow.sync_from_host(&logic);

    let mut pres = PresentationFrame::build_from_logic(&mut logic, 0);
    if pres.objects.is_empty() {
        return false;
    }
    let Some(before) = pres
        .objects
        .iter()
        .find(|o| o.id == oid)
        .map(|o| o.health_current)
    else {
        return false;
    };

    let n = pres.overlay_gameworld_shadow(&shadow);
    if n < 1 {
        return false;
    }
    let Some(obj) = pres.objects.iter().find(|o| o.id == oid) else {
        return false;
    };

    if (obj.health_current - 77.0).abs() > 1e-3 {
        return false;
    }
    if (obj.position.x - 42.0).abs() > 1e-3 || (obj.position.z - 43.0).abs() > 1e-3 {
        return false;
    }
    if (obj.orientation - 1.25).abs() > 1e-4 {
        return false;
    }
    let _ = before;

    true
}

#[test]
fn presentation_gameworld_overlay_runs() {
    assert!(presentation_gameworld_overlay());
}

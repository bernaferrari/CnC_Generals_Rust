//! Executable command and lifecycle regressions.

fn presentation_overlay_deepen() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresOverlayDeep189");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("OverlayDeep189") {
        let mut t = ThingTemplate::new("OverlayDeep189");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("OverlayDeep189".into(), t);
    }
    let Some(oid) = logic.create_object("OverlayDeep189", Team::USA, Vec3::new(11.0, 0.0, 12.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };

    let Some(object) = logic.host_object_mut(oid) else {
        return false;
    };
    object.movement.velocity = Vec3::new(3.5, 0.0, -2.25);
    object.movement.max_speed = 18.5;
    object.selected = true;
    object.team_color = [0.1, 0.2, 0.3, 1.0];
    object.body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged;
    object.health.current = 88.0;
    shadow.sync_from_host(&logic);

    let mut pres = PresentationFrame::build_from_logic(&mut logic, 0);
    if pres.objects.iter().all(|o| o.id != oid) {
        return false;
    }

    let n = pres.overlay_gameworld_shadow(&shadow);
    if n < 1 {
        return false;
    }
    let Some(obj) = pres.objects.iter().find(|o| o.id == oid) else {
        return false;
    };

    if (obj.velocity.x - 3.5).abs() > 1e-3 || (obj.velocity.z + 2.25).abs() > 1e-3 {
        return false;
    }
    if (obj.move_max_speed - 18.5).abs() > 1e-3 {
        return false;
    }
    if !obj.selected {
        return false;
    }
    if obj.team_color != [0.1, 0.2, 0.3, 1.0] {
        return false;
    }
    if obj.body_damage_state != 2 {
        return false;
    }
    if (obj.health_current - 88.0).abs() > 1e-3 {
        return false;
    }

    true
}

#[test]
fn presentation_overlay_deepen_runs() {
    assert!(presentation_overlay_deepen());
}

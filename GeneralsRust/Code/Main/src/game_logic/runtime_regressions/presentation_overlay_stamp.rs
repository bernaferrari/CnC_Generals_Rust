//! Executable command and lifecycle regressions.

fn presentation_overlay_stamp() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresOverlayStamp190");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("StampU190") {
        let mut t = ThingTemplate::new("StampU190");
        t.set_health(150.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("StampU190".into(), t);
    }
    let Some(oid) = logic.create_object("StampU190", Team::USA, Vec3::new(9.0, 0.0, 9.0)) else {
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
    object.health.current = 55.0;
    object.selected = true;
    shadow.sync_from_host(&logic);

    let mut pres = PresentationFrame::build_from_logic(&mut logic, 0);
    if pres.gameworld_overlay_stamped != 0 {
        return false;
    }
    let n = pres.overlay_gameworld_shadow(&shadow);
    if n < 1 {
        return false;
    }
    if pres.gameworld_overlay_stamped == 0 {
        return false;
    }
    if pres.gameworld_overlay_stamped != n {
        return false;
    }

    true
}

#[test]
fn presentation_overlay_stamp_runs() {
    assert!(presentation_overlay_stamp());
}

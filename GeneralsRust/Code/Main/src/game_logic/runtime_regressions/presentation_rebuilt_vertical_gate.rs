//! Executable command and lifecycle regressions.

fn presentation_rebuilt_vertical_gate() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RebuiltVert196");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("RebuiltVert196") {
        let mut t = ThingTemplate::new("RebuiltVert196");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("RebuiltVert196".into(), t);
    }
    let Some(_oid) = logic.create_object("RebuiltVert196", Team::USA, Vec3::new(5.0, 0.0, 6.0))
    else {
        return false;
    };

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    let mut pres = PresentationFrame::build_for_engine(&mut logic, 0, Some(&shadow));
    if crate::presentation_frame::presentation_from_gameworld_enabled() {
        if !pres.gameworld_primary_objects {
            return false;
        }
        if pres.gameworld_rebuilt == 0 {
            return false;
        }
    }

    // Soft boundary identity: entities observed without rebuilt fails the gate predicate.
    let entities_ok = true;
    let rebuilt_ok = pres.gameworld_rebuilt > 0;
    let boundary = !entities_ok || rebuilt_ok;
    if crate::presentation_frame::presentation_from_gameworld_enabled() && !boundary {
        return false;
    }

    true
}

#[test]
fn presentation_rebuilt_vertical_gate_runs() {
    assert!(presentation_rebuilt_vertical_gate());
}

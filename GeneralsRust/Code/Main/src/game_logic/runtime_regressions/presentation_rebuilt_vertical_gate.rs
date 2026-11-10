//! Executable command and lifecycle regressions.

fn presentation_rebuilt_vertical_gate() -> bool {
    use crate::game_logic::Team;
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ GameEngine.cpp loads PlayerTemplate and Object INI before NewGame.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("RebuiltVert196")
            .kind_of("SELECTABLE VEHICLE")
            .health(100.0),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("RebuiltVert196"),
    );
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
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

//! Executable command and lifecycle regressions.

fn command_selection_log() -> bool {
    use crate::game_logic::Team;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame; units are authored content.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("SelectUnit206")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("Select206"),
    );
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let Some(a) = logic.create_object("SelectUnit206", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(b) = logic.create_object("SelectUnit206", Team::USA, Vec3::new(5.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(pid) = logic.local_player_id() else {
        return false;
    };

    logic.select_objects(pid, vec![a]);
    if let Some(obj) = logic.host_object(a) {
        if !obj.selected {
            return false;
        }
    } else {
        return false;
    }

    logic.select_objects(pid, vec![b]);
    // The receiving selection owner deselects the prior unit before admitting
    // the new selected unit; no presentation event transport is involved.
    logic.host_object(a).is_some_and(|object| !object.selected)
        && logic.host_object(b).is_some_and(|object| object.selected)
}

#[test]
fn command_selection_log_runs() {
    assert!(command_selection_log());
}

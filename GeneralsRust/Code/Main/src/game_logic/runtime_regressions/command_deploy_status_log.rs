//! Executable command and lifecycle regressions.

fn command_deploy_status_log() -> bool {
    use crate::game_logic::Team;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    // C++ loads Object INI before NewGame; units are authored content.
    let mut logic = crate::skirmish_config::lobby_definition_tests::slot_world_with_objects(&[
        crate::fixture_content::FixtureObject::new("DeployUnit203")
            .kind_of("VEHICLE SELECTABLE")
            .health(100.0)
            .walker(),
    ]);
    let cfg = crate::skirmish_config::lobby_definition_tests::slot_config(
        &logic,
        &golden_skirmish_config("Deploy203"),
    );
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    let Some(uid) = logic.create_object("DeployUnit203", Team::USA, Vec3::new(30.0, 0.0, 30.0))
    else {
        return false;
    };

    if let Some((unit, audio_events)) = logic.host_object_and_audio_events_mut(uid) {
        unit.set_deployed(
            true,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        if !unit.is_deployed() {
            return false;
        }
    } else {
        return false;
    }
    logic
        .host_object(uid)
        .is_some_and(|object| object.is_deployed())
}

#[test]
fn command_deploy_status_log_runs() {
    assert!(command_deploy_status_log());
}

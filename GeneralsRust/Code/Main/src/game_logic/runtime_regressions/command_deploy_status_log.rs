//! Executable command and lifecycle regressions.

fn command_deploy_status_log() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Deploy203");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("DeployUnit203") {
        let mut t = ThingTemplate::new("DeployUnit203");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DeployUnit203".into(), t);
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

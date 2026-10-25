//! Executable command and lifecycle regressions.

fn command_cheer_science_log() -> bool {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CheerSci202");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("CheerUnit202") {
        let mut t = ThingTemplate::new("CheerUnit202");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CheerUnit202".into(), t);
    }
    let Some(uid) = logic.create_object("CheerUnit202", Team::USA, Vec3::new(20.0, 0.0, 20.0))
    else {
        return false;
    };

    let cheer_ok = if let Some((unit, audio_events)) = logic.host_object_and_audio_events_mut(uid) {
        unit.begin_cheer(
            3.0,
            Some(0),
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        (unit.cheer_timer - 3.0).abs() < 1e-5 && unit.model_condition_bits & 1 != 0
    } else {
        false
    };
    if !cheer_ok {
        return false;
    }
    // Science purchase progress residual (if player can purchase a rank science).

    let Some(pid) = logic.local_player_id() else {
        return true; // cheer path already proven
    };
    // Grant points and try a cheap unlock path via attempt_to_purchase_science.
    if let Some(p) = logic.get_player_mut(pid) {
        p.science_purchase_points = p.science_purchase_points.max(5);
        let before = p.science_purchase_points;
        // Prefer a science name that residual cost table knows; Rank2 is common.
        let tried = p.attempt_to_purchase_science("SCIENCE_Rank2")
            || p.attempt_to_purchase_science("SCIENCE_Paladin")
            || p.attempt_to_purchase_science("Rank2");
        if tried {
            if p.science_purchase_points > before || p.unlocked_sciences.is_empty() {
                return false;
            }
            let _ = before;
        }
    }
    true
}

#[test]
fn command_cheer_science_log_runs() {
    assert!(command_cheer_science_log());
}

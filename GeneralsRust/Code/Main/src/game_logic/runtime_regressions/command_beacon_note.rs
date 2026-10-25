//! Executable command and lifecycle regressions.

fn command_beacon_note() -> bool {
    use crate::game_logic::GameLogic;
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let pos = Vec3::new(11.0, 0.0, 22.0);
    logic.note_beacon_placed(pos);
    let recent = logic.recent_beacons();
    recent.iter().any(|p| (*p - pos).length() < 0.01)
}

#[test]
fn command_beacon_note_runs() {
    assert!(command_beacon_note());
}

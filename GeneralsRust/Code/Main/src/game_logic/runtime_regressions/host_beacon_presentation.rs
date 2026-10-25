//! Executable command and lifecycle regressions.

fn host_beacon_presentation() -> bool {
    use crate::game_logic::GameLogic;
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let a = Vec3::new(1.0, 0.0, 2.0);
    let b = Vec3::new(5.0, 0.0, 6.0);
    logic.note_beacon_placed(a);
    logic.note_beacon_placed(b);
    if logic.host_beacons().len() != 2 {
        return false;
    }
    // Near-duplicate place replaces within match threshold.
    logic.note_beacon_placed(a + Vec3::new(0.5, 0.0, 0.0));
    if logic.host_beacons().len() != 2 {
        return false;
    }
    logic.note_beacon_removed_latest();
    logic.host_beacons().len() == 1
}

#[test]
fn host_beacon_presentation_runs() {
    assert!(host_beacon_presentation());
}

//! C++ PathfindZoneManager scheduling (AIPathfind.cpp:2245-2256, 5881-5884):
//! a footprint change only marks zones dirty; existing cells keep their zone
//! until processPathfindQueue recalculates at most ZONE_UPDATE_FREQUENCY
//! (300) frames later, so clientSafeQuickDoesPathExist reads stale zones.
use super::*;

fn open_world() -> PathfindingSystem {
    let mut system = PathfindingSystem::new(640., 640.);
    system.grid.rebuild_path_zones();
    system
}

fn wall_off(system: &mut PathfindingSystem) {
    for y in 0..system.grid.height() {
        system
            .grid
            .set_cell_type(GridPos::new(30, y), PathfindCellType::Obstacle);
    }
    system.mark_zones_dirty();
}

#[test]
fn footprint_change_keeps_stale_zones_until_the_cpp_recalculation_frame_while_worlds_take_turns() {
    let start = Vec3::new(250., 0., 100.);
    let goal = Vec3::new(350., 0., 100.);
    let mut walled = open_world();
    let mut other = open_world();
    walled.note_logic_frame(100);
    other.note_logic_frame(100);
    wall_off(&mut walled);
    for frame in 100..400 {
        walled.note_logic_frame(frame);
        other.note_logic_frame(frame);
        assert!(!walled.calculate_zones_if_due(), "frame {frame}: recalculation is due at 400");
        assert!(!other.calculate_zones_if_due(), "frame {frame}: the other world never dirtied");
        assert!(walled.client_safe_quick_does_path_exist(start, goal),
            "frame {frame}: stale zones still connect both sides");
    }
    walled.note_logic_frame(400);
    other.note_logic_frame(400);
    assert!(walled.calculate_zones_if_due());
    assert!(!walled.client_safe_quick_does_path_exist(start, goal), "recalculated zones see the wall");
    assert!(!other.calculate_zones_if_due());
    assert!(other.client_safe_quick_does_path_exist(start, goal));
    // calculateZones resets the schedule; later marks use min(next, frame + 300).
    walled.note_logic_frame(401);
    assert!(!walled.calculate_zones_if_due());
}

#[test]
fn early_frame_marks_recalculate_on_frame_two_like_cpp() {
    let mut system = open_world();
    system.note_logic_frame(0);
    wall_off(&mut system);
    system.note_logic_frame(1);
    assert!(!system.calculate_zones_if_due());
    system.note_logic_frame(2);
    assert!(system.calculate_zones_if_due());
}

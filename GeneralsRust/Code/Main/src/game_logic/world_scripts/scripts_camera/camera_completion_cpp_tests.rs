//! Original compiled predicate/cancellation-tail records, not a terrain or
//! full-engine oracle. Regenerate with scripts/generate_camera_completion_cpp.py.
use super::*;
use game_client::display::view::{FilterMode, FilterType};

#[test]
fn receiving_camera_completion_and_look_at_match_original_cpp_component() {
    let filters = [
        FilterType::Null,
        FilterType::BlackAndWhite,
        FilterType::Crossfade,
        FilterType::MotionBlur,
    ];
    let modes = [
        FilterMode::Null,
        FilterMode::BWBlackAndWhite,
        FilterMode::BWRedAndWhite,
        FilterMode::BWGreenAndWhite,
        FilterMode::CrossfadeFbMask,
        FilterMode::MBInAndOutAlpha,
        FilterMode::MBInAndOutSaturate,
        FilterMode::MBInAlpha,
        FilterMode::MBOutAlpha,
        FilterMode::MBInSaturate,
        FilterMode::MBOutSaturate,
        FilterMode::MBEndPanAlpha,
        FilterMode::MBPanAlpha,
        FilterMode::MBPanAlpha1,
        FilterMode::MBPanAlpha2,
        FilterMode::MBPanAlpha3,
    ];
    let mut world = GameLogic::new();
    let mut count = 0;
    for line in include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/camera_completion_cpp.csv"
    ))
    .lines()
    .filter(|l| !l.starts_with('#'))
    {
        let row: Vec<usize> = line.split(',').map(|v| v.parse().unwrap()).collect();
        assert_eq!(row.len(), 8);
        let [mask, filter, mode, before, after, rotate, pitch, zoom]: [usize; 8] =
            row.try_into().unwrap();
        world.session_view.scripted.script_camera_rotate_remaining =
            if mask & 1 != 0 { 1. } else { 0. };
        world.session_view.scripted.script_camera_pitch_remaining =
            if mask & 2 != 0 { 1. } else { 0. };
        world.session_view.scripted.script_camera_zoom_remaining =
            if mask & 4 != 0 { 1. } else { 0. };
        world.session_view.scripted.script_camera_freeze_time = true;
        world.session_view.filter.set_filter(filters[filter]);
        world.session_view.filter.set_mode(modes[mode]);
        assert_eq!(
            world.camera_movement_finished(),
            before != 0,
            "before {line}"
        );
        assert_eq!(
            world.is_script_camera_time_frozen(),
            before == 0,
            "freeze before {line}"
        );
        world.cancel_scripted_camera_from_player_look_at();
        assert_eq!(world.camera_movement_finished(), after != 0, "after {line}");
        assert_eq!(
            world.is_script_camera_time_frozen(),
            after == 0,
            "freeze after {line}"
        );
        assert!(
            world.session_view.scripted.script_camera_freeze_time,
            "LookAt does not disarm freeze {line}"
        );
        assert_eq!(
            world.session_view.scripted.script_camera_rotate_remaining > 0.,
            rotate != 0,
            "rotate {line}"
        );
        assert_eq!(
            world.session_view.scripted.script_camera_pitch_remaining > 0.,
            pitch != 0,
            "pitch {line}"
        );
        assert_eq!(
            world.session_view.scripted.script_camera_zoom_remaining > 0.,
            zoom != 0,
            "zoom {line}"
        );
        count += 1;
    }
    assert_eq!(count, 512);
}

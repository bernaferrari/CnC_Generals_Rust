//! Original W3DShaderManager.cpp CPU execution, not expected values generated
//! from Rust. Fixture pins original source SHA and extraction ranges. Full
//! setup/constructor and CPU postRender blocks were compiled with clang++;
//! DX admission, UV transforms and drawing are explicitly outside this oracle.
use super::*;

#[derive(Deserialize)]
struct Fixture {
    scenarios: Vec<Scenario>,
}
#[derive(Deserialize)]
struct Scenario {
    scenario: String,
    mode: u32,
    rows: Vec<(u32, i32, bool, bool, bool, u32, bool)>,
}
fn original_mode(mode: u32) -> FilterMode {
    // Original CommandXlat.h enum order (also source-pinned in the fixture).
    match mode {
        6 => FilterMode::MBInAndOutAlpha,
        7 => FilterMode::MBInAndOutSaturate,
        8 => FilterMode::MBInAlpha,
        9 => FilterMode::MBOutAlpha,
        10 => FilterMode::MBInSaturate,
        11 => FilterMode::MBOutSaturate,
        12 => FilterMode::MBEndPanAlpha,
        14 | 31 => FilterMode::MBPanAlpha,
        other => panic!("unrecognized original mode {other}"),
    }
}

#[test]
fn original_cpp_motion_blur_reachable_cpu_rows_match_owned_state() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/motion_blur_cpp_cpu.json")).unwrap();
    let mut state = ViewFilterState::new();
    let mut admitted = 0;
    let mut controller_disabled = 0;
    for scenario in fixture.scenarios {
        let mode = original_mode(scenario.mode);
        if scenario.scenario.starts_with("endpan") {
            // Probe uses this same pan instance, then setup END_PAN. This
            // preserves priorDelta, count and custom17 pan factor.
            assert!(state.set_mode(mode));
        } else {
            state = ViewFilterState::new();
            if scenario.scenario.ends_with("_zoom") && !scenario.scenario.ends_with("no_zoom") {
                // Probe sets static zoom valid BEFORE setup; setup preserves it.
                state.set_pos(Point3::new(3., 4., 5.));
            }
            if scenario.scenario == "pan17" {
                state.set_motion_blur_follow(17);
            } else {
                assert!(state.set_filter(FilterType::MotionBlur));
                assert!(state.set_mode(mode));
            }
        }
        let mut active = true;
        for (ordinal, (frame, count, decrement, skip, keep, look_at, peak)) in
            scenario.rows.into_iter().enumerate()
        {
            // The C++ CPU harness intentionally kept calling its component
            // after false. Real W3DView disables the effect; Rust combines that
            // controller with CPU state, so those suffixes must stay Null.
            if !active {
                controller_disabled += 1;
                assert_eq!(state.filter(), FilterType::Null);
                assert_eq!(state.mode(), FilterMode::Null);
                continue;
            }
            let scroll = if scenario.scenario.starts_with("pan") {
                Vector2 {
                    x: if ordinal < 2 { 0.001 } else { 1. },
                    y: 0.,
                }
            } else {
                Vector2::zero()
            };
            let actual = state.post_render(frame, scroll);
            let where_at = format!("{} render{ordinal} frame{frame}", scenario.scenario);
            assert_eq!(actual.motion_blur_count, count, "{where_at}: count");
            assert_eq!(state.decrement, decrement, "{where_at}: decrement");
            assert_eq!(actual.skip_render, skip, "{where_at}: skip");
            assert_eq!(state.skip_render(), skip, "{where_at}: stored skip");
            assert_eq!(actual.continue_effect, keep, "{where_at}: continue");
            assert_eq!(
                actual.motion_blur_count == MOTION_BLUR_MAX_COUNT,
                peak,
                "{where_at}: count peak (not camera lookup)"
            );
            assert_eq!(
                u32::from(actual.look_at.is_some()),
                look_at,
                "{where_at}: original lookAt call"
            );
            if let Some(position) = actual.look_at {
                assert_eq!(
                    position,
                    Point3::new(3., 4., 5.),
                    "{where_at}: lookAt destination"
                );
            }
            admitted += 1;
            active = keep;
            if !keep {
                assert_eq!(state.filter(), FilterType::Null);
                assert_eq!(state.mode(), FilterMode::Null);
            }
        }
    }
    assert_eq!(admitted, 177, "reachable original CPU rows");
    assert_eq!(
        controller_disabled, 111,
        "raw component suffix rows excluded by real controller"
    );
}

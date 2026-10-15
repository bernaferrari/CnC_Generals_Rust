//! Executed original W3DView.cpp component, not Rust-generated expectations.
//! Original SHA256 0a62e7572862b8a3f75cb5d3427f19550f02dbf5d8920f55ced2ab98b11736ff.
//! Source ranges 3224-3281 (shake), 1273-1293 (full update shake branch).
//! Packet438 cpp-impulse/extraction.json records exact extraction and clang++
//! -std=c++17 -O0 -ffp-contract=off. Original math.h compiled on macOS ARM;
//! this establishes that component profile, not retail Windows libm or gameplay.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    input: Input,
    expected: Vec<Observation>,
}
#[derive(Deserialize)]
struct Input {
    name: String,
    initial_bits: [u32; 5],
    view_position_bits: [u32; 3],
    range_bits: u32,
    max_intensity_bits: u32,
    rule_intensities_bits: [u32; 6],
    events: Vec<Event>,
    steps: usize,
}
#[derive(Deserialize)]
struct Event {
    epicenter_bits: [u32; 3],
    kind: usize,
    angle_bits: u32,
}
#[derive(Deserialize)]
struct Observation {
    phase: String,
    index: usize,
    state_bits: [u32; 5],
    draws: usize,
}

#[test]
fn original_cpp_enqueue_and_update_match_all_recorded_scalar_bits() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("view_impulse_cpp_cases.json")).unwrap();
    assert_eq!(cases.len(), 22);
    let kinds = [
        CameraShakeType::Subtle,
        CameraShakeType::Normal,
        CameraShakeType::Strong,
        CameraShakeType::Severe,
        CameraShakeType::CineExtreme,
        CameraShakeType::CineInsane,
    ];
    let mut observations = 0;
    for case in cases {
        let input = case.input;
        let [mut intensity, mut cosine, mut sine, x, y] = input.initial_bits.map(f32::from_bits);
        let mut offset = [x, y];
        let mut rules = GlobalData::default();
        rules.max_shake_range = f32::from_bits(input.range_bits);
        rules.max_shake_intensity = f32::from_bits(input.max_intensity_bits);
        [
            rules.shake_subtle_intensity,
            rules.shake_normal_intensity,
            rules.shake_strong_intensity,
            rules.shake_severe_intensity,
            rules.shake_cine_extreme_intensity,
            rules.shake_cine_insane_intensity,
        ] = input.rule_intensities_bits.map(f32::from_bits);
        let view = input.view_position_bits.map(f32::from_bits);
        let mut draws = 0;
        for expected in case.expected {
            match expected.phase.as_str() {
                "initial" => {}
                "enqueue" => {
                    let event = &input.events[expected.index];
                    let center = event.epicenter_bits.map(f32::from_bits);
                    ViewImpulse {
                        intensity: &mut intensity,
                        cosine: &mut cosine,
                        sine: &mut sine,
                    }
                    .enqueue(
                        [center[0], center[1]],
                        [view[0], view[1]],
                        kinds[event.kind],
                        &rules,
                        &mut |low, high| {
                            assert_eq!((low, high), (0.0, 2.0 * std::f32::consts::PI));
                            draws += 1;
                            f32::from_bits(event.angle_bits)
                        },
                    );
                }
                "step" => {
                    assert!(expected.index < input.steps);
                    offset = ViewImpulse {
                        intensity: &mut intensity,
                        cosine: &mut cosine,
                        sine: &mut sine,
                    }
                    .step();
                }
                phase => panic!("unknown original observation {phase}"),
            }
            assert_eq!(
                [intensity, cosine, sine, offset[0], offset[1]].map(f32::to_bits),
                expected.state_bits,
                "{} {}:{}",
                input.name,
                expected.phase,
                expected.index
            );
            assert_eq!(draws, expected.draws, "{} angle admission", input.name);
            observations += 1;
        }
    }
    assert!(observations > 60);
}

//! Real View impulse state with supplied receiving rules, without global setup.
use super::*;

#[test]
fn shake_uses_receiving_view_position_and_rules_without_changing_peer() {
    let mut first = View::new();
    let mut peer = View::new();
    peer.position = Point3::new(1000.0, 0.0, 0.0);
    let mut rules = game_engine::common::global_data::GlobalData::default();
    rules.shake_normal_intensity = 2.0;
    rules.max_shake_range = 100.0;
    rules.max_shake_intensity = 10.0;
    first.shake_with_services(
        &Point3::new(50.0, 0.0, 0.0),
        CameraShakeType::Normal,
        &rules,
        &mut |_, _| 0.0,
    );
    peer.shake_with_services(
        &Point3::new(50.0, 0.0, 0.0),
        CameraShakeType::Normal,
        &rules,
        &mut |_, _| 0.0,
    );
    assert_eq!(first.shake_intensity, 1.0);
    assert_eq!(first.shake_offset.x, 1.0);
    assert_eq!(peer.shake_intensity, 0.0);
    assert_eq!(peer.shake_offset.x, 0.0);
}

#[test]
fn cpp_shake_overflow_clamps_to_three_after_supplied_impulse() {
    let mut view = View::new();
    let mut rules = game_engine::common::global_data::GlobalData::default();
    rules.shake_normal_intensity = 5.0;
    rules.max_shake_range = 100.0;
    rules.max_shake_intensity = 4.0;
    view.shake_with_services(
        &Point3::new(0.0, 0.0, 0.0),
        CameraShakeType::Normal,
        &rules,
        &mut |_, _| 0.0,
    );
    assert_eq!(view.shake_intensity, 3.0);
    assert_eq!(view.shake_offset.x, 3.0);
}

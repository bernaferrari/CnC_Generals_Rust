//! Trace the actual Rust health kernel for comparison with the executed C++ method.
use super::*;
use crate::game_logic::object::BodyStateFx;

fn body(id: u32) -> Object {
    let mut template = ThingTemplate::new("HealthComponentTrace");
    template.set_health(100.0);
    Object::new(template, ObjectId(id), Team::USA)
}
fn emit(object: &Object, scenario: &str, phase: &str, calls: u32) {
    println!(
        "BODY_CPP_PARITY {}",
        serde_json::json!({
            "scenario": scenario, "phase": phase,
            "hp_bits": object.health.current.to_bits(),
            "previous_bits": object.previous_health.to_bits(),
            "state": object.body_damage_state.ordinal(),
            "effectively_dead": object.status.effectively_dead,
            "under_construction": object.status.under_construction,
            "visual_calls": calls,
        })
    );
}
fn step(object: &mut Object, scenario: &str, delta: f32, calls: &mut u32) {
    object.internal_change_health_with_body_fx(delta, &mut |object, effect, _| {
        if let BodyStateFx::NativeVisualState(_) = effect {
            *calls += 1;
            emit(object, scenario, "visual", *calls);
        } else {
            panic!("ordinary health kernel cannot dispatch later DamageModule effects");
        }
    });
    emit(object, scenario, "complete", *calls);
}
#[test]
fn ordinary_health_callback_component_trace() {
    let mut ordinary = body(1);
    let mut calls = 0;
    step(&mut ordinary, "unchanged_capped", 10.0, &mut calls);
    step(&mut ordinary, "damage_crossing", -40.0, &mut calls);
    step(&mut ordinary, "unchanged_damage_state", -1.0, &mut calls);
    step(&mut ordinary, "lethal_crossing", -100.0, &mut calls);
    step(&mut ordinary, "heal_crossing_from_dead", 50.0, &mut calls);
    ordinary.status.under_construction = true;
    step(&mut ordinary, "construction_crossing", -30.0, &mut calls);
    ordinary.status.under_construction = false;
    step(&mut ordinary, "heal_to_pristine", 80.0, &mut calls);
    ordinary.status.effectively_dead = true;
    step(
        &mut ordinary,
        "unchanged_health_clears_ai_dead",
        0.0,
        &mut calls,
    );
    let mut thresholds = body(2);
    let mut calls = 0;
    step(
        &mut thresholds,
        "damaged_threshold_equal",
        -30.0,
        &mut calls,
    );
    step(
        &mut thresholds,
        "really_damaged_threshold_equal",
        -35.0,
        &mut calls,
    );
    let mut construction = body(3);
    let mut calls = 0;
    construction.status.under_construction = true;
    step(&mut construction, "construction_lethal", -100.0, &mut calls);
    construction.status.under_construction = false;
    step(
        &mut construction,
        "post_construction_same_state",
        0.0,
        &mut calls,
    );
    step(
        &mut construction,
        "post_construction_heal",
        100.0,
        &mut calls,
    );
}

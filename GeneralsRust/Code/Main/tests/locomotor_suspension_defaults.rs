//! Public Common, direct/named GameLogic conversion, and Main visual binding.
//! Binding records alone do not prove rendered physics, assets, or GPU behavior.
#[path = "../../GameEngine/Common/tests/support/locomotor_suspension.rs"]
mod fixture;
use game_engine::common::ini::ini_locomotor::LocomotorTemplate;
use gamelogic::locomotor::ini_bridge::{convert_named, from_common_ini_template};
use generals_main::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding;

fn direct(src: &LocomotorTemplate) -> [f32; 5] {
    let t = from_common_ini_template(src);
    [
        t.pitch_stiffness,
        t.roll_stiffness,
        t.pitch_damping,
        t.roll_damping,
        t.uniform_axial_damping,
    ]
}
fn named(src: &LocomotorTemplate) -> [f32; 5] {
    let t = convert_named(src.name.as_str()).unwrap();
    [
        t.pitch_stiffness,
        t.roll_stiffness,
        t.pitch_damping,
        t.roll_damping,
        t.uniform_axial_damping,
    ]
}
fn binding(src: &LocomotorTemplate) -> [f32; 5] {
    let t = resolve_host_locomotor_binding(src.name.as_str())
        .unwrap()
        .visual_physics;
    [
        t.pitch_stiffness,
        t.roll_stiffness,
        t.pitch_damping,
        t.roll_damping,
        t.uniform_axial_damping,
    ]
}

#[test]
fn common_omitted() {
    fixture::check("Maincommonomitted", "omitted", fixture::common);
}

#[test]
fn common_zero() {
    fixture::check("Maincommonzero", "zero", fixture::common);
}

#[test]
fn common_control() {
    fixture::check("Maincommoncontrol", "control", fixture::common);
}

#[test]
fn direct_omitted() {
    fixture::check("Maindirectomitted", "omitted", direct);
}

#[test]
fn direct_zero() {
    fixture::check("Maindirectzero", "zero", direct);
}

#[test]
fn direct_control() {
    fixture::check("Maindirectcontrol", "control", direct);
}

#[test]
fn named_omitted() {
    fixture::check("Mainnamedomitted", "omitted", named);
}

#[test]
fn named_zero() {
    fixture::check("Mainnamedzero", "zero", named);
}

#[test]
fn named_control() {
    fixture::check("Mainnamedcontrol", "control", named);
}

#[test]
fn binding_omitted() {
    fixture::check("Mainbindingomitted", "omitted", binding);
}

#[test]
fn binding_zero() {
    fixture::check("Mainbindingzero", "zero", binding);
}

#[test]
fn binding_control() {
    fixture::check("Mainbindingcontrol", "control", binding);
}

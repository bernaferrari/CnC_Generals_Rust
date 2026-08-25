//! Host present-path physics visual regressions.

use super::physics_visual_host::{
    HostPhysicsVisualFacts, apply_to_world_matrix, body_for_object_with_height_samples,
    insert_facts_for_test, loco_state, reset_host_physics_visual_state,
};
use super::physics_visual_host_inputs::{
    ObjectVisualIni, clear_test_object_visual_ini, set_test_object_visual_ini,
    terrain_normal_zup_from_height_samples,
};
use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
use game_client::physics_visual::{
    LocomotorVisualParams, PhysicsVisualAppearance, PhysicsVisualBody, PhysicsVisualLocoState,
};
use glam::Mat4;
use glam::Vec3;
use std::collections::HashMap;

fn hover_facts(motive_accel_x: f32, frozen: bool) -> HostPhysicsVisualFacts {
    HostPhysicsVisualFacts {
        appearance: PhysicsVisualAppearance::Hover,
        params: LocomotorVisualParams {
            forward_accel_coef: 0.5,
            pitch_stiffness: 0.1,
            pitch_damping: 0.9,
            uniform_axial_damping: 1.0,
            ..LocomotorVisualParams::default()
        },
        body: PhysicsVisualBody {
            is_motive: true,
            accel_x: motive_accel_x,
            dir_x: 1.0,
            dir_y: 0.0,
            ..PhysicsVisualBody::default()
        },
        object_disabled_held: false,
        show_client_physics: true,
        tactical_view_time_frozen: false,
        camera_movement_finished: true,
        script_time_frozen_debug: false,
        script_time_frozen_script: frozen,
    }
}

#[test]
fn accelerating_unit_pitches_on_host_present() {
    reset_host_physics_visual_state();
    let id = ObjectId(42);
    insert_facts_for_test(id, hover_facts(4.0, false));
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    let loco = loco_state(id);
    assert!(
        loco.acceleration_pitch_rate < 0.0,
        "forward accel should kick nose-up pitch rate, got {}",
        loco.acceleration_pitch_rate
    );
}

#[test]
fn stored_loco_state_advances_across_two_frames() {
    reset_host_physics_visual_state();
    let id = ObjectId(43);
    insert_facts_for_test(id, hover_facts(4.0, false));
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    let first = loco_state(id);
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    let second = loco_state(id);
    assert_ne!(first, second);
}

#[test]
fn script_freeze_stops_loco_advancement() {
    reset_host_physics_visual_state();
    let id = ObjectId(44);
    insert_facts_for_test(id, hover_facts(4.0, true));
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    assert_eq!(loco_state(id), PhysicsVisualLocoState::default());
}

#[test]
fn held_gate_skips_host_physics_visual() {
    reset_host_physics_visual_state();
    let id = ObjectId(45);
    let mut facts = hover_facts(4.0, false);
    facts.object_disabled_held = true;
    insert_facts_for_test(id, facts);
    let _ = apply_to_world_matrix(id, Mat4::IDENTITY);
    assert_eq!(loco_state(id), PhysicsVisualLocoState::default());
}

#[test]
fn wheel_pitch_follows_sloped_host_terrain_normal() {
    let pos = Vec3::new(0.0, 0.0, 0.0);
    let sloped = terrain_normal_zup_from_height_samples(|sample| Some(sample.x * 0.2), pos);
    assert!(
        sloped.0 < 0.0 && sloped.2 > 0.0,
        "rising +X slope must tilt the C++ Z-up normal, got {sloped:?}"
    );
    assert_ne!(sloped, (0.0, 0.0, 1.0));

    let flat = terrain_normal_zup_from_height_samples(|_| None, pos);
    assert_eq!(flat, (0.0, 0.0, 1.0));
}

fn test_object(id: u32, template: &str) -> Object {
    Object::new(ThingTemplate::new(template), ObjectId(id), Team::USA)
}

#[test]
fn treads_overlap_uses_ini_geometry_and_kindof_tokens() {
    clear_test_object_visual_ini();
    set_test_object_visual_ini(
        "SlopeTank",
        ObjectVisualIni {
            major_radius: Some(12.0),
            minor_radius: Some(4.0),
            height: Some(6.0),
            geometry: Some("BOX".to_string()),
            kindof: None,
        },
    );
    set_test_object_visual_ini(
        "Bush",
        ObjectVisualIni {
            major_radius: Some(3.0),
            minor_radius: Some(3.0),
            height: Some(2.0),
            geometry: Some("CYLINDER".to_string()),
            kindof: Some("SHRUBBERY STRUCTURE".to_string()),
        },
    );
    set_test_object_visual_ini(
        "Curb",
        ObjectVisualIni {
            major_radius: Some(5.0),
            minor_radius: Some(2.0),
            height: Some(1.0),
            geometry: Some("BOX".to_string()),
            kindof: Some("LOW_OVERLAPPABLE".to_string()),
        },
    );

    let mut tank = test_object(1, "SlopeTank");
    tank.physics_current_overlap = Some(ObjectId(2));
    let bush = test_object(2, "Bush");
    let mut objects = HashMap::new();
    objects.insert(bush.id, bush);
    let body = body_for_object_with_height_samples(&tank, &objects, |_| Some(0.0));
    assert!((body.major_radius - 12.0).abs() < f32::EPSILON);
    assert!((body.minor_radius - 4.0).abs() < f32::EPSILON);
    let overlap = body.current_overlap.expect("bush overlap");
    assert!(overlap.is_shrubbery);
    assert!(!overlap.is_low_overlappable);

    tank.physics_current_overlap = Some(ObjectId(3));
    let curb = test_object(3, "Curb");
    objects.insert(curb.id, curb);
    let body = body_for_object_with_height_samples(&tank, &objects, |_| Some(0.0));
    let overlap = body.current_overlap.expect("curb overlap");
    assert!(!overlap.is_shrubbery);
    assert!(overlap.is_low_overlappable);
    assert!((overlap.bounding_circle_radius - 5.0_f32.hypot(2.0)).abs() < 0.001);

    clear_test_object_visual_ini();
    let missing = test_object(4, "NoDefinition");
    let body = body_for_object_with_height_samples(&missing, &HashMap::new(), |_| None);
    assert_eq!(body.terrain_normal_x, 0.0);
    assert_eq!(body.terrain_normal_y, 0.0);
    assert_eq!(body.terrain_normal_z, 1.0);
    assert!((body.major_radius - missing.selection_radius.max(1.0)).abs() < f32::EPSILON);
}

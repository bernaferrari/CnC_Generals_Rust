//! Host present-path physics visual regressions.

use super::physics_visual_host::{
    FrozenHostPhysicsObject, FrozenHostPhysicsVisuals, HostPhysicsVisualFacts,
    HostPhysicsVisualState, body_for_object_with_height_samples,
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

fn frozen_test_frame(
    state: &HostPhysicsVisualState,
    id: ObjectId,
    facts: HostPhysicsVisualFacts,
    ordinal: u64,
) -> FrozenHostPhysicsVisuals {
    FrozenHostPhysicsVisuals {
        origin: std::sync::Arc::clone(&state.origin),
        ordinal,
        world_epoch: 1,
        frozen: false,
        objects: HashMap::from([(
            id,
            FrozenHostPhysicsObject {
                generation: 1,
                facts,
            },
        )]),
    }
}

#[test]
fn accelerating_unit_pitches_on_host_present() {
    let mut state = HostPhysicsVisualState::default();
    let id = ObjectId(42);
    let mut frame = frozen_test_frame(&state, id, hover_facts(4.0, false), 1);
    let _ = state.local_matrix(
        &frame,
        id,
        &mut game_client::physics_visual::ScriptedClientRng::ints(Vec::new()),
    );
    let loco = state.loco_state(id);
    assert!(
        loco.acceleration_pitch_rate < 0.0,
        "forward accel should kick nose-up pitch rate, got {}",
        loco.acceleration_pitch_rate
    );
}

#[test]
fn stored_loco_state_advances_across_two_frames() {
    let mut state = HostPhysicsVisualState::default();
    let id = ObjectId(43);
    let mut frame = frozen_test_frame(&state, id, hover_facts(4.0, false), 1);
    let _ = state.local_matrix(
        &frame,
        id,
        &mut game_client::physics_visual::ScriptedClientRng::ints(Vec::new()),
    );
    let first = state.loco_state(id);
    frame.ordinal += 1;
    let _ = state.local_matrix(
        &frame,
        id,
        &mut game_client::physics_visual::ScriptedClientRng::ints(Vec::new()),
    );
    let second = state.loco_state(id);
    assert_ne!(first, second);
}

#[test]
fn script_freeze_stops_loco_advancement() {
    let mut state = HostPhysicsVisualState::default();
    let id = ObjectId(44);
    let frame = frozen_test_frame(&state, id, hover_facts(4.0, true), 1);
    let _ = state.local_matrix(
        &frame,
        id,
        &mut game_client::physics_visual::ScriptedClientRng::ints(Vec::new()),
    );
    assert_eq!(state.loco_state(id), PhysicsVisualLocoState::default());
}

#[test]
fn held_gate_skips_host_physics_visual() {
    let mut state = HostPhysicsVisualState::default();
    let id = ObjectId(45);
    let mut facts = hover_facts(4.0, false);
    facts.object_disabled_held = true;
    let frame = frozen_test_frame(&state, id, facts, 1);
    let _ = state.local_matrix(
        &frame,
        id,
        &mut game_client::physics_visual::ScriptedClientRng::ints(Vec::new()),
    );
    assert_eq!(state.loco_state(id), PhysicsVisualLocoState::default());
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

#[test]
fn same_id_worlds_and_rebuilt_same_logic_frame_keep_independent_loco() {
    let id = ObjectId(42);
    let mut a = HostPhysicsVisualState::default();
    let mut b = HostPhysicsVisualState::default();
    let a1 = frozen_test_frame(&a, id, hover_facts(4.0, false), 1);
    let b1 = frozen_test_frame(&b, id, hover_facts(-9.0, false), 1);
    let mut rng = game_client::physics_visual::ScriptedClientRng::ints(Vec::new());
    a.local_matrix(&a1, id, &mut rng);
    let first = a.loco_state(id);
    let foreign = a.local_matrix(&b1, id, &mut rng);
    assert_eq!(foreign, None);
    assert_eq!(a.loco_state(id), first);
    b.local_matrix(&b1, id, &mut rng);
    let mut a2 = a1.clone();
    a2.ordinal = 2;
    a.local_matrix(&a2, id, &mut rng);
    let mut isolated = HostPhysicsVisualState::default();
    let i1 = frozen_test_frame(&isolated, id, hover_facts(4.0, false), 1);
    isolated.local_matrix(&i1, id, &mut rng);
    let mut i2 = i1.clone();
    i2.ordinal = 2;
    isolated.local_matrix(&i2, id, &mut rng);
    assert_eq!(a.loco_state(id), isolated.loco_state(id));
    let second = a.loco_state(id);
    assert_eq!(a.local_matrix(&a1, id, &mut rng), None);
    assert_eq!(a.loco_state(id), second);
    a = HostPhysicsVisualState::default();
    assert_eq!(a.local_matrix(&a2, id, &mut rng), None);
    assert_eq!(a.loco_state(id), PhysicsVisualLocoState::default());
}

#[test]
fn completed_inputs_are_inert_and_actual_admission_restore_reset_reject_stale_frames() {
    use crate::game_logic::{GameLogic, LocomotorAppearance};
    use crate::presentation_frame::{PresentationFrame, UnitRenderInput};
    use crate::save_load::SnapshotBuilder;
    let mut world = GameLogic::new();
    let id = ObjectId(77);
    let mut object = test_object(77, "OwnerHover");
    object.loco_appearance = LocomotorAppearance::Hover;
    world.add_object(object);
    let original_generation = world.host_object(id).unwrap().visual_object_generation;
    let mut frame = PresentationFrame::build_from_logic(&world, 1);
    frame
        .host_physics_visuals
        .objects
        .get_mut(&id)
        .unwrap()
        .facts = hover_facts(4.0, false);
    let mut input = UnitRenderInput::from_renderable(
        frame.objects.iter().find(|object| object.id == id).unwrap(),
    );
    world
        .host_physics_visuals
        .borrow_mut()
        .complete_input(&mut input, &frame);
    let matrix = input.world_matrix();
    let first = world.host_physics_visuals.borrow().loco_state(id);
    let _candidate = GameLogic::new();
    world.host_object_mut(id).unwrap().physics_previous_accel = Vec3::splat(999.0);
    world
        .host_physics_visuals
        .borrow_mut()
        .complete_input(&mut input, &frame);
    assert_eq!(input.world_matrix(), matrix);
    assert_eq!(world.host_physics_visuals.borrow().loco_state(id), first);
    let mut moved = input.clone();
    moved.position.x += 50.0;
    assert_ne!(
        moved.world_matrix(),
        matrix,
        "cache retains local transform, not old world position"
    );
    assert_eq!(world.host_physics_visuals.borrow().loco_state(id), first);
    let cloned = world.host_object(id).unwrap().clone();
    world.add_object(cloned);
    let replacement_generation = world.host_object(id).unwrap().visual_object_generation;
    assert_ne!(replacement_generation, original_generation);
    let mut replacement = PresentationFrame::build_from_logic(&world, 1);
    replacement
        .host_physics_visuals
        .objects
        .get_mut(&id)
        .unwrap()
        .facts = hover_facts(4.0, false);
    let mut new_input = UnitRenderInput::from_renderable(
        replacement
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap(),
    );
    world
        .host_physics_visuals
        .borrow_mut()
        .complete_input(&mut new_input, &replacement);
    assert_eq!(
        world.host_physics_visuals.borrow().loco_state(id),
        first,
        "replacement starts a fresh loco"
    );
    assert_eq!(input.world_matrix(), matrix);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut world)
        .unwrap();
    let restored_generation = world.host_object(id).unwrap().visual_object_generation;
    assert_ne!(restored_generation, replacement_generation);
    let mut stale = new_input.clone();
    world
        .host_physics_visuals
        .borrow_mut()
        .complete_input(&mut stale, &replacement);
    assert_eq!(stale.physics_visual_local, None);
    assert_eq!(
        world.host_physics_visuals.borrow().loco_state(id),
        PhysicsVisualLocoState::default()
    );
    world.reset();
    world
        .host_physics_visuals
        .borrow_mut()
        .complete_input(&mut stale, &frame);
    assert_eq!(stale.physics_visual_local, None);
    assert_eq!(input.world_matrix(), matrix);
}

#[derive(Default)]
struct CountingRng {
    draws: Vec<i32>,
}
impl game_client::physics_visual::ClientVisualRng for CountingRng {
    fn random_int(&mut self, lo: i32, hi: i32) -> i32 {
        let next = (self.draws.len() as i32).clamp(lo, hi);
        self.draws.push(next);
        next
    }
    fn random_real(&mut self, lo: f32, _hi: f32) -> f32 {
        self.draws.push(-1);
        lo
    }
}

#[test]
fn completion_order_repeat_freeze_and_generation_prune_preserve_rng() {
    let mut state = HostPhysicsVisualState::default();
    let mut facts = hover_facts(0.0, false);
    facts.appearance = PhysicsVisualAppearance::WheelsFour;
    facts.params.bounce_kick = 4.0;
    facts.body.velocity_magnitude = 10.0;
    facts.body.cur_locomotor_speed = 10.0;
    let mut frame = frozen_test_frame(&state, ObjectId(1), facts, 1);
    frame.objects.insert(
        ObjectId(2),
        FrozenHostPhysicsObject {
            generation: 1,
            facts,
        },
    );
    let mut rng = CountingRng::default();
    // Existing collector order is driving order, not HashMap order.
    state.local_matrix(&frame, ObjectId(2), &mut rng);
    state.local_matrix(&frame, ObjectId(1), &mut rng);
    assert_eq!(rng.draws, [0, 1]);
    assert!(state.loco_state(ObjectId(2)).pitch_rate < 0.0);
    assert!(state.loco_state(ObjectId(1)).pitch_rate > 0.0);
    state.local_matrix(&frame, ObjectId(2), &mut rng);
    assert_eq!(rng.draws, [0, 1]);
    let before = state.loco_state(ObjectId(2));
    frame.ordinal = 2;
    frame.frozen = true;
    state.local_matrix(&frame, ObjectId(2), &mut rng);
    assert_eq!(rng.draws, [0, 1]);
    assert_eq!(state.loco_state(ObjectId(2)), before);
    let empty_world = crate::game_logic::GameLogic::new();
    let _ = state.freeze(&empty_world);
    assert_eq!(
        state.loco_state(ObjectId(2)),
        PhysicsVisualLocoState::default()
    );
}

#[test]
fn both_creation_paths_admit_fresh_visual_identity_and_live_reinsertion_retains_it() {
    let mut world = crate::game_logic::GameLogic::new();
    world
        .templates
        .insert("AdmissionUnit".into(), ThingTemplate::new("AdmissionUnit"));
    let first = world
        .create_object("AdmissionUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    let second = world
        .create_object_under_construction("AdmissionUnit", Team::USA, Vec3::ZERO)
        .unwrap();
    let first_generation = world.host_object(first).unwrap().visual_object_generation;
    let second_generation = world.host_object(second).unwrap().visual_object_generation;
    assert_ne!(first_generation, 0);
    assert_ne!(second_generation, 0);
    assert_ne!(first_generation, second_generation);
    // Physics crush handling temporarily extracts and returns an existing
    // object. Returning that live object must not be treated as admission.
    let live_object = world.objects.remove(&first).unwrap();
    world.objects.insert(first, live_object);
    assert_eq!(
        world.host_object(first).unwrap().visual_object_generation,
        first_generation
    );
}

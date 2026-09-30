use super::*;
use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use game_client::core::PresentationSpecializedDrawKind;

fn frame_and_input() -> (Rc<PresentationFrame>, UnitRenderInput) {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("SpecializedBundleTank");
    template.add_kind_of(KindOf::Vehicle);
    logic.templates.insert(template.name.clone(), template);
    logic
        .create_object("SpecializedBundleTank", Team::USA, Vec3::ZERO)
        .unwrap();
    let frame = Rc::new(PresentationFrame::build_from_logic(&logic, 0));
    let input = UnitRenderInput::from_renderable(&frame.objects[0]);
    (frame, input)
}

fn snapshot(id: ObjectID, tread_uv: f32) -> PresentationSpecializedDrawSnapshot {
    PresentationSpecializedDrawSnapshot {
        kind: PresentationSpecializedDrawKind::Tank,
        module_name: "W3DTankDraw".into(),
        object_id: id.0,
        tread_uv,
        wheel_angle: 0.0,
        laser_width: 0.5,
        debris_state: 0,
        debris_anim_time: 0.0,
        model_name: "AnExactAuthoredModelNameWithAHeapAllocation".into(),
        science_hidden: false,
    }
}

#[test]
fn fresh_seed_and_exact_shared_frame_keep_bundle_without_copying_frame() {
    let (frame, mut input) = frame_and_input();
    let state = snapshot(input.id, 0.25);
    let identity = Rc::as_ptr(&frame);
    let strong_count = Rc::strong_count(&frame);
    // First-frame seeding captures before the pipeline installs its frame.
    let mut bundle = Some(FrozenSpecializedDrawFrame::capture(
        &frame,
        1,
        [(input.id.0, &state)],
    ));
    assert_eq!(Rc::strong_count(&frame), strong_count);
    assert_eq!(Rc::as_ptr(&frame), identity);
    assert_eq!(Rc::weak_count(&frame), 1);
    FrozenSpecializedDrawFrame::retain_for_frame(&mut bundle, Some(&frame));
    let same = Rc::clone(&frame);
    FrozenSpecializedDrawFrame::retain_for_frame(&mut bundle, Some(&same));
    assert!(bundle.is_some());
    assert_eq!(
        bundle
            .as_ref()
            .unwrap()
            .apply_to_inputs(&same, std::slice::from_mut(&mut input)),
        1
    );
    assert_eq!(input.specialized_draw.as_ref().unwrap().tread_uv, 0.25);
    assert_eq!(Rc::as_ptr(&frame), identity);
    assert_eq!(Rc::strong_count(&frame), strong_count + 1);
}

#[test]
fn replacement_even_at_same_logic_frame_and_none_clear_bundle() {
    let (frame, input) = frame_and_input();
    let state = snapshot(input.id, 0.25);
    let mut bundle = Some(FrozenSpecializedDrawFrame::capture(
        &frame,
        1,
        [(input.id.0, &state)],
    ));
    let replacement = Rc::new((*frame).clone());
    assert_eq!(replacement.frame, frame.frame);
    assert!(!Rc::ptr_eq(&replacement, &frame));
    FrozenSpecializedDrawFrame::retain_for_frame(&mut bundle, Some(&replacement));
    assert!(bundle.is_none());
    bundle = Some(FrozenSpecializedDrawFrame::capture(
        &frame,
        1,
        [(input.id.0, &state)],
    ));
    FrozenSpecializedDrawFrame::retain_for_frame(&mut bundle, None);
    assert!(bundle.is_none());
}

#[test]
fn one_lookup_per_object_shares_names_across_all_meshes_and_freezes_old_inputs() {
    let (frame, input) = frame_and_input();
    let old_state = snapshot(input.id, 0.25);
    let bundle = FrozenSpecializedDrawFrame::capture(&frame, 1, [(input.id.0, &old_state)]);
    let mut inputs = vec![input.clone(), input.clone(), input];
    inputs[1].id = ObjectID(99_999);
    assert_eq!(bundle.apply_to_inputs(&frame, &mut inputs), 3);
    let shared = bundle.snapshots.get(&inputs[0].id).unwrap();
    assert!(Rc::ptr_eq(
        inputs[0].specialized_draw.as_ref().unwrap(),
        shared
    ));
    assert!(Rc::ptr_eq(
        inputs[2].specialized_draw.as_ref().unwrap(),
        shared
    ));
    assert!(inputs[1].specialized_draw.is_none());
    let model_pointer = shared.model_name.as_ptr();
    let module_pointer = shared.module_name.as_ptr();
    let count = Rc::strong_count(shared);
    for _ in 0..100 {
        let projected = inputs[0].specialized_draw.as_deref().unwrap();
        assert_eq!(
            projected.tread_uv_for_mesh("Tank.TREADSL"),
            Some([0.25, 0.0])
        );
        assert_eq!(
            projected.tread_uv_for_mesh("Tank.TREADSR"),
            Some([0.75, 0.0])
        );
        assert_eq!(projected.model_name.as_ptr(), model_pointer);
        assert_eq!(projected.module_name.as_ptr(), module_pointer);
    }
    assert_eq!(Rc::strong_count(shared), count);
    let new_state = snapshot(inputs[0].id, 0.5);
    let next = FrozenSpecializedDrawFrame::capture(&frame, 1, [(inputs[0].id.0, &new_state)]);
    let mut next_inputs = vec![inputs[0].clone()];
    next.apply_to_inputs(&frame, &mut next_inputs);
    assert_eq!(
        next_inputs[0].specialized_draw.as_ref().unwrap().tread_uv,
        0.5
    );
    assert_eq!(inputs[0].specialized_draw.as_ref().unwrap().tread_uv, 0.25);
    let other_frame = Rc::new((*frame).clone());
    assert_eq!(next.apply_to_inputs(&other_frame, &mut next_inputs), 0);
    assert!(next_inputs[0].specialized_draw.is_none());
}

#[test]
fn collector_joins_completed_bundle_before_meshes_and_never_queries_live_snapshot() {
    let source = include_str!("pipeline_collect.rs");
    let join = source
        .find("bundle.apply_to_inputs(frame, &mut unit_inputs)")
        .unwrap();
    let object_loop = source.find("for mut u in unit_inputs").unwrap();
    let mesh_loop = source.find("for mesh in").unwrap();
    assert!(join < object_loop && object_loop < mesh_loop);
    assert!(source.contains("let specialized_draw = u.specialized_draw.as_deref()"));
    assert!(!source.contains("presentation_specialized_draw_snapshot("));
    let capture = include_str!("../../cnc_game_engine/camera_drain.rs");
    let sync = capture
        .find("sync_presentation_drawables(sync_entries)")
        .unwrap();
    let capture = capture.find(".capture_specialized_draw_inputs(").unwrap();
    assert!(sync < capture);
}

#[test]
fn paused_selection_frame_fork_recaptures_once_and_same_frame_never_reads_source() {
    let (mut frame, input) = frame_and_input();
    let state = snapshot(input.id, 0.25);
    let captures = std::cell::Cell::new(0);
    let mut bundle = None;
    assert!(FrozenSpecializedDrawFrame::ensure_for_frame(
        &mut bundle,
        &frame,
        1,
        || {
            captures.set(captures.get() + 1);
            std::iter::once((input.id.0, &state))
        },
    ));
    let mut old_inputs = vec![input.clone()];
    bundle
        .as_ref()
        .unwrap()
        .apply_to_inputs(&frame, &mut old_inputs);
    let installed_pipeline_frame = Rc::clone(&frame);
    let old_identity = Rc::as_ptr(&frame);
    // Actual selection path changes its immutable host frame while paused.
    Rc::make_mut(&mut frame).selected.push(input.id);
    assert_ne!(Rc::as_ptr(&frame), old_identity);
    assert_eq!(frame.frame, installed_pipeline_frame.frame);
    FrozenSpecializedDrawFrame::retain_for_frame(&mut bundle, Some(&frame));
    assert!(bundle.is_none());
    assert!(FrozenSpecializedDrawFrame::ensure_for_frame(
        &mut bundle,
        &frame,
        1,
        || {
            captures.set(captures.get() + 1);
            std::iter::once((input.id.0, &state))
        },
    ));
    let mut inputs = vec![input];
    bundle
        .as_ref()
        .unwrap()
        .apply_to_inputs(&frame, &mut inputs);
    assert_eq!(inputs[0].specialized_draw.as_ref().unwrap().tread_uv, 0.25);
    assert_eq!(
        old_inputs[0].specialized_draw.as_ref().unwrap().tread_uv,
        0.25
    );
    let retained_pointer = Rc::as_ptr(inputs[0].specialized_draw.as_ref().unwrap());
    for _ in 0..100 {
        assert!(!FrozenSpecializedDrawFrame::ensure_for_frame(
            &mut bundle,
            &frame,
            1,
            || {
                captures.set(captures.get() + 1);
                std::iter::once((inputs[0].id.0, &state))
            },
        ));
    }
    assert_eq!(captures.get(), 2);
    assert_eq!(
        Rc::as_ptr(
            bundle
                .as_ref()
                .unwrap()
                .snapshots
                .get(&inputs[0].id)
                .unwrap()
        ),
        retained_pointer
    );
    // A new world epoch forces recapture even if a caller reused the frame Rc.
    assert!(FrozenSpecializedDrawFrame::ensure_for_frame(
        &mut bundle,
        &frame,
        2,
        || std::iter::empty(),
    ));
    bundle
        .as_ref()
        .unwrap()
        .apply_to_inputs(&frame, &mut inputs);
    assert!(inputs[0].specialized_draw.is_none());
}

#[test]
fn capture_keeps_normal_fallback_and_resident_rubble_but_not_absent_or_nonresident_ids() {
    let (frame, input) = frame_and_input();
    let mut owned = (*frame).clone();
    owned.direct_host_drawables[0].resident = false;
    let mut rubble = owned.direct_host_drawables[0].clone();
    rubble.object.id = ObjectID(70_001);
    rubble.object.destroyed = true;
    rubble.resident = true;
    let mut removed = rubble.clone();
    removed.object.id = ObjectID(70_002);
    removed.resident = false;
    owned.direct_host_drawables.extend([rubble, removed]);
    let frame = Rc::new(owned);
    let normal_state = snapshot(input.id, 0.25);
    let rubble_state = snapshot(ObjectID(70_001), 0.5);
    let removed_state = snapshot(ObjectID(70_002), 0.75);
    let stale_state = snapshot(ObjectID(70_003), 0.9);
    let bundle = FrozenSpecializedDrawFrame::capture(
        &frame,
        1,
        [
            (input.id.0, &normal_state),
            (70_001, &rubble_state),
            (70_002, &removed_state),
            (70_003, &stale_state),
        ],
    );
    assert_eq!(bundle.snapshots.len(), 2);
    assert!(bundle.snapshots.contains_key(&input.id));
    assert!(bundle.snapshots.contains_key(&ObjectID(70_001)));
    assert!(!bundle.snapshots.contains_key(&ObjectID(70_002)));
    assert!(!bundle.snapshots.contains_key(&ObjectID(70_003)));
}

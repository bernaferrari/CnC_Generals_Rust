use super::*;
fn actor() -> Object {
    let mut template = ThingTemplate::new("PivotOwnerVehicle");
    template.add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, ObjectId(71), Team::USA);
    object.selection_radius = 2.0;
    object.set_position(glam::Vec3::new(20.0, 3.0, 20.0));
    object.turn_pivot_offset = 1.0;
    object
}
#[test]
fn pivot_full_transform_preserves_pitch_roll_scale_and_rotates_about_offset() {
    let mut object = actor();
    let original = glam::Mat4::from_translation(glam::Vec3::new(20.0, 3.0, 20.0))
        * glam::Mat4::from_rotation_x(0.4)
        * glam::Mat4::from_scale(glam::Vec3::new(2.0, 3.0, 4.0));
    object.set_transform_matrix(original);
    let (_, relative) = object
        .rotate_obj_around_loco_pivot(glam::Vec3::new(22.0, 3.0, 0.0), std::f32::consts::FRAC_PI_2);
    let result = object.get_transform_matrix();
    assert!((relative - std::f32::consts::FRAC_PI_2).abs() < 0.0001);
    assert!((result.w_axis.x - 22.0).abs() < 0.0001);
    assert!((result.w_axis.z - 22.0).abs() < 0.0001);
    assert!((result.w_axis.y - 3.0).abs() < 0.0001);
    assert!((result.x_axis.truncate().length() - 2.0).abs() < 0.0001);
    assert!((result.y_axis.truncate().length() - 3.0).abs() < 0.0001);
    assert!((result.z_axis.truncate().length() - 4.0).abs() < 0.0001);
    assert!(
        (result.y_axis.y - original.y_axis.y).abs() < 0.0001,
        "pitch/roll is not flattened by separate yaw setter"
    );
}
#[test]
fn zero_pivot_coincident_goal_still_turns_toward_cpp_atan2_zero() {
    let mut object = actor();
    object.turn_pivot_offset = 0.0;
    object.set_orientation(0.6);
    let position = object.get_position();
    let (turning, relative) = object.rotate_obj_around_loco_pivot(position, 0.2);
    assert_eq!(turning, PhysicsTurningType::TurnNegative);
    assert!((relative + 0.6).abs() < 0.0001);
    assert!((object.get_orientation() - 0.4).abs() < 0.0001);
}
#[cfg(feature = "game_client")]
#[test]
fn actual_receiving_pivot_installs_new_heading_before_tree_push_callback() {
    let mut world = crate::game_logic::GameLogic::new();
    let object = actor();
    let id = object.id;
    world.objects.insert(id, object);
    let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
    visual
        .tree_buffer_mut()
        .set_bounds(game_client::terrain::TreeRegion2D::new(
            glam::Vec2::ZERO,
            glam::Vec2::splat(100.0),
        ));
    let data = game_client::terrain::TreeModuleData {
        model_name: "PivotGrass".into(),
        frames_to_move_outward: 4,
        frames_to_move_inward: 4,
        do_topple: false,
        ..Default::default()
    };
    visual
        .tree_buffer_mut()
        .add_tree(
            900,
            glam::Vec3::new(23.0, 22.0, 3.0),
            1.0,
            0.0,
            1.0,
            data,
            game_client::terrain::TreeSphere {
                center: glam::Vec3::ZERO,
                radius: 2.0,
            },
        )
        .unwrap();
    assert!(
        world
            .rotate_owned_pivot_with_effects(
                id,
                glam::Vec3::new(22.0, 3.0, 0.0),
                std::f32::consts::FRAC_PI_2,
                13,
                &mut crate::game_logic::ScriptPresentationEffects {
                    terrain: Some(&mut visual),
                    ..Default::default()
                }
            )
            .is_some()
    );
    let tree = &visual.tree_buffer_mut().trees()[0];
    assert!((tree.push_aside_cos - 1.0).abs() < 0.0001);
    assert!(
        tree.push_aside_sin.abs() < 0.0001,
        "tree observed new heading, not old yaw"
    );
    assert_ne!(tree.push_aside_delta, 0.0);
    let object = world.objects.get(&id).unwrap();
    assert!((object.get_position().x - 22.0).abs() < 0.0001);
    assert!((object.get_position().z - 22.0).abs() < 0.0001);
}

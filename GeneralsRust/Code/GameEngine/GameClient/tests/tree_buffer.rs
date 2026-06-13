use game_client_rust::terrain::{
    BreezeInfo, TreeCollisionUnit, TreeConstructionGeometry, TreeFxKind, TreeGeometryType,
    TreeModuleData, TreeRandom, TreeRegion2D, TreeSaveRecord, TreeShroudStatus, TreeSphere,
    W3DToppleState, W3DTreeBuffer, ANGULAR_LIMIT, CONSTRUCTION_TREE_COLLISION_RADIUS,
    DELETED_TREE_TYPE, END_OF_PARTITION, MAX_TREES, MAX_TYPES, PARTITION_WIDTH_HEIGHT,
    TREE_RADIUS_APPROX, W3D_TOPPLE_OPTIONS_NO_BOUNCE, W3D_TOPPLE_OPTIONS_NO_FX,
};
use glam::{Mat4, Vec2, Vec3, Vec4};

fn module(model: &str, texture: &str) -> TreeModuleData {
    TreeModuleData {
        model_name: model.to_string(),
        texture_name: texture.to_string(),
        frames_to_move_outward: 4,
        frames_to_move_inward: 2,
        max_outward_movement: 1.0,
        do_topple: true,
        ..TreeModuleData::default()
    }
}

fn bounds() -> TreeSphere {
    TreeSphere {
        center: Vec3::new(1.0, 2.0, 3.0),
        radius: 5.0,
    }
}

fn approx_eq(a: f32, b: f32) {
    assert!((a - b).abs() < 0.001, "{a} != {b}");
}

fn approx_mat4_eq(a: Mat4, b: Mat4) {
    for (left, right) in a.to_cols_array().iter().zip(b.to_cols_array()) {
        approx_eq(*left, right);
    }
}

#[test]
fn constants_and_clear_match_cpp_shape() {
    let mut buffer = W3DTreeBuffer::new();
    assert_eq!(MAX_TREES, 4000);
    assert_eq!(MAX_TYPES, 64);
    assert_eq!(PARTITION_WIDTH_HEIGHT, 100);
    assert_eq!(buffer.bounds(), TreeRegion2D::default());
    assert_eq!(buffer.area_partition()[0], END_OF_PARTITION);

    buffer.set_bounds(TreeRegion2D::new(
        Vec2::new(0.0, 0.0),
        Vec2::new(200.0, 200.0),
    ));
    buffer
        .add_tree(
            7,
            Vec3::new(20.0, 30.0, 4.0),
            2.0,
            0.5,
            1.0,
            module("Oak", "OakT"),
            bounds(),
        )
        .unwrap();
    buffer.clear_all_trees();

    assert!(buffer.trees().is_empty());
    assert!(buffer.tree_types().is_empty());
    assert_eq!(buffer.bounds(), TreeRegion2D::default());
    assert!(buffer
        .area_partition()
        .iter()
        .all(|&v| v == END_OF_PARTITION));
}

#[test]
fn partition_bucket_preserves_cpp_formula() {
    let mut buffer = W3DTreeBuffer::new();
    buffer.set_bounds(TreeRegion2D::new(
        Vec2::new(10.0, 20.0),
        Vec2::new(110.0, 220.0),
    ));

    assert_eq!(buffer.get_partition_bucket(Vec3::new(-5.0, -5.0, 0.0)), 909);
    assert_eq!(
        buffer.get_partition_bucket(Vec3::new(110.0, 220.0, 0.0)),
        11_009
    );
}

#[test]
fn add_tree_dedupes_type_and_partitions_pushable_or_topple_trees() {
    let mut buffer = W3DTreeBuffer::new();
    buffer.set_bounds(TreeRegion2D::new(Vec2::ZERO, Vec2::new(100.0, 100.0)));
    let data = module("Oak", "OakTexture");

    let first = buffer
        .add_tree(
            1,
            Vec3::new(10.0, 20.0, 3.0),
            2.0,
            0.5,
            1.0,
            data.clone(),
            bounds(),
        )
        .unwrap();
    let second = buffer
        .add_tree(2, Vec3::new(15.0, 25.0, 4.0), 1.0, 1.0, 1.0, data, bounds())
        .unwrap();

    assert_eq!(buffer.tree_types().len(), 1);
    assert_eq!(buffer.trees()[first].tree_type, 0);
    assert_eq!(buffer.trees()[second].tree_type, 0);
    approx_eq(buffer.trees()[first].sin, 0.5f32.sin());
    approx_eq(buffer.trees()[first].cos, 0.5f32.cos());
    approx_eq(buffer.trees()[first].bounds.center.x, 12.0);
    approx_eq(buffer.trees()[first].bounds.radius, 10.0);
    let bucket = buffer.get_partition_bucket(Vec3::new(10.0, 20.0, 3.0)) as usize;
    assert_eq!(buffer.area_partition()[bucket], first as i16);
}

#[test]
fn tree_type_table_full_falls_back_to_type_zero() {
    let mut buffer = W3DTreeBuffer::new();

    for i in 0..MAX_TYPES {
        let model = format!("Tree{i}");
        let texture = format!("TreeTexture{i}");
        let index = buffer
            .add_tree(
                i as u32,
                Vec3::new(i as f32, 0.0, 0.0),
                1.0,
                0.0,
                0.0,
                module(&model, &texture),
                bounds(),
            )
            .unwrap();
        assert_eq!(index, i);
    }

    let overflow = buffer
        .add_tree(
            999,
            Vec3::new(99.0, 0.0, 0.0),
            1.0,
            0.0,
            0.0,
            module("OverflowTree", "OverflowTexture"),
            bounds(),
        )
        .unwrap();

    assert_eq!(overflow, MAX_TYPES);
    assert_eq!(buffer.trees()[overflow].tree_type, 0);
    assert_eq!(buffer.tree_types().len(), MAX_TYPES);
}

#[test]
fn remove_and_update_tree_match_cpp_side_effects() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            10,
            Vec3::new(1.0, 2.0, 3.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();

    assert!(buffer.update_tree_position(10, Vec3::new(5.0, 6.0, 7.0), 1.0));
    assert_eq!(buffer.trees()[id].location, Vec3::new(5.0, 6.0, 7.0));
    assert!(buffer.anything_changed());

    buffer.remove_tree(10);
    assert_eq!(buffer.trees()[id].tree_type, DELETED_TREE_TYPE);
    assert_eq!(buffer.trees()[id].location, Vec3::ZERO);
    assert_eq!(buffer.trees()[id].bounds.radius, 1.0);
}

#[test]
fn push_aside_uses_cpp_side_direction_and_three_frame_suppression() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            10,
            Vec3::new(10.0, 10.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();

    buffer.push_aside_tree(10, Vec3::new(10.0, 0.0, 0.0), Vec2::X, 99, 10);
    assert!(buffer.any_push_changed());
    approx_eq(buffer.trees()[id].push_aside_cos, 0.0);
    approx_eq(buffer.trees()[id].push_aside_sin, 1.0);
    approx_eq(buffer.trees()[id].push_aside_delta, 0.25);

    buffer.tree_mut(id).unwrap().push_aside_delta = 0.0;
    buffer.push_aside_tree(10, Vec3::new(10.0, 0.0, 0.0), Vec2::X, 99, 12);
    assert_eq!(buffer.trees()[id].last_frame_updated, 12);
    approx_eq(buffer.trees()[id].push_aside_delta, 0.0);
}

#[test]
fn unit_moved_topples_or_pushes_partitioned_trees() {
    let mut buffer = W3DTreeBuffer::new();
    buffer.set_bounds(TreeRegion2D::new(Vec2::ZERO, Vec2::new(100.0, 100.0)));
    let toppler = buffer
        .add_tree(
            1,
            Vec3::new(10.0, 10.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Topple", "T"),
            bounds(),
        )
        .unwrap();
    let mut push_data = module("Push", "T");
    push_data.do_topple = false;
    let pusher = buffer
        .add_tree(
            2,
            Vec3::new(20.0, 10.0, 0.0),
            1.0,
            0.0,
            1.0,
            push_data,
            bounds(),
        )
        .unwrap();

    buffer.unit_moved(
        TreeCollisionUnit {
            object_id: 100,
            position: Vec3::new(10.0, 10.0, 0.0),
            direction_2d: Vec2::X,
            major_radius: 1.0,
            minor_radius: 1.0,
            geometry_type: TreeGeometryType::Cylinder,
            crusher_level: 2,
            immobile: false,
        },
        20,
    );
    assert_eq!(
        buffer.trees()[toppler].topple_state,
        W3DToppleState::Falling
    );

    buffer.unit_moved(
        TreeCollisionUnit {
            object_id: 101,
            position: Vec3::new(20.0, 10.0, 0.0),
            direction_2d: Vec2::X,
            major_radius: 1.0,
            minor_radius: 1.0,
            geometry_type: TreeGeometryType::Cylinder,
            crusher_level: 1,
            immobile: false,
        },
        21,
    );
    assert!(buffer.trees()[pusher].push_aside_delta > 0.0);
}

#[test]
fn toppling_force_clamps_speed_and_no_bounce_sets_down() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            5,
            Vec3::new(0.0, 0.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();

    assert!(buffer.apply_toppling_force(5, Vec3::X, 0.0, W3D_TOPPLE_OPTIONS_NO_BOUNCE));
    approx_eq(buffer.trees()[id].angular_velocity, 0.1);
    approx_eq(buffer.trees()[id].angular_acceleration, 0.005);
    assert_eq!(buffer.trees()[id].topple_state, W3DToppleState::Falling);

    buffer.tree_mut(id).unwrap().angular_accumulation = ANGULAR_LIMIT - 0.01;
    buffer.update_toppling_tree(id, TreeShroudStatus::Clear);
    assert_eq!(buffer.trees()[id].topple_state, W3DToppleState::Down);
    assert_eq!(buffer.trees()[id].sink_frames_left, 300);
}

#[test]
fn toppling_force_emits_topple_fx_even_when_no_fx_option_is_set() {
    let mut buffer = W3DTreeBuffer::new();
    let mut data = module("Oak", "T");
    data.topple_fx = Some("TreeTopple".to_string());
    buffer
        .add_tree(5, Vec3::new(2.0, 3.0, 4.0), 1.0, 0.0, 1.0, data, bounds())
        .unwrap();

    assert!(buffer.apply_toppling_force(5, Vec3::X, 1.0, W3D_TOPPLE_OPTIONS_NO_FX));

    let events = buffer.take_pending_fx_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, TreeFxKind::Topple);
    assert_eq!(events[0].fx_name, "TreeTopple");
    assert_eq!(events[0].position, Vec3::new(2.0, 3.0, 4.0));
}

#[test]
fn toppling_update_emits_bounce_fx_unless_no_fx_option_is_set() {
    let mut buffer = W3DTreeBuffer::new();
    let mut data = module("Oak", "T");
    data.bounce_fx = Some("TreeBounce".to_string());
    let id = buffer
        .add_tree(5, Vec3::ZERO, 1.0, 0.0, 1.0, data, bounds())
        .unwrap();
    buffer.apply_toppling_force(5, Vec3::X, 1.0, 0);
    buffer.take_pending_fx_events();
    buffer.tree_mut(id).unwrap().angular_accumulation = ANGULAR_LIMIT - 0.01;

    buffer.update_toppling_tree(id, TreeShroudStatus::Clear);

    let events = buffer.take_pending_fx_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, TreeFxKind::Bounce);
    assert_eq!(events[0].fx_name, "TreeBounce");

    let mut suppressed = W3DTreeBuffer::new();
    let mut data = module("Pine", "T");
    data.bounce_fx = Some("TreeBounce".to_string());
    let id = suppressed
        .add_tree(6, Vec3::ZERO, 1.0, 0.0, 1.0, data, bounds())
        .unwrap();
    suppressed.apply_toppling_force(6, Vec3::X, 1.0, W3D_TOPPLE_OPTIONS_NO_FX);
    suppressed.take_pending_fx_events();
    suppressed.tree_mut(id).unwrap().angular_accumulation = ANGULAR_LIMIT - 0.01;

    suppressed.update_toppling_tree(id, TreeShroudStatus::Clear);

    assert!(suppressed.take_pending_fx_events().is_empty());
}

#[test]
fn toppling_update_pre_rotates_matrix_like_cpp() {
    let mut buffer = W3DTreeBuffer::new();
    let location = Vec3::new(3.0, 4.0, 5.0);
    let id = buffer
        .add_tree(5, location, 1.0, 0.0, 1.0, module("Oak", "T"), bounds())
        .unwrap();

    buffer.apply_toppling_force(5, Vec3::X, 0.0, 0);
    buffer.update_toppling_tree(id, TreeShroudStatus::Clear);

    let expected = Mat4::from_rotation_y(0.1) * Mat4::from_translation(location);
    approx_mat4_eq(buffer.trees()[id].matrix, expected);
}

#[test]
fn fogged_topple_freezes_then_resolves_to_down() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(5, Vec3::ZERO, 1.0, 0.0, 1.0, module("Oak", "T"), bounds())
        .unwrap();
    buffer.apply_toppling_force(5, Vec3::X, 1.0, 0);

    buffer.update_toppling_tree(id, TreeShroudStatus::Fogged);
    assert_eq!(buffer.trees()[id].topple_state, W3DToppleState::Fogged);

    buffer.update_toppling_tree(id, TreeShroudStatus::Clear);
    assert_eq!(buffer.trees()[id].topple_state, W3DToppleState::Down);
    assert_eq!(buffer.trees()[id].sink_frames_left, 0);
    approx_mat4_eq(
        buffer.trees()[id].matrix,
        Mat4::from_rotation_y(ANGULAR_LIMIT) * Mat4::from_translation(Vec3::ZERO),
    );
}

#[test]
fn terrain_pass_flag_is_consumed_by_cpu_tick() {
    let mut buffer = W3DTreeBuffer::new();
    assert!(!buffer.need_to_draw());
    buffer.set_is_terrain();
    assert!(buffer.need_to_draw());
    buffer.tick_cpu(false, |_| TreeShroudStatus::Clear);
    assert!(!buffer.need_to_draw());
}

#[test]
fn down_tree_with_zero_sink_frames_wraps_and_still_sinks_like_cpp() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            8,
            Vec3::new(1.0, 2.0, 3.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    {
        let tree = buffer.tree_mut(id).unwrap();
        tree.topple_state = W3DToppleState::Down;
        tree.sink_frames_left = 0;
        tree.matrix = Mat4::from_translation(tree.location);
    }

    buffer.tick_cpu(false, |_| TreeShroudStatus::Clear);

    assert_eq!(buffer.trees()[id].tree_type, DELETED_TREE_TYPE);
    assert_eq!(buffer.trees()[id].sink_frames_left, u32::MAX);
    approx_eq(buffer.trees()[id].location.z, 3.0 - 20.0 / 300.0);
    approx_eq(
        buffer.trees()[id].matrix.w_axis.z,
        buffer.trees()[id].location.z,
    );
}

#[test]
fn save_records_follow_cpp_xfer_order_for_existing_and_deleted_trees() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            9,
            Vec3::new(1.0, 2.0, 3.0),
            1.5,
            0.0,
            1.0,
            module("Oak", "Leaf"),
            bounds(),
        )
        .unwrap();
    buffer.apply_toppling_force(9, Vec3::Y, 2.0, W3D_TOPPLE_OPTIONS_NO_BOUNCE);
    buffer.remove_tree(9);

    let records = buffer.save_records();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].model_name, "");
    assert_eq!(records[0].model_texture, "");
    assert_eq!(records[0].location, Vec3::ZERO);
    assert_eq!(records[0].drawable_id, 9);
    assert_eq!(records[0].topple_state, W3DToppleState::Falling);
    assert_eq!(buffer.trees()[id].tree_type, DELETED_TREE_TYPE);
}

#[test]
fn load_records_rebuilds_from_known_types_and_restores_cpp_subset() {
    let mut buffer = W3DTreeBuffer::new();
    let data = module("Oak", "Leaf");
    buffer.add_tree_type(data, bounds()).unwrap();
    buffer
        .add_tree(
            100,
            Vec3::new(99.0, 99.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Existing", "T"),
            bounds(),
        )
        .unwrap();
    let matrix = Mat4::from_translation(Vec3::new(7.0, 8.0, 9.0));
    let records = vec![
        TreeSaveRecord {
            model_name: String::new(),
            model_texture: String::new(),
            location: Vec3::ZERO,
            scale: 1.0,
            sin: 0.0,
            cos: 1.0,
            drawable_id: 1,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Vec3::ZERO,
            topple_state: W3DToppleState::Upright,
            angular_accumulation: 0.0,
            options: 0,
            matrix: Mat4::IDENTITY,
            sink_frames_left: 0,
        },
        TreeSaveRecord {
            model_name: "Unknown".to_string(),
            model_texture: "Leaf".to_string(),
            location: Vec3::ZERO,
            scale: 1.0,
            sin: 0.0,
            cos: 1.0,
            drawable_id: 2,
            angular_velocity: 0.0,
            angular_acceleration: 0.0,
            topple_direction: Vec3::ZERO,
            topple_state: W3DToppleState::Upright,
            angular_accumulation: 0.0,
            options: 0,
            matrix: Mat4::IDENTITY,
            sink_frames_left: 0,
        },
        TreeSaveRecord {
            model_name: "Oak".to_string(),
            model_texture: "Leaf".to_string(),
            location: Vec3::new(3.0, 4.0, 5.0),
            scale: 1.5,
            sin: 0.75,
            cos: 0.25,
            drawable_id: 77,
            angular_velocity: 2.0,
            angular_acceleration: 0.5,
            topple_direction: Vec3::Y,
            topple_state: W3DToppleState::Falling,
            angular_accumulation: 1.0,
            options: W3D_TOPPLE_OPTIONS_NO_BOUNCE,
            matrix,
            sink_frames_left: 12,
        },
    ];

    buffer.load_records(&records);

    assert_eq!(buffer.trees().len(), 1);
    let tree = &buffer.trees()[0];
    assert_eq!(tree.drawable_id, 77);
    assert_eq!(tree.location, Vec3::new(3.0, 4.0, 5.0));
    approx_eq(tree.scale, 1.5);
    approx_eq(tree.sin, 0.0);
    approx_eq(tree.cos, 1.0);
    approx_eq(tree.angular_velocity, 2.0);
    approx_eq(tree.angular_acceleration, 0.5);
    assert_eq!(tree.topple_direction, Vec3::Y);
    assert_eq!(tree.topple_state, W3DToppleState::Falling);
    approx_eq(tree.angular_accumulation, 0.0);
    assert_eq!(tree.options, W3D_TOPPLE_OPTIONS_NO_BOUNCE);
    assert_eq!(tree.matrix, matrix);
    assert_eq!(tree.sink_frames_left, 12);
    assert!(buffer.anything_changed());
}

struct FixedRandom;

impl TreeRandom for FixedRandom {
    fn int_range(&mut self, min: i32, _max: i32) -> i32 {
        min
    }

    fn real_range(&mut self, min: f32, _max: f32) -> f32 {
        min
    }
}

struct AddTreeRandom {
    expected_real_min: f32,
    expected_real_max: f32,
    real: f32,
    int: i32,
}

impl TreeRandom for AddTreeRandom {
    fn int_range(&mut self, min: i32, max: i32) -> i32 {
        assert_eq!(min, 0);
        assert_eq!(max, 9);
        self.int
    }

    fn real_range(&mut self, min: f32, max: f32) -> f32 {
        approx_eq(min, self.expected_real_min);
        approx_eq(max, self.expected_real_max);
        self.real
    }
}

#[test]
fn add_tree_randomized_matches_cpp_scale_amount_and_sway_rng() {
    let mut buffer = W3DTreeBuffer::new();
    let mut rng = AddTreeRandom {
        expected_real_min: 0.75,
        expected_real_max: 1.25,
        real: 1.2,
        int: 7,
    };

    let id = buffer
        .add_tree_randomized(
            11,
            Vec3::new(10.0, 20.0, 3.0),
            2.0,
            0.0,
            0.25,
            module("Oak", "T"),
            bounds(),
            &mut rng,
        )
        .unwrap();

    approx_eq(buffer.trees()[id].scale, 2.4);
    assert_eq!(buffer.trees()[id].sway_type, 7);
    approx_eq(buffer.trees()[id].bounds.radius, 12.0);
}

#[test]
fn add_tree_zero_random_scale_amount_keeps_scale_but_still_draws_sway_type() {
    let mut buffer = W3DTreeBuffer::new();
    let mut rng = AddTreeRandom {
        expected_real_min: 1.0,
        expected_real_max: 1.0,
        real: 99.0,
        int: 3,
    };

    let id = buffer
        .add_tree_randomized(
            12,
            Vec3::ZERO,
            2.0,
            0.0,
            0.0,
            module("Oak", "T"),
            bounds(),
            &mut rng,
        )
        .unwrap();

    approx_eq(buffer.trees()[id].scale, 2.0);
    assert_eq!(buffer.trees()[id].sway_type, 3);
}

#[test]
fn cull_trees_updates_visibility_and_sort_key_like_cpp() {
    let mut buffer = W3DTreeBuffer::new();
    let first = buffer
        .add_tree(
            21,
            Vec3::new(4.0, 10.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    let second = buffer
        .add_tree(
            22,
            Vec3::new(30.0, 2.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Pine", "T"),
            bounds(),
        )
        .unwrap();

    buffer.cull_trees(Vec3::X, |sphere| sphere.center.x < 20.0);

    assert_eq!(buffer.camera_look_at_vector(), Vec3::X);
    assert!(buffer.trees()[first].visible);
    assert!(!buffer.trees()[second].visible);
    approx_eq(buffer.trees()[first].sort_key, 4.0);
    approx_eq(buffer.trees()[second].sort_key, 0.0);
    assert!(buffer.anything_changed());
    assert!(!buffer.update_all_keys());
}

#[test]
fn cull_trees_recomputes_visible_sort_keys_after_full_update() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            23,
            Vec3::new(4.0, 10.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    buffer.cull_trees(Vec3::X, |_| true);
    approx_eq(buffer.trees()[id].sort_key, 4.0);

    buffer.do_full_update();
    assert!(buffer.update_all_keys());
    buffer.cull_trees(Vec3::Y, |_| true);

    approx_eq(buffer.trees()[id].sort_key, 10.0);
    assert!(!buffer.update_all_keys());
}

#[test]
fn cull_trees_from_camera_transform_matches_cpp_negative_z_axis() {
    let mut buffer = W3DTreeBuffer::new();
    let id = buffer
        .add_tree(
            24,
            Vec3::new(1.0, 2.0, 3.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    let camera_transform = Mat4::from_cols(
        Vec4::X,
        Vec4::Y,
        Vec4::new(-2.0, 3.0, 4.0, 0.0),
        Vec4::W,
    );

    buffer.cull_trees_from_camera_transform(camera_transform, |_| true);

    assert_eq!(buffer.camera_look_at_vector(), Vec3::new(2.0, -3.0, -4.0));
    approx_eq(buffer.trees()[id].sort_key, -16.0);
}

#[test]
fn remove_trees_for_construction_uses_cpp_tree_cylinder_radius() {
    let mut buffer = W3DTreeBuffer::new();
    let removed = buffer
        .add_tree(
            31,
            Vec3::new(10.0, 0.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    let kept = buffer
        .add_tree(
            32,
            Vec3::new(40.0, 0.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Pine", "T"),
            bounds(),
        )
        .unwrap();

    buffer.remove_trees_for_construction(TreeConstructionGeometry {
        position: Vec3::ZERO,
        major_radius: 1.0,
        minor_radius: 1.0,
        geometry_type: TreeGeometryType::Cylinder,
        angle: 0.0,
    });

    assert_eq!(CONSTRUCTION_TREE_COLLISION_RADIUS, 14.0);
    assert_eq!(buffer.trees()[removed].tree_type, DELETED_TREE_TYPE);
    assert!(buffer.trees()[kept].tree_type >= 0);
    assert!(buffer.anything_changed());
}

#[test]
fn remove_trees_for_construction_skips_already_deleted_trees() {
    let mut buffer = W3DTreeBuffer::new();
    let deleted = buffer
        .add_tree(41, Vec3::ZERO, 1.0, 0.0, 1.0, module("Oak", "T"), bounds())
        .unwrap();
    let removed = buffer
        .add_tree(
            42,
            Vec3::new(12.0, 0.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Pine", "T"),
            bounds(),
        )
        .unwrap();
    buffer.remove_tree(41);
    buffer.tree_mut(deleted).unwrap().location = Vec3::new(12.0, 0.0, 0.0);

    buffer.remove_trees_for_construction(TreeConstructionGeometry {
        position: Vec3::ZERO,
        major_radius: 1.0,
        minor_radius: 1.0,
        geometry_type: TreeGeometryType::Cylinder,
        angle: 0.0,
    });

    assert_eq!(buffer.trees()[deleted].tree_type, DELETED_TREE_TYPE);
    assert_eq!(buffer.trees()[removed].tree_type, DELETED_TREE_TYPE);
}

#[test]
fn remove_trees_for_construction_uses_angle_aware_box_footprint() {
    let mut buffer = W3DTreeBuffer::new();
    let along_long_edge = buffer
        .add_tree(
            51,
            Vec3::new(35.0, 12.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    let outside_tree_cylinder = buffer
        .add_tree(
            52,
            Vec3::new(35.0, 20.5, 0.0),
            1.0,
            0.0,
            1.0,
            module("Pine", "T"),
            bounds(),
        )
        .unwrap();

    buffer.remove_trees_for_construction(TreeConstructionGeometry {
        position: Vec3::ZERO,
        major_radius: 40.0,
        minor_radius: 2.0,
        geometry_type: TreeGeometryType::Box,
        angle: 0.0,
    });

    assert_eq!(buffer.trees()[along_long_edge].tree_type, DELETED_TREE_TYPE);
    assert!(buffer.trees()[outside_tree_cylinder].tree_type >= 0);
}

#[test]
fn remove_trees_for_construction_rotates_box_footprint_like_cpp_angle() {
    let mut buffer = W3DTreeBuffer::new();
    let rotated_long_edge = buffer
        .add_tree(
            61,
            Vec3::new(-12.0, 35.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Oak", "T"),
            bounds(),
        )
        .unwrap();
    let outside_rotated_edge = buffer
        .add_tree(
            62,
            Vec3::new(-20.5, 35.0, 0.0),
            1.0,
            0.0,
            1.0,
            module("Pine", "T"),
            bounds(),
        )
        .unwrap();

    buffer.remove_trees_for_construction(TreeConstructionGeometry {
        position: Vec3::ZERO,
        major_radius: 40.0,
        minor_radius: 2.0,
        geometry_type: TreeGeometryType::Box,
        angle: std::f32::consts::FRAC_PI_2,
    });

    assert_eq!(buffer.trees()[rotated_long_edge].tree_type, DELETED_TREE_TYPE);
    assert!(buffer.trees()[outside_rotated_edge].tree_type >= 0);
}

#[test]
fn update_sway_matches_cpp_array_shapes_and_randomized_type_base() {
    let mut buffer = W3DTreeBuffer::new();
    buffer
        .add_tree(1, Vec3::ZERO, 1.0, 0.0, 1.0, module("Oak", "T"), bounds())
        .unwrap();
    let mut rng = FixedRandom;
    buffer.update_sway(
        BreezeInfo {
            breeze_version: 4,
            lean: 0.2,
            intensity: 0.1,
            direction_vec: Vec2::Y,
            randomness: 0.0,
            breeze_period: 50,
        },
        &mut rng,
    );

    assert_eq!(buffer.trees()[0].sway_type, 1);
    assert!(buffer.trees()[0].sway_type <= 10);
}

#[test]
fn tree_radius_approx_matches_cpp_unit_collision_constant() {
    assert_eq!(TREE_RADIUS_APPROX, 7.0);
}

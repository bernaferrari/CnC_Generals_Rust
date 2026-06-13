use game_client_rust::terrain::{
    BreezeInfo, TreeCollisionUnit, TreeGeometryType, TreeModuleData, TreeRandom, TreeRegion2D,
    TreeShroudStatus, TreeSphere, W3DToppleState, W3DTreeBuffer, ANGULAR_LIMIT, DELETED_TREE_TYPE,
    END_OF_PARTITION, MAX_TREES, MAX_TYPES, PARTITION_WIDTH_HEIGHT, TREE_RADIUS_APPROX,
    W3D_TOPPLE_OPTIONS_NO_BOUNCE,
};
use glam::{Vec2, Vec3};

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

struct FixedRandom;

impl TreeRandom for FixedRandom {
    fn int_range(&mut self, min: i32, _max: i32) -> i32 {
        min
    }

    fn real_range(&mut self, min: f32, _max: f32) -> f32 {
        min
    }
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

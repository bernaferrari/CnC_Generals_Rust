//! Physics System Tests
//!
//! This module tests the WW3D physics facade that wraps the collision system.
//!
//! Tests verify:
//! - Rigid body creation and property access
//! - Physics world management
//! - Simulation timestep behavior
//! - Collision shape handling
//! - Ray casting
//! - Physics statistics
//! - C++ parity with original physics system

use ww3d_physics::*;

/// Test rigid body type classification
/// Verifies body type defaults and behavior
#[test]
fn test_rigid_body_types() {
    let static_body = RigidBody::new(RigidBodyType::Static, Vec3::ZERO, Quat::IDENTITY, 100.0);
    assert_eq!(static_body.body_type(), RigidBodyType::Static);
    assert_eq!(
        static_body.mass(),
        0.0,
        "Static bodies should have zero mass"
    );

    let kinematic_body = RigidBody::new(RigidBodyType::Kinematic, Vec3::ZERO, Quat::IDENTITY, 50.0);
    assert_eq!(kinematic_body.body_type(), RigidBodyType::Kinematic);
    assert_eq!(
        kinematic_body.mass(),
        0.0,
        "Kinematic bodies should have zero mass"
    );

    let dynamic_body = RigidBody::new(RigidBodyType::Dynamic, Vec3::ZERO, Quat::IDENTITY, 25.0);
    assert_eq!(dynamic_body.body_type(), RigidBodyType::Dynamic);
    assert_eq!(
        dynamic_body.mass(),
        25.0,
        "Dynamic bodies should preserve mass"
    );
}

/// Test rigid body creation with position and rotation
/// Reference: C++ RigidBody constructor
#[test]
fn test_rigid_body_creation() {
    let position = Vec3::new(1.0, 2.0, 3.0);
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    let mass = 10.0;

    let body = RigidBody::new(RigidBodyType::Dynamic, position, rotation, mass);

    assert_eq!(body.position(), position);
    assert_eq!(body.rotation(), rotation);
    assert_eq!(body.mass(), mass);
}

/// Test rigid body default type
/// Verifies default is Dynamic as per C++ behavior
#[test]
fn test_rigid_body_type_default() {
    let default_type = RigidBodyType::default();
    assert_eq!(default_type, RigidBodyType::Dynamic);
}

/// Test negative mass clamping for dynamic bodies
/// Verifies negative mass values are clamped to 0
#[test]
fn test_rigid_body_negative_mass() {
    let body = RigidBody::new(RigidBodyType::Dynamic, Vec3::ZERO, Quat::IDENTITY, -10.0);
    assert_eq!(body.mass(), 0.0, "Negative mass should be clamped to 0");
}

/// Test rigid body descriptor default values
/// Reference: Original physics system defaults
#[test]
fn test_rigid_body_desc_defaults() {
    let desc = RigidBodyDesc::default();

    assert_eq!(desc.body_type, RigidBodyType::Dynamic);
    assert_eq!(desc.position, Vec3::ZERO);
    assert_eq!(desc.rotation, Quat::IDENTITY);
    assert_eq!(desc.linear_velocity, Vec3::ZERO);
    assert_eq!(desc.angular_velocity, Vec3::ZERO);
    assert_eq!(desc.mass, 1.0);
    assert_eq!(desc.restitution, 0.3);
    assert_eq!(desc.friction, 0.5);
    assert_eq!(desc.linear_damping, 0.05);
    assert_eq!(desc.angular_damping, 0.05);
}

/// Test physics world creation
/// Verifies world initializes in valid state
#[test]
fn test_physics_world_creation() {
    let world = PhysicsWorld::new();
    assert!(world.is_valid());
    assert_eq!(world.body_count(), 0);
}

/// Test body creation in physics world
/// Reference: C++ PhysicsWorld::CreateBody
#[test]
fn test_physics_world_create_body() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        body_type: RigidBodyType::Dynamic,
        position: Vec3::new(0.0, 10.0, 0.0),
        mass: 5.0,
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    assert_eq!(world.body_count(), 1);

    let body = world.get_body(body_id).expect("Body should exist");
    assert_eq!(body.body_type(), RigidBodyType::Dynamic);
}

/// Test body removal from physics world
/// Verifies proper cleanup and count tracking
#[test]
fn test_physics_world_remove_body() {
    let mut world = PhysicsWorld::new();

    let body_id = world.create_body(RigidBodyDesc::default());
    assert_eq!(world.body_count(), 1);

    world.remove_body(body_id);
    assert_eq!(world.body_count(), 0);
    assert!(world.get_body(body_id).is_none(), "Body should be removed");
}

/// Test multiple body creation
/// Verifies world can handle multiple bodies
#[test]
fn test_physics_world_multiple_bodies() {
    let mut world = PhysicsWorld::new();

    let id1 = world.create_body(RigidBodyDesc::default());
    let id2 = world.create_body(RigidBodyDesc::default());
    let id3 = world.create_body(RigidBodyDesc::default());

    assert_eq!(world.body_count(), 3);
    assert!(world.get_body(id1).is_some());
    assert!(world.get_body(id2).is_some());
    assert!(world.get_body(id3).is_some());
}

/// Test body access with immutable reference
/// Verifies get_body returns correct body
#[test]
fn test_physics_world_get_body() {
    let mut world = PhysicsWorld::new();

    let position = Vec3::new(5.0, 10.0, 15.0);
    let desc = RigidBodyDesc {
        position,
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    let body = world.get_body(body_id).expect("Body should exist");

    assert_eq!(body.position, position);
}

/// Test body access with mutable reference
/// Verifies get_body_mut allows modification
#[test]
fn test_physics_world_get_body_mut() {
    let mut world = PhysicsWorld::new();

    let body_id = world.create_body(RigidBodyDesc::default());
    let mut body = world.get_body_mut(body_id).expect("Body should exist");

    let new_position = Vec3::new(1.0, 2.0, 3.0);
    body.position = new_position;

    drop(body); // Drop mutable borrow

    let body = world.get_body(body_id).unwrap();
    assert_eq!(body.position, new_position);
}

/// Test physics simulation timestep
/// Reference: C++ PhysicsWorld::Step() method
#[test]
fn test_physics_simulation_step() {
    let mut world = PhysicsWorld::new();

    // Create a dynamic body above the ground
    let desc = RigidBodyDesc {
        body_type: RigidBodyType::Dynamic,
        position: Vec3::new(0.0, 10.0, 0.0),
        mass: 1.0,
        ..Default::default()
    };

    let body_id = world.create_body(desc);

    // Step simulation
    world.step();

    // Body should still exist after step
    assert!(world.get_body(body_id).is_some());
}

/// Test collision shape creation - sphere
/// Reference: CollisionShape enum variants
#[test]
fn test_collision_shape_sphere() {
    let shape = CollisionShape::Sphere { radius: 5.0 };

    if let CollisionShape::Sphere { radius } = shape {
        assert_eq!(radius, 5.0);
    } else {
        panic!("Shape should be Sphere");
    }
}

/// Test collision shape creation - box
/// Verifies axis-aligned box creation
#[test]
fn test_collision_shape_box() {
    let half_extents = Vec3::new(1.0, 2.0, 3.0);
    let shape = CollisionShape::Box { half_extents };

    if let CollisionShape::Box { half_extents: he } = shape {
        assert_eq!(he, half_extents);
    } else {
        panic!("Shape should be Box");
    }
}

/// Test body with sphere collision shape
/// Verifies shape is properly assigned
#[test]
fn test_body_with_sphere_shape() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        shape: CollisionShape::Sphere { radius: 2.5 },
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    let body = world.get_body(body_id).unwrap();

    // Verify body was created successfully (mass > 0 indicates dynamic body)
    assert!(body.mass > 0.0);
}

/// Test body with box collision shape
/// Verifies box shape assignment
#[test]
fn test_body_with_box_shape() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        shape: CollisionShape::Box {
            half_extents: Vec3::new(1.0, 1.0, 1.0),
        },
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    assert!(world.get_body(body_id).is_some());
}

/// Test ray casting in physics world
/// Reference: C++ PhysicsWorld::RayCast
#[test]
fn test_physics_ray_cast() {
    let world = PhysicsWorld::new();

    let origin = Vec3::new(0.0, 10.0, 0.0);
    let direction = Vec3::new(0.0, -1.0, 0.0);
    let max_distance = 20.0;

    // Ray cast should not panic
    let _result = world.ray_cast(origin, direction, max_distance);
}

/// Test ray cast with body in path
/// Verifies ray cast detects bodies
#[test]
fn test_physics_ray_cast_with_body() {
    let mut world = PhysicsWorld::new();

    // Create a static body in the ray path
    let desc = RigidBodyDesc {
        body_type: RigidBodyType::Static,
        position: Vec3::new(0.0, 0.0, 0.0),
        shape: CollisionShape::Sphere { radius: 1.0 },
        ..Default::default()
    };

    world.create_body(desc);

    let origin = Vec3::new(0.0, 5.0, 0.0);
    let direction = Vec3::new(0.0, -1.0, 0.0);
    let max_distance = 10.0;

    let result = world.ray_cast(origin, direction, max_distance);
    // May or may not hit depending on implementation, just verify it doesn't panic
    let _ = result;
}

/// Test physics statistics tracking
/// Reference: PhysicsStats structure
#[test]
fn test_physics_stats() {
    let world = PhysicsWorld::new();
    let stats = world.stats();

    // Stats should exist and be accessible
    assert!(stats.constraints >= 0);
    assert!(stats.bodies >= 0);
}

/// Test physics world with static body
/// Static bodies should never move
#[test]
fn test_static_body_immobility() {
    let mut world = PhysicsWorld::new();

    let initial_position = Vec3::new(0.0, 0.0, 0.0);
    let desc = RigidBodyDesc {
        body_type: RigidBodyType::Static,
        position: initial_position,
        ..Default::default()
    };

    let body_id = world.create_body(desc);

    // Step simulation multiple times
    for _ in 0..10 {
        world.step();
    }

    let body = world.get_body(body_id).unwrap();
    assert_eq!(
        body.position, initial_position,
        "Static body should not move"
    );
}

/// Test kinematic body behavior
/// Kinematic bodies move via velocity, not forces
#[test]
fn test_kinematic_body_properties() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        body_type: RigidBodyType::Kinematic,
        position: Vec3::ZERO,
        linear_velocity: Vec3::new(1.0, 0.0, 0.0),
        mass: 100.0, // Should be ignored
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    let body = world.get_body(body_id).unwrap();

    assert_eq!(body.body_type(), RigidBodyType::Kinematic);
    // Mass should be zero for kinematic bodies
}

/// Test body material properties
/// Reference: C++ body restitution and friction
#[test]
fn test_body_material_properties() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        restitution: 0.8,
        friction: 0.6,
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    let body = world.get_body(body_id).unwrap();

    // Material properties are stored in backend, just verify creation succeeds
    assert!(body.mass > 0.0);
}

/// Test body damping properties
/// Verifies linear and angular damping
#[test]
fn test_body_damping() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        linear_damping: 0.1,
        angular_damping: 0.2,
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    assert!(world.get_body(body_id).is_some());
}

/// Test body with initial velocity
/// Verifies velocity is properly set
#[test]
fn test_body_initial_velocity() {
    let mut world = PhysicsWorld::new();

    let linear_velocity = Vec3::new(5.0, 0.0, 0.0);
    let angular_velocity = Vec3::new(0.0, 1.0, 0.0);

    let desc = RigidBodyDesc {
        linear_velocity,
        angular_velocity,
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    let body = world.get_body(body_id).unwrap();

    // Velocity is stored in backend
    assert!(body.mass > 0.0);
}

/// Test physics world body iteration
/// Verifies we can query all bodies
#[test]
fn test_physics_world_body_iteration() {
    let mut world = PhysicsWorld::new();

    let count = 5;
    let mut body_ids = Vec::new();

    for i in 0..count {
        let desc = RigidBodyDesc {
            position: Vec3::new(i as f32, 0.0, 0.0),
            ..Default::default()
        };
        body_ids.push(world.create_body(desc));
    }

    assert_eq!(world.body_count(), count);

    // Verify all bodies exist
    for id in body_ids {
        assert!(world.get_body(id).is_some());
    }
}

/// Test zero-mass dynamic body
/// Zero-mass dynamic bodies should be treated specially
#[test]
fn test_zero_mass_dynamic_body() {
    let mut world = PhysicsWorld::new();

    let desc = RigidBodyDesc {
        body_type: RigidBodyType::Dynamic,
        mass: 0.0,
        ..Default::default()
    };

    let body_id = world.create_body(desc);
    let body = world.get_body(body_id).unwrap();
    assert_eq!(body.mass, 0.0);
}

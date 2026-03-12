#![cfg(feature = "internal")]
/*
** Command & Conquer Generals Zero Hour(tm)
** Copyright 2025 Electronic Arts Inc.
**
** This program is free software: you can redistribute it and/or modify
** it under the terms of the GNU General Public License as published by
** the Free Software Foundation, either version 3 of the License, or
** (at your option) any later version.
**
** This program is distributed in the hope that it will be useful,
** but WITHOUT ANY WARRANTY; without even the implied warranty of
** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
** GNU General Public License for more details.
**
** You should have received a copy of the GNU General Public License
** along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

//! Integration tests for the cinematic camera system

use game_client_rust::display::cinematic_camera::*;
use glam::Vec2;
use glam::Vec3;

#[test]
fn test_camera_shake_system_basic() {
    let mut system = CameraShakeSystem::new();

    // Add a shake at origin
    system.add_camera_shake(Vec3::ZERO, 50.0, 1.5, 1.0);

    // Should be shaking
    assert!(system.is_camera_shaking());

    // Should produce shake angles at epicenter
    let angles = system.update_camera_shaker(Vec3::ZERO);
    assert!(
        angles.length() > 0.0,
        "Shake angles should be non-zero at epicenter"
    );

    // Should produce no shake far away
    let far_angles = system.update_camera_shaker(Vec3::new(1000.0, 1000.0, 0.0));
    assert_eq!(
        far_angles,
        Vec3::ZERO,
        "Shake should be zero far from epicenter"
    );
}

#[test]
fn test_camera_shake_expiration() {
    let mut system = CameraShakeSystem::new();

    // Add short duration shake
    system.add_camera_shake(Vec3::ZERO, 50.0, 0.1, 1.0);
    assert!(system.is_camera_shaking());

    // Update past duration
    system.timestep(0.2);
    assert!(
        !system.is_camera_shaking(),
        "Shake should expire after duration"
    );
}

#[test]
fn test_multiple_shakes() {
    let mut system = CameraShakeSystem::new();

    // Add multiple shakes
    system.add_camera_shake(Vec3::ZERO, 50.0, 1.0, 1.0);
    system.add_camera_shake(Vec3::new(10.0, 10.0, 0.0), 50.0, 1.0, 1.0);
    system.add_camera_shake(Vec3::new(20.0, 20.0, 0.0), 50.0, 1.0, 1.0);

    // Should accumulate angles from all shakes
    let angles = system.update_camera_shaker(Vec3::ZERO);
    assert!(angles.length() > 0.0);
}

#[test]
fn test_parabolic_ease() {
    let ease = ParabolicEase::new(0.5, 0.5);

    // Start should be 0
    assert_eq!(ease.apply(0.0), 0.0);

    // End should be 1
    assert_eq!(ease.apply(1.0), 1.0);

    // Middle should be around 0.5
    let mid = ease.apply(0.5);
    assert!((mid - 0.5).abs() < 0.1);
}

#[test]
fn test_camera_path() {
    let waypoints = vec![
        CameraWaypoint {
            position: Vec3::ZERO,
            angle: 0.0,
            time_multiplier: 1,
        },
        CameraWaypoint {
            position: Vec3::new(100.0, 0.0, 0.0),
            angle: std::f32::consts::PI / 2.0,
            time_multiplier: 1,
        },
    ];

    let mut path = CameraPath::new(waypoints, 1000, 1, false, 0.0, 0.0);

    // Start position
    let start_pos = path.get_current_position();
    assert!((start_pos - Vec3::ZERO).length() < 0.01);

    // Halfway
    assert!(!path.update(500));
    let mid_pos = path.get_current_position();
    assert!((mid_pos.x - 50.0).abs() < 10.0, "Should be around halfway");

    // End
    assert!(path.update(500), "Path should be complete");
    assert!(path.is_complete());
}

#[test]
fn test_camera_constraints() {
    let mut constraints = CameraConstraints::new();
    constraints.set_bounds(Vec2::new(-100.0, -100.0), Vec2::new(100.0, 100.0));

    // Inside bounds - no change
    let pos = Vec3::new(50.0, 50.0, 0.0);
    let constrained = constraints.constrain(pos);
    assert_eq!(pos, constrained);

    // Outside bounds - clamped
    let pos = Vec3::new(200.0, -200.0, 0.0);
    let constrained = constraints.constrain(pos);
    assert_eq!(constrained.x, 100.0);
    assert_eq!(constrained.y, -100.0);
}

#[test]
fn test_camera_follow_system() {
    let mut follow = CameraFollowSystem::new();

    // Set target
    follow.set_target(Some(1), Vec3::new(100.0, 100.0, 0.0));
    assert!(follow.is_following());

    // Should move towards target
    let new_pos = follow.update(Vec3::ZERO, Vec3::new(100.0, 100.0, 0.0));
    assert!(new_pos.length() > 0.0);
    assert!(new_pos.length() < 100.0, "Should smoothly follow");
}

#[test]
fn test_death_camera() {
    let mut death = DeathCamera::new();

    death.activate(Vec3::new(100.0, 100.0, 0.0), Vec3::ZERO, 1.0, 60);

    assert!(death.is_active());

    // Should interpolate towards target
    if let Some((pos, zoom)) = death.update() {
        assert!(pos.length() > 0.0);
        assert!(zoom < 1.0, "Should zoom in");
    } else {
        panic!("Death camera should be active");
    }
}

#[test]
fn test_camera_rotate_transition() {
    let current_angle = 0.0;
    let mut rotation = CameraRotateTransition::new(1.0, 60, 0.0, 0.0, current_angle);

    // First frame
    assert!(!rotation.update());
    let angle1 = rotation.get_current_angle();
    assert!(angle1 > 0.0);

    // Should progress
    for _ in 0..58 {
        assert!(!rotation.update());
    }

    // Last frame
    assert!(rotation.update(), "Should complete after 60 frames");
}

#[test]
fn test_cinematic_camera_integration() {
    let mut system = CinematicCameraSystem::new();
    let state = CameraState::default();

    // Add shake
    system
        .shake_system
        .add_camera_shake(Vec3::ZERO, 50.0, 1.0, 1.0);

    // Update system
    let new_state = system.update(0.016, state);

    // Rotation should be affected by shake
    assert_ne!(new_state.rotation, state.rotation);
}

#[test]
fn test_camera_shake_types() {
    assert_eq!(CameraShakeType::Subtle.power(), 0.5);
    assert_eq!(CameraShakeType::Normal.power(), 1.0);
    assert_eq!(CameraShakeType::Strong.power(), 2.0);
    assert_eq!(CameraShakeType::Severe.power(), 4.0);
    assert_eq!(CameraShakeType::CineExtreme.power(), 8.0);
    assert_eq!(CameraShakeType::CineInsane.power(), 16.0);
}

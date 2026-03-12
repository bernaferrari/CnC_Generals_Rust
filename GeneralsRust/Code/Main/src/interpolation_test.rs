use crate::interpolation::InterpolationManager;
use crate::game_logic::ObjectId;
use glam::{Vec3, Quat};

/// Test the interpolation system with various framerates
pub fn test_interpolation_system() {
    println!("🧪 Testing Frame Interpolation System");
    println!("=====================================");
    
    // Create interpolation manager
    let mut interpolation_manager = InterpolationManager::new();
    
    // Test object
    let object_id = ObjectId(1);
    let initial_position = Vec3::new(0.0, 0.0, 0.0);
    let target_position = Vec3::new(100.0, 0.0, 0.0);
    let initial_rotation = Quat::IDENTITY;
    let target_rotation = Quat::from_rotation_y(std::f32::consts::PI / 2.0); // 90 degrees
    
    // Simulate 30Hz logic updates with 60Hz, 120Hz, and 144Hz rendering
    test_framerate(&mut interpolation_manager, object_id, 
                   initial_position, target_position, 
                   initial_rotation, target_rotation, 
                   60.0, "60Hz rendering");
    
    test_framerate(&mut interpolation_manager, object_id, 
                   initial_position, target_position, 
                   initial_rotation, target_rotation, 
                   120.0, "120Hz rendering");
    
    test_framerate(&mut interpolation_manager, object_id, 
                   initial_position, target_position, 
                   initial_rotation, target_rotation, 
                   144.0, "144Hz rendering");
    
    // Test camera interpolation
    test_camera_interpolation(&mut interpolation_manager);
    
    // Test alpha clamping (no extrapolation)
    test_alpha_clamping(&mut interpolation_manager, object_id);
    
    println!("✅ All interpolation tests passed!");
}

fn test_framerate(
    interpolation_manager: &mut InterpolationManager,
    object_id: ObjectId,
    initial_pos: Vec3,
    target_pos: Vec3,
    initial_rot: Quat,
    target_rot: Quat,
    render_fps: f32,
    test_name: &str
) {
    println!("\n🎯 Testing {} (Logic: 30Hz, Render: {:.0}Hz)", test_name, render_fps);
    
    // Clear previous test data
    interpolation_manager.clear();
    
    // Constants
    const LOGIC_FPS: f32 = 30.0;
    const LOGIC_FRAME_TIME: f32 = 1.0 / LOGIC_FPS;
    let render_frame_time: f32 = 1.0 / render_fps;
    const TEST_DURATION: f32 = 1.0; // 1 second test
    
    let mut logic_time = 0.0f32;
    let mut render_time = 0.0f32;
    let mut logic_frame = 0u32;
    
    println!("   Logic frame time: {:.3}ms", LOGIC_FRAME_TIME * 1000.0);
    println!("   Render frame time: {:.3}ms", render_frame_time * 1000.0);
    
    // Setup initial state
    interpolation_manager.prepare_for_logic_update();
    update_test_object_state(interpolation_manager, object_id, initial_pos, initial_rot);
    
    let mut render_frames = 0u32;
    let mut max_position_error = 0.0f32;
    
    while render_time < TEST_DURATION {
        // Update logic at 30Hz
        if render_time >= logic_time + LOGIC_FRAME_TIME {
            logic_time += LOGIC_FRAME_TIME;
            logic_frame += 1;
            
            // Prepare for logic update
            interpolation_manager.prepare_for_logic_update();
            
            // Simulate object movement (linear interpolation between initial and target)
            let progress = (logic_time / TEST_DURATION).clamp(0.0, 1.0);
            let current_pos = initial_pos.lerp(target_pos, progress);
            let current_rot = initial_rot.slerp(target_rot, progress);
            
            // Update interpolation state
            update_test_object_state(interpolation_manager, object_id, current_pos, current_rot);
        }
        
        // Render frame
        let accumulator = render_time - logic_time;
        let alpha = (accumulator / LOGIC_FRAME_TIME).clamp(0.0, 1.0);
        
        interpolation_manager.set_alpha(alpha);
        
        // Get interpolated position
        if let Some(interpolated_pos) = interpolation_manager.get_interpolated_position(object_id) {
            // Calculate expected position for this exact time
            let progress = (render_time / TEST_DURATION).clamp(0.0, 1.0);
            let expected_pos = initial_pos.lerp(target_pos, progress);
            let error = interpolated_pos.distance(expected_pos);
            
            if error > max_position_error {
                max_position_error = error;
            }
            
            // Log every 10th frame for verification
            if render_frames % 10 == 0 {
                println!("   Frame {}: alpha={:.3}, pos=({:.1}, {:.1}, {:.1}), error={:.3}", 
                         render_frames, alpha, 
                         interpolated_pos.x, interpolated_pos.y, interpolated_pos.z,
                         error);
            }
        }
        
        render_time += render_frame_time;
        render_frames += 1;
    }
    
    println!("   Completed {} render frames in {:.1}s", render_frames, TEST_DURATION);
    println!("   Logic frames: {}", logic_frame);
    println!("   Max position error: {:.3} units", max_position_error);
    
    // Verify error is within acceptable bounds
    assert!(max_position_error < 5.0, "Position error too high: {:.3}", max_position_error);
    
    let stats = interpolation_manager.get_stats();
    println!("   Final stats: {} objects tracked", stats.object_count);
}

fn test_camera_interpolation(interpolation_manager: &mut InterpolationManager) {
    println!("\n📷 Testing Camera Interpolation");
    
    let initial_pos = Vec3::new(0.0, 10.0, -20.0);
    let target_pos = Vec3::new(50.0, 15.0, 0.0);
    let initial_target = Vec3::new(0.0, 0.0, 0.0);
    let target_target = Vec3::new(50.0, 0.0, 0.0);
    let initial_zoom = 1.0;
    let target_zoom = 2.0;
    
    // Update camera states
    interpolation_manager.camera_state.update(initial_pos, initial_target, initial_zoom);
    interpolation_manager.camera_state.update(target_pos, target_target, target_zoom);
    
    // Test interpolation at different alpha values
    let test_alphas = [0.0, 0.25, 0.5, 0.75, 1.0];
    
    for alpha in test_alphas {
        interpolation_manager.set_alpha(alpha);
        
        let interpolated_pos = interpolation_manager.get_interpolated_camera_position();
        let interpolated_target = interpolation_manager.get_interpolated_camera_target();
        let interpolated_zoom = interpolation_manager.get_interpolated_camera_zoom();
        
        let expected_pos = initial_pos.lerp(target_pos, alpha);
        let expected_target = initial_target.lerp(target_target, alpha);
        let expected_zoom = initial_zoom * (1.0 - alpha) + target_zoom * alpha;
        
        let pos_error = interpolated_pos.distance(expected_pos);
        let target_error = interpolated_target.distance(expected_target);
        let zoom_error = (interpolated_zoom - expected_zoom).abs();
        
        println!("   Alpha {:.2}: pos_error={:.3}, target_error={:.3}, zoom_error={:.3}", 
                alpha, pos_error, target_error, zoom_error);
        
        assert!(pos_error < 0.01, "Camera position error too high");
        assert!(target_error < 0.01, "Camera target error too high");
        assert!(zoom_error < 0.01, "Camera zoom error too high");
    }
    
    println!("   ✅ Camera interpolation working correctly");
}

fn test_alpha_clamping(interpolation_manager: &mut InterpolationManager, object_id: ObjectId) {
    println!("\n🔒 Testing Alpha Clamping (No Extrapolation)");
    
    let initial_pos = Vec3::new(0.0, 0.0, 0.0);
    let target_pos = Vec3::new(10.0, 0.0, 0.0);
    
    // Setup test object
    interpolation_manager.prepare_for_logic_update();
    update_test_object_state(interpolation_manager, object_id, initial_pos, Quat::IDENTITY);
    interpolation_manager.prepare_for_logic_update();
    update_test_object_state(interpolation_manager, object_id, target_pos, Quat::IDENTITY);
    
    // Test with alpha values outside [0.0, 1.0] range
    let test_alphas = [-0.5, -0.1, 0.0, 0.5, 1.0, 1.1, 1.5, 2.0];
    
    for alpha in test_alphas {
        interpolation_manager.set_alpha(alpha);
        
        if let Some(interpolated_pos) = interpolation_manager.get_interpolated_position(object_id) {
            // Position should always be between initial_pos and target_pos
            let clamped_alpha = alpha.clamp(0.0, 1.0);
            let expected_pos = initial_pos.lerp(target_pos, clamped_alpha);
            let error = interpolated_pos.distance(expected_pos);
            
            println!("   Alpha {:.2} -> Clamped {:.2}: pos=({:.1}, {:.1}, {:.1}), error={:.3}", 
                    alpha, clamped_alpha, interpolated_pos.x, interpolated_pos.y, interpolated_pos.z, error);
            
            assert!(error < 0.01, "Alpha clamping failed for alpha={}", alpha);
            
            // Ensure no extrapolation occurred
            if alpha < 0.0 {
                assert!((interpolated_pos.distance(initial_pos) < 0.01), "Extrapolation occurred below 0.0");
            } else if alpha > 1.0 {
                assert!((interpolated_pos.distance(target_pos) < 0.01), "Extrapolation occurred above 1.0");
            }
        }
    }
    
    println!("   ✅ Alpha clamping working correctly (no extrapolation)");
}

fn update_test_object_state(
    interpolation_manager: &mut InterpolationManager,
    object_id: ObjectId,
    position: Vec3,
    rotation: Quat
) {
    // Create a mock game object for testing
    use crate::game_logic::{Object, ThingTemplate, Team};
    
    let template = ThingTemplate::new("TestObject");
    let mut mock_object = Object::new(template, object_id, Team::USA);
    mock_object.thing.set_position(position);
    mock_object.thing.set_orientation(rotation.to_euler(glam::EulerRot::XYZ).1); // Y rotation
    
    // Use the proper update method
    interpolation_manager.update_object_state(object_id, &mock_object);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_basic() {
        let mut manager = InterpolationManager::new();
        let object_id = ObjectId(1);
        
        // Test basic interpolation
        manager.prepare_for_logic_update();
        update_test_object_state(&mut manager, object_id, Vec3::ZERO, Quat::IDENTITY);
        
        manager.prepare_for_logic_update();
        update_test_object_state(&mut manager, object_id, Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY);
        
        // Test 50% interpolation
        manager.set_alpha(0.5);
        
        if let Some(pos) = manager.get_interpolated_position(object_id) {
            assert!((pos.x - 5.0).abs() < 0.01, "Interpolation should be at 50%");
        } else {
            panic!("Interpolated position should exist");
        }
    }

    #[test]
    fn test_alpha_clamping() {
        let mut manager = InterpolationManager::new();
        let object_id = ObjectId(1);
        
        manager.prepare_for_logic_update();
        update_test_object_state(&mut manager, object_id, Vec3::ZERO, Quat::IDENTITY);
        
        manager.prepare_for_logic_update();
        update_test_object_state(&mut manager, object_id, Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY);
        
        // Test alpha clamping
        manager.set_alpha(-0.5);
        assert_eq!(manager.get_alpha(), 0.0, "Alpha should be clamped to 0.0");
        
        manager.set_alpha(1.5);
        assert_eq!(manager.get_alpha(), 1.0, "Alpha should be clamped to 1.0");
    }
}
// Example: Basic W3D Model Loading and Rendering
// This example demonstrates how to use the WW3D library to load and render a model

use ww3d::*;
use std::sync::Arc;

fn main() -> Result<()> {
    println!("W3D Rendering System - Basic Usage Example");
    println!("==========================================\n");

    // Step 1: Create an asset manager
    println!("Creating asset manager...");
    let mut asset_manager = AssetManager::new();

    // Step 2: Load a W3D mesh file
    println!("Loading W3D mesh file...");
    let mesh_data = std::fs::read("assets/tank.w3d")
        .expect("Failed to read mesh file. Make sure assets/tank.w3d exists");

    asset_manager.load_w3d_mesh("tank".to_string(), &mesh_data)?;
    println!("✓ Mesh loaded successfully");

    // Step 3: Create a mesh instance
    println!("\nCreating mesh instance...");
    let mut tank = asset_manager.create_render_object("tank")?;
    println!("✓ Mesh instance created");

    // Step 4: Set up the transform
    println!("\nSetting up transform...");
    let mut transform = Mat4::identity();
    transform[(0, 3)] = 0.0;  // X position
    transform[(1, 3)] = 0.0;  // Y position
    transform[(2, 3)] = -10.0; // Z position
    tank.set_transform(transform);
    println!("✓ Transform set: position (0, 0, -10)");

    // Step 5: Create render info (camera and viewport)
    println!("\nSetting up camera...");
    let camera_position = Vec3::new(0.0, 5.0, 20.0);
    let look_at = Vec3::new(0.0, 0.0, 0.0);
    let up = Vec3::new(0.0, 1.0, 0.0);

    // Create view matrix (look-at)
    let forward = (look_at - camera_position).normalize();
    let right = forward.cross(&up).normalize();
    let cam_up = right.cross(&forward);

    let view_matrix = Mat4::new(
        right.x, cam_up.x, -forward.x, 0.0,
        right.y, cam_up.y, -forward.y, 0.0,
        right.z, cam_up.z, -forward.z, 0.0,
        -right.dot(&camera_position), -cam_up.dot(&camera_position), forward.dot(&camera_position), 1.0,
    );

    // Create projection matrix (perspective)
    let aspect_ratio = 1920.0 / 1080.0;
    let fov = 60.0_f32.to_radians();
    let near = 0.1;
    let far = 1000.0;

    let f = 1.0 / (fov / 2.0).tan();
    let projection_matrix = Mat4::new(
        f / aspect_ratio, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, (far + near) / (near - far), -1.0,
        0.0, 0.0, (2.0 * far * near) / (near - far), 0.0,
    );

    let render_info = RenderInfo {
        camera_position,
        view_matrix,
        projection_matrix,
        viewport_width: 1920,
        viewport_height: 1080,
    };
    println!("✓ Camera configured");

    // Step 6: Get mesh information
    println!("\nMesh Information:");
    println!("  Name: {}", tank.get_name());
    println!("  Class: {:?}", tank.class_id());
    println!("  Polygon count: {}", tank.get_num_polys());
    println!("  Visible: {}", tank.is_visible());

    let bbox = tank.get_bounding_box();
    println!("  Bounding box: min({:.2}, {:.2}, {:.2}), max({:.2}, {:.2}, {:.2})",
        bbox.min.x, bbox.min.y, bbox.min.z,
        bbox.max.x, bbox.max.y, bbox.max.z);

    let sphere = tank.get_bounding_sphere();
    println!("  Bounding sphere: center({:.2}, {:.2}, {:.2}), radius {:.2}",
        sphere.center.x, sphere.center.y, sphere.center.z, sphere.radius);

    // Step 7: Render the mesh (would integrate with actual wgpu context)
    println!("\nRendering mesh...");
    tank.render(&render_info)?;
    println!("✓ Render complete");

    // Step 8: Example of animation (if the model has a hierarchy)
    if tank.get_num_bones() > 0 {
        println!("\nHierarchy Information:");
        println!("  Bone count: {}", tank.get_num_bones());

        for i in 0..tank.get_num_bones() {
            if let Some(bone_name) = tank.get_bone_name(i) {
                println!("  Bone {}: {}", i, bone_name);
            }
        }

        // Example: Load and apply animation
        println!("\nLoading animation...");
        let anim_data = std::fs::read("assets/tank_move.w3d")
            .unwrap_or_else(|_| {
                println!("  (Animation file not found - skipping)");
                vec![]
            });

        if !anim_data.is_empty() {
            // In a real application, you would:
            // 1. Parse the animation from anim_data
            // 2. Create an HRawAnimation
            // 3. Apply it to the hierarchy
            println!("✓ Animation loaded and ready");
        }
    }

    // Step 9: Example of manipulation
    println!("\nManipulating mesh...");

    // Scale the mesh
    let scale_factor = 1.5;
    tank.scale(scale_factor);
    println!("✓ Scaled by {}", scale_factor);

    // Rotate the mesh
    let rotation = UnitQuat::from_euler_angles(0.0, 45.0_f32.to_radians(), 0.0);
    let mut new_transform = tank.get_transform().clone();
    let rotation_matrix = rotation.to_homogeneous();
    new_transform = new_transform * rotation_matrix;
    tank.set_transform(new_transform);
    println!("✓ Rotated 45 degrees");

    // Change visibility
    tank.set_visible(false);
    println!("✓ Hidden");
    tank.set_visible(true);
    println!("✓ Shown");

    println!("\n==========================================");
    println!("Example completed successfully!");
    println!("\nNext Steps:");
    println!("1. Integrate with a wgpu application");
    println!("2. Load actual W3D files from C&C Generals");
    println!("3. Apply animations and control bones");
    println!("4. Render multiple meshes in a scene");

    Ok(())
}

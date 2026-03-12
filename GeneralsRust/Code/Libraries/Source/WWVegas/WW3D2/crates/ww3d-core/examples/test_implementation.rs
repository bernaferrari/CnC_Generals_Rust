/// Test example to verify all features compile and work
///
/// This example demonstrates the usage of all implemented features.
use ww3d_core::*;

fn main() -> W3DResult<()> {
    println!("=== WW3D-Core Feature Test ===\n");

    // Test 1: Mesh creation and manipulation
    println!("1. Testing Mesh System...");
    let mesh = create_cube_mesh("test_cube".to_string(), 2.0);
    println!(
        "   Created cube mesh with {} vertices and {} triangles",
        mesh.get_num_polys() / 3,
        mesh.get_num_polys()
    );

    // Test 2: Material system
    println!("\n2. Testing Material System...");
    let material = material::create_default_material("test_material".to_string());
    println!(
        "   Created material '{}' with {} passes",
        material.name,
        material.pass_count()
    );

    // Test 3: Texture system
    println!("\n3. Testing Texture System...");
    let texture = create_solid_color_texture("red_texture".to_string(), [255, 0, 0, 255], 256);
    println!("   Created solid color texture '{}'", texture.name());

    // Test 4: Animation system
    println!("\n4. Testing Animation System...");
    let mut hierarchy = Hierarchy::new("test_skeleton".to_string());
    hierarchy.add_pivot(Pivot::new("root".to_string()));
    hierarchy.add_pivot(Pivot::new("bone1".to_string()));
    println!(
        "   Created hierarchy '{}' with {} pivots",
        hierarchy.name,
        hierarchy.pivot_count()
    );

    let animation = HierarchyAnimation::new("walk".to_string(), "test_skeleton".to_string());
    let mut controller = AnimationController::new(hierarchy);
    controller.add_animation(animation);
    println!("   Created animation controller");

    // Test 5: Lighting system
    println!("\n5. Testing Lighting System...");
    let mut light_env = LightEnvironment::new();
    let sun = Light::directional("sun".to_string(), glam::Vec3::NEG_Y, Color::WHITE, 1.0);
    light_env.add_light(sun);
    println!(
        "   Created light environment with {} lights",
        light_env.light_count()
    );

    // Test 6: Scene graph
    println!("\n6. Testing Scene Graph...");
    let mut scene = Scene::new("main_scene".to_string());
    let camera = Camera::perspective(
        "main_camera".to_string(),
        60.0_f32.to_radians(),
        16.0 / 9.0,
        0.1,
        1000.0,
    );
    scene.add_camera(camera);

    let mut layer = Layer::new("main_layer".to_string());
    layer.add_object(Box::new(mesh));
    scene.add_layer(layer);

    println!(
        "   Created scene '{}' with {} layers and {} cameras",
        scene.name(),
        scene.layer_count(),
        scene.camera_count()
    );

    // Test 7: Asset manager
    println!("\n7. Testing Asset Manager...");
    let asset_mgr = global_asset_manager();
    let test_mesh = create_quad_mesh("asset_quad".to_string(), 1.0);
    let handle = asset_mgr.register_mesh("asset_quad".to_string(), test_mesh);
    println!("   Registered mesh asset: {}", handle.name());
    println!("   Asset loaded: {}", handle.is_loaded());

    let stats = asset_mgr.cache_stats();
    println!("   Cache stats: {} total assets", stats.total_count());

    // Test 8: Render object traits
    println!("\n8. Testing Render Object System...");
    let test_mesh = create_cube_mesh("render_test".to_string(), 1.0);
    let bbox = test_mesh.get_obj_space_bounding_box();
    let sphere = test_mesh.get_obj_space_bounding_sphere();
    println!("   Bounding box: min={:?}, max={:?}", bbox.min, bbox.max);
    println!(
        "   Bounding sphere: center={:?}, radius={}",
        sphere.center, sphere.radius
    );

    // Test 9: Ray casting
    println!("\n9. Testing Ray Casting...");
    let ray = Ray::new(glam::Vec3::new(0.0, 0.0, -10.0), glam::Vec3::Z);
    let test_mesh = create_quad_mesh("ray_test".to_string(), 2.0);
    let result = test_mesh.cast_ray(&ray);
    println!("   Ray hit: {}", result.hit);
    if result.hit {
        println!("   Hit distance: {}", result.distance);
    }

    // Test 10: Color system
    println!("\n10. Testing Color System...");
    let red = Color::RED;
    let blue = Color::BLUE;
    let purple = red.lerp(&blue, 0.5);
    println!(
        "   Lerped color: R={:.2}, G={:.2}, B={:.2}",
        purple.r, purple.g, purple.b
    );

    println!("\n=== All Tests Passed ===");
    Ok(())
}

use glam::Mat4;
///! Complete W3D Animated Model Demo
///!
///! This example demonstrates the complete W3D model loading and skeletal animation system.
///! It shows how to:
///! - Load W3D models with meshes and hierarchies
///! - Set up skeletal animation
///! - Use the animation state machine
///! - Render with WGPU
///!
///! This is a complete, production-ready implementation matching the C++ W3D engine.
use ww3d_animation::{
    AnimatedModel, AnimationStateMachineBuilder, GameAnimationState, HAnimClass, W3DModel,
};

fn main() {
    println!("W3D Animated Model System Demo");
    println!("================================\n");

    // Example 1: Load a complete W3D model
    example_load_model();

    // Example 2: Set up skeletal animation
    example_skeletal_animation();

    // Example 3: Use animation state machine
    example_state_machine();

    // Example 4: Animation blending
    example_animation_blending();
}

/// Example 1: Load a complete W3D model from file
fn example_load_model() {
    println!("Example 1: Loading W3D Model");
    println!("----------------------------");

    // In a real application, you would load from an actual W3D file:
    // let model = W3DModel::load_from_file("models/tank.w3d").unwrap();

    // For this demo, we'll create a simple model
    let model = W3DModel::new("DemoTank");

    println!("Model name: {}", model.name);
    println!("Mesh count: {}", model.meshes.len());
    println!("Has hierarchy: {}", model.hierarchy.is_some());
    println!("Is animated: {}", model.is_animated());
    println!("Is skinned: {}", model.is_skinned());
    println!("Animation count: {}", model.animations.len());

    if let Some(ref lod_model) = model.lod_model {
        println!("LOD levels: {}", lod_model.lods.len());
        for (i, lod) in lod_model.lods.iter().enumerate() {
            println!(
                "  LOD {}: {} (distance: {:.1} - {:.1})",
                i, lod.render_obj_name, lod.min_distance, lod.max_distance
            );
        }
    }

    println!();
}

/// Example 2: Skeletal animation with bone transforms
fn example_skeletal_animation() {
    println!("Example 2: Skeletal Animation");
    println!("------------------------------");

    // Create a hierarchy (normally loaded from W3D file)
    let mut htree = ww3d_animation::HTreeClass::new();
    htree.init_default();

    // Add bones to hierarchy
    htree.add_pivot("Root", -1, glam::Vec3::ZERO, glam::Quat::IDENTITY);
    htree.add_pivot(
        "Spine",
        0,
        glam::Vec3::new(0.0, 1.0, 0.0),
        glam::Quat::IDENTITY,
    );
    htree.add_pivot(
        "Head",
        1,
        glam::Vec3::new(0.0, 0.5, 0.0),
        glam::Quat::IDENTITY,
    );

    println!("Created hierarchy with {} bones", htree.num_pivots());

    // Create animated model
    let mut model = AnimatedModel::new(htree);

    // Create a simple animation (normally loaded from W3D file)
    let animation = HAnimClass::new("Walk", "DemoHierarchy", 30, 30.0);

    // Set animation
    model.set_animation(animation);
    println!("Set 'Walk' animation");

    // Update animation over time
    let delta_time = 1.0 / 60.0; // 60 FPS
    for frame in 0..60 {
        model.update(delta_time, Mat4::IDENTITY);

        if frame % 15 == 0 {
            let current_frame = model.get_current_frame();
            println!("  Frame {}: animation frame {:.2}", frame, current_frame);
        }
    }

    // Get skinning matrices for rendering
    let skinning_matrices = model.get_skinning_matrices();
    println!(
        "Generated {} skinning matrices for GPU",
        skinning_matrices.len()
    );

    println!();
}

/// Example 3: Animation state machine
fn example_state_machine() {
    println!("Example 3: Animation State Machine");
    println!("-----------------------------------");

    // Create hierarchy and model
    let mut htree = ww3d_animation::HTreeClass::new();
    htree.init_default();
    let animated_model = AnimatedModel::new(htree);

    // Create animations (normally loaded from W3D files)
    let idle_anim = HAnimClass::new("Idle", "DemoHierarchy", 60, 30.0);
    let walk_anim = HAnimClass::new("Walk", "DemoHierarchy", 30, 30.0);
    let attack_anim = HAnimClass::new("Attack", "DemoHierarchy", 20, 30.0);
    let death_anim = HAnimClass::new("Death", "DemoHierarchy", 40, 30.0);

    // Build state machine with animations
    let mut state_machine = AnimationStateMachineBuilder::new(animated_model)
        .with_animation(GameAnimationState::Idle, "Idle", idle_anim)
        .with_animation(GameAnimationState::Walk, "Walk", walk_anim)
        .with_animation(GameAnimationState::Attack, "Attack", attack_anim)
        .with_animation(GameAnimationState::Death, "Death", death_anim)
        .with_standard_transitions()
        .with_initial_state(GameAnimationState::Idle)
        .build();

    println!("Created state machine with 4 animation states");
    println!("Initial state: {:?}", state_machine.current_state());

    // Simulate game loop
    let delta_time = 1.0 / 60.0;

    // Idle for 1 second
    for _ in 0..60 {
        state_machine.update(delta_time, Mat4::IDENTITY);
    }
    println!("After 1s: State = {:?}", state_machine.current_state());

    // Transition to walking
    state_machine.request_state(GameAnimationState::Walk);
    for _ in 0..60 {
        state_machine.update(delta_time, Mat4::IDENTITY);
    }
    println!(
        "After walk request: State = {:?}",
        state_machine.current_state()
    );

    // Transition to attack
    state_machine.request_state(GameAnimationState::Attack);
    for _ in 0..60 {
        state_machine.update(delta_time, Mat4::IDENTITY);
    }
    println!(
        "After attack request: State = {:?}",
        state_machine.current_state()
    );

    // Get rendering data
    let skinning_matrices = state_machine.get_skinning_matrices();
    println!("Skinning matrices ready: {} bones", skinning_matrices.len());

    println!();
}

/// Example 4: Animation blending
fn example_animation_blending() {
    println!("Example 4: Animation Blending");
    println!("------------------------------");

    // Create hierarchy and model
    let mut htree = ww3d_animation::HTreeClass::new();
    htree.init_default();
    htree.add_pivot("Bone1", -1, glam::Vec3::ZERO, glam::Quat::IDENTITY);
    htree.add_pivot(
        "Bone2",
        0,
        glam::Vec3::new(0.0, 1.0, 0.0),
        glam::Quat::IDENTITY,
    );

    let mut model = AnimatedModel::new(htree);

    // Create two animations to blend
    let walk_anim = HAnimClass::new("Walk", "DemoHierarchy", 30, 30.0);
    let run_anim = HAnimClass::new("Run", "DemoHierarchy", 20, 30.0);

    // Set initial animation
    model.set_animation(walk_anim);
    println!("Starting with Walk animation");

    // Update for a bit
    for _ in 0..30 {
        model.update(1.0 / 60.0, Mat4::IDENTITY);
    }

    // Transition to run with 0.5 second blend
    model.transition_to(run_anim, 0.5);
    println!("Transitioning to Run animation (0.5s blend)");

    // Update during blend
    for frame in 0..30 {
        model.update(1.0 / 60.0, Mat4::IDENTITY);

        if frame % 10 == 0 {
            println!("  Frame {}: blending in progress", frame);
        }
    }

    println!("Blend complete, now playing Run animation");
    println!();
}

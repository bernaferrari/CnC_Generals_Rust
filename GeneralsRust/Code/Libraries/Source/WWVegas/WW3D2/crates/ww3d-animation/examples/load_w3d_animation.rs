//! Example: Load W3D Animation
//!
//! This example demonstrates how to use the W3D animation loader to load
//! and parse animation files from the W3D format.

use ww3d_animation::{
    load_w3d_animation_from_file, load_w3d_hierarchy_from_file, w3d_animation_to_hanim,
};

fn main() {
    println!("W3D Animation Loader Example");
    println!("============================\n");

    // Example 1: Load a hierarchy (skeleton)
    println!("Example 1: Loading hierarchy from W3D file");
    println!("------------------------------------------");
    match load_w3d_hierarchy_from_file("assets/models/tank.w3d") {
        Ok(hierarchy) => {
            println!("Successfully loaded hierarchy: {}", hierarchy.name);
            println!("Number of pivots (bones): {}", hierarchy.pivots.len());
            for (idx, pivot) in hierarchy.pivots.iter().enumerate() {
                println!("  Pivot {}: {}", idx, pivot.name);
            }
        }
        Err(e) => {
            println!(
                "Failed to load hierarchy (this is expected if file doesn't exist): {}",
                e
            );
            println!("This is a demonstration example. In production, provide valid W3D files.");
        }
    }

    println!();

    // Example 2: Load an animation
    println!("Example 2: Loading animation from W3D file");
    println!("------------------------------------------");
    match load_w3d_animation_from_file("assets/animations/tank_move.w3d") {
        Ok(anim_data) => {
            println!("Successfully loaded animation: {}", anim_data.name);
            println!("Hierarchy: {}", anim_data.hierarchy_name);
            println!("Frames: {}", anim_data.num_frames);
            println!("Frame rate: {} FPS", anim_data.frame_rate);
            println!("Channels: {}", anim_data.channels.len());

            for (idx, channel) in anim_data.channels.iter().enumerate() {
                println!(
                    "  Channel {}: Pivot {} | Type: {:?} | Frames: {}-{}",
                    idx,
                    channel.pivot_index,
                    channel.channel_type,
                    channel.first_frame,
                    channel.last_frame
                );
            }

            // Convert to HAnimClass for use in the animation system
            let hanim = w3d_animation_to_hanim(anim_data);
            println!("\nConverted to HAnimClass: {}", hanim.get_name());
        }
        Err(e) => {
            println!(
                "Failed to load animation (this is expected if file doesn't exist): {}",
                e
            );
            println!("This is a demonstration example. In production, provide valid W3D files.");
        }
    }

    println!();

    // Example 3: Usage pattern
    println!("Example 3: Typical usage pattern");
    println!("---------------------------------");
    println!("1. Load hierarchy: let hierarchy = load_w3d_hierarchy_from_file(path)?;");
    println!("2. Load animation: let anim_data = load_w3d_animation_from_file(path)?;");
    println!("3. Convert to HAnimClass: let hanim = w3d_animation_to_hanim(anim_data);");
    println!("4. Use with HAnimCombo for blending and playback");
    println!();
    println!("Supported features:");
    println!("  - Uncompressed animations (W3D_CHUNK_ANIMATION)");
    println!("  - Compressed animations (W3D_CHUNK_COMPRESSED_ANIMATION)");
    println!("  - Time-coded channels");
    println!("  - Bit channels (visibility)");
    println!("  - Adaptive delta encoding");
    println!("  - Hierarchy/skeleton loading");
}

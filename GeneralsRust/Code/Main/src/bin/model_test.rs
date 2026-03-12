/*
** Test W3D model loading from BIG files
*/

use anyhow::Result;
use generals_main::assets::{archive::ArchiveFileSystem, models::W3DLoader};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("🎯 Testing W3D model loading from BIG files");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;

    // Create W3D loader
    let loader = W3DLoader::new();

    // Test models we confirmed exist
    let test_models = [
        "avhummer",   // Humvee - confirmed exists
        "uvscorpion", // Scorpion tank - confirmed exists
        "nvhelix",    // Helix helicopter - confirmed exists
        "nvmign",     // MiG fighter - confirmed exists
    ];

    for model_name in &test_models {
        println!("\n🔍 Testing model: {}", model_name);

        match loader.load_model(&mut archive_system, model_name).await {
            Ok(model) => {
                println!("✅ Successfully loaded: {}", model.name);
                println!("   Meshes: {}", model.meshes.len());

                let total_vertices: usize = model.meshes.iter().map(|m| m.vertices.len()).sum();
                let total_indices: usize = model.meshes.iter().map(|m| m.indices.len()).sum();
                let triangle_count = total_indices / 3;

                println!("   Total vertices: {}", total_vertices);
                println!("   Total triangles: {}", triangle_count);

                if triangle_count > 0 {
                    println!("   ✅ Has real geometry!");
                } else {
                    println!("   ⚠️ No triangles - might be fallback");
                }

                // Show mesh details
                for (i, mesh) in model.meshes.iter().enumerate() {
                    println!(
                        "     Mesh {}: '{}' - {} verts, {} tris",
                        i,
                        mesh.name,
                        mesh.vertices.len(),
                        mesh.indices.len() / 3
                    );
                    if let Some(texture) = &mesh.material.texture_name {
                        println!("       Texture: {}", texture);
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", model_name, e);
            }
        }
    }

    // Also test the high-level CnC model loader
    println!("\n🎯 Testing high-level CnC model loader:");

    let cnc_units = ["humvee", "scorpion", "helix", "mig"];
    for unit in &cnc_units {
        println!("\n🔍 Loading CnC unit: {}", unit);

        match loader.load_cnc_model(&mut archive_system, unit).await {
            Ok(model) => {
                let total_vertices: usize = model.meshes.iter().map(|m| m.vertices.len()).sum();
                let total_triangles: usize =
                    model.meshes.iter().map(|m| m.indices.len()).sum::<usize>() / 3;

                println!(
                    "✅ Loaded: {} ({} vertices, {} triangles)",
                    model.name, total_vertices, total_triangles
                );
            }
            Err(e) => {
                println!("❌ Failed: {}", e);
            }
        }
    }

    Ok(())
}

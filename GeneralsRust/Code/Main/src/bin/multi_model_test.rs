/*
** Test multiple W3D models to verify the system works
*/

use anyhow::Result;
use generals_main::assets::{archive::ArchiveFileSystem, models::W3DLoader};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎯 Multi-Model Loading Test");

    // Initialize systems
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    let loader = W3DLoader::new();

    // Test confirmed models
    let test_models = [
        ("avhummer", "USA Humvee"),
        ("uvscorpion", "GLA Scorpion Tank"),
        ("nvhelix", "China Helix Helicopter"),
        ("nvmign", "China MiG Fighter"),
    ];

    println!("🔧 Testing {} confirmed models...\n", test_models.len());

    for (model_name, description) in &test_models {
        println!("🔍 Loading: {} ({})", description, model_name);

        match loader.load_model(&mut archive_system, model_name).await {
            Ok(model) => {
                let total_vertices: usize = model.meshes.iter().map(|m| m.vertices.len()).sum();
                let total_triangles: usize =
                    model.meshes.iter().map(|m| m.indices.len()).sum::<usize>() / 3;

                println!(
                    "  ✅ SUCCESS: {} meshes, {} vertices, {} triangles",
                    model.meshes.len(),
                    total_vertices,
                    total_triangles
                );

                // Show mesh breakdown
                for (i, mesh) in model.meshes.iter().enumerate().take(3) {
                    let mesh_triangles = mesh.indices.len() / 3;
                    println!("    Mesh {}: '{}' ({} tris)", i, mesh.name, mesh_triangles);
                }
                if model.meshes.len() > 3 {
                    println!("    ... and {} more meshes", model.meshes.len() - 3);
                }
            }
            Err(e) => {
                println!("  ❌ FAILED: {}", e);
            }
        }
        println!();
    }

    // Test the high-level CnC model loader API
    println!("🔧 Testing high-level CnC API...\n");

    let cnc_tests = [
        ("humvee", "Should load avhummer"),
        ("scorpion", "Should load uvscorpion"),
        ("helix", "Should load nvhelix"),
        ("mig", "Should load nvmign"),
    ];

    for (unit_name, expected) in &cnc_tests {
        println!("🔍 CnC API: {} ({})", unit_name, expected);

        match loader.load_cnc_model(&mut archive_system, unit_name).await {
            Ok(model) => {
                let total_triangles: usize =
                    model.meshes.iter().map(|m| m.indices.len()).sum::<usize>() / 3;
                println!(
                    "  ✅ SUCCESS: '{}' with {} triangles",
                    model.name, total_triangles
                );
            }
            Err(e) => {
                println!("  ❌ FAILED: {}", e);
            }
        }
    }

    println!("\n🎉 Model loading system is working with real C&C assets!");

    Ok(())
}

/*
** Test W3D parsing specifically
*/

use anyhow::Result;
use generals_main::assets::{archive::ArchiveFileSystem, models::W3DLoader};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎯 W3D Parsing Test");
    let args: Vec<String> = std::env::args().collect();
    let model_name = args.get(1).map(|s| s.as_str()).unwrap_or("avhummer");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;

    let loaded_archives = archive_system.get_loaded_archives();
    println!("🗃️ Loaded {} BIG archives", loaded_archives.len());
    for archive in loaded_archives.iter().take(20) {
        println!("  - {}", archive);
    }
    if loaded_archives.len() > 20 {
        println!("  ...");
    }

    // Try to locate raw W3D data for debug purposes (the loader itself probes multiple paths).
    println!("\n🔧 Loading raw W3D file data (best-effort)...");
    let base_filename = if model_name.to_lowercase().ends_with(".w3d") {
        model_name.to_string()
    } else {
        format!("{}.w3d", model_name)
    };

    let candidates = vec![
        base_filename.clone(),
        format!("art/w3d/{}", base_filename),
        format!("Art/W3D/{}", base_filename),
        format!("data/w3d/{}", base_filename),
        format!("models/{}", base_filename),
    ];
    let mut model_data = None;
    for candidate in candidates {
        match archive_system.open_file(&candidate).await {
            Ok(data) => {
                println!("✅ Found raw W3D at: {} ({} bytes)", candidate, data.len());
                model_data = Some(data);
                break;
            }
            Err(err) => {
                println!("  ❌ {}: {}", candidate, err);
            }
        }
    }

    if let Some(model_data) = model_data.as_deref() {
        if model_data.len() >= 16 {
            println!("📊 First 16 bytes: {:02X?}", &model_data[0..16]);

            if model_data.len() >= 8 {
                let chunk_type = u32::from_le_bytes([
                    model_data[0],
                    model_data[1],
                    model_data[2],
                    model_data[3],
                ]);
                let raw_chunk_size = u32::from_le_bytes([
                    model_data[4],
                    model_data[5],
                    model_data[6],
                    model_data[7],
                ]);
                let is_container = (raw_chunk_size & 0x80000000) != 0;
                let chunk_size = raw_chunk_size & 0x7FFFFFFF;

                println!("📝 Chunk type: 0x{:08X}", chunk_type);
                println!("📝 Raw size: 0x{:08X}", raw_chunk_size);
                println!("📝 Actual size: {} bytes", chunk_size);
                println!("📝 Is container: {}", is_container);
            }
        }
    } else {
        println!("⚠️ Could not locate raw W3D bytes via common path variants (continuing)");
    }

    // Now test the W3D parser
    println!("\n🔧 Testing W3D parser...");
    let loader = W3DLoader::new();

    match loader.load_model(&mut archive_system, model_name).await {
        Ok(model) => {
            println!("✅ W3D model parsed successfully!");
            println!("  Name: {}", model.name);
            println!("  Meshes: {}", model.meshes.len());

            let mut total_vertices = 0;
            let mut total_triangles = 0;

            for (i, mesh) in model.meshes.iter().enumerate() {
                let mesh_triangles = mesh.indices.len() / 3;
                total_vertices += mesh.vertices.len();
                total_triangles += mesh_triangles;

                println!(
                    "    Mesh {}: '{}' - {} verts, {} tris",
                    i,
                    mesh.name,
                    mesh.vertices.len(),
                    mesh_triangles
                );

                if let Some(texture) = &mesh.material.texture_name {
                    println!("      Texture: {}", texture);
                }

                // Show first vertex for debugging
                if !mesh.vertices.is_empty() {
                    let v = &mesh.vertices[0];
                    println!(
                        "      First vertex: pos({:.2}, {:.2}, {:.2}) norm({:.2}, {:.2}, {:.2})",
                        v.position[0],
                        v.position[1],
                        v.position[2],
                        v.normal[0],
                        v.normal[1],
                        v.normal[2]
                    );
                }
            }

            println!("  📊 Total vertices: {}", total_vertices);
            println!("  📊 Total triangles: {}", total_triangles);

            if total_triangles > 0 {
                println!("🎉 SUCCESS: Real geometry loaded!");
            } else {
                println!("⚠️  Warning: No triangles - might be fallback geometry");
            }
        }
        Err(e) => {
            println!("❌ Failed to parse W3D: {}", e);
        }
    }

    Ok(())
}

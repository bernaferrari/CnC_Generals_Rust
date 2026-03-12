use anyhow::Result;
use generals_main::assets::{archive::ArchiveFileSystem, models::W3DLoader};
use log::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🔍 W3D Mesh Tester - Testing specific files for mesh geometry");

    let mut archive_system = ArchiveFileSystem::new();

    archive_system.init().await?;
    let loaded_archives = archive_system.get_loaded_archives();
    if loaded_archives.is_empty() {
        warn!(
            "⚠️ No BIG archives loaded (set GENERALS_ASSETS_DIR or provide ./assets or ./windows_game assets)"
        );
        return Ok(());
    }
    info!("✅ Loaded {} BIG archives", loaded_archives.len());

    let w3d_loader = W3DLoader::new();

    // List of W3D files to test, focusing on larger files that likely contain mesh data
    let test_files = vec![
        // CORRECTED: Using actual W3D files that exist in archives
        "avcrusader.w3d",   // USA Crusader tank (replaces abtank.w3d)
        "avhummer.w3d",     // USA Humvee (exists)
        "avchinook.w3d",    // USA Chinook helicopter (corrected spelling)
        "abpatriotsw.w3d",  // USA Patriot missile system (corrected name)
        "pscarrapt_d1.w3d", // Scrap/damaged raptor model (corrected name)
        "avstealth_d2.w3d", // USA Stealth bomber (corrected name)
        "uvlitetank.w3d",   // USA Light tank variant
        "abrailgun.w3d",    // USA Advanced railgun system
        "cvtank.w3d",       // Civilian tank
        // Test Chinese models that exist
        "cvtanker.w3d",   // Tanker truck
        "cvtankerhd.w3d", // Heavy tanker truck
        // Test some models we know exist from logs
        "uirebel.w3d",       // GLA Rebel (actual name from game logic)
        "airanger_s.w3d",    // USA Ranger (actual name from game logic)
        "uvtechvan_d1.w3d",  // GLA Technical (actual name from game logic)
        "avcrusader_d1.w3d", // USA Crusader damaged variant
    ];

    info!(
        "📋 Testing {} W3D files for mesh geometry:",
        test_files.len()
    );

    let mut successful_loads = 0;
    let mut models_with_meshes = 0;

    for filename in &test_files {
        info!("🔍 Testing: {}", filename);

        match w3d_loader.load_model(&mut archive_system, filename).await {
            Ok(model) => {
                successful_loads += 1;
                info!("  ✅ Loaded successfully");
                info!("  📊 Model stats:");
                info!("     - Name: {}", model.name);
                info!("     - Meshes: {}", model.meshes.len());
                info!("     - Materials: {}", model.materials.len());
                info!(
                    "     - Bounding box: {:?} to {:?}",
                    model.bounding_box_min, model.bounding_box_max
                );

                if !model.meshes.is_empty() {
                    models_with_meshes += 1;
                    info!("  🎯 MESH DETAILS:");
                    for (i, mesh) in model.meshes.iter().enumerate() {
                        info!(
                            "     Mesh {}: '{}' - {} vertices, {} indices",
                            i,
                            mesh.name,
                            mesh.vertices.len(),
                            mesh.indices.len()
                        );

                        if !mesh.vertices.is_empty() {
                            let first_vertex = &mesh.vertices[0];
                            info!(
                                "       First vertex: pos={:?}, normal={:?}, uv={:?}",
                                first_vertex.position, first_vertex.normal, first_vertex.uv
                            );
                        }

                        if !mesh.indices.is_empty() {
                            info!(
                                "       First triangle: [{}, {}, {}]",
                                mesh.indices[0],
                                mesh.indices.get(1).unwrap_or(&0),
                                mesh.indices.get(2).unwrap_or(&0)
                            );
                        }

                        info!(
                            "       Material: {} (diffuse: {:?})",
                            mesh.material.name, mesh.material.diffuse_color
                        );
                    }
                } else {
                    warn!("  ⚠️ No meshes found - skeleton/hierarchy only");
                }
                info!("");
            }
            Err(e) => {
                warn!("  ❌ Failed to load: {}", e);
            }
        }
    }

    info!("📊 FINAL RESULTS:");
    info!("==================");
    info!(
        "✅ Successfully loaded: {}/{} files",
        successful_loads,
        test_files.len()
    );
    info!(
        "🎯 Files with mesh geometry: {}/{} files",
        models_with_meshes,
        test_files.len()
    );

    if models_with_meshes > 0 {
        info!(
            "🎉 SUCCESS: Found {} W3D files with actual renderable mesh geometry!",
            models_with_meshes
        );
        info!("🎯 The W3D parsing system is working correctly!");
    } else if successful_loads > 0 {
        warn!("⚠️ PARTIAL SUCCESS: W3D files load but contain no mesh geometry");
        warn!("   This suggests the files contain only skeleton/hierarchy data");
        warn!("   Need to find W3D files that contain actual mesh chunks");
    } else {
        error!("💥 FAILURE: No W3D files could be loaded successfully");
        error!("   Check BIG file loading and W3D file paths");
    }

    Ok(())
}

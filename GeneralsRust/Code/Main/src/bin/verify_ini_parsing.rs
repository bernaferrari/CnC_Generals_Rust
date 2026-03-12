/*
** Verify INI parsing and texture lookups
*/

use anyhow::Result;
use generals_main::assets::archive::ArchiveFileSystem;
use generals_main::assets::ww3d_asset_manager::WW3DAssetManager;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Create and initialize WW3D Asset Manager
    let mut archive_system = ArchiveFileSystem::new();

    // Add asset search paths
    archive_system.add_search_path("assets");
    archive_system.init().await?;

    // Initialize WW3D Asset Manager
    let mut manager = WW3DAssetManager::new();
    manager.initialize(&mut archive_system).await?;

    println!("\n📊 INI PARSING VERIFICATION REPORT");
    println!("{}", "=".repeat(80));

    println!("\n✅ Total objects loaded: {}", manager.object_count());

    // Test some known objects
    let test_objects = vec![
        "USA_Ranger",
        "USA_Humvee",
        "USA_CrusaderTank",
        "China_RedGuard",
        "China_BattlemasterTank",
        "GLA_Soldier",
        "GLA_Technical",
        "CommandCenter",
        "SupplyCenter",
    ];
    let test_count = test_objects.len();

    println!("\n🔍 OBJECT DEFINITION LOOKUP:");
    println!("{}", "-".repeat(80));

    let mut objects_with_textures = 0;
    let mut objects_with_models = 0;

    for obj_name in test_objects.iter() {
        if let Some(def) = manager.get_object_definition(obj_name) {
            println!("\n📦 {}", obj_name);
            println!("   Type: {}", def.object_type);

            if let Some(model) = &def.model_name {
                println!("   Model: {}", model);
                objects_with_models += 1;
            }

            if !def.textures.is_empty() {
                println!("   Textures:");
                for (slot, texture) in &def.textures {
                    println!("     [{}]: {}", slot, texture);
                }
                objects_with_textures += 1;
            } else {
                println!("   Textures: (none defined in INI)");
            }

            if let Some(hp) = def.hit_points {
                println!("   Hit Points: {}", hp);
            }
        } else {
            println!("\n❌ {} - NOT FOUND", obj_name);
        }
    }

    println!("\n\n📈 STATISTICS:");
    println!("{}", "-".repeat(80));
    println!(
        "✅ Objects with texture definitions: {}/{}",
        objects_with_textures, test_count
    );
    println!(
        "✅ Objects with model definitions: {}/{}",
        objects_with_models, test_count
    );

    // Get all objects and analyze
    let all_defs = manager.get_all_objects();
    let mut with_models = 0;
    let mut with_textures = 0;
    let mut with_both = 0;

    for def in all_defs.values() {
        if def.model_name.is_some() {
            with_models += 1;
        }
        if !def.textures.is_empty() {
            with_textures += 1;
        }
        if def.model_name.is_some() && !def.textures.is_empty() {
            with_both += 1;
        }
    }

    println!("\n🎯 COMPLETE INVENTORY:");
    println!("{}", "-".repeat(80));
    println!("Total objects: {}", all_defs.len());
    println!(
        "With model definitions: {} ({:.1}%)",
        with_models,
        (with_models as f64 / all_defs.len() as f64) * 100.0
    );
    println!(
        "With texture definitions: {} ({:.1}%)",
        with_textures,
        (with_textures as f64 / all_defs.len() as f64) * 100.0
    );
    println!(
        "With BOTH model and texture: {} ({:.1}%)",
        with_both,
        (with_both as f64 / all_defs.len() as f64) * 100.0
    );

    println!("\n\n✨ SAMPLE OBJECTS WITH COMPLETE DEFINITIONS:");
    println!("{}", "-".repeat(80));

    let mut count = 0;
    for (name, def) in all_defs.iter() {
        if def.model_name.is_some() && !def.textures.is_empty() && count < 10 {
            println!("\n{}:", name);
            if let Some(model) = &def.model_name {
                println!("  Model: {}", model);
            }
            if let Some(tex) = def.get_primary_texture() {
                println!("  Texture: {}", tex);
            }
            count += 1;
        }
    }

    println!("\n\n✅ VERIFICATION COMPLETE");
    println!("{}", "=".repeat(80));

    Ok(())
}

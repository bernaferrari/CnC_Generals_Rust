/*
** List loaded objects from INI parsing
*/

use anyhow::Result;
use generals_main::assets::archive::ArchiveFileSystem;
use generals_main::assets::ww3d_asset_manager::WW3DAssetManager;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Create and initialize WW3D Asset Manager
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.add_search_path("assets");
    archive_system.init().await?;

    // Initialize WW3D Asset Manager
    let mut manager = WW3DAssetManager::new();
    manager.initialize(&mut archive_system).await?;

    println!("\n📊 LOADED OBJECTS REPORT");
    println!("{}", "=".repeat(100));

    let all_defs = manager.get_all_objects();

    // Show first 20 objects with models
    println!("\n🎯 First 20 objects WITH MODELS:");
    println!("{}", "-".repeat(100));
    let mut count = 0;
    for (name, def) in all_defs.iter() {
        if def.model_name.is_some() && count < 20 {
            println!(
                "  {:<40} | Type: {:<15} | Model: {}",
                name,
                def.object_type,
                def.model_name.as_ref().unwrap_or(&"".to_string())
            );
            count += 1;
        }
    }

    // Show first 10 objects with textures
    println!("\n🎨 First 10 objects WITH TEXTURES:");
    println!("{}", "-".repeat(100));
    let mut count = 0;
    for (name, def) in all_defs.iter() {
        if !def.textures.is_empty() && count < 10 {
            println!("  {}:", name);
            for (slot, texture) in &def.textures {
                println!("    [{}]: {}", slot, texture);
            }
            count += 1;
        }
    }

    // Show samples of different types
    println!("\n📋 SAMPLE OBJECTS BY TYPE:");
    println!("{}", "-".repeat(100));

    let types: std::collections::HashSet<_> =
        all_defs.values().map(|d| d.object_type.clone()).collect();
    for type_name in types.iter().take(10) {
        println!("\n  Type: {}", type_name);
        let mut count = 0;
        for (name, def) in all_defs.iter() {
            if &def.object_type == type_name && count < 3 {
                println!("    - {}", name);
                count += 1;
            }
        }
    }

    // Check for ranger, humvee, etc. with different cases
    println!("\n🔍 SEARCHING FOR COMMON UNIT NAMES:");
    println!("{}", "-".repeat(100));

    let search_terms = vec!["ranger", "humvee", "tank", "soldier", "china", "usa", "gla"];
    for term in search_terms {
        let matches: Vec<_> = all_defs
            .keys()
            .filter(|n| n.to_lowercase().contains(term))
            .take(3)
            .collect();

        if !matches.is_empty() {
            println!("\n  Containing '{}': {}", term, matches.len());
            for name in matches {
                println!("    - {}", name);
            }
        }
    }

    println!("\n\n✅ REPORT COMPLETE");
    println!("{}", "=".repeat(100));

    Ok(())
}

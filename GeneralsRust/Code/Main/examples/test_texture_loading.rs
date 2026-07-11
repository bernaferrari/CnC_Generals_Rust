/*
** Simple test to verify texture loading from archives
*/

use anyhow::Result;
use generals_main::assets::{ArchiveFileSystem, WW3DAssetManager};

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== TEXTURE LOADING TEST ===\n");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.add_search_path(std::path::PathBuf::from("assets"));
    archive_system.init().await?;

    println!("✅ Archive system initialized\n");

    // Check what's in TexturesZH.big
    println!("📦 Checking TexturesZH.big contents:");
    let all_files = archive_system.list_all_files();
    println!("Total files in archive: {}", all_files.len());

    // Search for specific texture names
    let search_terms = vec![
        "housecolor",
        "uvscorpion",
        "lightbeam",
        "zhca_airanger",
        "extnkmzl01",
    ];
    let mut found_textures: Vec<String> = Vec::new();

    for file in &all_files {
        let file_lower = file.to_lowercase();
        for term in &search_terms {
            if file_lower.contains(term) {
                found_textures.push(file.clone());
                break;
            }
        }
    }

    if found_textures.is_empty() {
        println!("No matching texture files found for search terms");
    } else {
        println!("Found {} matching texture files:", found_textures.len());
        for tex in &found_textures {
            println!("{}", tex);
        }
    }

    // Also show regular texture files for reference
    let texture_files: Vec<String> = archive_system
        .list_all_files()
        .into_iter()
        .filter(|f| f.to_lowercase().ends_with(".tga") || f.to_lowercase().ends_with(".dds"))
        .take(20)
        .collect();

    if !texture_files.is_empty() {
        println!("\n📋 Sample texture files (first 20):");
        for (i, tex) in texture_files.iter().enumerate() {
            println!("  {}. {}", i + 1, tex);
        }
    }

    println!("\n📋 Checking INI object definitions:");

    // Initialize WW3D Asset Manager
    let mut ww3d_manager = WW3DAssetManager::new();
    ww3d_manager.initialize(&mut archive_system).await?;

    println!(
        "✅ Loaded {} object definitions\n",
        ww3d_manager.object_count()
    );

    // Check which objects have textures
    let mut objects_with_textures = 0;
    let mut sample_objects = Vec::new();

    for name in [
        "USA_Ranger",
        "China_RedGuard",
        "GLA_Soldier",
        "USA_Humvee",
        "CommandCenter",
    ]
    .iter()
    {
        if let Some(obj_def) = ww3d_manager.get_object_definition(name) {
            println!("Object: {}", name);
            println!("  Model: {:?}", obj_def.model_name);
            println!("  Textures: {:?}", obj_def.textures);
            if let Some(tex) = obj_def.get_primary_texture() {
                println!("  Primary texture: {}", tex);
                objects_with_textures += 1;
                sample_objects.push((name.to_string(), tex.to_string()));
            } else {
                println!("  ❌ No texture defined");
            }
            println!();
        }
    }

    println!("📊 Summary:");
    println!("  Total objects with textures: {}", objects_with_textures);
    println!("  Total texture files available: {}", texture_files.len());

    // Try to actually load a texture file
    println!("\n🔍 Testing actual texture file loading:");
    if let Some(test_file) = texture_files.first() {
        println!("Attempting to load: {}", test_file);
        match archive_system.open_reader(test_file) {
            Ok(mut reader) => {
                use std::io::Read;
                let mut buffer = Vec::new();
                match reader.read_to_end(&mut buffer) {
                    Ok(_) => println!("✅ Successfully loaded {} bytes", buffer.len()),
                    Err(e) => println!("❌ Failed to read: {}", e),
                }
            }
            Err(e) => println!("❌ Failed to open: {}", e),
        }
    }

    Ok(())
}

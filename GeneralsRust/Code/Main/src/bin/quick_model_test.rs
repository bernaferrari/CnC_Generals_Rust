/*
** Quick W3D model test - step by step
*/

use anyhow::Result;
use generals_main::assets::archive::ArchiveFileSystem;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎯 Quick W3D model loading test");

    // Step 1: Initialize archive system
    println!("🔧 Step 1: Initializing archive system...");
    let mut archive_system = ArchiveFileSystem::new();

    match archive_system.init().await {
        Ok(_) => println!("✅ Archive system initialized"),
        Err(e) => {
            println!("❌ Failed to initialize archive system: {}", e);
            return Ok(());
        }
    }

    // Step 2: Check what archives are loaded
    println!("\n🔧 Step 2: Checking loaded archives...");
    let archives = archive_system.get_loaded_archives();
    for archive in &archives {
        println!("  📁 {}", archive);
    }

    if archives.is_empty() {
        println!("❌ No archives loaded!");
        return Ok(());
    }

    // Step 3: Test file existence for confirmed models
    println!("\n🔧 Step 3: Testing file existence...");
    let test_files = [
        "avhummer.w3d",
        "art/w3d/avhummer.w3d",
        "uvscorpion.w3d",
        "art/w3d/uvscorpion.w3d",
        "nvhelix.w3d",
        "art/w3d/nvhelix.w3d",
    ];

    for test_file in &test_files {
        let exists = archive_system.does_file_exist(test_file);
        println!("  {} {}", if exists { "✅" } else { "❌" }, test_file);
    }

    // Step 4: Try to load one confirmed file
    println!("\n🔧 Step 4: Attempting to load avhummer model data...");

    let model_paths = ["avhummer.w3d", "art/w3d/avhummer.w3d"];

    for model_path in &model_paths {
        println!("  🔍 Trying: {}", model_path);
        match archive_system.open_file(model_path).await {
            Ok(data) => {
                println!(
                    "  ✅ SUCCESS: Loaded {} bytes from {}",
                    data.len(),
                    model_path
                );

                // Show first few bytes
                if data.len() >= 16 {
                    println!("  📊 First 16 bytes: {:02X?}", &data[0..16]);

                    // Check if it looks like a W3D file
                    if data.len() >= 4 {
                        let signature = std::str::from_utf8(&data[0..4]).unwrap_or("????");
                        println!("  📝 File signature: '{}'", signature);
                    }
                }
                return Ok(());
            }
            Err(e) => {
                println!("  ❌ Failed: {}", e);
            }
        }
    }

    println!("❌ Could not load any test model");

    Ok(())
}

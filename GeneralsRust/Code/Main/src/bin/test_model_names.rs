/*
** Test Model Names - Check which models exist in archives
** This will help us identify which model names are causing hangs
*/

use generals_main::assets::archive::ArchiveFileSystem;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing Model Names in Archives");
    println!("==================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    println!("✅ Archive system initialized");

    // Extract all model names from game objects
    let all_models = vec![
        "abbtcmdhq",      // This one loaded successfully
        "absupplyct_a2",  // This might be the second one causing hang
        "abpwrplant_d06", // We fixed this one
        "airanger_s",
        "aimissletm",
        "avhummer",
        "avcrusader",
        "avraptorag",
        "uirebel",
        "uirguard02",
        "uvtechvan_d1",
        "uvscorpion",
        "uvlitetank",
        "nvovrlrdt",
        "nvmign",
        "abbarracks_fa",
        "abwarfact_e",
        "ubarfrccmd",
        "ubbarracksf",
        "ubsupply_f",
        "ubarmdealf",
        "abpatriotsw",
        "cbcmdhq",
        "cbbarracks",
        "cbsupply",
        "cbwarfactory",
        "cbnuclear",
        "bld_china_gattling",
        "ubstingers",
        "ubhole_a4",
    ];

    println!("\n🎯 Checking {} model files...", all_models.len());

    let mut exists_count = 0;
    let mut missing_count = 0;

    for model_name in &all_models {
        let w3d_filename = format!("{}.w3d", model_name);

        if archive_system.does_file_exist(&w3d_filename) {
            println!("✅ {} - EXISTS", model_name);
            exists_count += 1;
        } else {
            println!("❌ {} - MISSING", model_name);
            missing_count += 1;
        }

        // Small delay to avoid overwhelming the system
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    println!("\n📊 SUMMARY:");
    println!("   Found: {} models", exists_count);
    println!("   Missing: {} models", missing_count);
    println!("   Total: {} models", all_models.len());

    if missing_count > 0 {
        println!("\n⚠️  Missing models may be causing the main game to hang!");
        println!("   The game should use fallback models or skip missing ones.");
    }

    Ok(())
}

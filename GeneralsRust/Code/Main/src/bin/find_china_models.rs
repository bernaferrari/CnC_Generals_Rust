/*
** Find China Models - Search for China building models in archives
** This will help us find the correct names for China buildings
*/

use generals_main::assets::archive::ArchiveFileSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Searching for China Building Models");
    println!("====================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    println!("✅ Archive system initialized");

    // Get all .w3d files
    println!("📋 Getting list of all W3D models...");
    let w3d_files: Vec<String> = archive_system
        .list_all_files()
        .into_iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".w3d"))
        .collect();

    println!("🔍 Found {} W3D files total", w3d_files.len());

    // Search for China-related models
    let china_patterns = vec![
        "chin",  // China prefix
        "cb",    // China buildings (cb prefix)
        "ch",    // China prefix variation
        "cn",    // China prefix variation
        "nuc",   // Nuclear reactor
        "gatl",  // Gatling cannon
        "react", // Reactor
        "comm",  // Command center
        "barr",  // Barracks
        "fact",  // Factory
        "supp",  // Supply
        "power", // Power plant
    ];

    println!("\n🎯 Searching for China building models...");

    let mut china_candidates = Vec::new();

    for file in &w3d_files {
        let lowercase = file.to_lowercase();

        // Look for China-related patterns
        for pattern in &china_patterns {
            if lowercase.contains(pattern) {
                china_candidates.push(file.clone());
                break;
            }
        }
    }

    // Remove duplicates and sort
    china_candidates.sort();
    china_candidates.dedup();

    println!(
        "🏭 Found {} potential China building models:",
        china_candidates.len()
    );

    for model in &china_candidates {
        println!("   {}", model);
    }

    println!("\n🏢 Specific building type searches:");

    // Command Center
    let cmd_models: Vec<String> = w3d_files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("cmd") || lower.contains("comm") || lower.contains("hq")
        })
        .cloned()
        .collect();
    println!("Command Centers: {:?}", cmd_models);

    // Barracks
    let barr_models: Vec<String> = w3d_files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("barr")
        })
        .cloned()
        .collect();
    println!("Barracks: {:?}", barr_models);

    // Supply Centers
    let supply_models: Vec<String> = w3d_files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("supp")
        })
        .cloned()
        .collect();
    println!("Supply Centers: {:?}", supply_models);

    // War Factories
    let factory_models: Vec<String> = w3d_files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("fact") || lower.contains("war")
        })
        .cloned()
        .collect();
    println!("Factories: {:?}", factory_models);

    // Power/Nuclear
    let power_models: Vec<String> = w3d_files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("pow") || lower.contains("nuc") || lower.contains("react")
        })
        .cloned()
        .collect();
    println!("Power Plants: {:?}", power_models);

    // Gatling/Defense
    let defense_models: Vec<String> = w3d_files
        .iter()
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("gatl") || lower.contains("def") || lower.contains("turr")
        })
        .cloned()
        .collect();
    println!("Defense: {:?}", defense_models);

    Ok(())
}

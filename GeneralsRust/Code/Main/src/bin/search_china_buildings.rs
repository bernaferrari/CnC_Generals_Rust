/*
** Search China Buildings - Find China building models in BIG archives
** This will help us find the ACTUAL China building model names
*/

use generals_main::assets::archive::ArchiveFileSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Searching for China Building Models in BIG Archives");
    println!("====================================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    println!("✅ Archive system initialized");

    // Search for China building patterns
    let china_patterns = vec![
        ("Command Centers", vec!["command", "cmd", "hq"]),
        ("Barracks", vec!["barracks", "barr", "infantry"]),
        ("Supply Centers", vec!["supply", "depot"]),
        ("War Factories", vec!["war", "factory", "vehicle"]),
        ("Power Plants", vec!["power", "nuclear", "reactor", "nuke"]),
        ("Defenses", vec!["gatling", "gattling", "defense", "turret"]),
    ];

    for (building_type, patterns) in china_patterns {
        println!("\n🏢 Searching for China {}:", building_type);

        for pattern in patterns {
            // Search for models that might be China buildings
            let search_patterns = vec![
                format!("cb{}", pattern),    // cb + pattern
                format!("china{}", pattern), // china + pattern
                format!("c{}", pattern),     // c + pattern
                format!("{}china", pattern), // pattern + china
            ];

            for search_pattern in search_patterns {
                let w3d_filename = format!("{}.w3d", search_pattern);
                if archive_system.does_file_exist(&w3d_filename) {
                    println!("   ✅ FOUND: {} ({})", search_pattern, building_type);
                }
            }
        }
    }

    // Also search for any files that contain "cb" (China Building prefix)
    println!("\n🎯 All models starting with 'cb' (China Building prefix):");

    // We'll check common China building prefixes
    for i in 0..20 {
        for prefix in &["cb", "ch", "cn"] {
            for suffix in &[
                "", "a", "b", "c", "d", "e", "_a", "_b", "_c", "_d", "_e", "_f", "_g",
            ] {
                let model_name = if i == 0 {
                    format!("{}{}", prefix, suffix)
                } else {
                    format!("{}{:02}{}", prefix, i, suffix)
                };

                let w3d_filename = format!("{}.w3d", model_name);
                if archive_system.does_file_exist(&w3d_filename) {
                    println!("   ✅ Found: {}", model_name);
                }
            }
        }
    }

    println!("\n📋 Summary: Search complete!");
    println!("   If no China building models were found, they may use different naming");
    println!("   or may be defined differently in the original C++ code.");

    Ok(())
}

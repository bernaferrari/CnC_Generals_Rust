use generals_main::assets::archive::ArchiveFileSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔍 C&C Generals BIG File Search Tool");
    println!("====================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;

    // Search terms
    let search_terms = vec![
        "hummer",
        "china",
        "battlemaster",
        "avhummer",
        "uvhummer",
        "gla_technical",
        "usa_humvee",
        "china_battlemaster",
    ];

    println!("\n🔍 Searching for models containing these terms:");
    for term in &search_terms {
        println!("  - {}", term);
    }

    // Get all files from archives
    let all_files = archive_system.list_all_files();

    println!("\n📊 Total files in all archives: {}", all_files.len());

    // Search for matching files
    println!("\n🎯 Matching files found:");
    let mut found_count = 0;

    for search_term in &search_terms {
        println!("\n--- Files containing '{}' ---", search_term);
        let mut term_found = false;

        for file in &all_files {
            if file.to_lowercase().contains(&search_term.to_lowercase()) {
                println!("  ✅ {}", file);
                found_count += 1;
                term_found = true;
            }
        }

        if !term_found {
            println!("  ❌ No files found for '{}'", search_term);
        }
    }

    // Also show all W3D files
    println!("\n🎮 All W3D model files:");
    let mut w3d_count = 0;
    for file in &all_files {
        if file.to_lowercase().ends_with(".w3d") {
            println!("  📦 {}", file);
            w3d_count += 1;
        }
    }

    println!("\n📊 Summary:");
    println!("  - Total files searched: {}", all_files.len());
    println!("  - Matching files found: {}", found_count);
    println!("  - Total W3D models: {}", w3d_count);

    Ok(())
}

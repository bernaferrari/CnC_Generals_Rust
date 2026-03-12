use generals_main::assets::archive::ArchiveFileSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔧 C&C Generals Configuration File Search Tool");
    println!("===============================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;

    // Get all files from archives
    let all_files = archive_system.list_all_files();
    println!("📊 Total files in all archives: {}", all_files.len());

    // Configuration file extensions and keywords to search for
    let config_extensions = vec![".ini", ".xml", ".txt", ".cfg", ".dat"];
    let music_keywords = vec!["music", "audio", "track", "song", "theme"];

    println!("\n🔍 Searching for configuration files...");
    let mut config_files = Vec::new();

    for file in &all_files {
        let file_lower = file.to_lowercase();
        for ext in &config_extensions {
            if file_lower.ends_with(ext) {
                config_files.push(file.clone());
                break;
            }
        }
    }

    config_files.sort();
    println!("📁 Found {} configuration files:", config_files.len());

    // Group by extension
    let mut by_extension = std::collections::HashMap::new();
    for file in &config_files {
        let ext = file.split('.').last().unwrap_or("unknown");
        by_extension
            .entry(ext.to_string())
            .or_insert_with(Vec::new)
            .push(file.clone());
    }

    for (ext, files) in &by_extension {
        println!("\n--- .{} files ({}) ---", ext, files.len());
        for file in files.iter().take(20) {
            // Show first 20 files
            println!("  📄 {}", file);
        }
        if files.len() > 20 {
            println!("  ... and {} more files", files.len() - 20);
        }
    }

    // Now search through some configuration files for music references
    println!("\n🎵 Searching config files for music references...");

    let important_configs = vec![
        "data/ini/music.ini",
        "data/ini/musiczh.ini",
        "data/ini/default_settings.ini",
        "data/ini/gamedata.ini",
        "data/ini/controlbar.ini",
        "data/ini/faction.ini",
        "data/scripts/scripts.ini",
    ];

    for config_name in &important_configs {
        // Try both exact match and case-insensitive search
        let found_files: Vec<_> = all_files
            .iter()
            .filter(|f| {
                f.eq_ignore_ascii_case(config_name)
                    || f.to_lowercase().contains(&config_name.to_lowercase())
            })
            .collect();

        if !found_files.is_empty() {
            println!("\n--- Found config: {} variants ---", config_name);
            for found in found_files.iter().take(3) {
                println!("  📄 {}", found);

                // Try to load and search the file for music references
                match archive_system.open_file(found).await {
                    Ok(data) => {
                        if let Ok(content) = String::from_utf8(data) {
                            let lines: Vec<&str> = content.lines().take(100).collect(); // First 100 lines
                            let mut music_lines = Vec::new();

                            for (line_num, line) in lines.iter().enumerate() {
                                let line_lower = line.to_lowercase();
                                for keyword in &music_keywords {
                                    if line_lower.contains(keyword) {
                                        music_lines.push(format!(
                                            "    L{}: {}",
                                            line_num + 1,
                                            line.trim()
                                        ));
                                        break;
                                    }
                                }
                            }

                            if !music_lines.is_empty() {
                                println!("    🎵 Music references found:");
                                for music_line in music_lines.iter().take(10) {
                                    println!("      {}", music_line);
                                }
                                if music_lines.len() > 10 {
                                    println!(
                                        "      ... and {} more references",
                                        music_lines.len() - 10
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("    ❌ Failed to read: {}", e);
                    }
                }
            }
        }
    }

    // Search for any other files that might contain music track names
    println!("\n🔎 Searching all config files for faction music patterns...");

    let faction_patterns = vec!["usa_", "china_", "gla_", "chi_"];

    for config_file in config_files.iter().take(50) {
        // Check first 50 config files
        if let Ok(data) = archive_system.open_file(config_file).await {
            if let Ok(content) = String::from_utf8(data) {
                let content_lower = content.to_lowercase();

                let mut found_patterns = Vec::new();
                for pattern in &faction_patterns {
                    if content_lower.contains(pattern) {
                        found_patterns.push(*pattern);
                    }
                }

                if !found_patterns.is_empty() {
                    println!("\n📄 {} contains: {:?}", config_file, found_patterns);

                    // Show relevant lines
                    for line in content.lines().take(200) {
                        let line_lower = line.to_lowercase();
                        if faction_patterns.iter().any(|p| line_lower.contains(p))
                            && (line_lower.contains(".mp3")
                                || line_lower.contains("track")
                                || line_lower.contains("music"))
                        {
                            println!("    🎵 {}", line.trim());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

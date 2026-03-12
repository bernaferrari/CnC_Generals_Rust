use generals_main::assets::archive::ArchiveFileSystem;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🎵 C&C Generals Music File Search Tool");
    println!("=====================================");

    // Initialize archive system
    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;

    // Get all files from archives
    let all_files = archive_system.list_all_files();
    println!("📊 Total files in all archives: {}", all_files.len());

    // Music file extensions to search for
    let music_extensions = vec![".mp3", ".ogg", ".wav"];

    // Expected music files from the Rust code
    let expected_music_files = vec![
        "usa01.mp3",
        "usa02.mp3",
        "usa03.mp3",
        "usa04.mp3",
        "usa05.mp3",
        "china01.mp3",
        "china02.mp3",
        "china03.mp3",
        "china04.mp3",
        "china05.mp3",
        "gla01.mp3",
        "gla02.mp3",
        "gla03.mp3",
        "gla04.mp3",
        "gla05.mp3",
        "USA01.mp3",
        "USA02.mp3",
        "USA03.mp3",
        "USA04.mp3",
        "USA05.mp3",
        "China01.mp3",
        "China02.mp3",
        "China03.mp3",
        "China04.mp3",
        "China05.mp3",
        "GLA01.mp3",
        "GLA02.mp3",
        "GLA03.mp3",
        "GLA04.mp3",
        "GLA05.mp3",
        "Music01.mp3",
        "Music02.mp3",
        "Music03.mp3",
        "Music04.mp3",
        "Music05.mp3",
        "Music06.mp3",
        "Music07.mp3",
        "Music08.mp3",
        "Music09.mp3",
        "Music10.mp3",
        "MusicZH01.mp3",
        "MusicZH02.mp3",
        "MusicZH03.mp3",
        "MusicZH04.mp3",
        "MusicZH05.mp3",
    ];

    println!("\n🎼 Searching for all music files...");
    let mut all_music_files = Vec::new();

    for file in &all_files {
        let file_lower = file.to_lowercase();
        for ext in &music_extensions {
            if file_lower.ends_with(ext) {
                // Check if it's likely a music file
                if file_lower.contains("music")
                    || file_lower.contains("audio")
                    || file_lower.contains("sound")
                    || file_lower.contains("usa")
                    || file_lower.contains("china")
                    || file_lower.contains("gla")
                    || file_lower.contains("theme")
                    || file_lower.contains("song")
                    || file.len() > 0
                // Include all music files for now
                {
                    all_music_files.push(file.clone());
                    break;
                }
            }
        }
    }

    all_music_files.sort();
    println!("🎶 Found {} potential music files:", all_music_files.len());
    for music_file in &all_music_files {
        println!("  🎵 {}", music_file);
    }

    println!("\n🔍 Checking expected music files...");
    println!("Expected files vs Found files:");

    for expected in &expected_music_files {
        let found = all_files.iter().any(|f| {
            f.eq_ignore_ascii_case(expected) || f.to_lowercase().ends_with(&expected.to_lowercase())
        });

        if found {
            println!("  ✅ {} - FOUND", expected);
        } else {
            println!("  💥 {} - NOT FOUND", expected);
        }
    }

    // Look for similar file names
    println!("\n🔎 Looking for similar file names...");
    let search_patterns = vec!["usa", "china", "gla", "music"];

    for pattern in &search_patterns {
        println!("\n--- Files containing '{}' ---", pattern);
        let mut pattern_files = Vec::new();

        for file in &all_files {
            if file.to_lowercase().contains(&pattern.to_lowercase())
                && (file.to_lowercase().ends_with(".mp3")
                    || file.to_lowercase().ends_with(".ogg")
                    || file.to_lowercase().ends_with(".wav"))
            {
                pattern_files.push(file.clone());
            }
        }

        pattern_files.sort();
        if pattern_files.is_empty() {
            println!("  ❌ No files found for '{}'", pattern);
        } else {
            for file in pattern_files {
                println!("  🎵 {}", file);
            }
        }
    }

    // Show folder structure for music-related folders
    println!("\n📁 Music-related folder structure:");
    let mut folders = std::collections::HashSet::new();

    for file in &all_files {
        let file_lower = file.to_lowercase();
        if file_lower.ends_with(".mp3")
            || file_lower.ends_with(".ogg")
            || file_lower.ends_with(".wav")
        {
            if let Some(folder_end) = file.rfind('/') {
                let folder = &file[..folder_end];
                folders.insert(folder.to_string());
            }
        }
    }

    let mut folder_vec: Vec<_> = folders.into_iter().collect();
    folder_vec.sort();

    for folder in folder_vec {
        println!("  📂 {}", folder);

        // Show files in this folder
        let folder_files: Vec<_> = all_files
            .iter()
            .filter(|f| f.starts_with(&folder) && f.len() > folder.len() + 1)
            .filter(|f| {
                let file_lower = f.to_lowercase();
                file_lower.ends_with(".mp3")
                    || file_lower.ends_with(".ogg")
                    || file_lower.ends_with(".wav")
            })
            .collect();

        for file in folder_files.iter().take(10) {
            // Show first 10 files
            println!("    🎵 {}", file);
        }
        if folder_files.len() > 10 {
            println!("    ... and {} more files", folder_files.len() - 10);
        }
    }

    Ok(())
}

use anyhow::Result;
use generals_main::assets::big_file::BIGFile;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();

    println!("🎯 FINAL W3D SEARCH - Using correct path to BIG archives");
    println!("=========================================================");

    let game_dir = "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/Command & Conquer Generals Zero Hour";

    let archives = vec![
        "W3DZH.big",
        "W3DEnglishZH.big",
        "EnglishZH.big",
        "GensecZH.big",
    ];

    let mut all_unit_files = Vec::new();
    let mut files_with_mesh_data = Vec::new();

    for archive_name in archives {
        let archive_path = Path::new(game_dir).join(archive_name);

        if !archive_path.exists() {
            println!("⚠️  Archive not found: {}", archive_name);
            continue;
        }

        println!("\n📁 Opening archive: {}", archive_name);

        let mut big_file = BIGFile::new();

        match big_file.open(&archive_path).await {
            Ok(_) => {
                println!("✅ Successfully opened {}", archive_name);

                let all_files = big_file.list_files();
                let w3d_files: Vec<String> = all_files
                    .iter()
                    .filter(|f| f.to_lowercase().ends_with(".w3d"))
                    .cloned()
                    .collect();

                println!("🎯 Found {} W3D files", w3d_files.len());

                // Look for unit-related keywords
                let unit_keywords = vec![
                    "abrams",
                    "tank",
                    "humvee",
                    "chinook",
                    "patriot",
                    "ranger",
                    "soldier",
                    "guard",
                    "fighter",
                    "bomber",
                    "helicopter",
                    "jeep",
                    "vehicle",
                    "ai",
                    "av",
                    "uv",
                    "nv", // Common prefixes in C&C units
                    "ab",
                    "cb",
                    "gl",
                    "ch", // Faction prefixes
                ];

                for keyword in &unit_keywords {
                    let matching: Vec<_> = w3d_files
                        .iter()
                        .filter(|f| f.to_lowercase().contains(keyword))
                        .take(3) // Limit to avoid spam
                        .collect();

                    if !matching.is_empty() {
                        println!("\n🔍 Files containing '{}':", keyword);
                        for file in &matching {
                            println!("  - {}", file);
                            all_unit_files.push((archive_name.to_string(), (*file).clone()));

                            // Extract and analyze for mesh data
                            if let Some(file_info) = big_file.get_file_info(file) {
                                match big_file.extract_file(file).await {
                                    Ok(data) => {
                                        if analyze_for_mesh_data(&data, file) {
                                            files_with_mesh_data
                                                .push((archive_name.to_string(), (*file).clone()));
                                        }
                                    }
                                    Err(e) => {
                                        println!("    ❌ Failed to extract: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }

                // Also check some random files for mesh data
                println!("\n🔍 Checking random sample for mesh data...");
                let sample_size = std::cmp::min(20, w3d_files.len());
                let step = std::cmp::max(1, w3d_files.len() / sample_size);

                for i in (0..w3d_files.len()).step_by(step).take(sample_size) {
                    let file = &w3d_files[i];
                    if let Some(file_info) = big_file.get_file_info(file) {
                        if file_info.size > 5000 {
                            // Only check larger files
                            if let Ok(data) = big_file.extract_file(file).await {
                                if analyze_for_mesh_data(&data, file) {
                                    files_with_mesh_data
                                        .push((archive_name.to_string(), file.clone()));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ Failed to open {}: {}", archive_name, e);
            }
        }
    }

    println!("\n🎯 FINAL RESULTS");
    println!("================");
    println!("Unit-related files found: {}", all_unit_files.len());
    println!("Files with mesh data: {}", files_with_mesh_data.len());

    if !files_with_mesh_data.is_empty() {
        println!("\n✅ W3D FILES WITH MESH DATA:");
        for (i, (archive, filename)) in files_with_mesh_data.iter().enumerate() {
            println!("  {:2}: {} -> {}", i + 1, archive, filename);
        }
    }

    if !all_unit_files.is_empty() {
        println!("\n📋 ALL UNIT-RELATED FILES:");
        for (i, (archive, filename)) in all_unit_files.iter().enumerate() {
            println!("  {:2}: {} -> {}", i + 1, archive, filename);
            if i >= 20 {
                // Limit output
                println!("  ... and {} more files", all_unit_files.len() - 21);
                break;
            }
        }
    }

    // Make specific recommendations
    println!("\n💡 RECOMMENDATIONS:");
    if files_with_mesh_data.is_empty() {
        println!("❌ No files with detectable mesh data were found.");
        println!("📊 This could mean:");
        println!("  1. The W3D format uses different chunk IDs than expected");
        println!("  2. Mesh data is stored in a compressed or encoded format");
        println!("  3. The models are split across multiple files");
        println!("  4. Need to check other BIG archives like TexturesZH.big");

        if !all_unit_files.is_empty() {
            println!("\n🔧 NEXT STEPS:");
            println!("  1. Try loading these unit files directly:");
            for (archive, filename) in all_unit_files.iter().take(5) {
                let short_name = filename
                    .split('/')
                    .last()
                    .unwrap_or(filename)
                    .replace(".w3d", "");
                println!("     '{}' from {}", short_name, archive);
            }
            println!("  2. Update the game to use these exact filenames");
            println!("  3. Modify the W3D parser to handle the actual chunk format");
        }
    } else {
        println!("✅ Found files with mesh data! Use these instead:");
        for (archive, filename) in files_with_mesh_data.iter().take(5) {
            let short_name = filename
                .split('/')
                .last()
                .unwrap_or(filename)
                .replace(".w3d", "");
            println!("  '{}' from {}", short_name, archive);
        }
    }

    Ok(())
}

fn analyze_for_mesh_data(data: &[u8], filename: &str) -> bool {
    if data.len() < 8 {
        return false;
    }

    let short_name = filename.split('/').last().unwrap_or(filename);
    print!("  📄 {:<25} ({:6} bytes) - ", short_name, data.len());

    // Look for any chunk signatures in the file
    let chunk_signatures = vec![
        (0x00000000u32, "MESH"),
        (0x00000002u32, "VERTICES"),
        (0x00000020u32, "TRIANGLES"),
        (0x00000B00u32, "SHDMESH"),
        (0x00000300u32, "HMODEL"),
        (0x00000400u32, "LODMODEL"),
    ];

    let mut found_chunks: Vec<String> = Vec::new();
    let mut has_mesh_indication = false;

    // Scan through the file looking for chunk signatures
    for i in 0..data.len().saturating_sub(8) {
        let chunk_type = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);

        for (sig, name) in &chunk_signatures {
            if chunk_type == *sig {
                found_chunks.push(name.to_string());

                // If we found mesh, vertices, or triangles, check the size makes sense
                if *sig == 0x00000000 || *sig == 0x00000002 || *sig == 0x00000020 {
                    let chunk_size =
                        u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);

                    // Reasonable chunk size (not too small, not larger than file)
                    if chunk_size > 12 && chunk_size < data.len() as u32 {
                        has_mesh_indication = true;
                    }
                }

                break; // Only count each chunk type once per position
            }
        }
    }

    // Remove duplicates
    found_chunks.sort();
    found_chunks.dedup();

    if has_mesh_indication {
        println!("✅ HAS MESH! Chunks: {}", found_chunks.join(", "));
        true
    } else if !found_chunks.is_empty() {
        println!(
            "❓ Has chunks: {} (no mesh detected)",
            found_chunks.join(", ")
        );
        false
    } else {
        // Maybe it's a different format - show hex signature
        let hex_sig: Vec<String> = data[0..std::cmp::min(8, data.len())]
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect();
        println!("❌ No known chunks. Sig: {}", hex_sig.join(" "));
        false
    }
}

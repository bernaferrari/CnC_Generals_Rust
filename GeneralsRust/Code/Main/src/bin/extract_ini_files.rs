/*
** Extract and display INI files from BIG archives to examine unit templates
*/

use anyhow::Result;
use generals_main::assets::big_file::BIGFile;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Set up simple logging
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <game_directory>", args[0]);
        println!(
            "Example: {} '/path/to/Command & Conquer Generals Zero Hour'",
            args[0]
        );
        return Ok(());
    }

    let game_dir = &args[1];
    let ini_big_path = format!("{}/INIZH.big", game_dir);

    println!("🔍 Extracting INI files from: {}", ini_big_path);

    let mut big_file = BIGFile::new();
    match big_file.open(&ini_big_path).await {
        Ok(_) => {
            println!("✅ Successfully opened INIZH.big");
            println!("Total files: {}", big_file.get_file_count());

            // First, list all files to understand the structure
            println!("\n📂 All files in INIZH.big:");
            let all_files = big_file.list_files();

            // Filter for INI files
            let ini_files: Vec<&String> =
                all_files.iter().filter(|f| f.ends_with(".ini")).collect();

            println!("Found {} INI files", ini_files.len());

            // Show object INI files specifically
            let object_files: Vec<&String> = ini_files
                .iter()
                .filter(|f| f.contains("object/"))
                .copied()
                .collect();

            println!("\nObject INI files ({}):", object_files.len());
            for file in &object_files {
                println!("  - {}", file);
            }

            if ini_files.len() > object_files.len() {
                println!(
                    "\nOther INI files ({}):",
                    ini_files.len() - object_files.len()
                );
                for file in &ini_files[..std::cmp::min(20, ini_files.len() - object_files.len())] {
                    if !file.contains("object/") {
                        println!("  - {}", file);
                    }
                }
            }

            // Extract first few object INI files to examine
            let target_files: Vec<String> =
                object_files.iter().take(5).map(|s| s.to_string()).collect();

            println!("\n📖 Extracting {} sample INI files:", target_files.len());
            for filename in &target_files {
                println!("\n{}", "=".repeat(80));
                println!("📋 Extracting: {}", filename);
                println!("{}", "=".repeat(80));

                match big_file.extract_file(filename).await {
                    Ok(data) => {
                        match String::from_utf8(data.clone()) {
                            Ok(content) => {
                                // Look for specific unit templates and their model references
                                let lines: Vec<&str> = content.lines().collect();
                                let mut current_object = String::new();
                                let mut found_interesting = false;

                                for (i, line) in lines.iter().enumerate() {
                                    let line = line.trim();

                                    // Look for Object sections
                                    if line.starts_with("Object ") {
                                        current_object = line.to_string();
                                        found_interesting = true;
                                        println!("\n🎯 {}", current_object);
                                    } else if line.starts_with("End") && !current_object.is_empty()
                                    {
                                        current_object.clear();
                                        found_interesting = false;
                                    } else if found_interesting
                                        && (line.contains("Draw =")
                                            || line.contains("Model =")
                                            || line.contains("W3DModel")
                                            || line.contains(".w3d")
                                            || line.contains("DefaultConditionState")
                                            || line.contains("Model ="))
                                    {
                                        println!("  📝 {}", line);
                                    }

                                    // Show a few lines of context around model references
                                    if line.contains(".w3d") {
                                        let start = if i >= 2 { i - 2 } else { 0 };
                                        let end = std::cmp::min(i + 3, lines.len());

                                        println!("\n  🔍 W3D MODEL CONTEXT:");
                                        for j in start..end {
                                            let marker = if j == i { ">>>" } else { "   " };
                                            println!("  {} {}", marker, lines[j].trim());
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                println!("❌ Failed to decode as UTF-8, showing first 1000 bytes as hex:");
                                let preview = if data.len() > 1000 {
                                    &data[..1000]
                                } else {
                                    &data
                                };
                                println!("{:02X?}", preview);
                            }
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to extract {}: {}", filename, e);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to open BIG file: {}", e);
        }
    }

    Ok(())
}

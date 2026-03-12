/*
** Quick BIG file inspector to see what W3D models are available
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
        println!("Usage: {} <big_file_path>", args[0]);
        println!("Example: {} assets/W3DZH.big", args[0]);
        return Ok(());
    }

    let big_file_path = &args[1];

    println!("🔍 Inspecting BIG file: {}", big_file_path);

    let mut big_file = BIGFile::new();
    match big_file.open(big_file_path).await {
        Ok(_) => {
            println!("✅ Successfully opened BIG file");
            println!("📊 Total files: {}", big_file.get_file_count());

            let all_files = big_file.list_files();

            // Show W3D files
            let w3d_files: Vec<_> = all_files
                .iter()
                .filter(|f| f.to_lowercase().ends_with(".w3d"))
                .collect();

            println!("\n🎯 W3D Model Files ({}):", w3d_files.len());
            for (i, file) in w3d_files.iter().enumerate() {
                println!("  {}: {}", i + 1, file);
                if i >= 50 {
                    // Limit output
                    println!("  ... and {} more W3D files", w3d_files.len() - 51);
                    break;
                }
            }

            // Show some texture files
            let tga_files: Vec<_> = all_files
                .iter()
                .filter(|f| f.to_lowercase().ends_with(".tga"))
                .take(20)
                .collect();

            println!("\n🖼️ Sample Texture Files:");
            for file in tga_files {
                println!("  {}", file);
            }

            // Look for specific unit models we're trying to load
            let target_units = [
                "gla_tank",
                "usa_humvee",
                "china_battlemaster",
                "abtank",
                "avhummer",
            ];

            println!("\n🎯 Searching for target unit models:");
            for unit in &target_units {
                let matching_files: Vec<_> = all_files
                    .iter()
                    .filter(|f| f.to_lowercase().contains(&unit.to_lowercase()))
                    .collect();

                if !matching_files.is_empty() {
                    println!("  {}: {:?}", unit, matching_files);
                } else {
                    println!("  {}: NOT FOUND", unit);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to open BIG file: {}", e);
        }
    }

    Ok(())
}

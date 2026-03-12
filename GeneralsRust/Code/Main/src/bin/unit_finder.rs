/*
** Find actual unit models in BIG files with pattern matching
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

    println!("🔍 Finding unit models in: {}", big_file_path);

    let mut big_file = BIGFile::new();
    match big_file.open(big_file_path).await {
        Ok(_) => {
            let all_files = big_file.list_files();

            // Common patterns for C&C vehicle models
            let patterns = [
                // USA patterns
                ("abrams", "ab"),   // Abrams tank
                ("humvee", "av"),   // Army vehicle
                ("paladin", "av"),  // Artillery
                ("crusader", "av"), // Tank
                ("patriot", "av"),  // AA
                ("chinook", "av"),  // Transport heli
                ("raptor", "av"),   // Fighter
                ("stealth", "av"),  // Stealth bomber
                ("comanche", "av"), // Attack helicopter
                // China patterns
                ("battlemaster", "ch"), // Main battle tank
                ("dragon", "ch"),       // Dragon tank
                ("overlord", "ch"),     // Heavy tank
                ("gattling", "ch"),     // AA tank
                ("mig", "ch"),          // Fighter
                ("helix", "ch"),        // Transport heli
                // GLA patterns
                ("marauder", "gv"),  // GLA vehicle
                ("scorpion", "gv"),  // GLA tank
                ("technical", "gv"), // Technical truck
                ("toxin", "gv"),     // Toxin tractor
                ("scud", "gv"),      // SCUD launcher
                ("quad", "gv"),      // Quad cannon
                ("tunnel", "gb"),    // GLA building
                // Generic patterns
                ("tank", ""),
                ("vehicle", ""),
                ("hummer", ""),
                ("truck", ""),
            ];

            println!("\n🎯 Searching for unit models by pattern:");

            for (unit_type, prefix_hint) in &patterns {
                let mut matches = Vec::new();

                for file in &all_files {
                    let file_lower = file.to_lowercase();

                    // Check if filename contains the unit type
                    if file_lower.contains(unit_type)
                        || (prefix_hint.len() > 0
                            && file_lower.starts_with(&format!("art/w3d/{}", prefix_hint)))
                    {
                        matches.push(file.clone());
                    }
                }

                if !matches.is_empty() {
                    println!("  {}: {} matches", unit_type, matches.len());
                    for (i, m) in matches.iter().enumerate() {
                        if i < 5 {
                            println!("    - {}", m);
                        } else if i == 5 {
                            println!("    ... and {} more", matches.len() - 5);
                            break;
                        }
                    }
                }
            }

            // Also show some random vehicle-looking files
            println!("\n📋 Sample files that look like units (by prefix):");

            let prefixes = ["av", "ch", "gv", "gb", "ab", "cb"];

            for prefix in &prefixes {
                let matches: Vec<_> = all_files
                    .iter()
                    .filter(|f| f.to_lowercase().starts_with(&format!("art/w3d/{}", prefix)))
                    .take(10)
                    .collect();

                if !matches.is_empty() {
                    println!("  {} prefix ({} total):", prefix, matches.len());
                    for m in matches.iter().take(5) {
                        println!("    {}", m);
                    }
                    if matches.len() > 5 {
                        println!("    ... and {} more", matches.len() - 5);
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

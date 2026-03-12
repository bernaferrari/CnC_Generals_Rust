use std::path::Path;
use anyhow::Result;
use generals_main::assets::big_file::BIGFile;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();
    
    println!("🔍 W3D Explorer - Searching C&C Generals BIG Archives");
    println!("================================================");
    
    let assets_dir = "assets";
    let archives = vec![
        "W3DZH.big",
        "W3DEnglishZH.big", 
        "EnglishZH.big",
        "GensecZH.big",
        "TerrainZH.big"
    ];
    
    let mut all_w3d_files = Vec::new();
    
    for archive_name in archives {
        let archive_path = Path::new(assets_dir).join(archive_name);
        
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
                let w3d_files: Vec<_> = all_files.iter()
                    .filter(|f| f.to_lowercase().ends_with(".w3d"))
                    .collect();
                
                println!("📦 Total files in {}: {}", archive_name, all_files.len());
                println!("🎯 W3D files found: {}", w3d_files.len());
                
                if w3d_files.is_empty() {
                    println!("❌ No W3D files in {}", archive_name);
                } else {
                    println!("\n📋 W3D Files in {}:", archive_name);
                    for (i, w3d_file) in w3d_files.iter().enumerate() {
                        println!("  {:3}: {}", i + 1, w3d_file);
                        
                        if let Some(file_info) = big_file.get_file_info(w3d_file) {
                            println!("       Size: {} bytes, Offset: 0x{:08X}", 
                                   file_info.size, file_info.offset);
                            
                            // Extract and examine the first few W3D files
                            if i < 5 {
                                match big_file.extract_file(w3d_file).await {
                                    Ok(data) => {
                                        println!("       📄 Extracted {} bytes", data.len());
                                        
                                        // Show first 32 bytes
                                        let preview_len = std::cmp::min(32, data.len());
                                        let hex_preview: Vec<String> = data[0..preview_len]
                                            .iter()
                                            .map(|b| format!("{:02X}", b))
                                            .collect();
                                        println!("       🔍 First {} bytes: {}", preview_len, hex_preview.join(" "));
                                        
                                        // Show ASCII interpretation
                                        let ascii_preview: String = data[0..preview_len]
                                            .iter()
                                            .map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' })
                                            .collect();
                                        println!("       📝 ASCII: '{}'", ascii_preview);
                                        
                                        // Check for chunk headers
                                        if data.len() >= 8 {
                                            let chunk_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                            let chunk_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
                                            println!("       🔧 First chunk: Type=0x{:08X}, Size={}", chunk_type, chunk_size);
                                            
                                            // Check if this looks like mesh data (chunk type 0x00000000)
                                            if chunk_type == 0x00000000 {
                                                println!("       ✅ CONTAINS MESH CHUNK!");
                                            } else if chunk_type == 0x00000100 {
                                                println!("       🦴 Contains hierarchy/skeleton data");
                                            } else if chunk_type == 0x00000B00 {
                                                println!("       🎨 Contains shader mesh data");
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        println!("       ❌ Failed to extract: {}", e);
                                    }
                                }
                            }
                        }
                        
                        all_w3d_files.push((archive_name.to_string(), w3d_file.clone()));
                    }
                }
            },
            Err(e) => {
                println!("❌ Failed to open {}: {}", archive_name, e);
            }
        }
    }
    
    println!("\n🎯 SUMMARY");
    println!("==========");
    println!("Total W3D files found: {}", all_w3d_files.len());
    
    // Look for specific unit files
    let unit_patterns = vec![
        "airanger", "glrebel", "gltechn", // Infantry
        "abtank", "avhummer", "avchinok",  // USA units
        "glbattlemaster", "gldragntank",   // GLA units
        "chbattlemaster", "chdragontank",  // China units
    ];
    
    println!("\n🔍 Looking for specific unit models:");
    for pattern in &unit_patterns {
        let found: Vec<_> = all_w3d_files.iter()
            .filter(|(_, filename)| filename.to_lowercase().contains(pattern))
            .collect();
        
        if found.is_empty() {
            println!("❌ {}: Not found", pattern);
        } else {
            println!("✅ {}:", pattern);
            for (archive, filename) in found {
                println!("    {} -> {}", archive, filename);
            }
        }
    }
    
    Ok(())
}
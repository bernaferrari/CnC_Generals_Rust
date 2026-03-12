use anyhow::Result;
use generals_main::assets::big_file::BIGFile;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();

    println!("🔬 W3D Deep Analyzer - Understanding the actual chunk structure");
    println!("===============================================================");

    let assets_dir = std::env::var("GENERALS_ASSETS_DIR").unwrap_or_else(|_| {
        if std::path::Path::new("assets").exists() {
            "assets".to_string()
        } else if std::path::Path::new("windows_game/Command & Conquer Generals Zero Hour").exists()
        {
            "windows_game/Command & Conquer Generals Zero Hour".to_string()
        } else {
            "assets".to_string()
        }
    });
    let mut big_file = BIGFile::new();

    match big_file
        .open(Path::new(&assets_dir).join("W3DZH.big"))
        .await
    {
        Ok(_) => {
            println!("✅ Successfully opened W3DZH.big");

            let all_files = big_file.list_files();
            let w3d_files: Vec<String> = all_files
                .iter()
                .filter(|f| f.to_lowercase().ends_with(".w3d"))
                .take(20) // Analyze first 20 files in detail
                .cloned()
                .collect();

            println!(
                "📊 Analyzing first {} W3D files in detail...",
                w3d_files.len()
            );

            for (i, w3d_file) in w3d_files.iter().enumerate() {
                println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("📄 FILE {}: {}", i + 1, w3d_file);

                if let Some(file_info) = big_file.get_file_info(w3d_file) {
                    println!(
                        "📏 Size: {} bytes, Offset: 0x{:08X}",
                        file_info.size, file_info.offset
                    );

                    match big_file.extract_file(w3d_file).await {
                        Ok(data) => {
                            analyze_w3d_structure(&data);
                        }
                        Err(e) => {
                            println!("❌ Failed to extract: {}", e);
                        }
                    }
                } else {
                    println!("❌ File info not found");
                }

                if i >= 5 {
                    // Limit detailed analysis to first 5 files
                    println!("⏭️  Skipping remaining files for brevity...");
                    break;
                }
            }

            // Now let's specifically look for unit-related files
            println!("\n🎯 SEARCHING FOR UNIT FILES");
            println!("============================");

            let unit_keywords = vec!["soldier", "tank", "vehicle", "unit", "infantry", "trooper"];

            for keyword in unit_keywords {
                let matching: Vec<_> = all_files
                    .iter()
                    .filter(|f| {
                        f.to_lowercase().contains(keyword) && f.to_lowercase().ends_with(".w3d")
                    })
                    .take(3)
                    .collect();

                if !matching.is_empty() {
                    println!("\n🔍 Files containing '{}':", keyword);
                    for file in matching {
                        println!("  - {}", file);

                        if let Some(file_info) = big_file.get_file_info(file) {
                            if let Ok(data) = big_file.extract_file(file).await {
                                print!("    ");
                                analyze_first_chunks(&data);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to open W3DZH.big: {}", e);
        }
    }

    Ok(())
}

fn analyze_w3d_structure(data: &[u8]) {
    if data.len() < 8 {
        println!("❌ File too small: {} bytes", data.len());
        return;
    }

    // Show raw hex dump of first 128 bytes
    println!("🔍 HEX DUMP (first 128 bytes):");
    for i in (0..std::cmp::min(128, data.len())).step_by(16) {
        let end = std::cmp::min(i + 16, data.len());
        let hex_bytes: Vec<String> = data[i..end].iter().map(|b| format!("{:02X}", b)).collect();
        let ascii_chars: String = data[i..end]
            .iter()
            .map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' })
            .collect();
        println!("  {:04X}: {:<48} | {}", i, hex_bytes.join(" "), ascii_chars);
    }

    // Try multiple parsing approaches
    println!("\n🧪 PARSING ATTEMPTS:");

    // Attempt 1: Standard little-endian chunk format
    println!("🔸 Attempt 1: Standard LE chunks");
    parse_chunks_le(data, 0);

    // Attempt 2: Big-endian chunk format
    println!("🔸 Attempt 2: Big-endian chunks");
    parse_chunks_be(data, 0);

    // Attempt 3: Skip possible header and try parsing
    if data.len() > 64 {
        println!("🔸 Attempt 3: Skip 64-byte header");
        parse_chunks_le(data, 64);
    }

    // Attempt 4: Look for known chunk signatures anywhere in file
    println!("🔸 Attempt 4: Search for chunk signatures");
    search_for_signatures(data);
}

fn analyze_first_chunks(data: &[u8]) {
    if data.len() < 8 {
        println!("File too small");
        return;
    }

    let first_chunk = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let first_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    print!("First chunk: 0x{:08X} ({} bytes)", first_chunk, first_size);

    match first_chunk {
        0x00000000 => print!(" [MESH]"),
        0x00000100 => print!(" [HIERARCHY]"),
        0x00000B00 => print!(" [SHDMESH]"),
        0x00000300 => print!(" [HMODEL]"),
        0x00000400 => print!(" [LODMODEL]"),
        _ => print!(" [UNKNOWN]"),
    }

    println!();
}

fn parse_chunks_le(data: &[u8], start_offset: usize) -> bool {
    let mut offset = start_offset;
    let mut valid_chunks = 0;
    let mut chunks_info = Vec::new();

    while offset + 8 <= data.len() && valid_chunks < 10 {
        let chunk_type = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let chunk_size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;

        // Check if this looks like a valid chunk
        if chunk_size > 0 && chunk_size <= data.len() && offset + 8 + chunk_size <= data.len() {
            let chunk_name = match chunk_type {
                0x00000000 => "MESH",
                0x00000002 => "VERTICES",
                0x00000003 => "VERTEX_NORMALS",
                0x00000005 => "TEXCOORDS",
                0x00000020 => "TRIANGLES",
                0x0000001F => "MESH_HEADER3",
                0x00000100 => "HIERARCHY",
                0x00000200 => "ANIMATION",
                0x00000300 => "HMODEL",
                0x00000400 => "LODMODEL",
                0x00000B00 => "SHDMESH",
                _ => "UNKNOWN",
            };

            chunks_info.push(format!("0x{:08X}:{}", chunk_type, chunk_name));
            valid_chunks += 1;
            offset += 8 + chunk_size;
        } else {
            break;
        }
    }

    if valid_chunks > 0 {
        println!(
            "  ✅ Found {} valid LE chunks: {}",
            valid_chunks,
            chunks_info.join(", ")
        );
        true
    } else {
        println!("  ❌ No valid LE chunks found");
        false
    }
}

fn parse_chunks_be(data: &[u8], start_offset: usize) -> bool {
    let mut offset = start_offset;
    let mut valid_chunks = 0;
    let mut chunks_info = Vec::new();

    while offset + 8 <= data.len() && valid_chunks < 10 {
        let chunk_type = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let chunk_size = u32::from_be_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;

        // Check if this looks like a valid chunk
        if chunk_size > 0 && chunk_size <= data.len() && offset + 8 + chunk_size <= data.len() {
            chunks_info.push(format!("0x{:08X}", chunk_type));
            valid_chunks += 1;
            offset += 8 + chunk_size;
        } else {
            break;
        }
    }

    if valid_chunks > 0 {
        println!(
            "  ✅ Found {} valid BE chunks: {}",
            valid_chunks,
            chunks_info.join(", ")
        );
        true
    } else {
        println!("  ❌ No valid BE chunks found");
        false
    }
}

fn search_for_signatures(data: &[u8]) {
    let signatures = vec![
        (0x00000000u32, "MESH"),
        (0x00000002u32, "VERTICES"),
        (0x00000020u32, "TRIANGLES"),
        (0x00000100u32, "HIERARCHY"),
        (0x00000B00u32, "SHDMESH"),
    ];

    let mut found_any = false;

    for (sig, name) in signatures {
        let sig_bytes = sig.to_le_bytes();
        for i in 0..data.len().saturating_sub(4) {
            if data[i..i + 4] == sig_bytes {
                println!("  🎯 Found {} signature at offset 0x{:04X}", name, i);
                found_any = true;

                // Show context around the signature
                let start = i.saturating_sub(8);
                let end = std::cmp::min(i + 16, data.len());
                let context_hex: Vec<String> = data[start..end]
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect();
                println!("    Context: {}", context_hex.join(" "));
                break; // Only show first occurrence of each signature
            }
        }
    }

    if !found_any {
        println!("  ❌ No known chunk signatures found anywhere in file");
    }
}

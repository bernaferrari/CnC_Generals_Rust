use anyhow::Result;
use generals_main::assets::big_file::BIGFile;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();

    println!("🎯 W3D Mesh Finder - Looking for files with actual mesh data");
    println!("===========================================================");

    let assets_dir = "assets";
    let archives = vec!["W3DZH.big", "W3DEnglishZH.big"];

    let mut mesh_files = Vec::new();
    let mut total_examined = 0;

    for archive_name in archives {
        let archive_path = Path::new(assets_dir).join(archive_name);

        if !archive_path.exists() {
            println!("⚠️  Archive not found: {}", archive_name);
            continue;
        }

        println!("\n📁 Examining archive: {}", archive_name);

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

                println!(
                    "🎯 Examining {} W3D files for mesh content...",
                    w3d_files.len()
                );

                let mut files_with_meshes = 0;

                // Check specific unit patterns first
                let unit_patterns = vec![
                    "airanger",
                    "abrams",
                    "tank",
                    "humvee",
                    "chinook",
                    "patriot",
                    "soldier",
                    "ranger",
                    "guard",
                    "fighter",
                    "bomber",
                    "helicopter",
                ];

                for pattern in &unit_patterns {
                    let matching_files: Vec<_> = w3d_files
                        .iter()
                        .filter(|f| f.to_lowercase().contains(pattern))
                        .take(3) // Only check first 3 matches per pattern
                        .collect();

                    if !matching_files.is_empty() {
                        println!("\n🔍 Checking '{}' pattern files:", pattern);

                        for w3d_file in matching_files {
                            total_examined += 1;

                            if let Some(file_info) = big_file.get_file_info(w3d_file) {
                                match big_file.extract_file(w3d_file).await {
                                    Ok(data) => {
                                        let has_mesh_chunk = analyze_w3d_file(&data, w3d_file);
                                        if has_mesh_chunk {
                                            mesh_files
                                                .push((archive_name.to_string(), w3d_file.clone()));
                                            files_with_meshes += 1;
                                        }
                                    }
                                    Err(e) => {
                                        println!("    ❌ Failed to extract {}: {}", w3d_file, e);
                                    }
                                }
                            }
                        }
                    }
                }

                // Now check a random sample of other files for mesh data
                println!("\n🔍 Checking random sample of other files...");
                let step_size = std::cmp::max(1, w3d_files.len() / 50); // Sample every Nth file
                for (i, w3d_file) in w3d_files.iter().enumerate().step_by(step_size) {
                    total_examined += 1;

                    if let Some(file_info) = big_file.get_file_info(w3d_file) {
                        // Skip very small files
                        if file_info.size < 1000 {
                            continue;
                        }

                        match big_file.extract_file(w3d_file).await {
                            Ok(data) => {
                                let has_mesh_chunk = analyze_w3d_file(&data, w3d_file);
                                if has_mesh_chunk {
                                    mesh_files.push((archive_name.to_string(), w3d_file.clone()));
                                    files_with_meshes += 1;
                                }
                            }
                            Err(_) => {
                                // Silently skip extraction failures for random sampling
                            }
                        }

                        if total_examined >= 100 {
                            // Limit total examination
                            break;
                        }
                    }
                }

                println!(
                    "📊 Found {} files with mesh chunks out of {} examined in {}",
                    files_with_meshes, total_examined, archive_name
                );
            }
            Err(e) => {
                println!("❌ Failed to open {}: {}", archive_name, e);
            }
        }
    }

    println!("\n🎯 FINAL RESULTS");
    println!("================");
    println!("Total files examined: {}", total_examined);
    println!("Files with mesh chunks: {}", mesh_files.len());

    if !mesh_files.is_empty() {
        println!("\n✅ W3D files with actual mesh data:");
        for (i, (archive, filename)) in mesh_files.iter().enumerate() {
            println!("  {:2}: {} -> {}", i + 1, archive, filename);
        }

        // Show some recommendations
        println!("\n💡 RECOMMENDATIONS:");
        println!("1. Try loading these files instead of the skeleton-only files");
        println!("2. These files should contain actual renderable geometry");
        println!("3. Update the model name mapping to use these files");
    } else {
        println!("\n❌ NO FILES WITH MESH DATA FOUND!");
        println!("This suggests either:");
        println!("1. The W3D format parsing is incorrect");
        println!("2. Mesh data uses different chunk types than expected");
        println!("3. The archives don't contain unit models (might be in different archives)");
        println!("\n💡 Try checking these other archives:");
        println!("   - TexturesZH.big");
        println!("   - TerrainZH.big");
        println!("   - EnglishZH.big");
    }

    Ok(())
}

fn analyze_w3d_file(data: &[u8], filename: &str) -> bool {
    if data.len() < 8 {
        return false;
    }

    print!(
        "  📄 {:<30} ({:6} bytes) - ",
        filename.split('/').next_back().unwrap_or(filename),
        data.len()
    );

    // Check first chunk
    let first_chunk_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let first_chunk_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    let mut has_mesh_data = false;
    let mut chunk_types = Vec::new();
    let mut offset = 0;
    let mut chunks_examined = 0;

    // Scan through chunks looking for mesh data
    while offset + 8 <= data.len() && chunks_examined < 20 {
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

        chunk_types.push(chunk_type);
        chunks_examined += 1;

        // Check for chunk sizes that make sense
        if chunk_size == 0 || chunk_size > data.len() || offset + 8 + chunk_size > data.len() {
            break;
        }

        match chunk_type {
            0x00000000 => {
                // W3D_CHUNK_MESH
                // Look inside the mesh chunk for vertex/triangle data
                let mesh_data = &data[offset + 8..offset + 8 + chunk_size];
                if has_vertex_data(mesh_data) {
                    has_mesh_data = true;
                    break;
                }
            }
            0x00000B00 => {
                // W3D_CHUNK_SHDMESH (Shader mesh)
                let shdmesh_data = &data[offset + 8..offset + 8 + chunk_size];
                if has_vertex_data(shdmesh_data) {
                    has_mesh_data = true;
                    break;
                }
            }
            0x00000300 => {
                // W3D_CHUNK_HMODEL
                let hmodel_data = &data[offset + 8..offset + 8 + chunk_size];
                if has_vertex_data(hmodel_data) {
                    has_mesh_data = true;
                    break;
                }
            }
            0x00000400 => {
                // W3D_CHUNK_LODMODEL
                let lodmodel_data = &data[offset + 8..offset + 8 + chunk_size];
                if has_vertex_data(lodmodel_data) {
                    has_mesh_data = true;
                    break;
                }
            }
            _ => {}
        }

        offset += 8 + chunk_size;
    }

    let chunk_summary = chunk_types
        .iter()
        .map(|t| format!("{:08X}", t))
        .collect::<Vec<_>>()
        .join(" ");

    if has_mesh_data {
        println!("✅ HAS MESH DATA! Chunks: {}", chunk_summary);
    } else {
        println!("❌ No mesh data. Chunks: {}", chunk_summary);
    }

    has_mesh_data
}

fn has_vertex_data(chunk_data: &[u8]) -> bool {
    let mut offset = 0;
    let mut has_vertices = false;
    let mut subchunks_checked = 0;

    // Look for vertex chunks inside this chunk
    while offset + 8 <= chunk_data.len() && subchunks_checked < 10 {
        let subchunk_type = u32::from_le_bytes([
            chunk_data[offset],
            chunk_data[offset + 1],
            chunk_data[offset + 2],
            chunk_data[offset + 3],
        ]);
        let subchunk_size = u32::from_le_bytes([
            chunk_data[offset + 4],
            chunk_data[offset + 5],
            chunk_data[offset + 6],
            chunk_data[offset + 7],
        ]) as usize;

        subchunks_checked += 1;

        if subchunk_size == 0
            || subchunk_size > chunk_data.len()
            || offset + 8 + subchunk_size > chunk_data.len()
        {
            break;
        }

        match subchunk_type {
            0x00000002 => {
                // W3D_CHUNK_VERTICES
                if subchunk_size >= 36 {
                    // At least 3 vertices (12 bytes each)
                    has_vertices = true;
                    break;
                }
            }
            0x00000020 => {
                // W3D_CHUNK_TRIANGLES
                if subchunk_size >= 12 {
                    // At least 1 triangle (12 bytes)
                    has_vertices = true;
                    break;
                }
            }
            _ => {}
        }

        offset += 8 + subchunk_size;
    }

    has_vertices
}

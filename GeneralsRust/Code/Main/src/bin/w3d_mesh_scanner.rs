use anyhow::Result;
use generals_main::assets::archive::ArchiveFileSystem;
use log::{error, info, warn};

const W3D_CHUNK_MESH: u32 = 0x00000000; // Mesh definition (container)
const W3D_CHUNK_VERTICES: u32 = 0x00000002; // array of vertices
const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x00000003; // array of normals
const W3D_CHUNK_TEXCOORDS: u32 = 0x00000005; // texture coordinates
const W3D_CHUNK_TRIANGLES: u32 = 0x00000020; // triangles chunk
const W3D_CHUNK_HIERARCHY: u32 = 0x00000100; // hierarchy tree definition
const W3D_CHUNK_HMODEL: u32 = 0x00000300; // blueprint for hierarchy model
const W3D_CHUNK_LODMODEL: u32 = 0x00000400; // blueprint for LOD model
const W3D_CHUNK_SHDMESH: u32 = 0x00000B00; // Shader mesh

#[derive(Debug, Default)]
struct W3DFileAnalysis {
    filename: String,
    file_size: usize,
    has_mesh_chunks: bool,
    has_vertex_data: bool,
    has_triangle_data: bool,
    has_hierarchy: bool,
    has_hmodel: bool,
    has_lodmodel: bool,
    has_shader_mesh: bool,
    vertex_count_estimate: usize,
    triangle_count_estimate: usize,
    chunk_summary: Vec<String>,
}

fn analyze_w3d_file(filename: &str, data: &[u8]) -> Result<W3DFileAnalysis> {
    let mut analysis = W3DFileAnalysis {
        filename: filename.to_string(),
        file_size: data.len(),
        ..Default::default()
    };

    if data.len() < 8 {
        return Ok(analysis);
    }

    // Try standard chunk parsing first
    let mut valid_chunks_found = false;

    // Check if first chunk looks valid
    let _first_chunk_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let first_chunk_size =
        (u32::from_le_bytes([data[4], data[5], data[6], data[7]]) & 0x7FFFFFFF) as usize;

    if first_chunk_size > data.len() || first_chunk_size == 0 {
        // Try with header skip (common in C&C W3D files)
        return analyze_w3d_with_header_skip(filename, data);
    }

    // Parse chunks (including nested container chunks)
    valid_chunks_found = analyze_chunks(&mut analysis, data, 0);

    if !valid_chunks_found {
        return analyze_w3d_with_header_skip(filename, data);
    }

    Ok(analysis)
}

fn analyze_chunks(analysis: &mut W3DFileAnalysis, data: &[u8], depth: usize) -> bool {
    if depth > 64 {
        return false;
    }

    let mut offset = 0usize;
    let mut valid_chunks_found = false;

    while offset + 8 <= data.len() {
        let chunk_type = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        let raw_chunk_size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);

        let is_container = (raw_chunk_size & 0x80000000) != 0;
        let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

        if chunk_size == 0 || offset + 8 + chunk_size > data.len() {
            break;
        }

        valid_chunks_found = true;
        let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

        match chunk_type {
            W3D_CHUNK_MESH => {
                analysis.has_mesh_chunks = true;
                if depth == 0 {
                    analysis
                        .chunk_summary
                        .push(format!("MESH({} bytes)", chunk_size));
                }
            }
            W3D_CHUNK_VERTICES => {
                analysis.has_vertex_data = true;
                analysis.vertex_count_estimate =
                    analysis.vertex_count_estimate.max(chunk_size / 12);
                if depth == 0 {
                    analysis.chunk_summary.push(format!(
                        "VERTICES({} verts)",
                        analysis.vertex_count_estimate
                    ));
                }
            }
            W3D_CHUNK_TRIANGLES => {
                analysis.has_triangle_data = true;
                analysis.triangle_count_estimate =
                    analysis.triangle_count_estimate.max(chunk_size / 12);
                if depth == 0 {
                    analysis.chunk_summary.push(format!(
                        "TRIANGLES({} tris)",
                        analysis.triangle_count_estimate
                    ));
                }
            }
            W3D_CHUNK_HIERARCHY => {
                analysis.has_hierarchy = true;
                if depth == 0 {
                    analysis
                        .chunk_summary
                        .push(format!("HIERARCHY({} bytes)", chunk_size));
                }
            }
            W3D_CHUNK_HMODEL => {
                analysis.has_hmodel = true;
                if depth == 0 {
                    analysis
                        .chunk_summary
                        .push(format!("HMODEL({} bytes)", chunk_size));
                }
            }
            W3D_CHUNK_LODMODEL => {
                analysis.has_lodmodel = true;
                if depth == 0 {
                    analysis
                        .chunk_summary
                        .push(format!("LODMODEL({} bytes)", chunk_size));
                }
            }
            W3D_CHUNK_SHDMESH => {
                analysis.has_shader_mesh = true;
                if depth == 0 {
                    analysis
                        .chunk_summary
                        .push(format!("SHDMESH({} bytes)", chunk_size));
                }
            }
            _ => {
                if depth == 0 && chunk_size < 1_000_000 {
                    analysis.chunk_summary.push(format!(
                        "UNKNOWN_0x{:08X}({} bytes)",
                        chunk_type, chunk_size
                    ));
                }
            }
        }

        if is_container {
            let _ = analyze_chunks(analysis, chunk_data, depth + 1);
        }

        offset += 8 + chunk_size;
    }

    valid_chunks_found
}

fn analyze_w3d_with_header_skip(filename: &str, data: &[u8]) -> Result<W3DFileAnalysis> {
    let mut analysis = W3DFileAnalysis {
        filename: filename.to_string(),
        file_size: data.len(),
        ..Default::default()
    };

    if data.len() < 52 {
        return Ok(analysis);
    }

    // Try parsing with W3D header format
    let name_section_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
    let chunks_start = 16 + name_section_size;

    if chunks_start >= data.len() {
        return Ok(analysis);
    }

    analysis
        .chunk_summary
        .push(format!("W3D_HEADER(name_size={})", name_section_size));

    let chunk_region = &data[chunks_start..];
    let _ = analyze_chunks(&mut analysis, chunk_region, 0);

    Ok(analysis)
}

fn analyze_mesh_subchunks(analysis: &mut W3DFileAnalysis, data: &[u8]) {
    let mut offset = 0;
    while offset + 8 <= data.len() {
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

        if offset + 8 + chunk_size > data.len() {
            break;
        }

        match chunk_type {
            W3D_CHUNK_VERTICES => {
                analysis.has_vertex_data = true;
                analysis.vertex_count_estimate = chunk_size / 12;
            }
            W3D_CHUNK_TRIANGLES => {
                analysis.has_triangle_data = true;
                analysis.triangle_count_estimate = chunk_size / 12;
            }
            _ => {}
        }

        offset += 8 + chunk_size;
    }
}

fn analyze_hierarchy_for_meshes(analysis: &mut W3DFileAnalysis, data: &[u8]) {
    let mut offset = 0;
    while offset + 8 <= data.len() {
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

        if offset + 8 + chunk_size > data.len() {
            break;
        }

        if chunk_type == 0x00000000 {
            // Mesh inside hierarchy
            analysis.has_mesh_chunks = true;
            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            analyze_mesh_subchunks(analysis, chunk_data);
        }

        offset += 8 + chunk_size;
    }
}

fn analyze_hmodel_for_meshes(analysis: &mut W3DFileAnalysis, data: &[u8]) {
    let mut offset = 0;
    while offset + 8 <= data.len() {
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

        if offset + 8 + chunk_size > data.len() {
            break;
        }

        if chunk_type == 0x00000000 {
            // Mesh inside hmodel
            analysis.has_mesh_chunks = true;
            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            analyze_mesh_subchunks(analysis, chunk_data);
        }

        offset += 8 + chunk_size;
    }
}

fn analyze_lodmodel_for_meshes(analysis: &mut W3DFileAnalysis, data: &[u8]) {
    let mut offset = 0;
    while offset + 8 <= data.len() {
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

        if offset + 8 + chunk_size > data.len() {
            break;
        }

        if chunk_type == 0x00000000 {
            // Mesh inside lodmodel
            analysis.has_mesh_chunks = true;
            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            analyze_mesh_subchunks(analysis, chunk_data);
        }

        offset += 8 + chunk_size;
    }
}

fn analyze_shader_mesh_for_geometry(analysis: &mut W3DFileAnalysis, data: &[u8]) {
    let mut offset = 0;
    while offset + 8 <= data.len() {
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

        if offset + 8 + chunk_size > data.len() {
            break;
        }

        // Shader submesh chunks often contain geometry
        if chunk_type == 0x00000B20 {
            // W3D_CHUNK_SHDSUBMESH
            analysis.has_mesh_chunks = true;
            // Could contain vertices/triangles - mark as potential geometry
            if chunk_size > 1000 {
                // Reasonable size for geometry
                analysis.has_vertex_data = true; // Assume it has geometry
            }
        }

        offset += 8 + chunk_size;
    }
}

fn has_geometry_data(analysis: &W3DFileAnalysis) -> bool {
    analysis.has_vertex_data && analysis.has_triangle_data
}

fn has_potential_geometry(analysis: &W3DFileAnalysis) -> bool {
    analysis.has_mesh_chunks
        || analysis.has_shader_mesh
        || (analysis.has_hmodel && analysis.file_size > 10000)
        || (analysis.has_lodmodel && analysis.file_size > 10000)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🔍 W3D Mesh Scanner - Finding files with actual geometry data");

    let mut archive_system = ArchiveFileSystem::new();
    archive_system.init().await?;
    let loaded_archives = archive_system.get_loaded_archives();
    if loaded_archives.is_empty() {
        warn!(
            "⚠️ No BIG archives loaded (set GENERALS_ASSETS_DIR or provide ./assets or ./windows_game assets)"
        );
        return Ok(());
    }
    info!("✅ Loaded {} BIG archives", loaded_archives.len());

    // Get all W3D files
    let all_files = archive_system.list_all_files();
    let w3d_files: Vec<_> = all_files
        .iter()
        .filter(|f| f.to_lowercase().ends_with(".w3d"))
        .collect();

    info!("📊 Found {} W3D files total", w3d_files.len());

    // Analyze files by size categories
    let mut _analyses: Vec<W3DFileAnalysis> = Vec::new();
    let mut large_files = Vec::new(); // > 50KB
    let mut medium_files = Vec::new(); // 10-50KB
    let mut small_files = Vec::new(); // < 10KB

    for (i, filename) in w3d_files.iter().enumerate() {
        if i % 20 == 0 {
            info!("🔍 Analyzing files... {}/{}", i, w3d_files.len());
        }

        match archive_system.open_file(filename).await {
            Ok(data) => {
                match analyze_w3d_file(filename, &data) {
                    Ok(analysis) => {
                        if analysis.file_size > 51200 {
                            // > 50KB
                            large_files.push(analysis);
                        } else if analysis.file_size > 10240 {
                            // 10-50KB
                            medium_files.push(analysis);
                        } else {
                            // < 10KB
                            small_files.push(analysis);
                        }
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to analyze {}: {}", filename, e);
                    }
                }
            }
            Err(e) => {
                warn!("⚠️ Failed to load {}: {}", filename, e);
            }
        }
    }

    // Sort by file size (largest first)
    large_files.sort_by(|a, b| b.file_size.cmp(&a.file_size));
    medium_files.sort_by(|a, b| b.file_size.cmp(&a.file_size));

    // Report results
    info!("\n🎯 LARGE FILES (>50KB) - Most likely to contain mesh geometry:");
    info!("================================================================");

    let mut geometry_candidates = Vec::new();
    let mut potential_candidates = Vec::new();

    for analysis in &large_files {
        let status = if has_geometry_data(analysis) {
            geometry_candidates.push(analysis);
            "✅ HAS_GEOMETRY"
        } else if has_potential_geometry(analysis) {
            potential_candidates.push(analysis);
            "🔄 POTENTIAL_GEOMETRY"
        } else {
            "❌ SKELETON_ONLY"
        };

        info!(
            "📁 {} ({:6} KB) - {} - Chunks: [{}]",
            analysis.filename,
            analysis.file_size / 1024,
            status,
            analysis.chunk_summary.join(", ")
        );

        if has_geometry_data(analysis) {
            info!(
                "   └─ 🔺 Vertices: {}, Triangles: {}",
                analysis.vertex_count_estimate, analysis.triangle_count_estimate
            );
        }
    }

    info!("\n🎯 MEDIUM FILES (10-50KB) - Potential geometry files:");
    info!("=====================================================");
    for analysis in medium_files.iter().take(10) {
        // Show top 10
        let status = if has_geometry_data(analysis) {
            geometry_candidates.push(analysis);
            "✅ HAS_GEOMETRY"
        } else if has_potential_geometry(analysis) {
            potential_candidates.push(analysis);
            "🔄 POTENTIAL_GEOMETRY"
        } else {
            "❌ SKELETON_ONLY"
        };

        info!(
            "📁 {} ({:6} KB) - {} - Chunks: [{}]",
            analysis.filename,
            analysis.file_size / 1024,
            status,
            analysis.chunk_summary.join(", ")
        );
    }

    // Summary report
    info!("\n📊 SUMMARY REPORT:");
    info!("==================");
    info!("🔍 Total W3D files analyzed: {}", w3d_files.len());
    info!("📁 Large files (>50KB): {}", large_files.len());
    info!("📁 Medium files (10-50KB): {}", medium_files.len());
    info!("📁 Small files (<10KB): {}", small_files.len());
    info!(
        "✅ Files with confirmed geometry: {}",
        geometry_candidates.len()
    );
    info!(
        "🔄 Files with potential geometry: {}",
        potential_candidates.len()
    );

    if !geometry_candidates.is_empty() {
        info!("\n🎯 TOP MESH GEOMETRY CANDIDATES:");
        info!("=================================");
        for analysis in geometry_candidates.iter().take(5) {
            info!(
                "🏆 {} ({} KB) - {} vertices, {} triangles",
                analysis.filename,
                analysis.file_size / 1024,
                analysis.vertex_count_estimate,
                analysis.triangle_count_estimate
            );
        }
    }

    if !potential_candidates.is_empty() && geometry_candidates.is_empty() {
        info!("\n🔄 TOP POTENTIAL GEOMETRY CANDIDATES:");
        info!("=====================================");
        for analysis in potential_candidates.iter().take(5) {
            info!(
                "🔄 {} ({} KB) - {}",
                analysis.filename,
                analysis.file_size / 1024,
                analysis.chunk_summary.join(", ")
            );
        }
    }

    if geometry_candidates.is_empty() && potential_candidates.is_empty() {
        error!("\n💥 CRITICAL: No W3D files found with mesh geometry data!");
        error!("💥 All files appear to contain only skeleton/hierarchy data.");
        error!("💥 This suggests either:");
        error!("   1. Missing BIG archives that contain the actual mesh data");
        error!("   2. Different W3D format/version than expected");
        error!("   3. Meshes stored in different chunk types not yet handled");
    }

    Ok(())
}

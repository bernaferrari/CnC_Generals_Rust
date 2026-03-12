/*
** Command & Conquer Generals Zero Hour(tm)
**
** W3D Texture Audit - Load W3D models from BIG archives, resolve referenced textures,
** and validate that referenced textures can be located and decoded.
**
** Usage:
**   cargo run --features dev-tools --bin w3d_texture_audit -- <model1> [model2...]
**
** Notes:
** - Uses `ArchiveFileSystem` search paths. Override with `GENERALS_ASSETS_DIR` if needed.
** - Model arguments can be `avhummer` or `avhummer.w3d` or full virtual paths.
*/

use anyhow::{anyhow, Result};
use generals_main::assets::{archive::ArchiveFileSystem, models::W3DLoader};
use std::collections::{BTreeMap, HashMap};

fn build_virtual_path_index(archive: &ArchiveFileSystem) -> HashMap<String, String> {
    archive
        .list_all_files()
        .into_iter()
        .map(|path| (path.to_lowercase(), path))
        .collect()
}

fn texture_candidate_paths(name: &str) -> Vec<String> {
    let name = name.trim().trim_matches('\\').trim_matches('/');
    if name.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    candidates.push(name.to_string());

    // Extension fallbacks (many assets ship both `.tga` and `.dds` variants).
    if let Some((base, ext)) = name.rsplit_once('.') {
        let ext_lower = ext.to_lowercase();
        if ext_lower == "tga" {
            candidates.push(format!("{}.dds", base));
        } else if ext_lower == "dds" {
            candidates.push(format!("{}.tga", base));
        }
    }

    if !name.contains('/') && !name.contains('\\') {
        candidates.push(format!("art/textures/{}", name));
        candidates.push(format!("Art/Textures/{}", name));
        candidates.push(format!("art/w3d/{}", name));
        candidates.push(format!("Art/W3D/{}", name));
        candidates.push(format!("data/{}", name));

        if let Some((base, ext)) = name.rsplit_once('.') {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "tga" {
                candidates.push(format!("art/textures/{}.dds", base));
                candidates.push(format!("Art/Textures/{}.dds", base));
            } else if ext_lower == "dds" {
                candidates.push(format!("art/textures/{}.tga", base));
                candidates.push(format!("Art/Textures/{}.tga", base));
            }
        }
    }

    candidates
}

fn suggest_virtual_paths(index: &HashMap<String, String>, name: &str) -> Vec<String> {
    let name_lower = name.to_lowercase();
    let basename_lower = name_lower
        .rsplit_once('/')
        .map(|(_, base)| base)
        .unwrap_or(&name_lower);

    let base_without_ext = basename_lower
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(basename_lower);

    let mut suggestions: Vec<String> = index
        .keys()
        .filter(|path| {
            path.ends_with(basename_lower)
                || path.contains(&format!("/{}", basename_lower))
                || path.contains(base_without_ext)
        })
        .take(8)
        .cloned()
        .collect();

    suggestions.sort();
    suggestions
}

async fn try_open_virtual_file(
    archive: &mut ArchiveFileSystem,
    index: &HashMap<String, String>,
    virtual_path: &str,
) -> Result<(String, Vec<u8>)> {
    let key = virtual_path.replace('\\', "/").to_lowercase();
    let Some(actual_path) = index.get(&key) else {
        return Err(anyhow!("not found: {}", virtual_path));
    };
    let bytes = archive.open_file(actual_path).await?;
    Ok((actual_path.clone(), bytes))
}

fn decode_texture(path: &str, bytes: &[u8]) -> Result<(u32, u32)> {
    fn hexdump_prefix(bytes: &[u8], len: usize) -> String {
        bytes
            .iter()
            .take(len)
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }

    let by_guess = image::load_from_memory(bytes);
    if let Ok(image) = by_guess {
        return Ok((image.width(), image.height()));
    }

    let lower = path.to_lowercase();
    let mut attempts = Vec::new();

    if lower.ends_with(".tga") {
        attempts.push(image::ImageFormat::Tga);
    } else if lower.ends_with(".dds") {
        attempts.push(image::ImageFormat::Dds);
    } else {
        attempts.push(image::ImageFormat::Tga);
        attempts.push(image::ImageFormat::Dds);
    }

    for fmt in attempts {
        if let Ok(image) = image::load_from_memory_with_format(bytes, fmt) {
            return Ok((image.width(), image.height()));
        }
    }

    Err(anyhow!(
        "decode failed for {}: {} (magic: {}, len={})",
        path,
        by_guess
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        hexdump_prefix(bytes, 16),
        bytes.len()
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let models = if args.is_empty() {
        vec!["avhummer".to_string()]
    } else {
        args
    };

    let mut archive = ArchiveFileSystem::new();
    archive.init().await?;
    let index = build_virtual_path_index(&archive);

    let loader = W3DLoader::new();

    for model in &models {
        println!("\n=== MODEL: {} ===", model);
        let model_data = loader.load_model(&mut archive, model).await?;

        let mut texture_counts: BTreeMap<String, usize> = BTreeMap::new();
        for name in &model_data.texture_names {
            *texture_counts.entry(name.clone()).or_default() += 1;
        }
        for mesh in &model_data.meshes {
            if let Some(name) = &mesh.material.texture_name {
                *texture_counts.entry(name.clone()).or_default() += 1;
            }
            for name in &mesh.texture_library {
                *texture_counts.entry(name.clone()).or_default() += 1;
            }
            for pass in &mesh.per_pass_stage_texture_names {
                for stage in pass {
                    for name in stage {
                        *texture_counts.entry(name.clone()).or_default() += 1;
                    }
                }
            }
        }

        println!(
            "Meshes: {} | Unique textures: {} | Total texture refs: {}",
            model_data.meshes.len(),
            texture_counts.len(),
            texture_counts.values().copied().sum::<usize>()
        );

        let total_refs = texture_counts.values().copied().sum::<usize>();
        let mut missing_unique = 0usize;
        let mut missing_refs = 0usize;
        let mut decode_failed_unique = 0usize;
        let mut decode_failed_refs = 0usize;

        for (texture, count) in &texture_counts {
            let mut opened: Option<(String, Vec<u8>)> = None;
            let mut open_error: Option<anyhow::Error> = None;

            for candidate in texture_candidate_paths(texture) {
                match try_open_virtual_file(&mut archive, &index, &candidate).await {
                    Ok(found) => {
                        opened = Some(found);
                        break;
                    }
                    Err(err) => open_error = Some(err),
                }
            }

            let Some((resolved_path, bytes)) = opened else {
                missing_unique += 1;
                missing_refs += *count;
                let suggestions = suggest_virtual_paths(&index, texture);
                println!(
                    "MISSING ({:>3}x): {} ({})",
                    count,
                    texture,
                    open_error
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "not found".to_string())
                );
                if !suggestions.is_empty() {
                    println!("         suggestions:");
                    for suggestion in suggestions {
                        println!("           - {}", suggestion);
                    }
                }
                continue;
            };

            match decode_texture(&resolved_path, &bytes) {
                Ok((w, h)) => {
                    let archive_name = archive
                        .get_archive_filename_for_file(&resolved_path)
                        .unwrap_or_else(|| "?".to_string());
                    println!(
                        "OK      ({:>3}x): {} -> {} ({}x{}, {})",
                        count, texture, resolved_path, w, h, archive_name
                    );
                }
                Err(err) => {
                    decode_failed_unique += 1;
                    decode_failed_refs += *count;
                    println!(
                        "BADDEC  ({:>3}x): {} -> {} ({})",
                        count, texture, resolved_path, err
                    );
                }
            }
        }

        let ok_refs = total_refs.saturating_sub(missing_refs + decode_failed_refs);
        println!(
            "Summary: missing={}/{} decode_failed={}/{} ok_refs={}",
            missing_unique, missing_refs, decode_failed_unique, decode_failed_refs, ok_refs
        );
    }

    Ok(())
}

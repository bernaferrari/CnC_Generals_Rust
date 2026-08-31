#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::cargo)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(deprecated)]
#![allow(nonstandard_style)]
#![allow(unconditional_recursion)]
#![allow(mismatched_lifetime_syntaxes)]
#![allow(unexpected_cfgs)]
#![allow(private_interfaces)]

use anyhow::{Context, Result};
use base64::Engine as _;
use clap::Parser;
use env_logger::Env;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

mod w3d;
mod writer;
mod writer_buffers;
mod writer_materials;
// Note: BIG archive handling is intentionally out of scope here.

#[derive(Parser, Debug)]
#[command(author, version, about = "Convert Westwood W3D to glTF 2.0", long_about = None)]
struct Args {
    /// Input W3D file path (omit when using --big + --entry)
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Output file base path (without extension). If omitted, we derive a name and write to --out-dir (default: ./out)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Optional directory where outputs are written (defaults to ./out when --output is omitted)
    #[arg(long)]
    out_dir: Option<PathBuf>,

    /// Optional: path to a BIG archive to extract from instead of raw file
    // Removed: BIG handling is external to this tool
    // #[arg(long)]
    // big: Option<PathBuf>,

    /// Optional: internal path inside BIG (required when using --big)
    // #[arg(long)]
    // entry: Option<String>,

    // #[arg(long, default_value_t = false)]
    // extract_textures: bool,

    // #[arg(long, default_value_t = false)]
    // list_entries: bool,
    // #[arg(long)]
    // limit: Option<usize>,

    /// Batch convert: input directory containing .w3d files (non-recursive)
    #[arg(long)]
    batch_in: Option<PathBuf>,
    /// Batch convert: output root directory (defaults to ./out)
    #[arg(long)]
    batch_out: Option<PathBuf>,
    /// Batch: limit number of files to process
    #[arg(long)]
    batch_limit: Option<usize>,
    /// Batch: sample randomly from available files
    #[arg(long, default_value_t = false)]
    batch_random: bool,
    /// Batch: write a JSON report to this path
    #[arg(long)]
    batch_report: Option<PathBuf>,

    /// Scan a directory for W3D files using AdaptiveDelta compression (non-recursive)
    #[arg(long)]
    scan_ad: Option<PathBuf>,
    /// Scan a directory for animation usage (uncompressed, compressed timecoded, compressed AdaptiveDelta, morph)
    #[arg(long)]
    scan_anims: Option<PathBuf>,
    /// Scan a directory for broader W3D feature usage (AABTree, lights, per-tri materials, vertex colors, multi-UV, maps, bump, LOD/Collision/Shadow)
    #[arg(long)]
    scan_features: Option<PathBuf>,
    /// Limit number of files scanned
    #[arg(long)]
    scan_limit: Option<usize>,
    /// Sample randomly when scanning
    #[arg(long, default_value_t = false)]
    scan_random: bool,
    /// Scan recursively through subdirectories
    #[arg(long, default_value_t = false)]
    scan_recursive: bool,
    /// Scan report path (JSON) for --scan-anims or --scan-features
    #[arg(long)]
    scan_report: Option<PathBuf>,
    /// Scan: fail (non-zero) when no AdaptiveDelta files found
    #[arg(long, default_value_t = false)]
    scan_fail_on_empty: bool,

    /// Validate generated glTF structure (bounds, counts, indices)
    #[arg(long, default_value_t = false)]
    strict: bool,

    /// Validate a glTF file on disk (reads .gltf + .bin)
    #[arg(long)]
    validate_file: Option<PathBuf>,
    /// Validate all .gltf files in a directory (non-recursive unless --validate-recursive)
    #[arg(long)]
    validate_dir: Option<PathBuf>,
    /// Validate recursively under --validate-dir
    #[arg(long, default_value_t = false)]
    validate_recursive: bool,
    /// Write JSON report for validation results
    #[arg(long)]
    validate_report: Option<PathBuf>,

    /// When set, attempt to attach images: copy or embed from a source directory
    #[arg(long, default_value_t = false)]
    with_images: bool,
    /// Root directory where extracted images (tga/png/jpg/dds/…) can be found. Defaults to ./extracted_big_files_v2
    #[arg(long)]
    images_root: Option<PathBuf>,
    /// Embed found images as data URIs into the glTF JSON instead of copying alongside
    #[arg(long, default_value_t = false)]
    embed_images: bool,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    // Scan mode: find W3D files that use AdaptiveDelta compressed animations
    if let Some(scan_dir) = &args.scan_ad {
        return scan_adaptive_delta(
            scan_dir,
            args.scan_limit,
            args.scan_random,
            args.scan_recursive,
            args.scan_fail_on_empty,
        );
    }

    // Scan animations composition
    if let Some(scan_dir) = &args.scan_anims {
        return scan_animations(
            scan_dir,
            args.scan_limit,
            args.scan_random,
            args.scan_recursive,
            args.scan_report.as_deref(),
        );
    }

    // Scan feature usage
    if let Some(scan_dir) = &args.scan_features {
        return scan_features(
            scan_dir,
            args.scan_limit,
            args.scan_random,
            args.scan_recursive,
            args.scan_report.as_deref(),
        );
    }

    // Standalone validator modes
    if let Some(path) = &args.validate_file {
        return validate_gltf_on_disk(path, args.strict);
    }
    if let Some(dir) = &args.validate_dir {
        return validate_gltf_in_dir(
            dir,
            args.validate_recursive,
            args.validate_report.as_deref(),
            args.strict,
        );
    }

    // Batch mode
    if let Some(batch_dir) = &args.batch_in {
        return batch_convert(
            batch_dir,
            args.batch_out
                .as_deref()
                .unwrap_or(std::path::Path::new("out")),
            args.batch_limit,
            args.batch_random,
            args.batch_report.as_deref(),
            args.strict,
            args.with_images,
            args.embed_images,
            args.images_root.as_deref(),
        );
    }

    // Single-file mode: read from filesystem only
    let input_path = args.input.clone().context("--input is required")?;
    let data = fs::read(&input_path).with_context(|| format!("read {}", input_path.display()))?;
    log::info!(
        "Parsing W3D: {} ({} bytes)",
        input_path.display(),
        data.len()
    );

    let mut file = w3d::parser::parse_w3d_file(&data).context("parse W3D")?;
    // If this is an animation-only file, try to auto-load its referenced hierarchy from the same folder
    if file.hierarchies.is_empty() && !file.animations.is_empty() {
        if let Some(inp_path) = &args.input {
            try_load_local_hierarchy(
                &mut file,
                inp_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
            )?;
        }
    }
    // If this is an animation-only file, we do not attempt to auto-load hierarchies here.
    log::info!(
        "Parsed: {} meshes, {} hierarchies",
        file.meshes.len(),
        file.hierarchies.len()
    );

    let (mut gltf_root, bin) = writer::convert_to_gltf(&file).context("convert to glTF")?;

    // Derive output base (dir + stem)
    let (out_dir, out_stem) = {
        // Prefer explicit output stem if given
        let stem = args
            .output
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .or_else(|| {
                args.input
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "output".to_string());

        // Determine out dir
        let out_dir = if let Some(dir) = &args.out_dir {
            dir.clone()
        } else if let Some(out) = &args.output {
            // If output includes a parent, use it; else current dir
            out.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            PathBuf::from("out")
        };
        (out_dir, stem)
    };

    // Ensure out dir exists
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        log::warn!(
            "Could not create output directory {}: {}",
            out_dir.display(),
            e
        );
    }

    // Set buffer URI based on output filename
    {
        let uri = format!("{}.bin", out_stem);
        if let Some(buffers) = gltf_root.get_mut("buffers").and_then(Value::as_array_mut) {
            if let Some(buf0) = buffers.get_mut(0) {
                if let Some(obj) = buf0.as_object_mut() {
                    obj.insert("uri".to_string(), Value::String(uri));
                }
            }
        }
    }

    // Optionally resolve and attach images (copy or embed) before writing
    if args.with_images {
        let images_root = args
            .images_root
            .as_deref()
            .unwrap_or(std::path::Path::new("extracted_big_files_v2"));
        if args.embed_images {
            resolve_and_embed_images(&mut gltf_root, images_root)?;
        } else {
            resolve_and_copy_images(&gltf_root, &out_dir, images_root)?;
        }
    }

    let gltf_path = out_dir.join(format!("{}.gltf", out_stem));
    let bin_path = out_dir.join(format!("{}.bin", out_stem));

    // Write JSON and BIN after any adjustments
    let mut f =
        fs::File::create(&gltf_path).with_context(|| format!("create {}", gltf_path.display()))?;
    serde_json::to_writer_pretty(&mut f, &gltf_root).context("write glTF json")?;
    f.flush()?;
    fs::write(&bin_path, &bin).with_context(|| format!("write {}", bin_path.display()))?;

    log::info!("Wrote {} and {}", gltf_path.display(), bin_path.display());
    Ok(())
}

fn try_load_local_hierarchy(file: &mut w3d::structs::W3dFile, dir: &std::path::Path) -> Result<()> {
    // Use the first animation's declared hierarchy name to locate a .W3D in the same directory
    let hname = file.animations[0].hierarchy_name();
    if hname.is_empty() {
        return Ok(());
    }
    let hname_lc = hname.to_lowercase();

    // Build candidate list of .w3d files in dir
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("w3d"))
            .unwrap_or(false)
        {
            candidates.push(p);
        }
    }
    // Sort by best stem match (exact, starts_with/contains)
    candidates.sort_by_key(|p| {
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if stem == hname_lc {
            0
        } else if stem.starts_with(&hname_lc) || hname_lc.starts_with(&stem) {
            1
        } else if stem.contains(&hname_lc) || hname_lc.contains(&stem) {
            2
        } else {
            3
        }
    });

    // Try candidates in order until we find a file that actually contains a hierarchy
    for p in candidates {
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        // Quick filter: require some similarity
        let is_reasonable = stem == hname_lc
            || stem.starts_with(&hname_lc)
            || hname_lc.starts_with(&stem)
            || stem.contains(&hname_lc)
            || hname_lc.contains(&stem);
        if !is_reasonable {
            continue;
        }

        let bytes = match std::fs::read(&p) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(hfile) = w3d::parser::parse_w3d_file(&bytes) {
            if !hfile.hierarchies.is_empty() {
                file.hierarchies = hfile.hierarchies;
                log::info!("Loaded hierarchy '{}' from {}", hname, p.display());
                break;
            }
        }
    }
    Ok(())
}

// Note: BIG extraction/baking helpers removed as BIG handling is external.

fn scan_adaptive_delta(
    dir: &std::path::Path,
    limit: Option<usize>,
    random: bool,
    recursive: bool,
    fail_on_empty: bool,
) -> Result<()> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;
    use walkdir::WalkDir;
    // Collect .w3d files
    let mut files = Vec::new();
    if recursive {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("w3d"))
                .unwrap_or(false)
            {
                files.push(p);
            }
        }
    } else {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("w3d"))
                .unwrap_or(false)
            {
                files.push(p);
            }
        }
    }
    if files.is_empty() {
        println!("No .w3d files in {}", dir.display());
        return Ok(());
    }
    if random {
        files.shuffle(&mut thread_rng());
    }
    if let Some(n) = limit {
        if files.len() > n {
            files.truncate(n);
        }
    }

    let mut ad_files = Vec::new();
    let mut tc_files = Vec::new();
    let mut total = 0usize;
    for path in files {
        total += 1;
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if w3d_contains_adaptive_delta(&bytes) {
            ad_files.push(
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string(),
            );
        } else if w3d_contains_compressed_timecoded(&bytes) {
            tc_files.push(
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    println!("Scanned {} files in {}", total, dir.display());
    println!("AdaptiveDelta files: {}", ad_files.len());
    for f in &ad_files {
        println!("  {}", f);
    }
    println!("Timecoded compressed files: {}", tc_files.len());
    for f in &tc_files {
        println!("  {}", f);
    }
    if fail_on_empty && ad_files.is_empty() {
        anyhow::bail!("No AdaptiveDelta files found in {}", dir.display());
    }
    Ok(())
}

fn w3d_contains_adaptive_delta(data: &[u8]) -> bool {
    // Scan for CompressedAnimation chunks (0x0280) and read their Header (0x0281)
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::{Cursor, Read, Seek, SeekFrom};
    let mut r = Cursor::new(data);
    while (r.position() as usize) + 8 <= data.len() {
        let chunk_id = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        };
        let size = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        } & 0x7FFF_FFFF;
        let start = r.position();
        let end = start + size as u64;
        // Some files might place the header at top-level (rare)
        if chunk_id == 0x0000_0281 {
            if (r.position() as usize) + 4 + 16 + 16 + 4 + 2 + 2 <= data.len() {
                let _version = r.read_u32::<LittleEndian>().ok();
                let mut name = [0u8; 16];
                let _ = r.read_exact(&mut name);
                let mut hname = [0u8; 16];
                let _ = r.read_exact(&mut hname);
                let _frames = r.read_u32::<LittleEndian>().ok();
                let _fr = r.read_u16::<LittleEndian>().ok();
                let flavor = r.read_u16::<LittleEndian>().unwrap_or(0);
                if flavor != 0 {
                    return true;
                }
            }
        }
        if chunk_id == 0x0000_0280 {
            // CompressedAnimation container
            let mut sub_pos = start;
            while (sub_pos as usize) + 8 <= data.len() && sub_pos < end {
                r.seek(SeekFrom::Start(sub_pos)).ok();
                let sub_id = match r.read_u32::<LittleEndian>() {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let sub_size = match r.read_u32::<LittleEndian>() {
                    Ok(v) => v,
                    Err(_) => break,
                } & 0x7FFF_FFFF;
                let sub_data_start = r.position();
                let sub_end = sub_data_start + sub_size as u64;
                if sub_id == 0x0000_0281 {
                    // CompressedAnimationHeader
                    if (r.position() as usize) + 4 + 16 + 16 + 4 + 2 + 2 <= data.len() {
                        let _version = r.read_u32::<LittleEndian>().ok();
                        let mut name = [0u8; 16];
                        let _ = r.read_exact(&mut name);
                        let mut hname = [0u8; 16];
                        let _ = r.read_exact(&mut hname);
                        let _frames = r.read_u32::<LittleEndian>().ok();
                        let _fr = r.read_u16::<LittleEndian>().ok();
                        let flavor = r.read_u16::<LittleEndian>().unwrap_or(0);
                        if flavor != 0 {
                            return true;
                        }
                    }
                }
                sub_pos = sub_end;
            }
        }
        r.seek(SeekFrom::Start(end)).ok();
    }
    false
}

fn w3d_contains_compressed_timecoded(data: &[u8]) -> bool {
    // Scan for CompressedAnimation chunks (0x0280) and read their Header (0x0281)
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::{Cursor, Read, Seek, SeekFrom};
    let mut r = Cursor::new(data);
    while (r.position() as usize) + 8 <= data.len() {
        let chunk_id = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        };
        let size = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        } & 0x7FFF_FFFF;
        let start = r.position();
        let end = start + size as u64;
        // Some files might place the header at top-level (rare)
        if chunk_id == 0x0000_0281 {
            if (r.position() as usize) + 4 + 16 + 16 + 4 + 2 + 2 <= data.len() {
                let _version = r.read_u32::<LittleEndian>().ok();
                let mut name = [0u8; 16];
                let _ = r.read_exact(&mut name);
                let mut hname = [0u8; 16];
                let _ = r.read_exact(&mut hname);
                let _frames = r.read_u32::<LittleEndian>().ok();
                let _fr = r.read_u16::<LittleEndian>().ok();
                let flavor = r.read_u16::<LittleEndian>().unwrap_or(0);
                if flavor == 0 {
                    return true;
                }
            }
        }
        if chunk_id == 0x0000_0280 {
            // CompressedAnimation container
            let mut sub_pos = start;
            while (sub_pos as usize) + 8 <= data.len() && sub_pos < end {
                r.seek(SeekFrom::Start(sub_pos)).ok();
                let sub_id = match r.read_u32::<LittleEndian>() {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let sub_size = match r.read_u32::<LittleEndian>() {
                    Ok(v) => v,
                    Err(_) => break,
                } & 0x7FFF_FFFF;
                let sub_data_start = r.position();
                let sub_end = sub_data_start + sub_size as u64;
                if sub_id == 0x0000_0281 {
                    // CompressedAnimationHeader
                    if (r.position() as usize) + 4 + 16 + 16 + 4 + 2 + 2 <= data.len() {
                        let _version = r.read_u32::<LittleEndian>().ok();
                        let mut name = [0u8; 16];
                        let _ = r.read_exact(&mut name);
                        let mut hname = [0u8; 16];
                        let _ = r.read_exact(&mut hname);
                        let _frames = r.read_u32::<LittleEndian>().ok();
                        let _fr = r.read_u16::<LittleEndian>().ok();
                        let flavor = r.read_u16::<LittleEndian>().unwrap_or(0);
                        if flavor == 0 {
                            return true;
                        }
                    }
                }
                sub_pos = sub_end;
            }
        }
        r.seek(SeekFrom::Start(end)).ok();
    }
    false
}

fn w3d_contains_uncompressed_anim(data: &[u8]) -> bool {
    // Look for AnimationHeader (0x00000201), either at top-level or inside Hierarchy (0x00000100)
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::{Cursor, Seek, SeekFrom};
    let mut r = Cursor::new(data);
    while (r.position() as usize) + 8 <= data.len() {
        let chunk_id = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        };
        let size = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        } & 0x7FFF_FFFF;
        let start = r.position();
        let end = start + size as u64;
        if chunk_id == 0x0000_0201 {
            return true;
        }
        if chunk_id == 0x0000_0100 {
            // Hierarchy container
            let mut sub_pos = start;
            while sub_pos + 8 <= end {
                r.seek(SeekFrom::Start(sub_pos)).ok();
                let sid = match r.read_u32::<LittleEndian>() {
                    Ok(v) => v,
                    Err(_) => break,
                };
                let ssize = match r.read_u32::<LittleEndian>() {
                    Ok(v) => v,
                    Err(_) => break,
                } & 0x7FFF_FFFF;
                let sdata_start = r.position();
                let send = sdata_start + ssize as u64;
                if sid == 0x0000_0201 {
                    return true;
                }
                sub_pos = send;
            }
        }
        r.seek(SeekFrom::Start(end)).ok();
    }
    false
}

fn w3d_contains_morph_anim(data: &[u8]) -> bool {
    // MorphAnimation container exists
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::{Cursor, Seek, SeekFrom};
    let mut r = Cursor::new(data);
    while (r.position() as usize) + 8 <= data.len() {
        let chunk_id = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        };
        let size = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        } & 0x7FFF_FFFF;
        let start = r.position();
        let end = start + size as u64;
        if chunk_id == 0x0000_02C0 {
            return true;
        }
        r.seek(SeekFrom::Start(end)).ok();
    }
    false
}

fn scan_animations(
    dir: &std::path::Path,
    limit: Option<usize>,
    random: bool,
    recursive: bool,
    report: Option<&std::path::Path>,
) -> Result<()> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;
    use walkdir::WalkDir;
    let mut files = Vec::new();
    if recursive {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("w3d"))
                .unwrap_or(false)
            {
                files.push(p);
            }
        }
    } else {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("w3d"))
                .unwrap_or(false)
            {
                files.push(p);
            }
        }
    }
    if files.is_empty() {
        println!("No .w3d files in {}", dir.display());
        return Ok(());
    }
    if random {
        files.shuffle(&mut thread_rng());
    }
    if let Some(n) = limit {
        if files.len() > n {
            files.truncate(n);
        }
    }

    let mut count_uncompressed = 0usize;
    let mut count_comp_tc = 0usize;
    let mut count_comp_ad = 0usize;
    let mut count_morph = 0usize;
    let mut results: Vec<serde_json::Value> = Vec::new();
    for path in files {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let u = w3d_contains_uncompressed_anim(&bytes);
        let tc = w3d_contains_compressed_timecoded(&bytes);
        let ad = w3d_contains_adaptive_delta(&bytes);
        let m = w3d_contains_morph_anim(&bytes);
        if u {
            count_uncompressed += 1;
        }
        if tc {
            count_comp_tc += 1;
        }
        if ad {
            count_comp_ad += 1;
        }
        if m {
            count_morph += 1;
        }
        if report.is_some() {
            results.push(serde_json::json!({
                "file": path.display().to_string(),
                "uncompressed": u,
                "compressed_timecoded": tc,
                "compressed_adaptive": ad,
                "morph": m,
            }));
        }
    }
    println!("Scanned animations in {}", dir.display());
    println!(
        "- Uncompressed (AnimationHeader 0x0201): {}",
        count_uncompressed
    );
    println!("- Compressed Timecoded (flavor 0): {}", count_comp_tc);
    println!(
        "- Compressed AdaptiveDelta (flavor != 0): {}",
        count_comp_ad
    );
    println!("- MorphAnimation present: {}", count_morph);
    if let Some(rp) = report {
        let obj = serde_json::json!({
            "root": dir.display().to_string(),
            "summary": {
                "uncompressed": count_uncompressed,
                "compressed_timecoded": count_comp_tc,
                "compressed_adaptive": count_comp_ad,
                "morph": count_morph,
            },
            "results": results,
        });
        let mut f = std::fs::File::create(rp)?;
        serde_json::to_writer_pretty(&mut f, &obj)?;
    }
    Ok(())
}

fn scan_features(
    dir: &std::path::Path,
    limit: Option<usize>,
    random: bool,
    recursive: bool,
    report: Option<&std::path::Path>,
) -> Result<()> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;
    use walkdir::WalkDir;
    let mut files = Vec::new();
    if recursive {
        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let p = entry.path().to_path_buf();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("w3d"))
                .unwrap_or(false)
            {
                files.push(p);
            }
        }
    } else {
        for entry in std::fs::read_dir(dir)? {
            let e = entry?;
            let p = e.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("w3d"))
                .unwrap_or(false)
            {
                files.push(p);
            }
        }
    }
    if files.is_empty() {
        println!("No .w3d files in {}", dir.display());
        return Ok(());
    }
    if random {
        files.shuffle(&mut thread_rng());
    }
    if let Some(n) = limit {
        if files.len() > n {
            files.truncate(n);
        }
    }

    // Counters
    let mut c_aabtree = 0usize;
    let mut c_lights = 0usize;
    let mut c_pertri = 0usize;
    let mut c_vcol = 0usize;
    let mut c_multiuv = 0usize;
    let mut c_detail = 0usize;
    let mut c_spec = 0usize;
    let mut c_emissive = 0usize;
    let mut c_bump = 0usize;
    let mut c_lod = 0usize;
    let mut c_collision = 0usize;
    let mut c_shadow = 0usize;
    let mut c_uv_anim = 0usize;
    let mut c_alpha_test = 0usize;
    let mut rows: Vec<serde_json::Value> = Vec::new();

    for path in files {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let mut has_aabtree = false;
        let mut has_lights = false;
        let mut has_pertri = false;
        let mut has_vcol = false;
        let mut has_multiuv = false;
        let mut has_detail = false;
        let mut has_spec = false;
        let mut has_emissive = false;
        let mut has_bump = false;
        let mut has_lod = false;
        let mut has_collision = false;
        let mut has_shadow = false;
        let mut has_uv_anim = false;
        let mut has_alpha_test = false;

        // Quick raw checks for certain chunk types
        if raw_contains_chunk(&bytes, 0x00000090) {
            has_aabtree = true;
        }
        if raw_contains_chunk(&bytes, 0x00000460) {
            has_lights = true;
        }
        if raw_contains_chunk(&bytes, 0x00000021) {
            has_pertri = true;
        }
        if raw_contains_chunk(&bytes, 0x0000000D) {
            has_vcol = true;
        }
        if raw_contains_chunk(&bytes, 0x0000001C) {
            has_detail = true;
        }
        if raw_contains_chunk(&bytes, 0x0000001D) {
            has_spec = true;
        }
        if raw_contains_chunk(&bytes, 0x0000001E) {
            has_emissive = true;
        }
        if raw_contains_chunk(&bytes, 0x00000400) {
            has_lod = true;
        }
        if raw_contains_chunk(&bytes, 0x00000303) {
            has_collision = true;
        }
        if raw_contains_chunk(&bytes, 0x00000306) {
            has_shadow = true;
        }

        // Deeper checks via parser: multi-UV and bump / uv anim via TextureInfo
        if let Ok(file) = crate::w3d::parser::parse_w3d_file(&bytes) {
            for m in &file.meshes {
                if m.material_passes
                    .get(0)
                    .map(|p| p.texture_stages.len() > 1)
                    .unwrap_or(false)
                {
                    has_multiuv = true;
                }
                if m.textures.iter().any(|t| {
                    (t.info.attributes & crate::writer_materials::W3DTEXTURE_TYPE_BUMPMAP) != 0
                }) {
                    has_bump = true;
                }
                if m.textures
                    .iter()
                    .any(|t| t.info.frame_count > 1 || (t.info.frame_rate > 0.0))
                {
                    has_uv_anim = true;
                }
                if m.aabtree.is_some() {
                    has_aabtree = true;
                }
                if m.per_tri_materials.is_some() {
                    has_pertri = true;
                }
                if m.shaders.iter().any(|s| s.alpha_test != 0) {
                    has_alpha_test = true;
                }
            }
        }

        if has_aabtree {
            c_aabtree += 1;
        }
        if has_lights {
            c_lights += 1;
        }
        if has_pertri {
            c_pertri += 1;
        }
        if has_vcol {
            c_vcol += 1;
        }
        if has_multiuv {
            c_multiuv += 1;
        }
        if has_detail {
            c_detail += 1;
        }
        if has_spec {
            c_spec += 1;
        }
        if has_emissive {
            c_emissive += 1;
        }
        if has_bump {
            c_bump += 1;
        }
        if has_lod {
            c_lod += 1;
        }
        if has_collision {
            c_collision += 1;
        }
        if has_shadow {
            c_shadow += 1;
        }
        if has_uv_anim {
            c_uv_anim += 1;
        }
        if has_alpha_test {
            c_alpha_test += 1;
        }

        if report.is_some() {
            rows.push(serde_json::json!({
                "file": path.display().to_string(),
                "aabtree": has_aabtree,
                "lights": has_lights,
                "per_tri_materials": has_pertri,
                "vertex_colors": has_vcol,
                "multi_uv_stages": has_multiuv,
                "detail_map": has_detail,
                "specular_map": has_spec,
                "emissive_map": has_emissive,
                "bump_map": has_bump,
                "lod_model": has_lod,
                "collision_node": has_collision,
                "shadow_node": has_shadow,
                "uv_animation": has_uv_anim,
                "alpha_test": has_alpha_test,
            }));
        }
    }
    println!("Feature scan in {}", dir.display());
    println!("- AABTree: {}", c_aabtree);
    println!("- Lights: {}", c_lights);
    println!("- Per-triangle materials: {}", c_pertri);
    println!("- Vertex colors: {}", c_vcol);
    println!("- Multi-UV stages: {}", c_multiuv);
    println!("- Detail map (DI): {}", c_detail);
    println!("- Specular map (SC): {}", c_spec);
    println!("- Emissive map (SI): {}", c_emissive);
    println!("- Bump map (attr 0x1000): {}", c_bump);
    println!("- LOD model: {}", c_lod);
    println!("- Collision node: {}", c_collision);
    println!("- Shadow node: {}", c_shadow);
    println!("- UV/texture animation: {}", c_uv_anim);
    println!("- Alpha-test (shader flag): {}", c_alpha_test);
    if let Some(rp) = report {
        let obj = serde_json::json!({
            "root": dir.display().to_string(),
            "summary": {
                "aabtree": c_aabtree, "lights": c_lights, "per_tri_materials": c_pertri, "vertex_colors": c_vcol,
                "multi_uv_stages": c_multiuv, "detail_map": c_detail, "specular_map": c_spec, "emissive_map": c_emissive,
                "bump_map": c_bump, "lod_model": c_lod, "collision_node": c_collision, "shadow_node": c_shadow,
                "uv_animation": c_uv_anim, "alpha_test": c_alpha_test,
            },
            "results": rows,
        });
        let mut f = std::fs::File::create(rp)?;
        serde_json::to_writer_pretty(&mut f, &obj)?;
    }
    Ok(())
}

fn raw_contains_chunk(data: &[u8], needle_id: u32) -> bool {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::{Cursor, Seek, SeekFrom};
    let mut r = Cursor::new(data);
    while (r.position() as usize) + 8 <= data.len() {
        let id = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        };
        let size = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        } & 0x7FFF_FFFF;
        let start = r.position();
        let end = start + size as u64;
        if id == needle_id {
            return true;
        }
        r.seek(SeekFrom::Start(end)).ok();
    }
    false
}

fn batch_convert(
    in_dir: &std::path::Path,
    out_root: &std::path::Path,
    limit: Option<usize>,
    random: bool,
    report: Option<&std::path::Path>,
    strict: bool,
    with_images: bool,
    embed_images: bool,
    images_root_opt: Option<&std::path::Path>,
) -> Result<()> {
    use rand::seq::SliceRandom;
    use rand::thread_rng;
    use std::time::Instant;

    std::fs::create_dir_all(out_root).ok();
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    // Non-recursive: depth 1
    for entry in std::fs::read_dir(in_dir)? {
        let entry = entry?;
        let p = entry.path();
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("w3d") {
                files.push(p);
            }
        }
    }
    if files.is_empty() {
        anyhow::bail!("No .w3d files in {}", in_dir.display());
    }
    if random {
        files.shuffle(&mut thread_rng());
    }
    if let Some(n) = limit {
        if files.len() > n {
            files.truncate(n);
        }
    }

    let mut results: Vec<serde_json::Value> = Vec::new();
    for path in files {
        let start = Instant::now();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
        let out_dir = out_root.join(stem);
        std::fs::create_dir_all(&out_dir).ok();
        let out_base = out_dir.join(stem);
        let mut status = "ok".to_string();
        let mut msg = String::new();
        let mut stats = serde_json::json!({});
        match std::fs::read(&path) {
            Ok(bytes) => {
                match w3d::parser::parse_w3d_file(&bytes) {
                    Ok(mut file) => {
                        // Local hierarchy auto-lookup for animation-only files
                        if file.hierarchies.is_empty() && !file.animations.is_empty() {
                            if let Some(parent) = path.parent() {
                                let _ = try_load_local_hierarchy(&mut file, parent);
                            }
                        }
                        match writer::convert_to_gltf(&file) {
                            Ok((mut root, bin)) => {
                                if strict {
                                    if let Err(e) = validate_gltf(&root, &bin) {
                                        status = "validation_error".into();
                                        msg = format!("{}", e);
                                    }
                                }
                                // set buffer URI
                                if let Some(buffers) = root
                                    .get_mut("buffers")
                                    .and_then(serde_json::Value::as_array_mut)
                                {
                                    if let Some(buf0) = buffers.get_mut(0) {
                                        if let Some(obj) = buf0.as_object_mut() {
                                            obj.insert(
                                                "uri".into(),
                                                serde_json::json!(format!("{}.bin", stem)),
                                            );
                                        }
                                    }
                                }
                                // Optionally attach images in batch based on flags
                                if with_images {
                                    let images_root = images_root_opt
                                        .unwrap_or(std::path::Path::new("extracted_big_files_v2"));
                                    if embed_images {
                                        if let Err(e) =
                                            resolve_and_embed_images(&mut root, images_root)
                                        {
                                            log::warn!("Embed images failed for {}: {}", stem, e);
                                        }
                                    } else {
                                        if let Err(e) =
                                            resolve_and_copy_images(&root, &out_dir, images_root)
                                        {
                                            log::warn!("Image copy failed for {}: {}", stem, e);
                                        }
                                    }
                                }
                                let gltf_path = out_base.with_extension("gltf");
                                let bin_path = out_base.with_extension("bin");
                                let mut f = std::fs::File::create(&gltf_path)?;
                                serde_json::to_writer_pretty(&mut f, &root)?;
                                f.flush()?;
                                std::fs::write(&bin_path, &bin)?;
                                // gather stats
                                stats = serde_json::json!({
                                    "meshes": root.get("meshes").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
                                    "nodes": root.get("nodes").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
                                    "materials": root.get("materials").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
                                    "animations": root.get("animations").and_then(|v| v.as_array()).map_or(0, |a| a.len()),
                                });
                            }
                            Err(e) => {
                                status = "convert_error".into();
                                msg = format!("{}", e);
                            }
                        }
                    }
                    Err(e) => {
                        status = "parse_error".into();
                        msg = format!("{}", e);
                    }
                }
            }
            Err(e) => {
                status = "read_error".into();
                msg = format!("{}", e);
            }
        }
        results.push(serde_json::json!({
            "file": path.file_name().and_then(|s| s.to_str()).unwrap_or_default(),
            "status": status,
            "message": msg,
            "elapsed_ms": start.elapsed().as_millis(),
            "stats": stats,
        }));
    }
    if let Some(rp) = report {
        let report_obj = serde_json::json!({ "root": out_root, "results": results });
        let mut f = std::fs::File::create(rp)?;
        serde_json::to_writer_pretty(&mut f, &report_obj)?;
    }
    Ok(())
}

fn validate_gltf(root: &serde_json::Value, bin: &[u8]) -> anyhow::Result<()> {
    use anyhow::{anyhow, bail};
    let buffers = root
        .get("buffers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("buffers missing"))?;
    if buffers.is_empty() {
        bail!("no buffers");
    }
    let buf0_len = buffers[0]
        .get("byteLength")
        .and_then(|v| v.as_u64())
        .unwrap_or(bin.len() as u64) as usize;
    if buf0_len != bin.len() {
        bail!(
            "buffer[0] byteLength {} != bin data {}",
            buf0_len,
            bin.len()
        );
    }
    let views_arr: Vec<serde_json::Value> = root
        .get("bufferViews")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for (i, v) in views_arr.iter().enumerate() {
        let buffer_index = v.get("buffer").and_then(|x| x.as_u64()).unwrap_or(0);
        if buffer_index != 0 {
            bail!(
                "bufferView[{}] references non-zero buffer {}",
                i,
                buffer_index
            );
        }
        let off = v.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let len = v.get("byteLength").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        if off.checked_add(len).unwrap_or(usize::MAX) > bin.len() {
            bail!(
                "bufferView[{}] out of bounds ({}+{}>{})",
                i,
                off,
                len,
                bin.len()
            );
        }
    }
    let accessors_arr: Vec<serde_json::Value> = root
        .get("accessors")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for (i, acc) in accessors_arr.iter().enumerate() {
        let view_idx = acc.get("bufferView").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        if view_idx >= views_arr.len() {
            bail!("accessor[{}] invalid bufferView {}", i, view_idx);
        }
        let view = &views_arr[view_idx];
        let view_len = view.get("byteLength").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let acc_off = acc.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let count = acc.get("count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let ctype = acc
            .get("componentType")
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let type_str = acc.get("type").and_then(|x| x.as_str()).unwrap_or("SCALAR");
        let comp_size = match ctype {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            _ => 4,
        };
        let elems = match type_str {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            _ => 1,
        };
        let need = acc_off
            .checked_add(count.saturating_mul(elems).saturating_mul(comp_size))
            .unwrap_or(usize::MAX);
        if need > view_len {
            bail!(
                "accessor[{}] exceeds bufferView (need {}, view_len {})",
                i,
                need,
                view_len
            );
        }
    }
    // Index bounds vs POSITION count
    let meshes_arr: Vec<serde_json::Value> = root
        .get("meshes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for (mi, m) in meshes_arr.iter().enumerate() {
        if let Some(prims) = m.get("primitives").and_then(|v| v.as_array()) {
            for (pi, p) in prims.iter().enumerate() {
                let attrs = p
                    .get("attributes")
                    .and_then(|v| v.as_object())
                    .ok_or_else(|| anyhow!("mesh[{}].prim[{}] missing attributes", mi, pi))?;
                let pos_acc = attrs.get("POSITION").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let pos_count = accessors_arr
                    .get(pos_acc)
                    .and_then(|a| a.get("count"))
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0) as usize;
                if let Some(ind_acc_u64) = p.get("indices").and_then(|x| x.as_u64()) {
                    let ind_acc = ind_acc_u64 as usize;
                    let view_idx = accessors_arr[ind_acc]
                        .get("bufferView")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0) as usize;
                    let view = &views_arr[view_idx];
                    let view_off =
                        view.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let acc_off = accessors_arr[ind_acc]
                        .get("byteOffset")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0) as usize;
                    let count = accessors_arr[ind_acc]
                        .get("count")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0) as usize;
                    let ctype = accessors_arr[ind_acc]
                        .get("componentType")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0) as u32;
                    let base = view_off + acc_off;
                    let bytes_per = if ctype == 5123 { 2 } else { 4 };
                    if base
                        .checked_add(count.saturating_mul(bytes_per))
                        .unwrap_or(usize::MAX)
                        > bin.len()
                    {
                        bail!("mesh[{}].prim[{}] index buffer out of bin bounds", mi, pi);
                    }
                    let mut max_idx = 0usize;
                    if ctype == 5123 {
                        for k in 0..count {
                            let i = base + k * 2;
                            let idx = u16::from_le_bytes([bin[i], bin[i + 1]]) as usize;
                            if idx > max_idx {
                                max_idx = idx;
                            }
                        }
                    } else {
                        for k in 0..count {
                            let i = base + k * 4;
                            let idx =
                                u32::from_le_bytes([bin[i], bin[i + 1], bin[i + 2], bin[i + 3]])
                                    as usize;
                            if idx > max_idx {
                                max_idx = idx;
                            }
                        }
                    }
                    if max_idx >= pos_count {
                        bail!(
                            "mesh[{}].prim[{}] index {} >= vertex count {}",
                            mi,
                            pi,
                            max_idx,
                            pos_count
                        );
                    }
                }
                // Optional: check TANGENT xyz are non-zero (validators expect unit length; at minimum ensure not zero)
                if let Some(tan_acc_u64) = attrs.get("TANGENT").and_then(|x| x.as_u64()) {
                    let tan_acc = tan_acc_u64 as usize;
                    if tan_acc >= accessors_arr.len() {
                        bail!("mesh[{}].prim[{}] TANGENT accessor out of range", mi, pi);
                    }
                    let acc = &accessors_arr[tan_acc];
                    let ctype = acc
                        .get("componentType")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0) as u32;
                    let type_str = acc.get("type").and_then(|x| x.as_str()).unwrap_or("");
                    let count = acc.get("count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let view_idx =
                        acc.get("bufferView").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let acc_off =
                        acc.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    if ctype == 5126 && type_str == "VEC4" && view_idx < views_arr.len() {
                        let view = &views_arr[view_idx];
                        let view_off =
                            view.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        let stride = 16; // 4 f32 per element
                        let base = view_off + acc_off;
                        for k in 0..count {
                            let i = base + k * stride;
                            if i + 12 > bin.len() {
                                break;
                            }
                            let x =
                                f32::from_le_bytes([bin[i], bin[i + 1], bin[i + 2], bin[i + 3]]);
                            let y = f32::from_le_bytes([
                                bin[i + 4],
                                bin[i + 5],
                                bin[i + 6],
                                bin[i + 7],
                            ]);
                            let z = f32::from_le_bytes([
                                bin[i + 8],
                                bin[i + 9],
                                bin[i + 10],
                                bin[i + 11],
                            ]);
                            let len2 = x * x + y * y + z * z;
                            if !len2.is_finite() || len2 <= 0.0 {
                                bail!(
                                    "mesh[{}].prim[{}] TANGENT xyz zero-length at element {}",
                                    mi,
                                    pi,
                                    k
                                );
                            }
                        }
                    }
                }

                // Optional: check NORMAL length (non-zero, near unit if strict callers want stronger checks)
                if let Some(n_acc_u64) = attrs.get("NORMAL").and_then(|x| x.as_u64()) {
                    let n_acc = n_acc_u64 as usize;
                    if n_acc >= accessors_arr.len() {
                        bail!("mesh[{}].prim[{}] NORMAL accessor out of range", mi, pi);
                    }
                    let acc = &accessors_arr[n_acc];
                    let ctype = acc
                        .get("componentType")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0) as u32;
                    let type_str = acc.get("type").and_then(|x| x.as_str()).unwrap_or("");
                    let count = acc.get("count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let view_idx =
                        acc.get("bufferView").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    let acc_off =
                        acc.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    if ctype == 5126 && type_str == "VEC3" && view_idx < views_arr.len() {
                        let view = &views_arr[view_idx];
                        let view_off =
                            view.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                        let base = view_off + acc_off;
                        let stride = 12usize;
                        for k in 0..count {
                            let i = base + k * stride;
                            if i + 12 > bin.len() {
                                break;
                            }
                            let x =
                                f32::from_le_bytes([bin[i], bin[i + 1], bin[i + 2], bin[i + 3]]);
                            let y = f32::from_le_bytes([
                                bin[i + 4],
                                bin[i + 5],
                                bin[i + 6],
                                bin[i + 7],
                            ]);
                            let z = f32::from_le_bytes([
                                bin[i + 8],
                                bin[i + 9],
                                bin[i + 10],
                                bin[i + 11],
                            ]);
                            let len2 = x * x + y * y + z * z;
                            if !len2.is_finite() || len2 <= 0.0 {
                                bail!(
                                    "mesh[{}].prim[{}] NORMAL zero-length at element {}",
                                    mi,
                                    pi,
                                    k
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_gltf_on_disk(gltf_path: &std::path::Path, _strict: bool) -> anyhow::Result<()> {
    let data = std::fs::read_to_string(gltf_path)?;
    let root: serde_json::Value = serde_json::from_str(&data)?;
    // Resolve bin path from buffers[0].uri relative to gltf folder
    let bin_path = if let Some(buffers) = root.get("buffers").and_then(|v| v.as_array()) {
        if let Some(uri) = buffers
            .get(0)
            .and_then(|b| b.get("uri"))
            .and_then(|u| u.as_str())
        {
            gltf_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(uri)
        } else {
            let stem = gltf_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("buffer");
            gltf_path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(format!("{}.bin", stem))
        }
    } else {
        anyhow::bail!("No buffers[] in glTF; nothing to validate")
    };
    let bin = std::fs::read(&bin_path)?;
    validate_gltf(&root, &bin)?;
    println!("OK: {}", gltf_path.display());
    Ok(())
}

fn validate_gltf_in_dir(
    dir: &std::path::Path,
    recursive: bool,
    report: Option<&std::path::Path>,
    strict: bool,
) -> anyhow::Result<()> {
    let mut results: Vec<serde_json::Value> = Vec::new();
    if recursive {
        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("gltf"))
                .unwrap_or(false)
            {
                match validate_gltf_on_disk(p, strict) {
                    Ok(_) => results
                        .push(serde_json::json!({"file": p.display().to_string(), "status": "ok"})),
                    Err(e) => {
                        println!("ERR: {} => {}", p.display(), e);
                        results.push(serde_json::json!({"file": p.display().to_string(), "status": "error", "message": e.to_string()}));
                    }
                }
            }
        }
    } else {
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            if p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("gltf"))
                .unwrap_or(false)
            {
                match validate_gltf_on_disk(&p, strict) {
                    Ok(_) => results
                        .push(serde_json::json!({"file": p.display().to_string(), "status": "ok"})),
                    Err(e) => {
                        println!("ERR: {} => {}", p.display(), e);
                        results.push(serde_json::json!({"file": p.display().to_string(), "status": "error", "message": e.to_string()}));
                    }
                }
            }
        }
    }
    if let Some(rp) = report {
        let mut f = std::fs::File::create(rp)?;
        serde_json::to_writer_pretty(
            &mut f,
            &serde_json::json!({"root": dir.display().to_string(), "results": results}),
        )?;
    }
    Ok(())
}

fn build_filename_index(
    root: &std::path::Path,
) -> anyhow::Result<HashMap<String, std::path::PathBuf>> {
    let mut map = HashMap::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Some(name) = entry.path().file_name().and_then(|s| s.to_str()) {
                map.insert(name.to_lowercase(), entry.path().to_path_buf());
            }
        }
    }
    Ok(map)
}

fn is_data_uri(uri: &str) -> bool {
    uri.starts_with("data:")
}

fn resolve_and_copy_images(
    root: &serde_json::Value,
    out_dir: &std::path::Path,
    images_root: &std::path::Path,
) -> anyhow::Result<()> {
    let Some(images) = root.get("images").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    if images.is_empty() {
        return Ok(());
    }
    let index = build_filename_index(images_root)?;
    std::fs::create_dir_all(out_dir).ok();
    for img in images {
        let Some(uri) = img.get("uri").and_then(|v| v.as_str()) else {
            continue;
        };
        if is_data_uri(uri) {
            continue;
        }
        let name = std::path::Path::new(uri)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(uri);
        let key = name.to_lowercase();
        if let Some(src_path) = index.get(&key) {
            let dst = out_dir.join(name);
            // Best-effort copy
            std::fs::copy(src_path, &dst).ok();
        }
    }
    Ok(())
}

fn mime_from_ext(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".tga") {
        "image/x-tga"
    } else if lower.ends_with(".dds") {
        "image/vnd-ms.dds"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "application/octet-stream"
    }
}

fn resolve_and_embed_images(
    root: &mut serde_json::Value,
    images_root: &std::path::Path,
) -> anyhow::Result<()> {
    let Some(images) = root.get_mut("images").and_then(|v| v.as_array_mut()) else {
        return Ok(());
    };
    if images.is_empty() {
        return Ok(());
    }
    let index = build_filename_index(images_root)?;
    for img in images.iter_mut() {
        let Some(uri_val) = img.get_mut("uri") else {
            continue;
        };
        let Some(uri) = uri_val.as_str() else {
            continue;
        };
        if is_data_uri(uri) {
            continue;
        }
        let name = std::path::Path::new(uri)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(uri);
        let key = name.to_lowercase();
        if let Some(src_path) = index.get(&key) {
            if let Ok(data) = std::fs::read(src_path) {
                let mime = mime_from_ext(name);
                let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                *uri_val = serde_json::Value::String(format!("data:{};base64,{}", mime, b64));
            }
        }
    }
    Ok(())
}

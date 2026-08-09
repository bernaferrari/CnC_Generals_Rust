//! Source-assert: production GameClient must not import Direct3D Win32 APIs.
//!
//! Graphics go through wgpu. The `windows` crate D3D/DXGI features were unused
//! and are not part of the game production graph.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN_IMPORT: &str = "use windows::Win32::Graphics::Direct3D";
const FORBIDDEN_PATH: &str = "windows::Win32::Graphics::Direct3D";
const FORBIDDEN_CARGO_FEATURES: &[&str] = &[
    "Win32_Graphics_Direct3D",
    "Win32_Graphics_Direct3D11",
    "Win32_Graphics_Direct3D12",
    "Win32_Graphics_Dxgi",
    "Win32_Media_Audio_DirectSound",
];

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!")
}

#[test]
fn production_sources_do_not_import_direct3d() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(
        !files.is_empty(),
        "expected Rust sources under {}",
        src.display()
    );

    let mut offenders = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (idx, line) in text.lines().enumerate() {
            if is_comment_line(line) {
                continue;
            }
            if line.contains(FORBIDDEN_IMPORT) || line.contains(FORBIDDEN_PATH) {
                offenders.push(format!(
                    "{}:{}: {}",
                    path.display(),
                    idx + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "production sources must not `{FORBIDDEN_IMPORT}` (wgpu is the device backend):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn cargo_toml_omits_direct3d_windows_features() {
    let cargo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = fs::read_to_string(&cargo).unwrap_or_else(|e| panic!("read {}: {e}", cargo.display()));
    let mut offenders = Vec::new();
    for feature in FORBIDDEN_CARGO_FEATURES {
        if text.contains(feature) {
            offenders.push(*feature);
        }
    }
    assert!(
        offenders.is_empty(),
        "{} must not declare unused Direct3D/DirectSound features: {}",
        cargo.display(),
        offenders.join(", ")
    );
    assert!(
        !text.contains("gpu-allocator"),
        "{} must not depend on gpu-allocator (unused; wgpu owns GPU memory)",
        cargo.display()
    );
}

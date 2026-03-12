use anyhow::Result;
use serde_json::to_string_pretty;
use std::path::PathBuf;
use ww3d_validation::{capture_snapshot, diff_snapshots, read_snapshot};

const BASELINE_PATH: &str = "baselines/ww3d_smoke.json";

#[test]
fn ww3d_asset_parity_smoke() -> Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace layout");

    let assets = [
        repo_root.join(
            "GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/RequiredAssets/ShatterPlanes0.w3d",
        ),
        repo_root.join("windows_game/extracted_big_files_v2/W3DZH/Art/W3D/CBChalet3.w3d"),
    ];

    let snapshot = capture_snapshot(&assets)?;
    println!("captured snapshot:\n{}", to_string_pretty(&snapshot)?);
    let baseline = read_snapshot(manifest_dir.join(BASELINE_PATH))?;

    let diffs = diff_snapshots(&baseline, &snapshot);
    assert!(
        diffs.is_empty(),
        "WW3D asset smoke test mismatches:\n{}\nCaptured snapshot:\n{}",
        diffs.join("\n"),
        to_string_pretty(&snapshot)?
    );

    Ok(())
}

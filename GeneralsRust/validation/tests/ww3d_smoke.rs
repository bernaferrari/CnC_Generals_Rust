use anyhow::Result;
use serde_json::to_string_pretty;
use ww3d_core::W3DChunkType;
use ww3d_validation::{capture_snapshot_from_bytes, diff_snapshots, read_snapshot};

const BASELINE_PATH: &str = "baselines/ww3d_smoke.json";

#[test]
fn ww3d_asset_parity_smoke() -> Result<()> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let snapshot = capture_snapshot_from_bytes("fixture_mesh", &minimal_mesh_w3d("FixtureMesh"))?;
    let fixture = snapshot
        .assets
        .values()
        .next()
        .expect("required fixture snapshot");
    assert!(!fixture.prototype_names.is_empty());
    assert!(
        fixture.total_vertices > 0,
        "fixture topology must be nonzero"
    );
    assert!(
        fixture.total_triangles > 0,
        "fixture topology must be nonzero"
    );
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

fn minimal_mesh_w3d(mesh_name: &str) -> Vec<u8> {
    let mut header = Vec::new();
    push_u32(&mut header, 0x0003_0000);
    push_u32(&mut header, 0);
    header.extend_from_slice(&fixed_bytes::<16>(mesh_name));
    header.extend_from_slice(&[0; 16]);
    push_u32(&mut header, 1); // triangles
    push_u32(&mut header, 3); // vertices
    push_u32(&mut header, 0); // materials
    push_u32(&mut header, 0); // damage stages
    push_i32(&mut header, 0);
    push_u32(&mut header, 0);
    push_u32(&mut header, 0);
    push_u32(&mut header, 0x0000_0007);
    push_u32(&mut header, 0x0000_0001);
    push_vec3(&mut header, [0.0, 0.0, 0.0]);
    push_vec3(&mut header, [1.0, 1.0, 0.0]);
    push_vec3(&mut header, [0.5, 0.5, 0.0]);
    push_f32(&mut header, 1.0);

    let vertices = vectors(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
    let normals = vectors(&[[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]]);
    let mut triangles = Vec::new();
    for value in [0u32, 1, 2, 0] {
        push_u32(&mut triangles, value);
    }
    for value in [0.0f32, 0.0, 1.0, 0.0] {
        push_f32(&mut triangles, value);
    }

    chunk(
        W3DChunkType::Mesh,
        true,
        [
            chunk(W3DChunkType::MeshHeader3, false, header),
            chunk(W3DChunkType::Vertices, false, vertices),
            chunk(W3DChunkType::VertexNormals, false, normals),
            chunk(W3DChunkType::Triangles, false, triangles),
        ]
        .concat(),
    )
}

fn chunk(kind: W3DChunkType, sub_chunks: bool, payload: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + payload.len());
    push_u32(&mut bytes, kind.as_u32());
    push_u32(
        &mut bytes,
        payload.len() as u32 | if sub_chunks { 0x8000_0000 } else { 0 },
    );
    bytes.extend_from_slice(&payload);
    bytes
}

fn vectors(values: &[[f32; 3]]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 12);
    for value in values {
        push_vec3(&mut bytes, *value);
    }
    bytes
}

fn fixed_bytes<const N: usize>(value: &str) -> [u8; N] {
    let mut bytes = [0; N];
    let copy_len = value.len().min(N);
    bytes[..copy_len].copy_from_slice(&value.as_bytes()[..copy_len]);
    bytes
}

fn push_vec3(bytes: &mut Vec<u8>, value: [f32; 3]) {
    for component in value {
        push_f32(bytes, component);
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

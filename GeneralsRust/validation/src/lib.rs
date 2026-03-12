use anyhow::{Context, Result};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use ww3d_assets::prototypes::{
    CollectionPrototype, HlodPrototype, LodModelPrototype, MeshPrototype,
};
use ww3d_assets::AssetManager;
use ww3d_geometry::bounding_volumes::BoundingVolumeUtils;

/// Snapshot describing the state of loaded WW3D assets for validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub assets: BTreeMap<String, AssetSnapshot>,
    pub lod_models: BTreeMap<String, Vec<LodEntrySnapshot>>,
    pub collections: BTreeMap<String, CollectionSnapshot>,
    pub hlods: BTreeMap<String, HLodSnapshot>,
}

impl Snapshot {
    pub fn new() -> Self {
        Self {
            assets: BTreeMap::new(),
            lod_models: BTreeMap::new(),
            collections: BTreeMap::new(),
            hlods: BTreeMap::new(),
        }
    }
}

/// Summary for all mesh prototypes discovered in a single asset file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetSnapshot {
    pub meshes: BTreeMap<String, MeshSnapshot>,
    pub total_vertices: usize,
    pub total_triangles: usize,
}

/// Captures geometric stats for a mesh prototype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshSnapshot {
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub bounding_sphere_radius: f32,
    pub bounding_sphere_center: [f32; 3],
    pub aabb_min: [f32; 3],
    pub aabb_max: [f32; 3],
}

/// Captures a single LOD entry for a LodModelPrototype.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LodEntrySnapshot {
    pub render_obj: String,
    pub lod_min: f32,
    pub lod_max: f32,
}

/// Summary of collection placeholders and transforms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionSnapshot {
    pub object_names: Vec<String>,
    pub placeholder_count: usize,
    pub transform_node_count: usize,
    pub snap_point_count: usize,
}

/// Summary of HLOD sub-objects per LOD layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HLodSnapshot {
    pub hierarchy: String,
    pub lod_layers: Vec<HLodLayerSnapshot>,
    pub aggregate_chunk_count: usize,
}

/// Snapshot describing sub-objects bound to a single HLOD LOD level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HLodLayerSnapshot {
    pub max_screen_size: f32,
    pub sub_object_count: usize,
    pub proxy_count: usize,
}

/// Capture a validation snapshot for the provided assets. Paths are resolved relative to `cwd`.
pub fn capture_snapshot<P>(assets: &[P]) -> Result<Snapshot>
where
    P: AsRef<Path>,
{
    let mut manager = AssetManager::new();
    let mut snapshot = Snapshot::new();

    for asset_path in assets {
        let path = asset_path.as_ref();
        let label = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid asset path: {}", path.display()))?
            .to_string();

        let before: HashSet<String> = manager.asset_names().cloned().collect();
        manager
            .load_w3d(path)
            .with_context(|| format!("failed to load W3D asset {}", path.display()))?;

        let after: HashSet<String> = manager.asset_names().cloned().collect();
        let new_entries: Vec<String> = after.difference(&before).cloned().collect();

        let mut mesh_map = BTreeMap::new();
        let mut total_vertices = 0usize;
        let mut total_triangles = 0usize;

        for name in &new_entries {
            let prototype = manager
                .get_prototype(name)
                .with_context(|| format!("missing prototype {}", name))?;

            if let Some(mesh) = prototype.as_any().downcast_ref::<MeshPrototype>() {
                let Some(mesh_name) = sanitize_label(name) else {
                    continue;
                };
                let stats = capture_mesh_snapshot(mesh);
                total_vertices += stats.vertex_count;
                total_triangles += stats.triangle_count;
                mesh_map.insert(mesh_name, stats);
            } else if let Some(lod) = prototype.as_any().downcast_ref::<LodModelPrototype>() {
                if let Some(lod_name) = sanitize_label(&lod.name) {
                    snapshot
                        .lod_models
                        .insert(lod_name, capture_lod_snapshot(lod));
                }
            } else if let Some(collection) =
                prototype.as_any().downcast_ref::<CollectionPrototype>()
            {
                if let Some(collection_name) = sanitize_label(&collection.name) {
                    snapshot
                        .collections
                        .insert(collection_name, capture_collection_snapshot(collection));
                }
            } else if let Some(hlod) = prototype.as_any().downcast_ref::<HlodPrototype>() {
                if let Some(hlod_name) = sanitize_label(&hlod.name) {
                    snapshot
                        .hlods
                        .insert(hlod_name, capture_hlod_snapshot(hlod));
                }
            }
        }

        if !mesh_map.is_empty() {
            snapshot.assets.insert(
                label,
                AssetSnapshot {
                    meshes: mesh_map,
                    total_vertices,
                    total_triangles,
                },
            );
        }
    }

    Ok(snapshot)
}

fn sanitize_label(raw: &str) -> Option<String> {
    let trimmed = raw.trim_matches('\0').trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(trimmed.to_string())
}

fn capture_mesh_snapshot(mesh: &MeshPrototype) -> MeshSnapshot {
    let points: Vec<Vec3> = mesh.vertices.iter().map(|&v| Vec3::from(v)).collect();

    let sphere = BoundingVolumeUtils::compute_bounding_sphere(&points);
    let aabb = BoundingVolumeUtils::compute_optimal_aabb(&points);

    let min = aabb.center - aabb.extent;
    let max = aabb.center + aabb.extent;

    MeshSnapshot {
        vertex_count: mesh.vertices.len(),
        triangle_count: mesh.triangles.len(),
        bounding_sphere_radius: sphere.radius,
        bounding_sphere_center: sphere.center.into(),
        aabb_min: min.into(),
        aabb_max: max.into(),
    }
}

fn capture_lod_snapshot(lod: &LodModelPrototype) -> Vec<LodEntrySnapshot> {
    lod.lods
        .iter()
        .map(|entry| LodEntrySnapshot {
            render_obj: entry.render_obj_name.clone(),
            lod_min: entry.lod_min,
            lod_max: entry.lod_max,
        })
        .collect()
}

fn capture_collection_snapshot(collection: &CollectionPrototype) -> CollectionSnapshot {
    CollectionSnapshot {
        object_names: collection.object_names.clone(),
        placeholder_count: collection.placeholders.len(),
        transform_node_count: collection.transform_nodes.len(),
        snap_point_count: collection.snap_points.len(),
    }
}

fn capture_hlod_snapshot(hlod: &HlodPrototype) -> HLodSnapshot {
    HLodSnapshot {
        hierarchy: hlod.hierarchy_name.clone(),
        lod_layers: hlod
            .lods
            .iter()
            .map(|lod_entry| HLodLayerSnapshot {
                max_screen_size: lod_entry.max_screen_size,
                sub_object_count: lod_entry.models.len(),
                proxy_count: lod_entry
                    .models
                    .iter()
                    .filter(|model| model.bone_index != u32::MAX)
                    .count(),
            })
            .collect(),
        aggregate_chunk_count: hlod.aggregate_chunks.len(),
    }
}

/// Serialize a snapshot to JSON.
pub fn write_snapshot(snapshot: &Snapshot, path: impl AsRef<Path>) -> Result<()> {
    let json = serde_json::to_vec_pretty(snapshot)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a snapshot from JSON.
pub fn read_snapshot(path: impl AsRef<Path>) -> Result<Snapshot> {
    let bytes = std::fs::read(path.as_ref())
        .with_context(|| format!("failed to read snapshot {}", path.as_ref().display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Compare two snapshots and return a list of discrepancies. Empty list means parity.
pub fn diff_snapshots(expected: &Snapshot, actual: &Snapshot) -> Vec<String> {
    let mut diffs = Vec::new();

    for (asset, expected_snapshot) in &expected.assets {
        match actual.assets.get(asset) {
            Some(actual_snapshot) => {
                if expected_snapshot.total_vertices != actual_snapshot.total_vertices {
                    diffs.push(format!(
                        "asset {} vertex count mismatch: expected {}, got {}",
                        asset, expected_snapshot.total_vertices, actual_snapshot.total_vertices
                    ));
                }
                if expected_snapshot.total_triangles != actual_snapshot.total_triangles {
                    diffs.push(format!(
                        "asset {} triangle count mismatch: expected {}, got {}",
                        asset, expected_snapshot.total_triangles, actual_snapshot.total_triangles
                    ));
                }

                for (mesh_name, mesh_expected) in &expected_snapshot.meshes {
                    match actual_snapshot.meshes.get(mesh_name) {
                        Some(mesh_actual) => {
                            diff_mesh(asset, mesh_name, mesh_expected, mesh_actual, &mut diffs);
                        }
                        None => diffs.push(format!(
                            "asset {} missing mesh {} in actual snapshot",
                            asset, mesh_name
                        )),
                    }
                }
            }
            None => diffs.push(format!(
                "missing asset snapshot for {} in actual data",
                asset
            )),
        }
    }

    for asset in actual.assets.keys() {
        if !expected.assets.contains_key(asset) {
            diffs.push(format!(
                "unexpected asset {} present in actual snapshot",
                asset
            ));
        }
    }

    for (lod_name, expected_entries) in &expected.lod_models {
        match actual.lod_models.get(lod_name) {
            Some(actual_entries) => {
                if expected_entries.len() != actual_entries.len() {
                    diffs.push(format!(
                        "lod {} entry count mismatch: expected {}, got {}",
                        lod_name,
                        expected_entries.len(),
                        actual_entries.len()
                    ));
                    continue;
                }

                for (idx, (expected_entry, actual_entry)) in
                    expected_entries.iter().zip(actual_entries).enumerate()
                {
                    if expected_entry.render_obj != actual_entry.render_obj {
                        diffs.push(format!(
                            "lod {} entry {} render object mismatch: expected {}, got {}",
                            lod_name, idx, expected_entry.render_obj, actual_entry.render_obj
                        ));
                    }
                    if !approx_eq(expected_entry.lod_min, actual_entry.lod_min)
                        || !approx_eq(expected_entry.lod_max, actual_entry.lod_max)
                    {
                        diffs.push(format!(
                            "lod {} entry {} range mismatch: expected [{:.3}, {:.3}], got [{:.3}, {:.3}]",
                            lod_name,
                            idx,
                            expected_entry.lod_min,
                            expected_entry.lod_max,
                            actual_entry.lod_min,
                            actual_entry.lod_max
                        ));
                    }
                }
            }
            None => diffs.push(format!("missing lod model {} in actual snapshot", lod_name)),
        }
    }

    for lod_name in actual.lod_models.keys() {
        if !expected.lod_models.contains_key(lod_name) {
            diffs.push(format!(
                "unexpected lod model {} present in actual snapshot",
                lod_name
            ));
        }
    }

    for (collection, expected_collection) in &expected.collections {
        match actual.collections.get(collection) {
            Some(actual_collection) => {
                if expected_collection.placeholder_count != actual_collection.placeholder_count {
                    diffs.push(format!(
                        "collection {} placeholder count mismatch: expected {}, got {}",
                        collection,
                        expected_collection.placeholder_count,
                        actual_collection.placeholder_count
                    ));
                }
                if expected_collection.transform_node_count
                    != actual_collection.transform_node_count
                {
                    diffs.push(format!(
                        "collection {} transform count mismatch: expected {}, got {}",
                        collection,
                        expected_collection.transform_node_count,
                        actual_collection.transform_node_count
                    ));
                }
                if expected_collection.snap_point_count != actual_collection.snap_point_count {
                    diffs.push(format!(
                        "collection {} snap point count mismatch: expected {}, got {}",
                        collection,
                        expected_collection.snap_point_count,
                        actual_collection.snap_point_count
                    ));
                }
            }
            None => diffs.push(format!(
                "missing collection {} in actual snapshot",
                collection
            )),
        }
    }

    for collection in actual.collections.keys() {
        if !expected.collections.contains_key(collection) {
            diffs.push(format!(
                "unexpected collection {} present in actual snapshot",
                collection
            ));
        }
    }

    for (name, expected_hlod) in &expected.hlods {
        match actual.hlods.get(name) {
            Some(actual_hlod) => {
                if expected_hlod.hierarchy != actual_hlod.hierarchy {
                    diffs.push(format!(
                        "hlod {} hierarchy mismatch: expected {}, got {}",
                        name, expected_hlod.hierarchy, actual_hlod.hierarchy
                    ));
                }
                if expected_hlod.aggregate_chunk_count != actual_hlod.aggregate_chunk_count {
                    diffs.push(format!(
                        "hlod {} aggregate chunk count mismatch: expected {}, got {}",
                        name,
                        expected_hlod.aggregate_chunk_count,
                        actual_hlod.aggregate_chunk_count
                    ));
                }
                if expected_hlod.lod_layers.len() != actual_hlod.lod_layers.len() {
                    diffs.push(format!(
                        "hlod {} layer count mismatch: expected {}, got {}",
                        name,
                        expected_hlod.lod_layers.len(),
                        actual_hlod.lod_layers.len()
                    ));
                    continue;
                }
                for (idx, (expected_layer, actual_layer)) in expected_hlod
                    .lod_layers
                    .iter()
                    .zip(actual_hlod.lod_layers.iter())
                    .enumerate()
                {
                    if !approx_eq(expected_layer.max_screen_size, actual_layer.max_screen_size) {
                        diffs.push(format!(
                            "hlod {} layer {} max screen size mismatch: expected {:.3}, got {:.3}",
                            name, idx, expected_layer.max_screen_size, actual_layer.max_screen_size
                        ));
                    }
                    if expected_layer.sub_object_count != actual_layer.sub_object_count {
                        diffs.push(format!(
                            "hlod {} layer {} sub-object count mismatch: expected {}, got {}",
                            name,
                            idx,
                            expected_layer.sub_object_count,
                            actual_layer.sub_object_count
                        ));
                    }
                    if expected_layer.proxy_count != actual_layer.proxy_count {
                        diffs.push(format!(
                            "hlod {} layer {} proxy count mismatch: expected {}, got {}",
                            name, idx, expected_layer.proxy_count, actual_layer.proxy_count
                        ));
                    }
                }
            }
            None => diffs.push(format!("missing hlod {} in actual snapshot", name)),
        }
    }

    for name in actual.hlods.keys() {
        if !expected.hlods.contains_key(name) {
            diffs.push(format!(
                "unexpected hlod {} present in actual snapshot",
                name
            ));
        }
    }

    diffs
}

fn diff_mesh(
    asset: &str,
    mesh_name: &str,
    expected: &MeshSnapshot,
    actual: &MeshSnapshot,
    diffs: &mut Vec<String>,
) {
    if expected.vertex_count != actual.vertex_count {
        diffs.push(format!(
            "asset {} mesh {} vertex count mismatch: expected {}, got {}",
            asset, mesh_name, expected.vertex_count, actual.vertex_count
        ));
    }
    if expected.triangle_count != actual.triangle_count {
        diffs.push(format!(
            "asset {} mesh {} triangle count mismatch: expected {}, got {}",
            asset, mesh_name, expected.triangle_count, actual.triangle_count
        ));
    }
    if !approx_eq(
        expected.bounding_sphere_radius,
        actual.bounding_sphere_radius,
    ) {
        diffs.push(format!(
            "asset {} mesh {} bounding sphere radius mismatch: expected {:.3}, got {:.3}",
            asset, mesh_name, expected.bounding_sphere_radius, actual.bounding_sphere_radius
        ));
    }
    if !approx_vec(
        &expected.bounding_sphere_center,
        &actual.bounding_sphere_center,
    ) {
        diffs.push(format!(
            "asset {} mesh {} bounding sphere center mismatch: expected {:?}, got {:?}",
            asset, mesh_name, expected.bounding_sphere_center, actual.bounding_sphere_center
        ));
    }
    if !approx_vec(&expected.aabb_min, &actual.aabb_min)
        || !approx_vec(&expected.aabb_max, &actual.aabb_max)
    {
        diffs.push(format!(
            "asset {} mesh {} AABB mismatch: expected min {:?} max {:?}, got min {:?} max {:?}",
            asset,
            mesh_name,
            expected.aabb_min,
            expected.aabb_max,
            actual.aabb_min,
            actual.aabb_max
        ));
    }
}

fn approx_vec(lhs: &[f32; 3], rhs: &[f32; 3]) -> bool {
    lhs.iter().zip(rhs.iter()).all(|(a, b)| approx_eq(*a, *b))
}

fn approx_eq(lhs: f32, rhs: f32) -> bool {
    if lhs == rhs {
        return true;
    }
    let diff = (lhs - rhs).abs();
    let scale = lhs.abs().max(rhs.abs()).max(1.0);
    diff <= 1e-3 * scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn smoke_capture_on_required_asset() -> Result<()> {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../Code/Libraries/Source/WWVegas/WW3D2/RequiredAssets");
        let snapshot = capture_snapshot([base.join("ShatterPlanes0.w3d")])?;
        // Ensure we captured at least one mesh and aggregate stats make sense.
        assert!(
            !snapshot.assets.is_empty(),
            "expected at least one asset snapshot"
        );
        Ok(())
    }
}

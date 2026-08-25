use anyhow::{Context, Result};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use ww3d_assets::prototypes::{
    CollectionPrototype, HlodPrototype, LodModelPrototype, MeshPrototype,
};
use ww3d_assets::{AssetManager, W3DLoader};
use ww3d_geometry::bounding_volumes::BoundingVolumeUtils;

/// Snapshot describing the state of loaded WW3D assets for validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub assets: BTreeMap<String, AssetSnapshot>,
    pub lod_models: BTreeMap<String, Vec<LodEntrySnapshot>>,
    pub collections: BTreeMap<String, CollectionSnapshot>,
    pub hlods: BTreeMap<String, HLodSnapshot>,
    #[serde(default)]
    pub animations: BTreeMap<String, AnimationSnapshot>,
    #[serde(default)]
    pub hierarchies: BTreeMap<String, HierarchySnapshot>,
}

impl Snapshot {
    pub fn new() -> Self {
        Self {
            assets: BTreeMap::new(),
            lod_models: BTreeMap::new(),
            collections: BTreeMap::new(),
            hlods: BTreeMap::new(),
            animations: BTreeMap::new(),
            hierarchies: BTreeMap::new(),
        }
    }
}

/// Summary for all mesh prototypes discovered in a single asset file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetSnapshot {
    /// Every prototype name registered from this file, including non-mesh
    /// render objects.  C++ `WW3DAssetManager` rejects duplicate names, so
    /// an unexpected name is a parity failure even when its geometry is empty.
    #[serde(default)]
    pub prototype_names: Vec<String>,
    pub meshes: BTreeMap<String, MeshSnapshot>,
    pub total_vertices: usize,
    pub total_triangles: usize,
    /// Texture references in the order emitted by the W3D mesh loader.
    #[serde(default)]
    pub texture_order: Vec<String>,
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
    #[serde(default)]
    pub texture_order: Vec<String>,
    #[serde(default)]
    pub vertex_material_order: Vec<String>,
    #[serde(default)]
    pub material_info: Option<MaterialInfoSnapshot>,
    #[serde(default)]
    pub material_passes: Vec<MaterialPassSnapshot>,
}

/// Backend-neutral material counts and pass ordering.  This intentionally
/// contains no DirectX/WGPU handles: it describes the W3D material contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialInfoSnapshot {
    pub pass_count: u32,
    pub vertex_material_count: u32,
    pub shader_count: u32,
    pub texture_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaterialPassSnapshot {
    pub vertex_material_id: u32,
    pub shader_id: u32,
    pub texture_count: u32,
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
    #[serde(default)]
    pub placeholder_names: Vec<String>,
    #[serde(default)]
    pub transform_node_names: Vec<String>,
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
    #[serde(default)]
    pub proxy_objects: Vec<HLodObjectSnapshot>,
}

/// Snapshot describing sub-objects bound to a single HLOD LOD level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HLodLayerSnapshot {
    pub max_screen_size: f32,
    pub sub_object_count: usize,
    pub proxy_count: usize,
    #[serde(default)]
    pub sub_objects: Vec<HLodObjectSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HLodObjectSnapshot {
    pub name: String,
    pub bone_index: u32,
}

/// A hierarchy name and pivot ordering, matching C++ `HTreeManager`'s
/// ordered pivot table without including backend-specific transforms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HierarchySnapshot {
    pub pivot_order: Vec<String>,
    pub parent_indices: Vec<i32>,
}

/// Animation identity and channel presence/order.  The source C++ loader
/// keeps one channel set per bone; preserving that shape catches dropped or
/// reordered animation channels while remaining backend neutral.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimationSnapshot {
    pub hierarchy: String,
    pub num_frames: u32,
    pub frame_rate: u16,
    pub bone_channels: Vec<AnimationBoneSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimationBoneSnapshot {
    pub has_x: bool,
    pub has_y: bool,
    pub has_z: bool,
    pub has_rotation: bool,
    pub has_visibility: bool,
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

        // Keep a parser-level view as well as the AssetManager view.  The C++
        // manager handles hierarchy and animation chunks before registering
        // render-object prototypes; those resources are not all represented by
        // AssetManager prototypes yet, so dropping this view would make a
        // missing animation look like a successful empty validation.
        let model = W3DLoader::load(path)
            .with_context(|| format!("failed to parse W3D asset {}", path.display()))?;

        let before: HashSet<String> = manager.asset_names().cloned().collect();
        manager
            .load_w3d(path)
            .with_context(|| format!("failed to load W3D asset {}", path.display()))?;

        let after: HashSet<String> = manager.asset_names().cloned().collect();
        let new_entries: Vec<String> = after.difference(&before).cloned().collect();

        let mut prototype_names: Vec<String> = new_entries
            .iter()
            .filter_map(|name| sanitize_label(name))
            .collect();
        prototype_names.sort();

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

        let texture_order = model
            .textures
            .iter()
            .filter_map(|name| sanitize_label(name))
            .collect();
        snapshot.assets.insert(
            label,
            AssetSnapshot {
                prototype_names,
                meshes: mesh_map,
                total_vertices,
                total_triangles,
                texture_order,
            },
        );

        for hierarchy in model.hierarchies {
            let Some(name) = sanitize_label(&hierarchy.header.name) else {
                continue;
            };
            snapshot.hierarchies.insert(
                name,
                HierarchySnapshot {
                    pivot_order: hierarchy
                        .pivots
                        .iter()
                        .filter_map(|pivot| sanitize_label(&pivot.name))
                        .collect(),
                    parent_indices: hierarchy
                        .pivots
                        .iter()
                        .map(|pivot| pivot.parent_idx)
                        .collect(),
                },
            );
        }

        for animation in model.animations {
            let Some(name) = sanitize_label(&animation.header.name) else {
                continue;
            };
            snapshot.animations.insert(
                name,
                AnimationSnapshot {
                    hierarchy: animation.header.hierarchy_name.clone(),
                    num_frames: animation.header.num_frames,
                    frame_rate: animation.header.frame_rate,
                    bone_channels: animation
                        .bone_animations
                        .iter()
                        .map(|bone| AnimationBoneSnapshot {
                            has_x: bone.x_channel.is_some(),
                            has_y: bone.y_channel.is_some(),
                            has_z: bone.z_channel.is_some(),
                            has_rotation: bone.rotation_channel.is_some(),
                            has_visibility: bone.visibility_channel.is_some(),
                        })
                        .collect(),
                },
            );
        }
    }

    Ok(snapshot)
}

/// Capture a deterministic W3D byte fixture without requiring retail data on
/// disk.  The temporary file preserves the same parser path as retail assets
/// and is removed before returning (including on parse failure).
pub fn capture_snapshot_from_bytes(asset_name: &str, bytes: &[u8]) -> Result<Snapshot> {
    let stem = sanitize_label(asset_name).unwrap_or_else(|| "fixture".to_string());
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let directory =
        std::env::temp_dir().join(format!("ww3d-validation-{}-{}", std::process::id(), unique));
    std::fs::create_dir(&directory).with_context(|| {
        format!(
            "failed to create temporary W3D fixture directory {}",
            directory.display()
        )
    })?;
    let path = directory.join(format!("{stem}.w3d"));
    std::fs::write(&path, bytes)
        .with_context(|| format!("failed to write temporary W3D fixture {}", path.display()))?;
    let result = capture_snapshot(&[&path]);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir(&directory);
    result
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
        texture_order: mesh
            .textures
            .iter()
            .filter_map(|texture| fixed_name(&texture.name))
            .collect(),
        vertex_material_order: mesh
            .vertex_material_names
            .iter()
            .filter_map(|material| fixed_name(&material.material_name))
            .collect(),
        material_info: mesh
            .material_info
            .as_ref()
            .map(|info| MaterialInfoSnapshot {
                pass_count: info.pass_count,
                vertex_material_count: info.vert_matl_count,
                shader_count: info.shader_count,
                texture_count: info.texture_count,
            }),
        material_passes: mesh
            .passes
            .iter()
            .map(|pass| MaterialPassSnapshot {
                vertex_material_id: pass.vm_id,
                shader_id: pass.shader_id,
                texture_count: pass.texture_count,
            })
            .collect(),
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
        placeholder_names: collection
            .placeholders
            .iter()
            .map(|placeholder| placeholder.name.clone())
            .collect(),
        transform_node_names: collection
            .transform_nodes
            .iter()
            .map(|node| node.name.clone())
            .collect(),
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
                sub_objects: lod_entry
                    .models
                    .iter()
                    .map(|model| HLodObjectSnapshot {
                        name: model.name.clone(),
                        bone_index: model.bone_index,
                    })
                    .collect(),
            })
            .collect(),
        aggregate_chunk_count: hlod.aggregates.len(),
        proxy_objects: hlod
            .proxy_entries
            .iter()
            .map(|proxy| HLodObjectSnapshot {
                name: proxy.name.clone(),
                bone_index: proxy.bone_index,
            })
            .collect(),
    }
}

fn fixed_name(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    sanitize_label(std::str::from_utf8(&bytes[..end]).ok()?)
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

/// Resolve a retail asset only when the caller explicitly provisions one.
///
/// The repository intentionally contains no retail game data.  Validation
/// must not silently substitute a fixture (or silently pass) when a retail
/// comparison is requested, so a missing `WW3D_RETAIL_ASSET_ROOT` is an
/// actionable error.
pub fn provisioned_retail_asset(relative_path: impl AsRef<Path>) -> Result<std::path::PathBuf> {
    let root = std::env::var_os("WW3D_RETAIL_ASSET_ROOT").ok_or_else(|| {
        anyhow::anyhow!(
            "WW3D_RETAIL_ASSET_ROOT is not set; provision extracted retail W3D data before running retail validation"
        )
    })?;
    let root = std::path::PathBuf::from(root);
    if !root.is_dir() {
        anyhow::bail!(
            "WW3D_RETAIL_ASSET_ROOT is not a directory: {}",
            root.display()
        );
    }
    let path = root.join(relative_path.as_ref());
    if !path.is_file() {
        anyhow::bail!(
            "provisioned retail asset does not exist: {}",
            path.display()
        );
    }
    Ok(path)
}

/// Load a provisioned retail asset through the same snapshot path as fixtures.
/// Callers should compare the returned snapshot against a separately
/// provisioned retail baseline; no repository fixture is used as a fallback.
pub fn capture_provisioned_retail_snapshot(relative_path: impl AsRef<Path>) -> Result<Snapshot> {
    let path = provisioned_retail_asset(relative_path)?;
    capture_snapshot(&[path])
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

                diff_ordered(
                    &format!("asset {} prototype names", asset),
                    &expected_snapshot.prototype_names,
                    &actual_snapshot.prototype_names,
                    &mut diffs,
                );
                diff_ordered(
                    &format!("asset {} texture order", asset),
                    &expected_snapshot.texture_order,
                    &actual_snapshot.texture_order,
                    &mut diffs,
                );

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

                for mesh_name in actual_snapshot.meshes.keys() {
                    if !expected_snapshot.meshes.contains_key(mesh_name) {
                        diffs.push(format!(
                            "asset {} unexpected mesh {} in actual snapshot",
                            asset, mesh_name
                        ));
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
                diff_ordered(
                    &format!("collection {} object names", collection),
                    &expected_collection.object_names,
                    &actual_collection.object_names,
                    &mut diffs,
                );
                diff_ordered(
                    &format!("collection {} placeholder names", collection),
                    &expected_collection.placeholder_names,
                    &actual_collection.placeholder_names,
                    &mut diffs,
                );
                diff_ordered(
                    &format!("collection {} transform node names", collection),
                    &expected_collection.transform_node_names,
                    &actual_collection.transform_node_names,
                    &mut diffs,
                );
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
                diff_ordered(
                    &format!("hlod {} proxy objects", name),
                    &expected_hlod.proxy_objects,
                    &actual_hlod.proxy_objects,
                    &mut diffs,
                );
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
                    diff_ordered(
                        &format!("hlod {} layer {} sub-objects", name, idx),
                        &expected_layer.sub_objects,
                        &actual_layer.sub_objects,
                        &mut diffs,
                    );
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

    diff_named_maps(
        "hierarchy",
        &expected.hierarchies,
        &actual.hierarchies,
        &mut diffs,
    );
    diff_named_maps(
        "animation",
        &expected.animations,
        &actual.animations,
        &mut diffs,
    );

    diffs
}

fn diff_ordered<T: std::fmt::Debug + PartialEq>(
    label: &str,
    expected: &[T],
    actual: &[T],
    diffs: &mut Vec<String>,
) {
    if expected != actual {
        diffs.push(format!(
            "{} mismatch: expected {:?}, got {:?}",
            label, expected, actual
        ));
    }
}

fn diff_named_maps<T: std::fmt::Debug + PartialEq>(
    kind: &str,
    expected: &BTreeMap<String, T>,
    actual: &BTreeMap<String, T>,
    diffs: &mut Vec<String>,
) {
    for name in expected.keys() {
        match actual.get(name) {
            Some(actual_value) if actual_value == &expected[name] => {}
            Some(actual_value) => diffs.push(format!(
                "{} {} mismatch: expected {:?}, got {:?}",
                kind, name, expected[name], actual_value
            )),
            None => diffs.push(format!("missing {} {} in actual snapshot", kind, name)),
        }
    }
    for name in actual.keys() {
        if !expected.contains_key(name) {
            diffs.push(format!(
                "unexpected {} {} present in actual snapshot",
                kind, name
            ));
        }
    }
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
    diff_ordered(
        &format!("asset {} mesh {} texture order", asset, mesh_name),
        &expected.texture_order,
        &actual.texture_order,
        diffs,
    );
    diff_ordered(
        &format!("asset {} mesh {} vertex material order", asset, mesh_name),
        &expected.vertex_material_order,
        &actual.vertex_material_order,
        diffs,
    );
    if expected.material_info != actual.material_info {
        diffs.push(format!(
            "asset {} mesh {} material info mismatch: expected {:?}, got {:?}",
            asset, mesh_name, expected.material_info, actual.material_info
        ));
    }
    diff_ordered(
        &format!("asset {} mesh {} material pass order", asset, mesh_name),
        &expected.material_passes,
        &actual.material_passes,
        diffs,
    );
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

    #[test]
    fn diff_rejects_unexpected_mesh_and_object_names() {
        let mut expected = Snapshot::new();
        expected.assets.insert(
            "fixture".to_string(),
            AssetSnapshot {
                prototype_names: vec!["MeshA".to_string()],
                meshes: BTreeMap::from([(
                    "MeshA".to_string(),
                    MeshSnapshot {
                        vertex_count: 1,
                        triangle_count: 1,
                        bounding_sphere_radius: 1.0,
                        bounding_sphere_center: [0.0; 3],
                        aabb_min: [-1.0; 3],
                        aabb_max: [1.0; 3],
                        texture_order: vec!["base.tga".to_string()],
                        vertex_material_order: vec!["base".to_string()],
                        material_info: None,
                        material_passes: Vec::new(),
                    },
                )]),
                total_vertices: 1,
                total_triangles: 1,
                texture_order: vec!["base.tga".to_string()],
            },
        );
        let mut actual = expected.clone();
        let unexpected_mesh = actual.assets["fixture"].meshes["MeshA"].clone();
        actual
            .assets
            .get_mut("fixture")
            .unwrap()
            .prototype_names
            .push("Unexpected".into());
        actual
            .assets
            .get_mut("fixture")
            .unwrap()
            .meshes
            .insert("Unexpected".into(), unexpected_mesh);
        let diffs = diff_snapshots(&expected, &actual);
        assert!(diffs
            .iter()
            .any(|diff| diff.contains("unexpected mesh Unexpected")));
        assert!(diffs
            .iter()
            .any(|diff| diff.contains("prototype names mismatch")));
    }

    #[test]
    fn retail_resolution_is_fail_closed_without_provisioning() {
        if std::env::var_os("WW3D_RETAIL_ASSET_ROOT").is_none() {
            let error = provisioned_retail_asset("W3D/CBChalet3.w3d").unwrap_err();
            assert!(error.to_string().contains("WW3D_RETAIL_ASSET_ROOT"));
        }
    }

    #[test]
    #[ignore = "requires provisioned retail W3D data and is intentionally fail-closed"]
    fn provisioned_retail_snapshot_is_explicit() -> Result<()> {
        let snapshot = capture_provisioned_retail_snapshot("W3D/CBChalet3.w3d")?;
        assert!(
            !snapshot.assets.is_empty(),
            "retail snapshot must be nonempty"
        );
        Ok(())
    }
}

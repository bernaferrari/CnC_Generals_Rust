use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use ww3d_assets::assets::AssetManager;
use ww3d_assets::prototypes::{AnimationPrototype, MeshPrototype};

#[derive(Debug, Serialize)]
struct ProbeEntry {
    prefix: String,
    hierarchy: String,
    hint: String,
    selected: Option<String>,
    candidates: Vec<String>,
}

fn main() -> Result<()> {
    let data_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../../Tools/w3d_to_gltf/W3D");
    if !data_root.is_dir() {
        anyhow::bail!("W3D asset directory not found at {}", data_root.display());
    }

    let samples = [
        "AISTNG", "CIMILT1", "CIOX", "CIPOW", "CIEFMR1", "CVHRSE", "CVRKSH", "CVSCTR", "UIPART",
        "UIPART2", "UIPRTSN3", "NIAGNT", "NIOFCR",
    ];

    let mut results = Vec::new();

    for prefix in samples {
        let mut manager = AssetManager::new();
        load_prefix_assets(&mut manager, &data_root, prefix)?;

        let mesh_info: Vec<(String, Option<String>)> = manager
            .prototypes()
            .filter_map(|(name, proto)| {
                proto.as_any().downcast_ref::<MeshPrototype>().map(|mesh| {
                    let container = mesh.header.as_ref().map(|hdr| hdr.container_name_str());
                    (name.clone(), container)
                })
            })
            .collect();
        let mesh_names: Vec<String> = mesh_info.iter().map(|(name, _)| name.clone()).collect();
        let hierarchy_names: Vec<String> = collect_hierarchy_names(&manager);
        println!(
            "Loaded mesh prototypes for {prefix}: {mesh_names:?} | hierarchies: {hierarchy_names:?}"
        );
        for (name, container) in &mesh_info {
            println!("    mesh {name} container {:?}", container);
        }
        let anim_pairs: Vec<(String, String)> = collect_animation_pairs(&manager);
        println!("Animations for {prefix}: {anim_pairs:?}");

        let hierarchy_name =
            derive_hierarchy_name(&manager, prefix).unwrap_or_else(|| prefix.to_string());

        let (mesh_name, _mesh_proto) = select_mesh_prototype(&manager, &hierarchy_name)
            .with_context(|| format!("No mesh prototype found for hierarchy {hierarchy_name}"))?;

        let hint = mesh_name.clone();
        let selected = manager
            .find_animation_for_hierarchy(&hierarchy_name, Some(&hint))
            .map(|proto| proto.name.clone());

        let mut candidates = collect_candidates(&manager, &hierarchy_name);

        candidates.sort();
        results.push(ProbeEntry {
            prefix: prefix.to_string(),
            hierarchy: hierarchy_name,
            hint,
            selected,
            candidates,
        });
    }

    let mut summary = BTreeMap::new();
    for entry in &results {
        summary.insert(
            entry.prefix.clone(),
            entry
                .selected
                .clone()
                .unwrap_or_else(|| "<none>".to_string()),
        );
    }

    println!("Animation probe summary:");
    for (prefix, selected) in &summary {
        println!("  {prefix:>8} -> {selected}");
    }

    let output_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("validation")
        .join("animation_probe_snapshot.json");

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let serialized = serde_json::to_vec_pretty(&results)?;
    let mut file = fs::File::create(&output_path)?;
    file.write_all(&serialized)?;
    println!("Wrote snapshot to {}", output_path.display());

    Ok(())
}

fn load_prefix_assets(manager: &mut AssetManager, data_root: &Path, prefix: &str) -> Result<()> {
    let mut loaded_any = false;
    for entry in fs::read_dir(data_root)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if !file_name
            .to_ascii_uppercase()
            .starts_with(&prefix.to_ascii_uppercase())
        {
            continue;
        }
        if !file_name.ends_with(".W3D") && !file_name.ends_with(".w3d") {
            continue;
        }
        let path = entry.path();
        println!("    loading {}", path.display());
        manager
            .load_w3d(&path)
            .with_context(|| format!("failed to load {}", path.display()))?;
        loaded_any = true;
    }

    if !loaded_any {
        anyhow::bail!(
            "No W3D files with prefix {prefix} found under {}",
            data_root.display()
        );
    }
    Ok(())
}

fn collect_candidates(manager: &AssetManager, hierarchy: &str) -> Vec<String> {
    manager
        .prototypes()
        .filter_map(|(_, proto)| proto.as_any().downcast_ref::<AnimationPrototype>())
        .filter(|anim| anim.hierarchy_name.eq_ignore_ascii_case(hierarchy))
        .map(|anim| anim.name.clone())
        .collect()
}

fn select_mesh_prototype<'a>(
    manager: &'a AssetManager,
    hierarchy_name: &str,
) -> Option<(String, &'a MeshPrototype)> {
    let target_upper = hierarchy_name.to_ascii_uppercase();
    let base = hierarchy_base(&target_upper);
    let mut meshes: Vec<(String, &'a MeshPrototype)> = manager
        .prototypes()
        .filter_map(|(name, proto)| {
            proto
                .as_any()
                .downcast_ref::<MeshPrototype>()
                .map(|mesh| (name.clone(), mesh))
        })
        .collect();

    meshes.sort_by(|(a, _), (b, _)| a.cmp(b));

    let skl_upper = format!("{}_SKL", base).to_ascii_uppercase();
    let skn_upper = format!("{}_SKN", base).to_ascii_uppercase();

    meshes.into_iter().find(|(_, mesh)| {
        mesh.header
            .as_ref()
            .and_then(|hdr| {
                let container = hdr.container_name_str().to_ascii_uppercase();
                let container_base = hierarchy_base(&container);
                Some(
                    container == target_upper
                        || container_base == base
                        || container == skn_upper
                        || container == skl_upper,
                )
            })
            .unwrap_or(false)
    })
}

fn hierarchy_base(value: &str) -> String {
    value
        .trim_end_matches("_SKL")
        .trim_end_matches("_SKN")
        .trim_end_matches("_SKA")
        .trim_end_matches("_SKA2")
        .trim_end_matches("_SKL2")
        .trim_end_matches("_SKIN")
        .trim_end_matches("_SKEL")
        .trim_end_matches("_HIER")
        .trim_end_matches("_ANIM")
        .trim_end_matches("_SKELTON")
        .to_string()
}

fn derive_hierarchy_name(manager: &AssetManager, prefix: &str) -> Option<String> {
    let prefix_upper = prefix.to_ascii_uppercase();
    let target_exact = prefix_upper.clone();
    let target_skl = format!("{}_SKL", target_exact);

    let mut anim_candidates: Vec<String> = collect_animation_pairs(manager)
        .into_iter()
        .filter(|(name, _)| name.to_ascii_uppercase().starts_with(&prefix_upper))
        .map(|(_, hierarchy)| hierarchy)
        .collect();
    anim_candidates.sort();
    anim_candidates.dedup();
    if let Some(exact) = anim_candidates.iter().find(|name| {
        let upper = name.to_ascii_uppercase();
        upper == target_exact || upper == target_skl
    }) {
        return Some(exact.clone());
    }
    if let Some(candidate) = anim_candidates.into_iter().next() {
        return Some(candidate);
    }

    let mut hierarchy_candidates: Vec<String> = collect_hierarchy_names(manager)
        .into_iter()
        .filter(|name| name.to_ascii_uppercase().starts_with(&prefix_upper))
        .collect();
    hierarchy_candidates.sort();
    hierarchy_candidates.dedup();
    if let Some(exact) = hierarchy_candidates.iter().find(|name| {
        let upper = name.to_ascii_uppercase();
        upper == target_exact || upper == target_skl
    }) {
        return Some(exact.clone());
    }
    if let Some(candidate) = hierarchy_candidates.into_iter().next() {
        let candidate_upper = candidate.to_ascii_uppercase();
        let base = hierarchy_base(&candidate_upper);
        let skl = format!("{}_SKL", base);
        return Some(if candidate_upper.ends_with("_SKL") {
            candidate
        } else if collect_candidates(manager, &skl).is_empty() {
            candidate
        } else {
            skl
        });
    }

    let mut mesh_containers: Vec<String> = manager
        .prototypes()
        .filter_map(|(_, proto)| {
            proto
                .as_any()
                .downcast_ref::<MeshPrototype>()
                .and_then(|mesh| mesh.header.as_ref().map(|hdr| hdr.container_name_str()))
        })
        .collect();
    mesh_containers.sort();
    mesh_containers.dedup();
    if let Some(exact) = mesh_containers.iter().find(|name| {
        let upper = name.to_ascii_uppercase();
        upper == target_exact || upper == target_skl
    }) {
        return Some(exact.clone());
    }
    if let Some(container) = mesh_containers
        .into_iter()
        .find(|name| name.to_ascii_uppercase().starts_with(&prefix_upper))
    {
        let base_upper = hierarchy_base(&container.to_ascii_uppercase());
        let skl = format!("{}_SKL", base_upper);
        return Some(skl);
    }

    None
}

fn collect_animation_pairs(manager: &AssetManager) -> Vec<(String, String)> {
    manager
        .prototypes()
        .filter_map(|(name, proto)| {
            proto
                .as_any()
                .downcast_ref::<AnimationPrototype>()
                .map(|anim| (name.clone(), anim.hierarchy_name.clone()))
        })
        .collect()
}

fn collect_hierarchy_names(manager: &AssetManager) -> Vec<String> {
    manager
        .prototypes()
        .filter_map(|(_, proto)| {
            proto
                .as_any()
                .downcast_ref::<ww3d_assets::prototypes::HierarchyPrototype>()
                .map(|hier| hier.name.clone())
        })
        .collect()
}

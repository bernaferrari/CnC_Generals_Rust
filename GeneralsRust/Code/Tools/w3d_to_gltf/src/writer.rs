//! glTF 2.0 writer for W3D data

use serde_json::Value as JsonValue;
use serde_json::json;

use crate::w3d::structs::*;
use crate::writer_buffers::*;
use crate::writer_materials::{build_materials_for_mesh, push_material_for_mesh};
use anyhow::Result;

// material utilities are in writer_materials.rs

pub fn convert_to_gltf(file: &W3dFile) -> Result<(JsonValue, Vec<u8>)> {
    // We'll assemble a minimal valid glTF JSON ourselves for compatibility
    let mut root = json!({
        "asset": { "version": "2.0", "generator": "W3D->glTF (GeneralsRust)" },
        "scenes": [ { "nodes": [] } ],
        "scene": 0,
        "nodes": [],
        "meshes": [],
        "buffers": [ { "byteLength": 0 } ],
        "bufferViews": [],
        "accessors": [],
        "materials": [],
        "images": [],
        "textures": [],
        "samplers": [],
        "skins": [],
        "animations": []
    });

    let mut scene_nodes: Vec<usize> = Vec::new();
    let mut bin: Vec<u8> = Vec::new();
    let buffer_index = 0usize;

    // Convert hierarchy into nodes first (optional) and prepare skin joints
    let mut joint_node_indices: Option<Vec<usize>> = None;
    let mut skeleton_root_index: Option<usize> = None;
    let hierarchy_ref = file.hierarchies.first();
    if let Some(h) = hierarchy_ref {
        let mut node_indices: Vec<usize> = Vec::with_capacity(h.pivots.len());
        for p in &h.pivots {
            let node_index = push_node_json(
                &mut root,
                json!({
                    "name": p.name(),
                    "translation": [p.translation.x, p.translation.y, p.translation.z],
                    "rotation": [p.rotation.q[0], p.rotation.q[1], p.rotation.q[2], p.rotation.q[3]]
                }),
            );
            node_indices.push(node_index);
        }
        for (i, p) in h.pivots.iter().enumerate() {
            if p.parent_idx != 0xFFFF_FFFF {
                let parent = p.parent_idx as usize;
                ensure_children_vec_json(&mut root, parent);
                if let Some(nodes) = root.get_mut("nodes").and_then(|v| v.as_array_mut()) {
                    if let Some(parent_obj) = nodes.get_mut(parent).and_then(|v| v.as_object_mut())
                    {
                        let arr = parent_obj
                            .get_mut("children")
                            .and_then(|v| v.as_array_mut())
                            .unwrap();
                        arr.push(json!(node_indices[i]));
                    }
                }
            } else {
                scene_nodes.push(node_indices[i]);
                if skeleton_root_index.is_none() {
                    skeleton_root_index = Some(node_indices[i]);
                }
            }
        }
        joint_node_indices = Some(node_indices);
    }

    // Prepare skin (inverse bind matrices) only if there is a hierarchy and at least one mesh has valid per-vertex influences
    let mut skin_index_opt: Option<usize> = None;
    let uses_skin = file
        .meshes
        .iter()
        .any(|m| !m.vertex_influences.is_empty() && m.vertex_influences.len() == m.vertices.len());
    if uses_skin {
        if let (Some(h), Some(joints)) = (hierarchy_ref, &joint_node_indices) {
            // Build inverse bind matrices once
            let globals = compute_global_transforms(h);
            let ibms: Vec<[f32; 16]> = globals.iter().map(|m| invert_rt_mat4(m)).collect();
            // write IBMs to buffer
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let ibm_offset = bin.len();
            for m in &ibms {
                for f in m {
                    bin.extend_from_slice(&f.to_le_bytes());
                }
            }
            let ibm_view = push_view_json(
                &mut root,
                buffer_index,
                ibm_offset as u32,
                (bin.len() - ibm_offset) as u32,
            );
            let ibm_acc = push_accessor_mat4_f32_json(&mut root, ibm_view, ibms.len() as u32);
            // create skin referencing all joints
            let skins = root
                .get_mut("skins")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let skin_idx = skins.len();
            skins.push(json!({
                "inverseBindMatrices": ibm_acc,
                "joints": joints,
                "skeleton": skeleton_root_index.unwrap_or(0)
            }));
            skin_index_opt = Some(skin_idx);
        }
    }

    // Export lights (KHR_lights_punctual) if present
    if !file.lights.is_empty() {
        // Ensure extensionsUsed contains KHR_lights_punctual
        {
            let eu = root
                .get_mut("extensionsUsed")
                .and_then(|v| v.as_array_mut());
            if let Some(arr) = eu {
                if !arr
                    .iter()
                    .any(|e| e.as_str() == Some("KHR_lights_punctual"))
                {
                    arr.push(json!("KHR_lights_punctual"));
                }
            } else {
                root.as_object_mut()
                    .unwrap()
                    .insert("extensionsUsed".into(), json!(["KHR_lights_punctual"]));
            }
        }
        // Ensure root extensions object
        if root.get("extensions").is_none() {
            root.as_object_mut()
                .unwrap()
                .insert("extensions".into(), json!({}));
        }
        // Create lights array
        let mut light_indices: Vec<usize> = Vec::with_capacity(file.lights.len());
        {
            let extensions = root
                .get_mut("extensions")
                .and_then(|v| v.as_object_mut())
                .unwrap();
            if extensions.get("KHR_lights_punctual").is_none() {
                extensions.insert("KHR_lights_punctual".into(), json!({"lights":[]}));
            }
            let khr = extensions
                .get_mut("KHR_lights_punctual")
                .and_then(|v| v.as_object_mut())
                .unwrap();
            let lights_arr = khr
                .get_mut("lights")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            for l in &file.lights {
                let mut obj = json!({
                    "type": match l.kind { W3dLightKind::Point => "point", W3dLightKind::Directional => "directional", W3dLightKind::Spot => "spot" },
                    "color": [l.color.r as f32/255.0, l.color.g as f32/255.0, l.color.b as f32/255.0],
                    "intensity": l.intensity as f32
                });
                if let Some(r) = l.range {
                    obj.as_object_mut()
                        .unwrap()
                        .insert("range".into(), json!(r));
                }
                if let W3dLightKind::Spot = l.kind {
                    if let Some(ang) = l.spot_angle {
                        obj.as_object_mut().unwrap().insert(
                            "spot".into(),
                            json!({"outerConeAngle": (ang.to_radians()) }),
                        );
                    }
                }
                let idx = lights_arr.len();
                lights_arr.push(obj);
                light_indices.push(idx);
            }
        }
        // Create a node per light and attach extension reference
        for (i, l) in file.lights.iter().enumerate() {
            let mut node = json!({ "name": format!("Light_{}", i) });
            if let Some(p) = l.position {
                node.as_object_mut()
                    .unwrap()
                    .insert("translation".into(), json!([p.x, p.y, p.z]));
            }
            if let Some(d) = l.direction {
                let q = quat_from_to([0.0, 0.0, -1.0], [d.x, d.y, d.z]);
                node.as_object_mut()
                    .unwrap()
                    .insert("rotation".into(), json!([q[0], q[1], q[2], q[3]]));
            }
            // Add node extension reference
            node.as_object_mut().unwrap().insert(
                "extensions".into(),
                json!({"KHR_lights_punctual": {"light": light_indices[i]}}),
            );
            let ni = push_node_json(&mut root, node);
            scene_nodes.push(ni);
        }
    }

    // Convert meshes with better validation and error handling
    for mesh in &file.meshes {
        // Skip meshes without vertices or triangles
        if mesh.vertices.is_empty() {
            log::warn!("Skipping mesh '{}' - no vertices", mesh.mesh_name());
            continue;
        }
        if mesh.triangles.is_empty() {
            log::warn!("Skipping mesh '{}' - no triangles", mesh.mesh_name());
            continue;
        }

        // Align buffer to 4-byte boundary before starting mesh data
        while bin.len() % 4 != 0 {
            bin.push(0);
        }

        let positions_offset = bin.len();
        for v in &mesh.vertices {
            bin.extend_from_slice(&v.x.to_le_bytes());
            bin.extend_from_slice(&v.y.to_le_bytes());
            bin.extend_from_slice(&v.z.to_le_bytes());
        }
        while bin.len() % 4 != 0 {
            bin.push(0);
        }

        // Normals: prefer source if valid; otherwise compute from geometry. Always normalized.
        let mut norm_view_opt: Option<usize> = None;
        let normals_to_use: Vec<[f32; 3]> = {
            let mut valid = mesh.normals.len() == mesh.vertices.len() && !mesh.normals.is_empty();
            if valid {
                // verify all finite and non-zero; if many invalid, recompute
                let mut bad = 0usize;
                for n in &mesh.normals {
                    let len2 = n.x * n.x + n.y * n.y + n.z * n.z;
                    if !len2.is_finite() || len2 <= 1.0e-10 {
                        bad += 1;
                    }
                }
                if bad > 0 {
                    valid = false;
                }
            }
            if valid {
                mesh.normals
                    .iter()
                    .map(|n| {
                        let mut x = n.x;
                        let mut y = n.y;
                        let mut z = n.z;
                        let l2 = x * x + y * y + z * z;
                        if l2 > 1.0e-10 && l2.is_finite() {
                            let inv = 1.0 / l2.sqrt();
                            x *= inv;
                            y *= inv;
                            z *= inv;
                        } else {
                            x = 0.0;
                            y = 0.0;
                            z = 1.0;
                        }
                        [x, y, z]
                    })
                    .collect()
            } else {
                compute_vertex_normals(&mesh.vertices, &mesh.triangles)
            }
        };
        if !normals_to_use.is_empty() {
            let normals_offset = bin.len();
            for n in &normals_to_use {
                bin.extend_from_slice(&n[0].to_le_bytes());
                bin.extend_from_slice(&n[1].to_le_bytes());
                bin.extend_from_slice(&n[2].to_le_bytes());
            }
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let norm_view_len = (normals_to_use.len() * 3 * 4) as u32;
            let norm_view = push_view_json_with_target(
                &mut root,
                buffer_index,
                normals_offset as u32,
                norm_view_len,
                34962,
            );
            norm_view_opt = Some(norm_view);
        }

        // Take stage 0 UVs if present, with V-flip for OpenGL compatibility
        let uvs: Vec<W3dTexCoord> = mesh
            .material_passes
            .get(0)
            .and_then(|p| p.texture_stages.get(0))
            .map(|st| st.tex_coords.clone())
            .unwrap_or_default();
        let mut uv_view_opt: Option<usize> = None;
        // Only emit TEXCOORD_0 when it matches vertex count
        if !uvs.is_empty() && uvs.len() == mesh.vertices.len() {
            let uvs_offset = bin.len();
            for uv in &uvs {
                bin.extend_from_slice(&uv.u.to_le_bytes());
                bin.extend_from_slice(&(1.0 - uv.v).to_le_bytes());
            }
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let uv_view_len = (uvs.len() * 2 * 4) as u32;
            let uv_view = push_view_json_with_target(
                &mut root,
                buffer_index,
                uvs_offset as u32,
                uv_view_len,
                34962,
            );
            uv_view_opt = Some(uv_view);
        }

        // Choose index component type based on max index
        let mut max_index: u32 = 0;
        for t in &mesh.triangles {
            for idx in t.v_indices {
                if idx > max_index {
                    max_index = idx;
                }
            }
        }
        let use_u16 = max_index <= u16::MAX as u32;
        // Build buffer views
        let pos_view_len = (mesh.vertices.len() * 3 * 4) as u32;
        let pos_view = push_view_json_with_target(
            &mut root,
            buffer_index,
            positions_offset as u32,
            pos_view_len,
            34962,
        );

        // Accessors
        let pos_acc = push_accessor_vec3_json_with_bounds(
            &mut root,
            pos_view,
            mesh.vertices.len() as u32,
            compute_min_max_vec3(&mesh.vertices),
        );
        let mut norm_acc_opt: Option<usize> = None;
        if let Some(norm_view) = norm_view_opt {
            norm_acc_opt = Some(push_accessor_vec3_json(
                &mut root,
                norm_view,
                mesh.normals.len() as u32,
            ));
        }
        let mut uv_acc_opt: Option<usize> = None;
        if let Some(uv_view) = uv_view_opt {
            uv_acc_opt = Some(push_accessor_vec2_json(
                &mut root,
                uv_view,
                uvs.len() as u32,
            ));
        }

        let mut attributes = serde_json::Map::new();
        attributes.insert("POSITION".into(), json!(pos_acc));
        if let Some(norm_acc) = norm_acc_opt {
            attributes.insert("NORMAL".into(), json!(norm_acc));
        }
        if let Some(uv_acc) = uv_acc_opt {
            attributes.insert("TEXCOORD_0".into(), json!(uv_acc));
        }
        // Add additional TEXCOORD_n (stages) if exist
        if let Some(pass0) = mesh.material_passes.get(0) {
            for (stage_idx, stage) in pass0.texture_stages.iter().enumerate().skip(1) {
                if stage.tex_coords.len() == mesh.vertices.len() {
                    while bin.len() % 4 != 0 {
                        bin.push(0);
                    }
                    let off = bin.len();
                    for uv in &stage.tex_coords {
                        bin.extend_from_slice(&uv.u.to_le_bytes());
                        bin.extend_from_slice(&(1.0 - uv.v).to_le_bytes());
                    }
                    let view = push_view_json_with_target(
                        &mut root,
                        buffer_index,
                        off as u32,
                        (bin.len() - off) as u32,
                        34962,
                    );
                    let acc =
                        push_accessor_vec2_json(&mut root, view, stage.tex_coords.len() as u32);
                    attributes.insert(format!("TEXCOORD_{}", stage_idx), json!(acc));
                }
            }
        }
        // Add vertex colors as COLOR_0 if available
        if !mesh.vertex_colors.is_empty() && mesh.vertex_colors.len() == mesh.vertices.len() {
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let col_off = bin.len();
            for c in &mesh.vertex_colors {
                let rf = c.r as f32 / 255.0;
                let gf = c.g as f32 / 255.0;
                let bf = c.b as f32 / 255.0;
                bin.extend_from_slice(&rf.to_le_bytes());
                bin.extend_from_slice(&gf.to_le_bytes());
                bin.extend_from_slice(&bf.to_le_bytes());
            }
            let col_view = push_view_json_with_target(
                &mut root,
                buffer_index,
                col_off as u32,
                (bin.len() - col_off) as u32,
                34962,
            );
            let col_acc =
                push_accessor_vec3_f32_json(&mut root, col_view, mesh.vertex_colors.len() as u32);
            attributes.insert("COLOR_0".into(), json!(col_acc));
        }
        // Compute tangents if normals+uvs present and a normal map is likely used (bump flag)
        let has_bump = mesh
            .textures
            .iter()
            .any(|t| (t.info.attributes & crate::writer_materials::W3DTEXTURE_TYPE_BUMPMAP) != 0);
        if has_bump && !uvs.is_empty() {
            let tangents = compute_tangents_basic(&mesh.vertices, &uvs, &mesh.triangles, None);
            // Validate tangents: require all finite and non-zero; otherwise omit entirely and let viewer compute
            let mut invalid = 0usize;
            for t in &tangents {
                let l2 = t[0] * t[0] + t[1] * t[1] + t[2] * t[2];
                if !l2.is_finite() || l2 <= 1.0e-10 {
                    invalid += 1;
                }
            }
            if !tangents.is_empty() && invalid == 0 {
                while bin.len() % 4 != 0 {
                    bin.push(0);
                }
                let tan_off = bin.len();
                for t4 in &tangents {
                    for f in t4 {
                        bin.extend_from_slice(&f.to_le_bytes());
                    }
                }
                let tan_view = push_view_json_with_target(
                    &mut root,
                    buffer_index,
                    tan_off as u32,
                    (bin.len() - tan_off) as u32,
                    34962,
                );
                let tan_acc =
                    push_accessor_vec4_f32_json(&mut root, tan_view, tangents.len() as u32);
                attributes.insert("TANGENT".into(), json!(tan_acc));
            } else {
                log::info!(
                    "Omitting TANGENT (not robust) for mesh '{}'",
                    mesh.mesh_name()
                );
            }
        }
        // Basic skinning: if influences present, add JOINTS_0 (VEC4 u16) and WEIGHTS_0 (VEC4 f32)
        if !mesh.vertex_influences.is_empty() && mesh.vertex_influences.len() == mesh.vertices.len()
        {
            // Align
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let joints_view_off = bin.len();
            for inf in &mesh.vertex_influences {
                let j0: u16 = inf.bone_idx as u16;
                let j = [j0, 0u16, 0u16, 0u16];
                for comp in &j {
                    bin.extend_from_slice(&comp.to_le_bytes());
                }
            }
            let joints_view = push_view_json_with_target(
                &mut root,
                buffer_index,
                joints_view_off as u32,
                (bin.len() - joints_view_off) as u32,
                34962,
            );
            let joints_acc = push_accessor_vec4_u16_json(
                &mut root,
                joints_view,
                mesh.vertex_influences.len() as u32,
            );
            attributes.insert("JOINTS_0".into(), json!(joints_acc));

            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let weights_view_off = bin.len();
            for _ in &mesh.vertex_influences {
                let w = [1.0f32, 0.0f32, 0.0f32, 0.0f32];
                for comp in &w {
                    bin.extend_from_slice(&comp.to_le_bytes());
                }
            }
            let weights_view = push_view_json_with_target(
                &mut root,
                buffer_index,
                weights_view_off as u32,
                (bin.len() - weights_view_off) as u32,
                34962,
            );
            let weights_acc = push_accessor_vec4_f32_json(
                &mut root,
                weights_view,
                mesh.vertex_influences.len() as u32,
            );
            attributes.insert("WEIGHTS_0".into(), json!(weights_acc));
        }
        // If morph targets exist, write POSITION deltas to targets and create weights
        let mut targets_array: Vec<JsonValue> = Vec::new();
        // Keep morph weights separate from attribute map to avoid non-standard semantics
        let mut morph_weights_for_primitive: Option<JsonValue> = None;
        if !mesh.morph_targets.is_empty() {
            // create buffer view for each target's position deltas
            let mut target_weights: Vec<f32> = Vec::with_capacity(mesh.morph_targets.len());
            let mut target_names: Vec<String> = Vec::with_capacity(mesh.morph_targets.len());
            for mt in &mesh.morph_targets {
                // Align
                while bin.len() % 4 != 0 {
                    bin.push(0);
                }
                let off = bin.len();
                for d in &mt.deltas {
                    bin.extend_from_slice(&d[0].to_le_bytes());
                    bin.extend_from_slice(&d[1].to_le_bytes());
                    bin.extend_from_slice(&d[2].to_le_bytes());
                }
                let view = push_view_json(
                    &mut root,
                    buffer_index,
                    off as u32,
                    (bin.len() - off) as u32,
                );
                let acc = push_accessor_vec3_json(&mut root, view, mesh.vertices.len() as u32);
                let mut target_obj = serde_json::Map::new();
                target_obj.insert("POSITION".into(), json!(acc));
                targets_array.push(JsonValue::Object(target_obj));
                target_weights.push(mt.weight);
                target_names.push(mt.name.clone());
            }
            // Store weights for the mesh primitive (standard field on primitive)
            morph_weights_for_primitive = Some(json!(target_weights));
            // target_names are kept in extras (non-standard) on the mesh, if desired
        }

        // Build primitives. If per-triangle material indices exist, split primitives by material.
        let mut primitives: Vec<JsonValue> = Vec::new();
        if let Some(per_tri) = &mesh.per_tri_materials {
            // Prepare material indices in glTF for all W3D materials
            let gltf_mats = build_materials_for_mesh(&mut root, mesh);
            use std::collections::BTreeMap;
            let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
            for (ti, &mat_id) in per_tri.iter().enumerate() {
                let tri = &mesh.triangles[ti];
                groups
                    .entry(mat_id)
                    .or_default()
                    .extend_from_slice(&tri.v_indices);
            }
            for (mat_id, idxs) in groups {
                // write index data for this group
                while bin.len() % 4 != 0 {
                    bin.push(0);
                }
                let off = bin.len();
                if use_u16 {
                    for idx in &idxs {
                        bin.extend_from_slice(&(*idx as u16).to_le_bytes());
                    }
                } else {
                    for idx in &idxs {
                        bin.extend_from_slice(&idx.to_le_bytes());
                    }
                }
                let view = push_view_json_with_target(
                    &mut root,
                    buffer_index,
                    off as u32,
                    (bin.len() - off) as u32,
                    34963,
                );
                let ind_acc = if use_u16 {
                    push_accessor_scalar_u16_json(&mut root, view, idxs.len() as u32)
                } else {
                    push_accessor_scalar_u32_json(&mut root, view, idxs.len() as u32)
                };
                let material_index = gltf_mats
                    .get(mat_id as usize)
                    .copied()
                    .unwrap_or_else(|| push_material_for_mesh(&mut root, mesh));
                if idxs.is_empty() {
                    continue;
                } // skip zero-triangle primitive
                let mut prim = json!({ "attributes": attributes, "indices": ind_acc, "mode": 4, "material": material_index });
                if let Some(w) = &morph_weights_for_primitive {
                    if let Some(obj) = prim.as_object_mut() {
                        obj.insert("weights".into(), w.clone());
                    }
                }
                if !targets_array.is_empty() {
                    if let Some(obj) = prim.as_object_mut() {
                        obj.insert("targets".into(), JsonValue::Array(targets_array.clone()));
                    }
                }
                primitives.push(prim);
            }
        } else {
            // Single primitive path
            // Write combined indices once
            let indices_offset = bin.len();
            if use_u16 {
                for t in &mesh.triangles {
                    for idx in t.v_indices {
                        bin.extend_from_slice((idx as u16).to_le_bytes().as_slice());
                    }
                }
            } else {
                for t in &mesh.triangles {
                    for idx in t.v_indices {
                        bin.extend_from_slice((idx as u32).to_le_bytes().as_slice());
                    }
                }
            }
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let ind_view = push_view_json_with_target(
                &mut root,
                buffer_index,
                indices_offset as u32,
                (bin.len() - indices_offset) as u32,
                34963,
            );
            let ind_acc = if use_u16 {
                push_accessor_scalar_u16_json(
                    &mut root,
                    ind_view,
                    (mesh.triangles.len() * 3) as u32,
                )
            } else {
                push_accessor_scalar_u32_json(
                    &mut root,
                    ind_view,
                    (mesh.triangles.len() * 3) as u32,
                )
            };
            let mat_index = push_material_for_mesh(&mut root, mesh);
            let mut primitive = json!({ "attributes": attributes, "indices": ind_acc, "mode": 4, "material": mat_index });
            if let Some(w) = &morph_weights_for_primitive {
                if let Some(obj) = primitive.as_object_mut() {
                    obj.insert("weights".into(), w.clone());
                }
            }
            if !targets_array.is_empty() {
                if let Some(obj) = primitive.as_object_mut() {
                    obj.insert("targets".into(), JsonValue::Array(targets_array));
                }
            }
            primitives.push(primitive);
        }

        // Build mesh object, including AABTree extras if present
        let mut mesh_obj = json!({ "primitives": primitives });
        if let Some(tree) = &mesh.aabtree {
            let nodes: Vec<JsonValue> = tree
                .nodes
                .iter()
                .map(|n| {
                    json!({
                        "min": [n.min.x, n.min.y, n.min.z],
                        "max": [n.max.x, n.max.y, n.max.z],
                        "frontOrPoly0": n.front_or_poly0,
                        "backOrPolyCount": n.back_or_poly_count
                    })
                })
                .collect();
            let extras = json!({"w3dAABTree": {"polyIndices": tree.poly_indices, "nodes": nodes}});
            if let Some(obj) = mesh_obj.as_object_mut() {
                obj.insert("extras".into(), extras);
            }
        }
        let mesh_idx = push_mesh_json(&mut root, mesh_obj);

        // attach to a node, link skin if present and mesh has influences
        let mut node_obj = json!({ "name": mesh.container_name(), "mesh": mesh_idx });
        if skin_index_opt.is_some() && !mesh.vertex_influences.is_empty() {
            if let Some(obj) = node_obj.as_object_mut() {
                obj.insert("skin".into(), json!(skin_index_opt.unwrap()));
            }
        }
        let node_idx = push_node_json(&mut root, node_obj);
        scene_nodes.push(node_idx);
    }

    // scene
    if let Some(scene0) = root
        .get_mut("scenes")
        .and_then(|v| v.as_array_mut())
        .and_then(|a| a.get_mut(0))
    {
        if let Some(obj) = scene0.as_object_mut() {
            obj.insert(
                "nodes".to_string(),
                JsonValue::Array(scene_nodes.into_iter().map(|i| json!(i)).collect()),
            );
        }
    }
    // Export animations if present
    export_animations(
        &file,
        &mut root,
        &mut bin,
        buffer_index,
        &joint_node_indices,
        hierarchy_ref,
    );
    // Final safety pass: ensure NORMAL accessors are unit-length (repair in-place)
    ensure_unit_normals(&mut root, &mut bin);
    // Remove empty animations array if none were added (some validators dislike empty arrays)
    if let Some(anims) = root.get("animations").and_then(|v| v.as_array()) {
        if anims.is_empty() {
            if let Some(obj) = root.as_object_mut() {
                obj.remove("animations");
            }
        }
    }
    // Remove empty skins array if none were added
    if let Some(skins) = root.get("skins").and_then(|v| v.as_array()) {
        if skins.is_empty() {
            if let Some(obj) = root.as_object_mut() {
                obj.remove("skins");
            }
        }
    }
    // Remove empty arrays: accessors, bufferViews, materials, images, textures, samplers, nodes, meshes
    for key in [
        "accessors",
        "bufferViews",
        "materials",
        "images",
        "textures",
        "samplers",
        "nodes",
        "meshes",
    ] {
        if let Some(arr) = root.get(key).and_then(|v| v.as_array()) {
            if arr.is_empty() {
                if let Some(obj) = root.as_object_mut() {
                    obj.remove(key);
                }
            }
        }
    }
    // If scene has empty nodes array, remove it
    if let Some(scenes) = root.get_mut("scenes").and_then(|v| v.as_array_mut()) {
        for sc in scenes.iter_mut() {
            if let Some(obj) = sc.as_object_mut() {
                if let Some(nodes) = obj.get("nodes").and_then(|v| v.as_array()) {
                    if nodes.is_empty() {
                        obj.remove("nodes");
                    }
                }
            }
        }
    }

    // Finalize buffer length (after animations may append data)
    if let Some(buffers) = root.get_mut("buffers").and_then(|v| v.as_array_mut()) {
        if bin.is_empty() {
            if let Some(obj) = root.as_object_mut() {
                obj.remove("buffers");
            }
        } else if let Some(buf0) = buffers.get_mut(0) {
            if let Some(obj) = buf0.as_object_mut() {
                obj.insert("byteLength".into(), json!(bin.len()));
            }
        }
    } else {
        // If there is no buffers array and bin is empty, nothing to do
    }

    Ok((root, bin))
}

fn ensure_unit_normals(root: &mut JsonValue, bin: &mut [u8]) {
    let meshes = match root.get("meshes").and_then(|v| v.as_array()) {
        Some(m) => m,
        None => return,
    };
    let accessors = match root.get("accessors").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return,
    };
    let views = match root.get("bufferViews").and_then(|v| v.as_array()) {
        Some(v) => v,
        None => return,
    };
    for m in meshes {
        if let Some(prims) = m.get("primitives").and_then(|v| v.as_array()) {
            for p in prims {
                let attrs = match p.get("attributes").and_then(|v| v.as_object()) {
                    Some(a) => a,
                    None => continue,
                };
                let Some(norm_acc_idx) = attrs
                    .get("NORMAL")
                    .and_then(|x| x.as_u64())
                    .map(|u| u as usize)
                else {
                    continue;
                };
                if norm_acc_idx >= accessors.len() {
                    continue;
                }
                let acc = &accessors[norm_acc_idx];
                // Only handle float32 VEC3 tightly packed
                let ctype = acc
                    .get("componentType")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0) as u32;
                let type_str = acc.get("type").and_then(|x| x.as_str()).unwrap_or("");
                if ctype != 5126 || type_str != "VEC3" {
                    continue;
                }
                let count = acc.get("count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let view_idx = acc.get("bufferView").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                if view_idx >= views.len() {
                    continue;
                }
                let view = &views[view_idx];
                let view_off =
                    view.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let acc_off = acc.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let base = view_off + acc_off;
                let stride = 12usize; // tightly packed 3*f32
                for k in 0..count {
                    let i = base + k * stride;
                    if i + 12 > bin.len() {
                        break;
                    }
                    let mut x = f32::from_le_bytes([bin[i], bin[i + 1], bin[i + 2], bin[i + 3]]);
                    let mut y =
                        f32::from_le_bytes([bin[i + 4], bin[i + 5], bin[i + 6], bin[i + 7]]);
                    let mut z =
                        f32::from_le_bytes([bin[i + 8], bin[i + 9], bin[i + 10], bin[i + 11]]);
                    let len2 = x * x + y * y + z * z;
                    if !len2.is_finite() || len2 <= 1.0e-10 {
                        x = 0.0;
                        y = 0.0;
                        z = 1.0;
                    } else {
                        let inv = 1.0 / len2.sqrt();
                        x *= inv;
                        y *= inv;
                        z *= inv;
                        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                            x = 0.0;
                            y = 0.0;
                            z = 1.0;
                        }
                    }
                    let xb = x.to_le_bytes();
                    bin[i..i + 4].copy_from_slice(&xb);
                    let yb = y.to_le_bytes();
                    bin[i + 4..i + 8].copy_from_slice(&yb);
                    let zb = z.to_le_bytes();
                    bin[i + 8..i + 12].copy_from_slice(&zb);
                }
            }
        }
    }
}

// Compute quaternion rotating vector a to b (both 3D), return [x,y,z,w]
fn quat_from_to(a: [f32; 3], b: [f32; 3]) -> [f32; 4] {
    // use std::f32::consts::PI; // not needed currently
    let ax = a[0];
    let ay = a[1];
    let az = a[2];
    let bx = b[0];
    let by = b[1];
    let bz = b[2];
    let dot = ax * bx + ay * by + az * bz;
    let v = [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx];
    let w = (1.0 + dot).max(1e-6);
    let mut q = [v[0], v[1], v[2], w];
    // normalize
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len > 0.0 {
        q = [q[0] / len, q[1] / len, q[2] / len, q[3] / len];
    }
    q
}

fn push_node_json(root: &mut JsonValue, node_obj: JsonValue) -> usize {
    let nodes = root
        .get_mut("nodes")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    nodes.push(node_obj);
    nodes.len() - 1
}

fn push_mesh_json(root: &mut JsonValue, mesh_obj: JsonValue) -> usize {
    let meshes = root
        .get_mut("meshes")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    meshes.push(mesh_obj);
    meshes.len() - 1
}

fn ensure_children_vec_json(root: &mut JsonValue, parent_index: usize) {
    if let Some(nodes) = root.get_mut("nodes").and_then(|v| v.as_array_mut()) {
        if let Some(obj) = nodes.get_mut(parent_index).and_then(|v| v.as_object_mut()) {
            if !obj.contains_key("children") {
                obj.insert("children".to_string(), JsonValue::Array(Vec::new()));
            }
        }
    }
}

fn compute_tangents(
    positions: &[W3dVector],
    uvs: &[W3dTexCoord],
    triangles: &[W3dTriangle],
    normals_opt: Option<&[W3dVector]>,
) -> Vec<[f32; 4]> {
    // Deprecated by compute_tangents_basic; retained if other code paths call it.
    let num_verts = positions.len();
    if uvs.len() != num_verts || num_verts == 0 {
        return Vec::new();
    }
    let mut tan1 = vec![[0.0f32; 3]; num_verts];
    let mut tan2 = vec![[0.0f32; 3]; num_verts];

    for tri in triangles {
        let i0 = tri.v_indices[0] as usize;
        let i1 = tri.v_indices[1] as usize;
        let i2 = tri.v_indices[2] as usize;
        if i0 >= num_verts || i1 >= num_verts || i2 >= num_verts {
            continue;
        }
        let p0 = &positions[i0];
        let p1 = &positions[i1];
        let p2 = &positions[i2];
        let w0 = &uvs[i0];
        let w1 = &uvs[i1];
        let w2 = &uvs[i2];
        let x1 = p1.x - p0.x;
        let x2 = p2.x - p0.x;
        let y1 = p1.y - p0.y;
        let y2 = p2.y - p0.y;
        let z1 = p1.z - p0.z;
        let z2 = p2.z - p0.z;
        let s1 = w1.u - w0.u;
        let s2 = w2.u - w0.u;
        let t1 = 1.0 - w1.v - (1.0 - w0.v); // account for V flip (v' = 1-v)
        let t2 = 1.0 - w2.v - (1.0 - w0.v);
        let r = 1.0 / (s1 * t2 - s2 * t1).max(1e-8).min(1e8);
        let sdir = [
            (t2 * x1 - t1 * x2) * r,
            (t2 * y1 - t1 * y2) * r,
            (t2 * z1 - t1 * z2) * r,
        ];
        let tdir = [
            (s1 * x2 - s2 * x1) * r,
            (s1 * y2 - s2 * y1) * r,
            (s1 * z2 - s2 * z1) * r,
        ];
        for &i in &[i0, i1, i2] {
            tan1[i][0] += sdir[0];
            tan1[i][1] += sdir[1];
            tan1[i][2] += sdir[2];
            tan2[i][0] += tdir[0];
            tan2[i][1] += tdir[1];
            tan2[i][2] += tdir[2];
        }
    }
    // Orthonormalize against normals when available and ensure unit length
    let mut tangents = Vec::with_capacity(num_verts);
    for i in 0..num_verts {
        let mut t = tan1[i];
        // If normals provided, project out normal component for orthogonality
        if let Some(normals) = normals_opt {
            let n = normals[i];
            // t = normalize(t - n * dot(n, t))
            let dot_nt = n.x * t[0] + n.y * t[1] + n.z * t[2];
            t[0] -= n.x * dot_nt;
            t[1] -= n.y * dot_nt;
            t[2] -= n.z * dot_nt;
        }
        let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        if len < 1e-6 {
            // Robust fallback: derive an orthonormal tangent from normal if present
            if let Some(normals) = normals_opt {
                let n = normals[i];
                // choose a helper axis least aligned with n
                let helper = if n.z.abs() < 0.9 {
                    [0.0, 0.0, 1.0]
                } else {
                    [0.0, 1.0, 0.0]
                };
                // t = normalize(cross(helper, n))
                let cx = helper[1] * n.z - helper[2] * n.y;
                let cy = helper[2] * n.x - helper[0] * n.z;
                let cz = helper[0] * n.y - helper[1] * n.x;
                let clen = (cx * cx + cy * cy + cz * cz).sqrt();
                if clen > 1e-6 {
                    tangents.push([cx / clen, cy / clen, cz / clen, 1.0]);
                    continue;
                }
            }
            // Final fallback when normals missing or degenerate
            tangents.push([1.0, 0.0, 0.0, 1.0]);
        } else {
            tangents.push([t[0] / len, t[1] / len, t[2] / len, 1.0]);
        }
    }
    tangents
}

fn compute_tangents_basic(
    positions: &[W3dVector],
    uvs: &[W3dTexCoord],
    triangles: &[W3dTriangle],
    _normals_opt: Option<&[W3dVector]>,
) -> Vec<[f32; 4]> {
    let n = positions.len();
    if n == 0 || uvs.len() != n {
        return Vec::new();
    }
    let mut tan1 = vec![[0.0f32; 3]; n];
    let mut tan2 = vec![[0.0f32; 3]; n];
    for tri in triangles {
        let i0 = tri.v_indices[0] as usize;
        let i1 = tri.v_indices[1] as usize;
        let i2 = tri.v_indices[2] as usize;
        if i0 >= n || i1 >= n || i2 >= n {
            continue;
        }
        let p0 = &positions[i0];
        let p1 = &positions[i1];
        let p2 = &positions[i2];
        let w0 = &uvs[i0];
        let w1 = &uvs[i1];
        let w2 = &uvs[i2];
        let x1 = p1.x - p0.x;
        let x2 = p2.x - p0.x;
        let y1 = p1.y - p0.y;
        let y2 = p2.y - p0.y;
        let z1 = p1.z - p0.z;
        let z2 = p2.z - p0.z;
        let s1 = w1.u - w0.u;
        let s2 = w2.u - w0.u;
        let t1 = (1.0 - w1.v) - (1.0 - w0.v); // account for V flip
        let t2 = (1.0 - w2.v) - (1.0 - w0.v);
        let denom = s1 * t2 - s2 * t1;
        if denom.abs() <= 1.0e-12 {
            continue;
        }
        let r = 1.0 / denom;
        let sdir = [
            (t2 * x1 - t1 * x2) * r,
            (t2 * y1 - t1 * y2) * r,
            (t2 * z1 - t1 * z2) * r,
        ];
        let tdir = [
            (s1 * x2 - s2 * x1) * r,
            (s1 * y2 - s2 * y1) * r,
            (s1 * z2 - s2 * z1) * r,
        ];
        for &i in &[i0, i1, i2] {
            tan1[i][0] += sdir[0];
            tan1[i][1] += sdir[1];
            tan1[i][2] += sdir[2];
            tan2[i][0] += tdir[0];
            tan2[i][1] += tdir[1];
            tan2[i][2] += tdir[2];
        }
    }
    let mut tangents = Vec::with_capacity(n);
    for i in 0..n {
        let t = tan1[i];
        let l2 = t[0] * t[0] + t[1] * t[1] + t[2] * t[2];
        if l2 <= 1.0e-10 || !l2.is_finite() {
            tangents.push([0.0, 0.0, 0.0, 1.0]);
        } else {
            let inv = 1.0 / l2.sqrt();
            tangents.push([t[0] * inv, t[1] * inv, t[2] * inv, 1.0]);
        }
    }
    tangents
}

fn compute_vertex_normals(positions: &[W3dVector], triangles: &[W3dTriangle]) -> Vec<[f32; 3]> {
    let n = positions.len();
    let mut sums = vec![[0.0f32; 3]; n];
    for tri in triangles {
        let i0 = tri.v_indices[0] as usize;
        let i1 = tri.v_indices[1] as usize;
        let i2 = tri.v_indices[2] as usize;
        if i0 >= n || i1 >= n || i2 >= n {
            continue;
        }
        let p0 = &positions[i0];
        let p1 = &positions[i1];
        let p2 = &positions[i2];
        let ux = p1.x - p0.x;
        let uy = p1.y - p0.y;
        let uz = p1.z - p0.z;
        let vx = p2.x - p0.x;
        let vy = p2.y - p0.y;
        let vz = p2.z - p0.z;
        let cx = uy * vz - uz * vy;
        let cy = uz * vx - ux * vz;
        let cz = ux * vy - uy * vx;
        let l2 = cx * cx + cy * cy + cz * cz;
        if l2 <= 1.0e-16 || !l2.is_finite() {
            continue;
        }
        // area-weighted (use cross as-is)
        sums[i0][0] += cx;
        sums[i0][1] += cy;
        sums[i0][2] += cz;
        sums[i1][0] += cx;
        sums[i1][1] += cy;
        sums[i1][2] += cz;
        sums[i2][0] += cx;
        sums[i2][1] += cy;
        sums[i2][2] += cz;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = sums[i][0];
        let y = sums[i][1];
        let z = sums[i][2];
        let l2 = x * x + y * y + z * z;
        if l2 <= 1.0e-12 || !l2.is_finite() {
            out.push([0.0, 0.0, 1.0]);
        } else {
            let inv = 1.0 / l2.sqrt();
            out.push([x * inv, y * inv, z * inv]);
        }
    }
    out
}

fn compute_global_transforms(h: &W3dHierarchy) -> Vec<[f32; 16]> {
    let n = h.pivots.len();
    let mut globals: Vec<[f32; 16]> = vec![[0.0; 16]; n];
    for i in 0..n {
        let local = mat4_from_tr_quat(&h.pivots[i]);
        let g = if h.pivots[i].parent_idx != 0xFFFF_FFFF {
            let p = h.pivots[i].parent_idx as usize;
            mat4_mul(&globals[p], &local)
        } else {
            local
        };
        globals[i] = g;
    }
    globals
}

fn mat4_from_tr_quat(p: &W3dPivot) -> [f32; 16] {
    let x = p.rotation.q[0];
    let y = p.rotation.q[1];
    let z = p.rotation.q[2];
    let w = p.rotation.q[3];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    let m00 = 1.0 - 2.0 * (yy + zz);
    let m01 = 2.0 * (xy - wz);
    let m02 = 2.0 * (xz + wy);
    let m10 = 2.0 * (xy + wz);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - wx);
    let m20 = 2.0 * (xz - wy);
    let m21 = 2.0 * (yz + wx);
    let m22 = 1.0 - 2.0 * (xx + yy);
    let tx = p.translation.x;
    let ty = p.translation.y;
    let tz = p.translation.z;
    [
        m00, m01, m02, 0.0, m10, m11, m12, 0.0, m20, m21, m22, 0.0, tx, ty, tz, 1.0,
    ]
}

fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut r = [0.0; 16];
    for i in 0..4 {
        for j in 0..4 {
            r[i * 4 + j] = a[i * 4 + 0] * b[0 * 4 + j]
                + a[i * 4 + 1] * b[1 * 4 + j]
                + a[i * 4 + 2] * b[2 * 4 + j]
                + a[i * 4 + 3] * b[3 * 4 + j];
        }
    }
    r
}

fn invert_rt_mat4(m: &[f32; 16]) -> [f32; 16] {
    // inverse of rotation-translation: R^T and -R^T t
    let r00 = m[0];
    let r01 = m[1];
    let r02 = m[2];
    let r10 = m[4];
    let r11 = m[5];
    let r12 = m[6];
    let r20 = m[8];
    let r21 = m[9];
    let r22 = m[10];
    let tx = m[12];
    let ty = m[13];
    let tz = m[14];
    let rt00 = r00;
    let rt01 = r10;
    let rt02 = r20;
    let rt10 = r01;
    let rt11 = r11;
    let rt12 = r21;
    let rt20 = r02;
    let rt21 = r12;
    let rt22 = r22;
    let itx = -(rt00 * tx + rt01 * ty + rt02 * tz);
    let ity = -(rt10 * tx + rt11 * ty + rt12 * tz);
    let itz = -(rt20 * tx + rt21 * ty + rt22 * tz);
    [
        rt00, rt01, rt02, 0.0, rt10, rt11, rt12, 0.0, rt20, rt21, rt22, 0.0, itx, ity, itz, 1.0,
    ]
}

fn push_accessor_vec3_json_with_bounds(
    root: &mut JsonValue,
    view: usize,
    count: u32,
    minmax: Option<([f32; 3], [f32; 3])>,
) -> usize {
    let accessors = root
        .get_mut("accessors")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    let mut acc =
        json!({ "bufferView": view, "componentType": 5126, "count": count, "type": "VEC3" });
    if let Some((min, max)) = minmax {
        if let Some(obj) = acc.as_object_mut() {
            obj.insert("min".into(), json!(min));
            obj.insert("max".into(), json!(max));
        }
    }
    accessors.push(acc);
    accessors.len() - 1
}

fn export_animations(
    file: &W3dFile,
    root: &mut JsonValue,
    bin: &mut Vec<u8>,
    buffer_index: usize,
    joint_node_indices: &Option<Vec<usize>>,
    hierarchy_ref: Option<&W3dHierarchy>,
) {
    if file.animations.is_empty() {
        return;
    }
    let nodes_for_pivots = if let Some(v) = joint_node_indices {
        v
    } else {
        return;
    };
    let hname = hierarchy_ref.map(|h| h.name()).unwrap_or_default();

    for anim in &file.animations {
        // Match animation to current hierarchy if named
        let anim_hname = anim.hierarchy_name();
        if !anim_hname.is_empty() && !hname.is_empty() && anim_hname != hname {
            continue;
        }

        // Ensure animations array exists only when we actually push
        if root.get("animations").is_none() {
            if let Some(obj) = root.as_object_mut() {
                obj.insert("animations".into(), json!([]));
            }
        }
        let mut anim_obj = json!({ "name": anim.name(), "samplers": [], "channels": [] });
        // Build per-pivot aggregation for translation (X,Y,Z) and rotation (Q)
        let mut trans_src: std::collections::HashMap<
            u16,
            (
                Option<&W3dAnimationChannel>,
                Option<&W3dAnimationChannel>,
                Option<&W3dAnimationChannel>,
            ),
        > = std::collections::HashMap::new();
        let mut quat_src: std::collections::HashMap<u16, &W3dAnimationChannel> =
            std::collections::HashMap::new();
        // include classic channels
        for ch in &anim.channels {
            match ch.flags as u16 {
                0 => {
                    // X
                    let entry = trans_src.entry(ch.pivot).or_insert((None, None, None));
                    entry.0 = Some(ch);
                }
                1 => {
                    // Y
                    let entry = trans_src.entry(ch.pivot).or_insert((None, None, None));
                    entry.1 = Some(ch);
                }
                2 => {
                    // Z
                    let entry = trans_src.entry(ch.pivot).or_insert((None, None, None));
                    entry.2 = Some(ch);
                }
                6 => {
                    // Q quaternion
                    quat_src.insert(ch.pivot, ch);
                }
                _ => {}
            }
        }
        // include densified extra channels (from timecoded/compressed)
        for ch in &anim.extra_channels {
            match ch.flags as u16 {
                0 => {
                    let entry = trans_src.entry(ch.pivot).or_insert((None, None, None));
                    entry.0 = Some(ch);
                }
                1 => {
                    let entry = trans_src.entry(ch.pivot).or_insert((None, None, None));
                    entry.1 = Some(ch);
                }
                2 => {
                    let entry = trans_src.entry(ch.pivot).or_insert((None, None, None));
                    entry.2 = Some(ch);
                }
                6 => {
                    quat_src.insert(ch.pivot, ch);
                }
                _ => {}
            }
        }

        // Helper to write f32 array and get bufferView index (as a free fn to avoid borrow issues)
        fn write_f32_view(
            root: &mut JsonValue,
            bin: &mut Vec<u8>,
            buffer_index: usize,
            vals: &[f32],
        ) -> usize {
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let off = bin.len();
            for f in vals {
                bin.extend_from_slice(&f.to_le_bytes());
            }
            push_view_json(root, buffer_index, off as u32, (bin.len() - off) as u32)
        }

        // For each pivot with XYZ translation present, create sampler+channel
        for (pivot, (cx_opt, cy_opt, cz_opt)) in trans_src.into_iter() {
            // Ensure all X,Y,Z present
            let (cx, cy, cz) = match (cx_opt, cy_opt, cz_opt) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => continue,
            };
            if cx.vector_len != 1 || cy.vector_len != 1 || cz.vector_len != 1 {
                continue;
            }
            let first = cx.first_frame.max(cy.first_frame).max(cz.first_frame) as u32;
            let last = cx.last_frame.min(cy.last_frame).min(cz.last_frame) as u32;
            if last < first {
                continue;
            }
            let frame_count = (last - first + 1) as usize;
            let mut times: Vec<f32> = Vec::with_capacity(frame_count);
            let mut values: Vec<f32> = Vec::with_capacity(frame_count * 3);
            let fps = anim.header.frame_rate as f32;
            for f in 0..frame_count {
                let frame = first + f as u32;
                times.push(frame as f32 / fps);
                let ix = (frame as i32 - cx.first_frame as i32) as usize;
                let iy = (frame as i32 - cy.first_frame as i32) as usize;
                let iz = (frame as i32 - cz.first_frame as i32) as usize;
                values.push(cx.data[ix]);
                values.push(cy.data[iy]);
                values.push(cz.data[iz]);
            }
            let t_view = write_f32_view(root, bin, buffer_index, &times);
            let v_view = write_f32_view(root, bin, buffer_index, &values);
            let (tmin, tmax) = times
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(mn, mx), &v| {
                    (mn.min(v), mx.max(v))
                });
            let t_acc =
                push_accessor_scalar_f32_with_bounds(root, t_view, times.len() as u32, tmin, tmax);
            let v_acc = push_accessor_vec3_json(root, v_view, frame_count as u32);
            // Add sampler
            let samplers = anim_obj
                .get_mut("samplers")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let s_idx = samplers.len();
            samplers.push(json!({ "input": t_acc, "output": v_acc, "interpolation": "LINEAR" }));
            // Channel
            let channels = anim_obj
                .get_mut("channels")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let node_index = *nodes_for_pivots.get(pivot as usize).unwrap_or(&0usize);
            channels.push(json!({ "sampler": s_idx, "target": { "node": node_index, "path": "translation" } }));
        }

        // Rotation tracks: prefer quaternion; if absent but Euler XR/YR/ZR present, synthesize quaternions per frame
        let quat_pivots: std::collections::HashSet<u16> = quat_src.keys().copied().collect();
        for (pivot, cq) in quat_src.into_iter() {
            if cq.vector_len != 4 {
                continue;
            }
            let first = cq.first_frame as u32;
            let last = cq.last_frame as u32;
            if last < first {
                continue;
            }
            let frame_count = (last - first + 1) as usize;
            let mut times: Vec<f32> = Vec::with_capacity(frame_count);
            let mut values: Vec<f32> = Vec::with_capacity(frame_count * 4);
            let fps = anim.header.frame_rate as f32;
            for i in 0..frame_count {
                let frame = first + i as u32;
                times.push(frame as f32 / fps);
                let base = (i + (first as usize - cq.first_frame as usize)) * 4;
                // x,y,z,w already in file order
                values.extend_from_slice(&[
                    cq.data[base + 0],
                    cq.data[base + 1],
                    cq.data[base + 2],
                    cq.data[base + 3],
                ]);
            }
            let t_view = write_f32_view(root, bin, buffer_index, &times);
            let v_view = write_f32_view(root, bin, buffer_index, &values);
            let (tmin, tmax) = times
                .iter()
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(mn, mx), &v| {
                    (mn.min(v), mx.max(v))
                });
            let t_acc =
                push_accessor_scalar_f32_with_bounds(root, t_view, times.len() as u32, tmin, tmax);
            let v_acc = push_accessor_vec4_f32_json(root, v_view, frame_count as u32);
            let samplers = anim_obj
                .get_mut("samplers")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let s_idx = samplers.len();
            samplers.push(json!({ "input": t_acc, "output": v_acc, "interpolation": "LINEAR" }));
            let channels = anim_obj
                .get_mut("channels")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let node_index = *nodes_for_pivots.get(pivot as usize).unwrap_or(&0usize);
            channels.push(
                json!({ "sampler": s_idx, "target": { "node": node_index, "path": "rotation" } }),
            );
        }

        // Fallback Euler rotation to quaternion if no Q channel exists for that pivot but XR/YR/ZR channels do
        // Look through original channels to find XR/YR/ZR sets
        use std::collections::HashMap;
        let mut euler_map: HashMap<
            u16,
            (
                Option<&W3dAnimationChannel>,
                Option<&W3dAnimationChannel>,
                Option<&W3dAnimationChannel>,
            ),
        > = HashMap::new();
        for ch in &anim.channels {
            match ch.flags as u16 {
                3 => {
                    let e = euler_map.entry(ch.pivot).or_insert((None, None, None));
                    e.0 = Some(ch);
                } // XR
                4 => {
                    let e = euler_map.entry(ch.pivot).or_insert((None, None, None));
                    e.1 = Some(ch);
                } // YR
                5 => {
                    let e = euler_map.entry(ch.pivot).or_insert((None, None, None));
                    e.2 = Some(ch);
                } // ZR
                _ => {}
            }
        }
        for (pivot, (xr, yr, zr)) in euler_map.into_iter() {
            if quat_pivots.contains(&pivot) {
                continue;
            } // already have quaternion
            let (xr, yr, zr) = match (xr, yr, zr) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => continue,
            };
            if xr.vector_len != 1 || yr.vector_len != 1 || zr.vector_len != 1 {
                continue;
            }
            // Determine overlapping frame range
            let first = xr.first_frame.max(yr.first_frame).max(zr.first_frame) as u32;
            let last = xr.last_frame.min(yr.last_frame).min(zr.last_frame) as u32;
            if last < first {
                continue;
            }
            let frame_count = (last - first + 1) as usize;
            let mut times: Vec<f32> = Vec::with_capacity(frame_count);
            let mut values: Vec<f32> = Vec::with_capacity(frame_count * 4);
            let fps = anim.header.frame_rate as f32;
            for i in 0..frame_count {
                let frame = first + i as u32;
                times.push(frame as f32 / fps);
                let ix = (frame as i32 - xr.first_frame as i32) as usize;
                let iy = (frame as i32 - yr.first_frame as i32) as usize;
                let iz = (frame as i32 - zr.first_frame as i32) as usize;
                let rx = xr.data[ix];
                let ry = yr.data[iy];
                let rz = zr.data[iz];
                // Convert Euler (XYZ order, radians assumed) to quaternion
                let (sx, cx) = (0.5 * rx).sin_cos();
                let (sy, cy) = (0.5 * ry).sin_cos();
                let (sz, cz) = (0.5 * rz).sin_cos();
                let qx = sx * cy * cz + cx * sy * sz;
                let qy = cx * sy * cz - sx * cy * sz;
                let qz = cx * cy * sz + sx * sy * cz;
                let qw = cx * cy * cz - sx * sy * sz;
                values.extend_from_slice(&[qx, qy, qz, qw]);
            }
            let t_view = write_f32_view(root, bin, buffer_index, &times);
            let v_view = write_f32_view(root, bin, buffer_index, &values);
            let t_acc = push_accessor_scalar_f32_json(root, t_view, times.len() as u32);
            let v_acc = push_accessor_vec4_f32_json(root, v_view, frame_count as u32);
            let samplers = anim_obj
                .get_mut("samplers")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let s_idx = samplers.len();
            samplers.push(json!({ "input": t_acc, "output": v_acc, "interpolation": "LINEAR" }));
            let channels = anim_obj
                .get_mut("channels")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            let node_index = *nodes_for_pivots.get(pivot as usize).unwrap_or(&0usize);
            channels.push(
                json!({ "sampler": s_idx, "target": { "node": node_index, "path": "rotation" } }),
            );
        }

        // Morph animations: drive primitive weights if morph targets exist
        if !anim.morph_tracks.is_empty() {
            // For simplicity: apply morph animation to all mesh primitives in the scene
            // Build a map from morph target name to weight index
            let mut target_name_to_index: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            if let Some(meshes) = root.get("meshes").and_then(|v| v.as_array()) {
                if let Some(first_primitive) = meshes
                    .get(0)
                    .and_then(|m| m.get("primitives"))
                    .and_then(|p| p.as_array())
                    .and_then(|a| a.get(0))
                {
                    if let Some(_weights) =
                        first_primitive.get("weights").and_then(|w| w.as_array())
                    {
                        if let Some(names) = first_primitive
                            .get("attributes")
                            .and_then(|a| a.get("_MORPH_TARGET_NAMES"))
                            .and_then(|n| n.as_array())
                        {
                            for (i, n) in names.iter().enumerate() {
                                if let Some(s) = n.as_str() {
                                    target_name_to_index.insert(s.to_string(), i);
                                }
                            }
                            // Build times and weight arrays per track index
                            let mut sampler_inputs: Vec<Vec<f32>> = vec![Vec::new(); names.len()];
                            let mut sampler_outputs: Vec<Vec<f32>> = vec![Vec::new(); names.len()];
                            for tr in &anim.morph_tracks {
                                if let Some(&idx) = target_name_to_index.get(&tr.target_name) {
                                    let mut times = Vec::with_capacity(tr.keys.len());
                                    let mut vals = Vec::with_capacity(tr.keys.len());
                                    let fps = anim.header.frame_rate as f32;
                                    for k in &tr.keys {
                                        times.push(k.frame as f32 / fps);
                                        vals.push(k.weight);
                                    }
                                    sampler_inputs[idx] = times;
                                    sampler_outputs[idx] = vals;
                                }
                            }
                            // Create one animation sampler per target index and a channel targeting "weights"
                            for (idx, (times, vals)) in sampler_inputs
                                .into_iter()
                                .zip(sampler_outputs.into_iter())
                                .enumerate()
                            {
                                if times.is_empty() {
                                    continue;
                                }
                                let t_view = write_f32_view(root, bin, buffer_index, &times);
                                let v_view = write_f32_view(root, bin, buffer_index, &vals);
                                let (tmin, tmax) = times
                                    .iter()
                                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(mn, mx), &v| {
                                        (mn.min(v), mx.max(v))
                                    });
                                let t_acc = push_accessor_scalar_f32_with_bounds(
                                    root,
                                    t_view,
                                    times.len() as u32,
                                    tmin,
                                    tmax,
                                );
                                let v_acc =
                                    push_accessor_scalar_f32_json(root, v_view, vals.len() as u32);
                                let samplers = anim_obj
                                    .get_mut("samplers")
                                    .and_then(|v| v.as_array_mut())
                                    .unwrap();
                                let s_idx = samplers.len();
                                samplers.push(json!({ "input": t_acc, "output": v_acc, "interpolation": "LINEAR" }));
                                let channels = anim_obj
                                    .get_mut("channels")
                                    .and_then(|v| v.as_array_mut())
                                    .unwrap();
                                // Route to node 0 (first mesh node) for now
                                let node_index = 0usize;
                                channels.push(json!({ "sampler": s_idx, "target": { "node": node_index, "path": "weights", "extensions": {"_targetMorphIndex": idx} } }));
                            }
                        }
                    }
                }
            }
        }

        // Visibility (bit) channels -> animate node scale between [1,1,1] and [0,0,0]
        if !anim.bit_channels.is_empty() {
            for bch in &anim.bit_channels {
                let pivot = bch.pivot as usize;
                if pivot >= nodes_for_pivots.len() {
                    continue;
                }
                let node_index = nodes_for_pivots[pivot];
                let first = bch.first_frame as u32;
                let last = bch.last_frame as u32;
                if last < first {
                    continue;
                }
                let frame_count = (last - first + 1) as usize;
                let mut times: Vec<f32> = Vec::with_capacity(frame_count);
                let mut values: Vec<f32> = Vec::with_capacity(frame_count * 3);
                let fps = anim.header.frame_rate as f32;
                // decode bit stream
                for i in 0..frame_count {
                    let frame = first + i as u32;
                    times.push(frame as f32 / fps);
                    let bit_index = i;
                    let byte_index = bit_index / 8;
                    let bit_in_byte = bit_index % 8;
                    let visible = if byte_index < bch.data.len() {
                        ((bch.data[byte_index] >> bit_in_byte) & 1) != 0
                    } else {
                        bch.default_val != 0
                    };
                    if visible {
                        values.extend_from_slice(&[1.0, 1.0, 1.0]);
                    } else {
                        values.extend_from_slice(&[0.0, 0.0, 0.0]);
                    }
                }
                let t_view = write_f32_view(root, bin, buffer_index, &times);
                let v_view = write_f32_view(root, bin, buffer_index, &values);
                let (tmin, tmax) = times
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(mn, mx), &v| {
                        (mn.min(v), mx.max(v))
                    });
                let t_acc = push_accessor_scalar_f32_with_bounds(
                    root,
                    t_view,
                    times.len() as u32,
                    tmin,
                    tmax,
                );
                let v_acc = push_accessor_vec3_f32_json(root, v_view, frame_count as u32);
                let samplers = anim_obj
                    .get_mut("samplers")
                    .and_then(|v| v.as_array_mut())
                    .unwrap();
                let s_idx = samplers.len();
                samplers.push(json!({ "input": t_acc, "output": v_acc, "interpolation": "STEP" }));
                let channels = anim_obj
                    .get_mut("channels")
                    .and_then(|v| v.as_array_mut())
                    .unwrap();
                channels.push(
                    json!({ "sampler": s_idx, "target": { "node": node_index, "path": "scale" } }),
                );
            }
        }

        // Push animation if it has channels
        let channel_count = anim_obj
            .get("channels")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if channel_count > 0 {
            let animations = root
                .get_mut("animations")
                .and_then(|v| v.as_array_mut())
                .unwrap();
            animations.push(anim_obj);
        }
    }
}

// accessors & views moved to writer_buffers.rs

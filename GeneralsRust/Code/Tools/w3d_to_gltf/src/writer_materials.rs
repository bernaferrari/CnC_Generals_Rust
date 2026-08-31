use serde_json::{Value as JsonValue, json};

use crate::w3d::structs::*;

// W3D texture flags/hints (mirrors C++ header)
pub(crate) const W3DTEXTURE_NO_LOD: u16 = 0x0004;
pub(crate) const W3DTEXTURE_CLAMP_U: u16 = 0x0008;
pub(crate) const W3DTEXTURE_CLAMP_V: u16 = 0x0010;
pub(crate) const W3DTEXTURE_HINT_SHIFT: u16 = 8; // attributes high byte stores hint
pub(crate) const W3DTEXTURE_HINT_MASK: u16 = 0x00FF; // after shift
pub(crate) const W3DTEXTURE_HINT_BASE: u16 = 0x00;
pub(crate) const W3DTEXTURE_HINT_EMISSIVE: u16 = 0x01;
pub(crate) const W3DTEXTURE_TYPE_BUMPMAP: u16 = 0x1000;

pub(crate) fn push_material_for_mesh(root: &mut JsonValue, mesh: &W3dMesh) -> usize {
    // Determine if TEXCOORD_0 exists (stage 0 present and sized to vertex count)
    let has_uv0 = mesh
        .material_passes
        .get(0)
        .and_then(|p| p.texture_stages.get(0))
        .map(|st| st.tex_coords.len() == mesh.vertices.len() && !st.tex_coords.is_empty())
        .unwrap_or(false);
    // First preference: W3D material3 (if present)
    let (r, g, b, a, mut metallic, mut roughness, double_sided) =
        if let Some(mat3) = mesh.materials.get(0) {
            let info = &mat3.info;
            let base = info.diffuse_color;
            let spec = info.specular_color;
            // Heuristics for PBR conversion
            let spec_intensity = (spec.r as f32 + spec.g as f32 + spec.b as f32) / (255.0 * 3.0);
            let metallic_h = if spec_intensity > 0.8 {
                0.8
            } else if spec_intensity > 0.5 {
                0.3
            } else {
                0.1
            };
            let shininess = info.shininess.max(1.0).min(1000.0);
            let roughness_h = (1.0 - (shininess - 1.0) / 999.0).clamp(0.04, 1.0);
            let double_sided = (mesh.header.attributes & 0x00002000) != 0; // W3D_MESH_FLAG_TWO_SIDED
            (
                base.r as f32 / 255.0,
                base.g as f32 / 255.0,
                base.b as f32 / 255.0,
                info.opacity,
                metallic_h,
                roughness_h,
                double_sided,
            )
        } else if let Some(vm) = mesh.vertex_materials.get(0) {
            let c = vm.info.diffuse;
            let specular = vm.info.specular;
            let spec_intensity =
                (specular.r as f32 + specular.g as f32 + specular.b as f32) / (255.0 * 3.0);
            let metallic = if spec_intensity > 0.8 {
                0.8
            } else if spec_intensity > 0.5 {
                0.3
            } else {
                0.1
            };
            let shininess = vm.info.shininess.max(1.0).min(1000.0);
            let roughness = (1.0 - (shininess - 1.0) / 999.0).clamp(0.04, 1.0);
            let double_sided = (mesh.header.attributes & 0x00002000) != 0;
            (
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                vm.info.opacity,
                metallic,
                roughness,
                double_sided,
            )
        } else {
            (0.8, 0.8, 0.8, 1.0, 0.1, 0.8, false)
        };

    // Build material object first
    let mut material = json!({
        "pbrMetallicRoughness": {
            "baseColorFactor": [r, g, b, a],
            "metallicFactor": metallic,
            "roughnessFactor": roughness
        }
    });

    // Try to bind textures based on W3D Material3 maps first (DC/SC/SI)
    if let Some(mat3) = mesh.materials.get(0) {
        if has_uv0 {
            if let Some(dc) = mat3
                .maps
                .get("DC")
                .map(|m| m.filename.as_str())
                .filter(|s| !s.is_empty())
            {
                let tex_idx = ensure_texture_index_scoped(root, dc);
                material["pbrMetallicRoughness"]["baseColorTexture"] =
                    json!({ "index": tex_idx, "texCoord": 0 });
                // Record diffuse map for possible baking
                let extras = material
                    .as_object_mut()
                    .unwrap()
                    .entry("extras")
                    .or_insert(json!({}));
                if let Some(obj) = extras.as_object_mut() {
                    obj.insert("w3dDiffuseMap".into(), json!(dc));
                }
                record_texture_anim_extras(&mut material, mesh, dc, "baseColor");
            }
        }
        // Detail map (DI) for baking into base color
        if let Some(di) = mat3
            .maps
            .get("DI")
            .map(|m| m.filename.as_str())
            .filter(|s| !s.is_empty())
        {
            let extras = material
                .as_object_mut()
                .unwrap()
                .entry("extras")
                .or_insert(json!({}));
            if let Some(obj) = extras.as_object_mut() {
                obj.insert("w3dDetailMap".into(), json!(di));
            }
        }
        if has_uv0 {
            if let Some(si) = mat3
                .maps
                .get("SI")
                .map(|m| m.filename.as_str())
                .filter(|s| !s.is_empty())
            {
                let tex_idx = ensure_texture_index_scoped(root, si);
                material["emissiveTexture"] = json!({ "index": tex_idx, "texCoord": 0 });
                material["emissiveFactor"] = json!([1.0, 1.0, 1.0]);
                record_texture_anim_extras(&mut material, mesh, si, "emissive");
            }
        }
        // Specular map (SC): adjust factors if present (heuristic)
        if let Some(sc) = mat3
            .maps
            .get("SC")
            .map(|m| m.filename.as_str())
            .filter(|s| !s.is_empty())
        {
            metallic = (metallic + 0.2f32).clamp(0.0f32, 1.0f32);
            roughness = (roughness - 0.2f32).clamp(0.04f32, 1.0f32);
            material["pbrMetallicRoughness"]["metallicFactor"] = json!(metallic);
            material["pbrMetallicRoughness"]["roughnessFactor"] = json!(roughness);
            // Record specular map filename for optional baking stage
            let extras = material
                .as_object_mut()
                .unwrap()
                .entry("extras")
                .or_insert(json!({}));
            if let Some(obj) = extras.as_object_mut() {
                obj.insert("w3dSpecularMap".into(), json!(sc));
            }
        }
    }

    // If no material3 maps, try to bind textures based on W3D hints
    if has_uv0 {
        if let Some((tex_idx, attrs)) = find_texture_index_by_hint(root, mesh, W3DTEXTURE_HINT_BASE)
        {
            let sampler_idx = ensure_sampler_for_attributes(root, attrs);
            set_texture_sampler(root, tex_idx, sampler_idx);
            material["pbrMetallicRoughness"]["baseColorTexture"] =
                json!({ "index": tex_idx, "texCoord": 0 });
        } else if let Some(tex_name) = first_color_texture_name(mesh) {
            let tex_idx = ensure_texture_index_scoped(root, &tex_name);
            material["pbrMetallicRoughness"]["baseColorTexture"] =
                json!({ "index": tex_idx, "texCoord": 0 });
            record_texture_anim_extras(&mut material, mesh, &tex_name, "baseColor");
        }
    }

    if has_uv0 {
        if let Some((tex_idx, attrs)) =
            find_texture_index_by_hint(root, mesh, W3DTEXTURE_HINT_EMISSIVE)
        {
            let sampler_idx = ensure_sampler_for_attributes(root, attrs);
            set_texture_sampler(root, tex_idx, sampler_idx);
            material["emissiveTexture"] = json!({ "index": tex_idx, "texCoord": 0 });
            material["emissiveFactor"] = json!([1.0, 1.0, 1.0]);
        }
    }

    if has_uv0 {
        if let Some((tex_idx, attrs)) = find_bump_texture_index(root, mesh) {
            let sampler_idx = ensure_sampler_for_attributes(root, attrs);
            set_texture_sampler(root, tex_idx, sampler_idx);
            material["normalTexture"] = json!({ "index": tex_idx, "texCoord": 0 });
            // Also remember the bump map filename for optional baking
            if let Some(bump_name) = mesh
                .textures
                .iter()
                .find(|t| (t.info.attributes & W3DTEXTURE_TYPE_BUMPMAP) != 0)
                .map(|t| t.name.as_str())
                .filter(|s| !s.is_empty())
            {
                let extras = material
                    .as_object_mut()
                    .unwrap()
                    .entry("extras")
                    .or_insert(json!({}));
                if let Some(obj) = extras.as_object_mut() {
                    obj.insert("w3dBumpMap".into(), json!(bump_name));
                }
            }
        }
    }

    // Handle alpha and doubleSided
    if a < 1.0 {
        material["alphaMode"] = json!("BLEND");
    }
    if double_sided {
        material["doubleSided"] = json!(true);
    }

    // If shaders indicate alpha test, prefer MASK unless already BLEND
    let alpha_test_enabled = mesh
        .material_passes
        .get(0)
        .and_then(|p| p.shader_ids.get(0))
        .and_then(|sid| mesh.shaders.get(*sid as usize))
        .map(|sh| sh.alpha_test != 0)
        .unwrap_or(false);
    if alpha_test_enabled {
        if material.get("alphaMode").and_then(|v| v.as_str()) != Some("BLEND") {
            material["alphaMode"] = json!("MASK");
            // Empirical cutoff used by game shaders ~0.376
            material["alphaCutoff"] = json!(0.376);
        }
    }

    // Emissive from vertex material emissive color (if present)
    if let Some(vm) = mesh.vertex_materials.get(0) {
        let e = vm.info.emissive;
        if e.r != 0 || e.g != 0 || e.b != 0 {
            material["emissiveFactor"] =
                json!([e.r as f32 / 255.0, e.g as f32 / 255.0, e.b as f32 / 255.0]);
        }
    }

    // Add material name if available from mesh
    let mesh_name = mesh.mesh_name();
    if !mesh_name.is_empty() {
        material["name"] = json!(format!("{}_material", mesh_name));
    }

    // Record original W3D texture stages to help optional baking passes
    if let Some(pass0) = mesh.material_passes.get(0) {
        if pass0.texture_stages.len() > 1 {
            let mut stage_names: Vec<String> = Vec::new();
            for st in &pass0.texture_stages {
                for &tid in &st.texture_ids {
                    let i = tid as usize;
                    if i < mesh.textures.len() {
                        let name = mesh.textures[i].name.trim();
                        if !name.is_empty() {
                            stage_names.push(name.to_string());
                            break;
                        }
                    }
                }
            }
            if !stage_names.is_empty() {
                let extras = material
                    .as_object_mut()
                    .unwrap()
                    .entry("extras")
                    .or_insert(json!({}));
                if let Some(obj) = extras.as_object_mut() {
                    obj.insert("w3dTextureStages".into(), json!(stage_names));
                }
            }
        }
    }

    let mats = root
        .get_mut("materials")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    let idx = mats.len();
    mats.push(material);
    idx
}

pub(crate) fn first_color_texture_name(mesh: &W3dMesh) -> Option<String> {
    if let Some(pass) = mesh.material_passes.get(0) {
        if let Some(stage) = pass.texture_stages.get(0) {
            for &tid in &stage.texture_ids {
                let i = tid as usize;
                if i < mesh.textures.len() {
                    let name = mesh.textures[i].name.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }
    mesh.textures.get(0).and_then(|t| {
        if t.name.trim().is_empty() {
            None
        } else {
            Some(t.name.clone())
        }
    })
}

fn record_texture_anim_extras(
    material: &mut JsonValue,
    mesh: &W3dMesh,
    tex_name: &str,
    usage: &str,
) {
    // Find matching texture in mesh.textures and, if animated, record extras
    if let Some(tex) = mesh
        .textures
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(tex_name))
    {
        if tex.info.frame_count > 1 || tex.info.frame_rate > 0.0 {
            let entry = json!({
                "map": usage,
                "name": tex.name,
                "frameCount": tex.info.frame_count,
                "frameRate": tex.info.frame_rate,
            });
            let extras = material
                .as_object_mut()
                .unwrap()
                .entry("extras")
                .or_insert(json!({}));
            if let Some(obj) = extras.as_object_mut() {
                let arr = obj.entry("w3dTextureAnims").or_insert(json!([]));
                if let Some(v) = arr.as_array_mut() {
                    v.push(entry);
                }
            }
        }
    }
}

pub(crate) fn ensure_texture_index_scoped(root: &mut JsonValue, filename: &str) -> usize {
    let existing = {
        let textures = root
            .get_mut("textures")
            .and_then(|v| v.as_array_mut())
            .unwrap();
        textures
            .iter()
            .position(|t| t.get("name").and_then(|n| n.as_str()) == Some(filename))
    };
    if let Some(idx) = existing {
        return idx;
    }
    let img_idx = {
        let images = root
            .get_mut("images")
            .and_then(|v| v.as_array_mut())
            .unwrap();
        let idx = images.len();
        images.push(json!({ "uri": filename, "name": filename }));
        idx
    };
    let tex_idx = {
        let textures = root
            .get_mut("textures")
            .and_then(|v| v.as_array_mut())
            .unwrap();
        let idx = textures.len();
        textures.push(json!({ "source": img_idx, "name": filename }));
        idx
    };
    tex_idx
}

pub(crate) fn set_texture_sampler(
    root: &mut JsonValue,
    texture_index: usize,
    sampler_index: usize,
) {
    if let Some(textures) = root.get_mut("textures").and_then(|v| v.as_array_mut()) {
        if let Some(obj) = textures
            .get_mut(texture_index)
            .and_then(|v| v.as_object_mut())
        {
            obj.insert("sampler".into(), json!(sampler_index));
        }
    }
}

pub(crate) fn ensure_sampler_for_attributes(root: &mut JsonValue, attributes: u16) -> usize {
    // glTF constants: REPEAT=10497, CLAMP_TO_EDGE=33071; LINEAR=9729; LINEAR_MIPMAP_LINEAR=9987
    let wrap_s = if attributes & W3DTEXTURE_CLAMP_U != 0 {
        33071
    } else {
        10497
    };
    let wrap_t = if attributes & W3DTEXTURE_CLAMP_V != 0 {
        33071
    } else {
        10497
    };
    let min_filter = if attributes & W3DTEXTURE_NO_LOD != 0 {
        9729
    } else {
        9987
    };
    let mag_filter = 9729;

    let existing = {
        let samplers = root
            .get_mut("samplers")
            .and_then(|v| v.as_array_mut())
            .unwrap();
        samplers.iter().position(|s| {
            let so = s.as_object();
            so.and_then(|o| o.get("wrapS")).and_then(|v| v.as_i64()) == Some(wrap_s as i64)
                && so.and_then(|o| o.get("wrapT")).and_then(|v| v.as_i64()) == Some(wrap_t as i64)
                && so.and_then(|o| o.get("minFilter")).and_then(|v| v.as_i64())
                    == Some(min_filter as i64)
                && so.and_then(|o| o.get("magFilter")).and_then(|v| v.as_i64())
                    == Some(mag_filter as i64)
        })
    };
    if let Some(idx) = existing {
        return idx;
    }

    let samplers = root
        .get_mut("samplers")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    let idx = samplers.len();
    samplers.push(json!({
        "wrapS": wrap_s,
        "wrapT": wrap_t,
        "minFilter": min_filter,
        "magFilter": mag_filter
    }));
    idx
}

pub(crate) fn texture_hint_from_attributes(attributes: u16) -> u16 {
    ((attributes >> W3DTEXTURE_HINT_SHIFT) & W3DTEXTURE_HINT_MASK) as u16
}

pub(crate) fn find_texture_index_by_hint(
    root: &mut JsonValue,
    mesh: &W3dMesh,
    desired_hint: u16,
) -> Option<(usize, u16)> {
    if let Some(pass) = mesh.material_passes.get(0) {
        if let Some(stage) = pass.texture_stages.get(0) {
            for &tid in &stage.texture_ids {
                let i = tid as usize;
                if i < mesh.textures.len() {
                    let t = &mesh.textures[i];
                    let hint = texture_hint_from_attributes(t.info.attributes);
                    if hint == desired_hint {
                        let idx = ensure_texture_index_scoped(root, t.name.as_str());
                        return Some((idx, t.info.attributes));
                    }
                }
            }
        }
    }
    for t in &mesh.textures {
        if texture_hint_from_attributes(t.info.attributes) == desired_hint {
            let idx = ensure_texture_index_scoped(root, t.name.as_str());
            return Some((idx, t.info.attributes));
        }
    }
    None
}

pub(crate) fn find_bump_texture_index(
    root: &mut JsonValue,
    mesh: &W3dMesh,
) -> Option<(usize, u16)> {
    if let Some(pass) = mesh.material_passes.get(0) {
        if let Some(stage) = pass.texture_stages.get(0) {
            for &tid in &stage.texture_ids {
                let i = tid as usize;
                if i < mesh.textures.len() {
                    let t = &mesh.textures[i];
                    if (t.info.attributes & W3DTEXTURE_TYPE_BUMPMAP) != 0 {
                        let idx = ensure_texture_index_scoped(root, t.name.as_str());
                        return Some((idx, t.info.attributes));
                    }
                }
            }
        }
    }
    for t in &mesh.textures {
        if (t.info.attributes & W3DTEXTURE_TYPE_BUMPMAP) != 0 {
            let idx = ensure_texture_index_scoped(root, t.name.as_str());
            return Some((idx, t.info.attributes));
        }
    }
    None
}

pub(crate) fn push_material_from_mat3(
    root: &mut JsonValue,
    mesh: &W3dMesh,
    mat3: &W3dMaterial3,
) -> usize {
    // Determine if TEXCOORD_0 exists (stage 0 present and sized to vertex count)
    let has_uv0 = mesh
        .material_passes
        .get(0)
        .and_then(|p| p.texture_stages.get(0))
        .map(|st| st.tex_coords.len() == mesh.vertices.len() && !st.tex_coords.is_empty())
        .unwrap_or(false);
    let info = &mat3.info;
    let base = info.diffuse_color;
    let spec = info.specular_color;
    let mut metallic = {
        let spec_intensity = (spec.r as f32 + spec.g as f32 + spec.b as f32) / (255.0 * 3.0);
        if spec_intensity > 0.8 {
            0.8
        } else if spec_intensity > 0.5 {
            0.3
        } else {
            0.1
        }
    };
    let mut roughness =
        (1.0 - (info.shininess.max(1.0).min(1000.0) - 1.0) / 999.0).clamp(0.04, 1.0);
    let mut material = json!({
        "pbrMetallicRoughness": {
            "baseColorFactor": [base.r as f32/255.0, base.g as f32/255.0, base.b as f32/255.0, info.opacity],
            "metallicFactor": metallic,
            "roughnessFactor": roughness
        }
    });
    if has_uv0 {
        if let Some(dc) = mat3
            .maps
            .get("DC")
            .map(|m| m.filename.as_str())
            .filter(|s| !s.is_empty())
        {
            let tex_idx = ensure_texture_index_scoped(root, dc);
            material["pbrMetallicRoughness"]["baseColorTexture"] =
                json!({ "index": tex_idx, "texCoord": 0 });
            let extras = material
                .as_object_mut()
                .unwrap()
                .entry("extras")
                .or_insert(json!({}));
            if let Some(obj) = extras.as_object_mut() {
                obj.insert("w3dDiffuseMap".into(), json!(dc));
            }
        }
    }
    if let Some(di) = mat3
        .maps
        .get("DI")
        .map(|m| m.filename.as_str())
        .filter(|s| !s.is_empty())
    {
        let extras = material
            .as_object_mut()
            .unwrap()
            .entry("extras")
            .or_insert(json!({}));
        if let Some(obj) = extras.as_object_mut() {
            obj.insert("w3dDetailMap".into(), json!(di));
        }
    }
    if has_uv0 {
        if let Some(si) = mat3
            .maps
            .get("SI")
            .map(|m| m.filename.as_str())
            .filter(|s| !s.is_empty())
        {
            let tex_idx = ensure_texture_index_scoped(root, si);
            material["emissiveTexture"] = json!({ "index": tex_idx, "texCoord": 0 });
            material["emissiveFactor"] = json!([1.0, 1.0, 1.0]);
        }
    }
    if let Some(sc) = mat3
        .maps
        .get("SC")
        .map(|m| m.filename.as_str())
        .filter(|s| !s.is_empty())
    {
        metallic = (metallic + 0.2f32).clamp(0.0f32, 1.0f32);
        roughness = (roughness - 0.2f32).clamp(0.04f32, 1.0f32);
        material["pbrMetallicRoughness"]["metallicFactor"] = json!(metallic);
        material["pbrMetallicRoughness"]["roughnessFactor"] = json!(roughness);
        let extras = material
            .as_object_mut()
            .unwrap()
            .entry("extras")
            .or_insert(json!({}));
        if let Some(obj) = extras.as_object_mut() {
            obj.insert("w3dSpecularMap".into(), json!(sc));
        }
    }
    if (mesh.header.attributes & 0x00002000) != 0 {
        material["doubleSided"] = json!(true);
    }
    let alpha_test_enabled = mesh
        .material_passes
        .get(0)
        .and_then(|p| p.shader_ids.get(0))
        .and_then(|sid| mesh.shaders.get(*sid as usize))
        .map(|sh| sh.alpha_test != 0)
        .unwrap_or(false);
    if alpha_test_enabled {
        // C++ shader.cpp suggests ALPHAREF ~ 0x60 (best for mipmapped), ~96/255 ≈ 0.376
        material["alphaMode"] = json!("MASK");
        material["alphaCutoff"] = json!(0.376);
    }
    if !mat3.name.is_empty() {
        material["name"] = json!(mat3.name.clone());
    }
    let mats = root
        .get_mut("materials")
        .and_then(|v| v.as_array_mut())
        .unwrap();
    let idx = mats.len();
    mats.push(material);
    idx
}

pub(crate) fn build_materials_for_mesh(root: &mut JsonValue, mesh: &W3dMesh) -> Vec<usize> {
    let mut out = Vec::new();
    if mesh.materials.is_empty() {
        out.push(push_material_for_mesh(root, mesh));
        return out;
    }
    for m in &mesh.materials {
        out.push(push_material_from_mat3(root, mesh, m));
    }
    out
}

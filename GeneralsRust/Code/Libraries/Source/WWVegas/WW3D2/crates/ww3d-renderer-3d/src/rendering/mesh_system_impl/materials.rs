#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;
use crate::material_system::ColorSourceType;

pub(super) struct StageMasks {
    pub mask: u8,
    pub cube_mask: u32,
    pub hints: u32,
    pub alpha_mask: u32,
    pub uv_channels: u32,
}

pub(super) fn compute_stage_masks(pass: &MaterialPassClass) -> StageMasks {
    let mut mask: u8 = 0;
    let mut cube_mask: u32 = 0;
    let mut hints: u32 = 0;
    let mut alpha_mask: u32 = 0;
    let mut uv_channels: u32 = 0;

    // Feature bits of the `texture_stage_mask.w` (cube_mask) uniform consumed
    // by every mesh shader variant (opaque/alpha/additive/skinned/decal):
    //   bits 0-7  stage N samples a cube map
    //   bit  8    the pass carries a diffuse vertex-color array (DCG / COLOR1)
    //   bit  9    the pass carries an illumination vertex-color array (DIG)
    //   bit 10    dynamic lighting enabled (VertexMaterialClass::UseLighting);
    //             prelit meshes (lighting OFF, COLOR1 on) skip the light loop
    const VERTEX_DIFFUSE_BIT: u32 = 1 << 8;
    const VERTEX_ILLUMINATION_BIT: u32 = 1 << 9;
    const USE_LIGHTING_BIT: u32 = 1 << 10;

    if pass
        .diffuse_vertex_colors
        .as_ref()
        .is_some_and(|colors| !colors.is_empty())
    {
        cube_mask |= VERTEX_DIFFUSE_BIT;
    }
    if pass
        .illumination_vertex_colors
        .as_ref()
        .is_some_and(|colors| !colors.is_empty())
    {
        cube_mask |= VERTEX_ILLUMINATION_BIT;
    }
    // Passes without a vertex material keep the legacy lit behaviour.
    let use_lighting = pass
        .vertex_material
        .as_ref()
        .map(|material| material.use_lighting)
        .unwrap_or(true);
    if use_lighting {
        cube_mask |= USE_LIGHTING_BIT;
    }

    for stage in 0..MAX_TEXTURE_STAGES {
        if let Some(texture) = pass.get_texture(stage) {
            mask |= 1 << stage;
            let hint_bits = texture.stage_settings.hint.to_bits() & 0x0F;
            hints |= hint_bits << (stage * 4);
            if texture.stage_settings.alpha_is_bitmap {
                alpha_mask |= 1 << stage;
            }
            let channel_bits = (pass.stage_uv_channel(stage) as u32) & 0x3;
            uv_channels |= channel_bits << (stage * 2);
        }
    }

    StageMasks {
        mask,
        cube_mask,
        hints,
        alpha_mask,
        uv_channels,
    }
}

pub(super) fn sampler_descriptor_for_settings(
    settings: &TextureStageSettings,
) -> SamplerDescriptor<'static> {
    let (mag_filter, min_filter, mipmap_filter) = match settings.filter {
        TextureFilterMode::Point | TextureFilterMode::Nearest => (
            FilterMode::Nearest,
            FilterMode::Nearest,
            FilterMode::Nearest,
        ),
        TextureFilterMode::Linear => (FilterMode::Linear, FilterMode::Linear, FilterMode::Linear),
        TextureFilterMode::Anisotropic => {
            (FilterMode::Linear, FilterMode::Linear, FilterMode::Linear)
        }
    };

    SamplerDescriptor {
        label: Some("MeshManager Stage Sampler"),
        address_mode_u: convert_address_mode(settings.address_u),
        address_mode_v: convert_address_mode(settings.address_v),
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter,
        min_filter,
        mipmap_filter,
        ..Default::default()
    }
}

pub(super) fn convert_address_mode(mode: TextureAddressMode) -> AddressMode {
    match mode {
        TextureAddressMode::Wrap => AddressMode::Repeat,
        TextureAddressMode::Repeat => AddressMode::Repeat,
        TextureAddressMode::Clamp => AddressMode::ClampToEdge,
        TextureAddressMode::Mirror => AddressMode::MirrorRepeat,
        TextureAddressMode::Border => AddressMode::ClampToBorder,
    }
}

pub(super) fn material_properties(
    material: Option<&VertexMaterialClass>,
) -> ([f32; 4], [f32; 4], [f32; 4]) {
    if let Some(mat) = material {
        (
            [mat.diffuse.x, mat.diffuse.y, mat.diffuse.z, 1.0],
            [
                mat.specular.x,
                mat.specular.y,
                mat.specular.z,
                mat.shininess,
            ],
            [mat.emissive.x, mat.emissive.y, mat.emissive.z, 1.0],
        )
    } else {
        (
            [0.8, 0.8, 0.8, 1.0],
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )
    }
}

pub(super) fn compute_stage_uv_info(
    stage_texcoords: &[Vec<W3dTexCoordStruct>],
) -> (Vec<Vec<W3dTexCoordStruct>>, Vec<u8>) {
    const MAX_CHANNELS: usize = 4;
    let mut uv_sets: Vec<Vec<W3dTexCoordStruct>> = Vec::new();
    let mut stage_channels = Vec::with_capacity(stage_texcoords.len());
    let mut crc_to_channel: HashMap<u32, u8> = HashMap::new();

    for coords in stage_texcoords {
        if coords.is_empty() {
            stage_channels.push(0);
            continue;
        }

        let mut hasher = Hasher::new();
        for tc in coords {
            hasher.update(&tc.u.to_le_bytes());
            hasher.update(&tc.v.to_le_bytes());
        }
        let crc = hasher.finalize();

        let mut channel = if let Some(&existing) = crc_to_channel.get(&crc) {
            existing
        } else {
            let assigned = if uv_sets.len() < MAX_CHANNELS {
                let ch = uv_sets.len() as u8;
                uv_sets.push(coords.clone());
                ch
            } else {
                (MAX_CHANNELS.saturating_sub(1)) as u8
            };
            crc_to_channel.insert(crc, assigned);
            assigned
        };

        if channel as usize >= uv_sets.len() {
            if uv_sets.len() < MAX_CHANNELS {
                uv_sets.push(coords.clone());
            } else {
                channel = (MAX_CHANNELS.saturating_sub(1)) as u8;
            }
        }

        stage_channels.push(channel);
    }

    if uv_sets.is_empty() {
        uv_sets.push(Vec::new());
    }

    (uv_sets, stage_channels)
}

pub(super) fn build_material_passes_from_prototype(
    prototype: &MeshPrototype,
) -> Vec<MaterialPassClass> {
    if prototype.passes.is_empty() {
        return Vec::new();
    }

    let mut vertex_material_cache: Vec<Arc<VertexMaterialClass>> =
        Vec::with_capacity(prototype.vertex_materials.len());
    for (index, material) in prototype.vertex_materials.iter().enumerate() {
        let name = prototype
            .vertex_material_names
            .get(index)
            .map(|entry| w3d_string_from_bytes(&entry.material_name))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("VertexMaterial{}", index));
        let mut vm = VertexMaterialClass::from_w3d_material(&name, material);
        vm.name = name;
        vertex_material_cache.push(Arc::new(vm));
    }

    // C++ meshmdlio.cpp:1800-1808 — vertex-lit (PRELIT_VERTEX) meshes need the
    // dynamic lighting turned off; every other mesh stays lit.
    let lighting_enabled = !prototype
        .header
        .as_ref()
        .map(|header| (header.attrs & W3D_MESH_FLAG_PRELIT_VERTEX) != 0)
        .unwrap_or(false);

    let (_, stage_channels) = compute_stage_uv_info(&prototype.stage_texcoords);
    let mut stage_cursor = 0usize;

    prototype
        .passes
        .iter()
        .enumerate()
        .map(|(pass_index, info)| {
            let mut pass = MaterialPassClass::new();

            if let Some(material) = vertex_material_cache.get(info.vm_id as usize) {
                pass.vertex_material = Some(Arc::clone(material));
            }

            if let Some(shader_struct) = prototype.shaders.get(info.shader_id as usize) {
                pass.shader = MaterialFactory::create_shader_from_w3d(shader_struct);
            }

            if let Some(stage_ids) = prototype.per_pass_stage_texture_ids.get(pass_index) {
                for (stage, ids) in stage_ids.iter().enumerate() {
                    let uv_channel = stage_channels
                        .get(stage_cursor)
                        .copied()
                        .unwrap_or(stage as u8);
                    pass.set_stage_uv_channel(stage, uv_channel);
                    stage_cursor = stage_cursor.saturating_add(1);

                    if let Some(&texture_id) = ids.first() {
                        if let Some(texture_desc) = prototype.textures.get(texture_id as usize) {
                            let texture = Arc::new(TextureClass::from_w3d_descriptor(texture_desc));
                            pass.set_texture(stage, texture);
                        }
                    }
                }
            }

            if let Some(colors) = prototype.per_pass_dcg_colors.get(pass_index) {
                if !colors.is_empty() {
                    let diffuse = colors
                        .iter()
                        .map(|c| {
                            Vec4::new(
                                c.r as f32 / 255.0,
                                c.g as f32 / 255.0,
                                c.b as f32 / 255.0,
                                c.a as f32 / 255.0,
                            )
                        })
                        .collect();
                    pass.diffuse_vertex_colors = Some(diffuse);
                }
            }

            if let Some(colors) = prototype.per_pass_dig_colors.get(pass_index) {
                if !colors.is_empty() {
                    let illumination = colors
                        .iter()
                        .map(|c| {
                            Vec4::new(
                                c.r as f32 / 255.0,
                                c.g as f32 / 255.0,
                                c.b as f32 / 255.0,
                                c.a as f32 / 255.0,
                            )
                        })
                        .collect();
                    pass.illumination_vertex_colors = Some(illumination);
                }
            }

            // MeshMatDescClass::Configure_Material parity
            // (meshmatdesc.cpp:903-917): each pass gets its own configured
            // material — diffuse source COLOR1 exactly when the pass carries a
            // DCG array, emissive source COLOR1 when it carries a DIG array,
            // and UseLighting per the mesh's prelit flag.
            configure_pass_vertex_material(&mut pass, lighting_enabled);

            apply_mapper_from_prototype(&mut pass, prototype, pass_index);

            pass
        })
        .collect()
}

pub(super) fn apply_mapper_from_prototype(
    pass: &mut MaterialPassClass,
    prototype: &MeshPrototype,
    pass_index: usize,
) {
    if let Some(vm_ids) = prototype.per_pass_vertex_material_ids.get(pass_index) {
        if let Some(&vm_id) = vm_ids.first() {
            if let Some(config) = prototype.vertex_mapper_configs.get(vm_id as usize) {
                if let Some(mapper) = config.stage0.or(config.stage1) {
                    pass.set_mapper_id(mapper.mapper_type);
                    for (idx, arg) in mapper.args.iter().enumerate() {
                        pass.set_mapper_arg(idx, *arg);
                    }
                    pass.set_mapper_float_args(mapper.float_args);
                }
            }
        }
    }
}

/// MeshMatDescClass::Configure_Material parity (meshmatdesc.cpp:903-917 with
/// the DCG/DIG source rules of meshmatdesc.cpp:743-805): the shared prototype
/// material is cloned per pass and configured with the pass's vertex-color
/// sources and lighting flag. Vertex materials are small POD-like structs, so
/// the per-pass clone matches the C++ per-pass material instances.
pub(super) fn configure_pass_vertex_material(pass: &mut MaterialPassClass, lighting_enabled: bool) {
    let Some(material) = pass.vertex_material.as_ref().map(Arc::clone) else {
        return;
    };
    let mut configured = (*material).clone();
    configured.use_lighting = lighting_enabled;
    configured.diffuse_color_source = if pass
        .diffuse_vertex_colors
        .as_ref()
        .is_some_and(|colors| !colors.is_empty())
    {
        ColorSourceType::Color1
    } else {
        ColorSourceType::Material
    };
    configured.emissive_color_source = if pass
        .illumination_vertex_colors
        .as_ref()
        .is_some_and(|colors| !colors.is_empty())
    {
        ColorSourceType::Color1
    } else {
        ColorSourceType::Material
    };
    pass.vertex_material = Some(Arc::new(configured));
}

#[cfg(test)]
mod compute_stage_masks_tests {
    use super::*;

    const VERTEX_DIFFUSE_BIT: u32 = 1 << 8;
    const VERTEX_ILLUMINATION_BIT: u32 = 1 << 9;
    const USE_LIGHTING_BIT: u32 = 1 << 10;

    #[test]
    fn pass_without_material_or_colors_keeps_lighting_and_disables_vertex_colors() {
        let pass = MaterialPassClass::new();
        let masks = compute_stage_masks(&pass);

        // No vertex material bound: legacy lit behaviour.
        assert_eq!(masks.cube_mask & USE_LIGHTING_BIT, USE_LIGHTING_BIT);
        assert_eq!(masks.cube_mask & VERTEX_DIFFUSE_BIT, 0);
        assert_eq!(masks.cube_mask & VERTEX_ILLUMINATION_BIT, 0);
    }

    #[test]
    fn vertex_color_arrays_set_bits_8_and_9() {
        let mut pass = MaterialPassClass::new();
        assert_eq!(compute_stage_masks(&pass).cube_mask & VERTEX_DIFFUSE_BIT, 0);

        pass.diffuse_vertex_colors = Some(vec![Vec4::new(0.5, 0.5, 0.5, 1.0)]);
        pass.illumination_vertex_colors = Some(vec![Vec4::new(1.0, 1.0, 1.0, 1.0)]);
        let masks = compute_stage_masks(&pass);

        assert_eq!(masks.cube_mask & VERTEX_DIFFUSE_BIT, VERTEX_DIFFUSE_BIT);
        assert_eq!(
            masks.cube_mask & VERTEX_ILLUMINATION_BIT,
            VERTEX_ILLUMINATION_BIT
        );
    }

    #[test]
    fn prelit_material_clears_lighting_bit_and_lit_material_sets_it() {
        let mut prelit = MaterialPassClass::new();
        let mut material = VertexMaterialClass::new("prelit");
        material.use_lighting = false;
        prelit.vertex_material = Some(Arc::new(material));
        assert_eq!(
            compute_stage_masks(&prelit).cube_mask & USE_LIGHTING_BIT,
            0,
            "prelit meshes (lighting OFF, COLOR1 on) must skip the light loop"
        );

        let mut lit = MaterialPassClass::new();
        let mut material = VertexMaterialClass::new("lit");
        material.use_lighting = true;
        lit.vertex_material = Some(Arc::new(material));
        assert_eq!(
            compute_stage_masks(&lit).cube_mask & USE_LIGHTING_BIT,
            USE_LIGHTING_BIT
        );
    }

    #[test]
    fn configure_pass_vertex_material_sources_arrays_and_lighting() {
        let mut pass = MaterialPassClass::new();
        pass.diffuse_vertex_colors = Some(vec![Vec4::ONE]);
        pass.illumination_vertex_colors = Some(vec![Vec4::ONE]);
        pass.vertex_material = Some(Arc::new(VertexMaterialClass::new("w3d")));

        configure_pass_vertex_material(&mut pass, false);
        let material = pass.vertex_material.as_ref().expect("configured material");
        assert!(!material.use_lighting);
        assert_eq!(material.diffuse_color_source, ColorSourceType::Color1);
        assert_eq!(material.emissive_color_source, ColorSourceType::Color1);

        // Empty arrays are treated as "no array" (C++ DCGSource MATERIAL).
        pass.diffuse_vertex_colors = Some(Vec::new());
        pass.illumination_vertex_colors = None;
        configure_pass_vertex_material(&mut pass, true);
        let material = pass.vertex_material.as_ref().expect("configured material");
        assert!(material.use_lighting);
        assert_eq!(material.diffuse_color_source, ColorSourceType::Material);
        assert_eq!(material.emissive_color_source, ColorSourceType::Material);
    }
}

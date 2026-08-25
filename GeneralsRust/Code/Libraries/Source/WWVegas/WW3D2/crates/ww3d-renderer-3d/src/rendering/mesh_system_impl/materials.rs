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

pub(super) struct StageMasks {
    pub mask: u8,
    pub cube_mask: u32,
    pub hints: u32,
    pub alpha_mask: u32,
    pub uv_channels: u32,
}

pub(super) fn compute_stage_masks(pass: &MaterialPassClass) -> StageMasks {
    let mut mask: u8 = 0;
    let cube_mask: u32 = 0;
    let mut hints: u32 = 0;
    let mut alpha_mask: u32 = 0;
    let mut uv_channels: u32 = 0;

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

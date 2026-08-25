//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_format::*;
use super::w3d_loader::*;
use super::w3d_mesh::*;
use super::w3d_mesh_build::*;
use super::w3d_model::*;
use super::*;

impl W3DLoader {
    /// Resolve texture indices to actual texture names - matches C++ behavior
    /// W3D_CHUNK_MAP3_FILENAME may contain texture indices (e.g., "1", "2", "3")
    /// which need to be resolved against the model.texture_names array
    ///
    /// Special case: If texture_names is empty but materials have numeric texture references,
    /// we need to build a texture array from materials in order (C++ behavior when W3D_CHUNK_TEXTURES is missing)
    pub(super) fn resolve_texture_indices(&self, model: &mut W3DModel) {
        // Check if any texture references are numeric indices
        let has_numeric_indices = model.materials.values().any(|mat| {
            if let Some(ref tex_ref) = mat.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        }) || model.meshes.iter().any(|mesh| {
            if let Some(ref tex_ref) = mesh.material.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        });

        // If we have numeric indices but no texture_names array, build one from materials
        if has_numeric_indices && model.texture_names.is_empty() {
            debug!("Building texture array from materials (W3D_CHUNK_TEXTURES missing)");

            // Collect all actual texture filenames from materials in order they appear
            let mut collected_textures: Vec<String> = Vec::new();

            for material in model.materials.values() {
                // Some materials might point to actual filenames (from DC_MAP chunks)
                if let Some(ref tex_name) = material.texture_name {
                    // Only add non-numeric filenames - these are actual texture names
                    if tex_name.parse::<usize>().is_err() && !collected_textures.contains(tex_name)
                    {
                        debug!("  Added texture from material: {}", tex_name);
                        collected_textures.push(tex_name.clone());
                    }
                }
            }

            // If we collected any textures, use them as the texture_names array
            if !collected_textures.is_empty() {
                debug!(
                    "Collected {} textures from materials",
                    collected_textures.len()
                );
                model.texture_names = collected_textures;
            } else {
                // No actual filenames found - this might be a pure index-based model
                debug!("No texture filenames in materials, cannot resolve indices");
                return;
            }
        }

        if model.texture_names.is_empty() {
            debug!(
                "No texture names loaded from W3D_CHUNK_TEXTURES, skipping texture index resolution"
            );
            return;
        }

        debug!("Resolving texture indices for model: {}", model.name);
        debug!("  Available textures: {:?}", model.texture_names);

        // Go through each mesh and resolve texture indices
        for mesh in &mut model.meshes {
            if mesh.texture_library.is_empty() {
                mesh.texture_library = model.texture_names.clone();
            }

            if let Some(ref texture_ref) = mesh.material.texture_name {
                // Try to parse texture_ref as an index
                if let Ok(index) = texture_ref.parse::<usize>() {
                    // It's an index - resolve it
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved texture index {} to texture name: {}",
                            index, resolved_name
                        );
                        mesh.material.texture_name = Some(resolved_name);
                    } else {
                        warn!(
                            "Texture index {} out of bounds (only {} textures available)",
                            index,
                            model.texture_names.len()
                        );
                    }
                } else {
                    // It's a filename, keep as-is
                    debug!(
                        "Texture reference '{}' is not an index, keeping as filename",
                        texture_ref
                    );
                }
            }

            if !mesh.per_pass_stage_texture_ids.is_empty() {
                let mut per_pass_names = Vec::with_capacity(mesh.per_pass_stage_texture_ids.len());
                for stages in &mesh.per_pass_stage_texture_ids {
                    let mut stage_names = Vec::with_capacity(stages.len());
                    for ids in stages {
                        let names = ids
                            .iter()
                            .filter_map(|texture_id| {
                                if *texture_id == u32::MAX {
                                    None
                                } else {
                                    mesh.texture_name_from_library(*texture_id)
                                        .map(|name| name.to_string())
                                }
                            })
                            .collect::<Vec<_>>();
                        stage_names.push(names);
                    }
                    per_pass_names.push(stage_names);
                }
                mesh.per_pass_stage_texture_names = per_pass_names;

                if mesh.material.texture_name.is_none() {
                    mesh.material.texture_name = Self::stage_texture_from_mesh(mesh, 0, 0);
                }
            }
        }

        // Also update materials map if they have texture references
        for (name, material) in &mut model.materials {
            if let Some(ref texture_ref) = material.texture_name {
                if let Ok(index) = texture_ref.parse::<usize>() {
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved material '{}' texture index {} to: {}",
                            name, index, resolved_name
                        );
                        let mut updated_material = material.clone();
                        updated_material.texture_name = Some(resolved_name);
                        *material = updated_material;
                    }
                }
            }
        }
    }

    pub(super) fn parse_u32_array(&self, data: &[u8]) -> Result<Vec<u32>> {
        if !data.len().is_multiple_of(4) {
            return Err(anyhow!("invalid u32 array length {}", data.len()));
        }
        let mut values = Vec::with_capacity(data.len() / 4);
        let mut offset = 0usize;
        while offset + 4 <= data.len() {
            values.push(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
        }
        Ok(values)
    }

    pub(super) fn parse_rgba_colors(&self, data: &[u8]) -> Result<Vec<W3dRGBAStruct>> {
        if !data.len().is_multiple_of(4) {
            return Err(anyhow!("invalid RGBA array length {}", data.len()));
        }
        let mut colors = Vec::with_capacity(data.len() / 4);
        let mut offset = 0usize;
        while offset + 4 <= data.len() {
            colors.push(W3dRGBAStruct {
                r: data[offset],
                g: data[offset + 1],
                b: data[offset + 2],
                a: data[offset + 3],
            });
            offset += 4;
        }
        Ok(colors)
    }

    pub(super) fn parse_per_face_texcoord_ids(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        if !data.len().is_multiple_of(12) {
            return Err(anyhow!(
                "invalid per-face texcoord id array length {}",
                data.len()
            ));
        }
        let mut values = Vec::with_capacity(data.len() / 12);
        let mut offset = 0usize;
        while offset + 12 <= data.len() {
            values.push([
                u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]),
                u32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]),
                u32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]),
            ]);
            offset += 12;
        }
        Ok(values)
    }

    pub(super) fn parse_texture_stage_chunk(&self, data: &[u8]) -> Result<ParsedTextureStage> {
        let mut stage = ParsedTextureStage::default();
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_TEXTURE_IDS => {
                    stage.texture_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_STAGE_TEXCOORDS | W3D_CHUNK_TEXCOORDS => {
                    stage.texcoords = self.parse_texcoords(chunk_data)?;
                }
                W3D_CHUNK_PER_FACE_TEXCOORD_IDS => {
                    stage.per_face_texcoord_ids = self.parse_per_face_texcoord_ids(chunk_data)?;
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }
        Ok(stage)
    }

    pub(super) fn parse_material_pass_chunk(&self, data: &[u8]) -> Result<ParsedMaterialPass> {
        let mut pass = ParsedMaterialPass::default();
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_VERTEX_MATERIAL_IDS => {
                    pass.vertex_material_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_SHADER_IDS => {
                    pass.shader_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_DCG => {
                    pass.dcg_colors = self.parse_rgba_colors(chunk_data)?;
                }
                W3D_CHUNK_DIG => {
                    // C++ reads DIG as W3dRGBAStruct and uses RGB channels.
                    pass.dig_colors = self.parse_rgba_colors(chunk_data)?;
                }
                W3D_CHUNK_TEXTURE_STAGE => {
                    let stage = self.parse_texture_stage_chunk(chunk_data)?;
                    pass.stage_texture_ids.push(stage.texture_ids);
                    pass.stage_texcoords.push(stage.texcoords);
                    pass.stage_per_face_texcoord_ids
                        .push(stage.per_face_texcoord_ids);
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(pass)
    }

    pub(super) fn parse_shaders_chunk(&self, data: &[u8]) -> Result<Vec<W3dShaderStruct>> {
        // C++ W3dShaderStruct is 16 bytes (15 data bytes + 1 pad byte).
        if !data.len().is_multiple_of(16) {
            return Err(anyhow!("invalid shader chunk length {}", data.len()));
        }

        let mut shaders = Vec::with_capacity(data.len() / 16);
        let mut offset = 0usize;
        while offset + 16 <= data.len() {
            shaders.push(W3dShaderStruct {
                depth_compare: data[offset],
                depth_mask: data[offset + 1],
                color_mask: data[offset + 2],
                dest_blend: data[offset + 3],
                fog_func: data[offset + 4],
                pri_gradient: data[offset + 5],
                sec_gradient: data[offset + 6],
                src_blend: data[offset + 7],
                texturing: data[offset + 8],
                detail_color_func: data[offset + 9],
                detail_alpha_func: data[offset + 10],
                shader_preset: data[offset + 11],
                alpha_test: data[offset + 12],
                post_detail_color_func: data[offset + 13],
                post_detail_alpha_func: data[offset + 14],
            });
            offset += 16;
        }
        Ok(shaders)
    }

    pub(super) fn default_vertex_material() -> W3dVertexMaterialStruct {
        W3dVertexMaterialStruct {
            attributes: 0,
            ambient: W3dRGBAStruct {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            diffuse: W3dRGBAStruct {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            specular: W3dRGBAStruct {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            emissive: W3dRGBAStruct {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }

    pub(super) fn parse_vertex_material_info_chunk(
        &self,
        data: &[u8],
    ) -> Result<W3dVertexMaterialStruct> {
        // C++ W3dVertexMaterialStruct uses 3-byte RGB triplets with 4-byte alignment.
        // Accept both canonical 28-byte layout and 32-byte RGBA-expanded variant.
        if data.len() < 28 {
            return Err(anyhow!(
                "vertex material info chunk too small: {} bytes",
                data.len()
            ));
        }

        let mut material = Self::default_vertex_material();
        material.attributes = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        if data.len() >= 32 {
            material.ambient = W3dRGBAStruct {
                r: data[4],
                g: data[5],
                b: data[6],
                a: data[7],
            };
            material.diffuse = W3dRGBAStruct {
                r: data[8],
                g: data[9],
                b: data[10],
                a: data[11],
            };
            material.specular = W3dRGBAStruct {
                r: data[12],
                g: data[13],
                b: data[14],
                a: data[15],
            };
            material.emissive = W3dRGBAStruct {
                r: data[16],
                g: data[17],
                b: data[18],
                a: data[19],
            };
            material.shininess = f32::from_le_bytes([data[20], data[21], data[22], data[23]]);
            material.opacity = f32::from_le_bytes([data[24], data[25], data[26], data[27]]);
            material.translucency = f32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        } else {
            material.ambient = W3dRGBAStruct {
                r: data[4],
                g: data[5],
                b: data[6],
                a: 255,
            };
            material.diffuse = W3dRGBAStruct {
                r: data[7],
                g: data[8],
                b: data[9],
                a: 255,
            };
            material.specular = W3dRGBAStruct {
                r: data[10],
                g: data[11],
                b: data[12],
                a: 255,
            };
            material.emissive = W3dRGBAStruct {
                r: data[13],
                g: data[14],
                b: data[15],
                a: 255,
            };
            material.shininess = f32::from_le_bytes([data[16], data[17], data[18], data[19]]);
            material.opacity = f32::from_le_bytes([data[20], data[21], data[22], data[23]]);
            material.translucency = f32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        }

        Ok(material)
    }

    pub(super) fn parse_single_vertex_material_chunk(
        &self,
        data: &[u8],
    ) -> Result<(W3dVertexMaterialStruct, VertexMapperConfig)> {
        let mut material = Self::default_vertex_material();
        let mapper = VertexMapperConfig::default();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_VERTEX_MATERIAL_INFO => {
                    material = self.parse_vertex_material_info_chunk(chunk_data)?;
                }
                W3D_CHUNK_VERTEX_MATERIAL_NAME
                | W3D_CHUNK_VERTEX_MAPPER_ARGS0
                | W3D_CHUNK_VERTEX_MAPPER_ARGS1 => {}
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok((material, mapper))
    }

    pub(super) fn parse_vertex_materials_chunk(
        &self,
        data: &[u8],
    ) -> Result<(Vec<W3dVertexMaterialStruct>, Vec<VertexMapperConfig>)> {
        let mut materials = Vec::new();
        let mut mappers = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            if chunk_type == W3D_CHUNK_VERTEX_MATERIAL {
                let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
                let (material, mapper) = self.parse_single_vertex_material_chunk(chunk_data)?;
                materials.push(material);
                mappers.push(mapper);
            }

            offset += 8 + chunk_size;
        }

        Ok((materials, mappers))
    }

    /// Parse a W3D mesh chunk
    pub(super) fn parse_mesh_chunk(&self, data: &[u8]) -> Result<W3DMesh> {
        debug!("parse_mesh_chunk called, data size: {} bytes", data.len());
        let mut mesh = W3DMesh::new("unknown_mesh".to_string());
        let mut offset = 0;
        let mut has_valid_mesh_header = false;

        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut texcoords: Vec<[f32; 2]> = Vec::new();
        let mut vertex_colors: Vec<[f32; 4]> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut expected_vertex_count: Option<u32> = None;
        let mut mesh_header_version: Option<u32> = None;
        // C++ `MeshGeometryClass::read_vertex_influences` writes its one
        // allocated link array on every occurrence. Retain only the last
        // complete chunk here for the same overwrite behavior; a short chunk
        // returns an error immediately, so Main fails the whole mesh closed
        // instead of retaining its partial trusted-data link array.
        let mut vertex_influences: Option<Vec<W3dVertInfStruct>> = None;
        let mut texture_names: Vec<String> = Vec::new(); // C++ MeshLoadContextClass texture array

        // Parse mesh sub-chunks with safety counter
        let mut mesh_chunk_counter = 0;
        pub(super) const MAX_MESH_CHUNKS: usize = 1000; // Safety limit for mesh chunks

        while offset + 8 <= data.len() && mesh_chunk_counter < MAX_MESH_CHUNKS {
            mesh_chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let _is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Mesh sub-chunk extends beyond mesh: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for mesh chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized mesh chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH_HEADER => {
                    debug!(
                        "Parsing mesh header (W3dMeshHeader3Struct), size: {}",
                        chunk_size
                    );
                    let header = self
                        .parse_mesh_header(chunk_data)
                        .map_err(|e| anyhow!("invalid mesh header in '{}': {}", mesh.name, e))?;
                    has_valid_mesh_header = true;
                    mesh.name = header.mesh_name;
                    mesh.container_name = header.container_name;
                    expected_vertex_count = Some(header.num_vertices);
                    mesh_header_version = Some(header.version);
                    debug!(
                        "Mesh name: '{}', expecting {} vertices, {} triangles",
                        mesh.name, header.num_vertices, header.num_triangles
                    );
                }
                W3D_CHUNK_VERTICES => {
                    vertices = self.parse_vertices_with_count(chunk_data, expected_vertex_count)?;
                    debug!("Parsed {} vertices", vertices.len());
                }
                W3D_CHUNK_VERTEX_NORMALS => {
                    normals = self.parse_normals(chunk_data)?;
                    debug!("Parsed {} normals", normals.len());
                }
                W3D_CHUNK_TEXCOORDS => {
                    texcoords = self.parse_texcoords(chunk_data)?;
                    debug!("Parsed {} texture coordinates", texcoords.len());
                }
                W3D_CHUNK_VERTEX_COLORS => {
                    vertex_colors = self.parse_vertex_colors(chunk_data)?;
                    debug!("Parsed {} vertex colors", vertex_colors.len());
                }
                W3D_CHUNK_VERTEX_INFLUENCES => {
                    vertex_influences = Some(
                        self.parse_vertex_influences_with_count(chunk_data, expected_vertex_count)?,
                    );
                    debug!(
                        "Parsed {} exact W3dVertInfStruct records",
                        vertex_influences
                            .as_ref()
                            .map_or(0, |influences| influences.len())
                    );
                }
                W3D_CHUNK_TRIANGLES => {
                    triangles = self.parse_triangles(chunk_data)?;
                    debug!("Parsed {} triangles", triangles.len());
                }
                W3D_CHUNK_MATERIAL_INFO => {
                    debug!("Parsing material info chunk, size: {}", chunk_size);
                    if let Ok(material) = self.parse_material_info(chunk_data) {
                        mesh.material = material;
                        debug!(
                            "Parsed material: {} (texture: {:?})",
                            mesh.material.name, mesh.material.texture_name
                        );
                    } else {
                        warn!("Failed to parse material info chunk");
                    }
                }
                W3D_CHUNK_MAP3_FILENAME => {
                    // Extract texture filename from MAP3_FILENAME chunk
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }
                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        mesh.material.texture_name = Some(filename);
                    }
                }
                W3D_CHUNK_VERTEX_SHADE_INDICES => {
                    // Shade indices for vertex coloring - skip for now
                    debug!(
                        "Skipping W3D_CHUNK_VERTEX_SHADE_INDICES ({} bytes)",
                        chunk_size
                    );
                }
                W3D_CHUNK_SHADERS => match self.parse_shaders_chunk(chunk_data) {
                    Ok(shaders) => {
                        debug!("Parsed {} shaders", shaders.len());
                        mesh.shaders = shaders;
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_SHADERS: {}", err);
                    }
                },
                W3D_CHUNK_VERTEX_MATERIALS => match self.parse_vertex_materials_chunk(chunk_data) {
                    Ok((materials, mappers)) => {
                        debug!(
                            "Parsed {} vertex materials and {} mapper configs",
                            materials.len(),
                            mappers.len()
                        );
                        mesh.vertex_materials = materials;
                        mesh.vertex_mappers = mappers;
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_VERTEX_MATERIALS: {}", err);
                    }
                },
                W3D_CHUNK_MATERIAL_PASS => match self.parse_material_pass_chunk(chunk_data) {
                    Ok(pass_data) => {
                        let mut stage_texture_names = Vec::new();
                        for texture_ids in &pass_data.stage_texture_ids {
                            let names = texture_ids
                                .iter()
                                .filter_map(|texture_id| {
                                    if *texture_id == u32::MAX {
                                        return None;
                                    }
                                    texture_names.get(*texture_id as usize).cloned()
                                })
                                .collect::<Vec<_>>();
                            stage_texture_names.push(names);
                        }

                        mesh.passes.push(MaterialPassInfo {
                            vm_id: pass_data.vertex_material_ids.first().copied().unwrap_or(0),
                            shader_id: pass_data.shader_ids.first().copied().unwrap_or(0),
                            texture_count: pass_data.stage_texture_ids.len() as u32,
                        });
                        mesh.per_pass_vertex_material_ids
                            .push(pass_data.vertex_material_ids.clone());
                        mesh.per_pass_shader_ids.push(pass_data.shader_ids.clone());
                        mesh.per_pass_dcg_colors.push(pass_data.dcg_colors.clone());
                        mesh.per_pass_dig_colors.push(pass_data.dig_colors.clone());
                        mesh.per_pass_stage_texture_ids
                            .push(pass_data.stage_texture_ids.clone());
                        mesh.per_pass_stage_texture_names.push(stage_texture_names);

                        for (stage_index, stage_uvs) in pass_data.stage_texcoords.iter().enumerate()
                        {
                            mesh.stage_texcoords.push(stage_uvs.clone());
                            mesh.per_stage_face_texcoord_ids.push(
                                pass_data
                                    .stage_per_face_texcoord_ids
                                    .get(stage_index)
                                    .cloned()
                                    .unwrap_or_default(),
                            );
                        }
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_MATERIAL_PASS: {}", err);
                    }
                },
                W3D_CHUNK_TEXTURES => {
                    // Parse textures container - C++ read_textures() equivalent
                    debug!(
                        "Found W3D_CHUNK_TEXTURES inside mesh, size: {} bytes",
                        chunk_size
                    );
                    // Parse texture names from W3D_CHUNK_TEXTURE/W3D_CHUNK_TEXTURE_NAME
                    if let Ok(names) = self.parse_textures_chunk_into_array(chunk_data) {
                        debug!("Loaded {} texture(s) for mesh: {:?}", names.len(), names);
                        texture_names.extend(names);
                    }
                }
                _ => {
                    debug!("Unknown mesh sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        if mesh_chunk_counter >= MAX_MESH_CHUNKS {
            warn!(
                "⚠️  Mesh chunk parsing hit safety limit ({} chunks)",
                MAX_MESH_CHUNKS
            );
        }

        if !has_valid_mesh_header {
            return Err(anyhow!("mesh chunk missing required W3D mesh header"));
        }

        let stage0_fallback_texcoords = texcoords.clone();

        // Build final mesh (logging disabled)
        self.build_mesh_from_data(
            &mut mesh,
            vertices,
            normals,
            texcoords,
            vertex_colors,
            triangles,
        )?;

        if let Some(mut influences) = vertex_influences {
            let expected_count = expected_vertex_count
                .and_then(|count| usize::try_from(count).ok())
                .ok_or_else(|| anyhow!("vertex influences require a valid mesh header"))?;
            if influences.len() != expected_count || influences.len() != mesh.vertices.len() {
                return Err(anyhow!(
                    "mesh '{}' vertex influences do not match its exact vertex count",
                    mesh.name
                ));
            }

            // `MeshModelClass::Load_W3D` adjusts only successfully loaded
            // pre-3.0 skin links after all mesh chunks have been read. C++
            // stores those links as uint16, so the increment wraps at u16::MAX.
            if mesh_header_version.is_some_and(|version| version < W3D_HTREE_ROOT_VERSION) {
                for influence in &mut influences {
                    influence.bone_idx = influence.bone_idx.wrapping_add(1);
                }
            }
            mesh.vertex_influences = Some(influences);
        }

        if !texture_names.is_empty() {
            mesh.texture_library = texture_names.clone();
        }

        if mesh.stage_texcoords.is_empty() && !stage0_fallback_texcoords.is_empty() {
            mesh.stage_texcoords.push(stage0_fallback_texcoords);
            mesh.stage_uv_channels = vec![0];
            if mesh.per_stage_face_texcoord_ids.is_empty() {
                mesh.per_stage_face_texcoord_ids.push(Vec::new());
            }
        } else if !mesh.stage_texcoords.is_empty() {
            let (unique_layers, stage_channels) =
                deduplicate_stage_uv_layers(mesh.stage_texcoords.clone());
            mesh.stage_texcoords = unique_layers;
            mesh.stage_uv_channels = stage_channels;
            if mesh.per_stage_face_texcoord_ids.is_empty() {
                mesh.per_stage_face_texcoord_ids = vec![Vec::new(); mesh.stage_texcoords.len()];
            }
        }

        if !mesh.per_pass_stage_texture_ids.is_empty() {
            let mut per_pass_names = Vec::with_capacity(mesh.per_pass_stage_texture_ids.len());
            for stage_set in &mesh.per_pass_stage_texture_ids {
                let mut stage_names = Vec::with_capacity(stage_set.len());
                for ids in stage_set {
                    let names = ids
                        .iter()
                        .filter_map(|texture_id| {
                            if *texture_id == u32::MAX {
                                None
                            } else {
                                mesh.texture_name_from_library(*texture_id)
                                    .map(|name| name.to_string())
                            }
                        })
                        .collect::<Vec<_>>();
                    stage_names.push(names);
                }
                per_pass_names.push(stage_names);
            }
            mesh.per_pass_stage_texture_names = per_pass_names;
        }

        // C++ behavior: single-material fallback uses first texture if pass data does not bind one.
        if mesh.material.texture_name.is_none() && !texture_names.is_empty() {
            mesh.material.texture_name = Some(texture_names[0].clone());
        }
        if mesh.material.texture_name.is_none() {
            mesh.material.texture_name = Self::stage_texture_from_mesh(&mesh, 0, 0);
        }

        if let Some(texture_name) = &mesh.material.texture_name {
            debug!("Mesh '{}' will use texture: '{}'", mesh.name, texture_name);
        }

        // Map W3D shader blend factors to material blend_mode for C++ parity.
        // Uses the first shader, or the shader referenced by the first material pass.
        let shader_idx = mesh
            .passes
            .first()
            .map(|p| p.shader_id as usize)
            .unwrap_or(0);
        if let Some(shader) = mesh.shaders.get(shader_idx) {
            let (mode, alpha_test) =
                shader_blend_to_mode(shader.src_blend, shader.dest_blend, shader.alpha_test);
            mesh.material.blend_mode = mode;
            mesh.material.alpha_test_enabled = alpha_test;
            debug!(
                "Mesh '{}' blend_mode={:?}, alpha_test={} (src={}, dest={})",
                mesh.name,
                mesh.material.blend_mode,
                mesh.material.alpha_test_enabled,
                shader.src_blend,
                shader.dest_blend
            );
        }

        Ok(mesh)
    }

    /// Parse mesh header - C++ compatible W3dMeshHeader3Struct format
    pub(super) fn parse_mesh_header(&self, data: &[u8]) -> Result<MeshHeader> {
        // W3dMeshHeader3Struct layout:
        // 0: uint32 Version
        // 4: uint32 Attributes
        // 8: char MeshName[16]
        // 24: char ContainerName[16]
        // 40: uint32 NumTris
        // 44: uint32 NumVertices
        // 48: uint32 NumMaterials
        // 52: uint32 NumDamageStages
        // 56: sint32 SortLevel
        // 60: uint32 PrelitVersion
        // 64: uint32 FutureCounts[1]
        // 68: uint32 VertexChannels
        // 72: uint32 FaceChannels
        // Plus bounding box, sphere data...

        if data.len() < 76 {
            // Minimum size for core header fields
            return Err(anyhow!("Mesh header too small: {} bytes", data.len()));
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let attributes = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let num_triangles = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        let num_vertices = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);

        // Extract mesh name (null-terminated string at offset 8, max 16 chars)
        let mut mesh_name = String::new();
        for i in 8..24 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            mesh_name.push(data[i] as char);
        }

        // Extract container name (null-terminated string at offset 24, max 16 chars)
        let mut container_name = String::new();
        for i in 24..40 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            container_name.push(data[i] as char);
        }

        debug!(
            "Mesh header - version: 0x{:08X}, attributes: 0x{:08X}, triangles: {}, vertices: {}, mesh_name: '{}', container: '{}'",
            version, attributes, num_triangles, num_vertices, mesh_name, container_name
        );

        Ok(MeshHeader {
            version,
            flags: attributes, // attributes field is what was called flags in the old structure
            num_triangles,
            num_vertices,
            mesh_name: if mesh_name.is_empty() {
                "unnamed_mesh".to_string()
            } else {
                mesh_name
            },
            container_name,
        })
    }

    /// Parse one exact source `W3dVertInfStruct` per declared mesh vertex.
    ///
    /// `MeshGeometryClass::read_vertex_influences` reads `sizeof` bytes for
    /// every `Get_Vertex_Count()` entry and returns `WW3D_ERROR_LOAD_FAILED`
    /// on the first short read. It does not validate or reinterpret the pad
    /// bytes, and `Close_Chunk` discards any remaining trailing data. Keeping
    /// that shape matters: an arbitrary extra byte is not a malformed record,
    /// while even one missing byte from the required records makes the C++
    /// reader fail. Main rejects that mesh rather than leaving a partial skin
    /// array behind.
    pub(super) fn parse_vertex_influences_with_count(
        &self,
        data: &[u8],
        expected_count: Option<u32>,
    ) -> Result<Vec<W3dVertInfStruct>> {
        let vertex_count = usize::try_from(
            expected_count.ok_or_else(|| anyhow!("vertex influences precede mesh header"))?,
        )
        .map_err(|_| anyhow!("mesh vertex count does not fit usize"))?;
        let required_size = vertex_count
            .checked_mul(W3D_VERTEX_INFLUENCE_RECORD_SIZE)
            .ok_or_else(|| anyhow!("vertex influence byte count overflow"))?;
        if data.len() < required_size {
            return Err(anyhow!(
                "insufficient vertex influence data: need {} bytes, have {} (for {} vertices)",
                required_size,
                data.len(),
                vertex_count
            ));
        }

        let mut influences = Vec::with_capacity(vertex_count);
        for record in data[..required_size].chunks_exact(W3D_VERTEX_INFLUENCE_RECORD_SIZE) {
            let bone_idx = u16::from_le_bytes([record[0], record[1]]);
            let mut pad = [0u8; 6];
            pad.copy_from_slice(&record[2..W3D_VERTEX_INFLUENCE_RECORD_SIZE]);
            influences.push(W3dVertInfStruct { bone_idx, pad });
        }
        Ok(influences)
    }

    /// Parse vertices array with expected count validation - C++ compatible version
    pub(super) fn parse_vertices_with_count(
        &self,
        data: &[u8],
        expected_count: Option<u32>,
    ) -> Result<Vec<[f32; 3]>> {
        // In C++: reads vertex count from mesh header, then reads that many W3dVectorStruct (12 bytes each)
        // No headers or padding in vertex chunk data itself - just raw vertex data

        let vertex_count = if let Some(expected) = expected_count {
            expected as usize
        } else {
            // Fallback: assume data contains only vertices (12 bytes each)
            data.len() / 12
        };

        debug!(
            "parse_vertices_with_count: data.len()={}, expected_count={:?}, using vertex_count={}",
            data.len(),
            expected_count,
            vertex_count
        );

        // Verify we have enough data for the expected vertices
        let required_size = vertex_count * 12; // 12 bytes per W3dVectorStruct
        if data.len() < required_size {
            return Err(anyhow!(
                "Insufficient vertex data: need {} bytes, have {} (for {} vertices)",
                required_size,
                data.len(),
                vertex_count
            ));
        }

        let mut vertices = Vec::with_capacity(vertex_count);

        // Read vertices directly as W3dVectorStruct (float32 X, Y, Z)
        for i in 0..vertex_count {
            let offset = i * 12;
            if offset + 12 > data.len() {
                warn!(
                    "Vertex {} would exceed data bounds, stopping at {} vertices",
                    i,
                    vertices.len()
                );
                break;
            }

            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Validate vertices are reasonable (not NaN, not infinite)
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                warn!(
                    "Vertex {} has non-finite coordinates: ({}, {}, {})",
                    i, x, y, z
                );
                continue;
            }

            vertices.push([x, y, z]);

            // Log first few vertices for debugging
            if i < 3 {
                debug!("Vertex {}: ({:.3}, {:.3}, {:.3})", i, x, y, z);
            }
        }

        if vertices.is_empty() {
            return Err(anyhow!("No valid vertices parsed from data"));
        }

        debug!("Successfully parsed {} vertices", vertices.len());
        Ok(vertices)
    }

    /// Legacy parse vertices for backward compatibility
    pub(super) fn parse_vertices(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        self.parse_vertices_with_count(data, None)
    }

    /// Parse normals array
    pub(super) fn parse_normals(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        if !data.len().is_multiple_of(12) {
            return Err(anyhow!("Invalid normals data size: {}", data.len()));
        }

        let normal_count = data.len() / 12;
        let mut normals = Vec::with_capacity(normal_count);

        for i in 0..normal_count {
            let offset = i * 12;
            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);
            normals.push([x, y, z]);
        }

        Ok(normals)
    }

    /// Parse texture coordinates array
    pub(super) fn parse_texcoords(&self, data: &[u8]) -> Result<Vec<[f32; 2]>> {
        if !data.len().is_multiple_of(8) {
            return Err(anyhow!("Invalid texcoords data size: {}", data.len()));
        }

        let texcoord_count = data.len() / 8;
        let mut texcoords = Vec::with_capacity(texcoord_count);

        for i in 0..texcoord_count {
            let offset = i * 8;
            let u = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            // C++ parity: WW3D stores V upside-down in chunk payload and flips on load.
            texcoords.push([u, 1.0 - v]);
        }

        Ok(texcoords)
    }

    /// Parse vertex colors array
    pub(super) fn parse_vertex_colors(&self, data: &[u8]) -> Result<Vec<[f32; 4]>> {
        let mut colors = Vec::new();

        if data.len().is_multiple_of(3) {
            let color_count = data.len() / 3;
            colors.reserve(color_count);
            for i in 0..color_count {
                let offset = i * 3;
                colors.push([
                    data[offset] as f32 / 255.0,
                    data[offset + 1] as f32 / 255.0,
                    data[offset + 2] as f32 / 255.0,
                    1.0,
                ]);
            }
            return Ok(colors);
        }

        if data.len().is_multiple_of(4) {
            let color_count = data.len() / 4;
            colors.reserve(color_count);
            for i in 0..color_count {
                let offset = i * 4;
                colors.push([
                    data[offset] as f32 / 255.0,
                    data[offset + 1] as f32 / 255.0,
                    data[offset + 2] as f32 / 255.0,
                    data[offset + 3] as f32 / 255.0,
                ]);
            }
            return Ok(colors);
        }

        Err(anyhow!("Invalid vertex colors data size: {}", data.len()))
    }

    /// Parse material info
    pub(super) fn parse_material_info(&self, data: &[u8]) -> Result<W3DMaterial> {
        if data.len() < 4 {
            // Need at least 4 bytes for basic parsing
            return Err(anyhow!("Material info chunk too small: {}", data.len()));
        }

        // Material info structure is complex, let's extract basic information
        let mut material = W3DMaterial::default();

        // For small material info chunks (16 bytes), extract basic properties
        // For larger chunks, try to extract more detailed information

        if data.len() >= 48 {
            // Extract C++ VertexMaterialClass compatible color values for larger chunks
            let diffuse_r = f32::from_le_bytes(data[32..36].try_into().unwrap_or([0; 4]));
            let diffuse_g = f32::from_le_bytes(data[36..40].try_into().unwrap_or([0; 4]));
            let diffuse_b = f32::from_le_bytes(data[40..44].try_into().unwrap_or([0; 4]));

            if diffuse_r.is_finite() && diffuse_g.is_finite() && diffuse_b.is_finite() {
                material.diffuse_color = Vec3::new(diffuse_r, diffuse_g, diffuse_b);
            }
        }

        if data.len() >= 32 {
            // Try to extract material name for larger chunks
            let mut name = String::new();
            for i in 0..std::cmp::min(32, data.len()) {
                if data[i] == 0 {
                    break;
                }
                if data[i].is_ascii() && data[i] >= 32 {
                    name.push(data[i] as char);
                }
            }
            if !name.is_empty() {
                material.name = name;
            }
        } else if data.len() >= 16 {
            // For small material info chunks (16 bytes), extract basic properties
            debug!("Parsing 16-byte material info chunk - basic material properties");

            // Try to extract some basic properties from the first few bytes
            // Material index or type might be at the beginning
            let material_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            debug!("Material type/index: 0x{:08X}", material_type);

            // Set basic properties for small chunks
            material.name = format!("material_{:08X}", material_type);
            material.diffuse_color = Vec3::new(0.8, 0.8, 0.8); // Default gray
        }

        // Note: Texture names are now loaded separately from W3D_CHUNK_TEXTURES
        // They will be associated with materials through material passes

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        // Set texture name in stage 0 if found
        if let Some(ref texture_name) = material.texture_name {
            material.stage0_mapping.texture_name = Some(texture_name.clone());
        }

        debug!(
            "Parsed material: name='{}', diffuse={:?}, texture={:?}",
            material.name, material.diffuse_color, material.texture_name
        );

        Ok(material)
    }

    /// Parse W3D textures container chunk - contains individual texture definitions
    pub(super) fn parse_textures_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut texture_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid texture chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE => {
                    debug!("Parsing individual texture chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                            debug!("Found texture: {}", texture_name);
                            model.texture_names.push(texture_name);
                            texture_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Loaded {} textures from W3D_CHUNK_TEXTURES", texture_count);
        Ok(())
    }

    /// Parse W3D_CHUNK_TEXTURES and return array of texture names - C++ read_textures() equivalent
    pub(super) fn parse_textures_chunk_into_array(&self, data: &[u8]) -> Result<Vec<String>> {
        debug!("parse_textures_chunk_into_array: data.len()={}", data.len());
        let mut textures = Vec::new();
        let mut offset = 0;

        // C++ code: for (TextureClass *newtex = ::Load_Texture(cload); newtex != NULL; newtex = ::Load_Texture(cload))
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Check for container chunk flag (bit 31 set on chunk size - C++ behavior)
            let is_container = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            // C++ Load_Texture checks for W3D_CHUNK_TEXTURE
            if chunk_type == W3D_CHUNK_TEXTURE && is_container {
                if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                    textures.push(texture_name);
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Returning {} textures", textures.len());
        Ok(textures)
    }

    /// Parse a single W3D_CHUNK_TEXTURE and extract the texture name
    pub(super) fn parse_single_texture_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;
        let mut texture_name: Option<String> = None;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE_NAME => {
                    // Read null-terminated string directly from chunk data
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        debug!("Found texture name in W3D_CHUNK_TEXTURE_NAME: {}", name);
                        texture_name = Some(name);
                    }
                }
                W3D_CHUNK_TEXTURE_INFO => {
                    debug!("Found W3D_CHUNK_TEXTURE_INFO (not parsing texture properties yet)");
                    // W3dTextureInfoStruct parsing can be added here later if needed
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk in W3D_CHUNK_TEXTURE: 0x{:08X}",
                        chunk_type
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        texture_name.ok_or_else(|| anyhow!("No texture name found in W3D_CHUNK_TEXTURE"))
    }

    /// Parse W3D MATERIALS3 container chunk - contains material definitions with texture filenames
    /// This matches the C++ approach: create materials and directly assign texture names
    pub(super) fn parse_materials3_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut material_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid materials3 chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3 => {
                    debug!("Parsing individual material3 chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        // Parse the complete material (name + properties + texture) like C++ does
                        if let Ok(material) = self.parse_complete_material3_chunk(chunk_data) {
                            debug!(
                                "Found material3: '{}' with texture: {:?}",
                                material.name, material.texture_name
                            );

                            // Store the material in the model's materials HashMap
                            model
                                .materials
                                .insert(material.name.clone(), material.clone());

                            // Also add texture name to the model's texture list for loading
                            if let Some(ref texture_name) = material.texture_name {
                                model.texture_names.push(texture_name.clone());
                            }
                            material_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown materials3 sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!(
            "Loaded {} complete materials from W3D_CHUNK_MATERIALS3",
            material_count
        );
        Ok(())
    }

    /// Parse a complete W3D_CHUNK_MATERIAL3 exactly like C++ does:
    /// 1. Read W3D_CHUNK_MATERIAL3_NAME
    /// 2. Read W3D_CHUNK_MATERIAL3_INFO (material properties)
    /// 3. Read W3D_CHUNK_MATERIAL3_DC_MAP -> W3D_CHUNK_MAP3_FILENAME (texture)
    pub(super) fn parse_complete_material3_chunk(&self, data: &[u8]) -> Result<W3DMaterial> {
        let mut offset = 0;
        let mut material = W3DMaterial::default();
        let mut material_name: Option<String> = None;

        // Parse chunks inside W3D_CHUNK_MATERIAL3 container like C++ does
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_NAME => {
                    // 0x0000002D
                    // Read material name exactly like C++: cload.Read(name,cload.Cur_Chunk_Length());
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        material_name = Some(name);
                        debug!("Found material3 name: {}", material_name.as_ref().unwrap());
                    }
                }
                W3D_CHUNK_MATERIAL3_INFO => {
                    // 0x0000002E
                    // Read W3dMaterial3Struct like C++: cload.Read(&mat,sizeof(W3dMaterial3Struct))
                    debug!("Parsing W3D_CHUNK_MATERIAL3_INFO, size: {}", chunk_size);
                    // For now, set basic material properties - we can expand this later
                    material.diffuse_color = Vec3::new(0.8, 0.8, 0.8);
                    material.specular_color = Vec3::new(0.2, 0.2, 0.2);
                    material.shininess = 16.0;
                    material.opacity = 1.0;
                }
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    // 0x0000002F - Diffuse Color Map
                    debug!(
                        "Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename like C++"
                    );
                    let _is_container_chunk = (chunk_type & 0x80000000) != 0 || chunk_size > 256; // DC_MAP is a container

                    if let Ok(texture_filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                        debug!(
                            "C++ style: Found texture filename from DC_MAP: {}",
                            texture_filename
                        );
                        material.texture_name = Some(texture_filename);
                        material.stage0_mapping.texture_name = material.texture_name.clone();
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        // Set material name like C++: vmat->Set_Name(name);
        if let Some(name) = material_name {
            material.name = name;
        } else {
            material.name = "unnamed_material3".to_string();
        }

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        debug!(
            "Completed material3 parsing: '{}' with texture: {:?}",
            material.name, material.texture_name
        );

        Ok(material)
    }

    /// Parse a single W3D_CHUNK_MATERIAL3 and extract texture filenames from DC_MAP chunks
    pub(super) fn parse_single_material3_chunk(&self, data: &[u8]) -> Result<Vec<String>> {
        let mut offset = 0;
        let mut texture_names: Vec<String> = Vec::new();

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            let is_container_chunk = (chunk_type & 0x80000000) != 0;
            let chunk_type = chunk_type & 0x7FFFFFFF;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    debug!("Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename");
                    if is_container_chunk {
                        // Parse the DC_MAP container to find the filename
                        if let Ok(filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                            debug!("Found texture filename from material3 DC_MAP: {}", filename);
                            texture_names.push(filename);
                        }
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Ok(texture_names)
    }

    /// Parse W3D_CHUNK_MATERIAL3_DC_MAP to extract texture filename
    pub(super) fn parse_material3_dc_map_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MAP3_FILENAME => {
                    // 0x00000030
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }

                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        return Ok(filename);
                    }
                }
                _ => {
                    debug!("Unknown DC_MAP sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Err(anyhow!(
            "No texture filename found in W3D_CHUNK_MATERIAL3_DC_MAP"
        ))
    }

    /// Parse triangles array - C++ compatible W3dTriStruct format
    pub(super) fn parse_triangles(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        // W3dTriStruct format: 3 x uint32 vertex indices, uint32 attributes, W3dVectorStruct normal, float32 distance
        // Total size: 3*4 + 4 + 3*4 + 4 = 32 bytes per triangle
        pub(super) const TRI_STRUCT_SIZE: usize = 32;

        if !data.len().is_multiple_of(TRI_STRUCT_SIZE) {
            return Err(anyhow!(
                "Invalid triangles data size: {} (expected multiple of {})",
                data.len(),
                TRI_STRUCT_SIZE
            ));
        }

        let triangle_count = data.len() / TRI_STRUCT_SIZE;
        let mut triangles = Vec::with_capacity(triangle_count);

        for i in 0..triangle_count {
            let offset = i * TRI_STRUCT_SIZE;

            // Read the 3 vertex indices (first 12 bytes of W3dTriStruct)
            let v0 = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v1 = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let v2 = u32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Skip attributes (4 bytes), normal (12 bytes), and distance (4 bytes) for now
            // We only need the vertex indices for basic rendering

            triangles.push([v0, v1, v2]);

            // Log first few triangles for debugging
            if i < 3 {
                debug!("Triangle {}: [{}, {}, {}]", i, v0, v1, v2);
            }
        }

        debug!("Successfully parsed {} triangles", triangles.len());
        Ok(triangles)
    }

    /// Build final mesh from parsed data
    pub(super) fn build_mesh_from_data(
        &self,
        mesh: &mut W3DMesh,
        vertices: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        texcoords: Vec<[f32; 2]>,
        vertex_colors: Vec<[f32; 4]>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<()> {
        if vertices.is_empty() {
            return Err(anyhow!("No vertices in mesh"));
        }

        let vertex_count = vertices.len();
        mesh.vertices.clear();
        mesh.vertices.reserve(vertex_count);
        mesh.indices.clear();

        // Build vertices with available data
        for i in 0..vertex_count {
            let position = w3d_position_to_world(vertices[i]);
            let normal = if i < normals.len() {
                w3d_normal_to_world(normals[i])
            } else {
                [0.0, 1.0, 0.0]
            };
            let uv = if i < texcoords.len() {
                texcoords[i]
            } else {
                [0.0, 0.0]
            };
            let color = if i < vertex_colors.len() {
                vertex_colors[i]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            mesh.vertices.push(W3DVertex {
                position,
                normal,
                uv,
                color,
            });
        }
        mesh.vertices_in_render_space = true;
        mesh.has_explicit_vertex_colors = !vertex_colors.is_empty();

        // Build indices from triangles
        for triangle in triangles {
            if triangle[0] < vertex_count as u32
                && triangle[1] < vertex_count as u32
                && triangle[2] < vertex_count as u32
            {
                push_world_space_triangle(&mut mesh.indices, triangle[0], triangle[1], triangle[2]);
            }
        }

        // C++ parity: never synthesize triangle lists when triangle chunks are missing/invalid.
        if mesh.indices.is_empty() {
            return Err(anyhow!("mesh '{}' has no valid triangles", mesh.name));
        }

        debug!(
            "Built mesh with {} vertices and {} indices",
            mesh.vertices.len(),
            mesh.indices.len()
        );
        Ok(())
    }

    /// Load C&C model by exact asset name.
    pub async fn load_cnc_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        unit_name: &str,
    ) -> Result<W3DModel> {
        self.load_model(archive_system, unit_name).await
    }

    /// List available W3D models in archives
    pub fn list_available_models(&self, archive_system: &ArchiveFileSystem) -> Vec<String> {
        let mut models = Vec::new();
        let all_files = archive_system.list_all_files();

        for file in all_files {
            if file.to_lowercase().ends_with(".w3d") {
                models.push(file);
            }
        }

        models.sort();
        models
    }
}

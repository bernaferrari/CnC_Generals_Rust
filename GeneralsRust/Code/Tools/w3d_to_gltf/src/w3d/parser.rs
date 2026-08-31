//! W3D binary parser using authoritative C++ layout (w3d_file.h)

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read, Seek, SeekFrom};

use crate::w3d::chunks::{W3D_NAME_LEN, W3dChunkHeader as HeaderLite, W3dChunkType};
use crate::w3d::error::W3dResult;
use crate::w3d::structs::*;

fn read_w3d_name<R: Read>(r: &mut R) -> W3dResult<[u8; W3D_NAME_LEN]> {
    let mut buf = [0u8; W3D_NAME_LEN];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_vector<R: Read>(r: &mut R) -> W3dResult<W3dVector> {
    Ok(W3dVector {
        x: r.read_f32::<LittleEndian>()?,
        y: r.read_f32::<LittleEndian>()?,
        z: r.read_f32::<LittleEndian>()?,
    })
}

fn read_quat<R: Read>(r: &mut R) -> W3dResult<W3dQuaternion> {
    Ok(W3dQuaternion {
        q: [
            r.read_f32::<LittleEndian>()?,
            r.read_f32::<LittleEndian>()?,
            r.read_f32::<LittleEndian>()?,
            r.read_f32::<LittleEndian>()?,
        ],
    })
}

fn read_chunk_header<R: Read>(r: &mut R) -> W3dResult<HeaderLite> {
    let raw_id = r.read_u32::<LittleEndian>()?;
    let size = r.read_u32::<LittleEndian>()?;
    Ok(HeaderLite {
        chunk_type: W3dChunkType::from(raw_id),
        chunk_size: size,
    })
}

pub fn parse_w3d_file(data: &[u8]) -> W3dResult<W3dFile> {
    let mut cursor = Cursor::new(data);
    let mut result = W3dFile::default();

    while (cursor.position() as usize) + 8 <= data.len() {
        let start = cursor.position();
        let header = match read_chunk_header(&mut cursor) {
            Ok(h) => h,
            Err(_) => break,
        };
        let end = start + 8 + (header.chunk_size & 0x7FFF_FFFF) as u64;

        match header.chunk_type {
            W3dChunkType::Mesh => {
                let mesh = parse_mesh_chunk(&mut cursor, (header.chunk_size & 0x7FFF_FFFF) as u32)?;
                result.meshes.push(mesh);
            }
            W3dChunkType::Hierarchy => {
                let h =
                    parse_hierarchy_chunk(&mut cursor, (header.chunk_size & 0x7FFF_FFFF) as u32)?;
                result.hierarchies.push(h);
            }
            W3dChunkType::Animation => {
                let a =
                    parse_animation_chunk(&mut cursor, (header.chunk_size & 0x7FFF_FFFF) as u32)?;
                result.animations.push(a);
            }
            W3dChunkType::MorphAnimation => {
                let a = parse_morph_animation_chunk(
                    &mut cursor,
                    (header.chunk_size & 0x7FFF_FFFF) as u32,
                )?;
                result.animations.push(a);
            }
            W3dChunkType::CompressedAnimation => {
                let a = parse_compressed_animation_chunk(
                    &mut cursor,
                    (header.chunk_size & 0x7FFF_FFFF) as u32,
                )?;
                result.animations.push(a);
            }
            W3dChunkType::Light => {
                let l = parse_light_chunk(&mut cursor, (header.chunk_size & 0x7FFF_FFFF) as u32)?;
                result.lights.push(l);
            }
            _ => {
                // skip unknown
                cursor.seek(SeekFrom::Start(end))?;
                continue;
            }
        }

        cursor.seek(SeekFrom::Start(end))?;
    }

    Ok(result)
}

fn parse_mesh_chunk<R: Read + Seek>(r: &mut R, size: u32) -> W3dResult<W3dMesh> {
    let start = current_pos(r)?;
    let mut mesh = W3dMesh::default();

    while current_pos(r)? < start + size as u64 {
        let sub = read_chunk_header(r)?;
        let sub_start = current_pos(r)?;
        let sub_end = sub_start + (sub.chunk_size & 0x7FFF_FFFF) as u64;
        match sub.chunk_type {
            W3dChunkType::VertexInfluences => {
                // one influence per vertex (BoneIdx u16 + 6 pad bytes) per spec; if larger, read only count vertices
                let count = mesh.header.num_vertices as usize;
                let bytes_needed = count * (2 + 6);
                let read_count = if (sub.chunk_size & 0x7FFF_FFFF) as usize >= bytes_needed {
                    count
                } else {
                    ((sub.chunk_size & 0x7FFF_FFFF) as usize) / 8
                };
                mesh.vertex_influences.reserve(read_count);
                for _ in 0..read_count {
                    let bone_idx = r.read_u16::<LittleEndian>()?;
                    let mut pad = [0u8; 6];
                    r.read_exact(&mut pad)?;
                    mesh.vertex_influences
                        .push(W3dVertexInfluence { bone_idx, pad });
                }
            }
            W3dChunkType::Shaders => {
                // read array of W3dShader until exhausted
                let shader_size = 16; // bytes in W3dShader (from struct: 16 u8)
                let count = ((sub.chunk_size & 0x7FFF_FFFF) as usize) / shader_size;
                for _ in 0..count {
                    let mut bytes = [0u8; 16];
                    r.read_exact(&mut bytes)?;
                    let s = W3dShader {
                        depth_compare: bytes[0],
                        depth_mask: bytes[1],
                        color_mask: bytes[2],
                        dest_blend: bytes[3],
                        fog_func: bytes[4],
                        pri_gradient: bytes[5],
                        sec_gradient: bytes[6],
                        src_blend: bytes[7],
                        texturing: bytes[8],
                        detail_color_func: bytes[9],
                        detail_alpha_func: bytes[10],
                        shader_preset: bytes[11],
                        alpha_test: bytes[12],
                        post_detail_color_func: bytes[13],
                        post_detail_alpha_func: bytes[14],
                        pad: bytes[15],
                    };
                    mesh.shaders.push(s);
                }
            }
            W3dChunkType::VertexMaterials => {
                // multiple VertexMaterial entries
                while current_pos(r)? < sub_end {
                    let vm_hdr = read_chunk_header(r)?;
                    let vm_start = current_pos(r)?;
                    let vm_end = vm_start + (vm_hdr.chunk_size & 0x7FFF_FFFF) as u64;
                    if let W3dChunkType::VertexMaterial = vm_hdr.chunk_type {
                        let mut vmat = W3dVertexMaterial::default();
                        while current_pos(r)? < vm_end {
                            let fld = read_chunk_header(r)?;
                            let fld_start = current_pos(r)?;
                            let fld_end = fld_start + (fld.chunk_size & 0x7FFF_FFFF) as u64;
                            match fld.chunk_type {
                                W3dChunkType::VertexMaterialName => {
                                    let len = (fld.chunk_size & 0x7FFF_FFFF) as usize;
                                    let mut buf = vec![0u8; len];
                                    r.read_exact(&mut buf)?;
                                    // strip trailing NULs
                                    if let Some(pos) = buf.iter().position(|&b| b == 0) {
                                        buf.truncate(pos);
                                    }
                                    vmat.name = String::from_utf8_lossy(&buf).to_string();
                                }
                                W3dChunkType::VertexMaterialInfo => {
                                    // read fields in order
                                    vmat.info.attributes = r.read_u32::<LittleEndian>()?;
                                    vmat.info.ambient = read_rgb(r)?;
                                    vmat.info.diffuse = read_rgb(r)?;
                                    vmat.info.specular = read_rgb(r)?;
                                    vmat.info.emissive = read_rgb(r)?;
                                    vmat.info.shininess = r.read_f32::<LittleEndian>()?;
                                    vmat.info.opacity = r.read_f32::<LittleEndian>()?;
                                    vmat.info.translucency = r.read_f32::<LittleEndian>()?;
                                }
                                _ => {}
                            }
                            r.seek(SeekFrom::Start(fld_end))?;
                        }
                        mesh.vertex_materials.push(vmat);
                    }
                    r.seek(SeekFrom::Start(vm_end))?;
                }
            }
            W3dChunkType::Textures => {
                // multiple Texture entries
                while current_pos(r)? < sub_end {
                    let tx_hdr = read_chunk_header(r)?;
                    let tx_start = current_pos(r)?;
                    let tx_end = tx_start + (tx_hdr.chunk_size & 0x7FFF_FFFF) as u64;
                    if let W3dChunkType::Texture = tx_hdr.chunk_type {
                        let mut tex = W3dTexture::default();
                        while current_pos(r)? < tx_end {
                            let fld = read_chunk_header(r)?;
                            let fld_start = current_pos(r)?;
                            let fld_end = fld_start + (fld.chunk_size & 0x7FFF_FFFF) as u64;
                            match fld.chunk_type {
                                W3dChunkType::TextureName => {
                                    let len = (fld.chunk_size & 0x7FFF_FFFF) as usize;
                                    let mut buf = vec![0u8; len];
                                    r.read_exact(&mut buf)?;
                                    if let Some(pos) = buf.iter().position(|&b| b == 0) {
                                        buf.truncate(pos);
                                    }
                                    tex.name = String::from_utf8_lossy(&buf).to_string();
                                }
                                W3dChunkType::TextureInfo => {
                                    tex.info.attributes = r.read_u16::<LittleEndian>()?;
                                    tex.info.animation_type = r.read_u16::<LittleEndian>()?;
                                    tex.info.frame_count = r.read_u32::<LittleEndian>()?;
                                    tex.info.frame_rate = r.read_f32::<LittleEndian>()?;
                                }
                                _ => {}
                            }
                            r.seek(SeekFrom::Start(fld_end))?;
                        }
                        mesh.textures.push(tex);
                    }
                    r.seek(SeekFrom::Start(tx_end))?;
                }
            }
            W3dChunkType::MaterialPass => {
                // Parse a pass: vertex material ids, shader ids, stages
                let mut pass = W3dMaterialPass::default();
                while current_pos(r)? < sub_end {
                    let fld = read_chunk_header(r)?;
                    let fld_start = current_pos(r)?;
                    let fld_end = fld_start + (fld.chunk_size & 0x7FFF_FFFF) as u64;
                    match fld.chunk_type {
                        W3dChunkType::VertexMaterialIds => {
                            let n = (fld.chunk_size & 0x7FFF_FFFF) / 4;
                            for _ in 0..n {
                                pass.vertex_material_ids.push(r.read_u32::<LittleEndian>()?);
                            }
                        }
                        W3dChunkType::ShaderIds => {
                            let n = (fld.chunk_size & 0x7FFF_FFFF) / 4;
                            for _ in 0..n {
                                pass.shader_ids.push(r.read_u32::<LittleEndian>()?);
                            }
                        }
                        W3dChunkType::TextureStage => {
                            // nested: already handled earlier, but now into pass
                            let mut stage = W3dTextureStage::default();
                            while current_pos(r)? < fld_end {
                                let st = read_chunk_header(r)?;
                                let st_start = current_pos(r)?;
                                let st_end = st_start + (st.chunk_size & 0x7FFF_FFFF) as u64;
                                match st.chunk_type {
                                    W3dChunkType::TextureIds => {
                                        let n = (st.chunk_size & 0x7FFF_FFFF) / 4;
                                        for _ in 0..n {
                                            stage.texture_ids.push(r.read_u32::<LittleEndian>()?);
                                        }
                                    }
                                    W3dChunkType::StageTexCoords => {
                                        let count = mesh.header.num_vertices as usize;
                                        for _ in 0..count {
                                            stage.tex_coords.push(W3dTexCoord {
                                                u: r.read_f32::<LittleEndian>()?,
                                                v: r.read_f32::<LittleEndian>()?,
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                                r.seek(SeekFrom::Start(st_end))?;
                            }
                            pass.texture_stages.push(stage);
                        }
                        _ => {}
                    }
                    r.seek(SeekFrom::Start(fld_end))?;
                }
                mesh.material_passes.push(pass);
            }
            W3dChunkType::MeshHeader3 => {
                mesh.header = read_mesh_header3(r)?;
            }
            W3dChunkType::Vertices => {
                let count = mesh.header.num_vertices as usize;
                mesh.vertices.reserve(count);
                for _ in 0..count {
                    mesh.vertices.push(read_vector(r)?);
                }
            }
            W3dChunkType::VertexNormals => {
                let count = mesh.header.num_vertices as usize;
                mesh.normals.reserve(count);
                for _ in 0..count {
                    mesh.normals.push(read_vector(r)?);
                }
            }
            W3dChunkType::Triangles => {
                let count = mesh.header.num_tris as usize;
                mesh.triangles.reserve(count);
                for _ in 0..count {
                    let v0 = r.read_u32::<LittleEndian>()?;
                    let v1 = r.read_u32::<LittleEndian>()?;
                    let v2 = r.read_u32::<LittleEndian>()?;
                    let attributes = r.read_u32::<LittleEndian>()?;
                    let normal = read_vector(r)?;
                    let dist = r.read_f32::<LittleEndian>()?;
                    mesh.triangles.push(W3dTriangle {
                        v_indices: [v0, v1, v2],
                        attributes,
                        normal,
                        dist,
                    });
                }
            }
            W3dChunkType::PerTriMaterials => {
                // Read one u32 per triangle referencing a material index
                let tri_count = mesh.header.num_tris as usize;
                let mut mats = Vec::with_capacity(tri_count);
                for _ in 0..tri_count {
                    mats.push(r.read_u32::<LittleEndian>()?);
                }
                mesh.per_tri_materials = Some(mats);
            }
            W3dChunkType::TextureStage => {
                // parse stage content: TextureIds, StageTexCoords
                let mut stage = W3dTextureStage::default();
                while current_pos(r)? < sub_end {
                    let st = read_chunk_header(r)?;
                    let st_start = current_pos(r)?;
                    let st_end = st_start + (st.chunk_size & 0x7FFF_FFFF) as u64;
                    match st.chunk_type {
                        W3dChunkType::TextureIds => {
                            let num = (st.chunk_size & 0x7FFF_FFFF) / 4;
                            for _ in 0..num {
                                stage.texture_ids.push(r.read_u32::<LittleEndian>()?);
                            }
                        }
                        W3dChunkType::StageTexCoords => {
                            let count = mesh.header.num_vertices as usize;
                            stage.tex_coords.reserve(count);
                            for _ in 0..count {
                                let u = r.read_f32::<LittleEndian>()?;
                                let v = r.read_f32::<LittleEndian>()?;
                                stage.tex_coords.push(W3dTexCoord { u, v });
                            }
                        }
                        _ => {}
                    }
                    r.seek(SeekFrom::Start(st_end))?;
                }
                if mesh.material_passes.is_empty() {
                    mesh.material_passes.push(W3dMaterialPass::default());
                }
                mesh.material_passes[0].texture_stages.push(stage);
            }
            // Optional AABTree for culling/collision
            crate::w3d::chunks::W3dChunkType::AABTree => {
                let mut tree = W3dAABTree {
                    header: W3dAABTreeHeader::default(),
                    poly_indices: Vec::new(),
                    nodes: Vec::new(),
                };
                while current_pos(r)? < sub_end {
                    let a = read_chunk_header(r)?;
                    let a_start = current_pos(r)?;
                    let a_end = a_start + (a.chunk_size & 0x7FFF_FFFF) as u64;
                    match a.chunk_type {
                        crate::w3d::chunks::W3dChunkType::AABTreeHeader => {
                            tree.header.node_count = r.read_u32::<LittleEndian>()?;
                            tree.header.poly_count = r.read_u32::<LittleEndian>()?;
                            for i in 0..6 {
                                tree.header.padding[i] = r.read_u32::<LittleEndian>()?;
                            }
                        }
                        crate::w3d::chunks::W3dChunkType::AABTreePolyIndices => {
                            let count = (a.chunk_size & 0x7FFF_FFFF) as usize / 4;
                            for _ in 0..count {
                                tree.poly_indices.push(r.read_u32::<LittleEndian>()?);
                            }
                        }
                        crate::w3d::chunks::W3dChunkType::AABTreeNodes => {
                            let count = (a.chunk_size & 0x7FFF_FFFF) as usize / 32;
                            for _ in 0..count {
                                let min = read_vector(r)?;
                                let max = read_vector(r)?;
                                let front_or_poly0 = r.read_u32::<LittleEndian>()?;
                                let back_or_poly_count = r.read_u32::<LittleEndian>()?;
                                tree.nodes.push(W3dAABTreeNode {
                                    min,
                                    max,
                                    front_or_poly0,
                                    back_or_poly_count,
                                });
                            }
                        }
                        _ => {}
                    }
                    r.seek(SeekFrom::Start(a_end))?;
                }
                mesh.aabtree = Some(tree);
            }
            W3dChunkType::Deform => {
                // Parse deform sets and convert to morph targets per keyframe (positions only)
                let mut local_targets: Vec<W3dMorphTarget> = Vec::new();
                while current_pos(r)? < sub_end {
                    let set_hdr = read_chunk_header(r)?;
                    let set_start = current_pos(r)?;
                    let set_end = set_start + (set_hdr.chunk_size & 0x7FFF_FFFF) as u64;
                    if let W3dChunkType::DeformSet = set_hdr.chunk_type {
                        // iterate keyframes in this set
                        while current_pos(r)? < set_end {
                            let k_hdr = read_chunk_header(r)?;
                            let k_start = current_pos(r)?;
                            let k_end = k_start + (k_hdr.chunk_size & 0x7FFF_FFFF) as u64;
                            if let W3dChunkType::DeformKeyframe = k_hdr.chunk_type {
                                // Deform keyframe info: float DeformPercent, u32 DataCount, u32[2] reserved
                                let deform_percent = r.read_f32::<LittleEndian>()?;
                                let _data_count = r.read_u32::<LittleEndian>()? as usize;
                                let _reserved0 = r.read_u32::<LittleEndian>()?;
                                let _reserved1 = r.read_u32::<LittleEndian>()?;
                                // Inside keyframe: multiple DeformData chunks
                                let mut deltas: Vec<[f32; 3]> =
                                    vec![[0.0; 3]; mesh.header.num_vertices as usize];
                                while current_pos(r)? < k_end {
                                    let d_hdr = read_chunk_header(r)?;
                                    let d_start = current_pos(r)?;
                                    let d_end = d_start + (d_hdr.chunk_size & 0x7FFF_FFFF) as u64;
                                    if let W3dChunkType::DeformData = d_hdr.chunk_type {
                                        // Per entry: u32 vertex index, W3dVector new position, RGBA (skip), reserved[2] (skip)
                                        // But spec indicates the DeformData chunk wraps multiple entries.
                                        // We'll read entries until we exhaust chunk size.
                                        let mut bytes_left =
                                            (d_hdr.chunk_size & 0x7FFF_FFFF) as usize;
                                        while bytes_left >= (4 + 4 * 3 + 4 + 8) {
                                            let vidx = r.read_u32::<LittleEndian>()? as usize;
                                            let new_pos = read_vector(r)?; // absolute position
                                            // read color RGBA
                                            let mut rgba = [0u8; 4];
                                            r.read_exact(&mut rgba)?;
                                            // skip reserved[2]
                                            let _res0 = r.read_u32::<LittleEndian>()?;
                                            let _res1 = r.read_u32::<LittleEndian>()?;
                                            // compute delta = new_pos - base vertex
                                            if vidx < deltas.len() && vidx < mesh.vertices.len() {
                                                let base = &mesh.vertices[vidx];
                                                deltas[vidx] = [
                                                    new_pos.x - base.x,
                                                    new_pos.y - base.y,
                                                    new_pos.z - base.z,
                                                ];
                                            }
                                            bytes_left -= 4 + 4 * 3 + 4 + 8;
                                        }
                                    }
                                    r.seek(SeekFrom::Start(d_end))?;
                                }
                                let name = format!("deform_{:.0}", deform_percent * 100.0);
                                local_targets.push(W3dMorphTarget {
                                    name,
                                    deltas,
                                    weight: deform_percent,
                                });
                            }
                            r.seek(SeekFrom::Start(k_end))?;
                        }
                    }
                    r.seek(SeekFrom::Start(set_end))?;
                }
                mesh.morph_targets.extend(local_targets);
            }
            _ => {
                // unhandled sub chunk, skip
            }
        }
        r.seek(SeekFrom::Start(sub_end))?;
    }

    Ok(mesh)
}

fn parse_light_chunk<R: Read + Seek>(r: &mut R, size: u32) -> W3dResult<W3dLight> {
    use crate::w3d::chunks::W3dChunkType::*;
    let start = current_pos(r)?;
    let mut light = W3dLight::default();
    while current_pos(r)? < start + size as u64 {
        let sub = read_chunk_header(r)?;
        let sub_start = current_pos(r)?;
        let sub_end = sub_start + (sub.chunk_size & 0x7FFF_FFFF) as u64;
        match sub.chunk_type {
            LightInfo => {
                let attrs = r.read_u32::<LittleEndian>()?;
                let _unused = r.read_u32::<LittleEndian>()?;
                let _ambient = read_rgb(r)?;
                let diffuse = read_rgb(r)?;
                light.color = diffuse;
                let _specular = read_rgb(r)?;
                light.intensity = r.read_f32::<LittleEndian>()?;
                light.kind = match attrs & 0xFF {
                    1 => W3dLightKind::Point,
                    2 => W3dLightKind::Directional,
                    3 => W3dLightKind::Spot,
                    _ => W3dLightKind::Point,
                };
            }
            SpotLightInfo => {
                light.direction = Some(W3dVector {
                    x: r.read_f32::<LittleEndian>()?,
                    y: r.read_f32::<LittleEndian>()?,
                    z: r.read_f32::<LittleEndian>()?,
                });
                light.spot_angle = Some(r.read_f32::<LittleEndian>()?);
                light.spot_exponent = Some(r.read_f32::<LittleEndian>()?);
            }
            LightTransform => {
                let mut m = [[0f32; 4]; 3];
                for i in 0..3 {
                    for j in 0..4 {
                        m[i][j] = r.read_f32::<LittleEndian>()?;
                    }
                }
                light.position = Some(W3dVector {
                    x: m[0][3],
                    y: m[1][3],
                    z: m[2][3],
                });
            }
            _ => {}
        }
        r.seek(SeekFrom::Start(sub_end))?;
    }
    Ok(light)
}

fn read_mesh_header3<R: Read>(r: &mut R) -> W3dResult<W3dMeshHeader3> {
    let version = r.read_u32::<LittleEndian>()?;
    let attributes = r.read_u32::<LittleEndian>()?;
    let mesh_name = read_w3d_name(r)?;
    let container_name = read_w3d_name(r)?;
    let num_tris = r.read_u32::<LittleEndian>()?;
    let num_vertices = r.read_u32::<LittleEndian>()?;
    let num_materials = r.read_u32::<LittleEndian>()?;
    let num_damage_stages = r.read_u32::<LittleEndian>()?;
    let sort_level = r.read_i32::<LittleEndian>()?;
    let prelit_version = r.read_u32::<LittleEndian>()?;
    let future_counts = [r.read_u32::<LittleEndian>()?];
    let vertex_channels = r.read_u32::<LittleEndian>()?;
    let face_channels = r.read_u32::<LittleEndian>()?;
    let min = read_vector(r)?;
    let max = read_vector(r)?;
    let sph_center = read_vector(r)?;
    let sph_radius = r.read_f32::<LittleEndian>()?;

    Ok(W3dMeshHeader3 {
        version,
        attributes,
        mesh_name,
        container_name,
        num_tris,
        num_vertices,
        num_materials,
        num_damage_stages,
        sort_level,
        prelit_version,
        future_counts,
        vertex_channels,
        face_channels,
        min,
        max,
        sph_center,
        sph_radius,
    })
}

fn parse_hierarchy_chunk<R: Read + Seek>(r: &mut R, size: u32) -> W3dResult<W3dHierarchy> {
    let start = current_pos(r)?;
    let mut h = W3dHierarchy::default();

    while current_pos(r)? < start + size as u64 {
        let sub = read_chunk_header(r)?;
        let sub_start = current_pos(r)?;
        let sub_end = sub_start + (sub.chunk_size & 0x7FFF_FFFF) as u64;
        match sub.chunk_type {
            W3dChunkType::HierarchyHeader => {
                h.header = read_hierarchy_header(r)?;
            }
            W3dChunkType::Pivots => {
                let count = h.header.num_pivots as usize;
                h.pivots.reserve(count);
                for _ in 0..count {
                    h.pivots.push(read_pivot(r)?);
                }
            }
            W3dChunkType::PivotFixups => {
                let count = h.header.num_pivots as usize;
                h.pivot_fixups.reserve(count);
                for _ in 0..count {
                    h.pivot_fixups.push(read_pivot_fixup(r)?);
                }
            }
            _ => {}
        }
        r.seek(SeekFrom::Start(sub_end))?;
    }

    Ok(h)
}

fn parse_animation_chunk<R: Read + Seek>(r: &mut R, size: u32) -> W3dResult<W3dAnimation> {
    let start = current_pos(r)?;
    let mut anim = W3dAnimation::default();

    while current_pos(r)? < start + size as u64 {
        let sub = read_chunk_header(r)?;
        let sub_start = current_pos(r)?;
        let sub_end = sub_start + (sub.chunk_size & 0x7FFF_FFFF) as u64;
        match sub.chunk_type {
            W3dChunkType::AnimationHeader => {
                anim.header = read_anim_header(r)?;
            }
            W3dChunkType::AnimationChannel => {
                let ch = read_anim_channel(r, (sub.chunk_size & 0x7FFF_FFFF) as usize)?;
                anim.channels.push(ch);
            }
            W3dChunkType::BitChannel => {
                let bch = read_bit_channel(r, (sub.chunk_size & 0x7FFF_FFFF) as usize)?;
                anim.bit_channels.push(bch);
            }
            _ => {}
        }
        r.seek(SeekFrom::Start(sub_end))?;
    }
    Ok(anim)
}

fn parse_morph_animation_chunk<R: Read + Seek>(r: &mut R, size: u32) -> W3dResult<W3dAnimation> {
    use crate::w3d::chunks::W3dChunkType::*;
    let start = current_pos(r)?;
    let mut anim = W3dAnimation::default();
    let mut current_track: Option<W3dMorphTrack> = None;
    while current_pos(r)? < start + size as u64 {
        let sub = read_chunk_header(r)?;
        let sub_start = current_pos(r)?;
        let sub_end = sub_start + (sub.chunk_size & 0x7FFF_FFFF) as u64;
        match sub.chunk_type {
            MorphAnimHeader => {
                // version u32, name[16], hierarchy[16], FrameCount u32, FrameRate f32, ChannelCount u32
                anim.header.version = r.read_u32::<LittleEndian>()?;
                anim.header.name = read_w3d_name(r)?;
                anim.header.hierarchy_name = read_w3d_name(r)?;
                anim.header.num_frames = r.read_u32::<LittleEndian>()?;
                let frame_rate_f32 = r.read_f32::<LittleEndian>()?;
                anim.header.frame_rate = frame_rate_f32 as u32;
                let _channel_count = r.read_u32::<LittleEndian>()?;
            }
            MorphAnimChannel => {
                // Inside channel: POSENAME and KEYDATA; we build a track per channel
                if let Some(track) = current_track.take() {
                    anim.morph_tracks.push(track);
                }
                current_track = Some(W3dMorphTrack {
                    pivot: 0,
                    target_name: String::new(),
                    keys: Vec::new(),
                });
                while current_pos(r)? < sub_end {
                    let ch = read_chunk_header(r)?;
                    let ch_start = current_pos(r)?;
                    let ch_end = ch_start + (ch.chunk_size & 0x7FFF_FFFF) as u64;
                    match ch.chunk_type {
                        MorphAnimPoseName => {
                            let mut buf = vec![0u8; (ch.chunk_size & 0x7FFF_FFFF) as usize];
                            r.read_exact(&mut buf)?;
                            if let Some(pos) = buf.iter().position(|&b| b == 0) {
                                buf.truncate(pos);
                            }
                            if let Some(t) = &mut current_track {
                                t.target_name = String::from_utf8_lossy(&buf).to_string();
                            }
                        }
                        MorphAnimKeyData => {
                            // Repeating pairs: MorphFrame u32, PoseFrame u32
                            let entries = ((ch.chunk_size & 0x7FFF_FFFF) as usize) / 8;
                            for _ in 0..entries {
                                let morph_frame = r.read_u32::<LittleEndian>()?;
                                let _pose_frame = r.read_u32::<LittleEndian>()?; // we ignore pose frame; we only animate weights
                                if let Some(t) = &mut current_track {
                                    t.keys.push(W3dMorphTrackKey {
                                        frame: morph_frame,
                                        weight: 1.0,
                                    });
                                }
                            }
                        }
                        MorphAnimPivotChannelData => {
                            // u32 per pivot indicating which channel controls the pivot; we ignore mapping here
                            // We'll keep default pivot=0
                            let _bytes = (ch.chunk_size & 0x7FFF_FFFF) as usize;
                            // skip
                        }
                        _ => {}
                    }
                    r.seek(SeekFrom::Start(ch_end))?;
                }
            }
            _ => {}
        }
        r.seek(SeekFrom::Start(sub_end))?;
    }
    if let Some(track) = current_track.take() {
        anim.morph_tracks.push(track);
    }
    Ok(anim)
}

fn parse_compressed_animation_chunk<R: Read + Seek>(
    r: &mut R,
    size: u32,
) -> W3dResult<W3dAnimation> {
    use crate::w3d::chunks::W3dChunkType::*;
    let start = current_pos(r)?;
    let mut anim = W3dAnimation::default();
    let mut flavor: u16 = 0;
    // num_frames tracked in header; no separate local needed
    while current_pos(r)? < start + size as u64 {
        let sub = read_chunk_header(r)?;
        let sub_start = current_pos(r)?;
        let sub_end = sub_start + (sub.chunk_size & 0x7FFF_FFFF) as u64;
        match sub.chunk_type {
            CompressedAnimationHeader => {
                // version u32, name[16], hierarchy[16], num_frames u32, frame_rate u16, flavor u16
                let version = r.read_u32::<LittleEndian>()?;
                let name = read_w3d_name(r)?;
                let hierarchy_name = read_w3d_name(r)?;
                let num_frames = r.read_u32::<LittleEndian>()?;
                let frame_rate_u16 = r.read_u16::<LittleEndian>()?;
                flavor = r.read_u16::<LittleEndian>()?;
                anim.header = W3dAnimationHeader {
                    version,
                    name,
                    hierarchy_name,
                    num_frames,
                    frame_rate: frame_rate_u16 as u32,
                };
            }
            CompressedAnimationChannel => {
                if flavor == 0 {
                    // ANIM_FLAVOR_TIMECODED
                    let chs = read_timecoded_anim_channel(r, &anim.header)?;
                    anim.extra_channels.push(chs);
                } else {
                    // AdaptiveDelta attempt
                    if let Some(chs) = read_adaptive_delta_anim_channel(r, &anim.header, sub_end)? {
                        for ch in chs {
                            anim.extra_channels.push(ch);
                        }
                    } else if let Some(ch) =
                        try_read_adaptive_delta_channel(r, &anim.header, sub_end)?
                    {
                        anim.extra_channels.push(ch);
                    }
                }
            }
            CompressedBitChannel => {
                if flavor == 0 {
                    let bch = read_timecoded_bit_channel(r, &anim.header)?;
                    anim.bit_channels.push(bch);
                } else {
                    // Fallback: treat as timecoded bit channel layout
                    let bch = read_timecoded_bit_channel(r, &anim.header)?;
                    anim.bit_channels.push(bch);
                }
            }
            _ => {}
        }
        r.seek(SeekFrom::Start(sub_end))?;
    }
    Ok(anim)
}

// Timecoded channel: prefix each vector by u32 timecode. MSB indicates binary (step) movement; lower 31 bits is frame index
fn read_timecoded_anim_channel<R: Read>(
    r: &mut R,
    hdr: &W3dAnimationHeader,
) -> W3dResult<W3dAnimationChannel> {
    let num_timecodes = r.read_u32::<LittleEndian>()? as usize;
    let pivot = r.read_u16::<LittleEndian>()?;
    let vector_len = r.read_u8()? as u16;
    let flags_tc = r.read_u8()? as u16; // timecoded flag/channel type
    // Read timecodes + vectors
    let mut timecodes: Vec<(u32, bool)> = Vec::with_capacity(num_timecodes);
    let mut samples: Vec<Vec<f32>> = Vec::with_capacity(num_timecodes);
    for _ in 0..num_timecodes {
        let tc = r.read_u32::<LittleEndian>()?;
        let binary = (tc & 0x8000_0000) != 0;
        let frame = tc & 0x7FFF_FFFF;
        timecodes.push((frame, binary));
        let mut v = Vec::with_capacity(vector_len as usize);
        for _ in 0..vector_len {
            v.push(f32::from_bits(r.read_u32::<LittleEndian>()?));
        }
        samples.push(v);
    }
    // Densify to per-frame samples over [0, num_frames-1]
    let total_frames = hdr.num_frames as usize;
    let vec_len = vector_len as usize;
    let mut data: Vec<f32> = vec![0.0; total_frames * vec_len];
    if timecodes.is_empty() {
        return Ok(W3dAnimationChannel {
            first_frame: 0,
            last_frame: (hdr.num_frames.max(1) - 1) as u16,
            vector_len,
            flags: map_timecoded_flag(flags_tc),
            pivot,
            data,
        });
    }
    // Ensure sorted by frame
    let mut pairs: Vec<((u32, bool), Vec<f32>)> =
        timecodes.into_iter().zip(samples.into_iter()).collect();
    pairs.sort_by_key(|p| p.0.0);
    // Fill from start to first timecode
    let (first_frame_tc, _bin0) = pairs[0].0;
    for f in 0..(first_frame_tc.min(hdr.num_frames)) as usize {
        let base = f * vec_len;
        for c in 0..vec_len {
            data[base + c] = pairs[0].1[c];
        }
    }
    // For each segment between tc[i] and tc[i+1]
    for seg in 0..pairs.len() {
        let (f0, bin0) = pairs[seg].0;
        let v0 = &pairs[seg].1;
        let f1 = if seg + 1 < pairs.len() {
            pairs[seg + 1].0.0
        } else {
            hdr.num_frames - 1
        };
        let v1 = if seg + 1 < pairs.len() {
            &pairs[seg + 1].1
        } else {
            v0
        };
        let step = bin0; // if binary flag set at start timecode, step hold until next
        let start_f = f0 as usize;
        let end_f = f1 as usize;
        if start_f > end_f || start_f as u32 >= hdr.num_frames {
            continue;
        }
        let clamped_end = end_f.min(total_frames - 1);
        for f in start_f..=clamped_end {
            let t = if end_f == start_f {
                0.0
            } else {
                (f as f32 - start_f as f32) / (end_f as f32 - start_f as f32)
            };
            let base = f * vec_len;
            for c in 0..vec_len {
                data[base + c] = if step {
                    v0[c]
                } else {
                    v0[c] * (1.0 - t) + v1[c] * t
                };
            }
        }
    }
    Ok(W3dAnimationChannel {
        first_frame: 0,
        last_frame: (hdr.num_frames.max(1) - 1) as u16,
        vector_len,
        flags: map_timecoded_flag(flags_tc),
        pivot,
        data,
    })
}

fn build_filter_table() -> [f32; 256] {
    let mut table = [0.0f32; 256];
    let base: [f32; 16] = [
        0.00000001, 0.0000001, 0.000001, 0.00001, 0.0001, 0.001, 0.01, 0.1, 1.0, 10.0, 100.0,
        1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0,
    ];
    table[..16].copy_from_slice(&base);
    let gen_start = 16usize;
    let gen_size = 256 - gen_start;
    for i in 0..gen_size {
        let ratio = (i as f32) / (gen_size as f32);
        table[gen_start + i] = 1.0 - (std::f32::consts::FRAC_PI_2 * ratio).sin();
    }
    table
}

fn read_adaptive_delta_anim_channel<R: Read + Seek>(
    r: &mut R,
    hdr: &W3dAnimationHeader,
    sub_end: u64,
) -> W3dResult<Option<Vec<W3dAnimationChannel>>> {
    let num_frames = match r.read_u32::<LittleEndian>() {
        Ok(v) => v as usize,
        Err(_) => return Ok(None),
    };
    let pivot = match r.read_u16::<LittleEndian>() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let vector_len = match r.read_u8() {
        Ok(v) => v as usize,
        Err(_) => return Ok(None),
    };
    let _flags_raw = match r.read_u8() {
        Ok(v) => v as u16,
        Err(_) => return Ok(None),
    };
    let scale = match r.read_f32::<LittleEndian>() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if vector_len == 0 || num_frames == 0 {
        return Ok(None);
    }
    let blocks = (num_frames + 15) / 16;
    let packet_count = blocks * vector_len;
    let bytes_left = (sub_end.saturating_sub(current_pos(r)?)) as usize;
    if bytes_left < packet_count * 9 {
        return Ok(None);
    }
    let filter_table = build_filter_table();
    let mut data = vec![0.0f32; num_frames * vector_len];
    let mut last_vals = vec![0.0f32; vector_len];
    if vector_len == 4 {
        last_vals[3] = 1.0;
    }
    for b in 0..blocks {
        for vi in 0..vector_len {
            let b0 = r.read_u8()?;
            let filter_idx = (b0 & 0x7F) as usize;
            let mut nibbles = [0u8; 16];
            for byte_i in 0..8 {
                let byte = r.read_u8()?;
                nibbles[byte_i * 2] = (byte & 0x0F) as u8;
                nibbles[byte_i * 2 + 1] = (byte >> 4) as u8;
            }
            let filter = filter_table.get(filter_idx).copied().unwrap_or(1.0) * scale;
            for fi in 0..16 {
                let frame = b * 16 + fi;
                if frame >= num_frames {
                    break;
                }
                let raw = nibbles[fi] as i32;
                let factor = (raw - 8) as f32; // -8..7
                let value = last_vals[vi] + factor * filter;
                data[frame * vector_len + vi] = value;
                last_vals[vi] = value;
            }
        }
    }
    let mut out: Vec<W3dAnimationChannel> = Vec::new();
    if vector_len == 3 {
        for axis in 0..3 {
            let mut axis_data = Vec::with_capacity(num_frames);
            for f in 0..num_frames {
                axis_data.push(data[f * 3 + axis]);
            }
            out.push(W3dAnimationChannel {
                first_frame: 0,
                last_frame: (hdr.num_frames.max(1) - 1) as u16,
                vector_len: 1,
                flags: axis as u16,
                pivot,
                data: axis_data,
            });
        }
    } else if vector_len == 4 {
        for f in 0..num_frames {
            let i = f * 4;
            let x = data[i];
            let y = data[i + 1];
            let z = data[i + 2];
            let w = data[i + 3];
            let len = (x * x + y * y + z * z + w * w).sqrt();
            if len > 0.00001 {
                data[i] = x / len;
                data[i + 1] = y / len;
                data[i + 2] = z / len;
                data[i + 3] = w / len;
            }
        }
        out.push(W3dAnimationChannel {
            first_frame: 0,
            last_frame: (hdr.num_frames.max(1) - 1) as u16,
            vector_len: 4,
            flags: 6,
            pivot,
            data,
        });
    } else {
        let mut axis_data = Vec::with_capacity(num_frames);
        for f in 0..num_frames {
            axis_data.push(data[f]);
        }
        out.push(W3dAnimationChannel {
            first_frame: 0,
            last_frame: (hdr.num_frames.max(1) - 1) as u16,
            vector_len: 1,
            flags: 0,
            pivot,
            data: axis_data,
        });
    }
    Ok(Some(out))
}
// Adaptive Delta channel (compressed flavor 1):
// We don't have bitpacking details here, but we can safely accept a simple stream
// variant seen in some assets: pivot u16, vector_len u8, flags u8, then num_frames samples
// stored as f32 (vector_len per frame). If the byte count doesn't line up, we skip.
fn try_read_adaptive_delta_channel<R: Read + Seek>(
    r: &mut R,
    hdr: &W3dAnimationHeader,
    sub_end: u64,
) -> W3dResult<Option<W3dAnimationChannel>> {
    let _start = current_pos(r)?;
    // Try to read a minimal header: Pivot u16, VectorLen u8, Flags u8
    let pivot = match r.read_u16::<LittleEndian>() {
        Ok(v) => v,
        Err(_) => {
            r.seek(SeekFrom::Start(sub_end))?;
            return Ok(None);
        }
    };
    let vector_len = match r.read_u8() {
        Ok(v) => v as u16,
        Err(_) => {
            r.seek(SeekFrom::Start(sub_end))?;
            return Ok(None);
        }
    };
    let flags_raw = match r.read_u8() {
        Ok(v) => v as u16,
        Err(_) => {
            r.seek(SeekFrom::Start(sub_end))?;
            return Ok(None);
        }
    };
    // Compute how many f32 values fit until sub_end
    let cur = current_pos(r)?;
    let bytes_left = (sub_end.saturating_sub(cur)) as usize;
    let total_frames = hdr.num_frames as usize;
    let expected_values = total_frames * vector_len as usize;
    let expected_bytes = expected_values * 4;
    if bytes_left < expected_bytes {
        r.seek(SeekFrom::Start(sub_end))?;
        return Ok(None);
    }
    // Read values
    let mut data = Vec::with_capacity(expected_values);
    for _ in 0..expected_values {
        data.push(r.read_f32::<LittleEndian>()?);
    }
    Ok(Some(W3dAnimationChannel {
        first_frame: 0,
        last_frame: (hdr.num_frames.max(1) - 1) as u16,
        vector_len,
        flags: match flags_raw {
            3..=5 => flags_raw - 3,
            6 => 6,
            0..=2 => flags_raw,
            _ => 0,
        },
        pivot,
        data,
    }))
}

fn read_timecoded_bit_channel<R: Read>(
    r: &mut R,
    hdr: &W3dAnimationHeader,
) -> W3dResult<W3dBitChannel> {
    // NumTimeCodes u32, Pivot u16, Flags u8, DefaultVal u8, then Data[NumTimeCodes] (u32 time codes, MSB holds bit value)
    let num_timecodes = r.read_u32::<LittleEndian>()? as usize;
    let pivot = r.read_u16::<LittleEndian>()?;
    let flags = r.read_u8()? as u16;
    let default_val = r.read_u8()?;
    let mut timecodes: Vec<(u32, u8)> = Vec::with_capacity(num_timecodes);
    for _ in 0..num_timecodes {
        let tc = r.read_u32::<LittleEndian>()?;
        let value = if (tc & 0x8000_0000) != 0 { 1u8 } else { 0u8 };
        let frame = tc & 0x7FFF_FFFF;
        timecodes.push((frame, value));
    }
    let total_frames = hdr.num_frames as usize;
    let mut frames: Vec<u8> = vec![default_val; total_frames];
    if !timecodes.is_empty() {
        // sort by frame and fill forward until next change
        timecodes.sort_by_key(|p| p.0);
        let mut current_val = timecodes[0].1;
        let mut current_start = timecodes[0].0 as usize;
        for i in 0..timecodes.len() {
            let (f, v) = timecodes[i];
            // fill until this frame with current_val
            let end = f as usize;
            for ff in current_start.min(total_frames)..end.min(total_frames) {
                frames[ff] = current_val;
            }
            current_val = v;
            current_start = end;
        }
        for ff in current_start.min(total_frames)..total_frames {
            frames[ff] = current_val;
        }
    }
    // Pack bits into bytes like classic bit channel expects
    let bytes_needed = (total_frames + 7) / 8;
    let mut data = vec![0u8; bytes_needed];
    for i in 0..total_frames {
        if frames[i] != 0 {
            let byte = i / 8;
            let bit = i % 8;
            data[byte] |= 1u8 << bit;
        }
    }
    Ok(W3dBitChannel {
        first_frame: 0,
        last_frame: (hdr.num_frames.max(1) - 1) as u16,
        flags,
        pivot,
        default_val,
        data,
    })
}

fn map_timecoded_flag(flag: u16) -> u16 {
    // Map timecoded flags to classic flags: X=0, Y=1, Z=2, Q=6
    match flag {
        8 => 0,  // ANIM_CHANNEL_TIMECODED_X
        9 => 1,  // ANIM_CHANNEL_TIMECODED_Y
        10 => 2, // ANIM_CHANNEL_TIMECODED_Z
        11 => 6, // ANIM_CHANNEL_TIMECODED_Q
        _ => flag,
    }
}

fn read_anim_header<R: Read>(r: &mut R) -> W3dResult<W3dAnimationHeader> {
    Ok(W3dAnimationHeader {
        version: r.read_u32::<LittleEndian>()?,
        name: read_w3d_name(r)?,
        hierarchy_name: read_w3d_name(r)?,
        num_frames: r.read_u32::<LittleEndian>()?,
        frame_rate: r.read_u32::<LittleEndian>()?,
    })
}

fn read_anim_channel<R: Read>(r: &mut R, size: usize) -> W3dResult<W3dAnimationChannel> {
    let first_frame = r.read_u16::<LittleEndian>()?;
    let last_frame = r.read_u16::<LittleEndian>()?;
    let vector_len = r.read_u16::<LittleEndian>()?;
    let flags = r.read_u16::<LittleEndian>()?;
    let pivot = r.read_u16::<LittleEndian>()?;
    let _pad = r.read_u16::<LittleEndian>()?;
    // Remaining bytes are f32 data
    let remaining = size.saturating_sub(12);
    let count_f32 = remaining / 4;
    let mut data = Vec::with_capacity(count_f32);
    for _ in 0..count_f32 {
        data.push(r.read_f32::<LittleEndian>()?);
    }
    Ok(W3dAnimationChannel {
        first_frame,
        last_frame,
        vector_len,
        flags,
        pivot,
        data,
    })
}

fn read_bit_channel<R: Read>(r: &mut R, size: usize) -> W3dResult<W3dBitChannel> {
    let first_frame = r.read_u16::<LittleEndian>()?;
    let last_frame = r.read_u16::<LittleEndian>()?;
    let flags = r.read_u16::<LittleEndian>()?;
    let pivot = r.read_u16::<LittleEndian>()?;
    let default_val = r.read_u8()?;
    // Remaining bytes are the bit data
    let remaining = size.saturating_sub(9);
    let mut data = vec![0u8; remaining];
    r.read_exact(&mut data)?;
    Ok(W3dBitChannel {
        first_frame,
        last_frame,
        flags,
        pivot,
        default_val,
        data,
    })
}

fn read_hierarchy_header<R: Read>(r: &mut R) -> W3dResult<W3dHierarchyHeader> {
    Ok(W3dHierarchyHeader {
        version: r.read_u32::<LittleEndian>()?,
        name: read_w3d_name(r)?,
        num_pivots: r.read_u32::<LittleEndian>()?,
        center: read_vector(r)?,
    })
}

fn read_pivot<R: Read>(r: &mut R) -> W3dResult<W3dPivot> {
    Ok(W3dPivot {
        name: read_w3d_name(r)?,
        parent_idx: r.read_u32::<LittleEndian>()?,
        translation: read_vector(r)?,
        euler_angles: read_vector(r)?,
        rotation: read_quat(r)?,
    })
}

fn read_pivot_fixup<R: Read>(r: &mut R) -> W3dResult<W3dPivotFixup> {
    let mut tm = [[0.0f32; 3]; 4];
    for row in 0..4 {
        for col in 0..3 {
            tm[row][col] = r.read_f32::<LittleEndian>()?;
        }
    }
    Ok(W3dPivotFixup { tm })
}

fn current_pos<S: Seek>(s: &mut S) -> W3dResult<u64> {
    Ok(s.stream_position()?)
}

fn read_rgb<R: Read>(r: &mut R) -> W3dResult<W3dRgb> {
    let mut c = [0u8; 4];
    r.read_exact(&mut c)?;
    Ok(W3dRgb {
        r: c[0],
        g: c[1],
        b: c[2],
        pad: c[3],
    })
}

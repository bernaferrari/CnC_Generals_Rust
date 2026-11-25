//! Model-space geometry that can be read without constructing render objects.
//!
//! The section lookup follows W3DBridgeBuffer.cpp, and the HLOD traversal
//! and bind pose follow hlod.cpp and htree.cpp in WW3D2.

use std::io::Cursor;

use binrw::{BinRead, BinReaderExt};
use glam::{Affine3A, Quat, Vec3};

use crate::{
    W3DChunkType, W3DError, W3DResult, W3dChunkHeader, W3dHierarchyStruct, W3dHlodLodArrayStruct,
    W3dHlodStruct, W3dHlodSubObjectStruct, W3dMeshHeader3Struct, W3dPivotStruct, W3dVectorStruct,
    w3d_string_from_bytes,
};

/// Lateral bounds of a named rigid section in the parent model's bind pose.
///
/// W3DBridge::load scans the original HLOD subobjects in their file order,
/// including every LOD. It expands the section name to each matching child
/// name. If no child matches, it looks up the exact mesh at identity.
pub fn rigid_section_lateral_bounds(
    bytes: &[u8],
    model_name: &str,
    section_name: &str,
) -> W3DResult<(f32, f32)> {
    use W3DChunkType::*;

    let model = Chunks::read(bytes)?;
    let mut meshes = Vec::new();
    let mut hlod = None;
    for (kind, data) in model.0.iter().copied() {
        match kind {
            Some(Mesh) => {
                let mesh = Chunks::read(data)?;
                let header: W3dMeshHeader3Struct = mesh.read_header(MeshHeader3)?;
                let container = w3d_string_from_bytes(&header.container_name);
                let name = w3d_string_from_bytes(&header.mesh_name);
                let name = if container.is_empty() {
                    name
                } else {
                    format!("{container}.{name}")
                };
                meshes.push((name, header.num_verts, mesh.get(Vertices)));
            }
            Some(Hlod) if hlod.is_none() => {
                let chunks = Chunks::read(data)?;
                let header: W3dHlodStruct = chunks.read_header(HlodHeader)?;
                if w3d_string_from_bytes(&header.name).eq_ignore_ascii_case(model_name) {
                    hlod = Some((chunks, header));
                }
            }
            _ => {}
        }
    }

    let has_mesh = |name: &str| meshes.iter().any(|mesh| mesh.0.eq_ignore_ascii_case(name));
    let mut section = format!("{model_name}.{section_name}").to_ascii_lowercase();
    let mut transform = Affine3A::IDENTITY;
    if let Some((hlod, header)) = hlod {
        let mut bone = None;
        let lods = hlod
            .0
            .iter()
            .copied()
            .filter(|(kind, _)| *kind == Some(HlodLodArray))
            .take(header.num_lods as usize)
            .map(|(_, data)| Ok::<_, W3DError>(data));
        let additional = hlod
            .0
            .iter()
            .copied()
            .filter(|(kind, _)| *kind == Some(HlodAggregateArray))
            .map(|(_, data)| Ok(Chunks::read(data)?.get(HlodLodArray).unwrap_or(data)));
        for array in lods.chain(additional) {
            let children = Chunks::read(array?)?;
            let header: W3dHlodLodArrayStruct = children.read_header(HlodSubObjectArrayHeader)?;
            for record in children
                .0
                .iter()
                .filter(|(kind, _)| *kind == Some(HlodSubObject))
                .take(header.model_count as usize)
            {
                let child: W3dHlodSubObjectStruct = read_le(record.1)?;
                let name = w3d_string_from_bytes(&child.name).to_ascii_lowercase();
                if name.starts_with(&section) && has_mesh(&name) {
                    section = name;
                    bone = Some(child.bone_index);
                }
            }
        }
        if let Some(bone) = bone {
            let hierarchy = w3d_string_from_bytes(&header.hierarchy_name);
            transform = model.pivot_transform(&hierarchy, bone as usize)?;
        }
    } else if !has_mesh(model_name) {
        return Err(W3DError::AssetNotFound(model_name.into()));
    }

    let (_, count, data) = meshes
        .into_iter()
        .find(|mesh| mesh.0.eq_ignore_ascii_case(&section))
        .ok_or(W3DError::AssetNotFound(section))?;
    let data = data.ok_or(W3DError::CorruptedFile)?;
    let size = std::mem::size_of::<W3dVectorStruct>();
    if count == 0 || data.len() / size < count as usize {
        return Err(W3DError::CorruptedFile);
    }

    let mut vertices = Cursor::new(data);
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for _ in 0..count {
        let vertex = Vec3::from(vertices.read_le::<W3dVectorStruct>()?);
        let y = transform.transform_point3(vertex).y;
        if !y.is_finite() {
            return Err(W3DError::CorruptedFile);
        }
        min = min.min(y);
        max = max.max(y);
    }
    Ok((min, max))
}

struct Chunks<'a>(Vec<(Option<W3DChunkType>, &'a [u8])>);

impl<'a> Chunks<'a> {
    fn read(mut bytes: &'a [u8]) -> W3DResult<Self> {
        let mut chunks = Vec::new();
        while !bytes.is_empty() {
            let header: W3dChunkHeader = read_le(bytes)?;
            let (payload, remaining) = bytes[std::mem::size_of::<W3dChunkHeader>()..]
                .split_at_checked(header.actual_size() as usize)
                .ok_or(W3DError::CorruptedFile)?;
            chunks.push((header.chunk_type(), payload));
            bytes = remaining;
        }
        Ok(Self(chunks))
    }

    fn get(&self, kind: W3DChunkType) -> Option<&'a [u8]> {
        self.0
            .iter()
            .find_map(|&(id, data)| (id == Some(kind)).then_some(data))
    }

    fn read_header<T>(&self, kind: W3DChunkType) -> W3DResult<T>
    where
        T: for<'b> BinRead<Args<'b> = ()>,
    {
        read_le(self.get(kind).ok_or(W3DError::CorruptedFile)?)
    }

    fn pivot_transform(&self, hierarchy: &str, bone: usize) -> W3DResult<Affine3A> {
        use W3DChunkType::*;

        // HTreeClass::Base_Update always starts at identity for a new model.
        if bone == 0 {
            return Ok(Affine3A::IDENTITY);
        }
        for &(_, bytes) in self.0.iter().filter(|(kind, _)| *kind == Some(Hierarchy)) {
            let tree = Self::read(bytes)?;
            let header: W3dHierarchyStruct = tree.read_header(HierarchyHeader)?;
            if !w3d_string_from_bytes(&header.name).eq_ignore_ascii_case(hierarchy) {
                continue;
            }
            return tree.pivot_chain(header, bone);
        }
        Err(W3DError::AssetNotFound(hierarchy.into()))
    }

    fn pivot_chain(&self, header: W3dHierarchyStruct, mut bone: usize) -> W3DResult<Affine3A> {
        const ROOT_PIVOT_VERSION: u32 = 0x0003_0000;
        const PIVOT_SIZE: usize = std::mem::size_of::<W3dPivotStruct>() + 16;
        let records = self
            .get(W3DChunkType::Pivots)
            .ok_or(W3DError::CorruptedFile)?;
        let root_in_file = header.version >= ROOT_PIVOT_VERSION;
        let first = usize::from(!root_in_file);
        if bone - first >= header.num_pivots as usize {
            return Err(W3DError::CorruptedFile);
        }

        let (records, _) = records.as_chunks::<PIVOT_SIZE>();
        let mut transform = Affine3A::IDENTITY;
        while bone != 0 {
            let mut record = Cursor::new(records.get(bone - first).ok_or(W3DError::CorruptedFile)?);
            let pivot: W3dPivotStruct = record.read_le()?;
            let rotation: [f32; 4] = record.read_le()?;
            let rotation = Quat::from_array(rotation);
            let translation = Vec3::from(pivot.translation);
            if !rotation.is_finite() || !translation.is_finite() {
                return Err(W3DError::CorruptedFile);
            }
            let parent = usize::try_from(i64::from(pivot.parent_idx) + first as i64)
                .map_err(|_| W3DError::CorruptedFile)?;
            // The C++ pose update visits parents before their children.
            if parent >= bone {
                return Err(W3DError::CorruptedFile);
            }
            transform = Affine3A::from_rotation_translation(rotation, translation) * transform;
            bone = parent;
        }
        Ok(transform)
    }
}

fn read_le<T>(data: &[u8]) -> W3DResult<T>
where
    T: for<'a> BinRead<Args<'a> = ()>,
{
    Ok(Cursor::new(data).read_le()?)
}

use glam::{Mat4, Quat, Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrannyVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub bone_indices: [u16; 4],
    pub bone_weights: [f32; 4],
}

impl Default for GrannyVertex {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::Y,
            uv: Vec2::ZERO,
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyBone {
    pub name: String,
    pub parent_index: i32,
    pub inverse_bind_pose: Mat4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyMesh {
    pub name: String,
    pub vertices: Vec<GrannyVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyKeyframe {
    pub time_seconds: f32,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyAnimationTrack {
    pub bone_index: usize,
    pub keyframes: Vec<GrannyKeyframe>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyAnimation {
    pub name: String,
    pub duration_seconds: f32,
    pub tracks: Vec<GrannyAnimationTrack>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyModel {
    pub meshes: Vec<GrannyMesh>,
    pub bones: Vec<GrannyBone>,
    pub animations: Vec<GrannyAnimation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyGpuMesh {
    pub vertex_bytes: Vec<u8>,
    pub index_bytes: Vec<u8>,
    pub vertex_count: usize,
    pub index_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrannyPose {
    pub bone_matrices: Vec<Mat4>,
}

#[derive(Debug, thiserror::Error)]
pub enum GrannyError {
    #[error("invalid granny header")]
    InvalidHeader,
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("mesh data is malformed")]
    InvalidMeshData,
}

struct ByteReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_exact<const N: usize>(&mut self) -> Result<[u8; N], GrannyError> {
        if self.offset + N > self.bytes.len() {
            return Err(GrannyError::UnexpectedEof);
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.offset..self.offset + N]);
        self.offset += N;
        Ok(out)
    }

    fn read_u16(&mut self) -> Result<u16, GrannyError> {
        Ok(u16::from_le_bytes(self.read_exact()?))
    }

    fn read_u32(&mut self) -> Result<u32, GrannyError> {
        Ok(u32::from_le_bytes(self.read_exact()?))
    }

    fn read_i32(&mut self) -> Result<i32, GrannyError> {
        Ok(i32::from_le_bytes(self.read_exact()?))
    }

    fn read_f32(&mut self) -> Result<f32, GrannyError> {
        Ok(f32::from_le_bytes(self.read_exact()?))
    }

    fn read_string(&mut self) -> Result<String, GrannyError> {
        let len = self.read_u16()? as usize;
        if self.offset + len > self.bytes.len() {
            return Err(GrannyError::UnexpectedEof);
        }
        let out = String::from_utf8_lossy(&self.bytes[self.offset..self.offset + len]).to_string();
        self.offset += len;
        Ok(out)
    }
}

pub struct W3DGrannyLoader;

impl W3DGrannyLoader {
    pub fn load_gr2(bytes: &[u8]) -> Result<GrannyModel, GrannyError> {
        let mut reader = ByteReader::new(bytes);
        if &reader.read_exact::<4>()? != b"GR2R" {
            return Err(GrannyError::InvalidHeader);
        }
        let mesh_count = reader.read_u16()? as usize;
        let bone_count = reader.read_u16()? as usize;
        let anim_count = reader.read_u16()? as usize;

        let mut meshes = Vec::with_capacity(mesh_count);
        for _ in 0..mesh_count {
            let name = reader.read_string()?;
            let vertex_count = reader.read_u32()? as usize;
            let index_count = reader.read_u32()? as usize;
            if index_count % 3 != 0 {
                return Err(GrannyError::InvalidMeshData);
            }

            let mut vertices = Vec::with_capacity(vertex_count);
            for _ in 0..vertex_count {
                let position =
                    Vec3::new(reader.read_f32()?, reader.read_f32()?, reader.read_f32()?);
                let normal = Vec3::new(reader.read_f32()?, reader.read_f32()?, reader.read_f32()?);
                let uv = Vec2::new(reader.read_f32()?, reader.read_f32()?);
                let bone_indices = [
                    reader.read_u16()?,
                    reader.read_u16()?,
                    reader.read_u16()?,
                    reader.read_u16()?,
                ];
                let bone_weights = [
                    reader.read_f32()?,
                    reader.read_f32()?,
                    reader.read_f32()?,
                    reader.read_f32()?,
                ];
                vertices.push(GrannyVertex {
                    position,
                    normal,
                    uv,
                    bone_indices,
                    bone_weights,
                });
            }

            let mut indices = Vec::with_capacity(index_count);
            for _ in 0..index_count {
                indices.push(reader.read_u32()?);
            }

            meshes.push(GrannyMesh {
                name,
                vertices,
                indices,
            });
        }

        let mut bones = Vec::with_capacity(bone_count);
        for _ in 0..bone_count {
            let name = reader.read_string()?;
            let parent_index = reader.read_i32()?;
            let matrix = Mat4::from_cols_array(&[
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
                reader.read_f32()?,
            ]);
            bones.push(GrannyBone {
                name,
                parent_index,
                inverse_bind_pose: matrix,
            });
        }

        let mut animations = Vec::with_capacity(anim_count);
        for _ in 0..anim_count {
            let name = reader.read_string()?;
            let duration_seconds = reader.read_f32()?;
            let track_count = reader.read_u16()? as usize;
            let mut tracks = Vec::with_capacity(track_count);
            for _ in 0..track_count {
                let bone_index = reader.read_u16()? as usize;
                let keyframe_count = reader.read_u16()? as usize;
                let mut keyframes = Vec::with_capacity(keyframe_count);
                for _ in 0..keyframe_count {
                    keyframes.push(GrannyKeyframe {
                        time_seconds: reader.read_f32()?,
                        translation: Vec3::new(
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                        ),
                        rotation: Quat::from_xyzw(
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                        ),
                        scale: Vec3::new(
                            reader.read_f32()?,
                            reader.read_f32()?,
                            reader.read_f32()?,
                        ),
                    });
                }
                tracks.push(GrannyAnimationTrack {
                    bone_index,
                    keyframes,
                });
            }
            animations.push(GrannyAnimation {
                name,
                duration_seconds,
                tracks,
            });
        }

        Ok(GrannyModel {
            meshes,
            bones,
            animations,
        })
    }

    pub fn create_gpu_mesh(mesh: &GrannyMesh) -> GrannyGpuMesh {
        let mut vertex_bytes = Vec::with_capacity(mesh.vertices.len() * (3 + 3 + 2 + 4 + 4) * 4);
        for vertex in &mesh.vertices {
            for value in vertex.position.to_array() {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
            for value in vertex.normal.to_array() {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
            for value in vertex.uv.to_array() {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
            for value in vertex.bone_indices {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
            for value in vertex.bone_weights {
                vertex_bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut index_bytes = Vec::with_capacity(mesh.indices.len() * 4);
        for index in &mesh.indices {
            index_bytes.extend_from_slice(&index.to_le_bytes());
        }
        GrannyGpuMesh {
            vertex_bytes,
            index_bytes,
            vertex_count: mesh.vertices.len(),
            index_count: mesh.indices.len(),
        }
    }

    pub fn sample_animation(
        model: &GrannyModel,
        animation_name: &str,
        time_seconds: f32,
    ) -> GrannyPose {
        let mut bone_matrices = vec![Mat4::IDENTITY; model.bones.len()];
        let Some(animation) = model
            .animations
            .iter()
            .find(|animation| animation.name == animation_name)
        else {
            return GrannyPose { bone_matrices };
        };

        for track in &animation.tracks {
            if let Some(first) = track.keyframes.first() {
                let mut selected = first;
                for keyframe in &track.keyframes {
                    if keyframe.time_seconds <= time_seconds {
                        selected = keyframe;
                    } else {
                        break;
                    }
                }
                bone_matrices[track.bone_index] = Mat4::from_scale_rotation_translation(
                    selected.scale,
                    selected.rotation,
                    selected.translation,
                );
            }
        }
        GrannyPose { bone_matrices }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u16(buf: &mut Vec<u8>, value: u16) {
        buf.extend_from_slice(&value.to_le_bytes());
    }
    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }
    fn push_i32(buf: &mut Vec<u8>, value: i32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }
    fn push_f32(buf: &mut Vec<u8>, value: f32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }
    fn push_str(buf: &mut Vec<u8>, value: &str) {
        push_u16(buf, value.len() as u16);
        buf.extend_from_slice(value.as_bytes());
    }

    #[test]
    fn parses_simplified_granny_blob() {
        let mut bytes = b"GR2R".to_vec();
        push_u16(&mut bytes, 1);
        push_u16(&mut bytes, 1);
        push_u16(&mut bytes, 1);

        push_str(&mut bytes, "mesh");
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, 3);
        for value in [1.0, 2.0, 3.0, 0.0, 1.0, 0.0, 0.5, 0.25] {
            push_f32(&mut bytes, value);
        }
        for value in [0u16, 0, 0, 0] {
            push_u16(&mut bytes, value);
        }
        for value in [1.0, 0.0, 0.0, 0.0] {
            push_f32(&mut bytes, value);
        }
        for index in [0u32, 0, 0] {
            push_u32(&mut bytes, index);
        }

        push_str(&mut bytes, "root");
        push_i32(&mut bytes, -1);
        for value in Mat4::IDENTITY.to_cols_array() {
            push_f32(&mut bytes, value);
        }

        push_str(&mut bytes, "idle");
        push_f32(&mut bytes, 1.0);
        push_u16(&mut bytes, 1);
        push_u16(&mut bytes, 0);
        push_u16(&mut bytes, 1);
        push_f32(&mut bytes, 0.0);
        for value in [0.0, 0.0, 0.0] {
            push_f32(&mut bytes, value);
        }
        for value in [0.0, 0.0, 0.0, 1.0] {
            push_f32(&mut bytes, value);
        }
        for value in [1.0, 1.0, 1.0] {
            push_f32(&mut bytes, value);
        }

        let model = W3DGrannyLoader::load_gr2(&bytes).unwrap();
        assert_eq!(model.meshes.len(), 1);
        assert_eq!(model.bones.len(), 1);
        assert_eq!(model.animations.len(), 1);
        let gpu = W3DGrannyLoader::create_gpu_mesh(&model.meshes[0]);
        assert_eq!(gpu.vertex_count, 1);
        assert_eq!(gpu.index_count, 3);
    }
}

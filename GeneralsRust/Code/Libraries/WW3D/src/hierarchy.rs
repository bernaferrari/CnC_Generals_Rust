// Hierarchical Animation System
// Ported from htree.h, htree.cpp

use crate::math::*;
use crate::w3d_file::*;
use crate::{Result, W3DError};
use std::io::Read;

// Pivot (bone) structure
#[derive(Debug, Clone)]
pub struct Pivot {
    pub name: String,
    pub parent_idx: i32,
    pub translation: Vec3,
    pub rotation: UnitQuat,
    pub transform: Mat4,
    pub is_visible: bool,
    pub is_captured: bool,
    pub control_transform: Option<Mat4>,
}

impl Pivot {
    pub fn new(name: String, parent_idx: i32) -> Self {
        Self {
            name,
            parent_idx,
            translation: Vec3::zeros(),
            rotation: UnitQuat::identity(),
            transform: Mat4::identity(),
            is_visible: true,
            is_captured: false,
            control_transform: None,
        }
    }
}

// Hierarchy Tree - skeleton structure
pub struct HTree {
    pub name: String,
    pub num_pivots: usize,
    pub pivots: Vec<Pivot>,
    pub scale_factor: f32,
}

impl HTree {
    pub fn new(name: String) -> Self {
        Self {
            name,
            num_pivots: 0,
            pivots: Vec::new(),
            scale_factor: 1.0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn num_pivots(&self) -> usize {
        self.num_pivots
    }

    pub fn get_bone_index(&self, name: &str) -> Option<usize> {
        self.pivots.iter().position(|p| p.name == name)
    }

    pub fn get_bone_name(&self, bone_idx: usize) -> Option<&str> {
        self.pivots.get(bone_idx).map(|p| p.name.as_str())
    }

    pub fn get_parent_index(&self, bone_idx: usize) -> Option<i32> {
        self.pivots.get(bone_idx).map(|p| p.parent_idx)
    }

    pub fn get_transform(&self, pivot: usize) -> Option<&Mat4> {
        self.pivots.get(pivot).map(|p| &p.transform)
    }

    pub fn get_visibility(&self, pivot: usize) -> bool {
        self.pivots.get(pivot).map(|p| p.is_visible).unwrap_or(false)
    }

    pub fn get_root_transform(&self) -> &Mat4 {
        &self.pivots[0].transform
    }

    // Update hierarchy with base pose
    pub fn base_update(&mut self, root: &Mat4) {
        if self.pivots.is_empty() {
            return;
        }

        // Root pivot
        self.pivots[0].transform = *root * matrix_from_rotation_translation(
            &self.pivots[0].rotation,
            &self.pivots[0].translation,
        );

        // Update all children
        for i in 1..self.num_pivots {
            let parent_idx = self.pivots[i].parent_idx as usize;
            let parent_transform = self.pivots[parent_idx].transform;

            let local_transform = matrix_from_rotation_translation(
                &self.pivots[i].rotation,
                &self.pivots[i].translation,
            );

            self.pivots[i].transform = parent_transform * local_transform;
        }
    }

    // Capture a bone for manual control
    pub fn capture_bone(&mut self, bone_index: usize) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            pivot.is_captured = true;
            pivot.control_transform = Some(pivot.transform);
        }
    }

    // Release a captured bone
    pub fn release_bone(&mut self, bone_index: usize) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            pivot.is_captured = false;
            pivot.control_transform = None;
        }
    }

    // Check if a bone is captured
    pub fn is_bone_captured(&self, bone_index: usize) -> bool {
        self.pivots.get(bone_index).map(|p| p.is_captured).unwrap_or(false)
    }

    // Control a captured bone
    pub fn control_bone(&mut self, bone_index: usize, relative_tm: &Mat4, world_space_translation: bool) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            if pivot.is_captured {
                if world_space_translation {
                    // Apply world space translation
                    let mut transform = *relative_tm;
                    if pivot.parent_idx >= 0 {
                        let parent_transform = self.pivots[pivot.parent_idx as usize].transform;
                        if let Some(parent_inv) = parent_transform.try_inverse() {
                            transform = parent_inv * transform;
                        }
                    }
                    pivot.control_transform = Some(transform);
                } else {
                    pivot.control_transform = Some(*relative_tm);
                }
            }
        }
    }

    // Scale the entire hierarchy
    pub fn scale(&mut self, factor: f32) {
        self.scale_factor *= factor;
        for pivot in &mut self.pivots {
            pivot.translation *= factor;
        }
    }

    // Load hierarchy from W3D file
    pub fn load_w3d<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        loop {
            let header = match W3DChunkHeader::read(reader) {
                Ok(h) => h,
                Err(_) => break,
            };

            match W3DChunkType::from_u32(header.chunk_type) {
                Some(W3DChunkType::HierarchyHeader) => self.read_header(reader)?,
                Some(W3DChunkType::Pivots) => self.read_pivots(reader, header.chunk_size)?,
                _ => {
                    // Skip unknown chunks
                    let mut buf = vec![0u8; header.chunk_size as usize];
                    reader.read_exact(&mut buf)?;
                }
            }
        }

        Ok(())
    }

    fn read_header<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        let mut buf = [0u8; std::mem::size_of::<W3DHierarchyHeader>()];
        reader.read_exact(&mut buf)?;

        let version = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let name_bytes = &buf[4..20];
        let num_pivots = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]) as usize;

        // Extract name
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        self.name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        self.num_pivots = num_pivots;
        self.pivots = Vec::with_capacity(num_pivots);

        Ok(())
    }

    fn read_pivots<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let count = size as usize / std::mem::size_of::<W3DPivot>();

        for _ in 0..count {
            let mut buf = [0u8; std::mem::size_of::<W3DPivot>()];
            reader.read_exact(&mut buf)?;

            let name_bytes = &buf[0..16];
            let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
            let name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

            let parent_idx = i32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);

            let tx = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
            let ty = f32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
            let tz = f32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);

            let qx = f32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]);
            let qy = f32::from_le_bytes([buf[48], buf[49], buf[50], buf[51]]);
            let qz = f32::from_le_bytes([buf[52], buf[53], buf[54], buf[55]]);
            let qw = f32::from_le_bytes([buf[56], buf[57], buf[58], buf[59]]);

            let mut pivot = Pivot::new(name, parent_idx);
            pivot.translation = Vec3::new(tx, ty, tz);
            pivot.rotation = UnitQuat::from_quaternion(Quat::new(qw, qx, qy, qz));

            self.pivots.push(pivot);
        }

        Ok(())
    }
}

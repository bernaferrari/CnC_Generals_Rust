use cgmath::{Matrix4, Vector3};

/// A single bone node in the skeletal hierarchy used by the W3D bone pipeline.
/// This mirrors the C++ PivotClass/PivotAnim structure used by the parity layer.
#[derive(Debug, Clone)]
pub struct BoneNode {
    pub name: String,
    pub parent_index: Option<usize>,
    pub base_transform: Matrix4<f32>,
    pub transform: Matrix4<f32>,
    pub cap_transform: Matrix4<f32>,
    pub is_captured: bool,
    pub world_space_translation: bool,
}

/// Hierarchy container for bones with simple capture/control plumbing.
#[derive(Debug, Clone)]
pub struct HTree {
    pub bones: Vec<BoneNode>,
    pub scale_factor: f32,
}

impl HTree {
    pub fn new() -> Self {
        // Initialize a small, default bone tree sufficient for parity wiring.
        // We expose a fixed 8-bone skeleton to cover turret and recoil bones used by parity layer.
        let mut bones: Vec<BoneNode> = Vec::new();
        for i in 0..8 {
            bones.push(BoneNode {
                name: format!("bone{}", i),
                parent_index: None,
                base_transform: Matrix4::from_scale(1.0),
                transform: Matrix4::from_scale(1.0),
                cap_transform: Matrix4::from_translation(Vector3::new(0.0, 0.0, 0.0)),
                is_captured: false,
                world_space_translation: false,
            });
        }
        HTree {
            bones,
            scale_factor: 1.0,
        }
    }

    pub fn capture_bone(&mut self, index: usize) {
        if let Some(b) = self.bones.get_mut(index) {
            b.is_captured = true;
        }
    }

    pub fn release_bone(&mut self, index: usize) {
        if let Some(b) = self.bones.get_mut(index) {
            b.is_captured = false;
        }
    }

    pub fn control_bone(
        &mut self,
        index: usize,
        relative_tm: Matrix4<f32>,
        world_space_translation: bool,
    ) {
        if let Some(b) = self.bones.get_mut(index) {
            b.cap_transform = relative_tm;
            b.world_space_translation = world_space_translation;
        }
    }

    pub fn is_bone_captured(&self, index: usize) -> bool {
        self.bones
            .get(index)
            .map(|b| b.is_captured)
            .unwrap_or(false)
    }

    /// Basic base update: recompute transforms relative to parents.
    fn base_update(&mut self) {
        for i in 1..self.bones.len() {
            if let Some(parent) = self.bones[i].parent_index {
                let parent_transform = self.bones[parent].transform;
                let base = self.bones[i].base_transform;
                self.bones[i].transform = parent_transform * base;
            } else {
                self.bones[i].transform = self.bones[i].base_transform;
            }
        }
    }

    /// Capture_Update placeholder: apply CapTransform to the current transform.
    fn capture_update(&mut self, index: usize) {
        if let Some(b) = self.bones.get_mut(index) {
            // In the real parity path we'd handle WorldSpaceTranslation specially.
            // For now, simply post-multiply by CapTransform to carry overrides forward.
            b.transform = b.transform * b.cap_transform;
        }
    }

    /// Convenience lookup by bone name (case-insensitive).
    pub fn get_bone_index(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
    }

    /// Retrieve final bone transform for uploading to the GPU.
    pub fn get_bone_transform(&self, index: usize) -> Matrix4<f32> {
        self.bones
            .get(index)
            .map(|b| b.transform)
            .unwrap_or_else(|| Matrix4::from_scale(1.0))
    }

    /// Convert all bone transforms to a flat f32 array suitable for WGPU uniform buffer upload.
    /// Each bone produces 16 f32 values (4x4 column-major matrix).
    pub fn to_uniform_data(&self) -> Vec<f32> {
        let mut data = Vec::with_capacity(self.bones.len() * 16);
        for bone in &self.bones {
            let m: &[[f32; 4]; 4] = bone.transform.as_ref();
            for col in m {
                for &val in col {
                    data.push(val);
                }
            }
        }
        data
    }

    /// Perform a full update: base pose + capture overrides.
    /// C++ Reference: HTreeClass::Base_Update (htree.cpp) + Capture_Update per captured bone.
    pub fn full_update(&mut self) {
        self.base_update();
        for i in 0..self.bones.len() {
            if self.bones[i].is_captured {
                self.capture_update(i);
            }
        }
    }

    /// Expand the bone array to hold at least `count` bones.
    /// New bones are initialized with identity transforms.
    pub fn ensure_bone_count(&mut self, count: usize) {
        while self.bones.len() < count {
            self.bones.push(BoneNode {
                name: format!("bone{}", self.bones.len()),
                parent_index: None,
                base_transform: Matrix4::from_scale(1.0),
                transform: Matrix4::from_scale(1.0),
                cap_transform: Matrix4::from_scale(1.0),
                is_captured: false,
                world_space_translation: false,
            });
        }
    }
}

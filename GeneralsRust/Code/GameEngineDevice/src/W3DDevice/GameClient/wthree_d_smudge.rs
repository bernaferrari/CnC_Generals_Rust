use glam::{Vec2, Vec3};

pub const SMUDGE_DRAW_SIZE: usize = 500;
pub const SMUDGE_VERTICES_PER_DECAL: usize = 5;
pub const SMUDGE_INDICES_PER_DECAL: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmudgeVertex {
    pub position: Vec3,
    pub uv: Vec2,
    pub color: u32,
}

impl Default for SmudgeVertex {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            uv: Vec2::ZERO,
            color: 0xffff_ffff,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SmudgeDecal {
    pub center: Vec3,
    pub radius: f32,
    pub rotation: f32,
    pub intensity: f32,
    pub creation_frame: u64,
    pub lifetime_frames: u64,
    pub vertices: [SmudgeVertex; SMUDGE_VERTICES_PER_DECAL],
}

impl SmudgeDecal {
    pub fn new(
        center: Vec3,
        radius: f32,
        rotation: f32,
        intensity: f32,
        terrain_height: impl Fn(f32, f32) -> f32,
        creation_frame: u64,
        lifetime_frames: u64,
    ) -> Self {
        let alpha = (intensity.clamp(0.0, 1.0) * 255.0).round() as u32;
        let color = (alpha << 24) | 0x0040_3020;
        let sin_r = rotation.sin();
        let cos_r = rotation.cos();

        let rotate =
            |x: f32, y: f32| -> Vec2 { Vec2::new(x * cos_r - y * sin_r, x * sin_r + y * cos_r) };

        let samples = [
            (Vec2::new(-radius, radius), Vec2::new(0.0, 0.0)),
            (Vec2::new(-radius, -radius), Vec2::new(0.0, 1.0)),
            (Vec2::new(radius, -radius), Vec2::new(1.0, 1.0)),
            (Vec2::new(radius, radius), Vec2::new(1.0, 0.0)),
            (Vec2::ZERO, Vec2::new(0.5, 0.5)),
        ];

        let mut vertices = [SmudgeVertex::default(); SMUDGE_VERTICES_PER_DECAL];
        for (index, (offset, uv)) in samples.into_iter().enumerate() {
            let rotated = rotate(offset.x, offset.y);
            let x = center.x + rotated.x;
            let y = center.y + rotated.y;
            vertices[index] = SmudgeVertex {
                position: Vec3::new(x, y, terrain_height(x, y) + 0.05),
                uv,
                color,
            };
        }

        Self {
            center,
            radius,
            rotation,
            intensity: intensity.clamp(0.0, 1.0),
            creation_frame,
            lifetime_frames: lifetime_frames.max(1),
            vertices,
        }
    }

    pub fn alpha_at_frame(&self, frame: u64) -> f32 {
        let age = frame.saturating_sub(self.creation_frame) as f32;
        let ttl = self.lifetime_frames as f32;
        (1.0 - age / ttl).clamp(0.0, 1.0) * self.intensity
    }

    pub fn is_expired(&self, frame: u64) -> bool {
        frame.saturating_sub(self.creation_frame) >= self.lifetime_frames
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmudgeHardwareSupport {
    Unknown,
    No,
    Yes,
}

#[derive(Debug, Clone)]
pub struct SmudgeBatch {
    pub vertices: Vec<SmudgeVertex>,
    pub indices: Vec<u16>,
}

#[derive(Debug, Default)]
pub struct W3DSmudgeManager {
    smudges: Vec<SmudgeDecal>,
    index_buffer: Vec<u16>,
    back_buffer_width: u32,
    back_buffer_height: u32,
    hardware_support: SmudgeHardwareSupport,
}

impl W3DSmudgeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn init(&mut self, back_buffer_width: u32, back_buffer_height: u32) {
        self.back_buffer_width = back_buffer_width;
        self.back_buffer_height = back_buffer_height;
        self.re_acquire_resources();
    }

    pub fn reset(&mut self) {
        self.smudges.clear();
    }

    pub fn release_resources(&mut self) {
        self.index_buffer.clear();
    }

    pub fn re_acquire_resources(&mut self) {
        self.release_resources();
        self.index_buffer
            .reserve(SMUDGE_DRAW_SIZE * SMUDGE_INDICES_PER_DECAL);
        let mut vb_base = 0u16;
        for _ in 0..SMUDGE_DRAW_SIZE {
            self.index_buffer.extend_from_slice(&[
                vb_base,
                vb_base + 4,
                vb_base + 3,
                vb_base + 3,
                vb_base + 4,
                vb_base + 2,
                vb_base + 2,
                vb_base + 4,
                vb_base + 1,
                vb_base + 1,
                vb_base + 4,
                vb_base,
            ]);
            vb_base += SMUDGE_VERTICES_PER_DECAL as u16;
        }
    }

    pub fn test_hardware_support(
        &mut self,
        render_to_texture: bool,
        copy_rect_matches: bool,
    ) -> bool {
        self.hardware_support = if render_to_texture && copy_rect_matches {
            SmudgeHardwareSupport::Yes
        } else {
            SmudgeHardwareSupport::No
        };
        self.hardware_support == SmudgeHardwareSupport::Yes
    }

    pub fn add_smudge(
        &mut self,
        center: Vec3,
        radius: f32,
        rotation: f32,
        intensity: f32,
        terrain_height: impl Fn(f32, f32) -> f32,
        creation_frame: u64,
        lifetime_frames: u64,
    ) {
        self.smudges.push(SmudgeDecal::new(
            center,
            radius.max(0.1),
            rotation,
            intensity,
            terrain_height,
            creation_frame,
            lifetime_frames,
        ));
    }

    pub fn render(&mut self, frame: u64) -> Vec<SmudgeBatch> {
        self.smudges.retain(|smudge| !smudge.is_expired(frame));

        let mut batches = Vec::new();
        for chunk in self.smudges.chunks(SMUDGE_DRAW_SIZE) {
            let mut vertices = Vec::with_capacity(chunk.len() * SMUDGE_VERTICES_PER_DECAL);
            let mut indices = Vec::with_capacity(chunk.len() * SMUDGE_INDICES_PER_DECAL);
            for (chunk_index, smudge) in chunk.iter().enumerate() {
                let vertex_base = (chunk_index * SMUDGE_VERTICES_PER_DECAL) as u16;
                let alpha = (smudge.alpha_at_frame(frame) * 255.0).round() as u32;
                for vertex in smudge.vertices {
                    vertices.push(SmudgeVertex {
                        color: (alpha << 24) | (vertex.color & 0x00ff_ffff),
                        ..vertex
                    });
                }
                let ib_offset = chunk_index * SMUDGE_INDICES_PER_DECAL;
                indices.extend(
                    self.index_buffer[ib_offset..ib_offset + SMUDGE_INDICES_PER_DECAL]
                        .iter()
                        .map(|index| index + vertex_base),
                );
            }
            batches.push(SmudgeBatch { vertices, indices });
        }
        batches
    }

    pub fn smudge_count(&self) -> usize {
        self.smudges.len()
    }

    pub fn hardware_support(&self) -> SmudgeHardwareSupport {
        self.hardware_support
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_projected_quad_over_terrain() {
        let smudge = SmudgeDecal::new(
            Vec3::new(10.0, 20.0, 0.0),
            4.0,
            0.0,
            1.0,
            |x, y| x + y,
            0,
            100,
        );
        assert_eq!(smudge.vertices[4].uv, Vec2::new(0.5, 0.5));
        assert!(smudge
            .vertices
            .iter()
            .all(|vertex| vertex.position.z >= 30.0));
    }

    #[test]
    fn batches_and_fades_smudges() {
        let mut manager = W3DSmudgeManager::new();
        manager.init(1280, 720);
        manager.add_smudge(Vec3::ZERO, 3.0, 0.0, 1.0, |_, _| 0.0, 0, 10);
        let batches = manager.render(5);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].vertices.len(), 5);
        assert_eq!(batches[0].indices.len(), 12);
        let alpha = batches[0].vertices[0].color >> 24;
        assert!(alpha < 255 && alpha > 0);
    }
}

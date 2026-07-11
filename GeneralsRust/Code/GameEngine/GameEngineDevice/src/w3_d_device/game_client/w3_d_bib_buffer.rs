//! W3D bib buffer compatibility state.
//!
//! C++ source: `GameEngineDevice/Source/W3DDevice/GameClient/W3DBibBuffer.cpp`.
//! Bibs are terrain-aligned quads drawn in one dynamic vertex/index buffer, with
//! normal bibs emitted before highlighted bibs so C++ can draw the two texture
//! ranges separately.

/// C++ object id type used by bib ownership.
pub type ObjectId = u32;

/// C++ drawable id type used by bib ownership.
pub type DrawableId = u32;

/// Invalid C++ object id.
pub const INVALID_ID: ObjectId = 0;

/// Invalid C++ drawable id.
pub const INVALID_DRAWABLE_ID: DrawableId = 0;

/// Initial C++ vertex-buffer capacity.
pub const INITIAL_BIB_VERTEX: usize = 256;

/// Initial C++ index-buffer capacity.
pub const INITIAL_BIB_INDEX: usize = 384;

/// Maximum number of bib records.
pub const MAX_BIBS: usize = 1_000;

/// C++ `Vector3` subset used for bib corners.
pub type Vector3 = [f32; 3];

/// C++ `TBib`.
#[derive(Debug, Clone, PartialEq)]
pub struct Bib {
    /// Drawing corners in C++ order.
    pub corners: [Vector3; 4],
    /// Whether to use the highlight texture.
    pub highlight: bool,
    /// Reserved tint value. C++ writes zero.
    pub color: i32,
    /// Owning object id.
    pub object_id: ObjectId,
    /// Owning drawable id.
    pub drawable_id: DrawableId,
    /// True when the slot is available for reuse.
    pub unused: bool,
}

impl Default for Bib {
    fn default() -> Self {
        Self {
            corners: [[0.0; 3]; 4],
            highlight: false,
            color: 0,
            object_id: INVALID_ID,
            drawable_id: INVALID_DRAWABLE_ID,
            unused: true,
        }
    }
}

/// C++ `VertexFormatXYZDUV1` subset emitted for bib rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BibVertex {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Z coordinate.
    pub z: f32,
    /// Packed diffuse color, `0xAARRGGBB`.
    pub diffuse: u32,
    /// U texture coordinate.
    pub u1: f32,
    /// V texture coordinate.
    pub v1: f32,
}

/// CPU representation of the dynamic bib vertex/index buffers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BibRenderBuffers {
    /// Combined normal and highlighted vertices.
    pub vertices: Vec<BibVertex>,
    /// Combined normal and highlighted indices.
    pub indices: Vec<u16>,
    /// Number of indices belonging to normal bibs.
    pub normal_index_count: usize,
    /// Number of vertices belonging to normal bibs.
    pub normal_vertex_count: usize,
}

/// Ambient and diffuse terrain lighting used by the C++ bib buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BibLighting {
    /// Terrain ambient RGB.
    pub ambient: [f32; 3],
    /// Terrain diffuse RGB.
    pub diffuse: [f32; 3],
}

impl BibLighting {
    /// Create lighting from ambient and diffuse RGB triples.
    #[must_use]
    pub const fn new(ambient: [f32; 3], diffuse: [f32; 3]) -> Self {
        Self { ambient, diffuse }
    }

    fn packed_diffuse(self) -> u32 {
        let r = ((self.ambient[0] + self.diffuse[0]).min(1.0) * 255.0) as u32;
        let g = ((self.ambient[1] + self.diffuse[1]).min(1.0) * 255.0) as u32;
        let b = ((self.ambient[2] + self.diffuse[2]).min(1.0) * 255.0) as u32;
        b | (g << 8) | (r << 16) | (255 << 24)
    }
}

impl Default for BibLighting {
    fn default() -> Self {
        Self {
            ambient: [0.0; 3],
            diffuse: [1.0; 3],
        }
    }
}

/// W3D bib draw buffer.
#[derive(Debug, Clone)]
pub struct W3DBibBuffer {
    vertex_bib_size: usize,
    index_bib_size: usize,
    cur_num_bib_vertices: usize,
    cur_num_bib_indices: usize,
    cur_num_normal_bib_indices: usize,
    cur_num_normal_bib_vertex: usize,
    bibs: Vec<Bib>,
    anything_changed: bool,
    initialized: bool,
    cached_buffers: BibRenderBuffers,
}

impl W3DBibBuffer {
    /// Construct a bib buffer with the C++ initial buffer sizes.
    #[must_use]
    pub fn new() -> Self {
        let mut buffer = Self {
            vertex_bib_size: INITIAL_BIB_VERTEX,
            index_bib_size: INITIAL_BIB_INDEX,
            cur_num_bib_vertices: 0,
            cur_num_bib_indices: 0,
            cur_num_normal_bib_indices: 0,
            cur_num_normal_bib_vertex: 0,
            bibs: Vec::new(),
            anything_changed: true,
            initialized: false,
            cached_buffers: BibRenderBuffers::default(),
        };
        buffer.allocate_bib_buffers();
        buffer.clear_all_bibs();
        buffer.initialized = true;
        buffer
    }

    /// Construct with custom buffer sizes, useful for exact capacity tests.
    #[must_use]
    pub fn with_buffer_sizes(vertex_bib_size: usize, index_bib_size: usize) -> Self {
        let mut buffer = Self::new();
        buffer.vertex_bib_size = vertex_bib_size;
        buffer.index_bib_size = index_bib_size;
        buffer.allocate_bib_buffers();
        buffer
    }

    /// C++ `clearAllBibs`.
    pub fn clear_all_bibs(&mut self) {
        self.bibs.clear();
        self.anything_changed = true;
        self.cached_buffers = BibRenderBuffers::default();
    }

    /// C++ `removeHighlighting`.
    pub fn remove_highlighting(&mut self) {
        for bib in &mut self.bibs {
            bib.highlight = false;
        }
    }

    /// C++ `addBib`.
    pub fn add_bib(&mut self, corners: [Vector3; 4], id: ObjectId, highlight: bool) {
        let index = self.find_or_allocate_slot(|bib| !bib.unused && bib.object_id == id);
        if let Some(index) = index {
            self.write_bib(index, corners, highlight, id, INVALID_DRAWABLE_ID);
        }
    }

    /// C++ `addBibDrawable`.
    pub fn add_bib_drawable(&mut self, corners: [Vector3; 4], id: DrawableId, highlight: bool) {
        let index = self.find_or_allocate_slot(|bib| !bib.unused && bib.drawable_id == id);
        if let Some(index) = index {
            self.write_bib(index, corners, highlight, INVALID_ID, id);
        }
    }

    /// C++ `removeBib`.
    pub fn remove_bib(&mut self, id: ObjectId) {
        for bib in &mut self.bibs {
            if bib.object_id == id {
                bib.unused = true;
                bib.object_id = INVALID_ID;
                bib.drawable_id = INVALID_DRAWABLE_ID;
                self.anything_changed = true;
            }
        }
    }

    /// C++ `removeBibDrawable`.
    pub fn remove_bib_drawable(&mut self, id: DrawableId) {
        for bib in &mut self.bibs {
            if bib.drawable_id == id {
                bib.unused = true;
                bib.object_id = INVALID_ID;
                bib.drawable_id = INVALID_DRAWABLE_ID;
                self.anything_changed = true;
            }
        }
    }

    /// C++ `renderBibs` CPU buffer fill.
    pub fn render_bibs(&mut self, lighting: BibLighting) -> &BibRenderBuffers {
        self.load_bibs_in_vertex_and_index_buffers(lighting);
        &self.cached_buffers
    }

    /// Number of allocated bib slots, matching C++ `m_numBibs`.
    #[must_use]
    pub fn num_bibs(&self) -> usize {
        self.bibs.len()
    }

    /// Access current bib records.
    #[must_use]
    pub fn bibs(&self) -> &[Bib] {
        &self.bibs
    }

    /// Return whether the buffer is initialized.
    #[must_use]
    pub fn initialized(&self) -> bool {
        self.initialized
    }

    fn allocate_bib_buffers(&mut self) {
        self.cur_num_bib_vertices = 0;
        self.cur_num_bib_indices = 0;
        self.cached_buffers = BibRenderBuffers {
            vertices: Vec::with_capacity(self.vertex_bib_size + 4),
            indices: Vec::with_capacity(self.index_bib_size + 4),
            normal_index_count: 0,
            normal_vertex_count: 0,
        };
    }

    fn find_or_allocate_slot<F>(&mut self, matches_existing: F) -> Option<usize>
    where
        F: Fn(&Bib) -> bool,
    {
        if let Some(index) = self.bibs.iter().position(matches_existing) {
            return Some(index);
        }
        if let Some(index) = self.bibs.iter().position(|bib| bib.unused) {
            return Some(index);
        }
        if self.bibs.len() >= MAX_BIBS {
            return None;
        }
        self.bibs.push(Bib::default());
        Some(self.bibs.len() - 1)
    }

    fn write_bib(
        &mut self,
        index: usize,
        corners: [Vector3; 4],
        highlight: bool,
        object_id: ObjectId,
        drawable_id: DrawableId,
    ) {
        self.anything_changed = true;
        self.bibs[index] = Bib {
            corners,
            highlight,
            color: 0,
            object_id,
            drawable_id,
            unused: false,
        };
    }

    fn load_bibs_in_vertex_and_index_buffers(&mut self, lighting: BibLighting) {
        if !self.initialized || !self.anything_changed {
            return;
        }

        self.cur_num_bib_vertices = 0;
        self.cur_num_bib_indices = 0;
        self.cur_num_normal_bib_indices = 0;
        self.cur_num_normal_bib_vertex = 0;
        self.cached_buffers.vertices.clear();
        self.cached_buffers.indices.clear();
        self.cached_buffers.normal_index_count = 0;
        self.cached_buffers.normal_vertex_count = 0;

        if self.bibs.is_empty() {
            self.anything_changed = false;
            return;
        }

        let diffuse = lighting.packed_diffuse();
        for do_highlight in [false, true] {
            if do_highlight {
                self.cur_num_normal_bib_indices = self.cur_num_bib_indices;
                self.cur_num_normal_bib_vertex = self.cur_num_bib_vertices;
                self.cached_buffers.normal_index_count = self.cur_num_normal_bib_indices;
                self.cached_buffers.normal_vertex_count = self.cur_num_normal_bib_vertex;
            }

            for bib in &self.bibs {
                if bib.unused || bib.highlight != do_highlight {
                    continue;
                }
                if self.cur_num_bib_vertices + 4 + 2 >= self.vertex_bib_size {
                    break;
                }
                if self.cur_num_bib_indices + 6 + 6 >= self.index_bib_size {
                    break;
                }

                append_bib_quad(&mut self.cached_buffers, bib, diffuse);
                self.cur_num_bib_vertices += 4;
                self.cur_num_bib_indices += 6;
            }
        }

        self.anything_changed = false;
    }
}

impl Default for W3DBibBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn append_bib_quad(buffers: &mut BibRenderBuffers, bib: &Bib, diffuse: u32) {
    let start_vertex = buffers.vertices.len() as u16;
    let uvs = [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];

    for (corner, (u1, v1)) in bib.corners.iter().zip(uvs) {
        buffers.vertices.push(BibVertex {
            x: corner[0],
            y: corner[1],
            z: corner[2],
            diffuse,
            u1,
            v1,
        });
    }

    buffers.indices.extend_from_slice(&[
        start_vertex,
        start_vertex + 1,
        start_vertex + 2,
        start_vertex,
        start_vertex + 2,
        start_vertex + 3,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corners(offset: f32) -> [Vector3; 4] {
        [
            [offset, 0.0, 2.0],
            [offset + 10.0, 0.0, 2.0],
            [offset + 10.0, 10.0, 2.0],
            [offset, 10.0, 2.0],
        ]
    }

    fn lighting() -> BibLighting {
        BibLighting::new([0.25, 0.5, 0.75], [0.25, 0.25, 0.5])
    }

    #[test]
    fn constructor_matches_cpp_initial_state() {
        let buffer = W3DBibBuffer::new();

        assert!(buffer.initialized());
        assert_eq!(buffer.num_bibs(), 0);
    }

    #[test]
    fn add_bib_updates_existing_object_slot() {
        let mut buffer = W3DBibBuffer::new();
        buffer.add_bib(corners(0.0), 42, false);
        buffer.add_bib(corners(100.0), 42, true);

        assert_eq!(buffer.num_bibs(), 1);
        assert_eq!(buffer.bibs()[0].corners, corners(100.0));
        assert!(buffer.bibs()[0].highlight);
        assert_eq!(buffer.bibs()[0].object_id, 42);
        assert_eq!(buffer.bibs()[0].drawable_id, INVALID_DRAWABLE_ID);
    }

    #[test]
    fn add_bib_drawable_sets_object_invalid_and_drawable_id() {
        let mut buffer = W3DBibBuffer::new();
        buffer.add_bib_drawable(corners(0.0), 7, false);

        assert_eq!(buffer.bibs()[0].object_id, INVALID_ID);
        assert_eq!(buffer.bibs()[0].drawable_id, 7);
    }

    #[test]
    fn remove_bib_marks_slot_unused_and_reused_by_next_add() {
        let mut buffer = W3DBibBuffer::new();
        buffer.add_bib(corners(0.0), 1, false);
        buffer.add_bib(corners(20.0), 2, false);

        buffer.remove_bib(1);
        buffer.add_bib(corners(40.0), 3, true);

        assert_eq!(buffer.num_bibs(), 2);
        assert_eq!(buffer.bibs()[0].object_id, 3);
        assert!(buffer.bibs()[0].highlight);
    }

    #[test]
    fn remove_highlighting_clears_all_flags_without_removing_bibs() {
        let mut buffer = W3DBibBuffer::new();
        buffer.add_bib(corners(0.0), 1, true);
        buffer.add_bib(corners(20.0), 2, true);

        buffer.remove_highlighting();

        assert_eq!(buffer.num_bibs(), 2);
        assert!(buffer.bibs().iter().all(|bib| !bib.highlight));
    }

    #[test]
    fn render_orders_normal_bibs_before_highlight_bibs_and_records_split_counts() {
        let mut buffer = W3DBibBuffer::new();
        buffer.add_bib(corners(0.0), 1, true);
        buffer.add_bib(corners(20.0), 2, false);

        let rendered = buffer.render_bibs(lighting()).clone();

        assert_eq!(rendered.vertices.len(), 8);
        assert_eq!(rendered.indices, vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]);
        assert_eq!(rendered.normal_vertex_count, 4);
        assert_eq!(rendered.normal_index_count, 6);
        assert_eq!(rendered.vertices[0].x, 20.0);
        assert_eq!(rendered.vertices[4].x, 0.0);
    }

    #[test]
    fn render_vertices_match_cpp_uvs_and_packed_lighting() {
        let mut buffer = W3DBibBuffer::new();
        buffer.add_bib(corners(0.0), 1, false);

        let rendered = buffer.render_bibs(lighting()).clone();

        let diffuse = 0xff7fbfff;
        assert_eq!(
            rendered.vertices,
            vec![
                BibVertex {
                    x: 0.0,
                    y: 0.0,
                    z: 2.0,
                    diffuse,
                    u1: 0.0,
                    v1: 1.0,
                },
                BibVertex {
                    x: 10.0,
                    y: 0.0,
                    z: 2.0,
                    diffuse,
                    u1: 1.0,
                    v1: 1.0,
                },
                BibVertex {
                    x: 10.0,
                    y: 10.0,
                    z: 2.0,
                    diffuse,
                    u1: 1.0,
                    v1: 0.0,
                },
                BibVertex {
                    x: 0.0,
                    y: 10.0,
                    z: 2.0,
                    diffuse,
                    u1: 0.0,
                    v1: 0.0,
                },
            ]
        );
    }

    #[test]
    fn render_honors_cpp_vertex_and_index_capacity_checks() {
        let mut buffer = W3DBibBuffer::with_buffer_sizes(14, 24);
        for id in 1..=4 {
            buffer.add_bib(corners(id as f32), id, false);
        }

        let rendered = buffer.render_bibs(BibLighting::default()).clone();

        assert_eq!(rendered.vertices.len(), 8);
        assert_eq!(rendered.indices.len(), 12);
    }

    #[test]
    fn max_bibs_cap_matches_cpp() {
        let mut buffer = W3DBibBuffer::new();
        for id in 1..=(MAX_BIBS as u32 + 1) {
            buffer.add_bib(corners(id as f32), id, false);
        }

        assert_eq!(buffer.num_bibs(), MAX_BIBS);
    }
}

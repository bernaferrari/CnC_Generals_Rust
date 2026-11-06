//! Per-triangle sorting renderer (C++ `SortingRendererClass`,
//! `WW3D2/sortingrenderer.cpp`).
//!
//! Translucent passes of sorting meshes are not drawn when they are inserted.
//! `Flush` pools every queued node, computes each triangle's view-space depth
//! at its centroid, sorts all triangles of all nodes together back to front,
//! and draws each run of consecutive triangles that belong to one node with
//! that node's render state (`Flush_Sorting_Pool`). Triangles of different
//! meshes therefore interleave by depth instead of whole meshes being ordered.
//!
//! C++ `SortingRendererClass` is a static class; here every renderer owns its
//! own [`SortingRenderer`] so two game instances never flush each other's
//! triangles.

use std::ops::Range;
use std::sync::Arc;

/// CPU geometry of one inserted draw, read by the depth sort.
#[derive(Clone)]
pub struct SortingGeometry {
    /// Model-space positions, numbered like the node's vertex buffer.
    pub positions: Arc<[glam::Vec3]>,
    /// Triangle list indices into `positions` / the vertex buffer.
    pub indices: Arc<[u32]>,
    /// The node's triangles within `indices` (multiple of 3 long).
    pub index_range: Range<u32>,
    /// Model to view transform at insertion (C++ `sorting_state.world *
    /// sorting_state.view`).
    pub world_view: glam::Mat4,
}

impl SortingGeometry {
    fn triangle_count(&self) -> u32 {
        (self.index_range.end.saturating_sub(self.index_range.start)) / 3
    }
}

/// Render state a node was inserted with (C++ `RenderStateStruct`).
#[derive(Clone)]
pub struct SortingState {
    pub vertex_buffer: Arc<wgpu::Buffer>,
    pub pipeline: Arc<wgpu::RenderPipeline>,
    pub bind_groups: Vec<Arc<wgpu::BindGroup>>,
    /// Dynamic offsets per bind group (empty = none), parallel to `bind_groups`.
    pub bind_group_offsets: Vec<Vec<u32>>,
}

/// One inserted translucent draw (C++ `SortingNodeStruct`).
#[derive(Clone)]
pub struct SortingNode {
    pub state: SortingState,
    pub geometry: SortingGeometry,
}

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32-only. The `Arc<wgpu::*>` handles in `SortingState` are
// `Rc`-backed and `!Send` on the web backend, but wasm32 has no threads, so
// they can never move to or be observed from another thread. The impl only
// satisfies the `Send` bound of the renderer's `Vec<SortingNode>`.
unsafe impl Send for SortingNode {}
#[cfg(target_arch = "wasm32")]
// SAFETY: same wasm32-only scope: the target is single-threaded, so
// `&SortingNode` is never accessed from two threads and the handles' `!Sync`
// default is unreachable.
unsafe impl Sync for SortingNode {}

/// C++ `Flush_Sorting_Pool` order over `geometries`: every triangle, farthest
/// first, as (geometry index, first index) pairs.
///
/// Depth is the view-space z of the triangle centroid (C++ `tis_ptr->z`);
/// W3D views look down -Z, so ascending z is back to front, like the C++
/// `Sort` with `TempIndexStruct::operator<`. Equal depths keep insertion order.
pub fn sorted_triangle_order<'a>(
    geometries: impl IntoIterator<Item = &'a SortingGeometry>,
) -> Vec<(usize, u32)> {
    let mut pool = Vec::new();
    pool_triangles(geometries, &mut pool);
    pool.iter()
        .map(|triangle| (triangle.node as usize, triangle.first_index))
        .collect()
}

fn pool_triangles<'a>(
    geometries: impl IntoIterator<Item = &'a SortingGeometry>,
    pool: &mut Vec<PooledTriangle>,
) {
    pool.clear();
    for (node_index, geometry) in geometries.into_iter().enumerate() {
        let depth_row = geometry.world_view.row(2);
        let range = geometry.index_range.start as usize..geometry.index_range.end as usize;
        let Some(indices) = geometry.indices.get(range) else {
            continue;
        };
        for (triangle, corners) in indices.chunks_exact(3).enumerate() {
            let Some(sum) = corners.iter().try_fold(glam::Vec3::ZERO, |sum, &index| {
                geometry.positions.get(index as usize).map(|position| sum + *position)
            }) else {
                continue;
            };
            let centroid = sum / 3.0;
            pool.push(PooledTriangle {
                node: node_index as u32,
                first_index: geometry.index_range.start + triangle as u32 * 3,
                depth: depth_row.dot(centroid.extend(1.0)),
            });
        }
    }
    // Back to front; NaN depths compare equal (C++ asserts they never occur).
    pool.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(std::cmp::Ordering::Equal));
}

/// C++ `TempIndexStruct`: one pooled triangle and its sort depth.
#[derive(Clone, Copy)]
struct PooledTriangle {
    node: u32,
    /// Offset of the triangle's first index in its node's `indices`.
    first_index: u32,
    /// View-space z of the centroid.
    depth: f32,
}

/// Smallest sorted index buffer, in bytes.
const MIN_INDEX_BUFFER_BYTES: u64 = 64 * 1024;

/// Sorting renderer for translucent triangles.
pub struct SortingRenderer {
    nodes: Vec<SortingNode>,
    pool: Vec<PooledTriangle>,
    sorted_indices: Vec<u32>,
    /// Grow-only sorted index buffer; flushes within a frame append at
    /// `index_cursor` so a later flush never overwrites indices an earlier
    /// one's draws still read.
    index_buffer: Option<wgpu::Buffer>,
    index_cursor: u64,
    enable_triangle_draw: bool,
}

#[cfg(target_arch = "wasm32")]
// SAFETY: wasm32-only. `nodes` and `index_buffer` hold `!Send` wgpu handles
// on the web backend; wasm32 has no threads, so they can never move to or be
// observed from another thread. Native keeps the auto traits.
unsafe impl Send for SortingRenderer {}
#[cfg(target_arch = "wasm32")]
// SAFETY: same wasm32-only scope: the target is single-threaded, so
// `&SortingRenderer` is never accessed from two threads.
unsafe impl Sync for SortingRenderer {}

impl Default for SortingRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl SortingRenderer {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            pool: Vec::new(),
            sorted_indices: Vec::new(),
            index_buffer: None,
            index_cursor: 0,
            enable_triangle_draw: true,
        }
    }

    /// Start a frame: sorted indices are written from the buffer start again.
    pub fn begin_frame(&mut self) {
        self.index_cursor = 0;
    }

    /// C++ `Insert_Triangles`: queue a node for the next flush.
    pub fn insert_triangles(&mut self, node: SortingNode) {
        if node.geometry.triangle_count() > 0 {
            self.nodes.push(node);
        }
    }

    /// Queued nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Queued triangles over every node.
    pub fn triangle_count(&self) -> usize {
        self.nodes
            .iter()
            .map(|node| node.geometry.triangle_count() as usize)
            .sum()
    }

    /// C++ `_EnableTriangleDraw`.
    pub fn set_triangle_draw_enabled(&mut self, enabled: bool) {
        self.enable_triangle_draw = enabled;
    }

    pub fn is_triangle_draw_enabled(&self) -> bool {
        self.enable_triangle_draw
    }

    /// Drop queued nodes without drawing.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.pool.clear();
    }

    /// Draw every queued triangle back to front into `pass`, then clear.
    ///
    /// The sorted index stream goes into one grow-only index buffer; each
    /// run of triangles sharing a node is one indexed draw against that
    /// node's vertex buffer (C++ `Flush_Sorting_Pool`).
    pub fn flush(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if !self.enable_triangle_draw || self.nodes.is_empty() {
            self.clear();
            return;
        }
        pool_triangles(self.nodes.iter().map(|node| &node.geometry), &mut self.pool);
        if self.pool.is_empty() {
            self.clear();
            return;
        }

        self.sorted_indices.clear();
        for triangle in &self.pool {
            let indices = &self.nodes[triangle.node as usize].geometry.indices;
            let first = triangle.first_index as usize;
            self.sorted_indices.extend_from_slice(&indices[first..first + 3]);
        }
        let bytes: &[u8] = bytemuck::cast_slice(&self.sorted_indices);
        let size = bytes.len() as wgpu::BufferAddress;
        let fits = self
            .index_buffer
            .as_ref()
            .is_some_and(|buffer| self.index_cursor + size <= buffer.size());
        if !fits {
            // The old buffer stays alive while earlier draws reference it.
            let needed = self.index_cursor.max(MIN_INDEX_BUFFER_BYTES / 2) + size;
            self.index_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Sorting Renderer Indices"),
                size: needed.next_power_of_two().max(MIN_INDEX_BUFFER_BYTES),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.index_cursor = 0;
        }
        let Some(index_buffer) = self.index_buffer.as_ref() else {
            self.clear();
            return;
        };
        let offset = self.index_cursor;
        queue.write_buffer(index_buffer, offset, bytes);
        // Index data is 4-byte aligned already; keep `write_buffer`'s alignment.
        self.index_cursor = offset + size.next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT);
        pass.set_index_buffer(index_buffer.slice(offset..offset + size), wgpu::IndexFormat::Uint32);

        let mut run_start = 0usize;
        while run_start < self.pool.len() {
            let node_index = self.pool[run_start].node;
            let run_end = self.pool[run_start..]
                .iter()
                .position(|triangle| triangle.node != node_index)
                .map_or(self.pool.len(), |offset| run_start + offset);
            let state = &self.nodes[node_index as usize].state;
            pass.set_pipeline(&state.pipeline);
            for (slot, bind_group) in state.bind_groups.iter().enumerate() {
                let offsets = state
                    .bind_group_offsets
                    .get(slot)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                pass.set_bind_group(slot as u32, bind_group.as_ref(), offsets);
            }
            pass.set_vertex_buffer(0, state.vertex_buffer.slice(..));
            pass.draw_indexed((run_start * 3) as u32..(run_end * 3) as u32, 0, 0..1);
            run_start = run_end;
        }
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_geometry(z: f32, world: glam::Mat4) -> SortingGeometry {
        // Two triangles, the second one 10 units farther along -Z.
        let positions: Arc<[glam::Vec3]> = Arc::from(vec![
            glam::Vec3::new(0.0, 0.0, z),
            glam::Vec3::new(1.0, 0.0, z),
            glam::Vec3::new(0.0, 1.0, z),
            glam::Vec3::new(0.0, 0.0, z - 10.0),
            glam::Vec3::new(1.0, 0.0, z - 10.0),
            glam::Vec3::new(0.0, 1.0, z - 10.0),
        ]);
        SortingGeometry {
            positions,
            indices: Arc::from(vec![0, 1, 2, 3, 4, 5]),
            index_range: 0..6,
            world_view: view() * world,
        }
    }

    fn view() -> glam::Mat4 {
        // Camera at the origin looking down -Z (W3D / right-handed view).
        glam::Mat4::look_at_rh(glam::Vec3::ZERO, glam::Vec3::NEG_Z, glam::Vec3::Y)
    }

    #[test]
    fn triangles_of_different_nodes_interleave_back_to_front() {
        // Node 0 spans depths 20 and 30, node 1 spans 25 and 35: C++ sorts the
        // pooled triangles, so the draw order alternates between the nodes.
        let near = quad_geometry(-20.0, glam::Mat4::IDENTITY);
        let far = quad_geometry(-25.0, glam::Mat4::IDENTITY);
        let order = sorted_triangle_order([&near, &far]);
        assert_eq!(order, vec![(1, 3), (0, 3), (1, 0), (0, 0)]);
    }

    #[test]
    fn world_transform_moves_triangles_in_depth() {
        let moved = quad_geometry(-20.0, glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -100.0)));
        let fixed = quad_geometry(-50.0, glam::Mat4::IDENTITY);
        let order = sorted_triangle_order([&fixed, &moved]);
        assert_eq!(order, vec![(1, 3), (1, 0), (0, 3), (0, 0)]);
    }

    #[test]
    fn equal_depths_keep_insertion_order() {
        let a = quad_geometry(-20.0, glam::Mat4::IDENTITY);
        let b = quad_geometry(-20.0, glam::Mat4::IDENTITY);
        let order = sorted_triangle_order([&a, &b]);
        assert_eq!(order, vec![(0, 3), (1, 3), (0, 0), (1, 0)]);
    }

    #[test]
    fn empty_renderer_reports_nothing_queued() {
        let mut renderer = SortingRenderer::new();
        assert_eq!(renderer.node_count(), 0);
        assert_eq!(renderer.triangle_count(), 0);
        renderer.clear();
        assert!(renderer.is_triangle_draw_enabled());
        renderer.set_triangle_draw_enabled(false);
        assert!(!renderer.is_triangle_draw_enabled());
    }
}

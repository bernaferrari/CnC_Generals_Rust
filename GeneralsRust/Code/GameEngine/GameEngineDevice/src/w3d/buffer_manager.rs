//! C++ `W3DBufferManager` — partial vertex/index buffer slot pool.
//!
//! Used by volumetric shadow `constructVolumeVB` to allocate same-size slots
//! from a small set of large GPU buffers and recycle them after device reset.

use std::sync::{LazyLock, Mutex};

pub const MAX_FVF: usize = 18;
pub const MAX_VB_SIZES: usize = 128;
pub const MIN_SLOT_SIZE: i32 = 32;
pub const MIN_SLOT_SIZE_SHIFT: i32 = 5;
pub const MAX_VERTEX_BUFFERS_CREATED: usize = 32;
pub const DEFAULT_VERTEX_BUFFER_SIZE: i32 = 8192;
pub const MAX_NUMBER_SLOTS: usize = 4096;
pub const MAX_IB_SIZES: usize = 128;
pub const MAX_INDEX_BUFFERS_CREATED: usize = 32;
pub const DEFAULT_INDEX_BUFFER_SIZE: i32 = 32768;

/// C++ `W3DBufferManager::VBM_FVF_TYPES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum VbmFvfType {
    Xyz = 0,
    Xyzd = 1,
    Xyzuv = 2,
    Xyzduv = 3,
    Xyzuv2 = 4,
    Xyzduv2 = 5,
    Xyzn = 6,
    Xyznd = 7,
    Xyznuv = 8,
    Xyznduv = 9,
    Xyznuv2 = 10,
    Xyznduv2 = 11,
    Xyzrhw = 12,
    Xyzrhwd = 13,
    Xyzrhwuv = 14,
    Xyzrhwduv = 15,
    Xyzrhwuv2 = 16,
    Xyzrhwduv2 = 17,
}

impl VbmFvfType {
    pub const fn stride_bytes(self) -> u32 {
        match self {
            Self::Xyz => 12,
            Self::Xyzd => 16,
            Self::Xyzuv => 20,
            Self::Xyzduv => 24,
            Self::Xyzuv2 => 28,
            Self::Xyzduv2 => 32,
            Self::Xyzn => 24,
            Self::Xyznd => 28,
            Self::Xyznuv => 32,
            Self::Xyznduv => 36,
            Self::Xyznuv2 => 40,
            Self::Xyznduv2 => 44,
            Self::Xyzrhw => 16,
            Self::Xyzrhwd => 20,
            Self::Xyzrhwuv => 24,
            Self::Xyzrhwduv => 28,
            Self::Xyzrhwuv2 => 32,
            Self::Xyzrhwduv2 => 36,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VertexBufferSlot {
    pub size: i32,
    pub start: i32,
    pub buffer_index: usize,
    pub fvf: VbmFvfType,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexBufferSlot {
    pub size: i32,
    pub start: i32,
    pub buffer_index: usize,
}

#[derive(Debug)]
struct VertexBuffer {
    format: VbmFvfType,
    start_free_index: i32,
    size: i32,
    cpu: Vec<u8>,
    render_tasks: Vec<usize>,
}

#[derive(Debug)]
struct IndexBuffer {
    start_free_index: i32,
    size: i32,
    cpu: Vec<u16>,
}

/// C++ `W3DBufferManager`.
pub struct W3DBufferManager {
    vb_free: [[Vec<VertexBufferSlot>; MAX_VB_SIZES]; MAX_FVF],
    vertex_buffers: [Vec<VertexBuffer>; MAX_FVF],
    ib_free: [Vec<IndexBufferSlot>; MAX_IB_SIZES],
    index_buffers: Vec<IndexBuffer>,
    slots_allocated: usize,
    index_slots_allocated: usize,
}

impl Default for W3DBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DBufferManager {
    pub fn new() -> Self {
        Self {
            vb_free: std::array::from_fn(|_| std::array::from_fn(|_| Vec::new())),
            vertex_buffers: std::array::from_fn(|_| Vec::new()),
            ib_free: std::array::from_fn(|_| Vec::new()),
            index_buffers: Vec::new(),
            slots_allocated: 0,
            index_slots_allocated: 0,
        }
    }

    pub fn get_vertex_slot(&mut self, fvf: VbmFvfType, mut size: i32) -> Option<VertexBufferSlot> {
        if size <= 0 {
            return None;
        }
        size = (size + (MIN_SLOT_SIZE - 1)) & !(MIN_SLOT_SIZE - 1);
        let size_index = ((size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;
        if size_index >= MAX_VB_SIZES {
            return None;
        }
        let fvf_i = fvf as usize;
        if let Some(slot) = self.vb_free[fvf_i][size_index].pop() {
            return Some(slot);
        }
        self.allocate_vertex_slot(fvf, size)
    }

    pub fn release_vertex_slot(&mut self, slot: VertexBufferSlot) {
        let size_index = ((slot.size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;
        if size_index < MAX_VB_SIZES {
            self.vb_free[slot.fvf as usize][size_index].push(slot);
        }
    }

    fn allocate_vertex_slot(&mut self, fvf: VbmFvfType, size: i32) -> Option<VertexBufferSlot> {
        if self.slots_allocated >= MAX_NUMBER_SLOTS {
            return None;
        }
        let fvf_i = fvf as usize;
        for (buffer_index, vb) in self.vertex_buffers[fvf_i].iter_mut().enumerate() {
            if vb.size - vb.start_free_index >= size {
                let slot = VertexBufferSlot {
                    size,
                    start: vb.start_free_index,
                    buffer_index,
                    fvf,
                };
                vb.start_free_index += size;
                self.slots_allocated += 1;
                return Some(slot);
            }
        }
        if self.vertex_buffers[fvf_i].len() >= MAX_VERTEX_BUFFERS_CREATED {
            return None;
        }
        let vb_size = DEFAULT_VERTEX_BUFFER_SIZE.max(size);
        let stride = fvf.stride_bytes() as usize;
        self.vertex_buffers[fvf_i].push(VertexBuffer {
            format: fvf,
            start_free_index: size,
            size: vb_size,
            cpu: vec![0u8; vb_size as usize * stride],
            render_tasks: Vec::new(),
        });
        self.slots_allocated += 1;
        Some(VertexBufferSlot {
            size,
            start: 0,
            buffer_index: self.vertex_buffers[fvf_i].len() - 1,
            fvf,
        })
    }

    pub fn get_index_slot(&mut self, mut size: i32) -> Option<IndexBufferSlot> {
        if size <= 0 {
            return None;
        }
        size = (size + (MIN_SLOT_SIZE - 1)) & !(MIN_SLOT_SIZE - 1);
        let size_index = ((size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;
        if size_index >= MAX_IB_SIZES {
            return None;
        }
        if let Some(slot) = self.ib_free[size_index].pop() {
            return Some(slot);
        }
        self.allocate_index_slot(size)
    }

    pub fn release_index_slot(&mut self, slot: IndexBufferSlot) {
        let size_index = ((slot.size >> MIN_SLOT_SIZE_SHIFT) - 1) as usize;
        if size_index < MAX_IB_SIZES {
            self.ib_free[size_index].push(slot);
        }
    }

    fn allocate_index_slot(&mut self, size: i32) -> Option<IndexBufferSlot> {
        if self.index_slots_allocated >= MAX_NUMBER_SLOTS {
            return None;
        }
        for (buffer_index, ib) in self.index_buffers.iter_mut().enumerate() {
            if ib.size - ib.start_free_index >= size {
                let slot = IndexBufferSlot {
                    size,
                    start: ib.start_free_index,
                    buffer_index,
                };
                ib.start_free_index += size;
                self.index_slots_allocated += 1;
                return Some(slot);
            }
        }
        if self.index_buffers.len() >= MAX_INDEX_BUFFERS_CREATED {
            return None;
        }
        let ib_size = DEFAULT_INDEX_BUFFER_SIZE.max(size);
        self.index_buffers.push(IndexBuffer {
            start_free_index: size,
            size: ib_size,
            cpu: vec![0u16; ib_size as usize],
        });
        self.index_slots_allocated += 1;
        Some(IndexBufferSlot {
            size,
            start: 0,
            buffer_index: self.index_buffers.len() - 1,
        })
    }

    pub fn write_xyz(&mut self, slot: &VertexBufferSlot, verts: &[[f32; 3]]) {
        let Some(vb) = self.vertex_buffers[slot.fvf as usize].get_mut(slot.buffer_index) else {
            return;
        };
        let stride = slot.fvf.stride_bytes() as usize;
        let start = slot.start as usize * stride;
        for (i, v) in verts.iter().enumerate() {
            let off = start + i * stride;
            if off + 12 > vb.cpu.len() {
                break;
            }
            vb.cpu[off..off + 4].copy_from_slice(&v[0].to_le_bytes());
            vb.cpu[off + 4..off + 8].copy_from_slice(&v[1].to_le_bytes());
            vb.cpu[off + 8..off + 12].copy_from_slice(&v[2].to_le_bytes());
        }
    }

    pub fn write_indices(&mut self, slot: &IndexBufferSlot, indices: &[u16]) {
        let Some(ib) = self.index_buffers.get_mut(slot.buffer_index) else {
            return;
        };
        let start = slot.start as usize;
        for (i, idx) in indices.iter().enumerate() {
            if start + i >= ib.cpu.len() {
                break;
            }
            ib.cpu[start + i] = *idx;
        }
    }

    pub fn vertex_xyz_slice(&self, slot: &VertexBufferSlot) -> Vec<[f32; 3]> {
        let Some(vb) = self.vertex_buffers[slot.fvf as usize].get(slot.buffer_index) else {
            return Vec::new();
        };
        let stride = slot.fvf.stride_bytes() as usize;
        let start = slot.start as usize * stride;
        let count = slot.size as usize;
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let off = start + i * stride;
            if off + 12 > vb.cpu.len() {
                break;
            }
            out.push([
                f32::from_le_bytes(vb.cpu[off..off + 4].try_into().unwrap_or([0; 4])),
                f32::from_le_bytes(vb.cpu[off + 4..off + 8].try_into().unwrap_or([0; 4])),
                f32::from_le_bytes(vb.cpu[off + 8..off + 12].try_into().unwrap_or([0; 4])),
            ]);
        }
        out
    }

    pub fn index_slice(&self, slot: &IndexBufferSlot) -> Vec<u16> {
        let Some(ib) = self.index_buffers.get(slot.buffer_index) else {
            return Vec::new();
        };
        let start = slot.start as usize;
        let end = (start + slot.size as usize).min(ib.cpu.len());
        ib.cpu[start..end].to_vec()
    }

    pub fn push_render_task(&mut self, slot: &VertexBufferSlot, task: usize) {
        if let Some(vb) = self.vertex_buffers[slot.fvf as usize].get_mut(slot.buffer_index) {
            vb.render_tasks.push(task);
        }
    }

    pub fn drain_render_tasks(&mut self, fvf: VbmFvfType) -> Vec<(usize, Vec<usize>)> {
        let mut out = Vec::new();
        for (i, vb) in self.vertex_buffers[fvf as usize].iter_mut().enumerate() {
            if !vb.render_tasks.is_empty() {
                out.push((i, std::mem::take(&mut vb.render_tasks)));
            }
        }
        out
    }

    pub fn vertex_buffer_count(&self, fvf: VbmFvfType) -> usize {
        self.vertex_buffers[fvf as usize].len()
    }

    pub fn free_all_slots(&mut self) {
        for list in &mut self.vb_free {
            for size in list {
                size.clear();
            }
        }
        for list in &mut self.ib_free {
            list.clear();
        }
        self.slots_allocated = 0;
        self.index_slots_allocated = 0;
        for list in &mut self.vertex_buffers {
            for vb in list.iter_mut() {
                vb.start_free_index = 0;
                vb.render_tasks.clear();
            }
        }
        for ib in &mut self.index_buffers {
            ib.start_free_index = 0;
        }
    }

    pub fn free_all_buffers(&mut self) {
        self.free_all_slots();
        for list in &mut self.vertex_buffers {
            list.clear();
        }
        self.index_buffers.clear();
    }

    pub fn release_resources(&mut self) {
        // wgpu buffers are recreated on demand; CPU mirrors stay.
    }

    pub fn re_acquire_resources(&mut self) -> bool {
        true
    }
}

static THE_W3D_BUFFER_MANAGER: LazyLock<Mutex<W3DBufferManager>> =
    LazyLock::new(|| Mutex::new(W3DBufferManager::new()));

/// C++ `TheW3DBufferManager`.
pub fn the_w3d_buffer_manager() -> &'static Mutex<W3DBufferManager> {
    &THE_W3D_BUFFER_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_slot_size_and_recycles() {
        let mut mgr = W3DBufferManager::new();
        let a = mgr.get_vertex_slot(VbmFvfType::Xyz, 10).expect("slot");
        assert_eq!(a.size, 32);
        assert_eq!(a.start, 0);
        mgr.release_vertex_slot(a);
        let b = mgr.get_vertex_slot(VbmFvfType::Xyz, 10).expect("recycle");
        assert_eq!(b.start, 0);
        assert_eq!(mgr.vertex_buffer_count(VbmFvfType::Xyz), 1);
    }

    #[test]
    fn carves_second_slot_from_same_buffer() {
        let mut mgr = W3DBufferManager::new();
        let a = mgr.get_vertex_slot(VbmFvfType::Xyz, 32).unwrap();
        let b = mgr.get_vertex_slot(VbmFvfType::Xyz, 64).unwrap();
        assert_eq!(a.start, 0);
        assert_eq!(b.start, 32);
        assert_eq!(mgr.vertex_buffer_count(VbmFvfType::Xyz), 1);
    }
}

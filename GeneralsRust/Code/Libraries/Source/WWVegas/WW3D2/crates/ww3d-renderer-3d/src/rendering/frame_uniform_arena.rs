//! Per-frame uniform arena for batching renderer uniforms into a single GPU buffer.
//!
//! This arena is reset once per frame and avoids creating transient uniform buffers for every
//! draw call. Data is uploaded through `queue.write_buffer` at allocation time and bind groups
//! reference the shared buffer with per-draw offsets.

use std::num::NonZeroU64;
use std::sync::Arc;

use crate::core::error::{Error, Result};
use ww3d_gpu::device::GpuDevice;

const DEFAULT_ARENA_SIZE: usize = 512 * 1024; // 512 KiB per frame
const ARENA_ALIGNMENT: u64 = 256;

#[derive(Clone)]
pub struct FrameUniformSlice {
    pub buffer: Arc<wgpu::Buffer>,
    pub offset: u64,
    pub size: u64,
}

impl FrameUniformSlice {
    pub fn as_binding(&self) -> wgpu::BufferBinding<'_> {
        let size = NonZeroU64::new(self.size).expect("uniform slice size must be > 0");
        wgpu::BufferBinding {
            buffer: self.buffer.as_ref(),
            offset: self.offset,
            size: Some(size),
        }
    }
}

pub struct FrameUniformArena {
    pages: Vec<FrameUniformPage>,
    default_capacity: u64,
    active_page: usize,
}

struct FrameUniformPage {
    buffer: Arc<wgpu::Buffer>,
    capacity: u64,
    cursor: u64,
}

impl FrameUniformArena {
    pub fn new(device: &GpuDevice, size: usize) -> Self {
        let default_capacity = size.max(DEFAULT_ARENA_SIZE) as u64;
        let first_page = Self::create_page(device, default_capacity);

        Self {
            pages: vec![first_page],
            default_capacity,
            active_page: 0,
        }
    }

    fn create_page(device: &GpuDevice, capacity: u64) -> FrameUniformPage {
        let buffer = Arc::new(device.wgpu_device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("WW3D Frame Uniform Arena"),
            size: capacity,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        FrameUniformPage {
            buffer,
            capacity,
            cursor: 0,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        for page in &mut self.pages {
            page.cursor = 0;
        }
        self.active_page = 0;
    }

    #[inline]
    pub fn capacity(&self) -> u64 {
        self.pages.iter().map(|page| page.capacity).sum()
    }

    #[inline]
    pub fn used(&self) -> u64 {
        self.pages.iter().map(|page| page.cursor).sum()
    }

    pub fn allocate(
        &mut self,
        device: &GpuDevice,
        data: &[u8],
        alignment: u64,
    ) -> Result<FrameUniformSlice> {
        if data.is_empty() {
            return Err(Error::InvalidParameter(
                "cannot allocate zero-sized uniform slice".into(),
            ));
        }

        if self.pages.is_empty() {
            self.pages
                .push(Self::create_page(device, self.default_capacity));
            self.active_page = 0;
        }

        let alignment = alignment.max(ARENA_ALIGNMENT);
        let size = data.len() as u64;

        for index in self.active_page..self.pages.len() {
            let page = &mut self.pages[index];
            let aligned = align_up(page.cursor, alignment);
            let end = aligned
                .checked_add(size)
                .ok_or_else(|| Error::BufferOverflow("uniform arena overflow".into()))?;
            if end <= page.capacity {
                device
                    .queue()
                    .write_buffer(page.buffer.as_ref(), aligned, data);
                page.cursor = end;
                self.active_page = index;
                return Ok(FrameUniformSlice {
                    buffer: Arc::clone(&page.buffer),
                    offset: aligned,
                    size,
                });
            }
        }

        let required = size
            .checked_add(alignment)
            .ok_or_else(|| Error::BufferOverflow("uniform arena overflow".into()))?;
        let mut new_capacity = self.default_capacity.max(required);
        if let Some(grown) = new_capacity.checked_next_power_of_two() {
            new_capacity = grown;
        }

        let mut page = Self::create_page(device, new_capacity);
        let aligned = align_up(page.cursor, alignment);
        let end = aligned
            .checked_add(size)
            .ok_or_else(|| Error::BufferOverflow("uniform arena overflow".into()))?;
        if end > page.capacity {
            return Err(Error::BufferOverflow(
                "frame uniform arena exhausted".into(),
            ));
        }
        page.cursor = end;
        let buffer = Arc::clone(&page.buffer);
        self.pages.push(page);
        self.active_page = self.pages.len() - 1;
        device.queue().write_buffer(buffer.as_ref(), aligned, data);

        Ok(FrameUniformSlice {
            buffer,
            offset: aligned,
            size,
        })
    }
}

#[inline]
pub fn align_up(value: u64, alignment: u64) -> u64 {
    let mask = alignment - 1;
    (value + mask) & !mask
}

//! Shared text pages for rasterized labels (renderer plan R11).
//!
//! C++ `Render2DSentenceClass::Build_Textures` (render2dsentence.cpp) blits
//! each sentence's glyphs from the `FontCharsClass` cache into sentence
//! surfaces. The port keeps that per-sentence raster, so label pixels are
//! unchanged, but packs the sentences into a few shared pages instead of one
//! texture each. Texture memory is bounded, page bind groups are reused, and
//! consecutive labels on one page merge into a single UI draw.
//!
//! [`TextAtlasLayout`] is the CPU packing policy (shelves, least-recently-used
//! page reuse) and is tested without a device; [`TextAtlas`] applies its
//! placements to wgpu pages owned by the receiving `UIRenderer`.

use std::collections::HashMap;
use std::sync::Arc;

use wgpu::{Device, Queue, Texture, TextureView};

use super::UIRect;

/// Edge length of one text page in texels.
pub(super) const PAGE_SIZE: u32 = 1024;
/// Most pages one renderer keeps. Past this, the least recently drawn page
/// that no in-flight frame uses is cleared and reused.
pub(super) const MAX_PAGES: usize = 8;
/// Transparent texels around every label so bilinear sampling of a quad edge
/// never reads a neighbouring label.
const GUTTER: u32 = 1;
/// Shelf heights round up to this many texels so labels of nearby sizes share
/// shelves.
const SHELF_STEP: u32 = 4;
/// Labels too large for a page keep standalone textures; at most this many
/// are retained between frames.
const MAX_STANDALONE: usize = 64;
/// UI frames a page must stay undrawn before it can be cleared.
const RECYCLE_AGE: u64 = 2;

/// Identifies a text page: its slot plus the reset generation, so a stale
/// placement can never address a page that was cleared and refilled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct PageId {
    slot: usize,
    generation: u32,
}

impl PageId {
    pub(super) fn slot(self) -> usize {
        self.slot
    }
}

/// Texel rectangle inside a page, excluding the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TexelRect {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

impl TexelRect {
    /// Normalized texture coordinates of this rectangle in a page.
    fn uv(self) -> UIRect {
        let page = f32::from(u16::try_from(PAGE_SIZE).unwrap_or(u16::MAX));
        UIRect::new(
            texel_f32(self.x) / page,
            texel_f32(self.y) / page,
            texel_f32(self.width) / page,
            texel_f32(self.height) / page,
        )
    }

    /// The rectangle grown by the gutter on every side.
    fn padded(self) -> TexelRect {
        TexelRect {
            x: self.x - GUTTER,
            y: self.y - GUTTER,
            width: self.width + 2 * GUTTER,
            height: self.height + 2 * GUTTER,
        }
    }

    fn overlaps(self, other: TexelRect) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// Page coordinates never exceed [`PAGE_SIZE`], which `f32` holds exactly.
fn texel_f32(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// Where a label landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Placement {
    /// Packed into a page that already existed.
    Packed { page: PageId, rect: TexelRect },
    /// Packed into a page the caller must create first.
    NewPage { page: PageId, rect: TexelRect },
    /// Packed into a page that was just cleared; every label it held was
    /// dropped from the layout.
    Recycled { page: PageId, rect: TexelRect },
    /// Too large for a page, or every page is in use by the current frame.
    Standalone,
}

#[derive(Debug, Clone)]
struct Shelf {
    y: u32,
    height: u32,
    cursor_x: u32,
}

#[derive(Debug, Clone)]
struct PageLayout {
    generation: u32,
    shelves: Vec<Shelf>,
    next_shelf_y: u32,
    last_used_frame: u64,
}

impl PageLayout {
    fn new(generation: u32, frame: u64) -> Self {
        Self {
            generation,
            shelves: Vec::new(),
            next_shelf_y: 0,
            last_used_frame: frame,
        }
    }

    /// Shelf allocation of a padded `width` x `height` block, choosing the
    /// lowest-waste existing shelf before opening a new one.
    fn allocate(&mut self, width: u32, height: u32) -> Option<TexelRect> {
        let best = self
            .shelves
            .iter_mut()
            .filter(|shelf| shelf.height >= height && PAGE_SIZE - shelf.cursor_x >= width)
            .min_by_key(|shelf| shelf.height - height);
        let shelf = match best {
            Some(shelf) => shelf,
            None => {
                let shelf_height = height.div_ceil(SHELF_STEP) * SHELF_STEP;
                if PAGE_SIZE - self.next_shelf_y < shelf_height {
                    return None;
                }
                self.shelves.push(Shelf {
                    y: self.next_shelf_y,
                    height: shelf_height.min(PAGE_SIZE - self.next_shelf_y),
                    cursor_x: 0,
                });
                self.next_shelf_y += shelf_height;
                self.shelves.last_mut()?
            }
        };
        let rect = TexelRect {
            x: shelf.cursor_x + GUTTER,
            y: shelf.y + GUTTER,
            width: width - 2 * GUTTER,
            height: height - 2 * GUTTER,
        };
        shelf.cursor_x += width;
        Some(rect)
    }
}

#[derive(Debug, Clone, Copy)]
struct LayoutEntry {
    page: PageId,
    rect: TexelRect,
}

/// CPU packing policy of the text pages.
#[derive(Debug, Default)]
pub(super) struct TextAtlasLayout {
    pages: Vec<PageLayout>,
    entries: HashMap<u64, LayoutEntry>,
    frame: u64,
}

impl TextAtlasLayout {
    /// The placement of `key`, marking its page as used by this frame.
    pub(super) fn lookup(&mut self, key: u64) -> Option<(PageId, TexelRect)> {
        let entry = *self.entries.get(&key)?;
        self.pages[entry.page.slot].last_used_frame = self.frame;
        Some((entry.page, entry.rect))
    }

    /// Reserve space for a `width` x `height` label.
    pub(super) fn place(&mut self, key: u64, width: u32, height: u32) -> Placement {
        let (Some(padded_width), Some(padded_height)) = (
            width.checked_add(2 * GUTTER).filter(|w| *w <= PAGE_SIZE),
            height.checked_add(2 * GUTTER).filter(|h| *h <= PAGE_SIZE),
        ) else {
            return Placement::Standalone;
        };
        if width == 0 || height == 0 {
            return Placement::Standalone;
        }
        let frame = self.frame;
        for (slot, page) in self.pages.iter_mut().enumerate() {
            if let Some(rect) = page.allocate(padded_width, padded_height) {
                page.last_used_frame = frame;
                let id = PageId {
                    slot,
                    generation: page.generation,
                };
                self.entries.insert(key, LayoutEntry { page: id, rect });
                return Placement::Packed { page: id, rect };
            }
        }
        if self.pages.len() < MAX_PAGES {
            let slot = self.pages.len();
            let mut page = PageLayout::new(0, frame);
            let Some(rect) = page.allocate(padded_width, padded_height) else {
                return Placement::Standalone;
            };
            self.pages.push(page);
            let id = PageId {
                slot,
                generation: 0,
            };
            self.entries.insert(key, LayoutEntry { page: id, rect });
            return Placement::NewPage { page: id, rect };
        }
        // Clearing a page a queued frame still samples would overwrite its
        // pixels before that frame's submission. Keep the current and the
        // previous UI frame's pages: work for the next frame can be queued
        // before the previous frame's command buffer is submitted.
        let Some(slot) = self
            .pages
            .iter()
            .enumerate()
            .filter(|(_, page)| page.last_used_frame.saturating_add(RECYCLE_AGE) <= frame)
            .min_by_key(|(slot, page)| (page.last_used_frame, *slot))
            .map(|(slot, _)| slot)
        else {
            return Placement::Standalone;
        };
        let generation = self.pages[slot].generation.wrapping_add(1);
        self.entries.retain(|_, entry| entry.page.slot != slot);
        let mut page = PageLayout::new(generation, frame);
        let Some(rect) = page.allocate(padded_width, padded_height) else {
            return Placement::Standalone;
        };
        self.pages[slot] = page;
        let id = PageId { slot, generation };
        self.entries.insert(key, LayoutEntry { page: id, rect });
        Placement::Recycled { page: id, rect }
    }

    /// Close the current frame; later placements may recycle its pages.
    pub(super) fn end_frame(&mut self) {
        self.frame += 1;
    }

    /// Forget every placement and page (font registration changed the
    /// rasters). The caller drops its page textures too, so queued commands
    /// keep sampling the old pages while new labels go to fresh ones.
    pub(super) fn clear(&mut self) {
        self.pages.clear();
        self.entries.clear();
    }

    pub(super) fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub(super) fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// A label ready to draw: the texture to bind, its coordinates in that
/// texture, and the quad relative to the layout bounds.
#[derive(Clone)]
pub(super) struct TextSlot {
    pub(super) texture: Arc<TextureView>,
    pub(super) uv: Option<UIRect>,
    pub(super) quad: UIRect,
}

struct Standalone {
    texture: Arc<TextureView>,
    quad: UIRect,
    last_used_frame: u64,
}

struct PageTexture {
    texture: Texture,
    view: Arc<TextureView>,
}

/// Text pages of one `UIRenderer`.
#[derive(Default)]
pub(super) struct TextAtlas {
    layout: TextAtlasLayout,
    pages: Vec<PageTexture>,
    /// Relative quads of packed labels, keyed like the layout.
    quads: HashMap<u64, UIRect>,
    standalone: HashMap<u64, Standalone>,
}

impl TextAtlas {
    /// The cached label for `key`, if any.
    pub(super) fn get(&mut self, key: u64) -> Option<TextSlot> {
        if let Some((page, rect)) = self.layout.lookup(key) {
            let quad = *self.quads.get(&key)?;
            return Some(TextSlot {
                texture: Arc::clone(&self.pages[page.slot()].view),
                uv: Some(rect.uv()),
                quad,
            });
        }
        let frame = self.layout.frame;
        let standalone = self.standalone.get_mut(&key)?;
        standalone.last_used_frame = frame;
        Some(TextSlot {
            texture: Arc::clone(&standalone.texture),
            uv: None,
            quad: standalone.quad,
        })
    }

    /// Store a rasterized RGBA label of `quad.width` x `quad.height` texels.
    /// `standalone_upload` creates a dedicated texture when the label does not
    /// fit a page.
    pub(super) fn insert(
        &mut self,
        device: &Device,
        queue: &Queue,
        key: u64,
        quad: UIRect,
        pixels: &[u8],
        standalone_upload: impl FnOnce(u32, u32, &[u8]) -> Arc<TextureView>,
    ) -> TextSlot {
        let (width, height) = quad_extent(quad);
        let (page, rect) = match self.layout.place(key, width, height) {
            Placement::Packed { page, rect } => (page, rect),
            Placement::NewPage { page, rect } => {
                self.pages.push(PageTexture::new(device));
                debug_assert_eq!(self.pages.len(), page.slot() + 1);
                (page, rect)
            }
            Placement::Recycled { page, rect } => {
                self.quads
                    .retain(|key, _| self.layout.entries.contains_key(key));
                (page, rect)
            }
            Placement::Standalone => {
                return self.insert_standalone(key, quad, width, height, pixels, standalone_upload);
            }
        };
        self.pages[page.slot()].write(queue, rect, pixels);
        self.quads.insert(key, quad);
        TextSlot {
            texture: Arc::clone(&self.pages[page.slot()].view),
            uv: Some(rect.uv()),
            quad,
        }
    }

    fn insert_standalone(
        &mut self,
        key: u64,
        quad: UIRect,
        width: u32,
        height: u32,
        pixels: &[u8],
        upload: impl FnOnce(u32, u32, &[u8]) -> Arc<TextureView>,
    ) -> TextSlot {
        let frame = self.layout.frame;
        if self.standalone.len() >= MAX_STANDALONE {
            // Commands hold their own `Arc`, so dropping a texture used this
            // frame is safe; prefer the least recently drawn one.
            if let Some(oldest) = self
                .standalone
                .iter()
                .min_by_key(|(key, entry)| (entry.last_used_frame, **key))
                .map(|(key, _)| *key)
            {
                self.standalone.remove(&oldest);
            }
        }
        let texture = upload(width, height, pixels);
        self.standalone.insert(
            key,
            Standalone {
                texture: Arc::clone(&texture),
                quad,
                last_used_frame: frame,
            },
        );
        TextSlot {
            texture,
            uv: None,
            quad,
        }
    }

    pub(super) fn end_frame(&mut self) {
        self.layout.end_frame();
    }

    pub(super) fn clear(&mut self) {
        self.layout.clear();
        self.pages.clear();
        self.quads.clear();
        self.standalone.clear();
    }

    pub(super) fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub(super) fn label_count(&self) -> usize {
        self.layout.entry_count() + self.standalone.len()
    }
}

/// Texel extent of a canvas quad (canvas sizes are whole texels).
fn quad_extent(quad: UIRect) -> (u32, u32) {
    let texels = |value: f32| {
        let rounded = value.round();
        if rounded.is_finite() && rounded >= 1.0 {
            // Canvases are clamped to at most 2048 texels by the raster.
            u32::try_from(u16::try_from(rounded as i64).unwrap_or(u16::MAX)).unwrap_or(1)
        } else {
            1
        }
    };
    (texels(quad.width), texels(quad.height))
}

impl PageTexture {
    fn new(device: &Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI Text Page"),
            size: wgpu::Extent3d {
                width: PAGE_SIZE,
                height: PAGE_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = Arc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        Self { texture, view }
    }

    /// Upload `pixels` (tightly packed RGBA rows of `rect`) together with a
    /// transparent gutter, overwriting whatever a recycled page held there.
    fn write(&self, queue: &Queue, rect: TexelRect, pixels: &[u8]) {
        let padded = rect.padded();
        let row = to_usize(padded.width) * 4;
        let mut block = vec![0u8; row * to_usize(padded.height)];
        let src_row = to_usize(rect.width) * 4;
        for (y, src) in pixels
            .chunks_exact(src_row)
            .take(to_usize(rect.height))
            .enumerate()
        {
            let start = (y + to_usize(GUTTER)) * row + to_usize(GUTTER) * 4;
            block[start..start + src_row].copy_from_slice(src);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: padded.x,
                    y: padded.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &block,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded.width * 4),
                rows_per_image: Some(padded.height),
            },
            wgpu::Extent3d {
                width: padded.width,
                height: padded.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

fn to_usize(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_of(placement: Placement) -> (PageId, TexelRect) {
        match placement {
            Placement::Packed { page, rect }
            | Placement::NewPage { page, rect }
            | Placement::Recycled { page, rect } => (page, rect),
            Placement::Standalone => panic!("expected a packed label"),
        }
    }

    #[test]
    fn labels_share_a_page_without_overlapping_gutters() {
        let mut layout = TextAtlasLayout::default();
        let mut placed = Vec::new();
        for key in 0..200u64 {
            let width = 20 + u32::try_from(key % 7).unwrap() * 13;
            let height = 12 + u32::try_from(key % 3).unwrap() * 2;
            let placement = layout.place(key, width, height);
            if key == 0 {
                assert!(matches!(placement, Placement::NewPage { .. }));
            }
            let (page, rect) = rect_of(placement);
            assert_eq!((rect.width, rect.height), (width, height));
            assert!(rect.padded().x + rect.padded().width <= PAGE_SIZE);
            assert!(rect.padded().y + rect.padded().height <= PAGE_SIZE);
            placed.push((page, rect));
        }
        assert_eq!(layout.page_count(), 1);
        for (i, (page_a, a)) in placed.iter().enumerate() {
            for (page_b, b) in &placed[i + 1..] {
                if page_a == page_b {
                    assert!(!a.padded().overlaps(b.padded()), "{a:?} overlaps {b:?}");
                }
            }
        }
        assert_eq!(layout.lookup(17), Some(placed[17]));
    }

    #[test]
    fn oversized_or_empty_labels_stay_standalone() {
        let mut layout = TextAtlasLayout::default();
        assert_eq!(layout.place(1, PAGE_SIZE, 10), Placement::Standalone);
        assert_eq!(layout.place(2, 10, PAGE_SIZE - 1), Placement::Standalone);
        assert_eq!(layout.place(3, 0, 10), Placement::Standalone);
        assert_eq!(layout.page_count(), 0);
        assert!(matches!(
            layout.place(4, PAGE_SIZE - 2 * GUTTER, 10),
            Placement::NewPage { .. }
        ));
    }

    /// Fill every page with one full-page label each.
    fn fill_pages(layout: &mut TextAtlasLayout) {
        for key in 0..u64::try_from(MAX_PAGES).unwrap() {
            let side = PAGE_SIZE - 2 * GUTTER;
            assert!(matches!(
                layout.place(key, side, side),
                Placement::NewPage { .. }
            ));
            layout.end_frame();
        }
    }

    #[test]
    fn full_atlas_recycles_the_least_recently_drawn_page() {
        let mut layout = TextAtlasLayout::default();
        fill_pages(&mut layout);
        // Page 0 is drawn again, so page 1 is now the oldest.
        assert!(layout.lookup(0).is_some());
        layout.end_frame();
        layout.end_frame();
        let (page, _) = rect_of(layout.place(100, 50, 20));
        assert_eq!(page.slot(), 1);
        assert_eq!(page.generation, 1);
        assert!(layout.lookup(1).is_none(), "recycled labels are forgotten");
        assert!(layout.lookup(0).is_some());
        // Later labels pack into the recycled page.
        assert!(
            matches!(layout.place(101, 50, 20), Placement::Packed { page: p, .. } if p == page)
        );
    }

    #[test]
    fn pages_of_in_flight_frames_are_never_recycled() {
        let mut layout = TextAtlasLayout::default();
        fill_pages(&mut layout);
        for key in 0..u64::try_from(MAX_PAGES).unwrap() {
            assert!(layout.lookup(key).is_some());
        }
        assert_eq!(layout.place(500, 50, 20), Placement::Standalone);
        // The previous frame may not be submitted yet either.
        layout.end_frame();
        assert_eq!(layout.place(501, 50, 20), Placement::Standalone);
        layout.end_frame();
        assert!(matches!(
            layout.place(502, 50, 20),
            Placement::Recycled { .. }
        ));
    }

    #[test]
    fn clear_forgets_labels_and_pages() {
        let mut layout = TextAtlasLayout::default();
        rect_of(layout.place(7, 30, 12));
        layout.clear();
        assert!(layout.lookup(7).is_none());
        assert_eq!(layout.page_count(), 0);
        assert!(matches!(layout.place(7, 30, 12), Placement::NewPage { .. }));
    }

    #[test]
    fn uv_maps_texels_onto_the_page() {
        let rect = TexelRect {
            x: 1,
            y: 513,
            width: 256,
            height: 16,
        };
        let uv = rect.uv();
        assert_eq!(uv.x, 1.0 / 1024.0);
        assert_eq!(uv.y, 513.0 / 1024.0);
        assert_eq!(uv.width, 0.25);
        assert_eq!(uv.height, 16.0 / 1024.0);
    }
}

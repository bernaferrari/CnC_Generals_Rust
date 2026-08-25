//! C++ parity model for `TexturePage.h`.

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PageStatus: u32 {
        const READY = 0x0000_0001;
        const PAGE_ERROR = 0x0000_0002;
        const CANT_ALLOCATE_PACKED_IMAGE = 0x0000_0004;
        const CANT_ADD_IMAGE_DATA = 0x0000_0008;
        const NO_TEXTURE_DATA = 0x0000_0010;
        const ERROR_DURING_SAVE = 0x0000_0020;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasCell {
    Free,
    Used,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePlacement {
    pub image_index: usize,
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub rotated: bool,
    pub fit_bits: u32,
    pub gutter_used: (u32, u32),
}

/// C++ `ImageInfo::FIT_*` bits from `buildFitRegion`.
pub const FIT_XGUTTER: u32 = 0x0000_0001;
pub const FIT_YGUTTER: u32 = 0x0000_0002;
pub const FIT_XBORDER_RIGHT: u32 = 0x0000_0004;
pub const FIT_XBORDER_LEFT: u32 = 0x0000_0008;
pub const FIT_YBORDER_TOP: u32 = 0x0000_0010;
pub const FIT_YBORDER_BOTTOM: u32 = 0x0000_0020;

#[derive(Debug, Clone)]
pub struct TexturePage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub status: PageStatus,
    pub placements: Vec<ImagePlacement>,
    /// C++ `ImagePacker` gutter when `GAP_METHOD_GUTTER` is set.
    pub x_gutter: u32,
    pub y_gutter: u32,
    /// C++ `GAP_METHOD_EXTEND_RGB` → `allSidesBorder` (2px, or 1px if page-1).
    pub all_sides_border: bool,
    canvas: Vec<u8>,
}

/// Source rectangle to pack onto a [`TexturePage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageToPack {
    pub image_index: usize,
    pub width: u32,
    pub height: u32,
}

impl TexturePage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            id: 0,
            width,
            height,
            status: PageStatus::READY,
            placements: Vec::new(),
            x_gutter: 0,
            y_gutter: 0,
            all_sides_border: false,
            canvas: vec![0u8; width as usize * height as usize],
        }
    }

    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    pub fn add_image_placement(&mut self, placement: ImagePlacement) {
        self.placements.push(placement);
    }

    pub fn get_first_image(&self) -> Option<&ImagePlacement> {
        self.placements.first()
    }

    fn idx(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((y as usize) * (self.width as usize) + (x as usize))
    }

    fn spot_used(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return true;
        }
        match self.idx(x as u32, y as u32) {
            Some(i) => self.canvas[i] != 0,
            None => true,
        }
    }

    fn line_used(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        if x0 == x1 {
            let (lo, hi) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
            for y in lo..=hi {
                if self.spot_used(x0, y) {
                    return true;
                }
            }
            return false;
        }
        if y0 == y1 {
            let (lo, hi) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
            for x in lo..=hi {
                if self.spot_used(x, y0) {
                    return true;
                }
            }
            return false;
        }
        true
    }

    fn mark_region_used(&mut self, left: u32, top: u32, right: u32, bottom: u32) {
        for y in top..=bottom.min(self.height.saturating_sub(1)) {
            for x in left..=right.min(self.width.saturating_sub(1)) {
                if let Some(i) = self.idx(x, y) {
                    self.canvas[i] = 1;
                }
            }
        }
    }

    /// C++ `TexturePage::buildFitRegion`.
    fn build_fit_region(
        &self,
        start_x: u32,
        start_y: u32,
        image_w: u32,
        image_h: u32,
    ) -> Option<(u32, u32, u32, u32, u32, u32, u32)> {
        let mut x_gutter = self.x_gutter;
        let mut y_gutter = self.y_gutter;
        let mut x_border = if self.all_sides_border { 2u32 } else { 0 };
        let mut y_border = if self.all_sides_border { 2u32 } else { 0 };
        if image_w == self.width {
            x_gutter = 0;
            x_border = 0;
        }
        if image_h == self.height {
            y_gutter = 0;
            y_border = 0;
        }
        if image_w == self.width.saturating_sub(1) {
            x_border = 1;
        }
        if image_h == self.height.saturating_sub(1) {
            y_border = 1;
        }
        let hi_x = start_x
            .saturating_add(image_w.saturating_sub(1))
            .saturating_add(x_gutter)
            .saturating_add(x_border);
        let hi_y = start_y
            .saturating_add(image_h.saturating_sub(1))
            .saturating_add(y_gutter)
            .saturating_add(y_border);
        if hi_x >= self.width || hi_y >= self.height {
            return None;
        }
        let mut fit_bits = 0u32;
        if x_gutter != 0 {
            fit_bits |= FIT_XGUTTER;
        }
        if y_gutter != 0 {
            fit_bits |= FIT_YGUTTER;
        }
        if x_border >= 1 {
            fit_bits |= FIT_XBORDER_RIGHT;
        }
        if x_border == 2 {
            fit_bits |= FIT_XBORDER_LEFT;
        }
        if y_border >= 1 {
            fit_bits |= FIT_YBORDER_BOTTOM;
        }
        if y_border == 2 {
            fit_bits |= FIT_YBORDER_TOP;
        }
        Some((start_x, start_y, hi_x, hi_y, fit_bits, x_gutter, y_gutter))
    }

    /// C++ `TexturePage::addImage`: nested `for y / for x` canvas scan, then 90° CW.
    pub fn add_image(&mut self, image: &ImageToPack) -> bool {
        if image.width == 0 || image.height == 0 {
            self.status = PageStatus::CANT_ADD_IMAGE_DATA;
            return false;
        }
        for try_rotate in [false, true] {
            if try_rotate && image.width == image.height {
                continue;
            }
            let (w, h) = if try_rotate {
                (image.height, image.width)
            } else {
                (image.width, image.height)
            };
            if w > self.width || h > self.height {
                continue;
            }
            let mut y = 0u32;
            while y < self.height {
                let mut x = 0u32;
                while x < self.width {
                    let Some((lo_x, lo_y, hi_x, hi_y, fit_bits, xg, yg)) =
                        self.build_fit_region(x, y, w, h)
                    else {
                        break;
                    };
                    let lx = lo_x as i32;
                    let ly = lo_y as i32;
                    let hx = hi_x as i32;
                    let hy = hi_y as i32;
                    if self.spot_used(hx, ly) || self.spot_used(hx, hy) {
                        x = hi_x + 1;
                        continue;
                    }
                    if self.spot_used(lx, ly) || self.spot_used(lx, hy) {
                        x += 1;
                        continue;
                    }
                    if self.line_used(lx, ly, hx, ly)
                        || self.line_used(hx, ly, hx, hy)
                        || self.line_used(lx, hy, hx, hy)
                        || self.line_used(lx, ly, lx, hy)
                    {
                        x += 1;
                        continue;
                    }
                    self.mark_region_used(lo_x, lo_y, hi_x, hi_y);
                    let left = lo_x + u32::from(fit_bits & FIT_XBORDER_LEFT != 0);
                    let top = lo_y + u32::from(fit_bits & FIT_YBORDER_TOP != 0);
                    self.placements.push(ImagePlacement {
                        image_index: image.image_index,
                        left,
                        top,
                        right: left + w,
                        bottom: top + h,
                        rotated: try_rotate,
                        fit_bits,
                        gutter_used: (xg, yg),
                    });
                    return true;
                }
                y += 1;
            }
        }
        false
    }

    /// Pack a batch onto this page via C++ `addImage` canvas scan.
    pub fn pack_images(&mut self, images: &[ImageToPack]) -> bool {
        self.placements.clear();
        self.canvas.fill(0);
        self.status = PageStatus::READY;
        if self.width == 0 || self.height == 0 {
            self.status = PageStatus::CANT_ALLOCATE_PACKED_IMAGE;
            return false;
        }
        for image in images {
            if !self.add_image(image) {
                self.status = PageStatus::CANT_ADD_IMAGE_DATA;
                return false;
            }
        }
        true
    }

    /// C++ `TexturePage::addImageData` + `extendImageEdges` when
    /// `GAP_METHOD_EXTEND_RGB` is set. Copies RGBA into the page buffer then
    /// bleeds RGB using FIT border bits (`extendAlpha = FALSE` in C++).
    pub fn blit_with_rgb_extend(
        dest: &mut [u8],
        dest_w: u32,
        dest_h: u32,
        src: &[u8],
        src_w: u32,
        src_h: u32,
        dest_x: u32,
        dest_y: u32,
        extend_rgb: bool,
        fit_bits: u32,
    ) -> bool {
        let dest_len = dest_w as usize * dest_h as usize * 4;
        let src_len = src_w as usize * src_h as usize * 4;
        if dest.len() < dest_len || src.len() < src_len {
            return false;
        }
        if dest_x + src_w > dest_w || dest_y + src_h > dest_h {
            return false;
        }
        for sy in 0..src_h {
            for sx in 0..src_w {
                let si = ((sy * src_w + sx) * 4) as usize;
                let dx = dest_x + sx;
                let dy = dest_y + sy;
                let di = ((dy * dest_w + dx) * 4) as usize;
                dest[di..di + 4].copy_from_slice(&src[si..si + 4]);
            }
        }
        if !extend_rgb {
            return true;
        }
        extend_image_edges(
            dest, dest_w, dest_h, dest_x, dest_y, src_w, src_h, fit_bits, false,
        )
    }
}

fn rgba_index(dest_w: u32, x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 {
        return None;
    }
    Some((y as usize) * (dest_w as usize) * 4 + (x as usize) * 4)
}

fn pixel_occupied_rgba(dest: &[u8], i: usize) -> bool {
    dest.get(i + 3).copied().unwrap_or(0) != 0
}

fn copy_rgb_into(dest: &mut [u8], i: usize, r: u8, g: u8, b: u8, a: u8, extend_alpha: bool) {
    if i + 3 >= dest.len() {
        return;
    }
    dest[i] = r;
    dest[i + 1] = g;
    dest[i + 2] = b;
    if extend_alpha {
        dest[i + 3] = a;
    }
}

/// C++ `TexturePage::extendToRowIfOpen` (top-left RGBA dest; C++ TGA is flipped).
fn extend_to_row_if_open(
    dest: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    dest_x: i32,
    dest_y: i32,
    image_y: i32,
    image_h: i32,
    fit_bits: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    extend_alpha: bool,
) {
    let ny = if image_y < image_h / 2 && (image_y != 0 || fit_bits & FIT_YBORDER_TOP != 0) {
        dest_y - 1
    } else if image_y >= image_h / 2
        && (image_y != image_h - 1 || fit_bits & FIT_YBORDER_BOTTOM != 0)
    {
        dest_y + 1
    } else {
        return;
    };
    if ny < 0 || ny >= dest_h as i32 || dest_x < 0 || dest_x >= dest_w as i32 {
        return;
    }
    let Some(ni) = rgba_index(dest_w, dest_x, ny) else {
        return;
    };
    if pixel_occupied_rgba(dest, ni) {
        return;
    }
    copy_rgb_into(dest, ni, r, g, b, a, extend_alpha);
}

/// C++ `TexturePage::extendImageEdges`. Dest is top-left RGBA (engine atlas),
/// not TGA A,R,G,B; occupancy still uses alpha != 0. Vertical "up/down" is
/// flipped vs C++ TGA bottom-origin.
pub fn extend_image_edges(
    dest: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    page_x: u32,
    page_y: u32,
    image_w: u32,
    image_h: u32,
    fit_bits: u32,
    extend_alpha: bool,
) -> bool {
    if dest_w == 0 || dest_h == 0 || image_w == 0 || image_h == 0 {
        return false;
    }
    if page_x + image_w > dest_w || page_y + image_h > dest_h {
        return false;
    }
    let dest_len = dest_w as usize * dest_h as usize * 4;
    if dest.len() < dest_len {
        return false;
    }
    let image_w_i = image_w as i32;
    let image_h_i = image_h as i32;
    for iy in 0..image_h_i {
        let mut prev_pixel = false;
        for ix in 0..image_w_i {
            let dx = page_x as i32 + ix;
            let dy = page_y as i32 + iy;
            let Some(di) = rgba_index(dest_w, dx, dy) else {
                continue;
            };
            let r = dest[di];
            let g = dest[di + 1];
            let b = dest[di + 2];
            let a = dest[di + 3];
            let curr_pixel = a != 0;

            if curr_pixel && ix == image_w_i - 1 && fit_bits & FIT_XBORDER_RIGHT != 0 {
                if let Some(ni) = rgba_index(dest_w, dx + 1, dy) {
                    copy_rgb_into(dest, ni, r, g, b, a, extend_alpha);
                }
            }

            if curr_pixel {
                extend_to_row_if_open(
                    dest,
                    dest_w,
                    dest_h,
                    dx,
                    dy,
                    iy,
                    image_h_i,
                    fit_bits,
                    r,
                    g,
                    b,
                    a,
                    extend_alpha,
                );
            }

            if !prev_pixel && curr_pixel {
                if ix != 0 || fit_bits & FIT_XBORDER_LEFT != 0 {
                    if let Some(ni) = rgba_index(dest_w, dx - 1, dy) {
                        copy_rgb_into(dest, ni, r, g, b, a, extend_alpha);
                    }
                }
            } else if prev_pixel && !curr_pixel {
                if let Some(pi) = rgba_index(dest_w, dx - 1, dy) {
                    let pr = dest[pi];
                    let pg = dest[pi + 1];
                    let pb = dest[pi + 2];
                    let pa = dest[pi + 3];
                    copy_rgb_into(dest, di, pr, pg, pb, pa, extend_alpha);
                }
            }

            if curr_pixel {
                let diag = if ix == 0
                    && iy == 0
                    && fit_bits & FIT_XBORDER_LEFT != 0
                    && fit_bits & FIT_YBORDER_TOP != 0
                {
                    Some((dx - 1, dy - 1))
                } else if ix == image_w_i - 1
                    && iy == 0
                    && fit_bits & FIT_XBORDER_RIGHT != 0
                    && fit_bits & FIT_YBORDER_TOP != 0
                {
                    Some((dx + 1, dy - 1))
                } else if ix == image_w_i - 1
                    && iy == image_h_i - 1
                    && fit_bits & FIT_XBORDER_RIGHT != 0
                    && fit_bits & FIT_YBORDER_BOTTOM != 0
                {
                    Some((dx + 1, dy + 1))
                } else if ix == 0
                    && iy == image_h_i - 1
                    && fit_bits & FIT_XBORDER_LEFT != 0
                    && fit_bits & FIT_YBORDER_BOTTOM != 0
                {
                    Some((dx - 1, dy + 1))
                } else {
                    None
                };
                if let Some((cx, cy)) = diag {
                    if let Some(ni) = rgba_index(dest_w, cx, cy) {
                        copy_rgb_into(dest, ni, r, g, b, a, extend_alpha);
                    }
                }
            }

            prev_pixel = curr_pixel;
        }
    }
    true
}

/// One atlas page produced by [`pack_named_images_to_pages`].
#[derive(Debug, Clone)]
pub struct PackedAtlasPage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    /// Top-left RGBA (engine atlas), converted from [`Self::tga`].
    pub rgba: Vec<u8>,
    /// C++ `TexturePage` dest buffer from [`add_image_data_tga`] (bottom-up A,B,G,R).
    pub tga: Vec<u8>,
    pub sprites: Vec<PackedAtlasSprite>,
    pub status: PageStatus,
}

#[derive(Debug, Clone)]
pub struct PackedAtlasSprite {
    pub key: String,
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub rotated: bool,
}

/// Pack named RGBA images onto successive C++ `TexturePage`s of `page_size`.
pub fn pack_named_images_to_pages(
    images: &[(String, u32, u32, Vec<u8>)],
    page_size: u32,
    extend_rgb: bool,
) -> Vec<PackedAtlasPage> {
    let mut remaining: Vec<(usize, ImageToPack)> = images
        .iter()
        .enumerate()
        .map(|(i, (_, w, h, _))| {
            (
                i,
                ImageToPack {
                    image_index: i,
                    width: *w,
                    height: *h,
                },
            )
        })
        .collect();
    remaining.sort_by(|a, b| {
        (b.1.width * b.1.height)
            .cmp(&(a.1.width * a.1.height))
            .then_with(|| b.1.height.cmp(&a.1.height))
    });

    let mut pages = Vec::new();
    let mut page_id = 1u32;
    while !remaining.is_empty() {
        let mut page = TexturePage::new(page_size, page_size);
        page.all_sides_border = extend_rgb;
        page.set_id(page_id);
        let mut leftover = Vec::new();
        for (idx, img) in remaining.drain(..) {
            if !page.add_image(&img) {
                leftover.push((idx, img));
            }
        }
        if page.placements.is_empty() {
            break;
        }
        remaining = leftover;

        let mut tga = vec![0u8; (page_size * page_size * 4) as usize];
        let mut sprites = Vec::new();
        for placement in &page.placements {
            let (key, w, h, src) = &images[placement.image_index];
            add_image_data_tga(
                &mut tga,
                page_size,
                page_size,
                4,
                src,
                *w,
                *h,
                4,
                placement.left,
                placement.top,
                placement.rotated,
            );
            sprites.push(PackedAtlasSprite {
                key: key.clone(),
                left: placement.left,
                top: placement.top,
                right: placement.right,
                bottom: placement.bottom,
                rotated: placement.rotated,
            });
        }
        let mut rgba = cpp_tga4_to_rgba_top_left(&tga, page_size, page_size);
        if extend_rgb {
            for placement in &page.placements {
                let placed_w = placement.right.saturating_sub(placement.left);
                let placed_h = placement.bottom.saturating_sub(placement.top);
                extend_image_edges(
                    &mut rgba,
                    page_size,
                    page_size,
                    placement.left,
                    placement.top,
                    placed_w,
                    placed_h,
                    placement.fit_bits,
                    false,
                );
            }
        }
        pages.push(PackedAtlasPage {
            id: page_id,
            width: page_size,
            height: page_size,
            rgba,
            tga,
            sprites,
            status: page.status,
        });
        page_id += 1;
    }
    pages
}

/// One MappedImage written by C++ `ImagePacker::generateINIFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedImageIniEntry {
    pub name: String,
    pub left: u32,
    pub top: u32,
    /// Exclusive right (`m_pagePos.hi.x + 1` in C++).
    pub right: u32,
    /// Exclusive bottom (`m_pagePos.hi.y + 1` in C++).
    pub bottom: u32,
    pub rotated_90_cw: bool,
}

/// One texture page's INI block list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedImageIniPage {
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub status: PageStatus,
    pub images: Vec<MappedImageIniEntry>,
}

impl From<&PackedAtlasPage> for MappedImageIniPage {
    fn from(page: &PackedAtlasPage) -> Self {
        Self {
            id: page.id,
            width: page.width,
            height: page.height,
            status: page.status,
            images: page
                .sprites
                .iter()
                .map(|s| MappedImageIniEntry {
                    name: s.key.clone(),
                    left: s.left,
                    top: s.top,
                    right: s.right,
                    bottom: s.bottom,
                    rotated_90_cw: s.rotated,
                })
                .collect(),
        }
    }
}

/// C++ `ImagePacker::generateINIFile` text (`{outputFile}_{id:03}.tga`, skip PAGE_ERROR).
pub fn generate_mapped_image_ini(output_file: &str, pages: &[MappedImageIniPage]) -> String {
    let mut out = String::new();
    out.push_str("; ------------------------------------------------------------\n");
    out.push_str("; Do NOT edit by hand, ImagePacker.exe auto generated INI file\n");
    out.push_str("; ------------------------------------------------------------\n\n");
    for page in pages {
        if page.status.contains(PageStatus::PAGE_ERROR) {
            continue;
        }
        for image in &page.images {
            let status = if image.rotated_90_cw {
                "ROTATED_90_CLOCKWISE"
            } else {
                "NONE"
            };
            out.push_str(&format!("MappedImage {}\n", image.name));
            out.push_str(&format!("  Texture = {}_{:03}.tga\n", output_file, page.id));
            out.push_str(&format!("  TextureWidth = {}\n", page.width));
            out.push_str(&format!("  TextureHeight = {}\n", page.height));
            out.push_str(&format!(
                "  Coords = Left:{} Top:{} Right:{} Bottom:{}\n",
                image.left, image.top, image.right, image.bottom
            ));
            out.push_str(&format!("  Status = {}\n", status));
            out.push_str("End\n\n");
        }
    }
    out
}

/// Generate MappedImage INI from packed atlas pages.
pub fn generate_mapped_image_ini_from_pages(
    output_file: &str,
    pages: &[PackedAtlasPage],
) -> String {
    let ini_pages: Vec<MappedImageIniPage> = pages.iter().map(MappedImageIniPage::from).collect();
    generate_mapped_image_ini(output_file, &ini_pages)
}

/// C++ `TexturePage::addImageData` copy into a bottom-up TGA dest.
///
/// Source is top-left RGB(A). Dest is C++ packed TGA (`destBPP` 3 or 4):
/// 4bpp stores A,B,G,R per pixel; 3bpp stores B,G,R. Dest rows are flipped
/// (`destHeight-1-y`). 90° CW uses C++ `pagePos.lo.y + x` / `size.y-1-y`.
pub fn add_image_data_tga(
    dest: &mut [u8],
    dest_w: u32,
    dest_h: u32,
    dest_bpp: u32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
    src_bpp: u32,
    page_x: u32,
    page_y: u32,
    rotated_90_cw: bool,
) -> bool {
    if dest_bpp != 3 && dest_bpp != 4 {
        return false;
    }
    if src_bpp != 3 && src_bpp != 4 {
        return false;
    }
    let dest_len = dest_w as usize * dest_h as usize * dest_bpp as usize;
    let src_len = src_w as usize * src_h as usize * src_bpp as usize;
    if dest.len() < dest_len || src.len() < src_len {
        return false;
    }
    let copy_pixel = |dest: &mut [u8], di: usize, src: &[u8], si: usize| {
        let r = src[si];
        let g = src[si + 1];
        let b = src[si + 2];
        let a = if src_bpp == 4 { src[si + 3] } else { 0xFF };
        if dest_bpp == 4 {
            dest[di] = a;
            dest[di + 1] = b;
            dest[di + 2] = g;
            dest[di + 3] = r;
        } else {
            dest[di] = b;
            dest[di + 1] = g;
            dest[di + 2] = r;
        }
    };
    if !rotated_90_cw {
        if page_x + src_w > dest_w || page_y + src_h > dest_h {
            return false;
        }
        for y in 0..src_h {
            for x in 0..src_w {
                let si = ((y * src_w + x) * src_bpp) as usize;
                let dest_row = (dest_h - 1) - (page_y + y);
                let dest_col = page_x + x;
                let di = ((dest_row * dest_w + dest_col) * dest_bpp) as usize;
                copy_pixel(dest, di, src, si);
            }
        }
    } else {
        // C++ 90° CW: dest y = page.lo.y + x, dest x = page.lo.x + (srcH-1-y)
        if page_x + src_h > dest_w || page_y + src_w > dest_h {
            return false;
        }
        // C++ TexturePage.cpp:589-635: TGA src is already bottom-up so it indexes
        // `(size.y-1-y)`. Our `src` is top-left RGBA — visual row `y` is `y * src_w`.
        // Dest is still C++ TGA: dest_row = destH-1-(pageY+x), dest_col = pageX+(srcH-1-y).
        for y in 0..src_h {
            for x in 0..src_w {
                let si = ((y * src_w + x) * src_bpp) as usize;
                let dest_row = (dest_h - 1) - (page_y + x);
                let dest_col = page_x + (src_h - 1 - y);
                let di = ((dest_row * dest_w + dest_col) * dest_bpp) as usize;
                copy_pixel(dest, di, src, si);
            }
        }
    }
    true
}

/// C++ TGA dest (bottom-up A,B,G,R) → top-left RGBA for engine atlas / GPUI.
fn cpp_tga4_to_rgba_top_left(tga: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    for row in 0..h {
        for col in 0..w {
            let ti = ((row * w + col) * 4) as usize;
            if ti + 3 >= tga.len() {
                continue;
            }
            let a = tga[ti];
            let b = tga[ti + 1];
            let g = tga[ti + 2];
            let r = tga[ti + 3];
            let visual_y = h - 1 - row;
            let ri = ((visual_y * w + col) * 4) as usize;
            rgba[ri] = r;
            rgba[ri + 1] = g;
            rgba[ri + 2] = b;
            rgba[ri + 3] = a;
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::{
        FIT_XBORDER_LEFT, FIT_XBORDER_RIGHT, FIT_XGUTTER, FIT_YBORDER_BOTTOM, FIT_YBORDER_TOP,
        FIT_YGUTTER, ImagePlacement, TexturePage, add_image_data_tga,
    };

    fn tga_px(buf: &[u8], dest_w: u32, col: u32, row: u32) -> [u8; 4] {
        let i = ((row * dest_w + col) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn add_image_data_tga_matches_cpp_bottom_up_abgr_and_90cw() {
        // 1x2 top-left RGBA: visual top red, visual bottom green. Dest 2x2 TGA 4bpp.
        let src = [
            255u8, 0, 0, 255, // (0,0) red
            0, 255, 0, 255, // (0,1) green
        ];
        let mut dest = vec![0u8; 2 * 2 * 4];
        assert!(add_image_data_tga(
            &mut dest, 2, 2, 4, &src, 1, 2, 4, 0, 0, false
        ));
        // Non-rotated: dest_row = destH-1-(pageY+y) so visual top → TGA row 1.
        assert_eq!(tga_px(&dest, 2, 0, 1), [255, 0, 0, 255]); // red ABGR
        assert_eq!(tga_px(&dest, 2, 0, 0), [255, 0, 255, 0]); // green ABGR

        let mut rot = vec![0u8; 2 * 2 * 4];
        assert!(add_image_data_tga(
            &mut rot, 2, 2, 4, &src, 1, 2, 4, 0, 0, true
        ));
        // C++ TexturePage.cpp:601-603 — dest_row = destH-1-(pageY+x), dest_col = pageX+(srcH-1-y).
        // 1-wide: x=0 ⇒ dest_row=1 for both pixels; y=0 red → col=1; y=1 green → col=0.
        assert_eq!(
            tga_px(&rot, 2, 0, 1),
            [255, 0, 255, 0],
            "C++ 90° CW green at dest (col=0,row=1)"
        );
        assert_eq!(
            tga_px(&rot, 2, 1, 1),
            [255, 0, 0, 255],
            "C++ 90° CW red at dest (col=1,row=1)"
        );
        assert_eq!(tga_px(&rot, 2, 0, 0), [0, 0, 0, 0]);
        assert_eq!(tga_px(&rot, 2, 1, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn keeps_first_placement_accessible() {
        let mut page = TexturePage::new(512, 512);
        page.add_image_placement(ImagePlacement {
            image_index: 0,
            left: 5,
            top: 7,
            right: 35,
            bottom: 39,
            rotated: false,
            fit_bits: 0,
            gutter_used: (0, 0),
        });
        assert_eq!(page.get_first_image().map(|p| p.left), Some(5));
    }

    #[test]
    fn canvas_scan_pack_places_images_without_overlap() {
        let mut page = TexturePage::new(64, 64);
        let images = [
            super::ImageToPack {
                image_index: 0,
                width: 32,
                height: 32,
            },
            super::ImageToPack {
                image_index: 1,
                width: 32,
                height: 16,
            },
            super::ImageToPack {
                image_index: 2,
                width: 16,
                height: 16,
            },
        ];
        assert!(page.pack_images(&images));
        assert_eq!(page.placements.len(), 3);
        for a in 0..page.placements.len() {
            for b in (a + 1)..page.placements.len() {
                let p = page.placements[a];
                let q = page.placements[b];
                let overlap =
                    p.left < q.right && q.left < p.right && p.top < q.bottom && q.top < p.bottom;
                assert!(!overlap, "overlap {p:?} {q:?}");
            }
            let p = page.placements[a];
            assert!(p.right <= 64 && p.bottom <= 64);
        }
        let too_big = [super::ImageToPack {
            image_index: 9,
            width: 128,
            height: 8,
        }];
        let mut fail = TexturePage::new(64, 64);
        assert!(!fail.pack_images(&too_big));
        assert!(fail.status.contains(super::PageStatus::CANT_ADD_IMAGE_DATA));
    }

    #[test]
    fn add_image_tries_ninety_degree_cw_like_cpp() {
        let mut page = TexturePage::new(32, 48);
        assert!(page.add_image(&super::ImageToPack {
            image_index: 0,
            width: 40,
            height: 10,
        }));
        assert!(page.placements[0].rotated);
        assert_eq!(page.placements[0].right - page.placements[0].left, 10);
        assert_eq!(page.placements[0].bottom - page.placements[0].top, 40);
    }

    #[test]
    fn pack_named_images_to_pages_uses_texture_page_not_skyline_crate() {
        let red = vec![255u8, 0, 0, 255].repeat(8 * 8);
        let blue = vec![0u8, 0, 255, 255].repeat(8 * 8);
        let pages = super::pack_named_images_to_pages(
            &[("Red".into(), 8, 8, red), ("Blue".into(), 8, 8, blue)],
            24,
            true,
        );
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].sprites.len(), 2);
        assert!(pages[0].rgba.iter().any(|&b| b != 0));
        assert!(pages[0].tga.iter().any(|&b| b != 0));
    }

    #[test]
    fn pack_named_images_to_pages_blits_via_add_image_data_tga() {
        // 1x2 fits a 2x2 page unrotated; tga must be C++ addImageData dest.
        let col = vec![
            255u8, 0, 0, 255, // top red
            0, 255, 0, 255, // bottom green
        ];
        let pages =
            super::pack_named_images_to_pages(&[("Col".into(), 1, 2, col.clone())], 2, false);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].sprites.len(), 1);
        assert!(!pages[0].sprites[0].rotated);
        let left = pages[0].sprites[0].left;
        let top = pages[0].sprites[0].top;
        let mut oracle = vec![0u8; 2 * 2 * 4];
        assert!(add_image_data_tga(
            &mut oracle,
            2,
            2,
            4,
            &col,
            1,
            2,
            4,
            left,
            top,
            false
        ));
        assert_eq!(
            pages[0].tga, oracle,
            "pack must call add_image_data_tga for the page TGA dest"
        );

        // 6x8 fills the left of an 8x8 page (2px column leftover). 3x1 cannot
        // fit that column unless 90° CW → 1x3 (C++ addImage).
        let block = vec![128u8, 0, 0, 255].repeat(6 * 8);
        let strip = vec![
            255u8, 0, 0, 255, // R
            0, 255, 0, 255, // G
            0, 0, 255, 255, // B
        ];
        let pages = super::pack_named_images_to_pages(
            &[
                ("Block".into(), 6, 8, block.clone()),
                ("Strip".into(), 3, 1, strip.clone()),
            ],
            8,
            false,
        );
        assert_eq!(pages.len(), 1);
        let strip_sprite = pages[0]
            .sprites
            .iter()
            .find(|s| s.key == "Strip")
            .expect("strip packed");
        assert!(
            strip_sprite.rotated,
            "3x1 beside a 6x8 on 8x8 must 90° CW like C++ addImage"
        );
        let block_sprite = pages[0]
            .sprites
            .iter()
            .find(|s| s.key == "Block")
            .expect("block packed");
        let mut oracle = vec![0u8; 8 * 8 * 4];
        assert!(add_image_data_tga(
            &mut oracle,
            8,
            8,
            4,
            &block,
            6,
            8,
            4,
            block_sprite.left,
            block_sprite.top,
            block_sprite.rotated,
        ));
        assert!(add_image_data_tga(
            &mut oracle,
            8,
            8,
            4,
            &strip,
            3,
            1,
            4,
            strip_sprite.left,
            strip_sprite.top,
            strip_sprite.rotated,
        ));
        assert_eq!(pages[0].tga, oracle);
        // C++ dest_row = destH-1-(pageY+x); x=0 red at dest_col = pageX+(srcH-1-0)=left.
        assert_eq!(
            tga_px(&pages[0].tga, 8, strip_sprite.left, 7 - strip_sprite.top),
            [255, 0, 0, 255],
            "rotated strip x=0 red at dest_row=destH-1-(top+0)"
        );
    }

    #[test]
    fn generate_ini_file_matches_cpp_mapped_image_tokens() {
        let red = vec![255u8, 0, 0, 255].repeat(4 * 4);
        let pages = super::pack_named_images_to_pages(&[("ButtonUp".into(), 4, 4, red)], 16, false);
        assert_eq!(pages.len(), 1);
        let sprite = &pages[0].sprites[0];
        assert_eq!(sprite.key, "ButtonUp");
        assert_eq!(sprite.right, sprite.left + 4);
        assert_eq!(sprite.bottom, sprite.top + 4);

        let ini = super::generate_mapped_image_ini_from_pages("ArtPack", &pages);
        assert!(ini.contains("; Do NOT edit by hand, ImagePacker.exe auto generated INI file"));
        let keys = [
            "MappedImage ButtonUp",
            "Texture = ArtPack_001.tga",
            "TextureWidth = 16",
            "TextureHeight = 16",
            &format!(
                "Coords = Left:{} Top:{} Right:{} Bottom:{}",
                sprite.left, sprite.top, sprite.right, sprite.bottom
            ),
            "Status = NONE",
            "End",
        ];
        let mut last = 0usize;
        for key in keys {
            let at = ini[last..]
                .find(key)
                .unwrap_or_else(|| panic!("{key} missing after offset {last}\n{ini}"));
            last += at + key.len();
        }
        assert!(!ini.contains(".png"));

        let mut error_page = super::MappedImageIniPage::from(&pages[0]);
        error_page.status = super::PageStatus::PAGE_ERROR;
        let rotated = super::MappedImageIniPage {
            id: 2,
            width: 32,
            height: 32,
            status: super::PageStatus::READY,
            images: vec![super::MappedImageIniEntry {
                name: "Tall".into(),
                left: 0,
                top: 0,
                right: 8,
                bottom: 16,
                rotated_90_cw: true,
            }],
        };
        let mixed = super::generate_mapped_image_ini("ArtPack", &[error_page, rotated]);
        assert!(
            !mixed.contains("MappedImage ButtonUp"),
            "C++ skips PAGE_ERROR pages"
        );
        assert!(mixed.contains("MappedImage Tall"));
        assert!(mixed.contains("Texture = ArtPack_002.tga"));
        assert!(mixed.contains("Status = ROTATED_90_CLOCKWISE"));
        assert!(mixed.contains("Coords = Left:0 Top:0 Right:8 Bottom:16"));
    }

    #[test]
    fn rgb_extend_bleeds_edge_into_open_gutter_like_cpp() {
        let mut dest = vec![0u8; 6 * 6 * 4];
        let src = [
            255u8, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];
        let fit = FIT_XBORDER_LEFT | FIT_XBORDER_RIGHT | FIT_YBORDER_TOP | FIT_YBORDER_BOTTOM;
        assert!(TexturePage::blit_with_rgb_extend(
            &mut dest, 6, 6, &src, 2, 2, 2, 2, true, fit
        ));
        let at = |x: u32, y: u32| {
            let i = ((y * 6 + x) * 4) as usize;
            (dest[i], dest[i + 1], dest[i + 2], dest[i + 3])
        };
        assert_eq!(at(2, 2), (255, 0, 0, 255));
        // Left/top FIT borders: RGB bled, alpha stays 0 (C++ extendAlpha=FALSE).
        assert_eq!(at(1, 2), (255, 0, 0, 0));
        assert_eq!(at(2, 1), (255, 0, 0, 0));
        assert_eq!(at(1, 1), (255, 0, 0, 0), "diagonal top-left corner");
        assert_eq!(at(0, 0), (0, 0, 0, 0));
    }

    #[test]
    fn extend_image_edges_requires_fit_border_bits_like_cpp() {
        let mut dest = vec![0u8; 4 * 4 * 4];
        let src = [10u8, 20, 30, 255];
        assert!(TexturePage::blit_with_rgb_extend(
            &mut dest, 4, 4, &src, 1, 1, 1, 1, true, 0
        ));
        let sample = |buf: &[u8], x: u32, y: u32| {
            let i = ((y * 4 + x) * 4) as usize;
            (buf[i], buf[i + 1], buf[i + 2], buf[i + 3])
        };
        assert_eq!(sample(&dest, 1, 1), (10, 20, 30, 255));
        assert_eq!(sample(&dest, 0, 1), (0, 0, 0, 0), "no FIT_XBORDER_LEFT");
        assert_eq!(sample(&dest, 1, 0), (0, 0, 0, 0), "no FIT_YBORDER_TOP");
        assert!(super::extend_image_edges(
            &mut dest,
            4,
            4,
            1,
            1,
            1,
            1,
            FIT_XBORDER_LEFT | FIT_YBORDER_TOP,
            false,
        ));
        assert_eq!(sample(&dest, 0, 1), (10, 20, 30, 0));
        // 1x1: imageHeight/2==0 so extendToRowIfOpen never takes the "up" branch.
        assert_eq!(
            sample(&dest, 1, 0),
            (0, 0, 0, 0),
            "1x1 cannot FIT_YBORDER_TOP via extendToRowIfOpen"
        );
        assert_eq!(
            sample(&dest, 0, 0),
            (10, 20, 30, 0),
            "diagonal when both FIT bits"
        );
    }

    #[test]
    fn extend_image_edges_half_image_row_rule_like_cpp() {
        // 1x3 column: y=0 top half extends up; y=2 bottom half extends down.
        let mut dest = vec![0u8; 3 * 5 * 4];
        let src = [
            1u8, 2, 3, 255, // y=0
            4, 5, 6, 255, // y=1
            7, 8, 9, 255, // y=2
        ];
        let fit = FIT_YBORDER_TOP | FIT_YBORDER_BOTTOM;
        assert!(TexturePage::blit_with_rgb_extend(
            &mut dest, 3, 5, &src, 1, 3, 1, 1, true, fit
        ));
        let at = |x: u32, y: u32| {
            let i = ((y * 3 + x) * 4) as usize;
            (dest[i], dest[i + 1], dest[i + 2], dest[i + 3])
        };
        assert_eq!(
            at(1, 0),
            (1, 2, 3, 0),
            "top-half extends up into FIT_YBORDER_TOP"
        );
        assert_eq!(at(1, 4), (7, 8, 9, 0), "bottom-half extends down");
        assert_eq!(at(1, 1), (1, 2, 3, 255));
    }

    #[test]
    fn pack_tries_ninety_degree_clockwise_like_cpp_add_image() {
        // 40x10 does not fit a 32x32 page unless rotated to 10x40.
        let mut page = TexturePage::new(32, 48);
        let images = [super::ImageToPack {
            image_index: 0,
            width: 40,
            height: 10,
        }];
        assert!(page.pack_images(&images));
        assert_eq!(page.placements.len(), 1);
        let p = page.placements[0];
        assert!(p.rotated, "C++ second try is ROTATED90C");
        assert_eq!(p.right - p.left, 10);
        assert_eq!(p.bottom - p.top, 40);
        assert!(p.right <= 32 && p.bottom <= 48);
    }

    #[test]
    fn add_image_reserves_extend_rgb_fit_borders_like_cpp() {
        let mut page = TexturePage::new(16, 16);
        page.all_sides_border = true;
        assert!(page.add_image(&super::ImageToPack {
            image_index: 0,
            width: 8,
            height: 8,
        }));
        let p = page.placements[0];
        assert_eq!(p.right - p.left, 8);
        assert_eq!(p.bottom - p.top, 8);
        assert_eq!(p.left, 1, "FIT_XBORDER_LEFT shifts pagePos.lo.x");
        assert_eq!(p.top, 1, "FIT_YBORDER_TOP shifts pagePos.lo.y");
        assert!(p.fit_bits & FIT_XBORDER_LEFT != 0);
        assert!(p.fit_bits & FIT_XBORDER_RIGHT != 0);
        assert!(p.fit_bits & FIT_YBORDER_TOP != 0);
        assert!(p.fit_bits & FIT_YBORDER_BOTTOM != 0);
        // Second 8x8 + 2px border cannot fit 16x16 (needs 10x10 leftover).
        assert!(!page.add_image(&super::ImageToPack {
            image_index: 1,
            width: 8,
            height: 8,
        }));
    }

    #[test]
    fn add_image_gutter_fit_bits_like_cpp_build_fit_region() {
        let mut page = TexturePage::new(32, 32);
        page.x_gutter = 2;
        page.y_gutter = 1;
        assert!(page.add_image(&super::ImageToPack {
            image_index: 0,
            width: 8,
            height: 8,
        }));
        let p = page.placements[0];
        assert_eq!(p.gutter_used, (2, 1));
        assert!(p.fit_bits & FIT_XGUTTER != 0);
        assert!(p.fit_bits & FIT_YGUTTER != 0);
        assert_eq!(p.right - p.left, 8);
        assert_eq!(p.left, 0);
    }
}

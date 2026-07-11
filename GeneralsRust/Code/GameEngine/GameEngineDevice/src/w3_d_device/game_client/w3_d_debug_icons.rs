//! Debug icon render-object compatibility state.
//!
//! C++ source: `GameEngineDevice/Source/W3DDevice/GameClient/W3DDebugIcons.cpp`.
//! The original object exists only for `_DEBUG || _INTERNAL` builds and emits
//! many short-lived pathfinding icons as prelit, alpha-blended quads.  This Rust
//! port preserves the static queue and CPU geometry construction so a renderer can
//! submit the same batches without coupling tests to a graphics device.

use std::sync::{Mutex, MutexGuard};

use game_engine::map_object::{Coord3D, MAP_XY_FACTOR};

/// Maximum icons stored by the C++ static array.
pub const MAX_ICONS: usize = 100_000;

/// Maximum quads emitted in a single C++ render batch.
pub const MAX_RECT: usize = 5_000;

/// Number of frames over which icons fade out.
pub const FADE_FRAMES: i32 = 100;

/// Base alpha used before fade-out.
pub const BASE_ALPHA: u8 = 64;

/// C++ `RGBColor` with real components in the range `0..=1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBColor {
    /// Red component.
    pub red: f32,
    /// Green component.
    pub green: f32,
    /// Blue component.
    pub blue: f32,
}

impl RGBColor {
    /// Construct a color from real RGB components.
    #[must_use]
    pub const fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    /// Match C++ `RGBColor::getAsInt`.
    #[must_use]
    pub fn get_as_int(self) -> u32 {
        ((self.red * 255.0) as u32) << 16
            | ((self.green * 255.0) as u32) << 8
            | ((self.blue * 255.0) as u32)
    }
}

/// Queued C++ `DebugIcon`.
#[derive(Debug, Clone)]
pub struct DebugIcon {
    /// World-space icon center.
    pub position: Coord3D,
    /// Square width.
    pub width: f32,
    /// Icon color.
    pub color: RGBColor,
    /// Frame when this icon disappears.
    pub end_frame: i32,
}

/// Prelit vertex emitted by `W3DDebugIcons::Render`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DebugIconVertex {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Z coordinate.
    pub z: f32,
    /// Packed `0xAARRGGBB` diffuse color.
    pub diffuse: u32,
    /// First texture coordinate U. C++ writes zero.
    pub u1: f32,
    /// First texture coordinate V. C++ writes zero.
    pub v1: f32,
}

/// CPU-side batch equivalent to one dynamic VB/IB submission.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DebugIconBatch {
    /// Vertices for up to `MAX_RECT` icon quads.
    pub vertices: Vec<DebugIconVertex>,
    /// Indices, six per quad.
    pub indices: Vec<u16>,
}

/// Object-space sphere used by the render object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereClass {
    /// Sphere center.
    pub center: [f32; 3],
    /// Sphere radius.
    pub radius: f32,
}

/// Object-space axis-aligned bounding box used by the render object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABoxClass {
    /// Minimum point.
    pub min: [f32; 3],
    /// Maximum point.
    pub max: [f32; 3],
}

#[derive(Debug, Default)]
struct DebugIconStore {
    icons: Vec<DebugIcon>,
    max_icons_seen: usize,
}

impl DebugIconStore {
    const fn new() -> Self {
        Self {
            icons: Vec::new(),
            max_icons_seen: 0,
        }
    }
}

static DEBUG_ICONS: Mutex<DebugIconStore> = Mutex::new(DebugIconStore::new());

/// W3D debug icon render object compatibility adapter.
#[derive(Debug, Default)]
pub struct W3DDebugIcons;

impl W3DDebugIcons {
    /// Construct the debug icon render object and allocate the C++ static icon array.
    #[must_use]
    pub fn new() -> Self {
        allocate_icons_array();
        Self
    }

    /// C++ `Class_ID`.
    #[must_use]
    pub const fn class_id(&self) -> i32 {
        0
    }

    /// C++ `Cast_Ray`.
    #[must_use]
    pub const fn cast_ray(&self) -> bool {
        false
    }

    /// C++ `Clone`.
    #[must_use]
    pub fn clone_render_object(&self) -> W3DDebugIcons {
        W3DDebugIcons::new()
    }

    /// C++ `Get_Obj_Space_Bounding_Sphere`.
    #[must_use]
    pub fn get_obj_space_bounding_sphere(
        &self,
        water_extent_x: f32,
        water_extent_y: f32,
    ) -> SphereClass {
        let center = [water_extent_x, water_extent_y, 50.0 * MAP_XY_FACTOR];
        let radius = (center[0] * center[0] + center[1] * center[1] + center[2] * center[2]).sqrt();
        SphereClass { center, radius }
    }

    /// C++ `Get_Obj_Space_Bounding_Box`.
    #[must_use]
    pub fn get_obj_space_bounding_box(
        &self,
        water_extent_x: f32,
        water_extent_y: f32,
    ) -> AABoxClass {
        AABoxClass {
            min: [-2.0 * water_extent_x, -2.0 * water_extent_y, 0.0],
            max: [
                2.0 * water_extent_x,
                2.0 * water_extent_y,
                100.0 * MAP_XY_FACTOR,
            ],
        }
    }

    /// Static C++ `addIcon`, with the current logic frame supplied by the caller.
    pub fn add_icon(
        pos: Option<&Coord3D>,
        width: f32,
        num_frames_duration: i32,
        color: RGBColor,
        current_frame: i32,
    ) {
        add_icon(pos, width, num_frames_duration, color, current_frame);
    }

    /// Build the CPU geometry that C++ `Render` writes into dynamic buffers.
    #[must_use]
    pub fn render_batches(&self, current_frame: i32) -> Vec<DebugIconBatch> {
        render_batches(current_frame)
    }
}

impl Drop for W3DDebugIcons {
    fn drop(&mut self) {
        let mut store = lock_store();
        store.icons.clear();
        store.max_icons_seen = 0;
    }
}

/// Free function matching the C++ global `addIcon` helper.
pub fn add_icon(
    pos: Option<&Coord3D>,
    width: f32,
    num_frames_duration: i32,
    color: RGBColor,
    current_frame: i32,
) {
    let mut store = lock_store();
    if let Some(pos) = pos {
        if store.icons.len() >= MAX_ICONS {
            return;
        }
        store.icons.push(DebugIcon {
            position: pos.clone(),
            width,
            color,
            end_frame: current_frame + num_frames_duration,
        });
    } else {
        if store.icons.len() > store.max_icons_seen {
            store.max_icons_seen = store.icons.len();
        }
        store.icons.clear();
    }
}

/// Clear and reallocate the static icon array.
pub fn allocate_icons_array() {
    let mut store = lock_store();
    store.icons = Vec::with_capacity(MAX_ICONS);
    store.max_icons_seen = 0;
}

/// Return the number of currently queued icons.
#[must_use]
pub fn debug_icon_count() -> usize {
    lock_store().icons.len()
}

/// Return the maximum count observed during null-position clears.
#[must_use]
pub fn max_icons_seen() -> usize {
    lock_store().max_icons_seen
}

fn render_batches(current_frame: i32) -> Vec<DebugIconBatch> {
    let mut store = lock_store();
    if store.icons.is_empty() {
        return Vec::new();
    }

    let mut any_vanished = false;
    let mut batches = Vec::new();
    let num_rect = store.icons.len().min(MAX_RECT);
    let mut k = 0usize;

    while k < store.icons.len() {
        let mut batch = DebugIconBatch {
            vertices: Vec::with_capacity(num_rect * 4),
            indices: Vec::with_capacity(num_rect * 6),
        };

        while batch.vertices.len() < num_rect * 4 && k < store.icons.len() {
            let icon = &store.icons[k];
            k += 1;

            let frames_left = icon.end_frame - current_frame;
            if frames_left < 1 {
                any_vanished = true;
                continue;
            }

            append_icon_quad(&mut batch, icon, frames_left);
        }

        if batch.vertices.is_empty() {
            break;
        }
        batches.push(batch);
    }

    if any_vanished {
        compress_icons_array(&mut store, current_frame);
    }

    batches
}

fn append_icon_quad(batch: &mut DebugIconBatch, icon: &DebugIcon, frames_left: i32) {
    let mut alpha = f32::from(BASE_ALPHA);
    if frames_left < FADE_FRAMES {
        alpha *= frames_left as f32 / FADE_FRAMES as f32;
    }

    let diffuse = icon.color.get_as_int() | ((alpha as u32) << 24);
    let half_width = icon.width / 2.0;
    let base = batch.vertices.len() as u16;
    let x = icon.position.x;
    let y = icon.position.y;
    let z = icon.position.z;

    batch.vertices.extend_from_slice(&[
        DebugIconVertex {
            x: x - half_width,
            y: y - half_width,
            z,
            diffuse,
            u1: 0.0,
            v1: 0.0,
        },
        DebugIconVertex {
            x: x + half_width,
            y: y - half_width,
            z,
            diffuse,
            u1: 0.0,
            v1: 0.0,
        },
        DebugIconVertex {
            x: x + half_width,
            y: y + half_width,
            z,
            diffuse,
            u1: 0.0,
            v1: 0.0,
        },
        DebugIconVertex {
            x: x - half_width,
            y: y + half_width,
            z,
            diffuse,
            u1: 0.0,
            v1: 0.0,
        },
    ]);
    batch
        .indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn compress_icons_array(store: &mut DebugIconStore, current_frame: i32) {
    if store.icons.is_empty() {
        return;
    }

    let mut new_num = 0usize;
    for i in 0..store.icons.len() {
        if store.icons[i].end_frame >= current_frame && i > new_num {
            store.icons[new_num] = store.icons[i].clone();
            new_num += 1;
        }
    }
    store.icons.truncate(new_num);
}

fn lock_store() -> MutexGuard<'static, DebugIconStore> {
    DEBUG_ICONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coord(x: f32, y: f32, z: f32) -> Coord3D {
        Coord3D::new(x, y, z)
    }

    fn red() -> RGBColor {
        RGBColor::new(1.0, 0.0, 0.0)
    }

    #[test]
    fn add_icon_none_clears_buffer_and_tracks_max() {
        allocate_icons_array();
        add_icon(Some(&coord(1.0, 2.0, 3.0)), 4.0, 10, red(), 20);
        add_icon(Some(&coord(2.0, 3.0, 4.0)), 4.0, 10, red(), 20);

        add_icon(None, 0.0, 0, red(), 20);

        assert_eq!(debug_icon_count(), 0);
        assert_eq!(max_icons_seen(), 2);
    }

    #[test]
    fn add_icon_sets_end_frame_from_current_logic_frame() {
        allocate_icons_array();
        add_icon(Some(&coord(1.0, 2.0, 3.0)), 4.0, 200, red(), 30);

        let batches = render_batches(31);

        assert_eq!(debug_icon_count(), 1);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].vertices[0].diffuse >> 24, u32::from(BASE_ALPHA));
    }

    #[test]
    fn expired_icons_at_front_are_removed_after_render_geometry_build() {
        allocate_icons_array();
        add_icon(Some(&coord(0.0, 0.0, 0.0)), 2.0, 1, red(), 10);
        add_icon(Some(&coord(4.0, 4.0, 1.0)), 2.0, 100, red(), 10);

        let batches = render_batches(12);

        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].vertices.len(), 4);
        assert_eq!(debug_icon_count(), 1);
    }

    #[test]
    fn add_icon_ignores_entries_after_max_icons() {
        allocate_icons_array();
        for i in 0..MAX_ICONS {
            add_icon(Some(&coord(i as f32, 0.0, 0.0)), 2.0, 10, red(), 0);
        }

        add_icon(Some(&coord(999_999.0, 0.0, 0.0)), 2.0, 10, red(), 0);

        assert_eq!(debug_icon_count(), MAX_ICONS);
    }

    #[test]
    fn alpha_is_64_until_fade_window_then_scales_by_frames_left() {
        allocate_icons_array();
        add_icon(Some(&coord(0.0, 0.0, 0.0)), 2.0, 200, red(), 0);
        add_icon(Some(&coord(4.0, 4.0, 0.0)), 2.0, 50, red(), 0);

        let batches = render_batches(0);
        let first_alpha = batches[0].vertices[0].diffuse >> 24;
        let second_alpha = batches[0].vertices[4].diffuse >> 24;

        assert_eq!(first_alpha, 64);
        assert_eq!(second_alpha, 32);
    }

    #[test]
    fn quad_vertices_match_centered_square_xy_with_original_z() {
        allocate_icons_array();
        add_icon(Some(&coord(10.0, 20.0, 5.0)), 4.0, 200, red(), 0);

        let batches = render_batches(0);
        let vertices = &batches[0].vertices;

        assert_eq!(
            vertices,
            &[
                DebugIconVertex {
                    x: 8.0,
                    y: 18.0,
                    z: 5.0,
                    diffuse: 0x40ff0000,
                    u1: 0.0,
                    v1: 0.0,
                },
                DebugIconVertex {
                    x: 12.0,
                    y: 18.0,
                    z: 5.0,
                    diffuse: 0x40ff0000,
                    u1: 0.0,
                    v1: 0.0,
                },
                DebugIconVertex {
                    x: 12.0,
                    y: 22.0,
                    z: 5.0,
                    diffuse: 0x40ff0000,
                    u1: 0.0,
                    v1: 0.0,
                },
                DebugIconVertex {
                    x: 8.0,
                    y: 22.0,
                    z: 5.0,
                    diffuse: 0x40ff0000,
                    u1: 0.0,
                    v1: 0.0,
                },
            ]
        );
    }

    #[test]
    fn indices_are_two_triangles_per_icon() {
        allocate_icons_array();
        add_icon(Some(&coord(0.0, 0.0, 0.0)), 2.0, 10, red(), 0);
        add_icon(Some(&coord(4.0, 4.0, 0.0)), 2.0, 10, red(), 0);

        let batches = render_batches(0);

        assert_eq!(batches[0].indices, vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn render_batching_caps_each_batch_at_5000_icons() {
        allocate_icons_array();
        for i in 0..(MAX_RECT + 3) {
            add_icon(Some(&coord(i as f32, 0.0, 0.0)), 2.0, 10, red(), 0);
        }

        let batches = render_batches(0);

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].vertices.len(), MAX_RECT * 4);
        assert_eq!(batches[0].indices.len(), MAX_RECT * 6);
        assert_eq!(batches[1].vertices.len(), 3 * 4);
        assert_eq!(batches[1].indices.len(), 3 * 6);
    }

    #[test]
    fn bounding_shapes_match_cpp_water_extent_formulas() {
        let icons = W3DDebugIcons::new();

        let sphere = icons.get_obj_space_bounding_sphere(30.0, 40.0);
        let bbox = icons.get_obj_space_bounding_box(30.0, 40.0);

        assert_eq!(sphere.center, [30.0, 40.0, 50.0 * MAP_XY_FACTOR]);
        assert!(
            (sphere.radius - (30.0_f32 * 30.0 + 40.0 * 40.0 + 500.0 * 500.0).sqrt()).abs() < 0.001
        );
        assert_eq!(bbox.min, [-60.0, -80.0, 0.0]);
        assert_eq!(bbox.max, [60.0, 80.0, 100.0 * MAP_XY_FACTOR]);
    }
}

// FILE: w3_d_radar.rs
// Ported adapter for C++ W3DDevice/Common/System/W3DRadar.cpp.

pub use game_engine::common::system::radar::{
    interpolate_color_for_height, legal_radar_point, radar_draw_positions, radar_event_marker,
    radar_to_pixel, should_refresh_w3d_object_overlay, CellShroudStatus, Coord3D, ICoord2D,
    RadarEventMarker, RadarEventMarkerKind, RadarHeroReticleRect, RadarObject, RadarPriorityType,
    RadarSystem, RadarViewBoxLine, Region3D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH,
    W3D_RADAR_OVERLAY_REFRESH_RATE,
};

/// Minimal WW3D texture-format identity used by the W3D radar adapter.
///
/// The C++ code picks the first hardware-supported format from ordered preference lists.
/// Rust's renderer owns actual GPU texture allocation elsewhere, so this enum records the
/// same logical selection without exposing a DirectX texture handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DRadarTextureFormat {
    /// C++ `WW3D_FORMAT_R8G8B8`.
    R8G8B8,
    /// C++ `WW3D_FORMAT_X8R8G8B8`.
    X8R8G8B8,
    /// C++ `WW3D_FORMAT_R5G6B5`.
    R5G6B5,
    /// C++ `WW3D_FORMAT_X1R5G5B5`.
    X1R5G5B5,
    /// C++ `WW3D_FORMAT_A8R8G8B8`.
    A8R8G8B8,
    /// C++ `WW3D_FORMAT_A4R4G4B4`.
    A4R4G4B4,
    /// No acceptable format was available.
    Unknown,
}

impl W3DRadarTextureFormat {
    fn first_supported(
        preferred: &[W3DRadarTextureFormat],
        supported: &[W3DRadarTextureFormat],
    ) -> W3DRadarTextureFormat {
        preferred
            .iter()
            .copied()
            .find(|format| supported.contains(format))
            .unwrap_or(W3DRadarTextureFormat::Unknown)
    }
}

/// Texture formats selected by `W3DRadar::initializeTextureFormats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct W3DRadarTextureFormats {
    /// Terrain background texture format.
    pub terrain: W3DRadarTextureFormat,
    /// Object/event overlay texture format.
    pub overlay: W3DRadarTextureFormat,
    /// Shroud overlay texture format.
    pub shroud: W3DRadarTextureFormat,
}

impl Default for W3DRadarTextureFormats {
    fn default() -> Self {
        Self {
            terrain: W3DRadarTextureFormat::Unknown,
            overlay: W3DRadarTextureFormat::Unknown,
            shroud: W3DRadarTextureFormat::Unknown,
        }
    }
}

/// W3D-specific radar facade.
///
/// This struct mirrors the device-owned parts of C++ `W3DRadar`: texture-format
/// selection, resource deletion, view-box invalidation, and shroud/terrain refresh
/// entry points. The underlying radar state and software texture generation live in
/// `Common::RadarSystem`, which already ports the C++ radar lists, events, coordinate
/// transforms, terrain texture, shroud texture, and object overlay rasterization.
pub struct W3DRadar {
    radar: RadarSystem,
    texture_formats: W3DRadarTextureFormats,
    texture_width: i32,
    texture_height: i32,
    resources_allocated: bool,
    reconstruct_view_box: bool,
    view_angle: f32,
    view_zoom: f32,
    view_box: [ICoord2D; 4],
}

impl W3DRadar {
    /// Create a new W3D radar adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            radar: RadarSystem::new(),
            texture_formats: W3DRadarTextureFormats::default(),
            texture_width: RADAR_CELL_WIDTH as i32,
            texture_height: RADAR_CELL_HEIGHT as i32,
            resources_allocated: false,
            reconstruct_view_box: true,
            view_angle: 0.0,
            view_zoom: 0.0,
            view_box: [ICoord2D::new(0, 0); 4],
        }
    }

    /// Subsystem init: select default texture formats and reset W3D view/resource state.
    pub fn init(&mut self) {
        self.initialize_texture_formats();
        self.reset();
        self.resources_allocated = true;
    }

    /// Per-frame update, delegating event expiry and deferred refresh behavior.
    pub fn update(&mut self, current_frame: u32) {
        self.radar.update(current_frame);
    }

    /// Reset radar and W3D-only cached state.
    pub fn reset(&mut self) {
        self.radar.reset();
        self.radar.clear_terrain_texture_rgba();
        self.radar.clear_shroud();
        self.texture_width = RADAR_CELL_WIDTH as i32;
        self.texture_height = RADAR_CELL_HEIGHT as i32;
        self.reconstruct_view_box = true;
        self.view_angle = 0.0;
        self.view_zoom = 0.0;
        self.view_box = [ICoord2D::new(0, 0); 4];
    }

    /// Initialize radar state for a newly loaded map.
    pub fn new_map(
        &mut self,
        map_min: Coord3D,
        map_max: Coord3D,
        terrain_heights: &[(f32, f32, bool)],
    ) {
        self.radar.new_map(map_min, map_max, terrain_heights);
        self.reconstruct_view_box = true;
    }

    fn world_to_radar_unclamped(&self, world: &Coord3D) -> Option<ICoord2D> {
        let extent = self.radar.map_extent();
        let x_sample = extent.width() / RADAR_CELL_WIDTH as f32;
        let y_sample = extent.height() / RADAR_CELL_HEIGHT as f32;
        if x_sample <= f32::EPSILON || y_sample <= f32::EPSILON {
            return None;
        }

        Some(ICoord2D::new(
            ((world.x - extent.lo.x) / x_sample) as i32,
            ((world.y - extent.lo.y) / y_sample) as i32,
        ))
    }

    /// Rebuild the cached view-box radar-cell offsets from tactical-view corners.
    ///
    /// C++ `W3DRadar::reconstructViewBox` stores the first corner as `(0, 0)` and
    /// each later corner as a delta from the previous corner. Drawing then starts
    /// from the current screen-origin world point and walks these cached offsets.
    pub fn reconstruct_view_box_from_corners(
        &mut self,
        corner_world: [Coord3D; 4],
        view_angle: f32,
        view_zoom: f32,
    ) -> bool {
        let mut previous = ICoord2D::new(0, 0);

        for (index, world) in corner_world.iter().enumerate() {
            let Some(radar) = self.world_to_radar_unclamped(world) else {
                return false;
            };

            self.view_box[index] = if index == 0 {
                ICoord2D::new(0, 0)
            } else {
                ICoord2D::new(radar.x - previous.x, radar.y - previous.y)
            };
            previous = radar;
        }

        self.view_angle = view_angle;
        self.view_zoom = view_zoom;
        self.reconstruct_view_box = false;
        true
    }

    /// Mirror C++ draw-time invalidation when tactical-view angle or zoom changes.
    pub fn update_view_box_camera_state(&mut self, view_angle: f32, view_zoom: f32) -> bool {
        if view_angle != self.view_angle || view_zoom != self.view_zoom {
            self.reconstruct_view_box = true;
        }

        self.reconstruct_view_box
    }

    /// Select texture formats using C++ preference ordering.
    pub fn initialize_texture_formats(&mut self) {
        let supported = [
            W3DRadarTextureFormat::R8G8B8,
            W3DRadarTextureFormat::X8R8G8B8,
            W3DRadarTextureFormat::R5G6B5,
            W3DRadarTextureFormat::X1R5G5B5,
            W3DRadarTextureFormat::A8R8G8B8,
            W3DRadarTextureFormat::A4R4G4B4,
        ];
        self.initialize_texture_formats_with_supported(&supported);
    }

    /// Select texture formats from an explicit supported-format set.
    pub fn initialize_texture_formats_with_supported(
        &mut self,
        supported: &[W3DRadarTextureFormat],
    ) {
        const TERRAIN_FORMATS: &[W3DRadarTextureFormat] = &[
            W3DRadarTextureFormat::R8G8B8,
            W3DRadarTextureFormat::X8R8G8B8,
            W3DRadarTextureFormat::R5G6B5,
            W3DRadarTextureFormat::X1R5G5B5,
        ];
        const ALPHA_FORMATS: &[W3DRadarTextureFormat] = &[
            W3DRadarTextureFormat::A8R8G8B8,
            W3DRadarTextureFormat::A4R4G4B4,
        ];

        self.texture_formats = W3DRadarTextureFormats {
            terrain: W3DRadarTextureFormat::first_supported(TERRAIN_FORMATS, supported),
            overlay: W3DRadarTextureFormat::first_supported(ALPHA_FORMATS, supported),
            shroud: W3DRadarTextureFormat::first_supported(ALPHA_FORMATS, supported),
        };
    }

    /// Delete W3D-owned radar resources.
    pub fn delete_resources(&mut self) {
        self.resources_allocated = false;
    }

    /// Mark terrain as dirty and rebuild the software terrain texture.
    pub fn refresh_terrain(&mut self) {
        self.radar.refresh_terrain();
    }

    /// Clear all shroud cells.
    pub fn clear_shroud(&mut self) {
        self.radar.clear_shroud();
    }

    /// Set one shroud cell.
    pub fn set_shroud_level(&mut self, x: i32, y: i32, status: CellShroudStatus) {
        self.radar.set_shroud_level(x, y, status);
    }

    /// Compute radar draw rectangle for a HUD window.
    #[must_use]
    pub fn draw_positions(
        &self,
        pixel_x: i32,
        pixel_y: i32,
        width: i32,
        height: i32,
    ) -> (ICoord2D, ICoord2D) {
        radar_draw_positions(pixel_x, pixel_y, width, height, self.radar.map_extent())
    }

    /// Convert a radar-cell coordinate to a screen pixel.
    #[must_use]
    pub fn radar_to_pixel(
        &self,
        radar: &ICoord2D,
        radar_upper_left_x: i32,
        radar_upper_left_y: i32,
        radar_width: i32,
        radar_height: i32,
    ) -> ICoord2D {
        radar_to_pixel(
            radar,
            radar_upper_left_x,
            radar_upper_left_y,
            radar_width,
            radar_height,
        )
    }

    /// Underlying common radar system.
    #[must_use]
    pub fn radar(&self) -> &RadarSystem {
        &self.radar
    }

    /// Mutable underlying common radar system.
    pub fn radar_mut(&mut self) -> &mut RadarSystem {
        &mut self.radar
    }

    /// Selected texture formats.
    #[must_use]
    pub fn texture_formats(&self) -> W3DRadarTextureFormats {
        self.texture_formats
    }

    /// Whether W3D radar resources are currently allocated.
    #[must_use]
    pub fn resources_allocated(&self) -> bool {
        self.resources_allocated
    }

    /// Whether the cached view box should be reconstructed.
    #[must_use]
    pub fn should_reconstruct_view_box(&self) -> bool {
        self.reconstruct_view_box
    }

    /// Cached W3D view-box deltas.
    #[must_use]
    pub fn view_box(&self) -> &[ICoord2D; 4] {
        &self.view_box
    }
}

impl Default for W3DRadar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_leaves_texture_formats_unknown_until_init() {
        let radar = W3DRadar::new();

        assert_eq!(
            radar.texture_formats(),
            W3DRadarTextureFormats {
                terrain: W3DRadarTextureFormat::Unknown,
                overlay: W3DRadarTextureFormat::Unknown,
                shroud: W3DRadarTextureFormat::Unknown,
            }
        );
    }

    #[test]
    fn texture_format_selection_uses_cpp_preference_order() {
        let mut radar = W3DRadar::new();
        radar.initialize_texture_formats_with_supported(&[
            W3DRadarTextureFormat::A4R4G4B4,
            W3DRadarTextureFormat::X1R5G5B5,
            W3DRadarTextureFormat::A8R8G8B8,
        ]);

        assert_eq!(
            radar.texture_formats(),
            W3DRadarTextureFormats {
                terrain: W3DRadarTextureFormat::X1R5G5B5,
                overlay: W3DRadarTextureFormat::A8R8G8B8,
                shroud: W3DRadarTextureFormat::A8R8G8B8,
            }
        );
    }

    #[test]
    fn reset_clears_surfaces_but_preserves_allocated_resources() {
        let mut radar = W3DRadar::new();
        radar.init();
        assert!(radar.resources_allocated());

        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(128.0, 128.0, 10.0),
            &[],
        );

        assert!(radar.resources_allocated());
        assert!(radar.should_reconstruct_view_box());
        assert!(radar
            .radar()
            .get_terrain_texture()
            .chunks_exact(4)
            .any(|pixel| pixel[3] != 0));

        radar.reset();
        assert!(radar.resources_allocated());
        assert!(radar
            .radar()
            .get_terrain_texture()
            .iter()
            .all(|byte| *byte == 0));

        radar.delete_resources();
        assert!(!radar.resources_allocated());
    }

    #[test]
    fn map_and_terrain_refresh_do_not_allocate_w3d_resources() {
        let mut radar = W3DRadar::new();
        assert!(!radar.resources_allocated());

        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(128.0, 128.0, 10.0),
            &[],
        );
        assert!(!radar.resources_allocated());

        radar.init();
        assert!(radar.resources_allocated());
        radar.delete_resources();
        assert!(!radar.resources_allocated());

        radar.refresh_terrain();
        assert!(!radar.resources_allocated());
    }

    #[test]
    fn adapter_exposes_w3d_coordinate_helpers() {
        let radar = W3DRadar::new();
        assert!(legal_radar_point(0, 0));
        assert!(!legal_radar_point(RADAR_CELL_WIDTH as i32, 0));

        let pixel = radar.radar_to_pixel(&ICoord2D::new(0, 0), 10, 20, 128, 128);
        assert_eq!(pixel, ICoord2D::new(10, 147));
    }

    #[test]
    fn reconstruct_view_box_caches_cpp_corner_offsets() {
        let mut radar = W3DRadar::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(128.0, 128.0, 10.0),
            &[],
        );

        assert!(radar.should_reconstruct_view_box());
        assert!(radar.reconstruct_view_box_from_corners(
            [
                Coord3D::new(32.0, 32.0, 0.0),
                Coord3D::new(96.0, 32.0, 0.0),
                Coord3D::new(96.0, 96.0, 0.0),
                Coord3D::new(32.0, 96.0, 0.0),
            ],
            1.25,
            0.5,
        ));

        assert!(!radar.should_reconstruct_view_box());
        assert_eq!(
            radar.view_box(),
            &[
                ICoord2D::new(0, 0),
                ICoord2D::new(64, 0),
                ICoord2D::new(0, 64),
                ICoord2D::new(-64, 0),
            ]
        );

        assert!(!radar.update_view_box_camera_state(1.25, 0.5));
        assert!(radar.update_view_box_camera_state(1.25, 0.75));
        assert!(radar.should_reconstruct_view_box());
    }
}

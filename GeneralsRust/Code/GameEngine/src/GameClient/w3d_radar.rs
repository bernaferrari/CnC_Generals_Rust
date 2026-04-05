// FILE: w3d_radar.rs
// Author: Colin Day, January 2002 (C++ original)
// Rust port: 2025
// Desc: W3D radar implementation with device-specific drawing
//
// Ported from: /GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/Common/System/W3DRadar.cpp
//              /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/Common/W3DRadar.h

use super::radar::*;
use super::types::{Coord3D, ICoord2D};
use std::f32::consts::PI;

// CONSTANTS /////////////////////////////////////////////////////////////////////////////////////

/// Overlay texture refresh rate (every N frames)
/// Matches OVERLAY_REFRESH_RATE from W3DRadar.cpp line 40
const OVERLAY_REFRESH_RATE: Int = 6;

// TEXTURE FORMATS ///////////////////////////////////////////////////////////////////////////////

/// W3D texture format enumeration
/// Matches WW3DFormat from WW3DFormat.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum WW3DFormat {
    Unknown = 0,
    R8G8B8,
    X8R8G8B8,
    R5G6B5,
    X1R5G5B5,
    A8R8G8B8,
    A4R4G4B4,
}

/// MIP level constants
pub const MIP_LEVELS_1: Int = 1;

// TEXTURE AND IMAGE ABSTRACTIONS ////////////////////////////////////////////////////////////////

/// Abstract texture class (placeholder for actual W3D texture)
/// Matches C++ TextureClass from Texture.h
pub struct TextureClass {
    width: Int,
    height: Int,
    format: WW3DFormat,
    pixels: Vec<u32>,
}

impl TextureClass {
    /// Create a new texture
    pub fn new(width: Int, height: Int, format: WW3DFormat, _mip_levels: Int) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            format,
            pixels: vec![0; size],
        }
    }

    /// Get surface for rendering
    pub fn get_surface_level(&mut self) -> SurfaceClass {
        SurfaceClass {
            width: self.width,
            height: self.height,
            pixels: &mut self.pixels,
        }
    }

    /// Release reference (no-op in Rust with RAII)
    pub fn release_ref(&mut self) {
        // No-op, handled by Drop
    }
}

/// Abstract surface class (placeholder for actual rendering surface)
/// Matches C++ SurfaceClass
pub struct SurfaceClass<'a> {
    width: Int,
    height: Int,
    pixels: &'a mut Vec<u32>,
}

impl<'a> SurfaceClass<'a> {
    /// Clear the surface
    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    /// Draw a single pixel
    pub fn draw_pixel(&mut self, x: Int, y: Int, color: Color) {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            let idx = (y * self.width + x) as usize;
            if idx < self.pixels.len() {
                self.pixels[idx] = color;
            }
        }
    }

    /// Draw a horizontal line
    pub fn draw_h_line(&mut self, y: Int, x_start: Int, x_end: Int, color: Color) {
        if y >= 0 && y < self.height {
            let start = x_start.max(0) as usize;
            let end = (x_end.min(self.width - 1) + 1) as usize;
            let row_start = (y * self.width) as usize;
            for x in start..end {
                let idx = row_start + x;
                if idx < self.pixels.len() {
                    self.pixels[idx] = color;
                }
            }
        }
    }
}

/// Image abstraction (placeholder for actual image)
/// Matches C++ Image class
pub struct Image {
    width: Int,
    height: Int,
    texture: Option<Box<TextureClass>>,
}

impl Image {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            texture: None,
        }
    }

    pub fn set_texture(&mut self, texture: TextureClass) {
        self.texture = Some(Box::new(texture));
    }

    pub fn set_size(&mut self, width: Int, height: Int) {
        self.width = width;
        self.height = height;
    }

    pub fn get_width(&self) -> Int {
        self.width
    }

    pub fn get_height(&self) -> Int {
        self.height
    }
}

// DISPLAY INTERFACE /////////////////////////////////////////////////////////////////////////////

/// Display interface for drawing (placeholder)
/// Matches C++ TheDisplay interface
pub trait DisplayInterface {
    fn draw_image(&self, image: &Image, x1: Int, y1: Int, x2: Int, y2: Int);
    fn draw_fill_rect(&self, x: Int, y: Int, width: Int, height: Int, color: Color);
    fn draw_line(&self, x1: Int, y1: Int, x2: Int, y2: Int, width: Real, color: Color);
    fn draw_line_gradient(
        &self,
        x1: Int,
        y1: Int,
        x2: Int,
        y2: Int,
        width: Real,
        color1: Color,
        color2: Color,
    );
}

// W3D RADAR IMPLEMENTATION //////////////////////////////////////////////////////////////////////

/// W3D-specific radar implementation
/// Matches C++ W3DRadar class from W3DRadar.h
pub struct W3DRadar {
    // Base radar data
    base: Radar,

    // Texture formats
    terrain_texture_format: WW3DFormat,
    overlay_texture_format: WW3DFormat,
    shroud_texture_format: WW3DFormat,

    // Textures
    terrain_texture: Option<TextureClass>,
    overlay_texture: Option<TextureClass>,
    shroud_texture: Option<TextureClass>,

    // Images
    terrain_image: Option<Image>,
    overlay_image: Option<Image>,
    shroud_image: Option<Image>,

    // Texture dimensions
    texture_width: Int,
    texture_height: Int,

    // View box state
    reconstruct_view_box: Bool,
    view_angle: Real,
    view_zoom: Real,
    view_box: [ICoord2D; 4],

    // Cached hero positions for drawing icons
    cached_hero_pos_list: Vec<Coord3D>,

    // Frame counter
    frame_counter: UnsignedInt,
}

impl W3DRadar {
    /// Create a new W3D radar
    /// Matches C++ W3DRadar::W3DRadar() from W3DRadar.cpp line 799
    pub fn new() -> Self {
        Self {
            base: Radar::new(),
            terrain_texture_format: WW3DFormat::Unknown,
            overlay_texture_format: WW3DFormat::Unknown,
            shroud_texture_format: WW3DFormat::Unknown,
            terrain_texture: None,
            overlay_texture: None,
            shroud_texture: None,
            terrain_image: None,
            overlay_image: None,
            shroud_image: None,
            texture_width: RADAR_CELL_WIDTH,
            texture_height: RADAR_CELL_HEIGHT,
            reconstruct_view_box: true,
            view_angle: 0.0,
            view_zoom: 0.0,
            view_box: [ICoord2D::new(0, 0); 4],
            cached_hero_pos_list: Vec::new(),
            frame_counter: 0,
        }
    }

    /// Initialize texture formats
    /// Matches C++ W3DRadar::initializeTextureFormats() from W3DRadar.cpp line 79
    fn initialize_texture_formats(&mut self) {
        // In a real implementation, would check hardware capabilities
        // For now, use the most capable formats
        self.terrain_texture_format = WW3DFormat::X8R8G8B8;
        self.overlay_texture_format = WW3DFormat::A8R8G8B8;
        self.shroud_texture_format = WW3DFormat::A8R8G8B8;
    }

    /// Delete rendering resources
    /// Matches C++ W3DRadar::deleteResources() from W3DRadar.cpp line 116
    fn delete_resources(&mut self) {
        self.terrain_texture = None;
        self.terrain_image = None;
        self.overlay_texture = None;
        self.overlay_image = None;
        self.shroud_texture = None;
        self.shroud_image = None;
    }

    /// Reconstruct the view box
    /// Matches C++ W3DRadar::reconstructViewBox() from W3DRadar.cpp line 154
    fn reconstruct_view_box(&mut self) {
        // In full implementation, would get camera corners from tactical view
        // For now, use a simple fixed box
        self.view_box[0] = ICoord2D::new(0, 0);
        self.view_box[1] = ICoord2D::new(10, 0);
        self.view_box[2] = ICoord2D::new(0, 10);
        self.view_box[3] = ICoord2D::new(-10, 0);

        self.reconstruct_view_box = false;
    }

    /// Convert radar coordinates to pixel coordinates
    /// Matches C++ W3DRadar::radarToPixel() from W3DRadar.cpp line 211
    fn radar_to_pixel(
        &self,
        radar: &ICoord2D,
        radar_ul_x: Int,
        radar_ul_y: Int,
        radar_width: Int,
        radar_height: Int,
    ) -> ICoord2D {
        ICoord2D::new(
            (radar.x * radar_width / RADAR_CELL_WIDTH) + radar_ul_x,
            // Note: inverted Y to match world orientation (+x=right, -y=down)
            ((RADAR_CELL_HEIGHT - 1 - radar.y) * radar_height / RADAR_CELL_HEIGHT) + radar_ul_y,
        )
    }

    /// Draw a hero icon at a position
    /// Matches C++ W3DRadar::drawHeroIcon() from W3DRadar.cpp line 230
    fn draw_hero_icon(
        &self,
        display: &dyn DisplayInterface,
        pixel_x: Int,
        pixel_y: Int,
        width: Int,
        height: Int,
        _pos: &Coord3D,
    ) {
        // Matches C++ W3DRadar::drawHeroIcon() — draws hero reticle image
        // PARITY_NOTE: C++ loads s_heroReticleImage and draws it centered
        // at (pixelX, pixelY) with dimensions (width, height).
        // TheDisplay->drawImage(s_heroReticleImage, pixel_x, pixel_y, pixel_x + width, pixel_y + height);
        let _ = (display, pixel_x, pixel_y, width, height);
    }

    /// Draw the view box
    /// Matches C++ W3DRadar::drawViewBox() from W3DRadar.cpp line 260
    fn draw_view_box(
        &self,
        display: &dyn DisplayInterface,
        pixel_x: Int,
        pixel_y: Int,
        width: Int,
        height: Int,
    ) {
        // Get camera position and convert to radar
        // In full implementation, would get from tactical view
        let ul_radar = ICoord2D::new(32, 32); // Placeholder

        // Convert to pixel coords
        let ul_start = self.radar_to_pixel(&ul_radar, pixel_x, pixel_y, width, height);

        // Draw the box using view box offsets
        let mut points = [ICoord2D::new(0, 0); 4];
        points[0] = ul_start;

        for i in 1..4 {
            let radar = ICoord2D::new(
                ul_radar.x + self.view_box[i].x,
                ul_radar.y + self.view_box[i].y,
            );
            points[i] = self.radar_to_pixel(&radar, pixel_x, pixel_y, width, height);
        }

        // Draw lines between points
        let top_color = game_make_color(225, 225, 0, 255);
        let bottom_color = game_make_color(158, 158, 0, 255);

        display.draw_line(
            points[0].x,
            points[0].y,
            points[1].x,
            points[1].y,
            1.0,
            top_color,
        );
        display.draw_line_gradient(
            points[1].x,
            points[1].y,
            points[2].x,
            points[2].y,
            1.0,
            top_color,
            bottom_color,
        );
        display.draw_line(
            points[2].x,
            points[2].y,
            points[3].x,
            points[3].y,
            1.0,
            bottom_color,
        );
        display.draw_line_gradient(
            points[3].x,
            points[3].y,
            points[0].x,
            points[0].y,
            1.0,
            bottom_color,
            top_color,
        );
    }

    /// Draw a single beacon event
    /// Matches C++ W3DRadar::drawSingleBeaconEvent() from W3DRadar.cpp line 347
    fn draw_single_beacon_event(
        &self,
        display: &dyn DisplayInterface,
        pixel_x: Int,
        pixel_y: Int,
        width: Int,
        height: Int,
        event: &RadarEvent,
        current_frame: UnsignedInt,
    ) {
        const TIME_FROM_FULL_SIZE_TO_SMALL_SIZE: Real = LOGICFRAMES_PER_SECOND * 1.5;
        const TOTAL_ANGLES_TO_SPIN: Real = 2.0 * PI;

        let frame_diff = current_frame - event.create_frame;
        let max_event_size = int_to_real(width) / 10.0;
        let min_event_size = 6;

        // Compute size (shrinks over time)
        let mut event_size = real_to_int(
            max_event_size * (1.0 - int_to_real(frame_diff) / TIME_FROM_FULL_SIZE_TO_SMALL_SIZE),
        );
        if event_size < min_event_size {
            event_size = min_event_size;
        }

        // Compute rotation
        let add_angle =
            -TOTAL_ANGLES_TO_SPIN * (int_to_real(frame_diff) / TIME_FROM_FULL_SIZE_TO_SMALL_SIZE);

        // Create triangle points around the event
        let mut tri = [ICoord2D::new(0, 0); 3];

        for i in 0..3 {
            let angle = (i as Real) * 2.0 * PI / 3.0 - add_angle;
            let offset_x = (angle.cos() * int_to_real(event_size)) as Int;
            let offset_y = (angle.sin() * int_to_real(event_size)) as Int;
            let radar_pt =
                ICoord2D::new(event.radar_loc.x + offset_x, event.radar_loc.y + offset_y);
            tri[i] = self.radar_to_pixel(&radar_pt, pixel_x, pixel_y, width, height);
        }

        // Calculate fade alpha
        let mut alpha = event.color1.alpha;
        if current_frame > event.fade_frame {
            let fade_progress = int_to_real(current_frame - event.fade_frame)
                / int_to_real(event.die_frame - event.fade_frame);
            alpha = real_to_unsignedbyte((alpha as Real) * (1.0 - fade_progress));
        }

        // Draw the triangle
        let color = game_make_color(
            event.color1.red as UnsignedByte,
            event.color1.green as UnsignedByte,
            event.color1.blue as UnsignedByte,
            alpha,
        );

        display.draw_line(tri[0].x, tri[0].y, tri[1].x, tri[1].y, 1.0, color);
        display.draw_line(tri[1].x, tri[1].y, tri[2].x, tri[2].y, 1.0, color);
        display.draw_line(tri[2].x, tri[2].y, tri[0].x, tri[0].y, 1.0, color);
    }

    /// Draw a single generic event
    /// Matches C++ W3DRadar::drawSingleGenericEvent() from W3DRadar.cpp line 446
    fn draw_single_generic_event(
        &self,
        display: &dyn DisplayInterface,
        pixel_x: Int,
        pixel_y: Int,
        width: Int,
        height: Int,
        event: &RadarEvent,
        current_frame: UnsignedInt,
    ) {
        const TIME_FROM_FULL_SIZE_TO_SMALL_SIZE: Real = LOGICFRAMES_PER_SECOND * 1.5;
        const TOTAL_ANGLES_TO_SPIN: Real = 2.0 * PI;

        let frame_diff = current_frame - event.create_frame;
        let max_event_size = int_to_real(width) / 2.0;
        let min_event_size = 6;

        // Compute size (shrinks over time)
        let mut event_size = real_to_int(
            max_event_size * (1.0 - int_to_real(frame_diff) / TIME_FROM_FULL_SIZE_TO_SMALL_SIZE),
        );
        if event_size < min_event_size {
            event_size = min_event_size;
        }

        // Compute rotation (opposite direction from beacon)
        let add_angle =
            TOTAL_ANGLES_TO_SPIN * (int_to_real(frame_diff) / TIME_FROM_FULL_SIZE_TO_SMALL_SIZE);

        // Create triangle points
        let mut tri = [ICoord2D::new(0, 0); 3];

        for i in 0..3 {
            let angle = (i as Real) * 2.0 * PI / 3.0 - add_angle;
            let offset_x = (angle.cos() * int_to_real(event_size)) as Int;
            let offset_y = (angle.sin() * int_to_real(event_size)) as Int;
            let radar_pt =
                ICoord2D::new(event.radar_loc.x + offset_x, event.radar_loc.y + offset_y);
            tri[i] = self.radar_to_pixel(&radar_pt, pixel_x, pixel_y, width, height);
        }

        // Calculate fade alpha
        let mut alpha = event.color1.alpha;
        if current_frame > event.fade_frame {
            let fade_progress = int_to_real(current_frame - event.fade_frame)
                / int_to_real(event.die_frame - event.fade_frame);
            alpha = real_to_unsignedbyte((alpha as Real) * (1.0 - fade_progress));
        }

        // Draw the triangle
        let color = game_make_color(
            event.color1.red as UnsignedByte,
            event.color1.green as UnsignedByte,
            event.color1.blue as UnsignedByte,
            alpha,
        );

        display.draw_line(tri[0].x, tri[0].y, tri[1].x, tri[1].y, 1.0, color);
        display.draw_line(tri[1].x, tri[1].y, tri[2].x, tri[2].y, 1.0, color);
        display.draw_line(tri[2].x, tri[2].y, tri[0].x, tri[0].y, 1.0, color);
    }

    /// Draw all radar events
    /// Matches C++ W3DRadar::drawEvents() from W3DRadar.cpp line 546
    fn draw_events(
        &self,
        display: &dyn DisplayInterface,
        pixel_x: Int,
        pixel_y: Int,
        width: Int,
        height: Int,
        current_frame: UnsignedInt,
    ) {
        // Get events from base radar
        // In full implementation, would access base.events
        // For now, placeholder
    }

    /// Draw all radar icons
    /// Matches C++ W3DRadar::drawIcons() from W3DRadar.cpp line 582
    fn draw_icons(
        &self,
        display: &dyn DisplayInterface,
        pixel_x: Int,
        pixel_y: Int,
        width: Int,
        height: Int,
    ) {
        for pos in &self.cached_hero_pos_list {
            self.draw_hero_icon(display, pixel_x, pixel_y, width, height, pos);
        }
    }

    /// Render an object list to a texture
    /// Matches C++ W3DRadar::renderObjectList() from W3DRadar.cpp line 596
    fn render_object_list(
        &mut self,
        list_head: &Option<Box<RadarObject>>,
        texture: &mut TextureClass,
        calc_hero: Bool,
    ) {
        if calc_hero {
            self.cached_hero_pos_list.clear();
        }

        let mut surface = texture.get_surface_level();
        let mut current = list_head.as_ref();

        while let Some(radar_obj) = current {
            let color = radar_obj.get_color();

            // In full implementation, would:
            // 1. Get object from object_id
            // 2. Check visibility, shroud status, stealth
            // 3. Get object position and convert to radar
            // 4. Draw 2x2 pixel blip

            // For now, simplified
            // Draw a 2x2 blip at arbitrary position for demonstration
            let radar_point = ICoord2D::new(64, 64); // Placeholder

            // Draw 2x2 blip
            surface.draw_pixel(radar_point.x, radar_point.y, color);
            surface.draw_pixel(radar_point.x + 1, radar_point.y, color);
            surface.draw_pixel(radar_point.x, radar_point.y + 1, color);
            surface.draw_pixel(radar_point.x + 1, radar_point.y + 1, color);

            current = radar_obj.get_next();
        }
    }

    /// Interpolate color based on height
    /// Matches C++ W3DRadar::interpolateColorForHeight() from W3DRadar.cpp line 726
    fn interpolate_color_for_height(
        color: &mut RGBColor,
        height: Real,
        hi_z: Real,
        mid_z: Real,
        lo_z: Real,
    ) {
        const HOW_BRIGHT: Real = 0.95;
        const HOW_DARK: Real = 0.60;

        // Sanity checks on map height
        let mut hi_z = hi_z;
        let mut mid_z = mid_z;
        let mut lo_z = lo_z;

        if hi_z == mid_z {
            hi_z = mid_z + 0.1;
        }
        if mid_z == lo_z {
            lo_z = mid_z - 0.1;
        }
        if hi_z == lo_z {
            hi_z = lo_z + 0.2;
        }

        let (t, target_color) = if height >= mid_z {
            // Above middle - interpolate lighter
            let t = (height - mid_z) / (hi_z - mid_z);
            let target = RGBColor::new(
                color.red + (1.0 - color.red) * HOW_BRIGHT,
                color.green + (1.0 - color.green) * HOW_BRIGHT,
                color.blue + (1.0 - color.blue) * HOW_BRIGHT,
            );
            (t, target)
        } else {
            // Below middle - interpolate darker
            let t = (mid_z - height) / (mid_z - lo_z);
            let target = RGBColor::new(
                color.red * (1.0 - HOW_DARK),
                color.green * (1.0 - HOW_DARK),
                color.blue * (1.0 - HOW_DARK),
            );
            (t, target)
        };

        // Interpolate toward target
        color.red = color.red + (target_color.red - color.red) * t;
        color.green = color.green + (target_color.green - color.green) * t;
        color.blue = color.blue + (target_color.blue - color.blue) * t;

        // Clamp to valid range
        color.red = color.red.clamp(0.0, 1.0);
        color.green = color.green.clamp(0.0, 1.0);
        color.blue = color.blue.clamp(0.0, 1.0);
    }

    /// Build the terrain texture
    /// Matches C++ W3DRadar::buildTerrainTexture() from W3DRadar.cpp line 997
    fn build_terrain_texture(&mut self, terrain: &dyn TerrainLogicInterface) {
        self.reconstruct_view_box = true;

        // In full implementation, would sample terrain colors and heights
        // Build a colored representation of the map

        if let Some(ref mut texture) = self.terrain_texture {
            let mut surface = texture.get_surface_level();

            // Simplified: fill with a base color
            // Full implementation would sample terrain at each radar cell
            for y in 0..self.texture_height {
                for x in 0..self.texture_width {
                    // Sample terrain (simplified)
                    let radar_pt = ICoord2D::new(x, y);
                    if let Some(world) = self.base.radar_to_world_2d(&radar_pt) {
                        // Get terrain height
                        let height = terrain.get_ground_height(world.x, world.y);

                        // Create base color (simplified)
                        let mut color = RGBColor::new(0.4, 0.6, 0.3); // Greenish terrain

                        // Adjust for height
                        let extent = terrain.get_extent();
                        Self::interpolate_color_for_height(
                            &mut color,
                            height,
                            extent.hi.z,
                            self.base.get_terrain_average_z(),
                            extent.lo.z,
                        );

                        // Convert to integer color
                        let pixel_color = game_make_color(
                            real_to_unsignedbyte(color.red * 255.0),
                            real_to_unsignedbyte(color.green * 255.0),
                            real_to_unsignedbyte(color.blue * 255.0),
                            255,
                        );

                        surface.draw_pixel(x, y, pixel_color);
                    }
                }
            }
        }
    }
}

impl RadarInterface for W3DRadar {
    /// Initialize the W3D radar
    /// Matches C++ W3DRadar::init() from W3DRadar.cpp line 843
    fn init(&mut self) {
        self.base.init();
        self.initialize_texture_formats();

        // Allocate textures
        self.terrain_texture = Some(TextureClass::new(
            self.texture_width,
            self.texture_height,
            self.terrain_texture_format,
            MIP_LEVELS_1,
        ));

        self.overlay_texture = Some(TextureClass::new(
            self.texture_width,
            self.texture_height,
            self.overlay_texture_format,
            MIP_LEVELS_1,
        ));

        self.shroud_texture = Some(TextureClass::new(
            self.texture_width,
            self.texture_height,
            self.shroud_texture_format,
            MIP_LEVELS_1,
        ));

        // Create images
        self.terrain_image = Some(Image::new());
        self.overlay_image = Some(Image::new());
        self.shroud_image = Some(Image::new());
    }

    /// Reset the W3D radar
    /// Matches C++ W3DRadar::reset() from W3DRadar.cpp line 934
    fn reset(&mut self) {
        self.base.reset();

        // Clear textures
        if let Some(ref mut texture) = self.terrain_texture {
            let mut surface = texture.get_surface_level();
            surface.clear();
        }

        if let Some(ref mut texture) = self.overlay_texture {
            let mut surface = texture.get_surface_level();
            surface.clear();
        }

        // Clear shroud (sets to transparent, not black)
        self.clear_shroud();
    }

    /// Update the W3D radar
    /// Matches C++ W3DRadar::update() from W3DRadar.cpp line 966
    fn update(&mut self) {
        self.base.update();
        self.frame_counter += 1;
    }

    /// Setup for new map
    /// Matches C++ W3DRadar::newMap() from W3DRadar.cpp line 977
    fn new_map(&mut self, terrain: &dyn TerrainLogicInterface) {
        self.base.new_map(terrain);
        self.build_terrain_texture(terrain);
    }

    // Delegate most methods to base implementation
    fn add_object(&mut self, obj_id: ObjectID, priority: RadarPriorityType, color: Color) {
        self.base.add_object(obj_id, priority, color);
    }

    fn remove_object(&mut self, obj_id: ObjectID) {
        self.base.remove_object(obj_id);
    }

    fn radar_to_world(&self, radar: &ICoord2D) -> Option<Coord3D> {
        self.base.radar_to_world(radar)
    }

    fn radar_to_world_2d(&self, radar: &ICoord2D) -> Option<Coord3D> {
        self.base.radar_to_world_2d(radar)
    }

    fn world_to_radar(&self, world: &Coord3D) -> Option<ICoord2D> {
        self.base.world_to_radar(world)
    }

    fn local_pixel_to_radar(&self, pixel: &ICoord2D) -> Option<ICoord2D> {
        self.base.local_pixel_to_radar(pixel)
    }

    fn screen_pixel_to_world(&self, pixel: &ICoord2D) -> Option<Coord3D> {
        self.base.screen_pixel_to_world(pixel)
    }

    fn object_under_radar_pixel(&self, pixel: &ICoord2D) -> Option<ObjectID> {
        self.base.object_under_radar_pixel(pixel)
    }

    fn find_draw_positions(
        &self,
        start_x: Int,
        start_y: Int,
        width: Int,
        height: Int,
    ) -> (ICoord2D, ICoord2D) {
        self.base
            .find_draw_positions(start_x, start_y, width, height)
    }

    fn is_priority_visible(&self, priority: RadarPriorityType) -> Bool {
        self.base.is_priority_visible(priority)
    }

    fn create_event(&mut self, world: &Coord3D, event_type: RadarEventType, seconds_to_live: Real) {
        self.base.create_event(world, event_type, seconds_to_live);
    }

    fn create_player_event(
        &mut self,
        player_color: Color,
        world: &Coord3D,
        event_type: RadarEventType,
        seconds_to_live: Real,
    ) {
        self.base
            .create_player_event(player_color, world, event_type, seconds_to_live);
    }

    fn get_last_event_loc(&self) -> Option<Coord3D> {
        self.base.get_last_event_loc()
    }

    fn try_under_attack_event(&mut self, world: &Coord3D) {
        self.base.try_under_attack_event(world);
    }

    fn try_infiltration_event(&mut self, world: &Coord3D) {
        self.base.try_infiltration_event(world);
    }

    fn try_event(&mut self, event_type: RadarEventType, pos: &Coord3D) -> Bool {
        self.base.try_event(event_type, pos)
    }

    fn hide(&mut self, hide: Bool) {
        self.base.hide(hide);
    }

    fn is_radar_hidden(&self) -> Bool {
        self.base.is_radar_hidden()
    }

    fn force_on(&mut self, force: Bool) {
        self.base.force_on(force);
    }

    fn is_radar_forced(&self) -> Bool {
        self.base.is_radar_forced()
    }

    /// Refresh terrain display
    /// Matches C++ W3DRadar::refreshTerrain() from W3DRadar.cpp line 1422
    fn refresh_terrain(&mut self, terrain: &dyn TerrainLogicInterface) {
        self.base.refresh_terrain(terrain);
        self.build_terrain_texture(terrain);
    }

    fn queue_terrain_refresh(&mut self) {
        self.base.queue_terrain_refresh();
    }

    /// Clear all shroud
    /// Matches C++ W3DRadar::clearShroud() from W3DRadar.cpp line 1232
    fn clear_shroud(&mut self) {
        if let Some(ref mut texture) = self.shroud_texture {
            let mut surface = texture.get_surface_level();
            let clear_color = game_make_color(0, 0, 0, 0); // Transparent

            for y in 0..self.texture_height {
                surface.draw_h_line(y, 0, self.texture_width - 1, clear_color);
            }
        }
    }

    /// Set shroud level at a cell
    /// Matches C++ W3DRadar::setShroudLevel() from W3DRadar.cpp line 1252
    fn set_shroud_level(&mut self, shroud_x: Int, shroud_y: Int, setting: CellShroudStatus) {
        // In full implementation, would calculate radar coordinates from shroud coordinates
        // For now, simplified direct mapping

        let alpha = match setting {
            CellShroudStatus::Shrouded => 255,
            CellShroudStatus::Fogged => 127,
            CellShroudStatus::Clear => 0,
        };

        if let Some(ref mut texture) = self.shroud_texture {
            let mut surface = texture.get_surface_level();
            let shroud_color = game_make_color(0, 0, 0, alpha);

            // Draw shroud cell (simplified - would calculate actual radar area)
            let radar_min_x = shroud_x;
            let radar_min_y = shroud_y;
            let radar_max_x = shroud_x + 1;
            let radar_max_y = shroud_y + 1;

            for y in radar_min_y..=radar_max_y {
                for x in radar_min_x..=radar_max_x {
                    if x >= 0 && x < RADAR_CELL_WIDTH && y >= 0 && y < RADAR_CELL_HEIGHT {
                        surface.draw_pixel(x, y, shroud_color);
                    }
                }
            }
        }
    }

    /// Draw the radar
    /// Matches C++ W3DRadar::draw() from W3DRadar.cpp line 1326
    fn draw(&mut self, pixel_x: Int, pixel_y: Int, width: Int, height: Int) {
        // Matches C++ W3DRadar::draw() — layered texture composition:
        // terrain -> overlay (every OVERLAY_REFRESH_RATE frames) -> shroud -> icons -> events -> view box

        let (ul, lr) = self
            .base
            .find_draw_positions(pixel_x, pixel_y, width, height);
        let scaled_width = lr.x - ul.x;
        let scaled_height = lr.y - ul.y;

        // Draw letterbox borders (black fill + gray edge lines) when map aspect ratio
        // doesn't match the radar widget. Matches C++ W3DRadar::draw() lines 1360-1420.
        let border_color = game_make_color(0, 0, 0, 255);
        let edge_color = game_make_color(64, 64, 64, 255);

        if scaled_width < width {
            let side = (width - scaled_width) / 2;
            // PARITY_NOTE: TheDisplay->drawFillRect / drawLine calls are wired once
            // the Display trait is connected to the actual rendering backend.
            let _ = (side, border_color, edge_color);
        }
        if scaled_height < height {
            let side = (height - scaled_height) / 2;
            let _ = (side, border_color, edge_color);
        }

        // Terrain texture — drawn every frame
        // TheDisplay->drawImage(m_terrainImage, ul.x, ul.y, lr.x, lr.y);
        if let Some(ref img) = self.terrain_image {
            if img.texture.is_some() {
                let _ = (ul.x, ul.y, lr.x, lr.y);
            }
        }

        // Overlay texture — refreshed every OVERLAY_REFRESH_RATE frames
        // Matches C++ W3DRadar::draw() lines 1430-1448
        if self.frame_counter % OVERLAY_REFRESH_RATE as UnsignedInt == 0 {
            if let Some(ref mut texture) = self.overlay_texture {
                let mut surface = texture.get_surface_level();
                surface.clear();
                self.render_object_list(self.base.get_object_list(), texture, false);
                self.render_object_list(self.base.get_local_object_list(), texture, true);
            }
            if let Some(ref mut img) = self.overlay_image {
                if let Some(ref mut tex) = self.overlay_texture {
                    img.set_size(tex.width, tex.height);
                }
            }
        }

        // TheDisplay->drawImage(m_overlayImage, ul.x, ul.y, lr.x, lr.y);

        // Shroud texture
        // TheDisplay->drawImage(m_shroudImage, ul.x, ul.y, lr.x, lr.y);
        if let Some(ref img) = self.shroud_image {
            if img.texture.is_some() {
                let _ = (ul.x, ul.y, lr.x, lr.y);
            }
        }

        // Icons — hero reticle images at cached positions
        // Matches C++ W3DRadar::drawIcons() line 582
        // drawIcons(display, ul.x, ul.y, scaled_width, scaled_height);

        // Events — beacon/generic radar event triangles with fade-out
        // Matches C++ W3DRadar::drawEvents() line 546
        // drawEvents(display, ul.x, ul.y, scaled_width, scaled_height, current_frame);

        // View box — camera viewport as yellow trapezoid
        // Matches C++ W3DRadar::drawViewBox() line 260
        if self.reconstruct_view_box {
            self.reconstruct_view_box();
        }
        // drawViewBox(display, ul.x, ul.y, scaled_width, scaled_height);
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

    struct MockTerrain {
        extent: Region3D,
    }

    impl TerrainLogicInterface for MockTerrain {
        fn get_extent(&self) -> Region3D {
            self.extent
        }

        fn get_ground_height(&self, _x: Real, _y: Real) -> Real {
            10.0
        }

        fn is_underwater(&self, _x: Real, _y: Real) -> (Bool, Real) {
            (false, 10.0)
        }
    }

    #[test]
    fn test_w3d_radar_creation() {
        let radar = W3DRadar::new();
        assert_eq!(radar.texture_width, RADAR_CELL_WIDTH);
        assert_eq!(radar.texture_height, RADAR_CELL_HEIGHT);
    }

    #[test]
    fn test_texture_creation() {
        let texture = TextureClass::new(128, 128, WW3DFormat::X8R8G8B8, MIP_LEVELS_1);
        assert_eq!(texture.width, 128);
        assert_eq!(texture.height, 128);
    }

    #[test]
    fn test_color_interpolation() {
        let mut color = RGBColor::new(0.5, 0.5, 0.5);
        W3DRadar::interpolate_color_for_height(&mut color, 50.0, 100.0, 50.0, 0.0);
        // Color should still be around 0.5 since height = mid_z
        assert!((color.red - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_radar_to_pixel_conversion() {
        let radar = W3DRadar::new();
        let radar_pos = ICoord2D::new(64, 64);
        let pixel = radar.radar_to_pixel(&radar_pos, 0, 0, 256, 256);
        // Should be approximately at center
        assert!((pixel.x - 128).abs() < 5);
        assert!((pixel.y - 128).abs() < 5);
    }
}

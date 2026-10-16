//! Integer View rectangle and Display resize arithmetic (View.cpp, Display.cpp).
//! This value belongs to SessionCamera. Renderers receive immutable copies.
use glam::{Mat4, Vec2, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TacticalViewport {
    display_width: u32,
    display_height: u32,
    width: i32,
    height: i32,
    origin_x: i32,
    origin_y: i32,
}
impl Default for TacticalViewport {
    fn default() -> Self {
        Self {
            display_width: 640,
            display_height: 480,
            width: 640,
            height: 480,
            origin_x: 0,
            origin_y: 0,
        }
    }
}
impl TacticalViewport {
    pub(crate) fn for_display(width: u32, height: u32) -> Result<Self, &'static str> {
        Self::check_display(width, height)?;
        Ok(Self {
            display_width: width,
            display_height: height,
            width: width as i32,
            height: height as i32,
            origin_x: 0,
            origin_y: 0,
        })
    }
    fn check_display(width: u32, height: u32) -> Result<(), &'static str> {
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            Err("invalid tactical display extent")
        } else {
            Ok(())
        }
    }
    /// Original order: float division, float multiplication, integer conversion.
    /// Retain truncation from earlier UI operations, including through resizes.
    pub(crate) fn resize_display(&mut self, width: u32, height: u32) -> Result<(), &'static str> {
        Self::check_display(width, height)?;
        let scale =
            |value: i32, old: u32, new: u32| (value as f32 / old as f32 * new as f32) as i32;
        let next = Self {
            display_width: width,
            display_height: height,
            width: scale(self.width, self.display_width, width),
            height: scale(self.height, self.display_height, height),
            origin_x: scale(self.origin_x, self.display_width, width),
            origin_y: scale(self.origin_y, self.display_height, height),
        };
        if next.width <= 0
            || next.height <= 0
            || next.origin_x.checked_add(next.width).is_none()
            || next.origin_y.checked_add(next.height).is_none()
        {
            return Err("resize would empty tactical view");
        }
        *self = next;
        Ok(())
    }
    pub(crate) fn set_height_px(&mut self, height: i32) -> Result<(), &'static str> {
        if height <= 0 || self.origin_y.checked_add(height).is_none() {
            return Err("invalid tactical view height");
        }
        self.height = height;
        Ok(())
    }
    pub(crate) fn set_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<(), &'static str> {
        if width <= 0
            || height <= 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err("invalid tactical view rectangle");
        }
        self.origin_x = x;
        self.origin_y = y;
        self.width = width;
        self.height = height;
        Ok(())
    }
    /// Outbound observation for unmigrated presentation consumers. Native
    /// geometry never reads this copy back; it is not another camera owner.
    #[cfg(feature = "game_client")]
    pub(crate) fn observe_in_core_view(self, view: &mut game_client::display::view::View) {
        view.set_width(self.width);
        view.set_height(self.height);
        view.set_origin(self.origin_x, self.origin_y);
    }

    pub(crate) fn size(self) -> (i32, i32) {
        (self.width, self.height)
    }
    pub(crate) fn origin(self) -> (i32, i32) {
        (self.origin_x, self.origin_y)
    }
    pub(crate) fn display_size(self) -> (u32, u32) {
        (self.display_width, self.display_height)
    }
    pub(crate) fn pixel_rect(self) -> (f32, f32, f32, f32) {
        (
            self.origin_x as f32,
            self.origin_y as f32,
            self.width as f32,
            self.height as f32,
        )
    }
    pub(crate) fn normalized_bounds(self) -> (Vec2, Vec2) {
        (
            Vec2::new(
                self.origin_x as f32 / self.display_width as f32,
                self.origin_y as f32 / self.display_height as f32,
            ),
            Vec2::new(
                ((self.origin_x + self.width) as f32) / self.display_width as f32,
                ((self.origin_y + self.height) as f32) / self.display_height as f32,
            ),
        )
    }
    pub(crate) fn ndc(self, x: f32, y: f32) -> Vec2 {
        // PixelScreen_To_W3DLogicalScreen multiplies before dividing and
        // negates the complete Y expression (including its signed zero).
        let x = x - self.origin_x as f32;
        let y = y - self.origin_y as f32;
        Vec2::new(
            (2. * x) / self.width as f32 - 1.,
            -((2. * y) / self.height as f32 - 1.),
        )
    }
    pub(crate) fn constraint_pixels(self) -> [(i32, i32); 2] {
        let x = (0.5 * self.width as f32 + self.origin_x as f32) as i32;
        [
            (x, (0.5 * self.height as f32 + self.origin_y as f32) as i32),
            (x, (self.origin_y as f32 + 0.95 * self.height as f32) as i32),
        ]
    }
    pub(crate) fn pick_ray(
        self,
        view: Mat4,
        projection: Mat4,
        x: f32,
        y: f32,
    ) -> Option<(Vec3, Vec3)> {
        Self::ray_at_ndc(view, projection, self.ndc(x, y))
    }
    /// W3DView::lookAt uses Un_Project(0, 0), independently of integer picks.
    /// Keep the actual unprojection shared with pixel-based constraint rays.
    pub(crate) fn ray_at_ndc(view: Mat4, projection: Mat4, ndc: Vec2) -> Option<(Vec3, Vec3)> {
        let inverse = (projection * view).inverse();
        let point = |depth| {
            let p = inverse * Vec4::new(ndc.x, ndc.y, depth, 1.);
            (p.w.abs() > 1e-7).then(|| p.truncate() / p.w)
        };
        let a = point(0.)?;
        let b = point(1.)?;
        (a.is_finite() && b.is_finite()).then_some((a, b))
    }
    pub(crate) fn constraint_inset(self, view: Mat4, projection: Mat4, ground: f32) -> f32 {
        let hit = |(x, y): (i32, i32)| {
            let (a, b) = self.pick_ray(view, projection, x as f32, y as f32)?;
            let dy = b.y - a.y;
            if dy.abs() <= 1e-7 {
                return None;
            }
            let t = (ground - a.y) / dy;
            t.is_finite().then_some(a + (b - a) * t)
        };
        let [center, bottom] = self.constraint_pixels();
        hit(center).zip(hit(bottom)).map_or(0., |(a, b)| {
            // W3DView.cpp:490-502 subtracts center X/Y only. Its source Z
            // remains maxEdgeZ, so Coord3D::length includes ground squared.
            let x = a.x - b.x;
            let y = a.z - b.z;
            f64::from(x * x + y * y + ground * ground).sqrt() as f32
        })
    }
}

impl super::SessionCamera {
    pub(crate) fn resize_display(&mut self, width: u32, height: u32) -> Result<(), &'static str> {
        self.viewport.resize_display(width, height)?;
        self.rebuild_projection();
        Ok(())
    }
    pub(crate) fn set_view_height_px(&mut self, height: i32) -> Result<(), &'static str> {
        self.viewport.set_height_px(height)?;
        self.rebuild_projection();
        Ok(())
    }
    pub(crate) fn projection_for_clip_planes(&self, near: f32, far: f32) -> glam::Mat4 {
        let (width, height) = self.viewport.size();
        let (display_width, _) = self.viewport.display_size();
        super::projection::perspective_rh_from_horizontal_fov(
            width as f32 / display_width as f32 * super::projection::DEFAULT_VIEW_FOV_RADIANS,
            width as f32 / height as f32,
            near,
            far,
        )
    }
    pub(crate) fn rebuild_projection(&mut self) {
        // View rectangle changes the x/y lens only. Preserve the installed
        // depth coefficients bit-for-bit, including a restored historical lens.
        let mut projection = self.projection_for_clip_planes(
            super::projection::DEFAULT_VIEW_NEAR_CLIP,
            super::projection::DEFAULT_VIEW_FAR_CLIP,
        );
        if self.projection_matrix.is_finite()
            && self.projection_matrix.z_axis.w == -1.
            && self.projection_matrix.w_axis.w == 0.
        {
            projection.z_axis = self.projection_matrix.z_axis;
            projection.w_axis = self.projection_matrix.w_axis;
        }
        self.projection_matrix = projection;
    }
}

#[cfg(feature = "game_client")]
impl game_client::gui::callbacks::control_bar_callbacks::ControlBarViewport
    for super::SessionCamera
{
    fn display_size(&self) -> (u32, u32) {
        self.viewport.display_size()
    }
    fn set_height_px(&mut self, height: i32) {
        if let Err(error) = self.set_view_height_px(height) {
            log::warn!("Rejected empty or overflowing ControlBar viewport: {error}");
        }
    }
}
#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;

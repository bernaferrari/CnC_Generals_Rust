//! C++ parity port of `W3DDynamicLight`.

/// Three-component light color/vector value used by legacy W3D lights.
pub type Vector3 = [f32; 3];

/// Legacy W3D light kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightKind {
    /// Point light, matching `LightClass(LightClass::POINT)`.
    Point,
}

/// Dynamic point light with C++ frame fade behavior.
#[derive(Debug, Clone)]
pub struct W3DDynamicLight {
    /// Light kind inherited from the C++ `LightClass` base.
    pub kind: LightKind,
    /// Far attenuation start distance.
    pub far_atten_start: f32,
    /// Far attenuation end distance.
    pub far_atten_end: f32,
    /// Ambient light color.
    pub ambient: Vector3,
    /// Diffuse light color.
    pub diffuse: Vector3,
    prior_enable: bool,
    process_me: bool,
    prev_min_x: i32,
    prev_min_y: i32,
    prev_max_x: i32,
    prev_max_y: i32,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    enabled: bool,
    decay_range: bool,
    decay_color: bool,
    cur_decay_frame_count: u32,
    cur_increase_frame_count: u32,
    decay_frame_count: u32,
    increase_frame_count: u32,
    target_range: f32,
    target_ambient: Vector3,
    target_diffuse: Vector3,
}

impl W3DDynamicLight {
    /// Create a point dynamic light, matching the C++ constructor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kind: LightKind::Point,
            far_atten_start: 0.0,
            far_atten_end: 1.0,
            ambient: [0.0; 3],
            diffuse: [0.0; 3],
            prior_enable: false,
            process_me: false,
            prev_min_x: 0,
            prev_min_y: 0,
            prev_max_x: 0,
            prev_max_y: 0,
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
            enabled: true,
            decay_range: false,
            decay_color: false,
            cur_decay_frame_count: 0,
            cur_increase_frame_count: 0,
            decay_frame_count: 0,
            increase_frame_count: 0,
            target_range: 1.0,
            target_ambient: [0.0; 3],
            target_diffuse: [0.0; 3],
        }
    }

    /// Update fade state for one frame.
    pub fn on_frame_update(&mut self) {
        if !self.enabled {
            return;
        }

        let factor = if self.cur_increase_frame_count > 0 && self.increase_frame_count > 0 {
            self.cur_increase_frame_count -= 1;
            (self.increase_frame_count - self.cur_increase_frame_count) as f32
                / self.increase_frame_count as f32
        } else if self.decay_frame_count == 0 {
            1.0
        } else {
            self.cur_decay_frame_count = self.cur_decay_frame_count.saturating_sub(1);
            if self.cur_decay_frame_count == 0 {
                self.enabled = false;
                return;
            }
            self.cur_decay_frame_count as f32 / self.decay_frame_count as f32
        };

        if self.decay_range {
            self.far_atten_end = factor * self.target_range;
            if self.far_atten_end < self.far_atten_start {
                self.far_atten_end = self.far_atten_start;
            }
        }

        if self.decay_color {
            self.ambient = scale_vec3(self.target_ambient, factor);
            self.diffuse = scale_vec3(self.target_diffuse, factor);
        }
    }

    /// Configure frame fade timing and capture target color/range values.
    pub fn set_frame_fade(&mut self, frame_increase_time: u32, decay_frame_time: u32) {
        self.decay_frame_count = decay_frame_time;
        self.cur_decay_frame_count = decay_frame_time;
        self.cur_increase_frame_count = frame_increase_time;
        self.increase_frame_count = frame_increase_time;
        self.target_ambient = self.ambient;
        self.target_diffuse = self.diffuse;
        self.target_range = self.far_atten_end;
    }

    /// Enable or disable the light and clear fade modes, matching the C++ inline setter.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.decay_range = false;
        self.decay_frame_count = 0;
        self.decay_color = false;
        self.increase_frame_count = 0;
    }

    /// Return whether the light is enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable range decay for subsequent frame updates.
    pub const fn set_decay_range(&mut self) {
        self.decay_range = true;
    }

    /// Enable color decay for subsequent frame updates.
    pub const fn set_decay_color(&mut self) {
        self.decay_color = true;
    }

    /// Set terrain influence bounds used by `cull`.
    pub const fn set_bounds(&mut self, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.min_x = min_x;
        self.min_y = min_y;
        self.max_x = max_x;
        self.max_y = max_y;
    }

    /// Return true if the terrain vertex at `x,y` is outside this light's influence.
    #[must_use]
    pub const fn cull(&self, x: i32, y: i32) -> bool {
        x < self.min_x || y < self.min_y || x > self.max_x || y > self.max_y
    }

    /// Previous terrain update bounds.
    #[must_use]
    pub const fn previous_bounds(&self) -> (i32, i32, i32, i32) {
        (
            self.prev_min_x,
            self.prev_min_y,
            self.prev_max_x,
            self.prev_max_y,
        )
    }

    /// Whether the light was enabled during the previous terrain pass.
    #[must_use]
    pub const fn prior_enable(&self) -> bool {
        self.prior_enable
    }

    /// Whether terrain processing has queued this light.
    #[must_use]
    pub const fn process_me(&self) -> bool {
        self.process_me
    }
}

impl Default for W3DDynamicLight {
    fn default() -> Self {
        Self::new()
    }
}

fn scale_vec3(value: Vector3, factor: f32) -> Vector3 {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

#[cfg(test)]
mod tests {
    use super::{LightKind, W3DDynamicLight};

    #[test]
    fn constructor_matches_cpp_defaults_that_are_set_locally() {
        let light = W3DDynamicLight::new();

        assert_eq!(light.kind, LightKind::Point);
        assert!(light.is_enabled());
        assert!(!light.prior_enable());
        assert!(!light.process_me());
    }

    #[test]
    fn frame_fade_increases_before_decay() {
        let mut light = W3DDynamicLight::new();
        light.far_atten_end = 30.0;
        light.ambient = [0.6, 0.3, 0.15];
        light.diffuse = [1.0, 0.5, 0.25];
        light.set_frame_fade(3, 5);
        light.set_decay_range();
        light.set_decay_color();

        light.on_frame_update();

        assert!((light.far_atten_end - 10.0).abs() < f32::EPSILON);
        assert_vec3_close(light.ambient, [0.2, 0.1, 0.05]);
        assert_vec3_close(light.diffuse, [1.0 / 3.0, 1.0 / 6.0, 1.0 / 12.0]);
        assert!(light.is_enabled());
    }

    #[test]
    fn frame_fade_disables_on_final_decay_frame() {
        let mut light = W3DDynamicLight::new();
        light.far_atten_end = 20.0;
        light.set_frame_fade(0, 2);
        light.set_decay_range();

        light.on_frame_update();
        assert!(light.is_enabled());
        assert!((light.far_atten_end - 10.0).abs() < f32::EPSILON);

        light.on_frame_update();
        assert!(!light.is_enabled());
        assert!((light.far_atten_end - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn range_decay_clamps_to_attenuation_start() {
        let mut light = W3DDynamicLight::new();
        light.far_atten_start = 8.0;
        light.far_atten_end = 10.0;
        light.set_frame_fade(0, 4);
        light.set_decay_range();

        light.on_frame_update();

        assert_eq!(light.far_atten_end, 8.0);
    }

    #[test]
    fn cull_matches_cpp_bounds_check() {
        let mut light = W3DDynamicLight::new();
        light.set_bounds(2, 3, 8, 9);

        assert!(!light.cull(2, 3));
        assert!(!light.cull(8, 9));
        assert!(light.cull(1, 3));
        assert!(light.cull(2, 10));
    }

    #[test]
    fn set_enabled_clears_fade_modes() {
        let mut light = W3DDynamicLight::new();
        light.set_frame_fade(2, 5);
        light.set_decay_range();
        light.set_decay_color();
        light.set_enabled(true);

        light.on_frame_update();

        assert!(light.is_enabled());
        assert_eq!(light.far_atten_end, 1.0);
        assert_eq!(light.ambient, [0.0; 3]);
    }

    fn assert_vec3_close(actual: [f32; 3], expected: [f32; 3]) {
        for index in 0..3 {
            assert!(
                (actual[index] - expected[index]).abs() < 0.000_001,
                "component {index}: actual={} expected={}",
                actual[index],
                expected[index]
            );
        }
    }
}

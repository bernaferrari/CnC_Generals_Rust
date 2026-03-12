// FILE: sway_client_update.rs
// Author: Matthew D. Campbell, May 2002
// Desc: Tree sway client update module
// Ported from C++ to Rust

use crate::GameClient::drawable::Drawable;

// Type aliases matching C++ base types
pub type Real = f32;
pub type Bool = bool;
pub type Short = i16;
pub type UnsignedInt = u32;

/// Mathematical constant PI
pub const PI: Real = std::f32::consts::PI;

/// Client update module data - base configuration for modules
pub trait ClientUpdateModuleData {}

/// Base trait for client update modules
pub trait ClientUpdateModule {
    fn client_update(&mut self);
    fn crc(&self, xfer: &mut dyn XferInterface);
    fn xfer(&mut self, xfer: &mut dyn XferInterface);
    fn load_post_process(&mut self);
    fn get_drawable(&mut self) -> Option<&mut Drawable>;
}

/// Xfer interface for serialization
pub trait XferInterface {
    fn xfer_version(&mut self, version: &mut u32, current_version: u32);
    fn xfer_unsigned_int(&mut self, value: &mut UnsignedInt);
    fn xfer_real(&mut self, value: &mut Real);
    fn xfer_bool(&mut self, value: &mut Bool);
    fn xfer_short(&mut self, value: &mut Short);
    fn xfer_user(&mut self, data: &mut [u8]);
}

/// 3D vector structure
/// Matches C++ Coord3D / Vector3
#[derive(Debug, Clone, Copy)]
pub struct Vector3 {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Vector3 {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }
}

/// 3D transformation matrix
/// Matches C++ Matrix3D
#[derive(Debug, Clone, Copy)]
pub struct Matrix3D {
    pub data: [[Real; 4]; 4],
}

impl Matrix3D {
    pub fn new() -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// In-place pre-rotate around X axis
    /// Matches C++ Matrix3D::In_Place_Pre_Rotate_X
    pub fn in_place_pre_rotate_x(&mut self, angle: Real) {
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        for i in 0..4 {
            let y = self.data[i][1];
            let z = self.data[i][2];
            self.data[i][1] = y * cos_angle - z * sin_angle;
            self.data[i][2] = y * sin_angle + z * cos_angle;
        }
    }

    /// In-place pre-rotate around Y axis
    /// Matches C++ Matrix3D::In_Place_Pre_Rotate_Y
    pub fn in_place_pre_rotate_y(&mut self, angle: Real) {
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        for i in 0..4 {
            let x = self.data[i][0];
            let z = self.data[i][2];
            self.data[i][0] = x * cos_angle + z * sin_angle;
            self.data[i][2] = -x * sin_angle + z * cos_angle;
        }
    }
}

/// Breeze information structure
/// Matches C++ BreezeInfo from ScriptEngine
#[derive(Debug, Clone)]
pub struct BreezeInfo {
    /// Breeze version number (incremented when parameters change)
    pub breeze_version: Short,

    /// Breeze intensity (max sway angle)
    pub intensity: Real,

    /// Period of breeze oscillation
    pub breeze_period: Real,

    /// Amount of randomness in the sway
    pub randomness: Real,

    /// Lean angle (base offset)
    pub lean: Real,

    /// Direction vector of the breeze
    pub direction_vec: Vector3,
}

impl BreezeInfo {
    pub fn new() -> Self {
        Self {
            breeze_version: 0,
            intensity: 0.0,
            breeze_period: 1.0,
            randomness: 0.0,
            lean: 0.0,
            direction_vec: Vector3::new(1.0, 0.0, 0.0),
        }
    }
}

/// Script engine interface
/// Matches C++ ScriptEngine class
pub trait ScriptEngineInterface {
    fn get_breeze_info(&self) -> &BreezeInfo;
}

/// Object status bits
/// Matches C++ ObjectStatusBits
pub const OBJECT_STATUS_BURNED: u32 = 0x00000001;

/// Object interface
/// Matches C++ Object class
pub trait ObjectInterface {
    fn get_status_bits(&self) -> u32;
}

/// Random value generator for client-side rendering
/// Matches C++ GameClientRandomValueReal
pub fn game_client_random_value_real(min: Real, max: Real) -> Real {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

/// Cosine function wrapper
/// Matches C++ Cos function
#[inline]
pub fn cos(x: Real) -> Real {
    x.cos()
}

/// Sway client update module
/// Handles tree/object swaying in the breeze
/// Matches C++ SwayClientUpdate from SwayClientUpdate.h line 20
pub struct SwayClientUpdate {
    /// Pointer to the drawable this module is attached to
    drawable: Option<*mut Drawable>,

    /// Module configuration data
    module_data: Option<*const dyn ClientUpdateModuleData>,

    /// Current oscillation value (0 to 2*PI)
    /// Matches C++ SwayClientUpdate::m_curValue line 38
    cur_value: Real,

    /// Current sway angle
    /// Matches C++ SwayClientUpdate::m_curAngle line 39
    cur_angle: Real,

    /// Current delta per frame for oscillation
    /// Matches C++ SwayClientUpdate::m_curDelta line 40
    cur_delta: Real,

    /// Current angle limit (max sway)
    /// Matches C++ SwayClientUpdate::m_curAngleLimit line 41
    cur_angle_limit: Real,

    /// Lean angle (base offset from wind)
    /// Matches C++ SwayClientUpdate::m_leanAngle line 42
    lean_angle: Real,

    /// Current breeze version (to detect changes)
    /// Matches C++ SwayClientUpdate::m_curVersion line 43
    cur_version: Short,

    /// Whether this object is currently swaying
    /// Matches C++ SwayClientUpdate::m_swaying line 44
    swaying: Bool,

    /// Unused flag (for alignment/future use)
    /// Matches C++ SwayClientUpdate::m_unused line 45
    unused: Bool,

    /// Reference to script engine for breeze info
    script_engine: Option<*const dyn ScriptEngineInterface>,
}

impl SwayClientUpdate {
    /// Constructor
    /// Matches C++ SwayClientUpdate::SwayClientUpdate
    /// from SwayClientUpdate.cpp line 30
    pub fn new(
        drawable: Option<*mut Drawable>,
        module_data: Option<*const dyn ClientUpdateModuleData>,
        script_engine: Option<*const dyn ScriptEngineInterface>,
    ) -> Self {
        Self {
            drawable,
            module_data,
            cur_delta: 0.0,
            cur_value: 0.0,
            cur_angle: 0.0,
            cur_angle_limit: 0.0,
            lean_angle: 0.0,
            swaying: true,
            unused: false,
            cur_version: -1, // So that we never match the first time
            script_engine,
        }
    }

    /// Stop swaying (called when object burns)
    /// Matches C++ SwayClientUpdate::stopSway line 34
    pub fn stop_sway(&mut self) {
        self.swaying = false;
    }

    /// Update sway parameters based on current breeze info
    /// Matches C++ SwayClientUpdate::updateSway
    /// from SwayClientUpdate.cpp line 57
    fn update_sway(&mut self) {
        // Get breeze info from script engine
        let info = if let Some(se_ptr) = self.script_engine {
            unsafe { (*se_ptr).get_breeze_info() }
        } else {
            // No script engine, can't update
            return;
        };

        // If randomness is 0, set current value to 0
        // Matches C++ lines 60-63
        if info.randomness == 0.0 {
            self.cur_value = 0.0;
            return;
        }

        // Calculate sway parameters with randomness
        // Matches C++ lines 64-68
        let delta = info.randomness * 0.5;
        self.cur_angle_limit = info.intensity * game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.cur_delta = 2.0 * PI / info.breeze_period * game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.lean_angle = info.lean * game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.cur_version = info.breeze_version;
    }
}

impl ClientUpdateModule for SwayClientUpdate {
    /// The client update callback
    /// Matches C++ SwayClientUpdate::clientUpdate
    /// from SwayClientUpdate.cpp line 77
    fn client_update(&mut self) {
        // If not swaying, nothing to do
        // Matches C++ lines 79-80
        if !self.swaying {
            return;
        }

        // Get the drawable
        let draw = match self.drawable {
            Some(ptr) => unsafe { &mut *ptr },
            None => return,
        };

        // Check if breeze parameters have changed
        // If so, update even if not visible to prevent 'pop' when first viewed
        // Matches C++ lines 84-96
        let info = if let Some(se_ptr) = self.script_engine {
            unsafe { (*se_ptr).get_breeze_info() }
        } else {
            return;
        };

        if info.breeze_version != self.cur_version {
            // Breeze changed, update parameters
            self.update_sway();
        } else {
            // Otherwise, only update visible drawables
            // Matches C++ lines 93-96
            if !draw.is_visible() {
                return;
            }
        }

        // Update oscillation value
        // Matches C++ lines 98-100
        self.cur_value += self.cur_delta;
        if self.cur_value > 2.0 * PI {
            self.cur_value -= 2.0 * PI;
        }

        // Calculate target angle using cosine wave
        // Matches C++ lines 101-104
        let cosine = cos(self.cur_value);
        let target_angle = cosine * self.cur_angle_limit + self.lean_angle;
        let delta_angle = target_angle - self.cur_angle;

        // Apply rotation to instance matrix
        // Matches C++ lines 106-109
        let mut xfrm = *draw.get_instance_matrix();
        xfrm.in_place_pre_rotate_x(-delta_angle * info.direction_vec.x);
        xfrm.in_place_pre_rotate_y(delta_angle * info.direction_vec.y);
        draw.set_instance_matrix(&xfrm);

        // Update current angle
        // Matches C++ line 111
        self.cur_angle = target_angle;

        // Burned things don't sway
        // Matches C++ lines 113-116
        if let Some(obj) = draw.get_object() {
            if (obj.get_status_bits() & OBJECT_STATUS_BURNED) != 0 {
                self.stop_sway();
            }
        }
    }

    /// CRC calculation for save game verification
    /// Matches C++ SwayClientUpdate::crc
    /// from SwayClientUpdate.cpp line 123
    fn crc(&self, xfer: &mut dyn XferInterface) {
        // Extend base class
    }

    /// Serialization/deserialization
    /// Version Info:
    /// 1: Initial version
    /// Matches C++ SwayClientUpdate::xfer
    /// from SwayClientUpdate.cpp line 136
    fn xfer(&mut self, xfer: &mut dyn XferInterface) {
        // Version tracking
        // Matches C++ lines 140-142
        let current_version: u32 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version);

        // Extend base class
        // Matches C++ line 145

        // Current value
        // Matches C++ lines 147-148
        xfer.xfer_real(&mut self.cur_value);

        // Current angle
        // Matches C++ lines 150-151
        xfer.xfer_real(&mut self.cur_angle);

        // Current delta
        // Matches C++ lines 153-154
        xfer.xfer_real(&mut self.cur_delta);

        // Current angle limit
        // Matches C++ lines 156-157
        xfer.xfer_real(&mut self.cur_angle_limit);

        // Lean angle
        // Matches C++ lines 159-160
        xfer.xfer_real(&mut self.lean_angle);

        // Current version
        // Matches C++ lines 162-163
        xfer.xfer_short(&mut self.cur_version);

        // Swaying flag
        // Matches C++ lines 165-166
        xfer.xfer_bool(&mut self.swaying);
    }

    /// Load post process - resolve references after loading
    /// Matches C++ SwayClientUpdate::loadPostProcess
    /// from SwayClientUpdate.cpp line 173
    fn load_post_process(&mut self) {
        // Extend base class
        // Matches C++ line 177

        // Update sway parameters after loading
        // Matches C++ line 179
        self.update_sway();
    }

    /// Get the drawable this module is attached to
    fn get_drawable(&mut self) -> Option<&mut Drawable> {
        self.drawable.map(|ptr| unsafe { &mut *ptr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construction() {
        let module = SwayClientUpdate::new(None, None, None);
        assert_eq!(module.cur_value, 0.0);
        assert_eq!(module.cur_angle, 0.0);
        assert_eq!(module.cur_delta, 0.0);
        assert_eq!(module.swaying, true);
        assert_eq!(module.cur_version, -1);
    }

    #[test]
    fn test_stop_sway() {
        let mut module = SwayClientUpdate::new(None, None, None);
        assert!(module.swaying);

        module.stop_sway();
        assert!(!module.swaying);
    }

    #[test]
    fn test_oscillation_wraps() {
        let two_pi = 2.0 * PI;
        let mut module = SwayClientUpdate::new(None, None, None);

        module.cur_value = two_pi - 0.1;
        module.cur_delta = 0.2;

        // Simulate the wrap-around logic
        module.cur_value += module.cur_delta;
        if module.cur_value > two_pi {
            module.cur_value -= two_pi;
        }

        // Should wrap back to ~0.1
        assert!((module.cur_value - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_cosine_calculation() {
        // Test that cosine produces expected values
        assert!((cos(0.0) - 1.0).abs() < 0.001);
        assert!((cos(PI) - (-1.0)).abs() < 0.001);
        assert!(cos(PI / 2.0).abs() < 0.001);
    }

    #[test]
    fn test_matrix_rotation_x() {
        let mut matrix = Matrix3D::new();
        let angle = PI / 4.0; // 45 degrees

        matrix.in_place_pre_rotate_x(angle);

        // After rotating around X, the X axis should be unchanged
        assert!((matrix.data[0][0] - 1.0).abs() < 0.001);
        assert!(matrix.data[0][1].abs() < 0.001);
        assert!(matrix.data[0][2].abs() < 0.001);
    }

    #[test]
    fn test_matrix_rotation_y() {
        let mut matrix = Matrix3D::new();
        let angle = PI / 4.0; // 45 degrees

        matrix.in_place_pre_rotate_y(angle);

        // After rotating around Y, the Y axis should be unchanged
        assert!(matrix.data[1][0].abs() < 0.001);
        assert!((matrix.data[1][1] - 1.0).abs() < 0.001);
        assert!(matrix.data[1][2].abs() < 0.001);
    }

    #[test]
    fn test_breeze_info_creation() {
        let info = BreezeInfo::new();
        assert_eq!(info.breeze_version, 0);
        assert_eq!(info.intensity, 0.0);
        assert_eq!(info.breeze_period, 1.0);
        assert_eq!(info.randomness, 0.0);
    }
}

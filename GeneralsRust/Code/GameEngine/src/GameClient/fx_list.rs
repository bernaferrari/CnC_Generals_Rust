// FILE: fx_list.rs
// Author: Ported from C++ (Steven Johnson, December 2001)
// Desc: General Effects Descriptions - FXList system for audio/visual effects
//
// Ported from:
// - /GeneralsMD/Code/GameEngine/Include/GameClient/FXList.h
// - /GeneralsMD/Code/GameEngine/Source/GameClient/FXList.cpp

use std::collections::HashMap;
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::Common::{Coord3D, RGBColor};

// Forward declarations / imports (these would need to be properly imported)
// pub use crate::GameLogic::Object;
// pub use crate::GameClient::ParticleSys::ParticleSystem;
// pub use crate::GameClient::Display;

/// Matrix3D placeholder (would be imported from math library)
#[derive(Clone, Debug)]
pub struct Matrix3D {
    // Internal matrix representation
    data: [[f32; 4]; 4],
}

impl Matrix3D {
    pub fn new_identity() -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn rotate_vector(&self, v: &Coord3D) -> Coord3D {
        Coord3D {
            x: self.data[0][0] * v.x + self.data[0][1] * v.y + self.data[0][2] * v.z,
            y: self.data[1][0] * v.x + self.data[1][1] * v.y + self.data[1][2] * v.z,
            z: self.data[2][0] * v.x + self.data[2][1] * v.y + self.data[2][2] * v.z,
        }
    }

    pub fn rotate_z(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rotation = [
            [cos_a, -sin_a, 0.0, 0.0],
            [sin_a, cos_a, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        // Matrix multiplication would go here
        self.data = rotation;
    }
}

/// Placeholder for Object type
pub struct Object {
    // Would contain game object data
}

impl Object {
    pub fn get_position(&self) -> &Coord3D {
        // Placeholder
        static DEFAULT_POS: Coord3D = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        &DEFAULT_POS
    }

    pub fn get_transform_matrix(&self) -> &Matrix3D {
        // Placeholder
        static DEFAULT_MTX: Matrix3D = Matrix3D {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        &DEFAULT_MTX
    }
}

/// Helper function to adjust vector by matrix transformation
/// Matches C++ FXList.cpp:42-55
fn adjust_vector(vec: &mut Coord3D, mtx: Option<&Matrix3D>) {
    if let Some(matrix) = mtx {
        *vec = matrix.rotate_vector(vec);
    }
}

/// An FXNugget encapsulates a particular type of audio/video effect.
///
/// FXNuggets are virtually never used on their own, but rather as a component
/// of an FXList. As part of an FXList, an FXNugget is shared between multiple
/// units. Therefore, an FXNugget should not require private data storage aside
/// from parameters initialized at instantiation time (e.g., from INI file).
///
/// All methods are const to enforce this immutability constraint.
///
/// Matches C++ FXList.h:49-72
pub trait FXNugget: Send + Sync {
    /// Perform the sound and/or video effects at the specified position.
    ///
    /// # Arguments
    /// * `primary` - Primary position (can be None)
    /// * `primary_mtx` - Primary transformation matrix (optional)
    /// * `primary_speed` - Speed of primary object
    /// * `secondary` - Secondary position (can be None)
    /// * `override_radius` - Override radius for effect
    ///
    /// Matches C++ FXList.h:61
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        override_radius: f32,
    );

    /// Perform effects on objects.
    ///
    /// By default, this extracts positions and calls do_fx_pos().
    /// Note that primary and/or secondary can be None.
    ///
    /// Matches C++ FXList.h:67
    fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
        let p = primary.map(|obj| obj.get_position());
        let mtx = primary.map(|obj| obj.get_transform_matrix());
        let speed = 0.0; // C++ FXList.cpp:66 - always 0.0
        let s = secondary.map(|obj| obj.get_position());

        self.do_fx_pos(
            p,
            mtx,
            speed,
            s,
            0.0,
        );
    }
}

/// Sound effect nugget
///
/// Plays an audio event at a specified location.
/// Matches C++ FXList.cpp:72-119
struct SoundFXNugget {
    sound_name: String,
}

impl FXNugget for SoundFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // AudioEventRTS sound(m_soundName);
        // if (primary) sound.setPosition(primary);
        // TheAudio->addAudioEvent(&sound);

        // Placeholder - would call audio system
        if let Some(pos) = primary {
            println!("Playing sound '{}' at ({}, {}, {})",
                     self.sound_name, pos.x, pos.y, pos.z);
        }
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        // C++ FXList.cpp:90-100 - object-specific version with player index
        if let Some(obj) = primary {
            let pos = obj.get_position();
            println!("Playing sound '{}' at ({}, {}, {})",
                     self.sound_name, pos.x, pos.y, pos.z);
        }
    }
}

/// Tracer effect nugget
///
/// Creates projectile tracer effects between two points.
/// Matches C++ FXList.cpp:131-231
struct TracerFXNugget {
    tracer_name: String,
    bone_name: String,
    speed: f32,
    decay_at: f32,
    length: f32,
    width: f32,
    color: RGBColor,
    probability: f32,
}

impl Default for TracerFXNugget {
    fn default() -> Self {
        Self {
            tracer_name: "GenericTracer".to_string(),
            bone_name: String::new(),
            speed: 0.0, // 0 means use passed-in speed
            decay_at: 1.0,
            length: 10.0,
            width: 1.0,
            color: RGBColor { red: 1.0, green: 1.0, blue: 1.0 },
            probability: 1.0,
        }
    }
}

impl FXNugget for TracerFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // Matches C++ FXList.cpp:148-199

        // Probability check
        let rand_val: f32 = rand::random();
        if self.probability <= rand_val {
            return;
        }

        if let (Some(prim), Some(sec)) = (primary, secondary) {
            // Calculate tracer orientation to face from primary to secondary
            // Matches C++ FXList.cpp:165-171
            let dir_x = sec.x - prim.x;
            let dir_y = sec.y - prim.y;
            let dir_z = sec.z - prim.z;

            let length = (dir_x * dir_x + dir_y * dir_y + dir_z * dir_z).sqrt();
            if length > 0.0 {
                let _norm_x = dir_x / length;
                let _norm_y = dir_y / length;
                let _norm_z = dir_z / length;
            }

            // Use provided speed or default
            let speed = if self.speed == 0.0 { primary_speed } else { self.speed };

            // Calculate expiration
            // Matches C++ FXList.cpp:189-193
            let dist = calc_dist(prim, sec) - self.length;
            let frames = if dist >= 0.0 && speed >= 0.0 {
                dist / speed
            } else {
                1.0
            };
            let _frames_adjusted = (frames * self.decay_at).ceil() as u32;

            println!("Creating tracer '{}' from ({}, {}, {}) to ({}, {}, {})",
                     self.tracer_name, prim.x, prim.y, prim.z, sec.x, sec.y, sec.z);
        }
    }
}

/// Calculate distance between two 3D coordinates
/// Matches C++ FXList.cpp:122-128
fn calc_dist(src: &Coord3D, dst: &Coord3D) -> f32 {
    let dx = dst.x - src.x;
    let dy = dst.y - src.y;
    let dz = dst.z - src.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Ray effect nugget
///
/// Creates ray effects between two points (e.g., laser beams).
/// Matches C++ FXList.cpp:234-290
struct RayEffectFXNugget {
    template_name: String,
    primary_offset: Coord3D,
    secondary_offset: Coord3D,
}

impl Default for RayEffectFXNugget {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            primary_offset: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            secondary_offset: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
        }
    }
}

impl FXNugget for RayEffectFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // Matches C++ FXList.cpp:246-268
        if let (Some(prim), Some(sec)) = (primary, secondary) {
            let mut source_pos = *prim;
            source_pos.x += self.primary_offset.x;
            source_pos.y += self.primary_offset.y;
            source_pos.z += self.primary_offset.z;

            let mut target_pos = *sec;
            target_pos.x += self.secondary_offset.x;
            target_pos.y += self.secondary_offset.y;
            target_pos.z += self.secondary_offset.z;

            println!("Creating ray effect '{}' from ({}, {}, {}) to ({}, {}, {})",
                     self.template_name,
                     source_pos.x, source_pos.y, source_pos.z,
                     target_pos.x, target_pos.y, target_pos.z);
        }
    }
}

/// Light pulse nugget
///
/// Creates a pulsing light effect.
/// Matches C++ FXList.cpp:293-356
struct LightPulseFXNugget {
    color: RGBColor,
    radius: f32,
    bounding_circle_pct: f32,
    increase_frames: u32,
    decrease_frames: u32,
}

impl Default for LightPulseFXNugget {
    fn default() -> Self {
        Self {
            color: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
            radius: 0.0,
            bounding_circle_pct: 0.0,
            increase_frames: 0,
            decrease_frames: 0,
        }
    }
}

impl FXNugget for LightPulseFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // Matches C++ FXList.cpp:320-330
        if let Some(pos) = primary {
            println!("Creating light pulse at ({}, {}, {}) with radius {}",
                     pos.x, pos.y, pos.z, self.radius);
        }
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        // Matches C++ FXList.cpp:303-318
        if let Some(_obj) = primary {
            let mut radius = self.radius;
            if self.bounding_circle_pct > 0.0 {
                // radius = obj.getGeometryInfo().getBoundingCircleRadius() * bounding_circle_pct
                radius = 10.0 * self.bounding_circle_pct; // Placeholder
            }
            println!("Creating light pulse with radius {}", radius);
        }
    }
}

/// Camera shake types
/// Matches C++ View.h camera shake enums
#[derive(Clone, Copy, Debug)]
pub enum CameraShakeType {
    Subtle = 0,
    Normal = 1,
    Strong = 2,
    Severe = 3,
    CineExtreme = 4,
    CineInsane = 5,
}

/// View shake nugget
///
/// Shakes the camera view.
/// Matches C++ FXList.cpp:359-414
struct ViewShakeFXNugget {
    shake: CameraShakeType,
}

impl Default for ViewShakeFXNugget {
    fn default() -> Self {
        Self {
            shake: CameraShakeType::Normal,
        }
    }
}

impl FXNugget for ViewShakeFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // Matches C++ FXList.cpp:368-379
        if let Some(pos) = primary {
            println!("Shaking view at ({}, {}, {}) with intensity {:?}",
                     pos.x, pos.y, pos.z, self.shake);
        }
    }
}

/// Scorch types
/// Matches C++ scorches enums
#[derive(Clone, Copy, Debug)]
pub enum ScorchType {
    Scorch1 = 0,
    Scorch2 = 1,
    Scorch3 = 2,
    Scorch4 = 3,
    ShadowScorch = 4,
    Random = -1,
}

/// Terrain scorch nugget
///
/// Adds scorch marks to terrain.
/// Matches C++ FXList.cpp:417-478
struct TerrainScorchFXNugget {
    scorch: i32, // -1 for random
    radius: f32,
}

impl Default for TerrainScorchFXNugget {
    fn default() -> Self {
        Self {
            scorch: -1, // Random by default
            radius: 0.0,
        }
    }
}

impl FXNugget for TerrainScorchFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // Matches C++ FXList.cpp:426-441
        if let Some(pos) = primary {
            let scorch = if self.scorch < 0 {
                // Random scorch type
                (rand::random::<f32>() * 4.0) as i32
            } else {
                self.scorch
            };

            println!("Adding scorch {} at ({}, {}, {}) with radius {}",
                     scorch, pos.x, pos.y, pos.z, self.radius);
        }
    }
}

/// Random variable for client-side randomness
/// Matches C++ GameClientRandomVariable
#[derive(Clone, Copy, Debug)]
pub struct GameClientRandomVariable {
    min: f32,
    max: f32,
    distribution_type: RandomDistribution,
}

#[derive(Clone, Copy, Debug)]
pub enum RandomDistribution {
    Constant,
    Uniform,
}

impl GameClientRandomVariable {
    pub fn new_constant(value: f32) -> Self {
        Self {
            min: value,
            max: value,
            distribution_type: RandomDistribution::Constant,
        }
    }

    pub fn new_uniform(min: f32, max: f32) -> Self {
        Self {
            min,
            max,
            distribution_type: RandomDistribution::Uniform,
        }
    }

    pub fn get_value(&self) -> f32 {
        match self.distribution_type {
            RandomDistribution::Constant => self.min,
            RandomDistribution::Uniform => {
                self.min + rand::random::<f32>() * (self.max - self.min)
            }
        }
    }
}

/// Particle system FX nugget
///
/// Creates particle system effects.
/// Matches C++ FXList.cpp:481-658
struct ParticleSystemFXNugget {
    name: String,
    count: i32,
    offset: Coord3D,
    radius: GameClientRandomVariable,
    height: GameClientRandomVariable,
    delay: GameClientRandomVariable,
    rotate_x: f32,
    rotate_y: f32,
    rotate_z: f32,
    orient_to_object: bool,
    ricochet: bool,
    attach_to_object: bool,
    create_at_ground_height: bool,
    use_callers_radius: bool,
}

impl Default for ParticleSystemFXNugget {
    fn default() -> Self {
        Self {
            name: String::new(),
            count: 1,
            offset: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            radius: GameClientRandomVariable::new_constant(0.0),
            height: GameClientRandomVariable::new_constant(0.0),
            delay: GameClientRandomVariable::new_constant(-1.0),
            rotate_x: 0.0,
            rotate_y: 0.0,
            rotate_z: 0.0,
            orient_to_object: false,
            ricochet: false,
            attach_to_object: false,
            create_at_ground_height: false,
            use_callers_radius: false,
        }
    }
}

impl FXNugget for ParticleSystemFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        override_radius: f32,
    ) {
        // Matches C++ FXList.cpp:502-512
        if let Some(prim) = primary {
            self.really_do_fx(prim, primary_mtx, None, override_radius);
        }
    }

    fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
        // Matches C++ FXList.cpp:514-540
        if let Some(prim_obj) = primary {
            if self.ricochet && secondary.is_some() {
                // Build ricochet matrix
                // Matches C++ FXList.cpp:523-527
                let _sec_obj = secondary.unwrap();
                // Calculate aiming angle and matrix
                // let delta_x = prim_pos.x - sec_pos.x;
                // let delta_y = prim_pos.y - sec_pos.y;
                // let aiming_angle = delta_y.atan2(delta_x);
            }

            let pos = prim_obj.get_position();
            let mtx = prim_obj.get_transform_matrix();
            self.really_do_fx(pos, Some(mtx), Some(prim_obj), 0.0);
        }
    }
}

impl ParticleSystemFXNugget {
    /// Actually create the particle system effects
    /// Matches C++ FXList.cpp:570-641
    fn really_do_fx(
        &self,
        primary: &Coord3D,
        mtx: Option<&Matrix3D>,
        _thing_to_attach_to: Option<&Object>,
        override_radius: f32,
    ) {
        let mut offset = self.offset;
        if let Some(matrix) = mtx {
            adjust_vector(&mut offset, Some(matrix));
        }

        for i in 0..self.count {
            let radius = self.radius.get_value();
            let angle = rand::random::<f32>() * 2.0 * PI;

            let mut new_pos = Coord3D {
                x: primary.x + offset.x + radius * angle.cos(),
                y: primary.y + offset.y + radius * angle.sin(),
                z: primary.z + offset.z + self.height.get_value(),
            };

            if self.create_at_ground_height {
                // Get ground height
                new_pos.z = 0.0; // Placeholder
            }

            println!("Creating particle system '{}' #{} at ({}, {}, {})",
                     self.name, i, new_pos.x, new_pos.y, new_pos.z);

            if override_radius > 0.0 && self.use_callers_radius {
                println!("  Using caller's radius: {}", override_radius);
            }
        }
    }
}

/// FX list at bone position nugget
///
/// Executes another FX list at bone positions.
/// Matches C++ FXList.cpp:661-739
struct FXListAtBonePosFXNugget {
    fx: Option<Rc<FXList>>,
    bone_name: String,
    orient_to_bone: bool,
}

impl Default for FXListAtBonePosFXNugget {
    fn default() -> Self {
        Self {
            fx: None,
            bone_name: String::new(),
            orient_to_bone: true,
        }
    }
}

impl FXNugget for FXListAtBonePosFXNugget {
    fn do_fx_pos(
        &self,
        _primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // Must use object form
        // Matches C++ FXList.cpp:673-676
        panic!("You must use the object form for this effect");
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        // Matches C++ FXList.cpp:678-692
        if let Some(_obj) = primary {
            // Get bone positions and execute FX at each
            // Matches C++ FXList.cpp:711-728
            println!("Executing FX at bone positions for '{}'", self.bone_name);
        }
    }
}

/// An FXList is a collection of FXNuggets representing a complete audio/visual effect.
///
/// FXLists are specified solely by name and receive only position parameters when
/// performing their effects. There is no inheritance or overriding - if you need
/// a variant, create a new FXList.
///
/// An FXList is shared between multiple units, so all methods are const.
/// Unlike most game systems, FXLists cannot be overridden by subsequent INI loads.
///
/// Matches C++ FXList.h:99-162
pub struct FXList {
    nuggets: Vec<Box<dyn FXNugget>>,
}

impl FXList {
    /// Create a new empty FXList
    /// Matches C++ FXList.cpp:760-762
    pub fn new() -> Self {
        Self {
            nuggets: Vec::new(),
        }
    }

    /// Add a nugget to this FXList
    /// The FXList takes ownership of the nugget.
    /// Matches C++ FXList.h:115-118
    pub fn add_fx_nugget(&mut self, nugget: Box<dyn FXNugget>) {
        self.nuggets.push(nugget);
    }

    /// Perform FX at a position
    /// Matches C++ FXList.cpp:782-791
    pub fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        override_radius: f32,
    ) {
        // Check shroud status in real implementation
        // Matches C++ FXList.cpp:784-785

        for nugget in &self.nuggets {
            nugget.do_fx_pos(primary, primary_mtx, primary_speed, secondary, override_radius);
        }
    }

    /// Perform FX on objects
    /// Matches C++ FXList.cpp:794-805
    pub fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
        // Check shroud status in real implementation
        // Matches C++ FXList.cpp:796-797

        for nugget in &self.nuggets {
            nugget.do_fx_obj(primary, secondary);
        }
    }

    /// Convenience method to safely execute FX (handles None case)
    /// Matches C++ FXList.h:121-124
    pub fn do_fx_pos_safe(
        fx: Option<&FXList>,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        override_radius: f32,
    ) {
        if let Some(fx_list) = fx {
            fx_list.do_fx_pos(primary, primary_mtx, primary_speed, secondary, override_radius);
        }
    }

    /// Convenience method to safely execute FX on objects
    /// Matches C++ FXList.h:127-137
    pub fn do_fx_obj_safe(
        fx: Option<&FXList>,
        primary: Option<&Object>,
        secondary: Option<&Object>,
    ) {
        if let Some(fx_list) = fx {
            fx_list.do_fx_obj(primary, secondary);
        }
    }
}

impl Default for FXList {
    fn default() -> Self {
        Self::new()
    }
}

/// The FXListStore maintains all FXLists in existence.
///
/// FXLists are looked up by name (via name key hash).
/// Matches C++ FXList.h:168-195
pub struct FXListStore {
    fx_map: HashMap<String, Rc<FXList>>,
}

impl FXListStore {
    /// Create a new FXListStore
    /// Matches C++ FXList.cpp:814-816
    pub fn new() -> Self {
        Self {
            fx_map: HashMap::new(),
        }
    }

    /// Find an FXList by name
    /// Returns None if not found or if name is "None"
    /// Matches C++ FXList.cpp:825-836
    pub fn find_fx_list(&self, name: &str) -> Option<Rc<FXList>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }

        self.fx_map.get(name).cloned()
    }

    /// Add an FXList to the store
    pub fn add_fx_list(&mut self, name: String, fx_list: FXList) {
        self.fx_map.insert(name, Rc::new(fx_list));
    }

    /// Parse FXList definition from INI file
    /// Matches C++ FXList.cpp:839-847
    pub fn parse_fx_list_definition(&mut self, ini_data: &str) -> Result<(), String> {
        let parser = FXListINIParser::new(ini_data);
        parser.parse(self)
    }
}

impl Default for FXListStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global FXListStore singleton
/// Matches C++ FXList.h:198
pub static THE_FX_LIST_STORE: OnceLock<Mutex<FXListStore>> = OnceLock::new();

/// Initialize the global FXListStore
pub fn init_fx_list_store() {
    THE_FX_LIST_STORE.get_or_init(|| Mutex::new(FXListStore::new()));
}

/// Get reference to the global FXListStore
pub fn get_fx_list_store() -> Option<&'static Mutex<FXListStore>> {
    THE_FX_LIST_STORE.get()
}

/// Get mutable reference to the global FXListStore
pub fn get_fx_list_store_mut() -> Option<std::sync::MutexGuard<'static, FXListStore>> {
    THE_FX_LIST_STORE.get().and_then(|m| m.lock().ok())
}

/// INI Parser for FXList definitions
/// Matches C++ FXList.cpp parsing implementation
struct FXListINIParser<'a> {
    lines: Vec<&'a str>,
    current_line: usize,
}

impl<'a> FXListINIParser<'a> {
    fn new(ini_data: &'a str) -> Self {
        let lines: Vec<&str> = ini_data.lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with(';'))
            .collect();

        Self {
            lines,
            current_line: 0,
        }
    }

    /// Parse all FXLists from the INI data
    /// Matches C++ FXListStore::parseFXListDefinition (FXList.cpp:839-847)
    fn parse(&self, store: &mut FXListStore) -> Result<(), String> {
        let mut parser = self.clone();

        while parser.current_line < parser.lines.len() {
            if let Some(line) = parser.peek_line() {
                if line.starts_with("FXList ") {
                    parser.parse_single_fx_list(store)?;
                } else {
                    parser.current_line += 1;
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    /// Parse a single FXList definition
    /// Matches C++ FXList.cpp:839-847 and initFromINI pattern
    fn parse_single_fx_list(&mut self, store: &mut FXListStore) -> Result<(), String> {
        // Read "FXList <name>"
        let line = self.get_line()?;
        let name = line.strip_prefix("FXList ")
            .ok_or_else(|| format!("Expected 'FXList <name>', got: {}", line))?
            .trim()
            .to_string();

        let mut fx_list = FXList::new();

        // Parse nuggets until "End"
        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1; // consume "End"
                break;
            }

            // Parse nugget based on type
            // Matches C++ TheFXListFieldParse table (FXList.cpp:746-757)
            match line {
                "Sound" => self.parse_sound_nugget(&mut fx_list)?,
                "Tracer" => self.parse_tracer_nugget(&mut fx_list)?,
                "RayEffect" => self.parse_ray_effect_nugget(&mut fx_list)?,
                "LightPulse" => self.parse_light_pulse_nugget(&mut fx_list)?,
                "ViewShake" => self.parse_view_shake_nugget(&mut fx_list)?,
                "TerrainScorch" => self.parse_terrain_scorch_nugget(&mut fx_list)?,
                "ParticleSystem" => self.parse_particle_system_nugget(&mut fx_list)?,
                "FXListAtBonePos" => self.parse_fx_list_at_bone_pos_nugget(&mut fx_list)?,
                _ => {
                    return Err(format!("Unknown FXNugget type: {}", line));
                }
            }
        }

        store.add_fx_list(name, fx_list);
        Ok(())
    }

    /// Parse Sound nugget
    /// Matches C++ SoundFXNugget::parse (FXList.cpp:103-114)
    fn parse_sound_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "Sound"

        let mut sound_name = String::new();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("Name = ") {
                sound_name = value.trim().to_string();
                self.current_line += 1;
            } else {
                self.current_line += 1;
            }
        }

        if !sound_name.is_empty() {
            fx_list.add_fx_nugget(Box::new(SoundFXNugget { sound_name }));
        }

        Ok(())
    }

    /// Parse Tracer nugget
    /// Matches C++ TracerFXNugget::parse (FXList.cpp:201-219)
    fn parse_tracer_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "Tracer"

        let mut nugget = TracerFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("TracerName = ") {
                nugget.tracer_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("BoneName = ") {
                nugget.bone_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("Speed = ") {
                nugget.speed = value.trim().parse().map_err(|e| format!("Invalid Speed: {}", e))?;
            } else if let Some(value) = line.strip_prefix("DecayAt = ") {
                nugget.decay_at = value.trim().parse().map_err(|e| format!("Invalid DecayAt: {}", e))?;
            } else if let Some(value) = line.strip_prefix("Length = ") {
                nugget.length = value.trim().parse().map_err(|e| format!("Invalid Length: {}", e))?;
            } else if let Some(value) = line.strip_prefix("Width = ") {
                nugget.width = value.trim().parse().map_err(|e| format!("Invalid Width: {}", e))?;
            } else if let Some(value) = line.strip_prefix("Color = ") {
                nugget.color = Self::parse_rgb_color(value)?;
            } else if let Some(value) = line.strip_prefix("Probability = ") {
                nugget.probability = value.trim().parse().map_err(|e| format!("Invalid Probability: {}", e))?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    /// Parse RayEffect nugget
    /// Matches C++ RayEffectFXNugget::parse (FXList.cpp:270-283)
    fn parse_ray_effect_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "RayEffect"

        let mut nugget = RayEffectFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("Name = ") {
                nugget.template_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("PrimaryOffset = ") {
                nugget.primary_offset = Self::parse_coord3d(value)?;
            } else if let Some(value) = line.strip_prefix("SecondaryOffset = ") {
                nugget.secondary_offset = Self::parse_coord3d(value)?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    /// Parse LightPulse nugget
    /// Matches C++ LightPulseFXNugget::parse (FXList.cpp:332-347)
    fn parse_light_pulse_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "LightPulse"

        let mut nugget = LightPulseFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("Color = ") {
                nugget.color = Self::parse_rgb_color(value)?;
            } else if let Some(value) = line.strip_prefix("Radius = ") {
                nugget.radius = value.trim().parse().map_err(|e| format!("Invalid Radius: {}", e))?;
            } else if let Some(value) = line.strip_prefix("RadiusAsPercentOfObjectSize = ") {
                // Parse as percent (e.g., "50%" -> 0.5)
                let percent_str = value.trim().trim_end_matches('%');
                let percent: f32 = percent_str.parse().map_err(|e| format!("Invalid percent: {}", e))?;
                nugget.bounding_circle_pct = percent / 100.0;
            } else if let Some(value) = line.strip_prefix("IncreaseTime = ") {
                nugget.increase_frames = Self::parse_duration(value)?;
            } else if let Some(value) = line.strip_prefix("DecreaseTime = ") {
                nugget.decrease_frames = Self::parse_duration(value)?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    /// Parse ViewShake nugget
    /// Matches C++ ViewShakeFXNugget::parse (FXList.cpp:381-392)
    fn parse_view_shake_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "ViewShake"

        let mut nugget = ViewShakeFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("Type = ") {
                nugget.shake = Self::parse_shake_type(value)?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    /// Parse TerrainScorch nugget
    /// Matches C++ TerrainScorchFXNugget::parse (FXList.cpp:443-455)
    fn parse_terrain_scorch_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "TerrainScorch"

        let mut nugget = TerrainScorchFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("Type = ") {
                nugget.scorch = Self::parse_scorch_type(value)?;
            } else if let Some(value) = line.strip_prefix("Radius = ") {
                nugget.radius = value.trim().parse().map_err(|e| format!("Invalid Radius: {}", e))?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    /// Parse ParticleSystem nugget
    /// Matches C++ ParticleSystemFXNugget::parse (FXList.cpp:542-566)
    fn parse_particle_system_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "ParticleSystem"

        let mut nugget = ParticleSystemFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("Name = ") {
                nugget.name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("Count = ") {
                nugget.count = value.trim().parse().map_err(|e| format!("Invalid Count: {}", e))?;
            } else if let Some(value) = line.strip_prefix("Offset = ") {
                nugget.offset = Self::parse_coord3d(value)?;
            } else if let Some(value) = line.strip_prefix("Radius = ") {
                nugget.radius = Self::parse_random_variable(value)?;
            } else if let Some(value) = line.strip_prefix("Height = ") {
                nugget.height = Self::parse_random_variable(value)?;
            } else if let Some(value) = line.strip_prefix("InitialDelay = ") {
                nugget.delay = Self::parse_random_variable(value)?;
            } else if let Some(value) = line.strip_prefix("RotateX = ") {
                nugget.rotate_x = Self::parse_angle(value)?;
            } else if let Some(value) = line.strip_prefix("RotateY = ") {
                nugget.rotate_y = Self::parse_angle(value)?;
            } else if let Some(value) = line.strip_prefix("RotateZ = ") {
                nugget.rotate_z = Self::parse_angle(value)?;
            } else if let Some(value) = line.strip_prefix("OrientToObject = ") {
                nugget.orient_to_object = Self::parse_bool(value)?;
            } else if let Some(value) = line.strip_prefix("Ricochet = ") {
                nugget.ricochet = Self::parse_bool(value)?;
            } else if let Some(value) = line.strip_prefix("AttachToObject = ") {
                nugget.attach_to_object = Self::parse_bool(value)?;
            } else if let Some(value) = line.strip_prefix("CreateAtGroundHeight = ") {
                nugget.create_at_ground_height = Self::parse_bool(value)?;
            } else if let Some(value) = line.strip_prefix("UseCallersRadius = ") {
                nugget.use_callers_radius = Self::parse_bool(value)?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    /// Parse FXListAtBonePos nugget
    /// Matches C++ FXListAtBonePosFXNugget::parse (FXList.cpp:694-707)
    fn parse_fx_list_at_bone_pos_nugget(&mut self, fx_list: &mut FXList) -> Result<(), String> {
        self.current_line += 1; // consume "FXListAtBonePos"

        let mut nugget = FXListAtBonePosFXNugget::default();

        while let Some(line) = self.peek_line() {
            if line == "End" {
                self.current_line += 1;
                break;
            }

            if let Some(value) = line.strip_prefix("FX = ") {
                // FX reference would need to be resolved after all FXLists are loaded
                // For now, store the name as a placeholder
                // In a full implementation, this would be resolved in a second pass
                let _fx_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("BoneName = ") {
                nugget.bone_name = value.trim().to_string();
            } else if let Some(value) = line.strip_prefix("OrientToBone = ") {
                nugget.orient_to_bone = Self::parse_bool(value)?;
            }

            self.current_line += 1;
        }

        fx_list.add_fx_nugget(Box::new(nugget));
        Ok(())
    }

    // Helper parsing functions

    /// Parse Coord3D in format "X:1.0 Y:2.0 Z:3.0"
    /// Matches C++ INI::parseCoord3D
    fn parse_coord3d(value: &str) -> Result<Coord3D, String> {
        let mut coord = Coord3D { x: 0.0, y: 0.0, z: 0.0 };

        for part in value.split_whitespace() {
            if let Some(x) = part.strip_prefix("X:") {
                coord.x = x.parse().map_err(|e| format!("Invalid X coordinate: {}", e))?;
            } else if let Some(y) = part.strip_prefix("Y:") {
                coord.y = y.parse().map_err(|e| format!("Invalid Y coordinate: {}", e))?;
            } else if let Some(z) = part.strip_prefix("Z:") {
                coord.z = z.parse().map_err(|e| format!("Invalid Z coordinate: {}", e))?;
            }
        }

        Ok(coord)
    }

    /// Parse RGBColor in format "R:255 G:128 B:64"
    /// Matches C++ INI::parseRGBColor
    fn parse_rgb_color(value: &str) -> Result<RGBColor, String> {
        let mut r = 0u8;
        let mut g = 0u8;
        let mut b = 0u8;

        for part in value.split_whitespace() {
            if let Some(red) = part.strip_prefix("R:") {
                r = red.parse().map_err(|e| format!("Invalid R value: {}", e))?;
            } else if let Some(green) = part.strip_prefix("G:") {
                g = green.parse().map_err(|e| format!("Invalid G value: {}", e))?;
            } else if let Some(blue) = part.strip_prefix("B:") {
                b = blue.parse().map_err(|e| format!("Invalid B value: {}", e))?;
            }
        }

        Ok(RGBColor {
            red: r as f32 / 255.0,
            green: g as f32 / 255.0,
            blue: b as f32 / 255.0,
        })
    }

    /// Parse GameClientRandomVariable in format "min max [CONSTANT|UNIFORM]"
    /// Matches C++ INI::parseGameClientRandomVariable
    fn parse_random_variable(value: &str) -> Result<GameClientRandomVariable, String> {
        let parts: Vec<&str> = value.split_whitespace().collect();

        if parts.len() < 2 {
            return Err(format!("Invalid random variable format: {}", value));
        }

        let min: f32 = parts[0].parse().map_err(|e| format!("Invalid min: {}", e))?;
        let max: f32 = parts[1].parse().map_err(|e| format!("Invalid max: {}", e))?;

        let distribution = if parts.len() >= 3 {
            match parts[2] {
                "CONSTANT" => RandomDistribution::Constant,
                "UNIFORM" => RandomDistribution::Uniform,
                _ => RandomDistribution::Uniform,
            }
        } else {
            RandomDistribution::Uniform
        };

        Ok(GameClientRandomVariable {
            min,
            max,
            distribution_type: distribution,
        })
    }

    /// Parse boolean value (Yes/No, True/False, 1/0)
    /// Matches C++ INI::parseBool
    fn parse_bool(value: &str) -> Result<bool, String> {
        match value.trim().to_uppercase().as_str() {
            "YES" | "TRUE" | "1" => Ok(true),
            "NO" | "FALSE" | "0" => Ok(false),
            _ => Err(format!("Invalid boolean value: {}", value)),
        }
    }

    /// Parse angle in degrees, convert to radians
    /// Matches C++ INI::parseAngleReal
    fn parse_angle(value: &str) -> Result<f32, String> {
        let degrees: f32 = value.trim().parse().map_err(|e| format!("Invalid angle: {}", e))?;
        Ok(degrees * PI / 180.0)
    }

    /// Parse duration in milliseconds, convert to frames (30fps)
    /// Matches C++ INI::parseDurationUnsignedInt
    fn parse_duration(value: &str) -> Result<u32, String> {
        let msec: u32 = value.trim().parse().map_err(|e| format!("Invalid duration: {}", e))?;
        // Convert milliseconds to frames (30 fps = 33.33ms per frame)
        Ok((msec as f32 / 33.333).ceil() as u32)
    }

    /// Parse camera shake type
    /// Matches C++ ViewShakeFXNugget::parseShakeType (FXList.cpp:395-408)
    fn parse_shake_type(value: &str) -> Result<CameraShakeType, String> {
        match value.trim().to_uppercase().as_str() {
            "SUBTLE" => Ok(CameraShakeType::Subtle),
            "NORMAL" => Ok(CameraShakeType::Normal),
            "STRONG" => Ok(CameraShakeType::Strong),
            "SEVERE" => Ok(CameraShakeType::Severe),
            "CINE_EXTREME" => Ok(CameraShakeType::CineExtreme),
            "CINE_INSANE" => Ok(CameraShakeType::CineInsane),
            _ => Err(format!("Unknown shake type: {}", value)),
        }
    }

    /// Parse scorch type
    /// Matches C++ TerrainScorchFXNugget::parseScorchType (FXList.cpp:459-472)
    fn parse_scorch_type(value: &str) -> Result<i32, String> {
        match value.trim().to_uppercase().as_str() {
            "SCORCH_1" => Ok(0),
            "SCORCH_2" => Ok(1),
            "SCORCH_3" => Ok(2),
            "SCORCH_4" => Ok(3),
            "SHADOW_SCORCH" => Ok(4),
            "RANDOM" => Ok(-1),
            _ => Err(format!("Unknown scorch type: {}", value)),
        }
    }

    // Line management helpers

    fn peek_line(&self) -> Option<&str> {
        self.lines.get(self.current_line).copied()
    }

    fn get_line(&mut self) -> Result<&str, String> {
        if self.current_line < self.lines.len() {
            let line = self.lines[self.current_line];
            self.current_line += 1;
            Ok(line)
        } else {
            Err("Unexpected end of file".to_string())
        }
    }
}

impl Clone for FXListINIParser<'_> {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            current_line: self.current_line,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx_list_creation() {
        let fx_list = FXList::new();
        assert_eq!(fx_list.nuggets.len(), 0);
    }

    #[test]
    fn test_fx_list_store() {
        let mut store = FXListStore::new();
        let fx_list = FXList::new();
        store.add_fx_list("test_fx".to_string(), fx_list);

        let found = store.find_fx_list("test_fx");
        assert!(found.is_some());

        let not_found = store.find_fx_list("nonexistent");
        assert!(not_found.is_none());

        let none_fx = store.find_fx_list("None");
        assert!(none_fx.is_none());
    }

    #[test]
    fn test_calc_dist() {
        let a = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        let b = Coord3D { x: 3.0, y: 4.0, z: 0.0 };
        let dist = calc_dist(&a, &b);
        assert!((dist - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_random_variable() {
        let constant = GameClientRandomVariable::new_constant(42.0);
        assert_eq!(constant.get_value(), 42.0);

        let uniform = GameClientRandomVariable::new_uniform(0.0, 100.0);
        let val = uniform.get_value();
        assert!(val >= 0.0 && val <= 100.0);
    }

    #[test]
    fn test_ini_parse_simple_sound_fx() {
        let ini_data = r#"
            FXList FX_TestSound
              Sound
                Name = TestSoundEffect
              End
            End
        "#;

        let mut store = FXListStore::new();
        let result = store.parse_fx_list_definition(ini_data);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let fx_list = store.find_fx_list("FX_TestSound");
        assert!(fx_list.is_some());
        assert_eq!(fx_list.unwrap().nuggets.len(), 1);
    }

    #[test]
    fn test_ini_parse_particle_system() {
        let ini_data = r#"
            FXList FX_DamageTankStruck
              ParticleSystem
                Name = TankStruckSmoke
                Height = 10 10 CONSTANT
                OrientToObject = Yes
                Ricochet = Yes
              End
              LightPulse
                Color = R:255 G:255 B:128
                Radius = 30
                IncreaseTime = 0
                DecreaseTime = 500
              End
            End
        "#;

        let mut store = FXListStore::new();
        let result = store.parse_fx_list_definition(ini_data);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let fx_list = store.find_fx_list("FX_DamageTankStruck");
        assert!(fx_list.is_some());
        assert_eq!(fx_list.unwrap().nuggets.len(), 2);
    }

    #[test]
    fn test_ini_parse_view_shake() {
        let ini_data = r#"
            FXList WeaponFX_BattleshipGun
              ViewShake
                Type = SEVERE
              End
            End
        "#;

        let mut store = FXListStore::new();
        let result = store.parse_fx_list_definition(ini_data);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let fx_list = store.find_fx_list("WeaponFX_BattleshipGun");
        assert!(fx_list.is_some());
        assert_eq!(fx_list.unwrap().nuggets.len(), 1);
    }

    #[test]
    fn test_ini_parse_terrain_scorch() {
        let ini_data = r#"
            FXList FX_TestScorch
              TerrainScorch
                Type = SCORCH_3
                Radius = 25.0
              End
            End
        "#;

        let mut store = FXListStore::new();
        let result = store.parse_fx_list_definition(ini_data);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let fx_list = store.find_fx_list("FX_TestScorch");
        assert!(fx_list.is_some());
    }

    #[test]
    fn test_ini_parse_multiple_fx_lists() {
        let ini_data = r#"
            FXList FX_First
              Sound
                Name = FirstSound
              End
            End

            FXList FX_Second
              Sound
                Name = SecondSound
              End
            End

            FXList FX_Third
              ViewShake
                Type = NORMAL
              End
            End
        "#;

        let mut store = FXListStore::new();
        let result = store.parse_fx_list_definition(ini_data);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        assert!(store.find_fx_list("FX_First").is_some());
        assert!(store.find_fx_list("FX_Second").is_some());
        assert!(store.find_fx_list("FX_Third").is_some());
    }

    #[test]
    fn test_ini_parse_empty_fx_list() {
        // C++ code shows empty FXLists are valid (FXList.ini line 22)
        let ini_data = r#"
            FXList FX_GIDie
            ; yes, an empty FXList. Why? Not sure.
            End
        "#;

        let mut store = FXListStore::new();
        let result = store.parse_fx_list_definition(ini_data);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());

        let fx_list = store.find_fx_list("FX_GIDie");
        assert!(fx_list.is_some());
        assert_eq!(fx_list.unwrap().nuggets.len(), 0);
    }

    #[test]
    fn test_parse_rgb_color() {
        let color = FXListINIParser::parse_rgb_color("R:255 G:128 B:64").unwrap();
        assert!((color.red - 1.0).abs() < 0.01);
        assert!((color.green - 0.502).abs() < 0.01);
        assert!((color.blue - 0.251).abs() < 0.01);
    }

    #[test]
    fn test_parse_coord3d() {
        let coord = FXListINIParser::parse_coord3d("X:1.5 Y:2.5 Z:3.5").unwrap();
        assert_eq!(coord.x, 1.5);
        assert_eq!(coord.y, 2.5);
        assert_eq!(coord.z, 3.5);
    }

    #[test]
    fn test_parse_random_variable() {
        let var = FXListINIParser::parse_random_variable("10 20 CONSTANT").unwrap();
        assert_eq!(var.min, 10.0);
        assert_eq!(var.max, 20.0);

        let var2 = FXListINIParser::parse_random_variable("5.5 10.5 UNIFORM").unwrap();
        assert_eq!(var2.min, 5.5);
        assert_eq!(var2.max, 10.5);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(FXListINIParser::parse_bool("Yes").unwrap(), true);
        assert_eq!(FXListINIParser::parse_bool("No").unwrap(), false);
        assert_eq!(FXListINIParser::parse_bool("TRUE").unwrap(), true);
        assert_eq!(FXListINIParser::parse_bool("FALSE").unwrap(), false);
        assert_eq!(FXListINIParser::parse_bool("1").unwrap(), true);
        assert_eq!(FXListINIParser::parse_bool("0").unwrap(), false);
    }

    #[test]
    fn test_parse_shake_type() {
        assert!(matches!(
            FXListINIParser::parse_shake_type("SUBTLE").unwrap(),
            CameraShakeType::Subtle
        ));
        assert!(matches!(
            FXListINIParser::parse_shake_type("SEVERE").unwrap(),
            CameraShakeType::Severe
        ));
        assert!(matches!(
            FXListINIParser::parse_shake_type("CINE_INSANE").unwrap(),
            CameraShakeType::CineInsane
        ));
    }

    #[test]
    fn test_parse_scorch_type() {
        assert_eq!(FXListINIParser::parse_scorch_type("SCORCH_1").unwrap(), 0);
        assert_eq!(FXListINIParser::parse_scorch_type("SCORCH_4").unwrap(), 3);
        assert_eq!(FXListINIParser::parse_scorch_type("SHADOW_SCORCH").unwrap(), 4);
        assert_eq!(FXListINIParser::parse_scorch_type("RANDOM").unwrap(), -1);
    }

    #[test]
    fn test_parse_duration() {
        // 500ms at 30fps = ~15 frames
        let frames = FXListINIParser::parse_duration("500").unwrap();
        assert!(frames >= 14 && frames <= 16);

        // 0ms = 0 frames
        let frames_zero = FXListINIParser::parse_duration("0").unwrap();
        assert_eq!(frames_zero, 0);
    }

    #[test]
    fn test_parse_angle() {
        use std::f32::consts::PI;

        // 180 degrees = PI radians
        let radians = FXListINIParser::parse_angle("180").unwrap();
        assert!((radians - PI).abs() < 0.01);

        // 90 degrees = PI/2 radians
        let radians_90 = FXListINIParser::parse_angle("90").unwrap();
        assert!((radians_90 - PI / 2.0).abs() < 0.01);
    }
}

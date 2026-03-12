// FILE: beacon_client_update.rs
// Author: Matthew D. Campbell, August 2002
// Desc: Beacon client update module
// Ported from C++ to Rust

use crate::GameClient::drawable::Drawable;
use crate::GameClient::particle_sys::ParticleSystemID;
use crate::GameClient::particle_system::{ParticleSystem, ParticleSystemTemplate, ParticleSystemManager};

// Type aliases matching C++ base types
pub type UnsignedInt = u32;
pub type Bool = bool;
pub type Real = f32;
pub type Color = u32;

/// Invalid particle system ID constant
/// Matches C++ INVALID_PARTICLE_SYSTEM_ID from ParticleSys.h
pub const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemID = ParticleSystemID::MAX;

/// Seconds per logic frame (real-time)
/// Matches C++ SECONDS_PER_LOGICFRAME_REAL constant
pub const SECONDS_PER_LOGICFRAME_REAL: Real = 1.0 / 30.0;

/// Client update module data - base configuration for modules
/// Matches C++ ClientUpdateModuleData from ClientUpdateModule.h line 20
pub trait ClientUpdateModuleData {}

/// Base trait for client update modules
/// Matches C++ ClientUpdateModule from ClientUpdateModule.h line 25
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
    fn xfer_short(&mut self, value: &mut i16);
    fn xfer_user(&mut self, data: &mut [u8]);
}

/// Multi-INI field parse interface
/// Matches C++ MultiIniFieldParse from Common/INI.h
pub trait MultiIniFieldParse {
    fn add(&mut self, fields: &[FieldParse]);
}

/// Field parse structure for INI parsing
/// Matches C++ FieldParse struct
pub struct FieldParse {
    pub name: &'static str,
    pub parse_func: fn(&str) -> ParseResult,
    pub aux_data: Option<*const ()>,
    pub offset: usize,
}

/// Parse result type
pub type ParseResult = Result<Box<dyn std::any::Any>, String>;

/// Beacon client update module data
/// Configuration data specific to beacon updates
/// Matches C++ BeaconClientUpdateModuleData from BeaconClientUpdate.h line 19
pub struct BeaconClientUpdateModuleData {
    /// Number of frames between radar pulses
    /// Matches C++ BeaconClientUpdateModuleData::m_framesBetweenRadarPulses line 22
    pub frames_between_radar_pulses: UnsignedInt,

    /// Duration of radar pulse in frames
    /// Matches C++ BeaconClientUpdateModuleData::m_radarPulseDuration line 23
    pub radar_pulse_duration: UnsignedInt,
}

impl BeaconClientUpdateModuleData {
    /// Constructor
    /// Matches C++ BeaconClientUpdateModuleData::BeaconClientUpdateModuleData
    /// from BeaconClientUpdate.cpp line 19
    pub fn new() -> Self {
        Self {
            frames_between_radar_pulses: 30,
            radar_pulse_duration: 15,
        }
    }

    /// Build field parse table for INI parsing
    /// Matches C++ BeaconClientUpdateModuleData::buildFieldParse
    /// from BeaconClientUpdate.cpp line 31
    pub fn build_field_parse(&self, p: &mut dyn MultiIniFieldParse) {
        // Extend base class field parse
        // In C++ this calls ClientUpdateModuleData::buildFieldParse(p)

        // Field parse table
        // Matches C++ lines 35-40
        let data_field_parse = vec![
            // RadarPulseFrequency
            FieldParse {
                name: "RadarPulseFrequency",
                parse_func: parse_duration_unsigned_int,
                aux_data: None,
                offset: 0, // offset of frames_between_radar_pulses
            },
            // RadarPulseDuration
            FieldParse {
                name: "RadarPulseDuration",
                parse_func: parse_duration_unsigned_int,
                aux_data: None,
                offset: std::mem::size_of::<UnsignedInt>(), // offset of radar_pulse_duration
            },
        ];

        p.add(&data_field_parse);
    }
}

impl Default for BeaconClientUpdateModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientUpdateModuleData for BeaconClientUpdateModuleData {}

/// Parse duration as unsigned int from INI
/// Matches C++ INI::parseDurationUnsignedInt
fn parse_duration_unsigned_int(s: &str) -> ParseResult {
    s.parse::<UnsignedInt>()
        .map(|v| Box::new(v) as Box<dyn std::any::Any>)
        .map_err(|e| format!("Failed to parse duration: {}", e))
}

/// Beacon client update module
/// Handles beacon smoke effects and radar pulses
/// Matches C++ BeaconClientUpdate from BeaconClientUpdate.h line 33
pub struct BeaconClientUpdate {
    /// Pointer to the drawable this module is attached to
    drawable: Option<*mut Drawable>,

    /// Module configuration data
    module_data: Option<*const BeaconClientUpdateModuleData>,

    /// Particle system ID for the smoke effect
    /// Matches C++ BeaconClientUpdate::m_particleSystemID line 50
    particle_system_id: ParticleSystemID,

    /// Frame number of last radar pulse
    /// Matches C++ BeaconClientUpdate::m_lastRadarPulse line 51
    last_radar_pulse: UnsignedInt,

    /// Reference to particle system manager (for creating/finding systems)
    particle_system_manager: Option<*mut ParticleSystemManager>,

    /// Reference to radar system (for creating radar events)
    radar: Option<*mut dyn RadarInterface>,

    /// Reference to game logic (for getting current frame)
    game_logic: Option<*mut dyn GameLogicInterface>,
}

/// Radar interface for creating radar events
/// Matches C++ Radar class
pub trait RadarInterface {
    fn create_event(&mut self, pos: &Coord3D, event_type: RadarEventType, duration: Real);
}

/// Game logic interface
/// Matches C++ GameLogic class
pub trait GameLogicInterface {
    fn get_frame(&self) -> UnsignedInt;
}

/// 3D coordinate
/// Matches C++ Coord3D
#[derive(Debug, Clone, Copy)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

/// Radar event types
/// Matches C++ radar event enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarEventType {
    BeaconPulse,
}

/// ASCII string type
/// Matches C++ AsciiString
pub struct AsciiString {
    data: String,
}

impl AsciiString {
    pub fn new() -> Self {
        Self {
            data: String::new(),
        }
    }

    pub fn format(&mut self, fmt: &str, color: u32) {
        self.data = format!(fmt, color);
    }

    pub fn str(&self) -> &str {
        &self.data
    }
}

impl BeaconClientUpdate {
    /// Constructor
    /// Matches C++ BeaconClientUpdate::BeaconClientUpdate
    /// from BeaconClientUpdate.cpp line 46
    pub fn new(
        drawable: Option<*mut Drawable>,
        module_data: Option<*const BeaconClientUpdateModuleData>,
        particle_system_manager: Option<*mut ParticleSystemManager>,
        radar: Option<*mut dyn RadarInterface>,
        game_logic: Option<*mut dyn GameLogicInterface>,
    ) -> Self {
        // Get current frame from game logic
        let current_frame = if let Some(gl_ptr) = game_logic {
            unsafe { (*gl_ptr).get_frame() }
        } else {
            0
        };

        Self {
            drawable,
            module_data,
            particle_system_id: INVALID_PARTICLE_SYSTEM_ID,
            last_radar_pulse: current_frame,
            particle_system_manager,
            radar,
            game_logic,
        }
    }

    /// Hide the beacon (hide drawable, stop particle effect)
    /// Matches C++ BeaconClientUpdate::hideBeacon
    /// from BeaconClientUpdate.cpp line 104
    pub fn hide_beacon(&mut self) {
        // Hide the drawable and disable shadows
        // Matches C++ lines 106-111
        if let Some(drawable_ptr) = self.drawable {
            let draw = unsafe { &mut *drawable_ptr };
            draw.set_drawable_hidden(true);
            draw.set_shadows_enabled(false);
        }

        // Create particle system if needed
        // Matches C++ lines 113-119
        if let Some(drawable_ptr) = self.drawable {
            if self.particle_system_id == INVALID_PARTICLE_SYSTEM_ID {
                if let Some(system) = self.create_particle_system(drawable_ptr) {
                    self.particle_system_id = system.get_system_id();
                }
            }
        }

        // Clean up particle system (stop it)
        // Matches C++ lines 121-129
        if self.particle_system_id != INVALID_PARTICLE_SYSTEM_ID {
            if let Some(psm_ptr) = self.particle_system_manager {
                let psm = unsafe { &mut *psm_ptr };
                if let Some(system) = psm.find_particle_system(self.particle_system_id) {
                    system.stop();
                }
            }
        }
    }

    /// Create a particle system for the beacon smoke effect
    /// Matches C++ createParticleSystem static function
    /// from BeaconClientUpdate.cpp line 62
    fn create_particle_system(&self, drawable_ptr: *mut Drawable) -> Option<*mut ParticleSystem> {
        let draw = unsafe { &mut *drawable_ptr };

        // Get the object from the drawable
        // Matches C++ lines 66-68
        let obj = draw.get_object()?;

        // Format particle system template name based on indicator color
        // Matches C++ lines 70-71
        let mut template_name = AsciiString::new();
        template_name.format("BeaconSmoke%6.6X", 0xffffff & obj.get_indicator_color());

        // Find the particle system template
        // Matches C++ lines 72-74
        if let Some(psm_ptr) = self.particle_system_manager {
            let psm = unsafe { &mut *psm_ptr };
            let particle_template = psm.find_template(template_name.str());

            if let Some(template) = particle_template {
                // Create the particle system
                // Matches C++ lines 78-81
                let system = psm.create_particle_system(template)?;
                system.attach_to_drawable(draw);
                return Some(system);
            } else {
                // Failsafe: try to create from white template and tint it
                // Matches C++ lines 82-96
                template_name.format("BeaconSmokeFFFFFF", 0);
                let failsafe_template = psm.find_template(template_name.str())?;

                let system = psm.create_particle_system(failsafe_template)?;
                system.attach_to_drawable(draw);
                system.tint_all_colors(obj.get_indicator_color());
                return Some(system);
            }
        }

        None
    }
}

impl ClientUpdateModule for BeaconClientUpdate {
    /// The client update callback
    /// Matches C++ BeaconClientUpdate::clientUpdate
    /// from BeaconClientUpdate.cpp line 138
    fn client_update(&mut self) {
        // Get the drawable
        // Matches C++ lines 140-142
        let draw = match self.drawable {
            Some(ptr) => unsafe { &mut *ptr },
            None => return,
        };

        // Create particle system if needed
        // Matches C++ lines 144-149
        if self.particle_system_id == INVALID_PARTICLE_SYSTEM_ID {
            if let Some(system) = self.create_particle_system(self.drawable.unwrap()) {
                self.particle_system_id = unsafe { (*system).get_system_id() };
            }
        }

        // If drawable is visible, check for radar pulse
        // Matches C++ lines 151-159
        if !draw.is_drawable_effectively_hidden() {
            if let Some(module_data_ptr) = self.module_data {
                let module_data = unsafe { &*module_data_ptr };

                // Get current frame
                let current_frame = if let Some(gl_ptr) = self.game_logic {
                    unsafe { (*gl_ptr).get_frame() }
                } else {
                    0
                };

                // Check if it's time for a radar pulse
                // Matches C++ lines 154-158
                if current_frame > self.last_radar_pulse + module_data.frames_between_radar_pulses {
                    // Create radar event
                    if let Some(radar_ptr) = self.radar {
                        let radar = unsafe { &mut *radar_ptr };
                        let pos = draw.get_position();
                        let duration = module_data.radar_pulse_duration as Real * SECONDS_PER_LOGICFRAME_REAL;
                        radar.create_event(&pos, RadarEventType::BeaconPulse, duration);
                    }

                    self.last_radar_pulse = current_frame;
                }
            }
        }
    }

    /// CRC calculation for save game verification
    /// Matches C++ BeaconClientUpdate::crc
    /// from BeaconClientUpdate.cpp line 165
    fn crc(&self, xfer: &mut dyn XferInterface) {
        // Extend base class
    }

    /// Serialization/deserialization
    /// Version Info:
    /// 1: Initial version
    /// Matches C++ BeaconClientUpdate::xfer
    /// from BeaconClientUpdate.cpp line 178
    fn xfer(&mut self, xfer: &mut dyn XferInterface) {
        // Version tracking
        // Matches C++ lines 182-184
        let current_version: u32 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version);

        // Extend base class
        // Matches C++ line 187

        // Particle system ID
        // Matches C++ lines 189-190
        let mut id_bytes = self.particle_system_id.to_bytes();
        xfer.xfer_user(&mut id_bytes);
        self.particle_system_id = ParticleSystemID::from_bytes(&id_bytes);

        // Last radar pulse
        // Matches C++ lines 192-193
        xfer.xfer_unsigned_int(&mut self.last_radar_pulse);
    }

    /// Load post process - resolve references after loading
    /// Matches C++ BeaconClientUpdate::loadPostProcess
    /// from BeaconClientUpdate.cpp line 200
    fn load_post_process(&mut self) {
        // Extend base class
    }

    /// Get the drawable this module is attached to
    fn get_drawable(&mut self) -> Option<&mut Drawable> {
        self.drawable.map(|ptr| unsafe { &mut *ptr })
    }
}

// Stub implementations for ParticleSystemID to compile
// These would be properly implemented in the particle system module
impl ParticleSystemID {
    pub const MAX: ParticleSystemID = ParticleSystemID { id: u32::MAX };

    pub fn to_bytes(&self) -> Vec<u8> {
        self.id.to_le_bytes().to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let id = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Self { id }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_construction() {
        let data = BeaconClientUpdateModuleData::new();
        assert_eq!(data.frames_between_radar_pulses, 30);
        assert_eq!(data.radar_pulse_duration, 15);
    }

    #[test]
    fn test_invalid_particle_system_id() {
        assert_eq!(INVALID_PARTICLE_SYSTEM_ID.get_id(), u32::MAX);
    }

    #[test]
    fn test_seconds_per_logic_frame() {
        // Should be 1/30th of a second (30 FPS logic rate)
        assert!((SECONDS_PER_LOGICFRAME_REAL - 0.0333333).abs() < 0.0001);
    }
}

// FILE: xfer.rs ///////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002 (original C++)
// Rust port: 2025
// Desc:   The Xfer system is capable of setting up operations to work with blocks of data
//         from other subsystems.  It can work things such as file reading, file writing,
//         CRC computations etc
///////////////////////////////////////////////////////////////////////////////
// C++ Reference: /GeneralsMD/Code/GameEngine/Include/Common/Xfer.h
//                /GeneralsMD/Code/GameEngine/Source/Common/System/Xfer.cpp
///////////////////////////////////////////////////////////////////////////////

use super::geometry::{Coord3D, Matrix3D, Point2D};
use super::kind_of::KIND_OF_BIT_NAMES;
use super::snapshot::Snapshot;
use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini_upgrade::get_upgrade_center;
use crate::common::rts::science::{get_science_store, ScienceType, SCIENCE_INVALID};
use crate::common::thing::thing::KindOfType;
use crate::System::SaveGame::get_game_state;
use std::io;

fn upgrade_templates_in_serialization_order() -> Option<Vec<(String, u128)>> {
    let center = get_upgrade_center()?;
    Some(
        center
            .get_template_names()
            .into_iter()
            .filter_map(|name| {
                let template = center.find_template(&AsciiString::from(name.as_str()))?;
                Some((name.clone(), template.get_upgrade_mask()))
            })
            .collect::<Vec<_>>(),
    )
}

/// Type alias for XferVersion - matches C++ line 29
/// C++ Reference: typedef UnsignedByte XferVersion (1 byte)
/// CRITICAL: Must remain u8 for binary compatibility with C++ save files
pub type XferVersion = u8;

/// Xfer mode enumeration - matches C++ Xfer.h lines 33-42
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum XferMode {
    Invalid = 0,
    Save = 1,
    Load = 2,
    Crc = 3,
}

/// Xfer status - values must match C++ Xfer.h exactly (binary save/load compat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum XferStatus {
    Invalid = 0,
    Ok,
    Eof,
    FileNotFound,
    FileNotOpen,
    FileAlreadyOpen,
    ReadError,
    WriteError,
    ModeUnknown,
    SkipError,
    BeginEndMismatch,
    OutOfMemory,
    StringError,
    InvalidVersion,
    InvalidParameters,
    ListNotEmpty,
    UnknownString,
    ErrorUnknown,
    InvalidData, // Rust-only, NOT in C++
}

impl From<io::Error> for XferStatus {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::UnexpectedEof => XferStatus::Eof,
            io::ErrorKind::NotFound => XferStatus::FileNotFound,
            io::ErrorKind::InvalidInput => XferStatus::InvalidParameters,
            io::ErrorKind::InvalidData => XferStatus::InvalidData,
            io::ErrorKind::WriteZero | io::ErrorKind::BrokenPipe => XferStatus::WriteError,
            io::ErrorKind::WouldBlock
            | io::ErrorKind::TimedOut
            | io::ErrorKind::Interrupted
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset => XferStatus::ReadError,
            _ => XferStatus::ErrorUnknown,
        }
    }
}

/// Xfer options - matches C++ Xfer.h lines 74-80
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XferOptions {
    bits: u32,
}

impl XferOptions {
    pub const NONE: u32 = 0x00000000;
    pub const NO_POST_PROCESSING: u32 = 0x00000001;
    pub const ALL: u32 = 0xFFFFFFFF;

    pub fn new() -> Self {
        Self { bits: Self::NONE }
    }

    pub fn set(&mut self, options: u32) {
        self.bits |= options;
    }

    pub fn clear(&mut self, options: u32) {
        self.bits &= !options;
    }

    pub fn get(&self) -> u32 {
        self.bits
    }

    pub fn has(&self, option: u32) -> bool {
        (self.bits & option) != 0
    }
}

impl Default for XferOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for block size - matches C++ line 83
pub type XferBlockSize = i32;

/// Color type alias - matches C++ usage
pub type Color = i32;

/// RGB Color structure
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

/// RGBA Color structure (Real components)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBAColorReal {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

/// RGBA Color structure (Integer components)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBAColorInt {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub alpha: u32,
}

/// 2D Coordinate (integer)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

/// 3D Coordinate (integer)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ICoord3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// 2D Region (float)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region2D {
    pub lo: Point2D,
    pub hi: Point2D,
}

/// 3D Region (float)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

/// 2D Region (integer)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

/// 3D Region (integer)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IRegion3D {
    pub lo: ICoord3D,
    pub hi: ICoord3D,
}

/// Real range structure
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealRange {
    pub lo: f32,
    pub hi: f32,
}

/// Main Xfer trait - matches C++ Xfer class (Xfer.h lines 87-163)
/// This is the base interface for all data transfer operations
pub trait Xfer {
    /// Get the current xfer mode - matches C++ line 95
    fn get_xfer_mode(&self) -> XferMode;

    /// Check if reading mode
    fn is_reading(&self) -> bool {
        self.get_xfer_mode() == XferMode::Load
    }

    /// Check if writing mode
    fn is_writing(&self) -> bool {
        self.get_xfer_mode() == XferMode::Save
    }

    /// Get identifier - matches C++ line 96
    fn get_identifier(&self) -> &str;

    /// Set options - matches C++ line 99
    fn set_options(&mut self, options: u32);

    /// Clear options - matches C++ line 100
    fn clear_options(&mut self, options: u32);

    /// Get options - matches C++ line 101
    fn get_options(&self) -> u32;

    /// Open xfer - matches C++ line 102
    fn open(&mut self, identifier: &str) -> Result<(), XferStatus>;

    /// Close xfer - matches C++ line 103
    fn close(&mut self) -> Result<(), XferStatus>;

    /// Begin block - matches C++ line 104
    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus>;

    /// End block - matches C++ line 105
    fn end_block(&mut self) -> Result<(), XferStatus>;

    /// Skip data - matches C++ line 106
    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus>;

    /// Entry point for xfering a snapshot - matches C++ line 108
    fn xfer_snapshot(&mut self, snapshot: &mut Snapshot) -> Result<(), XferStatus>;

    // ============================================================================
    // Default transfer methods - these call the implementation method with the data
    // parameters. You may use the default, or derive and create new ways to xfer
    // each of these types of data (C++ Xfer.h lines 111-152)
    // ============================================================================

    /// Xfer version with validation - matches C++ Xfer.cpp lines 60-75
    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        current_version: XferVersion,
    ) -> io::Result<()> {
        // SAFETY: version is a valid reference
        unsafe {
            self.xfer_implementation(
                version as *mut XferVersion as *mut u8,
                std::mem::size_of::<XferVersion>(),
            )?;
        }

        // Sanity check: after the xfer, version data is never allowed to be higher than current version
        // C++ Xfer.cpp line 66
        if *version > current_version {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "XferVersion - Unknown version '{}' should be no higher than '{}'",
                    version, current_version
                ),
            ));
        }

        Ok(())
    }

    /// Xfer byte - matches C++ Xfer.cpp lines 51-56
    fn xfer_byte(&mut self, byte_data: &mut i8) -> io::Result<()> {
        // SAFETY: byte_data is a valid reference
        unsafe {
            self.xfer_implementation(byte_data as *mut i8 as *mut u8, std::mem::size_of::<i8>())
        }
    }

    /// Xfer unsigned byte - matches C++ Xfer.cpp lines 79-84
    fn xfer_unsigned_byte(&mut self, unsigned_byte_data: &mut u8) -> io::Result<()> {
        // SAFETY: unsigned_byte_data is a valid reference
        unsafe { self.xfer_implementation(unsigned_byte_data, std::mem::size_of::<u8>()) }
    }

    /// Xfer bool - matches C++ Xfer.cpp lines 88-93
    /// C++ Bool is typedef'd to Int (4 bytes). We must CRC/transfer 4 bytes, not 1.
    fn xfer_bool(&mut self, bool_data: &mut bool) -> io::Result<()> {
        let mut val: u32 = u32::from(*bool_data);
        // SAFETY: val is a valid reference
        unsafe {
            self.xfer_implementation(&mut val as *mut u32 as *mut u8, std::mem::size_of::<u32>())?;
        }
        *bool_data = val != 0;
        Ok(())
    }

    /// Xfer int - matches C++ Xfer.cpp lines 97-102
    fn xfer_int(&mut self, int_data: &mut i32) -> io::Result<()> {
        // SAFETY: int_data is a valid reference
        unsafe {
            self.xfer_implementation(int_data as *mut i32 as *mut u8, std::mem::size_of::<i32>())
        }
    }

    /// Xfer i32 (alias for xfer_int)
    fn xfer_i32(&mut self, i32_data: &mut i32) -> io::Result<()> {
        self.xfer_int(i32_data)
    }

    /// Xfer int64 - matches C++ Xfer.cpp lines 106-111
    fn xfer_int64(&mut self, int64_data: &mut i64) -> io::Result<()> {
        // SAFETY: int64_data is a valid reference
        unsafe {
            self.xfer_implementation(
                int64_data as *mut i64 as *mut u8,
                std::mem::size_of::<i64>(),
            )
        }
    }

    /// Xfer unsigned int - matches C++ Xfer.cpp lines 115-120
    fn xfer_unsigned_int(&mut self, unsigned_int_data: &mut u32) -> io::Result<()> {
        // SAFETY: unsigned_int_data is a valid reference
        unsafe {
            self.xfer_implementation(
                unsigned_int_data as *mut u32 as *mut u8,
                std::mem::size_of::<u32>(),
            )
        }
    }

    /// Xfer u32 (alias for xfer_unsigned_int)
    fn xfer_u32(&mut self, u32_data: &mut u32) -> io::Result<()> {
        self.xfer_unsigned_int(u32_data)
    }

    /// Xfer f32 (alias for xfer_real)
    fn xfer_f32(&mut self, f32_data: &mut f32) -> io::Result<()> {
        self.xfer_real(f32_data)
    }

    /// Xfer string (alias for xfer_ascii_string)
    fn xfer_string(&mut self, string_data: &mut String) -> io::Result<()> {
        self.xfer_ascii_string(string_data)
    }

    /// Xfer u64
    fn xfer_u64(&mut self, u64_data: &mut u64) -> io::Result<()> {
        // SAFETY: u64_data is a valid reference
        unsafe {
            self.xfer_implementation(u64_data as *mut u64 as *mut u8, std::mem::size_of::<u64>())
        }
    }

    /// Xfer u128 - for upgrade masks
    fn xfer_u128(&mut self, u128_data: &mut u128) -> io::Result<()> {
        // SAFETY: u128_data is a valid reference
        unsafe {
            self.xfer_implementation(
                u128_data as *mut u128 as *mut u8,
                std::mem::size_of::<u128>(),
            )
        }
    }

    /// Xfer short - matches C++ Xfer.cpp lines 124-129
    fn xfer_short(&mut self, short_data: &mut i16) -> io::Result<()> {
        // SAFETY: short_data is a valid reference
        unsafe {
            self.xfer_implementation(
                short_data as *mut i16 as *mut u8,
                std::mem::size_of::<i16>(),
            )
        }
    }

    /// Xfer unsigned short - matches C++ Xfer.cpp lines 133-138
    fn xfer_unsigned_short(&mut self, unsigned_short_data: &mut u16) -> io::Result<()> {
        // SAFETY: unsigned_short_data is a valid reference
        unsafe {
            self.xfer_implementation(
                unsigned_short_data as *mut u16 as *mut u8,
                std::mem::size_of::<u16>(),
            )
        }
    }

    /// Xfer real (float) - matches C++ Xfer.cpp lines 142-147
    fn xfer_real(&mut self, real_data: &mut f32) -> io::Result<()> {
        // SAFETY: real_data is a valid reference
        unsafe {
            self.xfer_implementation(real_data as *mut f32 as *mut u8, std::mem::size_of::<f32>())
        }
    }

    fn xfer_map_name(&mut self, map_name_data: &mut String) -> io::Result<()> {
        match self.get_xfer_mode() {
            XferMode::Save => {
                // PARITY_NOTE: C++ converts runtime map paths to portable save paths before writing.
                let mut portable_name =
                    get_game_state().real_map_path_to_portable_map_path(map_name_data.as_str());
                self.xfer_ascii_string(&mut portable_name)
            }
            XferMode::Load => {
                // PARITY_NOTE: C++ reads the portable bytes first, then translates back to the real path.
                self.xfer_ascii_string(map_name_data)?;
                *map_name_data =
                    get_game_state().portable_map_path_to_real_map_path(map_name_data.as_str());
                Ok(())
            }
            XferMode::Crc => {
                // PARITY_NOTE: The C++ implementation intentionally does nothing in CRC mode here.
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Xfer marker label - purely for readability, explicitly discarded on load
    /// Matches C++ Xfer.cpp lines 176-178
    fn xfer_marker_label(&mut self, _ascii_string_data: &str) -> io::Result<()> {
        // This is purely for readability purposes - it is explicitly discarded on load
        Ok(())
    }

    /// Xfer ASCII string - matches C++ Xfer.cpp lines 167-172
    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> io::Result<()>;

    /// Xfer Unicode string - matches C++ Xfer.cpp lines 182-187
    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> io::Result<()>;

    /// Xfer Coord3D - matches C++ Xfer.cpp lines 191-198
    fn xfer_coord_3d(&mut self, coord_3d: &mut Coord3D) -> io::Result<()> {
        self.xfer_real(&mut coord_3d.x)?;
        self.xfer_real(&mut coord_3d.y)?;
        self.xfer_real(&mut coord_3d.z)?;
        Ok(())
    }

    /// Xfer ICoord3D - matches C++ Xfer.cpp lines 202-209
    fn xfer_icoord_3d(&mut self, icoord_3d: &mut ICoord3D) -> io::Result<()> {
        self.xfer_int(&mut icoord_3d.x)?;
        self.xfer_int(&mut icoord_3d.y)?;
        self.xfer_int(&mut icoord_3d.z)?;
        Ok(())
    }

    /// Xfer Region3D - matches C++ Xfer.cpp lines 213-219
    fn xfer_region_3d(&mut self, region_3d: &mut Region3D) -> io::Result<()> {
        self.xfer_coord_3d(&mut region_3d.lo)?;
        self.xfer_coord_3d(&mut region_3d.hi)?;
        Ok(())
    }

    /// Xfer IRegion3D - matches C++ Xfer.cpp lines 223-229
    fn xfer_iregion_3d(&mut self, iregion_3d: &mut IRegion3D) -> io::Result<()> {
        self.xfer_icoord_3d(&mut iregion_3d.lo)?;
        self.xfer_icoord_3d(&mut iregion_3d.hi)?;
        Ok(())
    }

    /// Xfer Coord2D - matches C++ Xfer.cpp lines 233-239
    fn xfer_coord_2d(&mut self, coord_2d: &mut Point2D) -> io::Result<()> {
        self.xfer_real(&mut coord_2d.x)?;
        self.xfer_real(&mut coord_2d.y)?;
        Ok(())
    }

    /// Xfer ICoord2D - matches C++ Xfer.cpp lines 243-249
    fn xfer_icoord_2d(&mut self, icoord_2d: &mut ICoord2D) -> io::Result<()> {
        self.xfer_int(&mut icoord_2d.x)?;
        self.xfer_int(&mut icoord_2d.y)?;
        Ok(())
    }

    /// Xfer Region2D - matches C++ Xfer.cpp lines 253-259
    fn xfer_region_2d(&mut self, region_2d: &mut Region2D) -> io::Result<()> {
        self.xfer_coord_2d(&mut region_2d.lo)?;
        self.xfer_coord_2d(&mut region_2d.hi)?;
        Ok(())
    }

    /// Xfer IRegion2D - matches C++ Xfer.cpp lines 263-269
    fn xfer_iregion_2d(&mut self, iregion_2d: &mut IRegion2D) -> io::Result<()> {
        self.xfer_icoord_2d(&mut iregion_2d.lo)?;
        self.xfer_icoord_2d(&mut iregion_2d.hi)?;
        Ok(())
    }

    /// Xfer RealRange - matches C++ Xfer.cpp lines 273-279
    fn xfer_real_range(&mut self, real_range: &mut RealRange) -> io::Result<()> {
        self.xfer_real(&mut real_range.lo)?;
        self.xfer_real(&mut real_range.hi)?;
        Ok(())
    }

    /// Xfer Color - matches C++ Xfer.cpp lines 283-288
    fn xfer_color(&mut self, color: &mut Color) -> io::Result<()> {
        // SAFETY: color is a valid reference
        unsafe {
            self.xfer_implementation(color as *mut Color as *mut u8, std::mem::size_of::<Color>())
        }
    }

    /// Xfer RGBColor - matches C++ Xfer.cpp lines 292-299
    fn xfer_rgb_color(&mut self, rgb_color: &mut RGBColor) -> io::Result<()> {
        self.xfer_real(&mut rgb_color.red)?;
        self.xfer_real(&mut rgb_color.green)?;
        self.xfer_real(&mut rgb_color.blue)?;
        Ok(())
    }

    /// Xfer RGBAColorReal - matches C++ Xfer.cpp lines 303-311
    fn xfer_rgba_color_real(&mut self, rgba_color_real: &mut RGBAColorReal) -> io::Result<()> {
        self.xfer_real(&mut rgba_color_real.red)?;
        self.xfer_real(&mut rgba_color_real.green)?;
        self.xfer_real(&mut rgba_color_real.blue)?;
        self.xfer_real(&mut rgba_color_real.alpha)?;
        Ok(())
    }

    /// Xfer RGBAColorInt - matches C++ Xfer.cpp lines 315-323
    fn xfer_rgba_color_int(&mut self, rgba_color_int: &mut RGBAColorInt) -> io::Result<()> {
        self.xfer_unsigned_int(&mut rgba_color_int.red)?;
        self.xfer_unsigned_int(&mut rgba_color_int.green)?;
        self.xfer_unsigned_int(&mut rgba_color_int.blue)?;
        self.xfer_unsigned_int(&mut rgba_color_int.alpha)?;
        Ok(())
    }

    /// Xfer user data - matches C++ Xfer.cpp lines 809-814
    ///
    /// # Safety
    /// The caller must ensure that `data` points to a valid buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_user(&mut self, data: *mut u8, data_size: usize) -> io::Result<()> {
        self.xfer_implementation(data, data_size)
    }

    /// Xfer object ID (ObjectID is serialized as unsigned int in modern paths)
    /// Matches C++ Xfer.cpp lines 357-362
    fn xfer_object_id(&mut self, object_id: &mut u32) -> io::Result<()> {
        // SAFETY: object_id is a valid reference
        unsafe {
            self.xfer_implementation(object_id as *mut u32 as *mut u8, std::mem::size_of::<u32>())
        }
    }

    /// Xfer drawable ID (DrawableID is serialized same size as ObjectID)
    /// Matches C++ Xfer.cpp lines 366-371
    fn xfer_drawable_id(&mut self, drawable_id: &mut u32) -> io::Result<()> {
        // SAFETY: drawable_id is a valid reference
        unsafe {
            self.xfer_implementation(
                drawable_id as *mut u32 as *mut u8,
                std::mem::size_of::<u32>(),
            )
        }
    }

    fn xfer_stl_object_id_list(&mut self, object_id_list_data: &mut Vec<u32>) -> io::Result<()> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        let mut list_count = object_id_list_data.len() as u16;
        self.xfer_unsigned_short(&mut list_count)?;

        match self.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // PARITY_NOTE: C++ writes version, u16 count, then raw ObjectIDs in list order.
                for object_id in object_id_list_data.iter().copied() {
                    let mut object_id = object_id;
                    self.xfer_object_id(&mut object_id)?;
                }
                Ok(())
            }
            XferMode::Load => {
                if !object_id_list_data.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Xfer::xfer_stl_object_id_list - object list should be empty before loading",
                    ));
                }

                for _ in 0..list_count {
                    let mut object_id = 0u32;
                    self.xfer_object_id(&mut object_id)?;
                    object_id_list_data.push(object_id);
                }
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "xfer_stl_object_id_list - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                ),
            )),
        }
    }

    fn xfer_stl_int_list(&mut self, int_list_data: &mut Vec<i32>) -> io::Result<()> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        let mut list_count = int_list_data.len() as u16;
        self.xfer_unsigned_short(&mut list_count)?;

        match self.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // PARITY_NOTE: C++ uses the same versioned list encoding for Int values.
                for int_value in int_list_data.iter().copied() {
                    let mut int_value = int_value;
                    self.xfer_int(&mut int_value)?;
                }
                Ok(())
            }
            XferMode::Load => {
                if !int_list_data.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Xfer::xfer_stl_int_list - int list should be empty before loading",
                    ));
                }

                for _ in 0..list_count {
                    let mut int_value = 0i32;
                    self.xfer_int(&mut int_value)?;
                    int_list_data.push(int_value);
                }
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "xfer_stl_int_list - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                ),
            )),
        }
    }

    fn xfer_science_type(&mut self, science: &mut ScienceType) -> io::Result<()> {
        let mut science_name = String::new();

        match self.get_xfer_mode() {
            XferMode::Save => {
                // PARITY_NOTE: Save/load uses science internal names so NameKey reorderings stay compatible.
                science_name = get_science_store()
                    .map(|store| store.get_internal_name_for_science(*science).to_string())
                    .unwrap_or_default();
                self.xfer_ascii_string(&mut science_name)
            }
            XferMode::Load => {
                self.xfer_ascii_string(&mut science_name)?;
                let resolved = get_science_store()
                    .map(|store| store.get_science_from_internal_name(science_name.as_str()))
                    .unwrap_or(SCIENCE_INVALID);
                if resolved == SCIENCE_INVALID {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("xfer_science_type - Unknown science '{}'", science_name),
                    ));
                }
                *science = resolved;
                Ok(())
            }
            XferMode::Crc => self.xfer_int(science),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "xfer_science_type - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                ),
            )),
        }
    }

    fn xfer_science_vec(&mut self, science_vec: &mut Vec<ScienceType>) -> io::Result<()> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        let mut count = science_vec.len() as u16;
        self.xfer_unsigned_short(&mut count)?;

        match self.get_xfer_mode() {
            XferMode::Save => {
                for science in science_vec.iter().copied() {
                    let mut science = science;
                    self.xfer_science_type(&mut science)?;
                }
                Ok(())
            }
            XferMode::Load => {
                // PARITY_NOTE: C++ clears pre-seeded entries here instead of failing the load.
                if !science_vec.is_empty() {
                    science_vec.clear();
                }

                for _ in 0..count {
                    let mut science = SCIENCE_INVALID;
                    self.xfer_science_type(&mut science)?;
                    science_vec.push(science);
                }
                Ok(())
            }
            XferMode::Crc => {
                for science in science_vec.iter().copied() {
                    let mut science = science;
                    self.xfer_int(&mut science)?;
                }
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "xfer_science_vec - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                ),
            )),
        }
    }

    fn xfer_kind_of(&mut self, kind_of_data: &mut KindOfType) -> io::Result<()> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        match self.get_xfer_mode() {
            XferMode::Save => {
                // PARITY_NOTE: C++ persists the single-bit KindOf name so enum reorderings remain safe.
                let mut kind_of_name = KIND_OF_BIT_NAMES
                    .get(*kind_of_data as usize)
                    .copied()
                    .unwrap_or_default()
                    .to_string();
                self.xfer_ascii_string(&mut kind_of_name)
            }
            XferMode::Load => {
                let mut kind_of_name = String::new();
                self.xfer_ascii_string(&mut kind_of_name)?;
                if let Some(bit) = KIND_OF_BIT_NAMES
                    .iter()
                    .position(|name| *name == kind_of_name)
                {
                    *kind_of_data = bit as KindOfType;
                }
                Ok(())
            }
            XferMode::Crc => self.xfer_unsigned_int(kind_of_data),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "xfer_kind_of - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                ),
            )),
        }
    }

    fn xfer_upgrade_mask(&mut self, upgrade_mask_data: &mut u128) -> io::Result<()> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        match self.get_xfer_mode() {
            XferMode::Save => {
                let upgrade_templates =
                    upgrade_templates_in_serialization_order().unwrap_or_default();
                let mut selected_names = Vec::new();

                // PARITY_NOTE: C++ writes each set upgrade bit as a name string instead of raw bits.
                for (upgrade_name, upgrade_mask) in upgrade_templates {
                    if (*upgrade_mask_data & upgrade_mask) == upgrade_mask {
                        selected_names.push(upgrade_name);
                    }
                }

                let mut count = selected_names.len() as u16;
                self.xfer_unsigned_short(&mut count)?;
                for mut upgrade_name in selected_names {
                    self.xfer_ascii_string(&mut upgrade_name)?;
                }
                Ok(())
            }
            XferMode::Load => {
                let mut count = 0u16;
                self.xfer_unsigned_short(&mut count)?;
                *upgrade_mask_data = 0;

                let upgrade_templates = upgrade_templates_in_serialization_order();
                for _ in 0..count {
                    let mut upgrade_name = String::new();
                    self.xfer_ascii_string(&mut upgrade_name)?;

                    let Some(upgrade_mask) = upgrade_templates.as_ref().and_then(|templates| {
                        templates
                            .iter()
                            .find_map(|(name, mask)| (name == &upgrade_name).then_some(*mask))
                    }) else {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "Xfer::xfer_upgrade_mask - Unknown upgrade '{}'",
                                upgrade_name
                            ),
                        ));
                    };

                    *upgrade_mask_data |= upgrade_mask;
                }
                Ok(())
            }
            XferMode::Crc => self.xfer_u128(upgrade_mask_data),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "xfer_upgrade_mask - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                ),
            )),
        }
    }

    /// Xfer Matrix3D - matches C++ Xfer.cpp lines 818-843
    fn xfer_matrix_3d(&mut self, mtx: &mut Matrix3D) -> io::Result<()> {
        // This deserves a version number
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        // Xfer all matrix components (4x4 matrix, but only first 3 rows used)
        // Matches C++ lines 825-842
        for i in 0..3 {
            for j in 0..4 {
                self.xfer_real(&mut mtx.m[i][j])?;
            }
        }

        Ok(())
    }

    /// Xfer STL vector of integers - similar pattern to C++ xferSTLIntList
    fn xfer_vec_int(&mut self, int_vec_data: &mut Vec<i32>) -> io::Result<()> {
        // Version (matches C++ pattern lines 484-486)
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        // Xfer the count of the vector (matches C++ line 489-490)
        let mut list_count = int_vec_data.len() as u16;
        self.xfer_unsigned_short(&mut list_count)?;

        // Xfer vector data (matches C++ pattern lines 493-536)
        match self.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // Save all values
                for value in int_vec_data.iter() {
                    let mut val = *value;
                    self.xfer_int(&mut val)?;
                }
            }
            XferMode::Load => {
                // Sanity: the list should be empty before we transfer more data into it
                if !int_vec_data.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Xfer::xfer_vec_int - vector should be empty before loading",
                    ));
                }

                // Read all values
                for _ in 0..list_count {
                    let mut val = 0i32;
                    self.xfer_int(&mut val)?;
                    int_vec_data.push(val);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "xfer_vec_int - Unknown xfer mode {:?}",
                        self.get_xfer_mode()
                    ),
                ));
            }
        }

        Ok(())
    }

    /// Xfer STL vector of unsigned integers - mirrors xfer_vec_int with u32 payloads.
    fn xfer_vec_unsigned_int(&mut self, uint_vec_data: &mut Vec<u32>) -> io::Result<()> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        let mut list_count = uint_vec_data.len() as u16;
        self.xfer_unsigned_short(&mut list_count)?;

        match self.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for value in uint_vec_data.iter() {
                    let mut val = *value;
                    self.xfer_unsigned_int(&mut val)?;
                }
            }
            XferMode::Load => {
                if !uint_vec_data.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Xfer::xfer_vec_unsigned_int - vector should be empty before loading",
                    ));
                }

                for _ in 0..list_count {
                    let mut val = 0u32;
                    self.xfer_unsigned_int(&mut val)?;
                    uint_vec_data.push(val);
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!(
                        "xfer_vec_unsigned_int - Unknown xfer mode {:?}",
                        self.get_xfer_mode()
                    ),
                ));
            }
        }

        Ok(())
    }

    // ============================================================================
    // Implementation method - this is the actual xfer implementation that each
    // derived class should implement (C++ Xfer.h line 157)
    // ============================================================================

    /// The actual xfer implementation that each derived class should implement
    /// C++ Reference: Xfer.h line 157
    ///
    /// # Safety
    /// The caller must ensure that `data` points to a valid buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_implementation(&mut self, data: *mut u8, data_size: usize) -> io::Result<()>;
}

/// Helper trait for objects that can be xfered
pub trait Xferable {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::ini_upgrade::{get_upgrade_center_mut, initialize_upgrade_center};
    use crate::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn test_xfer_options() {
        let mut options = XferOptions::new();
        assert_eq!(options.get(), XferOptions::NONE);

        options.set(XferOptions::NO_POST_PROCESSING);
        assert!(options.has(XferOptions::NO_POST_PROCESSING));

        options.clear(XferOptions::NO_POST_PROCESSING);
        assert!(!options.has(XferOptions::NO_POST_PROCESSING));
    }

    #[test]
    fn test_xfer_mode() {
        assert_eq!(XferMode::Invalid as u32, 0);
        assert_eq!(XferMode::Save as u32, 1);
        assert_eq!(XferMode::Load as u32, 2);
        assert_eq!(XferMode::Crc as u32, 3);
    }

    #[test]
    fn test_geometry_types() {
        let mut coord = Coord3D::new(1.0, 2.0, 3.0);
        assert_eq!(coord.x, 1.0);
        assert_eq!(coord.y, 2.0);
        assert_eq!(coord.z, 3.0);

        let mut icoord = ICoord3D { x: 1, y: 2, z: 3 };
        assert_eq!(icoord.x, 1);
        assert_eq!(icoord.y, 2);
        assert_eq!(icoord.z, 3);

        let region = Region3D {
            lo: Coord3D::new(0.0, 0.0, 0.0),
            hi: Coord3D::new(10.0, 10.0, 10.0),
        };
        assert_eq!(region.lo.x, 0.0);
        assert_eq!(region.hi.x, 10.0);
    }

    #[test]
    fn xfer_upgrade_mask_saves_names_in_upgrade_center_order() {
        initialize_upgrade_center();

        let (mask_a, mask_c) = {
            let mut center = get_upgrade_center_mut().unwrap();
            let mask_a = center
                .get_or_create_template(&AsciiString::from("XferOrderUpgradeA"))
                .get_upgrade_mask();
            let _mask_b = center
                .get_or_create_template(&AsciiString::from("XferOrderUpgradeB"))
                .get_upgrade_mask();
            let mask_c = center
                .get_or_create_template(&AsciiString::from("XferOrderUpgradeC"))
                .get_upgrade_mask();
            (mask_a, mask_c)
        };

        let mut saved = Vec::new();
        {
            let cursor = Cursor::new(&mut saved);
            let mut xfer = XferSave::new(cursor, 0);
            let mut mask = mask_a | mask_c;
            xfer.xfer_upgrade_mask(&mut mask).unwrap();
        }

        let mut expected = vec![1, 2, 0];
        expected.push("XferOrderUpgradeC".len() as u8);
        expected.extend_from_slice(b"XferOrderUpgradeC");
        expected.push("XferOrderUpgradeA".len() as u8);
        expected.extend_from_slice(b"XferOrderUpgradeA");

        assert_eq!(saved, expected);
    }
}

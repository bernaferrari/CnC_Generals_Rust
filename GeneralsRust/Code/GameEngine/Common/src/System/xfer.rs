// FILE: xfer.rs
// Author: Ported from C++ (Colin Day, February 2002)
// Desc: The Xfer system is capable of setting up operations to work with blocks of data
//       from other subsystems. It can work things such as file reading, file writing,
//       CRC computations etc

use crate::common::ini::ini_upgrade::get_upgrade_center;
use crate::common::rts::science::{get_science_store, ScienceType, SCIENCE_INVALID};
use crate::common::system::geometry::Matrix3D;
use crate::common::system::kind_of::KIND_OF_BIT_NAMES;
use crate::common::thing::thing::KindOfType;
use std::fmt;

fn get_upgrade_names_sorted() -> Vec<String> {
    get_upgrade_center()
        .map(|center| {
            let mut names = center
                .get_template_names()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            names.sort();
            names
        })
        .unwrap_or_default()
}

/// Xfer version type
///
/// Matches the C++ `typedef UnsignedByte XferVersion;` semantics.
pub type XferVersion = u8;

/// Transfer block size type
pub type XferBlockSize = i32;

/// File position type for save/load operations
pub type XferFilePos = i64;

// ------------------------------------------------------------------------------------------------
// Xfer Mode Enumeration
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XferMode {
    Invalid = 0,
    Save,
    Load,
    Crc,
}

// ------------------------------------------------------------------------------------------------
// Xfer Status Enumeration
// ------------------------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XferStatus {
    Invalid = 0,
    Ok,                // all is green and good
    Eof,               // end of file encountered
    FileNotFound,      // requested file does not exist
    FileNotOpen,       // file was not open
    FileAlreadyOpen,   // this xfer is already open
    ReadError,         // error reading from file
    WriteError,        // error writing to file
    ModeUnknown,       // unknown xfer mode
    SkipError,         // error skipping file
    BeginEndMismatch,  // mismatched pair calls of begin/end block
    OutOfMemory,       // out of memory
    StringError,       // error with strings
    InvalidVersion,    // invalid version encountered
    InvalidParameters, // invalid parameters
    InvalidData,       // invalid data encountered
    ListNotEmpty,      // trying to xfer into a list that should be empty, but isn't
    UnknownBlock,      // unrecognized block identifier
    UnknownString,     // unrecognized string value
    ErrorUnknown,      // unknown error
}

impl fmt::Display for XferStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for XferStatus {}

// ------------------------------------------------------------------------------------------------
// Xfer Options Flags
// ------------------------------------------------------------------------------------------------
pub mod xfer_options {
    pub const NONE: u32 = 0x00000000;
    pub const NO_POST_PROCESSING: u32 = 0x00000001;
    pub const ALL: u32 = 0xFFFFFFFF;
}

// ------------------------------------------------------------------------------------------------
// Xfer Trait - Base interface for all transfer operations
// ------------------------------------------------------------------------------------------------
pub trait Xfer {
    /// Get the current xfer mode
    fn get_xfer_mode(&self) -> XferMode;

    /// Check if this xfer is in read mode (loading from disk)
    fn is_reading(&self) -> bool {
        self.get_xfer_mode() == XferMode::Load
    }

    /// Get the identifier for this xfer
    fn get_identifier(&self) -> &str;

    /// Set options
    fn set_options(&mut self, options: u32);

    /// Clear options
    fn clear_options(&mut self, options: u32);

    /// Get options
    fn get_options(&self) -> u32;

    /// Open xfer with identifier
    fn open(&mut self, identifier: String) -> Result<(), XferStatus>;

    /// Close xfer
    fn close(&mut self) -> Result<(), XferStatus>;

    /// Begin a data block
    fn begin_block(&mut self) -> Result<XferBlockSize, XferStatus>;

    /// End a data block
    fn end_block(&mut self) -> Result<(), XferStatus>;

    /// Skip data
    fn skip(&mut self, data_size: i32) -> Result<(), XferStatus>;

    /// Transfer a snapshot
    fn xfer_snapshot(&mut self, snapshot: &mut dyn Snapshot) -> Result<(), XferStatus>;

    // ------------------------------------------------------------------------------------------------
    // Default transfer methods for primitive types
    // ------------------------------------------------------------------------------------------------

    /// Transfer version with validation
    fn xfer_version(
        &mut self,
        version_data: &mut XferVersion,
        current_version: XferVersion,
    ) -> Result<(), XferStatus> {
        // SAFETY: version_data is a valid reference, so the pointer is valid
        unsafe {
            self.xfer_implementation(
                version_data as *mut XferVersion as *mut u8,
                std::mem::size_of::<XferVersion>(),
            )
        }?;

        // Sanity check: version data is never allowed to be higher than the current version
        if *version_data > current_version {
            eprintln!(
                "XferVersion - Unknown version '{}' should be no higher than '{}'",
                *version_data, current_version
            );
            return Err(XferStatus::InvalidVersion);
        }

        Ok(())
    }

    /// Transfer byte
    fn xfer_byte(&mut self, byte_data: &mut i8) -> Result<(), XferStatus> {
        // SAFETY: byte_data is a valid reference
        unsafe {
            self.xfer_implementation(byte_data as *mut i8 as *mut u8, std::mem::size_of::<i8>())
        }
    }

    /// Transfer unsigned byte
    fn xfer_unsigned_byte(&mut self, unsigned_byte_data: &mut u8) -> Result<(), XferStatus> {
        // SAFETY: unsigned_byte_data is a valid reference
        unsafe {
            self.xfer_implementation(unsigned_byte_data as *mut u8, std::mem::size_of::<u8>())
        }
    }

    /// Transfer bool
    fn xfer_bool(&mut self, bool_data: &mut bool) -> Result<(), XferStatus> {
        // SAFETY: bool_data is a valid reference
        unsafe {
            self.xfer_implementation(
                bool_data as *mut bool as *mut u8,
                std::mem::size_of::<bool>(),
            )
        }
    }

    /// Transfer int
    fn xfer_int(&mut self, int_data: &mut i32) -> Result<(), XferStatus> {
        // SAFETY: int_data is a valid reference
        unsafe {
            self.xfer_implementation(int_data as *mut i32 as *mut u8, std::mem::size_of::<i32>())
        }
    }

    /// Transfer int64
    fn xfer_int64(&mut self, int64_data: &mut i64) -> Result<(), XferStatus> {
        // SAFETY: int64_data is a valid reference
        unsafe {
            self.xfer_implementation(
                int64_data as *mut i64 as *mut u8,
                std::mem::size_of::<i64>(),
            )
        }
    }

    /// Transfer unsigned int
    fn xfer_unsigned_int(&mut self, unsigned_int_data: &mut u32) -> Result<(), XferStatus> {
        // SAFETY: unsigned_int_data is a valid reference
        unsafe {
            self.xfer_implementation(
                unsigned_int_data as *mut u32 as *mut u8,
                std::mem::size_of::<u32>(),
            )
        }
    }

    /// Transfer u32 (alias for xfer_unsigned_int)
    fn xfer_u32(&mut self, u32_data: &mut u32) -> Result<(), XferStatus> {
        self.xfer_unsigned_int(u32_data)
    }

    /// Transfer f32 (alias for xfer_real)
    fn xfer_f32(&mut self, f32_data: &mut f32) -> Result<(), XferStatus> {
        self.xfer_real(f32_data)
    }

    /// Transfer string (alias for xfer_ascii_string)
    fn xfer_string(&mut self, string_data: &mut String) -> Result<(), XferStatus> {
        self.xfer_ascii_string(string_data)
    }

    /// Transfer short
    fn xfer_short(&mut self, short_data: &mut i16) -> Result<(), XferStatus> {
        // SAFETY: short_data is a valid reference
        unsafe {
            self.xfer_implementation(
                short_data as *mut i16 as *mut u8,
                std::mem::size_of::<i16>(),
            )
        }
    }

    /// Transfer unsigned short
    fn xfer_unsigned_short(&mut self, unsigned_short_data: &mut u16) -> Result<(), XferStatus> {
        // SAFETY: unsigned_short_data is a valid reference
        unsafe {
            self.xfer_implementation(
                unsigned_short_data as *mut u16 as *mut u8,
                std::mem::size_of::<u16>(),
            )
        }
    }

    /// Transfer real (float)
    fn xfer_real(&mut self, real_data: &mut f32) -> Result<(), XferStatus> {
        // SAFETY: real_data is a valid reference
        unsafe {
            self.xfer_implementation(real_data as *mut f32 as *mut u8, std::mem::size_of::<f32>())
        }
    }

    /// Transfer marker label (for readability, explicitly discarded on load)
    fn xfer_marker_label(&mut self, _ascii_string_data: &str) -> Result<(), XferStatus> {
        // This is purely for readability purposes - it is explicitly discarded on load
        Ok(())
    }

    /// Transfer ASCII string
    fn xfer_ascii_string(&mut self, ascii_string_data: &mut String) -> Result<(), XferStatus>;

    /// Transfer Unicode string
    fn xfer_unicode_string(&mut self, unicode_string_data: &mut String) -> Result<(), XferStatus>;

    /// Transfer 3D coordinate
    fn xfer_coord_3d(&mut self, coord: &mut Coord3D) -> Result<(), XferStatus> {
        self.xfer_real(&mut coord.x)?;
        self.xfer_real(&mut coord.y)?;
        self.xfer_real(&mut coord.z)?;
        Ok(())
    }

    /// Transfer integer 3D coordinate
    fn xfer_icoord_3d(&mut self, coord: &mut ICoord3D) -> Result<(), XferStatus> {
        self.xfer_int(&mut coord.x)?;
        self.xfer_int(&mut coord.y)?;
        self.xfer_int(&mut coord.z)?;
        Ok(())
    }

    /// Transfer 3D region
    fn xfer_region_3d(&mut self, region: &mut Region3D) -> Result<(), XferStatus> {
        self.xfer_coord_3d(&mut region.lo)?;
        self.xfer_coord_3d(&mut region.hi)?;
        Ok(())
    }

    /// Transfer integer 3D region
    fn xfer_iregion_3d(&mut self, region: &mut IRegion3D) -> Result<(), XferStatus> {
        self.xfer_icoord_3d(&mut region.lo)?;
        self.xfer_icoord_3d(&mut region.hi)?;
        Ok(())
    }

    /// Transfer 2D coordinate
    fn xfer_coord_2d(&mut self, coord: &mut Coord2D) -> Result<(), XferStatus> {
        self.xfer_real(&mut coord.x)?;
        self.xfer_real(&mut coord.y)?;
        Ok(())
    }

    /// Transfer integer 2D coordinate
    fn xfer_icoord_2d(&mut self, coord: &mut ICoord2D) -> Result<(), XferStatus> {
        self.xfer_int(&mut coord.x)?;
        self.xfer_int(&mut coord.y)?;
        Ok(())
    }

    /// Transfer 2D region
    fn xfer_region_2d(&mut self, region: &mut Region2D) -> Result<(), XferStatus> {
        self.xfer_coord_2d(&mut region.lo)?;
        self.xfer_coord_2d(&mut region.hi)?;
        Ok(())
    }

    /// Transfer integer 2D region
    fn xfer_iregion_2d(&mut self, region: &mut IRegion2D) -> Result<(), XferStatus> {
        self.xfer_icoord_2d(&mut region.lo)?;
        self.xfer_icoord_2d(&mut region.hi)?;
        Ok(())
    }

    /// Transfer real range
    fn xfer_real_range(&mut self, range: &mut RealRange) -> Result<(), XferStatus> {
        self.xfer_real(&mut range.lo)?;
        self.xfer_real(&mut range.hi)?;
        Ok(())
    }

    /// Transfer RGB color
    fn xfer_rgb_color(&mut self, color: &mut RGBColor) -> Result<(), XferStatus> {
        self.xfer_real(&mut color.red)?;
        self.xfer_real(&mut color.green)?;
        self.xfer_real(&mut color.blue)?;
        Ok(())
    }

    /// Transfer RGBA color (real)
    fn xfer_rgba_color_real(&mut self, color: &mut RGBAColorReal) -> Result<(), XferStatus> {
        self.xfer_real(&mut color.red)?;
        self.xfer_real(&mut color.green)?;
        self.xfer_real(&mut color.blue)?;
        self.xfer_real(&mut color.alpha)?;
        Ok(())
    }

    /// Transfer RGBA color (int)
    fn xfer_rgba_color_int(&mut self, color: &mut RGBAColorInt) -> Result<(), XferStatus> {
        self.xfer_unsigned_int(&mut color.red)?;
        self.xfer_unsigned_int(&mut color.green)?;
        self.xfer_unsigned_int(&mut color.blue)?;
        self.xfer_unsigned_int(&mut color.alpha)?;
        Ok(())
    }

    /// Transfer object ID
    fn xfer_object_id(&mut self, object_id: &mut ObjectID) -> Result<(), XferStatus> {
        // SAFETY: object_id is a valid reference
        unsafe {
            self.xfer_implementation(
                object_id as *mut ObjectID as *mut u8,
                std::mem::size_of::<ObjectID>(),
            )
        }
    }

    /// Transfer drawable ID
    fn xfer_drawable_id(&mut self, drawable_id: &mut DrawableID) -> Result<(), XferStatus> {
        // SAFETY: drawable_id is a valid reference
        unsafe {
            self.xfer_implementation(
                drawable_id as *mut DrawableID as *mut u8,
                std::mem::size_of::<DrawableID>(),
            )
        }
    }

    /// Transfer STL vector of object IDs
    fn xfer_stl_object_id_vector(
        &mut self,
        object_id_vector: &mut Vec<ObjectID>,
    ) -> Result<(), XferStatus> {
        // Version this data structure
        let current_version: XferVersion = 1;
        let mut version = current_version;
        self.xfer_version(&mut version, current_version)?;

        // Transfer count
        let mut list_count = object_id_vector.len() as u16;
        self.xfer_unsigned_short(&mut list_count)?;

        match self.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // Save all IDs
                for object_id in object_id_vector.iter() {
                    let mut id = *object_id;
                    self.xfer_object_id(&mut id)?;
                }
            }
            XferMode::Load => {
                // List should be empty before loading
                if !object_id_vector.is_empty() {
                    eprintln!("Xfer::xfer_stl_object_id_vector - object vector should be empty before loading");
                    return Err(XferStatus::ListNotEmpty);
                }

                // Read all IDs
                for _ in 0..list_count {
                    let mut object_id: ObjectID = 0;
                    self.xfer_object_id(&mut object_id)?;
                    object_id_vector.push(object_id);
                }
            }
            XferMode::Invalid => {
                eprintln!("xfer_stl_object_id_vector - Unknown xfer mode");
                return Err(XferStatus::ModeUnknown);
            }
        }

        Ok(())
    }

    /// Transfer user data (raw bytes)
    ///
    /// # Safety
    /// The caller must ensure that `data` points to a valid buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_user(&mut self, data: *mut u8, data_size: usize) -> Result<(), XferStatus> {
        self.xfer_implementation(data, data_size)
    }

    /// Transfer a single ScienceType value as a string for enum reorder safety.
    /// Matches C++ Xfer.cpp xferScienceType lines 548-587.
    fn xfer_science_type(&mut self, science: &mut ScienceType) -> Result<(), XferStatus> {
        match self.get_xfer_mode() {
            XferMode::Save => {
                let mut science_name = get_science_store()
                    .map(|store| store.get_internal_name_for_science(*science).to_string())
                    .unwrap_or_default();
                self.xfer_ascii_string(&mut science_name)
            }
            XferMode::Load => {
                let mut science_name = String::new();
                self.xfer_ascii_string(&mut science_name)?;
                let resolved = get_science_store()
                    .map(|store| store.get_science_from_internal_name(science_name.as_str()))
                    .unwrap_or(SCIENCE_INVALID);
                if resolved == SCIENCE_INVALID {
                    eprintln!("xfer_science_type - Unknown science '{}'", science_name);
                    return Err(XferStatus::UnknownString);
                }
                *science = resolved;
                Ok(())
            }
            XferMode::Crc => {
                // C++ uses xferImplementation with sizeof(ScienceType)
                unsafe {
                    self.xfer_implementation(
                        science as *mut ScienceType as *mut u8,
                        std::mem::size_of::<ScienceType>(),
                    )
                }
            }
            _ => {
                eprintln!(
                    "xfer_science_type - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                );
                Err(XferStatus::ModeUnknown)
            }
        }
    }

    /// Transfer a vector of ScienceType values as strings for enum reorder safety.
    /// Matches C++ Xfer.cpp xferScienceVec lines 591-651.
    fn xfer_science_vec(&mut self, science_vec: &mut Vec<ScienceType>) -> Result<(), XferStatus> {
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
                // PARITY_NOTE: C++ clears pre-seeded entries instead of failing the load.
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
                // C++ uses xferImplementation with sizeof(ScienceType) per element
                for science in science_vec.iter() {
                    let mut science = *science;
                    unsafe {
                        self.xfer_implementation(
                            &mut science as *mut ScienceType as *mut u8,
                            std::mem::size_of::<ScienceType>(),
                        )?;
                    }
                }
                Ok(())
            }
            _ => {
                eprintln!(
                    "xfer_science_vec - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                );
                Err(XferStatus::ModeUnknown)
            }
        }
    }

    /// Transfer a KindOfType as a string so enum reorders remain safe.
    /// Matches C++ Xfer.cpp xferKindOf lines 659-704.
    fn xfer_kind_of(&mut self, kind_of_data: &mut KindOfType) -> Result<(), XferStatus> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        match self.get_xfer_mode() {
            XferMode::Save => {
                // PARITY_NOTE: C++ persists the single-bit KindOf name for enum reorder safety.
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
            XferMode::Crc => {
                // C++ uses xferImplementation with sizeof(KindOfType)
                unsafe {
                    self.xfer_implementation(
                        kind_of_data as *mut KindOfType as *mut u8,
                        std::mem::size_of::<KindOfType>(),
                    )
                }
            }
            _ => {
                eprintln!(
                    "xfer_kind_of - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                );
                Err(XferStatus::ModeUnknown)
            }
        }
    }

    /// Transfer an upgrade mask as string names for enum reorder safety.
    /// Matches C++ Xfer.cpp xferUpgradeMask lines 708-805.
    fn xfer_upgrade_mask(&mut self, upgrade_mask_data: &mut u128) -> Result<(), XferStatus> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        match self.get_xfer_mode() {
            XferMode::Save => {
                // Collect upgrade names in deterministic order, matching C++ linked-list iteration
                let upgrade_names = get_upgrade_names_sorted();
                let mut selected_names = Vec::new();

                // PARITY_NOTE: C++ writes each set upgrade bit as a name string instead of raw bits.
                for (index, upgrade_name) in upgrade_names.iter().enumerate() {
                    if (*upgrade_mask_data & (1u128 << index)) != 0 {
                        selected_names.push(upgrade_name.clone());
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

                let upgrade_names = get_upgrade_names_sorted();
                for _ in 0..count {
                    let mut upgrade_name = String::new();
                    self.xfer_ascii_string(&mut upgrade_name)?;

                    let Some(index) = upgrade_names.iter().position(|name| name == &upgrade_name)
                    else {
                        eprintln!(
                            "Xfer::xfer_upgrade_mask - Unknown upgrade '{}'",
                            upgrade_name
                        );
                        return Err(XferStatus::UnknownString);
                    };

                    *upgrade_mask_data |= 1u128 << index;
                }
                Ok(())
            }
            XferMode::Crc => {
                // C++ uses xferImplementation with sizeof(UpgradeMaskType)
                unsafe {
                    self.xfer_implementation(
                        upgrade_mask_data as *mut u128 as *mut u8,
                        std::mem::size_of::<u128>(),
                    )
                }
            }
            _ => {
                eprintln!(
                    "xfer_upgrade_mask - Unknown xfer mode {:?}",
                    self.get_xfer_mode()
                );
                Err(XferStatus::ModeUnknown)
            }
        }
    }

    /// Transfer a Matrix3D (4x3 transform matrix, 12 Reals).
    /// Matches C++ Xfer.cpp xferMatrix3D lines 818-843.
    fn xfer_matrix_3d(&mut self, mtx: &mut Matrix3D) -> Result<(), XferStatus> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        self.xfer_version(&mut version, CURRENT_VERSION)?;

        // C++ xfers 3 rows of 4 floats (Vector4 each): (*mtx)[0], (*mtx)[1], (*mtx)[2]
        for i in 0..3 {
            for j in 0..4 {
                self.xfer_real(&mut mtx.m[i][j])?;
            }
        }

        Ok(())
    }

    /// The actual xfer implementation that each derived struct should implement
    ///
    /// # Safety
    /// The caller must ensure that `data` points to a valid buffer
    /// of at least `data_size` bytes.
    unsafe fn xfer_implementation(
        &mut self,
        data: *mut u8,
        data_size: usize,
    ) -> Result<(), XferStatus>;
}

// ------------------------------------------------------------------------------------------------
// Snapshot Trait - Base interface for data structures that can be saved/loaded
// ------------------------------------------------------------------------------------------------
pub trait Snapshot: Send {
    /// Run the "light" CRC check on this data structure
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus>;

    /// Run save, load, or deep CRC check on this data structure
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus>;

    /// Post process phase for loading save games
    fn load_post_process(&mut self) -> Result<(), XferStatus>;
}

// ------------------------------------------------------------------------------------------------
// Supporting data structures (matching C++ types)
// ------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ICoord3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IRegion3D {
    pub lo: ICoord3D,
    pub hi: ICoord3D,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Region2D {
    pub lo: Coord2D,
    pub hi: Coord2D,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RealRange {
    pub lo: f32,
    pub hi: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RGBColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RGBAColorReal {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RGBAColorInt {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub alpha: u32,
}

// Type aliases for game-specific IDs
pub type ObjectID = u32;
pub type DrawableID = u32;

// ------------------------------------------------------------------------------------------------
// Base Xfer implementation structure
// ------------------------------------------------------------------------------------------------
pub struct XferBase {
    pub options: u32,
    pub xfer_mode: XferMode,
    pub identifier: String,
}

impl XferBase {
    pub fn new(xfer_mode: XferMode) -> Self {
        Self {
            options: xfer_options::NONE,
            xfer_mode,
            identifier: String::new(),
        }
    }

    pub fn open_base(&mut self, identifier: String) {
        self.identifier = identifier;
    }
}

// Helper functions for bit operations
pub fn bit_set(flags: &mut u32, bits: u32) {
    *flags |= bits;
}

pub fn bit_clear(flags: &mut u32, bits: u32) {
    *flags &= !bits;
}

pub fn bit_test(flags: u32, bits: u32) -> bool {
    (flags & bits) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xfer_version_is_byte_sized() {
        assert_eq!(std::mem::size_of::<XferVersion>(), 1);
    }
}

// FILE: xfer.rs
// Author: Ported from C++ (Colin Day, February 2002)
// Desc: The Xfer system is capable of setting up operations to work with blocks of data
//       from other subsystems. It can work things such as file reading, file writing,
//       CRC computations etc

use std::fmt;

/// Xfer version type
pub type XferVersion = u32;

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

use crate::game_logic::{KindOf, ObjectId, Team};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

/// Xfer operation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XferMode {
    Save,
    Load,
    Crc,
}

/// Xfer status codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XferStatus {
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
}

/// Xfer options
#[derive(Debug, Default)]
pub struct XferOptions {
    pub no_post_processing: bool,
}

/// Version type for save compatibility
pub type XferVersion = u8;

/// Block size for structured data
pub type XferBlockSize = i32;

/// Core Xfer trait for data serialization
pub trait Xfer {
    fn get_mode(&self) -> XferMode;
    fn get_identifier(&self) -> &str;

    fn set_options(&mut self, options: XferOptions);
    fn get_options(&self) -> &XferOptions;

    fn open(&mut self, identifier: &str) -> SaveLoadResult<()>;
    fn close(&mut self) -> SaveLoadResult<()>;

    fn begin_block(&mut self) -> SaveLoadResult<XferBlockSize>;
    fn end_block(&mut self) -> SaveLoadResult<()>;

    fn skip(&mut self, data_size: i32) -> SaveLoadResult<()>;

    // Version handling
    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        current_version: XferVersion,
    ) -> SaveLoadResult<()>;

    // Basic data types
    fn xfer_bool(&mut self, data: &mut bool) -> SaveLoadResult<()>;
    fn xfer_u8(&mut self, data: &mut u8) -> SaveLoadResult<()>;
    fn xfer_i8(&mut self, data: &mut i8) -> SaveLoadResult<()>;
    fn xfer_u16(&mut self, data: &mut u16) -> SaveLoadResult<()>;
    fn xfer_i16(&mut self, data: &mut i16) -> SaveLoadResult<()>;
    fn xfer_u32(&mut self, data: &mut u32) -> SaveLoadResult<()>;
    fn xfer_i32(&mut self, data: &mut i32) -> SaveLoadResult<()>;
    fn xfer_u64(&mut self, data: &mut u64) -> SaveLoadResult<()>;
    fn xfer_i64(&mut self, data: &mut i64) -> SaveLoadResult<()>;
    fn xfer_f32(&mut self, data: &mut f32) -> SaveLoadResult<()>;
    fn xfer_f64(&mut self, data: &mut f64) -> SaveLoadResult<()>;

    // String types
    fn xfer_string(&mut self, data: &mut String) -> SaveLoadResult<()>;
    fn xfer_marker_label(&mut self, label: &str) -> SaveLoadResult<()>;

    // Vector math types
    fn xfer_vec2(&mut self, data: &mut Vec2) -> SaveLoadResult<()>;
    fn xfer_vec3(&mut self, data: &mut Vec3) -> SaveLoadResult<()>;
    fn xfer_vec4(&mut self, data: &mut Vec4) -> SaveLoadResult<()>;
    fn xfer_mat3(&mut self, data: &mut Mat3) -> SaveLoadResult<()>;
    fn xfer_mat4(&mut self, data: &mut Mat4) -> SaveLoadResult<()>;

    // Game-specific types
    fn xfer_object_id(&mut self, data: &mut ObjectId) -> SaveLoadResult<()>;
    fn xfer_team(&mut self, data: &mut Team) -> SaveLoadResult<()>;
    fn xfer_kind_of(&mut self, data: &mut KindOf) -> SaveLoadResult<()>;

    // Collections
    fn xfer_vec<T>(&mut self, data: &mut Vec<T>) -> SaveLoadResult<()>
    where
        T: XferData,
        Self: Sized;

    fn xfer_hashmap<K, V>(&mut self, data: &mut HashMap<K, V>) -> SaveLoadResult<()>
    where
        K: XferData + std::hash::Hash + Eq,
        V: XferData,
        Self: Sized;

    // Raw data
    fn xfer_raw(&mut self, data: &mut [u8]) -> SaveLoadResult<()>;

    // Object-safe collection methods (for use with dyn Xfer)
    // For common XferData types, we provide concrete methods
    fn xfer_vec_u32(&mut self, data: &mut Vec<u32>) -> SaveLoadResult<()>;
    fn xfer_vec_string(&mut self, data: &mut Vec<String>) -> SaveLoadResult<()>;

    // Implementation specific
    fn xfer_implementation(&mut self, data: &mut [u8]) -> SaveLoadResult<()>;
}

/// Trait for types that can be transferred via Xfer
pub trait XferData {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()>;
}

/// Implement XferData for basic types
macro_rules! impl_xfer_data_basic {
    ($type:ty, $method:ident) => {
        impl XferData for $type {
            fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
                xfer.$method(self)
            }
        }
    };
}

impl_xfer_data_basic!(bool, xfer_bool);
impl_xfer_data_basic!(u8, xfer_u8);
impl_xfer_data_basic!(i8, xfer_i8);
impl_xfer_data_basic!(u16, xfer_u16);
impl_xfer_data_basic!(i16, xfer_i16);
impl_xfer_data_basic!(u32, xfer_u32);
impl_xfer_data_basic!(i32, xfer_i32);
impl_xfer_data_basic!(u64, xfer_u64);
impl_xfer_data_basic!(i64, xfer_i64);
impl_xfer_data_basic!(f32, xfer_f32);
impl_xfer_data_basic!(f64, xfer_f64);
impl_xfer_data_basic!(String, xfer_string);
impl_xfer_data_basic!(Vec2, xfer_vec2);
impl_xfer_data_basic!(Vec3, xfer_vec3);
impl_xfer_data_basic!(Vec4, xfer_vec4);
impl_xfer_data_basic!(Mat3, xfer_mat3);
impl_xfer_data_basic!(Mat4, xfer_mat4);
impl_xfer_data_basic!(ObjectId, xfer_object_id);
impl_xfer_data_basic!(Team, xfer_team);
impl_xfer_data_basic!(KindOf, xfer_kind_of);

// Note: Generic Vec<T> XferData implementation is commented out
// because it can't be used with trait objects. Use specific implementations
// like xfer_vec_u32, xfer_vec_string, etc., or call the generic xfer_vec
// method directly on concrete Xfer types.

// Note: Generic HashMap<K, V> XferData implementation is commented out
// because it can't be used with trait objects. Use manual serialization
// or call the generic xfer_hashmap method directly on concrete Xfer types.

/// Binary file Xfer implementation for saving
pub struct XferSave<W: Write + Seek> {
    mode: XferMode,
    identifier: String,
    options: XferOptions,
    writer: Option<W>,
    block_stack: Vec<u64>, // File positions for block size backfill
}

impl<W: Write + Seek> XferSave<W> {
    pub fn new(writer: W) -> Self {
        Self {
            mode: XferMode::Save,
            identifier: String::new(),
            options: XferOptions::default(),
            writer: Some(writer),
            block_stack: Vec::new(),
        }
    }
}

impl<W: Write + Seek> Xfer for XferSave<W> {
    fn get_mode(&self) -> XferMode {
        self.mode
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: XferOptions) {
        self.options = options;
    }

    fn get_options(&self) -> &XferOptions {
        &self.options
    }

    fn open(&mut self, identifier: &str) -> SaveLoadResult<()> {
        self.identifier = identifier.to_string();
        Ok(())
    }

    fn close(&mut self) -> SaveLoadResult<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush().map_err(SaveLoadError::Io)?;
        }
        Ok(())
    }

    fn begin_block(&mut self) -> SaveLoadResult<XferBlockSize> {
        let writer = self.writer.as_mut().ok_or(SaveLoadError::InvalidFormat)?;

        // Remember current position for backfill
        let pos = writer.stream_position().map_err(SaveLoadError::Io)?;
        self.block_stack.push(pos);

        // Write placeholder block size
        let placeholder: u32 = 0;
        writer
            .write_all(&placeholder.to_le_bytes())
            .map_err(SaveLoadError::Io)?;

        Ok(0)
    }

    fn end_block(&mut self) -> SaveLoadResult<()> {
        let writer = self.writer.as_mut().ok_or(SaveLoadError::InvalidFormat)?;
        let start_pos = self.block_stack.pop().ok_or(SaveLoadError::InvalidFormat)?;

        // Calculate block size
        let end_pos = writer.stream_position().map_err(SaveLoadError::Io)?;
        let block_size = (end_pos - start_pos - 4) as u32; // Subtract size field itself

        // Seek back and write actual size
        writer
            .seek(SeekFrom::Start(start_pos))
            .map_err(SaveLoadError::Io)?;
        writer
            .write_all(&block_size.to_le_bytes())
            .map_err(SaveLoadError::Io)?;

        // Seek back to end
        writer
            .seek(SeekFrom::Start(end_pos))
            .map_err(SaveLoadError::Io)?;

        Ok(())
    }

    fn skip(&mut self, _data_size: i32) -> SaveLoadResult<()> {
        // Skip is no-op for save
        Ok(())
    }

    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        _current_version: XferVersion,
    ) -> SaveLoadResult<()> {
        self.xfer_u8(version)
    }

    fn xfer_bool(&mut self, data: &mut bool) -> SaveLoadResult<()> {
        let mut byte = if *data { 1u8 } else { 0u8 };
        self.xfer_u8(&mut byte)
    }

    fn xfer_u8(&mut self, data: &mut u8) -> SaveLoadResult<()> {
        self.xfer_implementation(std::slice::from_mut(data))
    }

    fn xfer_i8(&mut self, data: &mut i8) -> SaveLoadResult<()> {
        self.xfer_implementation(unsafe {
            std::slice::from_raw_parts_mut(data as *mut i8 as *mut u8, 1)
        })
    }

    fn xfer_u16(&mut self, data: &mut u16) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_i16(&mut self, data: &mut i16) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_u32(&mut self, data: &mut u32) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_i32(&mut self, data: &mut i32) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_u64(&mut self, data: &mut u64) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_i64(&mut self, data: &mut i64) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_f32(&mut self, data: &mut f32) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_f64(&mut self, data: &mut f64) -> SaveLoadResult<()> {
        let bytes = data.to_le_bytes();
        self.xfer_implementation(&mut bytes.as_slice().to_vec())
    }

    fn xfer_string(&mut self, data: &mut String) -> SaveLoadResult<()> {
        let bytes = data.as_bytes();
        let mut len = bytes.len() as u32;
        self.xfer_u32(&mut len)?;
        self.xfer_implementation(&mut bytes.to_vec())
    }

    fn xfer_marker_label(&mut self, _label: &str) -> SaveLoadResult<()> {
        // Marker labels are discarded in save mode
        Ok(())
    }

    fn xfer_vec2(&mut self, data: &mut Vec2) -> SaveLoadResult<()> {
        self.xfer_f32(&mut data.x)?;
        self.xfer_f32(&mut data.y)
    }

    fn xfer_vec3(&mut self, data: &mut Vec3) -> SaveLoadResult<()> {
        self.xfer_f32(&mut data.x)?;
        self.xfer_f32(&mut data.y)?;
        self.xfer_f32(&mut data.z)
    }

    fn xfer_vec4(&mut self, data: &mut Vec4) -> SaveLoadResult<()> {
        self.xfer_f32(&mut data.x)?;
        self.xfer_f32(&mut data.y)?;
        self.xfer_f32(&mut data.z)?;
        self.xfer_f32(&mut data.w)
    }

    fn xfer_mat3(&mut self, data: &mut Mat3) -> SaveLoadResult<()> {
        let array = data.to_cols_array();
        for i in 0..9 {
            let mut val = array[i];
            self.xfer_f32(&mut val)?;
        }
        Ok(())
    }

    fn xfer_mat4(&mut self, data: &mut Mat4) -> SaveLoadResult<()> {
        let array = data.to_cols_array();
        for i in 0..16 {
            let mut val = array[i];
            self.xfer_f32(&mut val)?;
        }
        Ok(())
    }

    fn xfer_object_id(&mut self, data: &mut ObjectId) -> SaveLoadResult<()> {
        self.xfer_u32(&mut data.0)
    }

    fn xfer_team(&mut self, data: &mut Team) -> SaveLoadResult<()> {
        let mut variant = match *data {
            Team::GLA => 0u8,
            Team::USA => 1u8,
            Team::China => 2u8,
            Team::Neutral => 3u8,
        };
        self.xfer_u8(&mut variant)
    }

    fn xfer_kind_of(&mut self, data: &mut KindOf) -> SaveLoadResult<()> {
        let mut variant = match *data {
            KindOf::Structure => 0u8,
            KindOf::Infantry => 1u8,
            KindOf::Vehicle => 2u8,
            KindOf::Aircraft => 3u8,
            KindOf::Projectile => 4u8,
            KindOf::Resource => 5u8,
            KindOf::Selectable => 6u8,
            KindOf::Attackable => 7u8,
            KindOf::CommandCenter => 8u8,
            KindOf::SupplyCenter => 9u8,
            KindOf::PowerPlant => 10u8,
            KindOf::Harvestable => 11u8,
            KindOf::Worker => 12u8,
            KindOf::Hero => 13u8,
        };
        self.xfer_u8(&mut variant)
    }

    fn xfer_vec<T>(&mut self, data: &mut Vec<T>) -> SaveLoadResult<()>
    where
        T: XferData,
    {
        let mut len = data.len() as u32;
        self.xfer_u32(&mut len)?;

        for item in data.iter_mut() {
            item.xfer(self)?;
        }

        Ok(())
    }

    fn xfer_hashmap<K, V>(&mut self, data: &mut HashMap<K, V>) -> SaveLoadResult<()>
    where
        K: XferData + std::hash::Hash + Eq,
        V: XferData,
    {
        let mut len = data.len() as u32;
        self.xfer_u32(&mut len)?;

        // Serialize each key-value pair
        for (key, value) in data.iter_mut() {
            // Need to clone to get mutable access for xfer
            // This is safe because we're only saving
            let mut key_copy = unsafe { std::ptr::read(key as *const K) };
            key_copy.xfer(self)?;
            value.xfer(self)?;
            std::mem::forget(key_copy); // Prevent double-free
        }

        Ok(())
    }

    fn xfer_raw(&mut self, data: &mut [u8]) -> SaveLoadResult<()> {
        self.xfer_implementation(data)
    }

    fn xfer_vec_u32(&mut self, data: &mut Vec<u32>) -> SaveLoadResult<()> {
        let mut len = data.len() as u32;
        self.xfer_u32(&mut len)?;

        for item in data.iter_mut() {
            self.xfer_u32(item)?;
        }
        Ok(())
    }

    fn xfer_vec_string(&mut self, data: &mut Vec<String>) -> SaveLoadResult<()> {
        let mut len = data.len() as u32;
        self.xfer_u32(&mut len)?;

        for item in data.iter_mut() {
            self.xfer_string(item)?;
        }
        Ok(())
    }

    fn xfer_implementation(&mut self, data: &mut [u8]) -> SaveLoadResult<()> {
        if let Some(writer) = &mut self.writer {
            writer.write_all(data).map_err(SaveLoadError::Io)?;
            Ok(())
        } else {
            Err(SaveLoadError::InvalidFormat)
        }
    }
}

/// Binary file Xfer implementation for loading
pub struct XferLoad<R: Read + Seek> {
    mode: XferMode,
    identifier: String,
    options: XferOptions,
    reader: Option<R>,
}

impl<R: Read + Seek> XferLoad<R> {
    pub fn new(reader: R) -> Self {
        Self {
            mode: XferMode::Load,
            identifier: String::new(),
            options: XferOptions::default(),
            reader: Some(reader),
        }
    }
}

impl<R: Read + Seek> Xfer for XferLoad<R> {
    fn get_mode(&self) -> XferMode {
        self.mode
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: XferOptions) {
        self.options = options;
    }

    fn get_options(&self) -> &XferOptions {
        &self.options
    }

    fn open(&mut self, identifier: &str) -> SaveLoadResult<()> {
        self.identifier = identifier.to_string();
        Ok(())
    }

    fn close(&mut self) -> SaveLoadResult<()> {
        self.reader = None;
        Ok(())
    }

    fn begin_block(&mut self) -> SaveLoadResult<XferBlockSize> {
        let reader = self.reader.as_mut().ok_or(SaveLoadError::InvalidFormat)?;

        // Read block size
        let mut size_bytes = [0u8; 4];
        reader
            .read_exact(&mut size_bytes)
            .map_err(SaveLoadError::Io)?;
        let block_size = u32::from_le_bytes(size_bytes) as i32;

        Ok(block_size)
    }

    fn end_block(&mut self) -> SaveLoadResult<()> {
        // End block is no-op for loading
        Ok(())
    }

    fn skip(&mut self, data_size: i32) -> SaveLoadResult<()> {
        let reader = self.reader.as_mut().ok_or(SaveLoadError::InvalidFormat)?;

        reader
            .seek(SeekFrom::Current(data_size as i64))
            .map_err(SaveLoadError::Io)?;

        Ok(())
    }

    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        current_version: XferVersion,
    ) -> SaveLoadResult<()> {
        self.xfer_u8(version)?;
        if *version > current_version {
            return Err(SaveLoadError::VersionMismatch {
                expected: current_version as u32,
                actual: *version as u32,
            });
        }
        Ok(())
    }

    fn xfer_bool(&mut self, data: &mut bool) -> SaveLoadResult<()> {
        let mut byte = 0u8;
        self.xfer_u8(&mut byte)?;
        *data = byte != 0;
        Ok(())
    }

    fn xfer_u8(&mut self, data: &mut u8) -> SaveLoadResult<()> {
        self.xfer_implementation(std::slice::from_mut(data))
    }

    fn xfer_i8(&mut self, data: &mut i8) -> SaveLoadResult<()> {
        self.xfer_implementation(unsafe {
            std::slice::from_raw_parts_mut(data as *mut i8 as *mut u8, 1)
        })
    }

    fn xfer_u16(&mut self, data: &mut u16) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 2];
        self.xfer_implementation(&mut bytes)?;
        *data = u16::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_i16(&mut self, data: &mut i16) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 2];
        self.xfer_implementation(&mut bytes)?;
        *data = i16::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_u32(&mut self, data: &mut u32) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 4];
        self.xfer_implementation(&mut bytes)?;
        *data = u32::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_i32(&mut self, data: &mut i32) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 4];
        self.xfer_implementation(&mut bytes)?;
        *data = i32::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_u64(&mut self, data: &mut u64) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 8];
        self.xfer_implementation(&mut bytes)?;
        *data = u64::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_i64(&mut self, data: &mut i64) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 8];
        self.xfer_implementation(&mut bytes)?;
        *data = i64::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_f32(&mut self, data: &mut f32) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 4];
        self.xfer_implementation(&mut bytes)?;
        *data = f32::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_f64(&mut self, data: &mut f64) -> SaveLoadResult<()> {
        let mut bytes = [0u8; 8];
        self.xfer_implementation(&mut bytes)?;
        *data = f64::from_le_bytes(bytes);
        Ok(())
    }

    fn xfer_string(&mut self, data: &mut String) -> SaveLoadResult<()> {
        let mut len = 0u32;
        self.xfer_u32(&mut len)?;

        let mut bytes = vec![0u8; len as usize];
        self.xfer_implementation(&mut bytes)?;

        *data =
            String::from_utf8(bytes).map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(())
    }

    fn xfer_marker_label(&mut self, _label: &str) -> SaveLoadResult<()> {
        // Marker labels are ignored during load
        Ok(())
    }

    fn xfer_vec2(&mut self, data: &mut Vec2) -> SaveLoadResult<()> {
        self.xfer_f32(&mut data.x)?;
        self.xfer_f32(&mut data.y)
    }

    fn xfer_vec3(&mut self, data: &mut Vec3) -> SaveLoadResult<()> {
        self.xfer_f32(&mut data.x)?;
        self.xfer_f32(&mut data.y)?;
        self.xfer_f32(&mut data.z)
    }

    fn xfer_vec4(&mut self, data: &mut Vec4) -> SaveLoadResult<()> {
        self.xfer_f32(&mut data.x)?;
        self.xfer_f32(&mut data.y)?;
        self.xfer_f32(&mut data.z)?;
        self.xfer_f32(&mut data.w)
    }

    fn xfer_mat3(&mut self, data: &mut Mat3) -> SaveLoadResult<()> {
        let mut array = [0.0f32; 9];
        for val in array.iter_mut() {
            self.xfer_f32(val)?;
        }
        *data = Mat3::from_cols_array(&array);
        Ok(())
    }

    fn xfer_mat4(&mut self, data: &mut Mat4) -> SaveLoadResult<()> {
        let mut array = [0.0f32; 16];
        for val in array.iter_mut() {
            self.xfer_f32(val)?;
        }
        *data = Mat4::from_cols_array(&array);
        Ok(())
    }

    fn xfer_object_id(&mut self, data: &mut ObjectId) -> SaveLoadResult<()> {
        self.xfer_u32(&mut data.0)
    }

    fn xfer_team(&mut self, data: &mut Team) -> SaveLoadResult<()> {
        let mut variant = 0u8;
        self.xfer_u8(&mut variant)?;

        *data = match variant {
            0 => Team::GLA,
            1 => Team::USA,
            2 => Team::China,
            3 => Team::Neutral,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid team variant: {}",
                    variant
                )))
            }
        };

        Ok(())
    }

    fn xfer_kind_of(&mut self, data: &mut KindOf) -> SaveLoadResult<()> {
        let mut variant = 0u8;
        self.xfer_u8(&mut variant)?;

        *data = match variant {
            0 => KindOf::Structure,
            1 => KindOf::Infantry,
            2 => KindOf::Vehicle,
            3 => KindOf::Aircraft,
            4 => KindOf::Projectile,
            5 => KindOf::Resource,
            6 => KindOf::Selectable,
            7 => KindOf::Attackable,
            8 => KindOf::CommandCenter,
            9 => KindOf::SupplyCenter,
            10 => KindOf::PowerPlant,
            11 => KindOf::Harvestable,
            12 => KindOf::Worker,
            13 => KindOf::Hero,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid KindOf variant: {}",
                    variant
                )))
            }
        };

        Ok(())
    }

    fn xfer_vec<T>(&mut self, data: &mut Vec<T>) -> SaveLoadResult<()>
    where
        T: XferData,
    {
        let mut len = 0u32;
        self.xfer_u32(&mut len)?;

        data.clear();
        data.reserve(len as usize);

        for _ in 0..len {
            let mut item: T = unsafe { std::mem::zeroed() };
            item.xfer(self)?;
            data.push(item);
        }

        Ok(())
    }

    fn xfer_hashmap<K, V>(&mut self, data: &mut HashMap<K, V>) -> SaveLoadResult<()>
    where
        K: XferData + std::hash::Hash + Eq,
        V: XferData,
    {
        let mut len = 0u32;
        self.xfer_u32(&mut len)?;

        data.clear();
        data.reserve(len as usize);

        for _ in 0..len {
            let mut key: K = unsafe { std::mem::zeroed() };
            let mut value: V = unsafe { std::mem::zeroed() };

            key.xfer(self)?;
            value.xfer(self)?;

            data.insert(key, value);
        }

        Ok(())
    }

    fn xfer_raw(&mut self, data: &mut [u8]) -> SaveLoadResult<()> {
        self.xfer_implementation(data)
    }

    fn xfer_vec_u32(&mut self, data: &mut Vec<u32>) -> SaveLoadResult<()> {
        let mut len = 0u32;
        self.xfer_u32(&mut len)?;

        data.clear();
        data.reserve(len as usize);

        for _ in 0..len {
            let mut item = 0u32;
            self.xfer_u32(&mut item)?;
            data.push(item);
        }

        Ok(())
    }

    fn xfer_vec_string(&mut self, data: &mut Vec<String>) -> SaveLoadResult<()> {
        let mut len = 0u32;
        self.xfer_u32(&mut len)?;

        data.clear();
        data.reserve(len as usize);

        for _ in 0..len {
            let mut item = String::new();
            self.xfer_string(&mut item)?;
            data.push(item);
        }

        Ok(())
    }

    fn xfer_implementation(&mut self, data: &mut [u8]) -> SaveLoadResult<()> {
        if let Some(reader) = &mut self.reader {
            reader.read_exact(data).map_err(SaveLoadError::Io)?;
            Ok(())
        } else {
            Err(SaveLoadError::InvalidFormat)
        }
    }
}

use crate::game_logic::{KindOf, ObjectId, Team};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::system::xfer as common_xfer;
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::io::{Read, Seek, Write};

pub type XferMode = common_xfer::XferMode;
pub type XferStatus = common_xfer::XferStatus;
pub type XferVersion = common_xfer::XferVersion;
pub type XferBlockSize = common_xfer::XferBlockSize;

/// Local options shape kept for Main save/load compatibility.
#[derive(Debug, Clone, Copy, Default)]
pub struct XferOptions {
    pub no_post_processing: bool,
}

/// Core Xfer trait used by Main snapshot/save code.
///
/// The implementation authority is Common's Xfer stack; this trait keeps Main's
/// existing API surface while delegating behavior.
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

    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        current_version: XferVersion,
    ) -> SaveLoadResult<()>;

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

    fn xfer_string(&mut self, data: &mut String) -> SaveLoadResult<()>;
    fn xfer_marker_label(&mut self, label: &str) -> SaveLoadResult<()>;

    fn xfer_vec2(&mut self, data: &mut Vec2) -> SaveLoadResult<()>;
    fn xfer_vec3(&mut self, data: &mut Vec3) -> SaveLoadResult<()>;
    fn xfer_vec4(&mut self, data: &mut Vec4) -> SaveLoadResult<()>;
    fn xfer_mat3(&mut self, data: &mut Mat3) -> SaveLoadResult<()>;
    fn xfer_mat4(&mut self, data: &mut Mat4) -> SaveLoadResult<()>;

    fn xfer_object_id(&mut self, data: &mut ObjectId) -> SaveLoadResult<()>;
    fn xfer_team(&mut self, data: &mut Team) -> SaveLoadResult<()>;
    fn xfer_kind_of(&mut self, data: &mut KindOf) -> SaveLoadResult<()>;

    fn xfer_vec<T>(&mut self, data: &mut Vec<T>) -> SaveLoadResult<()>
    where
        T: XferData,
        Self: Sized;

    fn xfer_hashmap<K, V>(&mut self, data: &mut HashMap<K, V>) -> SaveLoadResult<()>
    where
        K: XferData + std::hash::Hash + Eq,
        V: XferData,
        Self: Sized;

    fn xfer_raw(&mut self, data: &mut [u8]) -> SaveLoadResult<()>;
    fn xfer_vec_u32(&mut self, data: &mut Vec<u32>) -> SaveLoadResult<()>;
    fn xfer_vec_string(&mut self, data: &mut Vec<String>) -> SaveLoadResult<()>;

    fn xfer_implementation(&mut self, data: &mut [u8]) -> SaveLoadResult<()>;
}

pub trait XferData {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()>;
}

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

fn map_status(status: XferStatus) -> SaveLoadError {
    SaveLoadError::Corrupted(format!("xfer error: {status:?}"))
}

fn apply_options<X: CommonXfer>(inner: &mut X, options: XferOptions) {
    if options.no_post_processing {
        CommonXfer::set_options(inner, common_xfer::XferOptions::NO_POST_PROCESSING);
    } else {
        CommonXfer::clear_options(inner, common_xfer::XferOptions::NO_POST_PROCESSING);
    }
}

fn write_team_variant(team: Team) -> u8 {
    match team {
        Team::GLA => 0,
        Team::USA => 1,
        Team::China => 2,
        Team::Neutral => 3,
    }
}

fn read_team_variant(variant: u8) -> SaveLoadResult<Team> {
    match variant {
        0 => Ok(Team::GLA),
        1 => Ok(Team::USA),
        2 => Ok(Team::China),
        3 => Ok(Team::Neutral),
        _ => Err(SaveLoadError::Corrupted(format!(
            "Invalid team variant: {variant}"
        ))),
    }
}

fn write_kind_of_variant(kind_of: KindOf) -> u8 {
    match kind_of {
        KindOf::Structure => 0,
        KindOf::Infantry => 1,
        KindOf::Vehicle => 2,
        KindOf::Aircraft => 3,
        KindOf::Projectile => 4,
        KindOf::Resource => 5,
        KindOf::Selectable => 6,
        KindOf::Attackable => 7,
        KindOf::CommandCenter => 8,
        KindOf::SupplyCenter => 9,
        KindOf::PowerPlant => 10,
        KindOf::Harvestable => 11,
        KindOf::Worker => 12,
        KindOf::Hero => 13,
        KindOf::Powered => 14,
    }
}

fn read_kind_of_variant(variant: u8) -> SaveLoadResult<KindOf> {
    match variant {
        0 => Ok(KindOf::Structure),
        1 => Ok(KindOf::Infantry),
        2 => Ok(KindOf::Vehicle),
        3 => Ok(KindOf::Aircraft),
        4 => Ok(KindOf::Projectile),
        5 => Ok(KindOf::Resource),
        6 => Ok(KindOf::Selectable),
        7 => Ok(KindOf::Attackable),
        8 => Ok(KindOf::CommandCenter),
        9 => Ok(KindOf::SupplyCenter),
        10 => Ok(KindOf::PowerPlant),
        11 => Ok(KindOf::Harvestable),
        12 => Ok(KindOf::Worker),
        13 => Ok(KindOf::Hero),
        14 => Ok(KindOf::Powered),
        _ => Err(SaveLoadError::Corrupted(format!(
            "Invalid KindOf variant: {variant}"
        ))),
    }
}

fn xfer_f64_bytes<X: CommonXfer>(inner: &mut X, data: &mut f64) -> SaveLoadResult<()> {
    let reading = inner.is_reading();
    let mut bytes = if reading {
        [0u8; 8]
    } else {
        data.to_le_bytes()
    };
    for byte in &mut bytes {
        CommonXfer::xfer_unsigned_byte(inner, byte).map_err(SaveLoadError::Io)?;
    }
    if reading {
        *data = f64::from_le_bytes(bytes);
    }
    Ok(())
}

pub struct XferSave<W: Write + Seek> {
    identifier: String,
    options: XferOptions,
    inner: CommonXferSave<W>,
}

impl<W: Write + Seek> XferSave<W> {
    pub fn new(writer: W) -> Self {
        Self {
            identifier: String::new(),
            options: XferOptions::default(),
            inner: CommonXferSave::new(writer, 1),
        }
    }
}

impl<W: Write + Seek> Xfer for XferSave<W> {
    fn get_mode(&self) -> XferMode {
        CommonXfer::get_xfer_mode(&self.inner)
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: XferOptions) {
        self.options = options;
        apply_options(&mut self.inner, options);
    }

    fn get_options(&self) -> &XferOptions {
        &self.options
    }

    fn open(&mut self, identifier: &str) -> SaveLoadResult<()> {
        self.identifier = identifier.to_string();
        CommonXfer::open(&mut self.inner, identifier).map_err(map_status)
    }

    fn close(&mut self) -> SaveLoadResult<()> {
        CommonXfer::close(&mut self.inner).map_err(map_status)
    }

    fn begin_block(&mut self) -> SaveLoadResult<XferBlockSize> {
        CommonXfer::begin_block(&mut self.inner).map_err(map_status)
    }

    fn end_block(&mut self) -> SaveLoadResult<()> {
        CommonXfer::end_block(&mut self.inner).map_err(map_status)
    }

    fn skip(&mut self, data_size: i32) -> SaveLoadResult<()> {
        CommonXfer::skip(&mut self.inner, data_size).map_err(map_status)
    }

    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        current_version: XferVersion,
    ) -> SaveLoadResult<()> {
        CommonXfer::xfer_version(&mut self.inner, version, current_version)
            .map_err(SaveLoadError::Io)
    }

    fn xfer_bool(&mut self, data: &mut bool) -> SaveLoadResult<()> {
        CommonXfer::xfer_bool(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u8(&mut self, data: &mut u8) -> SaveLoadResult<()> {
        CommonXfer::xfer_unsigned_byte(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i8(&mut self, data: &mut i8) -> SaveLoadResult<()> {
        CommonXfer::xfer_byte(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u16(&mut self, data: &mut u16) -> SaveLoadResult<()> {
        CommonXfer::xfer_unsigned_short(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i16(&mut self, data: &mut i16) -> SaveLoadResult<()> {
        CommonXfer::xfer_short(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u32(&mut self, data: &mut u32) -> SaveLoadResult<()> {
        CommonXfer::xfer_unsigned_int(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i32(&mut self, data: &mut i32) -> SaveLoadResult<()> {
        CommonXfer::xfer_int(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u64(&mut self, data: &mut u64) -> SaveLoadResult<()> {
        CommonXfer::xfer_u64(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i64(&mut self, data: &mut i64) -> SaveLoadResult<()> {
        CommonXfer::xfer_int64(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_f32(&mut self, data: &mut f32) -> SaveLoadResult<()> {
        CommonXfer::xfer_real(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_f64(&mut self, data: &mut f64) -> SaveLoadResult<()> {
        xfer_f64_bytes(&mut self.inner, data)
    }

    fn xfer_string(&mut self, data: &mut String) -> SaveLoadResult<()> {
        CommonXfer::xfer_ascii_string(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_marker_label(&mut self, label: &str) -> SaveLoadResult<()> {
        CommonXfer::xfer_marker_label(&mut self.inner, label).map_err(SaveLoadError::Io)
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
        let mut cols = data.to_cols_array();
        for value in &mut cols {
            self.xfer_f32(value)?;
        }
        Ok(())
    }

    fn xfer_mat4(&mut self, data: &mut Mat4) -> SaveLoadResult<()> {
        let mut cols = data.to_cols_array();
        for value in &mut cols {
            self.xfer_f32(value)?;
        }
        Ok(())
    }

    fn xfer_object_id(&mut self, data: &mut ObjectId) -> SaveLoadResult<()> {
        CommonXfer::xfer_object_id(&mut self.inner, &mut data.0).map_err(SaveLoadError::Io)
    }

    fn xfer_team(&mut self, data: &mut Team) -> SaveLoadResult<()> {
        let mut variant = write_team_variant(*data);
        self.xfer_u8(&mut variant)
    }

    fn xfer_kind_of(&mut self, data: &mut KindOf) -> SaveLoadResult<()> {
        let mut variant = write_kind_of_variant(*data);
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
        for (key, value) in data.iter_mut() {
            let mut key_copy = unsafe { std::ptr::read(key as *const K) };
            key_copy.xfer(self)?;
            value.xfer(self)?;
            std::mem::forget(key_copy);
        }
        Ok(())
    }

    fn xfer_raw(&mut self, data: &mut [u8]) -> SaveLoadResult<()> {
        for byte in data.iter_mut() {
            self.xfer_u8(byte)?;
        }
        Ok(())
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
        self.xfer_raw(data)
    }
}

pub struct XferLoad<R: Read + Seek> {
    identifier: String,
    options: XferOptions,
    inner: CommonXferLoad<R>,
}

impl<R: Read + Seek> XferLoad<R> {
    pub fn new(reader: R) -> Self {
        Self {
            identifier: String::new(),
            options: XferOptions::default(),
            inner: CommonXferLoad::new(reader, 1),
        }
    }
}

impl<R: Read + Seek> Xfer for XferLoad<R> {
    fn get_mode(&self) -> XferMode {
        CommonXfer::get_xfer_mode(&self.inner)
    }

    fn get_identifier(&self) -> &str {
        &self.identifier
    }

    fn set_options(&mut self, options: XferOptions) {
        self.options = options;
        apply_options(&mut self.inner, options);
    }

    fn get_options(&self) -> &XferOptions {
        &self.options
    }

    fn open(&mut self, identifier: &str) -> SaveLoadResult<()> {
        self.identifier = identifier.to_string();
        CommonXfer::open(&mut self.inner, identifier).map_err(map_status)
    }

    fn close(&mut self) -> SaveLoadResult<()> {
        CommonXfer::close(&mut self.inner).map_err(map_status)
    }

    fn begin_block(&mut self) -> SaveLoadResult<XferBlockSize> {
        CommonXfer::begin_block(&mut self.inner).map_err(map_status)
    }

    fn end_block(&mut self) -> SaveLoadResult<()> {
        CommonXfer::end_block(&mut self.inner).map_err(map_status)
    }

    fn skip(&mut self, data_size: i32) -> SaveLoadResult<()> {
        CommonXfer::skip(&mut self.inner, data_size).map_err(map_status)
    }

    fn xfer_version(
        &mut self,
        version: &mut XferVersion,
        current_version: XferVersion,
    ) -> SaveLoadResult<()> {
        CommonXfer::xfer_version(&mut self.inner, version, current_version)
            .map_err(SaveLoadError::Io)
    }

    fn xfer_bool(&mut self, data: &mut bool) -> SaveLoadResult<()> {
        CommonXfer::xfer_bool(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u8(&mut self, data: &mut u8) -> SaveLoadResult<()> {
        CommonXfer::xfer_unsigned_byte(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i8(&mut self, data: &mut i8) -> SaveLoadResult<()> {
        CommonXfer::xfer_byte(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u16(&mut self, data: &mut u16) -> SaveLoadResult<()> {
        CommonXfer::xfer_unsigned_short(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i16(&mut self, data: &mut i16) -> SaveLoadResult<()> {
        CommonXfer::xfer_short(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u32(&mut self, data: &mut u32) -> SaveLoadResult<()> {
        CommonXfer::xfer_unsigned_int(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i32(&mut self, data: &mut i32) -> SaveLoadResult<()> {
        CommonXfer::xfer_int(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_u64(&mut self, data: &mut u64) -> SaveLoadResult<()> {
        CommonXfer::xfer_u64(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_i64(&mut self, data: &mut i64) -> SaveLoadResult<()> {
        CommonXfer::xfer_int64(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_f32(&mut self, data: &mut f32) -> SaveLoadResult<()> {
        CommonXfer::xfer_real(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_f64(&mut self, data: &mut f64) -> SaveLoadResult<()> {
        xfer_f64_bytes(&mut self.inner, data)
    }

    fn xfer_string(&mut self, data: &mut String) -> SaveLoadResult<()> {
        CommonXfer::xfer_ascii_string(&mut self.inner, data).map_err(SaveLoadError::Io)
    }

    fn xfer_marker_label(&mut self, label: &str) -> SaveLoadResult<()> {
        CommonXfer::xfer_marker_label(&mut self.inner, label).map_err(SaveLoadError::Io)
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
        let mut cols = [0.0f32; 9];
        for value in &mut cols {
            self.xfer_f32(value)?;
        }
        *data = Mat3::from_cols_array(&cols);
        Ok(())
    }

    fn xfer_mat4(&mut self, data: &mut Mat4) -> SaveLoadResult<()> {
        let mut cols = [0.0f32; 16];
        for value in &mut cols {
            self.xfer_f32(value)?;
        }
        *data = Mat4::from_cols_array(&cols);
        Ok(())
    }

    fn xfer_object_id(&mut self, data: &mut ObjectId) -> SaveLoadResult<()> {
        CommonXfer::xfer_object_id(&mut self.inner, &mut data.0).map_err(SaveLoadError::Io)
    }

    fn xfer_team(&mut self, data: &mut Team) -> SaveLoadResult<()> {
        let mut variant = 0u8;
        self.xfer_u8(&mut variant)?;
        *data = read_team_variant(variant)?;
        Ok(())
    }

    fn xfer_kind_of(&mut self, data: &mut KindOf) -> SaveLoadResult<()> {
        let mut variant = 0u8;
        self.xfer_u8(&mut variant)?;
        *data = read_kind_of_variant(variant)?;
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
        for byte in data.iter_mut() {
            self.xfer_u8(byte)?;
        }
        Ok(())
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
        self.xfer_raw(data)
    }
}

//! MapCacheBuilder library: HeightMap v4 parse/getExtent + tool chrome model.

#![allow(clippy::type_complexity)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod cache;
pub mod chrome;

#[cfg(feature = "ui")]
pub mod ui;

pub use cache::{
    parse_map_bytes, run_cli, write_synthetic_ckmp_map, ExtractedMapInfo, MapCache, MapMetaData,
    CACHE_FILE_NAME, DEFAULT_MAP_DIRS, MAP_HEIGHT_SCALE, MAP_XY_FACTOR,
};
pub use chrome::{MapCacheChrome, MapExtentStatus};

// System-level modules
pub mod archive_file;
pub mod archive_file_system;
pub mod ascii_string;
pub mod big_file_system;
pub mod build_assistant;
pub mod cd_manager;
pub mod compression;
pub mod copy_protection;
pub mod critical_section;
pub mod data_chunk;
pub mod data_chunk_io;
pub mod debug;
pub mod directory;
pub mod disabled_types;
pub mod encrypt;
pub mod file;
pub mod file_system;
pub mod function_lexicon;
pub mod game_common;
pub mod game_memory;
pub mod game_type;
pub mod geometry;
pub mod kind_of;
pub mod linked_list;
pub mod list;
pub mod local_file;
pub mod local_file_system;
pub mod memory_init;
pub mod object_status_types;
pub mod quick_trig;
pub mod quoted_printable;
pub mod radar;
pub mod ram_file;
pub mod registry;
pub mod scene_submission;
pub mod snapshot;
pub mod stack_dump;
pub mod streaming_archive_file;
pub mod string;
pub mod subsystem_interface;
pub mod trig;
pub mod unicode_string;
pub mod upgrade;
pub mod xfer;
pub mod xfer_crc;
pub mod xfer_load;
pub mod xfer_postprocess;
pub mod xfer_save;
pub mod xfer_version;

#[cfg(test)]
pub mod xfer_tests;

pub mod save_game;

// Re-export commonly used types
pub use data_chunk_io::{DataChunkInfo, DataChunkInput, DataChunkOutput, DataChunkVersionType};
pub use geometry::{
    BoundingBox, Coord3D, GeometryInfo, GeometryType, Matrix3D, Point2D, Point3D, Rectangle,
};
pub use scene_submission::{SceneLineDesc, SceneLineId, SceneSubmission};
pub use snapshot::{Snapshot, SnapshotManager, Snapshotable};
pub use subsystem_interface::{SubsystemInterface, SubsystemResult, SubsystemState};
pub use xfer::{
    Color, ICoord2D, ICoord3D, IRegion2D, IRegion3D, RGBAColorInt, RGBAColorReal, RGBColor,
    RealRange, Region2D, Region3D, Xfer, XferBlockSize, XferMode, XferOptions, XferStatus,
    XferVersion, Xferable,
};

// Trait for overridable objects
pub trait Overridable {
    fn is_override(&self) -> bool;
    fn delete_overrides(&self);
}

//! WWSaveLoad - Westwood Studios Save/Load System
//!
//! This crate provides the core save/load functionality for the Command & Conquer
//! Generals Zero Hour game engine. It manages definitions, serialization, and
//! persistence of game state.
//!
//! This Rust conversion maintains the same fundamental architecture while using
//! modern Rust idioms for safety and performance.

pub mod definition;
pub mod parameter;
pub mod persist;
pub mod pointerremap;
pub mod saveload;
pub mod wwsaveload;

// Re-export the main types for easier access
pub mod definitionclassids;
pub mod definitionfactory;
pub mod definitionfactorymgr;
pub mod definitionmgr;
pub mod editable;
pub mod parameterlist;
pub mod parametertypes;
pub mod persistfactory;
pub mod postloadable;
pub mod saveloadids;
pub mod saveloadstatus;
pub mod saveloadsubsystem;
pub mod simpledefinitionfactory;
pub mod simpleparameter;
pub mod twiddler;
pub use wwsaveload::{DefinitionManager, SaveLoadError, SaveLoadResult, WWSaveLoad};

// Re-export the definition system types
pub use definition::{
    Definition, DefinitionClass, DefinitionError, DefinitionFactory, DefinitionResult,
    EditableClass,
};

// Re-export the parameter system types
pub use parameter::{
    EnumValue, Matrix3D, OBBox, Parameter, ParameterError, ParameterFactory, ParameterList,
    ParameterResult, ParameterType, ParameterValue, Range, Rect, Script, Vector2, Vector3,
};

// Re-export the pointer remapping system types
pub use pointerremap::{
    PointerRemap, PointerRemapClass, PointerRemapError, PointerRemapStatistics, RefCountPtr,
    RefCountable, RemapError, RemapStatistics, WeakRefCountPtr,
};

// Macros are automatically re-exported at the crate root

// Re-export the core save/load system types
pub use saveload::{
    get_save_load_system, ChunkId, ChunkLoad, ChunkLoadExt, ChunkSave, ChunkSaveExt, Persist,
    PersistFactory, PostLoadable, RefCount, RemapId, SaveLoadError as CoreSaveLoadError,
    SaveLoadResult as CoreSaveLoadResult, SaveLoadSubsystem, SaveLoadSystem,
};

// Re-export the persist system types
pub use persist::{PersistFactoryRegistry, SimplePersistFactory};

// Macros are re-exported automatically at the crate root
// The pointer remapping macros (request_pointer_remap!, request_ref_counted_pointer_remap!)
// are available at the crate root after importing the pointerremap module

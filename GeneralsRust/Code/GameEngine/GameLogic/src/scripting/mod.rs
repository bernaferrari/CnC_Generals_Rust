//! Script data, execution, and match-owned query interfaces.
#![allow(ambiguous_glob_reexports)]

use std::collections::HashMap;
use std::fmt;
use game_engine::common::system::{Xfer, XferStatus};
use serde::{Deserialize, Serialize};

pub mod chunk_codec;
pub mod conditions;
pub mod core;
pub mod engine;
pub mod evaluator;
pub mod events;
pub mod executor;
pub mod host_script_garrison;
pub mod ini_parser;
pub mod map_scripts;

pub use conditions::*;
pub use conditions::{
    HostObjectTriggerPersist, HostScriptPlayerCensus, HostScriptQueryObject,
    HostScriptQuerySnapshot, HostTechBuildingCensus, HostTriggerSlotPersist,
    host_object_did_enter, host_object_did_exit,
    host_object_has_kind, host_script_lookup_polygon_trigger,
};
pub use core::*;
pub use engine::*;
pub use events::*;
pub use executor::*;
pub use host_script_garrison::*;
pub use map_scripts::{MapMetadata, MapScriptLoader};

/// Snapshot serialization trait for scripting data.
pub trait XferSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus>;
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus>;
    fn load_post_process(&mut self) -> Result<(), XferStatus>;
}

/// Script execution priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScriptPriority {
    /// Background/ambient scripts
    Low = 0,
    /// Normal event scripts
    Normal = 1,
    /// Important gameplay scripts
    High = 2,
    /// Critical system scripts
    Critical = 3,
    /// Emergency/debug scripts
    Emergency = 4,
}

/// Script value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScriptValue {
    /// Null/None value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Int(i64),
    /// Float value  
    Float(f64),
    /// String value
    String(String),
    /// 3D coordinate
    Coord3D([f32; 3]),
    /// Object ID reference
    ObjectId(u32),
    /// Player ID reference
    PlayerId(u32),
    /// Team name
    Team(String),
    /// Array of values
    Array(Vec<ScriptValue>),
    /// Object/map of values
    Object(HashMap<String, ScriptValue>),
}

impl ScriptValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ScriptValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ScriptValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_real(&self) -> Option<f64> {
        match self {
            ScriptValue::Float(f) => Some(*f),
            ScriptValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            ScriptValue::Int(i) => Some(*i),
            ScriptValue::Float(f) => Some(*f as i64),
            _ => None,
        }
    }
}

impl fmt::Display for ScriptValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptValue::Null => write!(f, "null"),
            ScriptValue::Bool(b) => write!(f, "{}", b),
            ScriptValue::Int(i) => write!(f, "{}", i),
            ScriptValue::Float(fl) => write!(f, "{}", fl),
            ScriptValue::String(s) => write!(f, "\"{}\"", s),
            ScriptValue::Coord3D(coord) => write!(f, "({}, {}, {})", coord[0], coord[1], coord[2]),
            ScriptValue::ObjectId(id) => write!(f, "Object({})", id),
            ScriptValue::PlayerId(id) => write!(f, "Player({})", id),
            ScriptValue::Team(team) => write!(f, "Team({})", team),
            ScriptValue::Array(arr) => {
                write!(f, "[")?;
                for (i, val) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            ScriptValue::Object(obj) => {
                write!(f, "{{")?;
                for (i, (key, val)) in obj.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, val)?;
                }
                write!(f, "}}")
            }
        }
    }
}


//! FILE: ini_fx_list.rs
//! Author: Steven Johnson, December 2001 (Converted to Rust)
//! Desc: FX List parsing - audio/visual effect collections
//!
//! Matches C++ FXList.h and FXList.cpp

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;

pub type FXListResult<T> = Result<T, FXListError>;

#[derive(Debug, Clone, PartialEq)]
pub enum FXListError {
    InvalidName,
    ParseError(String),
    NotFound,
}

impl std::fmt::Display for FXListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FXListError::InvalidName => write!(f, "Invalid FXList name"),
            FXListError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            FXListError::NotFound => write!(f, "FXList not found"),
        }
    }
}

impl std::error::Error for FXListError {}

/// FX Nugget types - audio/visual effect components
/// Matches C++ FXNugget hierarchy from FXList.cpp
#[derive(Debug, Clone)]
pub enum FXNugget {
    Sound {
        name: AsciiString,
    },
    Tracer {
        name: AsciiString,
        speed: f32,
        length: f32,
        width: f32,
    },
    ParticleSystem {
        name: AsciiString,
        count: i32,
    },
    FXParticleSystem {
        name: AsciiString,
    },
    LightPulse {
        color: (f32, f32, f32),
        radius: f32,
    },
    ViewShake {
        intensity: f32,
    },
    TerrainScorch {
        scorch_type: AsciiString,
        radius: f32,
    },
}

/// FX List - collection of effects
/// Matches C++ FXList from FXList.h lines 99-162
#[derive(Debug, Clone)]
pub struct FXList {
    pub name: AsciiString,
    pub nuggets: Vec<FXNugget>,
}

impl FXList {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            nuggets: Vec::new(),
        }
    }

    pub fn add_nugget(&mut self, nugget: FXNugget) {
        self.nuggets.push(nugget);
    }
}

/// FX List store
pub struct FXListStore {
    fx_lists: HashMap<AsciiString, FXList>,
}

impl FXListStore {
    pub fn new() -> Self {
        Self {
            fx_lists: HashMap::new(),
        }
    }

    pub fn add_fx_list(&mut self, fx_list: FXList) {
        self.fx_lists.insert(fx_list.name.clone(), fx_list);
    }

    pub fn find_fx_list(&self, name: &str) -> Option<&FXList> {
        self.fx_lists.get(&AsciiString::from(name))
    }
}

impl Default for FXListStore {
    fn default() -> Self {
        Self::new()
    }
}

static FX_LIST_STORE: OnceCell<RwLock<FXListStore>> = OnceCell::new();

pub fn get_fx_list_store() -> RwLockReadGuard<'static, FXListStore> {
    FX_LIST_STORE
        .get_or_init(|| RwLock::new(FXListStore::new()))
        .read()
        .unwrap()
}

pub fn get_fx_list_store_mut() -> RwLockWriteGuard<'static, FXListStore> {
    FX_LIST_STORE
        .get_or_init(|| RwLock::new(FXListStore::new()))
        .write()
        .unwrap()
}

/// Parse FXList definition from INI
/// Matches C++ FXListStore::parseFXListDefinition
pub fn parse_fx_list_definition(
    name: &str,
    properties: &HashMap<String, String>,
) -> FXListResult<FXList> {
    let mut fx_list = FXList::new(AsciiString::from(name));

    // Parse nuggets from properties
    // In C++, this uses a complex sub-parsing system
    // For now, we create a simplified version
    for (key, value) in properties {
        match key.as_str() {
            "Sound" => {
                fx_list.add_nugget(FXNugget::Sound {
                    name: AsciiString::from(value.as_str()),
                });
            }
            "ParticleSystem" => {
                fx_list.add_nugget(FXNugget::ParticleSystem {
                    name: AsciiString::from(value.as_str()),
                    count: 1,
                });
            }
            _ => {}
        }
    }

    Ok(fx_list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx_list_creation() {
        let fx_list = FXList::new(AsciiString::from("TestFX"));
        assert_eq!(fx_list.name.to_str(), "TestFX");
        assert_eq!(fx_list.nuggets.len(), 0);
    }

    #[test]
    fn test_fx_nugget_addition() {
        let mut fx_list = FXList::new(AsciiString::from("TestFX"));
        fx_list.add_nugget(FXNugget::Sound {
            name: AsciiString::from("explosion"),
        });
        assert_eq!(fx_list.nuggets.len(), 1);
    }
}

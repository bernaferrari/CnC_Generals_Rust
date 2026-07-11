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

/// View shake types (C++ View::CameraShakeType, FXList.cpp:397)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraShakeType {
    Subtle,
    Normal,
    Strong,
    Severe,
    CineExtreme,
    CineInsane,
}

/// Terrain scorch types (C++ Scorches enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorchType {
    Scorch1,
    Scorch2,
    Scorch3,
    Scorch4,
    ShadowScorch,
    Random,
}

/// FX Nugget types - audio/visual effect components
/// Matches C++ TheFXListFieldParse[] (FXList.cpp:746)
#[derive(Debug, Clone)]
pub enum FXNugget {
    Sound {
        name: AsciiString,
    },
    Tracer {
        name: AsciiString,
        bone_name: AsciiString,
        speed: f32,
        decay_at: f32,
        length: f32,
        width: f32,
        color: (f32, f32, f32),
        probability: f32,
    },
    RayEffect {
        name: AsciiString,
        primary_offset: (f32, f32, f32),
        secondary_offset: (f32, f32, f32),
    },
    LightPulse {
        color: (f32, f32, f32),
        radius: f32,
        radius_as_percent_of_object_size: f32,
        increase_frames: u32,
        decrease_frames: u32,
    },
    ViewShake {
        shake_type: CameraShakeType,
    },
    TerrainScorch {
        scorch_type: ScorchType,
        radius: f32,
    },
    ParticleSystem {
        name: AsciiString,
        count: i32,
        offset: (f32, f32, f32),
        radius: f32,
        height: f32,
        initial_delay: f32,
        rotate_x: f32,
        rotate_y: f32,
        rotate_z: f32,
        orient_to_object: bool,
        attach_to_object: bool,
        create_at_ground_height: bool,
        use_callers_radius: bool,
    },
    FXListAtBonePos {
        fx_name: AsciiString,
        bone_name: AsciiString,
        orient_to_bone: bool,
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
/// Matches C++ FXListStore::parseFXListDefinition and TheFXListFieldParse[] (FXList.cpp:746)
pub fn parse_fx_list_definition(
    name: &str,
    properties: &HashMap<String, String>,
) -> FXListResult<FXList> {
    let mut fx_list = FXList::new(AsciiString::from(name));

    // Dispatch matching C++ TheFXListFieldParse (FXList.cpp:746-757)
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
                    offset: (0.0, 0.0, 0.0),
                    radius: 0.0,
                    height: 0.0,
                    initial_delay: -1.0,
                    rotate_x: 0.0,
                    rotate_y: 0.0,
                    rotate_z: 0.0,
                    orient_to_object: false,
                    attach_to_object: false,
                    create_at_ground_height: false,
                    use_callers_radius: false,
                });
            }
            "Tracer" => {
                fx_list.add_nugget(FXNugget::Tracer {
                    name: AsciiString::from(value.as_str()),
                    bone_name: AsciiString::new(),
                    speed: 0.0,
                    decay_at: 1.0,
                    length: 10.0,
                    width: 1.0,
                    color: (1.0, 1.0, 1.0),
                    probability: 1.0,
                });
            }
            "RayEffect" => {
                fx_list.add_nugget(FXNugget::RayEffect {
                    name: AsciiString::from(value.as_str()),
                    primary_offset: (0.0, 0.0, 0.0),
                    secondary_offset: (0.0, 0.0, 0.0),
                });
            }
            "LightPulse" => {
                fx_list.add_nugget(FXNugget::LightPulse {
                    color: (0.0, 0.0, 0.0),
                    radius: 0.0,
                    radius_as_percent_of_object_size: 0.0,
                    increase_frames: 0,
                    decrease_frames: 0,
                });
            }
            "ViewShake" => {
                let shake_type = match value.to_uppercase().as_str() {
                    "SUBTLE" => CameraShakeType::Subtle,
                    "NORMAL" => CameraShakeType::Normal,
                    "STRONG" => CameraShakeType::Strong,
                    "SEVERE" => CameraShakeType::Severe,
                    "CINE_EXTREME" => CameraShakeType::CineExtreme,
                    "CINE_INSANE" => CameraShakeType::CineInsane,
                    _ => CameraShakeType::Normal,
                };
                fx_list.add_nugget(FXNugget::ViewShake { shake_type });
            }
            "TerrainScorch" => {
                let scorch_type = match value.to_uppercase().as_str() {
                    "SCORCH_1" => ScorchType::Scorch1,
                    "SCORCH_2" => ScorchType::Scorch2,
                    "SCORCH_3" => ScorchType::Scorch3,
                    "SCORCH_4" => ScorchType::Scorch4,
                    "SHADOW_SCORCH" => ScorchType::ShadowScorch,
                    "RANDOM" => ScorchType::Random,
                    _ => ScorchType::Random,
                };
                fx_list.add_nugget(FXNugget::TerrainScorch {
                    scorch_type,
                    radius: 0.0,
                });
            }
            "FXListAtBonePos" => {
                fx_list.add_nugget(FXNugget::FXListAtBonePos {
                    fx_name: AsciiString::from(value.as_str()),
                    bone_name: AsciiString::new(),
                    orient_to_bone: true,
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

    #[test]
    fn test_parse_all_nugget_types() {
        let mut props = HashMap::new();
        props.insert("Sound".to_string(), "BoomSound".to_string());
        props.insert("Tracer".to_string(), "GenericTracer".to_string());
        props.insert("RayEffect".to_string(), "RayTemplate".to_string());
        props.insert("LightPulse".to_string(), "".to_string());
        props.insert("ViewShake".to_string(), "STRONG".to_string());
        props.insert("TerrainScorch".to_string(), "RANDOM".to_string());
        props.insert("ParticleSystem".to_string(), "ExplosionPS".to_string());
        props.insert("FXListAtBonePos".to_string(), "BoneFX".to_string());

        let fx_list = parse_fx_list_definition("AllTypesFX", &props).unwrap();
        assert_eq!(fx_list.nuggets.len(), 8);

        assert!(
            matches!(&fx_list.nuggets[0], FXNugget::Sound { name } if name.to_str() == "BoomSound")
        );
        assert!(matches!(&fx_list.nuggets[1], FXNugget::Tracer { .. }));
        assert!(matches!(&fx_list.nuggets[2], FXNugget::RayEffect { .. }));
        assert!(matches!(&fx_list.nuggets[3], FXNugget::LightPulse { .. }));
        assert!(matches!(
            &fx_list.nuggets[4],
            FXNugget::ViewShake {
                shake_type: CameraShakeType::Strong
            }
        ));
        assert!(matches!(
            &fx_list.nuggets[5],
            FXNugget::TerrainScorch {
                scorch_type: ScorchType::Random,
                ..
            }
        ));
        assert!(matches!(
            &fx_list.nuggets[6],
            FXNugget::ParticleSystem { .. }
        ));
        assert!(matches!(
            &fx_list.nuggets[7],
            FXNugget::FXListAtBonePos { .. }
        ));
    }
}

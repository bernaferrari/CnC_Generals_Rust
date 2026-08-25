//! FILE: ini_fx_list.rs
//! Author: Steven Johnson, December 2001 (Converted to Rust)
//! Desc: FX List parsing - audio/visual effect collections
//!
//! Matches C++ FXList.h and FXList.cpp

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::game_common::ObjectShroudStatus;
use crate::common::ini::ini::INI;

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
        ricochet: bool,
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

fn property<'a>(properties: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    properties
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn real(properties: &HashMap<String, String>, name: &str, default: f32) -> FXListResult<f32> {
    let Some(value) = property(properties, name) else {
        return Ok(default);
    };
    // GameClientRandomVariable accepts a range; this representation stores a
    // single deterministic value, so retain its first (base) component.
    let value = value.split_whitespace().next().unwrap_or(value);
    value
        .trim_end_matches('%')
        .parse::<f32>()
        .map(|parsed| {
            if value.ends_with('%') {
                parsed / 100.0
            } else {
                parsed
            }
        })
        .map_err(|_| FXListError::ParseError(format!("invalid {} value '{}'", name, value)))
}

fn integer(properties: &HashMap<String, String>, name: &str, default: i32) -> FXListResult<i32> {
    let Some(value) = property(properties, name) else {
        return Ok(default);
    };
    value
        .parse::<i32>()
        .map_err(|_| FXListError::ParseError(format!("invalid {} value '{}'", name, value)))
}

fn boolean(properties: &HashMap<String, String>, name: &str, default: bool) -> FXListResult<bool> {
    let Some(value) = property(properties, name) else {
        return Ok(default);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        _ => Err(FXListError::ParseError(format!(
            "invalid {} boolean '{}'",
            name, value
        ))),
    }
}

fn labelled_vec3(value: &str) -> FXListResult<(f32, f32, f32)> {
    let mut result = [None; 3];
    for component in value.split_whitespace() {
        if let Some((label, raw)) = component.split_once(':') {
            // Retail FXList.ini contains `Y:15:` in two offsets; C++ `atof`
            // accepts the numeric prefix, so tolerate that trailing colon.
            let parsed = raw.trim_end_matches(':').parse::<f32>().map_err(|_| {
                FXListError::ParseError(format!("invalid vector component '{}'", component))
            })?;
            match label.to_ascii_uppercase().as_str() {
                "X" | "R" => result[0] = Some(parsed),
                "Y" | "G" => result[1] = Some(parsed),
                "Z" | "B" => result[2] = Some(parsed),
                _ => {
                    return Err(FXListError::ParseError(format!(
                        "unknown vector component '{}'",
                        label
                    )));
                }
            }
        }
    }
    Ok((
        result[0].unwrap_or(0.0),
        result[1].unwrap_or(0.0),
        result[2].unwrap_or(0.0),
    ))
}

/// Parse one nested C++ FX nugget block while preserving repeated nuggets.
pub fn parse_fx_nugget_definition(
    kind: &str,
    properties: &HashMap<String, String>,
) -> FXListResult<FXNugget> {
    let name = |field: &str| AsciiString::from(property(properties, field).unwrap_or(""));
    match kind.to_ascii_lowercase().as_str() {
        "sound" => Ok(FXNugget::Sound { name: name("Name") }),
        "tracer" => Ok(FXNugget::Tracer {
            name: name("TracerName"),
            bone_name: name("BoneName"),
            speed: real(properties, "Speed", 0.0)?,
            decay_at: real(properties, "DecayAt", 1.0)?,
            length: real(properties, "Length", 10.0)?,
            width: real(properties, "Width", 1.0)?,
            color: property(properties, "Color")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((255.0, 255.0, 255.0)),
            probability: real(properties, "Probability", 1.0)?,
        }),
        "rayeffect" => Ok(FXNugget::RayEffect {
            name: name("Name"),
            primary_offset: property(properties, "PrimaryOffset")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
            secondary_offset: property(properties, "SecondaryOffset")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
        }),
        "lightpulse" => Ok(FXNugget::LightPulse {
            color: property(properties, "Color")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
            radius: real(properties, "Radius", 0.0)?,
            radius_as_percent_of_object_size: real(properties, "RadiusAsPercentOfObjectSize", 0.0)?,
            increase_frames: property(properties, "IncreaseTime")
                .map(INI::parse_duration_unsigned_int)
                .transpose()
                .map_err(|_| FXListError::ParseError("invalid IncreaseTime".into()))?
                .unwrap_or(0),
            decrease_frames: property(properties, "DecreaseTime")
                .map(INI::parse_duration_unsigned_int)
                .transpose()
                .map_err(|_| FXListError::ParseError("invalid DecreaseTime".into()))?
                .unwrap_or(0),
        }),
        "viewshake" => {
            let shake_type = match property(properties, "Type")
                .unwrap_or("NORMAL")
                .to_ascii_uppercase()
                .as_str()
            {
                "SUBTLE" => CameraShakeType::Subtle,
                "NORMAL" => CameraShakeType::Normal,
                "STRONG" => CameraShakeType::Strong,
                "SEVERE" => CameraShakeType::Severe,
                "CINE_EXTREME" => CameraShakeType::CineExtreme,
                "CINE_INSANE" => CameraShakeType::CineInsane,
                other => {
                    return Err(FXListError::ParseError(format!(
                        "unknown view shake type '{}'",
                        other
                    )));
                }
            };
            Ok(FXNugget::ViewShake { shake_type })
        }
        "terrainscorch" => {
            let scorch_type = match property(properties, "Type")
                .unwrap_or("RANDOM")
                .to_ascii_uppercase()
                .as_str()
            {
                "SCORCH_1" => ScorchType::Scorch1,
                "SCORCH_2" => ScorchType::Scorch2,
                "SCORCH_3" => ScorchType::Scorch3,
                "SCORCH_4" => ScorchType::Scorch4,
                "SHADOW_SCORCH" => ScorchType::ShadowScorch,
                "RANDOM" => ScorchType::Random,
                other => {
                    return Err(FXListError::ParseError(format!(
                        "unknown terrain scorch type '{}'",
                        other
                    )));
                }
            };
            Ok(FXNugget::TerrainScorch {
                scorch_type,
                radius: real(properties, "Radius", 0.0)?,
            })
        }
        "particlesystem" => Ok(FXNugget::ParticleSystem {
            name: name("Name"),
            count: integer(properties, "Count", 1)?,
            offset: property(properties, "Offset")
                .map(labelled_vec3)
                .transpose()?
                .unwrap_or((0.0, 0.0, 0.0)),
            radius: real(properties, "Radius", 0.0)?,
            height: real(properties, "Height", 0.0)?,
            initial_delay: real(properties, "InitialDelay", -1.0)?,
            rotate_x: real(properties, "RotateX", 0.0)?.to_radians(),
            rotate_y: real(properties, "RotateY", 0.0)?.to_radians(),
            rotate_z: real(properties, "RotateZ", 0.0)?.to_radians(),
            orient_to_object: boolean(properties, "OrientToObject", false)?,
            ricochet: boolean(properties, "Ricochet", false)?,
            attach_to_object: boolean(properties, "AttachToObject", false)?,
            create_at_ground_height: boolean(properties, "CreateAtGroundHeight", false)?,
            use_callers_radius: boolean(properties, "UseCallersRadius", false)?,
        }),
        "fxlistatbonepos" => Ok(FXNugget::FXListAtBonePos {
            fx_name: name("FX"),
            bone_name: name("BoneName"),
            orient_to_bone: boolean(properties, "OrientToBone", false)?,
        }),
        _ => Err(FXListError::ParseError(format!(
            "unknown FX nugget type '{}'",
            kind
        ))),
    }
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

/// C++ `FXList::doFXObj` live runner (GameClient registers the full nugget impls).
pub trait FxListObjRuntime: Send + Sync {
    /// Handle `FXList::doFXObj` for `name`. Return `true` when the client runner
    /// owns playback (shroud + every nugget). `false` lets Common dispatch locally.
    fn do_fx_obj(&self, name: &str, primary_id: Option<u32>, secondary_id: Option<u32>) -> bool;
    fn object_shrouded_status(&self, _object_id: u32) -> Option<ObjectShroudStatus> {
        None
    }
}

static FX_LIST_OBJ_RUNTIME: LazyLock<RwLock<Option<Arc<dyn FxListObjRuntime>>>> =
    LazyLock::new(|| RwLock::new(None));
static DISPATCHED_FX_NUGGETS: LazyLock<Mutex<Vec<DispatchedFxNugget>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

fn fx_list_obj_runtime_slot() -> &'static RwLock<Option<Arc<dyn FxListObjRuntime>>> {
    &FX_LIST_OBJ_RUNTIME
}

/// Register the live `FXList::doFXObj` runner (C++ DamageFX.cpp:73).
pub fn register_fx_list_obj_runtime(runtime: Arc<dyn FxListObjRuntime>) {
    if let Ok(mut slot) = fx_list_obj_runtime_slot().write() {
        *slot = Some(runtime);
    }
}

pub fn clear_fx_list_obj_runtime() {
    if let Ok(mut slot) = fx_list_obj_runtime_slot().write() {
        *slot = None;
    }
}

pub fn fx_list_obj_runtime() -> Option<Arc<dyn FxListObjRuntime>> {
    fx_list_obj_runtime_slot()
        .read()
        .ok()
        .and_then(|slot| slot.clone())
}

/// Nuggets actually visited by Common `FXList::doFXObj` (tests + leftover drain).
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchedFxNugget {
    Sound(String),
    Tracer(String),
    RayEffect(String),
    LightPulse,
    ViewShake(CameraShakeType),
    TerrainScorch(ScorchType),
    ParticleSystem(String),
    FXListAtBonePos(String),
}

fn dispatched_fx_nuggets() -> &'static Mutex<Vec<DispatchedFxNugget>> {
    &DISPATCHED_FX_NUGGETS
}

pub fn record_dispatched_fx_nugget(nugget: DispatchedFxNugget) {
    if let Ok(mut log) = dispatched_fx_nuggets().lock() {
        log.push(nugget);
    }
}

pub fn take_dispatched_fx_nuggets() -> Vec<DispatchedFxNugget> {
    dispatched_fx_nuggets()
        .lock()
        .map(|mut log| std::mem::take(&mut *log))
        .unwrap_or_default()
}

/// C++ `FXList.cpp:796` — skip FX when primary is fogged/shrouded.
pub fn fx_obj_is_visible(
    primary_id: Option<u32>,
    primary_shroud: Option<ObjectShroudStatus>,
) -> bool {
    let Some(primary_id) = primary_id else {
        return true;
    };
    let status = primary_shroud.or_else(|| {
        fx_list_obj_runtime().and_then(|runtime| runtime.object_shrouded_status(primary_id))
    });
    let Some(status) = status else {
        return true;
    };
    (status as u32) <= (ObjectShroudStatus::PartialClear as u32)
}

impl FXNugget {
    pub fn dispatched_kind(&self) -> DispatchedFxNugget {
        match self {
            FXNugget::Sound { name } => DispatchedFxNugget::Sound(name.as_str().to_string()),
            FXNugget::Tracer { name, .. } => DispatchedFxNugget::Tracer(name.as_str().to_string()),
            FXNugget::RayEffect { name, .. } => {
                DispatchedFxNugget::RayEffect(name.as_str().to_string())
            }
            FXNugget::LightPulse { .. } => DispatchedFxNugget::LightPulse,
            FXNugget::ViewShake { shake_type } => DispatchedFxNugget::ViewShake(*shake_type),
            FXNugget::TerrainScorch { scorch_type, .. } => {
                DispatchedFxNugget::TerrainScorch(*scorch_type)
            }
            FXNugget::ParticleSystem { name, .. } => {
                DispatchedFxNugget::ParticleSystem(name.as_str().to_string())
            }
            FXNugget::FXListAtBonePos { fx_name, .. } => {
                DispatchedFxNugget::FXListAtBonePos(fx_name.as_str().to_string())
            }
        }
    }
}

impl FXList {
    /// C++ `FXList::doFXObj` (FXList.cpp:794-804): shroud gate, then every nugget.
    pub fn do_fx_obj(&self, primary_id: Option<u32>, primary_shroud: Option<ObjectShroudStatus>) {
        if !fx_obj_is_visible(primary_id, primary_shroud) {
            return;
        }
        for nugget in &self.nuggets {
            record_dispatched_fx_nugget(nugget.dispatched_kind());
        }
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
                    ricochet: false,
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

    #[test]
    fn do_fx_obj_visits_every_nugget_unless_fogged() {
        // C++ FXList.cpp:794-804
        let _ = take_dispatched_fx_nuggets();
        let mut fx_list = FXList::new(AsciiString::from("DoFxObjTest"));
        fx_list.add_nugget(FXNugget::Sound {
            name: AsciiString::from("Hit"),
        });
        fx_list.add_nugget(FXNugget::ViewShake {
            shake_type: CameraShakeType::Strong,
        });
        fx_list.do_fx_obj(Some(1), Some(ObjectShroudStatus::Clear));
        let dispatched = take_dispatched_fx_nuggets();
        assert_eq!(dispatched.len(), 2);
        fx_list.do_fx_obj(Some(1), Some(ObjectShroudStatus::Fogged));
        assert!(take_dispatched_fx_nuggets().is_empty());
    }
}

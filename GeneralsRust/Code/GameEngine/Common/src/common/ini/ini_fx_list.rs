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

pub use generals_fx::{
    CameraShakeType, DispatchedFxNugget, Distribution, FXListError, FXListResult, FxCatalog,
    FxRandomVariable, ScorchType,
};
pub type FXNugget = generals_fx::FXNugget<AsciiString>;

struct EngineFxValues;
impl generals_fx::FxValueParser for EngineFxValues {
    fn velocity(&self, value: &str) -> FXListResult<f32> {
        INI::parse_velocity_real(value)
            .map_err(|_| FXListError::ParseError("invalid velocity".into()))
    }
    fn percent(&self, value: &str) -> FXListResult<f32> {
        INI::parse_percent_to_real(value)
            .map_err(|_| FXListError::ParseError("invalid percent".into()))
    }
    fn duration(&self, value: &str) -> FXListResult<u32> {
        INI::parse_duration_unsigned_int(value)
            .map_err(|_| FXListError::ParseError("invalid duration".into()))
    }
}
/// Shared pure parser with the engine's original unit conversions.
pub fn parse_fx_nugget_definition(
    kind: &str,
    properties: &HashMap<String, String>,
) -> FXListResult<FXNugget> {
    generals_fx::parse_fx_nugget_definition(&EngineFxValues, kind, properties)
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
    fx_lists: FxCatalog<FXList, AsciiString>,
}

impl FXListStore {
    pub fn new() -> Self {
        Self {
            fx_lists: FxCatalog::default(),
        }
    }

    pub fn add_fx_list(&mut self, fx_list: FXList) {
        self.fx_lists.insert(fx_list.name.clone(), fx_list);
    }

    pub fn find_fx_list(&self, name: &str) -> Option<&FXList> {
        self.fx_lists.find(name)
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
        let mut fx_list = FXList::new("AllTypesFX".into());
        for (kind, field, value) in [
            ("Sound", "Name", "BoomSound"),
            ("Tracer", "TracerName", "GenericTracer"),
            ("RayEffect", "Name", "RayTemplate"),
            ("LightPulse", "Radius", "0"),
            ("ViewShake", "Type", "STRONG"),
            ("TerrainScorch", "Type", "RANDOM"),
            ("ParticleSystem", "Name", "ExplosionPS"),
            ("FXListAtBonePos", "FX", "BoneFX"),
        ] {
            let properties = HashMap::from([(field.into(), value.into())]);
            fx_list.add_nugget(parse_fx_nugget_definition(kind, &properties).unwrap());
        }
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

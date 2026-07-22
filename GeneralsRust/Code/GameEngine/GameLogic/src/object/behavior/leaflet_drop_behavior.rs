//! LeafletDropBehavior - Rust conversion of C++ LeafletDropBehavior
//!
//! Drops leaflets and disables enemy infantry/vehicles in range.

use crate::common::xfer::XferExt;
use crate::common::DisabledType;
use crate::common::{AsciiString, Bool, ModuleData, Real, Relationship, UnsignedInt, XferVersion};
use crate::helpers::{TheGameLogic, TheParticleSystemManager, ThePartitionManager};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct LeafletDropBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub delay_frames: UnsignedInt,
    pub disabled_duration: UnsignedInt,
    pub radius: Real,
    pub leaflet_fx_particle_system: Option<AsciiString>,
}

impl Default for LeafletDropBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            delay_frames: 1,
            disabled_duration: 0,
            radius: 60.0,
            leaflet_fx_particle_system: None,
        }
    }
}

crate::impl_behavior_module_data_via_base!(LeafletDropBehaviorModuleData, base);

impl LeafletDropBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LEAFLET_DROP_BEHAVIOR_FIELDS)
    }
}

fn parse_duration_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    setter(INI::parse_duration_unsigned_int(required_value(tokens)?)?);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    setter(INI::parse_real(required_value(tokens)?)?);
    Ok(())
}

fn parse_leaflet_fx_particle_system(
    _ini: &mut INI,
    data: &mut LeafletDropBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.leaflet_fx_particle_system = None;
    } else {
        data.leaflet_fx_particle_system = Some(AsciiString::from(token));
    }
    Ok(())
}

const LEAFLET_DROP_BEHAVIOR_FIELDS: &[FieldParse<LeafletDropBehaviorModuleData>] = &[
    FieldParse {
        token: "Delay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.delay_frames = v, tokens)
        },
    },
    FieldParse {
        token: "DisabledDuration",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.disabled_duration = v, tokens)
        },
    },
    FieldParse {
        token: "AffectRadius",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.radius = v, tokens),
    },
    FieldParse {
        token: "LeafletFXParticleSystem",
        parse: parse_leaflet_fx_particle_system,
    },
];

#[derive(Debug)]
pub struct LeafletDropBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<LeafletDropBehaviorModuleData>,
    start_frame: UnsignedInt,
    fx_fired: Bool,
}

impl LeafletDropBehavior {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<LeafletDropBehaviorModuleData>()
            .ok_or("Invalid module data")?;

        let now = TheGameLogic::get_frame();
        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(data.clone()),
            start_frame: now.saturating_add(data.delay_frames),
            fx_fired: false,
        })
    }

    fn do_disable_attack(&self, obj: &GameObject) {
        let Some(partition) = ThePartitionManager::get() else {
            return;
        };
        let now = TheGameLogic::get_frame();
        let radius = self.module_data.radius;
        let candidates = partition.get_objects_in_range_boundary_3d(obj.get_position(), radius);

        for id in candidates {
            if id == obj.get_id() {
                continue;
            }

            let duration = self.module_data.disabled_duration;
            let _ = OBJECT_REGISTRY.with_object_mut(id, |target| {
                if !(target.is_kind_of(crate::common::KindOf::Infantry)
                    || target.is_kind_of(crate::common::KindOf::Vehicle))
                {
                    return;
                }
                if target.relationship_to(obj) != Relationship::Enemies {
                    return;
                }
                target.set_disabled_until(DisabledType::DisabledEmp, now + duration);
            });
        }
    }
}

impl UpdateModuleInterface for LeafletDropBehavior {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(obj_arc) = self.object.upgrade() else {
            return UpdateSleepTime::None;
        };
        let Ok(obj) = obj_arc.read() else {
            return UpdateSleepTime::None;
        };

        if !self.fx_fired {
            if let Some(manager) = TheParticleSystemManager::get() {
                if let Some(name) = self.module_data.leaflet_fx_particle_system.as_ref() {
                    if let Some(id) = manager.create_particle_system(Some(name.as_ref())) {
                        manager.attach_particle_system_to_object(id, obj.get_id());
                    }
                }
            }
            self.fx_fired = true;
        }

        let now = TheGameLogic::get_frame();
        if now < self.start_frame {
            return UpdateSleepTime::Forever;
        }

        self.do_disable_attack(&obj);
        UpdateSleepTime::None
    }
}

impl DieModuleInterface for LeafletDropBehavior {
    fn on_die(
        &mut self,
        _damage: &crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj) = obj_arc.read() {
                self.do_disable_attack(&obj);
            }
        }
        Ok(())
    }
}

impl BehaviorModuleInterface for LeafletDropBehavior {
    fn get_module_name(&self) -> &'static str {
        "LeafletDropBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for LeafletDropBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.start_frame)
            .map_err(|e| format!("Failed to xfer start_frame: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct LeafletDropBehaviorFactory;
impl LeafletDropBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(LeafletDropBehavior::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaflet_drop_fields_use_cpp_ini_token_handling() {
        let mut data = LeafletDropBehaviorModuleData::default();
        let mut ini = INI::new();

        parse_duration_field(&mut ini, &mut |v| data.delay_frames = v, &["=", "1s"])
            .expect("delay");
        parse_duration_field(
            &mut ini,
            &mut |v| data.disabled_duration = v,
            &["=", "2500ms"],
        )
        .expect("disabled duration");
        parse_real_field(&mut ini, &mut |v| data.radius = v, &["=", "90.5f"]).expect("radius");
        parse_leaflet_fx_particle_system(&mut ini, &mut data, &["=", "LeafletFX"])
            .expect("particle system");

        assert_eq!(data.delay_frames, 30);
        assert_eq!(data.disabled_duration, 75);
        assert!((data.radius - 90.5).abs() < f32::EPSILON);
        assert_eq!(
            data.leaflet_fx_particle_system.as_ref().map(|s| s.as_str()),
            Some("LeafletFX")
        );
    }

    #[test]
    fn leaflet_drop_rejects_missing_values_and_none_clears_particle_template() {
        let mut data = LeafletDropBehaviorModuleData::default();
        let mut ini = INI::new();

        let err =
            parse_duration_field(&mut ini, &mut |_| {}, &["="]).expect_err("missing duration");
        assert!(matches!(err, INIError::InvalidData));

        data.leaflet_fx_particle_system = Some(AsciiString::from("LeafletFX"));
        parse_leaflet_fx_particle_system(&mut ini, &mut data, &["=", "NONE"]).expect("none");
        assert!(data.leaflet_fx_particle_system.is_none());
    }
}

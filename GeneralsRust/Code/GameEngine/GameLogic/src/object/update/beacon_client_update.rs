// BeaconClientUpdate - client-side beacon pulses and smoke.
// Ported from C++ BeaconClientUpdate.cpp/.h.

use crate::common::{Color, Coord3D, ObjectID, Real, UnsignedInt, LOGICFRAMES_PER_SECOND};
use crate::helpers::{TheGameLogic, TheParticleSystemManager};
use crate::object::drawable::DrawableArcExt;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::radar::{
    get_radar_system, Coord3D as RadarCoord3D, RadarEventType,
};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BeaconClientUpdateConfig, ClientUpdateInterface, Module, ModuleData, NameKeyType,
};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BeaconClientUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    pub frames_between_radar_pulses: UnsignedInt,
    pub radar_pulse_duration: UnsignedInt,
}

impl Default for BeaconClientUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            frames_between_radar_pulses: 30,
            radar_pulse_duration: 15,
        }
    }
}

impl BeaconClientUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BEACON_CLIENT_UPDATE_FIELDS)
    }

    fn from_config(config: BeaconClientUpdateConfig) -> Self {
        Self {
            module_tag_name_key: 0,
            frames_between_radar_pulses: config.frames_between_radar_pulses,
            radar_pulse_duration: config.radar_pulse_duration,
        }
    }
}

impl Snapshotable for BeaconClientUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl crate::common::LegacyModuleData for BeaconClientUpdateModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl ModuleData for BeaconClientUpdateModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn get_beacon_client_update_config(&self) -> Option<BeaconClientUpdateConfig> {
        Some(BeaconClientUpdateConfig {
            frames_between_radar_pulses: self.frames_between_radar_pulses,
            radar_pulse_duration: self.radar_pulse_duration,
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl crate::common::types::ModuleData for BeaconClientUpdateModuleData {}

fn parse_radar_pulse_frequency(
    _ini: &mut INI,
    data: &mut BeaconClientUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.frames_between_radar_pulses = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_radar_pulse_duration(
    _ini: &mut INI,
    data: &mut BeaconClientUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.radar_pulse_duration = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

const BEACON_CLIENT_UPDATE_FIELDS: &[FieldParse<BeaconClientUpdateModuleData>] = &[
    FieldParse {
        token: "RadarPulseFrequency",
        parse: parse_radar_pulse_frequency,
    },
    FieldParse {
        token: "RadarPulseDuration",
        parse: parse_radar_pulse_duration,
    },
];

pub struct BeaconClientUpdateModule {
    module_name_key: NameKeyType,
    module_data: Arc<BeaconClientUpdateModuleData>,
    owner_id: ObjectID,
    particle_system_id: Option<u32>,
    last_radar_pulse: UnsignedInt,
}

impl BeaconClientUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        module_data: Arc<BeaconClientUpdateModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            module_data,
            owner_id,
            particle_system_id: None,
            last_radar_pulse: TheGameLogic::get_frame(),
        }
    }

    pub fn from_module_data(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        owner_id: ObjectID,
    ) -> Option<Self> {
        module_data
            .get_beacon_client_update_config()
            .map(BeaconClientUpdateModuleData::from_config)
            .map(|data| Self::new(module_name_key, Arc::new(data), owner_id))
    }

    pub fn hide_beacon(&mut self) {
        let Some(object) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };
        if let Some(drawable) = obj_guard.get_drawable() {
            drawable.set_drawable_hidden(true);
            drawable.set_shadows_enabled(false);
        }

        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if self.particle_system_id.is_none() {
                if let Some(drawable) = obj_guard.get_drawable() {
                    self.particle_system_id =
                        self.create_particle_system(&drawable, obj_guard.get_indicator_color());
                }
            }

            if let Some(system_id) = self.particle_system_id {
                ps_manager.stop_particle_system(system_id);
            }
        }
    }

    pub fn client_update(&mut self) {
        let Some(object) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };
        let Some(drawable) = obj_guard.get_drawable() else {
            return;
        };

        if self.particle_system_id.is_none() {
            self.particle_system_id =
                self.create_particle_system(&drawable, obj_guard.get_indicator_color());
        }

        if drawable.is_drawable_effectively_hidden() {
            return;
        }

        let now = TheGameLogic::get_frame();
        if now > self.last_radar_pulse + self.module_data.frames_between_radar_pulses {
            if let Ok(mut radar) = get_radar_system().write() {
                let pos = drawable
                    .read()
                    .ok()
                    .map(|guard| guard.get_position())
                    .unwrap_or(Coord3D::ZERO);
                let duration =
                    self.module_data.radar_pulse_duration as Real / LOGICFRAMES_PER_SECOND as Real;
                let radar_pos = RadarCoord3D {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                };
                radar.create_event(&radar_pos, RadarEventType::BeaconPulse, duration);
            }

            self.last_radar_pulse = now;
        }
    }

    fn create_particle_system(
        &self,
        drawable: &Arc<std::sync::RwLock<crate::object::drawable::Drawable>>,
        color: crate::common::Color,
    ) -> Option<u32> {
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return None;
        };

        let (template, tint_color) = Self::resolve_smoke_template(ps_manager, color)?;
        let system_id = ps_manager.create_particle_system(Some(&template))?;
        ps_manager.attach_particle_system_to_drawable(system_id, self.owner_id);
        if let Some(tint_color) = tint_color {
            ps_manager.tint_particle_system_all_colors(system_id, tint_color);
        }

        if let Ok(draw_guard) = drawable.read() {
            ps_manager.set_particle_system_position(system_id, &draw_guard.get_position());
        }

        Some(system_id)
    }

    fn resolve_smoke_template(
        ps_manager: &TheParticleSystemManager,
        color: Color,
    ) -> Option<(String, Option<Color>)> {
        Self::resolve_smoke_template_with_lookup(color, |name| {
            ps_manager.find_template(name).is_some()
        })
    }

    fn resolve_smoke_template_with_lookup(
        color: Color,
        mut template_exists: impl FnMut(&str) -> bool,
    ) -> Option<(String, Option<Color>)> {
        let rgb = ((color.r as u32) << 16) | ((color.g as u32) << 8) | color.b as u32;
        let exact = format!("BeaconSmoke{rgb:06X}");
        if template_exists(&exact) {
            return Some((exact, None));
        }

        let fallback = "BeaconSmokeFFFFFF";
        template_exists(fallback).then(|| (fallback.to_string(), Some(color)))
    }
}

impl Module for BeaconClientUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn get_client_update_interface(&mut self) -> Option<&mut dyn ClientUpdateInterface> {
        Some(self)
    }
}

impl ClientUpdateInterface for BeaconClientUpdateModule {
    fn client_update(&mut self) -> bool {
        BeaconClientUpdateModule::client_update(self);
        true
    }

    fn hide_beacon(&mut self) -> bool {
        BeaconClientUpdateModule::hide_beacon(self);
        true
    }
}

impl Snapshotable for BeaconClientUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut particle_system_id = self.particle_system_id.unwrap_or(0);
        xfer.xfer_unsigned_int(&mut particle_system_id)
            .map_err(|e| format!("{:?}", e))?;
        if xfer.is_reading() {
            self.particle_system_id = if particle_system_id == 0 {
                None
            } else {
                Some(particle_system_id)
            };
        }

        xfer.xfer_unsigned_int(&mut self.last_radar_pulse)
            .map_err(|e| format!("{:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn parse_radar_pulse_fields_accept_duration_suffixes() {
        let mut ini = INI::new();
        let mut data = BeaconClientUpdateModuleData::default();
        parse_radar_pulse_frequency(&mut ini, &mut data, &["1.5s"]).expect("frequency");
        parse_radar_pulse_duration(&mut ini, &mut data, &["500ms"]).expect("duration");
        assert_eq!(data.frames_between_radar_pulses, 45);
        assert_eq!(data.radar_pulse_duration, 15);
    }

    #[test]
    fn beacon_smoke_uses_exact_house_color_template_when_available() {
        let color = Color {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0xff,
        };
        let resolved =
            BeaconClientUpdateModule::resolve_smoke_template_with_lookup(color, |name| {
                name == "BeaconSmoke123456"
            });

        assert_eq!(resolved, Some(("BeaconSmoke123456".to_string(), None)));
    }

    #[test]
    fn beacon_smoke_falls_back_to_white_template_with_tint() {
        let color = Color {
            r: 0x0a,
            g: 0xb0,
            b: 0xff,
            a: 0xff,
        };
        let resolved =
            BeaconClientUpdateModule::resolve_smoke_template_with_lookup(color, |name| {
                name == "BeaconSmokeFFFFFF"
            });

        assert_eq!(
            resolved,
            Some(("BeaconSmokeFFFFFF".to_string(), Some(color)))
        );
    }

    #[test]
    fn beacon_client_update_xfer_preserves_cpp_runtime_fields() {
        let module_data = Arc::new(BeaconClientUpdateModuleData::default());
        let mut saved = BeaconClientUpdateModule::new(11, module_data.clone(), 22);
        saved.particle_system_id = Some(0x1234_5678);
        saved.last_radar_pulse = 9876;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("beacon_client_update").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = BeaconClientUpdateModule::new(11, module_data, 22);
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("beacon_client_update").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        assert_eq!(loaded.particle_system_id, Some(0x1234_5678));
        assert_eq!(loaded.last_radar_pulse, 9876);
    }

    #[test]
    fn beacon_client_update_exposes_typed_client_update_interface() {
        let module_data = Arc::new(BeaconClientUpdateModuleData::default());
        let mut module = BeaconClientUpdateModule::new(11, module_data, 22);

        assert!(module.get_client_update_interface().is_some());
    }

    #[test]
    fn beacon_client_update_builds_from_erased_module_data() {
        let mut data = BeaconClientUpdateModuleData::default();
        data.frames_between_radar_pulses = 90;
        data.radar_pulse_duration = 12;
        let module =
            BeaconClientUpdateModule::from_module_data(11, Arc::new(data), 22).expect("module");

        assert_eq!(module.module_data.frames_between_radar_pulses, 90);
        assert_eq!(module.module_data.radar_pulse_duration, 12);
    }
}

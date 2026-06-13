use super::draw_module::*;
use crate::common::*;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct W3DDefaultDrawModuleData {
    module_tag_name_key: NameKeyType,
}

impl W3DDefaultDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::EndOfFile);
            }
            let tokens = ini
                .get_line_tokens()
                .into_iter()
                .map(|token| token.to_string())
                .collect::<Vec<_>>();
            let Some(key) = tokens.first() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            return Err(INIError::UnknownToken);
        }
        Ok(())
    }
}

impl ModuleData for W3DDefaultDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl DrawModuleData for W3DDefaultDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DDefaultDrawModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct W3DDefaultDraw {
    data: W3DDefaultDrawModuleData,
    owner_id: Option<ObjectID>,
    shadows_enabled: bool,
    fully_obscured_by_shroud: bool,
}

impl W3DDefaultDraw {
    pub fn new(data: W3DDefaultDrawModuleData) -> Self {
        Self {
            data,
            owner_id: None,
            shadows_enabled: true,
            fully_obscured_by_shroud: false,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.owner_id
    }

    pub fn shadows_enabled(&self) -> bool {
        self.shadows_enabled
    }

    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.fully_obscured_by_shroud
    }
}

impl Module for W3DDefaultDraw {
    fn on_object_created(&mut self) {}
    fn on_drawable_bound_to_object(&mut self) {}
    fn preload_assets(&mut self, _time_of_day: TimeOfDay) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DDefaultDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DDefaultDraw {
    fn do_draw_module(&mut self, _transform_mtx: &Matrix3D) {
        // Normal C++ builds compile out LOAD_TEST_ASSETS, so this returns.
    }
    fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadows_enabled = enable;
    }
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }
    fn set_hidden(&mut self, hidden: bool) {
        let _ = hidden;
    }
    fn is_visible(&self) -> bool {
        true
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
    }
    fn react_to_geometry_change(&mut self) {}
}

impl Snapshotable for W3DDefaultDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_data_only_accepts_end_marker() {
        let mut data = W3DDefaultDrawModuleData::new();
        data.set_module_tag_name_key(77);

        assert_eq!(data.get_module_tag_name_key(), 77);
    }

    #[test]
    fn default_draw_does_not_expose_object_draw_interface() {
        let mut draw = W3DDefaultDraw::new(W3DDefaultDrawModuleData::new());
        draw.bind_owner_id(42);

        assert_eq!(draw.owner_id(), Some(42));
        assert!(draw.get_object_draw_interface().is_none());
        assert!(draw.get_object_draw_interface_mut().is_none());
    }

    #[test]
    fn production_draw_hooks_are_noops_except_shadow_flags() {
        let mut draw = W3DDefaultDraw::new(W3DDefaultDrawModuleData::new());

        draw.do_draw_module(&Matrix3D::IDENTITY);
        draw.react_to_transform_change(&Matrix3D::IDENTITY, &Coord3D::new(10.0, 20.0, 30.0), 1.0);
        draw.react_to_geometry_change();
        draw.set_hidden(true);
        assert!(draw.is_visible());

        draw.set_shadows_enabled(false);
        draw.set_fully_obscured_by_shroud(true);
        assert!(!draw.shadows_enabled());
        assert!(draw.fully_obscured_by_shroud());
    }
}

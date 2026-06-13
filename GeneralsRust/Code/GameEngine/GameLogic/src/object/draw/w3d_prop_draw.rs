use super::draw_module::*;
use crate::common::*;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct W3DPropDrawModuleData {
    module_tag_name_key: NameKeyType,
    pub model_name: AsciiString,
}
impl W3DPropDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            model_name: AsciiString::new(),
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
                .map(|t| t.to_string())
                .collect::<Vec<_>>();
            let Some(key) = tokens.first().cloned() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            let values = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|t| *t != "=")
                .collect::<Vec<_>>();
            match key.to_ascii_uppercase().as_str() {
                "MODELNAME" => {
                    let parsed = INI::parse_ascii_string(
                        values.first().copied().ok_or(INIError::InvalidData)?,
                    )?;
                    self.model_name = AsciiString::from(parsed.as_str())
                }
                _ => return Err(INIError::UnknownToken),
            }
        }
        Ok(())
    }
}
impl ModuleData for W3DPropDrawModuleData {
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
impl DrawModuleData for W3DPropDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DPropDrawModuleData {
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

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainPropAddRequest {
    /// Drawable id passed to `TheTerrainRenderObject->addProp`.
    pub drawable_id: ObjectID,
    /// Current drawable position.
    pub location: Coord3D,
    /// Current drawable orientation.
    pub orientation: Real,
    /// Current drawable scale.
    pub scale: Real,
    /// W3D model name from module data.
    pub model_name: AsciiString,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module_data(model_name: &str) -> W3DPropDrawModuleData {
        let mut data = W3DPropDrawModuleData::new();
        data.model_name = AsciiString::from(model_name);
        data
    }

    #[test]
    fn transform_at_origin_does_not_add_prop() {
        let mut draw = W3DPropDraw::new(module_data("Tree01"));
        draw.bind_owner_id(42);

        draw.react_to_current_transform(Coord3D::new(0.0, 0.0, 5.0), 1.0, 2.0);

        assert!(!draw.prop_added());
        assert_eq!(draw.take_pending_prop_add(), None);
    }

    #[test]
    fn current_transform_adds_terrain_prop_once() {
        let mut draw = W3DPropDraw::new(module_data("Tree01"));
        draw.bind_owner_id(42);

        draw.react_to_current_transform(Coord3D::new(10.0, 20.0, 5.0), 1.5, 2.0);
        draw.react_to_current_transform(Coord3D::new(30.0, 40.0, 5.0), 3.5, 4.0);

        assert!(draw.prop_added());
        assert_eq!(
            draw.take_pending_prop_add(),
            Some(TerrainPropAddRequest {
                drawable_id: 42,
                location: Coord3D::new(10.0, 20.0, 5.0),
                orientation: 1.5,
                scale: 2.0,
                model_name: AsciiString::from("Tree01"),
            })
        );
        assert_eq!(draw.take_pending_prop_add(), None);
    }

    #[test]
    fn do_draw_module_is_cpp_noop() {
        let mut draw = W3DPropDraw::new(module_data("Tree01"));
        draw.bind_owner_id(42);

        draw.do_draw_module(&Matrix3D::IDENTITY);

        assert!(!draw.prop_added());
        assert_eq!(draw.take_pending_prop_add(), None);
    }

    #[test]
    fn hidden_and_shroud_do_not_affect_visibility_for_cpp_prop_draw() {
        let mut draw = W3DPropDraw::new(module_data("Tree01"));

        draw.set_hidden(true);
        draw.set_fully_obscured_by_shroud(true);

        assert!(draw.is_visible());
    }
}

pub struct W3DPropDraw {
    data: W3DPropDrawModuleData,
    owner_id: Option<ObjectID>,
    prop_added: bool,
    pending_prop_add: Option<TerrainPropAddRequest>,
}
impl W3DPropDraw {
    pub fn new(data: W3DPropDrawModuleData) -> Self {
        Self {
            data,
            owner_id: None,
            prop_added: false,
            pending_prop_add: None,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    pub fn react_to_current_transform(
        &mut self,
        position: Coord3D,
        orientation: Real,
        scale: Real,
    ) {
        if self.prop_added {
            return;
        }
        if position.x == 0.0 && position.y == 0.0 {
            return;
        }
        self.prop_added = true;

        let Some(drawable_id) = self.owner_id else {
            return;
        };
        if self.data.model_name.is_empty() {
            return;
        }

        self.pending_prop_add = Some(TerrainPropAddRequest {
            drawable_id,
            location: position,
            orientation,
            scale,
            model_name: self.data.model_name.clone(),
        });
    }

    pub fn take_pending_prop_add(&mut self) -> Option<TerrainPropAddRequest> {
        self.pending_prop_add.take()
    }

    pub fn prop_added(&self) -> bool {
        self.prop_added
    }
}
impl Module for W3DPropDraw {
    fn on_object_created(&mut self) {}
    fn on_drawable_bound_to_object(&mut self) {}
    fn preload_assets(&mut self, _time_of_day: TimeOfDay) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DPropDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}
impl DrawModule for W3DPropDraw {
    fn do_draw_module(&mut self, _transform_mtx: &Matrix3D) {
        // C++ W3DPropDraw::doDrawModule returns immediately.  The terrain prop is
        // inserted once from reactToTransformChange instead.
    }
    fn set_shadows_enabled(&mut self, _enable: bool) {}
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        let _ = fully_obscured;
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
        old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        self.react_to_current_transform(*old_pos, _old_angle, 1.0);
    }
    fn react_to_geometry_change(&mut self) {}
}
impl Snapshotable for W3DPropDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
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

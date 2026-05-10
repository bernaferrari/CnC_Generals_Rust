use super::draw_module::*;
use super::w3d_truck_draw::*;
use crate::common::*;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DTankTruckDrawModuleData {
    pub base: W3DTruckDrawModuleData,
    pub tread_debris_name_left: AsciiString,
    pub tread_debris_name_right: AsciiString,
    pub tread_animation_rate: Real,
    pub tread_pivot_speed_fraction: Real,
    pub tread_drive_speed_fraction: Real,
}

impl W3DTankTruckDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DTruckDrawModuleData::new(),
            tread_debris_name_left: AsciiString::from("TrackDebrisDirtLeft"),
            tread_debris_name_right: AsciiString::from("TrackDebrisDirtRight"),
            tread_animation_rate: 0.0,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
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
            let handled = match key.to_ascii_uppercase().as_str() {
                "DUST" => {
                    self.base.dust_effect_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "DIRTSPRAY" => {
                    self.base.dirt_effect_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "POWERSLIDESPRAY" => {
                    self.base.powerslide_effect_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "LEFTFRONTTIREBONE" => {
                    self.base.front_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "RIGHTFRONTTIREBONE" => {
                    self.base.front_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "LEFTREARTIREBONE" => {
                    self.base.rear_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "RIGHTREARTIREBONE" => {
                    self.base.rear_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDLEFTFRONTTIREBONE" => {
                    self.base.mid_front_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDRIGHTFRONTTIREBONE" => {
                    self.base.mid_front_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDLEFTREARTIREBONE" => {
                    self.base.mid_rear_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDRIGHTREARTIREBONE" => {
                    self.base.mid_rear_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "TIREROTATIONMULTIPLIER" => {
                    self.base.rotation_speed_multiplier =
                        INI::parse_real(required_value(&values)?)?;
                    true
                }
                "POWERSLIDEROTATIONADDITION" => {
                    self.base.powerslide_rotation_addition =
                        INI::parse_real(required_value(&values)?)?;
                    true
                }
                "TREADDEBRISLEFT" => {
                    self.tread_debris_name_left = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "TREADDEBRISRIGHT" => {
                    self.tread_debris_name_right = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "TREADANIMATIONRATE" => {
                    self.tread_animation_rate = INI::parse_velocity_real(required_value(&values)?)?;
                    true
                }
                "TREADPIVOTSPEEDFRACTION" => {
                    self.tread_pivot_speed_fraction = INI::parse_real(required_value(&values)?)?;
                    true
                }
                "TREADDRIVESPEEDFRACTION" => {
                    self.tread_drive_speed_fraction = INI::parse_real(required_value(&values)?)?;
                    true
                }
                _ => false,
            };
            if !handled && !self.base.base.parse_ini_field(ini, key.as_str(), &values)? {
                return Err(INIError::UnknownToken);
            }
        }
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|t| !t.is_empty())
        .ok_or(INIError::InvalidData)
}

impl Default for W3DTankTruckDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DTankTruckDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl DrawModuleData for W3DTankTruckDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DTankTruckDrawModuleData {
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

pub struct W3DTankTruckDraw {
    data: W3DTankTruckDrawModuleData,
    base: W3DTruckDraw,
}

impl W3DTankTruckDraw {
    pub fn new(data: W3DTankTruckDrawModuleData) -> Self {
        Self {
            base: W3DTruckDraw::new(data.base.clone()),
            data,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }
}

impl Module for W3DTankTruckDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
    }
    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }
    fn on_delete(&mut self) {
        self.base.on_delete();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DTankTruckDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        self.base.do_draw_module(transform_mtx);
    }
    fn set_shadows_enabled(&mut self, enable: bool) {
        self.base.set_shadows_enabled(enable);
    }
    fn release_shadows(&mut self) {
        self.base.release_shadows();
    }
    fn allocate_shadows(&mut self) {
        self.base.allocate_shadows();
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.base.set_fully_obscured_by_shroud(fully_obscured);
    }
    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn react_to_transform_change(
        &mut self,
        old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        old_angle: Real,
    ) {
        self.base
            .react_to_transform_change(old_mtx, old_pos, old_angle);
    }
    fn react_to_geometry_change(&mut self) {}
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface()
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface_mut()
    }
}

impl Snapshotable for W3DTankTruckDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

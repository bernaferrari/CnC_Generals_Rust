use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DSupplyDrawModuleData {
    pub base: W3DModelDrawModuleData,
    pub supply_bone_prefix: AsciiString,
}
impl W3DSupplyDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
            supply_bone_prefix: AsciiString::new(),
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
            let value_tokens = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|t| *t != "=")
                .collect::<Vec<_>>();
            match key.to_ascii_uppercase().as_str() {
                "SUPPLYBONEPREFIX" => {
                    let parsed = INI::parse_ascii_string(required_value(&value_tokens)?)?;
                    self.supply_bone_prefix = AsciiString::from(parsed.as_str())
                }
                _ => {
                    if !self
                        .base
                        .parse_ini_field(ini, key.as_str(), &value_tokens)?
                    {
                        return Err(INIError::UnknownToken);
                    }
                }
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
impl Default for W3DSupplyDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
impl ModuleData for W3DSupplyDrawModuleData {
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
impl DrawModuleData for W3DSupplyDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DSupplyDrawModuleData {
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

pub struct W3DSupplyDraw {
    data: W3DSupplyDrawModuleData,
    base: W3DModelDraw,
    total_bones: i32,
    last_number_shown: i32,
}
impl W3DSupplyDraw {
    pub fn new(data: W3DSupplyDrawModuleData) -> Self {
        Self {
            base: W3DModelDraw::new(data.base.clone()),
            data,
            total_bones: -1,
            last_number_shown: 0,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }
}
impl Module for W3DSupplyDraw {
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
impl DrawModule for W3DSupplyDraw {
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
    fn react_to_geometry_change(&mut self) {
        self.base.react_to_geometry_change();
        self.total_bones = -1;
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(&self.base)
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base)
    }
}
impl ObjectDrawInterface for W3DSupplyDraw {
    fn client_only_get_render_obj_info(
        &self,
        pos: &mut Coord3D,
        r: &mut Real,
        transform: &mut Matrix3D,
    ) -> bool {
        self.base.client_only_get_render_obj_info(pos, r, transform)
    }
    fn client_only_get_render_obj_bound_box(&self, boundbox: &mut BoundingBox) -> bool {
        self.base.client_only_get_render_obj_bound_box(boundbox)
    }
    fn client_only_get_render_obj_bone_transform(
        &self,
        bone_name: &AsciiString,
        transform: &mut Matrix3D,
    ) -> bool {
        self.base
            .client_only_get_render_obj_bone_transform(bone_name, transform)
    }
    fn get_pristine_bone_positions(
        &self,
        condition: &ModelConditionFlags,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize {
        self.base.get_pristine_bone_positions(
            condition,
            bone_name_prefix,
            start_index,
            positions,
            transforms,
            max_bones,
        )
    }
    fn get_current_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Coord3D],
        transforms: &mut [Matrix3D],
        max_bones: usize,
    ) -> usize {
        self.base.get_current_bone_positions(
            bone_name_prefix,
            start_index,
            positions,
            transforms,
            max_bones,
        )
    }
    fn get_projectile_launch_offset(
        &self,
        condition: &ModelConditionFlags,
        weapon_slot: usize,
        barrel_index: i32,
        launch_pos: &mut Matrix3D,
        turret_type: TurretType,
        turret_rot_pos: &mut Coord3D,
        turret_pitch_pos: &mut Coord3D,
    ) -> bool {
        self.base.get_projectile_launch_offset(
            condition,
            weapon_slot,
            barrel_index,
            launch_pos,
            turret_type,
            turret_rot_pos,
            turret_pitch_pos,
        )
    }
    fn update_projectile_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: usize,
    ) {
        self.base
            .update_projectile_clip_status(shots_remaining, max_shots, weapon_slot);
    }
    fn update_supply_status(&mut self, max_supply: i32, current_supply: i32) {
        self.base.update_supply_status(max_supply, current_supply);
        if self.data.supply_bone_prefix.is_empty() || max_supply <= 0 {
            return;
        }
        if self.total_bones == -1 {
            let mut positions = vec![Coord3D::origin(); 128];
            let mut transforms = vec![Matrix3D::IDENTITY; 128];
            self.total_bones = self.base.get_pristine_bone_positions(
                &ModelConditionFlags::empty(),
                self.data.supply_bone_prefix.as_str(),
                1,
                &mut positions,
                &mut transforms,
                128,
            ) as i32;
            self.last_number_shown = self.total_bones;
        }
        let ratio = (current_supply.max(0) as Real) / (max_supply as Real);
        let bones_to_show = ((self.total_bones as Real) * ratio).ceil() as i32;
        let bones_to_show = bones_to_show.clamp(0, self.total_bones.max(0));
        if bones_to_show == self.last_number_shown {
            return;
        }
        let low = self.last_number_shown.min(bones_to_show);
        let high = self.last_number_shown.max(bones_to_show);
        let hide = bones_to_show < self.last_number_shown;
        for current in (low + 1)..=high {
            self.base.show_sub_object(
                &format!("{}{:02}", self.data.supply_bone_prefix.as_str(), current),
                !hide,
            );
        }
        self.base.update_sub_objects();
        self.last_number_shown = bones_to_show;
    }
    fn set_hidden(&mut self, hidden: bool) {
        ObjectDrawInterface::set_hidden(&mut self.base, hidden);
    }
    fn replace_model_condition_state(&mut self, condition: &ModelConditionFlags) {
        self.base.replace_model_condition_state(condition);
    }
    fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: usize,
        barrel_index: i32,
        victim_pos: &Coord3D,
    ) -> bool {
        self.base
            .handle_weapon_fire_fx(weapon_slot, barrel_index, victim_pos)
    }
    fn get_barrel_count(&self, weapon_slot: usize) -> i32 {
        self.base.get_barrel_count(weapon_slot)
    }
}
impl Snapshotable for W3DSupplyDraw {
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

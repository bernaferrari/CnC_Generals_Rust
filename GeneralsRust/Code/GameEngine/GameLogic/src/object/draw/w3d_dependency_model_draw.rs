use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::helpers::TheGameLogic;
use crate::object::drawable::DrawableArcExt;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DDependencyModelDrawModuleData {
    pub base: W3DModelDrawModuleData,
    pub attach_to_drawable_bone_in_container: AsciiString,
}
impl W3DDependencyModelDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
            attach_to_drawable_bone_in_container: AsciiString::new(),
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
                "ATTACHTOBONEINCONTAINER" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.attach_to_drawable_bone_in_container = AsciiString::from(parsed.as_str())
                }
                _ => {
                    if !self.base.parse_ini_field(ini, key.as_str(), &values)? {
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
impl Default for W3DDependencyModelDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
impl ModuleData for W3DDependencyModelDrawModuleData {
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
impl DrawModuleData for W3DDependencyModelDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DDependencyModelDrawModuleData {
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

pub struct W3DDependencyModelDraw {
    data: W3DDependencyModelDrawModuleData,
    base: W3DModelDraw,
    dependency_cleared: bool,
}
impl W3DDependencyModelDraw {
    pub fn new(data: W3DDependencyModelDrawModuleData) -> Self {
        Self {
            base: W3DModelDraw::new(data.base.clone()),
            data,
            dependency_cleared: false,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }
    fn adjusted_transform(&self, transform_mtx: &Matrix3D) -> Matrix3D {
        let Some(owner_id) = self.base.owner_id() else {
            return *transform_mtx;
        };
        let Some(owner) = TheGameLogic::find_object_by_id(owner_id) else {
            return *transform_mtx;
        };
        let Ok(owner_guard) = owner.read() else {
            return *transform_mtx;
        };
        let Some(container) = owner_guard.get_contained_by() else {
            return *transform_mtx;
        };
        let Some(container_arc) = TheGameLogic::find_object_by_id(container) else {
            return *transform_mtx;
        };
        let Ok(container_guard) = container_arc.read() else {
            return *transform_mtx;
        };
        let Some(contain) = container_guard.get_contain() else {
            return *transform_mtx;
        };
        let Ok(contain_guard) = contain.lock() else {
            return *transform_mtx;
        };
        if contain_guard.is_enclosing_container_for(&owner_guard) {
            return *transform_mtx;
        }
        drop(contain_guard);
        let Some(container_drawable) = container_guard.get_drawable() else {
            return *transform_mtx;
        };
        if !self.data.attach_to_drawable_bone_in_container.is_empty() {
            if let Ok(drawable_guard) = container_drawable.read() {
                if let Some(mtx) = drawable_guard.get_current_worldspace_client_bone_positions(
                    self.data.attach_to_drawable_bone_in_container.as_str(),
                ) {
                    return mtx;
                }
                return drawable_guard.get_transform_matrix();
            }
        }
        *transform_mtx
    }
}
impl Module for W3DDependencyModelDraw {
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
impl DrawModule for W3DDependencyModelDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        if !self.dependency_cleared {
            return;
        }
        let adjusted = self.adjusted_transform(transform_mtx);
        self.base.do_draw_module(&adjusted);
        self.dependency_cleared = false;
        if let Some(owner_id) = self
            .base
            .owner_id()
            .and_then(TheGameLogic::find_object_by_id)
        {
            if let Ok(owner_guard) = owner_id.read() {
                if let Some(container) = owner_guard.get_contained_by() {
                    if let Some(container_arc) = TheGameLogic::find_object_by_id(container) {
                        if let Ok(container_guard) = container_arc.read() {
                            if let Some(container_drawable) = container_guard.get_drawable() {
                                if let (Some(my_drawable), Ok(container_drawable_guard)) =
                                    (owner_guard.get_drawable(), container_drawable.read())
                                {
                                    if let Ok(mut my_drawable_guard) = my_drawable.write() {
                                        my_drawable_guard.set_stealth_look(
                                            container_drawable_guard.get_stealth_look(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(&self.base)
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base)
    }
}
impl ObjectDrawInterface for W3DDependencyModelDraw {
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
    }
    fn set_hidden(&mut self, hidden: bool) {
        ObjectDrawInterface::set_hidden(&mut self.base, hidden);
    }
    fn notify_draw_module_dependency_cleared(&mut self) {
        self.dependency_cleared = true;
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
impl Snapshotable for W3DDependencyModelDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)?;
        xfer.xfer_bool(&mut self.dependency_cleared)
            .map_err(|e| e.to_string())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

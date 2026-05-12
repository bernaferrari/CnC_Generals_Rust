// LaserUpdate - Handles laser update processing for render purposes and game control
// Author: Kris Morness, July 2002
// Ported to Rust

use crate::helpers::{TheGameLogic, TheParticleSystemManager};
use crate::object::draw::w3d_laser_draw::W3DLaserDraw;
use crate::object::drawable::DrawableArcExt;
use crate::object::ObjectArcExt;
use crate::player::ThePlayerList;
use crate::prelude::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    ClientUpdateInterface, LaserUpdateInterface, Module, ModuleData, NameKeyType,
};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct LaserUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    pub particle_system_name: String,
    pub target_particle_system_name: String,
    pub punch_through_scalar: f32,
}

impl Default for LaserUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            particle_system_name: String::new(),
            target_particle_system_name: String::new(),
            punch_through_scalar: 0.0,
        }
    }
}

impl LaserUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LASER_UPDATE_FIELDS)
    }
}

impl Snapshotable for LaserUpdateModuleData {
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

crate::impl_legacy_module_data_with_key_field!(LaserUpdateModuleData, module_tag_name_key);

fn parse_muzzle_particle_field(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.particle_system_name = INI::parse_ascii_string(value)?;
    Ok(())
}

fn parse_target_particle_field(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.target_particle_system_name = INI::parse_ascii_string(value)?;
    Ok(())
}

fn parse_punch_through_field(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.punch_through_scalar = INI::parse_real(value)?;
    Ok(())
}

const LASER_UPDATE_FIELDS: &[FieldParse<LaserUpdateModuleData>] = &[
    FieldParse {
        token: "MuzzleParticleSystem",
        parse: parse_muzzle_particle_field,
    },
    FieldParse {
        token: "TargetParticleSystem",
        parse: parse_target_particle_field,
    },
    FieldParse {
        token: "PunchThroughScalar",
        parse: parse_punch_through_field,
    },
];

#[derive(Debug, Clone)]
pub struct LaserUpdate {
    thing: ThingId,
    module_data: LaserUpdateModuleData,
    dirty: bool,
    start_pos: Coord3D,
    end_pos: Coord3D,
    particle_system_id: Option<ParticleSystemId>,
    target_particle_system_id: Option<ParticleSystemId>,
    widening: bool,
    widen_start_frame: u32,
    widen_finish_frame: u32,
    current_width_scalar: f32,
    decaying: bool,
    decay_start_frame: u32,
    decay_finish_frame: u32,
    parent_id: Option<DrawableId>,
    target_id: Option<DrawableId>,
    parent_bone_name: String,
}

/// Module wrapper for LaserUpdate client-update behavior.
pub struct LaserUpdateModule {
    module_name_key: NameKeyType,
    module_data: Arc<LaserUpdateModuleData>,
    update: LaserUpdate,
}

impl LaserUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        module_data: Arc<LaserUpdateModuleData>,
        owner: Option<ThingId>,
    ) -> Self {
        let thing_id = owner.unwrap_or(0);
        let update = LaserUpdate::new(thing_id, module_data.as_ref().clone());
        Self {
            module_name_key,
            module_data,
            update,
        }
    }

    pub fn update_mut(&mut self) -> &mut LaserUpdate {
        &mut self.update
    }
}

impl Module for LaserUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        ModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn get_client_update_interface(&mut self) -> Option<&mut dyn ClientUpdateInterface> {
        Some(self)
    }

    fn get_laser_update_interface(&mut self) -> Option<&mut dyn LaserUpdateInterface> {
        Some(self)
    }
}

impl ClientUpdateInterface for LaserUpdateModule {
    fn client_update(&mut self) -> bool {
        self.update.client_update();
        true
    }
}

impl LaserUpdateInterface for LaserUpdateModule {
    fn is_dirty(&self) -> bool {
        self.update.is_dirty()
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.update.set_dirty(dirty);
    }

    fn get_start_pos(&self) -> [f32; 3] {
        self.update.get_start_pos().to_array()
    }

    fn get_end_pos(&self) -> [f32; 3] {
        self.update.get_end_pos().to_array()
    }

    fn get_width_scale(&self) -> f32 {
        self.update.get_width_scale()
    }
}

impl Snapshotable for LaserUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let u = &mut self.update;

        let drawable_module_version = current_version;
        let mut drawable_version = drawable_module_version;
        xfer.xfer_version(&mut drawable_version, drawable_module_version)
            .map_err(|e| e.to_string())?;

        let module_version = current_version;
        let mut base_version = module_version;
        xfer.xfer_version(&mut base_version, module_version)
            .map_err(|e| e.to_string())?;

        xfer.xfer_real(&mut u.start_pos.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut u.start_pos.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut u.start_pos.z)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut u.end_pos.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut u.end_pos.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut u.end_pos.z)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut u.dirty).map_err(|e| e.to_string())?;

        let mut ps_id = u.particle_system_id.unwrap_or(0);
        xfer.xfer_unsigned_int(&mut ps_id)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            u.particle_system_id = if ps_id == 0 { None } else { Some(ps_id) };
        }

        let mut tps_id = u.target_particle_system_id.unwrap_or(0);
        xfer.xfer_unsigned_int(&mut tps_id)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            u.target_particle_system_id = if tps_id == 0 { None } else { Some(tps_id) };
        }

        xfer.xfer_bool(&mut u.widening).map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut u.decaying).map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut u.widen_start_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut u.widen_finish_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut u.current_width_scalar)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut u.decay_start_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut u.decay_finish_frame)
            .map_err(|e| e.to_string())?;

        let mut parent_id = u.parent_id.unwrap_or(0);
        xfer.xfer_drawable_id(&mut parent_id)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            u.parent_id = if parent_id == 0 {
                None
            } else {
                Some(parent_id)
            };
        }

        let mut target_id = u.target_id.unwrap_or(0);
        xfer.xfer_drawable_id(&mut target_id)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            u.target_id = if target_id == 0 {
                None
            } else {
                Some(target_id)
            };
        }

        xfer.xfer_ascii_string(&mut u.parent_bone_name)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl LaserUpdate {
    pub fn new(thing: ThingId, module_data: LaserUpdateModuleData) -> Self {
        Self {
            thing,
            module_data,
            dirty: false,
            start_pos: Coord3D::origin(),
            end_pos: Coord3D::origin(),
            particle_system_id: None,
            target_particle_system_id: None,
            widening: false,
            widen_start_frame: 0,
            widen_finish_frame: 0,
            current_width_scalar: 1.0,
            decaying: false,
            decay_start_frame: 0,
            decay_finish_frame: 0,
            parent_id: None,
            target_id: None,
            parent_bone_name: String::new(),
        }
    }

    pub fn init_laser(
        &mut self,
        parent: Option<&Object>,
        target: Option<&Object>,
        start_pos: Option<&Coord3D>,
        end_pos: Option<&Coord3D>,
        parent_bone_name: String,
        size_delta_frames: i32,
    ) {
        let now = TheGameLogic::get_frame();

        if size_delta_frames > 0 {
            self.widening = true;
            self.widen_start_frame = now;
            self.widen_finish_frame = now + size_delta_frames as u32;
            self.current_width_scalar = 0.0;
        } else if size_delta_frames < 0 {
            self.decaying = true;
            self.decay_start_frame = now;
            self.decay_finish_frame = now + (-size_delta_frames) as u32;
            self.current_width_scalar = 1.0;
        }

        self.parent_bone_name = parent_bone_name;

        // Record IDs if we have them, then figure out starting points
        if let Some(parent_obj) = parent {
            if let Some(drawable) = parent_obj.get_drawable() {
                self.parent_id = Some(drawable.get_id());
            }
            self.update_start_pos();
        } else if let Some(pos) = start_pos {
            self.start_pos = *pos;
        } else {
            // No start position available
            return;
        }

        // Handle target/end position
        if let Some(target_obj) = target {
            if end_pos.is_none() {
                if let Some(drawable) = target_obj.get_drawable() {
                    self.target_id = Some(drawable.get_id());
                }
                self.end_pos = *target_obj.get_position();
            }
        }

        if let Some(pos) = end_pos {
            self.end_pos = *pos;
        }

        // Create particle systems
        self.create_particle_systems(parent);

        self.dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn get_start_pos(&self) -> Coord3D {
        self.start_pos
    }

    pub fn get_end_pos(&self) -> Coord3D {
        self.end_pos
    }

    pub fn get_width_scale(&self) -> Real {
        self.current_width_scalar
    }

    fn update_start_pos(&mut self) {
        let Some(parent_id) = self.parent_id else {
            return;
        };

        let Some(parent_obj) = TheGameLogic::find_object_by_id(parent_id) else {
            return;
        };

        let old_start_pos = self.start_pos;

        if let Ok(parent_guard) = parent_obj.read() {
            if let Some(drawable) = parent_guard.get_drawable() {
                if let Ok(drawable_guard) = drawable.read() {
                    if !self.parent_bone_name.is_empty() {
                        if let Some(bone_matrix) = drawable_guard
                            .get_current_worldspace_client_bone_positions(&self.parent_bone_name)
                        {
                            let translation = bone_matrix.w_axis;
                            self.start_pos = Coord3D {
                                x: translation.x,
                                y: translation.y,
                                z: translation.z,
                            };
                        } else {
                            self.start_pos = drawable_guard.get_position();
                        }
                    } else {
                        self.start_pos = drawable_guard.get_position();
                    }
                }
            } else {
                self.start_pos = *parent_guard.get_position();
            }
        }

        if self.start_pos != old_start_pos {
            self.dirty = true;
        }
    }

    fn update_end_pos(&mut self) {
        let Some(target_id) = self.target_id else {
            return;
        };

        let old_end_pos = self.end_pos;

        let target_obj = TheGameLogic::find_object_by_id(target_id);
        let target_dead = target_obj
            .as_ref()
            .and_then(|obj| obj.read().ok())
            .map(|guard| guard.is_effectively_dead())
            .unwrap_or(true);

        if target_obj.is_none() || target_dead {
            // Target is gone. Punch through the old spot
            if self.module_data.punch_through_scalar > 0.0 {
                let laser_vector = Vector3::new(
                    self.end_pos.x - self.start_pos.x,
                    self.end_pos.y - self.start_pos.y,
                    self.end_pos.z - self.start_pos.z,
                ) * self.module_data.punch_through_scalar;

                self.end_pos.x = self.start_pos.x + laser_vector.x;
                self.end_pos.y = self.start_pos.y + laser_vector.y;
                self.end_pos.z = self.start_pos.z + laser_vector.z;
            }

            self.target_id = None;
        } else if let Some(target_obj) = target_obj {
            if let Ok(target_guard) = target_obj.read() {
                if let Some(drawable) = target_guard.get_drawable() {
                    if let Ok(drawable_guard) = drawable.read() {
                        self.end_pos = drawable_guard.get_position();
                    }
                } else {
                    self.end_pos = *target_guard.get_position();
                }
            }
        }

        if self.end_pos != old_end_pos {
            self.dirty = true;
        }
    }

    pub fn client_update(&mut self) {
        self.update_start_pos();
        self.update_end_pos();

        if self.dirty {
            if let Some(ps_manager) = TheParticleSystemManager::get() {
                if let Some(system_id) = self.particle_system_id {
                    ps_manager.set_particle_system_position(system_id, &self.start_pos);
                }
                if let Some(system_id) = self.target_particle_system_id {
                    ps_manager.set_particle_system_position(system_id, &self.end_pos);
                }
            }
        }

        let now = TheGameLogic::get_frame();

        if self.decaying {
            self.current_width_scalar = 1.0
                - (now - self.decay_start_frame) as f32
                    / (self.decay_finish_frame - self.decay_start_frame) as f32;
            self.dirty = true;

            if self.current_width_scalar <= 0.0 {
                self.current_width_scalar = 0.0;
                if let Some(ps_manager) = TheParticleSystemManager::get() {
                    if let Some(system_id) = self.particle_system_id.take() {
                        ps_manager.destroy_particle_system(system_id);
                    }
                    if let Some(system_id) = self.target_particle_system_id.take() {
                        ps_manager.destroy_particle_system(system_id);
                    }
                }
                // When decay is finished, delete the laser
                return;
            }
        } else if self.widening {
            self.current_width_scalar = (now - self.widen_start_frame) as f32
                / (self.widen_finish_frame - self.widen_start_frame) as f32;
            self.dirty = true;

            if self.current_width_scalar >= 1.0 {
                self.current_width_scalar = 1.0;
                self.widening = false;
            }
        }
    }

    pub fn set_decay_frames(&mut self, decay_frames: u32, current_frame: u32) {
        if decay_frames > 0 {
            self.decaying = true;
            self.decay_start_frame = current_frame;
            self.decay_finish_frame = current_frame + decay_frames;
            self.current_width_scalar = 1.0;
        }
    }

    pub fn get_current_laser_radius(&self) -> f32 {
        let Some(object) = TheGameLogic::find_object_by_id(self.thing) else {
            return 0.0;
        };

        if let Ok(object_guard) = object.read() {
            let Some(drawable) = object_guard.get_drawable() else {
                return 0.0;
            };

            for draw_module in drawable.get_draw_modules() {
                if let Some(width) =
                    draw_module.with_module_downcast::<W3DLaserDraw, _, f32>(|laser_draw| {
                        laser_draw.get_laser_template_width()
                    })
                {
                    return width * self.current_width_scalar;
                }
            }
        }

        0.0
    }

    fn create_particle_systems(&mut self, parent: Option<&Object>) {
        let local_visible = parent
            .and_then(|parent_obj| {
                let local_index = ThePlayerList()
                    .read()
                    .ok()
                    .map(|list| list.get_local_player_index())
                    .unwrap_or(-1);
                let shroud = parent_obj.get_shrouded_status(local_index);
                Some((shroud as u8) <= (ObjectShroudStatus::PartialClear as u8))
            })
            .unwrap_or(true);
        if !local_visible {
            return;
        }

        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if !self.module_data.particle_system_name.is_empty() {
                if let Some(system_id) =
                    ps_manager.create_particle_system(Some(&self.module_data.particle_system_name))
                {
                    self.particle_system_id = Some(system_id);
                    ps_manager.set_particle_system_position(system_id, &self.start_pos);
                }
            }

            if !self.module_data.target_particle_system_name.is_empty() {
                if let Some(system_id) = ps_manager
                    .create_particle_system(Some(&self.module_data.target_particle_system_name))
                {
                    self.target_particle_system_id = Some(system_id);
                    ps_manager.set_particle_system_position(system_id, &self.end_pos);
                }
            }
        }
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("LaserUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut start_pos = self.start_pos;
        xfer.xfer_coord3d(&mut start_pos);
        let mut end_pos = self.end_pos;
        xfer.xfer_coord3d(&mut end_pos);
        let mut dirty = self.dirty;
        xfer_io(xfer.xfer_bool(&mut dirty), "dirty");
        let mut particle_system_id = self.particle_system_id;
        xfer.xfer_option_particle_system_id("particle_system_id", &mut particle_system_id);
        let mut target_particle_system_id = self.target_particle_system_id;
        xfer.xfer_option_particle_system_id(
            "target_particle_system_id",
            &mut target_particle_system_id,
        );
        let mut widening = self.widening;
        xfer_io(xfer.xfer_bool(&mut widening), "widening");
        let mut decaying = self.decaying;
        xfer_io(xfer.xfer_bool(&mut decaying), "decaying");
        let mut widen_start_frame = self.widen_start_frame;
        xfer_io(xfer.xfer_u32(&mut widen_start_frame), "widen_start_frame");
        let mut widen_finish_frame = self.widen_finish_frame;
        xfer_io(xfer.xfer_u32(&mut widen_finish_frame), "widen_finish_frame");
        let mut current_width_scalar = self.current_width_scalar;
        xfer_io(
            xfer.xfer_f32(&mut current_width_scalar),
            "current_width_scalar",
        );
        let mut decay_start_frame = self.decay_start_frame;
        xfer_io(xfer.xfer_u32(&mut decay_start_frame), "decay_start_frame");
        let mut decay_finish_frame = self.decay_finish_frame;
        xfer_io(xfer.xfer_u32(&mut decay_finish_frame), "decay_finish_frame");
        let mut parent_id = self.parent_id;
        xfer.xfer_option_drawable_id("parent_id", &mut parent_id);
        let mut target_id = self.target_id;
        xfer.xfer_option_drawable_id("target_id", &mut target_id);
        let mut parent_bone_name = self.parent_bone_name.clone();
        xfer_io(xfer.xfer_string(&mut parent_bone_name), "parent_bone_name");
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("LaserUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer.xfer_coord3d(&mut self.start_pos);
            xfer.xfer_coord3d(&mut self.end_pos);
            xfer_io(xfer.xfer_bool(&mut self.dirty), "dirty");
            xfer.xfer_option_particle_system_id("particle_system_id", &mut self.particle_system_id);
            xfer.xfer_option_particle_system_id(
                "target_particle_system_id",
                &mut self.target_particle_system_id,
            );
            xfer_io(xfer.xfer_bool(&mut self.widening), "widening");
            xfer_io(xfer.xfer_bool(&mut self.decaying), "decaying");
            xfer_io(
                xfer.xfer_u32(&mut self.widen_start_frame),
                "widen_start_frame",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.widen_finish_frame),
                "widen_finish_frame",
            );
            xfer_io(
                xfer.xfer_f32(&mut self.current_width_scalar),
                "current_width_scalar",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.decay_start_frame),
                "decay_start_frame",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.decay_finish_frame),
                "decay_finish_frame",
            );
            xfer.xfer_option_drawable_id("parent_id", &mut self.parent_id);
            xfer.xfer_option_drawable_id("target_id", &mut self.target_id);
            xfer_io(
                xfer.xfer_string(&mut self.parent_bone_name),
                "parent_bone_name",
            );
        }
    }
}

impl Drop for LaserUpdate {
    fn drop(&mut self) {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if let Some(system_id) = self.particle_system_id {
                ps_manager.destroy_particle_system(system_id);
            }
            if let Some(system_id) = self.target_particle_system_id {
                ps_manager.destroy_particle_system(system_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn laser_update_xfer_preserves_cpp_runtime_fields() {
        let module_data = Arc::new(LaserUpdateModuleData::default());
        let mut saved = LaserUpdateModule::new(11, module_data.clone(), Some(22));
        let update = saved.update_mut();
        update.start_pos = Coord3D {
            x: 1.25,
            y: -2.5,
            z: 3.75,
        };
        update.end_pos = Coord3D {
            x: 4.5,
            y: -5.75,
            z: 6.125,
        };
        update.dirty = true;
        update.particle_system_id = Some(0x0102_0304);
        update.target_particle_system_id = Some(0x0506_0708);
        update.widening = true;
        update.decaying = true;
        update.widen_start_frame = 123;
        update.widen_finish_frame = 456;
        update.current_width_scalar = 0.625;
        update.decay_start_frame = 789;
        update.decay_finish_frame = 999;
        update.parent_id = Some(0x1111_2222);
        update.target_id = Some(0x3333_4444);
        update.parent_bone_name = "MuzzleFX01".to_string();

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("laser_update").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = LaserUpdateModule::new(11, module_data, Some(22));
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("laser_update").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        let update = loaded.update_mut();
        assert_eq!(
            update.start_pos,
            Coord3D {
                x: 1.25,
                y: -2.5,
                z: 3.75,
            }
        );
        assert_eq!(
            update.end_pos,
            Coord3D {
                x: 4.5,
                y: -5.75,
                z: 6.125,
            }
        );
        assert!(update.dirty);
        assert_eq!(update.particle_system_id, Some(0x0102_0304));
        assert_eq!(update.target_particle_system_id, Some(0x0506_0708));
        assert!(update.widening);
        assert!(update.decaying);
        assert_eq!(update.widen_start_frame, 123);
        assert_eq!(update.widen_finish_frame, 456);
        assert_eq!(update.current_width_scalar, 0.625);
        assert_eq!(update.decay_start_frame, 789);
        assert_eq!(update.decay_finish_frame, 999);
        assert_eq!(update.parent_id, Some(0x1111_2222));
        assert_eq!(update.target_id, Some(0x3333_4444));
        assert_eq!(update.parent_bone_name, "MuzzleFX01");
    }

    #[test]
    fn laser_update_xfer_loads_cpp_invalid_ids_as_none() {
        let module_data = Arc::new(LaserUpdateModuleData::default());
        let mut saved = LaserUpdateModule::new(11, module_data.clone(), Some(22));
        saved.update_mut().dirty = true;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("laser_update").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = LaserUpdateModule::new(11, module_data, Some(22));
        loaded.update_mut().particle_system_id = Some(1);
        loaded.update_mut().target_particle_system_id = Some(2);
        loaded.update_mut().parent_id = Some(3);
        loaded.update_mut().target_id = Some(4);
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("laser_update").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        let update = loaded.update_mut();
        assert_eq!(update.particle_system_id, None);
        assert_eq!(update.target_particle_system_id, None);
        assert_eq!(update.parent_id, None);
        assert_eq!(update.target_id, None);
    }

    #[test]
    fn laser_update_exposes_typed_client_update_interface() {
        let module_data = Arc::new(LaserUpdateModuleData::default());
        let mut module = LaserUpdateModule::new(11, module_data, Some(22));

        assert!(module.get_client_update_interface().is_some());
    }

    #[test]
    fn laser_update_exposes_typed_laser_update_interface() {
        let module_data = Arc::new(LaserUpdateModuleData::default());
        let mut module = LaserUpdateModule::new(11, module_data, Some(22));

        assert!(module.get_laser_update_interface().is_some());
    }
}

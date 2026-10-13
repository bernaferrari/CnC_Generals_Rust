// LaserUpdate - Handles laser update processing for render purposes and game control
// Author: Kris Morness, July 2002
// Ported to Rust

use crate::helpers::{TheGameClient, TheGameLogic, TheParticleSystemManager};
use crate::object::ObjectArcExt;
use crate::object::drawable::DrawableArcExt;
use crate::player::ThePlayerList;
use crate::prelude::*;
use game_engine::common::ini::{FieldParse, INI, INIError};
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_string(&mut self.particle_system_name)
            .map_err(|e| e.to_string())?;
        xfer.xfer_string(&mut self.target_particle_system_name)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.punch_through_scalar)
            .map_err(|e| e.to_string())?;
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

#[path = "laser_owned.rs"]
mod owned;
pub use owned::{LaserDrawableObservation, LaserExecution, LaserObjectObservation};

#[derive(Debug)]
pub struct LaserUpdate {
    thing: ThingId,
    external_particle_lifetime: bool,
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

    pub fn new_owned(
        module_name_key: NameKeyType,
        module_data: Arc<LaserUpdateModuleData>,
        owner: ThingId,
    ) -> Self {
        let update = LaserUpdate::new_owned(owner, module_data.as_ref().clone());
        Self {
            module_name_key,
            module_data,
            update,
        }
    }
    pub fn bind_owned_drawable(&mut self, owner: ThingId) {
        self.update.thing = owner;
    }
    pub fn tracked_drawables(&self) -> (Option<DrawableId>, Option<DrawableId>, &str) {
        (
            self.update.parent_id,
            self.update.target_id,
            &self.update.parent_bone_name,
        )
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
        // Receiving client modules execute through their supplied owner phase.
        // A generic compatibility drawable walk cannot select particle/client owners.
        if self.update.external_particle_lifetime {
            return false;
        }
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

    fn init_laser(
        &mut self,
        parent_id: Option<ObjectID>,
        target_id: Option<ObjectID>,
        start_pos: Option<[f32; 3]>,
        end_pos: Option<[f32; 3]>,
        parent_bone_name: String,
        size_delta_frames: i32,
    ) {
        let parent_arc = parent_id.and_then(TheGameLogic::find_object_by_id);
        let target_arc = target_id.and_then(TheGameLogic::find_object_by_id);
        let parent_guard = parent_arc.as_ref().and_then(|arc| arc.read().ok());
        let target_guard = target_arc.as_ref().and_then(|arc| arc.read().ok());
        let start_pos = start_pos.map(Coord3D::from_array);
        let end_pos = end_pos.map(Coord3D::from_array);

        self.update.init_laser(
            parent_guard.as_deref(),
            target_guard.as_deref(),
            start_pos.as_ref(),
            end_pos.as_ref(),
            parent_bone_name,
            size_delta_frames,
        );
    }

    fn set_decay_frames(&mut self, decay_frames: u32) {
        self.update.set_decay_frames(decay_frames);
    }
}

impl Snapshotable for LaserUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // LaserUpdate::crc only calls ClientUpdateModule::crc, which writes nothing.
        let _ = xfer;
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
            external_particle_lifetime: false,
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
        if let Err(error) = self.init_laser_with_services(
            parent.map(owned::object_observation),
            target.map(owned::object_observation),
            start_pos,
            end_pos,
            parent_bone_name,
            size_delta_frames,
            &mut owned::AmbientLaserExecution,
        ) {
            log::error!("LaserUpdate init failed: {error}");
        }
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

    pub fn set_loaded_segment(&mut self, start: Coord3D, end: Coord3D, dirty: bool) {
        self.start_pos = start;
        self.end_pos = end;
        self.dirty = dirty;
    }

    pub fn get_width_scale(&self) -> Real {
        self.current_width_scalar
    }

    fn update_start_pos(&mut self) {
        if let Err(error) = self.update_start_with_services(&owned::AmbientLaserExecution) {
            log::error!("LaserUpdate start observation failed: {error}");
        }
    }
    fn update_end_pos(&mut self) {
        if let Err(error) = self.update_end_with_services(&owned::AmbientLaserExecution) {
            log::error!("LaserUpdate end observation failed: {error}");
        }
    }

    pub fn client_update(&mut self) {
        if let Err(error) = self.client_update_with_services(&mut owned::AmbientLaserExecution) {
            log::error!("LaserUpdate tick failed: {error}");
        }
    }

    pub fn set_decay_frames(&mut self, decay_frames: u32) {
        self.set_decay_frames_at(decay_frames, TheGameLogic::get_frame());
    }

    pub fn get_current_laser_radius(&self) -> f32 {
        let Some(object) = TheGameLogic::find_object_by_id(self.thing) else {
            return 0.0;
        };

        if let Ok(object_guard) = object.read() {
            let Some(drawable) = object_guard.get_drawable() else {
                return 0.0;
            };

            if let Some(width) = drawable.get_laser_template_width() {
                return width * self.current_width_scalar;
            }
        }

        0.0
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
        if self.external_particle_lifetime {
            return;
        }
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
    use crate::common::Matrix3D;
    use crate::object::drawable::{Drawable, DrawableType};
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    use std::sync::{Mutex, RwLock};

    static DRAWABLE_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn register_test_drawable(id: u32, position: Coord3D) {
        let drawable = Arc::new(RwLock::new(Drawable::new(
            id,
            0x0bad_f00d,
            "TestDrawable".to_string(),
            DrawableType::Animated,
        )));
        drawable
            .write()
            .unwrap()
            .set_transform(Matrix3D::from_translation(position));
        TheGameClient::get()
            .unwrap()
            .register_drawable_arc_for_test(id, drawable);
    }

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
    fn laser_update_start_position_tracks_parent_drawable_id() {
        let _guard = DRAWABLE_TEST_LOCK.lock().unwrap();
        let client = TheGameClient::get().unwrap();
        let drawable_id = 0x0100_0001;
        client.destroy_drawable(drawable_id);

        let parent_position = Coord3D::new(12.0, -4.0, 7.5);
        register_test_drawable(drawable_id, parent_position);

        let mut update = LaserUpdate::new(99, LaserUpdateModuleData::default());
        update.start_pos = Coord3D::new(0.0, 0.0, 0.0);
        update.parent_id = Some(drawable_id);
        update.dirty = false;

        update.update_start_pos();

        assert_eq!(update.start_pos, parent_position);
        assert!(update.dirty);

        client.destroy_drawable(drawable_id);
    }

    #[test]
    fn laser_update_end_position_tracks_target_drawable_id() {
        let _guard = DRAWABLE_TEST_LOCK.lock().unwrap();
        let client = TheGameClient::get().unwrap();
        let drawable_id = 0x0100_0002;
        client.destroy_drawable(drawable_id);

        let target_position = Coord3D::new(-8.0, 3.25, 11.0);
        register_test_drawable(drawable_id, target_position);

        let mut update = LaserUpdate::new(99, LaserUpdateModuleData::default());
        update.end_pos = Coord3D::new(0.0, 0.0, 0.0);
        update.target_id = Some(drawable_id);
        update.dirty = false;

        update.update_end_pos();

        assert_eq!(update.end_pos, target_position);
        assert_eq!(update.target_id, Some(drawable_id));
        assert!(update.dirty);

        client.destroy_drawable(drawable_id);
    }

    #[test]
    fn laser_update_missing_target_drawable_clears_id_and_punches_through() {
        let _guard = DRAWABLE_TEST_LOCK.lock().unwrap();
        let client = TheGameClient::get().unwrap();
        let drawable_id = 0x0100_0003;
        client.destroy_drawable(drawable_id);

        let mut data = LaserUpdateModuleData::default();
        data.punch_through_scalar = 2.0;
        let mut update = LaserUpdate::new(99, data);
        update.start_pos = Coord3D::new(1.0, 2.0, 3.0);
        update.end_pos = Coord3D::new(4.0, 6.0, 3.0);
        update.target_id = Some(drawable_id);
        update.dirty = false;

        update.update_end_pos();

        assert_eq!(update.target_id, None);
        assert_eq!(update.end_pos, Coord3D::new(7.0, 10.0, 3.0));
        assert!(update.dirty);
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

    #[test]
    fn init_laser_parentless_sets_drawable_to_midpoint() {
        let _guard = DRAWABLE_TEST_LOCK.lock().unwrap();
        let client = TheGameClient::get().unwrap();
        let drawable_id = 0x0100_0004;
        client.destroy_drawable(drawable_id);
        register_test_drawable(drawable_id, Coord3D::ZERO);

        let mut update = LaserUpdate::new(drawable_id, LaserUpdateModuleData::default());
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(10.0, 4.0, 2.0);
        update.init_laser(None, None, Some(&start), Some(&end), String::new(), 0);

        let pos = client
            .find_drawable_by_id(drawable_id)
            .expect("drawable")
            .position;
        assert_eq!(pos, Coord3D::new(5.0, 2.0, 1.0));
        client.destroy_drawable(drawable_id);
    }

    #[test]
    fn init_laser_does_not_recreate_existing_particle_ids() {
        let mut data = LaserUpdateModuleData::default();
        data.particle_system_name = "Muzzle".to_string();
        data.target_particle_system_name = "Target".to_string();
        let mut update = LaserUpdate::new(1, data);
        update.particle_system_id = Some(42);
        update.target_particle_system_id = Some(43);
        let start = Coord3D::new(1.0, 2.0, 3.0);
        let end = Coord3D::new(4.0, 5.0, 6.0);
        update.init_laser(None, None, Some(&start), Some(&end), String::new(), 0);
        assert_eq!(update.particle_system_id, Some(42));
        assert_eq!(update.target_particle_system_id, Some(43));
    }

    #[test]
    fn set_decay_frames_starts_width_decay() {
        let mut update = LaserUpdate::new(1, LaserUpdateModuleData::default());
        update.set_decay_frames(60);
        assert!(update.decaying);
        assert_eq!(update.current_width_scalar, 1.0);
        assert_eq!(update.decay_finish_frame, update.decay_start_frame + 60);
    }

    #[test]
    fn laser_update_interface_exposes_set_decay_frames() {
        let module_data = Arc::new(LaserUpdateModuleData::default());
        let mut module = LaserUpdateModule::new(11, module_data, Some(22));
        module
            .get_laser_update_interface()
            .expect("laser interface")
            .set_decay_frames(30);
        assert!(module.update_mut().decaying);
    }
}

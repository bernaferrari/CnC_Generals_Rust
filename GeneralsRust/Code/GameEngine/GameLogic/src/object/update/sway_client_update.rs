// SwayClientUpdate - client-side sway for trees/props.
// Ported from C++ SwayClientUpdate.cpp/.h.

use crate::common::{ObjectID, ObjectStatusTypes};
use crate::drawable::Drawable;
use crate::helpers::{game_client_random_value_real, TheGameLogic};
use crate::object::drawable::DrawableArcExt;
use crate::scripting::engine::{get_script_engine, BreezeInfo};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use glam::Mat4;
use std::any::Any;
use std::f32::consts::PI;
use std::sync::Arc;

pub struct SwayClientUpdateModule {
    module_name_key: NameKeyType,
    module_data: Arc<dyn ModuleData>,
    owner_id: ObjectID,
    cur_value: f32,
    cur_angle: f32,
    cur_delta: f32,
    cur_angle_limit: f32,
    lean_angle: f32,
    cur_version: i16,
    swaying: bool,
}

impl SwayClientUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            module_data,
            owner_id,
            cur_value: 0.0,
            cur_angle: 0.0,
            cur_delta: 0.0,
            cur_angle_limit: 0.0,
            lean_angle: 0.0,
            cur_version: -1,
            swaying: true,
        }
    }

    pub fn stop_sway(&mut self) {
        self.swaying = false;
    }

    pub fn client_update(&mut self) {
        if !self.swaying {
            return;
        }

        let Some(info) = current_breeze_info() else {
            return;
        };

        let Some(object) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };
        let Some(drawable) = obj_guard.get_drawable() else {
            return;
        };

        if info.breeze_version != self.cur_version {
            self.update_sway(&info);
        } else if drawable
            .read()
            .ok()
            .map(|guard| !guard.is_visible())
            .unwrap_or(true)
        {
            return;
        }

        self.cur_value += self.cur_delta;
        if self.cur_value > 2.0 * PI {
            self.cur_value -= 2.0 * PI;
        }

        let cosine = self.cur_value.cos();
        let target_angle = cosine * self.cur_angle_limit + self.lean_angle;
        let delta_angle = target_angle - self.cur_angle;

        let mut xfrm = drawable.get_instance_matrix();
        xfrm = Mat4::from_rotation_x(-delta_angle * info.direction_vec[0]) * xfrm;
        xfrm = Mat4::from_rotation_y(delta_angle * info.direction_vec[1]) * xfrm;
        drawable.set_instance_matrix(Some(&xfrm));

        self.cur_angle = target_angle;

        if obj_guard.test_status(ObjectStatusTypes::Burned) {
            self.stop_sway();
        }
    }

    fn update_sway(&mut self, info: &BreezeInfo) {
        if info.randomness == 0.0 {
            self.cur_value = 0.0;
            return;
        }

        let delta = info.randomness * 0.5;
        self.cur_angle_limit =
            info.intensity * game_client_random_value_real(1.0 - delta, 1.0 + delta);

        let period = info.breeze_period.max(1) as f32;
        self.cur_delta =
            (2.0 * PI / period) * game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.lean_angle = info.lean * game_client_random_value_real(1.0 - delta, 1.0 + delta);
        self.cur_version = info.breeze_version;
    }
}

impl Module for SwayClientUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

impl Snapshotable for SwayClientUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        xfer.xfer_real(&mut self.cur_value)
            .map_err(|e| format!("{:?}", e))?;
        xfer.xfer_real(&mut self.cur_angle)
            .map_err(|e| format!("{:?}", e))?;
        xfer.xfer_real(&mut self.cur_delta)
            .map_err(|e| format!("{:?}", e))?;
        xfer.xfer_real(&mut self.cur_angle_limit)
            .map_err(|e| format!("{:?}", e))?;
        xfer.xfer_real(&mut self.lean_angle)
            .map_err(|e| format!("{:?}", e))?;
        xfer.xfer_short(&mut self.cur_version)
            .map_err(|e| format!("{:?}", e))?;
        xfer.xfer_bool(&mut self.swaying)
            .map_err(|e| format!("{:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(info) = current_breeze_info() {
            self.update_sway(&info);
        }
        Ok(())
    }
}

fn current_breeze_info() -> Option<BreezeInfo> {
    let engine = get_script_engine();
    let guard = engine.read().ok()?;
    guard
        .as_ref()
        .map(|engine| engine.get_breeze_info().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use game_engine::common::thing::module::BaseModuleData;
    use std::io::Cursor;

    #[test]
    fn test_sway_client_update_xfer_preserves_cpp_runtime_fields() {
        let module_data = Arc::new(BaseModuleData::new());
        let mut saved = SwayClientUpdateModule::new(11, module_data.clone(), 22);
        saved.cur_value = 1.25;
        saved.cur_angle = -0.5;
        saved.cur_delta = 0.125;
        saved.cur_angle_limit = 0.75;
        saved.lean_angle = -0.25;
        saved.cur_version = 7;
        saved.swaying = false;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("sway_client_update").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = SwayClientUpdateModule::new(11, module_data, 22);
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("sway_client_update").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        assert_eq!(loaded.cur_value, 1.25);
        assert_eq!(loaded.cur_angle, -0.5);
        assert_eq!(loaded.cur_delta, 0.125);
        assert_eq!(loaded.cur_angle_limit, 0.75);
        assert_eq!(loaded.lean_angle, -0.25);
        assert_eq!(loaded.cur_version, 7);
        assert!(!loaded.swaying);
    }
}

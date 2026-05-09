// AnimatedParticleSysBoneClientUpdate - update particle systems attached to bones.
// Ported from C++ AnimatedParticleSysBoneClientUpdate.cpp/.h.

use crate::common::ObjectID;
use crate::helpers::TheGameLogic;
use crate::object::draw::w3d_model_draw::W3DModelDraw;
use crate::object::drawable::DrawableArcExt;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::any::Any;
use std::sync::Arc;

pub struct AnimatedParticleSysBoneClientUpdateModule {
    module_name_key: NameKeyType,
    module_data: Arc<dyn ModuleData>,
    owner_id: ObjectID,
    life: u32,
}

impl AnimatedParticleSysBoneClientUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            module_data,
            owner_id,
            life: 0,
        }
    }

    pub fn client_update(&mut self) {
        self.life = self.life.wrapping_add(1);

        let Some(object) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };
        let Some(drawable) = obj_guard.get_drawable() else {
            return;
        };

        for module in drawable.get_draw_modules() {
            let updated = module
                .with_module_downcast::<W3DModelDraw, _, _>(|model_draw| {
                    model_draw.update_bones_for_client_particle_systems()
                })
                .unwrap_or(false);
            if updated {
                break;
            }
        }
    }
}

impl Module for AnimatedParticleSysBoneClientUpdateModule {
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

impl Snapshotable for AnimatedParticleSysBoneClientUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
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
    use game_engine::common::thing::module::BaseModuleData;
    use std::io::Cursor;

    #[test]
    fn animated_particle_sys_bone_xfer_writes_cpp_version_only_block() {
        let module_data = Arc::new(BaseModuleData::new());
        let mut saved = AnimatedParticleSysBoneClientUpdateModule::new(11, module_data.clone(), 22);
        saved.life = 1234;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("animated_particle_sys_bone").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = AnimatedParticleSysBoneClientUpdateModule::new(11, module_data, 22);
        loaded.life = 77;
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("animated_particle_sys_bone").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        assert_eq!(loaded.life, 77);
    }
}

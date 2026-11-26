//! WorkerAIUpdate module data + compatibility shim.
//!
//! Ported from GameLogic/Module/WorkerAIUpdate.h.

pub use crate::ai::modules::WorkerAIUpdate;

pub use game_engine::common::thing::builder_ai_data::WorkerAIUpdateModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleDataHandle, ModuleDataRead, NameKeyType};

/// Module wrapper for WorkerAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct WorkerAIUpdateModule {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
}

impl WorkerAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: ModuleDataHandle) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for WorkerAIUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }
}

impl Snapshotable for WorkerAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.read().crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.write().xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::thing::module::ModuleData;

    #[test]
    fn worker_module_reads_shared_template_data() {
        let data = ModuleDataHandle::new(WorkerAIUpdateModuleData::default());
        let module = WorkerAIUpdateModule::new(7, data.clone());
        data.write().set_module_tag_name_key(42);
        assert_eq!(module.get_module_tag_name_key(), 42);
        assert_eq!(module.get_module_data().get_module_tag_name_key(), 42);
    }
}

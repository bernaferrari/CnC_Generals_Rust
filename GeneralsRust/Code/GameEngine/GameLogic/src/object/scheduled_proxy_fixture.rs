//! Test-only admission through the actual Object and ModuleUpdateProxy boundary.

impl super::Object {
    pub(crate) fn installed_update_proxy_for_test(
        object_id: crate::common::ObjectID,
        name: &str,
        module: Box<dyn game_engine::common::thing::module::Module>,
        data: std::sync::Arc<dyn game_engine::common::thing::module::ModuleData>,
    ) -> (
        Self,
        game_engine::common::thing::update_module::UpdateModulePtr,
    ) {
        let mut object = Self::new_test(object_id, 100.0);
        object.install_update_module(name, module, data);
        let index = *object
            .update_module_handles
            .last()
            .expect("installed update");
        let entry = std::sync::Arc::clone(&object.modules[index]);
        let proxy: game_engine::common::thing::update_module::UpdateModulePtr = std::sync::Arc::new(
            std::sync::RwLock::new(super::ModuleUpdateProxy::new(entry, object_id)),
        );
        (object, proxy)
    }
}

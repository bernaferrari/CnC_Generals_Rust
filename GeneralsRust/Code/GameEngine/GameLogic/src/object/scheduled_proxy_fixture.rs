//! Test-only admission through the actual Object and ModuleUpdateProxy boundary.

impl super::Object {
    pub(crate) fn installed_update_proxy_for_test(
        object_id: crate::common::ObjectID,
        name: &str,
        module: Box<dyn game_engine::common::thing::module::Module>,
        data: game_engine::common::thing::module::ModuleDataHandle,
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

    /// Build a proxy for every installed update entry (install order) and
    /// attach the matching installed registration through the actual
    /// `ModuleUpdateProxy` boundary — the same indices `loadPostProcess`
    /// re-registers.
    pub(crate) fn attach_installed_update_proxies_for_test(
        &mut self,
    ) -> Vec<game_engine::common::thing::update_module::UpdateModulePtr> {
        let object_id = self.get_id();
        let handles = self.update_module_handles.clone();
        handles
            .iter()
            .map(|&index| {
                let entry = std::sync::Arc::clone(&self.modules[index]);
                let proxy: game_engine::common::thing::update_module::UpdateModulePtr =
                    std::sync::Arc::new(std::sync::RwLock::new(
                        super::ModuleUpdateProxy::new(std::sync::Arc::clone(&entry), object_id),
                    ));
                self.attach_update_module_registration(
                    std::sync::Arc::clone(&proxy),
                    Some(&entry),
                )
                .expect("installed registration in install order");
                proxy
            })
            .collect()
    }
}

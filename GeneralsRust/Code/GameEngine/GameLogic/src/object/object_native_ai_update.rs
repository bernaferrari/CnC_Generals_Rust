//! Exact installed AI module identity at the receiving scheduler boundary.
use super::*;

impl Object {
    pub(crate) fn scheduled_native_ai(
        &self,
        proxy: &UpdateModulePtr,
    ) -> Result<Option<Arc<Mutex<dyn crate::modules::AIUpdateInterface>>>, String> {
        let registration = self
            .update_module_registrations
            .iter()
            .find(|registration| Arc::ptr_eq(&registration.module, proxy))
            .ok_or("scheduled AI proxy belongs to another object")?;
        let index = registration
            .module_index
            .ok_or("native constructor-helper AI tick is not admitted")?;
        let entry = self
            .modules
            .get(index)
            .ok_or("scheduled AI module index is invalid")?;
        let runtime = entry
            .try_with_module(|module| {
                module
                    .as_any()
                    .downcast_ref::<crate::object::update::AIUpdateInterfaceModule>()
                    .map(|module| module.runtime_ai_owned())
            })
            .ok_or("scheduled AI module already borrowed")?;
        let Some(runtime) = runtime else {
            return Ok(None);
        };
        let runtime = runtime.ok_or("scheduled AI module runtime is absent")?;
        if !self
            .ai
            .as_ref()
            .is_some_and(|actual| Arc::ptr_eq(actual, &runtime))
        {
            return Err("scheduled AI module differs from canonical cached runtime".into());
        }
        Ok(Some(runtime))
    }
}

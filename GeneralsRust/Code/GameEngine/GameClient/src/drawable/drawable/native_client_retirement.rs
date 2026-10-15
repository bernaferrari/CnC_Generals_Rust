//! Installed CU retirement must not flush model conditions or discover services.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;
use gamelogic::object::update::laser_update::LaserUpdateModule;

impl BasicDrawable {
    pub(crate) fn native_client_particle_retirement_required(&mut self) -> Result<bool, String> {
        let mut required = false;
        for slot in &mut self.draw_modules {
            if let Some(module) = slot
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<LaserUpdateModule>())
            {
                if !module.uses_owned_services() {
                    return Err("native retirement cannot drop a classic LaserUpdate under receiving services".into());
                }
                required |= module.requires_owned_particle_retirement();
            }
        }
        Ok(required)
    }

    /// Invoked only after preflight and identity unlink, following Draw teardown.
    pub(crate) fn retire_native_client_particles(
        &mut self,
        manager: &mut ParticleSystemManager,
    ) -> Result<(), String> {
        for slot in &mut self.draw_modules {
            if let Some(module) = slot
                .logic_module_mut()
                .and_then(|module| module.as_any_mut().downcast_mut::<LaserUpdateModule>())
            {
                module.update_mut().retire_particles_with(|id| {
                    manager.destroy_particle_system(id);
                    Ok(())
                })?;
            }
        }
        Ok(())
    }
}

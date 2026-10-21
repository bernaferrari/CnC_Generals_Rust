//! Installed CU retirement must not flush model conditions or discover services.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;

impl BasicDrawable {
    pub(crate) fn native_client_particle_retirement_required(&mut self) -> Result<bool, String> {
        let mut required = false;
        for module in self.native_laser_updates() {
            if !module.uses_owned_services() {
                return Err(
                    "native retirement cannot drop a classic LaserUpdate under receiving services"
                        .into(),
                );
            }
            required |= module.requires_owned_particle_retirement();
        }
        Ok(required)
    }

    /// Invoked only after preflight and identity unlink, following Draw teardown.
    pub(crate) fn retire_native_client_particles(
        &mut self,
        manager: &mut ParticleSystemManager,
    ) -> Result<(), String> {
        for module in self.native_laser_updates() {
            module.update_mut().retire_particles_with(|id| {
                manager.destroy_particle_system(id);
                Ok(())
            })?;
        }
        Ok(())
    }
}

impl BasicDrawable {
    /// Explicit destruction tail; never called by ordinary particle retirement.
    /// C++ Drawable.cpp525–542 deletes both module arrays before ambient stop.
    pub(crate) fn finish_native_retirement(&mut self) {
        while let Some(bucket) = self
            .draw_modules
            .iter()
            .map(|module| module.drawable_module_type_index())
            .min()
        {
            while let Some(index) = self
                .draw_modules
                .iter()
                .position(|module| module.drawable_module_type_index() == bucket)
            {
                drop(self.draw_modules.remove(index));
            }
        }
        self.stop_ambient_sound();
        self.custom_sound_ambient_dynamic_info = None;
    }
}

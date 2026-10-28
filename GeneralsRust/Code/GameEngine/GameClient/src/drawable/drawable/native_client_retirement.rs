//! Installed CU retirement must not flush model conditions or discover services.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;

impl BasicDrawable {
    pub(crate) fn preflight_native_audio_device(
        audio_device: Option<
            &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
    ) -> Result<(), String> {
        if let Some(device) = audio_device {
            drop(
                device.lock().map_err(
                    |_| "receiving audio device lock poisoned during Drawable retirement",
                )?,
            );
        }
        Ok(())
    }

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
        self.finish_native_retirement_with_device(None)
            .expect("legacy Drawable retirement has no fallible audio loan");
    }

    /// Explicit native application route. Draw modules are still destroyed
    /// before their ambient handle is stopped; the stop is appended to the
    /// same receiving AudioManager request queue used by playback.
    pub(crate) fn finish_native_retirement_with_device(
        &mut self,
        audio_device: Option<
            &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
    ) -> Result<(), String> {
        // Reject an already-poisoned owner before destroying any module state.
        Self::preflight_native_audio_device(audio_device)?;
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
        if let Some(device) = audio_device {
            let mut audio = device
                .lock()
                .map_err(|_| "receiving audio device lock poisoned during Drawable retirement")?;
            self.stop_ambient_sound_with_audio(&mut audio);
        } else {
            // Standalone compatibility path for clients that were never bound
            // to a native application's audio service.
            self.stop_ambient_sound();
        }
        self.custom_sound_ambient_dynamic_info = None;
        self.custom_sound_ambient_base_resolved = false;
        Ok(())
    }
}

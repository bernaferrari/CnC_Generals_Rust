//! Installed CU retirement must not flush model conditions or discover services.
use super::*;
use crate::effects::particle_manager::ParticleSystemManager;

impl BasicDrawable {
    pub(crate) fn preflight_native_retirement_with_device(
        &self,
        audio_device: Option<
            &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
    ) -> Result<(), String> {
        if !self.native_ambient_stop_required() {
            return Ok(());
        }
        let Some(audio_device) = audio_device else {
            return Err("receiving Drawable retirement requires its bound audio service".into());
        };
        drop(
            audio_device
                .lock()
                .map_err(|_| "receiving audio device lock poisoned during Drawable retirement")?,
        );
        Ok(())
    }

    fn native_ambient_stop_required(&self) -> bool {
        self.ambient_sound_event
            .as_ref()
            .is_some_and(|event| event.get_playing_handle() != 0)
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
        self.finish_native_retirement_inner(None, self.receiving_template.is_some())
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
        self.finish_native_retirement_inner(audio_device, true)
    }

    fn finish_native_retirement_inner(
        &mut self,
        audio_device: Option<
            &std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
        require_receiving_audio: bool,
    ) -> Result<(), String> {
        // Admitted native drawables can own a live ambient handle. Letting an
        // application teardown fall through to TheAudio can stop a peer
        // session's same-ID handle, so reject before module destruction and
        // leave the Drawable intact until its receiving service is rebound.
        // Reject a missing or already-poisoned owner before destroying any
        // module state. The explicit compatibility entrypoint above is the
        // only route allowed to use process-global audio.
        if require_receiving_audio || audio_device.is_some() {
            self.preflight_native_retirement_with_device(audio_device)?;
        }
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
        if require_receiving_audio || audio_device.is_some() {
            if self.native_ambient_stop_required() {
                let device = audio_device.ok_or_else(|| {
                    "receiving Drawable retirement requires its bound audio service".to_owned()
                })?;
                let mut audio = device.lock().map_err(
                    |_| "receiving audio device lock poisoned during Drawable retirement",
                )?;
                self.stop_ambient_sound_with_audio(&mut audio);
            } else {
                // An inert event has no mixer request to stop.
                self.ambient_sound_event.take();
            }
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

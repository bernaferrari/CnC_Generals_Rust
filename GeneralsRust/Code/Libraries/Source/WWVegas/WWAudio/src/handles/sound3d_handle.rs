use std::sync::{Arc, Mutex};

use crate::{
    error::{Error, Result},
    sound3d::Sound3D,
};

use super::{base_handle::BaseSoundHandle, sound2d_handle::Sound2DHandle};

/// 3D sound handle implementation (`Sound3DHandleClass` analogue)
pub struct Sound3DHandle {
    pub base: BaseSoundHandle,
    sound: Arc<Mutex<Sound3D>>,
    sample_handle: Sound2DHandle,
}

impl Sound3DHandle {
    pub fn new(sound: Sound3D, sample_handle: Sound2DHandle) -> Self {
        Self {
            base: BaseSoundHandle::new(),
            sound: Arc::new(Mutex::new(sound)),
            sample_handle,
        }
    }

    pub fn with_shared_sound(sound: Arc<Mutex<Sound3D>>, sample_handle: Sound2DHandle) -> Self {
        Self {
            base: BaseSoundHandle::new(),
            sound,
            sample_handle,
        }
    }

    pub fn sound(&self) -> Arc<Mutex<Sound3D>> {
        Arc::clone(&self.sound)
    }

    pub fn sample_handle(&self) -> Sound2DHandle {
        self.sample_handle.clone()
    }

    pub fn start_sample(&self) -> Result<()> {
        {
            let mut sound = self
                .sound
                .lock()
                .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
            sound.play(true);
        }
        self.sample_handle.start_sample()
    }

    pub fn stop_sample(&self) -> Result<()> {
        {
            let mut sound = self
                .sound
                .lock()
                .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
            sound.remove_from_scene();
        }
        self.sample_handle.stop_sample()
    }

    pub fn resume_sample(&self) -> Result<()> {
        {
            let mut sound = self
                .sound
                .lock()
                .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
            sound.add_to_scene();
        }
        self.sample_handle.resume_sample()
    }

    pub fn end_sample(&self) -> Result<()> {
        self.sample_handle.end_sample()
    }

    pub fn set_sample_volume(&self, volume: f32) -> Result<()> {
        let volume = volume.clamp(0.0, 1.0);
        {
            let mut sound = self
                .sound
                .lock()
                .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
            sound.base.set_volume(volume);
        }
        let volume_level_u8 = (volume * 100.0).round().clamp(0.0, 100.0) as u8;
        let miles_volume = (i32::from(volume_level_u8) * 127 / 100).clamp(0, 127);
        self.sample_handle.set_sample_volume(miles_volume)
    }

    pub fn get_sample_volume(&self) -> Result<f32> {
        let sound = self
            .sound
            .lock()
            .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
        Ok(sound.base.volume())
    }

    pub fn set_max_vol_radius(&self, radius: f32) -> Result<()> {
        let mut sound = self
            .sound
            .lock()
            .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
        sound.set_max_vol_radius(radius);
        Ok(())
    }

    pub fn set_dropoff_radius(&self, radius: f32) -> Result<()> {
        let mut sound = self
            .sound
            .lock()
            .map_err(|_| Error::Audio("Sound3D lock poisoned".to_string()))?;
        sound.set_dropoff_radius(radius);
        Ok(())
    }
}

impl Clone for Sound3DHandle {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            sound: Arc::clone(&self.sound),
            sample_handle: self.sample_handle.clone(),
        }
    }
}

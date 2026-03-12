use std::sync::{Arc, Mutex};

use crate::{channel::AudioChannel, error::Result};

use super::{base_handle::BaseSoundHandle, sound2d_handle::Sound2DHandle};

/// Stream handle analogue (`SoundStreamHandleClass`)
pub struct SoundStreamHandle {
    pub base: BaseSoundHandle,
    channel: Arc<Mutex<AudioChannel>>,
    sample_handle: Sound2DHandle,
}

impl SoundStreamHandle {
    pub fn new(channel: AudioChannel) -> Self {
        let arc = Arc::new(Mutex::new(channel));
        let mixer = {
            let guard = arc.lock().expect("Channel lock poisoned");
            guard.mixer()
        };
        Self {
            base: BaseSoundHandle::new(),
            channel: Arc::clone(&arc),
            sample_handle: Sound2DHandle::with_shared_channel_and_mixer(arc, mixer),
        }
    }

    pub fn initialize(&mut self, buffer: Arc<crate::AudioSource>) {
        self.base.initialize(buffer.clone());
        self.sample_handle.initialize(buffer);
    }

    pub fn sample_handle(&self) -> Sound2DHandle {
        self.sample_handle.clone()
    }

    pub fn start_sample(&self) -> Result<()> {
        self.sample_handle.start_sample()
    }

    pub fn stop_sample(&self) -> Result<()> {
        self.sample_handle.stop_sample()
    }

    pub fn resume_sample(&self) -> Result<()> {
        self.sample_handle.resume_sample()
    }

    pub fn end_sample(&self) -> Result<()> {
        self.sample_handle.end_sample()
    }

    pub fn set_sample_pan(&self, pan: i32) -> Result<()> {
        self.sample_handle.set_sample_pan(pan)
    }

    pub fn get_sample_pan(&self) -> Result<i32> {
        self.sample_handle.get_sample_pan()
    }

    pub fn set_sample_volume(&self, volume: i32) -> Result<()> {
        self.sample_handle.set_sample_volume(volume)
    }

    pub fn get_sample_volume(&self) -> Result<i32> {
        self.sample_handle.get_sample_volume()
    }

    pub fn set_sample_loop_count(&self, count: u32) -> Result<()> {
        self.sample_handle.set_sample_loop_count(count)
    }

    pub fn get_sample_loop_count(&self) -> Result<u32> {
        self.sample_handle.get_sample_loop_count()
    }

    pub fn set_sample_ms_position(&self, ms: u32) -> Result<()> {
        self.sample_handle.set_sample_ms_position(ms)
    }

    pub fn get_sample_ms_position(&self) -> Result<(u32, u32)> {
        self.sample_handle.get_sample_ms_position()
    }
}

impl Clone for SoundStreamHandle {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            channel: Arc::clone(&self.channel),
            sample_handle: self.sample_handle.clone(),
        }
    }
}

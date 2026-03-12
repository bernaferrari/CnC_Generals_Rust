use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use crate::{
    channel::AudioChannel,
    error::{Error, Result},
    mixer::{AudioMixer, VoiceHandle, VoiceParams, VoiceSpatialParams},
};

use super::base_handle::{miles_to_volume, volume_to_miles, BaseSoundHandle};

/// 2D sound handle implementation mirroring `Sound2DHandleClass`
pub struct Sound2DHandle {
    pub base: BaseSoundHandle,
    channel: Arc<Mutex<AudioChannel>>,
    mixer: Arc<AudioMixer>,
}

impl Sound2DHandle {
    pub fn new(channel: AudioChannel) -> Self {
        let mixer = channel.mixer();
        Self {
            base: BaseSoundHandle::new(),
            channel: Arc::new(Mutex::new(channel)),
            mixer,
        }
    }

    pub fn with_shared_channel(channel: Arc<Mutex<AudioChannel>>) -> Self {
        let mixer = {
            let guard = channel.lock().expect("Channel lock poisoned");
            guard.mixer()
        };
        Self::with_shared_channel_and_mixer(channel, mixer)
    }

    pub fn with_shared_channel_and_mixer(
        channel: Arc<Mutex<AudioChannel>>,
        mixer: Arc<AudioMixer>,
    ) -> Self {
        Self {
            base: BaseSoundHandle::new(),
            channel,
            mixer,
        }
    }

    pub fn channel(&self) -> Arc<Mutex<AudioChannel>> {
        Arc::clone(&self.channel)
    }

    pub fn initialize(&mut self, buffer: Arc<crate::AudioSource>) {
        self.base.initialize(buffer);
    }

    pub fn start_sample(&self) -> Result<()> {
        let buffer = self
            .base
            .buffer()
            .ok_or_else(|| Error::Audio("Sound handle has no buffer".to_string()))?;

        let mut state: Option<(Option<VoiceHandle>, VoiceParams)> = None;
        let result = {
            let mut channel = self.lock_channel()?;
            channel.set_handle_id(self.base.miles_handle());
            let should_loop = channel.loop_count() == 0;
            let result = channel.play_source((*buffer).clone(), should_loop);
            if result.is_ok() {
                state = Some(Self::voice_state(&channel));
            }
            result
        };

        if let Some((voice_id, params)) = state {
            self.update_mixer_params(voice_id, params);
        }

        result
    }

    pub fn pause_sample(&self) -> Result<()> {
        let mut channel = self.lock_channel()?;
        channel.pause()
    }

    pub fn stop_sample(&self) -> Result<()> {
        let mut channel = self.lock_channel()?;
        channel.stop()
    }

    pub fn resume_sample(&self) -> Result<()> {
        let mut channel = self.lock_channel()?;
        channel.resume()
    }

    pub fn end_sample(&self) -> Result<()> {
        let mut channel = self.lock_channel()?;
        channel.stop_immediately();
        Ok(())
    }

    pub fn set_sample_pan(&self, pan: i32) -> Result<()> {
        let (voice_id, params) = {
            let mut channel = self.lock_channel()?;
            channel.set_pan(pan);
            Self::voice_state(&channel)
        };

        self.update_mixer_params(voice_id, params);
        Ok(())
    }

    pub fn get_sample_pan(&self) -> Result<i32> {
        let channel = self.lock_channel()?;
        Ok(channel.pan())
    }

    pub fn set_sample_volume(&self, volume: i32) -> Result<()> {
        let (voice_id, params, result) = {
            let mut channel = self.lock_channel()?;
            let result = channel.set_volume(miles_to_volume(volume));
            let state = Self::voice_state(&channel);
            (state.0, state.1, result)
        };

        if result.is_ok() {
            self.update_mixer_params(voice_id, params);
        }

        result
    }

    pub fn get_sample_volume(&self) -> Result<i32> {
        let channel = self.lock_channel()?;
        Ok(volume_to_miles(channel.volume()))
    }

    pub fn set_sample_loop_count(&self, count: u32) -> Result<()> {
        let (voice_id, params) = {
            let mut channel = self.lock_channel()?;
            channel.set_loop_count(count);
            Self::voice_state(&channel)
        };

        self.update_mixer_params(voice_id, params);
        Ok(())
    }

    pub fn get_sample_loop_count(&self) -> Result<u32> {
        let channel = self.lock_channel()?;
        Ok(channel.loop_count())
    }

    pub fn set_sample_ms_position(&self, ms: u32) -> Result<()> {
        let (voice_id, params, result) = {
            let mut channel = self.lock_channel()?;
            let result = channel.set_position(Duration::from_millis(ms as u64));
            let state = Self::voice_state(&channel);
            (state.0, state.1, result)
        };

        if result.is_ok() {
            self.update_mixer_params(voice_id, params);
        }

        result
    }

    pub fn get_sample_ms_position(&self) -> Result<(u32, u32)> {
        let channel = self.lock_channel()?;
        let length = channel.length_ms().unwrap_or(0);
        let position = channel.get_position().as_millis().min(u128::from(u32::MAX)) as u32;
        Ok((length, position))
    }

    pub fn set_sample_user_data(&self, index: usize, value: u32) -> Result<()> {
        if index >= 4 {
            return Err(Error::Audio("User data index out of range".to_string()));
        }
        let mut channel = self.lock_channel()?;
        channel.set_user_data(index, value)
    }

    pub fn get_sample_user_data(&self, index: usize) -> Result<u32> {
        if index >= 4 {
            return Err(Error::Audio("User data index out of range".to_string()));
        }
        let channel = self.lock_channel()?;
        channel
            .user_data(index)
            .ok_or_else(|| Error::Audio("User data missing".to_string()))
    }

    pub fn set_sample_playback_rate(&self, rate: u32) -> Result<()> {
        let (voice_id, params) = {
            let mut channel = self.lock_channel()?;
            channel.set_playback_rate(rate);
            Self::voice_state(&channel)
        };

        self.update_mixer_params(voice_id, params);
        Ok(())
    }

    pub fn get_sample_playback_rate(&self) -> Result<u32> {
        let channel = self.lock_channel()?;
        Ok(channel.playback_rate())
    }

    pub fn update_spatial_params(&self, spatial: VoiceSpatialParams) {
        if let Ok(mut channel) = self.lock_channel() {
            channel.set_spatial_params(spatial);
        }
    }

    fn lock_channel(&self) -> Result<MutexGuard<'_, AudioChannel>> {
        self.channel
            .lock()
            .map_err(|_| Error::Audio("Channel lock poisoned".to_string()))
    }

    fn voice_state(channel: &AudioChannel) -> (Option<VoiceHandle>, VoiceParams) {
        (channel.mixer_voice_handle(), channel.voice_params())
    }

    fn update_mixer_params(&self, voice_handle: Option<VoiceHandle>, params: VoiceParams) {
        if let Some(handle) = voice_handle {
            self.mixer.update_voice_params(handle, params);
        }
    }
}

impl Clone for Sound2DHandle {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            channel: Arc::clone(&self.channel),
            mixer: Arc::clone(&self.mixer),
        }
    }
}

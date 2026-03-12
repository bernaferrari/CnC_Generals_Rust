//! Audio channel management and playback control.
//!
//! Based on the original C&C Generals audio channel system.

use crate::{
    error::{Error, Result},
    level::VolumeUtils,
    mixer::{
        AudioMixer, VoiceDescriptor, VoiceHandle, VoiceParams, VoiceSpatialParams, VoiceStopReason,
    },
    Priority,
};
use log::{debug, info};
use parking_lot::RwLock;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// Channel type identifier for different audio uses
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    /// User-initiated audio playback
    User,
    /// System audio (UI sounds, notifications)
    System,
    /// Music playback
    Music,
    /// Voice/speech audio
    Voice,
    /// Ambient sounds
    Ambient,
}

/// Audio channel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    Stopped,
    Playing,
    Paused,
    Stopping,
}

/// Channel configuration
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub priority: Priority,
    pub volume: crate::Volume,
    pub looping: bool,
    pub channel_type: ChannelType,
    pub fade_in_duration: Option<std::time::Duration>,
    pub fade_out_duration: Option<std::time::Duration>,
}

/// Individual audio channel for playback - matches C++ audio channel interface
pub struct AudioChannel {
    pub id: u32,
    pub priority: Priority,
    pub is_playing: bool,
    state: Arc<RwLock<ChannelState>>,
    config: ChannelConfig,
    current_source: Option<crate::AudioSource>,
    start_time: Option<Instant>,
    position: std::time::Duration,
    is_looping: bool,
    volume: f32,
    target_volume: f32,
    fade_start_time: Option<Instant>,
    fade_initial_volume: f32,
    active_fade_duration: Option<std::time::Duration>,
    frame_duration: std::time::Duration,
    pan: i32,
    loop_count: u32,
    playback_rate: u32,
    user_data: [u32; 4],
    mixer: Arc<AudioMixer>,
    mixer_voice_handle: Option<VoiceHandle>,
    handle_id: Option<u32>,
    channels: u16,
    sample_rate: u32,
    spatial_params: VoiceSpatialParams,
}

impl AudioChannel {
    /// Create a new audio channel
    pub fn new(id: u32, priority: Priority, mixer: Arc<AudioMixer>) -> Self {
        let config = ChannelConfig {
            priority,
            ..ChannelConfig::default()
        };
        let initial_volume = VolumeUtils::volume_to_linear(config.volume);
        Self {
            id,
            priority,
            is_playing: false,
            state: Arc::new(RwLock::new(ChannelState::Stopped)),
            config,
            current_source: None,
            start_time: None,
            position: Duration::ZERO,
            is_looping: false,
            volume: initial_volume,
            target_volume: initial_volume,
            fade_start_time: None,
            fade_initial_volume: initial_volume,
            active_fade_duration: None,
            frame_duration: Duration::from_millis(16),
            pan: 0,
            loop_count: 1,
            playback_rate: 44_100,
            user_data: [0; 4],
            mixer,
            mixer_voice_handle: None,
            handle_id: None,
            channels: 2,
            sample_rate: 44_100,
            spatial_params: VoiceSpatialParams::default(),
        }
    }

    /// Play an audio source on this channel - matches C++ play functionality
    pub fn play_source(&mut self, source: crate::AudioSource, looping: bool) -> Result<()> {
        info!(
            "Playing audio source '{}' on channel {}",
            source.identifier(),
            self.id
        );

        self.stop_mixer_voice(VoiceStopReason::Command);

        let sample_arc = source.sample().ok_or_else(|| {
            Error::Audio("Audio source is missing decoded sample data".to_string())
        })?;
        let sample_ref = sample_arc.as_ref();

        let data = sample_ref
            .data
            .as_ref()
            .ok_or_else(|| Error::Audio("Audio sample has no PCM payload".to_string()))?;
        if data.len() < 2 {
            return Err(Error::Audio("Audio sample payload too small".to_string()));
        }

        let format = sample_ref
            .format
            .as_ref()
            .ok_or_else(|| Error::Audio("Audio sample missing format metadata".to_string()))?;

        let channels = format.channels.max(1);
        let sample_rate = format.rate.max(1);

        self.channels = channels;
        self.sample_rate = sample_rate;
        let mixer_source = Arc::new(source.clone());
        self.current_source = Some(source);
        self.is_looping = looping;
        if looping {
            self.loop_count = 0;
        } else if self.loop_count == 0 {
            self.loop_count = 1;
        }

        self.is_playing = true;
        self.position = Duration::ZERO;

        {
            let mut state = self.state.write();
            *state = ChannelState::Playing;
        }

        if let Some(fade_duration) = self.config.fade_in_duration {
            self.start_fade_in(fade_duration);
        }

        self.start_time = Some(Instant::now());
        debug!(
            "Audio channel {} started playing (looping: {})",
            self.id, looping
        );

        let mut params = self.voice_params();
        params.start_frame = 0;
        let descriptor = VoiceDescriptor {
            source: mixer_source,
            params,
            channel_id: self.id,
            handle_id: self.handle_id,
        };
        let voice_handle = self.mixer.start_voice(descriptor);
        self.mixer.resume_voice(voice_handle);
        self.mixer_voice_handle = Some(voice_handle);
        self.sync_mixer_voice_params();

        Ok(())
    }

    /// Pause playback - matches C++ pause functionality
    pub fn pause(&mut self) -> Result<()> {
        if !self.is_playing {
            return Ok(());
        }

        {
            let mut state = self.state.write();
            *state = ChannelState::Paused;
        }

        // Update position before pausing
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed();
            self.position += elapsed;
        }

        self.is_playing = false;
        self.start_time = None;

        if let Some(handle) = self.mixer_voice_handle {
            self.mixer.pause_voice(handle);
        }

        info!(
            "Audio channel {} paused at position {:?}",
            self.id, self.position
        );
        Ok(())
    }

    /// Resume playback - matches C++ resume functionality  
    pub fn resume(&mut self) -> Result<()> {
        if self.is_playing {
            return Ok(());
        }

        {
            let mut state = self.state.write();
            *state = ChannelState::Playing;
        }

        self.is_playing = true;
        self.start_time = Some(Instant::now());

        if let Some(handle) = self.mixer_voice_handle {
            self.mixer.resume_voice(handle);
            self.sync_mixer_voice_params();
        }

        info!(
            "Audio channel {} resumed from position {:?}",
            self.id, self.position
        );
        Ok(())
    }

    /// Stop playbook - matches C++ stop functionality
    pub fn stop(&mut self) -> Result<()> {
        info!("Stopping audio channel {}", self.id);

        // Apply fade-out if configured
        if let Some(fade_duration) = self.config.fade_out_duration {
            self.start_fade_out(fade_duration);

            // Set state to stopping, not stopped yet
            {
                let mut state = self.state.write();
                *state = ChannelState::Stopping;
            }
        } else {
            // Immediate stop
            self.stop_immediately_with_reason(VoiceStopReason::Command);
        }

        Ok(())
    }

    /// Stop immediately without fading
    pub fn stop_immediately(&mut self) {
        self.stop_immediately_with_reason(VoiceStopReason::Command);
    }

    pub fn stop_immediately_with_reason(&mut self, reason: VoiceStopReason) {
        self.stop_mixer_voice(reason);

        {
            let mut state = self.state.write();
            *state = ChannelState::Stopped;
        }

        self.is_playing = false;
        self.current_source = None;
        self.start_time = None;
        self.position = Duration::ZERO;
        self.is_looping = false;
        self.volume = 1.0;
        self.target_volume = 1.0;
        self.fade_start_time = None;
        self.fade_initial_volume = 1.0;
        self.active_fade_duration = None;
        self.pan = 0;
        self.loop_count = 1;
        self.playback_rate = 44_100;
        self.user_data = [0; 4];
        self.handle_id = None;
        self.spatial_params = VoiceSpatialParams::default();

        debug!("Audio channel {} stopped immediately", self.id);
    }

    /// Get current channel state
    pub fn state(&self) -> ChannelState {
        *self.state.read()
    }

    /// Wait for playback completion - matches C++ completion waiting
    pub async fn wait_for_completion(&self) -> Result<()> {
        debug!("Waiting for completion on channel {}", self.id);

        // In a real implementation, this would use proper async waiting
        // For now, simulate completion checking
        while self.is_playing {
            // Check if we've reached the end of non-looping audio
            if let Some(ref source) = self.current_source {
                if !self.is_looping {
                    let current_position = self.get_position();
                    if current_position >= source.duration() {
                        break;
                    }
                }
            }

            // Small delay to avoid busy waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        debug!("Audio channel {} playback completed", self.id);
        Ok(())
    }

    /// Set channel volume - matches C++ volume control
    pub fn set_volume(&mut self, volume: crate::Volume) -> Result<()> {
        let linear = VolumeUtils::volume_to_linear(volume);
        self.config.volume = volume;
        self.target_volume = linear;

        // Immediate volume change if not fading
        if self.fade_start_time.is_none() {
            self.volume = linear;
            self.fade_initial_volume = linear;
        }
        self.sync_mixer_voice_params();

        debug!("Set volume on channel {} to {}", self.id, linear);
        Ok(())
    }

    /// Get current playback position
    pub fn get_position(&self) -> std::time::Duration {
        if let Some(handle) = self.mixer_voice_handle {
            if let Some(state) = self.mixer.voice_timeline(handle) {
                if state.source_rate > 0 {
                    let seconds = (state.position_frames / state.source_rate as f64).max(0.0);
                    return Duration::from_secs_f64(seconds);
                }

                if self.sample_rate > 0 {
                    let seconds =
                        (state.rendered_frames as f64 / self.sample_rate.max(1) as f64).max(0.0);
                    return Duration::from_secs_f64(seconds);
                }
            }
        }

        if let Some(start_time) = self.start_time {
            self.position + start_time.elapsed()
        } else {
            self.position
        }
    }

    /// Set playback position (seeking)
    pub fn set_position(&mut self, position: std::time::Duration) -> Result<()> {
        self.position = position;

        let frame_offset = (position.as_secs_f64() * self.sample_rate as f64) as u64;
        if let Some(handle) = self.mixer_voice_handle {
            self.mixer.seek_voice(handle, frame_offset);
        }

        self.start_time = if self.is_playing {
            Some(Instant::now())
        } else {
            None
        };

        self.sync_mixer_voice_params();
        debug!("Seek on channel {} to position {:?}", self.id, position);
        Ok(())
    }

    pub fn set_spatial_params(&mut self, spatial: VoiceSpatialParams) {
        self.spatial_params = spatial;
        self.sync_mixer_voice_params();
    }

    pub fn spatial_params(&self) -> VoiceSpatialParams {
        self.spatial_params
    }

    /// Update channel state - should be called regularly
    pub fn update(&mut self) {
        // Update fading
        if let Some(fade_start) = self.fade_start_time {
            let fade_elapsed = fade_start.elapsed();
            let fade_duration = self
                .active_fade_duration
                .unwrap_or_else(|| Duration::from_millis(500));

            if fade_duration.is_zero() {
                self.volume = self.target_volume;
                self.fade_start_time = None;
                self.active_fade_duration = None;
            } else {
                let progress =
                    (fade_elapsed.as_secs_f32() / fade_duration.as_secs_f32()).clamp(0.0, 1.0);
                self.volume = self.fade_initial_volume
                    + (self.target_volume - self.fade_initial_volume) * progress;

                if progress >= 1.0 {
                    self.volume = self.target_volume;
                    self.fade_start_time = None;
                    self.active_fade_duration = None;

                    if self.state() == ChannelState::Stopping {
                        self.stop_immediately_with_reason(VoiceStopReason::Command);
                    }
                }
            }
        }

        // Check for loop completion
        if self.is_playing && !self.is_looping {
            if let Some(ref source) = self.current_source {
                let current_position = self.get_position();
                if current_position >= source.duration() {
                    // Non-looping audio finished
                    self.stop_immediately_with_reason(VoiceStopReason::Completed);
                }
            }
        }

        self.sync_mixer_voice_params();
    }

    pub(crate) fn voice_params(&self) -> VoiceParams {
        VoiceParams {
            gain: self.volume,
            pan: (self.pan as f32 / 1000.0).clamp(-1.0, 1.0),
            playback_rate: self.playback_rate,
            loop_count: self.loop_count,
            start_frame: 0,
            is_culled: !self.is_playing || !self.is_audible(),
            spatial: self.spatial_params,
        }
    }

    fn sync_mixer_voice_params(&self) {
        if let Some(handle) = self.mixer_voice_handle {
            self.mixer.update_voice_params(handle, self.voice_params());
        }
    }

    fn stop_mixer_voice(&mut self, reason: VoiceStopReason) {
        if let Some(handle) = self.mixer_voice_handle.take() {
            self.mixer.stop_voice(handle, reason);
        }
    }

    pub(crate) fn mixer(&self) -> Arc<AudioMixer> {
        Arc::clone(&self.mixer)
    }

    pub(crate) fn mixer_voice_handle(&self) -> Option<VoiceHandle> {
        self.mixer_voice_handle
    }

    /// Start fade-in effect
    fn start_fade_in(&mut self, duration: std::time::Duration) {
        self.volume = 0.0;
        self.fade_initial_volume = 0.0;
        self.target_volume = VolumeUtils::volume_to_linear(self.config.volume);
        self.fade_start_time = Some(Instant::now());
        self.active_fade_duration = Some(duration);
        self.config.fade_in_duration = Some(duration);

        debug!(
            "Started fade-in on channel {} (duration: {:?})",
            self.id, duration
        );
    }

    /// Start fade-out effect
    fn start_fade_out(&mut self, duration: std::time::Duration) {
        self.fade_initial_volume = self.volume;
        self.target_volume = 0.0;
        self.fade_start_time = Some(Instant::now());
        self.active_fade_duration = Some(duration);
        self.config.fade_out_duration = Some(duration);

        debug!(
            "Started fade-out on channel {} (duration: {:?})",
            self.id, duration
        );
    }

    /// Get the current effective volume (including fading)
    pub fn get_effective_volume(&self) -> f32 {
        self.volume
    }

    /// Check if channel is available for new playback
    pub fn is_available(&self) -> bool {
        matches!(self.state(), ChannelState::Stopped)
    }

    /// Get channel configuration
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }

    /// Get current audio source being played
    pub fn current_source(&self) -> Option<&crate::AudioSource> {
        self.current_source.as_ref()
    }

    /// Begin playback using existing configuration (alias for `resume`)
    pub fn start(&mut self) -> Result<()> {
        self.resume()
    }

    /// Retrieve the configured channel volume
    pub fn volume(&self) -> crate::Volume {
        self.config.volume
    }

    /// Smoothly fade to a target volume over the provided duration
    pub fn fade_to_volume(&mut self, volume: crate::Volume, duration: Duration) -> Result<()> {
        self.config.volume = volume;
        self.fade_initial_volume = self.volume;
        self.target_volume = VolumeUtils::volume_to_linear(volume);
        self.fade_start_time = Some(Instant::now());
        self.active_fade_duration = Some(duration);
        Ok(())
    }

    /// Check whether a fade operation is currently active
    pub fn is_fading(&self) -> bool {
        self.fade_start_time.is_some()
    }

    /// Determine if the channel currently outputs audible content
    pub fn is_audible(&self) -> bool {
        self.volume > f32::EPSILON
    }

    /// Get the nominal frame time used for timing calculations
    pub fn frame_time(&self) -> Duration {
        self.frame_duration
    }

    /// Set the nominal frame time used for updates
    pub fn set_frame_time(&mut self, duration: Duration) {
        self.frame_duration = duration;
    }

    /// Update the playback pan (-1000..1000 range)
    pub fn set_pan(&mut self, pan: i32) {
        self.pan = pan.clamp(-1000, 1000);
        debug!("Set pan on channel {} to {}", self.id, self.pan);
        self.sync_mixer_voice_params();
    }

    /// Retrieve current pan
    pub fn pan(&self) -> i32 {
        self.pan
    }

    /// Configure loop count (0 = infinite)
    pub fn set_loop_count(&mut self, count: u32) {
        self.loop_count = count;
        self.is_looping = count == 0 || count > 1;
        debug!(
            "Set loop count on channel {} to {} (looping: {})",
            self.id, self.loop_count, self.is_looping
        );
        self.sync_mixer_voice_params();
    }

    /// Get the configured loop count
    pub fn loop_count(&self) -> u32 {
        self.loop_count
    }

    /// Override playback rate (Hz)
    pub fn set_playback_rate(&mut self, rate: u32) {
        self.playback_rate = rate.max(1);
        debug!(
            "Set playback rate on channel {} to {}",
            self.id, self.playback_rate
        );
        self.sync_mixer_voice_params();
    }

    /// Get playback rate (Hz)
    pub fn playback_rate(&self) -> u32 {
        self.playback_rate
    }

    pub fn set_handle_id(&mut self, handle_id: Option<u32>) {
        self.handle_id = handle_id;
    }

    pub fn handle_id(&self) -> Option<u32> {
        self.handle_id
    }

    /// Set user data slot
    pub fn set_user_data(&mut self, index: usize, value: u32) -> Result<()> {
        if let Some(slot) = self.user_data.get_mut(index) {
            *slot = value;
            Ok(())
        } else {
            Err(Error::Channel(crate::error::ChannelError::InvalidState(
                format!("Invalid user data index {}", index),
            )))
        }
    }

    /// Get user data slot
    pub fn user_data(&self, index: usize) -> Option<u32> {
        self.user_data.get(index).copied()
    }

    /// Seek to a timestamp
    pub fn seek_to_timestamp(&mut self, timestamp: Duration) {
        let duration = self
            .current_source
            .as_ref()
            .map(|source| source.duration())
            .unwrap_or_default();

        let clamped = timestamp.min(duration);
        let position = Duration::from_millis(clamped.as_millis() as u64);
        let _ = self.set_position(position);
    }

    /// Get stream length in milliseconds if known
    pub fn length_ms(&self) -> Option<u32> {
        self.current_source.as_ref().map(|source| {
            let millis = source.duration().as_millis();
            let clamped = millis.min(u128::from(u32::MAX));
            clamped as u32
        })
    }
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            priority: Priority::Normal,
            volume: VolumeUtils::linear_to_volume(1.0), // Full volume
            looping: false,
            channel_type: ChannelType::User,
            fade_in_duration: None,
            fade_out_duration: None,
        }
    }
}

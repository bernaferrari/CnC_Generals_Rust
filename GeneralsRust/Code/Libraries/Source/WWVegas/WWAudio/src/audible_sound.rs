//! Core audible sound object mirroring WWAudio's `AudibleSoundClass`.

use crate::{
    aud_time::audio_get_time,
    error::Result,
    handles::Sound2DHandle,
    math::{Matrix3D, Vector3},
    sound_scene_obj::{SoundObjectId, SoundSceneObject},
    sound_types::{SoundClassId, SoundState, SoundType},
    AudioSource, Priority, Volume,
};
use log::warn;
use std::sync::Arc;

/// Runtime data representing a playable sound instance.
#[derive(Clone)]
pub struct AudibleSound {
    pub base: SoundSceneObject,
    pub sound_type: SoundType,
    pub source: Option<Arc<AudioSource>>,
    pub handle: Option<Sound2DHandle>,
    pub priority_scalar: f32,
    pub runtime_priority: i32,
    pub volume: f32,
    pub pan: i32,
    pub loop_count: u32,
    pub max_loop_count: u32,
    pub auto_calc_velocity: bool,
    pub last_position: Vector3,
    pub pitch_factor: f32,
    pub is_muted: bool,
    pub is_fading: bool,
    pub fade_target_volume: f32,
    pub fade_duration_ms: u32,
    pub fade_elapsed_ms: u32,
    pub length_ms: u32,
    pub current_position_ms: u32,
    pub timestamp_ms: u64,
    pub dropoff_radius: f32,
    pub start_offset: f32,
    pub is_dirty: bool,
    pub definition_id: Option<u32>,
    pub logical_sound_id: Option<SoundObjectId>,
    current_frame_override: Option<u64>,
}

impl AudibleSound {
    fn sample_rate_hz(&self) -> u32 {
        self.source
            .as_ref()
            .map(|source| u32::from(source.format().sample_rate))
            .unwrap_or(44_100)
    }

    pub fn playback_rate(&self) -> u32 {
        self.current_playback_rate()
            .unwrap_or_else(|| self.sample_rate_hz())
    }

    pub fn set_playback_rate(&mut self, rate: u32) {
        let base = self.sample_rate_hz().max(1);
        let factor = (rate as f32 / base as f32).clamp(0.01, 4.0);
        self.set_pitch_factor(factor);
    }

    pub fn current_frame(&self) -> u64 {
        if let Some(frame) = self.current_frame_override {
            return frame;
        }
        let rate = self.sample_rate_hz().max(1);
        (u64::from(self.current_position_ms) * u64::from(rate)) / 1000
    }

    pub fn set_current_frame(&mut self, frame: u64) {
        let rate = self.sample_rate_hz().max(1);
        let millis = ((frame as f64) * 1000.0 / rate as f64)
            .round()
            .clamp(0.0, u32::MAX as f64) as u32;
        self.current_position_ms = millis;
        self.current_frame_override = Some(frame);
        if let Some(handle) = self.handle.as_ref() {
            if let Err(err) = handle.set_sample_ms_position(millis) {
                warn!(
                    "Failed to update playback position for sound {}: {err:?}",
                    self.base.id
                );
            }
        }
    }

    pub fn new(id: SoundObjectId, class_id: SoundClassId) -> Self {
        Self {
            base: SoundSceneObject::new(id, class_id),
            sound_type: SoundType::SoundEffect,
            source: None,
            handle: None,
            priority_scalar: 1.0,
            runtime_priority: 0,
            volume: 1.0,
            pan: 0,
            loop_count: 1,
            max_loop_count: 1,
            auto_calc_velocity: false,
            last_position: Vector3::ZERO,
            pitch_factor: 1.0,
            is_muted: false,
            is_fading: false,
            fade_target_volume: 1.0,
            fade_duration_ms: 0,
            fade_elapsed_ms: 0,
            length_ms: 0,
            current_position_ms: 0,
            timestamp_ms: 0,
            dropoff_radius: 1.0,
            start_offset: 0.0,
            is_dirty: true,
            definition_id: None,
            logical_sound_id: None,
            current_frame_override: None,
        }
    }

    pub fn set_source(&mut self, source: Arc<AudioSource>) {
        self.length_ms = source.metadata().duration_ms.min(u64::from(u32::MAX)) as u32;
        self.current_position_ms = 0;
        self.current_frame_override = None;
        self.source = Some(source.clone());
        if let Some(handle) = self.handle.as_mut() {
            handle.initialize(source);
        }
        self.is_dirty = true;
    }

    pub fn set_handle(&mut self, mut handle: Sound2DHandle) {
        if let Some(source) = self.source.as_ref() {
            handle.initialize(Arc::clone(source));
        }
        self.handle = Some(handle);
        self.is_dirty = true;
    }

    pub fn set_sound_type(&mut self, sound_type: SoundType) {
        self.sound_type = sound_type;
        self.is_dirty = true;
    }

    pub fn set_priority_scalar(&mut self, scalar: f32) {
        self.priority_scalar = scalar.max(0.0);
        self.is_dirty = true;
    }

    pub fn priority(&self) -> f32 {
        self.base.priority() * self.priority_scalar
    }

    pub fn mark_static(&mut self, is_static: bool) {
        self.base.mark_static(is_static);
        self.is_dirty = true;
    }

    pub fn play(&mut self, looping: bool) -> Result<()> {
        self.base.set_state(SoundState::Playing);
        self.base.set_user_priority(Priority::Normal);
        self.base.flags.is_culled = false;
        self.is_dirty = true;

        self.loop_count = if looping {
            0
        } else {
            self.max_loop_count.max(1)
        };
        self.timestamp_ms = audio_get_time().as_millis();
        let start_offset_ms = (self.start_offset.clamp(0.0, 1.0) * self.length_ms as f32) as u32;
        self.current_position_ms = start_offset_ms;
        self.current_frame_override = None;

        if let Some(handle) = self.handle.as_ref() {
            handle.set_sample_loop_count(self.loop_count)?;
            handle.set_sample_pan(self.pan)?;
            let miles = (i32::from(self.effective_volume_as_level()) * 127 / 100).clamp(0, 127);
            handle.set_sample_volume(miles)?;
            if let Some(rate) = self.current_playback_rate() {
                handle.set_sample_playback_rate(rate)?;
            }
            if start_offset_ms > 0 {
                handle.set_sample_ms_position(start_offset_ms)?;
            }
            handle.start_sample()?;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if self.base.state() == SoundState::Playing {
            if let Some(handle) = self.handle.as_ref() {
                handle.pause_sample()?;
            }
            self.base.set_state(SoundState::Paused);
            self.is_dirty = true;
        }
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if self.base.state() == SoundState::Paused {
            if let Some(handle) = self.handle.as_ref() {
                handle.resume_sample()?;
            }
            self.base.set_state(SoundState::Playing);
            self.is_dirty = true;
        }
        Ok(())
    }

    pub fn stop(&mut self, remove_from_scene: bool) -> Result<()> {
        if let Some(handle) = self.handle.as_ref() {
            handle.stop_sample()?;
        }
        self.base.set_state(SoundState::Stopped);
        if remove_from_scene {
            self.base.mark_culled(true);
        }
        self.current_position_ms = 0;
        self.current_frame_override = None;
        self.is_dirty = true;
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.is_dirty = true;
        self.apply_volume_to_handle();
    }

    pub fn volume(&self) -> f32 {
        if self.is_muted {
            0.0
        } else {
            self.volume
        }
    }

    pub fn set_muted(&mut self, muted: bool) {
        if self.is_muted != muted {
            self.is_muted = muted;
            self.is_dirty = true;
            self.apply_volume_to_handle();
        }
    }

    pub fn set_pan(&mut self, pan: i32) {
        self.pan = pan.clamp(-1000, 1000);
        self.is_dirty = true;
        if let Some(handle) = self.handle.as_ref() {
            if let Err(err) = handle.set_sample_pan(self.pan) {
                warn!("Failed to update pan for sound {}: {err:?}", self.base.id);
            }
        }
    }

    pub fn pan(&self) -> i32 {
        self.pan
    }

    pub fn set_loop_count(&mut self, count: u32) {
        self.max_loop_count = count;
        self.loop_count = count;
        self.is_dirty = true;
        if let Some(handle) = self.handle.as_ref() {
            if let Err(err) = handle.set_sample_loop_count(count) {
                warn!(
                    "Failed to update loop count for sound {}: {err:?}",
                    self.base.id
                );
            }
        }
    }

    pub fn loop_count(&self) -> u32 {
        self.loop_count
    }

    pub fn set_runtime_priority(&mut self, value: i32) {
        self.runtime_priority = value;
        self.is_dirty = true;
    }

    pub fn runtime_priority(&self) -> i32 {
        self.runtime_priority
    }

    pub fn set_dropoff_radius(&mut self, radius: f32) {
        self.dropoff_radius = radius.max(0.0);
        self.is_dirty = true;
    }

    pub fn dropoff_radius(&self) -> f32 {
        self.dropoff_radius
    }

    pub fn set_start_offset(&mut self, offset_fraction: f32) {
        self.start_offset = offset_fraction.clamp(0.0, 1.0);
        let offset_ms = (self.start_offset * self.length_ms as f32) as u32;
        self.current_position_ms = offset_ms;
        self.current_frame_override = None;
        self.is_dirty = true;
        if let Some(handle) = self.handle.as_ref() {
            if let Err(err) = handle.set_sample_ms_position(offset_ms) {
                warn!(
                    "Failed to update start offset for sound {}: {err:?}",
                    self.base.id
                );
            }
        }
    }

    pub fn start_offset(&self) -> f32 {
        self.start_offset
    }

    pub fn set_pitch_factor(&mut self, pitch: f32) {
        self.pitch_factor = pitch.max(0.01);
        self.is_dirty = true;
        if let Some(handle) = self.handle.as_ref() {
            if let Some(rate) = self.current_playback_rate() {
                if let Err(err) = handle.set_sample_playback_rate(rate) {
                    warn!(
                        "Failed to update playback rate for sound {}: {err:?}",
                        self.base.id
                    );
                }
            }
        }
    }

    pub fn pitch_factor(&self) -> f32 {
        self.pitch_factor
    }

    pub fn seek(&mut self, milliseconds: u32) -> Result<()> {
        self.current_position_ms = milliseconds;
        self.current_frame_override = None;
        self.is_dirty = true;
        if let Some(handle) = self.handle.as_ref() {
            handle.set_sample_ms_position(milliseconds)?;
        }
        Ok(())
    }

    pub fn set_definition_id(&mut self, id: Option<u32>) {
        self.definition_id = id;
        self.is_dirty = true;
    }

    pub fn definition_id(&self) -> Option<u32> {
        self.definition_id
    }

    pub fn set_logical_sound_id(&mut self, id: Option<SoundObjectId>) {
        self.logical_sound_id = id;
        self.is_dirty = true;
    }

    pub fn logical_sound_id(&self) -> Option<SoundObjectId> {
        self.logical_sound_id
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn clear_dirty(&mut self) {
        self.is_dirty = false;
    }

    pub fn set_transform(&mut self, transform: Matrix3D) {
        self.last_position = self.base.position();
        self.base.set_transform(transform);
        self.is_dirty = true;
    }

    pub fn set_position(&mut self, position: Vector3) {
        self.last_position = self.base.position();
        self.base.set_position(position);
        self.is_dirty = true;
    }

    pub fn position(&self) -> Vector3 {
        self.base.position()
    }

    pub fn set_velocity(&mut self, velocity: Vector3) {
        self.base.set_velocity(velocity);
        self.auto_calc_velocity = false;
        self.is_dirty = true;
    }

    pub fn velocity(&self) -> Vector3 {
        self.base.velocity()
    }

    pub fn enable_auto_velocity(&mut self, enabled: bool) {
        self.auto_calc_velocity = enabled;
    }

    pub fn auto_velocity_enabled(&self) -> bool {
        self.auto_calc_velocity
    }

    pub fn update_velocity_from_position(&mut self, elapsed_ms: f32) {
        if !self.auto_calc_velocity {
            return;
        }

        if elapsed_ms > 0.0 {
            let new_position = self.base.position();
            let delta = new_position - self.last_position;
            self.last_position = new_position;
            self.base.set_velocity(delta / elapsed_ms.max(1.0));
        }
    }

    pub fn update_fade(&mut self, delta_ms: u32) {
        if !self.is_fading || self.fade_duration_ms == 0 {
            return;
        }

        self.fade_elapsed_ms = self.fade_elapsed_ms.saturating_add(delta_ms);
        let t = (self.fade_elapsed_ms as f32 / self.fade_duration_ms as f32).clamp(0.0, 1.0);
        self.volume = self.volume + (self.fade_target_volume - self.volume) * t;
        self.apply_volume_to_handle();
        if t >= 1.0 {
            self.is_fading = false;
        }
    }

    pub fn start_fade(&mut self, target_volume: f32, duration_ms: u32) {
        self.fade_target_volume = target_volume.clamp(0.0, 1.0);
        self.fade_duration_ms = duration_ms;
        self.fade_elapsed_ms = 0;
        self.is_fading = true;
        self.is_dirty = true;
    }

    pub fn on_loop_end(&mut self) {
        if self.max_loop_count != 0 {
            self.loop_count = self.loop_count.saturating_sub(1);
            if self.loop_count == 0 {
                self.base.set_state(SoundState::Stopped);
                if let Some(handle) = self.handle.as_ref() {
                    if let Err(err) = handle.stop_sample() {
                        warn!("Failed to stop sound {} at loop end: {err:?}", self.base.id);
                    }
                }
            }
        }
        self.is_dirty = true;
    }

    fn effective_volume_as_level(&self) -> Volume {
        let linear = if self.is_muted {
            0.0
        } else {
            self.volume.clamp(0.0, 1.0)
        };
        (linear * 100.0).round().clamp(0.0, 100.0) as Volume
    }

    pub fn set_culled(&mut self, culled: bool) {
        self.base.mark_culled(culled);
        self.is_dirty = true;
    }

    fn apply_volume_to_handle(&self) {
        if let Some(handle) = self.handle.as_ref() {
            let level = self.effective_volume_as_level();
            let miles = (i32::from(level) * 127 / 100).clamp(0, 127);
            if let Err(err) = handle.set_sample_volume(miles) {
                warn!(
                    "Failed to update volume for sound {}: {err:?}",
                    self.base.id
                );
            }
        }
    }

    fn current_playback_rate(&self) -> Option<u32> {
        let source = self.source.as_ref()?;
        let format = source.format();
        let base_rate: u32 = format.sample_rate.into();
        let rate = (base_rate as f32 * self.pitch_factor).round() as u32;
        Some(rate.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        formats::{AudioFormat, ChannelLayout, SampleRate, SampleWidth},
        handles::Sound2DHandle,
        sound_types::SoundClassId,
        AudioMixer, MixerConfig,
    };
    use std::sync::Arc;

    fn build_test_source() -> (Arc<AudioSource>, u32) {
        let format = AudioFormat {
            channels: 1,
            sample_rate: SampleRate::Hz44100,
            sample_width: SampleWidth::S16,
            channel_layout: ChannelLayout::Mono,
        };
        let samples = vec![0u8; 2 * 64];
        let source = AudioSource::from_memory(samples, format).expect("audio source");
        let duration = source.metadata().duration_ms.min(u64::from(u32::MAX)) as u32;
        (Arc::new(source), duration)
    }

    #[test]
    fn audible_sound_updates_playback_state() {
        let mixer = Arc::new(AudioMixer::new(MixerConfig::default()));

        let mut channel =
            crate::channel::AudioChannel::new(1, Priority::Normal, Arc::clone(&mixer));
        channel.set_loop_count(1);
        let mut sound_handle = Sound2DHandle::new(channel);
        let (source, _length_ms) = build_test_source();
        sound_handle.initialize(Arc::clone(&source));

        let mut audible = AudibleSound::new(100, SoundClassId::TwoD);
        audible.set_source(Arc::clone(&source));
        audible.set_handle(sound_handle.clone());

        audible.set_loop_count(3);
        assert_eq!(audible.loop_count(), 3);
        let loop_count = sound_handle
            .channel()
            .lock()
            .expect("channel lock")
            .loop_count();
        assert_eq!(loop_count, 3);

        audible.set_start_offset(0.5);
        let expected_offset = audible.current_position_ms;
        let (_, handle_position) = sound_handle.get_sample_ms_position().unwrap();
        assert_eq!(handle_position, expected_offset);

        audible.set_pitch_factor(1.5);
        assert!((audible.pitch_factor() - 1.5).abs() < f32::EPSILON);
        let expected_rate = (44100f32 * 1.5).round() as u32;
        assert_eq!(
            sound_handle.get_sample_playback_rate().unwrap(),
            expected_rate
        );
    }
}

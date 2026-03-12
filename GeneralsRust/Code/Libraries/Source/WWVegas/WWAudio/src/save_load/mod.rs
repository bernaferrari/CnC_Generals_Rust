use crate::{
    audible_sound::AudibleSound,
    logical_sound::LogicalSound,
    math::{Matrix3D, Vector3},
    mixer::{
        VoiceDescriptor, VoiceHandle, VoiceParams, VoicePlaybackState, VoiceSpatialMode,
        VoiceSpatialParams, VoiceTimelineState,
    },
    sound3d::Sound3D,
    sound_pseudo3d::SoundPseudo3D,
    sound_scene::SceneSound,
    sound_scene_obj::SoundObjectId,
    sound_types::SoundClassId,
};

#[derive(Debug, Clone, Default)]
pub struct SavedSoundRecord {
    pub id: SoundObjectId,
    pub class_id: SoundClassId,
    pub position: Vector3,
    pub priority: f32,
    pub dropoff_radius: f32,
    pub volume: f32,
    pub pan: i32,
    pub loop_count: u32,
    pub playback_rate: u32,
    pub start_frame: u64,
}

#[derive(Debug, Clone)]
pub struct SavedLogicalRecord {
    pub id: SoundObjectId,
    pub type_mask: u32,
    pub position: Vector3,
    pub dropoff_radius: f32,
    pub is_single_shot: bool,
    pub max_listeners: u32,
    pub notify_delay_ms: u32,
    pub last_notification_ms: u32,
    pub listener_timestamp: u32,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SavedMixerVoiceRecord {
    pub handle: VoiceHandle,
    pub channel_id: u32,
    pub miles_handle_id: Option<u32>,
    pub sound_id: Option<SoundObjectId>,
    pub source_identifier: Option<String>,
    pub params: SavedVoiceParams,
    pub timeline: SavedVoiceTimeline,
    pub playback_state: VoicePlaybackState,
    pub memory_source: Option<MemorySourceRecord>,
}

impl Default for SavedMixerVoiceRecord {
    fn default() -> Self {
        Self {
            handle: VoiceHandle::default(),
            channel_id: 0,
            miles_handle_id: None,
            sound_id: None,
            source_identifier: None,
            params: SavedVoiceParams::default(),
            timeline: SavedVoiceTimeline::default(),
            playback_state: VoicePlaybackState::Pending,
            memory_source: None,
        }
    }
}

impl SavedMixerVoiceRecord {
    pub fn from_mixer(
        handle: VoiceHandle,
        descriptor: &VoiceDescriptor,
        timeline: Option<VoiceTimelineState>,
        sound_id: Option<SoundObjectId>,
    ) -> Self {
        let params = SavedVoiceParams::from_voice_params(&descriptor.params);
        let mut record = Self {
            handle,
            channel_id: descriptor.channel_id,
            miles_handle_id: descriptor.handle_id,
            sound_id,
            source_identifier: Some(descriptor.source.identifier().to_string()),
            params,
            timeline: SavedVoiceTimeline::from_voice_params(&descriptor.params),
            playback_state: VoicePlaybackState::Pending,
            memory_source: None,
        };

        if let Some(state) = timeline {
            record.timeline = SavedVoiceTimeline::from_timeline_state(&state);
            record.playback_state = state.state;
            record.timeline.source_rate = state.source_rate;
        }

        if descriptor.source.identifier() == "<memory>" {
            if let Some(sample) = descriptor.source.sample() {
                if let Some(data) = sample.data.as_ref() {
                    let format = descriptor.source.format();
                    let sample_rate: u32 = format.sample_rate.into();
                    let sample_width: u16 = u16::from(u8::from(format.sample_width));
                    record.memory_source = Some(MemorySourceRecord {
                        data: data.clone(),
                        channels: format.channels,
                        sample_rate,
                        sample_width,
                    });
                }
            }
        }

        record
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemorySourceRecord {
    pub data: Vec<u8>,
    pub channels: u16,
    pub sample_rate: u32,
    pub sample_width: u16,
}

#[derive(Debug, Clone)]
pub struct SavedVoiceParams {
    pub gain: f32,
    pub pan: f32,
    pub playback_rate: u32,
    pub loop_count: u32,
    pub start_frame: u64,
    pub is_culled: bool,
    pub spatial: SavedVoiceSpatial,
}

impl Default for SavedVoiceParams {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            playback_rate: 44_100,
            loop_count: 1,
            start_frame: 0,
            is_culled: false,
            spatial: SavedVoiceSpatial::default(),
        }
    }
}

impl SavedVoiceParams {
    pub fn from_voice_params(params: &VoiceParams) -> Self {
        Self {
            gain: params.gain,
            pan: params.pan,
            playback_rate: params.playback_rate,
            loop_count: params.loop_count,
            start_frame: params.start_frame,
            is_culled: params.is_culled,
            spatial: SavedVoiceSpatial::from_spatial_params(&params.spatial),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SavedVoiceSpatial {
    pub mode: VoiceSpatialMode,
    pub position: Vector3,
    pub velocity: Vector3,
    pub listener_position: Vector3,
    pub listener_velocity: Vector3,
    pub listener_right: Vector3,
    pub listener_up: Vector3,
    pub listener_forward: Vector3,
    pub min_distance: f32,
    pub max_distance: f32,
}

impl Default for SavedVoiceSpatial {
    fn default() -> Self {
        Self {
            mode: VoiceSpatialMode::None,
            position: Vector3::ZERO,
            velocity: Vector3::ZERO,
            listener_position: Vector3::ZERO,
            listener_velocity: Vector3::ZERO,
            listener_right: Vector3::new(1.0, 0.0, 0.0),
            listener_up: Vector3::new(0.0, 1.0, 0.0),
            listener_forward: Vector3::new(0.0, 0.0, 1.0),
            min_distance: 0.0,
            max_distance: 1.0,
        }
    }
}

impl SavedVoiceSpatial {
    pub fn from_spatial_params(params: &VoiceSpatialParams) -> Self {
        let transform = params.listener_transform;
        Self {
            mode: params.mode,
            position: params.position,
            velocity: params.velocity,
            listener_position: params.listener_position,
            listener_velocity: params.listener_velocity,
            listener_right: transform.right_vector(),
            listener_up: transform.up_vector(),
            listener_forward: transform.forward_vector(),
            min_distance: params.min_distance,
            max_distance: params.max_distance,
        }
    }

    pub fn to_matrix(&self) -> Matrix3D {
        Matrix3D {
            rows: [
                [
                    self.listener_right.x,
                    self.listener_right.y,
                    self.listener_right.z,
                    0.0,
                ],
                [
                    self.listener_up.x,
                    self.listener_up.y,
                    self.listener_up.z,
                    0.0,
                ],
                [
                    self.listener_forward.x,
                    self.listener_forward.y,
                    self.listener_forward.z,
                    0.0,
                ],
                [
                    self.listener_position.x,
                    self.listener_position.y,
                    self.listener_position.z,
                    1.0,
                ],
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SavedVoiceTimeline {
    pub position_frames: f64,
    pub rendered_frames: u64,
    pub timeline_origin: u64,
    pub last_sequence: u64,
    pub source_rate: u32,
}

impl Default for SavedVoiceTimeline {
    fn default() -> Self {
        Self {
            position_frames: 0.0,
            rendered_frames: 0,
            timeline_origin: 0,
            last_sequence: 0,
            source_rate: 44_100,
        }
    }
}

impl SavedVoiceTimeline {
    pub fn from_timeline_state(state: &VoiceTimelineState) -> Self {
        Self {
            position_frames: state.position_frames,
            rendered_frames: state.rendered_frames,
            timeline_origin: state.timeline_origin,
            last_sequence: state.last_sequence,
            source_rate: state.source_rate,
        }
    }

    pub fn from_voice_params(params: &VoiceParams) -> Self {
        Self {
            position_frames: params.start_frame as f64,
            rendered_frames: 0,
            timeline_origin: params.start_frame,
            last_sequence: 0,
            source_rate: params.playback_rate,
        }
    }
}

impl Default for SavedLogicalRecord {
    fn default() -> Self {
        Self {
            id: 0,
            type_mask: 0,
            position: Vector3::ZERO,
            dropoff_radius: 0.0,
            is_single_shot: false,
            max_listeners: 0,
            notify_delay_ms: 0,
            last_notification_ms: 0,
            listener_timestamp: 0,
            display_name: None,
        }
    }
}

mod dynamic_audio;
/// Audio persistence subsystem similar to the original Static/DynamicAudioSaveLoadClass.
mod serializer;
mod static_audio;

pub use dynamic_audio::DynamicAudioSaveLoad;
pub use serializer::{AudioChunkId, AudioLoadDeserializer, AudioSaveSerializer};
pub use static_audio::StaticAudioSaveLoad;

impl SavedSoundRecord {
    pub fn from_scene_sound(sound: &SceneSound) -> Self {
        match sound {
            SceneSound::Audible(aud) => Self {
                id: aud.base.id,
                class_id: aud.base.class_id,
                position: aud.position(),
                priority: aud.priority(),
                dropoff_radius: 0.0,
                volume: aud.volume(),
                pan: aud.pan(),
                loop_count: aud.loop_count(),
                playback_rate: aud.playback_rate(),
                start_frame: aud.current_frame(),
            },
            SceneSound::Sound3D(sound3d) => Self {
                id: sound3d.base.base.id,
                class_id: sound3d.base.base.class_id,
                position: sound3d.position(),
                priority: sound3d.get_priority(),
                dropoff_radius: sound3d.dropoff_radius(),
                volume: sound3d.base.volume(),
                pan: sound3d.base.pan(),
                loop_count: sound3d.base.loop_count(),
                playback_rate: sound3d.base.playback_rate(),
                start_frame: sound3d.base.current_frame(),
            },
            SceneSound::Pseudo3D(pseudo) => Self {
                id: pseudo.base.base.base.id,
                class_id: pseudo.base.base.base.class_id,
                position: pseudo.base.position(),
                priority: pseudo.base.get_priority(),
                dropoff_radius: pseudo.base.dropoff_radius_value(),
                volume: pseudo.base.base.volume(),
                pan: pseudo.base.base.pan(),
                loop_count: pseudo.base.base.loop_count(),
                playback_rate: pseudo.base.base.playback_rate(),
                start_frame: pseudo.base.base.current_frame(),
            },
        }
    }

    pub fn instantiate(&self) -> SceneSound {
        match self.class_id {
            SoundClassId::ThreeD => {
                let mut sound = Sound3D::new(self.id);
                sound.set_position(self.position);
                sound.set_dropoff_radius(self.dropoff_radius);
                let audible = sound.as_audible_mut();
                audible.set_priority_scalar(self.priority);
                audible.set_volume(self.volume);
                audible.set_pan(self.pan);
                audible.set_loop_count(self.loop_count);
                audible.set_playback_rate(self.playback_rate);
                audible.set_current_frame(self.start_frame);
                SceneSound::Sound3D(sound)
            }
            SoundClassId::Pseudo3D => {
                let mut sound = SoundPseudo3D::new(self.id);
                sound.base.set_position(self.position);
                sound.base.set_dropoff_radius(self.dropoff_radius);
                let audible = sound.base.as_audible_mut();
                audible.set_priority_scalar(self.priority);
                audible.set_volume(self.volume);
                audible.set_pan(self.pan);
                audible.set_loop_count(self.loop_count);
                audible.set_playback_rate(self.playback_rate);
                audible.set_current_frame(self.start_frame);
                SceneSound::Pseudo3D(sound)
            }
            _ => {
                let mut audible = AudibleSound::new(self.id, self.class_id);
                audible.set_position(self.position);
                audible.base.set_priority(self.priority);
                audible.set_volume(self.volume);
                audible.set_pan(self.pan);
                audible.set_loop_count(self.loop_count);
                audible.set_playback_rate(self.playback_rate);
                audible.set_current_frame(self.start_frame);
                SceneSound::Audible(audible)
            }
        }
    }
}

impl SavedLogicalRecord {
    pub fn from_logical(logical: &LogicalSound) -> Self {
        Self {
            id: logical.base.id,
            type_mask: logical.type_mask(),
            position: logical.position(),
            dropoff_radius: logical.dropoff_radius(),
            is_single_shot: logical.is_single_shot(),
            max_listeners: logical.max_listeners() as u32,
            notify_delay_ms: logical.notify_delay(),
            last_notification_ms: logical.last_notification(),
            listener_timestamp: logical.listener_timestamp(),
            display_name: None,
        }
    }

    pub fn instantiate(&self) -> LogicalSound {
        let mut logical = LogicalSound::new(self.id);
        logical.set_type_mask(self.type_mask);
        logical.set_position(self.position);
        logical.set_dropoff_radius(self.dropoff_radius);
        logical.set_single_shot(self.is_single_shot);
        logical.set_max_listeners(self.max_listeners as usize);
        logical.set_notify_delay(self.notify_delay_ms);
        logical.set_last_notification(self.last_notification_ms);
        logical.set_listener_timestamp(self.listener_timestamp);
        logical
    }
}

use super::{
    AudioChunkId, AudioLoadDeserializer, AudioSaveSerializer, MemorySourceRecord,
    SavedLogicalRecord, SavedMixerVoiceRecord, SavedSoundRecord,
};
use crate::{
    chunk::{Chunk, ChunkReader, ChunkWriter, MicroChunkReader},
    error::Result,
    logical::list::LogicalSoundRegistry,
    logical_listener::LogicalListener,
    math::Vector3,
    mixer::{MixerTimelineSnapshot, VoiceHandle, VoicePlaybackState, VoiceSpatialMode},
    sound_scene::SoundScene,
    sound_types::SoundClassId,
};
use std::io::{Read, Seek, Write};

/// Rust analogue of `DynamicAudioSaveLoadClass`
#[derive(Debug)]
pub struct DynamicAudioSaveLoad {
    loaded_dynamic_sounds: Vec<SavedSoundRecord>,
    loaded_logical_sounds: Vec<SavedLogicalRecord>,
    loaded_mixer_voices: Vec<SavedMixerVoiceRecord>,
    logical_listener_global_scale: f32,
    mixer_snapshot: Option<MixerTimelineSnapshot>,
}

impl Default for DynamicAudioSaveLoad {
    fn default() -> Self {
        Self {
            loaded_dynamic_sounds: Vec::new(),
            loaded_logical_sounds: Vec::new(),
            loaded_mixer_voices: Vec::new(),
            logical_listener_global_scale: 1.0,
            mixer_snapshot: None,
        }
    }
}

const CHUNKID_DYNAMIC_SCENE: u32 = 0x1029_1221;
const CHUNKID_DYNAMIC_VARIABLES: u32 = 0x1029_1222;
const CHUNKID_DYNAMIC_MIXER_STATE: u32 = 0x0003_0200;
const CHUNKID_DYNAMIC_SOUND_ENTRY: u32 = 0x0003_0201;
const CHUNKID_DYNAMIC_LOGICAL_ENTRY: u32 = 0x0003_0202;
const CHUNKID_DYNAMIC_VOICE_ENTRY: u32 = 0x0003_0203;

const VARID_LOGICAL_LISTENER_GLOBAL_SCALE: u8 = 0x04;

const VARID_MIXER_SEQUENCE: u8 = 0x01;
const VARID_MIXER_FRAME: u8 = 0x02;
const VARID_MIXER_SAMPLE_RATE: u8 = 0x03;

const VARID_SOUND_ID: u8 = 0x01;
const VARID_SOUND_CLASS: u8 = 0x02;
const VARID_SOUND_POSITION: u8 = 0x03;
const VARID_SOUND_PRIORITY: u8 = 0x04;
const VARID_SOUND_DROPOFF: u8 = 0x05;
const VARID_SOUND_VOLUME: u8 = 0x06;
const VARID_SOUND_PAN: u8 = 0x07;
const VARID_SOUND_LOOP_COUNT: u8 = 0x08;
const VARID_SOUND_PLAYBACK_RATE: u8 = 0x09;
const VARID_SOUND_START_FRAME: u8 = 0x0A;

const VARID_LOGICAL_ID: u8 = 0x01;
const VARID_LOGICAL_TYPE_MASK: u8 = 0x02;
const VARID_LOGICAL_POSITION: u8 = 0x03;
const VARID_LOGICAL_DROPOFF: u8 = 0x04;
const VARID_LOGICAL_SINGLE_SHOT: u8 = 0x05;
const VARID_LOGICAL_MAX_LISTENERS: u8 = 0x06;
const VARID_LOGICAL_NOTIFY_DELAY: u8 = 0x07;
const VARID_LOGICAL_LAST_NOTIFY: u8 = 0x08;
const VARID_LOGICAL_LISTENER_TIMESTAMP: u8 = 0x09;
const VARID_LOGICAL_DISPLAY_NAME: u8 = 0x0A;

const VARID_VOICE_HANDLE_ID: u8 = 0x10;
const VARID_VOICE_HANDLE_GENERATION: u8 = 0x11;
const VARID_VOICE_CHANNEL_ID: u8 = 0x12;
const VARID_VOICE_MILES_HANDLE: u8 = 0x13;
const VARID_VOICE_SOURCE_NAME: u8 = 0x14;
const VARID_VOICE_GAIN: u8 = 0x15;
const VARID_VOICE_PAN: u8 = 0x16;
const VARID_VOICE_PLAYBACK_RATE: u8 = 0x17;
const VARID_VOICE_LOOP_COUNT: u8 = 0x18;
const VARID_VOICE_START_FRAME: u8 = 0x19;
const VARID_VOICE_IS_CULLED: u8 = 0x1A;
const VARID_VOICE_SPATIAL_MODE: u8 = 0x1B;
const VARID_VOICE_SPATIAL_POSITION: u8 = 0x1C;
const VARID_VOICE_SPATIAL_VELOCITY: u8 = 0x1D;
const VARID_VOICE_LISTENER_POSITION: u8 = 0x1E;
const VARID_VOICE_LISTENER_VELOCITY: u8 = 0x1F;
const VARID_VOICE_LISTENER_RIGHT: u8 = 0x20;
const VARID_VOICE_LISTENER_UP: u8 = 0x21;
const VARID_VOICE_LISTENER_FORWARD: u8 = 0x22;
const VARID_VOICE_MIN_DISTANCE: u8 = 0x23;
const VARID_VOICE_MAX_DISTANCE: u8 = 0x24;
const VARID_VOICE_TIMELINE_POSITION: u8 = 0x25;
const VARID_VOICE_TIMELINE_RENDERED: u8 = 0x26;
const VARID_VOICE_TIMELINE_ORIGIN: u8 = 0x27;
const VARID_VOICE_TIMELINE_SEQUENCE: u8 = 0x28;
const VARID_VOICE_SOURCE_RATE: u8 = 0x29;
const VARID_VOICE_PLAYBACK_STATE: u8 = 0x2A;
const VARID_VOICE_SOUND_ID: u8 = 0x2B;
const VARID_VOICE_MEMORY_CHANNELS: u8 = 0x2C;
const VARID_VOICE_MEMORY_SAMPLE_RATE: u8 = 0x2D;
const VARID_VOICE_MEMORY_SAMPLE_WIDTH: u8 = 0x2E;
const VARID_VOICE_MEMORY_DATA: u8 = 0x2F;

impl DynamicAudioSaveLoad {
    pub fn chunk_id() -> AudioChunkId {
        AudioChunkId::Dynamic
    }

    pub fn save<W: Write + Seek>(
        &mut self,
        scene: &SoundScene,
        registry: &LogicalSoundRegistry,
        voices: &[SavedMixerVoiceRecord],
        writer: W,
    ) -> Result<()> {
        let mut serializer = AudioSaveSerializer::new(writer, Self::chunk_id())?;
        let mut chunk = ChunkWriter::new();

        let global_scale = LogicalListener::global_scale();
        chunk.begin_chunk(CHUNKID_DYNAMIC_VARIABLES);
        chunk.begin_micro_chunk(VARID_LOGICAL_LISTENER_GLOBAL_SCALE);
        chunk.write_f32(global_scale);
        chunk.end_micro_chunk();
        chunk.end_chunk();

        let snapshot = self
            .mixer_snapshot
            .unwrap_or_else(|| MixerTimelineSnapshot {
                sequence: 0,
                current_frame: 0,
                sample_rate: 44_100,
            });

        let dynamic_records: Vec<SavedSoundRecord> = scene
            .dynamic_sounds
            .iter()
            .map(SavedSoundRecord::from_scene_sound)
            .collect();

        let logical_records: Vec<SavedLogicalRecord> = scene
            .logical_sounds
            .iter()
            .map(|logical| {
                let mut record = SavedLogicalRecord::from_logical(logical);
                if let Some(entry) = registry.lookup(logical.base.id) {
                    record.display_name = entry.display.clone();
                }
                record
            })
            .collect();

        chunk.begin_chunk(CHUNKID_DYNAMIC_SCENE);

        chunk.begin_chunk(CHUNKID_DYNAMIC_MIXER_STATE);
        chunk.begin_micro_chunk(VARID_MIXER_SEQUENCE);
        chunk.write_u32(snapshot.sequence as u32);
        chunk.end_micro_chunk();
        chunk.begin_micro_chunk(VARID_MIXER_FRAME);
        chunk.write_bytes(&snapshot.current_frame.to_le_bytes());
        chunk.end_micro_chunk();
        chunk.begin_micro_chunk(VARID_MIXER_SAMPLE_RATE);
        chunk.write_u32(snapshot.sample_rate);
        chunk.end_micro_chunk();
        chunk.end_chunk();

        for record in &dynamic_records {
            chunk.begin_chunk(CHUNKID_DYNAMIC_SOUND_ENTRY);

            chunk.begin_micro_chunk(VARID_SOUND_ID);
            chunk.write_u32(record.id);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_CLASS);
            chunk.write_u32(record.class_id as u32);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_POSITION);
            write_vector3(&mut chunk, record.position);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_PRIORITY);
            chunk.write_f32(record.priority);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_DROPOFF);
            chunk.write_f32(record.dropoff_radius);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_VOLUME);
            chunk.write_f32(record.volume);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_PAN);
            chunk.write_bytes(&record.pan.to_le_bytes());
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_LOOP_COUNT);
            chunk.write_u32(record.loop_count);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_PLAYBACK_RATE);
            chunk.write_u32(record.playback_rate);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_SOUND_START_FRAME);
            chunk.write_bytes(&record.start_frame.to_le_bytes());
            chunk.end_micro_chunk();

            chunk.end_chunk();
        }

        for record in &logical_records {
            chunk.begin_chunk(CHUNKID_DYNAMIC_LOGICAL_ENTRY);

            chunk.begin_micro_chunk(VARID_LOGICAL_ID);
            chunk.write_u32(record.id);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_TYPE_MASK);
            chunk.write_u32(record.type_mask);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_POSITION);
            write_vector3(&mut chunk, record.position);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_DROPOFF);
            chunk.write_f32(record.dropoff_radius);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_SINGLE_SHOT);
            chunk.write_u8(if record.is_single_shot { 1 } else { 0 });
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_MAX_LISTENERS);
            chunk.write_u32(record.max_listeners);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_NOTIFY_DELAY);
            chunk.write_u32(record.notify_delay_ms);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_LAST_NOTIFY);
            chunk.write_u32(record.last_notification_ms);
            chunk.end_micro_chunk();

            chunk.begin_micro_chunk(VARID_LOGICAL_LISTENER_TIMESTAMP);
            chunk.write_u32(record.listener_timestamp);
            chunk.end_micro_chunk();

            if let Some(display) = record.display_name.as_ref() {
                if !display.is_empty() {
                    chunk.begin_micro_chunk(VARID_LOGICAL_DISPLAY_NAME);
                    write_string(&mut chunk, display);
                    chunk.end_micro_chunk();
                }
            }

            chunk.end_chunk();
        }

        for voice in voices {
            write_voice_record(&mut chunk, voice);
        }

        chunk.end_chunk(); // CHUNKID_DYNAMIC_SCENE

        let data = chunk.finish();
        serializer.write_bytes(&data)?;
        let writer = serializer.finish()?;
        drop(writer);

        self.loaded_dynamic_sounds = dynamic_records;
        self.loaded_logical_sounds = logical_records;
        self.loaded_mixer_voices = voices.to_vec();
        self.logical_listener_global_scale = global_scale;
        self.mixer_snapshot = Some(snapshot);
        Ok(())
    }

    pub fn load<R: Read + Seek>(&mut self, reader: R) -> Result<()> {
        let mut loader = AudioLoadDeserializer::new(reader, Self::chunk_id())?;
        let payload = loader.read_remaining_bytes()?;

        self.loaded_dynamic_sounds.clear();
        self.loaded_logical_sounds.clear();
        self.loaded_mixer_voices.clear();
        self.logical_listener_global_scale = 1.0;
        self.mixer_snapshot = None;

        let mut root = ChunkReader::new(&payload);
        while let Some(chunk) = root.next() {
            match chunk.id {
                CHUNKID_DYNAMIC_VARIABLES => self.parse_variables(chunk.data()),
                CHUNKID_DYNAMIC_SCENE => self.parse_scene(chunk.data()),
                _ => {}
            }
        }

        Ok(())
    }

    fn parse_variables(&mut self, data: &[u8]) {
        let mut reader = MicroChunkReader::new(data);
        while let Some(micro) = reader.next() {
            if micro.id == VARID_LOGICAL_LISTENER_GLOBAL_SCALE {
                if let Some(scale) = micro.as_f32() {
                    self.logical_listener_global_scale = scale;
                }
            }
        }
    }

    fn parse_scene(&mut self, data: &[u8]) {
        let mut reader = ChunkReader::new(data);
        while let Some(chunk) = reader.next() {
            match chunk.id {
                CHUNKID_DYNAMIC_MIXER_STATE => {
                    if let Some(snapshot) = parse_mixer_chunk(&chunk) {
                        self.mixer_snapshot = Some(snapshot);
                    }
                }
                CHUNKID_DYNAMIC_SOUND_ENTRY => {
                    if let Some(record) = parse_sound_chunk(&chunk) {
                        self.loaded_dynamic_sounds.push(record);
                    }
                }
                CHUNKID_DYNAMIC_LOGICAL_ENTRY => {
                    if let Some(record) = parse_logical_chunk(&chunk) {
                        self.loaded_logical_sounds.push(record);
                    }
                }
                CHUNKID_DYNAMIC_VOICE_ENTRY => {
                    if let Some(record) = parse_voice_chunk(&chunk) {
                        self.loaded_mixer_voices.push(record);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn loaded_dynamic_sounds(&self) -> &[SavedSoundRecord] {
        &self.loaded_dynamic_sounds
    }

    pub fn loaded_logical_sounds(&self) -> &[SavedLogicalRecord] {
        &self.loaded_logical_sounds
    }

    pub fn loaded_mixer_voices(&self) -> &[SavedMixerVoiceRecord] {
        &self.loaded_mixer_voices
    }

    pub fn logical_listener_global_scale(&self) -> f32 {
        self.logical_listener_global_scale
    }

    pub fn set_mixer_snapshot(&mut self, snapshot: MixerTimelineSnapshot) {
        self.mixer_snapshot = Some(snapshot);
    }

    pub fn mixer_snapshot(&self) -> Option<MixerTimelineSnapshot> {
        self.mixer_snapshot
    }
}

fn parse_mixer_chunk(chunk: &Chunk<'_>) -> Option<MixerTimelineSnapshot> {
    let mut snapshot = MixerTimelineSnapshot {
        sequence: 0,
        current_frame: 0,
        sample_rate: 44_100,
    };
    let mut touched = false;
    let mut reader = chunk.micro_chunks();
    while let Some(micro) = reader.next() {
        match micro.id {
            VARID_MIXER_SEQUENCE => {
                if let Some(value) = micro.as_u32() {
                    snapshot.sequence = u64::from(value);
                    touched = true;
                }
            }
            VARID_MIXER_FRAME => {
                if let Some(value) = bytes_to_u64(micro.as_bytes()) {
                    snapshot.current_frame = value;
                    touched = true;
                }
            }
            VARID_MIXER_SAMPLE_RATE => {
                if let Some(value) = micro.as_u32() {
                    snapshot.sample_rate = value;
                    touched = true;
                }
            }
            _ => {}
        }
    }
    touched.then_some(snapshot)
}

fn parse_sound_chunk(chunk: &Chunk<'_>) -> Option<SavedSoundRecord> {
    let mut record = SavedSoundRecord::default();
    let mut class_id = None;
    let mut reader = chunk.micro_chunks();
    while let Some(micro) = reader.next() {
        match micro.id {
            VARID_SOUND_ID => {
                if let Some(id) = micro.as_u32() {
                    record.id = id;
                }
            }
            VARID_SOUND_CLASS => {
                if let Some(value) = micro.as_u32() {
                    class_id = Some(SoundClassId::from_u32(value));
                }
            }
            VARID_SOUND_POSITION => {
                record.position = parse_vector3(micro.as_bytes());
            }
            VARID_SOUND_PRIORITY => {
                if let Some(value) = micro.as_f32() {
                    record.priority = value;
                }
            }
            VARID_SOUND_DROPOFF => {
                if let Some(value) = micro.as_f32() {
                    record.dropoff_radius = value;
                }
            }
            VARID_SOUND_VOLUME => {
                if let Some(value) = micro.as_f32() {
                    record.volume = value;
                }
            }
            VARID_SOUND_PAN => {
                if let Some(value) = bytes_to_i32(micro.as_bytes()) {
                    record.pan = value;
                }
            }
            VARID_SOUND_LOOP_COUNT => {
                if let Some(value) = micro.as_u32() {
                    record.loop_count = value;
                }
            }
            VARID_SOUND_PLAYBACK_RATE => {
                if let Some(value) = micro.as_u32() {
                    record.playback_rate = value;
                }
            }
            VARID_SOUND_START_FRAME => {
                if let Some(value) = bytes_to_u64(micro.as_bytes()) {
                    record.start_frame = value;
                }
            }
            _ => {}
        }
    }

    class_id.map(|class| {
        record.class_id = class;
        record
    })
}

fn parse_logical_chunk(chunk: &Chunk<'_>) -> Option<SavedLogicalRecord> {
    let mut record = SavedLogicalRecord::default();
    let mut reader = chunk.micro_chunks();
    while let Some(micro) = reader.next() {
        match micro.id {
            VARID_LOGICAL_ID => {
                if let Some(id) = micro.as_u32() {
                    record.id = id;
                }
            }
            VARID_LOGICAL_TYPE_MASK => {
                if let Some(mask) = micro.as_u32() {
                    record.type_mask = mask;
                }
            }
            VARID_LOGICAL_POSITION => {
                record.position = parse_vector3(micro.as_bytes());
            }
            VARID_LOGICAL_DROPOFF => {
                if let Some(value) = micro.as_f32() {
                    record.dropoff_radius = value;
                }
            }
            VARID_LOGICAL_SINGLE_SHOT => {
                record.is_single_shot = micro.as_bytes().first().copied().unwrap_or(0) != 0;
            }
            VARID_LOGICAL_MAX_LISTENERS => {
                if let Some(value) = micro.as_u32() {
                    record.max_listeners = value;
                }
            }
            VARID_LOGICAL_NOTIFY_DELAY => {
                if let Some(value) = micro.as_u32() {
                    record.notify_delay_ms = value;
                }
            }
            VARID_LOGICAL_LAST_NOTIFY => {
                if let Some(value) = micro.as_u32() {
                    record.last_notification_ms = value;
                }
            }
            VARID_LOGICAL_LISTENER_TIMESTAMP => {
                if let Some(value) = micro.as_u32() {
                    record.listener_timestamp = value;
                }
            }
            VARID_LOGICAL_DISPLAY_NAME => {
                record.display_name = string_from_bytes(micro.as_bytes());
            }
            _ => {}
        }
    }

    Some(record)
}

fn write_voice_record(writer: &mut ChunkWriter, voice: &SavedMixerVoiceRecord) {
    writer.begin_chunk(CHUNKID_DYNAMIC_VOICE_ENTRY);

    writer.begin_micro_chunk(VARID_VOICE_HANDLE_ID);
    writer.write_u32(voice.handle.id());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_HANDLE_GENERATION);
    writer.write_u32(voice.handle.generation());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_CHANNEL_ID);
    writer.write_u32(voice.channel_id);
    writer.end_micro_chunk();

    if let Some(sound_id) = voice.sound_id {
        writer.begin_micro_chunk(VARID_VOICE_SOUND_ID);
        writer.write_u32(sound_id);
        writer.end_micro_chunk();
    }

    if let Some(miles) = voice.miles_handle_id {
        writer.begin_micro_chunk(VARID_VOICE_MILES_HANDLE);
        writer.write_u32(miles);
        writer.end_micro_chunk();
    }

    if let Some(identifier) = voice.source_identifier.as_ref() {
        if !identifier.is_empty() {
            writer.begin_micro_chunk(VARID_VOICE_SOURCE_NAME);
            write_string(writer, identifier);
            writer.end_micro_chunk();
        }
    }

    writer.begin_micro_chunk(VARID_VOICE_GAIN);
    writer.write_f32(voice.params.gain);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_PAN);
    writer.write_f32(voice.params.pan);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_PLAYBACK_RATE);
    writer.write_u32(voice.params.playback_rate);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_LOOP_COUNT);
    writer.write_u32(voice.params.loop_count);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_START_FRAME);
    writer.write_bytes(&voice.params.start_frame.to_le_bytes());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_IS_CULLED);
    writer.write_u8(if voice.params.is_culled { 1 } else { 0 });
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_SPATIAL_MODE);
    writer.write_u32(voice_mode_to_u32(voice.params.spatial.mode));
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_SPATIAL_POSITION);
    write_vector3(writer, voice.params.spatial.position);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_SPATIAL_VELOCITY);
    write_vector3(writer, voice.params.spatial.velocity);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_LISTENER_POSITION);
    write_vector3(writer, voice.params.spatial.listener_position);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_LISTENER_VELOCITY);
    write_vector3(writer, voice.params.spatial.listener_velocity);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_LISTENER_RIGHT);
    write_vector3(writer, voice.params.spatial.listener_right);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_LISTENER_UP);
    write_vector3(writer, voice.params.spatial.listener_up);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_LISTENER_FORWARD);
    write_vector3(writer, voice.params.spatial.listener_forward);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_MIN_DISTANCE);
    writer.write_f32(voice.params.spatial.min_distance);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_MAX_DISTANCE);
    writer.write_f32(voice.params.spatial.max_distance);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_TIMELINE_POSITION);
    writer.write_bytes(&voice.timeline.position_frames.to_le_bytes());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_TIMELINE_RENDERED);
    writer.write_bytes(&voice.timeline.rendered_frames.to_le_bytes());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_TIMELINE_ORIGIN);
    writer.write_bytes(&voice.timeline.timeline_origin.to_le_bytes());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_TIMELINE_SEQUENCE);
    writer.write_bytes(&voice.timeline.last_sequence.to_le_bytes());
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_SOURCE_RATE);
    writer.write_u32(voice.timeline.source_rate);
    writer.end_micro_chunk();

    writer.begin_micro_chunk(VARID_VOICE_PLAYBACK_STATE);
    writer.write_u32(voice_state_to_u32(voice.playback_state));
    writer.end_micro_chunk();

    if let Some(memory) = voice.memory_source.as_ref() {
        writer.begin_micro_chunk(VARID_VOICE_MEMORY_CHANNELS);
        writer.write_u32(memory.channels as u32);
        writer.end_micro_chunk();

        writer.begin_micro_chunk(VARID_VOICE_MEMORY_SAMPLE_RATE);
        writer.write_u32(memory.sample_rate);
        writer.end_micro_chunk();

        writer.begin_micro_chunk(VARID_VOICE_MEMORY_SAMPLE_WIDTH);
        writer.write_u32(memory.sample_width as u32);
        writer.end_micro_chunk();

        if !memory.data.is_empty() {
            for chunk_data in memory.data.chunks(255) {
                writer.begin_micro_chunk(VARID_VOICE_MEMORY_DATA);
                writer.write_bytes(chunk_data);
                writer.end_micro_chunk();
            }
        }
    }

    writer.end_chunk();
}

fn parse_voice_chunk(chunk: &Chunk<'_>) -> Option<SavedMixerVoiceRecord> {
    let mut record = SavedMixerVoiceRecord::default();
    let mut handle_id: Option<u32> = None;
    let mut generation: Option<u32> = None;
    let mut memory_channels: Option<u16> = None;
    let mut memory_sample_rate: Option<u32> = None;
    let mut memory_sample_width: Option<u16> = None;
    let mut memory_data = Vec::new();

    let mut reader = chunk.micro_chunks();
    while let Some(micro) = reader.next() {
        match micro.id {
            VARID_VOICE_HANDLE_ID => handle_id = micro.as_u32(),
            VARID_VOICE_HANDLE_GENERATION => generation = micro.as_u32(),
            VARID_VOICE_CHANNEL_ID => {
                if let Some(value) = micro.as_u32() {
                    record.channel_id = value;
                }
            }
            VARID_VOICE_SOUND_ID => {
                if let Some(value) = micro.as_u32() {
                    record.sound_id = Some(value);
                }
            }
            VARID_VOICE_MILES_HANDLE => {
                if let Some(value) = micro.as_u32() {
                    record.miles_handle_id = Some(value);
                }
            }
            VARID_VOICE_SOURCE_NAME => {
                record.source_identifier = string_from_bytes(micro.as_bytes());
            }
            VARID_VOICE_GAIN => {
                if let Some(value) = micro.as_f32() {
                    record.params.gain = value;
                }
            }
            VARID_VOICE_PAN => {
                if let Some(value) = micro.as_f32() {
                    record.params.pan = value;
                }
            }
            VARID_VOICE_PLAYBACK_RATE => {
                if let Some(value) = micro.as_u32() {
                    record.params.playback_rate = value;
                }
            }
            VARID_VOICE_LOOP_COUNT => {
                if let Some(value) = micro.as_u32() {
                    record.params.loop_count = value;
                }
            }
            VARID_VOICE_START_FRAME => {
                if let Some(value) = bytes_to_u64(micro.as_bytes()) {
                    record.params.start_frame = value;
                }
            }
            VARID_VOICE_IS_CULLED => {
                record.params.is_culled = micro.as_bytes().first().copied().unwrap_or(0) != 0;
            }
            VARID_VOICE_SPATIAL_MODE => {
                if let Some(value) = micro.as_u32() {
                    record.params.spatial.mode = voice_mode_from_u32(value);
                }
            }
            VARID_VOICE_SPATIAL_POSITION => {
                record.params.spatial.position = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_SPATIAL_VELOCITY => {
                record.params.spatial.velocity = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_LISTENER_POSITION => {
                record.params.spatial.listener_position = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_LISTENER_VELOCITY => {
                record.params.spatial.listener_velocity = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_LISTENER_RIGHT => {
                record.params.spatial.listener_right = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_LISTENER_UP => {
                record.params.spatial.listener_up = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_LISTENER_FORWARD => {
                record.params.spatial.listener_forward = parse_vector3(micro.as_bytes());
            }
            VARID_VOICE_MIN_DISTANCE => {
                if let Some(value) = micro.as_f32() {
                    record.params.spatial.min_distance = value;
                }
            }
            VARID_VOICE_MAX_DISTANCE => {
                if let Some(value) = micro.as_f32() {
                    record.params.spatial.max_distance = value;
                }
            }
            VARID_VOICE_TIMELINE_POSITION => {
                if let Some(value) = bytes_to_f64(micro.as_bytes()) {
                    record.timeline.position_frames = value;
                }
            }
            VARID_VOICE_TIMELINE_RENDERED => {
                if let Some(value) = bytes_to_u64(micro.as_bytes()) {
                    record.timeline.rendered_frames = value;
                }
            }
            VARID_VOICE_TIMELINE_ORIGIN => {
                if let Some(value) = bytes_to_u64(micro.as_bytes()) {
                    record.timeline.timeline_origin = value;
                }
            }
            VARID_VOICE_TIMELINE_SEQUENCE => {
                if let Some(value) = bytes_to_u64(micro.as_bytes()) {
                    record.timeline.last_sequence = value;
                }
            }
            VARID_VOICE_SOURCE_RATE => {
                if let Some(value) = micro.as_u32() {
                    record.timeline.source_rate = value;
                }
            }
            VARID_VOICE_PLAYBACK_STATE => {
                if let Some(value) = micro.as_u32() {
                    record.playback_state = voice_state_from_u32(value);
                }
            }
            VARID_VOICE_MEMORY_CHANNELS => {
                if let Some(value) = micro.as_u32() {
                    memory_channels = Some(value as u16);
                }
            }
            VARID_VOICE_MEMORY_SAMPLE_RATE => {
                if let Some(value) = micro.as_u32() {
                    memory_sample_rate = Some(value);
                }
            }
            VARID_VOICE_MEMORY_SAMPLE_WIDTH => {
                if let Some(value) = micro.as_u32() {
                    memory_sample_width = Some(value as u16);
                }
            }
            VARID_VOICE_MEMORY_DATA => {
                memory_data.extend_from_slice(micro.as_bytes());
            }
            _ => {}
        }
    }

    if !memory_data.is_empty() {
        let channels = memory_channels.unwrap_or(1);
        let sample_rate = memory_sample_rate.unwrap_or(44_100);
        let sample_width = memory_sample_width.unwrap_or(16);
        record.memory_source = Some(MemorySourceRecord {
            data: memory_data,
            channels,
            sample_rate,
            sample_width,
        });
    }

    match (handle_id, generation) {
        (Some(id), Some(gen)) => {
            record.handle = VoiceHandle::new(id, gen);
            if record.timeline.source_rate == 0 {
                record.timeline.source_rate = record.params.playback_rate;
            }
            Some(record)
        }
        _ => None,
    }
}

fn write_vector3(writer: &mut ChunkWriter, value: Vector3) {
    writer.write_f32(value.x);
    writer.write_f32(value.y);
    writer.write_f32(value.z);
}

fn write_string(writer: &mut ChunkWriter, value: &str) {
    let bytes = value.as_bytes();
    let slice = if bytes.len() > u8::MAX as usize {
        &bytes[..u8::MAX as usize]
    } else {
        bytes
    };
    writer.write_bytes(slice);
}

fn parse_vector3(bytes: &[u8]) -> Vector3 {
    if bytes.len() >= 12 {
        let x = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let y = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        let z = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
        Vector3::new(x, y, z)
    } else {
        Vector3::ZERO
    }
}

fn bytes_to_u64(bytes: &[u8]) -> Option<u64> {
    (bytes.len() == 8).then(|| u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn bytes_to_f64(bytes: &[u8]) -> Option<f64> {
    (bytes.len() == 8).then(|| f64::from_le_bytes(bytes.try_into().unwrap()))
}

fn bytes_to_i32(bytes: &[u8]) -> Option<i32> {
    (bytes.len() == 4).then(|| i32::from_le_bytes(bytes.try_into().unwrap()))
}

fn string_from_bytes(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn voice_mode_to_u32(mode: VoiceSpatialMode) -> u32 {
    match mode {
        VoiceSpatialMode::None => 0,
        VoiceSpatialMode::Pseudo3D => 1,
        VoiceSpatialMode::Full3D => 2,
    }
}

fn voice_mode_from_u32(value: u32) -> VoiceSpatialMode {
    match value {
        1 => VoiceSpatialMode::Pseudo3D,
        2 => VoiceSpatialMode::Full3D,
        _ => VoiceSpatialMode::None,
    }
}

fn voice_state_to_u32(state: VoicePlaybackState) -> u32 {
    match state {
        VoicePlaybackState::Pending => 0,
        VoicePlaybackState::Playing => 1,
        VoicePlaybackState::Paused => 2,
        VoicePlaybackState::Completed => 3,
    }
}

fn voice_state_from_u32(value: u32) -> VoicePlaybackState {
    match value {
        1 => VoicePlaybackState::Playing,
        2 => VoicePlaybackState::Paused,
        3 => VoicePlaybackState::Completed,
        _ => VoicePlaybackState::Pending,
    }
}

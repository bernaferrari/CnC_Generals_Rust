use super::SavedSoundRecord;
use crate::{
    chunk::{ChunkReader, ChunkWriter, MicroChunkReader},
    error::Result,
    math::Vector3,
    sound_scene::SoundScene,
};
use std::io::{Read, Seek, Write};

#[derive(Debug, Default)]
pub struct StaticAudioSaveLoad {
    loaded_sounds: Vec<SavedSoundRecord>,
    min_extents: Vector3,
    max_extents: Vector3,
}

const CHUNKID_STATIC_SCENE: u32 = 0x1029_1220;
const CHUNKID_VARIABLES: u32 = 0x0000_0100;
const CHUNKID_STATIC_SOUNDS: u32 = 0x0000_0101;
const VARID_MIN_DIM: u8 = 0x01;
const VARID_MAX_DIM: u8 = 0x02;

impl StaticAudioSaveLoad {
    pub fn chunk_id() -> u32 {
        0x0003_0005
    }

    pub fn save<W: Write + Seek>(&mut self, scene: &SoundScene, mut writer: W) -> Result<()> {
        let mut chunk = ChunkWriter::new();
        chunk.begin_chunk(CHUNKID_STATIC_SCENE);
        chunk.begin_chunk(CHUNKID_VARIABLES);
        chunk.begin_micro_chunk(VARID_MIN_DIM);
        write_vector3(&mut chunk, scene.min_extents);
        chunk.end_micro_chunk();
        chunk.begin_micro_chunk(VARID_MAX_DIM);
        write_vector3(&mut chunk, scene.max_extents);
        chunk.end_micro_chunk();
        chunk.end_chunk();

        chunk.begin_chunk(CHUNKID_STATIC_SOUNDS);
        for sound in &scene.static_sounds {
            let record = SavedSoundRecord::from_scene_sound(sound);
            write_sound_record(&mut chunk, &record);
        }
        chunk.end_chunk();
        chunk.end_chunk();

        let data = chunk.finish();
        writer.write_all(&data)?;
        writer.flush()?;
        Ok(())
    }

    pub fn load<R: Read + Seek>(&mut self, mut reader: R) -> Result<()> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        let mut reader = ChunkReader::new(&data);
        while let Some(chunk) = reader.next() {
            match chunk.id {
                CHUNKID_STATIC_SCENE => self.parse_static_scene(chunk.data()),
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_static_scene(&mut self, data: &[u8]) {
        let mut reader = ChunkReader::new(data);
        while let Some(chunk) = reader.next() {
            match chunk.id {
                CHUNKID_VARIABLES => self.parse_variables(chunk.data()),
                CHUNKID_STATIC_SOUNDS => self.parse_sounds(chunk.data()),
                _ => {}
            }
        }
    }

    fn parse_variables(&mut self, data: &[u8]) {
        let mut reader = MicroChunkReader::new(data);
        while let Some(micro) = reader.next() {
            match micro.id {
                VARID_MIN_DIM => self.min_extents = parse_vector3(micro.data()),
                VARID_MAX_DIM => self.max_extents = parse_vector3(micro.data()),
                _ => {}
            }
        }
    }

    fn parse_sounds(&mut self, data: &[u8]) {
        self.loaded_sounds.clear();
        let mut reader = ChunkReader::new(data);
        while let Some(chunk) = reader.next() {
            if let Some(record) = read_sound_record(chunk.data()) {
                self.loaded_sounds.push(record);
            }
        }
    }

    pub fn loaded_sounds(&self) -> &[SavedSoundRecord] {
        &self.loaded_sounds
    }

    pub fn min_extents(&self) -> Vector3 {
        self.min_extents
    }

    pub fn max_extents(&self) -> Vector3 {
        self.max_extents
    }
}

fn write_vector3(writer: &mut ChunkWriter, value: Vector3) {
    writer.write_f32(value.x);
    writer.write_f32(value.y);
    writer.write_f32(value.z);
}

fn parse_vector3(data: &[u8]) -> Vector3 {
    Vector3::new(
        f32::from_le_bytes(data[0..4].try_into().unwrap()),
        f32::from_le_bytes(data[4..8].try_into().unwrap()),
        f32::from_le_bytes(data[8..12].try_into().unwrap()),
    )
}

fn write_sound_record(writer: &mut ChunkWriter, record: &SavedSoundRecord) {
    use crate::sound_types::SoundClassId;
    let chunk_id = match record.class_id {
        SoundClassId::ThreeD => 0x0003_0003,
        SoundClassId::Pseudo3D => 0x0003_0004,
        _ => 0x0003_0001,
    };
    writer.begin_chunk(chunk_id);
    writer.begin_micro_chunk(0x01);
    writer.write_bytes(&record.id.to_le_bytes());
    writer.end_micro_chunk();
    writer.begin_micro_chunk(0x02);
    write_vector3(writer, record.position);
    writer.end_micro_chunk();
    writer.begin_micro_chunk(0x03);
    writer.write_f32(record.priority);
    writer.end_micro_chunk();
    writer.begin_micro_chunk(0x04);
    writer.write_f32(record.dropoff_radius);
    writer.end_micro_chunk();
    writer.end_chunk();
}

fn read_sound_record(data: &[u8]) -> Option<SavedSoundRecord> {
    let mut record = SavedSoundRecord::default();
    let mut reader = MicroChunkReader::new(data);
    while let Some(micro) = reader.next() {
        match micro.id {
            0x01 => {
                if micro.data().len() == 4 {
                    record.id = u32::from_le_bytes(micro.data().try_into().unwrap());
                }
            }
            0x02 => record.position = parse_vector3(micro.data()),
            0x03 => record.priority = f32::from_le_bytes(micro.data().try_into().unwrap()),
            0x04 => record.dropoff_radius = f32::from_le_bytes(micro.data().try_into().unwrap()),
            _ => {}
        }
    }
    Some(record)
}

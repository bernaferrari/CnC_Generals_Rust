// FILE: map_reader_writer_info.rs
// Ported from: GeneralsMD/Code/GameEngine/Include/Common/MapReaderWriterInfo.h
// Author: John Ahlquist, October 2001
//
// PARITY_NOTE: The C++ header defines version constants for each map chunk
// type (height map, blend tiles, objects, waypoints, triggers, lighting,
// etc.) and abstract I/O classes (OutputStream, InputStream,
// ChunkInputStream, CachedFileInputStream).  We port the version
// constants and I/O traits here.

pub const K_HEIGHT_MAP_VERSION_1: i32 = 1;
pub const K_HEIGHT_MAP_VERSION_2: i32 = 2;
pub const K_HEIGHT_MAP_VERSION_3: i32 = 3;
pub const K_HEIGHT_MAP_VERSION_4: i32 = 4;

pub const K_BLEND_TILE_VERSION_1: i32 = 1;
pub const K_BLEND_TILE_VERSION_2: i32 = 2;
pub const K_BLEND_TILE_VERSION_3: i32 = 3;
pub const K_BLEND_TILE_VERSION_4: i32 = 4;
pub const K_BLEND_TILE_VERSION_5: i32 = 5;
pub const K_BLEND_TILE_VERSION_6: i32 = 6;
pub const K_BLEND_TILE_VERSION_7: i32 = 7;
pub const K_BLEND_TILE_VERSION_8: i32 = 8;

pub const K_OBJECTS_VERSION_1: i32 = 1;
pub const K_OBJECTS_VERSION_2: i32 = 2;
pub const K_OBJECTS_VERSION_3: i32 = 3;
pub const K_MAP_OBJECT_VERSION_1: i32 = 1;
pub const K_WAYPOINTS_VERSION_1: i32 = 1;
pub const K_PLAYERLIST_VERSION_1: i32 = 1;
pub const K_TRIGGERS_VERSION_1: i32 = 1;
pub const K_TRIGGERS_VERSION_2: i32 = 2;
pub const K_TRIGGERS_VERSION_3: i32 = 3;
pub const K_TRIGGERS_VERSION_4: i32 = 4;
pub const K_LIGHTING_VERSION_1: i32 = 1;
pub const K_LIGHTING_VERSION_2: i32 = 2;
pub const K_LIGHTING_VERSION_3: i32 = 3;
pub const K_WORLDDICT_VERSION_1: i32 = 1;
pub const K_MAPPREVIEW_VERSION_1: i32 = 1;

pub trait OutputStream {
    fn write(&mut self, data: &[u8]) -> std::io::Result<i32>;
}

pub trait InputStream {
    fn read(&mut self, data: &mut [u8]) -> std::io::Result<i32>;
}

pub trait ChunkInputStream: InputStream {
    fn tell(&self) -> u32;
    fn absolute_seek(&mut self, pos: u32) -> bool;
    fn eof(&self) -> bool;
}

pub struct CachedFileInputStream {
    buffer: Vec<u8>,
    pos: usize,
}

impl CachedFileInputStream {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            pos: 0,
        }
    }

    pub fn open(&mut self, path: &str) -> bool {
        match std::fs::read(path) {
            Ok(data) => {
                self.buffer = data;
                self.pos = 0;
                true
            }
            Err(_) => false,
        }
    }

    pub fn close(&mut self) {
        self.buffer.clear();
        self.pos = 0;
    }

    pub fn rewind(&mut self) {
        self.pos = 0;
    }
}

impl Default for CachedFileInputStream {
    fn default() -> Self {
        Self::new()
    }
}

impl InputStream for CachedFileInputStream {
    fn read(&mut self, data: &mut [u8]) -> std::io::Result<i32> {
        let remaining = self.buffer.len().saturating_sub(self.pos);
        let to_read = data.len().min(remaining);
        data[..to_read].copy_from_slice(&self.buffer[self.pos..self.pos + to_read]);
        self.pos += to_read;
        Ok(to_read as i32)
    }
}

impl ChunkInputStream for CachedFileInputStream {
    fn tell(&self) -> u32 {
        self.pos as u32
    }

    fn absolute_seek(&mut self, pos: u32) -> bool {
        if (pos as usize) <= self.buffer.len() {
            self.pos = pos as usize;
            true
        } else {
            false
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.buffer.len()
    }
}

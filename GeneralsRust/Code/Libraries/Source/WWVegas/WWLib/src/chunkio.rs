//! Chunk-based file reader/writer (ported from WWLib chunkio.cpp/h).

use crate::iostruct::{IOQuaternionStruct, IOVector2Struct, IOVector3Struct, IOVector4Struct};
use crate::wwfile::{FileInterface, SeekDirection};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ChunkHeader {
    pub chunk_type: u32,
    pub chunk_size: u32,
}

impl ChunkHeader {
    pub fn new(chunk_type: u32, chunk_size: u32) -> Self {
        let mut header = ChunkHeader {
            chunk_type,
            chunk_size: 0,
        };
        header.set_size(chunk_size);
        header
    }

    pub fn set_type(&mut self, chunk_type: u32) {
        self.chunk_type = chunk_type;
    }

    pub fn get_type(&self) -> u32 {
        self.chunk_type
    }

    pub fn set_size(&mut self, size: u32) {
        self.chunk_size &= 0x8000_0000;
        self.chunk_size |= size & 0x7FFF_FFFF;
    }

    pub fn add_size(&mut self, add: u32) {
        let size = self.get_size();
        self.set_size(size + add);
    }

    pub fn get_size(&self) -> u32 {
        self.chunk_size & 0x7FFF_FFFF
    }

    pub fn set_sub_chunk_flag(&mut self, onoff: bool) {
        if onoff {
            self.chunk_size |= 0x8000_0000;
        } else {
            self.chunk_size &= 0x7FFF_FFFF;
        }
    }

    pub fn get_sub_chunk_flag(&self) -> bool {
        (self.chunk_size & 0x8000_0000) != 0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MicroChunkHeader {
    pub chunk_type: u8,
    pub chunk_size: u8,
}

impl MicroChunkHeader {
    pub fn new(chunk_type: u8, chunk_size: u8) -> Self {
        MicroChunkHeader {
            chunk_type,
            chunk_size,
        }
    }

    pub fn set_type(&mut self, chunk_type: u8) {
        self.chunk_type = chunk_type;
    }

    pub fn get_type(&self) -> u8 {
        self.chunk_type
    }

    pub fn set_size(&mut self, size: u8) {
        self.chunk_size = size;
    }

    pub fn add_size(&mut self, add: u8) {
        let size = self.get_size();
        self.set_size(size.wrapping_add(add));
    }

    pub fn get_size(&self) -> u8 {
        self.chunk_size
    }
}

pub struct ChunkSaveClass<'a> {
    file: &'a mut dyn FileInterface,
    stack_index: usize,
    position_stack: [i32; 256],
    header_stack: [ChunkHeader; 256],
    in_micro_chunk: bool,
    micro_chunk_position: i32,
    mc_header: MicroChunkHeader,
}

impl<'a> ChunkSaveClass<'a> {
    pub const MAX_STACK_DEPTH: usize = 256;

    pub fn new(file: &'a mut dyn FileInterface) -> Self {
        ChunkSaveClass {
            file,
            stack_index: 0,
            position_stack: [0; Self::MAX_STACK_DEPTH],
            header_stack: [ChunkHeader::default(); Self::MAX_STACK_DEPTH],
            in_micro_chunk: false,
            micro_chunk_position: 0,
            mc_header: MicroChunkHeader::default(),
        }
    }

    pub fn begin_chunk(&mut self, id: u32) -> bool {
        if self.stack_index > 0 {
            self.header_stack[self.stack_index - 1].set_sub_chunk_flag(true);
        }

        let mut chunkh = ChunkHeader::new(id, 0);
        let filepos = self.file.seek(0, SeekDirection::Current).unwrap_or(0) as i32;

        self.position_stack[self.stack_index] = filepos;
        self.header_stack[self.stack_index] = chunkh;
        self.stack_index += 1;

        let bytes = unsafe { as_bytes(&chunkh) };
        if self.file.write(bytes).unwrap_or(0) != bytes.len() {
            return false;
        }
        true
    }

    pub fn end_chunk(&mut self) -> bool {
        debug_assert!(!self.in_micro_chunk);
        let curpos = self.file.seek(0, SeekDirection::Current).unwrap_or(0) as i64;

        if self.stack_index == 0 {
            return false;
        }

        self.stack_index -= 1;
        let chunkpos = self.position_stack[self.stack_index];
        let chunkh = self.header_stack[self.stack_index];

        let _ = self.file.seek(chunkpos as i64, SeekDirection::Start);
        let bytes = unsafe { as_bytes(&chunkh) };
        if self.file.write(bytes).unwrap_or(0) != bytes.len() {
            return false;
        }

        if self.stack_index != 0 {
            let add = chunkh.get_size() + std::mem::size_of::<ChunkHeader>() as u32;
            self.header_stack[self.stack_index - 1].add_size(add);
        }

        let _ = self.file.seek(curpos, SeekDirection::Start);
        true
    }

    pub fn cur_chunk_depth(&self) -> i32 {
        self.stack_index as i32
    }

    pub fn begin_micro_chunk(&mut self, id: u32) -> bool {
        debug_assert!(id < 256);
        debug_assert!(!self.in_micro_chunk);

        self.mc_header.set_type(id as u8);
        self.mc_header.set_size(0);
        self.micro_chunk_position = self.file.seek(0, SeekDirection::Current).unwrap_or(0) as i32;

        let header = self.mc_header;
        let bytes = unsafe { as_bytes(&header) };
        if self.write(bytes) != bytes.len() as u32 {
            return false;
        }

        self.in_micro_chunk = true;
        true
    }

    pub fn end_micro_chunk(&mut self) -> bool {
        debug_assert!(self.in_micro_chunk);

        let curpos = self.file.seek(0, SeekDirection::Current).unwrap_or(0) as i64;
        let _ = self
            .file
            .seek(self.micro_chunk_position as i64, SeekDirection::Start);
        let bytes = unsafe { as_bytes(&self.mc_header) };
        if self.file.write(bytes).unwrap_or(0) != bytes.len() {
            return false;
        }

        let _ = self.file.seek(curpos, SeekDirection::Start);
        self.in_micro_chunk = false;
        true
    }

    pub fn write(&mut self, buf: &[u8]) -> u32 {
        debug_assert!(self.stack_index > 0);
        debug_assert!(!self.header_stack[self.stack_index - 1].get_sub_chunk_flag());

        let nbytes = buf.len() as u32;
        if self.file.write(buf).unwrap_or(0) != buf.len() {
            return 0;
        }

        self.header_stack[self.stack_index - 1].add_size(nbytes);

        if self.in_micro_chunk {
            debug_assert!(self.mc_header.get_size() < 255 - nbytes as u8);
            self.mc_header.add_size(nbytes as u8);
        }

        nbytes
    }

    pub fn write_vec2(&mut self, v: &IOVector2Struct) -> u32 {
        self.write(unsafe { as_bytes(v) })
    }

    pub fn write_vec3(&mut self, v: &IOVector3Struct) -> u32 {
        self.write(unsafe { as_bytes(v) })
    }

    pub fn write_vec4(&mut self, v: &IOVector4Struct) -> u32 {
        self.write(unsafe { as_bytes(v) })
    }

    pub fn write_quat(&mut self, q: &IOQuaternionStruct) -> u32 {
        self.write(unsafe { as_bytes(q) })
    }
}

pub struct ChunkLoadClass<'a> {
    file: &'a mut dyn FileInterface,
    stack_index: usize,
    position_stack: [u32; 256],
    header_stack: [ChunkHeader; 256],
    in_micro_chunk: bool,
    micro_chunk_position: u32,
    mc_header: MicroChunkHeader,
}

impl<'a> ChunkLoadClass<'a> {
    pub const MAX_STACK_DEPTH: usize = 256;

    pub fn new(file: &'a mut dyn FileInterface) -> Self {
        ChunkLoadClass {
            file,
            stack_index: 0,
            position_stack: [0; Self::MAX_STACK_DEPTH],
            header_stack: [ChunkHeader::default(); Self::MAX_STACK_DEPTH],
            in_micro_chunk: false,
            micro_chunk_position: 0,
            mc_header: MicroChunkHeader::default(),
        }
    }

    pub fn open_chunk(&mut self) -> bool {
        debug_assert!(!self.in_micro_chunk);
        debug_assert!(self.stack_index < Self::MAX_STACK_DEPTH - 1);

        if self.stack_index > 0 {
            let parent_size = self.header_stack[self.stack_index - 1].get_size();
            if self.position_stack[self.stack_index - 1] == parent_size {
                return false;
            }
        }

        let mut header = ChunkHeader::default();
        let bytes = unsafe { as_bytes_mut(&mut header) };
        if self.file.read(bytes).unwrap_or(0) != bytes.len() {
            return false;
        }

        self.header_stack[self.stack_index] = header;
        self.position_stack[self.stack_index] = 0;
        self.stack_index += 1;
        true
    }

    pub fn close_chunk(&mut self) -> bool {
        debug_assert!(!self.in_micro_chunk);
        debug_assert!(self.stack_index > 0);

        let csize = self.header_stack[self.stack_index - 1].get_size();
        let pos = self.position_stack[self.stack_index - 1];
        if pos < csize {
            let _ = self.file.seek((csize - pos) as i64, SeekDirection::Current);
        }

        self.stack_index -= 1;
        if self.stack_index > 0 {
            let add = csize + std::mem::size_of::<ChunkHeader>() as u32;
            self.position_stack[self.stack_index - 1] += add;
        }

        true
    }

    pub fn cur_chunk_id(&self) -> u32 {
        debug_assert!(self.stack_index >= 1);
        self.header_stack[self.stack_index - 1].get_type()
    }

    pub fn cur_chunk_length(&self) -> u32 {
        debug_assert!(self.stack_index >= 1);
        self.header_stack[self.stack_index - 1].get_size()
    }

    pub fn cur_chunk_depth(&self) -> i32 {
        self.stack_index as i32
    }

    pub fn contains_chunks(&self) -> i32 {
        if self.stack_index == 0 {
            return 0;
        }
        if self.header_stack[self.stack_index - 1].get_sub_chunk_flag() {
            1
        } else {
            0
        }
    }

    pub fn open_micro_chunk(&mut self) -> bool {
        debug_assert!(!self.in_micro_chunk);

        let mut header = MicroChunkHeader::default();
        if self.read(unsafe { as_bytes_mut(&mut header) })
            != std::mem::size_of::<MicroChunkHeader>() as u32
        {
            return false;
        }

        self.mc_header = header;
        self.in_micro_chunk = true;
        self.micro_chunk_position = 0;
        true
    }

    pub fn close_micro_chunk(&mut self) -> bool {
        debug_assert!(self.in_micro_chunk);
        self.in_micro_chunk = false;

        let csize = self.mc_header.get_size() as u32;
        let pos = self.micro_chunk_position;
        if pos < csize {
            let _ = self.file.seek((csize - pos) as i64, SeekDirection::Current);
            if self.stack_index > 0 {
                self.position_stack[self.stack_index - 1] += csize - pos;
            }
        }

        true
    }

    pub fn cur_micro_chunk_id(&self) -> u32 {
        debug_assert!(self.in_micro_chunk);
        self.mc_header.get_type() as u32
    }

    pub fn cur_micro_chunk_length(&self) -> u32 {
        debug_assert!(self.in_micro_chunk);
        self.mc_header.get_size() as u32
    }

    pub fn seek(&mut self, nbytes: u32) -> u32 {
        debug_assert!(self.stack_index >= 1);

        if self.position_stack[self.stack_index - 1] + nbytes
            > self.header_stack[self.stack_index - 1].get_size()
        {
            return 0;
        }

        if self.in_micro_chunk
            && self.micro_chunk_position + nbytes > self.mc_header.get_size() as u32
        {
            return 0;
        }

        let curpos = self.file.tell().unwrap_or(0);
        let newpos = self
            .file
            .seek(nbytes as i64, SeekDirection::Current)
            .unwrap_or(curpos);
        if (newpos as i64 - curpos as i64) != nbytes as i64 {
            return 0;
        }

        self.position_stack[self.stack_index - 1] += nbytes;
        if self.in_micro_chunk {
            self.micro_chunk_position += nbytes;
        }

        nbytes
    }

    pub fn read(&mut self, buf: &mut [u8]) -> u32 {
        let nbytes = buf.len() as u32;
        debug_assert!(self.stack_index >= 1);

        if self.position_stack[self.stack_index - 1] + nbytes
            > self.header_stack[self.stack_index - 1].get_size()
        {
            return 0;
        }

        if self.in_micro_chunk
            && self.micro_chunk_position + nbytes > self.mc_header.get_size() as u32
        {
            return 0;
        }

        if self.file.read(buf).unwrap_or(0) != buf.len() {
            return 0;
        }

        self.position_stack[self.stack_index - 1] += nbytes;
        if self.in_micro_chunk {
            self.micro_chunk_position += nbytes;
        }

        nbytes
    }

    pub fn read_vec2(&mut self, v: &mut IOVector2Struct) -> u32 {
        self.read(unsafe { as_bytes_mut(v) })
    }

    pub fn read_vec3(&mut self, v: &mut IOVector3Struct) -> u32 {
        self.read(unsafe { as_bytes_mut(v) })
    }

    pub fn read_vec4(&mut self, v: &mut IOVector4Struct) -> u32 {
        self.read(unsafe { as_bytes_mut(v) })
    }

    pub fn read_quat(&mut self, q: &mut IOQuaternionStruct) -> u32 {
        self.read(unsafe { as_bytes_mut(q) })
    }
}

unsafe fn as_bytes<T: Copy>(value: &T) -> &[u8] {
    std::slice::from_raw_parts((value as *const T) as *const u8, std::mem::size_of::<T>())
}

unsafe fn as_bytes_mut<T: Copy>(value: &mut T) -> &mut [u8] {
    std::slice::from_raw_parts_mut((value as *mut T) as *mut u8, std::mem::size_of::<T>())
}

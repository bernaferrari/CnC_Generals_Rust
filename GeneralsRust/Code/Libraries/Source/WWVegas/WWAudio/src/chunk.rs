use std::io::{self, Read};

#[derive(Default)]
pub struct ChunkWriter {
    buf: Vec<u8>,
    stack: Vec<ChunkEntry>,
    micro: Option<MicroEntry>,
}

#[derive(Clone, Copy)]
struct ChunkEntry {
    start: usize,
    id: u32,
    has_children: bool,
}

#[derive(Clone, Copy)]
struct MicroEntry {
    start: usize,
    id: u8,
}

impl ChunkWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_chunk(&mut self, id: u32) {
        if let Some(entry) = self.stack.last_mut() {
            entry.has_children = true;
        }
        let start = self.buf.len();
        self.buf.extend_from_slice(&[0; 8]);
        self.stack.push(ChunkEntry {
            start,
            id,
            has_children: false,
        });
    }

    pub fn end_chunk(&mut self) {
        let entry = self.stack.pop().expect("end_chunk without begin_chunk");
        let end = self.buf.len();
        let size = (end - entry.start - 8) as u32;
        let mut size_field = size;
        if entry.has_children {
            size_field |= 0x8000_0000;
        }
        self.buf[entry.start..entry.start + 4].copy_from_slice(&entry.id.to_le_bytes());
        self.buf[entry.start + 4..entry.start + 8].copy_from_slice(&size_field.to_le_bytes());
    }

    pub fn begin_micro_chunk(&mut self, id: u8) {
        assert!(self.micro.is_none(), "nested micro-chunks not supported");
        let start = self.buf.len();
        self.buf.push(id);
        self.buf.push(0); // size placeholder
        self.micro = Some(MicroEntry { start, id });
    }

    pub fn end_micro_chunk(&mut self) {
        let entry = self.micro.take().expect("end_micro_chunk without begin");
        let end = self.buf.len();
        let size = (end - entry.start - 2) as u8;
        self.buf[entry.start + 1] = size;
    }

    pub fn write_u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub fn write_u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn finish(mut self) -> Vec<u8> {
        assert!(self.stack.is_empty(), "Unclosed chunks");
        assert!(self.micro.is_none(), "Unclosed micro chunk");
        self.buf.shrink_to_fit();
        self.buf
    }
}

pub struct ChunkReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ChunkReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub fn next(&mut self) -> Option<Chunk<'a>> {
        if self.offset + 8 > self.data.len() {
            return None;
        }
        let id = u32::from_le_bytes(self.data[self.offset..self.offset + 4].try_into().unwrap());
        let size_field = u32::from_le_bytes(
            self.data[self.offset + 4..self.offset + 8]
                .try_into()
                .unwrap(),
        );
        let has_children = (size_field & 0x8000_0000) != 0;
        let size = (size_field & 0x7FFF_FFFF) as usize;
        let start = self.offset + 8;
        let end = start + size;
        if end > self.data.len() {
            return None;
        }
        self.offset = end;
        Some(Chunk {
            id,
            data: &self.data[start..end],
            has_children,
        })
    }
}

pub struct Chunk<'a> {
    pub id: u32,
    data: &'a [u8],
    has_children: bool,
}

impl<'a> Chunk<'a> {
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn sub_chunks(&self) -> ChunkReader<'a> {
        ChunkReader::new(self.data)
    }

    pub fn micro_chunks(&self) -> MicroChunkReader<'a> {
        MicroChunkReader::new(self.data)
    }
}

pub struct MicroChunkReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> MicroChunkReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub fn next(&mut self) -> Option<MicroChunk<'a>> {
        if self.offset + 2 > self.data.len() {
            return None;
        }
        let id = self.data[self.offset];
        let size = self.data[self.offset + 1] as usize;
        let start = self.offset + 2;
        let end = start + size;
        if end > self.data.len() {
            return None;
        }
        self.offset = end;
        Some(MicroChunk {
            id,
            data: &self.data[start..end],
        })
    }
}

pub struct MicroChunk<'a> {
    pub id: u8,
    data: &'a [u8],
}

impl<'a> MicroChunk<'a> {
    pub fn as_u32(&self) -> Option<u32> {
        if self.data.len() == 4 {
            Some(u32::from_le_bytes(self.data.try_into().unwrap()))
        } else {
            None
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        if self.data.len() == 4 {
            Some(f32::from_le_bytes(self.data.try_into().unwrap()))
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

pub fn read_all(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

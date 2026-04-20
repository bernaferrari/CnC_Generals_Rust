// data_chunk_io.rs - DataChunkInput/Output binary chunk parser.

use crate::common::dict::{Dict, DictType};
use crate::common::name_key_generator::NameKeyGenerator;

use std::collections::HashMap;

pub type DataChunkVersionType = u16;

const CHUNK_HEADER_BYTES: usize = 10;
const CHUNK_TAG: [u8; 4] = [b'C', b'k', b'M', b'p'];

#[derive(Debug, Clone)]
pub struct DataChunkInfo {
    pub label: String,
    pub parent_label: String,
    pub version: DataChunkVersionType,
    pub data_size: u32,
}

#[derive(Debug, Clone)]
struct Mapping {
    _id: u32,
    _name: String,
}

#[derive(Debug, Default, Clone)]
struct DataChunkTableOfContents {
    list: Vec<Mapping>,
    by_id: HashMap<u32, String>,
    opened: bool,
}

impl DataChunkTableOfContents {
    fn read(&mut self, stream: &mut ChunkInputStream) {
        let mut tag = [0u8; 4];
        if !stream.read_exact(&mut tag) {
            self.opened = false;
            return;
        }
        if tag != CHUNK_TAG {
            self.opened = false;
            return;
        }

        let count = stream.read_i32().unwrap_or(0);
        if count <= 0 {
            self.opened = false;
            return;
        }

        self.list.clear();
        self.by_id.clear();

        let mut max_id = 0u32;
        for _ in 0..count {
            let len = stream.read_u8().unwrap_or(0) as usize;
            let mut buf = vec![0u8; len];
            if len > 0 {
                if !stream.read_exact(&mut buf) {
                    break;
                }
            }
            let id = stream.read_u32().unwrap_or(0);
            let name = String::from_utf8_lossy(&buf).to_string();
            self.list.push(Mapping {
                _id: id,
                _name: name.clone(),
            });
            self.by_id.insert(id, name);
            max_id = max_id.max(id);
        }

        self.opened = max_id > 0;
    }

    fn get_name(&self, id: u32) -> String {
        self.by_id.get(&id).cloned().unwrap_or_default()
    }

    fn is_opened(&self) -> bool {
        self.opened
    }
}

#[derive(Debug, Default, Clone)]
pub struct ChunkInputStream {
    data: Vec<u8>,
    pos: usize,
}

impl ChunkInputStream {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }

    pub fn read_exact(&mut self, buf: &mut [u8]) -> bool {
        if self.pos + buf.len() > self.data.len() {
            return false;
        }
        buf.copy_from_slice(&self.data[self.pos..self.pos + buf.len()]);
        self.pos += buf.len();
        true
    }

    pub fn tell(&self) -> usize {
        self.pos
    }

    pub fn absolute_seek(&mut self, pos: usize) {
        self.pos = pos.min(self.data.len());
    }

    pub fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_u8(&mut self) -> Option<u8> {
        let mut buf = [0u8; 1];
        if self.read_exact(&mut buf) {
            Some(buf[0])
        } else {
            None
        }
    }

    fn read_u16(&mut self) -> Option<u16> {
        let mut buf = [0u8; 2];
        if self.read_exact(&mut buf) {
            Some(u16::from_le_bytes(buf))
        } else {
            None
        }
    }

    fn read_u32(&mut self) -> Option<u32> {
        let mut buf = [0u8; 4];
        if self.read_exact(&mut buf) {
            Some(u32::from_le_bytes(buf))
        } else {
            None
        }
    }

    fn read_i32(&mut self) -> Option<i32> {
        let mut buf = [0u8; 4];
        if self.read_exact(&mut buf) {
            Some(i32::from_le_bytes(buf))
        } else {
            None
        }
    }

    fn read_f32(&mut self) -> Option<f32> {
        let mut buf = [0u8; 4];
        if self.read_exact(&mut buf) {
            Some(f32::from_le_bytes(buf))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
struct InputChunk {
    id: u32,
    version: DataChunkVersionType,
    data_size: i32,
    data_left: i32,
    _chunk_start: usize,
}

type ParserFn = fn(&mut DataChunkInput, &DataChunkInfo, &mut dyn std::any::Any) -> bool;

#[derive(Debug, Clone)]
struct ParserEntry {
    label: String,
    parent_label: String,
    parser: ParserFn,
}

#[derive(Debug)]
pub struct DataChunkInput {
    stream: ChunkInputStream,
    contents: DataChunkTableOfContents,
    chunk_stack: Vec<InputChunk>,
    parser_list: Vec<ParserEntry>,
    _filepos_first_chunk: usize,
}

impl DataChunkInput {
    pub fn new(data: Vec<u8>) -> Self {
        let mut stream = ChunkInputStream::new(data);
        let mut contents = DataChunkTableOfContents::default();
        contents.read(&mut stream);
        let _filepos_first_chunk = stream.tell();
        Self {
            stream,
            contents,
            chunk_stack: Vec::new(),
            parser_list: Vec::new(),
            _filepos_first_chunk,
        }
    }

    pub fn is_valid_file_type(&self) -> bool {
        self.contents.is_opened()
    }

    pub fn register_parser(&mut self, label: &str, parent_label: &str, parser: ParserFn) {
        self.parser_list.push(ParserEntry {
            label: label.to_string(),
            parent_label: parent_label.to_string(),
            parser,
        });
    }

    pub fn parse(&mut self, user_data: &mut dyn std::any::Any) -> bool {
        if !self.contents.is_opened() {
            return false;
        }

        while !self.stream.eof() {
            if let Some(chunk) = self.chunk_stack.last() {
                if chunk.data_left < CHUNK_HEADER_BYTES as i32 {
                    break;
                }
            }

            let parent_label = self
                .chunk_stack
                .last()
                .map(|c| self.contents.get_name(c.id))
                .unwrap_or_default();

            let version = match self.open_data_chunk() {
                Some(ver) => ver,
                None => break,
            };

            let label = self
                .chunk_stack
                .last()
                .map(|c| self.contents.get_name(c.id))
                .unwrap_or_default();

            let info = DataChunkInfo {
                label: label.clone(),
                parent_label: parent_label.clone(),
                version,
                data_size: self.get_chunk_data_size(),
            };

            for parser in self.parser_list.clone() {
                if parser.label == label && parser.parent_label == parent_label {
                    let ok = (parser.parser)(self, &info, user_data);
                    if !ok {
                        return false;
                    }
                    break;
                }
            }

            self.close_data_chunk();
        }

        true
    }

    pub fn at_end_of_chunk(&self) -> bool {
        self.chunk_stack
            .last()
            .map(|c| c.data_left <= 0)
            .unwrap_or(true)
    }

    pub fn get_chunk_version(&self) -> DataChunkVersionType {
        self.chunk_stack.last().map(|c| c.version).unwrap_or(0)
    }

    pub fn get_chunk_data_size(&self) -> u32 {
        self.chunk_stack
            .last()
            .map(|c| c.data_size as u32)
            .unwrap_or(0)
    }

    fn decrement_data_left(&mut self, size: i32) {
        for chunk in &mut self.chunk_stack {
            chunk.data_left -= size;
        }
    }

    fn open_data_chunk(&mut self) -> Option<DataChunkVersionType> {
        let id = self.stream.read_u32()?;
        self.decrement_data_left(4);
        let version = self.stream.read_u16()?;
        self.decrement_data_left(2);
        let data_size = self.stream.read_i32()?;
        self.decrement_data_left(4);

        let chunk = InputChunk {
            id,
            version,
            data_size,
            data_left: data_size,
            _chunk_start: self.stream.tell(),
        };
        self.chunk_stack.push(chunk);
        Some(version)
    }

    fn close_data_chunk(&mut self) {
        if let Some(chunk) = self.chunk_stack.pop() {
            if chunk.data_left > 0 {
                let new_pos = self.stream.tell().saturating_add(chunk.data_left as usize);
                self.stream.absolute_seek(new_pos);
                self.decrement_data_left(chunk.data_left);
            }
        }
    }

    pub fn read_real(&mut self) -> f32 {
        let value = self.stream.read_f32().unwrap_or(0.0);
        self.decrement_data_left(4);
        value
    }

    pub fn read_int(&mut self) -> i32 {
        let value = self.stream.read_i32().unwrap_or(0);
        self.decrement_data_left(4);
        value
    }

    pub fn read_byte(&mut self) -> u8 {
        let value = self.stream.read_u8().unwrap_or(0);
        self.decrement_data_left(1);
        value
    }

    pub fn read_ascii_string(&mut self) -> String {
        let len = self.stream.read_u16().unwrap_or(0) as usize;
        self.decrement_data_left(2);
        let mut buf = vec![0u8; len];
        if len > 0 {
            if self.stream.read_exact(&mut buf) {
                self.decrement_data_left(len as i32);
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    }

    pub fn read_unicode_string(&mut self) -> String {
        let len = self.stream.read_u16().unwrap_or(0) as usize;
        self.decrement_data_left(2);
        if len == 0 {
            return String::new();
        }
        let mut buf = vec![0u8; len * 2];
        if self.stream.read_exact(&mut buf) {
            self.decrement_data_left((len * 2) as i32);
        }
        let mut utf16 = Vec::with_capacity(len);
        for chunk in buf.chunks_exact(2) {
            utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        String::from_utf16_lossy(&utf16)
    }

    pub fn read_name_key(&mut self) -> u32 {
        let key_and_type = self.read_int() as u32;
        let name_id = key_and_type >> 8;
        let name = self.contents.get_name(name_id);
        NameKeyGenerator::name_to_key(&name)
    }

    pub fn read_dict(&mut self) -> Dict {
        let len = self.stream.read_u16().unwrap_or(0) as usize;
        self.decrement_data_left(2);
        let mut dict = Dict::new();
        for _ in 0..len {
            let key_and_type = self.read_int();
            let data_type = (key_and_type & 0xff) as i32;
            let name_id = (key_and_type as u32) >> 8;
            let name = self.contents.get_name(name_id);
            let key = NameKeyGenerator::name_to_key(&name);

            match data_type {
                0 => dict.set_bool(key, self.read_byte() != 0),
                1 => dict.set_int(key, self.read_int()),
                2 => dict.set_real(key, self.read_real()),
                3 => dict.set_ascii_string(key, self.read_ascii_string()),
                4 => dict.set_unicode_string(key, self.read_unicode_string()),
                _ => dict.set_ascii_string(key, String::new()),
            }
        }
        dict
    }
}

#[derive(Debug, Default)]
pub struct DataChunkOutput {
    contents: HashMap<String, u32>,
    next_id: u32,
    buffer: Vec<u8>,
    chunk_stack: Vec<usize>,
}

impl DataChunkOutput {
    pub fn new() -> Self {
        Self {
            contents: HashMap::new(),
            next_id: 1,
            buffer: Vec::new(),
            chunk_stack: Vec::new(),
        }
    }

    pub fn open_data_chunk(&mut self, name: &str, version: DataChunkVersionType) {
        let id = *self.contents.entry(name.to_string()).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        });
        self.buffer.extend_from_slice(&id.to_le_bytes());
        self.buffer.extend_from_slice(&version.to_le_bytes());
        let size_pos = self.buffer.len();
        self.buffer.extend_from_slice(&0u32.to_le_bytes());
        self.chunk_stack.push(size_pos);
    }

    pub fn close_data_chunk(&mut self) {
        if let Some(size_pos) = self.chunk_stack.pop() {
            let here = self.buffer.len();
            let size = (here - size_pos - 4) as u32;
            self.buffer[size_pos..size_pos + 4].copy_from_slice(&size.to_le_bytes());
        }
    }

    pub fn write_real(&mut self, value: f32) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_int(&mut self, value: i32) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_byte(&mut self, value: u8) {
        self.buffer.push(value);
    }

    pub fn write_ascii_string(&mut self, value: &str) {
        let len = value.len() as u16;
        self.buffer.extend_from_slice(&len.to_le_bytes());
        self.buffer.extend_from_slice(value.as_bytes());
    }

    pub fn write_unicode_string(&mut self, value: &str) {
        let utf16: Vec<u16> = value.encode_utf16().collect();
        let len = utf16.len() as u16;
        self.buffer.extend_from_slice(&len.to_le_bytes());
        for unit in utf16 {
            self.buffer.extend_from_slice(&unit.to_le_bytes());
        }
    }

    pub fn write_name_key(&mut self, key: u32) {
        let name = NameKeyGenerator::key_to_name(key).unwrap_or_default();
        let id = *self.contents.entry(name).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        });
        let mut key_and_type = (id << 8) as u32;
        key_and_type |= DictType::AsciiString as u32;
        self.write_int(key_and_type as i32);
    }

    pub fn write_dict(&mut self, dict: &Dict) {
        let len = dict.get_pair_count() as u16;
        self.buffer.extend_from_slice(&len.to_le_bytes());
        for idx in 0..dict.get_pair_count() {
            let Some(key) = dict.get_nth_key(idx) else {
                continue;
            };
            let name = NameKeyGenerator::key_to_name(key).unwrap_or_default();
            let id = *self.contents.entry(name).or_insert_with(|| {
                let id = self.next_id;
                self.next_id += 1;
                id
            });
            let dtype = dict.get_nth_type(idx).unwrap_or(DictType::AsciiString) as u32;
            let key_and_type = ((id << 8) | dtype) as u32;
            self.write_int(key_and_type as i32);

            match dict.get_nth_type(idx) {
                Some(DictType::Bool) => self.write_byte(dict.get_nth_bool(idx) as u8),
                Some(DictType::Int) => self.write_int(dict.get_nth_int(idx)),
                Some(DictType::Real) => self.write_real(dict.get_nth_real(idx)),
                Some(DictType::AsciiString) => {
                    self.write_ascii_string(&dict.get_nth_ascii_string(idx))
                }
                Some(DictType::UnicodeString) => {
                    self.write_unicode_string(&dict.get_nth_unicode_string(idx))
                }
                None => self.write_byte(0),
            }
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }
}

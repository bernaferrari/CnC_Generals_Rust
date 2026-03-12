//! IFF helpers and compression utilities (ported from WWLib iff.h/load.cpp).

use crate::buff::Buffer;
use crate::lcw;
use crate::wwfile::{FileInterface, FileRights, SeekDirection, WWFile};
use std::sync::{Mutex, OnceLock};

pub const LZW_SUPPORTED: bool = false;

pub fn make_id(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (d as u32) << 24 | (c as u32) << 16 | (b as u32) << 8 | (a as u32)
}

#[repr(i32)]
#[derive(Clone, Copy, Debug)]
pub enum PicturePlaneType {
    BmAmiga = 0,
    BmMcga = 1,
}

impl PicturePlaneType {
    pub const BmDefault: Self = Self::BmMcga;
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum CompressionType {
    NoCompress = 0,
    Lzw12 = 1,
    Lzw14 = 2,
    Horizontal = 3,
    Lcw = 4,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CompHeaderType {
    pub method: u8,
    pub pad: u8,
    pub size: u32,
    pub skip: u16,
}

fn iff_files() -> &'static Mutex<Vec<Option<WWFile>>> {
    static FILES: OnceLock<Mutex<Vec<Option<WWFile>>>> = OnceLock::new();
    FILES.get_or_init(|| Mutex::new(Vec::new()))
}

fn uncomp_buffer() -> &'static Mutex<Option<Vec<u8>>> {
    static BUFFER: OnceLock<Mutex<Option<Vec<u8>>>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(None))
}

#[allow(non_snake_case)]
pub fn Open_Iff_File(filename: &str) -> i32 {
    let mut file = WWFile::with_path(filename);
    if file.open(FileRights::ReadWrite).is_err() {
        if file.open(FileRights::Read).is_err() {
            return -1;
        }
    }

    let mut files = iff_files().lock().unwrap();
    for (idx, slot) in files.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(file);
            return idx as i32;
        }
    }
    files.push(Some(file));
    (files.len() - 1) as i32
}

#[allow(non_snake_case)]
pub fn Close_Iff_File(handle: i32) {
    if handle < 0 {
        return;
    }
    let mut files = iff_files().lock().unwrap();
    if let Some(slot) = files.get_mut(handle as usize) {
        if let Some(mut file) = slot.take() {
            let _ = file.close();
        }
    }
}

#[allow(non_snake_case)]
pub fn Get_Iff_Chunk_Size(handle: i32, id: u32) -> u32 {
    let mut files = iff_files().lock().unwrap();
    let Some(file) = files
        .get_mut(handle as usize)
        .and_then(|slot| slot.as_mut())
    else {
        return 0;
    };
    find_chunk(file, id).map(|(_, size)| size).unwrap_or(0)
}

#[allow(non_snake_case)]
pub fn Read_Iff_Chunk(handle: i32, id: u32, buffer: &mut [u8]) -> u32 {
    let mut files = iff_files().lock().unwrap();
    let Some(file) = files
        .get_mut(handle as usize)
        .and_then(|slot| slot.as_mut())
    else {
        return 0;
    };

    let Some((pos, size)) = find_chunk(file, id) else {
        return 0;
    };

    if file.seek(pos as i64, SeekDirection::Start).is_err() {
        return 0;
    }

    let to_read = size.min(buffer.len() as u32);
    let mut total = 0u32;
    while total < to_read {
        let slice = &mut buffer[total as usize..to_read as usize];
        let read = match file.read(slice) {
            Ok(count) => count,
            Err(_) => 0,
        };
        if read == 0 {
            break;
        }
        total += read as u32;
    }

    total
}

#[allow(non_snake_case)]
pub fn Write_Iff_Chunk(handle: i32, id: u32, buffer: &[u8]) {
    let mut files = iff_files().lock().unwrap();
    let Some(file) = files
        .get_mut(handle as usize)
        .and_then(|slot| slot.as_mut())
    else {
        return;
    };

    let size = buffer.len() as u32;
    let header = [id.to_le_bytes(), size.to_le_bytes()].concat();
    let _ = file.write(&header);
    let _ = file.write(buffer);
    if size % 2 == 1 {
        let _ = file.write(&[0]);
    }
}

#[allow(non_snake_case)]
pub fn Load_Data(name: &str, ptr: &mut [u8]) -> u32 {
    let mut file = WWFile::with_path(name);
    if file.open(FileRights::Read).is_err() {
        return 0;
    }
    let mut total = 0u32;
    while total < ptr.len() as u32 {
        let read = match file.read(&mut ptr[total as usize..]) {
            Ok(count) => count,
            Err(_) => 0,
        };
        if read == 0 {
            break;
        }
        total += read as u32;
    }
    let _ = file.close();
    total
}

#[allow(non_snake_case)]
pub fn Write_Data(name: &str, ptr: &[u8]) -> u32 {
    let mut file = WWFile::with_path(name);
    if file.create().is_err() {
        return 0;
    }
    let mut total = 0u32;
    while total < ptr.len() as u32 {
        let written = match file.write(&ptr[total as usize..]) {
            Ok(count) => count,
            Err(_) => 0,
        };
        if written == 0 {
            break;
        }
        total += written as u32;
    }
    let _ = file.close();
    total
}

#[allow(non_snake_case)]
pub fn Load_Uncompress(
    filename: &str,
    uncomp_buff: &mut Buffer,
    dest_buff: &mut Buffer,
    reserved_data: Option<&mut [u8]>,
) -> u32 {
    let mut file = WWFile::with_path(filename);
    if file.open(FileRights::Read).is_err() {
        return 0;
    }

    let mut size_buf = [0u8; 2];
    if file.read(&mut size_buf).is_err() {
        let _ = file.close();
        return 0;
    }
    let mut size = u16::from_le_bytes(size_buf) as usize;

    let mut header_buf = [0u8; 8];
    if file.read(&mut header_buf).is_err() {
        let _ = file.close();
        return 0;
    }
    size = size.saturating_sub(header_buf.len());

    let header = CompHeaderType {
        method: header_buf[0],
        pad: header_buf[1],
        size: u32::from_le_bytes([header_buf[2], header_buf[3], header_buf[4], header_buf[5]]),
        skip: u16::from_le_bytes([header_buf[6], header_buf[7]]),
    };

    let mut remaining = size;
    if header.skip > 0 {
        remaining = remaining.saturating_sub(header.skip as usize);
        if let Some(reserved) = reserved_data {
            let to_read = header.skip as usize;
            let mut offset = 0usize;
            while offset < to_read {
                let read = match file.read(&mut reserved[offset..to_read]) {
                    Ok(count) => count,
                    Err(_) => 0,
                };
                if read == 0 {
                    break;
                }
                offset += read;
            }
        } else {
            let _ = file.seek(header.skip as i64, SeekDirection::Current);
        }
    }

    let mut payload = vec![0u8; header_buf.len() + remaining];
    payload[..header_buf.len()].copy_from_slice(&header_buf);
    let mut offset = header_buf.len();
    while offset < payload.len() {
        let read = match file.read(&mut payload[offset..]) {
            Ok(count) => count,
            Err(_) => 0,
        };
        if read == 0 {
            break;
        }
        offset += read;
    }

    if let Some(uncomp_slice) = uncomp_buff.as_mut_slice() {
        let copy_len = payload.len().min(uncomp_slice.len());
        uncomp_slice[..copy_len].copy_from_slice(&payload[..copy_len]);
    }

    if let Some(dest_slice) = dest_buff.as_mut_slice() {
        let size = Uncompress_Data(&payload, dest_slice);
        let _ = file.close();
        return size;
    }

    let _ = file.close();
    0
}

#[allow(non_snake_case)]
pub fn Uncompress_Data(src: &[u8], dst: &mut [u8]) -> u32 {
    if src.len() < 8 {
        return 0;
    }

    let header = CompHeaderType {
        method: src[0],
        pad: src[1],
        size: u32::from_le_bytes([src[2], src[3], src[4], src[5]]),
        skip: u16::from_le_bytes([src[6], src[7]]),
    };

    let mut offset = 8usize + header.skip as usize;
    if offset > src.len() {
        return 0;
    }

    let comp_data = &src[offset..];
    match header.method {
        x if x == CompressionType::NoCompress as u8 => {
            let to_copy = header.size.min(dst.len() as u32) as usize;
            dst[..to_copy].copy_from_slice(&comp_data[..to_copy]);
            header.size
        }
        x if x == CompressionType::Lcw as u8 => {
            let mut decoded = Vec::new();
            if lcw::decompress_to_vec(comp_data, &mut decoded).is_ok() {
                let to_copy = decoded.len().min(dst.len());
                dst[..to_copy].copy_from_slice(&decoded[..to_copy]);
                header.size
            } else {
                0
            }
        }
        _ => header.size,
    }
}

#[allow(non_snake_case)]
pub fn Set_Uncomp_Buffer(_buffer_segment: i32, _size_of_buffer: i32) {
    if _size_of_buffer <= 0 {
        *uncomp_buffer().lock().unwrap() = None;
        return;
    }
    let mut buf = vec![0u8; _size_of_buffer as usize];
    buf.fill(0);
    *uncomp_buffer().lock().unwrap() = Some(buf);
}

fn find_chunk(file: &mut WWFile, id: u32) -> Option<(u64, u32)> {
    let size = file.size().ok()? as u64;
    let mut pos = 0u64;
    let mut header = [0u8; 8];
    let _ = file.seek(0, SeekDirection::Start);

    while pos + 8 <= size {
        if file.read(&mut header).ok()? != 8 {
            break;
        }
        let chunk_id = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let chunk_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if chunk_id == id {
            return Some((pos + 8, chunk_size));
        }
        let advance = 8u64 + chunk_size as u64 + (chunk_size as u64 & 1);
        pos = pos.saturating_add(advance);
        let _ = file.seek(pos as i64, SeekDirection::Start);
    }

    None
}

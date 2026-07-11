use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::{calc_max_compressed_size_raw, compress_raw, decompress_raw, CompressionLevel};

pub type Bool = bool;
pub type Int = i32;
pub type UnsignedInt = u32;

pub const MAP_EXTENSION: &str = ".map";
pub const LZH_EXTENSION: &str = ".nxz";
pub const RUL_EXTENSION: &str = ".rul";

/// Matches C++ function: DecompressFile(char *infile, char *outfile)
pub fn decompress_file<P: AsRef<Path>>(infile: P, outfile: P) -> Bool {
    let mut input_file = match File::open(infile.as_ref()) {
        Ok(file) => file,
        Err(_) => return false,
    };

    let mut header = [0u8; 4];
    if input_file.read_exact(&mut header).is_err() {
        return false;
    }
    let raw_size = UnsignedInt::from_le_bytes(header) as usize;

    let mut compressed = Vec::new();
    if input_file.read_to_end(&mut compressed).is_err() {
        return false;
    }

    let decompressed = match decompress_raw(&compressed, raw_size) {
        Ok(data) => data,
        Err(_) => return false,
    };

    let mut output_file = match File::create(outfile.as_ref()) {
        Ok(file) => file,
        Err(_) => return false,
    };

    output_file.write_all(&decompressed).is_ok()
}

/// Matches C++ function: CompressFile(char *infile, char *outfile)
pub fn compress_file<P: AsRef<Path>>(infile: P, outfile: P) -> Bool {
    let input_data = match std::fs::read(infile.as_ref()) {
        Ok(data) => data,
        Err(_) => return false,
    };

    let compressed = match compress_raw(&input_data, CompressionLevel::Default) {
        Ok(data) => data,
        Err(_) => return false,
    };

    let mut output_file = match File::create(outfile.as_ref()) {
        Ok(file) => file,
        Err(_) => return false,
    };

    if output_file
        .write_all(&(input_data.len() as UnsignedInt).to_le_bytes())
        .is_err()
    {
        return false;
    }
    output_file.write_all(&compressed).is_ok()
}

/// Matches C++ function: CompressPacket(char *inPacket, char *outPacket)
pub fn compress_packet(_in_packet: &[u8], _out_packet: &mut [u8]) -> Bool {
    true
}

/// Matches C++ function: DecompressPacket(char *inPacket, char *outPacket)
pub fn decompress_packet(_in_packet: &[u8], _out_packet: &mut [u8]) -> Bool {
    true
}

/// Matches C++ function: CalcNewSize(UnsignedInt rawSize)
pub fn calc_new_size(raw_size: UnsignedInt) -> UnsignedInt {
    calc_max_compressed_size_raw(raw_size as usize) as UnsignedInt
}

/// Matches C++ function: DecompressMemory(void*, Int, void*, Int&)
pub fn decompress_memory(
    in_buffer: &[u8],
    in_size: Int,
    out_buffer: &mut [u8],
    out_size: &mut Int,
) -> Bool {
    if in_buffer.is_empty() || out_buffer.is_empty() || in_size < 4 || *out_size <= 0 {
        return false;
    }

    let expected_size = *out_size as usize;
    if out_buffer.len() < expected_size {
        return false;
    }

    let in_size = in_size as usize;
    if in_size > in_buffer.len() {
        return false;
    }

    match decompress_raw(&in_buffer[..in_size], expected_size) {
        Ok(data) => {
            if data.len() != expected_size {
                return false;
            }
            out_buffer[..data.len()].copy_from_slice(&data);
            true
        }
        Err(_) => false,
    }
}

/// Matches C++ function: CompressMemory(void*, Int, void*, Int&)
pub fn compress_memory(
    in_buffer: &[u8],
    in_size: Int,
    out_buffer: &mut [u8],
    out_size: &mut Int,
) -> Bool {
    if in_buffer.is_empty() || out_buffer.is_empty() || in_size < 4 || *out_size <= 0 {
        return false;
    }

    let in_size = in_size as usize;
    if in_size > in_buffer.len() {
        return false;
    }

    let capacity = *out_size as usize;
    if out_buffer.len() < capacity {
        return false;
    }

    let compressed = match compress_raw(&in_buffer[..in_size], CompressionLevel::Default) {
        Ok(data) => data,
        Err(_) => return false,
    };

    if compressed.len() > capacity {
        return false;
    }

    out_buffer[..compressed.len()].copy_from_slice(&compressed);
    *out_size = compressed.len() as Int;
    true
}

use std::io::{Read, Write};

use eac_compression::{
    compress_btree, compress_huffman, compress_refpack, decompress_btree, decompress_huffman,
    decompress_refpack,
};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression as FlateCompression};
use lzh_compression::{
    calc_max_compressed_size_raw, compress_raw, decompress_raw, CompressionLevel,
};

pub type Int = i32;
pub type Bool = bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CompressionType {
    None = 0,
    RefPack = 1,
    NoxLzh = 2,
    ZLib1 = 3,
    ZLib2 = 4,
    ZLib3 = 5,
    ZLib4 = 6,
    ZLib5 = 7,
    ZLib6 = 8,
    ZLib7 = 9,
    ZLib8 = 10,
    ZLib9 = 11,
    BTree = 12,
    Huff = 13,
}

impl CompressionType {
    #[allow(non_upper_case_globals)]
    pub const CompressionMin: CompressionType = CompressionType::None;
    #[allow(non_upper_case_globals)]
    pub const CompressionMax: CompressionType = CompressionType::RefPack;
}

pub struct CompressionManager;

impl CompressionManager {
    pub fn get_compression_name_by_type(comp_type: CompressionType) -> &'static str {
        match comp_type {
            CompressionType::None => "No compression",
            CompressionType::RefPack => "RefPack",
            CompressionType::NoxLzh => "LZHL",
            CompressionType::ZLib1 => "ZLib 1 (fast)",
            CompressionType::ZLib2 => "ZLib 2",
            CompressionType::ZLib3 => "ZLib 3",
            CompressionType::ZLib4 => "ZLib 4",
            CompressionType::ZLib5 => "ZLib 5 (default)",
            CompressionType::ZLib6 => "ZLib 6",
            CompressionType::ZLib7 => "ZLib 7",
            CompressionType::ZLib8 => "ZLib 8",
            CompressionType::ZLib9 => "ZLib 9 (slow)",
            CompressionType::BTree => "BTree",
            CompressionType::Huff => "Huff",
        }
    }

    pub fn get_decompression_name_by_type(comp_type: CompressionType) -> &'static str {
        match comp_type {
            CompressionType::None => "d_None",
            CompressionType::RefPack => "d_RefPack",
            CompressionType::NoxLzh => "d_NoxLZW",
            CompressionType::ZLib1 => "d_ZLib1",
            CompressionType::ZLib2 => "d_ZLib2",
            CompressionType::ZLib3 => "d_ZLib3",
            CompressionType::ZLib4 => "d_ZLib4",
            CompressionType::ZLib5 => "d_ZLib5",
            CompressionType::ZLib6 => "d_ZLib6",
            CompressionType::ZLib7 => "d_ZLib7",
            CompressionType::ZLib8 => "d_ZLib8",
            CompressionType::ZLib9 => "d_ZLib9",
            CompressionType::BTree => "d_BTree",
            CompressionType::Huff => "d_Huff",
        }
    }

    pub fn is_data_compressed(mem: &[u8]) -> Bool {
        Self::get_compression_type(mem) != CompressionType::None
    }

    pub fn get_preferred_compression() -> CompressionType {
        CompressionType::RefPack
    }

    pub fn get_compression_type(mem: &[u8]) -> CompressionType {
        if mem.len() < 8 {
            return CompressionType::None;
        }

        match &mem[0..4] {
            b"NOX\0" => CompressionType::NoxLzh,
            b"ZL1\0" => CompressionType::ZLib1,
            b"ZL2\0" => CompressionType::ZLib2,
            b"ZL3\0" => CompressionType::ZLib3,
            b"ZL4\0" => CompressionType::ZLib4,
            b"ZL5\0" => CompressionType::ZLib5,
            b"ZL6\0" => CompressionType::ZLib6,
            b"ZL7\0" => CompressionType::ZLib7,
            b"ZL8\0" => CompressionType::ZLib8,
            b"ZL9\0" => CompressionType::ZLib9,
            b"EAB\0" => CompressionType::BTree,
            b"EAH\0" => CompressionType::Huff,
            b"EAR\0" => CompressionType::RefPack,
            _ => CompressionType::None,
        }
    }

    pub fn get_max_compressed_size(uncompressed_len: Int, comp_type: CompressionType) -> Int {
        if uncompressed_len <= 0 {
            return 0;
        }

        let uncompressed_len_u = uncompressed_len as usize;
        match comp_type {
            CompressionType::NoxLzh => {
                (calc_max_compressed_size_raw(uncompressed_len_u) + 8) as Int
            }
            CompressionType::BTree | CompressionType::Huff | CompressionType::RefPack => {
                (uncompressed_len_u + 8) as Int
            }
            CompressionType::ZLib1
            | CompressionType::ZLib2
            | CompressionType::ZLib3
            | CompressionType::ZLib4
            | CompressionType::ZLib5
            | CompressionType::ZLib6
            | CompressionType::ZLib7
            | CompressionType::ZLib8
            | CompressionType::ZLib9 => {
                let estimate = (uncompressed_len_u as f64 * 1.1 + 12.0 + 8.0).ceil();
                estimate as Int
            }
            CompressionType::None => 0,
        }
    }

    pub fn get_uncompressed_size(mem: &[u8]) -> Int {
        if mem.len() < 8 {
            return mem.len() as Int;
        }

        match Self::get_compression_type(mem) {
            CompressionType::None => mem.len() as Int,
            _ => Int::from_le_bytes([mem[4], mem[5], mem[6], mem[7]]),
        }
    }

    pub fn compress_data(comp_type: CompressionType, src: &[u8], dest: &mut [u8]) -> Int {
        if dest.len() < 8 {
            return 0;
        }

        let payload_capacity = dest.len() - 8;

        match comp_type {
            CompressionType::BTree => {
                if let Ok(compressed) = compress_btree(src) {
                    if compressed.len() <= dest.len() {
                        dest[..compressed.len()].copy_from_slice(&compressed);
                        return compressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::Huff => {
                if let Ok(compressed) = compress_huffman(src) {
                    if compressed.len() <= dest.len() {
                        dest[..compressed.len()].copy_from_slice(&compressed);
                        return compressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::RefPack => {
                if let Ok(compressed) = compress_refpack(src) {
                    if compressed.len() <= dest.len() {
                        dest[..compressed.len()].copy_from_slice(&compressed);
                        return compressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::NoxLzh => {
                if let Ok(compressed) = compress_raw(src, CompressionLevel::Default) {
                    if compressed.len() > payload_capacity {
                        return 0;
                    }
                    dest[..4].copy_from_slice(b"NOX\0");
                    dest[4..8].copy_from_slice(&(src.len() as Int).to_le_bytes());
                    dest[8..8 + compressed.len()].copy_from_slice(&compressed);
                    return (compressed.len() + 8) as Int;
                }
                0
            }
            CompressionType::ZLib1
            | CompressionType::ZLib2
            | CompressionType::ZLib3
            | CompressionType::ZLib4
            | CompressionType::ZLib5
            | CompressionType::ZLib6
            | CompressionType::ZLib7
            | CompressionType::ZLib8
            | CompressionType::ZLib9 => {
                let level = (comp_type as i32 - CompressionType::ZLib1 as i32 + 1) as u32;
                let mut encoder = ZlibEncoder::new(Vec::new(), FlateCompression::new(level));
                if encoder.write_all(src).is_err() {
                    return 0;
                }
                let compressed = match encoder.finish() {
                    Ok(data) => data,
                    Err(_) => return 0,
                };
                if compressed.len() > payload_capacity {
                    return 0;
                }
                dest[..4].copy_from_slice(b"ZL0\0");
                dest[2] = b'0' + (level as u8);
                dest[4..8].copy_from_slice(&(src.len() as Int).to_le_bytes());
                dest[8..8 + compressed.len()].copy_from_slice(&compressed);
                (compressed.len() + 8) as Int
            }
            CompressionType::None => 0,
        }
    }

    pub fn decompress_data(src: &[u8], dest: &mut [u8]) -> Int {
        if src.len() < 8 {
            return 0;
        }

        match Self::get_compression_type(src) {
            CompressionType::BTree => {
                let compressed = &src[8..];
                if let Ok(decompressed) = decompress_btree(compressed, dest.len()) {
                    if decompressed.len() <= dest.len() {
                        dest[..decompressed.len()].copy_from_slice(&decompressed);
                        return decompressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::Huff => {
                let compressed = &src[8..];
                if let Ok(decompressed) = decompress_huffman(compressed, dest.len()) {
                    if decompressed.len() <= dest.len() {
                        dest[..decompressed.len()].copy_from_slice(&decompressed);
                        return decompressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::RefPack => {
                let compressed = &src[8..];
                if let Ok(decompressed) = decompress_refpack(compressed, dest.len()) {
                    if decompressed.len() <= dest.len() {
                        dest[..decompressed.len()].copy_from_slice(&decompressed);
                        return decompressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::NoxLzh => {
                let compressed = &src[8..];
                if let Ok(decompressed) = decompress_raw(compressed, dest.len()) {
                    if decompressed.len() <= dest.len() {
                        dest[..decompressed.len()].copy_from_slice(&decompressed);
                        return decompressed.len() as Int;
                    }
                }
                0
            }
            CompressionType::ZLib1
            | CompressionType::ZLib2
            | CompressionType::ZLib3
            | CompressionType::ZLib4
            | CompressionType::ZLib5
            | CompressionType::ZLib6
            | CompressionType::ZLib7
            | CompressionType::ZLib8
            | CompressionType::ZLib9 => {
                let compressed = &src[8..];
                let mut decoder = ZlibDecoder::new(compressed);
                let mut decompressed = Vec::with_capacity(dest.len());
                if decoder.read_to_end(&mut decompressed).is_err() {
                    return 0;
                }
                if decompressed.len() <= dest.len() {
                    dest[..decompressed.len()].copy_from_slice(&decompressed);
                    return decompressed.len() as Int;
                }
                0
            }
            CompressionType::None => 0,
        }
    }
}

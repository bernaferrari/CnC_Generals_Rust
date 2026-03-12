use crate::lzo::LzoCompressor;
use crate::straw::{Straw, StrawBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompControl {
    Compress,
    Decompress,
}

#[derive(Default, Clone, Copy)]
struct BlockHeader {
    comp_count: u16,
    uncomp_count: u16,
}

impl BlockHeader {
    fn to_bytes(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out[..2].copy_from_slice(&self.comp_count.to_le_bytes());
        out[2..].copy_from_slice(&self.uncomp_count.to_le_bytes());
        out
    }

    fn from_bytes(data: &[u8]) -> Self {
        let comp = u16::from_le_bytes([data[0], data[1]]);
        let uncomp = u16::from_le_bytes([data[2], data[3]]);
        Self {
            comp_count: comp,
            uncomp_count: uncomp,
        }
    }
}

/// LZO compression/decompression straw.
pub struct LzoStraw {
    base: StrawBase,
    control: CompControl,
    counter: usize,
    buffer: Vec<u8>,
    buffer2: Vec<u8>,
    block_size: usize,
    safety_margin: usize,
    block_header: BlockHeader,
}

impl LzoStraw {
    pub fn new(control: CompControl, block_size: usize) -> Self {
        let safety_margin = block_size;
        let buffer = vec![0u8; block_size + safety_margin];
        let buffer2 = vec![0u8; block_size + safety_margin];
        Self {
            base: StrawBase::new(),
            control,
            counter: 0,
            buffer,
            buffer2,
            block_size,
            safety_margin,
            block_header: BlockHeader::default(),
        }
    }
}

impl Straw for LzoStraw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, dest: &mut [u8]) -> i32 {
        if dest.is_empty() {
            return 0;
        }

        let mut total = 0i32;
        let mut offset = 0usize;
        let mut slen = dest.len();

        while slen > 0 {
            if self.counter > 0 {
                let len = slen.min(self.counter);
                if self.control == CompControl::Decompress {
                    let start = self.block_header.uncomp_count as usize - self.counter;
                    dest[offset..offset + len].copy_from_slice(&self.buffer[start..start + len]);
                } else {
                    let start = (self.block_header.comp_count as usize + 4usize) - self.counter;
                    dest[offset..offset + len].copy_from_slice(&self.buffer2[start..start + len]);
                }
                offset += len;
                slen -= len;
                self.counter -= len;
                total += len as i32;
            }

            if slen == 0 {
                break;
            }

            match self.control {
                CompControl::Decompress => {
                    let mut header_buf = [0u8; 4];
                    let got = Straw::get(self, &mut header_buf) as usize;
                    if got != 4 {
                        break;
                    }
                    self.block_header = BlockHeader::from_bytes(&header_buf);

                    let comp_len = self.block_header.comp_count as usize;
                    let mut staging = vec![0u8; comp_len];
                    let got = Straw::get(self, &mut staging) as usize;
                    if got != comp_len {
                        break;
                    }

                    if let Ok(size) =
                        LzoCompressor::decompress_to_buffer(&staging, &mut self.buffer)
                    {
                        let expected = self.block_header.uncomp_count as usize;
                        let len = expected.min(size);
                        self.block_header.uncomp_count = len as u16;
                        self.counter = len;
                    } else {
                        break;
                    }
                }
                CompControl::Compress => {
                    let mut block = vec![0u8; self.block_size];
                    let got = Straw::get(self, &mut block) as usize;
                    self.block_header.uncomp_count = got as u16;
                    if got == 0 {
                        break;
                    }
                    self.buffer[..got].copy_from_slice(&block[..got]);

                    if let Ok(size) = LzoCompressor::compress_to_buffer(
                        &self.buffer[..got],
                        &mut self.buffer2[4..],
                    ) {
                        self.block_header.comp_count = size as u16;
                        let header = self.block_header.to_bytes();
                        self.buffer2[..4].copy_from_slice(&header);
                        self.counter = size + 4;
                    } else {
                        break;
                    }
                }
            }
        }

        total
    }
}

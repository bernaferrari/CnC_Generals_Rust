use crate::lzo::LzoCompressor;
use crate::pipe::{Pipe, PipeBase};

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

/// LZO compression/decompression pipe.
pub struct LzoPipe {
    base: PipeBase,
    control: CompControl,
    counter: usize,
    buffer: Vec<u8>,
    buffer2: Vec<u8>,
    block_size: usize,
    safety_margin: usize,
    block_header: BlockHeader,
}

impl LzoPipe {
    pub fn new(control: CompControl, block_size: usize) -> Self {
        let safety_margin = block_size;
        let buffer = vec![0u8; block_size + safety_margin];
        let buffer2 = vec![0u8; block_size + safety_margin];
        let block_header = BlockHeader {
            comp_count: 0xFFFF,
            uncomp_count: 0,
        };
        Self {
            base: PipeBase::new(),
            control,
            counter: 0,
            buffer,
            buffer2,
            block_size,
            safety_margin,
            block_header,
        }
    }

    fn pipe_put(&mut self, data: &[u8]) -> i32 {
        Pipe::put(self, data)
    }
}

impl Pipe for LzoPipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if source.is_empty() {
            return Pipe::put(self, source);
        }

        let mut total = 0i32;
        let mut offset = 0usize;
        let mut slen = source.len();

        match self.control {
            CompControl::Decompress => {
                while slen > 0 {
                    if self.block_header.comp_count == 0xFFFF {
                        let needed = 4usize.saturating_sub(self.counter);
                        let len = slen.min(needed);
                        self.buffer[self.counter..self.counter + len]
                            .copy_from_slice(&source[offset..offset + len]);
                        self.counter += len;
                        slen -= len;
                        offset += len;

                        if self.counter == 4 {
                            self.block_header = BlockHeader::from_bytes(&self.buffer[..4]);
                            self.counter = 0;
                        }
                    }

                    if slen > 0 {
                        let target = self.block_header.comp_count as usize;
                        let len = slen.min(target.saturating_sub(self.counter));
                        self.buffer[self.counter..self.counter + len]
                            .copy_from_slice(&source[offset..offset + len]);
                        self.counter += len;
                        slen -= len;
                        offset += len;

                        if self.counter == target {
                            let compressed = self.buffer[..self.counter].to_vec();
                            if let Ok(size) =
                                LzoCompressor::decompress_to_buffer(&compressed, &mut self.buffer2)
                            {
                                let out_len = self.block_header.uncomp_count as usize;
                                let len = out_len.min(size);
                                let decompressed = self.buffer2[..len].to_vec();
                                total += self.pipe_put(&decompressed);
                            } else {
                                total += self.pipe_put(&compressed);
                            }
                            self.counter = 0;
                            self.block_header.comp_count = 0xFFFF;
                        }
                    }
                }
            }
            CompControl::Compress => {
                if self.counter > 0 {
                    let tocopy = slen.min(self.block_size - self.counter);
                    self.buffer[self.counter..self.counter + tocopy]
                        .copy_from_slice(&source[offset..offset + tocopy]);
                    self.counter += tocopy;
                    slen -= tocopy;
                    offset += tocopy;

                    if self.counter == self.block_size {
                        if let Ok(size) = LzoCompressor::compress_to_buffer(
                            &self.buffer[..self.block_size],
                            &mut self.buffer2,
                        ) {
                            self.block_header.comp_count = size as u16;
                            self.block_header.uncomp_count = self.block_size as u16;
                            total += self.pipe_put(&self.block_header.to_bytes());
                            let compressed = self.buffer2[..size].to_vec();
                            total += self.pipe_put(&compressed);
                        }
                        self.counter = 0;
                    }
                }

                while slen >= self.block_size {
                    if let Ok(size) = LzoCompressor::compress_to_buffer(
                        &source[offset..offset + self.block_size],
                        &mut self.buffer2,
                    ) {
                        self.block_header.comp_count = size as u16;
                        self.block_header.uncomp_count = self.block_size as u16;
                        total += self.pipe_put(&self.block_header.to_bytes());
                        let compressed = self.buffer2[..size].to_vec();
                        total += self.pipe_put(&compressed);
                    }
                    offset += self.block_size;
                    slen -= self.block_size;
                }

                if slen > 0 {
                    self.buffer[..slen].copy_from_slice(&source[offset..offset + slen]);
                    self.counter = slen;
                }
            }
        }

        total
    }

    fn flush(&mut self) -> i32 {
        let mut total = 0i32;

        if self.counter > 0 {
            match self.control {
                CompControl::Decompress => {
                    if self.block_header.comp_count == 0xFFFF {
                        let pending = self.buffer[..self.counter].to_vec();
                        total += self.pipe_put(&pending);
                        self.counter = 0;
                    }

                    if self.counter > 0 {
                        total += self.pipe_put(&self.block_header.to_bytes());
                        let pending = self.buffer[..self.counter].to_vec();
                        total += self.pipe_put(&pending);
                        self.counter = 0;
                        self.block_header.comp_count = 0xFFFF;
                    }
                }
                CompControl::Compress => {
                    if let Ok(size) = LzoCompressor::compress_to_buffer(
                        &self.buffer[..self.counter],
                        &mut self.buffer2,
                    ) {
                        self.block_header.comp_count = size as u16;
                        self.block_header.uncomp_count = self.counter as u16;
                        total += self.pipe_put(&self.block_header.to_bytes());
                        let compressed = self.buffer2[..size].to_vec();
                        total += self.pipe_put(&compressed);
                    }
                    self.counter = 0;
                }
            }
        }

        total + Pipe::flush(self)
    }
}

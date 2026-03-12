use crate::base64::{base64_decode, base64_encode};
use crate::pipe::{Pipe, PipeBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeControl {
    Encode,
    Decode,
}

/// Base64 pipe - equivalent to C++ Base64Pipe.
pub struct Base64Pipe {
    base: PipeBase,
    control: CodeControl,
    counter: usize,
    cbuffer: [u8; 4],
    pbuffer: [u8; 3],
}

impl Base64Pipe {
    pub fn new(control: CodeControl) -> Self {
        Self {
            base: PipeBase::new(),
            control,
            counter: 0,
            cbuffer: [0; 4],
            pbuffer: [0; 3],
        }
    }

    fn encode_block(source: &[u8], dest: &mut [u8]) -> usize {
        base64_encode(source, dest)
    }

    fn decode_block(source: &[u8], dest: &mut [u8]) -> usize {
        base64_decode(source, dest)
    }
}

impl Pipe for Base64Pipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if source.is_empty() {
            return self
                .base()
                .chain_to()
                .map_or(0, |next| next.borrow_mut().put(source));
        }

        let mut total = 0i32;
        let mut offset = 0usize;
        let mut slen = source.len();
        let (fromsize, tosize) = match self.control {
            CodeControl::Encode => (3usize, 4usize),
            CodeControl::Decode => (4usize, 3usize),
        };

        if self.counter > 0 {
            let len = slen.min(fromsize - self.counter);
            match self.control {
                CodeControl::Encode => self.pbuffer[self.counter..self.counter + len]
                    .copy_from_slice(&source[offset..offset + len]),
                CodeControl::Decode => self.cbuffer[self.counter..self.counter + len]
                    .copy_from_slice(&source[offset..offset + len]),
            }
            self.counter += len;
            slen -= len;
            offset += len;

            if self.counter == fromsize {
                let mut out = [0u8; 4];
                let outcount = match self.control {
                    CodeControl::Encode => {
                        Self::encode_block(&self.pbuffer[..], &mut out[..tosize])
                    }
                    CodeControl::Decode => {
                        Self::decode_block(&self.cbuffer[..], &mut out[..tosize])
                    }
                };
                total += self.base().chain_to().map_or(outcount as i32, |next| {
                    next.borrow_mut().put(&out[..outcount])
                });
                self.counter = 0;
            }
        }

        while slen >= fromsize {
            let mut out = [0u8; 4];
            let outcount = match self.control {
                CodeControl::Encode => {
                    Self::encode_block(&source[offset..offset + fromsize], &mut out[..tosize])
                }
                CodeControl::Decode => {
                    Self::decode_block(&source[offset..offset + fromsize], &mut out[..tosize])
                }
            };
            total += self.base().chain_to().map_or(outcount as i32, |next| {
                next.borrow_mut().put(&out[..outcount])
            });
            offset += fromsize;
            slen -= fromsize;
        }

        if slen > 0 {
            match self.control {
                CodeControl::Encode => {
                    self.pbuffer[..slen].copy_from_slice(&source[offset..offset + slen])
                }
                CodeControl::Decode => {
                    self.cbuffer[..slen].copy_from_slice(&source[offset..offset + slen])
                }
            }
            self.counter = slen;
        }

        total
    }

    fn flush(&mut self) -> i32 {
        let mut len = 0i32;
        if self.counter > 0 {
            len += match self.control {
                CodeControl::Encode => {
                    let mut out = [0u8; 4];
                    let outcount = Self::encode_block(&self.pbuffer[..self.counter], &mut out);
                    self.base().chain_to().map_or(outcount as i32, |next| {
                        next.borrow_mut().put(&out[..outcount])
                    })
                }
                CodeControl::Decode => {
                    let mut out = [0u8; 3];
                    let outcount = Self::decode_block(&self.cbuffer[..self.counter], &mut out);
                    self.base().chain_to().map_or(outcount as i32, |next| {
                        next.borrow_mut().put(&out[..outcount])
                    })
                }
            };
            self.counter = 0;
        }

        len + self
            .base()
            .chain_to()
            .map_or(0, |next| next.borrow_mut().flush())
    }
}

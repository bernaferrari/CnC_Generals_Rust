use crate::base64::{base64_decode, base64_encode};
use crate::straw::{Straw, StrawBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeControl {
    Encode,
    Decode,
}

pub struct Base64Straw {
    base: StrawBase,
    control: CodeControl,
    counter: usize,
    cbuffer: [u8; 4],
    pbuffer: [u8; 3],
}

impl Base64Straw {
    pub fn new(control: CodeControl) -> Self {
        Self {
            base: StrawBase::new(),
            control,
            counter: 0,
            cbuffer: [0u8; 4],
            pbuffer: [0u8; 3],
        }
    }
}

impl Straw for Base64Straw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if buffer.is_empty() {
            return 0;
        }

        let mut total = 0usize;
        let mut remaining = buffer.len();
        let mut out_ptr = 0usize;

        while remaining > 0 {
            if self.counter > 0 {
                let tosize = match self.control {
                    CodeControl::Encode => self.cbuffer.len(),
                    CodeControl::Decode => self.pbuffer.len(),
                };
                let len = remaining.min(self.counter);
                let start = tosize - self.counter;
                match self.control {
                    CodeControl::Encode => {
                        buffer[out_ptr..out_ptr + len]
                            .copy_from_slice(&self.cbuffer[start..start + len]);
                    }
                    CodeControl::Decode => {
                        buffer[out_ptr..out_ptr + len]
                            .copy_from_slice(&self.pbuffer[start..start + len]);
                    }
                }
                self.counter -= len;
                remaining -= len;
                out_ptr += len;
                total += len;
            }
            if remaining == 0 {
                break;
            }

            let incount = match self.control {
                CodeControl::Encode => self
                    .base()
                    .chain_to()
                    .map_or(0, |next| next.borrow_mut().get(&mut self.pbuffer[..])),
                CodeControl::Decode => self
                    .base()
                    .chain_to()
                    .map_or(0, |next| next.borrow_mut().get(&mut self.cbuffer[..])),
            };
            if incount <= 0 {
                break;
            }
            let incount = incount as usize;
            self.counter = match self.control {
                CodeControl::Encode => {
                    base64_encode(&self.pbuffer[..incount], &mut self.cbuffer[..])
                }
                CodeControl::Decode => {
                    base64_decode(&self.cbuffer[..incount], &mut self.pbuffer[..])
                }
            };
            if self.counter == 0 {
                break;
            }
        }

        total as i32
    }
}

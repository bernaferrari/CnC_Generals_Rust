use crate::crc::CrcEngine;

/// Obfuscate - Sufficiently transform parameter to thwart casual hackers.
/// Ported from WWLib obscure.cpp.
pub fn obfuscate(input: &str) -> i32 {
    obfuscate_bytes(input.as_bytes())
}

pub fn obfuscate_bytes(input: &[u8]) -> i32 {
    if input.is_empty() {
        return 0;
    }

    let mut buffer = [0xA5u8; 128];

    let copy_len = input.len().min(buffer.len() - 1);
    buffer[..copy_len].copy_from_slice(&input[..copy_len]);
    buffer[buffer.len() - 1] = 0;

    let mut length = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());

    for idx in 0..length {
        buffer[idx] = buffer[idx].to_ascii_uppercase();
    }

    for idx in 0..length {
        if !is_graph(buffer[idx]) {
            buffer[idx] = b'A' + (idx % 26) as u8;
        }
    }

    if length < 16 || (length & 0x03) != 0 {
        let mut maxlen = 16usize;
        let padded = (length + 3) & 0x00FC;
        if padded > maxlen {
            maxlen = padded;
        }

        let mut index = length;
        while index < maxlen {
            let prev = buffer[index - length];
            let val = (b'?' ^ prev).wrapping_add(index as u8);
            buffer[index] = b'A' + (val % 26);
            index += 1;
        }
        length = index;
        if length < buffer.len() {
            buffer[length] = 0;
        }
    }

    let mut crc_engine = CrcEngine::new();
    let mut code = crc_engine.update_buffer(&buffer[..length]) as u32;
    let copy = code;

    reverse_in_place(&mut buffer[..length]);
    let mut crc_engine = CrcEngine::new();
    code ^= crc_engine.update_buffer(&buffer[..length]) as u32;

    code ^= copy;

    reverse_in_place(&mut buffer[..length]);
    for idx in 0..length {
        code ^= buffer[idx] as u32;
        let temp = (code & 0xFF) as u8;
        buffer[idx] ^= temp;
        code >>= 8;
        code |= (temp as u32) << 24;
    }

    for idx in 0..length {
        const LOSS_BITS: [u8; 8] = [0x00, 0x08, 0x00, 0x20, 0x00, 0x04, 0x10, 0x00];
        const ADD_BITS: [u8; 8] = [0x10, 0x00, 0x00, 0x80, 0x40, 0x00, 0x00, 0x04];

        buffer[idx] |= ADD_BITS[idx % ADD_BITS.len()];
        buffer[idx] &= !LOSS_BITS[idx % LOSS_BITS.len()];
    }

    let mut idx = 0;
    while idx + 3 < length {
        let key1 = buffer[idx] as i8 as i16;
        let key2 = buffer[idx + 1] as i8 as i16;
        let key3 = buffer[idx + 2] as i8 as i16;
        let key4 = buffer[idx + 3] as i8 as i16;

        let mut val1 = key1;
        let mut val2 = key2;
        let mut val3 = key3;
        let mut val4 = key4;

        val1 = val1.wrapping_mul(key1);
        val2 = val2.wrapping_add(key2);
        val3 = val3.wrapping_add(key3);
        val4 = val4.wrapping_mul(key4);

        let s3 = val3;
        val3 = val3 ^ val1;
        val3 = val3.wrapping_mul(key1);
        let s2 = val2;
        val2 = val2 ^ val4;
        val2 = val2.wrapping_add(val3);
        val2 = val2.wrapping_mul(key3);
        val3 = val3.wrapping_add(val2);

        val1 = val1 ^ val2;
        val4 = val4 ^ val3;

        val2 = val2 ^ s3;
        val3 = val3 ^ s2;

        buffer[idx] = val1 as i8 as u8;
        buffer[idx + 1] = val2 as i8 as u8;
        buffer[idx + 2] = val3 as i8 as u8;
        buffer[idx + 3] = val4 as i8 as u8;

        idx += 4;
    }

    let mut crc_engine = CrcEngine::new();
    crc_engine.update_buffer(&buffer[..length])
}

fn reverse_in_place(slice: &mut [u8]) {
    let mut i = 0usize;
    let mut j = slice.len().saturating_sub(1);
    while i < j {
        slice.swap(i, j);
        i += 1;
        if j == 0 {
            break;
        }
        j -= 1;
    }
}

fn is_graph(value: u8) -> bool {
    value >= 0x21 && value <= 0x7E
}

use crate::straw::Straw;
use crate::trim::{strtrim, wcstrim};
use crate::wwfile::{FileInterface, FileRights};

pub fn read_line_file(file: &mut dyn FileInterface, buffer: &mut [u8], eof: &mut bool) -> usize {
    if buffer.is_empty() {
        return 0;
    }

    if !file.is_open() {
        if !file.is_available(false) {
            *eof = true;
            buffer[0] = 0;
            return 0;
        }
        if file.open(FileRights::Read).is_err() {
            *eof = true;
            buffer[0] = 0;
            return 0;
        }
    }

    let mut count = 0usize;
    loop {
        let mut ch = [0u8; 1];
        match file.read(&mut ch) {
            Ok(1) => {
                if ch[0] == b'\n' {
                    break;
                }
                if ch[0] != b'\r' && count + 1 < buffer.len() {
                    buffer[count] = ch[0];
                    count += 1;
                }
            }
            _ => {
                *eof = true;
                break;
            }
        }
    }

    if count < buffer.len() {
        buffer[count] = 0;
    }
    strtrim(buffer);
    buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len())
}

pub fn read_line_straw(file: &mut dyn Straw, buffer: &mut [u8], eof: &mut bool) -> usize {
    if buffer.is_empty() {
        return 0;
    }

    let mut count = 0usize;
    loop {
        let mut ch = [0u8; 1];
        if file.get(&mut ch) != 1 {
            *eof = true;
            break;
        }
        if ch[0] == b'\n' {
            break;
        }
        if ch[0] != b'\r' && count + 1 < buffer.len() {
            buffer[count] = ch[0];
            count += 1;
        }
    }

    if count < buffer.len() {
        buffer[count] = 0;
    }
    strtrim(buffer);
    buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len())
}

pub fn read_line_wide_straw(file: &mut dyn Straw, buffer: &mut [u16], eof: &mut bool) -> usize {
    if buffer.is_empty() {
        return 0;
    }

    let mut count = 0usize;
    loop {
        let mut bytes = [0u8; 2];
        if file.get(&mut bytes) != 2 {
            *eof = true;
            break;
        }
        let ch = u16::from_le_bytes(bytes);
        if ch == 0x000A {
            break;
        }
        if ch != 0x000D && count + 1 < buffer.len() {
            buffer[count] = ch;
            count += 1;
        }
    }

    if count < buffer.len() {
        buffer[count] = 0;
    }
    wcstrim(buffer);
    buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len())
}

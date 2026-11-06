//! String trim helpers — GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/trim.{h,cpp}
//!
//! Trims leading and trailing bytes with value `<= 32` (C++ whitespace rule) from
//! NUL-terminated `u8` / `u16` buffers in place.
//!
//! Intentional deviation: C++'s trailing loop inspected `*source` after the
//! leading strip (a latent bug once `source` advanced); Rust checks `buffer[index]`
//! only, which matches the documented intent.

/// `strtrim` — trim a NUL-terminated byte buffer in place.
pub fn strtrim(buffer: &mut [u8]) {
    if buffer.is_empty() {
        return;
    }

    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return;
    }

    let mut start = 0usize;
    while start < len && buffer[start] <= 32 {
        start += 1;
    }

    if start > 0 {
        let mut dst = 0usize;
        while start + dst < len {
            buffer[dst] = buffer[start + dst];
            dst += 1;
        }
        if dst < buffer.len() {
            buffer[dst] = 0;
        }
    }

    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return;
    }

    let mut index = len as isize - 1;
    while index >= 0 {
        if buffer[index as usize] <= 32 {
            buffer[index as usize] = 0;
            index -= 1;
        } else {
            break;
        }
    }
}

/// `wcstrim` — wide-char variant of [`strtrim`].
pub fn wcstrim(buffer: &mut [u16]) {
    if buffer.is_empty() {
        return;
    }

    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return;
    }

    let mut start = 0usize;
    while start < len && buffer[start] <= 32 {
        start += 1;
    }

    if start > 0 {
        let mut dst = 0usize;
        while start + dst < len {
            buffer[dst] = buffer[start + dst];
            dst += 1;
        }
        if dst < buffer.len() {
            buffer[dst] = 0;
        }
    }

    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if len == 0 {
        return;
    }

    let mut index = len as isize - 1;
    while index >= 0 {
        if buffer[index as usize] <= 32 {
            buffer[index as usize] = 0;
            index -= 1;
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cstr(bytes: &[u8]) -> Vec<u8> {
        let mut v = bytes.to_vec();
        v.push(0);
        v
    }

    #[test]
    fn strips_leading_and_trailing_spaces() {
        let mut buf = cstr(b"  hi  ");
        strtrim(&mut buf);
        let end = buf.iter().position(|&c| c == 0).unwrap();
        assert_eq!(&buf[..end], b"hi");
    }

    #[test]
    fn empty_and_all_space() {
        let mut empty = cstr(b"");
        strtrim(&mut empty);
        assert_eq!(empty[0], 0);

        let mut spaces = cstr(b"   ");
        strtrim(&mut spaces);
        assert_eq!(spaces[0], 0);
    }

    #[test]
    fn wcstrim_tabs() {
        let mut buf = [0u16; 8];
        buf[0] = 9; // tab
        buf[1] = b'A' as u16;
        buf[2] = b'B' as u16;
        buf[3] = 32;
        buf[4] = 0;
        wcstrim(&mut buf);
        assert_eq!(buf[0], b'A' as u16);
        assert_eq!(buf[1], b'B' as u16);
        assert_eq!(buf[2], 0);
    }
}

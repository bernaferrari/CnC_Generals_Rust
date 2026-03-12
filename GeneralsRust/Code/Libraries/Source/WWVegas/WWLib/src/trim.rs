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

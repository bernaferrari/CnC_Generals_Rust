use crate::rawfile::{RawFile, SeekOrigin};

pub struct TextFileClass {
    file: RawFile,
}

impl TextFileClass {
    pub fn new() -> Self {
        Self {
            file: RawFile::new(),
        }
    }

    pub fn with_name<P: AsRef<std::path::Path>>(filename: P) -> Self {
        Self {
            file: RawFile::with_name(filename),
        }
    }

    pub fn read_line(&mut self, output: &mut String) -> bool {
        output.clear();

        const BUFFER_SIZE: usize = 64;
        let mut buffer = [0u8; BUFFER_SIZE];
        let mut keep_going = true;

        while keep_going {
            let size = match self.file.read(&mut buffer[..BUFFER_SIZE - 1]) {
                Ok(read) => read,
                Err(_) => 0,
            };

            keep_going = size == BUFFER_SIZE - 1;
            if size > 0 {
                let mut newline_index = None;
                for index in 0..size {
                    if buffer[index] == b'\n' {
                        newline_index = Some(index + 1);
                        break;
                    }
                }

                if let Some(stop) = newline_index {
                    if stop < buffer.len() {
                        buffer[stop] = 0;
                    }
                    let back = size.saturating_sub(stop) as i64;
                    if back > 0 {
                        let _ = self.file.seek(-(back as i64), SeekOrigin::Current);
                    }
                    keep_going = false;
                }

                let slice = &buffer[..size];
                let chunk = String::from_utf8_lossy(slice);
                output.push_str(&chunk);
            }
        }

        if output.is_empty() {
            return false;
        }

        if output.ends_with("\r\n") {
            output.truncate(output.len() - 2);
        } else if output.ends_with('\n') {
            output.pop();
        }

        true
    }

    pub fn write_line(&mut self, line: &str) -> bool {
        let len = line.as_bytes().len();
        let size = self.file.write(line.as_bytes()).unwrap_or(0);
        if size == len {
            return self.file.write(b"\r\n").map(|w| w == 2).unwrap_or(false);
        }
        false
    }
}

impl Default for TextFileClass {
    fn default() -> Self {
        Self::new()
    }
}

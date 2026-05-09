use std::io;

use game_engine::common::{
    ascii_string::AsciiString,
    system::{
        file::{BaseFile, File, FileAccess, SeekMode},
        streaming_archive_file::{SharedArchiveFile, StreamingArchiveFile},
    },
};
use std::{cell::RefCell, rc::Rc};

struct TestFile {
    base: BaseFile,
    data: Vec<u8>,
    pos: i32,
}

impl TestFile {
    fn new(data: &[u8]) -> Self {
        let mut base = BaseFile::new();
        base.open_base("archive.big", FileAccess::READ.combine(FileAccess::BINARY))
            .unwrap();
        Self {
            base,
            data: data.to_vec(),
            pos: 0,
        }
    }
}

impl File for TestFile {
    fn open(&mut self, _filename: &str, _access: FileAccess) -> Result<(), io::Error> {
        Ok(())
    }

    fn close(&mut self) {
        self.base.close_base();
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, io::Error> {
        if self.pos >= self.data.len() as i32 {
            return Ok(0);
        }
        let start = self.pos as usize;
        let count = buffer.len().min(self.data.len() - start);
        buffer[..count].copy_from_slice(&self.data[start..start + count]);
        self.pos += count as i32;
        Ok(count)
    }

    fn write(&mut self, _buffer: &[u8]) -> Result<usize, io::Error> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "read-only"))
    }

    fn seek(&mut self, pos: i32, mode: SeekMode) -> Result<i32, io::Error> {
        let new_pos = match mode {
            SeekMode::Start => pos,
            SeekMode::Current => self.pos + pos,
            SeekMode::End => self.data.len() as i32 + pos,
        };
        self.pos = new_pos.clamp(0, self.data.len() as i32);
        Ok(self.pos)
    }

    fn next_line(&mut self, _buf: Option<&mut Vec<u8>>, _buf_size: Option<usize>) {}

    fn scan_int(&mut self) -> Result<i32, io::Error> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    fn scan_real(&mut self) -> Result<f32, io::Error> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    fn scan_string(&mut self) -> Result<AsciiString, io::Error> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    fn print(&mut self, _text: &str) -> Result<bool, io::Error> {
        Ok(false)
    }

    fn size(&self) -> i32 {
        self.data.len() as i32
    }

    fn position(&self) -> i32 {
        self.pos
    }

    fn eof(&self) -> bool {
        self.pos >= self.data.len() as i32
    }

    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn set_name(&mut self, name: &str) {
        self.base.set_name(name);
    }

    fn get_access(&self) -> FileAccess {
        self.base.get_access()
    }

    fn read_entire_and_close(&mut self) -> Result<Vec<u8>, io::Error> {
        self.pos = 0;
        let data = self.data.clone();
        self.close();
        Ok(data)
    }
}

#[test]
fn read_is_bounded_to_archive_slice() {
    let archive: Box<dyn File> = Box::new(TestFile::new(b"0123456789abcdef"));
    let mut file = StreamingArchiveFile::new();

    assert!(file.open_from_archive(archive, &AsciiString::from("slice.bin"), 4, 6));
    assert_eq!(file.size(), 6);
    assert_eq!(file.starting_pos(), 4);

    let mut buffer = [0u8; 8];
    let read = file.read(&mut buffer).unwrap();
    assert_eq!(read, 6);
    assert_eq!(&buffer[..read], b"456789");
    assert!(file.eof());
}

#[test]
fn shared_archive_views_keep_independent_virtual_positions() {
    let shared: SharedArchiveFile = Rc::new(RefCell::new(Box::new(TestFile::new(b"abcdefghij"))));
    let mut first = StreamingArchiveFile::new();
    let mut second = StreamingArchiveFile::new();

    assert!(first.open_from_shared_archive(shared.clone(), &AsciiString::from("first"), 2, 4));
    assert!(second.open_from_shared_archive(shared, &AsciiString::from("second"), 5, 3));

    let mut first_buffer = [0u8; 2];
    let mut second_buffer = [0u8; 3];

    assert_eq!(first.read(&mut first_buffer).unwrap(), 2);
    assert_eq!(&first_buffer, b"cd");
    assert_eq!(first.position(), 2);

    assert_eq!(second.read(&mut second_buffer).unwrap(), 3);
    assert_eq!(&second_buffer, b"fgh");
    assert_eq!(second.position(), 3);

    let mut rest = [0u8; 4];
    assert_eq!(first.read(&mut rest).unwrap(), 2);
    assert_eq!(&rest[..2], b"ef");
}

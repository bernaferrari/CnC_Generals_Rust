use game_engine::common::system::xfer::Xfer;
use game_engine::common::system::xfer_load::XferLoad;
use std::io::{self, Cursor, Read};

#[test]
fn ascii_and_utf16_lengths_payloads_and_truncation_are_counted_from_reader_results() {
    // Empty and nonempty strings use one-byte unit counts, with UTF-16 payload
    // bytes kept little-endian and counted in bytes rather than code units.
    let wire = vec![
        0, // empty ASCII
        2, b'A', b'B', // ASCII "AB"
        0,    // empty UTF-16
        2, 0x41, 0, 0x22, 0x6f, // UTF-16 "A漢"
    ];
    let mut load = XferLoad::new(Cursor::new(wire), 1);
    let mut ascii = "old".to_owned();
    let mut unicode = "old".to_owned();

    load.xfer_ascii_string(&mut ascii).unwrap();
    assert_eq!(ascii, "");
    assert_eq!(load.bytes_read(), 1);
    load.xfer_ascii_string(&mut ascii).unwrap();
    assert_eq!(ascii, "AB");
    assert_eq!(load.bytes_read(), 4);
    load.xfer_unicode_string(&mut unicode).unwrap();
    assert_eq!(unicode, "");
    assert_eq!(load.bytes_read(), 5);
    load.xfer_unicode_string(&mut unicode).unwrap();
    assert_eq!(unicode, "A漢");
    assert_eq!(load.bytes_read(), 10);

    // A short ASCII payload consumes the length byte and the one byte returned
    // before EOF, even though xfer_ascii_string returns an error.
    let mut truncated_ascii = XferLoad::new(Cursor::new(vec![3, b'A']), 1);
    assert!(truncated_ascii.xfer_ascii_string(&mut ascii).is_err());
    assert_eq!(truncated_ascii.bytes_read(), 2);
    assert_eq!(ascii, "AB");

    // The UTF-16 reader consumes one full code unit and one byte of the next
    // before EOF; all four bytes actually returned by Cursor are counted.
    let mut truncated_utf16 = XferLoad::new(Cursor::new(vec![2, 0x41, 0, 0x22]), 1);
    unicode = "preserved".into();
    assert!(truncated_utf16.xfer_unicode_string(&mut unicode).is_err());
    assert_eq!(truncated_utf16.bytes_read(), 4);
    assert_eq!(unicode, "preserved");
}

struct InterruptedOnceReader {
    interrupted: bool,
    bytes: Cursor<Vec<u8>>,
}

impl Read for InterruptedOnceReader {
    fn read(&mut self, destination: &mut [u8]) -> io::Result<usize> {
        if !self.interrupted {
            self.interrupted = true;
            return Err(io::ErrorKind::Interrupted.into());
        }
        self.bytes.read(destination)
    }
}

#[test]
fn scalar_and_skip_partial_reads_share_the_same_accounting() {
    let mut scalar = XferLoad::new(Cursor::new(vec![0x10, 0x20]), 1);
    let mut value = 0u32;
    assert!(scalar.xfer_unsigned_int(&mut value).is_err());
    assert_eq!(scalar.bytes_read(), 2);

    let mut skipped = XferLoad::new(Cursor::new(vec![0x30, 0x40]), 1);
    assert!(skipped.skip(4).is_err());
    assert_eq!(skipped.bytes_read(), 2);
}

#[test]
fn interrupted_reader_retries_without_double_counting() {
    let reader = InterruptedOnceReader {
        interrupted: false,
        bytes: Cursor::new(vec![2, b'O', b'K']),
    };
    let mut load = XferLoad::new(reader, 1);
    let mut value = String::new();

    load.xfer_ascii_string(&mut value).unwrap();

    assert_eq!(value, "OK");
    assert_eq!(load.bytes_read(), 3);
}

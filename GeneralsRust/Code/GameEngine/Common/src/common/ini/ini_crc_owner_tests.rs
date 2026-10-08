//! C++ INI.cpp:397-463 line feed / XferCRC.cpp:74-93,116-159 byte oracle.
use super::*;

fn install(ini: &mut INI) {
    ini.set_xfer(XferCRC::new(XferLoad::new(Cursor::new(Vec::new()), 1)));
}

fn read_all(ini: &mut INI) -> INIResult<()> {
    while !ini.is_eof() {
        ini.read_line()?;
    }
    Ok(())
}

// Consume the accumulator without retaining a parser borrow.
fn take(ini: &mut INI) -> XferCRC<XferLoad<Cursor<Vec<u8>>>> {
    ini.take_xfer().unwrap()
}

fn finish(ini: &mut INI) -> u32 {
    let mut crc = take(ini);
    crc.close().unwrap();
    crc.get_crc()
}

#[test]
fn independently_owned_crc_streams_can_be_interleaved() {
    let mut a = INI::new();
    let mut b = INI::new();
    install(&mut a);
    install(&mut b);
    a.with_inline_source("A = 5\nA = 6", |a| {
        a.read_line()?;
        b.with_inline_source("B = 7\n", read_all)?;
        read_all(a)
    })
    .unwrap();
    assert_eq!(finish(&mut a), 0x8b42_644cu32.to_be());
    assert_eq!(finish(&mut b), 0x8440_7a77u32.to_be());
}

#[test]
fn clear_and_reinstall_do_not_reuse_the_previous_crc_stream() {
    let mut ini = INI::new();
    install(&mut ini);
    ini.with_inline_source("Discarded = 19\n", read_all)
        .unwrap();
    ini.clear_xfer();
    assert!(ini.take_xfer().is_none());
    ini.with_inline_source("Untracked = 31\n", read_all)
        .unwrap();
    install(&mut ini);
    ini.with_inline_source("Tail = 42\n", read_all).unwrap();
    assert_eq!(finish(&mut ini), 0x91ff_e64bu32.to_be());
}

#[test]
fn taking_the_accumulator_moves_the_same_stream_to_another_parser() {
    let mut a = INI::new();
    let mut b = INI::new();
    install(&mut a);
    a.with_inline_source("Prefix = 1\n", read_all).unwrap();
    b.set_xfer(take(&mut a));
    assert!(a.take_xfer().is_none());
    a.with_inline_source("No longer tracked = 22\n", read_all)
        .unwrap();
    b.with_inline_source("Suffix = 2\n", read_all).unwrap();
    assert_eq!(finish(&mut b), 0xc696_45c2u32.to_be());
}

use super::*;

fn write_utf16_field(bytes: &mut Vec<u8>, value: &str) {
    for unit in value.encode_utf16().chain(std::iter::once(0)) {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
}

#[test]
fn replay_version_offsets_follow_all_eight_systemtime_words_at_every_hour() {
    // C++ Recorder.cpp:558-572: SYSTEMTIME then version/build strings and CRCs.
    // Literal authored headers make this independent of the machine's clock.
    for hour in [0u16, 1, 12, 23] {
        let mut bytes = vec![0; 6 + 8 + 8 + 4 + 1 + 1 + 8];
        bytes[..6].copy_from_slice(b"GENREP");
        write_utf16_field(&mut bytes, "FixtureReplay");
        for word in [2026, 10, 4, 15, hour, 27, 0, 0] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        let expected_version_start = bytes.len();
        write_utf16_field(&mut bytes, "Version 1.04");
        let expected_version_end = bytes.len();
        write_utf16_field(&mut bytes, "Oct 15 2026 00:00:00");
        let expected_number = bytes.len();
        bytes.extend_from_slice(&0x0100_0004u32.to_le_bytes());
        bytes.extend_from_slice(&0x1122_3344u32.to_le_bytes());
        bytes.extend_from_slice(&0x5566_7788u32.to_le_bytes());

        let (start, end, build_start, number, exe_crc, ini_crc) = replay_version_offsets(&bytes);
        assert_eq!(
            (start, end, build_start, number),
            (
                expected_version_start,
                expected_version_end,
                expected_version_end,
                expected_number
            ),
            "hour {hour}"
        );
        assert_eq!(replay_utf16_field(&bytes, start, end), "Version 1.04");
        assert_eq!(
            replay_utf16_field(&bytes, build_start, number),
            "Oct 15 2026 00:00:00"
        );
        assert_eq!(
            u32::from_le_bytes(bytes[number..number + 4].try_into().unwrap()),
            0x0100_0004
        );
        assert_eq!(
            u32::from_le_bytes(bytes[exe_crc..exe_crc + 4].try_into().unwrap()),
            0x1122_3344
        );
        assert_eq!(
            u32::from_le_bytes(bytes[ini_crc..ini_crc + 4].try_into().unwrap()),
            0x5566_7788
        );
    }
}

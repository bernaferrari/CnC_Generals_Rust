use super::*;

fn document(names: &[(u32, &str)], body: &[u8]) -> Vec<u8> {
    let mut bytes = b"CkMp".to_vec();
    bytes.extend_from_slice(&(names.len() as i32).to_le_bytes());
    for (id, name) in names {
        bytes.push(name.len() as u8);
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes.extend_from_slice(body);
    bytes
}

fn chunk(id: u32, version: u16, payload: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_le_bytes().to_vec();
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn traversal_borrows_payloads_and_preserves_repeated_chunk_order() {
    // DataChunkInput::parse reads declarations in stream order, including repetitions.
    let mut body = chunk(7, 1, &[10]);
    body.extend(chunk(7, 2, &[20]));
    let map = ChunkyMap::decode("same.map".into(), document(&[(7, "Script")], &body)).unwrap();
    let body = &map.bytes[map.body_offset..];
    let mut seen = Vec::new();
    parse_chunk_sequence(body, &map.toc, |label, version, payload| {
        assert!(payload.as_ptr() >= body.as_ptr());
        assert!(payload.as_ptr() < body.as_ptr().wrapping_add(body.len()));
        seen.push((label.to_string(), version, payload[0]));
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, [("Script".into(), 1, 10), ("Script".into(), 2, 20)]);
    assert!(
        find_chunk_by_label(body, &map.toc, "script")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        find_chunk_by_label(body, &map.toc, "Script")
            .unwrap()
            .unwrap()
            .0,
        1
    );
}

#[test]
fn repeated_toc_id_uses_last_mapping_like_cpp_prepend_lookup() {
    // DataChunkTableOfContents::read prepends mappings; getName searches from that head.
    let (toc, _) = parse_chunk_toc(&document(&[(1, "old"), (1, "new")], &[])).unwrap();
    assert_eq!(toc[&1], "new");
}

#[test]
fn documents_with_same_source_are_independent_and_inert() {
    let first = ChunkyMap::decode("same.map".into(), document(&[(1, "first")], &[])).unwrap();
    let second = ChunkyMap::decode("same.map".into(), document(&[(1, "second")], &[])).unwrap();
    assert_eq!(first.toc[&1], "first");
    assert_eq!(second.toc[&1], "second");
    let retained = first.clone();
    drop(first);
    assert_eq!(retained.toc[&1], "first");
    assert_eq!(second.toc[&1], "second");
}

#[test]
fn invalid_toc_counts_fail_before_allocating_from_untrusted_count() {
    for count in [-1i32, i32::MAX] {
        let mut bytes = b"CkMp".to_vec();
        bytes.extend_from_slice(&count.to_le_bytes());
        assert!(parse_chunk_toc(&bytes).is_err());
    }
    assert!(parse_chunk_toc(b"CkM").is_err());
    assert!(parse_chunk_toc(b"junk\0\0\0\0").is_err());
}

#[test]
fn parent_bounds_unknown_labels_and_negative_sizes_are_rejected() {
    let toc = [(1, "known".into())].into_iter().collect();
    assert!(find_chunk_by_label(&chunk(2, 1, &[]), &toc, "known").is_err());
    let mut negative = chunk(1, 1, &[]);
    negative[6..10].copy_from_slice(&(-1i32).to_le_bytes());
    assert!(find_chunk_by_label(&negative, &toc, "known").is_err());
    let mut truncated = chunk(1, 1, &[1, 2]);
    truncated.pop();
    assert!(find_chunk_by_label(&truncated, &toc, "known").is_err());
    // Preserve the existing adapter's end-of-sequence behavior for a short trailing header.
    assert!(
        find_chunk_by_label(&[1, 2], &toc, "known")
            .unwrap()
            .is_none()
    );
}

#[test]
fn scalars_strings_and_failed_read_preserve_cursor_semantics() {
    let mut bytes = 0x1234u16.to_le_bytes().to_vec();
    bytes.extend_from_slice(&(-42i32).to_le_bytes());
    bytes.extend_from_slice(&1.25f32.to_le_bytes());
    bytes.extend_from_slice(&3u16.to_le_bytes());
    bytes.extend_from_slice(b"USA");
    let text = "中国".encode_utf16().collect::<Vec<_>>();
    bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
    for unit in text {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let mut reader = BinaryReader::new(&bytes);
    assert_eq!(reader.read_u16().unwrap(), 0x1234);
    assert_eq!(reader.read_i32().unwrap(), -42);
    assert_eq!(reader.read_f32().unwrap(), 1.25);
    assert_eq!(reader.read_ascii_string().unwrap(), "USA");
    assert_eq!(reader.read_unicode_string().unwrap(), "中国");
    let end = reader.position();
    assert!(reader.read_u32().is_err());
    assert_eq!(reader.position(), end);
}

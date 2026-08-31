use std::path::PathBuf;

// This test scans the local W3D samples for AdaptiveDelta compressed animations.
// It is ignored by default to avoid failing in environments without the samples.
#[test]
#[ignore]
fn scan_local_w3d_for_adaptive_delta() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let w3d_dir = PathBuf::from(crate_dir).join("W3D");
    assert!(
        w3d_dir.exists(),
        "W3D directory not found at {}",
        w3d_dir.display()
    );
    // Reuse internal helpers by duplicating minimal logic here to avoid changing visibility
    let mut ad_count = 0usize;
    for entry in std::fs::read_dir(&w3d_dir).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let p = entry.path();
        if p.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("w3d"))
            .unwrap_or(false)
        {
            let bytes = std::fs::read(&p).expect("read w3d");
            if contains_adaptive_delta(&bytes) {
                ad_count += 1;
            }
        }
    }
    assert!(
        ad_count > 0,
        "No AdaptiveDelta files found in {}",
        w3d_dir.display()
    );
}

fn contains_adaptive_delta(data: &[u8]) -> bool {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::io::{Cursor, Read, Seek, SeekFrom};
    let mut r = Cursor::new(data);
    while (r.position() as usize) + 8 <= data.len() {
        let chunk_id = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        };
        let size = match r.read_u32::<LittleEndian>() {
            Ok(v) => v,
            Err(_) => break,
        } & 0x7FFF_FFFF;
        let start = r.position();
        let end = start + size as u64;
        if chunk_id == 0x0000_0281 {
            // CompressedAnimationHeader
            if (r.position() as usize) + 4 + 16 + 16 + 4 + 2 + 2 <= data.len() {
                let _version = r.read_u32::<LittleEndian>().ok();
                let mut name = [0u8; 16];
                r.read_exact(&mut name).ok();
                let mut hname = [0u8; 16];
                r.read_exact(&mut hname).ok();
                let _frames = r.read_u32::<LittleEndian>().ok();
                let _fr = r.read_u16::<LittleEndian>().ok();
                let flavor = r.read_u16::<LittleEndian>().unwrap_or(0);
                if flavor != 0 {
                    return true;
                }
            }
        }
        let _ = r.seek(SeekFrom::Start(end));
    }
    false
}

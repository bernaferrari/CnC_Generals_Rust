//! GameInfo.cpp489–588; FileTransfer.cpp157–216 shared sidecar path semantics.
/// Original FileTransfer path utilities are pure; accept both map separators.
fn sidecar_paths(map: &str) -> [(i32, String); 6] {
    let normalized = map.replace('\\', "/");
    let (base, file) = normalized
        .rsplit_once('/')
        .unwrap_or(("", normalized.as_str()));
    let stem = file.rsplit_once('.').map_or("", |(stem, _)| stem);
    let join = |leaf: &str| {
        if base.is_empty() {
            leaf.to_string()
        } else {
            format!("{base}/{leaf}")
        }
    };
    [
        (2, join(&format!("{stem}.tga"))),
        (4, join("map.ini")),
        (8, join("map.str")),
        (16, join("solo.ini")),
        (32, join("assetusage.txt")),
        (64, join("readme.txt")),
    ]
}
fn contents_with(map: &str, in_cache: bool, mut open: impl FnMut(&str) -> bool) -> i32 {
    if !in_cache {
        return 0;
    }
    let mut mask = 1;
    for (bit, path) in sidecar_paths(map) {
        if open(&path) {
            mask |= bit;
        }
    }
    mask
}
pub(super) fn observe_contents(map: &str, in_cache: bool) -> i32 {
    use game_engine::common::system::file_system::get_file_system;
    if !in_cache {
        return 0;
    }
    let fs = get_file_system();
    let mut fs = fs.lock().unwrap_or_else(|e| e.into_inner());
    contents_from_file_system(map, true, &mut fs)
}
fn contents_from_file_system(
    map: &str,
    in_cache: bool,
    fs: &mut game_engine::common::system::file_system::FileSystem,
) -> i32 {
    use game_engine::common::system::file::FileAccess;
    contents_with(map, in_cache, |path| {
        if let Some(mut file) = fs.open_file(path, FileAccess::READ) {
            file.close();
            true
        } else {
            false
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_sidecar_order_bits_and_cache_gate_match_cpp() {
        let mut opened = Vec::new();
        assert_eq!(
            contents_with(r"Maps\Alpine\Alpine.map", false, |_| panic!(
                "no cache no probe"
            )),
            0
        );
        assert_eq!(
            contents_with(r"Maps\Alpine\Alpine.map", true, |path| {
                opened.push(path.to_string());
                true
            }),
            127
        );
        assert_eq!(
            opened,
            [
                "Maps/Alpine/Alpine.tga",
                "Maps/Alpine/map.ini",
                "Maps/Alpine/map.str",
                "Maps/Alpine/solo.ini",
                "Maps/Alpine/assetusage.txt",
                "Maps/Alpine/readme.txt"
            ]
        );
        assert_eq!(
            contents_with("Maps/A/A.map", true, |path| path.ends_with("map.str")
                || path.ends_with("readme.txt")),
            1 | 8 | 64
        );
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod archive_tests {
    use super::*;
    #[test]
    fn native_probe_opens_archive_members_without_loose_sidecar_files() {
        use game_engine::common::system::{
            big_file_system::BigArchiveBackend, file_system::FileSystem,
            subsystem_interface::SubsystemInterface,
        };
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let names = [
            "Maps/Probe/Probe.tga",
            "Maps/Probe/map.str",
            "Maps/Probe/readme.txt",
        ];
        let header = 16 + names.iter().map(|name| 8 + name.len() + 1).sum::<usize>();
        let payload = b"sidecar";
        let total = header + payload.len() * names.len();
        let mut big = std::fs::File::create(dir.path().join("Sidecar.big")).unwrap();
        big.write_all(b"BIGF").unwrap();
        big.write_all(&(total as u32).to_le_bytes()).unwrap();
        big.write_all(&(names.len() as u32).to_be_bytes()).unwrap();
        big.write_all(&(header as u32).to_be_bytes()).unwrap();
        for (index, name) in names.iter().enumerate() {
            big.write_all(&((header + index * payload.len()) as u32).to_be_bytes())
                .unwrap();
            big.write_all(&(payload.len() as u32).to_be_bytes())
                .unwrap();
            big.write_all(name.as_bytes()).unwrap();
            big.write_all(&[0]).unwrap();
        }
        for _ in &names {
            big.write_all(payload).unwrap();
        }
        drop(big);
        let mut backend = BigArchiveBackend::new();
        backend.add_search_path(dir.path());
        let mut fs = FileSystem::new();
        fs.register_backend(Box::new(backend));
        fs.init().unwrap();
        assert!(!std::path::Path::new(names[0]).exists());
        assert_eq!(
            contents_from_file_system("Maps/Probe/Probe.map", true, &mut fs),
            1 | 2 | 8 | 64
        );
        assert_eq!(
            contents_from_file_system("Maps/Probe/Probe.map", false, &mut fs),
            0
        );
    }
}

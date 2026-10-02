use super::*;

#[test]
fn ten_absent_audio_paths_discover_once_and_keep_absence_cached() {
    let mut resolver = DataIniResolver::new();
    assert!(resolver.roots.is_none() && resolver.extracted.is_none());
    let mut discoveries = 0;
    let mut extractions = 0;
    for index in 0..10 {
        assert!(
            resolver
                .resolve_with_discovery(
                    &format!("Data/INI/MissingAudio{index}.ini"),
                    || {
                        discoveries += 1;
                        Vec::new()
                    },
                    |roots| {
                        extractions += 1;
                        assert!(roots.is_empty());
                        Vec::new()
                    },
                )
                .is_none()
        );
    }
    assert_eq!(discoveries, 1, "one discovery per ordered audio load");
    assert_eq!(
        extractions, 1,
        "empty extracted-root results must also be reused"
    );
}

struct Layout(std::path::PathBuf);
impl Layout {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "generals-ini-resolver-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, path: &str, contents: &[u8]) -> PathBuf {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }
}
impl Drop for Layout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const COMPLETE: &[u8] = b"; complete patch\nAudioEvent Fixture\nEnd\n";
const PATH: &str = "Data/INI/ResolverFixture.ini";

#[test]
fn direct_files_leave_discovery_lazy_and_preserve_explicit_path_behavior() {
    let layout = Layout::new("direct");
    let direct = layout.write(PATH, b" fragment retained for explicit paths\n");
    let mut resolver = DataIniResolver::new();
    assert_eq!(
        resolver.resolve_with_discovery(
            direct.to_str().unwrap(),
            || panic!("direct path does not discover installations"),
            |_| panic!("direct path does not scan extracted trees")
        ),
        Some(direct)
    );
    assert!(resolver.roots.is_none() && resolver.extracted.is_none());
}

#[test]
fn root_order_and_inizh_precedence_are_preserved_without_extract_scan() {
    let layout = Layout::new("precedence");
    let first = layout.0.join("first");
    let second = layout.0.join("second");
    let patch = layout.write(&format!("first/INIZH/{PATH}"), COMPLETE);
    layout.write(&format!("second/{PATH}"), COMPLETE);
    let mut resolver = DataIniResolver::new();
    let resolve = |resolver: &mut DataIniResolver| {
        resolver.resolve_with_discovery(
            PATH,
            || vec![first.clone(), second.clone()],
            |_| panic!("root hit must not scan"),
        )
    };
    assert_eq!(resolve(&mut resolver), Some(patch));
    let direct = layout.write(&format!("first/{PATH}"), COMPLETE);
    assert_eq!(resolve(&mut resolver), Some(direct));
    assert!(resolver.extracted.is_none());
}

#[test]
fn invalid_overrides_fall_through_and_file_contents_are_never_cached() {
    let layout = Layout::new("validation");
    let invalid = layout.write(PATH, b"\0Data\\INI\\Table.ini\0");
    let patch = layout.write(&format!("INIZH/{PATH}"), COMPLETE);
    let mut resolver = DataIniResolver::new();
    let resolve = |resolver: &mut DataIniResolver| {
        resolver.resolve_with_discovery(PATH, || vec![layout.0.clone()], |_| Vec::new())
    };
    assert_eq!(resolve(&mut resolver), Some(patch.clone()));
    std::fs::write(&invalid, COMPLETE).unwrap();
    assert_eq!(resolve(&mut resolver), Some(invalid.clone()));
    std::fs::write(&invalid, b" lowercase fragment\nEnd\n").unwrap();
    assert_eq!(resolve(&mut resolver), Some(patch.clone()));
    std::fs::remove_file(patch).unwrap();
    assert_eq!(resolve(&mut resolver), None);
}

#[test]
fn extracted_roots_are_discovered_once_but_file_misses_are_not_cached() {
    let layout = Layout::new("extracted");
    let extracted = layout.0.join("extract/INIZH");
    std::fs::create_dir_all(&extracted).unwrap();
    let mut resolver = DataIniResolver::new();
    let mut scans = 0;
    let mut resolve = |resolver: &mut DataIniResolver| {
        resolver.resolve_with_discovery(
            PATH,
            || vec![layout.0.join("install")],
            |_| {
                scans += 1;
                vec![extracted.clone()]
            },
        )
    };
    assert_eq!(resolve(&mut resolver), None);
    let patch = layout.write(&format!("extract/INIZH/{PATH}"), COMPLETE);
    assert_eq!(resolve(&mut resolver), Some(patch.clone()));
    std::fs::remove_file(patch).unwrap();
    assert_eq!(resolve(&mut resolver), None);
    assert_eq!(scans, 1);
}

#[test]
fn separate_loads_keep_their_own_discovery_results_and_reset_on_drop() {
    let first = Layout::new("instance-one");
    let second = Layout::new("instance-two");
    let first_path = first.write(&format!("extract/{PATH}"), COMPLETE);
    let second_path = second.write(&format!("extract/{PATH}"), COMPLETE);
    let mut a = DataIniResolver::new();
    let mut b = DataIniResolver::new();
    let mut discoveries = [0; 2];
    let mut scans = [0; 2];
    for _ in 0..3 {
        for (index, resolver, layout, expected) in [
            (0, &mut a, &first, &first_path),
            (1, &mut b, &second, &second_path),
        ] {
            assert_eq!(
                resolver.resolve_with_discovery(
                    PATH,
                    || {
                        discoveries[index] += 1;
                        vec![layout.0.join("install")]
                    },
                    |_| {
                        scans[index] += 1;
                        vec![layout.0.join("extract")]
                    }
                ),
                Some(expected.clone())
            );
        }
    }
    assert_eq!(discoveries, [1, 1]);
    assert_eq!(scans, [1, 1]);
    drop(a);
    let mut fresh = DataIniResolver::new();
    assert_eq!(
        fresh.resolve_with_discovery(
            PATH,
            || vec![second.0.join("install")],
            |_| vec![second.0.join("extract")]
        ),
        Some(second_path)
    );
}

#[test]
fn real_extraction_discovers_mixed_case_archive_directories() {
    let layout = Layout::new("mixed-case");
    let expected = layout.write(&format!("loose/iNiZh/{PATH}"), COMPLETE);
    let mut resolver = DataIniResolver::new();
    let found = resolver
        .resolve_with_discovery(
            "Data\\INI\\ResolverFixture.ini",
            || vec![layout.0.clone()],
            extracted_asset_roots_from,
        )
        .expect("mixed-case extracted archive tree");
    // Case-insensitive filesystems may return the conventional INIZH spelling.
    assert_eq!(
        found.canonicalize().unwrap(),
        expected.canonicalize().unwrap()
    );
}

#[test]
fn invalid_extracted_override_keeps_archive_fallback_available() {
    let layout = Layout::new("archive-fallback");
    layout.write(&format!("extract/INIZH/{PATH}"), b" broken extract\nEnd\n");
    let mut resolver = DataIniResolver::new();
    assert_eq!(
        resolver.resolve_with_discovery(
            PATH,
            || vec![layout.0.join("install")],
            |_| vec![layout.0.join("extract/INIZH")]
        ),
        None
    );
}

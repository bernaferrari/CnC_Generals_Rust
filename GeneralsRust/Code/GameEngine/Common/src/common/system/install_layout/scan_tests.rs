use super::scan::{
    find_dir_case_insensitive_in, find_file_case_insensitive_in, read_scanned_entries,
    scan_from_with,
};
use super::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "generals-install-scan-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_dir(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

fn make_file(path: &Path) {
    if let Some(parent) = path.parent() {
        make_dir(parent);
    }
    fs::write(path, b"marker").unwrap();
}

// Baseline copy of the pre-index scan operations. Source pin: install_layout.rs
// SHA-256 569085346cd1f0d78caf3a0e10e627acbf62956911960c9bd622a0ff167fa217
// (ranges 141-216 before this change). This intentionally retains the repeated
// lookup/read_dir shape.
fn old_find_dir_case_insensitive(dir: &Path, name: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_dir() {
        return Some(direct);
    }
    let want = name.to_ascii_lowercase();
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let file_name = entry.file_name();
        if file_name.to_string_lossy().eq_ignore_ascii_case(&want) && entry.path().is_dir() {
            return Some(entry.path());
        }
    }
    None
}

fn old_scan_from(start: &Path, max_depth: usize) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut installs = Vec::new();
    let mut extracted = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start.to_path_buf(), 0usize));
    let mut visited = 0usize;

    while let Some((dir, depth)) = queue.pop_front() {
        if visited >= MAX_SCAN_DIRS {
            break;
        }
        if !dir.is_dir() || !seen.insert(path_key(&dir)) {
            continue;
        }
        visited += 1;

        if MARKER_BIGS
            .iter()
            .any(|name| find_file_case_insensitive(&dir, name).is_some())
        {
            installs.push(dir.clone());
        }
        if EXTRACTED_DIR_NAMES.iter().any(|name| {
            let child = dir.join(name);
            child.is_dir() || old_find_dir_case_insensitive(&dir, name).is_some()
        }) {
            extracted.push(dir.clone());
            for name in EXTRACTED_DIR_NAMES {
                if let Some(child) = old_find_dir_case_insensitive(&dir, name) {
                    extracted.push(child);
                }
            }
        }

        if depth >= max_depth {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if is_skip_dir_name(&name) {
                continue;
            }
            queue.push_back((path, depth + 1));
        }
    }

    (installs, extracted)
}

fn old_extracted_asset_roots_from(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        let (_, extracted) = old_scan_from(root, MAX_SCAN_DEPTH);
        for path in extracted {
            push_unique(&mut out, &mut seen, path);
        }
        for name in EXTRACTED_DIR_NAMES {
            if let Some(child) = old_find_dir_case_insensitive(root, name) {
                push_unique(&mut out, &mut seen, child);
            }
        }
    }
    out
}

#[test]
fn indexed_scan_matches_old_outputs_and_enumerates_each_visited_directory_once() {
    let fixture = Fixture::new("equivalence");
    let root = fixture.root();

    // Multiple markers, mixed-case fallback, and exact direct-name precedence
    // over a competing case variant where the filesystem permits both names.
    make_file(&root.join("GensecZH.big"));
    make_file(&root.join("W3DZH.BIG"));
    make_file(&root.join("w3dzh.big"));

    make_dir(&root.join("INIZH"));
    make_dir(&root.join("inizh"));
    make_dir(&root.join("windowzh"));
    make_dir(&root.join("mApSzH"));

    let nested = root.join("visible-a").join("visible-b");
    make_file(&nested.join("AuDiOZh.big"));
    make_dir(&nested.join("TexturesZH"));
    make_file(
        &root
            .join("visible-a")
            .join("visible-b")
            .join("visible-c")
            .join("EnglishZH.big"),
    );
    make_file(
        &root
            .join("visible-a")
            .join("visible-b")
            .join("visible-c")
            .join("beyond-depth")
            .join("inizh.big"),
    );
    make_file(&root.join("target/should-not-scan/inizh.big"));
    make_file(&root.join("node_modules/should-not-scan/genseczh.big"));
    make_file(&root.join(".hidden/should-not-scan/musiczh.big"));
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.join("visible-a/visible-b"),
        root.join("linked-visible-b"),
    )
    .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("w3dzh.big"), root.join("TexturesZH.BIG")).unwrap();

    let expected = old_scan_from(root, MAX_SCAN_DEPTH);
    let mut counts = HashMap::<PathBuf, usize>::new();
    let actual = scan_from_with(root, MAX_SCAN_DEPTH, |dir| {
        *counts.entry(dir.to_path_buf()).or_default() += 1;
        read_scanned_entries(dir)
    });
    assert_eq!(
        actual, expected,
        "install/extracted output order and path casing changed"
    );
    assert!(counts.values().all(|count| *count == 1));
    assert!(!counts.keys().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(is_skip_dir_name)
            .unwrap_or(false)
    }));
    assert!(
        counts
            .keys()
            .any(|path| path.ends_with("visible-a/visible-b/visible-c"))
    );
    assert!(
        !counts
            .keys()
            .any(|path| { path.ends_with("visible-a/visible-b/visible-c/beyond-depth") })
    );

    let roots = [root.to_path_buf()];
    assert_eq!(
        extracted_asset_roots_from(&roots),
        old_extracted_asset_roots_from(&roots),
        "extracted root results or order changed"
    );

    let mut failed_reads = Vec::new();
    let direct_only = scan_from_with(root, MAX_SCAN_DEPTH, |dir| {
        failed_reads.push(dir.to_path_buf());
        None
    });
    assert_eq!(failed_reads, [root.to_path_buf()]);
    let direct_installs = if MARKER_BIGS.iter().any(|name| root.join(name).is_file()) {
        vec![root.to_path_buf()]
    } else {
        Vec::new()
    };
    let direct_children: Vec<_> = EXTRACTED_DIR_NAMES
        .iter()
        .filter_map(|name| {
            let child = root.join(name);
            child.is_dir().then_some(child)
        })
        .collect();
    let direct_extracted = if direct_children.is_empty() {
        Vec::new()
    } else {
        std::iter::once(root.to_path_buf())
            .chain(direct_children)
            .collect()
    };
    assert_eq!(direct_only, (direct_installs, direct_extracted));

    // On a case-sensitive filesystem both spellings coexist, so the exact
    // requested path must win. On a case-insensitive filesystem the paths
    // resolve to the same directory and still exercise direct lookup.
    let direct = root.join("w3dzh.big");
    let resolved = find_file_case_insensitive(root, "w3dzh.big").unwrap();
    assert_eq!(resolved, direct);
    let gensec_direct = root.join("genseczh.big");
    let gensec_expected = if gensec_direct.is_file() {
        gensec_direct
    } else {
        root.join("GensecZH.big")
    };
    assert_eq!(
        find_file_case_insensitive(root, "genseczh.big").unwrap(),
        gensec_expected
    );
    let marker_entries = read_scanned_entries(root);
    let symlink_marker_direct = root.join("textureszh.big");
    let symlink_marker_expected = if symlink_marker_direct.is_file() {
        Some(symlink_marker_direct)
    } else {
        #[cfg(unix)]
        {
            Some(root.join("TexturesZH.BIG"))
        }
        #[cfg(not(unix))]
        {
            None
        }
    };
    assert_eq!(
        find_file_case_insensitive_in(root, "textureszh.big", marker_entries.as_deref()),
        symlink_marker_expected
    );
    let direct_child = find_dir_case_insensitive_in(root, "INIZH", None).unwrap();
    assert_eq!(direct_child, root.join("INIZH"));
    let root_entries = read_scanned_entries(root);
    let window_direct = root.join("WindowZH");
    let window_expected = if window_direct.is_dir() {
        window_direct
    } else {
        root.join("windowzh")
    };
    assert_eq!(
        find_dir_case_insensitive_in(root, "WindowZH", root_entries.as_deref()).unwrap(),
        window_expected
    );

    let wrong_type_marker = root.join("audiozh.big");
    make_dir(&wrong_type_marker);
    let matching_marker_file = root.join("AUDIOZH.BIG");
    if !matching_marker_file.exists() {
        make_file(&matching_marker_file);
    }
    let wrong_type_entries = read_scanned_entries(root);
    let expected_marker = if matching_marker_file.is_file() {
        Some(matching_marker_file)
    } else {
        None
    };
    assert_eq!(
        find_file_case_insensitive_in(root, "audiozh.big", wrong_type_entries.as_deref()),
        expected_marker
    );

    let wrong_type_child = root.join("AudioZH");
    make_file(&wrong_type_child);
    let matching_child_dir = root.join("audiozh");
    if !matching_child_dir.exists() {
        make_dir(&matching_child_dir);
    }
    let wrong_type_entries = read_scanned_entries(root);
    let expected_child = if matching_child_dir.is_dir() {
        Some(matching_child_dir)
    } else {
        None
    };
    assert_eq!(
        find_dir_case_insensitive_in(root, "AudioZH", wrong_type_entries.as_deref()),
        expected_child
    );
}

#[test]
fn indexed_scan_keeps_the_ninety_six_directory_budget() {
    let fixture = Fixture::new("budget");
    for index in 0..110 {
        let child = fixture.root().join(format!("child-{index:03}"));
        make_file(&child.join("genseczh.big"));
    }

    let expected = old_scan_from(fixture.root(), MAX_SCAN_DEPTH);
    let mut visits = 0usize;
    let actual = scan_from_with(fixture.root(), MAX_SCAN_DEPTH, |dir| {
        visits += 1;
        read_scanned_entries(dir)
    });
    assert_eq!(actual, expected);
    assert_eq!(actual.0.len(), MAX_SCAN_DIRS - 1);
    assert_eq!(visits, MAX_SCAN_DIRS);
}

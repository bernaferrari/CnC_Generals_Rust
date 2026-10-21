//! Boot Object INI sources matching C++ ThingFactory init.
//!
//! `GameEngine.cpp:458` loads `Data\\INI\\Default\\Object.ini` then
//! `INI::loadDirectory("Data\\INI\\Object", TRUE, INI_LOAD_OVERWRITE)`.
//! Directory files are listed through FileSystem (local + archive), current
//! directory first (sorted), then subdirectories.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DEFAULT_OBJECT_INI: &str = "Data/INI/Default/Object.ini";
const OBJECT_INI_DIR: &str = "Data/INI/Object";

/// Virtual paths in C++ ThingFactory order: Default/Object.ini, then
/// `Data/INI/Object` current-dir files, then nested files.
pub fn collect_object_ini_virtual_paths() -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();

    push_unique(&mut ordered, &mut seen, DEFAULT_OBJECT_INI);

    let (current_dir, nested) = collect_object_directory_files();
    for path in current_dir {
        push_unique(&mut ordered, &mut seen, &path);
    }
    for path in nested {
        push_unique(&mut ordered, &mut seen, &path);
    }

    ordered
}

fn push_unique(ordered: &mut Vec<String>, seen: &mut BTreeSet<String>, path: &str) {
    let key = path.replace('\\', "/").to_ascii_lowercase();
    if seen.insert(key) {
        ordered.push(path.replace('\\', "/"));
    }
}

fn collect_object_directory_files() -> (Vec<String>, Vec<String>) {
    let mut current = BTreeSet::new();
    let mut nested = BTreeSet::new();

    collect_from_file_system(&mut current, &mut nested);
    collect_from_disk_roots(&mut current, &mut nested);
    collect_from_live_archive(&mut current, &mut nested);

    (current.into_iter().collect(), nested.into_iter().collect())
}

fn collect_from_file_system(current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::system::file_system::{FilenameList, get_file_system};

    let fs_lock = get_file_system();
    let Ok(fs) = fs_lock.lock() else {
        return;
    };
    let mut list = FilenameList::new();
    fs.get_file_list_in_directory(
        &AsciiString::from(OBJECT_INI_DIR),
        &AsciiString::from("*.ini"),
        &mut list,
        true,
    );
    for name in list {
        classify_object_ini(name.as_str(), current, nested);
    }
}

fn collect_from_disk_roots(current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    for root in super::CnCGameEngine::startup_ini_disk_roots() {
        let dir = if root == "." {
            PathBuf::from(OBJECT_INI_DIR)
        } else {
            Path::new(root).join(OBJECT_INI_DIR)
        };
        walk_object_ini_dir(&dir, OBJECT_INI_DIR, current, nested);
    }
}

fn walk_object_ini_dir(
    dir: &Path,
    virtual_prefix: &str,
    current: &mut BTreeSet<String>,
    nested: &mut BTreeSet<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ini"))
        {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                files.push(name.to_string());
            }
        }
    }
    files.sort_by_key(|n| n.to_ascii_lowercase());
    for name in files {
        current.insert(format!("{virtual_prefix}/{name}"));
    }
    subdirs.sort();
    for sub in subdirs {
        let Some(name) = sub.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let child_virtual = format!("{virtual_prefix}/{name}");
        walk_nested_object_ini_dir(&sub, &child_virtual, nested);
    }
}

fn walk_nested_object_ini_dir(dir: &Path, virtual_prefix: &str, nested: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ini"))
        {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                files.push(name.to_string());
            }
        }
    }
    files.sort_by_key(|n| n.to_ascii_lowercase());
    for name in files {
        nested.insert(format!("{virtual_prefix}/{name}"));
    }
    subdirs.sort();
    for sub in subdirs {
        let Some(name) = sub.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        walk_nested_object_ini_dir(&sub, &format!("{virtual_prefix}/{name}"), nested);
    }
}

fn collect_from_live_archive(current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    let Some(manager_arc) = crate::assets::manager::get_asset_manager() else {
        return;
    };
    let Ok(mgr) = manager_arc.try_lock() else {
        return;
    };
    for path in mgr.list_all_files() {
        classify_object_ini(&path, current, nested);
    }
}

fn classify_object_ini(path: &str, current: &mut BTreeSet<String>, nested: &mut BTreeSet<String>) {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_ascii_lowercase();
    if !lower.starts_with("data/ini/object/") || !lower.ends_with(".ini") {
        return;
    }
    let rest = &normalized["Data/INI/Object/".len()..];
    if rest.contains('/') || rest.contains('\\') {
        nested.insert(normalized);
    } else {
        current.insert(normalized);
    }
}

/// One actual receiving producer over the caller's existing ordered archive sources.
/// Missing optional files retain the old startup policy; malformed present content
/// rejects admission instead of publishing a partial global template stream.
pub(super) fn load_receiving_object_content(
    paths: &[String],
    read: impl FnMut(&str) -> Result<Option<String>, String>,
) -> Result<crate::assets::ReceivingObjectContent, String> {
    load_receiving_object_content_with_sciences(
        paths,
        read,
        std::sync::Arc::new(game_engine::common::rts::science::ScienceNameFacts::default()),
    )
}
pub(super) fn load_receiving_object_content_with_sciences(
    paths: &[String],
    mut read: impl FnMut(&str) -> Result<Option<String>, String>,
    sciences: std::sync::Arc<game_engine::common::rts::science::ScienceNameFacts>,
) -> Result<crate::assets::ReceivingObjectContent, String> {
    let mut sources = Vec::new();
    for path in paths {
        if let Some(source) = read(path)? {
            sources.push((path.clone(), source));
        }
    }
    crate::assets::ReceivingObjectContent::from_sources_with_sciences(sources, sciences)
}

/// Read one selected startup source without turning malformed/busy/unreadable
/// content into absence. The archive closure borrows the actual receiving service.
pub(super) fn read_receiving_object_source(
    virtual_path: &str,
    roots: &[&str],
    mut archive: impl FnMut(&str) -> Result<Option<Vec<u8>>, String>,
) -> Result<Option<String>, String> {
    let normalized = virtual_path.replace('\\', "/");
    for root in roots {
        let path = if *root == "." {
            PathBuf::from(&normalized)
        } else {
            Path::new(root).join(&normalized)
        };
        if *root != "."
            && !game_engine::common::system::install_layout::ini_loose_override_is_authoritative(
                &path,
            )
        {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) => return decode_receiving_object_source(virtual_path, bytes).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Cannot read selected Object INI '{}': {error}",
                    path.display()
                ));
            }
        }
    }
    archive(virtual_path)?
        .map(|bytes| decode_receiving_object_source(virtual_path, bytes))
        .transpose()
}
fn decode_receiving_object_source(path: &str, bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes)
        .map_err(|error| format!("Object INI '{path}' is not valid UTF-8: {error}"))
}

#[cfg(test)]
mod receiving_source_tests {
    use super::*;
    #[test]
    fn receiving_object_source_rejects_invalid_present_bytes_and_busy_service() {
        let invalid = read_receiving_object_source("Data/INI/Object/Present.ini", &[], |_| {
            Ok(Some(vec![0xff]))
        })
        .unwrap_err();
        assert!(
            invalid.contains("Present.ini") && invalid.contains("UTF-8"),
            "{invalid}"
        );
        let busy = read_receiving_object_source("Data/INI/Object/Present.ini", &[], |_| {
            Err("receiving asset manager busy".into())
        })
        .unwrap_err();
        assert_eq!(busy, "receiving asset manager busy");
        assert!(
            read_receiving_object_source("Absent.ini", &[], |_| Ok(None))
                .unwrap()
                .is_none()
        );
        let source = read_receiving_object_source("Present.ini", &[], |_| {
            Ok(Some(b"Object Actual\nScale = 2\nEnd".to_vec()))
        })
        .unwrap()
        .unwrap();
        assert!(source.starts_with("Object Actual"));
    }
    #[test]
    fn actual_local_object_source_invalid_utf8_read_error_and_missing_remain_distinct() {
        let root = tempfile::tempdir().unwrap();
        let invalid = root.path().join("Invalid.ini");
        std::fs::write(&invalid, [0xff]).unwrap();
        let mut archive_calls = 0;
        let error = read_receiving_object_source(invalid.to_str().unwrap(), &["."], |_| {
            archive_calls += 1;
            Ok(Some(b"Object Foreign\nEnd".to_vec()))
        })
        .unwrap_err();
        assert!(
            error.contains("Invalid.ini") && error.contains("UTF-8"),
            "{error}"
        );
        assert_eq!(
            archive_calls, 0,
            "selected present local bytes cannot fall back"
        );
        let error = read_receiving_object_source(root.path().to_str().unwrap(), &["."], |_| {
            archive_calls += 1;
            Ok(None)
        })
        .unwrap_err();
        assert!(error.contains("Cannot read selected Object INI"), "{error}");
        assert_eq!(archive_calls, 0);
        let absent = root.path().join("Absent.ini");
        assert!(
            read_receiving_object_source(absent.to_str().unwrap(), &["."], |_| {
                archive_calls += 1;
                Ok(None)
            })
            .unwrap()
            .is_none()
        );
        assert_eq!(archive_calls, 1, "genuine absence may consult archive");
    }
    #[test]
    fn receiving_object_boot_rejects_late_reader_failure_instead_of_accepting_partial_content() {
        let paths = vec!["First.ini".into(), "Busy.ini".into()];
        let mut observed = Vec::new();
        let error = load_receiving_object_content(&paths, |path| {
            observed.push(path.to_owned());
            if path == "First.ini" {
                Ok(Some("Object Actual\nScale = 2\nEnd".into()))
            } else {
                Err("receiving archive service busy".into())
            }
        })
        .err()
        .unwrap();
        assert_eq!(observed, paths);
        assert_eq!(error, "receiving archive service busy");
        let content = load_receiving_object_content(&["First.ini".into()], |_| {
            Ok(Some("Object Actual\nScale = 2\nEnd".into()))
        })
        .unwrap();
        assert_eq!(
            content
                .module_content()
                .admitted_template("Actual")
                .unwrap()
                .name(),
            "Actual"
        );
    }
}

#[cfg(test)]
mod retail_receiving_content_tests {
    use super::*;
    /// Explicit acceptance only: a missing licensed fixture is a failure when run.
    #[test]
    #[ignore = "requires user-provided retail Zero Hour content"]
    fn actual_retail_ordered_object_sources_admit_one_complete_receiving_content_owner() {
        let retail = std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .ancestors()
                    .nth(3)
                    .expect("Main remains within GeneralsRust/Code")
                    .join("windows_game")
            });
        let archive_dir = retail.join("Command & Conquer Generals Zero Hour/Data");
        assert!(
            archive_dir.join("INIZH.big").is_file(),
            "licensed retail INIZH.big is required: {}",
            archive_dir.display()
        );
        let mut files = crate::assets::ArchiveFileSystem::new();
        files.add_search_path(&archive_dir);
        pollster::block_on(files.init()).expect("actual retail archive mounts");
        let mut current = BTreeSet::new();
        let mut nested = BTreeSet::new();
        for path in files.list_all_files() {
            classify_object_ini(&path, &mut current, &mut nested);
        }
        let mut paths = Vec::new();
        let mut seen = BTreeSet::new();
        push_unique(&mut paths, &mut seen, DEFAULT_OBJECT_INI);
        for path in current.into_iter().chain(nested) {
            push_unique(&mut paths, &mut seen, &path);
        }
        assert_eq!(
            paths.len(),
            45,
            "stock retail Object source inventory must be complete"
        );
        assert_eq!(
            paths
                .iter()
                .filter(|path| path.eq_ignore_ascii_case("Data/INI/Object/System.ini"))
                .count(),
            1
        );
        let roots = [
            retail.join("extracted_big_files/INIZH"),
            retail.join("extracted_big_files_v2/INIZH"),
        ];
        let root_strings = roots
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let root_refs = root_strings.iter().map(String::as_str).collect::<Vec<_>>();
        let mut science_sources = Vec::new();
        for path in ["Data/INI/Default/Science.ini", "Data/INI/Science.ini"] {
            if let Some(bytes) = files
                .read_optional_definition_source(path)
                .expect("actual Science source read")
            {
                science_sources.push((path.to_owned(), bytes));
            }
        }
        assert!(
            science_sources
                .iter()
                .any(|(path, _)| path == "Data/INI/Science.ini"),
            "genuine retail Science definitions required"
        );
        let source_refs = science_sources
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice(), false))
            .collect::<Vec<_>>();
        let mut science_store = game_engine::common::rts::science::ScienceStore::new();
        science_store
            .load_from_sources(&source_refs)
            .expect("actual Science grammar into local application definition store");
        let sciences = std::sync::Arc::new(
            game_engine::common::rts::science::ScienceNameFacts::from_store(&science_store),
        );
        let mut reads = Vec::new();
        let mut hash = crc32fast::Hasher::new();
        let content = load_receiving_object_content_with_sciences(&paths, |path| {
            let text = read_receiving_object_source(path,&root_refs,
                |path| files.read_optional_definition_source(path).map_err(|error|error.to_string()))?
                .ok_or_else(||format!("listed retail Object source missing: {path}"))?;
            let source_hash = crc32fast::hash(text.as_bytes());
            if path.eq_ignore_ascii_case(DEFAULT_OBJECT_INI) {
                assert_eq!((text.len(),source_hash),(5530,0xc8f767c2),"genuine Default/Object bytes, not corrupt extracted offsets");
            }
            if path.eq_ignore_ascii_case("Data/INI/Object/System.ini") {
                assert_eq!((text.len(),source_hash),(77862,0x1aab427a),"genuine System bytes, no synthetic tracer replay");
            }
            // Canonical names make this source proof independent of archive index casing.
            hash.update(path.to_ascii_lowercase().as_bytes());
            hash.update(&[0]);hash.update(text.as_bytes());hash.update(&[0]);
            reads.push(path.to_owned());
            Ok(Some(text))
        }, sciences).expect("all actual retail Object definitions must admit strictly; no unknown-field/module skip");
        assert_eq!(
            reads, paths,
            "each actual selected path is read once in original order"
        );
        assert_eq!(
            hash.finalize(),
            RETAIL_ORDERED_CRC32,
            "complete genuine ordered source stream"
        );
        assert_eq!(content.module_content().admitted_templates().count(), 2104);
        assert_eq!(content.projections().rows().count(), 2104);
        for name in [
            "DefaultThingTemplate",
            "GenericTracer",
            "AmericaInfantryRanger",
            "AmericaVehicleHumvee",
        ] {
            assert!(
                content.module_content().admitted_template(name).is_some(),
                "typed retail {name}"
            );
            assert!(
                content.projections().get(name).is_some(),
                "same-source retail projection {name}"
            );
        }
        let tracer = content
            .module_content()
            .admitted_template("GenericTracer")
            .unwrap();
        let draw = tracer
            .draw_modules()
            .iter()
            .find(|module| module.name.as_str() == "W3DTracerDraw")
            .expect("actual System.ini typed tracer declaration");
        assert_eq!(draw.module_tag.as_str(), "ModuleTag_01");
        assert!(
            draw.data
                .as_any()
                .is::<gamelogic::object::draw::W3DTracerDrawModuleData>()
        );
    }
    const RETAIL_ORDERED_CRC32: u32 = 0x87d9e029;
}

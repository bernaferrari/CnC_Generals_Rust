//! Behavioral tests for Object INI template construction.
use super::*;

fn admit_fixture_content(
    logic: &mut GameLogic,
    content: crate::assets::ReceivingObjectContent,
    names: &[&str],
) {
    logic
        .admit_receiving_module_content(content)
        .expect("actual owned fixture Object admission");
    for name in names {
        let template = logic
            .templates
            .get(*name)
            .expect("admitted fixture template");
        assert!(template.receiving_object_definition.is_some(), "{name}");
        assert!(template.receiving_client_template.is_some(), "{name}");
    }
}

fn admit_fixture_source(logic: &mut GameLogic, path: &str, text: &str, names: &[&str]) {
    let content =
        crate::assets::ReceivingObjectContent::from_sources([(path.to_owned(), text.to_owned())])
            .expect("complete source grammar through the actual typed factory");
    admit_fixture_content(logic, content, names);
}

/// The licensed retail archive the receiving fixtures read their INI from.
fn retail_archive() -> crate::assets::ArchiveFileSystem {
    use std::path::Path;
    let retail = std::env::var_os("GENERALS_RETAIL_CONTENT_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(3)
                .unwrap()
                .join("windows_game")
        });
    let mut files = crate::assets::ArchiveFileSystem::new();
    files.add_search_path(retail.join("Command & Conquer Generals Zero Hour/Data"));
    pollster::block_on(files.init()).expect("licensed retail archive mounts");
    files
}

/// Retail CommandButton.ini and CommandSet.ini. C++ loads both into the
/// engine's own TheControlBar before NewGame (GameEngine.cpp:428-458,
/// ControlBar.cpp:1060-1120); presentation reads only that catalog.
/// Retail CommandButton.ini then CommandSet.ini, one source per file as the
/// shell loads them (buttons must exist before sets reference them).
fn retail_command_ini() -> Vec<String> {
    let mut files = retail_archive();
    ["Data/INI/CommandButton.ini", "Data/INI/CommandSet.ini"]
        .into_iter()
        .map(|path| {
            let bytes = files
                .read_optional_definition_source(path)
                .expect("retail command source read")
                .unwrap_or_else(|| panic!("retail {path} exists"));
            String::from_utf8(bytes).expect("retail command UTF-8")
        })
        .collect()
}

fn retail_receiving_fixture(extra: &[(&str, &str)]) -> crate::assets::ReceivingObjectContent {
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::sync::Arc;
    // GameEngine.cpp:458 loads Default/Object.ini then the complete Object
    // directory. Partial retail files cannot supply prerequisite/reskin
    // identity. The archive, Science facts, and receiving owner stay local.
    let mut files = retail_archive();
    let mut current = BTreeSet::new();
    let mut nested = BTreeSet::new();
    for path in files.list_all_files() {
        let path = path.replace('\\', "/");
        let lower = path.to_ascii_lowercase();
        if lower.starts_with("data/ini/object/") && lower.ends_with(".ini") {
            if path["Data/INI/Object/".len()..].contains('/') {
                nested.insert(path);
            } else {
                current.insert(path);
            }
        }
    }
    let paths = std::iter::once("Data/INI/Default/Object.ini".to_owned())
        .chain(current)
        .chain(nested)
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 45, "complete licensed retail Object namespace");
    let mut sciences = game_engine::common::rts::science::ScienceStore::new();
    let science_sources = ["Data/INI/Default/Science.ini", "Data/INI/Science.ini"]
        .into_iter()
        .filter_map(|path| {
            files
                .read_optional_definition_source(path)
                .expect("retail Science source read")
                .map(|bytes| (path.to_owned(), bytes))
        })
        .collect::<Vec<_>>();
    assert!(
        science_sources
            .iter()
            .any(|(path, _)| path == "Data/INI/Science.ini")
    );
    let refs = science_sources
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.as_slice(), false))
        .collect::<Vec<_>>();
    sciences
        .load_from_sources(&refs)
        .expect("owned retail Science definitions");
    let mut sources = paths
        .into_iter()
        .map(|path| {
            let bytes = files
                .read_optional_definition_source(&path)
                .expect("retail Object source read")
                .expect("listed retail Object source exists");
            (path, String::from_utf8(bytes).expect("retail Object UTF-8"))
        })
        .collect::<Vec<_>>();
    sources.extend(
        extra
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned())),
    );
    let facts =
        Arc::new(game_engine::common::rts::science::ScienceNameFacts::from_store(&sciences));
    let mut actions =
        game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions::default();
    let power_source = files
        .read_optional_definition_source("Data/INI/SpecialPower.ini")
        .expect("retail SpecialPower source read")
        .expect("licensed retail SpecialPower definitions");
    actions
        .apply_source(
            std::str::from_utf8(&power_source).expect("retail SpecialPower UTF-8"),
            &facts,
        )
        .expect("owned retail SpecialPower definitions with receiving Science facts");
    crate::assets::ReceivingObjectContent::from_sources_with_catalogs(
        sources,
        facts,
        Arc::new(actions),
    )
    .expect("actual complete retail receiving content, no partial namespace fallback")
}

mod contain_modules;
mod ini_stamps;
mod module_metadata;
mod production_and_supply;
mod template_catalogue;
mod vision_and_shroud;

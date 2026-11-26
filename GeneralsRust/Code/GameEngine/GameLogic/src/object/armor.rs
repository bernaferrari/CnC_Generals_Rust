//! Armor templates and store (Rust port of C++ `Armor.h` / `Armor.cpp`).

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;

use crate::common::AsciiString;
use generals_sim::armor::{ArmorParseError, parse_armor_templates};
pub use generals_sim::armor::{Armor, ArmorTemplate};

/// Thread-safe global store mirroring the legacy `TheArmorStore` singleton.
#[derive(Debug, Default)]
pub struct ArmorStore {
    templates: HashMap<String, Arc<ArmorTemplate>>,
}

impl ArmorStore {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Register (or replace) an armor template under the given name.
    pub fn register_template(
        &mut self,
        name: &AsciiString,
        template: ArmorTemplate,
    ) -> Arc<ArmorTemplate> {
        let key = canonical_key(name);
        let entry = Arc::new(template);
        self.templates.insert(key, entry.clone());
        entry
    }

    /// Retrieve an armor template by name (case-insensitive).
    pub fn find_template(&self, name: &AsciiString) -> Option<Arc<ArmorTemplate>> {
        let key = canonical_key(name);
        self.templates.get(&key).cloned()
    }

    /// Build an `Armor` instance from a named template.
    pub fn make_armor_by_name(&self, name: &AsciiString) -> Option<Armor> {
        self.find_template(name).map(Armor::from_template)
    }

    /// Build an armor instance from an already-shared template reference.
    pub fn make_armor(&self, template: Arc<ArmorTemplate>) -> Armor {
        Armor::from_template(template)
    }

    /// Remove all stored templates (primarily for tests or resets).
    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// Number of registered templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }
}

#[derive(Debug, Error)]
pub enum ArmorLoadError {
    #[error("Armor INI file not found")]
    FileNotFound,
    #[error("IO error while reading armor data: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Content(#[from] ArmorParseError),
}

pub fn load_armor_templates_from_path<P: AsRef<Path>>(path: P) -> Result<usize, ArmorLoadError> {
    let content = fs::read_to_string(&path)?;
    load_armor_templates_from_str(&content, Some(path.as_ref()))
}

pub fn load_armor_templates_from_str(
    content: &str,
    source: Option<&Path>,
) -> Result<usize, ArmorLoadError> {
    let parsed = parse_armor_templates(content)?;
    if parsed.is_empty() {
        return Err(ArmorParseError::Empty.into());
    }
    let services = crate::system::engine_stores::active_services();
    Ok(admit_armor_templates(&services, parsed, source, false))
}

fn load_armor_templates_into(
    services: &crate::system::engine_stores::WorldServices,
    content: &str,
    source: Option<&Path>,
    only_if_empty: bool,
) -> Result<usize, ArmorLoadError> {
    let parsed = parse_armor_templates(content)?;
    if parsed.is_empty() {
        return Err(ArmorParseError::Empty.into());
    }
    Ok(admit_armor_templates(services, parsed, source, only_if_empty))
}

fn admit_armor_templates(
    services: &crate::system::engine_stores::WorldServices,
    parsed: Vec<(String, ArmorTemplate)>,
    source: Option<&Path>,
    only_if_empty: bool,
) -> usize {

    let count = parsed.len();
    {
        let mut store = services.armor_store().write().expect("armor write");
        if only_if_empty && store.len() > 0 {
            return 0;
        }
        store.clear();
        for (name, template) in parsed {
            store.register_template(&AsciiString::from(&name), template);
        }
    }

    log::info!(
        "Loaded {} armor templates{}",
        count,
        source
            .map(|p| format!(" from {}", p.display()))
            .unwrap_or_default()
    );

    count
}

fn default_armor_paths() -> [PathBuf; 3] {
    [
        PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/Armor.ini"),
        PathBuf::from("windows_game/extracted_big_files_v2/INI/Armor.ini"),
        PathBuf::from("Data/INI/Armor.ini"),
    ]
}

fn load_default_templates_internal(
    services: &crate::system::engine_stores::WorldServices,
    paths: &[PathBuf],
    use_archive: bool,
) -> Result<(), ArmorLoadError> {
    {
        let __armor = services.armor_store();
        let store = __armor.read().expect("armor read");
        if store.len() > 0 {
            return Ok(());
        }
    }

    for path in paths {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            load_armor_templates_into(services, &content, Some(path), true)?;
            return Ok(());
        }
    }

    if use_archive {
        load_armor_from_retail_archive(services)
    } else {
        Err(ArmorLoadError::FileNotFound)
    }
}

fn load_armor_from_retail_archive(
    services: &crate::system::engine_stores::WorldServices,
) -> Result<(), ArmorLoadError> {
    for root in game_engine::common::system::install_layout::zh_install_roots() {
        for name in ["INIZH.big", "inizh.big"] {
            let Some(big_path) =
                game_engine::common::system::install_layout::find_file_case_insensitive(
                    &root, name,
                )
            else {
                continue;
            };
            let mut big = game_engine::common::system::big_file_system::BigFile::new();
            if big.open(&big_path).is_err() {
                continue;
            }
            let entry = big
                .entries()
                .iter()
                .find(|entry| {
                    entry
                        .filename
                        .replace('\\', "/")
                        .eq_ignore_ascii_case("data/ini/armor.ini")
                })
                .cloned();
            let Some(entry) = entry else {
                continue;
            };
            let Ok(bytes) = big.extract_file_data(&entry) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            load_armor_templates_into(services, &text, Some(big_path.as_path()), true)?;
            return Ok(());
        }
    }
    Err(ArmorLoadError::FileNotFound)
}

fn ensure_default_templates_loaded_in(
    services: &crate::system::engine_stores::WorldServices,
    paths: &[PathBuf],
    use_archive: bool,
) {
    services.armor_defaults_initialized().get_or_init(|| {
        if let Err(err) = load_default_templates_internal(services, paths, use_archive) {
            log::warn!("Armor templates could not be loaded: {err}");
        }
    });
}

pub fn ensure_default_templates_loaded() {
    let services = crate::system::engine_stores::active_services();
    if services.armor_defaults_initialized().get().is_some() {
        return;
    }
    ensure_default_templates_loaded_in(&services, &default_armor_paths(), true);
}

fn canonical_key(name: &AsciiString) -> String {
    name.as_str().to_ascii_lowercase()
}

/// Helper exposing the per-world store in a form similar to the original singleton.
pub struct TheArmorStore;

impl TheArmorStore {
    fn store_arc() -> std::sync::Arc<RwLock<ArmorStore>> {
        std::sync::Arc::clone(crate::system::engine_stores::active_services().armor_store())
    }

    /// Register (or overwrite) a template under the supplied name.
    pub fn register_template(name: &AsciiString, template: ArmorTemplate) -> Arc<ArmorTemplate> {
        let arc = Self::store_arc();
        arc.write()
            .expect("TheArmorStore write lock poisoned")
            .register_template(name, template)
    }

    /// Locate a template and clone the shared reference.
    pub fn find_template(name: &AsciiString) -> Option<Arc<ArmorTemplate>> {
        let arc = Self::store_arc();
        arc.read()
            .expect("TheArmorStore read lock poisoned")
            .find_template(name)
    }

    /// Convenience helper matching the C++ `makeArmor` inline.
    pub fn make_armor(template: Arc<ArmorTemplate>) -> Armor {
        let arc = Self::store_arc();
        arc.read()
            .expect("TheArmorStore read lock poisoned")
            .make_armor(template)
    }

    /// Try to build an armor instance from the named template.
    pub fn make_armor_by_name(name: &AsciiString) -> Option<Armor> {
        let arc = Self::store_arc();
        arc.read()
            .expect("TheArmorStore read lock poisoned")
            .make_armor_by_name(name)
    }

    /// Number of templates registered in the published world's armor store.
    pub fn template_count() -> usize {
        Self::store_arc()
            .read()
            .expect("TheArmorStore read lock poisoned")
            .len()
    }

    /// Remove all registered templates (useful for resetting between missions/tests).
    pub fn reset() {
        let arc = Self::store_arc();
        arc.write()
            .expect("TheArmorStore write lock poisoned")
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage::DamageType;
    use generals_sim::armor::ArmorDefinitions;

    #[test]
    fn default_armor_load_is_once_per_world_and_preserves_existing_content() {
        use crate::system::engine_stores::WorldServices;
        let dir = std::env::temp_dir().join(format!(
            "generals-armor-defaults-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Armor.ini");
        fs::write(&path, "Armor AuthoredWorldArmor\n Armor = DEFAULT 35%\nEnd\n").unwrap();
        let paths = [path.clone()];
        let first = WorldServices::new_for_world();
        let second = WorldServices::new_for_world();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| ensure_default_templates_loaded_in(&first, &paths, false));
            }
        });
        let name = AsciiString::from("AuthoredWorldArmor");
        assert_eq!(first.armor_store().read().unwrap().find_template(&name).unwrap()
            .adjust_damage(DamageType::Explosion, 100.0), 35.0);
        first.armor_store().write().unwrap().clear();
        ensure_default_templates_loaded_in(&first, &paths, false);
        assert!(first.armor_store().read().unwrap().find_template(&name).is_none());
        ensure_default_templates_loaded_in(&second, &paths, false);
        assert!(second.armor_store().read().unwrap().find_template(&name).is_some());
        drop(first);
        let third = WorldServices::new_for_world();
        ensure_default_templates_loaded_in(&third, &paths, false);
        assert!(third.armor_store().read().unwrap().find_template(&name).is_some());
        let existing = WorldServices::new_for_world();
        let mut template = ArmorTemplate::new();
        template.set_default(0.8);
        existing.armor_store().write().unwrap().register_template(&name, template);
        ensure_default_templates_loaded_in(&existing, &paths, false);
        assert_eq!(existing.armor_store().read().unwrap().find_template(&name).unwrap()
            .adjust_damage(DamageType::Explosion, 100.0), 80.0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_default_armor_attempt_is_not_retried_in_same_world() {
        use crate::system::engine_stores::WorldServices;
        let dir = std::env::temp_dir().join(format!(
            "generals-armor-failure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let paths = [dir.join("Armor.ini")];
        let first = WorldServices::new_for_world();
        ensure_default_templates_loaded_in(&first, &paths, false);
        fs::write(&paths[0], "Armor AfterFailure\n Armor = DEFAULT 40%\nEnd\n").unwrap();
        ensure_default_templates_loaded_in(&first, &paths, false);
        assert_eq!(first.armor_store().read().unwrap().len(), 0);
        let second = WorldServices::new_for_world();
        ensure_default_templates_loaded_in(&second, &paths, false);
        assert!(second.armor_store().read().unwrap()
            .find_template(&AsciiString::from("AfterFailure")).is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn owned_armor_full_ini_preserves_order_replacement_and_session_isolation() {
        // INI.cpp1586 scanPercentToReal divides; multiplication by 0.01 has
        // a different f32 result for source-valid coefficients such as 33%.
        let precise =
            ArmorDefinitions::load_from_str("Armor Precise\n Armor = DEFAULT 33%\nEnd\n").unwrap();
        assert_eq!(
            precise
                .find_template("Precise")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 1.0)
                .to_bits(),
            (33.0f32 / 100.0).to_bits()
        );
        use game_engine::common::ini::ini::{INI, armor_definitions};
        let mut parser = INI::new();
        parser.set_armor_definitions_target(armor_definitions::ArmorDefinitions::new());
        parser
            .with_inline_source(
                "Armor Shared\n Armor = FLAME 20%\n Armor = DEFAULT 50%\n Armor = FLAME 75%\nEnd\nArmor Untouched\n Armor = DEFAULT 30%\nEnd\n",
                |ini| ini.parse_current_file(),
            )
            .unwrap();
        let mut first = ArmorDefinitions::new();
        first
            .overlay_rows(&parser.take_armor_definitions_target().unwrap())
            .unwrap();
        let mut second = first.clone();
        parser.set_armor_definitions_target(armor_definitions::ArmorDefinitions::new());
        parser
            .with_inline_source("Armor Shared\n Armor = FLAME 25%\nEnd\n", |ini| {
                ini.parse_current_file()
            })
            .unwrap();
        second
            .overlay_rows(&parser.take_armor_definitions_target().unwrap())
            .unwrap();
        assert_eq!(
            first
                .find_template("SHARED")
                .unwrap()
                .adjust_damage(DamageType::Flame, 100.0),
            75.0
        );
        assert_eq!(
            first
                .find_template("shared")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            50.0
        );
        assert_eq!(
            second
                .find_template("shared")
                .unwrap()
                .adjust_damage(DamageType::Flame, 100.0),
            25.0
        );
        assert_eq!(
            second
                .find_template("shared")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            100.0
        );
        assert_eq!(
            second
                .find_template("Untouched")
                .unwrap()
                .adjust_damage(DamageType::Explosion, 100.0),
            (30.0f32 / 100.0) * 100.0,
        );
        assert!(ArmorDefinitions::new().find_template("shared").is_none());
    }
}

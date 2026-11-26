////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! INI Template Loader
//!
//! Loads weapon, upgrade, and science templates from BIG archives at startup,
//! matching the C++ original's INI loading order:
//!
//! 1. Weapon INIs from `Data/INI/Weapon.ini`, `Data/INI/Default/Weapon.ini`,
//!    and `Data/INI/Weapon/`
//! 2. Upgrade INIs: `Data/INI/Default/Upgrade.ini` then `Data/INI/Upgrade.ini`
//!    (C++ GameEngine.cpp:468), parsed by the Common INI `Upgrade` block
//!    handler into this application's UpgradeCenter
//! 3. Science INIs from `Data/INI/Default/Science.ini` then `Data/INI/Science.ini`
//!
//! The weapon catalog is shared with receiving worlds. The legacy WeaponStore
//! receives the same parsed definitions for startup adapters. The application
//! also owns the UpgradeCenter and ScienceStore populated here.

use generals_sim::weapon::{WeaponCatalog, WeaponDefinition, apply_weapon_ini_fields};
use crate::assets::archive::ArchiveFileSystem;
use log::{debug, info, warn};
use std::collections::HashMap;

/// Result statistics from INI template loading.
#[derive(Debug, Default)]
pub struct IniTemplateLoadStats {
    pub weapons_loaded: usize,
    pub upgrades_loaded: usize,
    pub sciences_loaded: usize,
    pub weapon_files_processed: usize,
    pub upgrade_files_processed: usize,
    pub science_files_processed: usize,
    /// Effective native rules built from this archive load, not the ambient store.
    pub(crate) weapon_definitions: WeaponCatalog,
    /// Upgrade mask allocation belongs to the application that loaded the INIs.
    pub(crate) upgrade_definitions: Option<game_engine::common::system::upgrade::UpgradeCenter>,
    /// Effective science names, prerequisites and identities for this application.
    pub(crate) science_definitions: Option<game_engine::common::rts::science::ScienceStore>,
}

/// Parse a block of INI text into section headers and their key=value properties.
///
/// Handles the C&C Generals INI format:
/// ```text
/// BlockType BlockName
///   Key1 = Value1
///   Key2 = Value2
///   ; comment
/// End
/// ```
///
/// An ordered sequence of INI assignments.
///
/// Most consumers only need C++'s usual last-assignment-wins behavior and can
/// collapse this into a `HashMap`. Weapon veterancy fields are different:
/// `VeterancyFireFX`, `VeterancyProjectileExhaust`, and their OCL/detonation
/// equivalents may legitimately occur more than once in one block. Retaining
/// source order is therefore necessary to reproduce `Weapon.cpp`'s parser.
pub(crate) type OrderedIniProperties = Vec<(String, String)>;
/// CPP WeaponStore permits an existing-name override only in CREATE_OVERRIDES.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeaponDefinitionLoadType {
    Normal,
    CreateOverrides,
}

/// Parse a block of INI text without discarding repeated assignments.
///
/// Returns `(block_type, block_name, ordered_properties)` tuples. Keep this as
/// the source parser; `parse_ini_sections` below is the compatibility view for
/// callers that intentionally use last-assignment-wins properties.
fn parse_ini_sections_ordered(content: &str) -> Vec<(String, String, OrderedIniProperties)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections: Vec<(String, String, OrderedIniProperties)> = Vec::new();
    let mut current_type: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut current_props: OrderedIniProperties = Vec::new();
    let mut depth: u32 = 0;

    for line in &lines {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty()
            || trimmed.starts_with(';')
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
        {
            continue;
        }

        // Skip [Section] headers (e.g. [WeaponSystem], [ScienceSystem])
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            continue;
        }

        // Detect block headers: "Weapon", "Upgrade", "Science", "Object", etc.
        if is_ini_block_header(trimmed) && depth == 0 {
            // Save previous section if any
            if let (Some(t), Some(n)) = (current_type.take(), current_name.take()) {
                if !current_props.is_empty() {
                    sections.push((t, n, std::mem::take(&mut current_props)));
                }
            }

            if let Some((block_type, block_name)) = parse_block_header(trimmed) {
                current_type = Some(block_type);
                current_name = Some(block_name);
                current_props.clear();
                depth = 1;
            }
            continue;
        }

        // Track nested End keywords (e.g. WeaponSet blocks inside objects)
        if trimmed.eq_ignore_ascii_case("End") {
            if depth > 1 {
                // Nested block terminator - just decrease depth
                depth -= 1;
                continue;
            }
            // Top-level End - finalize the current section
            if depth == 1 {
                if let (Some(t), Some(n)) = (current_type.take(), current_name.take()) {
                    sections.push((t, n, std::mem::take(&mut current_props)));
                }
                depth = 0;
            }
            continue;
        }

        // Track nested block opens (Behavior, Draw, WeaponSet, etc.)
        if depth > 0 && is_nested_block_header(trimmed) {
            depth += 1;
            continue;
        }

        // Parse key = value inside a block
        if depth > 0 {
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim().to_string();
                let mut value = trimmed[eq_pos + 1..].trim().to_string();

                // Remove quotes
                if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value = value[1..value.len() - 1].to_string();
                }

                // Handle inline comments
                value = strip_inline_comment(&value).to_string();

                if !key.is_empty() {
                    current_props.push((key, value));
                }
            }
        }
    }

    // Handle last section if file doesn't end with End
    if let (Some(t), Some(n)) = (current_type.take(), current_name.take()) {
        if !current_props.is_empty() {
            sections.push((t, n, current_props));
        }
    }

    sections
}

/// Parse INI sections using the historical last-assignment-wins view.
///
/// Existing non-weapon loaders use this API. Weapon loading uses the ordered
/// parser above so duplicate veterancy fields retain their C++ semantics.
fn parse_ini_sections(content: &str) -> Vec<(String, String, HashMap<String, String>)> {
    parse_ini_sections_ordered(content)
        .into_iter()
        .map(|(block_type, block_name, ordered)| {
            let properties = ordered.into_iter().collect::<HashMap<_, _>>();
            (block_type, block_name, properties)
        })
        .collect()
}

/// Check if a line is a top-level INI block header like "Weapon", "Upgrade", "Science".
fn is_ini_block_header(line: &str) -> bool {
    let first_word = line.split_whitespace().next().unwrap_or("");
    matches!(
        first_word.to_lowercase().as_str(),
        "weapon" | "upgrade" | "science" | "object" | "childobject" | "objectreskin"
    )
}

/// Check if a line starts a nested block (e.g., "Behavior", "Draw", "WeaponSet").
fn is_nested_block_header(line: &str) -> bool {
    let first_word = line.split_whitespace().next().unwrap_or("");
    matches!(
        first_word.to_lowercase().as_str(),
        "behavior"
            | "body"
            | "draw"
            | "weaponset"
            | "armorset"
            | "locomotorset"
            | "contain"
            | "physics"
            | "sound"
            | "clientupdate"
            | "moduletag"
            | "conditionstate"
            | "transitionstate"
            | "anim"
            | "particlesystem"
            | "fxlist"
            | "objectcreationlist"
            | "script"
            | "playertemplate"
            | "commandset"
            | "commandbutton"
            | "specialpower"
    )
}

/// Parse a block header line into (type, name).
fn parse_block_header(line: &str) -> Option<(String, String)> {
    let mut tokens = line.split_whitespace();
    let block_type = tokens.next()?.to_string();
    let block_name = tokens.next()?.to_string();
    Some((block_type, block_name))
}

/// Strip inline comments from a value string.
fn strip_inline_comment(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b';' | b'#' if !in_single && !in_double => return value[..i].trim_end().to_string(),
            b'/' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return value[..i].trim_end().to_string();
            }
            _ => {}
        }
        i += 1;
    }

    value.to_string()
}

/// Normalize an archive path for discovery and lookup.
///
/// This keeps the original casing intact, but converts separators to `/`,
/// removes repeated separators, and trims leading `./` or `/` prefixes so
/// archive variants compare deterministically.
fn normalize_archive_path(path: &str) -> String {
    let mut normalized = String::with_capacity(path.len());
    let mut previous_was_slash = false;

    for ch in path.trim().chars() {
        let ch = if ch == '\\' { '/' } else { ch };
        if ch == '/' {
            if previous_was_slash {
                continue;
            }
            previous_was_slash = true;
        } else {
            previous_was_slash = false;
        }
        normalized.push(ch);
    }

    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    while let Some(stripped) = normalized.strip_prefix('/') {
        normalized = stripped.to_string();
    }

    normalized
}

/// Normalize an archive path and convert it into a case-insensitive key.
fn archive_path_key(path: &str) -> String {
    normalize_archive_path(path).to_ascii_lowercase()
}

/// Deduplicate archive paths case-insensitively while keeping deterministic order.
fn sort_and_dedup_archive_paths(paths: &mut Vec<String>) {
    paths.sort_by(|a, b| {
        let a_key = archive_path_key(a);
        let b_key = archive_path_key(b);
        a_key.cmp(&b_key).then_with(|| a.cmp(b))
    });
    paths.dedup_by(|a, b| archive_path_key(a) == archive_path_key(b));
}

/// Returns `true` if `path_key` exactly matches `target` or ends with `/{target}`.
fn archive_key_matches_suffix(path_key: &str, target: &str) -> bool {
    path_key == target || path_key.ends_with(&format!("/{}", target))
}

/// Returns `true` when a path should be treated as a weapon INI.
fn is_weapon_ini_path(path: &str) -> bool {
    let key = archive_path_key(path);
    archive_key_matches_suffix(&key, "data/ini/weapon.ini")
        || archive_key_matches_suffix(&key, "data/ini/default/weapon.ini")
        || ((key.starts_with("data/ini/weapon/") || key.contains("/data/ini/weapon/"))
            && key.ends_with(".ini"))
}

/// Discover weapon INI files from the archive system.
///
/// In the C++ original, weapon INIs are loaded from:
/// - `Data/INI/Weapon.ini` (main weapon definitions)
/// - `Data/INI/Default/Weapon.ini` (base weapon definitions)
/// - `Data/INI/Weapon/*.ini` (faction-specific weapon files)
fn discover_weapon_ini_files_from_paths<I>(all_files: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut discovered: Vec<String> = all_files
        .into_iter()
        .map(|path| normalize_archive_path(&path))
        .filter(|path| is_weapon_ini_path(path))
        .collect();

    sort_and_dedup_archive_paths(&mut discovered);
    discovered
}

fn discover_weapon_ini_files(archive_system: &ArchiveFileSystem) -> Vec<String> {
    discover_weapon_ini_files_from_paths(archive_system.list_all_files())
}

/// C++ GameEngine.cpp:468 loads `Default/Upgrade.ini` then `Upgrade.ini`, in
/// that fixed order (mask bits are assigned in first-definition order).
const UPGRADE_INI_LOAD_ORDER: [&str; 2] = ["data/ini/default/upgrade.ini", "data/ini/upgrade.ini"];

/// Resolve the archive paths for the upgrade INIs in C++ load order.
fn upgrade_ini_files_in_load_order(all_files: Vec<String>) -> Vec<String> {
    let normalized: Vec<String> = all_files
        .into_iter()
        .map(|path| normalize_archive_path(&path))
        .collect();
    UPGRADE_INI_LOAD_ORDER
        .iter()
        .filter_map(|suffix| {
            normalized
                .iter()
                .find(|path| archive_key_matches_suffix(&path.to_ascii_lowercase(), suffix))
                .cloned()
        })
        .collect()
}

/// Discover science INI files from the archive system.
///
/// C++ `GameEngine.cpp:398` loads:
/// - `Data/INI/Default/Science.ini` first
/// - `Data/INI/Science.ini` overwrite
fn discover_science_ini_files(archive_system: &ArchiveFileSystem) -> Vec<String> {
    discover_science_ini_files_from_paths(archive_system.list_all_files())
}

fn discover_science_ini_files_from_paths<I>(all_files: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut defaults = Vec::new();
    let mut overrides = Vec::new();

    for path in all_files {
        let normalized = normalize_archive_path(&path);
        let key = normalized.to_ascii_lowercase();
        if archive_key_matches_suffix(&key, "data/ini/default/science.ini") {
            defaults.push(normalized);
        } else if archive_key_matches_suffix(&key, "data/ini/science.ini") {
            overrides.push(normalized);
        }
    }

    sort_and_dedup_archive_paths(&mut defaults);
    sort_and_dedup_archive_paths(&mut overrides);
    defaults.into_iter().next().into_iter()
        .chain(overrides.into_iter().next())
        .collect()
}

/// Load weapon templates from BIG archives and register them in the GameLogic WeaponStore.
///
/// This follows the same pattern as WW3DAssetManager::initialize() for object INIs.
pub async fn load_weapon_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<usize, String> {
    load_weapon_templates_with_definitions(archive_system)
        .await
        .map(|(count, _)| count)
}

async fn load_weapon_templates_with_definitions(
    archive_system: &mut ArchiveFileSystem,
) -> Result<(usize, WeaponCatalog), String> {
    let weapon_files = discover_weapon_ini_files(archive_system);
    if weapon_files.is_empty() {
        info!("No weapon INI files found in archives");
        return Ok((0, WeaponCatalog::default()));
    }

    info!(
        "Loading weapon templates from {} INI files",
        weapon_files.len()
    );
    debug!(
        "Weapon INI discovery matched {} file(s)",
        weapon_files.len()
    );

    let mut total_weapons = 0usize;
    let mut definitions = HashMap::new();

    for (idx, ini_file) in weapon_files.iter().enumerate() {
        debug!(
            "Loading weapon INI file {}/{}: {}",
            idx + 1,
            weapon_files.len(),
            ini_file
        );

        let data = match archive_system.open_file(ini_file).await {
            Ok(d) => d,
            Err(e) => {
                debug!("Cannot open weapon INI {}: {}", ini_file, e);
                continue;
            }
        };

        let content = match String::from_utf8(data) {
            Ok(c) => c,
            Err(_) => {
                warn!("Failed to decode weapon INI {} as UTF-8", ini_file);
                continue;
            }
        };

        let rows = parse_weapon_definition_rows_into(
            &mut definitions,
            &content,
            WeaponDefinitionLoadType::Normal,
        );
        let file_weapon_count = rows.len();
        // Legacy publication uses the exact same outputs in original block
        // order; the owned catalogue never depends on registration success.
        let _ = gamelogic::initialize_weapon_store();
        for definition in rows {
            let _ = publish_weapon_template(gamelogic::weapon::WeaponTemplate::from_definition(
                std::sync::Arc::new(definition),
            ));
        }
        total_weapons += file_weapon_count;
        debug!(
            "Loaded {} weapon templates from {}",
            file_weapon_count, ini_file
        );
    }

    info!("Loaded {} weapon templates total", total_weapons);
    let catalog = WeaponCatalog::from_named_definitions(definitions)
        .map_err(|error| error.to_string())?;
    Ok((total_weapons, catalog))
}

/// Load `Default/Upgrade.ini` then `Upgrade.ini` from BIG archives into the
/// application's UpgradeCenter (C++ `TheUpgradeCenter`, GameEngine.cpp:468).
///
/// Every `Upgrade` block goes through the Common INI dispatch, i.e. C++
/// `UpgradeCenter::parseUpgradeDefinition` — the single upgrade load path
/// (engine boot and map.ini alike). Runs during engine setup, before any
/// GameLogic world receives the definitions. Returns the number of templates
/// the files added and their owner.
pub async fn load_upgrade_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<(usize, game_engine::common::system::upgrade::UpgradeCenter), String> {
    let mut center = game_engine::common::system::upgrade::UpgradeCenter::new();
    center.init();
    let count_before = center.count();

    let upgrade_files = upgrade_ini_files_in_load_order(archive_system.list_all_files());
    if upgrade_files.is_empty() {
        info!("No upgrade INI files found in archives");
        return Ok((0, center));
    }

    for ini_file in &upgrade_files {
        let data = match archive_system.open_file(ini_file).await {
            Ok(d) => d,
            Err(e) => {
                debug!("Cannot open upgrade INI {}: {}", ini_file, e);
                continue;
            }
        };
        let content = match String::from_utf8(data) {
            Ok(c) => c,
            Err(_) => {
                warn!("Failed to decode upgrade INI {} as UTF-8", ini_file);
                continue;
            }
        };
        if let Err(err) = load_upgrade_ini_text(&mut center, &content) {
            warn!("Failed parsing upgrade INI '{}': {}", ini_file, err);
        }
    }

    let total = center.count().saturating_sub(count_before);
    info!("Loaded {} upgrade templates total", total);
    Ok((total, center))
}

/// Parse one Upgrade.ini text into its caller's UpgradeCenter through the
/// Common INI `Upgrade` block handler. Partial updates survive an error, as
/// they do when C++ parses directly into the receiving center.
pub fn load_upgrade_ini_text(
    center: &mut game_engine::common::system::upgrade::UpgradeCenter,
    content: &str,
) -> Result<(), String> {
    let mut ini = game_engine::common::ini::INI::new();
    ini.set_owned_upgrade_center_target(std::mem::take(center));
    let result = ini
        .with_inline_source(content, |ini| ini.parse_current_file())
        .map_err(|err| err.to_string());
    *center = ini
        .take_owned_upgrade_center_target()
        .expect("upgrade parser owns its center");
    result
}

/// Load science templates from BIG archives in C++ source order.
pub async fn load_science_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<(usize, game_engine::common::rts::science::ScienceStore), String> {
    let science_files = discover_science_ini_files(archive_system);
    let mut definitions = game_engine::common::rts::science::ScienceStore::new_owned();
    if science_files.is_empty() {
        info!("No science INI files found in archives");
        return Ok((0, definitions));
    }


    info!(
        "Loading science templates from {} INI files",
        science_files.len()
    );

    let mut source_bytes = Vec::new();

    for (idx, ini_file) in science_files.iter().enumerate() {
        debug!(
            "Loading science INI file {}/{}: {}",
            idx + 1,
            science_files.len(),
            ini_file
        );

        let data = match archive_system.open_file(ini_file).await {
            Ok(d) => d,
            Err(e) => {
                debug!("Cannot open science INI {}: {}", ini_file, e);
                continue;
            }
        };

        let content = match String::from_utf8(data) {
            Ok(c) => c,
            Err(_) => {
                warn!("Failed to decode science INI {} as UTF-8", ini_file);
                continue;
            }
        };

        let override_file = normalize_archive_path(ini_file)
            .to_ascii_lowercase()
            .ends_with("data/ini/science.ini");
        source_bytes.push((ini_file.clone(), content.as_bytes().to_vec(), override_file));

        debug!("Read science definitions from {}", ini_file);
    }

    let sources = source_bytes
        .iter()
        .map(|(path, bytes, is_override)| (path.as_str(), bytes.as_slice(), *is_override))
        .collect::<Vec<_>>();
    definitions
        .load_from_sources(&sources)
        .map_err(|error| error.to_string())?;
    let total_sciences = definitions.iter().count();
    info!("Loaded {} science templates total", total_sciences);
    Ok((total_sciences, definitions))
}

/// Load all INI templates (weapons, upgrades, sciences) from BIG archives.
///
/// This is the main entry point called during asset manager initialization,
/// right after BIG archives are loaded and before the game logic starts.
pub async fn load_all_ini_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<IniTemplateLoadStats, String> {
    info!("=== Loading INI templates from BIG archives ===");

    let mut stats = IniTemplateLoadStats::default();

    // 1. Load weapons first (objects may reference weapon names)
    match load_weapon_templates_with_definitions(archive_system).await {
        Ok((count, definitions)) => {
            stats.weapons_loaded = count;
            stats.weapon_definitions = definitions;
        }
        Err(e) => {
            warn!("Weapon template loading failed: {}", e);
        }
    }

    // 2. Load upgrades (depend on sciences for prerequisites)
    match load_upgrade_templates(archive_system).await {
        Ok((count, definitions)) => {
            stats.upgrades_loaded = count;
            stats.upgrade_definitions = Some(definitions);
        }
        Err(e) => {
            warn!("Upgrade template loading failed: {}", e);
        }
    }

    // 3. Load sciences (foundational - should be loaded before upgrades ideally,
    //    but C++ loads them in a specific order and sciences are resolved later)
    match load_science_templates(archive_system).await {
        Ok((count, definitions)) => {
            stats.sciences_loaded = count;
            stats.science_definitions = Some(definitions);
        }
        Err(e) => {
            warn!("Science template loading failed: {}", e);
        }
    }

    // Post-process: initialize science store (resolve root sciences)
    {
        let mut store = game_engine::common::ini::ini_science::get_science_store_mut();
        store.init();
    }

    info!(
        "=== INI template loading complete: {} weapons, {} upgrades, {} sciences ===",
        stats.weapons_loaded, stats.upgrades_loaded, stats.sciences_loaded
    );

    Ok(stats)
}

/// Register all `Weapon` blocks from raw INI text into the GameLogic WeaponStore.
///
/// Used by AssetManager (archive path) and host bootstrap (filesystem path).
/// Returns the number of successfully registered templates.
/// Existing synthetic/map adapters historically re-declared names as overrides.
/// Shipping archive admission uses explicit Normal mode and never calls this.
pub fn register_weapons_from_ini_text(content: &str) -> usize {
    register_weapons_from_ini_text_with_load_type(
        content,
        WeaponDefinitionLoadType::CreateOverrides,
    )
}

pub(crate) fn register_weapons_from_ini_text_with_load_type(
    content: &str,
    load_type: WeaponDefinitionLoadType,
) -> usize {
    let register = || register_weapons_from_ini_text_with_load_type_on_active(content, load_type);
    if gamelogic::system::engine_stores::try_active_services().is_some() {
        register()
    } else {
        // Free/test admission: temporary WorldServices so WeaponStore cell binds
        // without requiring each caller to wrap.
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, register)
    }
}

/// Register weapons then run `f` while the admitting WorldServices stay bound.
/// Prefer this over `register_weapons_from_ini_text` + a later free `with_weapon_store`
/// when the caller needs to read the store it just filled.
pub fn with_weapons_from_ini_text<R>(content: &str, f: impl FnOnce(usize) -> R) -> R {
    let run = || {
        let n = register_weapons_from_ini_text_with_load_type_on_active(
            content,
            WeaponDefinitionLoadType::CreateOverrides,
        );
        f(n)
    };
    if gamelogic::system::engine_stores::try_active_services().is_some() {
        run()
    } else {
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, run)
    }
}

fn register_weapons_from_ini_text_with_load_type_on_active(
    content: &str,
    load_type: WeaponDefinitionLoadType,
) -> usize {
    // Ensure store exists before registration attempts.
    if let Err(e) = gamelogic::initialize_weapon_store() {
        warn!("Cannot register weapons — WeaponStore init failed: {e}");
        return 0;
    }
    // Weapon.cpp parses fields in source order. In particular, several
    // `Veterancy*` fields are deliberately repeated in a single Weapon block,
    // so do not pass this through the last-assignment-wins compatibility view.
    let sections = parse_ini_sections_ordered(content);
    let mut count = 0usize;
    for (block_type, block_name, properties) in &sections {
        if block_type.eq_ignore_ascii_case("Weapon") {
            let exists = gamelogic::with_weapon_store(|store| {
                store.find_weapon_template_ci(block_name).is_some()
            })
            .unwrap_or(false);
            if load_type == WeaponDefinitionLoadType::Normal && exists {
                warn!(
                    "Weapon '{}' already exists, but CREATE_OVERRIDES not specified",
                    block_name
                );
                continue;
            }
            if register_weapon_template_ordered(block_name, properties) {
                count += 1;
            }
        }
    }
    count
}

/// Register a weapon template from the historical last-assignment-wins view.
///
/// This stays available for focused callers/tests. Real Weapon.ini loading uses
/// `register_weapon_template_ordered` so repeated veterancy properties survive.
fn register_weapon_template(name: &str, properties: &HashMap<String, String>) -> bool {
    let ordered = properties
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<OrderedIniProperties>();
    register_weapon_template_ordered(name, &ordered)
}

/// Register a Weapon block while preserving every assignment in source order.
fn register_weapon_template_ordered(name: &str, ordered: &OrderedIniProperties) -> bool {
    let mut template = gamelogic::with_weapon_store(|store| {
        store
            .find_weapon_template_ci(name)
            .map(|old| (**old).clone())
    })
    .ok()
    .flatten()
    .unwrap_or_else(|| gamelogic::weapon::WeaponTemplate::new(name.to_owned()));
    let changes = apply_weapon_ini_fields(&mut template, ordered);
    if changes.clears_historic_bonus_weapon {
        template.historic_bonus_weapon = None;
    }
    for index in 0..4 {
        if changes.fire_ocl[index] {
            template.fire_ocl[index] = None;
        }
        if changes.projectile_detonation_ocl[index] {
            template.projectile_detonation_ocl[index] = None;
        }
        if changes.projectile_exhaust[index] {
            template.projectile_exhaust[index] = None;
        }
    }
    publish_weapon_template(template)
}

fn publish_weapon_template(template: gamelogic::weapon::WeaponTemplate) -> bool {
    let name = template.name.clone();
    match gamelogic::with_weapon_store_mut(|store| {
        store.add_weapon_template(template);
    }) {
        Ok(()) => {
            debug!("Registered weapon template: {}", name);
            true
        }
        Err(error) => {
            warn!("Failed to register weapon '{}': {}", name, error);
            false
        }
    }
}

/// Source/file order controls effective definitions; sorting is only for export.
pub(crate) fn parse_weapon_definitions_ini_into(
    definitions: &mut HashMap<String, WeaponDefinition>,
    content: &str,
    load_type: WeaponDefinitionLoadType,
) -> usize {
    parse_weapon_definition_rows_into(definitions, content, load_type).len()
}

fn parse_weapon_definition_rows_into(
    definitions: &mut HashMap<String, WeaponDefinition>,
    content: &str,
    load_type: WeaponDefinitionLoadType,
) -> Vec<WeaponDefinition> {
    let mut rows = Vec::new();
    for (kind, name, ordered) in parse_ini_sections_ordered(content) {
        if !kind.eq_ignore_ascii_case("Weapon") || name.trim().is_empty() {
            continue;
        }
        let key = name.trim().to_ascii_lowercase();
        if load_type == WeaponDefinitionLoadType::Normal && definitions.contains_key(&key) {
            // CPP Weapon.cpp1689: DEBUG_CRASH then return without changing it.
            warn!(
                "Weapon '{}' already exists, but CREATE_OVERRIDES not specified",
                name
            );
            continue;
        }

        let mut template = definitions.get(&key).cloned()
            .unwrap_or_else(|| WeaponDefinition::new(name));
        apply_weapon_ini_fields(&mut template, &ordered);
        definitions.insert(key, template.clone());
        rows.push(template);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_weapon_section() {
        let ini_content = r#"
; Weapon definitions
Weapon AmericaTankCrushWeapon
  DamageType = Crush
  AttackRange = 5.0
  PrimaryDamage = 100.0
  AntiGround = Yes
  AntiAirborneVehicle = No
  MinDelayBetweenShots = 500
  MaxDelayBetweenShots = 500
  ClipSize = 0
  WeaponSpeed = 999999.0
End

Weapon AmericaVehicleHumveeGunWeapon
  DamageType = Small_Arms
  AttackRange = 150.0
  PrimaryDamage = 10.0
  AntiGround = Yes
  AntiAirborneVehicle = Yes
  MinDelayBetweenShots = 100
  MaxDelayBetweenShots = 200
  ClipSize = 0
  ProjectileTemplate = AmericaVehicleHumveeBullet
End
"#;

        let sections = parse_ini_sections(ini_content);
        assert_eq!(sections.len(), 2);

        assert_eq!(sections[0].0, "Weapon");
        assert_eq!(sections[0].1, "AmericaTankCrushWeapon");
        assert_eq!(sections[0].2.get("DamageType").unwrap(), "Crush");
        assert_eq!(sections[0].2.get("AttackRange").unwrap(), "5.0");
        assert_eq!(sections[0].2.get("PrimaryDamage").unwrap(), "100.0");

        assert_eq!(sections[1].0, "Weapon");
        assert_eq!(sections[1].1, "AmericaVehicleHumveeGunWeapon");
        assert_eq!(
            sections[1].2.get("ProjectileTemplate").unwrap(),
            "AmericaVehicleHumveeBullet"
        );
    }


    #[test]
    fn weapon_anti_mask_honors_no_and_every_cxx_target_category() {
        let ini_content = r#"
Weapon __RustExactWeaponAntiMask
  AntiGround = No
  AntiAirborneVehicle = Yes
  AntiAirborneInfantry = Yes
  AntiProjectile = Yes
  AntiSmallMissile = Yes
  AntiMine = Yes
  AntiParachute = Yes
  AntiBallisticMissile = Yes
End
"#;

        with_weapons_from_ini_text(ini_content, |n| {
            assert_eq!(n, 1);
            gamelogic::with_weapon_store(|store| {
            use generals_sim::weapon::WeaponAntiMask;

            let weapon = store
                .find_weapon_template("__RustExactWeaponAntiMask")
                .expect("registered exact-mask weapon");
            let mask = weapon.get_anti_mask();
            assert_eq!(mask & WeaponAntiMask::GROUND, 0);
            for bit in [
                WeaponAntiMask::AIRBORNE_VEHICLE,
                WeaponAntiMask::AIRBORNE_INFANTRY,
                WeaponAntiMask::PROJECTILE,
                WeaponAntiMask::SMALL_MISSILE,
                WeaponAntiMask::MINE,
                WeaponAntiMask::PARACHUTE,
                WeaponAntiMask::BALLISTIC_MISSILE,
            ] {
                assert_ne!(mask & bit, 0, "missing parsed anti-mask bit {bit:#x}");
            }
            })
            .expect("weapon store available");
        });
    }

    #[test]
    fn damage_status_type_parses_faerie_fire_bit() {
        let ini_content = r#"
Weapon __RustDamageStatusFaerie
  AttackRange = 200.0
  PrimaryDamage = 200.0
  DamageType = STATUS
  DamageStatusType = FAERIE_FIRE
End
"#;
        with_weapons_from_ini_text(ini_content, |n| {
            assert_eq!(n, 1);
            gamelogic::with_weapon_store(|store| {
                let weapon = store
                    .find_weapon_template("__RustDamageStatusFaerie")
                    .expect("registered status weapon");
                let status: gamelogic::common::ObjectStatusTypes = weapon.damage_status_type.into();
                assert_eq!(status, gamelogic::common::ObjectStatusTypes::FaerieFire);
            })
            .expect("weapon store available");
        });
    }

    #[test]
    fn weapon_shots_per_barrel_is_retained_from_authored_ini() {
        let ini_content = r#"
Weapon __RustShotsPerBarrelExact
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 3
End
"#;

        with_weapons_from_ini_text(ini_content, |n| {
            assert_eq!(n, 1);
            gamelogic::with_weapon_store(|store| {
                assert_eq!(
                    store
                        .find_weapon_template("__RustShotsPerBarrelExact")
                        .expect("registered exact barrel weapon")
                        .shots_per_barrel,
                    3
                );
            })
            .expect("weapon store available");
        });
    }

    #[test]
    fn redeclared_weapon_ini_merges_instead_of_replacing() {
        // C++ parseWeaponTemplateDefinition found-or-create then initFromINI.
        // A later partial block must keep unlisted fields.
        let first = r#"
Weapon __RustWeaponMergeBase
  AttackRange = 150.0
  PrimaryDamage = 100.0
  SecondaryDamage = 25.0
  ClipSize = 8
  ShotsPerBarrel = 2
End
"#;
        let overlay = r#"
Weapon __RustWeaponMergeBase
  PrimaryDamage = 40.0
  ClipSize = 4
End
"#;
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, || {
            assert_eq!(register_weapons_from_ini_text(first), 1);
            assert_eq!(register_weapons_from_ini_text(overlay), 1);
            gamelogic::with_weapon_store(|store| {
                let weapon = store
                    .find_weapon_template("__RustWeaponMergeBase")
                    .expect("merged weapon still registered");
                assert_eq!(weapon.primary_damage, 40.0);
                assert_eq!(weapon.clip_size, 4);
                assert_eq!(weapon.attack_range, 150.0);
                assert_eq!(weapon.secondary_damage, 25.0);
                assert_eq!(weapon.shots_per_barrel, 2);
            })
            .expect("weapon store available");
        });
    }

    #[test]
    fn weapon_effect_references_preserve_repeated_veterancy_lines_in_order() {
        // Mirrors Weapon.cpp's `parseAllVetLevels*` then repeated
        // `parsePerVetLevel*` handlers. A HashMap-only parser would keep only
        // the final repeated assignment and lose the Veteran/Elite overrides.
        let ini_content = r#"
Weapon __RustOrderedVeterancyEffectWeapon
  FireFX = FX_BaseFire
  VeterancyFireFX = VETERAN FX_VeteranFire
  VeterancyFireFX = ELITE None
  VeterancyFireFX = HEROIC FX_HeroicFire
  ProjectileDetonationFX = FX_BaseDetonate
  VeterancyProjectileDetonationFX = VETERAN FX_VeteranDetonate
  VeterancyProjectileDetonationFX = HEROIC FX_HeroicDetonate
  FireOCL = OCL_BaseFire
  VeterancyFireOCL = VETERAN OCL_VeteranFire
  VeterancyFireOCL = ELITE None
  VeterancyFireOCL = HEROIC OCL_HeroicFire
  ProjectileDetonationOCL = OCL_BaseDetonate
  VeterancyProjectileDetonationOCL = VETERAN OCL_VeteranDetonate
  VeterancyProjectileDetonationOCL = ELITE None
  ProjectileExhaust = Exhaust_Base
  VeterancyProjectileExhaust = VETERAN Exhaust_Veteran
  VeterancyProjectileExhaust = ELITE None
  VeterancyProjectileExhaust = HEROIC Exhaust_Heroic
  PlayFXWhenStealthed = Yes
End
"#;

        let ordered = parse_ini_sections_ordered(ini_content);
        assert_eq!(ordered.len(), 1);
        assert_eq!(
            ordered[0]
                .2
                .iter()
                .filter(|(key, _)| key.eq_ignore_ascii_case("VeterancyFireFX"))
                .count(),
            3,
            "repeated Weapon.ini properties must survive section parsing"
        );

        with_weapons_from_ini_text(ini_content, |n| {
            assert_eq!(n, 1);
            gamelogic::with_weapon_store(|store| {
            let weapon = store
                .find_weapon_template("__RustOrderedVeterancyEffectWeapon")
                .expect("ordered weapon registered");

            assert_eq!(
                weapon.fire_fx[0].as_ref().map(|fx| fx.name()),
                Some("FX_BaseFire")
            );
            assert_eq!(
                weapon.fire_fx[1].as_ref().map(|fx| fx.name()),
                Some("FX_VeteranFire")
            );
            assert!(weapon.fire_fx[2].is_none(), "ELITE None is a null FXList");
            assert_eq!(
                weapon.fire_fx[3].as_ref().map(|fx| fx.name()),
                Some("FX_HeroicFire")
            );

            assert_eq!(
                weapon.projectile_detonate_fx[0]
                    .as_ref()
                    .map(|fx| fx.name()),
                Some("FX_BaseDetonate")
            );
            assert_eq!(
                weapon.projectile_detonate_fx[1]
                    .as_ref()
                    .map(|fx| fx.name()),
                Some("FX_VeteranDetonate")
            );
            assert_eq!(
                weapon.projectile_detonate_fx[2]
                    .as_ref()
                    .map(|fx| fx.name()),
                Some("FX_BaseDetonate")
            );
            assert_eq!(
                weapon.projectile_detonate_fx[3]
                    .as_ref()
                    .map(|fx| fx.name()),
                Some("FX_HeroicDetonate")
            );

            use gamelogic::common::VeterancyLevel::{Elite, Heroic, Regular, Veteran};
            assert_eq!(weapon.get_fire_ocl_name(Regular), Some("OCL_BaseFire"));
            assert_eq!(weapon.get_fire_ocl_name(Veteran), Some("OCL_VeteranFire"));
            assert_eq!(weapon.get_fire_ocl_name(Elite), None);
            assert_eq!(weapon.get_fire_ocl_name(Heroic), Some("OCL_HeroicFire"));
            assert_eq!(
                weapon.get_projectile_detonation_ocl_name(Regular),
                Some("OCL_BaseDetonate")
            );
            assert_eq!(
                weapon.get_projectile_detonation_ocl_name(Veteran),
                Some("OCL_VeteranDetonate")
            );
            assert_eq!(weapon.get_projectile_detonation_ocl_name(Elite), None);
            assert_eq!(
                weapon.get_projectile_detonation_ocl_name(Heroic),
                Some("OCL_BaseDetonate")
            );

            assert_eq!(
                weapon.get_projectile_exhaust_name(Regular),
                Some("Exhaust_Base")
            );
            assert_eq!(
                weapon.get_projectile_exhaust_name(Veteran),
                Some("Exhaust_Veteran")
            );
            assert_eq!(weapon.get_projectile_exhaust_name(Elite), None);
            assert_eq!(
                weapon.get_projectile_exhaust_name(Heroic),
                Some("Exhaust_Heroic")
            );
            assert!(weapon.play_fx_when_stealthed);
            })
            .expect("weapon store available");
        });
    }

    #[test]
    fn test_parse_upgrade_section() {
        let ini_content = r#"
Upgrade AmericaTankCompositeArmor
  DisplayName = "LOC:Upgrade_CompositeArmor"
  BuildTime = 20.0
  Cost = 2000
  ResearchSound = ComancheUpgrade
End
"#;

        let sections = parse_ini_sections(ini_content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Upgrade");
        assert_eq!(sections[0].1, "AmericaTankCompositeArmor");
    }

    fn owned_upgrade(
        center: &game_engine::common::system::upgrade::UpgradeCenter,
        name: &str,
    ) -> std::sync::Arc<gamelogic::upgrade::UpgradeTemplate> {
        center.find_upgrade(name).expect("upgrade registered")
    }

    #[test]
    fn upgrade_ini_text_copies_button_image() {
        let mut center = game_engine::common::system::upgrade::UpgradeCenter::new();
        center.init();
        load_upgrade_ini_text(
            &mut center,
            "Upgrade Upgrade_TestButtonImageCameo\n  ButtonImage = SSTestCameo\nEnd\n",
        )
        .expect("canonical Upgrade block");
        let template = owned_upgrade(&center, "Upgrade_TestButtonImageCameo");
        assert_eq!(template.get_button_image_name().as_str(), "SSTestCameo");
    }

    #[test]
    fn upgrade_ini_override_overlays_existing_template() {
        let name = "Upgrade_TestButtonImageCameoExisting";
        let mut center = game_engine::common::system::upgrade::UpgradeCenter::new();
        center.init();

        // First block creates the template (C++ newUpgrade) and sets a field.
        load_upgrade_ini_text(
            &mut center,
            &format!("Upgrade {name}\n  BuildTime = 15.0\nEnd\n"),
        )
        .unwrap();
        let first = owned_upgrade(&center, name);
        assert!(first.get_button_image_name().as_str().is_empty());

        // C++ initFromINI on the existing template: overlay, same mask bit.
        load_upgrade_ini_text(
            &mut center,
            &format!(
                "Upgrade {name}\n  ButtonImage = SSTestCameo\n  DisplayName = CONTROLBAR:TestCameo\n  BuildCost = 500\nEnd\n"
            ),
        )
        .unwrap();
        let template = owned_upgrade(&center, name);
        assert_eq!(template.get_button_image_name().as_str(), "SSTestCameo");
        assert_eq!(template.get_display_name().as_str(), "CONTROLBAR:TestCameo");
        assert_eq!(template.get_cost(), 500);
        assert_eq!(template.get_build_time(), 15.0, "earlier field kept");
        assert_eq!(template.get_mask(), first.get_mask(), "no new bit");
    }

    #[test]
    fn upgrade_ini_files_load_default_before_override_regardless_of_case() {
        // A case-sensitive sort would put "Data/INI/Upgrade.ini" before
        // "data/ini/default/upgrade.ini"; C++ always loads Default first.
        let files = upgrade_ini_files_in_load_order(vec![
            "Data/INI/Upgrade.ini".to_string(),
            "Data/INI/Weapon.ini".to_string(),
            "data/ini/default/upgrade.ini".to_string(),
        ]);
        assert_eq!(files.len(), 2);
        assert!(
            files[0]
                .to_ascii_lowercase()
                .ends_with("default/upgrade.ini")
        );
        assert!(files[1].to_ascii_lowercase().ends_with("ini/upgrade.ini"));
        assert!(!files[1].to_ascii_lowercase().contains("default"));
    }

    #[test]
    fn test_parse_science_section() {
        let ini_content = r#"
Science Science_Patriotism
  DisplayName = "LOC:ScienceName_Patriotism"
  Description = "LOC:ScienceDesc_Patriotism"
  SciencePurchasePointCost = 1
  PrerequisiteSciences = Science_Superweapon
  IsGrantable = Yes
End

Science Science_Superweapon
  DisplayName = "LOC:ScienceName_Superweapon"
  Description = "LOC:ScienceDesc_Superweapon"
  SciencePurchasePointCost = 3
  PrerequisiteSciences = None
  IsGrantable = Yes
End
"#;

        let sections = parse_ini_sections(ini_content);
        assert_eq!(sections.len(), 2);

        assert_eq!(sections[0].0, "Science");
        assert_eq!(sections[0].1, "Science_Patriotism");
        assert_eq!(
            sections[0].2.get("PrerequisiteSciences").unwrap(),
            "Science_Superweapon"
        );

        assert_eq!(sections[1].0, "Science");
        assert_eq!(sections[1].1, "Science_Superweapon");
        assert_eq!(sections[1].2.get("PrerequisiteSciences").unwrap(), "None");
    }

    #[test]
    fn test_parse_inline_comments() {
        let ini_content = r#"
Weapon TestWeapon
  DamageType = Explosion ; this is a comment
  AttackRange = 100.0 // another comment
  PrimaryDamage = 50.0
End
"#;

        let sections = parse_ini_sections(ini_content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].2.get("DamageType").unwrap(), "Explosion");
        assert_eq!(sections[0].2.get("AttackRange").unwrap(), "100.0");
    }


    #[test]
    fn test_clip_reload_and_pre_attack_use_frame_conversion() {
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, || {
            // Register a weapon with msec durations; store must hold logic frames.
            let _ = gamelogic::initialize_weapon_store();
            let mut props = HashMap::new();
            props.insert("ClipReloadTime".into(), "2000".into()); // 2000ms → 60 frames
            props.insert("PreAttackDelay".into(), "500".into()); // 500ms → 15 frames
            props.insert("PrimaryDamage".into(), "10.0".into());
            props.insert("AttackRange".into(), "100.0".into());
            assert!(register_weapon_template("TestClipReloadWeapon", &props));

            let frames = gamelogic::with_weapon_store(|store| {
                let t = store
                    .find_weapon_template("TestClipReloadWeapon")
                    .expect("template registered");
                (t.clip_reload_time, t.pre_attack_delay)
            })
            .expect("weapon store available");
            assert_eq!(frames.0, 60, "ClipReloadTime 2000ms → 60 frames");
            assert_eq!(frames.1, 15, "PreAttackDelay 500ms → 15 frames");
        });
    }

    #[test]
    fn test_is_ini_block_header() {
        assert!(is_ini_block_header("Weapon TestWeapon"));
        assert!(is_ini_block_header("Upgrade TestUpgrade"));
        assert!(is_ini_block_header("Science TestScience"));
        assert!(is_ini_block_header("Object TestObject"));
        assert!(!is_ini_block_header("Behavior AIUpdate"));
        assert!(!is_ini_block_header("WeaponSet"));
    }

    #[test]
    fn test_strip_inline_comment() {
        assert_eq!(strip_inline_comment("Explosion ; comment"), "Explosion");
        assert_eq!(strip_inline_comment("100.0 // another comment"), "100.0");
        assert_eq!(strip_inline_comment("NoComment"), "NoComment");
    }

    #[test]
    fn test_normalize_archive_path() {
        assert_eq!(
            normalize_archive_path(r".\Data\\INI\Weapon.ini"),
            "Data/INI/Weapon.ini"
        );
        assert_eq!(
            normalize_archive_path(r"//Data/INI//Default\\Weapon.ini"),
            "Data/INI/Default/Weapon.ini"
        );
    }

    #[test]
    fn test_discover_weapon_ini_files_includes_canonical_variants() {
        let files = vec![
            r".\Data\INI\Default\Weapon.ini".to_string(),
            r"Data\INI\Weapon.ini".to_string(),
            r"INIZH\Data\INI\Weapon.ini".to_string(),
            r"Data/INI/Weapon/America.ini".to_string(),
            r"INIZH/Data/INI/Weapon/China.ini".to_string(),
            r"Data\INI\Weapon\alpha.ini".to_string(),
            r"Data/INI/Weapon\alpha.ini".to_string(),
            r"Data/INI/NotWeapon.ini".to_string(),
        ];

        let discovered = discover_weapon_ini_files_from_paths(files);

        assert_eq!(
            discovered,
            vec![
                "Data/INI/Default/Weapon.ini".to_string(),
                "Data/INI/Weapon.ini".to_string(),
                "Data/INI/Weapon/alpha.ini".to_string(),
                "Data/INI/Weapon/America.ini".to_string(),
                "INIZH/Data/INI/Weapon.ini".to_string(),
                "INIZH/Data/INI/Weapon/China.ini".to_string(),
            ]
        );
    }

    #[test]
    fn discover_science_ini_loads_default_then_override() {
        let discovered = discover_science_ini_files_from_paths(vec![
            r"Data\INI\Science.ini".to_string(),
            r"INIZH\Data\INI\Default\Science.ini".to_string(),
            "Data/INI/Default/Science.ini".to_string(),
            "Data/INI/Object.ini".to_string(),
        ]);
        assert_eq!(
            discovered,
            vec![
                "Data/INI/Default/Science.ini".to_string(),
                "Data/INI/Science.ini".to_string(),
            ]
        );
    }


    #[test]
    fn test_nested_blocks_ignored() {
        let ini_content = r#"
Weapon TestWeapon
  DamageType = Explosion
  AttackRange = 100.0
  PrimaryDamage = 50.0
  WeaponSet
    Weapon = TestWeapon
  End
End
"#;

        let sections = parse_ini_sections(ini_content);
        // Should only have the top-level Weapon section
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "Weapon");
        assert_eq!(sections[0].2.get("DamageType").unwrap(), "Explosion");
    }
}

#[cfg(test)]
#[path = "weapon_definition_tests.rs"]
mod weapon_definition_tests;

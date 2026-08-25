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
//! 2. Upgrade INIs from `Data/INI/Default/Upgrade.ini`
//! 3. Science INIs from `Data/INI/Default/Science.ini` then `Data/INI/Science.ini`
//!
//! These templates are registered into the GameLogic WeaponStore, the
//! GameLogic UpgradeCenter, and the Common ScienceStore respectively.

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
type OrderedIniProperties = Vec<(String, String)>;

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

/// Discover upgrade INI files from the archive system.
///
/// In the C++ original, upgrade INIs are loaded from:
/// - `Data/INI/Default/Upgrade.ini`
fn discover_upgrade_ini_files(archive_system: &ArchiveFileSystem) -> Vec<String> {
    let all_files = archive_system.list_all_files();

    let mut discovered: Vec<String> = all_files
        .into_iter()
        .map(|path| normalize_archive_path(&path))
        .filter(|path| {
            let normalized = path.to_ascii_lowercase();
            archive_key_matches_suffix(&normalized, "data/ini/default/upgrade.ini")
                || archive_key_matches_suffix(&normalized, "data/ini/upgrade.ini")
        })
        .collect();

    discovered.sort();
    discovered.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    discovered
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
    defaults.extend(overrides);
    defaults
}

/// Load weapon templates from BIG archives and register them in the GameLogic WeaponStore.
///
/// This follows the same pattern as WW3DAssetManager::initialize() for object INIs.
pub async fn load_weapon_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<usize, String> {
    let weapon_files = discover_weapon_ini_files(archive_system);
    if weapon_files.is_empty() {
        info!("No weapon INI files found in archives");
        return Ok(0);
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

        let file_weapon_count = register_weapons_from_ini_text(&content);
        total_weapons += file_weapon_count;
        debug!(
            "Loaded {} weapon templates from {}",
            file_weapon_count, ini_file
        );
    }

    info!("Loaded {} weapon templates total", total_weapons);
    Ok(total_weapons)
}

/// Load upgrade templates from BIG archives and register them in the GameLogic UpgradeCenter.
pub async fn load_upgrade_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<usize, String> {
    // C++ UpgradeCenter::init creates Upgrade_Veterancy_* before Upgrade.ini.
    gamelogic::upgrade::center::with_upgrade_center_mut(|center| {
        center.init();
    });

    let upgrade_files = discover_upgrade_ini_files(archive_system);
    if upgrade_files.is_empty() {
        info!("No upgrade INI files found in archives");
        return Ok(0);
    }

    info!(
        "Loading upgrade templates from {} INI files",
        upgrade_files.len()
    );

    let mut total_upgrades = 0usize;

    for (idx, ini_file) in upgrade_files.iter().enumerate() {
        debug!(
            "Loading upgrade INI file {}/{}: {}",
            idx + 1,
            upgrade_files.len(),
            ini_file
        );

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

        let sections = parse_ini_sections(&content);
        let mut file_upgrade_count = 0usize;

        for (block_type, block_name, properties) in &sections {
            if block_type.eq_ignore_ascii_case("Upgrade")
                && register_upgrade_template(block_name, properties)
            {
                file_upgrade_count += 1;
            }
        }

        total_upgrades += file_upgrade_count;
        debug!(
            "Loaded {} upgrade templates from {}",
            file_upgrade_count, ini_file
        );
    }

    info!("Loaded {} upgrade templates total", total_upgrades);
    Ok(total_upgrades)
}

/// Load science templates from BIG archives and register them in the Common ScienceStore.
pub async fn load_science_templates(
    archive_system: &mut ArchiveFileSystem,
) -> Result<usize, String> {
    let science_files = discover_science_ini_files(archive_system);
    if science_files.is_empty() {
        info!("No science INI files found in archives");
        return Ok(0);
    }

    info!(
        "Loading science templates from {} INI files",
        science_files.len()
    );

    let mut total_sciences = 0usize;

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

        let sections = parse_ini_sections(&content);
        let mut file_science_count = 0usize;

        for (block_type, block_name, properties) in &sections {
            if block_type.eq_ignore_ascii_case("Science")
                && register_science_template(block_name, properties)
            {
                file_science_count += 1;
            }
        }

        total_sciences += file_science_count;
        debug!(
            "Loaded {} science templates from {}",
            file_science_count, ini_file
        );
    }

    info!("Loaded {} science templates total", total_sciences);
    Ok(total_sciences)
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
    match load_weapon_templates(archive_system).await {
        Ok(count) => {
            stats.weapons_loaded = count;
        }
        Err(e) => {
            warn!("Weapon template loading failed: {}", e);
        }
    }

    // 2. Load upgrades (depend on sciences for prerequisites)
    match load_upgrade_templates(archive_system).await {
        Ok(count) => {
            stats.upgrades_loaded = count;
        }
        Err(e) => {
            warn!("Upgrade template loading failed: {}", e);
        }
    }

    // 3. Load sciences (foundational - should be loaded before upgrades ideally,
    //    but C++ loads them in a specific order and sciences are resolved later)
    match load_science_templates(archive_system).await {
        Ok(count) => {
            stats.sciences_loaded = count;
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
pub fn register_weapons_from_ini_text(content: &str) -> usize {
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
        if block_type.eq_ignore_ascii_case("Weapon")
            && register_weapon_template_ordered(block_name, properties)
        {
            count += 1;
        }
    }
    count
}

/// C++ ConvertDurationFromMsecsToFrames (logic clock at 30 FPS).
fn msec_to_logic_frames(msec: i32) -> i32 {
    if msec <= 0 {
        return 0;
    }
    // ceil(msec * FPS / 1000)
    ((msec as i64 * 30 + 999) / 1000) as i32
}

/// Parse C++ INI booleans: Yes/No, True/False, 1/0.
fn parse_ini_bool(val: &str) -> Option<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => val.parse::<bool>().ok(),
    }
}

/// C++ `ConvertVelocityInSecsToFrames` (`GameCommon.h`): dist/sec → dist/frame.
fn secs_to_frames_velocity(speed: f32) -> f32 {
    speed / 30.0
}

/// C++ `INI::parseAngleReal`: authored degrees → radians.
fn parse_angle_real(token: &str) -> Option<f32> {
    token
        .trim()
        .parse::<f32>()
        .ok()
        .map(|degrees| degrees * std::f32::consts::PI / 180.0)
}

/// C++ `WeaponTemplate::parseShotDelay`: one msec value, or `Min:N Max:M`.
/// Always overwrites both min and max, then converts msec → frames with ceil.
fn parse_shot_delay_msec(value: &str) -> Option<(i32, i32)> {
    let tokens: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ':')
        .filter(|token| !token.is_empty())
        .collect();
    let first = *tokens.first()?;
    if first.eq_ignore_ascii_case("Min") {
        let min = tokens.get(1)?.parse::<i32>().ok()?;
        let max = if tokens
            .get(2)
            .is_some_and(|token| token.eq_ignore_ascii_case("Max"))
        {
            tokens.get(3)?.parse::<i32>().ok()?
        } else {
            min
        };
        Some((min, max))
    } else {
        let msec = first.parse::<i32>().ok()?;
        Some((msec, msec))
    }
}

/// C++ `INI::parseBitString32`: `NONE`, bare tokens, or `+/-` (not mixed).
fn parse_bit_string_32(value: &str, names: &[&str]) -> Option<u32> {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.is_empty() {
        return Some(0);
    }
    let mut bits = 0u32;
    let mut found_normal = false;
    let mut found_add_or_sub = false;
    for token in tokens {
        if token.eq_ignore_ascii_case("NONE") {
            if found_normal || found_add_or_sub {
                return None;
            }
            return Some(0);
        }
        let (name, set) = if let Some(rest) = token.strip_prefix('+') {
            if found_normal {
                return None;
            }
            found_add_or_sub = true;
            (rest, true)
        } else if let Some(rest) = token.strip_prefix('-') {
            if found_normal {
                return None;
            }
            found_add_or_sub = true;
            (rest, false)
        } else {
            if found_add_or_sub {
                return None;
            }
            if !found_normal {
                bits = 0;
            }
            found_normal = true;
            (token, true)
        };
        let index = names
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(name))?;
        let mask = 1u32 << index;
        if set {
            bits |= mask;
        } else {
            bits &= !mask;
        }
    }
    Some(bits)
}

fn parse_percent_to_real(token: &str) -> Option<f32> {
    let trimmed = token.trim();
    if let Some(stripped) = trimmed.strip_suffix('%') {
        stripped.parse::<f32>().ok().map(|value| value / 100.0)
    } else {
        trimmed.parse::<f32>().ok()
    }
}

fn parse_weapon_bonus_condition(
    token: &str,
) -> Option<gamelogic::weapon::WeaponBonusConditionType> {
    use gamelogic::weapon::WeaponBonusConditionType::*;
    match token.trim().to_ascii_uppercase().as_str() {
        "GARRISONED" => Some(Garrisoned),
        "HORDE" => Some(Horde),
        "CONTINUOUS_FIRE_MEAN" => Some(ContinuousFireMean),
        "CONTINUOUS_FIRE_FAST" => Some(ContinuousFireFast),
        "NATIONALISM" => Some(Nationalism),
        "PLAYER_UPGRADE" => Some(PlayerUpgrade),
        "DRONE_SPOTTING" => Some(DroneSpotting),
        "DEMORALIZED" => Some(Demoralized),
        "ENTHUSIASTIC" => Some(Enthusiastic),
        "VETERAN" => Some(Veteran),
        "ELITE" => Some(Elite),
        "HERO" => Some(Hero),
        "BATTLEPLAN_BOMBARDMENT" => Some(BattleplanBombardment),
        "BATTLEPLAN_HOLDTHELINE" => Some(BattleplanHoldtheLine),
        "BATTLEPLAN_SEARCHANDDESTROY" => Some(BattleplanSearchAndDestroy),
        "SUBLIMINAL" => Some(Subliminal),
        "SOLO_HUMAN_EASY" => Some(SoloHumanEasy),
        "SOLO_HUMAN_NORMAL" => Some(SoloHumanNormal),
        "SOLO_HUMAN_HARD" => Some(SoloHumanHard),
        "SOLO_AI_EASY" => Some(SoloAiEasy),
        "SOLO_AI_NORMAL" => Some(SoloAiNormal),
        "SOLO_AI_HARD" => Some(SoloAiHard),
        "TARGET_FAERIE_FIRE" => Some(TargetFaerieFire),
        "FANATICISM" => Some(Fanaticism),
        "FRENZY_ONE" => Some(FrenzyOne),
        "FRENZY_TWO" => Some(FrenzyTwo),
        "FRENZY_THREE" => Some(FrenzyThree),
        _ => None,
    }
}

fn parse_weapon_bonus_field(token: &str) -> Option<gamelogic::weapon::WeaponBonusField> {
    use gamelogic::weapon::WeaponBonusField::*;
    match token.trim().to_ascii_uppercase().as_str() {
        "DAMAGE" => Some(Damage),
        "RADIUS" => Some(Radius),
        "RANGE" => Some(Range),
        "RATE_OF_FIRE" => Some(RateOfFire),
        "PRE_ATTACK" => Some(PreAttack),
        _ => None,
    }
}

fn apply_weapon_bonus_line(template: &mut gamelogic::WeaponTemplate, value: &str) {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.len() < 3 {
        return;
    }
    let Some(condition) = parse_weapon_bonus_condition(tokens[0]) else {
        return;
    };
    let Some(field) = parse_weapon_bonus_field(tokens[1]) else {
        return;
    };
    let Some(percent) = parse_percent_to_real(tokens[2]) else {
        return;
    };
    let set = template
        .extra_bonus
        .get_or_insert_with(gamelogic::WeaponBonusSet::new);
    let mut bonus = set
        .get_bonus(condition)
        .cloned()
        .unwrap_or_else(gamelogic::WeaponBonus::new);
    bonus.set_field(field, percent);
    set.set_bonus(condition, bonus);
}

fn apply_scatter_target_line(template: &mut gamelogic::WeaponTemplate, value: &str) {
    let tokens: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ':')
        .filter(|token| !token.is_empty())
        .filter(|token| !token.eq_ignore_ascii_case("X") && !token.eq_ignore_ascii_case("Y"))
        .collect();
    if tokens.len() < 2 {
        return;
    }
    if let (Ok(x), Ok(y)) = (tokens[0].parse::<f32>(), tokens[1].parse::<f32>()) {
        template
            .scatter_targets
            .push(gamelogic::weapon::Coord2D::new(x, y));
    }
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
    register_weapon_template_from_properties(name, properties, &ordered)
}

/// Register a Weapon block while preserving every assignment in source order.
fn register_weapon_template_ordered(name: &str, ordered: &OrderedIniProperties) -> bool {
    let properties = ordered.iter().cloned().collect::<HashMap<_, _>>();
    register_weapon_template_from_properties(name, &properties, ordered)
}

/// Register a weapon template parsed from INI into the GameLogic WeaponStore.
fn register_weapon_template_from_properties(
    name: &str,
    properties: &HashMap<String, String>,
    ordered: &OrderedIniProperties,
) -> bool {
    use gamelogic::WeaponTemplate;

    // C++ parseWeaponTemplateDefinition: find existing by NameKey, then
    // initFromINI listed fields. CREATE_OVERRIDES copies the base first
    // (newOverride). A fresh WeaponTemplate::new would zero unlisted
    // PrimaryDamage/range on a partial Map.ini re-declare.
    let mut template = gamelogic::with_weapon_store(|store| {
        store
            .find_weapon_template_ci(name)
            .map(|existing| (**existing).clone())
    })
    .ok()
    .flatten()
    .unwrap_or_else(|| WeaponTemplate::new(name.to_string()));

    // Map Common INI properties to GameLogic WeaponTemplate fields.

    if let Some(val) = properties.get("DamageType") {
        template.damage_type = parse_damage_type(val);
    }
    // C++ ObjectStatusMaskType::parseSingleBitFromINI → m_damageStatusType.
    // Default remains OBJECT_STATUS_NONE (Weapon.cpp:303).
    if let Some(val) = properties.get("DamageStatusType") {
        template.damage_status_type = parse_damage_status_type(val);
    }

    if let Some(val) = properties.get("WeaponSpeed") {
        if let Ok(speed) = val.parse::<f32>() {
            template.weapon_speed = secs_to_frames_velocity(speed);
        }
    }

    if let Some(val) = properties.get("MinWeaponSpeed") {
        if let Ok(speed) = val.parse::<f32>() {
            template.min_weapon_speed = secs_to_frames_velocity(speed);
        }
    }

    if let Some(val) = properties.get("AttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.attack_range = range;
        }
    }

    if let Some(val) = properties.get("MinimumAttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.minimum_attack_range = range;
        }
    }
    if let Some(val) = properties.get("MinRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.minimum_attack_range = range;
        }
    }

    if let Some(val) = properties.get("PrimaryDamage") {
        if let Ok(damage) = val.parse::<f32>() {
            template.primary_damage = damage;
        }
    }

    if let Some(val) = properties.get("SecondaryDamage") {
        if let Ok(damage) = val.parse::<f32>() {
            template.secondary_damage = damage;
        }
    }

    if let Some(val) = properties.get("PrimaryDamageRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.primary_damage_radius = radius;
        }
    }

    if let Some(val) = properties.get("SecondaryDamageRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.secondary_damage_radius = radius;
        }
    }

    if let Some(val) = properties.get("ShockWaveAmount") {
        if let Ok(amount) = val.parse::<f32>() {
            template.shock_wave_amount = amount;
        }
    }

    if let Some(val) = properties.get("ShockWaveRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.shock_wave_radius = radius;
        }
    }

    if let Some(val) = properties.get("ShockWaveTaperOff") {
        if let Ok(val) = val.parse::<f32>() {
            template.shock_wave_taper_off = val;
        }
    }

    // C++ only authors DelayBetweenShots (parseShotDelay). Always overwrite
    // both min/max, including the labeled `Min:N Max:M` retail form.
    if let Some(val) = properties.get("DelayBetweenShots") {
        if let Some((min_msec, max_msec)) = parse_shot_delay_msec(val) {
            template.min_delay_between_shots = msec_to_logic_frames(min_msec);
            template.max_delay_between_shots = msec_to_logic_frames(max_msec);
        }
    }

    if let Some(val) = properties.get("ClipSize") {
        if let Ok(size) = val.parse::<i32>() {
            template.clip_size = size;
        }
    }

    // C++ WeaponTemplateFieldParseTable owns this independently of clip
    // cadence.  It drives the per-Weapon barrel cursor, so do not silently
    // collapse a multi-shot barrel into one shot while importing Weapon.ini.
    if let Some(val) = properties.get("ShotsPerBarrel") {
        if let Ok(shots) = val.parse::<i32>() {
            template.shots_per_barrel = shots;
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames (30 FPS).
    if let Some(val) = properties.get("ClipReloadTime") {
        let tokens: Vec<&str> = val.split_whitespace().collect();
        if let Some(first) = tokens.first() {
            if let Ok(msec) = first.parse::<i32>() {
                template.clip_reload_time = msec_to_logic_frames(msec);
            }
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames (30 FPS).
    if let Some(val) = properties.get("PreAttackDelay") {
        let tokens: Vec<&str> = val.split_whitespace().collect();
        if let Some(first) = tokens.first() {
            if let Ok(msec) = first.parse::<i32>() {
                template.pre_attack_delay = msec_to_logic_frames(msec);
            }
        }
    }

    if let Some(val) = properties.get("ProjectileTemplate") {
        template.projectile_name = val.clone();
    }
    if let Some(val) = properties.get("ProjectileObject") {
        template.projectile_name = val.clone();
    }

    if let Some(val) = properties.get("ProjectileStreamName") {
        template.projectile_stream_name = val.clone();
    }

    if let Some(val) = properties.get("FireSound") {
        template.fire_sound = gamelogic::weapon::AudioEventRts::new(val.clone());
    }

    if let Some(val) = properties.get("ScatterRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.scatter_radius = radius;
        }
    }

    if let Some(val) = properties.get("AcceptableAimDelta") {
        if let Some(delta) = parse_angle_real(val) {
            template.aim_delta = delta;
        }
    }

    if let Some(val) = properties.get("RequestAssistRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.request_assist_range = range;
        }
    }

    if let Some(val) = properties.get("WeaponRecoil") {
        if let Some(recoil) = parse_angle_real(val) {
            template.weapon_recoil = recoil;
        }
    }

    if let Some(val) = properties.get("MinTargetPitch") {
        if let Some(pitch) = parse_angle_real(val) {
            template.min_target_pitch = pitch;
        }
    }
    if let Some(val) = properties.get("MaxTargetPitch") {
        if let Some(pitch) = parse_angle_real(val) {
            template.max_target_pitch = pitch;
        }
    }
    if let Some(val) = properties.get("RadiusDamageAngle") {
        if let Some(angle) = parse_angle_real(val) {
            template.radius_damage_angle = angle;
        }
    }

    if let Some(val) = properties.get("ScatterTargetScalar") {
        if let Ok(scalar) = val.parse::<f32>() {
            template.scatter_target_scalar = scalar;
        }
    }
    if let Some(val) = properties.get("ScatterRadiusVsInfantry") {
        if let Ok(dist) = val.parse::<f32>() {
            template.infantry_inaccuracy_dist = dist;
        }
    }

    // C++ `INI::parseBitInInt32` obeys both Yes and No.  In particular,
    // WeaponTemplate starts with AntiGround, so treating `AntiGround = No`
    // as a no-op quietly lets dedicated AA/anti-missile weapons hit ground.
    // Keep the exact per-bit result rather than inferring broad air/ground
    // capability from a weapon name or projectile type.
    let mut parse_anti_mask = |field: &str, bit: u32| {
        let Some(value) = properties.get(field) else {
            return;
        };
        match parse_ini_bool(value) {
            Some(true) => template.anti_mask.insert(bit),
            Some(false) => template.anti_mask.remove(bit),
            None => log::warn!("Invalid boolean for Weapon `{name}` field `{field}`: {value}"),
        }
    };
    parse_anti_mask(
        "AntiAirborneVehicle",
        gamelogic::weapon::WeaponAntiMask::AIRBORNE_VEHICLE,
    );
    parse_anti_mask("AntiGround", gamelogic::weapon::WeaponAntiMask::GROUND);
    parse_anti_mask(
        "AntiProjectile",
        gamelogic::weapon::WeaponAntiMask::PROJECTILE,
    );
    parse_anti_mask(
        "AntiSmallMissile",
        gamelogic::weapon::WeaponAntiMask::SMALL_MISSILE,
    );
    parse_anti_mask("AntiMine", gamelogic::weapon::WeaponAntiMask::MINE);
    parse_anti_mask(
        "AntiParachute",
        gamelogic::weapon::WeaponAntiMask::PARACHUTE,
    );
    parse_anti_mask(
        "AntiAirborneInfantry",
        gamelogic::weapon::WeaponAntiMask::AIRBORNE_INFANTRY,
    );
    parse_anti_mask(
        "AntiBallisticMissile",
        gamelogic::weapon::WeaponAntiMask::BALLISTIC_MISSILE,
    );
    // Release the closure's exclusive template borrow before parsing the
    // remainder of the Weapon.ini fields.
    drop(parse_anti_mask);

    if let Some(val) = properties.get("ScaleWeaponSpeed") {
        if let Some(b) = parse_ini_bool(val) {
            template.is_scale_weapon_speed = b;
        }
    }

    // C++ WeaponTemplateFieldParseTable: FireFX from an undetected stealth
    // source is suppressed unless this exact authored flag is true. Keep it
    // in the parsed template rather than reconstructing it from a weapon name.
    if let Some(val) = properties.get("PlayFXWhenStealthed") {
        if let Some(b) = parse_ini_bool(val) {
            template.play_fx_when_stealthed = b;
        }
    }

    if let Some(val) = properties.get("DeathType") {
        template.death_type = parse_death_type(val);
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames.
    if let Some(val) = properties.get("AutoReloadWhenIdle") {
        if let Ok(msec) = val.parse::<i32>() {
            template.auto_reload_when_idle_frames = msec_to_logic_frames(msec) as u32;
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames.
    if let Some(val) = properties.get("SuspendFXDelay") {
        if let Ok(msec) = val.parse::<i32>() {
            template.suspend_fx_delay = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("ContinueAttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.continue_attack_range = range;
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames.
    if let Some(val) = properties.get("HistoricBonusTime") {
        if let Ok(msec) = val.parse::<i32>() {
            template.historic_bonus_time = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("HistoricBonusRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.historic_bonus_radius = radius;
        }
    }

    if let Some(val) = properties.get("HistoricBonusCount") {
        if let Ok(count) = val.parse::<i32>() {
            template.historic_bonus_count = count;
        }
    }

    if let Some(val) = properties.get("HistoricBonusWeapon") {
        template.set_historic_bonus_weapon_name(val);
    }

    if let Some(val) = properties.get("LaserName") {
        template.laser_name = val.trim().to_string();
    }
    if let Some(val) = properties.get("LaserBoneName") {
        template.laser_bone_name = val.trim().to_string();
    }

    if let Some(val) = properties.get("ContinuousFireOne") {
        if let Ok(shots) = val.parse::<i32>() {
            template.continuous_fire_one_shots_needed = shots;
        }
    }
    if let Some(val) = properties.get("ContinuousFireTwo") {
        if let Ok(shots) = val.parse::<i32>() {
            template.continuous_fire_two_shots_needed = shots;
        }
    }
    if let Some(val) = properties.get("ContinuousFireCoast") {
        if let Ok(msec) = val.parse::<i32>() {
            template.continuous_fire_coast_frames = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("FireSoundLoopTime") {
        if let Ok(msec) = val.parse::<i32>() {
            template.fire_sound_loop_time = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("AutoReloadsClip") {
        if let Some(reload) = gamelogic::WeaponReloadType::from_ini(val.trim()) {
            template.reload_type = reload;
        }
    }
    if let Some(val) = properties.get("PreAttackType") {
        if let Some(prefire) = gamelogic::WeaponPrefireType::from_ini(val.trim()) {
            template.prefire_type = prefire;
        }
    }

    if let Some(val) = properties.get("RadiusDamageAffects") {
        if let Some(bits) = parse_bit_string_32(
            val,
            &[
                "SELF",
                "ALLIES",
                "ENEMIES",
                "NEUTRALS",
                "SUICIDE",
                "NOT_SIMILAR",
                "NOT_AIRBORNE",
            ],
        ) {
            template.affects_mask = gamelogic::WeaponAffectsMask::new(bits);
        }
    }
    if let Some(val) = properties.get("ProjectileCollidesWith") {
        if let Some(bits) = parse_bit_string_32(
            val,
            &[
                "ALLIES",
                "ENEMIES",
                "STRUCTURES",
                "SHRUBBERY",
                "PROJECTILES",
                "WALLS",
                "SMALL_MISSILES",
                "BALLISTIC_MISSILES",
                "CONTROLLED_STRUCTURES",
            ],
        ) {
            template.collide_mask = gamelogic::WeaponCollideMask::new(bits);
        }
    }

    if let Some(val) = properties.get("DamageDealtAtSelfPosition") {
        if let Some(b) = parse_ini_bool(val) {
            template.damage_dealt_at_self_position = b;
        }
    }
    if let Some(val) = properties.get("LeechRangeWeapon") {
        if let Some(b) = parse_ini_bool(val) {
            template.leech_range_weapon = b;
        }
    }
    if let Some(val) = properties.get("CapableOfFollowingWaypoints") {
        if let Some(b) = parse_ini_bool(val) {
            template.capable_of_following_waypoint = b;
        }
    }
    if let Some(val) = properties.get("ShowsAmmoPips") {
        if let Some(b) = parse_ini_bool(val) {
            template.is_shows_ammo_pips = b;
        }
    }
    if let Some(val) = properties.get("AllowAttackGarrisonedBldgs") {
        if let Some(b) = parse_ini_bool(val) {
            template.allow_attack_garrisoned_bldgs = b;
        }
    }
    if let Some(val) = properties.get("MissileCallsOnDie") {
        if let Some(b) = parse_ini_bool(val) {
            template.die_on_detonate = b;
        }
    }

    // `Weapon.cpp` applies these fields in file order. Keep this separate from
    // scalar-property parsing above because a HashMap has already discarded
    // repeated `Veterancy…` / `WeaponBonus` / `ScatterTarget` assignments.
    apply_weapon_effect_references(&mut template, ordered);

    // Register the template into the GameLogic WeaponStore
    match gamelogic::with_weapon_store_mut(|store| {
        store.add_weapon_template(template);
    }) {
        Ok(()) => {
            debug!("Registered weapon template: {}", name);
            true
        }
        Err(e) => {
            warn!("Failed to register weapon '{}': {}", name, e);
            false
        }
    }
}

/// Parse the one identifier C++ `INI::getNextToken` consumes for a named
/// Weapon.ini reference. `None` is a null pointer in the original engine, not
/// a valid FX/OCL/particle template name.
fn parse_weapon_reference_name(value: &str) -> Option<String> {
    let name = value.split_whitespace().next()?.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_string())
}

/// C++ `TheVeterancyNames` is exactly
/// `REGULAR`, `VETERAN`, `ELITE`, `HEROIC` in that order. Do not silently map
/// unknown names to Regular: that would turn malformed data into a visible
/// wrong-level effect.
fn parse_cpp_veterancy_index(value: &str) -> Option<usize> {
    if value.eq_ignore_ascii_case("regular") {
        Some(0)
    } else if value.eq_ignore_ascii_case("veteran") {
        Some(1)
    } else if value.eq_ignore_ascii_case("elite") {
        Some(2)
    } else if value.eq_ignore_ascii_case("heroic") {
        Some(3)
    } else {
        None
    }
}

/// Parse `Veterancy<Field> = LEVEL TemplateName` without losing the second
/// token. `Some((level, None))` is an intentional `None` override; `None`
/// means the property was malformed and is ignored like an unresolved C++ INI
/// field rather than being guessed.
fn parse_veterancy_weapon_reference(value: &str) -> Option<(usize, Option<String>)> {
    let mut tokens = value.split_whitespace();
    let level = parse_cpp_veterancy_index(tokens.next()?)?;
    let reference = tokens.next()?;
    let reference = (!reference.eq_ignore_ascii_case("none")).then(|| reference.to_string());
    Some((level, reference))
}

fn all_veterancy<T: Clone>(value: Option<T>) -> [Option<T>; 4] {
    std::array::from_fn(|_| value.clone())
}

/// Apply C++ Weapon.cpp lines 173–182 exactly enough for the active Rust
/// store:
///
/// - base fields populate Regular/Veteran/Elite/Heroic;
/// - every repeated `Veterancy…` field subsequently overwrites one named slot;
/// - `None` remains absent instead of becoming a fabricated template;
/// - OCL and particle names are retained for deferred, real-store resolution.
///
/// FXList is a small name handle in GameLogic. Constructing that handle does
/// not register a placeholder FX definition; GameClient resolves the exact
/// parsed name when executing the effect.
fn apply_weapon_effect_references(
    template: &mut gamelogic::WeaponTemplate,
    ordered: &OrderedIniProperties,
) {
    for (key, value) in ordered {
        if key.eq_ignore_ascii_case("FireFX") {
            let fx = parse_weapon_reference_name(value)
                .map(|name| gamelogic::effects::FXList::new(&name));
            template.fire_fx = all_veterancy(fx);
            continue;
        }

        if key.eq_ignore_ascii_case("ProjectileDetonationFX") {
            let fx = parse_weapon_reference_name(value)
                .map(|name| gamelogic::effects::FXList::new(&name));
            template.projectile_detonate_fx = all_veterancy(fx);
            continue;
        }

        if key.eq_ignore_ascii_case("FireOCL") {
            template.fire_ocl_names = all_veterancy(parse_weapon_reference_name(value));
            // A new INI reference supersedes any programmatic cached handle.
            template.fire_ocl = [None, None, None, None];
            continue;
        }

        if key.eq_ignore_ascii_case("ProjectileDetonationOCL") {
            template.projectile_detonation_ocl_names =
                all_veterancy(parse_weapon_reference_name(value));
            template.projectile_detonation_ocl = [None, None, None, None];
            continue;
        }

        if key.eq_ignore_ascii_case("ProjectileExhaust") {
            template.projectile_exhaust_names = all_veterancy(parse_weapon_reference_name(value));
            template.projectile_exhaust = [None, None, None, None];
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyFireFX") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.fire_fx[level] = name.map(|name| gamelogic::effects::FXList::new(&name));
            } else {
                warn!(
                    "Ignoring malformed VeterancyFireFX for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyProjectileDetonationFX") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.projectile_detonate_fx[level] =
                    name.map(|name| gamelogic::effects::FXList::new(&name));
            } else {
                warn!(
                    "Ignoring malformed VeterancyProjectileDetonationFX for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyFireOCL") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.fire_ocl_names[level] = name;
                template.fire_ocl[level] = None;
            } else {
                warn!(
                    "Ignoring malformed VeterancyFireOCL for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyProjectileDetonationOCL") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.projectile_detonation_ocl_names[level] = name;
                template.projectile_detonation_ocl[level] = None;
            } else {
                warn!(
                    "Ignoring malformed VeterancyProjectileDetonationOCL for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyProjectileExhaust") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.projectile_exhaust_names[level] = name;
                template.projectile_exhaust[level] = None;
            } else {
                warn!(
                    "Ignoring malformed VeterancyProjectileExhaust for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("WeaponBonus") {
            apply_weapon_bonus_line(template, value);
            continue;
        }

        if key.eq_ignore_ascii_case("ScatterTarget") {
            apply_scatter_target_line(template, value);
        }
    }
}

/// C++ `UpgradeTemplate::m_upgradeFieldParseTable` keys applied by GameLogic
/// `parse_from_ini` / setters (Upgrade.cpp:90-103).
const UPGRADE_INI_SETTER_FIELDS: &[&str] = &[
    "DisplayName",
    "Type",
    "BuildTime",
    "BuildCost",
    "ButtonImage",
    "ResearchSound",
    "UnitSpecificSound",
    "AcademyClassify",
];

/// Look up an INI property with C++-style case-insensitive field names.
fn find_ini_property<'a>(properties: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    if let Some(value) = properties.get(key) {
        return Some(value.as_str());
    }
    properties
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_str())
}

/// Apply C++ `initFromINI` fields onto an already-registered GameLogic template.
///
/// `UpgradeCenter::new_upgrade` stores `Arc<UpgradeTemplate>`, so setters cannot
/// mutate the live entry. `parse_upgrade_definition` clones the existing
/// template (C++ `findNonConstUpgradeByKey` + `initFromINI`), applies the
/// parse table, and stores it back.
fn apply_upgrade_template_ini_fields(
    name: &str,
    properties: &HashMap<String, String>,
) -> Result<(), String> {
    use game_engine::common::ini::{INI, INIError};
    use gamelogic::upgrade::center::with_upgrade_center_mut;

    let mut source = String::new();
    source.push_str(name);
    source.push('\n');

    let mut has_fields = false;
    for key in UPGRADE_INI_SETTER_FIELDS {
        if let Some(value) = find_ini_property(properties, key) {
            source.push_str(key);
            source.push_str(" = ");
            source.push_str(value);
            source.push('\n');
            has_fields = true;
        }
    }

    if !has_fields {
        return Ok(());
    }

    source.push_str("End\n");

    let mut ini = INI::new();
    ini.with_inline_source(&source, |ini| {
        ini.read_line()?;
        with_upgrade_center_mut(|center| {
            center
                .parse_upgrade_definition(ini)
                .map_err(|_| INIError::InvalidData)
        })
    })
    .map_err(|e| format!("{:?}", e))
}

/// Register an upgrade template parsed from INI into the GameLogic UpgradeCenter.
fn register_upgrade_template(name: &str, properties: &HashMap<String, String>) -> bool {
    use game_engine::common::ascii_string::AsciiString;
    use gamelogic::upgrade::center::with_upgrade_center_mut;

    let ascii_name = AsciiString::from(name);

    with_upgrade_center_mut(|center| {
        // C++ parseUpgradeDefinition: find existing or newUpgrade (inherits
        // DefaultUpgrade). Duplicate Upgrade blocks must not spam warnings.
        if center.find_upgrade(name).is_none() {
            let _template = center.new_upgrade(ascii_name);
        }
    });

    // C++ initFromINI on the (possibly pre-existing) template. Always apply
    // ButtonImage / DisplayName / BuildCost / BuildTime / Type when present.
    if let Err(e) = apply_upgrade_template_ini_fields(name, properties) {
        warn!("Failed to apply Upgrade.ini fields for '{}': {}", name, e);
    }

    debug!("Registered upgrade template: {}", name);
    true
}

/// Register a science template parsed from INI into leftover leftover + live ScienceStore.
fn register_science_template(name: &str, properties: &HashMap<String, String>) -> bool {
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::ini::ini_science::{get_science_store_mut, parse_science_definition};
    use game_engine::common::rts::science::{
        ScienceDefinition, get_science_store_mut as get_live_science_store_mut, init_science_store,
    };

    match parse_science_definition(name, properties) {
        Ok(info) => {
            {
                let mut store = get_science_store_mut();
                let _ = store.add_science(AsciiString::from(name), info);
            }
            // C++ TheScienceStore is one store. Host leftover leftover parse also
            // feeds the live rts store used by GrantScience / scripts.
            init_science_store();
            if let Some(mut live) = get_live_science_store_mut() {
                let prereq_names = properties.get("PrerequisiteSciences").map(|value| {
                    value
                        .split_whitespace()
                        .filter(|token| !token.is_empty() && !token.eq_ignore_ascii_case("None"))
                        .map(|token| token.to_string())
                        .collect::<Vec<_>>()
                });
                let cost = properties
                    .get("SciencePurchasePointCost")
                    .and_then(|value| value.parse().ok());
                let grantable = properties.get("IsGrantable").map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "yes" | "true" | "1"
                    )
                });
                live.ingest_definition(ScienceDefinition {
                    name: name.to_string(),
                    display_name: properties.get("DisplayName").cloned(),
                    description: properties.get("Description").cloned(),
                    prereq_names,
                    cost,
                    grantable,
                });
                live.rebuild_root_sciences();
            }
            debug!("Registered science template: {}", name);
            true
        }
        Err(e) => {
            warn!("Failed to parse science '{}': {:?}", name, e);
            false
        }
    }
}

/// Parse a damage type string into the GameLogic DamageType.
fn parse_damage_type(s: &str) -> gamelogic::DamageType {
    match s.to_lowercase().as_str() {
        "explosion" | "explosive" => gamelogic::DamageType::Explosion,
        "small_arms" | "smallarms" | "bullet" => gamelogic::DamageType::SmallArms,
        "flame" | "fire" | "burn" => gamelogic::DamageType::Flame,
        "crush" => gamelogic::DamageType::Crush,
        "armor_piercing" | "piercing" | "ap" => gamelogic::DamageType::ArmorPiercing,
        "hazard" | "chemical" | "hazard_cleanup" => gamelogic::DamageType::HazardCleanup,
        "heal" | "repair" | "healing" => gamelogic::DamageType::Healing,
        "disarm" => gamelogic::DamageType::Disarm,
        "sabotage" => gamelogic::DamageType::Hack,
        "snipe" | "sniper" => gamelogic::DamageType::Sniper,
        "laser" => gamelogic::DamageType::Laser,
        "radiation" | "rad" => gamelogic::DamageType::Radiation,
        "microwave" | "electric" | "electricity" | "emp" => gamelogic::DamageType::Microwave,
        "subdual" => gamelogic::DamageType::SubdualUnresistable,
        "status" => gamelogic::DamageType::Status,
        "combat" => gamelogic::DamageType::SmallArms,
        "particle" | "particlebeam" | "particle_beam" => gamelogic::DamageType::ParticleBeam,
        "poison" | "toxin" | "anthrax" => gamelogic::DamageType::Poison,
        "leadership" | "leadership_bonus" => gamelogic::DamageType::Unresistable,
        "demoralizing" | "demoralizing_shock" => gamelogic::DamageType::Penalty,
        "unresistable" | "none" => gamelogic::DamageType::Unresistable,
        "gattling" => gamelogic::DamageType::Gattling,
        "water" => gamelogic::DamageType::Water,
        "deploy" => gamelogic::DamageType::Deploy,
        "surrender" => gamelogic::DamageType::Surrender,
        "hack" => gamelogic::DamageType::Hack,
        "kill_pilot" => gamelogic::DamageType::KillPilot,
        "penalty" => gamelogic::DamageType::Penalty,
        "falling" => gamelogic::DamageType::Falling,
        "melee" => gamelogic::DamageType::Melee,
        "toppling" => gamelogic::DamageType::Toppling,
        "infantry_missile" => gamelogic::DamageType::InfantryMissile,
        "aurora_bomb" => gamelogic::DamageType::AuroraBomb,
        "land_mine" => gamelogic::DamageType::LandMine,
        "jet_missiles" => gamelogic::DamageType::JetMissiles,
        "stealthjet_missiles" => gamelogic::DamageType::StealthJetMissiles,
        "molotov_cocktail" => gamelogic::DamageType::MolotovCocktail,
        "comanche_vulcan" => gamelogic::DamageType::ComancheVulcan,
        "subdual_missile" => gamelogic::DamageType::SubdualMissile,
        "subdual_vehicle" => gamelogic::DamageType::SubdualVehicle,
        "subdual_building" => gamelogic::DamageType::SubdualBuilding,
        "subdual_unresistable" => gamelogic::DamageType::SubdualUnresistable,
        "kill_garrisoned" => gamelogic::DamageType::KillGarrisoned,
        _ => gamelogic::DamageType::Explosion, // Default fallback
    }
}

/// C++ `ObjectStatusMaskType::parseSingleBitFromINI` — one bit-name index.
///
/// Names match `ObjectStatusMaskType::s_bitNameList`. Unknown tokens stay
/// `OBJECT_STATUS_NONE` (ctor default) rather than inventing a bit.
fn parse_damage_status_type(s: &str) -> gamelogic::weapon::ObjectStatusTypes {
    use gamelogic::common::ObjectStatusTypes as CommonStatus;
    let status = match s.trim().to_ascii_uppercase().as_str() {
        "NONE" => CommonStatus::None,
        "DESTROYED" => CommonStatus::Destroyed,
        "CAN_ATTACK" => CommonStatus::CanAttack,
        "UNDER_CONSTRUCTION" => CommonStatus::UnderConstruction,
        "UNSELECTABLE" => CommonStatus::Unselectable,
        "NO_COLLISIONS" => CommonStatus::NoCollisions,
        "NO_ATTACK" => CommonStatus::NoAttack,
        "AIRBORNE_TARGET" => CommonStatus::AirborneTarget,
        "PARACHUTING" => CommonStatus::Parachuting,
        "REPULSOR" => CommonStatus::Repulsor,
        "HIJACKED" => CommonStatus::Hijacked,
        "AFLAME" => CommonStatus::Aflame,
        "BURNED" => CommonStatus::Burned,
        "WET" => CommonStatus::Wet,
        "IS_FIRING_WEAPON" => CommonStatus::IsFiringWeapon,
        "IS_BRAKING" => CommonStatus::Braking,
        "STEALTHED" => CommonStatus::Stealthed,
        "DETECTED" => CommonStatus::Detected,
        "CAN_STEALTH" => CommonStatus::CanStealth,
        "SOLD" => CommonStatus::Sold,
        "UNDERGOING_REPAIR" => CommonStatus::UndergoingRepair,
        "RECONSTRUCTING" => CommonStatus::Reconstructing,
        "MASKED" => CommonStatus::Masked,
        "IS_ATTACKING" => CommonStatus::IsAttacking,
        "USING_ABILITY" => CommonStatus::IsUsingAbility,
        "IS_AIMING_WEAPON" => CommonStatus::IsAimingWeapon,
        "NO_ATTACK_FROM_AI" => CommonStatus::NoAttackFromAi,
        "IGNORING_STEALTH" => CommonStatus::IgnoringStealth,
        "IS_CARBOMB" => CommonStatus::IsCarBomb,
        "DECK_HEIGHT_OFFSET" => CommonStatus::DeckHeightOffset,
        "STATUS_RIDER1" => CommonStatus::Rider1,
        "STATUS_RIDER2" => CommonStatus::Rider2,
        "STATUS_RIDER3" => CommonStatus::Rider3,
        "STATUS_RIDER4" => CommonStatus::Rider4,
        "STATUS_RIDER5" => CommonStatus::Rider5,
        "STATUS_RIDER6" => CommonStatus::Rider6,
        "STATUS_RIDER7" => CommonStatus::Rider7,
        "STATUS_RIDER8" => CommonStatus::Rider8,
        "FAERIE_FIRE" => CommonStatus::FaerieFire,
        "KILLING_SELF" => CommonStatus::MissileKillingSelf,
        "REASSIGN_PARKING" => CommonStatus::ReassignParking,
        "BOOBY_TRAPPED" => CommonStatus::BoobyTrapped,
        "IMMOBILE" => CommonStatus::Immobile,
        "DISGUISED" => CommonStatus::Disguised,
        "DEPLOYED" => CommonStatus::Deployed,
        _ => CommonStatus::None,
    };
    gamelogic::weapon::ObjectStatusTypes::new(status as u32)
}

/// Parse a death type string into the GameLogic DeathType.
fn parse_death_type(s: &str) -> gamelogic::DeathType {
    match s.to_lowercase().as_str() {
        "normal" => gamelogic::DeathType::Normal,
        "burned" | "fire" | "flame" => gamelogic::DeathType::Burned,
        "crushed" => gamelogic::DeathType::Crushed,
        "exploded" | "explosion" => gamelogic::DeathType::Exploded,
        "flooded" => gamelogic::DeathType::Flooded,
        "poisoned" | "poison" => gamelogic::DeathType::Poisoned,
        "poisoned_beta" | "poisonedbeta" => gamelogic::DeathType::PoisonedBeta,
        "poisoned_gamma" | "poisonedgamma" => gamelogic::DeathType::PoisonedGamma,
        "toppled" => gamelogic::DeathType::Toppled,
        "suicided" => gamelogic::DeathType::Suicided,
        "lasered" => gamelogic::DeathType::Lasered,
        _ => gamelogic::DeathType::Normal, // Default fallback
    }
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
    fn parse_shot_delay_accepts_min_max_colon_form() {
        assert_eq!(parse_shot_delay_msec("500"), Some((500, 500)));
        assert_eq!(parse_shot_delay_msec("Min:200 Max:800"), Some((200, 800)));
        assert_eq!(msec_to_logic_frames(500), 15);
        assert_eq!(secs_to_frames_velocity(600.0), 20.0);
        let delta = parse_angle_real("30").unwrap();
        assert!((delta - std::f32::consts::PI / 6.0).abs() < 1e-5);
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

        assert_eq!(register_weapons_from_ini_text(ini_content), 1);
        gamelogic::with_weapon_store(|store| {
            use gamelogic::weapon::WeaponAntiMask;

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
        assert_eq!(register_weapons_from_ini_text(ini_content), 1);
        gamelogic::with_weapon_store(|store| {
            let weapon = store
                .find_weapon_template("__RustDamageStatusFaerie")
                .expect("registered status weapon");
            let status: gamelogic::common::ObjectStatusTypes = weapon.damage_status_type.into();
            assert_eq!(status, gamelogic::common::ObjectStatusTypes::FaerieFire);
        })
        .expect("weapon store available");
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

        assert_eq!(register_weapons_from_ini_text(ini_content), 1);
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
    }

    #[test]
    fn redeclared_weapon_ini_merges_instead_of_replacing() {
        // C++ parseWeaponTemplateDefinition found-or-create then initFromINI.
        // A later partial block must keep unlisted fields.
        let _ = gamelogic::initialize_weapon_store();
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

        assert_eq!(register_weapons_from_ini_text(ini_content), 1);
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

    #[test]
    fn register_upgrade_template_copies_button_image() {
        let mut properties = HashMap::new();
        properties.insert("ButtonImage".to_string(), "SSTestCameo".to_string());

        assert!(register_upgrade_template(
            "Upgrade_TestButtonImageCameo",
            &properties
        ));

        gamelogic::upgrade::center::with_upgrade_center(|center| {
            let template = center
                .find_upgrade("Upgrade_TestButtonImageCameo")
                .expect("upgrade registered");
            assert_eq!(template.get_button_image_name().as_str(), "SSTestCameo");
        });
    }

    #[test]
    fn register_upgrade_template_updates_existing_button_image() {
        let name = "Upgrade_TestButtonImageCameoExisting";

        // First pass creates the template with no ButtonImage (C++ newUpgrade).
        assert!(register_upgrade_template(name, &HashMap::new()));
        gamelogic::upgrade::center::with_upgrade_center(|center| {
            let template = center.find_upgrade(name).expect("upgrade registered");
            assert!(template.get_button_image_name().as_str().is_empty());
        });

        // C++ initFromINI on existing: case-insensitive ButtonImage still applies.
        let mut properties = HashMap::new();
        properties.insert("buttonimage".to_string(), "SSTestCameo".to_string());
        properties.insert(
            "DisplayName".to_string(),
            "CONTROLBAR:TestCameo".to_string(),
        );
        properties.insert("BuildCost".to_string(), "500".to_string());
        properties.insert("BuildTime".to_string(), "15.0".to_string());
        assert!(register_upgrade_template(name, &properties));

        gamelogic::upgrade::center::with_upgrade_center(|center| {
            let template = center.find_upgrade(name).expect("upgrade still registered");
            assert_eq!(template.get_button_image_name().as_str(), "SSTestCameo");
            assert_eq!(template.get_display_name().as_str(), "CONTROLBAR:TestCameo");
            assert_eq!(template.get_cost(), 500);
            assert_eq!(template.get_build_time(), 15.0);
        });
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
    fn test_msec_to_logic_frames_matches_cpp_duration() {
        // C++ ConvertDurationFromMsecsToFrames: ceil(msec * 30 / 1000)
        assert_eq!(msec_to_logic_frames(0), 0);
        assert_eq!(msec_to_logic_frames(-1), 0);
        assert_eq!(msec_to_logic_frames(1000), 30); // 1 second
        assert_eq!(msec_to_logic_frames(100), 3); // ceil(3.0)
        assert_eq!(msec_to_logic_frames(33), 1); // ceil(0.99) = 1
        assert_eq!(msec_to_logic_frames(2000), 60);
    }

    #[test]
    fn test_clip_reload_and_pre_attack_use_frame_conversion() {
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
                "INIZH/Data/INI/Default/Science.ini".to_string(),
                "Data/INI/Science.ini".to_string(),
            ]
        );
    }

    #[test]
    fn test_parse_damage_type() {
        assert_eq!(parse_damage_type("Crush"), gamelogic::DamageType::Crush);
        assert_eq!(
            parse_damage_type("Small_Arms"),
            gamelogic::DamageType::SmallArms
        );
        assert_eq!(parse_damage_type("Flame"), gamelogic::DamageType::Flame);
        assert_eq!(parse_damage_type("Laser"), gamelogic::DamageType::Laser);
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

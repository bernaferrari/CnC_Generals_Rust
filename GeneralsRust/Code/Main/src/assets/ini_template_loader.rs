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
//! 3. Science INIs from `Data/INI/Science.ini`
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
/// Returns a list of `(block_type, block_name, properties)` tuples.
fn parse_ini_sections(content: &str) -> Vec<(String, String, HashMap<String, String>)> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections: Vec<(String, String, HashMap<String, String>)> = Vec::new();
    let mut current_type: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut current_props: HashMap<String, String> = HashMap::new();
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
                    current_props.insert(key, value);
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
            b'/' if !in_single
                && !in_double
                && i + 1 < bytes.len()
                && bytes[i + 1] == b'/' =>
            {
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

    discovered.sort_by(|a, b| a.cmp(b));
    discovered.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    discovered
}

/// Discover science INI files from the archive system.
///
/// In the C++ original, science INIs are loaded from:
/// - `Data/INI/Science.ini`
fn discover_science_ini_files(archive_system: &ArchiveFileSystem) -> Vec<String> {
    let all_files = archive_system.list_all_files();

    let mut discovered: Vec<String> = all_files
        .into_iter()
        .map(|path| normalize_archive_path(&path))
        .filter(|path| {
            let normalized = path.to_ascii_lowercase();
            archive_key_matches_suffix(&normalized, "data/ini/science.ini")
        })
        .collect();

    discovered.sort_by(|a, b| a.cmp(b));
    discovered.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    discovered
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
    debug!("Weapon INI discovery matched {} file(s)", weapon_files.len());

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

        let sections = parse_ini_sections(&content);
        let mut file_weapon_count = 0usize;

        for (block_type, block_name, properties) in &sections {
            if block_type.eq_ignore_ascii_case("Weapon") {
                if register_weapon_template(block_name, properties) {
                    file_weapon_count += 1;
                }
            }
        }

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
            if block_type.eq_ignore_ascii_case("Upgrade") {
                if register_upgrade_template(block_name, properties) {
                    file_upgrade_count += 1;
                }
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
            if block_type.eq_ignore_ascii_case("Science") {
                if register_science_template(block_name, properties) {
                    file_science_count += 1;
                }
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

/// Register a weapon template parsed from INI into the GameLogic WeaponStore.
fn register_weapon_template(name: &str, properties: &HashMap<String, String>) -> bool {
    use gamelogic::WeaponTemplate;

    let template_name = name.to_string();
    let mut template = WeaponTemplate::new(template_name);

    // Map Common INI properties to GameLogic WeaponTemplate fields.

    if let Some(val) = properties.get("DamageType") {
        template.damage_type = parse_damage_type(val);
    }

    if let Some(val) = properties.get("WeaponSpeed") {
        if let Ok(speed) = val.parse::<f32>() {
            template.weapon_speed = speed;
        }
    }

    if let Some(val) = properties.get("MinWeaponSpeed") {
        if let Ok(speed) = val.parse::<f32>() {
            template.min_weapon_speed = speed;
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

    if let Some(val) = properties.get("MinDelayBetweenShots") {
        if let Ok(delay) = val.parse::<i32>() {
            template.min_delay_between_shots = delay;
        }
    }

    if let Some(val) = properties.get("MaxDelayBetweenShots") {
        if let Ok(delay) = val.parse::<i32>() {
            template.max_delay_between_shots = delay;
        }
    }

    if let Some(val) = properties.get("ClipSize") {
        if let Ok(size) = val.parse::<i32>() {
            template.clip_size = size;
        }
    }

    if let Some(val) = properties.get("ClipReloadTime") {
        if let Ok(time) = val.parse::<i32>() {
            template.clip_reload_time = time;
        }
    }

    if let Some(val) = properties.get("PreAttackDelay") {
        if let Ok(delay) = val.parse::<i32>() {
            template.pre_attack_delay = delay;
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

    if let Some(val) = properties.get("AimDelta") {
        if let Ok(delta) = val.parse::<f32>() {
            template.aim_delta = delta;
        }
    }

    if let Some(val) = properties.get("RequestAssistRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.request_assist_range = range;
        }
    }

    if let Some(val) = properties.get("WeaponRecoil") {
        if let Ok(recoil) = val.parse::<f32>() {
            template.weapon_recoil = recoil;
        }
    }

    if let Some(val) = properties.get("AntiAirborneVehicle") {
        if let Ok(val) = val.parse::<bool>() {
            if val {
                template
                    .anti_mask
                    .insert(gamelogic::weapon::WeaponAntiMask::AIRBORNE_VEHICLE);
            }
        }
    }

    if let Some(val) = properties.get("AntiGround") {
        if let Ok(val) = val.parse::<bool>() {
            if val {
                template
                    .anti_mask
                    .insert(gamelogic::weapon::WeaponAntiMask::GROUND);
            }
        }
    }

    if let Some(val) = properties.get("AntiProjectile") {
        if let Ok(val) = val.parse::<bool>() {
            if val {
                template
                    .anti_mask
                    .insert(gamelogic::weapon::WeaponAntiMask::PROJECTILE);
            }
        }
    }

    if let Some(val) = properties.get("AntiSmallMissile") {
        if let Ok(val) = val.parse::<bool>() {
            if val {
                template
                    .anti_mask
                    .insert(gamelogic::weapon::WeaponAntiMask::SMALL_MISSILE);
            }
        }
    }

    if let Some(val) = properties.get("AntiMine") {
        if let Ok(val) = val.parse::<bool>() {
            if val {
                template
                    .anti_mask
                    .insert(gamelogic::weapon::WeaponAntiMask::MINE);
            }
        }
    }

    if let Some(val) = properties.get("ScaleWeaponSpeed") {
        if let Ok(val) = val.parse::<bool>() {
            template.is_scale_weapon_speed = val;
        }
    }

    if let Some(val) = properties.get("DeathType") {
        template.death_type = parse_death_type(val);
    }

    if let Some(val) = properties.get("AutoReloadWhenIdle") {
        if let Ok(frames) = val.parse::<u32>() {
            template.auto_reload_when_idle_frames = frames;
        }
    }

    if let Some(val) = properties.get("SuspendFXDelay") {
        if let Ok(delay) = val.parse::<u32>() {
            template.suspend_fx_delay = delay;
        }
    }

    if let Some(val) = properties.get("ContinueAttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.continue_attack_range = range;
        }
    }

    if let Some(val) = properties.get("HistoricBonusTime") {
        if let Ok(time) = val.parse::<u32>() {
            template.historic_bonus_time = time;
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

/// Register an upgrade template parsed from INI into the GameLogic UpgradeCenter.
fn register_upgrade_template(name: &str, _properties: &HashMap<String, String>) -> bool {
    use game_engine::common::ascii_string::AsciiString;
    use gamelogic::upgrade::center::with_upgrade_center_mut;

    let ascii_name = AsciiString::from(name);

    with_upgrade_center_mut(|center| {
        // C++ parity: duplicate Upgrade blocks can exist in INI and should not
        // spam warnings during registration.
        if center.find_upgrade(name).is_none() {
            let _template = center.new_upgrade(ascii_name);
        }
        debug!("Registered upgrade template: {}", name);
    });

    true
}

/// Register a science template parsed from INI into the Common ScienceStore.
fn register_science_template(name: &str, properties: &HashMap<String, String>) -> bool {
    use game_engine::common::ascii_string::AsciiString;
    use game_engine::common::ini::ini_science::{get_science_store_mut, parse_science_definition};

    let ascii_name = AsciiString::from(name);

    match parse_science_definition(name, properties) {
        Ok(info) => {
            let mut store = get_science_store_mut();
            if let Err(e) = store.add_science(ascii_name, info) {
                warn!("Failed to add science '{}': {:?}", name, e);
                return false;
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
        "armor_piercing" | "piercing" | "ap" => gamelogic::DamageType::Sniper,
        "hazard" | "chemical" => gamelogic::DamageType::Hazard,
        "heal" | "repair" => gamelogic::DamageType::Healing,
        "disarm" => gamelogic::DamageType::Disarm,
        "sabotage" => gamelogic::DamageType::DemoralizingShock,
        "snipe" | "sniper" => gamelogic::DamageType::Sniper,
        "laser" => gamelogic::DamageType::Laser,
        "radiation" | "rad" => gamelogic::DamageType::Radiation,
        "microwave" => gamelogic::DamageType::Microwave,
        "electric" | "electricity" | "emp" => gamelogic::DamageType::Emp,
        "subdual" => gamelogic::DamageType::Subdual,
        "status" => gamelogic::DamageType::Status,
        "combat" => gamelogic::DamageType::Combat,
        "particle" | "particlebeam" => gamelogic::DamageType::ParticleBeam,
        "poison" | "toxin" | "anthrax" => gamelogic::DamageType::Poison,
        "leadership" | "leadership_bonus" => gamelogic::DamageType::LeadershipBonus,
        "demoralizing" | "demoralizing_shock" => gamelogic::DamageType::DemoralizingShock,
        "unresistable" | "none" => gamelogic::DamageType::Unresistable,
        _ => gamelogic::DamageType::Explosion, // Default fallback
    }
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
        assert_eq!(
            sections[1].2.get("PrerequisiteSciences").unwrap(),
            "None"
        );
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

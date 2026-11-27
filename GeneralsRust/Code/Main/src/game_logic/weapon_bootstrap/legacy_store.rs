//! Compatibility publication for the legacy GameLogic WeaponStore.

use generals_game_domain::ini::{OrderedIniProperties, parse_ini_sections_ordered};
use generals_sim::weapon::{WeaponDefinition, apply_weapon_ini_fields};
use log::{debug, warn};
#[cfg(test)]
use std::collections::HashMap;
use std::sync::Arc;

/// Publish the archive definitions in block order. The descriptors are shared
/// with the owned catalog. Legacy per-template handles belong to this store.
pub(crate) fn publish_archive_weapon_definitions(
    definitions: impl IntoIterator<Item = Arc<WeaponDefinition>>,
) {
    let _ = gamelogic::initialize_weapon_store();
    for definition in definitions {
        let _ = publish_weapon_template(gamelogic::weapon::WeaponTemplate::from_definition(
            definition,
        ));
    }
}

/// Read authored data through the compatibility store until this caller
/// has a receiving catalog. Keep exact-case lookup and bootstrap semantics.
pub(in crate::game_logic) fn read_legacy_weapon_definition<R>(
    name: &str,
    read: impl FnOnce(Option<&WeaponDefinition>) -> R,
) -> generals_sim::GameLogicResult<R> {
    super::with_host_weapon_store(|store| {
        read(
            store
                .find_weapon_template(name)
                .map(|template| -> &WeaponDefinition { template }),
        )
    })
}

/// Freeze the named definition for a mapless Object or visual dispatch.
pub(in crate::game_logic) fn legacy_weapon_definition(name: &str) -> Option<Arc<WeaponDefinition>> {
    super::with_host_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|rule| rule.shared_definition())
    })
    .ok()
    .flatten()
}

/// Register compatibility overrides from raw Weapon.ini text.
pub fn register_weapons_from_ini_text(content: &str) -> usize {
    with_weapons_from_ini_text(content, |count| count)
}

/// Register weapons then run `f` while the admitting WorldServices stay bound.
/// Prefer this over `register_weapons_from_ini_text` + a later free `with_weapon_store`
/// when the caller needs to read the store it just filled.
pub fn with_weapons_from_ini_text<R>(content: &str, f: impl FnOnce(usize) -> R) -> R {
    let run = || {
        let n = register_weapons_on_active(content);
        f(n)
    };
    if gamelogic::system::engine_stores::try_active_services().is_some() {
        run()
    } else {
        let services = gamelogic::system::engine_stores::new_world_services();
        gamelogic::system::engine_stores::with_world_services(&services, run)
    }
}

fn register_weapons_on_active(content: &str) -> usize {
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
#[cfg(test)]
pub(crate) fn register_weapon_template(name: &str, properties: &HashMap<String, String>) -> bool {
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

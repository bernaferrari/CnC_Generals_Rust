//! Owned startup catalog admission for headless skirmish hosting.

use super::{
    DEFAULT_OBJECT_INI, OBJECT_INI_DIR, ordered_object_source_paths, read_receiving_object_source,
    walk_object_ini_dir,
};
use std::collections::BTreeSet;

/// Catalogs for a headless startup from a caller-owned, initialized archive.
/// The caller chooses its mounts and loose roots; this helper does not mount
/// a mod, language pack, or manual archive on the caller's behalf.
pub(crate) struct HeadlessRetailCatalogs {
    pub(crate) objects: crate::assets::ReceivingObjectContent,
    pub(crate) weapons: Vec<(String, gamelogic::weapon::WeaponTemplate)>,
}

/// Select and parse retail definitions without a GPU or a published asset
/// manager. This is a catalog admission step, not a complete engine boot.
pub(crate) fn load_headless_retail_catalogs(
    archive: &mut crate::assets::ArchiveFileSystem,
    roots: &[&str],
) -> Result<HeadlessRetailCatalogs, String> {
    let inventory = archive.list_all_files();
    // The archive also reads from its own local roots, which are not part of
    // its BIG-file inventory. Enumerate those same roots without publishing a
    // filesystem service; the reader below retains its existing precedence.
    let resolver = crate::assets::resolver::LiveAssetResolver::default();
    let mut local_current = BTreeSet::new();
    let mut local_nested = BTreeSet::new();
    for root in archive.local_search_roots() {
        if let Some(directory) = resolver.first_existing_candidate(&[root.join(OBJECT_INI_DIR)]) {
            walk_object_ini_dir(
                &directory,
                OBJECT_INI_DIR,
                &mut local_current,
                &mut local_nested,
            );
        }
    }
    let local_paths = local_current.into_iter().chain(local_nested).collect();
    let object_paths = ordered_object_source_paths(local_paths, inventory, roots);
    load_headless_catalogs_from_sources(&object_paths, |path| {
        read_receiving_object_source(path, roots, |path| {
            archive
                .read_optional_definition_source(path)
                .map_err(|error| format!("Receiving startup source '{path}': {error}"))
        })
    })
}

fn load_headless_catalogs_from_sources(
    object_paths: &[String],
    mut read: impl FnMut(&str) -> Result<Option<String>, String>,
) -> Result<HeadlessRetailCatalogs, String> {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    use game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions;
    use std::sync::Arc;

    fn required(
        read: &mut impl FnMut(&str) -> Result<Option<String>, String>,
        path: &str,
    ) -> Result<String, String> {
        read(path)?.ok_or_else(|| format!("Required retail startup source '{path}' is missing"))
    }

    let mut science_sources = Vec::new();
    for path in ["Data/INI/Default/Science.ini", "Data/INI/Science.ini"] {
        let source = if path == "Data/INI/Science.ini" {
            Some(required(&mut read, path)?)
        } else {
            read(path)?
        };
        if let Some(source) = source {
            science_sources.push((path, source.into_bytes()));
        }
    }
    let source_refs = science_sources
        .iter()
        .map(|(path, bytes)| {
            (
                *path,
                bytes.as_slice(),
                path.eq_ignore_ascii_case("Data/INI/Science.ini"),
            )
        })
        .collect::<Vec<_>>();
    let mut sciences = game_engine::common::rts::science::ScienceStore::new_owned();
    sciences
        .load_from_sources(&source_refs)
        .map_err(|error| format!("Receiving Science definitions: {error}"))?;
    let sciences =
        Arc::new(game_engine::common::rts::science::ScienceNameFacts::from_store(&sciences));

    // GameEngine.cpp initializes WeaponStore from this exact source only.
    // Read it through the same receiving overlay as the other startup sources,
    // including authoritative loose files that are not in the archive index.
    let source = required(&mut read, "Data/INI/Weapon.ini")?;
    let mut definitions = std::collections::HashMap::new();
    parse_weapon_definitions_ini_into(&mut definitions, &source, WeaponDefinitionLoadType::Normal);
    let mut actions = ReceivingActionDefinitions::default();
    for path in [
        "Data/INI/Default/SpecialPower.ini",
        "Data/INI/SpecialPower.ini",
    ] {
        let source = if path == "Data/INI/SpecialPower.ini" {
            Some(required(&mut read, path)?)
        } else {
            read(path)?
        };
        if let Some(source) = source {
            actions
                .apply_source_with_load_type(
                    &source,
                    &sciences,
                    game_engine::common::ini::INILoadType::Overwrite,
                )
                .map_err(|error| format!("Receiving SpecialPower source '{path}': {error}"))?;
        }
    }

    if !object_paths
        .iter()
        .any(|path| path.eq_ignore_ascii_case(DEFAULT_OBJECT_INI))
        || object_paths.len() < 2
    {
        return Err("Required retail Object source inventory is incomplete".into());
    }
    let object_sources = object_paths
        .iter()
        .map(|path| required(&mut read, path).map(|text| (path.clone(), text)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut objects = crate::assets::ReceivingObjectContent::from_sources_with_catalogs(
        object_sources,
        sciences,
        Arc::new(actions),
    )?;
    for path in [
        "Data/INI/Default/CommandButton.ini",
        "Data/INI/CommandButton.ini",
        "Data/INI/Default/CommandSet.ini",
        "Data/INI/CommandSet.ini",
    ] {
        let source = if path == "Data/INI/CommandButton.ini" || path == "Data/INI/CommandSet.ini" {
            Some(required(&mut read, path)?)
        } else {
            read(path)?
        };
        if let Some(source) = source {
            objects
                .admit_command_source(&source)
                .map_err(|error| format!("Receiving command source '{path}': {error}"))?;
        }
    }

    if definitions.is_empty() {
        return Err("Required retail Weapon.ini contains no admitted weapons".into());
    }
    let mut weapons = definitions
        .into_values()
        .map(|definition| (definition.name.clone(), definition))
        .collect::<Vec<_>>();
    weapons.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(HeadlessRetailCatalogs { objects, weapons })
}

#[cfg(test)]
mod headless_catalog_tests {
    use super::*;

    fn sources(damage: u32) -> std::collections::HashMap<String, String> {
        [
            ("Data/INI/Science.ini", "".to_owned()),
            ("Data/INI/SpecialPower.ini", "".to_owned()),
            (
                "Data/INI/CommandButton.ini",
                "CommandButton OwnedButton\nEnd\n".to_owned(),
            ),
            (
                "Data/INI/CommandSet.ini",
                "CommandSet OwnedSet\n 1 = OwnedButton\nEnd\n".to_owned(),
            ),
            (DEFAULT_OBJECT_INI, "Object OwnedDefault\nEnd\n".to_owned()),
            (
                "Data/INI/Object/Owned.ini",
                "Object OwnedUnit\nDraw = W3DModelDraw DrawTag\nEnd\nEnd\n".to_owned(),
            ),
            (
                "Data/INI/Weapon.ini",
                format!("Weapon OwnedWeapon\nPrimaryDamage = {damage}\nEnd\n"),
            ),
        ]
        .into_iter()
        .map(|(path, text)| (path.to_owned(), text))
        .collect()
    }

    fn build(
        sources: &std::collections::HashMap<String, String>,
    ) -> Result<HeadlessRetailCatalogs, String> {
        let objects = vec![
            DEFAULT_OBJECT_INI.into(),
            "Data/INI/Object/Owned.ini".into(),
        ];
        load_headless_catalogs_from_sources(&objects, |path| Ok(sources.get(path).cloned()))
    }

    #[test]
    fn headless_catalogs_are_independent_and_missing_sources_fail() {
        let first = build(&sources(7)).unwrap();
        let second = build(&sources(19)).unwrap();
        assert!(first.objects.projections().get("OwnedUnit").is_some());
        assert!(
            first
                .objects
                .module_content()
                .action_definitions()
                .buttons
                .find_command_button_resolved("OwnedButton")
                .is_some()
        );
        assert_eq!(
            first
                .objects
                .module_content()
                .action_definitions()
                .sets
                .find_command_set_resolved("OwnedSet")
                .unwrap()
                .buttons[0]
                .as_deref(),
            Some("OwnedButton")
        );
        assert_eq!(first.weapons[0].1.primary_damage, 7.0);
        assert_eq!(second.weapons[0].1.primary_damage, 19.0);
        assert_eq!(first.weapons[0].1.primary_damage, 7.0);

        let mut missing = sources(7);
        missing.remove("Data/INI/Object/Owned.ini");
        assert!(build(&missing).err().unwrap().contains("Owned.ini"));
        missing.remove("Data/INI/Science.ini");
        assert!(build(&missing).err().unwrap().contains("Science.ini"));
        for path in [
            "Data/INI/Weapon.ini",
            "Data/INI/CommandButton.ini",
            "Data/INI/CommandSet.ini",
        ] {
            let mut missing = sources(7);
            missing.remove(path);
            assert!(build(&missing).err().unwrap().contains(path));
        }
    }

    #[test]
    fn headless_catalog_reads_exact_weapon_source_before_objects_and_commands() {
        let objects = vec![
            DEFAULT_OBJECT_INI.into(),
            "Data/INI/Object/Owned.ini".into(),
        ];
        let mut input = sources(7);
        input.insert(
            "Data/INI/Default/Weapon.ini".into(),
            "Weapon OwnedWeapon\nPrimaryDamage = 99\nEnd\n".into(),
        );
        input.insert(
            "Data/INI/Weapon/Extra.ini".into(),
            "Weapon OwnedWeapon\nPrimaryDamage = 88\nEnd\n".into(),
        );
        let mut reads = Vec::new();
        let catalog = load_headless_catalogs_from_sources(&objects, |path| {
            reads.push(path.to_owned());
            Ok(input.get(path).cloned())
        })
        .unwrap();
        assert_eq!(catalog.weapons[0].1.primary_damage, 7.0);
        assert_eq!(
            reads,
            [
                "Data/INI/Default/Science.ini",
                "Data/INI/Science.ini",
                "Data/INI/Weapon.ini",
                "Data/INI/Default/SpecialPower.ini",
                "Data/INI/SpecialPower.ini",
                DEFAULT_OBJECT_INI,
                "Data/INI/Object/Owned.ini",
                "Data/INI/Default/CommandButton.ini",
                "Data/INI/CommandButton.ini",
                "Data/INI/Default/CommandSet.ini",
                "Data/INI/CommandSet.ini",
            ]
        );
    }

    #[test]
    fn headless_science_main_source_keeps_unspecified_default_fields() {
        let mut input = sources(7);
        input.insert(
            "Data/INI/Default/Science.ini".into(),
            "Science SCIENCE_Owned\n SciencePurchasePointCost = 7\n IsGrantable = No\nEnd\n".into(),
        );
        input.insert(
            "Data/INI/Science.ini".into(),
            "Science SCIENCE_Owned\n DisplayName = OwnedDisplay\nEnd\n".into(),
        );
        let catalog = build(&input).unwrap();
        let facts = catalog.objects.module_content().science_facts();
        let science = facts.science("SCIENCE_Owned").unwrap();
        assert_eq!(facts.definitions().get_science_purchase_cost(science), 7);
        assert!(!facts.definitions().is_science_grantable(science));
    }

    #[test]
    fn headless_inventory_discovers_archive_owner_local_roots() {
        let root = tempfile::tempdir().unwrap();
        let mut input = sources(7);
        // Loose INI policy rejects empty files; a comment is an authored,
        // still-empty definition file for this inventory test.
        for text in input.values_mut() {
            if text.is_empty() {
                *text = "; empty\n".into();
            }
        }
        for (path, text) in input {
            let target = root.path().join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, text).unwrap();
        }
        let mut archive = crate::assets::ArchiveFileSystem::new();
        archive.set_local_search_roots([root.path().to_path_buf()]);
        assert!(archive.list_all_files().is_empty());
        let result = load_headless_retail_catalogs(&mut archive, &[]).unwrap();
        assert!(result.objects.projections().get("OwnedUnit").is_some());
        assert_eq!(result.weapons[0].1.primary_damage, 7.0);
    }

    #[test]
    fn headless_catalog_propagates_selected_source_read_error() {
        let objects = vec![
            DEFAULT_OBJECT_INI.into(),
            "Data/INI/Object/Owned.ini".into(),
        ];
        let input = sources(7);
        let error = load_headless_catalogs_from_sources(&objects, |path| {
            if path == "Data/INI/Object/Owned.ini" {
                Err("selected object source unreadable".into())
            } else {
                Ok(input.get(path).cloned())
            }
        })
        .err()
        .unwrap();
        assert_eq!(error, "selected object source unreadable");
        let error = load_headless_catalogs_from_sources(&objects, |path| {
            if path == "Data/INI/Weapon.ini" {
                Err("selected weapon source unreadable".into())
            } else {
                Ok(input.get(path).cloned())
            }
        })
        .err()
        .unwrap();
        assert_eq!(error, "selected weapon source unreadable");
    }
}

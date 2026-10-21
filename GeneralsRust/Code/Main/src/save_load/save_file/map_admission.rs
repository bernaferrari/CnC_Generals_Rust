//! Detached GameStateMap header boundary. Original v2 counters precede the
//! optional original SkirmishGameInfo v4 stream (GameStateMap.cpp395–406).
use super::*;
use game_engine::common::system::skirmish_game_info::SkirmishGameInfo;

pub(super) fn read_source(payload: &[u8]) -> SaveLoadResult<(i32, Option<SkirmishGameInfo>)> {
    fn invalid() -> SaveLoadError {
        SaveLoadError::Corrupted("truncated GameStateMap initial admission".into())
    }
    fn take<'a>(remaining: &mut &'a [u8], size: usize) -> SaveLoadResult<&'a [u8]> {
        let value = remaining.get(..size).ok_or_else(invalid)?;
        *remaining = &remaining[size..];
        Ok(value)
    }
    fn word(remaining: &mut &[u8]) -> SaveLoadResult<u32> {
        Ok(u32::from_le_bytes(take(remaining, 4)?.try_into().unwrap()))
    }
    let mut bytes = payload;
    let version = *take(&mut bytes, 1)?.first().unwrap();
    if version != 1 && version != 2 {
        return Err(SaveLoadError::Corrupted(
            "unsupported GameStateMap version".into(),
        ));
    }
    for _ in 0..2 {
        let length = *take(&mut bytes, 1)?.first().unwrap() as usize;
        take(&mut bytes, length)?;
    }
    let mode = if version >= 2 {
        word(&mut bytes)? as i32
    } else {
        CPP_GAME_SINGLE_PLAYER
    };
    let map_size = word(&mut bytes)? as usize;
    take(&mut bytes, map_size)?;
    word(&mut bytes)?;
    word(&mut bytes)?; // original ObjectID/DrawableID counters
    if bytes.is_empty() {
        return Ok((mode, None));
    }
    if mode != CPP_GAME_SKIRMISH {
        return Err(SaveLoadError::Corrupted(
            "unexpected GameStateMap suffix outside Skirmish".into(),
        ));
    }
    let source = SkirmishGameInfo::decode(bytes)
        .map_err(|e| SaveLoadError::Corrupted(format!("invalid saved SkirmishGameInfo: {e}")))?;
    Ok((mode, Some(source)))
}

pub(super) fn bind_original_if_needed(
    snapshot: &mut WorldSnapshot,
    source: Option<&SkirmishGameInfo>,
) -> SaveLoadResult<()> {
    if crate::save_load::snapshot::initial_skirmish::read(snapshot)?.is_none()
        && crate::save_load::snapshot::initial_skirmish::source(snapshot)?.is_none()
    {
        if let Some(source) = source {
            // Pure decoded data only. Receiving definition resolution belongs
            // to stage/restore, never this file decoder.
            crate::save_load::snapshot::initial_skirmish::append_original(
                &mut snapshot.lifecycle_tail,
                source.clone(),
            )?;
            // Original SGI needs capability 29; never downgrade a newer world body.
            snapshot.version = snapshot.version.max(29);
        }
    }
    Ok(())
}

pub(super) fn validate_native(
    snapshot: &WorldSnapshot,
    mode: i32,
    source: Option<&SkirmishGameInfo>,
    normalize_map: impl Fn(&str) -> String,
) -> SaveLoadResult<()> {
    let initial = crate::save_load::snapshot::initial_skirmish::read(snapshot)?;
    if mode != CPP_GAME_SKIRMISH {
        if source.is_some()
            || initial.is_some()
            || crate::save_load::snapshot::initial_skirmish::source(snapshot)?.is_some()
        {
            return Err(SaveLoadError::Corrupted(
                "initial Skirmish admission supplied for another game mode".into(),
            ));
        }
        return Ok(());
    }
    if initial.is_none()
        && source.is_some()
        && crate::save_load::snapshot::initial_skirmish::source(snapshot)?.as_ref() == source
    {
        return Ok(());
    }
    let Some(initial) = initial else {
        return Err(SaveLoadError::Corrupted(if source.is_some() {
            "Original SkirmishGameInfo requires explicit receiving initial admission; missing native resolved admission".into()
        } else {
            "Missing saved initial SkirmishGameInfo and native resolved admission; cannot reconstruct participant definitions".into()
        }));
    };
    initial.validate().map_err(SaveLoadError::Corrupted)?;
    let project = |value: &SkirmishGameInfo| {
        let mut projected = value.clone();
        projected.map_name = normalize_map(&projected.map_name);
        projected
    };
    if initial.source.as_ref().map(project) != source.map(project) {
        return Err(SaveLoadError::Corrupted(
            "GameStateMap and native initial SkirmishGameInfo disagree".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skirmish_config::saved_admission::SavedSkirmishAdmission;
    use crate::skirmish_config::{
        GameRulesSnapshot, SkirmishMatchConfig, SkirmishPlayerTemplateSelection, SkirmishSlotConfig,
    };

    fn initial() -> SavedSkirmishAdmission {
        let mut source = SkirmishGameInfo::default();
        source.map_name = r"Maps\Alpine\Alpine.map".into();
        source.seed = 317;
        let mut rules = GameRulesSnapshot::default_rules();
        rules.starting_cash = 19000;
        source.starting_cash = 19000;
        source.superweapon_restriction = u16::from(rules.limit_superweapons);
        source.slots[0].state = 5;
        source.slots[0].name = "Initial Human".into();
        source.slots[0].start_pos = 1;
        source.slots[0].player_template = 3;
        source.slots[0].team_number = -1;
        SavedSkirmishAdmission {
            config: SkirmishMatchConfig {
                source_game_info: None,
                map: source.map_name.clone(),
                random_seed: 317,
                old_factions_only: false,
                rules,
                slots: vec![SkirmishSlotConfig {
                    slot_index: 0,
                    is_human: true,
                    is_active: true,
                    player_template: SkirmishPlayerTemplateSelection::Exact {
                        template_name: "SourceTemplate".into(),
                        template_index: 3,
                    },
                    faction: "SourceSide".into(),
                    color_rgb: (1, 2, 3),
                    team: -1,
                    start_position: 1,
                    player_name: "Initial Human".into(),
                    ai_difficulty: None,
                }],
            },
            source: Some(source),
            local_slot: Some(0),
            local_slot_from_client: true,
        }
    }

    #[test]
    fn original_map_name_uses_existing_portable_projection_without_changing_other_fields() {
        let admission = initial();
        let source = admission.source.as_ref().unwrap();
        let info = SaveGameInfo {
            pristine_map_name: None,
            filename: "source".into(),
            display_name: "Source".into(),
            description: "source map path".into(),
            map_name: String::new(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH,
            game_version: "test".into(),
            play_time: std::time::Duration::ZERO,
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        let mut bytes = Cursor::new(Vec::new());
        write_game_state_map_block(
            &mut CommonXferSave::new(&mut bytes, SAVE_FILE_VERSION),
            &info,
            Path::new("Save"),
            Some(source),
            CPP_GAME_SKIRMISH,
        )
        .unwrap();
        let (mode, observed) = read_source(bytes.get_ref()).unwrap();
        assert_eq!(mode, CPP_GAME_SKIRMISH);
        let mut expected = source.clone();
        expected.map_name = r"maps\alpine\alpine.map".into(); // Xfer.cpp151 + GameState848–875
        assert_eq!(observed, Some(expected));
        assert_eq!(
            admission.source.as_ref().unwrap().map_name,
            r"Maps\Alpine\Alpine.map"
        );
    }

    #[test]
    fn native_and_original_records_compare_portable_map_identity_and_every_other_scalar() {
        let admission = initial();
        // Original-only decoded metadata must raise old capabilities to 29,
        // while preserving the absolute Drawable scale capability in 30.
        for (version, expected_version) in [(23, 29), (29, 29), (30, 30)] {
            let mut original_only = WorldSnapshot::default();
            original_only.version = version;
            let source = admission.source.as_ref().unwrap();
            bind_original_if_needed(&mut original_only, Some(source)).unwrap();
            assert_eq!(original_only.version, expected_version);
            assert!(
                crate::save_load::snapshot::initial_skirmish::read(&original_only)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                crate::save_load::snapshot::initial_skirmish::source(&original_only)
                    .unwrap()
                    .as_ref(),
                Some(source)
            );
            let first = bincode_legacy::serialize(&original_only).unwrap();
            bind_original_if_needed(&mut original_only, Some(source)).unwrap();
            assert_eq!(
                bincode_legacy::serialize(&original_only).unwrap(),
                first,
                "rebinding Original metadata must not duplicate or alter the world body"
            );
            validate_native(
                &original_only,
                CPP_GAME_SKIRMISH,
                Some(source),
                str::to_owned,
            )
            .unwrap();
        }
        let mut world = WorldSnapshot::default();
        crate::save_load::snapshot::initial_skirmish::append(&mut world.lifecycle_tail, &admission)
            .unwrap();
        let mut observed = admission.source.clone().unwrap();
        observed.map_name = r"maps\alpine\alpine.map".into();
        let normalize =
            |map: &str| crate::save_load::map_paths::to_portable(map, Path::new("Save"), "");
        validate_native(&world, CPP_GAME_SKIRMISH, Some(&observed), normalize).unwrap();
        observed.starting_cash += 1;
        assert!(validate_native(&world, CPP_GAME_SKIRMISH, Some(&observed), normalize).is_err());
        observed.starting_cash -= 1;
        observed.map_name = r"maps\other\other.map".into();
        assert!(validate_native(&world, CPP_GAME_SKIRMISH, Some(&observed), normalize).is_err());
        assert!(
            validate_native(&world, CPP_GAME_SINGLE_PLAYER, Some(&observed), normalize).is_err()
        );
    }
}

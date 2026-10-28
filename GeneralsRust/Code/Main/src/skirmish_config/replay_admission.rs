//! Recorded GameInfo projection. No Client lobby or current runtime roster is queried.
use super::*;
use game_engine::common::recorder::{ReplayGameInfo, ReplaySlot};

pub(crate) fn config_from_recorded_info(
    info: &ReplayGameInfo,
    definitions: Option<&game_engine::common::rts::player_template::PlayerTemplateStore>,
) -> Result<Option<SkirmishMatchConfig>, String> {
    let occupied = info.slots.iter().any(|slot| slot.is_occupied);
    if !occupied {
        return Ok(None);
    }
    match definitions {
        Some(store) => config_from_recorded_info_in(info, store),
        None => {
            game_engine::common::ini::ensure_player_templates_loaded();
            config_from_recorded_info_in(
                info,
                &game_engine::common::rts::player_template::get_player_template_store(),
            )
        }
    }
}

fn config_from_recorded_info_in(
    info: &ReplayGameInfo,
    store: &game_engine::common::rts::player_template::PlayerTemplateStore,
) -> Result<Option<SkirmishMatchConfig>, String> {
    let mut slots = Vec::new();
    for (index, slot) in info.slots.iter().enumerate() {
        let active = slot.is_occupied;
        let (faction, selection) = if active && (slot.is_observer || slot.player_template == -2) {
            ("Observer".into(), SkirmishPlayerTemplateSelection::Observer)
        } else if active && slot.player_template >= 0 {
            let template = store
                .get_nth_player_template(slot.player_template as usize)
                .ok_or_else(|| {
                    format!(
                        "recorded slot {index} has invalid PlayerTemplate {}",
                        slot.player_template
                    )
                })?;
            let side = template.get_side();
            (
                if side.eq_ignore_ascii_case("America") {
                    "USA".into()
                } else {
                    side.to_string()
                },
                SkirmishPlayerTemplateSelection::Exact {
                    template_name: template.get_name().to_string(),
                    template_index: slot.player_template,
                },
            )
        } else if active && slot.player_template == -1 {
            ("Random".into(), SkirmishPlayerTemplateSelection::Random)
        } else if active {
            return Err(format!(
                "recorded slot {index} has unsupported PlayerTemplate {}",
                slot.player_template
            ));
        } else {
            ("USA".into(), SkirmishPlayerTemplateSelection::Random)
        };
        slots.push(SkirmishSlotConfig {
            slot_index: index,
            is_human: slot.is_human,
            is_active: active,
            player_template: selection,
            faction,
            color_rgb: color_rgb_from_multiplayer_index(slot.color, index),
            team: slot.team,
            start_position: slot.start_pos,
            player_name: slot.name.clone(),
            ai_difficulty: if active
                && !slot.is_human
                && !slot.is_observer
                && slot.player_template != -2
            {
                Some(
                    match slot.ai_difficulty {
                        1 => "Easy",
                        2 => "Medium",
                        3 => "Hard",
                        n => {
                            return Err(format!(
                                "recorded slot {index} has invalid AI difficulty {n}"
                            ));
                        }
                    }
                    .into(),
                )
            } else {
                None
            },
        });
    }
    Ok(Some(SkirmishMatchConfig {
        // A replay header is not an observed complete saved GameInfo v4 record.
        source_game_info: None,
        map: info.map.clone(),
        random_seed: info.seed,
        old_factions_only: info.old_factions_only,
        rules: GameRulesSnapshot {
            starting_cash: info.starting_cash as i32,
            game_speed: 1.0,
            limit_superweapons: info.superweapon_restriction != 0,
            allow_tech_buildings: true,
            crates_enabled: true,
            fog_of_war: game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(
                |settings| settings.is_shroud_in_multiplayer,
            ),
        },
        slots,
    }))
}

/// Observer presentation needs only the slot identity/team facts; unlike a
/// replay header, these remain available for valid native admissions that
/// cannot be serialized as GENREP.
pub(crate) fn observer_game_info(
    admission: &saved_admission::SavedSkirmishAdmission,
) -> ReplayGameInfo {
    use super::SkirmishPlayerTemplateSelection;

    let mut info = ReplayGameInfo::new();
    for source in &admission.config.slots {
        let Some(slot) = info.get_slot_mut(source.slot_index) else {
            continue;
        };
        slot.is_occupied = source.is_active;
        slot.is_human = source.is_human;
        slot.is_observer = matches!(
            &source.player_template,
            &SkirmishPlayerTemplateSelection::Observer
        );
        slot.team = source.team;
    }
    info
}

/// Freeze the actual resolved start admission, never reconstruct from live players.
pub(crate) fn recording_info(
    admission: &saved_admission::SavedSkirmishAdmission,
) -> Result<(ReplayGameInfo, i32), String> {
    let config = &admission.config;
    let mut info = ReplayGameInfo::new();
    info.map = config.map.clone();
    info.seed = config.random_seed;
    info.starting_cash = config.rules.starting_cash as u32;
    info.superweapon_restriction = u32::from(config.rules.limit_superweapons);
    info.old_factions_only = config.old_factions_only;
    if let Some(source) = &admission.source {
        info.crc_interval = source.crc_interval as u32;
        info.local_ip = source.local_ip;
        info.map_crc = source.map_crc;
        info.map_size = source.map_size;
        info.map_contents_mask = source.map_mask as u32;
        info.superweapon_restriction = u32::from(source.superweapon_restriction);
        info.starting_cash = source.starting_cash;
    }
    for slot in &config.slots {
        let destination = info
            .slots
            .get_mut(slot.slot_index)
            .ok_or("recording slot index out of range")?;
        if !slot.is_active {
            continue;
        }
        let template = match slot.player_template {
            SkirmishPlayerTemplateSelection::Exact { template_index, .. } => template_index,
            SkirmishPlayerTemplateSelection::Observer => -2,
            SkirmishPlayerTemplateSelection::Random => {
                return Err("recording start admission is unresolved".into());
            }
        };
        let color = if let Some(source) = &admission.source {
            source.slots[slot.slot_index].color
        } else {
            game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
                let mut matches = (0..settings.get_num_colors()).filter(|index| {
                    settings.get_color_value(*index).is_some_and(|packed| {
                        (
                            ((packed >> 16) & 255) as u8,
                            ((packed >> 8) & 255) as u8,
                            (packed & 255) as u8,
                        ) == slot.color_rgb
                    })
                });
                match (matches.next(), matches.next()) {
                    (Some(index), None) => Ok(index),
                    (None, _) => Err(format!(
                        "native recording slot {} color has no observed palette index",
                        slot.slot_index
                    )),
                    (Some(_), Some(_)) => Err(format!(
                        "native recording slot {} color has ambiguous palette identity",
                        slot.slot_index
                    )),
                }
            })?
        };
        if color < 0 {
            // Old native saves may contain the former white observer sentinel.
            // Reading those saves remains supported; newly written original
            // replay headers must contain a resolved authored index.
            return Err(format!(
                "recording occupied slot {} has unresolved or invalid color index {color}",
                slot.slot_index
            ));
        }
        *destination = ReplaySlot {
            name: slot.player_name.clone(),
            ip: 0,
            is_human: slot.is_human,
            is_observer: template == -2,
            is_occupied: true,
            color,
            player_template: template,
            start_pos: slot.start_position,
            team: slot.team,
            nat: 0,
            port: 0,
            accepted: true,
            has_map: true,
            ai_difficulty: match slot.ai_difficulty.as_deref() {
                Some("Easy") => 1,
                Some("Medium") => 2,
                Some("Hard" | "Brutal") => 3,
                _ if slot.is_human || template == -2 => 0,
                _ => return Err("recording AI difficulty is absent".into()),
            },
            is_open: false,
        };
    }
    Ok((
        info,
        admission.local_slot.map(|slot| slot as i32).unwrap_or(-1),
    ))
}

//! Resolved initial inputs retained by one match. Current Player/Team state is
//! deliberately not a reconstruction source. GameStateMap.cpp395–426 restores
//! these before PlayerList/TeamFactory consume the prepared map sides.
use super::*;
use game_engine::common::system::skirmish_game_info::{SKIRMISH_MAX_SLOTS, SkirmishGameInfo};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SavedSkirmishAdmission {
    pub config: SkirmishMatchConfig,
    /// Only genuine GameInfo observations may produce original v4 bytes.
    pub source: Option<SkirmishGameInfo>,
    pub local_slot: Option<u32>,
    /// Immutable origin of the local selection, independent of whether a
    /// complete original stream can be emitted for this native admission.
    pub local_slot_from_client: bool,
}
impl SavedSkirmishAdmission {
    pub(super) fn capture(
        config: &SkirmishMatchConfig,
        assigned: &[SkirmishSlotConfig],
        resolved: &[ResolvedSkirmishSlot<'_>],
    ) -> Result<Self, String> {
        let mut config_copy = config.clone();
        let observed = config_copy.source_game_info.take();
        config_copy.slots = assigned.to_vec();
        for slot in &mut config_copy.slots {
            if !slot.is_active {
                continue;
            }
            let selected = resolved
                .iter()
                .find(|r| r.slot.slot_index == slot.slot_index)
                .ok_or("resolved initial slot is missing")?;
            slot.player_template = SkirmishPlayerTemplateSelection::Exact {
                template_name: selected.player_template.template_name.clone(),
                template_index: selected
                    .player_template
                    .template_index
                    .ok_or("resolved initial template has no index")?,
            };
        }
        let (mut source, source_local) = match observed {
            Some((info, local)) => (Some(info), u32::try_from(local).ok()),
            None => (None, None),
        };
        if let Some(info) = &mut source {
            // Random faction/start selection has now completed once, matching
            // the source's saved current fields. Original fields stay intact.
            for slot in config_copy.slots.iter().filter(|slot| slot.is_active) {
                let SkirmishPlayerTemplateSelection::Exact { template_index, .. } =
                    &slot.player_template
                else {
                    unreachable!()
                };
                info.slots[slot.slot_index].player_template = *template_index;
                info.slots[slot.slot_index].start_pos = slot.start_position;
            }
        }
        let local_slot = if source.is_some() && source_local.is_none() {
            // Before GameInfo enters inGame its local-slot query returns -1.
            // Offline admission can prove a sole human, never guess among
            // several humans (the deferred network/local-IP domain).
            let mut humans = config_copy
                .slots
                .iter()
                .filter(|s| s.is_active && s.is_human);
            let local = humans.next().map(|s| s.slot_index as u32);
            if humans.next().is_some() {
                return Err(
                    "genuine initial GameInfo has multiple humans without an explicit local slot"
                        .into(),
                );
            }
            local
        } else {
            source_local.or_else(|| {
                config_copy
                    .slots
                    .iter()
                    .filter(|s| s.is_active && s.is_human)
                    .min_by_key(|s| s.slot_index)
                    .map(|s| s.slot_index as u32)
            })
        };
        let local_slot_from_client = source.is_some();
        // Native Main already projected unresolved lobby colors into its typed
        // RGB input. Preserve that exact native input, but never turn Random
        // (-1), an invalid index, or a mismatched palette into an invented
        // original selected-color index (GameLogic.cpp763–778).
        // Availability is decided once at initial admission. Later embedded
        // scratch-map paths never replace this stored logical map identity.
        let original_record_complete = source.as_ref().is_some_and(|info| {
            // Source menu enters and starts the match before new-game admission.
            // A direct pre-start observation stays native-only; never invent flags.
            info.in_game
                && info.in_progress
                && info.map_name == config_copy.map
                && config_copy
                    .slots
                    .iter()
                    .filter(|slot| slot.is_active)
                    .all(|slot| {
                        let color = info.slots[slot.slot_index].color;
                        color >= 0
                            && game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(
                                |settings| {
                                    settings.get_color_value(color).is_some_and(|packed| {
                                        (
                                            ((packed >> 16) & 255) as u8,
                                            ((packed >> 8) & 255) as u8,
                                            (packed & 255) as u8,
                                        ) == slot.color_rgb
                                    })
                                },
                            )
                    })
        });
        if !original_record_complete {
            source = None;
        }
        let result = Self {
            config: config_copy,
            source,
            local_slot,
            local_slot_from_client,
        };
        result.validate()?;
        Ok(result)
    }

    /// Import only facts present in a genuine original v4 stream. Original
    /// GameLogic wire remains a separate capability of the save reader.
    pub(crate) fn from_original(info: SkirmishGameInfo) -> Result<Self, String> {
        game_engine::common::ini::ensure_player_templates_loaded();
        let store = game_engine::common::rts::player_template::get_player_template_store();
        let mut slots = Vec::with_capacity(SKIRMISH_MAX_SLOTS);
        for (index, source) in info.slots.iter().enumerate() {
            if !(0..=5).contains(&source.state) {
                return Err("invalid original GameInfo slot state".into());
            }
            let active = (2..=5).contains(&source.state);
            let (selection, faction, color) = if active {
                let template = store
                    .get_nth_player_template_signed(source.player_template)
                    .ok_or(
                        "original saved occupied slot has no concrete receiving PlayerTemplate",
                    )?;
                if source.color < 0 {
                    return Err(format!(
                        "original saved occupied slot has unresolved or invalid color index {}",
                        source.color
                    ));
                }
                let packed = game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(
                    |settings| settings.get_color_value(source.color),
                )
                .ok_or_else(|| {
                    format!(
                        "original saved occupied slot has unresolved or invalid color index {}",
                        source.color
                    )
                })?;
                (
                    SkirmishPlayerTemplateSelection::Exact {
                        template_name: template.get_name().to_string(),
                        template_index: source.player_template,
                    },
                    template.get_side().to_string(),
                    (
                        ((packed >> 16) & 255) as u8,
                        ((packed >> 8) & 255) as u8,
                        (packed & 255) as u8,
                    ),
                )
            } else {
                (
                    SkirmishPlayerTemplateSelection::Random,
                    String::new(),
                    (0, 0, 0),
                )
            };
            slots.push(SkirmishSlotConfig {
                slot_index: index,
                is_human: source.state == 5,
                is_active: active,
                player_template: selection,
                faction,
                color_rgb: color,
                team: source.team_number,
                start_position: source.start_pos,
                player_name: source.name.clone(),
                ai_difficulty: match source.state {
                    2 => Some("Easy".into()),
                    3 => Some("Medium".into()),
                    4 => Some("Hard".into()),
                    _ => None,
                },
            });
        }
        drop(store);
        // Offline original import can prove one local human. Multiple humans
        // need the deferred network/local-IP ownership path, never a guessed ID.
        let humans: Vec<_> = slots
            .iter()
            .filter(|s| s.is_active && s.is_human)
            .map(|s| s.slot_index as u32)
            .collect();
        if humans.len() > 1 {
            return Err("original multi-human save requires explicit local slot admission".into());
        }
        let mut rules = GameRulesSnapshot::default_rules();
        rules.starting_cash = info.starting_cash as i32;
        rules.limit_superweapons = info.superweapon_restriction != 0;
        rules.fog_of_war =
            game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
                settings.is_shroud_in_multiplayer
            });
        let result = Self {
            config: SkirmishMatchConfig {
                source_game_info: None,
                map: info.map_name.clone(),
                random_seed: info.seed as u32,
                old_factions_only: false,
                rules,
                slots,
            },
            source: Some(info),
            local_slot: humans.first().copied(),
            local_slot_from_client: true,
        };
        result.validate_definitions()?;
        Ok(result)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.config.map.trim().is_empty() || !self.config.rules.game_speed.is_finite() {
            return Err("invalid saved initial Skirmish map/rules".into());
        }
        if self.config.source_game_info.is_some() {
            return Err("nested initial GameInfo observation".into());
        }
        let mut ids = HashSet::new();
        let mut active = 0;
        for slot in &self.config.slots {
            if slot.slot_index >= SKIRMISH_MAX_SLOTS || !ids.insert(slot.slot_index) {
                return Err("invalid or duplicate saved initial Skirmish slot".into());
            }
            if !slot.is_active {
                continue;
            }
            active += 1;
            match &slot.player_template {
                SkirmishPlayerTemplateSelection::Exact {
                    template_name,
                    template_index,
                } if !template_name.is_empty() && *template_index >= 0 => {}
                _ => {
                    return Err(format!(
                        "saved initial slot {} has no resolved PlayerTemplate",
                        slot.slot_index
                    ));
                }
            }
            if let Some(info) = &self.source {
                let raw = &info.slots[slot.slot_index];
                let state = match (slot.is_human, slot.ai_difficulty.as_deref()) {
                    (true, _) => 5,
                    (false, Some("Easy")) => 2,
                    (false, Some("Hard" | "Brutal")) => 4,
                    (false, _) => 3,
                };
                let SkirmishPlayerTemplateSelection::Exact { template_index, .. } =
                    &slot.player_template
                else {
                    unreachable!()
                };
                if raw.color < 0 {
                    return Err(
                        "complete original GameInfo contains an unresolved color index".into(),
                    );
                }
                if raw.state != state
                    || raw.start_pos != slot.start_position
                    || raw.player_template != *template_index
                    || raw.team_number != slot.team
                {
                    return Err(format!(
                        "original GameInfo disagrees with saved initial slot {}",
                        slot.slot_index
                    ));
                }
            }
        }
        if active == 0 {
            return Err("saved initial Skirmish admission has no occupied slots".into());
        }
        if let Some(local) = self.local_slot {
            if !self
                .config
                .slots
                .iter()
                .any(|s| s.is_active && s.is_human && s.slot_index == local as usize)
            {
                return Err("saved initial local slot is not an occupied human".into());
            }
        }
        if self.source.is_some() && !self.local_slot_from_client {
            return Err("original GameInfo has no genuine local selection provenance".into());
        }
        if self.local_slot_from_client
            && self.local_slot.is_none()
            && self.config.slots.iter().any(|s| s.is_active && s.is_human)
        {
            return Err("genuine initial admission has no local human selection".into());
        }
        if let Some(info) = &self.source {
            if info.map_name != self.config.map {
                return Err(
                    "complete original GameInfo disagrees with admitted map identity".into(),
                );
            }
            if info.seed as u32 != self.config.random_seed
                || info.starting_cash != self.config.rules.starting_cash as u32
                || (info.superweapon_restriction != 0) != self.config.rules.limit_superweapons
            {
                return Err("original GameInfo disagrees with saved initial rules".into());
            }
            let occupied = info
                .slots
                .iter()
                .filter(|s| (2..=5).contains(&s.state))
                .count();
            if occupied != active || info.slots.iter().any(|s| !(0..=5).contains(&s.state)) {
                return Err("original GameInfo occupied slot coverage differs".into());
            }
            info.encode().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Validate actual receiving definitions before any candidate reset/map IO.
    pub(crate) fn validate_definitions(&self) -> Result<(), String> {
        self.validate()?;
        let _ = resolve_skirmish_slots(&self.config)?; // all Exact; consumes no random values
        if let Some(info) = &self.source {
            for slot in self.config.slots.iter().filter(|slot| slot.is_active) {
                let color = info.slots[slot.slot_index].color;
                let agrees = game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(
                    |settings| {
                        color >= 0
                            && settings.get_color_value(color).is_some_and(|packed| {
                                (
                                    ((packed >> 16) & 255) as u8,
                                    ((packed >> 8) & 255) as u8,
                                    (packed & 255) as u8,
                                ) == slot.color_rgb
                            })
                    },
                );
                if !agrees {
                    return Err(format!(
                        "saved original slot {} has a receiving palette/RGB disagreement",
                        slot.slot_index
                    ));
                }
            }
        }
        Ok(())
    }

    /// Apply the captured Client local authority even when the optional
    /// original stream was omitted. Direct typed inputs retain ordinary flags.
    pub(super) fn admit_local_selection(&self, world: &mut GameLogic) {
        if self.local_slot_from_client {
            for player in world.get_players_mut().values_mut() {
                player.is_local = self.local_slot == Some(player.id);
            }
        }
    }

    /// Install the same player admission kernel without reroll/reset/recorder.
    pub(crate) fn admit(&self, world: &mut GameLogic) -> Result<(), String> {
        self.validate_definitions()?;
        let resolved = resolve_skirmish_slots(&self.config)?;
        world.clear_all_players();
        admit_resolved_slots(world, &self.config, &self.config.slots, resolved)?;
        self.admit_local_selection(world);
        world.initial_skirmish_admission = Some(Box::new(self.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Serialize)]
    struct HistoricalConfig<'a> {
        map: &'a str,
        random_seed: u32,
        old_factions_only: bool,
        rules: &'a GameRulesSnapshot,
        slots: &'a [SkirmishSlotConfig],
    }
    #[test]
    fn genuine_source_observation_does_not_extend_historical_config_bytes() {
        let mut config = SkirmishMatchConfig {
            source_game_info: None,
            map: "Map".into(),
            random_seed: 317,
            old_factions_only: false,
            rules: GameRulesSnapshot::default_rules(),
            slots: Vec::new(),
        };
        let expected = bincode_legacy::serialize(&HistoricalConfig {
            map: &config.map,
            random_seed: config.random_seed,
            old_factions_only: config.old_factions_only,
            rules: &config.rules,
            slots: &config.slots,
        })
        .unwrap();
        config.source_game_info = Some((SkirmishGameInfo::default(), 3));
        assert_eq!(bincode_legacy::serialize(&config).unwrap(), expected);
        let decoded: SkirmishMatchConfig = bincode_legacy::deserialize(&expected).unwrap();
        assert!(decoded.source_game_info.is_none());
    }
    #[test]
    fn ambiguous_genuine_local_slot_rejects_before_reset_or_seed_changes() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "ambiguous_genuine_local_slot_rejects_before_reset_or_seed_changes",
            || {
                let mut config = golden_skirmish_config("Lone Eagle");
                config.slots[1].is_human = true;
                config.slots[1].ai_difficulty = None;
                let mut source = SkirmishGameInfo::default();
                source.map_name = config.map.clone();
                source.seed = config.random_seed as i32;
                source.starting_cash = config.rules.starting_cash as u32;
                source.superweapon_restriction = u16::from(config.rules.limit_superweapons);
                for slot in &config.slots {
                    let raw = &mut source.slots[slot.slot_index];
                    raw.state = 5;
                    raw.name = slot.player_name.clone();
                    raw.start_pos = slot.start_position;
                    raw.team_number = slot.team;
                }
                config.source_game_info = Some((source, -1));
                let mut world = GameLogic::new();
                let mut peer = GameLogic::new();
                world.add_player(Player::new(
                    23,
                    Team::USA,
                    "Receiving before rejection",
                    true,
                ));
                peer.add_player(Player::new(23, Team::GLA, "Equal-ID peer", true));
                world.set_current_frame(713);
                peer.set_current_frame(29);
                let before = (
                    world.game_mode(),
                    world.frame,
                    world.logic_base_seed,
                    world.logic_random.seed_words(),
                );
                let peer_before = (
                    peer.game_mode(),
                    peer.frame,
                    peer.logic_base_seed,
                    peer.logic_random.seed_words(),
                );
                let prototypes = world
                    .team_factory
                    .lock()
                    .unwrap()
                    .list_team_prototypes_in_admission_order()
                    .into_iter()
                    .map(|p| (p.get_name().to_string(), p.get_id()))
                    .collect::<Vec<_>>();
                let error = apply_skirmish_config(&mut world, &config).unwrap_err();
                assert!(
                    error.contains("multiple humans without an explicit local slot"),
                    "{error}"
                );
                assert_eq!(
                    (
                        world.game_mode(),
                        world.frame,
                        world.logic_base_seed,
                        world.logic_random.seed_words()
                    ),
                    before
                );
                assert_eq!(
                    (
                        peer.game_mode(),
                        peer.frame,
                        peer.logic_base_seed,
                        peer.logic_random.seed_words()
                    ),
                    peer_before
                );
                assert_eq!(
                    world.get_player(23).unwrap().name,
                    "Receiving before rejection"
                );
                assert_eq!(peer.get_player(23).unwrap().name, "Equal-ID peer");
                assert_eq!(
                    world
                        .team_factory
                        .lock()
                        .unwrap()
                        .list_team_prototypes_in_admission_order()
                        .into_iter()
                        .map(|p| (p.get_name().to_string(), p.get_id()))
                        .collect::<Vec<_>>(),
                    prototypes
                );
                assert!(world.initial_skirmish_admission.is_none());
                assert!(peer.initial_skirmish_admission.is_none());
            },
        );
    }
}

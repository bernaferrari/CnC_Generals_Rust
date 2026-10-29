//! Score advice frozen from the receiving match before its players are reset.
use super::GameLogic;
use game_engine::common::rts::academy_stats::{AcademyAdviceContext, AcademyAdviceInfo};

impl GameLogic {
    #[cfg(feature = "game_client")]
    pub(crate) fn score_screen_player_roster(
        &self,
    ) -> Vec<game_client::gui::callbacks::score_screen::ScoreScreenPlayerSnapshot> {
        use crate::game_logic::game_logic::PlayerSideRole;
        use game_client::gui::callbacks::score_screen::ScoreScreenPlayerSnapshot;
        use gamelogic::common::Relationship;

        let local = self.local_player_id().and_then(|id| self.players.get(&id));
        // C++ grabSinglePlayerInfo substitutes the first human Player when
        // the selected local Player is an observer, then uses that Player for
        // exclusion and relationship grouping. It still uses the actual local
        // Player's template for the screen image below.
        let relationship_local = local.filter(|player| !player.is_observer).or_else(|| {
            self.players
                .values()
                .filter(|player| player.is_human)
                .min_by_key(|player| player.id)
        });
        let mut players: Vec<_> = self.players.values().collect();
        players.sort_by_key(|player| player.id);
        players
            .into_iter()
            .filter_map(|player| {
                let id = i32::try_from(player.id).ok()?;
                let template = self
                    .player_template_identity(player.id)
                    .and_then(|identity| self.resolve_player_template_identity(identity));
                let color = player.color_rgb;
                // C++ Player::getRelationship(const Team*) receives the
                // target's actual default Team pointer. Its fallback queries
                // playerRelations using that Team's current controller, which
                // can differ from the target Player after Team transfers.
                // A missing/stale Team returns Neutral before either map is
                // queried; a valid Team with no controller also has no player
                // fallback.
                let target_team = player.default_team_id.and_then(|team_id| {
                    let team = self.team_factory.lock().ok()?.find_team_by_id(team_id)?;
                    let controller_id = team.read().ok()?.get_controlling_player_id();
                    Some((team_id, controller_id))
                });
                let relationship_to_local = relationship_local
                    .and_then(|local| {
                        target_team.and_then(|(team_id, controller_id)| {
                            local.team_relationship_override(team_id).or_else(|| {
                                controller_id
                                    .and_then(|player_id| local.map_relationship(player_id))
                            })
                        })
                    })
                    .unwrap_or(Relationship::Neutral);
                let has_objects = self
                    .get_objects()
                    .registered_values()
                    .any(|object| self.player_owner_for_host_object(object) == Some(player.id));
                Some(ScoreScreenPlayerSnapshot {
                    id,
                    player_name: player.map_side.map_player_name.clone(),
                    display_name: player.name.clone(),
                    side: template
                        .as_ref()
                        .map(|template| template.get_side().to_string())
                        .unwrap_or_default(),
                    base_side: template
                        .as_ref()
                        .map(|template| template.get_base_side().to_string())
                        .unwrap_or_default(),
                    color_argb: 0xFF00_0000
                        | ((color.0 as u32) << 16)
                        | ((color.1 as u32) << 8)
                        | color.2 as u32,
                    side_icon: template
                        .as_ref()
                        .map(|template| template.get_side_icon_image().to_string())
                        .unwrap_or_default(),
                    score_screen_image: template
                        .as_ref()
                        .map(|template| template.get_score_screen().to_string())
                        .unwrap_or_default(),
                    is_local: self.local_player_id() == Some(player.id),
                    is_observer: player.is_observer,
                    is_human: player.is_human,
                    is_neutral: player.map_side.role == PlayerSideRole::Neutral,
                    is_active: player.is_alive,
                    has_objects,
                    list_in_score_screen: player.list_in_score_screen,
                    relationship_to_local,
                    rank_points: player.skill_points,
                    score_keeper: player.statistics.score_keeper().clone(),
                })
            })
            .collect()
    }

    pub(crate) fn score_screen_is_single_player(&self) -> bool {
        self.game_mode() == super::GameMode::SinglePlayer
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn score_screen_session_difficulty(&self) -> Option<i32> {
        self.session_difficulty.map(|difficulty| match difficulty {
            crate::ai::AIDifficulty::Easy => 0,
            crate::ai::AIDifficulty::Medium => 1,
            crate::ai::AIDifficulty::Hard => 2,
            crate::ai::AIDifficulty::Brutal => 3,
        })
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn score_screen_is_skirmish(&self) -> bool {
        self.game_mode() == super::GameMode::Skirmish
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn score_screen_skirmish_outcome(
        &self,
    ) -> Option<game_client::gui::callbacks::score_screen::ScoreScreenSkirmishOutcome> {
        use game_client::gui::callbacks::score_screen::ScoreScreenSkirmishOutcome;

        // C++ GameInfo::isSandbox is defined by its occupied slots and team
        // numbers. Player rows cannot reproduce empty-slot, observer or
        // unadmitted-slot semantics, so omit the honor decision if this match
        // has no receiving slot admission.
        let admission = self.initial_skirmish_admission.as_ref()?;
        let local_slot = admission.local_slot? as usize;
        let local_input = admission
            .config
            .slots
            .iter()
            .find(|slot| slot.slot_index == local_slot)?;
        let sandbox = !admission.config.slots.iter().any(|slot| {
            slot.slot_index != local_slot
                && slot.is_active
                && (slot.team < 0 || slot.team != local_input.team)
        });

        let local_id = self.local_player_id();
        let local = local_id.and_then(|id| self.players.get(&id));
        Some(ScoreScreenSkirmishOutcome {
            sandbox,
            allied_victory: self.victory_conditions.is_local_allied_victory_owned(
                &self.players,
                self.player_template_definitions.as_ref(),
                local_id,
            ),
            allied_defeat: self.victory_conditions.is_local_allied_defeat_owned(
                &self.players,
                self.player_template_definitions.as_ref(),
                local_id,
            ),
            local_player_active: local.is_some_and(|player| player.is_alive),
        })
    }

    pub(crate) fn score_academy_context(&self) -> AcademyAdviceContext {
        AcademyAdviceContext {
            frame: self.frame,
            // Input preference is app-owned; capture it once with match observations.
            use_alternate_mouse: game_engine::common::global_data::read_safe()
                .map(|data| data.use_alternate_mouse)
                .unwrap_or(false),
            local_player_mines: self
                .local_player_id()
                .and_then(|id| self.players.get(&id))
                .map(|p| p.academy_stats.get_mines())
                .unwrap_or(0),
            neutral_vehicles_sniped: self
                .players
                .values()
                .filter(|p| p.is_reserved_neutral())
                .min_by_key(|p| p.id)
                .map(|p| p.academy_stats.get_vehicles_sniped())
                .unwrap_or(0),
        }
    }

    pub(crate) fn score_academy_advice(&self) -> AcademyAdviceInfo {
        let mut advice = AcademyAdviceInfo::default();
        if let Some(player) = self.local_player_id().and_then(|id| self.players.get(&id)) {
            player
                .academy_stats
                .calculate_academy_advice_with_context(&mut advice, self.score_academy_context());
        }
        advice
    }
}

#[cfg(test)]
mod receiving_roster_relationship_tests {
    use super::*;
    use crate::game_logic::{Player, Team};
    use gamelogic::common::Relationship;

    #[test]
    fn score_roster_uses_player_relation_only_for_a_live_default_team() {
        let mut world = GameLogic::new();
        let mut local =
            Player::new(1, Team::USA, "Local", true).with_script_identity("LocalRosterPlayer");
        local.set_map_relationship(2, Relationship::Allies);
        local.set_map_relationship(3, Relationship::Enemies);
        world.add_player(local);
        world.add_player(
            Player::new(2, Team::China, "Target", false).with_script_identity("TargetRosterPlayer"),
        );
        assert!(world.select_local_player(Some(1)));
        let target_team = world.host_default_team_id(2).expect("admitted target Team");
        let target_team_arc = world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(target_team)
            .expect("admitted target Team");
        target_team_arc
            .write()
            .unwrap()
            .set_controlling_player_id(Some(3));

        let relation = |world: &GameLogic| {
            world
                .score_screen_player_roster()
                .into_iter()
                .find(|player| player.id == 2)
                .unwrap()
                .relationship_to_local
        };
        assert_eq!(relation(&world), Relationship::Enemies);

        // Team-specific overrides have precedence over playerRelations.
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_team_relationship_override(target_team, Relationship::Allies);
        assert_eq!(relation(&world), Relationship::Allies);
        world
            .players
            .get_mut(&1)
            .unwrap()
            .remove_team_relationship_override(target_team);

        // C++ Player::getRelationship(nullptr) is Neutral, even when the
        // Player relationship map contains entries for the Player or current
        // Team controller.
        world.players.get_mut(&2).unwrap().default_team_id = None;
        assert_eq!(relation(&world), Relationship::Neutral);
    }

    #[test]
    fn observer_score_roster_uses_first_human_and_current_team_controller() {
        let mut world = GameLogic::new();
        let mut observer = Player::new(0, Team::USA, "Observer", false)
            .with_script_identity("ObserverRosterPlayer");
        observer.is_observer = true;
        observer.set_map_relationship(3, Relationship::Enemies);
        world.add_player(observer);

        let mut human = Player::new(1, Team::USA, "FirstHuman", true)
            .with_script_identity("FirstHumanRosterPlayer");
        human.set_map_relationship(3, Relationship::Allies);
        world.add_player(human);
        world.add_player(
            Player::new(2, Team::China, "Target", false).with_script_identity("TargetRosterPlayer"),
        );
        world.add_player(
            Player::new(3, Team::GLA, "Controller", false)
                .with_script_identity("ControllerRosterPlayer"),
        );
        assert!(world.select_local_player(Some(0)));
        let target_team = world.host_default_team_id(2).expect("target default Team");
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(target_team)
            .unwrap()
            .write()
            .unwrap()
            .set_controlling_player_id(Some(3));

        let target = world
            .score_screen_player_roster()
            .into_iter()
            .find(|player| player.id == 2)
            .unwrap();
        assert_eq!(target.relationship_to_local, Relationship::Allies);
    }

    #[test]
    fn skirmish_sandbox_uses_admitted_occupied_slot_team_numbers() {
        fn sandbox(opponent_team: i32, opponent_occupied: bool) -> bool {
            let mut world = GameLogic::new();
            world.add_player(
                Player::new(0, Team::USA, "Local", true).with_script_identity("LocalSlot"),
            );
            let mut config = crate::skirmish_config::golden_skirmish_config("SandboxEvidence");
            config.slots[0].team = 4;
            config.slots[1].team = opponent_team;
            config.slots[1].is_active = opponent_occupied;
            world.initial_skirmish_admission = Some(Box::new(
                crate::skirmish_config::saved_admission::SavedSkirmishAdmission {
                    config,
                    source: None,
                    local_slot: Some(0),
                    local_slot_from_client: false,
                },
            ));
            world.score_screen_skirmish_outcome().unwrap().sandbox
        }

        assert!(sandbox(4, true), "same-team occupied slot is a sandbox");
        assert!(sandbox(-1, false), "empty slots do not break sandbox");
        assert!(!sandbox(-1, true), "occupied unteamed slot breaks sandbox");
        assert!(
            !sandbox(5, true),
            "occupied different-team slot breaks sandbox"
        );
    }

    #[test]
    fn score_outcome_uses_selected_player_id_not_initial_slot_number() {
        let mut world = GameLogic::new();
        world.add_player(
            Player::new(0, Team::USA, "InitialSlot", true)
                .with_script_identity("InitialSlotPlayer"),
        );
        let mut selected =
            Player::new(4, Team::China, "Selected", true).with_script_identity("SelectedPlayer");
        selected.is_alive = false;
        world.add_player(selected);
        assert!(world.select_local_player(Some(4)));

        let config = crate::skirmish_config::golden_skirmish_config("SelectedPlayerOutcome");
        world.initial_skirmish_admission = Some(Box::new(
            crate::skirmish_config::saved_admission::SavedSkirmishAdmission {
                config,
                source: None,
                local_slot: Some(0),
                local_slot_from_client: false,
            },
        ));

        assert!(
            !world
                .score_screen_skirmish_outcome()
                .unwrap()
                .local_player_active
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::game_logic::PlayerSideRole;
    use crate::game_logic::{Player, Team};

    fn world(frame: u32) -> GameLogic {
        let mut world = GameLogic::new();
        world.frame = frame;
        let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
        neutral.map_side.role = PlayerSideRole::Neutral;
        neutral.map_side.map_player_name.clear();
        world.add_player(neutral);
        let mut observer = Player::new(9, Team::Neutral, "ReplayObserver", true);
        observer.map_side.role = PlayerSideRole::ReplayObserver;
        observer.map_side.map_player_name = "ReplayObserver".into();
        observer.is_observer = true;
        observer.academy_stats.record_vehicle_sniped();
        world.add_player(observer);
        let mut local = Player::new(1, Team::USA, "Local", true);
        local.academy_stats.record_mine();
        world.add_player(local);
        world
    }

    #[test]
    fn score_context_uses_reserved_neutral_and_exact_receiving_frame() {
        let mut actual = world(100);
        let mut peer = world(900);
        actual
            .players
            .get_mut(&0)
            .unwrap()
            .academy_stats
            .record_vehicle_sniped();
        let frozen = actual.score_academy_context();
        for _ in 0..4 {
            peer.players
                .get_mut(&0)
                .unwrap()
                .academy_stats
                .record_vehicle_sniped();
        }
        assert_eq!(frozen.frame, 100);
        assert_eq!(frozen.local_player_mines, 1);
        assert_eq!(frozen.neutral_vehicles_sniped, 1);
        assert_eq!(peer.score_academy_context().neutral_vehicles_sniped, 4);
        assert_eq!(actual.score_academy_context(), frozen);
        peer.reset();
        drop(peer);
        assert_eq!(actual.score_academy_context(), frozen);
    }

    #[test]
    fn encoded_save_retains_the_receiving_score_counters() {
        let mut actual = world(100);
        actual
            .players
            .get_mut(&0)
            .unwrap()
            .academy_stats
            .record_vehicle_sniped();
        let expected = actual.score_academy_context();
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&actual).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let mut loaded = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut loaded)
            .unwrap();
        assert_eq!(loaded.score_academy_context(), expected);
    }
}

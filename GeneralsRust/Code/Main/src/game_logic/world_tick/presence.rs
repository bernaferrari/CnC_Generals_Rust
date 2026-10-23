//! Host tick `impl GameLogic` — `presence`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(in super::super) fn ensure_non_shell_player_presence(
        &mut self,
        parsed_settings: Option<&super::super::script_loader::MapMetadata>,
    ) {
        self.ensure_non_shell_player_presence_with_effects(parsed_settings, &mut Default::default())
    }

    pub(in super::super) fn ensure_non_shell_player_presence_with_effects(
        &mut self,
        parsed_settings: Option<&super::super::script_loader::MapMetadata>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if self.game_mode == GameMode::Shell {
            return;
        }
        // Wave 732: free fallback CC/dozer seeding is opt-in only (default fail-closed).
        // Retail maps must supply start objects. Vertical-slice/incomplete catalogs may set
        // GENERALS_RUNTIME_HOST_SEED_START_PRESENCE=1.
        let allow_seed =
            std::env::var_os("GENERALS_RUNTIME_HOST_SEED_START_PRESENCE").is_some_and(|v| {
                let s = v.to_string_lossy();
                !(s.is_empty()
                    || s == "0"
                    || s.eq_ignore_ascii_case("false")
                    || s.eq_ignore_ascii_case("no"))
            });
        if !allow_seed {
            return;
        }

        let mut team_order = Vec::new();
        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for player_id in player_ids {
            let Some(player) = self.players.get(&player_id) else {
                continue;
            };
            if player.team == Team::Neutral || team_order.contains(&player.team) {
                continue;
            }
            team_order.push(player.team);
        }
        if team_order.is_empty() {
            return;
        }

        let default_bounds_min = Vec3::new(-300.0, 0.0, -300.0);
        let default_bounds_max = Vec3::new(300.0, 0.0, 300.0);
        let (bounds_min, bounds_max) = parsed_settings
            .and_then(|meta| {
                meta.world_min.zip(meta.world_max).map(|(min, max)| {
                    (
                        Vec3::new(min.x, min.y, min.z),
                        Vec3::new(max.x, max.y, max.z),
                    )
                })
            })
            .filter(|(min, max)| (max.x - min.x).abs() >= 1.0 && (max.z - min.z).abs() >= 1.0)
            .unwrap_or((default_bounds_min, default_bounds_max));

        let span = bounds_max - bounds_min;
        let spawn_positions = [
            Vec3::new(
                bounds_min.x + span.x * 0.20,
                0.0,
                bounds_min.z + span.z * 0.20,
            ),
            Vec3::new(
                bounds_max.x - span.x * 0.20,
                0.0,
                bounds_max.z - span.z * 0.20,
            ),
            Vec3::new(
                bounds_max.x - span.x * 0.20,
                0.0,
                bounds_min.z + span.z * 0.20,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.20,
                0.0,
                bounds_max.z - span.z * 0.20,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.50,
                0.0,
                bounds_min.z + span.z * 0.15,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.50,
                0.0,
                bounds_max.z - span.z * 0.15,
            ),
        ];

        let mut spawned_count = 0usize;
        for (index, team) in team_order.into_iter().enumerate() {
            let has_presence = self
                .objects
                .registered_values()
                .any(|object| object.team == team && object.is_alive());
            if has_presence {
                continue;
            }

            let mut spawn_position = spawn_positions[index % spawn_positions.len()];
            if let Some(ground_height) =
                self.terrain_height_at(Vec3::new(spawn_position.x, 0.0, spawn_position.z))
            {
                spawn_position.y = ground_height;
            }

            // Ensure faction template catalog (CC, barracks, combat units) exists.
            self.ensure_ai_faction_templates(team);

            let primary_template = match team {
                Team::USA => "AmericaCommandCenter",
                Team::GLA => "GLACommandCenter",
                Team::China => "ChinaCommandCenter",
                Team::Neutral => "CommandCenter",
            };

            if self
                .create_object_with_script_effects(
                    primary_template,
                    team,
                    spawn_position,
                    None,
                    effects,
                )
                .is_none()
            {
                // Fallbacks for incomplete catalogs.
                let _ = self
                    .create_object_with_script_effects(
                        "CommandCenter",
                        team,
                        spawn_position,
                        None,
                        effects,
                    )
                    .or_else(|| {
                        self.create_object_with_script_effects(
                            "GoldenCC",
                            team,
                            spawn_position,
                            None,
                            effects,
                        )
                    });
            }
            // Seed a worker for the first human-capable team so DozerConstruct can start.
            if index == 0 {
                let dozer_pos = spawn_position + Vec3::new(20.0, 0.0, 0.0);
                let dozer_name = match team {
                    Team::USA => "USA_Dozer",
                    Team::China => "China_Dozer",
                    Team::GLA => "GLA_Worker",
                    Team::Neutral => "GoldenDozer",
                };
                if self
                    .create_object_with_script_effects(dozer_name, team, dozer_pos, None, effects)
                    .is_none()
                {
                    // Install a minimal dozer if faction worker missing.
                    if !self.templates.contains_key("GoldenDozer") {
                        let mut d = ThingTemplate::new("GoldenDozer");
                        d.set_health(300.0);
                        d.set_cost(1000, 0);
                        d.add_kind_of(KindOf::Vehicle);
                        d.add_kind_of(KindOf::Worker);
                        d.add_kind_of(KindOf::Dozer);
                        d.add_kind_of(KindOf::Selectable);
                        self.templates.insert("GoldenDozer".into(), d);
                    }
                    let _ = self.create_object_with_script_effects(
                        "GoldenDozer",
                        team,
                        dozer_pos,
                        None,
                        effects,
                    );
                }
            }
            spawned_count += 1;
        }

        if spawned_count > 0 {
            log::info!(
                "Seeded {} fallback player start structures for non-shell map '{}'",
                spawned_count,
                self.map_name
            );
        }
    }

    pub(in super::super) fn configure_victory_rules_for_map(&mut self, map_name: &str) {
        // C++ GameLogic.cpp:1606 startNewGame always calls
        // TheVictoryConditions->setVictoryConditions(VICTORY_NOBUILDINGS)
        // after the map is loaded. Skirmish is short-game: razing every
        // KINDOF_STRUCTURE that counts for victory defeats that player
        // even if units remain. Campaign maps keep scripted / INI rules.
        let rules = if matches!(self.game_mode, GameMode::Skirmish) {
            VictoryType::NO_BUILDINGS
        } else {
            victory_rules_for_map(map_name)
        };
        self.victory_conditions.set_victory_conditions(rules);
        log::info!(
            "Configured victory rules for map '{}': require units = {}, require buildings = {}",
            map_name,
            rules.requires_units(),
            rules.requires_buildings()
        );
    }

    /// C++ `Player::killPlayer` after `VictoryConditions` marks a slot dead:
    /// evacuate containers, Neutral-transfer KINDOF_TECH_BUILDING, kill army
    /// (including beacons), then wipe money unless single-player computer.
    pub(crate) fn kill_player_for_victory(&mut self, player_id: u32) {
        self.kill_player_for_victory_with_effects(player_id, &mut Default::default())
    }

    pub(crate) fn kill_player_for_victory_with_effects(
        &mut self,
        player_id: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.kill_player_with_interpreter_with_effects(player_id, None, effects);
    }

    pub(crate) fn kill_player_with_interpreter(
        &mut self,
        player_id: u32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) {
        self.kill_player_with_interpreter_with_effects(
            player_id,
            interpreter,
            &mut Default::default(),
        )
    }

    pub(crate) fn kill_player_with_interpreter_with_effects(
        &mut self,
        player_id: u32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let player_team = self.players.get(&player_id).map(|p| p.team);
        let unique_team = player_team.and_then(|team| {
            let mut found = None;
            for p in self.players.values() {
                if p.team != team || team == Team::Neutral {
                    continue;
                }
                if found.is_some() {
                    return None;
                }
                found = Some(p.id);
            }
            found
        });
        let owned = |obj: &Object| match obj.owner_player_id {
            Some(owner) => owner == player_id,
            None => unique_team == Some(player_id) && player_team == Some(obj.team),
        };

        let container_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, obj)| owned(obj) && !obj.contained_units().is_empty())
            .map(|(id, _)| *id)
            .collect();
        for container_id in container_ids {
            let _ = self.evacuate_container_now_with_effects(container_id, false, effects);
        }

        if let Some(player) = self.players.get_mut(&player_id) {
            player.is_alive = false;
        }

        let neutral_owner = self
            .players
            .values()
            .find(|p| {
                p.team == Team::Neutral
                    && !p.is_observer
                    && !p.name.to_ascii_lowercase().contains("observer")
            })
            .map(|p| p.id);

        let army: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, obj)| owned(obj))
            .map(|(id, _)| *id)
            .collect();
        for id in army {
            let is_tech = self
                .objects
                .get(&id)
                .is_some_and(|obj| obj.is_kind_of(KindOf::TechBuilding));
            if is_tech {
                // C++ Team::killTeam: KINDOF_TECH_BUILDING → Neutral default team.
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_team_and_owner(
                        Team::Neutral,
                        neutral_owner,
                        &mut self.ownership_events,
                    );
                }
                continue;
            }
            // C++ Team::killTeam keeps beacons in the kill list (effectively-dead
            // exception) and kill()s them. Do not name-sniff skip.
            self.host_script_damage_with_interpreter_with_effects(
                id,
                0.0,
                true,
                interpreter,
                effects,
            );
        }

        self.clear_player_drawable_selection(player_id);

        // C++ Player.cpp:2055-2063 — SP PLAYER_COMPUTER resurrect before cash wipe.
        if self.should_resurrect_sp_computer_player(player_id) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.is_alive = true;
                player.selected_objects.clear();
            }
            return;
        }

        if let Some(player) = self.players.get_mut(&player_id) {
            player.is_alive = false;
            player.resources.supplies = 0;

            player.selected_objects.clear();
        }
    }

    /// Player.cpp2055: the killed Player's own controller, independent of
    /// locality, observer presentation, strategic AI, or another session.
    /// GameCommon.h119 has exactly PLAYER_HUMAN and PLAYER_COMPUTER.
    pub(crate) fn should_resurrect_sp_computer_player(&self, player_id: u32) -> bool {
        self.is_single_player_scenario()
            && self
                .players
                .get(&player_id)
                .is_some_and(|player| !player.is_human)
    }

    pub(in super::super) fn convert_script_event(
        &self,
        event: &ScriptEvent,
    ) -> Option<MissionScriptEvent> {
        use ScriptValue as Val;
        let mut params = HashMap::new();
        let event_type = match event {
            ScriptEvent::PlayerDefeated { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "player_defeated"
            }
            ScriptEvent::AllianceStateChanged { player_id, state } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "state".into(),
                    Val::String(format!("{:?}", state).to_lowercase()),
                );
                "alliance_state_changed"
            }
            ScriptEvent::RevealMapForPlayer { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "reveal_map_for_player"
            }
            ScriptEvent::CompletedSpecialPower {
                player_id,
                special_power_name,
                creator_id,
            } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "special_power".into(),
                    Val::String(special_power_name.clone()),
                );
                params.insert("creator_id".into(), Val::String(creator_id.to_string()));
                "completed_special_power"
            }
        };

        Some(MissionScriptEvent {
            event_type: event_type.to_string(),
            parameters: params,
            timestamp: std::time::Instant::now(),
            priority: ScriptPriority::Normal,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, GameMode, KindOf, Player, Team, ThingTemplate};

    #[test]
    fn victory_kill_destroys_beacons() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let mut beacon = ThingTemplate::new("AmericaBeacon");
        beacon.add_kind_of(KindOf::Immobile).set_health(1.0);
        logic.templates.insert("AmericaBeacon".into(), beacon);
        let id = logic
            .create_object("AmericaBeacon", Team::USA, glam::Vec3::ZERO)
            .expect("beacon");
        logic.kill_player_for_victory(0);
        assert!(
            logic.host_object(id).is_none()
                || logic
                    .host_object(id)
                    .is_some_and(|o| !o.is_alive() || o.status.destroyed),
            "C++ Team::killTeam destroys beacons on killPlayer"
        );
    }

    fn host_sp_player_with_cash(
        id: u32,
        human: bool,
        local: bool,
    ) -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        logic.game_mode = GameMode::SinglePlayer;
        let mut player = Player::new(id, Team::GLA, "GLA", human);
        player.is_local = local;
        player.resources.supplies = 4_000;
        logic.add_player(player);
        let mut ranger = ThingTemplate::new("GLARebel");
        ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert("GLARebel".into(), ranger);
        let obj_id = logic
            .create_object("GLARebel", Team::GLA, glam::Vec3::ZERO)
            .expect("unit");
        (logic, obj_id)
    }

    #[test]
    fn sp_computer_kill_preserves_cash() {
        for local in [false, true] {
            let (mut logic, id) = host_sp_player_with_cash(1, false, local);
            logic.kill_player_for_victory(1);
            assert!(
                logic.host_object(id).is_none()
                    || logic
                        .host_object(id)
                        .is_some_and(|o| !o.is_alive() || o.status.destroyed)
            );
            let p = logic.get_player(1).expect("computer");
            assert!(p.is_alive, "C++ resurrects SP PLAYER_COMPUTER");
            assert_eq!(p.resources.supplies, 4_000, "SP AI keeps cash");
        }
    }

    #[test]
    fn sp_human_kill_wipes_cash_independently_of_locality() {
        for local in [false, true] {
            let (mut logic, _) = host_sp_player_with_cash(1, true, local);
            assert!(!logic.should_resurrect_sp_computer_player(1));
            logic.kill_player_for_victory(1);
            let p = logic.get_player(1).expect("human");
            assert!(!p.is_alive);
            assert_eq!(p.resources.supplies, 0);
        }
    }

    #[test]
    fn same_id_peer_controller_never_selects_victory_resurrection() {
        let (mut computer, _) = host_sp_player_with_cash(1, false, false);
        let (mut human, _) = host_sp_player_with_cash(1, true, false);
        assert!(computer.should_resurrect_sp_computer_player(1));
        assert!(!human.should_resurrect_sp_computer_player(1));
        assert!(!computer.should_resurrect_sp_computer_player(999));
        human.kill_player_for_victory(1);
        assert!(computer.should_resurrect_sp_computer_player(1));
        computer.kill_player_for_victory(1);
        assert!(computer.get_player(1).unwrap().is_alive);
        assert_eq!(computer.get_player(1).unwrap().resources.supplies, 4_000);
        assert!(!human.get_player(1).unwrap().is_alive);
        assert_eq!(human.get_player(1).unwrap().resources.supplies, 0);
        drop(human);
        assert!(computer.should_resurrect_sp_computer_player(1));
        computer.game_mode = GameMode::Skirmish;
        assert!(!computer.should_resurrect_sp_computer_player(1));
        computer.kill_player_for_victory(1);
        assert!(!computer.get_player(1).unwrap().is_alive);
        assert_eq!(computer.get_player(1).unwrap().resources.supplies, 0);
    }
    #[test]
    fn encoded_player_controller_preserves_victory_continuation() {
        use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};
        let mut source = GameLogic::new();
        source.game_mode = GameMode::SinglePlayer;
        for (id, human) in [(1, false), (2, true)] {
            let mut player = Player::new(id, Team::GLA, "Shared display", human);
            player.is_local = false;
            player.resources.supplies = 4_000;
            source.add_player(player);
        }
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let encoded = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
        let mut receiver = GameLogic::new();
        // Match mode is supplied by the receiving load session, not this DTO.
        receiver.game_mode = GameMode::SinglePlayer;
        // Admit the receiving roster before validating saved Team prototypes.
        // Opposite controllers prove restoration, rather than constructor defaults.
        receiver.add_player(Player::new(1, Team::GLA, "Receiver display", true));
        receiver.add_player(Player::new(2, Team::GLA, "Receiver display", false));
        builder
            .restore_from_snapshot(&decoded, &mut receiver)
            .unwrap();
        assert!(!receiver.get_player(1).unwrap().is_human);
        assert!(receiver.get_player(2).unwrap().is_human);
        assert!(receiver.should_resurrect_sp_computer_player(1));
        assert!(!receiver.should_resurrect_sp_computer_player(2));
        for id in [1, 2] {
            receiver.kill_player_for_victory(id);
        }
        assert!(receiver.get_player(1).unwrap().is_alive);
        assert_eq!(receiver.get_player(1).unwrap().resources.supplies, 4_000);
        assert!(!receiver.get_player(2).unwrap().is_alive);
        assert_eq!(receiver.get_player(2).unwrap().resources.supplies, 0);
        assert!(source.get_player(2).unwrap().is_alive);
        assert_eq!(source.get_player(2).unwrap().resources.supplies, 4_000);
    }
    #[test]
    fn actual_recorded_single_player_replay_resurrects_only_computer_and_keeps_cash() {
        use game_engine::common::recorder::{Recorder, RecorderOwnerInputs};
        // Original GameLogic::isInSinglePlayerGame includes playback's saved
        // game mode. Actual owned replay files calibrate both SP0 and Skirmish2.
        for (recorded_mode, human) in [(0, false), (0, true), (2, false), (2, true)] {
            let directory = tempfile::tempdir().unwrap();
            let inputs = RecorderOwnerInputs {
                map: "Maps/PresenceReplay.map".into(),
                seed: 19,
                replay_directory: directory.path().join("Replays"),
                exe_crc: 0,
                ini_crc: 0,
                save_stats: false,
                stats_directory: directory.path().join("Stats"),
            };
            let mut writer = Recorder::new_owned(inputs.clone());
            writer.start_recording(0, recorded_mode, 0, 30).unwrap();
            let filename = format!(
                "{}{}",
                writer.last_replay_filename(),
                writer.replay_extension()
            );
            writer.stop_recording();
            let (mut logic, id) = host_sp_player_with_cash(1, human, false);
            logic.replay_pending.reset_recorder_owned(inputs);
            assert!(
                logic
                    .replay_pending
                    .playback_file_owned(filename, false)
                    .unwrap()
            );
            logic.game_mode = GameMode::Replay;
            assert!(logic.replay_pending.recorder_owned().is_playback());
            assert_eq!(
                logic.replay_pending.recorder_owned().get_game_mode(),
                recorded_mode
            );
            let resurrect = recorded_mode == 0 && !human;
            assert_eq!(logic.should_resurrect_sp_computer_player(1), resurrect);
            logic.kill_player_for_victory(1);
            assert!(
                logic.host_object(id).is_none()
                    || logic
                        .host_object(id)
                        .is_some_and(|actor| !actor.is_alive() || actor.status.destroyed),
                "killTeam still runs before the playback resurrection decision"
            );
            let player = logic.get_player(1).unwrap();
            assert_eq!(player.is_alive, resurrect);
            assert_eq!(player.resources.supplies, if resurrect { 4_000 } else { 0 });
        }
    }
}

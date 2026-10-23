//! PlayerList.cpp97–139: neutral0, then exact named sides in declaration order.
//! Admit identity before any default Team or singleton creation callback.
use super::*;
impl GameLogic {
    pub(in crate::game_logic) fn prepared_side_identity_is_complete(&self) -> bool {
        if self.prepared_map_sides.get_num_sides() == 0 {
            return false;
        }
        (0..self.prepared_map_sides.get_num_sides()).all(|index| {
            let Some(side) = self.prepared_map_sides.get_side_info(index) else {
                return false;
            };
            let name = side.get_dict().get_ascii_string(key_player_name());
            self.players
                .values()
                .any(|player| player.script_identity() == Some(name.as_str()))
        })
    }
    pub(in crate::game_logic) fn admit_direct_map_players_from_prepared_sides(
        &mut self,
    ) -> gamelogic::GameLogicResult<()> {
        let invalid = |reason: &str| gamelogic::GameLogicError::Configuration(reason.into());
        if let Some(error) = self.object_retirement_error() {
            return Err(error);
        }
        if !self.host_objects().registered_is_empty() || self.has_pending_object_removals() {
            return Err(invalid(
                "direct map Player admission requires the retired outgoing Object roster",
            ));
        }
        let rows = (0..self.prepared_map_sides.get_num_sides())
            .map(|index| {
                self.prepared_map_sides
                    .get_side_info(index)
                    .expect("prepared side ordinal")
                    .get_dict()
                    .clone()
            })
            .collect::<Vec<_>>();
        let mut names = std::collections::HashSet::new();
        let mut candidates = Vec::new();
        let mut bindings = Vec::new();
        let mut local = None;
        let mut next = 1u32;
        let mut neutral = Player::new(0, Team::Neutral, "", false);
        neutral.map_side.role = PlayerSideRole::Neutral;
        neutral.map_side.map_player_name.clear();
        neutral.resources.supplies = 0;
        neutral.is_local = false;
        neutral
            .map_side
            .relations
            .insert(0, gamelogic::common::Relationship::Allies);
        candidates.push(neutral);
        for row in &rows {
            let name = row.get_ascii_string(key_player_name());
            if name.is_empty() {
                continue;
            }
            if !names.insert(name.clone()) {
                return Err(invalid("duplicate authored map playerName"));
            }
            if next as usize >= gamelogic::common::MAX_PLAYER_COUNT {
                return Err(invalid("authored map exceeds original PlayerList capacity"));
            }
            let faction = row.get_ascii_string(key_player_faction());
            let identity = PlayerTemplateIdentity::from_exact_name(&faction).ok_or_else(|| {
                invalid("authored map PlayerFaction has no exact receiving PlayerTemplate")
            })?;
            let team = identity
                .base_team()
                .ok_or_else(|| invalid("authored map PlayerTemplate has no supported base side"))?;
            let human = row.get_bool(key_player_is_human());
            let mut player = Player::new(
                next,
                team,
                &row.get_unicode_string(key_player_display_name()),
                human,
            );
            player.apply_map_side_identity_from_dict(row);
            player.resources.supplies = self.skirmish_rules.starting_cash;
            player.is_local = false;
            let local_key = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                "multiplayerIsLocal",
            );
            if row.get_bool(local_key) || (local.is_none() && human) {
                local = Some(next)
            }
            bindings.push((next, identity));
            candidates.push(player);
            next += 1;
        }
        let fallback_human = if local.is_none() {
            candidates
                .iter()
                .find(|player| !player.is_reserved_neutral())
                .map(|player| player.id)
        } else {
            None
        };
        let local = local.or(fallback_human);
        // Every input identity/template/capacity is resolved before replacing
        // the inert records. No Object or native lifetime is dropped here.
        // PlayerList::newGame clears the OLD TeamFactory before init/new
        // Players. Preserve the old controller records until this teardown.
        {
            let mut factory = self.team_factory.lock().map_err(|_| {
                invalid("receiving TeamFactory is unavailable before Player admission")
            })?;
            for team in factory.get_all_teams() {
                team.read()
                    .map_err(|_| invalid("receiving Team is unavailable before Player admission"))?
                    .validate_creation_snapshot()
                    .map_err(|reason| invalid(&reason))?;
            }
            factory.reset();
        }
        self.clear_all_players();
        for mut player in candidates {
            player.is_local = Some(player.id) == local;
            self.players.insert(player.id, player);
        }
        for (id, identity) in bindings {
            if !self.bind_player_template_identity(id, identity) {
                return Err(invalid(
                    "prepared receiving PlayerTemplate changed during map admission",
                ));
            }
        }
        // Existing implementation owns handicap/color/deposit/relationship
        // order; invoke it ONCE, after template initialization, not before it.
        self.apply_host_players_from_side_dicts(&rows, true);
        if let Some(id) = fallback_human {
            self.players
                .get_mut(&id)
                .expect("admitted fallback human")
                .is_human = true;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(name: &str, human: bool, money: i32) -> Dict {
        let mut row = Dict::new();
        row.set_ascii_string(key_player_name(), name);
        row.set_ascii_string(key_player_faction(), "FactionAmerica");
        row.set_unicode_string(key_player_display_name(), "Shared Display");
        row.set_bool(key_player_is_human(), human);
        row.set_int(key_player_start_money(), money);
        row
    }
    #[test]
    fn prepared_authored_rows_admit_exact_players_before_teams_with_neutral_last_and_human_fallback()
     {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "prepared_authored_rows_admit_exact_players_before_teams_with_neutral_last_and_human_fallback",
            || {
                crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
                let mut owner = GameLogic::new();
                owner.set_session_starting_cash(10_000);
                let peer = GameLogic::new();
                for row in [
                    row("FirstAuthored", false, 41),
                    row("SecondAuthored", false, 73),
                    row("", false, 0),
                ] {
                    owner.prepared_map_sides.add_side(&row);
                }
                owner
                    .admit_direct_map_players_from_prepared_sides()
                    .unwrap();
                assert!(owner.prepared_side_identity_is_complete());
                assert_eq!(owner.host_player_id_for_side_index(0), Some(1));
                assert_eq!(owner.host_player_id_for_side_index(1), Some(2));
                assert_eq!(owner.host_player_id_for_side_index(2), Some(0));
                assert_eq!(
                    owner.get_player(1).unwrap().script_identity(),
                    Some("FirstAuthored")
                );
                assert_eq!(
                    owner.get_player(2).unwrap().script_identity(),
                    Some("SecondAuthored")
                );
                assert!(owner.get_player(1).unwrap().is_human);
                assert!(owner.get_player(1).unwrap().is_local);
                assert!(!owner.get_player(2).unwrap().is_human);
                assert!(!owner.get_player(2).unwrap().is_local);
                assert_eq!(owner.get_player(1).unwrap().resources.supplies, 10_041);
                assert_eq!(owner.get_player(2).unwrap().resources.supplies, 10_073);
                let neutral = owner.get_player(0).unwrap();
                assert_eq!(neutral.script_identity(), Some(""));
                assert_eq!(neutral.map_side.map_player_name, "");
                assert_eq!(
                    neutral.map_side.relations.get(&0),
                    Some(&gamelogic::common::Relationship::Allies)
                );
                assert_eq!(neutral.resources.supplies, 0);
                owner.ensure_host_default_team(1);
                owner.ensure_host_default_team(2);
                let factory = owner.team_factory.lock().unwrap();
                for (id, name) in [(1, "teamFirstAuthored"), (2, "teamSecondAuthored")] {
                    let teams = factory.find_team_instances(name);
                    assert_eq!(teams.len(), 1);
                    assert_eq!(
                        teams[0].read().unwrap().get_controlling_player_id(),
                        Some(id)
                    );
                }
                assert!(factory.find_team_prototype("teamRustPlayer1").is_none());
                assert!(peer.get_players().is_empty());
            },
        );
    }
    #[test]
    fn explicit_map_local_overrides_first_human_without_changing_second_player_type() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "explicit_map_local_overrides_first_human_without_changing_second_player_type",
            || {
                crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
                let mut owner = GameLogic::new();
                let first = row("FirstAuthored", true, 0);
                let mut second = row("SecondAuthored", false, 0);
                second.set_bool(
                    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                        "multiplayerIsLocal",
                    ),
                    true,
                );
                for row in [first, second, row("", false, 0)] {
                    owner.prepared_map_sides.add_side(&row);
                }
                owner
                    .admit_direct_map_players_from_prepared_sides()
                    .unwrap();
                assert!(!owner.get_player(1).unwrap().is_local);
                assert!(owner.get_player(1).unwrap().is_human);
                assert!(owner.get_player(2).unwrap().is_local);
                assert!(!owner.get_player(2).unwrap().is_human);
            },
        );
    }
}

#[cfg(test)]
mod reordered_civilian_tests {
    use super::*;
    #[test]
    fn retry_reordered_authored_civilians_retires_old_default_teams_before_player_reinitialization()
    {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "retry_reordered_authored_civilians_retires_old_default_teams_before_player_reinitialization",
            || {
                crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
                let mut owner = GameLogic::new();
                owner.set_session_starting_cash(10_000);
                let peer = GameLogic::new();
                let row = |name: &str, money| {
                    let mut row = Dict::new();
                    row.set_ascii_string(key_player_name(), name);
                    row.set_ascii_string(key_player_faction(), "FactionCivilian");
                    row.set_unicode_string(key_player_display_name(), "Shared Civilian Display");
                    row.set_int(key_player_start_money(), money);
                    row
                };
                for row in [row("CivilianA", 11), row("CivilianB", 22), row("", 0)] {
                    owner.prepared_map_sides.add_side(&row);
                }
                owner
                    .admit_direct_map_players_from_prepared_sides()
                    .unwrap();
                assert_eq!(
                    owner.get_player(1).unwrap().script_identity(),
                    Some("CivilianA")
                );
                assert_eq!(
                    owner.get_player(2).unwrap().script_identity(),
                    Some("CivilianB")
                );
                {
                    let factory = owner.team_factory.lock().unwrap();
                    assert_eq!(
                        factory.find_team_instances("teamCivilianA")[0]
                            .read()
                            .unwrap()
                            .get_controlling_player_id(),
                        Some(1)
                    );
                }
                owner.prepared_map_sides.clear();
                for row in [row("CivilianB", 22), row("CivilianA", 11), row("", 0)] {
                    owner.prepared_map_sides.add_side(&row);
                }
                owner
                    .admit_direct_map_players_from_prepared_sides()
                    .unwrap();
                assert_eq!(
                    owner.get_player(1).unwrap().script_identity(),
                    Some("CivilianB")
                );
                assert_eq!(
                    owner.get_player(2).unwrap().script_identity(),
                    Some("CivilianA")
                );
                assert_eq!(owner.get_player(1).unwrap().resources.supplies, 10_022);
                assert_eq!(owner.get_player(2).unwrap().resources.supplies, 10_011);
                let factory = owner.team_factory.lock().unwrap();
                for (name, id) in [("teamCivilianB", 1), ("teamCivilianA", 2)] {
                    let teams = factory.find_team_instances(name);
                    assert_eq!(teams.len(), 1);
                    assert_eq!(
                        teams[0].read().unwrap().get_controlling_player_id(),
                        Some(id)
                    );
                }
                assert!(peer.get_players().is_empty());
            },
        );
    }
}

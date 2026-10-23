//! C++ SidesList preparation and host admission on the receiving map owner.
use super::*;

impl GameLogic {
    pub(crate) fn prepared_map_sides(&self) -> &gamelogic::sides_list::SidesList {
        &self.prepared_map_sides
    }

    fn prepare_map_sides(
        &self,
        side_dicts: &[Dict],
        team_dicts: &[Dict],
        side_builds: &[super::script_loader::SideBuildEntry],
        script_lists: &[ScriptList],
    ) -> gamelogic::sides_list::SidesList {
        let mut sides = gamelogic::sides_list::SidesList::new();
        for dict in side_dicts {
            sides.add_side(dict);
        }
        for dict in team_dicts {
            sides.add_team(dict);
        }

        let mut pos_by_side: HashMap<u32, i32> = HashMap::new();
        for entry in side_builds {
            let pos = *pos_by_side.get(&entry.side_index).unwrap_or(&0);
            let mut build = gamelogic::build_list_info::BuildListInfo::new();
            build.set_building_name(gamelogic::common::AsciiString::from(
                entry.building_name.as_str(),
            ));
            build.set_template_name(gamelogic::common::AsciiString::from(
                entry.template.as_str(),
            ));
            build.set_location(gamelogic::common::Coord3D::new(
                entry.position.x,
                entry.position.y,
                0.0,
            ));
            build.set_angle(entry.angle);
            build.set_initially_built(entry.initially_built);
            build.set_num_rebuilds(entry.num_rebuilds.max(0) as u32);
            if let Some(script) = &entry.script_name {
                build.set_script(gamelogic::common::AsciiString::from(script.as_str()));
            }
            if let Some(health) = entry.health {
                build.set_health(health);
            }
            if let Some(whiner) = entry.whiner {
                build.set_whiner(whiner);
            }
            if let Some(unsellable) = entry.unsellable {
                build.set_unsellable(unsellable);
            }
            if let Some(repairable) = entry.repairable {
                build.set_repairable(repairable);
            }
            if let Some(side) = sides.get_side_info_mut(entry.side_index as usize) {
                side.add_to_build_list(build, pos);
                pos_by_side.insert(entry.side_index, pos + 1);
            }
        }

        for (index, scripts) in script_lists.iter().enumerate() {
            if let Some(side) = sides.get_side_info_mut(index) {
                side.set_script_list(Some(Box::new(scripts.clone())));
            }
        }

        sides.validate_sides();

        if self.uses_lobby_side_foundation() {
            sides.prepare_for_mp_or_skirmish();
            self.add_host_players_as_sides(&mut sides);
            sides.validate_sides();
        }
        sides
    }

    pub(in super::super) fn sync_legacy_sides_list_from_dicts(
        &mut self,
        side_dicts: &[Dict],
        team_dicts: &[Dict],
        side_builds: &[super::script_loader::SideBuildEntry],
        script_lists: &[ScriptList],
    ) -> Vec<Dict> {
        // Capture one immutable catalog view at the actual definition-input
        // boundary. Player and AI queries never rediscover this process store.
        let catalog = {
            let templates = get_player_template_store();
            (0..templates.get_player_template_count())
                .filter_map(|index| {
                    templates
                        .get_nth_player_template_signed(index)
                        .map(|template| {
                            (
                                template.get_name().to_string(),
                                template.get_side().to_string(),
                            )
                        })
                })
                .collect::<HashMap<_, _>>()
        };
        self.sync_legacy_sides_list_from_dicts_with_controller_catalog(
            side_dicts,
            team_dicts,
            side_builds,
            script_lists,
            &catalog,
        )
    }

    pub(crate) fn sync_legacy_sides_list_from_dicts_with_controller_catalog(
        &mut self,
        side_dicts: &[Dict],
        team_dicts: &[Dict],
        side_builds: &[super::script_loader::SideBuildEntry],
        script_lists: &[ScriptList],
        catalog: &HashMap<String, String>,
    ) -> Vec<Dict> {
        // Preparation may load faction scripts. Neither that IO nor host admission
        // needs to hold, or discover its owner through, the compatibility registry.
        let prepared = self.prepare_map_sides(side_dicts, team_dicts, side_builds, script_lists);
        let rows: Vec<_> = (0..prepared.get_num_sides())
            .filter_map(|i| {
                prepared
                    .get_side_info(i)
                    .map(|side| side.get_dict().clone())
            })
            .collect();
        // PlayerList.cpp110–121 skips the unnamed row after independently creating
        // neutral and admits every retained named Civilian. Skirmish skips the
        // later authored-player overwrite, so these owners must be admitted here.
        if !self.players.is_empty() && self.uses_lobby_side_foundation() {
            // PlayerList::newGame reconstructs named map owners from this map.
            // Keep lobby/neutral/replay identities, but do not retain a prior map's
            // authored owners (or relationship references to their reused IDs).
            let retired: Vec<_> = self
                .players
                .iter()
                .filter_map(|(&id, player)| {
                    (player.map_side.role == PlayerSideRole::Authored
                        && !rows.iter().any(|row| {
                            row.get_ascii_string(key_player_name())
                                == player.map_side.map_player_name
                        }))
                    .then_some(id)
                })
                .collect();
            for id in &retired {
                self.players.remove(id);
                self.player_template_bindings.remove(id);
            }
            for player in self.players.values_mut() {
                player
                    .map_side
                    .relations
                    .retain(|id, _| !retired.contains(id));
                player
                    .team_instance_player_relations
                    .retain(|_, relations| {
                        relations.retain(|id, _| !retired.contains(id));
                        !relations.is_empty()
                    });
            }
            self.admit_retained_map_civilians(&rows);
        }
        // An empty/campaign host still uses the later map-player admission path.
        // Creating only civilians here would make its roster-preservation check
        // skip that path. Complete map-player reconstruction remains separate.

        // TeamFactory::initFromSides consumes this exact prepared order, not
        // the raw map dictionaries or a later ambient SidesList selection.
        let team_definitions = (0..prepared.get_num_teams())
            .filter_map(|index| {
                prepared
                    .get_team_info(index)
                    .map(|team| team.get_dict().clone())
            })
            .collect();
        self.prepared_player_controllers =
            crate::game_logic::player_controllers::PreparedControllers::capture(&prepared, catalog);
        for player in self.players.values_mut() {
            if let Some(controller) = self
                .prepared_player_controllers
                .for_side(&player.map_side.map_player_name)
            {
                controller.apply(player);
            }
        }
        self.prepared_map_sides = prepared;
        team_definitions
    }
}

#[cfg(test)]
#[path = "replay_authored_side_tests.rs"]
mod replay_authored_side_tests;

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
            match self.owned_script_engine.as_ref() {
                Some(engine) => {
                    let templates =
                        gamelogic::scripting::chunk_codec::ScriptTemplateLookup::from_engine(engine);
                    sides.prepare_for_mp_or_skirmish_with_definitions(
                        templates,
                        &self.world_services.common().name_key_namespace,
                    );
                }
                None => sides.prepare_for_mp_or_skirmish(),
            }
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
        general_name: Option<&str>,
    ) -> Vec<Dict> {
        // The prepared controller sides must come from this world's definitions.
        let collect =
            |templates: &game_engine::common::rts::player_template::PlayerTemplateStore| {
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
        let catalog = match &self.player_template_definitions {
            Some(templates) => collect(templates),
            None => collect(&get_player_template_store()),
        };
        self.sync_legacy_sides_list_from_dicts_with_controller_catalog(
            side_dicts,
            team_dicts,
            side_builds,
            script_lists,
            &catalog,
            general_name,
        )
    }

    pub(crate) fn sync_legacy_sides_list_from_dicts_with_controller_catalog(
        &mut self,
        side_dicts: &[Dict],
        team_dicts: &[Dict],
        side_builds: &[super::script_loader::SideBuildEntry],
        script_lists: &[ScriptList],
        catalog: &HashMap<String, String>,
        general_name: Option<&str>,
    ) -> Vec<Dict> {
        // Preparation may load faction scripts. Neither that IO nor host admission
        // needs to hold, or discover its owner through, the compatibility registry.
        let mut prepared = self.prepare_map_sides(side_dicts, team_dicts, side_builds, script_lists);
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
            }
            // Team overrides are owned by the Team, not copied onto Players.
            // Remove references to retired Player IDs before those IDs can be
            // reused by the next map's authored owners.
            if !retired.is_empty() {
                let teams = self
                    .team_factory
                    .with_read_without_flushing(|factory| factory.get_all_teams())
                    .unwrap_or_else(|error| error.into_inner());
                for team in teams {
                    let mut team = team.write().unwrap_or_else(|error| error.into_inner());
                    for id in &retired {
                        team.remove_override_player_relationship(*id as i32);
                    }
                }
            }
            self.admit_retained_map_civilians(&rows, general_name);
        }
        // An empty/campaign host still uses the later map-player admission path.
        // Creating only civilians here would make its roster-preservation check
        // skip that path. Complete map-player reconstruction remains separate.

        let controllers =
            crate::game_logic::player_controllers::PreparedControllers::capture(&prepared, catalog);
        // Player::initFromDict runs inside PlayerList::newGame, before
        // TeamFactory::initFromSides and ScriptEngine::newMap read the sides.
        if self.uses_lobby_side_foundation() {
            // Key accessors resolve through this namespace; lock it only to
            // spell the generic hook keys.
            let hook_keys = key_team_generic_script_hook_keys(&self.world_services);
            qualify_skirmish_side_scripts(&mut prepared, &controllers, catalog, &hook_keys);
        }

        // TeamFactory::initFromSides consumes this exact prepared order, not
        // the raw map dictionaries or a later ambient SidesList selection.
        let team_definitions = (0..prepared.get_num_teams())
            .filter_map(|index| {
                prepared
                    .get_team_info(index)
                    .map(|team| team.get_dict().clone())
            })
            .collect();
        self.prepared_player_controllers = controllers;
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

/// `TheKey_teamGenericScriptHook` + 0..MAX_GENERIC_SCRIPTS (Player.cpp:950-960)
/// in this world's NameKey namespace.
fn key_team_generic_script_hook_keys(
    services: &gamelogic::system::engine_stores::WorldServices,
) -> Vec<game_engine::common::name_key_generator::NameKeyType> {
    let base_key = game_engine::common::well_known_keys::key_team_generic_script_hook();
    let mut names = services
        .common()
        .name_key_namespace
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let base = names.key_to_name(base_key).unwrap_or_default();
    (0..gamelogic::team::MAX_GENERIC_SCRIPTS)
        .map(|hook| names.name_to_key(&format!("{base}{hook}")))
        .collect()
}

/// C++ Player::initFromDict (Player.cpp:823-966): a human copies the Civilian
/// skirmish scripts; a skirmish AI copies its side's skirmish scripts and
/// teams, qualified by its multiplayer start index.
fn qualify_skirmish_side_scripts(
    prepared: &mut gamelogic::sides_list::SidesList,
    controllers: &crate::game_logic::player_controllers::PreparedControllers,
    catalog: &HashMap<String, String>,
    generic_hook_keys: &[game_engine::common::name_key_generator::NameKeyType],
) {
    use game_engine::common::well_known_keys::{
        key_team_all_clear_script, key_team_enemy_sighted_script,
        key_team_on_create_script, key_team_on_destroyed_script, key_team_on_idle_script,
        key_team_on_unit_destroyed_script, key_team_production_condition,
    };
    if prepared.get_num_skirmish_sides() == 0 {
        return;
    }
    let skirmish_source = |sides: &gamelogic::sides_list::SidesList, side: &str| {
        (0..sides.get_num_skirmish_sides()).find(|&index| {
            sides
                .get_skirmish_side_info(index)
                .and_then(|source| {
                    catalog.get(&source.get_dict().get_ascii_string(key_player_faction()))
                })
                .is_some_and(|source_side| source_side == side)
        })
    };
    for side_index in 0..prepared.get_num_sides() {
        let Some(row) = prepared
            .get_side_info(side_index)
            .map(|side| side.get_dict().clone())
        else {
            continue;
        };
        let pname = row.get_ascii_string(key_player_name());
        let Some(class) = controllers.for_side(&pname) else {
            continue;
        };
        if class.human {
            // Player.cpp:823-851 qualifies with m_mpStartIndex before line 859
            // reads it from the dict: a fresh Player's 0.
            let Some(source) = skirmish_source(prepared, "Civilian") else {
                continue;
            };
            let source_info = prepared.get_skirmish_side_info(source).expect("found source");
            let qual = format!(
                "{}{}",
                source_info.get_dict().get_ascii_string(key_player_name()),
                0
            );
            let Some(scripts) = source_info
                .get_script_list()
                .map(|scripts| scripts.duplicate_and_qualify("0", &qual, &pname))
            else {
                continue;
            };
            if let Some(side) = prepared.get_side_info_mut(side_index) {
                side.set_script_list(Some(scripts));
            }
            if let Some(source_info) = prepared.get_skirmish_side_info_mut(source) {
                source_info.set_script_list(None);
            }
            continue;
        }
        if !class.skirmish {
            continue;
        }
        let Some(my_side) = catalog.get(&row.get_ascii_string(key_player_faction())) else {
            continue;
        };
        let Some(source) = skirmish_source(prepared, my_side) else {
            continue;
        };
        let start_index = row.get_int(key_multiplayer_start_index());
        let qualifier = start_index.to_string();
        let (original_player_name, scripts) = {
            let source_info = prepared.get_skirmish_side_info(source).expect("found source");
            let original = source_info.get_dict().get_ascii_string(key_player_name());
            let qual = format!("{original}{start_index}");
            let scripts = source_info
                .get_script_list()
                .map(|scripts| scripts.duplicate_and_qualify(&qualifier, &qual, &pname));
            (original, scripts)
        };
        if let Some(side) = prepared.get_side_info_mut(side_index) {
            side.set_script_list(scripts);
        }
        let mut team_index = 0;
        while team_index < prepared.get_num_teams() {
            let owned = prepared
                .get_team_info(team_index)
                .is_some_and(|team| team.get_dict().get_ascii_string(key_team_owner()) == pname);
            if owned {
                prepared.remove_team(team_index);
            } else {
                team_index += 1;
            }
        }
        for team_index in 0..prepared.get_num_skirmish_teams() {
            let Some(team) = prepared.get_skirmish_team_info(team_index) else {
                continue;
            };
            if team.get_dict().get_ascii_string(key_team_owner()) != original_player_name {
                continue;
            }
            let mut team_dict = team.get_dict().clone();
            let new_name = format!(
                "{}{}",
                team_dict.get_ascii_string(key_team_name()),
                start_index
            );
            if prepared.find_team_info(&new_name).is_some() {
                continue;
            }
            team_dict.set_ascii_string(key_team_owner(), pname.clone());
            team_dict.set_ascii_string(key_team_name(), new_name);
            let mut keys = vec![
                key_team_on_create_script(),
                key_team_on_idle_script(),
                key_team_on_unit_destroyed_script(),
                key_team_on_destroyed_script(),
                key_team_enemy_sighted_script(),
                key_team_all_clear_script(),
                key_team_production_condition(),
            ];
            keys.extend_from_slice(generic_hook_keys);
            for key in keys {
                if team_dict.get_type(key).is_some() {
                    let value = team_dict.get_ascii_string(key);
                    if !value.is_empty() {
                        team_dict.set_ascii_string(key, format!("{value}{start_index}"));
                    }
                }
            }
            prepared.add_team(&team_dict);
        }
    }
}

#[cfg(test)]
#[path = "replay_authored_side_tests.rs"]
mod replay_authored_side_tests;

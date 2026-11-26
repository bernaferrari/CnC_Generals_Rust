//! AIPlayer::buildStructureNow for an admitted campaign BuildList placement.
use super::*;
use game_engine::common::{dict::Dict, name_key_generator::NameKeyGenerator, well_known_keys::*};

impl GameLogic {
    /// BuildAssistant clears the site with the exact owning Player before
    /// the free authored structure is placed.
    pub(crate) fn build_prepared_campaign_initial_structure(
        &mut self,
        player: u32,
        row_index: usize,
        authored: &HostAuthoredBuild,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<Option<ObjectId>> {
        if !authored.initially_built
            || self
                .get_player(player)
                .is_none_or(|player| player.is_human || player.map_side.is_skirmish_ai)
        {
            return Ok(None);
        }
        let Some(row_token) = self
            .ai_manager
            .ai_players
            .get_mut(&player)
            .and_then(|ai| ai.reserve_campaign_initial_build_row(row_index, authored))
        else {
            return Ok(None);
        };
        let epoch = self.visual_world_epoch;
        let position = Vec3::new(
            authored.position.0,
            authored.position.2,
            authored.position.1,
        );
        self.prepare_campaign_build_object_now(
            player,
            &authored.template,
            position,
            authored.angle,
            effects,
        )?;
        let Some(id) = self.create_prepared_instant_structure_with_effects(
            &authored.template,
            player,
            position,
            authored.angle,
            None,
            effects,
        )?
        else {
            return Ok(None);
        };
        let Some(placed) = self.host_object(id) else {
            return Ok(None);
        };
        let birth = std::sync::Arc::downgrade(&placed.visual_draw_identity_token);

        // AIPlayer.cpp:434–465. The Player's construction callback and
        // CreateModule::onBuildComplete precede these authored properties.
        // That earlier callback cannot observe this row as bound just yet.
        let completion = &authored.completion;
        let mut properties = Dict::new();
        properties.set_ascii_string(key_object_name(), &completion.name);
        properties.set_ascii_string(
            NameKeyGenerator::name_to_key("objectScriptAttachment"),
            &completion.script,
        );
        properties.set_int(key_object_initial_health(), completion.health_percent);
        properties.set_bool(key_object_unsellable(), completion.unsellable);
        self.apply_update_obj_values_from_map_properties_with_effects(id, &properties, effects)
            .map_err(generals_sim::GameLogicError::Configuration)?;
        if !self.object_birth_matches(id, &birth) {
            return Ok(None);
        }
        if epoch != self.visual_world_epoch {
            return Ok(None);
        }
        let Some(row) = self.ai_manager.ai_players.get_mut(&player).and_then(|ai| {
            ai.building_queue
                .iter_mut()
                .find(|row| row.runtime_visit_token == row_token)
        }) else {
            return Err(generals_sim::GameLogicError::Configuration(
                "campaign initial construction lost its authored BuildList row".into(),
            ));
        };
        row.object_id = Some(id);
        row.is_built = true;
        if let Some(ordinary) = &mut row.ordinary {
            ordinary.object_timestamp = self.frame.wrapping_add(1);
            ordinary.under_construction = false;
        }

        self.set_object_under_construction_with_effects(id, false, effects);
        if !self.object_birth_matches(id, &birth) {
            return Ok(None);
        }
        self.host_object_mut(id)
            .expect("current campaign structure")
            .status
            .reconstructing = false;
        self.apply_researched_player_upgrades_to_object_with_effects(id, effects);
        if !self.object_birth_matches(id, &birth) {
            return Ok(None);
        }
        self.add_receiving_named_script_object(id);
        self.run_owned_object_script_with_effects(&completion.script, id, effects);
        if !self.object_birth_matches(id, &birth) {
            return Ok(None);
        }
        self.check_owned_ai_completed_supply_structure(player, id);
        Ok(Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::DataChunkOutput;
    use gamelogic::scripting::core::{ConditionType, Parameter, ParameterType, ScriptActionType};

    const TEMPLATE: &str = "AmericaPowerPlant";
    const BUILDING: &str = "AuthoredInitialPower";
    const SCRIPT: &str = "ObserveInitialPower";
    const ANGLE: f32 = 1.125;
    const MAP_START_MONEY: i32 = 2_000;

    fn write_initial_build_map(path: &std::path::Path) {
        let mut map = DataChunkOutput::new();
        map.open_data_chunk("HeightMapData", 3);
        for value in [41, 41, 0, 1681] {
            map.write_int(value);
        }
        for _ in 0..1681 {
            map.write_byte(24);
        }
        map.close_data_chunk();

        map.open_data_chunk("SidesList", 3);
        map.write_int(2);
        for (name, human) in [("Human", true), ("Commander", false)] {
            let mut side = Dict::new();
            side.set_ascii_string(key_player_name(), name);
            side.set_unicode_string(key_player_display_name(), name);
            side.set_ascii_string(key_player_faction(), "FactionAmerica");
            side.set_bool(key_player_is_human(), human);
            side.set_int(key_player_start_money(), MAP_START_MONEY);
            map.write_dict(&side);
            map.write_int(i32::from(!human));
            if !human {
                map.write_ascii_string(BUILDING);
                map.write_ascii_string(TEMPLATE);
                for value in [140.0, 160.0, 0.0, ANGLE] {
                    map.write_real(value);
                }
                map.write_byte(1); // InitiallyBuilt
                map.write_int(1); // Rebuild budget is not consumed at startup.
                map.write_ascii_string(SCRIPT);
                map.write_int(37); // InitialHealth percent
                map.write_byte(0); // Whiner
                map.write_byte(1); // Unsellable
                map.write_byte(1); // Repairable
            }
        }
        map.write_int(0); // Default Teams admitted from these actual sides.
        map.open_data_chunk("PlayerScriptsList", 1);
        map.open_data_chunk("ScriptList", 1);
        map.close_data_chunk();
        map.open_data_chunk("ScriptList", 1);
        map.open_data_chunk("Script", 2);
        for field in [SCRIPT, "", "", ""] {
            map.write_ascii_string(field);
        }
        for flag in [1, 0, 1, 1, 1, 1] {
            map.write_byte(flag);
        }
        map.write_int(0);
        map.open_data_chunk("OrCondition", 1);
        map.open_data_chunk("Condition", 4);
        map.write_int(ConditionType::ConditionTrue as i32);
        map.write_name_key(NameKeyGenerator::name_to_key("CONDITION_TRUE"));
        map.write_int(0);
        map.close_data_chunk();
        map.close_data_chunk();
        map.open_data_chunk("ScriptAction", 2);
        map.write_int(ScriptActionType::NamedFlashWhite as i32);
        map.write_name_key(NameKeyGenerator::name_to_key("NAMED_FLASH_WHITE"));
        map.write_int(2);
        for parameter in [
            Parameter::with_string(ParameterType::Unit, BUILDING.into()),
            Parameter::with_int(ParameterType::Int, 2),
        ] {
            map.write_int(parameter.param_type as i32);
            map.write_int(parameter.int_value);
            map.write_real(parameter.real_value);
            map.write_ascii_string(&parameter.string_value);
        }
        for _ in 0..5 {
            map.close_data_chunk();
        }
        std::fs::write(path, map.into_ckmp_bytes()).unwrap();
    }

    #[test]
    fn mounted_prepared_campaign_authored_structure_uses_bound_factory_and_runs_its_script() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "mounted_prepared_campaign_authored_structure_uses_bound_factory_and_runs_its_script",
            || {
                let retail = crate::parallel_harness::RetailWorldDefinitions::load().unwrap();
                let directory = tempfile::tempdir().unwrap();
                let map = directory.path().join("CampaignInitialStructure.map");
                let mut world = GameLogic::new();
                world.with_own_services_mut(|world| {
                    retail.admit(world).unwrap();
                    write_initial_build_map(&map);
                });
                world
                    .start_new_game_in_campaign_session(GameMode::SinglePlayer, true, false)
                    .unwrap();
                assert!(world.load_map(map.to_str().unwrap()));
                let owner = world.get_player(2).unwrap();
                assert_eq!(owner.script_identity(), Some("Commander"));
                let authored = owner.map_side.build_list[0].clone();
                assert_eq!(authored.angle.to_bits(), ANGLE.to_bits());
                assert!(authored.initially_built);
                assert!(
                    world
                        .owned_script_engine
                        .as_ref()
                        .unwrap()
                        .find_script_clone_by_name(SCRIPT)
                        .is_some(),
                    "the map admitted the attached subroutine"
                );
                let ground = world.raw_terrain_ground_height_at(Vec3::new(140.0, 0.0, 160.0));
                let row = &world.ai_manager.ai_players[&2].building_queue[0];
                let structure = row
                    .object_id
                    .expect("newMap built the original authored row");
                let object = world.host_object(structure).unwrap();
                assert_eq!(object.owner_player_id, Some(2));
                assert_eq!(object.producer_id, None);
                assert_eq!(object.get_position(), Vec3::new(140.0, ground, 160.0));
                assert!((object.get_orientation() - ANGLE).abs() < 1e-6);
                assert_eq!(object.name, BUILDING);
                assert!(object.is_script_unsellable());
                assert!((object.health.current - object.health.maximum * 0.37).abs() < 0.01);
                assert!(!object.status.under_construction);
                assert!(object.construction_power_admitted);
                assert_eq!(
                    object.flash_count, 4,
                    "attached subroutine found the newly named Object"
                );
                assert!(world.get_player(2).unwrap().power_produced > 0);
                // Player.cpp:422–435, 1007–1009 adds the authored deposit
                // after the original default. Initial construction is free.
                assert_eq!(
                    world.get_player(2).unwrap().resources.supplies,
                    world.skirmish_rules().starting_cash + MAP_START_MONEY as u32,
                );
                let row = &world.ai_manager.ai_players[&2].building_queue[0];
                assert_eq!(row.object_id, Some(structure));
                assert!(row.is_built);
                assert_eq!(row.rebuild_count, 0);
                assert!(
                    world
                        .build_prepared_campaign_initial_structure(
                            2,
                            0,
                            &authored,
                            &mut Default::default(),
                        )
                        .unwrap()
                        .is_none()
                );
            },
        );
    }
}

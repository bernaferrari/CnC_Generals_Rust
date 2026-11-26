//! AIPlayer ordinary campaign building phase. The Player flags pause its clocks.
use super::economic_owner::ScaffoldAttempt;
use super::*;
use crate::game_logic::ScriptPresentationEffects;
use game_engine::common::ini::AIData;

const LOGIC_SECOND: i32 = LOGIC_FRAMES_PER_SECOND as i32;

impl AIPlayer {
    pub(crate) fn remembered_supply_warehouse(&self) -> Option<ObjectId> {
        self.current_warehouse_id
    }

    pub(crate) fn admit_ordinary_supply_row(&mut self, row: AIBuildingInfo, warehouse: ObjectId) {
        self.building_queue.insert(0, row);
        self.current_warehouse_id = Some(warehouse);
    }
}

impl GameLogic {
    /// C++ AIPlayer.cpp:2699–2723. The receiving PlayerList calls this once
    /// per logic frame, before any ready or queued Team callbacks.
    pub(crate) fn update_owned_ordinary_ai_base_with_effects(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        if !self.get_player(player).is_some_and(|p| p.can_build_base) {
            return Ok(());
        }
        let Some(timing) = self
            .ai_manager
            .ai_players
            .get_mut(&player)
            .and_then(|ai| ai.ordinary_timing.as_mut())
        else {
            return Ok(());
        };
        if !timing.ready_to_build_structure {
            timing.structure_timer = timing.structure_timer.saturating_sub(1);
            if timing.structure_timer <= 0 {
                timing.ready_to_build_structure = true;
                timing.build_delay = 0;
            }
        }
        timing.build_delay = timing.build_delay.saturating_sub(1);
        if timing.build_delay >= 1 {
            return Ok(());
        }
        if timing.ready_to_build_structure {
            let epoch = self.host_visual_world_epoch();
            self.process_owned_ordinary_base_building(
                player,
                now,
                data.catalog().expect("receiving AIData"),
                effects,
            )?;
            if self.host_visual_world_epoch() != epoch {
                return Ok(());
            }
        }
        if let Some(timing) = self
            .ai_manager
            .ai_players
            .get_mut(&player)
            .and_then(|ai| ai.ordinary_timing.as_mut())
        {
            if timing.build_delay < 1 {
                timing.build_delay = 2 * LOGIC_SECOND;
            }
        }
        Ok(())
    }

    /// CPP AIPlayer.cpp:682–818. Visit the canonical BuildList in order and
    /// stop at the first accepted structure. No separate resume prepass.
    fn process_owned_ordinary_base_building(
        &mut self,
        player: u32,
        now: f32,
        rules: &AIData,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let frame = self.get_frame();
        let lost_on = frame.wrapping_add(1);
        let rebuild_frames = (rules.rebuild_delay_seconds as u32).wrapping_mul(LOGIC_SECOND as u32);
        let epoch = self.host_visual_world_epoch();
        let mut index = 0;
        while let Some(row) = self
            .ai_manager
            .ai_players
            .get(&player)
            .and_then(|ai| ai.building_queue.get(index))
        {
            let row_index = index;
            index += 1;
            if !self.templates.contains_key(&row.template_name) {
                continue;
            }
            if let Some(id) = row.object_id {
                if let Some(object) = self.host_object(id) {
                    if self.object_owned_by_player(object, player) {
                        if object.status.under_construction {
                            let builder = object
                                .builder_id
                                .filter(|&id| self.host_object(id).is_some());
                            let position = object.get_position();
                            let builder = builder
                                .or_else(|| self.find_owned_ordinary_dozer(player, position, now));
                            if let Some(builder) = builder {
                                self.resume_construction(&[builder], id)?;
                            }
                            if self.host_visual_world_epoch() != epoch {
                                return Ok(());
                            }
                        }
                        continue;
                    }
                }
                // Scan for a hole only after removal, not after capture. The
                // source keeps the last matching hole in its Object list.
                let hole = if self.host_object(id).is_none() {
                    self.host_objects()
                        .registered_values()
                        .filter(|object| {
                            object.is_rebuild_hole && object.rebuild_spawner_id == Some(id)
                        })
                        .last()
                        .map(|object| object.id)
                } else {
                    None
                };
                let row = &mut self
                    .ai_manager
                    .ai_players
                    .get_mut(&player)
                    .unwrap()
                    .building_queue[row_index];
                row.object_id = hole;
                row.is_built = false;
                Self::ordinary_row_fields(row)?.object_timestamp = lost_on;
                if hole.is_some() {
                    continue;
                }
            }

            let (template_name, position, angle, token) = {
                let ai = self.ai_manager.ai_players.get_mut(&player).unwrap();
                let row = &mut ai.building_queue[row_index];
                let fields = Self::ordinary_row_fields(row)?;
                if fields.object_timestamp != 0 {
                    if fields.object_timestamp.wrapping_add(rebuild_frames) > frame {
                        continue;
                    }
                    fields.object_timestamp = 0;
                }
                let angle = fields.angle;
                if !row.is_buildable() {
                    continue;
                }
                if row.runtime_visit_token == 0 {
                    row.runtime_visit_token = ai.next_building_visit_token;
                    ai.next_building_visit_token = ai
                        .next_building_visit_token
                        .checked_add(1)
                        .expect("AI building token exhaustion");
                }
                (
                    row.template_name.clone(),
                    row.position,
                    angle,
                    row.runtime_visit_token,
                )
            };
            let Some(dozer) = self.find_owned_ordinary_dozer(player, position, now) else {
                continue;
            };
            let mut cost = self.templates[&template_name].build_cost;
            cost.supplies =
                self.modified_build_cost_supplies(player, &template_name, cost.supplies);
            if !self.get_player(player).is_some_and(|p| p.can_afford(&cost)) {
                continue;
            }
            let Some(position) = self.prepare_ordinary_ai_build_site(
                player,
                dozer,
                &template_name,
                position,
                angle,
                effects,
            )?
            else {
                if self.host_visual_world_epoch() != epoch {
                    return Ok(());
                }
                continue;
            };
            if self.host_visual_world_epoch() != epoch {
                return Ok(());
            }
            let structure = if self.templates[&template_name].is_kind_of(KindOf::LineBuild) {
                // BuildAssistant.cpp:347–403 skips Dozer Idle/construct here.
                // The AI's affordability check still precedes this birth, but
                // the immediate factory completes without a cash withdrawal.
                self.create_instant_line_build_object_for_player_with_effects(
                    &template_name,
                    player,
                    position,
                    angle,
                    dozer,
                    effects,
                )?
            } else {
                ScaffoldAttempt {
                    player,
                    token,
                    template_name,
                    position,
                    angle,
                    dozer,
                    cost,
                }
                .execute_with_effects(self, effects)?
            };
            if self.host_visual_world_epoch() != epoch {
                return Ok(());
            }
            if let Some(structure) = structure {
                let frame = self.get_frame();
                let money = self
                    .get_player(player)
                    .map_or(0, |p| p.effective_supplies() as i32);
                let ai = self.ai_manager.ai_players.get_mut(&player).ok_or_else(|| {
                    generals_sim::GameLogicError::Configuration(
                        "ordinary structure lost its AI owner".into(),
                    )
                })?;
                let row = ai
                    .building_queue
                    .iter_mut()
                    .find(|row| row.runtime_visit_token == token)
                    .ok_or_else(|| {
                        generals_sim::GameLogicError::Configuration(
                            "ordinary structure lost its BuildList row during birth".into(),
                        )
                    })?;
                // AIPlayer::buildStructureWithDozer binds only after the
                // immediate arm's Player and BuildComplete callbacks return.
                // Its raw BuildList flag is true even for an instant wall.
                row.object_id = Some(structure);
                row.is_built = false;
                if row.is_buildable() {
                    row.decrement_num_rebuilds();
                }
                let fields = Self::ordinary_row_fields(row)?;
                fields.object_timestamp = frame.wrapping_add(1);
                fields.under_construction = true;
                let timing = ai.ordinary_timing.as_mut().expect("ordinary AI owner");
                timing.ready_to_build_structure = false;
                timing.structure_timer = ordinary_structure_timer(rules, money);
                timing.frame_last_building_built = frame as i32;
                ai.activity_count = ai.activity_count.saturating_add(1);
                break;
            }
        }
        Ok(())
    }

    fn ordinary_row_fields(
        row: &mut AIBuildingInfo,
    ) -> generals_sim::GameLogicResult<&mut OrdinaryBuildListFields> {
        let template = &row.template_name;
        row.ordinary.as_deref_mut().ok_or_else(|| {
            generals_sim::GameLogicError::Configuration(format!(
                "ordinary BuildList row {template} is missing its authored fields"
            ))
        })
    }

    /// AIPlayer::findDozer queues only when no eligible Dozer exists. A busy
    /// constructor can still suppress the replacement order.
    fn find_owned_ordinary_dozer(
        &mut self,
        player: u32,
        position: Vec3,
        now: f32,
    ) -> Option<ObjectId> {
        let repair = self.ai_manager.ai_players.get(&player)?.repair_dozer;
        let readiness = AIPlayer::scan_dozer_readiness(self, player, position, repair);
        if readiness.needs_dozer {
            self.admit_owned_ai_dozer(player, now);
        }
        readiness.available
    }
}

fn ordinary_structure_timer(rules: &AIData, money: i32) -> i32 {
    let mut timer = (rules.structure_seconds * LOGIC_FRAMES_PER_SECOND) as i32;
    let scale = if money < rules.resources_poor {
        rules.structures_poor_mod
    } else if money > rules.resources_wealthy {
        rules.structures_wealthy_mod
    } else {
        1.0
    };
    // Valid authored modifiers are positive. Preserve the unscaled timer
    // when a fixture has no modifier instead of inventing an infinite pause.
    if scale > 0.0 {
        timer = (timer as f32 / scale) as i32;
    }
    timer
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture_content::{FixtureObject, admit_fixture_content_with_actions};

    #[test]
    fn ordinary_linebuild_is_affordable_instant_and_stamps_the_owned_raw_build_row() {
        let mut world = GameLogic::new();
        admit_fixture_content_with_actions(
            &mut world,
            &[
                FixtureObject::new("CampaignWallDozer")
                    .kind_of("VEHICLE DOZER SELECTABLE")
                    .field("CommandSet = CampaignWallDozerSet")
                    .walker()
                    .dozer(),
                FixtureObject::new("CampaignWall")
                    .kind_of("STRUCTURE IMMOBILE LINEBUILD DEFENSIVE_WALL")
                    .field("Geometry = BOX")
                    .field("GeometryMajorRadius = 8")
                    .field("GeometryMinorRadius = 4")
                    .build_cost(250),
            ],
            &[],
            "CommandButton CampaignBuildWall
 Command = DOZER_CONSTRUCT
 Object = CampaignWall
End
             CommandSet CampaignWallDozerSet
 1 = CampaignBuildWall
End
",
        );
        for (id, supplies) in [(1, 249), (2, 950)] {
            let mut player = Player::new(id, Team::China, "Wall commander", false);
            player.resources.supplies = supplies;
            player.can_build_units = false;
            world.add_player(player);
        }
        let mut terrain = game_engine::map_data::MapData::new();
        terrain.width = 26;
        terrain.height = 26;
        terrain.heightmap = vec![32; 26 * 26];
        world
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_geometry(terrain);
        let ground = 20.0;
        let dozer = world
            .create_object_for_player("CampaignWallDozer", 1, Vec3::new(60.0, ground, 100.0))
            .unwrap();
        let peer = world
            .create_object_for_player("CampaignWallDozer", 2, Vec3::new(122.0, ground, 100.0))
            .unwrap();
        let position = Vec3::new(100.0, 5.0, 100.0);
        assert_eq!(world.raw_terrain_ground_height_at(position), ground);
        let angle = 0.825;
        let mut row = AIBuildingInfo::new("CampaignWall".into(), position, 2);
        row.ordinary = Some(Box::new(OrdinaryBuildListFields {
            rally_point_offset: [14.0, 7.0],
            angle,
            initially_built: false,
            whiner: true,
            repairable: true,
            object_timestamp: 0,
            under_construction: false,
            resource_gatherers: [None;
                gamelogic::build_list_info::BuildListInfo::MAX_RESOURCE_GATHERERS],
        }));
        row.completion = Some(AuthoredStructureCompletion {
            name: "BoundOnlyAfterInstantCallback".into(),
            ..Default::default()
        });
        world
            .ai_manager
            .add_ai_player(1, Team::China, AIDifficulty::Medium);
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.building_queue = vec![row];
        ai.ordinary_timing = Some(OrdinaryAiTiming {
            ready_to_build_team: false,
            ready_to_build_structure: true,
            team_timer: 2,
            structure_timer: 0,
            build_delay: 0,
            team_delay: 37,
            team_seconds: 11,
            frame_last_building_built: 1,
            bridge_timer: 0,
        });
        let data = AiDataView::from_world(&world);
        let tick = |world: &mut GameLogic, frame| {
            world.frame = frame;
            world
                .update_owned_ordinary_ai_base_with_effects(
                    1,
                    frame as f32 / LOGIC_FRAMES_PER_SECOND,
                    &data,
                    &mut Default::default(),
                )
                .unwrap();
        };

        tick(&mut world, 7);
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(ai.building_queue[0].object_id, None);
        assert_eq!(ai.building_queue[0].rebuild_count, 0);
        assert_eq!(ai.ordinary_timing.unwrap().team_delay, 37);
        world.get_player_mut(1).unwrap().resources.supplies = 250;
        for frame in 8..=67 {
            tick(&mut world, frame);
        }

        let ai = &world.ai_manager.ai_players[&1];
        let row = &ai.building_queue[0];
        assert_eq!(row.rebuild_count, 1);
        assert_eq!(row.position, position);
        assert!(!row.is_built);
        let fields = row.ordinary.as_deref().unwrap();
        assert_eq!(fields.object_timestamp, 68);
        assert!(fields.under_construction);
        let wall = world
            .host_object(row.object_id.expect("instant authored wall"))
            .unwrap();
        assert_eq!(wall.owner_player_id, Some(1));
        assert_eq!(
            wall.team_instance_id,
            world.get_player(1).unwrap().default_team_id
        );
        assert_eq!(wall.producer_id, Some(dozer));
        assert_eq!(wall.builder_id, None);
        assert_eq!(
            world.host_object(dozer).unwrap().dozer_task_build_target(),
            None
        );
        assert_eq!(
            world.host_object(peer).unwrap().dozer_task_build_target(),
            None
        );
        assert_eq!(
            wall.get_position(),
            Vec3::new(position.x, ground, position.z)
        );
        assert!((wall.get_orientation() - angle).abs() < 0.00001);
        assert_eq!(wall.construction_percent, 1.0);
        assert!(wall.status.under_construction && wall.construction_power_admitted);
        assert_ne!(wall.name, "BoundOnlyAfterInstantCallback");
        let owner = world.get_player(1).unwrap();
        assert_eq!(owner.resources.supplies, 250);
        assert_eq!(owner.statistics.resources_spent(), 250);
        assert_eq!(world.get_player(2).unwrap().resources.supplies, 950);
        let timing = ai.ordinary_timing.unwrap();
        assert_eq!(timing.frame_last_building_built, 67);
        assert_eq!(timing.team_delay, 0);
        assert_eq!(timing.build_delay, 60);
        assert!(!timing.ready_to_build_structure);
    }

    #[test]
    fn ordinary_base_cadence_preserves_a_lost_row_and_builds_the_first_eligible_authored_angle() {
        let mut world = GameLogic::new();
        admit_fixture_content_with_actions(
            &mut world,
            &[
                FixtureObject::new("CampaignDozer")
                    .kind_of("VEHICLE DOZER SELECTABLE")
                    .field("CommandSet = CampaignDozerSet")
                    .walker()
                    .dozer(),
                FixtureObject::new("CampaignPad")
                    .kind_of("STRUCTURE IMMOBILE")
                    .field("Geometry = BOX")
                    .field("GeometryMajorRadius = 8")
                    .field("GeometryMinorRadius = 4")
                    .build_cost(250),
            ],
            &[],
            "CommandButton CampaignBuildPad\n Command = DOZER_CONSTRUCT\n Object = CampaignPad\nEnd\n\
             CommandSet CampaignDozerSet\n 1 = CampaignBuildPad\nEnd\n",
        );
        let mut player = Player::new(1, Team::USA, "CampaignOwner", false);
        player.resources.supplies = 1000;
        player.can_build_base = false;
        player.can_build_units = false;
        world.add_player(player);
        world
            .ai_manager
            .add_ai_player(1, Team::USA, AIDifficulty::Medium);
        let dozer = world
            .create_object_for_player("CampaignDozer", 1, Vec3::new(60.0, 0.0, 100.0))
            .unwrap();
        let position = Vec3::new(100.0, 0.0, 100.0);
        let angle = 0.625;
        let fields = OrdinaryBuildListFields {
            rally_point_offset: [0.0; 2],
            angle,
            initially_built: false,
            whiner: true,
            repairable: true,
            object_timestamp: 0,
            under_construction: false,
            resource_gatherers: [None;
                gamelogic::build_list_info::BuildListInfo::MAX_RESOURCE_GATHERERS],
        };
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.ordinary_timing = Some(OrdinaryAiTiming {
            ready_to_build_team: false,
            ready_to_build_structure: false,
            team_timer: 2,
            structure_timer: 2,
            build_delay: 0,
            team_delay: 0,
            team_seconds: 11,
            frame_last_building_built: 0,
            bridge_timer: 0,
        });
        ai.building_queue = [
            Vec3::new(100.0, 0.0, 165.0),
            position,
            Vec3::new(160.0, 0.0, 100.0),
        ]
        .into_iter()
        .map(|point| {
            let mut row = AIBuildingInfo::new("CampaignPad".into(), point, 2);
            row.ordinary = Some(Box::new(fields.clone()));
            // Base AI visits this row independently of the automatic-pad flag.
            row.automatic_build = false;
            row
        })
        .collect();
        ai.building_queue[0].object_id = Some(ObjectId(10000));
        ai.building_queue[0].is_built = true;
        let data = AiDataView::from_world(&world);

        world.frame = 1;
        world
            .update_owned_ordinary_ai_base_with_effects(
                1,
                1.0 / 30.0,
                &data,
                &mut Default::default(),
            )
            .unwrap();
        let ai = &world.ai_manager.ai_players[&1];
        assert_eq!(ai.ordinary_timing.unwrap().structure_timer, 2);
        assert_eq!(ai.ordinary_timing.unwrap().build_delay, 0);
        assert_eq!(ai.building_queue[0].object_id, Some(ObjectId(10000)));

        world.get_player_mut(1).unwrap().can_build_base = true;
        world.frame = 2;
        world
            .update_owned_ordinary_ai_base_with_effects(
                1,
                2.0 / 30.0,
                &data,
                &mut Default::default(),
            )
            .unwrap();
        let timing = world.ai_manager.ai_players[&1].ordinary_timing.unwrap();
        assert_eq!((timing.structure_timer, timing.build_delay), (1, 60));
        world.frame = 3;
        world
            .update_owned_ordinary_ai_base_with_effects(
                1,
                3.0 / 30.0,
                &data,
                &mut Default::default(),
            )
            .unwrap();

        let ai = &world.ai_manager.ai_players[&1];
        let [missing, built, deferred] = ai.building_queue.as_slice() else {
            panic!("ordinary BuildList should keep all three rows");
        };
        assert_eq!(missing.object_id, None);
        assert_eq!(missing.ordinary.as_deref().unwrap().object_timestamp, 4);
        assert_eq!(missing.rebuild_count, 0);
        assert_eq!(deferred.object_id, None);
        assert_eq!(built.rebuild_count, 1);
        assert_eq!(built.position, position);
        let fields = built.ordinary.as_deref().unwrap();
        assert!(fields.under_construction);
        assert_eq!(fields.object_timestamp, 4);
        let structure = world.host_object(built.object_id.unwrap()).unwrap();
        assert!(structure.status.under_construction);
        assert_eq!(structure.builder_id, Some(dozer));
        assert!((structure.get_orientation() - angle).abs() < 0.00001);
        assert_eq!(world.get_player(1).unwrap().effective_supplies(), 750);
        let timing = ai.ordinary_timing.unwrap();
        assert!(!timing.ready_to_build_structure);
        assert_eq!(timing.frame_last_building_built, 3);
        assert_eq!(timing.build_delay, 60);
        assert_eq!(timing.team_timer, 2);
    }
}

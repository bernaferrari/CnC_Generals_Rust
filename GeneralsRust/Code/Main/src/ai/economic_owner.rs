//! Original base-building effects run with the canonical AI roster installed.
use super::*;
#[cfg(test)]
use gamelogic::object::update::ai_update::dozer_ai_update::DozerTask;

pub(super) enum BaseBuildingAction {
    Wait,
    QueueDozer,
    Scaffold(ScaffoldAttempt),
}

pub(super) struct ScaffoldAttempt {
    pub player: u32,
    pub token: u64,
    pub template_name: String,
    pub position: Vec3,
    pub angle: f32,
    pub dozer: ObjectId,
    pub cost: Resources,
}
impl ScaffoldAttempt {
    pub(super) fn execute(
        &self,
        world: &mut GameLogic,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        self.execute_with_effects(world, &mut Default::default())
    }

    pub(super) fn execute_with_effects(
        &self,
        world: &mut GameLogic,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        let Some(birth) = world.dozer_prepare_construct_owned_with_effects(self.dozer, ui)? else {
            return Ok(None);
        };
        let Some(structure) = world.create_dozer_scaffold_with_effects(
            &self.template_name,
            self.player,
            self.dozer,
            self.position,
            self.angle,
            self.cost,
            ui,
        )?
        else {
            return Ok(None);
        };
        world.dozer_admit_constructed_scaffold_owned(self.dozer, &birth, structure)?;
        Ok(Some(structure))
    }

    #[cfg(test)]
    fn execute_observing(
        &self,
        world: &mut GameLogic,
        observe: impl FnMut(&GameLogic, ObjectId),
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        let Some(birth) = world
            .dozer_prepare_construct_owned_with_effects(self.dozer, &mut Default::default())?
        else {
            return Ok(None);
        };
        let Some(structure) = world.create_dozer_scaffold_observing(
            &self.template_name,
            self.player,
            self.dozer,
            self.position,
            self.cost,
            observe,
        ) else {
            return Ok(None);
        };
        world.dozer_admit_constructed_scaffold_owned(self.dozer, &birth, structure)?;
        Ok(Some(structure))
    }
}

pub(super) struct DozerQueueAttempt {
    pub template: String,
    pub factory: ObjectId,
}
impl DozerQueueAttempt {
    pub(super) fn start(
        world: &mut GameLogic,
        player: u32,
        prepare: impl FnOnce(&mut GameLogic) -> Option<Self>,
    ) -> Option<(Self, bool)> {
        // AIPlayer.cpp:3134–3170 enables unit building for both the
        // isPossibleToMakeUnit factory probe and the paid production order.
        AIPlayer::with_can_build_units(world, player, true, |world| {
            let attempt = prepare(world)?;
            let queued = world.enqueue_production(attempt.factory, attempt.template.clone());
            Some((attempt, queued))
        })
    }
}

impl GameLogic {
    pub(crate) fn update_owned_ai_economy(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
    ) -> gamelogic::GameLogicResult<()> {
        self.update_owned_ai_economy_with_effects(player, now, data, &mut Default::default())
    }

    pub(crate) fn update_owned_ai_economy_with_effects(
        &mut self,
        player: u32,
        now: f32,
        data: &AiDataView,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return Ok(());
        };
        // C++ AIPlayer.cpp:58 m_structureTimer(2): "don't start building
        // structures until frame 1". A frame-0 dozer newTask stamps
        // m_taskOrderFrame=0, which getMostRecentCommand's strict `>` never
        // selects, so the worker would hold the task forever.
        if now < ai.next_building_time || self.frame == 0 {
            return Ok(());
        }
        let repair = ai.repair_dozer;
        let sites = ai.building_queue.iter().filter_map(|pad| pad.object_id).collect();
        self.resume_owned_ai_construction(player, repair, sites, |world| {
            world.admit_owned_ai_dozer(player, now);
        })?;
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return Ok(());
        };
        let action = ai.prepare_base_building_action(self, now, data);
        self.ai_manager.ai_players.insert(player, ai);
        match action {
            BaseBuildingAction::Wait => {}
            BaseBuildingAction::QueueDozer => self.admit_owned_ai_dozer(player, now),
            BaseBuildingAction::Scaffold(attempt) => {
                if let Some(object) = attempt.execute_with_effects(self, ui)? {
                    if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                        ai.finish_scaffold_attempt(&attempt, object);
                    }
                }
            }
        }
        let Some(mut ai) = self.ai_manager.ai_players.remove(&player) else {
            return Ok(());
        };
        ai.sync_build_list_object_status(self, now);
        ai.next_building_time =
            now + ai.scaled_interval_seconds(self, AIPlayer::STRUCTURE_SECONDS, true);
        self.ai_manager.ai_players.insert(player, ai);
        Ok(())
    }

    pub(super) fn resume_owned_ai_construction(
        &mut self,
        player: u32,
        repair: Option<ObjectId>,
        sites: Vec<ObjectId>,
        mut queue_dozer: impl FnMut(&mut Self),
    ) -> gamelogic::GameLogicResult<()> {
        for id in sites {
            let Some(site) = self.host_object(id).filter(|site| {
                self.object_owned_by_player(site, player)
                    && site.is_alive()
                    && site.status.under_construction
            }) else {
                continue;
            };
            let position = site.get_position();
            if let Some(builder) = site.builder_id.and_then(|builder| self.host_object(builder)) {
                if self.object_owned_by_player(builder, player) && !builder.status.disabled_unmanned {
                    let builder_id = builder.id;
                    // The original also sends Resume to the recorded builder.
                    // The action gate rejects a builder already executing Build.
                    let _ = self.resume_construction(&[builder_id], id)?;
                    continue;
                }
                // AISkirmishPlayer.cpp:137–140 releases a captured or unmanned
                // builder even if its old Build task still points to the site.
                self.host_object_mut(id).expect("live AI scaffold").builder_id = None;
            }
            // CPP143–150 starts training before it looks for someone to take
            // over. An available substitute does not cancel the paid order.
            queue_dozer(self);
            if let Some(dozer) = AIPlayer::find_available_dozer(self, player, position, repair) {
                let _ = self.resume_construction(&[dozer], id)?;
            }
        }
        Ok(())
    }

    pub(super) fn admit_owned_ai_dozer(&mut self, player: u32, now: f32) {
        let Some(ai) = self.ai_manager.ai_players.get(&player) else {
            return;
        };
        if ai.dozer_already_queued(self) {
            return;
        }
        let started = DozerQueueAttempt::start(self, player, |world| {
            let mut captured = Vec::new();
            let attempt = world.ai_manager.ai_players.get(&player)?
                .prepare_dozer_queue(world, |index| captured.push(index));
            world.ai_manager.ai_players.get_mut(&player)?
                .clear_factory_captures(captured);
            attempt
        });
        if let Some((attempt, queued)) = started {
            if let Some(ai) = self.ai_manager.ai_players.get_mut(&player) {
                ai.finish_dozer_queue(attempt, queued, now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> (GameLogic, ObjectId) {
        use crate::fixture_content::{FixtureObject, admit_fixture_content_with_actions};
        let mut world = GameLogic::new();
        // AIPlayer::findDozer hands work only to a DOZER with an AIUpdate and
        // its DozerAIInterface; both Objects are authored before NewGame
        // (GameEngine.cpp:428-458).
        admit_fixture_content_with_actions(
            &mut world,
            &[
                FixtureObject::new("OwnedEconomyDozer")
                    .kind_of("VEHICLE DOZER SELECTABLE")
                    .build_cost(500)
                    .build_time(0.001)
                    .walker()
                    .dozer(),
                FixtureObject::new("OwnedEconomyWorker")
                    .kind_of("INFANTRY DOZER HARVESTER SELECTABLE")
                    .build_cost(200)
                    .walker()
                    .module("Behavior = WorkerAIUpdate FixtureWorkerAI\nEnd"),
                FixtureObject::new("OwnedEconomyPad")
                    .kind_of("STRUCTURE")
                    .build_cost(100),
                FixtureObject::new("OwnedEconomyCommandCenter")
                    .kind_of("STRUCTURE COMMANDCENTER")
                    .health(1000.0)
                    .field("CommandSet = OwnedEconomyCommandSet")
                    .module("Behavior = ProductionUpdate Factory\nEnd")
                    .module("Behavior = DefaultProductionExitUpdate Exit\nEnd"),
            ],
            &[],
            "CommandButton OwnedEconomyTrainDozer\n Command = UNIT_BUILD\n Object = OwnedEconomyDozer\nEnd\n\
             CommandButton OwnedEconomyTrainWorker\n Command = UNIT_BUILD\n Object = OwnedEconomyWorker\nEnd\n\
             CommandSet OwnedEconomyCommandSet\n 1 = OwnedEconomyTrainDozer\nEnd\n\
             CommandSet OwnedEconomyWorkerCommandSet\n 1 = OwnedEconomyTrainWorker\nEnd\n",
        );
        for (id, name) in [(1, "BuilderOwner"), (2, "PeerOwner")] {
            let mut player = Player::new(id, Team::USA, name, false);
            player.resources.supplies = 10_000;
            world.add_player(player);
            world.add_ai_opponent(id, Team::USA, AIDifficulty::Medium);
            world
                .ai_manager
                .ai_players
                .get_mut(&id)
                .unwrap()
                .building_queue
                .clear();
        }
        let dozer = world
            .create_object_for_player("OwnedEconomyDozer", 1, Vec3::new(25.0, 0.0, 25.0))
            .unwrap();
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .add_building("OwnedEconomyPad", Vec3::new(100.0, 0.0, 100.0), 1);
        (world, dozer)
    }

    fn attach_factory(world: &mut GameLogic) -> ObjectId {
        let position = Vec3::new(30.0, 0.0, 160.0);
        let factory = world
            .create_object_for_player("OwnedEconomyCommandCenter", 1, position)
            .unwrap();
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.add_building("OwnedEconomyCommandCenter", position, 1);
        ai.building_queue.last_mut().unwrap().object_id = Some(factory);
        factory
    }

    #[test]
    fn mounted_automatic_base_rows_train_for_ferrying_or_reserved_workers() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "mounted_automatic_base_rows_train_for_ferrying_or_reserved_workers",
            || {
                let definitions = crate::parallel_harness::RetailWorldDefinitions::load()
                    .expect("mounted Zero Hour startup definitions");
                let mut world = GameLogic::new();
                world
                    .with_own_services_mut(|world| definitions.admit(world))
                    .unwrap();
                world.add_player(Player::new(0, Team::Neutral, "Neutral", false));
                world.frame = 1;
                let mut factories = Vec::new();
                let mut workers = Vec::new();
                for id in 1..=5 {
                    let mut player = Player::new(id, Team::GLA, &format!("GLA {id}"), false);
                    player.resources.supplies = 400;
                    world.add_player(player);
                    world.add_ai_opponent(id, Team::GLA, AIDifficulty::Medium);
                    let x = id as f32 * 1000.0;
                    let factory_position = Vec3::new(x, 0.0, 100.0);
                    let factory = world
                        .create_object_for_player("GLACommandCenter", id, factory_position)
                        .unwrap();
                    factories.push(factory);
                    if id != 3 {
                        let worker = world
                            .create_object_for_player(
                                "GLAInfantryWorker",
                                id,
                                Vec3::new(x + 100.0, 0.0, 100.0),
                            )
                            .unwrap();
                        assert!(world.host_object(worker).unwrap().worker_ai_update);
                        workers.push(worker);
                    }
                    let ai = world.ai_manager.ai_players.get_mut(&id).unwrap();
                    ai.building_queue.clear();
                    ai.add_building("GLACommandCenter", factory_position, 1);
                    ai.building_queue[0].object_id = Some(factory);
                    if id != 3 {
                        ai.add_layout_building("GLABarracks", Vec3::new(x + 200.0, 0.0, 120.0), 0);
                        let pad = ai.building_queue.last_mut().unwrap();
                        pad.rebuild_count = pad.max_rebuilds;
                        assert!(pad.automatic_build);
                        assert!(!pad.is_buildable());
                    }
                }

                // The neutral map warehouse accepts a real WorkerAI dock
                // order and puts player 1's worker into forced Wanting.
                let warehouse = world
                    .create_object("SupplyDock", Team::Neutral, Vec3::new(1100.0, 0.0, 150.0))
                    .expect("retail neutral SupplyDock");
                assert!(world.unit_command_dock_at_supply_warehouse(workers[0], warehouse));
                assert_eq!(
                    world.host_object(workers[0]).unwrap().supply_truck_state,
                    SupplyTruckState::Wanting
                );
                assert!(
                    world
                        .host_object(workers[0])
                        .unwrap()
                        .supply_truck_force_pending
                );

                // Player 2 owns a lone worker reserved for AI bridge repair.
                world
                    .ai_manager
                    .ai_players
                    .get_mut(&2)
                    .unwrap()
                    .repair_dozer = Some(workers[1]);
                // Player 3 has no live builder and only a completed factory
                // row. Players 4 and 5 have Workers already holding Build.
                for (id, worker) in [(4, workers[2]), (5, workers[3])] {
                    let ongoing = world
                        .create_object_under_construction_for_player(
                            "GLABarracks",
                            id,
                            Vec3::new(id as f32 * 1000.0 + 220.0, 0.0, 180.0),
                            0.0,
                        )
                        .unwrap()
                        .unwrap();
                    assert!(world.resume_construction(&[worker], ongoing).unwrap());
                    assert_eq!(
                        world.host_object(worker).unwrap().dozer_task_build_target(),
                        Some(ongoing),
                    );
                }
                // A captured faction building can consume a GLA player's
                // power. Brownout enables the automatic row's separate
                // queueDozer fallback even when a busy builder exists.
                world
                    .create_object_for_player("AmericaWarFactory", 5, Vec3::new(5300.0, 0.0, 200.0))
                    .unwrap();

                let data = AiDataView::from_world(&world);
                for (index, factory) in factories.into_iter().enumerate() {
                    let id = index as u32 + 1;
                    assert_eq!(
                        world
                            .get_player(id)
                            .unwrap()
                            .has_sufficient_energy_at(world.frame),
                        id != 5,
                    );
                    world.update_owned_ai_economy(id, 0.0, &data).unwrap();
                    let queue = &world
                        .host_object(factory)
                        .unwrap()
                        .building_data
                        .as_ref()
                        .unwrap()
                        .production_queue;
                    let ai = &world.ai_manager.ai_players[&id];
                    let pays_for_builder = id <= 2 || id == 5;
                    let expected = usize::from(pays_for_builder);
                    assert_eq!(
                        queue.len(),
                        expected,
                        "paid worker production for player {id}"
                    );
                    assert_eq!(
                        ai.team_queue.len(),
                        expected,
                        "priority orders for player {id}"
                    );
                    if let Some(queued) = queue.first() {
                        assert_eq!(queued.template_name, "GLAInfantryWorker");
                        let order = &ai.team_queue[0].work_orders[0];
                        assert_eq!(order.factory_id, Some(factory));
                        assert!(!order.is_resource_gatherer);
                    }
                    assert_eq!(
                        world.get_player(id).unwrap().resources.supplies,
                        if pays_for_builder { 200 } else { 400 }
                    );
                    assert!(
                        ai.building_queue
                            .iter()
                            .skip(1)
                            .all(|pad| pad.object_id.is_none()),
                        "automatic pads probe for a builder without selecting a scaffold"
                    );
                }
            },
        );
    }

    #[test]
    fn owned_economic_pass_trains_a_replacement_while_a_survivor_resumes_construction() {
        let (mut world, original) = world();
        let factory = attach_factory(&mut world);
        world.frame = 1;
        let data = AiDataView::from_world(&world);
        world.update_owned_ai_economy(1, 0.0, &data).unwrap();
        let scaffold = world.ai_manager.ai_players[&1].building_queue[0].object_id.unwrap();
        assert_eq!(world.host_object(scaffold).unwrap().builder_id, Some(original));
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9900);

        let survivor = world
            .create_object_for_player("OwnedEconomyDozer", 1, Vec3::new(50.0, 0.0, 120.0))
            .unwrap();
        world.destroy_object(original).unwrap();
        world.process_destroy_list();
        assert!(world.host_object(original).is_none());
        let now = world.ai_manager.ai_players[&1].next_building_time;
        world.frame += 1;
        world.update_owned_ai_economy(1, now, &data).unwrap();
        assert_eq!(
            world.host_object(scaffold).unwrap().builder_id,
            Some(survivor)
        );
        let queue = &world
            .host_object(factory)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue;
        assert_eq!(queue.len(), 1, "a survivor taking over still triggers a replacement");
        assert_eq!(queue[0].template_name, "OwnedEconomyDozer");
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9400);

        let next = world.ai_manager.ai_players[&1].next_building_time;
        world.frame += 1;
        world.update_owned_ai_economy(1, next, &data).unwrap();
        assert_eq!(
            world.host_object(factory).unwrap().building_data.as_ref().unwrap()
                .production_queue.len(),
            1
        );
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9400);
    }

    #[test]
    fn typed_dozer_order_uses_master_head_and_keeps_its_paid_identity_through_map_override() {
        use crate::fixture_content::{FixtureObject, admit_fixture_content_with_actions};
        use game_engine::common::system::kind_of::KindOfMask;
        use game_engine::common::system::DataChunkOutput;

        const FIRST_DECLARED: &str = "AardvarkMasterBuilder";
        const LAST_DECLARED: &str = "ZebraMasterBuilder";
        const FACTORY: &str = "MasterBuilderFactory";
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "typed_dozer_order_uses_master_head_and_keeps_its_paid_identity_through_map_override",
            || {
                for override_dozer_flag in [false, true] {
                    let dir = tempfile::tempdir().unwrap();
                    let file = dir.path().join("MasterBuilderOrder.map");
                    let mut map = DataChunkOutput::new();
                    map.open_data_chunk("HeightMapData", 3);
                    for word in [41, 41, 0, 1681] {
                        map.write_int(word);
                    }
                    for _ in 0..1681 {
                        map.write_byte(0);
                    }
                    map.close_data_chunk();
                    std::fs::write(&file, map.into_ckmp_bytes()).unwrap();
                    if override_dozer_flag {
                        std::fs::write(
                            dir.path().join("Map.ini"),
                            format!("Object {LAST_DECLARED}\nKindOf = -DOZER\nEnd\n"),
                        )
                        .unwrap();
                    }

                    let mut world = GameLogic::new();
                    admit_fixture_content_with_actions(
                        &mut world,
                        &[
                            FixtureObject::new(FIRST_DECLARED)
                                .kind_of("VEHICLE DOZER SELECTABLE")
                                .walker()
                                .dozer()
                                .build_cost(350),
                            FixtureObject::new(LAST_DECLARED)
                                .kind_of("VEHICLE DOZER SELECTABLE")
                                .walker()
                                .dozer()
                                .build_cost(425),
                            FixtureObject::new(FACTORY)
                                .kind_of("STRUCTURE IMMOBILE")
                                .health(1_000.0)
                                .field("CommandSet = MasterBuilderCommands")
                                .module("Behavior = ProductionUpdate Producer\nEnd")
                                .module("Behavior = DefaultProductionExitUpdate Exit\nEnd"),
                        ],
                        &[],
                        &format!(
                            "CommandButton BuildAardvark\n Command = UNIT_BUILD\n Object = {FIRST_DECLARED}\nEnd\n\
                             CommandButton BuildZebra\n Command = UNIT_BUILD\n Object = {LAST_DECLARED}\nEnd\n\
                             CommandSet MasterBuilderCommands\n 1 = BuildAardvark\n 2 = BuildZebra\nEnd\n",
                        ),
                    );
                    world.start_new_game(GameMode::SinglePlayer).unwrap();
                    assert!(world.load_map(file.to_str().unwrap()));
                    let catalog = world.receiving_module_content.as_ref().unwrap();
                    let master = catalog.master_template(LAST_DECLARED).unwrap();
                    let effective = catalog.admitted_template(LAST_DECLARED).unwrap();
                    assert!(master.template().is_kind_of_mask(KindOfMask::DOZER.bits()));
                    assert_eq!(
                        effective.template().is_kind_of_mask(KindOfMask::DOZER.bits()),
                        !override_dozer_flag,
                    );
                    assert_eq!(
                        world.templates[LAST_DECLARED].is_kind_of(KindOf::Dozer),
                        !override_dozer_flag,
                    );

                    let mut player = Player::new(1, Team::USA, "Master Builder Owner", false);
                    player.resources.supplies = 3_000;
                    world.add_player(player);
                    world.add_ai_opponent(1, Team::USA, AIDifficulty::Medium);
                    let position = Vec3::new(120.0, 0.0, 160.0);
                    let factory = world
                        .create_object_for_player(FACTORY, 1, position)
                        .unwrap();
                    let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
                    ai.building_queue.clear();
                    ai.add_building(FACTORY, position, 0);
                    ai.building_queue[0].object_id = Some(factory);
                    assert!(world.constructor_can_build_template(factory, FIRST_DECLARED));
                    assert!(world.constructor_can_build_template(factory, LAST_DECLARED));

                    world.get_player_mut(1).unwrap().set_can_build_units(false);
                    world.admit_owned_ai_dozer(1, 1.0);
                    world.admit_owned_ai_dozer(1, 1.0);
                    let ai = &world.ai_manager.ai_players[&1];
                    assert_eq!(ai.team_queue.len(), 1);
                    let order = &ai.team_queue[0].work_orders[0];
                    assert_eq!(order.template_name, LAST_DECLARED);
                    assert_eq!(order.factory_id, Some(factory));
                    assert_eq!(order.queued_count, 1);
                    assert!(!order.is_resource_gatherer);
                    let production = &world
                        .host_object(factory)
                        .unwrap()
                        .building_data
                        .as_ref()
                        .unwrap()
                        .production_queue;
                    assert_eq!(production.len(), 1);
                    assert_eq!(production[0].template_name, LAST_DECLARED);
                    assert_eq!(world.get_player(1).unwrap().resources.supplies, 2575);
                    assert!(!world.get_player(1).unwrap().can_build_units);

                    world.frame += 1;
                    world.update_production(1.0 / LOGIC_FRAMES_PER_SECOND);
                    let production = &world
                        .host_object(factory)
                        .unwrap()
                        .building_data
                        .as_ref()
                        .unwrap()
                        .production_queue;
                    assert_eq!(production.len(), 1);
                    assert_eq!(production[0].construction_frames, 1);
                    assert_eq!(world.get_player(1).unwrap().resources.supplies, 2575);
                }
            },
        );
    }

    #[test]
    fn owned_dozer_queue_discovers_and_uses_factory_with_unit_building_temporarily_enabled() {
        let (mut world, _) = world();
        world.frame = 1;
        for player in [1, 2] {
            world.get_player_mut(player).unwrap().set_can_build_units(false);
        }
        world.admit_owned_ai_dozer(1, 0.0);
        assert!(!world.get_player(1).unwrap().can_build_units);
        assert!(world.ai_manager.ai_players[&1].team_queue.is_empty());

        let factory = attach_factory(&mut world);
        assert!(!world.constructor_can_build_template(factory, "OwnedEconomyDozer"));
        world.admit_owned_ai_dozer(1, 0.0);

        let building = world.host_object(factory).unwrap().building_data.as_ref().unwrap();
        assert_eq!(building.production_queue.len(), 1);
        assert_eq!(building.production_queue[0].template_name, "OwnedEconomyDozer");
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9500);
        assert_eq!(world.get_player(2).unwrap().resources.supplies, 10_000);
        assert!(!world.get_player(1).unwrap().can_build_units);
        assert!(!world.get_player(2).unwrap().can_build_units);
    }

    #[test]
    fn owned_dozer_queue_does_not_count_a_paid_supply_worker_as_a_builder() {
        let (mut world, _) = world();
        world.frame = 1;
        let factory = attach_factory(&mut world);
        world.host_object_mut(factory).unwrap().set_command_set_override(
            Some("OwnedEconomyWorkerCommandSet".into()),
        );
        assert!(world.enqueue_production(factory, "OwnedEconomyWorker".into()));
        let mut gatherer = AIWorkOrder::new("OwnedEconomyWorker".into(), 1, 100);
        gatherer.is_resource_gatherer = true;
        gatherer.factory_id = Some(factory);
        gatherer.queued_count = 1;
        let request = AITeamQueue::new("Supply gatherer".into(), vec![gatherer], true, world.frame);
        world.ai_manager.ai_players.get_mut(&1).unwrap().team_queue.push_back(request);
        world.admit_owned_ai_dozer(1, 0.0);
        world.admit_owned_ai_dozer(1, 0.0);

        let team_queue = &world.ai_manager.ai_players[&1].team_queue;
        assert_eq!(team_queue.len(), 2);
        assert!(!team_queue[0].work_orders[0].is_resource_gatherer);
        assert!(team_queue[1].work_orders[0].is_resource_gatherer);
        let building = world.host_object(factory).unwrap().building_data.as_ref().unwrap();
        assert_eq!(building.production_queue.len(), 2);
        assert_eq!(building.production_queue[0].template_name, "OwnedEconomyWorker");
        assert_eq!(building.production_queue[1].template_name, "OwnedEconomyWorker");
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9600);
    }

    #[test]
    fn owned_dozer_queue_observes_its_delivery_without_claiming_older_factory_outputs() {
        let (mut world, _) = world();
        let factory = attach_factory(&mut world);
        let template = "OwnedEconomyDozer";
        let finish_one = |world: &mut GameLogic, older: Option<ObjectId>| {
            for _ in 0..4 {
                world.frame += 1;
                world.update_production(1.0 / LOGIC_FRAMES_PER_SECOND);
                if let Some(output) = world.host_objects().registered_values().find(|output| {
                    output.producer_id == Some(factory)
                        && output.template_name == template
                        && Some(output.id) != older
                }) {
                    return output.id;
                }
            }
            panic!("authored factory did not deliver its paid Dozer");
        };
        assert!(world.enqueue_production(factory, template.into()));
        let older = finish_one(&mut world, None);

        world.admit_owned_ai_dozer(1, world.frame as f32 / LOGIC_FRAMES_PER_SECOND);
        let order = &world.ai_manager.ai_players[&1].team_queue[0].work_orders[0];
        assert_eq!(order.factory_id, Some(factory));
        assert_eq!(order.queued_count, 1);
        assert_eq!(order.num_completed, 0);
        assert!(order.observed_unit_ids.is_empty());

        let delivered = finish_one(&mut world, Some(older));
        let order = &world.ai_manager.ai_players[&1].team_queue[0].work_orders[0];
        assert_eq!(order.num_completed, 1);
        assert_eq!(order.queued_count, 0);
        assert_eq!(order.observed_unit_ids, vec![delivered]);
        assert_eq!(world.host_object(older).unwrap().producer_id, Some(factory));
    }

    #[test]
    fn actual_scaffold_factory_observes_canonical_controllers_and_reconciles_reordered_pad() {
        let (mut world, dozer) = world();
        let data = AiDataView::from_world(&world);
        let mut ai = world.ai_manager.ai_players.remove(&1).unwrap();
        let BaseBuildingAction::Scaffold(attempt) =
            ai.prepare_base_building_action(&world, 0.0, &data)
        else {
            panic!("authored priority pad should begin with its available Dozer");
        };
        world.ai_manager.ai_players.insert(1, ai);
        // A synchronous owner effect can reorder the authored list. The
        // continuation identifies its row rather than trusting an old index.
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .building_queue
            .insert(0, AIBuildingInfo::new("OtherPad".into(), Vec3::ZERO, 1));
        let mut observed = false;
        let structure = attempt
            .execute_observing(&mut world, |world, _| {
                assert_eq!(world.ai_manager.ai_players.len(), 2);
                assert_eq!(world.ai_manager.ai_players[&1].player_id, 1);
                assert_eq!(world.ai_manager.ai_players[&2].player_id, 2);
                observed = true;
            })
            .unwrap()
            .unwrap();
        assert!(observed);
        world
            .ai_manager
            .ai_players
            .get_mut(&1)
            .unwrap()
            .finish_scaffold_attempt(&attempt, structure);
        let pads = &world.ai_manager.ai_players[&1].building_queue;
        assert!(pads[0].object_id.is_none());
        assert_eq!(pads[1].object_id, Some(structure));
        assert_eq!(pads[1].rebuild_count, 1);
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9900);
        assert_eq!(
            world.host_object(dozer).unwrap().dozer_tasks.requests[0].target,
            Some(structure)
        );
        assert!(
            world
                .host_object(structure)
                .unwrap()
                .status
                .under_construction
        );
    }

    #[test]
    fn owned_economic_pass_builds_one_paid_scaffold_with_peer_unchanged() {
        let (mut world, dozer) = world();
        let peer = world
            .create_object_for_player("OwnedEconomyDozer", 2, Vec3::new(70.0, 0.0, 100.0))
            .unwrap();
        let data = AiDataView::from_world(&world);
        // AIPlayer.cpp:58: base building starts on frame 1, never frame 0.
        world.frame = 1;
        assert!(world.unit_command_move_to_moving(dozer, Vec3::new(50.0, 0.0, 25.0)));
        assert!(world.unit_command_move_to_moving(peer, Vec3::new(65.0, 0.0, 75.0)));
        let peer_move = world.host_object(peer).unwrap().movement.target_position;
        assert!(peer_move.is_some());
        world.update_owned_ai_economy(1, 0.0, &data).unwrap();
        let scaffold = world.ai_manager.ai_players[&1].building_queue[0]
            .object_id
            .expect("the paying player's own Dozer must start construction");
        assert_eq!(
            world.host_object(dozer).unwrap().dozer_tasks.requests[0].target,
            Some(scaffold)
        );
        let peer = world.host_object(peer).unwrap();
        assert_eq!(peer.movement.target_position, peer_move);
        assert!(peer.dozer_tasks.requests[0].target.is_none());
        assert!(world.ai_manager.ai_players[&2].building_queue.is_empty());
        assert_eq!(world.get_player(1).unwrap().resources.supplies, 9900);
        assert_eq!(world.get_player(2).unwrap().resources.supplies, 10000);
        assert_eq!(world.ai_manager.ai_players.len(), 2);
    }

    #[test]
    fn owned_economy_prefers_the_closest_dozer_without_a_derived_task() {
        let (mut world, far) = world();
        let near = world
            .create_object_for_player("OwnedEconomyDozer", 1, Vec3::new(70.0, 0.0, 100.0))
            .unwrap();
        world.frame = 1;
        assert!(world.unit_command_move_to_moving(near, Vec3::new(65.0, 0.0, 75.0)));
        let moving = world.host_object(near).unwrap();
        assert_eq!(moving.ai_state(), AIState::Moving);
        assert!(moving.dozer_tasks.requests.iter().all(|task| task.target.is_none()));

        let data = AiDataView::from_world(&world);
        world.update_owned_ai_economy(1, 0.0, &data).unwrap();
        let scaffold = world.ai_manager.ai_players[&1].building_queue[0].object_id.unwrap();
        assert_eq!(
            world.host_object(near).unwrap().dozer_tasks.requests[DozerTask::Build as usize].target,
            Some(scaffold),
        );
        assert!(world.host_object(far).unwrap().dozer_tasks.requests[DozerTask::Build as usize].target.is_none());
    }

    #[test]
    fn owned_economy_keeps_registration_order_when_dozer_distances_tie() {
        let (mut world, older) = world();
        let newer = world
            .create_object_for_player("OwnedEconomyDozer", 1, Vec3::new(175.0, 0.0, 175.0))
            .unwrap();
        world.frame = 1;
        let data = AiDataView::from_world(&world);
        world.update_owned_ai_economy(1, 0.0, &data).unwrap();
        let scaffold = world.ai_manager.ai_players[&1].building_queue[0].object_id.unwrap();
        assert_eq!(
            world.host_object(newer).unwrap().dozer_tasks.requests[DozerTask::Build as usize].target,
            Some(scaffold),
        );
        assert!(world.host_object(older).unwrap().dozer_tasks.requests[DozerTask::Build as usize].target.is_none());
    }

    #[test]
    fn owned_economy_leaves_a_closer_worker_on_its_supply_order() {
        let (mut world, dozer) = world();
        let worker = world
            .create_object_for_player("OwnedEconomyWorker", 1, Vec3::new(70.0, 0.0, 100.0))
            .unwrap();
        let dock = world
            .create_object_for_player("OwnedEconomyPad", 1, Vec3::new(50.0, 0.0, 150.0))
            .unwrap();
        world.frame = 1;
        assert!(world.unit_command_return_supplies(worker, dock));
        assert!(world.host_object(worker).unwrap().worker_ai_update);
        let data = AiDataView::from_world(&world);
        world.update_owned_ai_economy(1, 0.0, &data).unwrap();
        let scaffold = world.ai_manager.ai_players[&1].building_queue[0].object_id.unwrap();
        assert_eq!(
            world.host_object(dozer).unwrap().dozer_tasks.requests[DozerTask::Build as usize].target,
            Some(scaffold),
        );
        let active_worker = world.host_object(worker).unwrap();
        assert!(active_worker.supply_truck_force_pending);
        assert_eq!(active_worker.preferred_dock_id, Some(dock));
    }
}

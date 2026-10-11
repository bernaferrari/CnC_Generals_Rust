//! C++-derived timing regressions for Main construction calculations.
use super::super::*;
use super::helpers::*;

#[derive(Clone, Copy)]
struct LowEnergyRates {
    penalty: f32,
    min_speed: f32,
    max_speed: f32,
}

struct LowEnergyConfigGuard {
    global:
        std::sync::Arc<parking_lot::RwLock<game_engine::common::ini::ini_game_data::GlobalData>>,
    previous: LowEnergyRates,
}

impl LowEnergyConfigGuard {
    fn install(rates: LowEnergyRates) -> Self {
        let global = game_engine::common::ini::ini_game_data::ensure_global_data();
        let previous = {
            let data = global.read();
            LowEnergyRates {
                penalty: data.low_energy_penalty_modifier,
                min_speed: data.min_low_energy_production_speed,
                max_speed: data.max_low_energy_production_speed,
            }
        };
        {
            let mut data = global.write();
            data.low_energy_penalty_modifier = rates.penalty;
            data.min_low_energy_production_speed = rates.min_speed;
            data.max_low_energy_production_speed = rates.max_speed;
        }

        Self { global, previous }
    }
}

impl Drop for LowEnergyConfigGuard {
    fn drop(&mut self) {
        let mut data = self.global.write();
        data.low_energy_penalty_modifier = self.previous.penalty;
        data.min_low_energy_production_speed = self.previous.min_speed;
        data.max_low_energy_production_speed = self.previous.max_speed;
    }
}

#[test]
fn unmapped_dozer_hp_gain_uses_energy_adjusted_build_time_frames() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "unmapped_dozer_hp_gain_uses_energy_adjusted_build_time_frames",
        || {
            let _config = LowEnergyConfigGuard::install(LowEnergyRates {
                penalty: 1.0,
                min_speed: 0.5,
                max_speed: 0.8,
            });

            let mut logic = GameLogic::new();
            ensure_test_structure_template(&mut logic);
            ensure_test_dozer_template(&mut logic);
            ensure_test_player_for_team(&mut logic, Team::USA);

            let sid = logic
                .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
                .expect("scaffold");
            if let Some(site) = logic.host_object_mut(sid) {
                site.template_mut().build_time = 10.0; // 300 authored frames
                site.construction_percent = 0.0;
            }
            let did = logic
                .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
                .expect("dozer");
            assert!(logic.resume_construction(&[did], sid));

            let before = logic.host_object(sid).expect("site before").clone();
            let power = logic.compute_player_power_factors();
            let configured_power_factor = power.get(&0).copied();

            // Main host-authoritative production path; one elapsed second is 30 logic frames.
            logic.update_construction(&[sid], 1.0);
            let after = logic.host_object(sid).expect("site after");
            let progress = after.construction_percent - before.construction_percent;
            let health_gain = after.health.current - before.health.current;
            assert_eq!(
                configured_power_factor,
                Some(0.5),
                "empty grid is half speed"
            );

            // C++ calcTimeToBuild divides 300 authored frames by the 0.5 low-power
            // rate, then DozerAIUpdate uses that same 600-frame count for both values.
            // Thus each construction percentage point adds the same fraction of max HP.
            let expected_hp_gain = before.health.maximum * progress;
            assert!(
                (health_gain - expected_hp_gain).abs() < 0.05,
                "C++ uses the energy-adjusted 600-frame duration for percent and HP; progress={progress}, HP gain={health_gain}, expected={expected_hp_gain}"
            );
        },
    );
}

#[test]
fn dozer_tick_truncates_handicap_before_player_template_modifier() {
    use crate::game_logic::game_logic::PlayerTemplateIdentity;
    use game_engine::common::rts::player_template::{
        PlayerTemplate, get_player_template_store_mut,
    };

    // Give the real Main PlayerTemplate lookup an exact test template; Main's
    // production construction call reads this and the Player's handicap.
    game_engine::common::ini::ensure_player_templates_loaded();
    let mut player_template = PlayerTemplate::new("TimingRegressionGeneral".to_string());
    player_template.side = "America".into();
    player_template.base_side = "America".into();
    player_template
        .production_time_changes
        .insert("TestBuilding".into(), -0.05);
    get_player_template_store_mut().add_template(player_template);

    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    ensure_test_dozer_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    logic
        .players
        .get_mut(&0)
        .expect("player")
        .map_side
        .handicap_build_time_buildings = 0.95;
    logic.players.get_mut(&0).expect("player").power_produced = 10;
    assert!(
        logic.bind_player_template_identity(
            0,
            PlayerTemplateIdentity::from_exact_name("TimingRegressionGeneral")
                .expect("exact timing test PlayerTemplate"),
        )
    );

    let sid = logic
        .create_object_under_construction("TestBuilding", Team::USA, glam::Vec3::ZERO)
        .expect("scaffold");
    if let Some(site) = logic.host_object_mut(sid) {
        site.template_mut().build_time = 0.3; // C++ Int(0.3 * 30) = 9 frames
        site.construction_percent = 0.0;
    }
    let did = logic
        .create_object("TestDozer", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer");
    assert!(logic.resume_construction(&[did], sid));

    // C++ assigns to Int after each modifier: 9 * 0.95 = 8, then
    // 8 * 0.95 = 7. One actual Main construction frame is therefore 1/7.
    logic.update_construction(&[sid], 1.0 / 30.0);
    let progress = logic.host_object(sid).expect("site").construction_percent;
    assert!(
        (progress - (1.0 / 7.0)).abs() < 1e-5,
        "Main dozer build tick must use C++ staged frame count 9 -> 8 -> 7; expected 1/7, got {progress}"
    );
}

#[test]
fn low_power_duration_truncates_cpp_non_divisible_frame_count() {
    // C++ Int(buildTime / penaltyRate): 7 authored frames / 0.8 = 8.75,
    // assigned back to Int as 8 before DozerAIUpdate derives either rate.
    assert_eq!(GameLogic::cpp_build_time_frames_after_power(7, 0.8), 8);
    // C++ only clamps a non-positive rate to 0.01; valid positive rates below
    // that threshold remain effective.
    assert_eq!(GameLogic::cpp_build_time_frames_after_power(7, 0.005), 1400);
    assert_eq!(GameLogic::cpp_build_time_frames_after_power(7, 0.0), 700);
}

#[test]
fn authored_build_frame_modifiers_keep_each_cpp_integer_boundary() {
    let cases = [
        // 9 * 0.95 = 8.55 -> 8; 8 * 0.95 = 7.6 -> 7.
        (0.3, 0.95, 0.95, 7),
        // 62 * 1.25 = 77.5 -> 77.
        (2.099, 1.0, 1.25, 77),
        // 150 * 1.5 = 225; 225 * 1.25 = 281.25 -> 281.
        (5.0, 1.5, 1.25, 281),
    ];

    for (seconds, handicap, player_template, cpp_frames) in cases {
        assert_eq!(
            GameLogic::cpp_build_time_frames_from_modifiers(seconds, handicap, player_template,),
            cpp_frames,
            "C++ staged conversion for {seconds}s × {handicap} × {player_template}"
        );
    }
}

#[test]
fn main_power_factors_follow_configured_bounds_and_cpp_order() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "main_power_factors_follow_configured_bounds_and_cpp_order",
        || {
            let cases = [
                // Low-power Min drives the factor below 0.01; CPP float division
                // yields 14999.999... and Int truncates to 14999.
                (
                    LowEnergyRates {
                        penalty: 1.0,
                        min_speed: 0.002,
                        max_speed: 0.005,
                    },
                    0,
                    10,
                    0.002,
                    14_999,
                ),
                // Zero fields stay authoritative, then CPP applies penaltyRate=0.01.
                (
                    LowEnergyRates {
                        penalty: 0.0,
                        min_speed: 0.0,
                        max_speed: 0.0,
                    },
                    0,
                    10,
                    0.01,
                    3_000,
                ),
                // Below-full-power applies Max after Min.
                (
                    LowEnergyRates {
                        penalty: 1.0,
                        min_speed: 0.8,
                        max_speed: 0.2,
                    },
                    0,
                    10,
                    0.2,
                    150,
                ),
                // Full power still applies Min, but does not apply Max.
                (
                    LowEnergyRates {
                        penalty: 1.0,
                        min_speed: 1.5,
                        max_speed: 0.8,
                    },
                    10,
                    10,
                    1.5,
                    20,
                ),
            ];

            for (rates, power_produced, power_consumed, expected_factor, expected_frames) in cases {
                let _config = LowEnergyConfigGuard::install(rates);
                let mut logic = GameLogic::new();
                ensure_test_player_for_team(&mut logic, Team::USA);
                if let Some(player) = logic.players.get_mut(&0) {
                    player.power_produced = power_produced;
                    player.power_consumed = power_consumed;
                }
                let configured_factor = logic.compute_player_power_factors().get(&0).copied();
                let final_frames = configured_factor
                    .map(|factor| GameLogic::cpp_build_time_frames_after_power(30, factor));

                assert_eq!(configured_factor, Some(expected_factor));
                assert_eq!(final_frames, Some(expected_frames));
            }
        },
    );
}

// Actual definition worker -> Main PlayerTemplate identity -> charged queue ->
// completion -> Object creation. Process isolation protects the existing
// immutable Common store without a new fixture mutex or namespace publication.
#[cfg(not(target_arch = "wasm32"))]
fn worker_production_definition() -> game_engine::common::rts::player_template::PlayerTemplate {
    use game_engine::common::ini::INI;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    use game_engine::common::rts::player_template::get_player_template_store;
    NameKeyGenerator::reset();
    let driving_key = NameKeyGenerator::name_to_key("TestInfantry");
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let (template, worker_key) = std::thread::spawn(|| {
        for index in 0..1_000 {
            NameKeyGenerator::name_to_key(&format!("main_worker_padding_{index}"));
        }
        let mut ini = INI::new();
        ini.with_inline_source(
            "PlayerTemplate Owner48WorkerGeneral\n Side = America\n BaseSide = America\n StartMoney = 1000\n\
             ProductionCostChange = TestInfantry -10%\n ProductionCostChange = TestInfantry -20%\n\
             ProductionTimeChange = TestInfantry 50%\n ProductionTimeChange = TestInfantry 25%\n\
             ProductionVeterancyLevel = TestInfantry VETERAN\n ProductionVeterancyLevel = TestInfantry ELITE\n\
             ProductionCostChange = testinfantry -50%\n ProductionTimeChange = testinfantry -50%\n\
             ProductionVeterancyLevel = testinfantry HEROIC\nEnd\n\
             PlayerTemplate Owner48ForeignGeneral\n Side = America\n BaseSide = America\n StartMoney = 1000\n\
             ProductionCostChange = TestInfantry 50%\n ProductionTimeChange = TestInfantry 50%\n\
             ProductionVeterancyLevel = TestInfantry VETERAN\nEnd\n",
            |ini| ini.parse_current_file(),
        ).unwrap();
        let template = get_player_template_store().find_template("Owner48WorkerGeneral").unwrap().clone();
        (template, NameKeyGenerator::name_to_key("TestInfantry"))
    }).join().unwrap();
    assert_ne!(
        worker_key, driving_key,
        "actual INI parse and Main consumption use different namespaces"
    );
    template
}

#[cfg(not(target_arch = "wasm32"))]
fn production_world(identity: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    ensure_test_barracks_template(&mut world);
    ensure_test_infantry_template(&mut world);
    {
        let template = world.templates.get_mut("TestInfantry").unwrap();
        template.build_cost.supplies = 101;
        template.build_time = 2.099;
    }
    world.add_player(Player::new(0, Team::USA, "production owner", true));
    assert!(world.bind_player_template_identity(
        0,
        PlayerTemplateIdentity::from_exact_name(identity).unwrap()
    ));
    world.players.get_mut(&0).unwrap().power_produced = 10;
    let producer = world
        .create_object_for_player("TestBarracks", 0, glam::Vec3::ZERO)
        .unwrap();
    assert!(world.host_object(producer).unwrap().building_data.is_some());
    (world, producer)
}

#[cfg(not(target_arch = "wasm32"))]
fn queued_production_frames(world: &GameLogic, producer: ObjectId) -> u32 {
    let item = &world
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue[0];
    gamelogic::world::entities::production_total_logic_frames(item.total_time, false, 1.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn finish_worker_production(world: &mut GameLogic, producer: ObjectId, already_elapsed: u32) {
    for _ in already_elapsed..76 {
        world.update_production(1.0 / 30.0);
        assert_eq!(
            world
                .host_object(producer)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue
                .len(),
            1
        );
        assert!(
            !world
                .objects
                .values()
                .any(|object| object.template_name == "TestInfantry")
        );
    }
    assert_eq!(
        world
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue[0]
            .construction_frames,
        76
    );
    world.update_production(1.0 / 30.0);
    assert!(
        world
            .host_object(producer)
            .unwrap()
            .building_data
            .as_ref()
            .unwrap()
            .production_queue
            .is_empty()
    );
    let produced = world
        .objects
        .values()
        .find(|object| object.template_name == "TestInfantry")
        .expect("real Main completion creates the unit at frame77");
    assert_eq!(produced.owner_player_id, Some(0));
    assert_eq!(
        produced.experience.level,
        VeterancyLevel::Elite,
        "real Object creation consumes the worker's exact General rank"
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn worker_modifier_handoff_charges_ticks_and_spawns_through_main_production() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "worker_modifier_handoff_charges_ticks_and_spawns_through_main_production",
        || {
            let definition = worker_production_definition();
            let mut core = gamelogic::player::Player::new(0);
            core.init(std::sync::Arc::new(
                gamelogic::player::PlayerTemplate::from_common(&definition),
            ));
            assert_eq!(
                core.get_production_cost_change_percent("TestInfantry"),
                -0.20
            );
            assert_eq!(
                core.get_production_time_change_percent("TestInfantry"),
                0.25
            );
            assert_eq!(
                core.get_production_veterancy_level("TestInfantry"),
                gamelogic::common::VeterancyLevel::Elite
            );
            let (mut world, producer) = production_world("Owner48WorkerGeneral");
            assert_eq!(
                world.modified_build_cost_supplies(0, "testinfantry", 101),
                50
            );
            assert_eq!(
                world.modified_build_cost_supplies(0, "TESTINFANTRY", 101),
                101,
                "case-different missing name defaults"
            );
            assert_eq!(
                world.modified_build_cost_supplies(0, "MissingInfantry", 101),
                101
            );
            assert_eq!(
                world.player_template_production_veterancy(0, "TESTINFANTRY"),
                Some(VeterancyLevel::Rookie)
            );
            let before = world.players[&0].effective_supplies();
            assert!(world.enqueue_production(producer, "TestInfantry".into()));
            assert_eq!(
                world.players[&0].effective_supplies(),
                before - 80,
                "C++ Int(101 * 0.8)"
            );
            assert_eq!(
                world
                    .host_object(producer)
                    .unwrap()
                    .building_data
                    .as_ref()
                    .unwrap()
                    .production_queue[0]
                    .cost
                    .supplies,
                80
            );
            assert_eq!(
                queued_production_frames(&world, producer),
                77,
                "C++ Int(Int(2.099 * 30) * 1.25)"
            );
            finish_worker_production(&mut world, producer, 0);
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn worker_modifier_save_restore_and_foreign_reset_keep_exact_identity() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "worker_modifier_save_restore_and_foreign_reset_keep_exact_identity",
        || {
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            worker_production_definition();
            let (mut source, producer) = production_world("Owner48WorkerGeneral");
            let produced = source
                .create_object_for_player("TestInfantry", 0, glam::Vec3::new(25.0, 0.0, 0.0))
                .unwrap();
            assert_eq!(
                source.host_object(produced).unwrap().experience.level,
                VeterancyLevel::Elite
            );
            source.players.get_mut(&0).unwrap().resources.supplies = 13;
            let retained_identity = source.player_template_identity(0).unwrap().clone();
            let (mut foreign, foreign_producer) = production_world("Owner48ForeignGeneral");
            assert_eq!(
                foreign_producer, producer,
                "different sessions intentionally share ObjectId"
            );
            assert!(foreign.enqueue_production(foreign_producer, "TestInfantry".into()));
            assert_eq!(
                foreign
                    .host_object(foreign_producer)
                    .unwrap()
                    .building_data
                    .as_ref()
                    .unwrap()
                    .production_queue[0]
                    .cost
                    .supplies,
                151
            );
            assert_eq!(queued_production_frames(&foreign, foreign_producer), 93);
            foreign.reset();
            drop(foreign);
            assert_eq!(
                source.player_template_identity(0).unwrap().template_name,
                retained_identity.template_name
            );
            assert_eq!(
                source.modified_build_cost_supplies(0, "TestInfantry", 101),
                80
            );
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            assert_eq!(snapshot.player_template_bindings.len(), 1);
            assert_eq!(
                snapshot.player_template_bindings[0].template_name,
                "Owner48WorkerGeneral"
            );
            let encoded = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
            let mut restored = GameLogic::new();
            restored.templates = source.templates.clone();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            let identity = restored.player_template_identity(0).unwrap();
            assert_eq!(identity.template_name, retained_identity.template_name);
            assert_eq!(
                identity.template_index,
                Some(snapshot.player_template_bindings[0].template_index)
            );
            assert_eq!(
                restored.players[&0].effective_supplies(),
                13,
                "restoring identity does not rerun PlayerTemplate starting cash"
            );
            assert_eq!(
                restored.host_object(produced).unwrap().experience.level,
                VeterancyLevel::Elite
            );
            // This fixture checks saved identity followed by a NEW queue. The
            // independent in-flight duration restore gap is hq-0tat5.
            restored.players.get_mut(&0).unwrap().resources.supplies = 1000;
            assert!(restored.enqueue_production(producer, "TestInfantry".into()));
            assert_eq!(restored.players[&0].effective_supplies(), 920);
            assert_eq!(queued_production_frames(&restored, producer), 77);
            source.reset();
            assert!(
                source.player_template_identity(0).is_none(),
                "reset removes old per-player binding"
            );
            assert_eq!(
                restored.modified_build_cost_supplies(0, "TestInfantry", 101),
                80
            );
        },
    );
}

// hq-0tat5: queue duration is derived from the restored owning player rather
// than adding another duration field to the historical queue record.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_duration_restore_continues_general_queue_at_cpp_frame77() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_duration_restore_continues_general_queue_at_cpp_frame77",
        || {
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            worker_production_definition();
            let (mut source, producer) = production_world("Owner48WorkerGeneral");
            assert!(source.enqueue_production(producer, "TestInfantry".into()));
            for _ in 0..9 {
                source.update_production(1.0 / 30.0);
            }
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = GameLogic::new();
            restored.templates = source.templates.clone();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(
                restored.player_template_identity(0).unwrap().template_name,
                "Owner48WorkerGeneral"
            );
            assert_eq!(
                restored.players[&0].effective_supplies(),
                source.players[&0].effective_supplies()
            );
            assert_eq!(queued_production_frames(&restored, producer), 77);
            let queue = &restored
                .host_object(producer)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue;
            assert_eq!(queue[0].construction_frames, 9);
            assert_eq!(queue[0].cost.supplies, 80);
            // This bounded timing witness operates both sessions at full
            // power. Energy reconstruction is a separate acceptance contract.
            restored.players.get_mut(&0).unwrap().power_produced = 10;
            let (mut foreign, foreign_producer) = production_world("Owner48ForeignGeneral");
            assert_eq!(foreign_producer, producer);
            assert!(foreign.enqueue_production(foreign_producer, "TestInfantry".into()));
            assert_eq!(queued_production_frames(&foreign, foreign_producer), 93);
            foreign.reset();
            drop(foreign);
            finish_worker_production(&mut source, producer, 9);
            finish_worker_production(&mut restored, producer, 9);
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_duration_restore_keeps_upgrade_authored_frames_without_general_or_power() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_duration_restore_keeps_upgrade_authored_frames_without_general_or_power",
        || {
            use crate::game_logic::host_upgrade_rules::register_test_upgrade;
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            const UPGRADE: &str = "Upgrade_Owner48Timing";
            worker_production_definition();
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source("PlayerTemplate Owner48WorkerGeneral\n ProductionTimeChange = Upgrade_Owner48Timing 100%\nEnd\n",
                |ini| ini.parse_current_file()).unwrap();
            let (mut source, producer) = production_world("Owner48WorkerGeneral");
            register_test_upgrade(&source, UPGRADE, "PLAYER", 40, 3);
            let cost = Resources {
                supplies: 40,
                power: 0,
            };
            assert!(source.get_player_mut(0).unwrap().queue_upgrade(
                UPGRADE,
                &cost,
                gamelogic::upgrade::UpgradeType::Player
            ));
            let authored_time = source.upgrade_research_time_secs(UPGRADE);
            assert!(
                source
                    .host_object_mut(producer)
                    .unwrap()
                    .building_data
                    .as_mut()
                    .unwrap()
                    .add_upgrade_to_queue(UPGRADE.into(), authored_time, cost)
            );
            for _ in 0..9 {
                source.update_production(1.0 / 30.0);
            }
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = GameLogic::new();
            restored.templates = source.templates.clone();
            register_test_upgrade(&restored, UPGRADE, "PLAYER", 40, 3);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            let queue = &restored
                .host_object(producer)
                .unwrap()
                .building_data
                .as_ref()
                .unwrap()
                .production_queue;
            assert!(queue[0].is_upgrade());
            assert_eq!(queue[0].construction_frames, 9);
            assert_eq!(queue[0].cost.supplies, 40);
            assert_eq!(queue[0].total_time, 3.0);
            assert_eq!(
                gamelogic::world::entities::production_total_logic_frames(
                    queue[0].total_time,
                    true,
                    0.5
                ),
                90
            );
            for world in [&mut source, &mut restored] {
                // Release UpgradeTemplate::calcTimeToBuild is independent of
                // energy and exact ThingTemplate General time entries.
                world.players.get_mut(&0).unwrap().power_produced = 0;
                for _ in 9..89 {
                    world.update_production(1.0 / 30.0);
                    assert!(world.players[&0].has_queued_upgrade(UPGRADE));
                    assert!(!world.players[&0].has_unlocked_upgrade(UPGRADE));
                }
                assert_eq!(
                    world
                        .host_object(producer)
                        .unwrap()
                        .building_data
                        .as_ref()
                        .unwrap()
                        .production_queue[0]
                        .construction_frames,
                    89
                );
                world.update_production(1.0 / 30.0);
                assert!(
                    world
                        .host_object(producer)
                        .unwrap()
                        .building_data
                        .as_ref()
                        .unwrap()
                        .production_queue
                        .is_empty()
                );
                assert!(!world.players[&0].has_queued_upgrade(UPGRADE));
                assert!(world.players[&0].has_unlocked_upgrade(UPGRADE));
            }
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_duration_restore_accepts_retained_inactive_controlling_player() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_duration_restore_accepts_retained_inactive_controlling_player",
        || {
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            worker_production_definition();
            let (mut source, producer) = production_world("Owner48WorkerGeneral");
            assert!(source.enqueue_production(producer, "TestInfantry".into()));
            for _ in 0..9 {
                source.update_production(1.0 / 30.0);
            }
            source.players.get_mut(&0).unwrap().is_alive = false;
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            assert!(
                !snapshot
                    .players
                    .iter()
                    .find(|player| player.id == 0)
                    .unwrap()
                    .is_active
            );
            for explicit_owner in [true, false] {
                let mut saved = builder.create_world_snapshot(&source).unwrap();
                if !explicit_owner {
                    saved
                        .objects
                        .get_mut(&producer)
                        .unwrap()
                        .collector_runtime
                        .as_mut()
                        .unwrap()
                        .owner_player_id = None;
                }
                let bytes = bincode_legacy::serialize(&saved).unwrap();
                let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
                let mut restored = GameLogic::new();
                restored.templates = source.templates.clone();
                builder
                    .restore_from_snapshot(&decoded, &mut restored)
                    .unwrap();
                assert!(!restored.players[&0].is_alive);
                assert_eq!(
                    restored.player_template_identity(0).unwrap().template_name,
                    "Owner48WorkerGeneral"
                );
                assert_eq!(queued_production_frames(&restored, producer), 77);
                let queue = &restored
                    .host_object(producer)
                    .unwrap()
                    .building_data
                    .as_ref()
                    .unwrap()
                    .production_queue;
                assert_eq!(queue[0].construction_frames, 9);
                assert_eq!(queue[0].cost.supplies, 80);
                assert_eq!(restored.players[&0].effective_supplies(), 920);
                // This verifies saved references and continuation state, not
                // whether an inactive player may resume production live.
            }
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_duration_restore_rejects_definition_synthesized_by_earlier_live_unit() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_duration_restore_rejects_definition_synthesized_by_earlier_live_unit",
        || {
            use crate::save_load::SnapshotBuilder;
            worker_production_definition();
            let (mut source, _) = production_world("Owner48WorkerGeneral");
            let existing_unit = source
                .create_object_for_player("TestInfantry", 0, glam::Vec3::new(10.0, 0.0, 0.0))
                .unwrap();
            let producer = source
                .create_object_for_player("TestBarracks", 0, glam::Vec3::new(20.0, 0.0, 0.0))
                .unwrap();
            assert!(
                existing_unit < producer,
                "the live unit restores before the queued producer"
            );
            assert!(source.enqueue_production(producer, "TestInfantry".into()));
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let mut restored = GameLogic::new();
            restored.templates = source.templates.clone();
            restored.templates.remove("TestInfantry");
            let error = builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("Queued production template 'TestInfantry'"),
                "{error}"
            );
            assert!(
                !restored.templates.contains_key("TestInfantry"),
                "missing queue content is detected before synthesizing object templates"
            );
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_duration_restore_rejects_missing_unit_definition_and_stale_or_ambiguous_owner() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_duration_restore_rejects_missing_unit_definition_and_stale_or_ambiguous_owner",
        || {
            use crate::save_load::SnapshotBuilder;
            worker_production_definition();
            let (mut source, producer) = production_world("Owner48WorkerGeneral");
            assert!(source.enqueue_production(producer, "TestInfantry".into()));
            let builder = SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let mut missing = GameLogic::new();
            missing.templates = source.templates.clone();
            missing.templates.remove("TestInfantry");
            let error = builder
                .restore_from_snapshot(&snapshot, &mut missing)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("Queued production template 'TestInfantry'"),
                "{error}"
            );
            let mut stale_snapshot = builder.create_world_snapshot(&source).unwrap();
            stale_snapshot
                .objects
                .get_mut(&producer)
                .unwrap()
                .collector_runtime
                .as_mut()
                .unwrap()
                .owner_player_id = Some(99);
            let mut stale = GameLogic::new();
            stale.templates = source.templates.clone();
            let error = builder
                .restore_from_snapshot(&stale_snapshot, &mut stale)
                .unwrap_err()
                .to_string();
            assert!(error.contains("no valid controlling player"), "{error}");
            source.add_player(Player::new(1, Team::USA, "same faction other owner", false));
            source.players.get_mut(&1).unwrap().is_alive = false;
            let mut ambiguous_snapshot = builder.create_world_snapshot(&source).unwrap();
            ambiguous_snapshot
                .objects
                .get_mut(&producer)
                .unwrap()
                .collector_runtime
                .as_mut()
                .unwrap()
                .owner_player_id = None;
            let mut ambiguous = GameLogic::new();
            ambiguous.templates = source.templates.clone();
            let error = builder
                .restore_from_snapshot(&ambiguous_snapshot, &mut ambiguous)
                .unwrap_err()
                .to_string();
            assert!(error.contains("no valid controlling player"), "{error}");
        },
    );
}

// hq-p6s42 / hq-7chz9: saved names resolve in the receiving world's catalog.
// The real owned INI parser supplies authored seconds; these bounded restore
// witnesses do not claim that the command/automatic-AI admission fallbacks
// already preserve every zero/fractional rule (that remains hq-ax77l).
#[cfg(not(target_arch = "wasm32"))]
fn register_upgrade_restore_rule(
    world: &GameLogic,
    name: &str,
    kind: &str,
    cost: u32,
    seconds: f32,
) {
    let source =
        format!("Upgrade {name}\nType = {kind}\nBuildCost = {cost}\nBuildTime = {seconds}\nEnd\n");
    let mut center = world.world_services.upgrade_center().write().unwrap();
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&source, |ini| {
        ini.read_line()?;
        center.parse_upgrade_definition(ini)
    })
    .expect("actual candidate-owned Upgrade INI parser");
}

#[cfg(not(target_arch = "wasm32"))]
fn queue_upgrade_restore_rule(world: &mut GameLogic, producer: ObjectId, name: &str) -> u32 {
    let definition = world.upgrade_template(name).unwrap();
    let before = world.players[&0].effective_supplies();
    let cost = Resources {
        supplies: definition.get_cost().try_into().unwrap(),
        power: 0,
    };
    assert!(world.get_player_mut(0).unwrap().queue_upgrade(
        name,
        &cost,
        definition.get_upgrade_type()
    ));
    assert!(
        world
            .host_object_mut(producer)
            .unwrap()
            .building_data
            .as_mut()
            .unwrap()
            .add_upgrade_to_queue(name.into(), definition.get_build_time(), cost)
    );
    assert_eq!(
        world.players[&0].effective_supplies(),
        before - cost.supplies
    );
    before
}

#[cfg(not(target_arch = "wasm32"))]
fn upgrade_restore_head(world: &GameLogic, producer: ObjectId) -> &ProductionItem {
    &world
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue[0]
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_upgrade_restore_rejects_missing_definition_from_foreign_catalog() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_upgrade_restore_rejects_missing_definition_from_foreign_catalog",
        || {
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            const NAME: &str = "Upgrade_Owner50MissingDefinition";
            worker_production_definition();
            let (mut live, producer) = production_world("Owner48WorkerGeneral");
            register_upgrade_restore_rule(&live, NAME, "PLAYER", 37, 1.25);
            queue_upgrade_restore_rule(&mut live, producer, NAME);
            for _ in 0..7 {
                live.update_production(1.0 / 30.0);
            }
            let wallet = live.players[&0].effective_supplies();
            let builder = SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&live).unwrap();
            let bytes = bincode_legacy::serialize(&saved).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let (mut foreign, foreign_producer) = production_world("Owner48ForeignGeneral");
            assert_eq!(foreign_producer, producer);
            register_upgrade_restore_rule(&foreign, NAME, "OBJECT", 911, 9.75);
            for case_only in [false, true] {
                let mut candidate = GameLogic::new();
                candidate.templates = live.templates.clone();
                if case_only {
                    register_upgrade_restore_rule(
                        &candidate,
                        &NAME.to_ascii_lowercase(),
                        "PLAYER",
                        37,
                        1.25,
                    );
                }
                assert!(candidate.upgrade_template(NAME).is_none());
                // Existing bounded compatibility publication deliberately
                // points elsewhere; a loader may never use it for this queue.
                let error = gamelogic::system::engine_stores::with_world_services(
                    &foreign.world_services,
                    || builder.restore_from_snapshot(&decoded, &mut candidate),
                )
                .unwrap_err()
                .to_string();
                assert!(
                    error.contains("Queued upgrade 'Upgrade_Owner50MissingDefinition'"),
                    "{error}"
                );
                assert!(
                    candidate.host_objects().is_empty(),
                    "reject before admitting the producer queue"
                );
                drop(candidate);
            }
            foreign.reset();
            drop(foreign);
            assert_eq!(live.players[&0].effective_supplies(), wallet);
            assert!(live.players[&0].has_queued_upgrade(NAME));
            let head = upgrade_restore_head(&live, producer);
            assert_eq!(head.template_name, NAME);
            assert_eq!(head.construction_frames, 7);
            assert_eq!(head.cost.supplies, 37);
            assert_eq!(head.cost.power, 0);
            assert_eq!(head.total_time, 1.25);
            live.update_production(1.0 / 30.0);
            assert_eq!(upgrade_restore_head(&live, producer).construction_frames, 8);
            // This exercises a detached SnapshotBuilder candidate. The full
            // map/host transaction and its rollback are separate containing gates.
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_upgrade_restore_preserves_authored_zero_and_fractional_continuation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_upgrade_restore_preserves_authored_zero_and_fractional_continuation",
        || {
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            worker_production_definition();
            // C++ Upgrade.cpp:143 truncates Real seconds*30 into Int, and
            // ProductionUpdate.cpp:687–707 observes completion after one update
            // for an authored zero/sub-frame duration. General/power do not
            // alter the release upgrade calculation.
            let cases: [(&str, f32, u32, u32, u32); 3] = [
                ("Upgrade_Owner50ZeroTime", 0.0, 0, 0, 1),
                ("Upgrade_Owner50FractionalTime", 1.25, 37, 7, 37),
                ("Upgrade_Owner50SubFrameTime", 0.005, 17, 0, 1),
            ];
            for (name, seconds, paid_cost, elapsed, terminal) in cases {
                let (mut source, producer) = production_world("Owner48WorkerGeneral");
                register_upgrade_restore_rule(&source, name, "PLAYER", paid_cost, seconds);
                let starting_wallet = queue_upgrade_restore_rule(&mut source, producer, name);
                for _ in 0..elapsed {
                    source.update_production(1.0 / 30.0);
                }
                let builder = SnapshotBuilder::new();
                let snapshot = builder.create_world_snapshot(&source).unwrap();
                let bytes = bincode_legacy::serialize(&snapshot).unwrap();
                let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
                let mut restored = GameLogic::new();
                restored.templates = source.templates.clone();
                register_upgrade_restore_rule(&restored, name, "PLAYER", paid_cost, seconds);
                let (mut foreign, foreign_producer) = production_world("Owner48ForeignGeneral");
                assert_eq!(foreign_producer, producer);
                register_upgrade_restore_rule(&foreign, name, "OBJECT", 901, 9.75);
                gamelogic::system::engine_stores::with_world_services(
                    &foreign.world_services,
                    || builder.restore_from_snapshot(&decoded, &mut restored),
                )
                .unwrap();
                let head = upgrade_restore_head(&restored, producer);
                assert!(head.is_upgrade());
                assert_eq!(head.template_name, name);
                assert_eq!(head.total_time.to_bits(), seconds.to_bits());
                assert_eq!(head.construction_frames, elapsed);
                assert_eq!(
                    head.cost.supplies, paid_cost,
                    "retain already charged saved cost"
                );
                assert_eq!(head.cost.power, 0);
                assert_eq!(head.quantity_total, 1);
                assert_eq!(head.quantity_produced, 0);
                assert_eq!(
                    restored.players[&0].effective_supplies(),
                    starting_wallet - paid_cost
                );
                assert_eq!(
                    restored.upgrade_type(name),
                    gamelogic::upgrade::UpgradeType::Player
                );
                assert!(!std::sync::Arc::ptr_eq(
                    &source.upgrade_template(name).unwrap(),
                    &restored.upgrade_template(name).unwrap()
                ));
                foreign.reset();
                drop(foreign);
                for world in [&mut source, &mut restored] {
                    // Keep the producer operational while demonstrating that
                    // the research threshold ignores its player's power rate.
                    world.players.get_mut(&0).unwrap().power_produced = 0;
                    for frame in elapsed..terminal - 1 {
                        world.update_production(1.0 / 30.0);
                        assert_eq!(
                            upgrade_restore_head(world, producer).construction_frames,
                            frame + 1
                        );
                        assert!(world.players[&0].has_queued_upgrade(name));
                        assert!(!world.players[&0].has_unlocked_upgrade(name));
                    }
                    world.update_production(1.0 / 30.0);
                    assert!(
                        world
                            .host_object(producer)
                            .unwrap()
                            .building_data
                            .as_ref()
                            .unwrap()
                            .production_queue
                            .is_empty()
                    );
                    assert!(!world.players[&0].has_queued_upgrade(name));
                    assert!(world.players[&0].has_unlocked_upgrade(name));
                    assert_eq!(
                        world.players[&0].effective_supplies(),
                        starting_wallet - paid_cost
                    );
                }
            }
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn queued_upgrade_restore_cancellation_ignores_same_name_object_power() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "queued_upgrade_restore_cancellation_ignores_same_name_object_power",
        || {
            use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
            const NAME: &str = "Upgrade_Owner50PowerCollision";
            worker_production_definition();
            let (mut source, producer) = production_world("Owner48WorkerGeneral");
            let mut collision = ThingTemplate::new(NAME);
            collision.build_cost = Resources {
                supplies: 5_000,
                power: -201,
            };
            collision.build_time = 99.0;
            source.templates.insert(NAME.into(), collision);
            register_upgrade_restore_rule(&source, NAME, "PLAYER", 37, 1.25);
            source.players.get_mut(&0).unwrap().power_available = 41;
            let initial_wallet = queue_upgrade_restore_rule(&mut source, producer, NAME);
            for _ in 0..7 {
                source.update_production(1.0 / 30.0);
            }
            let builder = SnapshotBuilder::new();
            let saved = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&saved).unwrap();
            let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
            let mut restored = GameLogic::new();
            restored.templates = source.templates.clone();
            register_upgrade_restore_rule(&restored, NAME, "PLAYER", 37, 1.25);
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            assert_eq!(upgrade_restore_head(&restored, producer).cost.supplies, 37);
            assert_eq!(upgrade_restore_head(&restored, producer).cost.power, 0);
            let before_power = restored.players[&0].power_available;
            let (mut foreign, foreign_producer) = production_world("Owner48ForeignGeneral");
            assert_eq!(foreign_producer, producer);
            foreign.players.get_mut(&0).unwrap().resources.supplies = 901;
            foreign.players.get_mut(&0).unwrap().power_available = 82;
            register_upgrade_restore_rule(&foreign, NAME, "OBJECT", 911, 9.75);
            assert!(gamelogic::system::engine_stores::with_world_services(
                &foreign.world_services,
                || restored.cancel_production_at_index(producer, 0),
            ));
            assert_eq!(restored.players[&0].effective_supplies(), initial_wallet);
            assert_eq!(
                restored.players[&0].power_available, before_power,
                "C++ upgrade refund changes money only"
            );
            assert!(!restored.players[&0].has_queued_upgrade(NAME));
            assert!(
                !restored.cancel_production_at_index(producer, 0),
                "paid cost refunded once"
            );
            assert_eq!(restored.players[&0].effective_supplies(), initial_wallet);
            assert_eq!(foreign.players[&0].effective_supplies(), 901);
            assert_eq!(foreign.players[&0].power_available, 82);
            foreign.reset();
            drop(foreign);
            assert_eq!(
                upgrade_restore_head(&source, producer).construction_frames,
                7
            );
            assert!(source.players[&0].has_queued_upgrade(NAME));
            assert_eq!(source.players[&0].effective_supplies(), initial_wallet - 37);
            assert_eq!(source.players[&0].power_available, 41);
        },
    );
}

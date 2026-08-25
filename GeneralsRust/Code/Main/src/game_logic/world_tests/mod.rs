//! Host GameLogic unit tests (child of `game_logic.rs` via `#[path]`).
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

mod helpers;
use helpers::*;

mod base_defenses;
mod cave_bridge;
mod combat_particles_and_economy;
mod crates_and_salvage;
mod network_and_scripts;
mod ocl_and_bombs;
mod parachute_and_rebuild;
mod phase3_produce;
mod pilots_and_movement;
mod production_and_mobs;
mod projectiles_air;
mod scatter_and_chain;
mod science_and_upgrades;
mod shells_and_missiles;
mod strategy_and_stealth;
mod superweapon_initiate_at_location;
mod superweapons_and_plans;
mod unit_residuals;
mod vehicles_and_lasers;

#[test]
fn fast_chunky_sync_fail_opens_when_legacy_globals_are_busy() {
    // Abandoned startup workers can still hold THE_TERRAIN_LOGIC / SidesList /
    // PlayerList / TeamFactory after generation bump. start_game_from_ui is
    // synchronous, so blocking writes hang Loading. Fail-open must return.
    let mut logic = GameLogic::new();

    let mut toc = std::collections::HashMap::new();
    toc.insert(1, "HeightMapData".to_string());
    let mut bytes = b"CkMp".to_vec();
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.push(13);
    bytes.extend_from_slice(b"HeightMapData");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&13i32.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.push(0);
    let chunky = crate::game_logic::script_loader::ChunkyMap {
        source: std::path::PathBuf::from("synthetic-busy-locks.map"),
        toc,
        body_offset: 4 + 4 + 1 + 13 + 4,
        bytes,
    };
    let terrain_logic = gamelogic::terrain::get_terrain_logic();
    let _terrain = terrain_logic.write().expect("hold THE_TERRAIN_LOGIC");
    let sides_list = gamelogic::sides_list::get_sides_list();
    let _sides = sides_list.write().expect("hold THE_SIDES_LIST");
    let player_list = gamelogic::player::ThePlayerList();
    let _players = player_list.write().expect("hold ThePlayerList");
    let team_factory = gamelogic::team::get_team_factory();
    let _teams = team_factory.lock().expect("hold THE_TEAM_FACTORY");
    let file_system = game_engine::common::system::file_system::get_file_system();
    let _fs = file_system.lock().expect("hold FileSystem");

    let started = std::time::Instant::now();
    logic.sync_legacy_runtime_from_fast_chunky(chunky.source.as_path(), &chunky);
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "sync blocked on contended locks for {:?}",
        elapsed
    );
}

#[test]
fn global_terrain_load_map_data_does_not_deadlock_on_bridge() {
    // C++ TerrainLogic::addBridgeToLogic (TerrainLogic.cpp:1514) registers
    // bridges while loadMap already owns TheTerrainLogic. Rust
    // classify_bridge_cells used to RwLock::read the same singleton and hang
    // Lone Eagle load_map inside Fast legacy terrain write.
    let mut map_data = gamelogic::system::map_loader::MapData::new();
    map_data.width = 4;
    map_data.height = 4;
    map_data.heightmap = vec![10; 16];
    map_data
        .bridges
        .push(gamelogic::system::map_loader::BridgeData::new(
            gamelogic::system::map_loader::Coord3D::new(0.0, 0.0, 20.0),
            gamelogic::system::map_loader::Coord3D::new(40.0, 0.0, 20.0),
            10.0,
            "TestBridge".to_string(),
        ));
    let started = std::time::Instant::now();
    {
        let mut terrain = gamelogic::terrain::get_terrain_logic()
            .write()
            .expect("THE_TERRAIN_LOGIC");
        terrain.reset();
        terrain.load_map_data(map_data);
        assert!(
            terrain.get_first_bridge().is_some(),
            "map bridge must still register"
        );
    }
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "load_map_data deadlocked on THE_TERRAIN_LOGIC for {:?}",
        started.elapsed()
    );
}
#[test]
fn process_destroy_list_runs_inside_each_logic_frame() {
    // C++ GameLogic.cpp:3762 processDestroyList is inside every update(),
    // not once after the fixed-step catch-up batch (hq-x4im).
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let id = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::ZERO)
        .expect("unit");
    logic.mark_object_for_destruction(id, None);
    assert!(logic.get_object(id).is_some(), "queued, not yet removed");
    logic.update_simulation(1.0 / 30.0);
    assert!(
        logic.get_object(id).is_none(),
        "processDestroyList must run inside update_simulation / each fixed step"
    );
}

#[test]
fn victory_conditions_update_inside_each_logic_frame() {
    // C++ GameLogic.cpp:3769 TheVictoryConditions->UPDATE() is inside
    // GameLogic::update, not only PresentationFrame::build (hq-en3j).
    use crate::game_logic::VictoryCondition;
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    ensure_test_infantry_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let _usa = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("usa");
    let gla = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("gla");
    logic.mark_object_for_destruction(gla, None);
    logic.update_simulation(1.0 / 30.0);
    assert!(
        logic.get_object(gla).is_none(),
        "loser unit must be gone before victory UPDATE"
    );
    let outcome = logic.evaluate_victory_condition();
    assert!(
        matches!(outcome, Some(VictoryCondition::Winner(_))),
        "victory must be decided on the logic frame that processed destroy, got {outcome:?}"
    );
}

#[test]
fn skirmish_razing_last_enemy_building_defeats_player_with_units_left() {
    // C++ GameLogic.cpp:1606 startNewGame sets VICTORY_NOBUILDINGS.
    // VictoryConditions.cpp:272-277 hasSinglePlayerBeenDefeated: with only
    // that flag, razing every enemy building defeats that player even if
    // units remain (C++ then killPlayer leftover infantry).
    use crate::game_logic::{VictoryCondition, VictoryType};
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    logic.clear_all_players();
    ensure_test_infantry_template(&mut logic);
    ensure_test_structure_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);

    assert_eq!(
        logic.victory_type(),
        VictoryType::NO_BUILDINGS,
        "skirmish must use C++ VICTORY_NOBUILDINGS, not annihilation"
    );

    let _usa_cc = logic
        .create_object("TestBuilding", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("usa building");
    let gla_cc = logic
        .create_object("TestBuilding", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("gla building");
    let gla_unit = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(90.0, 0.0, 0.0))
        .expect("gla infantry leftover");

    assert!(
        logic.evaluate_victory_condition().is_none(),
        "match must continue while the enemy still has a building"
    );
    // C++ Team::hasAnyBuildings skips isEffectivelyDead / isDestroyed. Host
    // structure topple keeps a sliver of HP so the object stays in the map;
    // victory must still treat that razed building as gone.
    if let Some(obj) = logic.host_object_mut(gla_cc) {
        obj.health.current = 0.0;
        obj.status.effectively_dead = true;
    }
    assert!(
        logic.get_object(gla_cc).is_some_and(|obj| !obj.is_alive()),
        "razed building must be effectively dead for hasAnyBuildings"
    );
    assert!(
        logic.get_object(gla_unit).is_some_and(|obj| obj.is_alive()),
        "leftover enemy units must not block NOBUILDINGS defeat"
    );

    let outcome = logic.evaluate_victory_condition();
    assert!(
        matches!(outcome, Some(VictoryCondition::Winner(_))),
        "razing the last enemy building must end the skirmish, got {outcome:?}"
    );
    // C++ VictoryConditions.cpp:196 + Player::killPlayer — leftover army
    // is destroyed and money is withdrawn to 0.
    assert!(
        logic.get_object(gla_unit).is_none()
            || logic
                .get_object(gla_unit)
                .is_some_and(|obj| !obj.is_alive()),
        "killPlayer must destroy leftover army"
    );
    let gla_player = logic.get_player(2).expect("gla player");
    assert_eq!(gla_player.resources.supplies, 0, "killPlayer zeros money");
    assert!(!gla_player.is_alive);
}

#[test]
fn campaign_does_not_run_multiplayer_annihilation() {
    // C++ VictoryConditions.cpp:125 early-return unless isMultiplayer.
    use crate::game_logic::VictoryCondition;
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::SinglePlayer);
    logic.clear_all_players();
    ensure_test_infantry_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::GLA);
    let _usa = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("usa");
    assert!(
        logic.evaluate_victory_condition().is_none(),
        "campaign must not hard-end via MP annihilation, got {:?}",
        logic.evaluate_victory_condition()
    );
    let _ = VictoryCondition::Draw;
}

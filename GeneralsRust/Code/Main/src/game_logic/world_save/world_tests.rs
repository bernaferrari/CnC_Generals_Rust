//! Host-world validation and parity tests.
#![allow(unused_imports, non_snake_case)]
use super::*;

#[cfg(test)]
mod sides_host_apply_tests {
    use super::*;

    #[test]
    fn map_side_dict_sets_host_money_color_and_enemies() {
        let mut logic = GameLogic::new();
        logic.add_player(
            Player::new(0, Team::USA, "PlyrAmerica", true).with_script_identity("PlyrAmerica"),
        );
        logic.add_player(
            Player::new(1, Team::GLA, "PlyrGLA", false).with_script_identity("PlyrGLA"),
        );

        let mut america = Dict::new();
        america.set_ascii_string(key_player_name(), "PlyrAmerica");
        america.set_int(key_player_start_money(), 3_000);
        america.set_int(key_player_color(), 0x0000_00ff);
        america.set_ascii_string(key_player_enemies(), "PlyrGLA");
        america.set_ascii_string(key_player_allies(), "");

        let mut gla = Dict::new();
        gla.set_ascii_string(key_player_name(), "PlyrGLA");
        gla.set_int(key_player_start_money(), 1_500);
        gla.set_ascii_string(key_player_enemies(), "PlyrAmerica");

        logic.apply_host_players_from_side_dicts(&[america, gla], true, None);

        let usa = logic.get_player(0).expect("usa");
        // Player.cpp1007 deposits onto the initialized fallback, not replaces it.
        assert_eq!(usa.resources.supplies, 13_000);
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 11_500);
        assert_eq!(usa.color_rgb, (0, 0, 0xff));
        assert_eq!(
            logic.player_relationship(0, 1),
            game_engine::common::game_common::Relationship::Enemies
        );
    }

    #[test]
    fn skirmish_sides_make_no_team_slots_enemies_and_teammates_allies() {
        // Native content/key bootstrap is still process-owned. Keep this
        // complete relationship witness independent of other fixture resets.
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "skirmish_sides_make_no_team_slots_enemies_and_teammates_allies",
            || {
                // C++ GameLogic.cpp:1332-1368: team -1 or a different team => enemy.
                use game_engine::common::game_common::Relationship;
                let mut logic = GameLogic::new();
                logic.game_mode = GameMode::Skirmish;
                for (id, team, alliance) in [
                    (0, Team::USA, -1),
                    (1, Team::China, -1),
                    (2, Team::GLA, 3),
                    (3, Team::USA, 3),
                ] {
                    let mut player = Player::new(id, team, &format!("Slot{id}"), id == 0);
                    player.alliance_team = alliance;
                    player.is_alive = true;
                    logic.add_player(player);
                }
                let mut sides = gamelogic::sides_list::SidesList::new();
                logic.add_host_players_as_sides(&mut sides);
                let dicts: Vec<Dict> = (0..sides.get_num_sides())
                    .filter_map(|i| sides.get_side_info(i).map(|side| side.get_dict().clone()))
                    .collect();
                logic.apply_host_players_from_side_dicts(&dicts, false, None);

                assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
                assert_eq!(logic.player_relationship(1, 0), Relationship::Enemies);
                assert_eq!(logic.player_relationship(0, 2), Relationship::Enemies);
                assert_eq!(logic.player_relationship(2, 0), Relationship::Enemies);
                assert_eq!(logic.player_relationship(2, 3), Relationship::Allies);
                assert_eq!(logic.player_relationship(3, 2), Relationship::Allies);
            },
        );
    }

    #[test]
    fn authored_map_sides_cannot_replace_occupied_skirmish_slots() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "authored_map_sides_cannot_replace_occupied_skirmish_slots",
            || {
                let dir = tempfile::tempdir().unwrap();
                let path = dir.path().join("SlotAdmission.map");
                let mut out = game_engine::common::system::DataChunkOutput::new();
                out.open_data_chunk("HeightMapData", 3);
                for value in [21, 21, 1, 441] {
                    out.write_int(value);
                }
                for _ in 0..441 {
                    out.write_byte(0);
                }
                out.close_data_chunk();
                out.open_data_chunk("ObjectsList", 3);
                out.open_data_chunk("Object", 3);
                for value in [10.0, 10.0, 0.0, 0.0] {
                    out.write_real(value);
                }
                out.write_int(0);
                out.write_ascii_string("SlotAdmissionMarker");
                out.close_data_chunk();
                out.close_data_chunk();
                out.open_data_chunk("SidesList", 2);
                out.write_int(3);
                // CPP Player792-819 forces an AI Human when its faction has no
                // matching source side. Supply that definition so this witness
                // exercises occupied-slot preservation on a valid skirmish map.
                for (name, faction) in [
                    ("UnrelatedMapSide", "FactionCivilian"),
                    ("PlyrCivilian", "FactionCivilian"),
                    ("PlyrGLA", "FactionGLA"),
                ] {
                    let mut dict = Dict::new();
                    dict.set_ascii_string(key_player_name(), name);
                    dict.set_ascii_string(key_player_faction(), faction);
                    dict.set_bool(key_player_is_human(), false);
                    dict.set_int(key_player_color(), 0);
                    dict.set_int(key_player_night_color(), 0);
                    dict.set_int(key_player_start_money(), 17);
                    out.write_dict(&dict);
                    out.write_int(0); // authored build entries
                }
                out.write_int(0); // authored teams
                out.close_data_chunk();
                std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
                let map = path.to_str().unwrap();
                // This admission witness supplies its immutable template inputs.
                // It does not rely on another test having populated retail globals.
                // The map's placed Object is authored content like its StartingBuildings.
                let mut world = GameLogic::new();
                crate::skirmish_config::lobby_definition_tests::admit_slot_world_with_objects(
                    &mut world,
                    &crate::skirmish_config::lobby_definition_tests::slot_definitions(),
                    &[crate::fixture_content::FixtureObject::new("SlotAdmissionMarker").health(10.0)],
                );
                let config = crate::skirmish_config::lobby_definition_tests::slot_config(
                    &world,
                    &crate::skirmish_config::golden_skirmish_config(map),
                );
                crate::skirmish_config::apply_skirmish_config(&mut world, &config).unwrap();
                let slots = |world: &GameLogic| {
                    [0, 1].map(|id| {
                        let player = world.get_player(id).unwrap();
                        (
                            player.is_human,
                            player.color_rgb,
                            player.color_night_rgb,
                            player.resources.supplies,
                            player.team,
                            player.name.clone(),
                        )
                    })
                };
                let before = slots(&world);
                assert!(world.load_map(map), "actual CKMP admission must execute");
                assert!(world.terrain.is_some(), "raw terrain must be admitted");
                assert!(
                    world
                        .objects
                        .registered_values()
                        .any(|object| object.thing().template.name == "SlotAdmissionMarker"),
                    "object-placement admission must also execute"
                );
                assert_eq!(
                    slots(&world),
                    before,
                    "authored Civilian side indices are not occupied GameInfo slots"
                );
                for name in ["UnrelatedMapSide", "PlyrCivilian"] {
                    assert!(
                        world
                            .players
                            .values()
                            .any(|p| p.map_side.map_player_name == name),
                        "actual map admits retained Civilian {name}"
                    );
                }
                assert!(
                    world.players.values().any(Player::is_reserved_neutral),
                    "actual map admits canonical neutral independently of slots"
                );
            },
        );
    }

    #[test]
    fn authored_build_list_replaces_hardcoded_ai_layout() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", false));
        logic.add_ai_opponent(1, Team::USA, crate::ai::AIDifficulty::Medium);
        let before = logic
            .ai_manager
            .ai_players
            .get(&1)
            .map(|ai| ai.building_queue.len())
            .unwrap_or(0);
        assert!(before > 0, "hardcoded layout seeds a queue");

        let builds = [super::super::script_loader::SideBuildEntry {
            building_name: "cc".into(),
            template: "AmericaCommandCenter".into(),
            position: gamelogic::scripting::core::Coord3D {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            },
            angle: 0.0,
            initially_built: true,
            num_rebuilds: 3,
            side_index: 1,
            script_name: None,
            health: None,
            whiner: None,
            unsellable: None,
            repairable: None,
        }];
        logic.stash_side_builds_on_host(&builds);
        let ai = logic.ai_manager.ai_players.get(&1).expect("ai");
        assert_eq!(ai.building_queue.len(), 1);
        assert_eq!(ai.building_queue[0].template_name, "AmericaCommandCenter");
        assert_eq!(ai.building_queue[0].max_rebuilds, 3);
        assert!(ai.building_queue[0].is_built);
    }

    #[test]
    fn world_info_weather_is_owned_by_each_runtime_session() {
        let mut logic = GameLogic::new();
        let mut peer = GameLogic::new();
        assert!(!logic.weather_state().is_snowy());
        logic.set_weather_state("Light SNoW flurries", 0.5, 30.0, 60.0);
        assert!(logic.weather_state().is_snowy());
        assert!(!peer.weather_state().is_snowy());
        logic.apply_world_info_weather(Some(1));
        assert_eq!(logic.weather_state().current_weather, "snowy");
        assert_eq!(peer.weather_state().current_weather, "normal");
        peer.apply_world_info_weather(Some(0));
        assert_eq!(peer.weather_state().current_weather, "normal");
        assert_eq!(logic.weather_state().current_weather, "snowy");
        logic.apply_world_info_weather(Some(0));
        assert_eq!(logic.weather_state().current_weather, "normal");
        logic.apply_world_info_weather(None);
        assert_eq!(logic.weather_state().current_weather, "normal");
    }

    #[test]
    fn replay_start_new_game_sets_local_player_to_replay_observer() {
        // C++ GameLogic.cpp:2222-2230 GAME_REPLAY switches local identity.
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA Commander", true));
        logic.game_mode = GameMode::Replay;
        let observer_id = logic.ensure_replay_observer_player();
        logic.install_replay_observer_side();
        logic.apply_replay_observer_as_local_player();

        let observer = logic
            .players
            .get(&observer_id)
            .expect("ReplayObserver host player");
        assert!(
            logic.is_local_player(observer.id),
            "ReplayObserver must be local in GAME_REPLAY"
        );
        assert_eq!(observer.name, "ReplayObserver");
        assert_eq!(
            usize::from(logic.local_player_id().is_some()),
            1,
            "only ReplayObserver is local"
        );
        assert!(logic.radar_forced);
        assert_eq!(
            logic
                .get_player(logic.local_player_id().unwrap())
                .unwrap()
                .script_identity(),
            Some("ReplayObserver")
        );
    }
}

#[cfg(test)]
mod authored_bridge_snapshot_tests {
    use super::encode_authored_bridge_visual;

    #[test]
    fn encode_authored_bridge_visual_carries_model_scale_and_towers() {
        // C++ W3DBridgeBuffer.cpp:182-191 findBridge + BridgeModelName/Scale/towers.
        let encoded = encode_authored_bridge_visual(
            "Concrete",
            "CBBridgeSt",
            0.7,
            [
                "BridgeTowerFromLeft",
                "BridgeTowerFromRight",
                "BridgeTowerToLeft",
                "BridgeTowerToRight",
            ],
        );
        assert!(encoded.starts_with("AUTHBR"));
        assert!(encoded.contains("Concrete"));
        assert!(encoded.contains("CBBridgeSt"));
        assert!(encoded.contains("0.7"));
        assert!(encoded.contains("BridgeTowerFromLeft"));
        assert!(encoded.contains("BridgeTowerToRight"));
        assert!(!encoded.contains("StoneBridge"));
        assert!(!encoded.contains("Granite"));
    }
}

#[cfg(test)]
mod landmark_bridge_and_new_map_tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn leftover_new_map_enables_water_grid_for_waveguide1() {
        let mut map_data = gamelogic::system::map_loader::MapData::new();
        map_data
            .waypoints
            .push(gamelogic::system::map_loader::MapWaypoint {
                id: 1,
                name: "WaveGuide1".to_string(),
                location: gamelogic::system::map_loader::Coord3D::new(20.0, 20.0, 5.0),
                path_label1: String::new(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
        let mut terrain = gamelogic::terrain::TerrainLogic::new();
        terrain.load_map_data(map_data);
        assert!(!terrain.is_water_grid_enabled());
        terrain.new_map(false);
        assert!(terrain.is_water_grid_enabled());
    }

    fn lifecycle_map_bytes(height: u8, waypoint_name: &str) -> Vec<u8> {
        let mut out = game_engine::common::system::DataChunkOutput::new();
        out.open_data_chunk("HeightMapData", 3);
        for value in [21, 21, 1, 441] {
            out.write_int(value);
        }
        for _ in 0..441 {
            out.write_byte(height);
        }
        out.close_data_chunk();
        out.open_data_chunk("ObjectsList", 3);
        out.open_data_chunk("Object", 3);
        for value in [40.0, 40.0, 999.0, 0.0] {
            out.write_real(value);
        }
        out.write_int(0);
        out.write_ascii_string("Waypoint");
        let mut dict = Dict::new();
        dict.set_int(NameKeyGenerator::name_to_key("waypointID"), 7);
        dict.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), waypoint_name);
        out.write_dict(&dict);
        out.close_data_chunk();
        out.close_data_chunk();
        out.into_ckmp_bytes()
    }

    fn terrain_lifecycle_state(world: &GameLogic) -> (String, f32, bool) {
        let terrain = world.world_services.terrain().read().unwrap();
        let waypoint = terrain
            .get_waypoint_by_id(7)
            .expect("admitted map waypoint");
        let location = waypoint.get_location();
        assert_eq!(
            location.z,
            terrain.get_ground_height(location.x, location.y, None),
            "C++ TerrainLogic::newMap grounds authored waypoint Z after height admission"
        );
        assert_ne!(location.z, 999.0, "authored height must be finalized");
        (
            waypoint.get_name().as_str().to_owned(),
            location.z,
            terrain.is_water_grid_enabled(),
        )
    }

    #[test]
    fn actual_map_reload_finalizes_receiving_waypoints_and_logical_water_grid() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "actual_map_reload_finalizes_receiving_waypoints_and_logical_water_grid",
            || {
                let temp = tempfile::tempdir().unwrap();
                let marker = temp.path().join("Marker.map");
                let plain = temp.path().join("Plain.map");
                let peer_map = temp.path().join("Peer.map");
                std::fs::write(&marker, lifecycle_map_bytes(8, "WaveGuide1")).unwrap();
                std::fs::write(&plain, lifecycle_map_bytes(24, "Replacement")).unwrap();
                std::fs::write(&peer_map, lifecycle_map_bytes(40, "WaveGuide1")).unwrap();
                let mut receiving = GameLogic::new();
                assert!(receiving.load_map(marker.to_str().unwrap()));
                let first = terrain_lifecycle_state(&receiving);
                assert_eq!((&*first.0, first.2), ("WaveGuide1", true));
                let mut peer = GameLogic::new();
                assert!(peer.load_map(peer_map.to_str().unwrap()));
                let peer_before = terrain_lifecycle_state(&peer);
                assert_ne!(
                    first.1, peer_before.1,
                    "same waypoint ID has independent terrain"
                );
                assert_eq!(terrain_lifecycle_state(&receiving), first);
                assert!(receiving.load_map(plain.to_str().unwrap()));
                let second = terrain_lifecycle_state(&receiving);
                assert_eq!((&*second.0, second.2), ("Replacement", false));
                assert_ne!(first.1, second.1);
                assert_eq!(terrain_lifecycle_state(&peer), peer_before);
                receiving.reset();
                assert_eq!(terrain_lifecycle_state(&peer), peer_before);
            },
        );
    }

    #[test]
    fn authored_map_placement_claims_its_position_before_the_first_tick() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "authored_map_placement_claims_its_position_before_the_first_tick",
            || {
                use crate::fixture_content::{FixtureObject, admit_fixture_objects};
                use crate::game_logic::{PathfindingGrid, pathfinding::GridPos};
                use game_engine::common::system::DataChunkOutput;

                const WALKER: &str = "MapPlacedPositionWalker";
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("PositionPlacement.map");
                let mut map = DataChunkOutput::new();
                map.open_data_chunk("HeightMapData", 3);
                for value in [41, 41, 0, 1681] {
                    map.write_int(value);
                }
                for _ in 0..1681 {
                    map.write_byte(0);
                }
                map.close_data_chunk();
                map.open_data_chunk("ObjectsList", 3);
                map.open_data_chunk("Object", 3);
                for value in [110.0, 110.0, 0.0, 0.0] {
                    map.write_real(value);
                }
                map.write_int(0);
                map.write_ascii_string(WALKER);
                let mut properties = Dict::new();
                properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "team");
                map.write_dict(&properties);
                map.close_data_chunk();
                map.close_data_chunk();
                std::fs::write(&path, map.into_ckmp_bytes()).unwrap();

                let mut world = GameLogic::new();
                admit_fixture_objects(
                    &mut world,
                    &[FixtureObject::new(WALKER)
                        .kind_of("INFANTRY SELECTABLE")
                        .health(100.0)
                        .walker()],
                );
                world.start_new_game(GameMode::SinglePlayer).unwrap();
                assert!(world.load_map(path.to_str().unwrap()));
                let actor = world
                    .host_objects()
                    .registered_values()
                    .find(|object| object.template_name == WALKER)
                    .expect("the selected CkMp map admitted the authored walker");
                assert_eq!(world.frame, 0);
                assert_eq!(actor.get_position(), Vec3::new(110.0, 0.0, 110.0));
                assert!(actor.has_ai_update_interface());
                assert!(PathfindingGrid::is_doing_ground_movement(actor));
                assert!(world.pathfinding_system.map_ready);
                assert_eq!(
                    world.pathfinding_ground_position(GridPos::new(11, 11)),
                    Some(actor.id),
                    "GameLogic.cpp:1698 initializes the pathfinder before the map's Object::setPosition"
                );
                assert_eq!(actor.pathfind_cur_cell, (11, 11));
            },
        );
    }

    #[test]
    fn authored_map_walker_on_bridge_starts_with_only_the_deck_position_claim() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "authored_map_walker_on_bridge_starts_with_only_the_deck_position_claim",
            || {
                use crate::fixture_content::{FixtureObject, admit_fixture_objects};
                use game_engine::common::system::DataChunkOutput;
                use generals_sim::pathfinding::PathfindLayerEnum;

                const WALKER: &str = "MapPlacedBridgeWalker";
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("BridgeWalker.map");
                let mut map = DataChunkOutput::new();
                map.open_data_chunk("HeightMapData", 3);
                for value in [21, 21, 1, 441] {
                    map.write_int(value);
                }
                for _ in 0..441 {
                    map.write_byte(32);
                }
                map.close_data_chunk();
                map.open_data_chunk("ObjectsList", 3);
                for (x, flags) in [(20.0, 0x10), (80.0, 0x20)] {
                    map.open_data_chunk("Object", 3);
                    for value in [x, 50.0, 21.0, 0.0] {
                        map.write_real(value);
                    }
                    map.write_int(flags);
                    map.write_ascii_string("OwnedBridge0");
                    map.close_data_chunk();
                }
                map.open_data_chunk("Object", 3);
                // The unit's stored Z is its offset from the sampled ground.
                for value in [55.0, 50.0, 0.25, 0.0] {
                    map.write_real(value);
                }
                map.write_int(0);
                map.write_ascii_string(WALKER);
                let mut properties = Dict::new();
                properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "team");
                map.write_dict(&properties);
                map.close_data_chunk();
                map.close_data_chunk();
                std::fs::write(&path, map.into_ckmp_bytes()).unwrap();

                let mut world = GameLogic::new();
                crate::fixture_content::bridge_art::attach(
                    &mut world,
                    directory.path(),
                    ["OwnedBridge0".into()],
                    [-10.0, 10.0],
                    0.0,
                    1.0,
                );
                admit_fixture_objects(
                    &mut world,
                    &[
                        FixtureObject::new("GenericBridge")
                            .kind_of("STRUCTURE BRIDGE IMMOBILE")
                            .health(300.0),
                        FixtureObject::new(WALKER)
                            .kind_of("INFANTRY SELECTABLE")
                            .health(100.0)
                            .field("Geometry = CYLINDER")
                            .field("GeometryMajorRadius = 3")
                            .field("GeometryHeight = 10")
                            .walker(),
                    ],
                );
                world.start_new_game(GameMode::SinglePlayer).unwrap();
                assert!(world.load_map(path.to_str().unwrap()));
                let actor = world
                    .host_objects()
                    .registered_values()
                    .find(|unit| unit.template_name == WALKER)
                    .expect("the deck placement was admitted");
                let pose = Vec3::new(55.0, 20.25, 50.0);
                assert_eq!(world.frame, 0);
                assert_eq!(actor.get_position(), pose);
                assert!(actor.has_ai_update_interface());

                let layer = PathfindLayerEnum::from_u32(world.path_layer_for_point(pose) as u32);
                let grid = &world.pathfinding_system.grid;
                let cell = grid.world_to_grid(pose);
                assert_ne!(layer, PathfindLayerEnum::Ground);
                assert!(grid.layer_cell_type(layer as u8, cell).is_some());
                assert!(!grid.object_interacts_with_bridge_end(pose, 3.0, layer));

                assert_eq!(actor.pathfind_layer, layer as u8);
                assert_eq!(actor.pathfind_cur_cell, (cell.x, cell.y));
                assert_eq!(grid.position_unit_on_layer(cell, layer), actor.id.0);
                assert_eq!(world.pathfinding_ground_position(cell), None);
            },
        );
    }

    #[test]
    fn landmark_bridge_object_registers_leftover_deck() {
        let mut logic = GameLogic::new();
        logic.override_world_size(64.0, 64.0);
        let mut tmpl = ThingTemplate::new("TsingMaLandmarkBridge");
        tmpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::LandmarkBridge)
            .add_kind_of(KindOf::Bridge)
            .set_health(400.0);
        // Bridge::Bridge reads the live geometry regardless of how it was seeded.
        tmpl.geometry_info.authored = false;
        tmpl.geometry_info.geom_type = crate::game_logic::HostGeometryType::Box;
        tmpl.geometry_info.major_radius = 6.0;
        tmpl.geometry_info.minor_radius = 2.0;
        logic.templates.insert("TsingMaLandmarkBridge".into(), tmpl);
        let id = logic
            .create_object(
                "TsingMaLandmarkBridge",
                Team::Neutral,
                Vec3::new(10.0, 5.0, 20.0),
            )
            .expect("spawn landmark");
        logic.register_landmark_bridges_from_spawned_objects();

        let owner = logic.world_services.terrain().clone();
        let info = owner
            .read()
            .expect("owner terrain")
            .get_first_bridge()
            .expect("owned span")
            .get_bridge_info()
            .clone();
        assert_eq!(info.bridge_object_id, id.0);
        assert!((info.bridge_width - 4.0).abs() < 0.01);
        assert!(logic.bridge_behavior.span(id).is_some());
        let deck = owner
            .read()
            .ok()
            .and_then(|tl| tl.host_deck_height_at(10.0, 20.0));
        assert!(
            deck.is_some_and(|z| (z - 5.0).abs() < 0.05),
            "deck height must come from leftover landmark span, got {deck:?}"
        );

        let frozen = crate::presentation_frame::PresentationWorldEnv::from_logic(&logic);
        assert!(frozen.height_samples_from_terrain);
        assert_eq!((frozen.height_grid_w, frozen.height_grid_h), (64, 64));
        assert!(frozen.height_samples.iter().any(|&height| height > 4.9));
        for (index, &height) in frozen.height_samples.iter().enumerate() {
            let x = (index % 64) as f32 - 31.5;
            let z = (index / 64) as f32 - 31.5;
            let scalar = logic.terrain_height_at(Vec3::new(x, 0.0, z)).unwrap_or(0.0);
            assert_eq!(height.to_bits(), scalar.to_bits(), "minimap cell ({x}, {z})");
        }
    }
}

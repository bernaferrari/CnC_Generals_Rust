//! Host-world validation and parity tests.
#![allow(unused_imports, non_snake_case)]
use super::*;

#[cfg(test)]
mod sides_host_apply_tests {
    use super::*;

    #[test]
    fn map_side_dict_sets_host_money_color_and_enemies() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "PlyrAmerica", true));
        logic.add_player(Player::new(1, Team::GLA, "PlyrGLA", false));

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

        logic.apply_host_players_from_side_dicts(&[america, gla], true);

        let usa = logic.get_player(0).expect("usa");
        assert_eq!(usa.resources.supplies, 3_000);
        assert_eq!(usa.color_rgb, (0, 0, 0xff));
        assert_eq!(
            logic.player_relationship(0, 1),
            gamelogic::common::Relationship::Enemies
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
    fn world_info_weather_snowy_sets_runtime_weather_state() {
        let mut logic = GameLogic::new();
        assert!(
            !logic
                .weather_state()
                .current_weather
                .to_ascii_lowercase()
                .contains("snow")
        );
        logic.apply_world_info_weather(Some(1));
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
            observer.is_local,
            "ReplayObserver must be local in GAME_REPLAY"
        );
        assert_eq!(observer.name, "ReplayObserver");
        assert_eq!(
            logic.players.values().filter(|p| p.is_local).count(),
            1,
            "only ReplayObserver is local"
        );
        assert!(logic.radar_forced);
        if let Ok(list) = ThePlayerList().read() {
            if let Some(local) = list.get_local_player() {
                if let Ok(guard) = local.read() {
                    assert_eq!(
                        guard.get_player_name_key(),
                        NameKeyGenerator::name_to_key("ReplayObserver")
                    );
                }
            }
        }
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

    #[test]
    fn load_map_data_sites_call_leftover_new_map() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game_logic/world_save.rs"
        ));
        let prod = src.split("#[cfg(test)]").next().expect("production");
        assert!(
            prod.matches("terrain.new_map(false)").count() >= 2,
            "both leftover load_map_data sites must call TerrainLogic::newMap"
        );
        assert!(prod.contains("register_spawned_landmark_bridges"));
        assert!(prod.contains("add_landmark_bridge_from_geometry"));
    }

    #[test]
    fn landmark_bridge_object_registers_leftover_deck() {
        {
            let mut terrain = gamelogic::terrain::get_terrain_logic()
                .write()
                .expect("terrain");
            terrain.reset();
        }
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("TsingMaLandmarkBridge");
        tmpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::LandmarkBridge)
            .add_kind_of(KindOf::Bridge)
            .set_health(400.0);
        tmpl.geometry_info.authored = true;
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

        let info = leftover_bridge_info_for_object(id.0).expect("leftover span");
        assert_eq!(info.bridge_object_id, id.0);
        assert!((info.bridge_width - 4.0).abs() < 0.01);
        assert!(logic.bridge_behavior.span(id).is_some());
        let deck = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|tl| tl.host_deck_height_at(10.0, 20.0));
        assert!(
            deck.is_some_and(|z| (z - 5.0).abs() < 0.05),
            "deck height must come from leftover landmark span, got {deck:?}"
        );
    }
}

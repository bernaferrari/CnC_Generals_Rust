//! Cold-owner regression for the six former first-use shroud-grid writes.
//! Register as a child test module of game_logic::game_logic::world_scripts.
use crate::game_logic::{GameLogic, Player, Team};
use glam::Vec3;

const CONTENT: &str = r#"
Object AmericaCommandCenter
  KindOf = STRUCTURE COMMANDCENTER
  Body = ActiveBody CommandCenterBody
    MaxHealth = 5000
    InitialHealth = 5000
  End
End
Object ChinaCommandCenter
  KindOf = STRUCTURE COMMANDCENTER
  Body = ActiveBody CommandCenterBody
    MaxHealth = 5000
    InitialHealth = 5000
  End
End
Object SpecialPowerViewObject
  KindOf = UNATTACKABLE
  Body = ActiveBody ViewBody
    MaxHealth = 1
    InitialHealth = 1
  End
End
"#;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn install_templates_and_players(
    world: &mut GameLogic,
) -> (crate::game_logic::ObjectId, crate::game_logic::ObjectId) {
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "cold-grid-world-tests.ini".into(),
                CONTENT.to_owned(),
            )])
            .expect("authored fixture content"),
        )
        .expect("admit fixture definitions before object birth");
    world.add_player(Player::new(0, Team::USA, "ColdGridUSA", true));
    world.add_player(Player::new(1, Team::China, "ColdGridChina", false));
    world
        .get_player_mut(0)
        .unwrap()
        .set_map_relationship(1, gamelogic::common::Relationship::Enemies);
    let source = world
        .create_object_for_player("AmericaCommandCenter", 0, Vec3::new(32.0, 0.0, 32.0))
        .expect("live admitted USA command center");
    let enemy = world
        .create_object_for_player("ChinaCommandCenter", 1, Vec3::new(96.0, 0.0, 96.0))
        .expect("live admitted China command center");
    (source, enemy)
}

fn grid(world: &GameLogic) -> Option<(usize, usize, f32)> {
    world
        .world_services
        .shroud()
        .lock()
        .expect("world-owned shroud")
        .grid_dimensions()
}

#[test]
fn cold_world_handlers_do_not_create_grid_or_reset_peer_admission() {
    isolated(
        "cold_world_handlers_do_not_create_grid_or_reset_peer_admission",
        || {
            let mut cold = GameLogic::new();
            let (source, enemy) = install_templates_and_players(&mut cold);
            assert_eq!(grid(&cold), None, "GameLogic construction is inert");

            // The peer uses the existing receiving-grid initializer with a
            // non-default admitted cell size. A query in `cold` must not touch it.
            let mut peer = GameLogic::new();
            peer.world_width = 256.0;
            peer.world_height = 192.0;
            peer.admit_camera_application_movement(
                game_engine::common::ini::ini_game_data::CameraMovementRules {
                    partition_cell_size: 64.0,
                    ..Default::default()
                },
            );
            peer.initialize_receiving_partition_grid()
                .expect("peer receiving grid");
            assert_eq!(grid(&peer), Some((4, 3, 64.0)));

            // CIA Intelligence: live enemy object reaches the per-unit shroud branch.
            assert!(cold.activate_cia_intelligence(0, Team::USA, Some(source)));
            assert!(
                cold.host_object(enemy)
                    .unwrap()
                    .is_vision_spied_by_player(0)
            );
            assert_eq!(grid(&cold), None);

            // Satellite Hack I: call the real upgrade implementation entry with its
            // authored module spec and a live, owned command center.
            let spec = crate::game_logic::host_satellite_hack::satellite_hack_spy_spec(
                "Upgrade_China_SatelliteHackOne",
            )
            .expect("authored Satellite Hack I spec");
            assert!(cold.activate_satellite_hack_spy_vision(source, spec));
            assert_eq!(grid(&cold), None);

            // RadarScan and SpySatellite activation both need real player identities
            // and source objects; neither should create the map grid as a side effect.
            assert!(cold.activate_radar_scan(
                0,
                Team::USA,
                Vec3::new(100.0, 0.0, 100.0),
                Some(source)
            ));
            assert_eq!(grid(&cold), None);
            assert!(cold.activate_spy_satellite(
                0,
                Team::USA,
                Vec3::new(120.0, 0.0, 120.0),
                Some(source),
            ));
            assert_eq!(grid(&cold), None);

            // SpecialPowerModule::createViewObject production handler must succeed by
            // creating its live view object while FOW itself remains unavailable.
            assert!(cold.create_special_power_view_object_at(
                source,
                Vec3::new(140.0, 0.0, 140.0),
                80.0,
                30,
            ));
            assert_eq!(grid(&cold), None);

            // Drive the actual receiving beam update and its due scorch pulse. The
            // registry insertion is the same production field producer used by the
            // existing beam update fixture; the source is a live world object.
            cold.special_power_strikes.spawn_beam_field(
                source,
                Team::USA,
                Vec3::new(160.0, 0.0, 160.0),
                cold.frame,
                1,
            );
            cold.update_particle_beam_fields();
            assert_eq!(
                cold.special_power_strikes.beam_fields()[0].scorch_marks_made,
                1,
                "actual beam handler consumed a due scorch pulse"
            );
            assert_eq!(grid(&cold), None);

            assert_eq!(
                grid(&peer),
                Some((4, 3, 64.0)),
                "cold-world handlers must not create/reset another owner's admitted grid"
            );
        },
    );
}

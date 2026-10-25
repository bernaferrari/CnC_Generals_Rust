//! Actual Host impact admission borrows only the driving terrain, after jitter.
use super::*;
use crate::game_logic::GameLogic;

fn loaded(raw: u8) -> GameLogic {
    let world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 8;
    map.height = 8;
    map.heightmap = vec![raw; 64];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    world
}

#[test]
fn actual_host_strafe_impact_uses_receiving_ground_with_foreign_owner_locked() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_host_strafe_impact_uses_receiving_ground_with_foreign_owner_locked",
        || {
            let a = loaded(16);
            let mut b = loaded(80);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let owner = foreign.services().terrain();
            let mut guard = owner.write().unwrap();
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 8;
            map.height = 8;
            map.heightmap = vec![240; 64];
            guard.try_load_map_geometry(map).unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let impact = |world: &GameLogic| {
                    spectre_gattling_strafe_smoke_impact(
                        Vec3::new(30., 999., 30.),
                        |x, z| world.terrain_ground_height_at(Vec3::new(x, 0., z)),
                        |_, _| 0.,
                    )
                };
                assert_eq!(impact(&a).y, 10.);
                assert_eq!(impact(&b).y, 50.);
                // Construction alone cannot publish over either driving owner.
                let _peer = GameLogic::new();
                guard
                    .try_apply_logic_height_map_bytes_with_effects(&[8; 64], &mut || {})
                    .unwrap();
                assert_eq!(impact(&a).y, 10.);
                assert_eq!(impact(&b).y, 50.);
                b.reset();
                assert_eq!(impact(&a).y, 10.);
            });
        },
    );
}

#[test]
fn explicit_absent_ground_retains_existing_mapless_aim_height() {
    let impact =
        spectre_gattling_strafe_smoke_impact(Vec3::new(30., 123., 30.), |_, _| None, |_, _| 0.);
    assert_eq!(impact.y, 123.);
}

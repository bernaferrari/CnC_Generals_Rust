//! Receiving projectile initialization and retarget use the same actual owner.
use super::*;

fn world(raw: u8) -> GameLogic {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![raw; 1024];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(32, 32)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content("Object ReceivingGroundProjectile\n Behavior = DumbProjectileBehavior Flight\n FirstHeight = 10\n SecondHeight = 20\n FirstPercentIndent = 0.25\n SecondPercentIndent = 0.75\n FlightPathAdjustDistPerSecond = 60\n End\nEnd\n", "receiving-projectile.ini").unwrap();
    assert!(
        world
            .projectile_definitions
            .admit_definition(
                "ReceivingGroundProjectile",
                parser.get_definition("ReceivingGroundProjectile").unwrap(),
                0.,
            )
            .unwrap()
    );
    let mut target = ThingTemplate::new("LiveProjectileTarget");
    target.add_kind_of(KindOf::Vehicle).set_health(100.);
    let mut target = Object::new(target, ObjectId(42), Team::GLA);
    target.set_position(Vec3::new(120., 20., 30.));
    assert!(target.is_alive());
    world.objects.insert(target.id, target);
    world
}
fn initialize(world: &mut GameLogic) -> ObjectId {
    let mut shot = tests::lifecycle_test_pending_projectile(
        "ReceivingGroundProjectile",
        Some(ObjectId(42)),
        Vec3::new(120., 20., 30.),
    );
    shot.shooter_pos = Vec3::new(20., 20., 30.);
    shot.speed = 30.;
    // Actual synchronous accepted-shot admission, not a second path builder.
    world.materialize_owned_passive_shot(shot, None);
    assert_eq!(world.combat_system.projectiles.len(), 1);
    *world.combat_system.projectiles.keys().next().unwrap()
}
fn peak(world: &GameLogic, id: ObjectId) -> f32 {
    world.combat_system.projectiles[&id]
        .flight_runtime
        .path
        .iter()
        .map(|p| p.y)
        .fold(f32::MIN, f32::max)
}
fn move_target(world: &mut GameLogic) {
    world
        .objects
        .get_mut(&ObjectId(42))
        .unwrap()
        .set_position(Vec3::new(150., 20., 30.));
}
#[test]
fn actual_receiving_projectile_initialization_and_retarget_ignore_locked_foreign_terrain() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_receiving_projectile_initialization_and_retarget_ignore_locked_foreign_terrain",
        || {
            let mut a = world(16);
            let mut b = world(80);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let mut foreign_terrain = foreign.services().terrain().write().unwrap();
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 32;
            map.height = 32;
            map.heightmap = vec![240; 1024];
            foreign_terrain.try_load_map_geometry(map).unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let aid = initialize(&mut a);
                let bid = initialize(&mut b);
                assert_eq!(aid, bid);
                assert!(peak(&b, bid) > peak(&a, aid) + 10.);
                let a_before = a.combat_system.projectiles[&aid]
                    .flight_runtime
                    .path
                    .clone();
                let b_before = b.combat_system.projectiles[&bid]
                    .flight_runtime
                    .path
                    .clone();
                move_target(&mut a);
                move_target(&mut b);
                a.update_owned_projectile_impacts(1. / 30.);
                b.update_owned_projectile_impacts(1. / 30.);
                assert_ne!(
                    a.combat_system.projectiles[&aid].flight_runtime.path,
                    a_before
                );
                assert_ne!(
                    b.combat_system.projectiles[&bid].flight_runtime.path,
                    b_before
                );
                assert!(peak(&b, bid) > peak(&a, aid) + 10.);
                assert_eq!(
                    a.combat_system.projectiles[&aid].flight_runtime.path_end.x,
                    122.
                );
                assert_eq!(
                    b.combat_system.projectiles[&bid].flight_runtime.path_end.x,
                    122.
                );
                // Real encoded arena continuation restores state, not launch callbacks.
                let encoded =
                    serde_json::to_vec(&a.combat_system.live_projectile_arena_snapshot()).unwrap();
                let saved: LiveProjectileArenaSnapshot = serde_json::from_slice(&encoded).unwrap();
                let mut restored = world(16);
                move_target(&mut restored);
                restored
                    .combat_system
                    .restore_live_projectile_arena(saved.validate().unwrap());
                b.reset();
                foreign_terrain
                    .try_apply_logic_height_map_bytes_with_effects(&[8; 1024], &mut || {})
                    .unwrap();
                a.update_owned_projectile_impacts(1. / 30.);
                restored.update_owned_projectile_impacts(1. / 30.);
                assert_eq!(
                    a.combat_system.projectiles[&aid].flight_runtime.path,
                    restored.combat_system.projectiles[&aid].flight_runtime.path,
                );
                assert_eq!(
                    a.combat_system.projectiles[&aid].position,
                    restored.combat_system.projectiles[&aid].position
                );
            });
        },
    );
}

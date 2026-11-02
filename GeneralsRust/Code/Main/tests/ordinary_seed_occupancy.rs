use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::{PathfindingGround, PathfindingSystem};
use generals_main::game_logic::{
    GameLogic, HostGeometryType, KindOf, ObjectId, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn fixture(
    stamp_enemy: bool,
    fixed: bool,
    crusher: u8,
) -> (GameLogic, PathfindingSystem, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world.add_player(Player::new(1, Team::GLA, "Enemy", false));
    let mut unit = ThingTemplate::new("Mover");
    unit.add_kind_of(KindOf::Vehicle).set_health(100.0);
    unit.geometry_info.geom_type = HostGeometryType::Sphere;
    unit.geometry_info.major_radius = 2.0;
    unit.geometry_info.authored = true;
    world.templates.insert("Mover".into(), unit);
    let mover = world
        .create_object_for_player("Mover", 0, Vec3::new(25.0, 0.0, 55.0))
        .unwrap();
    let enemy_pos = Vec3::new(75.0, 0.0, 55.0);
    let enemy = world
        .create_object_for_player("Mover", 1, enemy_pos)
        .unwrap();
    world.host_object_mut(mover).unwrap().crusher_level = crusher;
    world.host_object_mut(enemy).unwrap().crushable_level = 1;
    let mut finder = PathfindingSystem::new(200.0, 120.0);
    finder.grid.set_cell_type(
        generals_main::game_logic::pathfinding::GridPos::new(3, 4),
        gamelogic::ai::pathfind_astar::PathfindCellType::Impassable,
    );
    if stamp_enemy {
        let footprint = finder
            .grid
            .path_position_footprint(world.host_object(enemy).unwrap());
        finder
            .grid
            .update_path_position_cell(&footprint, (-1, -1), enemy_pos);
        if fixed {
            finder
                .grid
                .update_ground_goal_cell(enemy.0, 1, 2.0, false, (-1, -1), enemy_pos);
        }
    }
    (world, finder, mover)
}

#[test]
fn ordinary_seed_uses_receiving_world_moving_and_crushable_occupants() {
    let mut empty = fixture(false, false, 0);
    let mut moving = fixture(true, false, 0);
    let mut crushable = fixture(true, true, 2);
    let mut unmanned = fixture(true, true, 2);
    unmanned
        .0
        .host_object_mut(unmanned.2)
        .unwrap()
        .status
        .disabled_unmanned = true;
    let mut fixed_ally = fixture(true, true, 0);
    let mut ally_masks = [0u16; 16];
    ally_masks[0] = (1 << 0) | (1 << 1);
    fixed_ally.1.grid.set_player_ally_masks(ally_masks);
    assert_eq!(
        (empty.2, moving.2, crushable.2),
        (empty.2, empty.2, empty.2)
    );
    let query = |state: &mut (GameLogic, PathfindingSystem, ObjectId)| {
        state
            .1
            .find_path_ex_surfaces(
                Vec3::new(25.0, 0.0, 55.0),
                Vec3::new(145.0, 0.0, 55.0),
                state.0.host_objects(),
                false,
                SURFACE_GROUND,
                false,
                Some(state.2),
                &PathfindingGround::Flat(0.0),
            )
            .expect("ordinary path")
    };
    let expected = query(&mut empty);
    let moving_path = query(&mut moving);
    let crushable_path = query(&mut crushable);
    let ally_path = query(&mut fixed_ally);
    let unmanned_path = query(&mut unmanned);
    assert_eq!(
        moving_path, expected,
        "a moving enemy must not block line seeding"
    );
    assert_eq!(
        crushable_path, expected,
        "a crushable fixed enemy must not block line seeding"
    );
    assert_ne!(ally_path, expected, "a fixed ally must stop line seeding");
    assert_ne!(
        unmanned_path, expected,
        "an unmanned mover must not crush the fixed enemy"
    );
    assert_eq!(
        query(&mut empty),
        expected,
        "peer queries must not alter the first owner"
    );
}

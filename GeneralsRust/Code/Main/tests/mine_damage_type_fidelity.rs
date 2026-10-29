use gamelogic::common::Relationship;
use generals_main::game_logic::{GameLogic, GameMode, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    let mut china = Player::new(0, Team::China, "China", true);
    china.set_map_relationship(1, Relationship::Enemies);
    let mut usa = Player::new(1, Team::USA, "USA", false);
    usa.set_map_relationship(0, Relationship::Enemies);
    world.add_player(china);
    world.add_player(usa);

    let mut structure = ThingTemplate::new("StructureArmorMineWitness");
    structure
        .add_kind_of(KindOf::Structure)
        .set_health(10_000.0)
        .set_primary_weapon_none();
    // Leaving ArmorDefinitions absent deliberately selects Main's mapless
    // StructureArmor residual, whose LandMine coefficient is 0 and whose
    // EXPLOSION coefficient is 1. This exercises actual splash admission while
    // avoiding an inaccessible crate-private catalog-admission hook.
    world
        .templates
        .insert(structure.name.clone(), structure);
    world
}

fn structure_victim(world: &mut GameLogic, position: Vec3) -> generals_main::game_logic::ObjectId {
    world
        .create_object_for_player("StructureArmorMineWitness", 1, position)
        .expect("StructureArmor victim")
}

#[test]
fn land_mine_detonation_uses_land_mine_armor_type() {
    let mut world = world();
    let position = Vec3::ZERO;
    let victim = structure_victim(&mut world, position);
    let mine = world
        .place_land_mine(Team::China, position, None)
        .expect("place land mine");
    world.host_object_mut(mine).unwrap().owner_player_id = Some(0);
    {
        let data = world
            .host_object_mut(mine)
            .unwrap()
            .mine_data
            .as_mut()
            .unwrap();
        data.proximity_enabled = true;
        data.trigger_range = 30.0;
        data.detonation_damage = 500.0;
        data.detonation_radius = 100.0;
    }
    assert!(world.manual_detonate_mine(mine));

    assert_eq!(
        world.host_object(victim).unwrap().health.current,
        10_000.0,
        "StructureArmor's 0% LAND_MINE coefficient must absorb the real manual mine blast"
    );
    assert_eq!(world.mine_residual_manual_detonations(), 1);
}

#[test]
fn manual_demo_trap_uses_explosion_armor_type() {
    let mut world = world();
    let position = Vec3::ZERO;
    let victim = structure_victim(&mut world, position);
    let trap = world
        .place_demo_trap(Team::China, position, None)
        .expect("place demo trap");

    assert!(world.manual_detonate_mine(trap));

    let remaining = world.host_object(victim).unwrap().health.current;
    assert_eq!(
        remaining, 9_400.0,
        "Standard DemoTrap's 600 EXPLOSION damage uses StructureArmor's 100% coefficient"
    );
}

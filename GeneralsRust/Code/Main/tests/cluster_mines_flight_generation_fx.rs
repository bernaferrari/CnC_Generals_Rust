use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world_with_cluster_bomb(
    player: u32,
    position: Vec3,
    mine_name: &str,
    generation_fx: Option<&str>,
    include_mine_template: bool,
) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(player, Team::China, "cluster owner", true));

    let mut ini = format!(
        "Object ClusterMinesBomb\n KindOf = PROJECTILE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\n Behavior = GenerateMinefieldBehavior MinefieldTag\n MineName = {mine_name}\n SmartBorder = Yes\n DistanceAroundObject = 80\n"
    );
    if let Some(fx) = generation_fx {
        ini.push_str(&format!(" GenerationFX = {fx}\n"));
    }
    ini.push_str("End\nEnd\nObject TestClusterSource\n KindOf = SELECTABLE STRUCTURE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject ChinaJetCargoPlane\n KindOf = AIRCRAFT VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 600\n InitialHealth = 600\n End\nEnd\n");
    if include_mine_template {
        ini.push_str(&format!(
            "Object {mine_name}\n KindOf = SELECTABLE VEHICLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n"
        ));
    }
    world
        .admit_object_ini_sources([("ClusterMinesFlightGenerationFx.ini".into(), ini)])
        .unwrap();

    // The production flight admission must materialize its authored payload,
    // not install a bare same-named fallback before it reaches the factory.
    let source = world
        .create_object_for_player("TestClusterSource", player, position)
        .expect("authored source");
    world
        .spawn_cluster_mines_flight(source, position)
        .expect("authored cluster flight");

    // Start the live impact phase with an already-falling payload. This uses
    // the same public per-frame flight API that processes produced bombs.
    let bomb = world
        .create_object_for_player("ClusterMinesBomb", player, position)
        .expect("admitted ClusterMinesBomb");
    let payload = world.host_object_mut(bomb).unwrap();
    payload.cluster_mines_bomb = true;
    payload.movement.velocity = Vec3::ZERO;
    (world, bomb)
}

fn impact(world: &mut GameLogic) {
    world.update_cluster_mines_flights().unwrap();
}

fn mine_names(world: &GameLogic) -> Vec<String> {
    world
        .host_objects()
        .registered_values()
        .filter(|object| object.is_alive() && object.template_name.ends_with("ReceivingMine"))
        .map(|object| object.template_name.clone())
        .collect()
}

#[test]
fn authored_mine_name_is_resolved_from_each_receiving_world_on_the_real_impact_path() {
    let (mut left, left_bomb) = world_with_cluster_bomb(
        1,
        Vec3::new(188.0, 4.0, 190.0),
        "LeftReceivingMine",
        Some("FX_LeftClusterMine"),
        true,
    );
    let (mut right, right_bomb) = world_with_cluster_bomb(
        2,
        Vec3::new(311.0, 3.0, 337.0),
        "RightReceivingMine",
        Some("FX_RightClusterMine"),
        true,
    );
    assert_eq!(
        left_bomb, right_bomb,
        "peer bomb IDs are intentionally reused"
    );

    impact(&mut left);
    impact(&mut right);

    let left_mines = mine_names(&left);
    let right_mines = mine_names(&right);
    assert!(
        !left_mines.is_empty(),
        "left impact should place its minefield"
    );
    assert!(
        !right_mines.is_empty(),
        "right impact should place its minefield"
    );
    assert!(left_mines.iter().all(|name| name == "LeftReceivingMine"));
    assert!(right_mines.iter().all(|name| name == "RightReceivingMine"));
}

#[test]
fn unresolved_authored_mine_template_stops_real_impact_before_mine_births() {
    let (mut world, _) = world_with_cluster_bomb(
        1,
        Vec3::new(200.0, 4.0, 200.0),
        "MissingReceivingMine",
        Some("FX_ReceivingClusterMine"),
        false,
    );

    impact(&mut world);

    assert!(mine_names(&world).is_empty());
}

#[test]
fn nullable_generation_fx_does_not_block_authored_mine_placement_on_real_impact() {
    let (mut world, _) =
        world_with_cluster_bomb(1, Vec3::new(200.0, 4.0, 200.0), "ReceivingMine", None, true);

    impact(&mut world);

    assert!(!mine_names(&world).is_empty());
}

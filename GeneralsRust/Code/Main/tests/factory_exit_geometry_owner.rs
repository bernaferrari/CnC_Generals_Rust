use generals_main::game_logic::{GameLogic, Player, Team};
use glam::Vec3;

fn world(exit_width: f32) -> GameLogic {
    let mut world = GameLogic::new();
    world.override_world_size(2000.0, 2000.0);
    world.admit_object_ini_sources([("factory.ini".into(), format!(
        "Object SameFactory\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n FactoryExitWidth = {exit_width}\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject Blocker\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 5\n GeometryMinorRadius = 5\n GeometryHeight = 10\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
    ))]).unwrap();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world
        .create_object_for_player("SameFactory", 0, Vec3::new(460.0, 0.0, 500.0))
        .unwrap();
    world
}

#[test]
fn factory_exit_uses_receiving_template_during_legality_query() {
    let wide = world(60.0);
    let narrow = world(0.0);
    let p = Vec3::new(500.0, 0.0, 500.0);
    let wide_code = wide.legal_build_code_at(Team::USA, p, "Blocker");
    let narrow_code = narrow.legal_build_code_at(Team::USA, p, "Blocker");
    assert_eq!(wide_code, 3, "wide exit: LBC_OBJECTS_IN_THE_WAY");
    assert_eq!(narrow_code, 0, "narrow exit: LBC_OK");
    assert_eq!(wide_code, wide.legal_build_code_at(Team::USA, p, "Blocker"));
}

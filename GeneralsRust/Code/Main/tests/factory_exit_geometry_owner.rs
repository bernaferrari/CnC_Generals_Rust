use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;

fn world(exit_width: f32) -> GameLogic {
    let mut world = GameLogic::new();
    world.override_world_size(2000.0, 2000.0);
    world.admit_object_ini_sources([("factory.ini".into(), format!(
        "Object SameFactory\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n FactoryExitWidth = {exit_width}\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject Blocker\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
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

#[test]
fn candidate_factory_exit_detects_immobile_structure_outside_footprint() {
    fn receiving_world(exit_width: f32, blocker_z: f32) -> (GameLogic, ObjectId) {
        let mut world = GameLogic::new();
        world.override_world_size(2000.0, 2000.0);
        world.admit_object_ini_sources([("candidate-factory.ini".into(), format!(
            "Object CandidateFactory\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 10\n GeometryMinorRadius = 10\n GeometryHeight = 10\n FactoryExitWidth = {exit_width}\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject ExitBlocker\n KindOf = STRUCTURE IMMOBILE\n Geometry = BOX\n GeometryMajorRadius = 5\n GeometryMinorRadius = 5\n GeometryHeight = 10\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        ))]).unwrap();
        world.add_player(Player::new(0, Team::USA, "Owner", true));
        let blocker = world
            .create_object_for_player("ExitBlocker", 0, Vec3::new(540.0, 0.0, blocker_z))
            .unwrap();
        (world, blocker)
    }
    let (wide, wide_id) = receiving_world(60.0, 500.0);
    let (narrow, narrow_id) = receiving_world(0.0, 500.0);
    assert_eq!(wide_id, narrow_id);
    let (wide_side, _) = receiving_world(60.0, 520.0);
    let position = Vec3::new(500.0, 0.0, 500.0);
    assert_eq!(
        wide.legal_build_code_at(Team::USA, position, "CandidateFactory"),
        3,
        "the exit overlaps a structure even though the main footprints do not"
    );
    assert_eq!(
        narrow.legal_build_code_at(Team::USA, position, "CandidateFactory"),
        0,
        "the same object IDs and template names in the peer have no exit overlap"
    );
    assert_eq!(
        wide_side.legal_build_code_at(Team::USA, position, "CandidateFactory"),
        0,
        "the side of the exit rectangle must not inherit a circular blocker"
    );
}

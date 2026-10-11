//! Actual team-factory Xfer carries visitation order, never alphabetic or ID order.
use super::player_team_persist::{
    apply_teams_to_leftover, capture_team_factory_chunk, parse_team_factory_block,
};
use super::*;
use crate::game_logic::GameLogic;
use std::io::Cursor;

fn definitions(world: &GameLogic) {
    let mut factory = world.team_factory.lock().unwrap();
    for name in ["Zulu", "Alpha"] {
        factory
            .init_team_owned(name.into(), "Owner".into(), false, None, Some(7))
            .unwrap();
    }
}
#[test]
fn team_prototype_admission_order_round_trips_actual_factory_xfer() {
    let source = GameLogic::new();
    let destination = GameLogic::new();
    let peer = GameLogic::new();
    for world in [&source, &destination, &peer] {
        definitions(world);
    }
    source
        .team_factory
        .lock()
        .unwrap()
        .restore_prototype_admission_order(&["Alpha".into(), "Zulu".into()])
        .unwrap();
    let saved = capture_team_factory_chunk(&source.team_factory).unwrap();
    assert_eq!(
        saved
            .prototypes
            .iter()
            .map(|prototype| prototype.team_name.as_str())
            .collect::<Vec<_>>(),
        ["Alpha", "Zulu"]
    );
    let mut bytes = Cursor::new(Vec::new());
    let chunks = PlayerTeamChunks {
        teams: Some(saved),
        ..Default::default()
    };
    {
        let mut xfer = game_engine::common::system::xfer_save::XferSave::new(&mut bytes, 1);
        write_team_factory_block(&mut xfer, &chunks).unwrap();
    }
    let restored = parse_team_factory_block(&bytes.into_inner()).unwrap();
    apply_teams_to_leftover(&destination.team_factory, &restored).unwrap();
    assert_eq!(
        destination.team_factory.lock().unwrap().prototype_names(),
        ["Alpha", "Zulu"]
    );
    assert_eq!(
        peer.team_factory.lock().unwrap().prototype_names(),
        ["Zulu", "Alpha"]
    );
    assert_eq!(
        destination
            .team_factory
            .lock()
            .unwrap()
            .owned_team_prototypes_for_controller(7)
            .len(),
        2
    );
}

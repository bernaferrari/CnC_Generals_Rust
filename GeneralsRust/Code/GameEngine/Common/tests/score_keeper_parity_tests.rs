use game_engine::common::rts::score_keeper::{
    KindOf, KindOfMaskType, ScoreKeeper, MAX_PLAYER_COUNT,
};
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

fn scoring_unit_mask() -> KindOfMaskType {
    let mut mask = KindOfMaskType::new();
    mask.set(KindOf::Vehicle);
    mask.set(KindOf::Score);
    mask
}

#[test]
fn score_keeper_serializes_cpp_map_version_and_sixteen_player_slots() {
    let mut keeper = ScoreKeeper::new();
    let unit_mask = scoring_unit_mask();

    keeper.add_object_built("Tank", &unit_mask, false);
    keeper.add_object_destroyed("BossDozer", &unit_mask, 12, false);

    let serialized = keeper.serialize();
    let scalar_prefix_len =
        4 + 4 + 4 + (MAX_PLAYER_COUNT * 4) + 4 + 4 + (MAX_PLAYER_COUNT * 4) + 4 + 4 + 4 + 4 + 4 + 4;

    assert_eq!(MAX_PLAYER_COUNT, 16);
    assert_eq!(
        u32::from_le_bytes(
            serialized[scalar_prefix_len..scalar_prefix_len + 4]
                .try_into()
                .unwrap()
        ),
        1,
        "xferObjectCountMap starts with per-map version"
    );
    assert_eq!(
        u16::from_le_bytes(
            serialized[scalar_prefix_len + 4..scalar_prefix_len + 6]
                .try_into()
                .unwrap()
        ),
        1
    );

    let loaded = ScoreKeeper::deserialize(&serialized).expect("score keeper deserialize");
    assert_eq!(loaded.get_total_units_built(), 1);
    assert_eq!(loaded.get_total_units_destroyed(), 1);
}

#[test]
fn score_keeper_snapshot_xfer_preserves_full_score_state() {
    let mut source = ScoreKeeper::new();
    let unit_mask = scoring_unit_mask();

    source.reset(4);
    source.add_money_earned(5000);
    source.add_money_spent(1250);
    source.add_object_built("Tank", &unit_mask, false);
    source.add_object_destroyed("BossDozer", &unit_mask, 12, false);
    source.add_object_lost("LostTank", &unit_mask, false);
    source.calculate_score();

    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut xfer = XferSave::new(cursor, 1);
        source.xfer(&mut xfer).expect("save score keeper");
    }

    let mut loaded = ScoreKeeper::new();
    {
        let cursor = Cursor::new(bytes);
        let mut xfer = XferLoad::new(cursor, 1);
        loaded.xfer(&mut xfer).expect("load score keeper");
    }

    assert_eq!(loaded.get_total_money_earned(), 5000);
    assert_eq!(loaded.get_total_money_spent(), 1250);
    assert_eq!(loaded.get_total_units_built(), 1);
    assert_eq!(loaded.get_total_units_destroyed(), 1);
    assert_eq!(loaded.get_total_units_lost(), 1);
    assert_eq!(loaded.get_total_objects_built("Tank"), 1);
    assert_eq!(
        loaded.get_objects_destroyed_for_player(12).unwrap()["BossDozer"],
        1
    );
    assert_eq!(loaded.get_objects_lost_map()["LostTank"], 1);
}

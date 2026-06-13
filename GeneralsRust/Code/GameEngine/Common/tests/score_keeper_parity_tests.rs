use game_engine::common::rts::score_keeper::{
    KindOf, KindOfMaskType, ScoreKeeper, MAX_PLAYER_COUNT,
};

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

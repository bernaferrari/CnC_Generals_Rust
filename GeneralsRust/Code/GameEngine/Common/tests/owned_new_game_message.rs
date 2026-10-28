use game_engine::common::message_stream::{GameMessage, GameMessageType, MessageSerializer};

#[test]
fn request_map_is_cloned_with_the_message_without_changing_retail_bytes() {
    let mut first = GameMessage::new(GameMessageType::NewGame);
    for value in [2, 1, 0, 30] {
        first.append_integer_argument(value);
    }
    let retail_bytes = MessageSerializer::serialize(&first).unwrap();
    first.set_owned_new_game_map("Maps/First/First.map".into());
    let mut second = first.clone();
    second.set_owned_new_game_map("Maps/Second/Second.map".into());

    assert_eq!(first.owned_new_game_map(), Some("Maps/First/First.map"));
    assert_eq!(second.owned_new_game_map(), Some("Maps/Second/Second.map"));
    assert_eq!(MessageSerializer::serialize(&first).unwrap(), retail_bytes);
    assert_eq!(MessageSerializer::serialize(&second).unwrap(), retail_bytes);
    let restored = MessageSerializer::deserialize(&retail_bytes).unwrap();
    assert_eq!(restored.owned_new_game_map(), None);
    assert_eq!(restored.get_argument_count(), 4);
}

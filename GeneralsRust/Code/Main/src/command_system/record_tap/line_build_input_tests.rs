//! Original line-message facing and append-only host command payload controls.
use super::*;
use game_engine::common::message_stream::{MessageHeader, MessageSerializer};

fn facing_command(orientation: f32) -> GameCommand {
    GameCommand {
        command_type: CommandType::DozerConstructLineFacing {
            template_name: "ChinaWall".into(),
            start: Vec3::new(10.0, 2.0, 20.0),
            end: Vec3::new(10.0, 2.0, 40.0),
            orientation,
        },
        player_id: 7,
        command_id: 81,
        timestamp: SystemTime::UNIX_EPOCH,
        selected_units: vec![ObjectId(31), ObjectId(42)],
        modifier_keys: ModifierKeys::default(),
    }
}

#[test]
fn explicit_line_facing_survives_message_conversion_and_owned_admission() {
    // CPP GameLogicDispatch.cpp:1374/1390 reads facing at arg2 and end at
    // arg3. Neither explicit zero nor a non-heading angle may be inferred.
    for orientation in [0.0, -0.75] {
        let command = facing_command(orientation);
        let message = game_command_to_message(&command).expect("line message");
        assert_eq!(message.get_player_index(), 7);
        // GameMessage stores typed payload and separately appended arguments.
        // get_argument only queries the latter; the actual transport codec
        // emits the typed payload arguments in original dispatch order.
        let encoded = MessageSerializer::serialize(&message).expect("encode line transport");
        let header = MessageHeader::from_bytes(&encoded).expect("transport header");
        assert_eq!(header.message_type_id, 101);
        assert_eq!(header.player_index, 7);
        assert_eq!(header.argument_count, 4);
        let building_type = match message.get_type() {
            GameMessageType::DozerConstructLine(building_type, ..) => *building_type,
            other => panic!("expected line payload, got {other:?}"),
        };
        // Original arg0 integer/template, arg1 start, arg2 facing, arg3 end.
        let mut expected_args = vec![1];
        expected_args.extend_from_slice(&(building_type as i32).to_le_bytes());
        expected_args.push(8);
        for component in [10.0_f32, 2.0, 20.0] {
            expected_args.extend_from_slice(&component.to_le_bytes());
        }
        expected_args.push(2);
        expected_args.extend_from_slice(&orientation.to_le_bytes());
        expected_args.push(8);
        for component in [10.0_f32, 2.0, 40.0] {
            expected_args.extend_from_slice(&component.to_le_bytes());
        }
        assert_eq!(&encoded[MessageHeader::SIZE..], expected_args.as_slice());
        let transported = MessageSerializer::deserialize(&encoded).expect("decode line transport");
        assert_eq!(transported.get_player_index(), 7);
        assert!(matches!(transported.get_type(),
            GameMessageType::DozerConstructLine(_, start, end, facing)
                if *facing == orientation && start.x == 10.0 && start.y == 2.0 && start.z == 20.0
                    && end.x == 10.0 && end.y == 2.0 && end.z == 40.0));
        let mut pending = ReplayPendingState::default();
        apply_replay_messages_to_host(&mut pending, &[transported]);
        assert_eq!(pending.commands.len(), 1);
        let admitted = &pending.commands[0];
        assert_eq!(admitted.player_id, 7);
        assert_eq!(admitted.command_type, command.command_type);
        // Original message selection lives in the selected-group stream,
        // not in this line payload. Admission retains its remirror owner.
        assert!(admitted.selected_units.is_empty());
        assert_eq!(pending.take_selection_remirror(), vec![7]);
    }
}

#[test]
fn facing_payload_round_trip_retains_owner_and_ordered_selection() {
    let command = facing_command(-0.75);
    let data = bincode_legacy::serialize(&command).expect("serialize facing command");
    let decoded: GameCommand = bincode_legacy::deserialize(&data).expect("decode facing command");
    assert_eq!(decoded.command_type, command.command_type);
    assert_eq!(decoded.player_id, 7);
    assert_eq!(decoded.command_id, 81);
    assert_eq!(decoded.timestamp, SystemTime::UNIX_EPOCH);
    assert_eq!(decoded.selected_units, vec![ObjectId(31), ObjectId(42)]);
}

#[test]
fn original_line_payload_tag_and_layout_remain_unchanged() {
    // Accepted pre-facing types.rs SHA256
    // 8a4c383b321f0f6df1b2a3d298fae10d74b4ea7a66094b97ad322959ee646b70:
    // DozerConstructLine is ordinal37, followed by String/start/end only.
    let original = CommandType::DozerConstructLine {
        template_name: "ChinaWall".into(),
        start: Vec3::new(10.0, 2.0, 20.0),
        end: Vec3::new(10.0, 2.0, 40.0),
    };
    let mut expected = 37_u32.to_le_bytes().to_vec();
    expected.extend_from_slice(&9_u64.to_le_bytes());
    expected.extend_from_slice(b"ChinaWall");
    for component in [10.0_f32, 2.0, 20.0, 10.0, 2.0, 40.0] {
        expected.extend_from_slice(&component.to_le_bytes());
    }
    assert_eq!(bincode_legacy::serialize(&original).unwrap(), expected);
    assert_eq!(
        bincode_legacy::deserialize::<CommandType>(&expected).unwrap(),
        original
    );
}

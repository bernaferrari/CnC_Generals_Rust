//! Actual admitted Main capture/encode versus executed original-C++ segments.
//! Passive serialization inputs are installed directly, not through the codec
//! under test. Original inherited AI payload/gameplay remain outside this oracle.
use super::*;
use crate::game_logic::{GameLogic, Player, Team};
use crate::save_load::{XferLoad, XferSave};
use gamelogic::object::update::ai_update::dozer_ai_update::{
    DozerBuildSubTask, DozerDockPointInfo, DozerTask, owned_state::PrimaryState,
};
use glam::Vec3;
use std::io::Cursor;

const ORIGINAL_CPP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../GameEngine/GameLogic/src/object/update/ai_update/dozer_ai_update/snapshot/dozer_primary_original.jsonl"
));
fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn event(row: &serde_json::Value, label: &str) -> (usize, Option<usize>) {
    let text = row["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|value| value.as_str())
        .find(|text| text.rsplit(':').next() == Some(label))
        .unwrap_or_else(|| panic!("executed original trace lacks {label}"));
    let mut parts = text.split(':');
    let offset = parts.next().unwrap().parse().unwrap();
    let size = parts.next().unwrap().parse().ok();
    (offset, size)
}
fn passive_admitted_source(case: &str) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = 31;
    world.add_player(Player::new(0, Team::USA, "Original component", true));
    let content = crate::assets::ReceivingObjectContent::from_sources([(
        "original-dozer-xfer.ini".into(),
        "Object OriginalDozer\nKindOf = VEHICLE DOZER SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = DozerAIUpdate DozerTaskSource\nRepairHealthPercentPerSecond = 2\nEnd\nEnd\n".into(),
    )])
    .expect("actual authored content");
    world
        .admit_receiving_module_content(content)
        .expect("admission");
    let id = world
        .create_object_for_player("OriginalDozer", 0, Vec3::ZERO)
        .expect("actual typed factory")
        .expect("admitted Dozer");
    let actor = world.host_object_mut(id).unwrap();
    assert!(
        actor
            .thing()
            .template
            .receiving_client_template
            .as_ref()
            .and_then(|source| source.dozer_ai_identity())
            .is_some()
    );
    // This is a passive component fixture, deliberately not an update/command
    // test. The values match the C++ harness inputs, not its encoded output.
    let owner = &mut actor.dozer_tasks;
    (owner.primary.state, owner.current) = match case {
        "idle" => (PrimaryState::Idle, DozerTask::Invalid),
        "repair" => (PrimaryState::Repair, DozerTask::Repair),
        _ => panic!("unknown executed original fixture {case}"),
    };
    owner.primary.control.initialized = true;
    owner.primary.control.sleep_until = 77;
    owner.primary.control.default_id = 0;
    owner.primary.control.goal_object = 99;
    // Original XYZ=(11,22,33). Main uses XZ ground: literal setup swaps Y/Z.
    owner.primary.control.goal_position = Vec3::new(11.0, 33.0, 22.0);
    owner.primary.control.locked = true;
    // Core's private idle fields have serde data access. This does not call
    // save_primary/load_primary or create any expected byte representation.
    let mut idle = serde_json::to_value(&owner.primary.idle).unwrap();
    idle["idle_since"] = 31.into();
    idle["player"] = 2.into();
    idle["marked"] = true.into();
    owner.primary.idle = serde_json::from_value(idle).unwrap();
    owner.build_sub_task = DozerBuildSubTask::DoBuildAtDock;
    for i in 0..3 {
        owner.requests[i].target = Some(ObjectId(if i < 2 { 42 } else { 43 }));
        owner.requests[i].order_frame = 100 + (i / 2) as u32;
        for j in 0..3 {
            let step = (i * 10 + j) as f32;
            owner.docks[i][j] = DozerDockPointInfo {
                valid: (i + j) % 2 != 0,
                // Literal sourceXYZ=(100+step,200+step,300+step), manually
                // translated to MainXYZ. Includes all invalid raw locations.
                location: Vec3::new(100.0 + step, 300.0 + step, 200.0 + step),
            };
        }
    }
    (world, id)
}
#[test]
fn admitted_main_component_matches_executed_original_cpp_request_primary_and_dock_bytes() {
    let mut cases = Vec::new();
    for line in ORIGINAL_CPP.lines() {
        let row: serde_json::Value = serde_json::from_str(line).expect("executed C++ JSONL");
        let case = row["case"].as_str().unwrap();
        let (world, id) = passive_admitted_source(case);
        let mut component = DozerTaskObjectSnapshot::capture(world.host_object(id).unwrap())
            .expect("production admitted Main capture");
        let expected_source = component.source.clone().expect("typed source envelope");
        let mut encoded = Cursor::new(Vec::new());
        component
            .xfer(&mut XferSave::new(&mut encoded))
            .expect("actual Main component Xfer encode");
        let encoded = encoded.into_inner();
        // Remove only Main's explicit source envelope. These primitive reads
        // do not decode primary or prepare any fixture task/dock state.
        let mut cursor = Cursor::new(encoded.as_slice());
        {
            let mut reader = XferLoad::new(&mut cursor);
            let mut present = false;
            reader.xfer_bool(&mut present).unwrap();
            assert!(present);
            let mut ordinal = 0;
            let mut tag = String::new();
            reader.xfer_u32(&mut ordinal).unwrap();
            reader.xfer_string(&mut tag).unwrap();
            assert_eq!((ordinal, tag), expected_source);
        }
        let main_begin = cursor.position() as usize;
        let original = decode_hex(row["dozer_stubbed_base_hex"].as_str().unwrap());
        let original_primary = decode_hex(row["primary_hex"].as_str().unwrap());
        // Trace labels select the original derived-tail scope after the
        // original Dozer version and explicit inherited-AI stub call. No claim
        // is made about that excluded inherited payload or file/container.
        let (original_begin, _) = event(&row, "BASE_AI_STUB");
        let (first_target, _) = event(&row, "request0.target");
        assert_eq!(original_begin + 4, first_target, "original task count");
        let (last_frame, frame_size) = event(&row, "request2.frame");
        let primary_begin = last_frame + frame_size.unwrap();
        let (current_begin, current_size) = event(&row, "currentTask");
        let (first_dock, _) = event(&row, "dock00.valid");
        assert_eq!(current_begin + current_size.unwrap() + 4, first_dock);
        let (subtask_begin, subtask_size) = event(&row, "buildSubTask");
        assert_eq!(subtask_begin + subtask_size.unwrap(), original.len());
        assert_eq!(
            &original[primary_begin..current_begin],
            original_primary.as_slice(),
            "original primary standalone/tail consistency"
        );
        let request_bytes = primary_begin - original_begin;
        let length_offset = main_begin + request_bytes;
        assert_eq!(
            &encoded[main_begin..length_offset],
            &original[original_begin..primary_begin],
            "{case}: actual request count and all target/order fields"
        );
        // Remove only the explicit Main primary-length wrapper; compare the
        // actual payload directly, not a Rust-produced expected encoding.
        let primary_size = u32::from_le_bytes(
            encoded[length_offset..length_offset + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let main_primary = length_offset + 4;
        assert_eq!(primary_size, original_primary.len());
        assert_eq!(
            &encoded[main_primary..main_primary + primary_size],
            original_primary.as_slice(),
            "{case}: actual primary bytes, including one-way goal axis conversion"
        );
        assert_eq!(
            &encoded[main_primary + primary_size..],
            &original[current_begin..],
            "{case}: actual current, dock count, all nine valid/raw XYZ, subtask"
        );
        assert_eq!(row["invalid_docks_roundtrip"], true);
        cases.push(case.to_owned());
    }
    assert_eq!(
        cases,
        vec!["idle".to_owned(), "repair".to_owned()],
        "both original fixtures must execute"
    );
}

use game_engine::common::system::DataChunkOutput;
use game_engine::common::{dict::Dict, name_key_generator::NameKeyGenerator};
use gamelogic::scripting::{
    core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    },
    engine::ScriptEngine,
};
use generals_main::{
    game_logic::{BuildingData, BuildingType, GameLogic, GameMode, ObjectId, Player, Team},
    save_load::{Snapshot, SnapshotBuilder, XferLoad, XferSave, snapshot::WorldSnapshot},
};
use std::collections::HashMap;

const OBJECT_GRANT: &str = "Upgrade_OXFR_ObjectGrantedPlayerTyped";
const OBJECT_CREATE_GRANT: &str = "Upgrade_ObjectCreatedGrant";
const OLD_OWNER_RESEARCH: &str = "Upgrade_OldOwnerResearch";
const NEW_OWNER_RESEARCH: &str = "Upgrade_NewOwnerResearch";
const UNKNOWN_MAP_GRANT: &str = "Upgrade_NotInReceivingMap";

fn world(root: &std::path::Path, label: &str) -> (GameLogic, ObjectId, String) {
    let master = format!("{label}UpgradeMaster");
    let identity_building = format!("{label}GarrisonIdentityBuilding");
    let child = format!("{label}UpgradeChild");
    let create_child = format!("{label}CreateGrantChild");
    let ocl = format!("OCL_{label}UpgradeChild");
    let create_ocl = format!("OCL_{label}CreateGrantChild");
    let mut game = GameLogic::new();
    game.admit_object_creation_list_sources([format!(
        "ObjectCreationList {ocl}\n CreateObject\n  ObjectNames = {child}\n  Count = 1\n End\nEnd\n"
    ), format!(
        "ObjectCreationList {create_ocl}\n CreateObject\n  ObjectNames = {create_child}\n  Count = 1\n End\nEnd\n"
    )])
    .unwrap();
    game.admit_object_ini_sources([(
        format!("{label}Upgrade.ini"),
        format!(
            "Object {master}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = ObjectCreationUpgrade CreateNamedGrantChild\n  TriggeredBy = {OBJECT_GRANT} {NEW_OWNER_RESEARCH}\n  RequiresAllTriggers = Yes\n  UpgradeObject = {ocl}\n End\n Behavior = ObjectCreationUpgrade CreateOnCreateGrantChild\n  TriggeredBy = {OBJECT_CREATE_GRANT} {NEW_OWNER_RESEARCH}\n  RequiresAllTriggers = Yes\n  UpgradeObject = {create_ocl}\n End\n Behavior = GrantUpgradeCreate ModuleTag_ObjectGrant\n  UpgradeToGrant = {OBJECT_CREATE_GRANT}\n  ExemptStatus = UNDER_CONSTRUCTION\n End\nEnd\nObject {child}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\nObject {create_child}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\nObject {identity_building}\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\nObject MapGrantTarget\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
        ),
    )])
    .unwrap();

    let directory = root.join(label);
    std::fs::create_dir_all(&directory).unwrap();
    let map_path = directory.join("UpgradeAuthority.map");
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.open_data_chunk("Object", 3);
    for value in [20.0, 20.0, 0.0, 0.0] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string("MapGrantTarget");
    let mut grant_properties = Dict::new();
    grant_properties.set_ascii_string(
        NameKeyGenerator::name_to_key("objectGrantUpgrade0"),
        UNKNOWN_MAP_GRANT,
    );
    grant_properties.set_ascii_string(
        NameKeyGenerator::name_to_key("objectGrantUpgrade1"),
        OBJECT_GRANT,
    );
    grant_properties.set_ascii_string(NameKeyGenerator::name_to_key("upgrade"), UNKNOWN_MAP_GRANT);
    map.write_dict(&grant_properties);
    map.close_data_chunk();
    map.close_data_chunk();
    std::fs::write(&map_path, map.into_ckmp_bytes()).unwrap();
    std::fs::write(
        directory.join("Map.ini"),
        format!(
            "Upgrade {OBJECT_GRANT}\n Type = PLAYER\nEnd\nUpgrade {OBJECT_CREATE_GRANT}\n Type = OBJECT\nEnd\nUpgrade {OLD_OWNER_RESEARCH}\n Type = PLAYER\nEnd\nUpgrade {NEW_OWNER_RESEARCH}\n Type = PLAYER\nEnd\n"
        ),
    )
    .unwrap();

    game.start_new_game(GameMode::SinglePlayer);
    assert!(
        game.load_map(map_path.to_str().unwrap()),
        "authored MapGrantTarget and receiving Map.ini should admit"
    );
    game.add_player(Player::new(1, Team::USA, "OldOwner", true).with_script_identity("OldOwner"));
    game.add_player(
        Player::new(2, Team::China, "NewOwner", false).with_script_identity("NewOwner"),
    );
    let building_id = game
        .create_object_for_player(&identity_building, 1, glam::Vec3::new(30.0, 0.0, 30.0))
        .expect("create building for GOTS save witness");
    {
        let building_object = game.host_object_mut(building_id).unwrap();
        let mut building = BuildingData::new(BuildingType::Bunker);
        building.original_team_instance_id = building_object.team_instance_identity();
        building.original_team_instance_name = Some(building_object.team_instance_name.clone());
        building_object.building_data = Some(building);
    }
    let master_id = game
        .create_object_for_player(&master, 1, glam::Vec3::new(50.0, 0.0, 50.0))
        .expect("create receiving authored master");
    game.host_object_mut(master_id).unwrap().name = master;
    (game, master_id, child)
}

fn append_legacy_named_unit_history(bytes: &mut Vec<u8>) {
    let payload_name = b"LegacyNamedUnit";
    let mut payload = 1_u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&(payload_name.len() as u32).to_le_bytes());
    payload.extend_from_slice(payload_name);
    let mut frame = payload.clone();
    frame.extend_from_slice(&1_u32.to_le_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(b"NMLF");
    let footer = bytes
        .windows(4)
        .position(|tag| tag == b"OXFE")
        .expect("snapshot has OXFR footer");
    let record_end = footer + 4;
    bytes.splice(record_end..record_end, frame);
}

fn run_object_grant(game: &mut GameLogic, master: &str) {
    let mut action = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, master.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            OBJECT_GRANT.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "GrantObjectUpgrade".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    game.update_local_script_engine_with_owner(&engine).unwrap();
}

fn children(game: &GameLogic, master: &str) -> Vec<String> {
    let mut children: Vec<_> = game
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name != master)
        .filter(|object| !object.template_name.ends_with("GarrisonIdentityBuilding"))
        .filter(|object| object.template_name != "MapGrantTarget")
        .map(|object| object.template_name.clone())
        .collect();
    children.sort();
    children
}

#[test]
fn authored_map_ignores_unknown_object_grant_then_applies_valid_grant() {
    let root = tempfile::tempdir().unwrap();
    let (game, _, _) = world(root.path(), "MapGrant");
    let target = game
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "MapGrantTarget")
        .expect("authored map admitted its grant target");
    assert!(
        !target.applied_upgrades.contains(UNKNOWN_MAP_GRANT),
        "missing UpgradeTemplate is ignored without a partial grant"
    );
    assert!(
        target.applied_upgrades.contains(OBJECT_GRANT),
        "the following valid authored objectGrantUpgrade1 still applies"
    );
    assert!(target.completed_object_upgrades.contains(OBJECT_GRANT));
}

#[derive(serde::Serialize)]
struct LegacyOxfrV7 {
    stun: Vec<()>,
    battle_bus: Vec<()>,
    slow_death: Vec<()>,
    radar: Vec<()>,
    death_start: Vec<()>,
    retained_death: Vec<()>,
    object_creation_upgrade: Vec<()>,
    status_bits_executed_present: bool,
    cost_modifier_executed_present: bool,
}

fn make_ambiguous_v35_snapshot(snapshot: &WorldSnapshot, object_id: ObjectId) -> WorldSnapshot {
    use generals_main::save_load::snapshot::{ModuleSnapshot, UpgradeModuleSnapshot};

    let mut legacy: WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(snapshot).unwrap()).unwrap();
    legacy.version = 35;
    legacy.objects.get_mut(&object_id).unwrap().modules.insert(
        "Upgrade".into(),
        ModuleSnapshot::Upgrade(UpgradeModuleSnapshot {
            active_upgrades: vec![OBJECT_GRANT.into()],
            upgrade_progress: Default::default(),
        }),
    );
    let footer = legacy
        .lifecycle_tail
        .windows(4)
        .rposition(|bytes| bytes == b"OXFE")
        .expect("current snapshot contains OXFR locator");
    let payload_len = u32::from_le_bytes(
        legacy.lifecycle_tail[footer - 4..footer]
            .try_into()
            .unwrap(),
    ) as usize;
    let marker = footer - payload_len - 16;
    legacy.lifecycle_tail.drain(marker..footer + 4);
    let payload = bincode_legacy::serialize(&LegacyOxfrV7 {
        stun: vec![],
        battle_bus: vec![],
        slow_death: vec![],
        radar: vec![],
        death_start: vec![],
        retained_death: vec![],
        object_creation_upgrade: vec![],
        status_bits_executed_present: true,
        cost_modifier_executed_present: true,
    })
    .unwrap();
    let mut legacy_record = b"OXFR".to_vec();
    legacy_record.extend_from_slice(&7_u32.to_le_bytes());
    legacy_record.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    legacy_record.extend_from_slice(&payload);
    legacy.lifecycle_tail.splice(marker..marker, legacy_record);
    legacy
}

#[test]
fn object_grant_capture_and_restore_keep_object_and_player_upgrade_authority_separate() {
    let root = tempfile::tempdir().unwrap();
    let (mut receiving, receiving_id, receiving_child) = world(root.path(), "Receiving");
    let (mut peer, peer_id, _peer_child) = world(root.path(), "Peer");
    assert_eq!(
        receiving_id, peer_id,
        "sessions deliberately reuse the Object ID"
    );

    receiving
        .get_player_mut(1)
        .unwrap()
        .completed_upgrades
        .insert(OLD_OWNER_RESEARCH.into());
    peer.get_player_mut(1)
        .unwrap()
        .completed_upgrades
        .insert(OLD_OWNER_RESEARCH.into());
    receiving
        .get_player_mut(2)
        .unwrap()
        .completed_upgrades
        .insert(NEW_OWNER_RESEARCH.into());
    run_object_grant(&mut receiving, "ReceivingUpgradeMaster");
    run_object_grant(&mut peer, "PeerUpgradeMaster");
    for world in [&receiving, &peer] {
        let object = world.host_object(receiving_id).unwrap();
        assert!(object.completed_object_upgrades.contains(OBJECT_GRANT));
        assert!(
            object
                .completed_object_upgrades
                .contains(OBJECT_CREATE_GRANT)
        );
        assert!(
            !object
                .completed_object_upgrades
                .contains(OLD_OWNER_RESEARCH)
        );
        assert!(children(world, &object.template_name).is_empty());
    }

    assert!(receiving.transfer_object_to_player(receiving_id, 2));
    assert!(peer.transfer_object_to_player(peer_id, 2));
    // The peer's old owner has the stale former-player research, but its
    // receiving owner does not. Re-granting the Object upgrade invokes the
    // same Object::updateUpgradeModules path as NamedReceiveUpgrade; only the
    // receiving world's current Player may contribute its second mask bit.
    run_object_grant(&mut receiving, "ReceivingUpgradeMaster");
    run_object_grant(&mut peer, "PeerUpgradeMaster");
    assert_eq!(
        children(&receiving, "ReceivingUpgradeMaster"),
        vec!["ReceivingCreateGrantChild".into(), receiving_child]
    );
    assert!(children(&peer, "PeerUpgradeMaster").is_empty());

    receiving.process_player_create_team(1, 3, &[receiving_id]);
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&receiving).unwrap();
    assert_eq!(snapshot.garrison_original_team_instances.len(), 1);
    for tag in [b"A10F", b"BDRF", b"AIQF", b"GOTS", b"OXFR"] {
        assert!(
            snapshot
                .lifecycle_tail
                .windows(tag.len())
                .any(|part| part == tag),
            "captured v37 tail retains {}",
            String::from_utf8_lossy(tag)
        );
    }
    // Retain the historical NMLF tail and add the actual save-file HSQD
    // writer after the new OXFR frame, exercising the real footer composition.
    append_legacy_named_unit_history(&mut snapshot.lifecycle_tail);
    generals_main::save_load::snapshot::write_control_groups(
        &mut snapshot.lifecycle_tail,
        &generals_main::save_load::snapshot::ControlGroupSave {
            players: HashMap::from([(1, HashMap::from([(3, vec![receiving_id.0])]))]),
        },
    )
    .unwrap();
    assert_eq!(
        generals_main::save_load::snapshot::read_control_groups(&snapshot.lifecycle_tail)
            .unwrap()
            .unwrap()[&1][&3],
        [receiving_id.0]
    );
    let mut version_36_bincode: WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(&snapshot).unwrap()).unwrap();
    version_36_bincode.version = 36;
    let bytes = bincode_legacy::serialize(&version_36_bincode).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    assert_eq!(decoded.version, 36);
    assert!(decoded.lifecycle_tail.windows(4).any(|tag| tag == b"GOTS"));
    for tag in [b"A10F", b"BDRF", b"AIQF", b"OXFR", b"NMLF", b"HSQD"] {
        assert!(
            decoded
                .lifecycle_tail
                .windows(tag.len())
                .any(|part| part == tag),
            "bincode v36 tail retains {}",
            String::from_utf8_lossy(tag)
        );
    }
    let (mut restored, restored_id, restored_child) = world(root.path(), "Receiving");
    assert_eq!(restored_id, receiving_id);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let restored_building = restored
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "ReceivingGarrisonIdentityBuilding")
        .unwrap();
    assert_eq!(
        restored_building
            .building_data
            .as_ref()
            .unwrap()
            .original_team_instance_id,
        Some(snapshot.garrison_original_team_instances[0].team_instance_id)
    );
    assert_eq!(restored.player_hotkey_squad_ids(1, 3), [receiving_id]);
    assert!(
        restored
            .host_object(restored_id)
            .unwrap()
            .completed_object_upgrades
            .contains(OBJECT_GRANT)
    );

    // A v36 envelope already carries GOTS. Encoding it through direct Xfer
    // upgrades the envelope to v37 while retaining both exact Team identity
    // and Object-completion continuation ahead of APRO/CPAT/CPAD.
    let mut version_36: WorldSnapshot =
        bincode_legacy::deserialize(&bincode_legacy::serialize(&snapshot).unwrap()).unwrap();
    version_36.version = 36;
    let mut direct_bytes = std::io::Cursor::new(Vec::new());
    version_36
        .xfer(&mut XferSave::new(&mut direct_bytes))
        .unwrap();
    assert_eq!(version_36.version, 37);
    let mut direct_decoded = WorldSnapshot::default();
    direct_decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(
            direct_bytes.into_inner(),
        )))
        .unwrap();
    assert_eq!(direct_decoded.version, 37);
    assert!(
        direct_decoded
            .lifecycle_tail
            .windows(4)
            .any(|tag| tag == b"GOTS")
    );
    for tag in [b"A10F", b"BDRF", b"AIQF", b"OXFR", b"NMLF", b"HSQD"] {
        assert!(
            direct_decoded
                .lifecycle_tail
                .windows(tag.len())
                .any(|part| part == tag),
            "direct Xfer v37 tail retains {}",
            String::from_utf8_lossy(tag)
        );
    }
    assert!(
        direct_decoded
            .lifecycle_tail
            .windows(4)
            .any(|tag| tag == b"OXFR")
    );
    assert!(
        direct_decoded
            .lifecycle_tail
            .windows(4)
            .any(|tag| tag == b"GOTS")
    );
    let (mut direct_restored, _, _) = world(root.path(), "Receiving");
    builder
        .restore_from_snapshot(&direct_decoded, &mut direct_restored)
        .unwrap();
    let direct_restored_building = direct_restored
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "ReceivingGarrisonIdentityBuilding")
        .unwrap();
    assert_eq!(
        direct_restored_building
            .building_data
            .as_ref()
            .unwrap()
            .original_team_instance_id,
        Some(snapshot.garrison_original_team_instances[0].team_instance_id)
    );
    assert_eq!(
        direct_restored.player_hotkey_squad_ids(1, 3),
        [receiving_id]
    );
    assert!(
        direct_restored
            .host_object(restored_id)
            .unwrap()
            .completed_object_upgrades
            .contains(OBJECT_GRANT)
    );
    run_object_grant(&mut restored, "ReceivingUpgradeMaster");
    assert_eq!(
        children(&restored, "ReceivingUpgradeMaster"),
        vec!["ReceivingCreateGrantChild".into(), restored_child]
    );

    let ambiguous_legacy = make_ambiguous_v35_snapshot(&snapshot, receiving_id);
    let health_before = restored.host_object(restored_id).unwrap().health.current;
    let error = builder
        .restore_from_snapshot(&ambiguous_legacy, &mut restored)
        .expect_err("v35 mixed applied-upgrade state has no recoverable grant owner");
    assert!(
        error
            .to_string()
            .contains("ambiguous Object/player upgrade")
    );
    assert_eq!(
        restored.host_object(restored_id).unwrap().health.current,
        health_before
    );
}

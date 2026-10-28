//! The public Skirmish admission and script-driver path resolve tech queries
//! against the receiving world's prepared player identities.
use game_engine::common::{
    dict::Dict,
    ini::INILoadType,
    rts::player_template::PlayerTemplateStore,
    system::DataChunkOutput,
    well_known_keys::{key_player_faction, key_player_name},
};
use gamelogic::scripting::{
    core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    },
    engine::ScriptEngine,
};
use generals_main::{
    game_logic::{GameLogic, ObjectId, Team},
    skirmish_config::{
        SkirmishPlayerTemplateSelection, apply_skirmish_config, golden_skirmish_config,
    },
};

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();

    out.open_data_chunk("PolygonTriggers", 4);
    out.write_int(1);
    out.write_ascii_string("HomeBase");
    out.write_ascii_string("OwnerLayer");
    out.write_int(1901);
    out.write_byte(1);
    out.write_byte(0);
    out.write_int(0);
    out.write_int(4);
    for (x, y) in [(5, 5), (35, 5), (35, 35), (5, 35)] {
        out.write_int(x);
        out.write_int(y);
        out.write_int(0);
    }
    out.close_data_chunk();

    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    let mut authored = Dict::new();
    authored.set_ascii_string(key_player_name(), "MapFactionSource");
    authored.set_ascii_string(key_player_faction(), "FactionGLA");
    for side in [neutral, authored] {
        out.write_dict(&side);
        out.write_int(0);
    }
    out.write_int(0);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();

    let path = root.join("TechAdmission.map");
    std::fs::write(path, out.into_ckmp_bytes()).unwrap();
    root.join("TechAdmission.map")
}

fn condition() -> Condition {
    let mut condition = Condition::new(ConditionType::SkirmishTechBuildingWithinDistance);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "player0".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_real(ParameterType::Real, 20.0))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HomeBase".into(),
        ))
        .unwrap();
    condition
}

fn observing_engine() -> ScriptEngine {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition())));
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "ObservedTech".into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveAdmittedTech".into();
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("ObservedTech", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn admitted_world(root: &std::path::Path, nearby: bool) -> (GameLogic, ScriptEngine, ObjectId) {
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\n StartMoney = 10000\n StartingBuilding = AmericaCommandCenter\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut config = golden_skirmish_config("TechAdmission.map");
    config.slots.truncate(1);
    config.slots[0].player_template = SkirmishPlayerTemplateSelection::Exact {
        template_name: "FactionAmerica".into(),
        template_index: 0,
    };

    let mut world = GameLogic::new();
    world.admit_player_template_definitions(templates).unwrap();
    apply_skirmish_config(&mut world, &config).unwrap();
    world
        .admit_object_ini_sources([(
            "TechAdmission.ini".into(),
            "Object AdmissionTechBuilding\n KindOf = STRUCTURE TECH_BUILDING\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\n".into(),
        )])
        .unwrap();
    assert!(world.load_map(map_file(root).to_str().unwrap()));
    assert_eq!(
        world.get_player(0).unwrap().map_side.map_player_name,
        "player0"
    );
    // C++ excludes the querying player's own buildings. Keep the same
    // neutral object ID in both worlds, but place it near only one area.
    let position = if nearby {
        glam::Vec3::new(20.0, 0.0, 20.0)
    } else {
        glam::Vec3::new(300.0, 0.0, 300.0)
    };
    let tech = world
        .create_object("AdmissionTechBuilding", Team::Neutral, position)
        .unwrap();
    assert_ne!(world.host_object(tech).unwrap().owner_player_id, Some(0));
    (world, observing_engine(), tech)
}

#[test]
fn public_skirmish_preparation_and_driver_keep_tech_query_world_local() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let (mut owner, owner_engine, owner_tech) = admitted_world(owner_root.path(), true);
    let (mut peer, peer_engine, peer_tech) = admitted_world(peer_root.path(), false);
    assert_eq!(owner_tech, peer_tech);
    assert_eq!(
        owner.get_player(0).unwrap().id,
        peer.get_player(0).unwrap().id
    );

    peer.update_local_script_engine_with_owner(&peer_engine)
        .unwrap();
    owner
        .update_local_script_engine_with_owner(&owner_engine)
        .unwrap();
    assert_eq!(peer_engine.get_counter("ObservedTech").unwrap().value, 0);
    assert_eq!(owner_engine.get_counter("ObservedTech").unwrap().value, 1);
}

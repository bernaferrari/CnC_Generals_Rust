//! First ordinary-frame power observation after a public real-file restore.
//! Fresh processes share only authored inputs and save bytes. The deficit
//! witness is a scheduled PlayerHasPower script that latches a flag before the
//! late economy phase can repair stale restored player totals.
use game_engine::common::{
    dict::Dict,
    ini::{INILoadType, ini_locomotor::LocomotorStore},
    name_key_generator::NameKeyGenerator,
    rts::player_template::PlayerTemplateStore,
    system::DataChunkOutput,
};
use gamelogic::scripting::core::{ConditionType, Parameter, ParameterType, ScriptActionType};
use crate::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

const CHILD_TEST: &str = concat!(module_path!(), "::saved_power_process");
const OWNER: &str = "SavedGridOwner";
const OWNER_ID: u32 = 1;
const MAP: &str = "SavedPowerFixture.map";
const PRODUCER: &str = "SavedGridProducer";
const CONSUMER: &str = "SavedGridConsumer";
const POWER_FLAG: &str = "saved_grid_had_power";
const SEEN_FLAG: &str = "saved_grid_first_frame_seen";
const SCRIPTS: [&str; 2] = ["SavedGridPowerProbe", "SavedGridFrameProbe"];
const LOCOMOTOR_PATH: &str = "windows_game/extracted_big_files/INIZH/Data/INI/Locomotor.ini";
// Reviewed Strategy fixture admission: exact parser input at the first candidate,
// followed by the production bootstrap's fixed in-memory seed completion.
const LOCOMOTOR_INI: &str = "Locomotor BasicHumanLocomotor\nSurfaces = GROUND\nSpeed = 20\nAcceleration = 100\nTurnRate = 500\nAppearance = TWO_LEGS\nWanderWidthFactor = 1.0\nWanderLengthFactor = 1.0\nSpeedDamaged = 10\nAccelerationDamaged = 50\nEnd\nLocomotor TechnicalLocomotor\nSurfaces = GROUND\nSpeed = 90\nAcceleration = 100\nTurnRate = 180\nEnd\n";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Normal,
    DisabledProducer,
    DisabledConsumer,
    IncompleteProducer,
    DamagedProducer,
    Sabotaged,
    ForeignDeadProducer,
    ForeignIncompleteProducer,
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    produced: i32,
    consumed: i32,
    control: Control,
}

impl Case {
    fn expected_produced(self, restored: bool) -> i32 {
        // Object::xfer installs the Team before transferring m_disabledMask.
        // That load credits an authored producer even while its saved mask
        // disables it. Ordinary construction still gates the Team credit.
        if self.control == Control::IncompleteProducer
            || (self.control == Control::DisabledProducer && !restored)
        {
            0
        } else {
            self.produced
        }
    }

    fn expected_available(self, restored: bool) -> i32 {
        // Energy::getProduction returns zero during sabotage.
        let effective = if self.control == Control::Sabotaged {
            0
        } else {
            self.expected_produced(restored)
        };
        effective - self.consumed
    }

    fn foreign_world(self) -> bool {
        matches!(
            self.control,
            Control::ForeignDeadProducer | Control::ForeignIncompleteProducer
        )
    }
}

const CASES: [Case; 10] = [
    Case {
        name: "deficit",
        produced: 5,
        consumed: 8,
        control: Control::Normal,
    },
    Case {
        name: "surplus",
        produced: 8,
        consumed: 5,
        control: Control::Normal,
    },
    Case {
        name: "zero",
        produced: 0,
        consumed: 0,
        control: Control::Normal,
    },
    Case {
        name: "disabled_producer",
        produced: 8,
        consumed: 5,
        control: Control::DisabledProducer,
    },
    Case {
        name: "disabled_consumer",
        produced: 8,
        consumed: 5,
        control: Control::DisabledConsumer,
    },
    Case {
        name: "incomplete_producer",
        produced: 8,
        consumed: 5,
        control: Control::IncompleteProducer,
    },
    Case {
        name: "damaged_producer",
        produced: 8,
        consumed: 5,
        control: Control::DamagedProducer,
    },
    Case {
        name: "future_sabotage",
        produced: 8,
        consumed: 5,
        control: Control::Sabotaged,
    },
    Case {
        name: "foreign_dead_producer",
        produced: 8,
        consumed: 5,
        control: Control::ForeignDeadProducer,
    },
    Case {
        name: "foreign_incomplete_producer",
        produced: 8,
        consumed: 5,
        control: Control::ForeignIncompleteProducer,
    },
];

fn write_params(map: &mut DataChunkOutput, params: &[Parameter]) {
    map.write_int(params.len() as i32);
    for parameter in params {
        map.write_int(parameter.param_type as i32);
        map.write_int(parameter.int_value);
        map.write_real(parameter.real_value);
        map.write_ascii_string(&parameter.string_value);
    }
}

fn write_fixture_map(path: &Path) {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    map.open_data_chunk("SidesList", 2);
    map.write_int(1);
    let mut side = Dict::new();
    for (name, value) in [("playerName", OWNER), ("playerFaction", "FactionGLA")] {
        side.set_ascii_string(NameKeyGenerator::name_to_key(name), value);
    }
    side.set_unicode_string(NameKeyGenerator::name_to_key("playerDisplayName"), OWNER);
    side.set_bool(NameKeyGenerator::name_to_key("playerIsHuman"), true);
    map.write_dict(&side);
    map.write_int(0);
    map.write_int(0);
    map.open_data_chunk("PlayerScriptsList", 1);
    map.open_data_chunk("ScriptList", 1);
    for (index, script) in SCRIPTS.into_iter().enumerate() {
        map.open_data_chunk("Script", 2);
        for value in [script, "", "", ""] {
            map.write_ascii_string(value);
        }
        for value in [1, 1, 1, 1, 1, 0] {
            map.write_byte(value);
        }
        map.write_int(0);
        map.open_data_chunk("OrCondition", 1);
        map.open_data_chunk("Condition", 4);
        let (condition, key, params) = if index == 0 {
            (
                ConditionType::PlayerHasPower,
                "PLAYER_HAS_POWER",
                vec![Parameter::with_string(ParameterType::Side, OWNER.into())],
            )
        } else {
            (ConditionType::ConditionTrue, "CONDITION_TRUE", Vec::new())
        };
        map.write_int(condition as i32);
        map.write_name_key(NameKeyGenerator::name_to_key(key));
        write_params(&mut map, &params);
        map.close_data_chunk();
        map.close_data_chunk();
        map.open_data_chunk("ScriptAction", 2);
        map.write_int(ScriptActionType::SetFlag as i32);
        map.write_name_key(NameKeyGenerator::name_to_key("SET_FLAG"));
        let flag = if index == 0 { POWER_FLAG } else { SEEN_FLAG };
        write_params(
            &mut map,
            &[
                Parameter::with_string(ParameterType::Flag, flag.into()),
                Parameter::with_int(ParameterType::Boolean, 1),
            ],
        );
        map.close_data_chunk();
        map.close_data_chunk();
    }
    for _ in 0..3 {
        map.close_data_chunk();
    }
    std::fs::write(path, map.into_ckmp_bytes()).unwrap();
}

fn catalog_world(case: Case) -> GameLogic {
    assert_eq!(fs::read_to_string(LOCOMOTOR_PATH).unwrap(), LOCOMOTOR_INI);
    let mut locomotors = LocomotorStore::new_owned();
    assert_eq!(locomotors.apply_source(LOCOMOTOR_INI).unwrap(), 2);
    assert_eq!(locomotors.get_template_names().len(), 2);
    let mut logic = GameLogic::new();
    logic.admit_locomotor_definitions(locomotors).unwrap();
    admit_fixture_objects(&mut logic, case);
    let mut players = PlayerTemplateStore::new_owned();
    players
        .apply_source(
            "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\n\
             PlayerTemplate FactionGLA\n Side = GLA\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    logic.admit_player_template_definitions(players).unwrap();
    logic.start_new_game(GameMode::SinglePlayer).unwrap();
    assert!(logic.load_map(MAP));
    assert_eq!(logic.local_player_id(), Some(OWNER_ID));
    assert_eq!(logic.host_ai_player_count(), 0);
    catalog(&logic, case);
    logic
}

fn admit_fixture_objects(logic: &mut GameLogic, case: Case) {
    let mut object_ini = String::new();
    for (name, energy) in [(PRODUCER, case.produced), (CONSUMER, -case.consumed)] {
        object_ini.push_str(&format!(
            "Object {name}\nKindOf = STRUCTURE SELECTABLE IMMOBILE\nEnergyProduction = {energy}\n\
             Body = ActiveBody Body\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nEnd\n"
        ));
    }
    logic
        .admit_object_ini_sources([("SavedPowerObjects.ini".into(), object_ini)])
        .unwrap();
}

fn catalog(logic: &GameLogic, case: Case) -> Value {
    let content = logic.receiving_module_content.as_ref().unwrap();
    let names: Vec<_> = content.admitted_templates().map(|(name, _)| name).collect();
    assert_eq!(names, [CONSUMER, PRODUCER]);
    let templates: Vec<_> = names
        .into_iter()
        .map(|name| {
            let template = &logic.templates[name];
            assert!(template.primary_weapon_name.is_none());
            assert!(
                template.secondary_weapon_name.is_none()
                    && template.tertiary_weapon_name.is_none()
            );
            assert!(template.locomotor_name.is_none() && template.locomotor_set_names.is_empty());
            assert!(
                template.authored_locomotor_sets.as_ref().is_some_and(|sets| sets.is_empty())
                    && template.armor_sets.is_empty()
            );
            assert_eq!(
                template.energy_production,
                Some(if name == PRODUCER { case.produced } else { -case.consumed })
            );
            let mut value = serde_json::to_value(template).unwrap();
            value["kind_of"]
                .as_array_mut()
                .unwrap()
                .sort_by_key(Value::to_string);
            value
        })
        .collect();
    let locomotors = {
        let store = logic.locomotor_catalog();
        let mut names: Vec<_> = store
            .get_template_names()
            .into_iter()
            .map(|n| n.as_str())
            .collect();
        names.sort_unstable();
        names.into_iter().map(|name| json!({"name":name,"definition":format!("{:?}",store.find_template(name).unwrap())})).collect::<Vec<_>>()
    };
    assert_eq!(fs::read_to_string(LOCOMOTOR_PATH).unwrap(), LOCOMOTOR_INI);
    json!({"templates":templates,
        "locomotor_input":{"relative_path":LOCOMOTOR_PATH,"ini":LOCOMOTOR_INI,"parser_count":2},
        "locomotors":locomotors})
}

fn admit_source(logic: &mut GameLogic, case: Case) {
    assert!(logic.scripts_loaded);
    logic.scripts_loaded = false;
    for (index, name) in [PRODUCER, CONSUMER].into_iter().enumerate() {
        let id = logic
            .create_object_for_player(name, OWNER_ID, Vec3::new(index as f32 * 100.0, 0.0, 0.0))
            .unwrap();
        assert_eq!(id, ObjectId(index as u32 + 1));
    }
    // Preservation controls author source object state before the ordinary
    // settling frame; they never repair loaded state or set derived totals.
    match case.control {
        Control::DisabledProducer => {
            logic
                .host_object_mut(ObjectId(1))
                .unwrap()
                .status
                .disabled_held = true
        }
        Control::DisabledConsumer => {
            logic
                .host_object_mut(ObjectId(2))
                .unwrap()
                .status
                .disabled_held = true
        }
        Control::IncompleteProducer => {
            let object = logic.host_object_mut(ObjectId(1)).unwrap();
            object.status.under_construction = true;
            object.construction_percent = 0.5;
        }
        Control::DamagedProducer => {
            logic.host_object_mut(ObjectId(1)).unwrap().health.current = 500.0
        }
        Control::Sabotaged => logic.get_player_mut(OWNER_ID).unwrap().power_sabotaged_till_frame = 100,
        _ => {}
    }
    // An ordinary source frame establishes derived totals, without scripts.
    logic.update();
    logic.scripts_loaded = true;
    assert_eq!(logic.get_frame(), 1);
    let player = logic.get_player(OWNER_ID).unwrap();
    assert_eq!(
        (
            player.power_produced,
            player.power_consumed,
            player.power_available
        ),
        (
            case.expected_produced(false),
            case.consumed,
            case.expected_available(false)
        )
    );
    assert_eq!(logic.host_ai_player_count(), 0);
}

// The receiving save is loaded while a peer with conflicting state and the
// same numeric Object IDs is alive.
fn foreign_world(logic: &GameLogic, case: Case) -> GameLogic {
    let mut foreign = GameLogic::new();
    foreign
        .admit_locomotor_definitions(logic.locomotor_catalog().clone())
        .unwrap();
    admit_fixture_objects(&mut foreign, case);
    foreign.add_player(Player::new(OWNER_ID, Team::GLA, "ForeignGridOwner", true));
    for (index, name) in [PRODUCER, CONSUMER].into_iter().enumerate() {
        assert_eq!(
            foreign
                .create_object_for_player(name, OWNER_ID, Vec3::new(index as f32 * 100.0, 0.0, 0.0))
                .unwrap(),
            ObjectId(index as u32 + 1)
        );
    }
    let producer = foreign.host_object_mut(ObjectId(1)).unwrap();
    match case.control {
        Control::ForeignDeadProducer => producer.health.current = 0.0,
        Control::ForeignIncompleteProducer => {
            producer.status.under_construction = true;
            producer.construction_percent = 0.25;
        }
        _ => unreachable!(),
    }
    foreign
}

fn foreign_state(foreign: &GameLogic) -> Value {
    let objects: Vec<_> = [ObjectId(1), ObjectId(2)].into_iter().map(|id| {
        let object = foreign.host_object(id).unwrap();
        json!({
            "id": id.0,
            "health": object.health.current,
            "under_construction": object.status.under_construction,
            "construction_percent": object.construction_percent,
        })
    }).collect();
    json!({"objects":objects,"frame":foreign.get_frame()})
}

fn load_with_foreign_world(
    manager: &mut SaveFileManager,
    logic: &mut GameLogic,
    case: Case,
    dir: &Path,
) {
    let foreign = foreign_world(logic, case);
    let before = foreign_state(&foreign);
    let producer = foreign.host_object(ObjectId(1)).unwrap();
    match case.control {
        Control::ForeignDeadProducer => assert_eq!(producer.health.current, 0.0),
        Control::ForeignIncompleteProducer => {
            assert!(producer.status.under_construction);
            assert_eq!(producer.construction_percent, 0.25);
        }
        _ => unreachable!(),
    }
    let (result, receiving) = {
        let result = manager.load_game("power", logic).expect("restore with a same-ID peer");
        let producer = logic.host_object(ObjectId(1)).unwrap();
        write_json(
            &dir.join("reader-foreign-boundary.json"),
            &json!({
            "result":format!("{result:?}"),"foreign_before":before,
            "generic_alive":producer.is_alive(),"generic_constructed":producer.is_constructed(),
            "local_health":producer.health.current,"local_destroyed":producer.status.destroyed,
            "local_under_construction":producer.status.under_construction,
            "local_construction_percent":producer.construction_percent}),
        );
        // Ordinary predicates use only the receiving saved object. The positive
        // control above proves the same-ID peer really disagrees.
        assert!(
            producer.is_alive(),
            "receiving body remains alive despite a same-ID peer"
        );
        assert!(
            producer.is_constructed(),
            "receiving construction state cannot be replaced by a peer"
        );
        assert_eq!(producer.health.current, 1000.0);
        assert!(!producer.status.under_construction);
        assert_eq!(producer.construction_percent, 1.0);
        let player = logic.get_player(OWNER_ID).unwrap();
        (
            result,
            json!({"generic_alive":producer.is_alive(),"generic_constructed":producer.is_constructed(),
            "local_health":producer.health.current,"local_under_construction":producer.status.under_construction,
            "local_construction_percent":producer.construction_percent,
            "produced":player.power_produced,"consumed":player.power_consumed,"available":player.power_available}),
        )
    };
    let after = foreign_state(&foreign);
    write_json(
        &dir.join("reader-foreign-world.json"),
        &json!({"before":before,"after":after,"receiving":receiving,"result":format!("{result:?}")}),
    );
    assert_eq!(
        before, after,
        "restore must not modify the peer world's eligibility state"
    );
}

fn observe_authored_scripts(logic: &GameLogic) -> Value {
    assert!(logic.scripts_loaded);
    let engine = logic.owned_script_engine.as_ref().unwrap();
    for flag in [POWER_FLAG, SEEN_FLAG] {
        engine.set_flag(flag, false).unwrap();
    }
    let scripts = SCRIPTS.map(|name| {
        let script = engine.find_script_clone_by_name(name).unwrap();
        assert!(script.is_active && script.is_one_shot);
        script
    });
    serde_json::to_value(scripts).unwrap()
}

fn script_state(logic: &GameLogic) -> Value {
    let engine = logic.owned_script_engine.as_ref().unwrap();
    let scripts: Vec<_> = SCRIPTS.into_iter().map(|name| {
        let script = engine.find_script_clone_by_name(name).unwrap();
        json!({"name":name,"active":script.is_active,"one_shot":script.is_one_shot,
            "due_frame":script.frame_to_evaluate_at,"delay_seconds":script.delay_evaluation_seconds})
    }).collect();
    let (counters, flags) = engine.snapshot_named_trackers();
    json!({"power_flag":engine.get_flag(POWER_FLAG).unwrap().value,
        "first_frame_seen":engine.get_flag(SEEN_FLAG).unwrap().value,
        "scripts":scripts,"flags":flags,"counters":counters})
}

fn observe(logic: &GameLogic) -> Value {
    let player = logic.get_player(OWNER_ID).unwrap();
    let mut objects: Vec<_> = logic.host_objects().registered_values().collect();
    objects.sort_by_key(|object| object.id.0);
    let objects: Vec<_> = objects.into_iter().map(|object| json!({
        "id":object.id.0,"template":object.get_template().name,"owner":object.owner_player_id,
        "faction":format!("{:?}",object.team),"alive":object.is_alive(),
        "constructed":object.is_constructed(),"disabled":object.is_disabled(),
        "health":object.health.current,"under_construction":object.status.under_construction,
        "disabled_held":object.status.disabled_held,
        "construction_percent":object.construction_percent,"provided":object.power_provided,
        "consumed":object.power_consumed,"target":object.target.map(|id|id.0),
        "discharge":object.weapon_discharge_marker().sequence,"fire_intents":object.fire_intent_count,
        "weapon_slots":(0..3).map(|slot|object.weapon_slot(slot).is_some()).collect::<Vec<_>>()
    })).collect();
    json!({"frame":logic.get_frame(),"player":{"id":player.id,"name":player.name,
        "faction":format!("{:?}",player.team),"local":logic.local_player_id() == Some(player.id),"alive":player.is_alive,
        "produced":player.power_produced,"consumed":player.power_consumed,
        "available":player.power_available,"resources_power":player.resources.power,
        "sabotaged_till_frame":player.power_sabotaged_till_frame},
        "objects":objects,"ai_count":logic.host_ai_player_count(),"scripts_loaded":logic.scripts_loaded,
        "script_state":script_state(logic)})
}

// Collect errors and continue both frames so a stale scalar cannot hide the
// actual scheduled script observation, or its persistence after convergence.
fn violations(logic: &GameLogic, case: Case, updates: u32, restored: bool) -> Vec<String> {
    let value = observe(logic);
    let mut errors = Vec::new();
    let mut require = |ok: bool, message: &str| {
        if !ok {
            errors.push(message.to_string());
        }
    };
    require(
        logic.get_frame() == updates + 1,
        "ordinary frame progression",
    );
    require(
        logic.get_players().len() == 2 && logic.host_objects().registered_len() == 2,
        "exact player/object population",
    );
    require(
        value["player"]["name"] == OWNER
            && value["player"]["alive"] == true
            && value["player"]["local"] == true,
        "live explicit owner identity",
    );
    require(value["ai_count"] == 0, "empty AI roster");
    require(
        value["player"]["produced"] == case.expected_produced(restored),
        "derived production",
    );
    require(
        value["player"]["consumed"] == case.consumed,
        "derived consumption",
    );
    require(
        value["player"]["available"] == case.expected_available(restored),
        "derived available power",
    );
    require(
        value["player"]["sabotaged_till_frame"]
            == if case.control == Control::Sabotaged {
                100
            } else {
                0
            },
        "unchanged sabotage deadline",
    );
    for (index, name) in [PRODUCER, CONSUMER].into_iter().enumerate() {
        let object = logic.host_object(ObjectId(index as u32 + 1)).unwrap();
        require(
            object.owner_player_id == Some(OWNER_ID)
                && object.team == Team::GLA
                && object.get_template().name == name,
            "exact object identity/ownership",
        );
        let incomplete = index == 0 && case.control == Control::IncompleteProducer;
        let held = (index == 0 && case.control == Control::DisabledProducer)
            || (index == 1 && case.control == Control::DisabledConsumer);
        // The ordinary settling frame can run authored structure regeneration.
        // Keep exact HP in observations; eligibility needs alive and damaged.
        let health_ok = if index == 0 && case.control == Control::DamagedProducer {
            object.health.current > 0.0 && object.health.current < object.health.maximum
        } else {
            object.health.current == 1000.0
        };
        require(
            object.is_alive()
                && object.is_constructed() == !incomplete
                && object.is_disabled() == (incomplete || held)
                && health_ok,
            "preserved object health/completion/disabled eligibility",
        );
        require(
            object.status.disabled_held == held && object.status.under_construction == incomplete,
            "preserved detailed disabled/construction status",
        );
        require(
            object.construction_percent == if incomplete { 0.5 } else { 1.0 },
            "preserved construction progress",
        );
        require(
            object.power_provided == if index == 0 { case.produced } else { 0 },
            "authored object production",
        );
        require(
            object.power_consumed == if index == 0 { 0 } else { case.consumed },
            "authored object consumption",
        );
        require(
            object.target.is_none()
                && object.weapon_discharge_marker().sequence == 0
                && object.fire_intent_count == 0
                && (0..3).all(|slot| object.weapon_slot(slot).is_none()),
            "weaponless inactivity",
        );
    }
    let scripts = &value["script_state"];
    let has_power = case.control != Control::Sabotaged && case.expected_produced(restored) >= case.consumed;
    require(
        scripts["first_frame_seen"] == (updates > 0),
        "unconditional first-frame script observer",
    );
    require(
        scripts["power_flag"] == (updates > 0 && has_power),
        "latched first-frame PlayerHasPower outcome",
    );
    require(
        scripts["scripts"][0]["active"] == !(updates > 0 && has_power),
        "power one-shot active state",
    );
    require(
        scripts["scripts"][1]["active"] == (updates == 0),
        "unconditional one-shot active state",
    );
    errors
}

fn info() -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: "power".into(),
        display_name: "Saved power first frame".into(),
        description: String::new(),
        map_name: MAP.into(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
#[ignore = "child entry point; run through saved_power_survives_first_frame_scripts"]
fn saved_power_process() {
    let role = std::env::var("SAVED_POWER_ROLE").unwrap();
    assert!(role == "writer" || role == "reader");
    let name = std::env::var("SAVED_POWER_CASE").unwrap();
    let case = *CASES.iter().find(|case| case.name == name).unwrap();
    let dir = std::env::current_dir().unwrap();
    let mut journal = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{role}-observations.jsonl")))
        .unwrap();
    let mut logic = catalog_world(case);
    if role == "writer" {
        admit_source(&mut logic, case);
    } else {
        assert!(logic.get_players().len() == 2 && logic.host_objects().registered_is_empty());
    }
    let authored_scripts = observe_authored_scripts(&logic);
    let inputs = json!({"catalog":catalog(&logic, case),"scripts":authored_scripts,
        "initial_script_state":script_state(&logic),"owner":{"id":OWNER_ID,"name":OWNER,"faction":"GLA"},
        "control":format!("{:?}",case.control),"source_warmup_frames":1,"post_load_mutations":"ordinary GameLogic::update only"});
    write_json(&dir.join(format!("{role}-inputs.json")), &inputs);
    let mut manager = SaveFileManager::with_save_directory(dir.join("saves"));
    manager.init().unwrap();
    if role == "writer" {
        let admission_errors = violations(&logic, case, 0, false);
        write_json(
            &dir.join("writer-admission.json"),
            &json!({"observation":observe(&logic),"violations":admission_errors}),
        );
        assert!(
            admission_errors.is_empty(),
            "source admission failed: {admission_errors:?}"
        );
        let before = observe(&logic);
        manager.save_game("power", &logic, &info()).unwrap();
        assert_eq!(
            observe(&logic),
            before,
            "save must not execute scripts or mutate writer"
        );
    } else {
        // Decode evidence only; load_game below repeats the public decode and
        // clears/re-stages its compatibility companions normally.
        let crate::save_load::DecodedGameSave {
            snapshot: decoded, ..
        } = manager.load_game_snapshot("power").unwrap();
        let producer = &decoded.objects[&ObjectId(1)];
        write_json(
            &dir.join("reader-decoded.json"),
            &json!({"world_version":decoded.version,
            "producer_health":producer.health,"producer_status":producer.status,
            "producer_type":producer.object_type}),
        );
        let economy_before =
            crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, OWNER_ID)
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        crate::presentation_frame::PresentationEvent::EconomyChanged { .. }
                    )
                })
                .count();
        if case.foreign_world() {
            load_with_foreign_world(&mut manager, &mut logic, case, &dir);
        } else {
            manager.load_game("power", &mut logic).unwrap();
        }
        let economy_after =
            crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, OWNER_ID)
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        crate::presentation_frame::PresentationEvent::EconomyChanged { .. }
                    )
                })
                .count();
        write_json(
            &dir.join("reader-load.json"),
            &json!({"result":"Ok","economy_events_before":economy_before,"economy_events_after":economy_after}),
        );
        assert_eq!(
            economy_after, 0,
            "restore must not publish ordinary economy events"
        );
    }

    let mut observations = Vec::new();
    let mut errors = Vec::new();
    for updates in 0..=2 {
        if updates > 0 {
            logic.update();
        }
        let observation = observe(&logic);
        let current_errors = violations(&logic, case, updates, role == "reader");
        let entry =
            json!({"updates":updates,"observation":observation,"violations":current_errors});
        serde_json::to_writer(&mut journal, &entry).unwrap();
        journal.write_all(b"\n").unwrap();
        journal.flush().unwrap();
        observations.push(observation);
        errors.extend(
            current_errors
                .into_iter()
                .map(|error| format!("updates={updates}: {error}")),
        );
        assert_eq!(catalog(&logic, case), inputs["catalog"]);
    }
    let saved = fs::read(manager.get_save_path("power")).unwrap();
    write_json(
        &dir.join(format!("{role}.json")),
        &json!({"case":case.name,"role":role,
        "pid":std::process::id(),"cwd":dir,"executable":std::env::current_exe().unwrap(),
        "inputs":inputs,"observations":observations,"violations":errors,
        "save_bytes":saved.len(),"save_crc32":crc32fast::hash(&saved),
        "source_crc32":crc32fast::hash(include_bytes!("saved_power_first_frame.rs"))}),
    );
    assert!(
        errors.is_empty(),
        "strict saved power contract failed: {errors:?}"
    );
}

fn child(dir: &Path, case: Case, role: &str) -> bool {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            CHILD_TEST.split_once("::").unwrap().1,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("SAVED_POWER_ROLE", role)
        .env("SAVED_POWER_CASE", case.name)
        .current_dir(dir)
        .output()
        .expect("start fresh saved power process");
    fs::write(dir.join(format!("{role}.stdout.txt")), &output.stdout).unwrap();
    fs::write(dir.join(format!("{role}.stderr.txt")), &output.stderr).unwrap();
    write_json(
        &dir.join(format!("{role}-status.json")),
        &json!({"success":output.status.success(),"exit_code":output.status.code()}),
    );
    output.status.success() && dir.join(format!("{role}.json")).is_file()
}

fn expected_loaded_observations(case: Case, writer: &Value) -> Value {
    let mut expected = writer.clone();
    if case.control != Control::DisabledProducer {
        return expected;
    }
    for (index, observation) in expected.as_array_mut().unwrap().iter_mut().enumerate() {
        observation["player"]["produced"] = json!(case.expected_produced(true));
        observation["player"]["available"] = json!(case.expected_available(true));
        if index > 0 {
            let scripts = &mut observation["script_state"];
            scripts["power_flag"] = json!(true);
            scripts["scripts"][0]["active"] = json!(false);
            let power_flag = scripts["flags"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|row| row[0] == POWER_FLAG)
                .unwrap();
            power_flag[1] = json!(true);
        }
    }
    expected
}

#[test]
fn saved_power_survives_first_frame_scripts() {
    assert!(std::env::var_os("SAVED_POWER_ROLE").is_none());
    let root = std::env::var_os("SAVED_POWER_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| tempfile::TempDir::new().unwrap().keep());
    fs::create_dir_all(&root).unwrap();
    println!("SAVED_POWER_EVIDENCE {}", root.display());
    let mut failures = Vec::new();
    for case in CASES {
        let dir = root.join(case.name);
        fs::create_dir(&dir).expect("never overwrite prior evidence");
        let locomotor = dir.join(LOCOMOTOR_PATH);
        fs::create_dir_all(locomotor.parent().unwrap()).unwrap();
        fs::write(locomotor, LOCOMOTOR_INI).unwrap();
        write_fixture_map(&dir.join(MAP));
        let writer_ok = child(&dir, case, "writer");
        let reader_ok = writer_ok && child(&dir, case, "reader");
        println!(
            "SAVED_POWER_CASE {} writer={writer_ok} reader={reader_ok}",
            case.name
        );
        if !writer_ok || !reader_ok {
            failures.push(format!(
                "{} writer={writer_ok} reader={reader_ok}",
                case.name
            ));
        }
        // Failed child contracts still leave complete journals for comparison.
        if !dir.join("writer.json").is_file() || !dir.join("reader.json").is_file() {
            continue;
        }
        let read = |role| -> Value {
            serde_json::from_slice(&fs::read(dir.join(format!("{role}.json"))).unwrap()).unwrap()
        };
        let writer = read("writer");
        let reader = read("reader");
        assert_ne!(writer["pid"], reader["pid"]);
        let expected = expected_loaded_observations(case, &writer["observations"]);
        let mut differences = Vec::new();
        if expected != reader["observations"] {
            differences.push("observations");
            failures.push(format!("{} loaded observations differ", case.name));
        }
        for field in [
            "inputs",
            "save_bytes",
            "save_crc32",
            "source_crc32",
        ] {
            if writer[field] != reader[field] {
                differences.push(field);
                failures.push(format!("{} exact {field} differs", case.name));
            }
        }
        write_json(
            &dir.join("comparison.json"),
            &json!({"differences":differences,
            "checkpoint_equal":expected[0] == reader["observations"][0],
            "first_frame_equal":expected[1] == reader["observations"][1],
            "second_frame_equal":expected[2] == reader["observations"][2]}),
        );
    }
    write_json(
        &root.join("summary.json"),
        &json!({"cases":CASES.len(),"failures":failures,
        "scope":"real-file derived power and scheduled first-frame continuation; eligibility preservation and conflicting same-ID peer controls; no C++ runtime or whole-world isolation claim"}),
    );
    assert!(failures.is_empty(), "{failures:?}");
}

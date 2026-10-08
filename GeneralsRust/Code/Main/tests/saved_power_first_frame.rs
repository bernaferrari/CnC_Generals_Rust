//! First ordinary-frame power observation after a public real-file restore.
//! Fresh processes share only authored inputs and save bytes. The deficit
//! witness is a scheduled PlayerHasPower script that latches a flag before the
//! late economy phase can repair stale restored player totals.
use game_client::effects::particle_manager::{
    ParticleSystemManager, get_particle_system_manager, get_particle_system_manager_mut,
};
use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use gamelogic::object::armor::{TheArmorStore, load_armor_templates_from_str};
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine, with_script_engine_ref};
use gamelogic::weapon::{with_weapon_store, with_weapon_store_mut};
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

const CHILD_TEST: &str = "saved_power_process";
const OWNER: &str = "SavedGridOwner";
const PRODUCER: &str = "SavedGridProducer";
const CONSUMER: &str = "SavedGridConsumer";
const POWER_FLAG: &str = "saved_grid_had_power";
const SEEN_FLAG: &str = "saved_grid_first_frame_seen";
const SCRIPTS: [&str; 2] = ["SavedGridPowerProbe", "SavedGridFrameProbe"];
const ARMOR_INI: &str = "Armor PowerSaveUnusedArmor\nArmor = DEFAULT 100%\nEnd\n";
const LOCOMOTOR_PATH: &str = "windows_game/extracted_big_files/INIZH/Data/INI/Locomotor.ini";
// Reviewed Strategy fixture admission: exact parser input at the first candidate,
// followed by the production bootstrap's fixed in-memory seed completion.
const LOCOMOTOR_INI: &str = "Locomotor BasicHumanLocomotor\nSurfaces = GROUND\nSpeed = 20\nAcceleration = 100\nTurnRate = 500\nAppearance = TWO_LEGS\nWanderWidthFactor = 1.0\nWanderLengthFactor = 1.0\nSpeedDamaged = 10\nAccelerationDamaged = 50\nEnd\nLocomotor TechnicalLocomotor\nSurfaces = GROUND\nSpeed = 90\nAcceleration = 100\nTurnRate = 180\nEnd\n";
const GLA_SUPPORT: [&str; 12] = [
    "GLA_CommandCenter",
    "GLACommandCenter",
    "GLA_SupplyStash",
    "GLASupplyStash",
    "GLA_ArmsDealer",
    "GLAArmsDealer",
    "GLA_Barracks",
    "GLABarracks",
    "GLA_Soldier",
    "GLAInfantryRebel",
    "GLA_Technical",
    "GLAInfantryWorker",
];

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
    fn expected_produced(self) -> i32 {
        if matches!(
            self.control,
            Control::DisabledProducer | Control::IncompleteProducer | Control::Sabotaged
        ) {
            0
        } else {
            self.produced
        }
    }
    fn foreign_shadow(self) -> bool {
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

fn catalog_world(case: Case) -> GameLogic {
    assert!(generals_main::assets::get_asset_manager().is_none());
    gamelogic::initialize_weapon_store().unwrap();
    with_weapon_store_mut(|store| {
        assert_eq!(store.get_template_count(), 0);
        store.mark_host_bootstrap_complete();
    })
    .unwrap();
    assert_eq!(load_armor_templates_from_str(ARMOR_INI, None).unwrap(), 1);
    assert_eq!(fs::read_to_string(LOCOMOTOR_PATH).unwrap(), LOCOMOTOR_INI);
    assert!(get_locomotor_store().get_template_names().is_empty());
    assert_eq!(load_locomotors_from_str(LOCOMOTOR_INI).unwrap(), 2);
    assert_eq!(get_locomotor_store().get_template_names().len(), 2);
    {
        let mut guard = get_particle_system_manager_mut().unwrap();
        assert!(guard.is_none());
        let manager = guard.get_or_insert_with(ParticleSystemManager::new);
        for name in ["MuzzleFlash", "BulletImpact"] {
            assert!(manager.ensure_preset_template(name).is_some());
        }
        assert_eq!(manager.active_system_count(), 0);
    }
    let mut logic = GameLogic::new();
    assert!(logic.templates.is_empty());
    logic.ensure_ai_faction_templates(Team::GLA);
    assert_eq!(logic.host_ai_player_count(), 0);
    for (name, energy) in [(PRODUCER, case.produced), (CONSUMER, -case.consumed)] {
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Immobile)
            .set_health(1000.0)
            .set_primary_weapon_none();
        template.energy_production = Some(energy);
        logic.templates.insert(name.into(), template);
    }
    catalog(&logic, case);
    logic
}

fn catalog(logic: &GameLogic, case: Case) -> Value {
    assert!(generals_main::assets::get_asset_manager().is_none());
    assert!(
        game_engine::common::thing::thing_factory::try_get_thing_factory()
            .unwrap()
            .is_none()
    );
    with_weapon_store(|store| {
        assert!(store.host_bootstrap_is_complete());
        assert_eq!(store.get_template_count(), 0);
    })
    .unwrap();
    assert_eq!(TheArmorStore::read().len(), 1);
    let mut expected: Vec<_> = GLA_SUPPORT
        .into_iter()
        .chain([PRODUCER, CONSUMER])
        .collect();
    expected.sort_unstable();
    let mut names: Vec<_> = logic.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, expected);
    let templates: Vec<_> = names
        .into_iter()
        .map(|name| {
            let template = &logic.templates[name];
            if [PRODUCER, CONSUMER].contains(&name) {
                assert!(template.primary_weapon_name.is_none());
                assert!(
                    template.secondary_weapon_name.is_none()
                        && template.tertiary_weapon_name.is_none()
                );
                assert!(
                    template.locomotor_name.is_none() && template.locomotor_set_names.is_empty()
                );
                assert!(
                    template.authored_locomotor_sets.is_none() && template.armor_sets.is_empty()
                );
                assert_eq!(
                    template.energy_production,
                    Some(if name == PRODUCER {
                        case.produced
                    } else {
                        -case.consumed
                    })
                );
            }
            let mut value = serde_json::to_value(template).unwrap();
            value["kind_of"]
                .as_array_mut()
                .unwrap()
                .sort_by_key(Value::to_string);
            value
        })
        .collect();
    let locomotors = {
        let store = get_locomotor_store();
        let mut names: Vec<_> = store
            .get_template_names()
            .into_iter()
            .map(|n| n.as_str())
            .collect();
        names.sort_unstable();
        names.into_iter().map(|name| json!({"name":name,"definition":format!("{:?}",store.find_template(name).unwrap())})).collect::<Vec<_>>()
    };
    let particles = {
        let guard = get_particle_system_manager().unwrap();
        let manager = guard.as_ref().unwrap();
        ["MuzzleFlash", "BulletImpact"].into_iter().map(|name| {
            json!({"name":name,"definition":format!("{:?}",manager.find_template(name).unwrap())})
        }).collect::<Vec<_>>()
    };
    assert_eq!(fs::read_to_string(LOCOMOTOR_PATH).unwrap(), LOCOMOTOR_INI);
    json!({"templates":templates,"weapon_catalog":[],"armor_ini":ARMOR_INI,
        "locomotor_input":{"relative_path":LOCOMOTOR_PATH,"ini":LOCOMOTOR_INI,"parser_count":2},
        "locomotors":locomotors,"particles":particles,"asset_manager":false})
}

fn admit_source(logic: &mut GameLogic, case: Case) {
    assert!(!logic.scripts_loaded);
    logic.add_player(Player::new(0, Team::GLA, OWNER, true));
    for (index, name) in [PRODUCER, CONSUMER].into_iter().enumerate() {
        let id = logic
            .create_object_for_player(name, 0, Vec3::new(index as f32 * 100.0, 0.0, 0.0))
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
        Control::Sabotaged => logic.get_player_mut(0).unwrap().power_sabotaged_till_frame = 100,
        _ => {}
    }
    // An ordinary source frame establishes derived totals, without scripts.
    logic.update();
    assert_eq!(logic.get_frame(), 1);
    let player = logic.get_player(0).unwrap();
    assert_eq!(
        (
            player.power_produced,
            player.power_consumed,
            player.power_available
        ),
        (
            case.expected_produced(),
            case.consumed,
            case.expected_produced() - case.consumed
        )
    );
    assert_eq!(logic.host_ai_player_count(), 0);
}

// Existing safe APIs publish a distinct world with the same numeric ObjectIds
// only around load. Its health/construction deliberately contradicts the save.
fn foreign_shadow(
    logic: &GameLogic,
    case: Case,
) -> generals_main::gameworld_shadow::GameWorldShadow {
    let mut foreign = GameLogic::new();
    foreign.templates = logic.templates.clone();
    foreign.add_player(Player::new(0, Team::GLA, "ForeignGridOwner", true));
    for (index, name) in [PRODUCER, CONSUMER].into_iter().enumerate() {
        assert_eq!(
            foreign
                .create_object_for_player(name, 0, Vec3::new(index as f32 * 100.0, 0.0, 0.0))
                .unwrap(),
            ObjectId(index as u32 + 1)
        );
    }
    let mut shadow = generals_main::gameworld_shadow::GameWorldShadow::new(16);
    shadow.sync_from_host(&foreign);
    let entity = shadow.entity_for_host(ObjectId(1)).unwrap();
    let producer = shadow.world_mut().world_mut().entity_mut(entity).unwrap();
    match case.control {
        Control::ForeignDeadProducer => producer.health = 0.0,
        Control::ForeignIncompleteProducer => {
            producer.under_construction = true;
            producer.construction_percent = 0.25;
        }
        _ => unreachable!(),
    }
    shadow
}

fn shadow_state(shadow: &generals_main::gameworld_shadow::GameWorldShadow) -> Value {
    let objects: Vec<_> = [ObjectId(1), ObjectId(2)].into_iter().map(|id| {
        let entity = shadow.entity_for_host(id).unwrap();
        let object = shadow.world().entity(entity).unwrap();
        json!({"host_id":id.0,"entity":entity.get(),"health":object.health,
            "under_construction":object.under_construction,"construction_percent":object.construction_percent})
    }).collect();
    json!({"objects":objects,"mapped_count":shadow.mapped_count()})
}

fn load_with_foreign_shadow(
    manager: &mut SaveFileManager,
    logic: &mut GameLogic,
    case: Case,
    dir: &Path,
) {
    use generals_main::gameworld_shadow::{CoupledTickGuard, with_coupled_shadow};
    let mut shadow = foreign_shadow(logic, case);
    let before = shadow_state(&shadow);
    let guard = CoupledTickGuard::enter();
    let (result, receiving) = with_coupled_shadow(&mut shadow, || {
        let result = manager.load_game("power", logic);
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
        // These generic predicates must actually see the conflicting foreign
        // object. Merely creating a shadow without reaching it is not evidence.
        assert_eq!(
            producer.is_alive(),
            case.control != Control::ForeignDeadProducer
        );
        assert_eq!(
            producer.is_constructed(),
            case.control != Control::ForeignIncompleteProducer
        );
        assert_eq!(producer.health.current, 1000.0);
        assert!(!producer.status.under_construction);
        assert_eq!(producer.construction_percent, 1.0);
        let player = logic.get_player(0).unwrap();
        (
            result,
            json!({"generic_alive":producer.is_alive(),"generic_constructed":producer.is_constructed(),
            "local_health":producer.health.current,"local_under_construction":producer.status.under_construction,
            "local_construction_percent":producer.construction_percent,
            "produced":player.power_produced,"consumed":player.power_consumed,"available":player.power_available}),
        )
    });
    drop(guard);
    let after = shadow_state(&shadow);
    write_json(
        &dir.join("reader-foreign-shadow.json"),
        &json!({"before":before,"after":after,"receiving":receiving,"result":format!("{result:?}")}),
    );
    assert_eq!(
        before, after,
        "restore must not modify the foreign shadow's eligibility state"
    );
    result.unwrap();
}

fn admit_scripts(logic: &mut GameLogic) -> Value {
    assert!(!logic.scripts_loaded);
    let mut list = ScriptList::new();
    for (index, kind) in [ConditionType::PlayerHasPower, ConditionType::ConditionTrue]
        .into_iter()
        .enumerate()
    {
        let mut condition = Condition::new(kind);
        if index == 0 {
            condition
                .add_parameter(Parameter::with_string(ParameterType::Side, OWNER.into()))
                .unwrap();
        }
        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));
        let mut action = ScriptAction::new(ScriptActionType::SetFlag);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                if index == 0 { POWER_FLAG } else { SEEN_FLAG }.into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();
        let mut script = Script::new();
        script.script_name = SCRIPTS[index].into();
        script.is_one_shot = true;
        script.delay_evaluation_seconds = 0;
        script.condition = Some(Box::new(or_condition));
        script.action = Some(Box::new(action));
        list.append_script(Box::new(script));
    }
    let authored = serde_json::to_value(&list).unwrap();
    let engine = ScriptEngine::new().unwrap();
    assert!(engine.action_handler().is_none());
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    for flag in [POWER_FLAG, SEEN_FLAG] {
        engine.set_flag(flag, false).unwrap();
    }
    // This is identical definition admission before save/load, never a load
    // repair. Zero-delay list admission consumes no scheduling randomness.
    *get_script_engine().write().unwrap() = Some(engine);
    logic.scripts_loaded = true;
    authored
}

fn script_state() -> Value {
    with_script_engine_ref(|engine| {
        let scripts: Vec<_> = SCRIPTS.into_iter().map(|name| {
            let script = engine.find_script_clone_by_name(name).unwrap();
            json!({"name":name,"active":script.is_active,"one_shot":script.is_one_shot,
                "due_frame":script.frame_to_evaluate_at,"delay_seconds":script.delay_evaluation_seconds})
        }).collect();
        let (counters, flags) = engine.snapshot_named_trackers();
        json!({"power_flag":engine.get_flag(POWER_FLAG).unwrap().value,
            "first_frame_seen":engine.get_flag(SEEN_FLAG).unwrap().value,
            "scripts":scripts,"flags":flags,"counters":counters})
    }).unwrap()
}

fn observe(logic: &GameLogic) -> Value {
    assert!(!generals_main::gameworld_shadow::gameworld_damage_authority_live());
    assert!(!generals_main::gameworld_shadow::gameworld_construction_authority_live());
    let player = logic.get_player(0).unwrap();
    let mut objects: Vec<_> = logic.host_objects().values().collect();
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
        "faction":format!("{:?}",player.team),"local":player.is_local,"alive":player.is_alive,
        "produced":player.power_produced,"consumed":player.power_consumed,
        "available":player.power_available,"resources_power":player.resources.power,
        "sabotaged_till_frame":player.power_sabotaged_till_frame},
        "objects":objects,"ai_count":logic.host_ai_player_count(),"scripts_loaded":logic.scripts_loaded,
        "script_state":script_state()})
}

// Collect errors and continue both frames so a stale scalar cannot hide the
// actual scheduled script observation, or its persistence after convergence.
fn violations(logic: &GameLogic, case: Case, updates: u32) -> Vec<String> {
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
        logic.get_players().len() == 1 && logic.host_objects().len() == 2,
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
        value["player"]["produced"] == case.expected_produced(),
        "derived production",
    );
    require(
        value["player"]["consumed"] == case.consumed,
        "derived consumption",
    );
    require(
        value["player"]["available"] == case.expected_produced() - case.consumed,
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
            object.owner_player_id == Some(0)
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
    let has_power = case.control != Control::Sabotaged && case.expected_produced() >= case.consumed;
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
        filename: "power".into(),
        display_name: "Saved power first frame".into(),
        description: String::new(),
        map_name: "SavedPowerFixture".into(),
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
        assert!(logic.get_players().is_empty() && logic.host_objects().is_empty());
    }
    let authored_scripts = admit_scripts(&mut logic);
    let inputs = json!({"catalog":catalog(&logic, case),"scripts":authored_scripts,
        "initial_script_state":script_state(),"owner":{"id":0,"name":OWNER,"faction":"GLA"},
        "control":format!("{:?}",case.control),"source_warmup_frames":1,"post_load_mutations":"ordinary GameLogic::update only"});
    write_json(&dir.join(format!("{role}-inputs.json")), &inputs);
    let mut manager = SaveFileManager::with_save_directory(dir.join("saves"));
    manager.init().unwrap();
    if role == "writer" {
        let admission_errors = violations(&logic, case, 0);
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
        let (decoded, _) = manager.load_game_snapshot("power").unwrap();
        let producer = &decoded.objects[&ObjectId(1)];
        write_json(
            &dir.join("reader-decoded.json"),
            &json!({"world_version":decoded.version,
            "producer_health":producer.health,"producer_status":producer.status,
            "producer_type":producer.object_type}),
        );
        let economy_before = generals_main::game_logic::host_economy_log::len();
        if case.foreign_shadow() {
            load_with_foreign_shadow(&mut manager, &mut logic, case, &dir);
        } else {
            manager.load_game("power", &mut logic).unwrap();
        }
        let economy_after = generals_main::game_logic::host_economy_log::len();
        write_json(
            &dir.join("reader-load.json"),
            &json!({"result":"Ok","economy_events_before":economy_before,"economy_events_after":economy_after}),
        );
        assert_eq!(
            economy_before, economy_after,
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
        let current_errors = violations(&logic, case, updates);
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
            CHILD_TEST,
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
    output.status.success()
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
        let mut differences = Vec::new();
        for field in [
            "inputs",
            "observations",
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
            "checkpoint_equal":writer["observations"][0] == reader["observations"][0],
            "first_frame_equal":writer["observations"][1] == reader["observations"][1],
            "second_frame_equal":writer["observations"][2] == reader["observations"][2]}),
        );
    }
    write_json(
        &root.join("summary.json"),
        &json!({"cases":CASES.len(),"failures":failures,
        "scope":"real-file derived power and scheduled first-frame continuation; eligibility preservation and conflicting same-ID shadow controls; no C++ runtime or whole-world isolation claim"}),
    );
    assert!(failures.is_empty(), "{failures:?}");
}

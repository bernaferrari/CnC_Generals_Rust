//! Strict AI-registration preservation through the public real-file boundary.
//! Fresh writer/reader processes share authored inputs and save bytes only.
//! Synthetic v23 cases change the current v24 envelope, not its positional body;
//! the genuine historical v23 fixture is decoded separately and never relabeled.
use game_client::effects::particle_manager::{
    ParticleSystemManager, get_particle_system_manager, get_particle_system_manager_mut,
};
use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use gamelogic::object::armor::{TheArmorStore, load_armor_templates_from_str};
use gamelogic::weapon::{with_weapon_store, with_weapon_store_mut};
use generals_main::ai::AIDifficulty;
use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

const CHILD_TEST: &str = "ai_roster_save_process";
const ANCHORS: [&str; 3] = [
    "RosterLocalAnchor",
    "RosterAbsentAnchor",
    "RosterInactiveAnchor",
];
const ARMOR_INI: &str = "Armor RosterSaveUnusedArmor\nArmor = DEFAULT 100%\nEnd\n";
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
const HISTORICAL_V23: &[u8] = include_bytes!("fixtures/alliance_save_load/alliance_only.sav");

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    version: u32,
    populated: bool,
    inactive: bool,
}

const CASES: [Case; 6] = [
    Case {
        name: "empty_fresh_v24",
        version: 24,
        populated: false,
        inactive: false,
    },
    Case {
        name: "empty_populated_v24",
        version: 24,
        populated: true,
        inactive: false,
    },
    Case {
        name: "inactive_owner2_v24",
        version: 24,
        populated: false,
        inactive: true,
    },
    Case {
        name: "empty_fresh_synthetic_v23",
        version: 23,
        populated: false,
        inactive: false,
    },
    Case {
        name: "empty_populated_synthetic_v23",
        version: 23,
        populated: true,
        inactive: false,
    },
    Case {
        name: "inactive_owner2_synthetic_v23",
        version: 23,
        populated: false,
        inactive: true,
    },
];

fn catalog_world() -> GameLogic {
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
    for name in ANCHORS {
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Immobile)
            .set_health(1000.0)
            .set_primary_weapon_none();
        // C++ ThingTemplate defaults EnergyProduction to zero. Author it
        // explicitly so generic host CommandCenter fallback power is not an
        // incidental input to this roster witness. The original implicit-power
        // baseline is retained separately as a real immediate restore gap.
        template.energy_production = Some(0);
        logic.templates.insert(name.into(), template);
    }
    catalog(&logic);
    logic
}

fn catalog(logic: &GameLogic) -> Value {
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
    let mut expected: Vec<_> = GLA_SUPPORT.into_iter().chain(ANCHORS).collect();
    expected.sort_unstable();
    let mut names: Vec<_> = logic.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, expected);
    let templates: Vec<_> = names
        .into_iter()
        .map(|name| {
            let template = &logic.templates[name];
            if ANCHORS.contains(&name) {
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
                assert_eq!(template.energy_production, Some(0));
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

fn player_count(case: Case) -> u32 {
    if case.inactive { 3 } else { 2 }
}

fn admit_source(logic: &mut GameLogic, case: Case) {
    for id in 0..player_count(case) {
        let mut player = Player::new(id, Team::GLA, &format!("Saved owner {id}"), id == 0);
        player.alliance_team = 30 + id as i32;
        if !case.inactive {
            // Saved false must survive without skirmish constructor writeback.
            player.set_can_build_units(false);
            player.set_can_build_base(id == 0);
        }
        logic.add_player(player);
        let object = logic
            .create_object_for_player(
                ANCHORS[id as usize],
                id,
                Vec3::new(id as f32 * 2000.0, 0.0, 0.0),
            )
            .unwrap();
        assert_eq!(object, ObjectId(id + 1));
    }
    if case.inactive {
        logic.add_ai_opponent(2, Team::GLA, AIDifficulty::Hard);
        logic.set_ai_active(2, false);
        logic.relocate_host_ai_base(2, Vec3::new(47.0, 0.0, -31.0));
    }
    assert!(violations(logic, case).is_empty());
    logic.update();
    assert!(violations(logic, case).is_empty());
}

fn admit_stale_destination(logic: &mut GameLogic) {
    logic.add_player(Player::new(3, Team::GLA, "Stale destination owner", false));
    logic
        .create_object_for_player(ANCHORS[2], 3, Vec3::new(-2000.0, 0.0, 0.0))
        .unwrap();
    logic.add_ai_opponent(3, Team::GLA, AIDifficulty::Brutal);
    assert_eq!(logic.host_ai_player_count(), 1);
    assert!(logic.is_host_ai_active(3));
    assert_eq!(logic.host_ai_difficulty(3), Some(AIDifficulty::Brutal));
    assert_eq!(logic.snapshot_host_ai_players_for_save()[0].player_id, 3);
}

fn observe(logic: &GameLogic) -> Value {
    let mut players: Vec<_> = logic.get_players().values().collect();
    players.sort_by_key(|p| p.id);
    let players: Vec<_> = players.into_iter().map(|p| json!({
        "id":p.id,"name":p.name,"faction":format!("{:?}",p.team),"local":p.is_local,
        "alive":p.is_alive,"alliance":p.alliance_team,"can_build_units":p.can_build_units,
        "can_build_base":p.can_build_base,"supplies":p.effective_supplies(),"power":p.power_available,
        "ai_active":logic.is_host_ai_active(p.id),"ai_difficulty":format!("{:?}",logic.host_ai_difficulty(p.id))
    })).collect();
    let mut objects: Vec<_> = logic.host_objects().values().collect();
    objects.sort_by_key(|o| o.id.0);
    let objects: Vec<_> = objects.into_iter().map(|o| json!({
        "id":o.id.0,"template":o.get_template().name,"owner":o.owner_player_id,
        "faction":format!("{:?}",o.team),"alive":o.is_alive(),"position":o.get_position().to_array(),
        "health":o.health.current,"target":o.target.map(|id|id.0),"state":format!("{:?}",o.ai_state),
        "paused":logic.skirmish_ai_auto_engage_paused(o.id),"discharge":o.weapon_discharge_marker().sequence,
        "fire_intents":o.fire_intent_count,"weapon_slots":(0..3).map(|slot|o.weapon_slot(slot).is_some()).collect::<Vec<_>>()
    })).collect();
    json!({"frame":logic.get_frame(),"players":players,"objects":objects,
        "ai_count":logic.host_ai_player_count(),"ai_activity":logic.host_ai_activity_count(),
        "ai_rows":logic.snapshot_host_ai_players_for_save()})
}

// Collect strict contract violations so baseline evidence includes both the
// immediate load and its first ordinary frame before the child reports failure.
fn violations(logic: &GameLogic, case: Case) -> Vec<String> {
    let mut errors = Vec::new();
    let mut require = |condition: bool, label: String| {
        if !condition {
            errors.push(label);
        }
    };
    let count = player_count(case);
    require(
        logic.get_players().len() == count as usize,
        "player count".into(),
    );
    require(
        logic.host_objects().len() == count as usize,
        "object count".into(),
    );
    let rows = logic.snapshot_host_ai_players_for_save();
    let expected_ids: Vec<u32> = if case.inactive { vec![2] } else { vec![] };
    require(
        rows.iter().map(|r| r.player_id).collect::<Vec<_>>() == expected_ids,
        "exact AI roster".into(),
    );
    require(
        logic.host_ai_player_count() == expected_ids.len(),
        "AI count".into(),
    );
    require(logic.host_ai_activity_count() == 0, "AI activity".into());
    require(
        logic.get_player(3).is_none()
            && !logic.is_host_ai_active(3)
            && logic.host_ai_difficulty(3).is_none(),
        "stale owner/controller removed".into(),
    );
    for id in 0..count {
        require(
            !logic.is_host_ai_active(id),
            format!("owner {id} inactive/absent"),
        );
        let difficulty = if case.inactive && id == 2 {
            Some(AIDifficulty::Hard)
        } else {
            None
        };
        require(
            logic.host_ai_difficulty(id) == difficulty,
            format!("owner {id} difficulty"),
        );
        if let Some(p) = logic.get_player(id) {
            require(
                p.team == Team::GLA && p.is_local == (id == 0) && p.is_alive,
                format!("owner {id} identity/alive"),
            );
            require(
                p.can_build_units == case.inactive,
                format!("owner {id} build units"),
            );
            require(
                p.can_build_base == (case.inactive || id == 0),
                format!("owner {id} build base"),
            );
        } else {
            require(false, format!("owner {id} missing"));
        }
        if let Some(o) = logic.host_object(ObjectId(id + 1)) {
            require(
                o.owner_player_id == Some(id)
                    && o.team == Team::GLA
                    && o.get_template().name == ANCHORS[id as usize]
                    && o.is_alive(),
                format!("object {} identity/alive", id + 1),
            );
            require(
                logic.skirmish_ai_auto_engage_paused(o.id) == (id != 0),
                format!("owner {id} pause"),
            );
            require(
                o.target.is_none()
                    && o.weapon_discharge_marker().sequence == 0
                    && o.fire_intent_count == 0
                    && (0..3).all(|slot| o.weapon_slot(slot).is_none()),
                format!("owner {id} weaponless inactivity"),
            );
            require(
                o.power_provided == 0 && o.power_consumed == 0,
                format!("owner {id} authored zero energy"),
            );
        } else {
            require(false, format!("object {} missing", id + 1));
        }
    }
    if case.inactive {
        if let Some(row) = rows.iter().find(|r| r.player_id == 2) {
            require(
                !row.is_active
                    && row.difficulty == "Hard"
                    && row.base_center == Some(Vec3::new(47.0, 0.0, -31.0)),
                "specified AI configuration".into(),
            );
        }
    }
    errors
}

fn info() -> SaveGameInfo {
    SaveGameInfo {
        filename: "roster".into(),
        display_name: "AI roster restore".into(),
        description: String::new(),
        map_name: "AiRosterFixture".into(),
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

fn journal(file: &mut fs::File, phase: &str, observation: &Value, errors: &[String]) {
    serde_json::to_writer(
        &mut *file,
        &json!({"phase":phase,"observation":observation,"violations":errors}),
    )
    .unwrap();
    file.write_all(b"\n").unwrap();
    file.flush().unwrap();
}

fn synthetic_v23(bytes: &[u8]) -> Vec<u8> {
    let mut output = bytes.to_vec();
    let mut pos = 0;
    let mut changed = 0;
    while pos < bytes.len() {
        let len = bytes[pos] as usize;
        pos += 1;
        let name = std::str::from_utf8(&bytes[pos..pos + len]).unwrap();
        pos += len;
        if name == "SG_EOF" {
            break;
        }
        let size = i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        assert!(size >= 0);
        pos += 4;
        if name == "CHUNK_GameLogic" {
            assert!(size >= 4);
            assert_eq!(&bytes[pos..pos + 4], &24u32.to_le_bytes());
            output[pos..pos + 4].copy_from_slice(&23u32.to_le_bytes());
            changed += 1;
        }
        pos += size as usize;
    }
    assert_eq!(changed, 1);
    assert_eq!(bytes.iter().zip(&output).filter(|(a, b)| a != b).count(), 1);
    output
}

fn inspect_historical(dir: &Path) {
    let mut manager = SaveFileManager::with_save_directory(dir.join("saves"));
    manager.init().unwrap();
    fs::write(manager.get_save_path("genuine_v23"), HISTORICAL_V23).unwrap();
    let (snapshot, _) = manager.load_game_snapshot("genuine_v23").unwrap();
    write_json(
        &dir.join("historical-v23.json"),
        &json!({
        "provenance":"genuine pre-fix writer output from 82db175dd1e6ceb9c754ff72e47408c4428a1d86",
        "repository_fixture":"tests/fixtures/alliance_save_load/alliance_only.sav",
        "expected_sha256":"82f4d52426c1844ad218a138307c6e46268ae70f0b096a9fc303b1dc1f71604a",
        "bytes":HISTORICAL_V23.len(),"crc32":crc32fast::hash(HISTORICAL_V23),
        "world_version":snapshot.version,"ai_rows":snapshot.ai_players,
        "players":snapshot.players.len(),"objects":snapshot.objects.len(),
        "operation":"public decode only; no live restore or original C++ engine execution"}),
    );
    assert_eq!(snapshot.version, 23);
    assert!(snapshot.ai_players.is_empty());
    assert_eq!((snapshot.players.len(), snapshot.objects.len()), (2, 2));
}

#[test]
#[ignore = "child entry point; run through ai_rosters_survive_public_files_in_fresh_processes"]
fn ai_roster_save_process() {
    let role = std::env::var("AI_ROSTER_SAVE_ROLE").unwrap();
    let dir = std::env::current_dir().unwrap();
    if role == "historical" {
        inspect_historical(&dir);
        return;
    }
    assert!(role == "writer" || role == "reader");
    let name = std::env::var("AI_ROSTER_SAVE_CASE").unwrap();
    let case = *CASES.iter().find(|case| case.name == name).unwrap();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{role}-observations.jsonl")))
        .unwrap();
    let mut logic = catalog_world();
    let inputs = catalog(&logic);
    write_json(&dir.join(format!("{role}-inputs.json")), &inputs);
    let mut manager = SaveFileManager::with_save_directory(dir.join("saves"));
    manager.init().unwrap();
    if role == "writer" {
        admit_source(&mut logic, case);
        let before = observe(&logic);
        journal(&mut file, "before_save", &before, &violations(&logic, case));
        manager.save_game("roster", &logic, &info()).unwrap();
        assert_eq!(observe(&logic), before, "save must not change the writer");
        let original = fs::read(manager.get_save_path("roster")).unwrap();
        fs::write(dir.join("original-v24.sav"), &original).unwrap();
        if case.version == 23 {
            let derived = synthetic_v23(&original);
            fs::write(dir.join("synthetic-v23.sav"), &derived).unwrap();
            fs::write(manager.get_save_path("roster"), derived).unwrap();
        }
    } else {
        assert!(logic.get_players().is_empty() && logic.host_objects().is_empty());
        if case.populated {
            admit_stale_destination(&mut logic);
        }
        journal(&mut file, "destination_before_load", &observe(&logic), &[]);
    }
    if role == "reader" {
        // Decode only in the reader: even file decoding stages compatibility
        // companions, so it must not perturb the untouched writer continuation.
        let (decoded, _) = manager.load_game_snapshot("roster").unwrap();
        write_json(
            &dir.join("reader-decoded.json"),
            &json!({
            "world_version":decoded.version,"ai_rows":decoded.ai_players,
            "provenance":if case.version == 24 { "actual current public writer" } else { "synthetic v24 body with v23 envelope; Players v4 unchanged" }}),
        );
        assert_eq!(decoded.version, case.version);
        assert_eq!(
            decoded
                .ai_players
                .iter()
                .map(|row| row.player_id)
                .collect::<Vec<_>>(),
            if case.inactive { vec![2] } else { vec![] }
        );
        let result = manager.load_game("roster", &mut logic);
        journal(
            &mut file,
            "load_result",
            &json!({"result":format!("{result:?}"),"world":observe(&logic)}),
            &[],
        );
        result.unwrap();
    }
    let checkpoint = observe(&logic);
    let mut errors = violations(&logic, case);
    journal(&mut file, "checkpoint", &checkpoint, &errors);
    assert_eq!(logic.get_frame(), 1);
    assert_eq!(catalog(&logic), inputs);
    logic.update();
    let continuation = observe(&logic);
    let after_errors = violations(&logic, case);
    journal(&mut file, "after_one_frame", &continuation, &after_errors);
    errors.extend(after_errors);
    assert_eq!(logic.get_frame(), 2);
    assert_eq!(catalog(&logic), inputs);
    let saved = fs::read(manager.get_save_path("roster")).unwrap();
    write_json(
        &dir.join(format!("{role}.json")),
        &json!({
        "case":case.name,"role":role,"pid":std::process::id(),"cwd":dir,
        "executable":std::env::current_exe().unwrap(),"inputs":inputs,
        "checkpoint":checkpoint,"continuation":continuation,"violations":errors,
        "save_bytes":saved.len(),"save_crc32":crc32fast::hash(&saved),
        "source_crc32":crc32fast::hash(include_bytes!("ai_roster_save_restore.rs")),
        "post_load_mutations":"ordinary GameLogic::update only"}),
    );
    assert!(
        errors.is_empty(),
        "strict roster contract failed: {errors:?}"
    );
}

fn child(dir: &Path, case: Option<Case>, role: &str) -> bool {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            CHILD_TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("AI_ROSTER_SAVE_ROLE", role)
        .current_dir(dir);
    if let Some(case) = case {
        command.env("AI_ROSTER_SAVE_CASE", case.name);
    }
    let output = command.output().expect("start fresh roster process");
    fs::write(dir.join(format!("{role}.stdout.txt")), &output.stdout).unwrap();
    fs::write(dir.join(format!("{role}.stderr.txt")), &output.stderr).unwrap();
    write_json(
        &dir.join(format!("{role}-status.json")),
        &json!({
        "success":output.status.success(),"exit_code":output.status.code(),
        "status":output.status.to_string()}),
    );
    output.status.success()
}

#[test]
fn ai_rosters_survive_public_files_in_fresh_processes() {
    assert!(std::env::var_os("AI_ROSTER_SAVE_ROLE").is_none());
    let root = std::env::var_os("AI_ROSTER_SAVE_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| tempfile::TempDir::new().unwrap().keep());
    fs::create_dir_all(&root).unwrap();
    println!("AI_ROSTER_SAVE_EVIDENCE {}", root.display());
    let historical = root.join("genuine_historical_v23");
    fs::create_dir(&historical).expect("use a new evidence directory");
    let mut failures = Vec::new();
    if !child(&historical, None, "historical") {
        failures.push("genuine historical v23 decode".into());
    }
    for case in CASES {
        let dir = root.join(case.name);
        fs::create_dir(&dir).expect("never overwrite prior evidence");
        let locomotor = dir.join(LOCOMOTOR_PATH);
        fs::create_dir_all(locomotor.parent().unwrap()).unwrap();
        fs::write(locomotor, LOCOMOTOR_INI).unwrap();
        let writer_ok = child(&dir, Some(case), "writer");
        let reader_ok = writer_ok && child(&dir, Some(case), "reader");
        println!(
            "AI_ROSTER_CASE {} writer={writer_ok} reader={reader_ok}",
            case.name
        );
        if !writer_ok || !reader_ok {
            failures.push(format!(
                "{} writer={writer_ok} reader={reader_ok}",
                case.name
            ));
            continue;
        }
        let read = |role| -> Value {
            serde_json::from_slice(&fs::read(dir.join(format!("{role}.json"))).unwrap()).unwrap()
        };
        let writer = read("writer");
        let reader = read("reader");
        assert_ne!(writer["pid"], reader["pid"]);
        for field in [
            "inputs",
            "checkpoint",
            "continuation",
            "save_bytes",
            "save_crc32",
            "source_crc32",
        ] {
            if writer[field] != reader[field] {
                failures.push(format!("{} exact {field} differs", case.name));
            }
        }
    }
    write_json(
        &root.join("summary.json"),
        &json!({"cases":6,"failures":failures,
        "scope":"successful real-file AI roster preservation; no malformed-load atomicity or instance-isolation claim"}),
    );
    assert!(failures.is_empty(), "{failures:?}");
}

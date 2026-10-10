//! Real-file hostility continuation and instance-owned player/team save capsules.
use game_client::effects::particle_manager::{
    ParticleSystemManager, get_particle_system_manager_mut,
};
use gamelogic::common::Relationship;
use gamelogic::weapon::{with_weapon_store, with_weapon_store_mut};
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    AbleToAttackType, CanAttackResult, GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate,
    Weapon,
};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use serde_json::{Value, json};
use std::sync::Once;
use std::time::{Duration, UNIX_EPOCH};
use tempfile::TempDir;

const ATTACKER: &str = "AllianceSaveAttacker";
const TARGET: &str = "AllianceSaveTarget";

fn admit_catalogs() {
    assert!(generals_main::assets::get_asset_manager().is_none());
    assert!(
        game_engine::common::thing::thing_factory::get_thing_factory()
            .unwrap()
            .is_none()
    );
    static WEAPON_ADMISSION: Once = Once::new();
    WEAPON_ADMISSION.call_once(|| {
        gamelogic::initialize_weapon_store().unwrap();
        with_weapon_store_mut(|store| {
            assert_eq!(store.get_template_count(), 0);
            // Every fixture weapon is inline and unnamed. Finish this exact
            // empty catalog before pitch queries, without disk discovery.
            store.mark_host_bootstrap_complete();
        })
        .unwrap();
    });
    with_weapon_store(|store| {
        assert!(store.host_bootstrap_is_complete());
        assert_eq!(store.get_template_count(), 0);
    })
    .unwrap();
    // Preload the source-defined combat presets for writer and fresh supplied-
    // file readers alike. Preserve any active systems from earlier operations.
    let mut guard = get_particle_system_manager_mut().unwrap();
    let manager = guard.get_or_insert_with(ParticleSystemManager::new);
    for name in ["MuzzleFlash", "BulletImpact"] {
        assert!(manager.ensure_preset_template(name).is_some());
    }
}

fn world() -> GameLogic {
    admit_catalogs();
    let mut world = GameLogic::new();
    let mut attacker = ThingTemplate::new(ATTACKER);
    attacker
        .set_health(360.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Vehicle);
    attacker.set_primary_weapon(Weapon {
        damage: 25.0,
        range: 100.0,
        reload_time: 0.0,
        projectile_speed: 0.0,
        splash_radius: 0.0,
        ..Weapon::default()
    });
    let mut target = ThingTemplate::new(TARGET);
    target
        .set_health(240.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Vehicle)
        .set_primary_weapon_none();
    world.templates.insert(ATTACKER.into(), attacker);
    world.templates.insert(TARGET.into(), target);
    for template in world.templates.values() {
        assert!(template.primary_weapon_name.is_none());
        assert!(
            template.secondary_weapon_name.is_none() && template.tertiary_weapon_name.is_none()
        );
        assert!(template.locomotor_name.is_none() && template.locomotor_set_names.is_empty());
        assert!(template.authored_locomotor_sets.is_none() && template.armor_sets.is_empty());
    }
    world
}

fn checkpoint(world: &GameLogic, attacker: ObjectId, target: ObjectId) -> Value {
    let a = world.host_object(attacker).unwrap();
    let t = world.host_object(target).unwrap();
    let weapon = a.weapon.as_ref().unwrap();
    let players: Vec<_> = [0, 1]
        .into_iter()
        .map(|id| {
            let p = world.get_player(id).unwrap();
            let mut relations: Vec<_> = p
                .map_side
                .relations
                .iter()
                .map(|(id, r)| (*id, format!("{r:?}")))
                .collect();
            relations.sort();
            json!({"id": id, "alliance": p.alliance_team, "relations": relations})
        })
        .collect();
    json!({"frame":world.get_frame(), "owners":[a.owner_player_id,t.owner_player_id], "players":players,
        "relationships":[format!("{:?}",world.player_relationship(0,1)),format!("{:?}",world.player_relationship(1,0))],
        "object_relationship":format!("{:?}",world.object_relationship(a,t)),
        "eligibility":format!("{:?}",world.get_able_to_attack_specific_object(attacker,target,AbleToAttackType::NewTarget,true)),
        "target_position":t.get_position().to_array(), "health":[a.health.current,t.health.current],
        "templates":[a.get_template().name,t.get_template().name], "weapon_name":a.weapon_name_for_slot(0),
        "weapon":[weapon.damage,weapon.range,weapon.reload_time,weapon.projectile_speed,weapon.splash_radius,weapon.last_fire_time],
        "target_has_weapon":t.weapon.is_some(),"intents":a.fire_intent_count,"sequence":a.weapon_discharge_marker().sequence})
}

fn continuation(world: &mut GameLogic, attacker: ObjectId, target: ObjectId) -> Vec<Value> {
    world.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    let mut trace = Vec::new();
    for _ in 0..2 {
        let processed_frame = world.get_frame();
        world.update();
        let a = world.host_object(attacker).unwrap();
        let t = world.host_object(target).unwrap();
        let marker = a.weapon_discharge_marker();
        trace.push(json!({"processed_frame":processed_frame,"frame":world.get_frame(),"sequence":marker.sequence,
            "marker_frame":marker.logic_frame,"slot":marker.weapon_slot,"intents":a.fire_intent_count,
            "last_fire_target":a.last_fire_victim_host,"last_fire_damage":a.last_fire_damage,"last_fire_frame":a.last_fire_frame,
            "target":format!("{:?}",a.target),"ai":format!("{:?}",a.ai_state),"health":t.health.current,
            "damage_type":format!("{:?}",t.last_damage_info_type)}));
    }
    trace
}

// Retain controller/pause observations beside the saved-input/full-trace
// comparison; the AI-roster suite separately asserts exact registration.
fn pause_observation(world: &GameLogic, attacker: ObjectId, target: ObjectId) -> Value {
    let a = world.host_object(attacker).unwrap();
    let t = world.host_object(target).unwrap();
    let players: Vec<_> = [0, 1]
        .into_iter()
        .map(|id| {
            let p = world.get_player(id).unwrap();
            json!({"id":id,"faction":format!("{:?}",p.team),"local":p.is_local,
            "alive":p.is_alive,"ai_active":world.is_host_ai_active(id)})
        })
        .collect();
    json!({"frame":world.get_frame(),"owners":[a.owner_player_id,t.owner_player_id],
        "factions":[format!("{:?}",a.team),format!("{:?}",t.team)],
        "team_instances":[a.team_instance_name,t.team_instance_name],"players":players,
        "ai_count":world.host_ai_player_count(),"paused":world.skirmish_ai_auto_engage_paused(attacker),
        "target":format!("{:?}",a.target),"ai":format!("{:?}",a.ai_state),
        "attack_substate":format!("{:?}",a.attack_substate),
        "sequence":a.weapon_discharge_marker().sequence,"target_health":t.health.current})
}

#[test]
fn same_faction_command_phase_preserves_human_attack() {
    let mut source = world();
    for id in 0..2 {
        let mut p = Player::new(id, Team::USA, "same_faction", id == 0);
        p.alliance_team = [31, 45][id as usize];
        source.add_player(p);
    }
    let attacker = source
        .create_object_for_player(ATTACKER, 0, Vec3::ZERO)
        .unwrap();
    let target = source
        .create_object_for_player(TARGET, 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    manager.save_game("phase", &source, &info()).unwrap();
    let mut traces = Vec::new();
    for label in ["source", "restored"] {
        if label == "restored" {
            drop(source);
            source = world();
            manager.load_game("phase", &mut source).unwrap();
        }
        println!(
            "phase {label} before={}",
            pause_observation(&source, attacker, target)
        );
        source.queue_command(GameCommand {
            command_type: CommandType::AttackObject { target_id: target },
            player_id: 0,
            command_id: 1,
            timestamp: UNIX_EPOCH + Duration::from_secs(1),
            selected_units: vec![attacker],
            modifier_keys: ModifierKeys::default(),
        });
        source.process_commands();
        println!(
            "phase {label} admitted={}",
            pause_observation(&source, attacker, target)
        );
        assert_eq!(source.host_object(attacker).unwrap().target, Some(target));
        assert_eq!(
            source.host_object(attacker).unwrap().ai_state,
            generals_main::game_logic::AIState::Attacking
        );
        let mut trace = Vec::new();
        for _ in 0..2 {
            source.update();
            let observation = pause_observation(&source, attacker, target);
            println!("phase {label} tick={observation}");
            let a = source.host_object(attacker).unwrap();
            trace.push(json!({"target":format!("{:?}",a.target),"ai":format!("{:?}",a.ai_state),
                "sequence":a.weapon_discharge_marker().sequence,"health":source.host_object(target).unwrap().health.current}));
        }
        traces.push(trace);
    }
    assert_eq!(
        traces[0], traces[1],
        "ordinary commands must continue identically after admission"
    );
    assert_eq!(traces[0][1]["sequence"], json!(1));
    assert_eq!(traces[0][1]["health"], json!(215.0));
}

fn info() -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: "alliance".into(),
        display_name: "Alliance continuation".into(),
        description: String::new(),
        map_name: "AllianceFixture".into(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn round_trip(explicit: bool) {
    let label = if explicit {
        "explicit_map"
    } else {
        "alliance_only"
    };
    let mut source = world();
    for (id, team) in [(0, Team::USA), (1, Team::GLA)] {
        let mut p = Player::new(id, team, label, id == 0);
        if explicit {
            p.set_map_relationship(1 - id, Relationship::Enemies);
        } else {
            p.alliance_team = id as i32;
        }
        source.add_player(p);
    }
    let attacker = source
        .create_object_for_player(ATTACKER, 0, Vec3::ZERO)
        .unwrap();
    let target = source
        .create_object_for_player(TARGET, 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let before = checkpoint(&source, attacker, target);
    assert_eq!(before["owners"], json!([0, 1]));
    assert_eq!(
        before["players"],
        if explicit {
            json!([{"id":0,"alliance":-1,"relations":[[1,"Enemies"]]}, {"id":1,"alliance":-1,"relations":[[0,"Enemies"]]}])
        } else {
            json!([{"id":0,"alliance":0,"relations":[]}, {"id":1,"alliance":1,"relations":[]}])
        }
    );
    assert_eq!(before["weapon"], json!([25.0, 100.0, 0.0, 0.0, 0.0, 0.0]));
    assert_eq!(before["weapon_name"], Value::Null);
    assert_eq!(before["health"], json!([360.0, 240.0]));
    assert_eq!(before["target_has_weapon"], json!(false));
    assert_eq!(before["intents"], json!(0));
    assert_eq!(before["sequence"], json!(0));
    assert_eq!(source.player_relationship(0, 1), Relationship::Enemies);
    assert_eq!(
        source.get_able_to_attack_specific_object(
            attacker,
            target,
            AbleToAttackType::NewTarget,
            true
        ),
        CanAttackResult::Possible
    );
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    manager.save_game(label, &source, &info()).unwrap();
    let path = manager.get_save_path(label);
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    if let Ok(destination) = std::env::var("ALLIANCE_SAVE_OUTPUT") {
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::copy(
            &path,
            std::path::Path::new(&destination).join(format!("{label}.sav")),
        )
        .unwrap();
    }
    let after_save = checkpoint(&source, attacker, target);
    assert_eq!(before, after_save, "save must not change live authoring");
    let expected = continuation(&mut source, attacker, target);
    println!(
        "{label} before={before}\n{label} after_save={after_save}\n{label} untouched={}",
        json!(expected)
    );
    let shots = expected.last().unwrap()["sequence"].as_u64().unwrap();
    assert!(shots > 0, "untouched control must discharge");
    for row in &expected {
        assert_eq!(
            row["health"].as_f64().unwrap(),
            240.0 - 25.0 * row["sequence"].as_u64().unwrap() as f64
        );
        if row["sequence"].as_u64().unwrap() > 0 {
            assert_eq!(row["last_fire_target"], json!(target.0));
            assert_eq!(row["last_fire_damage"], json!(25.0));
            assert_eq!(row["marker_frame"], row["processed_frame"]);
            assert_eq!(row["last_fire_frame"], row["processed_frame"]);
            assert_eq!(row["slot"], json!(0));
        }
    }
    drop(source);
    let mut restored = world();
    manager.load_game(label, &mut restored).unwrap();
    let after = checkpoint(&restored, attacker, target);
    let actual = continuation(&mut restored, attacker, target);
    // Diagnose both immediate loss and actual command behavior before asserting.
    println!(
        "{label} immediately_loaded={after}\n{label} restored={}",
        json!(actual)
    );
    assert_eq!(
        (after, actual),
        (before, expected),
        "{label}: real-file continuation must preserve hostility inputs and combat"
    );
}

#[test]
fn alliance_only_real_file_continuation() {
    round_trip(false);
}
#[test]
fn explicit_map_real_file_positive_control() {
    round_trip(true);
}

/// Retain this compiled pre-fix reader with the old writer's actual .sav bytes.
#[test]
#[ignore = "external compatibility probe: ALLIANCE_SAVE_INPUT and optional ALLIANCE_EXPECT_REJECTION"]
fn externally_supplied_save_reader() {
    let input =
        std::path::PathBuf::from(std::env::var_os("ALLIANCE_SAVE_INPUT").expect("input path"));
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    std::fs::copy(input, manager.get_save_path("external")).unwrap();
    let mut restored = world();
    let result = manager.load_game("external", &mut restored);
    println!(
        "external result={result:?} frame={} players={} objects={}",
        restored.get_frame(),
        restored.get_players().len(),
        restored.host_objects().len()
    );
    if std::env::var_os("ALLIANCE_EXPECT_REJECTION").is_some() {
        assert!(result.is_err());
        assert_eq!(restored.get_frame(), 0);
        assert!(restored.get_players().is_empty());
        assert!(restored.host_objects().is_empty());
    } else {
        result.unwrap();
        let attacker = restored
            .host_objects()
            .values()
            .find(|o| o.get_template().name == ATTACKER)
            .unwrap()
            .id;
        let target = restored
            .host_objects()
            .values()
            .find(|o| o.get_template().name == TARGET)
            .unwrap()
            .id;
        println!(
            "external immediately_loaded={}",
            checkpoint(&restored, attacker, target)
        );
        println!(
            "external continuation={}",
            json!(continuation(&mut restored, attacker, target))
        );
    }
}

fn replace_chunk(bytes: &[u8], wanted: &str, replacement: Option<&[u8]>) -> Vec<u8> {
    let mut output = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let start = pos;
        let length = bytes[pos] as usize;
        pos += 1;
        let name = std::str::from_utf8(&bytes[pos..pos + length]).unwrap();
        pos += length;
        if name == "SG_EOF" {
            output.extend_from_slice(&bytes[start..]);
            break;
        }
        let size = i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let end = pos + size;
        if name == wanted {
            if let Some(payload) = replacement {
                output.extend_from_slice(&bytes[start..pos - 4]);
                output.extend_from_slice(&(payload.len() as i32).to_le_bytes());
                output.extend_from_slice(payload);
            }
        } else {
            output.extend_from_slice(&bytes[start..end]);
        }
        pos = end;
    }
    output
}

#[test]
fn pending_players_do_not_leak_across_real_file_loads() {
    let legacy = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/alliance_save_load");
    let original = std::fs::read(legacy.join("alliance_only.sav")).unwrap();
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    std::fs::copy(
        legacy.join("explicit_map.sav"),
        manager.get_save_path("seed"),
    )
    .unwrap();
    let cases: [(&str, Option<&[u8]>, bool); 5] = [
        ("missing", None, false),
        ("null", Some(&[1]), false),
        ("empty", Some(&[]), true),
        ("truncated", Some(&[3, 2, 0, 0]), true),
        ("future", Some(&[255]), true),
    ];
    let mut failures = Vec::new();
    for (name, payload, should_error) in cases {
        // Retain a successful decoded file while attempting a separate candidate.
        let _retained_seed = manager.load_game_snapshot("seed").unwrap();
        std::fs::write(
            manager.get_save_path(name),
            replace_chunk(&original, "CHUNK_Players", payload),
        )
        .unwrap();
        let mut restored = world();
        let result = manager.load_game(name, &mut restored);
        let inherited = restored
            .get_player(0)
            .map(|p| !p.map_side.relations.is_empty())
            .unwrap_or(false);
        println!(
            "negative {name}: result={result:?} inherited_previous_relations={inherited} players={}",
            restored.get_players().len()
        );
        if result.is_err() != should_error || inherited {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "stale or silently accepted payloads: {failures:?}"
    );
}

#[test]
fn exact_alliance_authorship_and_relationship_precedence_survive() {
    run_authority_cases(false, None);
}

#[test]
fn same_faction_real_file_continuation_known_gap() {
    run_authority_cases(true, Some("same_faction"));
}

fn run_authority_cases(continue_same_faction: bool, selected: Option<&str>) {
    use Relationship::{Allies, Enemies, Neutral};
    let mut mismatches = Vec::new();
    for (label, alliances, forward, reverse, same_faction, inactive, expected) in [
        ("allied", [7, 7], None, None, false, false, [Allies, Allies]),
        (
            "default",
            [-1, -1],
            None,
            None,
            false,
            false,
            [Neutral, Neutral],
        ),
        (
            "unassigned",
            [7, -1],
            None,
            None,
            false,
            false,
            [Neutral, Neutral],
        ),
        (
            "signed",
            [i32::MIN, i32::MAX],
            None,
            None,
            false,
            false,
            [Neutral, Neutral],
        ),
        (
            "same_faction",
            [31, 45],
            None,
            None,
            true,
            false,
            [Enemies, Enemies],
        ),
        (
            "neutral_override",
            [1, 2],
            Some(Neutral),
            None,
            false,
            false,
            [Neutral, Enemies],
        ),
        (
            "allied_override",
            [1, 2],
            Some(Allies),
            None,
            false,
            false,
            [Allies, Enemies],
        ),
        (
            "enemy_override",
            [9, 9],
            Some(Enemies),
            None,
            false,
            false,
            [Enemies, Allies],
        ),
        (
            "inactive",
            [1, 2],
            Some(Enemies),
            Some(Enemies),
            false,
            true,
            [Neutral, Neutral],
        ),
    ] {
        let diagnostic_selection = std::env::var("ALLIANCE_AUTHORITY_CASE").ok();
        if selected
            .or(diagnostic_selection.as_deref())
            .is_some_and(|selected| selected != label)
        {
            continue;
        }
        let mut source = world();
        for id in 0..2 {
            let team = if id == 0 || same_faction {
                Team::USA
            } else {
                Team::GLA
            };
            let mut p = Player::new(id, team, label, id == 0);
            p.alliance_team = alliances[id as usize];
            p.is_alive = !(id == 1 && inactive);
            if let Some(r) = if id == 0 { forward } else { reverse } {
                p.set_map_relationship(1 - id, r);
            }
            source.add_player(p);
        }
        let attacker = source
            .create_object_for_player(ATTACKER, 0, Vec3::ZERO)
            .unwrap();
        let target = source
            .create_object_for_player(TARGET, 1, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        let before = checkpoint(&source, attacker, target);
        if same_faction {
            println!(
                "authority {label} pause_before={}",
                pause_observation(&source, attacker, target)
            );
        }
        assert_eq!(
            [
                source.player_relationship(0, 1),
                source.player_relationship(1, 0)
            ],
            expected,
            "{label}"
        );
        assert_eq!(source.player_relationship(0, 0), Allies);
        assert_eq!(source.player_relationship(0, 99), Neutral);
        assert_eq!(before["owners"], json!([0, 1]));
        let directory = TempDir::new().unwrap();
        let mut manager = SaveFileManager::with_save_directory(directory.path());
        manager.init().unwrap();
        manager.save_game(label, &source, &info()).unwrap();
        if let Ok(destination) = std::env::var("ALLIANCE_SAVE_OUTPUT") {
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::copy(
                manager.get_save_path(label),
                std::path::Path::new(&destination).join(format!("{label}.sav")),
            )
            .unwrap();
        }
        assert_eq!(checkpoint(&source, attacker, target), before);
        // Normal same-faction coverage is only the exact saved-input boundary.
        // The separately runnable known-gap test keeps the original full trace
        // equality expectation, including the untouched source behavior.
        let compare_trace = !same_faction || continue_same_faction;
        let expected_trace = if compare_trace {
            continuation(&mut source, attacker, target)
        } else {
            Vec::new()
        };
        let shots = expected_trace
            .last()
            .map(|row| row["sequence"].as_u64().unwrap())
            .unwrap_or(0);
        println!("authority {label} untouched={}", json!(expected_trace));
        if compare_trace && !inactive && (shots > 0) != (expected[0] == Enemies) {
            mismatches.push(format!("{label}: untouched discharge eligibility"));
        }
        if same_faction && compare_trace {
            if expected_trace[1]["sequence"] != json!(1)
                || expected_trace[1]["health"] != json!(215.0)
                || expected_trace[1]["marker_frame"] != json!(1)
            {
                mismatches.push(format!("{label}: human continuation must discharge exactly 25 damage at processed frame 1"));
            }
        }
        drop(source);
        let mut restored = world();
        manager.load_game(label, &mut restored).unwrap();
        let after = checkpoint(&restored, attacker, target);
        if same_faction {
            println!(
                "authority {label} pause_loaded={}",
                pause_observation(&restored, attacker, target)
            );
        }
        assert_eq!(restored.get_player(1).unwrap().is_alive, !inactive);
        assert_eq!(restored.player_relationship(0, 0), Allies);
        assert_eq!(restored.player_relationship(0, 99), Neutral);
        let actual_trace = if compare_trace {
            continuation(&mut restored, attacker, target)
        } else {
            Vec::new()
        };
        println!(
            "authority {label}: before={before} after={after} trace={}",
            json!(actual_trace)
        );
        if (after, actual_trace) != (before, expected_trace) {
            mismatches.push(format!("{label}: checkpoint/continuation"));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

/// Actual historical row layouts, independently encoded; no relabeled v4 bytes.
fn players_payload(version: u8, alliances: [Option<i32>; 2]) -> Vec<u8> {
    use game_engine::common::system::{
        xfer::Xfer as CommonXfer, xfer_save::XferSave as CommonSave,
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut x = CommonSave::new(&mut cursor, 1);
        x.xfer_unsigned_byte(&mut version.clone()).unwrap();
        x.xfer_unsigned_short(&mut 2).unwrap();
        for id in 0..2u32 {
            x.xfer_unsigned_int(&mut id.clone()).unwrap();
            for _ in 0..3 {
                x.xfer_unsigned_short(&mut 0).unwrap();
            }
            x.xfer_unsigned_short(&mut 0).unwrap(); // team rows
            x.xfer_unsigned_short(&mut 1).unwrap(); // player rows
            x.xfer_int(&mut ((1 - id) as i32)).unwrap();
            x.xfer_int(&mut 0).unwrap(); // Enemies
            x.xfer_bool(&mut false).unwrap(); // can_build_units
            x.xfer_bool(&mut true).unwrap(); // can_build_base
            x.xfer_bool(&mut false).unwrap(); // observer
            x.xfer_real(&mut (1.5 + id as f32)).unwrap();
            x.xfer_bool(&mut true).unwrap(); // score list
            for flag in 0..16 {
                x.xfer_bool(&mut (flag == id)).unwrap();
            }
            x.xfer_int(&mut (21 + id as i32)).unwrap();
            x.xfer_bool(&mut false).unwrap(); // dead
            x.xfer_int(&mut (31 + id as i32)).unwrap();
            x.xfer_bool(&mut true).unwrap(); // radar disabled
            x.xfer_real(&mut (0.25 + id as f32)).unwrap();
            x.xfer_unsigned_short(&mut 0).unwrap(); // kind cost rows
            if version >= 2 {
                x.xfer_bool(&mut true).unwrap();
                x.xfer_unsigned_short(&mut 2).unwrap();
                x.xfer_unsigned_int(&mut 1).unwrap();
                x.xfer_unsigned_int(&mut 2).unwrap();
            }
            if version >= 3 {
                x.xfer_bool(&mut true).unwrap();
            }
            if version >= 4 {
                let a = alliances[id as usize];
                x.xfer_unsigned_byte(&mut u8::from(a.is_some())).unwrap();
                if let Some(mut value) = a {
                    x.xfer_int(&mut value).unwrap();
                }
            }
        }
    }
    cursor.into_inner()
}

#[test]
fn historical_players_rows_keep_their_own_layout_and_defaults() {
    let original = include_bytes!("fixtures/alliance_save_load/alliance_only.sav");
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    for version in 1..=4 {
        let payload = players_payload(version, [None, None]);
        std::fs::write(
            manager.get_save_path("legacy"),
            replace_chunk(original, "CHUNK_Players", Some(&payload)),
        )
        .unwrap();
        // The original fixture is genuine outer v23; only the sibling Players
        // layout varies here. Its explicit empty AI vector remains authoritative.
        let (snapshot, _) = manager.load_game_snapshot("legacy").unwrap();
        assert_eq!(snapshot.version, 23);
        assert!(snapshot.ai_players.is_empty());
        let mut decoded = world();
        for id in 0..2 {
            decoded.add_player(Player::new(id, Team::USA, "Codec", true));
        }
        let chunks =
            generals_main::save_load::stash_loaded_player_team_chunks(Some(&payload), None)
                .unwrap();
        generals_main::save_load::apply_pending_player_team_chunks(&mut decoded, &chunks).unwrap();
        for id in 0..2 {
            let p = decoded.get_player(id).unwrap();
            assert!(!p.can_build_units);
            assert_eq!(p.alliance_team, -1);
            assert_eq!(p.map_relationship(1 - id), Some(Relationship::Enemies));
            assert_eq!(p.units_should_hunt, version >= 2);
            assert_eq!(p.did_preorder, version >= 3);
            assert_eq!(
                p.selected_objects,
                if version >= 2 {
                    vec![ObjectId(1), ObjectId(2)]
                } else {
                    vec![]
                }
            );
        }
        drop(decoded);
        let mut restored = world();
        manager.load_game("legacy", &mut restored).unwrap();
        for id in 0..2 {
            let p = restored.get_player(id).unwrap();
            assert_eq!(
                p.alliance_team, -1,
                "v{version} native/legacy absence cannot invent host authoring"
            );
            assert_eq!(p.map_side.relations.len(), 1);
            assert_eq!(p.map_relationship(1 - id), Some(Relationship::Enemies));
            // C++ Player::xfer transfers canBuildUnits directly (Player.cpp:4293).
            // An explicitly empty AI roster must not overwrite the saved false
            // value with a constructor effect merely because a slot is nonlocal.
            assert!(!p.can_build_units);
            assert!(p.can_build_base && !p.is_observer && p.is_alive && p.radar_disabled);
            assert_eq!(p.skill_points_modifier, 1.5 + id as f32);
            assert_eq!(p.radar_count, 21 + id as i32);
            assert_eq!(p.disable_proof_radar_count, 31 + id as i32);
            assert_eq!(p.cash_bounty_percent, 0.25 + id as f32);
            assert_eq!(p.attacked_by.iter().filter(|&&v| v).count(), 1);
            assert!(p.attacked_by[id as usize]);
            assert_eq!(p.units_should_hunt, version >= 2);
            assert_eq!(
                p.selected_objects,
                if version >= 2 {
                    vec![ObjectId(1), ObjectId(2)]
                } else {
                    vec![]
                }
            );
            assert_eq!(p.did_preorder, version >= 3);
        }
    }
}

fn chunk_payload(bytes: &[u8], wanted: &str) -> Vec<u8> {
    let mut pos = 0;
    while pos < bytes.len() {
        let len = bytes[pos] as usize;
        pos += 1;
        let name = std::str::from_utf8(&bytes[pos..pos + len]).unwrap();
        pos += len;
        if name == "SG_EOF" {
            break;
        }
        let size = i32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if name == wanted {
            return bytes[pos..pos + size].to_vec();
        }
        pos += size;
    }
    panic!("missing fixture chunk {wanted}");
}

#[test]
fn world_versions_23_and_24_keep_nonempty_direct_and_bincode_alignment() {
    use generals_main::save_load::{Snapshot, WorldSnapshot, Xfer, XferLoad, XferSave};
    use std::io::Cursor;
    let original = include_bytes!("fixtures/alliance_save_load/explicit_map.sav");
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    std::fs::write(manager.get_save_path("original"), original).unwrap();
    let (mut snapshot, _) = manager.load_game_snapshot("original").unwrap();
    assert_eq!(snapshot.players.len(), 2);
    assert_eq!(snapshot.objects.len(), 2);
    for version in [23, 24] {
        snapshot.version = version;
        snapshot.frame_number = 123;
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut writer = XferSave::new(&mut bytes);
            snapshot.xfer(&mut writer).unwrap();
            writer.xfer_u32(&mut 0x71af91c0).unwrap();
        }
        let mut reader = XferLoad::new(Cursor::new(bytes.into_inner()));
        let mut loaded = WorldSnapshot::default();
        loaded.xfer(&mut reader).unwrap();
        let mut sentinel = 0;
        reader.xfer_u32(&mut sentinel).unwrap();
        assert_eq!(sentinel, 0x71af91c0);
        assert_eq!((loaded.version, loaded.frame_number), (version, 123));
        assert_eq!(
            serde_json::to_value(&loaded.players).unwrap(),
            serde_json::to_value(&snapshot.players).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&loaded.objects).unwrap(),
            serde_json::to_value(&snapshot.objects).unwrap()
        );
        let encoded = bincode_legacy::serialize(&snapshot).unwrap();
        let mut bundle = replace_chunk(original, "CHUNK_GameLogic", Some(&encoded));
        if version == 24 {
            bundle = replace_chunk(
                &bundle,
                "CHUNK_Players",
                Some(&players_payload(4, [Some(-1), Some(-1)])),
            );
        }
        std::fs::write(manager.get_save_path("version"), &bundle).unwrap();
        let (decoded, _) = manager.load_game_snapshot("version").unwrap();
        assert_eq!(decoded.version, version);
        // The named sibling chunks now belong to the returned snapshot. This
        // bundle deliberately changes Players, so expect the new capsule while
        // retaining an exact comparison of every other snapshot byte/field.
        let mut expected: WorldSnapshot = bincode_legacy::deserialize(&encoded).unwrap();
        let chunks = generals_main::save_load::stash_loaded_player_team_chunks(
            Some(&chunk_payload(&bundle, "CHUNK_Players")),
            Some(&chunk_payload(&bundle, "CHUNK_TeamFactory")),
        )
        .unwrap();
        let (players, teams) = encoded_player_team_chunks(&chunks);
        let bytes = bincode_legacy::serialize(&(
            chunks.players.as_ref().map(|_| players),
            chunks.teams.as_ref().map(|_| teams),
        ))
        .unwrap();
        assert!(expected.lifecycle_tail.ends_with(b"PTSC"));
        let old_len = u32::from_le_bytes(
            expected.lifecycle_tail
                [expected.lifecycle_tail.len() - 8..expected.lifecycle_tail.len() - 4]
                .try_into()
                .unwrap(),
        ) as usize;
        expected
            .lifecycle_tail
            .truncate(expected.lifecycle_tail.len() - 8 - old_len);
        let encoded: Vec<u8> = bytes
            .iter()
            .flat_map(|b| format!("{b:02x}").into_bytes())
            .collect();
        expected.lifecycle_tail.extend_from_slice(&encoded);
        expected
            .lifecycle_tail
            .extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        expected.lifecycle_tail.extend_from_slice(b"PTSC");
        assert_eq!(
            serde_json::to_value(&decoded).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
    for version in [0, 1, 22, 25, u32::MAX] {
        snapshot.version = version;
        let mut bytes = Cursor::new(Vec::new());
        assert!(snapshot.xfer(&mut XferSave::new(&mut bytes)).is_err());
        assert!(bytes.into_inner().is_empty());
        let input = [version.to_le_bytes(), 0x71af91c0u32.to_le_bytes()].concat();
        let mut reader = XferLoad::new(Cursor::new(input));
        assert!(WorldSnapshot::default().xfer(&mut reader).is_err());
        let mut sentinel = 0;
        reader.xfer_u32(&mut sentinel).unwrap();
        assert_eq!(sentinel, 0x71af91c0);
        let encoded = bincode_legacy::serialize(&snapshot).unwrap();
        std::fs::write(
            manager.get_save_path("rejected"),
            replace_chunk(original, "CHUNK_GameLogic", Some(&encoded)),
        )
        .unwrap();
        let mut host = world();
        assert!(manager.load_game("rejected", &mut host).is_err());
        assert!(host.get_players().is_empty() && host.host_objects().is_empty());
    }
    let mut body = chunk_payload(original, "CHUNK_GameLogic");
    body.push(0x5a);
    std::fs::write(
        manager.get_save_path("trailing"),
        replace_chunk(original, "CHUNK_GameLogic", Some(&body)),
    )
    .unwrap();
    assert!(manager.load_game_snapshot("trailing").is_err());
}

#[test]
fn new_file_bundle_requires_exact_host_alliance_coverage() {
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let mut source = world();
    for id in 0..2 {
        let mut p = Player::new(id, Team::USA, "Coverage", true);
        p.alliance_team = 41 + id as i32;
        source.add_player(p);
    }
    manager.save_game("seed", &source, &info()).unwrap();
    drop(source);
    let original = std::fs::read(manager.get_save_path("seed")).unwrap();
    assert_eq!(
        chunk_payload(&original, "CHUNK_GameLogic")[..4],
        24u32.to_le_bytes()
    );
    let all = players_payload(4, [Some(41), Some(42)]);
    let mut truncated = all.clone();
    truncated.pop();
    let mut invalid_tag = all.clone();
    let tag_index = invalid_tag.len() - 5;
    invalid_tag[tag_index] = 2;
    let mut trailing = all.clone();
    trailing.push(0);
    let mut duplicates = all.clone(); // equal-size authored rows; make row1 use row0's id
    let row_length = (duplicates.len() - 3) / 2;
    duplicates[3 + row_length..7 + row_length].copy_from_slice(&0u32.to_le_bytes());
    let cases = [
        ("missing", None),
        ("null", Some(vec![1])),
        ("old_rows", Some(players_payload(3, [None, None]))),
        ("absent_host", Some(players_payload(4, [Some(41), None]))),
        ("empty_rows", Some(vec![4, 0, 0])),
        ("duplicate_host", Some(duplicates)),
        ("truncated", Some(truncated)),
        ("invalid_tag", Some(invalid_tag)),
        ("trailing", Some(trailing)),
        ("version_zero", Some(vec![0, 0, 0])),
        ("future", Some(vec![5, 0, 0])),
    ];
    let retained =
        generals_main::save_load::stash_loaded_player_team_chunks(Some(&all), None).unwrap();
    let retained_bytes = encoded_player_team_chunks(&retained);
    for (name, payload) in cases {
        let _retained_seed = manager.load_game_snapshot("seed").unwrap();
        std::fs::write(
            manager.get_save_path(name),
            replace_chunk(&original, "CHUNK_Players", payload.as_deref()),
        )
        .unwrap();
        let mut restored = world();
        let result = manager.load_game(name, &mut restored);
        println!("v24 coverage {name}: {result:?}");
        assert!(result.is_err(), "{name}");
        assert!(restored.get_players().is_empty() && restored.host_objects().is_empty());
        // A failed candidate cannot modify a separately retained capsule.
        restored.add_player(Player::new(0, Team::USA, "Fresh", true));
        assert_eq!(restored.get_player(0).unwrap().alliance_team, -1, "{name}");
        assert_eq!(
            encoded_player_team_chunks(&retained),
            retained_bytes,
            "{name}"
        );
        generals_main::save_load::apply_pending_player_team_chunks(&mut restored, &retained)
            .unwrap();
        assert_eq!(restored.get_player(0).unwrap().alliance_team, 41, "{name}");
    }
    // Native-only rows do not require a host value in an empty host world.
    let empty = world();
    manager.save_game("empty", &empty, &info()).unwrap();
    drop(empty);
    let bytes = std::fs::read(manager.get_save_path("empty")).unwrap();
    for payload in [None, Some(vec![1]), Some(players_payload(4, [None, None]))] {
        std::fs::write(
            manager.get_save_path("empty_control"),
            replace_chunk(&bytes, "CHUNK_Players", payload.as_deref()),
        )
        .unwrap();
        assert!(manager.load_game_snapshot("empty_control").is_ok());
    }
}

// Capture returns an owned value; observing a live world cannot replace any
// independently decoded candidate waiting to be applied.
fn captured_team_next_id(host: &GameLogic) -> u32 {
    use game_engine::common::system::xfer_save::XferSave as CommonSave;
    let chunks = generals_main::save_load::stamp_player_team_chunks(host).unwrap();
    let mut cursor = std::io::Cursor::new(Vec::new());
    generals_main::save_load::write_team_factory_block(
        &mut CommonSave::new(&mut cursor, 1),
        &chunks,
    )
    .unwrap();
    let bytes = cursor.into_inner();
    assert_eq!(bytes[0], 5);
    u32::from_le_bytes(bytes[1..5].try_into().unwrap())
}

fn encoded_player_team_chunks(
    chunks: &generals_main::save_load::PlayerTeamChunks,
) -> (Vec<u8>, Vec<u8>) {
    use game_engine::common::system::xfer_save::XferSave as CommonSave;
    let mut players = std::io::Cursor::new(Vec::new());
    let mut teams = std::io::Cursor::new(Vec::new());
    generals_main::save_load::write_players_block(&mut CommonSave::new(&mut players, 1), chunks)
        .unwrap();
    generals_main::save_load::write_team_factory_block(&mut CommonSave::new(&mut teams, 1), chunks)
        .unwrap();
    (players.into_inner(), teams.into_inner())
}

#[test]
fn player_team_pair_does_not_publish_one_successful_half() {
    use generals_main::save_load::{
        apply_pending_player_team_chunks, stash_loaded_player_team_chunks,
    };
    let players = players_payload(4, [Some(71), Some(72)]);
    // Valid historical empty TeamFactory4; new writers use version5.
    let teams = vec![4, 77, 0, 0, 0, 0, 0, 0, 0];
    let retained = stash_loaded_player_team_chunks(Some(&players), Some(&teams)).unwrap();
    let encoded = encoded_player_team_chunks(&retained);
    let verify_empty_pair = |chunks: &generals_main::save_load::PlayerTeamChunks| {
        let mut host = world();
        host.add_player(Player::new(0, Team::USA, "Pair", true));
        let next = 1;
        assert_ne!(next, 77);
        apply_pending_player_team_chunks(&mut host, chunks).unwrap();
        assert_eq!(host.get_player(0).unwrap().alliance_team, -1);
        assert_eq!(captured_team_next_id(&host), next);
    };
    for bad in [vec![], vec![0], vec![255], vec![4, 1], {
        let mut t = teams.clone();
        t.push(0);
        t
    }] {
        assert!(stash_loaded_player_team_chunks(Some(&players), Some(&bad)).is_err());
        assert!(stash_loaded_player_team_chunks(Some(&bad), Some(&teams)).is_err());
        // No partially decoded value escapes either failure. A valid retained
        // pair stays intact and a fresh world remains completely unmodified.
        assert_eq!(encoded_player_team_chunks(&retained), encoded);
        verify_empty_pair(&stash_loaded_player_team_chunks(None, None).unwrap());
        let mut host = world();
        host.add_player(Player::new(0, Team::USA, "Pair", true));
        apply_pending_player_team_chunks(&mut host, &retained).unwrap();
        assert_eq!(host.get_player(0).unwrap().alliance_team, 71);
        assert_eq!(captured_team_next_id(&host), 77);
    }
    for absent in [None, Some(&[1][..])] {
        let empty = stash_loaded_player_team_chunks(absent, absent).unwrap();
        verify_empty_pair(&empty);
        // Absence belongs to this capsule only; its valid sibling still applies.
        let chunks = stash_loaded_player_team_chunks(Some(&players), absent).unwrap();
        let mut host = world();
        host.add_player(Player::new(0, Team::USA, "Pair", true));
        let next = 1;
        apply_pending_player_team_chunks(&mut host, &chunks).unwrap();
        assert_eq!(host.get_player(0).unwrap().alliance_team, 71);
        assert_eq!(captured_team_next_id(&host), next);
        drop(host);
        let chunks = stash_loaded_player_team_chunks(absent, Some(&teams)).unwrap();
        let mut host = world();
        host.add_player(Player::new(0, Team::USA, "Pair", true));
        apply_pending_player_team_chunks(&mut host, &chunks).unwrap();
        assert_eq!(host.get_player(0).unwrap().alliance_team, -1);
        assert_eq!(captured_team_next_id(&host), 77);
        assert_eq!(encoded_player_team_chunks(&retained), encoded);
    }
}

#[test]
fn mission_only_file_does_not_require_a_players_chunk_or_restore_host() {
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let mut source = world();
    let mut p = Player::new(0, Team::USA, "Mission", true);
    p.alliance_team = 15;
    source.add_player(p);
    let mut save_info = info();
    save_info.save_type = SaveFileType::Mission;
    manager.save_game("mission", &source, &save_info).unwrap();
    drop(source);
    let bytes = std::fs::read(manager.get_save_path("mission")).unwrap();
    assert!(
        !bytes
            .windows("CHUNK_Players".len())
            .any(|w| w == b"CHUNK_Players")
    );
    let mut restored = world();
    manager.load_game("mission", &mut restored).unwrap();
    assert!(restored.get_players().is_empty() && restored.host_objects().is_empty());
}

#[test]
fn explicit_unassigned_value_is_distinct_from_absent_host_authority() {
    let mut host = world();
    for id in 0..2 {
        let mut p = Player::new(id, Team::USA, "Authority", true);
        p.alliance_team = 91 + id as i32;
        host.add_player(p);
    }
    let chunks = generals_main::save_load::stash_loaded_player_team_chunks(
        Some(&players_payload(4, [Some(-1), None])),
        None,
    )
    .unwrap();
    generals_main::save_load::apply_pending_player_team_chunks(&mut host, &chunks).unwrap();
    assert_eq!(host.get_player(0).unwrap().alliance_team, -1);
    assert_eq!(host.get_player(1).unwrap().alliance_team, 92);
}

#[test]
fn file_open_or_container_failure_preserves_retained_owned_pair() {
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    std::fs::write(manager.get_save_path("invalid"), b"invalid container").unwrap();
    let players = players_payload(4, [Some(71), Some(72)]);
    let teams = vec![4, 77, 0, 0, 0, 0, 0, 0, 0];
    for name in ["missing", "invalid"] {
        let retained =
            generals_main::save_load::stash_loaded_player_team_chunks(Some(&players), Some(&teams))
                .unwrap();
        let encoded = encoded_player_team_chunks(&retained);
        assert!(manager.load_game_snapshot(name).is_err());
        assert_eq!(encoded_player_team_chunks(&retained), encoded);
        let mut host = world();
        host.add_player(Player::new(0, Team::USA, "Fresh", true));
        assert_eq!(host.get_player(0).unwrap().alliance_team, -1);
        assert_eq!(captured_team_next_id(&host), 1);
        generals_main::save_load::apply_pending_player_team_chunks(&mut host, &retained).unwrap();
        assert_eq!(host.get_player(0).unwrap().alliance_team, 71);
        assert_eq!(captured_team_next_id(&host), 77);
    }
}

fn capsule_host() -> GameLogic {
    let mut host = world();
    for id in 0..2 {
        host.add_player(Player::new(id, Team::USA, "Capsule", id == 0));
    }
    host
}

fn assert_capsule_host(host: &GameLogic, alliances: [i32; 2], next_team: u32) {
    for id in 0..2 {
        assert_eq!(
            host.get_player(id).unwrap().alliance_team,
            alliances[id as usize]
        );
    }
    assert_eq!(captured_team_next_id(host), next_team);
}

#[test]
fn retained_player_team_capsules_are_independent_of_decode_capture_and_other_worlds() {
    use generals_main::save_load::{
        apply_pending_player_team_chunks, stash_loaded_player_team_chunks,
    };
    let players_a = players_payload(4, [Some(71), Some(72)]);
    let players_b = players_payload(4, [Some(81), Some(82)]);
    let teams_a = vec![4, 77, 0, 0, 0, 0, 0, 0, 0];
    let teams_b = vec![4, 99, 0, 0, 0, 0, 0, 0, 0];
    let a = stash_loaded_player_team_chunks(Some(&players_a), Some(&teams_a)).unwrap();
    let encoded_a = encoded_player_team_chunks(&a);
    let b = stash_loaded_player_team_chunks(Some(&players_b), Some(&teams_b)).unwrap();
    let encoded_b = encoded_player_team_chunks(&b);
    assert_ne!(encoded_a, encoded_b);
    // Invalid and absent decodes cannot overwrite an already retained candidate.
    assert!(stash_loaded_player_team_chunks(Some(&players_a), Some(&[4, 1])).is_err());
    assert!(stash_loaded_player_team_chunks(Some(&[4, 1]), Some(&teams_b)).is_err());
    let absent = stash_loaded_player_team_chunks(None, None).unwrap();
    for reverse in [false, true] {
        let mut first = capsule_host();
        let mut second = capsule_host();
        assert_capsule_host(&first, [-1, -1], 1);
        assert_capsule_host(&second, [-1, -1], 1);
        // Capturing another live world and writing B cannot replace A.
        let _other_capture = generals_main::save_load::stamp_player_team_chunks(&second).unwrap();
        assert_eq!(encoded_player_team_chunks(&b), encoded_b);
        if reverse {
            apply_pending_player_team_chunks(&mut second, &b).unwrap();
            apply_pending_player_team_chunks(&mut first, &a).unwrap();
        } else {
            apply_pending_player_team_chunks(&mut first, &a).unwrap();
            apply_pending_player_team_chunks(&mut second, &b).unwrap();
        }
        assert_capsule_host(&first, [71, 72], 77);
        assert_capsule_host(&second, [81, 82], 99);
        for id in 0..2 {
            assert_eq!(
                first.get_player(id).unwrap().map_relationship(1 - id),
                Some(Relationship::Enemies)
            );
            assert_eq!(
                second.get_player(id).unwrap().map_relationship(1 - id),
                Some(Relationship::Enemies)
            );
        }
        second.reset();
        apply_pending_player_team_chunks(&mut second, &absent).unwrap();
        drop(second);
        assert_capsule_host(&first, [71, 72], 77);
        assert_eq!(encoded_player_team_chunks(&a), encoded_a);
        assert_eq!(encoded_player_team_chunks(&b), encoded_b);
        // Explicit capsules are reusable observations, not consumed global slots.
        apply_pending_player_team_chunks(&mut first, &a).unwrap();
        assert_capsule_host(&first, [71, 72], 77);
    }
}

#[test]
fn retained_decoded_files_restore_their_own_players_and_teams_in_either_order() {
    let directory = TempDir::new().unwrap();
    let mut manager = SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    for (name, alliances, next_team) in [("retained_a", [71, 72], 77), ("retained_b", [81, 82], 99)]
    {
        let mut source = capsule_host();
        for id in 0..2 {
            let player = source.get_player_mut(id).unwrap();
            player.alliance_team = alliances[id as usize];
            player.set_map_relationship(1 - id, Relationship::Enemies);
        }
        let mut historical_teams = vec![4];
        historical_teams.extend_from_slice(&u32::to_le_bytes(next_team));
        historical_teams.extend_from_slice(&[0, 0, 0, 0]);
        let teams = generals_main::save_load::stash_loaded_player_team_chunks(
            None,
            Some(&historical_teams),
        )
        .unwrap();
        generals_main::save_load::apply_pending_player_team_chunks(&mut source, &teams).unwrap();
        manager.save_game(name, &source, &info()).unwrap();
        assert_capsule_host(&source, alliances, next_team);
    }
    // Both public decodes complete before either candidate is restored. This
    // OLD-compatible sequence exposes B replacing A's sibling chunk metadata.
    let (a, _) = manager.load_game_snapshot("retained_a").unwrap();
    let (b, _) = manager.load_game_snapshot("retained_b").unwrap();
    assert!(
        manager
            .load_game_snapshot("missing_retained_candidate")
            .is_err()
    );
    let builder = generals_main::save_load::SnapshotBuilder::new();
    for reverse in [false, true] {
        let mut first = world();
        let mut second = world();
        if reverse {
            builder.restore_from_snapshot(&b, &mut second).unwrap();
            builder.restore_from_snapshot(&a, &mut first).unwrap();
        } else {
            builder.restore_from_snapshot(&a, &mut first).unwrap();
            builder.restore_from_snapshot(&b, &mut second).unwrap();
        }
        assert_capsule_host(&first, [71, 72], 77);
        assert_capsule_host(&second, [81, 82], 99);
        for id in 0..2 {
            assert_eq!(
                first.get_player(id).unwrap().map_relationship(1 - id),
                Some(Relationship::Enemies)
            );
            assert_eq!(
                second.get_player(id).unwrap().map_relationship(1 - id),
                Some(Relationship::Enemies)
            );
        }
        second.reset();
        drop(second);
        assert_capsule_host(&first, [71, 72], 77);
    }
}

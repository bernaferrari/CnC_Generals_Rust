//! Real-file Strategy continuation across fresh processes. The writer alone
//! admits objects/commands; the reader preloads the same authored catalogs and
//! then calls SaveFileManager::load_game followed only by ordinary updates.
//! C++ references and the bounded host fixture are in strategy_center_aim.rs.
//! This is Rust continuation evidence, not C++ save-byte compatibility.
use game_client::effects::particle_manager::{
    ParticleSystemManager, get_particle_system_manager, get_particle_system_manager_mut,
};
use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use gamelogic::common::Relationship;
use gamelogic::object::armor::{TheArmorStore, load_armor_templates_from_str};
use gamelogic::weapon::{with_weapon_store, with_weapon_store_mut};
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::host_strategy_center::{HostBattlePlan, HostBattlePlanTransition};
use generals_main::game_logic::object::TurretSubState;
use generals_main::game_logic::{
    AIState, AttackSubState, GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate,
};
use generals_main::save_load::{
    GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo, SnapshotBuilder,
};
use glam::Vec3;
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

const CENTER: &str = "AmericaStrategyCenter";
const GROUND: &str = "StrategySaveGroundTarget";
const ANCHOR: &str = "StrategySaveEnemyAnchor";
// No object references this entry. Its public parser admission also confines
// an incidental leftover ActiveBody constructor to this exact in-memory catalog.
const ARMOR_INI: &str = "Armor StrategySaveUnusedArmor\nArmor = DEFAULT 100%\nEnd\n";
const LOCOMOTOR_PATH: &str = "windows_game/extracted_big_files/INIZH/Data/INI/Locomotor.ini";
// Exact fields produced for these two names by seed_known_host_locomotors.
// They are the only locomotors referenced by the restore-required GLA catalog.
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
const CHILD_TEST: &str = "strategy_save_process";

#[derive(Clone, Copy, Debug)]
struct Case {
    name: &'static str,
    explicit: bool,
    checkpoint: u32,
    end: u32,
}

const CASES: [Case; 5] = [
    Case {
        name: "automatic_acquired",
        explicit: false,
        checkpoint: 212,
        end: 480,
    },
    Case {
        name: "automatic_mid_aim",
        explicit: false,
        checkpoint: 233,
        end: 480,
    },
    Case {
        name: "automatic_cooldown",
        explicit: false,
        checkpoint: 257,
        end: 480,
    },
    Case {
        name: "explicit_mid_aim",
        explicit: true,
        checkpoint: 233,
        end: 280,
    },
    Case {
        name: "explicit_pending",
        explicit: true,
        checkpoint: 256,
        end: 280,
    },
];

fn catalog_world() -> GameLogic {
    assert!(generals_main::assets::get_asset_manager().is_none());
    gamelogic::initialize_weapon_store().unwrap();
    with_weapon_store_mut(|store| {
        assert_eq!(store.get_template_count(), 0);
        // The three admitted scenario objects have no named weapons. Unused
        // GLA support definitions retain their normal names. Complete this
        // empty weapon catalog before any object construction.
        store.mark_host_bootstrap_complete();
    })
    .unwrap();
    assert_eq!(load_armor_templates_from_str(ARMOR_INI, None).unwrap(), 1);
    assert_eq!(fs::read_to_string(LOCOMOTOR_PATH).unwrap(), LOCOMOTOR_INI);
    assert!(get_locomotor_store().get_template_names().is_empty());
    // Verify the exact authored file parses before entering the bootstrap.
    // The bootstrap reads this same first candidate, stops after success,
    // fills its fixed in-memory seeds and marks its own admission complete.
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
    assert_eq!(
        logic.host_ai_player_count(),
        0,
        "definitions must not manufacture controllers"
    );
    let mut center = ThingTemplate::new(CENTER);
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Immobile)
        .set_health(1500.0);
    logic.templates.insert(CENTER.into(), center);
    for (name, kind) in [(GROUND, KindOf::Vehicle), (ANCHOR, KindOf::Structure)] {
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(kind)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_none();
        logic.templates.insert(name.into(), template);
    }
    assert_catalog(&logic);
    logic
}

fn assert_catalog(logic: &GameLogic) {
    assert!(generals_main::assets::get_asset_manager().is_none());
    assert!(
        game_engine::common::thing::thing_factory::try_get_thing_factory()
            .expect("fixture must not contend for the native template factory")
            .is_none()
    );
    with_weapon_store(|store| {
        assert!(store.host_bootstrap_is_complete());
        assert_eq!(
            store.get_template_count(),
            0,
            "unnamed fixture gained weapon definitions"
        );
    })
    .unwrap();
    assert_eq!(TheArmorStore::read().len(), 1);
    let mut expected: Vec<_> = GLA_SUPPORT
        .into_iter()
        .chain([CENTER, GROUND, ANCHOR])
        .collect();
    expected.sort_unstable();
    let mut names: Vec<_> = logic.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, expected);
    for name in [CENTER, GROUND, ANCHOR] {
        let t = &logic.templates[name];
        assert!(t.primary_weapon_name.is_none());
        assert!(t.secondary_weapon_name.is_none());
        assert!(t.tertiary_weapon_name.is_none());
        assert!(t.locomotor_name.is_none() && t.locomotor_set_names.is_empty());
        assert!(t.authored_locomotor_sets.is_none() && t.armor_sets.is_empty());
    }
    assert_eq!(fs::read_to_string(LOCOMOTOR_PATH).unwrap(), LOCOMOTOR_INI);
    let manager = get_particle_system_manager().unwrap();
    for name in ["MuzzleFlash", "BulletImpact"] {
        assert!(manager.as_ref().unwrap().find_template(name).is_some());
    }
}

fn template_value(template: &ThingTemplate) -> Value {
    let mut value = serde_json::to_value(template).unwrap();
    // HashSet iteration order is not a template property.
    value["kind_of"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(Value::to_string);
    value
}

fn inputs(logic: &GameLogic, case: Case) -> Value {
    let mut names: Vec<_> = logic.templates.keys().map(String::as_str).collect();
    names.sort_unstable();
    let templates: Vec<_> = names
        .into_iter()
        .map(|name| template_value(&logic.templates[name]))
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
    json!({
        "case":case.name,"explicit":case.explicit,
        "checkpoint_next_frame":case.checkpoint,"end_next_frame":case.end,
        "templates":templates,"weapon_catalog":[],"armor_ini":ARMOR_INI,
        "armor_ini_crc32":crc32fast::hash(ARMOR_INI.as_bytes()),
        "locomotor_input":{"relative_path":LOCOMOTOR_PATH,"ini":LOCOMOTOR_INI,
            "parser_count":2,"crc32":crc32fast::hash(LOCOMOTOR_INI.as_bytes())},
        "locomotor_catalog":locomotors,"particle_catalog":particles,
        "source_crc32":crc32fast::hash(include_bytes!("strategy_center_save_continuation.rs")),
        "players":[{"id":0,"team":"USA","alliance":31},{"id":1,"team":"GLA","alliance":45}],
        "positions":{"center":[0.0,0.0,0.0],"target":[150.0,0.0,0.0],"anchor":[2000.0,0.0,0.0]},
        "asset_manager":false,"scenario_named_weapon_bindings":false,"scenario_locomotor_bindings":false,
        "world_snapshot_version":generals_main::save_load::WORLD_SNAPSHOT_BINCODE_VERSION,
        "compiled_features":{"game_client":cfg!(feature="game_client"),"host_residuals":cfg!(feature="host-residuals"),
            "integration_tests":cfg!(feature="integration-tests"),"network":cfg!(feature="network")},
        "post_load_mutations":"ordinary GameLogic::update only"
    })
}

#[derive(Clone, Copy)]
struct Ids {
    center: ObjectId,
    target: ObjectId,
    anchor: ObjectId,
}

fn id_for_template(logic: &GameLogic, name: &str) -> ObjectId {
    let ids: Vec<_> = logic
        .host_objects()
        .iter()
        .filter(|(_, object)| object.get_template().name == name)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(
        ids.len(),
        1,
        "exactly one {name} must survive admission/load"
    );
    ids[0]
}

fn restored_ids(logic: &GameLogic) -> Ids {
    assert_eq!(logic.host_objects().len(), 3);
    Ids {
        center: id_for_template(logic, CENTER),
        target: id_for_template(logic, GROUND),
        anchor: id_for_template(logic, ANCHOR),
    }
}

fn admit(logic: &mut GameLogic, case: Case, journal: &mut fs::File) -> Ids {
    for (id, team, alliance) in [(0, Team::USA, 31), (1, Team::GLA, 45)] {
        let mut player = Player::new(id, team, "Strategy save fixture", id == 0);
        player.alliance_team = alliance;
        logic.add_player(player);
    }
    let center = logic
        .create_object_for_player(CENTER, 0, Vec3::ZERO)
        .unwrap();
    let anchor = logic
        .create_object_for_player(ANCHOR, 1, Vec3::new(2000.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.host_object(center).unwrap().weapon.is_none());
    assert!(!logic.host_object(center).unwrap().turret_enabled);
    assert!(logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(center)));
    for processed in 0..=210 {
        assert_eq!(logic.get_frame(), processed);
        logic.update();
        let object = logic.host_object(center).unwrap();
        journal_row(
            journal,
            &json!({"phase":"activation","processed_frame":processed,
            "next_frame":logic.get_frame(),"center":center.0,"anchor":anchor.0,
            "center_owner":object.owner_player_id,"weapon":object.weapon,
            "turret_enabled":object.turret_enabled,"plan":logic.battle_plans(),
            "players_alive":[logic.player_is_alive(0),logic.player_is_alive(1)]}),
        );
        assert_eq!(logic.get_frame(), processed + 1);
        assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
        assert!(object.target.is_none() && object.turret_target_id.is_none());
        assert_eq!(object.weapon_discharge_marker().sequence, 0);
        if processed < 210 {
            assert!(object.weapon.is_none() && !object.turret_enabled);
        }
    }
    let door = logic.battle_plans().door_state_for_center(center).unwrap();
    assert_eq!(door.status, HostBattlePlanTransition::Active);
    assert_eq!(door.door_plan, Some(HostBattlePlan::Bombardment));
    assert_eq!(logic.battle_plans().selection_count, 1);
    assert_eq!(logic.battle_plans().delayed_active_apply_count, 1);
    let target = logic
        .create_object_for_player(GROUND, 1, Vec3::new(150.0, 0.0, 0.0))
        .unwrap();
    let ids = Ids {
        center,
        target,
        anchor,
    };
    assert_admission(logic, ids);
    let object = logic.host_object(center).unwrap();
    assert!(object.target.is_none() && object.turret_target_id.is_none());
    assert_eq!(object.turret_angle_deg.to_bits(), (-90.0_f32).to_bits());
    assert_eq!(object.turret_pitch_deg.to_bits(), 45.0_f32.to_bits());
    if case.explicit {
        logic.queue_command(GameCommand {
            command_type: CommandType::AttackObject { target_id: target },
            player_id: 0,
            command_id: 1,
            timestamp: UNIX_EPOCH + Duration::from_secs(1),
            selected_units: vec![center],
            modifier_keys: ModifierKeys::default(),
        });
        logic.process_commands();
        let object = logic.host_object(center).unwrap();
        assert_eq!(object.ai_state, AIState::Attacking);
        assert_eq!(object.attack_substate, AttackSubState::AimAtTarget);
        assert_eq!(object.turret_substate, TurretSubState::Aim);
        assert_eq!(object.target, Some(target));
        assert_eq!(object.turret_target_id, Some(target));
        assert!(object.status.attacking && object.status.is_aiming_weapon);
        assert!(!object.turret_mood_target);
    }
    ids
}

fn assert_admission(logic: &GameLogic, ids: Ids) {
    assert_catalog(logic);
    assert_eq!(
        logic.host_objects().len(),
        3,
        "unexpected object creation is outside this bounded scenario"
    );
    assert_eq!(logic.get_players().len(), 2);
    assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
    assert_eq!(
        logic.battle_plans().active_plan_for_player(0),
        Some(HostBattlePlan::Bombardment)
    );
    for (id, name, owner, pos) in [
        (ids.center, CENTER, 0, Vec3::ZERO),
        (ids.target, GROUND, 1, Vec3::new(150.0, 0.0, 0.0)),
        (ids.anchor, ANCHOR, 1, Vec3::new(2000.0, 0.0, 0.0)),
    ] {
        let object = logic.host_object(id).unwrap();
        assert_eq!(object.owner_player_id, Some(owner));
        assert_eq!(object.get_position(), pos);
        assert!(object.is_alive() && object.is_constructed() && !object.is_disabled());
        assert_eq!(
            template_value(object.get_template()),
            template_value(&logic.templates[name])
        );
    }
    let center = logic.host_object(ids.center).unwrap();
    let weapon = center.weapon.as_ref().unwrap();
    assert!(center.turret_enabled && center.can_attack());
    assert_eq!(center.weapon_name_for_slot(0), None);
    assert_eq!(
        (
            weapon.damage,
            weapon.min_range,
            weapon.range,
            weapon.reload_time
        ),
        (200.0, 100.0, 400.0, 7.0)
    );
    assert_eq!(
        (
            weapon.projectile_speed,
            weapon.pre_attack_delay,
            weapon.ammo
        ),
        (150.0, 0.0, None)
    );
    assert_eq!(center.selected_weapon_slot(), Some(0));
    assert!(center.secondary_weapon.is_none() && center.tertiary_weapon.is_none());
    assert!(!logic.skirmish_ai_auto_engage_paused(ids.center));
    assert!(logic.host_object(ids.target).unwrap().weapon.is_none());
    let anchor = logic.host_object(ids.anchor).unwrap();
    assert!(anchor.weapon.is_none());
    assert_eq!(anchor.health.current, 1000.0);
}

fn float(value: f32) -> Value {
    json!({"value":value,"bits":value.to_bits()})
}

fn frame_time(frame: u32) -> f32 {
    // Main's private LOGIC_FRAME_TIMESTEP is f32(1/30); its fire paths
    // multiply the frame by that value. Division rounds differently.
    frame as f32 * (1.0_f32 / 30.0)
}

fn observe(logic: &GameLogic, ids: Ids, processed: Option<u32>) -> Value {
    let center = logic.host_object(ids.center).unwrap();
    let target = logic.host_object(ids.target).unwrap();
    let marker = center.weapon_discharge_marker();
    let next = logic.get_frame();
    let players: Vec<_> = [0, 1].into_iter().map(|id| {
        let p = logic.get_player(id).unwrap();
        let mut relations: Vec<_> = p.map_side.relations.iter()
            .map(|(other, relation)| (*other, format!("{relation:?}"))).collect();
        relations.sort();
        json!({"id":id,"alliance":p.alliance_team,"relations":relations,"alive":logic.player_is_alive(id)})
    }).collect();
    let weapons: Vec<_> = (0..3).map(|slot| center.weapon_slot(slot).map(|w| {
        json!({"slot":slot,"name":center.weapon_name_for_slot(slot),"fields":w,
            "float_bits":[w.damage.to_bits(),w.range.to_bits(),w.min_range.to_bits(),w.reload_time.to_bits(),
                w.last_fire_time.to_bits(),w.clip_reload_time.to_bits(),w.projectile_speed.to_bits(),
                w.pre_attack_delay.to_bits(),w.splash_radius.to_bits(),w.last_bonus_rof.to_bits()],
            "suspend_fx_frame":w.suspend_fx_frame,"ready_at_next_frame":Object::weapon_ready(w,frame_time(next))})
    })).collect();
    // Production capture observes pending queues without a test setter/drain.
    // It also refreshes save companion slots; it is not globally side-effect
    // free. No captured snapshot is used as the reader's input here.
    let snapshot = SnapshotBuilder::new().create_world_snapshot(logic).unwrap();
    let pending = serde_json::to_value(&snapshot.pending_combat).unwrap();
    let mut projectiles = logic.combat_system().projectiles_snapshot();
    projectiles.sort_by_key(|p| p.id);
    json!({
        "processed_frame":processed,"next_frame":next,"simulation_time_next_frame":float(frame_time(next)),
        "persisted":{
            "roster":roster(logic),
            "players":players,"ids":[ids.center.0,ids.target.0,ids.anchor.0],
            "owners":[center.owner_player_id,target.owner_player_id],
            "teams":[format!("{:?}",center.team),format!("{:?}",target.team)],
            "positions":[center.get_position().to_array(),target.get_position().to_array(),
                logic.host_object(ids.anchor).unwrap().get_position().to_array()],
            "relationship":format!("{:?}",logic.object_relationship(center,target)),
            "templates":[center.get_template().name,target.get_template().name,ANCHOR],
            "plan":logic.battle_plans(),"host_target":center.target.map(|id|id.0),
            "ai_state":format!("{:?}",center.ai_state),"attacking":center.status.attacking,
            "body_orientation":float(center.get_orientation()),
            "turret":{
                "goal":center.turret_target_id.map(|id|id.0),"mood":center.turret_mood_target,
                "force":center.turret_force_attacking,"state":format!("{:?}",center.turret_substate),
                "enabled":center.turret_enabled,"rotating":center.turret_rotating,
                "yaw":float(center.turret_angle_deg),"pitch":float(center.turret_pitch_deg),
                "natural_yaw":float(center.turret_natural_angle_deg),"natural_pitch":float(center.turret_natural_pitch_deg),
                "turn_rate":float(center.turret_turn_rate_rad),"recenter_frames":center.turret_recenter_frames,
                "idle_next":center.turret_idle_scan_next_frame,"idle_scanning":center.turret_idle_scanning,
                "idle_desired":float(center.turret_idle_scan_desired_angle_deg),"idle_index":center.turret_idle_scan_index,
                "holding":center.turret_holding,"hold_until":center.turret_hold_until_frame,
                "recentering":center.turret_idle_recentering
            },
            "weapons":weapons,"selected_slot":center.selected_weapon_slot(),
            "lock_type":format!("{:?}",center.weapon_lock_type),"lock_slot":center.weapon_lock_slot,
            "barrel_configuration":center.weapon_barrel_states.iter().map(|b|json!({
                "shots_per_barrel":b.shots_per_barrel,"barrel_count":b.barrel_count})).collect::<Vec<_>>(),
            "saved_barrel_cursors":(0..3).map(|slot|center.weapon_barrel_cursor_for_snapshot(slot)).collect::<Vec<_>>(),
            "pre_attack_target":center.pre_attack_target.map(|id|id.0),
            "pre_attack_ready_at":float(center.pre_attack_ready_at),
            "consecutive_shot_target":center.consecutive_shot_target.map(|id|id.0),
            "max_shots":center.max_shots_to_fire,"last_fire_frame":center.last_fire_frame,
            "scatter_unused":center.weapon_scatter_targets_unused,"scatter_inited":center.weapon_scatter_targets_inited
        },
        "body_observations":{
            "attack_substate":format!("{:?}",center.attack_substate),
            "aiming":center.status.is_aiming_weapon,"firing":center.status.is_firing_weapon,
            "consecutive_shots_at_target":center.consecutive_shots_at_target,
            "weapon_fire_status":format!("{:?}",center.weapon_fire_status),
            "fire_intent_count":center.fire_intent_count,
            "last_fire_victim":center.last_fire_victim_host,"last_fire_slot":center.last_fire_slot,
            "last_fire_damage":float(center.last_fire_damage),"last_fire_range":float(center.last_fire_range),
            "last_fire_sim_time":float(center.last_fire_sim_time)
        },
        "runtime_observations":{
            // The restore stages raw cursor values until draw topology exists.
            // Keep the representation difference visible without mistaking it
            // for loss of the strict saved_barrel_cursors above.
            "raw_barrel_cursors":center.weapon_barrel_states.iter().map(|b|json!({
                "current_barrel":b.current_barrel,"shots_left_on_barrel":b.shots_left_on_barrel})).collect::<Vec<_>>(),
            "host_ai_player_count":logic.host_ai_player_count(),
            "controllers":([0,1].map(|id|json!({"player_id":id,"active":logic.is_host_ai_active(id),
                "difficulty":format!("{:?}",logic.host_ai_difficulty(id))})))
        },
        "gameplay":{
            "sequence":marker.sequence,"slot":marker.weapon_slot,"barrel":marker.fired_barrel,
            "marker_frame":marker.logic_frame,"next_sequence":logic.weapon_discharge_next_sequence_for_snapshot(),
            "center_hp":float(center.health.current),"target_hp":float(target.health.current),
            "pending":pending,"projectiles":projectiles
        }
    })
}

fn assert_checkpoint(logic: &GameLogic, ids: Ids, case: Case, observation: &Value) {
    assert_eq!(logic.get_frame(), case.checkpoint);
    let center = logic.host_object(ids.center).unwrap();
    let target = logic.host_object(ids.target).unwrap();
    assert_eq!(center.target, Some(ids.target));
    assert_eq!(center.turret_target_id, Some(ids.target));
    assert_eq!(center.turret_mood_target, !case.explicit);
    let sequence = center.weapon_discharge_marker().sequence;
    match case.name {
        "automatic_acquired" => {
            assert_eq!(center.turret_substate, TurretSubState::Aim);
            assert_eq!(center.turret_angle_deg.to_bits(), (-90.0_f32).to_bits());
            assert_eq!(center.turret_pitch_deg.to_bits(), 45.0_f32.to_bits());
            assert_eq!((sequence, target.health.current), (0, 1000.0));
        }
        "automatic_mid_aim" | "explicit_mid_aim" => {
            assert_eq!(center.turret_substate, TurretSubState::Aim);
            assert!(center.turret_angle_deg > -90.0 && center.turret_angle_deg < 0.0);
            assert_eq!((sequence, target.health.current), (0, 1000.0));
        }
        "automatic_cooldown" => {
            assert_eq!(sequence, 1);
            assert!(target.health.current < 1000.0);
            assert_eq!(center.weapon_discharge_marker().logic_frame, 256);
            assert!(!Object::weapon_ready(
                center.weapon.as_ref().unwrap(),
                frame_time(case.checkpoint)
            ));
        }
        "explicit_pending" => {
            assert_eq!((sequence, target.health.current), (1, 1000.0));
            assert_eq!(center.weapon_discharge_marker().logic_frame, 255);
            let pending = &observation["gameplay"]["pending"];
            let accepted = pending["accepted"].as_array().unwrap().len();
            let delayed = pending["delayed"].as_array().unwrap().len();
            assert_eq!(
                accepted + delayed,
                1,
                "capture the actual pending shot, not a substituted queue: {observation}"
            );
            assert!(
                observation["gameplay"]["projectiles"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                "this bounded witness does not cover materialized flight"
            );
        }
        _ => unreachable!(),
    }
}

fn info() -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: "strategy".into(),
        display_name: "Strategy continuation".into(),
        description: String::new(),
        map_name: "StrategySaveFixture".into(),
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

fn journal_row(journal: &mut fs::File, value: &Value) {
    serde_json::to_writer(&mut *journal, value).unwrap();
    journal.write_all(b"\n").unwrap();
    journal.flush().unwrap();
}

fn record(
    logic: &GameLogic,
    ids: Ids,
    processed: Option<u32>,
    phase: &str,
    journal: &mut fs::File,
) -> Value {
    let row = observe(logic, ids, processed);
    journal_row(journal, &json!({"phase":phase,"observation":row}));
    // Preserve the observation before validating it. A restore divergence is
    // evidence, even when it also violates the fixture's admission contract.
    assert_admission(logic, ids);
    let pending = &row["gameplay"]["pending"];
    for shot in pending["accepted"].as_array().unwrap().iter().chain(
        pending["delayed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| &entry["pending"]),
    ) {
        for name in [
            "projectile_object_name",
            "fire_fx_name",
            "fire_ocl_name",
            "detonation_fx_name",
            "detonation_ocl_name",
            "exhaust_name",
        ] {
            assert_eq!(
                shot[name], "",
                "unnamed fixture gained an external effect binding"
            );
        }
    }
    row
}

fn roster(logic: &GameLogic) -> Value {
    let mut objects: Vec<_> = logic
        .host_objects()
        .iter()
        .map(|(id, object)| {
            json!({"id":id.0,"template":object.get_template().name,
            "owner":object.owner_player_id,"position":object.get_position(),
            "health":object.health,"target":object.target,"turret_target":object.turret_target_id})
        })
        .collect();
    objects.sort_by_key(|object| object["id"].as_u64().unwrap());
    json!({"next_frame":logic.get_frame(),"objects":objects,"player_count":logic.get_players().len()})
}

#[test]
#[ignore = "child entry point; run through strategy_save_continues_in_fresh_processes"]
fn strategy_save_process() {
    let role = std::env::var("STRATEGY_SAVE_ROLE").expect("child role required");
    assert!(role == "writer" || role == "reader");
    let name = std::env::var("STRATEGY_SAVE_CASE").unwrap();
    let case = *CASES.iter().find(|case| case.name == name).unwrap();
    let dir = std::env::current_dir().unwrap();
    let mut journal = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{role}-observations.jsonl")))
        .unwrap();
    let mut logic = catalog_world();
    let fixture_inputs = inputs(&logic, case);
    write_json(&dir.join(format!("{role}-inputs.json")), &fixture_inputs);
    write_json(
        &dir.join(format!("{role}-catalog-admission.json")),
        &json!({
            "host_ai_player_count":logic.host_ai_player_count(),"objects":logic.host_objects().len(),
            "players":logic.get_players().len(),"locomotor_file_parse_count":2,
            "particle_system_count":get_particle_system_manager().unwrap().as_ref().unwrap().active_system_count()
        }),
    );
    let mut manager = SaveFileManager::with_save_directory(dir.join("saves"));
    manager.init().unwrap();
    let mut prefix = Vec::new();
    let ids = if role == "writer" {
        let ids = admit(&mut logic, case, &mut journal);
        prefix.push(record(&logic, ids, None, "source_prefix", &mut journal));
        while logic.get_frame() < case.checkpoint {
            let processed = logic.get_frame();
            logic.update();
            prefix.push(record(
                &logic,
                ids,
                Some(processed),
                "source_prefix",
                &mut journal,
            ));
        }
        let checkpoint = record(
            &logic,
            ids,
            Some(case.checkpoint - 1),
            "checkpoint",
            &mut journal,
        );
        write_json(&dir.join("writer-checkpoint.json"), &checkpoint);
        write_json(&dir.join("writer-prefix.json"), &json!(prefix));
        assert_checkpoint(&logic, ids, case, &checkpoint);
        manager.save_game("strategy", &logic, &info()).unwrap();
        assert_eq!(
            record(
                &logic,
                ids,
                Some(case.checkpoint - 1),
                "after_save",
                &mut journal
            ),
            checkpoint,
            "save must not alter the uninterrupted writer"
        );
        ids
    } else {
        // No source-world state or trace is read by this process. All IDs are
        // discovered from the restored roster, not imposed by the driver.
        assert!(logic.host_objects().is_empty() && logic.get_players().is_empty());
        let loaded = manager.load_game("strategy", &mut logic);
        write_json(
            &dir.join("reader-load.json"),
            &json!({"result":format!("{loaded:?}"),"roster":roster(&logic)}),
        );
        loaded.unwrap();
        restored_ids(&logic)
    };
    let mut trace = vec![record(
        &logic,
        ids,
        Some(case.checkpoint - 1),
        "continuation_checkpoint",
        &mut journal,
    )];
    assert_eq!(inputs(&logic, case), fixture_inputs);
    assert_eq!(logic.get_frame(), case.checkpoint);
    while logic.get_frame() < case.end {
        let processed = logic.get_frame();
        logic.update();
        trace.push(record(
            &logic,
            ids,
            Some(processed),
            "continuation",
            &mut journal,
        ));
    }
    let save_path = manager.get_save_path("strategy");
    let save = fs::read(&save_path).unwrap();
    assert!(!save.is_empty());
    let output = json!({"version":1,"role":role,"case":case.name,"pid":std::process::id(),
        "cwd":dir,"executable":std::env::current_exe().unwrap(),
        "save_path":save_path,"save_bytes":save.len(),"save_crc32":crc32fast::hash(&save),
        "inputs":fixture_inputs,"source_prefix":prefix,"trace":trace});
    write_json(&dir.join(format!("{role}.json")), &output);
}

fn first_difference(expected: &Value, actual: &Value, path: &str) -> Option<Value> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => {
            for (key, value) in a {
                if let Some(diff) = first_difference(value, &b[key], &format!("{path}.{key}")) {
                    return Some(diff);
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (left, right)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = first_difference(left, right, &format!("{path}[{i}]")) {
                    return Some(diff);
                }
            }
        }
        _ => {}
    }
    Some(json!({"field":path,"writer":expected,"reader":actual}))
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
        .env("STRATEGY_SAVE_ROLE", role)
        .env("STRATEGY_SAVE_CASE", case.name)
        .current_dir(dir)
        .output()
        .expect("start fresh Strategy process");
    fs::write(dir.join(format!("{role}.stdout.txt")), &output.stdout).unwrap();
    fs::write(dir.join(format!("{role}.stderr.txt")), &output.stderr).unwrap();
    write_json(
        &dir.join(format!("{role}-status.json")),
        &json!({
            "success":output.status.success(),"exit_code":output.status.code(),
            "status":output.status.to_string(),"args":["--exact",CHILD_TEST,"--ignored","--nocapture","--test-threads=1"]
        }),
    );
    output.status.success()
}

fn read_result(dir: &Path, role: &str, case: Case) -> Value {
    let bytes =
        fs::read(dir.join(format!("{role}.json"))).expect("successful child must write its trace");
    assert!(!bytes.is_empty(), "empty trace is failed admission");
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["role"], role);
    assert_eq!(value["case"], case.name);
    let trace = value["trace"].as_array().expect("trace array required");
    assert_eq!(trace.len(), (case.end - case.checkpoint + 1) as usize);
    for (index, row) in trace.iter().enumerate() {
        let next = case.checkpoint + index as u32;
        assert_eq!(row["next_frame"], next);
        assert_eq!(row["processed_frame"], next - 1);
    }
    value
}

fn events(trace: &[Value]) -> Value {
    let mut discharges = Vec::new();
    let mut damage = Vec::new();
    for pair in trace.windows(2) {
        if pair[0]["gameplay"]["sequence"] != pair[1]["gameplay"]["sequence"] {
            discharges.push(json!({"processed_frame":pair[1]["processed_frame"],"gameplay":pair[1]["gameplay"]}));
        }
        if pair[0]["gameplay"]["target_hp"] != pair[1]["gameplay"]["target_hp"] {
            damage.push(json!({"processed_frame":pair[1]["processed_frame"],"hp":pair[1]["gameplay"]["target_hp"]}));
        }
    }
    json!({"discharges":discharges,"damage":damage})
}

#[test]
fn strategy_save_continues_in_fresh_processes() {
    assert!(
        std::env::var_os("STRATEGY_SAVE_ROLE").is_none(),
        "driver cannot run in a child"
    );
    // Retain before any assertion/child launch so every failure path preserves
    // its evidence, including malformed output and admission failures.
    let root = std::env::var_os("STRATEGY_SAVE_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| tempfile::TempDir::new().unwrap().keep());
    fs::create_dir_all(&root).unwrap();
    println!("STRATEGY_SAVE_EVIDENCE {}", root.display());
    let mut failures = Vec::new();
    let mut summaries = Vec::new();
    for case in CASES {
        let dir = root.join(case.name);
        fs::create_dir(&dir).expect("use a new evidence directory; never overwrite a baseline");
        let locomotor_path = dir.join(LOCOMOTOR_PATH);
        fs::create_dir_all(locomotor_path.parent().unwrap()).unwrap();
        fs::write(locomotor_path, LOCOMOTOR_INI).unwrap();
        println!("STRATEGY_SAVE_CASE {} {}", case.name, dir.display());
        let writer_ok = child(&dir, case, "writer");
        let reader_ok = writer_ok && child(&dir, case, "reader");
        if !writer_ok || !reader_ok {
            failures.push(format!(
                "{} child admission failed (writer={writer_ok}, reader={reader_ok})",
                case.name
            ));
            continue;
        }
        let writer = read_result(&dir, "writer", case);
        let reader = read_result(&dir, "reader", case);
        assert_eq!(
            writer["inputs"], reader["inputs"],
            "fresh processes must preload identical authored inputs"
        );
        assert_eq!(writer["save_crc32"], reader["save_crc32"]);
        let reference = writer["trace"].as_array().unwrap();
        let restored = reader["trace"].as_array().unwrap();
        let mut differences = Vec::new();
        let mut first_persisted = None;
        let mut first_body = None;
        let mut first_runtime = None;
        let mut first_gameplay = None;
        for (expected, actual) in reference.iter().zip(restored) {
            for (section, first) in [
                ("persisted", &mut first_persisted),
                ("body_observations", &mut first_body),
                ("runtime_observations", &mut first_runtime),
                ("gameplay", &mut first_gameplay),
            ] {
                if let Some(diff) = first_difference(&expected[section], &actual[section], section)
                {
                    let row = json!({"processed_frame":actual["processed_frame"],"next_frame":actual["next_frame"],
                        "immediate_checkpoint":actual["next_frame"] == case.checkpoint,"difference":diff});
                    if first.is_none() {
                        *first = Some(row.clone());
                    }
                    differences.push(row);
                }
            }
        }
        let final_sequence = reference.last().unwrap()["gameplay"]["sequence"]
            .as_u64()
            .unwrap();
        assert_eq!(
            final_sequence,
            if case.explicit { 1 } else { 2 },
            "bounded window must include the expected live discharges"
        );
        assert!(
            reference.last().unwrap()["gameplay"]["target_hp"]["value"]
                .as_f64()
                .unwrap()
                < 1000.0
        );
        let summary = json!({"case":case.name,"frames":reference.len(),
            "checkpoint":reference[0],"reader_checkpoint":restored[0],
            "first_persisted_difference":first_persisted,"first_body_difference":first_body,
            "first_runtime_difference":first_runtime,
            "first_gameplay_difference":first_gameplay,"writer_events":events(reference),"reader_events":events(restored),
            "observed_exact_state_equal":differences.is_empty(),
            "persisted_equal":first_persisted.is_none(),"gameplay_equal":first_gameplay.is_none()});
        write_json(
            &dir.join("comparison.json"),
            &json!({"summary":summary,"every_frame_differences":differences}),
        );
        println!(
            "STRATEGY_SAVE_SUMMARY {}",
            json!({"case":case.name,
            "first_persisted_difference":first_persisted,"first_body_difference":first_body,
            "first_runtime_difference":first_runtime,
            "first_gameplay_difference":first_gameplay})
        );
        // Body-machine omissions are retained as independent observations.
        // A nondefault omission alone is not a frame/discharge/HP defect.
        if first_persisted.is_some() || first_gameplay.is_some() {
            failures.push(format!(
                "{}: persisted={first_persisted:?}; gameplay={first_gameplay:?}",
                case.name
            ));
        }
        summaries.push(summary);
    }
    write_json(
        &root.join("summary.json"),
        &json!({"cases":summaries,"failures":failures}),
    );
    if !failures.is_empty() {
        panic!(
            "Strategy continuation failed; evidence={}\n{}",
            root.display(),
            failures.join("\n")
        );
    }
}

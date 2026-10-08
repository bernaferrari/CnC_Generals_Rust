//! Public Strategy Center activation, acquisition and attack through ordinary ticks.
//! C++ TurretAI.cpp:855-876 assigns a mood target without changing its angles;
//! :1055-1175 owns aiming. AIUpdate.cpp:998,1073-1077 advances each turret once.
//! Synthetic templates deliberately isolate this host route from retail asset loading.
use gamelogic::common::Relationship;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::host_strategy_center::{HostBattlePlan, HostBattlePlanTransition};
use generals_main::game_logic::object::TurretSubState;
use generals_main::game_logic::{
    AIState, AttackSubState, GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate,
    Weapon,
};
use glam::Vec3;
use serde_json::json;
use std::time::{Duration, UNIX_EPOCH};

const CENTER: &str = "AmericaStrategyCenter";
const GROUND: &str = "StrategyAimGroundTarget";
const AIR: &str = "StrategyAimAirTarget";
const ANCHOR: &str = "StrategyAimEnemyAnchor";
const OBSERVATION_FRAMES: u32 = 120;

struct Fixture {
    logic: GameLogic,
    center: ObjectId,
    anchor: ObjectId,
}

fn fixture(plan: Option<HostBattlePlan>) -> Fixture {
    let mut logic = GameLogic::new();
    for (id, team, alliance) in [(0, Team::USA, 31), (1, Team::GLA, 45)] {
        let mut player = Player::new(id, team, "Strategy aim fixture", id == 0);
        player.alliance_team = alliance;
        logic.add_player(player);
    }
    assert_eq!(logic.get_players().len(), 2);
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);

    // Every template is complete before any object is admitted. No live object,
    // plan registry, readiness, pose or owner is repaired by this fixture.
    let mut center = ThingTemplate::new(CENTER);
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Immobile)
        .set_health(1500.0);
    logic.templates.insert(CENTER.into(), center);
    for (name, kind) in [
        (GROUND, KindOf::Vehicle),
        (AIR, KindOf::Aircraft),
        (ANCHOR, KindOf::Structure),
    ] {
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(kind)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_none();
        logic.templates.insert(name.into(), template);
    }
    let center = logic
        .create_object_for_player(CENTER, 0, Vec3::ZERO)
        .unwrap();
    // A real far-away object keeps the enemy player alive during the door wait.
    // Without it, the ordinary victory pass makes that owner Neutral before
    // the immediate AttackObject control can be admitted.
    let anchor = logic
        .create_object_for_player(ANCHOR, 1, Vec3::new(2000.0, 0.0, 0.0))
        .unwrap();
    let initial = logic.host_object(center).unwrap();
    assert_eq!(initial.owner_player_id, Some(0));
    assert!(initial.is_constructed() && initial.is_alive());
    assert!(initial.weapon.is_none());
    assert!(!initial.turret_enabled);
    assert_eq!(initial.weapon_discharge_marker().sequence, 0);
    if let Some(plan) = plan {
        assert!(logic.activate_battle_plan(0, plan, Some(center)));
    }
    for processed_frame in 0..=210 {
        assert_eq!(logic.get_frame(), processed_frame);
        logic.update();
        let object = logic.host_object(center).unwrap();
        assert_eq!(logic.get_frame(), processed_frame + 1);
        assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
        let enemy_anchor = logic.host_object(anchor).unwrap();
        assert_eq!(enemy_anchor.get_position(), Vec3::new(2000.0, 0.0, 0.0));
        assert_eq!(enemy_anchor.health.current, 1000.0);
        assert_eq!(object.weapon_discharge_marker().sequence, 0);
        assert!(object.target.is_none() && object.turret_target_id.is_none());
        if processed_frame < 210 || plan != Some(HostBattlePlan::Bombardment) {
            assert!(object.weapon.is_none(), "gun appeared at {processed_frame}");
            assert!(!object.turret_enabled);
        }
    }
    if let Some(plan) = plan {
        let door = logic.battle_plans().door_state_for_center(center).unwrap();
        assert_eq!(door.status, HostBattlePlanTransition::Active);
        assert_eq!(door.door_plan, Some(plan));
    }
    assert_eq!(logic.battle_plans().active_plan_for_player(0), plan);
    assert_eq!(logic.battle_plans().active_plan_for_player(1), None);
    assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
    assert_eq!(logic.player_relationship(0, 1), Relationship::Enemies);
    println!(
        "STRATEGY_SETUP {}",
        json!({
            "plan":format!("{plan:?}"),"center_template":CENTER,"center":center.0,
            "anchor_template":ANCHOR,"anchor":anchor.0,"anchor_position":[2000.0,0.0,0.0],
            "owner_factions":["USA","GLA"],"alliances":[31,45],"next_frame":logic.get_frame()
        })
    );
    let fixture = Fixture {
        logic,
        center,
        anchor,
    };
    assert_center_admission(&fixture, plan == Some(HostBattlePlan::Bombardment));
    fixture
}

fn assert_center_admission(f: &Fixture, armed: bool) {
    let center = f.logic.host_object(f.center).unwrap();
    assert_eq!(center.owner_player_id, Some(0));
    assert_eq!(center.team, Team::USA);
    assert_eq!(center.get_position(), Vec3::ZERO);
    assert_eq!(center.get_orientation(), 0.0);
    assert!(center.is_alive() && center.is_constructed() && !center.is_disabled());
    assert!(!f.logic.skirmish_ai_auto_engage_paused(f.center));
    assert_eq!(center.turret_enabled, armed);
    assert_eq!(center.weapon.is_some(), armed);
    if armed {
        let weapon = center.weapon.as_ref().unwrap();
        assert_eq!(
            (weapon.damage, weapon.min_range, weapon.range),
            (200.0, 100.0, 400.0)
        );
        assert_eq!(center.selected_weapon_slot(), Some(0));
        assert!(center.can_attack());
        assert!(Object::weapon_ready(
            weapon,
            f.logic.get_frame() as f32 / 30.0
        ));
        assert!((center.turret_turn_rate_rad.to_degrees() - 2.0).abs() < 0.0001);
    }
    let anchor = f.logic.host_object(f.anchor).unwrap();
    assert_eq!(anchor.owner_player_id, Some(1));
    assert!(anchor.is_alive() && anchor.weapon.is_none());
    assert!(center.get_position().distance(anchor.get_position()) > 400.0);
}

fn target(f: &mut Fixture, template: &str, distance: f32) -> ObjectId {
    let target = f
        .logic
        .create_object_for_player(template, 1, Vec3::new(distance, 0.0, 0.0))
        .unwrap();
    let center = f.logic.host_object(f.center).unwrap();
    let object = f.logic.host_object(target).unwrap();
    assert_eq!(object.owner_player_id, Some(1));
    assert_eq!(object.team, Team::GLA);
    assert!(object.is_alive() && object.is_constructed() && object.weapon.is_none());
    assert_eq!(object.health.current, 1000.0);
    assert_eq!(
        f.logic.object_relationship(center, object),
        Relationship::Enemies
    );
    assert!(center.target.is_none() && center.turret_target_id.is_none());
    assert!(!center.turret_mood_target);
    target
}

fn explicit_attack_before_first_mood_tick(f: &mut Fixture, target: ObjectId) {
    let frame = f.logic.get_frame();
    f.logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![f.center],
        modifier_keys: ModifierKeys::default(),
    });
    f.logic.process_commands();
    assert_eq!(f.logic.get_frame(), frame);
    let center = f.logic.host_object(f.center).unwrap();
    assert_eq!(center.ai_state, AIState::Attacking);
    assert_eq!(center.target, Some(target));
    assert_eq!(center.turret_target_id, Some(target));
    assert_eq!(center.attack_substate, AttackSubState::AimAtTarget);
    assert_eq!(center.turret_substate, TurretSubState::Aim);
    assert!(center.status.attacking && center.status.is_aiming_weapon);
    assert!(!center.turret_mood_target);
    assert_eq!(center.weapon_discharge_marker().sequence, 0);
}

#[derive(Debug, Default)]
struct Outcome {
    first_mood_frame: Option<u32>,
    first_discharge_frame: Option<u32>,
    first_damage_frame: Option<u32>,
    sequence: u64,
    health: f32,
    maximum_yaw_step: f32,
    oversized_yaw_frames: Vec<u32>,
}

fn observe(f: &mut Fixture, target: ObjectId, label: &str) -> Outcome {
    let mut outcome = Outcome::default();
    let initial_health = f.logic.host_object(target).unwrap().health.current;
    let mut previous_yaw = f.logic.host_object(f.center).unwrap().turret_angle_deg;
    for tick in 0..=OBSERVATION_FRAMES {
        let processed_frame = if tick == 0 {
            None
        } else {
            Some(f.logic.get_frame())
        };
        if tick != 0 {
            f.logic.update();
        }
        let center = f.logic.host_object(f.center).unwrap();
        let victim = f.logic.host_object(target).unwrap();
        let yaw_step =
            Object::normalize_angle_rad((center.turret_angle_deg - previous_yaw).to_radians())
                .to_degrees()
                .abs();
        previous_yaw = center.turret_angle_deg;
        if tick != 0 {
            outcome.maximum_yaw_step = outcome.maximum_yaw_step.max(yaw_step);
            if yaw_step > center.turret_turn_rate_rad.to_degrees() + 0.001 {
                outcome.oversized_yaw_frames.push(processed_frame.unwrap());
            }
        }
        outcome.sequence = center.weapon_discharge_marker().sequence;
        outcome.health = victim.health.current;
        if center.turret_mood_target && outcome.first_mood_frame.is_none() {
            outcome.first_mood_frame = processed_frame;
        }
        if outcome.sequence > 0 && outcome.first_discharge_frame.is_none() {
            outcome.first_discharge_frame = processed_frame;
        }
        if outcome.health < initial_health && outcome.first_damage_frame.is_none() {
            outcome.first_damage_frame = processed_frame;
        }
        let projectiles: Vec<_> = f
            .logic
            .combat_system()
            .projectiles_snapshot()
            .into_iter()
            .filter(|p| p.shooter_id == f.center)
            .map(|p| {
                json!({"id":p.id.0,"target":p.target_id.map(|id|id.0),
                "position":p.position.to_array(),"speed_per_second":p.speed,"damage":p.damage})
            })
            .collect();
        println!(
            "STRATEGY_TRACE {}",
            json!({
                "case":label,"tick":tick,"processed_frame":processed_frame,"next_frame":f.logic.get_frame(),
                "owners":[center.owner_player_id,victim.owner_player_id],
                "players_alive":[f.logic.player_is_alive(0),f.logic.player_is_alive(1)],
                "relationship":format!("{:?}", f.logic.object_relationship(center,victim)),
                "positions":[center.get_position().to_array(),victim.get_position().to_array()],
                "body_yaw":center.get_orientation(),"yaw":center.turret_angle_deg,"yaw_step":yaw_step,
                "desired_yaw":center.relative_angle_2d_to(victim.get_position()).to_degrees(),
                "pitch":center.turret_pitch_deg,"mood":center.turret_mood_target,
                "target":center.target.map(|id|id.0),"turret_target":center.turret_target_id.map(|id|id.0),
                "ai":format!("{:?}",center.ai_state),"attack_substate":format!("{:?}",center.attack_substate),
                "turret_substate":format!("{:?}",center.turret_substate),
                "attacking":center.status.attacking,"aiming":center.status.is_aiming_weapon,
                "firing":center.status.is_firing_weapon,"disabled":center.is_disabled(),
                "enabled":center.turret_enabled,"can_attack":center.can_attack(),
                "slot":center.selected_weapon_slot(),"weapon":center.weapon.as_ref().map(|w|json!({
                    "damage":w.damage,"minimum":w.min_range,"range":w.range,"last_fire":w.last_fire_time,
                    "speed":w.projectile_speed,"ready":Object::weapon_ready(w,f.logic.get_frame() as f32/30.0)})),
                "object_in_range":center.is_within_attack_range_for_slot(0,victim),
                "position_in_range":center.is_within_attack_range_pos_for_slot(0,victim.get_position()),
                "radii":[center.thing().template.geometry_info.bounding_circle_radius(),victim.thing().template.geometry_info.bounding_circle_radius()],
                "sequence":outcome.sequence,"health":outcome.health,"projectiles":projectiles
            })
        );
        assert_eq!(center.owner_player_id, Some(0));
        assert_eq!(victim.owner_player_id, Some(1));
        assert_eq!(center.get_position(), Vec3::ZERO);
        if label.starts_with("explicit")
            && (outcome.first_damage_frame.is_none()
                || outcome.first_damage_frame == processed_frame)
        {
            assert_eq!(center.ai_state, AIState::Attacking);
            assert_eq!(center.target, Some(target));
            assert_eq!(center.turret_target_id, Some(target));
            assert!(
                !center.turret_mood_target,
                "explicit control became mood-owned"
            );
        }
        assert!(f.logic.player_is_alive(0) && f.logic.player_is_alive(1));
        assert_eq!(
            f.logic.object_relationship(center, victim),
            Relationship::Enemies
        );
        assert_eq!(
            f.logic.host_object(f.anchor).unwrap().get_position(),
            Vec3::new(2000.0, 0.0, 0.0)
        );
        assert_eq!(
            f.logic.host_object(f.anchor).unwrap().health.current,
            1000.0
        );
    }
    println!("STRATEGY_OUTCOME {label} {outcome:?}");
    outcome
}

fn firing_arm(explicit: bool, label: &str) -> Outcome {
    let mut f = fixture(Some(HostBattlePlan::Bombardment));
    let target = target(&mut f, GROUND, 150.0);
    let center = f.logic.host_object(f.center).unwrap();
    assert!(center.is_within_attack_range_for_slot(0, f.logic.host_object(target).unwrap()));
    assert!(center.is_within_attack_range_pos_for_slot(0, Vec3::new(150.0, 0.0, 0.0)));
    if explicit {
        explicit_attack_before_first_mood_tick(&mut f, target);
    }
    observe(&mut f, target, label)
}

fn assert_fired_and_damaged(outcome: &Outcome) {
    assert!(outcome.sequence > 0, "no accepted discharge: {outcome:?}");
    assert!(
        outcome.health < 1000.0,
        "discharge never damaged target: {outcome:?}"
    );
    assert!(
        outcome.first_damage_frame >= outcome.first_discharge_frame,
        "damage preceded discharge"
    );
    assert_eq!(
        outcome.sequence, 1,
        "210-frame reload allows exactly one accepted discharge in this120-frame window"
    );
}

#[test]
fn bombardment_mood_acquisition_reaches_a_real_discharge_and_damage() {
    let outcome = firing_arm(false, "automatic");
    assert!(
        outcome.first_mood_frame.is_some(),
        "auto path never acquired a mood target"
    );
    assert_fired_and_damaged(&outcome);
}

#[test]
fn explicit_attack_before_mood_acquisition_reaches_discharge_and_damage() {
    let outcome = firing_arm(true, "explicit");
    assert_fired_and_damaged(&outcome);
}

#[test]
fn automatic_aim_obeys_one_authored_turret_step_per_frame() {
    let outcome = firing_arm(false, "automatic-cadence");
    assert!(
        outcome.oversized_yaw_frames.is_empty(),
        "C++ turret advances once: {outcome:?}"
    );
}

#[test]
fn explicit_aim_obeys_one_authored_turret_step_per_frame() {
    let outcome = firing_arm(true, "explicit-cadence");
    assert!(
        outcome.oversized_yaw_frames.is_empty(),
        "body AI must leave turret aiming to its own state: {outcome:?}"
    );
}

fn inactive_gun_control(plan: Option<HostBattlePlan>, label: &str) {
    let mut f = fixture(plan);
    let target = target(&mut f, GROUND, 150.0);
    let outcome = observe(&mut f, target, label);
    assert_eq!(outcome.sequence, 0);
    assert_eq!(outcome.health, 1000.0);
    assert_eq!(outcome.first_mood_frame, None);
}

#[test]
fn no_plan_does_not_enable_the_gun() {
    inactive_gun_control(None, "no-plan");
}

#[test]
fn search_and_destroy_does_not_enable_the_gun() {
    inactive_gun_control(Some(HostBattlePlan::SearchAndDestroy), "search-and-destroy");
}

fn excluded_target_control(template: &str, distance: f32, label: &str) {
    let mut f = fixture(Some(HostBattlePlan::Bombardment));
    let target = target(&mut f, template, distance);
    assert_center_admission(&f, true);
    let center = f.logic.host_object(f.center).unwrap();
    let victim = f.logic.host_object(target).unwrap();
    if template == AIR {
        assert!(victim.is_kind_of(KindOf::Aircraft));
        assert!(!center.weapon.as_ref().unwrap().can_target_air);
    } else {
        assert!(!center.is_within_attack_range_for_slot(0, victim));
        assert!(distance < center.weapon.as_ref().unwrap().min_range);
    }
    let outcome = observe(&mut f, target, label);
    assert_eq!(outcome.sequence, 0);
    assert_eq!(outcome.health, 1000.0);
}

#[test]
fn active_bombardment_excludes_air() {
    excluded_target_control(AIR, 150.0, "air");
}

#[test]
fn active_bombardment_excludes_inside_minimum_range() {
    excluded_target_control(GROUND, 50.0, "too-close");
}

#[test]
fn idle_acquisition_sets_goal_without_turning_until_next_turret_update() {
    let mut f = fixture(Some(HostBattlePlan::Bombardment));
    let victim = target(&mut f, GROUND, 150.0);
    let initial = f.logic.host_object(f.center).unwrap();
    let pose = (initial.turret_angle_deg, initial.turret_pitch_deg);
    let rate = initial.turret_turn_rate_rad.to_degrees();
    f.logic.update();
    let acquired = f.logic.host_object(f.center).unwrap();
    println!(
        "STRATEGY_ACQUISITION {}",
        json!({
            "processed_frame":211,"before":pose,"after":[acquired.turret_angle_deg,acquired.turret_pitch_deg],
            "target":acquired.target.map(|id|id.0),"turret_target":acquired.turret_target_id.map(|id|id.0),
            "mood":acquired.turret_mood_target,"substate":format!("{:?}",acquired.turret_substate)
        })
    );
    assert!(acquired.turret_mood_target);
    assert_eq!(acquired.target, Some(victim));
    assert_eq!(acquired.turret_target_id, Some(victim));
    assert_eq!(acquired.turret_substate, TurretSubState::Aim);
    assert_eq!((acquired.turret_angle_deg, acquired.turret_pitch_deg), pose);
    assert_eq!(acquired.weapon_discharge_marker().sequence, 0);
    f.logic.update();
    let aiming = f.logic.host_object(f.center).unwrap();
    assert!((aiming.turret_angle_deg - pose.0 - rate).abs() < 0.001);
    assert_eq!(aiming.turret_pitch_deg, pose.1);
    assert_eq!(aiming.weapon_discharge_marker().sequence, 0);
}

fn explicit_takeover_after_mood(replace_goal: bool) {
    let mut f = fixture(Some(HostBattlePlan::Bombardment));
    let original = target(&mut f, GROUND, 150.0);
    f.logic.update();
    let acquired = f.logic.host_object(f.center).unwrap();
    assert!(acquired.turret_mood_target);
    assert_eq!(acquired.target, Some(original));
    assert_eq!(acquired.turret_target_id, Some(original));
    assert_eq!(acquired.weapon_discharge_marker().sequence, 0);
    let victim = if replace_goal {
        let id = f
            .logic
            .create_object_for_player(GROUND, 1, Vec3::new(-150.0, 0.0, 0.0))
            .unwrap();
        let object = f.logic.host_object(id).unwrap();
        assert_eq!(object.owner_player_id, Some(1));
        assert_eq!(object.health.current, 1000.0);
        assert!(object.is_alive() && object.weapon.is_none());
        id
    } else {
        original
    };
    println!(
        "STRATEGY_TAKEOVER original={} requested={} same_target={}",
        original.0, victim.0, !replace_goal
    );
    // The same command admission assertions apply after independent mood
    // acquisition: success must clear mood even when the goal ID is unchanged.
    explicit_attack_before_first_mood_tick(&mut f, victim);
    let outcome = observe(
        &mut f,
        victim,
        if replace_goal {
            "explicit-after-mood-replace"
        } else {
            "explicit-after-mood-same"
        },
    );
    assert_fired_and_damaged(&outcome);
    assert_eq!(
        outcome.sequence, 1,
        "210-frame reload permits only one discharge in this window"
    );
    if replace_goal {
        assert_eq!(
            f.logic.host_object(original).unwrap().health.current,
            1000.0,
            "old goal must not win a residual rescan"
        );
    }
}

#[test]
fn explicit_same_target_command_takes_ownership_after_mood() {
    explicit_takeover_after_mood(false);
}

#[test]
fn explicit_replacement_command_reaims_without_firing_at_the_old_goal() {
    explicit_takeover_after_mood(true);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GenericAimControl {
    Turning,
    NoTurret,
    ZeroTurn,
    OutsideTurretSlot,
}

fn generic_aim_control(mode: GenericAimControl) {
    let mut logic = GameLogic::new();
    for (id, team) in [(0, Team::USA), (1, Team::GLA)] {
        let mut player = Player::new(id, team, "Generic aiming control", id == 0);
        player.alliance_team = id as i32;
        logic.add_player(player);
    }
    let name = if matches!(
        mode,
        GenericAimControl::Turning | GenericAimControl::OutsideTurretSlot
    ) {
        "AmericaTankCrusader"
    } else {
        "StrategyAimBodyControl"
    };
    let mut template = ThingTemplate::new(name);
    template
        .set_health(1000.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    let weapon = Weapon {
        damage: 25.0,
        range: 300.0,
        reload_time: 1.0,
        projectile_speed: 0.0,
        ..Weapon::default()
    };
    if mode == GenericAimControl::OutsideTurretSlot {
        template
            .set_primary_weapon_none()
            .set_secondary_weapon(weapon.clone());
    } else {
        template.set_primary_weapon(weapon.clone());
    }
    logic.templates.insert(name.into(), template.clone());
    let mut enemy_template = ThingTemplate::new(GROUND);
    enemy_template
        .set_health(1000.0)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_none();
    logic.templates.insert(GROUND.into(), enemy_template);
    let mut anchor_template = ThingTemplate::new(ANCHOR);
    anchor_template
        .set_health(1000.0)
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_none();
    logic.templates.insert(ANCHOR.into(), anchor_template);
    let attacker = if matches!(
        mode,
        GenericAimControl::ZeroTurn | GenericAimControl::OutsideTurretSlot
    ) {
        // Explicit synthetic fallback objects, including concrete WeaponSet
        // bindings, completely configured before public admission. This is
        // not live state repair or a claim of retail Comanche/Crusader INI.
        let mut object = Object::new(template, ObjectId(100), Team::USA);
        object.owner_player_id = Some(0);
        if mode == GenericAimControl::ZeroTurn {
            object.turret_enabled = true;
            object.turret_turn_rate_rad = 0.0;
            object.turret_angle_deg = 0.0;
            assert!(object.replace_weapon_set_slot(0, Some(weapon)));
            object.set_active_weapon_slot(0);
        } else {
            assert!(object.replace_weapon_set_slot(0, None));
            assert!(object.replace_weapon_set_slot(1, Some(weapon)));
            object.set_active_weapon_slot(1);
        }
        logic.add_object(object)
    } else {
        logic.create_object_for_player(name, 0, Vec3::ZERO).unwrap()
    };
    let anchor = logic
        .create_object_for_player(ANCHOR, 1, Vec3::new(2000.0, 0.0, 0.0))
        .unwrap();
    // Wait using normal ticks with no in-range enemy, until the actual admitted
    // weapon is ready. Canonical Crusader creation supplies its own live gun
    // stats, so the template's requested range/reload are not assumptions.
    for _ in 0..210 {
        let source = logic.host_object(attacker).unwrap();
        let slot = source.selected_weapon_slot().unwrap();
        if Object::weapon_ready(
            source.weapon_slot(slot).unwrap(),
            logic.get_frame() as f32 / 30.0,
        ) {
            break;
        }
        logic.update();
        assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
        assert_eq!(
            logic
                .host_object(attacker)
                .unwrap()
                .weapon_discharge_marker()
                .sequence,
            0
        );
        assert_eq!(logic.host_object(anchor).unwrap().health.current, 1000.0);
    }
    let victim = logic
        .create_object_for_player(GROUND, 1, Vec3::new(0.0, 0.0, 100.0))
        .unwrap();
    let source = logic.host_object(attacker).unwrap();
    let target = logic.host_object(victim).unwrap();
    let slot = source.selected_weapon_slot().unwrap();
    let actual_weapon = source.weapon_slot(slot).unwrap();
    println!(
        "GENERIC_AIM_ADMISSION {}",
        json!({"mode":format!("{mode:?}"),"frame":logic.get_frame(),
        "slot":slot,"damage":actual_weapon.damage,"min_range":actual_weapon.min_range,"range":actual_weapon.range,
        "reload":actual_weapon.reload_time,"ready":Object::weapon_ready(actual_weapon,logic.get_frame() as f32/30.0),
        "object_range":source.is_within_attack_range_for_slot(slot,target),
        "position_range":source.is_within_attack_range_pos_for_slot(slot,target.get_position()),
        "out_of_weapon_range":logic.out_of_weapon_range_object(attacker,victim)})
    );
    assert!(Object::weapon_ready(
        actual_weapon,
        logic.get_frame() as f32 / 30.0
    ));
    assert!(actual_weapon.min_range + 10.0 < 100.0 && actual_weapon.range > 110.0);
    assert!(source.is_within_attack_range_for_slot(slot, target));
    assert!(source.is_within_attack_range_pos_for_slot(slot, target.get_position()));
    assert!(!logic.out_of_weapon_range_object(attacker, victim));
    assert_eq!(source.ai_state, AIState::Idle);
    assert!(source.target.is_none() && source.turret_target_id.is_none());
    assert_eq!(source.owner_player_id, Some(0));
    assert_eq!(source.get_position(), Vec3::ZERO);
    assert_eq!(source.get_orientation(), 0.0);
    assert!(source.is_alive() && source.is_constructed() && !source.is_disabled());
    assert_eq!(source.turret_enabled, mode != GenericAimControl::NoTurret);
    if mode == GenericAimControl::ZeroTurn {
        assert_eq!(source.turret_turn_rate_rad, 0.0);
        assert_eq!(source.selected_weapon_slot(), Some(0));
        assert!(source.is_weapon_slot_on_turret(0));
        assert_eq!(source.weapon_slot(0).unwrap().damage, 25.0);
    }
    if mode == GenericAimControl::OutsideTurretSlot {
        assert!(source.weapon_slot(0).is_none());
        assert_eq!(source.weapon_slot(1).unwrap().damage, 25.0);
        assert_eq!(source.selected_weapon_slot(), Some(1));
        assert!(!source.is_weapon_slot_on_turret(1));
    }
    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: victim },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
    let admitted = logic.host_object(attacker).unwrap();
    if mode == GenericAimControl::OutsideTurretSlot {
        assert!(admitted.weapon_slot(0).is_none());
        assert_eq!(admitted.weapon_slot(1).unwrap().damage, 25.0);
    }
    if mode == GenericAimControl::ZeroTurn {
        assert_eq!(admitted.turret_turn_rate_rad, 0.0);
    }
    let slot = admitted.selected_weapon_slot().unwrap();
    assert_eq!(
        slot,
        if mode == GenericAimControl::OutsideTurretSlot {
            1
        } else {
            0
        }
    );
    assert_eq!(
        admitted.is_weapon_slot_on_turret(slot),
        matches!(
            mode,
            GenericAimControl::Turning | GenericAimControl::ZeroTurn
        )
    );
    assert_eq!(admitted.ai_state, AIState::Attacking);
    assert_eq!(admitted.attack_substate, AttackSubState::AimAtTarget);
    assert_eq!(admitted.target, Some(victim));
    assert!(admitted.status.is_aiming_weapon);
    assert!(!logic.out_of_weapon_range_object(attacker, victim));
    let authored_rate = admitted.turret_turn_rate_rad.to_degrees();
    let mut prior_yaw = admitted.turret_angle_deg;
    let mut maximum_yaw_step = 0.0_f32;
    let mut maximum_body_turn = 0.0_f32;
    let mut first_shot = None;
    let mut first_damage = None;
    for _ in 0..120 {
        let frame = logic.get_frame();
        logic.update();
        let source = logic.host_object(attacker).unwrap();
        let target = logic.host_object(victim).unwrap();
        let step = Object::normalize_angle_rad((source.turret_angle_deg - prior_yaw).to_radians())
            .to_degrees()
            .abs();
        prior_yaw = source.turret_angle_deg;
        maximum_yaw_step = maximum_yaw_step.max(step);
        maximum_body_turn = maximum_body_turn.max(source.get_orientation().abs());
        if source.weapon_discharge_marker().sequence > 0 && first_shot.is_none() {
            first_shot = Some(frame);
        }
        if target.health.current < 1000.0 && first_damage.is_none() {
            first_damage = Some(frame);
        }
        println!(
            "GENERIC_AIM_TRACE {}",
            json!({"mode":format!("{mode:?}"),"frame":frame,
            "owner":source.owner_player_id,"position":source.get_position().to_array(),
            "body_yaw":source.get_orientation(),"turret_yaw":source.turret_angle_deg,"yaw_step":step,
            "rate":authored_rate,"slot":source.selected_weapon_slot(),"slot_on_turret":source.is_weapon_slot_on_turret(slot),
            "target":source.target.map(|id|id.0),"turret_target":source.turret_target_id.map(|id|id.0),
            "mood":source.turret_mood_target,"sequence":source.weapon_discharge_marker().sequence,
            "health":target.health.current,"attack_state":format!("{:?}",source.attack_substate),
            "turret_state":format!("{:?}",source.turret_substate)})
        );
        assert_eq!(source.owner_player_id, Some(0));
        assert_eq!(target.owner_player_id, Some(1));
        assert!(logic.player_is_alive(0) && logic.player_is_alive(1));
        assert_eq!(
            logic.host_object(anchor).unwrap().get_position(),
            Vec3::new(2000.0, 0.0, 0.0)
        );
        assert_eq!(logic.host_object(anchor).unwrap().health.current, 1000.0);
        if mode == GenericAimControl::Turning {
            assert_ne!(
                source.attack_substate,
                AttackSubState::ApproachTarget,
                "stationary aim witness unexpectedly entered approach"
            );
            assert_ne!(
                source.attack_substate,
                AttackSubState::ChaseTarget,
                "stationary aim witness unexpectedly entered chase"
            );
        }
    }
    let source = logic.host_object(attacker).unwrap();
    let target = logic.host_object(victim).unwrap();
    println!(
        "GENERIC_AIM_OUTCOME mode={mode:?} first_shot={first_shot:?} first_damage={first_damage:?} sequence={} health={} maximum_body_turn={maximum_body_turn} maximum_yaw_step={maximum_yaw_step} authored_rate={authored_rate}",
        source.weapon_discharge_marker().sequence,
        target.health.current
    );
    assert!(first_shot.is_some(), "control never discharged");
    assert!(first_damage.is_some(), "control never damaged target");
    if mode == GenericAimControl::Turning {
        assert!(
            maximum_body_turn < 0.0001,
            "a turning turret must not rotate its body to aim"
        );
        assert!(
            maximum_yaw_step <= authored_rate + 0.001,
            "turret must advance only once per frame"
        );
    } else {
        assert!(
            maximum_body_turn > 0.1,
            "body/fake-turret fallback must still aim its body"
        );
    }
}

#[test]
fn non_strategy_turning_turret_fires_without_rotating_its_body() {
    generic_aim_control(GenericAimControl::Turning);
}

#[test]
fn non_turret_weapon_keeps_body_aim_firing() {
    generic_aim_control(GenericAimControl::NoTurret);
}

#[test]
fn zero_turn_fake_turret_keeps_body_aim_firing() {
    generic_aim_control(GenericAimControl::ZeroTurn);
}

#[test]
fn weapon_outside_controlled_turret_slot_keeps_body_aim_firing() {
    generic_aim_control(GenericAimControl::OutsideTurretSlot);
}

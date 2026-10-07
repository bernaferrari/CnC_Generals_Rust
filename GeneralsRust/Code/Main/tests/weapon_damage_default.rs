//! Admission-first runtime witness for WeaponTemplate's EXPLOSION default.
//! The target armor coefficients are the current Rust residuals, not retail
//! Armor.ini or full original-game shot parity. Frame CRCs are diagnostic only.

use gamelogic::damage::DamageType as NativeDamageType;
use generals_main::assets::ini_template_loader::register_weapons_from_ini_text;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::deterministic_trace::FrameTrace;
use generals_main::game_logic::{
    DamageType, GameLogic, KindOf, Player, Team, ThingTemplate, Weapon,
};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

#[derive(Clone, Copy)]
enum Binding {
    Unnamed,
    Named(Option<(&'static str, NativeDamageType)>),
    Missing,
}

#[derive(Debug)]
struct Observation {
    frame: u32,
    shots: u32,
    health: f32,
    damage_type: Option<DamageType>,
}

fn run_case(
    label: &str,
    binding: Binding,
    speed: f32,
    radius: f32,
    vehicle: bool,
    frames: u32,
    arrival_delay: u32,
    armor_coefficient: f32,
) {
    let mut world = GameLogic::new();
    // Declare opposing skirmish alliances before admission. Distinct faction
    // enums alone do not make two owner-admitted players enemies.
    let mut source_player = Player::new(0, Team::USA, "USA", true);
    let mut target_player = Player::new(1, Team::GLA, "GLA", false);
    source_player.alliance_team = 0;
    target_player.alliance_team = 1;
    world.add_player(source_player);
    world.add_player(target_player);
    let attacker_name = format!("WeaponDefaultAttacker_{label}");
    let target_name = format!("WeaponDefaultTarget_{label}");
    // Unique fresh names avoid inheriting DamageType through a partial override.
    let rule_name = format!("WeaponDefaultRule_{label}");
    let mut attacker = ThingTemplate::new(&attacker_name);
    attacker
        .set_health(360.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Vehicle);
    let reload_time = match binding {
        Binding::Named(authored_type) => {
            gamelogic::initialize_weapon_store().unwrap();
            let field = authored_type
                .map(|(token, _)| format!("  DamageType = {token}\n"))
                .unwrap_or_default();
            assert!(
                gamelogic::weapon::with_weapon_store(|store| store
                    .find_weapon_template(&rule_name)
                    .is_none())
                .unwrap(),
                "named rule must be fresh before real loader registration"
            );
            assert_eq!(
                register_weapons_from_ini_text(&format!(
                    "Weapon {rule_name}\n  PrimaryDamage = 25\n  AttackRange = 100\n  DelayBetweenShots = 100\n  WeaponSpeed = {}\n  PrimaryDamageRadius = {radius}\n  AntiGround = Yes\n  ProjectileObject = NONE\n{field}End\n",
                    speed * 30.0
                )),
                1
            );
            let parsed = gamelogic::weapon::with_weapon_store(|store| {
                store.find_weapon_template(&rule_name).unwrap().clone()
            })
            .unwrap();
            assert_eq!(
                parsed.damage_type,
                authored_type
                    .map(|(_, ty)| ty)
                    .unwrap_or(NativeDamageType::Explosion)
            );
            assert_eq!(parsed.min_delay_between_shots, 3);
            assert_eq!(parsed.max_delay_between_shots, 3);
            attacker.set_primary_weapon_name(&rule_name);
            3.0 / 30.0
        }
        Binding::Unnamed | Binding::Missing => {
            attacker.set_primary_weapon(Weapon {
                damage: 25.0,
                range: 100.0,
                reload_time: 0.0,
                projectile_speed: speed,
                splash_radius: radius,
                ..Weapon::default()
            });
            if matches!(binding, Binding::Missing) {
                attacker.set_primary_weapon_name(&rule_name);
                assert!(ThingTemplate::weapon_from_store(&rule_name).is_none());
            }
            0.0
        }
    };
    let mut target = ThingTemplate::new(&target_name);
    target
        .set_health(240.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_none();
    if vehicle {
        target.add_kind_of(KindOf::Vehicle);
    }
    world.templates.insert(attacker_name.clone(), attacker);
    world.templates.insert(target_name.clone(), target);
    let attacker_id = world
        .create_object(&attacker_name, Team::USA, Vec3::ZERO)
        .unwrap();
    // Within the existing close-range LOS bypass; no terrain fixture repairs.
    let target_id = world
        .create_object(&target_name, Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();

    let admitted = world.get_objects().get(&attacker_id).unwrap();
    let expected_name = (!matches!(binding, Binding::Unnamed)).then_some(rule_name.as_str());
    assert_eq!(admitted.weapon_name_for_slot(0), expected_name);
    let weapon = admitted.weapon.as_ref().unwrap();
    assert_eq!(weapon.damage, 25.0);
    assert_eq!(weapon.range, 100.0);
    assert_eq!(weapon.reload_time, reload_time);
    assert_eq!(weapon.projectile_speed, speed);
    assert_eq!(weapon.splash_radius, radius);
    assert_eq!(weapon.last_fire_time, 0.0);
    assert_eq!(admitted.fire_intent_count, 0);
    assert_eq!(admitted.weapon_discharge_marker().sequence, 0);
    assert!(
        world
            .get_objects()
            .get(&target_id)
            .unwrap()
            .weapon
            .is_none()
    );
    println!(
        "admitted {label}: name={expected_name:?} damage=25 range=100 reload={reload_time} speed={speed} radius={radius} vehicle={vehicle}"
    );

    let command = GameCommand {
        command_type: CommandType::AttackObject { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![attacker_id],
        modifier_keys: ModifierKeys::default(),
    };
    println!(
        "eligibility {label}: can_attack={} can_fire={} cannot_attack={} specific={:?} owner={:?}",
        admitted.can_attack(),
        admitted.can_fire(0.033333334),
        world.cannot_possibly_attack_object(attacker_id, target_id, false),
        world.get_able_to_attack_specific_object(
            attacker_id,
            target_id,
            generals_main::game_logic::AbleToAttackType::ContinuedTarget,
            false
        ),
        admitted.owner_player_id
    );
    assert_eq!(
        world.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    assert_eq!(
        world.get_able_to_attack_specific_object(
            attacker_id,
            target_id,
            generals_main::game_logic::AbleToAttackType::NewTarget,
            true
        ),
        generals_main::game_logic::CanAttackResult::Possible
    );
    world.queue_command(command.clone());
    let mut observations = vec![Observation {
        frame: 0,
        shots: 0,
        health: 240.0,
        damage_type: None,
    }];
    let mut discharge_frames = Vec::new();
    let mut pending_pair = None;
    for frame in 1..=frames {
        let processed_frame = world.get_frame();
        world.update();
        let trace = FrameTrace::from_game_logic(
            &world,
            [0; 6], // Trace metadata only; this does not set or repair the RNG.
            if frame == 1 {
                vec![command.clone()]
            } else {
                Vec::new()
            },
            None,
        );
        let shooter = world.get_objects().get(&attacker_id).unwrap();
        let victim = world.get_objects().get(&target_id).unwrap();
        let traced = trace
            .objects
            .iter()
            .find(|object| object.id == target_id)
            .unwrap();
        assert_eq!(trace.frame, frame);
        assert_eq!(traced.health, victim.health.current);
        assert_eq!(shooter.weapon_name_for_slot(0), expected_name);
        assert_eq!(victim.get_position(), Vec3::new(10.0, 0.0, 0.0));
        println!(
            "state {label}: frame={frame} ai={:?} target={:?} substate={:?} can_fire={} can_attack={} cannot_attack={}",
            shooter.ai_state,
            shooter.target,
            shooter.attack_substate,
            shooter.can_fire(frame as f32 / 30.0),
            shooter.can_attack(),
            world.cannot_possibly_attack_object(attacker_id, target_id, false)
        );
        let previous_shots = observations.last().unwrap().shots;
        let shots = shooter.fire_intent_count;
        let marker = shooter.weapon_discharge_marker();
        assert_eq!(marker.sequence, u64::from(shots));
        assert!(shots == previous_shots || shots == previous_shots + 1);
        if shots > previous_shots {
            assert_eq!(shooter.last_fire_victim_host, target_id.0);
            assert_eq!(shooter.last_fire_slot, 0);
            assert_eq!(shooter.last_fire_damage, 25.0);
            assert_eq!(shooter.last_fire_frame, processed_frame);
            assert_eq!(marker.logic_frame, processed_frame);
            assert_eq!(marker.weapon_slot, 0);
            discharge_frames.push(frame);
        }
        println!(
            "trace {label}: frame={frame} shots={shots} marker={marker:?} last_fire_frame={} last_fire_time={} target_health={} damage_type={:?} crc={:08x}",
            shooter.last_fire_frame,
            shooter.weapon.as_ref().unwrap().last_fire_time,
            traced.health,
            victim.last_damage_info_type,
            trace.crc
        );
        if discharge_frames.len() == 1 && shots > previous_shots && arrival_delay > 0 {
            // Public read-only snapshot exposes the actual queued shot without
            // direct fire, post-admission repair, restoration, or a lethal probe.
            let snapshot = SnapshotBuilder::new()
                .create_world_snapshot(&world)
                .unwrap();
            let pending = serde_json::to_value(&snapshot.pending_combat).unwrap();
            let rows = pending["delayed"].as_array().unwrap();
            assert_eq!(rows.len(), 1);
            let row = &rows[0];
            assert_eq!(row["when"], processed_frame + arrival_delay);
            assert_eq!(row["damage_id"], target_id.0);
            assert_eq!(row["pending"]["shooter_id"], attacker_id.0);
            assert_eq!(row["pending"]["target_id"], target_id.0);
            println!("pending {label}: {row}");
            pending_pair = Some((
                row["pending"]["damage_type"].as_str().unwrap().to_owned(),
                row["pending"]["death_type"].as_str().unwrap().to_owned(),
            ));
        }
        observations.push(Observation {
            frame,
            shots,
            health: traced.health,
            damage_type: victim.last_damage_info_type,
        });
    }
    assert!(
        !discharge_frames.is_empty(),
        "{label}: no accepted discharge"
    );
    if matches!(binding, Binding::Named(_)) {
        assert_eq!(
            discharge_frames[0], 4,
            "logic frame 3 is visible in post-update trace frame 4"
        );
        assert!(
            discharge_frames
                .windows(2)
                .all(|pair| pair[1] - pair[0] >= 3)
        );
    } else {
        assert!(
            discharge_frames[0] <= 2,
            "primary two-frame witness must actually fire"
        );
    }
    if let Some((damage_type, death_type)) = pending_pair {
        let expected = match binding {
            Binding::Unnamed => ("Explosive", "Normal"),
            Binding::Missing if radius > 0.0 => ("Explosive", "Exploded"),
            Binding::Missing => ("Bullet", "Normal"),
            Binding::Named(None) => ("Explosive", "Exploded"),
            Binding::Named(Some(("LASER", _))) => ("Laser", "Lasered"),
            Binding::Named(Some(("SMALL_ARMS", _))) => ("Bullet", "Normal"),
            Binding::Named(Some(("GATTLING", _))) => ("Gattling", "Normal"),
            _ => panic!("unexpected fixture"),
        };
        assert_eq!(
            (damage_type.as_str(), death_type.as_str()),
            expected,
            "{label}: actual delayed shot retains its independent damage/death defaults"
        );
    }
    for observation in &observations {
        let arrived = discharge_frames
            .iter()
            .filter(|&&f| f + arrival_delay <= observation.frame)
            .count();
        let expected_type = if arrived == 0 || armor_coefficient == 0.0 {
            None
        } else {
            Some(match binding {
                Binding::Unnamed | Binding::Named(None) => DamageType::Explosive,
                Binding::Missing if radius > 0.0 => DamageType::Explosive,
                Binding::Missing | Binding::Named(Some(("SMALL_ARMS", _))) => DamageType::Bullet,
                Binding::Named(Some(("GATTLING", _))) => DamageType::Gattling,
                Binding::Named(Some(("LASER", _))) => DamageType::Laser,
                _ => panic!("unexpected fixture"),
            })
        };
        assert_eq!(
            observation.damage_type, expected_type,
            "{label}: frame={} accepted damage type",
            observation.frame
        );
        let expected = 240.0 - 25.0 * armor_coefficient * arrived as f32;
        assert_eq!(
            observation.health, expected,
            "{label}: frame={} shots={} arrived={arrived}",
            observation.frame, observation.shots
        );
    }
    assert!(
        discharge_frames[0] + arrival_delay <= frames,
        "must observe real arrival"
    );
}

#[test]
fn unnamed_zero_speed_defaults_to_explosion_after_admission() {
    run_case("unnamed_zero", Binding::Unnamed, 0.0, 0.0, true, 2, 0, 1.0);
}
#[test]
fn unnamed_instant_speed_defaults_to_explosion() {
    run_case(
        "unnamed_instant",
        Binding::Unnamed,
        999_999.0,
        0.0,
        true,
        2,
        0,
        1.0,
    );
}
#[test]
fn unnamed_finite_speed_defaults_to_explosion_with_real_arrival() {
    run_case(
        "unnamed_finite",
        Binding::Unnamed,
        2.0,
        0.0,
        true,
        8,
        5,
        1.0,
    );
}
#[test]
fn unnamed_zero_speed_radius_keeps_explosion_default() {
    run_case(
        "unnamed_zero_radius",
        Binding::Unnamed,
        0.0,
        4.0,
        true,
        2,
        0,
        1.0,
    );
}
#[test]
fn unnamed_finite_speed_radius_keeps_delivery_delay() {
    run_case(
        "unnamed_finite_radius",
        Binding::Unnamed,
        2.0,
        4.0,
        true,
        8,
        5,
        1.0,
    );
}
#[test]
fn unnamed_zero_speed_neutral_armor_control() {
    run_case(
        "unnamed_neutral",
        Binding::Unnamed,
        0.0,
        0.0,
        false,
        2,
        0,
        1.0,
    );
}
#[test]
fn fresh_named_omitted_damage_type_defaults_to_explosion() {
    run_case(
        "named_default",
        Binding::Named(None),
        0.0,
        0.0,
        true,
        8,
        0,
        1.0,
    );
}
#[test]
fn fresh_named_laser_remains_laser() {
    run_case(
        "named_laser",
        Binding::Named(Some(("LASER", NativeDamageType::Laser))),
        0.0,
        0.0,
        false,
        8,
        0,
        1.0,
    );
}
#[test]
fn fresh_named_small_arms_remains_small_arms() {
    run_case(
        "named_small_arms",
        Binding::Named(Some(("SMALL_ARMS", NativeDamageType::SmallArms))),
        0.0,
        0.0,
        true,
        8,
        0,
        0.25,
    );
}
#[test]
fn fresh_named_gattling_remains_gattling() {
    run_case(
        "named_gattling",
        Binding::Named(Some(("GATTLING", NativeDamageType::Gattling))),
        0.0,
        0.0,
        true,
        8,
        0,
        0.1,
    );
}
#[test]
fn named_unresolved_zero_speed_keeps_existing_laser_policy() {
    run_case("missing_zero", Binding::Missing, 0.0, 0.0, true, 2, 0, 0.0);
}
#[test]
fn named_unresolved_finite_speed_keeps_existing_bullet_policy() {
    run_case(
        "missing_finite",
        Binding::Missing,
        2.0,
        0.0,
        true,
        8,
        5,
        0.25,
    );
}
#[test]
fn named_unresolved_radius_keeps_existing_explosion_policy() {
    run_case(
        "missing_radius",
        Binding::Missing,
        2.0,
        4.0,
        true,
        8,
        5,
        1.0,
    );
}

#[test]
fn fresh_named_default_keeps_existing_deferred_death_policy() {
    run_case(
        "named_default_finite",
        Binding::Named(None),
        2.0,
        0.0,
        true,
        10,
        5,
        1.0,
    );
}
#[test]
fn fresh_named_laser_keeps_existing_deferred_death_policy() {
    run_case(
        "named_laser_finite",
        Binding::Named(Some(("LASER", NativeDamageType::Laser))),
        2.0,
        0.0,
        false,
        10,
        5,
        1.0,
    );
}

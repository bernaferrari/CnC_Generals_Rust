//! Actual Main attack-fire acceptance and ordinary materialization ownership.
//! CPP Weapon.cpp:900-953,998-1080; no factory/global queue injection.
use super::*;
use crate::game_logic::combat::{
    apply_ready_projectileless_delayed_damage, drain_pending_projectiles,
};
use crate::game_logic::{KindOf, Team, ThingTemplate};

const FIRST_WEAPON: &str = "AcceptedQueueFirstWeapon";
const SECOND_WEAPON: &str = "AcceptedQueueSecondWeapon";

const FLIGHT_FIRST: &str = "AcceptedFlightFirstWeapon";
const FLIGHT_SECOND: &str = "AcceptedFlightSecondWeapon";

/// Inert, receiving-owned content: no installed WeaponStore or shared test lock.
fn new_owned_world() -> GameLogic {
    let mut world = GameLogic::new();
    for (id, team, alliance, name) in [
        (0, Team::USA, 1, "accepted-source"),
        (1, Team::China, 2, "accepted-target"),
    ] {
        let mut player = crate::game_logic::Player::new(id, team, name, false);
        player.alliance_team = alliance;
        world.add_player(player);
    }
    let mut rows = HashMap::new();
    let names = [
        (
            FIRST_WEAPON,
            17,
            "SMALL_ARMS",
            "NONE",
            30000000,
            "OCL_AcceptedQueueFirst",
        ),
        (
            SECOND_WEAPON,
            13,
            "SMALL_ARMS",
            "NONE",
            30000000,
            "OCL_AcceptedQueueSecond",
        ),
        (
            "SnapshotFiniteFirstWeapon",
            17,
            "SMALL_ARMS",
            "NONE",
            300,
            "NONE",
        ),
        (
            "SnapshotFiniteSecondWeapon",
            13,
            "SMALL_ARMS",
            "NONE",
            300,
            "NONE",
        ),
        (
            "AcceptedImpactFrameDamage",
            17,
            "SMALL_ARMS",
            "NONE",
            300,
            "NONE",
        ),
        (
            "AcceptedImpactFrameHealing",
            17,
            "HEALING",
            "NONE",
            300,
            "NONE",
        ),
        (
            FLIGHT_FIRST,
            17,
            "SMALL_ARMS",
            "AcceptedOwnerMissile",
            300,
            "OCL_AcceptedQueueFirst",
        ),
        (
            FLIGHT_SECOND,
            13,
            "SMALL_ARMS",
            "AcceptedOwnerMissile",
            300,
            "OCL_AcceptedQueueSecond",
        ),
    ];
    for (name, damage, kind, projectile, speed, ocl) in names {
        let text = format!(
            "Weapon {name}\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 0\n AttackRange = 200\n DamageType = {kind}\n ProjectileObject = {projectile}\n WeaponSpeed = {speed}\n MinWeaponSpeed = {speed}\n DelayBetweenShots = 1000\n ClipSize = 8\n ClipReloadTime = 1000\n PreAttackDelay = 0\n AntiGround = Yes\n FireOCL = {ocl}\nEnd\n"
        );
        assert_eq!(
            crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                &mut rows,
                &text,
                crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
            ),
            1
        );
        let row = rows.get(&name.to_ascii_lowercase()).unwrap();
        assert_eq!(row.primary_damage, damage as f32);
        // Original GameCommon.h f32 multiplication, executed at C++ O0/O2.
        assert_eq!(
            row.weapon_speed.to_bits(),
            if speed == 300 {
                0x4120_0001
            } else {
                0x4974_2401
            }
        );
        assert_eq!(row.attack_range, 200.0);
        assert_eq!(row.clip_size, 8);
        assert_eq!(row.min_delay_between_shots, 30);
        assert_eq!(row.max_delay_between_shots, 30);
        assert_eq!(row.projectile_name, projectile);
    }
    let mut parser = crate::assets::ini_parser::IniParser::new();
    parser.parse_ini_content(
        "Object AcceptedOwnerMissile\n Behavior = MissileAIUpdate ModuleTag_Flight\n IgnitionDelay = 0\n TryToFollowTarget = Yes\n End\nEnd",
        "accepted-owner-content.ini",
    ).unwrap();
    let objects: Vec<_> = parser
        .get_all_definitions()
        .iter()
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect();
    world
        .admit_weapon_content_snapshot(rows.into_iter().collect(), &objects)
        .unwrap();
    for (ocl_name, child) in [
        ("OCL_AcceptedQueueFirst", "AcceptedFireFirstChild"),
        ("OCL_AcceptedQueueSecond", "AcceptedFireSecondChild"),
    ] {
        let mut definition = gamelogic::effects::ObjectCreationList::new();
        definition.add_nugget(std::sync::Arc::new(
            gamelogic::object_creation_list::GenericObjectCreationNugget {
                names: vec![child.into()],
                disposition: gamelogic::object_creation_list::DebrisDisposition::new(
                    gamelogic::object_creation_list::DebrisDisposition::LIKE_EXISTING
                        | gamelogic::object_creation_list::DebrisDisposition::INHERIT_VELOCITY,
                ),
                ..Default::default()
            },
        ));
        world
            .admit_weapon_ocl_definition(ocl_name, definition)
            .unwrap();
        world
            .templates
            .insert(child.into(), ThingTemplate::new(child));
    }
    world
}

fn admit(
    world: &mut GameLogic,
    name: &str,
    weapon_name: Option<&str>,
    team: Team,
    pos: Vec3,
) -> ObjectId {
    let mut template = ThingTemplate::new(name);
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    template.add_kind_of(KindOf::Attackable);
    if let Some(weapon_name) = weapon_name {
        template.set_primary_weapon_name(weapon_name);
    }
    world.templates.insert(name.to_string(), template);
    world
        .create_object(name, team, pos)
        .expect("actual driving world admission")
}

fn populate(world: &mut GameLogic, weapon_name: &str) -> (ObjectId, ObjectId) {
    let source = admit(
        world,
        "AcceptedQueueShooter",
        Some(weapon_name),
        Team::USA,
        Vec3::ZERO,
    );
    let target = admit(
        world,
        "AcceptedQueueVictim",
        None,
        Team::China,
        Vec3::new(30.0, 0.0, 0.0),
    );
    world.frame = 100;
    // The actual Object flight helper uses this live victim pose, consistent
    // with the real roster/range check in attack_fire_weapon_update.
    let target_pos = world.objects[&target].get_position();
    world.objects.get_mut(&source).unwrap().prev_victim_pos = Some(target_pos);
    (source, target)
}

fn world(weapon_name: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = new_owned_world();
    let (source, target) = populate(&mut world, weapon_name);
    (world, source, target)
}

fn accept(world: &mut GameLogic, source: ObjectId, target: ObjectId, time: f32) {
    let unit = &world.objects[&source];
    let victim = &world.objects[&target];
    let rule = unit
        .selected_native_weapon_rule(0)
        .expect("receiving native weapon binding");
    assert!(
        unit.weapon_name_for_slot(0)
            .is_some_and(|name| name.eq_ignore_ascii_case(&rule.name))
    );
    assert!(unit.is_alive() && victim.is_alive());
    assert!(
        unit.can_fire(time),
        "parsed real weapon is ready before intended queue witness"
    );
    assert!(unit.is_within_attack_range_for_slot(0, victim));
    let ammo = unit
        .weapon
        .as_ref()
        .expect("admission binds parsed named weapon")
        .ammo;
    let prior_sequence = unit.weapon_discharge_marker().sequence;
    assert_eq!(
        world.attack_fire_weapon_update(source, target, time),
        AttackFireResult::Success
    );
    let unit = &world.objects[&source];
    assert_eq!(unit.weapon.as_ref().unwrap().ammo, ammo.map(|n| n - 1));
    assert!(unit.weapon_discharge_marker().sequence > prior_sequence);
    assert_eq!(unit.weapon_discharge_marker().weapon_slot, 0);
}

/// Advance actual receiving flight until its arena is empty; never drain a
/// synthetic acceptance queue to manufacture a delayed fire phase.
fn finish_accepted_flight(world: &mut GameLogic) {
    for _ in 0..40 {
        world.frame += 1;
        world.update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP);
        if world.combat_system.projectile_count() == 0 {
            return;
        }
    }
    panic!("authored accepted missile must reach its real impact/retirement");
}

#[test]
fn accepted_queue_same_id_normal_fire_cannot_materialize_in_other_world() {
    let (mut first, source, target) = world(FLIGHT_FIRST);
    let first_hp = first.objects[&target].health.current;
    accept(&mut first, source, target, 10.0);
    assert_eq!(first.objects[&target].health.current, first_hp);
    assert_eq!(first.combat_system.projectile_count(), 1);
    // Construction after acceptance cannot clear/adopt the first live arena.
    let (mut second, other_source, other_target) = world(FLIGHT_SECOND);
    assert_eq!((source, target), (other_source, other_target));
    let second_hp = second.objects[&other_target].health.current;
    assert_eq!(second.combat_system.projectile_count(), 0);
    finish_accepted_flight(&mut second);
    assert_eq!(second.objects[&other_target].health.current, second_hp);
    assert_eq!(first.combat_system.projectile_count(), 1);
    accept(&mut second, other_source, other_target, 10.0);
    assert_eq!(second.combat_system.projectile_count(), 1);
    assert_eq!(second.objects[&other_target].health.current, second_hp);
    finish_accepted_flight(&mut first);
    assert_eq!(first.objects[&target].health.current, first_hp - 17.0);
    assert_eq!(second.objects[&other_target].health.current, second_hp);
    assert_eq!(second.combat_system.projectile_count(), 1);
    finish_accepted_flight(&mut second);
    assert_eq!(
        second.objects[&other_target].health.current,
        second_hp - 13.0
    );
    finish_accepted_flight(&mut first);
    finish_accepted_flight(&mut second);
    assert_eq!(first.objects[&target].health.current, first_hp - 17.0);
    assert_eq!(
        second.objects[&other_target].health.current,
        second_hp - 13.0
    );
    assert!(first.combat_system.take_fire_ocl().is_empty());
    assert!(second.combat_system.take_fire_ocl().is_empty());
}

#[test]
fn accepted_queue_reset_reuse_discards_only_reset_worlds_unmaterialized_shot() {
    let (mut first, source, target) = world(FLIGHT_FIRST);
    let (mut second, other_source, other_target) = world(FLIGHT_SECOND);
    assert_eq!((source, target), (other_source, other_target));
    accept(&mut first, source, target, 10.0);
    accept(&mut second, other_source, other_target, 10.0);
    assert_eq!(first.combat_system.projectile_count(), 1);
    assert_eq!(second.combat_system.projectile_count(), 1);
    let second_hp = second.objects[&other_target].health.current;
    first.reset();
    assert_eq!(first.combat_system.projectile_count(), 0);
    assert_eq!(second.combat_system.projectile_count(), 1);
    let (reused_source, reused_target) = populate(&mut first, FLIGHT_FIRST);
    assert_eq!((reused_source, reused_target), (source, target));
    assert_eq!(
        first.objects[&reused_source]
            .weapon_discharge_marker()
            .sequence,
        0
    );
    let reused_hp = first.objects[&reused_target].health.current;
    finish_accepted_flight(&mut first);
    assert_eq!(
        first.objects[&reused_target].health.current, reused_hp,
        "ID reuse cannot replay reset world's live missile"
    );
    finish_accepted_flight(&mut second);
    assert_eq!(
        second.objects[&other_target].health.current,
        second_hp - 13.0,
        "another world's reset cannot clear this accepted flight"
    );
}

#[test]
fn accepted_queue_fifo_repeated_shots_keep_frozen_source_after_retirement() {
    let (mut world, source, target) = world(FLIGHT_FIRST);
    let other_source = admit(
        &mut world,
        "AcceptedQueueOtherShooter",
        Some(FLIGHT_SECOND),
        Team::USA,
        Vec3::new(0.0, 0.0, 1.0),
    );
    let target_pos = world.objects[&target].get_position();
    world
        .objects
        .get_mut(&other_source)
        .unwrap()
        .prev_victim_pos = Some(target_pos);
    let source_orientation = world.objects[&source].get_orientation();
    let source_rank = world.objects[&source].experience.level;
    let hp = world.objects[&target].health.current;
    accept(&mut world, source, target, 10.0);
    accept(&mut world, other_source, target, 10.0);
    accept(&mut world, source, target, 12.0);
    assert_eq!(world.objects[&target].health.current, hp);
    assert_eq!(world.combat_system.projectile_count(), 3);
    let cues = world.take_weapon_discharges_for_presentation();
    assert_eq!(
        cues.iter().map(|cue| cue.source).collect::<Vec<_>>(),
        vec![source, other_source, source]
    );
    assert_eq!(
        cues.iter().map(|cue| cue.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    // CPP943 creates actual FireOCL children synchronously in firing order,
    // before any flight. Sort only this observation by allocation identity.
    let mut children: Vec<_> = world
        .objects
        .registered()
        .filter(|(_, child)| {
            matches!(
                child.template_name.as_str(),
                "AcceptedFireFirstChild" | "AcceptedFireSecondChild"
            )
        })
        .map(|(id, child)| (*id, child))
        .collect();
    children.sort_by_key(|(id, _)| id.0);
    assert_eq!(
        children
            .iter()
            .map(|(_, child)| child.template_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "AcceptedFireFirstChild",
            "AcceptedFireSecondChild",
            "AcceptedFireFirstChild"
        ]
    );
    assert_eq!(children[0].1.producer_id, Some(source));
    assert_eq!(children[1].1.producer_id, Some(other_source));
    assert_eq!(children[2].1.producer_id, Some(source));
    assert_eq!(children[0].1.team, Team::USA);
    assert_eq!(children[2].1.team, Team::USA);
    assert_eq!(children[0].1.get_orientation(), source_orientation);
    assert_eq!(
        children[0].1.get_position(),
        world.objects[&source].get_position()
    );
    assert_eq!(
        children[1].1.get_position(),
        world.objects[&other_source].get_position()
    );
    let mut flights = world.combat_system.projectiles_snapshot();
    flights.sort_by_key(|flight| flight.id.0);
    assert_eq!(
        flights
            .iter()
            .map(|flight| flight.shooter_id)
            .collect::<Vec<_>>(),
        vec![source, other_source, source]
    );
    assert_eq!(
        flights
            .iter()
            .map(|flight| flight.damage)
            .collect::<Vec<_>>(),
        vec![17.0, 13.0, 17.0]
    );
    assert!(flights.iter().all(|flight| flight.source_team == Team::USA));
    assert_eq!(flights[0].source_veterancy, source_rank);
    assert_eq!(flights[2].source_veterancy, source_rank);
    assert!(
        flights
            .iter()
            .all(|flight| flight.projectile_object_name == "AcceptedOwnerMissile")
    );
    assert!(
        world.combat_system.take_fire_ocl().is_empty(),
        "accepted effects cannot replay at flight"
    );
    world.destroy_object(source);
    world.process_destroy_list_if_needed();
    assert!(
        !world.objects.contains_key(&source),
        "retire through actual world destruction"
    );
    assert_eq!(world.combat_system.projectile_count(), 3);
    finish_accepted_flight(&mut world);
    assert_eq!(world.objects[&target].health.current, hp - 47.0);
    finish_accepted_flight(&mut world);
    assert_eq!(world.objects[&target].health.current, hp - 47.0);
    assert!(world.combat_system.take_fire_ocl().is_empty());
    assert_eq!(
        world
            .objects
            .registered_values()
            .filter(|child| matches!(
                child.template_name.as_str(),
                "AcceptedFireFirstChild" | "AcceptedFireSecondChild"
            ))
            .count(),
        3
    );
}

#[path = "accepted_fire_frame_owner_tests.rs"]
mod accepted_fire_frame_owner_tests;

#[path = "accepted_damage_frame_owner_tests.rs"]
mod accepted_damage_frame_owner_tests;

mod pending_combat_snapshot_tests;

#[path = "authored_projectile_stream_tests.rs"]
mod authored_projectile_stream_tests;

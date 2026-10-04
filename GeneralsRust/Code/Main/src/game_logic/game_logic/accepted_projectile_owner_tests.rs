//! Actual Main attack-fire acceptance and ordinary materialization ownership.
//! CPP Weapon.cpp:900-953,998-1080; no factory/global queue injection.
use super::*;
use crate::game_logic::combat::tests::combat_test_guard;
use crate::game_logic::combat::{
    apply_ready_projectileless_delayed_damage, drain_pending_projectiles,
};
use crate::game_logic::{KindOf, Team, ThingTemplate};

const FIRST_WEAPON: &str = "AcceptedQueueFirstWeapon";
const SECOND_WEAPON: &str = "AcceptedQueueSecondWeapon";

struct RestoreInputs {
    frame: u32,
    authority: GameWorldAuthority,
    weapon_store: Option<gamelogic::weapon::WeaponStore>,
}

impl RestoreInputs {
    fn install() -> Self {
        let saved = Self {
            frame: crate::game_logic::host_historic_bonus::logic_frame(),
            authority: current_gameworld_authority(),
            weapon_store: gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok(),
        };
        gameworld_authority::publish_gameworld_authority(GameWorldAuthority::DEFAULT_OFF);
        gamelogic::initialize_weapon_store().expect("real host WeaponStore");
        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        let source = r#"
Weapon AcceptedQueueFirstWeapon
  PrimaryDamage = 17
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 30000000
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
  FireOCL = OCL_AcceptedQueueFirst
End
Weapon AcceptedQueueSecondWeapon
  PrimaryDamage = 13
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 30000000
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
  FireOCL = OCL_AcceptedQueueSecond
End
"#;
        assert_eq!(
            crate::assets::ini_template_loader::register_weapons_from_ini_text(source),
            2
        );
        gamelogic::weapon::with_weapon_store(|store| {
            for (name, damage) in [(FIRST_WEAPON, 17.0), (SECOND_WEAPON, 13.0)] {
                let authored = store
                    .find_weapon_template(name)
                    .expect("actual parsed named rules");
                assert_eq!(authored.primary_damage, damage);
                assert_eq!(authored.weapon_speed, 1_000_000.0);
                assert_eq!(authored.attack_range, 200.0);
                assert_eq!(authored.clip_size, 8);
                assert_eq!(authored.min_delay_between_shots, 30);
                assert_eq!(authored.max_delay_between_shots, 30);
                assert!(authored.projectile_name.eq_ignore_ascii_case("NONE"));
            }
        })
        .expect("inspect actual registered rules");
        saved
    }
}

impl Drop for RestoreInputs {
    fn drop(&mut self) {
        if let Some(previous) = self.weapon_store.take() {
            gamelogic::weapon::with_weapon_store_mut(|store| *store = previous)
                .expect("restore prior real store");
        } else {
            gamelogic::weapon::shutdown_weapon_store().expect("restore absent store");
        }
        gameworld_authority::publish_gameworld_authority(self.authority);
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
    }
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
    let mut world = GameLogic::new();
    let (source, target) = populate(&mut world, weapon_name);
    (world, source, target)
}

fn accept(world: &mut GameLogic, source: ObjectId, target: ObjectId, time: f32) {
    assert_eq!(
        current_gameworld_authority(),
        GameWorldAuthority::DEFAULT_OFF
    );
    assert!(!crate::gameworld_shadow::gameworld_fire_spawn_authority_live());
    let unit = &world.objects[&source];
    let victim = &world.objects[&target];
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

#[test]
fn accepted_queue_same_id_normal_fire_cannot_materialize_in_other_world() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut first, source, target) = world(FIRST_WEAPON);
    accept(&mut first, source, target, 10.0);
    // Constructor is deliberately after acceptance: it must neither clear
    // the first shot nor adopt it into the newly admitted same-ID roster.
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    assert_eq!((source, target), (other_source, other_target));
    let first_hp = first.objects[&target].health.current;
    let second_hp = second.objects[&other_target].health.current;
    second.drain_pending_projectiles_into_combat();
    assert_eq!(
        second.objects[&other_target].health.current, second_hp,
        "ordinary foreign drain must not steal a normally accepted shot"
    );
    accept(&mut second, other_source, other_target, 10.0);
    first.drain_pending_projectiles_into_combat();
    assert_eq!(first.objects[&target].health.current, first_hp - 17.0);
    second.drain_pending_projectiles_into_combat();
    assert_eq!(
        second.objects[&other_target].health.current,
        second_hp - 13.0
    );
    first.drain_pending_projectiles_into_combat();
    second.drain_pending_projectiles_into_combat();
    assert_eq!(first.objects[&target].health.current, first_hp - 17.0);
    assert_eq!(
        second.objects[&other_target].health.current,
        second_hp - 13.0
    );
}

#[test]
fn accepted_queue_reset_reuse_discards_only_reset_worlds_unmaterialized_shot() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut first, source, target) = world(FIRST_WEAPON);
    let (mut second, other_source, other_target) = world(SECOND_WEAPON);
    assert_eq!((source, target), (other_source, other_target));
    accept(&mut first, source, target, 10.0);
    accept(&mut second, other_source, other_target, 10.0);
    let second_hp = second.objects[&other_target].health.current;
    first.reset();
    let (reused_source, reused_target) = populate(&mut first, FIRST_WEAPON);
    assert_eq!((reused_source, reused_target), (source, target));
    assert_eq!(
        first.objects[&reused_source]
            .weapon_discharge_marker()
            .sequence,
        0
    );
    let reused_hp = first.objects[&reused_target].health.current;
    first.drain_pending_projectiles_into_combat();
    assert_eq!(
        first.objects[&reused_target].health.current, reused_hp,
        "ID reuse cannot replay reset world's accepted shot"
    );
    second.drain_pending_projectiles_into_combat();
    assert_eq!(
        second.objects[&other_target].health.current,
        second_hp - 13.0,
        "another world's reset cannot clear this accepted shot"
    );
}

#[test]
fn accepted_queue_fifo_repeated_shots_keep_frozen_source_after_retirement() {
    let _serial = combat_test_guard();
    let _restore = RestoreInputs::install();
    let (mut world, source, target) = world(FIRST_WEAPON);
    let other_source = admit(
        &mut world,
        "AcceptedQueueOtherShooter",
        Some(SECOND_WEAPON),
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
    accept(&mut world, source, target, 10.0);
    accept(&mut world, other_source, target, 10.0);
    accept(&mut world, source, target, 12.0);
    let cues = world.take_weapon_discharges_for_presentation();
    assert_eq!(
        cues.iter().map(|cue| cue.source).collect::<Vec<_>>(),
        vec![source, other_source, source]
    );
    assert_eq!(
        cues.iter().map(|cue| cue.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    world.destroy_object(source);
    world.process_destroy_list_if_needed();
    assert!(
        !world.objects.contains_key(&source),
        "retire through actual world destruction"
    );
    let hp = world.objects[&target].health.current;
    // Same production materialization function as the ordinary driving drain.
    // Inspect its actual authored OCL records before world effect execution;
    // this control does not claim unresolved OCL definitions were executed.
    drain_pending_projectiles(&mut world.combat_system, &world.objects, world.frame);
    let fire = world.combat_system.take_fire_ocl();
    assert_eq!(
        fire.iter()
            .map(|event| event.shooter_id)
            .collect::<Vec<_>>(),
        vec![source, other_source, source]
    );
    assert_eq!(
        fire.iter()
            .map(|event| event.fire_ocl_name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "OCL_AcceptedQueueFirst",
            "OCL_AcceptedQueueSecond",
            "OCL_AcceptedQueueFirst"
        ]
    );
    assert_eq!(fire[0].source_team, Team::USA);
    assert_eq!(fire[2].source_team, Team::USA);
    assert_eq!(fire[0].source_orientation, source_orientation);
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        world.frame,
        Some(&world.players),
    );
    assert_eq!(world.objects[&target].health.current, hp - 47.0);
    world.drain_pending_projectiles_into_combat();
    assert_eq!(world.objects[&target].health.current, hp - 47.0);
    assert!(world.combat_system.take_fire_ocl().is_empty());
}

#[path = "accepted_fire_frame_owner_tests.rs"]
mod accepted_fire_frame_owner_tests;

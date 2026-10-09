//! Clock ownership at the actual host materialization boundary. The global
//! coupled shadow log remains separate migration debt; accepted and materialized delayed
//! damage belongs to the accepting CombatSystem.
//! C++ JetAIUpdate.cpp:2057-2074 and Weapon.cpp:804-817,998-1063.

use super::*;
use crate::game_logic::combat::tests::{combat_test_guard, lifecycle_test_pending_projectile};
use crate::game_logic::combat::*;
use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

struct RestoreCombatInputs {
    frame: u32,
    authority: GameWorldAuthority,
    weapon_store: Option<gamelogic::weapon::WeaponStore>,
}

impl RestoreCombatInputs {
    fn install() -> Self {
        let saved = Self {
            frame: crate::game_logic::host_historic_bonus::logic_frame(),
            authority: current_gameworld_authority(),
            // Finite projectileless fire also appends a canonical delayed
            // entry. Preserve the real store rather than leaking that entry
            // into another test, including when the RED assertion panics.
            weapon_store: gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok(),
        };
        gameworld_authority::publish_gameworld_authority(GameWorldAuthority::DEFAULT_OFF);
        saved
    }
}

impl Drop for RestoreCombatInputs {
    fn drop(&mut self) {
        if let Some(previous) = self.weapon_store.take() {
            gamelogic::weapon::with_weapon_store_mut(|store| *store = previous)
                .expect("restore the previously initialized weapon store");
        } else {
            gamelogic::weapon::shutdown_weapon_store().expect("restore absent weapon store");
        }
        gameworld_authority::publish_gameworld_authority(self.authority);
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
    }
}

fn admit(world: &mut GameLogic, name: &str, team: Team, pos: Vec3, kind: KindOf) -> ObjectId {
    let mut template = ThingTemplate::new(name);
    template.set_health(100.0);
    template.add_kind_of(kind);
    template.add_kind_of(KindOf::Attackable);
    world.templates.insert(template.name.clone(), template);
    world
        .create_object(name, team, pos)
        .expect("admit into driving game")
}

fn aurora_world(frame: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = frame;
    let source = admit(
        &mut world,
        "HqVcv4eShooter",
        Team::China,
        Vec3::ZERO,
        KindOf::Infantry,
    );
    let target = admit(
        &mut world,
        "AmericaJetAurora",
        Team::USA,
        Vec3::new(120.0, 50.0, 0.0),
        KindOf::Aircraft,
    );
    let aurora = world.objects.get_mut(&target).unwrap();
    aurora.set_ai_state(AIState::Idle);
    aurora.status.airborne_target = true;
    aurora.status.attacking = true;
    // Drive the named retail Aurora residual, not a manually assigned expiry.
    aurora.tick_jet_ai_update(frame);
    aurora.status.attacking = false;
    (world, source, target)
}

fn queue_shot(
    world: &mut GameLogic,
    source: ObjectId,
    target: ObjectId,
    projectile: &str,
    speed: f32,
) {
    let mut pending = lifecycle_test_pending_projectile(
        projectile,
        Some(target),
        world.objects[&target].get_position(),
    );
    pending.shooter_id = source;
    pending.shooter_pos = world.objects[&source].get_position();
    pending.source_context = None;
    pending.speed = speed;
    pending.speed_unit = ProjectileSpeedUnit::DistancePerLogicFrame;
    pending.splash_radius = 0.0;
    pending.damage = 40.0;
    pending.damage_type = DamageType::Bullet;
    pending.detonation_fx_name.clear();
    queue_projectile_direct(&mut world.combat_system, pending);
}

/// Exercise the actual materialization boundary without publishing the clock.
fn materialize(world: &mut GameLogic, frame: u32) {
    drain_pending_projectiles(&mut world.combat_system, &world.objects, frame);
}

#[test]
fn active_aurora_materializes_using_driving_frame_not_other_world_clock() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = aurora_world(100);
    let (second, other_source, other_target) = aurora_world(300);
    assert_eq!(source, other_source);
    assert_eq!(target, other_target);
    let aurora = &first.objects[&target];
    let offset = aurora
        .get_sneaky_targeting_offset(first.frame)
        .expect("active Aurora");
    assert!((offset.length() - 20.0).abs() < 0.001);
    let expected = aurora.get_position() + offset;
    crate::game_logic::host_historic_bonus::set_logic_frame(second.frame);
    queue_shot(&mut first, source, target, "PatriotMissile", 10.0);
    let frame = first.frame;
    materialize(&mut first, frame);
    let shots = first.combat_system.projectiles_snapshot();
    assert_eq!(shots.len(), 1);
    assert_eq!(
        shots[0].target_position, expected,
        "aim uses this game's active offset"
    );
    assert_eq!(
        shots[0].target_id, None,
        "C++ converts sneaky victim into coordinate target"
    );
    assert_eq!(second.combat_system.projectile_count(), 0);
}

#[test]
fn expired_aurora_materializes_using_driving_frame_not_other_world_clock() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = aurora_world(100);
    let (second, other_source, other_target) = aurora_world(100);
    assert_eq!(source, other_source);
    assert_eq!(target, other_target);
    // Query at the exact expiry before another AI update. C++ uses strict <,
    // so the old expiry value must not revive immunity under a foreign clock.
    first.frame = 160;
    assert!(
        first.objects[&target]
            .get_sneaky_targeting_offset(first.frame)
            .is_none()
    );
    assert!(
        first.objects[&target]
            .get_sneaky_targeting_offset(second.frame)
            .is_some()
    );
    let expected = first.objects[&target].get_position();
    crate::game_logic::host_historic_bonus::set_logic_frame(second.frame);
    queue_shot(&mut first, source, target, "PatriotMissile", 10.0);
    let frame = first.frame;
    materialize(&mut first, frame);
    let shots = first.combat_system.projectiles_snapshot();
    assert_eq!(shots.len(), 1);
    assert_eq!(
        shots[0].target_position, expected,
        "expired offset leaves live aim unchanged"
    );
    assert_eq!(shots[0].target_id, Some(target));
    assert_eq!(second.combat_system.projectile_count(), 0);
}

fn projectileless_world(frame: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.frame = frame;
    let source = admit(
        &mut world,
        "HqVcv4eShooter",
        Team::China,
        Vec3::ZERO,
        KindOf::Infantry,
    );
    let target = admit(
        &mut world,
        "HqVcv4eVictim",
        Team::GLA,
        Vec3::new(30.0, 0.0, 0.0),
        KindOf::Infantry,
    );
    (world, source, target)
}

fn apply(world: &mut GameLogic, frame: u32) {
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        frame,
        Some(&world.players),
        &mut world.health_events,
    );
}

#[test]
fn delayed_projectileless_due_frame_belongs_to_materializing_world() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = projectileless_world(100);
    let (second, other_source, other_target) = projectileless_world(300);
    assert_eq!(source, other_source);
    assert_eq!(target, other_target);
    let hp = first.objects[&target].health.current;
    let other_hp = second.objects[&other_target].health.current;
    crate::game_logic::host_historic_bonus::set_logic_frame(second.frame);
    queue_shot(&mut first, source, target, "", 10.0);
    let frame = first.frame;
    materialize(&mut first, frame);
    assert_eq!(first.combat_system.projectile_count(), 0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&first.combat_system),
        1
    );
    apply(&mut first, 102);
    assert_eq!(
        first.objects[&target].health.current, hp,
        "three-frame travel is not due early"
    );
    apply(&mut first, 103);
    assert!(
        first.objects[&target].health.current < hp,
        "damage is due at owner frame100+3"
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&first.combat_system),
        0
    );
    assert_eq!(second.objects[&other_target].health.current, other_hp);
}

#[test]
fn immediate_projectileless_damage_belongs_to_materializing_world_frame() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = projectileless_world(100);
    let (second, other_source, other_target) = projectileless_world(300);
    assert_eq!(source, other_source);
    assert_eq!(target, other_target);
    let hp = first.objects[&target].health.current;
    let other_hp = second.objects[&other_target].health.current;
    crate::game_logic::host_historic_bonus::set_logic_frame(second.frame);
    // Thirty units / one hundred units per frame is sub-frame travel.
    queue_shot(&mut first, source, target, "", 100.0);
    let frame = first.frame;
    materialize(&mut first, frame);
    assert_eq!(first.combat_system.projectile_count(), 0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&first.combat_system),
        1
    );
    apply(&mut first, frame);
    assert!(
        first.objects[&target].health.current < hp,
        "sub-frame shot applies this logic frame"
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&first.combat_system),
        0
    );
    assert_eq!(second.objects[&other_target].health.current, other_hp);
}

#[path = "delayed_queue_owner_tests.rs"]
mod delayed_queue_owner_tests;

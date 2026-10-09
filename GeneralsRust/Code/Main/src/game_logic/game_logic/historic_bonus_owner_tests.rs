//! C++ Weapon.cpp:1169-1251 history belongs to the weapon template in one
//! match, shared across firing objects; reset clears that match's templates
//! (Weapon.cpp:321-324,1615-1651). Exercise actual impact and Helix consumers.

use super::*;
use crate::game_logic::combat::tests::{
    combat_test_guard, lifecycle_test_pending_projectile, queue_delayed_impact_for_test,
};
use crate::game_logic::combat::*;
use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};

struct RetentionRules(u32);

impl RetentionRules {
    fn install(limit: u32) -> Self {
        let mut rules = game_engine::common::global_data::write();
        let previous = rules.historic_damage_limit;
        rules.historic_damage_limit = limit;
        Self(previous)
    }
}

impl Drop for RetentionRules {
    fn drop(&mut self) {
        game_engine::common::global_data::write().historic_damage_limit = self.0;
    }
}

fn admit_source(world: &mut GameLogic, team: Team) -> ObjectId {
    let mut template = ThingTemplate::new("HistoricOwnershipSource");
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    world
        .create_object("HistoricOwnershipSource", team, Vec3::ZERO)
        .expect("source is admitted by this GameLogic")
}

/// Configure the same authored impact facts carried by PendingProjectile,
/// then drive the production ground-impact branch. No history helper calls.
fn impact(
    world: &mut GameLogic,
    source: ObjectId,
    key: &str,
    pos: Vec3,
    count: i32,
    time_frames: u32,
) {
    let weapon = Weapon::default();
    let id = world
        .combat_system
        .fire_projectile(pos, pos, &weapon, source, None, 0.0);
    let projectile = world.combat_system.projectile_mut(id).unwrap();
    projectile.historic_weapon_key = key.into();
    projectile.historic_bonus_count = count;
    projectile.historic_bonus_time_frames = time_frames;
    projectile.historic_bonus_radius = 20.0;
    projectile.historic_bonus_weapon = "FirestormSmallCreationWeapon".into();
    world.combat_system.update_projectiles_with_relationships(
        1.0 / 30.0,
        &mut world.objects,
        None,
        world.frame,
        Some(&world.players),
        Some(&world.team_factory),
        &mut world.health_events,
    );
    assert_eq!(
        world.combat_system.projectile_count(),
        0,
        "real impact consumed projectile"
    );
}

fn zones(world: &GameLogic) -> usize {
    world.helix_napalm().active_count()
}

#[test]
fn historic_impacts_alternate_same_ids_without_combining_matches() {
    let _serial = combat_test_guard();
    let _rules = RetentionRules::install(90);
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    let first_source = admit_source(&mut first, Team::China);
    let second_source = admit_source(&mut second, Team::GLA);
    assert_eq!(first_source, second_source);
    first.frame = 100;
    second.frame = 300;
    let pos = Vec3::new(10.0, 4.0, 10.0);
    const KEY: &str = "HqVrkh8AlternatingInferno";

    for _ in 0..2 {
        impact(&mut first, first_source, KEY, pos, 3, 90);
        impact(&mut second, second_source, KEY, pos, 3, 90);
        first.drain_historic_bonus_firestorms();
        second.drain_historic_bonus_firestorms();
        assert_eq!(
            zones(&first),
            0,
            "other match cannot provide qualifying impacts"
        );
        assert_eq!(
            zones(&second),
            0,
            "other match cannot provide qualifying impacts"
        );
    }
    impact(&mut first, first_source, KEY, pos, 3, 90);
    second.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&second),
        0,
        "other consumer cannot drain first game's bonus"
    );
    first.drain_historic_bonus_firestorms();
    assert_eq!(zones(&first), 1);
    let first_zone = &first.helix_napalm().active_zones()[0];
    assert_eq!(first_zone.source_object, first_source);
    assert_eq!(first_zone.source_team, Team::China);
    assert_eq!(first_zone.activate_frame, 100);

    impact(&mut second, second_source, KEY, pos, 3, 90);
    second.drain_historic_bonus_firestorms();
    assert_eq!(zones(&second), 1);
    assert_eq!(
        second.helix_napalm().active_zones()[0].source_team,
        Team::GLA
    );
    assert_eq!(second.helix_napalm().active_zones()[0].activate_frame, 300);
    first.drain_historic_bonus_firestorms();
    assert_eq!(zones(&first), 1, "drain is one-shot");
}

#[test]
fn constructing_another_game_cannot_take_pending_historic_bonus() {
    let _serial = combat_test_guard();
    let _rules = RetentionRules::install(90);
    let mut first = GameLogic::new();
    let source = admit_source(&mut first, Team::China);
    first.frame = 100;
    const KEY: &str = "HqVrkh8PendingInferno";
    for _ in 0..3 {
        impact(&mut first, source, KEY, Vec3::new(10.0, 4.0, 10.0), 3, 90);
    }
    let mut second = GameLogic::new();
    assert_eq!(admit_source(&mut second, Team::GLA), source);
    second.frame = 500;
    second.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&second),
        0,
        "constructor and consumer leave other owner intact"
    );
    first.drain_historic_bonus_firestorms();
    assert_eq!(zones(&first), 1);
    assert_eq!(first.helix_napalm().active_zones()[0].activate_frame, 100);
}

#[test]
fn game_reset_clears_only_its_own_historic_samples_and_pending_bonus() {
    let _serial = combat_test_guard();
    let _rules = RetentionRules::install(90);
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    let first_source = admit_source(&mut first, Team::China);
    let second_source = admit_source(&mut second, Team::GLA);
    assert_eq!(first_source, second_source);
    first.frame = 100;
    second.frame = 200;
    let first_pos = Vec3::new(10.0, 4.0, 10.0);
    let second_pos = Vec3::new(100.0, 4.0, 10.0);
    const KEY: &str = "HqVrkh8ResetInferno";
    for _ in 0..2 {
        impact(&mut first, first_source, KEY, first_pos, 3, 90);
        impact(&mut second, second_source, KEY, second_pos, 3, 90);
    }
    // A separate template also has a queued bonus when the match resets.
    impact(
        &mut first,
        first_source,
        "HqVrkh8ResetPending",
        first_pos,
        1,
        90,
    );
    first.reset();
    first.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&first),
        0,
        "reset discards this owner's pending bonus"
    );
    let new_source = admit_source(&mut first, Team::China);
    assert_eq!(new_source, first_source);
    first.frame = 100;
    impact(&mut first, new_source, KEY, first_pos, 3, 90);
    first.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&first),
        0,
        "reused IDs cannot inherit pre-reset samples"
    );
    impact(&mut second, second_source, KEY, second_pos, 3, 90);
    second.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&second),
        1,
        "resetting another match preserves these samples"
    );
}

#[test]
fn delayed_impact_uses_consumer_frame_for_historic_expiration() {
    let _serial = combat_test_guard();
    let _rules = RetentionRules::install(90);
    let mut world = GameLogic::new();
    let source = admit_source(&mut world, Team::China);
    world.frame = 100;
    let pos = Vec3::new(10.0, 4.0, 10.0);
    const KEY: &str = "HqVrkh8DelayedInferno";
    impact(&mut world, source, KEY, pos, 2, 30);

    let pending = PendingProjectile {
        shooter_id: source,
        shooter_pos: pos,
        target_id: None,
        target_pos: Some(pos),
        historic_weapon_key: KEY.into(),
        historic_bonus_count: 2,
        historic_bonus_time_frames: 30,
        historic_bonus_radius: 20.0,
        historic_bonus_weapon: "FirestormSmallCreationWeapon".into(),
        ..lifecycle_test_pending_projectile("", None, pos)
    };
    queue_delayed_impact_for_test(&mut world.combat_system, 200, pending, pos);
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        199,
        Some(&world.players),
        &mut world.health_events,
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        1
    );
    world.frame = 200;
    apply_ready_projectileless_delayed_damage(
        &mut world.combat_system,
        &mut world.objects,
        world.frame,
        Some(&world.players),
        &mut world.health_events,
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        0
    );
    world.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&world),
        0,
        "old impact expired at the actual delayed-damage frame"
    );
    impact(&mut world, source, KEY, pos, 2, 30);
    world.drain_historic_bonus_firestorms();
    assert_eq!(
        zones(&world),
        1,
        "consumed delayed impact contributes at its own frame"
    );
    assert_eq!(world.helix_napalm().active_zones()[0].activate_frame, 200);
}

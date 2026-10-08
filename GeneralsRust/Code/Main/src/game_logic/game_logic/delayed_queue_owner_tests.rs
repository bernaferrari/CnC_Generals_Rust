//! Actual host shot materialization and due-damage consumption, not direct
//! private delayed-queue insertion. CPP Weapon.cpp:998-1075,1589-1655.
//! The coupled shadow log and native WeaponStore mirror remain ambient;
//! this evidence covers only the materialized CombatSystem queue owner.
use super::*;

fn queue_authored_shot(
    world: &mut GameLogic,
    source: ObjectId,
    target: ObjectId,
    damage: f32,
    speed: f32,
) {
    use game_engine::common::ini::ini_weapon::IniWeapon;
    let name = format!("HqDelayedQueueOwner{damage}_{speed}");
    let authored_weapon_speed_per_second = speed * 30.0_f32;
    let cpp_seconds_per_logic_frame = 1.0_f32 / 30.0_f32;
    let expected_cpp_speed_per_frame =
        authored_weapon_speed_per_second * cpp_seconds_per_logic_frame;
    let properties = HashMap::from([
        ("PrimaryDamage".to_string(), damage.to_string()),
        (
            "WeaponSpeed".to_string(),
            authored_weapon_speed_per_second.to_string(),
        ),
        ("ProjectileObject".to_string(), "NONE".to_string()),
        ("DamageType".to_string(), "SMALL_ARMS".to_string()),
    ]);
    let authored = IniWeapon::parse_weapon_template_block(name.clone().into(), properties)
        .expect("actual Common Weapon field parser");
    assert_eq!(authored.primary_damage, damage);
    assert_eq!(
        authored.projectile_speed, expected_cpp_speed_per_frame,
        "WeaponSpeed parses as units/sec times C++'s rounded 1/30f",
    );
    assert!(
        authored
            .effects
            .projectile_object
            .as_str()
            .eq_ignore_ascii_case("NONE")
    );
    // Actual materialization consults the real weapon store for laser status
    // and mirrors finite-speed damage there. Register exactly these authored
    // facts rather than populating the private live ready queue.
    // The surrounding fixture restores an originally absent store. Main's
    // independent bootstrap seed flag may already be set by an earlier test;
    // initialize this real owned store explicitly before registering authored
    // rules instead of mutating that production flag or an unrelated catalog.
    gamelogic::initialize_weapon_store().expect("initialize actual fixture weapon store");
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(name.clone());
        template.weapon_speed = authored.projectile_speed;
        template.primary_damage = authored.primary_damage;
        template.projectile_name.clear();
        store.add_weapon_template(template);
    })
    .expect("owned fixture store is installed");
    let mut pending = lifecycle_test_pending_projectile(
        authored.effects.projectile_object.as_str(),
        Some(target),
        world.objects[&target].get_position(),
    );
    pending.shooter_id = source;
    pending.shooter_pos = world.objects[&source].get_position();
    pending.source_context = None;
    pending.damage = authored.primary_damage;
    pending.speed = authored.projectile_speed;
    pending.speed_unit = ProjectileSpeedUnit::DistancePerLogicFrame;
    pending.damage_type = DamageType::Bullet;
    pending.splash_radius = 0.0;
    pending.secondary_damage = 0.0;
    pending.detonation_fx_name.clear();
    pending.historic_weapon_key = name;
    queue_projectile_direct(&mut world.combat_system, pending);
    let frame = world.frame;
    materialize(world, frame);
    assert_eq!(
        world.combat_system.projectile_count(),
        0,
        "no dummy projectile"
    );
}

#[test]
fn delayed_same_id_interleaved_consumers_cannot_steal_materialized_shots() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = projectileless_world(100);
    queue_authored_shot(&mut first, source, target, 40.0, 10.0);
    // A new constructor must not clear or adopt the first owner's pending shot.
    let (mut second, other_source, other_target) = projectileless_world(200);
    assert_eq!(source, other_source);
    assert_eq!(target, other_target);
    let hp_first = first.objects[&target].health.current;
    let hp_second = second.objects[&other_target].health.current;
    apply(&mut second, 203);
    assert_eq!(
        second.objects[&other_target].health.current, hp_second,
        "foreign due consumer cannot take first world's accepted materialization"
    );
    queue_authored_shot(&mut second, other_source, other_target, 10.0, 10.0);
    apply(&mut first, 102);
    assert_eq!(first.objects[&target].health.current, hp_first);
    apply(&mut first, 103);
    assert_eq!(first.objects[&target].health.current, hp_first - 40.0);
    apply(&mut first, 203);
    assert_eq!(
        first.objects[&target].health.current,
        hp_first - 40.0,
        "first cannot steal second's due shot either"
    );
    apply(&mut second, 202);
    assert_eq!(second.objects[&other_target].health.current, hp_second);
    apply(&mut second, 203);
    assert_eq!(
        second.objects[&other_target].health.current,
        hp_second - 10.0
    );
    apply(&mut second, 203);
    assert_eq!(
        second.objects[&other_target].health.current,
        hp_second - 10.0,
        "consumed shot never applies twice"
    );
}

#[test]
fn reset_discards_own_delayed_shot_but_preserves_other_match_shot() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = projectileless_world(100);
    let (mut second, other_source, other_target) = projectileless_world(100);
    assert_eq!(target, other_target);
    queue_authored_shot(&mut first, source, target, 40.0, 10.0);
    queue_authored_shot(&mut second, other_source, other_target, 10.0, 10.0);
    first.reset();
    let recreated_source = admit(
        &mut first,
        "HqVcv4eShooter",
        Team::China,
        Vec3::ZERO,
        KindOf::Infantry,
    );
    let recreated_target = admit(
        &mut first,
        "HqVcv4eVictim",
        Team::GLA,
        Vec3::new(30.0, 0.0, 0.0),
        KindOf::Infantry,
    );
    assert_eq!(recreated_source, source);
    assert_eq!(recreated_target, target);
    let hp = first.objects[&target].health.current;
    apply(&mut first, 103);
    assert_eq!(
        first.objects[&target].health.current, hp,
        "CPP reset deletes pending delayed damage before ObjectID reuse"
    );
    let other_hp = second.objects[&other_target].health.current;
    apply(&mut second, 103);
    assert_eq!(
        second.objects[&other_target].health.current,
        other_hp - 10.0,
        "reset of another match does not clear this owner's pending damage"
    );
}

#[test]
fn future_entry_survives_due_entry_and_each_applies_once() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut world, source, target) = projectileless_world(100);
    let hp = world.objects[&target].health.current;
    queue_authored_shot(&mut world, source, target, 10.0, 3.0); // due110
    queue_authored_shot(&mut world, source, target, 40.0, 10.0); // due103
    apply(&mut world, 102);
    assert_eq!(world.objects[&target].health.current, hp);
    apply(&mut world, 103);
    assert_eq!(world.objects[&target].health.current, hp - 40.0);
    apply(&mut world, 103);
    assert_eq!(world.objects[&target].health.current, hp - 40.0);
    apply(&mut world, 109);
    assert_eq!(world.objects[&target].health.current, hp - 40.0);
    apply(&mut world, 110);
    assert_eq!(world.objects[&target].health.current, hp - 50.0);
}

#[test]
fn subframe_materialization_stays_with_owner_at_same_frame() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let (mut first, source, target) = projectileless_world(100);
    let (mut second, _, other_target) = projectileless_world(100);
    queue_authored_shot(&mut first, source, target, 40.0, 100.0);
    let hp_first = first.objects[&target].health.current;
    let hp_second = second.objects[&other_target].health.current;
    apply(&mut second, 100);
    assert_eq!(
        second.objects[&other_target].health.current, hp_second,
        "same-frame consumer remains owner-scoped"
    );
    apply(&mut first, 100);
    assert_eq!(first.objects[&target].health.current, hp_first - 40.0);
}

#[test]
fn delayed_deadline_wraps_cpp_u32_and_is_due_in_current_high_frame() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let frame = u32::MAX - 1;
    let (mut world, source, target) = projectileless_world(frame);
    let hp = world.objects[&target].health.current;
    // Materialization receives the driving frame; the compatibility clock
    // differs deliberately. Thirty units / ten units per frame = three frames.
    crate::game_logic::host_historic_bonus::set_logic_frame(37);
    queue_authored_shot(&mut world, source, target, 40.0, 10.0);
    let mirrored =
        gamelogic::weapon::with_weapon_store(|store| store.delayed_damage_snapshot_residual())
            .expect("actual materialization mirror store");
    assert_eq!(mirrored.len(), 1);
    assert_eq!(world.objects[&target].health.current, hp);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        1
    );

    // CPP Weapon.cpp1057 stores unsigned (MAX-1)+3 = 1; update1594
    // compares curFrame >= due directly, so the shot is due this high frame.
    // First RED must be actual application, not only the mirror deadline.
    apply(&mut world, frame);
    assert_eq!(
        world.objects[&target].health.current,
        hp - 40.0,
        "unsigned wrapped deadline is already due under the CPP current>=due comparison"
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        0
    );
    assert_eq!(
        mirrored[0].delay_damage_frame, 1,
        "native WeaponStore receives the same frozen wrapped deadline"
    );
    assert_eq!(mirrored[0].delay_source_id, source.0);
    assert_eq!(mirrored[0].delay_intended_victim_id, target.0);
    apply(&mut world, frame);
    apply(&mut world, 1);
    assert_eq!(
        world.objects[&target].health.current,
        hp - 40.0,
        "consumed wrapped shot cannot apply again at either clock value"
    );
}

#[test]
fn delayed_deadline_before_rollover_waits_until_normal_due_and_applies_once() {
    let _serial = combat_test_guard();
    let _restore = RestoreCombatInputs::install();
    let frame = u32::MAX - 4;
    let (mut world, source, target) = projectileless_world(frame);
    let hp = world.objects[&target].health.current;
    crate::game_logic::host_historic_bonus::set_logic_frame(37);
    queue_authored_shot(&mut world, source, target, 40.0, 10.0);
    let mirrored =
        gamelogic::weapon::with_weapon_store(|store| store.delayed_damage_snapshot_residual())
            .expect("actual materialization mirror store");
    assert_eq!(mirrored.len(), 1);
    assert_eq!(mirrored[0].delay_damage_frame, u32::MAX - 1);
    assert_eq!(mirrored[0].delay_source_id, source.0);
    assert_eq!(mirrored[0].delay_intended_victim_id, target.0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        1
    );
    apply(&mut world, frame);
    apply(&mut world, u32::MAX - 2);
    assert_eq!(
        world.objects[&target].health.current, hp,
        "non-overflowing three-frame travel still waits until its exact deadline"
    );
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        1
    );
    apply(&mut world, u32::MAX - 1);
    assert_eq!(world.objects[&target].health.current, hp - 40.0);
    assert_eq!(
        live_projectileless_delayed_count_for_test(&world.combat_system),
        0
    );
    apply(&mut world, u32::MAX - 1);
    apply(&mut world, u32::MAX);
    assert_eq!(
        world.objects[&target].health.current,
        hp - 40.0,
        "ordinary high-frame shot applies exactly once"
    );
}

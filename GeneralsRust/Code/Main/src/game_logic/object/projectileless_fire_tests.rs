use super::*;
use crate::game_logic::combat::{
    CombatSystem, apply_ready_projectileless_delayed_damage, drain_pending_projectiles,
    live_projectileless_delayed_count_for_test, pending_projectile_queue_len_for_test,
};

// This legacy Object fire boundary still uses the compatibility store and
// accepted commands. Each test owns its queue; restore the other inputs on unwind.
struct RestoreInputs {
    store: Option<gamelogic::weapon::WeaponStore>,
    authority: crate::game_logic::game_logic::GameWorldAuthority,
    frame: u32,
}

impl RestoreInputs {
    fn install() -> Self {
        let restore = Self {
            store: gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok(),
            authority: crate::game_logic::game_logic::current_gameworld_authority(),
            frame: crate::game_logic::host_historic_bonus::logic_frame(),
        };
        crate::game_logic::game_logic::gameworld_authority::publish_gameworld_authority(
            crate::game_logic::game_logic::GameWorldAuthority::DEFAULT_OFF,
        );
        // The independent bootstrap seed flag may survive a previous fixture
        // restoring an absent store. Initialize the real store idempotently.
        gamelogic::initialize_weapon_store().expect("fixture weapon store");
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        restore
    }
}

impl Drop for RestoreInputs {
    fn drop(&mut self) {
        if let Some(store) = self.store.take() {
            gamelogic::weapon::with_weapon_store_mut(|current| *current = store)
                .expect("restore exact compatibility weapon store");
        } else {
            gamelogic::weapon::shutdown_weapon_store().expect("restore absent store");
        }
        crate::game_logic::game_logic::gameworld_authority::publish_gameworld_authority(
            self.authority,
        );
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
    }
}

fn victim(name: &str, position: Vec3) -> Object {
    let mut template = ThingTemplate::new(name);
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    template.add_kind_of(KindOf::Attackable);
    let mut victim = Object::new(template, ObjectId(2), Team::GLA);
    victim.set_position(position);
    victim
}

#[test]
fn fire_at_keeps_gattling_store_type() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    let mut combat = CombatSystem::new();
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    // Use the actual authored parser rather than rely on an already-seeded
    // catalog. Authored NONE is preserved as a raw native name; the live
    // empty-or-NONE projectile predicate selects projectileless materialization.
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(
            "Weapon GattlingTankGun\n PrimaryDamage = 15\n AttackRange = 150\n DamageType = GATTLING\n ProjectileObject = NONE\n WeaponSpeed = 999999\n DelayBetweenShots = 400\nEnd\n",
        ),
        1,
    );
    gamelogic::weapon::with_weapon_store(|store| {
        let definition = store
            .find_weapon_template("GattlingTankGun")
            .expect("authored weapon");
        assert_eq!(definition.damage_type, gamelogic::DamageType::Gattling);
        assert!(definition.projectile_name.eq_ignore_ascii_case("NONE"));
        assert_eq!(definition.primary_damage, 15.0);
        assert_eq!(definition.attack_range, 150.0);
        assert_eq!(definition.weapon_speed, 999_999.0 / 30.0);
        assert_eq!(definition.min_delay_between_shots, 12);
        assert_eq!(definition.max_delay_between_shots, 12);
        assert_eq!(definition.clip_size, 0);
    })
    .expect("real weapon store");
    let mut template = ThingTemplate::new("ChinaTankGattling");
    template.set_primary_weapon_name("GattlingTankGun");
    template.set_health(100.0);
    template.add_kind_of(KindOf::Vehicle);
    template.add_kind_of(KindOf::Attackable);
    let mut attacker = Object::new(template, ObjectId(1), Team::USA);
    // Primitive Object::new leaves slots empty; normal GameLogic admission
    // resolves this exact named store row separately. Use that same converter.
    attacker.weapon = Some(
        ThingTemplate::weapon_from_store("GattlingTankGun")
            .expect("bind the exact authored Gattling weapon, without fallback"),
    );
    attacker
        .weapon
        .as_mut()
        .expect("actual store-bound weapon")
        .last_fire_time = -10.0;
    let target_position = Vec3::new(100.0, 0.0, 0.0);
    attacker.prev_victim_pos = Some(target_position);
    assert!(attacker.fire_at(ObjectId(2), 1.0, 3, &mut combat, false));
    assert_eq!(
        crate::game_logic::combat::last_pending_projectile_damage_type_for_test(&combat),
        Some(crate::game_logic::combat::DamageType::Gattling),
        "the accepted command retains the authored damage classification",
    );
    assert_eq!(pending_projectile_queue_len_for_test(&combat), 1);
    let mut objects = HashMap::from([
        (ObjectId(1), attacker),
        (
            ObjectId(2),
            victim("ProjectilelessGattlingVictim", target_position),
        ),
    ]);
    drain_pending_projectiles(&mut combat, &objects, 3);
    assert_eq!(pending_projectile_queue_len_for_test(&combat), 0);
    assert_eq!(
        combat.projectile_count(),
        0,
        "C++ Weapon.cpp:998-1075: empty ProjectileObject never creates a flying projectile"
    );
    assert_eq!(live_projectileless_delayed_count_for_test(&combat), 1);
    assert_eq!(objects[&ObjectId(2)].health.current, 100.0);
    apply_ready_projectileless_delayed_damage(
        &mut combat,
        &mut objects,
        3,
        None,
        &mut health_events,
    );
    assert_eq!(objects[&ObjectId(2)].health.current, 85.0);
    assert_eq!(objects[&ObjectId(1)].health.current, 100.0);
    assert_eq!(live_projectileless_delayed_count_for_test(&combat), 0);
    apply_ready_projectileless_delayed_damage(
        &mut combat,
        &mut objects,
        3,
        None,
        &mut health_events,
    );
    assert_eq!(objects[&ObjectId(2)].health.current, 85.0);
    assert_eq!(combat.projectile_count(), 0);
}

#[test]
fn fire_at_projectileless_queues_leftover_delayed_damage() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    let mut combat = CombatSystem::new();
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let _restore = RestoreInputs::install();
    const NAME: &str = "__RustLiveProjectilelessDelay";
    gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        template.weapon_speed = 10.0;
        template.min_weapon_speed = 0.0;
        template.projectile_name.clear();
        template.primary_damage = 20.0;
        template.attack_range = 200.0;
        store.add_weapon_template(template);
    })
    .expect("register real weapon definition");
    let mut template = ThingTemplate::new("DelayShooter");
    template.set_primary_weapon_name(NAME);
    template.set_health(100.0);
    template.add_kind_of(KindOf::Attackable);
    let mut attacker = Object::new(template, ObjectId(1), Team::USA);
    attacker.set_position(Vec3::ZERO);
    let target_position = Vec3::new(100.0, 0.0, 0.0);
    attacker.prev_victim_pos = Some(target_position);
    attacker.weapon = Some(Weapon {
        damage: 20.0,
        range: 200.0,
        projectile_speed: 10.0,
        last_fire_time: -10.0,
        ..Weapon::default()
    });
    assert!(attacker.fire_at(ObjectId(2), 1.0, 3, &mut combat, false));
    // Preserve the original immediate compatibility identity assertion BEFORE
    // materialization appends its separate native mirror entry. This does not
    // assert that the remaining compatibility mirror executes only once.
    let snapshots =
        gamelogic::weapon::with_weapon_store(|store| store.delayed_damage_snapshot_residual())
            .expect("leftover WeaponStore");
    assert!(
        snapshots.iter().any(|snapshot| {
            snapshot.weapon_name == NAME
                && snapshot.delay_source_id == 1
                && snapshot.delay_intended_victim_id == 2
        }),
        "leftover WeaponStore must queue source/victim identity: {snapshots:?}"
    );
    assert_eq!(
        pending_projectile_queue_len_for_test(&combat),
        1,
        "one accepted shot record is not a flying projectile"
    );
    let mut objects = HashMap::from([
        (ObjectId(1), attacker),
        (
            ObjectId(2),
            victim("ProjectilelessDelayVictim", target_position),
        ),
    ]);
    drain_pending_projectiles(&mut combat, &objects, 3);
    assert_eq!(pending_projectile_queue_len_for_test(&combat), 0);
    assert_eq!(
        combat.projectile_count(),
        0,
        "C++ empty ProjectileObject must never create a dummy render projectile"
    );
    assert_eq!(live_projectileless_delayed_count_for_test(&combat), 1);
    assert_eq!(objects[&ObjectId(2)].health.current, 100.0);
    // C++ Weapon.cpp:1055-1063 ceil travel frames: 100/10, due at frame3+10.
    apply_ready_projectileless_delayed_damage(
        &mut combat,
        &mut objects,
        12,
        None,
        &mut health_events,
    );
    assert_eq!(objects[&ObjectId(2)].health.current, 100.0);
    apply_ready_projectileless_delayed_damage(
        &mut combat,
        &mut objects,
        13,
        None,
        &mut health_events,
    );
    assert_eq!(objects[&ObjectId(2)].health.current, 80.0);
    assert_eq!(objects[&ObjectId(1)].health.current, 100.0);
    assert_eq!(live_projectileless_delayed_count_for_test(&combat), 0);
    assert_eq!(combat.projectile_count(), 0);
    apply_ready_projectileless_delayed_damage(
        &mut combat,
        &mut objects,
        13,
        None,
        &mut health_events,
    );
    assert_eq!(
        objects[&ObjectId(2)].health.current,
        80.0,
        "same due consumer cannot apply a consumed shot twice"
    );
}

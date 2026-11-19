//! Host fallback admission follows the exact native WeaponStore lifetime.
//! CPP Weapon.cpp:1485-1501,1627-1643 owns its catalog/reset; Main's fallback
//! memo is implementation metadata, never a separate gameplay authority.
use super::*;

#[cfg(not(target_arch = "wasm32"))]
fn isolated(test: &str, run: impl FnOnce()) {
    use std::io::Read;
    use std::process::Stdio;
    const CHILD: &str = "GENERALS_WEAPON_BOOTSTRAP_OWNER_CHILD";
    let module = module_path!().split_once("::").expect("crate prefix").1;
    let exact = format!("{module}::{test}");
    if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
        // The child runs on libtest's main thread with no owning WorldServices;
        // WeaponStore has no process bag, so bind one for the witness (hq-2qedt.151).
        if gamelogic::system::engine_stores::try_active_services().is_none() {
            gamelogic::system::engine_stores::install_services(
                gamelogic::system::engine_stores::new_world_services(),
            );
        }
        run();
        return;
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn actual catalog lifetime witness");
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).expect("drain witness output");
            bytes
        })
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            child.kill().expect("kill stalled witness");
            break child.wait().expect("reap witness");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(
        !timed_out,
        "catalog witness exceeded deadline: {exact}: {output:?}"
    );
    assert!(
        status.success(),
        "catalog witness failed: {exact}: {output:?}"
    );
    assert!(
        output[0].contains("running 1 test") && output[0].contains("1 passed; 0 failed"),
        "exact witness must execute one test: {output:?}"
    );
}

#[cfg(target_arch = "wasm32")]
fn isolated(_: &str, run: impl FnOnce()) {
    run();
}

// Native-only execution isolation contains the preexisting ambient catalog.
// The regression itself swaps two actual stores on ONE thread, not new worlds
// or private definition-map injection. Restore even after an assertion panic.
struct RestoreStore(Option<gamelogic::weapon::WeaponStore>);
impl RestoreStore {
    fn install() -> Self {
        let previous = gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok();
        gamelogic::initialize_weapon_store().expect("actual native store initialization");
        Self(previous)
    }
}
impl Drop for RestoreStore {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            // Soft restores: a failed query may leave the catalog unbound; never
            // panic in Drop (double-panic → SIGABRT hides later suite results).
            let _ = gamelogic::initialize_weapon_store();
            let _ = gamelogic::weapon::with_weapon_store_mut(|store| *store = previous);
        } else {
            let _ = gamelogic::weapon::shutdown_weapon_store();
        }
    }
}

fn prime_catalog() {
    assert!(
        ensure_host_weapon_store() > 0,
        "first real admission fills missing rules"
    );
    assert!(store_has(RANGER_PRIMARY_WEAPON));
    assert_eq!(
        ensure_host_weapon_store(),
        0,
        "completed exact catalog avoids reseeding"
    );
}

fn admit_ranger(expected_damage: f32) {
    let mut world = crate::game_logic::GameLogic::new();
    let mut rules = ThingTemplate::new("BootstrapOwnedRanger");
    rules.add_kind_of(KindOf::Infantry);
    rules.add_kind_of(KindOf::Attackable);
    rules.set_health(100.0);
    rules.set_primary_weapon_name(RANGER_PRIMARY_WEAPON);
    world.templates.insert(rules.name.clone(), rules);
    let id = world
        .create_object("BootstrapOwnedRanger", Team::USA, Vec3::ZERO)
        .expect("actual Main admission");
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .weapon
            .as_ref()
            .expect("actual bound weapon")
            .damage,
        expected_damage,
        "admission must bind this store's rules, not default host stats"
    );
}

#[test]
fn bootstrap_completion_follows_same_thread_store_replacement() {
    isolated(
        "bootstrap_completion_follows_same_thread_store_replacement",
        || {
            let _restore = RestoreStore::install();
            prime_catalog();
            let first = gamelogic::weapon::with_weapon_store_mut(std::mem::take).unwrap();
            assert!(
                !store_has(RANGER_PRIMARY_WEAPON),
                "new actual catalog has no old seed definition"
            );
            assert!(
                ensure_host_weapon_store() > 0,
                "a foreign completed catalog cannot suppress new admission"
            );
            assert!(store_has(RANGER_PRIMARY_WEAPON));
            admit_ranger(5.0);
            gamelogic::weapon::with_weapon_store_mut(|store| *store = first).unwrap();
            assert_eq!(
                ensure_host_weapon_store(),
                0,
                "restored completed catalog retains its own memo"
            );
            admit_ranger(5.0);
        },
    );
}

#[test]
fn bootstrap_completion_clears_on_actual_catalog_reset() {
    isolated(
        "bootstrap_completion_clears_on_actual_catalog_reset",
        || {
            let _restore = RestoreStore::install();
            prime_catalog();
            gamelogic::weapon::with_weapon_store_mut(|store| store.reset())
                .unwrap()
                .unwrap();
            assert!(
                !store_has(RANGER_PRIMARY_WEAPON),
                "actual reset removes native fallback rules"
            );
            assert!(
                ensure_host_weapon_store() > 0,
                "reset catalog must be eligible for admission again"
            );
            admit_ranger(5.0);
            assert_eq!(ensure_host_weapon_store(), 0);
        },
    );
}

/// Weapon.ini text for the Ranger primary with the stated numbers (C++
/// TheWeaponStore is filled from Weapon.ini before any Object is created).
fn ranger_rule_ini(damage: f32, range: f32) -> String {
    format!(
        "Weapon {RANGER_PRIMARY_WEAPON}\n  PrimaryDamage = {damage}\n  AttackRange = {range}\n  DelayBetweenShots = 1000\n  WeaponSpeed = 30000000\n  ClipSize = 8\n  DamageType = SMALL_ARMS\n  AntiGround = Yes\n  ProjectileObject = NONE\nEnd\n"
    )
}

/// A fresh world whose own WeaponStore snapshot is the parsed `weapon_ini`;
/// creates one Ranger from it and returns the world and the Ranger's id.
fn world_with_own_ranger_rule(weapon_ini: &str) -> (crate::game_logic::GameLogic, crate::game_logic::ObjectId) {
    let mut world = crate::game_logic::GameLogic::new();
    let mut rules = std::collections::HashMap::new();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut rules,
        weapon_ini,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    world
        .admit_receiving_weapon_snapshot(
            rules.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
        )
        .expect("weapon content admitted before live Objects");
    let mut template = ThingTemplate::new("BootstrapOwnedRanger");
    template.add_kind_of(KindOf::Infantry);
    template.add_kind_of(KindOf::Attackable);
    template.set_health(100.0);
    template.set_primary_weapon_name(RANGER_PRIMARY_WEAPON);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("BootstrapOwnedRanger", Team::USA, Vec3::ZERO)
        .expect("actual Main admission");
    (world, id)
}

// Each world owns its WeaponStore snapshot (C++ one TheWeaponStore per match):
// another world's rules, or the thread-local compatibility store, never bind.
#[test]
fn ordinary_admission_binds_each_worlds_own_weapon_snapshot() {
    let (first, first_id) = world_with_own_ranger_rule(&ranger_rule_ini(37.0, 123.0));
    let (second, second_id) = world_with_own_ranger_rule(&ranger_rule_ini(55.0, 77.0));
    let first_weapon = first.host_object(first_id).unwrap().weapon.as_ref().unwrap();
    let second_weapon = second.host_object(second_id).unwrap().weapon.as_ref().unwrap();
    assert_eq!((first_weapon.damage, first_weapon.range), (37.0, 123.0));
    assert_eq!((second_weapon.damage, second_weapon.range), (55.0, 77.0));
}

// An authored override of the Ranger rule is the rule the world binds, and a
// later Object of the same world keeps it.
#[test]
fn world_admission_retains_parsed_override_rule() {
    let (mut world, id) = world_with_own_ranger_rule(&ranger_rule_ini(37.0, 123.0));
    let weapon = world.host_object(id).unwrap().weapon.as_ref().unwrap();
    assert_eq!(weapon.damage, 37.0);
    assert_eq!(weapon.range, 123.0);
    assert_eq!(weapon.ammo, Some(8));
    let again = world
        .create_object("BootstrapOwnedRanger", Team::USA, Vec3::ZERO)
        .expect("second Ranger from the same snapshot");
    let weapon = world.host_object(again).unwrap().weapon.as_ref().unwrap();
    assert_eq!((weapon.damage, weapon.range, weapon.ammo), (37.0, 123.0, Some(8)));
}

#[test]
fn stream_query_bootstraps_absent_actual_catalog() {
    isolated("stream_query_bootstraps_absent_actual_catalog", || {
        let _restore = RestoreStore::install();
        let _ = gamelogic::weapon::shutdown_weapon_store();
        assert!(gamelogic::weapon::with_weapon_store(|_| ()).is_err());
        // No direct ensure/initializer after shutdown: this reached stream query
        // must admit the actual host fallback rules before reading their field.
        let stream = host_projectile_stream_name_for_weapon_name("DragonTankFlameWeapon");
        assert!(
            !stream.is_empty(),
            "cold query must resolve the admitted stream"
        );
        gamelogic::weapon::with_weapon_store(|store| {
            let rule = store
                .find_weapon_template("DragonTankFlameWeapon")
                .expect("query admitted the actual rule");
            assert_eq!(
                stream, rule.projectile_stream_name,
                "use the exact admitted field"
            );
            assert_eq!(rule.primary_damage, 10.0);
        })
        .expect("query initialized the actual native catalog");
    });
}

#[test]
fn tomahawk_seed_keeps_shot_and_clip_timing_separate() {
    isolated("tomahawk_seed_keeps_shot_and_clip_timing_separate", || {
        let _restore = RestoreStore::install();
        assert!(super::store::seed_known_host_weapons() > 0);
        gamelogic::weapon::with_weapon_store(|store| {
            let rule = store.find_weapon_template(TOMAHAWK_MISSILE_WEAPON).unwrap();
            // Retail Weapon.ini: 1 ms shot delay rounds up to one frame,
            // while the one-shot clip takes 7000 ms to reload.
            assert_eq!(rule.min_delay_between_shots, 1);
            assert_eq!(rule.max_delay_between_shots, 1);
            assert_eq!(rule.clip_size, 1);
            assert_eq!(rule.clip_reload_time, 210);
            assert_eq!(rule.pre_attack_delay, 8);
        }).unwrap();
    });
}

#[path = "ranger_primary_admission_tests.rs"]
mod ranger_primary_admission_tests;

#[path = "status_definition_tests.rs"]
mod status_definition_tests;

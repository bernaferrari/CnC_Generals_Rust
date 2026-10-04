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
            // A failed absent-store query may leave the catalog absent. Restore
            // its actual presence before restoring the exact owned contents.
            gamelogic::initialize_weapon_store().expect("restore previous catalog presence");
            gamelogic::weapon::with_weapon_store_mut(|store| *store = previous)
                .expect("restore exact previous catalog");
        } else {
            gamelogic::weapon::shutdown_weapon_store().expect("restore absence");
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

#[test]
fn ordinary_admission_bootstraps_replacement_without_foreign_completion() {
    isolated(
        "ordinary_admission_bootstraps_replacement_without_foreign_completion",
        || {
            let _restore = RestoreStore::install();
            prime_catalog();
            let _first = gamelogic::weapon::with_weapon_store_mut(std::mem::take).unwrap();
            assert!(!store_has(RANGER_PRIMARY_WEAPON));
            // No direct ensure here: actual create_object -> template resolution
            // must admit the missing rules before binding the weapon.
            admit_ranger(5.0);
            assert!(store_has(RANGER_PRIMARY_WEAPON));
            assert_eq!(ensure_host_weapon_store(), 0);
        },
    );
}

#[test]
fn bootstrap_retains_actual_parsed_override_and_completed_catalog() {
    isolated(
        "bootstrap_retains_actual_parsed_override_and_completed_catalog",
        || {
            let _restore = RestoreStore::install();
            assert_eq!(
                crate::assets::ini_template_loader::register_weapons_from_ini_text(
                    r#"
Weapon RangerAdvancedCombatRifle
  PrimaryDamage = 37
  AttackRange = 123
  DelayBetweenShots = 1000
  WeaponSpeed = 30000000
  ClipSize = 8
  DamageType = SMALL_ARMS
  AntiGround = Yes
  ProjectileObject = NONE
End
"#
                ),
                1
            );
            assert!(
                ensure_host_weapon_store() > 0,
                "fills remaining names beside authored Ranger"
            );
            admit_ranger(37.0);
            gamelogic::weapon::with_weapon_store(|store| {
                let actual = store.find_weapon_template(RANGER_PRIMARY_WEAPON).unwrap();
                assert_eq!(actual.primary_damage, 37.0);
                assert_eq!(actual.attack_range, 123.0);
                assert_eq!(actual.clip_size, 8);
                assert!(actual.projectile_name.eq_ignore_ascii_case("NONE"));
            })
            .unwrap();
            assert_eq!(ensure_host_weapon_store(), 0);
            admit_ranger(37.0);
        },
    );
}

#[test]
fn stream_query_bootstraps_absent_actual_catalog() {
    isolated("stream_query_bootstraps_absent_actual_catalog", || {
        let _restore = RestoreStore::install();
        gamelogic::weapon::shutdown_weapon_store().expect("actual missing catalog");
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

#[path = "ranger_primary_admission_tests.rs"]
mod ranger_primary_admission_tests;

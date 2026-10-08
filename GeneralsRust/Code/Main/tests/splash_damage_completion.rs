//! Ordinary (non-cfg(test)) Main library: real Weapon/DamageFX/FXList
//! parsers, admission, public target assignment, combat, onDie and removal. Public Main templates are used
//! here; the crate-local companion additionally loads real Object INI and checks
//! throttle state plus authored InstantDeathBehavior recursion.
use game_engine::common::ini::INI;
use game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store;
use game_engine::common::ini::ini_fx_list::{
    DispatchedFxNugget, FxListObjRuntime, clear_fx_list_obj_runtime, register_fx_list_obj_runtime,
    take_dispatched_fx_nuggets,
};
use generals_main::assets::ini_template_loader::register_weapons_from_ini_text;
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate, VeterancyLevel};
use glam::Vec3;
use std::sync::{Arc, Mutex};

static SERIAL: Mutex<()> = Mutex::new(());
#[derive(Default)]
struct Observer(Mutex<Vec<(String, Option<u32>, Option<u32>)>>);
impl FxListObjRuntime for Observer {
    fn do_fx_obj(&self, name: &str, victim: Option<u32>, source: Option<u32>) -> bool {
        self.0.lock().unwrap().push((name.into(), victim, source));
        false
    }
}
struct ClearObserver;
impl Drop for ClearObserver {
    fn drop(&mut self) {
        clear_fx_list_obj_runtime();
        let _ = take_dispatched_fx_nuggets();
    }
}

fn run(scatter: bool, count: usize, health: f32, route_to_sink: bool) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let label = format!(
        "{}_{}_{}_{}",
        if scatter { "Scatter" } else { "Hit" },
        count,
        health as u32,
        route_to_sink
    );
    let rule = format!("LibrarySplash{label}Weapon");
    let rows = format!("LibrarySplash{label}Rows");
    let table = if scatter {
        "ScatterTarget = X:0 Y:10\n ScatterTargetScalar = 1"
    } else {
        ""
    };
    gamelogic::initialize_weapon_store().unwrap();
    assert_eq!(
        register_weapons_from_ini_text(&format!(
            r#"
Weapon {rule}
 PrimaryDamage = 100
 PrimaryDamageRadius = 5
 AttackRange = 200
 DamageType = SMALL_ARMS
 RadiusDamageAffects = ENEMIES NEUTRALS
 ProjectileObject = NONE
 WeaponSpeed = 30000000
 DelayBetweenShots = 1000
 ClipSize = 8
 ClipReloadTime = 1000
 PreAttackDelay = 0
 AntiGround = Yes
 {table}
End
"#
        )),
        1
    );
    init_global_damage_fx_store();
    INI::new()
        .with_inline_source(
            &format!(
                r#"
DamageFX {rows}
 VeterancyMajorFX = REGULAR SMALL_ARMS FX_{rows}_Regular
 VeterancyMajorFX = VETERAN SMALL_ARMS FX_{rows}_Veteran
 VeterancyMajorFX = ELITE SMALL_ARMS FX_{rows}_Elite
 VeterancyAmountForMajorFX = REGULAR SMALL_ARMS 0
 VeterancyAmountForMajorFX = VETERAN SMALL_ARMS 0
 VeterancyAmountForMajorFX = ELITE SMALL_ARMS 0
 VeterancyThrottleTime = REGULAR SMALL_ARMS 100
 VeterancyThrottleTime = VETERAN SMALL_ARMS 500
 VeterancyThrottleTime = ELITE SMALL_ARMS 900
End
FXList FX_{rows}_Regular
 Sound
  Name = Sound_{rows}_Regular
 End
End
FXList FX_{rows}_Veteran
 Sound
  Name = Sound_{rows}_Veteran
 End
End
FXList FX_{rows}_Elite
 Sound
  Name = Sound_{rows}_Elite
 End
End
"#
            ),
            |ini| ini.parse_current_file(),
        )
        .unwrap();
    let mut world = GameLogic::new();
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, &format!("LibrarySplashPlayer{id}"), true);
        player.alliance_team = id as i32;
        player.is_alive = true;
        world.add_player(player);
    }
    let make = |name: &str, health: f32, xp: f32| {
        let mut t = ThingTemplate::new(name);
        t.set_health(health)
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_none();
        t.is_trainable = true;
        t.veterancy_xp_thresholds = [10.0, 20.0, 1000.0];
        t.experience_values = [xp; 4];
        t.geometry_info.authored = true;
        t
    };
    let mut shooter = make("LibrarySplashShooter", 500.0, 0.0);
    shooter.set_primary_weapon_name(&rule);
    let mut target = make("LibrarySplashVictim", health, 10.0);
    target
        .armor_sets
        .push(generals_main::game_logic::HostArmorSet {
            conditions: 0,
            armor: None,
            damage_fx: Some(rows.clone()),
        });
    for t in [shooter, target, make("LibrarySplashAnchor", 500.0, 0.0)] {
        world.templates.insert(t.name.clone(), t);
    }
    let shooter = world
        .create_object_for_player("LibrarySplashShooter", 0, Vec3::ZERO)
        .unwrap();
    let anchor = world
        .create_object_for_player("LibrarySplashAnchor", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let impact = if scatter {
        Vec3::new(10.0, 0.0, 10.0)
    } else {
        Vec3::new(12.0, 0.0, 0.0)
    };
    let targets: Vec<_> = (0..count)
        .map(|_| {
            world
                .create_object_for_player("LibrarySplashVictim", 1, impact)
                .unwrap()
        })
        .collect();
    let sink = route_to_sink.then(|| {
        world
            .create_object_for_player("LibrarySplashAnchor", 0, Vec3::new(-100.0, 0.0, 0.0))
            .unwrap()
    });
    if let Some(sink) = sink {
        world
            .objects
            .get_mut(&shooter)
            .unwrap()
            .set_experience_sink(Some(sink));
    }
    assert!(world.objects[&shooter].is_trainable());
    assert_eq!(world.objects[&shooter].owner_player_id, Some(0));
    for target in &targets {
        assert_eq!(world.objects[target].kill_experience_value(), 10.0);
        assert_eq!(world.objects[target].owner_player_id, Some(1));
    }
    assert_eq!(world.objects[&shooter].experience.current, 0.0);
    assert_eq!(
        world.objects[&shooter].experience.level,
        VeterancyLevel::Rookie
    );
    assert_eq!(
        world.objects[&shooter].weapon.as_ref().unwrap().ammo,
        Some(8)
    );
    let observer = Arc::new(Observer::default());
    register_fx_list_obj_runtime(observer.clone());
    assert!(take_dispatched_fx_nuggets().is_empty());
    world.set_current_frame(100);
    assert_eq!(
        world.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    // AttackObject commands enter the separately scoped queued-damage runtime.
    // Public set_target drives the ordinary host-combat path being repaired.
    world
        .objects
        .get_mut(&shooter)
        .unwrap()
        .set_target(Some(anchor));
    world.update();
    assert_eq!(world.get_current_frame(), 101);
    let source = &world.objects[&shooter];
    assert_eq!(source.weapon.as_ref().unwrap().ammo, Some(7));
    assert_eq!(source.weapon_discharge_marker().logic_frame, 100);
    let awarded = if health > 100.0 {
        0.0
    } else {
        count as f32 * 10.0
    };
    let expected_xp = if route_to_sink { 0.0 } else { awarded };
    assert_eq!(source.experience.current, expected_xp);
    assert_eq!(
        source.experience.level,
        match expected_xp as u32 {
            0 => VeterancyLevel::Rookie,
            10 => VeterancyLevel::Veteran,
            20 => VeterancyLevel::Elite,
            _ => panic!("unexpected fixture XP"),
        }
    );
    if let Some(sink) = sink {
        assert_eq!(world.objects[&sink].experience.current, awarded);
    }
    for target in &targets {
        if health > 100.0 {
            assert_eq!(world.objects[target].health.current, health - 100.0);
            assert_eq!(world.objects[target].next_damage_fx_time, 103);
            assert_eq!(world.objects[target].last_damage_timestamp, Some(100));
        } else {
            assert!(!world.objects.contains_key(target));
        }
    }
    assert_eq!(
        world.objects[&anchor].health.current,
        if scatter { 500.0 } else { 400.0 }
    );
    let calls = observer.0.lock().unwrap().clone();
    assert_eq!(calls.len(), count);
    let mut observed_victims: Vec<_> = calls.iter().map(|call| call.1.unwrap()).collect();
    observed_victims.sort_unstable();
    let mut expected_victims: Vec<_> = targets.iter().map(|id| id.0).collect();
    expected_victims.sort_unstable();
    assert_eq!(
        observed_victims, expected_victims,
        "each victim completes exactly once"
    );
    let mut expected_sounds = Vec::new();
    for (index, (name, _, source)) in calls.iter().enumerate() {
        let level = if expected_xp == 0.0 {
            "Regular"
        } else if index == 0 {
            "Veteran"
        } else {
            "Elite"
        };
        assert_eq!(
            name,
            &format!("FX_{rows}_{level}"),
            "each kill completes its own promotion before DamageFX"
        );
        assert_eq!(*source, Some(shooter.0));
        expected_sounds.push(DispatchedFxNugget::Sound(format!("Sound_{rows}_{level}")));
    }
    assert_eq!(take_dispatched_fx_nuggets(), expected_sounds);
    world.update();
    assert_eq!(
        world.objects[&shooter].experience.current, expected_xp,
        "no duplicate kill credit"
    );
    if let Some(sink) = sink {
        assert_eq!(world.objects[&sink].experience.current, awarded);
    }
    assert!(take_dispatched_fx_nuggets().is_empty());
}

#[test]
fn ordinary_library_instant_splash_completes_kill_before_damage_fx() {
    run(false, 1, 100.0, false);
}
#[test]
fn ordinary_library_scatter_splash_completes_kill_before_damage_fx() {
    run(true, 1, 100.0, false);
}
#[test]
fn ordinary_library_instant_splash_completes_each_victim() {
    run(false, 2, 100.0, false);
}
#[test]
fn ordinary_library_scatter_splash_completes_each_victim() {
    run(true, 2, 100.0, false);
}
#[test]
fn ordinary_library_instant_splash_nonlethal_regular_fx() {
    run(false, 1, 150.0, false);
}
#[test]
fn ordinary_library_scatter_splash_nonlethal_regular_fx() {
    run(true, 1, 150.0, false);
}
#[test]
fn ordinary_library_instant_splash_sink_does_not_promote_source_fx() {
    run(false, 1, 100.0, true);
}
#[test]
fn ordinary_library_scatter_splash_sink_does_not_promote_source_fx() {
    run(true, 1, 100.0, true);
}

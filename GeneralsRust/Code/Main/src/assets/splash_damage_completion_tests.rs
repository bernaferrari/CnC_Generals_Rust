//! C++ ActiveBody.cpp:640–653 completes scoreTheKill/onDie/DamageFX for each
//! Weapon.cpp:1467 victim before advancing the iterator. These tests live under
//! AssetManager solely to install a real headless authored catalogue privately.
//! Each exact test executes in a fresh harness process; no production test API,
//! GPU initialization, inherited catalogues, or synthetic callback damage.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team, VeterancyLevel};
use game_engine::common::ini::INI;
use game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store;
use game_engine::common::ini::ini_fx_list::{
    DispatchedFxNugget, FxListObjRuntime, register_fx_list_obj_runtime, take_dispatched_fx_nuggets,
};
use glam::Vec3;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
enum Splash {
    Hit,
    Scatter,
}
impl Splash {
    fn weapon(self) -> &'static str {
        match self {
            Self::Hit => "SplashCompletionHit",
            Self::Scatter => "SplashCompletionScatter",
        }
    }
    fn source(self) -> &'static str {
        match self {
            Self::Hit => "SplashCompletionShooter",
            Self::Scatter => "SplashCompletionScatterShooter",
        }
    }
    fn apply(self, world: &mut GameLogic, source: ObjectId, team: Team, skip: ObjectId) -> u32 {
        match self {
            Self::Hit => world.apply_instant_hit_splash_at(
                Vec3::new(30.0, 0.0, 0.0),
                100.0,
                0.0,
                5.0,
                0.0,
                source,
                team,
                skip,
                Some(self.weapon()),
            ),
            Self::Scatter => world.apply_scatter_miss_splash_at(
                Vec3::new(30.0, 0.0, 0.0),
                100.0,
                5.0,
                source,
                team,
                skip,
                Some(self.weapon()),
            ),
        }
    }
}

// Match the repository's exact-test subprocess isolation: process-wide asset
// OnceLocks cannot be restored after publication. The parent never installs one.
fn isolated(test: &str, run: impl FnOnce()) {
    const CHILD: &str = "GENERALS_SPLASH_COMPLETION_CHILD";
    let module = module_path!().split_once("::").unwrap().1;
    let exact = format!("{module}::{test}");
    if std::env::var(CHILD).ok().as_deref() == Some(exact.as_str()) {
        run();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
        .env(CHILD, &exact)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run isolated exact splash witness");
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            child.kill().unwrap();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(
        !timed_out,
        "isolated witness timed out: {exact}: {output:?}"
    );
    assert!(status.success(), "{exact}: {output:?}");
    assert!(
        output[0].contains("running 1 test") && output[0].contains("1 passed; 0 failed"),
        "exact test did not run: {output:?}"
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FxCall {
    name: String,
    victim: Option<u32>,
    source: Option<u32>,
}
#[derive(Default)]
struct Observer(Mutex<Vec<FxCall>>);
impl FxListObjRuntime for Observer {
    fn do_fx_obj(&self, name: &str, victim: Option<u32>, source: Option<u32>) -> bool {
        self.0.lock().unwrap().push(FxCall {
            name: name.into(),
            victim,
            source,
        });
        false // Continue through the real parsed FXList Sound nugget.
    }
}
impl Observer {
    fn take(&self) -> Vec<FxCall> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

fn object(
    name: &str,
    health: u32,
    xp: u32,
    rows: &str,
    weapon: Option<&str>,
    behavior: &str,
) -> String {
    let weapon = weapon
        .map(|name| format!("WeaponSet\n Conditions = None\n Weapon = PRIMARY {name}\nEnd"))
        .unwrap_or_default();
    format!(
        r#"
Object {name}
 KindOf = INFANTRY SELECTABLE ATTACKABLE SCORE
 IsTrainable = Yes
 ExperienceRequired = 0 10 20 1000
 ExperienceValue = {xp} {xp} {xp} {xp}
 Geometry = SPHERE
 GeometryMajorRadius = 1
 Body = ActiveBody ModuleTag_Body
  MaxHealth = {health}
 End
 ArmorSet
  Conditions = None
  DamageFX = {rows}
 End
 {weapon}
 {behavior}
End
"#
    )
}

fn create_single_file_big(path: &Path, virtual_path: &str, data: &[u8]) {
    let data_offset = 0x10 + 8 + virtual_path.len() + 1;
    let mut file = File::create(path).unwrap();
    file.write_all(b"BIGF").unwrap();
    file.write_all(&((data_offset + data.len()) as u32).to_le_bytes())
        .unwrap();
    file.write_all(&1u32.to_be_bytes()).unwrap();
    file.write_all(&(data_offset as u32).to_be_bytes()).unwrap();
    file.write_all(&(data_offset as u32).to_be_bytes()).unwrap();
    file.write_all(&(data.len() as u32).to_be_bytes()).unwrap();
    file.write_all(virtual_path.as_bytes()).unwrap();
    file.write_all(&[0]).unwrap();
    file.write_all(data).unwrap();
}

fn install() -> (GameLogic, Arc<Observer>) {
    assert!(ASSET_MANAGER.get().is_none());
    gamelogic::initialize_weapon_store().unwrap();
    let mut weapons = String::new();
    for (name, radius, affects, scatter) in [
        ("SplashCompletionHit", 5, "ENEMIES NEUTRALS", ""),
        (
            "SplashCompletionScatter",
            5,
            "ENEMIES NEUTRALS",
            "ScatterTarget = X:0 Y:10\n ScatterTargetScalar = 1",
        ),
        ("SplashCompletionDeath", 20, "ALLIES ENEMIES NEUTRALS", ""),
    ] {
        weapons.push_str(&format!(
            r#"
Weapon {name}
 PrimaryDamage = 100
 PrimaryDamageRadius = {radius}
 AttackRange = 200
 DamageType = SMALL_ARMS
 RadiusDamageAffects = {affects}
 ProjectileObject = NONE
 WeaponSpeed = 30000000
 DelayBetweenShots = 1000
 ClipSize = 8
 ClipReloadTime = 1000
 PreAttackDelay = 0
 AntiGround = Yes
 {scatter}
End
"#
        ));
    }
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(&weapons),
        3
    );
    init_global_damage_fx_store();
    let mut effects = String::new();
    for rows in ["SplashCompletionRows", "SplashCompletionNestedRows"] {
        effects.push_str(&format!("DamageFX {rows}\n"));
        for (level, ms) in [("REGULAR", 100), ("VETERAN", 500), ("ELITE", 900)] {
            effects.push_str(&format!(" VeterancyMajorFX = {level} SMALL_ARMS FX_{rows}_{level}\n VeterancyAmountForMajorFX = {level} SMALL_ARMS 0\n VeterancyThrottleTime = {level} SMALL_ARMS {ms}\n"));
        }
        effects.push_str("End\n");
        for level in ["REGULAR", "VETERAN", "ELITE"] {
            effects.push_str(&format!(
                "FXList FX_{rows}_{level}\n Sound\n  Name = Sound_{rows}_{level}\n End\nEnd\n"
            ));
        }
    }
    INI::new()
        .with_inline_source(&effects, |ini| ini.parse_current_file())
        .unwrap();
    let slow = "Behavior = SlowDeathBehavior ModuleTag_Retain\n DestructionDelay = 10000\n SinkDelay = 10000\nEnd";
    let instant = "Behavior = InstantDeathBehavior ModuleTag_Explosion\n DeathTypes = ALL\n Weapon = SplashCompletionDeath\nEnd";
    let mut objects = object(
        "SplashCompletionShooter",
        500,
        0,
        "SplashCompletionRows",
        Some("SplashCompletionHit"),
        "",
    );
    objects.push_str(&object(
        "SplashCompletionScatterShooter",
        500,
        0,
        "SplashCompletionRows",
        Some("SplashCompletionScatter"),
        "",
    ));
    objects.push_str(&object(
        "SplashCompletionVictim",
        100,
        10,
        "SplashCompletionRows",
        None,
        slow,
    ));
    objects.push_str(&object(
        "SplashCompletionNonlethal",
        150,
        10,
        "SplashCompletionRows",
        None,
        slow,
    ));
    objects.push_str(&object(
        "SplashCompletionAnchor",
        500,
        0,
        "SplashCompletionRows",
        None,
        "",
    ));
    objects.push_str(&object(
        "SplashCompletionOuter",
        100,
        10,
        "SplashCompletionRows",
        None,
        instant,
    ));
    objects.push_str(&object(
        "SplashCompletionNested",
        100,
        10,
        "SplashCompletionNestedRows",
        None,
        slow,
    ));
    objects.push_str(&object(
        "SplashCompletionImmediate",
        100,
        10,
        "SplashCompletionRows",
        None,
        "",
    ));
    let temp = tempfile::tempdir().unwrap();
    let big = temp.path().join("splash.big");
    create_single_file_big(
        &big,
        "Data/INI/Object/SplashCompletion.ini",
        objects.as_bytes(),
    );
    let mut manager = AssetManager::new().unwrap();
    futures::executor::block_on(manager.archive_system.load_big_file(&big)).unwrap();
    manager.archive_system.set_local_search_roots(Vec::new());
    futures::executor::block_on(manager.ww3d_manager.initialize(&mut manager.archive_system))
        .unwrap();
    assert_eq!(manager.get_object_definition_count(), 8);
    assert!(ASSET_MANAGER.set(Arc::new(Mutex::new(manager))).is_ok());
    let mut world = GameLogic::new();
    for (id, team) in [(0, Team::USA), (1, Team::China), (2, Team::GLA)] {
        let mut player = Player::new(id, team, &format!("SplashPlayer{id}"), true);
        player.alliance_team = id as i32;
        player.is_alive = true;
        world.add_player(player);
    }
    world.set_current_frame(100);
    let observer = Arc::new(Observer::default());
    register_fx_list_obj_runtime(observer.clone());
    assert!(take_dispatched_fx_nuggets().is_empty());
    (world, observer)
}

fn admit(world: &mut GameLogic, name: &str, player: u32, pos: Vec3) -> ObjectId {
    let id = world
        .create_object_for_player(name, player, pos)
        .expect("actual authored admission");
    let o = &world.objects[&id];
    assert_eq!(o.owner_player_id, Some(player));
    assert_eq!(o.experience.current, 0.0);
    assert_eq!(o.experience.level, VeterancyLevel::Rookie);
    assert!(o.is_trainable());
    assert_eq!(
        world.templates[name].veterancy_xp_thresholds,
        [10.0, 20.0, 1000.0]
    );
    id
}
fn source(world: &mut GameLogic, kind: Splash) -> ObjectId {
    admit(world, kind.source(), 0, Vec3::ZERO)
}
fn victim(world: &mut GameLogic, name: &str) -> ObjectId {
    admit(world, name, 1, Vec3::new(30.0, 0.0, 0.0))
}
fn assert_fx(
    observer: &Observer,
    victim: ObjectId,
    source: Option<ObjectId>,
    rows: &str,
    level: &str,
) {
    assert_eq!(
        observer.take(),
        [FxCall {
            name: format!("FX_{rows}_{level}"),
            victim: Some(victim.0),
            source: source.map(|id| id.0)
        }]
    );
    assert_eq!(
        take_dispatched_fx_nuggets(),
        [DispatchedFxNugget::Sound(format!("Sound_{rows}_{level}"))]
    );
}
fn promotion(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    let target = victim(&mut world, "SplashCompletionVictim");
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 1);
    assert_eq!(world.objects[&source].experience.current, 10.0);
    assert_eq!(
        world.objects[&source].experience.level,
        VeterancyLevel::Veteran
    );
    assert!(!world.objects[&target].is_alive());
    assert!(world.objects[&target].kill_experience_awarded);
    assert_fx(&fx, target, Some(source), "SplashCompletionRows", "VETERAN");
    assert_eq!(world.objects[&target].last_damage_timestamp, Some(100));
    assert_eq!(world.objects[&target].next_damage_fx_time, 115);
}
fn two_victims(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    let a = victim(&mut world, "SplashCompletionVictim");
    let b = victim(&mut world, "SplashCompletionVictim");
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 2);
    assert_eq!(world.objects[&source].experience.current, 20.0);
    let calls = fx.take();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "FX_SplashCompletionRows_VETERAN");
    assert_eq!(calls[1].name, "FX_SplashCompletionRows_ELITE");
    assert_ne!(calls[0].victim, calls[1].victim);
    for (call, deadline) in calls.iter().zip([115, 127]) {
        let id = ObjectId(call.victim.unwrap());
        assert!(id == a || id == b);
        assert_eq!(call.source, Some(source.0));
        assert_eq!(world.objects[&id].next_damage_fx_time, deadline);
        assert!(world.objects[&id].kill_experience_awarded);
    }
    assert_eq!(take_dispatched_fx_nuggets().len(), 2);
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 0);
    assert_eq!(world.objects[&source].experience.current, 20.0);
    assert!(fx.take().is_empty());
    assert!(take_dispatched_fx_nuggets().is_empty());
}
fn nested(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    let outer = victim(&mut world, "SplashCompletionOuter");
    let secondary = admit(
        &mut world,
        "SplashCompletionNested",
        0,
        Vec3::new(42.0, 0.0, 0.0),
    );
    // 12 units between centers: outside the original 5+1 splash, inside the
    // real InstantDeathBehavior weapon's 20+1 radius. No injected callback.
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 1);
    assert!(!world.objects[&secondary].is_alive());
    assert_eq!(world.objects[&secondary].last_damage_source, Some(outer));
    assert_eq!(world.objects[&source].experience.current, 10.0);
    assert_eq!(world.objects[&outer].experience.current, 10.0);
    assert_eq!(
        fx.take(),
        [
            FxCall {
                name: "FX_SplashCompletionNestedRows_VETERAN".into(),
                victim: Some(secondary.0),
                source: Some(outer.0)
            },
            FxCall {
                name: "FX_SplashCompletionRows_VETERAN".into(),
                victim: Some(outer.0),
                source: Some(source.0)
            },
        ]
    );
    assert_eq!(world.objects[&secondary].next_damage_fx_time, 115);
    assert_eq!(world.objects[&outer].next_damage_fx_time, 115);
    assert_eq!(take_dispatched_fx_nuggets().len(), 2);
}
fn target_update(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    let anchor = admit(
        &mut world,
        "SplashCompletionAnchor",
        1,
        Vec3::new(10.0, 0.0, 0.0),
    );
    let pos = match kind {
        Splash::Hit => Vec3::new(12.0, 0.0, 0.0),
        Splash::Scatter => Vec3::new(10.0, 0.0, 10.0),
    };
    let target = admit(&mut world, "SplashCompletionVictim", 1, pos);
    // Public target assignment selects host combat; AttackObject commands
    // select the separately scoped queued-damage attack-machine runtime.
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .set_target(Some(anchor));
    world.update();
    let shooter = &world.objects[&source];
    assert_eq!(shooter.weapon.as_ref().unwrap().ammo, Some(7));
    assert_eq!(shooter.weapon_discharge_marker().logic_frame, 100);
    assert_eq!(shooter.experience.current, 10.0);
    assert_eq!(shooter.experience.level, VeterancyLevel::Veteran);
    assert!(!world.objects[&target].is_alive());
    assert_eq!(world.objects[&target].last_damage_timestamp, Some(100));
    assert_eq!(world.objects[&target].next_damage_fx_time, 115);
    let calls = fx.take();
    let splash: Vec<_> = calls
        .iter()
        .filter(|call| call.victim == Some(target.0))
        .collect();
    assert_eq!(splash.len(), 1);
    assert_eq!(splash[0].name, "FX_SplashCompletionRows_VETERAN");
    assert_eq!(splash[0].source, Some(source.0));
    assert_eq!(
        world.objects[&anchor].health.current,
        match kind {
            Splash::Hit => 400.0,
            Splash::Scatter => 500.0,
        }
    );
}
fn controls(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    let target = victim(&mut world, "SplashCompletionNonlethal");
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 1);
    assert_eq!(world.objects[&target].health.current, 50.0);
    assert_eq!(world.objects[&source].experience.current, 0.0);
    assert_fx(&fx, target, Some(source), "SplashCompletionRows", "REGULAR");
    assert_eq!(world.objects[&target].next_damage_fx_time, 103);
    let sink = admit(
        &mut world,
        "SplashCompletionAnchor",
        0,
        Vec3::new(-100.0, 0.0, 0.0),
    );
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .set_experience_sink(Some(sink));
    world.set_current_frame(104);
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 1);
    assert_eq!(world.objects[&sink].experience.current, 10.0);
    assert_eq!(world.objects[&source].experience.current, 0.0);
    assert_fx(&fx, target, Some(source), "SplashCompletionRows", "REGULAR");
    assert_eq!(world.objects[&target].next_damage_fx_time, 107);
}

fn nested_kills_next_candidate(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    let a = victim(&mut world, "SplashCompletionOuter");
    let b = victim(&mut world, "SplashCompletionOuter");
    let secondary = admit(
        &mut world,
        "SplashCompletionNested",
        0,
        Vec3::new(42.0, 0.0, 0.0),
    );
    // Whichever HashMap candidate comes first owns the original hit. Its real
    // death weapon kills the other candidate before the outer iterator resumes.
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 1);
    assert_eq!(world.objects[&source].experience.current, 10.0);
    for id in [a, b, secondary] {
        assert!(!world.objects[&id].is_alive());
    }
    let calls = fx.take();
    assert_eq!(calls.len(), 3);
    let mut observed: Vec<_> = calls.iter().map(|call| call.victim.unwrap()).collect();
    observed.sort_unstable();
    let mut expected = vec![a.0, b.0, secondary.0];
    expected.sort_unstable();
    assert_eq!(observed, expected, "each victim completes exactly once");
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.source == Some(source.0))
            .count(),
        1
    );
    assert_eq!(calls.last().unwrap().source, Some(source.0));
    assert_eq!(take_dispatched_fx_nuggets().len(), 3);
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 0);
    assert_eq!(world.objects[&source].experience.current, 10.0);
    assert!(fx.take().is_empty());
}
fn missing_source(kind: Splash) {
    let (mut world, fx) = install();
    let missing = ObjectId(98765);
    let target = victim(&mut world, "SplashCompletionImmediate");
    assert_eq!(kind.apply(&mut world, missing, Team::USA, ObjectId(0)), 1);
    assert_fx(&fx, target, None, "SplashCompletionRows", "REGULAR");
    assert_eq!(world.objects[&target].next_damage_fx_time, 103);
    assert_eq!(world.objects[&target].last_damage_source, Some(missing));
    world.process_destroy_list();
    assert!(!world.objects.contains_key(&target));
    assert_eq!(
        world.get_player(1).unwrap().statistics.units_lost,
        1,
        "supplied killer team keeps the pre-existing missing-source death event"
    );
}
fn supplied_team(kind: Splash) {
    let (mut world, fx) = install();
    let source = source(&mut world, kind);
    // The caller captured USA before the source changed faction. Keep that
    // explicit event provenance; do not replace it with the fresh FX source.
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .set_team_and_owner(Team::GLA, Some(2));
    let target = victim(&mut world, "SplashCompletionImmediate");
    assert_eq!(kind.apply(&mut world, source, Team::USA, ObjectId(0)), 1);
    assert_eq!(world.objects[&source].experience.current, 10.0);
    assert_fx(&fx, target, Some(source), "SplashCompletionRows", "VETERAN");
    world.process_destroy_list();
    assert_eq!(
        world.get_player(2).unwrap().statistics.units_destroyed,
        0,
        "event team is the explicit caller team, not the source's later team"
    );
    assert_eq!(world.get_player(1).unwrap().statistics.units_lost, 1);
}

macro_rules! cases {
    ($kind:expr; $($name:ident => $run:ident),+ $(,)?) => { $(#[test] fn $name() { isolated(stringify!($name), || $run($kind)); })+ };
}
cases!(Splash::Hit;
    instant_splash_promotion_before_fx=>promotion,
    instant_splash_two_victims_complete_in_order=>two_victims,
    instant_splash_nested_death_finishes_before_outer_fx=>nested,
    instant_splash_target_update_uses_promoted_fx=>target_update,
    instant_splash_nonlethal_and_sink_controls=>controls,
    instant_splash_nested_killed_candidate_is_not_replayed=>nested_kills_next_candidate,
    instant_splash_missing_source_keeps_team_and_regular_fx=>missing_source,
    instant_splash_preserves_supplied_killer_team=>supplied_team,
);
cases!(Splash::Scatter;
    scatter_splash_promotion_before_fx=>promotion,
    scatter_splash_two_victims_complete_in_order=>two_victims,
    scatter_splash_nested_death_finishes_before_outer_fx=>nested,
    scatter_splash_target_update_uses_promoted_fx=>target_update,
    scatter_splash_nonlethal_and_sink_controls=>controls,
    scatter_splash_nested_killed_candidate_is_not_replayed=>nested_kills_next_candidate,
    scatter_splash_missing_source_keeps_team_and_regular_fx=>missing_source,
    scatter_splash_preserves_supplied_killer_team=>supplied_team,
);

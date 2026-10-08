//! Ordinary production-library regression for queued projectileless Direct hits.
//! Real AttackObject admission, Weapon/DamageFX parsers, due frames, XP and FX.
use game_engine::common::ini::INI;
use game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store;
use game_engine::common::ini::ini_fx_list::{
    DispatchedFxNugget, FxListObjRuntime, clear_fx_list_obj_runtime, register_fx_list_obj_runtime,
    take_dispatched_fx_nuggets,
};
use generals_main::assets::ini_template_loader::register_weapons_from_ini_text;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, VeterancyLevel,
};
use glam::Vec3;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

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

struct Fixture {
    world: GameLogic,
    shooters: Vec<ObjectId>,
    victim: ObjectId,
    sink: Option<ObjectId>,
    rows: String,
}
fn fixture(
    label: &str,
    health: f32,
    sink: bool,
    speed: u32,
    count: usize,
    threshold: f32,
) -> Fixture {
    let rule = format!("LibraryQueuedDirect{label}Weapon");
    let rows = format!("LibraryQueuedDirect{label}Rows");
    gamelogic::initialize_weapon_store().unwrap();
    assert_eq!(
        register_weapons_from_ini_text(&format!(
            r#"
Weapon {rule}
 PrimaryDamage = 100
 PrimaryDamageRadius = 0
 AttackRange = 200
 DamageType = SMALL_ARMS
 RadiusDamageAffects = ENEMIES NEUTRALS
 ProjectileObject = NONE
 WeaponSpeed = {speed}
 DelayBetweenShots = 1000
 ClipSize = 8
 ClipReloadTime = 1000
 PreAttackDelay = 0
 AntiGround = Yes
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
        let mut player = Player::new(id, team, label, id == 0);
        player.alliance_team = id as i32;
        player.is_alive = true;
        world.add_player(player);
    }
    let make = |name: &str, hp: f32, xp: f32| {
        let mut t = ThingTemplate::new(name);
        t.set_health(hp)
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_none();
        t.is_trainable = true;
        t.veterancy_xp_thresholds = [threshold, 100.0, 1000.0];
        t.experience_values = [xp; 4];
        t.geometry_info.authored = true;
        t
    };
    let mut shooter = make("QueuedSource", 500.0, 0.0);
    shooter.set_primary_weapon_name(&rule);
    let mut target = make("QueuedVictim", health, 10.0);
    target
        .armor_sets
        .push(generals_main::game_logic::HostArmorSet {
            conditions: 0,
            armor: None,
            damage_fx: Some(rows.clone()),
        });
    for t in [shooter, target, make("QueuedSink", 500.0, 0.0)] {
        world.templates.insert(t.name.clone(), t);
    }
    let shooters: Vec<_> = (0..count)
        .map(|_| {
            world
                .create_object_for_player("QueuedSource", 0, Vec3::ZERO)
                .unwrap()
        })
        .collect();
    let victim = world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let sink = sink.then(|| {
        world
            .create_object_for_player("QueuedSink", 0, Vec3::new(-100.0, 0.0, 0.0))
            .unwrap()
    });
    for source in &shooters {
        if let Some(sink) = sink {
            world
                .objects
                .get_mut(source)
                .unwrap()
                .set_experience_sink(Some(sink));
        }
        assert!(world.objects[source].is_trainable());
        assert_eq!(world.objects[source].owner_player_id, Some(0));
        assert_eq!(world.objects[source].experience.current, 0.0);
        assert_eq!(
            world.objects[source].experience.level,
            VeterancyLevel::Rookie
        );
        assert_eq!(
            world.objects[source].weapon.as_ref().unwrap().splash_radius,
            0.0
        );
        assert_eq!(
            world.get_able_to_attack_specific_object(
                *source,
                victim,
                generals_main::game_logic::AbleToAttackType::NewTarget,
                true
            ),
            generals_main::game_logic::CanAttackResult::Possible
        );
    }
    assert_eq!(world.objects[&victim].owner_player_id, Some(1));
    assert_eq!(world.objects[&victim].kill_experience_value(), 10.0);
    assert_eq!(
        world.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    world.set_current_frame(99);
    world.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: victim },
        player_id: 0,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: shooters.clone(),
        modifier_keys: ModifierKeys::default(),
    });
    Fixture {
        world,
        shooters,
        victim,
        sink,
        rows,
    }
}
impl Fixture {
    fn accept(&mut self) {
        self.world.update(); // frame99 aims
        for source in &self.shooters {
            assert_eq!(
                self.world.objects[source].weapon.as_ref().unwrap().ammo,
                Some(8)
            );
            assert_eq!(
                self.world.objects[source]
                    .weapon_discharge_marker()
                    .sequence,
                0
            );
        }
        self.world.update(); // frame100 accepts exactly one shot per source
        assert_eq!(self.world.get_current_frame(), 101);
        for source in &self.shooters {
            let o = &self.world.objects[source];
            assert_eq!(o.weapon.as_ref().unwrap().ammo, Some(7));
            assert!(o.weapon_discharge_marker().sequence > 0);
            assert_eq!(o.weapon_discharge_marker().logic_frame, 100);
        }
        // Discharge sequence is world-owned, so simultaneous sources receive
        // distinct sequence numbers, while each consumes exactly one round.
        let mut sequences: Vec<_> = self
            .shooters
            .iter()
            .map(|id| self.world.objects[id].weapon_discharge_marker().sequence)
            .collect();
        sequences.sort_unstable();
        assert_eq!(
            sequences,
            (1..=self.shooters.len() as u64).collect::<Vec<_>>()
        );
    }
    fn xp(&self) -> f32 {
        self.shooters
            .iter()
            .filter_map(|id| self.world.objects.get(id))
            .map(|o| o.experience.current)
            .sum()
    }
    fn expect_fx(&self, observer: &Observer, level: &str, source: Option<ObjectId>) {
        assert_eq!(
            observer.0.lock().unwrap().drain(..).collect::<Vec<_>>(),
            vec![(
                format!("FX_{}_{level}", self.rows),
                Some(self.victim.0),
                source.map(|id| id.0)
            )]
        );
        assert_eq!(
            take_dispatched_fx_nuggets(),
            vec![DispatchedFxNugget::Sound(format!(
                "Sound_{}_{level}",
                self.rows
            ))]
        );
    }
    fn no_repeat(&mut self, observer: &Observer) {
        let xp = self.xp();
        let sink_xp = self
            .sink
            .map(|id| self.world.objects[&id].experience.current);
        self.world.update();
        assert_eq!(self.xp(), xp);
        assert_eq!(
            self.sink
                .map(|id| self.world.objects[&id].experience.current),
            sink_xp
        );
        assert!(observer.0.lock().unwrap().is_empty());
        assert!(take_dispatched_fx_nuggets().is_empty());
    }
}
fn observe() -> Arc<Observer> {
    clear_fx_list_obj_runtime();
    let _ = take_dispatched_fx_nuggets();
    let observer = Arc::new(Observer::default());
    register_fx_list_obj_runtime(observer.clone());
    observer
}
fn instant(label: &str, health: f32, sink: bool) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture(label, health, sink, 30000000, 1, 10.0);
    f.accept();
    let lethal = health <= 100.0;
    if lethal {
        assert!(!f.world.objects.contains_key(&f.victim));
    } else {
        let v = &f.world.objects[&f.victim];
        assert_eq!(v.health.current, health - 100.0);
        assert_eq!(v.last_damage_timestamp, Some(100));
        assert_eq!(v.next_damage_fx_time, 103);
    }
    let xp = if lethal && !sink { 10.0 } else { 0.0 };
    assert_eq!(f.xp(), xp);
    assert_eq!(
        f.world.objects[&f.shooters[0]].experience.level,
        if xp > 0.0 {
            VeterancyLevel::Veteran
        } else {
            VeterancyLevel::Rookie
        }
    );
    if let Some(sink) = f.sink {
        assert_eq!(
            f.world.objects[&sink].experience.current,
            if lethal { 10.0 } else { 0.0 }
        );
    }
    f.expect_fx(
        &observer,
        if xp > 0.0 { "Veteran" } else { "Regular" },
        Some(f.shooters[0]),
    );
    f.no_repeat(&observer);
}
#[test]
fn ordinary_library_queued_direct_lethal_completes_before_fx() {
    instant("Lethal", 100.0, false);
}
#[test]
fn ordinary_library_queued_direct_nonlethal() {
    instant("Nonlethal", 150.0, false);
}
#[test]
fn ordinary_library_queued_direct_sink_keeps_source_rookie_fx() {
    instant("Sink", 100.0, true);
}
fn delayed(label: &str, remove_source: bool) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture(label, 100.0, false, 30, 1, 10.0);
    let source = f.shooters[0];
    if remove_source {
        f.world
            .objects
            .get_mut(&source)
            .unwrap()
            .gain_experience(10.0);
        assert_eq!(
            f.world.objects[&source].experience.level,
            VeterancyLevel::Veteran
        );
    }
    f.accept();
    if remove_source {
        f.world.destroy_object(source);
        f.world.update();
        assert!(!f.world.objects.contains_key(&source));
    }
    while f.world.get_current_frame() <= 110 {
        let frame = f.world.get_current_frame();
        assert_eq!(
            f.world.objects[&f.victim].health.current, 100.0,
            "no early hit before frame110"
        );
        assert_eq!(f.xp(), 0.0);
        assert!(observer.0.lock().unwrap().is_empty());
        assert!(take_dispatched_fx_nuggets().is_empty());
        f.world.update();
        if frame < 110 {
            assert!(f.world.objects.contains_key(&f.victim));
        }
    }
    assert!(!f.world.objects.contains_key(&f.victim));
    assert_eq!(f.xp(), if remove_source { 0.0 } else { 10.0 });
    f.expect_fx(
        &observer,
        if remove_source { "Regular" } else { "Veteran" },
        (!remove_source).then_some(source),
    );
    f.no_repeat(&observer);
}
#[test]
fn ordinary_library_queued_direct_finite_speed_due_frame() {
    delayed("Delayed", false);
}
#[test]
fn ordinary_library_queued_direct_missing_source_at_arrival() {
    delayed("MissingSource", true);
}
#[test]
fn ordinary_library_queued_direct_two_accepted_hits_credit_once() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture("TwoHits", 100.0, false, 30000000, 2, 10.0);
    f.accept();
    assert!(!f.world.objects.contains_key(&f.victim));
    assert_eq!(f.xp(), 10.0);
    let killer = *f
        .shooters
        .iter()
        .find(|id| f.world.objects[id].experience.current == 10.0)
        .unwrap();
    f.expect_fx(&observer, "Veteran", Some(killer));
    f.no_repeat(&observer);
}
#[test]
fn ordinary_library_queued_direct_interleaved_worlds() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut a = fixture("WorldA", 100.0, false, 30, 1, 10.0);
    a.accept();
    let mut b = fixture("WorldB", 100.0, false, 30, 1, 50.0);
    assert_eq!(a.shooters, b.shooters);
    assert_eq!(a.victim, b.victim);
    assert_eq!(
        a.world.objects[&a.victim].health.current, 100.0,
        "constructing B must not change A"
    );
    b.accept();
    for _ in 101..110 {
        a.world.update();
        b.world.update();
        assert_eq!(a.xp(), 0.0);
        assert_eq!(b.xp(), 0.0);
        assert!(take_dispatched_fx_nuggets().is_empty());
    }
    a.world.update();
    assert_eq!(a.xp(), 10.0);
    assert_eq!(b.xp(), 0.0);
    assert_eq!(b.world.objects[&b.victim].health.current, 100.0);
    a.expect_fx(&observer, "Veteran", Some(a.shooters[0]));
    b.world.update();
    assert_eq!(b.xp(), 10.0);
    assert_eq!(
        a.world.objects[&a.shooters[0]].experience.level,
        VeterancyLevel::Veteran
    );
    assert_eq!(
        b.world.objects[&b.shooters[0]].experience.level,
        VeterancyLevel::Rookie
    );
    b.expect_fx(&observer, "Regular", Some(b.shooters[0]));
    a.no_repeat(&observer);
    b.no_repeat(&observer);
}

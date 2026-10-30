//! Ordinary production-library regression for moving-projectile Direct and Area hits.
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

#[test]
fn projectile_damage_fx_throttle_belongs_to_the_driving_session() {
    let _guard = SERIAL.lock().unwrap_or_else(|error| error.into_inner());
    let mut make = |time| {
        fixture_custom_with_ai(
            "OwnedArmorFx",
            150.0,
            false,
            30,
            1,
            10.0,
            0.0,
            "ENEMIES NEUTRALS",
            "",
            false,
            None,
            Some(time),
        )
    };
    let mut left = make(1000);
    let mut right = make(2000);
    assert_eq!(left.victim, right.victim);
    left.accept();
    right.accept();
    for _ in 0..50 {
        if left.world.host_object(left.victim).unwrap().health.current < 150.0
            && right
                .world
                .host_object(right.victim)
                .unwrap()
                .health
                .current
                < 150.0
        {
            break;
        }
        left.world.update();
        right.world.update();
    }
    let left_victim = left.world.host_object(left.victim).unwrap();
    let right_victim = right.world.host_object(right.victim).unwrap();
    assert_eq!(left_victim.health.current, 50.0);
    assert_eq!(right_victim.health.current, 50.0);
    assert_eq!(
        right_victim.next_damage_fx_time,
        left_victim.next_damage_fx_time + 30
    );
}
fn fixture(
    label: &str,
    health: f32,
    sink: bool,
    speed: u32,
    count: usize,
    threshold: f32,
) -> Fixture {
    fixture_custom(
        label,
        health,
        sink,
        speed,
        count,
        threshold,
        0.0,
        "ENEMIES NEUTRALS",
        "",
    )
}
fn fixture_custom(
    label: &str,
    health: f32,
    sink: bool,
    speed: u32,
    count: usize,
    threshold: f32,
    radius: f32,
    affects: &str,
    extra: &str,
) -> Fixture {
    fixture_custom_with_ai(
        label, health, sink, speed, count, threshold, radius, affects, extra, false, None, None,
    )
}

fn fixture_custom_with_ai(
    label: &str,
    health: f32,
    sink: bool,
    speed: u32,
    count: usize,
    threshold: f32,
    radius: f32,
    affects: &str,
    extra: &str,
    with_ai: bool,
    owned_angle: Option<f32>,
    owned_throttle_ms: Option<u32>,
) -> Fixture {
    let rule = format!("LibraryMoving{label}Weapon");
    let rows = format!("LibraryMoving{label}Rows");
    gamelogic::initialize_weapon_store().unwrap();
    assert_eq!(
        register_weapons_from_ini_text(&format!(
            r#"
Weapon {rule}
 PrimaryDamage = 100
 PrimaryDamageRadius = {radius}
 AttackRange = 200
 DamageType = SMALL_ARMS
 RadiusDamageAffects = {affects}
 ProjectileObject = LibraryMoving{label}Projectile
 WeaponSpeed = {speed}
 DelayBetweenShots = 1000
 ClipSize = 8
 ClipReloadTime = 1000
 PreAttackDelay = 0
 AntiGround = Yes
 {extra}
End
"#
        )),
        1
    );
    gamelogic::weapon::with_weapon_store(|store| {
        let parsed = store.find_weapon_template(&rule).unwrap();
        assert_eq!(
            parsed.projectile_name,
            format!("LibraryMoving{label}Projectile")
        );
        assert_eq!(parsed.weapon_speed, speed as f32 / 30.0);
    })
    .unwrap();
    init_global_damage_fx_store();
    let damage_fx_source = format!(
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
    );
    INI::new()
        .with_inline_source(&damage_fx_source, |ini| ini.parse_current_file())
        .unwrap();
    let mut world = GameLogic::new();
    world.admit_damage_fx_source(&damage_fx_source).unwrap();
    if let Some(throttle) = owned_throttle_ms {
        world
            .admit_damage_fx_source(&format!(
                "DamageFX {rows}\n ThrottleTime = SMALL_ARMS {throttle}\nEnd\n"
            ))
            .unwrap();
    }
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, label, id == 0);
        player.alliance_team = id as i32;
        player.is_alive = true;
        world.add_player(player);
    }
    if let Some(angle) = owned_angle {
        let mut weapon = gamelogic::weapon::with_weapon_store(|store| {
            (**store.find_weapon_template(&rule).unwrap()).clone()
        })
        .unwrap();
        weapon.radius_damage_angle = angle;
        world
            .admit_receiving_weapon_snapshot(vec![(rule.clone(), weapon)])
            .unwrap();
    }
    // Give snapshot capture the same admitted receiving AI source used by
    // the production continuation fixtures. The fixture's host-only combat
    // fields below remain customized after receiving admission.
    if with_ai {
        let projectile = if owned_angle.is_some() {
            format!(
                "Object LibraryMoving{label}Projectile\nKindOf = PROJECTILE\nBehavior = DumbProjectileBehavior Flight\n FirstHeight = 0\n SecondHeight = 0\n FirstPercentIndent = 0.25\n SecondPercentIndent = 0.75\nEnd\nEnd\n"
            )
        } else {
            String::new()
        };
        world
            .admit_object_ini_sources([(
                "MovingProjectileFixture.ini".into(),
                format!(
                    "Object QueuedSource\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 500\nEnd\nBehavior = AIUpdateInterface ActualQueuedSourceAI\nEnd\nEnd\nObject QueuedVictim\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody Body\nMaxHealth = {health}\nInitialHealth = {health}\nEnd\nBehavior = AIUpdateInterface ActualQueuedVictimAI\nEnd\nEnd\nObject QueuedSink\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 500\nEnd\nBehavior = AIUpdateInterface ActualQueuedSinkAI\nEnd\nEnd\n{projectile}"
                ),
            )])
            .unwrap();
    }
    let make = |name: &str, hp: f32, xp: f32| {
        let mut t = ThingTemplate::new(name);
        t.set_health(hp)
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_primary_weapon_none();
        t.is_trainable = true;
        t.veterancy_xp_thresholds = [threshold, 20.0_f32.max(threshold + 1.0), 1000.0];
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
        if with_ai {
            let admitted = world.templates.get_mut(&t.name).unwrap();
            admitted.set_health(t.max_health);
            admitted.kind_of = t.kind_of;
            admitted.is_trainable = t.is_trainable;
            admitted.veterancy_xp_thresholds = t.veterancy_xp_thresholds;
            admitted.experience_values = t.experience_values;
            admitted.geometry_info = t.geometry_info;
            admitted.armor_sets = t.armor_sets;
            admitted.primary_weapon = t.primary_weapon;
            admitted.primary_weapon_name = t.primary_weapon_name;
            admitted.primary_weapon_explicitly_none = t.primary_weapon_explicitly_none;
        } else {
            world.templates.insert(t.name.clone(), t);
        }
    }
    let shooters: Vec<_> = (0..count)
        .map(|_| {
            world
                .create_object_for_player("QueuedSource", 0, Vec3::ZERO)
                .unwrap()
        })
        .collect();
    let victim = world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let sink = sink.then(|| {
        world
            .create_object_for_player("QueuedSink", 0, Vec3::new(-100.0, 0.0, 0.0))
            .unwrap()
    });
    for source in &shooters {
        if let Some(sink) = sink {
            world
                .host_object_mut(*source)
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
            radius
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

fn set_team_override_for_members(
    world: &mut GameLogic,
    source: ObjectId,
    target: ObjectId,
    relationship: i32,
) {
    let mut chunks = generals_main::save_load::snapshot::stamp_player_team_chunks(world).unwrap();
    let teams = &mut chunks.teams.as_mut().unwrap().teams;
    let source_team = teams
        .iter()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&source.0))
        })
        .unwrap()
        .team_id;
    let target_team = teams
        .iter_mut()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&target.0))
        })
        .unwrap();
    target_team.team_relations.push(Default::default());
    let override_ = target_team.team_relations.last_mut().unwrap();
    override_.team_id = source_team;
    override_.relationship = relationship;
    generals_main::save_load::snapshot::apply_pending_player_team_chunks(world, &chunks).unwrap();
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
            .filter_map(|id| self.world.host_object(*id))
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

fn advance_to_impact(f: &mut Fixture, observer: &Observer, health: f32) {
    advance_with_dead_source(f, observer, health, None);
}
fn advance_with_dead_source(
    f: &mut Fixture,
    observer: &Observer,
    health: f32,
    dead_source: Option<ObjectId>,
) {
    let mut previous = None;
    let mut advanced = false;
    let mut source_marked_dead = false;
    for _ in 0..50 {
        let shots = f.world.combat_system().projectiles_snapshot();
        if shots.is_empty() {
            assert!(advanced);
            if let Some(source) = dead_source {
                assert!(source_marked_dead);
                let object = &f.world.objects[&source];
                assert!(!object.is_alive());
                assert!(!object.status.destroyed);
            }
            return;
        }
        assert_eq!(f.world.objects[&f.victim].health.current, health);
        assert!(observer.0.lock().unwrap().is_empty());
        assert!(take_dispatched_fx_nuggets().is_empty());
        assert_eq!(f.xp(), 0.0, "no early kill credit");
        if let Some(sink) = f.sink {
            assert_eq!(f.world.objects[&sink].experience.current, 0.0);
        }
        assert!(
            shots
                .iter()
                .all(|shot| shot.projectile_object_name.starts_with("LibraryMoving"))
        );
        assert!(
            shots
                .iter()
                .all(|shot| shot.speed == 30.0 && shot.flight.is_none())
        );
        let position = shots[0].position;
        if let Some(previous) = previous {
            if position != previous {
                advanced = true;
            }
        }
        previous = Some(position);
        // Use the actual generic flight pose and its existing five-unit target
        // collision threshold, not projectileless due-frame timing. Killing
        // the source earlier would allow ordinary cleanup before the impact.
        if let Some(source) = dead_source {
            let next_position = position + shots[0].velocity / 30.0;
            if next_position.distance(f.world.objects[&f.victim].get_position()) <= 5.0 {
                let object = f.world.host_object_mut(source).unwrap();
                object.health.current = 0.0;
                assert!(!object.is_alive());
                assert!(!object.status.destroyed);
                source_marked_dead = true;
            }
        }
        f.world.update();
    }
    panic!("moving shot failed to retire");
}
#[test]
fn moving_direct_lethal_completes_before_fx() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture("Lethal", 100.0, false, 30, 1, 10.0);
    f.accept();
    assert_eq!(f.world.combat_system().projectiles_snapshot().len(), 1);
    assert_eq!(f.xp(), 0.0);
    advance_to_impact(&mut f, &observer, 100.0);
    assert!(f.world.host_object(f.victim).is_none());
    assert_eq!(f.xp(), 10.0);
    f.expect_fx(&observer, "Veteran", Some(f.shooters[0]));
    f.no_repeat(&observer);
}

// These fixtures use parsed weapons and actual generic projectile flight. They
// do not claim parsed Dumb/Missile lifecycle or callback-time retirement proof.
fn moving_case(
    label: &str,
    radius: f32,
    health: f32,
    sink: bool,
    missing: bool,
    dead: bool,
    count: usize,
) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture_custom(
        label,
        health,
        sink,
        30,
        count,
        10.0,
        radius,
        "ENEMIES NEUTRALS",
        "",
    );
    let source = f.shooters[0];
    f.accept();
    assert_eq!(f.world.combat_system().projectiles_snapshot().len(), count);
    assert_eq!(f.xp(), 0.0);
    if missing {
        f.world.destroy_object(source);
        f.world.update();
        assert!(f.world.host_object(source).is_none());
    }
    advance_with_dead_source(&mut f, &observer, health, dead.then_some(source));
    let lethal = health <= 100.0;
    if lethal {
        assert!(f.world.host_object(f.victim).is_none());
    } else {
        let victim = &f.world.objects[&f.victim];
        assert_eq!(victim.health.current, health - 100.0);
        let frame = (f.world.get_current_frame() - 1) as u32;
        assert!(frame > 100);
        assert_eq!(victim.last_damage_timestamp, Some(frame));
        assert_eq!(victim.next_damage_fx_time, frame + 3);
    }
    let xp = if lethal && !sink && !missing {
        10.0
    } else {
        0.0
    };
    assert_eq!(f.xp(), xp);
    if let Some(sink) = f.sink {
        assert_eq!(f.world.objects[&sink].experience.current, 10.0);
    }
    let killer = if missing {
        None
    } else if count == 1 {
        Some(source)
    } else {
        f.shooters
            .iter()
            .copied()
            .find(|id| f.world.objects[id].experience.current == 10.0)
    };
    f.expect_fx(
        &observer,
        if xp > 0.0 { "Veteran" } else { "Regular" },
        killer,
    );
    f.no_repeat(&observer);
}
#[test]
fn moving_direct_nonlethal() {
    moving_case("DirectNonlethal", 0.0, 150.0, false, false, false, 1);
}
#[test]
fn moving_direct_sink() {
    moving_case("DirectSink", 0.0, 100.0, true, false, false, 1);
}
#[test]
fn moving_direct_missing_source() {
    moving_case("DirectMissing", 0.0, 100.0, false, true, false, 1);
}
#[test]
fn moving_direct_dead_installed_source() {
    moving_case("DirectDead", 0.0, 100.0, false, false, true, 1);
}
#[test]
fn moving_direct_duplicate_hits_credit_once() {
    moving_case("DirectDuplicate", 0.0, 100.0, false, false, false, 2);
}
#[test]
fn moving_area_lethal() {
    moving_case("AreaLethal", 10.0, 100.0, false, false, false, 1);
}
#[test]
fn moving_area_nonlethal() {
    moving_case("AreaNonlethal", 10.0, 150.0, false, false, false, 1);
}
#[test]
fn moving_area_sink() {
    moving_case("AreaSink", 10.0, 100.0, true, false, false, 1);
}
#[test]
fn moving_area_missing_source() {
    moving_case("AreaMissing", 10.0, 100.0, false, true, false, 1);
}
#[test]
fn moving_area_dead_installed_source() {
    moving_case("AreaDead", 10.0, 100.0, false, false, true, 1);
}
#[test]
fn moving_area_duplicate_hits_credit_once() {
    moving_case("AreaDuplicate", 10.0, 100.0, false, false, false, 2);
}

#[test]
fn moving_area_two_kills_complete_in_encounter_order() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture_custom(
        "TwoKills",
        100.0,
        false,
        30,
        1,
        10.0,
        10.0,
        "ENEMIES NEUTRALS",
        "",
    );
    let other = f
        .world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(31.0, 0.0, 0.0))
        .unwrap();
    f.accept();
    advance_to_impact(&mut f, &observer, 100.0);
    assert!(f.world.host_object(f.victim).is_none());
    assert!(f.world.host_object(other).is_none());
    assert_eq!(f.xp(), 20.0);
    assert_eq!(
        f.world.objects[&f.shooters[0]].experience.level,
        VeterancyLevel::Elite
    );
    let hits = observer.0.lock().unwrap().drain(..).collect::<Vec<_>>();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].0, format!("FX_{}_Veteran", f.rows));
    assert_eq!(hits[1].0, format!("FX_{}_Elite", f.rows));
    let mut victims = hits.iter().map(|h| h.1.unwrap()).collect::<Vec<_>>();
    victims.sort_unstable();
    let mut expected = vec![f.victim.0, other.0];
    expected.sort_unstable();
    assert_eq!(victims, expected);
    assert!(hits.iter().all(|h| h.2 == Some(f.shooters[0].0)));
    assert_eq!(
        take_dispatched_fx_nuggets(),
        vec![
            DispatchedFxNugget::Sound(format!("Sound_{}_Veteran", f.rows)),
            DispatchedFxNugget::Sound(format!("Sound_{}_Elite", f.rows))
        ]
    );
    f.no_repeat(&observer);
}
fn worlds(radius: f32) {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut a = fixture_custom(
        "WorldA",
        100.0,
        false,
        30,
        1,
        10.0,
        radius,
        "ENEMIES NEUTRALS",
        "",
    );
    a.accept();
    let mut b = fixture_custom(
        "WorldB",
        100.0,
        false,
        30,
        1,
        50.0,
        radius,
        "ENEMIES NEUTRALS",
        "",
    );
    assert_eq!(a.shooters, b.shooters);
    assert_eq!(a.victim, b.victim);
    assert_eq!(a.world.objects[&a.victim].health.current, 100.0);
    b.accept();
    for _ in 0..50 {
        a.world.update();
        if a.world.combat_system().projectiles_snapshot().is_empty() {
            assert_eq!(a.xp(), 10.0);
            assert_eq!(b.xp(), 0.0);
            assert_eq!(b.world.objects[&b.victim].health.current, 100.0);
            assert_eq!(b.world.combat_system().projectiles_snapshot().len(), 1);
            a.expect_fx(&observer, "Veteran", Some(a.shooters[0]));
            b.world.update();
            assert!(b.world.combat_system().projectiles_snapshot().is_empty());
            assert_eq!(b.xp(), 10.0);
            assert_eq!(
                b.world.objects[&b.shooters[0]].experience.level,
                VeterancyLevel::Rookie
            );
            b.expect_fx(&observer, "Regular", Some(b.shooters[0]));
            a.no_repeat(&observer);
            b.no_repeat(&observer);
            return;
        }
        b.world.update();
        assert_eq!(a.xp(), 0.0);
        assert_eq!(b.xp(), 0.0);
        assert!(observer.0.lock().unwrap().is_empty());
        assert!(take_dispatched_fx_nuggets().is_empty());
    }
    panic!("interleaved shots did not retire");
}
#[test]
fn moving_direct_interleaved_worlds() {
    worlds(0.0);
}
#[test]
fn moving_area_interleaved_worlds() {
    worlds(10.0);
}
#[test]
fn moving_area_uses_projectile_default_team_identity_not_launcher_custom_team() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture_custom(
        "TeamAdmission",
        150.0,
        false,
        30,
        1,
        10.0,
        10.0,
        "ALLIES",
        "",
    );
    let collateral = f
        .world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(31.0, 0.0, 0.0))
        .unwrap();
    let mut chunks =
        generals_main::save_load::snapshot::stamp_player_team_chunks(&f.world).unwrap();
    let teams = &mut chunks.teams.as_mut().unwrap().teams;
    let source_team = teams
        .iter()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&f.shooters[0].0))
        })
        .unwrap()
        .team_id;
    let collateral_team = teams
        .iter_mut()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&collateral.0))
        })
        .unwrap();
    collateral_team.team_relations.push(Default::default());
    let override_ = collateral_team.team_relations.last_mut().unwrap();
    override_.team_id = source_team;
    override_.relationship = 2;
    generals_main::save_load::snapshot::apply_pending_player_team_chunks(&mut f.world, &chunks)
        .unwrap();
    f.world
        .host_object_mut(f.shooters[0])
        .unwrap()
        .team_instance_name = "LauncherTeam".into();
    f.world
        .host_object_mut(collateral)
        .unwrap()
        .team_instance_name = "CollateralTeam".into();
    assert_eq!(
        f.world.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    f.accept();
    advance_to_impact(&mut f, &observer, 150.0);
    // The projectile belongs to the shooter's default Team, while the launcher
    // has a custom Team name. The exact default-Team override still admits the
    // collateral hit because C++ uses the projectile as detonation source.
    assert_eq!(f.world.objects[&collateral].health.current, 50.0);
    assert_eq!(f.world.objects[&f.victim].health.current, 50.0); // primary bypass
    assert_eq!(observer.0.lock().unwrap().len(), 2);
    assert_eq!(f.xp(), 0.0);
}

#[test]
fn moving_area_projectile_default_team_identity_controls_collateral_filter() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture_custom_with_ai(
        "DefaultTeamIdentity",
        150.0,
        false,
        30,
        1,
        10.0,
        10.0,
        "ALLIES",
        "",
        true,
        None,
        None,
    );
    let collateral = f
        .world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(31.0, 0.0, 0.0))
        .unwrap();
    f.world
        .host_object_mut(f.shooters[0])
        .unwrap()
        .team_instance_name = "LauncherCustomTeam".into();
    set_team_override_for_members(
        &mut f.world,
        f.shooters[0],
        collateral,
        2, // Allies in the Team relationship wire value.
    );
    f.accept();
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&f.world).unwrap();
    let mut receiver = fixture_custom_with_ai(
        "DefaultTeamIdentity",
        150.0,
        false,
        30,
        1,
        10.0,
        10.0,
        "ALLIES",
        "",
        true,
        None,
        None,
    )
    .world;
    builder
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    f.world = receiver;
    advance_to_impact(&mut f, &observer, 150.0);
    assert_eq!(f.world.objects[&collateral].health.current, 50.0);
}

#[test]
fn moving_directional_splash_uses_projectile_pose_and_receiving_weapon_angle() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    fn world(angle: f32) -> (Fixture, ObjectId) {
        let mut f = fixture_custom_with_ai(
            "SharedDirectional",
            150.0,
            false,
            30,
            1,
            10.0,
            10.0,
            "ENEMIES NEUTRALS",
            "",
            true,
            Some(angle),
            None,
        );
        let behind_impact = f
            .world
            .create_object_for_player("QueuedVictim", 1, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        (f, behind_impact)
    }
    let (mut narrow, narrow_victim) = world(0.1);
    let (mut full, full_victim) = world(std::f32::consts::PI);
    assert_eq!(narrow_victim, full_victim);
    assert_eq!(narrow.shooters, full.shooters);

    narrow.accept();
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&narrow.world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: generals_main::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&encoded).unwrap();
    let (candidate, _) = world(0.1);
    let mut restored = candidate.world;
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    narrow.world = restored;
    full.accept();
    for f in [&mut narrow, &mut full] {
        for _ in 0..50 {
            if f.world.combat_system().projectiles_snapshot().is_empty() {
                break;
            }
            f.world.update();
        }
        assert!(f.world.combat_system().projectiles_snapshot().is_empty());
    }
    assert_eq!(
        narrow.world.objects[&narrow_victim].health.current, 150.0,
        "the victim is behind the moving projectile even though it is ahead of the launcher"
    );
    assert_eq!(
        full.world.objects[&full_victim].health.current, 50.0,
        "the same-named weapon in the peer session has a full-circle angle"
    );
}

#[test]
fn moving_area_shock_final_state() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let _clear = ClearObserver;
    let observer = observe();
    let mut f = fixture_custom(
        "Shock",
        200.0,
        false,
        30,
        1,
        10.0,
        10.0,
        "ENEMIES NEUTRALS",
        "ShockWaveAmount = 20\n ShockWaveRadius = 60\n ShockWaveTaperOff = 1",
    );
    f.accept();
    advance_to_impact(&mut f, &observer, 200.0);
    let victim = &f.world.objects[&f.victim];
    assert_eq!(victim.health.current, 100.0);
    assert_eq!(victim.last_damage_source, Some(f.shooters[0]));
    assert_eq!(
        victim.last_damage_timestamp,
        Some((f.world.get_current_frame() - 1) as u32)
    );
    assert!(victim.movement.velocity.x > 0.0);
    assert!(victim.movement.velocity.y > 0.0);
    assert!(victim.is_shock_stunned());
    f.expect_fx(&observer, "Regular", Some(f.shooters[0]));
}

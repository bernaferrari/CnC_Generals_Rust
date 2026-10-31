//! Ordinary production-library regression for queued projectileless Direct hits.
//! Real AttackObject admission, Weapon/DamageFX parsers, due frames, XP and FX.
use game_engine::common::ini::INI;
use game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store;
#[path = "common/queued_damage_fx.rs"]
mod queued_damage_fx;
use generals_main::assets::ini_template_loader::register_weapons_from_ini_text;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, VeterancyLevel,
};
use glam::Vec3;
use queued_damage_fx::Observation;
use std::sync::mpsc::Receiver;
use std::time::{Duration, UNIX_EPOCH};

struct Fixture {
    world: GameLogic,
    shooters: Vec<ObjectId>,
    victim: ObjectId,
    sink: Option<ObjectId>,
    rows: String,
    fx: Receiver<Observation>,
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
    let fx = queued_damage_fx::install(&rows);
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
                .host_object_mut(*source)
                .unwrap()
                .set_experience_sink(Some(sink));
        }
        assert!(world.host_objects()[source].is_trainable());
        assert_eq!(world.host_objects()[source].owner_player_id, Some(0));
        assert_eq!(world.host_objects()[source].experience.current, 0.0);
        assert_eq!(
            world.host_objects()[source].experience.level,
            VeterancyLevel::Rookie
        );
        assert_eq!(
            world.host_objects()[source]
                .weapon
                .as_ref()
                .unwrap()
                .splash_radius,
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
    assert_eq!(world.host_objects()[&victim].owner_player_id, Some(1));
    assert_eq!(world.host_objects()[&victim].kill_experience_value(), 10.0);
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
        fx,
    }
}
impl Fixture {
    fn accept(&mut self) {
        self.world.update(); // frame99 aims
        for source in &self.shooters {
            assert_eq!(
                self.world.host_objects()[source]
                    .weapon
                    .as_ref()
                    .unwrap()
                    .ammo,
                Some(8)
            );
            assert_eq!(
                self.world.host_objects()[source]
                    .weapon_discharge_marker()
                    .sequence,
                0
            );
        }
        self.world.update(); // frame100 accepts exactly one shot per source
        assert_eq!(self.world.get_current_frame(), 101);
        for source in &self.shooters {
            let o = &self.world.host_objects()[source];
            assert_eq!(o.weapon.as_ref().unwrap().ammo, Some(7));
            assert!(o.weapon_discharge_marker().sequence > 0);
            assert_eq!(o.weapon_discharge_marker().logic_frame, 100);
        }
        // Discharge sequence is world-owned, so simultaneous sources receive
        // distinct sequence numbers, while each consumes exactly one round.
        let mut sequences: Vec<_> = self
            .shooters
            .iter()
            .map(|id| {
                self.world.host_objects()[id]
                    .weapon_discharge_marker()
                    .sequence
            })
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
            .filter_map(|id| self.world.host_objects().lookup(id))
            .map(|o| o.experience.current)
            .sum()
    }
    fn take_fx(&self) -> Vec<Observation> {
        self.fx.try_iter().collect()
    }
    fn expect_fx(&self, level: &str, source: Option<ObjectId>) {
        let observed = self.take_fx();
        assert_eq!(
            observed
                .iter()
                .map(|event| (event.list.clone(), event.victim, event.source))
                .collect::<Vec<_>>(),
            vec![(
                format!("FX_{}_{level}", self.rows),
                Some(self.victim.0),
                source.map(|id| id.0)
            )]
        );
        // The actual native list ran before this report. These are the Sound
        // names from its compiled nuggets, not a Common dispatch log or a
        // claim that an audio device played the sound.
        assert_eq!(
            observed
                .into_iter()
                .flat_map(|event| event.sounds)
                .collect::<Vec<_>>(),
            vec![format!("Sound_{}_{level}", self.rows)]
        );
    }
    fn no_repeat(&mut self) {
        let xp = self.xp();
        let sink_xp = self
            .sink
            .map(|id| self.world.host_objects()[&id].experience.current);
        self.world.update();
        assert_eq!(self.xp(), xp);
        assert_eq!(
            self.sink
                .map(|id| self.world.host_objects()[&id].experience.current),
            sink_xp
        );
        assert!(self.take_fx().is_empty());
    }
}
fn instant(label: &str, health: f32, sink: bool) {
    let mut f = fixture(label, health, sink, 30000000, 1, 10.0);
    f.accept();
    let lethal = health <= 100.0;
    if lethal {
        assert!(!f.world.host_objects().lookup(&f.victim).is_some());
    } else {
        let v = &f.world.host_objects()[&f.victim];
        assert_eq!(v.health.current, health - 100.0);
        assert_eq!(v.last_damage_timestamp, Some(100));
        assert_eq!(v.next_damage_fx_time, 103);
    }
    let xp = if lethal && !sink { 10.0 } else { 0.0 };
    assert_eq!(f.xp(), xp);
    assert_eq!(
        f.world.host_objects()[&f.shooters[0]].experience.level,
        if xp > 0.0 {
            VeterancyLevel::Veteran
        } else {
            VeterancyLevel::Rookie
        }
    );
    if let Some(sink) = f.sink {
        assert_eq!(
            f.world.host_objects()[&sink].experience.current,
            if lethal { 10.0 } else { 0.0 }
        );
    }
    f.expect_fx(
        if xp > 0.0 { "Veteran" } else { "Regular" },
        Some(f.shooters[0]),
    );
    f.no_repeat();
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
    let mut f = fixture(label, 100.0, false, 30, 1, 10.0);
    let source = f.shooters[0];
    if remove_source {
        // Setup-only promotion: explicit local audio, no receiving UI admission.
        let mut audio = generals_main::game_logic::SessionAudio::default();
        f.world.host_object_mut(source).unwrap().gain_experience(
            10.0,
            &mut generals_main::game_logic::ObjectAudio::queue(&mut audio),
            &mut generals_main::game_logic::VeterancyFeedback::standalone(),
        );
        assert_eq!(
            f.world.host_objects()[&source].experience.level,
            VeterancyLevel::Veteran
        );
    }
    f.accept();
    if remove_source {
        f.world.destroy_object(source);
        f.world.update();
        assert!(!f.world.host_objects().lookup(&source).is_some());
    }
    while f.world.get_current_frame() <= 110 {
        let frame = f.world.get_current_frame();
        assert_eq!(
            f.world.host_objects()[&f.victim].health.current,
            100.0,
            "no early hit before frame110"
        );
        assert_eq!(f.xp(), 0.0);
        assert!(f.take_fx().is_empty());
        f.world.update();
        if frame < 110 {
            assert!(f.world.host_objects().lookup(&f.victim).is_some());
        }
    }
    assert!(!f.world.host_objects().lookup(&f.victim).is_some());
    assert_eq!(f.xp(), if remove_source { 0.0 } else { 10.0 });
    f.expect_fx(
        if remove_source { "Regular" } else { "Veteran" },
        (!remove_source).then_some(source),
    );
    f.no_repeat();
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
    // CPP Weapon.cpp998-1075 completes sub-frame hits synchronously. A real
    // finite flight lets both sources accept while the intended target lives.
    let mut f = fixture("TwoHits", 100.0, false, 30, 2, 10.0);
    f.accept();
    while f.world.get_current_frame() <= 110 {
        let frame = f.world.get_current_frame();
        assert_eq!(f.world.host_objects()[&f.victim].health.current, 100.0);
        assert_eq!(f.xp(), 0.0);
        assert!(f.take_fx().is_empty());
        f.world.update();
        if frame < 110 {
            assert!(f.world.host_objects().lookup(&f.victim).is_some());
        }
    }
    assert!(!f.world.host_objects().lookup(&f.victim).is_some());
    assert_eq!(f.xp(), 10.0);
    let killer = *f
        .shooters
        .iter()
        .find(|id| f.world.host_objects()[id].experience.current == 10.0)
        .unwrap();
    f.expect_fx("Veteran", Some(killer));
    f.no_repeat();
}
#[test]
fn ordinary_library_queued_direct_interleaved_worlds() {
    let mut a = fixture("WorldA", 100.0, false, 30, 1, 10.0);
    a.accept();
    let mut b = fixture("WorldB", 100.0, false, 30, 1, 50.0);
    assert_eq!(a.shooters, b.shooters);
    assert_eq!(a.victim, b.victim);
    assert_eq!(
        a.world.host_objects()[&a.victim].health.current,
        100.0,
        "constructing B must not change A"
    );
    b.accept();
    for _ in 101..110 {
        a.world.update();
        b.world.update();
        assert_eq!(a.xp(), 0.0);
        assert_eq!(b.xp(), 0.0);
        assert!(a.take_fx().is_empty());
        assert!(b.take_fx().is_empty());
    }
    a.world.update();
    assert_eq!(a.xp(), 10.0);
    assert_eq!(b.xp(), 0.0);
    assert_eq!(b.world.host_objects()[&b.victim].health.current, 100.0);
    a.expect_fx("Veteran", Some(a.shooters[0]));
    assert!(b.take_fx().is_empty());
    b.world.update();
    assert_eq!(b.xp(), 10.0);
    assert_eq!(
        a.world.host_objects()[&a.shooters[0]].experience.level,
        VeterancyLevel::Veteran
    );
    assert_eq!(
        b.world.host_objects()[&b.shooters[0]].experience.level,
        VeterancyLevel::Rookie
    );
    b.expect_fx("Regular", Some(b.shooters[0]));
    assert!(a.take_fx().is_empty());
    a.no_repeat();
    assert!(b.take_fx().is_empty());
    b.no_repeat();
    assert!(a.take_fx().is_empty());
}

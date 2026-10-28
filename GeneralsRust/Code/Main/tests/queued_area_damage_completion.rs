//! Ordinary production-library regression for queued projectileless Area hits.
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
    intended: ObjectId,
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
    fixture_custom(
        label,
        health,
        sink,
        speed,
        count,
        threshold,
        5.0,
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
    fixture_custom_with_variation(
        label, health, sink, speed, count, threshold, radius, affects, extra, None,
    )
}

fn fixture_custom_with_variation(
    label: &str,
    health: f32,
    sink: bool,
    speed: u32,
    count: usize,
    threshold: f32,
    radius: f32,
    affects: &str,
    extra: &str,
    variation: Option<bool>,
) -> Fixture {
    let rule = format!("LibraryQueuedArea{label}Weapon");
    let rows = format!("LibraryQueuedArea{label}Rows");
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
 ProjectileObject = NONE
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
        let definition = store
            .find_weapon_template(&rule)
            .expect("authored Area weapon");
        assert!(definition.projectile_name.eq_ignore_ascii_case("NONE"));
        assert_eq!(definition.primary_damage_radius, radius);
    })
    .expect("real weapon store");
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
    if let Some(has_variation) = variation {
        let variation_line = if has_variation {
            "BuildVariations = QueuedVictim\n"
        } else {
            ""
        };
        let body = |name: &str, hp: f32, extra: &str| {
            format!(
                "Object {name}\nKindOf = INFANTRY SELECTABLE\n{extra}Body = ActiveBody Body\nMaxHealth = {hp}\nInitialHealth = {hp}\nEnd\nEnd\n"
            )
        };
        let source = [
            body("QueuedSource", 500.0, variation_line),
            body("QueuedVictim", health, ""),
            body("QueuedAim", 10000.0, ""),
            body("QueuedSink", 500.0, ""),
        ]
        .concat();
        world
            .admit_object_ini_sources([("SplashVariation.ini".into(), source)])
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
    for t in [
        shooter,
        target,
        make("QueuedAim", 10000.0, 0.0),
        make("QueuedSink", 500.0, 0.0),
    ] {
        if variation.is_some() {
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
    let intended = world
        .create_object_for_player("QueuedAim", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let victim = world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(11.0, 0.0, 0.0))
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
            radius
        );
        assert_eq!(
            world.get_able_to_attack_specific_object(
                *source,
                intended,
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
        command_type: CommandType::AttackObject {
            target_id: intended,
        },
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
        intended,
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
            .map(|id| self.world.objects[&id].experience.current);
        self.world.update();
        assert_eq!(self.xp(), xp);
        assert_eq!(
            self.sink
                .map(|id| self.world.objects[&id].experience.current),
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
        assert!(!f.world.objects.contains_key(&f.victim));
    } else {
        let v = &f.world.objects[&f.victim];
        assert_eq!(v.health.current, health - 100.0);
        assert_eq!(v.last_damage_timestamp, Some(100));
        assert_eq!(v.next_damage_fx_time, 103);
    }
    let xp = if lethal && !sink { 10.0 } else { 0.0 };
    assert_eq!(f.world.objects[&f.intended].health.current, 9900.0);
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
        if xp > 0.0 { "Veteran" } else { "Regular" },
        Some(f.shooters[0]),
    );
    f.no_repeat();
}
#[test]
fn ordinary_library_queued_area_lethal_completes_before_fx() {
    instant("Lethal", 100.0, false);
}
#[test]
fn ordinary_library_queued_area_nonlethal() {
    instant("Nonlethal", 150.0, false);
}
#[test]
fn ordinary_library_queued_area_sink_keeps_source_rookie_fx() {
    instant("Sink", 100.0, true);
}
fn delayed(label: &str, remove_source: bool) {
    let mut f = fixture(label, 100.0, false, 30, 1, 10.0);
    let source = f.shooters[0];
    if remove_source {
        // Setup-only promotion: explicit local audio, no receiving UI admission.
        let mut audio = generals_main::game_logic::SessionAudio::default();
        f.world.objects.get_mut(&source).unwrap().gain_experience(
            10.0,
            &mut generals_main::game_logic::ObjectAudio::queue(&mut audio),
            &mut generals_main::game_logic::VeterancyFeedback::standalone(),
        );
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
        assert!(f.take_fx().is_empty());
        f.world.update();
        if frame < 110 {
            assert!(f.world.objects.contains_key(&f.victim));
        }
    }
    assert!(!f.world.objects.contains_key(&f.victim));
    assert_eq!(f.xp(), if remove_source { 0.0 } else { 10.0 });
    f.expect_fx(
        if remove_source { "Regular" } else { "Veteran" },
        (!remove_source).then_some(source),
    );
    f.no_repeat();
}
#[test]
fn ordinary_library_queued_area_finite_speed_due_frame() {
    delayed("Delayed", false);
}
#[test]
fn ordinary_library_queued_area_missing_source_at_arrival() {
    delayed("MissingSource", true);
}

#[test]
fn missing_projectileless_source_bypasses_radius_affects_filters() {
    let mut f = fixture_custom(
        "MissingSourceBypass",
        200.0,
        false,
        30,
        1,
        10.0,
        5.0,
        "ENEMIES",
        "",
    );
    let same_owner_collateral = f
        .world
        .create_object_for_player("QueuedSink", 0, Vec3::new(12.0, 0.0, 0.0))
        .unwrap();
    let source = f.shooters[0];
    f.accept();
    f.world.destroy_object(source).unwrap();
    f.world.update();
    while f.world.get_current_frame() <= 110 {
        f.world.update();
    }
    assert_eq!(
        f.world.objects[&same_owner_collateral].health.current, 400.0,
        "C++ skips RadiusDamageAffects filtering when the delayed source no longer exists"
    );
}

#[test]
fn projectileless_area_uses_the_receiving_worlds_exact_team_override() {
    fn receiving_world(label: &str) -> Fixture {
        fixture_custom(label, 200.0, false, 30, 1, 10.0, 5.0, "ALLIES", "")
    }

    let mut first = receiving_world("TeamOverrideA");
    let mut peer = receiving_world("TeamOverrideB");
    assert_eq!(first.shooters, peer.shooters);
    assert_eq!(first.victim, peer.victim);

    let mut chunks =
        generals_main::save_load::snapshot::stamp_player_team_chunks(&first.world).unwrap();
    let teams = &mut chunks.teams.as_mut().unwrap().teams;
    let source_team = teams
        .iter()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&first.shooters[0].0))
        })
        .unwrap()
        .team_id;
    let victim_team = teams
        .iter_mut()
        .find(|team| {
            team.members
                .as_ref()
                .is_some_and(|members| members.contains(&first.victim.0))
        })
        .unwrap();
    victim_team.team_relations.push(Default::default());
    let relationship = victim_team.team_relations.last_mut().unwrap();
    relationship.team_id = source_team;
    relationship.relationship = 2; // Allies in the Team continuation format.
    generals_main::save_load::snapshot::apply_pending_player_team_chunks(&mut first.world, &chunks)
        .unwrap();

    first.accept();
    while first.world.get_current_frame() <= 110 {
        first.world.update();
    }
    assert_eq!(first.world.objects[&first.victim].health.current, 100.0);
    assert_eq!(peer.world.objects[&peer.victim].health.current, 200.0);

    peer.accept();
    while peer.world.get_current_frame() <= 110 {
        peer.world.update();
    }
    assert_eq!(
        peer.world.objects[&peer.victim].health.current, 200.0,
        "the other world's Enemy relationship does not admit an ALLIES splash"
    );
}

#[test]
fn splash_similarity_uses_receiving_definitions_and_source_to_victim_relationship() {
    fn world(label: &str, has_variation: bool) -> Fixture {
        fixture_custom_with_variation(
            label,
            200.0,
            false,
            30,
            1,
            10.0,
            5.0,
            "ENEMIES NOT_SIMILAR",
            "",
            Some(has_variation),
        )
    }
    fn source_considers_victim_allied(f: &mut Fixture) {
        let mut chunks =
            generals_main::save_load::snapshot::stamp_player_team_chunks(&f.world).unwrap();
        let team_chunk = chunks.teams.as_mut().unwrap();
        let victim_index = team_chunk
            .teams
            .iter()
            .position(|team| {
                team.members
                    .as_ref()
                    .is_some_and(|members| members.contains(&f.victim.0))
            })
            .unwrap();
        let mut intended_team = team_chunk.teams[victim_index].clone();
        intended_team.team_id = team_chunk.unique_team_id;
        team_chunk.unique_team_id += 1;
        intended_team.members = Some(vec![f.intended.0]);
        team_chunk.teams[victim_index]
            .members
            .as_mut()
            .unwrap()
            .retain(|&id| id != f.intended.0);
        team_chunk.teams.push(intended_team);
        let teams = &mut team_chunk.teams;
        let victim_team = teams
            .iter()
            .find(|team| {
                team.members
                    .as_ref()
                    .is_some_and(|members| members.contains(&f.victim.0))
            })
            .unwrap()
            .team_id;
        let source_team = teams
            .iter_mut()
            .find(|team| {
                team.members
                    .as_ref()
                    .is_some_and(|members| members.contains(&f.shooters[0].0))
            })
            .unwrap();
        source_team.team_relations.push(Default::default());
        let relationship = source_team.team_relations.last_mut().unwrap();
        relationship.team_id = victim_team;
        relationship.relationship = 2; // Allies in the Team continuation format.
        generals_main::save_load::snapshot::apply_pending_player_team_chunks(&mut f.world, &chunks)
            .unwrap();
    }

    let mut first = world("VariationFirst", true);
    let mut peer = world("VariationPeer", false);
    assert_eq!(
        (first.shooters[0], first.victim),
        (peer.shooters[0], peer.victim)
    );
    for f in [&mut first, &mut peer] {
        source_considers_victim_allied(f);
        assert_eq!(
            f.world.object_relationship(
                &f.world.objects[&f.shooters[0]],
                &f.world.objects[&f.victim]
            ),
            gamelogic::common::Relationship::Allies
        );
        assert_eq!(
            f.world.object_relationship(
                &f.world.objects[&f.victim],
                &f.world.objects[&f.shooters[0]]
            ),
            gamelogic::common::Relationship::Enemies
        );
        f.accept();
        while f.world.get_current_frame() <= 110 {
            f.world.update();
        }
    }
    assert_eq!(
        first.world.objects[&first.victim].health.current, 200.0,
        "source-to-victim Allies protects the receiving build variation"
    );
    assert_eq!(
        peer.world.objects[&peer.victim].health.current, 100.0,
        "the same names are not equivalent in the peer's definitions"
    );
}

#[test]
fn self_splash_still_requires_the_allies_relationship_mask() {
    let mut excluded = fixture_custom(
        "SelfWithoutAllies",
        200.0,
        false,
        30,
        1,
        10.0,
        20.0,
        "SELF ENEMIES",
        "",
    );
    let mut included = fixture_custom(
        "SelfWithAllies",
        200.0,
        false,
        30,
        1,
        10.0,
        20.0,
        "SELF ALLIES ENEMIES",
        "",
    );
    assert_eq!(excluded.shooters, included.shooters);
    for f in [&mut excluded, &mut included] {
        f.accept();
        while f.world.get_current_frame() <= 110 {
            f.world.update();
        }
    }
    assert_eq!(
        excluded.world.objects[&excluded.shooters[0]].health.current, 500.0,
        "SELF removes the self exclusion but does not imply ALLIES"
    );
    assert_eq!(
        included.world.objects[&included.shooters[0]].health.current,
        400.0
    );
}

#[test]
fn ordinary_library_queued_area_two_accepted_hits_credit_once() {
    let mut f = fixture("TwoHits", 100.0, false, 30000000, 2, 10.0);
    f.accept();
    assert!(!f.world.objects.contains_key(&f.victim));
    assert_eq!(f.xp(), 10.0);
    let killer = *f
        .shooters
        .iter()
        .find(|id| f.world.objects[id].experience.current == 10.0)
        .unwrap();
    f.expect_fx("Veteran", Some(killer));
    f.no_repeat();
}
#[test]
fn ordinary_library_queued_area_interleaved_worlds() {
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
        assert!(a.take_fx().is_empty());
        assert!(b.take_fx().is_empty());
    }
    a.world.update();
    assert_eq!(a.xp(), 10.0);
    assert_eq!(b.xp(), 0.0);
    assert_eq!(b.world.objects[&b.victim].health.current, 100.0);
    a.expect_fx("Veteran", Some(a.shooters[0]));
    assert!(b.take_fx().is_empty());
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
    b.expect_fx("Regular", Some(b.shooters[0]));
    assert!(a.take_fx().is_empty());
    a.no_repeat();
    assert!(b.take_fx().is_empty());
    b.no_repeat();
    assert!(a.take_fx().is_empty());
}

#[test]
fn ordinary_library_queued_area_two_collateral_kills_complete_in_encounter_order() {
    let mut f = fixture("TwoKills", 100.0, false, 30000000, 1, 10.0);
    let other = f
        .world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(12.0, 0.0, 0.0))
        .unwrap();
    f.accept();
    assert!(!f.world.objects.contains_key(&f.victim));
    assert!(!f.world.objects.contains_key(&other));
    assert_eq!(f.xp(), 20.0);
    assert_eq!(
        f.world.objects[&f.shooters[0]].experience.level,
        VeterancyLevel::Elite
    );
    let observed = f.take_fx();
    let hits = observed
        .iter()
        .map(|event| (event.list.clone(), event.victim, event.source))
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].0, format!("FX_{}_Veteran", f.rows));
    assert_eq!(hits[1].0, format!("FX_{}_Elite", f.rows));
    let mut victims = hits.iter().map(|h| h.1.unwrap()).collect::<Vec<_>>();
    victims.sort_unstable();
    let mut expected = vec![f.victim.0, other.0];
    expected.sort_unstable();
    assert_eq!(victims, expected); // HashMap encounter order is intentionally not sorted.
    assert!(hits.iter().all(|h| h.2 == Some(f.shooters[0].0)));
    assert_eq!(
        observed
            .into_iter()
            .flat_map(|event| event.sounds)
            .collect::<Vec<_>>(),
        vec![
            format!("Sound_{}_Veteran", f.rows),
            format!("Sound_{}_Elite", f.rows)
        ]
    );
    f.no_repeat();
}

#[test]
fn ordinary_library_queued_area_secondary_and_sphere_boundary() {
    let mut f = fixture_custom(
        "Bands",
        200.0,
        false,
        30000000,
        1,
        10.0,
        5.0,
        "ENEMIES NEUTRALS",
        "SecondaryDamage = 25\n SecondaryDamageRadius = 10",
    );
    // Sphere edge distance 5.
    assert!(
        f.world
            .set_registered_object_position(f.victim, Vec3::new(16.0, 0.0, 0.0))
    );
    let secondary = f
        .world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(21.0, 0.0, 0.0))
        .unwrap(); // edge distance 10
    let outside = f
        .world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(21.01, 0.0, 0.0))
        .unwrap();
    f.accept();
    assert_eq!(f.world.objects[&f.victim].health.current, 100.0);
    assert_eq!(f.world.objects[&secondary].health.current, 175.0);
    assert_eq!(f.world.objects[&outside].health.current, 200.0);
    assert_eq!(f.xp(), 0.0);
    let observed = f.take_fx();
    let hits = observed
        .iter()
        .map(|event| (event.list.clone(), event.victim, event.source))
        .collect::<Vec<_>>();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|h| h.0 == format!("FX_{}_Regular", f.rows)));
}

#[test]
fn ordinary_library_queued_area_elevation_uses_3d_sphere() {
    let mut f = fixture("Elevation", 200.0, false, 30000000, 1, 10.0);
    assert!(
        f.world
            .set_registered_object_position(f.victim, Vec3::new(11.0, 20.0, 0.0))
    );
    f.world.objects.get_mut(&f.victim).unwrap().status.airborne_target = true;
    f.accept();
    assert!(f.world.objects[&f.victim].get_position().y > 10.0);
    assert_eq!(f.world.objects[&f.victim].health.current, 200.0);
    assert_eq!(f.world.objects[&f.intended].health.current, 9900.0);
    assert!(f.take_fx().is_empty());
}

#[test]
fn ordinary_library_queued_area_mask_fallback_and_intended_bypass() {
    // Some(players)/None(team_factory) preserves the current frozen-team Neutral fallback,
    // despite the players' explicit Enemies relationship.
    let mut f = fixture_custom("Mask", 200.0, false, 30000000, 1, 10.0, 5.0, "ENEMIES", "");
    f.world.objects.get_mut(&f.shooters[0]).unwrap().producer_id = Some(f.intended);
    f.accept();
    assert_eq!(f.world.objects[&f.victim].health.current, 200.0);
    assert_eq!(f.world.objects[&f.intended].health.current, 9900.0);
    assert_eq!(f.xp(), 0.0);
    assert!(f.take_fx().is_empty());
}

#[test]
fn ordinary_library_queued_area_source_and_producer_excluded() {
    let mut f = fixture_custom(
        "Producer",
        200.0,
        false,
        30000000,
        1,
        10.0,
        20.0,
        "ALLIES ENEMIES NEUTRALS",
        "",
    );
    f.world.objects.get_mut(&f.shooters[0]).unwrap().producer_id = Some(f.victim);
    f.accept();
    assert_eq!(f.world.objects[&f.shooters[0]].health.current, 500.0);
    assert_eq!(f.world.objects[&f.victim].health.current, 200.0);
    assert_eq!(f.world.objects[&f.intended].health.current, 9900.0);
    assert!(f.take_fx().is_empty());
}

#[test]
fn ordinary_library_queued_area_self_mask_includes_source_and_producer() {
    let mut f = fixture_custom(
        "Self",
        200.0,
        false,
        30000000,
        1,
        10.0,
        20.0,
        "SELF ALLIES ENEMIES NEUTRALS",
        "",
    );
    f.world.objects.get_mut(&f.shooters[0]).unwrap().producer_id = Some(f.victim);
    f.accept();
    assert_eq!(f.world.objects[&f.shooters[0]].health.current, 400.0);
    assert_eq!(f.world.objects[&f.victim].health.current, 100.0);
    assert_eq!(f.xp(), 0.0);
    f.expect_fx("Regular", Some(f.shooters[0]));
}

#[test]
fn ordinary_library_queued_area_secondary_dual_threshold_is_preserved() {
    let mut f = fixture_custom(
        "Threshold",
        200.0,
        false,
        30000000,
        1,
        10.0,
        5.0,
        "ENEMIES NEUTRALS",
        "SecondaryDamage = 25\n SecondaryDamageRadius = 5.0005",
    );
    assert!(
        f.world
            .set_registered_object_position(f.victim, Vec3::new(16.00025, 0.0, 0.0))
    );
    f.accept();
    assert_eq!(f.world.objects[&f.victim].health.current, 200.0);
    assert!(f.take_fx().is_empty());
}

#[test]
fn ordinary_library_queued_area_shock_after_nonlethal_damage() {
    let mut f = fixture_custom(
        "Shock",
        200.0,
        false,
        30000000,
        1,
        10.0,
        5.0,
        "ENEMIES NEUTRALS",
        "ShockWaveAmount = 20\n ShockWaveRadius = 30\n ShockWaveTaperOff = 1",
    );
    f.accept();
    let victim = &f.world.objects[&f.victim];
    assert_eq!(victim.health.current, 100.0);
    assert_eq!(victim.last_damage_source, Some(f.shooters[0]));
    assert_eq!(victim.last_damage_timestamp, Some(100));
    assert!(victim.movement.velocity.x > 0.0);
    assert!(victim.movement.velocity.y > 0.0);
    assert!(victim.is_shock_stunned());
    assert_eq!(f.xp(), 0.0);
    f.expect_fx("Regular", Some(f.shooters[0]));
}

#[test]
fn ordinary_library_queued_area_dead_but_installed_source_at_arrival() {
    let mut f = fixture("DeadInstalled", 100.0, false, 30, 1, 10.0);
    f.accept();
    while f.world.get_current_frame() < 110 {
        f.world.update();
    }
    let source = f.shooters[0];
    let object = f.world.objects.get_mut(&source).unwrap();
    object.health.current = 0.0;
    assert!(!object.is_alive());
    assert!(!object.status.destroyed);
    assert_eq!(f.world.objects[&f.victim].health.current, 100.0);
    f.world.update();
    assert!(!f.world.objects.contains_key(&f.victim));
    assert!(f.world.objects.contains_key(&source));
    assert_eq!(f.xp(), 10.0);
    assert_eq!(
        f.world.objects[&source].experience.level,
        VeterancyLevel::Veteran
    );
    f.expect_fx("Veteran", Some(source));
    f.no_repeat();
}

//! Ordinary (non-cfg(test)) Main library: real Weapon/DamageFX/FXList
//! parsers, admission, public target assignment, combat, onDie and removal. Public Main templates are used
//! here; the crate-local companion additionally loads real Object INI and checks
//! throttle state plus authored InstantDeathBehavior recursion.
#[path = "common/queued_damage_fx.rs"]
mod queued_damage_fx;
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate, VeterancyLevel};
use glam::Vec3;

fn admit_weapon(world: &mut GameLogic, name: &str, affects: &str, scatter: bool) {
    let scatter_fields = if scatter {
        "ScatterTarget = X:0 Y:10\n ScatterTargetScalar = 1"
    } else {
        ""
    };
    let weapon = queued_damage_fx::parse_weapon_source(&format!(
        "Weapon {name}\n PrimaryDamage = 100\n PrimaryDamageRadius = 5\n\
         AttackRange = 200\n DamageType = SMALL_ARMS\n RadiusDamageAffects = {affects}\n\
         ProjectileObject = NONE\n WeaponSpeed = 30000000\n DelayBetweenShots = 1000\n\
         ClipSize = 8\n ClipReloadTime = 1000\n PreAttackDelay = 0\n AntiGround = Yes\n\
         {scatter_fields}\nEnd\n"
    ));
    world.admit_receiving_weapon_snapshot(vec![weapon]).unwrap();
}

fn run(scatter: bool, count: usize, health: f32, route_to_sink: bool) {
    let label = format!(
        "{}_{}_{}_{}",
        if scatter { "Scatter" } else { "Hit" },
        count,
        health as u32,
        route_to_sink
    );
    let rule = format!("LibrarySplash{label}Weapon");
    let rows = format!("LibrarySplash{label}Rows");
    let mut world = GameLogic::new();
    admit_weapon(&mut world, &rule, "ENEMIES NEUTRALS", scatter);
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
    assert_eq!(world.admit_damage_fx_source(&damage_fx_source).unwrap(), 1);
    let fx = queued_damage_fx::admit_observed_source(&mut world, &damage_fx_source, &rows);
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
            .host_object_mut(shooter)
            .unwrap()
            .set_experience_sink(Some(sink));
    }
    assert!(world.host_objects()[&shooter].is_trainable());
    assert_eq!(world.host_objects()[&shooter].owner_player_id, Some(0));
    for target in &targets {
        assert_eq!(world.host_objects()[target].kill_experience_value(), 10.0);
        assert_eq!(world.host_objects()[target].owner_player_id, Some(1));
    }
    assert_eq!(world.host_objects()[&shooter].experience.current, 0.0);
    assert_eq!(
        world.host_objects()[&shooter].experience.level,
        VeterancyLevel::Rookie
    );
    assert_eq!(
        world.host_objects()[&shooter].weapon.as_ref().unwrap().ammo,
        Some(8)
    );
    assert!(fx.try_recv().is_err());
    world.set_current_frame(100);
    assert_eq!(
        world.player_relationship(0, 1),
        game_engine::common::game_common::Relationship::Enemies
    );
    // AttackObject commands enter the separately scoped queued-damage runtime.
    // Public set_target drives the ordinary host-combat path being repaired.
    let mut attack_output = generals_main::game_logic::host_attack_log::AttackEvents::default();
    let frame = world.get_frame();
    world
        .with_host_object_audio(shooter, |object, audio| {
            object.set_target(Some(anchor), frame, &mut attack_output, audio)
        })
        .unwrap();
    world.update();
    assert_eq!(world.get_current_frame(), 101);
    let source = &world.host_objects()[&shooter];
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
        assert_eq!(world.host_objects()[&sink].experience.current, awarded);
    }
    for target in &targets {
        if health > 100.0 {
            assert_eq!(world.host_objects()[target].health.current, health - 100.0);
            assert_eq!(world.host_objects()[target].next_damage_fx_time, 103);
            assert_eq!(
                world.host_objects()[target].last_damage_timestamp,
                Some(100)
            );
        } else {
            assert!(!world.host_objects().lookup(target).is_some());
        }
    }
    assert_eq!(
        world.host_objects()[&anchor].health.current,
        if scatter { 500.0 } else { 400.0 }
    );
    let calls: Vec<_> = fx.try_iter().collect();
    assert_eq!(calls.len(), count);
    let mut observed_victims: Vec<_> = calls.iter().map(|call| call.victim.unwrap()).collect();
    observed_victims.sort_unstable();
    let mut expected_victims: Vec<_> = targets.iter().map(|id| id.0).collect();
    expected_victims.sort_unstable();
    assert_eq!(
        observed_victims, expected_victims,
        "each victim completes exactly once"
    );
    for (index, event) in calls.iter().enumerate() {
        let level = if expected_xp == 0.0 {
            "Regular"
        } else if index == 0 {
            "Veteran"
        } else {
            "Elite"
        };
        assert_eq!(
            event.list,
            format!("FX_{rows}_{level}"),
            "each kill completes its own promotion before DamageFX"
        );
        assert_eq!(event.source, Some(shooter.0));
        assert_eq!(event.sounds, vec![format!("Sound_{rows}_{level}")]);
    }
    world.update();
    assert_eq!(
        world.host_objects()[&shooter].experience.current,
        expected_xp,
        "no duplicate kill credit"
    );
    if let Some(sink) = sink {
        assert_eq!(world.host_objects()[&sink].experience.current, awarded);
    }
    assert!(fx.try_recv().is_err());
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

fn shock_relationship_world(
    label: &str,
    weapon_name: &str,
    variant_is_reskin: bool,
    source_to_victim: game_engine::common::game_common::Relationship,
    victim_to_source: game_engine::common::game_common::Relationship,
) -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut world = GameLogic::new();
    let mut source_player = Player::new(0, Team::USA, &format!("{label}Source"), true);
    source_player.set_map_relationship(1, source_to_victim);
    let mut victim_player = Player::new(1, Team::China, &format!("{label}Victim"), true);
    victim_player.set_map_relationship(0, victim_to_source);
    world.add_player(source_player);
    world.add_player(victim_player);

    let variant = if variant_is_reskin {
        "ObjectReskin ShockRelationshipVariant ShockRelationshipBase\nEnd\n".to_owned()
    } else {
        "Object ShockRelationshipVariant\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n  MaxHealth = 500\n  InitialHealth = 500\n End\nEnd\n".to_owned()
    };
    world
        .admit_object_ini_sources([(
            format!("{label}.ini"),
            format!(
                "Object ShockRelationshipBase\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n  MaxHealth = 500\n  InitialHealth = 500\n End\n WeaponSet\n  Conditions = NONE\n  Weapon = PRIMARY {weapon_name}\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n{variant}"
            ),
        )])
        .expect("owner admits its own reskin/equivalence catalogue");
    admit_weapon(
        &mut world,
        weapon_name,
        "ENEMIES NOT_SIMILAR",
        weapon_name == "ShockRelationshipScatterWeapon",
    );
    let source = world
        .create_object_for_player("ShockRelationshipBase", 0, Vec3::ZERO)
        .unwrap();
    let anchor = world
        .create_object_for_player("ShockRelationshipVariant", 1, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let neighbor_position = if weapon_name == "ShockRelationshipScatterWeapon" {
        Vec3::new(10.0, 0.0, 10.0)
    } else {
        Vec3::new(12.0, 0.0, 0.0)
    };
    let neighbor = world
        .create_object_for_player("ShockRelationshipVariant", 1, neighbor_position)
        .unwrap();
    (world, source, anchor, neighbor)
}

fn fire_shock_relationship_weapon(
    world: &mut GameLogic,
    source: generals_main::game_logic::ObjectId,
    anchor: generals_main::game_logic::ObjectId,
) {
    world.set_current_frame(100);
    let mut attack_output = generals_main::game_logic::host_attack_log::AttackEvents::default();
    let frame = world.get_frame();
    world
        .with_host_object_audio(source, |object, audio| {
            object.set_target(Some(anchor), frame, &mut attack_output, audio)
        })
        .unwrap();
    world.update();
}

#[test]
fn instant_splash_uses_source_to_victim_similarity_and_receiving_world_relationships() {
    let (mut ally_source, ally_id, ally_anchor, ally_neighbor) = shock_relationship_world(
        "ShockAlly",
        "ShockRelationshipWeapon",
        true,
        game_engine::common::game_common::Relationship::Allies,
        game_engine::common::game_common::Relationship::Enemies,
    );
    let (mut enemy_source, enemy_id, enemy_anchor, enemy_neighbor) = shock_relationship_world(
        "ShockEnemy",
        "ShockRelationshipWeapon",
        false,
        game_engine::common::game_common::Relationship::Enemies,
        game_engine::common::game_common::Relationship::Enemies,
    );
    assert_eq!(
        (ally_id, ally_anchor, ally_neighbor),
        (enemy_id, enemy_anchor, enemy_neighbor)
    );

    fire_shock_relationship_weapon(&mut ally_source, ally_id, ally_anchor);
    assert_eq!(
        ally_source.host_objects()[&ally_neighbor].health.current,
        500.0,
        "the source considers the equivalent victim an ally despite the reverse being enemy"
    );
    assert_eq!(
        enemy_source.host_objects()[&enemy_neighbor].health.current,
        500.0
    );

    fire_shock_relationship_weapon(&mut enemy_source, enemy_id, enemy_anchor);
    assert_eq!(
        ally_source.host_objects()[&ally_neighbor].health.current,
        500.0
    );
    assert_eq!(
        enemy_source.host_objects()[&enemy_neighbor].health.current,
        400.0,
        "an enemy relationship in both directions admits the receiving world's splash"
    );
}

#[test]
fn scatter_splash_uses_source_to_victim_similarity_and_receiving_world_relationships() {
    let (mut ally_source, ally_id, ally_anchor, ally_neighbor) = shock_relationship_world(
        "ScatterAlly",
        "ShockRelationshipScatterWeapon",
        true,
        game_engine::common::game_common::Relationship::Allies,
        game_engine::common::game_common::Relationship::Enemies,
    );
    let (mut enemy_source, enemy_id, enemy_anchor, enemy_neighbor) = shock_relationship_world(
        "ScatterEnemy",
        "ShockRelationshipScatterWeapon",
        false,
        game_engine::common::game_common::Relationship::Enemies,
        game_engine::common::game_common::Relationship::Enemies,
    );
    assert_eq!(
        (ally_id, ally_anchor, ally_neighbor),
        (enemy_id, enemy_anchor, enemy_neighbor)
    );

    fire_shock_relationship_weapon(&mut ally_source, ally_id, ally_anchor);
    assert_eq!(
        ally_source.host_objects()[&ally_neighbor].health.current,
        500.0
    );
    assert_eq!(
        enemy_source.host_objects()[&enemy_neighbor].health.current,
        500.0
    );

    fire_shock_relationship_weapon(&mut enemy_source, enemy_id, enemy_anchor);
    assert_eq!(
        ally_source.host_objects()[&ally_neighbor].health.current,
        500.0
    );
    assert_eq!(
        enemy_source.host_objects()[&enemy_neighbor].health.current,
        400.0,
        "the scatter miss follows source-to-victim similarity before victim-to-source mask"
    );
}

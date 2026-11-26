//! Ordinary production-library regression for moving-projectile small- and large-radius hits.
//! Real AttackObject admission, Weapon/DamageFX parsers, due frames, XP and FX.
use game_engine::common::audio::audio_event_rts::{AudioEventInfo, ST_GLOBAL};
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::system::DataChunkOutput;
use generals_main::assets::ini_template_loader::with_weapons_from_ini_text;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::host_base_defense::HostHiveDamageClass;
use generals_main::game_logic::weapon_bootstrap::HostProjectileFlight;
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team, VeterancyLevel};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

const TIGHT_SPLASH_RADIUS: f32 = 5.0;

struct Fixture {
    world: GameLogic,
    shooters: Vec<ObjectId>,
    victim: ObjectId,
    sink: Option<ObjectId>,
    rows: String,
    _map_dir: tempfile::TempDir,
}

#[test]
fn projectile_damage_fx_throttle_belongs_to_the_driving_session() {
    let make = |time| {
        fixture_custom_with_ai(
            "OwnedArmorFx",
            150.0,
            false,
            30,
            1,
            10.0,
            TIGHT_SPLASH_RADIUS,
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

#[test]
fn projectile_damage_fx_continues_after_restore_and_peer_reset() {
    let make = |time| {
        fixture_custom_with_ai(
            "OwnedArmorFxSave",
            2000.0,
            false,
            30,
            1,
            10.0,
            TIGHT_SPLASH_RADIUS,
            "ENEMIES NEUTRALS",
            "DelayBetweenShots = 100",
            true,
            None,
            Some(time),
        )
    };
    let mut running = make(1000);
    running.accept();
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&running.world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = bincode_legacy::deserialize(&encoded).unwrap();
    let mut restored = make(1000);
    builder
        .restore_from_snapshot(&decoded, &mut restored.world)
        .unwrap();

    let mut peer = make(2000);
    peer.world.reset();
    peer.world
        .admit_damage_fx_source(&format!(
            "DamageFX {}\n ThrottleTime = SMALL_ARMS 5000\nEnd\n",
            peer.rows
        ))
        .unwrap();
    let mut invalid = builder.create_world_snapshot(&running.world).unwrap();
    invalid.version = u32::MAX;
    let before = restored.world.get_current_frame();
    assert!(
        builder
            .restore_from_snapshot(&invalid, &mut restored.world)
            .is_err()
    );
    assert_eq!(restored.world.get_current_frame(), before);

    for _ in 0..50 {
        if running
            .world
            .host_object(running.victim)
            .unwrap()
            .health
            .current
            < 2000.0
            && restored
                .world
                .host_object(restored.victim)
                .unwrap()
                .health
                .current
                < 2000.0
        {
            break;
        }
        running.world.update();
        restored.world.update();
    }
    let original = running.world.host_object(running.victim).unwrap();
    let loaded = restored.world.host_object(restored.victim).unwrap();
    assert_eq!(original.health.current, 1900.0);
    assert_eq!(loaded.health.current, original.health.current);
    assert_eq!(loaded.next_damage_fx_time, original.next_damage_fx_time);
    assert_ne!(original.next_damage_fx_time, 0);
    let deadline = original.next_damage_fx_time;
    let damage_type = original.last_damage_fx_done;
    let after_impact = builder.create_world_snapshot(&running.world).unwrap();
    let encoded = bincode_legacy::serialize(&after_impact).unwrap();
    let decoded = bincode_legacy::deserialize(&encoded).unwrap();
    let mut after_load = make(1000);
    builder
        .restore_from_snapshot(&decoded, &mut after_load.world)
        .unwrap();
    let loaded = after_load.world.host_object(after_load.victim).unwrap();
    assert_eq!(loaded.next_damage_fx_time, deadline);
    assert_eq!(loaded.last_damage_fx_done, damage_type);
    let mut different_type = make(1000);
    builder
        .restore_from_snapshot(&decoded, &mut different_type.world)
        .unwrap();
    // HitStructure is the existing explosive damage entry point. Throttle
    // observes the damage type, so this does not require a launcher or splash.
    let (destroyed, blocked) = different_type
        .world
        .apply_host_hive_damage_from(
            different_type.victim,
            40.0,
            HostHiveDamageClass::HitStructure,
            None,
            Some(different_type.shooters[0]),
        )
        .unwrap();
    assert!(!destroyed && !blocked);
    let victim = different_type
        .world
        .host_object(different_type.victim)
        .unwrap();
    assert_eq!(victim.health.current, 1860.0);
    assert_eq!(
        victim.last_damage_fx_done,
        Some(generals_main::game_logic::combat::DamageType::Explosive)
    );
    assert!(
        victim.next_damage_fx_time < deadline,
        "a different damage type is not throttled"
    );
    {
        use generals_main::save_load::{Snapshot, WorldSnapshot, XferLoad, XferSave};
        let mut xfer_source = builder.create_world_snapshot(&running.world).unwrap();
        let mut stream = std::io::Cursor::new(Vec::new());
        xfer_source.xfer(&mut XferSave::new(&mut stream)).unwrap();
        let mut xfer_snapshot = WorldSnapshot::default();
        xfer_snapshot
            .xfer(&mut XferLoad::new(std::io::Cursor::new(
                stream.into_inner(),
            )))
            .unwrap();
        let mut xfer_receiver = make(1000);
        builder
            .restore_from_snapshot(&xfer_snapshot, &mut xfer_receiver.world)
            .unwrap();
        let loaded = xfer_receiver
            .world
            .host_object(xfer_receiver.victim)
            .unwrap();
        assert_eq!(loaded.next_damage_fx_time, deadline);
        assert_eq!(loaded.last_damage_fx_done, damage_type);
    }
    for _ in 0..20 {
        if running
            .world
            .host_object(running.victim)
            .unwrap()
            .health
            .current
            < 1900.0
            && after_load
                .world
                .host_object(after_load.victim)
                .unwrap()
                .health
                .current
                < 1900.0
        {
            break;
        }
        running.world.update();
        after_load.world.update();
    }
    let original = running.world.host_object(running.victim).unwrap();
    let loaded = after_load.world.host_object(after_load.victim).unwrap();
    assert!(
        original.health.current < 1900.0,
        "a second hit must occur inside the FX throttle"
    );
    assert_eq!(loaded.health.current, original.health.current);
    assert_eq!(original.next_damage_fx_time, deadline);
    assert_eq!(loaded.next_damage_fx_time, deadline);
    for _ in 0..100 {
        if running
            .world
            .host_object(running.victim)
            .unwrap()
            .next_damage_fx_time
            != deadline
            && after_load
                .world
                .host_object(after_load.victim)
                .unwrap()
                .next_damage_fx_time
                != deadline
        {
            break;
        }
        running.world.update();
        after_load.world.update();
    }
    let original = running.world.host_object(running.victim).unwrap();
    let loaded = after_load.world.host_object(after_load.victim).unwrap();
    assert!(
        original.next_damage_fx_time > deadline,
        "the next hit after expiry must admit FX again"
    );
    assert_eq!(loaded.health.current, original.health.current);
    assert_eq!(loaded.next_damage_fx_time, original.next_damage_fx_time);
    let deadline = original.next_damage_fx_time;

    // A malformed current Object Xfer chunk must leave the existing throttle intact.
    let mut bad = builder.create_world_snapshot(&running.world).unwrap();
    let marker = bad
        .lifecycle_tail
        .windows(4)
        .rposition(|window| window == b"OXOB")
        .unwrap();
    bad.lifecycle_tail[marker + 8..marker + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        builder
            .restore_from_snapshot(&bad, &mut after_load.world)
            .is_err()
    );
    assert_eq!(
        after_load
            .world
            .host_object(after_load.victim)
            .unwrap()
            .next_damage_fx_time,
        deadline
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
        TIGHT_SPLASH_RADIUS,
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
    let mut world = GameLogic::new();
    let weapon_ini = format!(
        r#"
Weapon {rule}
 PrimaryDamage = 100
 PrimaryDamageRadius = {radius}
 AttackRange = 200
 DamageType = SMALL_ARMS
 RadiusDamageAffects = {affects}
 ProjectileObject = LibraryMoving{label}Projectile
 WeaponSpeed = {speed}
 DelayBetweenShots = 2000
 ClipSize = 8
 ClipReloadTime = 1000
 PreAttackDelay = 0
 AntiGround = Yes
 {extra}
End
"#
    );
    let mut weapon = world.with_own_services(|| {
        with_weapons_from_ini_text(&weapon_ini, |count| {
            assert_eq!(count, 1);
            gamelogic::weapon::with_weapon_store(|store| {
                let parsed = store.find_weapon_template(&rule).unwrap();
                assert_eq!(
                    parsed.projectile_name,
                    format!("LibraryMoving{label}Projectile")
                );
                assert_eq!(parsed.weapon_speed, speed as f32 / 30.0);
                parsed.shared_definition().as_ref().clone()
            })
            .unwrap()
        })
    });
    if let Some(angle) = owned_angle {
        weapon.radius_damage_angle = angle;
    }
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
FXList FX_{rows}_BurnedShell
 Sound
  Name = Shell_{rows}_Burned
 End
End
"#
    );
    world.admit_damage_fx_source(&damage_fx_source).unwrap();
    world.admit_fx_source(&damage_fx_source).unwrap();
    if let Some(throttle) = owned_throttle_ms {
        world
            .admit_damage_fx_source(&format!(
                "DamageFX {rows}\n ThrottleTime = SMALL_ARMS {throttle}\nEnd\n"
            ))
            .unwrap();
    }
    let actor = |name: &str, hp: f32, xp: f32, armed| {
        let second_threshold = 20.0_f32.max(threshold + 1.0);
        let ai = if with_ai || armed {
            "Behavior = AIUpdateInterface FlightWitnessAI\nEnd"
        } else {
            ""
        };
        let body = if name == "QueuedVictim" {
            format!("ArmorSet\n Conditions = None\n DamageFX = {rows}\nEnd")
        } else {
            String::new()
        };
        let weapon_set = if armed {
            format!("WeaponSet\n Conditions = None\n Weapon = PRIMARY {rule}\nEnd")
        } else {
            String::new()
        };
        let death = if armed {
            "Behavior = SlowDeathBehavior OnDeath\n DestructionDelay = 5000\nEnd"
        } else {
            "Behavior = DestroyDie OnDeath\nEnd"
        };
        format!(
            r#"Object {name}
 KindOf = INFANTRY SELECTABLE
 Geometry = SPHERE
 GeometryMajorRadius = 1
 GeometryHeight = 2
 IsTrainable = Yes
 ExperienceRequired = 0 {threshold} {second_threshold} 1000
 ExperienceValue = {xp} {xp} {xp} {xp}
 Body = ActiveBody Body
  MaxHealth = {hp}
  InitialHealth = {hp}
 End
 {death}
 {ai}
 {body}
 {weapon_set}
End
"#
        )
    };
    world
        .admit_object_ini_sources([(
            "MovingProjectileFixture.ini".into(),
            format!(
                r#"{}{}{}Object LibraryMoving{label}Projectile
 KindOf = PROJECTILE
 Geometry = SPHERE
 GeometryMajorRadius = 1
 Body = ActiveBody Body
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = DumbProjectileBehavior Flight
  FirstHeight = 6
  SecondHeight = 6
  FirstPercentIndent = 25%
  SecondPercentIndent = 75%
 End
 Behavior = PhysicsBehavior ShellContact
  Mass = 0.01
 End
 Behavior = FXListDie BurnedShell
  DeathTypes = NONE +BURNED
  DeathFX = FX_{rows}_BurnedShell
 End
 Behavior = DestroyDie Retire
 End
End
"#,
                actor("QueuedSource", 500.0, 0.0, true),
                actor("QueuedVictim", health, 10.0, false),
                actor("QueuedSink", 500.0, 0.0, false)
            ),
        )])
        .unwrap();

    // DumbProjectileBehavior samples the height along its authored path.
    // A receiving terrain and partition must exist before the launch.
    let map_dir = tempfile::tempdir().unwrap();
    let path = map_dir.path().join("ProjectileFixture.map");
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [32, 32, 1, 1024] {
        map.write_int(value);
    }
    for _ in 0..1024 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    assert!(world.load_map(path.to_str().unwrap()));
    world
        .admit_receiving_weapon_snapshot(vec![(rule.clone(), weapon)])
        .unwrap();
    world.with_own_services(|| {
        let manager = initialize_global_audio_manager();
        let mut manager = manager.lock().unwrap();
        for level in ["Regular", "Veteran", "Elite"] {
            let name = format!("Sound_{rows}_{level}");
            manager.register_audio_event_info(AudioEventInfo {
                sounds: vec![format!("{name}.wav")],
                audio_name: name,
                type_field: ST_GLOBAL,
                ..AudioEventInfo::default()
            });
        }
        let name = format!("Shell_{rows}_Burned");
        manager.register_audio_event_info(AudioEventInfo {
            sounds: vec![format!("{name}.wav")],
            audio_name: name,
            type_field: ST_GLOBAL,
            ..AudioEventInfo::default()
        });
    });
    world.clear_all_players();
    for (id, team, name) in [(0, Team::USA, "Shooter"), (1, Team::China, "Victim")] {
        let mut player = Player::new(id, team, name, id == 0);
        player.alliance_team = id as i32;
        player.is_alive = true;
        world.add_player(player);
    }
    assert!(world.select_local_player(Some(0)));
    world.with_own_services(|| {
        gamelogic::system::engine_stores::shroud_manager()
            .lock()
            .unwrap()
            .reveal_map_for_player_permanently(0)
            .unwrap();
    });
    let shooters: Vec<_> = (0..count)
        .map(|_| {
            world
                .create_object_for_player("QueuedSource", 0, Vec3::new(120.0, 0.0, 120.0))
                .unwrap()
        })
        .collect();
    let victim = world
        .create_object_for_player("QueuedVictim", 1, Vec3::new(150.0, 0.0, 120.0))
        .unwrap();
    let sink = sink.then(|| {
        world
            .create_object_for_player("QueuedSink", 0, Vec3::new(20.0, 0.0, 120.0))
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
    assert_eq!(world.host_objects()[&victim].owner_player_id, Some(1));
    assert_eq!(world.host_objects()[&victim].kill_experience_value(), 10.0);
    assert_eq!(
        world.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    world.set_current_frame(99);
    Fixture {
        world,
        shooters,
        victim,
        sink,
        rows,
        _map_dir: map_dir,
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
        // A restore receiver starts without an outstanding player command.
        self.world.queue_command(GameCommand {
            command_type: CommandType::AttackObject {
                target_id: self.victim,
            },
            player_id: 0,
            command_id: 1,
            timestamp: UNIX_EPOCH + Duration::from_secs(1),
            selected_units: self.shooters.clone(),
            modifier_keys: ModifierKeys::default(),
        });
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
            .filter_map(|id| self.world.host_object(*id))
            .map(|o| o.experience.current)
            .sum()
    }
    fn damage_sounds(&self) -> Vec<(String, Vec3)> {
        self.world.with_own_services(|| {
            let manager =
                get_global_audio_manager().expect("fixture initialized its audio manager");
            let manager = manager.lock().unwrap();
            manager
                .pending_play_events()
                .filter(|event| event.get_event_name().starts_with("Sound_LibraryMoving"))
                .map(|event| {
                    let position = event
                        .get_position()
                        .expect("damage sound has a victim pose");
                    assert_eq!(event.get_player_index(), 1);
                    (
                        event.get_event_name().to_owned(),
                        Vec3::new(position.x, position.z, position.y),
                    )
                })
                .collect()
        })
    }

    fn expect_fx(&self, level: &str, source: Option<ObjectId>) {
        assert_eq!(
            self.damage_sounds(),
            [(
                format!("Sound_{}_{level}", self.rows),
                Vec3::new(150.0, 0.0, 120.0)
            )]
        );
        if let Some(victim) = self.world.host_object(self.victim) {
            assert_eq!(victim.last_damage_source, source);
        }
    }

    fn shell_death_sounds(&self) -> usize {
        self.world.with_own_services(|| {
            let manager =
                get_global_audio_manager().expect("fixture initialized its audio manager");
            manager
                .lock()
                .unwrap()
                .pending_play_request_count_for(&format!("Shell_{}_Burned", self.rows))
        })
    }

    fn no_repeat(&mut self) {
        let xp = self.xp();
        let sounds = self.damage_sounds();
        let health = self
            .world
            .host_object(self.victim)
            .map(|victim| victim.health.current);
        let sink_xp = self
            .sink
            .map(|id| self.world.host_objects()[&id].experience.current);
        for _ in 0..10 {
            self.world.update();
            assert_eq!(self.xp(), xp);
            assert_eq!(
                self.world
                    .host_object(self.victim)
                    .map(|victim| victim.health.current),
                health
            );
            assert!(self.world.combat_system().projectiles_snapshot().is_empty());
            assert_eq!(
                self.sink
                    .map(|id| self.world.host_objects()[&id].experience.current),
                sink_xp
            );
            assert_eq!(self.damage_sounds(), sounds);
        }
    }
}

fn advance_to_impact(f: &mut Fixture, health: f32) {
    let mut previous = None;
    let mut advanced = false;
    for _ in 0..50 {
        let shots = f.world.combat_system().projectiles_snapshot();
        if shots.is_empty() {
            assert!(advanced);
            return;
        }
        assert_eq!(f.world.host_objects()[&f.victim].health.current, health);
        assert!(f.damage_sounds().is_empty());
        assert_eq!(f.xp(), 0.0, "no early kill credit");
        if let Some(sink) = f.sink {
            assert_eq!(f.world.host_objects()[&sink].experience.current, 0.0);
        }
        assert!(
            shots
                .iter()
                .all(|shot| shot.projectile_object_name.starts_with("LibraryMoving"))
        );
        assert!(
            shots.iter().all(|shot| shot.speed == 30.0
                && matches!(shot.flight, Some(HostProjectileFlight::Dumb(_))))
        );
        let position = shots[0].position;
        if let Some(previous) = previous {
            if position != previous {
                advanced = true;
            }
        }
        previous = Some(position);
        f.world.update();
    }
    panic!("moving shot failed to retire");
}

#[test]
fn authored_shell_contact_requires_geometry_overlap() {
    for lateral_offset in [4.0, 0.0] {
        let mut f = fixture_custom(
            "GeometryContact",
            150.0,
            false,
            30,
            1,
            10.0,
            0.0,
            "ENEMIES",
            "",
        );
        f.accept();
        let position = Vec3::new(150.0, 0.0, 120.0 + lateral_offset);
        assert!(f.world.set_registered_object_position(f.victim, position));
        let shell = f.world.combat_system().projectiles_snapshot()[0];
        let remaining_samples = shell.flight_runtime.path[shell.flight_runtime.step..].to_vec();
        for sample in remaining_samples {
            f.world.update();
            let overlap = sample.distance_squared(position) <= 4.0; // two radius-one spheres
            assert_eq!(
                f.world.combat_system().projectiles_snapshot().is_empty(),
                overlap,
                "contact at {sample:?} with target at {position:?}"
            );
            if overlap {
                break;
            }
        }
        f.world.update();
        assert!(f.world.combat_system().projectiles_snapshot().is_empty());
        assert_eq!(f.world.host_object(f.victim).unwrap().health.current, 150.0);
    }
}

#[test]
fn authored_shell_incidental_contact_uses_its_weapon_mask() {
    for (mask, expect_contact) in [("ALLIES", false), ("ENEMIES", true)] {
        let mut f = fixture_custom(
            "IncidentalContact",
            150.0,
            false,
            30,
            1,
            10.0,
            0.0,
            "ENEMIES",
            &format!("ProjectileCollidesWith = {mask}"),
        );
        f.accept();
        let shell = f.world.combat_system().projectiles_snapshot()[0];
        assert_eq!(shell.target_id, Some(f.victim));
        let samples = shell.flight_runtime.path
            [shell.flight_runtime.step..shell.flight_runtime.step + 12]
            .to_vec();
        let incidental = Vec3::new(samples[3].x, 0.0, samples[3].z);
        assert!(samples.iter().any(|p| p.distance_squared(incidental) <= 4.0));
        let obstacle = f.world
            .create_object_for_player("QueuedVictim", 1, incidental)
            .unwrap();
        assert!(f.world.set_registered_object_position(obstacle, incidental));
        for sample in samples {
            f.world.update();
            assert_eq!(
                f.world.host_object(obstacle).unwrap().get_position(),
                incidental
            );
            let contact = expect_contact && sample.distance_squared(incidental) <= 4.0;
            assert_eq!(
                f.world.combat_system().projectiles_snapshot().is_empty(),
                contact,
                "{mask} at {sample:?} with incidental object at {incidental:?}"
            );
            if contact {
                break;
            }
        }
    }
}

#[test]
fn authored_shell_contact_uses_position_and_the_splash_mask() {
    for (radius, affects, expected_health) in [
        (0.0, "ENEMIES NEUTRALS", 150.0),
        (10.0, "ALLIES", 150.0),
        (10.0, "ENEMIES", 50.0),
    ] {
        let mut f = fixture_custom(
            "PositionalContact",
            150.0,
            false,
            30,
            1,
            10.0,
            radius,
            affects,
            "",
        );
        f.accept();
        advance_to_impact(&mut f, 150.0);
        assert_eq!(
            f.world.host_objects()[&f.victim].health.current,
            expected_health,
            "DumbProjectileBehavior::detonate fires at a position with radius {radius} and mask {affects}"
        );
        if expected_health == 150.0 {
            assert!(f.damage_sounds().is_empty());
        } else {
            f.expect_fx("Regular", Some(f.shooters[0]));
        }
        f.no_repeat();
    }
}

#[test]
fn projectile_suicide_applies_to_the_shell_inside_its_blast() {
    for (radius, affects, launcher_hp, victim_hp) in [
        (50.0, "SUICIDE ENEMIES", 500.0, 50.0),
        (50.0, "SUICIDE SELF ALLIES ENEMIES", 400.0, 50.0),
        (0.0, "SUICIDE ENEMIES", 500.0, 150.0),
    ] {
        let make = || {
            fixture_custom(
                "ShellSuicide",
                150.0,
                false,
                30,
                1,
                10.0,
                radius,
                affects,
                "DeathType = BURNED",
            )
        };
        let mut f = make();
        f.accept();
        let shell = f.world.combat_system().projectiles_snapshot()[0].id;
        assert_eq!(f.world.host_object(shell).unwrap().health.current, 100.0);
        assert_eq!(f.shell_death_sounds(), 0);
        let launch_position = f.world.host_object(shell).unwrap().get_position();
        for _ in 0..4 {
            f.world.update();
        }
        let flight_position = f.world.combat_system().projectiles_snapshot()[0].position;
        let shell_object = f.world.host_object(shell).unwrap();
        assert_ne!(flight_position, launch_position);
        assert_eq!(shell_object.get_position(), flight_position);
        assert_eq!(shell_object.producer_id, Some(f.shooters[0]));
        let frame =
            generals_main::presentation_frame::PresentationFrame::try_build_from_logic(&mut f.world, 0).unwrap();
        assert_eq!(
            frame
                .projectiles
                .iter()
                .filter(|shot| shot.id == shell)
                .count(),
            1
        );
        assert!(
            frame
                .direct_host_drawables
                .iter()
                .all(|item| item.object.id != shell)
        );

        let builder = generals_main::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&f.world).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = bincode_legacy::deserialize(&bytes).unwrap();
        let mut restored = make();
        builder
            .restore_from_snapshot(&decoded, &mut restored.world)
            .unwrap();
        let restored_shell = restored.world.host_object(shell).unwrap();
        assert_eq!(restored_shell.health.current, 100.0);
        assert_eq!(restored_shell.get_position(), flight_position);
        assert_eq!(restored_shell.producer_id, Some(restored.shooters[0]));

        for session in [&mut f, &mut restored] {
            advance_to_impact(session, 150.0);
            let launcher = session.world.host_object(session.shooters[0]).unwrap();
            assert_eq!(
                launcher.health.current, launcher_hp,
                "SUICIDE belongs to the shell"
            );
            assert_eq!(
                session
                    .world
                    .host_object(session.victim)
                    .unwrap()
                    .health
                    .current,
                victim_hp
            );
            assert!(session.world.host_object(shell).is_none());
            assert_eq!(session.shell_death_sounds(), 1);
            session.no_repeat();
            assert_eq!(session.shell_death_sounds(), 1);
        }
    }
}

#[test]
fn moving_tight_splash_lethal_completes_before_fx() {
    let mut f = fixture("Lethal", 100.0, false, 30, 1, 10.0);
    f.accept();
    assert_eq!(f.world.combat_system().projectiles_snapshot().len(), 1);
    assert_eq!(f.xp(), 0.0);
    advance_to_impact(&mut f, 100.0);
    assert!(f.world.host_object(f.victim).is_none());
    assert_eq!(f.xp(), 10.0);
    f.expect_fx("Veteran", Some(f.shooters[0]));
    f.no_repeat();
}

// Shots use the authored DumbProjectileBehavior flight and the live damage path.
fn moving_case(
    label: &str,
    radius: f32,
    health: f32,
    sink: bool,
    missing: bool,
    dead: bool,
    count: usize,
) {
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
        f.world.destroy_object(source).unwrap();
        f.world.update();
        assert!(f.world.host_object(source).is_none());
    } else if dead {
        let (lethal, blocked) = f
            .world
            .apply_host_hive_damage_from(
                source,
                500.0,
                HostHiveDamageClass::HitStructure,
                None,
                None,
            )
            .unwrap();
        assert!(lethal && !blocked);
    }
    advance_to_impact(&mut f, health);
    if dead {
        let launcher = f
            .world
            .host_object(source)
            .expect("authored slow death retains the launcher through impact");
        assert!(!launcher.is_alive());
        assert!(!launcher.status.destroyed);
    }
    let lethal = health <= 100.0;
    if lethal {
        assert!(f.world.host_object(f.victim).is_none());
    } else {
        let victim = &f.world.host_objects()[&f.victim];
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
        assert_eq!(f.world.host_objects()[&sink].experience.current, 10.0);
    }
    let killer = if missing {
        None
    } else if count == 1 {
        Some(source)
    } else {
        f.shooters
            .iter()
            .copied()
            .find(|id| f.world.host_objects()[id].experience.current == 10.0)
    };
    f.expect_fx(if xp > 0.0 { "Veteran" } else { "Regular" }, killer);
    f.no_repeat();
}
#[test]
fn moving_tight_splash_nonlethal() {
    moving_case(
        "TightSplashNonlethal",
        TIGHT_SPLASH_RADIUS,
        150.0,
        false,
        false,
        false,
        1,
    );
}
#[test]
fn moving_tight_splash_sink() {
    moving_case(
        "TightSplashSink",
        TIGHT_SPLASH_RADIUS,
        100.0,
        true,
        false,
        false,
        1,
    );
}
#[test]
fn moving_tight_splash_missing_source() {
    moving_case(
        "TightSplashMissing",
        TIGHT_SPLASH_RADIUS,
        100.0,
        false,
        true,
        false,
        1,
    );
}
#[test]
fn moving_tight_splash_dead_installed_source() {
    moving_case(
        "TightSplashDead",
        TIGHT_SPLASH_RADIUS,
        100.0,
        false,
        false,
        true,
        1,
    );
}
#[test]
fn moving_tight_splash_duplicate_hits_credit_once() {
    moving_case(
        "TightSplashDuplicate",
        TIGHT_SPLASH_RADIUS,
        100.0,
        false,
        false,
        false,
        2,
    );
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
        .create_object_for_player("QueuedVictim", 1, Vec3::new(151.0, 0.0, 120.0))
        .unwrap();
    f.accept();
    advance_to_impact(&mut f, 100.0);
    assert!(f.world.host_object(f.victim).is_none());
    assert!(f.world.host_object(other).is_none());
    assert_eq!(f.xp(), 20.0);
    assert_eq!(
        f.world.host_objects()[&f.shooters[0]].experience.level,
        VeterancyLevel::Elite
    );
    let hits = f.damage_sounds();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].0, format!("Sound_{}_Veteran", f.rows));
    assert_eq!(hits[1].0, format!("Sound_{}_Elite", f.rows));
    let mut positions = [hits[0].1, hits[1].1];
    positions.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert_eq!(
        positions,
        [Vec3::new(150.0, 0.0, 120.0), Vec3::new(151.0, 0.0, 120.0)]
    );
    f.no_repeat();
}
fn worlds(radius: f32) {
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
    assert_eq!(a.world.host_objects()[&a.victim].health.current, 100.0);
    b.accept();
    for _ in 0..50 {
        a.world.update();
        if a.world.combat_system().projectiles_snapshot().is_empty() {
            assert_eq!(a.xp(), 10.0);
            assert_eq!(b.xp(), 0.0);
            assert_eq!(b.world.host_objects()[&b.victim].health.current, 100.0);
            assert_eq!(b.world.combat_system().projectiles_snapshot().len(), 1);
            a.expect_fx("Veteran", Some(a.shooters[0]));
            b.world.update();
            assert!(b.world.combat_system().projectiles_snapshot().is_empty());
            assert_eq!(b.xp(), 10.0);
            assert_eq!(
                b.world.host_objects()[&b.shooters[0]].experience.level,
                VeterancyLevel::Rookie
            );
            b.expect_fx("Regular", Some(b.shooters[0]));
            a.no_repeat();
            b.no_repeat();
            return;
        }
        b.world.update();
        assert_eq!(a.xp(), 0.0);
        assert_eq!(b.xp(), 0.0);
        assert!(a.damage_sounds().is_empty());
        assert!(b.damage_sounds().is_empty());
    }
    panic!("interleaved shots did not retire");
}
#[test]
fn moving_tight_splash_interleaved_worlds() {
    worlds(TIGHT_SPLASH_RADIUS);
}
#[test]
fn moving_area_interleaved_worlds() {
    worlds(10.0);
}
#[test]
fn moving_area_uses_projectile_default_team_identity_not_launcher_custom_team() {
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
        .create_object_for_player("QueuedVictim", 1, Vec3::new(151.0, 0.0, 120.0))
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
    advance_to_impact(&mut f, 150.0);
    // The projectile belongs to the shooter's default Team, while the launcher
    // has a custom Team name. The exact default-Team override still admits the
    // collateral hit because C++ uses the projectile as detonation source.
    assert_eq!(f.world.host_objects()[&collateral].health.current, 50.0);
    assert_eq!(f.world.host_objects()[&f.victim].health.current, 50.0);
    assert_eq!(f.damage_sounds().len(), 2);
    assert_eq!(f.xp(), 0.0);
}

#[test]
fn moving_area_projectile_default_team_identity_controls_collateral_filter() {
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
        .create_object_for_player("QueuedVictim", 1, Vec3::new(151.0, 0.0, 120.0))
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
    advance_to_impact(&mut f, 150.0);
    assert_eq!(f.world.host_objects()[&collateral].health.current, 50.0);
}

#[test]
fn moving_directional_splash_uses_projectile_pose_and_receiving_weapon_angle() {
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
            .create_object_for_player("QueuedVictim", 1, Vec3::new(140.0, 0.0, 120.0))
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
        narrow.world.host_objects()[&narrow_victim].health.current,
        150.0,
        "the victim is behind the moving projectile even though it is ahead of the launcher"
    );
    assert_eq!(
        full.world.host_objects()[&full_victim].health.current,
        50.0,
        "the same-named weapon in the peer session has a full-circle angle"
    );
}

#[test]
fn moving_area_shock_final_state() {
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
    advance_to_impact(&mut f, 200.0);
    let victim = &f.world.host_objects()[&f.victim];
    assert_eq!(victim.health.current, 100.0);
    assert_eq!(victim.last_damage_source, Some(f.shooters[0]));
    assert_eq!(
        victim.last_damage_timestamp,
        Some((f.world.get_current_frame() - 1) as u32)
    );
    assert!(victim.movement.velocity.x > 0.0);
    assert!(victim.movement.velocity.y > 0.0);
    assert!(victim.is_shock_stunned());
    f.expect_fx("Regular", Some(f.shooters[0]));
}

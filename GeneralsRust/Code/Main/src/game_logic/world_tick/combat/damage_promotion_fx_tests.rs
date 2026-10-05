//! CPP ActiveBody.cpp646-653 credits the actual killer BEFORE damage FX.
//! DamageFX.cpp61,83 selects the then-current SOURCE veterancy, not the XP sink.
//! Actual parsed Main rules/admission and normal combat; no fabricated XP/context.
use super::*;
use crate::game_logic::combat::tests::combat_test_guard;
use crate::game_logic::game_logic::gameworld_authority::{
    GameWorldAuthority, current_gameworld_authority, publish_gameworld_authority,
};
use crate::game_logic::host_transition_damage_fx::take_dispatched_armor_damage_fx;
use game_engine::common::ini::INI;
use game_engine::common::ini::ini_damage_fx::{
    DamageFX, get_damage_fx_store_mut, init_global_damage_fx_store,
};

const ROWS: &str = "DamagePromotionOrderRows";
const REGULAR_FX: &str = "FX_DamagePromotionOrderRegular";
const VETERAN_FX: &str = "FX_DamagePromotionOrderVeteran";
const RULE: &str = "DamagePromotionOrderWeapon";

#[cfg(not(target_arch = "wasm32"))]
fn isolated(test: &str, run: impl FnOnce()) {
    use std::io::Read;
    use std::process::Stdio;
    const CHILD: &str = "GENERALS_DAMAGE_PROMOTION_FX_CHILD";
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
        .expect("spawn actual promotion-order witness");
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
        "promotion-order witness exceeded deadline: {exact}: {output:?}"
    );
    assert!(
        status.success(),
        "promotion-order witness failed: {exact}: {output:?}"
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

// Child isolation contains the existing process catalogs/output queues. Exact
// previous WeaponStore, authority/clock and prior named DamageFX are restored
// on normal/unwind return; the child may initialize an absent DamageFX catalog.
struct RestoreInputs {
    store: Option<gamelogic::weapon::WeaponStore>,
    authority: GameWorldAuthority,
    frame: u32,
    previous_rows: Option<DamageFX>,
}
impl RestoreInputs {
    fn install() -> Self {
        init_global_damage_fx_store();
        let saved = Self {
            store: gamelogic::weapon::with_weapon_store_mut(std::mem::take).ok(),
            authority: current_gameworld_authority(),
            frame: crate::game_logic::host_historic_bonus::logic_frame(),
            previous_rows: get_damage_fx_store_mut().unwrap().remove_damage_fx(ROWS),
        };
        publish_gameworld_authority(GameWorldAuthority::DEFAULT_OFF);
        gamelogic::initialize_weapon_store().expect("actual catalog presence");
        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            crate::assets::ini_template_loader::register_weapons_from_ini_text(
                r#"
Weapon DamagePromotionOrderWeapon
  PrimaryDamage = 100
  PrimaryDamageRadius = 0
  AttackRange = 200
  DamageType = SMALL_ARMS
  ProjectileObject = NONE
  WeaponSpeed = 30000000
  DelayBetweenShots = 1000
  ClipSize = 8
  ClipReloadTime = 1000
  PreAttackDelay = 0
  AntiGround = Yes
End
"#
            ),
            1
        );
        // Actual Common INI registration, including milliseconds->logic frames.
        INI::new()
            .with_inline_source(
                r#"
DamageFX DamagePromotionOrderRows
  VeterancyMajorFX = REGULAR SMALL_ARMS FX_DamagePromotionOrderRegular
  VeterancyMajorFX = VETERAN SMALL_ARMS FX_DamagePromotionOrderVeteran
  VeterancyAmountForMajorFX = REGULAR SMALL_ARMS 0
  VeterancyAmountForMajorFX = VETERAN SMALL_ARMS 0
  VeterancyThrottleTime = REGULAR SMALL_ARMS 100
  VeterancyThrottleTime = VETERAN SMALL_ARMS 500
End
"#,
                |ini| ini.parse_current_file(),
            )
            .expect("real DamageFX rule parser");
        saved
    }
}
impl Drop for RestoreInputs {
    fn drop(&mut self) {
        let mut fx = get_damage_fx_store_mut().unwrap();
        fx.remove_damage_fx(ROWS);
        if let Some(previous) = self.previous_rows.take() {
            fx.add_damage_fx(ROWS.into(), previous);
        }
        drop(fx);
        if let Some(previous) = self.store.take() {
            gamelogic::weapon::with_weapon_store_mut(|store| *store = previous).unwrap();
        } else {
            gamelogic::weapon::shutdown_weapon_store().unwrap();
        }
        publish_gameworld_authority(self.authority);
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
    }
}

fn eligible_world() -> GameLogic {
    let mut world = GameLogic::new();
    let mut source_player = Player::new(0, Team::USA, "PromotionUSA", true);
    source_player.alliance_team = 0;
    source_player.is_alive = true;
    let mut victim_player = Player::new(1, Team::China, "PromotionChina", false);
    victim_player.alliance_team = 1;
    victim_player.is_alive = true;
    world.add_player(source_player);
    world.add_player(victim_player);
    assert!(world.player_is_playable_side(0) && world.player_is_playable_side(1));
    assert_eq!(
        world.player_relationship(0, 1),
        gamelogic::common::Relationship::Enemies
    );
    world
}

fn authored(
    world: &mut GameLogic,
    name: &str,
    health: u32,
    xp: u32,
    armed: bool,
    team: Team,
    position: Vec3,
) -> ObjectId {
    let weapon = if armed {
        "WeaponSet\n  Conditions = None\n  Weapon = PRIMARY DamagePromotionOrderWeapon\nEnd"
    } else {
        ""
    };
    let text = format!(
        r#"
Object {name}
  KindOf = INFANTRY SELECTABLE ATTACKABLE
  IsTrainable = Yes
  ExperienceRequired = 0 10 100 1000
  ExperienceValue = {xp} {xp} {xp} {xp}
  Body = ActiveBody ModuleTag_Body
    MaxHealth = {health}
  End
  ArmorSet
    Conditions = None
    DamageFX = DamagePromotionOrderRows
  End
  {weapon}
End
"#
    );
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(&text, "damage_promotion_order.ini")
            .unwrap(),
        1
    );
    let definition = parser.get_definition(name).unwrap();
    let template = GameLogic::build_template_from_object_definition(name, definition, None);
    assert_eq!(template.max_health, health as f32);
    assert_eq!(template.veterancy_xp_thresholds, [10.0, 100.0, 1000.0]);
    assert!(template.is_trainable);
    assert_eq!(template.experience_values, [xp as f32; 4]);
    assert_eq!(template.armor_sets.len(), 1);
    assert_eq!(template.armor_sets[0].damage_fx.as_deref(), Some(ROWS));
    world.templates.insert(name.into(), template);
    let player_id = if team == Team::USA { 0 } else { 1 };
    let id = world
        .create_object_for_player(name, player_id, position)
        .expect("actual authored player admission");
    let owner = &world.objects[&id];
    assert_eq!(owner.owner_player_id, Some(player_id));
    assert_eq!(owner.team, team);
    assert_eq!(owner.experience.current, 0.0);
    assert_eq!(owner.experience.level, VeterancyLevel::Rookie);
    assert_eq!(owner.health.current, health as f32);
    assert!(owner.is_trainable());
    if armed {
        assert_eq!(owner.weapon.as_ref().unwrap().damage, 100.0);
        assert_eq!(owner.weapon.as_ref().unwrap().ammo, Some(8));
        assert_eq!(owner.weapon_name_for_slot(0).as_deref(), Some(RULE));
    }
    id
}

fn ordinary_hit(world: &mut GameLogic, source: ObjectId, victim: ObjectId) {
    world.frame = 100;
    assert!(world.objects[&source].is_alive() && world.objects[&victim].is_alive());
    assert_ne!(world.objects[&source].team, world.objects[&victim].team);
    assert_eq!(world.objects[&source].owner_player_id, Some(0));
    assert_eq!(world.objects[&victim].owner_player_id, Some(1));
    assert!(world.score_the_kill_victim_counts(&world.objects[&victim]));
    assert!(!world.objects[&victim].status.under_construction);
    assert!(world.objects[&source].is_within_attack_range_for_slot(0, &world.objects[&victim]));
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .set_target(Some(victim));
    assert_eq!(world.objects[&source].target, Some(victim));
    assert_eq!(world.objects[&source].ai_state, AIState::Attacking);
    assert!(
        take_dispatched_armor_damage_fx().is_empty(),
        "no setup impact FX"
    );
    world.update_combat(&[source], 1.0 / 30.0);
    // Actual accepted normal-shot control, not a direct private damage call.
    let shooter = &world.objects[&source];
    assert_eq!(shooter.weapon.as_ref().unwrap().ammo, Some(7));
    assert_eq!(shooter.weapon_discharge_marker().logic_frame, 100);
    assert_eq!(shooter.weapon_discharge_marker().weapon_slot, 0);
    assert!(shooter.weapon_discharge_marker().sequence > 0);
    let cues = world.take_weapon_discharges_for_presentation();
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].source, source);
    assert_eq!(cues[0].logic_frame, 100);
    let target = &world.objects[&victim];
    assert_eq!(target.last_damage_source, Some(source));
    assert_eq!(target.last_damage_timestamp, Some(100));
    assert_eq!(
        target.last_damage_fx_done,
        Some(crate::game_logic::combat::DamageType::Bullet)
    );
}

fn check_fx(world: &GameLogic, victim: ObjectId, expected: &str, throttle: u32) {
    assert_eq!(
        take_dispatched_armor_damage_fx(),
        [ROWS.to_string(), expected.to_string()],
        "CPP scoreTheKill precedes source-level DamageFX list selection"
    );
    assert_eq!(
        world.objects[&victim].next_damage_fx_time,
        100 + throttle,
        "CPP throttle uses the same then-current source row as its selected list"
    );
}

#[test]
fn normal_eligible_kill_promotion_is_visible_to_damage_fx() {
    isolated(
        "normal_eligible_kill_promotion_is_visible_to_damage_fx",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            let source = authored(
                &mut world,
                "PromotionOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            let victim = authored(
                &mut world,
                "PromotionOrderVictim",
                100,
                10,
                false,
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            );
            ordinary_hit(&mut world, source, victim);
            assert_eq!(world.objects[&victim].health.current, 0.0);
            assert!(world.objects[&victim].kill_experience_awarded);
            assert_eq!(world.objects[&source].experience.current, 10.0);
            assert_eq!(
                world.objects[&source].experience.level,
                VeterancyLevel::Veteran
            );
            check_fx(&world, victim, VETERAN_FX, 15);
        },
    );
}

#[test]
fn normal_eligible_kill_without_promotion_keeps_regular_damage_fx() {
    isolated(
        "normal_eligible_kill_without_promotion_keeps_regular_damage_fx",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            let source = authored(
                &mut world,
                "NoPromotionOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            let victim = authored(
                &mut world,
                "NoPromotionOrderVictim",
                100,
                5,
                false,
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            );
            ordinary_hit(&mut world, source, victim);
            assert_eq!(world.objects[&victim].health.current, 0.0);
            assert_eq!(world.objects[&source].experience.current, 5.0);
            assert_eq!(
                world.objects[&source].experience.level,
                VeterancyLevel::Rookie
            );
            check_fx(&world, victim, REGULAR_FX, 3);
        },
    );
}

#[test]
fn normal_nonlethal_hit_neither_awards_xp_nor_promotes_damage_fx() {
    isolated(
        "normal_nonlethal_hit_neither_awards_xp_nor_promotes_damage_fx",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            let source = authored(
                &mut world,
                "NonlethalOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            let victim = authored(
                &mut world,
                "NonlethalOrderVictim",
                150,
                10,
                false,
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            );
            ordinary_hit(&mut world, source, victim);
            assert_eq!(world.objects[&victim].health.current, 50.0);
            assert!(!world.objects[&victim].kill_experience_awarded);
            assert_eq!(world.objects[&source].experience.current, 0.0);
            assert_eq!(
                world.objects[&source].experience.level,
                VeterancyLevel::Rookie
            );
            check_fx(&world, victim, REGULAR_FX, 3);
        },
    );
}

#[test]
fn normal_kill_sink_promotion_does_not_change_actual_source_fx_row() {
    isolated(
        "normal_kill_sink_promotion_does_not_change_actual_source_fx_row",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            let source = authored(
                &mut world,
                "SinkOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            let victim = authored(
                &mut world,
                "SinkOrderVictim",
                100,
                10,
                false,
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            );
            let sink = authored(
                &mut world,
                "SinkOrderRecipient",
                100,
                0,
                false,
                Team::USA,
                Vec3::new(-100.0, 0.0, 0.0),
            );
            world
                .objects
                .get_mut(&source)
                .unwrap()
                .set_experience_sink(Some(sink));
            assert_eq!(world.objects[&source].experience_sink, Some(sink));
            ordinary_hit(&mut world, source, victim);
            assert_eq!(world.objects[&victim].health.current, 0.0);
            assert_eq!(world.objects[&source].experience.current, 0.0);
            assert_eq!(
                world.objects[&source].experience.level,
                VeterancyLevel::Rookie
            );
            assert_eq!(world.objects[&sink].experience.current, 10.0);
            assert_eq!(
                world.objects[&sink].experience.level,
                VeterancyLevel::Veteran
            );
            check_fx(&world, victim, REGULAR_FX, 3);
        },
    );
}

// Same original ActiveBody scoreTheKill -> onDie -> DamageFX ordering applies
// to a no-Object weapon target. Main's nearest-impact victim selector remains
// its existing host residual; this is the real ground driver, not a direct hit.
#[test]
fn normal_ground_kill_promotion_is_visible_to_damage_fx() {
    isolated(
        "normal_ground_kill_promotion_is_visible_to_damage_fx",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            let source = authored(
                &mut world,
                "GroundPromotionOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            let victim = authored(
                &mut world,
                "GroundPromotionOrderVictim",
                100,
                10,
                false,
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            );
            world.frame = 100;
            let impact = world.objects[&victim].get_position();
            {
                let shooter = world.objects.get_mut(&source).unwrap();
                shooter.set_target_location(Some(impact));
                shooter.set_force_attack(true);
                assert_eq!(shooter.target, None);
                assert_eq!(shooter.target_location, Some(impact));
                assert_eq!(shooter.ai_state, AIState::Attacking);
                assert!(shooter.force_attack);
                assert!(shooter.is_within_attack_range_pos_for_slot(0, impact));
            }
            assert!(world.score_the_kill_victim_counts(&world.objects[&victim]));
            assert_eq!(world.objects[&source].owner_player_id, Some(0));
            assert_eq!(world.objects[&victim].owner_player_id, Some(1));
            assert!(
                take_dispatched_armor_damage_fx().is_empty(),
                "no setup impact FX"
            );
            world.update_combat(&[source], 1.0 / 30.0);
            let shooter = &world.objects[&source];
            assert_eq!(shooter.weapon.as_ref().unwrap().ammo, Some(7));
            assert_eq!(shooter.weapon_discharge_marker().logic_frame, 100);
            assert_eq!(shooter.weapon_discharge_marker().weapon_slot, 0);
            assert!(shooter.weapon_discharge_marker().sequence > 0);
            assert_eq!(shooter.experience.current, 10.0);
            assert_eq!(shooter.experience.level, VeterancyLevel::Veteran);
            let cues = world.take_weapon_discharges_for_presentation();
            assert_eq!(cues.len(), 1);
            assert_eq!(cues[0].source, source);
            assert_eq!(cues[0].logic_frame, 100);
            let target = &world.objects[&victim];
            assert_eq!(target.health.current, 0.0);
            assert!(target.kill_experience_awarded);
            assert_eq!(target.last_damage_source, Some(source));
            assert_eq!(target.last_damage_timestamp, Some(100));
            assert_eq!(
                target.last_damage_fx_done,
                Some(crate::game_logic::combat::DamageType::Bullet)
            );
            check_fx(&world, victim, VETERAN_FX, 15);
        },
    );
}

// CPP Object.cpp2914-2916 denies kill XP for ALLIES even when force fire
// physically hits them. A projected-veterancy shortcut must not promote FX.
#[test]
fn normal_forced_allied_kill_does_not_promote_damage_fx() {
    isolated(
        "normal_forced_allied_kill_does_not_promote_damage_fx",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            world.players.get_mut(&1).unwrap().alliance_team = 0;
            assert_eq!(
                world.player_relationship(0, 1),
                gamelogic::common::Relationship::Allies
            );
            let source = authored(
                &mut world,
                "AlliedPromotionOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            let victim = authored(
                &mut world,
                "AlliedPromotionOrderVictim",
                100,
                10,
                false,
                Team::China,
                Vec3::new(30.0, 0.0, 0.0),
            );
            world
                .objects
                .get_mut(&source)
                .unwrap()
                .set_force_attack(true);
            assert!(world.objects[&source].force_attack);
            ordinary_hit(&mut world, source, victim);
            assert_eq!(world.objects[&victim].health.current, 0.0);
            assert!(world.objects[&victim].kill_experience_awarded);
            assert_eq!(world.objects[&source].experience.current, 0.0);
            assert_eq!(
                world.objects[&source].experience.level,
                VeterancyLevel::Rookie
            );
            check_fx(&world, victim, REGULAR_FX, 3);
        },
    );
}

// CPP Weapon.cpp2703 point fire shares privateFireWeapon with object fire;
// 2617-2623 consumes one clip round and advances one barrel per accepted shot.
// An empty impact point cannot gain XP, so this isolates discharge accounting
// from the promotion/typed damage continuation packet.
#[test]
fn normal_ground_without_victim_commits_exactly_one_discharge() {
    isolated(
        "normal_ground_without_victim_commits_exactly_one_discharge",
        || {
            let _serial = combat_test_guard();
            let _restore = RestoreInputs::install();
            let mut world = eligible_world();
            let source = authored(
                &mut world,
                "EmptyGroundOrderShooter",
                100,
                0,
                true,
                Team::USA,
                Vec3::ZERO,
            );
            world.frame = 100;
            let impact = Vec3::new(30.0, 0.0, 0.0);
            let expected_sequence = world.weapon_discharge_next_sequence_for_snapshot();
            {
                let shooter = world.objects.get_mut(&source).unwrap();
                shooter.set_target_location(Some(impact));
                shooter.set_force_attack(true);
                assert!(shooter.is_within_attack_range_pos_for_slot(0, impact));
                assert_eq!(shooter.weapon.as_ref().unwrap().ammo, Some(8));
            }
            assert_eq!(world.objects.len(), 1, "actual ground impact has no victim");
            assert!(take_dispatched_armor_damage_fx().is_empty());
            world.update_combat(&[source], 1.0 / 30.0);
            let shooter = &world.objects[&source];
            assert_eq!(
                shooter.weapon.as_ref().unwrap().ammo,
                Some(7),
                "one accepted position-target shot consumes one round"
            );
            assert!(
                (shooter.weapon.as_ref().unwrap().last_fire_time - 100.0 / 30.0).abs() < 1.0e-5
            );
            assert_eq!(
                shooter.weapon_discharge_marker().sequence,
                expected_sequence
            );
            assert_eq!(shooter.weapon_discharge_marker().logic_frame, 100);
            assert_eq!(shooter.weapon_discharge_marker().weapon_slot, 0);
            assert_eq!(shooter.experience.current, 0.0);
            assert_eq!(shooter.experience.level, VeterancyLevel::Rookie);
            assert_eq!(shooter.health.current, 100.0);
            assert_eq!(
                world.weapon_discharge_next_sequence_for_snapshot(),
                expected_sequence + 1
            );
            let cues = world.take_weapon_discharges_for_presentation();
            assert_eq!(
                cues.len(),
                1,
                "no duplicate position-target cue/barrel commit"
            );
            assert_eq!(cues[0].source, source);
            assert_eq!(cues[0].sequence, expected_sequence);
            assert_eq!(cues[0].logic_frame, 100);
            assert!(
                take_dispatched_armor_damage_fx().is_empty(),
                "no invented empty-ground damage FX"
            );
        },
    );
}

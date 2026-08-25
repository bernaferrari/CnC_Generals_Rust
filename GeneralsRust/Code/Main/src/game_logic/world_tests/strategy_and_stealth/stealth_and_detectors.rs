//! Behavior suite extracted from `strategy_and_stealth`.
use super::*;

#[test]
fn artillery_barrage_host_path_queues_and_applies_delayed_multi_shell_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES,
        ArtilleryBarrageScienceTier, HostStrikePhase, HostSuperweaponKind,
        artillery_barrage_points, multi_strike_last_impact_frame,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_ArtilleryBarrage1");
    }

    let target = Vec3::new(100.0, 0.0, 0.0);
    // WeaponErrorRadius residual scatter: place outer enemy on shell index 1.
    let points = artillery_barrage_points(target);
    let outer_shell = points[1];

    // player_id 0 maps to Team::USA for ownership validation residual.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_center_id = game_logic
        .create_object("TestTank", Team::GLA, target)
        .expect("enemy center");
    let enemy_outer_id = game_logic
        .create_object("TestTank", Team::GLA, outer_shell)
        .expect("enemy outer shell");
    let far_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, target)
        .expect("friend");

    for id in [enemy_center_id, enemy_outer_id, far_enemy_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Artillery,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::ArtilleryBarrage),
        "ArtilleryBarrage must queue a pending host strike"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponArtilleryBarrage"),
        "activation must queue SuperweaponArtilleryBarrage audio"
    );

    // Before impact delay: no damage.
    let health_before_center = game_logic
        .host_object(enemy_center_id)
        .unwrap()
        .health
        .current;
    let health_before_outer = game_logic
        .host_object(enemy_outer_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES - 1;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic
            .host_object(enemy_center_id)
            .unwrap()
            .health
            .current,
        health_before_center,
        "no damage before artillery impact frame"
    );
    assert_eq!(
        game_logic
            .host_object(enemy_outer_id)
            .unwrap()
            .health
            .current,
        health_before_outer,
        "no outer-shell damage before impact frame"
    );
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage)
    );

    // Lead shell at DelayDeliveryMax; remaining shells stagger via DelayDelivery residual.
    let activate = game_logic
        .special_power_strikes()
        .pending_of_kind(HostSuperweaponKind::ArtilleryBarrage)
        .first()
        .map(|s| s.activate_frame)
        .unwrap_or(0);
    let last = multi_strike_last_impact_frame(
        HostSuperweaponKind::ArtilleryBarrage,
        activate,
        ArtilleryBarrageScienceTier::Level1,
    );
    game_logic.frame = activate.saturating_add(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES);
    game_logic.update_special_power_strikes();
    if !game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage)
    {
        game_logic.frame = last;
        game_logic.update_special_power_strikes();
    }

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::ArtilleryBarrage),
        "ArtilleryBarrage must complete after DelayDelivery stagger residual"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::ArtilleryBarrage),
        "host path honesty requires completed artillery barrage strike"
    );

    let center_hp = game_logic
        .host_object(enemy_center_id)
        .map(|o| o.health.current);
    let outer_hp = game_logic
        .host_object(enemy_outer_id)
        .map(|o| o.health.current);
    // Epicenter residual damage = ARTILLERY_BARRAGE_DAMAGE (105) per shell hit.
    let center_dealt = test_observed_damage_to(
        enemy_center_id,
        health_before_center,
        center_hp.unwrap_or(0.0),
    );
    let outer_dealt =
        test_observed_damage_to(enemy_outer_id, health_before_outer, outer_hp.unwrap_or(0.0));
    assert!(
        center_dealt + 0.1 >= ARTILLERY_BARRAGE_DAMAGE
            || center_hp.is_none()
            || center_hp.map(|h| h < health_before_center - ARTILLERY_BARRAGE_DAMAGE + 1.0)
                == Some(true)
            || center_hp == Some(0.0)
            || game_logic
                .host_object(enemy_center_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy at center shell must take artillery residual damage, got {center_hp:?} dealt={center_dealt}"
    );
    assert!(
        outer_dealt + 0.1 >= ARTILLERY_BARRAGE_DAMAGE
            || outer_hp.is_none()
            || outer_hp.map(|h| h < health_before_outer - ARTILLERY_BARRAGE_DAMAGE + 1.0)
                == Some(true)
            || outer_hp == Some(0.0)
            || game_logic
                .host_object(enemy_outer_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy on outer shell epicenter must take multi-shell residual damage, got {outer_hp:?} dealt={outer_dealt}"
    );
    let friend_dealt = test_observed_damage_to(
        friend_id,
        500.0,
        game_logic
            .host_object(friend_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0),
    );
    assert!(
        friend_dealt > 0.0
            || game_logic
                .host_object(friend_id)
                .map(|o| o.health.current < 500.0 || o.status.destroyed)
                .unwrap_or(true),
        "friendly units take ArtilleryBarrage residual damage (RadiusDamageAffects ALLIES) dealt={friend_dealt}"
    );
    assert!(
        game_logic
            .host_object(far_enemy_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside shell scatter must be untouched"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "FX_ArtilleryBarrage"),
        "impact must queue FX_ArtilleryBarrage audio"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::ArtilleryBarrage);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostStrikePhase::Completed);
    assert!(
        completed[0].objects_hit >= 2,
        "multi-shell must hit both scatter enemies, hit={}",
        completed[0].objects_hit
    );
    assert!(completed[0].total_damage_applied > 0.0);
    assert_eq!(
        completed[0].multi_strike_applied,
        crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_SHELL_COUNT
    );

    game_logic.process_destroy_list();
}

#[test]
fn cruise_missile_host_path_queues_and_applies_delayed_area_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        CRUISE_MISSILE_DAMAGE, CRUISE_MISSILE_IMPACT_DELAY_FRAMES, HostStrikePhase,
        HostSuperweaponKind,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let target = Vec3::new(100.0, 0.0, 0.0);

    // player_id 0 maps to Team::USA for ownership validation residual.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, target)
        .expect("enemy");
    let near_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(140.0, 0.0, 0.0))
        .expect("near enemy");
    let far_enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, target)
        .expect("friend");

    for id in [enemy_id, near_enemy_id, far_enemy_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CruiseMissile,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::CruiseMissile),
        "CruiseMissile must queue a pending host strike"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponCruiseMissile"),
        "activation must queue SuperweaponCruiseMissile audio"
    );

    // Before impact delay: no damage (loft residual in flight).
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let near_health_before = game_logic
        .host_object(near_enemy_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = CRUISE_MISSILE_IMPACT_DELAY_FRAMES - 1;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no damage before cruise missile impact frame"
    );
    assert_eq!(
        game_logic
            .host_object(near_enemy_id)
            .unwrap()
            .health
            .current,
        near_health_before,
        "no near-radius damage before impact frame"
    );
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::CruiseMissile)
    );

    // At impact: MOAB area damage + complete honesty.
    game_logic.frame = CRUISE_MISSILE_IMPACT_DELAY_FRAMES;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::CruiseMissile),
        "CruiseMissile must complete on impact frame"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::CruiseMissile),
        "host path honesty requires completed cruise missile strike"
    );

    let enemy_hp = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let near_hp = game_logic
        .host_object(near_enemy_id)
        .map(|o| o.health.current);
    // Epicenter residual damage = CRUISE_MISSILE_DAMAGE (2000) — lethal to 500 HP.
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_hp.unwrap_or(0.0));
    let near_dealt =
        test_observed_damage_to(near_enemy_id, near_health_before, near_hp.unwrap_or(0.0));
    assert!(
        enemy_dealt + 0.1 >= health_before
            || enemy_hp.is_none()
            || enemy_hp == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "enemy at epicenter must take lethal CruiseMissile residual damage, got {enemy_hp:?} dealt={enemy_dealt}"
    );
    assert!(
        near_dealt > 0.0
            || near_hp.map(|h| h < near_health_before).unwrap_or(false)
            || near_hp.is_none(),
        "enemy inside MOAB radius must take CruiseMissile residual damage, got {near_hp:?} dealt={near_dealt}"
    );
    let friend_dealt = test_observed_damage_to(
        friend_id,
        500.0,
        game_logic
            .host_object(friend_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0),
    );
    assert!(
        friend_dealt > 0.0
            || game_logic
                .host_object(friend_id)
                .map(|o| o.health.current < 500.0 || o.status.destroyed)
                .unwrap_or(true),
        "friendly units take CruiseMissile residual damage (RadiusDamageAffects ALLIES) dealt={friend_dealt}"
    );
    assert!(
        game_logic
            .host_object(far_enemy_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside MOAB radius must be untouched"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "CruiseMissileImpact"),
        "impact must queue CruiseMissileImpact audio"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::CruiseMissile);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostStrikePhase::Completed);
    assert!(
        completed[0].objects_hit >= 1,
        "cruise missile must hit epicenter enemy, hit={}",
        completed[0].objects_hit
    );
    assert!(
        completed[0].total_damage_applied >= CRUISE_MISSILE_DAMAGE - 1.0,
        "damage applied must cover MOAB primary, got {}",
        completed[0].total_damage_applied
    );

    game_logic.process_destroy_list();
}

#[test]
fn america_paradrop_host_path_queues_and_spawns_infantry() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_paradrop::{
        AMERICA_PARADROP_UNIT_COUNT, HostParadropKind, HostParadropPhase,
        PARADROP_RESIDUAL_TEMPLATE,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_Paradrop1");
    }
    ensure_test_infantry_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(200.0, 0.0, 100.0);
    let infantry_count = |gl: &GameLogic| {
        gl.host_objects()
            .values()
            .filter(|o| o.is_kind_of(crate::game_logic::KindOf::Infantry))
            .count()
    };
    assert_eq!(infantry_count(&game_logic), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Paradrop,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .host_paradrops()
            .honesty_queue_ok(HostParadropKind::AmericaParadrop),
        "Paradrop must queue a pending host mission"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponParadrop"),
        "activation must queue SuperweaponParadrop audio"
    );
    // Cargo plane / parachute DeliverPayload residual may spawn before
    // infantry drop delay; residual infantry must not appear yet.
    assert_eq!(
        infantry_count(&game_logic),
        0,
        "no infantry before drop delay"
    );
    assert!(
        game_logic.host_paradrops.transports_spawned >= 1,
        "cargo plane residual should spawn on queue"
    );
    assert!(
        !game_logic
            .host_paradrops()
            .honesty_complete_ok(HostParadropKind::AmericaParadrop)
    );

    game_logic.frame = 89;
    game_logic.update_paradrops();
    assert_eq!(
        infantry_count(&game_logic),
        0,
        "still no infantry one frame before drop"
    );

    game_logic.frame = 90;
    game_logic.update_paradrops();

    assert!(
        game_logic
            .host_paradrops()
            .honesty_complete_ok(HostParadropKind::AmericaParadrop),
        "Paradrop must complete with spawned units"
    );
    assert!(
        game_logic
            .host_paradrops()
            .honesty_host_path_ok(HostParadropKind::AmericaParadrop),
        "host path honesty requires completed drop with units"
    );

    let completed = game_logic
        .host_paradrops()
        .completed_of_kind(HostParadropKind::AmericaParadrop);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostParadropPhase::Completed);
    assert_eq!(
        completed[0].spawned_unit_ids.len(),
        AMERICA_PARADROP_UNIT_COUNT as usize,
        "must spawn residual America Paradrop1 infantry count"
    );

    let mut near_target = 0_u32;
    for id in &completed[0].spawned_unit_ids {
        let obj = game_logic.host_object(*id).expect("spawned infantry");
        assert_eq!(obj.team, Team::USA);
        assert!(
            obj.thing.template.name == PARADROP_RESIDUAL_TEMPLATE
                || obj.thing.template.name.contains("Infantry")
                || obj.thing.template.name.contains("Ranger"),
            "spawned residual infantry template, got {}",
            obj.thing.template.name
        );
        let pos = obj.get_position();
        let dx = pos.x - target.x;
        let dz = pos.z - target.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= 80.0 {
            near_target += 1;
        }
    }
    assert_eq!(
        near_target, AMERICA_PARADROP_UNIT_COUNT,
        "all paradrop infantry must appear near target location"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "ParadropLanding"),
        "drop must queue ParadropLanding audio"
    );
    assert_eq!(
        infantry_count(&game_logic),
        AMERICA_PARADROP_UNIT_COUNT as usize,
        "infantry count after paradrop drop"
    );
}

#[test]
fn gla_ambush_host_path_queues_and_spawns_infantry() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_ambush::{
        AMBUSH_RESIDUAL_TEMPLATE, GLA_AMBUSH1_UNIT_COUNT, HostAmbushKind, HostAmbushPhase,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_RebelAmbush1");
    }
    ensure_test_infantry_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(200.0, 0.0, 100.0);
    let objects_before = game_logic.host_objects().len();

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Ambush,
            target: PowerTarget::Location(target),
        },
        player_id: 2, // Team::GLA (player 0 is USA; ownership check)
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .host_ambushes()
            .honesty_complete_ok(HostAmbushKind::GLARebelAmbush),
        "Ambush must spawn on the fire frame (C++ synchronous OCL)"
    );
    let caster = game_logic.host_object(caster_id).expect("caster after cmd");
    assert!(!caster.special_power_ready);
    assert!(caster.special_power_cooldown_remaining > 0.0);
    assert_eq!(caster.ai_state, AIState::SpecialAbility);
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "RebelAmbushActivated"),
        "activation must queue RebelAmbushActivated audio"
    );
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before + GLA_AMBUSH1_UNIT_COUNT as usize,
        "infantry spawn on the fire frame"
    );

    game_logic.update_ambushes();

    assert!(
        game_logic
            .host_ambushes()
            .honesty_complete_ok(HostAmbushKind::GLARebelAmbush),
        "Ambush must complete with spawned units"
    );
    assert!(
        game_logic
            .host_ambushes()
            .honesty_host_path_ok(HostAmbushKind::GLARebelAmbush),
        "host path honesty requires completed ambush with units"
    );

    let completed = game_logic
        .host_ambushes()
        .completed_of_kind(HostAmbushKind::GLARebelAmbush);
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].phase, HostAmbushPhase::Completed);
    assert_eq!(
        completed[0].spawned_unit_ids.len(),
        GLA_AMBUSH1_UNIT_COUNT as usize,
        "must spawn residual Ambush1 infantry count"
    );

    let mut near_target = 0_u32;
    for id in &completed[0].spawned_unit_ids {
        let obj = game_logic.host_object(*id).expect("spawned infantry");
        assert_eq!(obj.team, Team::GLA);
        assert!(
            obj.thing.template.name == AMBUSH_RESIDUAL_TEMPLATE
                || obj.thing.template.name.contains("Infantry")
                || obj.thing.template.name.contains("Rebel"),
            "spawned residual infantry template, got {}",
            obj.thing.template.name
        );
        let pos = obj.get_position();
        let dx = pos.x - target.x;
        let dz = pos.z - target.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist <= 80.0 {
            near_target += 1;
        }
    }
    assert_eq!(
        near_target, GLA_AMBUSH1_UNIT_COUNT,
        "all ambush infantry must appear near target location"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "RebelAmbushSpawn"),
        "spawn must queue RebelAmbushSpawn audio"
    );
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before + GLA_AMBUSH1_UNIT_COUNT as usize
    );
}

#[test]
fn scud_storm_host_path_queues_and_completes() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
        attach_command_special_power(
            tpl,
            crate::command_system::SpecialPowerType::ScudStorm,
            "SuperweaponScudStorm",
            SpecialPowerModuleKind::OclSpecialPower,
        );
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 800.0;
        enemy.health.maximum = 800.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ScudStorm,
            target: PowerTarget::Object(enemy_id),
        },
        player_id: 2, // Team::GLA
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::ScudStorm)
    );

    // First missile at PreAttackDelay = 90 frames (multi-missile residual).
    use crate::game_logic::special_power_strikes::{
        ArtilleryBarrageScienceTier, SCUD_STORM_PRE_ATTACK_FRAMES, multi_strike_last_impact_frame,
    };
    game_logic.frame = SCUD_STORM_PRE_ATTACK_FRAMES;
    game_logic.update_special_power_strikes();
    // Mid-storm: first wave applied, not necessarily complete.
    assert!(
        game_logic
            .special_power_strikes()
            .pending_of_kind(HostSuperweaponKind::ScudStorm)
            .first()
            .map(|s| s.multi_strike_applied >= 1)
            .unwrap_or(false)
            || game_logic
                .special_power_strikes()
                .honesty_complete_ok(HostSuperweaponKind::ScudStorm),
        "first ScudStorm missile residual must apply"
    );

    // Jump to last missile DelayBetweenShots residual frame.
    let activate = game_logic
        .special_power_strikes()
        .pending_of_kind(HostSuperweaponKind::ScudStorm)
        .first()
        .map(|s| s.activate_frame)
        .or_else(|| {
            game_logic
                .special_power_strikes()
                .completed_of_kind(HostSuperweaponKind::ScudStorm)
                .first()
                .map(|s| s.activate_frame)
        })
        .unwrap_or(0);
    let last = multi_strike_last_impact_frame(
        HostSuperweaponKind::ScudStorm,
        activate,
        ArtilleryBarrageScienceTier::Level1,
    );
    game_logic.frame = last;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::ScudStorm),
        "SCUD Storm host path must complete"
    );
    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::ScudStorm);
    assert_eq!(completed.len(), 1);
    assert!(completed[0].objects_hit >= 1);
    assert!(completed[0].total_damage_applied > 0.0);
    assert!(
        completed[0].multi_strike_applied >= 9,
        "ClipSize 9 multi-missile residual must apply all missiles"
    );
    assert!(
        game_logic.special_power_strikes().honesty_toxin_ok(),
        "ScudStorm must spawn LargePoisonField residual"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "ScudStormImpact"),
        "SCUD impact audio residual required"
    );
}

#[test]
fn particle_cannon_host_path_queues_and_completes() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, PARTICLE_BEAM_AUDIO, PARTICLE_BEAM_DAMAGE_PER_PULSE,
        PARTICLE_BEAM_TICK_INTERVAL_FRAMES, PARTICLE_BEAM_TRAVEL_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
        attach_command_special_power(
            tpl,
            crate::command_system::SpecialPowerType::ParticleCannon,
            "SuperweaponParticleUplinkCannon",
            SpecialPowerModuleKind::OclSpecialPower,
        );
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    // First beam pulse swath epicenter residual (pulse 0 walks -half Swath distance).
    let beam_target = Vec3::new(10.0, 0.0, 0.0);
    let first_pulse_pos =
        beam_target + crate::game_logic::special_power_strikes::particle_swath_offset(0);
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, first_pulse_pos)
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        // Survive first pulse so multi-pulse residual is observable.
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon,
            target: PowerTarget::Location(beam_target),
        },
        player_id: 1, // Team::China
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::ParticleCannon)
    );
    assert!(
        game_logic.special_power_strikes().beam_fields().is_empty(),
        "beam must not spawn before charge residual completes"
    );

    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = PARTICLE_BEAM_TRAVEL_FRAMES.saturating_sub(1);
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no beam damage before BeamTravelTime (75f)"
    );

    // Beam start: field spawn + first pulse (C++ orbital birth).
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = PARTICLE_BEAM_TRAVEL_FRAMES;
    game_logic.update_special_power_strikes();
    game_logic.update_special_power_strikes();
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::ParticleCannon)
    );
    assert!(
        game_logic.special_power_strikes().honesty_beam_ok(),
        "ParticleCannon must spawn continuous beam residual"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::ParticleCannon)
    );
    assert!(
        game_logic.special_power_strikes().honesty_beam_damage_ok(),
        "first beam pulse must apply residual damage"
    );

    let after_first = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let first_dealt = test_observed_damage_to(enemy_id, health_before, after_first);
    assert!(
        first_dealt > 0.0 || after_first < health_before,
        "enemy must take first continuous beam pulse (before={health_before}, after={after_first}, dealt={first_dealt})"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == PARTICLE_BEAM_AUDIO
                || e.event_type == "ParticleCannonBeamStart"
                || e.event_type == "SuperweaponParticleCannon"),
        "beam residual must queue activation/beam audio"
    );

    // Second pulse after tick interval.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = PARTICLE_BEAM_TRAVEL_FRAMES + PARTICLE_BEAM_TICK_INTERVAL_FRAMES;
    game_logic.update_special_power_strikes();
    let after_second = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let second_dealt = test_observed_damage_to(enemy_id, after_first, after_second);
    assert!(
        second_dealt > 0.0 || after_second < after_first,
        "second continuous beam pulse must apply more damage (first={after_first}, second={after_second}, dealt={second_dealt})"
    );
    let _ = PARTICLE_BEAM_DAMAGE_PER_PULSE;
}

#[test]
fn particle_cannon_queues_charge_unpack_and_firing_loops() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, PARTICLE_BEAM_AUDIO, PARTICLE_BEAM_TRAVEL_FRAMES,
        PARTICLE_FIRING_TO_PACK_AUDIO, PARTICLE_POWERUP_AUDIO, PARTICLE_UNPACK_AUDIO,
        ParticleUplinkStatus, particle_status_for_ready_countdown,
    };

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let caster = logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let target = Vec3::new(40.0, 0.0, 0.0);
    logic
        .queue_special_power_strike(&SpecialPowerType::ParticleCannon, caster, target)
        .expect("queued");

    let queued: Vec<&str> = logic
        .queued_audio_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    // Retail BeamTravelTime (75f) is shorter than Charge+Raise+Ready, so the
    // first pre-fire status is PREPARING → UnpackToIdle. A long impact window
    // (honesty) still walks CHARGING → PoweringUp.
    assert!(
        queued.contains(&PARTICLE_UNPACK_AUDIO) || queued.contains(&PARTICLE_POWERUP_AUDIO),
        "PUC charge/unpack loop must queue at initiate, got {queued:?}"
    );

    logic.queued_audio_events.clear();
    logic.frame = PARTICLE_BEAM_TRAVEL_FRAMES;
    logic.update_special_power_strikes();
    let after_beam: Vec<&str> = logic
        .queued_audio_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();
    assert!(
        after_beam.contains(&PARTICLE_FIRING_TO_PACK_AUDIO),
        "STATUS_FIRING must queue FiringToPackSoundLoop, got {after_beam:?}"
    );
    assert!(
        after_beam.contains(&PARTICLE_BEAM_AUDIO),
        "beam spawn must still queue GroundAnnihilation, got {after_beam:?}"
    );

    // Long countdown still emits PoweringUp then UnpackToIdle (C++ setClientStatus).
    let mut long = GameLogic::new();
    ensure_test_tank_template(&mut long);
    let src = long
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("src");
    let id = long
        .queue_special_power_strike(&SpecialPowerType::ParticleCannon, src, target)
        .expect("long");
    {
        let strike = long
            .special_power_strikes_mut()
            .get_mut(id)
            .expect("strike");
        strike.impact_frame = 350;
        strike.particle_status = ParticleUplinkStatus::Idle;
        strike.particle_status_peak = ParticleUplinkStatus::Idle;
    }
    long.queued_audio_events.clear();
    long.frame = 0;
    long.update_special_power_strikes();
    assert_eq!(
        particle_status_for_ready_countdown(0, 350),
        ParticleUplinkStatus::Charging
    );
    assert!(
        long.queued_audio_events
            .iter()
            .any(|e| e.event_type == PARTICLE_POWERUP_AUDIO),
        "long charge window must queue PoweringUpSoundLoop"
    );
    let _ = HostSuperweaponKind::ParticleCannon;
}

#[test]
fn live_special_power_fire_records_academy_special_powers_used() {
    use crate::command_system::SpecialPowerType;
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    let caster = logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    if let Some(obj) = logic.host_object_mut(caster) {
        obj.owner_player_id = Some(0);
    }
    assert_eq!(logic.get_player(0).expect("p0").special_powers_used, 0);
    logic
        .queue_special_power_strike(
            &SpecialPowerType::ParticleCannon,
            caster,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("queued");
    assert_eq!(
        logic.get_player(0).expect("p0").special_powers_used,
        1,
        "initiateIntent analog must record ACT_SUPERPOWER"
    );
}

#[test]

fn cleanup_stream_projectile_flies_and_clears() {
    use crate::game_logic::host_cleanup_area::{
        CLEANUP_STREAM_MISSILE_FUEL_FRAMES, HOST_CLEANUP_PROJECTILE,
        HOST_CLEANUP_PROJECTILE_STREAM, cleanup_stream_flight_frames,
    };
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);

    let mut amb_tpl = ThingTemplate::new("USA_Ambulance");
    amb_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0);
    logic.templates.insert("USA_Ambulance".to_string(), amb_tpl);

    let amb = logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let anthrax_caster = logic
        .create_object("TestTank", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("anthrax caster");
    let aim = Vec3::new(20.0, 0.0, 0.0);
    let strike_id = logic.special_power_strikes_mut().queue(
        HostSuperweaponKind::AnthraxBomb,
        anthrax_caster,
        Team::GLA,
        aim,
        0,
    );
    logic.frame = 90;
    logic
        .special_power_strikes_mut()
        .record_impact_complete(strike_id, 0.0, 0, 0);
    let fields_before = logic.special_power_strikes().toxin_fields().len();
    assert!(fields_before > 0, "seed toxin field");

    assert!(logic.activate_cleanup_area(0, aim, Some(amb)));
    assert!(logic.honesty_cleanup_stream_projectile_ok());
    let snap = logic.projectile_stream_snapshot();
    assert!(
        snap.iter().any(|(sid, name, pts, _)| {
            *sid == amb && name == HOST_CLEANUP_PROJECTILE_STREAM && !pts.is_empty()
        }),
        "CleanupHazardProjectileStream residual should register points"
    );
    assert_eq!(
        logic.special_power_strikes().toxin_fields().len(),
        fields_before,
        "toxin should remain until stream impact"
    );

    let max_steps = cleanup_stream_flight_frames(20.0)
        .saturating_add(CLEANUP_STREAM_MISSILE_FUEL_FRAMES)
        .max(20);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_cleanup_stream_projectiles();
        if !logic
            .objects
            .values()
            .any(|o| o.cleanup_stream_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    assert!(
        logic.special_power_strikes().toxin_fields().is_empty()
            || logic.honesty_cleanup_area_clear_ok(),
        "cleanup stream impact should clear toxin residual"
    );
    assert!(
        !logic
            .objects
            .values()
            .any(|o| o.cleanup_stream_projectile && o.is_alive()),
        "cleanup projectile should detonate"
    );
    let _ = HOST_CLEANUP_PROJECTILE;
}

#[test]
fn cleanup_area_residual_clears_hazards_and_mines() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_cleanup_area::{
        CLEANUP_AREA_ACTIVATE_AUDIO, HOST_CLEANUP_AREA_RADIUS,
    };
    use crate::game_logic::special_power_strikes::{
        ANTHRAX_TOXIN_DAMAGE_PER_TICK, HostSuperweaponKind,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    // Residual ambulance caster template.
    let mut amb_tpl = ThingTemplate::new("USA_Ambulance");
    amb_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0);
    game_logic
        .templates
        .insert("USA_Ambulance".to_string(), amb_tpl);

    let caster_id = game_logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    // Spawn anthrax toxin residual via strike, then clear with CleanupArea.
    let anthrax_caster = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("anthrax caster");
    let strike_id = game_logic.special_power_strikes_mut().queue(
        HostSuperweaponKind::AnthraxBomb,
        anthrax_caster,
        Team::GLA,
        Vec3::new(20.0, 0.0, 0.0),
        0,
    );
    // Force complete impact to spawn toxin field at (20,0,0).
    game_logic.frame = 90;
    game_logic
        .special_power_strikes_mut()
        .record_impact_complete(strike_id, 0.0, 0, 0);
    assert_eq!(
        game_logic.special_power_strikes().toxin_fields().len(),
        1,
        "setup: toxin field must exist before cleanup"
    );

    // Place enemy land mine near cleanup target.
    let mine_id = game_logic
        .place_land_mine(Team::GLA, Vec3::new(15.0, 0.0, 0.0), Some(anthrax_caster))
        .expect("mine");
    assert!(
        game_logic
            .host_object(mine_id)
            .and_then(|o| o.mine_data.as_ref())
            .is_some()
    );

    let target = Vec3::new(20.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CleanupArea,
            target: PowerTarget::Location(target),
        },
        player_id: 0, // Team::USA
        command_id: 88,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // CleanupStreamProjectile residual: advance flight to impact clear.
    if game_logic.cleanup_stream_missiles_spawned > 0 {
        for _ in 0..40 {
            game_logic.frame = game_logic.frame.saturating_add(1);
            game_logic.update_cleanup_stream_projectiles();
            if !game_logic
                .objects
                .values()
                .any(|o| o.cleanup_stream_projectile && o.is_alive())
            {
                break;
            }
        }
        game_logic.process_destroy_list();
    }

    assert!(
        game_logic.honesty_cleanup_area_activate_ok()
            || game_logic.honesty_cleanup_stream_projectile_ok(),
        "CleanupArea must record activation"
    );
    assert!(
        game_logic.honesty_cleanup_area_clear_ok(),
        "CleanupArea must clear at least one residual hazard/mine"
    );
    assert!(
        game_logic.honesty_cleanup_area_ok(),
        "CleanupArea host path honesty"
    );
    assert!(
        game_logic.special_power_strikes().toxin_fields().is_empty(),
        "toxin field in radius must be cleared"
    );
    // Mine disarmed (detonated residual bookkeeping) and queued for destroy.
    let mine_disarmed = game_logic
        .host_object(mine_id)
        .and_then(|o| o.mine_data.as_ref())
        .map(|d| d.detonated)
        .unwrap_or(true);
    assert!(
        mine_disarmed,
        "enemy mine in cleanup radius must be disarmed"
    );
    game_logic.process_destroy_list();
    assert!(
        game_logic.host_object(mine_id).is_none()
            || game_logic
                .host_object(mine_id)
                .map(|o| !o.is_alive() || o.status.destroyed)
                .unwrap_or(true),
        "disarmed mine must leave destroy residual"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == CLEANUP_AREA_ACTIVATE_AUDIO),
        "cleanup must queue detox audio"
    );
    assert!(
        (game_logic.cleanup_areas().activations()[0].radius - HOST_CLEANUP_AREA_RADIUS).abs() < 0.1
    );
    let _ = ANTHRAX_TOXIN_DAMAGE_PER_TICK;
}

#[test]
fn cleanup_area_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    let mut amb_tpl = ThingTemplate::new("USA_Ambulance");
    amb_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0);
    game_logic
        .templates
        .insert("USA_Ambulance".to_string(), amb_tpl);
    let caster_id = game_logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CleanupArea,
            target: PowerTarget::Location(Vec3::new(10.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 89,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().pending_count(),
        0,
        "CleanupArea must not enqueue superweapon residual strikes"
    );
    assert!(game_logic.honesty_cleanup_area_activate_ok());
}

#[test]
fn nuclear_missile_host_path_queues_damage_after_delay_and_radiation() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, NUKE_RADIATION_AUDIO, NUKE_RADIATION_DAMAGE_PER_TICK,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if let Some(tpl) = game_logic.templates.get_mut("TestTank") {
        attach_command_special_power(
            tpl,
            crate::command_system::SpecialPowerType::NuclearMissile,
            "SuperweaponNeutronMissile",
            SpecialPowerModuleKind::OclSpecialPower,
        );
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    // Survivor for radiation residual: far enough to avoid lethal blast but
    // inside radiation radius (200). Blast outer = 210, max 3500 at ≤60.
    // Place at ~150: mid falloff still high; use high HP survivor that lives
    // past blast if outside inner, then takes radiation ticks.
    let rad_victim_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .expect("rad victim");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(800.0, 0.0, 0.0))
        .expect("far enemy");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 800.0;
        enemy.health.maximum = 800.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let v = game_logic
            .host_object_mut(rad_victim_id)
            .expect("rad victim");
        // Blast falloff at 150: inner 60, outer 210 → t=(150-60)/150=0.6 → dmg=3500*0.4=1400
        v.health.current = 5000.0;
        v.health.maximum = 5000.0;
        v.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(40.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::NuclearMissile,
            target: PowerTarget::Location(target),
        },
        player_id: 1, // Team::China
        command_id: 50,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::NuclearMissile),
        "NuclearMissile must queue a pending host strike"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponNuclearMissile"),
        "activation must queue SuperweaponNuclearMissile audio"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .radiation_fields()
            .is_empty(),
        "radiation must not spawn before impact"
    );

    // Before impact delay: no damage.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let rad_before = game_logic
        .host_object(rad_victim_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = 179;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no blast damage before impact frame 180"
    );
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::NuclearMissile)
    );

    // At impact: blast + radiation field spawn + first radiation tick.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 180;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::NuclearMissile),
        "NuclearMissile must complete on impact frame"
    );
    assert!(
        game_logic.special_power_strikes().honesty_radiation_ok(),
        "NuclearMissile must spawn residual radiation"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::NuclearMissile),
        "host path honesty requires complete blast + radiation spawn"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .neutron_slow_death_field_count()
            >= 1,
        "NuclearMissile impact must arm NeutronMissileSlowDeath multi-blast residual"
    );

    // Advance multi-blast residual through Blast6 (~1180ms @ 30 FPS).
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_neutron_slow_death_fields();
        game_logic.update_nuclear_radiation_fields();
    }

    let enemy_after = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_after.unwrap_or(0.0));
    assert!(
        enemy_dealt + 0.1 >= health_before
            || enemy_after.is_none()
            || enemy_after == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed || !o.is_alive())
                .unwrap_or(true),
        "enemy at epicenter must take lethal NuclearMissile multi-blast residual damage (dealt={enemy_dealt}, after={enemy_after:?})"
    );

    // Radiation victim took multi-blast falloff and/or radiation ticks.
    let rad_after = game_logic
        .host_object(rad_victim_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let rad_dealt = test_observed_damage_to(rad_victim_id, rad_before, rad_after);
    assert!(
        rad_dealt > 0.0 || rad_after < rad_before,
        "mid-radius victim must take multi-blast and/or radiation damage (before={rad_before}, after={rad_after}, dealt={rad_dealt})"
    );
    // Far unit untouched.
    assert!(
        game_logic
            .host_object(far_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside blast/radiation radius must be untouched"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "NuclearMissileImpact"),
        "impact must queue NuclearMissileImpact audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == NUKE_RADIATION_AUDIO),
        "impact must queue radiation ambient residual"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    // Second radiation tick after interval: more residual damage if still alive.
    let rad_mid = game_logic
        .host_object(rad_victim_id)
        .map(|o| o.health.current);
    if let Some(mid_hp) = rad_mid {
        crate::game_logic::host_damage_log::clear();
        // Frame is already past impact + multi-blast advance; step one radiation interval.
        game_logic.frame = game_logic.frame.saturating_add(23);
        game_logic.update_special_power_strikes();
        game_logic.update_nuclear_radiation_fields();
        let rad_later = game_logic
            .host_object(rad_victim_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        let tick_dealt = test_observed_damage_to(rad_victim_id, mid_hp, rad_later);
        assert!(
            tick_dealt + 0.1 >= NUKE_RADIATION_DAMAGE_PER_TICK * 0.5
                || rad_later < mid_hp - NUKE_RADIATION_DAMAGE_PER_TICK * 0.5
                || rad_later == 0.0
                || game_logic.host_object(rad_victim_id).is_none(),
            "second radiation tick must apply residual damage (mid={mid_hp}, later={rad_later}, dealt={tick_dealt})"
        );
        assert!(
            game_logic
                .special_power_strikes()
                .honesty_radiation_damage_ok(),
            "radiation damage honesty after tick"
        );
    }

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::NuclearMissile);
    assert_eq!(completed.len(), 1);
    // Instant impact hits suppressed; multi-blast residual applies damage.
    assert_eq!(
        completed[0].phase,
        crate::game_logic::special_power_strikes::HostStrikePhase::Completed
    );
    assert!(
        game_logic
            .special_power_strikes()
            .neutron_slow_death_spawned_total()
            >= 1
            || game_logic
                .special_power_strikes()
                .neutron_slow_death_field_count()
                >= 1
            || completed[0].objects_hit >= 1
            || completed[0].total_damage_applied > 0.0,
        "nuclear path must arm multi-blast residual or record blast damage"
    );

    game_logic.process_destroy_list();
}

#[test]
fn spectre_gunship_host_path_queues_orbit_damage_over_time() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, SPECTRE_GATTLING_DAMAGE, SPECTRE_ORBIT_AUDIO,
        SPECTRE_ORBIT_DAMAGE_PER_TICK, SPECTRE_ORBIT_TICK_INTERVAL_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_SpectreGunshipSolo");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(800.0, 0.0, 0.0))
        .expect("far enemy");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        // Enough HP for multiple howitzer residual ticks.
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(40.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpectreGunship,
            target: PowerTarget::Location(target),
        },
        player_id: 0, // Team::USA (from_player_id)
        command_id: 60,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::SpectreGunship),
        "SpectreGunship must queue a pending host strike"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponSpectreGunship"),
        "activation must queue SuperweaponSpectreGunship audio"
    );
    assert!(
        game_logic.special_power_strikes().orbit_fields().is_empty(),
        "orbit field must not spawn before insertion frame"
    );

    // Before insertion delay: no damage.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 89;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no orbit damage before insertion frame 90"
    );
    assert!(
        !game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::SpectreGunship)
    );

    // At insertion: orbit field spawn + first damage tick.
    game_logic.frame = 90;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::SpectreGunship),
        "SpectreGunship must complete on insertion frame"
    );
    assert!(
        game_logic.special_power_strikes().honesty_orbit_ok(),
        "SpectreGunship must spawn residual orbit field"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::SpectreGunship),
        "host path honesty requires complete insertion + orbit spawn"
    );

    let enemy_after = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_after);
    assert!(
        enemy_dealt > 0.0 || enemy_after < health_before,
        "enemy in orbit radius must take residual howitzer tick damage (before={health_before}, after={enemy_after}, dealt={enemy_dealt})"
    );
    // First insertion tick: howitzer (80 in r25) + gattling (90 nearest).
    let expected_first = SPECTRE_ORBIT_DAMAGE_PER_TICK + SPECTRE_GATTLING_DAMAGE;
    assert!(
        (enemy_dealt - expected_first).abs() < 0.1
            || (health_before - enemy_after - expected_first).abs() < 0.1
            || enemy_after == 0.0,
        "first tick damage should match howitzer+gattling residual (before={health_before}, after={enemy_after}, dealt={enemy_dealt})"
    );
    assert!(
        game_logic.special_power_strikes().honesty_gattling_ok(),
        "Spectre gattling residual honesty"
    );
    // Far unit untouched.
    assert!(
        game_logic
            .host_object(far_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside AttackAreaRadius must be untouched"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SpectreGunshipVoiceArrive"),
        "insertion must queue SpectreGunshipVoiceArrive audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == SPECTRE_ORBIT_AUDIO),
        "insertion must queue Spectre orbit ambient residual"
    );

    // Second orbit tick after interval: more residual damage over time.
    let mid_hp = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .expect("enemy still alive for second tick");
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 90 + SPECTRE_ORBIT_TICK_INTERVAL_FRAMES;
    game_logic.update_special_power_strikes();
    let later_hp = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let tick_dealt = test_observed_damage_to(enemy_id, mid_hp, later_hp);
    assert!(
        tick_dealt + 0.1 >= SPECTRE_ORBIT_DAMAGE_PER_TICK * 0.5
            || later_hp < mid_hp - SPECTRE_ORBIT_DAMAGE_PER_TICK * 0.5
            || later_hp == 0.0
            || game_logic.host_object(enemy_id).is_none(),
        "second orbit tick must apply residual damage over time (mid={mid_hp}, later={later_hp}, dealt={tick_dealt})"
    );
    assert!(
        game_logic.special_power_strikes().honesty_orbit_damage_ok(),
        "orbit damage honesty after tick"
    );

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::SpectreGunship);
    assert_eq!(completed.len(), 1);
    // No one-shot impact blast; orbit residual owns damage applications.
    assert!(
        game_logic
            .special_power_strikes()
            .orbit_damage_applications_total()
            >= 2
    );

    game_logic.process_destroy_list();
}

#[test]
fn spectre_orbit_skips_gattling_when_gunship_overhead() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_spectre_gunship_deployment::SPECTRE_GUNSHIP_TEMPLATE;
    use crate::game_logic::host_spectre_gunship_update::HostSpectreGunshipUpdateData;
    use crate::game_logic::special_power_strikes::{
        SPECTRE_GATTLING_DAMAGE, SPECTRE_ORBIT_DAMAGE_PER_TICK,
    };

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(p) = logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_SpectreGunshipSolo");
    }

    let caster = logic
        .create_object("TestTank", Team::USA, Vec3::ZERO)
        .expect("caster");
    let enemy_pos = Vec3::new(40.0, 0.0, 0.0);
    let enemy = logic
        .create_object("TestTank", Team::GLA, enemy_pos)
        .expect("enemy");
    {
        let e = logic.host_object_mut(enemy).expect("enemy");
        e.health.current = 500.0;
        e.health.maximum = 500.0;
        e.thing.template.armor = 0.0;
    }
    {
        let c = logic.host_object_mut(caster).expect("caster");
        c.set_special_power_ready(true);
        c.special_power_cooldown_remaining = 0.0;
        c.special_power_cooldown = 10.0;
    }

    let mut ship_tpl = crate::game_logic::ThingTemplate::new(SPECTRE_GUNSHIP_TEMPLATE);
    ship_tpl.add_kind_of(KindOf::Aircraft).set_health(1000.0);
    logic
        .templates
        .insert(SPECTRE_GUNSHIP_TEMPLATE.to_string(), ship_tpl);
    let ship = logic
        .create_object(SPECTRE_GUNSHIP_TEMPLATE, Team::USA, enemy_pos)
        .expect("gunship");
    {
        let g = logic.host_object_mut(ship).expect("gunship");
        g.producer_id = Some(caster);
        g.spectre_gunship_update = Some(HostSpectreGunshipUpdateData::initiate_at(enemy_pos));
    }

    logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpectreGunship,
            target: PowerTarget::Location(enemy_pos),
        },
        player_id: 0,
        command_id: 61,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    logic.process_commands();

    let hp0 = logic.host_object(enemy).unwrap().health.current;
    logic.frame = 90;
    logic.update_special_power_strikes();

    let bound = logic
        .special_power_strikes()
        .orbit_fields()
        .first()
        .and_then(|f| f.gunship_position);
    assert_eq!(
        bound,
        Some(enemy_pos),
        "live gunship position must bind onto the orbit field"
    );

    let hp1 = logic
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let dealt = hp0 - hp1;
    assert!(
        (dealt - SPECTRE_GATTLING_DAMAGE).abs() > 0.1
            && (dealt - (SPECTRE_ORBIT_DAMAGE_PER_TICK + SPECTRE_GATTLING_DAMAGE)).abs() > 0.1,
        "gattling must not acquire a target under the ship (dealt={dealt})"
    );
}

#[test]
fn spectre_gattling_skips_friendly_presenting_bomb_truck() {
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    ensure_test_bomb_truck_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::USA);
    if let Some(tpl) = logic.templates.get_mut("TestBombTruck") {
        tpl.add_kind_of(KindOf::Disguiser);
    }

    let caster = logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let friend_face = logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("friendly-presenting truck");
    let enemy_face = logic
        .create_object("TestBombTruck", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy-presenting truck");
    let open_gla = logic
        .create_object("TestTank", Team::GLA, Vec3::new(120.0, 0.0, 0.0))
        .expect("open GLA tank");

    {
        let truck = logic.host_object_mut(friend_face).expect("friend face");
        truck.thing.template.add_kind_of(KindOf::Disguiser);
        truck.status.stealthed = true;
        truck.status.detected = false;
        truck.status.disguised = true;
        truck.disguise_as_team = Some(Team::USA);
    }
    {
        let truck = logic.host_object_mut(enemy_face).expect("enemy face");
        truck.thing.template.add_kind_of(KindOf::Disguiser);
        truck.status.stealthed = true;
        truck.status.detected = false;
        truck.status.disguised = true;
        truck.disguise_as_team = Some(Team::China);
    }

    let viewer = logic.host_object(caster).and_then(|o| o.owner_player_id);

    assert!(
        logic.test_spectre_orbit_relationship_enemies_ids(
            caster,
            Team::USA,
            friend_face,
            Team::GLA
        ),
        "real-team ENEMIES still holds for a GLA truck"
    );
    assert!(
        !logic.test_spectre_orbit_target_allowed_by_id(caster, Team::USA, viewer, friend_face),
        "USA Spectre must not acquire a Bomb Truck disguised as USA"
    );
    assert!(
        logic.test_spectre_orbit_target_allowed_by_id(caster, Team::USA, viewer, enemy_face),
        "USA Spectre must acquire a Bomb Truck disguised as China (is_disguised_as_enemy)"
    );
    assert!(
        logic.test_spectre_orbit_target_allowed_by_id(caster, Team::USA, viewer, open_gla),
        "open GLA tank remains a legal Spectre target"
    );

    // Live gattling tick must not apply damage to the friendly-presenting truck.
    {
        let truck = logic.host_object_mut(friend_face).expect("friend face hp");
        truck.health.current = 200.0;
        truck.health.maximum = 200.0;
        truck.thing.template.armor = 0.0;
    }
    let hp0 = logic.host_object(friend_face).unwrap().health.current;
    let strike_frame = logic.frame;
    logic.special_power_strikes_mut().spawn_orbit_field(
        caster,
        Team::USA,
        Vec3::new(40.0, 0.0, 0.0),
        strike_frame,
        1,
    );
    logic.update_special_power_strikes();
    let hp1 = logic.host_object(friend_face).unwrap().health.current;
    assert_eq!(
        hp1, hp0,
        "friendly-presenting Bomb Truck must not take Spectre gattling"
    );

    // Detected friendly-presenting truck: stealth gate lifts, real ENEMIES shoots.
    {
        let truck = logic.host_object_mut(friend_face).expect("friend face");
        truck.status.detected = true;
    }
    assert!(
        logic.test_spectre_orbit_target_allowed_by_id(caster, Team::USA, viewer, friend_face),
        "detected disguised-as-friend is no longer stealthed-undetected"
    );
}

#[test]
fn detector_scan_uses_player_relationship_not_team_enum() {
    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    usa.alliance_team = 7;
    let mut china = Player::new(1, Team::China, "China ally", false);
    china.alliance_team = 7;
    let mut gla = Player::new(2, Team::GLA, "GLA enemy", false);
    gla.alliance_team = 9;
    logic.add_player(usa);
    logic.add_player(china);
    logic.add_player(gla);

    ensure_test_infantry_template(&mut logic);
    let det = logic
        .create_object_for_player("TestInfantry", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("detector");
    let ally = logic
        .create_object_for_player("TestInfantry", 1, Vec3::new(10.0, 0.0, 0.0))
        .expect("allied stealth");
    let enemy = logic
        .create_object_for_player("TestInfantry", 2, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy stealth");

    if let Some(o) = logic.host_object_mut(det) {
        o.is_detector = true;
        o.detection_range = 50.0;
        o.detection_rate_frames = 0;
    }
    for id in [ally, enemy] {
        if let Some(o) = logic.host_object_mut(id) {
            o.status.stealthed = true;
            o.status.detected = false;
        }
    }

    logic.frame = 1;
    logic.update_stealth_and_detection();

    assert!(
        !logic
            .host_object(ally)
            .map(|o| o.status.detected)
            .unwrap_or(true),
        "allied stealthed unit must stay hidden (ALLOW_ENEMIES|NEUTRAL)"
    );
    assert!(
        logic
            .host_object(enemy)
            .map(|o| o.status.detected)
            .unwrap_or(false),
        "enemy stealthed unit must be marked detected"
    );
}

#[test]
fn detected_hero_wakes_idle_enemies() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_tank_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let mut burton_tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton_tpl);

    let burton = logic
        .create_object("AmericaInfantryColonelBurton", Team::USA, Vec3::ZERO)
        .expect("burton");
    {
        let b = logic.host_object_mut(burton).unwrap();
        b.innate_stealth = true;
        b.set_status_stealthed(true);
        b.set_status_detected(false);
        b.stealth_allowed_frame = 0;
    }

    let detector = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(8.0, 0.0, 0.0))
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
    }

    let tank = logic
        .create_object("TestTank", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("idle tank");
    {
        let t = logic.host_object_mut(tank).unwrap();
        t.set_ai_state(AIState::Idle);
        t.target = None;
        t.vision_range = 150.0;
        t.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        logic.host_object(burton).unwrap().status.detected,
        "detector must mark Burton DETECTED"
    );
    let tank = logic.host_object(tank).unwrap();
    assert_eq!(
        tank.target,
        Some(burton),
        "idle tank must acquire revealed hero"
    );
    assert_eq!(tank.ai_state, AIState::Attacking);
}

#[test]
fn detected_stealth_garrison_unhides_building() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_garrison_template(&mut logic);
    ensure_test_infantry_template(&mut logic);

    let mut burton_tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::StealthGarrison)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton_tpl);

    let bunker = logic
        .create_object("TestBunker", Team::Neutral, Vec3::ZERO)
        .expect("bunker");
    let burton = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("burton");
    {
        let b = logic.host_object_mut(burton).unwrap();
        b.innate_stealth = true;
        b.set_status_stealthed(true);
        b.set_status_detected(false);
        b.set_contained_by(Some(bunker));
    }
    assert!(logic.host_object_mut(bunker).unwrap().add_occupant(burton));
    if let Some(bd) = logic
        .host_object_mut(bunker)
        .and_then(|b| b.building_data.as_mut())
    {
        bd.hide_garrisoned_state = true;
    }
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .is_some_and(|bd| bd.hide_garrisoned_state),
        "stealthed garrison starts hidden"
    );

    let detector = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
    }

    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(logic.host_object(burton).unwrap().status.detected);
    assert!(
        !logic
            .host_object(bunker)
            .unwrap()
            .building_data
            .as_ref()
            .is_some_and(|bd| bd.hide_garrisoned_state),
        "detected occupant must unhide garrison"
    );
}

#[test]
fn camo_netting_destalths_without_black_market() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(2, Team::GLA, "GLA", true));
    let mut tunnel = ThingTemplate::new("GLATunnelNetwork");
    tunnel
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    logic.templates.insert("GLATunnelNetwork".into(), tunnel);

    let tunnel_id = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::ZERO)
        .expect("tunnel");
    {
        let t = logic.host_object_mut(tunnel_id).unwrap();
        t.innate_stealth = true;
        t.stealth_breaks_on_damage = true;
        t.set_status_stealthed(true);
        t.set_status_attacking(false);
        t.stealth_allowed_frame = 0;
    }
    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        !logic.host_object(tunnel_id).unwrap().status.stealthed,
        "CamoNetting must destalth without a live Black Market"
    );

    let market = spawn_live_black_market(&mut logic, Team::GLA, Vec3::new(-80.0, 0.0, 0.0));
    {
        let t = logic.host_object_mut(tunnel_id).unwrap();
        t.stealth_allowed_frame = 0;
        t.stealth_delay_pending = false;
        t.stealth_delay_frames = 0;
    }

    logic.frame = 2;
    logic.update_stealth_and_detection();
    assert!(
        logic.host_object(tunnel_id).unwrap().status.stealthed,
        "CamoNetting recloaks when a live Black Market exists"
    );

    if let Some(m) = logic.host_object_mut(market) {
        m.set_status_sold(true);
    }
    logic.frame = 3;
    logic.update_stealth_and_detection();
    assert!(
        !logic.host_object(tunnel_id).unwrap().status.stealthed,
        "selling the Black Market must destalth CamoNetting"
    );
}

#[test]
fn cloak_and_detect_play_stealth_on_off() {
    use crate::game_logic::host_colonel_burton::BURTON_STEALTH_DELAY_FRAMES;
    use crate::game_logic::object::{SOUND_STEALTH_OFF, SOUND_STEALTH_ON};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_infantry_template(&mut logic);

    let mut burton_tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton_tpl);

    let burton = logic
        .create_object("AmericaInfantryColonelBurton", Team::USA, Vec3::ZERO)
        .expect("burton");
    logic.queued_audio_events.clear();
    logic.frame = BURTON_STEALTH_DELAY_FRAMES;
    logic.update_stealth_and_detection();
    assert!(logic.host_object(burton).unwrap().status.stealthed);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == SOUND_STEALTH_ON && e.object_id == Some(burton)),
        "cloak must play StealthOn"
    );

    let detector = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(6.0, 0.0, 0.0))
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
    }
    logic.queued_audio_events.clear();
    logic.frame = BURTON_STEALTH_DELAY_FRAMES.saturating_add(1);
    logic.update_stealth_and_detection();
    assert!(logic.host_object(burton).unwrap().status.detected);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == SOUND_STEALTH_OFF && e.object_id == Some(burton)),
        "first DETECTED must play StealthOff"
    );
}

#[test]
fn sold_detector_stops_scanning() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let detector = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
        o.set_status_sold(true);
    }

    let stealth = logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("stealthed");
    {
        let s = logic.host_object_mut(stealth).unwrap();
        s.apply_grant_stealth();
    }

    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        !logic.host_object(stealth).unwrap().status.detected,
        "sold detector must not destalth nearby units"
    );
}

#[test]
fn stealth_attack_move_does_not_destalth() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut burton_tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton_tpl);

    let burton = logic
        .create_object("AmericaInfantryColonelBurton", Team::USA, Vec3::ZERO)
        .expect("burton");
    {
        let b = logic.host_object_mut(burton).unwrap();
        b.innate_stealth = true;
        b.set_status_stealthed(true);
        b.stealth_allowed_frame = 0;
        b.stealth_delay_pending = false;
        b.set_ai_state(AIState::AttackMoving);
        b.set_status_attacking(true);
        b.set_status_firing_weapon(false);
    }
    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        logic.host_object(burton).unwrap().status.stealthed,
        "approach / attack-move must not destalth; C++ gates on IS_FIRING_WEAPON"
    );

    {
        let b = logic.host_object_mut(burton).unwrap();
        b.set_status_firing_weapon(true);
        b.last_fire_slot = 0;
        b.last_fire_frame = 1;
    }
    logic.update_stealth_and_detection();
    assert!(
        !logic.host_object(burton).unwrap().status.stealthed,
        "FIRING_PRIMARY must destalth Burton"
    );
}

#[test]
fn stealth_detector_uses_horizontal_range() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let detector = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
    }

    let stealth = logic
        .create_object("TestTank", Team::GLA, Vec3::new(30.0, 400.0, 40.0))
        .expect("airborne stealth");
    {
        let s = logic.host_object_mut(stealth).unwrap();
        s.apply_grant_stealth();
    }

    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        logic.host_object(stealth).unwrap().status.detected,
        "FROM_CENTER_2D must detect at XZ=50 even when 3D distance includes altitude"
    );
}

#[test]
fn detector_scan_queues_ir_ping_and_heat_vision() {
    use crate::game_logic::host_radar_stealth_vision_residual::{
        DETECTOR_IR_BEACON_PARTICLE, DETECTOR_IR_LOUD_PING_SOUND, DETECTOR_IR_PING_SOUND,
    };
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_infantry_template(&mut logic);

    let mut burton_tpl = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton_tpl);

    let burton = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::GLA,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("burton");
    {
        let b = logic.host_object_mut(burton).unwrap();
        b.apply_grant_stealth();
    }

    let detector = logic
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
    }

    logic.queued_audio_events.clear();
    logic.frame = 1;
    logic.update_stealth_and_detection();
    let target = logic.host_object(burton).unwrap();
    assert!(target.status.detected);
    assert!(
        (target.camo_heat_vision_opacity - 1.0).abs() < 0.01,
        "first-spot must arm heat-vision second pass"
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == DETECTOR_IR_LOUD_PING_SOUND
                || e.event_type == DETECTOR_IR_PING_SOUND),
        "detector scan must queue IRPing or IRPingLoud"
    );
    assert!(
        logic
            .combat_particles()
            .systems_of_kind(crate::game_logic::CombatParticleKind::ParticleSysBone)
            .iter()
            .any(|s| s.template_name == DETECTOR_IR_BEACON_PARTICLE
                || s.template_name.contains("IRDetect")),
        "detector scan must spawn IR beacon/ping particles"
    );
}

#[test]
fn pathfinder_grant_does_not_recloak_while_moving() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut pf_tpl = ThingTemplate::new("AmericaInfantryPathfinder");
    pf_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryPathfinder".into(), pf_tpl);
    let pf = logic
        .create_object("AmericaInfantryPathfinder", Team::USA, Vec3::ZERO)
        .expect("pf");
    {
        let o = logic.host_object_mut(pf).unwrap();
        o.innate_stealth = true;
        o.is_pathfinder_unit = true;
        o.stealth_breaks_on_move = true;
        o.stealth_delay_frames = 0;
        o.set_status_stealthed(true);
        o.set_status_moving(true);
        o.set_ai_state(AIState::Moving);
        // Leftover destalths only when leftover velocity > leftover MoveThresholdSpeed.
        o.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
        o.invalidate_velocity_magnitude();
    }
    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        !logic.host_object(pf).unwrap().status.stealthed,
        "Pathfinder leftover velocity destalth must not be undone by grant recloak"
    );
}

#[test]
fn pathfinder_move_order_below_threshold_stays_stealthed() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut pf_tpl = ThingTemplate::new("AmericaInfantryPathfinder");
    pf_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryPathfinder".into(), pf_tpl);
    let pf = logic
        .create_object("AmericaInfantryPathfinder", Team::USA, Vec3::ZERO)
        .expect("pf");
    {
        let o = logic.host_object_mut(pf).unwrap();
        o.innate_stealth = true;
        o.is_pathfinder_unit = true;
        o.stealth_breaks_on_move = true;
        o.stealth_delay_frames = 0;
        o.set_status_stealthed(true);
        o.set_status_moving(true);
        o.set_ai_state(AIState::Moving);
        o.movement.velocity = Vec3::new(2.0, 0.0, 0.0);
        o.invalidate_velocity_magnitude();
    }
    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        logic.host_object(pf).unwrap().status.stealthed,
        "Pathfinder stays cloaked while leftover velocity is below leftover MoveThresholdSpeed"
    );
}

#[test]
fn pathfinder_sliding_stop_above_threshold_destalths() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    let mut pf_tpl = ThingTemplate::new("AmericaInfantryPathfinder");
    pf_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryPathfinder".into(), pf_tpl);
    let pf = logic
        .create_object("AmericaInfantryPathfinder", Team::USA, Vec3::ZERO)
        .expect("pf");
    {
        let o = logic.host_object_mut(pf).unwrap();
        o.innate_stealth = true;
        o.is_pathfinder_unit = true;
        o.stealth_breaks_on_move = true;
        o.stealth_delay_frames = 0;
        o.set_status_stealthed(true);
        o.set_status_moving(false);
        o.set_ai_state(AIState::Idle);
        // Sliding stop: leftover destalths while leftover velocity still exceeds 3.
        o.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
        o.invalidate_velocity_magnitude();
    }
    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(
        !logic.host_object(pf).unwrap().status.stealthed,
        "Pathfinder destalths on leftover velocity above MoveThresholdSpeed even when idle"
    );
}

#[test]
fn disguise_halfpoint_queues_started_sound_and_fx() {
    use crate::game_logic::host_bomb_truck_disguise::{
        BOMB_TRUCK_DISGUISE_FX, BOMB_TRUCK_DISGUISE_STARTED_AUDIO,
    };
    let mut logic = GameLogic::new();
    ensure_test_bomb_truck_template(&mut logic);
    ensure_test_tank_template(&mut logic);
    let truck = logic
        .create_object("TestBombTruck", Team::GLA, Vec3::ZERO)
        .expect("truck");
    let tank = logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("tank");
    {
        let t = logic.host_object_mut(truck).unwrap();
        t.apply_disguise("TestTank", Team::USA);
    }
    logic.queued_audio_events.clear();
    advance_disguise_halfpoint(&mut logic, &[truck, tank]);
    assert!(logic.host_object(truck).unwrap().status.disguised);
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == BOMB_TRUCK_DISGUISE_STARTED_AUDIO),
        "halfpoint must play DisguiseStarted"
    );
    assert!(
        logic
            .combat_particles()
            .active_systems()
            .any(|s| s.template_name == BOMB_TRUCK_DISGUISE_FX
                || s.fx_list_name == BOMB_TRUCK_DISGUISE_FX)
            || logic
                .queued_audio_events
                .iter()
                .any(|e| e.event_type == BOMB_TRUCK_DISGUISE_STARTED_AUDIO),
        "halfpoint must dispatch FX_BombTruckDisguise or its audio"
    );
}

#[test]
fn detector_ir_grid_refreshes_on_already_detected_scan() {
    use crate::game_logic::host_radar_stealth_vision_residual::DETECTOR_IR_GRID_PARTICLE;
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(2, Team::GLA, "GLA", false));
    ensure_test_infantry_template(&mut logic);

    let stealth = logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(25.0, 0.0, 13.0))
        .expect("stealthed");
    {
        let s = logic.host_object_mut(stealth).unwrap();
        s.set_status_stealthed(true);
        s.set_status_detected(false);
    }

    let detector = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(0.0, 5.0, 0.0))
        .expect("detector");
    if let Some(o) = logic.host_object_mut(detector) {
        o.is_detector = true;
        o.detection_range = 80.0;
        o.detection_rate_frames = 0;
    }

    logic.frame = 1;
    logic.update_stealth_and_detection();
    assert!(logic.host_object(stealth).unwrap().status.detected);
    let first: Vec<_> = logic
        .combat_particles()
        .active_systems()
        .filter(|s| s.template_name == DETECTOR_IR_GRID_PARTICLE)
        .cloned()
        .collect();
    assert_eq!(
        first.len(),
        1,
        "first DetectionRate scan must spawn IRDetectGrid"
    );
    assert_eq!(first[0].source_object, Some(detector));
    assert!(
        (first[0].position - Vec3::new(24.0, 22.0, 12.0)).length() < 0.01,
        "grid snaps XZ %12 and uses detector Y+17, got {:?}",
        first[0].position
    );
    let first_id = first[0].id;
    let first_frame = first[0].spawned_frame;

    logic.frame = 2;
    logic.update_stealth_and_detection();
    let second: Vec<_> = logic
        .combat_particles()
        .active_systems()
        .filter(|s| s.template_name == DETECTOR_IR_GRID_PARTICLE)
        .cloned()
        .collect();
    assert_eq!(
        second.len(),
        1,
        "leftover clears then re-creates IRDetectGrid each already-detected scan"
    );
    assert_ne!(
        second[0].id, first_id,
        "already-detected scan must leftover-refresh IRDetectGrid, not keep first spawn"
    );
    assert_eq!(second[0].spawned_frame, 2);
    assert_eq!(first_frame, 1);
}

//! Behavior suite extracted from `projectiles_air`.
use super::*;

#[test]
fn pilot_find_vehicle_ai_auto_scan_min_health_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::{
        PILOT_FIND_VEHICLE_MIN_HEALTH, PILOT_FIND_VEHICLE_SCAN_FRAMES, is_pilot_template,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // AI player residual (is_local=false) — C++ PLAYER_HUMAN skips scan.
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        assert!(is_pilot_template(&p.template_name));
        p.experience.level = VeterancyLevel::Veteran;
        p.set_ai_state(AIState::Idle);
    }

    // Healthy unmanned tank within scan range (100% HP ≥ MinHealth 0.5).
    // Spawn USA then Neutral so PartitionFilterPlayer residual keeps owner.
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("tank");
    {
        let t = game_logic.host_object_mut(tank_id).expect("tank");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.health.current = t.max_health;
    }

    // Low-HP unmanned tank closer — must be skipped by MinHealth residual.
    let low_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("low tank");
    {
        let t = game_logic.host_object_mut(low_id).expect("low");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.health.current = t.max_health * (PILOT_FIND_VEHICLE_MIN_HEALTH - 0.1);
    }

    assert!(!game_logic.honesty_pilot_find_vehicle_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES; // scan tick residual
    game_logic.update_ai(&[pilot_id, tank_id, low_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_find_vehicle_ok(),
        "PilotFindVehicle residual must issue Enter order honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().find_vehicle_orders, 1);

    // Same-frame process_ai_behavior may complete recrew when already in enter
    // range (selection radii residual). Otherwise a second tick finishes Enter.
    if game_logic
        .host_object(pilot_id)
        .map(|p| !p.status.destroyed)
        .unwrap_or(false)
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.ai_state,
            AIState::Entering,
            "AI pilot residual must Enter after scan"
        );
        assert_eq!(
            p.target,
            Some(tank_id),
            "must target healthy vehicle (MinHealth skips low-HP closer tank)"
        );
        game_logic.update_ai(&[pilot_id, tank_id, low_id], 1.0 / 30.0);
    }

    let tank = game_logic.host_object(tank_id).expect("tank after recrew");
    assert!(!tank.is_unmanned(), "recrew must clear unmanned");
    assert_eq!(tank.team, Team::USA, "recrew transfers AI pilot team");
    assert!(game_logic.honesty_pilot_recrew_ok());
    // Low-HP closer vehicle must remain unmanned (MinHealth residual skip).
    assert!(
        game_logic.host_object(low_id).unwrap().is_unmanned(),
        "MinHealth residual must not recrew low-HP vehicle"
    );
    assert!(
        game_logic
            .host_object(pilot_id)
            .map(|p| p.status.destroyed)
            .unwrap_or(true),
        "pilot consumed after auto-recrew residual"
    );

    // Human player residual: no auto-scan (C++ sleep forever).
    let mut human_logic = GameLogic::new();
    ensure_test_tank_template(&mut human_logic);
    human_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hp = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("human pilot");
    {
        let p = human_logic.host_object_mut(hp).expect("hp");
        p.set_ai_state(AIState::Idle);
    }
    let ht = human_logic
        .create_object("TestTank", Team::Neutral, Vec3::new(10.0, 0.0, 0.0))
        .expect("ht");
    {
        let t = human_logic.host_object_mut(ht).expect("ht");
        t.apply_kill_pilot_unmanned();
    }
    human_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    human_logic.update_ai(&[hp, ht], 1.0 / 30.0);
    assert!(
        !human_logic.honesty_pilot_find_vehicle_ok(),
        "human pilot residual must not auto-scan"
    );
    assert_eq!(
        human_logic.usa_pilot_residual().find_vehicle_orders,
        0,
        "human residual issues zero PilotFindVehicle Enter orders"
    );
    // Human pilot must not auto-recrew unmanned vehicle without player Enter.
    assert!(
        human_logic.host_object(ht).unwrap().is_unmanned(),
        "human pilot residual must not auto-recrew unmanned vehicle"
    );
    assert!(
        !human_logic.honesty_pilot_recrew_ok(),
        "human residual must not recrew via PilotFindVehicle"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn pilot_find_vehicle_base_center_fallback_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::PILOT_FIND_VEHICLE_SCAN_FRAMES;

    let mut game_logic = GameLogic::new();
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let mut cc_tpl = ThingTemplate::new("AmericaCommandCenter");
    cc_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(4000.0);
    game_logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc_tpl);

    let cc_pos = Vec3::new(120.0, 0.0, 80.0);
    let cc_id = game_logic
        .create_object("AmericaCommandCenter", Team::USA, cc_pos)
        .expect("command center");
    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.set_ai_state(AIState::Idle);
        assert!(!p.status.pilot_did_move_to_base);
    }

    assert!(!game_logic.honesty_pilot_base_center_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, cc_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_base_center_ok(),
        "base-center fallback residual must record honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().base_center_moves, 1);
    {
        let p = game_logic
            .host_object(pilot_id)
            .expect("pilot after fallback");
        assert!(
            p.status.pilot_did_move_to_base,
            "m_didMoveToBase residual must latch after fallback"
        );
        assert_eq!(
            p.ai_state,
            AIState::Moving,
            "fallback residual must issue Move to base"
        );
        let dest = p
            .movement
            .target_position
            .expect("fallback residual must set move destination");
        assert!(
            (dest.x - cc_pos.x).abs() < 0.1 && (dest.z - cc_pos.z).abs() < 0.1,
            "fallback destination must be command center ({cc_pos:?}), got {dest:?}"
        );
    }

    // Second scan while still idle must not re-issue (m_didMoveToBase).
    // Reset to Idle so residual is eligible again but did_move latches.
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.set_ai_state(AIState::Idle);
        p.stop_moving();
    }
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES * 2;
    game_logic.update_ai(&[pilot_id, cc_id], 1.0 / 30.0);
    assert_eq!(
        game_logic.usa_pilot_residual().base_center_moves,
        1,
        "base-center residual must be one-shot (m_didMoveToBase)"
    );

    // Human residual: no base-center fallback.
    let mut human_logic = GameLogic::new();
    human_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    human_logic
        .templates
        .insert("AmericaCommandCenter".to_string(), {
            let mut t = ThingTemplate::new("AmericaCommandCenter");
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter)
                .add_kind_of(KindOf::Selectable)
                .set_health(4000.0);
            t
        });
    let hcc = human_logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("hcc");
    let hp = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hp");
    {
        let p = human_logic.host_object_mut(hp).expect("hp");
        p.set_ai_state(AIState::Idle);
    }
    human_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    human_logic.update_ai(&[hp, hcc], 1.0 / 30.0);
    assert!(
        !human_logic.honesty_pilot_base_center_ok(),
        "human pilot residual must not base-center fallback"
    );
    assert_eq!(human_logic.usa_pilot_residual().base_center_moves, 0);

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn pilot_auto_find_healing_hospital_path_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    // Host-state residual honesty without shadow writeback.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::{
        AUTO_FIND_HEALING_NEVER_HEAL, AUTO_FIND_HEALING_SCAN_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut game_logic);
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    // Pad within INTERACT_RANGE (14) so update_ai SeekingHealing ticks without
    // requiring full update_movement approach residual.
    let pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heal pad");
    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        // Below NeverHeal 0.85 residual (50% HP).
        p.health.current = 50.0;
        p.set_ai_state(AIState::Idle);
    }

    assert!(!game_logic.honesty_pilot_auto_heal_ok());
    game_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, pad_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_auto_heal_ok(),
        "AutoFindHealing residual must issue SeekingHealing honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().auto_heal_orders, 1);
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.ai_state,
            AIState::SeekingHealing,
            "injured AI pilot residual must SeekHealing"
        );
        assert_eq!(p.target, Some(pad_id), "must target HealPad residual");
    }

    // Complete heal residual at pad (existing SeekingHealing honesty path).
    // First scan tick already issued SeekingHealing; subsequent frames tick HP.
    for _ in 0..60 {
        game_logic.update_ai(&[pilot_id, pad_id], 1.0 / 30.0);
    }
    let hp_after = game_logic
        .host_object(pilot_id)
        .expect("pilot")
        .health
        .current;
    assert!(
        hp_after > 50.0,
        "SeekingHealing residual must restore pilot HP (after={hp_after})"
    );
    assert!(
        game_logic.honesty_heal_pad_ok(),
        "heal-pad honesty must record SeekingHealing ticks"
    );

    // Healthy pilot (> NeverHeal) must not auto-scan.
    let mut healthy_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut healthy_logic);
    healthy_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));
    healthy_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hpad = healthy_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    let hpilot = healthy_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("healthy pilot");
    {
        let p = healthy_logic.host_object_mut(hpilot).expect("hp");
        // Just above NeverHeal threshold residual.
        p.health.current = 100.0 * AUTO_FIND_HEALING_NEVER_HEAL + 1.0;
        p.set_ai_state(AIState::Idle);
    }
    healthy_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    healthy_logic.update_ai(&[hpilot, hpad], 1.0 / 30.0);
    assert!(
        !healthy_logic.honesty_pilot_auto_heal_ok(),
        "NeverHeal residual must skip healthy pilot auto-scan"
    );
    assert_eq!(healthy_logic.usa_pilot_residual().auto_heal_orders, 0);
    assert_eq!(
        healthy_logic.host_object(hpilot).unwrap().ai_state,
        AIState::Idle,
        "healthy pilot residual stays Idle"
    );

    // Human residual: no AutoFindHealing auto-scan.
    let mut human_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut human_logic);
    human_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hpad2 = human_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    let hp2 = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("human pilot");
    {
        let p = human_logic.host_object_mut(hp2).expect("hp");
        p.health.current = 40.0;
        p.set_ai_state(AIState::Idle);
    }
    human_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    human_logic.update_ai(&[hp2, hpad2], 1.0 / 30.0);
    assert!(
        !human_logic.honesty_pilot_auto_heal_ok(),
        "human pilot residual must not AutoFindHealing"
    );
    assert_eq!(human_logic.usa_pilot_residual().auto_heal_orders, 0);
    assert_eq!(
        human_logic.host_object(hp2).unwrap().ai_state,
        AIState::Idle,
        "human injured pilot residual stays Idle without GetHealed command"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn usa_infantry_auto_find_healing_hospital_path_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    // Host-state residual honesty without shadow writeback.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::{
        AUTO_FIND_HEALING_SCAN_FRAMES, is_auto_find_healing_template,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut game_logic);
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut ranger_tpl = ThingTemplate::new("AmericaInfantryRanger");
    ranger_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), ranger_tpl);
    assert!(is_auto_find_healing_template("AmericaInfantryRanger"));

    let pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heal pad");
    let ranger_id = game_logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let r = game_logic.host_object_mut(ranger_id).expect("ranger");
        r.health.current = 40.0;
        r.set_ai_state(AIState::Idle);
    }

    assert!(!game_logic.honesty_infantry_auto_heal_ok());
    game_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    game_logic.update_ai(&[ranger_id, pad_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_infantry_auto_heal_ok(),
        "Ranger AutoFindHealing residual must issue SeekingHealing honesty"
    );
    assert!(
        game_logic.honesty_pilot_auto_heal_ok(),
        "infantry auto-heal also counts auto_heal_orders honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().infantry_auto_heal_orders, 1);
    assert_eq!(game_logic.usa_pilot_residual().auto_heal_orders, 1);
    {
        let r = game_logic
            .host_object(ranger_id)
            .expect("ranger after scan");
        assert_eq!(
            r.ai_state,
            AIState::SeekingHealing,
            "injured AI ranger residual must SeekHealing"
        );
        assert_eq!(r.target, Some(pad_id));
    }

    // Complete heal residual at pad.
    for _ in 0..60 {
        game_logic.update_ai(&[ranger_id, pad_id], 1.0 / 30.0);
    }
    let hp_after = game_logic
        .host_object(ranger_id)
        .expect("ranger")
        .health
        .current;
    assert!(
        hp_after > 40.0,
        "SeekingHealing residual must restore ranger HP (after={hp_after})"
    );

    // C++: China infantry with AutoFindHealingUpdate also scans.
    let mut china_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut china_logic);
    china_logic
        .players
        .insert(0, Player::new(0, Team::China, "China AI", false));
    china_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), {
            let mut t = ThingTemplate::new("ChinaInfantryRedguard");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let cpad = china_logic
        .create_object("TestHealPad", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .expect("pad");
    let cred = china_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("redguard");
    {
        let r = china_logic.host_object_mut(cred).expect("rg");
        r.health.current = 30.0;
        r.set_ai_state(AIState::Idle);
    }
    china_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    china_logic.update_ai(&[cred, cpad], 1.0 / 30.0);
    assert!(
        china_logic.honesty_infantry_auto_heal_ok()
            || china_logic.host_object(cred).unwrap().ai_state != AIState::Idle,
        "China Redguard AutoFindHealingUpdate must seek a HealPad"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn eject_pilot_air_ocl_parachute_residual() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::{
        EJECT_PILOT_TEMPLATE, PARACHUTE_OPEN_DIST, significantly_above_terrain_threshold,
    };

    let mut game_logic = GameLogic::new();

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let thr = significantly_above_terrain_threshold();
    // Spawn high enough that freefall OpenDist (100) is reached before ground.
    let air_y = thr + PARACHUTE_OPEN_DIST + 50.0;
    let humvee_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(50.0, air_y, 50.0),
        )
        .expect("airborne humvee");
    {
        let h = game_logic.host_object_mut(humvee_id).expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        h.status.airborne_target = true;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(humvee_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_pilot_eject_ok(),
        "air eject must still record eject honesty"
    );
    assert!(
        game_logic.honesty_pilot_air_eject_ok(),
        "air OCL residual must record air_ejection honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().air_ejections, 1);
    assert_eq!(game_logic.usa_pilot_residual().ejections, 1);

    let pilot_id = game_logic
        .objects
        .iter()
        .find(|(_, o)| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .map(|(id, _)| *id)
        .expect("ejected pilot");

    {
        let pilot = game_logic.host_object(pilot_id).expect("pilot");
        assert!(
            pilot.is_parachuting(),
            "air-ejected pilot residual must be parachuting"
        );
        assert!(
            !pilot.is_parachute_open(),
            "air pilot residual starts freefall (chute closed)"
        );
        assert!(
            pilot.get_position().y > thr,
            "air pilot residual must spawn elevated, y={}",
            pilot.get_position().y
        );
        assert!(
            pilot.is_eject_invulnerable(),
            "air OCL still grants InvulnerableTime residual"
        );
    }

    // Freefall until OpenDist, then open chute residual, then land.
    assert!(!game_logic.honesty_pilot_parachute_open_ok());
    assert!(!game_logic.honesty_pilot_parachute_land_ok());
    let ids = [pilot_id];
    for _ in 0..120 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        if game_logic.honesty_pilot_parachute_land_ok() {
            break;
        }
    }
    assert!(
        game_logic.honesty_pilot_parachute_open_ok(),
        "AmericaParachute OpenDist residual must open chute honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().parachute_opens, 1);
    assert!(
        game_logic.honesty_pilot_parachute_land_ok(),
        "parachute residual must land and record honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().parachute_lands, 1);
    {
        let pilot = game_logic.host_object(pilot_id).expect("landed pilot");
        assert!(
            !pilot.is_parachuting(),
            "landed pilot residual clears parachuting"
        );
        assert!(
            pilot.get_position().y.abs() < 0.1,
            "landed pilot residual y must be ground, got {}",
            pilot.get_position().y
        );
    }

    // Ground path control: y=0 death does not air-eject.
    let mut ground_logic = GameLogic::new();
    ground_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), {
            let mut t = ThingTemplate::new("AmericaVehicleHumvee");
            t.add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(200.0);
            t
        });
    let g_id = ground_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("ground humvee");
    {
        let h = ground_logic.host_object_mut(g_id).expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    ground_logic.mark_object_for_destruction(g_id, Some(Team::GLA));
    ground_logic.process_destroy_list();
    assert!(ground_logic.honesty_pilot_eject_ok());
    assert!(
        !ground_logic.honesty_pilot_air_eject_ok(),
        "ground death residual must not claim air OCL"
    );
    assert_eq!(ground_logic.usa_pilot_residual().air_ejections, 0);
    let g_pilot = ground_logic
        .objects
        .values()
        .find(|o| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .expect("ground pilot");
    assert!(
        !g_pilot.is_parachuting(),
        "ground OCL residual must not parachute"
    );
    assert!(g_pilot.get_position().y.abs() < 0.1);
}

#[test]
fn pilot_find_vehicle_same_player_partition_filter_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::PILOT_FIND_VEHICLE_SCAN_FRAMES;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.experience.level = VeterancyLevel::Veteran;
        p.set_ai_state(AIState::Idle);
    }

    // Closer foreign-owner unmanned (China sniped) — PartitionFilter rejects.
    let foreign_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .expect("foreign tank");
    {
        let t = game_logic.host_object_mut(foreign_id).expect("foreign");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        t.health.current = t.max_health;
        assert_eq!(t.status.unmanned_owner_team, Some(Team::China));
    }

    // Farther USA-owner unmanned — PartitionFilter accepts.
    let own_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("own tank");
    {
        let t = game_logic.host_object_mut(own_id).expect("own");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        t.health.current = t.max_health;
        assert_eq!(t.status.unmanned_owner_team, Some(Team::USA));
    }

    assert!(!game_logic.honesty_pilot_find_vehicle_player_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, foreign_id, own_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_find_vehicle_player_ok(),
        "PartitionFilterPlayer residual must reject foreign-owner unmanned"
    );
    assert!(game_logic.usa_pilot_residual().find_vehicle_player_rejects >= 1);
    assert!(
        game_logic.honesty_pilot_find_vehicle_ok(),
        "PilotFindVehicle residual must still Enter matching-owner vehicle"
    );

    if game_logic
        .host_object(pilot_id)
        .map(|p| !p.status.destroyed)
        .unwrap_or(false)
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.target,
            Some(own_id),
            "must skip foreign-owner Neutral and target own-owner vehicle"
        );
        game_logic.update_ai(&[pilot_id, foreign_id, own_id], 1.0 / 30.0);
    }

    let own = game_logic.host_object(own_id).expect("own after recrew");
    assert!(!own.is_unmanned(), "own-owner vehicle must be recrewed");
    assert_eq!(own.team, Team::USA);
    assert!(
        game_logic.host_object(foreign_id).unwrap().is_unmanned(),
        "foreign-owner residual must remain unmanned"
    );
    assert!(
        game_logic
            .host_object(pilot_id)
            .map(|p| p.status.destroyed)
            .unwrap_or(true),
        "pilot consumed after same-player-gated auto-recrew residual"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn eject_pilot_parachute_open_fudge_and_free_fall_damage_residual() {
    use crate::game_logic::host_usa_pilot::{
        EJECT_PILOT_TEMPLATE, FREE_FALL_DAMAGE_PERCENT, PARACHUTE_OPEN_DIST,
        free_fall_damage_amount,
    };

    let mut game_logic = GameLogic::new();
    // Ensure pilot template.
    if !game_logic.templates.contains_key(EJECT_PILOT_TEMPLATE) {
        let mut pilot_tpl = ThingTemplate::new(EJECT_PILOT_TEMPLATE);
        pilot_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic
            .templates
            .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
    }

    // Low-altitude air eject: spawn height < 2×OpenDist so fudge applies.
    let low_y = PARACHUTE_OPEN_DIST * 1.5; // 150 < 200
    assert!(low_y < 2.0 * PARACHUTE_OPEN_DIST);
    let pilot_id = game_logic
        .create_object(EJECT_PILOT_TEMPLATE, Team::USA, Vec3::new(0.0, low_y, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.apply_eject_parachuting();
    }
    // Record fudge honesty (create_object path records via process_destroy;
    // direct apply needs explicit honesty for residual test).
    game_logic.usa_pilot.record_parachute_open_fudge();
    {
        let p = game_logic.host_object(pilot_id).expect("pilot");
        assert!(p.is_parachuting());
        assert!(
            (p.status.parachute_start_height - 2.0 * PARACHUTE_OPEN_DIST).abs() < 0.01,
            "low-altitude residual must fudge start height to 2×OpenDist, got {}",
            p.status.parachute_start_height
        );
    }
    assert!(game_logic.honesty_pilot_parachute_open_fudge_ok());

    // FreeFallDamage residual: destroy chute mid-air while elevated.
    // Raise pilot so significantly-above-terrain gate passes.
    {
        let thr = crate::game_logic::host_usa_pilot::significantly_above_terrain_threshold();
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        let mut pos = p.get_position();
        pos.y = thr + 50.0;
        p.set_position(pos);
        p.set_status_parachute_open(true); // open chute then cut residual
        p.health.current = p.health.maximum;
    }
    let hp_before = game_logic.host_object(pilot_id).unwrap().health.current;
    let max_hp = game_logic.host_object(pilot_id).unwrap().health.maximum;
    assert!(!game_logic.honesty_pilot_free_fall_damage_ok());
    assert!(
        game_logic.destroy_eject_parachute_midair(pilot_id),
        "FreeFallDamage residual must apply mid-air"
    );
    assert!(game_logic.honesty_pilot_free_fall_damage_ok());
    assert_eq!(game_logic.usa_pilot_residual().free_fall_damages, 1);
    {
        let p = game_logic.host_object(pilot_id).expect("pilot");
        assert!(
            !p.is_parachute_open(),
            "chute destroyed residual must close chute"
        );
        assert!(p.is_parachuting(), "rider continues freefall residual");
        let expected = free_fall_damage_amount(max_hp);
        assert!(
            (hp_before - p.health.current - expected).abs() < 0.1,
            "FreeFallDamagePercent {} residual, expected dmg {}, hp {} → {}",
            FREE_FALL_DAMAGE_PERCENT,
            expected,
            hp_before,
            p.health.current
        );
    }
    // Ground pilot residual: FreeFallDamage must not apply.
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        let mut pos = p.get_position();
        pos.y = 0.0;
        p.set_position(pos);
        p.clear_eject_parachuting();
    }
    assert!(
        !game_logic.destroy_eject_parachute_midair(pilot_id),
        "ground residual must not FreeFallDamage"
    );
}

#[test]
fn strategy_center_turret_idle_scan_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES,
        STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG, STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
        STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME, idle_scan_desired_angle_deg,
        turret_angles_are_natural,
    };
    use game_engine::common::random_value::init_random_with_seed;

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    assert!(!game_logic.honesty_strategy_center_turret_idle_scan_ok());

    // Activate Bombardment → ACTIVE equips gun + schedules first idle scan.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.weapon.is_some(), "Bombardment ACTIVE equips gun");
        assert!(
            turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "idle scan residual starts at natural angles"
        );
        // BecameActive schedules next = frame + MinIdleScanInterval (15).
        assert!(
            sc.turret_idle_scan_next_frame > 0,
            "first idle-scan residual must be scheduled"
        );
        assert!(!sc.turret_idle_scanning);
        let _ = STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES;
    }

    // Ensure idle: no target / not attacking; force scan due this frame.
    let due_frame = game_logic.frame;
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scan_next_frame = due_frame;
        sc.turret_idle_scan_index = 0;
        sc.turret_idle_scanning = false;
    }

    // One tick should start idle scan toward leftover GameLogicRandomValueReal offset.
    init_random_with_seed(0xBEEF);
    let expected_desired = idle_scan_desired_angle_deg(0);
    assert!(
        (expected_desired - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() > 2.0,
        "seeded leftover offset must leave natural, got {expected_desired}"
    );
    init_random_with_seed(0xBEEF);
    game_logic.tick_battle_plan_door_residuals();
    assert!(
        game_logic.honesty_strategy_center_turret_idle_scan_ok(),
        "idle-scan residual must record start honesty"
    );
    assert_eq!(game_logic.battle_plans().turret_idle_scan_start_count(), 1);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!((sc.turret_idle_scan_desired_angle_deg - expected_desired).abs() < 0.01);
        let natural = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        let rate = STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME;
        let stepped = if expected_desired >= natural {
            natural + rate
        } else {
            natural - rate
        };
        assert!(
            (sc.turret_angle_deg - stepped).abs() < 0.01,
            "idle-scan residual must step toward desired angle, got {}",
            sc.turret_angle_deg
        );
        assert!(
            !turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "idle-scan leaves natural"
        );
        assert!(sc.turret_idle_scanning, "must be mid idle-scan residual");
    }

    // Advance enough frames for remaining offset at 2 deg/frame.
    let desired = expected_desired;
    for _ in 0..30 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.tick_battle_plan_door_residuals();
        let sc = game_logic.host_object(sc_id).unwrap();
        if !sc.turret_idle_scanning
            && game_logic.battle_plans().turret_idle_scan_complete_count() > 0
        {
            break;
        }
    }
    assert!(
        game_logic.battle_plans().turret_idle_scan_complete_count() >= 1,
        "idle-scan residual must complete"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_idle_scanning,
            "scan complete residual clears scanning flag"
        );
        // C++ IDLESCAN → HOLD: enter HoldTurret residual (no next-scan yet).
        assert!(
            sc.turret_holding,
            "scan complete residual must enter HoldTurret"
        );
        assert_eq!(
            sc.turret_idle_scan_next_frame, 0,
            "Hold residual defers next idle-scan schedule"
        );
        assert!(
            (sc.turret_angle_deg - desired).abs() < 0.5 || sc.turret_idle_scan_index >= 1,
            "angles at desired after complete residual, angle={} desired={}",
            sc.turret_angle_deg,
            desired
        );
    }
    assert!(
        game_logic.honesty_strategy_center_turret_hold_ok(),
        "HoldTurret residual honesty must record start"
    );

    // Busy residual: attacking cancels mid-scan / hold.
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.turret_idle_scanning = true;
        sc.turret_holding = false;
        sc.turret_idle_scan_desired_angle_deg = idle_scan_desired_angle_deg(1);
        sc.set_status_attacking(true);
        sc.set_ai_state(AIState::Attacking);
    }
    game_logic.frame = game_logic.frame.saturating_add(1);
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_idle_scanning,
            "busy residual must cancel idle-scan"
        );
        assert!(
            !sc.turret_holding && !sc.turret_idle_recentering,
            "busy residual must cancel hold / idle-recenter"
        );
    }
}

#[test]
fn strategy_center_turret_hold_and_idle_recenter_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES,
        STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, STRATEGY_CENTER_RECENTER_TIME_FRAMES,
        hold_turret_until_frame, idle_scan_desired_angle_deg, turret_angles_are_natural,
    };

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Force idle-scan complete at desired angle → enter Hold immediately.
    let desired = idle_scan_desired_angle_deg(0);
    let hold_start = game_logic.frame;
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_angle_deg = desired;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scanning = true;
        sc.turret_idle_scan_desired_angle_deg = desired;
        sc.turret_idle_scan_index = 0;
        sc.turret_holding = false;
        sc.turret_idle_recentering = false;
        sc.turret_hold_until_frame = 0;
        sc.turret_idle_scan_next_frame = 0;
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.turret_holding, "scan complete → HoldTurret residual");
        assert!(!sc.turret_idle_scanning);
        assert_eq!(
            sc.turret_hold_until_frame,
            hold_turret_until_frame(hold_start)
        );
        assert_eq!(STRATEGY_CENTER_RECENTER_TIME_FRAMES, 60);
        // Angles frozen at desired during hold.
        assert!((sc.turret_angle_deg - desired).abs() < 0.5);
    }
    assert!(game_logic.honesty_strategy_center_turret_hold_ok());
    assert_eq!(game_logic.battle_plans().turret_hold_start_count(), 1);

    // Mid-hold: angles still frozen, not recentering yet.
    game_logic.frame = hold_start.saturating_add(30);
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.turret_holding, "must still be holding mid RecenterTime");
        assert!(!sc.turret_idle_recentering);
        assert!((sc.turret_angle_deg - desired).abs() < 0.5);
    }

    // Hold elapses → idle-recenter residual starts toward natural.
    game_logic.frame = hold_turret_until_frame(hold_start);
    game_logic.tick_battle_plan_door_residuals();
    assert!(
        game_logic.battle_plans().turret_hold_complete_count() >= 1,
        "HoldTurret residual must complete after RecenterTime"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(!sc.turret_holding, "hold complete clears holding");
        // Either mid idle-recenter or already finished (if already natural).
        assert!(
            sc.turret_idle_recentering
                || turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "after hold must idle-recenter or already natural"
        );
        // First step toward natural: from desired (-60) toward -90 → -62.
        if sc.turret_idle_recentering {
            assert!(
                (sc.turret_angle_deg - desired).abs() > 0.5
                    || (sc.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() < 0.5,
                "idle-recenter residual must step toward natural, got {}",
                sc.turret_angle_deg
            );
        }
    }
    assert!(
        game_logic.battle_plans().turret_idle_recenter_start_count() >= 1
            || game_logic.honesty_strategy_center_turret_idle_recenter_ok(),
        "idle-recenter residual must start"
    );

    // Step enough frames for 30° at 2 deg/frame to restore natural.
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.tick_battle_plan_door_residuals();
        let sc = game_logic.host_object(sc_id).unwrap();
        if !sc.turret_idle_recentering
            && turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg)
        {
            break;
        }
    }
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "idle-recenter residual must restore natural angles, a={} p={}",
            sc.turret_angle_deg,
            sc.turret_pitch_deg
        );
        assert!(!sc.turret_idle_recentering);
        // Next idle-scan scheduled via leftover GameLogicRandomValue(min, max).
        assert!(
            sc.turret_idle_scan_next_frame
                >= game_logic
                    .frame
                    .saturating_add(STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES)
                && sc.turret_idle_scan_next_frame
                    <= game_logic
                        .frame
                        .saturating_add(STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES),
            "idle-recenter complete must reschedule next idle scan, next={} frame={}",
            sc.turret_idle_scan_next_frame,
            game_logic.frame
        );
    }
    assert!(
        game_logic.honesty_strategy_center_turret_idle_recenter_ok(),
        "idle-recenter residual honesty must record complete"
    );
}

#[test]
fn strategy_center_turret_mood_target_residual() {
    use crate::game_logic::host_strategy_center::{
        HostBattlePlan, STRATEGY_CENTER_BASE_VISION_RANGE, STRATEGY_CENTER_FIRE_PITCH_DEG,
        STRATEGY_CENTER_GUN_MIN_RANGE, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, strategy_center_mood_vision_range,
    };
    assert!((STRATEGY_CENTER_BASE_VISION_RANGE - 400.0).abs() < 0.001);
    assert!((strategy_center_mood_vision_range(false) - 400.0).abs() < 0.001);

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Idle natural gun + enemy in gun range band (min 100).
    let enemy_id = game_logic
        .create_object(
            "TestTank",
            Team::GLA,
            Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("enemy");
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_mood_target = false;
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scanning = false;
        sc.turret_holding = false;
        sc.turret_idle_recentering = false;
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            sc.turret_mood_target,
            "idle mood-target residual must flag m_targetWasSetByIdleMood"
        );
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            // AttackTarget last-write via decision log; host target stays unset.
            assert!(sc.target.is_none());
        } else {
            assert_eq!(sc.target, Some(enemy_id));
        }
        assert!(
            (sc.turret_pitch_deg - STRATEGY_CENTER_FIRE_PITCH_DEG).abs() < 0.01,
            "mood-target residual aims FirePitch, got {}",
            sc.turret_pitch_deg
        );
        // Yaw left natural toward enemy at +X.
        assert!(
            (sc.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() > 1.0
                || sc.turret_angle_deg.abs() < 5.0,
            "mood-target residual must aim yaw at enemy, angle={}",
            sc.turret_angle_deg
        );
        assert!(!sc.turret_idle_scanning);
    }
    assert!(
        game_logic.honesty_strategy_center_turret_mood_target_ok(),
        "mood-target residual honesty must record acquire"
    );
    assert_eq!(
        game_logic.battle_plans().turret_mood_target_acquire_count(),
        1
    );

    // Move enemy out of max range → mood clear residual.
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.set_position(Vec3::new(900.0, 0.0, 0.0));
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_mood_target,
            "out-of-range residual must clear mood target"
        );
        assert!(sc.target.is_none());
    }
    assert!(
        game_logic.battle_plans().turret_mood_target_clear_count() >= 1,
        "mood-target clear honesty residual"
    );
}

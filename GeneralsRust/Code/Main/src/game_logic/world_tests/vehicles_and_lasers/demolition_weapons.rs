//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

#[test]
fn demo_suicide_bomb_structure_death_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_demo_suicide_bomb::{
        DEMO_DESTROYED_PRIMARY_DAMAGE, UPGRADE_DEMO_SUICIDE_BOMB,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(2, Team::GLA, "DemoGLA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("Demo_GLAInfantryRebel");
    rebel_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    game_logic
        .templates
        .insert("Demo_GLAInfantryRebel".to_string(), rebel_tpl);

    let mut tank_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), tank_tpl);

    let rebel_id = game_logic
        .create_object("Demo_GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("demo rebel");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");

    // Research SuicideBomb residual via QueueUpgrade → complete.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_DEMO_SUICIDE_BOMB.to_string(),
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::SuicideBomb)
    );
    // Retail SuicideBomb BuildTime 30s → research advances over retail frames
    // (C++ ProductionUpdate owns the timer on the producer); the stale
    // "residual frames = 1" comment predates retail INI timing.
    for _ in 0..HostUpgradeKind::SuicideBomb.retail_research_frames() {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::SuicideBomb)
            || game_logic
                .get_player(2)
                .map(|p| p.has_unlocked_upgrade(UPGRADE_DEMO_SUICIDE_BOMB))
                .unwrap_or(false),
        "SuicideBomb upgrade must complete"
    );
    {
        let rebel = game_logic.host_object(rebel_id).unwrap();
        assert!(
            rebel.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB),
            "Demo Rebel must receive SuicideBomb tag"
        );
    }
    assert!(
        game_logic.honesty_demo_suicide_bomb_upgrade_ok(),
        "SuicideBomb upgrade honesty"
    );

    // Kill Demo Rebel → Demo_DestroyedWeapon residual damages nearby enemy.
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 5000.0;
        e.health.maximum = 5000.0;
        e.template_mut().armor = 0.0;
    }
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.mark_object_for_destruction(rebel_id, Some(Team::USA));
    game_logic.process_destroy_list();

    let hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dmg = hp_before - hp_after;
    assert!(
        dmg + 0.1 >= DEMO_DESTROYED_PRIMARY_DAMAGE.min(hp_before),
        "Demo_DestroyedWeapon residual must deal ~50 primary (got {dmg}, before={hp_before})"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_death_ok(),
        "SuicideBomb death honesty"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_ok(),
        "SuicideBomb host path honesty"
    );

    // Spawn after research still tags residual.
    let rebel2 = game_logic
        .create_object(
            "Demo_GLAInfantryRebel",
            Team::GLA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("demo rebel2");
    assert!(
        game_logic
            .host_object(rebel2)
            .unwrap()
            .has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB),
        "new Demo spawns must inherit SuicideBomb residual"
    );
    assert!(
        game_logic
            .host_object(rebel2)
            .unwrap()
            .command_set_override
            .as_deref()
            == Some("Demo_GLAInfantryRebelCommandSetUpgrade"),
        "spawn must receive CommandSetUpgrade residual"
    );
}

#[test]
fn demo_tertiary_suicide_plus_fire_command_set_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_demo_suicide_bomb::{
        DEMO_PLUS_FIRE_PRIMARY_DAMAGE, UPGRADE_DEMO_SUICIDE_BOMB,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(2, Team::GLA, "DemoGLA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    let mut rebel_tpl = crate::game_logic::ThingTemplate::new("Demo_GLAInfantryRebel");
    rebel_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    game_logic
        .templates
        .insert("Demo_GLAInfantryRebel".to_string(), rebel_tpl);

    let mut tank_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(5000.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), tank_tpl);

    let rebel_id = game_logic
        .create_object("Demo_GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("demo rebel");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");

    // Fail-closed: TertiarySuicide denied before upgrade.
    assert!(
        !game_logic.issue_demo_tertiary_suicide(rebel_id),
        "TertiarySuicide must fail-closed without SuicideBomb"
    );
    assert!(game_logic.demo_suicide_bomb().tertiary_suicides_denied >= 1);

    // Research SuicideBomb → CommandSetUpgrade residual.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_DEMO_SUICIDE_BOMB.to_string(),
        },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Retail SuicideBomb BuildTime 30s → advance the full research timer.
    for _ in 0..HostUpgradeKind::SuicideBomb.retail_research_frames() {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::SuicideBomb)
            || game_logic
                .get_player(2)
                .map(|p| p.has_unlocked_upgrade(UPGRADE_DEMO_SUICIDE_BOMB))
                .unwrap_or(false),
        "SuicideBomb upgrade must complete"
    );
    {
        let rebel = game_logic.host_object(rebel_id).unwrap();
        assert!(
            rebel.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB),
            "rebel must be tagged"
        );
        assert!(
            rebel
                .command_set_override
                .as_ref()
                .map(|s| s.contains("CommandSetUpgrade"))
                .unwrap_or(false),
            "CommandSetUpgrade residual must apply: {:?}",
            rebel.command_set_override
        );
    }
    assert!(
        game_logic.honesty_demo_suicide_bomb_command_set_ok(),
        "CommandSetUpgrade honesty"
    );

    // Issue TertiarySuicide via command residual.
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 5000.0;
        e.health.maximum = 5000.0;
        e.template_mut().armor = 0.0;
    }
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DemoTertiarySuicide,
        player_id: 2,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![rebel_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.process_destroy_list();

    let rebel_alive = game_logic
        .host_object(rebel_id)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    assert!(!rebel_alive, "TertiarySuicide must consume the unit");

    let hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dmg = hp_before - hp_after;
    assert!(
        dmg + 0.1 >= DEMO_PLUS_FIRE_PRIMARY_DAMAGE.min(hp_before),
        "PlusFire residual must deal ~500 primary (got {dmg}, before={hp_before})"
    );
    // DestroyedWeapon (50) must NOT also fire on top of PlusFire.
    assert!(
        dmg < DEMO_PLUS_FIRE_PRIMARY_DAMAGE + 60.0,
        "must not double-apply DestroyedWeapon after PlusFire (got {dmg})"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_suicided_ok(),
        "suicided PlusFire honesty"
    );
    assert!(
        game_logic.honesty_demo_suicide_bomb_plus_fire_ok(),
        "PlusFire + CommandSetUpgrade host path honesty"
    );
    assert_eq!(
        game_logic.demo_suicide_bomb().death_detonations,
        0,
        "normal DestroyedWeapon path must not fire on SUICIDED residual"
    );
    assert!(
        game_logic.demo_suicide_bomb().suicided_detonations >= 1,
        "PlusFire detonation counter"
    );
}

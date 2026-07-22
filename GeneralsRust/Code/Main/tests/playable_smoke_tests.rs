use generals_main::command_system::{
    CommandType, GameCommand, ModifierKeys, PowerTarget, SpecialPowerType,
};
use generals_main::game_logic::{
    AIState, GameLogic, GameMode, KindOf, ObjectId, Player, Team, ThingTemplate, VictoryCondition,
    Weapon,
};
use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use glam::Vec3;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

fn command(
    command_id: u32,
    player_id: u32,
    command_type: CommandType,
    selected_units: Vec<ObjectId>,
) -> GameCommand {
    GameCommand {
        command_type,
        player_id,
        command_id,
        timestamp: UNIX_EPOCH + Duration::from_secs(command_id as u64),
        selected_units,
        modifier_keys: ModifierKeys::default(),
    }
}

fn template(
    name: &str,
    kind_of: &[KindOf],
    health: f32,
    supplies: u32,
    build_time: f32,
) -> ThingTemplate {
    let mut template = ThingTemplate::new(name);
    template.set_health(health);
    template.set_cost(supplies, 0);
    template.build_time = build_time;
    for kind in kind_of {
        template.add_kind_of(*kind);
    }
    template
}

fn install_smoke_templates(game_logic: &mut GameLogic) {
    let templates = [
        template(
            "SmokeCommandCenter",
            &[KindOf::Structure, KindOf::Selectable, KindOf::CommandCenter],
            2000.0,
            2000,
            0.1,
        ),
        template(
            "SmokePowerPlant",
            &[KindOf::Structure, KindOf::Selectable],
            800.0,
            800,
            0.1,
        ),
        template(
            "SmokeDozer",
            &[KindOf::Vehicle, KindOf::Worker, KindOf::Selectable],
            300.0,
            1000,
            0.1,
        ),
        template(
            "SmokeBarracks",
            &[KindOf::Structure, KindOf::Selectable],
            1000.0,
            500,
            0.1,
        ),
        template(
            "SmokeRanger",
            &[KindOf::Infantry, KindOf::Selectable, KindOf::Attackable],
            120.0,
            100,
            0.05,
        ),
        template(
            "SmokeSupplyDock",
            &[KindOf::Resource, KindOf::Harvestable],
            1000.0,
            0,
            0.1,
        ),
    ];

    for template in templates {
        game_logic.templates.insert(template.name.clone(), template);
    }
}

fn run_frames(game_logic: &mut GameLogic, frames: usize) {
    for _ in 0..frames {
        game_logic.update();
    }
}

fn run_until<F>(game_logic: &mut GameLogic, max_frames: usize, mut condition: F) -> bool
where
    F: FnMut(&GameLogic) -> bool,
{
    for _ in 0..max_frames {
        if condition(game_logic) {
            return true;
        }
        game_logic.update();
    }
    condition(game_logic)
}

fn smoke_save_info(filename: &str) -> SaveGameInfo {
    SaveGameInfo {
        filename: filename.to_string(),
        display_name: "Playable Smoke Save".to_string(),
        description: "Mini skirmish smoke test round trip".to_string(),
        map_name: "SmokeTestMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(12),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

fn opposing_team(team: Team) -> Team {
    match team {
        Team::USA => Team::China,
        Team::China => Team::GLA,
        Team::GLA => Team::USA,
        Team::Neutral => Team::USA,
    }
}

fn run_basic_faction_flow(human_team: Team) {
    let enemy_team = opposing_team(human_team);
    let mut game_logic = GameLogic::new();
    game_logic.start_new_game(GameMode::Skirmish);
    game_logic.clear_all_players();
    install_smoke_templates(&mut game_logic);
    game_logic.add_player(Player::new(0, human_team, human_team.get_name(), true));
    game_logic.add_player(Player::new(1, enemy_team, enemy_team.get_name(), false));

    let _command_center = game_logic
        .create_object("SmokeCommandCenter", human_team, Vec3::ZERO)
        .expect("human command center should spawn");
    let power_plant = game_logic
        .create_object("SmokePowerPlant", human_team, Vec3::new(-24.0, 0.0, 0.0))
        .expect("human power plant should spawn");
    let dozer = game_logic
        .create_object("SmokeDozer", human_team, Vec3::new(70.0, 0.0, 0.0))
        .expect("human dozer should spawn");
    let supply_dock = game_logic
        .create_object("SmokeSupplyDock", Team::Neutral, Vec3::new(40.0, 0.0, 0.0))
        .expect("neutral supply dock should spawn");
    let enemy_command_center = game_logic
        .create_object("SmokeCommandCenter", enemy_team, Vec3::new(160.0, 0.0, 0.0))
        .expect("enemy command center should spawn");

    game_logic.queue_command(command(
        10,
        0,
        CommandType::DozerConstruct {
            template_name: "SmokeBarracks".to_string(),
            location: Vec3::new(80.0, 0.0, 0.0),
            orientation: 0.0,
        },
        vec![dozer],
    ));
    game_logic.process_commands();
    let barracks_pending = game_logic
        .get_objects()
        .values()
        .any(|o| o.template_name == "SmokeBarracks");
    assert!(
        barracks_pending,
        "DozerConstruct must place SmokeBarracks under construction"
    );

    assert!(
        run_until(&mut game_logic, 90, |game_logic| game_logic
            .get_objects()
            .values()
            .any(|object| object.template_name == "SmokeBarracks"
                && object.team == human_team
                && object.is_constructed())),
        "{} barracks should finish construction",
        human_team.get_name()
    );

    let barracks_id = game_logic
        .get_objects()
        .values()
        .find(|object| {
            object.template_name == "SmokeBarracks"
                && object.team == human_team
                && object.is_constructed()
        })
        .map(|object| object.id)
        .expect("dozer construct command should create a faction-owned barracks");

    game_logic.queue_command(command(
        13,
        0,
        CommandType::Gather {
            target_id: supply_dock,
        },
        vec![dozer],
    ));
    game_logic.process_commands();
    let dozer_state = game_logic
        .get_object(dozer)
        .expect("dozer should exist after gather command");
    assert_eq!(
        dozer_state.ai_state,
        AIState::Gathering,
        "{} worker should accept gather commands from player slot 0",
        human_team.get_name()
    );
    assert_eq!(dozer_state.target, Some(supply_dock));

    game_logic.queue_command(command(
        11,
        0,
        CommandType::QueueUnitCreate {
            template_name: "SmokeRanger".to_string(),
            quantity: 1,
        },
        vec![barracks_id],
    ));
    assert!(
        run_until(&mut game_logic, 90, |game_logic| game_logic
            .get_objects()
            .values()
            .any(
                |object| object.template_name == "SmokeRanger" && object.team == human_team
            )),
        "{} infantry should finish production",
        human_team.get_name()
    );

    let ranger_id = game_logic
        .get_objects()
        .values()
        .find(|object| object.template_name == "SmokeRanger" && object.team == human_team)
        .map(|object| object.id)
        .expect("barracks production should spawn faction-owned infantry");
    {
        let ranger = game_logic
            .get_object_mut(ranger_id)
            .expect("infantry should exist before attack");
        ranger.weapon = Some(Weapon {
            damage: 60.0,
            range: 200.0,
            reload_time: 0.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        });
    }

    let supplies_before_sell = game_logic
        .get_player(0)
        .expect("human player should exist before sell")
        .resources
        .supplies;
    game_logic.queue_command(command(
        14,
        0,
        CommandType::Sell {
            object_id: power_plant,
        },
        Vec::new(),
    ));
    assert!(
        run_until(&mut game_logic, 120, |g| {
            g.get_player(0)
                .map(|p| p.resources.supplies > supplies_before_sell)
                .unwrap_or(false)
        }),
        "{} player in slot 0 should refund supplies on multi-frame sell",
        human_team.get_name()
    );

    let enemy_health_before = game_logic
        .get_object(enemy_command_center)
        .expect("enemy command center should exist")
        .health
        .current;
    game_logic.queue_command(command(
        12,
        0,
        CommandType::AttackObject {
            target_id: enemy_command_center,
        },
        vec![ranger_id],
    ));
    assert!(
        run_until(&mut game_logic, 180, |g| {
            g.get_object(enemy_command_center)
                .map(|o| o.health.current < enemy_health_before)
                .unwrap_or(false)
        }),
        "{} infantry should damage the enemy command center",
        human_team.get_name()
    );

    game_logic
        .get_object_mut(enemy_command_center)
        .expect("enemy command center should exist before defeat")
        .status
        .destroyed = true;
    let victory = game_logic.evaluate_victory_condition();
    assert_eq!(
        victory,
        Some(VictoryCondition::Winner(0)),
        "{} should win after eliminating the enemy command center",
        human_team.get_name()
    );
}

#[test]
fn mini_skirmish_playable_flow_smoke() {
    let mut game_logic = GameLogic::new();
    game_logic.start_new_game(GameMode::Skirmish);
    game_logic.clear_all_players();
    install_smoke_templates(&mut game_logic);
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    game_logic.add_player(Player::new(1, Team::China, "China", false));

    let command_center = game_logic
        .create_object("SmokeCommandCenter", Team::USA, Vec3::ZERO)
        .expect("USA command center should spawn");
    let _power_plant = game_logic
        .create_object("SmokePowerPlant", Team::USA, Vec3::new(-24.0, 0.0, 0.0))
        .expect("USA power plant should spawn");
    let dozer = game_logic
        .create_object("SmokeDozer", Team::USA, Vec3::new(70.0, 0.0, 0.0))
        .expect("USA dozer should spawn");
    let supply_dock = game_logic
        .create_object("SmokeSupplyDock", Team::Neutral, Vec3::new(40.0, 0.0, 0.0))
        .expect("neutral supply dock should spawn");
    let enemy_command_center = game_logic
        .create_object(
            "SmokeCommandCenter",
            Team::China,
            Vec3::new(160.0, 0.0, 0.0),
        )
        .expect("China command center should spawn");

    let starting_supplies = game_logic
        .get_player(0)
        .expect("USA player should exist")
        .resources
        .supplies;

    game_logic.queue_command(command(
        1,
        0,
        CommandType::DozerConstruct {
            template_name: "SmokeBarracks".to_string(),
            location: Vec3::new(80.0, 0.0, 0.0),
            orientation: 0.0,
        },
        vec![dozer],
    ));
    assert!(run_until(&mut game_logic, 90, |game_logic| game_logic
        .get_objects()
        .values()
        .any(
            |object| object.template_name == "SmokeBarracks" && object.is_constructed()
        )));

    let barracks = game_logic
        .get_objects()
        .values()
        .find(|object| object.template_name == "SmokeBarracks")
        .expect("dozer construct command should create barracks");
    assert!(barracks.is_constructed());
    let barracks_id = barracks.id;

    let after_barracks_supplies = game_logic
        .get_player(0)
        .expect("USA player should exist")
        .resources
        .supplies;
    assert!(
        after_barracks_supplies <= starting_supplies - 500 + 1,
        "barracks construction should charge its build cost"
    );

    game_logic.queue_command(command(
        2,
        0,
        CommandType::QueueUnitCreate {
            template_name: "SmokeRanger".to_string(),
            quantity: 1,
        },
        vec![barracks_id],
    ));
    assert!(run_until(&mut game_logic, 90, |game_logic| game_logic
        .get_objects()
        .values()
        .any(
            |object| object.template_name == "SmokeRanger" && object.team == Team::USA
        )));

    let ranger_id = game_logic
        .get_objects()
        .values()
        .find(|object| object.template_name == "SmokeRanger" && object.team == Team::USA)
        .map(|object| object.id)
        .expect("barracks production should spawn a ranger");
    let after_ranger_supplies = game_logic
        .get_player(0)
        .expect("USA player should exist")
        .resources
        .supplies;
    assert!(
        after_ranger_supplies <= after_barracks_supplies - 100 + 1,
        "ranger production should charge its build cost"
    );

    game_logic.queue_command(command(
        7,
        0,
        CommandType::Sell {
            object_id: ranger_id,
        },
        Vec::new(),
    ));
    game_logic.process_commands();
    assert_eq!(
        game_logic
            .get_player(0)
            .expect("USA player should exist after rejected unit sell")
            .resources
            .supplies,
        after_ranger_supplies,
        "sell should not refund non-structure objects"
    );
    assert!(
        game_logic
            .get_object(ranger_id)
            .expect("ranger should still exist after rejected sell")
            .is_alive(),
        "sell should not destroy non-structure objects"
    );

    game_logic.queue_command(command(
        3,
        0,
        CommandType::Gather {
            target_id: supply_dock,
        },
        vec![dozer],
    ));
    game_logic.process_commands();
    let dozer_state = game_logic
        .get_object(dozer)
        .expect("dozer should exist after gather command");
    assert_eq!(dozer_state.ai_state, AIState::Gathering);
    assert_eq!(dozer_state.target, Some(supply_dock));

    game_logic.queue_command(command(
        4,
        0,
        CommandType::DoSpecialPower {
            power_type: SpecialPowerType::RadarScan,
            target: PowerTarget::None,
        },
        vec![command_center],
    ));
    game_logic.process_commands();
    let command_center_state = game_logic
        .get_object(command_center)
        .expect("command center should exist after special power command");
    assert_eq!(command_center_state.ai_state, AIState::SpecialAbility);
    assert!(!command_center_state.special_power_ready);

    {
        let ranger = game_logic
            .get_object_mut(ranger_id)
            .expect("ranger should exist before attack");
        ranger.weapon = Some(Weapon {
            damage: 60.0,
            range: 200.0,
            reload_time: 0.0,
            projectile_speed: 0.0,
            ..Weapon::default()
        });
    }

    let save_dir = TempDir::new().expect("smoke save temp dir should be created");
    let mut save_manager = SaveFileManager::with_save_directory(save_dir.path());
    save_manager
        .init()
        .expect("smoke save manager should initialize");
    let save_info = smoke_save_info("mini_skirmish_smoke");
    save_manager
        .save_game("mini_skirmish_smoke", &game_logic, &save_info)
        .expect("mini skirmish should save");

    let mut loaded_game_logic = GameLogic::new();
    install_smoke_templates(&mut loaded_game_logic);
    let loaded_info = save_manager
        .load_game("mini_skirmish_smoke", &mut loaded_game_logic)
        .expect("mini skirmish should load");
    assert_eq!(loaded_info.display_name, save_info.display_name);
    assert_eq!(
        loaded_game_logic
            .get_player(0)
            .expect("loaded USA player should exist")
            .resources
            .supplies,
        after_ranger_supplies
    );
    let loaded_command_center = loaded_game_logic
        .get_object(command_center)
        .expect("loaded command center should exist");
    assert_eq!(loaded_command_center.ai_state, AIState::SpecialAbility);
    assert!(!loaded_command_center.special_power_ready);
    assert!(loaded_command_center.special_power_cooldown_remaining > 0.0);
    assert_eq!(
        loaded_game_logic
            .get_object(dozer)
            .expect("loaded dozer should exist")
            .target,
        Some(supply_dock)
    );
    assert!(loaded_game_logic
        .get_object(ranger_id)
        .expect("loaded ranger should exist")
        .weapon
        .is_some());

    let mut game_logic = loaded_game_logic;
    // load_game rebinds host skirmish AI for non-local players. Pause it so the
    // post-load combat frames do not spawn extra China objects and obscure the
    // intentional single-CC defeat path (AI depth is covered by ai_skirmish_gate).
    game_logic.set_ai_active(1, false);

    let enemy_health_before = game_logic
        .get_object(enemy_command_center)
        .expect("enemy command center should exist")
        .health
        .current;

    game_logic.queue_command(command(
        5,
        0,
        CommandType::AttackObject {
            target_id: enemy_command_center,
        },
        vec![ranger_id],
    ));
    assert!(
        run_until(&mut game_logic, 120, |g| {
            g.get_object(enemy_command_center)
                .map(|o| o.health.current < enemy_health_before)
                .unwrap_or(false)
        }),
        "post-load ranger attack should damage enemy CC"
    );

    // Eliminate every living China object (fail-closed if AI left residuals).
    let china_ids: Vec<_> = game_logic
        .get_objects()
        .values()
        .filter(|o| o.team == Team::China && o.is_alive())
        .map(|o| o.id)
        .collect();
    assert!(
        china_ids.contains(&enemy_command_center),
        "enemy command center should still be among living China objects before defeat"
    );
    for id in china_ids {
        if let Some(obj) = game_logic.get_object_mut(id) {
            obj.status.destroyed = true;
        }
    }
    let victory = game_logic.evaluate_victory_condition();
    assert_eq!(victory, Some(VictoryCondition::Winner(0)));

    let summary = game_logic.build_victory_summary(Some(0));
    assert!(summary.player_results.len() >= 2);
    assert!(summary
        .player_results
        .iter()
        .any(|result| result.player_id == 0
            && result.outcome == generals_main::game_logic::PlayerOutcome::Won));
}

#[test]
fn retail_factions_build_train_attack_smoke() {
    for human_team in [Team::USA, Team::China, Team::GLA] {
        run_basic_faction_flow(human_team);
    }
}

#[test]
fn dozer_construct_places_barracks_under_construction() {
    use generals_main::game_logic::host_production_buildable_command_residual::LBC_OK;
    let mut game_logic = GameLogic::new();
    game_logic.start_new_game(GameMode::Skirmish);
    game_logic.clear_all_players();
    install_smoke_templates(&mut game_logic);
    game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    let dozer = game_logic
        .create_object("SmokeDozer", Team::USA, Vec3::new(70.0, 0.0, 0.0))
        .expect("dozer");
    let cc = game_logic
        .create_object("SmokeCommandCenter", Team::USA, Vec3::ZERO)
        .expect("cc");
    let _ = cc;
    let loc = Vec3::new(80.0, 0.0, 0.0);
    let code =
        game_logic.legal_build_code_at_for_builder(Team::USA, loc, "SmokeBarracks", Some(dozer));
    assert_eq!(code, LBC_OK, "legal build code {code}");
    let d = game_logic.get_object(dozer).expect("dozer obj");
    assert!(d.can_construct(), "dozer must can_construct");
    assert!(d.is_worker());
    assert!(d.can_move());
    game_logic.queue_command(command(
        1,
        0,
        CommandType::DozerConstruct {
            template_name: "SmokeBarracks".to_string(),
            location: loc,
            orientation: 0.0,
        },
        vec![dozer],
    ));
    game_logic.process_commands();
    assert!(
        game_logic
            .get_objects()
            .values()
            .any(|o| o.template_name == "SmokeBarracks"),
        "barracks must exist after DozerConstruct; supplies={}",
        game_logic
            .get_player(0)
            .map(|p| p.resources.supplies)
            .unwrap_or(0)
    );
    let supplies = game_logic.get_player(0).unwrap().resources.supplies;
    assert!(
        supplies <= 10000 - 500,
        "must charge 500 for barracks, supplies={supplies}"
    );
}

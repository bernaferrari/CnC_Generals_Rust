use generals_main::command_system::{
    CommandType, GameCommand, ModifierKeys, PowerTarget, SpecialPowerType,
};
use generals_main::game_logic::{
    AIState, GameLogic, GameMode, KindOf, ObjectId, Player, Team, ThingTemplate, VictoryCondition,
    Weapon,
};
use glam::Vec3;
use std::time::{Duration, UNIX_EPOCH};

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

#[test]
fn mini_skirmish_playable_flow_smoke() {
    let mut game_logic = GameLogic::new();
    game_logic.start_new_game(GameMode::Skirmish);
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
        .create_object("SmokeDozer", Team::USA, Vec3::new(12.0, 0.0, 0.0))
        .expect("USA dozer should spawn");
    let supply_dock = game_logic
        .create_object("SmokeSupplyDock", Team::Neutral, Vec3::new(40.0, 0.0, 0.0))
        .expect("neutral supply dock should spawn");
    let enemy_command_center = game_logic
        .create_object("SmokeCommandCenter", Team::China, Vec3::new(80.0, 0.0, 0.0))
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
            location: Vec3::new(20.0, 0.0, 0.0),
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
    run_frames(&mut game_logic, 2);
    let enemy_health_after = game_logic
        .get_object(enemy_command_center)
        .expect("enemy command center should still exist")
        .health
        .current;
    assert!(enemy_health_after < enemy_health_before);

    game_logic
        .get_object_mut(enemy_command_center)
        .expect("enemy command center should exist before defeat")
        .status
        .destroyed = true;
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

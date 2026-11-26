use super::{HeadlessSkirmish, HumanOrders, SharedSkirmishAssets};
use crate::save_load::SaveFileManager;
use generals_sim::dozer::{ActionState, DozerBuildSubTask, DozerTask};

#[test]
#[ignore = "requires retail assets"]
fn authored_power_plant_resumes_from_a_skirmish_save_file() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(check_power_plant_save)
        .unwrap()
        .join()
        .unwrap();
}

fn check_power_plant_save() {
    let assets = SharedSkirmishAssets::load_retail().expect("retail skirmish assets");
    let seed = 1234;
    let human = HumanOrders::PLAYER;
    let plant = HumanOrders::STRUCTURE;
    let mut game = HeadlessSkirmish::start(&assets, seed).unwrap();
    let dozer = game.owned_ids(human, HumanOrders::PRODUCED_UNIT)[0];
    game.issue_human_orders().unwrap();
    let (site, progress) = (0..900)
        .find_map(|_| {
            game.advance();
            let sites = game.construction(human, plant);
            match sites.as_slice() {
                &[(site, true, progress)] if progress > 0.0 => Some((site, progress)),
                _ => None,
            }
        })
        .expect("the retail Dozer must start the authored power plant");
    let builder = game.logic.host_object(dozer).unwrap();
    assert_eq!(
        builder.dozer_tasks.build_sub_task,
        DozerBuildSubTask::DoBuildAtDock
    );
    let dock = builder.dozer_dock_action().unwrap();
    let action = &builder.dozer_tasks.primary.actions[DozerTask::Build as usize];
    let ActionState::DoAction { enter_frame } = action.state else {
        panic!("the saved Dozer must already be building");
    };
    assert_eq!(action.control.goal_object, site.0);
    let scaffold = game.logic.host_object(site).unwrap();
    let health = scaffold.health.current;
    let plant_output = scaffold.get_template().energy_production.unwrap();
    assert!(plant_output > 0);
    let production =
        |world: &HeadlessSkirmish| world.logic.get_player(human).unwrap().power_produced;
    let saved_power = production(&game);
    let saved_frame = game.frame();
    let saved_observation = game.observe();
    let neutral = game
        .logic
        .get_players()
        .values()
        .find(|player| player.is_reserved_neutral())
        .expect("retail Neutral player");
    let neutral_id = neutral.id;
    let neutral_power = neutral.power_produced;
    let saved_passability = game.logic.snapshot_pathfinding_passability();
    let gla = 1;
    let saved_command_center = game.owned_ids(gla, "GLACommandCenter");
    assert_eq!(saved_command_center.len(), 1);

    let directory = tempfile::tempdir().unwrap();
    let mut archive = SaveFileManager::with_save_directory(directory.path());
    archive.init().unwrap();
    game.logic
        .with_own_services(|| archive.quick_save(&game.logic))
        .expect("write the active skirmish to a save file");

    let mut receiving = HeadlessSkirmish::start(&assets, seed).unwrap();
    receiving
        .logic
        .with_own_services_mut(|logic| archive.load_game("quicksave", logic))
        .expect("load the active skirmish through the staged archive path");
    assert_eq!(receiving.frame(), saved_frame);
    assert_eq!(
        receiving.construction(human, plant),
        [(site, true, progress)]
    );
    let restored = receiving.logic.host_object(site).unwrap();
    assert_eq!(restored.builder_id, Some(dozer));
    assert_eq!(restored.health.current, health);
    assert_eq!(
        receiving
            .logic
            .host_object(dozer)
            .unwrap()
            .dozer_dock_action(),
        Some(dock)
    );
    assert_eq!(production(&receiving), saved_power);
    assert_eq!(
        receiving
            .logic
            .get_player(neutral_id)
            .unwrap()
            .power_produced,
        neutral_power,
        "loading an unfinished scaffold must not grant Neutral power"
    );
    assert!(
        receiving.logic.snapshot_pathfinding_passability() == saved_passability,
        "map reclassification must restore the saved structures' pathfinding footprints"
    );
    assert_eq!(receiving.observe(), saved_observation);

    let restored_builder = receiving.logic.host_object(dozer).unwrap();
    let restored_action = &restored_builder.dozer_tasks.primary.actions[DozerTask::Build as usize];
    assert_eq!(restored_action.control.goal_object, site.0);
    assert!(matches!(
        restored_action.state,
        ActionState::DoAction { enter_frame: restored_frame } if restored_frame == enter_frame
    ));
    receiving.advance();
    assert_eq!(game.frame(), saved_frame);
    assert_eq!(game.construction(human, plant), [(site, true, progress)]);
    assert_eq!(production(&game), saved_power);
    game.advance();
    assert_eq!(
        receiving.owned_ids(gla, "GLACommandCenter"),
        saved_command_center,
        "restoring a match must not repeat the skirmish AI's map initialization"
    );
    assert_eq!(
        receiving.construction(human, plant),
        game.construction(human, plant)
    );
    let next_health = game.logic.host_object(site).unwrap().health.current;
    assert!(
        next_health > health,
        "the uninterrupted builder contributes on this tick"
    );
    assert_eq!(
        receiving.logic.host_object(site).unwrap().health.current,
        next_health
    );
    assert_eq!(
        receiving.observe(),
        game.observe(),
        "first resumed logic tick"
    );

    let finished = (0..900).any(|_| {
        receiving.advance();
        game.advance();
        compare_tick(&game, &receiving);
        let sites = receiving.construction(human, plant);
        assert_eq!(
            sites,
            game.construction(human, plant),
            "logic frame {}",
            game.frame()
        );
        if sites == [(site, false, 1.0)] {
            return true;
        }
        assert_eq!(production(&receiving), saved_power);
        false
    });
    assert!(
        finished,
        "the restored Dozer must finish without another order"
    );
    for _ in 0..30 {
        receiving.advance();
        game.advance();
        compare_tick(&game, &receiving);
    }
    assert_eq!(production(&receiving), saved_power + plant_output);
}

fn compare_tick(source: &HeadlessSkirmish, receiving: &HeadlessSkirmish) {
    let expected = source.observe();
    let actual = receiving.observe();
    if actual != expected {
        let actors = |world: &HeadlessSkirmish| {
            world
                .logic
                .host_objects()
                .registered_values()
                .map(|actor| {
                    (
                        actor.id,
                        (
                            actor.template_name.clone(),
                            actor.get_position(),
                            actor.health.current,
                            actor.health.maximum,
                            actor.owner_player_id,
                            actor.ai_state().clone(),
                        ),
                    )
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let source_actors = actors(source);
        let receiving_actors = actors(receiving);
        for (id, actor) in &source_actors {
            if receiving_actors.get(id) != Some(actor) {
                eprintln!(
                    "{id:?} source {actor:?}; restored {:?}",
                    receiving_actors.get(id)
                );
            }
        }
        for (id, actor) in &receiving_actors {
            if !source_actors.contains_key(id) {
                eprintln!("{id:?} newly restored {actor:?}");
            }
        }
    }
    assert_eq!(
        actual, expected,
        "first differing logic frame {}",
        expected.frame
    );
}

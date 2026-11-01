//! The UI projection keeps C++ scoring rules and never reads another world.
pub use generals_main::game_logic;
#[path = "../src/cnc_game_engine/observer_counts.rs"]
mod observer_counts;

use game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

fn populate(world: &mut GameLogic, player: u32, name: &str, kinds: &[KindOf]) {
    let mut template = ThingTemplate::new(name);
    for kind in kinds {
        template.add_kind_of(*kind);
    }
    world.templates.insert(name.to_string(), template);
    world
        .create_object_for_player(name, player, Vec3::ZERO)
        .unwrap();
}

fn original_counts(world: &GameLogic, player: u32) -> observer_counts::ObserverCounts {
    let count = |required: u128, forbidden: u128| {
        world
            .get_objects()
            .registered_values()
            .filter(|object| {
                object
                    .owner_player_id
                    .filter(|id| world.get_player(*id).is_some())
                    == Some(player)
                    && object.is_kind_of_multi(required, forbidden)
            })
            .count() as i32
    };
    const SCORE: u128 = 1 << 35;
    const STRUCTURE: u128 = 1 << 7;
    const CREATE: u128 = 1 << 36;
    const DESTROY: u128 = 1 << 37;
    observer_counts::ObserverCounts {
        units: count(SCORE, STRUCTURE),
        buildings: count(SCORE | STRUCTURE, 0)
            + count(CREATE | STRUCTURE, 0)
            + count(DESTROY | STRUCTURE, 0),
    }
}

#[test]
fn one_pass_matches_original_queries_and_isolates_worlds() {
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    for world in [&mut first, &mut second] {
        world.add_player(Player::new(0, Team::USA, "First", true));
        world.add_player(Player::new(1, Team::China, "Second", false));
    }
    populate(&mut first, 0, "ScoredUnit", &[KindOf::Score]);
    populate(
        &mut first,
        0,
        "AllScoreBuilding",
        &[
            KindOf::Structure,
            KindOf::Score,
            KindOf::ScoreCreate,
            KindOf::ScoreDestroy,
        ],
    );
    populate(
        &mut first,
        1,
        "CreatedBuilding",
        &[KindOf::Structure, KindOf::ScoreCreate],
    );
    populate(&mut first, 1, "Unscored", &[]);
    populate(&mut first, 1, "RetiredOwner", &[KindOf::Score]);
    let orphan = first
        .get_objects()
        .registered_values()
        .find(|object| object.thing().template.name == "RetiredOwner")
        .unwrap()
        .id;
    first.host_object_mut(orphan).unwrap().owner_player_id = Some(999);
    populate(&mut second, 0, "OtherUnit", &[KindOf::Score]);
    populate(
        &mut second,
        1,
        "OtherBuilding",
        &[KindOf::Structure, KindOf::ScoreDestroy],
    );
    for world in [&first, &second] {
        let counts = observer_counts::count_observer_objects(world);
        for player in [0, 1] {
            assert_eq!(
                counts.get(&player).copied().unwrap_or_default(),
                original_counts(world, player)
            );
        }
    }
    let first_counts = observer_counts::count_observer_objects(&first);
    let second_counts = observer_counts::count_observer_objects(&second);
    assert!(!first_counts.contains_key(&999));
    assert_eq!(first_counts[&0].units, 1);
    assert_eq!(first_counts[&0].buildings, 3);
    assert_eq!(second_counts[&0].buildings, 0);
}

#[test]
#[ignore = "manual same-process roster benchmark"]
fn benchmark_observer_count_scans() {
    use std::{hint::black_box, time::Instant};
    let mut world = GameLogic::new();
    for player in 0..5 {
        world.add_player(Player::new(player, Team::USA, "Player", false));
    }
    let mut template = ThingTemplate::new("ScoredBenchmarkUnit");
    template.add_kind_of(KindOf::Score);
    world.templates.insert(template.name.clone(), template);
    for index in 0..500 {
        world
            .create_object_for_player("ScoredBenchmarkUnit", index % 5, Vec3::ZERO)
            .unwrap();
    }
    let rounds = 100;
    let started = Instant::now();
    for _ in 0..rounds {
        for player in 0..5 {
            black_box(original_counts(&world, player));
        }
    }
    let old = started.elapsed();
    let started = Instant::now();
    for _ in 0..rounds {
        black_box(observer_counts::count_observer_objects(&world));
    }
    let new = started.elapsed();
    println!(
        "observer counts: {rounds} rounds, 500 objects, 5 players: previous={old:?}, one_pass={new:?}"
    );
}

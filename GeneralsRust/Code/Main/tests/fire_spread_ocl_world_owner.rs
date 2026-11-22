use generals_main::game_logic::host_fire_spread::HostFlammableState;
use generals_main::game_logic::{GameLogic, GameMode, Team};
use glam::Vec3;

const SPREADER_INI: &str = "Object FlameSpreader\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = FlammableUpdate BurnTag\n FlameDamageLimit = 7\n FlameDamageExpiration = 2000\n BurnedDelay = 4500\n AflameDuration = 3500\n AflameDamageDelay = 500\n AflameDamageAmount = 5\n BurningSoundName = GenericFireMediumLoop\n End\n Behavior = FireSpreadUpdate SpreadTag\n MinSpreadDelay = 500\n MaxSpreadDelay = 500\n SpreadTryRange = 50\n OCLEmbers = CustomEmberList\n End\nEnd\n";

const KINDLING_INI: &str = "\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\n Behavior = FlammableUpdate BurnTag\n FlameDamageLimit = 2\n FlameDamageExpiration = 2000\n BurnedDelay = 2500\n AflameDuration = 3500\n AflameDamageDelay = 500\n AflameDamageAmount = 5\n BurningSoundName = GenericFireMediumLoop\n End\nEnd\n";

const EMBER_OBJECT_INI: &str = "Object EmberFixtureActor\n Body = ActiveBody BodyTag\n MaxHealth = 1\n InitialHealth = 1\n End\nEnd\n";

const EMBER_OCL_INI: &str = "ObjectCreationList CustomEmberList\n CreateObject\n ObjectNames = EmberFixtureActor\n Count = 1\n End\nEnd\n";

fn ignite_now(world: &mut GameLogic, id: generals_main::game_logic::ObjectId) {
    let fire = world
        .host_object_mut(id)
        .and_then(|object| object.fire_spread.as_mut())
        .expect("spread source admits FlammableUpdate and FireSpreadUpdate");
    fire.state = HostFlammableState::Aflame;
    fire.body_aflame = true;
    fire.aflame_end_frame = u32::MAX;
    fire.next_spread_frame = 0;
}

fn is_lit(world: &GameLogic, id: generals_main::game_logic::ObjectId) -> bool {
    world
        .host_object(id)
        .and_then(|object| object.fire_spread.as_ref().map(|fire| fire.is_aflame()))
        .unwrap_or(false)
}

#[test]
fn authored_ocl_embers_and_partition_tie_drive_world_spread() {
    let mut world = GameLogic::new();
    // Arbitrary names: nothing in this fixture keys off "tree" naming. The
    // decoy keeps a DogwoodTree-shaped name with no fire modules to prove the
    // old heuristic installs nothing.
    // C++ INI reads only the first name token for OCLEmbers; a trailing token must not corrupt it.
    let spreader = SPREADER_INI.replace("OCLEmbers = CustomEmberList", "OCLEmbers = CustomEmberList ignored_token");
    let ini = format!(
        "{spreader}Object KindlingWest{KINDLING_INI}Object KindlingEast{KINDLING_INI}Object DogwoodTreeHeuristic\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n{EMBER_OBJECT_INI}"
    );
    world
        .admit_object_ini_sources([("FireSpreadOwner.ini".into(), ini)])
        .unwrap();
    world
        .admit_object_creation_list_sources([EMBER_OCL_INI.to_string()])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer).unwrap();

    let source = world
        .create_object("FlameSpreader", Team::Neutral, Vec3::new(5.0, 0.0, 5.0))
        .expect("spread source");
    let first = world
        .create_object("KindlingWest", Team::Neutral, Vec3::new(7.0, 0.0, 5.0))
        .expect("earlier-registered equally distant candidate");
    let second = world
        .create_object("KindlingEast", Team::Neutral, Vec3::new(3.0, 0.0, 5.0))
        .expect("later-registered equally distant candidate");
    let decoy = world
        .create_object("DogwoodTreeHeuristic", Team::Neutral, Vec3::new(5.0, 0.0, 9.0))
        .expect("tree-named object without fire modules");

    // Authored module data — not the template name — configures the runtime.
    {
        let spread = world
            .host_object(source)
            .and_then(|object| object.fire_spread.as_ref())
            .expect("spread module installed from authored modules");
        assert!(spread.spread_enabled);
        assert_eq!(spread.min_spread_delay, 15, "500ms authored fixed delay");
        assert_eq!(spread.max_spread_delay, 15, "500ms authored fixed delay");
        assert_eq!(spread.spread_try_range, 50.0);
        assert_eq!(spread.ocl_embers, "CustomEmberList");
        assert_eq!(spread.burned_delay, 135, "4500ms authored BurnedDelay");
        assert_eq!(spread.flame_damage_limit, 7.0);
    }
    for id in [first, second] {
        let fire = world
            .host_object(id)
            .and_then(|object| object.fire_spread.as_ref())
            .expect("flammable-only neighbor installs flammable state");
        assert!(!fire.spread_enabled, "flammable-only neighbor cannot spread");
    }
    assert!(
        world.host_object(decoy).unwrap().fire_spread.is_none(),
        "DogwoodTree-shaped name without authored modules installs nothing"
    );

    ignite_now(&mut world, source);

    // Fixed-step the world past the first scheduled spread attempt (the
    // partition phase seeds cells on the first tick; the 15-frame authored
    // delay puts the first selection well inside the window).
    for _ in 0..18 {
        world.update_with_dt(1.0 / 30.0);
    }

    // C++ getClosestObject walks each cell COI head-first and keeps the
    // strictly closest allowed object (PartitionManager.cpp:3245-3315,
    // FASTER_GCO). Objects prepend to the cell COI when the dirty list drains
    // newest-first (prependToDirtyModules + friend_addToCellList), so for one
    // create batch the earliest-attached ends at the head and wins an
    // equal-distance tie.
    assert!(
        is_lit(&world, first),
        "head-of-cell equally distant candidate ignites"
    );
    assert!(
        !is_lit(&world, second),
        "the later-registered equally distant candidate loses the COI tie"
    );
    assert!(!is_lit(&world, decoy), "module-less tree name never ignites");
    assert!(
        world
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "EmberFixtureActor"),
        "authored CustomEmberList must birth a real EmberFixtureActor"
    );
}

#[test]
fn authored_range_excludes_candidate_at_exact_boundary() {
    let mut world = GameLogic::new();
    let ini = format!(
        "{SPREADER_INI}Object EdgeKindling{KINDLING_INI}Object InsideKindling{KINDLING_INI}{EMBER_OBJECT_INI}"
    );
    world
        .admit_object_ini_sources([("FireSpreadOwner.ini".into(), ini)])
        .unwrap();
    world
        .admit_object_creation_list_sources([EMBER_OCL_INI.to_string()])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer).unwrap();

    let source = world
        .create_object("FlameSpreader", Team::Neutral, Vec3::ZERO)
        .expect("spread source");
    let edge = world
        .create_object("EdgeKindling", Team::Neutral, Vec3::new(50.0, 0.0, 0.0))
        .expect("candidate exactly at spread_try_range");
    let inside = world
        .create_object("InsideKindling", Team::Neutral, Vec3::new(40.0, 0.0, 0.0))
        .expect("candidate strictly inside spread_try_range");

    ignite_now(&mut world, source);
    for _ in 0..35 {
        world.update_with_dt(1.0 / 30.0);
    }

    assert!(
        is_lit(&world, inside),
        "the strictly inside candidate ignites on a scheduled spread attempt"
    );
    assert!(
        !is_lit(&world, edge),
        "C++ getClosestObject keeps the strict radius: distance == range is excluded"
    );
    assert!(
        world
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == "EmberFixtureActor"),
        "due spread attempts still birth authored embers"
    );
}

#[test]
fn missing_receiving_ember_ocl_does_not_synthesize_a_gameplay_object() {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "FireSpreadOwner.ini".into(),
            "Object DogwoodTreeFireSpreadOclOwner\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = FlammableUpdate FlammableTag\n End\n Behavior = FireSpreadUpdate SpreadTag\n OCLEmbers = MissingList\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer).unwrap();

    let tree = world
        .create_object("DogwoodTreeFireSpreadOclOwner", Team::Neutral, Vec3::ZERO)
        .expect("create source tree");

    let fire = world
        .host_object_mut(tree)
        .and_then(|object| object.fire_spread.as_mut())
        .expect("tree admits FlammableUpdate and FireSpreadUpdate");
    fire.state = HostFlammableState::Aflame;
    fire.body_aflame = true;
    fire.aflame_end_frame = u32::MAX;
    fire.next_spread_frame = 0;
    fire.spread_try_range = 0.0;

    // The public fixed-step path reaches the real FireSpreadUpdate callback.
    // The authored OCLEmbers name has no admitted list, so C++
    // ObjectCreationList::create is a no-op; Main must not invent a generic
    // BurningEmbers actor or particle.
    world.update_with_dt(1.0 / 30.0);

    assert_eq!(world.host_objects().registered_len(), 1);
    assert!(
        !world
            .combat_particles()
            .active_systems()
            .any(|p| p.ocl_list_name == "MissingList"),
        "no synthesized particle may stand in for the missing authored list"
    );
    let source = world.host_object(tree).expect("source stays live");
    assert!(source.fire_spread.as_ref().unwrap().next_spread_frame > 0);
}

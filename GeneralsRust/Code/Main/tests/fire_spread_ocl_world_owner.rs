use generals_main::game_logic::host_fire_spread::HostFlammableState;
use generals_main::game_logic::{GameLogic, GameMode, Team, VeterancyLevel};
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
    let ember = EMBER_OBJECT_INI.replace(
        "Object EmberFixtureActor\n",
        "Object EmberFixtureActor\n IsTrainable = Yes\n ExperienceRequired = 0 10 100 1000\n",
    );
    let ini = format!(
        "{spreader}Object KindlingWest{KINDLING_INI}Object KindlingEast{KINDLING_INI}Object DogwoodTreeHeuristic\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n{ember}"
    );
    world
        .admit_object_ini_sources([("FireSpreadOwner.ini".into(), ini)])
        .unwrap();
    world
        .admit_object_creation_list_sources([EMBER_OCL_INI.replace(
            " Count = 1\n",
            " Count = 1\n InheritsVeterancy = Yes\n",
        )])
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

    world.host_object_mut(source).unwrap().experience.level = VeterancyLevel::Elite;
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
    let ember = world
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "EmberFixtureActor")
        .expect("authored CustomEmberList must birth a real EmberFixtureActor");
    assert_eq!(ember.experience.level, VeterancyLevel::Elite);
}

#[test]
fn authored_random_force_embers_launch_with_authored_physics() {
    // Stock OCL_BurningEmbers shape (retail ObjectCreationList.ini): RANDOM_FORCE
    // with authored offset/friction/bounce onto a PhysicsBehavior Mass=2 ember.
    const EMBER_PHYSICS_INI: &str = "Object EmberFixtureActor\n Body = ActiveBody BodyTag\n MaxHealth = 1\n InitialHealth = 1\n End\n Behavior = PhysicsBehavior PhysTag\n Mass = 2\n End\n Behavior = LifetimeUpdate LifeTag\n MinLifetime = 5000\n MaxLifetime = 10000\n End\nEnd\n";
    const EMBER_FORCE_OCL_INI: &str = "ObjectCreationList CustomEmberList\n CreateObject\n ObjectNames = EmberFixtureActor\n Count = 1\n Offset = X:0 Y:0 Z:20\n Disposition = RANDOM_FORCE\n MinForceMagnitude = 2\n MaxForceMagnitude = 4\n MinForcePitch = 60\n MaxForcePitch = 70\n ExtraBounciness = -1.0\n ExtraFriction = 30\n End\nEnd\n";

    let mut world = GameLogic::new();
    let ini = format!("{SPREADER_INI}{EMBER_PHYSICS_INI}");
    world
        .admit_object_ini_sources([("FireSpreadOwner.ini".into(), ini)])
        .unwrap();
    world
        .admit_object_creation_list_sources([EMBER_FORCE_OCL_INI.to_string()])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer).unwrap();

    let source = world
        .create_object("FlameSpreader", Team::Neutral, Vec3::new(5.0, 0.0, 5.0))
        .expect("spread source");
    ignite_now(&mut world, source);

    // One fixed step: the due spread attempt births the ember even with no
    // spread candidate (C++ ember OCL runs before candidate selection).
    world.update_with_dt(1.0 / 30.0);

    let ember = world
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == "EmberFixtureActor")
        .expect("RANDOM_FORCE CustomEmberList must birth a real EmberFixtureActor");
    assert_eq!(ember.producer_id, Some(source));
    let ember_position = ember.get_position();
    // The birth tick already integrates the launch impulse once, so the
    // authored Z:20 offset (host Y-up) is position minus that first step.
    assert!(
        (ember_position.y - ember.movement.velocity.y - 20.0).abs() < 0.15,
        "authored Z:20 offset maps to host Y +20, got y={} velocity.y={}",
        ember_position.y,
        ember.movement.velocity.y
    );
    assert_eq!(ember.physics_mass, 2.0, "authored PhysicsBehavior Mass");
    // parseFrictionPerSec stores ExtraFriction / 30 per frame.
    assert!(
        (ember.extra_friction - 1.0).abs() < 1e-6,
        "ExtraFriction = 30 parses to 1.0 per frame, got {}",
        ember.extra_friction
    );
    assert!(
        (ember.extra_bounciness - (-1.0)).abs() < 1e-6,
        "ExtraBounciness = -1.0 preserved, got {}",
        ember.extra_bounciness
    );
    // apply_physics_force stores force/mass in physics_accel; if the same
    // tick already integrated it, the impulse lives in velocity.y instead
    // (minus at most one frame of SHOCK_GRAVITY ≈ -0.071).
    let launch = ember.physics_accel.y + ember.movement.velocity.y;
    let min_launch = 2.0 * 60.0f32.to_radians().sin() / 2.0;
    let max_launch = 4.0 * 70.0f32.to_radians().sin() / 2.0;
    assert!(
        launch > min_launch - 0.1 && launch <= max_launch + 1e-3,
        "RANDOM_FORCE vertical impulse mag*sin(pitch)/mass expected in \
         [{min_launch}, {max_launch}], got {launch}"
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

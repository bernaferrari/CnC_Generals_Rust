use game_engine::common::random_value::SessionRandom;
use generals_main::game_logic::{GameLogic, Player, Team};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

const VICTIM: &str = "ReceivingEmpSparkVehicle";

fn world(seed: u32) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "emp-spark-rng-owner.ini".into(),
            format!(
                "Object {VICTIM}\n KindOf = VEHICLE SELECTABLE\n Geometry = BOX\n GeometryMajorRadius = 8\n GeometryMinorRadius = 5\n GeometryHeight = 12\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
            ),
        )])
        .expect("receiving EMP victim template");
    logic.add_player(Player::new(0, Team::USA, "Owner", true));
    let victim = logic
        .create_object_for_player(VICTIM, 0, Vec3::new(10.0, 0.0, 10.0))
        .expect("victim");

    let builder = SnapshotBuilder::new();
    let mut baseline = builder.create_world_snapshot(&logic).unwrap();
    let random = SessionRandom::default();
    random.seed_random(seed);
    baseline.random_seed = u64::from(seed);
    baseline.logic_rng_seed_words = random.seed_words();
    builder
        .restore_from_snapshot(&baseline, &mut logic)
        .unwrap();
    (logic, victim)
}

#[test]
fn interleaved_emp_sparks_use_receiving_rng_and_particle_registry() {
    let (mut first, first_victim) = world(0xA11C_E001);
    let (mut second, second_victim) = world(0xB22D_F002);
    assert_eq!(
        first_victim, second_victim,
        "worlds reuse the same object ID"
    );

    let before_first = SnapshotBuilder::new()
        .create_world_snapshot(&first)
        .unwrap()
        .logic_rng_seed_words;
    let second_before = SnapshotBuilder::new()
        .create_world_snapshot(&second)
        .unwrap()
        .logic_rng_seed_words;
    let victim = second.host_object(second_victim).unwrap();
    let geom = second.templates[VICTIM].geometry_info;
    let height = if geom.height > 0.0 {
        geom.height
    } else {
        victim.selection_radius.max(1.0)
    };
    let footprint =
        generals_main::game_logic::host_hero_abilities::leftover_disable_fx_footprint_area(
            geom.authored,
            geom.geom_type as u32,
            geom.major_radius,
            geom.minor_radius,
            victim.selection_radius,
        );
    let emitter_count = generals_main::game_logic::host_emp_pulse::leftover_emp_spark_emitter_count(
        footprint, height,
    );
    let expected_random = SessionRandom::default();
    expected_random.seed_random(0xB22D_F002);
    for _ in 0..emitter_count {
        let _ =
            generals_main::game_logic::host_hero_abilities::leftover_disable_fx_footprint_offset(
                &geom,
                &expected_random,
            );
        let _ = generals_main::game_logic::host_emp_pulse::leftover_emp_spark_z(
            height,
            &expected_random,
        );
        // No receiving native particle manager is supplied by this headless
        // public path, so C++'s initial-delay draw is correctly absent too.
    }

    let ambient = SessionRandom::default();
    ambient.seed_random(0xC33E_0003);
    let ambient_words = ambient.seed_words();
    game_engine::common::random_value::set_game_logic_random_seed_state(ambient_words);

    assert!(second.apply_emp_pulse_disable_field_at(0, Vec3::new(10.0, 0.0, 10.0), None,));

    let second_after = SnapshotBuilder::new()
        .create_world_snapshot(&second)
        .unwrap()
        .logic_rng_seed_words;
    assert_ne!(
        second_after, second_before,
        "the recipient consumes spark draws"
    );
    assert_eq!(
        second_after,
        expected_random.seed_words(),
        "draw count and order match the authored offset + height draws"
    );
    assert_eq!(
        SnapshotBuilder::new()
            .create_world_snapshot(&first)
            .unwrap()
            .logic_rng_seed_words,
        before_first,
        "a peer's EMP callback leaves the first world's RNG untouched"
    );
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        ambient_words,
        "the receiving EMP callback does not draw from ambient process RNG"
    );
    assert_eq!(
        first.combat_particles().active_systems().count(),
        0,
        "the peer registry receives no spark systems"
    );
    assert_eq!(
        second.combat_particles().active_systems().count(),
        15,
        "the receiving registry records the authored minimum emitter count"
    );
}

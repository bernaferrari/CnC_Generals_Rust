use generals_main::game_logic::{GameLogic, Player, Team};

fn world(
    admit_ocl: bool,
    die_list: &str,
    children: &[&str],
    count: u32,
) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut game = GameLogic::new();
    if admit_ocl {
        game.admit_object_creation_list_sources([format!(
            "ObjectCreationList {die_list}\nCreateObject\nObjectNames = {}\nCount = {count}\nEnd\nEnd\n",
            children.join(" ")
        )])
        .unwrap();
    }
    let child_definitions = children
        .iter()
        .map(|child| {
            format!(
                "Object {child}\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
            )
        })
        .collect::<String>();
    let objects = format!(
        "Object SharedDieVictim\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = CreateObjectDie Replacement\nCreationList = {die_list}\nEnd\nEnd\n{child_definitions}"
    );
    game.admit_object_ini_sources([("Object.ini".into(), objects)])
        .unwrap();
    game.add_player(Player::new(7, Team::USA, "Owner", true));
    let victim = game
        .create_object_for_player("SharedDieVictim", 7, glam::Vec3::ZERO)
        .unwrap();
    (game, victim)
}

fn spawned_templates(game: &GameLogic) -> Vec<String> {
    game.host_objects()
        .registered_values()
        .filter(|object| object.template_name != "SharedDieVictim")
        .map(|object| object.template_name.clone())
        .collect()
}

#[test]
fn same_named_create_object_die_ocl_uses_receiving_world_and_selects_one_candidate() {
    let (mut first, first_id) = world(true, "SharedDieList", &["FirstChildA", "FirstChildB"], 1);
    let (mut second, second_id) =
        world(true, "SharedDieList", &["SecondChildA", "SecondChildB"], 1);
    assert_eq!(first_id, second_id);

    // Each C++ onDie-equivalent resolves the same OCL identity against its
    // own immutable catalog. Count=1 chooses one candidate, never both.
    first.apply_host_damage(first_id, 1000.0);
    let first_children = spawned_templates(&first);
    assert_eq!(first_children.len(), 1);
    assert!(first_children[0].starts_with("FirstChild"));
    assert!(spawned_templates(&second).is_empty());

    second.apply_host_damage(second_id, 1000.0);
    let second_children = spawned_templates(&second);
    assert_eq!(second_children.len(), 1);
    assert!(second_children[0].starts_with("SecondChild"));
    assert!(spawned_templates(&first)
        .iter()
        .all(|name| !name.starts_with("SecondChild")));
}

#[test]
fn receiving_zero_count_and_missing_ocl_do_not_invent_spawns() {
    let (mut zero, zero_id) = world(true, "ZeroCountList", &["ZeroCountChild"], 0);
    zero.apply_host_damage(zero_id, 1000.0);
    assert!(spawned_templates(&zero).is_empty());

    // This identity has a legacy one-name peel, but the receiving owner's
    // missing OCL is the C++ null handle and must not select that fallback.
    let (mut missing, missing_id) = world(false, "OCL_PoisonFieldSmall", &["PoisonFieldSmall"], 1);
    missing.apply_host_damage(missing_id, 1000.0);
    assert!(spawned_templates(&missing).is_empty());
}

#[test]
fn standalone_body_pending_receipt_resolves_through_the_receiving_owner() {
    let (mut game, victim) = world(true, "SharedDieList", &["PendingChild"], 1);
    let frame = u32::try_from(game.get_current_frame()).unwrap();
    {
        let object = game.host_object_mut(victim).unwrap();
        let mut health_events = generals_main::game_logic::HostHealthEvents::default();
        let _ = object.take_damage_from_immediate(1000.0, None, frame, &mut health_events);
        assert!(object.pending_create_object_die_spawns.is_empty());
    }
    assert!(!spawned_templates(&game).contains(&"PendingChild".to_owned()));
    game.apply_pending_create_object_die(victim);
    assert!(spawned_templates(&game).contains(&"PendingChild".to_owned()));
}

#[test]
fn received_object_without_create_die_does_not_take_a_name_fallback() {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "Object.ini".into(),
        "Object DaisyCutterBomb\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )])
    .unwrap();
    game.add_player(Player::new(7, Team::USA, "Owner", true));
    let id = game
        .create_object_for_player("DaisyCutterBomb", 7, glam::Vec3::ZERO)
        .unwrap();
    assert!(
        game.host_object(id).unwrap().create_object_die.is_none(),
        "the receiving Object's explicit absence of CreateObjectDie must not discover the legacy name fallback"
    );
}

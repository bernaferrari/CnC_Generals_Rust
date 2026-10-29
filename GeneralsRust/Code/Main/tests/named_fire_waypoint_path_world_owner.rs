use game_engine::common::{
    dict::Dict,
    ini::ini_locomotor::LocomotorStore,
    name_key_generator::NameKeyGenerator,
    system::DataChunkOutput,
    well_known_keys::{key_player_faction, key_player_is_human, key_player_name},
};
use gamelogic::{
    scripting::{
        chunk_codec::ScriptTemplateLookup,
        core::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
            ScriptActionType, ScriptList,
        },
        engine::ScriptEngine,
    },
    sides_list::SidesList,
    weapon::WeaponTemplate,
};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};

const UNIT_NAME: &str = "NamedPathLauncher";
const OBJECT_NAME: &str = "SharedNamedScud";
const WAYPOINT_PATH: &str = "SharedCruisePath";
const WEAPON: &str = "SameNamePathWeapon";
const HIGHER_PRIORITY_WEAPON: &str = "HigherPriorityPathWeapon";
const OWNER_PROJECTILE: &str = "OwnerNamedPathProjectile";
const PEER_PROJECTILE: &str = "PeerNamedPathProjectile";

fn waypoint_map(root: &std::path::Path, endpoint_x: f32) -> std::path::PathBuf {
    let mut sides = SidesList::new();
    let mut player = Dict::new();
    player.set_ascii_string(key_player_name(), "Owner");
    player.set_ascii_string(key_player_faction(), "FactionAmerica");
    player.set_bool(key_player_is_human(), true);
    sides.add_side(&player);

    let mut action = ScriptAction::new(ScriptActionType::NamedFireWeaponFollowingWaypointPath);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            OBJECT_NAME.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            WAYPOINT_PATH.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedNamedFireAlongPath".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    sides
        .get_side_info_mut(0)
        .expect("Owner side")
        .set_script_list(Some(Box::new(scripts)));
    let script_engine = ScriptEngine::new().expect("script templates");

    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(0);
    }
    map.close_data_chunk();

    map.open_data_chunk("ObjectsList", 3);
    for (x, y, object_type, properties) in [
        (50.0, 50.0, "Waypoint", {
            let mut dict = Dict::new();
            dict.set_int(NameKeyGenerator::name_to_key("waypointID"), 1);
            dict.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), "CruiseStart");
            dict.set_ascii_string(
                NameKeyGenerator::name_to_key("waypointPathLabel1"),
                WAYPOINT_PATH,
            );
            dict
        }),
        (endpoint_x, 50.0, "Waypoint", {
            let mut dict = Dict::new();
            dict.set_int(NameKeyGenerator::name_to_key("waypointID"), 2);
            dict.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), "CruiseEnd");
            dict.set_ascii_string(
                NameKeyGenerator::name_to_key("waypointPathLabel1"),
                WAYPOINT_PATH,
            );
            dict
        }),
        (50.0, 50.0, UNIT_NAME, {
            let mut dict = Dict::new();
            dict.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), OBJECT_NAME);
            dict.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "Owner");
            dict
        }),
    ] {
        map.open_data_chunk("Object", 3);
        for value in [x, y, 0.0, 0.0] {
            map.write_real(value);
        }
        map.write_int(0);
        map.write_ascii_string(object_type);
        map.write_dict(&properties);
        map.close_data_chunk();
    }
    map.close_data_chunk();
    map.open_data_chunk("WaypointsList", 1);
    map.write_int(1);
    map.write_int(1);
    map.write_int(2);
    map.close_data_chunk();
    sides.write_sides_data_chunk(&mut map, &ScriptTemplateLookup::from_engine(&script_engine));

    let directory = root.join(format!("endpoint-{endpoint_x}"));
    std::fs::create_dir(&directory).expect("create map fixture directory");
    let path = directory.join("NamedFireOwner.map");
    std::fs::write(&path, map.into_ckmp_bytes()).expect("write map fixture");
    std::fs::write(directory.join("Map.ini"), "").expect("write Map.ini");
    path
}

fn admitted_world(
    path: &std::path::Path,
    projectile_name: &str,
    capable_of_following_waypoint: bool,
    secondary_weapon: Option<(&str, &str, bool)>,
) -> GameLogic {
    admitted_world_with_projectile_ai(
        path,
        projectile_name,
        capable_of_following_waypoint,
        secondary_weapon,
        true,
    )
}

fn admitted_world_with_projectile_ai(
    path: &std::path::Path,
    projectile_name: &str,
    capable_of_following_waypoint: bool,
    secondary_weapon: Option<(&str, &str, bool)>,
    projectile_has_ai: bool,
) -> GameLogic {
    let mut game = GameLogic::new();
    let mut player_templates =
        game_engine::common::rts::player_template::PlayerTemplateStore::new_owned();
    player_templates.apply_source(
        "PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\nPlayerTemplate FactionAmerica\n Side = America\nEnd\n",
        game_engine::common::ini::INILoadType::Overwrite,
    ).unwrap();
    game.admit_player_template_definitions(player_templates)
        .unwrap();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source("Locomotor OwnedProjectileLoco\n Surfaces = AIR\n Speed = 100\n Acceleration = 100\n TurnRate = 120\n Appearance = WINGS\n End\n")
        .unwrap();
    game.admit_locomotor_definitions(locomotors).unwrap();
    let secondary_weapon_set = secondary_weapon
        .map(|(name, _, _)| format!(" Weapon = SECONDARY {name}\n"))
        .unwrap_or_default();
    let projectile_ai = if projectile_has_ai {
        " Behavior = AIUpdateInterface ProjectileAI\n End\n"
    } else {
        ""
    };
    game.admit_object_ini_sources([(
        "NamedFireOwner.ini".into(),
        format!(
            "Object {UNIT_NAME}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 400\n InitialHealth = 400\n End\n WeaponSet\n  Conditions = NONE\n  Weapon = PRIMARY {WEAPON}\n{secondary_weapon_set} End\nEnd\n\
             Object {projectile_name}\n KindOf = PROJECTILE AIRCRAFT\n Locomotor = SET_NORMAL OwnedProjectileLoco\n Body = ActiveBody ProjectileBody\n MaxHealth = 100\n InitialHealth = 100\n End\n{projectile_ai}End\n"
        ),
    )])
    .expect("admit launcher and projectile object definitions");
    let mut weapon = WeaponTemplate::new(WEAPON.to_owned());
    weapon.capable_of_following_waypoint = capable_of_following_waypoint;
    weapon.projectile_name = projectile_name.to_owned();
    let mut weapons = vec![(WEAPON.into(), weapon)];
    if let Some((name, projectile, capable)) = secondary_weapon {
        let mut secondary = WeaponTemplate::new(name.to_owned());
        secondary.capable_of_following_waypoint = capable;
        secondary.projectile_name = projectile.to_owned();
        weapons.push((name.to_owned(), secondary));
    }
    game.admit_receiving_weapon_snapshot(weapons)
        .expect("admit receiving weapon definition");
    game.start_new_game(GameMode::SinglePlayer);
    assert!(game.load_map(path.to_str().expect("UTF-8 map path")));
    game
}

#[test]
fn projectile_without_ai_is_created_but_receives_no_waypoint_order() {
    let root = tempfile::tempdir().unwrap();
    let map = waypoint_map(root.path(), 200.0);
    let mut world = admitted_world_with_projectile_ai(&map, OWNER_PROJECTILE, true, None, false);
    let shooter = world
        .host_objects()
        .registered_values()
        .find(|object| object.name == OBJECT_NAME)
        .unwrap()
        .id;
    world.update();
    let projectile = world
        .host_objects()
        .registered_values()
        .find(|object| {
            object.template_name == OWNER_PROJECTILE && object.producer_id == Some(shooter)
        })
        .expect("the projectile exists even when it has no AI interface");
    assert!(!projectile.has_ai_update_interface());
    assert!(projectile.requested_destination.is_none());
    assert!(projectile.movement.target_position.is_none());
    assert!(projectile.movement.path.is_empty());
}

fn fired_endpoint(game: &GameLogic, shooter: ObjectId, template: &str) -> f32 {
    let projectile = game
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == template && object.producer_id == Some(shooter))
        .expect("receiving script action creates the projectile definition from its world");
    projectile
        .requested_destination
        .or(projectile.movement.target_position)
        .or_else(|| projectile.movement.path.last().copied())
        .expect("projectile receives a waypoint destination")
        .x
}

#[test]
fn authored_named_fire_script_uses_receiving_weapon_projectile_and_waypoint_path() {
    let root = tempfile::tempdir().unwrap();
    let owner_map = waypoint_map(root.path(), 200.0);
    let peer_map = waypoint_map(root.path(), 350.0);
    let mut owner = admitted_world(&owner_map, OWNER_PROJECTILE, true, None);
    let mut peer = admitted_world(&peer_map, PEER_PROJECTILE, true, None);
    let mut incapable = admitted_world(&owner_map, PEER_PROJECTILE, false, None);
    let owner_shooter = owner
        .host_objects()
        .registered_values()
        .find(|object| object.name == OBJECT_NAME)
        .expect("owner map admits named launcher")
        .id;
    let peer_shooter = peer
        .host_objects()
        .registered_values()
        .find(|object| object.name == OBJECT_NAME)
        .expect("peer map admits same named launcher")
        .id;
    let incapable_shooter = incapable
        .host_objects()
        .registered_values()
        .find(|object| object.name == OBJECT_NAME)
        .expect("third world admits the same named launcher")
        .id;
    assert_eq!(
        owner_shooter, peer_shooter,
        "worlds reuse the same Object ID"
    );
    assert_eq!(owner_shooter, incapable_shooter);

    // GameLogic::update drives the map-authored ScriptEngine through Main's
    // HostScriptExecutionDriver. No helper call or global weapon-store setup
    // stands in for the production action route.
    owner.update();
    assert_eq!(
        fired_endpoint(&owner, owner_shooter, OWNER_PROJECTILE),
        200.0
    );
    assert!(
        peer.host_objects()
            .registered_values()
            .all(|object| object.producer_id != Some(peer_shooter)),
        "executing the owner's map script does not mutate the same-ID peer"
    );

    peer.update();
    assert_eq!(fired_endpoint(&peer, peer_shooter, PEER_PROJECTILE), 350.0);
    incapable.update();
    assert!(
        incapable
            .host_objects()
            .registered_values()
            .all(|object| object.producer_id != Some(incapable_shooter)),
        "a conflicting non-capable same-name row suppresses fire in that world"
    );
    assert!(
        owner.host_objects().registered_values().any(|object| {
            object.template_name == OWNER_PROJECTILE && object.producer_id == Some(owner_shooter)
        }),
        "same-name weapon resolves through the owner's distinct projectile object definition"
    );
}

#[test]
fn first_capable_slot_with_no_projectile_does_not_fall_through_to_primary() {
    let root = tempfile::tempdir().unwrap();
    let map = waypoint_map(root.path(), 200.0);
    for projectile in ["NONE", "UnadmittedProjectile"] {
        let mut world = admitted_world(
            &map,
            OWNER_PROJECTILE,
            true,
            Some((HIGHER_PRIORITY_WEAPON, projectile, true)),
        );
        let shooter = world
            .host_objects()
            .registered_values()
            .find(|object| object.name == OBJECT_NAME)
            .expect("map admits named launcher")
            .id;

        // Secondary precedes Primary in C++ findWaypointFollowingCapableWeapon.
        // A missing selected projectile cannot fall through or be invented.
        world.update();
        assert!(
            world
                .host_objects()
                .registered_values()
                .all(|object| object.producer_id != Some(shooter)),
            "missing selected projectile {projectile} must not fire"
        );
    }
}

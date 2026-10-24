//! C++ AIStates.cpp:1088–1113 distinguishes a null AI from an AI with no locomotor.
use super::*;

const AI_WEAPON_SOURCE: &str = "Weapon AiPresenceRifle\n PrimaryDamage = 25\n AttackRange = 200\n DamageType = SMALL_ARMS\nEnd\nWeapon AiPresenceTouch\n PrimaryDamage = 25\n AttackRange = 5\n DamageType = SMALL_ARMS\nEnd\n";

fn load_rules() {
    let mut ini = game_engine::common::ini::ini::INI::new();
    ini.with_inline_source(AI_WEAPON_SOURCE, |ini| ini.parse_current_file())
        .expect("authored selected weapon");
    game_engine::common::ini::ini_locomotor::load_locomotors_from_str(
        "Locomotor AiPresenceGround\n Surfaces = GROUND\n Speed = 20\n Acceleration = 100\n Braking = 100\n Appearance = TWO_LEGS\nEnd\n",
    )
    .expect("authored ground locomotor");
}

fn parsed_template(name: &str, ai_class: Option<&str>, ground: bool) -> (ThingTemplate, String) {
    parsed_weapon_template(name, ai_class, ground, "AiPresenceRifle")
}

fn parsed_weapon_template(
    name: &str,
    ai_class: Option<&str>,
    ground: bool,
    weapon: &str,
) -> (ThingTemplate, String) {
    let behavior = ai_class
        .map(|class| format!(" Behavior = {class} ModuleTag_AI\n End\n"))
        .unwrap_or_default();
    let locomotor = if ground {
        " Locomotor = SET_NORMAL AiPresenceGround\n"
    } else {
        ""
    };
    // Contact reaches overlapping geometry, even with distinct center cells.
    let geometry = if weapon == "AiPresenceTouch" {
        " Geometry = CYLINDER\n GeometryMajorRadius = 100\n GeometryHeight = 5\n"
    } else {
        ""
    };
    let text = format!(
        "Object {name}\n KindOf = INFANTRY ATTACK_NEEDS_LINE_OF_SIGHT\n{geometry} Body = ActiveBody ModuleTag_Body\n MaxHealth = 200\n InitialHealth = 200\n End\n{behavior}{locomotor} WeaponSet\n Conditions = None\n Weapon = PRIMARY {weapon}\n End\nEnd\n"
    );
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser.parse_ini_content(&text, "ai_presence.ini").unwrap(),
        1
    );
    let definition = parser.get_definition(name).unwrap();
    assert_eq!(
        definition
            .behavior_modules
            .iter()
            .any(|module| { ai_class.is_some_and(|class| module.class_name == class) }),
        ai_class.is_some(),
        "test must preserve the actual authored module declaration"
    );
    (
        GameLogic::build_template_from_object_definition(name, definition, None),
        text,
    )
}

fn set_line_blocked(logic: &mut GameLogic, source: ObjectId, target: ObjectId, blocked: bool) {
    let start = logic
        .pathfinding_system
        .grid
        .world_to_grid(logic.host_object(source).unwrap().get_position());
    let goal = logic
        .pathfinding_system
        .grid
        .world_to_grid(logic.host_object(target).unwrap().get_position());
    assert_eq!(start.y, goal.y);
    for x in start.x + 1..goal.x {
        logic.set_pathfinding_static_block(x, start.y, blocked);
    }
}

fn scene(template: (ThingTemplate, String)) -> (GameLogic, ObjectId, ObjectId) {
    scene_for_weapon(template, "AiPresenceRifle")
}

fn scene_for_weapon(
    fixture: (ThingTemplate, String),
    weapon: &str,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    let (template, source_text) = fixture;
    let name = template.name.clone();
    if source_text.is_empty() {
        // Explicit uncaptured programmatic control: absence is not proven.
        logic.templates.insert(name.clone(), template);
        let mut target = ThingTemplate::new("AiPresenceTarget");
        target.add_kind_of(KindOf::Infantry).set_health(200.);
        logic.templates.insert("AiPresenceTarget".into(), target);
    } else {
        let (_, target_text) = parsed_template("AiPresenceTarget", None, false);
        logic
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([
                    ("ai_presence_source.ini".into(), source_text),
                    ("ai_presence_target.ini".into(), target_text),
                ])
                .unwrap(),
            )
            .unwrap();
        let mut weapons = std::collections::HashMap::new();
        assert_eq!(
            crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                &mut weapons,
                AI_WEAPON_SOURCE,
                crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
            ),
            2
        );
        logic
            .admit_weapon_content_snapshot(
                weapons
                    .into_values()
                    .map(|rule| (rule.name.clone(), rule))
                    .collect(),
                &[],
            )
            .unwrap();
    }
    let source = logic
        .create_object(&name, Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let target = logic
        .create_object(
            "AiPresenceTarget",
            Team::GLA,
            glam::Vec3::new(90.0, 0.0, 10.0),
        )
        .unwrap();
    let source_object = logic.host_object(source).unwrap();
    let slot = source_object
        .selected_weapon_slot()
        .expect("selected authored weapon");
    assert_eq!(source_object.weapon_name_for_slot(slot), Some(weapon));
    assert!(
        source_object.is_within_attack_range_for_slot(slot, logic.host_object(target).unwrap())
    );
    set_line_blocked(&mut logic, source, target, true);
    assert!(
        ({
            let ground_owner = logic.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                logic.map_loaded,
                logic.terrain.as_ref(),
                logic.pathfinding_height_samples.as_ref(),
                &logic.pathfinding_system.grid,
            );
            logic.pathfinding_system.is_attack_view_blocked(
                logic.host_object(source).unwrap().get_position(),
                logic.host_object(target).unwrap().get_position(),
                &ground_source,
            )
        })
    );
    (logic, source, target)
}

#[test]
fn authored_ai_interface_contact_weapon_skips_los_with_and_without_ai() {
    crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at(
        module_path!(),
        "authored_ai_interface_contact_weapon_skips_los_with_and_without_ai",
        || {
            load_rules();
            assert!(
                crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name("AiPresenceTouch")
            );
            for (name, ai, ground) in [
                ("AiPresenceContactAbsent", None, false),
                ("AiPresenceContactMoving", Some("AIUpdateInterface"), true),
            ] {
                let (logic, source, target) = scene_for_weapon(
                    parsed_weapon_template(name, ai, ground, "AiPresenceTouch"),
                    "AiPresenceTouch",
                );
                assert!(
                    !logic
                        .host_object(source)
                        .unwrap()
                        .leech_range_active_primary
                );
                assert!(!logic.out_of_weapon_range_object(source, target), "{name}");
            }
        },
    );
}

#[test]
fn authored_ai_interface_presence_changes_only_the_cpp_ground_los_branch() {
    crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at(
        module_path!(),
        "authored_ai_interface_presence_changes_only_the_cpp_ground_los_branch",
        || {
            load_rules();
            for (name, ai, ground, presence, blocked) in [
                ("AiPresenceAbsent", None, false, Some(false), true),
                (
                    "AiPresenceIdle",
                    Some("AIUpdateInterface"),
                    false,
                    Some(true),
                    false,
                ),
                (
                    "AiPresenceMoving",
                    Some("AIUpdateInterface"),
                    true,
                    Some(true),
                    true,
                ),
                (
                    "AiPresenceFlightDeck",
                    Some("FlightDeckBehavior"),
                    false,
                    Some(true),
                    false,
                ),
                (
                    "AiPresenceSlaved",
                    Some("SlavedUpdate"),
                    false,
                    Some(false),
                    true,
                ),
            ] {
                let (logic, source, target) = scene(parsed_template(name, ai, ground));
                assert_eq!(
                    logic
                        .host_object(source)
                        .unwrap()
                        .cur_locomotor_name
                        .is_some(),
                    ground
                );
                assert_eq!(
                    logic
                        .host_object(source)
                        .unwrap()
                        .get_template()
                        .authored_ai_update_interface(),
                    presence
                );
                assert_eq!(
                    logic.out_of_weapon_range_object(source, target),
                    blocked,
                    "{name}"
                );
                let mut logic = logic;
                set_line_blocked(&mut logic, source, target, false);
                assert!(
                    !logic.out_of_weapon_range_object(source, target),
                    "{name} in the open"
                );
            }
        },
    );
}

#[test]
fn authored_ai_interface_los_preserves_held_airborne_target_and_leech_exceptions() {
    crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at(
        module_path!(),
        "authored_ai_interface_los_preserves_held_airborne_target_and_leech_exceptions",
        || {
            load_rules();
            let (mut logic, source, target) = scene(parsed_template(
                "AiPresenceExceptions",
                Some("AIUpdateInterface"),
                true,
            ));
            assert!(logic.out_of_weapon_range_object(source, target));
            logic.host_object_mut(source).unwrap().status.disabled_held = true;
            assert!(!logic.out_of_weapon_range_object(source, target));
            logic.host_object_mut(source).unwrap().status.disabled_held = false;
            let object = logic.host_object_mut(source).unwrap();
            object.allow_to_fall = true;
            object.set_position(glam::Vec3::new(10.0, 60.0, 10.0));
            assert!(object.is_above_terrain());
            assert!(!logic.out_of_weapon_range_object(source, target));
            let (object, fixture_partition_object) =
                logic.host_object_and_partition_mut(source).unwrap();
            object.set_position_with_partition_owner(
                glam::Vec3::new(10.0, 0.0, 10.0),
                0,
                &mut Default::default(),
                fixture_partition_object,
            );
            object.allow_to_fall = false;
            assert!(logic.out_of_weapon_range_object(source, target));
            let (object, fixture_partition_object) =
                logic.host_object_and_partition_mut(target).unwrap();
            object.set_position_with_partition_owner(
                glam::Vec3::new(90.0, 60.0, 10.0),
                0,
                &mut Default::default(),
                fixture_partition_object,
            );
            assert!(object.is_significantly_above_terrain());
            assert!(!logic.out_of_weapon_range_object(source, target));
            logic.set_fixture_object_position(target, glam::Vec3::new(90.0, 0.0, 10.0));
            assert!(logic.out_of_weapon_range_object(source, target));
            logic
                .host_object_mut(source)
                .unwrap()
                .leech_range_active_primary = true;
            assert!(!logic.out_of_weapon_range_object(source, target));
        },
    );
}

#[test]
fn authored_ai_interface_metadata_survives_clone_restore_and_equal_id_worlds() {
    crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at(
        module_path!(),
        "authored_ai_interface_metadata_survives_clone_restore_and_equal_id_worlds",
        || {
            load_rules();
            let absent = parsed_template("AiPresenceSavedAbsent", None, false);
            assert_eq!(absent.0.clone().authored_ai_update_interface(), Some(false));
            let (source_world, source, target) = scene(absent);
            let (other_world, other_source, other_target) = scene(parsed_template(
                "AiPresenceSavedPresent",
                Some("AIUpdateInterface"),
                false,
            ));
            assert_eq!(source, other_source);
            for _ in 0..3 {
                assert!(source_world.out_of_weapon_range_object(source, target));
                assert!(!other_world.out_of_weapon_range_object(other_source, other_target));
            }
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source_world).unwrap();
            let mut restored = GameLogic::new();
            restored.templates = source_world.templates.clone();
            restored.receiving_module_content = source_world.receiving_module_content.clone();
            restored.receiving_object_definitions =
                source_world.receiving_object_definitions.clone();
            restored.receiving_template_baseline = source_world.receiving_template_baseline.clone();
            restored.weapon_definition_base = source_world.weapon_definition_base.clone();
            restored.weapon_definitions = source_world.weapon_definitions.clone();
            restored.projectile_definitions = source_world.projectile_definitions.clone();
            builder
                .restore_from_snapshot(&snapshot, &mut restored)
                .unwrap();
            // Static map obstacles are map input, not a new Object save field.
            set_line_blocked(&mut restored, source, target, true);
            assert_eq!(
                restored
                    .host_object(source)
                    .unwrap()
                    .get_template()
                    .authored_ai_update_interface(),
                Some(false)
            );
            assert!(restored.out_of_weapon_range_object(source, target));
            assert!(!other_world.out_of_weapon_range_object(other_source, other_target));

            // A hand-built or old serialized template carries no proof of
            // module absence. Retain that uncertainty through admission.
            let mut template = ThingTemplate::new("AiPresenceHandBuilt");
            template.add_kind_of(KindOf::Infantry);
            template.add_kind_of(KindOf::AttackNeedsLineOfSight);
            template.set_primary_weapon_name("AiPresenceRifle");
            let (unknown, id, victim) = scene((template, String::new()));
            assert_eq!(
                unknown
                    .host_object(id)
                    .unwrap()
                    .get_template()
                    .authored_ai_update_interface(),
                None
            );
            assert!(!unknown.out_of_weapon_range_object(id, victim));
        },
    );
}

#[test]
fn unknown_ai_behavior_is_rejected_by_actual_receiving_module_factory() {
    let (projection, text) =
        parsed_template("AiPresenceUnknown", Some("DecorationAIUpdate"), false);
    assert_eq!(projection.authored_ai_update_interface(), None);
    let error = crate::assets::ReceivingObjectContent::from_sources([(
        "unknown-ai-module.ini".into(),
        text,
    )])
    .err()
    .expect("C++ ModuleFactory has no DecorationAIUpdate registration");
    assert!(error.contains("DecorationAIUpdate"), "{error}");
}

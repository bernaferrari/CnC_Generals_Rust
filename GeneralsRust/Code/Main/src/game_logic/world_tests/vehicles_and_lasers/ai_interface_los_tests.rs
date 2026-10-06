//! C++ AIStates.cpp:1088–1113 distinguishes a null AI from an AI with no locomotor.
use super::*;

fn load_rules() {
    let mut ini = game_engine::common::ini::ini::INI::new();
    ini.with_inline_source(
        "Weapon AiPresenceRifle\n PrimaryDamage = 25\n AttackRange = 200\n DamageType = SMALL_ARMS\nEnd\nWeapon AiPresenceTouch\n PrimaryDamage = 25\n AttackRange = 5\n DamageType = SMALL_ARMS\nEnd\n",
        |ini| ini.parse_current_file(),
    )
    .expect("authored selected weapon");
    game_engine::common::ini::ini_locomotor::load_locomotors_from_str(
        "Locomotor AiPresenceGround\n Surfaces = GROUND\n Speed = 20\n Acceleration = 100\n Braking = 100\n Appearance = TWO_LEGS\nEnd\n",
    )
    .expect("authored ground locomotor");
}

fn parsed_template(name: &str, ai_class: Option<&str>, ground: bool) -> ThingTemplate {
    parsed_weapon_template(name, ai_class, ground, "AiPresenceRifle")
}

fn parsed_weapon_template(
    name: &str,
    ai_class: Option<&str>,
    ground: bool,
    weapon: &str,
) -> ThingTemplate {
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
        "Object {name}\n KindOf = INFANTRY ATTACKABLE ATTACK_NEEDS_LINE_OF_SIGHT\n{geometry} Body = ActiveBody ModuleTag_Body\n MaxHealth = 200\n End\n{behavior}{locomotor} WeaponSet\n Conditions = None\n Weapon = PRIMARY {weapon}\n End\nEnd\n"
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
    GameLogic::build_template_from_object_definition(name, definition, None)
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

fn scene(template: ThingTemplate) -> (GameLogic, ObjectId, ObjectId) {
    scene_for_weapon(template, "AiPresenceRifle")
}

fn scene_for_weapon(template: ThingTemplate, weapon: &str) -> (GameLogic, ObjectId, ObjectId) {
    let mut logic = GameLogic::new();
    let name = template.name.clone();
    logic.templates.insert(name.clone(), template);
    logic.templates.insert(
        "AiPresenceTarget".into(),
        parsed_template("AiPresenceTarget", None, false),
    );
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
    assert!(logic.pathfinding_system.is_attack_view_blocked(
        logic.host_object(source).unwrap().get_position(),
        logic.host_object(target).unwrap().get_position(),
    ));
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
                (
                    "AiPresenceUnknown",
                    Some("DecorationAIUpdate"),
                    false,
                    None,
                    false,
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
            let object = logic.host_object_mut(source).unwrap();
            object.set_position(glam::Vec3::new(10.0, 0.0, 10.0));
            object.allow_to_fall = false;
            assert!(logic.out_of_weapon_range_object(source, target));
            let object = logic.host_object_mut(target).unwrap();
            object.set_position(glam::Vec3::new(90.0, 60.0, 10.0));
            assert!(object.is_significantly_above_terrain());
            assert!(!logic.out_of_weapon_range_object(source, target));
            logic
                .host_object_mut(target)
                .unwrap()
                .set_position(glam::Vec3::new(90.0, 0.0, 10.0));
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
            assert_eq!(absent.clone().authored_ai_update_interface(), Some(false));
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
            let (unknown, id, victim) = scene(template);
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

//! C++ BuildAssistant::buildObjectLineNow / buildObjectNow actual command controls.
use super::*;

pub(super) fn admit_retail_line_build_rules(world: &mut GameLogic) {
    let rules = world
        .production_energy_rules()
        .overlay_ini_text("GameData\n MaxLineBuildObjects = 50\nEnd\n")
        .expect("parse original retail line-build limit");
    world.admit_production_energy_base(rules);
}

pub(super) fn admit_constructor_commands(
    world: &mut GameLogic,
    builder: ObjectId,
    stem: &str,
    target: &str,
) {
    // Private immutable catalog names avoid restoring/clearing shared content.
    let text = format!(
        "CommandButton {stem}Button\n Command = DOZER_CONSTRUCT\n Object = {target}\nEnd\nCommandSet {stem}Set\n 1 = {stem}Button\nEnd\n"
    );
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&text, |ini| ini.parse_current_file())
        .unwrap();
    world
        .objects
        .get_mut(&builder)
        .unwrap()
        .command_set_override = Some(format!("{stem}Set"));
}

// Original INIZH.big Data/INI/Object/FactionBuilding.ini ChinaWall, renamed
// only at the Object header. Full source block retains its death/body/art rows.
const CHINA_WALL: &str = r#"Object Owner61ChinaWall

  ; *** ART Parameters ***
  SelectPortrait         = SNWall_L
  ButtonImage            = SNWall
  Draw = W3DModelDraw ModuleTag_01
    OkToChangeModelColor = Yes

    ; day
    DefaultConditionState
      Model         = NBWall02Y
    End
    ConditionState = DAMAGED
      Model         = NBWall02Y_D
    End
    ConditionState = REALLYDAMAGED RUBBLE
      Model         = NBWall02Y_E
    End

    ConditionState = SNOW
      Model         = NBWall02Y_S
    End
    ConditionState = DAMAGED SNOW
      Model         = NBWall02Y_DS
    End
    ConditionState = REALLYDAMAGED RUBBLE SNOW
      Model         = NBWall02Y_ES
    End

  End

  ; ***DESIGN parameters ***
  DisplayName      = OBJECT:ConcreteWall
  Side = China
  EditorSorting    = STRUCTURE

  BuildCost           = 100
  BuildTime           = 2.0           ; in seconds
  VisionRange         = 150.0           ; Shroud clearing distance
  ShroudClearingRange = 150
  ArmorSet
    Conditions      = None
    Armor           = StructureArmor
    DamageFX        = StructureDamageFXNoShake
  End
  ExperienceValue     = 50 50 50 50  ; Experience point value at each level

  FenceWidth      = 9  ; Spacing used by the fence tool in worldbuilder.
  FenceXOffset    = 0   ; Offset used by the fence tool in worldbuilder so corners line up.
  CrushableLevel  = 2  ;What am I?:        0 = for infantry, 1 = for trees, 2 = general vehicles

  ; *** AUDIO Parameters ***
  VoiceSelect = NoSound

  ; *** ENGINEERING Parameters ***
  KindOf          = PRELOAD STRUCTURE SELECTABLE STICK_TO_TERRAIN_SLOPE IMMOBILE LINEBUILD DEFENSIVE_WALL
  Body            = StructureBody ModuleTag_02
    MaxHealth       = 1000.0
    InitialHealth   = 1000.0

  End

  Behavior                 = DestroyDie ModuleTag_03
    ; none
  End
  Behavior                 = CreateObjectDie ModuleTag_04
    CreationList      = OCL_GenericWallSegmentDebris
  End
  Behavior                 = FXListDie ModuleTag_05
    DeathFX           = FX_GenericWallSegmentDeath
  End

  Geometry            = BOX
  GeometryMajorRadius = 4.0
  GeometryMinorRadius = 6.0
  GeometryHeight      = 12.0
  GeometryIsSmall     = No
  Shadow          = SHADOW_VOLUME
  BuildCompletion = PLACED_BY_PLAYER

End

"#;

fn line_world(stem: &str, funds: u32) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    admit_retail_line_build_rules(&mut world);
    // Admit every receiving factory definition once, before either builder
    // exists. Projections alone do not supply the typed Body/Behavior owner.
    let mut source = CHINA_WALL.to_string();
    source.push_str("Object TestDozer\nKindOf = VEHICLE SELECTABLE DOZER SCORE\nBuildCost = 1000\nBody = ActiveBody DozerBody\nMaxHealth = 300\nInitialHealth = 300\nEnd\nBehavior = DozerAIUpdate DozerAI\nEnd\nEnd\n");
    for (name, radius) in [
        ("Owner61WallStartBlocker", 20.),
        ("Owner61OrdinaryStartBlocker", 20.),
        ("Owner67FacingBlocker0", 1.),
        ("Owner67FacingBlocker1", 1.),
    ] {
        source.push_str(&format!("Object {name}\nKindOf = STRUCTURE IMMOBILE\nBody = ActiveBody BlockerBody\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nGeometry = BOX\nGeometryIsSmall = No\nGeometryHeight = 20\nGeometryMajorRadius = {radius}\nGeometryMinorRadius = {radius}\nEnd\n"));
    }
    source.push_str("Object Owner61PriorScaffold\nKindOf = STRUCTURE IMMOBILE\nBuildCost = 500\nBody = ActiveBody ScaffoldBody\nMaxHealth = 1200\nInitialHealth = 1200\nEnd\nGeometry = CYLINDER\nGeometryIsSmall = No\nGeometryHeight = 20\nGeometryMajorRadius = 6\nGeometryMinorRadius = 6\nEnd\n");
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "owner61_china_wall.ini".into(),
                source,
            )])
            .expect("parse complete receiving line-build fixture"),
        )
        .unwrap();
    let wall = world.templates.get("Owner61ChinaWall").unwrap();
    assert!(wall.is_kind_of(KindOf::Structure));
    assert!(wall.is_kind_of(KindOf::Immobile));
    assert!(wall.is_kind_of(KindOf::LineBuild));
    assert_eq!(wall.geometry_info.major_radius, 4.0);
    assert_eq!(wall.geometry_info.minor_radius, 6.0);
    ensure_test_player_for_team(&mut world, Team::USA);
    world.get_player_mut(0).unwrap().resources.supplies = funds;
    let first = world
        .create_object_for_player("TestDozer", 0, Vec3::new(-100.0, 0.0, -100.0))
        .unwrap();
    let second = world
        .create_object_for_player("TestDozer", 0, Vec3::new(-80.0, 0.0, -100.0))
        .unwrap();
    admit_constructor_commands(&mut world, first, stem, "Owner61ChinaWall");
    (world, first, second)
}

fn order(world: &mut GameLogic, selected: Vec<ObjectId>) {
    world.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstructLineFacing {
            template_name: "Owner61ChinaWall".into(),
            start: Vec3::new(10.0, 0.0, 10.0),
            end: Vec3::new(18.0, 0.0, 10.0),
            orientation: std::f32::consts::PI,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: selected,
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    world.process_commands();
}

fn wall_count(world: &GameLogic) -> usize {
    world
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "Owner61ChinaWall")
        .count()
}

#[test]
fn china_wall_linebuild_uses_first_constructor_full_body_and_no_cash_debit() {
    let (mut a, first, second) = line_world("Owner61WallInstant", 50);
    let (mut b, b_first, b_second) = line_world("Owner61WallOther", 777);
    assert_eq!((first, second), (b_first, b_second));
    let b_position = b.host_object(b_first).unwrap().get_position();
    order(&mut a, vec![first, second]);
    let mut walls = a
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "Owner61ChinaWall")
        .collect::<Vec<_>>();
    walls.sort_by(|a, b| a.get_position().x.total_cmp(&b.get_position().x));
    assert_eq!(walls.len(), 2);
    assert_eq!(walls[0].get_position(), Vec3::new(10.0, 0.0, 10.0));
    assert_eq!(walls[1].get_position(), Vec3::new(18.0, 0.0, 10.0));
    for wall in walls {
        assert!(wall.status.under_construction);
        assert_eq!(wall.construction_percent, 1.0);
        assert_eq!(wall.health.current, 1000.0);
        assert_eq!(wall.health.maximum, 1000.0);
        assert_eq!(wall.body_initial_health(), 1000.0);
        assert_eq!(wall.producer_id, Some(first));
        assert_eq!(wall.builder_id, None);
        assert_eq!(wall.get_orientation(), std::f32::consts::PI);
    }
    for id in [first, second] {
        let builder = a.host_object(id).unwrap();
        assert_eq!(builder.ai_state, AIState::Idle);
        assert!(builder.dozer_task_build_target.is_none());
        assert!(builder.movement.target_position.is_none());
    }
    assert_eq!(a.get_player(0).unwrap().resources.supplies, 50);
    assert_eq!(a.get_player(0).unwrap().statistics.resources_spent, 200);
    assert_eq!(b.get_player(0).unwrap().resources.supplies, 777);
    assert_eq!(b.get_player(0).unwrap().statistics.resources_spent, 0);
    assert_eq!(wall_count(&b), 0);
    assert_eq!(b.host_object(b_first).unwrap().get_position(), b_position);
    b.reset();
    drop(b);
    assert_eq!(wall_count(&a), 2);
    assert_eq!(a.get_player(0).unwrap().resources.supplies, 50);
}

#[test]
fn linebuild_rejects_invalid_first_constructor_without_switching_to_second() {
    for (index, case) in ["missing_capability", "dead", "foreign"]
        .into_iter()
        .enumerate()
    {
        let (mut world, first, second) = line_world(&format!("Owner61WallReject{index}"), 50);
        admit_constructor_commands(
            &mut world,
            second,
            &format!("Owner61WallValidSecond{index}"),
            "Owner61ChinaWall",
        );
        let builder = world.objects.get_mut(&first).unwrap();
        match case {
            "missing_capability" => builder.command_set_override = Some("Owner61AbsentSet".into()),
            "dead" => builder.health.current = 0.0,
            "foreign" => builder.owner_player_id = Some(1),
            _ => unreachable!(),
        }
        order(&mut world, vec![first, second]);
        assert_eq!(wall_count(&world), 0, "invalid first constructor: {case}");
        assert_eq!(world.get_player(0).unwrap().resources.supplies, 50);
        assert_eq!(world.get_player(0).unwrap().statistics.resources_spent, 0);
        assert!(
            world
                .host_object(second)
                .unwrap()
                .dozer_task_build_target
                .is_none()
        );
        assert!(
            world
                .host_object(second)
                .unwrap()
                .movement
                .target_position
                .is_none()
        );
    }
}

fn admit_existing_line_blocker(world: &mut GameLogic, location: Vec3, name: &str) -> ObjectId {
    admit_existing_line_blocker_with_radius(world, location, name, 20.0)
}

fn admit_existing_line_blocker_with_radius(
    world: &mut GameLogic,
    location: Vec3,
    name: &str,
    radius: f32,
) -> ObjectId {
    let template = world
        .templates
        .get(name)
        .expect("pre-admitted blocker definition");
    assert!(template.is_kind_of(KindOf::Structure));
    assert!(template.is_kind_of(KindOf::Immobile));
    assert_eq!(template.geometry_info.major_radius, radius);
    assert_eq!(template.geometry_info.minor_radius, radius);
    assert_eq!(template.geometry_info.height, 20.);
    world
        .create_object_for_player(name, 0, location)
        .expect("admit preexisting immobile blocker")
}

#[test]
fn instant_linebuild_still_rejects_a_preexisting_immobile_start_blocker() {
    let (mut world, first, second) = line_world("Owner61SettledWallBlocker", 50);
    let blocker = admit_existing_line_blocker(
        &mut world,
        Vec3::new(10.0, 0.0, 10.0),
        "Owner61WallStartBlocker",
    );
    assert!(world.host_object(blocker).unwrap().is_alive());
    order(&mut world, vec![first, second]);
    assert_eq!(wall_count(&world), 0);
    assert!(world.host_object(blocker).unwrap().is_alive());
    assert_eq!(world.get_player(0).unwrap().resources.supplies, 50);
    assert_eq!(world.get_player(0).unwrap().statistics.resources_spent, 0);
    for id in [first, second] {
        assert!(
            world
                .host_object(id)
                .unwrap()
                .dozer_task_build_target
                .is_none()
        );
        assert!(
            world
                .host_object(id)
                .unwrap()
                .movement
                .target_position
                .is_none()
        );
    }
}

#[test]
fn blocked_ordinary_line_preserves_the_prior_real_dozer_task_and_order() {
    let (mut world, first, second) = line_world("Owner61OrdinaryPriorSetup", 10_000);
    const TEMPLATE: &str = "Owner61PriorScaffold";
    let template = world
        .templates
        .get(TEMPLATE)
        .expect("pre-admitted scaffold definition");
    assert_eq!(template.build_cost.supplies, 500);
    assert_eq!(template.geometry_info.major_radius, 6.);
    admit_constructor_commands(&mut world, first, "Owner61OrdinaryPriorTask", TEMPLATE);
    world.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstruct {
            template_name: TEMPLATE.into(),
            location: Vec3::new(-50.0, 0.0, -50.0),
            orientation: 0.0,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![first],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    world.process_commands();
    let builder = world.host_object(first).unwrap();
    let prior_task = builder
        .dozer_task_build_target
        .expect("real single command must create a task");
    let prior_order = builder.target;
    let prior_destination = builder.movement.target_position;
    let prior_state = builder.ai_state.clone();
    assert_eq!(prior_state, AIState::Moving);
    assert!(prior_destination.is_some());
    let prior_object = world.host_object(prior_task).unwrap();
    assert_eq!(prior_object.template_name, TEMPLATE);
    assert_eq!(prior_object.health.current, 1.0);
    assert_eq!(prior_object.construction_percent, 0.0);
    let funds_after_prior = world.get_player(0).unwrap().resources.supplies;
    assert_eq!(funds_after_prior, 9_500);
    let blocker = admit_existing_line_blocker(
        &mut world,
        Vec3::new(100.0, 0.0, 100.0),
        "Owner61OrdinaryStartBlocker",
    );
    world.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstructLineFacing {
            template_name: TEMPLATE.into(),
            start: Vec3::new(100.0, 0.0, 100.0),
            end: Vec3::new(112.0, 0.0, 100.0),
            orientation: 0.0,
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![first, second],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    world.process_commands();
    let builder = world.host_object(first).unwrap();
    assert_eq!(builder.dozer_task_build_target, Some(prior_task));
    assert_eq!(builder.target, prior_order);
    assert_eq!(builder.movement.target_position, prior_destination);
    assert_eq!(builder.ai_state, prior_state);
    assert_eq!(
        world
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == TEMPLATE)
            .count(),
        1
    );
    assert_eq!(
        world.get_player(0).unwrap().resources.supplies,
        funds_after_prior
    );
    assert!(world.host_object(blocker).unwrap().is_alive());
    assert!(
        world
            .host_object(second)
            .unwrap()
            .dozer_task_build_target
            .is_none()
    );
}

#[test]
fn parsed_signed_line_limit_is_consumed_by_actual_wall_command() {
    for (index, limit) in [0i32, -3, 1, 2, 50].into_iter().enumerate() {
        let (mut world, first, second) = line_world(&format!("Owner61SignedLimit{index}"), 50);
        let rules = world
            .production_energy_rules()
            .overlay_ini_text(&format!("GameData\n MaxLineBuildObjects = {limit}\nEnd\n",))
            .expect("parse signed original line limit");
        world.admit_production_energy_base(rules);
        order(&mut world, vec![first, second]);
        let expected = limit.max(1).min(2) as usize;
        assert_eq!(
            wall_count(&world),
            expected,
            "consumed authored limit {limit}"
        );
        assert_eq!(world.get_player(0).unwrap().resources.supplies, 50);
        assert_eq!(
            world.get_player(0).unwrap().statistics.resources_spent,
            expected as u32 * 100
        );
        for wall in world
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "Owner61ChinaWall")
        {
            assert!(wall.status.under_construction);
            assert_eq!(wall.construction_percent, 1.0);
            assert_eq!(wall.health.current, 1000.0);
            assert_eq!(wall.producer_id, Some(first));
            assert_eq!(wall.builder_id, None);
        }
        for id in [first, second] {
            assert!(
                world
                    .host_object(id)
                    .unwrap()
                    .dozer_task_build_target
                    .is_none()
            );
        }
    }
}

#[test]
fn linebuild_authored_facing_controls_rotated_footprints_in_the_driving_world() {
    use crate::game_logic::host_production_buildable_command_residual::{
        LBC_OBJECTS_IN_THE_WAY, LBC_OK, LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK,
        LOCAL_LEGAL_IGNORE_STEALTHED,
    };
    let mut worlds = [
        line_world("Owner67UnrotatedWall", 50),
        line_world("Owner67RotatedWall", 50),
    ];
    assert_eq!(worlds[0].1, worlds[1].1);
    for (index, (world, first, second)) in worlds.iter_mut().enumerate() {
        let name = format!("Owner67FacingBlocker{index}");
        let blocker =
            admit_existing_line_blocker_with_radius(world, Vec3::new(23.5, 0.0, 10.0), &name, 1.0);
        // The normal factory admitted this immobile object; its owned geometry
        // supplies the settled collision cohort before either line creates tiles.
        assert!(world.host_object(blocker).unwrap().is_alive());
        let orientation = if index == 0 {
            0.0
        } else {
            std::f32::consts::FRAC_PI_2
        };
        let code = world.legal_build_code_at_angle_for_builder_ex(
            Team::USA,
            Vec3::new(18.0, 0.0, 10.0),
            "Owner61ChinaWall",
            orientation,
            Some(*first),
            LOCAL_LEGAL_IGNORE_STEALTHED | LOCAL_LEGAL_FAIL_STEALTHED_WITHOUT_FEEDBACK,
        );
        assert_eq!(
            code,
            if index == 0 {
                LBC_OK
            } else {
                LBC_OBJECTS_IN_THE_WAY
            }
        );
        world.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DozerConstructLineFacing {
                template_name: "Owner61ChinaWall".into(),
                start: Vec3::new(10.0, 0.0, 10.0),
                end: Vec3::new(34.0, 0.0, 10.0),
                orientation,
            },
            player_id: 0,
            command_id: 4,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![*first, *second],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        world.process_commands();
        let mut walls = world
            .host_objects()
            .registered_values()
            .filter(|object| object.template_name == "Owner61ChinaWall")
            .collect::<Vec<_>>();
        walls.sort_by(|a, b| a.get_position().x.total_cmp(&b.get_position().x));
        let positions = walls
            .iter()
            .map(|wall| wall.get_position())
            .collect::<Vec<_>>();
        let expected = if index == 0 {
            vec![Vec3::new(10.0, 0.0, 10.0), Vec3::new(18.0, 0.0, 10.0)]
        } else {
            vec![Vec3::new(10.0, 0.0, 10.0)]
        };
        assert_eq!(positions, expected);
        for wall in &walls {
            assert_eq!(wall.get_orientation(), orientation);
            assert!(wall.status.under_construction);
            assert_eq!(wall.construction_percent, 1.0);
            assert_eq!(wall.health.current, 1000.0);
            assert_eq!(wall.producer_id, Some(*first));
            assert_eq!(wall.builder_id, None);
        }
        assert_eq!(world.get_player(0).unwrap().resources.supplies, 50);
        assert_eq!(
            world.get_player(0).unwrap().statistics.resources_spent,
            walls.len() as u32 * 100
        );
        for id in [*first, *second] {
            assert_eq!(world.host_object(id).unwrap().ai_state, AIState::Idle);
            assert!(
                world
                    .host_object(id)
                    .unwrap()
                    .dozer_task_build_target
                    .is_none()
            );
        }
    }
    let [(a, _, _), (mut b, _, _)] = worlds;
    b.reset();
    drop(b);
    assert_eq!(wall_count(&a), 2);
    assert_eq!(a.get_player(0).unwrap().statistics.resources_spent, 200);
}

#[test]
fn linebuild_diagonal_positions_preserve_cpp_division_and_product_order() {
    let (mut world, first, second) = line_world("Owner70DiagonalWall", 50);
    let start = Vec3::new(10.0, 0.0, 10.0);
    let dx = 23.0_f32;
    let dz = 37.0_f32;
    let length = (dx * dx + dz * dz).sqrt();
    // Captured from the original Coord3D type and unmodified CPP tiling
    // statements with clang++ -O0 -ffp-contract=off. Independent expected
    // values, rather than recomputing the Rust implementation in this test.
    // BaseType.h313 / BuildAssistant.cpp1169–1171, dx23/dy37/size8.
    let expected = [
        (0x41200000, 0x41200000),
        (0x41639358, 0x41865ab1),
        (0x41939358, 0x41bcb562),
        (0x41b55d05, 0x41f31013),
        (0x41d726b1, 0x4214b562),
        (0x41f8f05d, 0x422fe2ba),
    ]
    .into_iter()
    .map(|(x, z)| Vec3::new(f32::from_bits(x), 0.0, f32::from_bits(z)))
    .collect::<Vec<_>>();
    let count = expected.len();
    assert_ne!(
        ((dx * (1.0 / length)) * 8.0) * 3.0 + start.x,
        expected[3].x,
        "the chosen original case detects reassociated arithmetic",
    );
    world.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstructLineFacing {
            template_name: "Owner61ChinaWall".into(),
            start,
            end: start + Vec3::new(dx, 0.0, dz),
            orientation: 0.0,
        },
        player_id: 0,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![first, second],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    world.process_commands();
    let mut actual = world
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == "Owner61ChinaWall")
        .map(|object| object.get_position())
        .collect::<Vec<_>>();
    actual.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert_eq!(actual, expected);
    assert_eq!(world.get_player(0).unwrap().resources.supplies, 50);
    assert_eq!(
        world.get_player(0).unwrap().statistics.resources_spent,
        count as u32 * 100
    );
}

#[test]
fn instant_linebuild_records_money_spent_even_when_object_scoring_is_disabled() {
    for (index, scoring) in [false, true].into_iter().enumerate() {
        let (mut world, first, second) = line_world(&format!("Owner70Spending{index}"), 50);
        world
            .templates
            .get_mut("Owner61ChinaWall")
            .unwrap()
            .add_kind_of(KindOf::ScoreCreate);
        world.set_scoring_enabled(scoring);
        order(&mut world, vec![first, second]);
        assert_eq!(wall_count(&world), 2);
        let player = world.get_player(0).unwrap();
        assert_eq!(
            player.statistics.structures_built,
            if scoring { 2 } else { 0 }
        );
        assert_eq!(player.statistics.resources_spent, 200);
        assert_eq!(player.resources.supplies, 50);
    }
}

use super::*;
#[test]
fn test_parse_simple_ini() {
    let ini_content = r#"
; Test INI content
Object USA_Ranger
  Type = Infantry
  DisplayName = "USA Ranger"
  Model = "USA_INFANTRY_RANGER.w3d"
  Texture = "USA_RANGER.tga"
  ArmorType = infantry
  HitPoints = 60
End
"#;

    let mut parser = IniParser::new();
    let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

    assert_eq!(count, 1);
    let def = parser.get_definition("USA_Ranger").unwrap();
    assert_eq!(def.object_type, "Infantry");
    assert_eq!(def.display_name, "USA Ranger");
    assert_eq!(def.model_name, Some("USA_INFANTRY_RANGER.w3d".to_string()));
    assert_eq!(def.hit_points, Some(60.0));
}

#[test]
fn display_name_label_is_translated_via_game_text() {
    game_engine::common::language::Language::register_localized_string(
        "OBJECT:AmericaRanger",
        "Ranger",
    );
    let ini_content = r#"
Object AmericaInfantryRanger
  DisplayName = OBJECT:AmericaRanger
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "display_name.ini")
        .expect("parse");
    let def = parser.get_definition("AmericaInfantryRanger").unwrap();
    assert_eq!(def.display_name, "Ranger");
    game_engine::common::language::Language::clear_localized_strings();
}

#[test]
fn test_parse_multiple_objects() {
    let ini_content = r#"
Object Unit1
  Type = Infantry
End

Object Unit2
  Type = Vehicle
End

Object Unit3
  Type = Building
End
"#;

    let mut parser = IniParser::new();
    let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

    assert_eq!(count, 3);
    assert!(parser.get_definition("Unit1").is_some());
    assert!(parser.get_definition("Unit2").is_some());
    assert!(parser.get_definition("Unit3").is_some());
}

#[test]
fn behavior_modules_keep_dock_fields_with_their_own_module() {
    let source = r#"
Object RetailDockProbe
  Behavior = SomeOtherUpdate ModuleTag_01
    Slots = 99
  End
  Behavior = SupplyWarehouseDockUpdate ModuleTag_06
    StartingBoxes = 400
    NumberApproachPositions = 9
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(source, "retail_dock_probe.ini")
        .expect("parse dock probe");
    let definition = parser
        .get_definition("RetailDockProbe")
        .expect("dock probe definition");

    assert_eq!(definition.behavior_modules.len(), 2);
    assert_eq!(
        definition.behavior_modules[0].attribute("Slots"),
        Some("99"),
        "a repeated field belongs to its preceding Behavior block"
    );
    let dock = &definition.behavior_modules[1];
    assert_eq!(dock.class_name, "SupplyWarehouseDockUpdate");
    assert_eq!(dock.module_tag.as_deref(), Some("ModuleTag_06"));
    assert_eq!(dock.attribute("StartingBoxes"), Some("400"));
    assert_eq!(dock.attribute("NumberApproachPositions"), Some("9"));
    assert_eq!(dock.attribute("Slots"), None);
}

#[test]
fn behavior_module_keeps_deploy_style_fields_after_nested_turret() {
    // Retail DeployStyleAIUpdate places its timing and policy fields
    // *after* a nested Turret block.  They must remain attached to the
    // same Behavior rather than being silently discarded at Turret::End.
    let source = r#"
Object DeployStyleProbe
  Behavior = DeployStyleAIUpdate ModuleTag_04
    Turret
      TurretTurnRate = 80
    End
    PackTime = 3333
    UnpackTime = 3333
    ResetTurretBeforePacking = No
    TurretsFunctionOnlyWhenDeployed = Yes
    TurretsMustCenterBeforePacking = Yes
    ManualDeployAnimations = Yes
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(source, "deploy_style_probe.ini")
        .expect("parse deploy-style probe");
    let definition = parser
        .get_definition("DeployStyleProbe")
        .expect("deploy-style definition");
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("DeployStyleAIUpdate")
        })
        .expect("DeployStyleAIUpdate module");

    assert_eq!(module.attribute("TurretTurnRate"), Some("80"));
    assert_eq!(module.attribute("PackTime"), Some("3333"));
    assert_eq!(module.attribute("UnpackTime"), Some("3333"));
    assert_eq!(module.attribute("ResetTurretBeforePacking"), Some("No"));
    assert_eq!(
        module.attribute("TurretsFunctionOnlyWhenDeployed"),
        Some("Yes")
    );
    assert_eq!(
        module.attribute("TurretsMustCenterBeforePacking"),
        Some("Yes")
    );
    assert_eq!(module.attribute("ManualDeployAnimations"), Some("Yes"));
}

#[test]
fn test_parse_object_reskin_parent_header() {
    let ini_content = r#"
Object BaseTree
  Type = Structure
  Model = BASETREE
End

ObjectReskin FancyTree BaseTree
  ModelName = FANCYTREE
End
"#;

    let mut parser = IniParser::new();
    let count = parser.parse_ini_content(ini_content, "test.ini").unwrap();

    assert_eq!(count, 2);
    let def = parser.get_definition("FancyTree").unwrap();
    assert_eq!(def.parent_name.as_deref(), Some("BaseTree"));
    assert_eq!(def.model_name.as_deref(), Some("FANCYTREE"));
}

#[test]
fn test_nested_end_does_not_terminate_object() {
    let ini_content = r#"
Object TestStructure
  Draw = W3DModelDraw ModuleTag_01
    ConditionState = NONE
      Model = TESTMODEL
    End
    ConditionState = RUBBLE
      Model = NONE
    End
  End
  KindOf = STRUCTURE SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 1500
  End
End
"#;

    let mut parser = IniParser::new();
    let count = parser
        .parse_ini_content(ini_content, "test_nested.ini")
        .unwrap();

    assert_eq!(count, 1);
    let def = parser.get_definition("TestStructure").unwrap();
    assert_eq!(def.model_name.as_deref(), Some("TESTMODEL"));
    assert_eq!(def.hit_points, Some(1500.0));
    assert_eq!(
        def.attributes.get("KindOf").map(|s| s.as_str()),
        Some("STRUCTURE SELECTABLE")
    );
}

#[test]
fn prerequisites_block_keeps_and_or_object_lines() {
    let ini_content = r#"
Object GLABlackMarket
  KindOf = STRUCTURE
  Prerequisites
    Object = GLAPalace
  End
  BuildCost = 2500
End
Object AmericaStrategyCenter
  KindOf = STRUCTURE
  Prerequisites
    Object = AmericaWarFactory AmericaAirfield
    Object = AmericaCommandCenter
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "prereq_probe.ini")
        .unwrap();
    let market = parser.get_definition("GLABlackMarket").unwrap();
    assert_eq!(
        market.prerequisite_lines,
        vec![("Object".to_string(), "GLAPalace".to_string())]
    );
    assert_eq!(
        market.attributes.get("BuildCost").map(String::as_str),
        Some("2500")
    );
    assert!(
        !market.attributes.contains_key("Object"),
        "Prerequisites Object must not leak into the lossy attribute map"
    );
    let strategy = parser.get_definition("AmericaStrategyCenter").unwrap();
    assert_eq!(
        strategy.prerequisite_lines,
        vec![
            (
                "Object".to_string(),
                "AmericaWarFactory AmericaAirfield".to_string()
            ),
            ("Object".to_string(), "AmericaCommandCenter".to_string()),
        ]
    );
}

fn model_condition_bit(name: &str) -> u128 {
    let index = crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(name)
        .expect("known C++ ModelCondition flag");
    let shift = u32::try_from(index).expect("condition bit fits u32");
    1u128
        .checked_shl(shift)
        .expect("condition bit fits retained u128 bank")
}

#[test]
fn retained_draw_states_select_source_models_with_default_inheritance_and_aliases() {
    let ini_content = r#"
Object ConditionStateProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
    End
    ConditionState = DAMAGED MOVING
      Model = ProbeDamagedMoving
    End
    AliasConditionState REALLYDAMAGED MOVING
    ConditionState = DAMAGED
    End
    ConditionState = RUBBLE
      Model = NONE
    End
    TransitionState = TRANS_Standing TRANS_Moving
      Model = ProbeTransitionOnly
    End
  End
End
"#;

    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "condition_state_probe.ini")
        .expect("parse source Draw state table");
    let definition = parser
        .get_definition("ConditionStateProbe")
        .expect("parsed object definition");

    assert_eq!(definition.draw_modules.len(), 1);
    let module = &definition.draw_modules[0];
    assert_eq!(module.condition_states.len(), 5);
    assert_eq!(
        module.condition_states[1].condition_sets,
        vec![
            vec!["DAMAGED".to_string(), "MOVING".to_string()],
            vec!["REALLYDAMAGED".to_string(), "MOVING".to_string()],
        ],
        "the compact AliasConditionState spelling must retain raw source order"
    );

    assert_eq!(
        definition.select_primary_model_for_conditions(0),
        AuthoredConditionModelSelection::Model("ProbePristine".to_string())
    );
    assert_eq!(
        definition.select_primary_model_for_conditions(model_condition_bit("DAMAGED")),
        AuthoredConditionModelSelection::Model("ProbePristine".to_string()),
        "normal states inherit DefaultConditionState Model exactly like C++"
    );
    assert_eq!(
        definition.select_primary_model_for_conditions(
            model_condition_bit("DAMAGED") | model_condition_bit("MOVING"),
        ),
        AuthoredConditionModelSelection::Model("ProbeDamagedMoving".to_string()),
        "more matching source bits win"
    );
    assert_eq!(
        definition.select_primary_model_for_conditions(
            model_condition_bit("REALLYDAMAGED") | model_condition_bit("MOVING"),
        ),
        AuthoredConditionModelSelection::Model("ProbeDamagedMoving".to_string()),
        "an alias selects its preceding source state"
    );
    assert_eq!(
        definition.select_primary_model_for_conditions(model_condition_bit("RUBBLE")),
        AuthoredConditionModelSelection::Suppressed,
        "Model = NONE must not fall through to a guessed pristine model"
    );
}

#[test]
fn object_scale_parses_decimal_with_retail_inline_comment() {
    let source = r#"
Object ScaledRetailObject
  Scale = .66 ; cinematics use this exact Object INI form
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(source, "scaled_retail_object.ini")
        .expect("parse object scale");
    let definition = parser
        .get_definition("ScaledRetailObject")
        .expect("scaled definition");
    assert!(definition.scale_was_specified);
    assert!((definition.scale - 0.66).abs() < f32::EPSILON);
}

#[test]
fn transition_key_and_acbits_parse_and_play_before_destination() {
    let ini_content = r#"
Object TransitionPlayProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbeIdle
      TransitionKey = TRANS_Standing
      Flags = ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT
    End
    ConditionState = MOVING
      Model = ProbeMove
      TransitionKey = TRANS_Moving
    End
    TransitionState = TRANS_Standing TRANS_Moving
      Model = ProbeStandToMove
      Animation = ProbeHier.StandToMove
      AnimationMode = ONCE
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "transition_play_probe.ini")
        .expect("parse TransitionKey/Flags probe");
    let definition = parser
        .get_definition("TransitionPlayProbe")
        .expect("parsed transition probe");
    let module = &definition.draw_modules[0];
    assert_eq!(module.condition_states[0].transition_key, "trans_standing");
    assert_eq!(
        module.condition_states[0].flags,
        1u32 << ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT
    );
    assert_eq!(module.condition_states[1].transition_key, "trans_moving");
    assert_eq!(
        module.condition_states[1].flags,
        1u32 << ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT,
        "normal states inherit Default Flags like C++"
    );
    assert!(module.condition_states[2].is_transition);
    assert!(module.condition_states[2].transition_key.is_empty());

    let dest = definition
        .select_draw_models_for_conditions(model_condition_bit("MOVING"))
        .expect("dest moving state");
    assert_eq!(dest[0].model_key, "ProbeMove");
    assert!(!dest[0].is_transition);

    let playing = definition
        .select_draw_models_for_conditions_from(model_condition_bit("MOVING"), &[(0, 0, false)])
        .expect("transition should play");
    assert_eq!(playing[0].model_key, "ProbeStandToMove");
    assert!(playing[0].is_transition);
    assert_eq!(playing[0].animations[0].name, "probehier.standtomove");

    let finished = definition
        .select_draw_models_for_conditions_from(
            model_condition_bit("MOVING"),
            &[(0, playing[0].selected_condition_state_index, true)],
        )
        .expect("completed transition yields dest");
    assert_eq!(finished[0].model_key, "ProbeMove");
    assert!(!finished[0].is_transition);

    assert_eq!(construction_percent_height_delta(0.0, 40.0), Some(-40.0));
    assert_eq!(construction_percent_height_delta(25.0, 40.0), Some(-30.0));
    assert_eq!(construction_percent_height_delta(-1.0, 40.0), None);
    assert!(authored_draw_adjusts_height_by_construction(&dest));
}

#[test]
fn retained_draw_modules_select_each_non_suppressed_model_in_source_order() {
    let ini_content = r#"
Object MultiDrawProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbeBody
    End
    ConditionState = DAMAGED
      Model = ProbeBodyDamaged
    End
  End
  Draw = W3DModelDraw ModuleTag_02
    DefaultConditionState
      Model = NONE
    End
  End
  Draw = W3DModelDraw ModuleTag_03
    DefaultConditionState
      Model = ProbeDoor
    End
    ConditionState = DOOR_1_OPENING
      Model = ProbeDoorOpening
    End
  End
End
"#;

    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "multi_draw_probe.ini")
        .expect("parse source Draw modules");
    let definition = parser
        .get_definition("MultiDrawProbe")
        .expect("parsed object definition");

    assert_eq!(
        definition.select_draw_models_for_conditions(0),
        Some(vec![
            AuthoredDrawModel {
                module_index: 0,
                model_key: "ProbeBody".to_string(),
                ..Default::default()
            },
            AuthoredDrawModel {
                module_index: 2,
                model_key: "ProbeDoor".to_string(),
                ..Default::default()
            },
        ]),
        "each selected W3D module must remain distinct and preserve source order"
    );
    assert_eq!(
        definition.select_draw_models_for_conditions(
            model_condition_bit("DAMAGED") | model_condition_bit("DOOR_1_OPENING"),
        ),
        Some(vec![
            AuthoredDrawModel {
                module_index: 0,
                selected_condition_state_index: 1,
                model_key: "ProbeBodyDamaged".to_string(),
                ..Default::default()
            },
            AuthoredDrawModel {
                module_index: 2,
                selected_condition_state_index: 1,
                model_key: "ProbeDoorOpening".to_string(),
                ..Default::default()
            },
        ]),
        "condition matching is independent for every authored Draw module"
    );
}

#[test]
fn w3d_hlod_visibility_draw_states_retain_exact_animation_identity_and_inheritance() {
    let ini_content = r#"
Object DrawAnimationProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
      Animation = ProbeHier.ProbeIdle 0 2
      AnimationMode = LOOP
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
    End
    ConditionState = REALLYDAMAGED
      Model = ProbeReallyDamaged
      IdleAnimation = ProbeHier.ProbeIdleBackwards
      AnimationMode = ONCE_BACKWARDS
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "draw_animation_probe.ini")
        .expect("parse source Draw animation states");
    let definition = parser
        .get_definition("DrawAnimationProbe")
        .expect("parsed Draw animation probe");

    let pristine = definition
        .select_draw_models_for_conditions(0)
        .expect("select default draw state");
    assert_eq!(pristine.len(), 1);
    assert_eq!(pristine[0].animations.len(), 2);
    assert!(
        pristine[0]
            .animations
            .iter()
            .all(|animation| animation.name == "probehier.probeidle")
    );
    assert_eq!(pristine[0].animation_mode, AuthoredDrawAnimationMode::Loop);

    let damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
        .expect("select inherited damaged draw state");
    assert_eq!(damaged[0].model_key, "ProbeDamaged");
    assert_eq!(
        damaged[0].animations, pristine[0].animations,
        "a ConditionState copies Default animations until it authors one"
    );
    assert_eq!(damaged[0].animation_mode, AuthoredDrawAnimationMode::Loop);

    let really_damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
        .expect("select local IdleAnimation state");
    assert_eq!(really_damaged[0].animations.len(), 1);
    assert_eq!(
        really_damaged[0].animations[0],
        AuthoredDrawAnimation {
            name: "probehier.probeidlebackwards".to_string(),
            is_idle: true,
            distance_covered_token: None,
        },
        "first local IdleAnimation replaces Default's repeated entries"
    );
    assert_eq!(
        really_damaged[0].animation_mode,
        AuthoredDrawAnimationMode::OnceBackwards
    );
}

#[test]
fn w3d_hlod_visibility_hide_show_subobjects_inherit_overwrite_and_clear() {
    let ini_content = r#"
Object DrawSubobjectVisibilityProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = VisibilityProbe
      HideSubObject = Hull Turret
      ShowSubObject = turret Door
    End
    ConditionState = WEAPONSET_PLAYER_UPGRADE
      ShowSubObject = Hull
    End
    ConditionState = DAMAGED
      HideSubObject = None IgnoredAfterClear
      HideSubObject = Rack Missile
      ShowSubObject = missile
    End
    TransitionState = TRANS_Standing TRANS_Moving
      HideSubObject = Door
    End
  End
End
"#;

    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "draw_subobject_visibility_probe.ini")
        .expect("parse source Draw subobject state table");
    let definition = parser
        .get_definition("DrawSubobjectVisibilityProbe")
        .expect("parsed Draw subobject visibility probe");

    let default = definition
        .select_draw_models_for_conditions(0)
        .expect("select DefaultConditionState");
    assert_eq!(
        default[0].subobject_visibility,
        vec![
            AuthoredDrawSubobjectVisibility {
                name: "hull".to_string(),
                hidden: true,
            },
            AuthoredDrawSubobjectVisibility {
                name: "turret".to_string(),
                hidden: false,
            },
            AuthoredDrawSubobjectVisibility {
                name: "door".to_string(),
                hidden: false,
            },
        ],
        "one line may contain several names and a duplicate must overwrite in place"
    );

    let upgraded = definition
        .select_draw_models_for_conditions(model_condition_bit("WEAPONSET_PLAYER_UPGRADE"))
        .expect("select inherited player-upgrade state");
    assert_eq!(
        upgraded[0].subobject_visibility,
        vec![
            AuthoredDrawSubobjectVisibility {
                name: "hull".to_string(),
                hidden: false,
            },
            AuthoredDrawSubobjectVisibility {
                name: "turret".to_string(),
                hidden: false,
            },
            AuthoredDrawSubobjectVisibility {
                name: "door".to_string(),
                hidden: false,
            },
        ],
        "normal ConditionState starts from Default then applies its local overwrite"
    );

    let damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
        .expect("select None-cleared damage state");
    assert_eq!(
        damaged[0].subobject_visibility,
        vec![
            AuthoredDrawSubobjectVisibility {
                name: "rack".to_string(),
                hidden: true,
            },
            AuthoredDrawSubobjectVisibility {
                name: "missile".to_string(),
                hidden: false,
            },
        ],
        "None clears every inherited directive and ignores later tokens on its line"
    );

    let transition = definition.draw_modules[0]
        .condition_states
        .iter()
        .find(|state| state.is_transition)
        .expect("retained transition state");
    assert_eq!(
        transition.subobject_visibility,
        vec![
            AuthoredDrawSubobjectVisibility {
                name: "hull".to_string(),
                hidden: true,
            },
            AuthoredDrawSubobjectVisibility {
                name: "turret".to_string(),
                hidden: false,
            },
            AuthoredDrawSubobjectVisibility {
                name: "door".to_string(),
                hidden: true,
            },
        ],
        "TransitionState inherits Default too, then overwrites Door in place"
    );
}

#[test]
fn w3d_hlod_turret_draw_states_inherit_primary_bones_and_art_offsets() {
    let ini_content = r#"
Object DrawTurretProbe
  Draw = W3DTankDraw ModuleTag_01
    DefaultConditionState
      Model = TurretProbe
      Turret = HullYaw
      TurretArtAngle = 90
      TurretPitch = BarrelPitch
      TurretArtPitch = -30
      AltTurret = None
    End
    ConditionState = DAMAGED
      Turret = DamageYaw
      AltTurretPitch = AlternatePitch
    End
    ConditionState = REALLYDAMAGED
      Turret = None
      TurretPitch = NONE
    End
    TransitionState = TRANS_Standing TRANS_Moving
      TurretArtAngle = 45
    End
  End
End
"#;

    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "draw_turret_probe.ini")
        .expect("parse source Draw turret state table");
    let definition = parser
        .get_definition("DrawTurretProbe")
        .expect("parsed Draw turret probe");

    let default = definition
        .select_draw_models_for_conditions(0)
        .expect("select source DefaultConditionState");
    let default_turret = &default[0].primary_turret;
    assert_eq!(default_turret.yaw_bone.as_deref(), Some("hullyaw"));
    assert_eq!(default_turret.pitch_bone.as_deref(), Some("barrelpitch"));
    assert!(
        (default_turret.yaw_art_angle_radians() - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6,
        "C++ INI::parseAngleReal converts source degrees to radians"
    );
    assert!(
        (default_turret.pitch_art_angle_radians() + std::f32::consts::FRAC_PI_6).abs() < 1.0e-6
    );
    assert!(default_turret.primary_fields_valid);
    assert!(!default_turret.has_unsupported_alternate_turret());

    let damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
        .expect("select inherited damaged Draw state");
    let damaged_turret = &damaged[0].primary_turret;
    assert_eq!(damaged_turret.yaw_bone.as_deref(), Some("damageyaw"));
    assert_eq!(damaged_turret.pitch_bone.as_deref(), Some("barrelpitch"));
    assert!(
        (damaged_turret.yaw_art_angle_radians() - default_turret.yaw_art_angle_radians()).abs()
            < 1.0e-6,
        "normal ConditionState inherits Default's primary art offset"
    );
    assert!(
        damaged_turret.has_unsupported_alternate_turret(),
        "an active AltTurretPitch must not be routed into primary control"
    );

    let really_damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
        .expect("select None-cleared turret state");
    assert!(
        !really_damaged[0].primary_turret.has_primary_bone(),
        "C++ parseBoneNameKey clears each explicit None primary binding"
    );

    let transition = definition.draw_modules[0]
        .condition_states
        .iter()
        .find(|state| state.is_transition)
        .expect("retained transition state");
    assert_eq!(
        transition.primary_turret.yaw_bone.as_deref(),
        Some("hullyaw"),
        "TransitionState starts as a DefaultConditionState copy"
    );
    assert!(
        (transition.primary_turret.yaw_art_angle_radians() - std::f32::consts::FRAC_PI_4).abs()
            < 1.0e-6,
        "TransitionState local art angle overwrites its inherited source value"
    );
}

#[test]
fn w3d_hlod_weapon_bones_inherit_clear_exact_slots_and_freeze_state_identity() {
    let ini_content = r#"
Object DrawWeaponBoneProbe
  Draw = W3DTankDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
      WeaponFireFXBone = PRIMARY "Fx Bone"
      WeaponRecoilBone = PRIMARY Recoil
      WeaponMuzzleFlash = PRIMARY MuzzleFX
      WeaponLaunchBone = PRIMARY Launch
      WeaponFireFXBone = SECONDARY SecondaryFx
      WeaponLaunchBone = TERTIARY ThirdLaunch
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
      WeaponRecoilBone = PRIMARY DamageRecoil
      WeaponFireFXBone = SECONDARY None
      WeaponMuzzleFlash = TERTIARY "Third Muzzle FX"
    End
    ConditionState = REALLYDAMAGED
      Model = ProbeReallyDamaged
      WeaponFireFXBone = PRIMARY NONE
      WeaponRecoilBone = PRIMARY NONE
      WeaponMuzzleFlash = PRIMARY NONE
      WeaponLaunchBone = PRIMARY NONE
    End
  End
End
"#;

    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "draw_weapon_bone_probe.ini")
        .expect("parse source Draw weapon-bone states");
    let definition = parser
        .get_definition("DrawWeaponBoneProbe")
        .expect("parsed weapon-bone probe");

    let default = definition
        .select_draw_models_for_conditions(0)
        .expect("select pristine state");
    assert_eq!(default.len(), 1);
    assert_eq!(default[0].selected_condition_state_index, 0);
    let primary = default[0]
        .weapon_bone_bindings
        .slot(0)
        .expect("primary slot");
    assert_eq!(primary.fire_fx_bone_base.as_deref(), Some("fx bone"));
    assert_eq!(primary.recoil_bone_base.as_deref(), Some("recoil"));
    assert_eq!(primary.muzzle_flash_bone_base.as_deref(), Some("muzzlefx"));
    assert_eq!(primary.launch_bone_base.as_deref(), Some("launch"));
    assert_eq!(
        default[0]
            .weapon_bone_bindings
            .slot(1)
            .and_then(|slot| slot.fire_fx_bone_base.as_deref()),
        Some("secondaryfx")
    );
    assert_eq!(
        default[0]
            .weapon_bone_bindings
            .slot(2)
            .and_then(|slot| slot.launch_bone_base.as_deref()),
        Some("thirdlaunch")
    );

    let damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
        .expect("select damaged state");
    assert_eq!(damaged.len(), 1);
    assert_eq!(damaged[0].selected_condition_state_index, 1);
    let damaged_primary = damaged[0]
        .weapon_bone_bindings
        .slot(0)
        .expect("inherited primary slot");
    assert_eq!(
        damaged_primary.fire_fx_bone_base.as_deref(),
        Some("fx bone")
    );
    assert_eq!(
        damaged_primary.recoil_bone_base.as_deref(),
        Some("damagerecoil"),
        "a local field overwrites just its own inherited C++ slot base"
    );
    assert_eq!(
        damaged[0]
            .weapon_bone_bindings
            .slot(1)
            .and_then(|slot| slot.fire_fx_bone_base.as_deref()),
        None,
        "None clears only the exact declared SECONDARY source field"
    );
    assert_eq!(
        damaged[0]
            .weapon_bone_bindings
            .slot(2)
            .and_then(|slot| slot.launch_bone_base.as_deref()),
        Some("thirdlaunch"),
        "unrelated slots remain inherited"
    );
    assert_eq!(
        damaged[0]
            .weapon_bone_bindings
            .slot(2)
            .and_then(|slot| slot.muzzle_flash_bone_base.as_deref()),
        Some("third muzzle fx"),
        "quoted C++ AsciiString bone names retain their full lowercased identity"
    );

    let really_damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
        .expect("select really damaged state");
    assert_eq!(really_damaged[0].selected_condition_state_index, 2);
    assert!(
        really_damaged[0]
            .weapon_bone_bindings
            .slot(0)
            .is_some_and(|slot| {
                slot.fire_fx_bone_base.is_none()
                    && slot.recoil_bone_base.is_none()
                    && slot.muzzle_flash_bone_base.is_none()
                    && slot.launch_bone_base.is_none()
            })
    );
    assert!(really_damaged[0].weapon_bone_bindings.source_fields_valid);
}

#[test]
fn w3d_projectile_bone_feedback_keeps_module_mask_and_state_override_identity() {
    let ini_content = r#"
Object DrawProjectileFeedbackProbe
  Draw = W3DModelDraw ModuleTag_01
    ProjectileBoneFeedbackEnabledSlots = PRIMARY SECONDARY
    ProjectileBoneFeedbackEnabledSlots = +TERTIARY -SECONDARY
    DefaultConditionState
      Model = ProbePristine
      WeaponLaunchBone = PRIMARY Rack
      WeaponHideShowBone = PRIMARY "Missile Bay"
      WeaponLaunchBone = TERTIARY ThirdRack
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
      WeaponHideShowBone = PRIMARY None
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "draw_projectile_feedback_probe.ini")
        .expect("parse source projectile feedback records");
    let definition = parser
        .get_definition("DrawProjectileFeedbackProbe")
        .expect("parsed projectile feedback probe");

    let pristine = definition
        .select_draw_models_for_conditions(0)
        .expect("select pristine draw state")
        .pop()
        .expect("one source Draw module");
    assert_eq!(pristine.projectile_bone_feedback.enabled_slots, 0b101);
    assert!(pristine.projectile_bone_feedback.source_fields_valid);
    assert!(pristine.projectile_bone_feedback.is_enabled_for_slot(0));
    assert!(!pristine.projectile_bone_feedback.is_enabled_for_slot(1));
    assert!(pristine.projectile_bone_feedback.is_enabled_for_slot(2));
    assert_eq!(
        pristine
            .weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
        Some("missile bay"),
        "C++ parseWeaponBoneName preserves one lowercased exact override name"
    );

    let damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
        .expect("select damaged draw state")
        .pop()
        .expect("one source Draw module");
    assert_eq!(
        damaged.projectile_bone_feedback,
        pristine.projectile_bone_feedback
    );
    assert_eq!(
        damaged
            .weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.launch_bone_base.as_deref()),
        Some("rack"),
        "ConditionState inherits Default's unrelated launch bone"
    );
    assert_eq!(
        damaged
            .weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
        None,
        "an explicit C++ None clears only the selected state's override bone"
    );
}

#[test]
fn w3d_projectile_bone_feedback_fails_closed_for_malformed_module_input() {
    let ini_content = r#"
Object MalformedProjectileFeedbackProbe
  Draw = W3DModelDraw ModuleTag_01
    ProjectileBoneFeedbackEnabledSlots = PRIMARY SECONDARY
    DefaultConditionState
      Model = Probe
      ProjectileBoneFeedbackEnabledSlots = TERTIARY
      WeaponLaunchBone = PRIMARY Rack
    End
  End
End

Object MixedProjectileFeedbackProbe
  Draw = W3DModelDraw ModuleTag_01
    ProjectileBoneFeedbackEnabledSlots = PRIMARY +SECONDARY
    DefaultConditionState
      Model = Probe
      WeaponLaunchBone = PRIMARY Rack
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "malformed_projectile_feedback_probe.ini")
        .expect("retain malformed source records for fail-closed selection");

    for object_name in [
        "MalformedProjectileFeedbackProbe",
        "MixedProjectileFeedbackProbe",
    ] {
        let draw = parser
            .get_definition(object_name)
            .expect("parsed malformed projectile feedback probe")
            .select_draw_models_for_conditions(0)
            .expect("source still selects its model")
            .pop()
            .expect("one Draw module");
        assert!(
            !draw.projectile_bone_feedback.source_fields_valid,
            "{object_name} must not turn malformed module input into enabled visual feedback"
        );
    }
}

#[test]
fn w3d_projectile_bone_feedback_retail_tomahawk_scorpion_and_scud_keep_exact_slots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ini_root = [
        root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object"),
        root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object"),
    ]
    .into_iter()
    .find(|candidate| {
        candidate.join("AmericaVehicle.ini").is_file()
            && candidate.join("GLAVehicle.ini").is_file()
            && candidate.join("FactionBuilding.ini").is_file()
    });
    let Some(ini_root) = ini_root else {
        eprintln!(
            "skip: retail AmericaVehicle.ini/GLAVehicle.ini/FactionBuilding.ini are not available on disk"
        );
        return;
    };

    let mut parser = IniParser::new();
    for filename in [
        "AmericaVehicle.ini",
        "GLAVehicle.ini",
        "FactionBuilding.ini",
    ] {
        let path = ini_root.join(filename);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read retail {}: {error}", path.display()));
        parser
            .parse_ini_content(&content, filename)
            .unwrap_or_else(|error| panic!("parse retail {filename}: {error}"));
    }

    let tomahawk = parser
        .get_definition("AmericaVehicleTomahawk")
        .expect("retail Tomahawk definition")
        .select_draw_models_for_conditions(0)
        .expect("retail Tomahawk default state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("AVTomahawk"))
        .expect("retail Tomahawk model");
    assert_eq!(tomahawk.projectile_bone_feedback.enabled_slots, 0b001);
    assert_eq!(
        tomahawk
            .weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
        Some("missile"),
        "retail Tomahawk uses one direct MISSILE child, not missile01"
    );
    let tomahawk_damaged = parser
        .get_definition("AmericaVehicleTomahawk")
        .expect("retail Tomahawk definition")
        .select_draw_models_for_conditions(model_condition_bit("REALLYDAMAGED"))
        .expect("retail damaged Tomahawk state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("AVTomahawk_D"))
        .expect("retail damaged Tomahawk model");
    assert_eq!(
        tomahawk_damaged
            .weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.projectile_hide_show_bone.as_deref()),
        Some("missile"),
        "C++ normal ConditionState begins as a DefaultConditionState copy"
    );

    let scorpion = parser
        .get_definition("GLATankScorpion")
        .expect("retail Scorpion definition")
        .select_draw_models_for_conditions(model_condition_bit("WEAPONSET_PLAYER_UPGRADE"))
        .expect("retail upgraded Scorpion state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
        .expect("retail upgraded Scorpion model");
    assert_eq!(scorpion.projectile_bone_feedback.enabled_slots, 0b010);
    assert_eq!(
        scorpion
            .weapon_bone_bindings
            .slot(1)
            .and_then(|slot| slot.launch_bone_base.as_deref()),
        Some("weapona"),
        "retail Scorpion feedback belongs to its SECONDARY missile slot"
    );
    assert!(
        scorpion
            .weapon_bone_bindings
            .slot(1)
            .and_then(|slot| slot.projectile_hide_show_bone.as_deref())
            .is_none(),
        "without a C++ override the renderer must use numbered WeaponA01..NN children"
    );

    let scud = parser
        .get_definition("GLAScudStorm")
        .expect("retail Scud Storm definition")
        .select_draw_models_for_conditions(model_condition_bit("ATTACKING"))
        .expect("retail attacking Scud Storm state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UBScudStrm_A2"))
        .expect("retail attacking Scud Storm model");
    assert_eq!(scud.projectile_bone_feedback.enabled_slots, 0b001);
    assert_eq!(
        scud.weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.launch_bone_base.as_deref()),
        Some("weapona"),
        "retail Scud Storm feedback keeps its PRIMARY WeaponA launch base"
    );
    assert!(
        scud.weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.projectile_hide_show_bone.as_deref())
            .is_none(),
        "retail Scud Storm relies on exact WeaponA01 through WeaponA09 children"
    );
}

#[test]
fn w3d_hlod_weapon_bones_fail_closed_for_unknown_slot_token() {
    let ini_content = r#"
Object MalformedWeaponBoneProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = Probe
      WeaponFireFXBone = QUATERNARY InventedBone
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "malformed_weapon_bone_probe.ini")
        .expect("retain malformed source record for fail-closed selection");
    let draw = parser
        .get_definition("MalformedWeaponBoneProbe")
        .expect("parsed malformed weapon-bone probe")
        .select_draw_models_for_conditions(0)
        .expect("source still has a selected model")
        .pop()
        .expect("one Draw module");
    assert!(
        !draw.weapon_bone_bindings.source_fields_valid,
        "an unsupported WeaponSlotType must disable later recoil/topology use rather than alias PRIMARY"
    );
}

#[test]
fn w3d_hlod_weapon_bones_retail_scorpion_preserves_distinct_default_and_upgrade_slots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ini_path = [
        root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object/GLAVehicle.ini"),
        root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/GLAVehicle.ini"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file());
    let Some(ini_path) = ini_path else {
        eprintln!("skip: retail GLAVehicle.ini is not available on disk");
        return;
    };
    let ini_content =
        std::fs::read_to_string(&ini_path).expect("read retail GLAVehicle source Object INI");
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(&ini_content, "GLAVehicle.ini")
        .expect("parse retail Scorpion Draw states");
    let scorpion = parser
        .get_definition("GLATankScorpion")
        .expect("retail Scorpion definition");
    let pristine = scorpion
        .select_draw_models_for_conditions(0)
        .expect("retail pristine Scorpion Draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
        .expect("retail pristine Scorpion uses UVLiteTank");
    let upgrade_bits = model_condition_bit("WEAPONSET_PLAYER_UPGRADE");
    let upgraded = scorpion
        .select_draw_models_for_conditions(upgrade_bits)
        .expect("retail upgrade Scorpion Draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
        .expect("retail upgrade Scorpion keeps UVLiteTank");

    assert_ne!(
        pristine.selected_condition_state_index, upgraded.selected_condition_state_index,
        "same retail mesh is used by distinct selected source states and cannot identify recoil topology by basename"
    );
    let pristine_primary = pristine
        .weapon_bone_bindings
        .slot(0)
        .expect("retail pristine PRIMARY");
    assert_eq!(
        pristine_primary.fire_fx_bone_base.as_deref(),
        Some("muzzle")
    );
    assert_eq!(pristine_primary.recoil_bone_base.as_deref(), Some("barrel"));
    assert_eq!(
        pristine_primary.muzzle_flash_bone_base.as_deref(),
        Some("muzzlefx")
    );
    assert_eq!(pristine_primary.launch_bone_base.as_deref(), Some("muzzle"));
    let upgraded_secondary = upgraded
        .weapon_bone_bindings
        .slot(1)
        .expect("retail upgraded SECONDARY");
    assert_eq!(
        upgraded_secondary.fire_fx_bone_base.as_deref(),
        Some("weapona")
    );
    assert_eq!(
        upgraded_secondary.launch_bone_base.as_deref(),
        Some("weapona")
    );
    assert_eq!(upgraded_secondary.recoil_bone_base, None);
    assert_eq!(upgraded_secondary.muzzle_flash_bone_base, None);
    assert_eq!(
        upgraded
            .weapon_bone_bindings
            .slot(0)
            .and_then(|slot| slot.recoil_bone_base.as_deref()),
        Some("barrel"),
        "the upgrade state preserves DefaultConditionState PRIMARY topology while adding exact SECONDARY launch data"
    );
}

#[test]
fn w3d_hlod_recoil_kinematics_freeze_module_defaults_and_velocity_overrides() {
    let ini_content = r#"
Object DrawRecoilProbe
  Draw = W3DModelDraw ModuleTag_01
    InitialRecoilSpeed = 120
    MaxRecoilDistance = 8
    RecoilDamping = .25
    RecoilSettleSpeed = 6
    DefaultConditionState
      Model = ProbePristine
    End
    ConditionState = DAMAGED
      Model = ProbeDamaged
    End
  End
  Draw = W3DModelDraw ModuleTag_02
    DefaultConditionState
      Model = CppDefaultProbe
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "draw_recoil_probe.ini")
        .expect("parse source Draw recoil module data");
    let definition = parser
        .get_definition("DrawRecoilProbe")
        .expect("parsed recoil definition");

    let pristine = definition
        .select_draw_models_for_conditions(0)
        .expect("select pristine Draw modules");
    let damaged = definition
        .select_draw_models_for_conditions(model_condition_bit("DAMAGED"))
        .expect("select damaged Draw modules");
    let pristine_recoil = &pristine[0].recoil_kinematics;
    let damaged_recoil = &damaged[0].recoil_kinematics;
    assert!(pristine_recoil.is_visual_usable());
    assert_eq!(pristine_recoil, damaged_recoil);
    assert_eq!(
        pristine_recoil.initial_recoil_per_logic_frame().to_bits(),
        (120.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits(),
        "C++ INI::parseVelocityReal divides authored recoil speed by logic FPS"
    );
    assert_eq!(pristine_recoil.max_recoil_distance(), 8.0);
    assert_eq!(pristine_recoil.recoil_damping(), 0.25);
    assert_eq!(
        pristine_recoil.recoil_settle_per_logic_frame().to_bits(),
        (6.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits(),
        "C++ parses settle speed through the same velocity conversion"
    );

    let cpp_defaults = &pristine[1].recoil_kinematics;
    assert!(cpp_defaults.is_visual_usable());
    assert_eq!(cpp_defaults.initial_recoil_per_logic_frame(), 2.0);
    assert_eq!(cpp_defaults.max_recoil_distance(), 3.0);
    assert_eq!(cpp_defaults.recoil_damping(), 0.4);
    assert_eq!(cpp_defaults.recoil_settle_per_logic_frame(), 0.065);
    assert_ne!(
        cpp_defaults.initial_recoil_per_logic_frame().to_bits(),
        (2.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits(),
        "constructor defaults are already stored values, not source velocity overrides"
    );
}

#[test]
fn w3d_hlod_recoil_kinematics_fail_closed_for_bad_or_nested_source() {
    let ini_content = r#"
Object BadDrawRecoilProbe
  Draw = W3DModelDraw ModuleTag_01
    InitialRecoilSpeed = not_a_number
    DefaultConditionState
      Model = Probe
      MaxRecoilDistance = 9
    End
  End
End

Object NonFiniteDrawRecoilProbe
  Draw = W3DModelDraw ModuleTag_01
    InitialRecoilSpeed = NaN
    DefaultConditionState
      Model = Probe
    End
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "bad_draw_recoil_probe.ini")
        .expect("retain bad source data for a fail-closed presentation decision");

    let bad = parser
        .get_definition("BadDrawRecoilProbe")
        .expect("bad recoil definition")
        .select_draw_models_for_conditions(0)
        .expect("select bad recoil Draw state")
        .pop()
        .expect("one bad recoil Draw module");
    assert!(
        !bad.recoil_kinematics.source_fields_valid && !bad.recoil_kinematics.is_visual_usable(),
        "unknown numeric and nested module-only source fields must not silently use C++ defaults"
    );

    let nonfinite = parser
        .get_definition("NonFiniteDrawRecoilProbe")
        .expect("non-finite recoil definition")
        .select_draw_models_for_conditions(0)
        .expect("select non-finite recoil Draw state")
        .pop()
        .expect("one non-finite recoil Draw module");
    assert!(nonfinite.recoil_kinematics.source_fields_valid);
    assert!(
        !nonfinite.recoil_kinematics.is_visual_usable(),
        "a parsed NaN remains retained but cannot authorize visual recoil"
    );
}

#[test]
fn w3d_hlod_recoil_kinematics_retail_nuke_and_sentry_keep_distinct_overrides() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ini_root = [
        root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object"),
        root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object"),
    ]
    .into_iter()
    .find(|candidate| {
        candidate.join("ChinaVehicle.ini").is_file()
            && candidate.join("AmericaVehicle.ini").is_file()
    });
    let Some(ini_root) = ini_root else {
        eprintln!("skip: retail ChinaVehicle.ini/AmericaVehicle.ini are not available on disk");
        return;
    };

    let mut parser = IniParser::new();
    for filename in ["ChinaVehicle.ini", "AmericaVehicle.ini"] {
        let path = ini_root.join(filename);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read retail {}: {error}", path.display()));
        parser
            .parse_ini_content(&content, filename)
            .unwrap_or_else(|error| panic!("parse retail {filename}: {error}"));
    }

    let nuke = parser
        .get_definition("ChinaVehicleNukeLauncher")
        .expect("retail China Nuke Cannon")
        .select_draw_models_for_conditions(0)
        .expect("select retail Nuke Cannon Draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("NVNukeCn"))
        .expect("retail Nuke Cannon W3D module");
    let sentry = parser
        .get_definition("AmericaVehicleSentryDrone")
        .expect("retail Sentry Drone")
        .select_draw_models_for_conditions(0)
        .expect("select retail Sentry Drone Draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("AVSENTRY"))
        .expect("retail Sentry Drone W3D module");

    assert!(nuke.recoil_kinematics.is_visual_usable());
    assert!(sentry.recoil_kinematics.is_visual_usable());
    assert_eq!(
        nuke.recoil_kinematics
            .initial_recoil_per_logic_frame()
            .to_bits(),
        (120.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
    );
    assert_eq!(nuke.recoil_kinematics.max_recoil_distance(), 8.0);
    assert_eq!(
        nuke.recoil_kinematics
            .recoil_settle_per_logic_frame()
            .to_bits(),
        (6.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
    );
    assert_eq!(nuke.recoil_kinematics.recoil_damping(), 0.4);

    assert_eq!(
        sentry
            .recoil_kinematics
            .initial_recoil_per_logic_frame()
            .to_bits(),
        (10.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
    );
    assert_eq!(sentry.recoil_kinematics.max_recoil_distance(), 1.5);
    assert_eq!(
        sentry
            .recoil_kinematics
            .recoil_settle_per_logic_frame()
            .to_bits(),
        (3.0 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL).to_bits()
    );
    assert_ne!(
        nuke.recoil_kinematics, sentry.recoil_kinematics,
        "retail source values must remain module identity, not a fixed recoil preset"
    );
}

#[test]
fn unknown_source_condition_token_fails_closed_instead_of_selecting_default() {
    let ini_content = r#"
Object UnsupportedConditionProbe
  Draw = W3DModelDraw ModuleTag_01
    DefaultConditionState
      Model = ProbePristine
    End
    ConditionState = PORT_ONLY_CONDITION
      Model = WouldBeWrongToGuess
    End
  End
End
"#;

    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "unsupported_condition_probe.ini")
        .expect("parse source Draw state table");
    let definition = parser
        .get_definition("UnsupportedConditionProbe")
        .expect("parsed object definition");
    assert_eq!(
        definition.select_primary_model_for_conditions(0),
        AuthoredConditionModelSelection::Unresolved,
        "unsupported source state must not silently disappear during matching"
    );
}

#[test]
fn test_child_object_header_parsing() {
    let ini_content = r#"
ChildObject ChildTemplate ParentTemplate
  Model = CHILDMODEL
End
"#;

    let mut parser = IniParser::new();
    let count = parser.parse_ini_content(ini_content, "child.ini").unwrap();
    assert_eq!(count, 1);
    let def = parser.get_definition("ChildTemplate").unwrap();
    assert_eq!(def.model_name.as_deref(), Some("CHILDMODEL"));
}

#[test]
fn test_modelname_and_draw_parse() {
    let ini_content = r#"
ObjectReskin Bush08 Bush01
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTBush08
    TextureName = PTBush01.tga
  End
End
"#;

    let mut parser = IniParser::new();
    let count = parser
        .parse_ini_content(ini_content, "nature_prop.ini")
        .unwrap();
    assert_eq!(count, 1);
    let def = parser.get_definition("Bush08").unwrap();
    assert_eq!(def.model_name.as_deref(), Some("PTBush08"));
    assert_eq!(def.draw_module.as_deref(), Some("W3DTreeDraw ModuleTag_01"));
    assert_eq!(
        def.textures.get("TextureName").map(|s| s.as_str()),
        Some("PTBush01.tga")
    );
    assert!(!def.textures.contains_key("texturename"));
}

#[test]
fn tree_spruce03_object_reskin_uses_modelname_not_object_name() {
    let ini_content = r#"
Object GenericOptTree
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTDogwod01
    TextureName = PTDogwod01.tga
  End
End

ObjectReskin TreeSpruce03 GenericOptTree
  Draw = W3DTreeDraw ModuleTag_01
    ModelName = PTXPine03
    TextureName = PTXPine03.tga
  End
End
"#;

    let mut parser = IniParser::new();
    let count = parser
        .parse_ini_content(ini_content, "NatureProp.ini")
        .unwrap();
    assert_eq!(count, 2);
    let def = parser.get_definition("TreeSpruce03").unwrap();
    assert_eq!(def.model_name.as_deref(), Some("PTXPine03"));
    assert_eq!(
        def.select_primary_model_for_conditions(0),
        AuthoredConditionModelSelection::Model("PTXPine03".to_string()),
        "W3DTreeDraw ModelName must be the selectable mesh, not TreeSpruce03"
    );
    let draw_models = def
        .select_draw_models_for_conditions(0)
        .expect("W3DTreeDraw ModelName is selectable Draw state");
    assert_eq!(draw_models.len(), 1);
    assert_eq!(draw_models[0].model_key, "PTXPine03");
}

#[test]
fn test_object_assignment_does_not_start_template() {
    let ini_content = r#"
Object TestStructure
  Behavior = GrantScienceUpgrade ModuleTag_Science
    GrantScience = SCIENCE_Test
    Object = TestHelperObject
  End
End
"#;

    let mut parser = IniParser::new();
    let count = parser
        .parse_ini_content(ini_content, "object_assignment.ini")
        .unwrap();

    assert_eq!(count, 1);
    assert!(parser.get_definition("TestStructure").is_some());
    assert!(parser.get_definition("=").is_none());
}

#[test]
fn parses_each_concrete_weapon_slot_without_overwrite() {
    let ini_content = r#"
Object USA_Ranger
  Type = Infantry
  Weapon = PRIMARY AmericaRangerMachineGun
  Weapon = SECONDARY AmericaRangerFlashBangGrenade
  Weapon = TERTIARY AmericaRangerTertiaryTest
  HitPoints = 120
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "weapon_primary.ini")
        .unwrap();
    let def = parser.get_definition("USA_Ranger").expect("def");
    assert_eq!(
        def.primary_weapon.as_deref(),
        Some("AmericaRangerMachineGun"),
        "PRIMARY must stick when SECONDARY follows"
    );
    assert_eq!(
        def.secondary_weapon.as_deref(),
        Some("AmericaRangerFlashBangGrenade"),
        "SECONDARY must be recorded independently of PRIMARY"
    );
    assert_eq!(
        def.tertiary_weapon.as_deref(),
        Some("AmericaRangerTertiaryTest"),
        "TERTIARY must remain a concrete third slot"
    );
}

#[test]
fn parses_secondary_none_does_not_register() {
    let ini_content = r#"
Object GLA_Scorpion
  Type = Vehicle
  Weapon = PRIMARY ScorpionTankGun
  Weapon = SECONDARY None
  Weapon = TERTIARY None
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "weapon_secondary_none.ini")
        .unwrap();
    let def = parser.get_definition("GLA_Scorpion").expect("def");
    assert_eq!(def.primary_weapon.as_deref(), Some("ScorpionTankGun"));
    assert!(
        def.secondary_weapon.is_none(),
        "SECONDARY None must fail-closed (no name)"
    );
    assert!(
        def.tertiary_weapon.is_none(),
        "TERTIARY None must fail-closed (no name)"
    );
}

#[test]
fn nested_weapon_sets_preserve_the_retail_mine_detail_row_without_flattening_it() {
    let ini_content = r#"
Object AmericaVehicleDozer
  WeaponSet
    Conditions = None
    Weapon = PRIMARY None
  End
  WeaponSet
    Conditions = MINE_CLEARING_DETAIL
    Weapon = PRIMARY DozerMineDisarmingWeapon
  End
End
"#;
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(ini_content, "dozer_mine_weapon_set.ini")
        .expect("parse mine detail WeaponSet");
    let definition = parser
        .get_definition("AmericaVehicleDozer")
        .expect("dozer definition");

    assert_eq!(definition.weapon_sets.len(), 2);
    assert!(definition.base_weapon_name(0).is_none());
    assert_eq!(
        definition.mine_clearing_primary_weapon_name(),
        Some("DozerMineDisarmingWeapon")
    );
    assert!(
        definition.primary_weapon.is_none() && !definition.attributes.contains_key("Weapon"),
        "nested Weapon rows must not overwrite the legacy top-level view"
    );
}

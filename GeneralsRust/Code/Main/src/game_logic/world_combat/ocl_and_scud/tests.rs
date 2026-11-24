//! Host OCL execution tests.

use super::*;
use crate::game_logic::ThingTemplate;

#[test]
fn receiving_ocl_puts_a_child_in_its_new_parachute_before_setting_the_final_pose() {
    use crate::assets::pristine_bones::{PristineBoneAssets, PristineBoneSource};
    use crate::game_logic::{Player, ScriptPresentationEffects};
    use game_engine::common::random_value::SessionRandom;
    use gamelogic::object_creation_list::GenericObjectCreationNugget;

    struct NoBones;
    impl PristineBoneAssets for NoBones {
        fn pristine_bones_for_draws(
            &self,
            _: &PristineBoneSource<'_>,
            _: &str,
            _: i32,
            _: usize,
        ) -> Result<Vec<glam::Mat4>, String> {
            Ok(Vec::new())
        }
    }

    const OCL: &str = "ContainedEjection";
    let origin = Vec3::new(12.0, 40.0, 30.0);
    let fixture = |mask: &str, container: &str| {
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([(
                "ocl-parachute.ini".into(),
                format!(
                    "Object Source
 KindOf = VEHICLE
End
Object Canopy
 KindOf = PARACHUTE
 Behavior = ParachuteContain Chute
  AllowInsideKindOf = {mask}
  PitchRateMax = 0
  RollRateMax = 0
 End
 Behavior = PhysicsBehavior Physics
  Mass = 10
 End
End
Object Rider
 KindOf = INFANTRY
 Behavior = PhysicsBehavior Physics
  Mass = 5
 End
End"
                ),
            )])
            .unwrap();
        world
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList {OCL}
 CreateObject
  ObjectNames = Rider
  PutInContainer = {container}
  Count = 2
  IgnorePrimaryObstacle = Yes
  RequiresLivePlayer = Yes
  Disposition = RANDOM_FORCE
  MinForceMagnitude = 10
  MaxForceMagnitude = 12
  MinForcePitch = 50
  MaxForcePitch = 60
  SpinRate = 0
  Offset = X:1 Y:2 Z:3
 End
End"
            )])
            .unwrap();
        world.add_player(Player::new(1, Team::USA, "Player", true).with_script_identity("Player"));
        let source = world.create_object_for_player("Source", 1, origin).unwrap();
        (world, source)
    };

    let (mut owner, source) = fixture("INFANTRY", "Canopy");
    let (mut peer, peer_source) = fixture("VEHICLE", "Canopy");
    assert_eq!(source, peer_source);
    let random = SessionRandom::from_state(owner.logic_random.snapshot());
    let list = owner.object_creation_list_definition(OCL).unwrap();
    let nugget = list.nuggets()[0]
        .as_any()
        .downcast_ref::<GenericObjectCreationNugget>()
        .unwrap();
    let execute = |world: &mut GameLogic, source| {
        world.execute_receiving_object_ocl_with_births(
            OCL,
            source,
            None,
            &mut ScriptPresentationEffects {
                pristine_bones: Some(&NoBones),
                ..Default::default()
            },
        )
    };
    let result = execute(&mut owner, source);
    assert!(result.error.is_none(), "{:?}", result.error);
    let [(chute, _), (attached, _), (overflow, _)] = result.created.as_slice() else {
        panic!("C++ creates the container first, then both candidate riders");
    };
    assert_eq!(owner.objects[chute].template_name, "Canopy");
    assert_eq!(owner.objects[attached].contained_by, Some(*chute));
    assert!(owner.objects[attached].is_held_disabled());
    assert_eq!(owner.objects[chute].contained_units(), vec![*attached]);
    assert_eq!(owner.objects[overflow].contained_by, None);
    assert!(!owner.objects[overflow].is_held_disabled());
    for id in [chute, attached, overflow] {
        let object = &owner.objects[id];
        assert_eq!(object.producer_id, Some(source));
        assert_eq!(object.owner_player_id, Some(1));
        assert_eq!(object.get_position(), origin + Vec3::new(1.0, 3.0, 2.0));
        assert_eq!(object.ignore_collisions_with, Some(source));
    }
    for id in [attached, overflow, chute] {
        if id != chute {
            random.next_int(0, 0);
        }
        let (force, _) = nugget.draw_flight(|min, max| random.next_real(min, max));
        let object = &owner.objects[id];
        let actual = object.physics_accel * object.physics_get_mass();
        assert!(actual.abs_diff_eq(Vec3::new(force.x, force.z, force.y), 1.0e-5));
    }
    assert_eq!(owner.logic_random.snapshot(), random.snapshot());

    assert!(!peer.objects.contains_key(chute));
    let peer_result = execute(&mut peer, peer_source);
    assert!(peer_result.error.is_none(), "{:?}", peer_result.error);
    assert_eq!(peer_result.created.len(), 3);
    for (id, _) in peer_result.created {
        assert_eq!(peer.objects[&id].contained_by, None);
        assert!(peer.objects[&id].contained_units().is_empty());
    }

    let (mut missing, missing_source) = fixture("INFANTRY", "Missing");
    let result = execute(&mut missing, missing_source);
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.created.len(), 2);
    for (id, _) in result.created {
        assert_eq!(missing.objects[&id].template_name, "Rider");
        assert_eq!(missing.objects[&id].contained_by, None);
    }
}

#[test]
fn receiving_ocl_requires_an_active_controlling_player_before_drawing_randomness() {
    use crate::game_logic::Player;
    use crate::game_logic::game_logic::PlayerSideRole;
    use game_engine::common::random_value::SessionRandom;

    const LIST: &str = "OCL_ActiveOwner";
    const OWNER: u32 = 7;
    const NEUTRAL: u32 = 0;
    let fixture = |player: Option<(bool, bool)>| {
        let mut world = GameLogic::new();
        for name in ["Source", "Gated", "Ungated"] {
            world
                .templates
                .insert(name.into(), ThingTemplate::new(name));
        }
        world
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList {LIST}
 CreateObject
  ObjectNames = Gated
  Disposition = LIKE_EXISTING
  RequiresLivePlayer = Yes
 End
 CreateObject
  ObjectNames = Ungated
  Disposition = LIKE_EXISTING
 End
End"
            )])
            .unwrap();
        let source = if let Some((alive, observer)) = player {
            let mut neutral = Player::new(NEUTRAL, Team::Neutral, "", false);
            neutral.map_side.role = PlayerSideRole::Neutral;
            world.add_player(neutral);
            let player = Player::new(OWNER, Team::USA, "Owner", true).with_script_identity("Owner");
            world.add_player(player);
            let source = world
                .create_object_for_player("Source", OWNER, Vec3::ZERO)
                .unwrap();
            // A source created after defeat would already belong to Neutral.
            let player = world.get_player_mut(OWNER).unwrap();
            player.is_alive = alive;
            player.is_observer = observer;
            source
        } else {
            world
                .create_object("Source", Team::Neutral, Vec3::ZERO)
                .unwrap()
        };
        assert_eq!(
            world.objects[&source].owner_player_id,
            player.map(|_| OWNER)
        );
        (world, source)
    };
    let verify = |world: &mut GameLogic, source, expected_names: &[&str], expected_owner| {
        let expected_random = SessionRandom::from_state(world.logic_random.snapshot());
        for _ in expected_names {
            expected_random.next_int(0, 0);
        }

        let outcome = world.execute_receiving_object_ocl_with_births(
            LIST,
            source,
            None,
            &mut Default::default(),
        );
        assert!(outcome.error.is_none());
        let names: Vec<_> = outcome
            .created
            .iter()
            .map(|(id, _)| {
                let child = &world.objects[id];
                assert_eq!(child.producer_id, Some(source));
                assert_eq!(child.owner_player_id, expected_owner);
                child.template_name.as_str()
            })
            .collect();
        assert_eq!(names, expected_names);
        assert_eq!(world.logic_random.snapshot(), expected_random.snapshot());
    };

    let (mut active, source) = fixture(Some((true, false)));
    for player in [Some((false, false)), Some((true, true)), None] {
        let (mut peer, peer_source) = fixture(player);
        assert_eq!(source, peer_source);
        verify(&mut active, source, &["Gated", "Ungated"], Some(OWNER));
        verify(
            &mut peer,
            peer_source,
            &["Ungated"],
            player.map(|_| NEUTRAL),
        );
    }
}

#[test]
fn fire_ocl_never_uses_a_foreign_list_or_foreign_object_template() {
    const NAME: &str = "OCL_WeaponOwnerNegativeControl";
    let text = |child: &str| {
        format!(
            "ObjectCreationList {NAME}\n CreateObject\n ObjectNames = {child}\n Count = 1\n End\nEnd\n"
        )
    };
    let foreign = gamelogic::object_creation_list::store::parse_object_creation_lists_from_ini(
        &text("ForeignChild"),
    )
    .unwrap();
    for (name, list) in foreign {
        gamelogic::helpers::TheObjectCreationListStore::register_object_creation_list(&name, list);
    }
    let mut first = GameLogic::new();
    let mut peer = GameLogic::new();
    for world in [&mut first, &mut peer] {
        for name in ["SameSource", "OwnedChild", "ForeignChild"] {
            world
                .templates
                .insert(name.into(), ThingTemplate::new(name));
        }
    }
    peer.apply_receiving_ocl_overrides(&[text("OwnedChild")])
        .unwrap();
    let first_id = first
        .create_object("SameSource", Team::USA, Vec3::ZERO)
        .unwrap();
    let peer_id = peer
        .create_object("SameSource", Team::USA, Vec3::ZERO)
        .unwrap();
    assert_eq!(first_id, peer_id);
    let execute = |world: &mut GameLogic, id| {
        world.execute_parsed_weapon_ocl_at(
            NAME,
            Some(id),
            Team::USA,
            VeterancyLevel::Rookie,
            0.0,
            Vec3::ZERO,
            Vec3::ZERO,
        ).unwrap()
    };
    assert!(execute(&mut first, first_id).is_empty());
    let children = execute(&mut peer, peer_id);
    assert_eq!(children.len(), 1);
    assert_eq!(peer.objects[&children[0]].template_name, "OwnedChild");
    peer.templates.remove("OwnedChild");
    assert!(execute(&mut peer, peer_id).is_empty());
}
#[test]
fn receiving_ocl_effects_follow_overlays_and_world_reset() {
    let text = |child: &str| format!(
        "ObjectCreationList SameEffect\n CreateObject\n ObjectNames = {child}\n Count = 1\n End\nEnd\n"
    );
    let mut left = GameLogic::new();
    let mut right = GameLogic::new();
    left.admit_ocl_application_sources(&[text("LeftBase")]).unwrap();
    right.admit_ocl_application_sources(&[text("RightBase")]).unwrap();
    for world in [&mut left, &mut right] {
        for name in ["Source", "LeftBase", "LeftMap", "LeftSolo", "RightBase"] {
            world.templates.insert(name.into(), ThingTemplate::new(name));
        }
    }
    let source = left.create_object("Source", Team::USA, Vec3::ZERO).unwrap();
    let peer_source = right.create_object("Source", Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(source, peer_source);
    left.apply_receiving_ocl_overrides(&[text("LeftMap"), text("LeftSolo")]).unwrap();
    let execute = |world: &mut GameLogic, id| {
        let ids = world.execute_parsed_weapon_ocl_at(
            "SameEffect", Some(id), Team::USA, VeterancyLevel::Rookie,
            0.0, Vec3::ZERO, Vec3::ZERO,
        ).unwrap();
        assert_eq!(ids.len(), 1);
        world.objects[&ids[0]].template_name.clone()
    };
    assert_eq!(execute(&mut left, source), "LeftSolo");
    assert_eq!(execute(&mut right, peer_source), "RightBase");
    left.reset();
    let source = left.create_object("Source", Team::USA, Vec3::ZERO).unwrap();
    assert_eq!(execute(&mut left, source), "LeftBase");
    assert_eq!(execute(&mut right, peer_source), "RightBase");
}

#[test]
fn retail_fire_field_ocl_creates_the_authored_object_for_the_source_team() {
    // Use the production asset filesystem's configured roots and BIG reader.
    // The compiled source directory and private runtime CWD need not contain
    // any extracted licensed INIs. Parse the owned retail bytes using the
    // existing OCL parser; absence is a genuine admission failure, not a skip.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("retail asset IO runtime");
    let mut assets = crate::assets::archive::ArchiveFileSystem::new();
    runtime
        .block_on(assets.init())
        .expect("retail asset filesystem must initialize");
    let bytes = assets
        .open_file_sync("Data/INI/ObjectCreationList.ini")
        .expect("configured retail ObjectCreationList.ini must be readable");
    let text = std::str::from_utf8(&bytes)
        .expect("retail ObjectCreationList.ini must contain valid UTF-8");
    let mut logic = GameLogic::new();
    logic
        .apply_receiving_ocl_overrides(&[text.to_owned()])
        .expect("retail ObjectCreationList.ini must parse");
    logic.templates.insert(
        "__OclBridgeSource".into(),
        ThingTemplate::new("__OclBridgeSource"),
    );
    // The test supplies the exact existing Object identity; production
    // obtains it from the parsed asset definition before creating it.
    logic.templates.insert(
        "FireFieldSmall".into(),
        ThingTemplate::new("FireFieldSmall"),
    );
    let source = logic
        .create_object("__OclBridgeSource", Team::China, Vec3::new(10.0, 3.0, 20.0))
        .expect("source object");

    let created = logic.execute_parsed_weapon_ocl_at(
        "OCL_FireFieldSmall",
        Some(source),
        Team::China,
        VeterancyLevel::Veteran,
        0.0,
        Vec3::ZERO,
        Vec3::new(10.0, 3.0, 20.0),
    ).unwrap();

    assert_eq!(created.len(), 1);
    let field = logic.objects.get(&created[0]).expect("created field");
    assert_eq!(field.template_name, "FireFieldSmall");
    assert_eq!(field.team, Team::China);
    assert_eq!(field.producer_id, Some(source));
    assert_eq!(field.get_position(), Vec3::new(10.0, 3.0, 20.0));
}

#[test]
fn mixed_ocl_still_creates_supported_nuggets_when_one_needs_leftover_physics() {
    // C++ ObjectCreationList.cpp:1524-1534 createInternal runs every nugget.
    // A leftover-physics CreateDebris must not drop the CreateObject spawn.
    let ini = r#"
ObjectCreationList OCL_HostMixedParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
  End
  CreateDebris
    ModelNames = EXRockChunk
    Count = 1
    Disposition = SEND_IT_FLYING
    ExtraFriction = -0.3
  End
End
"#;
    let mut logic = GameLogic::new();
    logic
        .apply_receiving_ocl_overrides(&[ini.to_owned()])
        .expect("mixed OCL must parse");
    logic.templates.insert(
        "__OclBridgeSource".into(),
        ThingTemplate::new("__OclBridgeSource"),
    );
    logic.templates.insert(
        "FireFieldSmall".into(),
        ThingTemplate::new("FireFieldSmall"),
    );
    let source = logic
        .create_object("__OclBridgeSource", Team::China, Vec3::new(4.0, 1.0, 8.0))
        .expect("source object");

    let created = logic.execute_parsed_weapon_ocl_at(
        "OCL_HostMixedParity",
        Some(source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(4.0, 1.0, 8.0),
    ).unwrap();

    assert_eq!(created.len(), 1);
    let field = logic.objects.get(&created[0]).expect("created field");
    assert_eq!(field.template_name, "FireFieldSmall");
    assert_eq!(field.team, Team::China);
}

/// C++ ObjectCreationList.cpp:1302-1305 — OCL create uses the source
/// controlling player, not `unique_player_id_for_team` (None in 2v2).
#[test]
fn weapon_ocl_create_uses_source_owner_player_id_in_2v2() {
    let ini = r#"
ObjectCreationList OCL_HostOwnerParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
  End
End
"#;
    let mut logic = GameLogic::new();
    logic
        .apply_receiving_ocl_overrides(&[ini.to_owned()])
        .expect("owner OCL must parse");
    logic.add_player(Player::new(3, Team::China, "ChinaA", true));
    logic.add_player(Player::new(7, Team::China, "ChinaB", false));
    assert_eq!(logic.unique_player_id_for_team(Team::China), None);

    logic.templates.insert(
        "__OclOwnerSource".into(),
        ThingTemplate::new("__OclOwnerSource"),
    );
    logic.templates.insert(
        "FireFieldSmall".into(),
        ThingTemplate::new("FireFieldSmall"),
    );
    let source = logic
        .create_object_for_player("__OclOwnerSource", 7, Vec3::new(10.0, 3.0, 20.0))
        .expect("source object");
    assert_eq!(
        logic.objects.get(&source).and_then(|o| o.owner_player_id),
        Some(7)
    );

    let created = logic.execute_parsed_weapon_ocl_at(
        "OCL_HostOwnerParity",
        Some(source),
        Team::China,
        VeterancyLevel::Veteran,
        0.0,
        Vec3::ZERO,
        Vec3::new(10.0, 3.0, 20.0),
    ).unwrap();

    assert_eq!(created.len(), 1);
    let field = logic.objects.get(&created[0]).expect("created field");
    assert_eq!(field.template_name, "FireFieldSmall");
    assert_eq!(field.team, Team::China);
    assert_eq!(field.owner_player_id, Some(7));
    assert_eq!(field.producer_id, Some(source));
}

/// C++ ObjectCreationList.cpp:996-1005 — inherit only if isTrainable
/// and transferObjectName with the source script name.
#[test]
fn ocl_inherit_veterancy_only_if_trainable_and_transfers_script_name() {
    let ini = r#"
ObjectCreationList OCL_HostInheritVetParity
  CreateObject
    ObjectNames = OclEjectedPilot
    Count = 1
    InheritsVeterancy = Yes
  End
End
ObjectCreationList OCL_HostInheritFieldParity
  CreateObject
    ObjectNames = FireFieldSmall
    Count = 1
    InheritsVeterancy = Yes
  End
End
"#;
    let mut logic = GameLogic::new();
    logic
        .apply_receiving_ocl_overrides(&[ini.to_owned()])
        .expect("inherit OCL must parse");
    let mut source_tpl = ThingTemplate::new("__OclVetSource");
    source_tpl.is_trainable = true;
    logic.templates.insert("__OclVetSource".into(), source_tpl);

    let mut pilot = ThingTemplate::new("OclEjectedPilot");
    pilot.is_trainable = true;
    logic.templates.insert("OclEjectedPilot".into(), pilot);
    logic.templates.insert(
        "FireFieldSmall".into(),
        ThingTemplate::new("FireFieldSmall"),
    );

    let source = logic
        .create_object("__OclVetSource", Team::USA, Vec3::new(2.0, 0.0, 4.0))
        .expect("source");
    logic
        .with_object_veterancy_feedback(source, |src, audio, feedback| {
            src.name = "PilotOne".into();
            src.set_min_veterancy_level(VeterancyLevel::Elite, audio, feedback);
        })
        .expect("actual OCL source owner");

    logic.initialize_receiving_named_cache_for_fixture();
    assert_eq!(
        logic.host_object_id_by_script_name("PilotOne"),
        Some(source)
    );
    let created = logic.execute_parsed_weapon_ocl_at(
        "OCL_HostInheritVetParity",
        Some(source),
        Team::USA,
        VeterancyLevel::Elite,
        0.0,
        Vec3::ZERO,
        Vec3::new(2.0, 0.0, 4.0),
    ).unwrap();
    assert_eq!(created.len(), 1);
    let pilot_id = created[0];
    let spawned = logic.objects.get(&pilot_id).expect("pilot");
    assert_eq!(spawned.experience.level, VeterancyLevel::Elite);
    assert_eq!(spawned.name, "PilotOne");
    assert_eq!(
        logic.objects.get(&source).map(|o| o.name.as_str()),
        Some("PilotOne"),
        "CPP transferObjectName preserves the donor's field"
    );
    assert_eq!(
        logic.host_object_id_by_script_name("PilotOne"),
        Some(pilot_id)
    );

    let field_src = logic
        .create_object("__OclVetSource", Team::USA, Vec3::new(8.0, 0.0, 4.0))
        .expect("field source");
    logic
        .with_object_veterancy_feedback(field_src, |src, audio, feedback| {
            src.name = "FieldSrc".into();
            src.set_min_veterancy_level(VeterancyLevel::Elite, audio, feedback);
        })
        .expect("actual OCL source owner");
    let created_field = logic.execute_parsed_weapon_ocl_at(
        "OCL_HostInheritFieldParity",
        Some(field_src),
        Team::USA,
        VeterancyLevel::Elite,
        0.0,
        Vec3::ZERO,
        Vec3::new(8.0, 0.0, 4.0),
    ).unwrap();
    assert_eq!(created_field.len(), 1);
    let field = logic.objects.get(&created_field[0]).expect("field");
    assert!(!field.is_trainable());
    assert_eq!(field.experience.level, VeterancyLevel::Rookie);
    assert!(field.name.is_empty());
    assert_eq!(
        logic.objects.get(&field_src).map(|o| o.name.as_str()),
        Some("FieldSrc")
    );
}

/// hq-2qedt.60: C++ ObjectCreationList.cpp:996-1001 calls the exact
/// `ExperienceTracker::setVeterancyLevel` (ExperienceTracker.cpp:82-95),
/// not setMinVeterancyLevel, after construction/onCreate already seeded the
/// receiving rank (Object.cpp:456, VeterancyGainCreate). The inherited
/// source rank therefore replaces — and may lower — that rank.
#[test]
fn ocl_inherits_veterancy_sets_exact_rank_over_receiving_create_rank() {
    let ini = r#"
ObjectCreationList OCL_Hq60Inherit
  CreateObject
    ObjectNames = Hq60Spawned
    Count = 1
    InheritsVeterancy = Yes
  End
End
"#;
    let mut logic = GameLogic::new();
    logic
        .apply_receiving_ocl_overrides(&[ini.to_owned()])
        .expect("inherit OCL must parse");
    let mut source_tpl = ThingTemplate::new("__Hq60Source");
    source_tpl.is_trainable = true;
    logic.templates.insert("__Hq60Source".into(), source_tpl);
    let mut spawned_tpl = ThingTemplate::new("Hq60Spawned");
    spawned_tpl.is_trainable = true;
    spawned_tpl.veterancy_gain_creates = vec![crate::game_logic::VeterancyGainCreateMetadata {
        starting_level: VeterancyLevel::Elite,
        science_required: None,
    }];
    logic.templates.insert("Hq60Spawned".into(), spawned_tpl);
    let source = logic
        .create_object("__Hq60Source", Team::USA, Vec3::new(2.0, 0.0, 4.0))
        .expect("source");

    // C++ VeterancyGainCreate::onCreate only acts for a controlling player.
    logic.add_player(crate::game_logic::Player::new(0, Team::USA, "Hq60Owner", true));
    let baseline = logic
        .create_object_for_player("Hq60Spawned", 0, Vec3::new(20.0, 0.0, 4.0))
        .expect("baseline");
    assert_eq!(
        logic.objects.get(&baseline).unwrap().experience.level,
        VeterancyLevel::Elite,
        "fixture: receiving create seeds Elite before OCL inherit"
    );

    for (source_rank, x) in [
        (VeterancyLevel::Veteran, 6.0),
        (VeterancyLevel::Rookie, 10.0),
    ] {
        let created = logic.execute_parsed_weapon_ocl_at(
            "OCL_Hq60Inherit",
            Some(source),
            Team::USA,
            source_rank,
            0.0,
            Vec3::ZERO,
            Vec3::new(x, 0.0, 4.0),
        ).unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(
            logic.objects.get(&created[0]).unwrap().experience.level,
            source_rank,
            "C++ setVeterancyLevel is exact: source {source_rank:?} replaces created Elite"
        );
    }
}

/// C++ HelixContain.cpp:340 — portable gattling fires without host shot.
#[test]
fn overlord_gattling_independent_acquire_not_stacked_primary() {
    use crate::game_logic::{AIState, KindOf, Team};
    let mut logic = GameLogic::new();
    let mut overlord = ThingTemplate::new("ChinaTankOverlord");
    overlord
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0);
    overlord.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    overlord.contain_module.is_overlord_contain = true;
    overlord.contain_module.slots = Some(1);
    overlord.contain_module.passengers_allowed_to_fire = true;
    let mut turret = ThingTemplate::new("AuthoredPortableGun");
    turret.add_kind_of(KindOf::PortableStructure).set_health(100.0);
    logic.templates.insert(turret.name.clone(), turret);
    logic
        .templates
        .insert("ChinaTankOverlord".to_string(), overlord);
    let mut enemy = ThingTemplate::new("UsaRanger");
    enemy
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("UsaRanger".to_string(), enemy);

    let tank = logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    let turret = logic
        .create_object("AuthoredPortableGun", Team::China, Vec3::ZERO)
        .expect("portable gun");
    logic.attach_overlord_portable_occupant(tank, turret);
    let frame = logic.frame;
    {
        let o = logic.host_object_mut(turret).unwrap();
        o.set_ai_state(AIState::Idle, frame, &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
        crate::fixture_content::arm_with_stats(
            o,
            crate::game_logic::Weapon {
                damage: 10.0,
                range: 150.0,
                reload_time: 0.1,
                last_fire_time: -10.0,
                projectile_speed: 0.0, // WeaponSpeed 999999: instant hit
                ..crate::game_logic::Weapon::default()
            },
        );
    }
    let victim = logic
        .create_object("UsaRanger", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("victim");
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.set_current_frame(30);
    logic.try_transport_passenger_residual_fire(turret);
    let hp_after = logic.host_object(victim).unwrap().health.current;
    let dealt = hp_before - hp_after;
    assert!(
        (dealt - crate::game_logic::host_overlord_addons::OVERLORD_GATTLING_GROUND_DAMAGE).abs()
            < 0.2,
        "independent gattling deals 10 not stacked primary+10, dealt={dealt}"
    );
    assert!(logic.host_object(tank).unwrap().target.is_none());
}

#[test]
fn fade_in_nugget_spawns_and_starts_drawable_fade() {
    let ini = r#"
ObjectCreationList OCL_HostFadeInParity
  CreateObject
    ObjectNames = FadeDebris
    Count = 1
    FadeIn = Yes
    FadeTime = 1000
  End
End
"#;
    let mut logic = GameLogic::new();
    logic
        .apply_receiving_ocl_overrides(&[ini.to_owned()])
        .expect("fade OCL must parse");
    logic.frame = 40;
    logic.templates.insert(
        "__OclBridgeSource".into(),
        ThingTemplate::new("__OclBridgeSource"),
    );
    logic
        .templates
        .insert("FadeDebris".into(), ThingTemplate::new("FadeDebris"));
    let source = logic
        .create_object("__OclBridgeSource", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("source object");

    let created = logic.execute_parsed_weapon_ocl_at(
        "OCL_HostFadeInParity",
        Some(source),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(1.0, 0.0, 1.0),
    ).unwrap();
    assert_eq!(created.len(), 1);
    let debris = logic.objects.get(&created[0]).expect("faded debris");
    assert_eq!(
        debris.drawable_fade_mode,
        crate::game_logic::DRAWABLE_FADE_IN
    );
    assert_eq!(debris.drawable_fade_start_frame, 40);
    assert!(debris.drawable_fade_frames > 0);
    assert!((debris.drawable_fade_opacity(40) - 0.0).abs() < 1e-5);
    assert!(debris.drawable_fade_opacity(40 + debris.drawable_fade_frames) > 0.99);
}

/// hq-2qedt.1: C++ ObjectCreationUpgrade -> OCL CreateObject
/// ContainInsideSourceObject -> OverlordContain (TransportContain)
/// addToContain. The retail gattling rider must be admitted, and the
/// host_overlord_addons occupant path must reuse it (no double spawn).
#[test]
fn overlord_contain_admits_retail_gattling_ocl_exactly_once() {
    // The retail name also drives the native host_overlord_addons occupant
    // (double-spawn guard); the arbitrary name has only the authored
    // OverlordContain + ObjectCreationUpgrade, so it proves OCL admission.
    for host_name in ["ChinaTankOverlord", "ArbitraryAuthoredContainHost"] {
        overlord_contain_gattling_ocl_case(host_name);
    }
}

fn overlord_contain_gattling_ocl_case(host_name: &str) {
    use crate::game_logic::host_overlord_addons::{
        OCL_OVERLORD_GATTLING, OVERLORD_PAYLOAD_GATTLING, UPGRADE_OVERLORD_GATTLING,
    };
    let mut logic = GameLogic::new();
    logic
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList {OCL_OVERLORD_GATTLING}\nCreateObject\nObjectNames = {OVERLORD_PAYLOAD_GATTLING}\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd"
            )])
            .expect("retail Overlord addon OCL parses");
    logic
            .admit_object_ini_sources([(
                "OverlordPortable.ini".into(),
                format!(
                    "Object {host_name}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 1100\n InitialHealth = 1100\n End\n Behavior = OverlordContain Contain\n Slots = 1\n AllowInsideKindOf = PORTABLE_STRUCTURE\n End\n Behavior = ObjectCreationUpgrade Gattling\n TriggeredBy = {UPGRADE_OVERLORD_GATTLING}\n UpgradeObject = {OCL_OVERLORD_GATTLING}\n End\nEnd\nObject {OVERLORD_PAYLOAD_GATTLING}\n KindOf = VEHICLE PORTABLE_STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 1100\n InitialHealth = 1100\n End\nEnd\n"
                ),
            )])
            .expect("ini");
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(
        &format!("Upgrade {UPGRADE_OVERLORD_GATTLING}\nType = OBJECT\nEnd\n"),
        |ini| {
            ini.read_line()?;
            logic.upgrade_definitions.parse_upgrade_definition(ini)
        },
    )
    .expect("object upgrade definition should admit");
    logic.players.insert(
        1,
        crate::game_logic::Player::new(1, Team::China, "Owner", true),
    );
    let overlord = logic
        .create_object_for_player(host_name, 1, Vec3::ZERO)
        .expect("overlord");
    logic.apply_upgrade_to_object(overlord, UPGRADE_OVERLORD_GATTLING);

    let all: Vec<_> = logic
        .host_objects()
        .registered_values()
        .filter(|object| object.template_name == OVERLORD_PAYLOAD_GATTLING)
        .map(|object| (object.id, object.status.destroyed, object.contained_by))
        .collect();
    assert_eq!(
        all.len(),
        1,
        "{host_name}: exactly one gattling payload: {all:?}"
    );
    let (payload, destroyed, contained_by) = all[0];
    assert!(!destroyed);
    assert_eq!(contained_by, Some(overlord));
    assert_eq!(
        logic
            .host_object(overlord)
            .unwrap()
            .overlord_portable_occupant,
        Some(payload)
    );
}

#[test]
fn authored_helix_creation_uses_payload_and_weapon_not_carrier_name() {
    use crate::save_load::snapshot::{apply_lifecycle_tail_to_host, capture_lifecycle_tail};
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([(
        "AuthoredHelixPayload.ini".into(),
        "Object CustomAirCarrier\n\
         KindOf = VEHICLE AIRCRAFT\n\
         WeaponSet\n Conditions = None\n Weapon = PRIMARY HelixMinigunWeapon\n End\n\
         Behavior = HelixContain Cargo\n Slots = 2\n AllowInsideKindOf = INFANTRY\n\
         PayloadTemplateName = CustomPortableGun\n PayloadTemplateName = CustomPassenger CustomPassenger\n End\nEnd\n\
         Object CustomPortableGun\n KindOf = VEHICLE PORTABLE_STRUCTURE\n End\n\
         Object CustomPassenger\n KindOf = INFANTRY\n TransportSlotCount = 1\n End\n\
         Object ChinaVehicleHelix\n KindOf = VEHICLE AIRCRAFT\n End\n".into(),
    )]).unwrap();
    world.add_player(crate::game_logic::Player::new(1, Team::China, "Owner", true));
    let carrier = world.create_object_for_player("CustomAirCarrier", 1, Vec3::ZERO).unwrap();
    let decoy = world.create_object_for_player("ChinaVehicleHelix", 1, Vec3::ZERO).unwrap();
    let host = world.host_object(carrier).unwrap();
    assert!(host.is_helix_transport);
    assert_eq!(host.authored_weapon_name_for_slot(0), Some("HelixMinigunWeapon"));
    let payload = host.overlord_portable_occupant.expect("authored initial rider");
    assert_eq!(world.host_object(payload).unwrap().contained_by, Some(carrier));
    assert!(!world.host_object(carrier).unwrap().occupants.contains(&payload));
    assert!(!world.host_object(payload).unwrap().is_held_disabled());
    assert_eq!(world.host_object(carrier).unwrap().occupants.len(), 2);
    let passenger = world.host_object(carrier).unwrap().occupants.iter().copied()
        .find(|&id| id != payload).expect("second authored payload");
    assert_eq!(world.host_object(passenger).unwrap().contained_by, Some(carrier));
    assert!(world.host_object(passenger).unwrap().is_held_disabled());
    let named = world.host_object(decoy).unwrap();
    assert!(!named.is_helix_transport);
    assert!(named.overlord_portable_occupant.is_none());
    assert_eq!(named.authored_weapon_name_for_slot(0), None);
    let saved = capture_lifecycle_tail(&world).unwrap();
    assert!(!saved.contain_links.iter().any(|link| link.occupant_id == payload.0));
    world.host_object_mut(carrier).unwrap().occupants.clear();
    world.host_object_mut(payload).unwrap().contained_by = None;
    apply_lifecycle_tail_to_host(&saved, &mut world).unwrap();
    assert_eq!(world.host_object(carrier).unwrap().overlord_portable_occupant, Some(payload));
    assert_eq!(world.host_object(payload).unwrap().contained_by, Some(carrier));
}

#[test]
fn authored_overlord_initial_payload_redirects_after_bunker_is_installed() {
    use crate::save_load::snapshot::{apply_lifecycle_tail_to_host, capture_lifecycle_tail};
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([(
        "AuthoredOverlordPayload.ini".into(),
        "Object CustomGroundCarrier\n KindOf = VEHICLE\n\
         Behavior = OverlordContain Cargo\n Slots = 1\n AllowInsideKindOf = PORTABLE_STRUCTURE\n\
         PayloadTemplateName = CustomRiderBunker\n PayloadTemplateName = CustomBunkerPassenger\n\
         End\nEnd\n\
         Object CustomRiderBunker\n KindOf = VEHICLE PORTABLE_STRUCTURE\n TransportSlotCount = 1\n\
         Behavior = TransportContain Cargo\n Slots = 2\n AllowInsideKindOf = INFANTRY\n End\nEnd\n\
         Object CustomBunkerPassenger\n KindOf = INFANTRY\n TransportSlotCount = 1\n End\n\
         Object ChinaTankOverlord\n KindOf = VEHICLE\n End\n".into(),
    )]).unwrap();
    world.add_player(crate::game_logic::Player::new(1, Team::China, "Owner", true));
    let host = world.create_object_for_player("CustomGroundCarrier", 1, Vec3::ZERO).unwrap();
    let named = world.create_object_for_player("ChinaTankOverlord", 1, Vec3::ZERO).unwrap();
    let bunker = world.host_object(host).unwrap().overlord_portable_occupant.unwrap();
    let passenger = world.host_object(bunker).unwrap().contained_units()[0];
    assert_eq!(world.host_object(host).unwrap().contained_units(), vec![bunker]);
    assert_eq!(world.host_object(passenger).unwrap().contained_by, Some(bunker));
    assert!(world.host_object(passenger).unwrap().is_held_disabled());
    assert!(world.host_object(named).unwrap().occupants.is_empty());
    let saved = capture_lifecycle_tail(&world).unwrap();
    world.host_object_mut(bunker).unwrap().occupants.clear();
    world.host_object_mut(passenger).unwrap().contained_by = None;
    apply_lifecycle_tail_to_host(&saved, &mut world).unwrap();
    assert_eq!(world.host_object(passenger).unwrap().contained_by, Some(bunker));
    assert_eq!(world.host_object(bunker).unwrap().contained_units(), vec![passenger]);
}

#[test]
fn receiving_object_ocl_places_and_orients_children_with_the_complete_source_basis() {
    const LIST: &str = "OCL_ObjectBasis";
    let source_pos = Vec3::new(20.0, 4.0, 30.0);
    let pose = glam::Mat4::from_cols(
        Vec3::Y.extend(0.0),
        Vec3::Z.extend(0.0),
        Vec3::X.extend(0.0),
        source_pos.extend(1.0),
    );
    let fixture = |matrix: glam::Mat4| {
        let mut world = GameLogic::new();
        world.admit_object_ini_sources([("ocl-basis.ini".into(), r"
Object OclBasisSource
 KindOf = VEHICLE
End
Object OclBasisChild
 KindOf = IMMOBILE
End
".into())]).unwrap();
        world.admit_object_creation_list_sources([format!(
            "ObjectCreationList {LIST}\nCreateObject\nObjectNames = OclBasisChild\nCount = 1\nDisposition = LIKE_EXISTING\nOffset = X:1 Y:2 Z:3\nEnd\nEnd"
        )]).unwrap();
        let id = world.create_object("OclBasisSource", Team::USA, source_pos).unwrap();
        assert!(world.set_registered_object_transform(id, matrix));
        (world, id)
    };
    let (mut world, id) = fixture(pose);
    let (peer, peer_id) = fixture(glam::Mat4::from_translation(source_pos));
    assert_eq!(peer_id, id);

    let outcome = world.execute_receiving_object_ocl_with_births(
        LIST, id, None, &mut Default::default(),
    );
    assert!(outcome.error.is_none());
    let [(child, _)] = outcome.created.as_slice() else {
        panic!("the original LIKE_EXISTING OCL creates one child");
    };
    let transform = world.objects[child].get_transform_matrix();
    let observed: Vec<_> = [transform.x_axis, transform.y_axis, transform.z_axis, transform.w_axis]
        .into_iter()
        .flat_map(|column| column.truncate().to_array())
        .collect();
    let original: Vec<f32> = include_str!("object_basis.trace")
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();
    assert_eq!(observed, original);
    assert_eq!(world.objects[child].producer_id, Some(id));
    assert_eq!(peer.objects[&peer_id].get_position(), source_pos);
    assert!(!peer.objects.contains_key(child));
}

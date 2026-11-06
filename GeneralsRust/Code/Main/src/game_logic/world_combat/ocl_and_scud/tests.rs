//! Host OCL execution tests.

use super::*;
use crate::game_logic::ThingTemplate;

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
        )
    };
    assert!(execute(&mut first, first_id).is_empty());
    let children = execute(&mut peer, peer_id);
    assert_eq!(children.len(), 1);
    assert_eq!(peer.objects[&children[0]].template_name, "OwnedChild");
    peer.templates.remove("OwnedChild");
    assert!(execute(&mut peer, peer_id).is_empty());
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
    );

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
    );

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
    );

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
    );
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
    );
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

    let baseline = logic
        .create_object("Hq60Spawned", Team::USA, Vec3::new(20.0, 0.0, 4.0))
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
        );
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
    {
        let o = logic.host_object_mut(turret).unwrap();
        o.set_ai_state(AIState::Idle, &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
        let mut weapon = crate::game_logic::Weapon::default();
        weapon.last_fire_time = -10.0;
        weapon.reload_time = 0.1;
        weapon.range = 150.0;
        weapon.damage = 10.0;
        o.weapon = Some(weapon);
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
    );
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

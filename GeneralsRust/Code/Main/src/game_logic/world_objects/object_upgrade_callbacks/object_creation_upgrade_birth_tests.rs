use super::*;
use glam::Vec3;

const UPGRADE: &str = "Upgrade_AmericaScoutDrone";
const SECOND_UPGRADE: &str = "Upgrade_ObjectCreationUpgradeSecondTrigger";
const CONFLICT_UPGRADE: &str = "Upgrade_ObjectCreationUpgradeConflict";

fn admit_upgrade(world: &mut GameLogic, name: &str) {
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&format!("Upgrade {name}\nEnd\n"), |ini| {
        ini.read_line()?;
        world.upgrade_definitions.parse_upgrade_definition(ini)
    })
    .expect("upgrade definition should admit");
}

fn admit_object_upgrade(world: &mut GameLogic, name: &str) {
    let mut ini = game_engine::common::ini::INI::new();
    ini.with_inline_source(&format!("Upgrade {name}\nType = OBJECT\nEnd\n"), |ini| {
        ini.read_line()?;
        world.upgrade_definitions.parse_upgrade_definition(ini)
    })
    .expect("object upgrade definition should admit");
}

#[test]
fn max_health_captured_callback_rejects_same_id_replacement() {
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(0, Team::USA, "Owner", true));
    admit_object_upgrade(&mut world, "Upgrade_HealthBirth");
    world.admit_object_ini_sources([(
        "HealthBirth.ini".into(),
        "Object HealthBirthHost\nKindOf = VEHICLE\nBody = ActiveBody Body\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = MaxHealthUpgrade Health\nTriggeredBy = Upgrade_HealthBirth\nAddMaxHealth = 100\nChangeType = ADD_CURRENT_HEALTH_TOO\nEnd\nEnd\n".into(),
    )]).unwrap();
    let id = world
        .create_object_for_player("HealthBirthHost", 0, Vec3::ZERO)
        .unwrap();
    let pass = world
        .prepare_authored_upgrade_pass(id, Some("Upgrade_HealthBirth"))
        .expect("the live Object has a controlling Player");
    let original = world.objects.get(&id).unwrap();
    let mut replacement = Object::new(original.thing().template.clone(), id, Team::USA);
    replacement.owner_player_id = Some(0);
    world.objects.insert(id, replacement);
    world
        .execute_authored_ordered_upgrade_modules(pass, None, &mut Default::default())
        .unwrap();
    assert_eq!(
        world.host_object(id).unwrap().health.maximum,
        200.0,
        "captured upgrade callbacks belong to the original Object, not its numeric ID"
    );
    world.apply_upgrade_to_object(id, "Upgrade_HealthBirth");
    assert_eq!(
        world.host_object(id).unwrap().health.maximum,
        300.0,
        "the replacement owns a fresh execution latch"
    );
}

#[test]
fn authored_passenger_fire_upgrade_requires_its_combined_trigger_mask() {
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::China,
        "Owner",
        true,
    ));
    admit_object_upgrade(&mut world, "Upgrade_TestFireA");
    admit_object_upgrade(&mut world, "Upgrade_TestFireB");
    world.admit_object_ini_sources([(
        "PassengerFireUpgrade.ini".into(),
        "Object AuthoredHelix\nKindOf = VEHICLE AIRCRAFT\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = HelixContain Contain\nSlots = 5\nEnd\nBehavior = PassengersFireUpgrade Fire\nTriggeredBy = Upgrade_TestFireA Upgrade_TestFireB\nRequiresAllTriggers = Yes\nEnd\nEnd\nObject NamedHelixWithoutUpgrade\nKindOf = VEHICLE AIRCRAFT\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = HelixContain Contain\nSlots = 5\nEnd\nEnd\n".into(),
    )]).unwrap();
    let source = world
        .create_object_for_player("AuthoredHelix", 0, Vec3::ZERO)
        .unwrap();
    let peer = world
        .create_object_for_player("NamedHelixWithoutUpgrade", 0, Vec3::ZERO)
        .unwrap();
    world.apply_upgrade_to_object(source, "Upgrade_TestFireA");
    assert!(
        !world
            .host_object(source)
            .unwrap()
            .passengers_allowed_to_fire
    );
    world.apply_upgrade_to_object(source, "Upgrade_TestFireB");
    assert!(
        world
            .host_object(source)
            .unwrap()
            .passengers_allowed_to_fire
    );
    world.apply_upgrade_to_object(peer, "Upgrade_TestFireA");
    world.apply_upgrade_to_object(peer, "Upgrade_TestFireB");
    assert!(!world.host_object(peer).unwrap().passengers_allowed_to_fire);
}

#[test]
fn passenger_fire_upgrade_follows_ocl_declaration_order() {
    for fire_first in [true, false] {
        let mut world = GameLogic::new();
        world.add_player(crate::game_logic::Player::new(
            0,
            Team::China,
            "Owner",
            true,
        ));
        admit_object_upgrade(&mut world, "Upgrade_TestOrdered");
        world.admit_object_creation_list_sources([
            "ObjectCreationList OCL_OrderedRider\nCreateObject\nObjectNames = OrderedRider\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd\n".into(),
        ]).unwrap();
        let fire =
            "Behavior = PassengersFireUpgrade Fire\nTriggeredBy = Upgrade_TestOrdered\nEnd\n";
        let create = "Behavior = ObjectCreationUpgrade Create\nTriggeredBy = Upgrade_TestOrdered\nUpgradeObject = OCL_OrderedRider\nEnd\n";
        let modules = if fire_first {
            format!("{fire}{create}")
        } else {
            format!("{create}{fire}")
        };
        world.admit_object_ini_sources([(
            "OrderedContain.ini".into(),
            format!("Object OrderedHull\nKindOf = VEHICLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = OverlordContain Contain\nSlots = 1\nAllowInsideKindOf = PORTABLE_STRUCTURE\nEnd\n{modules}End\nObject OrderedRider\nKindOf = PORTABLE_STRUCTURE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = TransportContain Contain\nSlots = 5\nEnd\nEnd\n"),
        )]).unwrap();
        let hull = world
            .create_object_for_player("OrderedHull", 0, Vec3::ZERO)
            .unwrap();
        world.apply_upgrade_to_object(hull, "Upgrade_TestOrdered");
        let object = world.host_object(hull).unwrap();
        let rider = object
            .overlord_portable_occupant
            .expect("authored OCL rider");
        assert_eq!(object.passengers_allowed_to_fire, fire_first);
        assert_eq!(
            world.host_object(rider).unwrap().passengers_allowed_to_fire,
            !fire_first
        );
    }
}

fn world(triggers: &str, ocl_name: &str, child_name: Option<&str>) -> (GameLogic, ObjectId) {
    world_with_mux(triggers, false, ocl_name, child_name)
}

fn world_with_mux(
    triggers: &str,
    requires_all: bool,
    ocl_name: &str,
    child_name: Option<&str>,
) -> (GameLogic, ObjectId) {
    world_with_mux_conflicting(triggers, requires_all, ocl_name, child_name, "")
}

fn world_with_mux_conflicting(
    triggers: &str,
    requires_all: bool,
    ocl_name: &str,
    child_name: Option<&str>,
    conflicts: &str,
) -> (GameLogic, ObjectId) {
    world_with_mux_order(
        triggers,
        requires_all,
        ocl_name,
        child_name,
        conflicts,
        true,
        "",
    )
}

fn world_with_mux_order(
    triggers: &str,
    requires_all: bool,
    ocl_name: &str,
    child_name: Option<&str>,
    conflicts: &str,
    status_first: bool,
    creation_removes: &str,
) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "ObjectCreationUpgradeOwner",
        true,
    ));
    admit_upgrade(&mut world, UPGRADE);
    admit_upgrade(&mut world, SECOND_UPGRADE);
    admit_upgrade(&mut world, CONFLICT_UPGRADE);
    let status_module = format!(
        "Behavior = StatusBitsUpgrade AuthoredStatusBitsUpgrade\n  TriggeredBy = {triggers}\n  ConflictsWith = {conflicts}\n  RequiresAllTriggers = {}\n  StatusToSet = BOOBY_TRAPPED\n End",
        if requires_all { "Yes" } else { "No" },
    );
    let creation_module = format!(
        "Behavior = ObjectCreationUpgrade AuthoredCreationUpgrade\n  TriggeredBy = {triggers}\n  ConflictsWith = {conflicts}\n  RemovesUpgrades = {creation_removes}\n  RequiresAllTriggers = {}\n  UpgradeObject = {ocl_name}\n End",
        if requires_all { "Yes" } else { "No" },
    );
    let modules = if status_first {
        format!("{status_module}\n {creation_module}")
    } else {
        format!("{creation_module}\n {status_module}")
    };
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "ObjectCreationUpgradeOwner.ini".into(),
            format!(
                "Object AuthoredCreationUpgradeHost\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n {modules}\nEnd\n{}",
                child_name.map_or_else(String::new, |name| {
                    format!("Object {name}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n")
                })
            ),
        )])
        .unwrap(),
    )
    .expect("typed ObjectCreationUpgrade module should admit");
    if let Some(child_name) = child_name {
        world
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList {ocl_name}\n CreateObject\n  ObjectNames = {child_name}\n  Count = 1\n End\nEnd\n"
            )])
            .expect("receiving OCL should admit");
    }
    let source = world
        .create_object_for_player("AuthoredCreationUpgradeHost", 0, glam::Vec3::ZERO)
        .expect("upgrade source should create");
    (world, source)
}

fn apply(world: &mut GameLogic, source: ObjectId, upgrade: &str) {
    world
        .apply_upgrade_to_object_with_effects_checked(
            source,
            upgrade,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("receiving ObjectCreationUpgrade callback should complete");
}

fn spawned_template_names(world: &GameLogic, source: ObjectId) -> Vec<String> {
    world
        .objects
        .registered()
        .filter(|(id, _)| **id != source)
        .map(|(_, object)| object.template_name.clone())
        .collect()
}

#[test]
fn stale_ocl_child_receipt_does_not_destroy_same_id_replacement() {
    let mut world = GameLogic::new();
    let id = ObjectId(77);
    let template = ThingTemplate::new("BirthReceiptChild");
    let original = Object::new(template.clone(), id, Team::USA);
    let original_birth = std::sync::Arc::downgrade(&original.visual_draw_identity_token);
    world.objects.insert(id, original);

    let replacement = Object::new(template, id, Team::China);
    let replacement_birth = std::sync::Arc::downgrade(&replacement.visual_draw_identity_token);
    world.objects.insert(id, replacement);

    let destroyed = world
        .destroy_object_if_birth_matches_with_effects(
            id,
            &original_birth,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
        .expect("mismatched receipt should be a harmless no-op");
    assert!(!destroyed);
    assert!(world.object_birth_matches(id, &replacement_birth));
}

#[test]
fn upgrade_module_mask_separates_receiving_player_and_object_completions() {
    const FORMER_PLAYER_UPGRADE: &str = "Upgrade_FormerOwnerResearch";
    const RECEIVING_PLAYER_UPGRADE: &str = "Upgrade_ReceivingOwnerResearch";
    const OBJECT_UPGRADE: &str = "Upgrade_AuthoredObjectCompletion";
    const PLAYER_TYPED_OBJECT_GRANT: &str = "Upgrade_PlayerTypedObjectGrant";

    let (mut world_a, source_a) = world(UPGRADE, "OCL_UpgradeMaskA", None);
    let (mut world_b, source_b) = world(UPGRADE, "OCL_UpgradeMaskB", None);
    assert_eq!(
        source_a, source_b,
        "the worlds must reuse the same ObjectId"
    );
    for world in [&mut world_a, &mut world_b] {
        admit_upgrade(world, FORMER_PLAYER_UPGRADE);
        admit_upgrade(world, RECEIVING_PLAYER_UPGRADE);
        admit_object_upgrade(world, OBJECT_UPGRADE);
        world
            .objects
            .get_mut(&source_a)
            .expect("source object")
            .apply_upgrade_tag(FORMER_PLAYER_UPGRADE);
    }
    admit_upgrade(&mut world_a, PLAYER_TYPED_OBJECT_GRANT);
    admit_upgrade(&mut world_b, PLAYER_TYPED_OBJECT_GRANT);
    for (world, source) in [(&mut world_a, source_a), (&mut world_b, source_b)] {
        apply(world, source, OBJECT_UPGRADE);
        apply(world, source, PLAYER_TYPED_OBJECT_GRANT);
    }
    world_a
        .players
        .get_mut(&0)
        .expect("world A receiving owner")
        .add_completed_upgrade(RECEIVING_PLAYER_UPGRADE);
    world_b
        .players
        .get_mut(&0)
        .expect("world B receiving owner")
        .add_completed_upgrade(FORMER_PLAYER_UPGRADE);

    let mask_a = world_a.combined_object_and_player_upgrade_mask(source_a, None);
    let mask_b = world_b.combined_object_and_player_upgrade_mask(source_b, None);
    let former = world_a
        .upgrade_template(FORMER_PLAYER_UPGRADE)
        .unwrap()
        .get_mask();
    let receiving = world_a
        .upgrade_template(RECEIVING_PLAYER_UPGRADE)
        .unwrap()
        .get_mask();
    let object = world_a.upgrade_template(OBJECT_UPGRADE).unwrap().get_mask();
    assert!(!mask_a.test_for_any(former));
    assert!(mask_a.test_for_all(receiving | object));
    assert!(mask_b.test_for_all(former | object));
    assert!(!mask_b.test_for_any(receiving));
    let player_typed = world_a
        .upgrade_template(PLAYER_TYPED_OBJECT_GRANT)
        .unwrap()
        .get_mask();
    assert!(mask_a.test_for_all(player_typed));
    assert!(mask_b.test_for_all(player_typed));

    // C++ Object::updateUpgradeModules always includes the object's own
    // mask, even when there is no controlling Player to contribute one.
    let unowned_id = ObjectId(91_004);
    let mut unowned = Object::new(
        ThingTemplate::new("UnownedUpgradeMask"),
        unowned_id,
        Team::Neutral,
    );
    world_a.objects.insert(unowned_id, unowned);
    apply(&mut world_a, unowned_id, OBJECT_UPGRADE);
    assert!(
        world_a
            .combined_object_and_player_upgrade_mask(unowned_id, None)
            .test_for_all(object)
    );
}

#[test]
fn object_creation_upgrade_uses_each_same_id_worlds_authored_module_ocl() {
    let (mut owner, owner_source) = world(
        UPGRADE,
        "OCL_OwnerAuthoredDrone",
        Some("OwnerAuthoredDrone"),
    );
    let (mut peer, peer_source) =
        world(UPGRADE, "OCL_PeerAuthoredDrone", Some("PeerAuthoredDrone"));
    assert_eq!(
        owner_source, peer_source,
        "fixture must collide on ObjectId"
    );
    apply(&mut owner, owner_source, UPGRADE);
    apply(&mut peer, peer_source, UPGRADE);

    assert_eq!(
        spawned_template_names(&owner, owner_source),
        vec!["OwnerAuthoredDrone".to_owned()]
    );
    assert_eq!(
        spawned_template_names(&peer, peer_source),
        vec!["PeerAuthoredDrone".to_owned()]
    );
    assert!(
        owner
            .host_object(owner_source)
            .unwrap()
            .has_upgrade_tag(UPGRADE)
    );
    assert!(
        peer.host_object(peer_source)
            .unwrap()
            .has_upgrade_tag(UPGRADE)
    );
    assert!(
        owner
            .host_object(owner_source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(
        peer.host_object(peer_source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn missing_or_none_authored_ocl_does_not_synthesize_a_drone() {
    let (mut none, none_source) = world(UPGRADE, "None", None);
    apply(&mut none, none_source, UPGRADE);
    assert!(spawned_template_names(&none, none_source).is_empty());

    let (mut missing, missing_source) = world(UPGRADE, "OCL_NotAdmittedForThisWorld", None);
    apply(&mut missing, missing_source, UPGRADE);
    assert!(spawned_template_names(&missing, missing_source).is_empty());
}

#[test]
fn object_creation_upgrade_or_triggers_execute_once_and_reset_on_trigger_removal() {
    let (mut world, source) = world(
        &format!("{UPGRADE} {SECOND_UPGRADE}"),
        "OCL_AuthoredOrTriggerChild",
        Some("AuthoredOrTriggerChild"),
    );
    apply(&mut world, source, UPGRADE);
    apply(&mut world, source, SECOND_UPGRADE);
    assert_eq!(
        spawned_template_names(&world, source),
        vec!["AuthoredOrTriggerChild".to_owned()],
        "a second OR trigger must not execute the same UpgradeMux module again"
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .object_creation_upgrade_executed,
        [true]
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .status_bits_upgrade_executed,
        [true]
    );

    assert!(world.remove_object_upgrade_with_receiving_rules(source, UPGRADE));
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .status_bits_upgrade_executed,
        [false]
    );
    apply(&mut world, source, SECOND_UPGRADE);
    assert_eq!(
        spawned_template_names(&world, source),
        vec![
            "AuthoredOrTriggerChild".to_owned(),
            "AuthoredOrTriggerChild".to_owned(),
        ],
        "C++ resetUpgrade clears the per-module latch when any activation bit is removed"
    );
}

#[test]
fn both_upgrade_families_honor_conflict_masks_before_latching() {
    let (mut world, source) = world_with_mux_conflicting(
        UPGRADE,
        false,
        "OCL_AuthoredConflictChild",
        Some("AuthoredConflictChild"),
        CONFLICT_UPGRADE,
    );
    apply(&mut world, source, CONFLICT_UPGRADE);
    apply(&mut world, source, UPGRADE);
    assert!(spawned_template_names(&world, source).is_empty());
    assert!(
        !world
            .host_object(source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .object_creation_upgrade_executed,
        [false]
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .status_bits_upgrade_executed,
        [false]
    );

    assert!(world.remove_object_upgrade_with_receiving_rules(source, CONFLICT_UPGRADE));
    apply(&mut world, source, UPGRADE);
    assert_eq!(
        spawned_template_names(&world, source),
        vec!["AuthoredConflictChild"]
    );
    assert!(
        world
            .host_object(source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .object_creation_upgrade_executed,
        [true]
    );
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .status_bits_upgrade_executed,
        [true]
    );
}

#[test]
fn typed_upgrade_dispatch_accepts_either_authored_module_order() {
    for status_first in [true, false] {
        let (mut world, source) = world_with_mux_order(
            UPGRADE,
            false,
            "OCL_AuthoredOrderChild",
            Some("AuthoredOrderChild"),
            "",
            status_first,
            UPGRADE,
        );
        apply(&mut world, source, UPGRADE);
        assert!(
            world
                .host_object(source)
                .unwrap()
                .has_object_status_bit("BOOBY_TRAPPED")
        );
        assert_eq!(
            spawned_template_names(&world, source),
            vec!["AuthoredOrderChild"]
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .object_creation_upgrade_executed,
            [true]
        );
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .status_bits_upgrade_executed,
            [!status_first],
            "the later ObjectCreationUpgrade removal must reset an earlier StatusBitsUpgrade latch"
        );
    }
}

#[test]
fn object_creation_upgrade_combines_receiving_player_and_object_masks() {
    let (mut world, source) = world_with_mux(
        &format!("{UPGRADE} {SECOND_UPGRADE}"),
        true,
        "OCL_AuthoredCombinedMaskChild",
        Some("AuthoredCombinedMaskChild"),
    );
    world
        .players
        .get_mut(&0)
        .unwrap()
        .add_completed_upgrade(UPGRADE);
    apply(&mut world, source, UPGRADE);
    assert!(
        !world
            .host_object(source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(
        spawned_template_names(&world, source).is_empty(),
        "RequiresAllTriggers must wait while only the player-side A mask is set"
    );
    apply(&mut world, source, SECOND_UPGRADE);
    assert!(
        world
            .host_object(source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert_eq!(
        spawned_template_names(&world, source),
        vec!["AuthoredCombinedMaskChild".to_owned()],
        "RequiresAllTriggers must combine the receiving player's A bit with this object's B bit"
    );
}

#[test]
fn construction_gate_keeps_upgrade_tag_and_replays_module_after_completion() {
    let (mut world, source) = world(
        UPGRADE,
        "OCL_ConstructedCreationUpgradeChild",
        Some("ConstructedCreationUpgradeChild"),
    );
    world
        .host_object_mut(source)
        .unwrap()
        .status
        .under_construction = true;

    apply(&mut world, source, UPGRADE);
    assert!(world.host_object(source).unwrap().has_upgrade_tag(UPGRADE));
    assert!(
        !world
            .host_object(source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED"),
        "C++ updateUpgradeModules gates StatusBitsUpgrade during construction"
    );
    assert!(
        spawned_template_names(&world, source).is_empty(),
        "C++ updateUpgradeModules skips module callbacks during construction"
    );

    world
        .host_object_mut(source)
        .unwrap()
        .status
        .under_construction = false;
    world.apply_researched_player_upgrades_to_object_with_interpreter(
        source,
        None,
        &mut crate::game_logic::ScriptPresentationEffects::default(),
    );
    assert!(
        world
            .host_object(source)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED"),
        "construction completion replays the typed StatusBitsUpgrade module"
    );
    assert_eq!(
        spawned_template_names(&world, source),
        vec!["ConstructedCreationUpgradeChild".to_owned()],
        "the construction-complete update must re-evaluate the existing object upgrade mask"
    );
}

#[test]
fn authored_ocl_enslaves_slaved_child_without_drone_name_sniff() {
    // ObjectCreationUpgrade.cpp:68 -> ObjectCreationList::create(ocl, getObject());
    // GenericObjectCreationNugget (ObjectCreationList.cpp:985-993) calls
    // SlavedUpdate::onEnslave(sourceObj) for any created SlavedUpdate child,
    // independent of the upgrade or OCL spelling.
    const AUTHORED: &str = "Upgrade_AuthoredSlaveCompanion";
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(
        0,
        Team::USA,
        "SlaveOwner",
        true,
    ));
    admit_upgrade(&mut world, AUTHORED);
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "AuthoredSlaveOcl.ini".into(),
                "Object AuthoredSlaveHost\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = ObjectCreationUpgrade CreateCompanion\n  TriggeredBy = Upgrade_AuthoredSlaveCompanion\n  UpgradeObject = OCL_AuthoredSlaveCompanion\n End\nEnd\nObject AuthoredSlaveCompanion\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\n Behavior = SlavedUpdate SlaveTag\n  GuardMaxRange = 30\n End\nEnd\n".into(),
            )])
            .unwrap(),
        )
        .expect("authored content admits");
    world
        .admit_object_creation_list_sources([
            "ObjectCreationList OCL_AuthoredSlaveCompanion\n CreateObject\n  ObjectNames = AuthoredSlaveCompanion\n  Count = 1\n End\nEnd\n".to_owned(),
        ])
        .expect("OCL admits");
    let source = world
        .create_object_for_player("AuthoredSlaveHost", 0, glam::Vec3::ZERO)
        .expect("host creates");
    apply(&mut world, source, AUTHORED);
    let children: Vec<_> = world
        .objects
        .registered()
        .filter(|(id, _)| **id != source)
        .map(|(_, object)| object)
        .collect();
    assert_eq!(children.len(), 1);
    let child = children[0];
    assert_eq!(child.producer_id, Some(source));
    assert_eq!(
        child.slaved_update.as_ref().and_then(|state| state.master),
        Some(source),
        "OCL nugget must onEnslave the SlavedUpdate child to the receiving source"
    );
    assert!(
        child.status.unselectable,
        "startSlavedEffects marks UNSELECTABLE"
    );
}

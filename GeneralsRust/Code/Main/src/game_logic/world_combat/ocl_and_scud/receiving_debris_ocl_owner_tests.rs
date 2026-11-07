//! Receiving-world debris OCL ownership tests.

use super::*;

fn world(
    model_names: &str,
    count: u32,
    mass: f32,
    min_lifetime: u32,
    max_lifetime: u32,
) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
            .admit_object_creation_list_sources([format!(
                "ObjectCreationList OCL_ReceivingBridgeDebris\nCreateDebris\nModelNames = {model_names}\nCount = {count}\nMass = {mass}\nOffset = 1 2 3\nMinLifetime = {min_lifetime}f\nMaxLifetime = {max_lifetime}f\nDisposition = SEND_IT_FLYING\nDispositionIntensity = 1\nEnd\nEnd\n"
            )])
            .expect("OCL belongs to this session");
    let mut source_template = ThingTemplate::new("ReceivingBridgeSource");
    source_template
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_health(100.0);
    let mut debris_template = ThingTemplate::new("GenericDebris");
    debris_template
        .add_kind_of(crate::game_logic::KindOf::Projectile)
        .set_health(100.0);
    debris_template.native_lifecycle_definitions =
        Some(
            vec![crate::game_logic::native_lifecycle_definition::NativeLifecycleDefinition {
                module_tag: Some("DebrisLife".into()),
                kind: crate::game_logic::native_lifecycle_definition::NativeLifecycleKind::Lifetime(
                    crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                        minimum: 1,
                        maximum: 1,
                    },
                ),
            }]
            .into_boxed_slice(),
        );
    world
        .templates
        .insert("ReceivingBridgeSource".into(), source_template);
    world
        .templates
        .insert("GenericDebris".into(), debris_template);
    // The receiving model override is only meaningful through an authored
    // W3DDebrisDraw receiver. Keep this fixture on the same module path as
    // the retail GenericDebris template rather than inventing a Draw row.
    let mut parser = crate::assets::IniParser::new();
    parser
            .parse_ini_content(
                "Object GenericDebris\nKindOf = PROJECTILE\nBody = ActiveBody DebrisBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nDraw = W3DDebrisDraw DebrisDrawTag\nEnd\nEnd\n",
                "generic-debris-owner-test.ini",
            )
            .expect("authored GenericDebris Draw fixture");
    let definition = parser
        .get_definition("GenericDebris")
        .expect("GenericDebris definition")
        .clone();
    world.seed_asset_definition_templates_from_snapshot([("GenericDebris".into(), definition)]);
    let source = world
        .create_object(
            "ReceivingBridgeSource",
            Team::USA,
            Vec3::new(20.0, 0.0, 30.0),
        )
        .expect("source object");
    world
        .objects
        .get_mut(&source)
        .expect("source Object")
        .set_orientation_with_partition_owner(
            std::f32::consts::FRAC_PI_2,
            &mut world.partition_manager,
        );
    (world, source)
}

fn helix_portable_world(position: Vec3) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_combat_bonus_ini_sources([
        "GameData\n WeaponBonus = GARRISONED RANGE 133%\nEnd\n"
    ]).unwrap();
    world
            .admit_object_creation_list_sources(["ObjectCreationList OCL_HelixGattlingCannon\nCreateObject\nObjectNames = ChinaTankOverlordGattlingCannon\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd".into()])
            .expect("retail Helix addon OCL parses");
    let mut helix = ThingTemplate::new("ChinaJetHelix");
    helix
        .add_kind_of(crate::game_logic::KindOf::Aircraft)
        .set_health(100.0);
    helix.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    helix.contain_module.is_helix_contain = true;
    let mut payload = ThingTemplate::new("ChinaTankOverlordGattlingCannon");
    payload
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .add_kind_of(crate::game_logic::KindOf::PortableStructure)
        .set_health(100.0);
    world.templates.insert(helix.name.clone(), helix);
    world.templates.insert(payload.name.clone(), payload);
    let source = world
        .create_object("ChinaJetHelix", Team::China, position)
        .expect("Helix source");
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .install_helix_transport();
    (world, source)
}

#[test]
fn retail_helix_contain_inside_source_ocl_attaches_in_each_receiving_world() {
    let (mut left, left_source) = helix_portable_world(Vec3::new(10.0, 0.0, 20.0));
    let (mut right, right_source) = helix_portable_world(Vec3::new(100.0, 0.0, 200.0));
    assert_eq!(left_source, right_source);

    let left_ids = left.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon",
        Some(left_source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 20.0),
    );
    let right_ids = right.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon",
        Some(right_source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(100.0, 0.0, 200.0),
    );

    assert_eq!(left_ids.len(), 1);
    assert_eq!(right_ids.len(), 1);
    assert_eq!(left_ids[0], right_ids[0]);
    for (world, source, child) in [
        (&left, left_source, left_ids[0]),
        (&right, right_source, right_ids[0]),
    ] {
        let host = world.host_object(source).unwrap();
        let payload = world.host_object(child).unwrap();
        assert_eq!(host.overlord_portable_occupant, Some(child));
        assert_eq!(payload.contained_by, Some(source));
        assert!(!payload.status.disabled_held, "Helix portable slot bypasses onContaining");
        assert!(!host.occupants.contains(&child));
        assert!((world.receiving_weapon_bonus_fields_for_slot(payload, 0).1 - 1.0).abs() < 0.001);
        assert!(!host.is_enclosing_container_for(payload));
        assert_eq!(payload.template_name, "ChinaTankOverlordGattlingCannon");
    }
}

#[test]
fn helix_portable_rider_fires_its_own_weapon_without_passenger_upgrade() {
    let (mut world, helix) = helix_portable_world(Vec3::ZERO);
    let rider = world.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon", Some(helix), Team::China,
        VeterancyLevel::Rookie, 0.0, Vec3::ZERO, Vec3::ZERO,
    )[0];
    {
        let object = world.host_object_mut(rider).unwrap();
        assert!(!object.status.disabled_held);
        let mut weapon = crate::game_logic::Weapon::default();
        weapon.last_fire_time = -10.0;
        weapon.reload_time = 0.1;
        weapon.range = 150.0;
        weapon.damage = 10.0;
        object.weapon = Some(weapon);
    }
    let mut victim = ThingTemplate::new("HelixRiderTarget");
    victim.add_kind_of(crate::game_logic::KindOf::Infantry)
        .add_kind_of(crate::game_logic::KindOf::Attackable).set_health(100.0);
    world.templates.insert(victim.name.clone(), victim);
    let target = world.create_object("HelixRiderTarget", Team::USA, Vec3::new(20.0, 0.0, 0.0)).unwrap();
    let before = world.host_object(target).unwrap().health.current;
    world.set_current_frame(30);
    world.try_transport_passenger_residual_fire(rider);
    assert!(world.host_object(target).unwrap().health.current < before);
    assert!(!world.host_object(helix).unwrap().passengers_allowed_to_fire);
}

#[test]
fn retail_helix_contain_inside_rejects_an_occupied_portable_slot() {
    let (mut world, source) = helix_portable_world(Vec3::ZERO);
    world
        .objects
        .get_mut(&source)
        .unwrap()
        .overlord_portable_occupant = Some(ObjectId(999));

    let created = world.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon",
        Some(source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::ZERO,
    );

    assert_eq!(
        created.len(),
        1,
        "the OCL still returns its committed creation receipt"
    );
    assert!(world.host_object(created[0]).unwrap().status.destroyed);
    assert_eq!(
        world
            .host_object(source)
            .unwrap()
            .overlord_portable_occupant,
        Some(ObjectId(999))
    );
    assert!(
        !world
            .host_object(source)
            .unwrap()
            .occupants
            .contains(&created[0])
    );
}

#[test]
fn other_contain_inside_source_routes_remain_guarded_before_creation() {
    let (mut world, source) = helix_portable_world(Vec3::ZERO);
    let host = world.objects.get_mut(&source).unwrap();
    host.is_helix_transport = false;
    host.template_mut().contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    host.template_mut().contain_module.slots = Some(1);
    host.max_transport = 1;

    let created = world.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon",
        Some(source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::ZERO,
    );

    assert!(created.is_empty());
    assert!(
        world
            .host_objects()
            .registered_values()
            .all(|object| object.template_name != "ChinaTankOverlordGattlingCannon")
    );
}

#[test]
fn portable_containment_rejects_replaced_source_or_child_birth() {
    for replace_source in [true, false] {
        let (mut world, source) = helix_portable_world(Vec3::ZERO);
        let child = world
            .create_object("ChinaTankOverlordGattlingCannon", Team::China, Vec3::ZERO)
            .expect("portable payload");
        let source_birth = (world.host_object(source).unwrap().visual_object_generation, world.visual_world_epoch);
        let child_birth = (world.host_object(child).unwrap().visual_object_generation, world.visual_world_epoch);
        let replaced_id = if replace_source { source } else { child };
        let replacement = world.objects.remove(&replaced_id).unwrap();
        world.try_admit_host_object(replacement).unwrap();

        assert!(!world.admit_ocl_portable_structure(source, child, source_birth, child_birth));
        assert_eq!(
            world
                .host_object(source)
                .unwrap()
                .overlord_portable_occupant,
            None
        );
        assert_eq!(world.host_object(child).unwrap().contained_by, None);
        let child_matches = world.spawn_identity_matches(child, child_birth.0, child_birth.1);
        assert_eq!(child_matches, replace_source);
        if child_matches {
            world.destroy_object_with_effects(child, &mut Default::default()).unwrap();
        }
        if replace_source {
            assert!(world.host_object(child).unwrap().status.destroyed);
        } else {
            assert!(!world.host_object(child).unwrap().status.destroyed);
        }
    }
}

#[test]
fn portable_admission_uses_authored_kindof_not_template_spelling() {
    let (mut world, custom_source) = helix_portable_world(Vec3::ZERO);
    world
            .admit_object_creation_list_sources([
                "ObjectCreationList OCL_CustomPortable\nCreateObject\nObjectNames = CustomAddonPayload\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd".into(),
                "ObjectCreationList OCL_MisleadingPortable\nCreateObject\nObjectNames = ChinaTankHelixPretender\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd".into(),
            ])
            .expect("custom containment OCLs parse");
    let mut misleading = ThingTemplate::new("ChinaTankHelixPretender");
    misleading
        .add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(100.0);
    let mut parser = crate::assets::IniParser::new();
    parser
            .parse_ini_content(
                "Object CustomAddonPayload\nKindOf = VEHICLE PORTABLE_STRUCTURE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n",
                "portable-kindof-test.ini",
            )
            .expect("authored portable KindOf parses");
    let definition = parser
        .get_definition("CustomAddonPayload")
        .expect("custom payload definition")
        .clone();
    world
        .seed_asset_definition_templates_from_snapshot([("CustomAddonPayload".into(), definition)]);
    world.templates.insert(misleading.name.clone(), misleading);

    let tagged_ids = world.execute_parsed_weapon_ocl_at(
        "OCL_CustomPortable",
        Some(custom_source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::ZERO,
    );
    assert_eq!(tagged_ids.len(), 1);
    assert!(
        world
            .host_object(tagged_ids[0])
            .unwrap()
            .is_kind_of(crate::game_logic::KindOf::PortableStructure)
    );
    assert_eq!(
        world.host_object(tagged_ids[0]).unwrap().contained_by,
        Some(custom_source)
    );

    let mut misleading_host = ThingTemplate::new("ChinaJetHelix");
    misleading_host
        .add_kind_of(crate::game_logic::KindOf::Aircraft)
        .set_health(100.0);
    world
        .templates
        .insert(misleading_host.name.clone(), misleading_host);
    let misleading_source = world
        .create_object("ChinaJetHelix", Team::China, Vec3::ZERO)
        .expect("second Helix source");
    world
        .objects
        .get_mut(&misleading_source)
        .unwrap()
        .install_helix_transport();
    let misleading_ids = world.execute_parsed_weapon_ocl_at(
        "OCL_MisleadingPortable",
        Some(misleading_source),
        Team::China,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::ZERO,
    );
    assert!(misleading_ids.is_empty());
    assert!(
        world
            .host_objects()
            .registered_values()
            .all(|object| { object.template_name != "ChinaTankHelixPretender" })
    );
    assert_eq!(
        world
            .host_object(misleading_source)
            .unwrap()
            .overlord_portable_occupant,
        None
    );
}

#[test]
fn later_factory_error_retains_the_ordered_committed_ocl_prefix() {
    let mut world = GameLogic::new();
    world
            .admit_object_creation_list_sources(["ObjectCreationList OCL_PartialFactoryError\nCreateObject\nObjectNames = OclPartialGood\nCount = 1\nEnd\nCreateObject\nObjectNames = OclPartialBad\nCount = 1\nEnd\nEnd\n".into()])
            .expect("OCL is valid receiving content");
    let mut source = ThingTemplate::new("OclPartialSource");
    source
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .set_health(100.0);
    let mut good = ThingTemplate::new("OclPartialGood");
    good.add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(100.0);
    let mut bad = ThingTemplate::new("OclPartialBad");
    bad.add_kind_of(crate::game_logic::KindOf::Vehicle)
        .set_health(100.0);
    // Private factory fault injection after receiving OCL admission. The
    // public content parser correctly rejects this malformed Body field;
    // this test targets the later-factory-error boundary itself.
    bad.body_definition_error = Some("injected checked factory error".into());
    world.templates.insert(source.name.clone(), source);
    world.templates.insert(good.name.clone(), good);
    world.templates.insert(bad.name.clone(), bad);
    let source_id = world
        .create_object("OclPartialSource", Team::USA, Vec3::ZERO)
        .expect("OCL source object");

    let created = world.execute_parsed_weapon_ocl_at(
        "OCL_PartialFactoryError",
        Some(source_id),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::ZERO,
    );

    assert_eq!(created.len(), 1);
    assert_eq!(
        world.host_object(created[0]).unwrap().template_name,
        "OclPartialGood"
    );
    assert!(
        world
            .host_objects()
            .registered_values()
            .all(|object| object.template_name != "OclPartialBad")
    );
}

#[test]
fn parsed_bridge_debris_is_owned_per_world_and_uses_receiving_lod_mask() {
    let (mut left, left_source) = world("LeftChunk", 2, 5.0, 10, 20);
    let (mut right, right_source) = world("RightChunk", 2, 5.0, 10, 20);
    assert_eq!(left_source, right_source);

    // C++ isDebrisSkipped increments then masks; mask 1 keeps generation 1
    // and skips generation 2. The peer world has an independent counter.
    left.admit_dynamic_debris_skip_mask(1);
    let left_ids = left.execute_parsed_weapon_ocl_at(
        "OCL_ReceivingBridgeDebris",
        Some(left_source),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(20.0, 0.0, 30.0),
    );
    let right_ids = right.execute_parsed_weapon_ocl_at(
        "OCL_ReceivingBridgeDebris",
        Some(right_source),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(20.0, 0.0, 30.0),
    );

    assert_eq!(left_ids.len(), 1);
    assert_eq!(right_ids.len(), 2);
    assert_eq!(left_ids[0], right_ids[0]);
    let left_chunk = left.objects.get(&left_ids[0]).expect("left debris");
    let right_chunk = right.objects.get(&right_ids[0]).expect("right debris");
    assert_eq!(left_chunk.template_name, "GenericDebris");
    assert_eq!(right_chunk.template_name, "GenericDebris");
    assert_eq!(
        left_chunk.debris_draw.as_ref().unwrap().model_name,
        "LeftChunk"
    );
    assert_eq!(
        right_chunk.debris_draw.as_ref().unwrap().model_name,
        "RightChunk"
    );
    assert_eq!(
        left_chunk.get_visual_info().model_name.as_deref(),
        Some("LeftChunk")
    );
    assert_eq!(
        right_chunk.get_visual_info().model_name.as_deref(),
        Some("RightChunk")
    );
    assert_eq!(left_chunk.physics_mass, 5.0);
    assert_eq!(right_chunk.physics_mass, 5.0);
    let expected_offset =
        glam::Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2) * Vec3::new(1.0, 2.0, 3.0);
    assert!(
        (left_chunk.get_position() - (Vec3::new(20.0, 0.0, 30.0) + expected_offset)).length()
            < 1e-4
    );
    assert_eq!(
        left.live_drawable_model_names(),
        vec!["LeftChunk".to_owned()]
    );
    assert_eq!(
        right.live_drawable_model_names(),
        vec!["RightChunk".to_owned()]
    );
    assert_eq!(left.templates["GenericDebris"].model_name, None);
    assert_eq!(right.templates["GenericDebris"].model_name, None);

    // Exercise the production presentation projection consumed by
    // RenderPipeline, not only ObjectVisualInfo or the shell preloader.
    let left_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut left, 0);
    let right_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut right, 0);
    let left_mesh = left_frame
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == left_ids[0])
        .expect("left debris reaches the production mesh collector");
    let right_mesh = right_frame
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == right_ids[0])
        .expect("right debris reaches the production mesh collector");
    assert_eq!(left_mesh.template_name, "GenericDebris");
    assert_eq!(right_mesh.template_name, "GenericDebris");
    assert_eq!(
        left_mesh
            .receiving_draw_models
            .as_deref()
            .unwrap()
            .module_class_names()
            .as_slice(),
        ["W3DDebrisDraw"]
    );
    assert_eq!(
        right_mesh
            .receiving_draw_models
            .as_deref()
            .unwrap()
            .module_class_names()
            .as_slice(),
        ["W3DDebrisDraw"]
    );
    assert_eq!(left_mesh.model_key, "LeftChunk");
    assert_eq!(right_mesh.model_key, "RightChunk");
    assert_eq!(left_mesh.draw_models[0].module_index, 0);
    assert_eq!(right_mesh.draw_models[0].module_index, 0);
    assert_eq!(left_mesh.draw_models[0].model_key, "LeftChunk");
    assert_eq!(right_mesh.draw_models[0].model_key, "RightChunk");
}

#[test]
fn debris_model_override_does_not_invent_missing_receiving_draw_geometry() {
    let mut world = GameLogic::new();
    world
            .admit_object_creation_list_sources(["ObjectCreationList OCL_NoDebrisDraw\nCreateDebris\nModelNames = AuthoredChunk\nCount = 1\nMass = 5\nDisposition = SEND_IT_FLYING\nDispositionIntensity = 1\nEnd\nEnd".into()])
            .unwrap();
    let mut source_template = ThingTemplate::new("NoDrawSource");
    source_template.set_health(100.0);
    let mut debris_template = ThingTemplate::new("GenericDebris");
    debris_template
        .add_kind_of(crate::game_logic::KindOf::Projectile)
        .set_health(100.0);
    world
        .templates
        .insert("NoDrawSource".into(), source_template);
    world
        .templates
        .insert("GenericDebris".into(), debris_template);
    let source = world
        .create_object("NoDrawSource", Team::USA, Vec3::ZERO)
        .unwrap();
    let ids = world.execute_parsed_weapon_ocl_at(
        "OCL_NoDebrisDraw",
        Some(source),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::ZERO,
    );
    assert_eq!(ids.len(), 1);
    assert_eq!(
        world.objects[&ids[0]]
            .debris_draw
            .as_ref()
            .unwrap()
            .model_name,
        "AuthoredChunk"
    );

    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    let input = frame
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == ids[0])
        .unwrap();
    assert_eq!(input.template_name, "GenericDebris");
    assert!(input.receiving_draw_models.is_none());
    assert_ne!(input.model_key, "AuthoredChunk");
    assert!(
        input
            .draw_models
            .iter()
            .all(|draw| draw.model_key != "AuthoredChunk")
    );
}

#[test]
fn authored_debris_model_choices_mass_and_lifetime_ranges_stay_with_each_owner() {
    let (mut left, left_source) = world("LeftA LeftB", 6, 5.0, 10, 20);
    let (mut right, right_source) = world("RightA RightB", 6, 9.0, 30, 40);
    assert_eq!(left_source, right_source);

    let left_ids = left.execute_parsed_weapon_ocl_at(
        "OCL_ReceivingBridgeDebris",
        Some(left_source),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(20.0, 0.0, 30.0),
    );
    let right_ids = right.execute_parsed_weapon_ocl_at(
        "OCL_ReceivingBridgeDebris",
        Some(right_source),
        Team::USA,
        VeterancyLevel::Rookie,
        0.0,
        Vec3::ZERO,
        Vec3::new(20.0, 0.0, 30.0),
    );
    assert_eq!(left_ids.len(), 6);
    assert_eq!(right_ids.len(), 6);

    let mut left_choices = Vec::new();
    let mut right_choices = Vec::new();
    for (left_id, right_id) in left_ids.iter().zip(&right_ids) {
        let left_debris = left.objects.get(left_id).expect("left debris object");
        let right_debris = right.objects.get(right_id).expect("right debris object");
        assert_eq!(left_debris.template_name, "GenericDebris");
        assert_eq!(right_debris.template_name, "GenericDebris");
        let left_model = left_debris
            .debris_draw
            .as_ref()
            .unwrap()
            .model_name
            .as_str();
        let right_model = right_debris
            .debris_draw
            .as_ref()
            .unwrap()
            .model_name
            .as_str();
        assert!(matches!(left_model, "LeftA" | "LeftB"));
        assert!(matches!(right_model, "RightA" | "RightB"));
        // OCL ModelNames performs one receiving logic-RNG choice per item;
        // peer owners start from the same seed, so the selected list index
        // is equal despite distinct model namespaces and lifetime ranges.
        left_choices.push(left_model.ends_with('B'));
        right_choices.push(right_model.ends_with('B'));
        assert_eq!(left_debris.physics_mass, 5.0);
        assert_eq!(right_debris.physics_mass, 9.0);
        let left_deadline = left_debris
            .native_lifecycle
            .as_ref()
            .and_then(|runtime| runtime.first_lifetime_deadline())
            .expect("receiving LifetimeUpdate");
        let right_deadline = right_debris
            .native_lifecycle
            .as_ref()
            .and_then(|runtime| runtime.first_lifetime_deadline())
            .expect("receiving LifetimeUpdate");
        assert!((10..=20).contains(&left_deadline.saturating_sub(left.frame)));
        assert!((30..=40).contains(&right_deadline.saturating_sub(right.frame)));
    }
    assert_eq!(left_choices, right_choices);

    let left_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut left, 0);
    let right_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut right, 0);
    for (id, model) in left_ids.iter().zip(left_choices.iter()) {
        let expected = if *model { "LeftB" } else { "LeftA" };
        let submitted = left_frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == *id)
            .unwrap();
        assert_eq!(submitted.model_key, expected);
    }
    for (id, model) in right_ids.iter().zip(right_choices.iter()) {
        let expected = if *model { "RightB" } else { "RightA" };
        let submitted = right_frame
            .unit_render_inputs()
            .into_iter()
            .find(|input| input.id == *id)
            .unwrap();
        assert_eq!(submitted.model_key, expected);
    }
}

#[test]
fn deleting_helix_destroys_separate_portable_rider() {
    let (mut world, helix) = helix_portable_world(Vec3::ZERO);
    let rider = world.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon", Some(helix), Team::China,
        VeterancyLevel::Rookie, 0.0, Vec3::ZERO, Vec3::ZERO,
    )[0];
    assert!(!world.host_object(helix).unwrap().occupants.contains(&rider));
    world.destroy_object(helix).unwrap();
    assert!(world.host_object(rider).is_none_or(|object| object.status.destroyed));
}

#[test]
fn helix_portable_update_follows_hull_position_and_orientation() {
    let (mut world, helix) = helix_portable_world(Vec3::new(20.0, 0.0, 30.0));
    let rider = world.execute_parsed_weapon_ocl_at(
        "OCL_HelixGattlingCannon", Some(helix), Team::China,
        VeterancyLevel::Rookie, 0.0, Vec3::ZERO, Vec3::ZERO,
    )[0];
    {
        let (host, partition) = world.host_object_and_partition_mut(helix).unwrap();
        host.set_orientation_with_partition_owner(1.2, partition);
    }
    world.mirror_overlord_addon_damage_to_occupant(helix);
    assert_eq!(world.host_object(rider).unwrap().get_position(), Vec3::new(20.0, 0.0, 30.0));
    assert!((world.host_object(rider).unwrap().get_orientation() - 1.2).abs() < 0.0001);
}

use super::*;

// C++ Module.h:96-99,138-144: empty tables still parse and malformed module
// fields propagate out of friend_newModuleData. No module instance is made.
const MODULES: &[(ModuleType, &str)] = &[
    (ModuleType::Draw, "W3DModelDraw"),
    (ModuleType::Draw, "W3DDefaultDraw"),
    (ModuleType::Draw, "W3DDependencyModelDraw"),
    (ModuleType::Draw, "W3DTankDraw"),
    (ModuleType::Draw, "W3DOverlordTankDraw"),
    (ModuleType::Draw, "W3DOverlordAircraftDraw"),
    (ModuleType::Draw, "W3DOverlordTruckDraw"),
    (ModuleType::Draw, "W3DPoliceCarDraw"),
    (ModuleType::Draw, "W3DProjectileStreamDraw"),
    (ModuleType::Draw, "W3DRopeDraw"),
    (ModuleType::Draw, "W3DScienceModelDraw"),
    (ModuleType::Draw, "W3DSupplyDraw"),
    (ModuleType::Draw, "W3DTankTruckDraw"),
    (ModuleType::Draw, "W3DTracerDraw"),
    (ModuleType::Draw, "W3DTreeDraw"),
    (ModuleType::Draw, "W3DPropDraw"),
    (ModuleType::Draw, "W3DTruckDraw"),
    (ModuleType::Draw, "W3DLaserDraw"),
    (ModuleType::Draw, "W3DDebrisDraw"),
    (ModuleType::ClientUpdate, "LaserUpdate"),
    (ModuleType::ClientUpdate, "BeaconClientUpdate"),
    (ModuleType::ClientUpdate, "SwayClientUpdate"),
    (
        ModuleType::ClientUpdate,
        "AnimatedParticleSysBoneClientUpdate",
    ),
    (ModuleType::Behavior, "InactiveBody"),
    (ModuleType::Behavior, "ActiveBody"),
    (ModuleType::Behavior, "StructureBody"),
    (ModuleType::Behavior, "HighlanderBody"),
    (ModuleType::Behavior, "ImmortalBody"),
    (ModuleType::Behavior, "HiveStructureBody"),
    (ModuleType::Behavior, "UndeadBody"),
];

struct DataParsers {
    callbacks: std::collections::HashMap<(usize, &'static str), FallibleModuleDataProc>,
}

impl DataParserRegistrar for DataParsers {
    fn register_data_parser(
        &mut self,
        name: &'static str,
        kind: ModuleType,
        parser: FallibleModuleDataProc,
    ) {
        assert!(
            self.callbacks
                .insert((kind as usize, name), parser)
                .is_none(),
            "duplicate parser {name}"
        );
    }
}

impl DataParsers {
    fn construct(
        &self,
        ini: Option<&mut INI>,
        name: &str,
        kind: ModuleType,
    ) -> Result<Arc<dyn ModuleData>, String> {
        let parser = self
            .callbacks
            .iter()
            .find_map(|(&(registered_kind, registered_name), parser)| {
                (registered_kind == kind as usize && registered_name == name).then_some(*parser)
            })
            .ok_or_else(|| format!("No actual typed parser for {name}"))?;
        parser(ini).map(Arc::from)
    }
}

fn catalog() -> DataParsers {
    let mut parsers = DataParsers {
        callbacks: std::collections::HashMap::new(),
    };
    register_native_data_parsers(&mut parsers);
    parsers
}

fn parse(
    factory: &mut DataParsers,
    kind: ModuleType,
    name: &str,
    body: &str,
) -> Result<Arc<dyn ModuleData>, String> {
    INI::new()
        .with_inline_source(body, |ini| Ok(factory.construct(Some(ini), name, kind)))
        .expect("owned inline source")
}

#[test]
fn every_native_draw_client_update_and_body_has_an_error_preserving_parser() {
    let mut factory = catalog();
    assert_eq!(MODULES.len(), 30);
    for &(kind, name) in MODULES {
        let data = parse(&mut factory, kind, name, "End\n")
            .unwrap_or_else(|error| panic!("{name} empty/default data: {error}"));
        assert_eq!(
            data.get_module_tag_name_key(),
            game_engine::common::name_key_generator::NAMEKEY_INVALID,
            "{name} validation data remains unkeyed until runtime admission"
        );
    }
}

#[test]
fn every_native_data_parser_propagates_unknown_fields_including_empty_tables() {
    let mut factory = catalog();
    for &(kind, name) in MODULES {
        let error = parse(
            &mut factory,
            kind,
            name,
            "NotAnAuthoredModuleField = 4\nEnd\n",
        )
        .expect_err(name);
        assert!(error.contains(name), "class diagnostic: {error}");
        assert!(error.contains("line"), "source diagnostic: {error}");
    }
}

#[test]
fn every_native_data_parser_rejects_an_unterminated_body() {
    let mut factory = catalog();
    for &(kind, name) in MODULES {
        assert!(parse(&mut factory, kind, name, "\n").is_err(), "{name}");
    }
}

#[test]
fn strict_data_keeps_real_typed_values_and_independent_receiving_catalogs() {
    let mut a = catalog();
    let mut b = catalog();
    let draw = parse(
        &mut a,
        ModuleType::Draw,
        "W3DLaserDraw",
        "NumBeams = 7\nEnd\n",
    )
    .expect("typed laser draw");
    assert_eq!(
        draw.as_any()
            .downcast_ref::<W3DLaserDrawModuleData>()
            .unwrap()
            .num_beams,
        7
    );
    let peer = parse(
        &mut b,
        ModuleType::Draw,
        "W3DLaserDraw",
        "NumBeams = 2\nEnd\n",
    )
    .expect("peer typed laser draw");
    assert_eq!(
        peer.as_any()
            .downcast_ref::<W3DLaserDrawModuleData>()
            .unwrap()
            .num_beams,
        2
    );
    assert_eq!(
        draw.as_any()
            .downcast_ref::<W3DLaserDrawModuleData>()
            .unwrap()
            .num_beams,
        7
    );

    let body = parse(
        &mut a,
        ModuleType::Behavior,
        "ActiveBody",
        "MaxHealth = 123.5\nEnd\n",
    )
    .expect("typed body");
    assert_eq!(
        body.as_any()
            .downcast_ref::<ActiveBodyModuleData>()
            .unwrap()
            .max_health,
        123.5
    );
    let client = parse(
        &mut a,
        ModuleType::ClientUpdate,
        "BeaconClientUpdate",
        "RadarPulseFrequency = 2000ms\nEnd\n",
    )
    .expect("typed client update");
    assert_eq!(
        client
            .as_any()
            .downcast_ref::<BeaconClientUpdateModuleData>()
            .unwrap()
            .frames_between_radar_pulses,
        60
    );
}

#[test]
fn strict_numeric_errors_do_not_change_compatibility_constructor_behavior() {
    let mut factory = catalog();
    let mut compatibility_factory = ModuleFactory::new();
    super::super::register_native_module_overrides(&mut compatibility_factory).unwrap();
    for (kind, name, body) in [
        (
            ModuleType::Draw,
            "W3DLaserDraw",
            "NumBeams = invalid\nEnd\n",
        ),
        (
            ModuleType::Behavior,
            "ActiveBody",
            "MaxHealth = invalid\nEnd\n",
        ),
        (
            ModuleType::ClientUpdate,
            "BeaconClientUpdate",
            "RadarPulseFrequency = invalid\nEnd\n",
        ),
    ] {
        assert!(parse(&mut factory, kind, name, body).is_err(), "{name}");
        INI::new()
            .with_inline_source(body, |ini| {
                assert!(
                    compatibility_factory
                        .new_module_data_from_ini(Some(ini), name, kind, "Compatibility")
                        .is_ok(),
                    "{name}"
                );
                Ok(())
            })
            .expect("owned compatibility source");
    }
}

fn behavior_classes() -> impl Iterator<Item = &'static str> {
    [
        death::NAMES,
        projectile_updates::NAMES,
        updates::NAMES,
        production::NAMES,
        ai_upgrades_powers::NAMES,
        behaviors::NAMES,
        collide::NAMES,
        creation::NAMES,
        contain::NAMES,
    ]
    .into_iter()
    .flatten()
    .copied()
}

#[test]
fn every_registered_behavior_class_has_real_typed_fallible_data() {
    let mut factory = catalog();
    let mut names = std::collections::HashSet::new();
    for name in behavior_classes() {
        assert!(names.insert(name), "duplicate strict registration {name}");
        let data = factory
            .construct(None, name, ModuleType::Behavior)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            !data
                .as_any()
                .is::<game_engine::common::thing::module::CapturedModuleData>(),
            "{name} must retain actual typed data"
        );
    }
    assert_eq!(names.len(), 187);
    for name in [
        "DestroyDie",
        "PhysicsBehavior",
        "AIUpdateInterface",
        "OpenContain",
        "ProductionUpdate",
        "GrantUpgradeCreate",
    ] {
        assert!(names.contains(name), "critical native family {name}");
    }
}

#[test]
fn native_behavior_data_propagates_real_parser_errors() {
    let mut factory = catalog();
    for (name, body) in [
        ("DestroyDie", "NotADieField = 1\nEnd\n"),
        ("PhysicsBehavior", "Mass = invalid\nEnd\n"),
        (
            "AIUpdateInterface",
            "AutoAcquireEnemiesWhenIdle = invalid\nEnd\n",
        ),
        ("OpenContain", "ContainMax = invalid\nEnd\n"),
        ("ProductionUpdate", "NumDoorAnimations = invalid\nEnd\n"),
        ("GrantUpgradeCreate", "NotACreateField = 1\nEnd\n"),
    ] {
        let error = parse(&mut factory, ModuleType::Behavior, name, body).expect_err(name);
        assert!(error.contains(name), "{name}: {error}");
        assert!(error.contains("line"), "{name}: {error}");
    }
}

#[test]
fn native_behavior_data_keeps_physics_ai_contain_and_production_values() {
    let mut factory = catalog();
    let physics = parse(
        &mut factory,
        ModuleType::Behavior,
        "PhysicsBehavior",
        "Mass = 12.5\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        physics
            .as_any()
            .downcast_ref::<PhysicsBehaviorModuleData>()
            .unwrap()
            .mass,
        12.5
    );
    let ai = parse(
        &mut factory,
        ModuleType::Behavior,
        "AIUpdateInterface",
        "AutoAcquireEnemiesWhenIdle = YES\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        ai.as_any()
            .downcast_ref::<AIUpdateModuleData>()
            .unwrap()
            .auto_acquire_enemies_when_idle(),
        crate::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE
    );
    let contain = parse(
        &mut factory,
        ModuleType::Behavior,
        "OpenContain",
        "ContainMax = 7\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        contain
            .as_any()
            .downcast_ref::<super::super::contain::ContainModuleDataAdapter<OpenContainModuleData>>(
            )
            .unwrap()
            .contain_data()
            .contain_max,
        7
    );
    let production = parse(
        &mut factory,
        ModuleType::Behavior,
        "ProductionUpdate",
        "NumDoorAnimations = 3\nEnd\n",
    )
    .unwrap();
    assert_eq!(
        production
            .as_any()
            .downcast_ref::<ProductionUpdateModuleData>()
            .unwrap()
            .num_door_animations,
        3
    );
}

#[path = "inherited_fields_tests.rs"]
mod inherited_fields;

#[test]
fn data_only_default_callbacks_do_not_allocate_ambient_module_tag_keys() {
    // Only default/empty data is covered here. Authored model bone/name keys
    // still require the separate explicit identity migration (hq-bl6s7).
    let before = NameKeyGenerator::name_to_key("StrictNativeDataTagSentinelBefore");
    let mut parsers = catalog();
    for &(kind, name) in MODULES {
        let data = parse(&mut parsers, kind, name, "End\n").unwrap();
        assert_eq!(data.get_module_tag_name_key(), 0, "{name}");
    }
    for name in behavior_classes() {
        let data = parsers.construct(None, name, ModuleType::Behavior).unwrap();
        assert_eq!(data.get_module_tag_name_key(), 0, "{name}");
    }
    let after = NameKeyGenerator::name_to_key("StrictNativeDataTagSentinelAfter");
    assert_eq!(after, before + 1, "default data allocated an ambient key");
}

#[test]
fn native_catalog_records_actual_class_masks_before_authored_validation() {
    use game_engine::common::thing::module::ModuleInterfaceType;
    let mut factory = ModuleFactory::new();
    super::super::register_native_module_overrides(&mut factory).unwrap();
    for &(kind, name) in MODULES {
        let expected = match kind {
            ModuleType::Draw => ModuleInterfaceType::DRAW,
            ModuleType::ClientUpdate => ModuleInterfaceType::CLIENT_UPDATE,
            ModuleType::Behavior => ModuleInterfaceType::BODY,
            _ => unreachable!(),
        };
        assert_eq!(
            factory.find_module_interface_mask(name, kind),
            expected,
            "{name}"
        );
    }
    for name in behavior_classes() {
        assert_ne!(
            factory.find_module_interface_mask(name, ModuleType::Behavior),
            ModuleInterfaceType::NONE,
            "actual registered class {name} cannot acquire its mask from an authored field"
        );
    }
    for kind in [ModuleType::Draw, ModuleType::ClientUpdate] {
        let actual: Vec<_> = factory
            .descriptors_in_registration_order(kind)
            .iter()
            .map(|descriptor| descriptor.name.to_string())
            .collect();
        let expected: Vec<_> = MODULES
            .iter()
            .filter_map(|&(module_kind, name)| (module_kind == kind).then_some(name.to_owned()))
            .collect();
        assert_eq!(actual, expected, "actual class registration order");
    }
}

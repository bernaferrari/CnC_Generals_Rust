//! Equivalence to the former independently constructed builtin catalog.

use super::*;

const TYPES: [ModuleType; 3] = [
    ModuleType::Behavior,
    ModuleType::Draw,
    ModuleType::ClientUpdate,
];

#[test]
fn every_builtin_mask_matches_a_fresh_factory_for_all_module_types() {
    let factory = ModuleFactory::new();
    for &(name, _) in BUILTIN_BEHAVIOR_DESCRIPTORS {
        for module_type in TYPES {
            assert_eq!(
                ModuleFactory::find_builtin_module_interface_mask(name, module_type),
                factory.find_module_interface_mask(name, module_type),
                "builtin query differs for {name:?} / {module_type:?}",
            );
        }
    }
}

#[test]
fn builtin_name_case_empty_missing_and_whitespace_match_fresh_catalog() {
    let factory = ModuleFactory::new();
    for &(name, _) in BUILTIN_BEHAVIOR_DESCRIPTORS {
        for candidate in [
            name.to_ascii_lowercase(),
            name.to_ascii_uppercase(),
            format!(" {name}"),
            format!("{name} "),
        ] {
            for module_type in TYPES {
                assert_eq!(
                    ModuleFactory::find_builtin_module_interface_mask(&candidate, module_type),
                    factory.find_module_interface_mask(&candidate, module_type),
                    "name normalization changed for {candidate:?} / {module_type:?}",
                );
            }
        }
    }
    for name in [
        "",
        "NotRegistered",
        "activebody",
        "W3DModelDraw",
        "SwayClientUpdate",
    ] {
        for module_type in TYPES {
            assert_eq!(
                ModuleFactory::find_builtin_module_interface_mask(name, module_type),
                factory.find_module_interface_mask(name, module_type),
                "edge lookup differs for {name:?} / {module_type:?}",
            );
        }
    }
}

#[test]
#[ignore = "optional existing-owner timing comparison, no elapsed-time assertions"]
fn compare_builtin_query_with_former_reconstruction_and_existing_catalog() {
    use std::hint::black_box;
    use std::time::Instant;

    const ITERATIONS: usize = 512;
    let queries = [
        ("ActiveBody", ModuleType::Behavior),
        ("PhysicsBehavior", ModuleType::Behavior),
        ("SalvageCrateCollide", ModuleType::Behavior),
        ("NotRegistered", ModuleType::Behavior),
        ("ActiveBody", ModuleType::Draw),
        ("", ModuleType::ClientUpdate),
    ];
    let start = Instant::now();
    let mut former_checksum = 0u64;
    for _ in 0..ITERATIONS {
        for &(name, module_type) in &queries {
            let factory = black_box(ModuleFactory::new());
            let mask = factory.find_module_interface_mask(black_box(name), black_box(module_type));
            former_checksum = former_checksum.wrapping_add(black_box(mask.0) as u64);
        }
    }
    let former_elapsed = start.elapsed();

    let start = Instant::now();
    let mut query_checksum = 0u64;
    for _ in 0..ITERATIONS {
        for &(name, module_type) in &queries {
            let mask = ModuleFactory::find_builtin_module_interface_mask(
                black_box(name),
                black_box(module_type),
            );
            query_checksum = query_checksum.wrapping_add(black_box(mask.0) as u64);
        }
    }
    let query_elapsed = start.elapsed();

    // Construct this owner outside its timer: already registered known masks
    // use this branch in normal initialized lookup and need no fallback query.
    let factory = black_box(ModuleFactory::new());
    let start = Instant::now();
    let mut owner_checksum = 0u64;
    for _ in 0..ITERATIONS {
        for &(name, module_type) in &queries {
            let mask = factory.find_module_interface_mask(black_box(name), black_box(module_type));
            owner_checksum = owner_checksum.wrapping_add(black_box(mask.0) as u64);
        }
    }
    let owner_elapsed = start.elapsed();
    assert_eq!(query_checksum, former_checksum);
    assert_eq!(owner_checksum, former_checksum);
    eprintln!(
        "builtin mask comparison: lookups={} former_reconstruction={former_elapsed:?} immutable_query={query_elapsed:?} existing_catalog={owner_elapsed:?} checksum={former_checksum}",
        ITERATIONS * queries.len(),
    );
}

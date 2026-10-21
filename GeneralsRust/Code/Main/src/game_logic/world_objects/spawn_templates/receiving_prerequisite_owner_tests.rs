//! C++ Factory postProcessLoad/override resolution and template equivalence.
use super::*;
use game_engine::common::rts::science::{ScienceInfo, ScienceNameFacts, ScienceStore};
use std::sync::Arc;
fn body(name: &str, fields: &str) -> String {
    format!(
        "Object {name}\nKindOf = VEHICLE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\n{fields}\nEnd\n"
    )
}
fn science(name: &str) -> Arc<ScienceNameFacts> {
    let mut store = ScienceStore::new();
    store.add_science(ScienceInfo::new(701, name));
    Arc::new(ScienceNameFacts::from_store(&store))
}
fn receiving(source: &str, facts: Arc<ScienceNameFacts>) -> GameLogic {
    let source = crate::assets::ReceivingObjectContent::from_sources_with_sciences(
        [("owned-production.ini".to_owned(), source.to_owned())],
        facts,
    )
    .unwrap();
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(source).unwrap();
    world.add_player(Player::new(1, Team::USA, "ReceivingPlayer", true));
    world
}
fn satisfied(world: &GameLogic) -> bool {
    world.templates["Candidate"].prerequisites_satisfied(world, &world.players[&1])
}
#[test]
fn actual_receiving_host_admission_binds_prerequisite_handles_variations_and_reskin_before_factory_queries()
 {
    let first_source = [
        body("Candidate", "Prerequisites\nObject = FactoryA\nEnd"),
        body("FactoryA", "BuildVariations = FactoryVariant"),
        body("factoryvariant", ""),
        "ObjectReskin FactorySkin FactoryA\nEnd\n".to_owned(),
    ]
    .concat();
    let peer_source = [
        body("Candidate", "Prerequisites\nObject = ForeignFactory\nEnd"),
        body("ForeignFactory", ""),
    ]
    .concat();
    let mut first = receiving(&first_source, Arc::new(ScienceNameFacts::default()));
    let mut peer = receiving(&peer_source, Arc::new(ScienceNameFacts::default()));
    let a = first
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template("FactoryA")
        .unwrap();
    let b = peer
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template("ForeignFactory")
        .unwrap();
    assert_eq!(
        a.template_id(),
        b.template_id(),
        "same numeric IDs belong to different receiving catalogs"
    );
    let requirement =
        &first.templates["Candidate"].production_prerequisites[0].get_unit_prereqs()[0];
    assert!(
        requirement.name.is_empty(),
        "actual admission resolved the name like original postProcessLoad"
    );
    assert_eq!(requirement.unit.unwrap().value(), a.template_id() as u32);
    assert_eq!(
        first.templates["FactoryA"].build_variations,
        ["FactoryVariant"]
    );
    assert_eq!(
        first.templates["FactorySkin"].reskinned_from.as_deref(),
        Some("FactoryA")
    );
    assert!(!satisfied(&first));
    assert!(!satisfied(&peer));
    let own = first
        .create_object_for_player("factoryvariant", 1, Vec3::ZERO)
        .unwrap();
    let foreign = peer
        .create_object_for_player("ForeignFactory", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(own, foreign);
    assert_eq!(first.host_object(own).unwrap().health.current, 40.0);
    assert!(
        satisfied(&first),
        "only BuildVariations uses original compareNoCase"
    );
    assert!(satisfied(&peer));
    peer.reset();
    assert!(
        satisfied(&first),
        "foreign owner reset cannot rename a resolved handle"
    );
    let mut skin = receiving(&first_source, Arc::new(ScienceNameFacts::default()));
    skin.create_object_for_player("FactorySkin", 1, Vec3::ZERO)
        .unwrap();
    assert!(satisfied(&skin), "actual reskin equivalence retained");
}
#[test]
fn exact_receiving_prerequisite_identity_does_not_fold_case_without_variation_or_reskin() {
    let source = [
        body("Candidate", "Prerequisites\nObject = Foo\nEnd"),
        body("Foo", ""),
        body("foo", ""),
    ]
    .concat();
    let mut world = receiving(&source, Arc::new(ScienceNameFacts::default()));
    world
        .create_object_for_player("foo", 1, Vec3::ZERO)
        .unwrap();
    assert!(
        !satisfied(&world),
        "original distinct Foo and foo pointers are not equivalent"
    );
    world
        .create_object_for_player("Foo", 1, Vec3::ZERO)
        .unwrap();
    assert!(satisfied(&world));
}
#[test]
fn receiving_science_facts_and_failed_map_admission_keep_own_names_and_atomic_requirements() {
    let mut first = receiving(
        &body("Candidate", "Prerequisites\nScience = SCIENCE_First\nEnd"),
        science("SCIENCE_First"),
    );
    let mut peer = receiving(
        &body("Candidate", "Prerequisites\nScience = SCIENCE_Peer\nEnd"),
        science("SCIENCE_Peer"),
    );
    first
        .players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_Peer".into());
    peer.players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_Peer".into());
    assert!(!satisfied(&first));
    assert!(satisfied(&peer));
    first
        .players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_First".into());
    assert!(satisfied(&first));
    let before = first
        .receiving_module_content
        .as_ref()
        .unwrap()
        .admitted_template("Candidate")
        .unwrap();
    let error = first
        .apply_receiving_module_overrides(&[
            "Object Candidate\nScale = 9\nPrerequisites\nScience = SCIENCE_Unknown\nEnd\nEnd"
                .into(),
        ])
        .unwrap_err();
    assert!(
        error.contains("Unknown receiving science prerequisite"),
        "{error}"
    );
    assert!(Arc::ptr_eq(
        &before,
        &first
            .receiving_module_content
            .as_ref()
            .unwrap()
            .admitted_template("Candidate")
            .unwrap()
    ));
    assert_eq!(first.templates["Candidate"].asset_scale, 1.0);
    assert!(satisfied(&first));
    assert!(satisfied(&peer));
}

#[test]
fn explicit_compatibility_definition_builder_keeps_captured_prerequisite_adapter() {
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            &body(
                "StandaloneExplicitPrerequisiteProbe",
                "Prerequisites\nObject = StandaloneRequiredProbe\nEnd",
            ),
            "standalone.ini",
        )
        .unwrap();
    let template = GameLogic::build_template_from_object_definition(
        "StandaloneExplicitPrerequisiteProbe",
        parser
            .get_definition("StandaloneExplicitPrerequisiteProbe")
            .unwrap(),
        None,
    );
    let prerequisites = &template.production_prerequisites;
    assert_eq!(prerequisites.len(), 1);
    assert_eq!(
        prerequisites[0].get_unit_prereqs()[0].name,
        "StandaloneRequiredProbe"
    );
}

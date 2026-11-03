//! hq-wq7g6y: build-facility template name census is cached per world.
use generals_main::game_logic::{GameLogic, KindOf, ThingTemplate};

fn main() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();

    let mut cc = ThingTemplate::new("CacheCC");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter);
    a.templates.insert("CacheCC".into(), cc.clone());
    a.invalidate_build_facility_template_names_cache();
    b.templates.insert("CacheCC".into(), cc);
    b.invalidate_build_facility_template_names_cache();

    let first_a = a.build_facility_template_names();
    assert!(
        first_a.contains("cachecc"),
        "CommandCenter template must appear in facility census"
    );
    let second_a = a.build_facility_template_names();
    assert_eq!(first_a, second_a, "cached census must be stable");

    let first_b = b.build_facility_template_names();
    assert!(first_b.contains("cachecc"));

    a.templates.clear();
    a.invalidate_build_facility_template_names_cache();
    let after = a.build_facility_template_names();
    assert!(
        !after.contains("cachecc"),
        "cleared templates must invalidate the facility census"
    );
    assert!(
        b.build_facility_template_names().contains("cachecc"),
        "peer world cache must stay independent"
    );
    println!("facility_cache_gate: PASS build_facility_template_names_cache_is_per_world_and_invalidates");
}

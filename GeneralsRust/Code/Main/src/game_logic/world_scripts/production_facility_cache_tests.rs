//! hq-wq7g6y: build-facility name set is cached per world and invalidated on template mutation.
use crate::game_logic::{GameLogic, KindOf, ThingTemplate};

#[test]
fn build_facility_template_names_cache_is_per_world_and_invalidates() {
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
    assert!(first_a.contains("cachecc"));
    // Second call must reuse the cached set (pointer-stable via equal contents).
    let second_a = a.build_facility_template_names();
    assert_eq!(first_a, second_a);

    // Peer world keeps an independent cache.
    let first_b = b.build_facility_template_names();
    assert!(first_b.contains("cachecc"));

    // Mutation + invalidate must drop "cachecc" from A's census.
    a.templates.clear();
    a.invalidate_build_facility_template_names_cache();
    let after = a.build_facility_template_names();
    assert!(
        !after.contains("cachecc"),
        "cleared templates must invalidate the facility census"
    );
    // B unchanged.
    assert!(b.build_facility_template_names().contains("cachecc"));
}

use generals_main::ui::construction_panel::PlacementPreview;

#[test]
fn same_named_templates_keep_each_sessions_admitted_preview_facts() {
    let mut first_session = PlacementPreview::default();
    first_session.start("SameName", "SameName", 0, 0.25, (12.0, 7.0));

    let mut second_session = PlacementPreview::default();
    second_session.start("SameName", "SameName", 0, 1.5, (31.0, 18.0));

    assert_eq!(first_session.facing_radians, 0.25);
    assert_eq!(first_session.footprint_half_extents, (12.0, 7.0));
    assert_eq!(second_session.facing_radians, 1.5);
    assert_eq!(second_session.footprint_half_extents, (31.0, 18.0));

    first_session.set_facing(0.75);
    assert_eq!(first_session.facing_radians, 0.75);
    assert_eq!(second_session.facing_radians, 1.5);

    // Re-arming a placement resets to the new admitted template's angle and
    // geometry while preserving the existing preview's drag/update behavior.
    first_session.start("SameName", "SameName", 0, -0.5, (9.0, 4.0));
    assert_eq!(first_session.facing_radians, -0.5);
    assert_eq!(first_session.footprint_half_extents, (9.0, 4.0));
}

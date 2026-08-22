use gamelogic::object::contain::InitialRoster;

#[test]
fn initial_roster_parse_from_tokens_defaults_count_to_one() {
    let roster = InitialRoster::parse_from_tokens(&["GLAInfantryRebel"]).expect("roster");
    assert_eq!(roster.template_name, "GLAInfantryRebel");
    assert_eq!(roster.count, 1);
    assert!(roster.is_populated());
}

#[test]
fn initial_roster_parse_from_tokens_reads_count() {
    let roster = InitialRoster::parse_from_tokens(&["GLAInfantryRebel", "3"]).expect("roster");
    assert_eq!(roster.template_name, "GLAInfantryRebel");
    assert_eq!(roster.count, 3);
}

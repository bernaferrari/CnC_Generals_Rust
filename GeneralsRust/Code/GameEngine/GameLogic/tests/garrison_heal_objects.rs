use gamelogic::object::contain::garrison_heal_single_amount;

#[test]
fn garrison_heal_single_amount_slivers_then_snaps() {
    assert!((garrison_heal_single_amount(80.0, 0, 60.0) - (80.0 / 60.0)).abs() < 0.0001);
    assert!((garrison_heal_single_amount(80.0, 60, 60.0) - 80.0).abs() < 0.0001);
    assert_eq!(garrison_heal_single_amount(80.0, 0, 0.0), 0.0);
}

use super::*;
use crate::game_logic::host_upgrade_rules::register_test_upgrade;

#[test]
fn command_upgrade_cost_and_time_use_driving_world() {
    let name = "Upgrade_CommandWorldRules";
    let mut a = GameLogic::new();
    register_test_upgrade(&a, name, "PLAYER", 321, 7);
    let mut b = GameLogic::new();
    register_test_upgrade(&b, name, "OBJECT", 654, 19);
    for _ in 0..3 {
        let executor = CommandExecutor::new(&mut a, 1);
        assert_eq!(executor.resolve_upgrade_cost_supplies(name), 321);
        assert_eq!(executor.resolve_upgrade_build_time_secs(name), 7.0);
        let executor = CommandExecutor::new(&mut b, 1);
        assert_eq!(executor.resolve_upgrade_cost_supplies(name), 654);
        assert_eq!(executor.resolve_upgrade_build_time_secs(name), 19.0);
    }
    b.reset();
    let executor = CommandExecutor::new(&mut a, 1);
    assert_eq!(executor.resolve_upgrade_cost_supplies(name), 321);
    assert_eq!(executor.resolve_upgrade_build_time_secs(name), 7.0);
}

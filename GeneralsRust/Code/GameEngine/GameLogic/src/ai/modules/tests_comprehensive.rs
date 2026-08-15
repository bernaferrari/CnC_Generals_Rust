//! Comprehensive AI module tests.
//!
//! These focus on lightweight invariants that should remain stable across parity work.

use super::{AIModulePriority, AIModuleState, AIModuleType, AIUpdateModule, AIUpdateModuleTrait};

#[test]
fn tests_comprehensive_smoke() {
    let mut module = AIUpdateModule::new(AIModuleType::Dozer, AIModulePriority::High);
    assert!(module.is_enabled());
    assert_eq!(module.get_module_type(), AIModuleType::Dozer);
    assert_eq!(module.get_priority(), AIModulePriority::High);
    assert_eq!(module.get_state(), AIModuleState::Idle);
    module.set_state(AIModuleState::Active);
    assert_eq!(module.get_state(), AIModuleState::Active);
    module.set_enabled(false);
    assert!(!module.is_enabled());
}

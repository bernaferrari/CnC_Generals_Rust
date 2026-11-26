//! Mask conditions shared by upgrade modules.

/// Masks resolved from the receiving world's upgrade catalog.
#[derive(Debug, Clone, Copy)]
pub struct UpgradeConditions {
    pub activation: u128,
    pub conflicts: u128,
    pub requires_all_triggers: bool,
}

impl UpgradeConditions {
    /// C++ UpgradeMux::wouldUpgrade requires an activation condition.
    /// The caller owns the module's execution flag.
    pub fn can_trigger(self, key_mask: u128) -> bool {
        self.activation != 0 && key_mask != 0 && self.is_satisfied(key_mask)
    }

    /// C++ UpgradeMux::testUpgradeConditions also allows conflict-only conditions.
    pub fn is_satisfied(self, key_mask: u128) -> bool {
        key_mask & self.conflicts == 0
            && (self.activation == 0
                || if self.requires_all_triggers {
                    key_mask & self.activation == self.activation
                } else {
                    key_mask & self.activation != 0
                })
    }
}

#[cfg(test)]
mod tests {
    use super::UpgradeConditions;

    #[test]
    fn conflict_only_conditions_pass_without_triggering_a_module() {
        let conflict = 1 << 127;
        let conditions = UpgradeConditions {
            activation: 0,
            conflicts: conflict,
            requires_all_triggers: true,
        };
        assert!(conditions.is_satisfied(0));
        assert!(conditions.is_satisfied(1));
        assert!(!conditions.is_satisfied(conflict));
        assert!(!conditions.can_trigger(1));
    }
}

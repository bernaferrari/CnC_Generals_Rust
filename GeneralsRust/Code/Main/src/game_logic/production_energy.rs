//! C++ GlobalData.cpp:826-828,1138-1195 — baseline and map overrides.
use game_engine::common::ini::ini_game_data::ProductionEnergyRules;
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProductionEnergyDefinitions {
    pub(crate) base: ProductionEnergyRules,
    pub(crate) effective: ProductionEnergyRules,
}
impl super::GameLogic {
    pub(crate) fn admit_production_energy_base(&mut self, rules: ProductionEnergyRules) {
        self.production_energy = ProductionEnergyDefinitions {
            base: rules,
            effective: rules,
        };
    }
    pub(crate) fn production_energy_rules(&self) -> &ProductionEnergyRules {
        &self.production_energy.effective
    }
    pub(crate) fn production_energy_baseline(&self) -> ProductionEnergyRules {
        self.production_energy.base
    }
}

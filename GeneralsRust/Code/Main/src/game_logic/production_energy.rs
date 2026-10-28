//! Owned GameData construction/production definitions: baseline and map overrides.
//! CPP GlobalData.cpp826–835; BuildAssistant.cpp444,1144–1156.
//! One runtime projection travels through native admission, reset and saved-map loading.
use game_engine::common::ini::ini_game_data::{PowerBarRules, ProductionEnergyRules};
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ProductionEnergyDefinitions {
    pub(crate) base: ProductionEnergyRules,
    pub(crate) effective: ProductionEnergyRules,
}

/// Presentation parameters follow the same session/map lifetime as power.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PowerBarDefinitions {
    pub(crate) base: PowerBarRules,
    pub(crate) effective: PowerBarRules,
}
impl super::GameLogic {
    pub(crate) fn admit_power_bar_base(&mut self, rules: PowerBarRules) {
        self.power_bar_definitions = PowerBarDefinitions {
            base: rules,
            effective: rules,
        };
    }
    /// Effective GameData/Map.ini/Solo.ini parameters for this world's UI.
    pub fn power_bar_rules(&self) -> PowerBarRules {
        self.power_bar_definitions.effective
    }
    pub(crate) fn power_bar_baseline(&self) -> PowerBarRules {
        self.power_bar_definitions.base
    }
    /// C++ clamps the tiled count but always retains its first tile. Keep the
    /// signed authored value intact; the tiler owns that final count policy.
    pub(crate) fn line_build_object_limit(&self) -> i32 {
        self.production_energy.effective.max_line_build_objects
    }

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

use super::*;

/// Player energy/power management (matching C++ Energy class)
#[derive(Debug, Clone)]
pub struct PlayerEnergy {
    pub(super) production: Int,
    pub(super) consumption: Int,
    pub(super) power_sabotaged_till_frame: UnsignedInt,
}

impl PlayerEnergy {
    pub fn new() -> Self {
        Self {
            production: 0,
            consumption: 0,
            power_sabotaged_till_frame: 0,
        }
    }

    /// Reset energy bookkeeping to defaults (matches C++ Energy::init).
    pub fn reset(&mut self) {
        self.production = 0;
        self.consumption = 0;
    }

    pub fn get_power(&self) -> Int {
        self.production() - self.consumption
    }

    pub fn is_low_power(&self) -> bool {
        !self.has_sufficient_power()
    }

    pub fn production(&self) -> Int {
        if TheGameLogic::get_frame() < self.power_sabotaged_till_frame {
            0
        } else {
            self.production
        }
    }

    pub fn consumption(&self) -> Int {
        self.consumption
    }

    pub fn supply_ratio(&self) -> Real {
        if TheGameLogic::get_frame() < self.power_sabotaged_till_frame {
            return 0.0;
        }

        if self.consumption <= 0 {
            return self.production() as Real;
        }

        (self.production() as Real) / (self.consumption as Real)
    }

    pub fn add_power_production(&mut self, amount: Int) {
        self.production += amount;
        debug_assert!(
            self.production >= 0 && self.consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.production,
            self.consumption
        );
    }

    pub fn add_power_consumption(&mut self, amount: Int) {
        self.consumption += amount;
        debug_assert!(
            self.production >= 0 && self.consumption >= 0,
            "Energy - Negative Energy numbers, Produce={} Consume={}",
            self.production,
            self.consumption
        );
    }

    /// Adjust power based on a delta and whether we're adding/removing (matches C++ Energy::adjustPower).
    pub fn adjust_power(&mut self, power_delta: Int, adding: Bool) {
        if power_delta == 0 {
            return;
        }

        if power_delta > 0 {
            if adding {
                self.add_power_production(power_delta);
            } else {
                self.add_power_production(-power_delta);
            }
        } else if adding {
            self.add_power_consumption(-power_delta);
        } else {
            self.add_power_consumption(power_delta);
        }
    }

    /// Register a newly influenced object to adjust production/consumption (matches C++ Energy::objectEnteringInfluence).
    pub fn object_entering_influence(&mut self, obj: &Object) {
        let energy = obj.get_template().get_energy_production();
        if energy < 0 {
            self.add_power_consumption(-energy);
        } else if energy > 0 {
            self.add_power_production(energy);
        }
    }

    /// Remove influence from an object (matches C++ Energy::objectLeavingInfluence).
    pub fn object_leaving_influence(&mut self, obj: &Object) {
        let energy = obj.get_template().get_energy_production();
        if energy < 0 {
            self.add_power_consumption(energy);
        } else if energy > 0 {
            self.add_power_production(-energy);
        }
    }

    pub fn add_power_bonus(&mut self, obj: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(bonus) = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj, |object_guard| {
                object_guard.get_template().get_energy_bonus()
            })
        {
            if bonus != 0 {
                self.add_power_production(bonus);
            }
        }
        self.touch();
    }

    pub fn remove_power_bonus(&mut self, obj: ObjectID) {
        // Wave 268: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(bonus) = crate::object::registry::OBJECT_REGISTRY
            .with_object(obj, |object_guard| {
                object_guard.get_template().get_energy_bonus()
            })
        {
            if bonus != 0 {
                self.add_power_production(-bonus);
            }
        }
    }

    pub fn touch(&mut self) {}

    /// Set sabotage timer for the player's power supply
    /// Matches C++ Energy::setPowerSabotagedTillFrame
    pub fn set_power_sabotaged_till_frame(&mut self, frame: UnsignedInt) {
        self.power_sabotaged_till_frame = frame;
    }

    pub fn get_power_sabotaged_till_frame(&self) -> UnsignedInt {
        self.power_sabotaged_till_frame
    }

    pub fn is_power_sabotaged(&self) -> bool {
        TheGameLogic::get_frame() < self.power_sabotaged_till_frame
    }

    pub fn has_sufficient_power(&self) -> bool {
        if self.is_power_sabotaged() {
            false
        } else {
            self.production >= self.consumption
        }
    }
}

/// Resource snapshot exposed to high-level managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerResources {
    pub supplies: Int,
    pub power_available: Int,
    pub power_used: Int,
}

/// Player handicap system (matching C++ Handicap class)
#[derive(Debug, Clone)]
pub struct PlayerHandicap {
    pub(super) damage_multiplier: Real,
    pub(super) cost_multiplier: Real,
    pub(super) build_time_multiplier: Real,
    pub(super) vision_multiplier: Real,
    pub(super) build_cost_generic: Real,
    pub(super) build_cost_buildings: Real,
    pub(super) build_time_generic: Real,
    pub(super) build_time_buildings: Real,
}

impl PlayerHandicap {
    pub fn new() -> Self {
        Self {
            damage_multiplier: 1.0,
            cost_multiplier: 1.0,
            build_time_multiplier: 1.0,
            vision_multiplier: 1.0,
            build_cost_generic: 1.0,
            build_cost_buildings: 1.0,
            build_time_generic: 1.0,
            build_time_buildings: 1.0,
        }
    }

    pub fn get_damage_multiplier(&self) -> Real {
        self.damage_multiplier
    }

    pub fn get_cost_multiplier(&self) -> Real {
        self.cost_multiplier
    }

    pub fn get_build_time_multiplier(&self) -> Real {
        self.build_time_multiplier
    }

    pub fn get_vision_multiplier(&self) -> Real {
        self.vision_multiplier
    }

    pub fn set_all(&mut self, value: Real) {
        let value = value.max(0.0);
        self.damage_multiplier = value;
        self.cost_multiplier = value;
        self.build_time_multiplier = value;
        self.vision_multiplier = value;
        self.build_cost_generic = value;
        self.build_cost_buildings = value;
        self.build_time_generic = value;
        self.build_time_buildings = value;
    }

    pub fn read_from_dict(&mut self, dict: &crate::common::Dict) {
        let keys = [
            ("HANDICAP_BUILDCOST_GENERIC", true, true),
            ("HANDICAP_BUILDCOST_BUILDINGS", true, false),
            ("HANDICAP_BUILDTIME_GENERIC", false, true),
            ("HANDICAP_BUILDTIME_BUILDINGS", false, false),
        ];

        for (name, is_cost, is_generic) in keys {
            let key = NameKeyGenerator::name_to_key(name);
            if dict.get_type(key).is_some() {
                let value = dict.get_real(key);
                if is_cost {
                    if is_generic {
                        self.build_cost_generic = value;
                    } else {
                        self.build_cost_buildings = value;
                    }
                } else if is_generic {
                    self.build_time_generic = value;
                } else {
                    self.build_time_buildings = value;
                }
            }
        }

        self.cost_multiplier = self.build_cost_generic;
        self.build_time_multiplier = self.build_time_generic;
    }

    pub fn get_cost_multiplier_for_template<T: ThingTemplate + ?Sized>(
        &self,
        template: &T,
    ) -> Real {
        if template.is_kind_of(KindOf::Structure) {
            self.build_cost_buildings
        } else {
            self.build_cost_generic
        }
    }

    pub fn get_build_time_multiplier_for_template<T: ThingTemplate + ?Sized>(
        &self,
        template: &T,
    ) -> Real {
        if template.is_kind_of(KindOf::Structure) {
            self.build_time_buildings
        } else {
            self.build_time_generic
        }
    }
}

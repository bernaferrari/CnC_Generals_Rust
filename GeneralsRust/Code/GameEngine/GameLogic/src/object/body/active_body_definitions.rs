//! Effective immutable armor/FX/rules for the constructing session's body.
use super::{Arc, ArmorTemplate, DamageFX};
use generals_sim::armor::ArmorDefinitions;
use std::collections::HashMap;

pub struct BodyDefinitionCatalog {
    thresholds: Option<(f32, f32)>,
    armor: ArmorDefinitions,
    damage_fx: HashMap<String, Arc<DamageFX>>,
}
impl BodyDefinitionCatalog {
    pub fn new(
        rules: Option<&game_engine::common::global_data::GlobalData>,
        armor: HashMap<String, Arc<ArmorTemplate>>,
        damage_fx: HashMap<String, Arc<DamageFX>>,
    ) -> Result<Self, String> {
        let thresholds =
            rules.map(|rules| (rules.unit_damaged_thresh, rules.unit_really_damaged_thresh));
        if thresholds.is_some_and(|(damaged, really)| !damaged.is_finite() || !really.is_finite()) {
            return Err("non-finite receiving body damage thresholds".into());
        }
        Ok(Self {
            thresholds,
            armor: ArmorDefinitions::from_exact_templates(armor),
            damage_fx,
        })
    }
    pub(super) fn thresholds(&self) -> Option<(f32, f32)> {
        self.thresholds
    }
    pub(super) fn armor(&self, name: &str) -> Option<Arc<ArmorTemplate>> {
        self.armor.shared_template_exact(name)
    }
    pub(super) fn damage_fx(&self, name: &str) -> Option<&DamageFX> {
        self.damage_fx.get(name).map(Arc::as_ref)
    }
}

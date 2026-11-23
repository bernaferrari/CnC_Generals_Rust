use std::sync::Arc;

use crate::common::{KindOfMaskType, ObjectID, Real, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{
    UpgradeMuxData, mux_can_upgrade,
};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::kind_of::KindOfMask;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, ModuleDataView, NameKeyType};

/// Module data describing the cost modifier upgrade.
#[derive(Debug, Clone)]
pub struct CostModifierUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    kind_of: KindOfMaskType,
    percentage: Real,
}

impl Default for CostModifierUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            kind_of: crate::common::KIND_OF_MASK_NONE,
            percentage: 0.0,
        }
    }
}

impl CostModifierUpgradeModuleData {
    pub fn kind_of(&self) -> KindOfMaskType {
        self.kind_of
    }

    pub fn percentage(&self) -> Real {
        self.percentage
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COST_MODIFIER_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(CostModifierUpgradeModuleData, module_tag_name_key);

impl Snapshotable for CostModifierUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Upgrade module that modifies production costs for buildings/factories.
pub struct CostModifierUpgrade {
    module_name_key: NameKeyType,
    data: game_engine::common::thing::module::ModuleDataHandle,
    applied: bool,
}

impl CostModifierUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: game_engine::common::thing::module::ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            applied: false,
        }
    }

    pub(crate) fn prepare_upgrade(
        &self,
        upgrade_mask: UpgradeMaskType,
    ) -> Option<ModuleDataView<CostModifierUpgradeModuleData>> {
        let can_upgrade = {
            let data = self.data.read();
            let data = data
                .as_any()
                .downcast_ref::<CostModifierUpgradeModuleData>()
                .expect("CostModifierUpgradeModuleData");
            mux_can_upgrade(&data.upgrade_mux_data, self.applied, upgrade_mask)
        };
        can_upgrade.then(|| {
            ModuleDataView::new(self.data.clone(), |data| {
                (data as &dyn std::any::Any)
                    .downcast_ref::<CostModifierUpgradeModuleData>()
                    .expect("CostModifierUpgradeModuleData")
            })
        })
    }

    pub(crate) fn finish_upgrade(&mut self) {
        self.applied = true;
    }
}

impl CostModifierUpgradeModuleData {
    /// C++ CostModifierUpgrade::upgradeImplementation uses the Thing that
    /// owns this module; the live Object caller passes that owner explicitly.
    pub(crate) fn apply_to_object(&self, owner: &mut crate::object::Object) {
        owner.with_controlling_player_mut(|player| {
            player.add_kind_of_production_cost_change(self.kind_of, self.percentage);
        });
    }
}

impl Module for CostModifierUpgrade {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.data)
    }
}

impl Snapshotable for CostModifierUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            std::any::type_name::<Self>(),
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for CostModifierUpgrade {
    fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().as_any().downcast_ref::<CostModifierUpgradeModuleData>().expect("CostModifierUpgradeModuleData").upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ does not remove the cost change; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.read().as_any().downcast_ref::<CostModifierUpgradeModuleData>().expect("CostModifierUpgradeModuleData").upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }

    fn on_delete(&mut self, object: &mut crate::object::Object) {
        if !self.applied {
            return;
        }

        let (kind, percentage) = {
            let data = self.data.read();
            let data = data.as_any().downcast_ref::<CostModifierUpgradeModuleData>().expect("CostModifierUpgradeModuleData");
            (data.kind_of(), data.percentage())
        };
        object.with_controlling_player_mut(|player_guard| {
            player_guard.remove_kind_of_production_cost_change(
                kind,
                percentage,
            );
        });

        self.applied = false;
    }

    fn on_capture(
        &mut self,
        _object: &mut crate::object::Object,
        old_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
    ) {
        if !self.applied {
            return;
        }
        let (kind, percentage) = {
            let data = self.data.read();
            let data = data.as_any().downcast_ref::<CostModifierUpgradeModuleData>().expect("CostModifierUpgradeModuleData");
            (data.kind_of(), data.percentage())
        };

        if let Some(old_owner) = old_owner {
            if let Ok(mut player_guard) = old_owner.write() {
                player_guard.remove_kind_of_production_cost_change(
                    kind,
                    percentage,
                );
                self.applied = false;
            }
        }

        if let Some(new_owner) = new_owner {
            if let Ok(mut player_guard) = new_owner.write() {
                player_guard.add_kind_of_production_cost_change(
                    kind,
                    percentage,
                );
                self.applied = true;
            }
        }
    }
}

fn parse_effect_kind_of_field(
    _ini: &mut INI,
    data: &mut CostModifierUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let previous = KindOfMask::from_bits_retain(data.kind_of);
    data.kind_of = KindOfMask::parse_ini(previous, &tokens.join(" "))
        .map_err(|_| INIError::InvalidNameList)?
        .bits();
    Ok(())
}

fn parse_percentage_field(
    _ini: &mut INI,
    data: &mut CostModifierUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.percentage = INI::parse_percent_to_real(tokens[0])?;
    Ok(())
}

crate::impl_upgrade_mux_field_parsers!(CostModifierUpgradeModuleData);

const COST_MODIFIER_UPGRADE_FIELDS: &[FieldParse<CostModifierUpgradeModuleData>] = crate::upgrade_mux_field_table!(
    FieldParse {
        token: "EffectKindOf",
        parse: parse_effect_kind_of_field,
    },
    FieldParse {
        token: "Percentage",
        parse: parse_percentage_field,
    },
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_modifier_effect_kind_of_uses_the_full_ini_bit_list() {
        let mut data = CostModifierUpgradeModuleData::default();
        let mut ini = INI::new();
        ini.with_inline_source(
            "EffectKindOf = VEHICLE NO_HEAL_ICON\nEffectKindOf = -VEHICLE +AIRCRAFT\nEnd\n",
            |ini| data.parse_from_ini(ini),
        )
        .unwrap();
        assert_eq!(
            data.kind_of(),
            (KindOfMask::AIRCRAFT | KindOfMask::NO_HEAL_ICON).bits()
        );

        let result = ini.with_inline_source("EffectKindOf = NO_SUCH_KIND\nEnd\n", |ini| {
            data.parse_from_ini(ini)
        });
        assert_eq!(result, Err(INIError::InvalidNameList));
    }

    #[test]
    fn cost_modifier_rows_require_all_bits_and_keep_exact_authored_percentages() {
        let mut player = crate::player::Player::new(0);
        let vehicle = KindOfMask::VEHICLE.bits();
        let filter = (KindOfMask::VEHICLE | KindOfMask::AIRCRAFT).bits();
        let discount = -0.25_f32;
        let adjacent = f32::from_bits(discount.to_bits() - 1);
        player.add_kind_of_production_cost_change(filter, discount);
        player.add_kind_of_production_cost_change(filter, discount);
        player.add_kind_of_production_cost_change(filter, adjacent);

        assert_eq!(
            player.kind_of_production_change_entries(),
            [(filter, discount, 2), (filter, adjacent, 1)]
        );
        assert_eq!(player.get_production_cost_change_based_on_kind_of(vehicle), 1.0);
        assert_eq!(
            player.get_production_cost_change_based_on_kind_of(filter),
            (1.0 + discount) * (1.0 + adjacent)
        );
        player.remove_kind_of_production_cost_change(filter, discount);
        assert_eq!(player.kind_of_production_change_entries()[0], (filter, discount, 1));
        player.remove_kind_of_production_cost_change(filter, discount);
        assert_eq!(player.kind_of_production_change_entries(), [(filter, adjacent, 1)]);

        player.add_kind_of_production_cost_change(KindOfMask::empty().bits(), discount);
        assert_eq!(player.get_production_cost_change_based_on_kind_of(vehicle), 1.0 + discount);
    }
}

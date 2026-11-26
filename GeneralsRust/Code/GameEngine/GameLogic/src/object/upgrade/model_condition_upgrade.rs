use crate::common::{ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{mux_can_upgrade, mux_give_self_upgrade_for_object};
pub use game_engine::common::system::upgrade::ModelConditionUpgradeModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, NameKeyType};

/// Wave 448: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Upgrade module that changes model conditions (visual state) on the owning object.
pub struct ModelConditionUpgrade {
    module_name_key: NameKeyType,
    data: game_engine::common::thing::module::ModuleDataHandle,
    object_id: ObjectID,
    applied: bool,
}

impl ModelConditionUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: game_engine::common::thing::module::ModuleDataHandle,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }
}

impl Module for ModelConditionUpgrade {
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

impl Snapshotable for ModelConditionUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
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

impl UpgradeModuleInterface for ModelConditionUpgrade {
    fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().as_any().downcast_ref::<ModelConditionUpgradeModuleData>().expect("ModelConditionUpgradeModuleData").upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        // Wave 448: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.applied {
            return false;
        }
        { let data = self.data.read().as_any().downcast_ref::<ModelConditionUpgradeModuleData>().expect("ModelConditionUpgradeModuleData").upgrade_mux_data.clone(); mux_give_self_upgrade_for_object(&data, self.object_id) };
        use crate::object::registry::OBJECT_REGISTRY;

        let flag = self.data.read().as_any().downcast_ref::<ModelConditionUpgradeModuleData>().expect("ModelConditionUpgradeModuleData").condition_flag();
        let Some(()) = OBJECT_REGISTRY.with_object_mut(self.object_id, |object_guard| {
            if !flag.is_empty() {
                let _ = object_guard.set_model_condition_flags(flag);
            }
        }) else {
            log::warn!("ModelConditionUpgrade: Object {} not found", self.object_id);
            return false;
        };

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ does not clear model condition flags; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.read().as_any().downcast_ref::<ModelConditionUpgradeModuleData>().expect("ModelConditionUpgradeModuleData").upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ModelConditionFlags;
    use game_engine::common::ini::INI;

    fn parse_model_condition_flag(name: &str) -> Option<ModelConditionFlags> {
        let mut data = ModelConditionUpgradeModuleData::default();
        INI::new()
            .with_inline_source(&format!("ConditionFlag = {name}\nEnd\n"), |ini| {
                data.parse_from_ini(ini)
            })
            .ok()?;
        Some(data.condition_flag())
    }

    #[test]
    fn crush_flags_are_distinct_from_toppled_and_flooded() {
        let front = parse_model_condition_flag("FRONTCRUSHED").expect("FRONTCRUSHED");
        let back = parse_model_condition_flag("BACKCRUSHED").expect("BACKCRUSHED");
        let toppled = parse_model_condition_flag("TOPPLED").expect("TOPPLED");
        let flooded = parse_model_condition_flag("FLOODED").expect("FLOODED");
        assert_eq!(front, ModelConditionFlags::FRONTCRUSHED);
        assert_eq!(back, ModelConditionFlags::BACKCRUSHED);
        assert_ne!(front, toppled);
        assert_ne!(back, flooded);
    }

    #[test]
    fn missing_cpp_condition_flags_parse() {
        assert_eq!(
            parse_model_condition_flag("PREATTACK_A"),
            Some(ModelConditionFlags::PREATTACK_A)
        );
        assert_eq!(
            parse_model_condition_flag("USING_WEAPON_B"),
            Some(ModelConditionFlags::USING_WEAPON_B)
        );
        assert_eq!(
            parse_model_condition_flag("DOOR_4_WAITING_TO_CLOSE"),
            Some(ModelConditionFlags::DOOR_4_WAITING_TO_CLOSE)
        );
        assert_eq!(
            parse_model_condition_flag("RIDER3"),
            Some(ModelConditionFlags::RIDER3)
        );
        assert_eq!(
            parse_model_condition_flag("TURRET_ROTATE"),
            Some(ModelConditionFlags::TURRET_ROTATE)
        );
        assert_eq!(
            parse_model_condition_flag("CONTINUOUS_FIRE_FAST"),
            Some(ModelConditionFlags::CONTINUOUS_FIRE_FAST)
        );
    }
}

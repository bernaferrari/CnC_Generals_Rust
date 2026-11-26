use crate::common::{ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{mux_can_upgrade, mux_give_self_upgrade_for_object};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, NameKeyType};

pub use game_engine::common::system::upgrade::CommandSetUpgradeModuleData;

/// Upgrade module that changes available commands on the owning object.
pub struct CommandSetUpgrade {
    module_name_key: NameKeyType,
    data: game_engine::common::thing::module::ModuleDataHandle,
    object_id: ObjectID,
    applied: bool,
}

impl CommandSetUpgrade {
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

impl Module for CommandSetUpgrade {
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

impl Snapshotable for CommandSetUpgrade {
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

impl UpgradeModuleInterface for CommandSetUpgrade {
    fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().as_any().downcast_ref::<CommandSetUpgradeModuleData>().expect("CommandSetUpgradeModuleData").upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        { let data = self.data.read().as_any().downcast_ref::<CommandSetUpgradeModuleData>().expect("CommandSetUpgradeModuleData").upgrade_mux_data.clone(); mux_give_self_upgrade_for_object(&data, self.object_id) };
        use crate::helpers::TheGameLogic;
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::upgrade::center::with_upgrade_center;

        let trigger_alt = self.data.read().as_any().downcast_ref::<CommandSetUpgradeModuleData>().expect("CommandSetUpgradeModuleData").trigger_alt().clone();
        let command_set_alt = self.data.read().as_any().downcast_ref::<CommandSetUpgradeModuleData>().expect("CommandSetUpgradeModuleData").command_set_alt().clone();
        let command_set_name = self.data.read().as_any().downcast_ref::<CommandSetUpgradeModuleData>().expect("CommandSetUpgradeModuleData").command_set_name().clone();

        let apply_to = |object_guard: &mut crate::object::Object| {
            let mut use_alt = false;
            if !trigger_alt.is_empty() {
                let upgrade =
                    with_upgrade_center(|center| center.find_upgrade(trigger_alt.as_str()));
                if let Some(template) = upgrade {
                    let mask_bits = UpgradeMaskType::from_bits_retain(template.mask().bits());

                    if object_guard
                        .with_controlling_player(|player_guard| {
                            player_guard
                                .get_completed_upgrade_mask()
                                .intersects(mask_bits)
                        })
                        == Some(true)
                    {
                        use_alt = true;
                    }

                    if !use_alt && object_guard.completed_upgrades().intersects(mask_bits) {
                        use_alt = true;
                    }
                }
            }

            if use_alt {
                object_guard.set_command_set_string_override(&command_set_alt);
            } else {
                object_guard.set_command_set_string_override(&command_set_name);
            }
            use_alt
        };

        let found = OBJECT_REGISTRY
            .with_object_mut(self.object_id, |object_guard| apply_to(object_guard))
            .or_else(|| {
                TheGameLogic::find_object_by_id(self.object_id)
                    .and_then(|obj| obj.write().ok().map(|mut guard| apply_to(&mut guard)))
            });
        if found.is_none() {
            log::warn!("CommandSetUpgrade: Object {} not found", self.object_id);
            return false;
        }
        let world = OBJECT_REGISTRY
            .with_object(self.object_id, |guard| guard.owning_world_services())
            .flatten();
        crate::control_bar::mark_ui_dirty_from(world.as_ref());

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ leaves the override; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.read().as_any().downcast_ref::<CommandSetUpgradeModuleData>().expect("CommandSetUpgradeModuleData").upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}

use std::any::Any;

use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{
    ActiveShroudUpgradeConfig, ModuleData, NameKeyType, RadarUpdateConfig, RadarUpgradeConfig,
};
use game_engine::thing::StaticGameLodLevel;

/// Legacy module-data bridge that mirrors the WW3D expectations.
pub trait LegacyModuleData: Snapshotable + Clone + Send + Sync + std::fmt::Debug + Any {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType);
    fn get_module_tag_name_key(&self) -> NameKeyType;

    fn is_ai_module_data(&self) -> bool {
        false
    }

    fn get_as_w3d_model_draw_module_data(&self) -> Option<&dyn Any> {
        None
    }

    fn get_as_w3d_tree_draw_module_data(&self) -> Option<&dyn Any> {
        None
    }

    fn get_minimum_required_game_lod(&self) -> StaticGameLodLevel {
        StaticGameLodLevel::Low
    }

    fn get_radar_update_config(&self) -> Option<RadarUpdateConfig> {
        None
    }

    fn get_active_shroud_upgrade_config(&self) -> Option<ActiveShroudUpgradeConfig> {
        None
    }

    fn get_radar_upgrade_config(&self) -> Option<RadarUpgradeConfig> {
        None
    }

    fn get_type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn clone_box(&self) -> Box<dyn ModuleData>
    where
        Self: game_engine::thing::ModuleData,
    {
        Box::new(self.clone())
    }
}

#[macro_export]
macro_rules! impl_legacy_module_data_with_key_field {
    ($ty:ty, $field:ident) => {
        impl $crate::common::LegacyModuleData for $ty {
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }

            fn set_module_tag_name_key(&mut self, key: $crate::common::NameKeyType) {
                self.$field = key;
            }

            fn get_module_tag_name_key(&self) -> $crate::common::NameKeyType {
                self.$field
            }
        }

        impl ::game_engine::common::thing::module::ModuleData for $ty {
            fn as_any(&self) -> &dyn ::std::any::Any {
                $crate::common::LegacyModuleData::as_any(self)
            }

            fn set_module_tag_name_key(
                &mut self,
                key: ::game_engine::common::thing::module::NameKeyType,
            ) {
                $crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
            }

            fn get_module_tag_name_key(&self) -> ::game_engine::common::thing::module::NameKeyType {
                $crate::common::LegacyModuleData::get_module_tag_name_key(self)
            }

            fn is_ai_module_data(&self) -> bool {
                $crate::common::LegacyModuleData::is_ai_module_data(self)
            }

            fn get_as_w3d_model_draw_module_data(&self) -> Option<&dyn ::std::any::Any> {
                $crate::common::LegacyModuleData::get_as_w3d_model_draw_module_data(self)
            }

            fn get_as_w3d_tree_draw_module_data(&self) -> Option<&dyn ::std::any::Any> {
                $crate::common::LegacyModuleData::get_as_w3d_tree_draw_module_data(self)
            }

            fn get_minimum_required_game_lod(&self) -> ::game_engine::thing::StaticGameLodLevel {
                $crate::common::LegacyModuleData::get_minimum_required_game_lod(self)
            }
        }

        impl $crate::common::types::ModuleData for $ty {}
    };
}

#[macro_export]
macro_rules! impl_legacy_module_data_via_base {
    ($ty:ty, $field:ident) => {
        impl $crate::common::LegacyModuleData for $ty {
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }

            fn set_module_tag_name_key(&mut self, key: $crate::common::NameKeyType) {
                ::game_engine::common::thing::module::ModuleData::set_module_tag_name_key(
                    &mut self.$field,
                    key,
                );
            }

            fn get_module_tag_name_key(&self) -> $crate::common::NameKeyType {
                ::game_engine::common::thing::module::ModuleData::get_module_tag_name_key(
                    &self.$field,
                )
            }
        }

        impl ::game_engine::common::thing::module::ModuleData for $ty {
            fn as_any(&self) -> &dyn ::std::any::Any {
                $crate::common::LegacyModuleData::as_any(self)
            }

            fn set_module_tag_name_key(
                &mut self,
                key: ::game_engine::common::thing::module::NameKeyType,
            ) {
                $crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
            }

            fn get_module_tag_name_key(&self) -> ::game_engine::common::thing::module::NameKeyType {
                $crate::common::LegacyModuleData::get_module_tag_name_key(self)
            }

            fn is_ai_module_data(&self) -> bool {
                $crate::common::LegacyModuleData::is_ai_module_data(self)
            }

            fn get_as_w3d_model_draw_module_data(&self) -> Option<&dyn ::std::any::Any> {
                $crate::common::LegacyModuleData::get_as_w3d_model_draw_module_data(self)
            }

            fn get_as_w3d_tree_draw_module_data(&self) -> Option<&dyn ::std::any::Any> {
                $crate::common::LegacyModuleData::get_as_w3d_tree_draw_module_data(self)
            }

            fn get_minimum_required_game_lod(&self) -> ::game_engine::thing::StaticGameLodLevel {
                $crate::common::LegacyModuleData::get_minimum_required_game_lod(self)
            }
        }

        impl $crate::common::types::ModuleData for $ty {}
    };
}

#[macro_export]
macro_rules! impl_behavior_module_data_via_base {
    ($ty:ty, $field:ident) => {
        impl ::game_engine::common::system::Snapshotable for $ty {
            fn crc(&self, xfer: &mut dyn ::game_engine::system::Xfer) -> Result<(), String> {
                self.$field.crc(xfer)
            }

            fn xfer(&mut self, xfer: &mut dyn ::game_engine::system::Xfer) -> Result<(), String> {
                self.$field.xfer(xfer)
            }

            fn load_post_process(&mut self) -> Result<(), String> {
                self.$field.load_post_process()
            }
        }

        $crate::impl_legacy_module_data_via_base!($ty, $field);
    };
}

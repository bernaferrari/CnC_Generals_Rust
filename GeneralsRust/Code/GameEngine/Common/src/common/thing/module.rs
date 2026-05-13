////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Module system for objects and drawables
//! These are class instances that we can assign to objects, drawables, and things
//! to contain data and code for specific events, or just to hold data

pub use crate::common::rts::NameKeyType;
use crate::common::{
    ini::ini_upgrade::{get_upgrade_center, UpgradeTemplate},
    rts::AsciiString,
    system::{build_assistant::ObjectID, Snapshotable, Xfer},
};
use std::{
    any::Any,
    sync::{Arc, OnceLock, RwLock},
};

/// Time of day enumeration for asset preloading
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
    Night,
}

/// Static game LOD level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StaticGameLodLevel {
    Low = 0,
    Medium = 1,
    High = 2,
}

/// Module type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
    Behavior = 0,
    Draw = 1,
    ClientUpdate = 2,
}

impl ModuleType {
    pub const FIRST_DRAWABLE_MODULE_TYPE: ModuleType = ModuleType::Draw;
    pub const LAST_DRAWABLE_MODULE_TYPE: ModuleType = ModuleType::ClientUpdate;
    pub const NUM_MODULE_TYPES: usize = 3;
    pub const NUM_DRAWABLE_MODULE_TYPES: usize = 2;
}

/// Module interface type flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleInterfaceType(pub u32);

impl ModuleInterfaceType {
    pub const NONE: ModuleInterfaceType = ModuleInterfaceType(0x00000000);
    pub const UPDATE: ModuleInterfaceType = ModuleInterfaceType(0x00000001);
    pub const DIE: ModuleInterfaceType = ModuleInterfaceType(0x00000002);
    pub const DAMAGE: ModuleInterfaceType = ModuleInterfaceType(0x00000004);
    pub const CREATE: ModuleInterfaceType = ModuleInterfaceType(0x00000008);
    pub const COLLIDE: ModuleInterfaceType = ModuleInterfaceType(0x00000010);
    pub const BODY: ModuleInterfaceType = ModuleInterfaceType(0x00000020);
    pub const CONTAIN: ModuleInterfaceType = ModuleInterfaceType(0x00000040);
    pub const UPGRADE: ModuleInterfaceType = ModuleInterfaceType(0x00000080);
    pub const SPECIAL_POWER: ModuleInterfaceType = ModuleInterfaceType(0x00000100);
    pub const DESTROY: ModuleInterfaceType = ModuleInterfaceType(0x00000200);
    pub const DRAW: ModuleInterfaceType = ModuleInterfaceType(0x00000400);
    pub const CLIENT_UPDATE: ModuleInterfaceType = ModuleInterfaceType(0x00000800);
}

impl std::ops::BitOr for ModuleInterfaceType {
    type Output = ModuleInterfaceType;

    fn bitor(self, rhs: ModuleInterfaceType) -> Self::Output {
        ModuleInterfaceType(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ModuleInterfaceType {
    fn bitor_assign(&mut self, rhs: ModuleInterfaceType) {
        self.0 |= rhs.0;
    }
}

/// Base trait for data read from INI files for modules
pub trait ModuleData: Snapshotable + Send + Sync + std::fmt::Debug + Any {
    fn set_module_tag_name_key(&mut self, key: NameKeyType);
    fn get_module_tag_name_key(&self) -> NameKeyType;
    fn is_ai_module_data(&self) -> bool {
        false
    }

    // For W3D model compatibility (hack)
    fn get_as_w3d_model_draw_module_data(&self) -> Option<&dyn std::any::Any> {
        None
    }
    fn get_as_w3d_tree_draw_module_data(&self) -> Option<&dyn std::any::Any> {
        None
    }
    fn get_special_power_completion_template(&self) -> Option<&str> {
        None
    }
    fn get_beacon_client_update_config(&self) -> Option<BeaconClientUpdateConfig> {
        None
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
    fn get_dynamic_shroud_clearing_range_update_config(
        &self,
    ) -> Option<DynamicShroudClearingRangeUpdateConfig> {
        None
    }
    fn get_shroud_crate_collide_config(&self) -> Option<ShroudCrateCollideConfig> {
        None
    }
    fn get_minimum_required_game_lod(&self) -> StaticGameLodLevel {
        StaticGameLodLevel::Low
    }

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeaconClientUpdateConfig {
    pub frames_between_radar_pulses: u32,
    pub radar_pulse_duration: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarUpdateConfig {
    pub radar_extend_time: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveShroudUpgradeConfig {
    pub new_shroud_range: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarUpgradeConfig {
    pub is_disable_proof: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicShroudClearingRangeUpdateConfig {
    pub shrink_delay: u32,
    pub shrink_time: u32,
    pub grow_delay: u32,
    pub grow_time: u32,
    pub final_vision: f32,
    pub change_interval: u32,
    pub grow_interval: u32,
    pub do_spy_sat_fx: bool,
    pub grid_decal_template: RadiusDecalTemplateConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadiusDecalTemplateConfig {
    pub radius: f32,
    pub opacity: f32,
    pub color: u32,
    pub texture_name: String,
    pub shadow_type: u32,
    pub min_opacity: f32,
    pub max_opacity: f32,
    pub opacity_throb_time: u32,
    pub only_visible_to_owning_player: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShroudCrateCollideConfig {
    pub required_kind_of: u64,
    pub forbidden_kind_of: u64,
    pub is_forbid_owner_player: bool,
    pub is_building_pickup: bool,
    pub is_human_only_pickup: bool,
    pub pickup_science: i32,
    pub execute_fx: Option<String>,
    pub execution_animation_template: String,
    pub execute_animation_display_time_seconds: f32,
    pub execute_animation_z_rise_per_second: f32,
    pub execute_animation_fades: bool,
}

impl dyn ModuleData {
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

/// Concrete implementation of ModuleData
#[derive(Debug, Clone)]
pub struct BaseModuleData {
    module_tag_name_key: NameKeyType,
}

impl BaseModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl ModuleData for BaseModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for BaseModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // CRC implementation
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Serialization implementation
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Post-load processing
        Ok(())
    }
}

/// Common interface for thing modules
/// Create interface for modules that need initialization
pub trait CreateInterface {
    fn on_create(&self);
    fn on_build_complete(&self);
    fn should_do_on_build_complete(&self) -> bool;
}

pub trait ClientUpdateInterface {
    fn client_update(&mut self) -> bool {
        false
    }

    fn hide_beacon(&mut self) -> bool {
        false
    }
}

pub trait LaserUpdateInterface {
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, dirty: bool);
    fn get_start_pos(&self) -> [f32; 3];
    fn get_end_pos(&self) -> [f32; 3];
    fn get_width_scale(&self) -> f32;
    fn init_laser(
        &mut self,
        parent_id: Option<ObjectID>,
        target_id: Option<ObjectID>,
        start_pos: Option<[f32; 3]>,
        end_pos: Option<[f32; 3]>,
        parent_bone_name: String,
        size_delta_frames: i32,
    );
}

pub trait RadarUpdateInterface {
    fn extend_radar(&mut self);
    fn is_radar_active(&self) -> bool;
}

pub trait ProjectileStreamDrawInterface {
    fn projectile_stream_points(&mut self) -> Vec<[f32; 3]>;
}

pub trait TrainControlInterface {
    fn set_held(&mut self, held: bool);
}

pub trait CleanupHazardControlInterface {
    fn set_cleanup_area_parameters(&mut self, x: f32, y: f32, z: f32, range: f32);
}

pub trait SupplyWarehouseDockInterface {
    fn boxes_stored(&self) -> i32;
    fn set_cash_value(&mut self, cash_value: i32);
}

pub trait DeletionLifetimeInterface {
    fn set_lifetime_range(&mut self, min_lifetime: u32, max_lifetime: u32);
}

pub trait LifetimeControlInterface {
    fn die_frame(&self) -> u32;
}

pub trait BoneFxControlInterface {
    fn change_body_damage_state(&mut self, old_state: u32, new_state: u32);
    fn stop_all_bone_fx(&mut self);
}

pub trait ProneControlInterface {
    fn go_prone(&mut self, damage_dealt: i32);
}

pub trait StickyBombControlInterface {
    fn init_sticky_bomb(&mut self, target_id: ObjectID, bomber_id: ObjectID);
    fn detonate(&mut self);
    fn get_target(&self) -> ObjectID;
    fn set_target_object_id(&mut self, target_id: ObjectID);
}

pub trait OclUpdateControlInterface {
    fn reset_timer(&mut self);
    fn tick_ocl_update(&mut self);
}

pub trait HijackerControlInterface {
    fn configure_hijacked_vehicle(&mut self, target_id: ObjectID);
}

pub trait SpyVisionControlInterface {
    fn set_disabled_until_frame(&mut self, frame: u32);
}

pub trait StealthDetectorControlInterface {
    fn set_sd_enabled(&mut self, enabled: bool);
}

pub trait ModuleAny {
    fn module_as_any(&self) -> &dyn Any;
    fn module_as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> ModuleAny for T {
    fn module_as_any(&self) -> &dyn Any {
        self
    }
    fn module_as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub trait Module: ModuleAny + Snapshotable + Send + Sync + Any {
    fn as_any(&self) -> &dyn Any {
        ModuleAny::module_as_any(self)
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        ModuleAny::module_as_any_mut(self)
    }

    fn get_module_name_key(&self) -> NameKeyType {
        log::error!(
            "Module::get_module_name_key fell back to 0 for {}",
            std::any::type_name::<Self>()
        );
        0
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.get_module_data().get_module_tag_name_key()
    }

    /// Called after all modules for a given Thing are created
    fn on_object_created(&mut self) {}

    /// Called whenever a drawable is bound to the object
    fn on_drawable_bound_to_object(&mut self) {}

    /// Preload any assets for this time of day
    fn preload_assets(&mut self, _time_of_day: TimeOfDay) {}

    /// Called on all modules before deletion
    fn on_delete(&mut self) {}

    fn get_module_data(&self) -> &dyn ModuleData {
        static FALLBACK_MODULE_DATA: OnceLock<BaseModuleData> = OnceLock::new();
        log::error!(
            "Module::get_module_data fell back to BaseModuleData for {}",
            std::any::type_name::<Self>()
        );
        FALLBACK_MODULE_DATA.get_or_init(BaseModuleData::new)
    }

    /// Get create interface if this module supports creation
    fn get_create_interface(&self) -> Option<&dyn CreateInterface> {
        None
    }

    fn get_client_update_interface(&mut self) -> Option<&mut dyn ClientUpdateInterface> {
        None
    }

    fn get_laser_update_interface(&mut self) -> Option<&mut dyn LaserUpdateInterface> {
        None
    }

    fn get_radar_update_interface(&mut self) -> Option<&mut dyn RadarUpdateInterface> {
        None
    }

    fn get_projectile_stream_draw_interface(
        &mut self,
    ) -> Option<&mut dyn ProjectileStreamDrawInterface> {
        None
    }

    fn get_train_control_interface(&mut self) -> Option<&mut dyn TrainControlInterface> {
        None
    }

    fn get_cleanup_hazard_control_interface(
        &mut self,
    ) -> Option<&mut dyn CleanupHazardControlInterface> {
        None
    }

    fn get_supply_warehouse_dock_interface(
        &mut self,
    ) -> Option<&mut dyn SupplyWarehouseDockInterface> {
        None
    }

    fn get_deletion_lifetime_interface(&mut self) -> Option<&mut dyn DeletionLifetimeInterface> {
        None
    }

    fn get_lifetime_control_interface(&mut self) -> Option<&mut dyn LifetimeControlInterface> {
        None
    }

    fn get_bone_fx_control_interface(&mut self) -> Option<&mut dyn BoneFxControlInterface> {
        None
    }

    fn get_prone_control_interface(&mut self) -> Option<&mut dyn ProneControlInterface> {
        None
    }

    fn get_sticky_bomb_control_interface(&mut self) -> Option<&mut dyn StickyBombControlInterface> {
        None
    }

    fn get_ocl_update_control_interface(&mut self) -> Option<&mut dyn OclUpdateControlInterface> {
        None
    }

    fn get_hijacker_control_interface(&mut self) -> Option<&mut dyn HijackerControlInterface> {
        None
    }

    fn get_spy_vision_control_interface(&mut self) -> Option<&mut dyn SpyVisionControlInterface> {
        None
    }

    fn get_stealth_detector_control_interface(
        &mut self,
    ) -> Option<&mut dyn StealthDetectorControlInterface> {
        None
    }
}

/// Base module implementation
pub struct BaseModule {
    module_data: Arc<dyn ModuleData>,
    module_name_key: NameKeyType,
}

impl BaseModule {
    pub fn new(module_data: Arc<dyn ModuleData>, module_name_key: NameKeyType) -> Self {
        Self {
            module_data,
            module_name_key,
        }
    }
}

impl Module for BaseModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        &*self.module_data
    }
}

impl Snapshotable for BaseModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Base CRC implementation
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Base serialization implementation
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Base post-load processing
        Ok(())
    }
}

/// Forward declarations for game object types
pub trait Object: Send + Sync {
    /// Get the unique identifier for this object.
    fn get_object_id(&self) -> ObjectID {
        0
    }

    /// Get all behavior modules for this object
    fn get_behavior_modules(&self) -> Vec<Arc<dyn Module>> {
        Vec::new() // Default empty implementation
    }

    /// Initialize the object
    fn init_object(&self) {
        // Default empty implementation
    }

    /// Try to upgrade into a concrete object handle if available.
    fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn Object>>> {
        None
    }

    /// Remove an upgrade from the object.
    fn remove_upgrade(&self, _upgrade_template: Option<&UpgradeTemplate>) {}
}

pub trait Drawable: Send + Sync {
    fn get_drawable_id(&self) -> u32 {
        0
    }
}

pub trait Thing: std::fmt::Debug + Send + Sync {
    fn as_object(&self) -> Option<&dyn Object> {
        None
    }
    fn as_drawable(&self) -> Option<&dyn Drawable> {
        None
    }
}

pub trait Player: Send + Sync {
    // Player-specific methods would be defined here
}

/// Module interface specific for Objects
pub trait ObjectModule: Module {
    fn on_capture(&mut self, _old_owner: Option<&dyn Player>, _new_owner: Option<&dyn Player>) {}
    fn on_disabled_edge(&mut self, _now_disabled: bool) {}

    fn get_object(&self) -> Option<&dyn Object>;
}

/// Object module implementation
pub struct BaseObjectModule {
    base: BaseModule,
    object: Option<Arc<dyn Object>>,
}

impl BaseObjectModule {
    pub fn new(
        module_data: Arc<dyn ModuleData>,
        module_name_key: NameKeyType,
        object: Option<Arc<dyn Object>>,
    ) -> Self {
        Self {
            base: BaseModule::new(module_data, module_name_key),
            object,
        }
    }
}

impl Module for BaseObjectModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.base.get_module_data()
    }
}

impl ObjectModule for BaseObjectModule {
    fn get_object(&self) -> Option<&dyn Object> {
        self.object.as_ref().map(|o| &**o)
    }
}

impl Snapshotable for BaseObjectModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// Module interface specific for Drawables
pub trait DrawableModule: Module {
    fn get_drawable(&self) -> Option<&dyn Drawable>;
}

/// Drawable module implementation
pub struct BaseDrawableModule {
    base: BaseModule,
    drawable: Option<Arc<dyn Drawable>>,
}

impl BaseDrawableModule {
    pub fn new(
        module_data: Arc<dyn ModuleData>,
        module_name_key: NameKeyType,
        drawable: Option<Arc<dyn Drawable>>,
    ) -> Self {
        Self {
            base: BaseModule::new(module_data, module_name_key),
            drawable,
        }
    }
}

impl Module for BaseDrawableModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.base.get_module_data()
    }
}

impl DrawableModule for BaseDrawableModule {
    fn get_drawable(&self) -> Option<&dyn Drawable> {
        self.drawable.as_ref().map(|d| &**d)
    }
}

impl Snapshotable for BaseDrawableModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// Upgrade processing data
#[derive(Debug, Clone)]
pub struct UpgradeMuxData {
    fx_list_upgrade: Option<Arc<dyn std::any::Any + Send + Sync>>, // FXList pointer
    removal_upgrade_names: Vec<AsciiString>,
    trigger_upgrade_names: Vec<AsciiString>,
    activation_upgrade_names: Vec<AsciiString>,
    conflicting_upgrade_names: Vec<AsciiString>,
    activation_mask: u64,  // UpgradeMaskType
    conflicting_mask: u64, // UpgradeMaskType
    requires_all_triggers: bool,
}

impl UpgradeMuxData {
    pub fn new() -> Self {
        Self {
            fx_list_upgrade: None,
            removal_upgrade_names: Vec::new(),
            trigger_upgrade_names: Vec::new(),
            activation_upgrade_names: Vec::new(),
            conflicting_upgrade_names: Vec::new(),
            activation_mask: 0,
            conflicting_mask: 0,
            requires_all_triggers: false,
        }
    }

    pub fn perform_upgrade_fx(&self, _obj: &dyn Object) {
        if let Some(_fx_list) = &self.fx_list_upgrade {
            // FXList::doFXObj(fxList, obj);
        }
    }

    pub fn mux_data_process_upgrade_removal(&self, obj: &dyn Object) {
        if self.removal_upgrade_names.is_empty() {
            return;
        }

        if let Some(upgrade_center) = get_upgrade_center() {
            for upgrade_name in &self.removal_upgrade_names {
                let lookup_name =
                    crate::common::ascii_string::AsciiString::from(upgrade_name.as_str());
                let the_template = if lookup_name.is_empty() || lookup_name.is_none() {
                    None
                } else {
                    upgrade_center.find_template(&lookup_name)
                };

                if the_template.is_none() && !lookup_name.is_empty() && !lookup_name.is_none() {
                    panic!(
                        "An upgrade module references {}, which is not an Upgrade",
                        lookup_name.as_str()
                    );
                }

                obj.remove_upgrade(the_template);
            }
        }
    }

    pub fn is_triggered_by(&self, upgrade: &str) -> bool {
        for trigger in &self.trigger_upgrade_names {
            if trigger.to_lowercase() == upgrade.to_lowercase() {
                return true;
            }
        }
        false
    }

    pub fn get_upgrade_activation_masks(&self) -> (u64, u64) {
        // This would compute and cache the activation masks from upgrade names
        // For now, return cached values
        (self.activation_mask, self.conflicting_mask)
    }

    pub fn requires_all_triggers(&self) -> bool {
        self.requires_all_triggers
    }

    #[allow(dead_code)] // C++ API parity: ModuleData setter
    pub fn set_requires_all_triggers(&mut self, value: bool) {
        self.requires_all_triggers = value;
    }

    pub fn activation_upgrade_names(&self) -> &[AsciiString] {
        &self.activation_upgrade_names
    }

    pub fn conflicting_upgrade_names(&self) -> &[AsciiString] {
        &self.conflicting_upgrade_names
    }

    pub fn trigger_upgrade_names(&self) -> &[AsciiString] {
        &self.trigger_upgrade_names
    }

    pub fn removal_upgrade_names(&self) -> &[AsciiString] {
        &self.removal_upgrade_names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct DefaultAnyModule;

    impl Snapshotable for DefaultAnyModule {
        fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
            Ok(())
        }

        fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
            Ok(())
        }

        fn load_post_process(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    impl Module for DefaultAnyModule {}

    #[test]
    fn module_default_as_any_supports_downcast_without_override() {
        let mut module = DefaultAnyModule;

        assert!(module.as_any().downcast_ref::<DefaultAnyModule>().is_some());
        assert!(module
            .as_any_mut()
            .downcast_mut::<DefaultAnyModule>()
            .is_some());
    }
}

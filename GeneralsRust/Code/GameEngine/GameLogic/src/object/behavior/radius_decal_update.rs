//! RadiusDecalUpdate - Rust conversion of C++ RadiusDecalUpdate
//!
//! Update module that manages radius decals on terrain (visual indicators).
//! Used for targeting indicators and effect radii.
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, CoordOrigin, ModuleData, ObjectStatusTypes, RadiusDecal,
    RadiusDecalTemplate, Real, UnsignedInt, XferVersion, INVALID_ID,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use crate::player::ThePlayerList;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use std::sync::{Arc, RwLock, Weak};

fn decal_is_empty(decal: &RadiusDecal) -> bool {
    decal.radius <= 0.0
}

/// INI-configurable data for RadiusDecalUpdate
#[derive(Clone, Debug)]
pub struct RadiusDecalUpdateModuleData {
    pub base: BehaviorModuleData,
    // Commented out in C++ original - kept for parity:
    // pub delivery_decal_template: RadiusDecalTemplate,
    // pub delivery_decal_radius: Real,
}

impl Default for RadiusDecalUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(RadiusDecalUpdateModuleData, base);

/// RadiusDecalUpdate - manages terrain radius decals for visual feedback
pub struct RadiusDecalUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<RadiusDecalUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,

    /// The radius decal being managed
    delivery_decal: RadiusDecal,
    /// Whether to kill decal when object stops attacking
    kill_when_no_longer_attacking: bool,
    /// Sleep state for optimization
    sleeping: bool,
}

impl RadiusDecalUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<RadiusDecalUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let mut decal = RadiusDecal::new(Coord3D::origin(), 0.0);
        decal.clear();

        if let Ok(obj) = object.read() {
            TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(data.clone()),
            next_call_frame_and_phase: 0,
            delivery_decal: decal,
            kill_when_no_longer_attacking: false,
            sleeping: true, // Start sleeping (UPDATE_SLEEP_FOREVER in C++)
        })
    }

    /// Create a radius decal at specified position
    pub fn create_radius_decal(
        &mut self,
        template: &RadiusDecalTemplate,
        radius: Real,
        pos: &Coord3D,
    ) {
        self.delivery_decal.clear();

        let owner_index = self
            .object
            .upgrade()
            .and_then(|obj| obj.read().ok().and_then(|o| o.get_controlling_player()))
            .and_then(|player| player.read().ok().map(|p| p.get_player_index()));
        let local_index = ThePlayerList()
            .read()
            .ok()
            .map(|list| list.get_local_player_index());
        let allow_decal = if template.only_visible_to_owning_player {
            matches!((local_index, owner_index), (Some(local), Some(owner)) if local == owner)
        } else {
            true
        };
        if allow_decal {
            let mut decal = template.create_radius_decal_with_radius(*pos, radius);
            if !decal.is_empty() {
                if template.color == 0 {
                    if let (Some(owner), Ok(list)) = (owner_index, ThePlayerList().read()) {
                        if let Some(player) = list.get_player(owner).and_then(|p| p.read().ok()) {
                            decal.color = player.get_player_color().to_argb_u32();
                        }
                    }
                }
                self.delivery_decal = decal;
            }
        }
        self.sleeping = decal_is_empty(&self.delivery_decal);
        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj) = obj_arc.read() {
                let sleep = if self.sleeping {
                    UpdateSleepTime::Forever
                } else {
                    UpdateSleepTime::None
                };
                TheGameLogic::set_wake_frame(obj.get_id(), sleep);
            }
        }
    }

    /// Set whether to kill decal when object stops attacking
    pub fn kill_when_no_longer_attacking(&mut self, value: bool) {
        self.kill_when_no_longer_attacking = value;
    }

    /// Kill the radius decal immediately
    pub fn kill_radius_decal(&mut self) {
        self.delivery_decal.clear();
        self.sleeping = true;
        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj) = obj_arc.read() {
                TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
            }
        }
    }
}

impl UpdateModuleInterface for RadiusDecalUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // If sleeping and nothing to update, stay asleep
        if self.sleeping && decal_is_empty(&self.delivery_decal) {
            return UPDATE_SLEEP_FOREVER;
        }

        // Check if we should kill decal when object stops attacking
        if self.kill_when_no_longer_attacking {
            if let Some(obj_arc) = self.object.upgrade() {
                if let Ok(obj) = obj_arc.read() {
                    // Check if object is no longer attacking
                    if !obj.get_status_bits().test(ObjectStatusTypes::IsAttacking) {
                        self.delivery_decal.clear();
                        self.sleeping = true;
                        if let Some(obj_id) = self
                            .object
                            .upgrade()
                            .and_then(|o| o.read().ok().map(|g| g.get_id()))
                        {
                            TheGameLogic::set_wake_frame(obj_id, UpdateSleepTime::Forever);
                        }
                        return UPDATE_SLEEP_FOREVER;
                    }
                }
            }
        }

        // Update the decal
        self.delivery_decal.update();
        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for RadiusDecalUpdate {
    fn get_module_name(&self) -> &'static str {
        "RadiusDecalUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

/// Glue that exposes RadiusDecalUpdate through the common Module trait.
pub struct RadiusDecalUpdateModule {
    behavior: RadiusDecalUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<RadiusDecalUpdateModuleData>,
}

impl RadiusDecalUpdateModule {
    pub fn new(
        behavior: RadiusDecalUpdate,
        module_name: &AsciiString,
        module_data: Arc<RadiusDecalUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut RadiusDecalUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for RadiusDecalUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for RadiusDecalUpdateModule {
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
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

/// Interface for RadiusDecalUpdate behavior
pub trait RadiusDecalUpdateInterface {
    fn create_radius_decal(&mut self, template: &RadiusDecalTemplate, radius: Real, pos: &Coord3D);
    fn kill_when_no_longer_attacking(&mut self, value: bool);
    fn kill_radius_decal(&mut self);
}

impl RadiusDecalUpdateInterface for RadiusDecalUpdate {
    fn create_radius_decal(&mut self, template: &RadiusDecalTemplate, radius: Real, pos: &Coord3D) {
        RadiusDecalUpdate::create_radius_decal(self, template, radius, pos);
    }

    fn kill_when_no_longer_attacking(&mut self, value: bool) {
        RadiusDecalUpdate::kill_when_no_longer_attacking(self, value);
    }

    fn kill_radius_decal(&mut self) {
        RadiusDecalUpdate::kill_radius_decal(self);
    }
}

impl Snapshotable for RadiusDecalUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        // xfer_radius_decal_mut follows the legacy extension API and does not surface Result.
        xfer.xfer_radius_decal_mut(&mut self.delivery_decal);
        xfer.xfer_bool(&mut self.kill_when_no_longer_attacking)
            .map_err(|e| format!("Failed to xfer kill_when_no_longer_attacking: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct RadiusDecalUpdateFactory;
impl RadiusDecalUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(RadiusDecalUpdate::new(thing, module_data)?))
    }
}

pub fn radius_decal_update_data_factory(
    _ini: Option<&mut game_engine::common::ini::INI>,
) -> Box<dyn EngineModuleData> {
    Box::new(RadiusDecalUpdateModuleData::default())
}

pub fn radius_decal_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<RadiusDecalUpdateModuleData>()
        .expect("RadiusDecalUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("RadiusDecalUpdate requires object");
    let behavior = RadiusDecalUpdate::new(object, module_data_arc.clone())
        .expect("RadiusDecalUpdate failed to initialize");
    let module_name = AsciiString::from("RadiusDecalUpdate");
    Box::new(RadiusDecalUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

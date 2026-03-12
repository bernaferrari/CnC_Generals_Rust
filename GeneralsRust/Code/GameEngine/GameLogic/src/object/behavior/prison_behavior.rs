//! PrisonBehavior - Rust conversion of C++ PrisonBehavior.
//!
//! Extends OpenContain to hold surrendered units and optionally render yard visuals.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock, Weak};

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::audio::TimeOfDay;
use crate::common::{Bool, Coord3D, DisabledType, LegacyModuleData, ObjectID, Real, INVALID_ID};
use crate::helpers::{TheGameClient, TheGlobalData};
use crate::modules::{
    BehaviorModuleInterface, ContainModuleInterface, ContainWant, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::contain::{OpenContain, OpenContainModuleData};
use crate::object::Object;

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
pub struct PrisonBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    pub base: OpenContainModuleData,
    pub show_prisoners: Bool,
    pub prison_yard_bone_prefix: AsciiString,
}

#[cfg(feature = "allow_surrender")]
impl Default for PrisonBehaviorModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: OpenContainModuleData::default(),
            show_prisoners: false,
            prison_yard_bone_prefix: AsciiString::from(""),
        }
    }
}

#[cfg(feature = "allow_surrender")]
fn parse_show_prisoners(
    _ini: &mut INI,
    data: &mut PrisonBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.show_prisoners = INI::parse_bool(token)?;
    Ok(())
}

#[cfg(feature = "allow_surrender")]
fn parse_yard_bone_prefix(
    _ini: &mut INI,
    data: &mut PrisonBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.prison_yard_bone_prefix = AsciiString::from(token);
    Ok(())
}

#[cfg(feature = "allow_surrender")]
const PRISON_BEHAVIOR_FIELDS: &[FieldParse<PrisonBehaviorModuleData>] = &[
    FieldParse {
        token: "ShowPrisoners",
        parse: parse_show_prisoners,
    },
    FieldParse {
        token: "YardBonePrefix",
        parse: parse_yard_bone_prefix,
    },
];

#[cfg(feature = "allow_surrender")]
impl PrisonBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields(self, PRISON_BEHAVIOR_FIELDS)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for PrisonBehaviorModuleData {
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

#[cfg(feature = "allow_surrender")]
crate::impl_legacy_module_data_with_key_field!(PrisonBehaviorModuleData, module_tag_name_key);

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone)]
struct PrisonVisual {
    object_id: ObjectID,
    drawable_id: crate::common::DrawableID,
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug, Clone, Copy)]
struct Region2D {
    lo_x: Real,
    lo_y: Real,
    hi_x: Real,
    hi_y: Real,
}

#[cfg(feature = "allow_surrender")]
impl Region2D {
    fn width(&self) -> Real {
        self.hi_x - self.lo_x
    }

    fn height(&self) -> Real {
        self.hi_y - self.lo_y
    }
}

#[cfg(feature = "allow_surrender")]
fn point_inside_area_2d(point: &Coord3D, polygon: &[Coord3D]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let xi = polygon[i].x;
        let yi = polygon[i].y;
        let xj = polygon[j].x;
        let yj = polygon[j].y;
        let intersects = ((yi > point.y) != (yj > point.y))
            && (point.x < (xj - xi) * (point.y - yi) / (yj - yi + f32::EPSILON) + xi);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct PrisonBehavior {
    object: Weak<RwLock<Object>>,
    module_data: Arc<PrisonBehaviorModuleData>,
    contain: OpenContain,
    visuals: Vec<PrisonVisual>,
}

#[cfg(feature = "allow_surrender")]
impl PrisonBehavior {
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<PrisonBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let contain = OpenContain::new(Arc::downgrade(&object), &module_data.base)?;
        Ok(Self {
            object: Arc::downgrade(&object),
            module_data,
            contain,
            visuals: Vec::new(),
        })
    }

    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object.upgrade()
    }

    fn pick_visual_location(&self) -> Coord3D {
        let Some(owner) = self.get_object() else {
            return Coord3D::ZERO;
        };

        let Ok(owner_guard) = owner.read() else {
            return Coord3D::ZERO;
        };

        let mut picked = *owner_guard.get_position();
        let yard_positions = owner_guard
            .get_multi_logical_bone_position(self.module_data.prison_yard_bone_prefix.as_str(), 16);

        if yard_positions.len() >= 3 {
            let mut region = Region2D {
                lo_x: yard_positions[0].x,
                lo_y: yard_positions[0].y,
                hi_x: yard_positions[0].x,
                hi_y: yard_positions[0].y,
            };

            for pos in &yard_positions[1..] {
                region.lo_x = region.lo_x.min(pos.x);
                region.lo_y = region.lo_y.min(pos.y);
                region.hi_x = region.hi_x.max(pos.x);
                region.hi_y = region.hi_y.max(pos.y);
            }

            picked.x = region.lo_x + region.width() * 0.5;
            picked.y = region.lo_y + region.height() * 0.5;

            let max_tries = 32;
            for _ in 0..max_tries {
                let loc = Coord3D::new(
                    crate::GameLogicRandomValueReal!(region.lo_x, region.hi_x),
                    crate::GameLogicRandomValueReal!(region.lo_y, region.hi_y),
                    picked.z,
                );
                if point_inside_area_2d(&loc, &yard_positions) {
                    picked = loc;
                    break;
                }
            }
        }

        picked
    }

    fn add_visual(&mut self, obj: &Object) {
        let Some(client) = TheGameClient::get() else {
            return;
        };

        let draw_id = client.create_drawable(obj.get_template().as_ref());
        if draw_id == 0 {
            return;
        }

        let color = TheGlobalData::get()
            .map(|data| data.get_time_of_day())
            .unwrap_or(TimeOfDay::Day);
        let indicator = match color {
            TimeOfDay::Night => obj.get_night_indicator_color(),
            _ => obj.get_indicator_color(),
        };

        let pos = self.pick_visual_location();
        let orient = crate::GameLogicRandomValueReal!(0.0, std::f32::consts::PI * 2.0);

        client.set_drawable_indicator_color(draw_id, indicator);
        client.set_drawable_position(draw_id, &pos);
        client.set_drawable_orientation(draw_id, orient);

        if let Some(owner) = self.get_object() {
            let owner_id = owner
                .read()
                .map(|guard| guard.get_id())
                .unwrap_or(INVALID_ID);
            client.set_drawable_shroud_status_object_id(draw_id, owner_id);
        }

        self.visuals.push(PrisonVisual {
            object_id: obj.get_id(),
            drawable_id: draw_id,
        });
    }

    /// Mirrors C++ PrisonBehavior::onDelete (cleanup visuals and containment state).
    pub fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.cleanup_visuals();
        Ok(())
    }

    fn remove_visual(&mut self, obj: &Object) {
        let Some(client) = TheGameClient::get() else {
            return;
        };

        if let Some(index) = self
            .visuals
            .iter()
            .position(|vis| vis.object_id == obj.get_id())
        {
            let drawable_id = self.visuals[index].drawable_id;
            self.visuals.remove(index);
            client.destroy_drawable(drawable_id);
        }
    }

    fn cleanup_visuals(&mut self) {
        let Some(client) = TheGameClient::get() else {
            self.visuals.clear();
            return;
        };

        for visual in self.visuals.drain(..) {
            client.destroy_drawable(visual.drawable_id);
        }
    }
}

#[cfg(feature = "allow_surrender")]
impl UpdateModuleInterface for PrisonBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.contain
            .update()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[cfg(feature = "allow_surrender")]
impl ContainModuleInterface for PrisonBehavior {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        self.contain.can_contain(object_id)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.contain.contain_object(object_id)
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.contain.release_object(object_id)
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        self.contain.get_contained_objects()
    }

    fn get_contained_count(&self) -> usize {
        self.contain.get_contained_count()
    }

    fn get_max_capacity(&self) -> usize {
        self.contain.get_max_capacity()
    }

    fn is_enclosing_container_for(&self, obj: &Object) -> bool {
        self.contain.is_enclosing_container_for(obj)
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.contain.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.add_to_contain(obj)
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.on_object_wants_to_enter_or_exit(obj, want)
    }

    fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.on_containing(Arc::clone(&obj), was_selected)?;
        if let Ok(mut guard) = obj.write() {
            guard.set_disabled(DisabledType::Held);
            if self.module_data.show_prisoners {
                self.add_visual(&*guard);
            }
        }
        Ok(())
    }

    fn on_removing(
        &mut self,
        obj: Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut guard) = obj.write() {
            if self.module_data.show_prisoners {
                self.remove_visual(&*guard);
            }
            guard.clear_disabled(DisabledType::Held);
        }
        self.contain.on_removing(obj)
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.remove_all_contained(expose_stealth)
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain.client_visible_contained_flash_as_selected()
    }

    fn friend_get_rider(&self) -> Option<ObjectID> {
        self.contain.friend_get_rider()
    }
}

#[cfg(feature = "allow_surrender")]
impl BehaviorModuleInterface for PrisonBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_contain(&mut self) -> Option<&mut dyn ContainModuleInterface> {
        Some(self)
    }
}

#[cfg(feature = "allow_surrender")]
#[derive(Debug)]
pub struct PrisonBehaviorModule {
    behavior: Arc<Mutex<PrisonBehavior>>,
    module_name_key: NameKeyType,
    module_data: Arc<PrisonBehaviorModuleData>,
}

#[cfg(feature = "allow_surrender")]
impl PrisonBehaviorModule {
    pub fn new(
        behavior: PrisonBehavior,
        module_name: &AsciiString,
        module_data: Arc<PrisonBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            module_name_key,
            module_data,
        }
    }

    pub fn contain_handle(&self) -> Arc<Mutex<dyn ContainModuleInterface>> {
        Arc::clone(&self.behavior)
    }
}

#[cfg(feature = "allow_surrender")]
impl Snapshotable for PrisonBehaviorModule {
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

#[cfg(feature = "allow_surrender")]
impl Module for PrisonBehaviorModule {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_delete(&mut self) {
        if let Ok(mut guard) = self.behavior.lock() {
            guard.cleanup_visuals();
        }
    }
}

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default, Clone)]
pub struct PrisonBehaviorModuleData;

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default)]
pub struct PrisonBehavior;

#[cfg(not(feature = "allow_surrender"))]
#[derive(Debug, Default)]
pub struct PrisonBehaviorModule;

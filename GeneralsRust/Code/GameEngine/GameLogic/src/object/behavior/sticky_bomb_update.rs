//! StickyBombUpdate - Sticky bomb that attaches to targets
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, KindOf, ModuleData, ObjectID, ObjectStatusMaskType, PlayerMaskType, Real,
    UnsignedInt, LOGICFRAMES_PER_SECOND,
};
use crate::damage::DamageInfo;
use crate::effects::FXList;
use crate::helpers::{
    TheAudio, TheFXListStore, TheGameLogic, ThePartitionManager, TheTerrainLogic,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use crate::weapon::{with_weapon_store, WeaponBonus, WeaponTemplate};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    StickyBombControlInterface, Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct StickyBombUpdateModuleData {
    pub base: BehaviorModuleData,
    pub attach_to_bone: AsciiString,
    pub offset_z: Real,
    pub geometry_based_damage_weapon_template: Option<Arc<WeaponTemplate>>,
    pub geometry_based_damage_fx: Option<Arc<FXList>>,
}

impl Default for StickyBombUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            attach_to_bone: AsciiString::new(),
            offset_z: 10.0,
            geometry_based_damage_weapon_template: None,
            geometry_based_damage_fx: None,
        }
    }
}

impl Snapshotable for StickyBombUpdateModuleData {
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

crate::impl_legacy_module_data_via_base!(StickyBombUpdateModuleData, base);

impl StickyBombUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STICKY_BOMB_UPDATE_FIELDS)
    }
}

fn parse_attach_to_bone(
    _ini: &mut INI,
    data: &mut StickyBombUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.attach_to_bone = AsciiString::from(*token);
    Ok(())
}

fn parse_offset_z(
    _ini: &mut INI,
    data: &mut StickyBombUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.offset_z = INI::parse_real(token)?;
    Ok(())
}

fn parse_geometry_based_damage_weapon(
    _ini: &mut INI,
    data: &mut StickyBombUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let template =
        with_weapon_store(|weapon_store| weapon_store.find_weapon_template(token).cloned())
            .ok()
            .flatten();
    data.geometry_based_damage_weapon_template = template;
    Ok(())
}

fn parse_geometry_based_damage_fx(
    _ini: &mut INI,
    data: &mut StickyBombUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.geometry_based_damage_fx = None;
    } else {
        data.geometry_based_damage_fx = TheFXListStore::find_fx_list(token);
    }
    Ok(())
}

const STICKY_BOMB_UPDATE_FIELDS: &[FieldParse<StickyBombUpdateModuleData>] = &[
    FieldParse {
        token: "AttachToTargetBone",
        parse: parse_attach_to_bone,
    },
    FieldParse {
        token: "OffsetZ",
        parse: parse_offset_z,
    },
    FieldParse {
        token: "GeometryBasedDamageWeapon",
        parse: parse_geometry_based_damage_weapon,
    },
    FieldParse {
        token: "GeometryBasedDamageFX",
        parse: parse_geometry_based_damage_fx,
    },
];

#[derive(Debug)]
pub struct StickyBombUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<StickyBombUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    target_id: ObjectID,
    die_frame: UnsignedInt,
    next_ping_frame: UnsignedInt,
}

impl StickyBombUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<StickyBombUpdateModuleData>()
            .ok_or("Invalid module data")?;

        if let Ok(obj) = object.read() {
            TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            target_id: OBJECT_INVALID_ID,
            die_frame: 0,
            next_ping_frame: 0,
        })
    }

    /// Attach sticky bomb to a target - C++ initStickyBomb()
    pub fn init_sticky_bomb(
        &mut self,
        target: Option<&GameObject>,
        bomber: Option<&GameObject>,
        specific_pos: Option<&Coord3D>,
    ) {
        self.target_id = target.map(|t| t.get_id()).unwrap_or(OBJECT_INVALID_ID);

        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(mut obj) = obj_arc.write() {
                obj.set_producer(target);

                let now = TheGameLogic::get_frame();
                let mut die_frame = 0;
                if let Some(module) = obj.find_update_module("LifetimeUpdate") {
                    module.with_module(|module| {
                        if let Some(lifetime) = module.get_lifetime_control_interface() {
                            die_frame = lifetime.die_frame();
                        }
                    });
                }
                if die_frame == 0 {
                    for module in obj.behavior_modules() {
                        module.with_module(|module| {
                            if let Some(lifetime) = module.get_lifetime_control_interface() {
                                die_frame = lifetime.die_frame();
                            }
                        });
                        if die_frame != 0 {
                            break;
                        }
                    }
                }

                self.die_frame = die_frame;
                if die_frame > 0 {
                    let remaining = die_frame.wrapping_sub(now);
                    let pings = remaining / LOGICFRAMES_PER_SECOND;
                    self.next_ping_frame = die_frame.wrapping_sub(pings * LOGICFRAMES_PER_SECOND);
                } else {
                    self.next_ping_frame = now.wrapping_add(LOGICFRAMES_PER_SECOND);
                }
                TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::None);

                if let Some(target) = target {
                    let mut pos = *target.get_position();
                    if let Some(specific_pos) = specific_pos {
                        pos = *specific_pos;
                        if let Some(terrain) = TheTerrainLogic::get() {
                            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                        }
                    } else if target.is_kind_of(crate::common::KindOf::Immobile) && bomber.is_some()
                    {
                        if let Some(bomber) = bomber {
                            pos = *bomber.get_position();
                            if let Some(terrain) = TheTerrainLogic::get() {
                                pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                            }
                        }
                    } else {
                        pos.z += self.module_data.offset_z;
                    }
                    let _ = obj.set_position(&pos);

                    if obj.is_kind_of(KindOf::BoobyTrap) {
                        if let Some(target_arc) = TheGameLogic::find_object_by_id(target.get_id()) {
                            if let Ok(mut target_guard) = target_arc.write() {
                                target_guard.set_status(ObjectStatusMaskType::BOOBY_TRAPPED, true);
                            }
                        }
                    }

                    if let Some(sound) = obj.get_template().get_per_unit_sound("StickyBombCreated")
                    {
                        if let Some(audio) = TheAudio::get() {
                            let mut event = sound.clone();
                            event.set_position(&(pos.x, pos.y, pos.z));
                            audio.add_audio_event(&event);
                        }
                    }
                }
            }
        }
    }

    pub fn init_sticky_bomb_by_id(&mut self, target_id: ObjectID, bomber_id: ObjectID) {
        let target = if target_id == OBJECT_INVALID_ID {
            None
        } else {
            TheGameLogic::find_object_by_id(target_id)
        };
        let bomber = if bomber_id == OBJECT_INVALID_ID {
            None
        } else {
            TheGameLogic::find_object_by_id(bomber_id)
        };

        match (target, bomber) {
            (Some(target), Some(bomber)) => {
                if let (Ok(target), Ok(bomber)) = (target.read(), bomber.read()) {
                    self.init_sticky_bomb(Some(&target), Some(&bomber), None);
                }
            }
            (Some(target), None) => {
                if let Ok(target) = target.read() {
                    self.init_sticky_bomb(Some(&target), None, None);
                }
            }
            (None, Some(bomber)) => {
                if let Ok(bomber) = bomber.read() {
                    self.init_sticky_bomb(None, Some(&bomber), None);
                }
            }
            (None, None) => self.init_sticky_bomb(None, None, None),
        }
    }

    /// Get the target object this bomb is attached to
    pub fn get_target(&self) -> ObjectID {
        self.target_id
    }

    /// Match C++ getTargetObject().
    pub fn get_target_object(&self) -> Option<Arc<RwLock<GameObject>>> {
        if self.target_id == OBJECT_INVALID_ID {
            return None;
        }
        TheGameLogic::find_object_by_id(self.target_id)
    }

    /// Set the target object (mirrors C++ setTargetObject).
    pub fn set_target_object(&mut self, obj: Option<&GameObject>) {
        self.target_id = obj.map(|o| o.get_id()).unwrap_or(OBJECT_INVALID_ID);
    }

    /// Returns true if the bomb uses a lifetime timer.
    pub fn is_timed_bomb(&self) -> bool {
        self.die_frame > 0
    }

    /// Get the frame when this bomb will detonate.
    pub fn get_detonation_frame(&self) -> UnsignedInt {
        self.die_frame
    }

    /// Detonate the sticky bomb - C++ detonate()
    pub fn detonate(&mut self) {
        let booby_trapped = self.get_target_object();

        if let Some(template) = self
            .module_data
            .geometry_based_damage_weapon_template
            .as_ref()
        {
            if let (Some(target_arc), Some(object_arc)) =
                (booby_trapped.as_ref(), self.object.upgrade())
            {
                if let (Ok(target_guard), Ok(obj)) = (target_arc.read(), object_arc.read()) {
                    let bonus = WeaponBonus::default();
                    let bounding_circle = target_guard
                        .get_geometry_info()
                        .get_bounding_circle_radius();
                    let primary_damage = template.get_primary_damage(&bonus);
                    let secondary_damage = template.get_secondary_damage(&bonus);
                    let primary_range =
                        template.get_primary_damage_radius(&bonus) + bounding_circle;
                    let secondary_range =
                        template.get_secondary_damage_radius(&bonus) + bounding_circle;
                    let primary_range_sqr = primary_range * primary_range;
                    let radius = primary_range.max(secondary_range);
                    let target_pos = *target_guard.get_position();
                    let source_player_mask = match obj.get_controlling_player() {
                        Some(player_arc) => player_arc
                            .read()
                            .ok()
                            .map(|player| player.get_player_mask())
                            .unwrap_or(PlayerMaskType::none()),
                        None => PlayerMaskType::none(),
                    };

                    let mut damage_info = DamageInfo::new();
                    damage_info.input.source_id = obj.get_id();
                    damage_info.input.source_player_mask = source_player_mask;
                    damage_info.input.damage_type = template.damage_type.into();
                    damage_info.input.death_type = template.death_type.into();
                    damage_info.input.damage_status_type = template.damage_status_type.into();

                    if let Some(partition) = ThePartitionManager::get() {
                        for id in partition.get_objects_in_range_boundary_3d(&target_pos, radius) {
                            let Some(victim_arc) = TheGameLogic::find_object_by_id(id) else {
                                continue;
                            };
                            let Ok(mut victim) = victim_arc.write() else {
                                continue;
                            };

                            let victim_pos = *victim.get_position();
                            let geom = victim.get_geometry_info();
                            let center_z_delta = (geom.bounds.min.z + geom.bounds.max.z) * 0.5;
                            let delta = Coord3D::new(
                                victim_pos.x - target_pos.x,
                                victim_pos.y - target_pos.y,
                                (victim_pos.z + center_z_delta) - target_pos.z,
                            );
                            let center_dist = delta.length();
                            let victim_radius = geom.get_bounding_sphere_radius();
                            let boundary_dist = if center_dist <= victim_radius {
                                0.0
                            } else {
                                center_dist - victim_radius
                            };
                            let dist_sqr = boundary_dist * boundary_dist;
                            damage_info.input.amount = if dist_sqr <= primary_range_sqr {
                                primary_damage
                            } else {
                                secondary_damage
                            };
                            damage_info.sync_from_input();
                            let _ = victim.attempt_damage(&mut damage_info);
                        }
                    }

                    if let Some(fx) = self.module_data.geometry_based_damage_fx.as_ref() {
                        let _ = fx.do_fx_at_position_with_radius(&target_pos, secondary_range);
                    }
                }
            }
        }

        if let Some(target_arc) = booby_trapped {
            if let Ok(mut target_guard) = target_arc.write() {
                if let Some(object_arc) = self.object.upgrade() {
                    if let Ok(obj) = object_arc.read() {
                        if obj.is_kind_of(KindOf::BoobyTrap) {
                            target_guard.set_status(ObjectStatusMaskType::BOOBY_TRAPPED, false);
                        }
                    }
                }
            }
        }

        if let Some(object_arc) = self.object.upgrade() {
            if let Ok(mut obj) = object_arc.write() {
                obj.kill(None, None);
            }
        }
    }
}

impl UpdateModuleInterface for StickyBombUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let current_frame = TheGameLogic::get_frame();

        // Check if target is dead - if so, destroy the bomb
        if self.target_id != OBJECT_INVALID_ID {
            if let Some(target) = self.get_target_object() {
                if let Ok(target_guard) = target.read() {
                    if target_guard.is_effectively_dead() {
                        if let Some(object_arc) = self.object.upgrade() {
                            if let Ok(obj) = object_arc.read() {
                                let _ = TheGameLogic::destroy_object(&*obj);
                            }
                        }
                        return Ok(UpdateSleepTime::None);
                    }

                    // Update bomb position to follow target - C++ update()
                    if let Some(object_arc) = self.object.upgrade() {
                        if let Ok(mut obj) = object_arc.write() {
                            let mut new_pos =
                                if target_guard.is_kind_of(crate::common::KindOf::Immobile) {
                                    *obj.get_position()
                                } else {
                                    *target_guard.get_position()
                                };
                            if target_guard.is_kind_of(crate::common::KindOf::Immobile) {
                                if let Some(terrain) = TheTerrainLogic::get() {
                                    new_pos.z =
                                        terrain.get_ground_height(new_pos.x, new_pos.y, None);
                                }
                            } else {
                                new_pos.z += self.module_data.offset_z;
                            }
                            let _ = obj.set_position(&new_pos);
                        }
                    }
                }
            }
        }

        if current_frame >= self.next_ping_frame {
            self.next_ping_frame = self.next_ping_frame.wrapping_add(LOGICFRAMES_PER_SECOND);
            if let Some(obj_arc) = self.object.upgrade() {
                if let Ok(obj) = obj_arc.read() {
                    if let Some(sound) = obj.get_template().get_per_unit_sound("UnitBombPing") {
                        if let Some(audio) = TheAudio::get() {
                            let mut event = sound.clone();
                            event.set_object_id(obj.get_id());
                            audio.add_audio_event(&event);
                        }
                    }
                }
            }
        }

        Ok(UpdateSleepTime::None)
    }
}

impl BehaviorModuleInterface for StickyBombUpdate {
    fn get_module_name(&self) -> &'static str {
        "StickyBombUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_sticky_bomb_control_interface(&mut self) -> Option<&mut dyn StickyBombControlInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let obj = obj_arc.read().ok();
        let Some(obj) = obj else {
            return Ok(());
        };

        let shooter_id = obj.get_producer_id();
        let shooter = TheGameLogic::find_object_by_id(shooter_id);
        if let Some(shooter) = shooter {
            if let Ok(shooter_guard) = shooter.read() {
                if let Some(ai) = shooter_guard.get_ai_update_interface() {
                    if let Some(goal) = ai.get_goal_object() {
                        if let Ok(goal_guard) = goal.read() {
                            self.init_sticky_bomb(Some(&goal_guard), None, None);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl StickyBombControlInterface for StickyBombUpdate {
    fn init_sticky_bomb(&mut self, target_id: ObjectID, bomber_id: ObjectID) {
        self.init_sticky_bomb_by_id(target_id, bomber_id);
    }

    fn detonate(&mut self) {
        StickyBombUpdate::detonate(self);
    }

    fn get_target(&self) -> ObjectID {
        StickyBombUpdate::get_target(self)
    }

    fn set_target_object_id(&mut self, target_id: ObjectID) {
        self.target_id = target_id;
    }
}

impl Snapshotable for StickyBombUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_object_id(&mut self.target_id)
            .map_err(|e| format!("Failed to xfer target_id: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.die_frame)
            .map_err(|e| format!("Failed to xfer die_frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_ping_frame)
            .map_err(|e| format!("Failed to xfer next_ping_frame: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes StickyBombUpdate through the common Module trait.
pub struct StickyBombUpdateModule {
    behavior: StickyBombUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<StickyBombUpdateModuleData>,
}

impl StickyBombUpdateModule {
    pub fn new(
        behavior: StickyBombUpdate,
        module_name: &AsciiString,
        module_data: Arc<StickyBombUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut StickyBombUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for StickyBombUpdateModule {
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

impl Module for StickyBombUpdateModule {
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

    fn get_sticky_bomb_control_interface(&mut self) -> Option<&mut dyn StickyBombControlInterface> {
        Some(self)
    }
}

impl StickyBombControlInterface for StickyBombUpdateModule {
    fn init_sticky_bomb(&mut self, target_id: ObjectID, bomber_id: ObjectID) {
        self.behavior.init_sticky_bomb_by_id(target_id, bomber_id);
    }

    fn detonate(&mut self) {
        self.behavior.detonate();
    }

    fn get_target(&self) -> ObjectID {
        self.behavior.get_target()
    }

    fn set_target_object_id(&mut self, target_id: ObjectID) {
        self.behavior.target_id = target_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_bomb_update_exposes_typed_control_interface() {
        let data = Arc::new(StickyBombUpdateModuleData::default());
        let behavior = StickyBombUpdate {
            object: Weak::new(),
            module_data: data.clone(),
            next_call_frame_and_phase: 0,
            target_id: OBJECT_INVALID_ID,
            die_frame: 0,
            next_ping_frame: 0,
        };
        let mut module =
            StickyBombUpdateModule::new(behavior, &AsciiString::from("StickyBombUpdate"), data);

        let control = module
            .get_sticky_bomb_control_interface()
            .expect("StickyBombUpdate should expose StickyBombControlInterface");
        control.detonate();
        control.init_sticky_bomb(OBJECT_INVALID_ID, OBJECT_INVALID_ID);

        assert_eq!(module.behavior.target_id, OBJECT_INVALID_ID);
    }
}

pub struct StickyBombUpdateFactory;
impl StickyBombUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(StickyBombUpdate::new(thing, module_data)?))
    }
}

pub fn sticky_bomb_update_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = StickyBombUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse StickyBombUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn sticky_bomb_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<StickyBombUpdateModuleData>()
        .expect("StickyBombUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(crate::common::INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("StickyBombUpdate requires object");
    let behavior = StickyBombUpdate::new(object, module_data_arc.clone())
        .expect("StickyBombUpdate failed to initialize");
    let module_name = AsciiString::from("StickyBombUpdate");
    Box::new(StickyBombUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

//! StickyBombUpdate - Sticky bomb that attaches to targets
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, KindOf, LOGICFRAMES_PER_SECOND, ModuleData, ObjectID,
    ObjectStatusMaskType, PlayerMaskType, Real, UnsignedInt,
};
use crate::damage::DamageInfo;
use crate::effects::FXList;
use crate::helpers::{
    TheAudio, TheFXListStore, TheGameLogic, ThePartitionManager, TheTerrainLogic,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{INVALID_ID as OBJECT_INVALID_ID, Object as GameObject};
use crate::weapon::{WeaponBonus, WeaponTemplate, with_weapon_store};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    StickyBombControlInterface, Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

/// Wave 305: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

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
    let token = required_value(tokens)?;
    data.attach_to_bone = AsciiString::from(INI::parse_ascii_string(token)?.as_str());
    Ok(())
}

fn parse_offset_z(
    _ini: &mut INI,
    data: &mut StickyBombUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.offset_z = INI::parse_real(token)?;
    Ok(())
}

fn parse_geometry_based_damage_weapon(
    _ini: &mut INI,
    data: &mut StickyBombUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
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
    let token = required_value(tokens)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.geometry_based_damage_fx = None;
    } else {
        data.geometry_based_damage_fx = TheFXListStore::find_fx_list(token);
    }
    Ok(())
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
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
    object_id: ObjectID,
    module_data: Arc<StickyBombUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    target_id: ObjectID,
    die_frame: UnsignedInt,
    next_ping_frame: UnsignedInt,
}

impl StickyBombUpdate {
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<StickyBombUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let forever = UpdateSleepTime::Forever.to_u32();
        let _ = OBJECT_REGISTRY.with_object(object_id, |obj| {
            obj.reschedule_named_update("StickyBombUpdate", forever);
        });

        Ok(Self {
            object_id: object_id,
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: forever,
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
        // Wave 305: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        self.target_id = target.map(|t| t.get_id()).unwrap_or(OBJECT_INVALID_ID);

        let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj| {
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
                self.next_call_frame_and_phase = now.saturating_add(1);
                obj.reschedule_named_update("StickyBombUpdate", self.next_call_frame_and_phase);

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
                    let created = obj
                        .get_template()
                        .get_per_unit_sound("StickyBombCreated");
                    let _ = obj.set_position(&pos);


                    if let Some(sound) = created {
                        if let Some(audio) = TheAudio::get() {
                            let mut event = sound;
                            event.set_position(&(pos.x, pos.y, pos.z));
                            audio.add_audio_event(&event);
                        }
                    }
                }
        });
    }

    pub fn init_sticky_bomb_by_id(&mut self, target_id: ObjectID, bomber_id: ObjectID) {
        self.init_sticky_bomb_by_id_at(target_id, bomber_id, None);
    }

    pub fn init_sticky_bomb_by_id_at(
        &mut self,
        target_id: ObjectID,
        bomber_id: ObjectID,
        specific_pos: Option<Coord3D>,
    ) {
        let has_target = target_id != OBJECT_INVALID_ID && TheGameLogic::find_object_by_id(target_id);
        let has_bomber = bomber_id != OBJECT_INVALID_ID && TheGameLogic::find_object_by_id(bomber_id);
        let pos = specific_pos;
        match (has_target, has_bomber) {
            (true, true) => {
                let _ = OBJECT_REGISTRY.with_object(target_id, |target| {
                    let _ = OBJECT_REGISTRY.with_object(bomber_id, |bomber| {
                        self.init_sticky_bomb(Some(target), Some(bomber), pos.as_ref());
                    });
                });
            }
            (true, false) => {
                let _ = OBJECT_REGISTRY.with_object(target_id, |target| {
                    self.init_sticky_bomb(Some(target), None, pos.as_ref());
                });
            }
            (false, true) => {
                let _ = OBJECT_REGISTRY.with_object(bomber_id, |bomber| {
                    self.init_sticky_bomb(None, Some(bomber), pos.as_ref());
                });
            }
            (false, false) => self.init_sticky_bomb(None, None, pos.as_ref()),
        }
        if target_id != OBJECT_INVALID_ID {
            let mark = OBJECT_REGISTRY
                .with_object(self.object_id, |obj| obj.is_kind_of(KindOf::BoobyTrap))
                .unwrap_or(false);
            if mark {
                let _ = OBJECT_REGISTRY.with_object_mut(target_id, |target_guard| {
                    target_guard.set_status(ObjectStatusMaskType::BOOBY_TRAPPED, true);
                });
            }
        }
    }

    /// Get the target object this bomb is attached to
    pub fn get_target(&self) -> ObjectID {
        self.target_id
    }

    /// Match C++ getTargetObject().
    pub fn get_target_object(&self) -> Option<ObjectID> {
        if self.target_id == OBJECT_INVALID_ID || !TheGameLogic::find_object_by_id(self.target_id) {
            return None;
        }
        Some(self.target_id)
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
        // Wave 305: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let booby_trapped = self.get_target_object();

        if let Some(template) = self
            .module_data
            .geometry_based_damage_weapon_template
            .as_ref()
        {
            let target_id = *booby_trapped;
            let bomb_present = self.object_id != crate::common::INVALID_ID
                && TheGameLogic::find_object_by_id(self.object_id);
            if let (Some(target_id), true) = (target_id, bomb_present) {
                let bomb_id = self.object_id;
                let blast = OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                    OBJECT_REGISTRY.with_object(bomb_id, |obj| {
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
                        let player_index = obj.get_controlling_player();
                        let source_player_mask = player_index
                            .and_then(|index| {
                                crate::player::with_player(index, |player| player.get_player_mask())
                            })
                            .unwrap_or(PlayerMaskType::none());
                        let mut damage_info = DamageInfo::new();
                        damage_info.input.source_id = obj.get_id();
                        damage_info.input.source_player_mask = source_player_mask;
                        damage_info.input.damage_type = template.damage_type.into();
                        damage_info.input.death_type = template.death_type.into();
                        damage_info.input.damage_status_type = template.damage_status_type.into();
                        (
                            *target_guard.get_position(),
                            primary_damage,
                            secondary_damage,
                            primary_range,
                            secondary_range,
                            damage_info,
                        )
                    })
                });
                if let Some(Some((
                    target_pos,
                    primary_damage,
                    secondary_damage,
                    primary_range,
                    secondary_range,
                    mut damage_info,
                ))) = blast
                {
                    let primary_range_sqr = primary_range * primary_range;
                    let radius = primary_range.max(secondary_range);
                    if let Some(partition) = ThePartitionManager::get() {
                        for id in partition.get_objects_in_range_boundary_3d(&target_pos, radius) {
                            if !TheGameLogic::find_object_by_id(id) {
                                continue;
                            }
                            let _ = OBJECT_REGISTRY.with_object_mut(id, |victim| {
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
                            });
                        }
                    }
                    if let Some(fx) = self.module_data.geometry_based_damage_fx.as_ref() {
                        let _ = fx.do_fx_at_position_with_radius(&target_pos, secondary_range);
                    }
                }
            }
        }

        if let Some(target_id) = booby_trapped {
            let bomb_id = self.object_id;
            let is_trap = OBJECT_REGISTRY
                .with_object(bomb_id, |obj| obj.is_kind_of(KindOf::BoobyTrap))
                .unwrap_or(false);
            if is_trap {
                let _ = OBJECT_REGISTRY.with_object_mut(target_id, |target_guard| {
                    target_guard.set_status(ObjectStatusMaskType::BOOBY_TRAPPED, false);
                });
            }
        }

        let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj| {
            obj.kill(None, None);
        });
    }
}

impl UpdateModuleInterface for StickyBombUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        // Wave 305: empty dual-world → Forever.
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::Forever);
        }

        let current_frame = TheGameLogic::get_frame();

        // Check if target is dead - if so, destroy the bomb
        if self.target_id != OBJECT_INVALID_ID {
            if let Some(target_id) = self.get_target_object() {
                let follow = OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                    (
                        target_guard.is_effectively_dead(),
                        target_guard.is_kind_of(crate::common::KindOf::Immobile),
                        *target_guard.get_position(),
                    )
                });
                if let Some((dead, immobile, target_pos)) = follow {
                    let bomb_present = self.object_id != crate::common::INVALID_ID
                        && TheGameLogic::find_object_by_id(self.object_id);
                    if dead {
                        if bomb_present {
                            let _ = TheGameLogic::destroy_object_by_id(self.object_id);
                        }
                        return Ok(UpdateSleepTime::None);
                    }
                    if bomb_present {
                        let offset_z = self.module_data.offset_z;
                        let _ = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj| {
                            let mut new_pos = if immobile {
                                *obj.get_position()
                            } else {
                                target_pos
                            };
                            if immobile {
                                if let Some(terrain) = TheTerrainLogic::get() {
                                    new_pos.z = terrain.get_ground_height(new_pos.x, new_pos.y, None);
                                }
                            } else {
                                new_pos.z += offset_z;
                            }
                            let _ = obj.set_position(&new_pos);
                        });
                    }
                }
            }
        }

        if current_frame >= self.next_ping_frame {
            self.next_ping_frame = self.next_ping_frame.wrapping_add(LOGICFRAMES_PER_SECOND);
            let _ = OBJECT_REGISTRY.with_object(self.object_id, |obj| {
                if let Some(sound) = obj.get_template().get_per_unit_sound("UnitBombPing") {
                    if let Some(audio) = TheAudio::get() {
                        let mut event = sound.clone();
                        event.set_object_id(obj.get_id());
                        audio.add_audio_event(&event);
                    }
                }
            });
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
        // Wave 305: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let shooter_id = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.object_id, |obj| obj.get_producer_id())
            .flatten();
        let Some(shooter_id) = shooter_id else {
            return Ok(());
        };
        let goal_id = crate::object::registry::OBJECT_REGISTRY.with_object(shooter_id, |shooter| {
            shooter
                .get_ai_update_interface()
                .map(|ai| ai.get_goal_object_id())
        });
        let goal_id = goal_id.flatten().filter(|id| *id != crate::common::INVALID_ID);
        if let Some(goal_id) = goal_id {
            self.init_sticky_bomb_by_id(goal_id, crate::common::INVALID_ID);
        }
        Ok(())
    }
}

impl StickyBombControlInterface for StickyBombUpdate {
    fn init_sticky_bomb(&mut self, target_id: ObjectID, bomber_id: ObjectID) {
        self.init_sticky_bomb_by_id(target_id, bomber_id);
    }
    fn init_sticky_bomb_at(
        &mut self,
        target_id: ObjectID,
        bomber_id: ObjectID,
        x: f32,
        y: f32,
        z: f32,
    ) {
        self.init_sticky_bomb_by_id_at(target_id, bomber_id, Some(Coord3D::new(x, y, z)));
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

    fn is_timed_bomb(&self) -> bool {
        StickyBombUpdate::is_timed_bomb(self)
    }

    fn get_detonation_frame(&self) -> u32 {
        StickyBombUpdate::get_detonation_frame(self)
    }
}

impl Snapshotable for StickyBombUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
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
    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.behavior.next_call_frame_and_phase
    }

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
    fn init_sticky_bomb_at(
        &mut self,
        target_id: ObjectID,
        bomber_id: ObjectID,
        x: f32,
        y: f32,
        z: f32,
    ) {
        self.behavior
            .init_sticky_bomb_by_id_at(target_id, bomber_id, Some(Coord3D::new(x, y, z)));
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

    fn is_timed_bomb(&self) -> bool {
        self.behavior.is_timed_bomb()
    }

    fn get_detonation_frame(&self) -> u32 {
        self.behavior.get_detonation_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticky_bomb_update_exposes_typed_control_interface() {
        let data = Arc::new(StickyBombUpdateModuleData::default());
        let behavior = StickyBombUpdate {
            object_id: crate::common::INVALID_ID,
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

    #[test]
    fn sticky_bomb_fields_use_cpp_ini_token_handling() {
        let mut ini = INI::new();
        let mut data = StickyBombUpdateModuleData::default();

        parse_attach_to_bone(&mut ini, &mut data, &["=", "BOMB_BONE"]).unwrap();
        parse_offset_z(&mut ini, &mut data, &["=", "14.5"]).unwrap();
        parse_geometry_based_damage_weapon(&mut ini, &mut data, &["=", "DemoTrapDetonationWeapon"])
            .unwrap();
        parse_geometry_based_damage_fx(&mut ini, &mut data, &["=", "NONE"]).unwrap();

        assert_eq!(data.attach_to_bone.as_str(), "BOMB_BONE");
        assert_eq!(data.offset_z, 14.5);
        assert!(data.geometry_based_damage_weapon_template.is_none());
        assert!(data.geometry_based_damage_fx.is_none());
    }

    #[test]
    fn sticky_bomb_rejects_missing_values_like_cpp_parsers() {
        let mut ini = INI::new();
        let mut data = StickyBombUpdateModuleData::default();

        assert!(matches!(
            parse_attach_to_bone(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_offset_z(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_geometry_based_damage_weapon(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_geometry_based_damage_fx(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
    }
}

pub struct StickyBombUpdateFactory;
impl StickyBombUpdateFactory {
    pub fn create_behavior(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(StickyBombUpdate::new(object_id, module_data)?))
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
    let behavior = StickyBombUpdate::new(object_id, module_data_arc.clone())
        .expect("StickyBombUpdate failed to initialize");
    let module_name = AsciiString::from("StickyBombUpdate");
    Box::new(StickyBombUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}

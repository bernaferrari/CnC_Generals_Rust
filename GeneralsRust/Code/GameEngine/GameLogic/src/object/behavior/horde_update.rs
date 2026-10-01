//! HordeUpdate - Rust conversion of C++ HordeUpdate
//!
//! Horde mechanics for GLA units.
//! Author: Steven Johnson, Feb 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Bool, Int, KIND_OF_MASK_NONE, KindOf, KindOfMaskType, ModuleData, ObjectID, Real,
    UnsignedInt, WeaponBonusConditionFlags,
};
use crate::common::{FROM_CENTER_2D, GameLogicRandomValue, LOGICFRAMES_PER_SECOND};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use crate::object::draw::draw_module::TerrainDecalType;
use crate::object::drawable::DrawableArcExt;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::Arc;

/// Host-only leftover ticks still drive membership when the dual-world
/// factory registry is empty. The live module must keep running (C++
/// `HordeUpdate::update` never Forever-sleeps on an empty client list).

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HordeActionType {
    Horde = 0,
}

#[derive(Clone, Debug)]
pub struct HordeUpdateModuleData {
    pub base: BehaviorModuleData,
    pub update_rate: UnsignedInt,
    pub kindof: KindOfMaskType,
    pub min_count: Int,
    pub min_dist: Real,
    pub allies_only: Bool,
    pub exact_match: Bool,
    pub rub_off_radius: Real,
    pub action: HordeActionType,
    pub allowed_nationalism: Bool,
    pub flag_sub_obj_names: Vec<String>,
}

impl Default for HordeUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            update_rate: LOGICFRAMES_PER_SECOND as UnsignedInt,
            kindof: 0,
            min_count: 0,
            min_dist: 0.0,
            allies_only: true,
            exact_match: false,
            rub_off_radius: 20.0,
            action: HordeActionType::Horde,
            allowed_nationalism: true,
            flag_sub_obj_names: Vec::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(HordeUpdateModuleData, base);

impl HordeUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HORDE_UPDATE_FIELDS)
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_int(tokens: &[&str]) -> Result<Int, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    token.parse::<Int>().map_err(|_| INIError::InvalidData)
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_bool(tokens: &[&str]) -> Result<Bool, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_bool(token)
}

pub fn horde_terrain_decal_type(
    is_infantry: bool,
    has_nationalism: bool,
    has_fanaticism: bool,
) -> TerrainDecalType {
    if has_nationalism {
        if has_fanaticism {
            TerrainDecalType::HordeWithFanaticismUpgrade
        } else {
            TerrainDecalType::HordeWithNationalismUpgrade
        }
    } else if is_infantry {
        TerrainDecalType::Horde
    } else {
        TerrainDecalType::HordeVehicle
    }
}

/// C++ `W3DModelDraw::setTerrainDecal` sizes infantry rings from ShadowSize.
pub fn leftover_infantry_horde_decal_size(shadow_size_x: Real, shadow_size_y: Real) -> Real {
    if shadow_size_x > 0.0 {
        shadow_size_x
    } else {
        shadow_size_y.max(0.0)
    }
}

/// C++ `HordeUpdate.cpp:253` vehicle membership gate (`frame > last + UpdateRate`).
pub fn leftover_vehicle_horde_membership_due(
    current_frame: UnsignedInt,
    last_horde_refresh_frame: UnsignedInt,
    update_rate: UnsignedInt,
) -> bool {
    current_frame > last_horde_refresh_frame.saturating_add(update_rate)
}

/// C++ `HordeUpdate.cpp:146-147` constructor `UPDATE_SLEEP(GameLogicRandomValue(1, delay))`.
pub fn leftover_horde_first_wake_delay(update_rate: UnsignedInt) -> UnsignedInt {
    let delay = update_rate.max(1) as i32;
    GameLogicRandomValue(1, delay).max(1) as UnsignedInt
}

/// C++ `HordeUpdate.cpp:362-365` join/leave fade. `None` when membership did not change.
fn horde_terrain_decal_fade(was_in_horde: bool, now_in_horde: bool) -> Option<(Real, Real)> {
    if !was_in_horde && now_in_horde {
        Some((1.0, 0.03))
    } else if was_in_horde && !now_in_horde {
        Some((0.0, -0.03))
    } else {
        None
    }
}

/// C++ `HordeUpdate.cpp:369` — infantry `UPDATE_SLEEP(UpdateRate)`, vehicle `UPDATE_SLEEP_NONE`.
/// Empty `OBJECT_REGISTRY` is not a reason to Forever-sleep.
fn horde_update_sleep_after_tick(is_infantry: bool, update_rate: UnsignedInt) -> UpdateSleepTime {
    if is_infantry {
        UpdateSleepTime::from_u32(update_rate)
    } else {
        UpdateSleepTime::None
    }
}

fn parse_update_rate(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.update_rate = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_kindof(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.kindof = crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_min_count(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_count = parse_int(tokens)?;
    Ok(())
}

fn parse_min_dist(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_dist = parse_real(tokens)?;
    Ok(())
}

fn parse_rub_off_radius(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.rub_off_radius = parse_real(tokens)?;
    Ok(())
}

fn parse_allies_only(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.allies_only = parse_bool(tokens)?;
    Ok(())
}

fn parse_exact_match(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.exact_match = parse_bool(tokens)?;
    Ok(())
}

fn parse_allowed_nationalism(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.allowed_nationalism = parse_bool(tokens)?;
    Ok(())
}

fn parse_action(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    let idx =
        INI::parse_index_list(token, HORDE_ACTION_NAMES).map_err(|_| INIError::InvalidData)?;
    data.action = HORDE_ACTION_TYPES
        .get(idx)
        .copied()
        .unwrap_or(HordeActionType::Horde);
    Ok(())
}

fn parse_flag_sub_obj_names(
    _ini: &mut INI,
    data: &mut HordeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.flag_sub_obj_names = tokens
        .iter()
        .copied()
        .filter(|t| *t != "=")
        .map(|t| t.to_string())
        .collect();
    Ok(())
}

const HORDE_ACTION_TYPES: &[HordeActionType] = &[HordeActionType::Horde];
const HORDE_ACTION_NAMES: &[&str] = &["HORDE"];

const HORDE_UPDATE_FIELDS: &[FieldParse<HordeUpdateModuleData>] = &[
    FieldParse {
        token: "UpdateRate",
        parse: parse_update_rate,
    },
    FieldParse {
        token: "KindOf",
        parse: parse_kindof,
    },
    FieldParse {
        token: "Count",
        parse: parse_min_count,
    },
    FieldParse {
        token: "Radius",
        parse: parse_min_dist,
    },
    FieldParse {
        token: "RubOffRadius",
        parse: parse_rub_off_radius,
    },
    FieldParse {
        token: "AlliesOnly",
        parse: parse_allies_only,
    },
    FieldParse {
        token: "ExactMatch",
        parse: parse_exact_match,
    },
    FieldParse {
        token: "Action",
        parse: parse_action,
    },
    FieldParse {
        token: "FlagSubObjectNames",
        parse: parse_flag_sub_obj_names,
    },
    FieldParse {
        token: "AllowedNationalism",
        parse: parse_allowed_nationalism,
    },
];

pub struct HordeUpdate {
    object_id: ObjectID,
    module_data: Arc<HordeUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    last_horde_refresh_frame: UnsignedInt,
    in_horde: Bool,
    true_horde_member: Bool,
    has_flag: Bool,
}

impl HordeUpdate {
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<HordeUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let mut instance = Self {
            object_id: object_id,
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            last_horde_refresh_frame: TheGameLogic::get_frame(),
            in_horde: false,
            true_horde_member: false,
            has_flag: false,
        };
        instance.arm_initial_wake();

        Ok(instance)
    }

    pub fn new_from_object_handle(
        object_id: ObjectID,
        module_data: Arc<HordeUpdateModuleData>,
    ) -> Self {
        let mut instance = Self {
            object_id: object_id,
            module_data,
            next_call_frame_and_phase: 0,
            last_horde_refresh_frame: TheGameLogic::get_frame(),
            in_horde: false,
            true_horde_member: false,
            has_flag: false,
        };
        instance.arm_initial_wake();

        instance
    }

    fn arm_initial_wake(&mut self) {
        let delay = self.module_data.update_rate;
        if delay > 0 {
            let wake = GameLogicRandomValue(1, delay as i32) as u32;
            let now = TheGameLogic::get_frame();
            self.next_call_frame_and_phase = now.saturating_add(wake);
        }
    }

    pub fn is_in_horde(&self) -> Bool {
        self.in_horde
    }

    pub fn is_true_horde_member(&self) -> Bool {
        self.true_horde_member && self.in_horde
    }

    pub fn has_flag(&self) -> Bool {
        self.has_flag
    }

    pub fn is_allowed_nationalism(&self) -> Bool {
        self.module_data.allowed_nationalism
    }

    fn show_hide_flag(&mut self, show: Bool) {
        // C++ showHideFlag always walks FlagSubObjectNames when present.
        self.has_flag = show;
        if self.module_data.flag_sub_obj_names.is_empty() {
            return;
        }
        let object_id = self.object_id;
        if object_id == crate::common::INVALID_ID {
            return;
        }
        let module_data = Arc::clone(&self.module_data);
        let _ = OBJECT_REGISTRY.with_object(object_id, |obj| {
            let Some(drawable) = obj.get_drawable() else {
                return;
            };
            let Ok(mut draw_guard) = drawable.write() else {
                return;
            };
            for name in &module_data.flag_sub_obj_names {
                draw_guard.show_sub_object(name, show);
            }
            draw_guard.update_sub_objects();
        });
    }

    fn check_horde_status(&mut self) {
        // C++ PartitionFilterHordeMember + radius scan always run.
        // Owner stays checked out for the scan so other ids can be borrowed
        // alongside it; same-id re-entry would miss the checked-out owner.
        if self.object_id == crate::common::INVALID_ID {
            return;
        }
        let Some(partition) = ThePartitionManager::get() else {
            return;
        };

        let object_id = self.object_id;
        let exact_match = self.module_data.exact_match;
        let kindof = self.module_data.kindof;
        let allies_only = self.module_data.allies_only;
        let min_dist = self.module_data.min_dist;
        let rub_off_radius_sq = self.module_data.rub_off_radius * self.module_data.rub_off_radius;
        let required = self.module_data.min_count - 1;

        let Some(horde_candidates) = OBJECT_REGISTRY.with_object(object_id, |obj| {
            let owner_id = obj.get_id();
            let mut horde_candidates = Vec::new();
            for id in partition.get_objects_in_range_boundary_3d_from_object(obj, min_dist) {
                if id == owner_id {
                    continue;
                }
                let passes = OBJECT_REGISTRY
                    .with_object(id, |other| {
                        if exact_match && obj.get_template_name() != other.get_template_name() {
                            return false;
                        }

                        let mut has_horde = false;
                        other.with_horde_update_interface(|_| {
                            has_horde = true;
                        });
                        if !has_horde {
                            return false;
                        }

                        if !other.is_kind_of_multi(kindof, KIND_OF_MASK_NONE) {
                            return false;
                        }

                        if allies_only {
                            let relationship = obj.relationship_to(other);
                            if !matches!(relationship, crate::common::Relationship::Allies) {
                                return false;
                            }
                        }

                        if obj.is_off_map() != other.is_off_map() {
                            return false;
                        }

                        true
                    })
                    .unwrap_or(false);
                if passes {
                    horde_candidates.push(id);
                }
            }
            horde_candidates
        }) else {
            return;
        };

        if required <= 0 || horde_candidates.len() as Int >= required {
            self.in_horde = true;
            self.true_horde_member = true;
            return;
        }

        self.in_horde = false;
        self.true_horde_member = false;

        let joined_by_rub_off = OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                for id in &horde_candidates {
                    let near_true_member = OBJECT_REGISTRY
                        .with_object(*id, |other| {
                            let mut is_true = false;
                            other.with_horde_update_interface(|hui| {
                                if hui.is_true_horde_member() {
                                    is_true = true;
                                }
                            });
                            if !is_true {
                                return false;
                            }
                            let dist_sq = ThePartitionManager::get_distance_squared(
                                obj,
                                other,
                                FROM_CENTER_2D,
                            );
                            dist_sq <= rub_off_radius_sq
                        })
                        .unwrap_or(false);
                    if near_true_member {
                        return true;
                    }
                }
                false
            })
            .unwrap_or(false);
        if joined_by_rub_off {
            self.in_horde = true;
        }
    }
}

impl UpdateModuleInterface for HordeUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // C++ HordeUpdate::update always runs membership + decals.
        // Missing owner is UPDATE_SLEEP_FOREVER (HordeUpdate.cpp:242-243).
        // An empty OBJECT_REGISTRY is not itself a Forever-sleep reason.
        if self.object_id == crate::common::INVALID_ID {
            return UpdateSleepTime::Forever;
        }

        let current_frame = TheGameLogic::get_frame();
        let object_id = self.object_id;
        let update_rate = self.module_data.update_rate;
        let was_in_horde = self.in_horde;
        let Some(is_infantry) = OBJECT_REGISTRY.with_object(object_id, |obj| {
            obj.is_kind_of(crate::common::KindOf::Infantry)
        }) else {
            // C++ FOREVER is only a null owning object. An empty factory
            // store must keep waking; Forever here would stick the module
            // asleep for the rest of the match.
            if OBJECT_REGISTRY.store_is_empty() {
                return horde_update_sleep_after_tick(false, update_rate);
            }
            return UpdateSleepTime::Forever;
        };

        if is_infantry || current_frame > self.last_horde_refresh_frame + update_rate {
            self.last_horde_refresh_frame = current_frame;
            self.check_horde_status();
            // evaluateMoraleBonus needs &mut AI, still ahead of the icon pass.
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_mut| {
                if let Some(ai) = obj_mut.get_ai_update_interface_mut() {
                    let _ = ai.evaluate_morale_bonus();
                }
            });
        }

        let in_horde = self.in_horde;
        let decals_applied = OBJECT_REGISTRY.with_object(object_id, |obj| {
            if let Some(drawable) = obj.get_drawable() {
                if !obj.is_effectively_dead() {
                    let draw_icon_ui = TheGameLogic::get_draw_icon_ui();
                    let is_portable_structure = obj.is_kind_of(KindOf::PortableStructure);
                    let bonus_flags = obj.get_weapon_bonus_condition();
                    let has_nationalism =
                        bonus_flags.contains(WeaponBonusConditionFlags::NATIONALISM);
                    let has_fanaticism =
                        bonus_flags.contains(WeaponBonusConditionFlags::FANATICISM);

                    if draw_icon_ui {
                        if in_horde && !is_portable_structure {
                            let decal_type = if is_infantry {
                                horde_terrain_decal_type(true, has_nationalism, has_fanaticism)
                            } else {
                                let geom = obj.get_geometry_info();
                                let size =
                                    3.5 * ((geom.bounds.max.x - geom.bounds.min.x).abs() * 0.5);
                                drawable.set_terrain_decal_size(size, size);
                                horde_terrain_decal_type(false, has_nationalism, has_fanaticism)
                            };

                            drawable.set_terrain_decal(decal_type);
                        }
                    } else {
                        drawable.set_terrain_decal(TerrainDecalType::None);
                    }

                    if let Some((target, rate)) = horde_terrain_decal_fade(was_in_horde, in_horde) {
                        drawable.set_terrain_decal_fade_target(target, rate);
                    }
                }
            }
        });
        if decals_applied.is_none() {
            return UpdateSleepTime::Forever;
        }

        horde_update_sleep_after_tick(is_infantry, update_rate)
    }
}

impl Snapshotable for HordeUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_bool(&mut self.in_horde)
            .map_err(|e| format!("Failed to xfer in_horde: {:?}", e))?;
        xfer.xfer_bool(&mut self.has_flag)
            .map_err(|e| format!("Failed to xfer has_flag: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
impl BehaviorModuleInterface for HordeUpdate {
    fn get_module_name(&self) -> &'static str {
        "HordeUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_horde_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::HordeUpdateInterface> {
        Some(self)
    }
}

impl crate::modules::HordeUpdateInterface for HordeUpdate {
    fn is_true_horde_member(&self) -> bool {
        self.is_true_horde_member()
    }

    fn is_in_horde(&self) -> bool {
        self.is_in_horde()
    }

    fn is_allowed_nationalism(&self) -> bool {
        self.is_allowed_nationalism()
    }

    fn has_flag(&self) -> bool {
        self.has_flag()
    }
}

/// Glue that exposes HordeUpdate through the common Module trait.
pub struct HordeUpdateModule {
    behavior: HordeUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<HordeUpdateModuleData>,
}

impl HordeUpdateModule {
    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.behavior.next_call_frame_and_phase
    }

    pub fn new(
        behavior: HordeUpdate,
        module_name: &AsciiString,
        module_data: Arc<HordeUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut HordeUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for HordeUpdateModule {
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

impl Module for HordeUpdateModule {
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

    fn on_drawable_bound_to_object(&mut self) {
        // C++ HordeUpdate::onDrawableBoundToObject — always hide leftover flags.
        self.behavior.show_hide_flag(false);
    }
}

pub struct HordeUpdateFactory;
impl HordeUpdateFactory {
    pub fn create_behavior(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HordeUpdate::new(object_id, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horde_decal_selection_matches_cpp_bonus_nesting() {
        assert_eq!(
            horde_terrain_decal_type(true, false, false),
            TerrainDecalType::Horde
        );
        assert_eq!(
            horde_terrain_decal_type(true, true, false),
            TerrainDecalType::HordeWithNationalismUpgrade
        );
        assert_eq!(
            horde_terrain_decal_type(true, false, true),
            TerrainDecalType::Horde
        );
        assert_eq!(
            horde_terrain_decal_type(true, true, true),
            TerrainDecalType::HordeWithFanaticismUpgrade
        );

        assert_eq!(
            horde_terrain_decal_type(false, false, false),
            TerrainDecalType::HordeVehicle
        );
        assert_eq!(
            horde_terrain_decal_type(false, true, false),
            TerrainDecalType::HordeWithNationalismUpgrade
        );
        assert_eq!(
            horde_terrain_decal_type(false, false, true),
            TerrainDecalType::HordeVehicle
        );
        assert_eq!(
            horde_terrain_decal_type(false, true, true),
            TerrainDecalType::HordeWithFanaticismUpgrade
        );
    }

    #[test]
    fn horde_decal_fade_matches_cpp_join_leave() {
        // C++ HordeUpdate.cpp:362-365
        assert_eq!(horde_terrain_decal_fade(false, true), Some((1.0, 0.03)));
        assert_eq!(horde_terrain_decal_fade(true, false), Some((0.0, -0.03)));
        assert_eq!(horde_terrain_decal_fade(true, true), None);
        assert_eq!(horde_terrain_decal_fade(false, false), None);
    }

    #[test]
    fn horde_update_sleep_never_forever_because_registry_empty() {
        // C++ HordeUpdate.cpp:369 — infantry UpdateRate, vehicle UPDATE_SLEEP_NONE.
        // Constructor only random-sleeps the first wake (`:146-147`). Empty
        // OBJECT_REGISTRY is not a Forever-sleep reason.
        assert_ne!(
            horde_update_sleep_after_tick(true, 30),
            UpdateSleepTime::Forever
        );
        assert_eq!(
            horde_update_sleep_after_tick(false, 30),
            UpdateSleepTime::None
        );
        assert_eq!(
            horde_update_sleep_after_tick(true, 30),
            UpdateSleepTime::from_u32(30)
        );
    }

    #[test]
    fn infantry_decal_size_uses_shadow_size_not_invented_40() {
        assert_eq!(leftover_infantry_horde_decal_size(14.0, 12.0), 14.0);
        assert_eq!(leftover_infantry_horde_decal_size(0.0, 9.0), 9.0);
        assert_eq!(leftover_infantry_horde_decal_size(0.0, 0.0), 0.0);
    }

    #[test]
    fn vehicle_membership_due_matches_cpp_update_rate_gate() {
        assert!(!leftover_vehicle_horde_membership_due(0, 0, 30));
        assert!(!leftover_vehicle_horde_membership_due(30, 0, 30));
        assert!(leftover_vehicle_horde_membership_due(31, 0, 30));
        let delay = leftover_horde_first_wake_delay(30);
        assert!(delay >= 1 && delay <= 30);
    }

    #[test]
    fn on_drawable_bound_hides_flag_subobjects() {
        let mut horde = HordeUpdate {
            object_id: crate::common::INVALID_ID,
            module_data: Arc::new(HordeUpdateModuleData {
                flag_sub_obj_names: vec!["FLAG".to_string(), "HordeFlag".to_string()],
                ..HordeUpdateModuleData::default()
            }),
            next_call_frame_and_phase: 0,
            last_horde_refresh_frame: 0,
            in_horde: false,
            true_horde_member: false,
            has_flag: true,
        };
        horde.show_hide_flag(false);
        assert!(!horde.has_flag());
        let mut module = HordeUpdateModule {
            behavior: horde,
            module_name_key: 0,
            module_data: Arc::new(HordeUpdateModuleData::default()),
        };
        module.behavior.has_flag = true;
        Module::on_drawable_bound_to_object(&mut module);
        assert!(!module.behavior.has_flag());
        assert!(!crate::modules::HordeUpdateInterface::has_flag(
            &module.behavior
        ));
    }
}

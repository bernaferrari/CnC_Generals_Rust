//! Helper modules for game logic system
//!
//! This module provides helper functionality for various game systems,
//! matching the C++ helper architecture.

use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};
use crate::common::audio::TimeOfDay;
use crate::common::types::{
    EmissionVolumeType, FXListManagerInterface, ParticleSystemManagerInterface,
};
use crate::common::Matrix3D;
use crate::common::{
    AsciiString, Bool, Color, Coord3D, DisabledMaskType, DisabledType, DistanceType, FXListId, Int,
    KindOf, MessageType, NameKeyGenerator, NameKeyType, ObjectID, PathfindLayerEnum,
    PlayerMaskType, Real, Relationship, UnsignedInt, VeterancyLevel, DISABLED_COUNT, INVALID_ID,
    NEVER,
};
use crate::effects::{FXList, ObjectCreationList};
use crate::error::GameLogicError as GameError;
use crate::modules::UpdateModulePtr;
use crate::object::collide::crate_collide::AudioEvent;
use crate::object::draw::w3d_laser_draw::W3DLaserDrawModuleData;
use crate::object::draw::w3d_tree_draw::W3DTreeDrawModuleData;
use crate::object::drawable::{Drawable, DrawableArcExt, DrawableThingHandle, DrawableType};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::AudioEventRts;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::Object;
use crate::weapon::WeaponBonusSet;
use game_engine::common::audio::audio_event_rts::{
    register_audio_event_owner_resolver, AudioEventOwnerResolver,
};
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager, register_audio_locality_resolver,
    register_audio_view_resolver, AudioLocalityRelationship, AudioLocalityResolver,
    AudioViewResolver,
};
use game_engine::common::audio::game_sounds::{
    register_audio_shroud_resolver, AudioShroudResolver,
};
use game_engine::common::audio::{
    AudioAffect as EngineAudioAffect, AudioEventRts as EngineAudioEventRts,
    Coord3D as EngineCoord3D, TimeOfDay as EngineTimeOfDay,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_game_data::ensure_global_data as ensure_engine_global_data;
use game_engine::common::ini::{
    get_global_data as get_engine_global_data, TimeOfDay as IniTimeOfDay,
};
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::radar::get_radar_system;
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleInterfaceType, ModuleType, Thing as ModuleThing,
};
use game_engine::common::thing::module_factory::get_module_factory;
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use game_engine::common::thing::thing_template::BuildCompletionType;
use game_engine::common::thing::thing_template::{
    ModuleDescriptorSet, ThingTemplate as EngineThingTemplate,
};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

struct EngineThingTemplateAdapter {
    inner: Arc<EngineThingTemplate>,
    name: crate::common::AsciiString,
    geometry: crate::common::GeometryInfo,
    kindof_mask: u64,
    behavior_modules: Vec<crate::common::TemplateModuleInfo>,
    draw_modules: Vec<crate::common::TemplateModuleInfo>,
    client_update_modules: Vec<crate::common::TemplateModuleInfo>,
    command_set_string: crate::common::AsciiString,
}

impl std::fmt::Debug for EngineThingTemplateAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineThingTemplateAdapter")
            .field("name", &self.name)
            .field("template_id", &self.inner.get_template_id())
            .finish()
    }
}

impl EngineThingTemplateAdapter {
    fn convert_module_info(
        info: &game_engine::common::thing::thing_template::ModuleInfo,
    ) -> Vec<crate::common::TemplateModuleInfo> {
        info.iter()
            .map(|entry| crate::common::TemplateModuleInfo {
                name: entry.name.clone().into(),
                module_tag: entry.module_tag.clone().into(),
                data: Arc::clone(entry.data),
                interface_mask: entry.interface_flags(),
            })
            .collect()
    }

    fn new(inner: Arc<EngineThingTemplate>) -> Self {
        let name = crate::common::AsciiString::from(inner.get_name().as_str());
        let command_set_string =
            crate::common::AsciiString::from(inner.get_command_set_string().as_str());
        let geo = inner.get_template_geometry_info();
        let half_w = (geo.width.max(0.0) * 0.5) as f32;
        let half_d = (geo.depth.max(0.0) * 0.5) as f32;
        let height = geo.height.max(0.0) as f32;

        let geometry = crate::common::GeometryInfo {
            position: crate::common::Coord3D::ZERO,
            angle: 0.0,
            bounds: crate::common::AABox {
                min: crate::common::Coord3D::new(-half_w, -half_d, 0.0),
                max: crate::common::Coord3D::new(half_w, half_d, height),
            },
            height_above_terrain: 0.0,
            geometry_type: geo.geometry_type,
            is_small: geo.is_small,
        };

        Self {
            kindof_mask: inner.get_kindof_mask(),
            behavior_modules: Self::convert_module_info(inner.get_behavior_module_info()),
            draw_modules: Self::convert_module_info(inner.get_draw_module_info()),
            client_update_modules: Self::convert_module_info(inner.get_client_update_module_info()),
            inner,
            name,
            geometry,
            command_set_string,
        }
    }

    fn build_facility_context(
        &self,
        player: &crate::player::Player,
    ) -> Option<crate::object::production::build_cost_calculator::BuildFacilityContext> {
        if self.inner.get_build_completion() != BuildCompletionType::AppearsAtRallyPoint {
            return None;
        }

        if self.inner.get_prereq_count() == 0 {
            return None;
        }

        let prereq = self.inner.get_prereq(0)?;
        let candidates = prereq.get_all_possible_build_facility_templates(32);
        if candidates.is_empty() {
            return None;
        }

        for handle in candidates {
            let Some(template) = TheThingFactory::find_template_by_id(handle.value()) else {
                continue;
            };

            let mut counts = [0i32; 1];
            player.count_objects_by_thing_template(&[template], false, false, &mut counts);
            if counts[0] > 0 {
                return Some(
                    crate::object::production::build_cost_calculator::BuildFacilityContext {
                        facility_count: counts[0],
                        appears_at_rally_point: true,
                    },
                );
            }
        }

        None
    }

    fn kind_index(kind: crate::common::KindOf) -> Option<u32> {
        match kind {
            crate::common::KindOf::Prison => Some(15),
            crate::common::KindOf::CollectsPrisonBounty => Some(16),
            crate::common::KindOf::PowTruck => Some(17),
            crate::common::KindOf::RepairPad => Some(31),
            crate::common::KindOf::CashGenerator => Some(34),
            crate::common::KindOf::RebuildHole => Some(37),
            crate::common::KindOf::CanRappel => Some(42),
            crate::common::KindOf::CanSurrender => Some(44),
            crate::common::KindOf::MobNexus => Some(46),
            crate::common::KindOf::IgnoredInGui => Some(47),
            crate::common::KindOf::Capturable => Some(49),
            crate::common::KindOf::FSTechnology => Some(64),
            crate::common::KindOf::ImmuneToCapture => Some(80),
            crate::common::KindOf::NoGarrison => Some(27),
            crate::common::KindOf::Powered => Some(70),
            crate::common::KindOf::GarrisonableUntilDestroyed => Some(78),
            _ => crate::common::ALL_KIND_OF
                .iter()
                .position(|k| *k == kind)
                .map(|idx| idx as u32),
        }
    }
}

impl crate::common::ThingTemplate for EngineThingTemplateAdapter {
    fn get_name(&self) -> &crate::common::AsciiString {
        &self.name
    }

    fn get_template_geometry_info(&self) -> crate::common::GeometryInfo {
        self.geometry.clone()
    }

    fn get_template_geometry_type(&self) -> Option<game_engine::system::geometry::GeometryType> {
        Some(self.inner.get_template_geometry_info().geometry_type)
    }

    fn calc_vision_range(&self) -> crate::common::Real {
        self.inner.calc_vision_range()
    }

    fn calc_shroud_clearing_range(&self) -> crate::common::Real {
        self.inner.calc_shroud_clearing_range()
    }

    fn is_enter_guard(&self) -> bool {
        self.inner.is_enter_guard()
    }

    fn is_hijack_guard(&self) -> bool {
        self.inner.is_hijack_guard()
    }

    fn is_build_facility(&self) -> bool {
        self.inner.is_build_facility()
    }

    fn get_command_set_string(&self) -> &crate::common::AsciiString {
        &self.command_set_string
    }

    fn get_energy_production(&self) -> crate::common::Int {
        self.inner.get_energy_production()
    }

    fn get_energy_bonus(&self) -> crate::common::Int {
        self.inner.get_energy_bonus()
    }

    fn get_per_unit_sound(&self, name: &str) -> Option<crate::common::audio::AudioEventRts> {
        let key = game_engine::common::rts::AsciiString::from(name);
        let sound = self.inner.get_per_unit_sound(&key)?;
        let event_name = if !sound.event_name.is_empty() {
            sound.event_name.clone()
        } else {
            sound.filename_to_load.clone()
        };
        Some(crate::common::audio::AudioEventRts::new(event_name))
    }

    fn get_voice_attack(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_voice_attack()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn get_voice_attack_special(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_voice_attack_special()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn get_voice_attack_air(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_voice_attack_air()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn get_sound_move_start(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_sound_move_start()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn get_sound_move_start_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_sound_move_start_damaged()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn get_sound_move_loop(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_sound_move_loop()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn get_sound_move_loop_damaged(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_sound_move_loop_damaged()
            .map(|sound| {
                let event_name = if !sound.event_name.is_empty() {
                    sound.event_name.clone()
                } else {
                    sound.filename_to_load.clone()
                };
                crate::common::audio::AudioEventRts::new(event_name)
            })
            .unwrap_or_default()
    }

    fn is_kind_of(&self, kind: crate::common::KindOf) -> bool {
        let Some(idx) = Self::kind_index(kind) else {
            return false;
        };
        self.kindof_mask
            .checked_shr(idx)
            .map(|bits| (bits & 1) != 0)
            .unwrap_or(false)
    }

    fn get_id(&self) -> u32 {
        self.inner.get_template_id() as u32
    }

    fn weapon_template_sets(
        &self,
    ) -> &[game_engine::common::thing::thing_template::WeaponTemplateSet] {
        self.inner.weapon_template_sets()
    }

    fn get_build_cost(&self) -> crate::common::Int {
        self.inner.get_build_cost() as crate::common::Int
    }

    fn get_experience_value(&self, level: usize) -> crate::common::Int {
        self.inner.get_experience_value(level)
    }

    fn get_experience_required(&self, level: usize) -> crate::common::Int {
        self.inner.get_experience_required(level)
    }

    fn is_trainable(&self) -> bool {
        self.inner.is_trainable()
    }

    fn get_build_time(&self) -> crate::common::Real {
        self.inner.get_build_time()
    }

    fn get_threat_value(&self) -> UnsignedInt {
        self.inner.get_threat_value() as UnsignedInt
    }

    fn get_shroud_reveal_to_all_range(&self) -> Real {
        self.inner.get_shroud_reveal_to_all_range()
    }

    fn get_occlusion_delay(&self) -> u32 {
        self.inner.get_occlusion_delay()
    }

    fn get_crusher_level(&self) -> u32 {
        self.inner.get_crusher_level() as u32
    }

    fn get_crushable_level(&self) -> u32 {
        self.inner.get_crushable_level() as u32
    }

    fn calc_cost_to_build(&self, player: Option<&dyn std::any::Any>) -> crate::common::Int {
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            return self.get_build_cost();
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_cost_change_percent =
            player.get_production_cost_change_percent(self.get_name().as_str());
        mods.handicap_cost_multiplier = player.get_handicap().get_cost_multiplier();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(self.kindof_mask);

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        calc.calc_cost_to_build(self.get_build_cost(), &mods)
    }

    fn calc_time_to_build(&self, player: Option<&dyn std::any::Any>) -> crate::common::Int {
        let Some(player) = player.and_then(|p| p.downcast_ref::<crate::player::Player>()) else {
            let frames = (self.get_build_time() * crate::common::LOGICFRAMES_PER_SECOND as f32)
                .round() as i32;
            return frames.max(0);
        };

        let mut mods =
            crate::object::production::build_cost_calculator::PlayerBuildModifiers::default();
        mods.production_time_change_percent =
            player.get_production_time_change_percent(self.get_name().as_str());
        mods.handicap_time_multiplier = player.get_handicap().get_build_time_multiplier();
        mods.energy_supply_ratio = player.get_energy().supply_ratio();
        mods.production_cost_change_by_kind =
            player.get_production_cost_change_based_on_kind_of(self.kindof_mask);
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        {
            mods.builds_instantly = player.builds_instantly();
        }

        let global_mods =
            crate::object::production::build_cost_calculator::GlobalBuildModifiers::from_global_data();
        let calc =
            crate::object::production::build_cost_calculator::BuildCostCalculator::with_modifiers(
                global_mods,
            );
        let facility_context = self.build_facility_context(player);
        calc.calc_time_to_build(self.get_build_time(), &mods, facility_context.as_ref())
            as crate::common::Int
    }

    fn module_descriptors(&self) -> ModuleDescriptorSet {
        self.inner.module_descriptors()
    }

    fn get_draw_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.draw_modules
    }

    fn get_client_update_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.client_update_modules
    }

    fn get_behavior_module_info(&self) -> &[crate::common::TemplateModuleInfo] {
        &self.behavior_modules
    }

    fn get_radar_priority(&self) -> crate::common::RadarPriorityType {
        // Convert engine RadarPriorityType to game logic RadarPriorityType.
        // The engine uses Invalid/Low/Medium/High/Critical while
        // the logic uses Invalid/NotOnRadar/Structure/Unit/LocalUnitOnly.
        match self.inner.get_radar_priority() {
            game_engine::common::thing::thing_template::RadarPriorityType::Invalid => {
                crate::common::RadarPriorityType::Invalid
            }
            game_engine::common::thing::thing_template::RadarPriorityType::Low => {
                crate::common::RadarPriorityType::NotOnRadar
            }
            game_engine::common::thing::thing_template::RadarPriorityType::Medium => {
                crate::common::RadarPriorityType::Structure
            }
            game_engine::common::thing::thing_template::RadarPriorityType::High => {
                crate::common::RadarPriorityType::Unit
            }
            game_engine::common::thing::thing_template::RadarPriorityType::Critical => {
                crate::common::RadarPriorityType::LocalUnitOnly
            }
        }
    }
}

/// Global random number generator state for game logic
static GAME_LOGIC_SEED: Mutex<[u32; 6]> = Mutex::new([
    0xf22d0e56, 0x883126e9, 0xc624dd2f, 0x702c49c, 0x9e353f7d, 0x6fdf3b64,
]);
static GAME_CLIENT_SEED: Lazy<Mutex<[u32; 6]>> = Lazy::new(|| {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0x1234_5678);
    Mutex::new([
        0x1d2c3b4a ^ nanos,
        0x8b31_26e9 ^ nanos.rotate_left(5),
        0xc624_dd2f ^ nanos.rotate_left(11),
        0x0702_c49c ^ nanos.rotate_left(17),
        0x9e35_3f7d ^ nanos.rotate_left(23),
        0x6fdf_3b64 ^ nanos.rotate_left(29),
    ])
});

/// Adds two numbers with carry flag (matching C++ ADC macro)
fn adc(a: u32, b: u32, carry: u32) -> (u32, u32) {
    let sum = a.wrapping_add(b).wrapping_add(carry);
    let new_carry = if sum < a || sum < b { 1 } else { 0 };
    (sum, new_carry)
}

/// Generate random value using game logic seed (matching C++ randomValue function)
fn random_value(seed: &mut [u32; 6]) -> u32 {
    let mut ax;
    let mut c = 0;

    // Add with carry operations
    (ax, c) = adc(seed[5], seed[4], c);
    seed[4] = ax;

    (ax, c) = adc(ax, seed[3], c);
    seed[3] = ax;

    (ax, c) = adc(ax, seed[2], c);
    seed[2] = ax;

    (ax, c) = adc(ax, seed[1], c);
    seed[1] = ax;

    (ax, c) = adc(ax, seed[0], c);
    seed[0] = ax;

    // Increment seed array, bubbling up the carries
    seed[5] = seed[5].wrapping_add(1);
    if seed[5] == 0 {
        seed[4] = seed[4].wrapping_add(1);
        if seed[4] == 0 {
            seed[3] = seed[3].wrapping_add(1);
            if seed[3] == 0 {
                seed[2] = seed[2].wrapping_add(1);
                if seed[2] == 0 {
                    seed[1] = seed[1].wrapping_add(1);
                    if seed[1] == 0 {
                        seed[0] = seed[0].wrapping_add(1);
                        ax = ax.wrapping_add(1);
                    }
                }
            }
        }
    }

    let _ = c; // carry explicitly ignored after bubbling
    ax
}

/// Gets game logic random integer value (matching C++ GetGameLogicRandomValue)
pub fn get_game_logic_random_value(lo: Int, hi: Int) -> Int {
    if hi < lo {
        return hi;
    }

    let delta = (hi - lo + 1) as u32;
    if delta == 0 {
        return hi;
    }

    let mut seed = GAME_LOGIC_SEED.lock().unwrap();
    let rval = (random_value(&mut *seed) % delta) as Int + lo;
    rval
}

/// Gets game logic random u32 value (convenience wrapper for u32 params)
/// Matches C++ GetGameLogicRandomValue behavior
pub fn game_logic_random_value(lo: u32, hi: u32) -> u32 {
    if hi < lo {
        return hi;
    }

    let delta = hi - lo + 1;
    if delta == 0 {
        return hi;
    }

    let mut seed = GAME_LOGIC_SEED.lock().unwrap();
    let rval = (random_value(&mut *seed) % delta) + lo;
    rval
}

/// Gets game logic random real value (matching C++ GetGameLogicRandomValueReal)
pub fn get_game_logic_random_value_real(lo: Real, hi: Real) -> Real {
    if hi <= lo {
        return hi;
    }

    let delta = hi - lo;
    let mult_factor = 1.0 / (2.0_f32.powi(32) - 1.0);

    let mut seed = GAME_LOGIC_SEED.lock().unwrap();
    let rval = (random_value(&mut *seed) as Real * mult_factor) * delta + lo;
    rval
}

/// Client-side random value (visual-only; not network-synchronized).
pub fn game_client_random_value(lo: Int, hi: Int) -> Int {
    if hi < lo {
        return hi;
    }

    let delta = (hi - lo + 1) as u32;
    if delta == 0 {
        return hi;
    }

    let mut seed = GAME_CLIENT_SEED.lock().unwrap();
    let rval = (random_value(&mut *seed) % delta) as Int + lo;
    rval
}

/// Client-side random real (visual-only; not network-synchronized).
pub fn game_client_random_value_real(lo: Real, hi: Real) -> Real {
    if hi <= lo {
        return hi;
    }

    let delta = hi - lo;
    let mult_factor = 1.0 / (2.0_f32.powi(32) - 1.0);

    let mut seed = GAME_CLIENT_SEED.lock().unwrap();
    let rval = (random_value(&mut *seed) as Real * mult_factor) * delta + lo;
    rval
}

/// Gets the CRC of the game logic random seed state (matching C++ GetGameLogicRandomSeedCRC)
/// CRITICAL for network synchronization - ensures all players have same random state
pub fn get_game_logic_random_seed_crc() -> UnsignedInt {
    let seed = GAME_LOGIC_SEED.lock().unwrap();
    // Calculate CRC32 of the entire seed array - 6 * 4 = 24 bytes
    let seed_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(seed.as_ptr() as *const u8, 6 * 4) };
    crc32fast::hash(seed_bytes)
}

/// Sets the game logic random seed (matching C++ SetGameLogicRandomSeed)
pub fn set_game_logic_random_seed(new_seed: [u32; 6]) {
    let mut seed = GAME_LOGIC_SEED.lock().unwrap();
    *seed = new_seed;
}

/// Game logic random value macro (matching C++ GameLogicRandomValue macro)
#[macro_export]
macro_rules! GameLogicRandomValue {
    ($lo:expr, $hi:expr) => {
        crate::helpers::get_game_logic_random_value($lo as i32, $hi as i32)
    };
}

/// Game logic random real value macro (matching C++ GameLogicRandomValueReal macro)
#[macro_export]
macro_rules! GameLogicRandomValueReal {
    ($lo:expr, $hi:expr) => {
        crate::helpers::get_game_logic_random_value_real($lo, $hi)
    };
}

/// Client random real value macro (matching C++ GameClientRandomValueReal macro)
#[macro_export]
macro_rules! GameClientRandomValueReal {
    ($lo:expr, $hi:expr) => {
        crate::helpers::game_client_random_value_real($lo, $hi)
    };
}

/// Make object status mask macro (matching C++ MAKE_OBJECT_STATUS_MASK).
#[macro_export]
macro_rules! MAKE_OBJECT_STATUS_MASK {
    ($status:expr) => {
        crate::common::ObjectStatusMaskType::from_status($status)
    };
}

/// Make model condition mask macro (matching C++ MAKE_MODELCONDITION_MASK).
#[macro_export]
macro_rules! MAKE_MODELCONDITION_MASK {
    ($condition:expr) => {
        $condition
    };
}

/// TheGameLogic singleton bridge - global game state access (matching C++ TheGameLogic)
pub struct TheGameLogic;

static GAME_PAUSED: AtomicBool = AtomicBool::new(false);
static GAME_PAUSE_MUSIC: AtomicBool = AtomicBool::new(false);
static INTRO_MOVIE_PLAYING: AtomicBool = AtomicBool::new(false);
static START_NEW_GAME_REQUESTED: AtomicBool = AtomicBool::new(false);
static GAME_START_RANK_POINTS: AtomicI32 = AtomicI32::new(0);
static GLOBAL_DIFFICULTY: AtomicI32 = AtomicI32::new(0);
static LOCAL_ALLIED_VICTORY: AtomicBool = AtomicBool::new(false);
static HULK_MAX_LIFETIME_OVERRIDE: AtomicI32 = AtomicI32::new(-1);
static INPUT_ENABLED: AtomicBool = AtomicBool::new(true);

impl TheGameLogic {
    /// Destroy an object (matches C++ TheGameLogic::destroyObject)
    pub fn destroy_object(
        object: &crate::object::Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_id = object.get_id();
        if object_id == INVALID_ID {
            return Ok(());
        }

        let mut logic = crate::system::game_logic::get_game_logic()
            .lock()
            .map_err(|_| "Failed to lock game logic")?;
        logic.destroy_object(object_id);
        Ok(())
    }

    /// Destroy object by id (mirrors C++ overload used by behavior modules)
    pub fn destroy_object_by_id(
        object_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if object_id == INVALID_ID {
            return Ok(());
        }

        let mut logic = crate::system::game_logic::get_game_logic()
            .lock()
            .map_err(|_| "Failed to lock game logic")?;
        logic.destroy_object(object_id);
        Ok(())
    }

    pub fn queue_objects_changed_trigger_areas(object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.queue_objects_changed_trigger_areas(object_id);
        }
    }

    /// Get current frame number (mirrors C++ TheGameLogic::Get_Frame)
    pub fn get_frame() -> UnsignedInt {
        crate::system::game_logic::current_frame()
    }

    /// Get number of sleepy update modules queued in GameLogic.
    pub fn get_number_sleepy_updates() -> usize {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_number_sleepy_updates())
            .unwrap_or(0)
    }

    /// Get the next object-id counter value from GameLogic.
    pub fn get_object_id_counter() -> ObjectID {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_object_id_counter())
            .unwrap_or(1)
    }

    /// Get whether draw icon UI indicators are enabled.
    pub fn get_draw_icon_ui() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_draw_icon_ui())
            .unwrap_or(true)
    }

    /// Get whether behind-building markers are enabled.
    pub fn get_show_behind_building_markers() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_show_behind_building_markers())
            .unwrap_or(true)
    }

    /// Set whether behind-building markers are enabled.
    pub fn set_show_behind_building_markers(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_show_behind_building_markers(enabled);
        }
    }

    /// Set whether draw icon UI indicators are enabled.
    pub fn set_draw_icon_ui(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_draw_icon_ui(enabled);
        }
    }

    /// Get whether dynamic LOD is enabled.
    pub fn get_show_dynamic_lod() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_show_dynamic_lod())
            .unwrap_or(true)
    }

    /// Set whether dynamic LOD is enabled.
    pub fn set_show_dynamic_lod(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_show_dynamic_lod(enabled);
        }
    }

    /// Get whether scoring is enabled.
    pub fn is_scoring_enabled() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_scoring_enabled())
            .unwrap_or(true)
    }

    /// Enable/disable scoring.
    pub fn set_scoring_enabled(enabled: Bool) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_scoring_enabled(enabled);
        }
    }

    /// Get the global map/script rank cap.
    pub fn get_rank_level_limit() -> Int {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_rank_level_limit())
            .unwrap_or(1000)
    }

    /// Set the global map/script rank cap.
    pub fn set_rank_level_limit(level: Int) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_rank_level_limit(level);
        }
    }

    /// Set a runtime buildability override for a template.
    pub fn set_buildable_status_override(template_name: &str, status: Int) {
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_buildable_status_override(template_name, status);
        }
    }

    /// Find a runtime buildability override for a template.
    pub fn find_buildable_status_override(template_name: &str) -> Option<Int> {
        crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .and_then(|logic| logic.find_buildable_status_override(template_name))
    }

    /// Set the paused state of the game (matches C++ TheGameLogic::setGamePaused).
    pub fn set_game_paused(paused: Bool, pause_music: Bool) {
        let current = GAME_PAUSED.load(Ordering::Relaxed);
        if current == paused {
            return;
        }

        GAME_PAUSED.store(paused, Ordering::Relaxed);
        GAME_PAUSE_MUSIC.store(paused && pause_music, Ordering::Relaxed);

        if let Some(hooks) = game_pause_hooks() {
            hooks.on_game_pause_state_changed(paused);
        }

        if let Some(audio) = TheAudio::get() {
            if paused {
                if pause_music {
                    audio.pause_audio(EngineAudioAffect::All);
                } else {
                    audio.pause_audio(EngineAudioAffect::Sound);
                    audio.pause_audio(EngineAudioAffect::Sound3D);
                    audio.pause_audio(EngineAudioAffect::Speech);
                }
            } else if pause_music {
                audio.resume_audio(EngineAudioAffect::All);
            } else {
                audio.resume_audio(EngineAudioAffect::Sound);
                audio.resume_audio(EngineAudioAffect::Sound3D);
                audio.resume_audio(EngineAudioAffect::Speech);
            }
        }
    }

    /// Return the paused state of the game.
    pub fn is_game_paused() -> Bool {
        GAME_PAUSED.load(Ordering::Relaxed)
    }

    /// Return whether pause music is active.
    pub fn is_pause_music_active() -> Bool {
        GAME_PAUSE_MUSIC.load(Ordering::Relaxed)
    }

    /// Set whether the intro movie is playing.
    pub fn set_intro_movie_playing(playing: Bool) {
        INTRO_MOVIE_PLAYING.store(playing, Ordering::Relaxed);
    }

    /// Return whether the intro movie is playing.
    pub fn is_intro_movie_playing() -> Bool {
        INTRO_MOVIE_PLAYING.load(Ordering::Relaxed)
    }

    /// Set whether player input is enabled (ScriptActions::doDisableInput / doEnableInput).
    pub fn set_input_enabled(enabled: Bool) {
        INPUT_ENABLED.store(enabled, Ordering::Relaxed);
    }

    /// Return whether player input is enabled.
    pub fn is_input_enabled() -> Bool {
        INPUT_ENABLED.load(Ordering::Relaxed)
    }

    /// Return whether the game is currently loading a map.
    pub fn is_loading_map() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_loading_map())
            .unwrap_or(false)
    }

    /// Return whether the game is currently in multiplayer mode.
    pub fn is_in_multiplayer_game() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_in_multiplayer_game())
            .unwrap_or(false)
    }

    /// Return whether the game is currently in skirmish mode.
    pub fn is_in_skirmish_game() -> Bool {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.is_in_skirmish_game())
            .unwrap_or(false)
    }

    /// Return whether the game is currently replaying.
    pub fn is_in_replay_game() -> Bool {
        Self::get_game_mode() == crate::system::game_logic::GAME_REPLAY
    }

    /// Return whether the game has entered any mode (matches C++ GameLogic::isInGame()).
    pub fn is_in_game() -> Bool {
        let mode = Self::get_game_mode();
        mode != crate::system::game_logic::GAME_NONE
    }

    /// Get the current game mode.
    pub fn get_game_mode() -> Int {
        crate::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_game_mode())
            .unwrap_or(crate::system::game_logic::GAME_NONE)
    }

    /// Prepare for a new game (matches C++ GameLogic::prepareNewGame).
    pub fn prepare_new_game(game_mode: Int, difficulty: Int, rank_points: Int) {
        TheScriptEngine::set_global_difficulty(difficulty);
        Self::clear_start_new_game_request();
        if let Some(hooks) = prepare_new_game_hooks() {
            hooks.ensure_background_window();
        }
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(game_mode);
        }
        if let Some(data) = get_engine_global_data() {
            let mut data = data.write();
            if !data.pending_file.is_empty() {
                data.map_name = data.pending_file.clone();
                data.pending_file.clear();
            }
        }
        Self::set_rank_points_to_add_at_game_start(rank_points);
        if game_mode != crate::system::game_logic::GAME_SHELL {
            if let Some(hooks) = prepare_new_game_hooks() {
                hooks.hide_shell();
            }
        }
    }

    /// Start a prepared game (C++ parity: GameLogic::startNewGame(FALSE)).
    pub fn start_new_game(is_load_game: Bool) -> Result<(), String> {
        if !is_load_game {
            Self::request_start_new_game();
            return Ok(());
        }

        Self::clear_start_new_game_request();

        let map_path = get_engine_global_data()
            .map(|data| data.read().map_name.clone())
            .unwrap_or_default();

        if map_path.is_empty() {
            return Err("Cannot start game: global map_name is empty".to_string());
        }

        // C++ parity: GameLogic::startNewGame(FALSE) records pristine map name before
        // map INI/sidecar resolution so save-directory maps can remap to original path.
        if !is_load_game {
            let mut state = game_engine::System::get_game_state();
            state.set_pristine_map_name(map_path.clone());
            if state.is_in_save_directory(std::path::Path::new(&map_path)) {
                log::error!(
                    "Pristine map name points to save directory map '{}'; sidecar lookup may diverge from C++ expected source-map semantics",
                    map_path
                );
            }
        }

        let params = crate::system::game_initialization::GameInitParams {
            map_path,
            game_mode: Self::to_init_game_mode(Self::get_game_mode()),
            difficulty: Self::to_init_difficulty(TheScriptEngine::get_global_difficulty()),
            num_players: Self::detect_player_count_for_init(),
            player_templates: Vec::new(),
            victory_type: crate::system::victory_conditions::VictoryType::Annihilation,
            score_limit: None,
            time_limit: None,
            fog_of_war_enabled: true,
            starting_resources: 0,
            ai_script: "DefaultAI".to_string(),
        };

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_loading_map(true);
        }

        let init_result =
            crate::system::game_initialization::GameInitializer::initialize_game(params)
                .map(|_| ())
                .map_err(|err| format!("Game initialization failed: {}", err));

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_loading_map(false);
        }

        init_result
    }

    /// Request that the next game-logic update complete a staged new-game start.
    pub fn request_start_new_game() {
        START_NEW_GAME_REQUESTED.store(true, Ordering::Relaxed);
    }

    /// Check whether a staged new-game start is waiting to be completed.
    pub fn is_start_new_game_requested() -> Bool {
        START_NEW_GAME_REQUESTED.load(Ordering::Relaxed)
    }

    /// Clear any staged new-game start request.
    pub fn clear_start_new_game_request() {
        START_NEW_GAME_REQUESTED.store(false, Ordering::Relaxed);
    }

    fn to_init_game_mode(mode: Int) -> crate::system::game_initialization::GameMode {
        match mode {
            crate::system::game_logic::GAME_SHELL => {
                crate::system::game_initialization::GameMode::ShellMap
            }
            crate::system::game_logic::GAME_SKIRMISH => {
                crate::system::game_initialization::GameMode::Skirmish
            }
            crate::system::game_logic::GAME_LAN | crate::system::game_logic::GAME_INTERNET => {
                crate::system::game_initialization::GameMode::Multiplayer
            }
            crate::system::game_logic::GAME_REPLAY => {
                crate::system::game_initialization::GameMode::Replay
            }
            _ => crate::system::game_initialization::GameMode::SinglePlayer,
        }
    }

    fn to_init_difficulty(difficulty: Int) -> crate::system::game_initialization::GameDifficulty {
        match difficulty {
            0 => crate::system::game_initialization::GameDifficulty::Easy,
            2 => crate::system::game_initialization::GameDifficulty::Hard,
            3 => crate::system::game_initialization::GameDifficulty::Brutal,
            _ => crate::system::game_initialization::GameDifficulty::Normal,
        }
    }

    fn detect_player_count_for_init() -> usize {
        if let Ok(sides_guard) = crate::sides_list::get_sides_list().read() {
            let count = sides_guard.get_num_sides().max(1) as usize;
            return count.min(crate::system::player_init::MAX_PLAYER_COUNT);
        }

        if let Ok(player_list) = crate::player::ThePlayerList().read() {
            let count = player_list.iter().count();
            if count > 0 {
                return count.min(crate::system::player_init::MAX_PLAYER_COUNT);
            }
        }

        2
    }

    /// Reset game logic state (matches C++ TheGameLogic::clearGameData).
    pub fn clear_game_data() -> Result<(), String> {
        if !Self::is_in_game() {
            return Err("clear_game_data called while not in game".to_string());
        }

        // C++ parity: GameLogic::clearGameData() performs an engine reset, then forces
        // GAME_NONE and conditionally marks the engine quitting for initial-file startup.
        if let Some(engine) = get_game_engine() {
            let mut guard = engine.lock();
            let _ = futures::executor::block_on(guard.reset());
        }

        crate::system::game_logic::reset_game_logic()?;

        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(crate::system::game_logic::GAME_NONE);
        }

        let has_initial_file = get_engine_global_data()
            .map(|data| !data.read().initial_file.is_empty())
            .unwrap_or(false);
        if has_initial_file {
            if let Some(engine) = get_game_engine() {
                engine.lock().set_quitting(true);
            }
        }

        Ok(())
    }

    /// Set rank points used at game start.
    pub fn set_rank_points_to_add_at_game_start(points: Int) {
        GAME_START_RANK_POINTS.store(points, Ordering::Relaxed);
    }

    /// Get rank points used at game start.
    pub fn get_rank_points_to_add_at_game_start() -> Int {
        GAME_START_RANK_POINTS.load(Ordering::Relaxed)
    }

    /// Get the global weapon bonus set used by all weapons.
    pub fn get_global_weapon_bonus_set() -> Option<WeaponBonusSet> {
        crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .map(|logic| logic.get_global_weapon_bonus_set().clone())
    }

    /// Try to get current frame but propagate locking errors.
    pub fn try_get_frame() -> Result<UnsignedInt, String> {
        crate::system::game_logic::try_current_frame()
    }

    /// Find object by ID using the global registry (mirrors C++ TheGameLogic::Find_Object_By_ID)
    pub fn find_object_by_id(
        id: ObjectID,
    ) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
        OBJECT_REGISTRY.get_object(id)
    }

    /// Register a newly created object handle with the global registry.
    pub fn register_object(
        object: std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
    ) -> Result<(), GameError> {
        let id = { object.read().map_err(|_| GameError::LockError)?.get_id() };
        OBJECT_REGISTRY.register_object(id, &object);
        register_legacy_object(&object);
        Ok(())
    }

    /// Remove an object handle from the registry.
    pub fn remove_object(object_id: ObjectID) {
        OBJECT_REGISTRY.unregister_object(object_id);
        unregister_legacy_object(object_id);
    }

    /// Deselect object (mirroring GameLogic::deselectObject selection flow).
    pub fn deselect_object(
        object: &crate::object::Object,
        mask: PlayerMaskType,
        affect_client: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::commands::{get_selection_manager, SelectionType};

        let object_id = object.get_id();
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;

        if let Ok(list) = crate::player::player_list().read() {
            for (player_index, player_arc) in list.iter().enumerate() {
                let bit = PlayerMaskType::from_bits_truncate(1u32 << (player_index as u32));
                if !mask.contains(bit) {
                    continue;
                }

                let legacy_obj = crate::ai::object_registry::get_legacy_object(object_id);
                let mut actually_removed = false;

                if let Ok(mut player) = player_arc.write() {
                    if let Some(legacy_obj) = legacy_obj.as_ref() {
                        let mut group = crate::ai::AIGroup::new(0);
                        player.get_current_selection_as_ai_group(&mut group);
                        if let Ok(deleted) = group.remove(legacy_obj) {
                            actually_removed = true;
                            if deleted {
                                player.set_currently_selected_ai_group(None);
                            } else {
                                player.set_currently_selected_ai_group(Some(&group));
                            }
                        }
                    } else if player.remove_object_from_current_selection(object_id) {
                        actually_removed = true;
                    }

                    if actually_removed && affect_client {
                        if let Some(drawable) = object.get_drawable() {
                            TheInGameUI::deselect_drawable(&drawable);
                        }
                    }
                }

                if actually_removed {
                    if let Some(selection) = manager.get_player_selection(player_index as i32) {
                        selection.select_objects(vec![object_id], SelectionType::Remove);
                    }
                }
            }
        }
        Ok(())
    }

    /// Select object (mirroring GameLogic::selectObject selection flow).
    pub fn select_object(
        object: &crate::object::Object,
        create_new_selection: bool,
        mask: PlayerMaskType,
        affect_client: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::commands::{get_selection_manager, SelectionType};

        if !object.is_mass_selectable() && !create_new_selection {
            return Ok(());
        }

        let object_id = object.get_id();
        let can_add_to_group = object.get_ai_update_interface().is_some()
            || object.is_any_kind_of(&[KindOf::Structure, KindOf::AlwaysSelectable]);
        let selection_manager = get_selection_manager();
        let mut manager = selection_manager
            .write()
            .map_err(|_| "Failed to lock selection manager")?;

        let selection_type = if create_new_selection {
            SelectionType::Replace
        } else {
            SelectionType::Add
        };

        if let Ok(list) = crate::player::player_list().read() {
            for (player_index, player_arc) in list.iter().enumerate() {
                let bit = PlayerMaskType::from_bits_truncate(1u32 << (player_index as u32));
                if !mask.contains(bit) {
                    continue;
                }

                let legacy_obj = crate::ai::object_registry::get_legacy_object(object_id);
                let mut added_to_group = false;

                if let Ok(mut player) = player_arc.write() {
                    if let Some(legacy_obj) = legacy_obj.as_ref() {
                        let mut group = crate::ai::AIGroup::new(0);
                        let _ = group.add(legacy_obj.clone());
                        added_to_group = group.get_count() > 0;
                        if create_new_selection {
                            player.set_currently_selected_ai_group(Some(&group));
                        } else {
                            player.add_ai_group_to_current_selection(&group);
                        }
                    } else if create_new_selection {
                        if can_add_to_group {
                            player.set_current_selection_to_object(object_id);
                            added_to_group = true;
                        } else {
                            player.set_currently_selected_ai_group(None);
                        }
                    } else if can_add_to_group {
                        player.add_object_to_current_selection(object_id);
                        added_to_group = true;
                    }
                }

                if added_to_group || (legacy_obj.is_none() && can_add_to_group) {
                    if let Some(selection) = manager.get_player_selection(player_index as i32) {
                        selection.select_objects(vec![object_id], selection_type);
                    }
                }

                if affect_client {
                    if let Some(drawable) = object.get_drawable() {
                        TheInGameUI::select_drawable(&drawable);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get hulk max lifetime override.
    pub fn get_hulk_max_lifetime_override() -> Int {
        HULK_MAX_LIFETIME_OVERRIDE.load(Ordering::Relaxed)
    }

    /// Set hulk max lifetime override (used by scripting)
    pub fn set_hulk_max_lifetime_override(lifetime: Int) {
        HULK_MAX_LIFETIME_OVERRIDE.store(lifetime, Ordering::Relaxed);
    }

    /// Register an update module with the global scheduler.
    pub fn register_update_module(
        object_id: ObjectID,
        module: UpdateModulePtr,
        wake_frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mutex = crate::system::game_logic::get_game_logic();
        let mut logic = mutex
            .lock()
            .map_err(|err| format!("Failed to lock GameLogic: {}", err))?;

        logic.unregister_update_module(object_id, module.clone());

        if wake_frame == 0 {
            logic.register_normal_update_module(object_id, module);
        } else {
            logic.register_sleepy_update_module(object_id, module, wake_frame);
        }

        Ok(())
    }

    /// Remove an update module from the global scheduler.
    pub fn unregister_update_module(
        object_id: ObjectID,
        module: UpdateModulePtr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mutex = crate::system::game_logic::get_game_logic();
        let mut logic = mutex
            .lock()
            .map_err(|err| format!("Failed to lock GameLogic: {}", err))?;

        logic.unregister_update_module(object_id, module);
        Ok(())
    }

    /// Set wake frame for an object's update modules (matches C++ setWakeFrame)
    pub fn set_wake_frame(object_id: ObjectID, sleep_time: crate::modules::UpdateSleepTime) {
        use crate::object_manager::get_object_manager;

        let current_frame = Self::get_frame();
        let manager_arc = get_object_manager();
        let manager_lock = &*manager_arc;
        let Ok(manager) = manager_lock.read() else {
            return;
        };
        let Some(instance_arc) = manager.get_object(object_id) else {
            if let Some(object_arc) = Self::find_object_by_id(object_id) {
                if let Ok(mut object) = object_arc.write() {
                    object.wake_update_modules_after(current_frame, sleep_time);
                }
            }
            return;
        };
        let instance_lock = &*instance_arc;
        if let Ok(mut instance) = instance_lock.write() {
            instance.wake_all_update_modules_after(current_frame, sleep_time);
        };

        if let Some(object_arc) = Self::find_object_by_id(object_id) {
            if let Ok(mut object) = object_arc.write() {
                object.wake_update_modules_after(current_frame, sleep_time);
            }
        }
    }
}

/// Terrain logic bridge for gameplay queries (matching C++ TheTerrainLogic)
pub struct TheTerrainLogic;

impl TheTerrainLogic {
    pub fn get() -> Option<&'static Self> {
        static TERRAIN: OnceLock<TheTerrainLogic> = OnceLock::new();
        Some(TERRAIN.get_or_init(|| TheTerrainLogic))
    }

    pub fn is_underwater(
        &self,
        _x: Real,
        _y: Real,
        water_z: Option<&mut f32>,
        terrain_z: Option<&mut f32>,
    ) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.read() {
            return guard.is_underwater(_x as f32, _y as f32, water_z, terrain_z);
        }
        if let Some(wz) = water_z {
            *wz = 0.0;
        }
        if let Some(tz) = terrain_z {
            *tz = 0.0;
        }
        false
    }

    pub fn is_cliff_cell(&self, _x: Real, _y: Real) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.read() {
            return guard.is_cliff_cell(_x as f32, _y as f32);
        }
        false
    }

    /// Get terrain height at coordinates (bridges to TerrainLogic when available).
    pub fn get_height_at(&self, x: Real, y: Real) -> Real {
        self.get_ground_height(x, y, None)
    }

    /// Get layer height at coordinates (bridges to TerrainLogic when available).
    pub fn get_layer_height(&self, x: Real, y: Real, layer: PathfindLayerEnum) -> Real {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return 0.0;
        };
        let terrain_layer = match layer {
            PathfindLayerEnum::Top | PathfindLayerEnum::Air => crate::path::PathfindLayerEnum::Top,
            PathfindLayerEnum::Bridge1 => crate::path::PathfindLayerEnum::Bridge1,
            PathfindLayerEnum::Bridge2 => crate::path::PathfindLayerEnum::Bridge2,
            PathfindLayerEnum::Bridge3 => crate::path::PathfindLayerEnum::Bridge3,
            PathfindLayerEnum::Bridge4 => crate::path::PathfindLayerEnum::Bridge4,
            PathfindLayerEnum::Wall => crate::path::PathfindLayerEnum::Wall,
            _ => crate::path::PathfindLayerEnum::Ground,
        };
        guard.get_layer_height(x, y, terrain_layer, None, true)
    }

    /// Get highest layer for destination (bridges to TerrainLogic when available).
    pub fn get_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return PathfindLayerEnum::Ground;
        };
        match guard.get_highest_layer_for_destination(pos) {
            crate::path::PathfindLayerEnum::Top => PathfindLayerEnum::Top,
            crate::path::PathfindLayerEnum::Bridge1 => PathfindLayerEnum::Bridge1,
            crate::path::PathfindLayerEnum::Bridge2 => PathfindLayerEnum::Bridge2,
            crate::path::PathfindLayerEnum::Bridge3 => PathfindLayerEnum::Bridge3,
            crate::path::PathfindLayerEnum::Bridge4 => PathfindLayerEnum::Bridge4,
            crate::path::PathfindLayerEnum::Wall => PathfindLayerEnum::Wall,
            crate::path::PathfindLayerEnum::Invalid => PathfindLayerEnum::Invalid,
            _ => PathfindLayerEnum::Ground,
        }
    }

    /// Get destination layer (bridges to TerrainLogic::get_layer_for_destination).
    pub fn get_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return PathfindLayerEnum::Ground;
        };
        match guard.get_layer_for_destination(pos) {
            crate::path::PathfindLayerEnum::Top => PathfindLayerEnum::Top,
            crate::path::PathfindLayerEnum::Bridge1 => PathfindLayerEnum::Bridge1,
            crate::path::PathfindLayerEnum::Bridge2 => PathfindLayerEnum::Bridge2,
            crate::path::PathfindLayerEnum::Bridge3 => PathfindLayerEnum::Bridge3,
            crate::path::PathfindLayerEnum::Bridge4 => PathfindLayerEnum::Bridge4,
            crate::path::PathfindLayerEnum::Wall => PathfindLayerEnum::Wall,
            crate::path::PathfindLayerEnum::Invalid => PathfindLayerEnum::Invalid,
            _ => PathfindLayerEnum::Ground,
        }
    }

    /// Bridge interaction helper for locomotor/path layers.
    pub fn object_interacts_with_bridge_layer(
        &self,
        obj: &Object,
        layer: PathfindLayerEnum,
        consider_bridge_health: bool,
    ) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return false;
        };
        let terrain_layer = match layer {
            PathfindLayerEnum::Top | PathfindLayerEnum::Air => crate::path::PathfindLayerEnum::Top,
            PathfindLayerEnum::Bridge1 => crate::path::PathfindLayerEnum::Bridge1,
            PathfindLayerEnum::Bridge2 => crate::path::PathfindLayerEnum::Bridge2,
            PathfindLayerEnum::Bridge3 => crate::path::PathfindLayerEnum::Bridge3,
            PathfindLayerEnum::Bridge4 => crate::path::PathfindLayerEnum::Bridge4,
            PathfindLayerEnum::Wall => crate::path::PathfindLayerEnum::Wall,
            _ => crate::path::PathfindLayerEnum::Ground,
        };
        guard.object_interacts_with_bridge_layer(obj, terrain_layer, consider_bridge_health)
    }

    /// Get ground height with optional normal output (mirrors TerrainLogic::getGroundHeight).
    pub fn get_ground_height(&self, x: Real, y: Real, mut normal: Option<&mut Coord3D>) -> Real {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            if let Some(n) = normal.as_deref_mut() {
                *n = Coord3D::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        };
        guard.get_ground_height(x, y, normal.as_deref_mut())
    }

    pub fn is_clear_line_of_sight(&self, from: &Coord3D, to: &Coord3D) -> bool {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return false;
        };
        guard.is_clear_line_of_sight(from, to)
    }

    /// Get map extent including border. Uses a large fallback region when no map data is wired.
    pub fn get_extent_including_border(&self) -> crate::common::Region3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.read() {
            let extent = guard.get_extent_including_border();
            if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
                return extent;
            }
        }
        let lo = crate::common::Coord3D::new(0.0, 0.0, 0.0);
        let hi = crate::common::Coord3D::new(50000.0, 50000.0, 0.0);
        crate::common::Region3D::new(lo, hi)
    }

    /// Get maximum pathfind extent (playable area excluding border).
    pub fn get_maximum_pathfind_extent(&self) -> crate::common::Region3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.read() {
            let extent = guard.get_maximum_pathfind_extent();
            if extent.hi.x > extent.lo.x && extent.hi.y > extent.lo.y {
                return extent;
            }
        }
        self.get_extent_including_border()
    }

    /// Find closest edge point to a location (fallback uses extent bounds).
    pub fn find_closest_edge_point(&self, location: &Coord3D) -> Coord3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.read() {
            return guard.find_closest_edge_point(location);
        }

        let extent = self.get_maximum_pathfind_extent();
        let distances = [
            (location.y - extent.lo.y).abs(), // top
            (location.x - extent.hi.x).abs(), // right
            (location.y - extent.hi.y).abs(), // bottom
            (location.x - extent.lo.x).abs(), // left
        ];
        let mut best_index = 0usize;
        let mut best_distance = distances[0];
        for (idx, distance) in distances.iter().copied().enumerate().skip(1) {
            if distance < best_distance {
                best_distance = distance;
                best_index = idx;
            }
        }

        let mut ret = *location;
        match best_index {
            0 => ret.y = extent.lo.y,
            1 => ret.x = extent.hi.x,
            2 => ret.y = extent.hi.y,
            _ => ret.x = extent.lo.x,
        }
        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Find farthest edge point from a location (fallback uses extent bounds).
    pub fn find_farthest_edge_point(&self, location: &Coord3D) -> Coord3D {
        let terrain = crate::terrain::get_terrain_logic();
        if let Ok(guard) = terrain.read() {
            return guard.find_farthest_edge_point(location);
        }

        let extent = self.get_maximum_pathfind_extent();
        let mid_x = (extent.hi.x - extent.lo.x) * 0.5;
        let mid_y = (extent.hi.y - extent.lo.y) * 0.5;

        let mut ret = *location;
        if location.x < mid_x {
            ret.x = extent.hi.x;
        } else {
            ret.x = extent.lo.x;
        }

        if location.y < mid_y {
            ret.y = extent.hi.y;
        } else {
            ret.y = extent.lo.y;
        }

        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Find the closest waypoint that matches a path label.
    pub fn get_closest_waypoint_on_path(&self, pos: &Coord3D, label: &str) -> Option<Coord3D> {
        let terrain = crate::terrain::get_terrain_logic();
        let guard = terrain.read().ok()?;
        guard
            .get_closest_waypoint_on_path(pos, label)
            .map(|way| *way.get_location())
    }

    /// Build a linked waypoint chain starting from waypoint name.
    pub fn get_waypoint_chain_by_name(&self, start_name: &str, max_points: usize) -> Vec<Coord3D> {
        let mut out = Vec::new();
        if start_name.trim().is_empty() {
            return out;
        }

        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return out;
        };

        let name = AsciiString::from(start_name);
        let Some(start) = guard.get_waypoint_by_name(&name) else {
            return out;
        };

        let mut visited = HashSet::new();
        let mut current_id = Some(start.get_id());
        let limit = max_points.max(1);

        while let Some(id) = current_id {
            if !visited.insert(id) || out.len() >= limit {
                break;
            }

            let Some(waypoint) = guard.get_waypoint_by_id(id) else {
                break;
            };
            out.push(*waypoint.get_location());

            current_id = (0..waypoint.get_num_links())
                .filter_map(|idx| waypoint.get_link(idx))
                .find(|candidate| !visited.contains(candidate));
        }

        out
    }
}

/// FX list bridge to client-side effect manager (matching C++ TheFXList)
pub struct TheFXList;

static FX_LIST_MANAGER: OnceLock<Arc<dyn FXListManagerInterface>> = OnceLock::new();

pub fn register_fx_list_manager(manager: Arc<dyn FXListManagerInterface>) -> bool {
    FX_LIST_MANAGER.set(manager).is_ok()
}

pub fn get_fx_list_manager() -> Option<&'static Arc<dyn FXListManagerInterface>> {
    FX_LIST_MANAGER.get()
}

impl TheFXList {
    pub fn get() -> Option<&'static Self> {
        static FXLIST: OnceLock<TheFXList> = OnceLock::new();
        Some(FXLIST.get_or_init(|| TheFXList))
    }

    pub fn do_fx_at_position(&self, fx_template: &str, pos: &Coord3D) {
        let Some(manager) = FX_LIST_MANAGER.get() else {
            return;
        };
        let fx_id = NameKeyGenerator::name_to_key(fx_template) as FXListId;
        manager.do_fx_pos(fx_id, pos, None);
    }
}

/// Particle system manager bridge to the client-side implementation.
pub struct TheParticleSystemManager;

static PARTICLE_SYSTEM_MANAGER: OnceLock<Arc<dyn ParticleSystemManagerInterface>> = OnceLock::new();

pub fn register_particle_system_manager(manager: Arc<dyn ParticleSystemManagerInterface>) -> bool {
    PARTICLE_SYSTEM_MANAGER.set(manager).is_ok()
}

fn get_particle_system_manager() -> Option<&'static Arc<dyn ParticleSystemManagerInterface>> {
    PARTICLE_SYSTEM_MANAGER.get()
}

pub type ScorchHook = Arc<dyn Fn(&Coord3D, Real, i32) + Send + Sync>;
static SCORCH_HOOK: OnceLock<ScorchHook> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct TerrainTreeRegistration {
    pub drawable_id: u32,
    pub location: Coord3D,
    pub scale: Real,
    pub angle: Real,
    pub random_scale_amount: Real,
    pub module_data: W3DTreeDrawModuleData,
}

#[derive(Clone, Debug)]
pub enum TerrainTreeEvent {
    Add(TerrainTreeRegistration),
    Remove(u32),
}

pub type TerrainTreeHook = Arc<dyn Fn(TerrainTreeEvent) + Send + Sync>;
static TERRAIN_TREE_HOOK: OnceLock<TerrainTreeHook> = OnceLock::new();
pub type AnimationMetadataHook = Arc<dyn Fn(&str) -> Option<Real> + Send + Sync>;
static ANIMATION_METADATA_HOOK: OnceLock<AnimationMetadataHook> = OnceLock::new();

pub fn register_scorch_hook(hook: ScorchHook) -> bool {
    SCORCH_HOOK.set(hook).is_ok()
}

fn get_scorch_hook() -> Option<&'static ScorchHook> {
    SCORCH_HOOK.get()
}

pub fn register_terrain_tree_hook(hook: TerrainTreeHook) -> bool {
    TERRAIN_TREE_HOOK.set(hook).is_ok()
}

fn get_terrain_tree_hook() -> Option<&'static TerrainTreeHook> {
    TERRAIN_TREE_HOOK.get()
}

pub fn register_animation_metadata_hook(hook: AnimationMetadataHook) -> bool {
    ANIMATION_METADATA_HOOK.set(hook).is_ok()
}

fn get_animation_metadata_hook() -> Option<&'static AnimationMetadataHook> {
    ANIMATION_METADATA_HOOK.get()
}

impl TheParticleSystemManager {
    pub fn get() -> Option<&'static Self> {
        static MGR: OnceLock<TheParticleSystemManager> = OnceLock::new();
        Some(MGR.get_or_init(|| TheParticleSystemManager))
    }

    pub fn create_particle_system(&self, template: Option<&str>) -> Option<u32> {
        let manager = get_particle_system_manager()?;
        let name = template?;
        let template_id = manager.find_template(name)?;
        manager.create_particle_system(template_id)
    }

    pub fn create_attached_particle_system_id(
        &self,
        template: Option<&str>,
        object_id: ObjectID,
    ) -> Option<u32> {
        let manager = get_particle_system_manager()?;
        let name = template?;
        let template_id = manager.find_template(name)?;
        manager.create_attached_particle_system_id(template_id, object_id)
    }

    pub fn set_particle_system_position(&self, id: u32, position: &Coord3D) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_position(id, position);
        }
    }

    pub fn get_particle_system_position(&self, id: u32) -> Option<Coord3D> {
        let manager = get_particle_system_manager()?;
        manager.get_particle_system_position(id)
    }

    pub fn attach_particle_system_to_object(&self, id: u32, object_id: ObjectID) {
        if let Some(manager) = get_particle_system_manager() {
            manager.attach_particle_system_to_object(id, object_id);
        }
    }

    pub fn attach_particle_system_to_drawable(&self, id: u32, drawable_id: ObjectID) {
        if let Some(manager) = get_particle_system_manager() {
            manager.attach_particle_system_to_drawable(id, drawable_id);
        }
    }

    pub fn set_particle_system_transform(&self, id: u32, transform: &Matrix3D) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_transform(id, transform);
        }
    }

    pub fn destroy_particle_system(&self, id: u32) {
        if let Some(manager) = get_particle_system_manager() {
            manager.destroy_particle_system(id);
        }
    }

    pub fn destroy_attached_systems(&self, object_id: ObjectID) {
        if let Some(manager) = get_particle_system_manager() {
            manager.destroy_attached_systems(object_id);
        }
    }

    pub fn start_particle_system(&self, id: u32) {
        if let Some(manager) = get_particle_system_manager() {
            manager.start_particle_system(id);
        }
    }

    pub fn stop_particle_system(&self, id: u32) {
        if let Some(manager) = get_particle_system_manager() {
            manager.stop_particle_system(id);
        }
    }

    pub fn set_particle_system_velocity_multiplier(&self, id: u32, multiplier: &Coord3D) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_velocity_multiplier(id, multiplier);
        }
    }

    pub fn set_particle_system_burst_count_multiplier(&self, id: u32, multiplier: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_burst_count_multiplier(id, multiplier);
        }
    }

    pub fn find_particle_system(&self, id: u32) -> Option<Box<dyn std::any::Any>> {
        let manager = get_particle_system_manager()?;
        manager.find_particle_system(id)
    }

    pub fn get_particle_system_emission_volume_type(&self, id: u32) -> Option<EmissionVolumeType> {
        let manager = get_particle_system_manager()?;
        manager.get_particle_system_emission_volume_type(id)
    }

    pub fn set_particle_system_emission_volume_sphere_radius(&self, id: u32, radius: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_emission_volume_sphere_radius(id, radius);
        }
    }

    pub fn set_particle_system_emission_volume_cylinder_radius(&self, id: u32, radius: Real) {
        if let Some(manager) = get_particle_system_manager() {
            manager.set_particle_system_emission_volume_cylinder_radius(id, radius);
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectileStreamState {
    pub lines: Vec<Vec<Coord3D>>,
    pub texture_name: AsciiString,
    pub width: Real,
    pub tile_factor: Real,
    pub scroll_rate: Real,
}

/// Bone transform override for animated models (turret rotations, recoil shifts).
/// Consumed by the render bridge to produce `render_bridge::BoneOverride`.
#[derive(Clone, Debug)]
pub struct BoneOverrideState {
    pub bone_index: i32,
    pub transform: Matrix3D,
}

/// Per-frame model draw data written by W3DModelDraw::do_draw_module().
/// Read by the GameClient device layer to produce `render_bridge::DrawSubmission`.
#[derive(Clone, Debug)]
pub struct ModelDrawState {
    pub model_name: String,
    pub world_transform: Matrix3D,
    /// Raw ModelConditionFlags bits (u128); client maps to RenderConditionFlags.
    pub condition_flags_bits: u128,
    pub bone_overrides: Vec<BoneOverrideState>,
    pub animation_name: Option<String>,
    /// 0.0–1.0 fraction through the current animation cycle.
    pub animation_time: f32,
    /// Matches AnimMode discriminant (0=Manual … 5=OnceBackwards).
    pub animation_mode: i32,
}

#[derive(Clone, Debug)]
pub struct DrawableState {
    pub template_name: String,
    pub indicator_color: Color,
    pub position: Coord3D,
    pub orientation: Real,
    pub shroud_status_object_id: ObjectID,
    pub beam_start: Option<Coord3D>,
    pub beam_end: Option<Coord3D>,
    pub beam_width: Option<Real>,
    pub projectile_stream: Option<ProjectileStreamState>,
    /// Per-frame model draw data written by W3DModelDraw::do_draw_module().
    pub model_draw: Option<ModelDrawState>,
    pub drawable: Option<Arc<RwLock<Drawable>>>,
    pub expiration_frame: Option<UnsignedInt>,
}

static DRAWABLE_STATE: Lazy<Mutex<HashMap<u32, DrawableState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static TERRAIN_TREE_STATE: Lazy<Mutex<HashMap<u32, TerrainTreeRegistration>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Bridge trait for camera view operations.
///
/// Implemented by GameClient to forward calls to the real `View` struct
/// (which lives in GameClient and cannot be imported by GameLogic).
///
/// All methods use `&self` because the implementation uses interior mutability
/// (the real View is accessed via `with_tactical_view` which uses a thread-local
/// `RefCell`).
///
/// Integer types are used for enums (`CameraLockType`, `FilterType`, `FilterMode`)
/// so that GameLogic does not need to import GameClient enum types. Values must
/// match the C++ enum ordering exactly.
pub trait CameraViewBridge: Send + Sync {
    fn set_camera_lock(&self, id: Option<u32>);
    fn set_snap_mode(&self, lock_type: i32, distance: f32);
    fn snap_to_camera_lock(&self);
    fn move_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        ms: i32,
        shutter: i32,
        enabled: bool,
        ease_in: f32,
        ease_out: f32,
    );
    fn zoom_camera(&self, zoom: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn pitch_camera(&self, pitch: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn rotate_camera(&self, rotations: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn camera_mod_look_toward(&self, x: f32, y: f32, z: f32);
    fn camera_mod_final_look_toward(&self, x: f32, y: f32, z: f32);
    fn camera_mod_final_pitch(&self, pitch: f32, ease_in: f32, ease_out: f32);
    fn camera_mod_final_zoom(&self, zoom: f32, ease_in: f32, ease_out: f32);
    fn camera_mod_freeze_time(&self);
    fn camera_mod_freeze_angle(&self);
    fn set_default_view(&self, pitch: f32, angle: f32, max_height: f32);
    fn reset_camera(&self, x: f32, y: f32, z: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn look_at(&self, x: f32, y: f32, z: f32);
    fn set_view_filter(&self, filter_type: i32) -> bool;
    fn set_view_filter_mode(&self, mode: i32) -> bool;
    fn set_view_filter_pos(&self, x: f32, y: f32, z: f32);
    fn rotate_camera_toward_object(
        &self,
        object_id: u32,
        milliseconds: i32,
        hold_milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    );
    fn rotate_camera_toward_position(
        &self,
        x: f32,
        y: f32,
        z: f32,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
        reverse: bool,
    );
}

static CAMERA_VIEW_BRIDGE: OnceLock<Arc<dyn CameraViewBridge>> = OnceLock::new();

pub fn register_camera_view_bridge(bridge: Arc<dyn CameraViewBridge>) -> bool {
    CAMERA_VIEW_BRIDGE.set(bridge).is_ok()
}

pub fn get_camera_view_bridge() -> Option<&'static Arc<dyn CameraViewBridge>> {
    CAMERA_VIEW_BRIDGE.get()
}

/// Game client bridge for drawables/scorch marks and visual effects
pub struct TheGameClient;

impl TheGameClient {
    pub fn get() -> Option<&'static Self> {
        static CLIENT: OnceLock<TheGameClient> = OnceLock::new();
        Some(CLIENT.get_or_init(|| TheGameClient))
    }

    pub fn register_camera_view_bridge(bridge: Arc<dyn CameraViewBridge>) -> bool {
        register_camera_view_bridge(bridge)
    }

    pub fn get_camera_view_bridge() -> Option<&'static Arc<dyn CameraViewBridge>> {
        get_camera_view_bridge()
    }

    /// Synchronize the client frame counter with the logic frame.
    ///
    /// ## C++ Reference: GameLogic.cpp line 3596
    /// C++: TheGameClient->setFrame(now);
    pub fn set_frame(&self, _frame: UnsignedInt) {
        // In the full implementation, this would sync the drawable/camera
        // frame counter so client-side animations and effects advance
        // in lock-step with the simulation. The current Rust client-side
        // does not maintain a separate frame counter.
        let _ = _frame; // suppress unused warning until full implementation
    }

    pub fn notify_terrain_object_moved(&self, object_id: ObjectID) {
        log::debug!("GameClient::notify_terrain_object_moved({})", object_id);
    }

    pub fn create_drawable(&self, template: &dyn crate::common::ThingTemplate) -> u32 {
        let id = Drawable::allocate_drawable_id();
        let beam_width = template
            .as_any()
            .downcast_ref::<EngineThingTemplateAdapter>()
            .and_then(|adapter| {
                adapter.draw_modules.iter().find_map(|entry| {
                    if entry.name.as_str().eq_ignore_ascii_case("W3DLaserDraw") {
                        entry
                            .data
                            .as_any()
                            .downcast_ref::<W3DLaserDrawModuleData>()
                            .map(|data| data.outer_beam_width * 0.5)
                    } else {
                        None
                    }
                })
            });

        let model_name = template.get_model_name().to_string();
        let drawable_type = if template.is_kind_of(KindOf::Structure) {
            DrawableType::Static
        } else {
            DrawableType::Animated
        };
        let drawable = Arc::new(RwLock::new(Drawable::new(
            id,
            INVALID_ID,
            model_name,
            drawable_type,
        )));

        let module_thing: Arc<dyn ModuleThing> = Arc::new(DrawableThingHandle::new(&drawable));
        let mut drawable_modules: Vec<(
            ModuleInterfaceType,
            AsciiString,
            AsciiString,
            Arc<dyn ModuleData>,
            Box<dyn Module>,
        )> = Vec::new();

        if let Ok(factory_guard) = get_module_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                for entry in template.get_draw_module_info().iter() {
                    let module_name = entry.name.clone();
                    let module_data = Arc::clone(&entry.data);
                    let module_data_for_entry = Arc::clone(&module_data);
                    let interface_mask = entry.interface_flags();

                    if factory.find_module_interface_mask(&module_name, ModuleType::Draw)
                        == ModuleInterfaceType::NONE
                    {
                        continue;
                    }

                    if let Ok(module) = factory.new_module(
                        module_thing.clone(),
                        &module_name,
                        module_data,
                        ModuleType::Draw,
                    ) {
                        drawable_modules.push((
                            interface_mask,
                            module_name.clone(),
                            entry.module_tag.clone(),
                            module_data_for_entry,
                            module,
                        ));
                    }
                }

                for entry in template.get_client_update_module_info().iter() {
                    let module_name = entry.name.clone();
                    let module_data = Arc::clone(&entry.data);
                    let module_data_for_entry = Arc::clone(&module_data);
                    let interface_mask = entry.interface_flags();

                    if factory.find_module_interface_mask(&module_name, ModuleType::ClientUpdate)
                        == ModuleInterfaceType::NONE
                    {
                        continue;
                    }

                    if let Ok(module) = factory.new_module(
                        module_thing.clone(),
                        &module_name,
                        module_data,
                        ModuleType::ClientUpdate,
                    ) {
                        drawable_modules.push((
                            interface_mask,
                            module_name.clone(),
                            entry.module_tag.clone(),
                            module_data_for_entry,
                            module,
                        ));
                    }
                }
            }
        }

        if !drawable_modules.is_empty() {
            if let Ok(mut guard) = drawable.write() {
                for (interface_mask, name, tag, module_data, module) in drawable_modules {
                    let _ = guard.add_module(interface_mask, name, tag, module_data, module);
                }
            }
        }
        let mut map = DRAWABLE_STATE.lock().unwrap();
        map.insert(
            id,
            DrawableState {
                template_name: template.get_name().to_string(),
                indicator_color: Color::default(),
                position: Coord3D::ZERO,
                orientation: 0.0,
                shroud_status_object_id: INVALID_ID,
                beam_start: None,
                beam_end: None,
                beam_width,
                projectile_stream: None,
                model_draw: None,
                drawable: Some(Arc::clone(&drawable)),
                expiration_frame: None,
            },
        );
        id
    }

    pub fn destroy_drawable(&self, id: u32) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        map.remove(&id);
        drop(map);

        let mut tree_map = TERRAIN_TREE_STATE.lock().unwrap();
        let removed = tree_map.remove(&id).is_some();
        drop(tree_map);

        if removed {
            if let Some(hook) = get_terrain_tree_hook() {
                hook(TerrainTreeEvent::Remove(id));
            }
        }
    }

    pub fn set_drawable_indicator_color(&self, id: u32, color: Color) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.indicator_color = color;
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    guard.set_indicator_color(color);
                }
            }
        }
    }

    pub fn set_drawable_position(&self, id: u32, position: &Coord3D) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.position = *position;
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    guard.set_transform(Matrix3D::from_translation(*position));
                }
            }
        }
    }

    pub fn set_drawable_orientation(&self, id: u32, orientation: Real) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.orientation = orientation;
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    let translation = guard.get_position();
                    let rotation = glam::Quat::from_rotation_z(orientation);
                    let transform = Matrix3D::from_scale_rotation_translation(
                        glam::Vec3::ONE,
                        rotation,
                        glam::Vec3::new(translation.x, translation.y, translation.z),
                    );
                    guard.set_transform(transform);
                }
            }
        }
    }

    pub fn set_drawable_hidden(&self, id: u32, hidden: bool) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    let _ = guard.set_drawable_hidden(hidden);
                }
            }
        }
    }

    pub fn set_drawable_shroud_status_object_id(&self, id: u32, object_id: ObjectID) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.shroud_status_object_id = object_id;
        }
    }

    pub fn set_drawable_beam(&self, id: u32, start: &Coord3D, end: &Coord3D) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.beam_start = Some(*start);
            state.beam_end = Some(*end);
        }
    }

    pub fn set_drawable_projectile_stream(
        &self,
        id: u32,
        lines: Vec<Vec<Coord3D>>,
        texture_name: AsciiString,
        width: Real,
        tile_factor: Real,
        scroll_rate: Real,
    ) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.projectile_stream = Some(ProjectileStreamState {
                lines,
                texture_name,
                width,
                tile_factor,
                scroll_rate,
            });
        }
    }

    pub fn get_drawable_projectile_stream(&self, id: u32) -> Option<ProjectileStreamState> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id)
            .and_then(|state| state.projectile_stream.clone())
    }

    pub fn set_drawable_model_draw(&self, id: u32, model_draw: ModelDrawState) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.model_draw = Some(model_draw);
        }
    }

    pub fn get_drawable_model_draw(&self, id: u32) -> Option<ModelDrawState> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id).and_then(|state| state.model_draw.clone())
    }

    pub fn find_drawable_by_id(&self, id: u32) -> Option<DrawableState> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id).cloned()
    }

    pub fn get_drawable_beam_width(&self, id: u32) -> Option<Real> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id).and_then(|state| state.beam_width)
    }

    pub fn get_drawable_arc(&self, id: u32) -> Option<Arc<RwLock<Drawable>>> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id)
            .and_then(|state| state.drawable.as_ref().cloned())
    }

    pub fn set_drawable_expiration_date(&self, id: u32, frame: UnsignedInt) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.expiration_frame = Some(frame);
        }
    }

    pub fn update_drawables(&self, frame: UnsignedInt) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        let expired: Vec<u32> = map
            .iter()
            .filter_map(|(id, state)| {
                if let Some(expiration) = state.expiration_frame {
                    if frame >= expiration {
                        return Some(*id);
                    }
                }
                None
            })
            .collect();

        for id in expired {
            map.remove(&id);
            let removed_tree = {
                let mut tree_map = TERRAIN_TREE_STATE.lock().unwrap();
                tree_map.remove(&id).is_some()
            };
            if removed_tree {
                if let Some(hook) = get_terrain_tree_hook() {
                    hook(TerrainTreeEvent::Remove(id));
                }
            }
        }
    }

    pub fn add_scorch(&self, _pos: &Coord3D, _size: Real, _scorch_type: i32) {
        if let Some(hook) = get_scorch_hook() {
            hook(_pos, _size, _scorch_type);
        }
    }

    pub fn get_animation_duration_ms(&self, animation_name: &str) -> Option<Real> {
        let hook = get_animation_metadata_hook()?;
        if animation_name.trim().is_empty() {
            return None;
        }
        hook(animation_name)
    }

    pub fn add_tree(
        &self,
        drawable_id: u32,
        location: &Coord3D,
        scale: Real,
        angle: Real,
        random_scale_amount: Real,
        module_data: &W3DTreeDrawModuleData,
    ) {
        if drawable_id == INVALID_ID {
            return;
        }

        let registration = TerrainTreeRegistration {
            drawable_id,
            location: *location,
            scale,
            angle,
            random_scale_amount,
            module_data: module_data.clone(),
        };

        let mut tree_map = TERRAIN_TREE_STATE.lock().unwrap();
        tree_map.insert(drawable_id, registration.clone());
        drop(tree_map);

        if let Some(hook) = get_terrain_tree_hook() {
            hook(TerrainTreeEvent::Add(registration));
        }
    }

    pub fn get_registered_tree(&self, drawable_id: u32) -> Option<TerrainTreeRegistration> {
        let tree_map = TERRAIN_TREE_STATE.lock().ok()?;
        tree_map.get(&drawable_id).cloned()
    }
}

// Make model condition mask macro (matching C++ MAKE_MODELCONDITION_MASK)

/// Firing tracker for weapon firing statistics (matching C++ FiringTracker).
#[derive(Debug)]
pub struct FiringTracker {
    object_id: ObjectID,
    consecutive_shots: i32,
    victim_id: ObjectID,
    frame_to_start_cooldown: UnsignedInt,
    frame_to_force_reload: UnsignedInt,
    frame_to_stop_looping_sound: UnsignedInt,
    audio_handle: crate::common::audio::AudioHandle,
    last_shot_frame: UnsignedInt,
}

impl Drop for FiringTracker {
    fn drop(&mut self) {
        if self.audio_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.audio_handle);
            }
            self.audio_handle = 0;
        }
    }
}

impl FiringTracker {
    pub fn new(object_id: ObjectID) -> Self {
        if object_id != INVALID_ID {
            TheGameLogic::set_wake_frame(object_id, crate::modules::UPDATE_SLEEP_FOREVER);
        }

        Self {
            object_id,
            consecutive_shots: 0,
            victim_id: INVALID_ID,
            frame_to_start_cooldown: 0,
            frame_to_force_reload: 0,
            frame_to_stop_looping_sound: 0,
            audio_handle: 0,
            last_shot_frame: 0,
        }
    }

    pub fn get_last_shot_frame(&self) -> UnsignedInt {
        self.last_shot_frame
    }

    pub fn get_last_shot_victim(&self) -> ObjectID {
        self.victim_id
    }

    pub fn get_num_consecutive_shots_at_victim(&self, victim_id: ObjectID) -> i32 {
        if victim_id != INVALID_ID && victim_id == self.victim_id {
            self.consecutive_shots
        } else {
            0
        }
    }

    pub fn shot_fired(&mut self, weapon: &crate::weapon::Weapon, victim_id: ObjectID) {
        let now = TheGameLogic::get_frame();
        self.last_shot_frame = now;

        let Some(owner_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };

        let mut owner_guard = match owner_arc.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        let victim_has_faerie_fire = TheGameLogic::find_object_by_id(victim_id)
            .map(|victim| {
                victim
                    .read()
                    .ok()
                    .map(|victim_guard| {
                        victim_guard.test_status(crate::common::ObjectStatusTypes::FaerieFire)
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        if victim_has_faerie_fire {
            if !owner_guard
                .get_weapon_bonus_condition()
                .contains(crate::common::types::WeaponBonusConditionFlags::TARGET_FAERIE_FIRE)
            {
                owner_guard.set_weapon_bonus_condition(
                    crate::common::types::WeaponBonusConditionType::TargetFaerieFire,
                );
            }
        } else if owner_guard
            .get_weapon_bonus_condition()
            .contains(crate::common::types::WeaponBonusConditionFlags::TARGET_FAERIE_FIRE)
        {
            owner_guard.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::TargetFaerieFire,
            );
        }

        if victim_id == self.victim_id {
            self.consecutive_shots += 1;
        } else if now < self.frame_to_start_cooldown {
            self.consecutive_shots += 1;
            self.victim_id = victim_id;
        } else {
            self.consecutive_shots = 1;
            self.victim_id = victim_id;
        }

        let template = weapon.get_template();
        if template.auto_reload_when_idle_frames > 0 {
            self.frame_to_force_reload = now.saturating_add(template.auto_reload_when_idle_frames);
        }

        if template.continuous_fire_coast_frames > 0 {
            self.frame_to_start_cooldown = weapon
                .get_possible_next_shot_frame()
                .saturating_add(template.continuous_fire_coast_frames);
        } else {
            self.frame_to_start_cooldown = 0;
        }

        let shots_needed_one = template.continuous_fire_one_shots_needed;
        let shots_needed_two = template.continuous_fire_two_shots_needed;

        let bonus_flags = owner_guard.get_weapon_bonus_condition();
        if bonus_flags
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN)
        {
            if self.consecutive_shots < shots_needed_one {
                self.cool_down(&mut owner_guard);
            } else if self.consecutive_shots > shots_needed_two {
                self.speed_up(&mut owner_guard);
            }
        } else if bonus_flags
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST)
        {
            if self.consecutive_shots < shots_needed_two {
                self.cool_down(&mut owner_guard);
            }
        } else if self.consecutive_shots > shots_needed_one {
            self.speed_up(&mut owner_guard);
        }

        let fire_sound_loop_time = template.fire_sound_loop_time;
        if fire_sound_loop_time != 0 {
            let mut needs_restart = self.frame_to_stop_looping_sound == 0;
            if !needs_restart {
                if self.audio_handle == 0 {
                    needs_restart = true;
                } else {
                    let _manager =
                        get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
                    if self.audio_handle == 0 {
                        needs_restart = true;
                    }
                }
            }

            if needs_restart {
                let sound = template.fire_sound.clone();
                if !sound.is_empty() {
                    let mut event = AudioEventRts::new(sound.name().to_string());
                    event.set_object_id(self.object_id);
                    if let Some(audio) = TheAudio::get() {
                        self.audio_handle = audio.add_audio_event(&event);
                    }
                }
            }
            self.frame_to_stop_looping_sound =
                now.saturating_add(fire_sound_loop_time as UnsignedInt);
        } else {
            let sound = template.fire_sound.clone();
            if !sound.is_empty() {
                let mut event = AudioEventRts::new(sound.name().to_string());
                event.set_object_id(self.object_id);
                if let Some(audio) = TheAudio::get() {
                    audio.add_audio_event(&event);
                }
            }
            self.frame_to_stop_looping_sound = 0;
        }

        let sleep_time = self.calc_time_to_sleep(now);
        TheGameLogic::set_wake_frame(self.object_id, sleep_time);
    }

    pub fn update(&mut self) -> crate::modules::UpdateSleepTime {
        let now = TheGameLogic::get_frame();

        if self.frame_to_force_reload != 0 && now >= self.frame_to_force_reload {
            if let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) {
                if let Ok(mut guard) = owner.write() {
                    let _ = guard.reload_all_ammo(true);
                }
            }
            self.frame_to_force_reload = 0;
        }

        if self.frame_to_stop_looping_sound != 0 && now >= self.frame_to_stop_looping_sound {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.audio_handle);
            }
            self.audio_handle = 0;
            self.frame_to_stop_looping_sound = 0;
        }

        if self.frame_to_start_cooldown != 0 && now > self.frame_to_start_cooldown {
            self.frame_to_start_cooldown =
                now.saturating_add(crate::common::LOGICFRAMES_PER_SECOND);
            if let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) {
                if let Ok(mut guard) = owner.write() {
                    self.cool_down(&mut guard);
                }
            }
            return crate::modules::UpdateSleepTime::Frames(crate::common::LOGICFRAMES_PER_SECOND);
        }

        self.calc_time_to_sleep(now)
    }

    fn calc_time_to_sleep(&self, now: UnsignedInt) -> crate::modules::UpdateSleepTime {
        if self.frame_to_stop_looping_sound == 0
            && self.frame_to_start_cooldown == 0
            && self.frame_to_force_reload == 0
        {
            return crate::modules::UpdateSleepTime::Forever;
        }

        let mut sleep_time = u32::MAX;

        if self.frame_to_stop_looping_sound != 0 {
            if self.frame_to_stop_looping_sound <= now {
                sleep_time = 0;
            } else {
                sleep_time = sleep_time.min(self.frame_to_stop_looping_sound - now);
            }
        }

        if self.frame_to_start_cooldown != 0 {
            if self.frame_to_start_cooldown <= now {
                sleep_time = 0;
            } else {
                sleep_time = sleep_time.min(self.frame_to_start_cooldown - now);
            }
        }

        if self.frame_to_force_reload != 0 {
            if self.frame_to_force_reload <= now {
                sleep_time = 0;
            } else {
                sleep_time = sleep_time.min(self.frame_to_force_reload - now);
            }
        }

        crate::modules::UpdateSleepTime::from_u32(sleep_time)
    }

    fn speed_up(&mut self, owner: &mut Object) {
        let clear = crate::common::ModelConditionFlags::empty();
        let set = crate::common::ModelConditionFlags::empty();

        if owner
            .get_weapon_bonus_condition()
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST)
        {
            // Already at max speed, nothing to do.
        } else if owner
            .get_weapon_bonus_condition()
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN)
        {
            if let Some(mut sound) = owner.get_template().get_per_unit_sound("VoiceRapidFire") {
                sound.set_object_id(self.object_id);
                if let Some(audio) = TheAudio::get() {
                    audio.add_audio_event(&sound);
                }
            }

            owner.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
        } else {
            owner.set_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
        }

        let _ = owner.clear_and_set_model_condition_flags(clear, set);
    }

    fn cool_down(&mut self, owner: &mut Object) {
        let clear = crate::common::ModelConditionFlags::empty();
        let set = crate::common::ModelConditionFlags::empty();

        let bonus_flags = owner.get_weapon_bonus_condition();
        if bonus_flags
            .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_FAST)
            || bonus_flags
                .contains(crate::common::types::WeaponBonusConditionFlags::CONTINUOUS_FIRE_MEAN)
        {
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
        } else {
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireFast,
            );
            owner.clear_weapon_bonus_condition(
                crate::common::types::WeaponBonusConditionType::ContinuousFireMean,
            );
            self.frame_to_start_cooldown = 0;
        }

        let _ = owner.clear_and_set_model_condition_flags(clear, set);

        self.consecutive_shots = 0;
        self.victim_id = INVALID_ID;
    }
}

/// Object held helper (matching C++ ObjectHeldHelper)
#[derive(Debug)]
pub struct ObjectHeldHelper {
    is_held: bool,
    holder_id: ObjectID,
}

impl ObjectHeldHelper {
    pub fn new() -> Self {
        Self {
            is_held: false,
            holder_id: INVALID_ID,
        }
    }

    pub fn is_held(&self) -> bool {
        self.is_held
    }

    pub fn set_held(&mut self, held: bool, holder_id: ObjectID) {
        self.is_held = held;
        self.holder_id = if held { holder_id } else { INVALID_ID };
    }
}

/// Object disabled helper (matching C++ ObjectDisabledHelper)
#[derive(Debug)]
pub struct ObjectDisabledHelper {
    disabled_mask: DisabledMaskType,
    disabled_until: [UnsignedInt; DISABLED_COUNT],
}

impl ObjectDisabledHelper {
    pub fn new() -> Self {
        Self {
            disabled_mask: DisabledMaskType::none(),
            disabled_until: [NEVER; DISABLED_COUNT],
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled_mask.any()
    }

    pub fn set_disabled(&mut self, disabled_type: DisabledType, _until_frame: UnsignedInt) {
        self.disabled_mask.set_disabled(disabled_type);
        // Set the frame when this disability expires
    }

    pub fn clear_disabled(&mut self, disabled_type: DisabledType) {
        self.disabled_mask.clear(disabled_type);
    }
}

/// TheWeaponStore singleton - weapon management system (matching C++ TheWeaponStore)
pub struct TheWeaponStore;

impl TheWeaponStore {
    /// Get the weapon store instance
    pub fn get() -> Option<Self> {
        Some(Self)
    }

    /// Create and fire a temporary weapon at a position
    pub fn create_and_fire_temp_weapon_at_pos(
        &self,
        weapon_template: &Arc<crate::weapon::WeaponTemplate>,
        source_id: ObjectID,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let result = crate::weapon::with_weapon_store(|store| {
            store.create_and_fire_temp_weapon(weapon_template, source_id, None, Some(position))
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(format!("{:?}", err).into()),
            Err(err) => Err(format!("{:?}", err).into()),
        }
    }

    /// Create and fire a temporary weapon
    pub fn create_and_fire_temp_weapon(
        weapon_name: &str,
        source: &crate::object::Object,
        position: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let template = crate::weapon::with_weapon_store(|store| {
            store.find_weapon_template(weapon_name).cloned()
        })
        .map_err(|err| format!("{:?}", err))?;

        let Some(template) = template else {
            return Err(format!("Weapon template '{}' not found", weapon_name).into());
        };

        let source_id = source.get_id();
        let result = crate::weapon::with_weapon_store(|store| {
            store.create_and_fire_temp_weapon(&template, source_id, None, Some(position))
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(format!("{:?}", err).into()),
            Err(err) => Err(format!("{:?}", err).into()),
        }
    }
}

/// TheGameLODManager singleton - level of detail management (matching C++ TheGameLODManager)
pub struct TheGameLODManager;

impl TheGameLODManager {
    /// Get slow death scale factor (matches GameLOD.ini DynamicGameLOD::SlowDeathScale)
    pub fn get_slow_death_scale() -> Real {
        game_engine::common::game_lod::get_slow_death_scale() as Real
    }
}

/// Hooks for GameClient integration during prepareNewGame.
pub trait PrepareNewGameHooks: Send + Sync {
    fn ensure_background_window(&self);
    fn hide_shell(&self);
}

static PREPARE_NEW_GAME_HOOKS: OnceLock<Arc<dyn PrepareNewGameHooks>> = OnceLock::new();

/// Hooks provided by GameClient so pause transitions can mirror the original
/// C++ cursor/input restore behavior.
pub trait GamePauseHooks: Send + Sync {
    fn on_game_pause_state_changed(&self, paused: Bool);
}

static GAME_PAUSE_HOOKS: OnceLock<Arc<dyn GamePauseHooks>> = OnceLock::new();

/// Hooks provided by GameClient so gameplay-side audio locality can query
/// observer camera focus when the local player is dead/spectating.
pub trait ObserverAudioLocalityHooks: Send + Sync {
    fn get_observer_look_at_player_index(&self) -> Option<Int>;
}

/// Hooks provided by GameClient so gameplay-side audio view resolver can pull
/// tactical-view/camera state instead of using zeroed placeholders.
pub trait ObserverAudioViewHooks: Send + Sync {
    fn get_tactical_view_position(&self) -> Option<(Real, Real, Real)>;
    fn get_tactical_view_angle(&self) -> Option<Real>;
    fn get_3d_camera_position(&self) -> Option<(Real, Real, Real)>;
}

static OBSERVER_AUDIO_LOCALITY_HOOKS: OnceLock<Arc<dyn ObserverAudioLocalityHooks>> =
    OnceLock::new();
static OBSERVER_AUDIO_VIEW_HOOKS: OnceLock<Arc<dyn ObserverAudioViewHooks>> = OnceLock::new();

pub fn register_prepare_new_game_hooks(hooks: Arc<dyn PrepareNewGameHooks>) -> bool {
    PREPARE_NEW_GAME_HOOKS.set(hooks).is_ok()
}

fn prepare_new_game_hooks() -> Option<&'static Arc<dyn PrepareNewGameHooks>> {
    PREPARE_NEW_GAME_HOOKS.get()
}

pub fn register_game_pause_hooks(hooks: Arc<dyn GamePauseHooks>) -> bool {
    GAME_PAUSE_HOOKS.set(hooks).is_ok()
}

fn game_pause_hooks() -> Option<&'static Arc<dyn GamePauseHooks>> {
    GAME_PAUSE_HOOKS.get()
}

pub fn register_observer_audio_locality_hooks(hooks: Arc<dyn ObserverAudioLocalityHooks>) -> bool {
    OBSERVER_AUDIO_LOCALITY_HOOKS.set(hooks).is_ok()
}

fn observer_audio_locality_hooks() -> Option<&'static Arc<dyn ObserverAudioLocalityHooks>> {
    OBSERVER_AUDIO_LOCALITY_HOOKS.get()
}

pub fn register_observer_audio_view_hooks(hooks: Arc<dyn ObserverAudioViewHooks>) -> bool {
    OBSERVER_AUDIO_VIEW_HOOKS.set(hooks).is_ok()
}

fn observer_audio_view_hooks() -> Option<&'static Arc<dyn ObserverAudioViewHooks>> {
    OBSERVER_AUDIO_VIEW_HOOKS.get()
}

use game_engine::common::system::scene_submission::{SceneLineDesc, SceneLineId, SceneSubmission};

static SCENE_SUBMISSION: OnceLock<Arc<dyn SceneSubmission>> = OnceLock::new();

pub fn register_scene_submission(impl_: Arc<dyn SceneSubmission>) -> bool {
    SCENE_SUBMISSION.set(impl_).is_ok()
}

fn get_scene_submission() -> Option<&'static Arc<dyn SceneSubmission>> {
    SCENE_SUBMISSION.get()
}

pub fn submit_scene_line(drawable_id: u32, desc: &SceneLineDesc) -> Option<SceneLineId> {
    get_scene_submission().and_then(|s| s.submit_line(drawable_id, desc))
}

pub fn update_scene_line(id: SceneLineId, desc: &SceneLineDesc) {
    if let Some(s) = get_scene_submission() {
        s.update_line(id, desc);
    }
}

pub fn remove_scene_line(id: SceneLineId) {
    if let Some(s) = get_scene_submission() {
        s.remove_line(id);
    }
}

/// Global data singleton (matches C++ TheGlobalData)
pub struct TheGlobalData;

impl TheGlobalData {
    pub fn get() -> Option<&'static Self> {
        let _ = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        static GLOBAL: TheGlobalData = TheGlobalData;
        Some(&GLOBAL)
    }

    pub fn get_max_tunnel_capacity(&self) -> i32 {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.max_tunnel_capacity
    }

    pub fn get_base_regen_health_percent_per_second(&self) -> Real {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.base_regen_health_percent_per_second
    }

    pub fn get_base_regen_delay(&self) -> UnsignedInt {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.base_regen_delay
    }

    pub fn get_special_power_view_object_name(&self) -> String {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.special_power_view_object_name.clone()
    }

    /// Check if special powers use delay (matches C++ TheGlobalData->m_specialPowerUsesDelay)
    /// When false (debug/cheat mode), all special powers are instantly ready
    pub fn get_special_power_uses_delay(&self) -> bool {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.special_power_uses_delay
    }

    /// Prison bounty multiplier (matches GlobalData::m_prisonBountyMultiplier).
    pub fn get_prison_bounty_multiplier(&self) -> Real {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let guard = data.read();
        guard.prison_bounty_multiplier
    }

    /// Prison bounty floating text color (matches GlobalData::m_prisonBountyTextColor).
    pub fn get_prison_bounty_text_color(&self) -> crate::common::Color {
        let data = get_engine_global_data().unwrap_or_else(ensure_engine_global_data);
        let color = data.read().prison_bounty_text_color;
        crate::common::Color::rgb(
            (color.r.clamp(0.0, 1.0) * 255.0) as u8,
            (color.g.clamp(0.0, 1.0) * 255.0) as u8,
            (color.b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    pub fn get_shroud_alpha(&self) -> u8 {
        get_engine_global_data()
            .map(|data| data.read().shroud_alpha)
            .unwrap_or(0)
    }

    pub fn get_clear_alpha(&self) -> u8 {
        get_engine_global_data()
            .map(|data| data.read().clear_alpha)
            .unwrap_or(255)
    }

    pub fn get_time_of_day(&self) -> TimeOfDay {
        if let Some(data) = get_engine_global_data() {
            return map_time_of_day(data.read().time_of_day);
        }

        let guard = GLOBAL_TIME_OF_DAY.lock().unwrap();
        *guard
    }

    pub fn set_time_of_day(&self, value: TimeOfDay) {
        if let Some(data) = get_engine_global_data() {
            let mut guard = data.write();
            let mapped = match value {
                TimeOfDay::Morning => IniTimeOfDay::Morning,
                TimeOfDay::Evening => IniTimeOfDay::Evening,
                TimeOfDay::Night => IniTimeOfDay::Night,
                TimeOfDay::Day => IniTimeOfDay::Afternoon,
            };
            guard.set_time_of_day(mapped);
        }

        let mut guard = GLOBAL_TIME_OF_DAY.lock().unwrap();
        *guard = value;

        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_arc.write() else {
                continue;
            };
            if let Some(drawable) = obj_guard.get_drawable() {
                if let Ok(mut draw_guard) = drawable.write() {
                    draw_guard.set_time_of_day(value);
                    draw_guard.changed_team(&obj_guard);
                }
            }
        }
    }
}

static GLOBAL_TIME_OF_DAY: Lazy<Mutex<TimeOfDay>> = Lazy::new(|| Mutex::new(TimeOfDay::Day));

fn map_time_of_day(value: IniTimeOfDay) -> TimeOfDay {
    match value {
        IniTimeOfDay::Invalid => TimeOfDay::Day,
        IniTimeOfDay::Morning => TimeOfDay::Morning,
        IniTimeOfDay::Evening => TimeOfDay::Evening,
        IniTimeOfDay::Night => TimeOfDay::Night,
        IniTimeOfDay::Afternoon => TimeOfDay::Day,
    }
}

fn map_audio_time_of_day(value: TimeOfDay) -> EngineTimeOfDay {
    match value {
        TimeOfDay::Morning => EngineTimeOfDay::Morning,
        TimeOfDay::Evening => EngineTimeOfDay::Evening,
        TimeOfDay::Night => EngineTimeOfDay::Night,
        TimeOfDay::Day => EngineTimeOfDay::Day,
    }
}

/// TheThingFactory singleton - object creation factory (matching C++ TheThingFactory)
pub struct TheThingFactory;

impl TheThingFactory {
    /// Get a reference to TheThingFactory
    pub fn get() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    fn resolve_build_variation_name(
        template: &std::sync::Arc<dyn crate::common::ThingTemplate>,
    ) -> String {
        let mut template_name = template.get_name().to_string();
        let adapter = template
            .as_any()
            .downcast_ref::<EngineThingTemplateAdapter>();
        if let Some(adapter) = adapter {
            let variations = adapter.inner.get_build_variations();
            if !variations.is_empty() {
                let max = (variations.len().saturating_sub(1)) as Int;
                let index = if max == 0 {
                    0
                } else {
                    get_game_logic_random_value(0, max)
                } as usize;
                if let Some(candidate) = variations.get(index) {
                    if let Some(variation) = Self::find_template(candidate.as_str()) {
                        template_name = variation.get_name().to_string();
                    }
                }
            }
        }
        template_name
    }

    /// Returns true if `candidate_name` is one of `template`'s build variations.
    /// Mirrors C++ Team.cpp helper `isInBuildVariations(...)`.
    pub fn has_build_variation_name(
        template: &std::sync::Arc<dyn crate::common::ThingTemplate>,
        candidate_name: &str,
    ) -> bool {
        let Some(adapter) = template
            .as_any()
            .downcast_ref::<EngineThingTemplateAdapter>()
        else {
            return false;
        };

        adapter
            .inner
            .get_build_variations()
            .iter()
            .any(|variation| variation.as_str() == candidate_name)
    }

    /// Find template by name using the shared ThingFactory.
    pub fn find_template(name: impl AsRef<str>) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let key = crate::common::AsciiString::from(name.as_ref());
        let mut should_retry_init = true;
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                should_retry_init = factory.first_template().is_none();
                if let Some(template) = factory.find_template(key.as_str(), false) {
                    return Some(Arc::new(EngineThingTemplateAdapter::new(template)));
                }
            } else {
                should_retry_init = true;
            }
        }

        // The original C++ runtime assumes the global thing database is available when scripts
        // start creating units. Retry once by rebuilding the shared factory on demand only when
        // the factory appears uninitialized to avoid expensive reloads on normal misses.
        if should_retry_init && init_thing_factory().is_ok() {
            if let Ok(factory_guard) = get_thing_factory() {
                if let Some(factory) = factory_guard.as_ref() {
                    if let Some(template) = factory.find_template(key.as_str(), false) {
                        return Some(Arc::new(EngineThingTemplateAdapter::new(template)));
                    }
                }
            }
        }

        None
    }

    /// Find template by ID using the shared ThingFactory.
    pub fn find_template_by_id(id: u32) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let Ok(factory_guard) = get_thing_factory() else {
            return None;
        };
        let Some(factory) = factory_guard.as_ref() else {
            return None;
        };
        let template = factory.find_by_template_id(id as u16)?;
        Some(Arc::new(EngineThingTemplateAdapter::new(template)))
    }

    /// Create new object from template
    pub fn new_object(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: &crate::team::Team,
    ) -> Result<
        std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        use crate::object_manager::get_object_manager;
        use crate::object_manager::ObjectCreationFlags;
        use crate::team::get_team_factory;

        let template_name = Self::resolve_build_variation_name(&template);

        let team_arc = get_team_factory()
            .lock()
            .ok()
            .and_then(|factory| factory.find_team_by_id(team.get_id()));

        let object_id = get_object_manager()
            .write()
            .map_err(|_| "ObjectManager lock poisoned")?
            .create_object(
                &template_name,
                Coord3D::new(0.0, 0.0, 0.0),
                team_arc,
                ObjectCreationFlags::from_template(),
            )
            .map_err(|e| e.to_string())?;

        let instance = get_object_manager()
            .read()
            .map_err(|_| "ObjectManager lock poisoned")?
            .get_object(object_id)
            .ok_or_else(|| "Created object not found in ObjectManager".to_string())?;

        let base = instance
            .read()
            .map_err(|_| "GameObjectInstance lock poisoned")?
            .base
            .clone();

        Ok(base)
    }

    /// Create new object from template with an optional team (matches C++ NULL-team usage).
    pub fn new_object_optional_team(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: Option<&crate::team::Team>,
    ) -> Result<
        std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        use crate::object_manager::get_object_manager;
        use crate::object_manager::ObjectCreationFlags;
        use crate::team::get_team_factory;

        let template_name = Self::resolve_build_variation_name(&template);

        let team_arc = team.and_then(|team| {
            get_team_factory()
                .lock()
                .ok()
                .and_then(|factory| factory.find_team_by_id(team.get_id()))
        });

        let object_id = get_object_manager()
            .write()
            .map_err(|_| "ObjectManager lock poisoned")?
            .create_object(
                &template_name,
                Coord3D::new(0.0, 0.0, 0.0),
                team_arc,
                ObjectCreationFlags::from_template(),
            )
            .map_err(|e| e.to_string())?;

        let instance = get_object_manager()
            .read()
            .map_err(|_| "ObjectManager lock poisoned")?
            .get_object(object_id)
            .ok_or_else(|| "Created object not found in ObjectManager".to_string())?;

        let base = instance
            .read()
            .map_err(|_| "GameObjectInstance lock poisoned")?
            .base
            .clone();

        Ok(base)
    }
}

/// ThePartitionManager singleton bridge - spatial partitioning system (matching C++ ThePartitionManager)
pub struct ThePartitionManager;

/// FindPosition options for ThePartitionManager::find_position_around_with_options.
#[derive(Debug, Clone)]
pub struct FindPositionOptions {
    pub min_radius: Real,
    pub max_radius: Real,
    pub start_angle: Option<Real>,
    pub max_z_delta: Real,
    pub flags: u32,
    pub relationship_object_id: Option<ObjectID>,
    pub ignore_object_id: Option<ObjectID>,
    pub source_to_path_to_dest_id: Option<ObjectID>,
}

impl Default for FindPositionOptions {
    fn default() -> Self {
        Self {
            min_radius: 0.0,
            max_radius: 0.0,
            start_angle: None,
            max_z_delta: 99999.0,
            flags: 0,
            relationship_object_id: None,
            ignore_object_id: None,
            source_to_path_to_dest_id: None,
        }
    }
}

pub const FPF_NONE: u32 = 0x00;
pub const FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS: u32 = 0x01;
pub const FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES: u32 = 0x02;
pub const FPF_IGNORE_ENEMY_UNITS: u32 = 0x04;
pub const FPF_IGNORE_ENEMY_STRUCTURES: u32 = 0x08;
pub const FPF_IGNORE_ALL_OBJECTS: u32 = 0x10;
pub const FPF_IGNORE_WATER: u32 = 0x20;
pub const FPF_WATER_ONLY: u32 = 0x40;
pub const FPF_CLEAR_CELLS_ONLY: u32 = 0x80;
pub const FPF_USE_HIGHEST_LAYER: u32 = 0x100;

#[derive(Debug, Default)]
pub struct ThePartitionManagerBridge;

impl ThePartitionManager {
    pub fn get() -> Option<&'static Self> {
        static PARTITION: OnceLock<ThePartitionManager> = OnceLock::new();
        Some(PARTITION.get_or_init(|| ThePartitionManager))
    }

    /// Get objects in range.
    ///
    /// C++ uses `ThePartitionManager->iterateObjectsInRange(...)` for most radius queries.
    /// The Rust port does not yet have a single unified partition system, so we bridge through
    /// `ObjectManager`'s spatial partition and then validate against live objects in the registry.
    pub fn get_objects_in_range(
        &self,
        pos: &Coord3D,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        let candidate_ids = if let Ok(logic) = crate::system::game_logic::get_game_logic().lock() {
            logic
                .partition_manager()
                .find_objects_in_radius(*pos, radius)
        } else {
            let manager_ref = crate::object_manager::get_object_manager();
            let Ok(manager) = manager_ref.read() else {
                return Vec::new();
            };
            manager.find_objects_in_radius(*pos, radius)
        };

        let radius_sqr = radius * radius;
        candidate_ids
            .into_iter()
            .filter_map(|id| {
                let obj = OBJECT_REGISTRY.get_object(id)?;
                let obj_guard = obj.read().ok()?;
                let obj_pos = obj_guard.get_position();
                let dx = obj_pos.x - pos.x;
                let dy = obj_pos.y - pos.y;
                if dx * dx + dy * dy <= radius_sqr {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find a legal position around a point (matching C++ PartitionManager::findPositionAround).
    pub fn find_position_around(
        &self,
        center: &Coord3D,
        min_radius: Real,
        max_radius: Real,
        result: &mut Coord3D,
    ) -> bool {
        let mut options = FindPositionOptions::default();
        options.min_radius = min_radius;
        options.max_radius = max_radius;
        self.find_position_around_with_options(center, &options, result)
    }

    /// Full FindPositionAround implementation with options (closer to C++).
    pub fn find_position_around_with_options(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
        result: &mut Coord3D,
    ) -> bool {
        const RING_SPACING: Real = 5.0;
        const TWO_PI: Real = std::f32::consts::PI * 2.0;

        fn try_position<F>(
            center: &Coord3D,
            dist: Real,
            angle: Real,
            terrain: Option<&TheTerrainLogic>,
            in_extent: &F,
            options: &FindPositionOptions,
            result: &mut Coord3D,
        ) -> bool
        where
            F: Fn(&Coord3D) -> bool,
        {
            let mut pos = Coord3D::new(
                center.x + dist * angle.cos(),
                center.y + dist * angle.sin(),
                center.z,
            );

            if !in_extent(&pos) {
                return false;
            }

            if let Some(terrain) = terrain {
                let mut layer = PathfindLayerEnum::Ground;
                if (options.flags & FPF_USE_HIGHEST_LAYER) != 0 {
                    pos.z = 99999.0;
                    layer = terrain.get_highest_layer_for_destination(&pos);
                    pos.z = terrain.get_layer_height(pos.x, pos.y, layer);
                    if layer != PathfindLayerEnum::Ground {
                        pos.z += 1.0;
                    }
                } else {
                    pos.z = terrain.get_ground_height(pos.x, pos.y, None);
                }

                if (pos.z - center.z).abs() > options.max_z_delta {
                    return false;
                }

                if terrain.is_cliff_cell(pos.x, pos.y) {
                    return false;
                }

                if (options.flags & FPF_IGNORE_WATER) == 0 {
                    let underwater = terrain.is_underwater(pos.x, pos.y, None, None);
                    if (options.flags & FPF_WATER_ONLY) != 0 {
                        if !underwater {
                            return false;
                        }
                    } else if underwater {
                        return false;
                    }
                }

                if (options.flags & FPF_CLEAR_CELLS_ONLY) != 0 {
                    if let Ok(ai) = crate::ai::THE_AI.read() {
                        if let Some(ps) = ai.pathfinding_system() {
                            if let Ok(ps_guard) = ps.read() {
                                if !ps_guard.is_cell_clear_at(&pos, layer) {
                                    return false;
                                }
                            }
                        }
                    }
                }
            }

            if (options.flags & FPF_IGNORE_ALL_OBJECTS) == 0 {
                let relation_obj = options
                    .relationship_object_id
                    .and_then(|id| OBJECT_REGISTRY.get_object(id));

                for obj_arc in OBJECT_REGISTRY.get_all_objects() {
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let obj_id = obj_guard.get_id();

                    if options.ignore_object_id == Some(obj_id) {
                        continue;
                    }
                    if options.source_to_path_to_dest_id == Some(obj_id) {
                        continue;
                    }

                    if let Some(rel_arc) = &relation_obj {
                        if let Ok(rel_guard) = rel_arc.read() {
                            let relation = rel_guard.relationship_to(&obj_guard);
                            let is_unit = obj_guard.is_kind_of(KindOf::Infantry)
                                || obj_guard.is_kind_of(KindOf::Vehicle);
                            let is_structure = obj_guard.is_kind_of(KindOf::Structure);

                            if (options.flags & FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS) != 0
                                && relation != Relationship::Enemies
                                && is_unit
                            {
                                continue;
                            }
                            if (options.flags & FPF_IGNORE_ALLY_OR_NEUTRAL_STRUCTURES) != 0
                                && relation != Relationship::Enemies
                                && is_structure
                            {
                                continue;
                            }
                            if (options.flags & FPF_IGNORE_ENEMY_UNITS) != 0
                                && relation == Relationship::Enemies
                                && is_unit
                            {
                                continue;
                            }
                            if (options.flags & FPF_IGNORE_ENEMY_STRUCTURES) != 0
                                && relation == Relationship::Enemies
                                && is_structure
                            {
                                continue;
                            }
                        }
                    }

                    let obj_pos = obj_guard.get_position();
                    let dx = obj_pos.x - pos.x;
                    let dy = obj_pos.y - pos.y;
                    let radius = obj_guard.get_geometry_info().get_bounding_circle_radius() + 5.0;
                    if dx * dx + dy * dy <= radius * radius {
                        return false;
                    }
                }
            }

            if let Some(source_id) = options.source_to_path_to_dest_id {
                if let Some(source_arc) = OBJECT_REGISTRY.get_object(source_id) {
                    if let Ok(source_guard) = source_arc.read() {
                        if let Some(terrain) = terrain {
                            if !terrain.is_clear_line_of_sight(source_guard.get_position(), &pos) {
                                return false;
                            }
                        }
                    }
                }
            }

            *result = pos;
            true
        }

        let terrain = TheTerrainLogic::get();
        let extent = terrain
            .map(|t| t.get_maximum_pathfind_extent())
            .unwrap_or_else(|| crate::common::Region3D::new(*center, *center));

        let in_extent = |pos: &Coord3D| {
            pos.x >= extent.lo.x
                && pos.x <= extent.hi.x
                && pos.y >= extent.lo.y
                && pos.y <= extent.hi.y
        };

        if !in_extent(center) {
            *result = *center;
            return true;
        }

        if (options.flags & FPF_IGNORE_WATER) != 0 && (options.flags & FPF_WATER_ONLY) != 0 {
            return false;
        }

        let max_radius = if options.max_radius < options.min_radius {
            options.min_radius
        } else {
            options.max_radius
        };
        let start_angle = options
            .start_angle
            .unwrap_or_else(|| GameLogicRandomValueReal!(0.0, TWO_PI));

        let mut dist = options.min_radius;
        while dist <= max_radius {
            let angle_spacing = if dist == options.min_radius {
                TWO_PI
            } else {
                (RING_SPACING / (dist + 1.0)) * (TWO_PI / 6.0)
            };

            let samples = ((TWO_PI / angle_spacing) / 2.0).ceil() as i32;
            for i in 0..samples {
                let angle_offset = angle_spacing * i as f32;
                if try_position(
                    center,
                    dist,
                    start_angle + angle_offset,
                    terrain,
                    &in_extent,
                    options,
                    result,
                ) {
                    return true;
                }
                if i != 0
                    && try_position(
                        center,
                        dist,
                        start_angle - angle_offset,
                        terrain,
                        &in_extent,
                        options,
                        result,
                    )
                {
                    return true;
                }
            }

            dist += RING_SPACING;
        }

        false
    }

    /// Get objects in range using boundary-to-boundary distance in 2D.
    ///
    /// Mirrors C++ `FROM_BOUNDINGSPHERE_2D` distance calculation when the query
    /// is a position (no source object).
    pub fn get_objects_in_range_boundary_2d(
        &self,
        pos: &Coord3D,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        let radius_sqr = radius * radius;
        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj = obj_arc.read().ok()?;
                let obj_pos = obj.get_position();
                let dx = obj_pos.x - pos.x;
                let dy = obj_pos.y - pos.y;
                let center_dist = (dx * dx + dy * dy).sqrt();
                let obj_radius = obj.get_geometry_info().get_bounding_circle_radius();
                let boundary_dist = if center_dist <= obj_radius {
                    0.0
                } else {
                    center_dist - obj_radius
                };
                if boundary_dist * boundary_dist <= radius_sqr {
                    Some(obj.get_id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get objects in range using boundary-to-boundary distance in 3D.
    ///
    /// Mirrors C++ `FROM_BOUNDINGSPHERE_3D` distance calculation when the query
    /// is a position (no source object).
    pub fn get_objects_in_range_boundary_3d(
        &self,
        pos: &Coord3D,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        let radius_sqr = radius * radius;
        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj = obj_arc.read().ok()?;
                let obj_pos = obj.get_position();
                let geom = obj.get_geometry_info();
                let center_z_delta = (geom.bounds.min.z + geom.bounds.max.z) * 0.5;
                let dx = obj_pos.x - pos.x;
                let dy = obj_pos.y - pos.y;
                let dz = (obj_pos.z + center_z_delta) - pos.z;
                let center_dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let obj_radius = geom.get_bounding_sphere_radius();
                let boundary_dist = if center_dist <= obj_radius {
                    0.0
                } else {
                    center_dist - obj_radius
                };
                if boundary_dist * boundary_dist <= radius_sqr {
                    Some(obj.get_id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get objects in range using boundary-to-boundary distance in 3D from a source object.
    ///
    /// Mirrors C++ `iterateObjectsInRange(source, radius, FROM_BOUNDINGSPHERE_3D, ...)`.
    pub fn get_objects_in_range_boundary_3d_from_object(
        &self,
        source: &crate::object::Object,
        radius: Real,
    ) -> Vec<crate::common::ObjectID> {
        let radius_sqr = radius * radius;
        let source_pos = source.get_position();
        let source_geom = source.get_geometry_info();
        let source_center_z = (source_geom.bounds.min.z + source_geom.bounds.max.z) * 0.5;
        let source_radius = source_geom.get_bounding_sphere_radius();

        OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|obj_arc| {
                let obj = obj_arc.read().ok()?;
                let obj_pos = obj.get_position();
                let geom = obj.get_geometry_info();
                let center_z_delta = (geom.bounds.min.z + geom.bounds.max.z) * 0.5;
                let dx = obj_pos.x - source_pos.x;
                let dy = obj_pos.y - source_pos.y;
                let dz = (obj_pos.z + center_z_delta) - (source_pos.z + source_center_z);
                let center_dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let obj_radius = geom.get_bounding_sphere_radius();
                let combined_radius = source_radius + obj_radius;
                let boundary_dist = if center_dist <= combined_radius {
                    0.0
                } else {
                    center_dist - combined_radius
                };
                if boundary_dist * boundary_dist <= radius_sqr {
                    Some(obj.get_id())
                } else {
                    None
                }
            })
            .collect()
    }
    /// Get the closest object in range that satisfies a filter
    /// C++ Reference: PartitionManager::getClosestObject
    pub fn get_closest_object<F>(
        &self,
        pos: &Coord3D,
        radius: Real,
        mut filter: F,
    ) -> Option<ObjectID>
    where
        F: FnMut(&crate::object::Object) -> bool,
    {
        let candidate_ids = self.get_objects_in_range(pos, radius);
        let mut closest_id = None;
        let mut min_dist_sqr = radius * radius + 1.0; // Plus 1 to ensure we pick up objects exactly on the radius if needed

        for id in candidate_ids {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(id) {
                if let Ok(obj) = obj_arc.read() {
                    if filter(&obj) {
                        let obj_pos = obj.get_position();
                        let dist_sqr = pos.distance_squared(*obj_pos);
                        if dist_sqr < min_dist_sqr {
                            min_dist_sqr = dist_sqr;
                            closest_id = Some(id);
                        }
                    }
                }
            }
        }
        closest_id
    }

    /// Get the closest object in range using 2D distance that satisfies a filter.
    /// Mirrors C++ `FROM_CENTER_2D` selection for closest-object queries.
    pub fn get_closest_object_2d<F>(
        &self,
        pos: &Coord3D,
        radius: Real,
        mut filter: F,
    ) -> Option<ObjectID>
    where
        F: FnMut(&crate::object::Object) -> bool,
    {
        let candidate_ids = self.get_objects_in_range(pos, radius);
        let mut closest_id = None;
        let mut min_dist_sqr = radius * radius + 1.0;

        for id in candidate_ids {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(id) {
                if let Ok(obj) = obj_arc.read() {
                    if filter(&obj) {
                        let obj_pos = obj.get_position();
                        let dx = obj_pos.x - pos.x;
                        let dy = obj_pos.y - pos.y;
                        let dist_sqr = dx * dx + dy * dy;
                        if dist_sqr < min_dist_sqr {
                            min_dist_sqr = dist_sqr;
                            closest_id = Some(id);
                        }
                    }
                }
            }
        }
        closest_id
    }

    /// Get distance squared between two objects
    /// Matches C++ PartitionManager::getDistanceSquared
    pub fn get_distance_squared(
        obj1: &crate::object::Object,
        obj2: &crate::object::Object,
        flags: DistanceType,
    ) -> Real {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_3D, FROM_EDGE_2D};

        let pos1 = obj1.get_position();
        let pos2 = obj2.get_position();

        if flags == FROM_CENTER_3D {
            return pos1.distance_squared(*pos2);
        }

        let dx = pos1.x - pos2.x;
        let dy = pos1.y - pos2.y;
        let center_dist = (dx * dx + dy * dy).sqrt();

        if flags == FROM_BOUNDING_SPHERE_2D || flags == FROM_EDGE_2D {
            let radius_sum = obj1.get_geometry_info().get_bounding_circle_radius()
                + obj2.get_geometry_info().get_bounding_circle_radius();
            let boundary_dist = if center_dist <= radius_sum {
                0.0
            } else {
                center_dist - radius_sum
            };
            boundary_dist * boundary_dist
        } else {
            dx * dx + dy * dy
        }
    }

    /// Get distance squared between object and position
    /// Matches C++ PartitionManager::getDistanceSquared (position variant)
    pub fn get_distance_squared_to_pos(
        obj: &crate::object::Object,
        pos: &Coord3D,
        flags: DistanceType,
    ) -> Real {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_3D, FROM_EDGE_2D};

        let obj_pos = obj.get_position();

        if flags == FROM_CENTER_3D {
            return obj_pos.distance_squared(*pos);
        }

        let dx = obj_pos.x - pos.x;
        let dy = obj_pos.y - pos.y;
        let center_dist = (dx * dx + dy * dy).sqrt();

        if flags == FROM_BOUNDING_SPHERE_2D || flags == FROM_EDGE_2D {
            let radius = obj.get_geometry_info().get_bounding_circle_radius();
            let boundary_dist = if center_dist <= radius {
                0.0
            } else {
                center_dist - radius
            };
            boundary_dist * boundary_dist
        } else {
            dx * dx + dy * dy
        }
    }

    /// Estimate terrain extremes along a line (matching C++ PartitionManager::estimateTerrainExtremesAlongLine).
    /// Returns false if the line travels off-map.
    pub fn estimate_terrain_extremes_along_line(
        &self,
        start: Coord3D,
        end: Coord3D,
        highest: &mut Real,
    ) -> bool {
        let Some(terrain) = TheTerrainLogic::get() else {
            return true;
        };

        let extent = terrain.get_maximum_pathfind_extent();
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let step = crate::common::MAP_XY_FACTOR.max(1.0);
        let steps = ((dist / step).ceil() as i32).max(1);
        let mut max_height = f32::MIN;

        for i in 0..=steps {
            let t = (i as f32) / (steps as f32);
            let x = start.x + dx * t;
            let y = start.y + dy * t;
            if x < extent.lo.x || x > extent.hi.x || y < extent.lo.y || y > extent.hi.y {
                return false;
            }
            let z = terrain.get_ground_height(x, y, None);
            if z > max_height {
                max_height = z;
            }
        }

        *highest = if max_height.is_finite() {
            max_height
        } else {
            0.0
        };
        true
    }

    /// Mirrors C++ ThePartitionManager->doShroudReveal().
    pub fn do_shroud_reveal(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_shroud_reveal(center, radius, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->undoShroudReveal().
    pub fn undo_shroud_reveal(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_shroud_reveal(center, radius, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->queueUndoShroudReveal().
    pub fn queue_undo_shroud_reveal(
        &self,
        center: &Coord3D,
        radius: Real,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        let persist_frames = game_engine::common::global_data::read_safe()
            .map(|data| data.unlook_persist_duration)
            .unwrap_or(0);
        let current_frame = TheGameLogic::get_frame();
        shroud.queue_undo_shroud_reveal(
            center,
            radius,
            player_mask.bits(),
            persist_frames,
            current_frame,
        );
    }

    /// Mirrors C++ ThePartitionManager->doShroudCover().
    pub fn do_shroud_cover(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_shroud_cover(center, radius, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->undoShroudCover().
    pub fn undo_shroud_cover(&self, center: &Coord3D, radius: Real, player_mask: PlayerMaskType) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_shroud_cover(center, radius, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->doThreatAffect().
    pub fn do_threat_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        threat_value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_threat_affect(center, radius, threat_value, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->undoThreatAffect().
    pub fn undo_threat_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        threat_value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_threat_affect(center, radius, threat_value, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->doValueAffect().
    pub fn do_value_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.do_value_affect(center, radius, value, player_mask.bits());
    }

    /// Mirrors C++ ThePartitionManager->undoValueAffect().
    pub fn undo_value_affect(
        &self,
        center: &Coord3D,
        radius: Real,
        value: u32,
        player_mask: PlayerMaskType,
    ) {
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        shroud.undo_value_affect(center, radius, value, player_mask.bits());
    }
}

impl crate::common::types::PartitionManagerInterface for ThePartitionManagerBridge {
    fn get_distance_squared(
        &self,
        a: &crate::object::Object,
        b: &crate::object::Object,
        distance_type: crate::common::types::PartitionDistanceType,
    ) -> f32 {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_2D, FROM_CENTER_3D};

        let flags = match distance_type {
            crate::common::types::PartitionDistanceType::Center2D => FROM_CENTER_2D,
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                FROM_BOUNDING_SPHERE_2D
            }
            crate::common::types::PartitionDistanceType::Center3D => FROM_CENTER_3D,
        };
        ThePartitionManager::get_distance_squared(a, b, flags)
    }

    fn get_distance_squared_to_pos(
        &self,
        obj: &crate::object::Object,
        pos: &Coord3D,
        distance_type: crate::common::types::PartitionDistanceType,
    ) -> f32 {
        use crate::common::{FROM_BOUNDING_SPHERE_2D, FROM_CENTER_2D, FROM_CENTER_3D};

        let flags = match distance_type {
            crate::common::types::PartitionDistanceType::Center2D => FROM_CENTER_2D,
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                FROM_BOUNDING_SPHERE_2D
            }
            crate::common::types::PartitionDistanceType::Center3D => FROM_CENTER_3D,
        };
        ThePartitionManager::get_distance_squared_to_pos(obj, pos, flags)
    }

    fn get_closest_object(
        &self,
        from: &crate::object::Object,
        max_range: f32,
        distance_type: crate::common::types::PartitionDistanceType,
        filters: &[crate::common::types::PartitionFilter],
    ) -> Option<std::sync::Arc<std::sync::RwLock<crate::object::Object>>> {
        let partition = ThePartitionManager::get()?;
        let from_pos = from.get_position();
        let candidates = match distance_type {
            crate::common::types::PartitionDistanceType::Center3D => {
                partition.get_objects_in_range_boundary_3d(from_pos, max_range)
            }
            crate::common::types::PartitionDistanceType::FromBoundingSphere2D => {
                partition.get_objects_in_range_boundary_2d(from_pos, max_range)
            }
            crate::common::types::PartitionDistanceType::Center2D => {
                partition.get_objects_in_range(from_pos, max_range)
            }
        };
        let mut best: Option<(f32, ObjectID)> = None;

        for id in candidates {
            let obj_arc = crate::object::registry::OBJECT_REGISTRY.get_object(id)?;
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if !partition_filter_allows(from, &*obj, filters) {
                continue;
            }
            let dist = self.get_distance_squared(from, &*obj, distance_type);
            if dist <= max_range * max_range {
                if best.map_or(true, |(best_dist, _)| dist < best_dist) {
                    best = Some((dist, id));
                }
            }
        }

        best.and_then(|(_, id)| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }
}

fn partition_filter_allows(
    from: &crate::object::Object,
    candidate: &crate::object::Object,
    filters: &[crate::common::types::PartitionFilter],
) -> bool {
    use crate::common::types::PartitionFilter;
    use crate::common::Relationship;
    use crate::object::ObjectScriptStatusBit;

    for filter in filters {
        let allowed = match *filter {
            PartitionFilter::Flammable => candidate.find_update_module("FlammableUpdate").is_some(),
            PartitionFilter::Enemy => {
                matches!(from.relationship_to(candidate), Relationship::Enemies)
            }
            PartitionFilter::Friendly => matches!(
                from.relationship_to(candidate),
                Relationship::Allies | Relationship::Allies | Relationship::Allies
            ),
            PartitionFilter::Neutral => {
                matches!(from.relationship_to(candidate), Relationship::Neutral)
            }
            PartitionFilter::Targetable => {
                if candidate.is_effectively_dead() {
                    false
                } else if candidate.test_script_status_bit(ObjectScriptStatusBit::ScriptTargetable)
                {
                    true
                } else {
                    !candidate.test_script_status_bit(ObjectScriptStatusBit::ScriptDisabled)
                        && !candidate.is_off_map()
                }
            }
            PartitionFilter::Attackable => {
                !candidate.is_effectively_dead()
                    && !candidate.is_off_map()
                    && !candidate.test_script_status_bit(ObjectScriptStatusBit::ScriptDisabled)
            }
            PartitionFilter::CanHeal => object_can_heal(candidate),
            PartitionFilter::CanRepair => object_can_repair(candidate),
            PartitionFilter::KindOf(kind) => candidate.is_kind_of(kind),
        };
        if !allowed {
            return false;
        }
    }
    true
}

fn object_can_heal(candidate: &crate::object::Object) -> bool {
    if candidate.is_kind_of(crate::common::KindOf::HealPad) {
        return true;
    }
    for module_handle in candidate.behavior_modules() {
        let mut matched = false;
        module_handle.with_module(|module| {
            if module
                .as_any()
                .is::<crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule>()
            {
                matched = true;
            }
        });
        if matched {
            return true;
        }
    }
    false
}

fn object_can_repair(candidate: &crate::object::Object) -> bool {
    if candidate.is_kind_of(crate::common::KindOf::RepairPad) {
        return true;
    }
    if candidate.with_dock_update_interface(|_| true).is_some() {
        return true;
    }
    false
}

impl crate::special_power_module::integration::PartitionManagerInterface
    for ThePartitionManagerBridge
{
    fn find_objects_in_radius(
        &self,
        center: &Coord3D,
        radius: Real,
        filter: Option<crate::special_power_module::integration::ObjectFilter>,
    ) -> Vec<ObjectID> {
        let partition = ThePartitionManager::get();
        let Some(partition) = partition else {
            return Vec::new();
        };
        let mut results = partition.get_objects_in_range(center, radius);
        if let Some(filter) = filter {
            let local_team = crate::player::player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()));
            results.retain(|id| {
                let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(*id) else {
                    return false;
                };
                let Ok(obj) = obj_arc.read() else {
                    return false;
                };
                match filter {
                    crate::special_power_module::integration::ObjectFilter::All => true,
                    crate::special_power_module::integration::ObjectFilter::Infantry => {
                        obj.is_kind_of(crate::common::KindOf::Infantry)
                    }
                    crate::special_power_module::integration::ObjectFilter::Vehicles => {
                        obj.is_kind_of(crate::common::KindOf::Vehicle)
                    }
                    crate::special_power_module::integration::ObjectFilter::Structures => {
                        obj.is_kind_of(crate::common::KindOf::Structure)
                            || obj.is_kind_of(crate::common::KindOf::Building)
                    }
                    crate::special_power_module::integration::ObjectFilter::Aircraft => {
                        obj.is_kind_of(crate::common::KindOf::Aircraft)
                    }
                    crate::special_power_module::integration::ObjectFilter::Enemy => {
                        let Some(team_arc) = local_team.as_ref() else {
                            return true;
                        };
                        let Ok(team_guard) = team_arc.read() else {
                            return true;
                        };
                        let Some(obj_team) = obj.get_team() else {
                            return false;
                        };
                        let Ok(obj_team_guard) = obj_team.read() else {
                            return false;
                        };
                        team_guard.get_relationship(&obj_team_guard)
                            == crate::common::Relationship::Enemies
                    }
                    crate::special_power_module::integration::ObjectFilter::Friendly => {
                        let Some(team_arc) = local_team.as_ref() else {
                            return true;
                        };
                        let Ok(team_guard) = team_arc.read() else {
                            return true;
                        };
                        let Some(obj_team) = obj.get_team() else {
                            return false;
                        };
                        let Ok(obj_team_guard) = obj_team.read() else {
                            return false;
                        };
                        matches!(
                            team_guard.get_relationship(&obj_team_guard),
                            crate::common::Relationship::Allies
                                | crate::common::Relationship::Allies
                                | crate::common::Relationship::Allies
                        )
                    }
                }
            });
        }
        results
    }

    fn find_position_around(
        &self,
        location: &Coord3D,
        max_radius: Real,
        flags: crate::special_power_module::integration::FindPositionFlags,
    ) -> Option<Coord3D> {
        let partition = ThePartitionManager::get()?;
        let mut options = FindPositionOptions::default();
        options.max_radius = max_radius;
        if flags
            .contains(crate::special_power_module::integration::FindPositionFlags::CLEAR_CELLS_ONLY)
        {
            options.flags |= FPF_CLEAR_CELLS_ONLY;
        }
        if flags.contains(crate::special_power_module::integration::FindPositionFlags::NO_WATER) {
            options.flags |= FPF_IGNORE_WATER;
        }
        if flags.contains(crate::special_power_module::integration::FindPositionFlags::PASSABLE) {
            options.flags |= FPF_CLEAR_CELLS_ONLY;
        }
        let mut result = Coord3D::new(0.0, 0.0, 0.0);
        if partition.find_position_around_with_options(location, &options, &mut result) {
            Some(result)
        } else {
            None
        }
    }
}

/// TheMessageStream singleton - message handling system (matching C++ TheMessageStream)
pub struct TheMessageStream;

impl TheMessageStream {
    /// Append a message - returns a builder that queues on drop.
    pub fn append_message(msg_type: MessageType) -> crate::messages::MessageBuilder {
        crate::messages::append_message(msg_type)
    }
}

#[derive(Debug, Clone)]
struct FloatingTextEntry {
    text: String,
    position: Coord3D,
    color: crate::common::Color,
    created_frame: UnsignedInt,
}

#[derive(Debug, Default)]
struct InGameUIState {
    displayed_max_warning: bool,
    floating_texts: Vec<FloatingTextEntry>,
    messages: Vec<String>,
    idle_worker_additions: Vec<(ObjectID, Int)>,
    idle_worker_removals: Vec<(ObjectID, Int)>,
    last_selection_frame: UnsignedInt,
    superweapons: Vec<SuperweaponEntry>,
}

#[derive(Debug, Clone)]
struct SuperweaponEntry {
    player_index: Int,
    power_name: String,
    object_id: ObjectID,
    template_id: u32,
}

static IN_GAME_UI_STATE: Lazy<RwLock<InGameUIState>> =
    Lazy::new(|| RwLock::new(InGameUIState::default()));

/// TheInGameUI singleton - in-game user interface (matching C++ TheInGameUI)
pub struct TheInGameUI;

impl TheInGameUI {
    /// Select drawable object.
    pub fn select_drawable(drawable: &Arc<RwLock<crate::object::drawable::Drawable>>) {
        if let Ok(mut guard) = drawable.write() {
            guard.set_selected(true);
            guard.flash_as_selected();
        }
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.last_selection_frame = TheGameLogic::get_frame();
        }

        let object_id = drawable.get_object_id();
        if object_id != INVALID_ID {
            if let Ok(list) = crate::player::player_list().read() {
                let local_index = list.get_local_player_index();
                if local_index != crate::player::PLAYER_INDEX_INVALID {
                    let selection_manager = crate::commands::selection::get_selection_manager();
                    let manager_lock = selection_manager.write();
                    if let Ok(mut manager) = manager_lock {
                        if let Some(selection) = manager.get_player_selection(local_index) {
                            let _ = selection.select_objects(
                                vec![object_id],
                                crate::commands::selection::SelectionType::Add,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Deselect drawable object.
    pub fn deselect_drawable(drawable: &Arc<RwLock<crate::object::drawable::Drawable>>) {
        if let Ok(mut guard) = drawable.write() {
            guard.set_selected(false);
        }
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.last_selection_frame = TheGameLogic::get_frame();
        }

        let object_id = drawable.get_object_id();
        if object_id != INVALID_ID {
            if let Ok(list) = crate::player::player_list().read() {
                let local_index = list.get_local_player_index();
                if local_index != crate::player::PLAYER_INDEX_INVALID {
                    let selection_manager = crate::commands::selection::get_selection_manager();
                    let manager_lock = selection_manager.write();
                    if let Ok(mut manager) = manager_lock {
                        if let Some(selection) = manager.get_player_selection(local_index) {
                            let _ = selection.select_objects(
                                vec![object_id],
                                crate::commands::selection::SelectionType::Remove,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Set displayed maximum warning.
    pub fn set_displayed_max_warning(show: bool) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.displayed_max_warning = show;
        }
    }

    /// Display floating combat text.
    pub fn add_floating_text(
        text: &str,
        position: &Coord3D,
        color: crate::common::Color,
    ) -> Result<(), GameError> {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.floating_texts.push(FloatingTextEntry {
                text: text.to_string(),
                position: *position,
                color,
                created_frame: TheGameLogic::get_frame(),
            });
        }
        Ok(())
    }

    /// Display a message to the player.
    /// Matches C++ InGameUI message display functionality
    pub fn display_message(message: &str) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.messages.push(message.to_string());
        }
        log::info!("UI Message: {}", message);
    }

    pub fn add_superweapon(
        player_index: Int,
        power_name: String,
        object_id: ObjectID,
        template: &SpecialPowerTemplate,
    ) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.superweapons.retain(|entry| {
                !(entry.player_index == player_index
                    && entry.object_id == object_id
                    && entry.template_id == template.get_id())
            });
            state.superweapons.push(SuperweaponEntry {
                player_index,
                power_name,
                object_id,
                template_id: template.get_id(),
            });
        }
    }

    pub fn remove_superweapon(
        player_index: Int,
        power_name: String,
        object_id: ObjectID,
        template: &SpecialPowerTemplate,
    ) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state.superweapons.retain(|entry| {
                !(entry.player_index == player_index
                    && entry.object_id == object_id
                    && entry.template_id == template.get_id()
                    && entry.power_name == power_name)
            });
        }
    }

    /// Remove a worker from the idle worker UI list.
    pub fn remove_idle_worker(object: &crate::object::Object, player_index: Int) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state
                .idle_worker_removals
                .push((object.get_id(), player_index));
        }
    }

    /// Add a worker to the idle worker UI list.
    pub fn add_idle_worker(object: &crate::object::Object, player_index: Int) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            state
                .idle_worker_additions
                .push((object.get_id(), player_index));
        }
    }

    /// Drain pending idle worker add/remove events (matches InGameUI idle worker bookkeeping).
    pub fn take_idle_worker_events() -> (Vec<(ObjectID, Int)>, Vec<(ObjectID, Int)>) {
        if let Ok(mut state) = IN_GAME_UI_STATE.write() {
            let additions = std::mem::take(&mut state.idle_worker_additions);
            let removals = std::mem::take(&mut state.idle_worker_removals);
            return (additions, removals);
        }
        (Vec::new(), Vec::new())
    }
}

/// TheFXListStore singleton - FX list storage system (matching C++ TheFXListStore)
pub struct TheFXListStore;

static FX_LIST_STORE: Lazy<RwLock<HashMap<NameKeyType, Arc<FXList>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

impl TheFXListStore {
    /// Lookup an existing FX list without creating a placeholder entry.
    pub fn lookup_fx_list(name: &str) -> Option<Arc<FXList>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        let key = NameKeyGenerator::name_to_key(name) as NameKeyType;
        FX_LIST_STORE
            .read()
            .ok()
            .and_then(|store| store.get(&key).cloned())
    }

    /// Find FX list (matches C++ TheFXListStore::findFXList)
    pub fn find_fx_list(name: &str) -> Option<Arc<FXList>> {
        Self::lookup_fx_list(name)
    }

    /// Register an FX list for later lookup. Returns the stored handle.
    pub fn register_fx_list(name: &str, fx: FXList) -> Arc<FXList> {
        let key = NameKeyGenerator::name_to_key(name) as NameKeyType;
        let mut store = FX_LIST_STORE.write().expect("FX list store lock poisoned");
        store.entry(key).or_insert_with(|| Arc::new(fx)).clone()
    }

    /// Ensure an FX list exists.
    pub fn ensure_fx_list(name: &str) -> Arc<FXList> {
        if name.eq_ignore_ascii_case("None") {
            panic!("FXList name must be valid");
        }
        if let Some(existing) = Self::lookup_fx_list(name) {
            return existing;
        }
        let key = NameKeyGenerator::name_to_key(name) as NameKeyType;
        let fx = Arc::new(FXList::new(name));
        if let Ok(mut store) = FX_LIST_STORE.write() {
            return store.entry(key).or_insert_with(|| Arc::clone(&fx)).clone();
        }
        fx
    }
}

/// TheObjectCreationListStore singleton - object creation list storage (matching C++ TheObjectCreationListStore)
pub struct TheObjectCreationListStore;

impl TheObjectCreationListStore {
    pub fn get() -> Option<&'static Self> {
        static STORE: OnceLock<TheObjectCreationListStore> = OnceLock::new();
        Some(STORE.get_or_init(|| TheObjectCreationListStore))
    }

    /// Find object creation list.
    pub fn find_object_creation_list(
        name: &str,
    ) -> Option<Arc<crate::object_creation_list::store::ObjectCreationList>> {
        crate::object_creation_list::store::ensure_default_object_creation_lists_loaded();
        let key = normalize_resource_name(name);
        let store = crate::object_creation_list::store::get_object_creation_list_store();
        store
            .as_ref()
            .and_then(|store| store.find_object_creation_list(&key))
    }

    /// Lookup an existing object creation list without creating placeholders.
    pub fn lookup_object_creation_list(
        name: &str,
    ) -> Option<Arc<crate::object_creation_list::store::ObjectCreationList>> {
        Self::find_object_creation_list(name)
    }

    /// Register an object creation list for later lookup.
    pub fn register_object_creation_list(
        name: &str,
        ocl: crate::object_creation_list::store::ObjectCreationList,
    ) -> Arc<crate::object_creation_list::store::ObjectCreationList> {
        let key = normalize_resource_name(name);
        if crate::object_creation_list::store::get_object_creation_list_store()
            .as_ref()
            .is_none()
        {
            crate::object_creation_list::store::init_object_creation_list_store();
        }
        let mut store = crate::object_creation_list::store::get_object_creation_list_store_mut();
        store
            .as_mut()
            .expect("ObjectCreationListStore not initialized")
            .register_ocl(key, ocl)
    }

    /// Ensure an object creation list exists (lookup-only).
    ///
    /// This intentionally does not fabricate placeholder OCL entries.
    pub fn ensure_object_creation_list(
        name: &str,
    ) -> Option<Arc<crate::object_creation_list::store::ObjectCreationList>> {
        Self::find_object_creation_list(name)
    }

    /// Explicitly create/register an empty object creation list for a name.
    pub fn create_empty_object_creation_list(
        name: &str,
    ) -> Arc<crate::object_creation_list::store::ObjectCreationList> {
        Self::register_object_creation_list(
            name,
            crate::object_creation_list::store::ObjectCreationList::new(),
        )
    }
}

fn normalize_resource_name(name: &str) -> String {
    name.trim().trim_matches('"').to_string()
}

/// Simple text lookup helper emulating the legacy localization queries.
pub struct TheGameText;

static MAP_STRING_OVERLAY: Lazy<RwLock<HashMap<String, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

fn parse_map_string_file(contents: &str, out: &mut HashMap<String, String>) {
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.eq_ignore_ascii_case("END") {
            if let Some(key) = current_key.take() {
                out.insert(key, current_value.clone());
            }
            current_value.clear();
            continue;
        }
        if line.starts_with('"') {
            let mut value = line.trim_matches('"').to_string();
            value = unescape_map_string_value(&value);
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(&value);
            continue;
        }
        current_key = Some(line.to_string());
    }
}

fn unescape_map_string_value(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

impl TheGameText {
    /// Fetch a localized string; currently returns the key as-is.
    pub fn fetch(key: &str) -> String {
        let key = key.trim();
        if key.is_empty() {
            return String::new();
        }

        if let Ok(overlay) = MAP_STRING_OVERLAY.read() {
            if let Some(value) = overlay.get(key) {
                return value.clone();
            }
        }

        game_engine::common::language::Language::get_localized_string(key)
    }

    pub fn init_map_string_file(path: &str) -> Result<(), String> {
        let bytes = std::fs::read(path).or_else(|_| {
            let fs_arc = get_file_system();
            let mut fs = fs_arc
                .lock()
                .map_err(|_| std::io::Error::other("FileSystem mutex poisoned"))?;
            let mut file = fs
                .open_file(path, FileAccess::READ.combine(FileAccess::BINARY))
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, path))?;
            file.read_entire_and_close()
        });

        let contents = bytes
            .map(|raw| String::from_utf8_lossy(&raw).into_owned())
            .map_err(|err| format!("Failed reading map string file '{}': {err}", path))?;

        if let Ok(mut overlay) = MAP_STRING_OVERLAY.write() {
            overlay.clear();
            parse_map_string_file(&contents, &mut overlay);
        }

        Ok(())
    }

    pub fn clear_map_string_file() {
        if let Ok(mut overlay) = MAP_STRING_OVERLAY.write() {
            overlay.clear();
        }
    }
}

/// Minimal misc-audio descriptor providing stable handles for crate events.
#[derive(Clone)]
pub struct MiscAudioEvents {
    pub crate_heal: AudioEvent,
    pub crate_shroud: AudioEvent,
    pub crate_salvage: AudioEvent,
    pub crate_free_unit: AudioEvent,
    pub crate_money: AudioEvent,
    pub battle_cry_sound: AudioEvent,
    pub money_deposit: AudioEvent,
    pub money_withdraw: AudioEvent,
    pub sabotage_shut_down_building: AudioEvent,
    pub sabotage_reset_timer_building: AudioEvent,
}

impl Default for MiscAudioEvents {
    fn default() -> Self {
        Self {
            crate_heal: AudioEvent::new(0, "crate_heal"),
            crate_shroud: AudioEvent::new(0, "crate_shroud"),
            crate_salvage: AudioEvent::new(0, "crate_salvage"),
            crate_free_unit: AudioEvent::new(0, "crate_free_unit"),
            crate_money: AudioEvent::new(0, "crate_money"),
            battle_cry_sound: AudioEvent::new(0, "battle_cry_sound"),
            money_deposit: AudioEvent::new(0, "money_deposit"),
            money_withdraw: AudioEvent::new(0, "money_withdraw"),
            sabotage_shut_down_building: AudioEvent::new(0, "sabotage_shut_down_building"),
            sabotage_reset_timer_building: AudioEvent::new(0, "sabotage_reset_timer_building"),
        }
    }
}

pub struct TheAudio;

struct GameLogicAudioEventOwnerResolver;

impl AudioEventOwnerResolver for GameLogicAudioEventOwnerResolver {
    fn resolve_object_position(&self, object_id: ObjectID) -> Option<EngineCoord3D> {
        let object = TheGameLogic::find_object_by_id(object_id)?;
        let guard = object.read().ok()?;
        let position = *guard.get_position();
        Some(EngineCoord3D {
            x: position.x,
            y: position.y,
            z: position.z,
        })
    }

    fn resolve_drawable_position(&self, drawable_id: u32) -> Option<EngineCoord3D> {
        let client = TheGameClient::get()?;
        let state = client.find_drawable_by_id(drawable_id)?;
        Some(EngineCoord3D {
            x: state.position.x,
            y: state.position.y,
            z: state.position.z,
        })
    }

    fn resolve_object_player_index(&self, object_id: ObjectID) -> Option<Int> {
        let object = TheGameLogic::find_object_by_id(object_id)?;
        let guard = object.read().ok()?;
        let player = guard.get_controlling_player()?;
        let player_guard = player.read().ok()?;
        Some(player_guard.get_player_index())
    }

    fn resolve_drawable_player_index(&self, drawable_id: u32) -> Option<Int> {
        let client = TheGameClient::get()?;
        let state = client.find_drawable_by_id(drawable_id)?;

        if state.shroud_status_object_id != INVALID_ID {
            return self.resolve_object_player_index(state.shroud_status_object_id);
        }

        let drawable = state.drawable?;
        let object_id = drawable.read().ok()?.get_object_id();
        if object_id == INVALID_ID {
            return None;
        }

        self.resolve_object_player_index(object_id)
    }
}

struct GameLogicAudioShroudResolver;

impl AudioShroudResolver for GameLogicAudioShroudResolver {
    fn is_position_visible_to_local_player(&self, position: &EngineCoord3D) -> Bool {
        let local_player_index = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()));

        let Some(local_player_index) = local_player_index else {
            return true;
        };
        if local_player_index < 0 {
            return true;
        }

        let Ok(shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return true;
        };

        let world = Coord3D {
            x: position.x,
            y: position.y,
            z: position.z,
        };

        matches!(
            shroud.get_shroud_state(local_player_index as u32, &world),
            crate::system::shroud_manager::ShroudState::Visible
        )
    }
}

struct GameLogicAudioLocalityResolver;

impl AudioLocalityResolver for GameLogicAudioLocalityResolver {
    fn get_local_player_index(&self) -> Option<Int> {
        let list = crate::player::player_list().read().ok()?;
        let player = list.get_local_player()?.clone();
        let guard = player.read().ok()?;
        Some(guard.get_player_index())
    }

    fn is_player_active(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        let list = match crate::player::player_list().read() {
            Ok(list) => list,
            Err(_) => return false,
        };
        let Some(player) = list.get_player(player_index).cloned() else {
            return false;
        };
        player
            .read()
            .ok()
            .map(|guard| guard.is_player_active())
            .unwrap_or(false)
    }

    fn player_exists(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        crate::player::player_list()
            .read()
            .ok()
            .map(|list| list.get_player(player_index).is_some())
            .unwrap_or(false)
    }

    fn has_default_team(&self, player_index: Int) -> Bool {
        if player_index < 0 {
            return false;
        }
        let list = match crate::player::player_list().read() {
            Ok(list) => list,
            Err(_) => return false,
        };
        let Some(player) = list.get_player(player_index).cloned() else {
            return false;
        };
        player
            .read()
            .ok()
            .and_then(|guard| guard.get_default_team())
            .is_some()
    }

    fn get_observer_look_at_player_index(&self) -> Option<Int> {
        observer_audio_locality_hooks().and_then(|hooks| hooks.get_observer_look_at_player_index())
    }

    fn get_relationship_to_local_team(
        &self,
        source_player_index: Int,
        local_player_index: Int,
    ) -> AudioLocalityRelationship {
        if source_player_index < 0 || local_player_index < 0 {
            return AudioLocalityRelationship::Neutral;
        }

        let list = match crate::player::player_list().read() {
            Ok(list) => list,
            Err(_) => return AudioLocalityRelationship::Neutral,
        };

        let Some(source_player) = list.get_player(source_player_index).cloned() else {
            return AudioLocalityRelationship::Neutral;
        };
        let Some(local_player) = list.get_player(local_player_index).cloned() else {
            return AudioLocalityRelationship::Neutral;
        };

        let local_team = local_player
            .read()
            .ok()
            .and_then(|guard| guard.get_default_team());
        let Some(local_team) = local_team else {
            return AudioLocalityRelationship::Neutral;
        };

        let Ok(local_team_guard) = local_team.read() else {
            return AudioLocalityRelationship::Neutral;
        };
        let Ok(source_guard) = source_player.read() else {
            return AudioLocalityRelationship::Neutral;
        };

        match source_guard.get_relationship_with_team(&local_team_guard) {
            Relationship::Allies | Relationship::Allies | Relationship::Allies => {
                AudioLocalityRelationship::Allies
            }
            Relationship::Enemies => AudioLocalityRelationship::Enemies,
            Relationship::Neutral => AudioLocalityRelationship::Neutral,
        }
    }
}

/// Resolver that provides camera/terrain view information for 3D audio positioning.
///
/// C++ equivalent: TheTacticalView and TheTerrainLogic access in AudioManager::update().
/// Uses the real terrain logic for ground height and provides tactical view data
/// from the game client when available.
struct GameLogicAudioViewResolver;

impl AudioViewResolver for GameLogicAudioViewResolver {
    fn get_tactical_view_position(&self) -> EngineCoord3D {
        if let Some((x, y, z)) =
            observer_audio_view_hooks().and_then(|hooks| hooks.get_tactical_view_position())
        {
            return EngineCoord3D { x, y, z };
        }

        // C++ reads TheTacticalView->getPosition(). Fallback remains deterministic.
        EngineCoord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    fn get_tactical_view_angle(&self) -> f32 {
        if let Some(angle) =
            observer_audio_view_hooks().and_then(|hooks| hooks.get_tactical_view_angle())
        {
            return angle;
        }

        // C++ reads TheTacticalView->getAngle(). Fallback remains deterministic.
        0.0
    }

    fn get_3d_camera_position(&self) -> EngineCoord3D {
        if let Some((x, y, z)) =
            observer_audio_view_hooks().and_then(|hooks| hooks.get_3d_camera_position())
        {
            return EngineCoord3D { x, y, z };
        }

        // C++ reads TheTacticalView->get3DCameraPosition(). Fallback remains deterministic.
        EngineCoord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    fn get_ground_height(&self, x: f32, y: f32) -> f32 {
        // C++ reads TheTerrainLogic->getGroundHeight(x, y).
        // This uses the real terrain logic - the most important resolver method
        // for correct audio attenuation over terrain.
        crate::terrain::get_terrain_logic()
            .read()
            .map(|terrain| terrain.get_ground_height(x, y, None))
            .unwrap_or(0.0)
    }
}

fn ensure_audio_event_resolvers_registered() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let _ = register_audio_event_owner_resolver(Arc::new(GameLogicAudioEventOwnerResolver));
        let _ = register_audio_shroud_resolver(Arc::new(GameLogicAudioShroudResolver));
        let _ = register_audio_locality_resolver(Arc::new(GameLogicAudioLocalityResolver));
        let _ = register_audio_view_resolver(Arc::new(GameLogicAudioViewResolver));
    });
}

impl TheAudio {
    pub fn get() -> Option<&'static Self> {
        static AUDIO: OnceLock<TheAudio> = OnceLock::new();
        let audio = AUDIO.get_or_init(|| TheAudio);
        ensure_audio_event_resolvers_registered();
        Some(audio)
    }

    pub fn get_misc_audio() -> &'static MiscAudioEvents {
        static MISC_AUDIO: OnceLock<MiscAudioEvents> = OnceLock::new();
        MISC_AUDIO.get_or_init(|| {
            let Some(misc_audio) = game_engine::common::ini::ini_misc_audio::get_misc_audio()
            else {
                return MiscAudioEvents::default();
            };

            let misc_audio = misc_audio.read();
            let crate_heal = AudioEvent::new(0, misc_audio.crate_heal.sound_file.as_str());
            let crate_shroud = AudioEvent::new(0, misc_audio.crate_shroud.sound_file.as_str());
            let crate_salvage = AudioEvent::new(0, misc_audio.crate_salvage.sound_file.as_str());
            let crate_free_unit =
                AudioEvent::new(0, misc_audio.crate_free_unit.sound_file.as_str());
            let crate_money = AudioEvent::new(0, misc_audio.crate_money.sound_file.as_str());
            let battle_cry_sound =
                AudioEvent::new(0, misc_audio.battle_cry_sound.sound_file.as_str());
            let money_deposit =
                AudioEvent::new(0, misc_audio.money_deposit_sound.sound_file.as_str());
            let money_withdraw =
                AudioEvent::new(0, misc_audio.money_withdraw_sound.sound_file.as_str());
            let sabotage_shut_down_building = AudioEvent::new(
                0,
                misc_audio.sabotage_shut_down_building.sound_file.as_str(),
            );
            let sabotage_reset_timer_building = AudioEvent::new(
                0,
                misc_audio.sabotage_reset_timer_building.sound_file.as_str(),
            );

            MiscAudioEvents {
                crate_heal,
                crate_shroud,
                crate_salvage,
                crate_free_unit,
                crate_money,
                battle_cry_sound,
                money_deposit,
                money_withdraw,
                sabotage_shut_down_building,
                sabotage_reset_timer_building,
            }
        })
    }

    pub fn add_audio_event(&self, event: &AudioEventRts) -> u32 {
        let mut engine_event = if let Some((x, y, z)) = event.position {
            let pos = EngineCoord3D { x, y, z };
            EngineAudioEventRts::with_position(event.get_event_name(), &pos)
        } else {
            EngineAudioEventRts::with_event_name(event.get_event_name())
        };

        if let Some(drawable_id) = event.drawable_id {
            engine_event.set_drawable_id_override(drawable_id);
        } else if event.position.is_none() && event.object_id != 0 {
            engine_event.set_object_id(event.object_id);
        }

        if let Some(time_of_day) = event.time_of_day {
            engine_event.set_time_of_day(map_audio_time_of_day(time_of_day));
        }

        if let Some(player_index) = event.player_index {
            engine_event.set_player_index(player_index as i32);
        }
        engine_event.set_should_fade(event.should_fade());
        engine_event.set_is_logical_audio(event.is_logical_audio());
        engine_event.set_uninterruptable(event.is_uninterruptable());

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let mut manager = match manager.lock() {
            Ok(guard) => guard,
            Err(_) => return 0,
        };
        if let Some(info) = manager.find_audio_event_info(engine_event.get_event_name()) {
            engine_event.set_audio_event_info(info.clone());
            engine_event.set_volume(info.volume);
        } else if let Some(info) =
            manager.new_audio_event_info(engine_event.get_event_name().to_string())
        {
            engine_event.set_audio_event_info(info.clone());
            engine_event.set_volume(info.volume);
        }

        manager.add_audio_event(&engine_event)
    }

    pub fn add_misc_audio_event(&self, event: &AudioEvent) -> u32 {
        let mut mapped = AudioEventRts::new(event.sound_type.as_str());
        mapped.object_id = event.object_id;
        self.add_audio_event(&mapped)
    }

    pub fn remove_audio_event(&self, handle: u32) {
        if handle == 0 {
            return;
        }

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut manager) = manager_lock {
            manager.remove_audio_event(handle);
        }
    }

    pub fn is_currently_playing(&self, handle: u32) -> Bool {
        if handle == 0 {
            return false;
        }

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(manager) = manager_lock {
            manager.is_currently_playing(handle)
        } else {
            false
        }
    }

    pub fn get_audio_length_ms(&self, event: &AudioEventRts) -> Real {
        let mut engine_event = if let Some((x, y, z)) = event.position {
            let pos = EngineCoord3D { x, y, z };
            EngineAudioEventRts::with_position(event.get_event_name(), &pos)
        } else {
            EngineAudioEventRts::with_event_name(event.get_event_name())
        };

        if let Some(drawable_id) = event.drawable_id {
            engine_event.set_drawable_id_override(drawable_id);
        } else if event.position.is_none() && event.object_id != 0 {
            engine_event.set_object_id(event.object_id);
        }

        if let Some(time_of_day) = event.time_of_day {
            engine_event.set_time_of_day(map_audio_time_of_day(time_of_day));
        }
        if let Some(player_index) = event.player_index {
            engine_event.set_player_index(player_index as i32);
        }
        engine_event.set_should_fade(event.should_fade());
        engine_event.set_is_logical_audio(event.is_logical_audio());
        engine_event.set_uninterruptable(event.is_uninterruptable());

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        let Ok(mut manager) = manager_lock else {
            return 0.0;
        };

        if let Some(info) = manager.find_audio_event_info(engine_event.get_event_name()) {
            engine_event.set_audio_event_info(info.clone());
            engine_event.set_volume(info.volume);
        } else if let Some(info) =
            manager.new_audio_event_info(engine_event.get_event_name().to_string())
        {
            engine_event.set_audio_event_info(info.clone());
            engine_event.set_volume(info.volume);
        }

        manager.get_audio_length_ms(&engine_event)
    }

    pub fn set_volume(&self, volume: Real, affect: EngineAudioAffect) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.set_volume(volume, affect);
        }
    }

    pub fn update(&self) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.update();
        }
    }

    pub fn pause_audio(&self, affect: EngineAudioAffect) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.pause_audio(affect);
        }
    }

    pub fn resume_audio(&self, affect: EngineAudioAffect) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.resume_audio(affect);
        }
    }

    pub fn set_audio_event_enabled(&self, event_name: &str, enabled: Bool) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.set_audio_event_enabled(event_name.to_string(), enabled);
        }
    }

    pub fn set_audio_event_volume_override(&self, event_name: &str, volume: Real) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.set_audio_event_volume_override(event_name.to_string(), volume);
        }
    }

    pub fn remove_audio_event_by_name(&self, event_name: &str) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.remove_playing_audio(event_name);
        }
    }

    pub fn remove_disabled_events(&self) {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager_lock = manager.lock();
        if let Ok(mut guard) = manager_lock {
            guard.remove_all_disabled_audio();
        }
    }
}

pub struct TheRadar;

impl TheRadar {
    pub fn get() -> Option<&'static Self> {
        static RADAR: OnceLock<TheRadar> = OnceLock::new();
        Some(RADAR.get_or_init(|| TheRadar))
    }

    pub fn create_event(
        &self,
        position: &Coord3D,
        event_type: game_engine::common::system::radar::RadarEventType,
        seconds_to_live: Real,
    ) {
        let radar = get_radar_system();
        let radar_lock = radar.write();
        if let Ok(mut guard) = radar_lock {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            guard.create_event(&world_loc, event_type, seconds_to_live);
        }
    }

    pub fn try_infiltration_event(target: Arc<RwLock<Object>>) -> Result<(), GameError> {
        let Ok(target_guard) = target.read() else {
            return Err(GameError::LockError);
        };
        if target_guard.is_destroyed() {
            return Ok(());
        }
        if !target_guard.is_locally_controlled() {
            return Ok(());
        }

        let position = *target_guard.get_position();
        drop(target_guard);

        let radar = get_radar_system();
        if let Ok(mut guard) = radar.write() {
            let world_loc = game_engine::system::radar::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            };
            guard.try_infiltration_event(&world_loc);
        }
        Ok(())
    }

    pub fn refresh_terrain(&self) {
        let radar = get_radar_system();
        let radar_lock = radar.write();
        if let Ok(mut guard) = radar_lock {
            guard.refresh_terrain();
        }
    }
}

/// Terrain visual effects bridge (matching C++ TheTerrainVisual).
pub struct TheTerrainVisual;

impl TheTerrainVisual {
    pub fn get() -> Option<&'static Self> {
        static VISUAL: OnceLock<TheTerrainVisual> = OnceLock::new();
        Some(VISUAL.get_or_init(|| TheTerrainVisual))
    }

    pub fn add_water_velocity(&self, _x: Real, _y: Real, _velocity: Real, _preferred_height: Real) {
        let frame = TheGameLogic::get_frame();
        let mut impulses = WATER_VELOCITY_IMPULSES
            .lock()
            .expect("Water impulse lock poisoned");
        if impulses.len() >= MAX_WATER_VELOCITY_IMPULSES {
            impulses.remove(0);
        }
        impulses.push(WaterVelocityImpulse {
            x: _x,
            y: _y,
            velocity: _velocity,
            preferred_height: _preferred_height,
            frame,
        });

        log::debug!(
            "Water velocity impulse at ({:.1}, {:.1}) v={:.2} h={:.2} frame={}",
            _x,
            _y,
            _velocity,
            _preferred_height,
            frame
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct WaterVelocityImpulse {
    x: Real,
    y: Real,
    velocity: Real,
    preferred_height: Real,
    frame: UnsignedInt,
}

const MAX_WATER_VELOCITY_IMPULSES: usize = 128;
static WATER_VELOCITY_IMPULSES: Lazy<Mutex<Vec<WaterVelocityImpulse>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaEvent {
    BuildingSabotaged,
    BuildingLost,
    CashStolen,
    UnitLost,
    VehicleStolen,
    BeaconDetected,
    GeneralLevelUp,
    SuperweaponDetectedOwnParticleCannon,
    SuperweaponDetectedAllyParticleCannon,
    SuperweaponDetectedEnemyParticleCannon,
    SuperweaponDetectedOwnNuke,
    SuperweaponDetectedAllyNuke,
    SuperweaponDetectedEnemyNuke,
    SuperweaponDetectedOwnScudStorm,
    SuperweaponDetectedAllyScudStorm,
    SuperweaponDetectedEnemyScudStorm,
    SuperweaponLaunchedOwnParticleCannon,
    SuperweaponLaunchedAllyParticleCannon,
    SuperweaponLaunchedEnemyParticleCannon,
    SuperweaponLaunchedOwnNuke,
    SuperweaponLaunchedAllyNuke,
    SuperweaponLaunchedEnemyNuke,
    SuperweaponLaunchedOwnScudStorm,
    SuperweaponLaunchedAllyScudStorm,
    SuperweaponLaunchedEnemyScudStorm,
    SuperweaponLaunchedOwnGpsScrambler,
    SuperweaponLaunchedAllyGpsScrambler,
    SuperweaponLaunchedEnemyGpsScrambler,
    SuperweaponLaunchedOwnSneakAttack,
    SuperweaponLaunchedAllySneakAttack,
    SuperweaponLaunchedEnemySneakAttack,
}

#[derive(Debug, Default)]
struct EvaState {
    enabled: bool,
    queued: Vec<EvaEvent>,
}

pub struct TheEva;

impl TheEva {
    fn state() -> &'static Mutex<EvaState> {
        static EVA_STATE: OnceLock<Mutex<EvaState>> = OnceLock::new();
        EVA_STATE.get_or_init(|| {
            Mutex::new(EvaState {
                enabled: true,
                queued: Vec::new(),
            })
        })
    }

    pub fn set_should_play(event: EvaEvent) -> Result<(), GameError> {
        let state = Self::state();
        let mut guard = state.lock().map_err(|_| GameError::LockError)?;
        if guard.enabled {
            guard.queued.push(event);
        }
        Ok(())
    }

    pub fn set_enabled(enabled: bool) -> Result<(), GameError> {
        let state = Self::state();
        let mut guard = state.lock().map_err(|_| GameError::LockError)?;
        guard.enabled = enabled;
        if !enabled {
            guard.queued.clear();
        }
        Ok(())
    }

    pub fn is_enabled() -> Result<bool, GameError> {
        let state = Self::state();
        let guard = state.lock().map_err(|_| GameError::LockError)?;
        Ok(guard.enabled)
    }

    pub fn drain_events() -> Result<Vec<EvaEvent>, GameError> {
        let state = Self::state();
        let mut guard = state.lock().map_err(|_| GameError::LockError)?;
        let drained = guard.queued.drain(..).collect();
        Ok(drained)
    }
}

/// TheScriptEngine singleton facade for minimal global script state.
pub struct TheScriptEngine;

impl TheScriptEngine {
    pub fn is_game_ending() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_game_ending()))
            .unwrap_or(false)
    }

    pub fn set_global_difficulty(difficulty: Int) {
        GLOBAL_DIFFICULTY.store(difficulty, Ordering::Relaxed);
        let mapped = match difficulty {
            0 => crate::player::GameDifficulty::Easy,
            1 => crate::player::GameDifficulty::Normal,
            2 => crate::player::GameDifficulty::Hard,
            3 => crate::player::GameDifficulty::Brutal,
            _ => crate::player::GameDifficulty::Normal,
        };
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.set_global_difficulty(mapped);
            }
        }
    }

    pub fn get_global_difficulty() -> Int {
        GLOBAL_DIFFICULTY.load(Ordering::Relaxed)
    }

    pub fn signal_ui_interact(hook_name: &str) {
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.signal_ui_interact(hook_name);
            }
        }
    }

    pub fn notify_of_object_creation_or_destruction() {
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.notify_of_object_creation_or_destruction();
            }
        }
    }

    pub fn notify_of_completed_video(video_name: &str) {
        if let Ok(mut guard) = crate::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.notify_of_completed_video(video_name);
            }
        }
    }

    pub fn is_video_complete(video_name: &str, remove_from_list: bool) -> bool {
        crate::scripting::engine::get_script_engine()
            .write()
            .ok()
            .and_then(|mut guard| {
                guard
                    .as_mut()
                    .map(|engine| engine.is_video_complete(video_name, remove_from_list))
            })
            .unwrap_or(false)
    }

    pub fn is_time_frozen_script() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_time_frozen_script()))
            .unwrap_or(false)
    }

    pub fn is_time_frozen_debug() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_time_frozen_debug()))
            .unwrap_or(false)
    }

    pub fn is_time_frozen() -> Bool {
        crate::scripting::engine::get_script_engine()
            .read()
            .ok()
            .and_then(|engine| engine.as_ref().map(|engine| engine.is_time_frozen()))
            .unwrap_or(false)
    }
}

/// TheVictoryConditions singleton facade for minimal victory state.
pub struct TheVictoryConditions;

impl TheVictoryConditions {
    pub fn set_local_allied_victory(victory: Bool) {
        LOCAL_ALLIED_VICTORY.store(victory, Ordering::Relaxed);
    }

    pub fn is_local_allied_victory() -> Bool {
        LOCAL_ALLIED_VICTORY.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::TheGameLogic;
    use crate::system::game_logic::{get_game_logic, GAME_NONE, GAME_SHELL};

    #[test]
    fn is_in_game_matches_cpp_for_shell_mode() {
        let mut logic = get_game_logic().lock().unwrap();
        let previous_mode = logic.get_game_mode();

        logic.set_game_mode(GAME_NONE);
        drop(logic);
        assert!(!TheGameLogic::is_in_game());

        let mut logic = get_game_logic().lock().unwrap();
        logic.set_game_mode(GAME_SHELL);
        drop(logic);
        assert!(TheGameLogic::is_in_game());

        let mut logic = get_game_logic().lock().unwrap();
        logic.set_game_mode(previous_mode);
    }
}

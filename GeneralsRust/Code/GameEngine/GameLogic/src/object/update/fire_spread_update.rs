// FireSpreadUpdate - Update looks for ::Aflame and explicitly ignites someone nearby if set
// Author: Graham Smallwood, April 2002
// Ported to Rust

use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::prelude::*;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    FireSpreadControlInterface, Module, ModuleData, ModuleDataHandle, ModuleDataRead, NameKeyType,
};
use std::sync::Arc;

/// Module data for FireSpreadUpdate
/// Matches C++ FireSpreadUpdate.cpp
#[derive(Debug, Clone)]
pub struct FireSpreadUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Object creation list for ember effects, kept as the authored name —
    /// the store keys by authored string, which a numeric NameKey cannot
    /// recover once the catalog moves.
    pub ocl_embers: Option<Arc<str>>,
    /// Minimum delay between spread attempts (frames)
    pub min_spread_try_delay: u32,
    /// Maximum delay between spread attempts (frames)
    pub max_spread_try_delay: u32,
    /// Range within which fire can spread
    pub spread_try_range: f32,
}

impl Default for FireSpreadUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            ocl_embers: None,
            min_spread_try_delay: 0,
            max_spread_try_delay: 0,
            spread_try_range: 0.0,
        }
    }
}

impl ModuleData for FireSpreadUpdateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for FireSpreadUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl FireSpreadUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_SPREAD_UPDATE_FIELDS)
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_ocl_embers(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = first_value_token(tokens)?;
    data.ocl_embers = if name.eq_ignore_ascii_case("None") {
        None
    } else {
        // Preserve name-key allocation order without storing the local key.
        let _ = name_key_generate(name);
        Some(Arc::from(name))
    };
    Ok(())
}

fn parse_min_spread_delay(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_spread_try_delay = INI::parse_duration_unsigned_int(first_value_token(tokens)?)?;
    Ok(())
}

fn parse_max_spread_delay(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_spread_try_delay = INI::parse_duration_unsigned_int(first_value_token(tokens)?)?;
    Ok(())
}

fn parse_spread_try_range(
    _ini: &mut INI,
    data: &mut FireSpreadUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.spread_try_range = INI::parse_real(first_value_token(tokens)?)?;
    Ok(())
}

const FIRE_SPREAD_UPDATE_FIELDS: &[FieldParse<FireSpreadUpdateModuleData>] = &[
    FieldParse {
        token: "OCLEmbers",
        parse: parse_ocl_embers,
    },
    FieldParse {
        token: "MinSpreadDelay",
        parse: parse_min_spread_delay,
    },
    FieldParse {
        token: "MaxSpreadDelay",
        parse: parse_max_spread_delay,
    },
    FieldParse {
        token: "SpreadTryRange",
        parse: parse_spread_try_range,
    },
];

/// FireSpreadUpdate - Spreads fire from burning objects to nearby flammable objects
/// Matches C++ FireSpreadUpdate.cpp:80-156
#[derive(Debug, Clone)]
pub struct FireSpreadUpdate {
    thing: ThingId,
    module_data: ModuleDataHandle,
    next_call_frame_and_phase: UnsignedInt,
}

/// Per-frame interface for the sleepy-update proxy. `FireSpreadUpdate` owns an
/// inherent `update_simple()` (it predates the shared trait), so the trait
/// impl forwards to it instead of the trait's `update()` default — identical to
/// what the former `downcast_mut::<FireSpreadUpdateModule>()` branch called.
impl game_engine::common::thing::update_module::UpdateModuleInterface for FireSpreadUpdate {
    fn update_simple(&mut self) -> game_engine::common::thing::update_module::UpdateSleepTime {
        FireSpreadUpdate::update_simple(self)
    }

    /// C++ FireSpreadUpdate.cpp:93-135 through the driving context's owned
    /// world: aflame check, authored OCL embers, partition closest-target
    /// selection, target ignition, and the logic-random retry draw.
    fn update_scheduled(
        &mut self,
        context: &mut dyn game_engine::common::thing::update_module::UpdateScheduleContext,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if !context.fire_spread_source_is_aflame(self.thing) {
            return Ok(UpdateSleepTime::Forever);
        }
        let (ocl, range, min, max) = {
            let data = self.module_data.read();
            let data = data
                .as_any()
                .downcast_ref::<FireSpreadUpdateModuleData>()
                .expect("invalid FireSpread data");
            (
                data.ocl_embers.clone(),
                data.spread_try_range,
                data.min_spread_try_delay,
                data.max_spread_try_delay,
            )
        };
        if let Some(ocl) = ocl.as_deref() {
            context.create_fire_spread_embers(self.thing, ocl);
        }
        if range != 0.0 {
            if let Some(id) = context.closest_fire_spread_target(self.thing, range) {
                context.ignite_fire_spread_target(id);
            }
        }
        Ok(UpdateSleepTime::Frames(
            (context.fire_spread_random_int(min as i32, max as i32) as u32).max(1),
        ))
    }

    fn record_scheduled_wake(&mut self, frame: u32, phase: crate::modules::SleepyUpdatePhase) {
        self.next_call_frame_and_phase = (frame.min(0x3fff_ffff) << 2) | phase as u32;
    }
}

impl FireSpreadUpdate {
    /// Create new FireSpreadUpdate module
    /// Matches C++ FireSpreadUpdate.cpp:80-83
    pub fn new(thing: ThingId, module_data: FireSpreadUpdateModuleData) -> Self {
        Self::with_module_data(thing, ModuleDataHandle::new(module_data))
    }

    pub fn with_module_data(thing: ThingId, module_data: ModuleDataHandle) -> Self {
        Self {
            thing,
            module_data,
            // The C++ FireSpreadUpdate constructor sets UPDATE_SLEEP_FOREVER.
            next_call_frame_and_phase: (0x3fff_ffff << 2)
                | crate::modules::SleepyUpdatePhase::Normal as u32,
        }
    }

    /// Scheduling-only `update_simple` has no owning context — the C++ body
    /// cannot run without a source object, partition, or logic-random owner.
    pub fn update_simple(&mut self) -> UpdateSleepTime {
        UpdateSleepTime::Forever
    }

    fn wake_delay_if_aflame(&mut self, is_aflame: bool) -> Option<u32> {
        is_aflame.then(|| self.calc_next_spread_delay())
    }

    /// Calculate next spread delay with randomization
    /// Matches C++ FireSpreadUpdate.cpp:149-156
    fn calc_next_spread_delay(&self) -> u32 {
        self.calc_next_spread_delay_with(game_logic_random_value)
    }

    fn calc_next_spread_delay_with(&self, mut draw: impl FnMut(u32, u32) -> u32) -> u32 {
        let (min, max) = {
            let guard = self.module_data.read();
            let data = guard.as_any().downcast_ref::<FireSpreadUpdateModuleData>().expect("invalid FireSpread data");
            (data.min_spread_try_delay, data.max_spread_try_delay)
        };
        draw(min, max).max(1)
    }

    /// C++ draws `getGameLogicRandomValue(min, max)` unconditionally — equal
    /// bounds still consume one draw from the supplied logic stream.
    fn wake_delay_if_aflame_with_random(
        &self,
        is_aflame: bool,
        random: &mut dyn game_engine::common::common_services::LogicRandom,
    ) -> Option<u32> {
        is_aflame.then(|| {
            self.calc_next_spread_delay_with(|min, max| {
                random.next_int(min as i32, max as i32) as u32
            })
        })
    }

    /// Save state to xfer
    /// Matches C++ FireSpreadUpdate.cpp:174-184
    pub fn save(&self, xfer: &mut dyn Xfer) {
        xfer.xfer_version_write(1);
        // No instance data to save, only module data
    }

    /// Load state from xfer
    /// Matches C++ FireSpreadUpdate.cpp:174-184
    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let version = xfer.xfer_version_read();
        if version >= 1 {
            // No instance data to load
        }
    }
}

impl Snapshotable for FireSpreadUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes FireSpreadUpdate through the common Module trait.
pub struct FireSpreadUpdateModule {
    behavior: FireSpreadUpdate,
    module_name_key: NameKeyType,
    module_data: ModuleDataHandle,
}

impl FireSpreadUpdateModule {
    pub fn new(
        mut behavior: FireSpreadUpdate,
        module_name: &AsciiString,
        module_data: ModuleDataHandle,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        behavior.module_data = module_data.clone();
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FireSpreadUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for FireSpreadUpdateModule {
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

impl Module for FireSpreadUpdateModule {
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
        self.module_data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.module_data)
    }

    fn get_fire_spread_control_interface(&mut self) -> Option<&mut dyn FireSpreadControlInterface> {
        Some(self)
    }

    /// C++ `UpdateModule : Module` interface query
    /// (`Module::DynamicInterfaceCast(ModuleInterfaceType::UPDATE)`): forwards
    /// to the wrapped behavior's per-frame hooks. Replaces the former
    /// `as_any().downcast_mut::<Self>()` branch in `ModuleUpdateProxy`.
    fn get_update_module_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        Some(&mut self.behavior)
    }

    /// C++ `UpdateModule` initial wake, decoded from the serialized packed
    /// base state (frame << 2 | phase).
    fn get_initial_wake_frame(&mut self) -> Option<u32> {
        Some(self.behavior.next_call_frame_and_phase >> 2)
    }

    /// Record the canonical scheduler deadline in the serialized base state.
    fn record_scheduled_wake(&mut self, frame: u32, phase: crate::modules::SleepyUpdatePhase) {
        self.behavior.record_scheduled_wake(frame, phase);
    }
}

impl FireSpreadControlInterface for FireSpreadUpdateModule {
    fn wake_delay_if_aflame(&mut self, is_aflame: bool) -> Option<u32> {
        self.behavior.wake_delay_if_aflame(is_aflame)
    }
    fn wake_delay_if_aflame_with_random(
        &mut self,
        is_aflame: bool,
        random: &mut dyn game_engine::common::common_services::LogicRandom,
    ) -> Result<Option<u32>, String> {
        Ok(self
            .behavior
            .wake_delay_if_aflame_with_random(is_aflame, random))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fire_spread_creation() {
        let data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 30,
            max_spread_try_delay: 60,
            spread_try_range: 50.0,
            ocl_embers: None,
        };

        let update = FireSpreadUpdate::new(1, data);
        let guard = update.module_data.read();
        let data = guard.as_any().downcast_ref::<FireSpreadUpdateModuleData>().unwrap();
        assert_eq!(data.min_spread_try_delay, 30);
        assert_eq!(data.max_spread_try_delay, 60);
        assert_eq!(data.spread_try_range, 50.0);
    }

    #[test]
    fn test_calc_next_spread_delay() {
        let data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 10,
            max_spread_try_delay: 20,
            spread_try_range: 100.0,
            ocl_embers: None,
        };

        let update = FireSpreadUpdate::new(1, data);

        // Test multiple times to verify randomness
        for _ in 0..10 {
            let delay = update.calc_next_spread_delay();
            assert!(delay >= 10 && delay <= 20, "Delay {} out of range", delay);
        }
    }

    #[test]
    fn test_minimum_delay() {
        let data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 0,
            max_spread_try_delay: 0,
            spread_try_range: 50.0,
            ocl_embers: None,
        };

        let update = FireSpreadUpdate::new(1, data);
        let delay = update.calc_next_spread_delay();

        // Should be at least 1 (C++ line 153-154)
        assert_eq!(delay, 1);
    }

    struct OrderContext {
        ember_done: bool,
        ignited: bool,
        draws: u32,
    }

    impl game_engine::common::thing::update_module::UpdateScheduleContext for OrderContext {
        fn frame(&self) -> u32 {
            20
        }

        fn fire_spread_source_is_aflame(&self, owner_id: u32) -> bool {
            owner_id == 9
        }

        fn create_fire_spread_embers(&mut self, owner_id: u32, ocl: &str) {
            assert_eq!(owner_id, 9);
            assert_eq!(ocl, "CustomEmberList");
            self.ember_done = true;
        }

        fn closest_fire_spread_target(&self, owner_id: u32, range: f32) -> Option<u32> {
            assert!(self.ember_done, "embers are created before target selection");
            assert_eq!(owner_id, 9);
            assert_eq!(range, 50.0);
            Some(10)
        }

        fn ignite_fire_spread_target(&mut self, object_id: u32) {
            assert_eq!(object_id, 10);
            self.ignited = true;
        }

        fn fire_spread_random_int(&mut self, lo: i32, hi: i32) -> i32 {
            assert!(self.ignited, "ignition runs before the retry draw");
            assert_eq!((lo, hi), (0, 0));
            self.draws += 1;
            0
        }

        fn awaken(
            &mut self,
            _module: &game_engine::common::thing::update_module::UpdateModulePtr,
            _wake_frame: u32,
        ) {
            panic!("scheduled FireSpread must not awaken");
        }

        fn unregister(
            &mut self,
            _module: &game_engine::common::thing::update_module::UpdateModulePtr,
        ) {
            panic!("scheduled FireSpread must not unregister");
        }
    }

    #[test]
    fn scheduled_update_orders_authored_embers_target_ignition_and_retry_draw() {
        let mut data = FireSpreadUpdateModuleData {
            module_tag_name_key: 0,
            min_spread_try_delay: 0,
            max_spread_try_delay: 0,
            spread_try_range: 50.0,
            ocl_embers: None,
        };
        let mut ini = INI::new();
        parse_ocl_embers(&mut ini, &mut data, &["=", "CustomEmberList"]).unwrap();
        assert_eq!(data.ocl_embers.as_deref(), Some("CustomEmberList"));

        let mut update = FireSpreadUpdate::new(9, data);
        let mut context = OrderContext {
            ember_done: false,
            ignited: false,
            draws: 0,
        };
        let sleep = game_engine::common::thing::update_module::UpdateModuleInterface::update_scheduled(
            &mut update,
            &mut context,
        )
        .unwrap();
        assert_eq!(sleep, UpdateSleepTime::Frames(1));
        assert!(context.ember_done);
        assert!(context.ignited);
        assert_eq!(context.draws, 1);
    }
}

fn game_logic_random_value(min: u32, max: u32) -> u32 {
    if min >= max {
        return min;
    }
    crate::helpers::game_logic_random_value(min, max)
}

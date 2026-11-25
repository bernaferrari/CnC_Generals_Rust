//! LifetimeUpdate - Rust conversion of C++ LifetimeUpdate
//!
//! Object destruction after a lifetime expires.
//! Author: Colin Day, December 2001 (C++ version)
//! Rust conversion: 2025

use crate::common::{AsciiString, Bool, Int, KindOf, UnsignedInt, XferVersion};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use crate::object::{ModuleEntry, Object as GameObject};
use game_engine::common::common_services::{LogicRandom, ReceivingLogicRandom};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    LifetimeControlInterface, Module, ModuleData as EngineModuleData, ModuleDataHandle,
    NameKeyType, Thing as ModuleThing,
};
use log::warn;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock, Weak};

const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

#[derive(Clone, Debug)]
pub struct LifetimeUpdateModuleData {
    pub base: BehaviorModuleData,
    pub min_frames: UnsignedInt,
    pub max_frames: UnsignedInt,
}

impl Default for LifetimeUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            min_frames: 0,
            max_frames: 0,
        }
    }
}

impl LifetimeUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LIFETIME_UPDATE_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(LifetimeUpdateModuleData, base);

fn parse_min_lifetime(
    _ini: &mut INI,
    data: &mut LifetimeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.min_frames = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
    Ok(())
}

fn parse_max_lifetime(
    _ini: &mut INI,
    data: &mut LifetimeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_frames = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const LIFETIME_UPDATE_FIELDS: &[FieldParse<LifetimeUpdateModuleData>] = &[
    FieldParse {
        token: "MinLifetime",
        parse: parse_min_lifetime,
    },
    FieldParse {
        token: "MaxLifetime",
        parse: parse_max_lifetime,
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;

    struct FrameRestore(u64);

    impl FrameRestore {
        fn at(frame: u64) -> Self {
            let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
            let previous = logic.get_frame() as u64;
            logic.set_current_frame(frame);
            Self(previous)
        }
    }

    impl Drop for FrameRestore {
        fn drop(&mut self) {
            crate::system::game_logic::get_game_logic()
                .lock()
                .unwrap()
                .set_current_frame(self.0);
        }
    }

    struct LifetimeOwner {
        logic: crate::system::game_logic::GameLogic,
        id: crate::common::ObjectID,
    }

    impl LifetimeOwner {
        fn admit(object: &Arc<RwLock<Object>>) -> Self {
            let id = object.read().unwrap().get_id();
            let mut logic = crate::system::game_logic::GameLogic::new();
            logic.register_object(object.clone()).unwrap();
            Self { logic, id }
        }
    }

    impl Drop for LifetimeOwner {
        fn drop(&mut self) {
            self.logic.destroy_object(self.id);
            self.logic
                .process_destroy_list()
                .expect("canonical lifetime fixture retirement");
        }
    }

    fn parse_field(data: &mut LifetimeUpdateModuleData, token: &str, values: &[&str]) {
        let field = LIFETIME_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn lifetime_fields_accept_ini_equals_token() {
        let mut data = LifetimeUpdateModuleData::default();

        parse_field(&mut data, "MinLifetime", &["=", "1000"]);
        parse_field(&mut data, "MaxLifetime", &["=", "2500"]);

        assert_eq!(data.min_frames, 30);
        assert_eq!(data.max_frames, 75);
    }

    #[test]
    fn hulk_lifetime_override_replaces_configured_range() {
        assert_eq!(
            LifetimeUpdate::effective_lifetime_range(true, 30, 75, 42),
            (42, 42)
        );
        assert_eq!(
            LifetimeUpdate::effective_lifetime_range(false, 30, 75, 42),
            (30, 75)
        );
        assert_eq!(
            LifetimeUpdate::effective_lifetime_range(true, 30, 75, -1),
            (30, 75)
        );
    }

    #[test]
    fn lifetime_update_module_exposes_typed_control_interface() {
        let data = game_engine::common::thing::module::ModuleDataHandle::new(LifetimeUpdateModuleData::default());
        let behavior = LifetimeUpdate {
            owner: Weak::new(),
            next_call_frame_and_phase: 0,
            die_frame: 123,
        };
        let mut module =
            LifetimeUpdateModule::new(behavior, &AsciiString::from("LifetimeUpdate"), data);

        let control = module
            .get_lifetime_control_interface()
            .expect("LifetimeUpdate should expose LifetimeControlInterface");

        assert_eq!(control.die_frame(), 123);
    }

    #[test]
    fn lifetime_module_reads_shared_definition() {
        let data = game_engine::common::thing::module::ModuleDataHandle::new(LifetimeUpdateModuleData::default());
        let behavior = LifetimeUpdate {
            owner: Weak::new(),
            next_call_frame_and_phase: 0,
            die_frame: 0,
        };
        let module = LifetimeUpdateModule::new(behavior, &AsciiString::from("LifetimeUpdate"), data.clone());
        data.write().as_any_mut().downcast_mut::<LifetimeUpdateModuleData>().unwrap().min_frames = 42;
        let guard = module.get_module_data();
        assert_eq!(guard.as_any().downcast_ref::<LifetimeUpdateModuleData>().unwrap().min_frames, 42);
    }

    #[test]
    fn lifetime_update_constructor_schedules_initial_wake_for_die_frame() {
        let _guard = crate::test_sync::lock();
        let _frame = FrameRestore::at(100);

        let object = Arc::new(RwLock::new(Object::new_test(4242, 100.0)));
        let module_data = game_engine::common::thing::module::ModuleDataHandle::new(LifetimeUpdateModuleData {
            min_frames: 30,
            max_frames: 30,
            ..LifetimeUpdateModuleData::default()
        });

        let behavior = LifetimeUpdate::new(object, module_data).expect("LifetimeUpdate creates");

        assert_eq!(behavior.get_die_frame(), 130);
        assert_eq!(behavior.initial_wake_frame(), 130);
    }

    #[test]
    fn update_kills_even_before_die_frame_when_invoked_like_cpp() {
        let _guard = crate::test_sync::lock();
        let _frame = FrameRestore::at(100);
        let object = Arc::new(RwLock::new(Object::new_test(4244, 100.0)));
        let foreign = Arc::new(RwLock::new(Object::new_test(4244, 100.0)));
        // The compatibility registry deliberately resolves this ID to a different Object.
        let foreign_registration = LifetimeOwner::admit(&foreign);
        let data = game_engine::common::thing::module::ModuleDataHandle::new(LifetimeUpdateModuleData {
            min_frames: 30,
            max_frames: 30,
            ..LifetimeUpdateModuleData::default()
        });
        let behavior =
            LifetimeUpdate::new(object.clone(), data.clone()).expect("LifetimeUpdate creates");
        assert_eq!(behavior.get_die_frame(), 130);
        let mut wrapper =
            LifetimeUpdateModule::new(behavior, &AsciiString::from("LifetimeUpdate"), data);
        assert_eq!(
            wrapper
                .get_update_module_interface()
                .unwrap()
                .update_simple(),
            UPDATE_SLEEP_FOREVER
        );
        {
            let object = object.read().unwrap();
            assert_eq!(object.get_health(), 0.0);
            assert!(object.is_effectively_dead());
            assert!(
                !object.is_destroyed(),
                "C++ LifetimeUpdate kills, not destroys"
            );
        }
        assert_eq!(foreign.read().unwrap().get_health(), 100.0);
        drop(object);
        assert_eq!(wrapper.behavior_mut().update_simple(), UPDATE_SLEEP_FOREVER);
        assert_eq!(foreign.read().unwrap().get_health(), 100.0);
        drop(foreign_registration);
    }

    /// Two independent worlds carrying the same numeric object id: the
    /// constructor must take owner identity, hulk policy, RNG stream and the
    /// base frame from the supplied handle — not whichever world happens to
    /// be published, and not an ambient-registry same-id peer.
    #[test]
    fn authored_lifetime_uses_supplied_owner_birth_frame_and_receiving_streams() {
        let _guard = crate::test_sync::lock();
        use crate::system::engine_stores::{new_for_world, with_active_stores};
        use game_engine::common::random_value::RandomState;

        let first_world = new_for_world();
        let second_world = new_for_world();

        let mut hulk_template =
            crate::common::DefaultThingTemplate::new("HulkOwner".to_string());
        hulk_template.add_kind_of(KindOf::Hulk);
        let mut first_object =
            Object::new_test_from_template(0x1F71, 100.0, Arc::new(hulk_template));
        let mut second_object = Object::new_test(0x1F71, 100.0);
        first_object.bind_owner_world_for_test(first_world.services());
        second_object.bind_owner_world_for_test(second_world.services());
        let first_arc = Arc::new(RwLock::new(first_object));
        let second_arc = Arc::new(RwLock::new(second_object));
        // Same-id peers registered on both sides so an ID-resolving
        // implementation finds a valid (but wrong) object instead of panicking.
        first_world
            .services()
            .object_registry()
            .register_object(0x1F71, &first_arc);
        second_world
            .services()
            .object_registry()
            .register_object(0x1F71, &second_arc);

        first_world
            .services()
            .session_flags()
            .hulk_max_lifetime_override
            .store(42, Ordering::Relaxed);
        second_world
            .services()
            .session_flags()
            .hulk_max_lifetime_override
            .store(84, Ordering::Relaxed);
        first_world.services().common().random.init_with_seed(123);
        second_world.services().common().random.init_with_seed(567);

        let mut first_reference = RandomState::default();
        first_reference.seed_random(123);
        let mut second_reference = RandomState::default();
        second_reference.seed_random(567);
        let second_initial_words = second_reference.seed_words();

        // Ambient frames differ from the supplied birth frames so an ambient
        // frame read produces a visibly wrong deadline.
        with_active_stores(&first_world, || {
            crate::system::game_logic::get_game_logic()
                .lock()
                .unwrap()
                .set_current_frame(100);
        });
        with_active_stores(&second_world, || {
            crate::system::game_logic::get_game_logic()
                .lock()
                .unwrap()
                .set_current_frame(200);
        });

        let module_data = || {
            ModuleDataHandle::new(LifetimeUpdateModuleData {
                min_frames: 30,
                max_frames: 75,
                ..Default::default()
            })
        };

        // Construct A while world B is the published one.
        let mut first_module = with_active_stores(&second_world, || {
            lifetime_update_module_factory(
                Arc::new(crate::object::ObjectThingHandle::with_world_and_birth_frame(
                    &first_arc,
                    Some(crate::object::ObjectWorld::of(first_world.services())),
                    Some(19),
                )),
                module_data(),
            )
        });
        // Hulk owner in world A: override 42 -> draw (42,42) on A's stream.
        let first_delay = first_reference.next_int(42, 42) as u32;
        let first_die = 19u32.wrapping_add(first_delay);
        assert_eq!(first_module.get_initial_wake_frame(), Some(first_die));
        assert_eq!(
            first_module
                .get_lifetime_control_interface()
                .map(|control| control.die_frame()),
            Some(first_die)
        );
        // A's stream advanced exactly once; B's untouched.
        assert_eq!(
            first_world
                .services()
                .common()
                .random
                .with_logic_mut(|random| random.seed_words()),
            first_reference.seed_words()
        );
        assert_eq!(
            second_world
                .services()
                .common()
                .random
                .with_logic_mut(|random| random.seed_words()),
            second_initial_words
        );

        // Construct B while world A is the published one.
        let mut second_module = with_active_stores(&first_world, || {
            lifetime_update_module_factory(
                Arc::new(crate::object::ObjectThingHandle::with_world_and_birth_frame(
                    &second_arc,
                    Some(crate::object::ObjectWorld::of(second_world.services())),
                    Some(71),
                )),
                module_data(),
            )
        });
        // Non-hulk owner in world B: authored range -> draw (30,75) on B's
        // stream; its own override 84 must not apply.
        let second_delay = second_reference.next_int(30, 75) as u32;
        let second_die = 71u32.wrapping_add(second_delay);
        assert_eq!(second_module.get_initial_wake_frame(), Some(second_die));
        assert_eq!(
            second_module
                .get_lifetime_control_interface()
                .map(|control| control.die_frame()),
            Some(second_die)
        );
        assert_eq!(
            second_world
                .services()
                .common()
                .random
                .with_logic_mut(|random| random.seed_words()),
            second_reference.seed_words()
        );
        assert_eq!(
            first_world
                .services()
                .common()
                .random
                .with_logic_mut(|random| random.seed_words()),
            first_reference.seed_words()
        );
    }
}

pub struct LifetimeUpdate {
    // Like C++ getObject(), this identifies the installed Object across ID restore.
    owner: Weak<RwLock<GameObject>>,
    next_call_frame_and_phase: UnsignedInt,
    die_frame: UnsignedInt,
}

impl LifetimeUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: ModuleDataHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_birth_frame(&object, &module_data, None)
    }

    /// C++ LifetimeUpdate.cpp:23-40: choose the receiving world's hulk policy,
    /// draw once even for fixed bounds, add the frame and schedule Normal.
    /// Legacy callers without a birth frame still read the clock through
    /// GameLogic. Only objects without a world use the ambient hulk policy
    /// and RNG.
    fn new_with_birth_frame(
        object: &Arc<RwLock<GameObject>>,
        module_data: &ModuleDataHandle,
        birth_frame: Option<u32>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (is_hulk, world) = {
            let object = object.read().map_err(|_| "LifetimeUpdate owner poisoned")?;
            (
                object.is_kind_of(KindOf::Hulk),
                object.owning_world_services(),
            )
        };
        let (min, max) = {
            let data = module_data.read();
            let data = data
                .downcast_ref::<LifetimeUpdateModuleData>()
                .ok_or("Invalid LifetimeUpdate module data")?;
            (data.min_frames, data.max_frames)
        };
        let now = birth_frame.unwrap_or_else(crate::helpers::TheGameLogic::get_frame);
        let override_frames = world.as_ref().map_or_else(
            crate::helpers::TheGameLogic::get_hulk_max_lifetime_override,
            |world| {
                world
                    .session_flags()
                    .hulk_max_lifetime_override
                    .load(Ordering::Relaxed)
            },
        );
        let (min, max) = Self::effective_lifetime_range(is_hulk, min, max, override_frames);
        let mut update = Self {
            owner: Arc::downgrade(object),
            next_call_frame_and_phase: 0,
            die_frame: 0,
        };
        let deadline = update.set_lifetime_range_at(now, min, max, |lo, hi| {
            match &world {
                Some(world) => ReceivingLogicRandom::new(world.common()).next_int(lo, hi),
                None => crate::helpers::get_game_logic_random_value(lo, hi),
            }
        });
        update.record_wake(deadline, crate::modules::SleepyUpdatePhase::Normal);
        Ok(update)
    }

    /// C++ `LifetimeUpdate::setLifetimeRange` (LifetimeUpdate.cpp:51-64):
    /// draw the delay through the caller's RNG, derive the die frame, and
    /// return it so the owning scheduler can adjust the registration
    /// (GameLogic.cpp:2965-3004) instead of this module re-registering itself.
    /// An inverted authored range keeps the Core helper's legacy guard.
    pub(crate) fn set_lifetime_range_at(
        &mut self,
        now: u32,
        min: u32,
        max: u32,
        draw: impl FnOnce(i32, i32) -> i32,
    ) -> u32 {
        let (lo, hi) = (min as i32, max as i32);
        let frames = if hi < lo { hi } else { draw(lo, hi) };
        let delay = (frames as u32).max(1);
        self.die_frame = Self::die_frame_after(now, delay);
        self.die_frame
    }

    /// LifetimeUpdate.cpp:64 adds unsigned frames; the scheduler caps its
    /// wake separately. `die_frame` and the 30-bit scheduler wake are
    /// distinct quantities — C++ overflow wraps, it does not saturate.
    fn die_frame_after(now: u32, delay: u32) -> u32 {
        now.wrapping_add(delay)
    }

    /// C++ `UpdateModule::setWakeFrame` (UpdateModule.h:181-188): pack the
    /// wake frame and phase into the serialized call frame.
    fn record_wake(&mut self, frame: u32, phase: crate::modules::SleepyUpdatePhase) {
        self.next_call_frame_and_phase = (frame.min(0x3fff_ffff) << 2) | phase as u32;
    }

    pub fn get_die_frame(&self) -> UnsignedInt {
        self.die_frame
    }

    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.next_call_frame_and_phase >> 2
    }

    fn effective_lifetime_range(
        is_hulk: Bool,
        min_frames: UnsignedInt,
        max_frames: UnsignedInt,
        override_frames: Int,
    ) -> (UnsignedInt, UnsignedInt) {
        if is_hulk && override_frames != -1 {
            let override_frames = override_frames as UnsignedInt;
            (override_frames, override_frames)
        } else {
            (min_frames, max_frames)
        }
    }

    /// Get remaining frames until death
    pub fn get_remaining_frames(&self) -> UnsignedInt {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.die_frame.saturating_sub(current_frame)
    }

    /// Check if the object has expired
    pub fn is_expired(&self) -> bool {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        current_frame >= self.die_frame
    }

    fn expire(owner: Option<Arc<RwLock<GameObject>>>) -> UpdateSleepTime {
        if let Some(object) = owner {
            if let Ok(mut guard) = object.write() {
                guard.kill(None, None);
            } else {
                return UpdateSleepTime::None;
            }
        }
        UPDATE_SLEEP_FOREVER
    }
}

impl UpdateModuleInterface for LifetimeUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        LifetimeUpdate::expire(self.owner.upgrade())
    }

    fn record_scheduled_wake(&mut self, frame: u32, phase: crate::modules::SleepyUpdatePhase) {
        self.record_wake(frame, phase);
    }
}

impl BehaviorModuleInterface for LifetimeUpdate {
    fn get_module_name(&self) -> &'static str {
        "LifetimeUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for LifetimeUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // version -- C++ LifetimeUpdate.cpp line 96: currentVersion = 1
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("LifetimeUpdate version xfer failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        // die frame -- C++ LifetimeUpdate.cpp line 104
        xfer.xfer_unsigned_int(&mut self.die_frame)
            .map_err(|e| format!("LifetimeUpdate die_frame xfer failed: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes LifetimeUpdate through the common Module trait.
pub struct LifetimeUpdateModule {
    behavior: LifetimeUpdate,
    module_name_key: NameKeyType,
    module_data: game_engine::common::thing::module::ModuleDataHandle,
}

impl LifetimeUpdateModule {
    pub fn new(
        behavior: LifetimeUpdate,
        module_name: &AsciiString,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut LifetimeUpdate {
        &mut self.behavior
    }

    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.behavior.initial_wake_frame()
    }

    /// LifetimeUpdate.cpp:69–74 kills synchronously. Release the installed
    /// module before invoking death hooks, which can revisit the same entry.
    pub(in crate::object) fn run_installed(entry: &ModuleEntry) -> Option<UpdateSleepTime> {
        let owner = entry.with_module(|module| {
            module
                .as_any()
                .downcast_ref::<Self>()
                .map(|lifetime| lifetime.behavior.owner.upgrade())
        })?;
        Some(LifetimeUpdate::expire(owner))
    }
}

impl Snapshotable for LifetimeUpdateModule {
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

impl Module for LifetimeUpdateModule {
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

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.module_data)
    }

    fn get_lifetime_control_interface(&mut self) -> Option<&mut dyn LifetimeControlInterface> {
        Some(self)
    }

    /// C++ `UpdateModule : Module` interface query
    /// (`Module::DynamicInterfaceCast(ModuleInterfaceType::UPDATE)`): forwards
    /// to the wrapped behavior's per-frame hooks. Replaces the former
    /// `as_any().downcast_mut::<Self>()` branch in `ModuleUpdateProxy`.
    fn get_update_module_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        Some(self.behavior_mut())
    }

    /// Sleepy-update subset (disabled-state mask + phase hint); see
    /// `Module::get_sleepy_update_interface` for why this set is narrower than
    /// the full update interface.
    fn get_sleepy_update_interface(
        &mut self,
    ) -> Option<&mut dyn game_engine::common::thing::update_module::UpdateModuleInterface> {
        Some(self.behavior_mut())
    }

    /// C++ `UpdateModule` initial wake frame, reached through the typed
    /// `Module::get_initial_wake_frame()` accessor instead of the former
    /// `as_any().downcast_ref::<Self>()` chain in `initial_update_wake_frame`.
    fn get_initial_wake_frame(&mut self) -> Option<u32> {
        Some(self.initial_wake_frame())
    }

    fn record_scheduled_wake(
        &mut self,
        frame: u32,
        phase: game_engine::common::thing::update_module::SleepyUpdatePhase,
    ) {
        self.behavior.record_wake(frame, phase);
    }
}

impl LifetimeControlInterface for LifetimeUpdateModule {
    fn die_frame(&self) -> u32 {
        self.behavior.get_die_frame()
    }
}

pub struct LifetimeUpdateFactory;
impl LifetimeUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(LifetimeUpdate::new(thing, module_data)?))
    }
}

pub fn lifetime_update_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = LifetimeUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LifetimeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn lifetime_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: ModuleDataHandle,
) -> Box<dyn Module> {
    let object = crate::object::supplied_module_owner(thing.as_ref())
        .expect("LifetimeUpdate requires its supplied Object owner");
    let frame = crate::object::supplied_module_birth_frame(thing.as_ref());
    let behavior = LifetimeUpdate::new_with_birth_frame(&object, &module_data, frame)
        .expect("LifetimeUpdate must initialize from its supplied Object and module data");
    let module_name = AsciiString::from("LifetimeUpdate");
    Box::new(LifetimeUpdateModule::new(
        behavior,
        &module_name,
        module_data,
    ))
}

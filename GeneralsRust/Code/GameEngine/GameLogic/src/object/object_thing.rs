//! Thing/handle adapters, display, score/bounty traits, and [`ObjectArcExt`].

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl DrawableExt for Object {
    fn get_drawable(&self) -> Option<Arc<RwLock<Drawable>>> {
        self.drawable.clone()
    }

    fn set_drawable(&mut self, drawable: Option<Arc<RwLock<Drawable>>>) {
        self.drawable = drawable;
        self.update_drawable_team_visuals();

        if self.drawable.is_some() {
            let time_of_day = TimeOfDay::Morning;
            for entry in &self.modules {
                entry.with_module(|module| {
                    module.on_drawable_bound_to_object();
                    module.preload_assets(time_of_day);
                });
            }
        }
    }
}

// Implement Thing trait for higher-level gameplay API
impl Thing for Object {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_object_id(&self) -> Option<ObjectID> {
        Some(self.get_id())
    }

    fn get_template(&self) -> Option<&dyn ThingTemplate> {
        Some(self.thing_template.as_ref())
    }

    fn get_position(&self) -> &Coord3D {
        Object::get_position(self)
    }

    fn set_position(&mut self, pos: &Coord3D) {
        let _ = Object::set_position(self, pos);
    }

    fn get_angle(&self) -> Real {
        self.geometry_info.angle
    }

    fn set_angle(&mut self, angle: Real) {
        let _ = Object::set_orientation(self, angle);
    }
}

impl engine_module::Object for Object {
    fn get_object_id(&self) -> ObjectID {
        self.id
    }

    fn get_behavior_modules(&self) -> Vec<Arc<dyn engine_module::Module>> {
        self.modules
            .iter()
            .map(|entry| {
                Arc::new(BehaviorModuleProxy::new(Arc::clone(entry)))
                    as Arc<dyn engine_module::Module>
            })
            .collect()
    }

    fn init_object(&self) {
        // The engine-facing Object trait only provides `&self`; mutating here would
        // require undefined behavior. Real initialization occurs on owned object handles.
    }
}

impl engine_module::Thing for Object {
    fn as_object(&self) -> Option<&dyn engine_module::Object> {
        Some(self)
    }

    fn as_drawable(&self) -> Option<&dyn engine_module::Drawable> {
        None
    }
}

impl Object {
    /// The world this Object was assembled in, or whose GameLogic admitted
    /// it (C++ the GameEngine that owns it).
    pub(crate) fn owning_world_services(
        &self,
    ) -> Option<Arc<crate::system::engine_stores::WorldServices>> {
        self.owner_world
            .as_ref()
            .and_then(Weak::upgrade)
            .or_else(|| self.pathfinding_owner.as_ref().map(|owner| Arc::clone(owner.services())))
    }

    /// Fixture Objects built outside assembly join `world` as if assembled there.
    #[cfg(test)]
    pub(crate) fn bind_owner_world_for_test(
        &mut self,
        world: &Arc<crate::system::engine_stores::WorldServices>,
    ) {
        self.owner_world = Some(Arc::downgrade(world));
    }
}

/// The receiving world an Object is assembled in: C++ resolves its
/// `TheGameLogic`, `TheCaveSystem` and `ThePlayerList` from the GameEngine that
/// owns the Object. Modules keep this instead of consulting the published world.
#[derive(Clone)]
pub struct ObjectWorld {
    world: Weak<crate::system::engine_stores::WorldServices>,
    cave: Arc<Mutex<crate::system::cave_system::CaveSystem>>,
}

impl ObjectWorld {
    /// The world's own services and CaveSystem.
    pub fn of(world: &Arc<crate::system::engine_stores::WorldServices>) -> Self {
        Self::with_cave(world, Arc::clone(&world.logic_managers().cave))
    }

    /// `world` with an explicitly supplied CaveSystem (fixtures).
    pub fn with_cave(
        world: &Arc<crate::system::engine_stores::WorldServices>,
        cave: Arc<Mutex<crate::system::cave_system::CaveSystem>>,
    ) -> Self {
        Self { world: Arc::downgrade(world), cave }
    }

    pub fn cave(&self) -> &Arc<Mutex<crate::system::cave_system::CaveSystem>> {
        &self.cave
    }

    pub fn world(&self) -> &Weak<crate::system::engine_stores::WorldServices> {
        &self.world
    }
}

impl std::fmt::Debug for ObjectWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectWorld")
            .field("world", &self.world.as_ptr())
            .field("cave", &Arc::as_ptr(&self.cave))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ObjectThingHandle {
    object: Weak<RwLock<Object>>,
    world: Option<ObjectWorld>,
}

impl ObjectThingHandle {
    pub(crate) fn new(object: &Arc<RwLock<Object>>) -> Self {
        Self {
            object: Arc::downgrade(object),
            world: None,
        }
    }

    pub(crate) fn with_world(object: &Arc<RwLock<Object>>, world: Option<ObjectWorld>) -> Self {
        Self { object: Arc::downgrade(object), world }
    }

    fn with_object<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&Object) -> R,
    {
        self.object
            .upgrade()
            .and_then(|arc| arc.read().ok().map(|guard| f(&*guard)))
    }
}

impl ModuleObjectTrait for ObjectThingHandle {
    fn get_object_id(&self) -> ObjectID {
        self.with_object(|object| object.get_id())
            .unwrap_or(INVALID_ID)
    }

    fn get_behavior_modules(&self) -> Vec<Arc<dyn engine_module::Module>> {
        self.with_object(|object| {
            object
                .modules
                .iter()
                .map(|entry| {
                    Arc::new(BehaviorModuleProxy::new(Arc::clone(entry)))
                        as Arc<dyn engine_module::Module>
                })
                .collect()
        })
        .unwrap_or_default()
    }

    fn init_object(&self) {
        if let Some(arc) = self.object.upgrade() {
            if let Ok(guard) = arc.write() {
                let _ = guard.init_object();
            }
        }
    }

    fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn engine_module::Object>>> {
        None
    }

    fn remove_upgrade(
        &self,
        upgrade_template: Option<&game_engine::common::system::upgrade::UpgradeTemplate>,
    ) {
        let Some(template) = upgrade_template else {
            return;
        };
        // C++ Object::removeUpgrade clears the template's own mask bit.
        let mask_bits =
            crate::common::UpgradeMaskType::from_bits_retain(template.get_mask().bits());
        if mask_bits.is_empty() {
            return;
        }

        if let Some(arc) = self.object.upgrade() {
            if let Ok(mut guard) = arc.write() {
                guard.remove_upgrade_mask(mask_bits);
            }
        }
    }
}

impl ModuleThing for ObjectThingHandle {
    fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
        Some(self)
    }

    fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
        None
    }
}

#[derive(Debug, Clone)]
struct ObjectDrawableThingHandle {
    object: ObjectThingHandle,
    drawable: DrawableThingHandle,
}

impl ObjectDrawableThingHandle {
    fn new(object: ObjectThingHandle, drawable: DrawableThingHandle) -> Self {
        Self { object, drawable }
    }
}

impl ModuleThing for ObjectDrawableThingHandle {
    fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
        Some(&self.object)
    }

    fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
        Some(&self.drawable)
    }
}

/// Pin the exact owner carried by a factory Thing. Numeric IDs never select
/// a replacement object, including after this handle's owner is retired.
pub(crate) fn supplied_module_owner(
    thing: &dyn ModuleThing,
) -> Result<Arc<RwLock<Object>>, String> {
    let thing: &dyn std::any::Any = thing;
    let handle = if let Some(handle) = thing.downcast_ref::<ObjectThingHandle>() {
        handle
    } else if let Some(handle) = thing.downcast_ref::<ObjectDrawableThingHandle>() {
        &handle.object
    } else {
        return Err("module Thing does not carry an exact Object owner".into());
    };
    handle
        .object
        .upgrade()
        .ok_or("module Thing owner has retired".into())
}

/// Receiving world passed at Object assembly, if that assembly has a driving world.
pub(crate) fn supplied_module_world(thing: &dyn ModuleThing) -> Option<ObjectWorld> {
    let thing: &dyn std::any::Any = thing;
    let handle = if let Some(handle) = thing.downcast_ref::<ObjectThingHandle>() {
        handle
    } else {
        &thing.downcast_ref::<ObjectDrawableThingHandle>()?.object
    };
    handle.world.clone()
}

pub(crate) fn make_drawable_module_thing_handle(
    object: &Arc<RwLock<Object>>,
    drawable: &Arc<RwLock<Drawable>>,
) -> Arc<dyn ModuleThing> {
    let object_handle = ObjectThingHandle::new(object);
    let drawable_handle = DrawableThingHandle::new(drawable);
    Arc::new(ObjectDrawableThingHandle::new(
        object_handle,
        drawable_handle,
    ))
}

// Display implementation for debugging
impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.name.is_empty() {
            let team_name = {
                let team = self.get_team();
                team.and_then(|t| t.try_read().ok().map(|g| g.get_name().to_string()))
                    .unwrap_or_else(|| "None".to_string())
            };
            write!(
                f,
                "Object {} ({}) [Team: {}]",
                self.id, self.name, team_name
            )
        } else {
            let team_name = {
                let team = self.get_team();
                team.and_then(|t| t.try_read().ok().map(|g| g.get_name().to_string()))
                    .unwrap_or_else(|| "None".to_string())
            };
            write!(f, "Object {} [Team: {}]", self.id, team_name)
        }
    }
}

// Object has no raw pointers / UnsafeCell. `Send`/`Sync` come from field types
// (`Arc<Mutex<dyn Trait + Send + Sync>>`, IDs, plain data). This is never
// called; rustc still checks the bound so a non-auto field cannot sneak back
// in behind a blanket `unsafe impl`.
fn _object_is_send_sync() {
    fn assert_ss<T: Send + Sync>() {}
    assert_ss::<Object>();
}

/// Extension trait for Arc<rhai::Locked<Object>> to provide helper methods
pub trait ObjectArcExt {
    fn get_kind_of(&self) -> KindOfMask;
    fn is_kind_of(&self, kind: KindOf) -> bool;
    fn is_any_kind_of(&self, kinds: &[KindOf]) -> bool;
    fn set_disabled_until(&self, disabled_type: DisabledType, frame: UnsignedInt);
    fn is_special_zero_slot_container(&self) -> bool;
    fn is_effectively_dead(&self) -> bool;
    fn find_flammable_update(&self) -> Option<BehaviorInterfaceHandle>;
}

impl ObjectArcExt for Arc<rhai::Locked<Object>> {
    /// Get the kind of the object
    fn get_kind_of(&self) -> KindOfMask {
        if let Ok(guard) = self.read() {
            guard.get_kind_of()
        } else {
            0
        }
    }

    /// Check if object is of the specified kind
    fn is_kind_of(&self, kind: KindOf) -> bool {
        if let Ok(guard) = self.read() {
            guard.is_kind_of(kind)
        } else {
            false
        }
    }

    /// Check if object is any of the specified kinds
    /// Returns true if the object matches any kind in the slice
    fn is_any_kind_of(&self, kinds: &[KindOf]) -> bool {
        if let Ok(guard) = self.read() {
            guard.is_any_kind_of(kinds)
        } else {
            false
        }
    }

    /// Set disabled state until a specific frame
    /// This allows temporary disabling of objects (e.g., EMP effects)
    fn set_disabled_until(&self, disabled_type: DisabledType, frame: UnsignedInt) {
        if let Ok(mut guard) = self.write() {
            guard.set_disabled_until(disabled_type, frame);
        }
    }

    /// Check if this object is a special zero-slot container (like a parachute)
    /// Zero-slot containers don't count towards normal containment limits
    fn is_special_zero_slot_container(&self) -> bool {
        if let Ok(guard) = self.read() {
            // Check if this object has a contain module
            if let Some(contain) = &guard.contain {
                if let Ok(contain_guard) = contain.lock() {
                    // A zero-slot container has a max capacity of 0
                    // This is typical for parachute containers
                    return contain_guard.get_max_capacity() == 0;
                }
            }
        }
        false
    }

    /// Check if object is effectively dead
    fn is_effectively_dead(&self) -> bool {
        if let Ok(guard) = self.read() {
            guard.is_effectively_dead()
        } else {
            false
        }
    }

    /// Find the flammable update module for this object.
    /// Returns None if object has no flammable update module.
    fn find_flammable_update(&self) -> Option<BehaviorInterfaceHandle> {
        let guard = self.read().ok()?;
        for mut module in guard.get_behavior_modules() {
            let would_ignite = {
                let Ok(module_guard) = module.try_access() else {
                    continue;
                };
                module_guard
                    .as_any()
                    .downcast_ref::<crate::object::behavior::flammable_update::FlammableUpdate>()
                    .map(|flammable| flammable.would_ignite())
                    .unwrap_or(false)
            };
            if would_ignite {
                return Some(module);
            }
        }
        None
    }
}

// =========================================================
// Trait Implementations for ScoreKeeper and Bounty System
// These allow the Object to work with Player and ScoreKeeper
// without creating circular dependencies.
// =========================================================

impl game_engine::common::rts::score_keeper::ScoreableObject for Object {
    fn get_score_template_name(&self) -> &str {
        self.get_template_name()
    }

    fn get_score_kindof_mask(&self) -> game_engine::common::rts::score_keeper::KindOfMaskType {
        // Convert from the game's KindOf to the score_keeper's KindOfMaskType
        use game_engine::common::rts::score_keeper::KindOf as ScoreKindOf;

        let mut mask = game_engine::common::rts::score_keeper::KindOfMaskType::new();

        // Map the game's KindOf to score_keeper's simplified KindOf
        // Note: We use `is_kind_of` which takes crate::common::KindOf
        if self.is_kind_of(KindOf::Structure) {
            mask.set(ScoreKindOf::Structure);
        }
        if self.is_kind_of(KindOf::Infantry) {
            mask.set(ScoreKindOf::Infantry);
        }
        if self.is_kind_of(KindOf::Vehicle) {
            mask.set(ScoreKindOf::Vehicle);
        }
        if self.is_kind_of(KindOf::Score) {
            mask.set(ScoreKindOf::Score);
        }
        if self.is_kind_of(KindOf::ScoreCreate) {
            mask.set(ScoreKindOf::ScoreCreate);
        }
        if self.is_kind_of(KindOf::ScoreDestroy) {
            mask.set(ScoreKindOf::ScoreDestroy);
        }
        mask
    }

    fn get_score_controlling_player_index(&self) -> Option<i32> {
        self.get_controlling_player()
            .and_then(|p| p.read().ok().map(|g| g.get_player_index()))
    }

    fn is_score_under_construction(&self) -> bool {
        self.test_status(ObjectStatusTypes::UnderConstruction)
    }
}

impl game_engine::common::rts::player::BountyObject for Object {
    fn calc_cost_to_build(&self) -> i32 {
        let player = self.get_controlling_player();
        let Ok(guard) = player.as_ref().map(|player| player.read()).transpose() else {
            return self.thing_template.calc_cost_to_build(None);
        };
        match guard.as_ref() {
            Some(guard) => self
                .thing_template
                .calc_cost_to_build(Some(&**guard as &dyn std::any::Any)),
            None => self.thing_template.calc_cost_to_build(None),
        }
    }

    fn is_under_construction(&self) -> bool {
        self.test_status(ObjectStatusTypes::UnderConstruction)
    }

    fn bounty_anchor(&self) -> (f32, f32, f32) {
        let pos = self.get_position();
        (pos.x, pos.y, pos.z)
    }
}

impl game_engine::common::rts::player::SkillPointObject for Object {
    fn get_skill_point_value(
        &self,
        _killer: &dyn game_engine::common::rts::player::SkillPointObject,
    ) -> i32 {
        let level = self.get_veterancy_level() as usize;
        self.thing_template.get_skill_point_value(level)
    }

    fn get_veterancy_level(&self) -> i32 {
        // Get veterancy level from experience tracker if available
        if let Some(tracker) = &self.experience_tracker {
            return tracker.get_veterancy_level() as i32;
        }
        0
    }

    fn is_under_construction(&self) -> bool {
        self.test_status(crate::common::ObjectStatusTypes::UnderConstruction)
    }
}

#[cfg(test)]
mod owning_world_eva_tests {
    use super::*;
    use crate::common::{DefaultThingTemplate, ObjectStatusMaskType};
    use crate::helpers::{EvaEvent, TheEva};
    use crate::system::engine_stores::{with_world_services, EngineStores};

    fn object(id: ObjectID) -> Arc<RwLock<Object>> {
        let template = Arc::new(DefaultThingTemplate::new(format!("EvaOwner{id}")));
        Object::new_with_id(template, id, ObjectStatusMaskType::none(), None).expect("test object")
    }

    #[test]
    fn object_eva_lands_in_the_world_that_admitted_the_object() {
        // Two worlds on one thread: construction-complete, superweapon-launch and
        // stealth-detection Eva go to the GameEngine owning the Object (C++),
        // whichever world happens to be published.
        let a = Arc::new(EngineStores::new_for_world());
        let b = Arc::new(EngineStores::new_for_world());
        let in_a = object(71);
        let in_b = object(72);
        let unadmitted = object(73);
        in_a.write().unwrap().bind_pathfinding_owner(&a).unwrap();
        in_b.write().unwrap().bind_pathfinding_owner(&b).unwrap();
        assert!(Arc::ptr_eq(
            &in_b.read().unwrap().owning_world_services().unwrap(),
            b.services()
        ));

        with_world_services(a.services(), || {
            TheEva::set_should_play_for(&in_b.read().unwrap(), EvaEvent::SuperweaponLaunchedOwnNuke)
                .unwrap();
            // An Object of no world plays nothing, even with a world published.
            TheEva::set_should_play_for(&unadmitted.read().unwrap(), EvaEvent::UnitLost).unwrap();
        });
        with_world_services(b.services(), || {
            TheEva::set_should_play_for(&in_a.read().unwrap(), EvaEvent::BuildingLost).unwrap();
        });
        assert_eq!(
            TheEva::drain_events_in(a.services()).unwrap(),
            vec![EvaEvent::BuildingLost]
        );
        assert_eq!(
            TheEva::drain_events_in(b.services()).unwrap(),
            vec![EvaEvent::SuperweaponLaunchedOwnNuke]
        );
    }

    #[test]
    fn main_admitted_object_eva_and_control_bar_stay_in_its_world() {
        // An Object prepared by Main's world GameLogic (no core EngineStores)
        // keeps its world from assembly: its Eva, ControlBar and markUIDirty
        // land there while another world is published.
        use crate::system::engine_stores::new_world_services;
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct DirtyHooks(Arc<AtomicUsize>);
        impl crate::control_bar::ControlBarUiHooks for DirtyHooks {
            fn mark_ui_dirty(&self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
            fn on_player_science_purchase_points_changed(&self, _: i32, _: i32) {}
            fn on_player_rank_changed(&self, _: i32, _: i32, _: i32) {}
        }
        let main = new_world_services();
        let other = new_world_services();
        let dirty_main = Arc::new(AtomicUsize::new(0));
        let dirty_other = Arc::new(AtomicUsize::new(0));
        *main.control_bar_ui_hooks().lock().unwrap() =
            Some(Arc::new(DirtyHooks(Arc::clone(&dirty_main))));
        *other.control_bar_ui_hooks().lock().unwrap() =
            Some(Arc::new(DirtyHooks(Arc::clone(&dirty_other))));
        let main_bridge = Arc::new(crate::control_bar::ControlBarBridge::from_command_sets(vec![
            crate::command_button::CommandSet::new("MainSet".to_string()),
        ]));
        *main.control_bar_bridge().write().unwrap() = Some(Arc::clone(&main_bridge));
        *other.control_bar_bridge().write().unwrap() =
            Some(Arc::new(crate::control_bar::ControlBarBridge::from_command_sets(vec![])));

        let mut logic = crate::system::game_logic::GameLogic::new_with_world_services(Arc::clone(&main));
        let mut factory = crate::object::object_factory::ObjectFactory::new();
        let id = factory
            .prepare_object(
                &mut logic,
                Arc::new(DefaultThingTemplate::new("MainAdmitted".into())),
                crate::common::Coord3D::ZERO,
                None,
                crate::object::object_factory::ObjectCreationFlags::NO_AI,
            )
            .expect("prepared object");
        let object = factory
            .get_object(id)
            .and_then(|slot| slot.get_base_object())
            .expect("prepared Object");

        with_world_services(&other, || {
            let guard = object.read().unwrap();
            assert!(Arc::ptr_eq(&guard.owning_world_services().expect("own world"), &main));
            TheEva::set_should_play_for(&guard, EvaEvent::UnitLost).unwrap();
            let bridge = crate::control_bar::control_bar_bridge_for(&guard).expect("Main's ControlBar");
            assert!(Arc::ptr_eq(&bridge, &main_bridge));
            crate::control_bar::mark_ui_dirty_for(&guard);
        });
        assert_eq!(TheEva::drain_events_in(&main).unwrap(), vec![EvaEvent::UnitLost]);
        assert!(TheEva::drain_events_in(&other).unwrap().is_empty());
        assert_eq!(
            (dirty_main.load(Ordering::Relaxed), dirty_other.load(Ordering::Relaxed)),
            (1, 0)
        );
    }

}

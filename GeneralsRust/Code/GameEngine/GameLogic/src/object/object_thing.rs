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
        self.geometry_info.angle = angle;
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

#[derive(Debug, Clone)]
pub(crate) struct ObjectThingHandle {
    object: Weak<RwLock<Object>>,
}

impl ObjectThingHandle {
    pub(crate) fn new(object: &Arc<RwLock<Object>>) -> Self {
        Self {
            object: Arc::downgrade(object),
        }
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
        upgrade_template: Option<&game_engine::common::ini::ini_upgrade::UpgradeTemplate>,
    ) {
        let Some(template) = upgrade_template else {
            return;
        };
        let upgrade_name = template.name.as_str();
        if upgrade_name.is_empty() {
            return;
        }

        let mask_bits = upgrade_mask_for_ascii(upgrade_name);
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
    fn find_flammable_update(&self) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>>;
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
    fn find_flammable_update(&self) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        let guard = self.read().ok()?;
        for module in guard.get_behavior_modules() {
            if let Ok(module_guard) = module.try_lock() {
                if module_guard
                    .as_any()
                    .downcast_ref::<crate::object::behavior::flammable_update::FlammableUpdate>()
                    .map(|flammable| flammable.would_ignite())
                    .unwrap_or(false)
                {
                    return Some(Arc::clone(&module));
                }
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
    fn get_build_cost(&self) -> i32 {
        // Get cost from template - pass None for player since we don't have easy access here
        self.thing_template.calc_cost_to_build(None)
    }

    fn is_under_construction(&self) -> bool {
        self.test_status(ObjectStatusTypes::UnderConstruction)
    }
}

impl game_engine::common::rts::player::SkillPointObject for Object {
    fn get_skill_point_value(
        &self,
        _killer: &dyn game_engine::common::rts::player::SkillPointObject,
    ) -> i32 {
        // Get experience value from experience tracker if available
        // Use object cost as a basis for skill point value
        if let Some(tracker) = &self.experience_tracker {
            if let Ok(tracker_guard) = tracker.lock() {
                // Get the build cost as a basis for skill points
                let cost = self.thing_template.calc_cost_to_build(None);
                // killer is never an ally for skill point calculation in this context
                return tracker_guard.get_experience_value(cost, false);
            }
        }
        0
    }

    fn get_veterancy_level(&self) -> i32 {
        // Get veterancy level from experience tracker if available
        if let Some(tracker) = &self.experience_tracker {
            if let Ok(tracker_guard) = tracker.lock() {
                return tracker_guard.get_veterancy_level() as i32;
            }
        }
        0
    }
}

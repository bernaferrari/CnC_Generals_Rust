//! Split-out inherent `module accessors and install helpers` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    // Module access
    pub fn get_body_module(&self) -> Option<Arc<Mutex<dyn BodyModuleInterface>>> {
        self.body.clone()
    }

    /// Compatibility alias that mirrors the original C++ Object API.
    pub fn get_body(&self) -> Option<Arc<Mutex<dyn BodyModuleInterface>>> {
        self.get_body_module()
    }

    #[allow(dead_code)]
    pub(crate) fn set_body_module(&mut self, body: Option<Arc<Mutex<dyn BodyModuleInterface>>>) {
        self.body = body;
    }

    pub fn get_contain(&self) -> Option<Arc<Mutex<dyn ContainModuleInterface>>> {
        self.contain.clone()
    }

    pub fn set_contain(&mut self, contain: Option<Arc<Mutex<dyn ContainModuleInterface>>>) {
        self.contain = contain;
    }

    /// Mark whether this object is currently transporting occupants (used by containment modules).
    pub fn set_is_transporting(&mut self, transporting: Bool) {
        self.is_transporting = transporting;
    }

    /// Whether this object currently holds occupants.
    pub fn is_transporting(&self) -> Bool {
        self.is_transporting
    }

    pub fn get_stealth(&self) -> Option<StealthUpdateHandle> {
        self.stealth.clone()
    }

    pub fn get_stealth_module(&self) -> Option<StealthUpdateHandle> {
        self.get_stealth()
    }

    pub fn is_stealthed(&self) -> bool {
        if let Some(handle) = &self.stealth {
            if let Ok(stealth) = handle.lock() {
                return stealth.is_stealthed();
            }
        }
        false
    }

    pub fn set_stealth_module(&mut self, module: StealthUpdateHandle) {
        self.stealth = Some(module);
    }

    pub fn get_ai_update_interface(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.ai.clone()
    }

    /// Mutable access to AI update interface (note: Arc<Mutex<>> already provides interior mutability)
    pub fn get_ai_update_interface_mut(&mut self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.ai.clone()
    }

    pub fn get_ai(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.get_ai_update_interface()
    }

    pub fn set_ai_update_interface(&mut self, ai: Option<Arc<Mutex<dyn AIUpdateInterface>>>) {
        self.ai = ai;
    }

    pub fn attach_ai_update_to_module(&mut self, ai: Arc<Mutex<dyn AIUpdateInterface>>) {
        for entry in &self.modules {
            entry.with_module(|module| {
                if let Some(ai_module) = (module as &mut dyn Any)
                    .downcast_mut::<crate::object::update::ai_update_interface::AIUpdateInterfaceModule>()
                {
                    ai_module.set_runtime_ai(Arc::clone(&ai));
                }
            });
        }
    }

    /// Invoke a callback with the first dock update interface found.
    pub fn with_dock_update_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(&mut dyn DockUpdateInterface) -> R,
    {
        let mut func = func;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_dock_update_kind(module).map(|kind| func(kind.into_dock_interface()))
            });

            if result.is_some() {
                return result;
            }
        }
        None
    }

    /// Invoke a callback with the first railed transport dock update interface found.
    pub fn with_railed_transport_dock_update_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(&mut dyn RailedTransportDockUpdateInterface) -> R,
    {
        let mut func = func;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_dock_update_kind(module)
                    .and_then(DockUpdateModuleKindMut::into_railed_transport_interface)
                    .map(&mut func)
            });

            if result.is_some() {
                return result;
            }
        }
        None
    }

    /// Invoke a callback with the first horde update interface found.
    pub fn with_horde_update_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(&mut dyn crate::modules::HordeUpdateInterface) -> R,
    {
        let mut func = func;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_horde_interface)
                    .map(&mut func)
            });

            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub fn with_overcharge_behavior_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(
            &mut dyn crate::object::behavior::behavior_module::OverchargeBehaviorInterface,
        ) -> R,
    {
        let mut func = func;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_overcharge_interface)
                    .map(&mut func)
            });

            if result.is_some() {
                return result;
            }
        }

        for behavior in &self.behaviors {
            let result = {
                let Ok(mut guard) = behavior.lock() else {
                    continue;
                };
                guard.get_overcharge_behavior_interface().map(&mut func)
            };
            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub fn with_power_plant_update_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(&mut dyn PowerPlantUpdateInterface) -> R,
    {
        let mut func = func;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_power_plant_update_interface)
                    .map(&mut func)
            });
            if result.is_some() {
                return result;
            }
        }

        for behavior in &self.behaviors {
            let result = {
                let Ok(mut guard) = behavior.lock() else {
                    continue;
                };
                guard.get_power_plant_update_interface().map(&mut func)
            };
            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub fn with_radar_update_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(&mut dyn game_engine::common::thing::module::RadarUpdateInterface) -> R,
    {
        let mut func = func;
        for entry in &self.modules {
            let result =
                entry.with_module(|module| module.get_radar_update_interface().map(&mut func));
            if result.is_some() {
                return result;
            }
        }
        None
    }

    /// Invoke a callback with the first exit interface found.
    pub fn with_object_exit_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn ExitInterface) -> R,
    {
        let exit_interface = self.get_object_exit_interface()?;
        let Ok(mut guard) = exit_interface.lock() else {
            return None;
        };
        Some(func(&mut *guard))
    }

    /// Find an update module by name.
    /// Matches C++ Object::FindUpdateModule but routed through module entries.
    pub fn find_update_module(&self, module_name: &str) -> Option<BehaviorModuleHandle> {
        let name = AsciiString::from(module_name);
        self.modules
            .iter()
            .find(|entry| {
                entry.name() == &name && (entry.mask().0 & ModuleInterfaceType::UPDATE.0) != 0
            })
            .cloned()
            .map(BehaviorModuleHandle::new)
    }

    /// Install an update module after construction (tests / internal harnesses).
    #[cfg(any(test, feature = "internal"))]
    pub fn install_update_module(
        &mut self,
        name: &str,
        module: Box<dyn Module>,
        module_data: Arc<dyn ModuleData>,
    ) {
        let entry = Arc::new(ModuleEntry::new(
            AsciiString::from(name),
            AsciiString::new(),
            ModuleInterfaceType::UPDATE,
            module_data,
            module,
        ));
        self.modules.push(Arc::clone(&entry));
        self.update_module_handles.push(entry);
        self.rebuild_behavior_list();
    }

    /// Install a module with an explicit interface mask (tests / internal harnesses).
    #[cfg(any(test, feature = "internal"))]
    pub fn install_module_for_test(
        &mut self,
        name: &str,
        module: Box<dyn Module>,
        module_data: Arc<dyn ModuleData>,
        mask: ModuleInterfaceType,
    ) {
        let entry = Arc::new(ModuleEntry::new(
            AsciiString::from(name),
            AsciiString::new(),
            mask,
            module_data,
            module,
        ));
        if (mask.0 & ModuleInterfaceType::DESTROY.0) != 0 {
            self.die_module_handles.push(Arc::clone(&entry));
        }
        if (mask.0 & ModuleInterfaceType::DAMAGE.0) != 0 {
            // Damage walks get_behavior_modules(); keep the entry on `modules`.
        }
        self.modules.push(entry);
        self.rebuild_behavior_list();
    }

    /// Find a legacy behavior module by name (behavior list only).
    pub fn find_update_behavior(
        &self,
        module_name: &str,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        self.behaviors.iter().find_map(|module| {
            let Ok(guard) = module.lock() else {
                return None;
            };
            if guard.get_module_name() == module_name {
                Some(Arc::clone(module))
            } else {
                None
            }
        })
    }

    /// Find a module by its NameKeyType (matches C++ Object::findModule).
    /// This is the primary module lookup used for inter-module communication.
    ///
    /// # Arguments
    /// * `key` - The NameKeyType generated from the module class name
    ///
    /// # Returns
    /// The matching module entry if found, or None
    ///
    /// # C++ Reference
    /// Object.cpp:2847 - Object::findModule(NameKeyType key)
    pub fn find_module_by_name_key(&self, key: NameKeyType) -> Option<Arc<ModuleEntry>> {
        // First search behavior modules (matching C++ order)
        for behavior_arc in &self.behaviors {
            let Ok(guard) = behavior_arc.lock() else {
                continue;
            };
            // Check if this module has a matching name key via the Module trait
            if guard.get_module_name_key() == key {
                // Return a synthetic ModuleEntry for the behavior
                drop(guard);
                // We need to convert the behavior back to a module entry
                // For now, search the modules list since behaviors is separate
                break;
            }
        }

        // Search through module entries by name key
        for entry in &self.modules {
            if entry.module_name_key() == key {
                return Some(Arc::clone(entry));
            }
        }

        None
    }

    /// Find a module by its module tag name key.
    /// Module tags are unique identifiers assigned per-object instance.
    ///
    /// # Arguments
    /// * `tag_key` - The NameKeyType of the module tag
    ///
    /// # Returns
    /// The matching module entry if found, or None
    pub fn find_module_by_tag_key(&self, tag_key: NameKeyType) -> Option<Arc<ModuleEntry>> {
        for entry in &self.modules {
            if entry.module_tag_key() == tag_key {
                return Some(Arc::clone(entry));
            }
        }
        None
    }

    /// Find a module by name string (convenience wrapper around find_module_by_name_key).
    ///
    /// # Arguments
    /// * `module_name` - The module class name (e.g., "ToppleUpdate")
    ///
    /// # Returns
    /// The matching module entry if found, or None
    pub fn find_module_by_name(&self, module_name: &str) -> Option<Arc<ModuleEntry>> {
        let key = crate::common::name_key_generate(module_name);
        self.find_module_by_name_key(key)
    }

    pub fn with_update_behavior_downcast<T: 'static, F, R>(
        &self,
        module_name: &str,
        func: F,
    ) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        let behavior = self.find_update_behavior(module_name)?;
        let mut guard = behavior.lock().ok()?;
        behavior_with_downcast::<T, _, _>(&mut *guard, func)
    }

    pub fn get_behavior_modules(&self) -> Vec<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        self.behaviors.iter().cloned().collect()
    }

    pub fn status_damage_helper(
        &self,
    ) -> Option<Arc<Mutex<crate::object::helper::StatusDamageHelper>>> {
        self.status_damage_helper.clone()
    }

    pub fn has_ctor_helpers(&self) -> bool {
        self.smc_helper.is_some()
            && self.status_damage_helper.is_some()
            && self.subdual_damage_helper.is_some()
    }

    /// C++ Object.cpp:299-384 — helpers first on `m_behaviors`, then template modules.
    pub(super) fn install_ctor_helpers(&mut self) {
        // Object.cpp:301-305 — always ObjectSMCHelper / ModuleTag_SMCHelper.
        if self.smc_helper.is_none() {
            self.smc_helper = Some(Arc::new(Mutex::new(
                crate::object::helper::ObjectSMCHelper::new(
                    crate::object::helper::ObjectSMCHelperModuleData::new(),
                ),
            )));
        }

        // Object.cpp:307-335 — InactiveBody cannot take special damage.
        let inactive_body = self
            .thing_template
            .as_ref()
            .get_behavior_module_info()
            .iter()
            .any(|entry| entry.name.as_str().eq_ignore_ascii_case("InactiveBody"));
        if !inactive_body {
            if self.status_damage_helper.is_none() {
                self.status_damage_helper = Some(Arc::new(Mutex::new(
                    crate::object::helper::StatusDamageHelper::new(
                        self.id,
                        crate::object::helper::StatusDamageHelperModuleData::new(),
                    ),
                )));
            }
            if self.subdual_damage_helper.is_none() {
                self.subdual_damage_helper = Some(Arc::new(Mutex::new(
                    crate::object::helper::SubdualDamageHelper::new(
                        self.id,
                        crate::object::helper::SubdualDamageHelperModuleData::new(),
                    ),
                )));
            }
        }

        // Object.cpp:337-347 — TheAI && enableRepulsors && KINDOF_CAN_BE_REPULSED.
        if self.repulsor_helper.is_none()
            && template_wants_repulsor_helper(self.thing_template.as_ref())
        {
            self.repulsor_helper = Some(Arc::new(Mutex::new(
                crate::object::helper::ObjectRepulsorHelper::new(
                    crate::object::helper::ObjectRepulsorHelperModuleData::new(),
                ),
            )));
        }

        // Object.cpp:354-362 — shrubbery cannot defect.
        if self.defection_helper.is_none() && !self.thing_template.is_kind_of(KindOf::Shrubbery) {
            self.defection_helper = Some(Arc::new(Mutex::new(
                crate::object::helper::ObjectDefectionHelper::new(
                    crate::object::helper::ObjectDefectionHelperModuleData::new(),
                ),
            )));
        }

        // Object.cpp:364-384 — weapon helpers only if the template can have a weapon.
        if template_can_possibly_have_any_weapon(self.thing_template.as_ref()) {
            if self.ws_helper.is_none() {
                self.ws_helper = Some(Arc::new(Mutex::new(
                    crate::object::helper::ObjectWeaponStatusHelper::new(
                        crate::object::helper::ObjectWeaponStatusHelperModuleData::new(),
                        true,
                    ),
                )));
            }
            if self.firing_tracker.is_none() {
                self.firing_tracker = Some(Arc::new(Mutex::new(FiringTracker::new(self.id))));
            }
            if self.temp_weapon_bonus_helper.is_none() {
                self.temp_weapon_bonus_helper = Some(Arc::new(Mutex::new(
                    crate::object::helper::TempWeaponBonusHelper::new(
                        self.id,
                        crate::object::helper::TempWeaponBonusHelperModuleData::new(),
                    ),
                )));
            }
        }

        self.rebuild_behavior_list();
    }

    /// C++ Object.cpp:458-462 — call onObjectCreated in m_behaviors list order
    /// after helpers + template modules are all installed.
    pub(super) fn invoke_on_object_created_after_install(&mut self) {
        #[cfg(test)]
        {
            LAST_ON_CREATED_SIBLING_COUNT
                .store(self.behaviors.len(), std::sync::atomic::Ordering::Relaxed);
        }
        for entry in &self.modules {
            entry.with_module(|module| module.on_object_created());
        }
    }

    /// `get_behavior_modules()` == C++ Object.cpp:299-384 helper order, then template modules.
    pub(super) fn rebuild_behavior_list(&mut self) {
        let mut behaviors: Vec<Arc<Mutex<dyn BehaviorModuleInterface>>> = Vec::new();
        if self.smc_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "ObjectSMCHelper",
            })));
        }
        if self.status_damage_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "StatusDamageHelper",
            })));
        }
        if self.subdual_damage_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "SubdualDamageHelper",
            })));
        }
        if self.repulsor_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "ObjectRepulsorHelper",
            })));
        }
        if self.defection_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "ObjectDefectionHelper",
            })));
        }
        if self.ws_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "ObjectWeaponStatusHelper",
            })));
        }
        if self.firing_tracker.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "FiringTracker",
            })));
        }
        if self.temp_weapon_bonus_helper.is_some() {
            behaviors.push(Arc::new(Mutex::new(CtorHelperBehavior {
                name: "TempWeaponBonusHelper",
            })));
        }
        for entry in &self.modules {
            behaviors.push(Arc::new(Mutex::new(TemplateModuleBehavior {
                name: entry.name().to_string(),
                entry: Arc::clone(entry),
            })));
        }
        self.behaviors = behaviors;
    }

    /// Tags written for ctor helpers in C++ Object::xfer v9 module-list order.
    pub(crate) fn ctor_helper_xfer_tags(&self) -> Vec<&'static str> {
        let mut tags = Vec::new();
        if self.smc_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_SMC);
        }
        if self.status_damage_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_STATUS);
        }
        if self.subdual_damage_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_SUBDUAL);
        }
        if self.repulsor_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_REPULSOR);
        }
        if self.defection_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_DEFECTION);
        }
        if self.ws_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_WEAPON_STATUS);
        }
        if self.firing_tracker.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_FIRING_TRACKER);
        }
        if self.temp_weapon_bonus_helper.is_some() {
            tags.push(super::object_xfer::HELPER_TAG_TEMP_WEAPON_BONUS);
        }
        tags
    }

    /// Borrow-first flammable module lookup (no outer Object Arc required).
    pub fn find_flammable_update_module(&self) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for module in self.get_behavior_modules() {
            let would_ignite = {
                let Ok(module_guard) = module.try_lock() else {
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

    pub fn with_spawn_behavior_full_interface<R, F>(&self, f: F) -> Option<R>
    where
        F: FnMut(&mut dyn crate::object::behavior::spawn_behavior::SpawnBehaviorInterface) -> R,
    {
        let mut f = f;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_spawn_interface)
                    .map(&mut f)
            });

            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub fn with_slaved_update_interface<R, F>(&self, f: F) -> Option<R>
    where
        F: FnMut(&mut dyn SlavedUpdateInterface) -> R,
    {
        let mut f = f;
        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_behavior_utility_kind(module)
                    .and_then(BehaviorUtilityModuleKindMut::into_slaved_update_interface)
                    .map(&mut f)
            });

            if result.is_some() {
                return result;
            }
        }

        None
    }

    pub fn get_object_exit_interface(&self) -> Option<Arc<Mutex<dyn ExitInterface>>> {
        for entry in &self.modules {
            let has_exit = entry.with_module(|module| {
                module_production_behavior_kind(module)
                    .map(|kind| kind.is_exit_capable())
                    .unwrap_or(false)
            });
            if has_exit {
                return Some(Arc::new(Mutex::new(ModuleExitInterfaceProxy {
                    entry: Arc::clone(entry),
                })));
            }
        }

        for behavior in &self.behaviors {
            let has_exit = {
                let Ok(mut guard) = behavior.lock() else {
                    continue;
                };
                guard.get_update_exit_interface().is_some()
            };

            if has_exit {
                return Some(Arc::new(Mutex::new(ExitInterfaceProxy {
                    behavior: behavior.clone(),
                })));
            }
        }

        if let Some(contain) = &self.contain {
            return Some(Arc::new(Mutex::new(ContainExitInterfaceProxy {
                contain: Arc::clone(contain),
            })));
        }

        None
    }

    pub fn get_physics(&self) -> Option<Arc<Mutex<dyn PhysicsBehavior>>> {
        self.physics.clone()
    }

    pub fn set_physics(&mut self, physics: Option<Arc<Mutex<dyn PhysicsBehavior>>>) {
        self.physics = physics;
    }

    /// Get mutable access to physics behavior
    /// Returns Arc<Mutex<>> to allow interior mutability through locking
    pub fn get_physics_mut(&mut self) -> Option<Arc<Mutex<dyn PhysicsBehavior>>> {
        self.physics.clone()
    }

    /// Iterate over modules that advertise a given interface mask.
    #[must_use]
    pub fn modules_with_interface(
        &self,
        interface: ModuleInterfaceType,
    ) -> Vec<BehaviorModuleHandle> {
        self.modules
            .iter()
            .filter(|entry| (entry.mask().0 & interface.0) != 0)
            .map(|entry| BehaviorModuleHandle::new(Arc::clone(entry)))
            .collect()
    }

    /// Retrieve all registered behavior modules.
    pub fn behavior_modules(&self) -> Vec<BehaviorModuleHandle> {
        self.modules
            .iter()
            .cloned()
            .map(BehaviorModuleHandle::new)
            .collect()
    }

    /// Retrieve a module by its logical name.
    pub fn module_by_name(&self, name: &AsciiString) -> Option<BehaviorModuleHandle> {
        self.modules
            .iter()
            .find(|entry| entry.name() == name)
            .cloned()
            .map(BehaviorModuleHandle::new)
    }

    /// Retrieve a module by its tag identifier.
    pub fn module_by_tag(&self, tag: &AsciiString) -> Option<BehaviorModuleHandle> {
        self.modules
            .iter()
            .find(|entry| entry.tag() == tag)
            .cloned()
            .map(BehaviorModuleHandle::new)
    }

    /// Retrieve draw modules currently attached to the object.
    pub fn drawable_modules(&self) -> Vec<DrawableModuleHandle> {
        self.drawable
            .as_ref()
            .and_then(|drawable| drawable.read().ok().map(|guard| guard.modules()))
            .unwrap_or_default()
    }

    /// Retrieve draw modules that advertise the supplied interface flags.
    pub fn drawable_modules_with_interface(
        &self,
        interface: ModuleInterfaceType,
    ) -> Vec<DrawableModuleHandle> {
        self.drawable
            .as_ref()
            .and_then(|drawable| {
                drawable
                    .read()
                    .ok()
                    .map(|guard| guard.modules_with_interface(interface))
            })
            .unwrap_or_default()
    }

    /// Retrieve drawable/client-update modules that advertise the CLIENT_UPDATE interface.
    pub fn client_update_modules(&self) -> Vec<DrawableModuleHandle> {
        self.drawable_modules_with_interface(ModuleInterfaceType::CLIENT_UPDATE)
    }

    // Private helper methods
    pub(super) fn init_modules_for(
        object: &Arc<RwLock<Self>>,
        thing_template: &dyn ThingTemplate,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Err(err) = crate::contain_module_overrides::ensure_module_overrides_installed() {
            warn!(
                "Failed to install module overrides before module init: {}",
                err
            );
        }

        // Register the template's descriptors with the global factory (if initialised).
        let _ = thing_template.module_descriptors();

        let thing_handle: Arc<ObjectThingHandle> = Arc::new(ObjectThingHandle::new(object));
        let module_handle: Arc<dyn ModuleThing> = thing_handle.clone();
        let mut modules_to_install: Vec<Arc<ModuleEntry>> = Vec::new();

        let mut install_behavior_modules = |factory: &ModuleFactory| {
            for entry in thing_template.get_behavior_module_info().iter() {
                let module_name = entry.name.clone();
                let module_data = Arc::clone(&entry.data);
                let module_data_for_entry = Arc::clone(&module_data);
                let interface_mask = entry.interface_flags();

                match factory.new_module(
                    module_handle.clone(),
                    &module_name,
                    module_data,
                    ModuleType::Behavior,
                ) {
                    Ok(module) => {
                        // C++ Object.cpp:458-462 — onObjectCreated runs only after
                        // every helper + template module is installed.
                        modules_to_install.push(Arc::new(ModuleEntry::new(
                            module_name,
                            entry.module_tag.clone(),
                            interface_mask,
                            module_data_for_entry,
                            module,
                        )));
                    }
                    Err(err) => {
                        let object_id = object
                            .read()
                            .ok()
                            .map(|guard| guard.id)
                            .unwrap_or(INVALID_ID);
                        warn!(
                            "Failed to instantiate behavior module '{}' for object {}: {}",
                            module_name, object_id, err
                        );
                    }
                }
            }
        };

        let mut installed = false;
        match get_module_factory() {
            Ok(factory_guard) => {
                if let Some(factory) = factory_guard.as_ref() {
                    install_behavior_modules(factory);
                    installed = true;
                }
            }
            Err(_) => warn!("Failed to lock ModuleFactory when creating modules"),
        }

        if !installed {
            if init_module_factory().is_ok() {
                match get_module_factory() {
                    Ok(factory_guard) => {
                        if let Some(factory) = factory_guard.as_ref() {
                            install_behavior_modules(factory);
                        } else {
                            warn!(
                                "ModuleFactory still not initialised after retry while creating modules"
                            );
                        }
                    }
                    Err(_) => {
                        warn!("Failed to lock ModuleFactory after retry while creating modules")
                    }
                }
            } else {
                warn!("ModuleFactory initialisation failed while creating modules");
            }
        }

        {
            let mut guard = object
                .write()
                .map_err(|_| "object lock poisoned during module installation")?;

            guard.modules.extend(modules_to_install.into_iter());
            guard.body_module_handles.clear();
            guard.die_module_handles.clear();
            guard.update_module_handles.clear();
            guard.collide_module_handles.clear();
            guard.contain_module_handles.clear();
            guard.upgrade_module_handles.clear();

            let module_entries: Vec<Arc<ModuleEntry>> = guard.modules.iter().cloned().collect();
            for entry in &module_entries {
                let mask = entry.mask();
                if (mask.0 & ModuleInterfaceType::BODY.0) != 0 {
                    guard.body_module_handles.push(Arc::clone(entry));
                }
                if (mask.0 & ModuleInterfaceType::DIE.0) != 0 {
                    guard.die_module_handles.push(Arc::clone(entry));
                }
                if (mask.0 & ModuleInterfaceType::UPDATE.0) != 0
                    && (mask.0 & ModuleInterfaceType::CONTAIN.0) == 0
                {
                    guard.update_module_handles.push(Arc::clone(entry));
                }
                if (mask.0 & ModuleInterfaceType::COLLIDE.0) != 0 {
                    guard.collide_module_handles.push(Arc::clone(entry));
                }
                if (mask.0 & ModuleInterfaceType::CONTAIN.0) != 0 {
                    guard.contain_module_handles.push(Arc::clone(entry));
                }
                if (mask.0 & ModuleInterfaceType::UPGRADE.0) != 0 {
                    guard.upgrade_module_handles.push(Arc::clone(entry));
                }
            }

            #[cfg(feature = "allow_surrender")]
            if guard.contain.is_none() {
                for entry in &guard.contain_module_handles {
                    let contain_handle = entry.with_module(|module| {
                        module
                            .as_any()
                            .downcast_ref::<crate::object::behavior::pow_truck_behavior::POWTruckBehaviorModule>()
                            .map(|module| module.contain_handle())
                            .or_else(|| {
                                module
                                    .as_any()
                                    .downcast_ref::<crate::object::behavior::prison_behavior::PrisonBehaviorModule>()
                                    .map(|module| module.contain_handle())
                            })
                            .or_else(|| {
                                module
                                    .as_any()
                                    .downcast_ref::<crate::object::behavior::propaganda_center_behavior::PropagandaCenterBehaviorModule>()
                                    .map(|module| module.contain_handle())
                            })
                    });
                    if let Some(handle) = contain_handle {
                        guard.set_contain(Some(handle));
                        break;
                    }
                }
            }

            guard.experience_tracker = Some(Arc::new(Mutex::new(ExperienceTracker::new(guard.id))));
            // C++ Object.cpp:454-456 — starting rank from Player::getProductionVeterancyLevel.
            let production_level = {
                let template_name = guard.get_template_name().to_string();
                guard
                    .get_controlling_player()
                    .and_then(|player| {
                        player.read().ok().map(|player_guard| {
                            player_guard.get_production_veterancy_level(&template_name)
                        })
                    })
                    .unwrap_or(crate::common::types::VeterancyLevel::Regular)
            };
            if let Some(tracker) = guard.experience_tracker.clone() {
                if let Ok(mut tracker_guard) = tracker.lock() {
                    if let Some(old_level) = tracker_guard.set_veterancy_level(production_level) {
                        let new_level = tracker_guard.get_veterancy_level();
                        drop(tracker_guard);
                        guard.on_veterancy_level_changed(old_level, new_level, true);
                    }
                }
            }

            let object_id = guard.id;
            guard.update_module_registrations.clear();
            let update_handles: Vec<Arc<ModuleEntry>> =
                guard.update_module_handles.iter().cloned().collect();
            for entry in &update_handles {
                let proxy: UpdateModulePtr = Arc::new(RwLock::new(ModuleUpdateProxy::new(
                    Arc::clone(entry),
                    object_id,
                )));
                entry.with_module(|module| {
                    if let Some(slow_death) = (module as &mut dyn Any).downcast_mut::<
                        crate::object::behavior::slow_death_behavior::SlowDeathBehavior,
                    >() {
                        slow_death.bind_update_proxy(proxy.clone());
                    }
                });
                let wake_frame = initial_update_wake_frame(entry.as_ref());
                if let Err(err) = crate::helpers::TheGameLogic::register_update_module(
                    object_id,
                    proxy.clone(),
                    wake_frame,
                ) {
                    warn!(
                        "Failed to register update module '{}' for object {}: {}",
                        entry.name(),
                        object_id,
                        err
                    );
                }
                guard.update_module_registrations.push(proxy);
            }

            // Helpers first on m_behaviors, then template modules (Object.cpp:299-437).
            guard.install_ctor_helpers();
            // C++ Object.cpp:458-471 — inter-module resolution after the full list exists.
            guard.invoke_on_object_created_after_install();
            guard.modules_ready = true;
        }

        // C++ parity: after AI module construction, seed attitude from team prototype.
        if let Ok(obj_guard) = object.read() {
            obj_guard.apply_team_ai_profile();
        }

        // Apply battle plan bonuses after modules are ready (C++ Object::onObjectCreated parity).
        if let Ok(obj_guard) = object.read() {
            if let Some(player_arc) = obj_guard.get_controlling_player() {
                if let Ok(player_guard) = player_arc.read() {
                    if player_guard.get_num_battle_plans_active() > 0 {
                        drop(player_guard);
                        drop(obj_guard);
                        if let (Ok(player_guard), Ok(mut obj_guard)) =
                            (player_arc.read(), object.write())
                        {
                            player_guard.apply_battle_plan_bonuses_for_object(&mut obj_guard);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get garrison contain module data
    /// C++ Reference: Object.cpp - Garrison contain module accessor
    pub fn get_garrison_contain_module_data(
        &self,
    ) -> Result<Arc<crate::object::contain::garrison_contain::GarrisonContainModuleData>, String>
    {
        for entry in &self.contain_module_handles {
            if let Some(ContainModuleDataKind::Garrison(data)) =
                ContainModuleDataKind::from_module_data(entry.module_data.as_ref())
            {
                return Ok(Arc::new(data.clone()));
            }
        }

        Err("GarrisonContainModuleData not found".to_string())
    }

    /// Get transport contain module data
    /// C++ Reference: Object.cpp - Transport contain module accessor
    pub fn get_transport_contain_module_data(
        &self,
    ) -> Result<crate::object::contain::transport_contain::TransportContainModuleData, String> {
        for entry in &self.contain_module_handles {
            if let Some(ContainModuleDataKind::Transport(data)) =
                ContainModuleDataKind::from_module_data(entry.module_data.as_ref())
            {
                return Ok(data.clone());
            }
        }

        Err("TransportContainModuleData not found".to_string())
    }

    /// Invoke a callback with the parking place behavior module if this object has one.
    /// C++ Reference: Object.cpp - Parking place behavior accessor
    pub fn with_parking_place_behavior<F, R>(&self, func: F) -> Option<R>
    where
        F: FnMut(
            &mut dyn crate::object::behavior::behavior_module::ParkingPlaceBehaviorInterface,
        ) -> R,
    {
        let mut func = func;
        for behavior in &self.behaviors {
            if let Ok(mut guard) = behavior.lock() {
                if let Some(parking) = guard.get_parking_place_behavior_interface() {
                    return Some(func(parking));
                }
            }
        }

        for entry in &self.modules {
            let result = entry.with_module(|module| {
                module_production_behavior_kind(module)
                    .and_then(ProductionBehaviorModuleKindMut::into_parking_place_interface)
                    .map(|parking| func(parking))
            });
            if result.is_some() {
                return result;
            }
        }

        None
    }

    // ========================================================================
    // MODULE INTERFACE ACCESSORS (5 methods)
    // C++ Reference: Object.cpp getProjectileUpdateInterface, etc.
    // ========================================================================

    pub fn get_projectile_update_interface(
        &self,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if guard.get_projectile_update_interface().is_some() {
                drop(guard);
                return Some(behavior.clone());
            }
        }
        None
    }

    pub fn get_spawn_behavior_interface_public(
        &self,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if guard.get_spawn_behavior_interface().is_some() {
                drop(guard);
                return Some(behavior.clone());
            }
        }
        None
    }

    pub fn get_production_update_interface(
        &self,
    ) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if guard.get_production_update_interface().is_some() {
                drop(guard);
                return Some(behavior.clone());
            }
        }
        None
    }

    pub fn get_dock_update_interface(&self) -> Option<Arc<Mutex<dyn BehaviorModuleInterface>>> {
        for behavior in &self.behaviors {
            let Ok(mut guard) = behavior.lock() else {
                continue;
            };
            if guard.get_dock_update_interface().is_some() {
                drop(guard);
                return Some(behavior.clone());
            }
        }
        None
    }

    pub fn force_refresh_sub_object_upgrade_status(&mut self) {
        for entry in &self.upgrade_module_handles {
            entry.with_module(|module| {
                if let Some(UpgradeModuleKindMut::SubObjects(sub_obj)) = module_upgrade_kind(module)
                {
                    sub_obj.force_refresh_upgrade();
                }
            });
        }
        for handle in SubObjectsUpgradeHandle::for_object(self.id) {
            handle.force_refresh();
        }
    }
}

#[cfg(test)]
static LAST_ON_CREATED_SIBLING_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

impl Object {
    #[cfg(test)]
    pub(crate) fn last_on_created_sibling_count() -> usize {
        LAST_ON_CREATED_SIBLING_COUNT.load(std::sync::atomic::Ordering::Relaxed)
    }
}

fn template_wants_repulsor_helper(template: &dyn ThingTemplate) -> bool {
    if !template.is_kind_of(KindOf::CanBeRepulsed) {
        return false;
    }
    crate::ai::THE_AI
        .read()
        .ok()
        .and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|data| data.enable_repulsors)
        })
        .unwrap_or(false)
}

fn template_can_possibly_have_any_weapon(template: &dyn ThingTemplate) -> bool {
    template
        .weapon_template_sets()
        .iter()
        .any(|set| set.has_any_weapons())
}

/// C++ helper modules live on `m_behaviors` so destroy/damage/xfer walk them.
struct CtorHelperBehavior {
    name: &'static str,
}

impl BehaviorModuleInterface for CtorHelperBehavior {
    fn get_module_name(&self) -> &str {
        self.name
    }
}

/// Template `ModuleEntry` listed after helpers on `get_behavior_modules()`.
struct TemplateModuleBehavior {
    name: String,
    entry: Arc<ModuleEntry>,
}

impl BehaviorModuleInterface for TemplateModuleBehavior {
    fn get_module_name(&self) -> &str {
        &self.name
    }

    fn get_destroy(&mut self) -> Option<&mut dyn crate::modules::DestroyModuleInterface> {
        if (self.entry.mask().0 & ModuleInterfaceType::DESTROY.0) != 0 {
            Some(self)
        } else {
            None
        }
    }

    fn get_damage(&mut self) -> Option<&mut dyn crate::modules::DamageModuleInterface> {
        if (self.entry.mask().0 & ModuleInterfaceType::DAMAGE.0) != 0 {
            Some(self)
        } else {
            None
        }
    }
}

impl crate::modules::DestroyModuleInterface for TemplateModuleBehavior {
    fn on_destroy(&mut self, object_id: ObjectID) {
        let _ = object_id;
        self.entry.with_module(|module| module.on_delete());
    }
}

impl crate::modules::DamageModuleInterface for TemplateModuleBehavior {
    fn receive_damage(&mut self, object_id: ObjectID, damage: &DamageInfo) -> Real {
        let _ = (object_id, damage);
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.entry.with_module(|module| {
            if let Some(auto_heal) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule,
            >() {
                return auto_heal.behavior_mut().on_damage(damage_info);
            }
            if let Some(bridge) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>(
            ) {
                return bridge.behavior_mut().on_damage(damage_info);
            }
            if let Some(tower) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule,
            >() {
                return tower.behavior_mut().on_damage(damage_info);
            }
            Ok(())
        })
    }

    fn on_healing(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.entry.with_module(|module| {
            if let Some(bridge) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>(
            ) {
                return bridge.behavior_mut().on_healing(damage_info);
            }
            if let Some(tower) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule,
            >() {
                return tower.behavior_mut().on_healing(damage_info);
            }
            Ok(())
        })
    }

    fn on_body_damage_state_change(
        &mut self,
        damage_info: &DamageInfo,
        old_state: crate::damage::BodyDamageType,
        new_state: crate::damage::BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.entry.with_module(|module| {
            if let Some(bridge) = (module as &mut dyn Any)
                .downcast_mut::<crate::object::behavior::bridge_behavior::BridgeBehaviorModule>(
            ) {
                return bridge.behavior_mut().on_body_damage_state_change(
                    damage_info,
                    old_state,
                    new_state,
                );
            }
            if let Some(tower) = (module as &mut dyn Any).downcast_mut::<
                crate::object::behavior::bridge_tower_behavior::BridgeTowerBehaviorModule,
            >() {
                return tower.behavior_mut().on_body_damage_state_change(
                    damage_info,
                    old_state,
                    new_state,
                );
            }
            Ok(())
        })
    }
}

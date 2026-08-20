// ============================================================================
// LOCOMOTOR SET
// ============================================================================

/// Locomotor set for managing multiple locomotors per unit
/// Matches C++ LocomotorSet.h
#[derive(Debug, Clone)]
pub struct LocomotorSet {
    locomotors: HashMap<String, Arc<Mutex<Locomotor>>>,
    locomotor_order: Vec<String>,
    active_locomotor: Option<String>,
    /// Bitmask of valid surfaces across all added locomotors
    /// Matches C++ LocomotorSet::m_validLocomotorSurfaces
    valid_surfaces: LocomotorSurfaceTypeMask,
    /// Whether this set only allows downhill movement
    /// Matches C++ LocomotorSet::m_downhillOnly
    downhill_only: bool,
}

impl LocomotorSet {
    pub fn new() -> Self {
        Self {
            locomotors: HashMap::new(),
            locomotor_order: Vec::new(),
            active_locomotor: None,
            valid_surfaces: 0,
            downhill_only: false,
        }
    }

    /// Clear all locomotors - matches C++ LocomotorSet::clear()
    pub fn clear(&mut self) {
        self.locomotors.clear();
        self.locomotor_order.clear();
        self.active_locomotor = None;
        self.valid_surfaces = 0;
        self.downhill_only = false;
    }

    /// Add a locomotor from a template - matches C++ LocomotorSet::addLocomotor()
    pub fn add_locomotor(&mut self, name: String, locomotor: Arc<Mutex<Locomotor>>) {
        // Accumulate valid surfaces - matches C++ addLocomotor
        if let Ok(loco) = locomotor.lock() {
            self.valid_surfaces |= loco.get_legal_surfaces();
            if loco.template.downhill_only {
                self.downhill_only = true;
            }
        }
        if self.active_locomotor.is_none() {
            self.active_locomotor = Some(name.clone());
        }
        if !self.locomotors.contains_key(&name) {
            self.locomotor_order.push(name.clone());
        }
        self.locomotors.insert(name, locomotor);
    }

    /// Find a locomotor that supports the given surface type mask
    /// Matches C++ LocomotorSet::findLocomotor(LocomotorSurfaceTypeMask t)
    pub fn find_locomotor(
        &self,
        surface_mask: LocomotorSurfaceTypeMask,
    ) -> Option<Arc<Mutex<Locomotor>>> {
        // C++ iterates m_locomotors and returns the first one whose template
        // surfaces overlap with the requested mask
        for name in &self.locomotor_order {
            if let Some(loco) = self.locomotors.get(name) {
                if let Ok(l) = loco.lock() {
                    if (l.get_legal_surfaces() & surface_mask) != 0 {
                        return Some(loco.clone());
                    }
                }
            }
        }
        None
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        if self.locomotors.contains_key(name) {
            self.active_locomotor = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn get_active(&self) -> Option<Arc<Mutex<Locomotor>>> {
        self.active_locomotor
            .as_ref()
            .and_then(|name| self.locomotors.get(name).cloned())
    }

    pub fn get_locomotor(&self, name: &str) -> Option<Arc<Mutex<Locomotor>>> {
        self.locomotors.get(name).cloned()
    }

    /// Get the valid surface mask across all locomotors
    /// Matches C++ LocomotorSet::getValidSurfaces()
    pub fn get_valid_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.valid_surfaces
    }

    /// Check if this set only allows downhill movement
    /// Matches C++ LocomotorSet::isDownhillOnly()
    pub fn is_downhill_only(&self) -> bool {
        self.downhill_only
    }

    /// Returns the currently active locomotor (or the first entry) matching the C++ default logic.
    pub fn get_default_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        if let Some(active) = self.get_active() {
            return Some(active);
        }
        self.locomotor_order
            .first()
            .and_then(|name| self.locomotors.get(name).cloned())
    }

    /// Get number of locomotors in set
    pub fn len(&self) -> usize {
        self.locomotors.len()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.locomotors.is_empty()
    }

    /// Iterate over all locomotors
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<Mutex<Locomotor>>)> {
        self.locomotors.iter()
    }

    /// Serialize this set plus the caller's current locomotor pointer.
    /// Matches C++ LocomotorSet::xferSelfAndCurLocoPtr.
    pub fn xfer_self_and_cur_loco_ptr(
        &mut self,
        xfer: &mut dyn game_engine::system::Xfer,
        current_locomotor: &mut Option<Arc<Mutex<Locomotor>>>,
    ) -> Result<(), String> {
        const CURRENT_VERSION: u8 = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("LocomotorSet xfer version: {:?}", e))?;

        let mut count: u16 = if xfer.is_loading() {
            0
        } else {
            self.locomotor_order.len() as u16
        };
        xfer.xfer_unsigned_short(&mut count)
            .map_err(|e| format!("LocomotorSet xfer count: {:?}", e))?;

        if xfer.is_loading() {
            if !self.is_empty() {
                return Err("LocomotorSet::xfer expected empty set on load".to_string());
            }
            for _ in 0..count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("LocomotorSet xfer template name: {:?}", e))?;
                let template = LOCOMOTOR_STORE
                    .get_template(&name)
                    .ok_or_else(|| format!("LocomotorSet xfer unknown template {name}"))?;
                let mut loco = Locomotor::new(template);
                loco.loco_xfer(xfer)?;
                self.add_locomotor(name, Arc::new(Mutex::new(loco)));
            }
        } else {
            for name in self.locomotor_order.clone() {
                let loco = self
                    .locomotors
                    .get(&name)
                    .ok_or_else(|| format!("LocomotorSet missing ordered locomotor {name}"))?;
                let mut xfer_name = name;
                xfer.xfer_ascii_string(&mut xfer_name)
                    .map_err(|e| format!("LocomotorSet xfer template name: {:?}", e))?;
                let mut guard = loco
                    .lock()
                    .map_err(|_| "LocomotorSet locomotor lock poisoned".to_string())?;
                guard.loco_xfer(xfer)?;
            }
        }

        let mut valid_surfaces = self.valid_surfaces as i32;
        xfer.xfer_int(&mut valid_surfaces)
            .map_err(|e| format!("LocomotorSet xfer valid surfaces: {:?}", e))?;
        if xfer.is_loading() {
            self.valid_surfaces = valid_surfaces as LocomotorSurfaceTypeMask;
        }
        xfer.xfer_bool(&mut self.downhill_only)
            .map_err(|e| format!("LocomotorSet xfer downhill only: {:?}", e))?;

        let mut current_name = if xfer.is_loading() {
            String::new()
        } else {
            current_locomotor
                .as_ref()
                .and_then(|loco| {
                    loco.lock()
                        .ok()
                        .map(|guard| guard.get_template_name().to_string())
                })
                .unwrap_or_default()
        };
        xfer.xfer_ascii_string(&mut current_name)
            .map_err(|e| format!("LocomotorSet xfer current locomotor: {:?}", e))?;

        if xfer.is_loading() {
            if current_name.is_empty() {
                *current_locomotor = None;
                self.active_locomotor = None;
            } else {
                let loco = self.get_locomotor(&current_name).ok_or_else(|| {
                    format!("LocomotorSet xfer current template {current_name} not found")
                })?;
                self.active_locomotor = Some(current_name);
                *current_locomotor = Some(loco);
            }
        }

        Ok(())
    }
}

// ============================================================================
// TERRAIN SPEED MULTIPLIERS
// ============================================================================

/// Terrain speed multiplier table
#[derive(Debug, Clone)]
pub struct TerrainSpeedTable {
    multipliers: HashMap<(LocomotorAppearance, u8), Real>,
}

impl TerrainSpeedTable {
    pub fn new() -> Self {
        let mut table = Self {
            multipliers: HashMap::new(),
        };
        table.init_default_multipliers();
        table
    }

    fn init_default_multipliers(&mut self) {
        use LocomotorAppearance::*;

        // Terrain types: 0=clear, 1=rough, 2=very_rough, 3=water, 4=cliff, 5=road

        // Infantry
        self.set(TwoLegs, 0, 1.0); // Clear
        self.set(TwoLegs, 1, 0.8); // Rough
        self.set(TwoLegs, 2, 0.6); // Very rough
        self.set(TwoLegs, 3, 0.0); // Water (can't cross)
        self.set(TwoLegs, 4, 0.4); // Cliff (slow climb)
        self.set(TwoLegs, 5, 1.0); // Road (no bonus)

        // Wheeled
        self.set(FourWheels, 0, 1.0);
        self.set(FourWheels, 1, 0.7);
        self.set(FourWheels, 2, 0.4);
        self.set(FourWheels, 3, 0.0);
        self.set(FourWheels, 4, 0.0); // Can't climb
        self.set(FourWheels, 5, 1.5); // Road bonus

        // Tracked
        self.set(Treads, 0, 1.0);
        self.set(Treads, 1, 0.9);
        self.set(Treads, 2, 0.7);
        self.set(Treads, 3, 0.0);
        self.set(Treads, 4, 0.0);
        self.set(Treads, 5, 1.2); // Slight road bonus

        // Hover
        self.set(Hover, 0, 1.0);
        self.set(Hover, 1, 1.0);
        self.set(Hover, 2, 1.0);
        self.set(Hover, 3, 1.0); // Can cross water
        self.set(Hover, 4, 0.7); // Slower over cliffs
        self.set(Hover, 5, 1.0);

        // Aircraft (ignore terrain)
        for terrain in 0..6 {
            self.set(Thrust, terrain, 1.0);
            self.set(Wings, terrain, 1.0);
        }

        // Climber
        self.set(Climber, 0, 1.0);
        self.set(Climber, 1, 0.8);
        self.set(Climber, 2, 0.7);
        self.set(Climber, 3, 0.0);
        self.set(Climber, 4, 0.8); // Can climb cliffs
        self.set(Climber, 5, 1.0);

        // Other (generic)
        for terrain in 0..6 {
            self.set(Other, terrain, 1.0);
        }
    }

    fn set(&mut self, appearance: LocomotorAppearance, terrain: u8, multiplier: Real) {
        self.multipliers.insert((appearance, terrain), multiplier);
    }

    pub fn get_multiplier(&self, appearance: LocomotorAppearance, terrain: u8) -> Real {
        *self.multipliers.get(&(appearance, terrain)).unwrap_or(&1.0)
    }
}

// ============================================================================
// LOCOMOTOR STORE (GLOBAL REGISTRY)
// ============================================================================

/// Global locomotor template store
pub struct LocomotorStore {
    templates: RwLock<HashMap<String, Arc<LocomotorTemplate>>>,
    terrain_speeds: TerrainSpeedTable,
}

impl LocomotorStore {
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(HashMap::new()),
            terrain_speeds: TerrainSpeedTable::new(),
        }
    }

    pub fn register_template(&self, template: LocomotorTemplate) {
        let name = template.name.clone();
        if let Ok(mut templates) = self.templates.write() {
            templates.insert(name, Arc::new(template));
        }
    }

    pub fn get_template(&self, name: &str) -> Option<Arc<LocomotorTemplate>> {
        if let Ok(templates) = self.templates.read() {
            if let Some(found) = templates.get(name).cloned() {
                return Some(found);
            }
        }
        let converted = crate::locomotor::ini_bridge::convert_named(name)?;
        self.register_template(converted);
        if let Ok(templates) = self.templates.read() {
            templates.get(name).cloned()
        } else {
            None
        }
    }

    pub fn create_locomotor(&self, template_name: &str) -> Option<Locomotor> {
        self.get_template(template_name)
            .map(|template| Locomotor::new(template))
    }

    pub fn get_terrain_multiplier(&self, appearance: LocomotorAppearance, terrain: u8) -> Real {
        self.terrain_speeds.get_multiplier(appearance, terrain)
    }

    /// Per-frame update - matches C++ LocomotorStore::update() (Locomotor.cpp:540)
    pub fn update(&self) {}
}

// Global instance
pub static LOCOMOTOR_STORE: Lazy<Arc<LocomotorStore>> = Lazy::new(|| {
    let store = Arc::new(LocomotorStore::new());

    // Fallback stubs used before / without INI. Retail names come from Common.
    store.register_template(LocomotorTemplate::new_infantry("Infantry".to_string()));
    store.register_template(LocomotorTemplate::new_wheeled("Wheeled".to_string()));
    store.register_template(LocomotorTemplate::new_tracked("Tracked".to_string()));
    store.register_template(LocomotorTemplate::new_hover("Hover".to_string()));
    store.register_template(LocomotorTemplate::new_thrust("Thrust".to_string()));
    store.register_template(LocomotorTemplate::new_wings("Wings".to_string()));
    store.register_template(LocomotorTemplate::new_climber("Climber".to_string()));

    crate::locomotor::ini_bridge::sync_common_store_into(&store);

    store
});


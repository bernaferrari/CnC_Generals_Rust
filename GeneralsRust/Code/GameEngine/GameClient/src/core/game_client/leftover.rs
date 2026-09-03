// Remaining GameClient drawable/xfer/snapshot/drop implementation.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

impl GameClient {
    fn should_save_drawable(drawable: &dyn Drawable) -> bool {
        if drawable.get_status().has(DrawableStatus::NO_SAVE) {
            if drawable.get_object_id().is_none() {
                return false;
            }
            log::warn!("Drawable marked NO_SAVE but bound to an object; keeping for parity.");
        }

        true
    }

    fn resolve_drawable_template_name(drawable: &dyn Drawable) -> Option<String> {
        // Prefer drawable residual template name first (host presentation path).
        if let Some(name) = drawable.get_template_name() {
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        // Wave 976: host empty dual-world → presentation translator catalog residual.
        if dual_world_registry_unavailable() {
            let object_id = drawable.get_object_id()?;
            return crate::presentation_translator_residual::translator_catalog_entry(object_id)
                .map(|e| {
                    // Wave 1051: apparent disguise template for non-allied viewers.
                    crate::presentation_translator_residual::translator_entry_apparent_template(&e)
                        .to_string()
                })
                .filter(|n| !n.is_empty());
        }

        let object_id = drawable.get_object_id()?;
        let object_arc = OBJECT_REGISTRY.get_object(object_id)?;
        let object_guard = object_arc.read().ok()?;
        let name = object_guard.get_template().get_name().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    fn collect_saveable_drawables_sorted(&self) -> Result<Vec<(DrawableId, String)>, String> {
        let mut entries = Vec::new();
        for (&id, drawable) in &self.drawable_map {
            if !Self::should_save_drawable(drawable.as_ref()) {
                continue;
            }

            let template_name = Self::resolve_drawable_template_name(drawable.as_ref())
                .ok_or_else(|| format!("Drawable '{}' missing template name for save", id.0))?;
            entries.push((id, template_name));
        }

        // HashMap traversal order is nondeterministic; save/load parity expects stable ordering.
        entries.sort_by_key(|(id, _)| id.0);
        Ok(entries)
    }

    fn add_toc_entry(&mut self, name: String, id: u16) {
        self.drawable_toc.push(DrawableTOCEntry { name, id });
    }

    fn find_toc_entry_by_name(&self, name: &str) -> Option<&DrawableTOCEntry> {
        self.drawable_toc.iter().find(|entry| entry.name == name)
    }

    fn find_toc_entry_by_id(&self, id: u16) -> Option<&DrawableTOCEntry> {
        self.drawable_toc.iter().find(|entry| entry.id == id)
    }

    fn xfer_drawable_toc(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.drawable_toc.clear();

        let mut toc_count: u32 = 0;
        if xfer.is_writing() {
            let save_entries = self.collect_saveable_drawables_sorted()?;
            let mut toc_names: Vec<String> = Vec::new();
            for (_, template_name) in save_entries {
                if toc_names.iter().any(|name| name == &template_name) {
                    continue;
                }
                toc_names.push(template_name);
            }

            for name in toc_names {
                toc_count = toc_count.saturating_add(1);
                self.add_toc_entry(name, toc_count as u16);
            }

            xfer.xfer_unsigned_int(&mut toc_count)
                .map_err(|e| e.to_string())?;

            for entry in &mut self.drawable_toc {
                let mut name = entry.name.clone();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                entry.name = name;

                let mut id = entry.id;
                xfer.xfer_unsigned_short(&mut id)
                    .map_err(|e| e.to_string())?;
                entry.id = id;
            }
        } else {
            xfer.xfer_unsigned_int(&mut toc_count)
                .map_err(|e| e.to_string())?;

            for _ in 0..toc_count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| e.to_string())?;
                let mut id: u16 = 0;
                xfer.xfer_unsigned_short(&mut id)
                    .map_err(|e| e.to_string())?;
                self.add_toc_entry(name, id);
            }
        }

        Ok(())
    }

    fn xfer_drawable_snapshot(
        drawable: &mut dyn Drawable,
        xfer: &mut dyn Xfer,
    ) -> Result<(), String> {
        drawable.xfer_snapshot(xfer)
    }

    fn drawable_matches_saved_template(
        drawable: &dyn Drawable,
        saved_template: &Arc<ThingTemplate>,
        factory: &game_engine::common::thing::ThingFactory,
    ) -> bool {
        let Some(existing_name) = Self::resolve_drawable_template_name(drawable) else {
            return false;
        };
        let Some(existing_template) = factory.find_template(&existing_name, false) else {
            return false;
        };

        let existing_final = ThingTemplate::get_final_override(&existing_template);
        let saved_final = ThingTemplate::get_final_override(saved_template);
        Arc::ptr_eq(&existing_final, &saved_final)
            || existing_final.get_name() == saved_final.get_name()
    }

    /// Retrieve the platform context (window + graphics + audio) for external event-loop driving.
    pub fn take_platform_context(&mut self) -> Option<PlatformContext> {
        self.subsystem_manager.platform_context.take()
    }

    /// Registers a drawable and assigns it a unique ID
    pub fn register_drawable(
        &mut self,
        drawable: Box<dyn Drawable>,
    ) -> GameClientResult<DrawableId> {
        self.register_drawable_with_template(drawable, None)
    }

    pub fn register_drawable_with_template(
        &mut self,
        mut drawable: Box<dyn Drawable>,
        template_name: Option<String>,
    ) -> GameClientResult<DrawableId> {
        // Wave 976: host empty dual-world still allows presentation/template registration.
        // Factory OBJECT_REGISTRY binds remain dual-world-only below.

        if let Some(name) = template_name {
            drawable.set_template_name(Some(name));
        } else if drawable.get_template_name().is_none() {
            if let Some(object_id) = drawable.get_object_id() {
                // Wave 1019: dual-world peels template name from translator catalog.
                if dual_world_registry_unavailable() {
                    if let Some(entry) =
                        crate::presentation_translator_residual::translator_catalog_entry(object_id)
                    {
                        // Wave 1051: apparent disguise template for non-allied viewers.
                        let apparent = crate::presentation_translator_residual::translator_entry_apparent_template(
                            &entry,
                        );
                        if !apparent.is_empty() {
                            drawable.set_template_name(Some(apparent.to_string()));
                        }
                    }
                } else if let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) {
                    if let Ok(object_guard) = object_arc.read() {
                        let fallback_name = object_guard.get_template().get_name().to_string();
                        if !fallback_name.is_empty() {
                            drawable.set_template_name(Some(fallback_name));
                        }
                    }
                }
            }
        }

        let id = self.alloc_drawable_id();
        drawable.set_id(id);

        let object_id = drawable.get_object_id();
        if let Some(object_id) = object_id {
            if let Some(previous_drawable_id) = self.drawable_object_map.get(&object_id).copied() {
                if previous_drawable_id != id {
                    if let Some(previous_drawable) =
                        self.drawable_map.get_mut(&previous_drawable_id)
                    {
                        previous_drawable.set_object_id(None);
                    }
                    self.drawable_object_map.remove(&object_id);
                    // A generic registration displaced a presentation-owned
                    // direct visual.  Its runtime key is no longer valid;
                    // host sync will mint a new one if this remains a direct
                    // presentation resident.
                    self.presentation_direct_drawable_bindings
                        .remove(&previous_drawable_id);
                }
            }
        }
        self.drawable_map.insert(id, drawable);
        if let Some(object_id) = object_id {
            self.drawable_object_map.insert(object_id, id);
        }

        log::debug!("Registered drawable with ID {:?}", id);
        Ok(id)
    }

    pub fn create_drawable_from_template(
        &mut self,
        template: &ThingTemplate,
    ) -> GameClientResult<DrawableId> {
        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        for module in Self::create_snapshot_modules_from_template(template) {
            drawable.add_draw_module(module);
        }
        let drawable: Box<dyn Drawable> = Box::new(drawable);
        self.register_drawable_with_template(drawable, Some(template.get_name().to_string()))
    }

    fn create_snapshot_modules_from_template(template: &ThingTemplate) -> Vec<Box<dyn DrawModule>> {
        let mut snapshot_modules: Vec<Box<dyn DrawModule>> = Vec::new();

        for entry in template.get_draw_module_info().iter() {
            let identifier = if entry.module_tag.is_empty() {
                entry.name.as_str()
            } else {
                entry.module_tag.as_str()
            };

            match entry.name.as_str() {
                "W3DTreeDraw" => {
                    if let Some(data) = entry.data.as_any().downcast_ref::<W3DTreeDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DTreeDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DModelDraw" => {
                    if let Some(data) = entry.data.as_any().downcast_ref::<W3DModelDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DModelDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DTankDraw" => {
                    if let Some(data) = entry.data.as_any().downcast_ref::<W3DTankDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DTankDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DTruckDraw" => {
                    if let Some(data) = entry.data.as_any().downcast_ref::<W3DTruckDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DTruckDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DTankTruckDraw" => {
                    if let Some(data) =
                        entry.data.as_any().downcast_ref::<W3DTankTruckDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DTankTruckDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DOverlordTankDraw" => {
                    if let Some(data) = entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DOverlordTankDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DOverlordTankDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DOverlordTruckDraw" => {
                    if let Some(data) = entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DOverlordTruckDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DOverlordTruckDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DOverlordAircraftDraw" => {
                    if let Some(data) = entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DOverlordAircraftDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DOverlordAircraftDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DLaserDraw" => {
                    if let Some(data) = entry.data.as_any().downcast_ref::<W3DLaserDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DLaserDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DPoliceCarDraw" => {
                    if let Some(data) = entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DPoliceCarDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DPoliceCarDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DScienceModelDraw" => {
                    if let Some(data) = entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DScienceModelDrawModuleData>()
                    {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::draw_module(
                                identifier.to_string(),
                                Box::new(W3DScienceModelDraw::new(data.clone())),
                            ),
                        ));
                    }
                }
                "W3DDebrisDraw" => {
                    let data = entry
                        .data
                        .as_any()
                        .downcast_ref::<W3DDebrisDrawModuleData>()
                        .cloned()
                        .unwrap_or_else(W3DDebrisDrawModuleData::new);
                    snapshot_modules.push(Box::new(LogicDrawModuleSnapshotAdapter::draw_module(
                        identifier.to_string(),
                        Box::new(W3DDebrisDraw::new(data)),
                    )));
                }
                _ => {}
            }
        }

        for entry in template.get_client_update_module_info().iter() {
            let identifier = if entry.module_tag.is_empty() {
                entry.name.as_str()
            } else {
                entry.module_tag.as_str()
            };
            let module_name_key = NameKeyGenerator::name_to_key(entry.name.as_str());

            match entry.name.as_str() {
                "BeaconClientUpdate" => {
                    if let Some(module) = BeaconClientUpdateModule::from_module_data(
                        module_name_key,
                        Arc::clone(entry.data),
                        INVALID_ID,
                    ) {
                        snapshot_modules.push(Box::new(
                            LogicDrawModuleSnapshotAdapter::client_update_module(
                                identifier.to_string(),
                                Box::new(module),
                            ),
                        ));
                    }
                }
                "SwayClientUpdate" => {
                    snapshot_modules.push(Box::new(
                        LogicDrawModuleSnapshotAdapter::client_update_module(
                            identifier.to_string(),
                            Box::new(SwayClientUpdateModule::new(
                                module_name_key,
                                Arc::clone(entry.data),
                                INVALID_ID,
                            )),
                        ),
                    ));
                }
                "AnimatedParticleSysBoneClientUpdate" => {
                    snapshot_modules.push(Box::new(
                        LogicDrawModuleSnapshotAdapter::client_update_module(
                            identifier.to_string(),
                            Box::new(AnimatedParticleSysBoneClientUpdateModule::new(
                                module_name_key,
                                Arc::clone(entry.data),
                                INVALID_ID,
                            )),
                        ),
                    ));
                }
                _ => {}
            }
        }

        snapshot_modules
    }

    /// Finds a drawable by its ID
    pub fn find_drawable_by_id(&self, id: DrawableId) -> Option<&dyn Drawable> {
        self.drawable_map.get(&id).map(|d| d.as_ref())
    }

    /// Finds a mutable drawable by its ID
    pub fn find_drawable_by_id_mut(&mut self, id: DrawableId) -> Option<&mut Box<dyn Drawable>> {
        self.drawable_map.get_mut(&id)
    }

    /// Destroys a drawable and removes it from all systems
    pub fn destroy_drawable(&mut self, id: DrawableId) -> GameClientResult<()> {
        if let Some(drawable) = self.drawable_map.get(&id) {
            // Notify UI systems
            if let Some(ref ui) = self.subsystem_manager.in_game_ui {
                ui.lock()
                    .map_err(|_| {
                        GameClientError::SubsystemError("In-game UI lock poisoned".to_string())
                    })?
                    .disregard_drawable(drawable.as_ref())?;
            }
        }

        // Remove from the map (this drops the drawable)
        if let Some(drawable) = self.drawable_map.remove(&id) {
            if let Some(object_id) = drawable.get_object_id() {
                if self.drawable_object_map.get(&object_id).copied() == Some(id) {
                    self.drawable_object_map.remove(&object_id);
                }
                prune_presentation_specialized_draw(object_id);
            }
        }
        // Direct visual identity is runtime-only and dies with the Drawable.
        self.presentation_direct_drawable_bindings.remove(&id);



        // Remove from text bearing list
        self.text_bearing_drawables
            .retain(|&stored_id| stored_id != id);

        Ok(())
    }

    pub fn bind_drawable_to_object(
        &mut self,
        drawable_id: DrawableId,
        object_id: ObjectID,
    ) -> GameClientResult<()> {
        let old_object_id = self
            .drawable_map
            .get(&drawable_id)
            .and_then(|drawable| drawable.get_object_id());

        // C++ `Drawable::friend_bindToObject` does not reset its volatile
        // shroud state when it is rebound to the same Object.  Retain the
        // direct binding only for that fully validated no-op association;
        // any owner change, displacement, or broken inverse map is a new
        // visual lifetime and must receive a fresh key on the next host sync.
        let retains_same_direct_binding = old_object_id == Some(object_id)
            && self.drawable_object_map.get(&object_id).copied() == Some(drawable_id)
            && self
                .presentation_direct_drawable_bindings
                .get(&drawable_id)
                .is_some_and(|binding| {
                    binding.binding_key.object_id == object_id
                        && binding.binding_key.drawable_id == drawable_id
                });
        if !retains_same_direct_binding {
            self.presentation_direct_drawable_bindings
                .remove(&drawable_id);
        }

        if let Some(old_object_id) = old_object_id {
            if old_object_id != object_id
                && self.drawable_object_map.get(&old_object_id).copied() == Some(drawable_id)
            {
                self.drawable_object_map.remove(&old_object_id);
            }
        }

        if let Some(previous_drawable_id) = self.drawable_object_map.get(&object_id).copied() {
            if previous_drawable_id != drawable_id {
                if let Some(previous_drawable) = self.drawable_map.get_mut(&previous_drawable_id) {
                    previous_drawable.set_object_id(None);
                }
                self.presentation_direct_drawable_bindings
                    .remove(&previous_drawable_id);
            }
        }

        if let Some(drawable) = self.drawable_map.get_mut(&drawable_id) {
            drawable.set_object_id(Some(object_id));
            self.drawable_object_map.insert(object_id, drawable_id);
            Ok(())
        } else {
            Err(GameClientError::DrawableNotFound(drawable_id))
        }
    }

    pub fn get_drawable_for_object(&self, object_id: ObjectID) -> Option<DrawableId> {
        self.drawable_object_map.get(&object_id).copied()
    }

    /// Iterates over drawables in a given region
    pub fn iterate_drawables_in_region<F>(
        &self,
        region: Option<&Region3D>,
        mut callback: F,
    ) -> GameClientResult<()>
    where
        F: FnMut(&dyn Drawable),
    {
        for drawable in self.drawable_map.values() {
            let position = drawable.get_position();

            let in_region = match region {
                None => true,
                Some(r) => {
                    position.x >= r.lo.x
                        && position.x <= r.hi.x
                        && position.y >= r.lo.y
                        && position.y <= r.hi.y
                        && position.z >= r.lo.z
                        && position.z <= r.hi.z
                }
            };

            if in_region {
                callback(drawable.as_ref());
            }
        }

        Ok(())
    }

    /// Sets the current frame number
    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame;
        publish_live_game_client_frame(self);
    }

    /// Sets the local player identifier used for command routing.
    pub fn set_local_player_id(&mut self, player_id: i32) {
        self.local_player_id = player_id;
        set_local_player_id(player_id);
    }

    /// Gets the current frame number
    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// Gets all drawable IDs
    pub fn get_drawable_ids(&self) -> Vec<DrawableId> {
        self.drawable_map.keys().copied().collect()
    }

    /// Evaluates context commands for a drawable
    pub fn evaluate_context_command(
        &self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: CommandEvaluateType,
    ) -> GameClientResult<GameMessageType> {
        match &self.command_translator {
            Some(translator) => {
                Ok(translator.evaluate_context_command(drawable, position, cmd_type)?)
            }
            None => Ok(GameMessageType::Invalid),
        }
    }

    /// Adds a drawable to the text-bearing list for UI rendering
    pub fn add_text_bearing_drawable(&mut self, drawable_id: DrawableId) {
        self.text_bearing_drawables.push(drawable_id);
    }

    /// Flushes all text-bearing drawables
    pub fn flush_text_bearing_drawables(&mut self) -> GameClientResult<()> {
        for &drawable_id in &self.text_bearing_drawables {
            if let Some(drawable) = self.drawable_map.get(&drawable_id) {
                drawable.draw_ui_text()?;
            }
        }
        self.text_bearing_drawables.clear();
        Ok(())
    }

    /// Sets time of day for all drawables
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) -> GameClientResult<()> {
        if let Some(display) = self.subsystem_manager.display.as_ref().map(Arc::clone) {
            let mut display = display.lock().map_err(|_| {
                GameClientError::SubsystemError("Display lock poisoned".to_string())
            })?;
            display.set_time_of_day(map_client_time_of_day_to_ini(tod));
        }

        self.iterate_drawables_in_region(None, |drawable| {
            let _ = drawable.set_time_of_day(tod);
        })
    }

    /// Loads a map
    pub fn load_map(&mut self, map_name: &str) -> GameClientResult<bool> {
        if map_name.is_empty() {
            return Ok(false);
        }

        if self
            .loaded_map
            .as_ref()
            .map(|map| map.name == map_name)
            .unwrap_or(false)
        {
            log::debug!("Map '{}' already loaded", map_name);
            return Ok(true);
        }

        let asset_manager = self
            .subsystem_manager
            .asset_manager
            .as_ref()
            .ok_or_else(|| {
                GameClientError::InitializationFailed(
                    "Asset manager not initialized before map load".to_string(),
                )
            })?;

        let normalized_name = map_name.replace('\\', "/");
        let candidates = [
            format!("Maps/{0}/{0}.map", normalized_name),
            format!("Maps/{0}.map", normalized_name),
            format!("{0}.map", normalized_name),
        ];

        let mut last_error = None;
        for candidate in candidates.iter() {
            let path = PathBuf::from(candidate);
            match pollster::block_on(
                asset_manager.load_asset(path.clone(), AssetPriority::Critical),
            ) {
                Ok(handle) => {
                    log::info!("Loaded map asset: {}", candidate);

                    if let Some(previous) = self.loaded_map.take() {
                        asset_manager.release_asset(previous.handle);
                    }

                    self.loaded_map = Some(LoadedMap {
                        name: map_name.to_string(),
                        handle,
                    });

                    return Ok(true);
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        if let Some(err) = last_error {
            log::error!("Failed to load map '{}': {}", map_name, err);
            return Err(GameClientError::ResourceLoadingFailed(err.to_string()));
        }

        Ok(false)
    }

    /// Unloads a map
    pub fn unload_map(&mut self, map_name: &str) -> GameClientResult<()> {
        log::info!("Unloading map: {}", map_name);

        if let Some(loaded) = self.loaded_map.take() {
            if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
                asset_manager.release_asset(loaded.handle);
            }
        }

        Ok(())
    }

    /// Preloads assets for performance optimization
    pub fn preload_assets(&mut self, time_of_day: TimeOfDay) -> GameClientResult<()> {
        log::info!("Preloading assets for time of day: {:?}", time_of_day);

        // Preload assets for existing drawables
        self.iterate_drawables_in_region(None, |drawable| {
            let _ = drawable.preload_assets(time_of_day);
        })?;

        // Preload common assets from thing factory
        self.preload_template_assets_from_factory(time_of_day)?;

        // Preload UI assets
        if let Some(ref display) = self.subsystem_manager.display {
            display
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .preload_common_textures()?;
        }

        // C++ `GameClient::preloadAssets` finishes the client preloading pass
        // with `TheParticleSystemManager->preloadAssets(timeOfDay)`.  Gather
        // the exact authored texture names while holding the simulation-side
        // manager, then release it before touching WGPU resources.
        let particle_textures = {
            match crate::effects::particle_manager::get_particle_system_manager_mut() {
                Ok(mut manager_guard) => match manager_guard.as_mut() {
                    Some(manager) => {
                        manager.preload_assets();
                        manager.preloaded_texture_assets().to_vec()
                    }
                    None => Vec::new(),
                },
                Err(_) => Vec::new(),
            }
        };
        if !particle_textures.is_empty() {
            crate::effects::particle_renderer::with_particle_renderer(|renderer| {
                if let Ok(mut renderer) = renderer.lock() {
                    renderer.preload_authored_textures(&particle_textures);
                }
            });
        }

        if let Some(ref asset_manager) = self.subsystem_manager.asset_manager {
            pollster::block_on(asset_manager.preload_configured_assets()).map_err(|e| {
                GameClientError::SubsystemError(format!("Asset preloading failed: {e}"))
            })?;
        }

        log::info!("Asset preloading completed");
        Ok(())
    }

    /// Gets rendered object count for performance monitoring
    pub fn get_rendered_object_count(&self) -> u32 {
        self.rendered_object_count
    }

    /// Increments rendered object count
    pub fn increment_rendered_object_count(&mut self) {
        self.rendered_object_count += 1;
    }

    /// Resets rendered object count
    pub fn reset_rendered_object_count(&mut self) {
        self.rendered_object_count = 0;
    }

    // ==================================================================================
    // Shadow System
    // C++ reference: GameClient::releaseShadows, allocateShadows; ShadowManager
    // ==================================================================================

    /// Create a shadow for an object.  Returns a reference to the newly-created
    /// shadow, or `None` if the object already has a shadow entry.
    ///
    /// C++ parity: `ShadowManager::createShadow` creates a blob or volumetric
    /// shadow per-object and stores it in the shadow table keyed by ObjectID.
    pub fn create_shadow(
        &mut self,
        object_id: ObjectID,
        shadow_type: ShadowType,
        position: crate::system::Coord3D,
        radius: f32,
    ) -> Option<&Shadow> {
        if !self.shadows_enabled {
            return None;
        }
        if self.shadow_map.contains_key(&object_id) {
            return self.shadow_map.get(&object_id);
        }
        let shadow = match shadow_type {
            ShadowType::None => return None,
            ShadowType::Blob => Shadow::new_blob(position, radius),
            ShadowType::Volume => Shadow::new_volume(position),
            ShadowType::Decal => Shadow {
                position,
                radius: radius.max(0.0),
                opacity: 1.0,
                shadow_type: ShadowType::Decal,
                angle: 0.0,
                visible: true,
            },
        };
        self.shadow_map.insert(object_id, shadow);
        self.shadow_map.get(&object_id)
    }

    /// Destroy the shadow associated with the given object.
    ///
    /// C++ parity: `ShadowManager::destroyShadow` removes the shadow from the
    /// table so the renderer no longer projects it.
    pub fn destroy_shadow(&mut self, object_id: ObjectID) {
        self.shadow_map.remove(&object_id);
    }

    /// Update a shadow's position and orientation after the owning object moves.
    ///
    /// C++ parity: called from the drawable update loop when the parent object
    /// changes position or orientation.  The shadow is re-projected onto the
    /// terrain at the new XY coordinates.
    pub fn update_shadow(
        &mut self,
        object_id: ObjectID,
        position: crate::system::Coord3D,
        angle: f32,
    ) {
        if let Some(shadow) = self.shadow_map.get_mut(&object_id) {
            shadow.position = position;
            shadow.angle = angle;
        }
    }

    /// Free all shadow resources — used by the Options screen when the user
    /// disables shadows.
    ///
    /// C++ reference: `GameClient::releaseShadows` iterates all drawables and
    /// calls `releaseShadows()` on each.
    pub fn release_shadows(&mut self) {
        self.shadow_map.clear();
        self.shadows_enabled = false;
        // C++ also iterates drawables and calls drawable->releaseShadows().
        // PARITY_NOTE: Drawable shadow release is handled per-drawable in the
        // W3D renderer layer, not in the core GameClient.
    }

    /// Re-allocate shadow resources — used by the Options screen when the user
    /// re-enables shadows.
    ///
    /// C++ reference: `GameClient::allocateShadows` iterates all drawables and
    /// calls `allocateShadows()` on each.
    pub fn allocate_shadows(&mut self) {
        self.shadows_enabled = true;
    }

    /// Return a reference to the shadow for a given object, if one exists.
    pub fn get_shadow(&self, object_id: ObjectID) -> Option<&Shadow> {
        self.shadow_map.get(&object_id)
    }

    // ==================================================================================
    // View Management
    // C++ reference: GameClient -> TheTacticalView, View::lookAt, W3DView
    // ==================================================================================

    /// Access the current tactical view immutably through a closure.
    ///
    /// C++ parity: `TheGameClient->getWindowManager()->getWindow(0)->getView()`
    /// returns the current tactical view.  In this Rust port the tactical view
    /// is stored as a thread-local.
    pub fn get_view<R>(&self, f: impl FnOnce(&crate::display::view::View) -> R) -> R {
        crate::display::view::with_tactical_view_ref(f)
    }

    /// Access the current tactical view mutably through a closure.
    pub fn get_view_mut<R>(&mut self, f: impl FnOnce(&mut crate::display::view::View) -> R) -> R {
        crate::display::view::with_tactical_view(f)
    }

    // ==================================================================================
    // Shroud Status Queries
    // C++ reference: GameClient visible-object loop (line ~672) calls
    //   object->getShroudedStatus(localPlayerIndex) and collapses to
    //   visible / obscured for the drawable.
    // ==================================================================================

    /// Return the shroud status for a given world position as seen by a player.
    ///
    /// C++ parity: The C++ code calls
    /// `ThePartitionManager->getShroudStatusForPlayer(playerIndex, &pos)`
    /// which checks the player's shroud map at the cell containing `pos`.
    ///
    pub fn get_shroud_status_for_player(
        &self,
        player_index: i32,
        pos: &crate::system::Coord3D,
    ) -> ShroudStatus {
        if player_index < 0 {
            return ShroudStatus::Shrouded;
        }

        let position = gamelogic::common::Coord3D {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        };
        let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
        let Ok(shroud) = shroud_manager.lock() else {
            return ShroudStatus::Shrouded;
        };

        ShroudStatus::from(shroud.get_shroud_state(player_index as u32, &position))
    }

    // ==================================================================================
    // Drawable Creation Hooks
    // C++ reference: GameClient::friend_createDrawable (called for build
    //   placement preview, crash wreckage, etc.).  Purely visual — no
    //   gameplay logic attached.
    // ==================================================================================

    /// Create a drawable at an explicit position and angle.
    ///
    /// Unlike `create_drawable_from_template`, this sets the drawable's world
    /// transform immediately, which is required for build-placement previews
    /// and crash-wreckage spawn.
    pub fn create_drawable_at_pos(
        &mut self,
        template: &ThingTemplate,
        pos: crate::system::Coord3D,
        angle: f32,
    ) -> GameClientResult<DrawableId> {
        let id = self.create_drawable_from_template(template)?;
        if let Some(drawable) = self.drawable_map.get_mut(&id) {
            drawable.set_position(Vector3::new(pos.x, pos.y, pos.z));
            // PARITY_NOTE: C++ stores angle as orientation on the drawable.
            // The Drawable trait doesn't currently expose set_orientation, so
            // we encode it in the transform matrix instead when the renderer
            // consumes the drawable.
            let _ = angle;
        }
        Ok(id)
    }

    // ==================================================================================
    // Message Dispatch
    // C++ reference: GameClientMessageDispatcher::translateGameMessage
    // ==================================================================================

    /// Translate a game message through the client's dispatcher pipeline.
    ///
    /// C++ parity: `GameClientMessageDispatcher::translateGameMessage` is the
    /// last translator on the message stream before messages go to the network.
    /// It gives the client a chance to respond to or create new messages.
    pub fn translate_game_message(&self, msg: &GameMessage) -> GameMessageDisposition {
        self.message_dispatcher.translate_game_message(msg)
    }

    // ==================================================================================
    // GameLogic Object Iteration Methods
    // Reference: GameClient.cpp line 661-698
    // ==================================================================================

    /// Iterate over all GameLogic objects that have drawables bound to them
    ///
    /// This method provides access to GameLogic objects for rendering purposes.
    /// It iterates through all registered objects in the GameLogic layer and invokes
    /// the callback for each object that has an associated drawable.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function called for each object with drawable
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All objects iterated successfully
    /// * `Err(GameClientError)` - If object registry access fails
    ///
    /// # C++ Reference
    ///
    /// Matches C++ GameClient.cpp lines 661-698 - drawable visibility update loop
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_client_rust::core::GameClient;
    /// # let mut client = GameClient::new().unwrap();
    /// client.iterate_objects_with_drawables(|obj_ref| {
    ///     // Process each object that has a drawable
    ///     if let Ok(obj) = obj_ref.read() {
    ///         let pos = obj.get_position();
    ///         println!("Object at ({}, {}, {})", pos.x, pos.y, pos.z);
    ///     }
    /// })?;
    /// # Ok::<(), game_client_rust::core::GameClientError>(())
    /// ```
    pub fn iterate_objects_with_drawables<F>(&self, mut callback: F) -> GameClientResult<()>
    where
        F: FnMut(&Arc<RwLock<GameLogicObject>>),
    {
        // Dual-world residual: GameLogic objects registered in OBJECT_REGISTRY.
        // Host/presentation path keeps drawables only in `drawable_map` and does not
        // populate the registry — this becomes a no-op there (callers use local ticks).
        if OBJECT_REGISTRY.is_empty() {
            return Ok(());
        }
        let all_objects = OBJECT_REGISTRY.get_all_objects();

        // Iterate through objects and invoke callback for those with drawables
        for object_ref in all_objects {
            let has_drawable = object_ref
                .read()
                .ok()
                .and_then(|obj| obj.get_drawable())
                .is_some();
            if has_drawable {
                callback(&object_ref);
            }
        }

        Ok(())
    }

    /// Find a specific GameLogic object by its ID
    ///
    /// Retrieves a strong reference to a GameLogic object given its ObjectID.
    /// This is useful for looking up specific objects during rendering or
    /// command processing.
    ///
    /// # Arguments
    ///
    /// * `object_id` - The ID of the object to find
    ///
    /// # Returns
    ///
    /// * `Ok(Some(object))` - Object found
    /// * `Ok(None)` - Object not found
    /// * `Err(GameClientError)` - Registry access error
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_client_rust::core::GameClient;
    /// # use game_engine::common::ObjectID;
    /// # let client = GameClient::new().unwrap();
    /// let object_id = 42; // Example ID
    /// if let Some(obj_ref) = client.find_game_object(object_id)? {
    ///     if let Ok(obj) = obj_ref.read() {
    ///         println!("Found object: {:?}", obj.get_id());
    ///     }
    /// }
    /// # Ok::<(), game_client_rust::core::GameClientError>(())
    /// ```
    pub fn find_game_object(
        &self,
        object_id: ObjectID,
    ) -> GameClientResult<Option<Arc<RwLock<GameLogicObject>>>> {
        // Wave 269/1002: empty dual-world cannot return Object Arc.
        // Presentation catalog may still know the id — use presentation_object_known.
        if dual_world_registry_unavailable() {
            // Catalog residual consulted for honesty; Arc path stays fail-closed.
            let _ = self.presentation_object_known(object_id);
            return Ok(None);
        }

        Ok(OBJECT_REGISTRY.get_object(object_id))
    }

    /// Wave 1002: dual-world residual — object id present in presentation translator catalog.
    /// Does not yield an Object Arc; authority remains GameLogic/GameWorld.
    pub fn presentation_object_known(&self, object_id: ObjectID) -> bool {
        crate::presentation_translator_residual::translator_catalog_entry(object_id).is_some()
    }

    /// Wave 1006: presentation/shell drawable map size residual (dual-world safe).
    pub fn drawable_count(&self) -> usize {
        self.drawable_map.len()
    }

    /// Update drawable visibility based on shroud/fog of war status
    ///
    /// Synchronizes drawable visibility with the GameLogic shroud system.
    /// Objects in fog of war are marked as obscured so they aren't rendered.
    ///
    /// # Arguments
    ///
    /// * `local_player_index` - The local player's index for shroud calculations
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Visibility updated successfully
    /// * `Err(GameClientError)` - If update fails
    ///
    /// # C++ Reference
    ///
    /// Matches C++ GameClient.cpp:661-698 shroud visibility update
    ///
    /// # Note
    ///
    /// Uses GameLogic shroud status to hide or reveal drawables for the local player.
    pub fn update_drawable_visibility(&mut self, local_player_index: i32) -> GameClientResult<()> {
        use gamelogic::common::types::ObjectShroudStatus;

        // Wave 1020/1044: host empty dual-world peels presentation catalog shroud/
        // status onto drawable_map. Catalog shroud_status residual: 0/1 clear-ish,
        // >=2 fogged/shrouded (fully obscured). Also hide destroyed and non-local
        // effectively-stealthed residuals (C++ drawable effectively hidden).
        if dual_world_registry_unavailable() {
            let _ = local_player_index;
            let pairs: Vec<(ObjectID, DrawableId)> = self
                .drawable_object_map
                .iter()
                .map(|(oid, did)| (*oid, *did))
                .collect();
            for (object_id, drawable_id) in pairs {
                let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(object_id)
                else {
                    continue;
                };
                let fully_obscured = entry.shroud_status >= 2;
                let local =
                    crate::presentation_translator_residual::translator_entry_is_local(&entry);
                // Wave 1044: destroyed residual always hidden; stealthed non-local hidden.
                // Wave 1067: sold residual always hidden (C++ OBJECT_STATUS_SOLD drawable).
                // Wave 1069: masked residual always hidden (C++ OBJECT_STATUS_MASKED drawable).
                let status_hidden = entry.destroyed
                    || entry.sold
                    || entry.masked
                    || (entry.effectively_stealthed && !local);
                if let Some(drawable) = self.drawable_map.get_mut(&drawable_id) {
                    // Trait path: stealth look residual for non-local effectively stealthed.
                    if entry.effectively_stealthed && !local {
                        drawable
                            .set_stealth_look(crate::drawable::drawable::StealthLook::Invisible);
                    }
                    drawable.set_visible(!fully_obscured && !status_hidden);
                    drawable.set_fully_obscured_by_shroud(fully_obscured);
                }
            }
            return Ok(());
        }

        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(mut obj) = obj_ref.write() else {
                return;
            };

            if obj.is_destroyed() {
                if let Some(drawable) = obj.get_drawable() {
                    if let Ok(mut drawable_guard) = drawable.write() {
                        drawable_guard.set_visible(false);
                    }
                }
                return;
            }

            // Keep object-level visibility bookkeeping up to date.
            let _ = obj.update_visibility_for_all_players(self.frame);

            let shroud = obj.get_shrouded_status(local_player_index);
            let fully_obscured = matches!(
                shroud,
                ObjectShroudStatus::Fogged
                    | ObjectShroudStatus::Shrouded
                    | ObjectShroudStatus::InvalidButPreviousValid
            );

            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    drawable_guard.set_visible(!fully_obscured);
                }
            }
        })?;

        Ok(())
    }

    /// Synchronize GameClient drawables with GameLogic objects.
    ///
    /// Updates drawable transforms from their owning GameLogic objects. This mirrors the
    /// C++ render-sync step that keeps drawables aligned with object positions/orientations.
    pub fn sync_with_game_logic(&mut self) -> GameClientResult<()> {
        // Wave 1023/1050: host empty dual-world peels translator catalog pose onto drawable_map.
        // PresentationFrame apply_presentation_pose remains primary; this covers callers
        // that still hit sync_with_game_logic without a fresh pose batch.
        // Wave 1050: also peel visibility status (destroyed/stealth/FOW) with pose.
        if dual_world_registry_unavailable() {
            let pairs: Vec<(ObjectID, DrawableId)> = self
                .drawable_object_map
                .iter()
                .map(|(oid, did)| (*oid, *did))
                .collect();
            for (object_id, drawable_id) in pairs {
                let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(object_id)
                else {
                    continue;
                };
                let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
                    continue;
                };
                // Wave 1023/1024: catalog pose residual (position + yaw).
                let position =
                    Vector3::new(entry.position[0], entry.position[1], entry.position[2]);
                drawable.set_position(position);
                // C++ Matrix3D translation * Rotate_Y residual.
                let transform =
                    Matrix4::translation(position).mul(&Matrix4::rotation_y(entry.orientation));
                drawable.set_instance_transform(transform);
                // Wave 1050: status/FOW visibility residual (parity with update_drawable_visibility).
                let fully_obscured = entry.shroud_status >= 2;
                let local =
                    crate::presentation_translator_residual::translator_entry_is_local(&entry);
                // Wave 1067: sold residual always hidden (C++ OBJECT_STATUS_SOLD drawable).
                // Wave 1069: masked residual always hidden (C++ OBJECT_STATUS_MASKED drawable).
                let status_hidden = entry.destroyed
                    || entry.sold
                    || entry.masked
                    || (entry.effectively_stealthed && !local);
                if entry.effectively_stealthed && !local {
                    drawable.set_stealth_look(crate::drawable::drawable::StealthLook::Invisible);
                } else if entry.effectively_stealthed && local {
                    drawable.set_stealth_look(
                        crate::drawable::drawable::StealthLook::VisibleFriendly,
                    );
                }
                drawable.set_visible(!fully_obscured && !status_hidden);
                drawable.set_fully_obscured_by_shroud(fully_obscured);
            }
            return Ok(());
        }

        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(obj) = obj_ref.read() else {
                return;
            };
            let pos = *obj.get_position();
            let angle = obj.get_orientation();
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let mut transform =
                        glam::Mat4::from_translation(glam::vec3(pos.x, pos.y, pos.z));
                    transform *= glam::Mat4::from_rotation_y(angle);
                    drawable_guard.set_transform(transform);
                }
            }
        })?;

        Ok(())
    }

    /// Main rendering update - called each frame
    ///
    /// Performs all per-frame updates needed for rendering:
    /// - Syncs with GameLogic objects
    /// - Updates drawable positions from object positions
    /// - Updates visibility based on shroud
    /// - Updates animations
    ///
    /// # Arguments
    ///
    /// * `timing` - Frame timing information
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Update successful
    /// * `Err(GameClientError)` - If update fails
    ///
    /// # C++ Reference
    ///
    /// Matches C++ GameClient.cpp Draw functions and update loop
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use game_client_rust::core::GameClient;
    /// # use game_engine::common::frame_clock::{FrameClock, FrameTiming};
    /// # let mut client = GameClient::new().unwrap();
    /// # let mut clock = FrameClock::new();
    /// let timing = clock.next_frame();
    /// client.update_for_rendering(&timing)?;
    /// # Ok::<(), game_client_rust::core::GameClientError>(())
    /// ```
    pub fn update_for_rendering(&mut self, timing: &FrameTiming) -> GameClientResult<()> {
        let visual_delta = if self.should_freeze_visual_time() {
            0.0
        } else {
            timing.delta_seconds()
        };

        // Host/presentation path: no dual-world OBJECT_REGISTRY. Main owns pose/shroud
        // via PresentationFrame apply_*; only local drawable modules tick here.
        if OBJECT_REGISTRY.is_empty() {
            self.update_drawables_local(visual_delta)?;
            // Wave 1021: catalog shroud residual on presentation shell render path.
            self.update_drawable_visibility(self.local_player_id)?;
            self.submit_projectile_streams_to_bridge()?;
            return Ok(());
        }

        // Dual-world residual: sync pose/visibility from registry-bound objects.
        self.sync_with_game_logic()?;
        self.update_drawable_visibility(self.local_player_id)?;
        self.update_drawable_animations(visual_delta)?;
        self.submit_projectile_streams_to_bridge()?;

        Ok(())
    }

    /// Update all drawable animations
    ///
    /// Steps animation state forward for all active drawables.
    ///
    /// # Arguments
    ///
    /// * `delta_time` - Time elapsed since last frame in seconds
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Animations updated
    /// * `Err(GameClientError)` - If update fails
    fn update_drawable_animations(&mut self, delta_time: f32) -> GameClientResult<()> {
        let frame = self.frame;

        for drawable in self.drawable_map.values_mut() {
            // Update drawable animation state
            // The drawable's update method advances animation frames
            drawable.update(delta_time);
        }

        // Dual-world residual only — host drawables live solely in drawable_map above.
        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(mut obj) = obj_ref.write() else {
                return;
            };
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    let _ = drawable_guard.update(delta_time, frame);
                }
            }
        })?;
        let _ = frame;
        Ok(())
    }

    fn submit_projectile_streams_to_bridge(&mut self) -> GameClientResult<()> {
        let Some(the_client) = TheGameClient::get() else {
            return Ok(());
        };

        let object_ids: Vec<u32> = self.drawable_object_map.keys().copied().collect();

        if let Ok(mut bridge_guard) = crate::render_bridge::get_render_bridge().lock() {
            if let Some(bridge) = bridge_guard.as_mut() {
                for object_id in object_ids {
                    if let Some(stream) = the_client.get_drawable_projectile_stream(object_id) {
                        let lines = stream
                            .lines
                            .iter()
                            .map(|seg| {
                                seg.iter()
                                    .map(|p| glam::Vec3::new(p.x, p.y, p.z))
                                    .collect::<Vec<_>>()
                            })
                            .collect();

                        let submission = crate::render_bridge::ProjectileStreamSubmission {
                            drawable_id: object_id,
                            lines,
                            texture_name: stream.texture_name.as_str().to_string(),
                            width: stream.width,
                            tile_factor: stream.tile_factor,
                            scroll_rate: stream.scroll_rate,
                        };
                        bridge.submit_projectile_stream(submission);
                    }
                }
            }
        }
        Ok(())
    }

    // Private implementation methods

    fn alloc_drawable_id(&mut self) -> DrawableId {
        let global_next = GLOBAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed).max(1);
        if self.next_drawable_id.0 < global_next {
            self.next_drawable_id = DrawableId(global_next);
        }
        let id = self.next_drawable_id;
        let next = self.next_drawable_id.0.saturating_add(1).max(1);
        self.next_drawable_id = DrawableId(next);
        GLOBAL_NEXT_DRAWABLE_ID.fetch_max(next, Ordering::Relaxed);
        id
    }

    fn get_drawable_id_counter(&self) -> u32 {
        self.next_drawable_id.0.max(1)
    }

    fn set_drawable_id_counter(&mut self, next_drawable_id: u32) {
        let normalized = next_drawable_id.max(1);
        self.next_drawable_id = DrawableId(normalized);
        GLOBAL_NEXT_DRAWABLE_ID.fetch_max(normalized, Ordering::Relaxed);
    }

    fn global_drawable_id_counter() -> u32 {
        GLOBAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed).max(1)
    }

    fn set_global_drawable_id_counter(next_drawable_id: u32) {
        GLOBAL_NEXT_DRAWABLE_ID.store(next_drawable_id.max(1), Ordering::Relaxed);
    }

    /// Pull TheGameClient DRAWABLE_STATE objectless rows into this archive so
    /// leftover `GameClient::xfer` writes them with `objectID == INVALID_ID`.
    pub fn import_objectless_from_logic_client(&mut self) {
        let Some(logic_client) = TheGameClient::get() else {
            return;
        };
        for (id, state) in logic_client.snapshot_objectless_drawables() {
            if id == 0 || state.template_name.trim().is_empty() {
                continue;
            }
            let drawable_id = DrawableId(id);
            if self.drawable_map.contains_key(&drawable_id) {
                continue;
            }
            let mut drawable = BasicDrawable::new(drawable_id);
            drawable.set_id(drawable_id);
            drawable.set_template_name(Some(state.template_name));
            drawable.set_position(Vector3::new(
                state.position.x,
                state.position.y,
                state.position.z,
            ));
            self.drawable_map.insert(drawable_id, Box::new(drawable));
            if id >= self.next_drawable_id.0 {
                self.next_drawable_id = DrawableId(id.saturating_add(1).max(1));
                self.set_drawable_id_counter(self.next_drawable_id.0);
            }
        }
    }

    /// Push leftover objectless drawables back into TheGameClient so PUC /
    /// lock-on / rope IDs rematch after load.
    pub fn export_objectless_to_logic_client(&self) {
        let Some(logic_client) = TheGameClient::get() else {
            return;
        };
        logic_client.clear_objectless_drawables();
        for (id, drawable) in &self.drawable_map {
            if drawable.get_object_id().is_some() {
                continue;
            }
            let Some(template_name) = drawable.get_template_name() else {
                continue;
            };
            if template_name.is_empty() {
                continue;
            }
            let position = drawable.get_position();
            logic_client.restore_objectless_drawable(
                id.0,
                &gamelogic::helpers::DrawableState {
                    template_name: template_name.to_string(),
                    indicator_color: gamelogic::common::Color::default(),
                    position: gamelogic::common::Coord3D::new(position.x, position.y, position.z),
                    orientation: 0.0,
                    shroud_status_object_id: INVALID_ID,
                    beam_start: None,
                    beam_end: None,
                    beam_width: None,
                    laser_growth_frames: None,
                    laser_growth_start_frame: None,
                    projectile_stream: None,
                    drawable: None,
                    expiration_frame: None,
                },
            );
        }
    }

}

fn log_startup_shell_mapped_images() {
    let collection = get_mapped_image_collection();
    let collection = collection.read();
    for name in [
        "MainMenuBackdrop",
        "MainMenuPulse",
        "GeneralsLogo",
        "MainMenuRuler",
    ] {
        match collection.find_image_by_name(name) {
            Some(image) => log::debug!(
                "startup mapped image: name={} file={} uv=({}, {}, {}, {}) size={}x{} tex={}x{}",
                name,
                image.get_filename(),
                image.get_uv().min.x,
                image.get_uv().min.y,
                image.get_uv().max.x,
                image.get_uv().max.y,
                image.get_image_width(),
                image.get_image_height(),
                image.get_texture_size().x,
                image.get_texture_size().y,
            ),
            None => log::debug!("startup mapped image missing: {name}"),
        }
    }
}

impl Snapshotable for GameClient {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 3;
        xfer.xfer_version(&mut version, 3)
            .map_err(|e| e.to_string())?;

        let mut frame = self.frame;
        xfer.xfer_unsigned_int(&mut frame)
            .map_err(|e| e.to_string())?;

        // Drawable TOC — inlined from xfer_drawable_toc (version 1)
        let mut toc_version: XferVersion = 1;
        xfer.xfer_version(&mut toc_version, 1)
            .map_err(|e| e.to_string())?;

        let mut toc_count: u32 = self.drawable_toc.len() as u32;
        xfer.xfer_unsigned_int(&mut toc_count)
            .map_err(|e| e.to_string())?;

        for entry in &self.drawable_toc {
            let mut name = entry.name.clone();
            xfer.xfer_ascii_string(&mut name)
                .map_err(|e| e.to_string())?;
            let mut id = entry.id;
            xfer.xfer_unsigned_short(&mut id)
                .map_err(|e| e.to_string())?;
        }

        let save_entries = self.collect_saveable_drawables_sorted()?;
        let mut drawable_count: u16 = save_entries
            .len()
            .try_into()
            .map_err(|_| "Too many drawables to CRC".to_string())?;
        xfer.xfer_unsigned_short(&mut drawable_count)
            .map_err(|e| e.to_string())?;

        let toc_lookup: HashMap<String, u16> = self
            .drawable_toc
            .iter()
            .map(|entry| (entry.name.clone(), entry.id))
            .collect();

        for (drawable_id, template_name) in &save_entries {
            let mut toc_id = toc_lookup
                .get(template_name)
                .copied()
                .ok_or_else(|| "TOC entry not found during CRC".to_string())?;
            xfer.xfer_unsigned_short(&mut toc_id)
                .map_err(|e| e.to_string())?;

            xfer.begin_block().map_err(|e| format!("{:?}", e))?;

            if let Some(drawable) = self.drawable_map.get(drawable_id) {
                let mut object_id: ObjectID = drawable.get_object_id().unwrap_or(INVALID_ID);
                xfer.xfer_unsigned_int(&mut object_id)
                    .map_err(|e| e.to_string())?;
            }

            xfer.end_block().map_err(|e| format!("{:?}", e))?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let mut frame = self.frame;
        xfer.xfer_unsigned_int(&mut frame)
            .map_err(|e| e.to_string())?;
        self.frame = frame;
        publish_live_game_client_frame(self);

        self.xfer_drawable_toc(xfer)?;

        let save_entries = if xfer.is_writing() {
            self.collect_saveable_drawables_sorted()?
        } else {
            self.drawable_map.clear();
            self.drawable_object_map.clear();
            self.presentation_direct_drawable_bindings.clear();
            Vec::new()
        };

        let mut drawable_count: u16 = save_entries
            .len()
            .try_into()
            .map_err(|_| "Too many drawables to serialize".to_string())?;

        xfer.xfer_unsigned_short(&mut drawable_count)
            .map_err(|e| e.to_string())?;

        if xfer.is_writing() {
            let toc_lookup: HashMap<String, u16> = self
                .drawable_toc
                .iter()
                .map(|entry| (entry.name.clone(), entry.id))
                .collect();

            for (drawable_id, template_name) in save_entries {
                let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
                    return Err(format!(
                        "Drawable '{}' disappeared during save serialization",
                        drawable_id.0
                    ));
                };

                let mut toc_id = toc_lookup
                    .get(&template_name)
                    .copied()
                    .ok_or_else(|| "Drawable TOC entry not found".to_string())?;
                xfer.xfer_unsigned_short(&mut toc_id)
                    .map_err(|e| e.to_string())?;

                xfer.begin_block().map_err(|e| format!("{:?}", e))?;

                let mut object_id: ObjectID = drawable.get_object_id().unwrap_or(INVALID_ID);
                xfer.xfer_unsigned_int(&mut object_id)
                    .map_err(|e| e.to_string())?;

                Self::xfer_drawable_snapshot(drawable.as_mut(), xfer)?;

                xfer.end_block().map_err(|e| format!("{:?}", e))?;
            }
        } else if drawable_count > 0 {
            // Logic-only in-match saves (headless host, unit tests) carry
            // drawable_count == 0; C++ Drawable::xfer assumes TheThingFactory
            // always exists, but demanding it here failed those loads before
            // a single drawable was even decoded. Require it only when there
            // is actually a drawable to resolve a template for.
            let factory_guard = get_thing_factory().map_err(|_| "ThingFactory lock failed")?;
            let factory = factory_guard
                .as_ref()
                .ok_or_else(|| "ThingFactory not initialized".to_string())?;

            for _ in 0..drawable_count {
                let mut toc_id: u16 = 0;
                xfer.xfer_unsigned_short(&mut toc_id)
                    .map_err(|e| e.to_string())?;

                let toc_name = self
                    .find_toc_entry_by_id(toc_id)
                    .map(|entry| entry.name.clone())
                    .ok_or_else(|| "Drawable TOC entry not found for id".to_string())?;

                let data_size = xfer.begin_block().map_err(|e| format!("{:?}", e))?;

                let Some(template) = factory.find_template(&toc_name, false) else {
                    xfer.skip(data_size).map_err(|e| format!("{:?}", e))?;
                    continue;
                };

                let mut object_id: ObjectID = INVALID_ID;
                xfer.xfer_unsigned_int(&mut object_id)
                    .map_err(|e| e.to_string())?;

                // Host/presentation path: allow drawable load without dual-world registry.
                // object_id stays on the drawable; bind only when registry is populated.

                let mut reuse_id = None;
                if object_id != INVALID_ID {
                    if let Some(existing_id) = self.get_drawable_for_object(object_id) {
                        reuse_id = Some(existing_id);
                    }
                }

                let mut drawable = if let Some(existing_id) = reuse_id {
                    let needs_replace = self
                        .drawable_map
                        .get(&existing_id)
                        .map(|existing| {
                            !Self::drawable_matches_saved_template(
                                existing.as_ref(),
                                &template,
                                factory,
                            )
                        })
                        .unwrap_or(true);
                    if needs_replace {
                        self.destroy_drawable(existing_id)
                            .map_err(|e| e.to_string())?;
                        None
                    } else {
                        self.drawable_map.remove(&existing_id)
                    }
                } else {
                    None
                };

                if drawable.is_none() {
                    let created_id = self
                        .create_drawable_from_template(template.as_ref())
                        .map_err(|e| {
                            format!(
                                "GameClient::xfer - Unable to create drawable for '{}': {}",
                                template.get_name(),
                                e
                            )
                        })?;
                    let mut created = self.drawable_map.remove(&created_id).ok_or_else(|| {
                        format!(
                            "GameClient::xfer - Created drawable '{}' was not registered",
                            created_id.0
                        )
                    })?;
                    if object_id != INVALID_ID {
                        created.set_object_id(Some(object_id));
                    }
                    drawable = Some(created);
                }

                let mut drawable = drawable.expect("drawable exists");
                Self::xfer_drawable_snapshot(drawable.as_mut(), xfer)?;

                let id = drawable.get_id();
                if let Some(object_id) = drawable.get_object_id() {
                    self.drawable_object_map.insert(object_id, id);
                }
                self.drawable_map.insert(id, drawable);

                xfer.end_block().map_err(|e| format!("{:?}", e))?;

                if object_id != INVALID_ID {
                    // Dual-world residual bind only; host uses drawable_object_map.
                    if OBJECT_REGISTRY.get_object(object_id).is_some() {
                        let _ = self.bind_drawable_to_object(id, object_id);
                    }
                }
            }
        }

        // C++ GameClient::xfer v2+ briefing history (GameClient.cpp:1531-1560).
        xfer_diplomacy_briefing_history(xfer, version)?;

        if xfer.is_reading() {
            self.load_post_process()?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Binding keys describe pre-load runtime Drawable instances and are
        // intentionally not serialized.  Recreated presentation drawables
        // receive fresh keys on the next host sync.
        self.presentation_direct_drawable_bindings.clear();
        self.drawable_object_map.clear();
        let mut next_drawable_id = self.next_drawable_id.0.max(1);

        for drawable in self.drawable_map.values() {
            let id = drawable.get_id();
            if id.0 >= next_drawable_id {
                next_drawable_id = id.0.saturating_add(1).max(1);
            }
            if let Some(object_id) = drawable.get_object_id() {
                self.drawable_object_map.insert(object_id, id);
            }
        }

        // C++ scans the global drawable list; include GameLogic-owned drawables as well
        // so the next ID counter cannot regress after load.
        // Host path: registry empty — drawable_map already drove next_drawable_id.
        if OBJECT_REGISTRY.is_empty() {
            self.next_drawable_id = DrawableId(next_drawable_id.max(1));
            self.set_drawable_id_counter(self.next_drawable_id.0);
            return Ok(());
        }
        for obj_ref in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj_guard) = obj_ref.read() else {
                continue;
            };
            let Some(drawable_ref) = obj_guard.get_drawable() else {
                continue;
            };
            let Ok(drawable_guard) = drawable_ref.read() else {
                continue;
            };

            let drawable_id = drawable_guard.get_drawable_id();
            if drawable_id >= next_drawable_id {
                next_drawable_id = drawable_id.saturating_add(1).max(1);
            }

            let object_id = drawable_guard.get_object_id();
            if object_id != INVALID_ID {
                self.drawable_object_map
                    .insert(object_id, DrawableId(drawable_id));
            }
        }

        self.next_drawable_id = DrawableId(next_drawable_id.max(1));
        self.set_drawable_id_counter(self.next_drawable_id.0);
        Ok(())
    }
}

impl Drop for GameClient {
    fn drop(&mut self) {
        log::info!("GameClient shutting down");
        clear_live_game_client(self);
        GameClient::reset_global_video_player_streams();
        reset_script_action_runtime_state();
        register_script_display_bridge(None);
        clear_load_screen_presentation_pump();
        shutdown_video_player();

        // Clear all drawables (they'll be dropped automatically)
        self.drawable_map.clear();

        // Subsystems will be dropped automatically through Arc

        log::info!("GameClient shutdown complete");
    }
}

/// C++ `GameClient::xfer` v2+ (`GameClient.cpp:1531-1560`): persist
/// `GetBriefingTextList` and restore via `UpdateDiplomacyBriefingText`.
fn xfer_diplomacy_briefing_history(
    xfer: &mut dyn Xfer,
    version: XferVersion,
) -> Result<(), String> {
    if version < 2 {
        return Ok(());
    }

    if xfer.is_writing() {
        let list = crate::gui::callbacks::diplomacy::get_briefing_text_list();
        let mut num_entries = i32::try_from(list.len()).unwrap_or(i32::MAX);
        xfer.xfer_int(&mut num_entries)
            .map_err(|e| e.to_string())?;
        for mut temp_str in list {
            xfer.xfer_ascii_string(&mut temp_str)
                .map_err(|e| e.to_string())?;
        }
    } else {
        let mut num_entries = 0i32;
        xfer.xfer_int(&mut num_entries)
            .map_err(|e| e.to_string())?;
        crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text("", true);
        for _ in 0..num_entries.max(0) {
            let mut temp_str = String::new();
            xfer.xfer_ascii_string(&mut temp_str)
                .map_err(|e| e.to_string())?;
            crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(&temp_str, false);
        }
    }
    Ok(())
}

fn write_game_client_xfer_bytes(client: &mut GameClient) -> Result<Vec<u8>, String> {
    client.import_objectless_from_logic_client();
    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut xfer = game_engine::common::system::xfer_save::XferSave::new(cursor, 3);
        client.xfer(&mut xfer)?;
    }
    Ok(bytes)
}

fn read_game_client_xfer_bytes(client: &mut GameClient, bytes: &[u8]) -> Result<(), String> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut xfer = game_engine::common::system::xfer_load::XferLoad::new(cursor, 3);
    client.xfer(&mut xfer)?;
    client.export_objectless_to_logic_client();
    Ok(())
}

/// C++ `CHUNK_GameClient` payload: leftover `GameClient::xfer` plus objectless
/// TheGameClient DRAWABLE_STATE rows (PUC beams, lock-on, ropes) and Diplomacy
/// briefing history.
pub fn capture_live_game_client_xfer_bytes() -> Result<Vec<u8>, String> {
    if let Some(result) = with_live_game_client_mut(write_game_client_xfer_bytes) {
        return result;
    }
    let mut client = GameClient::new().map_err(|e| e.to_string())?;
    write_game_client_xfer_bytes(&mut client)
}

pub fn restore_live_game_client_from_xfer_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Ok(());
    }
    if let Some(result) =
        with_live_game_client_mut(|client| read_game_client_xfer_bytes(client, bytes))
    {
        return result;
    }
    let mut client = GameClient::new().map_err(|e| e.to_string())?;
    read_game_client_xfer_bytes(&mut client, bytes)
}


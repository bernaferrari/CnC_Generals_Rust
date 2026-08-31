// Thing-template, factory, FX/OCL store, and GameText lookup helpers
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

struct EngineThingTemplateAdapter {
    inner: Arc<EngineThingTemplate>,
    name: crate::common::AsciiString,
    geometry: crate::common::GeometryInfo,
    kindof_mask: u128,
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
            kindof_mask: inner.get_kindof_bits(),
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
        kind.cpp_bit()
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

    fn get_raw_transport_slot_count(&self) -> crate::common::UnsignedByte {
        self.inner.get_raw_transport_slot_count()
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

    fn get_voice_created(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_voice_created()
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

    fn get_voice_defect(&self) -> crate::common::audio::AudioEventRts {
        self.inner
            .get_voice_defect()
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

    fn is_equivalent_to(&self, other: &dyn crate::common::ThingTemplate) -> bool {
        if self.get_name() == other.get_name() {
            return true;
        }
        if let Some(other_adapter) = other.as_any().downcast_ref::<EngineThingTemplateAdapter>() {
            return self.inner.is_equivalent_to(&other_adapter.inner);
        }
        let other_name = other.get_name();
        self.inner
            .get_build_variations()
            .iter()
            .any(|variation| variation.eq_ignore_ascii_case(other_name.as_str()))
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

    fn get_fence_width(&self) -> crate::common::Real {
        self.inner.get_fence_width()
    }

    fn get_fence_x_offset(&self) -> crate::common::Real {
        self.inner.get_fence_x_offset()
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

    fn get_placement_view_angle(&self) -> crate::common::Real {
        self.inner.get_placement_view_angle()
    }

    fn get_buildable_status(&self) -> Option<game_engine::common::thing::BuildableStatus> {
        Some(self.inner.get_buildable())
    }

    fn get_production_prerequisites(&self) -> &[game_engine::common::rts::ProductionPrerequisite] {
        self.inner.get_prereqs()
    }

    fn get_max_simultaneous_of_type(&self) -> u32 {
        self.inner.get_max_simultaneous_of_type() as u32
    }

    fn get_max_simultaneous_link_key(&self) -> u32 {
        self.inner.get_max_simultaneous_link_key()
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
        mods.handicap_cost_multiplier = player
            .get_handicap()
            .get_cost_multiplier_for_template(self);
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
        mods.handicap_time_multiplier = player
            .get_handicap()
            .get_build_time_multiplier_for_template(self);
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
        // Both layers now preserve the C++ RadarPriorityType discriminants.
        match self.inner.get_radar_priority() {
            game_engine::common::thing::thing_template::RadarPriorityType::Invalid => {
                crate::common::RadarPriorityType::Invalid
            }
            game_engine::common::thing::thing_template::RadarPriorityType::NotOnRadar => {
                crate::common::RadarPriorityType::NotOnRadar
            }
            game_engine::common::thing::thing_template::RadarPriorityType::Structure => {
                crate::common::RadarPriorityType::Structure
            }
            game_engine::common::thing::thing_template::RadarPriorityType::Unit => {
                crate::common::RadarPriorityType::Unit
            }
            game_engine::common::thing::thing_template::RadarPriorityType::LocalUnitOnly => {
                crate::common::RadarPriorityType::LocalUnitOnly
            }
        }
    }

    fn get_shadow_type_bits(&self) -> u32 {
        self.inner.get_shadow_type().bits() as u32
    }
    fn get_shadow_size_x(&self) -> crate::common::Real {
        self.inner.get_shadow_size_x()
    }
    fn get_shadow_size_y(&self) -> crate::common::Real {
        self.inner.get_shadow_size_y()
    }
    fn get_shadow_offset_x(&self) -> crate::common::Real {
        self.inner.get_shadow_offset_x()
    }
    fn get_shadow_offset_y(&self) -> crate::common::Real {
        self.inner.get_shadow_offset_y()
    }
    fn get_shadow_texture_name(&self) -> &str {
        self.inner.get_shadow_texture_name().as_str()
    }
}

// Residual closed (wave 29): GameLogic helpers RNG draws share the Common
// `random_value` ADC stream (no parallel GAME_LOGIC_SEED / GAME_CLIENT_SEED).
// Fail-closed: network residual still deferred; CRC of seed words remains
// host-local crc32fast of the Common 6-word state.

/// TheThingFactory singleton - object creation factory (matching C++ TheThingFactory)
pub struct TheThingFactory;

impl TheThingFactory {
    /// Get a reference to TheThingFactory
    pub fn get() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    /// C++ `System.ini` `Object GenericTracer` / `GenericRope` (DRAWABLE_ONLY).
    /// Does not run full `init_thing_factory` (all Object INI).
    pub fn ensure_system_ini_drawable_only_templates() -> bool {
        game_engine::common::thing::thing_factory::ensure_system_ini_drawable_only_templates()
    }

    pub fn generic_tracer_matches_system_ini() -> bool {
        game_engine::common::thing::thing_factory::generic_tracer_template_matches_system_ini()
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

    /// Find template by name WITHOUT lazy-initializing the shared factory.
    ///
    /// C++ presentation reads TheThingFactory only after the startup INI
    /// load (GameLogic::init); a frame-build must never bootstrap the whole
    /// Object INI tree from a render-time lookup.  `find_template` keeps the
    /// on-demand rebuild for script/creation paths; presentation-side helpers
    /// use this initialized-only variant so headless logic frames neither pay
    /// the full INI scan nor mutate process-global gating state mid-run.
    pub fn find_template_initialized(
        name: impl AsRef<str>,
    ) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let key = crate::common::AsciiString::from(name.as_ref());
        let factory_guard = get_thing_factory().ok()?;
        let factory = factory_guard.as_ref()?;
        let template = factory.find_template(key.as_str(), false)?;
        Some(Arc::new(EngineThingTemplateAdapter::new(template)))
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

    /// Create new object from template (C++ statusBits default empty).
    pub fn new_object(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: &crate::team::Team,
    ) -> Result<
        std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.new_object_with_status(template, team, crate::common::ObjectStatusMaskType::NONE)
    }

    /// C++ `ThingFactory::newObject(tmplate, team, statusBits)` — status is set
    /// on `friend_createObject` **before** `CreateModuleInterface::onCreate`.
    pub fn new_object_with_status(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: &crate::team::Team,
        status_bits: crate::common::ObjectStatusMaskType,
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

        let mut flags = ObjectCreationFlags::from_template();
        flags.status_mask = status_bits;

        let object_id = get_object_manager()
            .write()
            .map_err(|_| "ObjectManager lock poisoned")?
            .create_object(
                &template_name,
                Coord3D::new(0.0, 0.0, 0.0),
                team_arc,
                flags,
            )
            .map_err(|e| e.to_string())?;

        let base = get_object_manager()
            .read()
            .map_err(|_| "ObjectManager lock poisoned")?
            .with_object(object_id, |instance| instance.base())
            .ok_or_else(|| "Created object not found in ObjectManager".to_string())?;

        Ok(base)
    }

    /// Create new object while preserving the exact team handle supplied by the caller.
    ///
    /// C++ ThingFactory receives a Team pointer directly. This helper is for call sites
    /// that still hold the owning Arc and must not re-resolve by TeamFactory ID.
    pub fn new_object_with_team_handle(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: std::sync::Arc<std::sync::RwLock<crate::team::Team>>,
    ) -> Result<
        std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.new_object_with_team_handle_and_status(
            template,
            team,
            crate::common::ObjectStatusMaskType::NONE,
        )
    }

    /// C++ `ThingFactory::newObject` with an explicit statusBits mask.
    pub fn new_object_with_team_handle_and_status(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: std::sync::Arc<std::sync::RwLock<crate::team::Team>>,
        status_bits: crate::common::ObjectStatusMaskType,
    ) -> Result<
        std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        use crate::object_manager::get_object_manager;
        use crate::object_manager::ObjectCreationFlags;

        let template_name = Self::resolve_build_variation_name(&template);

        let mut flags = ObjectCreationFlags::from_template();
        flags.status_mask = status_bits;

        let object_id = get_object_manager()
            .write()
            .map_err(|_| "ObjectManager lock poisoned")?
            .create_object(
                &template_name,
                Coord3D::new(0.0, 0.0, 0.0),
                Some(team),
                flags,
            )
            .map_err(|e| e.to_string())?;

        let base = get_object_manager()
            .read()
            .map_err(|_| "ObjectManager lock poisoned")?
            .with_object(object_id, |instance| instance.base())
            .ok_or_else(|| "Created object not found in ObjectManager".to_string())?;

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
        self.new_object_optional_team_with_status(
            template,
            team,
            crate::common::ObjectStatusMaskType::NONE,
        )
    }

    /// C++ `ThingFactory::newObject(tmplate, team, statusBits)` with optional team.
    pub fn new_object_optional_team_with_status(
        &self,
        template: std::sync::Arc<dyn crate::common::ThingTemplate>,
        team: Option<&crate::team::Team>,
        status_bits: crate::common::ObjectStatusMaskType,
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

        let mut flags = ObjectCreationFlags::from_template();
        flags.status_mask = status_bits;

        let object_id = get_object_manager()
            .write()
            .map_err(|_| "ObjectManager lock poisoned")?
            .create_object(
                &template_name,
                Coord3D::new(0.0, 0.0, 0.0),
                team_arc,
                flags,
            )
            .map_err(|e| e.to_string())?;

        let base = get_object_manager()
            .read()
            .map_err(|_| "ObjectManager lock poisoned")?
            .with_object(object_id, |instance| instance.base())
            .ok_or_else(|| "Created object not found in ObjectManager".to_string())?;

        Ok(base)
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

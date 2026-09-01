//! Retail Object INI seeding and template/model lookup helpers.
use super::*;

impl GameLogic {
    /// Seed templates for retail Object INI entries that the hand-authored
    /// bootstrap does not already cover.
    ///
    /// Starter templates retain their host behavior that generic Object INI
    /// fields do not yet represent.  Their authored Drawable `Scale` is still
    /// refreshed from retail data; additions use only exact object identity
    /// and authored attributes and do not imply an unavailable mesh or an
    /// unsupported behavior module has been ported.
    pub(in super::super::super) fn seed_asset_definition_templates(&mut self) -> usize {
        let Some(manager_arc) = get_asset_manager() else {
            return 0;
        };
        let (definitions, available_models) = match manager_arc.lock() {
            Ok(manager) => {
                let definitions = manager.object_definitions_snapshot();
                let available_models = manager
                    .list_available_models()
                    .into_iter()
                    .map(|model| Self::normalize_model_lookup_key(&model))
                    .collect();
                (definitions, available_models)
            }
            Err(error) => {
                log::warn!(
                    "Skipping retail Object INI seed because AssetManager is poisoned: {error}"
                );
                return 0;
            }
        };
        self.seed_asset_definition_templates_from_snapshot_with_models(
            definitions,
            Some(&available_models),
        )
    }

    /// Catalogue seeding core with no global state, used to characterize the
    /// exact-data-only policy in tests.
    pub(in super::super::super) fn seed_asset_definition_templates_from_snapshot<I>(
        &mut self,
        definitions: I,
    ) -> usize
    where
        I: IntoIterator<Item = (String, ObjectDefinition)>,
    {
        self.seed_asset_definition_templates_from_snapshot_with_models(definitions, None)
    }

    /// As [`Self::seed_asset_definition_templates_from_snapshot`], with the
    /// exact archive W3D names available to the active asset manager.
    ///
    /// `Some` is required in production: a visual gameplay template whose
    /// pristine basename is absent must remain unavailable rather than being
    /// spawned with a proxy mesh.  `None` is reserved for pure data tests,
    /// where no asset archive exists to prove or disprove the exact name.
    pub(super) fn seed_asset_definition_templates_from_snapshot_with_models<I>(
        &mut self,
        definitions: I,
        available_models: Option<&HashSet<String>>,
    ) -> usize
    where
        I: IntoIterator<Item = (String, ObjectDefinition)>,
    {
        let mut inserted = 0usize;
        for (name, definition) in definitions {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }

            // Hand-authored starter templates still own their unported host
            // behavior, but Object INI remains authoritative for the Drawable
            // asset scale.  Without this narrow enrichment every curated
            // starter silently retained 1.0 even when the retail object said
            // `Scale = ...`.
            if let Some(template) = self.templates.get_mut(name) {
                if let Some(kind_of) = Self::object_definition_attr(&definition, "kindof") {
                    Self::apply_authored_semantic_kind_bits(template, &kind_of);
                    Self::apply_authored_capture_metadata(template, &kind_of, &definition);
                }
                let rider_change_normal_locomotors =
                    unambiguous_locomotors_for_set(&definition, "SET_NORMAL");
                Self::apply_authored_dock_and_contain_modules(
                    template,
                    &definition,
                    rider_change_normal_locomotors.as_deref(),
                );
                if definition_has_rider_change_contain(&definition) {
                    if let Some(locomotor) = rider_change_normal_locomotors
                        .as_deref()
                        .and_then(|names| names.first())
                    {
                        template.set_locomotor_name(locomotor);
                    }
                }
                apply_locomotor_set_names_from_definition(
                    template,
                    rider_change_normal_locomotors.as_deref(),
                    Self::object_definition_attr(&definition, "locomotor").as_deref(),
                );

                Self::apply_authored_parking_place_metadata(template, &definition);
                Self::apply_authored_flight_deck_metadata(template, &definition);
                Self::apply_authored_supply_truck_metadata(template, &definition);
                // Starter templates are retained by the host before the full
                // Object INI catalogue is seeded.  They still need the exact
                // authored exit interface; otherwise a retail ChinaBarracks
                // silently falls back to the legacy/no-interface production
                // path solely because it already existed in the template map.
                Self::apply_authored_production_exit_metadata(template, &definition);
                Self::apply_authored_eject_pilot_die_metadata(template, &definition);
                Self::apply_authored_rebuild_hole_expose_die_metadata(template, &definition);
                Self::apply_authored_hack_internet_metadata(template, &definition);
                Self::apply_authored_special_power_module_metadata(template, &definition);
                Self::apply_authored_hacker_disable_building_metadata(template, &definition);
                Self::apply_authored_charge_plant_metadata(template, &definition);
                Self::apply_authored_leftover_sa_trigger_sound(template, &definition);

                Self::apply_authored_overcharge_metadata(template, &definition);
                Self::apply_authored_power_plant_update_metadata(template, &definition);
                Self::apply_authored_temporary_weapon_behavior_metadata(template, &definition);
                Self::apply_authored_physics_behavior_metadata(template, &definition);
                Self::apply_authored_geometry(template, &definition);
                Self::apply_authored_weapon_set_create_policy(template, &definition);
                crate::game_logic::host_move_ambient_audio::apply_authored_audio_events(
                    template,
                    &definition,
                );

                // Existing curated starters keep their broader host combat
                // bindings, but a mine-clear conditional primary is source
                // authority only. Clear any stale value when a resolved
                // ChildObject replaced its parent's WeaponSet collection.
                template.mine_clearing_primary_weapon = None;
                template.mine_clearing_primary_weapon_name = definition
                    .mine_clearing_primary_weapon_name()
                    .map(str::to_string);
                if definition.scale_was_specified {
                    template.set_asset_scale(definition.scale);
                }
                continue;
            }

            let model_name = definition
                .model_name
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty() && !model.eq_ignore_ascii_case("none"));
            let is_audio_only = model_name.is_none()
                && crate::game_logic::host_move_ambient_audio::definition_has_sound_ambient(
                    &definition,
                );
            // The generic host template cannot represent an object whose only
            // identity is a behavior/draw module — except SoundAmbient-only map
            // objects, which are seeded so Drawable startAmbientSound can play.
            // Those carry no draw/behavior payload, so the map-object skip list
            // (CINE_/Amb_/Scorch/…) must not silence them in the seed path;
            // should_spawn_fallback keeps the full list.
            if model_name.is_none() && !is_audio_only {
                continue;
            }
            if Self::should_skip_map_object_template(name) && !is_audio_only {
                continue;
            }
            if let Some(model_name) = model_name {
                if let Some(available_models) = available_models {
                    let exact_key = Self::normalize_model_lookup_key(model_name);
                    if !available_models.contains(&exact_key) {
                        log::debug!(
                            "Not seeding retail template '{name}': exact W3D '{model_name}' is unavailable"
                        );
                        continue;
                    }
                }
            }

            let texture_hint = definition.get_primary_texture().map(str::to_string);
            let template = Self::build_template_from_object_definition(
                name,
                &definition,
                texture_hint.as_deref(),
            );
            self.templates.insert(name.to_string(), template);
            inserted = inserted.saturating_add(1);
        }
        inserted
    }

    pub(in super::super::super) fn register_leftover_object_create_overrides_overlay() {
        game_engine::common::thing::register_object_create_overrides_live_overlay(
            overlay_leftover_object_create_overrides_to_live,
        );
    }

    pub(in super::super::super) fn apply_all_leftover_object_create_overrides(&mut self) -> usize {
        Self::register_leftover_object_create_overrides_overlay();
        let overrides = game_engine::common::thing::leftover_object_create_overrides();
        for entry in &overrides {
            self.apply_leftover_object_create_override(
                &entry.name,
                &entry.reskin_from,
                &entry.properties,
            );
        }
        overrides.len()
    }

    pub(in super::super::super) fn apply_pending_leftover_object_override(
        &mut self,
        template_name: &str,
    ) {
        if let Some(entry) =
            game_engine::common::thing::leftover_object_create_override(template_name)
        {
            self.apply_leftover_object_create_override(
                &entry.name,
                &entry.reskin_from,
                &entry.properties,
            );
        }
    }

    pub(in super::super::super) fn apply_leftover_object_create_override(
        &mut self,
        name: &str,
        reskin_from: &str,
        properties: &std::collections::HashMap<String, String>,
    ) {
        if let Some(manager_arc) = get_asset_manager() {
            if let Ok(mut manager) = manager_arc.try_lock() {
                manager.overlay_object_create_overrides(name, reskin_from, properties);
            }
        }
        let definition = leftover_object_definition_for_live(name, reskin_from, properties);
        let existing_key = if self.templates.contains_key(name) {
            Some(name.to_string())
        } else {
            self.templates
                .keys()
                .find(|existing| existing.eq_ignore_ascii_case(name))
                .cloned()
        };
        let Some(existing_key) = existing_key else {
            return;
        };
        let Some(template) = self.templates.get_mut(&existing_key) else {
            return;
        };
        Self::apply_object_definition_create_overrides(template, &definition);
    }

    pub(super) fn apply_object_definition_create_overrides(
        template: &mut ThingTemplate,
        definition: &crate::assets::ObjectDefinition,
    ) {
        if !definition.display_name.is_empty() {
            template.display_name = definition.display_name.clone();
        }
        if let Some(hit_points) = definition.hit_points {
            if hit_points.is_finite() {
                template.set_health(hit_points);
            }
        }
        if definition.scale_was_specified {
            template.set_asset_scale(definition.scale);
        }
        if let Some(model_name) = definition.model_name.as_deref() {
            let model_name = model_name.trim();
            if !model_name.is_empty() && !model_name.eq_ignore_ascii_case("none") {
                template.set_model(model_name);
            }
        }

        let kind_of = Self::object_definition_attr(definition, "kindof").unwrap_or_default();
        if !kind_of.is_empty() {
            Self::apply_authored_semantic_kind_bits(template, &kind_of);
            Self::apply_authored_capture_metadata(template, &kind_of, definition);
            Self::add_faction_structure_kind_bits(template, &kind_of);
            let lower = kind_of.to_ascii_lowercase();
            if lower
                .split_whitespace()
                .any(|token| token == "structure" || token == "immobile")
            {
                template
                    .add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::Attackable);
            }
            if lower.contains("selectable") {
                template.add_kind_of(KindOf::Selectable);
            }
        }

        if let Some(sight) = Self::object_definition_attr(definition, "visionrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
        {
            template.sight_range = sight;
        }
        if let Some(scr) = Self::object_definition_attr(definition, "shroudclearingrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
        {
            template.shroud_clearing_range = scr;
        }
        if let Some(r) = Self::object_definition_attr(definition, "shroudrevealtoallrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
        {
            template.shroud_reveal_to_all_range = r;
        }
        if let Some(cost) = Self::object_definition_attr(definition, "buildcost")
            .and_then(|s| s.trim().parse::<u32>().ok())
        {
            template.build_cost.supplies = cost;
        }
        if let Some(build_time) = Self::object_definition_attr(definition, "buildtime")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|value| value.is_finite())
        {
            template.build_time = build_time;
        }
        if let Some(refund_value) = Self::object_definition_attr(definition, "refundvalue") {
            if let Ok(value) = refund_value.trim().parse::<u16>() {
                template.refund_value = value;
            }
        }
        if let Some(energy) = Self::object_definition_attr(definition, "energyproduction")
            .and_then(|value| value.trim().parse::<i32>().ok())
        {
            template.energy_production = Some(energy);
        }

        if let Some(wname) = definition.base_weapon_name(0) {
            template.set_primary_weapon_name(wname);
        }
        if let Some(wname) = definition.base_weapon_name(1) {
            template.set_secondary_weapon_name(wname);
        }
        if let Some(wname) = definition.base_weapon_name(2) {
            template.set_tertiary_weapon_name(wname);
        }
        Self::apply_authored_weapon_set_create_policy(template, definition);

        let rider_change_normal_locomotors =
            unambiguous_locomotors_for_set(definition, "SET_NORMAL");
        Self::apply_authored_dock_and_contain_modules(
            template,
            definition,
            rider_change_normal_locomotors.as_deref(),
        );
        Self::apply_authored_parking_place_metadata(template, definition);
        Self::apply_authored_flight_deck_metadata(template, definition);
        Self::apply_authored_supply_truck_metadata(template, definition);
        Self::apply_authored_production_exit_metadata(template, definition);
        Self::apply_authored_eject_pilot_die_metadata(template, definition);
        Self::apply_authored_rebuild_hole_expose_die_metadata(template, definition);
        Self::apply_authored_hack_internet_metadata(template, definition);
        Self::apply_authored_special_power_module_metadata(template, definition);
        Self::apply_authored_hacker_disable_building_metadata(template, definition);
        Self::apply_authored_charge_plant_metadata(template, definition);
        Self::apply_authored_leftover_sa_trigger_sound(template, definition);

        Self::apply_authored_overcharge_metadata(template, definition);
        Self::apply_authored_power_plant_update_metadata(template, definition);
        Self::apply_authored_temporary_weapon_behavior_metadata(template, definition);
        Self::apply_authored_physics_behavior_metadata(template, definition);
        Self::apply_authored_geometry(template, definition);
        Self::apply_authored_stealth_update_metadata(template, definition);
        apply_production_prerequisites_from_definition(template, definition);
    }

    pub(in super::super::super) fn add_faction_structure_kind_bits(
        template: &mut ThingTemplate,
        kind_of: &str,
    ) {
        let compact_kind_of = kind_of.replace('_', "");
        let mappings = [
            ("fsbarracks", KindOf::FSBarracks),
            ("fswarfactory", KindOf::FSWarFactory),
            ("fsairfield", KindOf::FSAirfield),
            ("fsinternetcenter", KindOf::FSInternetCenter),
            ("fspower", KindOf::FSPower),
            ("fsbasedefense", KindOf::FSBaseDefense),
            ("fssupplydropzone", KindOf::FSSupplyDropzone),
            ("fssupplycenter", KindOf::FSSupplyCenter),
            ("fssuperweapon", KindOf::FSSuperweapon),
            ("fsstrategycenter", KindOf::FSStrategyCenter),
            ("fsfake", KindOf::FSFake),
            ("fstechnology", KindOf::FSTechnology),
            ("fsblackmarket", KindOf::FSBlackMarket),
            ("fsadvancedtech", KindOf::FSAdvancedTech),
        ];

        for (token, kind) in mappings {
            if compact_kind_of.contains(token) {
                template.add_kind_of(kind);
            }
        }
    }

    pub(in super::super::super) fn object_definition_attr(
        definition: &ObjectDefinition,
        key: &str,
    ) -> Option<String> {
        definition
            .attributes
            .iter()
            .find_map(|(attr, value)| attr.eq_ignore_ascii_case(key).then(|| value.clone()))
    }

    pub(in super::super::super) fn is_model_asset_available(model_name: &str) -> bool {
        let model_name = model_name.trim();
        if model_name.is_empty() {
            return false;
        }

        let Some(manager_arc) = get_asset_manager() else {
            // Keep gameplay path permissive during early startup or in tests
            // where the asset manager may not be initialized.
            return true;
        };
        let Ok(mut manager) = manager_arc.lock() else {
            return true;
        };

        let w3d_filename = if model_name.to_ascii_lowercase().ends_with(".w3d") {
            model_name.to_string()
        } else {
            format!("{model_name}.w3d")
        };

        let mut candidates = vec![
            format!("art/w3d/{w3d_filename}"),
            format!("Art/W3D/{w3d_filename}"),
            w3d_filename.clone(),
            format!("data/w3d/{w3d_filename}"),
            format!("models/{w3d_filename}"),
        ];
        candidates.push(candidates[0].to_ascii_uppercase());
        candidates.push(candidates[0].to_ascii_lowercase());

        candidates
            .into_iter()
            .any(|candidate| manager.can_open_file_sync(&candidate))
    }

    pub(in super::super::super) fn resolve_spawn_model_name(model_name: &str) -> Option<String> {
        if Self::is_model_asset_available(model_name) {
            return Some(
                model_name
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(model_name)
                    .trim()
                    .trim_end_matches(".w3d")
                    .trim_end_matches(".W3D")
                    .to_string(),
            );
        }

        let requested_key = Self::normalize_model_lookup_key(model_name);
        let manager_arc = get_asset_manager()?;
        let manager = manager_arc.lock().ok()?;
        Self::find_exact_available_model_name(
            &requested_key,
            manager.list_available_models().into_iter(),
        )
    }

    /// Locate the requested retail W3D by canonical basename only.
    ///
    /// C++ W3DModelDraw selects damage, construction, snow, and faction state
    /// meshes through its ConditionState graph.  A nearest-name heuristic here
    /// would silently render the wrong state, so an unavailable pristine model
    /// remains unavailable instead of being substituted.
    pub(in super::super::super) fn find_exact_available_model_name<I>(
        requested_key: &str,
        mut available_models: I,
    ) -> Option<String>
    where
        I: Iterator<Item = String>,
    {
        let requested_key = Self::normalize_model_lookup_key(requested_key);
        available_models.find_map(|available_model| {
            let candidate_key = Self::normalize_model_lookup_key(&available_model);
            if candidate_key != requested_key {
                return None;
            }
            Some(
                available_model
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&available_model)
                    .trim_end_matches(".w3d")
                    .trim_end_matches(".W3D")
                    .to_string(),
            )
        })
    }

    pub(in super::super::super) fn normalize_model_lookup_key(model_name: &str) -> String {
        model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D")
            .to_ascii_lowercase()
    }

    /// Build a visual-only fallback only when the authored W3D basename
    /// exists in the mounted archive.  C++ ObjectReskin / W3DTreeDraw load
    /// `ModelName` (PTXPine03), never the object identity (TreeSpruce03).
    /// Do not invent geometry when the base Generals mesh is absent.
    pub(in super::super::super) fn build_fallback_template(
        template_name: &str,
    ) -> Option<ThingTemplate> {
        let lower = template_name.to_ascii_lowercase();
        let mut template = ThingTemplate::new(template_name);
        template.set_health(250.0);
        let authored = crate::assets::drawable_w3d_model_key(template_name);
        let probe = if authored.is_empty() {
            template_name
        } else {
            authored.as_str()
        };
        let fallback_model_name = Self::resolve_spawn_model_name(probe)?;
        template.set_model(&fallback_model_name);

        if let Some(manager_arc) = get_asset_manager() {
            if let Ok(manager) = manager_arc.lock() {
                if let Some(texture_name) = manager.get_texture_for_object(template_name) {
                    if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                        template.texture_name = Some(texture_name);
                    }
                }
            }
        }

        // This path is visual-only: no parsed Object definition means no
        // gameplay evidence for KINDOF_SUPPLY_SOURCE.  Keep the existing
        // mesh fallback policy, but never manufacture a Gather target from a
        // basename such as "SupplyDock" or "Crate".
        let is_structure = Self::should_spawn_fallback_template(template_name);

        if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
        }

        if lower.contains("commandcenter") {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter);
        }
        // Faction drop-off buildings only — not map SupplyDock/SupplyPile sources.
        if is_structure
            && (lower.contains("supplycenter")
                || lower.contains("supplystash")
                || lower.contains("supplydropzone")
                || lower == "supplycenter")
        {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        Some(template)
    }

    pub(in super::super::super) fn build_visual_fallback_template(
        template_name: &str,
    ) -> Option<ThingTemplate> {
        let template = Self::build_fallback_template(template_name)?;
        let model_name = template.model_name.as_deref()?.trim();
        if model_name.is_empty() || !Self::is_model_asset_available(model_name) {
            return None;
        }
        Some(template)
    }

    /// Wave 243: first player id for a team without exposing `&Player`.
    pub fn player_id_for_team(&self, team: Team) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.team == team)
            .map(|player| player.id)
    }

    /// Return a player only when a faction has exactly one active host owner.
    /// This preserves legacy team-only spawns for simple worlds without
    /// silently assigning them to the first of two same-faction players.
    pub fn unique_player_id_for_team(&self, team: Team) -> Option<u32> {
        if team == Team::Neutral {
            return None;
        }
        let mut player_ids = self
            .players
            .values()
            .filter(|player| player.is_alive && player.team == team)
            .map(|player| player.id);
        let first = player_ids.next()?;
        player_ids.next().is_none().then_some(first)
    }
}

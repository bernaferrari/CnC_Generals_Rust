//! Host objects `impl GameLogic` — `spawn_templates`.
//! templates, vision, spawn_faction_base. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// Return every exact source locomotor from one named Object INI set.  The
/// source parser preserves outer declaration order, so duplicate SET_NORMAL
/// rows deliberately remain ambiguous rather than silently becoming the row
/// a lossy attribute map happened to retain.
fn unambiguous_locomotors_for_set(
    definition: &crate::assets::ObjectDefinition,
    set_name: &str,
) -> Option<Vec<String>> {
    let mut matching = definition
        .locomotor_sets
        .iter()
        .filter(|row| row.set_name.eq_ignore_ascii_case(set_name));
    let row = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    (!row.locomotor_names.is_empty()).then(|| row.locomotor_names.clone())
}

fn locomotor_member_names_from_raw(raw: &str) -> Vec<String> {
    let mut parts = raw.split_whitespace();
    let Some(first) = parts.next() else {
        return Vec::new();
    };
    let skip_set = first.eq_ignore_ascii_case("SET_NORMAL")
        || first.eq_ignore_ascii_case("SET_NORMAL_UPGRADED")
        || first.eq_ignore_ascii_case("SET_PANIC")
        || first.eq_ignore_ascii_case("SET_TAXIING")
        || first.eq_ignore_ascii_case("SET_FREEFALL")
        || first.eq_ignore_ascii_case("SET_WANDER")
        || first.eq_ignore_ascii_case("SET_SUPERSONIC")
        || first.eq_ignore_ascii_case("SET_SLUGGISH");
    let mut names = Vec::new();
    if !skip_set && !first.is_empty() && !first.eq_ignore_ascii_case("none") {
        names.push(first.to_string());
    }
    for part in parts {
        if !part.is_empty() && !part.eq_ignore_ascii_case("none") {
            names.push(part.to_string());
        }
    }
    names
}

fn parse_auto_acquire_idle_bits_from_ini(text: &str) -> u32 {
    use gamelogic::object::update::ai_update_interface::{
        AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS, AUTO_ACQUIRE_IDLE_NO,
        AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING, AUTO_ACQUIRE_IDLE_STEALTHED,
    };
    let mut bits = 0u32;
    for tok in text.split(|c: char| c.is_whitespace() || matches!(c, '+' | '|' | ',')) {
        let t = tok.trim();
        if t.is_empty() || t == "=" {
            continue;
        }
        match t.to_ascii_uppercase().as_str() {
            "YES" => bits |= AUTO_ACQUIRE_IDLE,
            "STEALTHED" => bits |= AUTO_ACQUIRE_IDLE_STEALTHED,
            "NO" => bits |= AUTO_ACQUIRE_IDLE_NO,
            "NOTWHILEATTACKING" => bits |= AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING,
            "ATTACK_BUILDINGS" => bits |= AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS,
            _ => {}
        }
    }
    bits
}


fn leftover_object_definition_for_live(
    name: &str,
    reskin_from: &str,
    properties: &std::collections::HashMap<String, String>,
) -> crate::assets::ObjectDefinition {
    let mut definition = if let Some(manager_arc) = get_asset_manager() {
        manager_arc.lock().ok().and_then(|manager| {
            manager
                .resolve_object_definition(name, None)
                .cloned()
                .or_else(|| {
                    (!reskin_from.is_empty())
                        .then(|| manager.resolve_object_definition(reskin_from, None).cloned())
                        .flatten()
                        .map(|mut parent| {
                            parent.name = name.to_string();
                            parent.parent_name = Some(reskin_from.to_string());
                            parent
                        })
                })
        })
    } else {
        None
    }
    .unwrap_or_else(|| crate::assets::ObjectDefinition::new(name.to_string()));
    definition.apply_create_override_properties(properties);
    definition
}

fn overlay_leftover_object_create_overrides_to_live(
    name: &str,
    reskin_from: &str,
    properties: &std::collections::HashMap<String, String>,
) {
    if let Some(manager_arc) = get_asset_manager() {
        if let Ok(mut manager) = manager_arc.try_lock() {
            manager.overlay_object_create_overrides(name, reskin_from, properties);
        }
    }
}


fn apply_locomotor_set_names_from_definition(
    template: &mut crate::game_logic::ThingTemplate,
    unambiguous_normal: Option<&[String]>,
    raw_locomotor: Option<&str>,
) {
    if let Some(names) = unambiguous_normal {
        if !names.is_empty() {
            template.set_locomotor_set_names(names);
            return;
        }
    }
    if let Some(raw) = raw_locomotor {
        let names = locomotor_member_names_from_raw(raw);
        if !names.is_empty() {
            template.set_locomotor_set_names(&names);
            return;
        }
    }
    let fallback =
        crate::game_logic::locomotor_bootstrap::locomotor_set_names_for_unit(&template.name);
    if fallback.len() >= 2 {
        template.set_locomotor_set_names(&fallback);
    }
}



#[inline]
fn definition_has_rider_change_contain(definition: &crate::assets::ObjectDefinition) -> bool {
    definition
        .behavior_modules
        .iter()
        .any(|module| module.class_name.eq_ignore_ascii_case("RiderChangeContain"))
}

fn host_unlook_persist_frames() -> u32 {
    crate::game_logic::host_gamedata_lobby_residual::UNLOOK_PERSIST_DURATION_FRAMES_RESIDUAL.max(0)
        as u32
}

fn container_blocks_passenger_look(container: &Object) -> bool {
    let kind = container.thing.template.contain_module.kind;
    kind != crate::game_logic::ContainModuleKind::None && !container.is_garrison_contain()
}

fn restamp_host_partition_look(
    last: &mut std::collections::HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    live: &mut std::collections::HashSet<ObjectId>,
    shroud_mgr: &mut gamelogic::system::shroud_manager::ShroudManager,
    cell_ops: &mut Vec<(gamelogic::common::Coord3D, f32, u32, bool)>,
    id: ObjectId,
    center: gamelogic::common::Coord3D,
    range: f32,
    mask: u32,
    persist: u32,
    frame: u32,
) {
    live.insert(id);
    let next = (center.x, center.y, center.z, range, mask);
    if let Some(prev) = last.get(&id).copied() {
        let same = (prev.0 - next.0).abs() < 1e-4
            && (prev.1 - next.1).abs() < 1e-4
            && (prev.2 - next.2).abs() < 1e-4
            && (prev.3 - next.3).abs() < 1e-4
            && prev.4 == next.4;
        if same {
            return;
        }
        let old = gamelogic::common::Coord3D::new(prev.0, prev.1, prev.2);
        shroud_mgr.queue_undo_shroud_reveal(&old, prev.3, prev.4, persist, frame);
        cell_ops.push((old, prev.3, prev.4, false));
    }
    shroud_mgr.do_shroud_reveal(&center, range, mask);
    cell_ops.push((center, range, mask, true));
    last.insert(id, next);
}

fn unlook_stale_host_partition_looks(
    last: &mut std::collections::HashMap<ObjectId, (f32, f32, f32, f32, u32)>,
    live: &std::collections::HashSet<ObjectId>,
    shroud_mgr: &mut gamelogic::system::shroud_manager::ShroudManager,
    cell_ops: &mut Vec<(gamelogic::common::Coord3D, f32, u32, bool)>,
    persist: u32,
    frame: u32,
) {
    let stale: Vec<ObjectId> = last
        .keys()
        .copied()
        .filter(|id| !live.contains(id))
        .collect();
    for id in stale {
        if let Some(prev) = last.remove(&id) {
            let old = gamelogic::common::Coord3D::new(prev.0, prev.1, prev.2);
            shroud_mgr.queue_undo_shroud_reveal(&old, prev.3, prev.4, persist, frame);
            cell_ops.push((old, prev.3, prev.4, false));
        }
    }
}


impl GameLogic {
    /// C++ parity: veterancy-level XP multiplier. In C++ each template
    /// defines per-level ExperienceValue; we approximate by scaling the
    /// base value.  C++ values are modest multipliers, not large ones.
    pub(in super::super) fn veterancy_xp_multiplier(level: VeterancyLevel) -> f32 {
        match level {
            VeterancyLevel::Rookie => 1.0,
            VeterancyLevel::Veteran => 1.25,
            VeterancyLevel::Elite => 1.5,
            VeterancyLevel::Heroic => 2.0,
        }
    }

    pub(in super::super) fn should_track_player_stats(&self) -> bool {
        self.sim_time_seconds > 0.0 || self.frame > 0
    }

    pub(in super::super) fn live_score_counts_as_unit_create(obj: &Object) -> bool {
        !obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Infantry) || obj.is_kind_of(KindOf::Vehicle))
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreCreate))
    }

    pub(in super::super) fn live_score_counts_as_building_create(obj: &Object) -> bool {
        obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreCreate))
    }

    pub(in super::super) fn live_score_counts_as_unit_destroy(obj: &Object) -> bool {
        !obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Infantry) || obj.is_kind_of(KindOf::Vehicle))
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreDestroy))
    }

    pub(in super::super) fn live_score_counts_as_building_destroy(obj: &Object) -> bool {
        obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreDestroy))
    }

    pub(in super::super) fn record_unit_production(&mut self, object_id: ObjectId) {
        if !self.should_track_player_stats() {
            return;
        }
        if !gamelogic::helpers::TheGameLogic::is_scoring_enabled() {
            return;
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        if !Self::live_score_counts_as_unit_create(obj) {
            return;
        }
        let template = obj.template_name.clone();
        let player_id = obj
            .owner_player_id
            .or_else(|| self.player_id_for_team(obj.team));
        if let Some(player_id) = player_id {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_unit_produced();
            }
            gamelogic::player::notify_live_object_built(player_id, &template);
        }
    }

    pub(in super::super) fn record_structure_completion(&mut self, object_id: ObjectId) {
        if !self.should_track_player_stats() {
            return;
        }
        if !gamelogic::helpers::TheGameLogic::is_scoring_enabled() {
            return;
        }
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        if !Self::live_score_counts_as_building_create(obj) {
            return;
        }
        let template = obj.template_name.clone();
        let player_id = obj
            .owner_player_id
            .or_else(|| self.player_id_for_team(obj.team));
        if let Some(player_id) = player_id {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_structure_built();
            }
            gamelogic::player::notify_live_object_built(player_id, &template);
        }
    }

    pub(in super::super) fn template_counts_as_unit(template: &ThingTemplate) -> bool {
        !template.is_kind_of(KindOf::Structure)
            && (template.is_kind_of(KindOf::Infantry)
                || template.is_kind_of(KindOf::Vehicle)
                || template.is_kind_of(KindOf::Aircraft))
    }

    pub(in super::super) fn should_skip_map_object_template(template_name: &str) -> bool {
        const ILLEGAL_TEMPLATE_NAMES: &[&str] = &[
            "EMPPulseBomb",
            "GLAAngryMobRockProjectileObject",
            "ClusterMinesBomb",
            "BlackNapalmFirestormSmall",
            "CabooseFullOfTerrorists",
            "GLAAngryMobMolotovCocktailProjectileObject",
            "Firestorm",
            "Avalanche",
            "InfernoTankShell",
            "ChinaArtilleryBarrageShell",
            "ChinaTankOverlordBattleBunker",
            "ChinaTankOverlordPropagandaTower",
            "ChinaTankOverlordGattlingCannon",
            "CINE",
            "GLAInfantryAngryMobNexus",
            "AircraftCarrier",
            "GermanMuseum",
            "Cin_",
            "Amb_",
            "Ambient",
            "GC_",
            "SpecialEffectsTrainCrashObject",
            "Scorch",
        ];

        ILLEGAL_TEMPLATE_NAMES.iter().any(|illegal| {
            template_name.starts_with(illegal)
                || template_name.ends_with(illegal)
                || template_name == *illegal
        })
    }

    pub(in super::super) fn should_spawn_fallback_template(template_name: &str) -> bool {
        if Self::should_skip_map_object_template(template_name) {
            return false;
        }

        let lower = template_name.to_ascii_lowercase();
        lower.contains("tech")
            || lower.contains("supply")
            || lower.contains("oil")
            || lower.contains("bunker")
            || lower.contains("guardtower")
            || lower.contains("tower")
            || lower.contains("commandcenter")
            || lower.contains("refinery")
            || lower.contains("crate")
    }

    pub(in super::super) fn build_template_from_asset_definition(
        template_name: &str,
    ) -> Option<ThingTemplate> {
        let manager_arc = get_asset_manager()?;
        let (definition, texture_hint) = {
            let manager = manager_arc.lock().ok()?;
            let definition = manager
                // Object name is the retail identity.  A model lookup can
                // select a different faction or ConditionState definition.
                .resolve_object_definition(template_name, None)
                .cloned()?;
            let texture_hint = manager.get_texture_for_object(template_name);
            (definition, texture_hint)
        };

        // Audio-only map objects (SoundAmbient, no model) are valid host
        // templates so create_object can start the looping 3D event.


        Some(Self::build_template_from_object_definition(
            template_name,
            &definition,
            texture_hint.as_deref(),
        ))
    }

    // Internal callers outside `game_logic` use this only to build a parsed
    // Object INI fixture before exercising the frozen input/authority seam.
    // Keeping the parser-to-template boundary crate-visible avoids a second,
    // hand-built RiderChange test representation.
    pub(crate) fn build_template_from_object_definition(
        template_name: &str,
        definition: &ObjectDefinition,
        texture_hint: Option<&str>,
    ) -> ThingTemplate {
        let mut template = ThingTemplate::new(template_name);
        let lower = template_name.to_ascii_lowercase();
        let kind_of = Self::object_definition_attr(definition, "kindof")
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !definition.display_name.is_empty() {
            template.display_name = definition.display_name.clone();
        }

        if let Some(hit_points) = definition.hit_points {
            if hit_points > 0.0 {
                template.set_health(hit_points);
            }
        }

        if let Some(model_name) = definition.model_name.as_deref() {
            let model_name = model_name.trim();
            if !model_name.is_empty() && !model_name.eq_ignore_ascii_case("none") {
                // Retain the exact authored basename.  The WGPU collection
                // path resolves it against archives and skips a genuine miss;
                // it must not draw a guessed damage/snow/faction variant.
                template.set_model(model_name);
            }
        }

        // C++ Drawable initializes instance scale from
        // ThingTemplate::getAssetScale().  Preserve the authored Object INI
        // value on the host template so snapshots and rendering never need a
        // template-name scale fallback.
        template.set_asset_scale(definition.scale);
        Self::apply_authored_stealth_update_metadata(&mut template, definition);
        let rider_change_normal_locomotors =
            unambiguous_locomotors_for_set(definition, "SET_NORMAL");
        Self::apply_authored_dock_and_contain_modules(
            &mut template,
            definition,
            rider_change_normal_locomotors.as_deref(),
        );
        Self::apply_authored_parking_place_metadata(&mut template, definition);
        Self::apply_authored_flight_deck_metadata(&mut template, definition);
        Self::apply_authored_deploy_style_metadata(&mut template, definition);
        Self::apply_authored_auto_acquire_metadata(&mut template, definition);

        Self::apply_authored_supply_truck_metadata(&mut template, definition);
        Self::apply_authored_production_exit_metadata(&mut template, definition);
        Self::apply_authored_pilot_veterancy_metadata(&mut template, definition);
        Self::apply_authored_eject_pilot_die_metadata(&mut template, definition);
        Self::apply_authored_rebuild_hole_expose_die_metadata(&mut template, definition);
        Self::apply_authored_hack_internet_metadata(&mut template, definition);
        Self::apply_authored_special_power_module_metadata(&mut template, definition);
        Self::apply_authored_hacker_disable_building_metadata(&mut template, definition);
        Self::apply_authored_charge_plant_metadata(&mut template, definition);

        Self::apply_authored_overcharge_metadata(&mut template, definition);
        Self::apply_authored_power_plant_update_metadata(&mut template, definition);
        Self::apply_authored_temporary_weapon_behavior_metadata(&mut template, definition);
        Self::apply_authored_physics_behavior_metadata(&mut template, definition);
        Self::apply_authored_geometry(&mut template, definition);
        if let Some(sx) = Self::object_definition_attr(definition, "ShadowSizeX")
            .and_then(|v| v.parse::<f32>().ok())
        {
            template.shadow_size_x = sx;
        }
        if let Some(sy) = Self::object_definition_attr(definition, "ShadowSizeY")
            .and_then(|v| v.parse::<f32>().ok())
        {
            template.shadow_size_y = sy;
        }
        if let Some(shadow) = Self::object_definition_attr(definition, "Shadow") {
            template.shadow_type = crate::game_logic::host_enum_table_residual::parse_shadow_type_bits(shadow.as_str());
        }
        if let Some(ox) = Self::object_definition_attr(definition, "ShadowOffsetX")
            .and_then(|v| v.parse::<f32>().ok())
        {
            template.shadow_offset_x = ox;
        }
        if let Some(oy) = Self::object_definition_attr(definition, "ShadowOffsetY")
            .and_then(|v| v.parse::<f32>().ok())
        {
            template.shadow_offset_y = oy;
        }
        if let Some(tex) = Self::object_definition_attr(definition, "ShadowTexture") {
            let tex = tex.trim();
            if !tex.is_empty() && !tex.eq_ignore_ascii_case("none") {
                template.shadow_texture = Some(tex.to_string());
            }
        }




        let primary_texture = texture_hint.or_else(|| definition.get_primary_texture());
        if let Some(texture_name) = primary_texture {
            let texture_name = texture_name.trim();
            if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                template.texture_name = Some(texture_name.to_string());
            }
        }

        // Retail SupplyDock/SupplyPile carry SUPPLY_SOURCE (not "resource")
        // KindOf bits.  These are exact token comparisons: `HARVESTER`
        // denotes a collector unit and a template basename containing
        // "Supply" must never turn either object into a supply source.
        let has_kind = |token: &str| {
            kind_of
                .split(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, ',' | '|')
                })
                .any(|candidate| candidate.eq_ignore_ascii_case(token))
        };
        let is_supply_source = has_kind("supply_source");
        // Resource/Harvestable are existing host bridge kinds.  They remain
        // valid only when explicitly authored as a token; the retail
        // SUPPLY_SOURCE capability feeds both so the frozen Gather path does
        // not need a second template-name rule.
        let is_resource = is_supply_source || has_kind("resource") || has_kind("harvestable");
        // An authored Object INI is authoritative for object classification.
        // `should_spawn_fallback_template` is only a policy for unresolved map
        // objects; applying its broad name filter here turned movable retail
        // objects such as ChinaVehicleSupplyTruck into static structures just
        // because their names contain "supply".
        let is_structure = has_kind("structure") || has_kind("immobile");

        // Capture is not a generic structure/infantry feature.  Preserve the
        // exact KindOf and Behavior-module inputs that C++ ActionManager uses
        // so physical RMB classification and authority never need a template
        // name fallback.
        Self::apply_authored_capture_metadata(&mut template, &kind_of, definition);

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        }
        Self::apply_authored_semantic_kind_bits(&mut template, &kind_of);
        if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
        }
        // The dock module, not a spelling convention, identifies a drop-off
        // building.  This also lets preferred-dock selection recognize retail
        // faction variants whose template name is not a host bootstrap name.
        if template.dock_kind == crate::game_logic::DockKind::SupplyCenter {
            template.add_kind_of(KindOf::SupplyCenter);
        }
        if kind_of.contains("selectable") || is_structure {
            template.add_kind_of(KindOf::Selectable);
        }
        if kind_of.contains("powered") {
            template.add_kind_of(KindOf::Powered);
        }
        // Wave 982: C++ KINDOF_IGNORED_IN_GUI residual.
        if kind_of.contains("ignored_in_gui") || kind_of.contains("ignoredingui") {
            template.add_kind_of(KindOf::IgnoredInGui);
        }
        Self::add_faction_structure_kind_bits(&mut template, &kind_of);

        if lower.contains("commandcenter") {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter);
        }
        // Faction drop-off buildings only — not map SupplyDock/SupplyPile sources.
        if is_structure
            && !is_resource
            && (lower.contains("supplycenter")
                || lower.contains("supplystash")
                || lower.contains("supplydropzone")
                || lower == "supplycenter")
        {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        if template.max_health <= 1.0 {
            template.set_health(if is_structure { 1200.0 } else { 250.0 });
        }

        // C++ ThingTemplate ctor zeros m_experienceValues[LEVEL_COUNT].
        // Parse ExperienceValue only when authored — do not invent 50/100.
        if let Some(xp) = Self::object_definition_attr(definition, "experiencevalue") {
            let vals: Vec<f32> = xp
                .split_whitespace()
                .filter_map(|s| s.parse::<f32>().ok())
                .collect();
            if !vals.is_empty() {
                template.experience_value = vals[0];
                let mut table = [vals[0]; 4];
                for (i, v) in vals.iter().take(4).enumerate() {
                    table[i] = *v;
                }
                template.experience_values = table;
            }
        }

        if let Some(sp) = Self::object_definition_attr(definition, "skillpointvalue") {
            use crate::game_logic::host_rank_ui_residual::USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL;
            let mut table = [USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL; 4];
            for (i, tok) in sp.split_whitespace().take(4).enumerate() {
                if tok.eq_ignore_ascii_case("USE_EXP_VALUE") || tok == "-999" {
                    table[i] = USE_EXP_VALUE_FOR_SKILL_VALUE_RESIDUAL;
                } else if let Ok(v) = tok.parse::<i32>() {
                    table[i] = v;
                }
            }
            template.skill_point_values = table;
        }

        if let Some(req) = Self::object_definition_attr(definition, "experiencerequired") {
            let vals: Vec<f32> = req
                .split_whitespace()
                .filter_map(|s| s.parse::<f32>().ok())
                .collect();
            // C++ list is [Regular, Veteran, Elite, Heroic]; host thresholds
            // are [Veteran, Elite, Heroic].
            if vals.len() >= 4 {
                template.veterancy_xp_thresholds = [vals[1], vals[2], vals[3]];
            } else if vals.len() == 3 {
                template.veterancy_xp_thresholds = [vals[0], vals[1], vals[2]];
            }
        }

        if let Some(trainable) = Self::object_definition_attr(definition, "istrainable") {
            template.is_trainable = matches!(
                trainable.trim().to_ascii_lowercase().as_str(),
                "yes" | "true" | "1"
            );
        }

        if let Some(enter_guard) = Self::object_definition_attr(definition, "enterguard") {
            template.enter_guard = matches!(
                enter_guard.trim().to_ascii_lowercase().as_str(),
                "yes" | "true" | "1"
            );
        }
        if let Some(hijack_guard) = Self::object_definition_attr(definition, "hijackguard") {
            template.hijack_guard = matches!(
                hijack_guard.trim().to_ascii_lowercase().as_str(),
                "yes" | "true" | "1"
            );
        }

        // C++ ThingTemplate.cpp:151-155 UpgradeCameo1..5 → m_upgradeCameoUpgradeNames.
        for (i, key) in [
            "upgradecameo1",
            "upgradecameo2",
            "upgradecameo3",
            "upgradecameo4",
            "upgradecameo5",
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(name) = Self::object_definition_attr(definition, key) {
                let trimmed = name.trim();
                if !trimmed.is_empty() {
                    template.upgrade_cameo_names[i] = trimmed.to_string();
                }
            }
        }

        Self::apply_authored_veterancy_gain_create(&mut template, definition);
        Self::apply_authored_create_modules(&mut template, definition);

        // C++ parity: parse Armor from INI (default 0).
        if let Some(armor_val) = Self::object_definition_attr(definition, "armor")
            .and_then(|s| s.trim().parse::<f32>().ok())
        {
            template.armor = armor_val;
        }

        template.armor_sets = definition
            .armor_sets
            .iter()
            .map(|set| {
                let mut conditions = 0u8;
                for token in &set.conditions {
                    if let Some(bit) =
                        crate::game_logic::host_armor_residual::armor_set_condition_bit(token)
                    {
                        conditions |= bit;
                    }
                }
                crate::game_logic::HostArmorSet {
                    conditions,
                    armor: set.armor.clone(),
                    damage_fx: set.damage_fx.clone(),
                }
            })
            .collect();
        if let Some(cap) = definition.subdual_damage_cap.filter(|cap| cap.is_finite()) {
            template.subdual_damage_cap = cap.max(0.0);
        }
        if let Some(rate) = definition.subdual_heal_rate_frames {
            template.subdual_heal_rate_frames = rate;
        }
        if let Some(amount) = definition.subdual_heal_amount.filter(|amount| amount.is_finite()) {
            template.subdual_heal_amount = amount.max(0.0);
        }

        // C++ ThingTemplate.cpp:100 INI::parseReal VisionRange. 0 is authored.
        if let Some(sight) = Self::object_definition_attr(definition, "visionrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
        {
            template.sight_range = sight;
        }

        // C++ Object.cpp doShroudReveal uses getShroudClearingRange, not VisionRange.
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

        if let Some(raw) = Self::object_definition_attr(definition, "radarpriority") {
            template.radar_priority = match raw.trim().to_ascii_uppercase().as_str() {
                "NOT_ON_RADAR" => 1,
                "STRUCTURE" => 2,
                "UNIT" => 3,
                "LOCAL_UNIT_ONLY" => 4,
                _ => 0,
            };
        }

        // C++ ThingTemplate.cpp:226-227 INI::parseUnsignedByte CrusherLevel/CrushableLevel.
        // 0 is authored (cannot crush / most crushable); do not treat as missing.
        if let Some(level) = Self::object_definition_attr(definition, "crusherlevel")
            .and_then(|s| s.trim().parse::<u8>().ok())
        {
            template.crusher_level = level;
        }
        if let Some(level) = Self::object_definition_attr(definition, "crushablelevel")
            .and_then(|s| s.trim().parse::<u8>().ok())
        {
            template.crushable_level = level;
        }

        // C++ parity: parse BuildCost from INI.
        if let Some(cost) = Self::object_definition_attr(definition, "buildcost")
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|&v| v > 0)
        {
            template.build_cost.supplies = cost;
        }

        // C++ ThingTemplate parses `RefundValue` as an unsigned short.  Zero
        // is not missing data: it explicitly retains the normal
        // BuildCost × SellPercentage refund calculation.
        if let Some(refund_value) = Self::object_definition_attr(definition, "refundvalue") {
            template.refund_value = refund_value.trim().parse::<u16>().unwrap_or_else(|_| {
                panic!(
                    "Object INI RefundValue for '{}' must be an unsigned short (0..=65535), got {:?}",
                    template_name, refund_value
                )
            });
        }

        // C++ Object INI BuildTime is expressed in logic seconds.  Preserve
        // the authored value instead of letting catalogue-seeded units use
        // ThingTemplate's one-second constructor default.
        if let Some(build_time) = Self::object_definition_attr(definition, "buildtime")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
        {
            template.build_time = build_time;
        }

        // C++ `Weapon = ...` lives inside an outer WeaponSet block.  Resolve
        // only its no-flag row for normal combat; a conditional row must not
        // overwrite PRIMARY just because it appeared later in the INI.
        let has_base_weapon_set = definition
            .weapon_sets
            .iter()
            .any(|set| set.is_unconditional());
        if let Some(wname) = definition.base_weapon_name(0) {
            template.set_primary_weapon_name(wname);
        } else if has_base_weapon_set {
            template.set_primary_weapon_none();
        }

        // Secondary and tertiary names come from the same selected no-flag
        // WeaponSet.  No raw-attribute scan is allowed here because it loses
        // both the slot and the condition owning a repeated `Weapon` field.
        if let Some(wname) = definition.base_weapon_name(1) {
            template.set_secondary_weapon_name(wname);
        }
        if let Some(wname) = definition.base_weapon_name(2) {
            template.set_tertiary_weapon_name(wname);
        }

        // Bounded C++ `USES_MINE_CLEARING_WEAPONSET` support: retain only an
        // exact single-condition MINE_CLEARING_DETAIL primary.  It remains a
        // separate object weapon instance until the parsed button arms it.
        if let Some(wname) = definition.mine_clearing_primary_weapon_name() {
            template.set_mine_clearing_primary_weapon_name(wname);
        }

        Self::apply_authored_weapon_set_create_policy(&mut template, definition);

        // SET_NORMAL Locomotor name from Object INI when present; else known host map.
        // A RiderChange container needs the unambiguous source SET_NORMAL
        // primary that was validated with its roster above.  It must never
        // inherit the last raw outer Locomotor row (normally SET_SLUGGISH on
        // a Combat Bike) merely because the legacy attributes map is lossy.
        if definition_has_rider_change_contain(definition) {
            if let Some(lname) = rider_change_normal_locomotors
                .as_deref()
                .and_then(|names| names.first())
            {
                template.set_locomotor_name(lname);
            }
        } else if let Some(raw) = Self::object_definition_attr(definition, "locomotor") {
            // Formats: "SET_NORMAL BasicHumanLocomotor" or "SET_NORMAL A B" (take first).
            let mut parts = raw.split_whitespace();
            let first = parts.next().unwrap_or("");
            let loco = if first.eq_ignore_ascii_case("SET_NORMAL")
                || first.eq_ignore_ascii_case("SET_NORMAL_UPGRADED")
                || first.eq_ignore_ascii_case("SET_PANIC")
                || first.eq_ignore_ascii_case("SET_TAXIING")
                || first.eq_ignore_ascii_case("SET_FREEFALL")
            {
                parts.next()
            } else if !first.is_empty() {
                Some(first)
            } else {
                None
            };
            if let Some(lname) = loco {
                template.set_locomotor_name(lname);
            }
        } else if let Some(lname) =
            super::super::locomotor_bootstrap::locomotor_name_for_unit(template_name)
        {
            template.set_locomotor_name(lname);
        }
        apply_locomotor_set_names_from_definition(
            &mut template,
            rider_change_normal_locomotors.as_deref(),
            Self::object_definition_attr(definition, "locomotor").as_deref(),
        );



        // Combat unit KindOf from object type / kindof string so store weapons can attach.
        let otype = definition.object_type.to_ascii_lowercase();
        if otype.contains("infantry") || kind_of.contains("infantry") {
            template
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable);
        }
        if otype.contains("vehicle") || kind_of.contains("vehicle") {
            template
                .add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable);
        }
        if otype.contains("aircraft") || kind_of.contains("aircraft") {
            template
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::Selectable);
        }

        crate::game_logic::host_move_ambient_audio::apply_authored_audio_events(
            &mut template,
            definition,
        );


        template
    }

    /// Apply the gameplay-relevant Object INI KindOf capabilities which are
    /// safe to enrich on an existing hand-authored host template.  Starter
    /// templates retain their bespoke Rust behavior, but they must not erase
    /// exact C++ targetability, collector, or projectile identity from the
    /// retail definition that describes that same object.
    fn apply_authored_semantic_kind_bits(template: &mut ThingTemplate, kind_of: &str) {
        let has_kind = |token: &str| {
            kind_of
                .split(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, ',' | '|')
                })
                .any(|candidate| candidate.eq_ignore_ascii_case(token))
        };

        if has_kind("dozer") {
            template.add_kind_of(KindOf::Dozer);
        }
        if has_kind("harvester") {
            template.add_kind_of(KindOf::Harvester);
        }
        // ActionManager uses these exact authored KindOf bits for service
        // target validation.  Do not collapse them into BuildingType or
        // a template-name convention: retail barracks/war factories happen
        // to carry the tags, but a similarly named arbitrary object does not.
        if has_kind("repair_pad") {
            template.add_kind_of(KindOf::RepairPad);
        }
        if has_kind("heal_pad") {
            template.add_kind_of(KindOf::HealPad);
        }
        // `InternetHackContain` accepts C++ `MONEY_HACKER`, not generic
        // infantry and not an identity inferred from a Hacker-ish basename.
        if has_kind("money_hacker") {
            template.add_kind_of(KindOf::MoneyHacker);
        }
        // C++ SupplyDock/SupplyPile authority.  This must remain distinct
        // from Resource/Harvestable: crates and host test fixtures may carry
        // those bridge kinds without becoming construction-exclusion or dock
        // supply sources.
        if has_kind("supply_source") {
            template.add_kind_of(KindOf::SupplySource);
        }
        if has_kind("cannot_build_near_supplies") {
            template.add_kind_of(KindOf::CannotBuildNearSupplies);
        }
        if has_kind("unattackable") {
            template.add_kind_of(KindOf::Unattackable);
        }
        // Preserve the authored C++ WeaponSet victim categories.  They are
        // gameplay-only and let parsed AntiProjectile/AntiMine/etc. weapons
        // make the same target decision as retail rather than being collapsed
        // into broad host air/ground booleans.
        if has_kind("mine") {
            template.add_kind_of(KindOf::Mine);
        }
        if has_kind("demotrap") {
            template.add_kind_of(KindOf::DemoTrap);
        }
        if has_kind("small_missile") {
            template.add_kind_of(KindOf::SmallMissile);
        }
        if has_kind("ballistic_missile") {
            template.add_kind_of(KindOf::BallisticMissile);
        }
        // C++ treats every small/ballistic missile as a projectile too.  The
        // more-specific categories above win in WeaponSet victim-mask
        // selection, while ordinary PROJECTILE objects keep their own
        // AntiProjectile route.
        if has_kind("projectile") || has_kind("small_missile") || has_kind("ballistic_missile") {
            template.add_kind_of(KindOf::Projectile);
        }
        if has_kind("parachute") {
            template.add_kind_of(KindOf::Parachute);
        }
        if has_kind("disguiser") {
            template.add_kind_of(KindOf::Disguiser);
        }
        if has_kind("crate") {
            template.add_kind_of(KindOf::Crate);
        }
        if has_kind("ignores_select_all") {
            template.add_kind_of(KindOf::IgnoresSelectAll);
        }
        if has_kind("salvager") {
            template.add_kind_of(KindOf::Salvager);
        }
        // C++ KindOf.h:63-67,86 — authored bits EVA/victory/selection consume.
        if has_kind("always_selectable") {
            template.add_kind_of(KindOf::AlwaysSelectable);
        }
        if has_kind("mp_count_for_victory") {
            template.add_kind_of(KindOf::MpCountForVictory);
        }
        if has_kind("score") {
            template.add_kind_of(KindOf::Score);
        }
        if has_kind("score_create") {
            template.add_kind_of(KindOf::ScoreCreate);
        }
        if has_kind("score_destroy") {
            template.add_kind_of(KindOf::ScoreDestroy);
        }
        if has_kind("no_garrison") {
            template.add_kind_of(KindOf::NoGarrison);
        }
        if has_kind("stealth_garrison") {
            template.add_kind_of(KindOf::StealthGarrison);
        }
        if has_kind("tech_building") {
            template.add_kind_of(KindOf::TechBuilding);
        }
        if has_kind("garrisonable_until_destroyed") {
            template.add_kind_of(KindOf::GarrisonableUntilDestroyed);
        }

        if has_kind("drone") {
            template.add_kind_of(KindOf::Drone);
        }
        if has_kind("boat") {
            template.add_kind_of(KindOf::Boat);
        }
        if has_kind("transport") {
            template.add_kind_of(KindOf::Transport);
        }
        if has_kind("immune_to_capture") {
            template.add_kind_of(KindOf::ImmuneToCapture);
            template.immune_to_capture = true;
        }
        if has_kind("defensive_wall") {
            template.add_kind_of(KindOf::DefensiveWall);
        }
        if has_kind("walk_on_top_of_wall") {
            template.add_kind_of(KindOf::WalkOnTopOfWall);
        }
        if has_kind("bridge") {
            template.add_kind_of(KindOf::Bridge);
        }
        if has_kind("landmark_bridge") {
            template.add_kind_of(KindOf::LandmarkBridge);
            template.add_kind_of(KindOf::Bridge);
        }
        if has_kind("bridge_tower") {
            template.add_kind_of(KindOf::BridgeTower);
        }
        if has_kind("can_see_through") || has_kind("can_see_through_structure") {
            template.add_kind_of(KindOf::CanSeeThrough);
        }
        if has_kind("reveal_to_all") {
            template.reveal_to_all = true;
        }
        if has_kind("always_visible") {
            template.always_visible = true;
        }
        // C++ KINDOF_AUTO_RALLYPOINT — factory empty-ground SET_RALLY_POINT.
        if has_kind("auto_rallypoint") || has_kind("auto_rally_point") {
            template.add_kind_of(KindOf::AutoRallypoint);
        }
        if has_kind("mob_nexus") {
            template.add_kind_of(KindOf::MobNexus);
        }
        // C++ KINDOF_NO_COLLIDE — PartitionData::collidesWith is FALSE.
        if has_kind("no_collide") {
            template.add_kind_of(KindOf::NoCollide);
        }
        // C++ KINDOF_FORCEATTACKABLE — civ fences / cargo planes pickable
        // via force-attack and hover even when not Selectable.
        if has_kind("forceattackable") || has_kind("force_attackable") {
            template.add_kind_of(KindOf::ForceAttackable);
        }
        if has_kind("shrubbery") {
            template.add_kind_of(KindOf::Shrubbery);
        }
        if has_kind("cleared_by_build") || has_kind("clearedbybuild") {
            template.add_kind_of(KindOf::ClearedByBuild);
        }
        if has_kind("inert") {
            template.add_kind_of(KindOf::Inert);
        }
    }


    /// Preserve the exact DockUpdate and normal-containment slice that the
    /// physical RMB path needs from Object INI Behavior declarations.  C++
    /// `ActionManager::canEnterObject` asks for a `ContainModuleInterface`,
    /// so a Vehicle KindOf or a template basename must never fabricate one.
    fn apply_authored_dock_and_contain_modules(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
        rider_change_normal_locomotors: Option<&[String]>,
    ) {
        use crate::game_logic::{
            ContainAdmission, ContainModuleKind, ContainModuleMetadata, DockKind,
            RiderChangeRiderMetadata,
        };

        fn parse_bool(value: &str) -> Option<bool> {
            match value.trim().to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        fn parse_contain_audio_event(
            module: &crate::assets::BehaviorModuleDefinition,
            key: &str,
        ) -> String {
            module
                .attribute(key)
                .map(str::trim)
                .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("NONE"))
                .unwrap_or("")
                .to_string()
        }


        /// Decode only the mobile-kind masks represented by the live Rust
        /// object model.  A mask that needs a missing kind is fail-closed;
        /// this is preferable to accepting a tank in an infantry-only cabin.
        fn parse_admission(module: &crate::assets::BehaviorModuleDefinition) -> ContainAdmission {
            fn tokenize(raw: &str) -> Vec<&str> {
                raw.split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ',' | '|'))
                    .filter(|token| !token.is_empty())
                    .collect()
            }
            if let Some(raw) = module.attribute("AllowInsideKindOf") {
                let tokens = tokenize(raw);
                if tokens
                    .iter()
                    .any(|token| token.eq_ignore_ascii_case("MONEY_HACKER"))
                {
                    // Main can faithfully admit this exact one-kind mask.
                    // Any combined/forbidden custom mask needs C++'s full
                    // KindOf algebra and remains unavailable rather than
                    // widening Internet Center entry.
                    return if tokens.len() == 1
                        && tokens[0].eq_ignore_ascii_case("MONEY_HACKER")
                        && module.attribute("ForbidInsideKindOf").is_none()
                    {
                        ContainAdmission::MoneyHackerOnly
                    } else {
                        ContainAdmission::Unsupported
                    };
                }
            }
            let mut allowed = [true, true, true]; // infantry, vehicle, aircraft
            if let Some(raw) = module.attribute("AllowInsideKindOf") {
                allowed = [false, false, false];
                for token in raw
                    .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ',' | '|'))
                    .filter(|token| !token.is_empty())
                {
                    match token.to_ascii_uppercase().as_str() {
                        "INFANTRY" => allowed[0] = true,
                        "VEHICLE" => allowed[1] = true,
                        "AIRCRAFT" => allowed[2] = true,
                        // Rust has no portable-structure source path and
                        // normal Enter independently rejects structures.
                        "PORTABLE_STRUCTURE" | "STRUCTURE" => {}
                        "ALL" => allowed = [true, true, true],
                        _ => return ContainAdmission::Unsupported,
                    }
                }
            }
            if let Some(raw) = module.attribute("ForbidInsideKindOf") {
                for token in raw
                    .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, ',' | '|'))
                    .filter(|token| !token.is_empty())
                {
                    match token.to_ascii_uppercase().as_str() {
                        "INFANTRY" => allowed[0] = false,
                        "VEHICLE" => allowed[1] = false,
                        "AIRCRAFT" => allowed[2] = false,
                        // These kinds are already rejected by normal Enter.
                        "PORTABLE_STRUCTURE" | "STRUCTURE" => {}
                        _ => return ContainAdmission::Unsupported,
                    }
                }
            }

            match allowed {
                [true, true, true] => ContainAdmission::AnyMobile,
                [true, false, false] => ContainAdmission::InfantryOnly,
                [true, true, false] => ContainAdmission::InfantryOrVehicle,
                _ => ContainAdmission::Unsupported,
            }
        }

        fn parse_slots(
            module: &crate::assets::BehaviorModuleDefinition,
            name: &str,
        ) -> Option<usize> {
            module
                .attribute(name)
                .and_then(|value| value.trim().parse::<i64>().ok())
                .and_then(|value| usize::try_from(value).ok())
        }

        /// C++ `INI::parseDurationUnsignedInt`: scan the unsigned numeric
        /// prefix as milliseconds, then round up to 30 Hz logic frames.  Do
        /// not adopt Rust duration suffix semantics: C++ treats `1s` as one
        /// millisecond because `sscanf("%u")` stops at the suffix.
        fn parse_duration_frames(value: &str) -> Option<u32> {
            let digits: String = value
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            let milliseconds = digits.parse::<u64>().ok()?;
            let frames = milliseconds.checked_mul(30)?.checked_add(999)? / 1_000;
            u32::try_from(frames).ok()
        }

        /// Retain every representable C++ RiderChange token.  This parser
        /// intentionally does *not* infer rider class from its template name:
        /// the slot is only live when the authored template identity and its
        /// complete record are available.
        fn parse_rider_change(
            module: &crate::assets::BehaviorModuleDefinition,
            definition: &ObjectDefinition,
            rider_change_normal_locomotors: Option<&[String]>,
        ) -> (Vec<RiderChangeRiderMetadata>, Option<u32>, String, u128) {
            let mut riders = Vec::new();
            // C++ chooses a SET_NORMAL member by terrain surface.  The host
            // keeps one active movement profile, so accept the full authored
            // row only when every distinct-surface member resolves and has
            // identical represented behavior.  A partial/ambiguous set is
            // retained below but never becomes a physical Enter capability.
            let normal_locomotor_binding = rider_change_normal_locomotors.and_then(
                crate::game_logic::locomotor_bootstrap::resolve_uniform_host_locomotor_set,
            );
            let sluggish_names = unambiguous_locomotors_for_set(definition, "SET_SLUGGISH");
            let sluggish_locomotor_binding = sluggish_names.as_deref().and_then(
                crate::game_logic::locomotor_bootstrap::resolve_uniform_host_locomotor_set,
            );

            for slot in 1u8..=8 {
                let key = format!("Rider{slot}");
                let Some(raw) = module.attribute(&key) else {
                    continue;
                };
                let fields: Vec<&str> = raw.split_whitespace().collect();
                // C++ parseRiderInfo consumes exactly six positional fields.
                // A malformed/custom line must not silently shift a command
                // set or locomotor token into another semantic slot.
                if fields.len() != 6 || fields.iter().any(|field| field.is_empty()) {
                    continue;
                }
                let model_condition = fields[1].to_string();
                let object_status = fields[3].to_string();
                let model_condition_mask =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        &model_condition,
                    )
                    .filter(|bit| *bit < 128)
                    .map(|bit| 1u128 << bit)
                    .unwrap_or(0);
                let object_status_mask =
                    crate::game_logic::host_enum_table_residual::object_status_bit_name_index(
                        &object_status,
                    )
                    .filter(|bit| *bit < 64)
                    .map(|bit| 1u64 << bit)
                    .unwrap_or(0);
                let weapon_set = fields[2].to_string();
                let command_set = fields[4].to_string();
                let locomotor_set = fields[5].to_string();
                // The active bounded bridge can apply the retail Combat Cycle
                // weapon table only by its authored RiderN index.  Do not
                // accept arbitrary WeaponSet spellings or an unimplemented
                // eighth weapon slot as if they had a host weapon mapping.
                let expected_weapon_set = format!("WEAPON_RIDER{slot}");
                let expected_model_condition = format!("RIDER{slot}");
                let expected_object_status = format!("STATUS_RIDER{slot}");
                let set_binding = if locomotor_set.eq_ignore_ascii_case("SET_NORMAL") {
                    normal_locomotor_binding.as_ref()
                } else if locomotor_set.eq_ignore_ascii_case("SET_SLUGGISH") {
                    sluggish_locomotor_binding.as_ref()
                } else {
                    None
                };
                let (active_locomotor_name, active_locomotor_names, active_locomotor_surfaces) =
                    set_binding
                        .map(|binding| {
                            (
                                Some(binding.representative_name.clone()),
                                binding.locomotor_names.clone(),
                                binding.locomotor_surfaces,
                            )
                        })
                        .unwrap_or((None, Vec::new(), 0));
                let physical_enter_supported = slot <= 7
                    && model_condition_mask != 0
                    && object_status_mask != 0
                    && model_condition.eq_ignore_ascii_case(&expected_model_condition)
                    && weapon_set.eq_ignore_ascii_case(&expected_weapon_set)
                    && object_status.eq_ignore_ascii_case(&expected_object_status)
                    && !command_set.is_empty()
                    && set_binding.is_some();

                riders.push(RiderChangeRiderMetadata {
                    slot,
                    template_name: fields[0].to_string(),
                    model_condition,
                    weapon_set,
                    object_status,
                    command_set,
                    locomotor_set,
                    active_locomotor_name,
                    active_locomotor_names,
                    active_locomotor_surfaces,
                    model_condition_mask,
                    object_status_mask,
                    physical_enter_supported,
                });
            }

            let scuttle_delay_frames = match module.attribute("ScuttleDelay") {
                Some(value) => parse_duration_frames(value),
                // C++ RiderChangeContain defaults to zero frames.
                None => Some(0),
            };
            let scuttle_status = module
                .attribute("ScuttleStatus")
                .unwrap_or("TOPPLED")
                .trim()
                .to_string();
            let scuttle_status_mask =
                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                    &scuttle_status,
                )
                .filter(|bit| *bit < 128)
                .map(|bit| 1u128 << bit)
                .unwrap_or(0);
            (
                riders,
                scuttle_delay_frames,
                scuttle_status,
                scuttle_status_mask,
            )
        }

        let mut dock_kind = DockKind::None;
        template.dock_starting_boxes = None;
        template.dock_delete_when_empty = false;
        template.railed_transport_slots = None;
        template.railed_path_prefix_name = definition
            .behavior_modules
            .iter()
            .find(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("RailedTransportAIUpdate")
            })
            .and_then(|module| module.attribute("PathPrefixName"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default();

        template.contain_module = ContainModuleMetadata::default();
        template.transport_slot_count =
            Self::object_definition_attr(definition, "transportslotcount")
                .and_then(|value| value.trim().parse::<i64>().ok())
                .and_then(|value| usize::try_from(value).ok());
        let mut parsed_contain: Option<ContainModuleMetadata> = None;
        for module in &definition.behavior_modules {
            let candidate = if module
                .class_name
                .eq_ignore_ascii_case("SupplyCenterDockUpdate")
            {
                Some(DockKind::SupplyCenter)
            } else if module
                .class_name
                .eq_ignore_ascii_case("SupplyWarehouseDockUpdate")
            {
                Some(DockKind::SupplyWarehouse)
            } else if module
                .class_name
                .eq_ignore_ascii_case("RailedTransportDockUpdate")
            {
                Some(DockKind::RailedTransport)
            } else {
                None
            };

            if let Some(candidate) = candidate {
                // Multiple different dock interfaces would be ambiguous in
                // this compact host mapping.  Retail objects use one; reject
                // malformed/custom combinations rather than guessing priority.
                if dock_kind != DockKind::None && dock_kind != candidate {
                    template.dock_kind = DockKind::None;
                    template.dock_starting_boxes = None;
                    return;
                }
                dock_kind = candidate;
                if candidate == DockKind::SupplyWarehouse {
                    template.dock_starting_boxes = module
                        .attribute("StartingBoxes")
                        .and_then(|value| value.trim().parse::<i64>().ok())
                        .and_then(|value| u32::try_from(value).ok());
                    template.dock_delete_when_empty =
                        module.attribute("DeleteWhenEmpty").is_some_and(|value| {
                            matches!(
                                value.trim().to_ascii_lowercase().as_str(),
                                "yes" | "true" | "1"
                            )
                        });
                }
            }

            let kind_and_slots = if module.class_name.eq_ignore_ascii_case("TransportContain")
                || module.class_name.eq_ignore_ascii_case("HelixContain")
            {
                Some((ContainModuleKind::Transport, parse_slots(module, "Slots")))
            } else if module.class_name.eq_ignore_ascii_case("RiderChangeContain") {
                Some((ContainModuleKind::RiderChange, parse_slots(module, "Slots")))
            } else if module
                .class_name
                .eq_ignore_ascii_case("RailedTransportContain")
            {
                let slots = parse_slots(module, "Slots");
                template.railed_transport_slots = slots;
                Some((ContainModuleKind::RailedTransport, slots))
            } else if module.class_name.eq_ignore_ascii_case("GarrisonContain") {
                Some((
                    ContainModuleKind::Garrison,
                    parse_slots(module, "ContainMax"),
                ))
            } else if module
                .class_name
                .eq_ignore_ascii_case("InternetHackContain")
            {
                Some((
                    ContainModuleKind::InternetHack,
                    parse_slots(module, "Slots"),
                ))
            } else if module.class_name.eq_ignore_ascii_case("HealContain") {
                Some((
                    ContainModuleKind::Heal,
                    parse_slots(module, "ContainMax"),
                ))
            } else if module.class_name.eq_ignore_ascii_case("CaveContain") {
                Some((
                    ContainModuleKind::Cave,
                    parse_slots(module, "ContainMax"),
                ))
            } else if module.class_name.eq_ignore_ascii_case("TunnelContain") {
                Some((
                    ContainModuleKind::Tunnel,
                    parse_slots(module, "ContainMax"),
                ))
            } else {
                None
            };

            if let Some((kind, slots)) = kind_and_slots {
                let (
                    rider_change_riders,
                    rider_change_scuttle_delay_frames,
                    rider_change_scuttle_status,
                    rider_change_scuttle_status_mask,
                ) = if kind == ContainModuleKind::RiderChange {
                    parse_rider_change(module, definition, rider_change_normal_locomotors)
                } else {
                    (Vec::new(), None, String::new(), 0)
                };

                let frames_for_full_heal = match module.attribute("TimeForFullHeal") {
                    Some(value) => parse_duration_frames(value),
                    None if kind == ContainModuleKind::Heal => Some(0),
                    None if kind == ContainModuleKind::Tunnel => Some(1),
                    None => None,
                };
                let immune_to_clear_building_attacks = if kind == ContainModuleKind::Garrison {
                    module
                        .attribute("ImmuneToClearBuildingAttacks")
                        .and_then(parse_bool)
                        .unwrap_or(false)
                } else {
                    false
                };
                let is_enclosing_container = if kind == ContainModuleKind::Garrison {
                    module
                        .attribute("IsEnclosingContainer")
                        .and_then(parse_bool)
                        .unwrap_or(true)
                } else {
                    true
                };
                let cave_index = if kind == ContainModuleKind::Cave {
                    module
                        .attribute("CaveIndex")
                        .and_then(|v| v.trim().parse::<i32>().ok())
                        .unwrap_or(0)
                } else {
                    0
                };
                let (heal_objects, initial_roster_template, initial_roster_count) =
                    if kind == ContainModuleKind::Garrison {
                        let heal_objects = module
                            .attribute("HealObjects")
                            .and_then(parse_bool)
                            .unwrap_or(false);
                        let roster = module
                            .attribute("InitialRoster")
                            .map(|raw| raw.split_whitespace().collect::<Vec<_>>())
                            .and_then(|tokens| {
                                gamelogic::object::contain::InitialRoster::parse_from_tokens(
                                    &tokens,
                                )
                                .ok()
                            })
                            .unwrap_or_default();
                        (heal_objects, roster.template_name, roster.count)
                    } else {
                        (false, String::new(), 0)
                    };
                let candidate = ContainModuleMetadata {
                    kind,
                    slots,
                    admission: parse_admission(module),
                    allow_allies_inside: module
                        .attribute("AllowAlliesInside")
                        .and_then(parse_bool)
                        .unwrap_or(true),
                    allow_enemies_inside: module
                        .attribute("AllowEnemiesInside")
                        .and_then(parse_bool)
                        .unwrap_or(true),
                    allow_neutral_inside: module
                        .attribute("AllowNeutralInside")
                        .and_then(parse_bool)
                        .unwrap_or(true),
                    rider_change_riders,
                    rider_change_scuttle_delay_frames,
                    rider_change_scuttle_status,
                    rider_change_scuttle_status_mask,
                    frames_for_full_heal,
                    immune_to_clear_building_attacks,
                    is_enclosing_container,
                    cave_index,
                    heal_objects,
                    initial_roster_template,
                    initial_roster_count,
                    weapon_bonus_passed_to_passengers: false,
                    enter_sound: parse_contain_audio_event(module, "EnterSound"),
                    exit_sound: parse_contain_audio_event(module, "ExitSound"),
                };
                // Retail gives an object one active normal contain interface.
                // A malformed/custom stack is not safely representable here;
                // reject it rather than guessing declaration precedence.
                if parsed_contain.is_some() {
                    template.contain_module = ContainModuleMetadata::default();
                    template.railed_transport_slots = None;
                    return;
                }
                parsed_contain = Some(candidate);
            }
        }
        template.dock_kind = dock_kind;
        if let Some(contain) = parsed_contain {
            template.contain_module = contain;
        }
    }

    /// Retain the source-authored `ParkingPlaceBehaviorModuleData` needed by
    /// the host aircraft return/landing path.  A faction-building KindOf or
    /// an airfield-like template name never fabricates this behavior: without
    /// it Main has no C++ parking reservation contract to honor.
    fn apply_authored_parking_place_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::ParkingPlaceMetadata;

        fn parse_bool(value: &str) -> Option<bool> {
            match value.trim().to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        // `getPP` and ActionManager walk behavior modules in declaration
        // order and use the first ParkingPlace interface.  Preserve that
        // source order instead of merging malformed multiple modules into a
        // synthetic capacity.
        let Some(module) = definition.behavior_modules.iter().find(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("ParkingPlaceBehavior")
        }) else {
            template.parking_place = None;
            return;
        };

        let parse = || -> Option<ParkingPlaceMetadata> {
            // These are C++ module-data constructor defaults, not a retail
            // airfield fallback.  An explicitly malformed field invalidates
            // the whole behavior so player-facing landing fails closed.
            let num_rows = match module.attribute("NumRows") {
                Some(value) => value.trim().parse::<i32>().ok()?,
                None => 0,
            };
            let num_cols = match module.attribute("NumCols") {
                Some(value) => value.trim().parse::<i32>().ok()?,
                None => 0,
            };
            let approach_height = match module.attribute("ApproachHeight") {
                Some(value) => value.trim().parse::<f32>().ok()?,
                None => 0.0,
            };
            let landing_deck_height_offset = match module.attribute("LandingDeckHeightOffset") {
                Some(value) => value.trim().parse::<f32>().ok()?,
                None => 0.0,
            };
            let has_runways = match module.attribute("HasRunways") {
                Some(value) => parse_bool(value)?,
                None => false,
            };
            let park_in_hangars = match module.attribute("ParkInHangars") {
                Some(value) => parse_bool(value)?,
                None => false,
            };
            let heal_amount_per_second = match module.attribute("HealAmountPerSecond") {
                Some(value) => value.trim().parse::<f32>().ok()?,
                None => 0.0,
            };

            let metadata = ParkingPlaceMetadata {
                num_rows,
                num_cols,
                approach_height,
                landing_deck_height_offset,
                has_runways,
                park_in_hangars,
                heal_amount_per_second,
            };
            metadata.is_well_formed().then_some(metadata)
        };

        template.parking_place = parse();
    }

    /// Retain source-authored `FlightDeckBehaviorModuleData`.  A carrier
    /// KindOf or template basename never fabricates this behavior.
    fn apply_authored_flight_deck_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::FlightDeckMetadata;

        fn parse_duration_frames(value: &str) -> Option<u32> {
            let digits: String = value
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            let milliseconds = digits.parse::<u64>().ok()?;
            let frames = milliseconds.checked_mul(30)?.checked_add(999)? / 1_000;
            u32::try_from(frames).ok()
        }

        let Some(module) = definition.behavior_modules.iter().find(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("FlightDeckBehavior")
        }) else {
            template.flight_deck = None;
            return;
        };

        let parse = || -> Option<FlightDeckMetadata> {
            let num_rows = match module.attribute("NumSpacesPerRunway") {
                Some(value) => value.trim().parse::<i32>().ok()?,
                None => 0,
            };
            let num_cols = match module.attribute("NumRunways") {
                Some(value) => value.trim().parse::<i32>().ok()?,
                None => 0,
            };
            let approach_height = match module.attribute("ApproachHeight") {
                Some(value) => value.trim().parse::<f32>().ok()?,
                None => 0.0,
            };
            let landing_deck_height_offset = match module.attribute("LandingDeckHeightOffset") {
                Some(value) => value.trim().parse::<f32>().ok()?,
                None => 0.0,
            };
            let heal_amount_per_second = match module.attribute("HealAmountPerSecond") {
                Some(value) => value.trim().parse::<f32>().ok()?,
                None => 0.0,
            };
            let cleanup_frames = match module.attribute("ParkingCleanupPeriod") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let human_follow_frames = match module.attribute("HumanFollowPeriod") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let replacement_frames = match module.attribute("ReplacementDelay") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let dock_animation_frames = match module.attribute("DockAnimationDelay") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let launch_wave_frames = match module.attribute("LaunchWaveDelay") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let launch_ramp_frames = match module.attribute("LaunchRampDelay") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let lower_ramp_frames = match module.attribute("LowerRampDelay") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let catapult_fire_frames = match module.attribute("CatapultFireDelay") {
                Some(value) => parse_duration_frames(value)?,
                None => 0,
            };
            let payload_template = module
                .attribute("PayloadTemplate")
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            let catapult_system = [
                module
                    .attribute("Runway1CatapultSystem")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                module
                    .attribute("Runway2CatapultSystem")
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
            ];
            let metadata = FlightDeckMetadata {
                payload_template,
                num_rows,
                num_cols,
                approach_height,
                landing_deck_height_offset,
                heal_amount_per_second,
                cleanup_frames,
                human_follow_frames,
                replacement_frames,
                dock_animation_frames,
                launch_wave_frames,
                launch_ramp_frames,
                lower_ramp_frames,
                catapult_fire_frames,
                catapult_system,
            };
            metadata.is_well_formed().then_some(metadata)
        };

        template.flight_deck = parse();
    }


    /// Retain one exact C++ `DeployStyleAIUpdateModuleData` record from the
    /// Object INI.  A vehicle kind, command-set name, or template basename is
    /// never treated as deploy behavior: commands only acquire deploy
    /// authority when this concrete Behavior is present and well-formed.
    fn apply_authored_deploy_style_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::DeployStyleMetadata;

        fn parse_bool(value: &str) -> Option<bool> {
            match value.trim().to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        /// C++ `INI::scanUnsignedInt` consumes an unsigned numeric token and
        /// `parseDurationUnsignedInt` converts milliseconds to 30 Hz frames
        /// with ceil.  Reject malformed or overflowing custom values rather
        /// than clamping them into a different state-machine duration.
        fn parse_duration_frames(value: &str) -> Option<u32> {
            let digits: String = value
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            let milliseconds = digits.parse::<u64>().ok()?;
            let frames = milliseconds.checked_mul(30)?.checked_add(999)? / 1_000;
            u32::try_from(frames).ok()
        }

        let mut deploy_modules = definition.behavior_modules.iter().filter(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("DeployStyleAIUpdate")
        });
        let Some(module) = deploy_modules.next() else {
            template.deploy_style_metadata = None;
            return;
        };

        // `Object::getAI` exposes one active AI update.  A malformed/custom
        // object declaring two DeployStyle modules has no unambiguous compact
        // representation, so retain neither as player-facing authority.
        if deploy_modules.next().is_some() {
            template.deploy_style_metadata = None;
            return;
        }

        let parse = || -> Option<DeployStyleMetadata> {
            Some(DeployStyleMetadata {
                // These defaults are from DeployStyleAIUpdateModuleData's
                // constructor.  `0` stays distinct from an absent Behavior.
                pack_time_frames: match module.attribute("PackTime") {
                    Some(value) => parse_duration_frames(value)?,
                    None => 0,
                },
                unpack_time_frames: match module.attribute("UnpackTime") {
                    Some(value) => parse_duration_frames(value)?,
                    None => 0,
                },
                reset_turret_before_packing: match module.attribute("ResetTurretBeforePacking") {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
                turrets_function_only_when_deployed: match module
                    .attribute("TurretsFunctionOnlyWhenDeployed")
                {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
                turrets_must_center_before_packing: match module
                    .attribute("TurretsMustCenterBeforePacking")
                {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
                manual_deploy_animations: match module.attribute("ManualDeployAnimations") {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
            })
        };

        template.deploy_style_metadata = parse();
    }

    /// C++ `AIUpdateModuleData::m_autoAcquireEnemiesWhenIdle` / `m_forbidPlayerCommands`.
    fn apply_authored_auto_acquire_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        let is_ai_update = |class_name: &str| {
            class_name.eq_ignore_ascii_case("AIUpdateInterface")
                || class_name.eq_ignore_ascii_case("AIUpdate")
                || class_name.to_ascii_lowercase().ends_with("aiupdate")
        };
        let mut bits = 0u32;
        let mut forbid = false;
        let mut found = false;
        for module in &definition.behavior_modules {
            if !is_ai_update(&module.class_name) {
                continue;
            }
            found = true;
            if let Some(text) = module.attribute("AutoAcquireEnemiesWhenIdle") {
                bits |= parse_auto_acquire_idle_bits_from_ini(text);
            }
            if module
                .attribute("ForbidPlayerCommands")
                .is_some_and(|v| {
                    matches!(v.trim().to_ascii_lowercase().as_str(), "yes" | "true" | "1")
                })
            {
                forbid = true;
            }
        }
        if found {
            template.auto_acquire_enemies_when_idle = bits;
            template.forbid_player_commands = forbid;
        }
    }


    /// Retain exact `SupplyTruckAIUpdate` data. A generic HARVESTER KindOf is
    /// insufficient: it deliberately receives no autonomous collector state.
    fn apply_authored_supply_truck_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::SupplyTruckMetadata;

        fn unsigned(value: &str) -> Option<u32> {
            value.trim().parse().ok()
        }
        fn finite(value: &str) -> Option<f32> {
            let value = value.trim().parse::<f32>().ok()?;
            value.is_finite().then_some(value)
        }
        fn duration_frames(value: &str) -> Option<u32> {
            let milliseconds = unsigned(value)? as u64;
            u32::try_from(milliseconds.checked_mul(30)?.checked_add(999)? / 1_000).ok()
        }

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                let class = module.class_name.as_str();
                class.eq_ignore_ascii_case("SupplyTruckAIUpdate")
                    || class.eq_ignore_ascii_case("ChinookAIUpdate")
                    || class.eq_ignore_ascii_case("WorkerAIUpdate")
            })
            .collect();
        let [module] = modules.as_slice() else {
            template.supply_truck_metadata = None;
            template.supplies_depleted_voice.clear();
            return;
        };
        template.supplies_depleted_voice = module
            .attribute("SuppliesDepletedVoice")
            .unwrap_or("")
            .trim()
            .to_string();
        template.supply_truck_metadata = Some(SupplyTruckMetadata {
            max_boxes: module.attribute("MaxBoxes").and_then(unsigned).unwrap_or(1),
            warehouse_scan_distance: module
                .attribute("SupplyWarehouseScanDistance")
                .and_then(finite)
                .unwrap_or(0.0),
            warehouse_delay_frames: module
                .attribute("SupplyWarehouseActionDelay")
                .and_then(duration_frames)
                .unwrap_or(0),
            center_delay_frames: module
                .attribute("SupplyCenterActionDelay")
                .and_then(duration_frames)
                .unwrap_or(0),
            upgraded_supply_boost: module
                .attribute("UpgradedSupplyBoost")
                .and_then(unsigned)
                .unwrap_or(0),
        });

    }

    /// Retain exactly one source production-exit declaration.  The producer's C++ exit
    /// interface is behavior-authored, not a Barracks/WarFactory name rule;
    /// malformed or ambiguous declarations therefore expose no compact live
    /// authority rather than selecting a guessed exit style.
    fn apply_authored_production_exit_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::{ProductionExitMetadata, ProductionExitStyle};

        fn parse_bool(value: &str) -> Option<bool> {
            match value.trim().to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        /// C++ `INI::parseDurationUnsignedInt`: a source millisecond duration
        /// becomes a ceil-rounded 30 Hz logic-frame count.
        fn parse_duration_frames(value: &str) -> Option<u32> {
            let digits: String = value
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            let milliseconds = digits.parse::<u64>().ok()?;
            let frames = milliseconds.checked_mul(30)?.checked_add(999)? / 1_000;
            u32::try_from(frames).ok()
        }

        fn parse_unsigned(value: &str) -> Option<u32> {
            let digits: String = value
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            digits.parse::<u32>().ok()
        }

        /// Parse C++ `INI::parseCoord3D`'s `X: ... Y: ... Z: ...` spelling
        /// without accepting a partial/malformed vector as an authored exit
        /// position.  Semicolon comments are already retained in the raw
        /// value, so stop each numeric scan at the first non-float token.
        fn parse_coord3(value: &str) -> Option<[f32; 3]> {
            fn axis(value: &str, wanted: u8) -> Option<f32> {
                let bytes = value.as_bytes();
                let mut index = 0usize;
                while index + 1 < bytes.len() {
                    if bytes[index].eq_ignore_ascii_case(&wanted) && bytes[index + 1] == b':' {
                        let mut start = index + 2;
                        while start < bytes.len() && bytes[start].is_ascii_whitespace() {
                            start += 1;
                        }
                        let end = bytes[start..]
                            .iter()
                            .position(|byte| {
                                !matches!(byte, b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E')
                            })
                            .map(|offset| start + offset)
                            .unwrap_or(bytes.len());
                        let parsed = value.get(start..end)?.parse::<f32>().ok()?;
                        return parsed.is_finite().then_some(parsed);
                    }
                    index += 1;
                }
                None
            }

            Some([axis(value, b'X')?, axis(value, b'Y')?, axis(value, b'Z')?])
        }

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("QueueProductionExitUpdate")
                    || module
                        .class_name
                        .eq_ignore_ascii_case("DefaultProductionExitUpdate")
                    || module
                        .class_name
                        .eq_ignore_ascii_case("SupplyCenterProductionExitUpdate")
            })
            .collect();
        let [module] = modules.as_slice() else {
            template.production_exit_metadata = None;
            return;
        };

        let style = if module
            .class_name
            .eq_ignore_ascii_case("QueueProductionExitUpdate")
        {
            ProductionExitStyle::Queue
        } else if module
            .class_name
            .eq_ignore_ascii_case("SupplyCenterProductionExitUpdate")
        {
            ProductionExitStyle::SupplyCenter
        } else {
            ProductionExitStyle::Default
        };
        let metadata = (|| -> Option<ProductionExitMetadata> {
            let unit_create_point = match module.attribute("UnitCreatePoint") {
                Some(value) => parse_coord3(value)?,
                // Both C++ module constructors zero this field.
                None => [0.0, 0.0, 0.0],
            };
            let natural_rally_point = match module.attribute("NaturalRallyPoint") {
                Some(value) => parse_coord3(value)?,
                // Both C++ module constructors zero this field.
                None => [0.0, 0.0, 0.0],
            };
            match style {
                ProductionExitStyle::Queue => Some(ProductionExitMetadata {
                    style,
                    unit_create_point,
                    natural_rally_point,
                    exit_delay_frames: match module.attribute("ExitDelay") {
                        Some(value) => parse_duration_frames(value)?,
                        None => 0,
                    },
                    allow_airborne_creation: match module.attribute("AllowAirborneCreation") {
                        Some(value) => parse_bool(value)?,
                        None => false,
                    },
                    initial_burst: match module.attribute("InitialBurst") {
                        Some(value) => parse_unsigned(value)?,
                        None => 0,
                    },
                    use_spawn_rally_point: false,
                    grant_temporary_stealth_frames: 0,
                }),
                ProductionExitStyle::Default => Some(ProductionExitMetadata {
                    style,
                    unit_create_point,
                    natural_rally_point,
                    exit_delay_frames: 0,
                    allow_airborne_creation: false,
                    initial_burst: 0,
                    use_spawn_rally_point: match module.attribute("UseSpawnRallyPoint") {
                        Some(value) => parse_bool(value)?,
                        None => false,
                    },
                    grant_temporary_stealth_frames: 0,
                }),
                ProductionExitStyle::SupplyCenter => Some(ProductionExitMetadata {
                    style,
                    unit_create_point,
                    natural_rally_point,
                    exit_delay_frames: 0,
                    allow_airborne_creation: false,
                    initial_burst: 0,
                    use_spawn_rally_point: false,
                    grant_temporary_stealth_frames: match module.attribute("GrantTemporaryStealth") {
                        Some(value) => parse_duration_frames(value)?,
                        None => 0,
                    },
                }),
            }
        })();
        template.production_exit_metadata = metadata;
    }

    /// Retain the exact retail `VeterancyCrateCollide IsPilot` behavior used
    /// by `AmericaInfantryPilot` to re-crew an unmanned vehicle.  This is not
    /// a generic veterancy-crate parser: C++ has a wide crate matrix and the
    /// compact host only has an authority path for the explicit pilot shape.
    ///
    /// A pilot-named object with no well-formed `IsPilot = Yes` behavior is
    /// intentionally indistinguishable from ordinary infantry to live Enter
    /// authority.  Likewise, an unrepresentable kind mask remains retained
    /// as pilot metadata for its own `VeterancyGainCreate` starting level but
    /// cannot invent a re-crew target criterion.
    fn apply_authored_pilot_veterancy_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::{VeterancyCrateCollideMetadata, VeterancyLevel};

        fn parse_bool(value: &str) -> Option<bool> {
            match value.trim().to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        fn exactly_one_kind_token(value: &str, expected: &str) -> bool {
            let mut tokens = value
                .split(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, ',' | '|')
                })
                .filter(|token| !token.is_empty());
            let Some(token) = tokens.next() else {
                return false;
            };
            token.eq_ignore_ascii_case(expected) && tokens.next().is_none()
        }

        fn parse_starting_level(value: &str) -> Option<VeterancyLevel> {
            match value.trim().to_ascii_uppercase().as_str() {
                // C++ spelling is REGULAR, while the compact host calls the
                // same base rank Rookie.  Accept both only at this parser
                // representation boundary.
                "REGULAR" | "ROOKIE" => Some(VeterancyLevel::Rookie),
                "VETERAN" => Some(VeterancyLevel::Veteran),
                "ELITE" => Some(VeterancyLevel::Elite),
                "HEROIC" => Some(VeterancyLevel::Heroic),
                _ => None,
            }
        }

        // There may be ordinary level-up crate modules on other objects.  A
        // live pilot source exists only when there is exactly one explicitly
        // marked IsPilot behavior; duplicate pilot behaviors have cumulative
        // C++ semantics the bounded re-crew path cannot safely collapse.
        let pilot_modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("VeterancyCrateCollide")
                    && module.attribute("IsPilot").and_then(parse_bool) == Some(true)
            })
            .collect();
        let [module] = pilot_modules.as_slice() else {
            template.veterancy_crate_collide = None;
            return;
        };

        let gain_modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("VeterancyGainCreate")
            })
            .collect();
        let starting_level = match gain_modules.as_slice() {
            [gain] => gain
                .attribute("StartingLevel")
                .and_then(parse_starting_level),
            // No behavior or multiple create behaviors is intentionally not
            // approximated as a default veteran source.
            _ => None,
        };

        let effect_range = module
            .attribute("EffectRange")
            .and_then(|value| value.trim().parse::<f32>().ok())
            .filter(|value| value.is_finite());
        let metadata = VeterancyCrateCollideMetadata {
            is_pilot: true,
            required_kind_of_vehicle: module
                .attribute("RequiredKindOf")
                .is_some_and(|value| exactly_one_kind_token(value, "VEHICLE")),
            forbidden_kind_of_dozer: module
                .attribute("ForbiddenKindOf")
                .is_some_and(|value| exactly_one_kind_token(value, "DOZER")),
            effect_range,
            adds_owner_veterancy: module.attribute("AddsOwnerVeterancy").and_then(parse_bool)
                == Some(true),
            starting_level,
        };
        template.veterancy_crate_collide = Some(metadata);
    }

    /// C++ `VeterancyGainCreate` modules: StartingLevel + optional ScienceRequired.
    fn apply_authored_veterancy_gain_create(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        fn parse_starting_level(value: &str) -> Option<VeterancyLevel> {
            match value.trim().to_ascii_uppercase().as_str() {
                "REGULAR" | "ROOKIE" => Some(VeterancyLevel::Rookie),
                "VETERAN" => Some(VeterancyLevel::Veteran),
                "ELITE" => Some(VeterancyLevel::Elite),
                "HEROIC" => Some(VeterancyLevel::Heroic),
                _ => None,
            }
        }

        template.veterancy_gain_creates = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("VeterancyGainCreate")
            })
            .map(|module| crate::game_logic::VeterancyGainCreateMetadata {
                starting_level: module
                    .attribute("StartingLevel")
                    .and_then(parse_starting_level)
                    .unwrap_or(VeterancyLevel::Rookie),
                science_required: module
                    .attribute("ScienceRequired")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            })
            .collect();
    }

    /// C++ Create modules authored on the Object INI (not template-name heuristics).
    fn apply_authored_create_modules(template: &mut ThingTemplate, definition: &ObjectDefinition) {
        template.grant_upgrade_creates = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("GrantUpgradeCreate"))
            .filter_map(|module| {
                let upgrade_name = module
                    .attribute("UpgradeToGrant")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())?;
                let exempt = module
                    .attribute("ExemptStatus")
                    .map(|s| s.to_ascii_uppercase())
                    .unwrap_or_default();
                Some(crate::game_logic::GrantUpgradeCreateMetadata {
                    upgrade_name,
                    exempt_under_construction: exempt.contains("UNDER_CONSTRUCTION"),
                })
            })
            .collect();

        template.lock_weapon_slot = definition.behavior_modules.iter().find_map(|module| {
            if !module.class_name.eq_ignore_ascii_case("LockWeaponCreate") {
                return None;
            }
            let slot = module
                .attribute("SlotToLock")
                .unwrap_or("PRIMARY_WEAPON")
                .trim()
                .to_ascii_uppercase();
            match slot.as_str() {
                "PRIMARY" | "PRIMARY_WEAPON" => Some(0),
                "SECONDARY" | "SECONDARY_WEAPON" => Some(1),
                "TERTIARY" | "TERTIARY_WEAPON" => Some(2),
                _ => Some(0),
            }
        });

        template.has_preorder_create = definition
            .behavior_modules
            .iter()
            .any(|module| module.class_name.eq_ignore_ascii_case("PreorderCreate"));
        template.has_special_power_create = definition
            .behavior_modules
            .iter()
            .any(|module| module.class_name.eq_ignore_ascii_case("SpecialPowerCreate"));
        template.has_supply_center_create = definition
            .behavior_modules
            .iter()
            .any(|module| module.class_name.eq_ignore_ascii_case("SupplyCenterCreate"));
        template.has_supply_warehouse_create = definition
            .behavior_modules
            .iter()
            .any(|module| module.class_name.eq_ignore_ascii_case("SupplyWarehouseCreate"));
    }

    /// Retain one C++ `EjectPilotDieModuleData` declaration as typed Object
    /// metadata.  `getEjectPilotDieInterface()` is a module-presence query,
    /// so even a custom/unrepresentable module remains visible to the
    /// Hijacker path.  The death path, however, must not manufacture an OCL
    /// result from data it cannot execute exactly.
    fn apply_authored_eject_pilot_die_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::{
            EjectPilotCreationList, EjectPilotDeathTypes, EjectPilotDieMetadata,
            EjectPilotExemptStatus, EjectPilotRequiredStatus, EjectPilotVeterancyLevels,
        };

        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        fn normalized_tokens(value: &str) -> Vec<String> {
            let mut tokens: Vec<_> = stripped_value(value)
                .split(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, ',' | '|')
                })
                .filter(|token| !token.is_empty())
                .map(|token| token.to_ascii_uppercase())
                .collect();
            tokens.sort_unstable();
            tokens
        }

        fn parse_creation_list(value: &str) -> Option<EjectPilotCreationList> {
            match stripped_value(value) {
                value if value.eq_ignore_ascii_case("OCL_EjectPilotOnGround") => {
                    Some(EjectPilotCreationList::OnGround)
                }
                value if value.eq_ignore_ascii_case("OCL_EjectPilotViaParachute") => {
                    Some(EjectPilotCreationList::ViaParachute)
                }
                _ => None,
            }
        }

        fn parse_duration_ms(value: &str) -> Option<u32> {
            // C++ uses `INI::parseDurationUnsignedInt`.  The active retail
            // blocks omit this field (constructor default 0); a non-bare
            // custom duration is not approximated by the compact bridge.
            stripped_value(value).parse::<u32>().ok()
        }

        fn parse_death_types(value: &str) -> EjectPilotDeathTypes {
            match normalized_tokens(value).as_slice() {
                [all] if all == "ALL" => EjectPilotDeathTypes::All,
                [crushed, splatted, all]
                    if crushed == "-CRUSHED" && splatted == "-SPLATTED" && all == "ALL" =>
                {
                    EjectPilotDeathTypes::AllExceptCrushedAndSplatted
                }
                _ => EjectPilotDeathTypes::Unsupported,
            }
        }

        fn parse_veterancy_levels(value: &str) -> EjectPilotVeterancyLevels {
            match normalized_tokens(value).as_slice() {
                [all] if all == "ALL" => EjectPilotVeterancyLevels::All,
                [regular, all] if regular == "-REGULAR" && all == "ALL" => {
                    EjectPilotVeterancyLevels::AllExceptRegular
                }
                _ => EjectPilotVeterancyLevels::Unsupported,
            }
        }

        fn parse_exempt_status(value: &str) -> EjectPilotExemptStatus {
            let tokens = normalized_tokens(value);
            match tokens.as_slice() {
                [] => EjectPilotExemptStatus::None,
                [none] if none == "NONE" => EjectPilotExemptStatus::None,
                [hijacked] if hijacked == "HIJACKED" => EjectPilotExemptStatus::Hijacked,
                _ => EjectPilotExemptStatus::Unsupported,
            }
        }

        fn parse_required_status(value: &str) -> EjectPilotRequiredStatus {
            let tokens = normalized_tokens(value);
            match tokens.as_slice() {
                [] => EjectPilotRequiredStatus::None,
                [none] if none == "NONE" => EjectPilotRequiredStatus::None,
                _ => EjectPilotRequiredStatus::Unsupported,
            }
        }

        let mut eject_modules = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("EjectPilotDie"));
        let Some(module) = eject_modules.next() else {
            template.eject_pilot_die = None;
            return;
        };

        // C++ exposes an EjectPilotDie interface for each module.  The host
        // can retain that fact for Hijacker even when multiple die modules
        // cannot be losslessly collapsed into one spawned-pilot behavior.
        if eject_modules.next().is_some() {
            let mut metadata = EjectPilotDieMetadata::default();
            metadata.invulnerable_time_ms = None;
            metadata.death_types = EjectPilotDeathTypes::Unsupported;
            metadata.veterancy_levels = EjectPilotVeterancyLevels::Unsupported;
            metadata.exempt_status = EjectPilotExemptStatus::Unsupported;
            metadata.required_status = EjectPilotRequiredStatus::Unsupported;
            template.eject_pilot_die = Some(metadata);
            return;
        }

        let metadata = EjectPilotDieMetadata {
            ground_creation_list: module
                .attribute("GroundCreationList")
                .and_then(parse_creation_list),
            air_creation_list: module
                .attribute("AirCreationList")
                .and_then(parse_creation_list),
            // EjectPilotDieModuleData's constructor has `m_invulnerableTime
            // = 0`; the retail 2000ms shield belongs to the selected OCL,
            // not this Behavior field.
            invulnerable_time_ms: match module.attribute("InvulnerableTime") {
                Some(value) => parse_duration_ms(value),
                None => Some(0),
            },
            death_types: module
                .attribute("DeathTypes")
                .map(parse_death_types)
                .unwrap_or(EjectPilotDeathTypes::All),
            veterancy_levels: module
                .attribute("VeterancyLevels")
                .map(parse_veterancy_levels)
                .unwrap_or(EjectPilotVeterancyLevels::All),
            exempt_status: module
                .attribute("ExemptStatus")
                .map(parse_exempt_status)
                .unwrap_or(EjectPilotExemptStatus::None),
            required_status: module
                .attribute("RequiredStatus")
                .map(parse_required_status)
                .unwrap_or(EjectPilotRequiredStatus::None),
        };
        template.eject_pilot_die = Some(metadata);
    }

    /// Retain one C++ `RebuildHoleExposeDieModuleData` declaration.
    /// Module presence is the die authority; a GLA/template-name heuristic
    /// never fabricates HoleName.
    fn apply_authored_rebuild_hole_expose_die_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        fn parse_bool(value: &str) -> Option<bool> {
            match stripped_value(value) {
                value if value.eq_ignore_ascii_case("yes") || value.eq_ignore_ascii_case("true") => {
                    Some(true)
                }
                value if value.eq_ignore_ascii_case("no") || value.eq_ignore_ascii_case("false") => {
                    Some(false)
                }
                _ => None,
            }
        }

        let mut modules = definition.behavior_modules.iter().filter(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("RebuildHoleExposeDie")
        });
        let Some(module) = modules.next() else {
            template.rebuild_hole_expose = None;
            return;
        };
        let _ = modules.next();

        let hole_name = module
            .attribute("HoleName")
            .map(stripped_value)
            .filter(|name| !name.is_empty())
            .unwrap_or("")
            .to_string();
        if hole_name.is_empty() {
            template.rebuild_hole_expose = None;
            return;
        }
        template.rebuild_hole_expose = Some(crate::game_logic::RebuildHoleExposeDieMetadata {
            hole_name,
            hole_max_health: module
                .attribute("HoleMaxHealth")
                .and_then(|value| stripped_value(value).parse::<f32>().ok())
                .unwrap_or(0.0),
            transfer_attackers: module
                .attribute("TransferAttackers")
                .and_then(parse_bool)
                .unwrap_or(true),
        });
    }

    /// Retain one exact `HackInternetAIUpdate` behavior from Object INI.
    ///
    /// C++ owns these fields in `HackInternetAIUpdateModuleData`; it does not
    /// derive them from a China faction, `MONEY_HACKER`, or a template name.
    /// Missing fields preserve the C++ constructor's zero defaults.  A
    /// malformed present field or multiple update modules cannot be merged
    /// safely, so the live Hacker command/income path rejects that template.
    fn apply_authored_hack_internet_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::HackInternetAIUpdateMetadata;

        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        // `INI::parseDurationUnsignedInt`: scan an unsigned millisecond
        // prefix and round *up* to 30 Hz logic frames.
        fn parse_duration_frames(value: &str) -> Option<u32> {
            let digits: String = stripped_value(value)
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            let milliseconds = digits.parse::<u64>().ok()?;
            let frames = milliseconds.checked_mul(30)?.checked_add(999)? / 1_000;
            u32::try_from(frames).ok()
        }

        // C++ `parseUnsignedInt` uses an unsigned numeric prefix.  Retain
        // that permissive numeric boundary but reject an empty/non-numeric
        // authored value rather than substituting a retail constant.
        fn parse_unsigned(value: &str) -> Option<u32> {
            let digits: String = stripped_value(value)
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            digits.parse::<u32>().ok()
        }

        fn parse_real(value: &str) -> Option<f32> {
            stripped_value(value)
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
        }

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("HackInternetAIUpdate")
            })
            .collect();
        let [module] = modules.as_slice() else {
            template.hack_internet_ai_update = None;
            return;
        };

        let parse = || -> Option<HackInternetAIUpdateMetadata> {
            Some(HackInternetAIUpdateMetadata {
                unpack_time_frames: match module.attribute("UnpackTime") {
                    Some(value) => parse_duration_frames(value)?,
                    None => 0,
                },
                pack_time_frames: match module.attribute("PackTime") {
                    Some(value) => parse_duration_frames(value)?,
                    None => 0,
                },
                cash_update_delay_frames: match module.attribute("CashUpdateDelay") {
                    Some(value) => parse_duration_frames(value)?,
                    None => 0,
                },
                cash_update_delay_fast_frames: match module.attribute("CashUpdateDelayFast") {
                    Some(value) => parse_duration_frames(value)?,
                    None => 0,
                },
                regular_cash_amount: match module.attribute("RegularCashAmount") {
                    Some(value) => parse_unsigned(value)?,
                    None => 0,
                },
                veteran_cash_amount: match module.attribute("VeteranCashAmount") {
                    Some(value) => parse_unsigned(value)?,
                    None => 0,
                },
                elite_cash_amount: match module.attribute("EliteCashAmount") {
                    Some(value) => parse_unsigned(value)?,
                    None => 0,
                },
                heroic_cash_amount: match module.attribute("HeroicCashAmount") {
                    Some(value) => parse_unsigned(value)?,
                    None => 0,
                },
                xp_per_cash_update: match module.attribute("XpPerCashUpdate") {
                    // C++ stores this as `UnsignedInt`; retain the exact
                    // accepted syntax before adapting to Main's f32 XP API.
                    Some(value) => parse_unsigned(value)? as f32,
                    None => 0.0,
                },
                pack_unpack_variation_factor: match module.attribute("PackUnpackVariationFactor") {
                    Some(value) => parse_real(value)?,
                    None => 0.0,
                },
            })
        };
        template.hack_internet_ai_update = parse();
    }

    /// Retain generic C++ `SpecialPowerModule` interfaces in Object INI
    /// declaration order.  C++ does not infer this ability from a structure
    /// name or KindOf bit: `Object::getSpecialPowerModule` walks the actual
    /// behavior-module list and compares the resolved SpecialPower template.
    ///
    /// Hacker Disable keeps its paired `SpecialAbilityUpdate` record in the
    /// dedicated parser below.  This generic record only preserves the source
    /// module interface and must not alter that channel's timing/target rules.
    fn apply_authored_special_power_module_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::command_system::special_power_type_from_template_name;
        use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
        use game_engine::common::ini::ini_science::{get_science_store, ScienceType};
        use game_engine::common::rts::special_power::{get_special_power_store, SCIENCE_INVALID};

        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        // C++ `INI::parseInt` accepts a signed numeric prefix.  A malformed
        // authored EnergyProduction remains unavailable rather than silently
        // borrowing the historical Particle/Nuke name table.
        fn parse_cxx_int(value: &str) -> Option<i32> {
            let value = stripped_value(value).trim_start();
            let sign_len = if value.starts_with('+') || value.starts_with('-') {
                1
            } else {
                0
            };
            let digits_len = value[sign_len..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();
            (digits_len > 0)
                .then(|| &value[..sign_len + digits_len])
                .and_then(|integer| integer.parse::<i32>().ok())
        }

        fn parse_bool(value: &str) -> Option<bool> {
            match stripped_value(value).to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        template.energy_production = Self::object_definition_attr(definition, "energyproduction")
            .and_then(|value| parse_cxx_int(&value));
        template.max_simultaneous_link_key =
            Self::object_definition_attr(definition, "maxsimultaneouslinkkey")
                .map(|value| stripped_value(&value).to_string())
                .filter(|value| !value.is_empty());
        // C++ ThingTemplate::parseMaxSimultaneous: numeric UnsignedShort or
        // DeterminedBySuperweaponRestriction (stores 0 + the restriction bool).
        if let Some(value) = Self::object_definition_attr(definition, "maxsimultaneousoftype") {
            let token = stripped_value(&value);
            if token.eq_ignore_ascii_case("DeterminedBySuperweaponRestriction") {
                template.max_simultaneous_determined_by_superweapon_restriction = true;
                template.max_simultaneous_of_type = 0;
            } else if let Some(n) = parse_cxx_int(token).and_then(|v| u16::try_from(v).ok()) {
                template.max_simultaneous_of_type = n;
                template.max_simultaneous_determined_by_superweapon_restriction = false;
            }
        }

        template.special_power_modules.clear();
        let power_store = get_special_power_store();
        for (source_index, module) in definition.behavior_modules.iter().enumerate() {
            let Some(module_kind) =
                SpecialPowerModuleKind::from_behavior_class_name(&module.class_name)
            else {
                continue;
            };
            let Some(raw_template_name) = module.attribute("SpecialPowerTemplate") else {
                // A SpecialPowerModule subclass without the exact source
                // template cannot authorize an arbitrary host enum.
                continue;
            };
            let Some(power) = power_store.find_template(stripped_value(raw_template_name)) else {
                continue;
            };
            let required_science = if power.required_science == SCIENCE_INVALID {
                Some(None)
            } else {
                get_science_store()
                    .get_internal_name_for_science(ScienceType(power.required_science))
                    .map(|science| Some(science.as_str().to_string()))
            };
            let Some(required_science) = required_science else {
                // The C++ module points at a live ScienceTemplate.  Do not
                // weaken an unresolved science prerequisite into `None`.
                continue;
            };

            let parsed = (|| -> Option<SpecialPowerModuleMetadata> {
                Some(SpecialPowerModuleMetadata {
                    source_index: source_index.min(u32::MAX as usize) as u32,
                    module_tag: module.module_tag.clone(),
                    module_kind,
                    special_power_template: power.name.clone(),
                    special_power_template_id: power.id,
                    command_power: special_power_type_from_template_name(&power.name),
                    reload_time_frames: power.reload_time,
                    required_science,
                    public_timer: power.public_timer,
                    shared_n_sync: power.shared_n_sync,
                    shortcut_power: power.shortcut_power,
                    update_module_starts_attack: match module.attribute("UpdateModuleStartsAttack")
                    {
                        Some(value) => parse_bool(value)?,
                        None => false,
                    },
                    starts_paused: match module.attribute("StartsPaused") {
                        Some(value) => parse_bool(value)?,
                        None => false,
                    },
                    scripted_special_power_only: match module.attribute("ScriptedSpecialPowerOnly")
                    {
                        Some(value) => parse_bool(value)?,
                        None => false,
                    },
                })
            })();
            if let Some(parsed) = parsed {
                template.special_power_modules.push(parsed);
            }
        }
    }

    /// Retain a complete `SPECIAL_HACKER_DISABLE_BUILDING` module pair.
    ///
    /// C++ `ActionManager::canDisableBuildingViaHacking` queries the source
    /// `SpecialAbility` module, while the actual target channel belongs to a
    /// matching `SpecialAbilityUpdate`.  An HDB enum by itself is therefore
    /// insufficient.  Parse a unique pair pointing at the *same loaded*
    /// SpecialPower template — either the Hacker or Microwave retail identity
    /// (both C++ Enum `SPECIAL_HACKER_DISABLE_BUILDING`).  A partial,
    /// duplicate, or malformed pair remains unavailable rather than inheriting
    /// retail Hacker timings by name.
    fn apply_authored_hacker_disable_building_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::command_system::{
            special_power_type_from_template_name, SpecialPowerType as HostSpecialPowerType,
        };
        use crate::game_logic::HackerDisableBuildingMetadata;
        use game_engine::common::ini::ini_science::{get_science_store, ScienceType};
        use game_engine::common::rts::special_power::{
            get_special_power_store, SpecialPowerType, SCIENCE_INVALID,
        };

        const DEFAULT_ABILITY_RANGE: f32 = 10_000_000.0;

        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        // `INI::parseDurationUnsignedInt`: retain a numeric millisecond
        // prefix but reject a malformed *present* field.  The channel itself
        // integrates these source milliseconds without a retail constant.
        fn parse_duration_ms(value: &str) -> Option<u32> {
            let digits: String = stripped_value(value)
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            digits.parse::<u32>().ok()
        }

        fn parse_nonnegative_real(value: &str) -> Option<f32> {
            stripped_value(value)
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.0)
        }

        fn parse_bool(value: &str) -> Option<bool> {
            match stripped_value(value).to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        template.hacker_disable_building = None;
        let power_store = get_special_power_store();
        let abilities: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter_map(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("SpecialAbility")
                    .then_some(module)
                    .and_then(|module| {
                        let raw = module.attribute("SpecialPowerTemplate")?.trim();
                        let power = power_store.find_template(raw)?;
                        // C++ has no distinct Microwave SpecialPowerType: both
                        // retail templates are SPECIAL_HACKER_DISABLE_BUILDING.
                        // Admit either authored identity so Microwave uses the
                        // same disable-building channel with its own timings.
                        (power.power_type == SpecialPowerType::HackerDisableBuilding
                            && matches!(
                                special_power_type_from_template_name(&power.name),
                                Some(HostSpecialPowerType::HackerDisableBuilding)
                                    | Some(HostSpecialPowerType::MicrowaveDisableBuilding)
                            ))
                        .then_some((module, power))
                    })
            })
            .collect();
        let [(ability, power)] = abilities.as_slice() else {
            return;
        };

        // The update must name this exact SpecialPowerTemplate, not merely a
        // second template that happens to use the HDB enum.
        let updates: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("SpecialAbilityUpdate")
            })
            .filter(|module| {
                module
                    .attribute("SpecialPowerTemplate")
                    .and_then(|raw| power_store.find_template(raw.trim()))
                    .is_some_and(|candidate| candidate.id == power.id)
            })
            .collect();
        let [update] = updates.as_slice() else {
            return;
        };

        let required_science = if power.required_science == SCIENCE_INVALID {
            Some(None)
        } else {
            get_science_store()
                .get_internal_name_for_science(ScienceType(power.required_science))
                .map(|science| Some(science.as_str().to_string()))
        };
        let Some(required_science) = required_science else {
            return;
        };

        let parse = || -> Option<HackerDisableBuildingMetadata> {
            Some(HackerDisableBuildingMetadata {
                special_power_template: power.name.clone(),
                update_module_starts_attack: match ability.attribute("UpdateModuleStartsAttack") {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
                starts_paused: match ability.attribute("StartsPaused") {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
                scripted_special_power_only: match ability.attribute("ScriptedSpecialPowerOnly") {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
                reload_time_frames: power.reload_time,
                required_science,
                shared_n_sync: power.shared_n_sync,
                start_ability_range: match update.attribute("StartAbilityRange") {
                    Some(value) => parse_nonnegative_real(value)?,
                    None => DEFAULT_ABILITY_RANGE,
                },
                ability_abort_range: match update.attribute("AbilityAbortRange") {
                    Some(value) => parse_nonnegative_real(value)?,
                    None => DEFAULT_ABILITY_RANGE,
                },
                approach_requires_los: match update.attribute("ApproachRequiresLOS") {
                    Some(value) => parse_bool(value)?,
                    // `SpecialAbilityUpdateModuleData` defaults this to Yes.
                    None => true,
                },
                unpack_time_ms: match update.attribute("UnpackTime") {
                    Some(value) => parse_duration_ms(value)?,
                    None => 0,
                },
                preparation_time_ms: match update.attribute("PreparationTime") {
                    Some(value) => parse_duration_ms(value)?,
                    None => 0,
                },
                persistent_prep_time_ms: match update.attribute("PersistentPrepTime") {
                    Some(value) => parse_duration_ms(value)?,
                    None => 0,
                },
                effect_duration_ms: match update.attribute("EffectDuration") {
                    Some(value) => parse_duration_ms(value)?,
                    None => 0,
                },
                pack_time_ms: match update.attribute("PackTime") {
                    Some(value) => parse_duration_ms(value)?,
                    None => 0,
                },
                pack_unpack_variation_factor: match update.attribute("PackUnpackVariationFactor") {
                    Some(value) => parse_nonnegative_real(value)?,
                    None => 0.0,
                },

                persistence_requires_recharge: match update.attribute("PersistenceRequiresRecharge")
                {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
            })
        };
        template.hacker_disable_building = parse();
    }


    /// Retain authored Burton/TNT `SpecialAbilityUpdate` pack/unpack/flee data.
    /// C++ `SpecialAbilityUpdateModuleData` (`SpecialAbilityUpdate.h:113`).
    fn apply_authored_charge_plant_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        fn parse_duration_ms(value: &str) -> Option<u32> {
            value
                .trim()
                .parse::<i64>()
                .ok()
                .and_then(|ms| u32::try_from(ms).ok())
        }
        fn parse_real(value: &str) -> Option<f32> {
            value
                .trim()
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.0)
        }
        fn parse_bool(value: &str) -> bool {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "yes" | "true" | "1"
            )
        }
        fn is_charge_plant_power(name: &str) -> bool {
            let name = name.to_ascii_lowercase();
            name.contains("timedcharges")
                || name.contains("remotecharges")
                || name.contains("tntattack")
        }

        template.charge_plant_abilities.clear();
        for module in &definition.behavior_modules {
            if !module
                .class_name
                .eq_ignore_ascii_case("SpecialAbilityUpdate")
            {
                continue;
            }
            let Some(power) = module
                .attribute("SpecialPowerTemplate")
                .map(str::trim)
                .filter(|name| !name.is_empty() && is_charge_plant_power(name))
            else {
                continue;
            };
            template
                .charge_plant_abilities
                .push(ChargePlantAbilityMetadata {
                    special_power_template: power.to_string(),
                    unpack_time_ms: module
                        .attribute("UnpackTime")
                        .and_then(parse_duration_ms)
                        .unwrap_or(0),
                    pack_time_ms: module
                        .attribute("PackTime")
                        .and_then(parse_duration_ms)
                        .unwrap_or(0),
                    pack_unpack_variation_factor: module
                        .attribute("PackUnpackVariationFactor")
                        .and_then(parse_real)
                        .unwrap_or(0.0),
                    flee_range_after_completion: module
                        .attribute("FleeRangeAfterCompletion")
                        .and_then(parse_real)
                        .unwrap_or(0.0),
                    flip_object_after_unpacking: module
                        .attribute("FlipOwnerAfterUnpacking")
                        .is_some_and(parse_bool),
                    flip_object_after_packing: module
                        .attribute("FlipOwnerAfterPacking")
                        .is_some_and(parse_bool),
                });
        }
    }

    /// Retain the exact StealthUpdate friendly opacity bounds used by
    /// C++ `StealthUpdate::getFriendlyOpacity`. Missing fields retain the
    /// module-data defaults; malformed present values fail closed to those
    /// defaults rather than inventing a non-finite presentation value.
    fn apply_authored_stealth_update_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        fn parse_percent(value: &str) -> Option<f32> {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches('%')
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| (value / 100.0).clamp(0.0, 1.0))
        }

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("StealthUpdate"))
            .collect();
        let [module] = modules.as_slice() else {
            return;
        };

        if let Some(value) = module
            .attribute("FriendlyOpacityMin")
            .and_then(parse_percent)
        {
            template.stealth_friendly_opacity_min = value;
        }
        if let Some(value) = module
            .attribute("FriendlyOpacityMax")
            .and_then(parse_percent)
        {
            template.stealth_friendly_opacity_max = value;
        }
        if template.stealth_friendly_opacity_min > template.stealth_friendly_opacity_max {
            template.stealth_friendly_opacity_min = 0.5;
            template.stealth_friendly_opacity_max = 1.0;
        }
    }

    /// Retain the exact Object INI data C++ `OverchargeBehavior` consumes.
    ///
    /// This is intentionally neither a China-faction nor a `PowerPlant`
    /// classification.  The live toggle/damage path is authorized only by
    /// this behavior; a separately parsed ThingTemplate `EnergyBonus` only
    /// determines its power delta and defaults to zero in C++.
    /// Missing behavior stays absent; missing fields *inside* a real behavior
    /// retain C++ `OverchargeBehaviorModuleData` constructor defaults of 0%.
    fn apply_authored_overcharge_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::OverchargeBehaviorMetadata;

        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        // C++ `INI::parsePercentToReal` accepts a bare numeric or one ending
        // in `%`, then divides by 100.  Reject NaN/Infinity because the host
        // cannot safely represent a non-finite damage rate.
        fn parse_percent_to_real(value: &str) -> Option<f32> {
            stripped_value(value)
                .trim_end_matches('%')
                .parse::<f32>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| value / 100.0)
        }

        // C++ `INI::parseInt` ultimately uses `sscanf("%d")`: accept a
        // signed integer prefix (so `5.0` retains its C++ value of 5) but do
        // not manufacture a value when no integer begins the field.
        fn parse_cxx_int(value: &str) -> Option<i32> {
            let value = stripped_value(value).trim_start();
            let sign_len = if value.starts_with('+') || value.starts_with('-') {
                1
            } else {
                0
            };
            let digits_len = value[sign_len..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();
            (digits_len > 0)
                .then(|| &value[..sign_len + digits_len])
                .and_then(|integer| integer.parse::<i32>().ok())
        }

        // `EnergyBonus` belongs to ThingTemplate rather than the Behavior.
        // Its C++ constructor default is zero, and that default does *not*
        // remove an authored OverchargeBehavior interface.  In contrast, a
        // malformed present field cannot be safely given a made-up delta, so
        // reject the whole live behavior rather than turn it into a +0 guess.
        template.energy_bonus = match Self::object_definition_attr(definition, "energybonus") {
            Some(value) => match parse_cxx_int(&value) {
                Some(value) => Some(value),
                None => {
                    template.overcharge_behavior = None;
                    return;
                }
            },
            None => None,
        };

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("OverchargeBehavior"))
            .collect();
        let [module] = modules.as_slice() else {
            template.overcharge_behavior = None;
            return;
        };

        template.overcharge_behavior = Some(OverchargeBehaviorMetadata {
            health_percent_to_drain_per_second: match module
                .attribute("HealthPercentToDrainPerSecond")
            {
                Some(value) => match parse_percent_to_real(value) {
                    Some(value) => value,
                    None => {
                        template.overcharge_behavior = None;
                        return;
                    }
                },
                None => 0.0,
            },
            not_allowed_when_health_below_percent: match module
                .attribute("NotAllowedWhenHealthBelowPercent")
            {
                Some(value) => match parse_percent_to_real(value) {
                    Some(value) => value,
                    None => {
                        template.overcharge_behavior = None;
                        return;
                    }
                },
                None => 0.0,
            },
        });
    }

    /// Retain the exact `PowerPlantUpdate` interface used by
    /// `OverchargeBehavior::enable` solely for its rod model conditions.  An
    /// object may legitimately have Overcharge without this interface, so an
    /// absent/malformed update never removes the typed power toggle; it only
    /// suppresses a visual state Main cannot faithfully drive.
    fn apply_authored_power_plant_update_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::PowerPlantUpdateMetadata;

        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        // `INI::parseDurationUnsignedInt`: scan an unsigned millisecond
        // prefix and ceil-convert it to the 30 Hz logic frame counter.
        fn parse_duration_frames(value: &str) -> Option<u32> {
            let digits: String = stripped_value(value)
                .trim_start()
                .bytes()
                .take_while(u8::is_ascii_digit)
                .map(char::from)
                .collect();
            let milliseconds = digits.parse::<u64>().ok()?;
            let frames = milliseconds.checked_mul(30)?.checked_add(999)? / 1_000;
            u32::try_from(frames).ok()
        }

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("PowerPlantUpdate"))
            .collect();
        let [module] = modules.as_slice() else {
            template.power_plant_update = None;
            return;
        };

        template.power_plant_update = match module.attribute("RodsExtendTime") {
            Some(value) => parse_duration_frames(value).map(|rods_extend_time_frames| {
                PowerPlantUpdateMetadata {
                    rods_extend_time_frames,
                }
            }),
            // C++ `PowerPlantUpdateModuleData` constructor default.
            None => Some(PowerPlantUpdateMetadata {
                rods_extend_time_frames: 0,
            }),
        };
    }

    /// Retain exact source data for C++ `FireWeaponWhenDamagedBehavior` and
    /// `FireWeaponWhenDeadBehavior` without activating a second live firing
    /// path.  The former owns eight independent mutable PRIMARY Weapons and
    /// C++ Xfers every present one; Main's current Object snapshot has no
    /// behavior-runtime tail, so attaching those states before a coordinated
    /// schema revision would silently corrupt cooldown/ammo/barrel state on
    /// save/load.  Source metadata is safe and lets that later activation use
    /// retained module identity rather than template-name residuals.
    fn apply_authored_temporary_weapon_behavior_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::host_temporary_weapon_behavior::{
            parse_fire_weapon_when_damaged_metadata, parse_fire_weapon_when_dead_metadata,
        };

        template.fire_weapon_when_damaged_behaviors = definition
            .behavior_modules
            .iter()
            .enumerate()
            .filter_map(|(source_index, module)| {
                parse_fire_weapon_when_damaged_metadata(module, source_index)
            })
            .collect();
        template.fire_weapon_when_dead_behaviors = definition
            .behavior_modules
            .iter()
            .enumerate()
            .filter_map(|(source_index, module)| {
                parse_fire_weapon_when_dead_metadata(module, source_index)
            })
            .collect();
    }

    /// C++ PhysicsBehaviorModuleData Mass / Shock / friction / COM / fall / KillWhenResting.
    fn apply_authored_physics_behavior_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        fn stripped(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }
        fn parse_positive(value: &str) -> Option<f32> {
            stripped(value)
                .trim_end_matches('f')
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite() && *v > 0.0)
        }
        fn parse_real(value: &str) -> Option<f32> {
            stripped(value)
                .trim_end_matches('f')
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
        }
        fn parse_bool(value: &str) -> Option<bool> {
            match stripped(value).to_ascii_lowercase().as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }
        // C++ parseFrictionPerSec: per-sec → per-frame.
        const SECONDS_PER_LOGICFRAME: f32 = 1.0 / 30.0;
        fn parse_friction(value: &str) -> Option<f32> {
            parse_real(value).map(|v| v * SECONDS_PER_LOGICFRAME)
        }

        let modules: Vec<_> = definition
            .behavior_modules
            .iter()
            .filter(|module| module.class_name.eq_ignore_ascii_case("PhysicsBehavior"))
            .collect();
        let [module] = modules.as_slice() else {
            return;
        };
        if let Some(mass) = module.attribute("Mass").and_then(parse_positive) {
            template.physics_mass = mass;
        }
        if let Some(res) = module.attribute("ShockResistance").and_then(parse_positive) {
            template.shock_resistance = res;
        }
        if let Some(factor) = module
            .attribute("PitchRollYawFactor")
            .and_then(parse_real)
        {
            template.pitch_roll_yaw_factor = factor;
        }
        if let Some(f) = module.attribute("ForwardFriction").and_then(parse_friction) {
            template.forward_friction = f;
        }
        if let Some(f) = module.attribute("LateralFriction").and_then(parse_friction) {
            template.lateral_friction = f;
        }
        if let Some(f) = module.attribute("ZFriction").and_then(parse_friction) {
            template.z_friction = f;
        }
        if let Some(f) = module
            .attribute("AerodynamicFriction")
            .and_then(parse_friction)
        {
            template.aerodynamic_friction = f;
        }
        if let Some(off) = module.attribute("CenterOfMassOffset").and_then(parse_real) {
            template.center_of_mass_offset = off;
        }
        if let Some(v) = module.attribute("AllowBouncing").and_then(parse_bool) {
            template.allow_bouncing = v;
        }
        if let Some(v) = module.attribute("AllowCollideForce").and_then(parse_bool) {
            template.allow_collide_force = v;
        }
        if let Some(v) = module
            .attribute("KillWhenRestingOnGround")
            .and_then(parse_bool)
        {
            template.kill_when_resting_on_ground = v;
        }
        if let Some(h) = module
            .attribute("MinFallHeightForDamage")
            .and_then(parse_real)
        {
            template.min_fall_speed_for_damage = (2.0 * h.abs()).sqrt();
        }
        if let Some(f) = module
            .attribute("FallHeightDamageFactor")
            .and_then(parse_real)
        {
            template.fall_height_damage_factor = f;
        }
    }


    /// C++ `ThingTemplate.cpp:201-205` / `Geometry.cpp:26-58`.
    /// Each Geometry* field writes independently; unknown tokens fail closed.
    fn apply_authored_geometry(template: &mut ThingTemplate, definition: &ObjectDefinition) {
        fn parse_real(value: &str) -> Option<f32> {
            let parsed = value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_end_matches('f')
                .parse::<f32>()
                .ok()?;
            parsed.is_finite().then_some(parsed)
        }
        fn parse_bool(value: &str) -> Option<bool> {
            match value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            }
        }

        let mut geom = template.geometry_info;
        if let Some(raw) = Self::object_definition_attr(definition, "geometry") {
            if let Some(ty) = crate::game_logic::HostGeometryType::from_ini(&raw) {
                geom.geom_type = ty;
                geom.authored = true;
            }
        }
        if let Some(v) = Self::object_definition_attr(definition, "geometrymajorradius")
            .as_deref()
            .and_then(parse_real)
        {
            geom.major_radius = v;
            geom.authored = true;
        }
        if let Some(v) = Self::object_definition_attr(definition, "geometryminorradius")
            .as_deref()
            .and_then(parse_real)
        {
            geom.minor_radius = v;
            geom.authored = true;
        }
        if let Some(v) = Self::object_definition_attr(definition, "geometryheight")
            .as_deref()
            .and_then(parse_real)
        {
            geom.height = v;
            geom.authored = true;
        }
        if let Some(v) =
            Self::object_definition_attr(definition, "geometryissmall").as_deref().and_then(parse_bool)
        {
            geom.is_small = v;
            geom.authored = true;
        }
        template.geometry_info = geom;
        if let Some(v) = Self::object_definition_attr(definition, "structurerubbleheight")
            .as_deref()
            .and_then(|raw| raw.trim().parse::<u8>().ok())
        {
            template.structure_rubble_height = v;
        }
        if let Some(v) = Self::object_definition_attr(definition, "fencewidth")
            .as_deref()
            .and_then(parse_real)
        {
            template.fence_width = v;
        }
        if let Some(v) = Self::object_definition_attr(definition, "fencexoffset")
            .as_deref()
            .and_then(parse_real)
        {
            template.fence_x_offset = v;
        }
    }
    /// C++ Object.cpp:160-497 builds weapons and modules from ThingTemplate
    /// INI data, not from unit-name residuals.  Capture the create-time
    /// policy bits `create_object_with_owner` used to hardcode.
    fn apply_authored_weapon_set_create_policy(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        if let Some(set) = definition
            .weapon_sets
            .iter()
            .find(|set| set.is_unconditional())
        {
            template.primary_auto_choose_none = set.auto_choose_primary_none();
            template.apply_weapon_set_definition(set);
        }
        template.apply_retail_button_only_auto_choose();
        template.has_fire_ocl_after_weapon_cooldown = definition.behavior_modules.iter().any(
            |module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("FireOCLAfterWeaponCooldownUpdate")
            },
        );
    }

    /// Retain the small exact data slice consumed by C++
    /// `ActionManager::canCaptureBuilding`: source capture SpecialPower,
    /// target CAPTURABLE/IMMUNE_TO_CAPTURE flags, and GarrisonContain state.
    ///
    /// The parser keeps fields per Behavior block, so a `SpecialPowerTemplate`
    /// from an unrelated module cannot accidentally enable capture.
    fn apply_authored_capture_metadata(
        template: &mut ThingTemplate,
        kind_of: &str,
        definition: &ObjectDefinition,
    ) {
        use crate::game_logic::CapturePowerKind;

        let has_kind = |token: &str| {
            kind_of
                .split(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, ',' | '|')
                })
                .any(|candidate| candidate.eq_ignore_ascii_case(token))
        };

        template.capturable = has_kind("capturable");
        template.immune_to_capture = has_kind("immune_to_capture");
        template.garrison_contain_max = definition
            .behavior_modules
            .iter()
            .find(|module| module.class_name.eq_ignore_ascii_case("GarrisonContain"))
            .and_then(|module| module.attribute("ContainMax"))
            .and_then(|value| value.trim().parse::<i64>().ok())
            .and_then(|value| usize::try_from(value).ok());

        // Only `SpecialAbility` grants the SpecialPower interface queried by
        // C++; a stray `SpecialAbilityUpdate` must not invent one.  Multiple
        // distinct capture powers on one object are malformed for this compact
        // host mapping, so reject rather than choose by module declaration.
        let mut power = CapturePowerKind::None;
        for module in &definition.behavior_modules {
            if !module.class_name.eq_ignore_ascii_case("SpecialAbility") {
                continue;
            }
            let candidate = module
                .attribute("SpecialPowerTemplate")
                .map(CapturePowerKind::from_special_power_template)
                .unwrap_or(CapturePowerKind::None);
            if candidate == CapturePowerKind::None {
                continue;
            }
            if power != CapturePowerKind::None && power != candidate {
                template.capture_power = CapturePowerKind::None;
                template.capture_starts_paused = false;
                template.capture_upgrade_trigger = None;
                template.capture_start_ability_range = None;
                template.capture_unpack_time_ms = None;
                template.capture_preparation_time_ms = None;
                template.capture_pack_time_ms = None;
                template.capture_pack_unpack_variation_factor = 0.0;
                template.capture_unpack_sound = None;
                template.capture_pack_sound = None;

                return;
            }
            power = candidate;
        }

        template.capture_power = power;
        template.capture_starts_paused = false;
        template.capture_upgrade_trigger = None;
        template.capture_start_ability_range = None;
        template.capture_unpack_time_ms = None;
        template.capture_preparation_time_ms = None;
        template.capture_pack_time_ms = None;
        template.capture_pack_unpack_variation_factor = 0.0;
        template.capture_unpack_sound = None;
        template.capture_pack_sound = None;

        if power == CapturePowerKind::None {
            return;
        }

        for module in &definition.behavior_modules {
            let module_power = module
                .attribute("SpecialPowerTemplate")
                .map(CapturePowerKind::from_special_power_template)
                .unwrap_or(CapturePowerKind::None);
            if module_power != power {
                continue;
            }
            if module.class_name.eq_ignore_ascii_case("SpecialAbility") {
                template.capture_starts_paused =
                    module.attribute("StartsPaused").is_some_and(|value| {
                        matches!(
                            value.trim().to_ascii_lowercase().as_str(),
                            "yes" | "true" | "1"
                        )
                    });
            } else if module
                .class_name
                .eq_ignore_ascii_case("SpecialAbilityUpdate")
            {
                template.capture_start_ability_range = module
                    .attribute("StartAbilityRange")
                    .and_then(|value| value.trim().parse::<f32>().ok())
                    .filter(|value| value.is_finite() && *value >= 0.0);
                template.capture_unpack_time_ms = module
                    .attribute("UnpackTime")
                    .and_then(|value| value.trim().parse::<i64>().ok())
                    .and_then(|value| u32::try_from(value).ok());
                template.capture_preparation_time_ms = module
                    .attribute("PreparationTime")
                    .and_then(|value| value.trim().parse::<i64>().ok())
                    .and_then(|value| u32::try_from(value).ok());
                template.capture_pack_time_ms = module
                    .attribute("PackTime")
                    .and_then(|value| value.trim().parse::<i64>().ok())
                    .and_then(|value| u32::try_from(value).ok());
                template.capture_pack_unpack_variation_factor = module
                    .attribute("PackUnpackVariationFactor")
                    .and_then(|value| value.trim().parse::<f32>().ok())
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .unwrap_or(0.0);
                template.capture_unpack_sound = module
                    .attribute("UnpackSound")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                template.capture_pack_sound = module
                    .attribute("PackSound")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);

            } else if module
                .class_name
                .eq_ignore_ascii_case("UnpauseSpecialPowerUpgrade")
            {
                template.capture_upgrade_trigger = module
                    .attribute("TriggeredBy")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
            }
        }

    }

    /// Seed templates for retail Object INI entries that the hand-authored
    /// bootstrap does not already cover.
    ///
    /// Starter templates retain their host behavior that generic Object INI
    /// fields do not yet represent.  Their authored Drawable `Scale` is still
    /// refreshed from retail data; additions use only exact object identity
    /// and authored attributes and do not imply an unavailable mesh or an
    /// unsupported behavior module has been ported.
    pub(in super::super) fn seed_asset_definition_templates(&mut self) -> usize {
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
    pub(in super::super) fn seed_asset_definition_templates_from_snapshot<I>(
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
    fn seed_asset_definition_templates_from_snapshot_with_models<I>(
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

            if Self::should_skip_map_object_template(name) {
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
            if model_name.is_none() && !is_audio_only {
                // The generic host template cannot represent an object whose
                // only identity is a behavior/draw module.  SoundAmbient-only
                // map objects are seeded so Drawable startAmbientSound can play.
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

    pub(in super::super) fn register_leftover_object_create_overrides_overlay() {
        game_engine::common::thing::register_object_create_overrides_live_overlay(
            overlay_leftover_object_create_overrides_to_live,
        );
    }

    pub(in super::super) fn apply_all_leftover_object_create_overrides(&mut self) -> usize {
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

    pub(in super::super) fn apply_pending_leftover_object_override(&mut self, template_name: &str) {
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

    pub(in super::super) fn apply_leftover_object_create_override(
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

    fn apply_object_definition_create_overrides(
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
            if lower.split_whitespace().any(|token| token == "structure" || token == "immobile")
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
        Self::apply_authored_overcharge_metadata(template, definition);
        Self::apply_authored_power_plant_update_metadata(template, definition);
        Self::apply_authored_temporary_weapon_behavior_metadata(template, definition);
        Self::apply_authored_physics_behavior_metadata(template, definition);
        Self::apply_authored_geometry(template, definition);
        Self::apply_authored_stealth_update_metadata(template, definition);
    }


    pub(in super::super) fn add_faction_structure_kind_bits(
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

    pub(in super::super) fn object_definition_attr(
        definition: &ObjectDefinition,
        key: &str,
    ) -> Option<String> {
        definition
            .attributes
            .iter()
            .find_map(|(attr, value)| attr.eq_ignore_ascii_case(key).then(|| value.clone()))
    }

    pub(in super::super) fn is_model_asset_available(model_name: &str) -> bool {
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

    pub(in super::super) fn resolve_spawn_model_name(model_name: &str) -> Option<String> {
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
    pub(in super::super) fn find_exact_available_model_name<I>(
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

    pub(in super::super) fn normalize_model_lookup_key(model_name: &str) -> String {
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
    pub(in super::super) fn build_fallback_template(template_name: &str) -> Option<ThingTemplate> {
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

    pub(in super::super) fn build_visual_fallback_template(
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

    /// Feed Main-crate object positions into ShroudManager.
    ///
    /// C++ Object::look/unlook: unlook previous looker then look at
    /// ShroudClearingRange on move/death. Do not add lookers every frame.
    pub(in super::super) fn update_main_crate_vision(&mut self) {
        use gamelogic::common::Coord3D;

        let shroud = get_shroud_manager();
        let mut shroud_mgr = match shroud.lock() {
            Ok(mgr) => mgr,
            Err(_) => return,
        };

        let persist = host_unlook_persist_frames();
        let frame = self.frame;
        shroud_mgr.process_pending_undo_shroud_reveals(frame);

        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for &pid in &player_ids {
            shroud_mgr.clear_host_object_visibility(pid);
        }

        let mut live_lookers = std::collections::HashSet::new();
        let mut live_reveal_all = std::collections::HashSet::new();
        let mut cell_ops: Vec<(Coord3D, f32, u32, bool)> = Vec::new();

        let snaps: Vec<_> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive())
            .map(|obj| {
                let pos = obj.get_position();
                let tpl = obj.get_template();
                let vision_range = if obj.vision_range > 0.0 {
                    obj.vision_range
                } else {
                    tpl.sight_range
                };
                // C++ Object::look has no UNDER_CONSTRUCTION branch on the
                // ally/owner reveal (Object.cpp:4938-4966). Use the stored
                // ShroudClearingRange, not the pick footprint.
                let mut shroud_range = if obj.shroud_clearing_range > 0.0 {
                    obj.shroud_clearing_range
                } else {
                    tpl.resolved_shroud_clearing_range()
                };
                if shroud_range < 0.0 {
                    shroud_range = vision_range;
                }
                let owner_pid = obj
                    .owner_player_id
                    .or_else(|| self.player_id_for_team(obj.team));
                let blocked = obj.contained_by.is_some_and(|cid| {
                    self.objects
                        .get(&cid)
                        .is_some_and(container_blocks_passenger_look)
                });
                let stealthed_hidden = obj.status.stealthed
                    && !obj.status.detected
                    && !obj.status.disguised;
                (
                    obj.id,
                    pos,
                    owner_pid,
                    shroud_range,
                    tpl.shroud_reveal_to_all_range,
                    tpl.reveal_to_all,
                    obj.status.under_construction,
                    blocked,
                    stealthed_hidden,
                )
            })
            .collect();

        for (
            id,
            pos,
            owner_pid,
            shroud_range,
            reveal_all_range,
            reveal_to_all_kind,
            under_construction,
            blocked,
            stealthed_hidden,
        ) in snaps
        {
            if blocked {
                continue;
            }
            let Some(owner_pid) = owner_pid else {
                continue;
            };
            let center = Coord3D::new(pos.x, pos.z, pos.y);

            if shroud_range > 0.0 {
                let player_mask = if reveal_to_all_kind {
                    player_ids
                        .iter()
                        .fold(0u32, |mask, &pid| mask | (1u32 << pid.min(31)))
                } else {
                    let mut mask = 0u32;
                    for &pid in &player_ids {
                        if self.player_relationship(owner_pid, pid)
                            == gamelogic::common::Relationship::Allies
                        {
                            mask |= 1u32 << pid.min(31);
                        }
                    }
                    mask
                };
                if player_mask != 0 {
                    restamp_host_partition_look(
                        &mut self.vision_last_looks,
                        &mut live_lookers,
                        &mut shroud_mgr,
                        &mut cell_ops,
                        id,
                        center,
                        shroud_range,
                        player_mask,
                        persist,
                        frame,
                    );
                }
            }

            if reveal_all_range > 0.0 && !under_construction && !stealthed_hidden {
                let mut reveal_mask = 0u32;
                for &pid in &player_ids {
                    let rel = self.player_relationship(owner_pid, pid);
                    if matches!(
                        rel,
                        gamelogic::common::Relationship::Enemies
                            | gamelogic::common::Relationship::Neutral
                    ) {
                        reveal_mask |= 1u32 << pid.min(31);
                    }
                }
                if reveal_mask != 0 {
                    restamp_host_partition_look(
                        &mut self.vision_last_reveal_all,
                        &mut live_reveal_all,
                        &mut shroud_mgr,
                        &mut cell_ops,
                        id,
                        center,
                        reveal_all_range,
                        reveal_mask,
                        persist,
                        frame,
                    );
                }
            }
        }

        unlook_stale_host_partition_looks(
            &mut self.vision_last_looks,
            &live_lookers,
            &mut shroud_mgr,
            &mut cell_ops,
            persist,
            frame,
        );
        unlook_stale_host_partition_looks(
            &mut self.vision_last_reveal_all,
            &live_reveal_all,
            &mut shroud_mgr,
            &mut cell_ops,
            persist,
            frame,
        );
        drop(shroud_mgr);
        for (center, radius, mask, add) in cell_ops {
            gamelogic::object::stamp_partition_cell_lookers(&center, radius, mask, add);
        }

        // C++ PartitionData::getShroudedStatus — object FOW is the footprint
        // COI mix, not a VisionRange circle (hq-mvlin).
        let Ok(mut shroud_mgr) = shroud.lock() else {
            return;
        };
        use crate::game_logic::partition_coi::{
            cells_touched_for_footprint, mix_object_shroud_from_cells, HostPartitionFootprint,
        };
        use game_engine::common::system::radar::CellShroudStatus;
        use gamelogic::common::{Relationship, types::ObjectShroudStatus};

        let object_snaps: Vec<_> = self
            .objects
            .values()
            .filter(|o| o.is_alive())
            .map(|o| {
                let pos = o.get_position();
                let geom = &o.thing.template.geometry_info;
                let fp = if geom.authored {
                    HostPartitionFootprint {
                        major_radius: geom.major_radius,
                        minor_radius: geom.minor_radius,
                        angle: o.get_orientation(),
                        is_small: geom.is_small,
                        is_box: matches!(
                            geom.geom_type,
                            crate::game_logic::HostGeometryType::Box
                        ),
                    }
                } else {
                    HostPartitionFootprint::small_circle(o.selection_radius.max(1.0))
                };
                (
                    o.id,
                    o.owner_player_id,
                    o.contained_by.is_some(),
                    o.is_kind_of(KindOf::Immobile) || o.is_kind_of(KindOf::Structure),
                    o.is_kind_of(KindOf::Mine),
                    o.get_template().always_visible,
                    pos.x,
                    pos.z,
                    fp,
                )
            })
            .collect();

        for (id, owner, contained, immobile, mine, always_visible, x, z, fp) in object_snaps {
            let cells = cells_touched_for_footprint(x, z, fp);
            for &pid in &player_ids {
                if always_visible || contained {
                    shroud_mgr.set_host_object_shroud_status(
                        pid,
                        id.0,
                        ObjectShroudStatus::Clear,
                    );
                    shroud_mgr.mark_host_object_seen(pid, id.0);
                    shroud_mgr.set_host_object_ever_seen(pid, id.0, true);
                    continue;
                }
                let mut shrouded_cells = 0usize;
                let mut fogged_cells = 0usize;
                for &(cx, cz) in &cells {
                    match gamelogic::object::partition_cell_shroud_status(pid as i32, cx, cz)
                    {
                        CellShroudStatus::Shrouded => shrouded_cells += 1,
                        CellShroudStatus::Fogged => fogged_cells += 1,
                        CellShroudStatus::Clear => {}
                    }
                }
                let ever = shroud_mgr.host_object_ever_seen(pid, id.0);
                let relationship_neutral = match owner {
                    Some(oid) => self.player_relationship(pid, oid) == Relationship::Neutral,
                    None => true,
                };
                let (status, ever_now) = mix_object_shroud_from_cells(
                    cells.len(),
                    shrouded_cells,
                    fogged_cells,
                    relationship_neutral,
                    immobile,
                    mine,
                    ever,
                );
                shroud_mgr.set_host_object_ever_seen(pid, id.0, ever_now);
                shroud_mgr.set_host_object_shroud_status(pid, id.0, status);
                match status {
                    ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear => {
                        shroud_mgr.mark_host_object_seen(pid, id.0);
                    }
                    ObjectShroudStatus::Fogged => {
                        shroud_mgr.mark_host_object_explored(pid, id.0);
                    }
                    _ => {}
                }
            }
        }

    }

    pub(in super::super) fn shroud_visibility_snapshot_for_team(
        &self,
        viewing_team: Team,
    ) -> Option<ShroudVisibilitySnapshot> {
        let player_id = self.player_id_for_team(viewing_team)?;
        let shroud_mgr = get_shroud_manager().lock().ok()?;
        let raw_visible_objects = shroud_mgr.get_visible_objects(player_id);

        // Match existing fail-open behavior while shroud has not produced runtime visibility yet.
        let runtime_active =
            shroud_mgr.get_last_update_frame() > 0 || !raw_visible_objects.is_empty();
        if !runtime_active {
            return None;
        }

        // Apply stealth-aware visibility to currently visible objects.
        let mut visible_objects = HashSet::with_capacity(raw_visible_objects.len());
        for object_id in raw_visible_objects {
            if shroud_mgr
                .can_see_object_with_stealth(player_id, object_id)
                .unwrap_or(true)
            {
                visible_objects.insert(object_id);
            }
        }

        Some(ShroudVisibilitySnapshot {
            visible_objects,
            explored_objects: shroud_mgr
                .get_explored_objects(player_id)
                .into_iter()
                .collect(),
        })
    }

    pub(in super::super) fn is_object_visible_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            snapshot.visible_objects.contains(&id) || snapshot.explored_objects.contains(&id)
        } else {
            true
        }
    }

    pub(in super::super) fn is_object_visible_on_minimap_for_team(
        object_id: ObjectId,
        object: &Object,
        viewing_team: Team,
        shroud_snapshot: Option<&ShroudVisibilitySnapshot>,
    ) -> bool {
        if !object.is_alive() || !object.is_visible_to_team(viewing_team) {
            return false;
        }

        if object.team == viewing_team {
            return true;
        }

        if let Some(snapshot) = shroud_snapshot {
            let id = object_id.0;
            if snapshot.visible_objects.contains(&id) {
                return true;
            }
            // Keep explored structures on minimap for strategic continuity.
            return object.is_kind_of(KindOf::Structure) && snapshot.explored_objects.contains(&id);
        }

        true
    }

    pub fn first_opponent_id(&self, player_id: u32) -> Option<u32> {
        self.players
            .values()
            .find(|player| player.id != player_id)
            .map(|player| player.id)
    }

    pub fn build_victory_summary(&self, winner_id: Option<u32>) -> VictorySummary {
        let mission_name = if self.map_loaded {
            Some(self.map_name.clone())
        } else {
            None
        };

        let duration = if self.sim_time_seconds > 0.0 {
            Some(Duration::from_secs_f32(self.sim_time_seconds))
        } else {
            None
        };

        let mut player_results = Vec::new();
        for player in self.players.values() {
            let outcome = match winner_id {
                Some(id) if id == player.id => PlayerOutcome::Won,
                Some(_) => PlayerOutcome::Lost,
                None => PlayerOutcome::Draw,
            };

            player_results.push(PlayerResult {
                player_id: player.id,
                player_name: player.name.clone(),
                faction: player.team,
                units_built: player.statistics.units_built,
                units_destroyed: player.statistics.units_destroyed,
                units_lost: player.statistics.units_lost,
                structures_built: player.statistics.structures_built,
                structures_destroyed: player.statistics.structures_destroyed,
                structures_lost: player.statistics.structures_lost,
                resources_collected: player.statistics.resources_collected,
                resources_spent: player.statistics.resources_spent,
                score: player.calculate_score().max(0) as u32,
                outcome,
            });
        }

        VictorySummary {
            mission_name,
            duration,
            player_results,
        }
    }

    pub(in super::super) fn setup_templates(&mut self) {
        log::debug!("Setting up comprehensive RTS unit templates");

        // ====== USA FACTION UNITS ======

        // USA Infantry
        let mut usa_ranger = ThingTemplate::new("USA_Ranger");
        usa_ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(60.0)
            .set_cost(80, 0)
            // AmericaInfantryRanger → AIRngr_SKN (W3DZH.big).
            .set_model("airngr_skn")
            .set_primary_weapon_name(super::super::weapon_bootstrap::RANGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(super::super::weapon_bootstrap::RANGER_SECONDARY_WEAPON)
            .set_locomotor_name(super::super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
        self.templates.insert("USA_Ranger".to_string(), usa_ranger);

        let mut usa_missile_defender = ThingTemplate::new("USA_MissileDefender");
        usa_missile_defender
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(300, 0)
            // AmericaInfantryMissileDefender → NITHNT_SKN (W3DZH.big).
            .set_model("nithnt_skn")
            .set_primary_weapon_name(
                super::super::weapon_bootstrap::MISSILE_DEFENDER_MISSILE_WEAPON,
            )
            .set_secondary_weapon_name(
                super::super::weapon_bootstrap::MISSILE_DEFENDER_LASER_GUIDED_WEAPON,
            );
        self.templates
            .insert("USA_MissileDefender".to_string(), usa_missile_defender);

        // USA Vehicles
        let mut usa_humvee = ThingTemplate::new("USA_Humvee");
        usa_humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(250.0)
            .set_cost(600, 0)
            .set_model("avhummer") // USA Humvee vehicle model
            .set_primary_weapon_name(super::super::weapon_bootstrap::HUMVEE_PRIMARY_WEAPON)
            .set_secondary_weapon_name(super::super::weapon_bootstrap::HUMVEE_SECONDARY_WEAPON)
            .set_locomotor_name(super::super::locomotor_bootstrap::HUMVEE_LOCOMOTOR);
        self.templates.insert("USA_Humvee".to_string(), usa_humvee);

        let mut usa_crusader = ThingTemplate::new("USA_CrusaderTank");
        usa_crusader
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(400.0)
            .set_cost(1200, 0)
            // AmericaTankCrusader → AVLeopard (W3DZH.big).
            .set_model("avleopard")
            .set_primary_weapon_name(super::super::weapon_bootstrap::CRUSADER_TANK_GUN)
            .set_locomotor_name(super::super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
        self.templates
            .insert("USA_CrusaderTank".to_string(), usa_crusader);

        let mut usa_paladin = ThingTemplate::new("USA_PaladinTank");
        usa_paladin
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(600.0)
            .set_cost(1800, 0)
            // AmericaTankPaladin → AVPaladin.  Do not draw the Crusader as a proxy.
            .set_model("avpaladin")
            .set_primary_weapon_name(super::super::weapon_bootstrap::PALADIN_TANK_GUN)
            .set_locomotor_name(super::super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
        self.templates
            .insert("USA_PaladinTank".to_string(), usa_paladin);

        // USA Aircraft
        let mut usa_raptor = ThingTemplate::new("USA_Raptor");
        usa_raptor
            // Retail `AmericaJetRaptor` declares both VEHICLE and AIRCRAFT.
            // `WeaponSet::getVictimAntiMask` classifies an airborne aircraft
            // through VEHICLE, so retaining only the presentation-facing
            // Aircraft bit makes it incorrectly untargetable.
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(180.0)
            .set_cost(1000, 0)
            // AmericaJetRaptor → AVRaptor (W3DZH.big).
            .set_model("avraptor")
            .set_primary_weapon_name(super::super::weapon_bootstrap::RAPTOR_JET_MISSILE_WEAPON);
        self.templates.insert("USA_Raptor".to_string(), usa_raptor);

        // ====== GLA FACTION UNITS ======

        // GLA Infantry
        let mut gla_soldier = ThingTemplate::new("GLA_Soldier");
        gla_soldier
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(50.0)
            .set_cost(60, 0)
            // GLAInfantryRebel → UIRGrd_SKN (W3DZH.big).
            .set_model("uirgrd_skn")
            .set_primary_weapon_name(super::super::weapon_bootstrap::GLA_REBEL_PRIMARY_WEAPON)
            .set_locomotor_name(super::super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
        self.templates
            .insert("GLA_Soldier".to_string(), gla_soldier);

        let mut gla_rpg = ThingTemplate::new("GLA_RPGTrooper");
        gla_rpg
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(60.0)
            .set_cost(100, 0)
            // GLAInfantryTunnelDefender → UITunF_SKN.  Do not draw a guard as a proxy.
            .set_model("uitunf_skn")
            .set_primary_weapon_name(super::super::weapon_bootstrap::TUNNEL_DEFENDER_ROCKET_WEAPON)
            .set_locomotor_name(super::super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
        self.templates.insert("GLA_RPGTrooper".to_string(), gla_rpg);

        // GLA Vehicles
        let mut gla_technical = ThingTemplate::new("GLA_Technical");
        gla_technical
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_cost(400, 0)
            // GLAVehicleTechnical → UVTechTrck, not a damaged ConditionState mesh.
            .set_model("uvtechtrck")
            .set_primary_weapon_name(super::super::weapon_bootstrap::TECHNICAL_MACHINE_GUN)
            .set_locomotor_name(super::super::locomotor_bootstrap::TECHNICAL_LOCOMOTOR);
        self.templates
            .insert("GLA_Technical".to_string(), gla_technical);

        let mut gla_scorpion = ThingTemplate::new("GLA_ScorpionTank");
        gla_scorpion
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_cost(900, 0)
            // GLATankScorpion → UVLiteTank (W3DZH.big).
            .set_model("uvlitetank")
            .set_locomotor_name(super::super::locomotor_bootstrap::SCORPION_LOCOMOTOR)
            .set_primary_weapon_name(super::super::weapon_bootstrap::SCORPION_TANK_GUN);
        self.templates
            .insert("GLA_ScorpionTank".to_string(), gla_scorpion);

        let mut gla_marauder = ThingTemplate::new("GLA_MarauderTank");
        gla_marauder
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(450.0)
            .set_cost(1400, 0)
            // GLATankMarauder → UVMarauder.  Do not draw a Scorpion as a proxy.
            .set_model("uvmarauder")
            .set_primary_weapon_name(super::super::weapon_bootstrap::MARAUDER_TANK_GUN)
            .set_locomotor_name(super::super::locomotor_bootstrap::SCORPION_LOCOMOTOR);
        self.templates
            .insert("GLA_MarauderTank".to_string(), gla_marauder);

        // C++ shell scripts and map logic still reference original INI object names.
        // Keep those aliases live so the simplified template table does not change behavior.
        if let Some(base) = self.templates.get("GLA_Soldier").cloned() {
            for alias in ["GLAInfantryRebel", "GLAInfantryTerrorist"] {
                let mut template = base.clone();
                template.name = alias.to_string();
                template.display_name = alias.to_string();
                if alias == "GLAInfantryTerrorist" {
                    // GLAInfantryTerrorist → UITRST_SKN.  The behavior
                    // scaffold remains deliberately curated, but its visual
                    // identity must not borrow the Rebel's mesh.
                    template.set_model("uitrst_skn");
                }
                self.templates.insert(alias.to_string(), template);
            }
        }
        if let Some(base) = self.templates.get("GLA_RPGTrooper").cloned() {
            let mut template = base.clone();
            template.name = "GLAInfantryTunnelDefender".to_string();
            template.display_name = "GLAInfantryTunnelDefender".to_string();
            self.templates
                .insert("GLAInfantryTunnelDefender".to_string(), template);
        }
        {
            let mut stinger = ThingTemplate::new("GLAInfantryStingerSoldier");
            stinger
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(crate::game_logic::host_base_defense::STINGER_SOLDIER_MAX_HEALTH)
                .set_cost(100, 0)
                .set_primary_weapon_name(
                    super::super::weapon_bootstrap::STINGER_PRIMARY_WEAPON,
                )
                .set_locomotor_name(super::super::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
            self.templates
                .insert("GLAInfantryStingerSoldier".to_string(), stinger);
        }
        if let Some(base) = self.templates.get("GLA_Technical").cloned() {
            let mut template = base;
            template.name = "GLAVehicleCombatBike".to_string();
            template.display_name = "GLAVehicleCombatBike".to_string();
            // GLAVehicleCombatBike → UVComBike, not the Technical chassis.
            template.set_model("uvcombike");
            self.templates
                .insert("GLAVehicleCombatBike".to_string(), template);
        }

        // ====== CHINA FACTION UNITS ======

        // China Infantry
        let mut china_infantry = ThingTemplate::new("China_RedGuard");
        china_infantry
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(55.0)
            .set_cost(70, 0)
            // ChinaInfantryRedguard → NICNSC_SKN.  Do not draw a Rebel as a proxy.
            .set_model("nicnsc_skn")
            .set_primary_weapon_name(super::super::weapon_bootstrap::REDGUARD_PRIMARY_WEAPON)
            .set_locomotor_name(super::super::locomotor_bootstrap::REDGUARD_LOCOMOTOR);
        self.templates
            .insert("China_RedGuard".to_string(), china_infantry);

        let mut china_tank_hunter = ThingTemplate::new("China_TankHunter");
        china_tank_hunter
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(110, 0)
            // ChinaInfantryTankHunter → NIMSST_SKN.  Do not draw a guard as a proxy.
            .set_model("nimsst_skn")
            .set_primary_weapon_name(super::super::weapon_bootstrap::TANK_HUNTER_PRIMARY_WEAPON);
        self.templates
            .insert("China_TankHunter".to_string(), china_tank_hunter);

        // China Vehicles
        let mut china_battlemaster = ThingTemplate::new("China_BattlemasterTank");
        china_battlemaster
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(360.0)
            .set_cost(1100, 0)
            // ChinaTankBattleMaster → NVBtMstr.  Do not draw a Scorpion as a proxy.
            .set_model("nvbtmstr")
            .set_primary_weapon_name(super::super::weapon_bootstrap::BATTLE_MASTER_TANK_GUN)
            .set_locomotor_name(super::super::locomotor_bootstrap::BATTLE_MASTER_LOCOMOTOR);
        self.templates
            .insert("China_BattlemasterTank".to_string(), china_battlemaster);

        let mut china_overlord = ThingTemplate::new("China_OverlordTank");
        china_overlord
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(700.0)
            .set_cost(2000, 0)
            // ChinaTankOverlord → NVOvrlrd (W3DZH.big).
            .set_model("nvovrlrd")
            .set_primary_weapon_name(super::super::weapon_bootstrap::OVERLORD_TANK_GUN);
        self.templates
            .insert("China_OverlordTank".to_string(), china_overlord);

        // China Inferno Cannon — residual FireFieldSmall DoT on shell impact.
        let mut china_inferno = ThingTemplate::new("China_InfernoCannon");
        china_inferno
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_cost(900, 0)
            // ChinaVehicleInfernoCannon → NVInferno (W3DZH.big).
            .set_model("nvinferno")
            .set_primary_weapon_name(super::super::weapon_bootstrap::INFERNO_CANNON_PRIMARY_WEAPON);
        self.templates
            .insert("China_InfernoCannon".to_string(), china_inferno.clone());
        // Retail INI name alias.
        {
            let mut alias = china_inferno;
            alias.name = "ChinaVehicleInfernoCannon".to_string();
            alias.display_name = "ChinaVehicleInfernoCannon".to_string();
            self.templates
                .insert("ChinaVehicleInfernoCannon".to_string(), alias);
        }

        // China Aircraft
        let mut china_mig = ThingTemplate::new("China_MiG");
        china_mig
            // Retail `ChinaJetMIG`: VEHICLE AIRCRAFT.
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_cost(900, 0)
            // ChinaJetMIG → NVMIG (W3DZH.big).
            .set_model("nvmig");
        self.templates.insert("China_MiG".to_string(), china_mig);

        let mut china_helix = ThingTemplate::new("China_Helix");
        china_helix
            // Retail `ChinaVehicleHelix`: VEHICLE AIRCRAFT.
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_cost(1200, 0)
            // ChinaVehicleHelix → NVHELIX.  Do not draw a Humvee as a proxy.
            .set_model("nvhelix");
        self.templates
            .insert("China_Helix".to_string(), china_helix);

        // ====== BUILDINGS (SHARED) ======

        let mut command_center = ThingTemplate::new("CommandCenter");
        command_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(2000.0)
            .set_cost(2000, 0)
            .set_model("abbtcmdhq"); // USA Command Center model - correct model name
        self.templates
            .insert("CommandCenter".to_string(), command_center);

        let mut supply_center = ThingTemplate::new("SupplyCenter");
        supply_center
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::SupplyCenter)
            .set_health(1000.0)
            .set_cost(1000, 0)
            .set_model("absupplyct"); // FactionBuilding.ini pristine USA supply center
        self.templates
            .insert("SupplyCenter".to_string(), supply_center);

        let mut power_plant = ThingTemplate::new("PowerPlant");
        power_plant
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::PowerPlant)
            .set_health(800.0)
            .set_cost(800, 0)
            .set_model("abpwrplant"); // FactionBuilding.ini pristine USA power plant
        self.templates.insert("PowerPlant".to_string(), power_plant);

        // CRITICAL: Add missing generic building templates that are referenced in the code
        // These templates ensure perfect alignment with C++ implementation expectations

        // Generic Barracks template (matches what's expected by the engine)
        let mut barracks = ThingTemplate::new("Barracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0)
            .set_cost(600, -1)
            .set_model("abbarracks"); // FactionBuilding.ini pristine USA barracks
        self.templates.insert("Barracks".to_string(), barracks);

        // Generic WarFactory template (matches what's expected by the engine)
        let mut war_factory = ThingTemplate::new("WarFactory");
        war_factory
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1500.0)
            .set_cost(1000, -2)
            .set_model("abwarfact"); // FactionBuilding.ini pristine USA war factory
        self.templates.insert("WarFactory".to_string(), war_factory);

        // Add faction-specific building templates for complete C++ alignment
        self.add_faction_building_templates();

        log::info!(
            "Set up {} comprehensive RTS unit templates covering all factions",
            self.templates.len()
        );
    }

    pub(in super::super) fn create_default_players(&mut self) {
        // If map-defined players already exist, keep them; otherwise seed defaults.
        if !self.players.is_empty() {
            return;
        }
        let player1 = Player::new(0, Team::USA, "USA Commander", true);
        let player2 = Player::new(1, Team::GLA, "GLA General", false);
        let player3 = Player::new(2, Team::China, "China Commander", false);

        self.players.insert(0, player1);
        self.players.insert(1, player2);
        self.players.insert(2, player3);

        log::info!(
            "Created {} default players for shell/skirmish bootstrap",
            self.players.len()
        );
    }

    pub(in super::super) fn create_test_map(&mut self) {
        // Wave 733: free demo test map army seed is opt-in only (default fail-closed).
        // Shares GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE with spawn_faction_base.
        let allow = std::env::var_os("GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE").is_some_and(|v| {
            let s = v.to_string_lossy();
            !(s.is_empty()
                || s == "0"
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"))
        });
        if !allow {
            return;
        }
        println!("🗺️ Creating comprehensive RTS test map with faction-aware bases...");

        let mut player_ids: Vec<u32> = self.players.keys().cloned().collect();
        player_ids.sort_unstable();
        let spawn_positions = [
            Vec3::new(-200.0, 0.0, -200.0),
            Vec3::new(200.0, 0.0, 200.0),
            Vec3::new(200.0, 0.0, -200.0),
            Vec3::new(-200.0, 0.0, 200.0),
        ];

        for (idx, player_id) in player_ids.iter().enumerate() {
            let team = self
                .players
                .get(player_id)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            let origin = spawn_positions.get(idx).cloned().unwrap_or(Vec3::ZERO);
            self.spawn_faction_base(team, origin);
        }

        // Neutral center props to mimic tech buildings and abandoned vehicles.
        println!("Adding neutral objectives in center...");
        self.create_object("OilDerrick", Team::Neutral, Vec3::new(0.0, 0.0, 0.0));
        self.create_object("OilRefinery", Team::Neutral, Vec3::new(50.0, 0.0, 0.0));
        self.create_object("TechHospital", Team::Neutral, Vec3::new(-50.0, 0.0, 50.0));
        self.create_object("USA_Humvee", Team::Neutral, Vec3::new(0.0, 0.0, 0.0));
        self.create_object("GLA_Technical", Team::Neutral, Vec3::new(20.0, 0.0, 20.0));

        println!(
            "✅ Comprehensive RTS test map created with {} objects across all factions!",
            self.objects.len()
        );

        // Demonstrate the RTS functionality
        self.demonstrate_rts_features();

        // Set up AI opponents for a proper skirmish match
        self.setup_skirmish_ai(0);

        // Demonstrate AI functionality
        self.demonstrate_ai_functionality();
    }

    pub(in super::super) fn spawn_faction_base(&mut self, team: Team, origin: Vec3) {
        // Wave 733: free demo faction army/base spawn is opt-in only (default fail-closed).
        // Not retail skirmish start — vertical-slice/demo harness may set
        // GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE=1.
        let allow = std::env::var_os("GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE").is_some_and(|v| {
            let s = v.to_string_lossy();
            !(s.is_empty()
                || s == "0"
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"))
        });
        if !allow {
            return;
        }
        println!("Creating {:?} base at {:?}", team, origin);
        match team {
            Team::USA => {
                self.create_object("CommandCenter", team, origin);
                self.create_object("SupplyCenter", team, origin + Vec3::new(50.0, 0.0, 50.0));
                self.create_object("PowerPlant", team, origin + Vec3::new(80.0, 0.0, 20.0));

                self.create_object("USA_Ranger", team, origin + Vec3::new(100.0, 0.0, 100.0));
                self.create_object("USA_Ranger", team, origin + Vec3::new(110.0, 0.0, 100.0));
                self.create_object("USA_Ranger", team, origin + Vec3::new(120.0, 0.0, 100.0));
                self.create_object(
                    "USA_MissileDefender",
                    team,
                    origin + Vec3::new(100.0, 0.0, 90.0),
                );
                self.create_object(
                    "USA_MissileDefender",
                    team,
                    origin + Vec3::new(110.0, 0.0, 90.0),
                );

                self.create_object("USA_Humvee", team, origin + Vec3::new(120.0, 0.0, 80.0));
                self.create_object("USA_Humvee", team, origin + Vec3::new(110.0, 0.0, 70.0));
                self.create_object(
                    "USA_CrusaderTank",
                    team,
                    origin + Vec3::new(140.0, 0.0, 60.0),
                );
                self.create_object(
                    "USA_PaladinTank",
                    team,
                    origin + Vec3::new(160.0, 0.0, 50.0),
                );

                self.create_object("USA_Raptor", team, origin + Vec3::new(180.0, 20.0, 40.0));
            }
            Team::GLA => {
                self.create_object("GLA_CommandCenter", team, origin);
                self.create_object("GLA_SupplyStash", team, origin + Vec3::new(0.0, 0.0, 50.0));
                self.create_object("GLA_ArmsDealer", team, origin + Vec3::new(30.0, 0.0, 20.0));

                self.create_object("GLA_Rebel", team, origin + Vec3::new(-10.0, 0.0, -10.0));
                self.create_object("GLA_Rebel", team, origin + Vec3::new(-20.0, 0.0, -10.0));
                self.create_object("GLA_Rebel", team, origin + Vec3::new(-30.0, 0.0, -10.0));
                self.create_object(
                    "GLA_RPGTrooper",
                    team,
                    origin + Vec3::new(-10.0, 0.0, -20.0),
                );
                self.create_object(
                    "GLA_RPGTrooper",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -20.0),
                );

                self.create_object("GLA_Technical", team, origin + Vec3::new(10.0, 0.0, -40.0));
                self.create_object("GLA_Technical", team, origin + Vec3::new(20.0, 0.0, -50.0));
                self.create_object(
                    "GLA_ScorpionTank",
                    team,
                    origin + Vec3::new(0.0, 0.0, -60.0),
                );
                self.create_object(
                    "GLA_MarauderTank",
                    team,
                    origin + Vec3::new(-10.0, 0.0, -60.0),
                );

                self.create_object(
                    "GLA_ScudLauncher",
                    team,
                    origin + Vec3::new(10.0, 0.0, 10.0),
                );
                self.create_object("GLA_Worker", team, origin + Vec3::new(-15.0, 0.0, -15.0));
                self.create_object("GLA_Worker", team, origin + Vec3::new(5.0, 0.0, -10.0));
            }
            Team::China => {
                self.create_object("China_CommandCenter", team, origin);
                self.create_object(
                    "China_SupplyCenter",
                    team,
                    origin + Vec3::new(30.0, 0.0, 30.0),
                );
                self.create_object(
                    "China_NuclearReactor",
                    team,
                    origin + Vec3::new(50.0, 0.0, 10.0),
                );

                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-30.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_RedGuard",
                    team,
                    origin + Vec3::new(-40.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_TankHunter",
                    team,
                    origin + Vec3::new(-20.0, 0.0, -30.0),
                );
                self.create_object(
                    "China_TankHunter",
                    team,
                    origin + Vec3::new(-30.0, 0.0, -30.0),
                );

                self.create_object(
                    "China_BattlemasterTank",
                    team,
                    origin + Vec3::new(20.0, 0.0, -20.0),
                );
                self.create_object(
                    "China_BattlemasterTank",
                    team,
                    origin + Vec3::new(10.0, 0.0, -10.0),
                );
                self.create_object(
                    "China_OverlordTank",
                    team,
                    origin + Vec3::new(40.0, 0.0, -40.0),
                );
                self.create_object(
                    "China_DragonTank",
                    team,
                    origin + Vec3::new(30.0, 0.0, -50.0),
                );
                self.create_object(
                    "China_GatlingTank",
                    team,
                    origin + Vec3::new(20.0, 0.0, -60.0),
                );

                self.create_object("China_MiG", team, origin + Vec3::new(60.0, 20.0, -30.0));
                self.create_object("China_Helix", team, origin + Vec3::new(40.0, 25.0, -20.0));
            }
            Team::Neutral => {
                self.create_object("CommandCenter", team, origin);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retail_service_and_dozer_kindof_tags_survive_object_ini_parsing() {
        use std::path::Path;

        // Parse the actual retail definitions rather than a name-shaped
        // fixture.  C++ ActionManager distinguishes these exact KindOf tags:
        // AmericaBarracks is a HEAL_PAD, AmericaWarFactory a REPAIR_PAD,
        // AmericaAirfield an FS_AIRFIELD, and the construction unit a DOZER.
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        let america_vehicles =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/AmericaVehicle.ini",
            ))
            .expect("retail AmericaVehicle.ini");

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail faction structures");
        parser
            .parse_ini_content(&america_vehicles, "AmericaVehicle.ini")
            .expect("parse retail USA vehicles");

        let parsed = |name| {
            GameLogic::build_template_from_object_definition(
                name,
                parser
                    .get_definition(name)
                    .unwrap_or_else(|| panic!("missing retail Object {name}")),
                None,
            )
        };
        assert!(parsed("AmericaAirfield").is_kind_of(KindOf::FSAirfield));
        assert!(parsed("AmericaBarracks").is_kind_of(KindOf::HealPad));
        assert!(parsed("AmericaWarFactory").is_kind_of(KindOf::RepairPad));
        assert!(parsed("AmericaVehicleDozer").is_kind_of(KindOf::Dozer));
    }

    #[test]
    fn retail_hack_internet_metadata_drives_field_and_contained_income() {
        use glam::Vec3;
        use std::path::Path;

        // This is deliberately a retail parser-to-runtime proof, rather than
        // a `TestHacker` basename fixture.  ChinaInfantryHacker carries the
        // exact HackInternetAIUpdate and MONEY_HACKER KindOf; ChinaInternetCenter
        // carries InternetHackContain's eight transport slots and one-kind
        // admission mask.
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let china_infantry =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/ChinaInfantry.ini",
            ))
            .expect("retail ChinaInfantry.ini");
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&china_infantry, "ChinaInfantry.ini")
            .expect("parse retail China infantry");
        parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail faction buildings");

        let hacker_template = GameLogic::build_template_from_object_definition(
            "ChinaInfantryHacker",
            parser
                .get_definition("ChinaInfantryHacker")
                .expect("retail China hacker definition"),
            None,
        );
        let hacker_data = hacker_template
            .hack_internet_ai_update
            .expect("retail HackInternetAIUpdate metadata");
        assert!(hacker_template.is_kind_of(KindOf::MoneyHacker));
        assert_eq!(hacker_template.transport_slot_count, Some(1));
        assert_eq!(hacker_data.unpack_time_frames, 219);
        assert_eq!(hacker_data.pack_time_frames, 154);
        assert_eq!(hacker_data.cash_update_delay_frames, 60);
        assert_eq!(hacker_data.cash_update_delay_fast_frames, 54);
        assert_eq!(hacker_data.regular_cash_amount, 5);
        assert_eq!(hacker_data.veteran_cash_amount, 6);
        assert_eq!(hacker_data.elite_cash_amount, 8);
        assert_eq!(hacker_data.heroic_cash_amount, 10);
        assert!((hacker_data.xp_per_cash_update - 1.0).abs() < f32::EPSILON);

        let center_template = GameLogic::build_template_from_object_definition(
            "ChinaInternetCenter",
            parser
                .get_definition("ChinaInternetCenter")
                .expect("retail China Internet Center definition"),
            None,
        );
        assert_eq!(
            center_template.contain_module.kind,
            ContainModuleKind::InternetHack
        );
        assert_eq!(center_template.contain_module.slots, Some(8));
        assert_eq!(
            center_template.contain_module.admission,
            ContainAdmission::MoneyHackerOnly
        );

        let mut logic = GameLogic::new();
        let mut china = Player::new(1, Team::China, "China", true);
        china.resources.supplies = 0;
        logic.add_player(china);
        logic
            .templates
            .insert("ChinaInfantryHacker".to_string(), hacker_template);
        logic
            .templates
            .insert("ChinaInternetCenter".to_string(), center_template);

        let field_hacker = logic
            .create_object_for_player("ChinaInfantryHacker", 1, Vec3::new(16.0, 0.0, 0.0))
            .expect("field hacker");
        let contained_hacker = logic
            .create_object_for_player("ChinaInfantryHacker", 1, Vec3::new(1.0, 0.0, 0.0))
            .expect("contained hacker");
        let center = logic
            .create_object_for_player("ChinaInternetCenter", 1, Vec3::ZERO)
            .expect("Internet Center");

        // This checks the frozen input authority and the arrival-side
        // containment bookkeeping together: a generic infantry unit cannot
        // stand in for the parsed MONEY_HACKER contract.
        assert!(logic.can_unit_enter_normal_target(contained_hacker, center));
        assert!(logic
            .host_object_mut(center)
            .expect("Internet Center object")
            .add_occupant(contained_hacker));
        logic
            .host_object_mut(contained_hacker)
            .expect("contained hacker object")
            .set_contained_by(Some(center));

        logic.frame = 0;
        assert!(logic.start_hacker_internet_hack(field_hacker));
        logic.update_hacker_income();
        assert!(logic.hacker_income().is_hacking(contained_hacker));
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);

        // UNPACKING (219) then CashUpdateDelay, then decrement-fire.
        // Contained uses fast delay 54 → 219+54+1=274; field 219+60+1=280.
        logic.frame = 273;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);
        logic.frame = 274;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
        assert_eq!(logic.get_player(1).unwrap().statistics.money_earned, 5);
        logic.frame = 279;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
        logic.frame = 280;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 10);
        assert_eq!(logic.get_player(1).unwrap().statistics.money_earned, 10);
    }

    #[test]
    fn retail_hacker_disable_pair_uses_exact_power_identity_not_microwave_alias() {
        use std::collections::HashMap;
        use std::path::Path;

        // The live boot path owns this global store.  Focused unit tests do
        // not run the shell bootstrap, so seed only the two actual retail
        // records if they are absent.  Both deliberately share Common's C++
        // enum; the parser below must still admit HDB alone by canonical
        // SpecialPowerTemplate identity.
        {
            use game_engine::common::rts::special_power::get_special_power_store_mut;

            let mut powers = get_special_power_store_mut();
            for (name, reload) in [
                ("SpecialAbilityHackerDisableBuilding", "500"),
                ("SpecialAbilityMicrowaveDisableBuilding", "4000"),
            ] {
                if powers.find_template(name).is_none() {
                    let mut fields = HashMap::new();
                    fields.insert(
                        "Enum".to_string(),
                        "SPECIAL_HACKER_DISABLE_BUILDING".to_string(),
                    );
                    fields.insert("ReloadTime".to_string(), reload.to_string());
                    fields.insert("PublicTimer".to_string(), "No".to_string());
                    powers
                        .parse_special_power_definition(name, &fields)
                        .unwrap_or_else(|error| {
                            panic!("seed required retail special power {name}: {error}")
                        });
                }
            }
        }

        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let special_power = std::fs::read_to_string(
            repo_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/SpecialPower.ini"),
        )
        .expect("retail SpecialPower.ini");
        assert!(special_power.contains("SpecialPower SpecialAbilityHackerDisableBuilding"));
        assert!(special_power.contains("SpecialPower SpecialAbilityMicrowaveDisableBuilding"));
        assert!(special_power.contains("ReloadTime        = 500"));
        assert!(special_power.contains("ReloadTime        = 4000"));

        let china_infantry =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/ChinaInfantry.ini",
            ))
            .expect("retail ChinaInfantry.ini");
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&china_infantry, "ChinaInfantry.ini")
            .expect("parse retail China hacker");
        // Microwave shares the C++ HDB enum and must expose the same
        // disable-building channel, keyed by its own SpecialPowerTemplate.
        parser
            .parse_ini_content(
                r#"
Object MicrowaveAliasProbe
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = SpecialAbility ModuleTag_Microwave
    SpecialPowerTemplate = SpecialAbilityMicrowaveDisableBuilding
    UpdateModuleStartsAttack = Yes
  End
  Behavior = SpecialAbilityUpdate ModuleTag_MicrowaveUpdate
    SpecialPowerTemplate = SpecialAbilityMicrowaveDisableBuilding
    StartAbilityRange = 150.0
    UnpackTime = 1
    PreparationTime = 1
    PersistentPrepTime = 1
    EffectDuration = 1
    PackTime = 1
  End
End
"#,
                "microwave_alias_probe.ini",
            )
            .expect("parse microwave alias probe");

        let hacker = GameLogic::build_template_from_object_definition(
            "ChinaInfantryHacker",
            parser
                .get_definition("ChinaInfantryHacker")
                .expect("retail China hacker"),
            None,
        );
        let hdb = hacker
            .hacker_disable_building
            .expect("retail HDB pair must survive parser");
        assert_eq!(
            hdb.special_power_template,
            "SpecialAbilityHackerDisableBuilding"
        );
        assert_eq!(hdb.reload_time_frames, 15);
        assert_eq!(hdb.start_ability_range, 150.0);
        assert_eq!(hdb.unpack_time_ms, 7_300);
        assert_eq!(hdb.preparation_time_ms, 3_000);
        assert_eq!(hdb.persistent_prep_time_ms, 333);
        assert_eq!(hdb.effect_duration_ms, 2_000);
        assert_eq!(hdb.pack_time_ms, 5_133);
        assert!(!hdb.persistence_requires_recharge);

        let microwave = GameLogic::build_template_from_object_definition(
            "MicrowaveAliasProbe",
            parser
                .get_definition("MicrowaveAliasProbe")
                .expect("microwave alias probe"),
            None,
        );
        let microwave_hdb = microwave
            .hacker_disable_building
            .expect("Microwave SPECIAL_HACKER_DISABLE_BUILDING pair must expose the disable channel");
        assert_eq!(
            microwave_hdb.special_power_template,
            "SpecialAbilityMicrowaveDisableBuilding"
        );
        assert_eq!(
            microwave_hdb.command_power(),
            crate::command_system::SpecialPowerType::MicrowaveDisableBuilding
        );
        assert!(!microwave_hdb.is_hacker_command());
        assert_eq!(microwave_hdb.reload_time_frames, 120);
        assert_eq!(microwave_hdb.start_ability_range, 150.0);
        assert_eq!(microwave_hdb.unpack_time_ms, 1);
        assert_eq!(microwave_hdb.preparation_time_ms, 1);
        assert_eq!(microwave_hdb.persistent_prep_time_ms, 1);
        assert_eq!(microwave_hdb.effect_duration_ms, 1);
        assert_eq!(microwave_hdb.pack_time_ms, 1);
    }

    #[test]
    fn retail_superweapon_modules_drive_arm_limit_energy_and_presentation_not_names() {
        use glam::Vec3;
        use std::collections::HashMap;
        use std::path::Path;

        // The fixture uses the actual retail FactionBuilding declarations and
        // only seeds the corresponding loaded SpecialPower records when the
        // wider shell bootstrap has not already done so.
        {
            use game_engine::common::rts::special_power::get_special_power_store_mut;

            let mut powers = get_special_power_store_mut();
            for (name, enum_name, reload) in [
                (
                    "SuperweaponParticleUplinkCannon",
                    "SPECIAL_PARTICLE_UPLINK_CANNON",
                    "240000",
                ),
                ("SuperweaponScudStorm", "SPECIAL_SCUD_STORM", "300000"),
                (
                    "SuperweaponNeutronMissile",
                    "SPECIAL_NEUTRON_MISSILE",
                    "360000",
                ),
            ] {
                if powers.find_template(name).is_none() {
                    let mut fields = HashMap::new();
                    fields.insert("Enum".to_string(), enum_name.to_string());
                    fields.insert("ReloadTime".to_string(), reload.to_string());
                    fields.insert("PublicTimer".to_string(), "Yes".to_string());
                    powers
                        .parse_special_power_definition(name, &fields)
                        .unwrap_or_else(|error| {
                            panic!("seed required retail special power {name}: {error}")
                        });
                }
            }
        }

        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail faction structures");
        parser
            .parse_ini_content(
                r#"
Object AmericaParticleCannonUplinkNamedButNoModule
  Type = Structure
  KindOf = STRUCTURE SELECTABLE POWERED FS_SUPERWEAPON
End
"#,
                "superweapon_name_spoof.ini",
            )
            .expect("parse name spoof");

        let parsed = |name| {
            GameLogic::build_template_from_object_definition(
                name,
                parser
                    .get_definition(name)
                    .unwrap_or_else(|| panic!("missing retail Object {name}")),
                None,
            )
        };
        let particle = parsed("AmericaParticleCannonUplink");
        let scud = parsed("GLAScudStorm");
        let nuke = parsed("ChinaNuclearMissileLauncher");
        let spoof = parsed("AmericaParticleCannonUplinkNamedButNoModule");

        fn first_power_name(template: &ThingTemplate) -> Option<&str> {
            template
                .special_power_modules
                .first()
                .map(|module| module.special_power_template.as_str())
        }
        assert_eq!(
            first_power_name(&particle),
            Some("SuperweaponParticleUplinkCannon")
        );
        assert_eq!(first_power_name(&scud), Some("SuperweaponScudStorm"));
        assert_eq!(first_power_name(&nuke), Some("SuperweaponNeutronMissile"));
        assert!(particle.special_power_modules[0].module_tag.is_some());
        assert_eq!(particle.energy_production, Some(-10));
        assert_eq!(scud.energy_production, Some(0));
        assert_eq!(nuke.energy_production, Some(-10));
        assert_eq!(
            particle.max_simultaneous_link_key.as_deref(),
            Some("Superweapon")
        );
        assert!(particle.max_simultaneous_determined_by_superweapon_restriction);
        assert!(spoof.special_power_modules.is_empty());
        assert_eq!(spoof.energy_production, None);
        assert!(!spoof.has_superweapon_restriction_link_key());

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.templates.insert(particle.name.clone(), particle);
        logic.templates.insert(scud.name.clone(), scud);
        logic.templates.insert(nuke.name.clone(), nuke);
        logic.templates.insert(spoof.name.clone(), spoof);
        logic.skirmish_rules.limit_superweapons = true;

        let particle_id = logic
            .create_object_for_player("AmericaParticleCannonUplink", 1, Vec3::ZERO)
            .expect("retail particle structure");
        let spoof_id = logic
            .create_object_for_player(
                "AmericaParticleCannonUplinkNamedButNoModule",
                1,
                Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("name spoof structure");
        use crate::command_system::SpecialPowerType as P;
        assert!(logic
            .host_object(particle_id)
            .expect("particle object")
            .special_power_cooldowns
            .contains_key(&P::ParticleCannon));
        assert!(logic
            .host_object(spoof_id)
            .expect("spoof object")
            .special_power_cooldowns
            .is_empty());
        assert!(!logic.can_start_superweapon_building_for_player(1, "AmericaParticleCannonUplink"));
        assert!(logic.can_start_superweapon_building_for_player(
            1,
            "AmericaParticleCannonUplinkNamedButNoModule"
        ));

        let particle_object = logic.host_object_mut(particle_id).expect("particle object");
        particle_object
            .special_power_cooldowns
            .remove(&P::ParticleCannon);
        particle_object.special_power_cooldown_remaining = 0.0;
        particle_object.refresh_special_power_aggregate_cooldown();
        assert!(logic.is_special_power_ready_for(particle_id, &P::ParticleCannon));
        assert!(!logic.is_special_power_ready_for(spoof_id, &P::ParticleCannon));
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
        let particle_presentation = frame
            .objects
            .iter()
            .find(|object| object.id == particle_id)
            .expect("particle presentation");
        assert_eq!(
            particle_presentation
                .special_power_ready_template_name
                .as_deref(),
            Some("SuperweaponParticleUplinkCannon")
        );
        assert!(frame
            .objects
            .iter()
            .find(|object| object.id == spoof_id)
            .expect("spoof presentation")
            .special_power_ready_template_name
            .is_none());

        logic.select_objects(1, vec![particle_id]);
        let particle_frame =
            crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
        assert!(particle_frame.unit_command_buttons().iter().any(|button| {
            let n = button.command_name.to_ascii_lowercase();
            n.contains("particle") && button.enabled
        }));
        logic.select_objects(1, vec![spoof_id]);
        let spoof_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
        assert!(!spoof_frame.unit_command_buttons().iter().any(|button| {
            button
                .command_name
                .to_ascii_lowercase()
                .contains("particle")
        }));
    }

    #[test]
    fn parsed_parking_place_behavior_keeps_authored_shape_without_name_fallback() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryParkingIdentity
  Type = Structure
  Model = ArbitraryParkingModel
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
  Behavior = ParkingPlaceBehavior ModuleTag_Parking
    NumRows = 3
    NumCols = 2
    ApproachHeight = 72.5
    LandingDeckHeightOffset = 4.25
    HasRunways = Yes
    ParkInHangars = Yes
    HealAmountPerSecond = 7.5
  End
End
Object AirfieldNamedButNoParkingBehavior
  Type = Structure
  Model = NoParkingModel
  KindOf = STRUCTURE SELECTABLE FS_AIRFIELD
End
"#,
                "parking_place_metadata_probe.ini",
            )
            .expect("parse parking place metadata probe");

        let parsed = GameLogic::build_template_from_object_definition(
            "ArbitraryParkingIdentity",
            parser
                .get_definition("ArbitraryParkingIdentity")
                .expect("parking definition"),
            None,
        );
        let metadata = parsed.parking_place.expect("authored parking metadata");
        assert_eq!(metadata.num_rows, 3);
        assert_eq!(metadata.num_cols, 2);
        assert_eq!(metadata.capacity(), Some(6));
        assert_eq!(metadata.runway_count(), Some(2));
        assert!((metadata.approach_height - 72.5).abs() < f32::EPSILON);
        assert!((metadata.landing_deck_height_offset - 4.25).abs() < f32::EPSILON);
        assert!(metadata.has_runways);
        assert!(metadata.park_in_hangars);
        assert!((metadata.heal_amount_per_second - 7.5).abs() < f32::EPSILON);

        let no_behavior = GameLogic::build_template_from_object_definition(
            "AirfieldNamedButNoParkingBehavior",
            parser
                .get_definition("AirfieldNamedButNoParkingBehavior")
                .expect("no-behavior definition"),
            None,
        );
        assert!(no_behavior.is_kind_of(KindOf::FSAirfield));
        assert!(
            no_behavior.parking_place.is_none(),
            "FSAirfield/name alone must not fabricate ParkingPlaceBehavior"
        );
    }

    #[test]
    fn parsed_rebuild_hole_expose_die_keeps_authored_hole_name() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object GLAScudStorm
  KindOf = STRUCTURE
  Behavior = RebuildHoleExposeDie ModuleTag_Hole
    HoleName = GLAScudStormRebuildHole
    HoleMaxHealth = 500.0
    TransferAttackers = Yes
  End
End
Object AmericaCommandCenter
  KindOf = STRUCTURE COMMANDCENTER
End
"#,
            )
            .expect("parse rebuild hole expose probe");
        let scud = GameLogic::build_template_from_object_definition(
            "GLAScudStorm",
            parser
                .get_definition("GLAScudStorm")
                .expect("scud definition"),
            None,
        );
        let expose = scud
            .rebuild_hole_expose
            .expect("authored RebuildHoleExposeDie");
        assert_eq!(expose.hole_name, "GLAScudStormRebuildHole");
        assert!((expose.hole_max_health - 500.0).abs() < f32::EPSILON);
        assert!(expose.transfer_attackers);

        let usa = GameLogic::build_template_from_object_definition(
            "AmericaCommandCenter",
            parser
                .get_definition("AmericaCommandCenter")
                .expect("usa cc definition"),
            None,
        );
        assert!(
            usa.rebuild_hole_expose.is_none(),
            "USA CC name/command KindOf must not fabricate a hole module"
        );
    }

    #[test]
    fn parsed_max_simultaneous_of_type_reads_numeric_and_restriction() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object AmericaWarFactory
  KindOf = STRUCTURE
  MaxSimultaneousOfType = 1
End
Object AmericaInfantryColonelBurton
  KindOf = INFANTRY
  MaxSimultaneousOfType = 1
End
Object AmericaParticleCannonUplink
  KindOf = STRUCTURE
  MaxSimultaneousOfType = DeterminedBySuperweaponRestriction
  MaxSimultaneousLinkKey = Superweapon
End
Object AmericaInfantryRanger
  KindOf = INFANTRY
End
"#,
                "max_simultaneous_probe.ini",
            )
            .expect("parse MaxSimultaneous probe");
        let factory = GameLogic::build_template_from_object_definition(
            "AmericaWarFactory",
            parser
                .get_definition("AmericaWarFactory")
                .expect("factory"),
            None,
        );
        assert_eq!(factory.max_simultaneous_of_type, 1);
        assert!(!factory.max_simultaneous_determined_by_superweapon_restriction);

        let burton = GameLogic::build_template_from_object_definition(
            "AmericaInfantryColonelBurton",
            parser
                .get_definition("AmericaInfantryColonelBurton")
                .expect("burton"),
            None,
        );
        assert_eq!(burton.max_simultaneous_of_type, 1);

        let particle = GameLogic::build_template_from_object_definition(
            "AmericaParticleCannonUplink",
            parser
                .get_definition("AmericaParticleCannonUplink")
                .expect("puc"),
            None,
        );
        assert_eq!(particle.max_simultaneous_of_type, 0);
        assert!(particle.max_simultaneous_determined_by_superweapon_restriction);
        assert_eq!(
            particle.max_simultaneous_link_key.as_deref(),
            Some("Superweapon")
        );

        let ranger = GameLogic::build_template_from_object_definition(
            "AmericaInfantryRanger",
            parser
                .get_definition("AmericaInfantryRanger")
                .expect("ranger"),
            None,
        );
        assert_eq!(ranger.max_simultaneous_of_type, 0);
        assert!(!ranger.max_simultaneous_determined_by_superweapon_restriction);
    }

    #[test]
    fn parsed_flight_deck_behavior_keeps_authored_shape_without_name_fallback() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryCarrierIdentity
  Type = Vehicle
  Model = ArbitraryCarrierModel
  KindOf = VEHICLE SELECTABLE AIRCRAFT_CARRIER
  Behavior = FlightDeckBehavior ModuleTag_Deck
    NumRunways = 2
    NumSpacesPerRunway = 4
    ApproachHeight = 50
    LandingDeckHeightOffset = 22
    HealAmountPerSecond = 10
    ParkingCleanupPeriod = 1000
    HumanFollowPeriod = 500
    PayloadTemplate = AircraftCarrierRaptor
    ReplacementDelay = 10000
    DockAnimationDelay = 2000
    LaunchWaveDelay = 1500
    LaunchRampDelay = 1000
    LowerRampDelay = 2000
    CatapultFireDelay = 500
    Runway1CatapultSystem = AircraftCarrierCatapultSteam
  End
End
Object CarrierNamedButNoDeckBehavior
  Type = Vehicle
  Model = NoDeckModel
  KindOf = VEHICLE SELECTABLE AIRCRAFT_CARRIER
End
"#,
                "flight_deck_metadata_probe.ini",
            )
            .expect("parse flight deck metadata probe");

        let parsed = GameLogic::build_template_from_object_definition(
            "ArbitraryCarrierIdentity",
            parser
                .get_definition("ArbitraryCarrierIdentity")
                .expect("carrier definition"),
            None,
        );
        let metadata = parsed.flight_deck.expect("authored flight deck metadata");
        assert_eq!(metadata.num_rows, 4);
        assert_eq!(metadata.num_cols, 2);
        assert_eq!(metadata.capacity(), Some(8));
        assert_eq!(metadata.payload_template, "AircraftCarrierRaptor");
        assert!((metadata.approach_height - 50.0).abs() < f32::EPSILON);
        assert!((metadata.landing_deck_height_offset - 22.0).abs() < f32::EPSILON);
        assert_eq!(metadata.cleanup_frames, 30);
        assert_eq!(metadata.human_follow_frames, 15);
        assert_eq!(metadata.replacement_frames, 300);
        assert_eq!(metadata.dock_animation_frames, 60);
        assert_eq!(metadata.launch_wave_frames, 45);
        assert_eq!(metadata.launch_ramp_frames, 30);
        assert_eq!(metadata.lower_ramp_frames, 60);
        assert_eq!(metadata.catapult_fire_frames, 15);
        assert_eq!(
            metadata.catapult_system[0].as_deref(),
            Some("AircraftCarrierCatapultSteam")
        );

        let no_behavior = GameLogic::build_template_from_object_definition(
            "CarrierNamedButNoDeckBehavior",
            parser
                .get_definition("CarrierNamedButNoDeckBehavior")
                .expect("no-behavior definition"),
            None,
        );
        assert!(
            no_behavior.flight_deck.is_none(),
            "carrier KindOf/name alone must not fabricate FlightDeckBehavior"
        );
    }


    #[test]
    fn retail_deploy_style_modules_preserve_authored_timings_and_flags() {
        // These are the source-authored DeployStyle blocks for the retail
        // Sentry Drone and Nuke Launcher.  Sentry's nested Turret is
        // intentional: its following fields prove module attribution remains
        // intact through a nested `End`.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object AmericaVehicleSentryDrone
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = DeployStyleAIUpdate ModuleTag_04
    Turret
      TurretTurnRate = 180
    End
    PackTime = 1000
    UnpackTime = 1000
    TurretsFunctionOnlyWhenDeployed = Yes
    TurretsMustCenterBeforePacking = Yes
  End
End
Object ChinaVehicleNukeLauncher
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = DeployStyleAIUpdate ModuleTag_04
    Turret
      TurretTurnRate = 80
    End
    PackTime = 3333
    UnpackTime = 3333
    ResetTurretBeforePacking = No
    TurretsFunctionOnlyWhenDeployed = Yes
    TurretsMustCenterBeforePacking = Yes
    ManualDeployAnimations = Yes
  End
End
Object VehicleWithoutDeployStyle
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
End
"#,
                "retail_deploy_style_probe.ini",
            )
            .expect("parse retail DeployStyle probes");

        let sentry = GameLogic::build_template_from_object_definition(
            "AmericaVehicleSentryDrone",
            parser
                .get_definition("AmericaVehicleSentryDrone")
                .expect("sentry definition"),
            None,
        );
        let sentry_data = sentry
            .deploy_style_metadata
            .as_ref()
            .expect("sentry authored DeployStyle");
        assert_eq!(sentry_data.pack_time_frames, 30);
        assert_eq!(sentry_data.unpack_time_frames, 30);
        assert!(!sentry_data.reset_turret_before_packing);
        assert!(sentry_data.turrets_function_only_when_deployed);
        assert!(sentry_data.turrets_must_center_before_packing);
        assert!(!sentry_data.manual_deploy_animations);

        let nuke = GameLogic::build_template_from_object_definition(
            "ChinaVehicleNukeLauncher",
            parser
                .get_definition("ChinaVehicleNukeLauncher")
                .expect("nuke definition"),
            None,
        );
        let nuke_data = nuke
            .deploy_style_metadata
            .as_ref()
            .expect("nuke authored DeployStyle");
        assert_eq!(nuke_data.pack_time_frames, 100);
        assert_eq!(nuke_data.unpack_time_frames, 100);
        assert!(!nuke_data.reset_turret_before_packing);
        assert!(nuke_data.turrets_function_only_when_deployed);
        assert!(nuke_data.turrets_must_center_before_packing);
        assert!(nuke_data.manual_deploy_animations);

        let ordinary = GameLogic::build_template_from_object_definition(
            "VehicleWithoutDeployStyle",
            parser
                .get_definition("VehicleWithoutDeployStyle")
                .expect("ordinary vehicle definition"),
            None,
        );
        assert!(ordinary.is_kind_of(KindOf::Vehicle));
        assert!(
            ordinary.deploy_style_metadata.is_none(),
            "VEHICLE KindOf/name must not synthesize DeployStyle authority"
        );
    }

    #[test]
    fn production_exit_modules_preserve_queue_and_default_authority() {
        // Retail-shaped values from FactionBuilding.ini: ChinaBarracks has a
        // QueueProductionExitUpdate (300 ms = 9 logic frames); AmericaWarFactory
        // uses DefaultProductionExitUpdate and therefore has no artificial
        // delay/burst state.  Neither result is inferred from the Object name.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object RetailChinaProducer
  Type = Structure
  KindOf = STRUCTURE
  Behavior = QueueProductionExitUpdate ModuleTag_Exit
    UnitCreatePoint = X:0.0 Y:-25.0 Z:0.0
    NaturalRallyPoint = X:36.0 Y:-25.0 Z:0.0
    ExitDelay = 300
    AllowAirborneCreation = Yes
    InitialBurst = 2
  End
End
Object RetailAmericaProducer
  Type = Structure
  KindOf = STRUCTURE
  Behavior = DefaultProductionExitUpdate ModuleTag_Exit
    UnitCreatePoint = X:-10.0 Y:-30.0 Z:0.0
    NaturalRallyPoint = X:53.0 Y:-30.0 Z:0.0
    UseSpawnRallyPoint = Yes
  End
End
Object RetailSupplyCenterProducer
  Type = Structure
  KindOf = STRUCTURE SUPPLY_CENTER
  Behavior = SupplyCenterProductionExitUpdate ModuleTag_Exit
    UnitCreatePoint = X:0.0 Y:0.0 Z:0.0
    NaturalRallyPoint = X:24.0 Y:0.0 Z:0.0
  End
End
Object BarracksNamedWithoutExitBehavior
  Type = Structure
  KindOf = STRUCTURE
End
"#,
                "production_exit_metadata_probe.ini",
            )
            .expect("parse production exit probes");

        let china = GameLogic::build_template_from_object_definition(
            "RetailChinaProducer",
            parser
                .get_definition("RetailChinaProducer")
                .expect("Queue definition"),
            None,
        );
        let queue = china
            .production_exit_metadata
            .expect("authored QueueProductionExitUpdate");
        assert_eq!(queue.style, ProductionExitStyle::Queue);
        assert_eq!(queue.unit_create_point, [0.0, -25.0, 0.0]);
        assert_eq!(queue.natural_rally_point, [36.0, -25.0, 0.0]);
        assert_eq!(queue.exit_delay_frames, 9);
        assert!(queue.allow_airborne_creation);
        assert_eq!(queue.initial_burst, 2);

        let america = GameLogic::build_template_from_object_definition(
            "RetailAmericaProducer",
            parser
                .get_definition("RetailAmericaProducer")
                .expect("Default definition"),
            None,
        );
        let default_exit = america
            .production_exit_metadata
            .expect("authored DefaultProductionExitUpdate");
        assert_eq!(default_exit.style, ProductionExitStyle::Default);
        assert_eq!(default_exit.unit_create_point, [-10.0, -30.0, 0.0]);
        assert_eq!(default_exit.natural_rally_point, [53.0, -30.0, 0.0]);
        assert_eq!(default_exit.exit_delay_frames, 0);
        assert!(default_exit.use_spawn_rally_point);

        let supply_center = GameLogic::build_template_from_object_definition(
            "RetailSupplyCenterProducer",
            parser
                .get_definition("RetailSupplyCenterProducer")
                .expect("SupplyCenter definition"),
            None,
        );
        let supply_exit = supply_center
            .production_exit_metadata
            .expect("authored SupplyCenterProductionExitUpdate");
        assert_eq!(supply_exit.style, ProductionExitStyle::SupplyCenter);
        assert!(supply_exit.is_supply_center());
        assert_eq!(supply_exit.natural_rally_point, [24.0, 0.0, 0.0]);

        let absent = GameLogic::build_template_from_object_definition(
            "BarracksNamedWithoutExitBehavior",
            parser
                .get_definition("BarracksNamedWithoutExitBehavior")
                .expect("no-exit definition"),
            None,
        );
        assert!(
            absent.production_exit_metadata.is_none(),
            "a producer-shaped basename must not synthesize an exit interface"
        );

        // The runtime seeds retail definitions over a small starter-template
        // catalogue.  Enrichment must preserve this exact Queue interface
        // when the producer already exists; it is not only a newly-created
        // template concern.
        let mut seeded = GameLogic::new();
        seeded.templates.insert(
            "RetailChinaProducer".to_string(),
            crate::game_logic::ThingTemplate::new("RetailChinaProducer"),
        );
        assert_eq!(
            seeded.seed_asset_definition_templates_from_snapshot(vec![(
                "RetailChinaProducer".to_string(),
                parser
                    .get_definition("RetailChinaProducer")
                    .expect("Queue definition for existing template")
                    .clone(),
            )]),
            0,
        );
        assert_eq!(
            seeded
                .templates
                .get("RetailChinaProducer")
                .and_then(|template| template.production_exit_metadata),
            Some(queue),
            "existing starter producers must receive parsed exit metadata"
        );

        // Exercise the actual retail definitions too, including their trailing
        // INI comments and module tags.  These values are the live China Red
        // Guard Queue producer and USA factory Default producer, not a
        // hand-copied template-name fallback.
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let retail =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        let mut retail_parser = crate::assets::IniParser::new();
        retail_parser
            .parse_ini_content(&retail, "FactionBuilding.ini")
            .expect("parse retail production exits");
        let retail_china = GameLogic::build_template_from_object_definition(
            "ChinaBarracks",
            retail_parser
                .get_definition("ChinaBarracks")
                .expect("retail ChinaBarracks"),
            None,
        );
        let retail_queue = retail_china
            .production_exit_metadata
            .expect("retail China QueueProductionExitUpdate");
        assert_eq!(retail_queue.style, ProductionExitStyle::Queue);
        assert_eq!(retail_queue.exit_delay_frames, 9);
        assert_eq!(retail_queue.initial_burst, 0);
        assert_eq!(retail_queue.unit_create_point, [0.0, -25.0, 0.0]);
        assert_eq!(retail_queue.natural_rally_point, [36.0, -25.0, 0.0]);

        let retail_factory = GameLogic::build_template_from_object_definition(
            "AmericaWarFactory",
            retail_parser
                .get_definition("AmericaWarFactory")
                .expect("retail AmericaWarFactory"),
            None,
        );
        let retail_default = retail_factory
            .production_exit_metadata
            .expect("retail DefaultProductionExitUpdate");
        assert_eq!(retail_default.style, ProductionExitStyle::Default);
        assert_eq!(retail_default.unit_create_point, [-10.0, -30.0, 0.0]);
        assert_eq!(retail_default.natural_rally_point, [53.0, -30.0, 0.0]);
    }

    #[test]
    fn retail_refund_value_keeps_zero_fallback_and_exact_override() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object RetailFallbackRefund
  Type = Structure
  Model = RetailFallbackRefundModel
  KindOf = STRUCTURE SELECTABLE
  BuildCost = 1000
  RefundValue = 0
End
Object GLASupplyStash
  Type = Structure
  Model = UBSupply
  KindOf = STRUCTURE SELECTABLE
  BuildCost = 1500
  RefundValue = 650
End
"#,
                "retail_refund_value_probe.ini",
            )
            .expect("parse refund value probe");

        let fallback = GameLogic::build_template_from_object_definition(
            "RetailFallbackRefund",
            parser
                .get_definition("RetailFallbackRefund")
                .expect("fallback definition"),
            None,
        );
        assert_eq!(
            fallback.refund_value, 0,
            "zero retains SellPercentage fallback"
        );

        // Retail GLA Supply Stash uses the exact override rather than the
        // 1500 BuildCost × SellPercentage route.
        let stash = GameLogic::build_template_from_object_definition(
            "GLASupplyStash",
            parser
                .get_definition("GLASupplyStash")
                .expect("GLA Supply Stash definition"),
            None,
        );
        assert_eq!(stash.build_cost.supplies, 1_500);
        assert_eq!(stash.refund_value, 650);
    }

    #[test]
    fn parsed_dock_modules_define_dock_family_without_template_name_matching() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryWarehouseIdentity
  Type = Structure
  Model = ArbitraryWarehouseModel
  KindOf = STRUCTURE SELECTABLE SUPPLY_SOURCE
  Behavior = SupplyWarehouseDockUpdate ModuleTag_06
    StartingBoxes = 400
  End
End
Object ArbitraryFerryIdentity
  Type = Structure
  Model = ArbitraryFerryModel
  KindOf = SELECTABLE TRANSPORT
  Behavior = RailedTransportContain ModuleTag_03
    Slots = 10
  End
  Behavior = RailedTransportDockUpdate ModuleTag_06
    NumberApproachPositions = 9
  End
End
"#,
                "dock_metadata_probe.ini",
            )
            .expect("parse dock metadata probe");

        let warehouse = GameLogic::build_template_from_object_definition(
            "ArbitraryWarehouseIdentity",
            parser
                .get_definition("ArbitraryWarehouseIdentity")
                .expect("warehouse definition"),
            None,
        );
        assert_eq!(warehouse.dock_kind, DockKind::SupplyWarehouse);
        assert_eq!(warehouse.dock_starting_boxes, Some(400));

        let ferry = GameLogic::build_template_from_object_definition(
            "ArbitraryFerryIdentity",
            parser
                .get_definition("ArbitraryFerryIdentity")
                .expect("ferry definition"),
            None,
        );
        assert_eq!(ferry.dock_kind, DockKind::RailedTransport);
        assert_eq!(ferry.railed_transport_slots, Some(10));
        assert!(
            !ferry.is_kind_of(KindOf::Vehicle),
            "the test proves RailedTransportContain is not inferred from VEHICLE"
        );
    }

    #[test]
    fn authored_supply_source_kindof_drives_gather_bridge_not_template_name() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryRetailSupplyIdentity
  Type = Structure
  Model = SupplySourceModel
  KindOf = STRUCTURE SELECTABLE SUPPLY_SOURCE CANNOT_BUILD_NEAR_SUPPLIES
End
Object SupplyNamedButNotAuthored
  Type = Structure
  Model = SupplyLookingModel
  KindOf = STRUCTURE SELECTABLE
End
"#,
                "supply_source_kindof_probe.ini",
            )
            .expect("parse supply source metadata probe");

        let source = GameLogic::build_template_from_object_definition(
            "ArbitraryRetailSupplyIdentity",
            parser
                .get_definition("ArbitraryRetailSupplyIdentity")
                .expect("authored supply source definition"),
            None,
        );
        assert!(source.is_kind_of(KindOf::SupplySource));
        assert!(source.is_kind_of(KindOf::CannotBuildNearSupplies));
        assert!(source.is_kind_of(KindOf::Resource));
        assert!(source.is_kind_of(KindOf::Harvestable));

        let lookalike = GameLogic::build_template_from_object_definition(
            "SupplyNamedButNotAuthored",
            parser
                .get_definition("SupplyNamedButNotAuthored")
                .expect("supply-looking definition"),
            None,
        );
        assert!(!lookalike.is_kind_of(KindOf::SupplySource));
        assert!(!lookalike.is_kind_of(KindOf::CannotBuildNearSupplies));
        assert!(!lookalike.is_kind_of(KindOf::Resource));
        assert!(!lookalike.is_kind_of(KindOf::Harvestable));

        // Actual retail map supply objects declare the same capability.  The
        // parser must retain it rather than relying on their familiar names.
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("Main crate must remain three levels below repository root");
        let retail = std::fs::read_to_string(repo_root.join(
            "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/CivilianBuilding.ini",
        ))
        .expect("retail CivilianBuilding.ini");
        let mut retail_parser = crate::assets::IniParser::new();
        retail_parser
            .parse_ini_content(&retail, "CivilianBuilding.ini")
            .expect("parse retail CivilianBuilding.ini");
        for template_name in ["SupplyDock", "SupplyPile", "SupplyPileSmall"] {
            let template = GameLogic::build_template_from_object_definition(
                template_name,
                retail_parser
                    .get_definition(template_name)
                    .expect("retail supply source definition"),
                None,
            );
            assert!(
                template.is_kind_of(KindOf::SupplySource),
                "{template_name} must retain retail KINDOF_SUPPLY_SOURCE"
            );
        }
        let faction_buildings =
            std::fs::read_to_string(repo_root.join(
                "windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/FactionBuilding.ini",
            ))
            .expect("retail FactionBuilding.ini");
        retail_parser
            .parse_ini_content(&faction_buildings, "FactionBuilding.ini")
            .expect("parse retail FactionBuilding.ini");
        for template_name in ["AmericaSupplyCenter", "GLASupplyStash", "ChinaSupplyCenter"] {
            let template = GameLogic::build_template_from_object_definition(
                template_name,
                retail_parser
                    .get_definition(template_name)
                    .expect("retail supply-center definition"),
                None,
            );
            assert!(
                template.is_kind_of(KindOf::CannotBuildNearSupplies),
                "{template_name} must retain retail KINDOF_CANNOT_BUILD_NEAR_SUPPLIES"
            );
        }

        // Normal offline boot already has a hand-authored starter template
        // before the retail Object catalogue enriches it.  Enrichment must
        // carry this exact build rule too; otherwise headless games would
        // regress to a name-based exception even though parsed data is live.
        let mut seeded = GameLogic::new();
        seeded.templates.insert(
            "AmericaSupplyCenter".to_string(),
            ThingTemplate::new("AmericaSupplyCenter"),
        );
        assert_eq!(
            seeded.seed_asset_definition_templates_from_snapshot(vec![(
                "AmericaSupplyCenter".to_string(),
                retail_parser
                    .get_definition("AmericaSupplyCenter")
                    .expect("retail America supply center definition")
                    .clone(),
            )]),
            0,
            "the existing starter template is enriched rather than replaced"
        );
        assert!(seeded
            .templates
            .get("AmericaSupplyCenter")
            .is_some_and(|template| template.is_kind_of(KindOf::CannotBuildNearSupplies)));
    }

    #[test]
    fn parsed_capture_modules_keep_power_target_and_garrison_semantics() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryCaptureSourceIdentity
  Type = Infantry
  Model = ArbitraryCaptureSourceModel
  KindOf = INFANTRY SELECTABLE
  Behavior = SpecialAbility ModuleTag_Capture
    SpecialPowerTemplate = SpecialAbilityRebelCaptureBuilding
    StartsPaused = Yes
  End
  Behavior = SpecialAbilityUpdate ModuleTag_CaptureUpdate
    SpecialPowerTemplate = SpecialAbilityRebelCaptureBuilding
    StartAbilityRange = 5.0
    UnpackTime = 3000
    PreparationTime = 20000
    PackTime = 2000
  End
  Behavior = UnpauseSpecialPowerUpgrade ModuleTag_CaptureUpgrade
    SpecialPowerTemplate = SpecialAbilityRebelCaptureBuilding
    TriggeredBy = Upgrade_InfantryCaptureBuilding
  End
End
Object ArbitraryCaptureTargetIdentity
  Type = Structure
  Model = ArbitraryCaptureTargetModel
  KindOf = STRUCTURE SELECTABLE CAPTURABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
  End
End
Object ArbitraryImmuneTargetIdentity
  Type = Structure
  Model = ArbitraryImmuneTargetModel
  KindOf = STRUCTURE SELECTABLE IMMUNE_TO_CAPTURE
End
"#,
                "capture_metadata_probe.ini",
            )
            .expect("parse capture metadata probe");

        let source = GameLogic::build_template_from_object_definition(
            "ArbitraryCaptureSourceIdentity",
            parser
                .get_definition("ArbitraryCaptureSourceIdentity")
                .expect("capture source definition"),
            None,
        );
        assert_eq!(source.capture_power, CapturePowerKind::Rebel);
        assert!(source.capture_starts_paused);
        assert_eq!(
            source.capture_upgrade_trigger.as_deref(),
            Some("Upgrade_InfantryCaptureBuilding")
        );
        assert_eq!(source.capture_start_ability_range, Some(5.0));
        assert_eq!(source.capture_unpack_time_ms, Some(3_000));
        assert_eq!(source.capture_preparation_time_ms, Some(20_000));
        assert_eq!(source.capture_pack_time_ms, Some(2_000));

        let target = GameLogic::build_template_from_object_definition(
            "ArbitraryCaptureTargetIdentity",
            parser
                .get_definition("ArbitraryCaptureTargetIdentity")
                .expect("capture target definition"),
            None,
        );
        assert!(target.capturable);
        assert!(!target.immune_to_capture);
        assert_eq!(target.garrison_contain_max, Some(5));

        let immune = GameLogic::build_template_from_object_definition(
            "ArbitraryImmuneTargetIdentity",
            parser
                .get_definition("ArbitraryImmuneTargetIdentity")
                .expect("immune target definition"),
            None,
        );
        assert!(!immune.capturable);
        assert!(immune.immune_to_capture);
    }

    #[test]
    fn parsed_charge_plant_keeps_unpack_variation_and_flee() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ArbitraryBurtonChargeIdentity
  Type = Infantry
  Model = ArbitraryBurtonChargeModel
  KindOf = INFANTRY SELECTABLE
  Behavior = SpecialAbilityUpdate ModuleTag_Timed
    SpecialPowerTemplate = SpecialAbilityColonelBurtonTimedCharges
    UnpackTime = 5500
    PackTime = 0
    PackUnpackVariationFactor = 0.2
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
  Behavior = SpecialAbilityUpdate ModuleTag_Remote
    SpecialPowerTemplate = SpecialAbilityColonelBurtonRemoteCharges
    UnpackTime = 5500
    PackUnpackVariationFactor = 0.2
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
End
"#,
                "charge_plant_metadata_probe.ini",
            )
            .expect("parse charge plant metadata probe");

        let source = GameLogic::build_template_from_object_definition(
            "ArbitraryBurtonChargeIdentity",
            parser
                .get_definition("ArbitraryBurtonChargeIdentity")
                .expect("charge source definition"),
            None,
        );
        let timed = source
            .charge_plant_ability_for_timed()
            .expect("timed C4 update");
        assert_eq!(timed.unpack_time_ms, 5_500);
        assert_eq!(timed.pack_unpack_variation_factor, 0.2);
        assert_eq!(timed.flee_range_after_completion, 100.0);
        assert!(timed.flip_object_after_unpacking);
        let remote = source
            .charge_plant_ability_for_remote()
            .expect("remote C4 update");
        assert_eq!(remote.unpack_time_ms, 5_500);
        assert_eq!(remote.pack_unpack_variation_factor, 0.2);
        assert_eq!(remote.flee_range_after_completion, 100.0);
    }

    #[test]
    fn burton_charge_unpacks_then_plants_then_flees() {
        // SpecialAbilityUpdate.cpp:397-441 unpack, trigger, finishAbility flee.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object BurtonChargeUnpackProbe
  Type = Infantry
  KindOf = INFANTRY SELECTABLE CAN_ATTACK
  Behavior = SpecialAbilityUpdate ModuleTag_Timed
    SpecialPowerTemplate = SpecialAbilityColonelBurtonTimedCharges
    UnpackTime = 200
    PackUnpackVariationFactor = 0
    FleeRangeAfterCompletion = 100
    FlipOwnerAfterUnpacking = Yes
  End
End
Object ChargePlantTargetProbe
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
End
"#,
                "burton_charge_live_probe.ini",
            )
            .expect("parse live charge probe");

        let mut logic = GameLogic::new();
        let burton_tpl = GameLogic::build_template_from_object_definition(
            "BurtonChargeUnpackProbe",
            parser
                .get_definition("BurtonChargeUnpackProbe")
                .expect("burton probe"),
            None,
        );
        assert_eq!(
            burton_tpl
                .charge_plant_ability_for_timed()
                .map(|m| m.unpack_time_ms),
            Some(200)
        );
        logic
            .templates
            .insert("BurtonChargeUnpackProbe".into(), burton_tpl);
        let mut target_tpl = GameLogic::build_template_from_object_definition(
            "ChargePlantTargetProbe",
            parser
                .get_definition("ChargePlantTargetProbe")
                .expect("target probe"),
            None,
        );
        target_tpl.set_health(500.0);
        logic
            .templates
            .insert("ChargePlantTargetProbe".into(), target_tpl);

        let burton_id = logic
            .create_object(
                "BurtonChargeUnpackProbe",
                Team::USA,
                Vec3::new(2.0, 0.0, 0.0),
            )
            .expect("burton");
        let target_id = logic
            .create_object(
                "ChargePlantTargetProbe",
                Team::GLA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("target");
        {
            let burton = logic.host_object_mut(burton_id).expect("burton mut");
            burton.set_ai_state(AIState::SpecialAbility);
            burton.set_target(Some(target_id));
        }
        logic.queue_pending_special_ability(
            burton_id,
            PendingSpecialAbility::PlantTimedDemoCharge { target_id },
        );

        logic.update_ai(&[burton_id, target_id], 1.0 / 60.0);
        assert_eq!(
            logic.mine_residual_places(),
            0,
            "UnpackTime 200ms must delay plant"
        );

        for _ in 0..20 {
            logic.update_ai(&[burton_id, target_id], 1.0 / 60.0);
        }
        assert!(
            logic.mine_residual_places() >= 1,
            "charge plants after UnpackTime"
        );
        let pos = logic
            .host_object(burton_id)
            .expect("burton after plant")
            .get_position();
        assert!(
            pos.distance(Vec3::new(2.0, 0.0, 0.0)) > 1.0,
            "finishAbility flee after plant, pos={pos:?}"
        );
    }



    #[test]
    fn temporary_weapon_behavior_metadata_is_source_ordered_and_not_live_state() {
        use crate::game_logic::host_temporary_weapon_behavior::{
            FireWeaponWhenDamagedWeaponRole, TemporaryWeaponSlot,
        };

        let source = r#"
Object TemporaryWeaponContractProbe
  Behavior = FireWeaponWhenDamagedBehavior ModuleTag_Damage
    StartsActive = Yes
    DamageTypes = NONE +FLAME +POISON -FLAME
    DamageAmount = 2.5
    ReactionWeaponDamaged = SharedTempWeapon
    ContinuousWeaponDamaged = SharedTempWeapon
    TriggeredBy = Upgrade_A Upgrade_B
    ConflictsWith = Upgrade_C
    RemovesUpgrades = Upgrade_D
    FXListUpgrade = UpgradeFX
    RequiresAllTriggers = Yes
  End
  Behavior = FireWeaponWhenDeadBehavior ModuleTag_Death
    StartsActive = No
    DeathWeapon = DeathTempWeapon
    DeathTypes = NONE +EXPLODED
    VeterancyLevels = NONE +VETERAN
    ExemptStatus = +UNDER_CONSTRUCTION
    RequiredStatus = DEPLOYED
  End
End
"#;
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(source, "temporary_weapon_contract_probe.ini")
            .expect("parse source behavior modules");
        let template = GameLogic::build_template_from_object_definition(
            "TemporaryWeaponContractProbe",
            parser
                .get_definition("TemporaryWeaponContractProbe")
                .expect("source definition"),
            None,
        );

        assert_eq!(template.fire_weapon_when_damaged_behaviors.len(), 1);
        assert_eq!(template.fire_weapon_when_dead_behaviors.len(), 1);
        let damaged = &template.fire_weapon_when_damaged_behaviors[0];
        assert_eq!(damaged.module_source_index, 0);
        assert_eq!(damaged.module_tag.as_deref(), Some("ModuleTag_Damage"));
        assert!(damaged.starts_active);
        assert_eq!(damaged.damage_types.0, 1u64 << 9, "POISON only");
        assert_eq!(damaged.damage_amount, 2.5);
        assert_eq!(damaged.upgrade_mux.triggered_by, ["Upgrade_A", "Upgrade_B"]);
        let specs = damaged.runtime_specs();
        assert_eq!(specs.len(), 2);
        assert_ne!(specs[0].key, specs[1].key);
        assert_eq!(
            specs[0].key.role,
            FireWeaponWhenDamagedWeaponRole::ReactionDamaged
        );
        assert_eq!(
            specs[1].key.role,
            FireWeaponWhenDamagedWeaponRole::ContinuousDamaged
        );
        assert!(specs
            .iter()
            .all(|spec| spec.weapon_slot == TemporaryWeaponSlot::Primary));

        let dead = &template.fire_weapon_when_dead_behaviors[0];
        assert_eq!(dead.module_source_index, 1);
        assert_eq!(dead.module_tag.as_deref(), Some("ModuleTag_Death"));
        assert!(!dead.starts_active);
        assert_eq!(dead.death_weapon.as_deref(), Some("DeathTempWeapon"));
        assert_eq!(dead.death_types.0, 1u32 << 4);
        assert_eq!(dead.veterancy_levels.0, 1u8 << 1);
        assert!(dead.die_mux_allows(4, 1, 1u64 << 44));
        assert_eq!(
            dead.ephemeral_weapon_spec()
                .expect("C++ creates a fresh death weapon")
                .weapon_slot,
            TemporaryWeaponSlot::Primary
        );

        // The template retains source contracts only.  There is deliberately
        // no object-owned damaged-behavior runtime here until a versioned
        // ObjectSnapshot tail can preserve all eight C++ Weapon Xfers.
        assert!(template
            .fire_weapon_when_damaged_behaviors
            .iter()
            .all(|metadata| !metadata.runtime_specs().is_empty()));
    }

    #[test]
    fn exact_retail_catalogue_seed_preserves_data_and_never_overwrites_curated_templates() {
        let mut logic = GameLogic::new();
        let mut curated = ThingTemplate::new("AmericaTankCrusader");
        curated.set_health(777.0).set_model("CuratedExactModel");
        logic
            .templates
            .insert("AmericaTankCrusader".to_string(), curated);

        let mut retail_unit = ObjectDefinition::new("AmericaTankCrusader".to_string());
        retail_unit.object_type = "Vehicle".to_string();
        retail_unit.hit_points = Some(480.0);
        retail_unit.model_name = Some("AVCrusader".to_string());
        retail_unit.scale = 0.9;
        retail_unit.scale_was_specified = true;
        retail_unit.attributes.insert(
            "KindOf".to_string(),
            "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
        );
        retail_unit
            .attributes
            .insert("BuildCost".to_string(), "900".to_string());

        let mut retail_new = ObjectDefinition::new("AmericaTankPaladin".to_string());
        retail_new.object_type = "Vehicle".to_string();
        retail_new.hit_points = Some(600.0);
        retail_new.model_name = Some("AVPaladin".to_string());
        retail_new.scale = 0.66;
        retail_new.scale_was_specified = true;
        retail_new.primary_weapon = Some("PaladinTankGun".to_string());
        retail_new.secondary_weapon = Some("PaladinPointDefenseLaser".to_string());
        retail_new.attributes.insert(
            "KindOf".to_string(),
            "VEHICLE SELECTABLE CAN_ATTACK".to_string(),
        );
        retail_new
            .attributes
            .insert("BuildCost".to_string(), "1100".to_string());

        let mut sound_anchor = ObjectDefinition::new("AmbientOnlyRetailAnchor".to_string());
        sound_anchor
            .attributes
            .insert("SoundAmbient".to_string(), "AmbientWind".to_string());

        let inserted = logic.seed_asset_definition_templates_from_snapshot(vec![
            ("AmericaTankPaladin".to_string(), retail_new),
            ("AmericaTankCrusader".to_string(), retail_unit),
            ("AmbientOnlyRetailAnchor".to_string(), sound_anchor),
        ]);

        assert_eq!(inserted, 2);
        let curated_after = logic
            .templates
            .get("AmericaTankCrusader")
            .expect("curated template retained");
        assert_eq!(curated_after.max_health, 777.0);
        assert_eq!(
            curated_after.model_name.as_deref(),
            Some("CuratedExactModel")
        );
        assert!((curated_after.asset_scale - 0.9).abs() < f32::EPSILON);

        let added = logic
            .templates
            .get("AmericaTankPaladin")
            .expect("retail definition seeded");
        assert_eq!(added.max_health, 600.0);
        assert_eq!(added.build_cost.supplies, 1100);
        assert_eq!(added.model_name.as_deref(), Some("AVPaladin"));
        assert!((added.asset_scale - 0.66).abs() < f32::EPSILON);
        assert_eq!(added.primary_weapon_name.as_deref(), Some("PaladinTankGun"));
        assert_eq!(
            added.secondary_weapon_name.as_deref(),
            Some("PaladinPointDefenseLaser")
        );
        assert!(added.is_kind_of(KindOf::Vehicle));
        let ambient = logic
            .templates
            .get("AmbientOnlyRetailAnchor")
            .expect("SoundAmbient-only map object is now a live template");
        assert_eq!(ambient.sound_ambient.as_deref(), Some("AmbientWind"));
        assert!(ambient.model_name.is_none());
    }

    #[test]
    fn pristine_models_do_not_match_condition_or_faction_variants() {
        let pairs = [
            ("ABPWRPLANT", "ABPWRPLANT_d06"),
            ("ABBarracks", "ABBarracks_FA"),
            ("ABPatriot", "ABPatriotSW"),
            ("NBConYard", "NBConYard_FA"),
            ("UBCmdHQ", "UBArFrcCmd"),
            ("PTDogwod01", "PTDogwod01_S"),
        ];

        for (pristine, wrong_variant) in pairs {
            assert_eq!(
                GameLogic::find_exact_available_model_name(
                    pristine,
                    vec![format!("{wrong_variant}.W3D")].into_iter(),
                ),
                None,
                "{pristine} must not select distinct ConditionState/faction asset {wrong_variant}"
            );
            assert_eq!(
                GameLogic::find_exact_available_model_name(
                    pristine,
                    vec![format!("Art/W3D/{pristine}.W3D")].into_iter(),
                ),
                Some(pristine.to_string()),
                "{pristine} must retain exact retail basename lookup"
            );
        }
    }

    #[test]
    fn hand_authored_unit_templates_keep_their_verified_retail_w3d_identity() {
        let mut logic = GameLogic::new();
        logic.setup_templates();

        // Each value is the DefaultConditionState `Model` for the named
        // retail Object INI identity and an exact W3DZH.big basename.  Keep
        // this table explicit: a visual fallback must be unavailable rather
        // than silently turning one game unit into another.
        let expected_models = [
            ("USA_Ranger", "airngr_skn"),
            ("USA_MissileDefender", "nithnt_skn"),
            ("USA_CrusaderTank", "avleopard"),
            ("USA_PaladinTank", "avpaladin"),
            ("USA_Raptor", "avraptor"),
            ("GLA_Soldier", "uirgrd_skn"),
            ("GLA_RPGTrooper", "uitunf_skn"),
            ("GLA_Technical", "uvtechtrck"),
            ("GLA_ScorpionTank", "uvlitetank"),
            ("GLA_MarauderTank", "uvmarauder"),
            ("GLAInfantryTerrorist", "uitrst_skn"),
            ("GLAVehicleCombatBike", "uvcombike"),
            ("China_RedGuard", "nicnsc_skn"),
            ("China_TankHunter", "nimsst_skn"),
            ("China_BattlemasterTank", "nvbtmstr"),
            ("China_OverlordTank", "nvovrlrd"),
            ("China_InfernoCannon", "nvinferno"),
            ("China_MiG", "nvmig"),
            ("China_Helix", "nvhelix"),
        ];

        for (template_name, expected_model) in expected_models {
            let template = logic
                .templates
                .get(template_name)
                .unwrap_or_else(|| panic!("missing curated template {template_name}"));
            assert_eq!(
                template.model_name.as_deref(),
                Some(expected_model),
                "{template_name} must keep its own retail visual identity"
            );
        }

        // C++ WeaponSet classifies airborne targets through their gameplay
        // KindOf, not their Drawable model.  These curated starters must
        // therefore retain the authored VEHICLE bit as well as AIRCRAFT.
        for template_name in ["USA_Raptor", "China_MiG", "China_Helix"] {
            let template = logic
                .templates
                .get(template_name)
                .unwrap_or_else(|| panic!("missing curated aircraft {template_name}"));
            assert!(template.is_kind_of(KindOf::Vehicle));
            assert!(template.is_kind_of(KindOf::Aircraft));
        }
    }

    #[test]
    fn create_object_uses_ini_create_policy_not_unit_name_hardcodes() {
        // C++ Object.cpp:160-497 / GameLogic::friend_createObject: Object ctor
        // builds from ThingTemplate INI and each module's onObjectCreated.
        // Strategy Center AutoChooseSources=PRIMARY NONE is FactionBuilding.ini:6970;
        // Quad Cannon AntiGround/AntiAirborne is Weapon.ini:2637-2660;
        // Toxin Tractor FireOCLAfterWeaponCooldownUpdate is GLAVehicle.ini:3697.
        // create_object must honor those template/module bits. Name residuals
        // remain only when Object INI was never loaded (unit tests).
        use glam::Vec3;

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object AmericaStrategyCenter
  Type = Building
  KindOf = PRELOAD STRUCTURE SELECTABLE IMMOBILE FS_STRATEGY_CENTER
  WeaponSet
    Conditions = None
    Weapon = PRIMARY StrategyCenterGun
    AutoChooseSources = PRIMARY NONE
  End
End
Object GLAVehicleQuadCannon
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  WeaponSet
    Conditions = None
    Weapon = PRIMARY QuadCannonGun
    Weapon = SECONDARY QuadCannonGunAir
  End
End
Object GLAVehicleToxinTruck
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  WeaponSet
    Conditions = None
    Weapon = PRIMARY ToxinTruckGun
    Weapon = SECONDARY ToxinTruckSprayer
    AutoChooseSources = SECONDARY NONE
  End
  Behavior = FireOCLAfterWeaponCooldownUpdate ModuleTag_13
    WeaponSlot = SECONDARY
    OCL = OCL_PoisonFieldMedium
    MinShotsToCreateOCL = 4
  End
End
Object ChemSpillSprayer
  Type = Vehicle
  KindOf = SELECTABLE CAN_ATTACK VEHICLE
  Behavior = FireOCLAfterWeaponCooldownUpdate ModuleTag_01
    WeaponSlot = SECONDARY
    OCL = OCL_PoisonFieldMedium
  End
End
Object OrdinaryAttackableBunker
  Type = Building
  KindOf = STRUCTURE SELECTABLE CAN_ATTACK
End
"#,
                "create_policy_probe.ini",
            )
            .expect("parse create-policy probes");

        let parsed = |name: &str| {
            GameLogic::build_template_from_object_definition(
                name,
                parser
                    .get_definition(name)
                    .unwrap_or_else(|| panic!("missing probe Object {name}")),
                None,
            )
        };

        let strategy = parsed("AmericaStrategyCenter");
        assert!(
            strategy.primary_auto_choose_none,
            "authored AutoChooseSources=PRIMARY NONE must land on the template"
        );
        assert!(!strategy.has_fire_ocl_after_weapon_cooldown);
        assert_eq!(
            strategy.primary_weapon_name.as_deref(),
            Some("StrategyCenterGun")
        );

        let toxin = parsed("GLAVehicleToxinTruck");
        assert!(
            toxin.has_fire_ocl_after_weapon_cooldown,
            "FireOCLAfterWeaponCooldownUpdate must install from the Behavior, not the unit name"
        );
        assert!(!toxin.primary_auto_choose_none);

        let spill = parsed("ChemSpillSprayer");
        assert!(
            spill.has_fire_ocl_after_weapon_cooldown,
            "a non-toxin-tractor name with the module must still carry FireOCL create policy"
        );

        let bunker = parsed("OrdinaryAttackableBunker");
        assert!(
            !bunker.primary_auto_choose_none,
            "CAN_ATTACK alone must not invent AutoChooseSources=PRIMARY NONE"
        );

        crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();

        let mut logic = GameLogic::new();
        for name in [
            "AmericaStrategyCenter",
            "GLAVehicleQuadCannon",
            "GLAVehicleToxinTruck",
            "ChemSpillSprayer",
            "OrdinaryAttackableBunker",
        ] {
            logic.templates.insert(name.to_string(), parsed(name));
        }

        let center_id = logic
            .create_object(
                "AmericaStrategyCenter",
                Team::USA,
                Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("strategy center");
        {
            let center = logic.host_object(center_id).expect("center");
            if let Some(weapon) = center.weapon.as_ref() {
                assert!(
                    (weapon.damage - 25.0).abs() > 0.01 || (weapon.range - 100.0).abs() > 0.01,
                    "AutoChooseSources=PRIMARY NONE must not invent Weapon::default"
                );
            }
        }

        let bunker_id = logic
            .create_object(
                "OrdinaryAttackableBunker",
                Team::USA,
                Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("bunker");
        {
            let bunker = logic.host_object(bunker_id).expect("bunker");
            let weapon = bunker
                .weapon
                .as_ref()
                .expect("kind-based default remains when Object INI has no AutoChoose NONE");
            assert!((weapon.damage - 25.0).abs() < 0.01);
        }

        let spill_id = logic
            .create_object("ChemSpillSprayer", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .expect("spill sprayer");
        {
            let spill = logic.host_object(spill_id).expect("spill");
            assert!(
                spill.fire_ocl_after_cooldown.is_some(),
                "FireOCL module data must install without a toxin-tractor template name"
            );
            assert!(
                spill.secondary_weapon.is_none(),
                "non-toxin FireOCL must not invent ToxinTruckSprayer"
            );
        }

        let toxin_id = logic
            .create_object(
                "GLAVehicleToxinTruck",
                Team::GLA,
                Vec3::new(30.0, 0.0, 0.0),
            )
            .expect("toxin");
        assert!(logic
            .host_object(toxin_id)
            .expect("toxin")
            .fire_ocl_after_cooldown
            .is_some());

        let quad_id = logic
            .create_object(
                "GLAVehicleQuadCannon",
                Team::GLA,
                Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("quad");
        {
            let quad = logic.host_object(quad_id).expect("quad");
            if let Some(primary) = quad.weapon.as_ref() {
                assert!(primary.can_target_ground);
                assert!(!primary.can_target_air);
            }
            if let Some(secondary) = quad.secondary_weapon.as_ref() {
                assert!(secondary.can_target_air);
                assert!(!secondary.can_target_ground);
            }
        }

        // Missing-INI fallback: name residual still binds when Object INI
        // never set has_fire_ocl_after_weapon_cooldown.
        let mut fallback = ThingTemplate::new("TestToxinTruck");
        fallback
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .set_health(240.0);
        assert!(!fallback.has_fire_ocl_after_weapon_cooldown);
        logic
            .templates
            .insert("TestToxinTruck".to_string(), fallback);
        let fallback_id = logic
            .create_object("TestToxinTruck", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .expect("missing-INI toxin");
        {
            let fallback = logic.host_object(fallback_id).expect("fallback toxin");
            assert!(
                fallback.fire_ocl_after_cooldown.is_some(),
                "missing-INI toxin name still installs FireOCL residual"
            );
            assert!(
                fallback.secondary_weapon.is_some(),
                "missing-INI toxin name still binds spray residual"
            );
        }
    }

    #[test]
    fn create_object_uses_already_loaded_host_templates_not_thing_factory() {
        // C++ ThingFactory.cpp findTemplate is an in-memory hash after
        // GameEngine::init loaded Object INI once. Rust
        // TheThingFactory::find_template lazy-inits every Object INI (14s+
        // on Lone Eagle). Host create_object must bind already-loaded
        // self.templates only.
        let create_src = include_str!("create_destroy_die.rs");
        assert!(
            create_src.contains("fn ensure_host_spawn_template"),
            "create_object must resolve host catalog via ensure_host_spawn_template"
        );
        assert!(
            !create_src.contains("TheThingFactory::find_template("),
            "create_object must not call TheThingFactory::find_template"
        );
        assert!(
            !create_src.contains("init_thing_factory("),
            "create_object must not lazy-init ThingFactory"
        );

        let mut logic = GameLogic::new();
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let exact = logic
            .create_object(
                "USA_Ranger",
                Team::USA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("exact host template");
        assert_eq!(
            logic.host_object(exact).expect("exact").template_name,
            "USA_Ranger"
        );

        let aliased = logic
            .create_object(
                "usa_ranger",
                Team::USA,
                glam::Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("case-insensitive already-loaded host template");
        assert_eq!(
            logic.host_object(aliased).expect("alias").template_name,
            "USA_Ranger"
        );

        let cached_miss = "CachedMissPropThatMustNotResynthesize";
        logic
            .unresolved_spawn_templates
            .insert(cached_miss.to_string());
        let before = logic.templates.len();
        assert!(
            logic
                .create_object(cached_miss, Team::Neutral, glam::Vec3::ZERO)
                .is_none()
        );
        assert_eq!(
            logic.templates.len(),
            before,
            "cached unresolved names must not re-enter template synthesis"
        );

        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            crate::game_logic::weapon_bootstrap::ensure_host_weapon_store(),
            0,
            "weapon store must not reload after first host seed"
        );
        let _ = crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store();
        assert_eq!(
            crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store(),
            0,
            "locomotor store must not reload after first host seed"
        );
    }

    #[test]
    fn heal_cave_tunnel_contain_kinds_map_and_heal_contain_auto_exits() {
        use glam::Vec3;

        // C++ HealContain.cpp:68-157 heals then auto-exits.
        // C++ TunnelTracker.cpp:225-268 healObjects sliver / snap-to-max.
        // Pre-fix: spawn_templates mapped Heal/Cave/Tunnel to ContainModuleKind::None.
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeHealContainBarracks
  Type = Structure
  KindOf = STRUCTURE SELECTABLE HEAL_PAD
  Body = ActiveBody ModuleTag_01
    MaxHealth = 1000.0
  End
  Behavior = HealContain ModuleTag_Heal
    ContainMax = 10
    TimeForFullHeal = 2000
    AllowInsideKindOf = INFANTRY
    AllowAlliesInside = Yes
    AllowEnemiesInside = No
    AllowNeutralInside = No
  End
End
Object ProbeCaveContain
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = CaveContain ModuleTag_Cave
    ContainMax = 10
    CaveIndex = 4
  End
End
Object ProbeTunnelContain
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = TunnelContain ModuleTag_Tunnel
    TimeForFullHeal = 5000
  End
End
Object ProbeHealInfantry
  Type = Infantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 80.0
  End
End

"#,
                "heal_cave_tunnel_contain_probe.ini",
            )
            .expect("parse contain kind probe");

        let barracks = GameLogic::build_template_from_object_definition(
            "ProbeHealContainBarracks",
            parser
                .get_definition("ProbeHealContainBarracks")
                .expect("heal barracks"),
            None,
        );
        assert_eq!(barracks.contain_module.kind, ContainModuleKind::Heal);
        assert_eq!(barracks.contain_module.slots, Some(10));
        assert_eq!(barracks.contain_module.frames_for_full_heal, Some(60));
        assert_eq!(
            barracks.contain_module.admission,
            ContainAdmission::InfantryOnly
        );
        assert!(barracks.contain_module.allow_allies_inside);
        assert!(!barracks.contain_module.allow_enemies_inside);
        assert!(!barracks.contain_module.allow_neutral_inside);
        assert!(barracks.garrison_contain_max.is_none());

        let cave = GameLogic::build_template_from_object_definition(
            "ProbeCaveContain",
            parser.get_definition("ProbeCaveContain").expect("cave"),
            None,
        );
        assert_eq!(cave.contain_module.kind, ContainModuleKind::Cave);
        assert_eq!(cave.contain_module.slots, Some(10));
        assert_eq!(cave.contain_module.cave_index, 4);

        let tunnel = GameLogic::build_template_from_object_definition(
            "ProbeTunnelContain",
            parser.get_definition("ProbeTunnelContain").expect("tunnel"),
            None,
        );
        assert_eq!(tunnel.contain_module.kind, ContainModuleKind::Tunnel);
        assert_eq!(tunnel.contain_module.frames_for_full_heal, Some(150));

        let infantry = GameLogic::build_template_from_object_definition(
            "ProbeHealInfantry",
            parser
                .get_definition("ProbeHealInfantry")
                .expect("infantry"),
            None,
        );

        let mut logic = GameLogic::new();
        let usa = Player::new(1, Team::USA, "USA", true);
        logic.add_player(usa);
        logic
            .templates
            .insert("ProbeHealContainBarracks".to_string(), barracks);
        logic
            .templates
            .insert("ProbeHealInfantry".to_string(), infantry);
        logic
            .templates
            .insert("ProbeTunnelContain".to_string(), tunnel);

        let pad = logic
            .create_object_for_player("ProbeHealContainBarracks", 1, Vec3::ZERO)
            .expect("heal barracks");
        let unit = logic
            .create_object_for_player("ProbeHealInfantry", 1, Vec3::ZERO)
            .expect("infantry");
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 20.0;
            obj.health.maximum = 80.0;
        }

        assert!(
            logic.can_unit_enter_normal_target(unit, pad),
            "damaged infantry must enter HealContain"
        );
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 80.0;
        }
        assert!(
            !logic.can_unit_enter_normal_target(unit, pad),
            "C++ ActionManager.cpp:636-644 rejects full-health HealContain enter"
        );
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 20.0;
            obj.set_ai_state(AIState::Entering);
            obj.target = Some(pad);
        }

        logic.frame = 10;
        logic.update_support_states(&[unit], 1.0 / 30.0);
        assert_eq!(
            logic.host_object(unit).and_then(|o| o.contained_by),
            Some(pad)
        );
        let after_enter = logic
            .host_object(unit)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        // First contained update applies max/60 sliver (HealContain.cpp:148).
        assert!(
            (after_enter - (20.0 + 80.0 / 60.0)).abs() < 0.05
                || after_enter > 20.0,
            "HealContain must sliver-heal, got {after_enter}"
        );

        for _ in 0..60 {
            logic.frame = logic.frame.saturating_add(1);
            logic.update_support_states(&[unit], 1.0 / 30.0);
        }
        let after = logic.host_object(unit).expect("unit after heal");
        assert!(
            after.health.current >= after.health.maximum - 0.01,
            "HealContain must finish at max health"
        );
        assert!(
            after.contained_by.is_none(),
            "HealContain.cpp:97-101 auto-exits when done healing"
        );
        assert!(logic.tunnel_network.honesty_heal_contain_auto_exit_ok());

        let tunnel_id = logic
            .create_object_for_player("ProbeTunnelContain", 1, Vec3::new(40.0, 0.0, 0.0))
            .expect("tunnel");
        let rider = logic
            .create_object_for_player("ProbeHealInfantry", 1, Vec3::new(40.0, 0.0, 0.0))
            .expect("tunnel rider");
        if let Some(obj) = logic.host_object_mut(rider) {
            obj.health.current = 30.0;
            obj.health.maximum = 80.0;
        }
        assert!(logic.tunnel_network.record_enter(
            crate::game_logic::host_tunnel_network::tunnel_system_key(Some(1), Team::USA),
            rider,
            tunnel_id,
        ));
        logic.tunnel_network.stamp_contained_by_frame(rider, logic.frame);
        let before_tunnel = logic
            .host_object(rider)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        logic.frame = logic.frame.saturating_add(1);
        logic.update_support_states(&[rider], 1.0 / 30.0);
        let after_tunnel = logic
            .host_object(rider)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        assert!(
            after_tunnel > before_tunnel,
            "TunnelTracker::healObjects must sliver-heal, {before_tunnel} -> {after_tunnel}"
        );
        assert!(logic.tunnel_network.honesty_heal_objects_ok());
    }

    #[test]
    fn garrison_initial_roster_parses_template_and_count() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeGarrisonRosterBunker
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    InitialRoster = GLAInfantryRebel 3
  End
End
"#,
                "garrison_initial_roster_probe.ini",
            )
            .expect("parse garrison roster probe");

        let bunker = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonRosterBunker",
            parser
                .get_definition("ProbeGarrisonRosterBunker")
                .expect("roster bunker"),
            None,
        );
        assert_eq!(bunker.contain_module.kind, ContainModuleKind::Garrison);
        assert_eq!(
            bunker.contain_module.initial_roster_template,
            "GLAInfantryRebel"
        );
        assert_eq!(bunker.contain_module.initial_roster_count, 3);
    }

    #[test]
    fn contain_module_parses_enter_and_exit_sounds() {
        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeGarrisonEnterSound
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    EnterSound = GarrisonEnter
    ExitSound = GarrisonExit
  End
End
Object ProbeTransportEnterSound
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
    EnterSound = GarrisonEnter
    ExitSound = GarrisonExit
  End
End
Object ProbeHumveeSilentContain
  Type = Vehicle
  KindOf = VEHICLE SELECTABLE
  Behavior = TransportContain ModuleTag_Transport
    Slots = 5
  End
End
"#,
                "contain_enter_exit_sound_probe.ini",
            )
            .expect("parse contain sound probe");

        let bunker = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonEnterSound",
            parser
                .get_definition("ProbeGarrisonEnterSound")
                .expect("garrison sound"),
            None,
        );
        assert_eq!(bunker.contain_module.enter_sound, "GarrisonEnter");
        assert_eq!(bunker.contain_module.exit_sound, "GarrisonExit");

        let transport = GameLogic::build_template_from_object_definition(
            "ProbeTransportEnterSound",
            parser
                .get_definition("ProbeTransportEnterSound")
                .expect("transport sound"),
            None,
        );
        assert_eq!(transport.contain_module.kind, ContainModuleKind::Transport);
        assert_eq!(transport.contain_module.enter_sound, "GarrisonEnter");
        assert_eq!(transport.contain_module.exit_sound, "GarrisonExit");

        let humvee = GameLogic::build_template_from_object_definition(
            "ProbeHumveeSilentContain",
            parser
                .get_definition("ProbeHumveeSilentContain")
                .expect("silent transport"),
            None,
        );
        assert_eq!(humvee.contain_module.kind, ContainModuleKind::Transport);
        assert!(
            humvee.contain_module.enter_sound.is_empty()
                && humvee.contain_module.exit_sound.is_empty(),
            "Humvee comments EnterSound out — C++ stays silent"
        );
    }


    #[test]
    fn garrison_heal_objects_parses_and_heals_occupants() {
        use glam::Vec3;

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(
                r#"
Object ProbeGarrisonHealBunker
  Type = Structure
  KindOf = STRUCTURE SELECTABLE
  Behavior = GarrisonContain ModuleTag_Garrison
    ContainMax = 5
    HealObjects = Yes
    TimeForFullHeal = 2000
  End
End
Object ProbeGarrisonHealInfantry
  Type = Infantry
  KindOf = INFANTRY SELECTABLE
  TransportSlotCount = 1
  Body = ActiveBody ModuleTag_01
    MaxHealth = 80.0
  End
End
"#,
                "garrison_heal_objects_probe.ini",
            )
            .expect("parse garrison heal probe");

        let bunker = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonHealBunker",
            parser
                .get_definition("ProbeGarrisonHealBunker")
                .expect("heal bunker"),
            None,
        );
        assert!(bunker.contain_module.heal_objects);
        assert_eq!(bunker.contain_module.frames_for_full_heal, Some(60));

        let infantry = GameLogic::build_template_from_object_definition(
            "ProbeGarrisonHealInfantry",
            parser
                .get_definition("ProbeGarrisonHealInfantry")
                .expect("heal infantry"),
            None,
        );

        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("ProbeGarrisonHealBunker".to_string(), bunker);
        logic
            .templates
            .insert("ProbeGarrisonHealInfantry".to_string(), infantry);
        let pad = logic
            .create_object("ProbeGarrisonHealBunker", Team::USA, Vec3::ZERO)
            .expect("heal bunker");
        let unit = logic
            .create_object("ProbeGarrisonHealInfantry", Team::USA, Vec3::ZERO)
            .expect("infantry");
        assert!(logic.host_object_mut(pad).expect("bunker").add_occupant(unit));
        if let Some(obj) = logic.host_object_mut(unit) {
            obj.health.current = 20.0;
            obj.health.maximum = 80.0;
            obj.set_contained_by(Some(pad));
        }
        logic.tunnel_network.stamp_contained_by_frame(unit, logic.frame);
        logic.frame = logic.frame.saturating_add(1);
        logic.update_support_states(&[unit], 1.0 / 30.0);
        let after = logic
            .host_object(unit)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        assert!(
            after > 20.0,
            "GarrisonContain::healObjects must sliver-heal, got {after}"
        );
        assert!(
            logic.host_object(unit).is_some_and(|o| o.contained_by == Some(pad)),
            "garrison heal must not auto-exit like HealContain"
        );
    }

    #[test]
    fn crate_vision_uses_shroud_clearing_range_and_unlooks_on_move() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        let mut tpl = ThingTemplate::new("VisionProbe");
        tpl.sight_range = 100.0;
        tpl.shroud_clearing_range = 240.0;
        logic.templates.insert("VisionProbe".into(), tpl);
        let id = logic
            .create_object_for_player("VisionProbe", 1, Vec3::new(10.0, 0.0, 20.0))
            .expect("spawn");

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        logic.update_main_crate_vision();
        let first = *logic.vision_last_looks.get(&id).expect("looker");
        assert!((first.3 - 240.0).abs() < 0.01, "look radius {}", first.3);
        logic.update_main_crate_vision();
        assert_eq!(logic.vision_last_looks.len(), 1);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.set_position(Vec3::new(80.0, 0.0, 90.0));
        }
        logic.update_main_crate_vision();
        let moved = *logic.vision_last_looks.get(&id).expect("moved looker");
        assert!((moved.0 - 80.0).abs() < 0.01);
        assert!((moved.1 - 90.0).abs() < 0.01);
        assert!((moved.3 - 240.0).abs() < 0.01);

        if let Some(obj) = logic.host_object_mut(id) {
            obj.health.current = 0.0;
        }
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_looks.contains_key(&id),
            "death must unlook"
        );
    }

    /// C++ PartitionManager.cpp:1582-1688 — object FOW is footprint COI mix.
    #[test]
    fn object_fow_uses_coi_mix_not_vision_range_circle() {
        use gamelogic::common::types::ObjectShroudStatus;
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));
        logic.add_player(Player::new(2, Team::China, "China", false));

        let mut looker = ThingTemplate::new("Looker");
        looker.sight_range = 100.0;
        looker.shroud_clearing_range = 80.0;
        logic.templates.insert("Looker".into(), looker);

        let mut scout = ThingTemplate::new("EnemyScout");
        scout.add_kind_of(KindOf::Infantry).set_health(80.0);
        logic.templates.insert("EnemyScout".into(), scout);

        let mut bunker = ThingTemplate::new("EnemyBunker");
        bunker
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Immobile)
            .set_health(400.0);
        bunker.geometry_info = crate::game_logic::HostGeometryInfo {
            geom_type: crate::game_logic::HostGeometryType::Box,
            is_small: false,
            height: 20.0,
            major_radius: 20.0,
            minor_radius: 20.0,
            authored: true,
        };
        logic.templates.insert("EnemyBunker".into(), bunker);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let _looker_id = logic
            .create_object_for_player("Looker", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("looker");
        let near_id = logic
            .create_object_for_player("EnemyScout", 2, Vec3::new(10.0, 0.0, 0.0))
            .expect("near");
        let far_id = logic
            .create_object_for_player("EnemyScout", 2, Vec3::new(300.0, 0.0, 0.0))
            .expect("far");
        let bunker_id = logic
            .create_object_for_player("EnemyBunker", 2, Vec3::new(10.0, 0.0, 10.0))
            .expect("bunker");

        logic.update_main_crate_vision();
        {
            let shroud = get_shroud_manager();
            let mgr = shroud.lock().expect("shroud");
            assert_eq!(
                mgr.get_host_object_shroud_status(1, near_id.0),
                Some(ObjectShroudStatus::Clear),
                "enemy on revealed cells is CLEAR"
            );
            assert_eq!(
                mgr.get_host_object_shroud_status(1, far_id.0),
                Some(ObjectShroudStatus::Shrouded),
                "enemy on unrevealed cells is SHROUDED, not a VisionRange ghost"
            );
            assert_eq!(
                mgr.get_host_object_shroud_status(1, bunker_id.0),
                Some(ObjectShroudStatus::Clear)
            );
            assert!(mgr.host_object_ever_seen(1, bunker_id.0));
        }

        if let Some(obj) = logic.host_object_mut(_looker_id) {
            obj.set_position(Vec3::new(300.0, 0.0, 300.0));
        }
        logic.update_main_crate_vision();
        {
            let shroud = get_shroud_manager();
            let mgr = shroud.lock().expect("shroud");
            assert_eq!(
                mgr.get_host_object_shroud_status(1, near_id.0),
                Some(ObjectShroudStatus::Shrouded),
                "mobile enemy in fog must not linger as a fog ghost"
            );
            assert_eq!(
                mgr.get_host_object_shroud_status(1, bunker_id.0),
                Some(ObjectShroudStatus::Fogged),
                "seen immobile enemy building stays FOGGED ghost"
            );
        }
    }

    #[test]
    fn looker_mask_uses_player_relationship_and_unlook_persist_150() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        assert_eq!(super::host_unlook_persist_frames(), 150);

        let mut logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.alliance_team = 7;
        let mut china = Player::new(1, Team::China, "China", false);
        china.alliance_team = 7;
        logic.add_player(usa);
        logic.add_player(china);

        let mut tpl = ThingTemplate::new("AllyLooker");
        tpl.sight_range = 50.0;
        tpl.shroud_clearing_range = 80.0;
        logic.templates.insert("AllyLooker".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("AllyLooker", 0, Vec3::new(10.0, 0.0, 10.0))
            .expect("spawn");
        logic.update_main_crate_vision();
        let look = *logic.vision_last_looks.get(&id).expect("looker");
        assert_ne!(look.4 & (1u32 << 0), 0, "owner bit");
        assert_ne!(look.4 & (1u32 << 1), 0, "script/skirmish ally must share look");
    }

    #[test]
    fn transport_passengers_stop_looking() {
        use crate::game_logic::{ContainModuleKind, ContainModuleMetadata};
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "USA", true));

        let mut chinook = ThingTemplate::new("ChinookLook");
        chinook.shroud_clearing_range = 200.0;
        chinook.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::Transport,
            slots: Some(8),
            ..ContainModuleMetadata::default()
        };
        logic.templates.insert("ChinookLook".into(), chinook);

        let mut ranger = ThingTemplate::new("RangerLook");
        ranger.shroud_clearing_range = 150.0;
        logic.templates.insert("RangerLook".into(), ranger);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let bird = logic
            .create_object_for_player("ChinookLook", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("chinook");
        let rider = logic
            .create_object_for_player("RangerLook", 1, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        if let Some(obj) = logic.host_object_mut(rider) {
            obj.set_contained_by(Some(bird));
        }
        logic.update_main_crate_vision();
        assert!(
            logic.vision_last_looks.contains_key(&bird),
            "container still looks"
        );
        assert!(
            !logic.vision_last_looks.contains_key(&rider),
            "transport passenger must not look"
        );
    }

    #[test]
    fn shroud_reveal_to_all_range_looks_for_enemies() {
        use gamelogic::system::shroud_manager::get_shroud_manager;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        logic.add_player(Player::new(1, Team::China, "China", false));

        let mut tpl = ThingTemplate::new("StratCenter");
        tpl.shroud_clearing_range = 200.0;
        tpl.shroud_reveal_to_all_range = 50.0;
        logic.templates.insert("StratCenter".into(), tpl);

        {
            let shroud = get_shroud_manager();
            let mut mgr = shroud.lock().expect("shroud");
            mgr.init_shroud_grid(512.0, 512.0);
        }

        let id = logic
            .create_object_for_player("StratCenter", 0, Vec3::new(20.0, 0.0, 20.0))
            .expect("center");
        logic.update_main_crate_vision();
        let reveal = *logic.vision_last_reveal_all.get(&id).expect("reveal-all");
        assert!((reveal.3 - 50.0).abs() < 0.01);
        assert_ne!(reveal.4 & (1u32 << 1), 0, "enemy bit in reveal-all mask");
        assert_eq!(reveal.4 & (1u32 << 0), 0, "owner is not enemies/neutral");

        if let Some(obj) = logic.host_object_mut(id) {
            obj.status.stealthed = true;
            obj.status.detected = false;
            obj.status.disguised = false;
        }
        logic.update_main_crate_vision();
        assert!(
            !logic.vision_last_reveal_all.contains_key(&id),
            "stealthed-not-detected skips reveal-to-all"
        );
    }

    #[test]
    fn ini_crusher_crushable_levels_stamp_live_objects() {
        // C++ ThingTemplate.cpp:226-227 parseUnsignedByte CrusherLevel/CrushableLevel.
        // C++ Object.cpp:1156-1164 getCrusherLevel/getCrushableLevel from template.
        // C++ Object.cpp:1146 crush when crusherLevel > crushableLevel.
        // C++ ToppleUpdate.cpp:352 / W3DTreeBuffer.cpp:1179 crusher_level > 1.
        let mut car_def = ObjectDefinition::new("CivilianCar01".to_string());
        car_def
            .attributes
            .insert("CrushableLevel".to_string(), "1".to_string());
        car_def
            .attributes
            .insert("KindOf".to_string(), "VEHICLE".to_string());

        let mut tank_def = ObjectDefinition::new("AmericaTankCrusader".to_string());
        tank_def
            .attributes
            .insert("CrusherLevel".to_string(), "2".to_string());
        tank_def
            .attributes
            .insert("CrushableLevel".to_string(), "2".to_string());
        tank_def
            .attributes
            .insert("KindOf".to_string(), "VEHICLE".to_string());

        let mut overlord_def = ObjectDefinition::new("ChinaTankOverlord".to_string());
        overlord_def
            .attributes
            .insert("CrusherLevel".to_string(), "3".to_string());
        overlord_def
            .attributes
            .insert("CrushableLevel".to_string(), "3".to_string());
        overlord_def
            .attributes
            .insert("KindOf".to_string(), "VEHICLE".to_string());

        let mut prop_def = ObjectDefinition::new("TreeOak".to_string());
        prop_def
            .attributes
            .insert("CrushableLevel".to_string(), "1".to_string());

        let car_t = GameLogic::build_template_from_object_definition(
            "CivilianCar01",
            &car_def,
            None,
        );
        let tank_t = GameLogic::build_template_from_object_definition(
            "AmericaTankCrusader",
            &tank_def,
            None,
        );
        let ov_t = GameLogic::build_template_from_object_definition(
            "ChinaTankOverlord",
            &overlord_def,
            None,
        );
        let prop_t = GameLogic::build_template_from_object_definition("TreeOak", &prop_def, None);

        assert_eq!(car_t.crusher_level, 0);
        assert_eq!(car_t.crushable_level, 1);
        assert_eq!(tank_t.crusher_level, 2);
        assert_eq!(tank_t.crushable_level, 2);
        assert_eq!(ov_t.crusher_level, 3);
        assert_eq!(ov_t.crushable_level, 3);
        assert_eq!(prop_t.crushable_level, 1);

        let car = Object::new(car_t, ObjectId(1), Team::Neutral);
        let tank = Object::new(tank_t, ObjectId(2), Team::USA);
        let ov = Object::new(ov_t, ObjectId(3), Team::China);
        let prop = Object::new(prop_t, ObjectId(4), Team::Neutral);

        assert_eq!(car.crushable_level, 1);
        assert_eq!(car.crusher_level, 0);
        assert_eq!(tank.crusher_level, 2);
        assert_eq!(ov.crusher_level, 3);
        assert_eq!(prop.crushable_level, 1);
        assert!(ov.crusher_level > tank.crusher_level);

        use crate::game_logic::host_partition_collision_physics_residual::can_crush_only_residual;
        assert!(
            can_crush_only_residual(tank.crusher_level, car.crushable_level, false, false),
            "tank CrusherLevel 2 must crush car CrushableLevel 1"
        );
        assert!(
            can_crush_only_residual(tank.crusher_level, prop.crushable_level, false, false),
            "tank must crush props"
        );
        assert!(
            can_crush_only_residual(ov.crusher_level, tank.crushable_level, false, false),
            "Overlord 3 must crush tank 2"
        );
        assert!(!can_crush_only_residual(
            tank.crusher_level,
            ov.crushable_level,
            false,
            false
        ));
        assert!(!crate::game_logic::host_topple::crusher_can_topple(1));
        assert!(crate::game_logic::host_topple::crusher_can_topple(
            tank.crusher_level
        ));
        assert!(crate::game_logic::host_topple::crusher_can_topple(
            ov.crusher_level
        ));

        // KindOf Vehicle must not invent CrusherLevel=1 (pre-fix physics_motion.rs:113).
        let mut bare = ThingTemplate::new("BareCar");
        bare.add_kind_of(KindOf::Vehicle);
        let bare_obj = Object::new(bare, ObjectId(5), Team::Neutral);
        assert_eq!(bare_obj.crusher_level, 0);
        assert_eq!(bare_obj.crushable_level, 255);
    }

    #[test]
    fn ini_geometry_stamps_live_template_not_kind_radii() {
        // C++ ThingTemplate.cpp:201-205 / Geometry.cpp:26-58 parse Geometry*.
        // Retail AmericaTankBattleMaster: BOX 13 / 9 / 10, IsSmall Yes.
        let mut def = ObjectDefinition::new("AmericaTankBattleMaster".to_string());
        def.attributes
            .insert("KindOf".to_string(), "VEHICLE SELECTABLE CAN_ATTACK".to_string());
        def.attributes
            .insert("Geometry".to_string(), "BOX".to_string());
        def.attributes
            .insert("GeometryMajorRadius".to_string(), "13.0".to_string());
        def.attributes
            .insert("GeometryMinorRadius".to_string(), "9.0".to_string());
        def.attributes
            .insert("GeometryHeight".to_string(), "10.0".to_string());
        def.attributes
            .insert("GeometryIsSmall".to_string(), "Yes".to_string());
        def.attributes
            .insert("StructureRubbleHeight".to_string(), "8".to_string());

        let template = GameLogic::build_template_from_object_definition(
            "AmericaTankBattleMaster",
            &def,
            None,
        );
        assert!(template.geometry_info.authored);
        assert_eq!(
            template.geometry_info.geom_type,
            crate::game_logic::HostGeometryType::Box
        );
        assert!((template.geometry_info.major_radius - 13.0).abs() < 1e-4);
        assert!((template.geometry_info.minor_radius - 9.0).abs() < 1e-4);
        assert!((template.geometry_info.height - 10.0).abs() < 1e-4);
        assert!(template.geometry_info.is_small);
        assert_eq!(template.structure_rubble_height, 8);

        let obj = Object::new(template, ObjectId(1), Team::USA);
        let expected_circle = 13.0f32.hypot(9.0);
        assert!(
            (obj.selection_radius - expected_circle).abs() < 1e-4,
            "selection must be bounding circle {}, not Vehicle 15; got {}",
            expected_circle,
            obj.selection_radius
        );
        assert!((obj.thing.geometry.radius - expected_circle).abs() < 1e-4);
        assert!((obj.thing.geometry.bounds_max.x - 13.0).abs() < 1e-4);
        assert!((obj.thing.geometry.bounds_max.z - 9.0).abs() < 1e-4);
        assert!((obj.thing.geometry.bounds_max.y - 10.0).abs() < 1e-4);

        // Unknown Geometry token fails closed (keeps default SPHERE).
        let mut bad = ObjectDefinition::new("BogusGeom".to_string());
        bad.attributes
            .insert("Geometry".to_string(), "PYRAMID".to_string());
        let bare = GameLogic::build_template_from_object_definition("BogusGeom", &bad, None);
        assert!(!bare.geometry_info.authored);
        assert_eq!(
            bare.geometry_info.geom_type,
            crate::game_logic::HostGeometryType::Sphere
        );
    }

    #[test]
    fn ini_shroud_reveal_to_all_range_and_kindofs() {
        let mut def = ObjectDefinition::new("AmericaStrategyCenter".to_string());
        def.attributes.insert(
            "ShroudRevealToAllRange".to_string(),
            "50.0".to_string(),
        );
        def.attributes.insert(
            "KindOf".to_string(),
            "STRUCTURE REVEAL_TO_ALL ALWAYS_VISIBLE".to_string(),
        );
        let template = GameLogic::build_template_from_object_definition(
            "AmericaStrategyCenter",
            &def,
            None,
        );
        assert!((template.shroud_reveal_to_all_range - 50.0).abs() < 1e-4);
        assert!(template.reveal_to_all);
        assert!(template.always_visible);
    }

    #[test]
    fn omitted_experience_value_defaults_to_zero_not_invented_50_100() {
        let tree = GameLogic::build_template_from_object_definition(
            "Tree",
            &ObjectDefinition::new("Tree".to_string()),
            None,
        );
        assert_eq!(tree.experience_value, 0.0);
        assert_eq!(tree.experience_values, [0.0; 4]);

        let mut structure = ObjectDefinition::new("CivilianBuilding".to_string());
        structure
            .attributes
            .insert("KindOf".to_string(), "STRUCTURE".to_string());
        let structure = GameLogic::build_template_from_object_definition(
            "CivilianBuilding",
            &structure,
            None,
        );
        assert_eq!(structure.experience_value, 0.0);
        assert_eq!(structure.experience_values, [0.0; 4]);

        let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
        authored
            .attributes
            .insert("ExperienceValue".to_string(), "20 20 40 60".to_string());
        let authored = GameLogic::build_template_from_object_definition(
            "AmericaInfantryRanger",
            &authored,
            None,
        );
        assert_eq!(authored.experience_value, 20.0);
        assert_eq!(authored.experience_values, [20.0, 20.0, 40.0, 60.0]);
    }

    #[test]
    fn omitted_vision_range_defaults_to_zero_not_150() {
        let tree = GameLogic::build_template_from_object_definition(
            "Tree",
            &ObjectDefinition::new("Tree".to_string()),
            None,
        );
        assert_eq!(tree.sight_range, 0.0);

        let mut authored = ObjectDefinition::new("AmericaInfantryRanger".to_string());
        authored
            .attributes
            .insert("VisionRange".to_string(), "150.0".to_string());
        let authored = GameLogic::build_template_from_object_definition(
            "AmericaInfantryRanger",
            &authored,
            None,
        );
        assert!((authored.sight_range - 150.0).abs() < 1e-4);

        let mut zero = ObjectDefinition::new("Prop".to_string());
        zero.attributes
            .insert("VisionRange".to_string(), "0".to_string());
        let zero = GameLogic::build_template_from_object_definition("Prop", &zero, None);
        assert_eq!(zero.sight_range, 0.0);
    }

}



//! Object-definition template construction and authored metadata.
use super::*;

impl GameLogic {
    /// C++ parity: veterancy-level XP multiplier. In C++ each template
    /// defines per-level ExperienceValue; we approximate by scaling the
    /// base value.  C++ values are modest multipliers, not large ones.
    pub(in super::super::super) fn veterancy_xp_multiplier(level: VeterancyLevel) -> f32 {
        match level {
            VeterancyLevel::Rookie => 1.0,
            VeterancyLevel::Veteran => 1.25,
            VeterancyLevel::Elite => 1.5,
            VeterancyLevel::Heroic => 2.0,
        }
    }

    pub(in super::super::super) fn should_track_player_stats(&self) -> bool {
        self.sim_time_seconds > 0.0 || self.frame > 0
    }

    pub(in super::super::super) fn live_score_counts_as_unit_create(obj: &Object) -> bool {
        !obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Infantry) || obj.is_kind_of(KindOf::Vehicle))
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreCreate))
    }

    pub(in super::super::super) fn live_score_counts_as_building_create(obj: &Object) -> bool {
        obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreCreate))
    }

    pub(in super::super::super) fn live_score_counts_as_unit_destroy(obj: &Object) -> bool {
        !obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Infantry) || obj.is_kind_of(KindOf::Vehicle))
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreDestroy))
    }

    pub(in super::super::super) fn live_score_counts_as_building_destroy(obj: &Object) -> bool {
        obj.is_kind_of(KindOf::Structure)
            && (obj.is_kind_of(KindOf::Score) || obj.is_kind_of(KindOf::ScoreDestroy))
    }

    pub(in super::super::super) fn record_unit_production(&mut self, object_id: ObjectId) {
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

    pub(in super::super::super) fn record_structure_completion(&mut self, object_id: ObjectId) {
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

    pub(in super::super::super) fn template_counts_as_unit(template: &ThingTemplate) -> bool {
        !template.is_kind_of(KindOf::Structure)
            && (template.is_kind_of(KindOf::Infantry)
                || template.is_kind_of(KindOf::Vehicle)
                || template.is_kind_of(KindOf::Aircraft))
    }

    pub(in super::super::super) fn should_skip_map_object_template(template_name: &str) -> bool {
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

    pub(in super::super::super) fn should_spawn_fallback_template(template_name: &str) -> bool {
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

    pub(in super::super::super) fn build_template_from_asset_definition(
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
        Self::apply_authored_leftover_sa_trigger_sound(&mut template, definition);

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
            template.shadow_type =
                crate::game_logic::host_enum_table_residual::parse_shadow_type_bits(
                    shadow.as_str(),
                );
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
        if let Some(amount) = definition
            .subdual_heal_amount
            .filter(|amount| amount.is_finite())
        {
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

        // C++ ThingTemplate.cpp:223 INI::parseUnsignedShort ThreatValue.
        // 0 is authored (Object::addThreat stamps getThreatValue as-is).
        if let Some(threat) = Self::object_definition_attr(definition, "threatvalue")
            .and_then(|s| s.trim().parse::<u16>().ok())
        {
            template.threat_value = threat;
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
            super::super::super::locomotor_bootstrap::locomotor_name_for_unit(template_name)
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
        apply_production_prerequisites_from_definition(&mut template, definition);

        template
    }
}

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

#[inline]
fn definition_has_rider_change_contain(definition: &crate::assets::ObjectDefinition) -> bool {
    definition
        .behavior_modules
        .iter()
        .any(|module| module.class_name.eq_ignore_ascii_case("RiderChangeContain"))
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

    pub(in super::super) fn record_unit_production(&mut self, team: Team) {
        if !self.should_track_player_stats() {
            return;
        }
        if let Some(player_id) = self.player_id_for_team(team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_unit_produced();
            }
        }
    }

    pub(in super::super) fn record_structure_completion(&mut self, team: Team) {
        if !self.should_track_player_stats() {
            return;
        }
        if let Some(player_id) = self.player_id_for_team(team) {
            if let Some(player) = self.players.get_mut(&player_id) {
                player.record_structure_built();
            }
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

        // C++ data includes audio-only ambient map objects with Draw blocks that contain no model.
        // Keep them out of visual spawn synthesis to avoid bogus model fallback loads.
        if definition.model_name.is_none()
            && Self::object_definition_attr(&definition, "soundambient").is_some()
        {
            return None;
        }

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
        Self::apply_authored_deploy_style_metadata(&mut template, definition);
        Self::apply_authored_supply_truck_metadata(&mut template, definition);
        Self::apply_authored_production_exit_metadata(&mut template, definition);
        Self::apply_authored_pilot_veterancy_metadata(&mut template, definition);
        Self::apply_authored_eject_pilot_die_metadata(&mut template, definition);
        Self::apply_authored_hack_internet_metadata(&mut template, definition);
        Self::apply_authored_special_power_module_metadata(&mut template, definition);
        Self::apply_authored_hacker_disable_building_metadata(&mut template, definition);
        Self::apply_authored_overcharge_metadata(&mut template, definition);
        Self::apply_authored_power_plant_update_metadata(&mut template, definition);
        Self::apply_authored_temporary_weapon_behavior_metadata(&mut template, definition);

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

        // C++ parity: parse ExperienceValue from INI (first value = Rookie level).
        // If not set, use a default based on the object type.
        let xp_val = Self::object_definition_attr(definition, "experiencevalue")
            .and_then(|s| s.split_whitespace().next()?.parse::<f32>().ok())
            .unwrap_or(if is_structure { 100.0 } else { 50.0 });
        template.experience_value = xp_val;

        // C++ parity: parse Armor from INI (default 0).
        if let Some(armor_val) = Self::object_definition_attr(definition, "armor")
            .and_then(|s| s.trim().parse::<f32>().ok())
        {
            template.armor = armor_val;
        }

        // C++ parity: parse VisionRange from INI.
        if let Some(sight) = Self::object_definition_attr(definition, "visionrange")
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|&v| v > 0.0)
        {
            template.sight_range = sight;
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
                let (active_locomotor_name, active_locomotor_names, active_locomotor_surfaces) =
                    if locomotor_set.eq_ignore_ascii_case("SET_NORMAL") {
                        normal_locomotor_binding
                            .as_ref()
                            .map(|binding| {
                                (
                                    Some(binding.representative_name.clone()),
                                    binding.locomotor_names.clone(),
                                    binding.locomotor_surfaces,
                                )
                            })
                            .unwrap_or((None, Vec::new(), 0))
                    } else {
                        (None, Vec::new(), 0)
                    };
                let physical_enter_supported = slot <= 7
                    && model_condition_mask != 0
                    && object_status_mask != 0
                    && model_condition.eq_ignore_ascii_case(&expected_model_condition)
                    && weapon_set.eq_ignore_ascii_case(&expected_weapon_set)
                    && object_status.eq_ignore_ascii_case(&expected_object_status)
                    && !command_set.is_empty()
                    // Retail's Terrorist row uses `SET_SLUGGISH`, which
                    // selects a separate multi-surface locomotor table.  The
                    // active Rust ThingTemplate retains one resolved
                    // locomotor, not that table, so admitting it would leave
                    // the bike on the previous/default movement behavior.
                    // Keep the exact record for presentation/diagnostics but
                    // fail this row closed until all set-specific locomotors
                    // are representable.  `SET_NORMAL` is the only set the
                    // live movement path can apply without approximation.
                    && locomotor_set.eq_ignore_ascii_case("SET_NORMAL")
                    && normal_locomotor_binding.is_some();
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
                    parse_rider_change(module, rider_change_normal_locomotors)
                } else {
                    (Vec::new(), None, String::new(), 0)
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
                module
                    .class_name
                    .eq_ignore_ascii_case("SupplyTruckAIUpdate")
            })
            .collect();
        let [module] = modules.as_slice() else {
            template.supply_truck_metadata = None;
            return;
        };
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
                }),
                ProductionExitStyle::SupplyCenter => Some(ProductionExitMetadata {
                    style,
                    unit_create_point,
                    natural_rally_point,
                    exit_delay_frames: 0,
                    allow_airborne_creation: false,
                    initial_burst: 0,
                    use_spawn_rally_point: false,
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
        template.max_simultaneous_determined_by_superweapon_restriction =
            Self::object_definition_attr(definition, "maxsimultaneousoftype").is_some_and(
                |value| {
                    stripped_value(&value)
                        .eq_ignore_ascii_case("DeterminedBySuperweaponRestriction")
                },
            );

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

    /// Retain a complete, exact `SPECIAL_HACKER_DISABLE_BUILDING` module pair.
    ///
    /// C++ `ActionManager::canDisableBuildingViaHacking` queries the source
    /// `SpecialAbility` module, while the actual target channel belongs to a
    /// matching `SpecialAbilityUpdate`.  An HDB enum by itself is therefore
    /// insufficient.  Parse only a unique pair pointing at the *same loaded*
    /// SpecialPower template; a partial, duplicate, or malformed pair remains
    /// unavailable rather than inheriting retail Hacker timings by name.
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
                        // Common's enum deliberately aliases the retail
                        // Microwave template to SPECIAL_HACKER_DISABLE_BUILDING.
                        // HDB's player channel belongs only to the exact
                        // `SpecialAbilityHackerDisableBuilding` identity in
                        // CommandButton/SpecialPower data, not every source
                        // template with that shared C++ enum value.
                        (power.power_type == SpecialPowerType::HackerDisableBuilding
                            && special_power_type_from_template_name(&power.name)
                                == Some(HostSpecialPowerType::HackerDisableBuilding))
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
                persistence_requires_recharge: match update.attribute("PersistenceRequiresRecharge")
                {
                    Some(value) => parse_bool(value)?,
                    None => false,
                },
            })
        };
        template.hacker_disable_building = parse();
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

    /// C++ Object.cpp:160-497 builds weapons and modules from ThingTemplate
    /// INI data, not from unit-name residuals.  Capture the create-time
    /// policy bits `create_object_with_owner` used to hardcode.
    fn apply_authored_weapon_set_create_policy(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        template.primary_auto_choose_none = definition
            .weapon_sets
            .iter()
            .find(|set| set.is_unconditional())
            .is_some_and(|set| set.auto_choose_primary_none());
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
                Self::apply_authored_parking_place_metadata(template, &definition);
                Self::apply_authored_supply_truck_metadata(template, &definition);
                // Starter templates are retained by the host before the full
                // Object INI catalogue is seeded.  They still need the exact
                // authored exit interface; otherwise a retail ChinaBarracks
                // silently falls back to the legacy/no-interface production
                // path solely because it already existed in the template map.
                Self::apply_authored_production_exit_metadata(template, &definition);
                Self::apply_authored_eject_pilot_die_metadata(template, &definition);
                Self::apply_authored_hack_internet_metadata(template, &definition);
                Self::apply_authored_special_power_module_metadata(template, &definition);
                Self::apply_authored_hacker_disable_building_metadata(template, &definition);
                Self::apply_authored_overcharge_metadata(template, &definition);
                Self::apply_authored_power_plant_update_metadata(template, &definition);
                Self::apply_authored_temporary_weapon_behavior_metadata(template, &definition);
                Self::apply_authored_weapon_set_create_policy(template, &definition);
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

            if Self::should_skip_map_object_template(name)
                // Cinematics/effect anchors have dedicated execution paths;
                // a global template seed must not make them map-spawnable.
                // The host does not yet have SoundAmbient behavior.  Retain
                // the lazy path's existing exclusion instead of adding
                // invisible, silent proxy objects.
                || (definition.model_name.is_none()
                    && Self::object_definition_attr(&definition, "soundambient").is_some())
            {
                continue;
            }

            let Some(model_name) = definition
                .model_name
                .as_deref()
                .map(str::trim)
                .filter(|model| !model.is_empty() && !model.eq_ignore_ascii_case("none"))
            else {
                // The generic host template cannot represent an object whose
                // only identity is a behavior/draw module.  Keep it out of
                // broad production seeding until that module is ported.
                continue;
            };
            if let Some(available_models) = available_models {
                let exact_key = Self::normalize_model_lookup_key(model_name);
                if !available_models.contains(&exact_key) {
                    log::debug!(
                        "Not seeding retail template '{name}': exact W3D '{model_name}' is unavailable"
                    );
                    continue;
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

    /// Build a visual-only fallback only when the object's own exact basename
    /// exists in the mounted archive.  C++ ConditionState/faction resolution
    /// belongs to W3DModelDraw; this generic path may not substitute a nearby
    /// mesh when the authored one is absent.
    pub(in super::super) fn build_fallback_template(template_name: &str) -> Option<ThingTemplate> {
        let lower = template_name.to_ascii_lowercase();
        let mut template = ThingTemplate::new(template_name);
        template.set_health(250.0);
        let fallback_model_name = Self::resolve_spawn_model_name(template_name)?;
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

    /// Feed Main-crate object positions and sight ranges into the
    /// gamelogic ShroudManager so that fog-of-war reveals around
    /// player-owned units and structures.
    ///
    /// The gamelogic ShroudManager's own `update()` only iterates
    /// objects in the gamelogic OBJECT_REGISTRY; Main-crate objects
    /// are not registered there, so we must push vision directly.
    pub(in super::super) fn update_main_crate_vision(&self) {
        use gamelogic::common::Coord3D;

        let shroud = get_shroud_manager();
        let mut shroud_mgr = match shroud.lock() {
            Ok(mgr) => mgr,
            Err(_) => return,
        };

        // Host residual: clear current object visibility membership for known players
        // before rebuilding from Main objects (explored territory persists).
        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for &pid in &player_ids {
            shroud_mgr.clear_host_object_visibility(pid);
        }

        // Snapshot alive viewers with vision + all alive targets once.
        let mut viewers: Vec<(crate::game_logic::ObjectId, u32, glam::Vec3, f32)> = Vec::new();
        let mut targets: Vec<(crate::game_logic::ObjectId, glam::Vec3)> = Vec::new();
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            let pos = obj.get_position();
            targets.push((obj.id, pos));
            let vision_range = obj.get_template().sight_range;
            if vision_range <= 0.0 {
                continue;
            }
            let Some(owner_pid) = self.player_id_for_team(obj.team) else {
                continue;
            };
            viewers.push((obj.id, owner_pid, pos, vision_range));

            // Terrain looker residual (grid FOW) for allies sharing vision.
            let center = Coord3D::new(pos.x, pos.z, pos.y);
            let mut player_mask = 0u32;
            for (&pid, player) in &self.players {
                if player.team == obj.team {
                    player_mask |= 1u32 << pid.min(31);
                }
            }
            if player_mask != 0 {
                shroud_mgr.do_shroud_reveal(&center, vision_range, player_mask);
            }
        }

        // Own-force residual: every alive object on a player's team is always
        // membership-visible to that player (C++ always draws controlling player units).
        for obj in self.objects.values() {
            if !obj.is_alive() {
                continue;
            }
            for (&pid, player) in &self.players {
                if player.team == obj.team && player.team != Team::Neutral {
                    shroud_mgr.mark_host_object_seen(pid, obj.id.0);
                }
            }
        }

        // Object membership residual: mark host objects seen by each viewer's allies.
        // Required because ShroudManager::update() only consults ObjectManager, which
        // does not hold Main host objects on the default authority path.
        for &(viewer_id, owner_pid, viewer_pos, vision_range) in &viewers {
            let mut ally_pids: Vec<u32> = self
                .players
                .iter()
                .filter_map(|(&pid, p)| {
                    self.players
                        .get(&owner_pid)
                        .map(|owner| p.team == owner.team)
                        .unwrap_or(false)
                        .then_some(pid)
                })
                .collect();
            if ally_pids.is_empty() {
                ally_pids.push(owner_pid);
            }
            let range_sq = vision_range * vision_range;
            for &pid in &ally_pids {
                // Always see the viewer itself.
                shroud_mgr.mark_host_object_seen(pid, viewer_id.0);
            }
            for &(target_id, target_pos) in &targets {
                if target_id == viewer_id {
                    continue;
                }
                let dx = target_pos.x - viewer_pos.x;
                let dz = target_pos.z - viewer_pos.z;
                if dx * dx + dz * dz <= range_sq {
                    for &pid in &ally_pids {
                        shroud_mgr.mark_host_object_seen(pid, target_id.0);
                    }
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

        // `HackInternetState` decrements a positive timer, then deposits on
        // its following update.  The contained hacker therefore fires at
        // 54 + 1, before the field hacker's 60 + 1.
        logic.frame = 54;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 0);
        logic.frame = 55;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
        logic.frame = 60;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 5);
        logic.frame = 61;
        logic.update_hacker_income();
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, 10);
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
        // This synthetic probe intentionally gives Microwave the same Common
        // enum/module shape as HDB.  It is the collision the exact template
        // lookup protects against; a real Microwave runtime stays separately
        // unsupported rather than borrowing HDB's persistent channel.
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
        assert!(
            microwave.hacker_disable_building.is_none(),
            "a Common SPECIAL_HACKER_DISABLE_BUILDING enum alias must not expose the Hacker channel"
        );
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
            button
                .command_name
                .eq_ignore_ascii_case("Command_ParticleCannon")
                && button.enabled
        }));
        logic.select_objects(1, vec![spoof_id]);
        let spoof_frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 1);
        assert!(!spoof_frame.unit_command_buttons().iter().any(|button| {
            button
                .command_name
                .eq_ignore_ascii_case("Command_ParticleCannon")
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

        assert_eq!(inserted, 1);
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
        assert!(!logic.templates.contains_key("AmbientOnlyRetailAnchor"));
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
}

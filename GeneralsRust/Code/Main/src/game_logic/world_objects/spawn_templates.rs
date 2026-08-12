//! Host objects `impl GameLogic` — `spawn_templates`.
//! templates, vision, spawn_faction_base. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

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

    pub(in super::super) fn build_template_from_asset_definition(template_name: &str) -> Option<ThingTemplate> {
        let manager_arc = get_asset_manager()?;
        let remapped_model = Self::remap_known_model_alias(template_name);
        let (definition, texture_hint) = {
            let manager = manager_arc.lock().ok()?;
            let definition = manager
                .resolve_object_definition(template_name, Some(remapped_model.as_str()))
                .or_else(|| manager.resolve_object_definition(template_name, None))
                .cloned()?;
            let texture_hint = manager
                .get_texture_for_object(template_name)
                .or_else(|| manager.get_texture_for_object(remapped_model.as_str()));
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

    pub(in super::super) fn build_template_from_object_definition(
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
            if hit_points > 0 {
                template.set_health(hit_points as f32);
            }
        }

        if let Some(model_name) = definition.model_name.as_deref() {
            let model_name = model_name.trim();
            if !model_name.is_empty() && !model_name.eq_ignore_ascii_case("none") {
                let resolved_model_name = Self::resolve_spawn_model_name(model_name)
                    .unwrap_or_else(|| Self::remap_known_model_alias(model_name));
                template.set_model(&resolved_model_name);
            }
        }

        let primary_texture = texture_hint.or_else(|| definition.get_primary_texture());
        if let Some(texture_name) = primary_texture {
            let texture_name = texture_name.trim();
            if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                template.texture_name = Some(texture_name.to_string());
            }
        }

        // Retail SupplyDock/SupplyPile carry SUPPLY_SOURCE (not "resource"/"harvest")
        // KindOf bits; map props must still be gatherable by dozer/chinook paths.
        let kind_compact = kind_of.replace('_', "");
        let is_resource = lower.contains("supplypile")
            || lower.contains("supplydock")
            || lower.contains("tempsupplydock")
            || lower.contains("crate")
            || kind_of.contains("resource")
            || kind_of.contains("harvest")
            || kind_compact.contains("supplysource");
        let is_structure = kind_of.contains("structure")
            || kind_of.contains("immobile")
            || (Self::should_spawn_fallback_template(template_name) && !is_resource);

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        }
        if is_structure {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Attackable);
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

        // Primary weapon name from Object INI (Weapon = PRIMARY Foo) for WeaponStore bind.
        if let Some(wname) = definition.primary_weapon.as_deref() {
            template.set_primary_weapon_name(wname);
        } else if let Some(raw) = Self::object_definition_attr(definition, "weapon") {
            // Fallback: scan attribute "PRIMARY Name" (last Weapon= line may be secondary)
            let mut parts = raw.split_whitespace();
            if parts
                .next()
                .map(|s| s.eq_ignore_ascii_case("PRIMARY"))
                .unwrap_or(false)
            {
                if let Some(wname) = parts.next() {
                    template.set_primary_weapon_name(wname);
                }
            }
        }

        // Secondary weapon name from Object INI (Weapon = SECONDARY Foo). Fail-closed residual.
        if let Some(wname) = definition.secondary_weapon.as_deref() {
            template.set_secondary_weapon_name(wname);
        }

        // SET_NORMAL Locomotor name from Object INI when present; else known host map.
        // Fail-closed residual: single primary locomotor only (not multi-set / surface matrix).
        if let Some(raw) = Self::object_definition_attr(definition, "locomotor") {
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

    pub(in super::super) fn add_faction_structure_kind_bits(template: &mut ThingTemplate, kind_of: &str) {
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

    pub(in super::super) fn object_definition_attr(definition: &ObjectDefinition, key: &str) -> Option<String> {
        definition
            .attributes
            .iter()
            .find_map(|(attr, value)| attr.eq_ignore_ascii_case(key).then(|| value.clone()))
    }

    pub(in super::super) fn remap_known_model_alias(model_name: &str) -> String {
        let model_name_lower = model_name.to_ascii_lowercase();
        if let Some(alias) = Self::remap_pt_vegetation_alias(&model_name_lower) {
            return alias.to_string();
        }

        match model_name_lower.as_str() {
            // Defcon6 / neutral civilian model aliases that do not exist under their INI base id
            // in the mounted archive set, but have shipped equivalents.
            "cbnukebunk2" => "CBNukeBunk".to_string(),
            "pmcrates01" => "PMWldCrate".to_string(),
            "pmcrates03" => "PMWldCrate".to_string(),
            "pmcrat01" => "PMWldCrate".to_string(),
            "pmcrat02" => "PMWldCrate".to_string(),
            "zbsmalpile" => "ZBSmalPile_S".to_string(),
            "cbbunker01" => "CBBunker01_SN".to_string(),
            "cbtower2" => "CBTower2_SN".to_string(),
            "cbtower" => "CBTower01".to_string(),
            "cbtower02" => "CBTower02_SN".to_string(),
            "cbtower03" => "CBTower03_SN".to_string(),
            "cbtower04" => "CBTower03_SN".to_string(),
            "cbtower05" => "CBTower05_N".to_string(),
            "cbtaltower" => "CBTalTower_N".to_string(),
            "cbtaltower_tr" => "CBTalTower_N".to_string(),
            "cbtower01_tr" => "CBTower02_TR".to_string(),
            "cbtower04_tr" => "CBTower03_SN".to_string(),
            "cbtower05_tr" => "CBTower05_N".to_string(),
            "cbtoildepo" => "CBOilRefny".to_string(),
            "cbtoiltnk1" => "CBOilRefny".to_string(),
            "cbtoiltnk2" => "CBOilRefny".to_string(),
            "cboilrfny" => "CBOilRfny_SN".to_string(),
            "cbchembunk" => "CBChemBunk_SN".to_string(),
            "pmwtrtwr" => "PMTower".to_string(),
            "pmwtrtwr02" => "PMTower2".to_string(),
            "pmctrslpy" => "PMDock08".to_string(),
            "absupdrop" => "PMWldCrate".to_string(),
            "uvtechjeep" => "UVTechJeep_d4".to_string(),
            "uvtechvan" => "UVTechVan_d1".to_string(),
            "uvtechtrck" => "UVTechTrck_D4".to_string(),
            "nvssupplytk" => "NVSSupplyTk_B".to_string(),
            "nbptower" => "NBPwrPti".to_string(),
            "nbbunker" => "NBBunkerI".to_string(),
            "zbhospibib" => "ZBHospibib_S".to_string(),
            "cbnfcitych" => "CBCityBlok".to_string(),
            "salvagecrate" => "PMWldCrate".to_string(),
            "smalllevelupcrate" => "PMWldCrate".to_string(),
            "mediumlevelupcrate" => "PMWldCrate".to_string(),
            "2freecrusaderscrate" => "PMWldCrate".to_string(),
            "100dollarcrate" => "PMWldCrate".to_string(),
            "200dollarcrate" => "PMWldCrate".to_string(),
            "1000dollarcrate" => "PMWldCrate".to_string(),
            "1500dollarcrate" => "PMWldCrate".to_string(),
            "2500dollarcrate" => "PMWldCrate".to_string(),
            "zzsupplydock" => "PMWldCrate".to_string(),
            "zbsupplydk" => "PMWldCrate".to_string(),
            // Decorative map-object aliases observed in challenge/skirmish maps.
            "pmboulders" => "PMBoulders_D".to_string(),
            "pmlclusters" => "PMLClusters_D".to_string(),
            "pmmcluster" => "PMMCluster_D".to_string(),
            "pmcluster" => "PMCluster_D".to_string(),
            "pmrocks02" | "pmrocks03" | "pmrocks05" | "pmrocks06" | "pmrocks07" => {
                "PMBoulders_D".to_string()
            }
            "pmrocks01b" | "pmrocks02b" => "PMBoulders_D".to_string(),
            // Zero Hour INIs reference a few decorative props whose exact W3D ids are absent from
            // the mounted archive set in this workspace. Route them to the closest shipped props
            // so challenge/shell maps keep their background dressing instead of dropping objects.
            "ptcypress01" => "PTXARBVT01".to_string(),
            "ptxpine03" => "PTXFIR07".to_string(),
            "pmswing" => "PMBikeRack".to_string(),
            "pmplygdst" => "PMPavilion".to_string(),
            // AVChinook_A2 is an animation-root file; route model fallback to renderable mesh.
            "avamphib" | "avamphib_a" | "avamphib_a1" => "AVChinook".to_string(),
            "avchinook_a2" => "AVChinook_A2MSH".to_string(),
            "avpaladin" => "AVCrusader_A".to_string(),
            "avpaladin_d" => "avcrusader_d".to_string(),
            "avpaladin_d1" | "avpaladin_d2" | "avpaladin_d3" => "avcrusader_d1".to_string(),
            "pmtrshpp03" | "pmtrshpl02" => "PMBrnTrshPl_D".to_string(),
            "pmpump" => "PMWldCrate".to_string(),
            "pmcrates" => "PMWldCrate".to_string(),
            "cbsandbw2" => "CBSandBWY1".to_string(),
            "cbsandbw4c" => "CBSandBWX".to_string(),
            "cvtruck" => "CVTruck_D1".to_string(),
            "cbnshack" => "CBNShack_S".to_string(),
            "cbtraintnl" => "UIRTunnel".to_string(),
            _ => model_name.to_string(),
        }
    }

    pub(in super::super) fn pt_vegetation_alias_mode() -> &'static str {
        static MODE: OnceLock<String> = OnceLock::new();
        MODE.get_or_init(|| {
            std::env::var("GENERALS_PT_VEGETATION_ALIAS_MODE")
                .unwrap_or_else(|_| "all_fir".to_string())
                .to_ascii_lowercase()
        })
        .as_str()
    }

    pub(in super::super) fn remap_pt_vegetation_alias(model_name_lower: &str) -> Option<&'static str> {
        let tree_target = match Self::pt_vegetation_alias_mode() {
            "trees_birch" | "all_birch" => Some("PTXBirch06"),
            "trees_oak" | "all_oak" => Some("PTXOak06"),
            "trees_palm" | "all_palm" => Some("PTPalm01"),
            "trees_maple" | "all_maple" => Some("PTMaple02"),
            "trees" | "trees_fir" | "all" | "all_fir" | "tree_pine1" | "tree_pine2"
            | "tree_spruce2" | "tree_spruce05" | "trees_pines" | "trees_spruces"
            | "trees_three" | "bushes_pines" | "bushes_spruces" => Some("PTXFir07"),
            _ => None,
        };

        match Self::pt_vegetation_alias_mode() {
            "bushes" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                _ => None,
            },
            "trees" | "trees_fir" | "trees_birch" | "trees_oak" | "trees_palm" | "trees_maple" => {
                match model_name_lower {
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            "tree_pine1" => match model_name_lower {
                "ptpine01" => tree_target,
                _ => None,
            },
            "tree_pine2" => match model_name_lower {
                "ptpine02" => tree_target,
                _ => None,
            },
            "tree_spruce2" => match model_name_lower {
                "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "tree_spruce05" => match model_name_lower {
                "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_pines" => match model_name_lower {
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "trees_spruces" => match model_name_lower {
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_three" => match model_name_lower {
                "ptpine01" | "ptpine02" | "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "bushes_pines" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "bushes_spruces" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "all" | "all_fir" | "all_birch" | "all_oak" | "all_palm" | "all_maple" => {
                match model_name_lower {
                    "ptbush02" => Some("PTBush17"),
                    "ptbush03" => Some("PTBush18"),
                    "ptbush08" => Some("PTBush20"),
                    "ptbush11" => Some("PTBush21"),
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            _ => None,
        }
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
        let remapped = Self::remap_known_model_alias(model_name);
        if Self::is_model_asset_available(&remapped) {
            return Some(remapped);
        }

        let requested_key = Self::normalize_model_lookup_key(&remapped);
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

    pub(in super::super) fn build_fallback_template(template_name: &str) -> ThingTemplate {
        let lower = template_name.to_ascii_lowercase();
        let mut template = ThingTemplate::new(template_name);
        template.set_health(250.0);
        let fallback_model_name = Self::resolve_spawn_model_name(template_name)
            .unwrap_or_else(|| Self::remap_known_model_alias(template_name));
        template.set_model(&fallback_model_name);

        if let Some(manager_arc) = get_asset_manager() {
            if let Ok(manager) = manager_arc.lock() {
                let remapped_model = Self::remap_known_model_alias(template_name);
                if let Some(texture_name) = manager
                    .get_texture_for_object(template_name)
                    .or_else(|| manager.get_texture_for_object(remapped_model.as_str()))
                {
                    if !texture_name.is_empty() && !texture_name.eq_ignore_ascii_case("none") {
                        template.texture_name = Some(texture_name);
                    }
                }
            }
        }

        let is_resource = lower.contains("supplypile")
            || lower.contains("supplydock")
            || lower.contains("tempsupplydock")
            || lower.contains("crate");
        let is_structure = Self::should_spawn_fallback_template(template_name) && !is_resource;

        if is_resource {
            template
                .add_kind_of(KindOf::Resource)
                .add_kind_of(KindOf::Harvestable);
        } else if is_structure {
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
            && !is_resource
            && (lower.contains("supplycenter")
                || lower.contains("supplystash")
                || lower.contains("supplydropzone")
                || lower == "supplycenter")
        {
            template.add_kind_of(KindOf::SupplyCenter);
        }

        template
    }

    pub(in super::super) fn build_visual_fallback_template(template_name: &str) -> Option<ThingTemplate> {
        let template = Self::build_fallback_template(template_name);
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
            .set_model("airanger_s") // USA Ranger infantry model
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
            .set_model("aimissletm") // USA Missile Defender
            .set_primary_weapon_name(super::super::weapon_bootstrap::MISSILE_DEFENDER_MISSILE_WEAPON)
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
            .set_model("avcrusader") // USA Crusader tank
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
            .set_model("avcrusader") // USA Paladin tank (using Crusader model since avpaldin doesn't exist)
            .set_primary_weapon_name(super::super::weapon_bootstrap::PALADIN_TANK_GUN)
            .set_locomotor_name(super::super::locomotor_bootstrap::CRUSADER_LOCOMOTOR);
        self.templates
            .insert("USA_PaladinTank".to_string(), usa_paladin);

        // USA Aircraft
        let mut usa_raptor = ThingTemplate::new("USA_Raptor");
        usa_raptor
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(180.0)
            .set_cost(1000, 0)
            .set_model("avraptorag") // USA F-22 Raptor
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
            .set_model("uirebel") // GLA Rebel infantry model
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
            .set_model("uirguard02") // GLA RPG Trooper (using guard model since uirpgtrp doesn't exist)
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
            .set_model("uvtechvan_d1") // GLA Technical vehicle model
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
            .set_model("uvscorpion") // GLA Scorpion tank
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
            .set_model("uvlitetank") // GLA Marauder tank (using lite tank model since uvmarudr doesn't exist)
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
            .set_model("uirebel") // China Red Guard (using rebel model since ciredgrd doesn't exist)
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
            .set_model("uirguard02") // China Tank Hunter (using guard model since citankht doesn't exist)
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
            .set_model("uvscorpion") // China Battlemaster tank (using scorpion model since cvbtlmst doesn't exist)
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
            .set_model("nvovrlrdt") // China Overlord tank (using correct nv pattern model)
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
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_cost(900, 0)
            .set_model("nvmign"); // China MiG (using correct nv pattern model)
        self.templates.insert("China_MiG".to_string(), china_mig);

        let mut china_helix = ThingTemplate::new("China_Helix");
        china_helix
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_cost(1200, 0)
            .set_model("avhummer"); // China Helix helicopter (using humvee model since cahelix doesn't exist)
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
                GameLogic::remap_known_model_alias(pristine),
                pristine,
                "{pristine} must not be remapped to a different retail state"
            );
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
}

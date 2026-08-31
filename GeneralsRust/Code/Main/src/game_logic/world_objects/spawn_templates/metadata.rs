//! Authored Object INI behavior metadata applied to host templates.
use super::*;

impl GameLogic {
    /// Apply the gameplay-relevant Object INI KindOf capabilities which are
    /// safe to enrich on an existing hand-authored host template.  Starter
    /// templates retain their bespoke Rust behavior, but they must not erase
    /// exact C++ targetability, collector, or projectile identity from the
    /// retail definition that describes that same object.
    pub(super) fn apply_authored_semantic_kind_bits(template: &mut ThingTemplate, kind_of: &str) {
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
        // C++ KINDOF_BLAST_CRATER — permanent pathfind crater footprints.
        if has_kind("blast_crater") {
            template.add_kind_of(KindOf::BlastCrater);
        }
        if has_kind("huge_vehicle") {
            template.add_kind_of(KindOf::HugeVehicle);
        }
    }

    /// Preserve the exact DockUpdate and normal-containment slice that the
    /// physical RMB path needs from Object INI Behavior declarations.  C++
    /// `ActionManager::canEnterObject` asks for a `ContainModuleInterface`,
    /// so a Vehicle KindOf or a template basename must never fabricate one.
    pub(super) fn apply_authored_dock_and_contain_modules(
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

        /// Decode leftover `AllowInsideKindOf` / `ForbidInsideKindOf` KindOf
        /// masks.  Infantry / Vehicle / Aircraft drive the coarse admission
        /// enum.  Other leftover-known bits (HUGE_VEHICLE, STRUCTURE, …) stay
        /// on the module mask and must not fail-close Enter.
        fn leftover_kind_token_known(token: &str) -> bool {
            game_engine::common::system::kind_of::KindOfMask::from_string(token).is_some()
        }

        /// C++ GarrisonContain / TransportContain / MobNexusContain ctors
        /// (and leftover Defaults) set `m_allowInsideKindOf = KINDOF_INFANTRY`.
        /// HelixContain inherits the TransportContain module-data ctor.
        /// Plain OpenContain still defaults allow-everything.
        fn leftover_unauthored_allow_inside_is_infantry(class_name: &str) -> bool {
            class_name.eq_ignore_ascii_case("GarrisonContain")
                || class_name.eq_ignore_ascii_case("TransportContain")
                || class_name.eq_ignore_ascii_case("HelixContain")
                || class_name.eq_ignore_ascii_case("MobNexusContain")
        }

        fn parse_leftover_kind_of_mask(raw: Option<&str>) -> u128 {
            let Some(raw) = raw else {
                return 0;
            };
            use game_engine::common::system::kind_of::KindOfMask;
            KindOfMask::parse_ini(KindOfMask::empty(), raw)
                .map(|mask| mask.bits())
                .unwrap_or(0)
        }

        fn parse_leftover_allow_inside_kind_of(
            module: &crate::assets::BehaviorModuleDefinition,
        ) -> u128 {
            match module.attribute("AllowInsideKindOf") {
                Some(raw) => parse_leftover_kind_of_mask(Some(raw)),
                None if leftover_unauthored_allow_inside_is_infantry(&module.class_name) => {
                    game_engine::common::system::kind_of::KindOfMask::INFANTRY.bits()
                }
                None => 0,
            }
        }

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
            // Un-authored AllowInsideKindOf: leftover/C++ Infantry-only for
            // Garrison/Transport/MobNexus. OpenContain stays AnyMobile.
            let mut allowed = if leftover_unauthored_allow_inside_is_infantry(&module.class_name) {
                [true, false, false]
            } else {
                [true, true, true] // infantry, vehicle, aircraft
            };
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
                        "ALL" => allowed = [true, true, true],
                        other if leftover_kind_token_known(other) => {}
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
                        other if leftover_kind_token_known(other) => {}
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
        /// complete record are available.  A row is live when it resolves as
        /// a complete authored Locomotor set (every member live, disjoint
        /// surfaces); C++ picks the member by terrain surface at use time.
        fn parse_rider_change(
            module: &crate::assets::BehaviorModuleDefinition,
            definition: &ObjectDefinition,
            rider_change_normal_locomotors: Option<&[String]>,
        ) -> (Vec<RiderChangeRiderMetadata>, Option<u32>, String, u128) {
            let mut riders = Vec::new();
            let normal_locomotor_binding = rider_change_normal_locomotors.and_then(
                crate::game_logic::locomotor_bootstrap::resolve_complete_host_locomotor_set,
            );
            let sluggish_names = unambiguous_locomotors_for_set(definition, "SET_SLUGGISH");
            let sluggish_locomotor_binding = sluggish_names.as_deref().and_then(
                crate::game_logic::locomotor_bootstrap::resolve_complete_host_locomotor_set,
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
                Some((ContainModuleKind::Heal, parse_slots(module, "ContainMax")))
            } else if module.class_name.eq_ignore_ascii_case("CaveContain") {
                Some((ContainModuleKind::Cave, parse_slots(module, "ContainMax")))
            } else if module.class_name.eq_ignore_ascii_case("TunnelContain") {
                Some((ContainModuleKind::Tunnel, parse_slots(module, "ContainMax")))
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
                let (heal_objects, initial_roster_template, initial_roster_count) = if kind
                    == ContainModuleKind::Garrison
                {
                    let heal_objects = module
                        .attribute("HealObjects")
                        .and_then(parse_bool)
                        .unwrap_or(false);
                    let roster = module
                        .attribute("InitialRoster")
                        .map(|raw| raw.split_whitespace().collect::<Vec<_>>())
                        .and_then(|tokens| {
                            gamelogic::object::contain::InitialRoster::parse_from_tokens(&tokens)
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
                    allow_inside_kind_of: parse_leftover_allow_inside_kind_of(module),
                    forbid_inside_kind_of: parse_leftover_kind_of_mask(
                        module.attribute("ForbidInsideKindOf"),
                    ),
                    keep_container_velocity_on_exit: module
                        .attribute("KeepContainerVelocityOnExit")
                        .and_then(parse_bool)
                        .unwrap_or(false),
                    door_open_time: module
                        .attribute("DoorOpenTime")
                        .and_then(parse_duration_frames)
                        .unwrap_or(1),
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
    pub(super) fn apply_authored_parking_place_metadata(
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
    pub(super) fn apply_authored_flight_deck_metadata(
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

        let Some(module) = definition
            .behavior_modules
            .iter()
            .find(|module| module.class_name.eq_ignore_ascii_case("FlightDeckBehavior"))
        else {
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
    pub(super) fn apply_authored_deploy_style_metadata(
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
    pub(super) fn apply_authored_auto_acquire_metadata(
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
            if module.attribute("ForbidPlayerCommands").is_some_and(|v| {
                matches!(v.trim().to_ascii_lowercase().as_str(), "yes" | "true" | "1")
            }) {
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
    pub(super) fn apply_authored_supply_truck_metadata(
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
    pub(super) fn apply_authored_production_exit_metadata(
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
                    grant_temporary_stealth_frames: match module.attribute("GrantTemporaryStealth")
                    {
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
    pub(super) fn apply_authored_pilot_veterancy_metadata(
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
    pub(super) fn apply_authored_veterancy_gain_create(
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
    pub(super) fn apply_authored_create_modules(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
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
        template.has_supply_warehouse_create = definition.behavior_modules.iter().any(|module| {
            module
                .class_name
                .eq_ignore_ascii_case("SupplyWarehouseCreate")
        });
    }

    /// Retain one C++ `EjectPilotDieModuleData` declaration as typed Object
    /// metadata.  `getEjectPilotDieInterface()` is a module-presence query,
    /// so even a custom/unrepresentable module remains visible to the
    /// Hijacker path.  The death path, however, must not manufacture an OCL
    /// result from data it cannot execute exactly.
    pub(super) fn apply_authored_eject_pilot_die_metadata(
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
    pub(super) fn apply_authored_rebuild_hole_expose_die_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        fn stripped_value(value: &str) -> &str {
            value.split(';').next().unwrap_or_default().trim()
        }

        fn parse_bool(value: &str) -> Option<bool> {
            match stripped_value(value) {
                value
                    if value.eq_ignore_ascii_case("yes") || value.eq_ignore_ascii_case("true") =>
                {
                    Some(true)
                }
                value
                    if value.eq_ignore_ascii_case("no") || value.eq_ignore_ascii_case("false") =>
                {
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
    pub(super) fn apply_authored_hack_internet_metadata(
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
    pub(super) fn apply_authored_special_power_module_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::command_system::special_power_type_from_template_name;
        use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
        use game_engine::common::ini::ini_science::{ScienceType, get_science_store};
        use game_engine::common::rts::special_power::{SCIENCE_INVALID, get_special_power_store};

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
    pub(super) fn apply_authored_hacker_disable_building_metadata(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        use crate::command_system::{
            SpecialPowerType as HostSpecialPowerType, special_power_type_from_template_name,
        };
        use crate::game_logic::HackerDisableBuildingMetadata;
        use game_engine::common::ini::ini_science::{ScienceType, get_science_store};
        use game_engine::common::rts::special_power::{
            SCIENCE_INVALID, SpecialPowerType, get_special_power_store,
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
    pub(super) fn apply_authored_charge_plant_metadata(
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

    /// C++ `SpecialAbilityUpdateModuleData::m_triggerSound` (INI `TriggerSound`).
    /// Retail Lotus steal/disable author `BlackLotusTrigger`.
    pub(super) fn apply_authored_leftover_sa_trigger_sound(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
        template.leftover_sa_trigger_sound = None;
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
                .filter(|name| !name.is_empty())
            else {
                continue;
            };
            let lower = power.to_ascii_lowercase();
            if !(lower.contains("stealcash") || lower.contains("disablevehicle")) {
                continue;
            }
            if let Some(sound) = module
                .attribute("TriggerSound")
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                template.leftover_sa_trigger_sound = Some(sound.to_string());
            }
        }
    }

    /// Retain the exact StealthUpdate friendly opacity bounds used by
    /// C++ `StealthUpdate::getFriendlyOpacity`. Missing fields retain the
    /// module-data defaults; malformed present values fail closed to those
    /// defaults rather than inventing a non-finite presentation value.
    pub(super) fn apply_authored_stealth_update_metadata(
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
    pub(super) fn apply_authored_overcharge_metadata(
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
    pub(super) fn apply_authored_power_plant_update_metadata(
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
    pub(super) fn apply_authored_temporary_weapon_behavior_metadata(
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
    pub(super) fn apply_authored_physics_behavior_metadata(
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
        if let Some(factor) = module.attribute("PitchRollYawFactor").and_then(parse_real) {
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
        // Leftover parseHeightToSpeed: min_fall = sqrt(|2 * leftover_gravity * h|).
        // Default height 40 → ~2.385 at retail Gravity -64/900, not sqrt(80).
        if let Some(speed) = leftover_physics_min_fall_speed_for_damage(&template.name)
            .or_else(|| leftover_physics_min_fall_speed_for_damage(&definition.name))
            .filter(|v| leftover_min_fall_is_gravity_aware(*v))
        {
            template.min_fall_speed_for_damage = speed;
        } else if let Some(h) = module
            .attribute("MinFallHeightForDamage")
            .and_then(parse_real)
        {
            template.min_fall_speed_for_damage = Object::height_to_fall_speed(h);
        } else {
            template.min_fall_speed_for_damage = Object::min_fall_speed_for_damage();
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
    pub(super) fn apply_authored_geometry(
        template: &mut ThingTemplate,
        definition: &ObjectDefinition,
    ) {
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
        if let Some(v) = Self::object_definition_attr(definition, "geometryissmall")
            .as_deref()
            .and_then(parse_bool)
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
    pub(super) fn apply_authored_weapon_set_create_policy(
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
        template.has_fire_ocl_after_weapon_cooldown =
            definition.behavior_modules.iter().any(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("FireOCLAfterWeaponCooldownUpdate")
            });
    }

    /// Retain the small exact data slice consumed by C++
    /// `ActionManager::canCaptureBuilding`: source capture SpecialPower,
    /// target CAPTURABLE/IMMUNE_TO_CAPTURE flags, and GarrisonContain state.
    ///
    /// The parser keeps fields per Behavior block, so a `SpecialPowerTemplate`
    /// from an unrelated module cannot accidentally enable capture.
    pub(super) fn apply_authored_capture_metadata(
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
                template.capture_trigger_sound = None;

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
        template.capture_trigger_sound = None;

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
                template.capture_trigger_sound = module
                    .attribute("TriggerSound")
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
}

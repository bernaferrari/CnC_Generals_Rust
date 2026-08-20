//! Host objects `impl GameLogic` — `create_destroy_die`.
//! create/destroy, mark_object_for_destruction, die, dam. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// C++ `SupplyCenterProductionExitUpdate::exitObjectViaDoor` finishes the
    /// ordinary authored exit path before asking `SupplyTruckAIUpdate` to
    /// enter Wanting.  The compact host keeps that path on the unit and only
    /// changes its post-exit AI destination here; non-harvester outputs never
    /// gain this supply-center-specific behavior.
    pub(crate) fn force_supply_center_collector_wanting(
        &mut self,
        collector_id: ObjectId,
        center_id: ObjectId,
    ) -> bool {
        let Some((collector_owner, collector_team, collector_pos, is_harvester, scan_distance)) =
            self.host_object(collector_id).map(|collector| {
                (
                    self.player_owner_for_host_object(collector),
                    collector.team,
                    collector.get_position(),
                    collector.is_kind_of(KindOf::Harvester),
                    collector
                        .thing
                        .template
                        .supply_truck_metadata
                        .map(|metadata| metadata.warehouse_scan_distance),
                )
            })
        else {
            return false;
        };
        let legal_center = self.host_object(center_id).is_some_and(|center| {
            center.is_alive()
                && center.is_constructed()
                && (center.is_kind_of(KindOf::SupplyCenter)
                    || center.is_kind_of(KindOf::FSSupplyCenter))
                && self.player_owner_for_host_object(center) == collector_owner
        });
        if !is_harvester || !legal_center {
            return false;
        }
        let Some(source_id) = self.find_nearest_harvestable_supply(collector_team, collector_pos)
        else {
            return false;
        };
        if scan_distance.is_some_and(|distance| {
            distance > 0.0
                && self
                    .host_object(source_id)
                    .is_some_and(|source| source.get_position().distance(collector_pos) > distance)
        }) {
            return false;
        }
        if let Some(collector) = self.host_object_mut(collector_id) {
            // `m_preferredDock` is set by the supply-center/AI handoff, and
            // stays distinct from merely sharing a faction team.
            collector.preferred_dock_id = Some(center_id);
            collector.set_target(Some(source_id));
            // Keep a concrete one-shot latch/state mirror alongside the
            // host AIState.  The latter still owns path following; the former
            // makes the supply dock cadence observable and saveable.
            collector.supply_truck_force_pending = true;
            collector.supply_truck_state = SupplyTruckState::Wanting;
            collector.supply_truck_next_dock_action_frame = 0;
            // Do not replace `movement.path`: it already contains the
            // producer's natural/custom exit route.  Once it completes, the
            // normal Gathering update proceeds to this acquired warehouse.
            collector.set_ai_state(AIState::Gathering);
        }
        true
    }

    /// Resolve the one-shot `SpawnBehavior` payload authored by a SupplyCenter
    /// or SupplyStash.  The live Object INI catalog preserves the nested
    /// `SpawnTemplateName` fields, so general-specific centers keep their
    /// own collector (for example Air Force Chinooks) instead of being
    /// rewritten to a base-faction name.
    ///
    /// The exact three base-game pairs are retained for headless/test worlds
    /// that intentionally do not initialize an AssetManager.  This is not a
    /// name-derived fallback: these are the literal `SpawnBehavior ModuleTag_12`
    /// entries in FactionBuilding.ini.
    fn authored_supply_center_one_shot_template(template_name: &str) -> Option<String> {
        let parsed = get_asset_manager().and_then(|manager| {
            let manager = manager.lock().ok()?;
            let definition = manager.resolve_object_definition(template_name, None)?;
            let one_shot = Self::object_definition_attr(definition, "OneShot")
                .is_some_and(|value| value.eq_ignore_ascii_case("yes"));
            let spawn_count = Self::object_definition_attr(definition, "SpawnNumber")
                .and_then(|value| value.trim().parse::<u32>().ok());
            let spawn_template = Self::object_definition_attr(definition, "SpawnTemplateName")?;
            (one_shot && spawn_count == Some(1))
                .then_some(spawn_template.trim().to_string())
                .filter(|name| !name.is_empty())
        });

        parsed.or_else(|| match template_name {
            "AmericaSupplyCenter" => Some("AmericaVehicleChinook".to_string()),
            "ChinaSupplyCenter" => Some("ChinaVehicleSupplyTruck".to_string()),
            "GLASupplyStash" => Some("GLAInfantryWorker".to_string()),
            _ => None,
        })
    }

    /// Return the authored starter-collector template for a concrete supply
    /// center.  The AI uses this to put later paid collectors through the same
    /// typed producer path as the original C++ `queueSupplyTruck` code.
    pub(crate) fn supply_center_one_shot_collector_template(
        &self,
        center_id: ObjectId,
    ) -> Option<String> {
        let center = self.host_object(center_id)?;
        if !center.is_kind_of(KindOf::SupplyCenter) && !center.is_kind_of(KindOf::FSSupplyCenter) {
            return None;
        }
        Self::authored_supply_center_one_shot_template(&center.template_name)
    }

    /// C++ `SpawnBehavior::createSpawn` slice for the SupplyCenter/Stash
    /// starter collector.  This is deliberately separate from ProductionUpdate:
    /// retail creates the first collector as a one-shot SpawnBehavior payload,
    /// with the center as its producer, before AI later pays for replacements.
    pub(crate) fn spawn_supply_center_one_shot_collector(
        &mut self,
        center_id: ObjectId,
    ) -> Option<ObjectId> {
        let (team, owner_player_id, position, orientation) = {
            let center = self.host_object(center_id)?;
            if !center.is_alive()
                || !center.is_constructed()
                || center.status.under_construction
                || center.status.sold
                || center.status.reconstructing
                || center.supply_center_spawn_behavior_fired
                || center.team == Team::Neutral
                || (!center.is_kind_of(KindOf::SupplyCenter)
                    && !center.is_kind_of(KindOf::FSSupplyCenter))
            {
                return None;
            }
            (
                center.team,
                center.owner_player_id,
                center.get_position(),
                center.get_orientation(),
            )
        };

        let spawn_template = self.supply_center_one_shot_collector_template(center_id)?;

        // Do not let create_object synthesize a visual/name fallback for a
        // gameplay collector.  A loaded Object INI definition is sufficient to
        // make a typed template here; otherwise the authored behavior simply
        // waits for a valid template rather than inventing a unit.
        if !self.templates.contains_key(&spawn_template) {
            let template = Self::build_template_from_asset_definition(&spawn_template)?;
            if !template.is_kind_of(KindOf::Harvester) {
                return None;
            }
            self.templates.insert(spawn_template.clone(), template);
        }
        if !self
            .templates
            .get(&spawn_template)
            .is_some_and(|template| template.is_kind_of(KindOf::Harvester))
        {
            return None;
        }

        // Both activation entry points are mutually exclusive in normal play,
        // but retain an object-level guard so duplicate completion writeback
        // cannot turn C++ OneShot into a second free collector.
        if self.host_objects().values().any(|object| {
            object.producer_id == Some(center_id)
                && object.template_name.eq_ignore_ascii_case(&spawn_template)
        }) {
            // Old snapshots did not carry the explicit one-shot bit. A live
            // authored child is conclusive evidence this behavior already
            // fired, so repair the state before declining a duplicate.
            if let Some(center) = self.host_object_mut(center_id) {
                center.supply_center_spawn_behavior_fired = true;
            }
            return None;
        }

        // The real module's UnitCreatePoint is parsed below when the object
        // catalog is available.  Preserve producer identity here; the common
        // exit handoff applies its authored route immediately after creation.
        let spawned_id = match owner_player_id {
            Some(player_id) => self.create_object_for_player(&spawn_template, player_id, position),
            None => self.create_object(&spawn_template, team, position),
        }?;
        if let Some(spawned) = self.host_object_mut(spawned_id) {
            spawned.producer_id = Some(center_id);
            spawned.set_orientation(orientation);
        }
        let (natural, has_supply_exit) = self
            .host_object(center_id)
            .map(|center| {
                let forward = center.thing.get_direction_vector();
                let metadata = center.thing.template.production_exit_metadata;
                let natural = metadata
                    .map(|exit| {
                        let point = exit.natural_rally_point_with_path_offset(
                            crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F,
                        );
                        crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                            center.get_position(), forward, (point[0], point[1], point[2]),
                        )
                    })
                    .unwrap_or(center.get_position());
                (natural, metadata.is_some_and(|exit| exit.is_supply_center()))
            })
            .unwrap_or((position, false));
        self.path_approach_with_state(spawned_id, natural, AIState::Moving);
        if has_supply_exit {
            let _ = self.force_supply_center_collector_wanting(spawned_id, center_id);
        }
        if let Some(center) = self.host_object_mut(center_id) {
            center.supply_center_spawn_behavior_fired = true;
        }
        Some(spawned_id)
    }

    /// Create a new object for an unambiguous faction owner.  Callers that
    /// know the controlling player (map records, commands, and producers)
    /// must use `create_object_for_player` instead.
    pub fn create_object(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        let owner_player_id = self.unique_player_id_for_team(team);
        self.create_object_with_owner(template_name, team, owner_player_id, position)
    }

    /// Create an object for a specific controlling player while retaining the
    /// player's faction in `team` for INI/template and visual selection.
    pub fn create_object_for_player(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
    ) -> Option<ObjectId> {
        let team = self.players.get(&owner_player_id)?.team;
        if team == Team::Neutral {
            return None;
        }
        self.create_object_with_owner(template_name, team, Some(owner_player_id), position)
    }

    /// Create through a player-aware path when a caller has exact provenance,
    /// otherwise retain the legacy faction-only creation behavior.  This is a
    /// small boundary helper for systems whose old payloads carried `Team`
    /// while newer producer/map paths also carry the controlling player.
    pub(in super::super) fn create_object_for_owner_or_team(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
    ) -> Option<ObjectId> {
        match owner_player_id {
            Some(player_id) => self.create_object_for_player(template_name, player_id, position),
            None => self.create_object(template_name, team, position),
        }
    }
    /// Bind `template_name` to an already-loaded host ThingTemplate.
    ///
    /// C++ `ThingFactory` is filled once in `GameEngine::init`. The host catalog
    /// (`self.templates` plus AssetManager object definitions loaded at boot)
    /// is the equivalent already-loaded set. Never call
    /// `TheThingFactory::find_template` here: that helper lazy-inits every
    /// Object INI (14s+ on Lone Eagle).
    fn ensure_host_spawn_template(&mut self, template_name: &str) -> bool {
        if self.templates.contains_key(template_name) {
            return true;
        }
        if let Some(canonical) = self
            .templates
            .keys()
            .find(|name| name.eq_ignore_ascii_case(template_name))
            .cloned()
        {
            if let Some(template) = self.templates.get(&canonical).cloned() {
                self.templates.insert(template_name.to_string(), template);
                return true;
            }
        }
        if self.unresolved_spawn_templates.contains(template_name) {
            return false;
        }

        let mut injected = false;
        let should_spawn_fallback = Self::should_spawn_fallback_template(template_name);

        if let Some(template) = Self::build_template_from_asset_definition(template_name) {
            let missing_model = template
                .model_name
                .as_deref()
                .filter(|model| !Self::is_model_asset_available(model))
                .map(|model| model.to_string());

            if missing_model.is_none() || should_spawn_fallback {
                self.templates.insert(template_name.to_string(), template);
                injected = true;
                log::debug!(
                    "Synthesized template for '{}' from WW3D object definitions",
                    template_name
                );
            } else if let Some(model) = missing_model {
                log::debug!(
                    "Falling back for decorative map object template '{}' after unavailable definition model '{}'",
                    template_name,
                    model
                );
            }
        }

        if !injected {
            if let Some(fallback_template) = Self::build_visual_fallback_template(template_name) {
                let model_name = fallback_template
                    .model_name
                    .clone()
                    .unwrap_or_else(|| template_name.to_string());
                self.templates
                    .insert(template_name.to_string(), fallback_template);
                if should_spawn_fallback {
                    log::warn!(
                        "Injected fallback template for unresolved object '{}' using model '{}'",
                        template_name,
                        model_name
                    );
                } else {
                    log::debug!(
                        "Injected visual-only fallback template for decorative object '{}' using model '{}'",
                        template_name,
                        model_name
                    );
                }
                injected = true;
            }
        }

        if injected {
            self.unresolved_spawn_templates.remove(template_name);
            true
        } else {
            self.unresolved_spawn_templates
                .insert(template_name.to_string());
            false
        }
    }


    fn create_object_with_owner(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
    ) -> Option<ObjectId> {
        if owner_player_id.is_some_and(|player_id| {
            self.players.get(&player_id).map(|player| player.team) != Some(team)
        }) {
            return None;
        }
        // Map-load skip list: decorative / overloaded templates (AngryMob nexus
        // projectiles, cinematic shells, …). Intentional residual / test spawns
        // that already registered a template are fail-open (host Angry Mob path).
        if Self::should_skip_map_object_template(template_name)
            && !self
                .templates
                .keys()
                .any(|name| name.eq_ignore_ascii_case(template_name))
        {
            return None;
        }

        if !self.ensure_host_spawn_template(template_name) {
            // Do not invent a proxy for an unresolved gameplay or map
            // object.  A visible but wrong faction/condition mesh is
            // less faithful than an explicit unsupported-object miss.
            if Self::should_spawn_fallback_template(template_name) {
                log::warn!(
                    "Skipping unresolved object '{}' because no exact retail W3D is available",
                    template_name
                );
            } else {
                log::debug!(
                    "Skipping unsupported decorative map object template '{}'",
                    template_name
                );
            }
            return None;
        }

        if let Some(template) = self.templates.get(template_name).cloned() {
            // C++ Object.cpp asks the controlling Player for the exact
            // `ProductionVeterancyLevel` while constructing every Object.
            // Resolve it before moving the ThingTemplate into Object so a
            // selected General's authored per-unit rank is independent of the
            // shared base Team.
            let player_template_veterancy = owner_player_id.and_then(|player_id| {
                self.player_template_production_veterancy(player_id, template_name)
            });
            let is_structure = template.is_kind_of(KindOf::Structure);
            let counts_as_unit = Self::template_counts_as_unit(&template);
            let id = self.allocate_object_id();
            // Resolve weapons / locomotor before move into Object.
            let weapon = template.resolve_primary_weapon();
            let mine_clearing_primary_weapon = template.resolve_mine_clearing_primary_weapon();
            let secondary_weapon = template.resolve_secondary_weapon();
            let tertiary_weapon = template.resolve_tertiary_weapon();
            let movement_stats = template.resolve_movement();
            let loco_binding = template
                .locomotor_name
                .as_deref()
                .and_then(crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding);
            // Sentry residual: detect explicit template primary before move.
            let sentry_had_explicit_primary =
                template.primary_weapon.is_some() || template.primary_weapon_name.is_some();
            // C++ Object.cpp:160-497 / onObjectCreated: create policy comes
            // from ThingTemplate INI (AutoChooseSources, FireOCLAfterWeaponCooldown).
            // Name residuals below are missing-INI fallbacks only.
            let primary_auto_choose_none = template.primary_auto_choose_none;
            let has_fire_ocl_after_weapon_cooldown =
                template.has_fire_ocl_after_weapon_cooldown;
            let partition_cash = template.build_cost.supplies;
            let mut object = Object::new_with_logic_frame(template, id, team, self.frame);
            object.owner_player_id = owner_player_id;
            object.partition_cash_value = partition_cash;
            object.partition_threat_value = partition_cash.max(1);
            object.set_position(position);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    id,
                    Some([position.x, position.y, position.z]),
                );
                object.record_host_movement();
            }
            let starts_under_construction = object.status.under_construction;

            // Primary weapon from template when defined; kind-based fallback only as last resort.
            if let Some(weapon) = weapon {
                object.weapon = Some(weapon);
            }
            if let Some(mine_clearing_primary_weapon) = mine_clearing_primary_weapon {
                object.mine_clearing_primary_weapon = Some(mine_clearing_primary_weapon);
            }
            // Secondary slot: fail-closed (only when template names/stats resolve).
            if let Some(secondary) = secondary_weapon {
                object.secondary_weapon = Some(secondary);
            }
            // Tertiary is a separate WeaponSet storage slot.  Conditional
            // templates may strip/rebind it below (for example Comanche pods),
            // but it must never overwrite SECONDARY on creation.
            if let Some(tertiary) = tertiary_weapon {
                object.tertiary_weapon = Some(tertiary);
            }

            // Strategy Center: AutoChooseSources=PRIMARY NONE is on the
            // template when Object INI loaded (C++ FactionBuilding.ini:6970).
            // resolve_primary_weapon already refuses kind-based Weapon::default.
            // Missing-INI fallback: hand-built FS_STRATEGY_CENTER fixtures
            // still strip the kind-default so Bombardment can re-equip.
            if !primary_auto_choose_none
                && (crate::game_logic::host_strategy_center::is_strategy_center_template(
                    template_name,
                ) || object.is_kind_of(KindOf::FSStrategyCenter))
            {
                use crate::game_logic::host_strategy_center::STRATEGY_CENTER_GUN_DAMAGE;
                let is_gun = object.weapon.as_ref().is_some_and(|w| {
                    (w.damage - STRATEGY_CENTER_GUN_DAMAGE).abs() < 0.001
                        && (w.range - 400.0).abs() < 0.001
                });
                if !is_gun {
                    object.weapon = None;
                    object.secondary_weapon = None;
                }
            }

            // Quad Cannon: Weapon.ini AntiGround/AntiAirborne already flow
            // through weapon_from_store. Force masks only when the store
            // left C++ defaults (missing-INI / unparsed anti-mask).
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                let store_missed_masks = object
                    .weapon
                    .as_ref()
                    .is_some_and(|w| w.can_target_air)
                    || object
                        .secondary_weapon
                        .as_ref()
                        .is_some_and(|w| w.can_target_ground);
                if store_missed_masks {
                    if let Some(w) = object.weapon.as_mut() {
                        w.can_target_ground = true;
                        w.can_target_air = false;
                    }
                    if let Some(w) = object.secondary_weapon.as_mut() {
                        w.can_target_air = true;
                        w.can_target_ground = false;
                    }
                }
            }

            // FireOCLAfterWeaponCooldownUpdate: crate module init from the
            // authored Behavior. Name residual is missing-INI fallback for
            // hand-built GLAVehicleToxinTruck fixtures (C++ GLAVehicle.ini:3697).
            if has_fire_ocl_after_weapon_cooldown
                || crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(template_name)
            {
                object.fire_ocl_after_cooldown = Some(
                    crate::game_logic::host_toxin_tractor::HostFireOclAfterCooldownData::new(),
                );
                // Retail ToxinTruckSprayer PrimaryDamage=0 fails
                // weapon_from_store; host seed uses 0.001. Missing-INI only.
                if object.secondary_weapon.is_none()
                    && crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(
                        template_name,
                    )
                {
                    use crate::game_logic::host_toxin_tractor::{
                        delay_frames_to_reload_secs, TOXIN_SPRAY_DELAY_FRAMES, TOXIN_SPRAY_RANGE,
                    };
                    object.secondary_weapon = Some(Weapon {
                        damage: 0.001,
                        range: TOXIN_SPRAY_RANGE,
                        min_range: 0.0,
                        reload_time: delay_frames_to_reload_secs(TOXIN_SPRAY_DELAY_FRAMES),
                        last_fire_time: 0.0,
                        ammo: None,
                        clip_size: 0,
                        clip_reload_time: 0.0,
                        can_target_air: false,
                        can_target_ground: true,
                        projectile_speed: 600.0,
                        pre_attack_delay: 0.0,
                        splash_radius: 0.0,
                        suspend_fx_frame: 0,
                    });
                }
            }

            // Locomotor catalog → host Movement + braking/wander/damaged.
            // Fail-closed: only when template sets locomotor_name and store resolves.
            if let Some(binding) = loco_binding {
                crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(
                    &mut object,
                    &binding,
                );
            } else if let Some(stats) = movement_stats {
                object.movement.max_speed = stats.max_speed;
                object.movement.acceleration = stats.acceleration;
                object.movement.turn_rate = stats.turn_rate;
            }

            // Host residual: bind mine/demo-trap data for recognized templates.
            if let Some(mine_data) =
                crate::game_logic::host_mines::residual_data_for_template(template_name, self.frame)
            {
                object.mine_data = Some(mine_data);
                object.record_host_demo_mine_cheer();
            }

            // Host residual: GLA Battle Bus TransportContain Slots=8 + passenger fire.
            if crate::game_logic::host_battle_bus::is_battle_bus_template(template_name) {
                object.install_battle_bus_transport();
            }
            if crate::game_logic::host_highlander_body::is_highlander_body_template(template_name) {
                object.install_highlander_body();
            }
            object.install_deploy_style_if_needed();
            object.install_tensile_formation_if_needed();
            if object.has_tensile_formation() {
                self.tensile_formation_reg.record_install();
            }
            object.install_fire_spread_if_needed();
            if object.has_fire_spread() {
                self.fire_spread_reg.record_install();
            }
            object.install_base_regenerate_if_needed();
            if object.base_regenerate.is_some() {
                self.base_regenerate_reg.record_install();
            }
            object.install_enemy_near_if_needed();
            if object.enemy_near.is_some() {
                self.enemy_near_reg.record_install();
            }
            object.install_animation_steering_if_needed();
            if object.animation_steering.is_some() {
                self.animation_steering_reg.record_install();
            }
            object.install_float_update_if_needed();
            if object.float_update.is_some() {
                self.float_update_reg.record_install();
            }
            object.install_prone_update_if_needed();
            if object.prone_update.is_some() {
                self.prone_update_reg.record_install();
            }
            object.install_radius_decal_update_if_needed();
            if object.radius_decal_update.is_some() {
                self.radius_decal_update_reg.record_install();
            }
            object.install_checkpoint_update_if_needed();
            if object.checkpoint_update.is_some() {
                self.checkpoint_update_reg.record_install();
            }
            object.install_spectre_gunship_deployment_if_needed();
            if object.spectre_gunship_deployment.is_some() {
                self.spectre_gunship_deployment_reg.record_install();
            }
            object.install_smart_bomb_target_homing_if_needed();
            if object.smart_bomb_target_homing.is_some() {
                self.smart_bomb_target_homing_reg.record_install();
            }
            if let Some(up) =
                crate::game_logic::host_upgrade_die::upgrade_to_remove_for_template(template_name)
            {
                object.install_upgrade_die(up);
            }

            // Host residual: GLA Technical TransportContain Slots=5 (infantry passengers)
            // + PRIMARY TechnicalMachineGunWeapon residual (salvage tiers swap later).
            // Fail-closed: not chassis reskin / PassengersAllowedToFire.
            if crate::game_logic::host_technical::is_technical_template(template_name) {
                use crate::game_logic::host_technical::{
                    technical_weapon_for_tier, TechnicalWeaponTier,
                };
                object.install_technical_transport();
                // Force residual MG when template lacked primary_weapon_name (Weapon::default path).
                object.weapon = Some(technical_weapon_for_tier(TechnicalWeaponTier::Base));
            }

            // Host residual: China Battlemaster PRIMARY BattleMasterTankGun residual.
            // Fail-closed: Uranium/horde/nationalism applied via refresh_battlemaster_weapon.
            if crate::game_logic::host_battlemaster::is_battlemaster_template(template_name) {
                use crate::game_logic::host_battlemaster::battlemaster_weapon;
                object.weapon = Some(battlemaster_weapon(false, false, false));
            }

            // Host residual: GLA Marauder PRIMARY MarauderTankGun residual (salvage tiers).
            // Fail-closed: not full SalvageCrate W3D turret subobject matrix.
            if crate::game_logic::host_marauder::is_marauder_template(template_name) {
                use crate::game_logic::host_marauder::{
                    marauder_weapon_for_tier, MarauderWeaponTier,
                };
                object.weapon = Some(marauder_weapon_for_tier(MarauderWeaponTier::Base));
            }

            // Host residual: GLA Combat Cycle RiderChangeContain Slots=1 + rider weapon.
            // Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
            if crate::game_logic::host_combat_cycle::is_combat_cycle_template(template_name) {
                object.install_combat_cycle_transport();
                // Retail InitialPayload residual: spawn with default rider weapon bound.
                let rider = crate::game_logic::host_combat_cycle::default_spawn_rider_for_template(
                    template_name,
                );
                object.combat_cycle_rider = rider.as_u8();
                object.weapon =
                    crate::game_logic::host_combat_cycle::combat_cycle_weapon_for_rider(rider);
            }

            // Host residual: GLA Tunnel Network TunnelContain (shared MaxTunnelCapacity=10)
            // + PRIMARY TunnelNetworkGun residual (base-defense auto-fire path).
            // Fail-closed: not GuardTunnelNetwork AI / CaveSystem / heal matrix.
            if crate::game_logic::host_tunnel_network::is_tunnel_network_template(template_name) {
                object.install_tunnel_network_residual();
                object.weapon =
                    Some(crate::game_logic::host_tunnel_network::tunnel_network_gun_weapon());
            }

            // Host residual: AirF Combat Chinook TransportContain Slots=8 + passenger fire.
            // Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
            if crate::game_logic::host_combat_chinook::is_combat_chinook_template(template_name) {
                object.install_combat_chinook_transport();
            }

            // Host residual: China Listening Outpost detect 300 + transport Slots=2 +
            // InnateStealth + ArmedRiders dummy. Fail-closed: not IR FX / multi-door.
            let is_listening_outpost_spawn =
                crate::game_logic::host_listening_outpost::is_listening_outpost_template(
                    template_name,
                );
            if is_listening_outpost_spawn {
                object.install_listening_outpost_transport();
            }

            // Host residual: China Troop Crawler TransportContain Slots=8 +
            // StealthDetector (VisionRange 175) + TroopCrawlerAssault DEPLOY.
            // Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve.
            let is_troop_crawler_spawn =
                crate::game_logic::host_troop_crawler::is_troop_crawler_template(template_name);
            if is_troop_crawler_spawn {
                object.install_troop_crawler_transport();
                object.weapon =
                    Some(crate::game_logic::host_troop_crawler::troop_crawler_assault_weapon());
                if crate::game_logic::host_troop_crawler::troop_crawler_spawn_is_detector(
                    template_name,
                ) {
                    object.is_detector = true;
                    object.record_host_detector();
                    if let Some(range) =
                        crate::game_logic::host_troop_crawler::troop_crawler_detection_range(
                            template_name,
                        )
                    {
                        object.detection_range = range;
                        object.record_host_detector();
                    }
                }
                // VisionRange residual (175) for effective_detection_range fallback.
                object.thing.template.sight_range = object
                    .thing
                    .template
                    .sight_range
                    .max(crate::game_logic::host_troop_crawler::TROOP_CRAWLER_VISION_RANGE);
            }

            // Host residual: China Overlord / Helix / Emperor portable addons + transport.
            // Fail-closed: not full OverlordContain / HelixContain portable-structure spawn.
            if crate::game_logic::host_overlord_addons::is_overlord_tank_template(template_name) {
                // OverlordContain style: portable slot reserved; bunker residual separate.
                object.overlord_bunker_capacity = Some(0);
                object.record_host_overlord();
            }
            if crate::game_logic::host_overlord_addons::is_helix_template(template_name) {
                object.install_helix_transport();
                // Host residual: Helix PRIMARY HelixMinigunWeapon (always retained with addons).
                // Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
                object.weapon = Some(crate::game_logic::host_helix_minigun::helix_minigun_weapon());
            }
            if crate::game_logic::host_overlord_addons::is_emperor_template(template_name) {
                // Innate PropagandaTowerBehavior AffectsSelf residual.
                object.has_overlord_propaganda_addon = true;
                object.record_host_overlord();
                object.overlord_bunker_capacity = Some(0);
                object.record_host_overlord();
            }
            let emperor_spawn =
                crate::game_logic::host_overlord_addons::is_emperor_template(template_name);
            let helix_spawn =
                crate::game_logic::host_overlord_addons::is_helix_template(template_name);

            // Host residual: America Humvee TransportContain Slots=5 + passenger fire.
            // Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
            if crate::game_logic::host_humvee::is_humvee_template(template_name) {
                object.install_humvee_transport();
            }

            // Host residual: America Avenger designator primary + air laser secondary.
            // Fail-closed: not portable laser turret OverlordContain passenger.
            if crate::game_logic::host_avenger::is_avenger_template(template_name) {
                object.weapon = Some(crate::game_logic::host_avenger::avenger_designator_weapon());
                object.secondary_weapon =
                    Some(crate::game_logic::host_avenger::avenger_air_laser_weapon());
            }

            // Host residual: America Sentry Drone StealthDetectorUpdate (DetectionRange 225).
            // Always detector from spawn; gun is PLAYER_UPGRADE residual.
            if crate::game_logic::host_sentry_drone::sentry_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_sentry_drone::sentry_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                // Innate stealth residual (StealthUpdate InnateStealth = Yes).
                // C++ AmericaVehicleSentryDrone: FIRING_PRIMARY + MOVING uncloak,
                // StealthDelay 2000ms (60f) before re-cloak.
                object.set_status_stealthed(true);
                object.innate_stealth = true;
                object.stealth_breaks_on_attack = true;
                object.stealth_breaks_on_move = true;
                object.stealth_delay_frames =
                    crate::game_logic::host_sentry_drone::SENTRY_STEALTH_DELAY_FRAMES;
                object.stealth_allowed_frame = 0;
                object.stealth_delay_pending = false;
                object.record_host_stealth_flags();
                object.record_host_stealth_delay();
                object.record_host_stealth_flags();
                // Retail WeaponSet Conditions=None has PRIMARY None until PLAYER_UPGRADE.
                // Strip kind-based Weapon::default fallback from resolve_primary_weapon.
                // Explicit template primary_weapon(_name) still keeps a bound gun (test/seed).
                if !sentry_had_explicit_primary {
                    object.weapon = None;
                }
            }

            // Host residual: America Pathfinder StealthDetectorUpdate + InnateStealth.
            // DetectionRange unset → VisionRange 200; stays stealthed while attacking;
            // uncloaks only while MOVING (StealthForbiddenConditions = MOVING).
            if crate::game_logic::host_pathfinder::pathfinder_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_pathfinder::pathfinder_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                object.set_status_stealthed(true);
                object.innate_stealth = true;
                object.is_pathfinder_unit = true;
                object.record_host_stealth_flags();
                object.stealth_breaks_on_attack = false;
                object.record_host_stealth_flags();
                object.stealth_breaks_on_move = true;
                object.record_host_stealth_flags();
            }

            // C++ StealthUpdate InnateStealth=Yes on Burton / Kell / Lotus /
            // Saboteur / Hijacker. Pathfinder already cloaks above; heroes
            // were spawning permanently visible.
            {
                use crate::game_logic::host_colonel_burton::is_colonel_burton_template;
                use crate::game_logic::host_hero_abilities::is_black_lotus_template;
                use crate::game_logic::host_jarmen_kell::is_jarmen_kell_template;
                let n = template_name.to_ascii_lowercase();
                let is_hero_stealth = is_colonel_burton_template(template_name)
                    || is_jarmen_kell_template(template_name)
                    || is_black_lotus_template(template_name)
                    || n.contains("saboteur")
                    || n.contains("hijacker");
                if is_hero_stealth {
                    object.set_status_stealthed(true);
                    object.innate_stealth = true;
                    object.stealth_breaks_on_attack = true;
                    object.stealth_breaks_on_move = false;
                    object.record_host_stealth_flags();
                }
            }

            // Host residual: China Dragon Tank primary flame weapon bind.
            // Fail-closed: FireWall secondary is host_firewall special-power residual.
            if crate::game_logic::host_dragon_tank::is_dragon_tank_template(template_name) {
                use crate::game_logic::host_dragon_tank::{
                    dragon_flame_weapon, has_black_napalm_upgrade,
                };
                let upgraded = has_black_napalm_upgrade(&object.applied_upgrades);
                // Force residual flame stats when store/template leaves defaults.
                object.weapon = Some(dragon_flame_weapon(upgraded));
            }

            // Host residual: China Nuke Cannon neutron secondary is PLAYER_UPGRADE only.
            // Fail-closed: Upgrade_ChinaNeutronShells equips SECONDARY; without it, no secondary.
            // Explicit template.secondary_weapon_name (tests / seeds) still keeps a bound weapon.
            if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
                use crate::game_logic::host_neutron_shell::UPGRADE_CHINA_NEUTRON_SHELLS;
                use crate::game_logic::weapon_bootstrap::{
                    ensure_host_weapon_store, NUKE_CANNON_NEUTRON_WEAPON,
                };
                let has_neutron = object.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS)
                    || object.has_upgrade_tag("Upgrade_ChinaNeutronShells")
                    || self.players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_CHINA_NEUTRON_SHELLS)
                    });
                if has_neutron {
                    ensure_host_weapon_store();
                    if let Some(w) = ThingTemplate::weapon_from_store(NUKE_CANNON_NEUTRON_WEAPON) {
                        object.secondary_weapon = Some(w);
                    }
                    object.apply_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS);
                } else if object.thing.template.secondary_weapon_name.is_none()
                    && object.thing.template.secondary_weapon.is_none()
                {
                    // Strip residual map auto-equip; keep explicit test/seed secondaries.
                    object.secondary_weapon = None;
                }
            }

            // Host residual: China Gattling Tank dual ground/AA + continuous-fire ramp state.
            // Fail-closed: not Overlord/Helix/building gattling payloads.
            if crate::game_logic::host_gattling_tank::is_gattling_tank_template(template_name) {
                use crate::game_logic::host_gattling_tank::{
                    gattling_air_weapon, gattling_ground_weapon, has_chain_guns_upgrade,
                    GattlingFireLevel,
                };
                let chain = has_chain_guns_upgrade(&object.applied_upgrades);
                object.weapon = Some(gattling_ground_weapon(GattlingFireLevel::Base, chain));
                object.secondary_weapon = Some(gattling_air_weapon(GattlingFireLevel::Base, chain));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: China Gattling Cannon structure dual ground/AA + continuous-fire ramp.
            // Fail-closed: not full CONTINUOUS_FIRE_* model-condition animation matrix.
            if crate::game_logic::host_base_defense::is_gattling_cannon_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    gattling_building_air_weapon, gattling_building_ground_weapon,
                    gattling_building_has_chain_guns,
                };
                use crate::game_logic::host_gattling_tank::GattlingFireLevel;
                let chain = gattling_building_has_chain_guns(&object.applied_upgrades);
                object.weapon = Some(gattling_building_ground_weapon(
                    GattlingFireLevel::Base,
                    chain,
                ));
                object.secondary_weapon =
                    Some(gattling_building_air_weapon(GattlingFireLevel::Base, chain));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: GLA Stinger Site SPAWNS_ARE_THE_WEAPONS dual ground/AA +
            // HiveStructureBody / SpawnBehavior residual (3 soldiers) + physical roster.
            if crate::game_logic::host_base_defense::is_stinger_site_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    init_stinger_hive_slave_roster, stinger_air_weapon, stinger_ground_weapon,
                    stinger_has_ap_rockets, sync_hive_slave_mirrors,
                };
                let ap = stinger_has_ap_rockets(&object.applied_upgrades);
                object.weapon = Some(stinger_ground_weapon(ap));
                object.secondary_weapon = Some(stinger_air_weapon(ap));
                let roster = init_stinger_hive_slave_roster();
                object.hive_slaves = roster;
                let (slaves, slave_hp) = sync_hive_slave_mirrors(&roster);
                object.hive_slave_count = slaves;
                object.record_host_hive();
                object.hive_slave_hp = slave_hp;
                object.record_host_hive();
                object.hive_slave_respawn_frame = 0;
            }

            // Host residual: USA Patriot dual ground/AA secondary.
            // Laser General residual uses Lazr_Patriot* damage (40/35) via template.
            // Fail-closed: not full AssistedTargetingModule assist clips / RequestAssistRange.
            if crate::game_logic::host_base_defense::is_patriot_battery_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    patriot_air_weapon_for_template, patriot_ground_weapon_for_template,
                };
                object.weapon = Some(patriot_ground_weapon_for_template(template_name));
                object.secondary_weapon = Some(patriot_air_weapon_for_template(template_name));
            }

            // Host residual: USA Crusader / Paladin PRIMARY tank gun
            // (Laser General Lazr_* → Lazr_CrusaderTankGun / Lazr_PaladinTankGun).
            // Fail-closed: not full LaserName beam drawable / shell lob matrix.
            if crate::game_logic::host_usa_tanks::is_crusader_template(template_name)
                || crate::game_logic::host_usa_tanks::is_paladin_template(template_name)
            {
                object.weapon = Some(
                    crate::game_logic::host_usa_tanks::usa_tank_gun_weapon_for_template(
                        template_name,
                    ),
                );
            }

            // Host residual: GLA Scorpion PRIMARY gun (+ secondary rocket if unlocked).
            // Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
            if crate::game_logic::host_scorpion::is_scorpion_template(template_name) {
                use crate::game_logic::host_scorpion::{
                    has_ap_rockets_upgrade, has_scorpion_rocket_upgrade,
                    salvage_tier_from_upgrades, scorpion_gun_weapon, scorpion_missile_weapon,
                };
                let tier = salvage_tier_from_upgrades(&object.applied_upgrades);
                object.weapon = Some(scorpion_gun_weapon(tier));
                if has_scorpion_rocket_upgrade(&object.applied_upgrades) {
                    let ap = has_ap_rockets_upgrade(&object.applied_upgrades);
                    object.secondary_weapon =
                        Some(scorpion_missile_weapon(ap, tier.dual_missile_clip()));
                }
            }

            // Host residual: USA Tomahawk PRIMARY dual-radius missile.
            // TomahawkMissile projectile lob residual closed (MissileAI peels + impact).
            if crate::game_logic::host_tomahawk::is_tomahawk_template(template_name) {
                use crate::game_logic::host_tomahawk::tomahawk_weapon;
                object.weapon = Some(tomahawk_weapon());
            }

            // Host residual: USA Raptor PRIMARY jet missiles (+ Laser Missiles upgrade).
            // RETURN_TO_BASE ClipReload airfield rearm residual closed (dock + timer).
            if crate::game_logic::host_raptor::is_raptor_template(template_name) {
                use crate::game_logic::host_raptor::{
                    has_laser_missiles_upgrade, is_king_raptor_template, raptor_weapon,
                };
                let king = is_king_raptor_template(template_name);
                let laser = has_laser_missiles_upgrade(&object.applied_upgrades);
                object.weapon = Some(raptor_weapon(king, laser));
            }

            // Host residual: China MiG PRIMARY napalm / Nuke dual-radius missiles.
            // Fail-closed: not full RETURN_TO_BASE ClipReload / HistoricBonus Firestorm matrix.
            if crate::game_logic::host_mig::is_mig_template(template_name) {
                use crate::game_logic::host_mig::{is_nuke_mig_template, mig_loadout, mig_weapon};
                let loadout = mig_loadout(
                    is_nuke_mig_template(template_name),
                    &object.applied_upgrades,
                );
                object.weapon = Some(mig_weapon(loadout));
            }

            // Host residual: America Fire Base PRIMARY howitzer.
            // Fail-closed: not full SPAWNS_ARE_THE_WEAPONS / garrison HiveStructure matrix.
            if crate::game_logic::host_fire_base::is_fire_base_template(template_name) {
                use crate::game_logic::host_fire_base::fire_base_weapon;
                object.weapon = Some(fire_base_weapon());
            }

            // Host residual: USA Stealth Fighter PRIMARY jet missiles.
            // Fail-closed: not full RETURN_TO_BASE ClipReload / science production matrix.
            if crate::game_logic::host_stealth_fighter::is_stealth_fighter_template(template_name) {
                use crate::game_logic::host_stealth_fighter::stealth_fighter_weapon;
                object.weapon = Some(stealth_fighter_weapon());
            }

            // Host residual: USA Comanche PRIMARY 20mm + SECONDARY anti-tank.
            // Retail rocket pods are a PLAYER_UPGRADE TERTIARY weapon; keep
            // anti-tank bound in SECONDARY and only expose pods after the team
            // owns the real upgrade.
            if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(template_name) {
                use crate::game_logic::host_comanche_rocket_pods::{
                    comanche_antitank_weapon, comanche_cannon_weapon, comanche_rocket_pod_weapon,
                    UPGRADE_COMANCHE_ROCKET_PODS,
                };
                object.weapon = Some(comanche_cannon_weapon());
                object.secondary_weapon = Some(comanche_antitank_weapon());
                let has_pods = object.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                    || object.has_upgrade_tag("Upgrade_ComancheRocketPods")
                    || self.players.values().any(|player| {
                        player.team == team
                            && player.has_unlocked_upgrade(UPGRADE_COMANCHE_ROCKET_PODS)
                    });
                if has_pods {
                    object.tertiary_weapon = Some(comanche_rocket_pod_weapon());
                    object.apply_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS);
                    object.weapon_set_player_upgrade = true;
                } else {
                    // The simple ObjectDefinition parser preserves the source
                    // name but does not evaluate full WeaponSet Conditions.
                    // Do not grant a condition-gated pod declaration early.
                    object.tertiary_weapon = None;
                }
            }

            // Host residual: USA Battle Drone PRIMARY machine gun.
            // Fail-closed: not full SlavedUpdate repair arm weld FX matrix.
            if crate::game_logic::host_slave_drones::is_battle_drone_template(template_name) {
                use crate::game_logic::host_slave_drones::battle_drone_weapon;
                object.weapon = Some(battle_drone_weapon());
            }

            // Host residual: China Overlord / Emperor PRIMARY dual-radius tank gun.
            // Fail-closed: not full ClipSize=2 dual-volley / Nuclear Tanks death residual.
            if crate::game_logic::host_overlord_gun::is_overlord_gun_chassis(template_name) {
                use crate::game_logic::host_overlord_gun::{
                    has_uranium_shells_upgrade, overlord_gun_weapon,
                };
                let uranium = has_uranium_shells_upgrade(&object.applied_upgrades);
                object.weapon = Some(overlord_gun_weapon(uranium));
            }

            // Host residual: GLA Jarmen Kell PRIMARY sniper residual.
            // Fail-closed: pilot-snipe special remains host_hero_abilities.
            if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
                use crate::game_logic::host_jarmen_kell::{
                    has_ap_bullets_upgrade, jarmen_kell_weapon,
                };
                let ap = has_ap_bullets_upgrade(&object.applied_upgrades);
                object.weapon = Some(jarmen_kell_weapon(ap));
            }

            // Host residual: China Red Guard PRIMARY machine gun residual.
            // Fail-closed: bayonet residual applied at fire-time for close infantry.
            if crate::game_logic::host_red_guard::is_red_guard_template(template_name) {
                use crate::game_logic::host_red_guard::red_guard_weapon;
                object.weapon = Some(red_guard_weapon(false, false));
            }

            // Host residual: China Tank Hunter PRIMARY RPG residual (AA + ground + splash).
            // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
            if crate::game_logic::host_tank_hunter::is_tank_hunter_template(template_name) {
                use crate::game_logic::host_tank_hunter::tank_hunter_weapon;
                object.weapon = Some(tank_hunter_weapon(false, false));
            }

            // Host residual: GLA Rebel PRIMARY machine gun residual.
            // Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
            if crate::game_logic::host_gla_rebel::is_gla_rebel_template(template_name) {
                use crate::game_logic::host_gla_rebel::{has_ap_bullets_upgrade, rebel_weapon};
                let ap = has_ap_bullets_upgrade(&object.applied_upgrades);
                object.weapon = Some(rebel_weapon(ap));
            }

            // Host residual: USA Ranger PRIMARY rifle residual.
            // FlashBang secondary is PLAYER_UPGRADE only (Upgrade_AmericaRangerFlashBangGrenade)
            // — parity with neutron shells / rocket pods: residual map may name the weapon,
            // but create strips it unless research is unlocked or template explicitly seeds it.
            // Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
            if crate::game_logic::host_ranger::is_ranger_template(template_name) {
                use crate::game_logic::host_ranger::{
                    has_flashbang_equipped, ranger_flashbang_weapon, ranger_rifle_weapon,
                    UPGRADE_AMERICA_FLASHBANG,
                };
                object.weapon = Some(ranger_rifle_weapon());
                let has_flashbang = has_flashbang_equipped(false, &object.applied_upgrades)
                    || self.players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG)
                    });
                if has_flashbang {
                    object.secondary_weapon = Some(ranger_flashbang_weapon());
                    object.apply_upgrade_tag(UPGRADE_AMERICA_FLASHBANG);
                } else if object.thing.template.secondary_weapon_name.is_none()
                    && object.thing.template.secondary_weapon.is_none()
                {
                    // Strip residual map auto-equip; keep explicit test/seed secondaries.
                    object.secondary_weapon = None;
                } else if object.secondary_weapon.is_some() {
                    // Explicit seed/test secondary — normalize to residual flashbang stats.
                    object.secondary_weapon = Some(ranger_flashbang_weapon());
                }
            }

            // Host residual: China MiniGunner dual ground/AA + continuous fire ramp.
            // Fail-closed: not full FiringTracker CONTINUOUS_FIRE_* anim / bayonet tertiary.
            if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
                use crate::game_logic::host_gattling_tank::GattlingFireLevel;
                use crate::game_logic::host_minigunner::{
                    has_chain_guns_upgrade, minigunner_air_weapon, minigunner_ground_weapon,
                };
                let chain = has_chain_guns_upgrade(&object.applied_upgrades);
                object.weapon = Some(minigunner_ground_weapon(
                    GattlingFireLevel::Base,
                    chain,
                    false,
                    false,
                ));
                object.secondary_weapon = Some(minigunner_air_weapon(
                    GattlingFireLevel::Base,
                    chain,
                    false,
                    false,
                ));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: Colonel Burton PRIMARY sniper residual.
            // Fail-closed: knife residual applied at fire-time for close infantry.
            if crate::game_logic::host_colonel_burton::is_colonel_burton_template(template_name) {
                use crate::game_logic::host_colonel_burton::burton_sniper_weapon;
                object.weapon = Some(burton_sniper_weapon());
            }

            // `VeterancyGainCreate StartingLevel` for an explicitly parsed
            // `VeterancyCrateCollide IsPilot` source.  A pilot-shaped
            // basename alone must not grant veterancy: retail data owns this
            // behavior, while ejection/parachute naming remains a separate
            // residual path.
            if let Some(target) = object
                .thing
                .template
                .veterancy_crate_collide
                .as_ref()
                .and_then(|metadata| metadata.pilot_starting_level())
            {
                use crate::game_logic::VeterancyLevel;
                if object.experience.level != target {
                    let prev = object.experience.level;
                    object.experience.level = target;
                    // Seed residual XP so level does not immediately drop on gain_experience.
                    if matches!(target, VeterancyLevel::Veteran) {
                        object.experience.current = object.experience.current.max(1.0);
                    }
                    let _ = prev;
                    // Apply bonuses only if promoting from Rookie.
                    if !matches!(
                        prev,
                        VeterancyLevel::Veteran | VeterancyLevel::Elite | VeterancyLevel::Heroic
                    ) {
                        // Direct level set; apply_veterancy_bonuses is private — call gain path
                        // by using a public residual: re-apply via temporary if needed.
                        // Object::apply_pilot_recrew already handles merge; for spawn we set level
                        // and leave HP/weapon multipliers fail-closed at template defaults until
                        // first combat XP event. Veterans still recrew-transfer as Veteran.
                    }
                }
            }

            // Host residual: GLA Worker base speed / WorkerShoes speed if already unlocked.
            // Fail-closed: not full WorkerAIUpdate bored auto-task matrix.
            if crate::game_logic::host_gla_worker::is_gla_worker_template(template_name) {
                use crate::game_logic::host_gla_worker::{
                    worker_residual_speed, UPGRADE_GLA_WORKER_SHOES,
                };
                let shoes = object.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
                object.movement.max_speed = worker_residual_speed(shoes);
            }

            // Host residual: GLA RPG Trooper / Tunnel Defender PRIMARY rocket residual.
            // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
            if crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(template_name) {
                use crate::game_logic::host_rpg_trooper::{
                    has_ap_rockets_upgrade, rpg_trooper_weapon,
                };
                let ap = has_ap_rockets_upgrade(&object.applied_upgrades);
                object.weapon = Some(rpg_trooper_weapon(ap));
            }

            // Host residual: GLA Terrorist PRIMARY TerroristSuicideWeapon residual.
            // Chem Beta/Gamma + Demo death-weapon residual profiles.
            // Fail-closed: not ConvertToCarBomb full matrix / SlowDeath fling.
            if crate::game_logic::host_terrorist::is_terrorist_template(template_name) {
                use crate::game_logic::host_terrorist::{
                    terrorist_death_profile, terrorist_suicide_weapon_for_profile,
                };
                let has_gamma = object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                    || object.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                let has_beta = object.has_upgrade_tag("Upgrade_GLAAnthraxBeta")
                    || object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxBeta");
                let profile = terrorist_death_profile(template_name, has_gamma, has_beta);
                object.weapon = Some(terrorist_suicide_weapon_for_profile(profile));
                object.secondary_weapon = None;
            }

            // Host residual: USA Missile Defender PRIMARY missile + SECONDARY laser guided.
            // Fail-closed: not full SpecialAbilityUpdate prep / LaserBeam object matrix.
            if crate::game_logic::host_missile_defender::is_missile_defender_template(template_name)
            {
                use crate::game_logic::host_missile_defender::{
                    missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
                };
                object.weapon = Some(missile_defender_primary_weapon());
                object.secondary_weapon = Some(missile_defender_laser_guided_weapon());
            }

            // Host residual: America Scout Drone StealthDetectorUpdate (VisionRange 150).
            if crate::game_logic::host_slave_drones::scout_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_slave_drones::scout_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                // Sensor drone: strip kind-based default gun if no explicit primary.
                // Reuse sentry_had_explicit_primary (same template fields, captured pre-move).
                if !sentry_had_explicit_primary {
                    object.weapon = None;
                }
            }

            // Host residual: America Hellfire Drone AutoAcquire + HellfireMissileWeapon.
            // Weapon bound via weapon_bootstrap primary; no extra strip.
            // Auto-fire residual runs from update_combat when idle.

            if let Some(level) = player_template_veterancy {
                let _ = object.set_min_veterancy_level(level);
            }

            object.ensure_fire_weapon_when_damaged();
            object.ensure_transition_damage_fx();
            object.ensure_fx_list_die();
            object.ensure_create_object_die();
            object.ensure_lifetime_update(self.frame);
            object.ensure_height_die(self.frame);
            self.objects.insert(id, object);
            if !starts_under_construction {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.stamp_partition_value_threat();
                }
            }
            // C++ Object.cpp onCreate residual: inherit team prototype attitude + attack priority.
            self.inherit_team_ai_defaults(id);

            // C++ SpecialPowerModule StartsPaused=Yes residual (pauseCountdown TRUE on create).
            self.init_starts_paused_special_powers(id);

            // C++ SupplyWarehouseCreate::onCreate residual — StartingBoxes.
            self.init_supply_warehouse_create(id);

            // C++ SpawnBehavior ModuleTag_12 on stock and general-specific
            // SupplyCenter/Stash objects.  `create_object_under_construction`
            // intentionally does not call this; that path fires on the real
            // construction-complete activation edge below.
            let _ = self.spawn_supply_center_one_shot_collector(id);

            // Residual honesty: Emperor innate propaganda counts as install on spawn.
            if emperor_spawn {
                self.overlord_addons.record_propaganda_install();
            }
            let _ = helix_spawn;

            // Host residual: Listening Outpost InitialPayload TankHunter × 2.
            // Dock after insert so recursive create_object cannot re-enter mid-build.
            // Fail-closed: no payload if TankHunter template is absent.
            if is_listening_outpost_spawn {
                self.apply_listening_outpost_initial_payload(id, team, position);
            }

            // Host residual: Troop Crawler InitialPayload Redguard × 8.
            // Dock after insert so recursive create_object cannot re-enter mid-build.
            if is_troop_crawler_spawn {
                self.apply_troop_crawler_initial_payload(id, team, position);
            }

            // C++ VeterancyGainCreate::onCreate, then PlayerTemplate fallback.
            {
                use crate::game_logic::host_unit_training::{
                    normalize_identity, unit_training_level_for_template, veterancy_rank,
                };
                let sciences: Vec<String> = self
                    .players
                    .values()
                    .filter(|p| p.team == team)
                    .flat_map(|p| p.unlocked_sciences.iter().cloned())
                    .collect();
                if let Some(obj) = self.objects.get_mut(&id) {
                    let mut ini_level = None;
                    for module in &obj.thing.template.veterancy_gain_creates {
                        let science_ok = match &module.science_required {
                            None => true,
                            Some(sci) => sciences.iter().any(|s| {
                                normalize_identity(s) == normalize_identity(sci)
                            }),
                        };
                        if science_ok && obj.is_trainable() {
                            let lvl = module.starting_level;
                            if ini_level
                                .map(|best| veterancy_rank(lvl) > veterancy_rank(best))
                                .unwrap_or(true)
                            {
                                ini_level = Some(lvl);
                            }
                        }
                    }
                    if let Some(level) = ini_level {
                        let _ = obj.set_min_veterancy_level(level);
                    } else if let Some((kind, level)) =
                        unit_training_level_for_template(template_name, &sciences)
                    {
                        if obj.set_min_veterancy_level(level) {
                            self.unit_training.record_grant(kind);
                        }
                    }
                }
            }

            // Host residual: Demo SuicideBomb tag + CommandSetUpgrade if researched.
            {
                use crate::game_logic::host_demo_suicide_bomb::{
                    demo_command_set_upgrade_for_template, is_demo_suicide_bomb_eligible_template,
                    is_demo_suicide_bomb_upgrade, UPGRADE_DEMO_SUICIDE_BOMB,
                };
                if is_demo_suicide_bomb_eligible_template(template_name) {
                    let has_upgrade = self.players.values().any(|p| {
                        p.team == team
                            && p.unlocked_sciences
                                .iter()
                                .any(|s| is_demo_suicide_bomb_upgrade(s))
                    });
                    if has_upgrade {
                        if let Some(obj) = self.objects.get_mut(&id) {
                            if !obj.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB) {
                                obj.apply_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB);
                                self.demo_suicide_bomb.record_tag();
                            }
                            if obj.command_set_override.is_none() {
                                if let Some(cs) =
                                    demo_command_set_upgrade_for_template(&obj.template_name)
                                {
                                    obj.set_command_set_override(Some(cs));
                                    self.demo_suicide_bomb.record_command_set_upgrade(1);
                                }
                            }
                        }
                    }
                }
            }

            if counts_as_unit {
                self.record_unit_production(team);
            } else if is_structure && !starts_under_construction {
                self.record_structure_completion(team);
                // Static path/LOS obstacle (C++ pathfind structure residual).
                self.block_structure_object_path(id);
                // Map-placed / instant SW: onSpecialPowerCreation residual.
                self.on_structure_superweapon_creation(id);
            }
            log::debug!(
                "Created object {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            let team_ord = match team {
                Team::USA => 0u8,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            };
            crate::game_logic::host_spawn_log::record(
                id,
                template_name.to_string(),
                team_ord,
                [position.x, position.y, position.z],
            );
            // Wave 680: mid-frame GameWorld map while coupled shadow tick is live.
            // End-of-tick host_spawn_log drain remains idempotent for unmapped IDs.
            let _ = crate::gameworld_shadow::eager_map_host_spawn_if_coupled(
                self,
                &crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id,
                    template: template_name.to_string(),
                    team_ordinal: team_ord,
                    position: [position.x, position.y, position.z],
                },
            );
            if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                obj.record_model_mesh_from_template();
                obj.record_kind_of_bits_from_template();
            }
            // C++ Object.cpp:473 TheRadar->addObject(this) after modules ready.
            self.host_radar_add_object(id);
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Create a construction object for an unambiguous faction owner. Command
    /// and builder paths that know the player must use the owned variant.
    pub fn create_object_under_construction(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        let owner_player_id = self.unique_player_id_for_team(team);
        self.create_object_under_construction_with_owner(
            template_name,
            team,
            owner_player_id,
            position,
        )
    }

    /// Create a construction object for one controlling player.
    pub fn create_object_under_construction_for_player(
        &mut self,
        template_name: &str,
        owner_player_id: u32,
        position: Vec3,
    ) -> Option<ObjectId> {
        let team = self.players.get(&owner_player_id)?.team;
        if team == Team::Neutral {
            return None;
        }
        self.create_object_under_construction_with_owner(
            template_name,
            team,
            Some(owner_player_id),
            position,
        )
    }

    fn create_object_under_construction_with_owner(
        &mut self,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
    ) -> Option<ObjectId> {
        if owner_player_id.is_some_and(|player_id| {
            self.players.get(&player_id).map(|player| player.team) != Some(team)
        }) {
            return None;
        }
        // C++ BuildAssistant isLocationLegalToBuild residual (objects-in-way / bounds).
        if !self.is_location_legal_to_build(team, position, template_name) {
            log::debug!(
                "Blocked construction {} at {:?} (LegalBuildCode residual)",
                template_name,
                position
            );
            return None;
        }
        // C++ ProductionPrerequisite residual (known sample table / SW tech tree).
        let prerequisites_ok = owner_player_id
            .map(|player_id| self.player_satisfies_build_prerequisites(player_id, template_name))
            .unwrap_or_else(|| self.team_satisfies_build_prerequisites(team, template_name));
        if !prerequisites_ok {
            log::debug!(
                "Blocked construction {} for team {:?} (Prerequisites residual)",
                template_name,
                team
            );
            return None;
        }
        // C++ MaxSimultaneousOfType=DeterminedBySuperweaponRestriction residual.
        let superweapon_ok = owner_player_id
            .map(|player_id| {
                self.can_start_superweapon_building_for_player(player_id, template_name)
            })
            .unwrap_or_else(|| self.can_start_superweapon_building(team, template_name));
        if !superweapon_ok {
            log::debug!(
                "Blocked superweapon construction {} for team {:?} (MaxSimultaneous residual)",
                template_name,
                team
            );
            return None;
        }
        if let Some(template) = self.templates.get(template_name).cloned() {
            // Keep the same PlayerTemplate veterancy binding for a placed
            // structure as for a completed production spawn.  C++ creates the
            // ExperienceTracker before construction completes, not only when
            // its build timer reaches 100%.
            let player_template_veterancy = owner_player_id.and_then(|player_id| {
                self.player_template_production_veterancy(player_id, template_name)
            });
            let id = self.allocate_object_id();
            let partition_cash = template.build_cost.supplies;
            let mut object = Object::new_under_construction(template, id, team);
            object.owner_player_id = owner_player_id;
            object.partition_cash_value = partition_cash;
            object.partition_threat_value = partition_cash.max(1);
            object.set_position(position);
            if let Some(level) = player_template_veterancy {
                let _ = object.set_min_veterancy_level(level);
            }
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    id,
                    Some([position.x, position.y, position.z]),
                );
                object.record_host_movement();
            }

            self.objects.insert(id, object);
            self.inherit_team_ai_defaults(id);

            let team_ord = match team {
                Team::USA => 0u8,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            };
            crate::game_logic::host_spawn_log::record(
                id,
                template_name.to_string(),
                team_ord,
                [position.x, position.y, position.z],
            );
            // Wave 680: mid-frame GameWorld map while coupled shadow tick is live.
            // End-of-tick host_spawn_log drain remains idempotent for unmapped IDs.
            let _ = crate::gameworld_shadow::eager_map_host_spawn_if_coupled(
                self,
                &crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id,
                    template: template_name.to_string(),
                    team_ordinal: team_ord,
                    position: [position.x, position.y, position.z],
                },
            );
            // Wave 199: GameWorld SetConstruction sole-tick / progress last-writer.
            crate::game_logic::host_construction_progress_log::record(id, 0.0, true, 0.0);
            if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                obj.record_model_mesh_from_template();
                obj.record_kind_of_bits_from_template();
            }
            // C++ Object.cpp:473 TheRadar->addObject(this) (under construction too).
            self.host_radar_add_object(id);

            log::debug!(
                "Started construction of {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Destroy an object
    pub fn destroy_object(&mut self, id: ObjectId) {
        self.mark_object_for_destruction(id, None);
    }

    /// Wave 482: sell residual kill (parked aircraft) — queue remove without
    /// SlowDeath/Topple deferral peels used for combat deaths.
    pub(in super::super) fn destroy_object_for_sell_residual(&mut self, id: ObjectId) {
        self.maybe_notify_special_power_completion(id);
        self.maybe_apply_dam_die(id);
        let _ = self.apply_ocl_random_force(id);
        self.maybe_apply_upgrade_die(id);
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer: None });
    }

    /// C++ FireWeaponWhenDeadBehavior::onDie residual — death weapon splash.
    pub(in super::super) fn apply_fire_weapon_when_dead(&mut self, dying_id: ObjectId) {
        use crate::game_logic::host_fire_weapon_when_dead::{
            death_weapon_for_template, splash_damage_at_distance,
        };

        let Some(obj) = self.objects.get(&dying_id) else {
            return;
        };
        if obj.fire_weapon_when_dead_fired {
            return;
        }
        if obj.status.under_construction {
            return;
        }
        let Some(splash) = death_weapon_for_template(&obj.template_name) else {
            return;
        };
        let pos = obj.get_position();
        let team = obj.team;
        let max_r = splash.primary_radius.max(splash.secondary_radius);

        let is_helix_napalm_bomb = obj.helix_napalm_bomb_projectile;
        let napalm_source = obj.producer_id;
        let black_napalm_bomb = obj.template_name.to_ascii_lowercase().contains("black");

        // Mark fired
        if let Some(obj) = self.objects.get_mut(&dying_id) {
            obj.fire_weapon_when_dead_fired = true;
        }

        let victims: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if *id == dying_id || !o.is_alive() {
                    return None;
                }
                let p = o.get_position();
                let dx = p.x - pos.x;
                let dz = p.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= max_r {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        let mut destroy_ids = Vec::new();
        for vid in victims {
            let Some(v) = self.objects.get_mut(&vid) else {
                continue;
            };
            let p = v.get_position();
            let dx = p.x - pos.x;
            let dz = p.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let dmg = splash_damage_at_distance(&splash, dist);
            if dmg <= 0.0 {
                continue;
            }
            let destroyed = v.take_damage_from_immediate(dmg, Some(dying_id));
            if destroyed {
                destroy_ids.push(vid);
            }
        }
        // Presentation residual: death explosion particle at epicenter.
        let _ = self.combat_particles.spawn(
            crate::game_logic::combat_particles::CombatParticleKind::DeathExplosion,
            pos,
            self.frame,
            Some(dying_id),
            None,
        );
        if is_helix_napalm_bomb {
            // Honesty: HeightDie detonation residual counted as blast path.
            self.helix_napalm.blast_hits = self
                .helix_napalm
                .blast_hits
                .saturating_add(destroy_ids.len() as u32);
            let _ = (napalm_source, black_napalm_bomb);
        }
        let _ = team;
        for id in destroy_ids {
            // Avoid re-entrancy loops: queue destroy without re-firing this dying unit.
            if id != dying_id {
                self.objects_to_destroy.push_back(DestructionEvent {
                    id,
                    killer: Some(team),
                });
            }
        }
    }

    /// Wave 752: lethal finish that respects damage-authority HP last-write.
    /// Prefer this over direct host HP zeroing for production destroy residual.
    #[allow(dead_code)]
    pub(crate) fn host_lethal_finish_object(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
    ) -> bool {
        let Some(o) = self.objects.get_mut(&id) else {
            return false;
        };
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            crate::game_logic::host_damage_log::record(id, hp, source, true);
        } else {
            o.health.current = 0.0;
        }
        o.status.destroyed = true;
        o.status.effectively_dead = true;
        true
    }

    /// Return the subset of a selected EjectPilot OCL that this live host can
    /// reproduce without substituting a name-shaped effect.
    ///
    /// `EjectPilotDie` owns only the typed ground/air OCL selection.  In
    /// particular, its own `InvulnerableTime` field is not consumed by the
    /// C++ `onDie`; `GenericObjectCreationNugget::InvulnerableTime` on the
    /// selected OCL is the source of the spawned pilot's protection.  Keep
    /// that distinction here so a module default of zero never becomes a
    /// fabricated 2000 ms grant.
    fn parsed_eject_pilot_ocl_plan(
        creation_list: crate::game_logic::EjectPilotCreationList,
    ) -> Option<(bool, u32)> {
        use crate::game_logic::host_usa_pilot::EJECT_PILOT_TEMPLATE;
        use gamelogic::object_creation_list::{
            DebrisDisposition, GenericObjectCreationNugget, ObjectCreationNugget,
        };

        // These are the only two OCL identities the typed parser admits.  The
        // enum is deliberately not a free-form INI name, so this lookup cannot
        // make an arbitrary creation list act like EjectPilotDie.
        let (ocl_name, parachute_ocl, expected_container, min_force, max_force) =
            match creation_list {
                crate::game_logic::EjectPilotCreationList::OnGround => {
                    ("OCL_EjectPilotOnGround", false, "", 2.0, 3.0)
                }
                crate::game_logic::EjectPilotCreationList::ViaParachute => (
                    "OCL_EjectPilotViaParachute",
                    true,
                    "AmericaParachute",
                    10.0,
                    12.0,
                ),
            };
        let ocl =
            gamelogic::helpers::TheObjectCreationListStore::lookup_object_creation_list(ocl_name)?;
        let [nugget] = ocl.nuggets() else {
            return None;
        };
        let generic = nugget
            .as_any()
            .downcast_ref::<GenericObjectCreationNugget>()?;

        // The existing ejection/parachute host represents precisely the two
        // retail OCL shapes below.  Refuse a changed/mixed OCL rather than
        // silently issuing only its familiar-looking pilot portion.
        let supported_shape = generic.name_are_objects
            && generic.debris_to_generate == 1
            && generic.names.len() == 1
            && generic.names[0].eq_ignore_ascii_case(EJECT_PILOT_TEMPLATE)
            && generic.ignore_primary_obstacle
            && generic.inherit_veterancy
            && generic.disposition == DebrisDisposition::new(DebrisDisposition::RANDOM_FORCE)
            && generic.min_mag == min_force
            && generic.max_mag == max_force
            // `parse_angle_real` stores the authored degree literals in the
            // engine's radians representation.
            && generic.min_pitch == 50.0_f32.to_radians()
            && generic.max_pitch == 60.0_f32.to_radians()
            && generic.spin_rate == 0.0
            && generic.requires_live_player
            && generic
                .put_in_container
                .eq_ignore_ascii_case(expected_container)
            && !generic.contain_inside_source_object
            && !generic.skip_if_significantly_airborne
            && !generic.dies_on_bad_land
            && !generic.spread_formation
            && !generic.fade_in
            && !generic.fade_out;
        supported_shape.then_some((parachute_ocl, generic.invulnerable_time))
    }

    /// Wave 754: C++ EjectPilotDie::onDie residual at death start (mark_object),
    /// not only final process_destroy remove. SlowDeath defers remove and must
    /// not suppress pilot spawn / honesty residual.
    pub(crate) fn maybe_apply_eject_pilot_die(&mut self, id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            air_eject_spawn_height, is_significantly_above_terrain, HostDeathType,
            EJECT_PILOT_TEMPLATE, PILOT_EJECT_AUDIO,
        };
        use crate::game_logic::{
            EjectPilotCreationList, EjectPilotDeathTypes, EjectPilotExemptStatus,
            EjectPilotVeterancyLevels, VeterancyLevel,
        };

        let (
            metadata,
            pilot_team,
            pilot_owner_player_id,
            death_pos,
            veterancy,
            death_type,
            is_hijacked,
        ) = {
            let Some(obj) = self.objects.get(&id) else {
                return;
            };
            if obj.eject_pilot_die_applied {
                return;
            }
            let Some(metadata) = obj.thing.template.eject_pilot_die else {
                // Module presence, not an object basename, is the C++ die
                // authority.  A name-shaped vehicle with no parsed module is
                // intentionally inert here.
                return;
            };
            (
                metadata,
                obj.team,
                obj.owner_player_id,
                obj.get_position(),
                obj.experience.level,
                obj.status.death_type,
                obj.status.hijacked,
            )
        };

        // C++ invokes a DieModule once per death.  The host may visit this
        // object again while SlowDeath unwinds, so record the attempt before
        // any supported filter/OCL can decline it.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.eject_pilot_die_applied = true;
        }

        let death_is_crushed_or_splatted =
            matches!(death_type, HostDeathType::Crushed | HostDeathType::Splatted);
        let veterancy_is_regular = matches!(veterancy, VeterancyLevel::Rookie);
        let death_types_gate = match metadata.death_types {
            EjectPilotDeathTypes::All => true,
            EjectPilotDeathTypes::AllExceptCrushedAndSplatted => !death_is_crushed_or_splatted,
            EjectPilotDeathTypes::Unsupported => false,
        };
        let veterancy_gate = match metadata.veterancy_levels {
            EjectPilotVeterancyLevels::All => true,
            EjectPilotVeterancyLevels::AllExceptRegular => !veterancy_is_regular,
            EjectPilotVeterancyLevels::Unsupported => false,
        };
        let exempt_status_gate = match metadata.exempt_status {
            EjectPilotExemptStatus::None => true,
            EjectPilotExemptStatus::Hijacked => !is_hijacked,
            EjectPilotExemptStatus::Unsupported => false,
        };

        // Preserve the existing observability counters, but now only when the
        // corresponding parsed DieMux clause actually owns the block.
        if matches!(
            metadata.veterancy_levels,
            EjectPilotVeterancyLevels::AllExceptRegular
        ) && death_types_gate
            && exempt_status_gate
            && !veterancy_gate
        {
            self.usa_pilot.record_eject_veterancy_block();
        }
        if matches!(
            metadata.death_types,
            EjectPilotDeathTypes::AllExceptCrushedAndSplatted
        ) && veterancy_gate
            && exempt_status_gate
            && !death_types_gate
        {
            self.usa_pilot.record_eject_death_type_block();
        }
        if matches!(metadata.exempt_status, EjectPilotExemptStatus::Hijacked)
            && veterancy_gate
            && death_types_gate
            && !exempt_status_gate
        {
            self.usa_pilot.record_eject_hijacked_block();
        }

        if !metadata.allows_supported_death(
            death_is_crushed_or_splatted,
            veterancy_is_regular,
            is_hijacked,
        ) {
            return;
        }

        // C++ `EjectPilotDie::onDie` selects the OCL solely from
        // `Object::isSignificantlyAboveTerrain()`.  `airborne_target` is not
        // an alternate authorization route.
        let terrain_y = self.terrain_height_at(death_pos).unwrap_or(0.0);
        let significantly_above_terrain = is_significantly_above_terrain(death_pos.y - terrain_y);
        let Some(creation_list) = metadata.creation_list_for_air_path(significantly_above_terrain)
        else {
            // A null/unsupported selected OCL is C++'s no-op `ejectPilot`.
            return;
        };
        let Some((parachute_ocl, invulnerable_frames)) =
            Self::parsed_eject_pilot_ocl_plan(creation_list)
        else {
            return;
        };

        // `RequiresLivePlayer = Yes` is part of both retail OCLs.  C++ rejects
        // a missing source controller as well as a defeated one; do not let a
        // team-only fallback create a useful pilot for an ownerless wreck.
        let Some(pilot_owner_player_id) = pilot_owner_player_id.filter(|player_id| {
            self.players
                .get(player_id)
                .is_some_and(|player| player.is_alive && player.team == pilot_team)
        }) else {
            return;
        };
        if !self.templates.contains_key(EJECT_PILOT_TEMPLATE) {
            // OCL_EjectPilot* names this exact retail object.  Do not inject
            // a synthetic pilot when the authored template cannot be loaded:
            // missing Object INI data must not turn a source name into a live
            // ejection action.
            let Some(pilot_tpl) = Self::build_template_from_asset_definition(EJECT_PILOT_TEMPLATE)
            else {
                return;
            };
            self.templates
                .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
        }
        // Offset slightly so pilot is not buried under death debris residual.
        // The chosen OCL (not vehicle kind/name) controls whether the live
        // host applies the existing AmericaParachute residual.
        let spawn_pos = if parachute_ocl {
            glam::Vec3::new(
                death_pos.x + 2.0,
                air_eject_spawn_height(death_pos.y),
                death_pos.z + 2.0,
            )
        } else {
            death_pos + glam::Vec3::new(2.0, 0.0, 2.0)
        };
        if let Some(pilot_id) =
            self.create_object_for_player(EJECT_PILOT_TEMPLATE, pilot_owner_player_id, spawn_pos)
        {
            self.usa_pilot.record_ejection();
            if parachute_ocl {
                self.usa_pilot.record_air_ejection();
            }
            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                if invulnerable_frames > 0 {
                    pilot.apply_eject_invulnerable(self.frame.saturating_add(invulnerable_frames));
                }
                if parachute_ocl {
                    let raw_y = pilot.get_position().y;
                    pilot.apply_eject_parachuting();
                    if crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged(
                        raw_y, 0.0,
                    ) {
                        self.usa_pilot.record_parachute_open_fudge();
                    }
                }
                // OCL_EjectPilot* has `InheritsVeterancy = Yes`.
                pilot.experience.level = veterancy;
            }
            if invulnerable_frames > 0 {
                self.usa_pilot.record_invulnerable_grant();
            }
            self.queue_audio_event(
                AudioEventRequest::new(PILOT_EJECT_AUDIO)
                    .with_position(spawn_pos)
                    .with_priority(170),
            );
            let _ = pilot_id;
        }
    }

    pub(crate) fn mark_object_for_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.unstamp_partition_value_threat();
        }
        // C++ ProductionUpdate cancelAndRefund on death start (before topple/slow-death deferral).
        self.cancel_all_production(id);
        // C++ SpecialPowerCompletionDie::onDie residual.
        self.maybe_notify_special_power_completion(id);
        // C++ DamDie::onDie residual fires with other die modules at death start.
        self.maybe_apply_dam_die(id);
        // Wave 754: C++ EjectPilotDie::onDie at death start (before SlowDeath defer).
        self.maybe_apply_eject_pilot_die(id);
        // C++ OCL ApplyRandomForceNugget residual (air-death toss before debris).
        let _ = self.apply_ocl_random_force(id);
        // C++ UpgradeDie::onDie residual — free producer's upgrade slot.
        self.maybe_apply_upgrade_die(id);
        // Wave 482: BuildAssistant sell finish removes the object immediately.
        // Do not defer into StructureTopple/Collapse / SlowDeath / KeepObjectDie —
        // those combat-death peels left sold structures alive forever in host-only tests.
        let (sold, under_construction) = self
            .objects
            .get(&id)
            .map(|o| (o.status.sold, o.status.under_construction))
            .unwrap_or((false, false));
        // Wave 715: MSG_DOZER_CANCEL_CONSTRUCT / unfinished builds remove immediately.
        // Do not defer into StructureTopple — cancel would leave the shell alive a frame+.
        if !sold && !under_construction {
            // C++ StructureTopple/Collapse residual: buildings fall/sink before remove.
            if self.try_begin_structure_topple_instead_of_destroy(id, killer) {
                return;
            }
            // C++ SlowDeathBehavior residual: infantry/vehicles delay destroy + sink.
            if self.try_begin_slow_death_instead_of_destroy(id, killer) {
                return;
            }
            // C++ KeepObjectDie residual: leave rubble, do not DestroyDie-remove.
            if self.try_begin_keep_object_die_instead_of_destroy(id, killer) {
                return;
            }
        }
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer });
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.status.destroyed = true;
        }
        let _ = crate::gameworld_shadow::eager_mark_host_destroy_if_coupled(id);
    }

    /// C++ KeepObjectDie residual: convert to lasting rubble, skip remove.
    pub(in super::super) fn try_begin_keep_object_die_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Wave 775: StructureCollapse/Topple already ran their presentation; after Done
        // allow normal destroy instead of KeepObjectDie forever-defer (civilian barns).
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        if obj.status.keep_as_rubble {
            let _ = killer;
            return true;
        }
        if !obj.begin_keep_object_die(frame) {
            return false;
        }
        let _ = killer;
        // Death FX / OCL peels without world removal.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.fire_fx_list_die();
            obj.fire_create_object_die();
        }
        self.apply_pending_create_object_die(id);
        let is_dam = self
            .objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false);
        if is_dam {
            self.apply_dam_die_enable_waveguides();
        }
        true
    }

    /// C++ DamDie::onDie residual — enable KINDOF_WAVEGUIDE objects.
    /// C++ UpgradeDie::onDie residual.
    pub(in super::super) fn maybe_apply_upgrade_die(&mut self, id: ObjectId) {
        let (producer, upgrade) = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            let Some(ud) = obj.upgrade_die.as_mut() else {
                return;
            };
            if ud.fired {
                return;
            }
            ud.fired = true;
            (obj.producer_id, ud.upgrade_to_remove.clone())
        };
        let Some(pid) = producer else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        let Some(master) = self.objects.get_mut(&pid) else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        if master.remove_upgrade_tag(&upgrade) {
            self.upgrade_die_reg.record_removal();
        } else {
            self.upgrade_die_reg.record_missing_upgrade();
        }
    }

    pub(in super::super) fn maybe_apply_dam_die(&mut self, id: ObjectId) {
        let is_dam = self
            .objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false);
        if is_dam {
            self.apply_dam_die_enable_waveguides();
        }
    }

    pub(in super::super) fn apply_dam_die_enable_waveguides(&mut self) {
        let frame = self.frame;
        for obj in self.objects.values_mut() {
            let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                || crate::game_logic::host_dam_die::is_wave_guide_template(&obj.template_name)
                || crate::game_logic::host_wave_guide::is_wave_guide_template(&obj.template_name);
            if is_wg {
                obj.status.disabled_default = false;
                if obj.wave_guide_data.is_none() {
                    let mut wg = crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                    wg.facing = obj.get_orientation();
                    wg.ensure_active(frame.max(1));
                    obj.wave_guide_data = Some(wg);
                } else if let Some(wg) = obj.wave_guide_data.as_mut() {
                    wg.ensure_active(frame.max(1));
                }
            }
        }
    }

    pub(in super::super) fn try_begin_slow_death_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Jet crash residual.
        if obj.jet_slow_death.as_ref().map(|j| j.done).unwrap_or(false) {
            return false;
        }
        if obj
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_jet_slow_death() {
            let _ = killer;
            return true;
        }
        // Helicopter spiral crash residual.
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.done)
            .unwrap_or(false)
        {
            return false;
        }
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_helicopter_slow_death() {
            let _ = killer;
            return true;
        }
        // Already finished slow death → allow destroy.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_done())
            .unwrap_or(false)
        {
            return false;
        }
        // Mid slow death → keep deferring.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        // C++ SlowDeathBehavior::onDie (SlowDeathBehavior.cpp:456-501) runs only
        // when the object has that Die module. DestroyDie.cpp:35-40 then calls
        // GameLogic::destroyObject (GameLogic.cpp:3932-3967): OBJECT_STATUS_DESTROYED
        // + m_objectsToDestroy.push_back same frame. `begin_slow_death` used
        // wants_slow_death(KindOf::Vehicle) for every tank, which is not a module
        // check — it reset hp=0.01 / destroyed=false and skipped the destroy
        // queue, so processDestroyList (GameLogic.cpp:3762) never awarded bounty
        // (Player.cpp:2400 doBountyForKill) or UnitDie audio (Object.cpp:4595-4606
        // EVA_UnitLost onDie path). Do not auto-start residual SlowDeath here.
        false
    }

    pub(crate) fn apply_structure_topple_crush_samples(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
    ) {
        if samples.is_empty() {
            return;
        }
        let building_team = self.objects.get(&building_id).map(|o| o.team);
        let mut destroy: Vec<ObjectId> = Vec::new();
        const SAMPLE_RADIUS: f32 = 18.0;
        let victims: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in victims {
            if id == building_id {
                continue;
            }
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let pos = obj.get_position();
            let mut best_dmg = 0.0_f32;
            for s in &samples {
                let dx = pos.x - s.x;
                let dz = pos.z - s.z;
                if dx * dx + dz * dz <= SAMPLE_RADIUS * SAMPLE_RADIUS {
                    best_dmg = best_dmg.max(s.damage);
                }
            }
            if best_dmg <= 0.0 {
                continue;
            }
            let killed = if let Some(obj) = self.objects.get_mut(&id) {
                // Structure topple crush residual is effectively unresistable for units
                // under the fall sweep (C++ doDamageLine lethality residual).
                let mut dead = obj.take_damage_from_typed_death(
                    best_dmg,
                    Some(building_id),
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                );
                // Fail-closed lethal finish: crush sweep leaves no standing unit residual.
                // Wave 746: under damage authority, do not zero host HP (dual with GW
                // HP writeback). Project lethal via damage log + destroyed flags;
                // non-authority path keeps host HP clear.
                if !obj.status.destroyed {
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        let hp = obj.health.current.max(1.0);
                        crate::game_logic::host_damage_log::record(id, hp, Some(building_id), true);
                        obj.status.destroyed = true;
                        obj.status.effectively_dead = true;
                        obj.status.death_type =
                            crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
                    } else {
                        // Wave 753: under damage authority, do not zero host HP mid-frame
                        // (dual with GW HP writeback). Project lethal via damage log + flags.
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            let hp = obj.health.current.max(1.0);
                            let oid = obj.id;
                            crate::game_logic::host_damage_log::record(oid, hp, None, true);
                        } else {
                            obj.health.current = 0.0;
                        }
                        obj.status.destroyed = true;
                        obj.status.effectively_dead = true;
                        obj.status.death_type =
                            crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
                    }
                    dead = true;
                }
                dead
            } else {
                false
            };
            if killed
                || self
                    .objects
                    .get(&id)
                    .map(|o| o.status.destroyed || o.health.current <= 0.0)
                    .unwrap_or(false)
            {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, building_team);
        }
    }

    /// C++ FireWeaponWhenDamagedBehavior forceFireWeapon residual at object position.

    /// C++ CreateObjectDie::onDie residual — spawn OCL templates at dying object.
    pub fn apply_pending_create_object_die(&mut self, dying_id: ObjectId) {
        let (spawns, transfer_dmg, transfer, team, pos) = {
            let Some(o) = self.objects.get_mut(&dying_id) else {
                return;
            };
            let (spawns, dmg, transfer) = o.take_pending_create_object_die_spawns();
            (spawns, dmg, transfer, o.team, o.get_position())
        };
        if spawns.is_empty() {
            return;
        }
        for tmpl in spawns {
            // CreateDebris disposition residual for GenericDebris peels.
            let tl = tmpl.to_ascii_lowercase();
            if tl.contains("debris") || tl.contains("barrel") {
                use crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan;
                let plan = if tl.contains("barrel") {
                    HostOclCreateDebrisPlan::damaged_barrel()
                } else {
                    let mut p = HostOclCreateDebrisPlan::generic_tank_debris();
                    p.model_or_template = tmpl.clone();
                    p.count = 1;
                    p
                };
                let inherit = self
                    .objects
                    .get(&dying_id)
                    .map(|o| o.movement.velocity)
                    .unwrap_or(Vec3::ZERO);
                let ids = self.spawn_ocl_create_debris(&plan, team, pos, inherit);
                if transfer && transfer_dmg > 0.0 {
                    for id in ids {
                        if let Some(n) = self.objects.get_mut(&id) {
                            let _ = n.take_damage(transfer_dmg);
                        }
                    }
                }
                continue;
            }
            // Ensure template name exists for residual peels.
            if !self.templates.contains_key(&tmpl) {
                let mut t = ThingTemplate::new(&tmpl);
                t.set_health(100.0);
                if tmpl.to_ascii_lowercase().contains("tunnel")
                    || tmpl.to_ascii_lowercase().contains("network")
                {
                    t.add_kind_of(KindOf::Structure);
                }
                self.templates.insert(tmpl.clone(), t);
            }
            let Some(new_id) = self.create_object(&tmpl, team, pos) else {
                continue;
            };
            // C++ CreateObject Disposition=LIKE_EXISTING residual: copy pose.
            if let Some(dying) = self.objects.get(&dying_id) {
                let yaw = dying.get_orientation();
                if let Some(n) = self.objects.get_mut(&new_id) {
                    n.set_orientation(yaw);
                    n.producer_id = Some(dying_id);
                }
            }
            // FuelAir gas SlowDeath + HeightDie residual.
            if let Some(n) = self.objects.get_mut(&new_id) {
                n.ensure_fuel_air_gas_slow_death(self.frame);
                if n.fuel_air_gas_slow_death.is_some() {
                    self.fuel_air_gas_reg.record_install();
                }
            }
            if transfer && transfer_dmg > 0.0 {
                if let Some(n) = self.objects.get_mut(&new_id) {
                    let _ = n.take_damage_from_typed(
                        transfer_dmg,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                    );
                }
            }
        }
    }

    pub(in super::super) fn apply_fire_weapon_when_damaged_named(
        &mut self,
        source_id: ObjectId,
        weapon_name: &str,
    ) -> u32 {
        let (pos, team) = match self.objects.get(&source_id) {
            Some(o) => (o.get_position(), o.team),
            None => return 0,
        };
        let (pd, pr, sd, sr) =
            crate::game_logic::host_fire_weapon_when_damaged::fire_when_damaged_weapon_splash(
                weapon_name,
            );
        // Intended = self so splash doesn't skip others incorrectly... API skips intended_id.
        // Pass a dummy non-existent intended so all in radius can be hit except we should not hit self.
        // apply_instant_hit_splash_at skips intended_id only — use source as intended to skip self.
        self.apply_instant_hit_splash_at(
            pos,
            pd,
            sd,
            pr,
            sr,
            source_id,
            team,
            source_id,
            Some(weapon_name),
        )
    }

    pub(in super::super) fn try_begin_structure_topple_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let attacker_pos = {
            let src = self.objects.get(&id).and_then(|o| o.last_damage_source);
            src.and_then(|sid| {
                self.objects.get(&sid).map(|s| {
                    let p = s.get_position();
                    (p.x, p.z)
                })
            })
        };
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        // Already finished collapse or topple → allow normal destroy.
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        // Mid-animation: keep deferring destroy.
        if obj
            .structure_collapse_data
            .as_ref()
            .map(|d| d.is_active())
            .unwrap_or(false)
            || obj
                .structure_topple_data
                .as_ref()
                .map(|d| d.is_active())
                .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        // Prefer StructureCollapse for civilian/prop peels; else StructureTopple.
        if obj.begin_structure_collapse(frame) {
            let _ = killer;
            return true;
        }
        if obj.begin_structure_topple(frame, attacker_pos) {
            let _ = killer;
            return true;
        }
        false
    }
}

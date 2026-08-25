use super::super::super::*;

impl GameLogic {
    /// C++ PowerPlantUpdate::extendRods(true) residual — start rod animation timer.
    pub fn begin_power_plant_rods_extend(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        use crate::game_logic::host_special_power_completion_die::rods_extend_frames_for_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if obj.power_plant_rods_extended && obj.power_plant_rods_done_frame == 0 {
            // Already fully extended.
            return false;
        }
        if obj.power_plant_rods_done_frame > 0 {
            // Already extending.
            return false;
        }
        // Parsed `PowerPlantUpdate::RodsExtendTime` owns this value whenever
        // this object crossed the Object INI metadata boundary.  Keep the
        // older residual helper only for hand-authored templates used by
        // unrelated existing PowerPlantUpgrade paths.
        let frames = obj
            .thing
            .template
            .power_plant_update
            .map(|metadata| metadata.rods_extend_time_frames)
            .unwrap_or_else(|| rods_extend_frames_for_template(&obj.template_name));
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
            obj.model_condition_bits |= 1u128 << bit;
        }
        // Clear upgraded while animating.
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        obj.power_plant_rods_done_frame = self.frame.saturating_add(frames.max(1));
        obj.power_plant_rods_extended = true;
        self.special_power_completion_log.record_rods_start();
        true
    }

    /// C++ `PowerPlantUpdate::extendRods(false)` — retract immediately and
    /// clear both animation conditions.  Overcharge calls this only when the
    /// parsed object actually exposes the PowerPlantUpdate interface.
    pub fn retract_power_plant_rods(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
            obj.model_condition_bits &= !(1u128 << bit);
        }
        obj.power_plant_rods_extended = false;
        obj.power_plant_rods_done_frame = 0;
        true
    }

    /// C++ PowerPlantUpdate::update residual — finish rod extend.
    pub fn update_power_plant_rods(&mut self) {
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let now = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.power_plant_rods_done_frame > 0 && o.power_plant_rods_done_frame <= now
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
                    obj.model_condition_bits &= !(1u128 << bit);
                }
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                    obj.model_condition_bits |= 1u128 << bit;
                }
                obj.power_plant_rods_done_frame = 0;
                self.special_power_completion_log.record_rods_complete();
            }
        }
    }

    pub(in crate::game_logic) fn init_supply_warehouse_create(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_structure_economy_residual::starting_supplies_for_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        // Retail `SupplyWarehouseDockUpdate::StartingBoxes` is authoritative.
        // Retain the legacy bootstrap table only for hand-authored templates
        // that have no parsed Behavior metadata; it never grants Dock ability.
        let supplies = if obj.thing.template.dock_kind
            == crate::game_logic::DockKind::SupplyWarehouse
        {
            obj.thing.template.dock_starting_boxes.map(|boxes| {
                boxes.saturating_mul(
                    crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32,
                )
            })
        } else {
            starting_supplies_for_template(&obj.template_name)
        };
        let has_warehouse_create = obj.thing.template.has_supply_warehouse_create
            || obj.thing.template.dock_kind == crate::game_logic::DockKind::SupplyWarehouse;
        let had_supplies = supplies.is_some();
        if let Some(supplies) = supplies {
            // Only seed if empty (map may already set amount).
            if obj.stored_resources.supplies == 0 {
                obj.set_stored_supplies(supplies);
            }
        }
        if !has_warehouse_create && !had_supplies {
            return;
        }
        drop(obj);
        if has_warehouse_create || had_supplies {
            self.supply_create_warehouse_registers =
                self.supply_create_warehouse_registers.saturating_add(1);
        }
        if has_warehouse_create {
            for player in self.players.values_mut() {
                player.add_supply_warehouse(object_id);
            }
            if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                for player_arc in list.iter() {
                    let Ok(mut player_guard) = player_arc.write() else {
                        continue;
                    };
                    let Some(manager) = player_guard.get_resource_manager_mut() else {
                        continue;
                    };
                    manager.add_supply_warehouse(object_id.0);
                }
            }
        }
    }

    /// C++ SupplyCenterCreate::onBuildComplete residual.
    pub(in crate::game_logic) fn on_supply_center_build_complete(&mut self, object_id: ObjectId) {
        use crate::game_logic::host_upgrades::is_supply_center_template;
        let is_supply_center = self.objects.get(&object_id).is_some_and(|obj| {
            obj.thing.template.has_supply_center_create
                || obj.is_kind_of(KindOf::SupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || is_supply_center_template(&obj.template_name)
        });
        if is_supply_center {
            self.supply_create_center_registers =
                self.supply_create_center_registers.saturating_add(1);
            // C++ walks every player ResourceGatheringManager::addSupplyCenter.
            for player in self.players.values_mut() {
                player.add_supply_center(object_id);
            }
            if let Ok(list) = gamelogic::player::ThePlayerList().read() {
                for player_arc in list.iter() {
                    let Ok(mut player_guard) = player_arc.write() else {
                        continue;
                    };
                    let Some(manager) = player_guard.get_resource_manager_mut() else {
                        continue;
                    };
                    manager.add_supply_center(object_id.0);
                }
            }
            // C++ SupplyCenter/Stash SpawnBehavior ModuleTag_12 only becomes
            // eligible after UNDER_CONSTRUCTION clears.  It creates the free
            // starter collector outside the paid ProductionUpdate queue.
            let _ = self.spawn_supply_center_one_shot_collector(object_id);
        }
    }

    /// C++ GenerateMinefieldBehavior::upgradeImplementation + EMP swap residual.
    pub(in crate::game_logic) fn place_structure_minefield_for_upgrade(
        &mut self,
        object_id: ObjectId,
        upgrade: &str,
    ) -> u32 {
        self.apply_structure_minefield_upgrade(object_id, upgrade)
    }

    pub(in crate::game_logic) fn init_starts_paused_special_powers(&mut self, object_id: ObjectId) {
        use crate::command_system::SpecialPowerType as P;
        use crate::game_logic::host_upgrade_module_residuals::power_starts_paused;
        // C++ SpecialPowerModule starts the authored ReloadTime on creation
        // before applying StartsPaused.  HDB is not covered by the old
        // handwritten special-power table, so retain the paired Object INI
        // metadata here rather than falling through to a Hacker name.
        let hacker_disable = self
            .objects
            .get(&object_id)
            .and_then(|obj| obj.thing.template.hacker_disable_building.clone());
        let handled_hdb = hacker_disable.is_some();
        if let Some(metadata) = hacker_disable {
            let owner_id = self
                .objects
                .get(&object_id)
                .and_then(|obj| self.player_owner_for_host_object(obj));
            if metadata.shared_n_sync {
                // SharedNSync belongs to the exact controller, never the
                // first player selected by faction.  Missing/stale ownership
                // remains unavailable through the typed readiness gate.
                if let Some(owner_id) = owner_id {
                    if let Some(player) = self.get_player_mut(owner_id) {
                        player.reset_shared_special_power_timer(
                            &P::HackerDisableBuilding,
                            metadata.reload_time_frames as f32 / 30.0,
                        );
                    }
                }
            } else if let Some(object) = self.objects.get_mut(&object_id) {
                if !object.status.under_construction {
                    object.start_power_recharge_with_frames(
                        &P::HackerDisableBuilding,
                        metadata.reload_time_frames,
                    );
                }
            }
            if metadata.starts_paused {
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.pause_special_power_countdown(&P::HackerDisableBuilding, true);
                }
            }
        }
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return;
        };
        // C++ SpecialPowerModule ctor: startPowerRecharge (pre-built, non-SharedNSync)
        // then pauseCountdown(TRUE) once when StartsPaused. Pause is a refcount —
        // a second pause here or in SpecialPowerCreate leaves the upgrade unpause
        // one short. Nuke Helix maps to HelixNukeBomb, not HelixNapalmBomb.
        let under_construction = obj.status.under_construction;
        let mut planned: Vec<(P, Option<u32>, bool)> = Vec::new();
        let already = |planned: &[(P, Option<u32>, bool)], power: &P| {
            planned.iter().any(|(existing, _, _)| existing == power)
        };

        for module in &obj.thing.template.special_power_modules {
            if !module.starts_paused {
                continue;
            }
            let Some(power) = module.command_power.clone() else {
                continue;
            };
            if handled_hdb && matches!(power, P::HackerDisableBuilding) {
                continue;
            }
            if already(&planned, &power) {
                continue;
            }
            planned.push((power, Some(module.reload_time_frames), module.shared_n_sync));
        }

        if obj.thing.template.capture_starts_paused {
            if let Some(power) = obj.thing.template.capture_power.special_power_type() {
                if !already(&planned, &power) {
                    planned.push((power, None, false));
                }
            }
        }

        let name = obj.template_name.to_ascii_lowercase();
        if power_starts_paused(&P::RadarScan)
            && (name.contains("radarvan") || name.contains("radar_van"))
            && !already(&planned, &P::RadarScan)
        {
            planned.push((P::RadarScan, None, false));
        }
        let helix_bomb_planned =
            already(&planned, &P::HelixNapalmBomb) || already(&planned, &P::HelixNukeBomb);
        if name.contains("helix") && !helix_bomb_planned {
            let power = if name.contains("nuke") {
                P::HelixNukeBomb
            } else {
                P::HelixNapalmBomb
            };
            if power_starts_paused(&power) {
                planned.push((power, None, false));
            }
        }

        for (power, frames, shared_n_sync) in planned {
            if !under_construction && !shared_n_sync {
                match frames {
                    Some(frames) => obj.start_power_recharge_with_frames(&power, frames),
                    None => obj.start_power_recharge(&power),
                }
            }
            obj.pause_special_power_countdown(&power, true);
        }
    }

    /// C++ `ThingTemplate::calcCostToBuild` player modifier path.
    ///
    /// The exact `PlayerTemplate::ProductionCostChange` comes first, then the
    /// independently stacked KindOf upgrade modifiers.  The old host helper
    /// returned early for an unclassified template, which incorrectly skipped
    /// a General's exact-name discount even though C++ applies it before the
    /// KindOf query.
    pub fn modified_build_cost_supplies(
        &self,
        player_id: u32,
        template_name: &str,
        base_supplies: u32,
    ) -> u32 {
        use crate::game_logic::host_upgrade_module_residuals::{
            apply_production_cost_factor, kindof_cost_tokens,
        };
        let Some(player) = self.players.get(&player_id) else {
            return base_supplies;
        };
        let (is_vehicle, is_infantry, is_aircraft, is_structure) = self
            .templates
            .get(template_name)
            .map(|t| {
                (
                    t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                    t.is_kind_of(crate::game_logic::KindOf::Infantry),
                    t.is_kind_of(crate::game_logic::KindOf::Aircraft),
                    t.is_kind_of(crate::game_logic::KindOf::Structure),
                )
            })
            .unwrap_or((false, false, false, false));
        let tokens = kindof_cost_tokens(is_vehicle, is_infantry, is_aircraft, is_structure);
        let kindof_factor = player.production_cost_factor(&tokens);
        let template_factor = self.player_template_production_cost_factor(player_id, template_name);
        let handicap = player.handicap_build_cost_multiplier(is_structure);
        apply_production_cost_factor(base_supplies, template_factor * kindof_factor * handicap)
    }

    /// C++ `ThingTemplate::calcTimeToBuild` authored pre-power frame count.
    ///
    /// Retail first converts `getBuildTime() * 30` to `Int`, then applies the
    /// selected PlayerTemplate's `ProductionTimeChange` and converts to `Int`
    /// again.  Keep this integer form available to both queued production and
    /// dozer construction, before either path applies the low-power penalty.
    pub(crate) fn cpp_build_time_frames_from_factor(base_seconds: f32, factor: f32) -> u32 {
        const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;

        if !base_seconds.is_finite() || !factor.is_finite() {
            // Invalid INI values must not manufacture a near-instant unit.
            return u32::MAX;
        }

        let base_frames = (base_seconds * LOGIC_FRAMES_PER_SECOND)
            .trunc()
            .clamp(0.0, u32::MAX as f32) as u32;
        ((base_frames as f32) * factor.max(0.0))
            .trunc()
            .clamp(0.0, u32::MAX as f32) as u32
    }

    /// Encode C++ `ThingTemplate::calcTimeToBuild`'s authored pre-power frame
    /// count in Main's legacy seconds carrier.
    ///
    /// Retail first converts `getBuildTime() * 30` to `Int`, then applies the
    /// selected PlayerTemplate's `ProductionTimeChange` and converts to `Int`
    /// again.  Main's queue stores seconds but its existing completion code
    /// recovers an integer frame count before applying the low-power penalty.
    /// Preserve that ordering by encoding the already-truncated pre-power
    /// frame count just above its lower frame boundary.  A direct
    /// `frames as f32 / 30.0` can round below that boundary (for example frame
    /// 63), causing the downstream `.trunc()` to lose a frame.
    pub(crate) fn cpp_build_time_seconds_from_factor(base_seconds: f32, factor: f32) -> f32 {
        const LOGIC_FRAMES_PER_SECOND: f32 = 30.0;
        const FRAME_ENCODING_FRACTION: f32 = 0.25;
        let authored_frames = Self::cpp_build_time_frames_from_factor(base_seconds, factor);
        if authored_frames == 0 {
            return 0.0;
        }

        (authored_frames as f32 + FRAME_ENCODING_FRACTION) / LOGIC_FRAMES_PER_SECOND
    }

    /// C++ `ThingTemplate::calcTimeToBuild` authored PlayerTemplate stage.
    ///
    /// This returns a seconds carrier for `ProductionItem`; its existing
    /// logic-frame completion path then applies the C++ low-energy penalty.
    pub(crate) fn modified_build_time_seconds(
        &self,
        player_id: u32,
        template_name: &str,
        base_seconds: f32,
    ) -> f32 {
        let is_structure = self
            .templates
            .get(template_name)
            .map(|t| t.is_kind_of(crate::game_logic::KindOf::Structure))
            .unwrap_or(false);
        let handicap = self
            .players
            .get(&player_id)
            .map(|p| p.handicap_build_time_multiplier(is_structure))
            .unwrap_or(1.0);
        let factor =
            self.player_template_production_time_factor(player_id, template_name) * handicap;
        Self::cpp_build_time_seconds_from_factor(base_seconds, factor)
    }
}

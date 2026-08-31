use super::*;

// =========================================================
// Snapshotable Implementation (save/load and CRC)
// C++ Reference: Player.cpp lines 3936-4526
// =========================================================

impl Snapshotable for Player {
    /// CRC computation for network synchronization.
    /// C++ Reference: Player::crc() lines 3939-3960
    ///
    /// C++ xfers:
    ///   1. xferBool(battlePlanBonus) - whether BattlePlanBonuses is present
    ///   2. IF present: xferReal(armorScalar), xferReal(sightRangeScalar),
    ///      xferInt(bombardment), xferInt(holdTheLine), xferInt(searchAndDestroy),
    ///      kindOf.xfer(validKindOf), kindOf.xfer(invalidKindOf)
    ///   3. xferInt(skillPoints)
    ///   4. xferInt(sciencePurchasePoints)
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Player::crc (Player.cpp:3941-3955): present flag, then bonus fields if non-NULL.
        let mut battle_plan_bonus = self.battle_plan_bonuses.is_some();
        xfer.xfer_bool(&mut battle_plan_bonus)
            .map_err(|e| format!("CRC battle_plan_bonus failed: {}", e))?;
        if let Some(bonuses) = &self.battle_plan_bonuses {
            let mut armor_scalar = bonuses.armor_scalar;
            let mut sight_range_scalar = bonuses.sight_range_scalar;
            let mut bombardment = bonuses.bombardment;
            let mut hold_the_line = bonuses.hold_the_line;
            let mut search_and_destroy = bonuses.search_and_destroy;
            let mut valid_kind = bonuses.valid_kind_of;
            let mut invalid_kind = bonuses.invalid_kind_of;
            xfer.xfer_real(&mut armor_scalar)
                .map_err(|e| format!("CRC armor_scalar failed: {}", e))?;
            xfer.xfer_real(&mut sight_range_scalar)
                .map_err(|e| format!("CRC sight_range_scalar failed: {}", e))?;
            xfer.xfer_int(&mut bombardment)
                .map_err(|e| format!("CRC bombardment failed: {}", e))?;
            xfer.xfer_int(&mut hold_the_line)
                .map_err(|e| format!("CRC hold_the_line failed: {}", e))?;
            xfer.xfer_int(&mut search_and_destroy)
                .map_err(|e| format!("CRC search_and_destroy failed: {}", e))?;
            xfer_kind_of_mask(xfer, &mut valid_kind)?;
            xfer_kind_of_mask(xfer, &mut invalid_kind)?;
        }

        // Skill points
        let mut skill_points = self.skill_points;
        xfer.xfer_int(&mut skill_points)
            .map_err(|e| format!("CRC skill_points failed: {}", e))?;

        // Science purchase points
        let mut science_purchase_points = self.science_purchase_points;
        xfer.xfer_int(&mut science_purchase_points)
            .map_err(|e| format!("CRC science_purchase_points failed: {}", e))?;

        Ok(())
    }

    /// Save/load player state.
    /// C++ Reference: Player::xfer() lines 3975-4516
    ///
    /// Version History:
    ///   1: Initial version
    ///   2: Player can now have a modifier on his skill points (multiplicative)
    ///   3: Player can be excluded from the score screen via script.
    ///   4: Player stores a list of specialpowerreadyframe timers
    ///   5: Sciences use xferScienceVec
    ///   6: Store m_unitsShouldHunt
    ///   7: Added Preorder flag
    ///   8: Save m_disabledSciences & m_hiddenSciences
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 8;
        let mut version = CURRENT_VERSION;

        // --- 1. Version ---
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("xfer_version failed: {}", e))?;

        // --- 2. Money xferSnapshot ---
        // C++ line 3984: xfer->xferSnapshot(&m_money)
        // Money has its own Snapshotable xfer (version + u32 money value)
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                let money_data = self.money.xfer_save();
                // Money xfer is: version byte (1) + 4 bytes money value = 5 bytes raw
                                // SAFETY: money_data is an owned Vec of exactly the serialized
                                // length; xfer_user reads it without writing.
                unsafe {
                    xfer.xfer_user(money_data.as_ptr() as *mut u8, money_data.len())
                        .map_err(|e| format!("money xfer_user failed: {}", e))?;
                }
            }
            XferMode::Load => {
                // Money xfer starts with version byte, then u32 money value (5 bytes total)
                let mut money_data = vec![0u8; 5];
                                // SAFETY: money_data is an owned 5-byte Vec; xfer_user writes
                                // exactly its length into it before xfer_load parses a copy.
                unsafe {
                    xfer.xfer_user(money_data.as_mut_ptr(), money_data.len())
                        .map_err(|e| format!("money xfer_user load failed: {}", e))?;
                }
                self.money
                    .xfer_load(&money_data)
                    .map_err(|e| e.to_string())?;
            }
            XferMode::Crc => {
                let money_data = self.money.xfer_save();
                                // SAFETY: owned Vec, CRC path only reads the exact length.
                unsafe {
                    xfer.xfer_user(money_data.as_ptr() as *mut u8, money_data.len())
                        .map_err(|e| format!("money crc failed: {}", e))?;
                }
            }
            _ => {}
        }

        // --- 3. Upgrade list count ---
        // C++ lines 3987-3991
        let mut upgrade_count = self.upgrade_list.len() as u16;
        xfer.xfer_unsigned_short(&mut upgrade_count)
            .map_err(|e| format!("upgrade_count xfer failed: {}", e))?;

        // --- 4. Version >= 7: Preorder ---
        // C++ lines 3993-3997
        if version >= 7 {
            xfer.xfer_bool(&mut self.is_preorder)
                .map_err(|e| format!("is_preorder xfer failed: {}", e))?;
        }

        // --- 5. Version >= 8: Disabled/Hidden sciences ---
        // C++ lines 3999-4003: xferScienceVec(&m_sciencesDisabled), xferScienceVec(&m_sciencesHidden)
        if version >= 8 {
            // Convert HashSet to Vec for xfer_science_vec
            let mut disabled_vec: Vec<ScienceType> =
                self.sciences_disabled.iter().copied().collect();
            let mut hidden_vec: Vec<ScienceType> = self.sciences_hidden.iter().copied().collect();

            xfer.xfer_science_vec(&mut disabled_vec)
                .map_err(|e| format!("sciences_disabled xfer failed: {}", e))?;
            xfer.xfer_science_vec(&mut hidden_vec)
                .map_err(|e| format!("sciences_hidden xfer failed: {}", e))?;

            if matches!(xfer.get_xfer_mode(), XferMode::Load) {
                self.sciences_disabled = disabled_vec.into_iter().collect();
                self.sciences_hidden = hidden_vec.into_iter().collect();
            }
        }

        // --- 6. Upgrade instances: name + xferSnapshot ---
        // C++ lines 4005-4053
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for upgrade in &self.upgrade_list {
                    // C++ Player.cpp:4014-4018 — name + Upgrade::xfer (version + status only).
                    let mut name = upgrade.get_name().to_string();
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| format!("upgrade name xfer failed: {}", e))?;
                    let mut status = upgrade.get_status();
                    xfer_upgrade_instance(xfer, &mut status)?;
                }
            }
            XferMode::Load => {
                self.upgrade_list.clear();
                for _ in 0..upgrade_count {
                    let mut name = String::new();
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| format!("load upgrade name failed: {}", e))?;
                    let mut status = UpgradeStatus::Pending;
                    xfer_upgrade_instance(xfer, &mut status)?;
                    let mut upgrade = UpgradeInfo::new(name);
                    upgrade.set_status(status);
                    self.upgrade_list.push(upgrade);
                }
            }
            _ => {}
        }

        // --- 7. Radar info ---
        // C++ lines 4055-4059
        xfer.xfer_int(&mut self.radar_count)
            .map_err(|e| format!("radar_count xfer failed: {}", e))?;
        // --- 8. Is player dead ---
        xfer.xfer_bool(&mut self.is_player_dead)
            .map_err(|e| format!("is_player_dead xfer failed: {}", e))?;
        // --- 9. Disable proof radar count ---
        xfer.xfer_int(&mut self.disable_proof_radar_count)
            .map_err(|e| format!("disable_proof_radar_count xfer failed: {}", e))?;
        // --- 10. Radar disabled ---
        xfer.xfer_bool(&mut self.radar_disabled)
            .map_err(|e| format!("radar_disabled xfer failed: {}", e))?;

        // --- 11. Upgrades in progress ---
        // C++ line 4062: xfer->xferUpgradeMask(&m_upgradesInProgress)
        let mut upgrades_in_progress_mask = self.upgrades_in_progress;
        xfer.xfer_upgrade_mask(&mut upgrades_in_progress_mask)
            .map_err(|e| format!("upgrades_in_progress xfer failed: {}", e))?;

        // --- 12. Upgrades completed ---
        // C++ line 4065: xfer->xferUpgradeMask(&m_upgradesCompleted)
        let mut upgrades_completed_mask = self.upgrades_completed;
        xfer.xfer_upgrade_mask(&mut upgrades_completed_mask)
            .map_err(|e| format!("upgrades_completed xfer failed: {}", e))?;

        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.upgrades_in_progress = upgrades_in_progress_mask;
            self.upgrades_completed = upgrades_completed_mask;
        }

        // --- 13. Energy xferSnapshot ---
        // C++ line 4068: xfer->xferSnapshot(&m_energy)
        // Energy has its own xfer method (version 3) matching C++ Energy::xfer
        self.energy.xfer(xfer);

        // --- 14. Team prototypes ---
        // C++ lines 4074-4122: prototype count + xferUser raw TeamPrototypeID for each
        let mut prototype_count = self.team_prototypes.len() as u16;
        xfer.xfer_unsigned_short(&mut prototype_count)
            .map_err(|e| format!("prototype_count xfer failed: {}", e))?;
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // C++ writes raw TeamPrototypeID bytes via xferUser
                // Since we store names, write dummy 4-byte IDs (will be resolved by TeamFactory on load)
                for _ in 0..prototype_count {
                    let mut dummy_id: u32 = 0;
                                        // SAFETY: &mut dummy_id is a valid aligned u32 for the
                                        // sizeof(u32) raw transfer matching C++ xferUser.
                    unsafe {
                        xfer.xfer_user(
                            &mut dummy_id as *mut u32 as *mut u8,
                            std::mem::size_of::<u32>(),
                        )
                        .map_err(|e| format!("prototype id xfer failed: {}", e))?;
                    }
                }
            }
            XferMode::Load => {
                self.team_prototypes.clear();
                for _ in 0..prototype_count {
                    let mut dummy_id: u32 = 0;
                                        // SAFETY: &mut dummy_id is a valid aligned u32; loaded
                                        // value is consumed after the call.
                    unsafe {
                        xfer.xfer_user(
                            &mut dummy_id as *mut u32 as *mut u8,
                            std::mem::size_of::<u32>(),
                        )
                        .map_err(|e| format!("load prototype id failed: {}", e))?;
                    }
                    // In C++, this resolves via TheTeamFactory->findTeamPrototypeByID
                    // Store as string representation since we don't have team factory
                    self.team_prototypes
                        .push(format!("team_proto_{}", dummy_id));
                }
            }
            _ => {}
        }

        // --- 15. Build list info ---
        // C++ lines 4124-4176: buildListInfoCount + xferSnapshot for each
        let mut build_list_info_count = 0u16;
        let mut current: Option<&BuildListInfo> = self.build_list.as_deref();
        while let Some(info) = current {
            build_list_info_count += 1;
            current = info.get_next();
        }
        xfer.xfer_unsigned_short(&mut build_list_info_count)
            .map_err(|e| format!("build_list_info_count xfer failed: {}", e))?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                current = self.build_list.as_deref();
                while let Some(info) = current {
                    // BuildListInfo xferSnapshot - inline serialization
                    // C++ BuildListInfo::xfer version 2.
                    const BUILD_LIST_VERSION: XferVersion = 2;
                    let mut bl_version = BUILD_LIST_VERSION;
                    xfer.xfer_version(&mut bl_version, BUILD_LIST_VERSION)
                        .map_err(|e| format!("build_list version failed: {}", e))?;
                    let mut building_name = info.get_building_name().to_string();
                    xfer.xfer_ascii_string(&mut building_name)
                        .map_err(|e| format!("build_list building_name failed: {}", e))?;
                    let mut name = info.get_template_name().to_string();
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| format!("build_list name failed: {}", e))?;
                    let mut location_x = info.get_location().x;
                    let mut location_y = info.get_location().y;
                    let mut location_z = info.get_location().z;
                    xfer.xfer_real(&mut location_x)
                        .map_err(|e| format!("build_list location_x failed: {}", e))?;
                    xfer.xfer_real(&mut location_y)
                        .map_err(|e| format!("build_list location_y failed: {}", e))?;
                    xfer.xfer_real(&mut location_z)
                        .map_err(|e| format!("build_list location_z failed: {}", e))?;
                    let mut rally_offset = *info.get_rally_offset();
                    xfer.xfer_coord_2d(&mut rally_offset)
                        .map_err(|e| format!("build_list rally_offset failed: {}", e))?;
                    let mut angle = info.get_angle();
                    xfer.xfer_real(&mut angle)
                        .map_err(|e| format!("build_list angle failed: {}", e))?;
                    let mut initially_built = info.is_initially_built();
                    xfer.xfer_bool(&mut initially_built)
                        .map_err(|e| format!("build_list initially_built failed: {}", e))?;
                    let mut num_rebuilds = info.get_num_rebuilds();
                    xfer.xfer_unsigned_int(&mut num_rebuilds)
                        .map_err(|e| format!("build_list num_rebuilds failed: {}", e))?;
                    let mut script = info.get_script().to_string();
                    xfer.xfer_ascii_string(&mut script)
                        .map_err(|e| format!("build_list script failed: {}", e))?;
                    let mut health = info.get_health();
                    xfer.xfer_int(&mut health)
                        .map_err(|e| format!("build_list health failed: {}", e))?;
                    let mut whiner = info.get_whiner();
                    xfer.xfer_bool(&mut whiner)
                        .map_err(|e| format!("build_list whiner failed: {}", e))?;
                    let mut unsellable = info.get_unsellable();
                    xfer.xfer_bool(&mut unsellable)
                        .map_err(|e| format!("build_list unsellable failed: {}", e))?;
                    let mut repairable = info.get_repairable();
                    xfer.xfer_bool(&mut repairable)
                        .map_err(|e| format!("build_list repairable failed: {}", e))?;
                    let mut automatically_build = info.is_automatic_build();
                    xfer.xfer_bool(&mut automatically_build)
                        .map_err(|e| format!("build_list automatically_build failed: {}", e))?;
                    let mut object_id = info.get_object_id();
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| format!("build_list object_id failed: {}", e))?;
                    let mut timestamp = info.get_object_timestamp();
                    xfer.xfer_unsigned_int(&mut timestamp)
                        .map_err(|e| format!("build_list timestamp failed: {}", e))?;
                    let mut under_construction = info.is_under_construction();
                    xfer.xfer_bool(&mut under_construction)
                        .map_err(|e| format!("build_list under_construction failed: {}", e))?;
                    for index in 0..MAX_BUILD_LIST_RESOURCE_GATHERERS {
                        let mut gatherer_id = info.get_gatherer_id(index);
                        xfer.xfer_object_id(&mut gatherer_id)
                            .map_err(|e| format!("build_list gatherer failed: {}", e))?;
                    }
                    let mut supply_building = info.is_supply_building();
                    xfer.xfer_bool(&mut supply_building)
                        .map_err(|e| format!("build_list supply_building failed: {}", e))?;
                    let mut desired_gatherers = info.get_desired_gatherers();
                    xfer.xfer_int(&mut desired_gatherers)
                        .map_err(|e| format!("build_list desired_gatherers failed: {}", e))?;
                    let mut priority = info.is_priority_build();
                    xfer.xfer_bool(&mut priority)
                        .map_err(|e| format!("build_list priority failed: {}", e))?;
                    let mut current_gatherers = info.get_current_gatherers();
                    xfer.xfer_int(&mut current_gatherers)
                        .map_err(|e| format!("build_list current_gatherers failed: {}", e))?;
                    current = info.get_next();
                }
            }
            XferMode::Load => {
                // C++ lines 4145-4147: destroy existing build list
                self.build_list = None;
                for _ in 0..build_list_info_count {
                    const BUILD_LIST_VERSION: XferVersion = 2;
                    let mut bl_version = BUILD_LIST_VERSION;
                    xfer.xfer_version(&mut bl_version, BUILD_LIST_VERSION)
                        .map_err(|e| format!("load build_list version failed: {}", e))?;
                    let mut building_name = String::new();
                    xfer.xfer_ascii_string(&mut building_name)
                        .map_err(|e| format!("load build_list building_name failed: {}", e))?;
                    let mut name = String::new();
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| format!("load build_list name failed: {}", e))?;
                    let mut location_x = 0.0f32;
                    let mut location_y = 0.0f32;
                    let mut location_z = 0.0f32;
                    xfer.xfer_real(&mut location_x)
                        .map_err(|e| format!("load build_list location_x failed: {}", e))?;
                    xfer.xfer_real(&mut location_y)
                        .map_err(|e| format!("load build_list location_y failed: {}", e))?;
                    xfer.xfer_real(&mut location_z)
                        .map_err(|e| format!("load build_list location_z failed: {}", e))?;
                    let location = Coord3D::new(location_x, location_y, location_z);
                    let mut rally_offset = Point2D::new(0.0, 0.0);
                    xfer.xfer_coord_2d(&mut rally_offset)
                        .map_err(|e| format!("load build_list rally_offset failed: {}", e))?;
                    let mut angle = 0.0f32;
                    xfer.xfer_real(&mut angle)
                        .map_err(|e| format!("load build_list angle failed: {}", e))?;
                    let mut initially_built = false;
                    xfer.xfer_bool(&mut initially_built)
                        .map_err(|e| format!("load build_list initially_built failed: {}", e))?;
                    let mut num_rebuilds = 0u32;
                    xfer.xfer_unsigned_int(&mut num_rebuilds)
                        .map_err(|e| format!("load build_list num_rebuilds failed: {}", e))?;
                    let mut script = String::new();
                    xfer.xfer_ascii_string(&mut script)
                        .map_err(|e| format!("load build_list script failed: {}", e))?;
                    let mut health = 100;
                    xfer.xfer_int(&mut health)
                        .map_err(|e| format!("load build_list health failed: {}", e))?;
                    let mut whiner = true;
                    xfer.xfer_bool(&mut whiner)
                        .map_err(|e| format!("load build_list whiner failed: {}", e))?;
                    let mut unsellable = false;
                    xfer.xfer_bool(&mut unsellable)
                        .map_err(|e| format!("load build_list unsellable failed: {}", e))?;
                    let mut repairable = true;
                    xfer.xfer_bool(&mut repairable)
                        .map_err(|e| format!("load build_list repairable failed: {}", e))?;
                    let mut automatically_build = true;
                    xfer.xfer_bool(&mut automatically_build).map_err(|e| {
                        format!("load build_list automatically_build failed: {}", e)
                    })?;
                    let mut object_id = 0u32;
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| format!("load build_list object_id failed: {}", e))?;
                    let mut timestamp = 0u32;
                    xfer.xfer_unsigned_int(&mut timestamp)
                        .map_err(|e| format!("load build_list timestamp failed: {}", e))?;
                    let mut under_construction = false;
                    xfer.xfer_bool(&mut under_construction)
                        .map_err(|e| format!("load build_list under_construction failed: {}", e))?;
                    let mut resource_gatherers =
                        [INVALID_OBJECT_ID; MAX_BUILD_LIST_RESOURCE_GATHERERS];
                    for gatherer_id in &mut resource_gatherers {
                        xfer.xfer_object_id(gatherer_id)
                            .map_err(|e| format!("load build_list gatherer failed: {}", e))?;
                    }
                    let mut supply_building = false;
                    xfer.xfer_bool(&mut supply_building)
                        .map_err(|e| format!("load build_list supply_building failed: {}", e))?;
                    let mut desired_gatherers = 0;
                    xfer.xfer_int(&mut desired_gatherers)
                        .map_err(|e| format!("load build_list desired_gatherers failed: {}", e))?;
                    let mut priority = false;
                    xfer.xfer_bool(&mut priority)
                        .map_err(|e| format!("load build_list priority failed: {}", e))?;
                    let mut current_gatherers = 0;
                    if bl_version >= 2 {
                        xfer.xfer_int(&mut current_gatherers).map_err(|e| {
                            format!("load build_list current_gatherers failed: {}", e)
                        })?;
                    }

                    // Attach to end of list (matching C++ behavior)
                    let mut info = Box::new(BuildListInfo::new(name, location, angle));
                    info.set_building_name(building_name);
                    info.set_rally_offset(rally_offset);
                    info.set_initially_built(initially_built);
                    info.set_object_id(object_id);
                    info.set_num_rebuilds(num_rebuilds);
                    info.set_script(script);
                    info.set_health(health);
                    info.set_whiner(whiner);
                    info.set_unsellable(unsellable);
                    info.set_repairable(repairable);
                    info.set_automatic_build(automatically_build);
                    info.set_object_timestamp(timestamp);
                    info.resource_gatherers = resource_gatherers;
                    info.set_supply_building(supply_building);
                    info.set_desired_gatherers(desired_gatherers);
                    info.set_current_gatherers(current_gatherers);
                    if priority {
                        info.mark_priority_build();
                    }
                    info.set_under_construction(under_construction);

                    if self.build_list.is_none() {
                        self.build_list = Some(info);
                    } else {
                        // Walk to end and append
                        let mut last = self.build_list.as_deref_mut().unwrap();
                        while last.get_next().is_some() {
                            last = last.get_next_mut().unwrap();
                        }
                        last.set_next(Some(info));
                    }
                }
            }
            _ => {}
        }

        // --- 16. AI player data ---
        // C++ lines 4178-4189: xferBool(aiPlayerPresent), if present xferSnapshot(&m_ai)
        let mut ai_player_present = self.ai.is_some();
        xfer.xfer_bool(&mut ai_player_present)
            .map_err(|e| format!("ai_player_present xfer failed: {}", e))?;
        // Note: AI xferSnapshot requires AIPlayer Snapshotable impl.
        // When AI xfer is implemented, call it here if ai_player_present is true.

        // --- 17. Resource gathering manager ---
        // C++ lines 4191-4203: xferBool(rgmPresent), if present xferSnapshot(&m_resourceGatheringManager)
        let mut rgm_present = !self.supply_centers.is_empty() || !self.supply_warehouses.is_empty();
        xfer.xfer_bool(&mut rgm_present)
            .map_err(|e| format!("rgm_present xfer failed: {}", e))?;
        if rgm_present {
            let mut rgm_version: XferVersion = 1;
            xfer.xfer_version(&mut rgm_version, 1)
                .map_err(|e| format!("resource gathering manager version xfer failed: {}", e))?;

            let mut warehouses: Vec<u32> = match xfer.get_xfer_mode() {
                XferMode::Load => Vec::new(),
                _ => self.supply_warehouses.clone(),
            };
            let mut centers: Vec<u32> = match xfer.get_xfer_mode() {
                XferMode::Load => Vec::new(),
                _ => self.supply_centers.clone(),
            };

            xfer.xfer_vec_unsigned_int(&mut warehouses)
                .map_err(|e| format!("resource gathering warehouses xfer failed: {}", e))?;
            xfer.xfer_vec_unsigned_int(&mut centers)
                .map_err(|e| format!("resource gathering centers xfer failed: {}", e))?;

            if matches!(xfer.get_xfer_mode(), XferMode::Load) {
                self.supply_warehouses = warehouses;
                self.supply_centers = centers;
            }
        } else if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.supply_warehouses.clear();
            self.supply_centers.clear();
        }

        // --- 18. Tunnel tracking system ---
        // C++ lines 4205-4217: xferBool(tunnelTrackerPresent), if present xferSnapshot(&m_tunnelSystem)
        let mut tunnel_present = !self.tunnel_entrances.is_empty();
        xfer.xfer_bool(&mut tunnel_present)
            .map_err(|e| format!("tunnel_present xfer failed: {}", e))?;
        // Note: TunnelTracker xferSnapshot requires its Snapshotable impl.
        // When tunnel xfer is implemented, call it here if tunnel_present is true.

        // --- 19. Default team ---
        // C++ lines 4219-4223: xferUser(&teamID, sizeof(TeamID))
        let mut team_id = self.default_team.unwrap_or(0);
                // SAFETY: &mut team_id is a valid aligned u32 for the sizeof(u32)
                // transfer mirroring C++ xferUser(&teamID, sizeof(TeamID)).
        unsafe {
            xfer.xfer_user(
                &mut team_id as *mut u32 as *mut u8,
                std::mem::size_of::<u32>(),
            )
            .map_err(|e| format!("default_team xfer failed: {}", e))?;
        }
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.default_team = if team_id != 0 { Some(team_id) } else { None };
        }

        // --- 20. Sciences ---
        // C++ lines 4225-4266: version >= 5 uses xferScienceVec, else old format
        if version >= 5 {
            // Convert HashSet to Vec for xfer_science_vec
            let mut sciences_vec: Vec<ScienceType> = self.sciences.iter().copied().collect();
            if matches!(xfer.get_xfer_mode(), XferMode::Load) {
                sciences_vec.clear();
            }
            xfer.xfer_science_vec(&mut sciences_vec)
                .map_err(|e| format!("sciences xfer failed: {}", e))?;
            if matches!(xfer.get_xfer_mode(), XferMode::Load) {
                self.sciences = sciences_vec.into_iter().collect();
            }
        } else {
            // Old format (version < 5): count + raw ScienceType bytes
            let mut science_count = self.sciences.len() as u16;
            xfer.xfer_unsigned_short(&mut science_count)
                .map_err(|e| format!("science_count xfer failed: {}", e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save => {
                    for &science in &self.sciences {
                        let mut sci = science;
                                                // SAFETY: &mut sci is a live i32 (ScienceType) sized
                                                // by size_of::<ScienceType>() for this transfer.
                        unsafe {
                            xfer.xfer_user(
                                &mut sci as *mut i32 as *mut u8,
                                std::mem::size_of::<ScienceType>(),
                            )
                            .map_err(|e| format!("science xfer failed: {}", e))?;
                        }
                    }
                }
                XferMode::Load => {
                    self.sciences.clear();
                    for _ in 0..science_count {
                        let mut science: ScienceType = 0;
                                                // SAFETY: &mut science is a live i32 initialized to
                                                // SCIENCE_INVALID and overwritten by the load.
                        unsafe {
                            xfer.xfer_user(
                                &mut science as *mut i32 as *mut u8,
                                std::mem::size_of::<ScienceType>(),
                            )
                            .map_err(|e| format!("load science failed: {}", e))?;
                        }
                        self.sciences.insert(science);
                    }
                }
                _ => {}
            }
        }

        // --- 21. Rank level ---
        // C++ line 4269
        xfer.xfer_int(&mut self.rank_level)
            .map_err(|e| format!("rank_level xfer failed: {}", e))?;

        // --- 22. Skill points ---
        // C++ line 4272
        xfer.xfer_int(&mut self.skill_points)
            .map_err(|e| format!("skill_points xfer failed: {}", e))?;

        // --- 23. Science purchase points ---
        // C++ line 4275
        xfer.xfer_int(&mut self.science_purchase_points)
            .map_err(|e| format!("science_purchase_points xfer failed: {}", e))?;

        // --- 24. Level up ---
        // C++ line 4278
        xfer.xfer_int(&mut self.level_up)
            .map_err(|e| format!("level_up xfer failed: {}", e))?;

        // --- 25. Level down ---
        // C++ line 4281
        xfer.xfer_int(&mut self.level_down)
            .map_err(|e| format!("level_down xfer failed: {}", e))?;

        // --- 26. General name (UNICODE string) ---
        // C++ line 4284: xfer->xferUnicodeString(&m_generalName)
        xfer.xfer_unicode_string(&mut self.general_name)
            .map_err(|e| format!("general_name xfer failed: {}", e))?;

        // --- 27. Player relations ---
        // C++ line 4287: xfer->xferSnapshot(m_playerRelations)
        self.player_relations
            .xfer(xfer)
            .map_err(|e| format!("player_relations xfer failed: {}", e))?;

        // --- 28. Team relations ---
        // C++ line 4290: xfer->xferSnapshot(m_teamRelations)
        self.team_relations
            .xfer(xfer)
            .map_err(|e| format!("team_relations xfer failed: {}", e))?;

        // --- 29. Can build units ---
        // C++ line 4293
        xfer.xfer_bool(&mut self.can_build_units)
            .map_err(|e| format!("can_build_units xfer failed: {}", e))?;

        // --- 30. Can build base ---
        // C++ line 4296
        xfer.xfer_bool(&mut self.can_build_base)
            .map_err(|e| format!("can_build_base xfer failed: {}", e))?;

        // --- 31. Observer ---
        // C++ line 4299
        xfer.xfer_bool(&mut self.observer)
            .map_err(|e| format!("observer xfer failed: {}", e))?;

        // --- 32. Version >= 2: Skill points modifier ---
        // C++ lines 4301-4309
        if version >= 2 {
            xfer.xfer_real(&mut self.skill_points_modifier)
                .map_err(|e| format!("skill_points_modifier xfer failed: {}", e))?;
        } else if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.skill_points_modifier = 1.0;
        }

        // --- 33. Version >= 3: List in score screen ---
        // C++ lines 4311-4318
        if version >= 3 {
            xfer.xfer_bool(&mut self.list_in_score_screen)
                .map_err(|e| format!("list_in_score_screen xfer failed: {}", e))?;
        } else if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.list_in_score_screen = true;
        }

        // --- 34. Attacked by (raw byte blob) ---
        // C++ line 4320: xfer->xferUser(m_attackedBy, sizeof(Bool) * MAX_PLAYER_COUNT)
        // In C++, Bool is typedef'd to Int (4 bytes), so this is MAX_PLAYER_COUNT * 4 bytes
        {
            let max_players = super::super::player_list::MAX_PLAYER_COUNT;
            let blob_size = max_players * std::mem::size_of::<u32>(); // Bool = Int = 4 bytes
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    let mut blob = vec![0u8; blob_size];
                    for i in 0..max_players {
                        let val: u32 = if i < self.attacked_by.len() && self.attacked_by[i] {
                            1
                        } else {
                            0
                        };
                        let start = i * std::mem::size_of::<u32>();
                        blob[start..start + 4].copy_from_slice(&val.to_le_bytes());
                    }
                                        // SAFETY: blob is an owned Vec<u8> of exactly blob_size;
                                        // save/crc only read it.
                    unsafe {
                        xfer.xfer_user(blob.as_ptr() as *mut u8, blob_size)
                            .map_err(|e| format!("attacked_by xfer failed: {}", e))?;
                    }
                }
                XferMode::Load => {
                    let mut blob = vec![0u8; blob_size];
                                        // SAFETY: blob is an owned Vec<u8> of exactly blob_size;
                                        // load fills every byte before parsing.
                    unsafe {
                        xfer.xfer_user(blob.as_mut_ptr(), blob_size)
                            .map_err(|e| format!("attacked_by load failed: {}", e))?;
                    }
                    for i in 0..max_players {
                        let start = i * std::mem::size_of::<u32>();
                        if start + 4 <= blob.len() && i < self.attacked_by.len() {
                            let val = u32::from_le_bytes(
                                blob[start..start + 4].try_into().unwrap_or([0; 4]),
                            );
                            self.attacked_by[i] = val != 0;
                        }
                    }
                }
                _ => {}
            }
        }

        // --- 35. Cash bounty percent ---
        // C++ line 4323
        xfer.xfer_real(&mut self.cash_bounty_percent)
            .map_err(|e| format!("cash_bounty_percent xfer failed: {}", e))?;

        // --- 36. Score keeper xferSnapshot ---
        // C++ line 4326: xfer->xferSnapshot(&m_scoreKeeper)
        self.score_keeper
            .xfer(xfer)
            .map_err(|e| format!("score_keeper xfer failed: {}", e))?;

        // --- 37. KindOf percent production change list ---
        // C++ lines 4328-4386: count + for each: kindOf.xfer, xferReal(percent), xferUnsignedInt(ref)
        let mut percent_production_change_count = self.kind_of_production_cost_changes.len() as u16;
        xfer.xfer_unsigned_short(&mut percent_production_change_count)
            .map_err(|e| format!("percent_production_change_count xfer failed: {}", e))?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for entry in &mut self.kind_of_production_cost_changes {
                    // C++ Player.cpp:4346-4352 — BitFlags name list + percent + ref.
                    xfer_kind_of_mask(xfer, &mut entry.kind_of)?;
                    xfer.xfer_real(&mut entry.percent)
                        .map_err(|e| format!("kindof percent xfer failed: {}", e))?;
                    xfer.xfer_unsigned_int(&mut entry.refs)
                        .map_err(|e| format!("kindof ref xfer failed: {}", e))?;
                }
            }
            XferMode::Load => {
                self.kind_of_production_cost_changes.clear();
                for _ in 0..percent_production_change_count {
                    let mut kind_of = KindOfMask::empty();
                    xfer_kind_of_mask(xfer, &mut kind_of)?;
                    let mut percent = 0.0f32;
                    xfer.xfer_real(&mut percent)
                        .map_err(|e| format!("load kindof percent failed: {}", e))?;
                    let mut refs = 0u32;
                    xfer.xfer_unsigned_int(&mut refs)
                        .map_err(|e| format!("load kindof ref failed: {}", e))?;
                    self.kind_of_production_cost_changes
                        .push(KindOfPercentProductionChange {
                            kind_of,
                            percent,
                            refs,
                        });
                }
            }
            _ => {}
        }

        // --- 38. Version >= 4: Special power ready timers ---
        // C++ lines 4392-4434
        if version >= 4 {
            let mut timer_list_size = self.special_power_timers.len() as u16;
            xfer.xfer_unsigned_short(&mut timer_list_size)
                .map_err(|e| format!("timer_list_size xfer failed: {}", e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save => {
                    for (&template_id, &ready_frame) in &self.special_power_timers {
                        let mut tid = template_id;
                        let mut rf = ready_frame;
                        xfer.xfer_unsigned_int(&mut tid)
                            .map_err(|e| format!("timer template_id failed: {}", e))?;
                        xfer.xfer_unsigned_int(&mut rf)
                            .map_err(|e| format!("timer ready_frame failed: {}", e))?;
                    }
                }
                XferMode::Load => {
                    self.special_power_timers.clear();
                    for _ in 0..timer_list_size {
                        let mut template_id = 0u32;
                        let mut ready_frame = 0u32;
                        xfer.xfer_unsigned_int(&mut template_id)
                            .map_err(|e| format!("load timer template_id failed: {}", e))?;
                        xfer.xfer_unsigned_int(&mut ready_frame)
                            .map_err(|e| format!("load timer ready_frame failed: {}", e))?;
                        self.special_power_timers.insert(template_id, ready_frame);
                    }
                }
                _ => {}
            }
        } else if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.special_power_timers.clear();
        }

        // --- 39. Squads (NUM_HOTKEY_SQUADS count + xferSnapshot for each) ---
        // C++ lines 4440-4463
        let mut squad_count = NUM_HOTKEY_SQUADS as u16;
        xfer.xfer_unsigned_short(&mut squad_count)
            .map_err(|e| format!("squad_count xfer failed: {}", e))?;

        // C++ validates squadCount == NUM_HOTKEY_SQUADS
        if squad_count as usize != NUM_HOTKEY_SQUADS {
            return Err(format!(
                "Player::xfer - squad count mismatch: expected {}, got {}",
                NUM_HOTKEY_SQUADS, squad_count
            ));
        }

        for i in 0..NUM_HOTKEY_SQUADS {
            // Squad xferSnapshot - inline serialization matching C++ Squad::xfer
            // C++ Squad::xfer writes version + count + ObjectID list
            const SQUAD_VERSION: XferVersion = 1;
            let mut sq_version = SQUAD_VERSION;
            xfer.xfer_version(&mut sq_version, SQUAD_VERSION)
                .map_err(|e| format!("squad[{}] version failed: {}", i, e))?;
            let mut obj_count = self.hotkey_squads[i].len() as u16;
            xfer.xfer_unsigned_short(&mut obj_count)
                .map_err(|e| format!("squad[{}] obj_count failed: {}", i, e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    for &obj_id in self.hotkey_squads[i].get_object_ids() {
                        let mut id = obj_id;
                        xfer.xfer_unsigned_int(&mut id)
                            .map_err(|e| format!("squad[{}] obj_id failed: {}", i, e))?;
                    }
                }
                XferMode::Load => {
                    self.hotkey_squads[i].clear();
                    for _ in 0..obj_count {
                        let mut obj_id = 0u32;
                        xfer.xfer_unsigned_int(&mut obj_id)
                            .map_err(|e| format!("load squad[{}] obj_id failed: {}", i, e))?;
                        self.hotkey_squads[i].add_object(obj_id);
                    }
                }
                _ => {}
            }
        }

        // --- 40. Current selection ---
        // C++ lines 4465-4478: xferBool(currentSelectionPresent), if present xferSnapshot
        let mut current_selection_present = true; // C++ always has m_currentSelection allocated
        xfer.xfer_bool(&mut current_selection_present)
            .map_err(|e| format!("current_selection_present xfer failed: {}", e))?;
        if current_selection_present {
            // Squad xferSnapshot for current selection
            const SQUAD_VERSION: XferVersion = 1;
            let mut sq_version = SQUAD_VERSION;
            xfer.xfer_version(&mut sq_version, SQUAD_VERSION)
                .map_err(|e| format!("current_selection version failed: {}", e))?;
            let mut obj_count = self.current_selection.len() as u16;
            xfer.xfer_unsigned_short(&mut obj_count)
                .map_err(|e| format!("current_selection obj_count failed: {}", e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    for &obj_id in self.current_selection.get_object_ids() {
                        let mut id = obj_id;
                        xfer.xfer_unsigned_int(&mut id)
                            .map_err(|e| format!("current_selection obj_id failed: {}", e))?;
                    }
                }
                XferMode::Load => {
                    self.current_selection.clear();
                    for _ in 0..obj_count {
                        let mut obj_id = 0u32;
                        xfer.xfer_unsigned_int(&mut obj_id)
                            .map_err(|e| format!("load current_selection obj_id failed: {}", e))?;
                        self.current_selection.add_object(obj_id);
                    }
                }
                _ => {}
            }
        }

        // --- 41. Battle plan bonuses ---
        // C++ Player.cpp:4480-4504 — present flag; on load replace pointer; then fields if non-NULL.
        let mut battle_plan_bonus = self.battle_plan_bonuses.is_some();
        xfer.xfer_bool(&mut battle_plan_bonus)
            .map_err(|e| format!("battle_plan_bonus xfer failed: {}", e))?;
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.battle_plan_bonuses = if battle_plan_bonus {
                Some(BattlePlanBonuses::default())
            } else {
                None
            };
        }
        if let Some(bonuses) = &mut self.battle_plan_bonuses {
            xfer.xfer_real(&mut bonuses.armor_scalar)
                .map_err(|e| format!("armor_scalar xfer failed: {}", e))?;
            xfer.xfer_real(&mut bonuses.sight_range_scalar)
                .map_err(|e| format!("sight_range_scalar xfer failed: {}", e))?;
            xfer.xfer_int(&mut bonuses.bombardment)
                .map_err(|e| format!("bombardment xfer failed: {}", e))?;
            xfer.xfer_int(&mut bonuses.hold_the_line)
                .map_err(|e| format!("hold_the_line xfer failed: {}", e))?;
            xfer.xfer_int(&mut bonuses.search_and_destroy)
                .map_err(|e| format!("search_and_destroy xfer failed: {}", e))?;
            xfer_kind_of_mask(xfer, &mut bonuses.valid_kind_of)?;
            xfer_kind_of_mask(xfer, &mut bonuses.invalid_kind_of)?;
        }

        // --- 42-44. Battle plan counts ---
        // C++ lines 4505-4507
        xfer.xfer_int(&mut self.bombard_battle_plans)
            .map_err(|e| format!("bombard_battle_plans xfer failed: {}", e))?;
        xfer.xfer_int(&mut self.hold_the_line_battle_plans)
            .map_err(|e| format!("hold_the_line_battle_plans xfer failed: {}", e))?;
        xfer.xfer_int(&mut self.search_and_destroy_battle_plans)
            .map_err(|e| format!("search_and_destroy_battle_plans xfer failed: {}", e))?;

        // --- 45. Version >= 6: Units should hunt ---
        // C++ lines 4509-4514
        if version >= 6 {
            xfer.xfer_bool(&mut self.units_should_hunt)
                .map_err(|e| format!("units_should_hunt xfer failed: {}", e))?;
        } else if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.units_should_hunt = false;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ Player::loadPostProcess() is empty (Player.cpp line 4522)
        Ok(())
    }
}

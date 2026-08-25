use super::*;

/// C++ `BitFlags<NUMBITS>::xfer` (BitFlagsIO.h:134-207) for KindOf masks.
fn xfer_kind_of_mask(xfer: &mut dyn Xfer, mask: &mut KindOfMaskType) -> Result<(), String> {
    use game_engine::common::system::kind_of::KindOfMask;

    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| format!("KindOfMask version xfer failed: {}", e))?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            let named = KindOfMask::from_bits_truncate(*mask as u128);
            let names = named.to_string_list();
            let mut count = names.len() as i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("KindOfMask count xfer failed: {}", e))?;
            for mut name in names {
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("KindOfMask name xfer failed: {}", e))?;
            }
            Ok(())
        }
        XferMode::Load => {
            let mut named = KindOfMask::empty();
            let mut count = 0i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("KindOfMask count load failed: {}", e))?;
            for _ in 0..count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("KindOfMask name load failed: {}", e))?;
                let bit = KindOfMask::from_string(&name)
                    .ok_or_else(|| format!("KindOfMask invalid bit name '{}'", name))?;
                named |= bit;
            }
            *mask = named.bits() as KindOfMaskType;
            Ok(())
        }
        XferMode::Crc => {
            let mut bits = *mask as u128;
            xfer.xfer_u128(&mut bits)
                .map_err(|e| format!("KindOfMask crc failed: {}", e))?;
            Ok(())
        }
        _ => Err(format!(
            "KindOfMask xfer - unknown mode {:?}",
            xfer.get_xfer_mode()
        )),
    }
}

/// Save/load support for Player.
/// Matches C++ Player::xfer (Player.cpp:3975, version 8).
impl Snapshotable for Player {
    /// C++ Player::crc is intentionally much narrower than Player::xfer.
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut has_battle_plan_bonus = self.battle_plan_bonuses.is_some();
        xfer.xfer_bool(&mut has_battle_plan_bonus)
            .map_err(|e| e.to_string())?;
        if let Some(bonuses) = &self.battle_plan_bonuses {
            let mut armor_scalar = bonuses.armor_scalar;
            xfer.xfer_real(&mut armor_scalar)
                .map_err(|e| e.to_string())?;
            let mut sight_range_scalar = bonuses.sight_range_scalar;
            xfer.xfer_real(&mut sight_range_scalar)
                .map_err(|e| e.to_string())?;
            let mut bombardment = bonuses.bombardment;
            xfer.xfer_int(&mut bombardment).map_err(|e| e.to_string())?;
            let mut hold_the_line = bonuses.hold_the_line;
            xfer.xfer_int(&mut hold_the_line)
                .map_err(|e| e.to_string())?;
            let mut search_and_destroy = bonuses.search_and_destroy;
            xfer.xfer_int(&mut search_and_destroy)
                .map_err(|e| e.to_string())?;
            let mut valid_kind_of = bonuses.valid_kind_of;
            xfer_kind_of_mask(xfer, &mut valid_kind_of)?;
            let mut invalid_kind_of = bonuses.invalid_kind_of;
            xfer_kind_of_mask(xfer, &mut invalid_kind_of)?;
        }

        let mut skill_points = self.skill_points;
        xfer.xfer_int(&mut skill_points)
            .map_err(|e| e.to_string())?;
        let mut science_purchase_points = self.science_purchase_points;
        xfer.xfer_int(&mut science_purchase_points)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Player::xfer version 8
        let current_version: XferVersion = 8;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // Money (inline, matching C++ Money::xfer v1: just the amount)
        {
            let mut money_version: XferVersion = 1;
            xfer.xfer_version(&mut money_version, 1)
                .map_err(|e| e.to_string())?;
            let mut money_amount = self.money.amount as u32;
            xfer.xfer_u32(&mut money_amount)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.money.amount = money_amount as Int;
            }
        }

        // Upgrade list count
        let mut upgrade_count = self.upgrade_list.len() as u16;
        xfer.xfer_unsigned_short(&mut upgrade_count)
            .map_err(|e| e.to_string())?;

        // Version 7: preorder
        if version >= 7 {
            xfer.xfer_bool(&mut self.is_preorder)
                .map_err(|e| e.to_string())?;
        }

        // Version 8: disabled/hidden science vectors
        if version >= 8 {
            xfer.xfer_science_vec(&mut self.sciences_disabled)
                .map_err(|e| e.to_string())?;
            xfer.xfer_science_vec(&mut self.sciences_hidden)
                .map_err(|e| e.to_string())?;
        }

        // Upgrade instances
        if xfer.get_xfer_mode() == XferMode::Save {
            for upgrade in &mut self.upgrade_list {
                let mut upgrade_name = upgrade.get_template().get_name().to_string();
                xfer.xfer_ascii_string(&mut upgrade_name)
                    .map_err(|e| e.to_string())?;
                upgrade.xfer(xfer)?;
            }
        } else {
            self.upgrade_list.clear();
            for _ in 0..upgrade_count {
                let mut upgrade_name = String::new();
                xfer.xfer_ascii_string(&mut upgrade_name)
                    .map_err(|e| e.to_string())?;

                let template = crate::upgrade::center::get_upgrade_center()
                    .read()
                    .ok()
                    .and_then(|center| center.find_upgrade(&upgrade_name));
                if template.is_none() {
                    return Err(format!(
                        "Player::xfer - Unable to find upgrade '{}'",
                        upgrade_name
                    ));
                }

                let template_arc = template.unwrap();
                let mut upgrade = crate::upgrade::Upgrade::new(template_arc);
                upgrade.xfer(xfer)?;
                self.upgrade_list.push(upgrade);
            }
        }

        // Radar info
        xfer.xfer_int(&mut self.radar_count)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_player_dead)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.disable_proof_radar_count)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.radar_disabled)
            .map_err(|e| e.to_string())?;

        // C++ Player.cpp:4062/4065 xferUpgradeMask (Xfer.cpp:708-805): version + name list.
        {
            let mut in_progress = self.upgrades_in_progress.bits();
            xfer.xfer_upgrade_mask(&mut in_progress)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.upgrades_in_progress = UpgradeMaskType::from_bits_truncate(in_progress);
            }
        }
        {
            let mut completed = self.upgrades_completed.bits();
            xfer.xfer_upgrade_mask(&mut completed)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.upgrades_completed = UpgradeMaskType::from_bits_truncate(completed);
            }
        }

        // Energy (inline, matching C++ Energy::xfer v3)
        {
            let mut energy_version: XferVersion = 3;
            xfer.xfer_version(&mut energy_version, 3)
                .map_err(|e| e.to_string())?;
            if energy_version < 2 {
                let mut production: Int = self.energy.production;
                xfer.xfer_int(&mut production).map_err(|e| e.to_string())?;
                let mut consumption: Int = self.energy.consumption;
                xfer.xfer_int(&mut consumption).map_err(|e| e.to_string())?;
                if xfer.get_xfer_mode() == XferMode::Load {
                    self.energy.production = 0; // rebuilt from objects
                    self.energy.consumption = 0; // rebuilt from objects
                }
            }
            let mut owning_player_index = self.player_index;
            xfer.xfer_int(&mut owning_player_index)
                .map_err(|e| e.to_string())?;
            if energy_version >= 3 {
                xfer.xfer_u32(&mut self.energy.power_sabotaged_till_frame)
                    .map_err(|e| e.to_string())?;
            }
        }

        // Team prototypes (count + IDs, resolved on load via TeamFactory)
        {
            let mut prototype_count = self.player_team_prototypes.len() as u16;
            xfer.xfer_unsigned_short(&mut prototype_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for prototype in &self.player_team_prototypes {
                    let mut proto_id = prototype.get_id();
                    xfer.xfer_u32(&mut proto_id).map_err(|e| e.to_string())?;
                }
            } else {
                self.player_team_prototypes.clear();
                let factory = crate::team::get_team_factory();
                let Ok(factory_guard) = factory.lock() else {
                    return Err("Player::xfer - cannot lock TeamFactory".to_string());
                };
                for _ in 0..prototype_count {
                    let mut proto_id: UnsignedInt = 0;
                    xfer.xfer_u32(&mut proto_id).map_err(|e| e.to_string())?;
                    if let Some(prototype) = factory_guard.find_team_prototype_by_id(proto_id) {
                        self.player_team_prototypes.push(prototype);
                    }
                }
            }
        }

        // Build list info (count + snapshots)
        {
            let mut build_list_count: UnsignedShort = 0;
            if xfer.get_xfer_mode() == XferMode::Save {
                let mut entry = self.build_list.as_deref();
                while let Some(info) = entry {
                    build_list_count = build_list_count.saturating_add(1);
                    entry = info.get_next();
                }
            }

            xfer.xfer_unsigned_short(&mut build_list_count)
                .map_err(|e| e.to_string())?;

            if xfer.get_xfer_mode() == XferMode::Save {
                let mut entry = self.build_list.as_deref_mut();
                while let Some(info) = entry {
                    info.xfer(xfer);
                    entry = info.get_next_mut();
                }
            } else {
                let mut entries = Vec::with_capacity(build_list_count as usize);
                for _ in 0..build_list_count {
                    let mut info = BuildListInfo::new();
                    info.xfer(xfer);
                    entries.push(info);
                }

                self.build_list = None;
                for mut info in entries.into_iter().rev() {
                    info.set_next_build_list_boxed(self.build_list.take());
                    self.build_list = Some(Box::new(info));
                }
            }
        }

        // AI player data.  C++ writes a presence bool, then the
        // AIPlayer/AISkirmishPlayer snapshot.  The controller belongs to the
        // already-initialized map/player graph, so a load must never invent a
        // controller merely to consume bytes.
        {
            let player_id = self.player_index as u32;
            let runtime_ai_present = crate::ai::integration::with_ai_integration(|manager| {
                manager.has_ai_player(player_id)
            })
            .unwrap_or(false);
            let mut ai_present = runtime_ai_present;
            xfer.xfer_bool(&mut ai_present).map_err(|e| e.to_string())?;

            if ai_present != runtime_ai_present {
                return Err(format!(
                    "Player::xfer - AI presence mismatch for player {}",
                    player_id
                ));
            }

            if ai_present {
                let xfer_result = crate::ai::integration::with_ai_integration_mut(|manager| {
                    manager.xfer_ai_player(player_id, self.is_skirmish_ai, xfer)
                });

                match xfer_result {
                    Some(Ok(())) => {}
                    Some(Err(err)) => return Err(err),
                    None => {
                        return Err(format!(
                            "Player::xfer - AI integration manager unavailable for player {}",
                            player_id
                        ));
                    }
                }
            }
        }

        // Resource gathering manager
        {
            let runtime_rgm_present = self.resource_manager.is_some();
            let mut rgm_present = runtime_rgm_present;
            xfer.xfer_bool(&mut rgm_present)
                .map_err(|e| e.to_string())?;
            if rgm_present != runtime_rgm_present {
                return Err(
                    "Player::xfer - resource gathering manager presence mismatch".to_string(),
                );
            }
            if let Some(manager) = self.resource_manager.as_mut() {
                manager.xfer(xfer)?;
            }
        }

        // Tunnel tracker
        {
            let runtime_tunnel_present = self.tunnel_tracker.is_some();
            let mut tunnel_present = runtime_tunnel_present;
            xfer.xfer_bool(&mut tunnel_present)
                .map_err(|e| e.to_string())?;
            if tunnel_present != runtime_tunnel_present {
                return Err("Player::xfer - tunnel tracker presence mismatch".to_string());
            }
            if let Some(tracker) = self.tunnel_tracker.as_mut() {
                tracker.xfer(xfer)?;
            }
        }

        // Default team ID
        {
            let mut team_id: UnsignedInt = self
                .default_team
                .as_ref()
                .and_then(|t| t.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::team::TEAM_ID_INVALID);
            xfer.xfer_u32(&mut team_id).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                let factory = crate::team::get_team_factory();
                if let Ok(mut factory_guard) = factory.lock() {
                    self.default_team = factory_guard.find_team_by_id(team_id);
                }
            }
        }

        // Sciences (version >= 5)
        if version >= 5 {
            if xfer.get_xfer_mode() == XferMode::Load {
                self.sciences.clear();
            }
            xfer.xfer_science_vec(&mut self.sciences)
                .map_err(|e| e.to_string())?;
        }

        // Rank/skill
        xfer.xfer_int(&mut self.rank_level)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.skill_points)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.science_purchase_points)
            .map_err(|e| e.to_string())?;

        // Level up/down (C++ has these, Rust may not track them separately)
        let mut level_up: Int = 0;
        xfer.xfer_int(&mut level_up).map_err(|e| e.to_string())?;
        let mut level_down: Int = 0;
        xfer.xfer_int(&mut level_down).map_err(|e| e.to_string())?;

        // General name (C++ Player::xfer writes UnicodeString)
        xfer.xfer_unicode_string(&mut self.general_name)
            .map_err(|e| e.to_string())?;

        // Player relations (inline, matching C++ PlayerRelationMap::xfer v1)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;
            let mut rel_count = self.player_relations.map.len() as u16;
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for (&pidx, &rel) in &self.player_relations.map {
                    let mut player_idx = pidx;
                    let mut rel_raw = rel as Int;
                    xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                }
            } else {
                self.player_relations.map.clear();
                for _ in 0..rel_count {
                    let mut player_idx: Int = 0;
                    let mut rel_raw: Int = 0;
                    xfer.xfer_int(&mut player_idx).map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    let rel = match rel_raw {
                        0 => Relationship::Enemies,
                        1 => Relationship::Neutral,
                        2 => Relationship::Allies,
                        _ => Relationship::Neutral,
                    };
                    self.player_relations.map.insert(player_idx, rel);
                }
            }
        }

        // Team relations (inline, matching C++ TeamRelationMap::xfer v1)
        {
            let mut rel_version: XferVersion = 1;
            xfer.xfer_version(&mut rel_version, 1)
                .map_err(|e| e.to_string())?;
            let mut rel_count = self
                .team_relations
                .as_ref()
                .map(|r| r.map.len() as u16)
                .unwrap_or(0);
            xfer.xfer_unsigned_short(&mut rel_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                if let Some(ref relations) = self.team_relations {
                    for (&tid, &rel) in &relations.map {
                        let mut team_id_val = tid;
                        let mut rel_raw = rel as Int;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                    }
                }
            } else {
                self.team_relations = None;
                if rel_count > 0 {
                    let mut map = crate::team::TeamRelationMap::new();
                    for _ in 0..rel_count {
                        let mut team_id_val: UnsignedInt = 0;
                        let mut rel_raw: Int = 0;
                        xfer.xfer_u32(&mut team_id_val).map_err(|e| e.to_string())?;
                        xfer.xfer_int(&mut rel_raw).map_err(|e| e.to_string())?;
                        let rel = match rel_raw {
                            0 => Relationship::Enemies,
                            1 => Relationship::Neutral,
                            2 => Relationship::Allies,
                            _ => Relationship::Neutral,
                        };
                        map.map.insert(team_id_val, rel);
                    }
                    self.team_relations = Some(map);
                }
            }
        }

        // Build flags
        xfer.xfer_bool(&mut self.can_build_units)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.can_build_base)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_observer)
            .map_err(|e| e.to_string())?;

        // Version 2: skill points modifier
        if version >= 2 {
            xfer.xfer_real(&mut self.skill_points_modifier)
                .map_err(|e| e.to_string())?;
        }

        // Version 3: list in score screen
        if version >= 3 {
            xfer.xfer_bool(&mut self.list_in_score_screen)
                .map_err(|e| e.to_string())?;
        }

        // Attacked by array (raw bytes matching C++ xferUser)
        for i in 0..MAX_PLAYER_COUNT {
            let mut val = self.attacked_by[i];
            xfer.xfer_bool(&mut val).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.attacked_by[i] = val;
            }
        }

        // Cash bounty percent
        xfer.xfer_real(&mut self.cash_bounty_percent)
            .map_err(|e| e.to_string())?;

        // ScoreKeeper (inline, matching C++ ScoreKeeper::xfer v1)
        {
            let mut sk_version: XferVersion = 1;
            xfer.xfer_version(&mut sk_version, 1)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.supplies_collected)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.supplies_spent)
                .map_err(|e| e.to_string())?;
            for i in 0..MAX_PLAYER_COUNT {
                xfer.xfer_int(&mut self.score_keeper.units_destroyed_by_player[i])
                    .map_err(|e| e.to_string())?;
            }
            xfer.xfer_int(&mut self.score_keeper.units_built)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.units_lost)
                .map_err(|e| e.to_string())?;
            for i in 0..MAX_PLAYER_COUNT {
                xfer.xfer_int(&mut self.score_keeper.buildings_destroyed_by_player[i])
                    .map_err(|e| e.to_string())?;
            }
            xfer.xfer_int(&mut self.score_keeper.buildings_built)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.buildings_lost)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.tech_buildings_captured)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.faction_buildings_captured)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.current_score)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut self.score_keeper.my_player_idx)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.score_keeper.recompute_destroyed_aggregates();
            }
            ScoreKeeper::xfer_object_count_map(xfer, &mut self.score_keeper.objects_built)?;

            // objects destroyed per-player array
            let mut destroyed_array_size = MAX_PLAYER_COUNT as u16;
            xfer.xfer_unsigned_short(&mut destroyed_array_size)
                .map_err(|e| e.to_string())?;
            if destroyed_array_size as usize != MAX_PLAYER_COUNT {
                return Err(format!(
                    "ScoreKeeper::xfer - objects destroyed array size mismatch: expected {}, got {}",
                    MAX_PLAYER_COUNT, destroyed_array_size
                ));
            }
            for i in 0..MAX_PLAYER_COUNT {
                ScoreKeeper::xfer_object_count_map(
                    xfer,
                    &mut self.score_keeper.objects_destroyed[i],
                )?;
            }
            ScoreKeeper::xfer_object_count_map(xfer, &mut self.score_keeper.objects_lost)?;
            ScoreKeeper::xfer_object_count_map(xfer, &mut self.score_keeper.objects_captured)?;
        }

        // KindOf percent production change list
        {
            let mut change_count = self.kind_of_percent_production_change_list.len() as u16;
            xfer.xfer_unsigned_short(&mut change_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                for entry in &self.kind_of_percent_production_change_list {
                    let mut kind_of_raw = entry.kind_of;
                    xfer_kind_of_mask(xfer, &mut kind_of_raw)?;
                    let mut percent = entry.percent;
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                    let mut refs = entry.refs;
                    xfer.xfer_u32(&mut refs).map_err(|e| e.to_string())?;
                }
            } else {
                self.kind_of_percent_production_change_list.clear();
                for _ in 0..change_count {
                    let mut kind_of_raw: KindOfMaskType = 0;
                    xfer_kind_of_mask(xfer, &mut kind_of_raw)?;
                    let mut percent: Real = 0.0;
                    xfer.xfer_real(&mut percent).map_err(|e| e.to_string())?;
                    let mut refs: UnsignedInt = 0;
                    xfer.xfer_u32(&mut refs).map_err(|e| e.to_string())?;
                    self.kind_of_percent_production_change_list.push(
                        KindOfPercentProductionChange {
                            kind_of: kind_of_raw,
                            percent,
                            refs,
                        },
                    );
                }
            }
        }

        // Version 4+: special power ready timer list
        if version >= 4 {
            let mut timer_count: u16 = 0;
            if let Ok(timers) = self.special_power_ready_timers.read() {
                timer_count = timers.len() as u16;
            }
            xfer.xfer_unsigned_short(&mut timer_count)
                .map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Save {
                if let Ok(timers) = self.special_power_ready_timers.read() {
                    for timer in timers.iter() {
                        let mut template_id = timer.template_id;
                        let mut ready_frame = timer.ready_frame;
                        xfer.xfer_u32(&mut template_id).map_err(|e| e.to_string())?;
                        xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                    }
                }
            } else if let Ok(mut timers) = self.special_power_ready_timers.write() {
                timers.clear();
                for _ in 0..timer_count {
                    let mut template_id: UnsignedInt = 0;
                    let mut ready_frame: UnsignedInt = 0;
                    xfer.xfer_u32(&mut template_id).map_err(|e| e.to_string())?;
                    xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                    timers.push(SpecialPowerReadyTimer {
                        template_id,
                        ready_frame,
                    });
                }
            }
        }

        // Squads
        {
            let mut squad_count = NUM_HOTKEY_SQUADS as u16;
            xfer.xfer_unsigned_short(&mut squad_count)
                .map_err(|e| e.to_string())?;
            if squad_count as usize != NUM_HOTKEY_SQUADS {
                return Err("Player::xfer - squad count mismatch".to_string());
            }
            for slot in &mut self.squads {
                if slot.is_none() {
                    *slot = Some(Squad::new());
                }
                if let Some(squad) = slot.as_mut() {
                    squad.xfer(xfer)?;
                }
            }
        }

        // Current selection (present bool + snapshot)
        {
            let mut selection_present = self.current_selection.is_some();
            xfer.xfer_bool(&mut selection_present)
                .map_err(|e| e.to_string())?;
            if selection_present {
                if self.current_selection.is_none() {
                    self.current_selection = Some(Squad::new());
                }
                if let Some(ref mut selection) = self.current_selection {
                    selection.xfer(xfer)?;
                }
            } else {
                self.current_selection = None;
            }
        }

        // Battle plan bonuses
        {
            let mut has_bonus = self.battle_plan_bonuses.is_some();
            xfer.xfer_bool(&mut has_bonus).map_err(|e| e.to_string())?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.battle_plan_bonuses = None;
                if has_bonus {
                    self.battle_plan_bonuses = Some(BattlePlanBonuses {
                        armor_scalar: 1.0,
                        sight_range_scalar: 1.0,
                        bombardment: 0,
                        hold_the_line: 0,
                        search_and_destroy: 0,
                        valid_kind_of: crate::common::KIND_OF_MASK_NONE,
                        invalid_kind_of: crate::common::KIND_OF_MASK_NONE,
                    });
                }
            }
            if let Some(ref mut bonuses) = self.battle_plan_bonuses {
                xfer.xfer_real(&mut bonuses.armor_scalar)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_real(&mut bonuses.sight_range_scalar)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut bonuses.bombardment)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut bonuses.hold_the_line)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_int(&mut bonuses.search_and_destroy)
                    .map_err(|e| e.to_string())?;
                let mut valid_kind_of = bonuses.valid_kind_of;
                xfer_kind_of_mask(xfer, &mut valid_kind_of)?;
                bonuses.valid_kind_of = valid_kind_of;
                let mut invalid_kind_of = bonuses.invalid_kind_of;
                xfer_kind_of_mask(xfer, &mut invalid_kind_of)?;
                bonuses.invalid_kind_of = invalid_kind_of;
            }
        }

        // Battle plan counts
        xfer.xfer_int(&mut self.bombard_battle_plans)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.hold_the_line_battle_plans)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.search_and_destroy_battle_plans)
            .map_err(|e| e.to_string())?;

        // Version 6: units_should_hunt
        if version >= 6 {
            xfer.xfer_bool(&mut self.units_should_hunt)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        let player_id = self.player_index as u32;
        let _ = crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.load_post_process_ai_player(player_id);
        });
        if let Some(manager) = self.resource_manager.as_mut() {
            manager.load_post_process()?;
        }
        if let Some(tracker) = self.tunnel_tracker.as_mut() {
            tracker.load_post_process()?;
        }
        Ok(())
    }
}

impl PlayerInterface for Player {
    fn get_or_start_special_power_ready_frame(
        &self,
        power_id: SpecialPowerID,
        current_frame: FrameCount,
    ) -> FrameCount {
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == power_id {
                    return timer.ready_frame;
                }
            }

            let mut timer = SpecialPowerReadyTimer::new();
            timer.template_id = power_id;
            timer.ready_frame = current_frame;
            timers.push(timer);
        }

        current_frame
    }

    fn express_special_power_ready_frame(&mut self, power_id: SpecialPowerID, frame: FrameCount) {
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == power_id {
                    timer.ready_frame = frame;
                    return;
                }
            }

            let mut timer = SpecialPowerReadyTimer::new();
            timer.template_id = power_id;
            timer.ready_frame = frame;
            timers.push(timer);
        }
    }

    fn reset_or_start_special_power_ready_frame(
        &mut self,
        power_id: SpecialPowerID,
        current_frame: FrameCount,
        reload_time: FrameCount,
    ) {
        let ready_frame = current_frame.saturating_add(reload_time);
        if let Ok(mut timers) = self.special_power_ready_timers.write() {
            for timer in timers.iter_mut() {
                if timer.template_id == power_id {
                    timer.ready_frame = ready_frame;
                    return;
                }
            }

            let mut timer = SpecialPowerReadyTimer::new();
            timer.template_id = power_id;
            timer.ready_frame = ready_frame;
            timers.push(timer);
        }
    }

    fn has_science(&self, science_name: &str) -> bool {
        let Some(store) = get_science_store() else {
            return false;
        };
        let science = store.get_science_from_internal_name(science_name);
        ScienceAccess::has_science(self, science)
    }

    fn get_player_index(&self) -> UnsignedInt {
        self.player_index as UnsignedInt
    }

    #[cfg(any(debug_assertions, feature = "allow_debug_cheats"))]
    fn builds_instantly(&self) -> bool {
        self.builds_instantly()
    }

    #[cfg(not(any(debug_assertions, feature = "allow_debug_cheats")))]
    fn builds_instantly(&self) -> bool {
        false
    }

    fn get_money(&self) -> &dyn MoneyInterface {
        &self.money
    }

    fn get_build_time_modifier(&self) -> f32 {
        let mut modifier = self.handicap.get_build_time_multiplier();
        let energy_ratio = self.energy.supply_ratio();

        let (low_energy_penalty_modifier, min_speed, max_speed) =
            if let Some(data) = game_engine::common::ini::get_global_data() {
                let guard = data.read();
                (
                    guard.low_energy_penalty_modifier,
                    guard.min_low_energy_production_speed,
                    guard.max_low_energy_production_speed,
                )
            } else {
                (0.5_f32, 0.5_f32, 1.0_f32)
            };

        let energy_percent = energy_ratio.min(1.0);
        let energy_short = (1.0 - energy_percent) * low_energy_penalty_modifier;
        let mut penalty_rate = 1.0 - energy_short;
        penalty_rate = penalty_rate.max(min_speed);
        if energy_percent < 1.0 {
            penalty_rate = penalty_rate.min(max_speed);
        }
        let penalty_rate = if penalty_rate <= 0.0 {
            0.01
        } else {
            penalty_rate
        };

        modifier /= penalty_rate;
        modifier
    }

    fn get_cost_modifier(&self) -> f32 {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            if self.builds_for_free() {
                return 0.0;
            }
        }

        self.handicap.get_cost_multiplier()
    }
}

impl game_engine::common::thing::module::Thing for Player {
    fn get_production_cost_change_percent(&self, template_name: &str) -> f32 {
        self.get_production_cost_change_percent(template_name)
    }

    fn get_production_time_change_percent(&self, template_name: &str) -> f32 {
        self.get_production_time_change_percent(template_name)
    }

    fn get_production_cost_change_based_on_kind_of(&self, kind_of: u64) -> f32 {
        self.get_production_cost_change_based_on_kind_of(kind_of as KindOfMaskType)
    }

    fn get_build_cost_handicap(&self, template: &game_engine::common::thing::ThingTemplate) -> f32 {
        if template
            .is_kind_of(game_engine::common::system::kind_of::KindOfMask::STRUCTURE.bits() as u64)
        {
            self.handicap.build_cost_buildings
        } else {
            self.handicap.build_cost_generic
        }
    }

    fn get_build_time_handicap(&self, template: &game_engine::common::thing::ThingTemplate) -> f32 {
        if template
            .is_kind_of(game_engine::common::system::kind_of::KindOfMask::STRUCTURE.bits() as u64)
        {
            self.handicap.build_time_buildings
        } else {
            self.handicap.build_time_generic
        }
    }

    fn get_energy_supply_ratio(&self) -> f32 {
        self.energy.supply_ratio()
    }

    fn builds_instantly_for_debug(&self) -> bool {
        #[cfg(any(debug_assertions, feature = "allow_debug_cheats"))]
        {
            return self.builds_instantly();
        }

        #[cfg(not(any(debug_assertions, feature = "allow_debug_cheats")))]
        {
            false
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Player::new(0)
    }
}

impl ScienceAccess for Player {
    fn has_science(&self, science: ScienceType) -> bool {
        science != SCIENCE_INVALID && self.sciences.contains(&science)
    }
}

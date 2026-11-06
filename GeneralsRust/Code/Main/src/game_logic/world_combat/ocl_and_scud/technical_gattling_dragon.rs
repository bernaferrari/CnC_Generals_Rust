//! Technical, napalm, gattling chain-gun and dragon flame residuals.

use super::*;

impl GameLogic {
    /// Infer Technical salvage tier from residual weapon stats / upgrade tags.
    /// Technical tier from the authored weapon in `slot`; `None` when the slot
    /// does not hold a Technical weapon (hq-2qedt.173).
    pub(in super::super::super) fn technical_tier_for_authored_weapon(
        obj: &Object,
        slot: u8,
    ) -> Option<crate::game_logic::host_technical::TechnicalWeaponTier> {
        use crate::game_logic::host_technical::{
            TECHNICAL_CANNON, TECHNICAL_MACHINE_GUN, TECHNICAL_RPG, TechnicalWeaponTier,
        };
        match obj.selected_weapon_name(slot)? {
            TECHNICAL_MACHINE_GUN => Some(TechnicalWeaponTier::Base),
            TECHNICAL_CANNON => Some(TechnicalWeaponTier::One),
            TECHNICAL_RPG => Some(TechnicalWeaponTier::Two),
            _ => None,
        }
    }

    pub(in super::super::super) fn technical_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_technical::TechnicalWeaponTier {
        use crate::game_logic::host_technical::TechnicalWeaponTier;
        if obj.test_weapon_condition(5)
            || obj.has_upgrade_tag("TechnicalCrateUpgradeTwo")
        {
            return TechnicalWeaponTier::Two;
        }
        if obj.test_weapon_condition(4)
            || obj.has_upgrade_tag("TechnicalCrateUpgradeOne")
        {
            return TechnicalWeaponTier::One;
        }
        // Infer from primary damage residual when tags absent.
        if let Some(w) = obj.weapon.as_ref() {
            if (w.damage - 50.0).abs() < 0.5 {
                return TechnicalWeaponTier::Two;
            }
            if (w.damage - 45.0).abs() < 0.5 {
                return TechnicalWeaponTier::One;
            }
        }
        TechnicalWeaponTier::Base
    }

    /// Apply residual salvage weapon tier to a Technical (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D gunner subobject swap.
    pub fn apply_technical_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_technical::TechnicalWeaponTier,
    ) -> bool {
        use crate::game_logic::host_technical::{
            TECHNICAL_TRANSPORT_SLOTS, delay_frames_to_reload_secs, is_technical_weapon_name,
            technical_weapon_for_tier, technical_weapon_name_for_tier, technical_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        // Salvage tier swaps the authored WeaponSet slot weapon; gate on it.
        if !obj
            .authored_weapon_name_for_slot(0)
            .is_some_and(is_technical_weapon_name)
        {
            return false;
        }

        // Ensure passenger residual capacity is installed.
        if !obj.is_technical_style_container() {
            obj.install_technical_transport();
        }
        let _ = TECHNICAL_TRANSPORT_SLOTS;

        obj.set_weapon_condition_at_frame(
            4,
            tier == crate::game_logic::host_technical::TechnicalWeaponTier::One,
            self.frame,
        );
        obj.set_weapon_condition_at_frame(
            5,
            tier == crate::game_logic::host_technical::TechnicalWeaponTier::Two,
            self.frame,
        );

        let name = technical_weapon_name_for_tier(tier);
        let (dmg, range, min_range, delay, _splash) = technical_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| technical_weapon_for_tier(tier));
        // Force residual stats (store may lack min-range / reload).
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = min_range;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        let _ = obj.replace_weapon_set_slot(0, Some(weapon));

        self.technical_residual_weapon_upgrades =
            self.technical_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Technical residual fire (MG direct or cannon/RPG splash).
    pub(in super::super::super) fn apply_technical_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_technical_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn apply_technical_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_technical::{
            TECH_CANNON_DAMAGE_TYPE, TECH_CANNON_DEATH_TYPE, TECH_FIRE_AUDIO, TECH_MG_DAMAGE_TYPE,
            TECH_MG_DEATH_TYPE, TECH_RPG_DAMAGE_TYPE, TECH_RPG_DEATH_TYPE, TechnicalWeaponTier,
            is_legal_technical_splash_target, technical_cannon_scatter_aim,
            technical_cannon_scatter_misses_infantry, technical_splash_damage_at,
            technical_weapon_stats,
        };

        let tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::technical_tier_from_object)
            .unwrap_or(TechnicalWeaponTier::Base);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        let (_dmg, _range, _min, _delay, splash) = technical_weapon_stats(tier);

        // C++ TechnicalCannonWeapon ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let cannon_tier =
            matches!(tier, TechnicalWeaponTier::One | TechnicalWeaponTier::Two) && splash > 0.0;
        if intended_is_infantry && cannon_tier {
            let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
                source.map(|s| s.0).unwrap_or(0),
                intended_target.map(|id| id.0).unwrap_or(0),
                self.frame,
            );
            let hit_r = intended_target
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let (new_impact, scattered) = technical_cannon_scatter_aim(impact, true, seed);
            if scattered {
                self.technical_cannon_scatter_applied =
                    self.technical_cannon_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if technical_cannon_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > splash {
                        self.technical_cannon_scatter_misses =
                            self.technical_cannon_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        // Collect candidates: intended always; splash ring when tier has radius.
        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_technical_splash_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                // Scatter miss residual: intended infantry outside splash is not force-hit.
                if is_intended
                    && intended_is_infantry
                    && cannon_tier
                    && (splash <= 0.0 || dist > splash)
                {
                    return None;
                }
                if is_intended || (splash > 0.0 && dist <= splash) {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = technical_splash_damage_at(tier, is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let (dt_name, death_name) = match tier {
                    TechnicalWeaponTier::Base => (TECH_MG_DAMAGE_TYPE, TECH_MG_DEATH_TYPE),
                    TechnicalWeaponTier::One => (TECH_CANNON_DAMAGE_TYPE, TECH_CANNON_DEATH_TYPE),
                    TechnicalWeaponTier::Two => (TECH_RPG_DAMAGE_TYPE, TECH_RPG_DEATH_TYPE),
                };
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        dt_name,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        death_name,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.technical_residual_fires = self.technical_residual_fires.saturating_add(1);
        self.technical_residual_units_hit = self.technical_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TECH_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    /// Record Technical residual passenger load honesty (Enter residual).
    pub fn record_technical_residual_load(&mut self) {
        self.technical_residual_loads = self.technical_residual_loads.saturating_add(1);
    }

    /// Record Technical residual passenger unload honesty.
    pub fn record_technical_residual_unload(&mut self) {
        self.technical_residual_unloads = self.technical_residual_unloads.saturating_add(1);
    }

    /// Infer Marauder salvage tier from residual weapon reload / upgrade tags.
    pub(in super::super::super) fn marauder_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_marauder::MarauderWeaponTier {
        use crate::game_logic::host_marauder::MarauderWeaponTier;
        if obj.test_weapon_condition(5)
            || obj.has_upgrade_tag("MarauderCrateUpgradeTwo")
        {
            return MarauderWeaponTier::Two;
        }
        if obj.test_weapon_condition(4)
            || obj.has_upgrade_tag("MarauderCrateUpgradeOne")
        {
            return MarauderWeaponTier::One;
        }
        // Infer from reload residual when tags absent (faster = higher tier).
        if let Some(w) = obj.weapon.as_ref() {
            // Tier2 ~23/30s, Tier1 ~45/30s, Base ~60/30s.
            if w.reload_time <= (23.0 / 30.0) + 0.02 {
                return MarauderWeaponTier::Two;
            }
            if w.reload_time <= (45.0 / 30.0) + 0.02 {
                return MarauderWeaponTier::One;
            }
        }
        MarauderWeaponTier::Base
    }

    /// Apply BlackNapalm residual to an Inferno Cannon (PLAYER_UPGRADE fire field residual).
    ///
    /// Tags the unit so subsequent shell impacts spawn FireFieldUpgradedSmall
    /// residual (7.5 dmg/tick). Fail-closed: not HistoricBonus Firestorm matrix.
    pub fn apply_inferno_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::UPGRADE_CHINA_BLACK_NAPALM;
        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_inferno_cannon_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        self.inferno_black_napalm_residual_upgrades = self
            .inferno_black_napalm_residual_upgrades
            .saturating_add(1);
        true
    }

    /// True when the authored primary weapon is `DragonTankFlameWeapon(/Upgraded)`.
    pub(in super::super::super) fn has_authored_dragon_flame_weapon(obj: &Object) -> bool {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_TANK_FLAME_WEAPON, DRAGON_TANK_FLAME_WEAPON_UPGRADED,
        };
        obj.authored_weapon_name_for_slot(0).is_some_and(|w| {
            w.eq_ignore_ascii_case(DRAGON_TANK_FLAME_WEAPON)
                || w.eq_ignore_ascii_case(DRAGON_TANK_FLAME_WEAPON_UPGRADED)
        })
    }

    /// Apply BlackNapalm residual to a Dragon Tank (PLAYER_UPGRADE flame residual).
    pub fn apply_dragon_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_TANK_FLAME_WEAPON, DRAGON_TANK_FLAME_WEAPON_UPGRADED,
            UPGRADE_CHINA_BLACK_NAPALM, dragon_flame_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        // Black Napalm is a WeaponBonusUpgrade on the authored Dragon flame weapon,
        // not a chassis-name special case.
        if !Self::has_authored_dragon_flame_weapon(obj) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        let _ = obj.replace_weapon_set_slot(0, Some(dragon_flame_weapon(true)));
        self.dragon_tank_residual_black_napalm_upgrades = self
            .dragon_tank_residual_black_napalm_upgrades
            .saturating_add(1);
        true
    }

    /// Authored-weapon lane for Chain Guns: `Some(true)` for `GattlingTankGun`,
    /// `Some(false)` for `GattlingBuildingGun`, `None` otherwise.
    pub(in super::super::super) fn gattling_chain_guns_lane(obj: &Object) -> Option<bool> {
        use crate::game_logic::host_gattling_tank::GATTLING_TANK_GUN;
        use crate::game_logic::host_overlord_addons::GATTLING_BUILDING_GUN;
        let w = obj.authored_weapon_name_for_slot(0)?;
        if w.eq_ignore_ascii_case(GATTLING_TANK_GUN) {
            Some(true)
        } else if w.eq_ignore_ascii_case(GATTLING_BUILDING_GUN) {
            Some(false)
        } else {
            None
        }
    }

    /// Apply Chain Guns residual to a Gattling Tank or Gattling Cannon structure
    /// (PLAYER_UPGRADE damage residual × 1.25).
    pub fn apply_gattling_chain_guns_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_base_defense::{
            gattling_building_air_weapon, gattling_building_ground_weapon,
            is_authored_gattling_building,
        };
        use crate::game_logic::host_gattling_tank::{
            GattlingFireLevel, UPGRADE_CHINA_CHAIN_GUNS, gattling_air_weapon,
            gattling_ground_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        // Chain Guns is a WeaponBonusUpgrade on the authored Gattling weapon,
        // not a chassis-name special case.
        let Some(is_tank) = Self::gattling_chain_guns_lane(obj) else {
            return false;
        };
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        let level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        if is_tank {
            // Chain Guns is a C++ WeaponBonusUpgrade, so it must retain the
            // live weapon/barrel state rather than rebind the WeaponSet.
            obj.weapon = Some(gattling_ground_weapon(level, true));
            obj.secondary_weapon = Some(gattling_air_weapon(level, true));
            self.gattling_tank_residual_chain_gun_upgrades = self
                .gattling_tank_residual_chain_gun_upgrades
                .saturating_add(1);
        } else {
            obj.weapon = Some(gattling_building_ground_weapon(level, true));
            obj.secondary_weapon = Some(gattling_building_air_weapon(level, true));
            self.gattling_building_residual_chain_gun_upgrades = self
                .gattling_building_residual_chain_gun_upgrades
                .saturating_add(1);
        }
        true
    }

    /// Advance structure Gattling Cannon continuous-fire ramp residual after a shot.
    pub(in super::super::super) fn advance_gattling_building_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_base_defense::{
            GATTLING_BUILDING_RAPID_FIRE_AUDIO, gattling_building_air_weapon,
            gattling_building_coast_until_after_shot, gattling_building_ground_weapon,
            gattling_building_has_chain_guns, gattling_building_on_shot_fired,
        };
        use crate::game_logic::host_gattling_tank::GattlingFireLevel;

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.consecutive_shots_at_target;
        let prev_victim = obj.consecutive_shot_target.map(|id| id.0);
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;

        let (new_level, consecutive, entered_fast) = gattling_building_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        let chain = gattling_building_has_chain_guns(&obj.applied_upgrades);
        obj.continuous_fire_level = new_level.as_u8();
        obj.consecutive_shots_at_target = consecutive;
        // C++ coolDown resets the victim together with the count
        // (FiringTracker.cpp:319-321); consecutive == 0 marks that reset.
        obj.consecutive_shot_target = if consecutive == 0 { None } else { target_id };
        // C++ m_frameToStartCooldown = getPossibleNextShotFrame() + coast
        // (FiringTracker.cpp:106-110): the next-shot frame was stamped with
        // the FIRED shot's (pre-promotion) delay (Weapon.cpp:2645-2647;
        // speedUp runs after firing), so the deadline uses prev_level.
        obj.continuous_fire_coast_until_frame =
            gattling_building_coast_until_after_shot(frame, prev_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_building_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_building_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }

        if slot == 1 {
            self.gattling_building_residual_aa_fires =
                self.gattling_building_residual_aa_fires.saturating_add(1);
        } else {
            self.gattling_building_residual_ground_fires = self
                .gattling_building_residual_ground_fires
                .saturating_add(1);
        }
        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.gattling_building_residual_ramp_mean =
                self.gattling_building_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.gattling_building_residual_ramp_fast =
                self.gattling_building_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                GATTLING_BUILDING_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
    }

    /// Advance Gattling continuous-fire ramp residual after a successful shot.
    pub(in super::super::super) fn advance_gattling_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_gattling_tank::{
            GATTLING_RAPID_FIRE_AUDIO, GattlingFireLevel, gattling_air_weapon,
            gattling_coast_until_after_shot, gattling_ground_weapon, gattling_on_shot_fired,
            has_chain_guns_upgrade,
        };

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.consecutive_shots_at_target;
        let prev_victim = obj.consecutive_shot_target.map(|id| id.0);
        let new_victim = target_id.map(|id| id.0);
        let coast_until = obj.continuous_fire_coast_until_frame;

        let (new_level, consecutive, entered_fast) = gattling_on_shot_fired(
            prev_level,
            prev_consec,
            prev_victim,
            new_victim,
            frame,
            coast_until,
        );

        let chain = has_chain_guns_upgrade(&obj.applied_upgrades);
        obj.continuous_fire_level = new_level.as_u8();
        obj.consecutive_shots_at_target = consecutive;
        // C++ coolDown resets the victim together with the count
        // (FiringTracker.cpp:319-321); consecutive == 0 marks that reset.
        obj.consecutive_shot_target = if consecutive == 0 { None } else { target_id };
        // C++ m_frameToStartCooldown = getPossibleNextShotFrame() + coast
        // (FiringTracker.cpp:106-110): the next-shot frame was stamped with
        // the FIRED shot's (pre-promotion) delay (Weapon.cpp:2645-2647;
        // speedUp runs after firing), so the deadline uses prev_level.
        obj.continuous_fire_coast_until_frame = gattling_coast_until_after_shot(frame, prev_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }

        if slot == 1 {
            self.gattling_tank_residual_aa_fires =
                self.gattling_tank_residual_aa_fires.saturating_add(1);
        } else {
            self.gattling_tank_residual_ground_fires =
                self.gattling_tank_residual_ground_fires.saturating_add(1);
        }
        if new_level == GattlingFireLevel::Mean && prev_level != GattlingFireLevel::Mean {
            self.gattling_tank_residual_ramp_mean =
                self.gattling_tank_residual_ramp_mean.saturating_add(1);
        }
        let became_fast = entered_fast
            || (new_level == GattlingFireLevel::Fast && prev_level != GattlingFireLevel::Fast);
        if became_fast {
            self.gattling_tank_residual_ramp_fast =
                self.gattling_tank_residual_ramp_fast.saturating_add(1);
            // C++ FiringTracker::speedUp MEAN→FAST: getPerUnitSound("VoiceRapidFire") + setObjectID.
            self.queue_resolved_per_unit_sound(
                attacker_id,
                GATTLING_RAPID_FIRE_AUDIO,
                true,
                false,
                None,
                140,
            );
        }
    }

    /// Apply Dragon Tank flame residual at impact (primary + secondary splash).
    ///
    /// Returns (units_hit, any_destroyed).
    pub(in super::super::super) fn apply_dragon_flame_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_dragon_flame_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in super::super::super) fn apply_dragon_flame_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_dragon_tank::{
            DRAGON_DAMAGE_TYPE, DRAGON_DEATH_TYPE, DRAGON_FIRE_AUDIO, DRAGON_SECONDARY_RADIUS,
            dragon_flame_damage_at, has_black_napalm_upgrade, is_legal_dragon_flame_target,
        };

        let upgraded = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| has_black_napalm_upgrade(&o.applied_upgrades))
            .unwrap_or(false);
        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_dragon_flame_target(
                    obj.is_alive(),
                    false,
                    obj.status.under_construction,
                    combat_kind,
                ) {
                    return None;
                }
                let pos = obj.get_position();
                let dist = {
                    let dx = impact_xz.0 - pos.x;
                    let dz = impact_xz.1 - pos.z;
                    (dx * dx + dz * dz).sqrt()
                };
                let is_intended = intended_target == Some(*id);
                if is_intended || dist <= DRAGON_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = dragon_flame_damage_at(upgraded, is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let receiving_damage_target_id = obj.id;
                let destroyed = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_target_id,
                    dmg,
                    source,
                    crate::game_logic::host_armor_residual::host_damage_type_from_residual_name(
                        DRAGON_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        DRAGON_DEATH_TYPE,
                    ),
                    None,
                    presentation_effects,
                );
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction_with_effects(id, killer, presentation_effects);
        }

        self.dragon_tank_residual_fires = self.dragon_tank_residual_fires.saturating_add(1);
        self.dragon_tank_residual_units_hit =
            self.dragon_tank_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(DRAGON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }
}

//! Host combat `impl GameLogic` — `ocl_and_scud`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {

    pub fn apply_nuke_cannon_primary_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        source_team: Team,
    ) -> (u32, bool) {
        use crate::game_logic::host_nuke_cannon::{
            is_legal_nuke_cannon_splash_target, nuke_cannon_primary_damage_at,
            nuke_cannon_splash_radius, MEDIUM_RADIATION_AUDIO, NUKE_CANNON_FIRE_AUDIO,
        };

        let impact_xz = (impact.x, impact.z);
        let max_r = nuke_cannon_splash_radius();
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_nuke_cannon_splash_target(
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
                if dist <= max_r {
                    Some((*id, dist))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist) in candidates {
            let dmg = nuke_cannon_primary_damage_at(dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from(dmg, source);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.nuke_cannon_residual.record_primary_blast(hits);

        // Medium radiation field residual at impact.
        let source_id = source.unwrap_or(ObjectId(0));
        let _ = self.nuke_cannon_residual.spawn_radiation_zone(
            source_id,
            source_team,
            impact,
            self.frame,
        );
        self.queue_audio_event(
            AudioEventRequest::new(MEDIUM_RADIATION_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        self.queue_audio_event(
            AudioEventRequest::new(NUKE_CANNON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(160),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                None,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Overlord/Helix portable gattling residual at impact.
    ///
    /// Slot 1 = AA secondary only. Slot 0 = primary weapon_damage path already
    /// handled? Residual: passenger ground gattling damage + when slot 1 AA only.
    /// For simplicity host residual: slot 1 deals AA dmg; slot 0 deals ground
    /// gattling passenger residual (primary tank gun still dealt via weapon_damage
    /// when not taking this exclusive branch). This branch is exclusive — so for
    /// slot 0 we deal both OverlordTankGun residual damage (weapon_damage) AND
    /// passenger gattling ground damage.
    pub(crate) fn apply_overlord_gattling_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            is_legal_overlord_gattling_target, overlord_gattling_ground_damage,
            OVERLORD_GATTLING_AIR_DAMAGE, OVERLORD_GATTLING_FIRE_AUDIO,
        };

        let (chain, primary_dmg) = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| {
                let chain = has_chain_guns_upgrade(&o.applied_upgrades);
                let primary = o.weapon.as_ref().map(|w| w.damage).unwrap_or(80.0);
                (chain, primary)
            })
            .unwrap_or((false, 80.0));

        let (dmg, is_aa) = if slot == 1 {
            let mult = if chain { 1.25 } else { 1.0 };
            (OVERLORD_GATTLING_AIR_DAMAGE * mult, true)
        } else {
            // Primary tank/minigun residual + passenger ground gattling residual.
            (primary_dmg + overlord_gattling_ground_damage(chain), false)
        };

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: obj.team == Team::Neutral,
                            under_construction: obj.status.under_construction,
                            combat_kind,
                            effectively_stealthed: obj.is_effectively_stealthed(),
                            is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                            eject_invulnerable: obj.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| {
                        is_legal_overlord_gattling_target(
                            true,
                            false,
                            c.under_construction,
                            c.combat_kind,
                        )
                    },
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        if is_aa {
                            self.overlord_addons.record_gattling_aa_fire(0);
                        } else {
                            self.overlord_addons.record_gattling_ground_fire(0);
                        }
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            let combat_kind = obj.is_kind_of(KindOf::Attackable)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry)
                || obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Aircraft);
            if is_legal_overlord_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                combat_kind,
            ) {
                let destroyed = obj.take_damage_from(dmg, source);
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        if is_aa {
            self.overlord_addons.record_gattling_aa_fire(hits);
        } else {
            self.overlord_addons.record_gattling_ground_fire(hits);
        }

        let muzzle = source
            .and_then(|id| self.objects.get(&id).map(|o| o.get_position()))
            .unwrap_or(impact);
        self.queue_audio_event(
            AudioEventRequest::new(OVERLORD_GATTLING_FIRE_AUDIO)
                .with_position(muzzle)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                muzzle,
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Residual honesty: Overlord/Helix gattling addon install + fire path.
    pub fn honesty_overlord_gattling_ok(&self) -> bool {
        self.overlord_addons.honesty_gattling_install_ok()
            && self.overlord_addons.honesty_gattling_fire_ok()
    }

    /// Residual honesty: Overlord/Helix/Emperor propaganda addon residual.
    pub fn honesty_overlord_propaganda_ok(&self) -> bool {
        self.overlord_addons.honesty_propaganda_install_ok() || self.honesty_propaganda_heal_ok()
    }

    /// Residual honesty: Helix transport residual.
    pub fn honesty_helix_transport_ok(&self) -> bool {
        self.overlord_addons.honesty_helix_transport_ok()
    }

    pub fn overlord_addons(
        &self,
    ) -> &crate::game_logic::host_overlord_addons::HostOverlordAddonRegistry {
        &self.overlord_addons
    }

    /// Residual honesty: Nuke Cannon primary area + radiation residual.
    pub fn honesty_nuke_cannon_primary_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_primary_ok()
    }

    pub fn honesty_nuke_cannon_radiation_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_radiation_ok()
    }

    pub fn honesty_nuke_cannon_ok(&self) -> bool {
        self.nuke_cannon_residual.honesty_host_path_ok()
    }

    pub fn nuke_cannon_residual(
        &self,
    ) -> &crate::game_logic::host_nuke_cannon::HostNukeCannonRegistry {
        &self.nuke_cannon_residual
    }

    /// Infer Technical salvage tier from residual weapon stats / upgrade tags.
    pub(in super::super) fn technical_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_technical::TechnicalWeaponTier {
        use crate::game_logic::host_technical::TechnicalWeaponTier;
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO")
            || obj.has_upgrade_tag("TechnicalCrateUpgradeTwo")
        {
            return TechnicalWeaponTier::Two;
        }
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE")
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
            delay_frames_to_reload_secs, is_technical_template, technical_weapon_for_tier,
            technical_weapon_name_for_tier, technical_weapon_stats, TECHNICAL_TRANSPORT_SLOTS,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_technical_template(&obj.template_name) {
            return false;
        }

        // Ensure passenger residual capacity is installed.
        if !obj.is_technical_style_container() {
            obj.install_technical_transport();
        }
        let _ = TECHNICAL_TRANSPORT_SLOTS;

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
        obj.weapon = Some(weapon);
        obj.record_host_weapon_stats();

        // Tag residual crate upgrade for tier inference.
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_technical::TechnicalWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_technical::TechnicalWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_technical::TechnicalWeaponTier::Base => {}
        }

        self.technical_residual_weapon_upgrades =
            self.technical_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Technical residual fire (MG direct or cannon/RPG splash).
    pub(in super::super) fn apply_technical_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_technical::{
            is_legal_technical_splash_target, is_technical_template, technical_cannon_scatter_aim,
            technical_cannon_scatter_misses_infantry, technical_splash_damage_at,
            technical_weapon_stats, TechnicalWeaponTier, TECH_FIRE_AUDIO,
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
            .iter()
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
                let destroyed = obj.take_damage_from(dmg, source);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.technical_residual_fires = self.technical_residual_fires.saturating_add(1);
        self.technical_residual_units_hit = self.technical_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(TECH_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_technical_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

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
    pub(in super::super) fn marauder_tier_from_object(
        obj: &Object,
    ) -> crate::game_logic::host_marauder::MarauderWeaponTier {
        use crate::game_logic::host_marauder::MarauderWeaponTier;
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_TWO")
            || obj.has_upgrade_tag("MarauderCrateUpgradeTwo")
        {
            return MarauderWeaponTier::Two;
        }
        if obj.has_upgrade_tag("WEAPONSET_CRATEUPGRADE_ONE")
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

    /// Apply BlackNapalm residual to a Dragon Tank (PLAYER_UPGRADE flame residual).
    pub fn apply_dragon_black_napalm_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_dragon_tank::{
            dragon_flame_weapon, is_dragon_tank_template, UPGRADE_CHINA_BLACK_NAPALM,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_dragon_tank_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_BLACK_NAPALM.to_string());
        obj.weapon = Some(dragon_flame_weapon(true));
        self.dragon_tank_residual_black_napalm_upgrades = self
            .dragon_tank_residual_black_napalm_upgrades
            .saturating_add(1);
        true
    }

    /// Apply Chain Guns residual to a Gattling Tank or Gattling Cannon structure
    /// (PLAYER_UPGRADE damage residual × 1.25).
    pub fn apply_gattling_chain_guns_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_base_defense::{
            gattling_building_air_weapon, gattling_building_ground_weapon,
            is_gattling_cannon_structure,
        };
        use crate::game_logic::host_gattling_tank::{
            gattling_air_weapon, gattling_ground_weapon, is_gattling_tank_template,
            GattlingFireLevel, UPGRADE_CHINA_CHAIN_GUNS,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let is_tank = is_gattling_tank_template(&obj.template_name);
        let is_building = is_gattling_cannon_structure(&obj.template_name);
        if !is_tank && !is_building {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
        let level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        if is_tank {
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
    pub(in super::super) fn advance_gattling_building_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_base_defense::{
            gattling_building_air_weapon, gattling_building_coast_until_after_shot,
            gattling_building_ground_weapon, gattling_building_has_chain_guns,
            gattling_building_on_shot_fired, GATTLING_BUILDING_RAPID_FIRE_AUDIO,
        };
        use crate::game_logic::host_gattling_tank::GattlingFireLevel;

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.continuous_fire_consecutive;
        let prev_victim = if obj.continuous_fire_victim == 0 {
            None
        } else {
            Some(obj.continuous_fire_victim)
        };
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
        obj.record_host_continuous_fire();
        obj.continuous_fire_consecutive = consecutive;
        obj.continuous_fire_victim = new_victim.unwrap_or(0);
        obj.continuous_fire_coast_until_frame =
            gattling_building_coast_until_after_shot(frame, new_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_building_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        obj.record_host_weapon_stats();
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_building_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }
        obj.record_host_weapon_stats();

        let pos = obj.get_position();

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
            self.queue_audio_event(
                AudioEventRequest::new(GATTLING_BUILDING_RAPID_FIRE_AUDIO)
                    .with_object(attacker_id)
                    .with_position(pos)
                    .with_priority(140),
            );
        }
    }

    /// Advance Gattling continuous-fire ramp residual after a successful shot.
    pub(in super::super) fn advance_gattling_continuous_fire(
        &mut self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        slot: u8,
    ) {
        use crate::game_logic::host_gattling_tank::{
            gattling_air_weapon, gattling_coast_until_after_shot, gattling_ground_weapon,
            gattling_on_shot_fired, has_chain_guns_upgrade, GattlingFireLevel,
            GATTLING_RAPID_FIRE_AUDIO,
        };

        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&attacker_id) else {
            return;
        };
        let prev_level = GattlingFireLevel::from_u8(obj.continuous_fire_level);
        let prev_consec = obj.continuous_fire_consecutive;
        let prev_victim = if obj.continuous_fire_victim == 0 {
            None
        } else {
            Some(obj.continuous_fire_victim)
        };
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
        obj.record_host_continuous_fire();
        obj.continuous_fire_consecutive = consecutive;
        obj.continuous_fire_victim = new_victim.unwrap_or(0);
        obj.continuous_fire_coast_until_frame = gattling_coast_until_after_shot(frame, new_level);

        // Rebind weapons with ramped reload residual.
        if let Some(w) = obj.weapon.as_mut() {
            let refreshed = gattling_ground_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = false;
            w.can_target_ground = true;
        }
        obj.record_host_weapon_stats();
        if let Some(w) = obj.secondary_weapon.as_mut() {
            let refreshed = gattling_air_weapon(new_level, chain);
            w.damage = refreshed.damage;
            w.range = refreshed.range;
            w.reload_time = refreshed.reload_time;
            w.can_target_air = true;
            w.can_target_ground = false;
        }
        obj.record_host_weapon_stats();

        let pos = obj.get_position();

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
            self.queue_audio_event(
                AudioEventRequest::new(GATTLING_RAPID_FIRE_AUDIO)
                    .with_object(attacker_id)
                    .with_position(pos)
                    .with_priority(140),
            );
        }
    }

    /// Apply Dragon Tank flame residual at impact (primary + secondary splash).
    ///
    /// Returns (units_hit, any_destroyed).
    pub(in super::super) fn apply_dragon_flame_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_dragon_tank::{
            dragon_flame_damage_at, has_black_napalm_upgrade, is_legal_dragon_flame_target,
            DRAGON_FIRE_AUDIO, DRAGON_SECONDARY_RADIUS,
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
            .iter()
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
                let destroyed = obj.take_damage_from(dmg, source);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.dragon_tank_residual_fires = self.dragon_tank_residual_fires.saturating_add(1);
        self.dragon_tank_residual_units_hit =
            self.dragon_tank_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(DRAGON_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Gattling Tank residual hit at impact (single-target residual).
    ///
    /// Returns (units_hit, any_destroyed). Continuous-fire ramp advances in
    /// `advance_gattling_continuous_fire` after the fire path records last_fire_time.
    pub(in super::super) fn apply_gattling_tank_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        slot: u8,
    ) -> (u32, bool) {
        use crate::game_logic::host_gattling_tank::{
            gattling_damage_with_chain_guns, has_chain_guns_upgrade, is_legal_gattling_target,
            GATTLING_AIR_DAMAGE, GATTLING_FIRE_AUDIO, GATTLING_GROUND_DAMAGE,
        };

        let (base_dmg, chain) = source
            .and_then(|id| self.objects.get(&id))
            .map(|o| {
                let chain = has_chain_guns_upgrade(&o.applied_upgrades);
                let base = if slot == 1 {
                    GATTLING_AIR_DAMAGE
                } else {
                    GATTLING_GROUND_DAMAGE
                };
                (base, chain)
            })
            .unwrap_or((GATTLING_GROUND_DAMAGE, false));
        let dmg = gattling_damage_with_chain_guns(base_dmg, chain);

        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();
        let source_team = source.and_then(|id| self.objects.get(&id).map(|o| o.team));

        let tid = match intended_target {
            Some(id) => id,
            None => {
                // Pure residual acquire: nearest combat target near impact (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&id, obj)| {
                        let combat_kind =
                            crate::game_logic::host_residual_acquire::residual_combat_kind(
                                obj.is_kind_of(KindOf::Attackable),
                                obj.is_kind_of(KindOf::Structure),
                                obj.is_kind_of(KindOf::Infantry),
                                obj.is_kind_of(KindOf::Vehicle),
                                obj.is_kind_of(KindOf::Aircraft),
                            );
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: obj.team == Team::Neutral,
                            under_construction: obj.status.under_construction,
                            combat_kind,
                            effectively_stealthed: obj.is_effectively_stealthed(),
                            is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                            eject_invulnerable: obj.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                match crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    source,
                    (impact.x, impact.z),
                    candidates,
                    12.0,
                    |c| is_legal_gattling_target(true, false, c.under_construction, c.combat_kind),
                ) {
                    Some((id, _, _)) => id,
                    None => {
                        self.queue_audio_event(
                            AudioEventRequest::new(GATTLING_FIRE_AUDIO)
                                .with_position(impact)
                                .with_priority(140),
                        );
                        return (0, false);
                    }
                }
            }
        };

        if let Some(obj) = self.objects.get_mut(&tid) {
            if is_legal_gattling_target(
                obj.is_alive(),
                source == Some(tid),
                obj.status.under_construction,
                true,
            ) {
                let destroyed = obj.take_damage_from(dmg, source);
                hits = 1;
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((tid, source_team));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.queue_audio_event(
            AudioEventRequest::new(GATTLING_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(140),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                Some(tid),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply residual salvage fire-rate tier to a Marauder (crate upgrade residual).
    ///
    /// Fail-closed: not full SalvageCrate collate / W3D turret subobject swap.
    pub fn apply_marauder_weapon_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_marauder::MarauderWeaponTier,
    ) -> bool {
        use crate::game_logic::host_marauder::{
            delay_frames_to_reload_secs, is_marauder_template, marauder_weapon_for_tier,
            marauder_weapon_name_for_tier, marauder_weapon_stats,
        };
        use crate::game_logic::thing::ThingTemplate;

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_marauder_template(&obj.template_name) {
            return false;
        }

        let name = marauder_weapon_name_for_tier(tier);
        let (dmg, range, delay, _splash, speed) = marauder_weapon_stats(tier);
        let mut weapon = ThingTemplate::weapon_from_store(name)
            .unwrap_or_else(|| marauder_weapon_for_tier(tier));
        weapon.damage = dmg;
        weapon.range = range;
        weapon.min_range = 0.0;
        weapon.reload_time = delay_frames_to_reload_secs(delay);
        weapon.projectile_speed = speed;
        weapon.can_target_ground = true;
        weapon.can_target_air = false;
        obj.weapon = Some(weapon);
        obj.record_host_weapon_stats();

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            crate::game_logic::host_marauder::MarauderWeaponTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            crate::game_logic::host_marauder::MarauderWeaponTier::Base => {}
        }

        self.marauder_residual_weapon_upgrades =
            self.marauder_residual_weapon_upgrades.saturating_add(1);
        true
    }

    /// Apply Marauder residual fire (primary on intended + small splash radius).
    /// C++ MarauderTankShell DumbProjectile residual.
    pub fn spawn_marauder_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        weapon_speed: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_marauder::{
            marauder_shell_flight_frames, MARAUDER_SHELL_MAX_HEALTH, MARAUDER_TANK_SHELL,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(MARAUDER_TANK_SHELL) {
            let mut t = ThingTemplate::new(MARAUDER_TANK_SHELL);
            t.add_kind_of(KindOf::Projectile)
                .set_health(MARAUDER_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(MARAUDER_TANK_SHELL.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on MarauderTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_marauder::marauder_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_marauder::marauder_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_marauder::MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(MARAUDER_TANK_SHELL, team, start)?;
        let frames = marauder_shell_flight_frames(start, aim, weapon_speed).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.marauder_shell_projectile = true;
            o.marauder_shell_from = Some([start.x, start.y, start.z]);
            o.marauder_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.marauder_shell_launch_frame = Some(self.frame);
            o.marauder_shell_flight_frames = frames;
            o.marauder_shell_intended = intended.map(|id| id.0);
            o.marauder_shell_weapon_speed = weapon_speed;
            o.producer_id = Some(source_id);
            o.health.maximum = MARAUDER_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, MARAUDER_SHELL_MAX_HEALTH);
        }
        self.marauder_shells_spawned = self.marauder_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_marauder_shell_projectiles(&mut self) {
        use crate::game_logic::host_marauder::marauder_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.marauder_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, from, aim, launch, frames) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .marauder_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .marauder_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    o.marauder_shell_intended.map(ObjectId),
                    from,
                    aim,
                    o.marauder_shell_launch_frame.unwrap_or(frame),
                    o.marauder_shell_flight_frames.max(1),
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = marauder_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, intended, aim));
            }
        }
        for (id, source, intended, pos) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.marauder_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_marauder_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
    }

    pub fn honesty_marauder_shell_projectile_ok(&self) -> bool {
        self.marauder_shells_spawned > 0
    }

    pub fn apply_marauder_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        use crate::game_logic::host_marauder::{
            is_legal_marauder_splash_target, is_marauder_template, marauder_scatter_aim,
            marauder_scatter_misses_infantry, marauder_splash_damage_at, MARAUDER_FIRE_AUDIO,
            MARAUDER_SPLASH_RADIUS,
        };

        // Fire-rate tier residual is encoded on the weapon; damage is constant across tiers.
        let _tier = source
            .and_then(|sid| self.objects.get(&sid))
            .map(Self::marauder_tier_from_object);
        let source_team = source
            .and_then(|sid| self.objects.get(&sid).map(|o| o.team))
            .unwrap_or(Team::Neutral);

        // C++ MarauderTankGun ScatterRadiusVsInfantry residual on instant apply.
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        if intended_is_infantry {
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
            let (new_impact, scattered) = marauder_scatter_aim(impact, true, seed);
            if scattered {
                self.marauder_scatter_applied = self.marauder_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if marauder_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > MARAUDER_SPLASH_RADIUS {
                        self.marauder_scatter_misses =
                            self.marauder_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let impact_xz = (impact.x, impact.z);
        let mut hits = 0u32;
        let mut any_destroyed = false;
        let mut destroy_ids: Vec<(ObjectId, Option<Team>)> = Vec::new();

        let candidates: Vec<(ObjectId, f32, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if source == Some(*id) {
                    return None;
                }
                let combat_kind = obj.is_kind_of(KindOf::Attackable)
                    || obj.is_kind_of(KindOf::Structure)
                    || obj.is_kind_of(KindOf::Infantry)
                    || obj.is_kind_of(KindOf::Vehicle)
                    || obj.is_kind_of(KindOf::Aircraft);
                if !is_legal_marauder_splash_target(
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
                if is_intended && intended_is_infantry && dist > MARAUDER_SPLASH_RADIUS {
                    return None;
                }
                if is_intended || dist <= MARAUDER_SPLASH_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let dmg = marauder_splash_damage_at(is_intended, dist);
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let destroyed = obj.take_damage_from(dmg, source);
                hits = hits.saturating_add(1);
                if destroyed {
                    any_destroyed = true;
                    destroy_ids.push((id, Some(source_team)));
                }
            }
        }

        for (id, killer) in destroy_ids {
            self.mark_object_for_destruction(id, killer);
        }

        self.marauder_residual_fires = self.marauder_residual_fires.saturating_add(1);
        self.marauder_residual_units_hit = self.marauder_residual_units_hit.saturating_add(hits);

        self.queue_audio_event(
            AudioEventRequest::new(MARAUDER_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        if let Some(sid) = source {
            let _ = self.combat_particles.spawn_weapon_fire_fx(
                self.objects
                    .get(&sid)
                    .map(|o| o.get_position())
                    .unwrap_or(impact),
                Some(impact),
                self.frame,
                sid,
                intended_target,
            );
            let _ = is_marauder_template(
                &self
                    .objects
                    .get(&sid)
                    .map(|o| o.template_name.clone())
                    .unwrap_or_default(),
            );
        }

        (hits, any_destroyed)
    }

    /// Apply Scorpion salvage gun tier residual (primary damage 20 → 25).
    pub fn apply_scorpion_salvage_tier(
        &mut self,
        object_id: ObjectId,
        tier: crate::game_logic::host_scorpion::ScorpionSalvageTier,
    ) -> bool {
        use crate::game_logic::host_scorpion::{
            has_ap_rockets_upgrade, has_scorpion_rocket_upgrade, is_scorpion_template,
            scorpion_gun_weapon, scorpion_missile_weapon, ScorpionSalvageTier,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }

        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_ONE");
        obj.applied_upgrades.remove("WEAPONSET_CRATEUPGRADE_TWO");
        match tier {
            ScorpionSalvageTier::One => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_ONE".to_string());
            }
            ScorpionSalvageTier::Two => {
                obj.applied_upgrades
                    .insert("WEAPONSET_CRATEUPGRADE_TWO".to_string());
            }
            ScorpionSalvageTier::Base => {}
        }

        let last_fire = obj.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let mut gun = scorpion_gun_weapon(tier);
        gun.last_fire_time = last_fire;
        obj.weapon = Some(gun);

        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(ap, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            obj.secondary_weapon = Some(missile);
        }

        self.scorpion_residual_salvage_upgrades =
            self.scorpion_residual_salvage_upgrades.saturating_add(1);
        true
    }

    /// Equip Scorpion Rocket secondary residual (Upgrade_GLAScorpionRocket).
    pub fn apply_scorpion_rocket_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            has_ap_rockets_upgrade, is_scorpion_template, salvage_tier_from_upgrades,
            scorpion_missile_weapon, UPGRADE_GLA_SCORPION_ROCKET,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_SCORPION_ROCKET.to_string());
        let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
        let ap = has_ap_rockets_upgrade(&obj.applied_upgrades);
        obj.secondary_weapon = Some(scorpion_missile_weapon(ap, tier.dual_missile_clip()));
        self.scorpion_residual_rocket_upgrades =
            self.scorpion_residual_rocket_upgrades.saturating_add(1);
        true
    }

    /// Apply AP Rockets residual damage mult to Scorpion missile secondary.
    pub fn apply_scorpion_ap_rockets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_scorpion::{
            has_scorpion_rocket_upgrade, is_scorpion_template, salvage_tier_from_upgrades,
            scorpion_missile_weapon, UPGRADE_GLA_AP_ROCKETS,
        };

        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_scorpion_template(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        if has_scorpion_rocket_upgrade(&obj.applied_upgrades) {
            let tier = salvage_tier_from_upgrades(&obj.applied_upgrades);
            let sec_last = obj
                .secondary_weapon
                .as_ref()
                .map(|w| w.last_fire_time)
                .unwrap_or(0.0);
            let mut missile = scorpion_missile_weapon(true, tier.dual_missile_clip());
            missile.last_fire_time = sec_last;
            obj.secondary_weapon = Some(missile);
        }
        true
    }

    /// C++ ScorpionTankShell DumbProjectile residual.
    pub fn spawn_scorpion_shell_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_scorpion::{
            scorpion_shell_flight_frames, SCORPION_SHELL_MAX_HEALTH, SCORPION_TANK_SHELL,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCORPION_TANK_SHELL) {
            let mut t = ThingTemplate::new(SCORPION_TANK_SHELL);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCORPION_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCORPION_TANK_SHELL.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on ScorpionTankGun vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_scorpion::scorpion_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_scorpion::scorpion_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_scorpion::SCORPION_GUN_SPLASH_RADIUS {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 4.0;
        let pid = self.create_object(SCORPION_TANK_SHELL, team, start)?;
        let frames = scorpion_shell_flight_frames(start, aim).max(1);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scorpion_shell_projectile = true;
            o.scorpion_shell_from = Some([start.x, start.y, start.z]);
            o.scorpion_shell_aim = Some([aim.x, aim.y, aim.z]);
            o.scorpion_shell_launch_frame = Some(self.frame);
            o.scorpion_shell_flight_frames = frames;
            o.scorpion_shell_slot = slot;
            o.producer_id = Some(source_id);
            o.health.maximum = SCORPION_SHELL_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, SCORPION_SHELL_MAX_HEALTH);
        }
        self.scorpion_shells_spawned = self.scorpion_shells_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_scorpion_shell_projectiles(&mut self) {
        use crate::game_logic::host_scorpion::scorpion_shell_bezier_point;
        let frame = self.frame;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.scorpion_shell_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, glam::Vec3, u8)> = Vec::new();
        for id in flying {
            let (source, from, aim, launch, frames, slot) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let from = o
                    .scorpion_shell_from
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let aim = o
                    .scorpion_shell_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or(from);
                (
                    o.producer_id,
                    from,
                    aim,
                    o.scorpion_shell_launch_frame.unwrap_or(frame),
                    o.scorpion_shell_flight_frames.max(1),
                    o.scorpion_shell_slot,
                )
            };
            let elapsed = frame.saturating_sub(launch);
            let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
            let pos = scorpion_shell_bezier_point(from, aim, t);
            if let Some(o) = self.objects.get_mut(&id) {
                let prev = o.get_position();
                o.set_position(pos);
                let d = pos - prev;
                if d.length_squared() > 1.0e-6 {
                    o.set_orientation(d.z.atan2(d.x));
                }
            }
            if elapsed >= frames {
                impact.push((id, source, aim, slot));
            }
        }
        for (id, source, pos, slot) in impact {
            let team = self.objects.get(&id).map(|o| o.team);
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.scorpion_shell_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_scorpion_residual_at(pos, source, None, slot);
            self.mark_object_for_destruction(id, team);
        }
    }

    /// C++ ScorpionMissile ProjectileObject residual.
    pub fn spawn_scorpion_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        slot: u8,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_scorpion::{
            SCORPION_MISSILE, SCORPION_MISSILE_FUEL_FRAMES, SCORPION_MISSILE_INITIAL_VELOCITY,
            SCORPION_MISSILE_MAX_HEALTH, SCORPION_MISSILE_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCORPION_MISSILE) {
            let mut t = ThingTemplate::new(SCORPION_MISSILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCORPION_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCORPION_MISSILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on ScorpionMissileWeapon vs infantry.
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) =
            crate::game_logic::host_scorpion::scorpion_scatter_aim(aim, target_is_infantry, seed);
        if scattered {
            self.scorpion_scatter_applied = self.scorpion_scatter_applied.saturating_add(1);
        }
        if target_is_infantry {
            let hit_r = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| {
                    if o.selection_radius > 0.0 {
                        o.selection_radius
                    } else {
                        crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS
                    }
                })
                .unwrap_or(crate::game_logic::weapon_bootstrap::DEFAULT_SCATTER_HIT_RADIUS);
            let intended_pos = intended
                .and_then(|id| self.objects.get(&id))
                .map(|o| o.get_position());
            if crate::game_logic::host_scorpion::scorpion_scatter_misses_infantry(true, seed, hit_r)
            {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_scorpion::SCORPION_MISSILE_SECONDARY_RADIUS {
                        self.scorpion_scatter_misses =
                            self.scorpion_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 6.0;
        let pid = self.create_object(SCORPION_MISSILE, team, start)?;
        let launch = SCORPION_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scorpion_missile_projectile = true;
            o.scorpion_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.scorpion_missile_intended = intended.map(|id| id.0);
            o.scorpion_missile_travelled = 0.0;
            o.scorpion_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(SCORPION_MISSILE_FUEL_FRAMES));
            o.scorpion_missile_slot = slot;
            o.producer_id = Some(source_id);
            o.health.maximum = SCORPION_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, SCORPION_MISSILE_MAX_HEALTH);
            o.movement.velocity = dir * launch;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        let _ = SCORPION_MISSILE_PROJECTILE_SPEED; // cruise used in update
        self.scorpion_missiles_spawned = self.scorpion_missiles_spawned.saturating_add(1);
        Some(pid)
    }
}

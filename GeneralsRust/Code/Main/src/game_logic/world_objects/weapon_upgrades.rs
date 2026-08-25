//! Host objects `impl GameLogic` — `weapon_upgrades`.
//! apply_*_to_team weapon upgrades. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use std::cell::Cell;

// Upgrade completion reaches many C++ `giveUpgrade` residuals which retain a
// team-shaped public API.  The authoritative completion path supplies a
// PlayerId, so scope that identity for the synchronous fan-out rather than
// letting every routine select all same-faction slots.  The guard restores the
// previous scope on every exit, including an early return or panic.
thread_local! {
    static ACTIVE_UPGRADE_OWNER: Cell<Option<u32>> = Cell::new(None);
}

pub(super) struct UpgradeOwnerScope {
    previous: Option<u32>,
}

pub(super) fn enter_upgrade_owner_scope(player_id: u32) -> UpgradeOwnerScope {
    let previous = ACTIVE_UPGRADE_OWNER.with(|owner| owner.replace(Some(player_id)));
    UpgradeOwnerScope { previous }
}

impl Drop for UpgradeOwnerScope {
    fn drop(&mut self) {
        ACTIVE_UPGRADE_OWNER.with(|owner| owner.set(self.previous));
    }
}

/// `None` means an old direct team-only caller, which retains legacy behavior.
/// A live player-owned completion requires exact ownership; an unowned object
/// is intentionally not guessed to belong to one of two same-faction players.
#[inline]
pub(super) fn upgrade_targets_object(object: &Object, team: Team) -> bool {
    object.team == team
        && ACTIVE_UPGRADE_OWNER.with(|owner| match owner.get() {
            Some(player_id) => object.owner_player_id == Some(player_id),
            None => true,
        })
}

#[inline]
pub(super) fn upgrade_targets_player(player: &Player, team: Team) -> bool {
    player.team == team
        && ACTIVE_UPGRADE_OWNER.with(|owner| match owner.get() {
            Some(player_id) => player.id == player_id,
            None => true,
        })
}

#[inline]
pub(super) fn active_upgrade_owner() -> Option<u32> {
    ACTIVE_UPGRADE_OWNER.with(|owner| owner.get())
}

impl GameLogic {
    /// C++ CashBountyPower / SCIENCE_CashBounty residual via upgrade complete.
    /// Tag Helix casters with Napalm/Nuke bomb upgrade residual unlock.
    pub(in super::super) fn apply_helix_bomb_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
        canonical_tag: &str,
    ) -> u32 {
        use crate::game_logic::host_helix_napalm::is_helix_napalm_caster;
        let mut n = 0u32;
        let ids: Vec<_> = self.objects.keys().copied().collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_helix_napalm_caster(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(canonical_tag) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(canonical_tag);
            if upgrade_name != canonical_tag {
                obj.apply_upgrade_tag(upgrade_name);
            }
            n = n.saturating_add(1);
        }
        // Player unlock residual for science/UI gates.
        let player = match active_upgrade_owner() {
            Some(player_id) => self.get_player_mut(player_id),
            None => self.get_player_mut_by_team(team),
        };
        if let Some(p) = player {
            p.unlocked_sciences.insert(canonical_tag.to_string());
            if upgrade_name != canonical_tag {
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    pub(in super::super) fn apply_cash_bounty_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science;

        let pct = cash_bounty_percent_for_science(upgrade_name).unwrap_or(0.05);
        let mut n = 0u32;
        let mut player_ids = Vec::new();
        for (id, p) in self.players.iter_mut() {
            if !upgrade_targets_player(p, team) {
                continue;
            }
            p.unlocked_sciences.insert(upgrade_name.to_string());
            // SCIENCE names for kill path residual.
            if pct >= 0.20 - f32::EPSILON {
                p.unlocked_sciences
                    .insert("SCIENCE_CashBounty3".to_string());
            } else if pct >= 0.10 - f32::EPSILON {
                p.unlocked_sciences
                    .insert("SCIENCE_CashBounty2".to_string());
            } else {
                p.unlocked_sciences
                    .insert("SCIENCE_CashBounty1".to_string());
            }
            player_ids.push(*id);
            n = n.saturating_add(1);
        }
        // C++ CashBountyPower::onSpecialPowerCreation only runs on a palace module.
        for player_id in player_ids {
            let _ = self.apply_cash_bounty_from_palace_modules(player_id, Some(upgrade_name));
        }
        n
    }

    /// C++ America Scout/Battle/Hellfire drone object-upgrade residual.
    ///
    /// Attaches the residual slave drone to each living master vehicle that does
    /// not already have the upgrade tag (ObjectCreationUpgrade attach residual)
    /// and is not ConflictsWith another drone (leftover UpgradeMux::wouldUpgrade).
    pub(in super::super) fn apply_slave_drone_upgrade_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_slave_drones::{
            SlaveDroneKind, is_slave_drone_master_template, slave_drone_conflicts_with_owned,
        };

        let kind = SlaveDroneKind::from_upgrade_name(upgrade_name).unwrap_or(SlaveDroneKind::Scout);
        let tag = kind.upgrade_name();

        let masters: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                upgrade_targets_object(o, team)
                    && o.is_alive()
                    && !o.status.under_construction
                    && is_slave_drone_master_template(&o.template_name)
                    && !o.has_upgrade_tag(tag)
                    && !o.has_upgrade_tag(upgrade_name)
                    && !slave_drone_conflicts_with_owned(
                        upgrade_name,
                        o.applied_upgrades.iter().map(String::as_str),
                    )
            })
            .map(|(id, _)| *id)
            .collect();

        let mut n = 0u32;
        for mid in masters {
            if self.residual_attach_slave_drone(mid, kind).is_some() {
                if let Some(m) = self.objects.get_mut(&mid) {
                    m.apply_upgrade_tag(upgrade_name);
                    m.apply_upgrade_tag(tag);
                }
                n = n.saturating_add(1);
            }
        }
        // Player unlock residual for production UI / late builds.
        let _ = self.apply_player_unlock_upgrade(team, upgrade_name, tag);
        n
    }

    /// C++ Upgrade_AmericaChemicalSuits residual — ChemSuitHumanArmor on infantry.
    pub(in super::super) fn apply_chemical_suits_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_CHEMICAL_SUITS;
        let mut n =
            self.apply_player_unlock_upgrade(team, upgrade_name, UPGRADE_AMERICA_CHEMICAL_SUITS);
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Infantry) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_CHEMICAL_SUITS)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_CHEMICAL_SUITS);
            obj.applied_upgrades
                .insert(UPGRADE_AMERICA_CHEMICAL_SUITS.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_ChinaSatelliteHackOne/Two residual — player FOW/intel unlock.
    pub(in super::super) fn apply_satellite_hack_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            UPGRADE_CHINA_SATELLITE_HACK_ONE, UPGRADE_CHINA_SATELLITE_HACK_TWO,
        };
        let lower = upgrade_name.to_ascii_lowercase();
        let canonical = if lower.contains("two") || lower.contains("2") {
            UPGRADE_CHINA_SATELLITE_HACK_TWO
        } else {
            UPGRADE_CHINA_SATELLITE_HACK_ONE
        };
        let n = self.apply_player_unlock_upgrade(team, upgrade_name, canonical);
        // Also unlock both tiers when Two is researched (Two implies One residual).
        if canonical == UPGRADE_CHINA_SATELLITE_HACK_TWO {
            let _ = self.apply_player_unlock_upgrade(
                team,
                UPGRADE_CHINA_SATELLITE_HACK_ONE,
                UPGRADE_CHINA_SATELLITE_HACK_ONE,
            );
        }
        n
    }

    /// C++ Upgrade_AmericaCountermeasures residual — tag aircraft for flare residual.
    pub(in super::super) fn apply_countermeasures_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_COUNTERMEASURES;
        let mut n =
            self.apply_player_unlock_upgrade(team, upgrade_name, UPGRADE_AMERICA_COUNTERMEASURES);
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Aircraft) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_COUNTERMEASURES);
            obj.applied_upgrades
                .insert(UPGRADE_AMERICA_COUNTERMEASURES.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// Generic player-level upgrade unlock residual (mines / radar scan / flags).
    pub(in super::super) fn apply_player_unlock_upgrade(
        &mut self,
        team: Team,
        upgrade_name: &str,
        canonical: &str,
    ) -> u32 {
        let mut n = 0u32;
        for p in self.players.values_mut() {
            if !upgrade_targets_player(p, team) {
                continue;
            }
            p.unlocked_sciences.insert(canonical.to_string());
            p.unlocked_sciences.insert(upgrade_name.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_GLAFortifiedStructure residual — +max health on GLA structures.
    pub(in super::super) fn apply_fortified_structure_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            FORTIFIED_STRUCTURE_ADD_MAX_HEALTH, UPGRADE_GLA_FORTIFIED_STRUCTURE,
        };
        let add = FORTIFIED_STRUCTURE_ADD_MAX_HEALTH;
        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_FORTIFIED_STRUCTURE)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_FORTIFIED_STRUCTURE);
            obj.max_health = (obj.max_health + add).max(1.0);
            obj.record_host_max_health();
            obj.health.maximum = (obj.health.maximum + add).max(1.0);
            let new_hp = (obj.health.current + add).min(obj.health.maximum);
            Self::write_object_health_authority_aware(obj, new_hp);
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_FORTIFIED_STRUCTURE.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ faction RadarUpgrade residual — unlock + tag radar providers.
    pub(in super::super) fn apply_radar_research_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_radar::{UPGRADE_GLA_RADAR, is_radar_provider_template};
        use crate::game_logic::host_structure_economy_residual::UPGRADE_AMERICA_RADAR;

        let canonical = if upgrade_name.to_ascii_lowercase().contains("china") {
            "Upgrade_ChinaRadar"
        } else if upgrade_name.to_ascii_lowercase().contains("gla") {
            UPGRADE_GLA_RADAR
        } else {
            UPGRADE_AMERICA_RADAR
        };

        let mut n = self.apply_player_unlock_upgrade(team, upgrade_name, canonical);
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_radar_provider_template(&obj.template_name)
                && !obj.is_command_center()
                && !obj.is_kind_of(KindOf::CommandCenter)
            {
                continue;
            }
            if obj.has_upgrade_tag(canonical) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(canonical);
            // C++ RadarUpgrade model residual.
            if let Some(bit) =
                crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                    "RADAR_UPGRADED",
                )
            {
                obj.model_condition_bits |= 1u128 << bit;
            }
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_AmericaDroneArmor residual — +max health on slave drones.
    pub(in super::super) fn apply_drone_armor_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_slave_drones::{
            SlaveDroneKind, UPGRADE_AMERICA_DRONE_ARMOR, drone_armor_add_max_health,
            is_battle_drone_template, is_hellfire_drone_template, is_scout_drone_template,
        };

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            let kind = if is_battle_drone_template(&obj.template_name) {
                Some(SlaveDroneKind::Battle)
            } else if is_hellfire_drone_template(&obj.template_name) {
                Some(SlaveDroneKind::Hellfire)
            } else if is_scout_drone_template(&obj.template_name) {
                Some(SlaveDroneKind::Scout)
            } else {
                None
            };
            let Some(kind) = kind else {
                continue;
            };
            if obj.has_upgrade_tag(UPGRADE_AMERICA_DRONE_ARMOR) || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            let add = drone_armor_add_max_health(kind);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_DRONE_ARMOR);
            obj.applied_upgrades
                .insert(UPGRADE_AMERICA_DRONE_ARMOR.to_string());
            obj.max_health = (obj.max_health + add).max(1.0);
            obj.record_host_max_health();
            obj.health.maximum = (obj.health.maximum + add).max(1.0);
            let new_hp = (obj.health.current + add).min(obj.health.maximum);
            Self::write_object_health_authority_aware(obj, new_hp);
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_AMERICA_DRONE_ARMOR.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaAircraftArmor residual — +40 max health on MiGs.
    pub(in super::super) fn apply_aircraft_armor_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_mig::{
            UPGRADE_CHINA_AIRCRAFT_ARMOR, apply_mig_aircraft_armor_health, is_mig_template,
        };

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_mig_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_CHINA_AIRCRAFT_ARMOR)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_AIRCRAFT_ARMOR);
            obj.applied_upgrades
                .insert(UPGRADE_CHINA_AIRCRAFT_ARMOR.to_string());
            let mut max_h = obj.max_health;
            let mut cur = obj.health.current;
            let mut maximum = obj.health.maximum;
            apply_mig_aircraft_armor_health(&mut max_h, &mut cur, &mut maximum);
            obj.set_body_max_health(max_h);
            obj.record_host_max_health();
            Self::write_object_health_authority_aware(obj, cur);
            obj.health.maximum = maximum;
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_AIRCRAFT_ARMOR.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_AmericaAdvancedTraining residual — 2× XP gain player unlock.
    pub(in super::super) fn apply_advanced_training_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_unit_training::UPGRADE_AMERICA_ADVANCED_TRAINING;
        let mut n = 0u32;
        for p in self.players.values_mut() {
            if !upgrade_targets_player(p, team) {
                continue;
            }
            p.unlocked_sciences
                .insert(UPGRADE_AMERICA_ADVANCED_TRAINING.to_string());
            p.unlocked_sciences.insert(upgrade_name.to_string());
            n = n.saturating_add(1);
        }
        // Tag living USA combat units so XP path can read unit tags residual.
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING);
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ Upgrade_ChinaTacticalNukeMig residual — Nuke General MiG loadout.
    pub(in super::super) fn apply_tactical_nuke_mig_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_mig::{UPGRADE_CHINA_TACTICAL_NUKE_MIG, is_nuke_mig_template};
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                upgrade_targets_object(o, team)
                    && o.is_alive()
                    && is_nuke_mig_template(&o.template_name)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if self.apply_mig_tactical_nuke_upgrade(id) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_CHINA_TACTICAL_NUKE_MIG);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_TACTICAL_NUKE_MIG.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAAnthraxBeta residual — toxin tractor + SCUD + scud storm tier.
    pub(in super::super) fn apply_anthrax_beta_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_scud_launcher::{
            UPGRADE_GLA_ANTHRAX_BETA, is_scud_launcher_template,
        };
        use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            let is_tt = is_toxin_tractor_template(&obj.template_name);
            let is_scud = is_scud_launcher_template(&obj.template_name);
            if !is_tt && !is_scud {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_ANTHRAX_BETA);
            obj.applied_upgrades
                .insert(UPGRADE_GLA_ANTHRAX_BETA.to_string());
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_ANTHRAX_BETA.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAToxinShells residual — enables SCUD toxin secondary path.
    pub(in super::super) fn apply_toxin_shells_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_scud_launcher::is_scud_launcher_template;
        use crate::game_logic::host_upgrades::UPGRADE_GLA_TOXIN_SHELLS;

        let mut n = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_scud_launcher_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_TOXIN_SHELLS) || obj.has_upgrade_tag(upgrade_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_TOXIN_SHELLS);
            obj.applied_upgrades
                .insert(UPGRADE_GLA_TOXIN_SHELLS.to_string());
            // Toxin shells residual also unlocks toxin secondary preference.
            n = n.saturating_add(1);
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_TOXIN_SHELLS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAAPBullets residual — Rebel / Jarmen / Technical / Quad.
    pub(in super::super) fn apply_ap_bullets_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_gla_rebel::is_gla_rebel_template;
        use crate::game_logic::host_jarmen_kell::{
            UPGRADE_GLA_AP_BULLETS, is_jarmen_kell_template,
        };
        use crate::game_logic::host_quad_cannon::is_quad_cannon_template;
        use crate::game_logic::host_technical::is_technical_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_jarmen_kell_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_gla_rebel_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_technical_template(&o.template_name) {
                    Some((*id, 2))
                } else if is_quad_cannon_template(&o.template_name) {
                    Some((*id, 3))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            match kind {
                0 => {
                    if self.apply_jarmen_kell_ap_bullets_upgrade(id) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.apply_upgrade_tag(upgrade_name);
                            o.apply_upgrade_tag(UPGRADE_GLA_AP_BULLETS);
                        }
                        n = n.saturating_add(1);
                    }
                }
                1 => {
                    if self.apply_rebel_ap_bullets_upgrade(id) {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.apply_upgrade_tag(upgrade_name);
                            o.apply_upgrade_tag(UPGRADE_GLA_AP_BULLETS);
                        }
                        n = n.saturating_add(1);
                    }
                }
                2 | 3 => {
                    // Technical / Quad: tag residual; damage path reads applied_upgrades.
                    if let Some(o) = self.objects.get_mut(&id) {
                        if !o.has_upgrade_tag(UPGRADE_GLA_AP_BULLETS) {
                            o.apply_upgrade_tag(upgrade_name);
                            o.apply_upgrade_tag(UPGRADE_GLA_AP_BULLETS);
                            o.applied_upgrades
                                .insert(UPGRADE_GLA_AP_BULLETS.to_string());
                            n = n.saturating_add(1);
                        }
                    }
                }
                _ => {}
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_AP_BULLETS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaUraniumShells residual — Battlemaster / Overlord gun damage.
    pub(in super::super) fn apply_uranium_shells_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_battlemaster::{
            UPGRADE_CHINA_URANIUM_SHELLS, is_battlemaster_template,
        };
        use crate::game_logic::host_overlord_gun::is_overlord_gun_chassis;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_battlemaster_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_overlord_gun_chassis(&o.template_name) {
                    Some((*id, 1))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_battlemaster_uranium_upgrade(id),
                1 => self.apply_overlord_gun_uranium_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_CHINA_URANIUM_SHELLS);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_URANIUM_SHELLS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaBlackNapalm residual — MiG / Inferno / Dragon fire field.
    pub(in super::super) fn apply_black_napalm_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_dragon_tank::is_dragon_tank_template;
        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
        use crate::game_logic::host_mig::is_mig_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_mig_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_inferno_cannon_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_dragon_tank_template(&o.template_name) {
                    Some((*id, 2))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_mig_black_napalm_upgrade(id),
                1 => self.apply_inferno_black_napalm_upgrade(id),
                2 => self.apply_dragon_black_napalm_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag("Upgrade_ChinaBlackNapalm");
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert("Upgrade_ChinaBlackNapalm".to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_GLAScorpionRocket residual — equip SECONDARY on all Scorpions.
    pub(in super::super) fn apply_scorpion_rocket_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_scorpion::{UPGRADE_GLA_SCORPION_ROCKET, is_scorpion_template};
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                upgrade_targets_object(o, team)
                    && o.is_alive()
                    && is_scorpion_template(&o.template_name)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if self.apply_scorpion_rocket_upgrade(id) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_GLA_SCORPION_ROCKET);
                }
                n = n.saturating_add(1);
            }
        }
        n
    }

    /// C++ Upgrade_GLAAPRockets residual — AP damage on Scorpions (+ RPG if present).
    /// C++ Upgrade_GLAAPRockets residual — Scorpion / RPG / Stinger AP.
    pub(in super::super) fn apply_ap_rockets_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_base_defense::is_stinger_site_structure;
        use crate::game_logic::host_rpg_trooper::is_rpg_trooper_template;
        use crate::game_logic::host_scorpion::{UPGRADE_GLA_AP_ROCKETS, is_scorpion_template};

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_scorpion_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_rpg_trooper_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_stinger_site_structure(&o.template_name) {
                    Some((*id, 2))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_scorpion_ap_rockets_upgrade(id),
                1 => self.apply_rpg_trooper_ap_rockets_upgrade(id),
                2 => self.apply_stinger_ap_rockets_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_GLA_AP_ROCKETS);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_AmericaLaserMissiles residual — Raptor jet damage.
    pub(in super::super) fn apply_laser_missiles_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_raptor::{UPGRADE_AMERICA_LASER_MISSILES, is_raptor_template};
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                upgrade_targets_object(o, team)
                    && o.is_alive()
                    && is_raptor_template(&o.template_name)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0u32;
        for id in ids {
            if self.apply_raptor_laser_missiles_upgrade(id) {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_AMERICA_LASER_MISSILES);
                }
                n = n.saturating_add(1);
            }
        }
        n
    }

    /// C++ Upgrade_ChinaNationalism residual — horde ROF tag on infantry/tanks.
    /// C++ `evaluateMoraleBonus` runs on every HordeUpdate unit, not just
    /// Battlemaster / Red Guard / Tank Hunter / MiniGunner.
    pub(in super::super) fn apply_nationalism_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_battlemaster::{
            UPGRADE_NATIONALISM, is_battlemaster_template, is_china_horde_update_unit,
        };
        use crate::game_logic::host_minigunner::is_minigunner_template;
        use crate::game_logic::host_red_guard::is_red_guard_template;
        use crate::game_logic::host_tank_hunter::is_tank_hunter_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_battlemaster_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_red_guard_template(&o.template_name) {
                    Some((*id, 1))
                } else if is_tank_hunter_template(&o.template_name) {
                    Some((*id, 2))
                } else if is_minigunner_template(&o.template_name) {
                    Some((*id, 3))
                } else if is_china_horde_update_unit(&o.template_name) {
                    Some((*id, 4))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_battlemaster_nationalism_upgrade(id),
                1 => self.apply_red_guard_nationalism_upgrade(id),
                2 => self.apply_tank_hunter_nationalism_upgrade(id),
                3 => self.apply_minigunner_nationalism_upgrade(id),
                4 => {
                    if let Some(o) = self.objects.get_mut(&id) {
                        o.applied_upgrades.insert(UPGRADE_NATIONALISM.to_string());
                    }
                    self.evaluate_horde_morale_bonus(id);
                    true
                }
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_NATIONALISM);
                }
                n = n.saturating_add(1);
            }
        }
        // Player-level unlock residual so late-built units can inherit.
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences.insert(UPGRADE_NATIONALISM.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_Fanaticism: tag only. evaluateMoraleBonus sets FANATICISM
    /// only inside `if (nationalism)` where nationalism is Upgrade_Nationalism.
    pub(in super::super) fn apply_fanaticism_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_battlemaster::{
            UPGRADE_FANATICISM, is_china_horde_update_unit, leftover_horde_fanaticism_bonus,
        };

        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_china_horde_update_unit(&o.template_name) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for id in ids {
            if let Some(o) = self.objects.get_mut(&id) {
                o.apply_upgrade_tag(upgrade_name);
                o.apply_upgrade_tag(UPGRADE_FANATICISM);
                o.weapon_bonus_fanaticism =
                    leftover_horde_fanaticism_bonus(o.weapon_bonus_nationalism, true);

                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences.insert(UPGRADE_FANATICISM.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaChainGuns residual — gattling/minigun damage ×1.25.
    pub(in super::super) fn apply_chain_guns_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_base_defense::is_gattling_cannon_structure;
        use crate::game_logic::host_gattling_tank::{
            UPGRADE_CHINA_CHAIN_GUNS, is_gattling_tank_template,
        };
        use crate::game_logic::host_minigunner::is_minigunner_template;

        let ids: Vec<(ObjectId, u8)> = self
            .objects
            .iter()
            .filter(|(_, o)| upgrade_targets_object(o, team) && o.is_alive())
            .filter_map(|(id, o)| {
                if is_minigunner_template(&o.template_name) {
                    Some((*id, 0u8))
                } else if is_gattling_tank_template(&o.template_name)
                    || is_gattling_cannon_structure(&o.template_name)
                {
                    Some((*id, 1))
                } else {
                    None
                }
            })
            .collect();
        let mut n = 0u32;
        for (id, kind) in ids {
            let ok = match kind {
                0 => self.apply_minigunner_chain_guns_upgrade(id),
                1 => self.apply_gattling_chain_guns_upgrade(id),
                _ => false,
            };
            if ok {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.apply_upgrade_tag(upgrade_name);
                    o.apply_upgrade_tag(UPGRADE_CHINA_CHAIN_GUNS);
                }
                n = n.saturating_add(1);
            }
        }
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_CHAIN_GUNS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_ChinaSubliminalMessaging residual.
    ///
    /// Tags propaganda towers and unlocks upgraded heal/buff rate path
    /// (player unlocked_sciences + tower upgrade tags).
    pub(in super::super) fn apply_subliminal_messaging_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_propaganda::{
            UPGRADE_CHINA_SUBLIMINAL_MESSAGING, is_propaganda_tower,
        };
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            let is_tower =
                is_propaganda_tower(&obj.template_name) || obj.has_overlord_propaganda_residual();
            if !is_tower {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_CHINA_SUBLIMINAL_MESSAGING)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_SUBLIMINAL_MESSAGING);
            affected = affected.saturating_add(1);
        }
        // Also unlock at player level so towers without tags still see upgraded rate.
        for p in self.players.values_mut() {
            if upgrade_targets_player(p, team) {
                p.unlocked_sciences
                    .insert(UPGRADE_CHINA_SUBLIMINAL_MESSAGING.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        self.subliminal_messaging_upgrades = self.subliminal_messaging_upgrades.saturating_add(1);
        self.subliminal_towers_affected = self.subliminal_towers_affected.saturating_add(affected);
        affected
    }

    /// C++ PowerPlantUpgrade::upgradeImplementation for one America plant.
    /// Adds EnergyBonus and starts extendRods when Advanced Control Rods is
    /// already researched (Object::updateUpgradeModules on a new Cold Fusion).
    pub(in super::super) fn apply_advanced_control_rods_to_object(
        &mut self,
        plant_id: ObjectId,
        upgrade_name: &str,
    ) -> bool {
        use crate::game_logic::host_structure_economy_residual::{
            AMERICA_POWER_ENERGY_BONUS, UPGRADE_AMERICA_ADVANCED_CONTROL_RODS,
            is_power_plant_template,
        };

        let bonus = AMERICA_POWER_ENERGY_BONUS;
        let Some(obj) = self.objects.get_mut(&plant_id) else {
            return false;
        };
        if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        let is_plant = is_power_plant_template(&obj.template_name)
            || obj.is_kind_of(KindOf::PowerPlant)
            || obj.is_kind_of(KindOf::FSPower);
        if !is_plant {
            return false;
        }
        let n = obj.template_name.to_ascii_lowercase();
        let america = n.contains("america") || n.contains("usa") || n.contains("coldfusion");
        if !america {
            return false;
        }
        if obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_CONTROL_RODS)
            || obj.has_upgrade_tag(upgrade_name)
        {
            return false;
        }
        obj.apply_upgrade_tag(upgrade_name);
        obj.apply_upgrade_tag(UPGRADE_AMERICA_ADVANCED_CONTROL_RODS);
        obj.power_provided = obj.power_provided.saturating_add(bonus);
        obj.record_host_entity_power();
        let _ = self.begin_power_plant_rods_extend(plant_id);
        true
    }

    /// C++ PowerPlantUpgrade Advanced Control Rods residual.
    ///
    /// Tags America power plants and adds EnergyBonus to power_provided;
    /// sets POWER_PLANT_UPGRADED model condition (extendRods residual).
    pub(in super::super) fn apply_advanced_control_rods_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_structure_economy_residual::{
            UPGRADE_AMERICA_ADVANCED_CONTROL_RODS, is_power_plant_template,
        };

        let mut plant_ids: Vec<ObjectId> = Vec::new();
        for (id, obj) in self.objects.iter() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let is_plant = is_power_plant_template(&obj.template_name)
                || obj.is_kind_of(KindOf::PowerPlant)
                || obj.is_kind_of(KindOf::FSPower);
            if !is_plant {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            let america = n.contains("america") || n.contains("usa") || n.contains("coldfusion");
            if !america {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_CONTROL_RODS)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            plant_ids.push(*id);
        }
        let mut affected = 0u32;
        for id in plant_ids {
            if self.apply_advanced_control_rods_to_object(id, upgrade_name) {
                affected = affected.saturating_add(1);
            }
        }
        self.control_rods_upgrades = self.control_rods_upgrades.saturating_add(1);
        self.control_rods_plants_affected =
            self.control_rods_plants_affected.saturating_add(affected);
        affected
    }

    /// Apply WorkerShoes residual: speed 30 + upgrade tag on GLA workers.
    pub(in super::super) fn apply_worker_shoes_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_gla_worker::{
            UPGRADE_GLA_WORKER_SHOES, WORKER_SHOES_AUDIO, is_gla_worker_template,
            worker_residual_speed,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_gla_worker_template(&obj.template_name) && !obj.is_worker() {
                continue;
            }
            // Prefer GLA worker templates; also accept KINDOF_WORKER residual
            // whose template name matches worker residual (not USA/China dozer).
            if !is_gla_worker_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES) || obj.has_upgrade_tag(upgrade_name) {
                // Already applied — refresh speed residual only.
                obj.movement.max_speed = worker_residual_speed(true);
                continue;
            }
            obj.movement.max_speed = worker_residual_speed(true);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.gla_worker.record_shoes_applied(affected);
            self.queue_audio_event(AudioEventRequest::new(WORKER_SHOES_AUDIO).with_priority(140));
        }
        affected
    }

    /// Apply Nuclear Tanks residual: death-weapon tag + nuclear locomotor speed.
    pub(in super::super) fn apply_nuclear_tanks_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_nuclear_tanks::{
            NUCLEAR_TANKS_UPGRADE_AUDIO, UPGRADE_CHINA_NUCLEAR_TANKS, has_nuclear_tanks_upgrade,
            is_nuclear_tanks_eligible, nuclear_tanks_residual_speed,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_nuclear_tanks_eligible(&obj.template_name) {
                continue;
            }
            if has_nuclear_tanks_upgrade(&obj.applied_upgrades) || obj.has_upgrade_tag(upgrade_name)
            {
                // Refresh speed residual only.
                obj.movement.max_speed = nuclear_tanks_residual_speed(&obj.template_name);
                continue;
            }
            obj.movement.max_speed = nuclear_tanks_residual_speed(&obj.template_name);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_NUCLEAR_TANKS);
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.nuclear_tanks.record_upgrade_applied(affected);
            self.queue_audio_event(
                AudioEventRequest::new(NUCLEAR_TANKS_UPGRADE_AUDIO).with_priority(140),
            );
        }
        affected
    }

    /// Apply BoobyTrap residual unlock tag on GLA Rebel infantry.
    pub(in super::super) fn apply_booby_trap_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_booby_trap::{
            UPGRADE_GLA_REBEL_BOOBY_TRAP, is_booby_trap_planter_template,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_booby_trap_planter_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_REBEL_BOOBY_TRAP);
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.booby_trap.record_upgrade_applied(affected);
        }
        affected
    }

    /// Equip FlashBang secondary on team rangers + apply upgrade tag.
    pub(in super::super) fn apply_flashbang_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::is_flashbang_unit_template;
        use crate::game_logic::weapon_bootstrap::{
            RANGER_SECONDARY_WEAPON, ensure_host_weapon_store,
        };

        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(RANGER_SECONDARY_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_flashbang_unit_template(&obj.template_name) {
                continue;
            }
            if obj.secondary_weapon.is_none() {
                if let Some(ref w) = secondary {
                    let _ = obj.replace_weapon_set_slot(1, Some(w.clone()));
                }
            }
            obj.apply_upgrade_tag(upgrade_name);
            // Canonical retail name tag for ability checks.
            obj.apply_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG);
            let _ = obj.set_weapon_set_flag(0, true);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip TOW secondary on team Humvees + apply upgrade tag.
    pub(in super::super) fn apply_tow_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::is_tow_unit_template;
        use crate::game_logic::weapon_bootstrap::{
            HUMVEE_SECONDARY_WEAPON, ensure_host_weapon_store,
        };

        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(HUMVEE_SECONDARY_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_tow_unit_template(&obj.template_name) {
                continue;
            }
            if obj.secondary_weapon.is_none() {
                if let Some(mut w) = secondary.clone() {
                    // Residual: ground TOW + air tertiary capability (PreferredAgainst AIRCRAFT).
                    // Damage boost vs air applied in combat path (HUMVEE_AIR_TOW_DAMAGE).
                    w.can_target_air = true;
                    w.range = w
                        .range
                        .max(crate::game_logic::host_humvee::HUMVEE_AIR_TOW_RANGE);
                    let _ = obj.replace_weapon_set_slot(1, Some(w));
                }
            } else if let Some(w) = obj.secondary_weapon.as_mut() {
                w.can_target_air = true;
                w.range = w
                    .range
                    .max(crate::game_logic::host_humvee::HUMVEE_AIR_TOW_RANGE);
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW);
            let _ = obj.set_weapon_set_flag(0, true);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Apply Composite Armor MaxHealthUpgrade residual (+100 HP) to Crusader / Paladin.
    pub(in super::super) fn apply_composite_armor_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_usa_tanks::{
            UPGRADE_AMERICA_COMPOSITE_ARMOR, apply_composite_armor_health,
            is_composite_armor_unit_template,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_composite_armor_unit_template(&obj.template_name) {
                continue;
            }
            // Idempotent: skip if already tagged.
            if obj.has_upgrade_tag(UPGRADE_AMERICA_COMPOSITE_ARMOR)
                || obj.has_upgrade_tag(upgrade_name)
            {
                continue;
            }
            let mut max_h = obj.max_health;
            let mut cur = obj.health.current;
            let mut maximum = obj.health.maximum;
            apply_composite_armor_health(&mut max_h, &mut cur, &mut maximum);
            obj.set_body_max_health(max_h);
            obj.record_host_max_health();
            Self::write_object_health_authority_aware(obj, cur);
            obj.health.maximum = maximum;
            crate::game_logic::host_heal_log::record(obj.id, obj.health.current);
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_COMPOSITE_ARMOR);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip Neutron Shell secondary on team Nuke Cannons + apply upgrade tag.
    pub(in super::super) fn apply_neutron_shells_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_neutron_shell::UPGRADE_CHINA_NEUTRON_SHELLS;
        use crate::game_logic::host_upgrades::is_neutron_shell_unit_template;
        use crate::game_logic::weapon_bootstrap::{
            NUKE_CANNON_NEUTRON_WEAPON, ensure_host_weapon_store,
        };

        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(NUKE_CANNON_NEUTRON_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_neutron_shell_unit_template(&obj.template_name) {
                continue;
            }
            if let Some(ref w) = secondary {
                // Always re-bind neutron secondary residual so stats stay correct.
                let _ = obj.replace_weapon_set_slot(1, Some(w.clone()));
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Equip Comanche Rocket Pods tertiary + apply upgrade tag.
    ///
    /// Retail: WeaponSetUpgrade TriggeredBy = Upgrade_ComancheRocketPods unlocks
    /// TERTIARY ComancheRocketPodWeapon.  SECONDARY remains the independent
    /// ComancheAntiTankMissileWeapon slot.
    pub(in super::super) fn apply_comanche_rocket_pods_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_comanche_rocket_pods::{
            UPGRADE_COMANCHE_ROCKET_PODS, comanche_antitank_weapon, comanche_rocket_pod_weapon,
            is_comanche_template,
        };

        let tertiary = comanche_rocket_pod_weapon();
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_comanche_template(&obj.template_name) {
                continue;
            }
            // Preserve the retail anti-tank SECONDARY.  Rebind it only when
            // an older host object lacked that slot, never by guessing a
            // substitute from the pod weapon.
            if obj.secondary_weapon.is_none() {
                let _ = obj.replace_weapon_set_slot(1, Some(comanche_antitank_weapon()));
            }
            let _ = obj.replace_weapon_set_slot(2, Some(tertiary.clone()));
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS);
            // C++ WeaponSetUpgrade → setWeaponSetFlag(WEAPONSET_PLAYER_UPGRADE).
            let _ = obj.set_weapon_set_flag(0, true);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// C++ `WeaponSetUpgrade` + `updateWeaponSet` for Sentry PLAYER_UPGRADE.
    /// Bind PRIMARY `SentryDroneGun`, stamp the retail tag, set flag 0.
    pub(in super::super) fn equip_sentry_drone_gun(obj: &mut Object) {
        use crate::game_logic::host_sentry_drone::{
            SENTRY_DRONE_GUN_WEAPON, UPGRADE_AMERICA_SENTRY_DRONE_GUN,
        };
        use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

        ensure_host_weapon_store();
        if let Some(w) = ThingTemplate::weapon_from_store(SENTRY_DRONE_GUN_WEAPON) {
            let _ = obj.replace_weapon_set_slot(0, Some(w));
        }
        obj.apply_upgrade_tag(UPGRADE_AMERICA_SENTRY_DRONE_GUN);
        let _ = obj.set_weapon_set_flag(0, true);
    }

    /// Equip Sentry Drone Gun primary + apply upgrade tag.
    ///
    /// Retail: WeaponSetUpgrade TriggeredBy = Upgrade_AmericaSentryDroneGun unlocks
    /// PRIMARY SentryDroneGun. Host residual binds primary weapon for auto-fire.
    pub(in super::super) fn apply_sentry_drone_gun_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_sentry_drone::is_sentry_drone_template;

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_sentry_drone_template(&obj.template_name) {
                continue;
            }
            Self::equip_sentry_drone_gun(obj);
            obj.apply_upgrade_tag(upgrade_name);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Tag team Stealth Fighters with Bunker Busters residual upgrade.
    ///
    /// C++ BunkerBusterBehavior checks player upgrade on missile detonation;
    /// host residual tags carriers so combat can apply garrison kill + bunker mult.
    pub(in super::super) fn apply_bunker_busters_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_bunker_buster::{
            UPGRADE_AMERICA_BUNKER_BUSTERS, is_bunker_buster_carrier,
        };
        use crate::game_logic::weapon_bootstrap::{
            STEALTH_JET_MISSILE_WEAPON, ensure_host_weapon_store,
        };

        ensure_host_weapon_store();
        let primary = ThingTemplate::weapon_from_store(STEALTH_JET_MISSILE_WEAPON);
        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_bunker_buster_carrier(&obj.template_name) {
                continue;
            }
            if let Some(ref w) = primary {
                // Ensure residual anti-structure missile stats when store available.
                if obj.weapon.is_none() {
                    let _ = obj.replace_weapon_set_slot(0, Some(w.clone()));
                }
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Tag capture-capable infantry so capture unlock is unit-observable.
    pub(in super::super) fn apply_capture_unlock_tags_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            UPGRADE_INFANTRY_CAPTURE, is_capture_capable_infantry_template,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Infantry) {
                continue;
            }
            if !is_capture_capable_infantry_template(&obj.template_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_INFANTRY_CAPTURE);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Grant GLA Camouflage residual stealth to Rebel infantry.
    ///
    /// C++ StealthUpgrade.cpp:31 sets OBJECT_STATUS_CAN_STEALTH only.
    /// StealthUpdate.cpp:739 re-arms `m_stealthAllowedFrame = now + StealthDelay`
    /// rather than cloaking on the upgrade frame.
    pub(in super::super) fn apply_camouflage_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            CAMOUFLAGE_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMOUFLAGE,
            camouflage_stealth_allowed_frame, is_camouflage_unit_template,
        };

        let mut affected = 0u32;
        let now = self.frame;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !obj.is_kind_of(KindOf::Infantry) {
                continue;
            }
            if !is_camouflage_unit_template(&obj.template_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE);
            obj.set_status_stealthed(false);
            obj.set_status_detected(false);
            obj.detection_expires_frame = 0;
            obj.innate_stealth = true;
            obj.record_host_stealth_flags();
            obj.stealth_breaks_on_attack = true;
            obj.record_host_stealth_flags();
            obj.stealth_breaks_on_move = false;
            obj.record_host_stealth_flags();
            obj.stealth_delay_frames = CAMOUFLAGE_STEALTH_DELAY_FRAMES;
            obj.stealth_allowed_frame = camouflage_stealth_allowed_frame(now);
            obj.stealth_delay_pending = false;
            obj.record_host_stealth_delay();
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Grant GLA CamoNetting residual stealth to eligible GLA structures.
    ///
    /// C++ StealthUpgrade.cpp:31 CAN_STEALTH only; StealthUpdate.cpp:739 re-arms
    /// StealthDelay (2500ms / 75f) before STEALTHED.
    pub(in super::super) fn apply_camo_netting_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            CAMO_NETTING_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMO_NETTING,
            camo_netting_stealth_allowed_frame, is_camo_netting_structure_template,
        };

        let mut affected = 0u32;
        let now = self.frame;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_camo_netting_structure_template(&obj.template_name) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_CAMO_NETTING);
            obj.set_status_stealthed(false);
            obj.set_status_detected(false);
            obj.detection_expires_frame = 0;
            obj.innate_stealth = true;
            obj.record_host_stealth_flags();
            obj.stealth_breaks_on_attack = true;
            obj.record_host_stealth_flags();
            obj.stealth_breaks_on_damage = true;
            obj.stealth_breaks_on_move = false;
            obj.record_host_stealth_flags();
            obj.stealth_delay_frames = CAMO_NETTING_STEALTH_DELAY_FRAMES;
            obj.stealth_allowed_frame = camo_netting_stealth_allowed_frame(now);
            obj.stealth_delay_pending = false;
            obj.record_host_stealth_delay();
            // Sub-object net mesh residual: upgrade shows CamoNet presentation.
            obj.camo_net_sub_object_shown = true;
            obj.camo_net_sub_object_observer_visible = true;
            affected = affected.saturating_add(1);
        }
        if affected > 0 {
            self.camo_netting_opacity_cloak_count = self
                .camo_netting_opacity_cloak_count
                .saturating_add(affected);
            self.camo_netting_sub_object_show_count = self
                .camo_netting_sub_object_show_count
                .saturating_add(affected);
        }
        affected
    }

    /// Tag toxin combat units for Anthrax Gamma residual (Chem general).
    ///
    /// Fail-closed: not full WeaponSet PLAYER_UPGRADE module / particle gamma FX.
    pub(in super::super) fn apply_anthrax_gamma_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_mines::{DemoTrapProfile, HostMineKind, is_demo_trap_template};
        use crate::game_logic::host_toxin_tractor::{
            UPGRADE_GLA_ANTHRAX_GAMMA, UPGRADE_GLA_ANTHRAX_GAMMA_ALT, is_toxin_tractor_template,
        };
        use crate::game_logic::host_upgrades::{
            UPGRADE_CHEM_ANTHRAX_GAMMA, is_anthrax_gamma_unit_template,
        };

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            let demo_trap = is_demo_trap_template(&obj.template_name)
                && obj.mine_data.as_ref().is_some_and(|md| {
                    matches!(md.kind, HostMineKind::DemoTrap)
                        && md.demo_trap_profile.spawns_poison()
                });
            if !is_anthrax_gamma_unit_template(&obj.template_name)
                && !is_toxin_tractor_template(&obj.template_name)
                && !demo_trap
            {
                continue;
            }
            if demo_trap {
                if let Some(md) = obj.mine_data.as_mut() {
                    md.demo_trap_profile = DemoTrapProfile::ChemGamma;
                }
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_CHEM_ANTHRAX_GAMMA);
            obj.apply_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA);
            obj.apply_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA_ALT);
            affected = affected.saturating_add(1);
        }
        affected
    }

    /// Apply Demo SuicideBomb residual: tag eligible Demo units/structures +
    /// CommandSetUpgrade residual override for TertiarySuicide.
    pub(in super::super) fn apply_demo_suicide_bomb_unlock_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_demo_suicide_bomb::{
            UPGRADE_DEMO_SUICIDE_BOMB, demo_command_set_upgrade_for_template,
            is_demo_suicide_bomb_eligible_template,
        };

        let mut affected = 0u32;
        let mut command_sets = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if !is_demo_suicide_bomb_eligible_template(&obj.template_name) {
                continue;
            }
            if obj.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB) || obj.has_upgrade_tag(upgrade_name) {
                // Still ensure CommandSetUpgrade residual is applied if missing.
                if obj.command_set_override.is_none() {
                    if let Some(cs) = demo_command_set_upgrade_for_template(&obj.template_name) {
                        obj.set_command_set_override(Some(cs));
                        command_sets = command_sets.saturating_add(1);
                    }
                }
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB);
            if let Some(cs) = demo_command_set_upgrade_for_template(&obj.template_name) {
                obj.set_command_set_override(Some(cs));
                command_sets = command_sets.saturating_add(1);
            }
            affected = affected.saturating_add(1);
        }
        self.demo_suicide_bomb.record_upgrade_complete(affected);
        if command_sets > 0 {
            self.demo_suicide_bomb
                .record_command_set_upgrade(command_sets);
        }
        affected
    }

    /// Tag supply centers for Supply Lines residual observability.
    pub(in super::super) fn apply_supply_lines_tags_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::is_supply_center_template;

        let mut affected = 0u32;
        for obj in self.objects.values_mut() {
            if !upgrade_targets_object(obj, team) || !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(KindOf::SupplyCenter) || is_supply_center_template(&obj.template_name)
            {
                obj.apply_upgrade_tag(upgrade_name);
                affected = affected.saturating_add(1);
            }
        }
        affected
    }
}

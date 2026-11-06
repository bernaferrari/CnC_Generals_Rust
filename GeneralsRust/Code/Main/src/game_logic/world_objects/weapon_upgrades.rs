//! Host objects `impl GameLogic` — `weapon_upgrades`.
//! apply_*_to_team weapon upgrades. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;
/// Explicit recipients of one synchronous upgrade operation. Match state
/// remains owned by the driving GameLogic; no call-stack selection is published.
#[derive(Clone, Copy, Debug)]
pub(in crate::game_logic) enum UpgradeRecipients {
    Player {
        team: Team,
        player_id: u32,
    },
    Team(Team),
    Object {
        team: Team,
        player_id: u32,
        object_id: ObjectId,
    },
    PlayerLedger {
        team: Team,
        player_id: u32,
    },
}
impl UpgradeRecipients {
    pub(super) fn team(self) -> Team {
        match self {
            Self::Player { team, .. }
            | Self::Team(team)
            | Self::Object { team, .. }
            | Self::PlayerLedger { team, .. } => team,
        }
    }
    pub(super) fn matches_object(self, object: &Object) -> bool {
        object.team == self.team()
            && match self {
                Self::Player { player_id, .. } => object.owner_player_id == Some(player_id),
                Self::Team(_) => true,
                Self::Object {
                    player_id,
                    object_id,
                    ..
                } => object.id == object_id && object.owner_player_id == Some(player_id),
                Self::PlayerLedger { .. } => false,
            }
    }
    fn matches_player(self, player: &Player) -> bool {
        player.team == self.team()
            && match self {
                Self::Player { player_id, .. } => player.id == player_id,
                Self::Team(_) => true,
                Self::Object { .. } => false,
                Self::PlayerLedger { player_id, .. } => player.id == player_id,
            }
    }
}
// Bare faction calls exist only in component fixtures. Production completion
// must construct the explicit Player variant at its admission boundary.
#[cfg(test)]
impl From<Team> for UpgradeRecipients {
    fn from(team: Team) -> Self {
        Self::Team(team)
    }
}

impl GameLogic {
    pub(in super::super) fn apply_cash_bounty_upgrade_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science;

        let pct = cash_bounty_percent_for_science(upgrade_name).unwrap_or(0.05);
        let mut n = 0u32;
        let mut player_ids = Vec::new();
        for (id, p) in self.players.iter_mut() {
            if !recipients.matches_player(p) {
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
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        self.apply_slave_drone_upgrade_to_team_with_effects(
            recipients,
            upgrade_name,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn apply_slave_drone_upgrade_to_team_with_effects(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        self.apply_slave_drone_upgrade_to_team_with_interpreter_and_effects(
            recipients,
            upgrade_name,
            None,
            effects,
        )
    }
    pub(in super::super) fn apply_slave_drone_upgrade_to_team_with_interpreter_and_effects(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        use crate::game_logic::host_slave_drones::{
            SlaveDroneKind, is_slave_drone_master_template, slave_drone_conflicts_with_owned,
        };

        // C++ ObjectCreationUpgrade is keyed by the granted upgrade itself;
        // no drone kind for an unknown upgrade (no Scout fallback).
        let Some(kind) = SlaveDroneKind::from_upgrade_name(upgrade_name) else {
            return 0;
        };

        let masters: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| {
                recipients.matches_object(o)
                    && o.is_alive()
                    && !o.status.under_construction
                    && is_slave_drone_master_template(&o.template_name)
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
            // C++ Object::giveUpgrade sets the bit before ObjectCreationUpgrade
            // creates the drone (Object.cpp giveUpgrade -> updateUpgradeModules).
            if let Some(m) = self.objects.get_mut(&mid) {
                m.apply_upgrade_tag(upgrade_name);
            }
            if self
                .residual_attach_slave_drone_with_interpreter_and_effects(
                    mid,
                    kind,
                    interpreter,
                    effects,
                )
                .is_some()
            {
                n = n.saturating_add(1);
            }
        }
        // Player unlock residual for production UI / late builds.
        let _ = self.apply_player_unlock_upgrade(recipients, upgrade_name, upgrade_name);
        n
    }

    /// Generic player-level upgrade unlock residual (mines / radar scan / flags).
    pub(in super::super) fn apply_player_unlock_upgrade(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
        canonical: &str,
    ) -> u32 {
        let mut n = 0u32;
        for p in self.players.values_mut() {
            if !recipients.matches_player(p) {
                continue;
            }
            p.unlocked_sciences.insert(canonical.to_string());
            p.unlocked_sciences.insert(upgrade_name.to_string());
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ faction RadarUpgrade residual — unlock + tag radar providers.
    pub(in super::super) fn apply_radar_research_to_team(
        &mut self,
        team: Team,
        upgrade_name: &str,
    ) -> u32 {
        self.apply_radar_research_to_team_with_effects(team, upgrade_name, &mut Default::default())
    }

    pub(in super::super) fn apply_radar_research_to_team_with_effects(
        &mut self,
        team: Team,
        upgrade_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let Some(owner) = self.unique_player_id_for_team(team) else {
            return 0;
        };
        self.apply_radar_research_to_player_with_effects(owner, upgrade_name, effects)
    }

    pub(in super::super) fn apply_radar_research_to_player(
        &mut self,
        owner: u32,
        upgrade_name: &str,
    ) -> u32 {
        self.apply_radar_research_to_player_with_effects(
            owner,
            upgrade_name,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn apply_radar_research_to_player_with_effects(
        &mut self,
        owner: u32,
        upgrade_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
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

        let Some(player) = self.players.get_mut(&owner) else {
            return 0;
        };
        player.unlocked_sciences.insert(canonical.to_string());
        player.unlocked_sciences.insert(upgrade_name.to_string());
        let mut n = 1u32;
        let ids: Vec<_> = self
            .objects
            .registered_values()
            .filter(|object| {
                object.is_alive() && self.player_owner_for_host_object(object) == Some(owner)
            })
            .map(|object| object.id)
            .collect();
        for id in &ids {
            let obj = self.objects.get_mut(id).unwrap();
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
        for id in ids {
            self.admit_host_radar_provider_with_effects(id, effects);
        }
        n
    }

    /// C++ Upgrade_GLAAnthraxBeta residual — toxin tractor + SCUD + scud storm tier.
    pub(in super::super) fn apply_anthrax_beta_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_scud_launcher::{
            UPGRADE_GLA_ANTHRAX_BETA, authors_scud_gun,
        };
        use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;

        let mut n = 0u32;
        for obj in self.objects.registered_values_mut() {
            if !recipients.matches_object(obj) || !obj.is_alive() {
                continue;
            }
            let is_tt = is_toxin_tractor_template(&obj.template_name);
            let is_scud = authors_scud_gun(obj.authored_primary_weapon_names());
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
            if recipients.matches_player(p) {
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
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_scud_launcher::authors_scud_gun;
        use crate::game_logic::host_upgrades::UPGRADE_GLA_TOXIN_SHELLS;

        let mut n = 0u32;
        for obj in self.objects.registered_values_mut() {
            if !recipients.matches_object(obj) || !obj.is_alive() {
                continue;
            }
            if !authors_scud_gun(obj.authored_primary_weapon_names()) {
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
            if recipients.matches_player(p) {
                p.unlocked_sciences
                    .insert(UPGRADE_GLA_TOXIN_SHELLS.to_string());
                p.unlocked_sciences.insert(upgrade_name.to_string());
            }
        }
        n
    }

    /// C++ Upgrade_AmericaLaserMissiles residual — Raptor jet damage.
    pub(in super::super) fn apply_laser_missiles_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_raptor::{
            UPGRADE_AMERICA_LASER_MISSILES, authors_raptor_missile,
        };
        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| {
                recipients.matches_object(o)
                    && o.is_alive()
                    && authors_raptor_missile(o.authored_primary_weapon_names(), None)
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

    /// Grant GLA Camouflage residual stealth to Rebel infantry.
    ///
    /// C++ StealthUpgrade.cpp:31 sets OBJECT_STATUS_CAN_STEALTH only.
    /// StealthUpdate.cpp:739 re-arms `m_stealthAllowedFrame = now + StealthDelay`
    /// rather than cloaking on the upgrade frame.
    pub(in super::super) fn apply_camouflage_unlock_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            CAMOUFLAGE_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMOUFLAGE,
            camouflage_stealth_allowed_frame,
        };

        let mut affected = 0u32;
        let now = self.frame;
        for obj in self.objects.registered_values_mut() {
            if !recipients.matches_object(obj) || !obj.is_alive() {
                continue;
            }
            // StealthUpgrade.cpp: only objects authoring a StealthUpgrade
            // triggered by this upgrade gain CAN_STEALTH.
            if !super::authored_upgrade_modules::authors_triggered_stealth_upgrade(
                &obj.thing().template,
                |name| name.eq_ignore_ascii_case(upgrade_name),
            ) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_CAMOUFLAGE);
            obj.set_status_stealthed(false);
            obj.set_status_detected(false);
            obj.detection_expires_frame = 0;
            obj.innate_stealth = true;

            obj.stealth_breaks_on_attack = true;

            obj.stealth_breaks_on_move = false;

            obj.stealth_delay_frames = CAMOUFLAGE_STEALTH_DELAY_FRAMES;
            obj.stealth_allowed_frame = camouflage_stealth_allowed_frame(now);
            obj.stealth_delay_pending = false;
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
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_upgrades::{
            CAMO_NETTING_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMO_NETTING,
            camo_netting_stealth_allowed_frame,
        };

        let mut affected = 0u32;
        let now = self.frame;
        for obj in self.objects.registered_values_mut() {
            if !recipients.matches_object(obj) || !obj.is_alive() {
                continue;
            }
            if !super::authored_upgrade_modules::authors_triggered_stealth_upgrade(
                &obj.thing().template,
                |name| name.eq_ignore_ascii_case(upgrade_name),
            ) {
                continue;
            }
            obj.apply_upgrade_tag(upgrade_name);
            obj.apply_upgrade_tag(UPGRADE_GLA_CAMO_NETTING);
            obj.set_status_stealthed(false);
            obj.set_status_detected(false);
            obj.detection_expires_frame = 0;
            obj.innate_stealth = true;

            obj.stealth_breaks_on_attack = true;

            obj.stealth_breaks_on_damage = true;
            obj.stealth_breaks_on_move = false;

            obj.stealth_delay_frames = CAMO_NETTING_STEALTH_DELAY_FRAMES;
            obj.stealth_allowed_frame = camo_netting_stealth_allowed_frame(now);
            obj.stealth_delay_pending = false;
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

    /// Apply Demo SuicideBomb residual: tag eligible Demo units/structures +
    /// CommandSetUpgrade residual override for TertiarySuicide.
    pub(in super::super) fn apply_demo_suicide_bomb_unlock_to_team(
        &mut self,
        recipients: UpgradeRecipients,
        upgrade_name: &str,
    ) -> u32 {
        use crate::game_logic::host_demo_suicide_bomb::{
            UPGRADE_DEMO_SUICIDE_BOMB, demo_command_set_upgrade_for_template,
            is_demo_suicide_bomb_eligible_template,
        };

        let mut affected = 0u32;
        let mut command_sets = 0u32;
        for obj in self.objects.registered_values_mut() {
            if !recipients.matches_object(obj) || !obj.is_alive() {
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

}

#[cfg(test)]
mod recipient_owner_tests {
    use super::*;

    fn same_faction_players(world: &mut GameLogic, team: Team) {
        world.add_player(Player::new(0, team, "First", true));
        world.add_player(Player::new(7, team, "Second", false));
    }

    #[test]
    fn actual_completion_changes_only_exact_player_infantry_not_same_faction_or_unowned() {
        let mut owner = GameLogic::new();
        let mut peer = GameLogic::new();
        let mut ids = Vec::new();
        for world in [&mut owner, &mut peer] {
            same_faction_players(world, Team::USA);
            let mut template = ThingTemplate::new("RecipientInfantry");
            template.add_kind_of(KindOf::Infantry).set_health(100.0);
            world.templates.insert("RecipientInfantry".into(), template);
            let first = world
                .create_object_for_player("RecipientInfantry", 0, glam::Vec3::ZERO)
                .unwrap();
            let second = world
                .create_object_for_player("RecipientInfantry", 7, glam::Vec3::X)
                .unwrap();
            let unowned = world
                .create_object("RecipientInfantry", Team::USA, glam::Vec3::Z)
                .unwrap();
            assert_eq!(world.objects[&unowned].owner_player_id, None);
            ids.push([first, second, unowned]);
        }
        assert_eq!(ids[0], ids[1]);
        let upgrade = crate::game_logic::host_upgrades::UPGRADE_AMERICA_CHEMICAL_SUITS;
        owner.apply_host_upgrade_complete(Team::USA, 0, upgrade);
        peer.apply_host_upgrade_complete(Team::USA, 7, upgrade);
        for (world, row, selected) in [(&owner, ids[0], 0), (&peer, ids[1], 1)] {
            for (index, id) in row.into_iter().enumerate() {
                assert_eq!(
                    world.objects[&id].has_upgrade_tag(upgrade),
                    index == selected
                );
            }
            for id in [0, 7] {
                assert_eq!(
                    world.get_player(id).unwrap().has_unlocked_upgrade(upgrade),
                    id == if selected == 0 { 0 } else { 7 }
                );
            }
        }
    }

}

#[cfg(test)]
mod canonical_upgrade_visitation_tests {
    use super::*;

    #[test]
    fn receiving_upgrade_visits_actual_default_team_members_in_cpp_prepend_order() {
        let mut world = GameLogic::new();
        let mut peer = GameLogic::new();
        for world in [&mut world, &mut peer] {
            world.add_player(Player::new(0, Team::GLA, "First", true));
            world.add_player(Player::new(7, Team::GLA, "SameFaction", false));
            let mut template = ThingTemplate::new("GLAInfantryWorker");
            template.add_kind_of(KindOf::Infantry).set_health(100.0);
            world.templates.insert(template.name.clone(), template);
        }
        let first = world
            .create_object_for_player("GLAInfantryWorker", 0, glam::Vec3::ZERO)
            .unwrap();
        let second = world
            .create_object_for_player("GLAInfantryWorker", 0, glam::Vec3::X)
            .unwrap();
        let other = world
            .create_object_for_player("GLAInfantryWorker", 7, glam::Vec3::Y)
            .unwrap();
        let peer_first = peer
            .create_object_for_player("GLAInfantryWorker", 0, glam::Vec3::ZERO)
            .unwrap();
        assert_eq!(first, peer_first);
        assert_eq!(world.ordered_upgrade_owner_object_ids(0), [second, first]);
        let name = "Upgrade_GLAWorkerShoes";
        world.add_completed_player_upgrade(0, name);
        assert!(world.host_object(first).unwrap().has_upgrade_tag(name));
        assert!(world.host_object(second).unwrap().has_upgrade_tag(name));
        assert!(!world.host_object(other).unwrap().has_upgrade_tag(name));
        assert!(!peer.host_object(peer_first).unwrap().has_upgrade_tag(name));
        assert!(world.get_player(0).unwrap().has_unlocked_upgrade(name));
        assert!(!world.get_player(7).unwrap().has_unlocked_upgrade(name));
        assert!(!peer.get_player(0).unwrap().has_unlocked_upgrade(name));
    }
}

#[cfg(test)]
mod upgrade_lookup_admission_tests {
    use super::*;

    #[test]
    fn player_upgrade_visits_registered_team_members_not_lookup_only_constructor_residents() {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::GLA, "owner", true));
        let mut template = ThingTemplate::new("GLAInfantryWorker");
        template.add_kind_of(KindOf::Infantry).set_health(100.);
        world
            .templates
            .insert(template.name.clone(), template.clone());
        let registered = world
            .create_object_for_player(&template.name, 0, glam::Vec3::ZERO)
            .unwrap();
        // This existing lookup-only seam models the period before Team admission.
        // A controlling Player value alone does not make a Team list member.
        let raw_id = ObjectId(900);
        let mut raw = Object::new(template, raw_id, Team::GLA);
        raw.owner_player_id = Some(0);
        world.try_admit_host_object(raw).unwrap();
        assert!(world.objects[&raw_id].team_instance_id.is_none());
        assert_eq!(world.ordered_upgrade_owner_object_ids(0), [registered]);
        let name = "Upgrade_GLAWorkerShoes";
        world.add_completed_player_upgrade(0, name);
        assert!(world.objects[&registered].has_upgrade_tag(name));
        assert!(!world.objects[&raw_id].has_upgrade_tag(name));
        assert!(world.get_player(0).unwrap().has_unlocked_upgrade(name));
    }
}

#[cfg(test)]
mod authored_mig_gate_tests {
    use super::UpgradeRecipients;
    use crate::game_logic::*;

    /// hq-2qedt.248: Black Napalm / Tactical Nuke MiG direct apply paths (vehicle_shells)
    /// key on authored MiG missile PRIMARY, not template-name sniffing.
    #[test]
    fn mig_direct_upgrade_apply_keys_on_authored_missile_not_template_name() {
        use crate::game_logic::host_mig::{
            NAPALM_MISSILE_WEAPON, NUKE_MIG_MISSILE_WEAPON, UPGRADE_CHINA_BLACK_NAPALM,
            UPGRADE_CHINA_TACTICAL_NUKE_MIG,
        };
        let mut gl = GameLogic::new();
        for (tpl_name, primary) in [
            ("ChinaJetMIG", "HumveeGun"),
            ("Nuke_ChinaJetMIG", "HumveeGun"),
            ("AuthoredAirframeX", NAPALM_MISSILE_WEAPON),
            ("AuthoredAirframeN", NUKE_MIG_MISSILE_WEAPON),
        ] {
            let mut tpl = crate::game_logic::ThingTemplate::new(tpl_name);
            tpl.add_kind_of(KindOf::Aircraft)
                .set_health(300.0)
                .set_primary_weapon_name(primary);
            gl.templates.insert(tpl_name.to_string(), tpl);
        }
        let mk = |gl: &mut GameLogic, n: &str| {
            gl.create_object(n, Team::China, Vec3::new(0.0, 0.0, 0.0))
                .unwrap()
        };
        let named = mk(&mut gl, "ChinaJetMIG");
        let named_n = mk(&mut gl, "Nuke_ChinaJetMIG");
        let auth = mk(&mut gl, "AuthoredAirframeX");
        let auth_n = mk(&mut gl, "AuthoredAirframeN");
        for id in [named, auth, auth_n] {
            let _ = gl.apply_mig_black_napalm_upgrade(id);
        }
        for id in [named_n, auth_n, auth] {
            let _ = gl.apply_mig_tactical_nuke_upgrade(id);
        }
        let has =
            |gl: &GameLogic, id, u: &str| gl.host_object(id).unwrap().applied_upgrades.contains(u);
        assert!(!has(&gl, named, UPGRADE_CHINA_BLACK_NAPALM));
        assert!(has(&gl, auth, UPGRADE_CHINA_BLACK_NAPALM));
        assert!(!has(&gl, auth_n, UPGRADE_CHINA_BLACK_NAPALM));
        assert!(!has(&gl, named_n, UPGRADE_CHINA_TACTICAL_NUKE_MIG));
        assert!(has(&gl, auth_n, UPGRADE_CHINA_TACTICAL_NUKE_MIG));
        assert!(!has(&gl, auth, UPGRADE_CHINA_TACTICAL_NUKE_MIG));
    }

    /// hq-2qedt.249: Laser Missiles (WeaponSetUpgrade) keys on authored Raptor missile
    /// PRIMARY, not template-name sniffing (team + direct apply paths).
    #[test]
    fn raptor_laser_missiles_keys_on_authored_missile_not_template_name() {
        use crate::game_logic::host_raptor::{
            AIRF_RAPTOR_JET_MISSILE_WEAPON, RAPTOR_JET_MISSILE_WEAPON,
            UPGRADE_AMERICA_LASER_MISSILES,
        };
        let mut gl = GameLogic::new();
        for (tpl_name, primary) in [
            ("AmericaJetRaptor", "HumveeGun"),
            ("AuthoredAirframeR", RAPTOR_JET_MISSILE_WEAPON),
            ("AuthoredAirframeK", AIRF_RAPTOR_JET_MISSILE_WEAPON),
        ] {
            let mut tpl = crate::game_logic::ThingTemplate::new(tpl_name);
            tpl.add_kind_of(KindOf::Aircraft)
                .set_health(160.0)
                .set_primary_weapon_name(primary);
            gl.templates.insert(tpl_name.to_string(), tpl);
        }
        let named = gl
            .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let auth = gl
            .create_object("AuthoredAirframeR", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let king = gl
            .create_object("AuthoredAirframeK", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        assert!(!gl.apply_raptor_laser_missiles_upgrade(named));
        assert!(gl.apply_raptor_laser_missiles_upgrade(king));
        let n = gl.apply_laser_missiles_to_team(
            UpgradeRecipients::Team(Team::USA),
            UPGRADE_AMERICA_LASER_MISSILES,
        );
        assert_eq!(n, 2);
        let has = |gl: &GameLogic, id| {
            gl.host_object(id)
                .unwrap()
                .applied_upgrades
                .contains(UPGRADE_AMERICA_LASER_MISSILES)
        };
        assert!(!has(&gl, named));
        assert!(has(&gl, auth));
        assert!(has(&gl, king));
    }

    /// hq-2qedt.250: Anthrax Beta / Toxin Shells key on authored SCUD gun PRIMARY,
    /// not template-name sniffing.
    #[test]
    fn scud_upgrades_key_on_authored_scud_gun_not_template_name() {
        use crate::game_logic::host_scud_launcher::UPGRADE_GLA_ANTHRAX_BETA;
        use crate::game_logic::host_upgrades::UPGRADE_GLA_TOXIN_SHELLS;
        use crate::game_logic::weapon_bootstrap::SCUD_GUN_EXPLOSIVE;
        let mut gl = GameLogic::new();
        for (tpl_name, primary) in [
            ("GLAVehicleScudLauncher", "HumveeGun"),
            ("AuthoredRackS", SCUD_GUN_EXPLOSIVE),
        ] {
            let mut tpl = crate::game_logic::ThingTemplate::new(tpl_name);
            tpl.add_kind_of(KindOf::Vehicle)
                .set_health(200.0)
                .set_primary_weapon_name(primary);
            gl.templates.insert(tpl_name.to_string(), tpl);
        }
        let named = gl
            .create_object("GLAVehicleScudLauncher", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let auth = gl
            .create_object("AuthoredRackS", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        assert_eq!(
            gl.apply_anthrax_beta_to_team(UpgradeRecipients::Team(Team::GLA), UPGRADE_GLA_ANTHRAX_BETA),
            1
        );
        assert_eq!(
            gl.apply_toxin_shells_to_team(UpgradeRecipients::Team(Team::GLA), UPGRADE_GLA_TOXIN_SHELLS),
            1
        );
        let has = |gl: &GameLogic, id, u: &str| gl.host_object(id).unwrap().applied_upgrades.contains(u);
        assert!(!has(&gl, named, UPGRADE_GLA_ANTHRAX_BETA));
        assert!(has(&gl, auth, UPGRADE_GLA_ANTHRAX_BETA));
        assert!(!has(&gl, named, UPGRADE_GLA_TOXIN_SHELLS));
        assert!(has(&gl, auth, UPGRADE_GLA_TOXIN_SHELLS));
    }
}

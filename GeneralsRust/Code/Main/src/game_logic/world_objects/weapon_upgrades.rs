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
    /// Bind PRIMARY `SentryDroneGun`, stamp the retail tag, set PLAYER_UPGRADE.
    pub(in super::super) fn equip_sentry_drone_gun(obj: &mut Object, frame: u32) {
        use crate::game_logic::host_sentry_drone::{
            SENTRY_DRONE_GUN_WEAPON, UPGRADE_AMERICA_SENTRY_DRONE_GUN,
        };
        use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

        ensure_host_weapon_store();
        if let Some(w) = ThingTemplate::weapon_from_store(SENTRY_DRONE_GUN_WEAPON) {
            let _ = obj.replace_weapon_set_slot(0, Some(w));
        }
        obj.apply_upgrade_tag(UPGRADE_AMERICA_SENTRY_DRONE_GUN);
        obj.set_weapon_condition_at_frame(3, true, frame);
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
mod anthrax_gamma_authored_tests {
    use super::*;

    #[test]
    fn gamma_completion_and_new_objects_use_authored_weapon_upgrade_only() {
        let mut world = GameLogic::new();
        world.add_player(Player::new(0, Team::GLA, "Owner", true));
        world.add_player(Player::new(7, Team::GLA, "Other", false));
        // Non-local owners with no active AI are intentionally barred from
        // combat admission, even when explicitly ordered to attack.
        world.ai_manager.add_ai_player(7, Team::GLA, crate::ai::AIDifficulty::Medium);
        world.set_ai_active(7, true);
        // Player::addUpgrade keys modules on the UpgradeTemplate mask; an
        // unknown name has no mask and triggers nothing.
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, "Chem_Upgrade_GLAAnthraxGamma", "PLAYER", 0, 1,
        );
        world.admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "gamma-recipients.ini".into(),
                "Object CustomToxinVehicle\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                 WeaponSet\nConditions = None\nWeapon = PRIMARY GammaTestBase\nEnd\n\
                 WeaponSet\nConditions = PLAYER_UPGRADE\nWeapon = PRIMARY GammaTestUpgraded\nEnd\n\
                 Behavior = WeaponSetUpgrade GammaTag\nTriggeredBy = Chem_Upgrade_GLAAnthraxGamma\nEnd\nEnd\n\
                 Object Chem_GLAVehicleToxinTruck\nKindOf = VEHICLE SELECTABLE CAN_ATTACK\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n\
                 WeaponSet\nConditions = None\nWeapon = PRIMARY GammaTestBase\nEnd\n\
                 WeaponSet\nConditions = PLAYER_UPGRADE\nWeapon = PRIMARY GammaTestUpgraded\nEnd\nEnd\n\
                 Object GammaTarget\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 1000\nInitialHealth = 1000\nEnd\nEnd\n".into(),
            )]).unwrap(),
        ).unwrap();
        let mut definitions = std::collections::HashMap::new();
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon GammaTestBase\nPrimaryDamage = 10\nPrimaryDamageRadius = 0\nAttackRange = 200\nDamageType = ARMOR_PIERCING\nDeathType = NORMAL\nWeaponSpeed = 999999\nDelayBetweenShots = 2000\nEnd\n\
             Weapon GammaTestUpgraded\nPrimaryDamage = 25\nPrimaryDamageRadius = 0\nAttackRange = 200\nDamageType = ARMOR_PIERCING\nDeathType = NORMAL\nWeaponSpeed = 999999\nDelayBetweenShots = 2000\nEnd\n",
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        );
        world.admit_weapon_content_snapshot(
            definitions.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
            &[],
        ).unwrap();
        let authored = world.create_object_for_player("CustomToxinVehicle", 0, glam::Vec3::ZERO).unwrap();
        let named = world.create_object_for_player("Chem_GLAVehicleToxinTruck", 0, glam::Vec3::ZERO).unwrap();
        let other = world.create_object_for_player("CustomToxinVehicle", 7, glam::Vec3::ZERO).unwrap();
        world.add_completed_player_upgrade(0, "Chem_Upgrade_GLAAnthraxGamma");
        let later = world.create_object_for_player("CustomToxinVehicle", 0, glam::Vec3::ZERO).unwrap();
        let later_other = world.create_object_for_player("CustomToxinVehicle", 7, glam::Vec3::ZERO).unwrap();
        for (index, (id, damage)) in [(authored, 25.0), (later, 25.0), (named, 10.0), (other, 10.0), (later_other, 10.0)].into_iter().enumerate() {
            assert_eq!(world.host_object(id).unwrap().weapon_slot(0).unwrap().damage, damage);
            let target = world.create_object("GammaTarget", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0)).unwrap();
            let start = 100 + index as u64 * 100;
            world.set_current_frame(start);
            {
                let (shooter, events) = world.host_object_and_attack_events_mut(id).unwrap();
                shooter.attack_target(target, start as u32, events, &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
            }
            for frame in start..start + 10 {
                world.set_current_frame(frame);
                world.update_combat(&[id, target], LOGIC_FRAME_TIMESTEP).unwrap();
                world.apply_owned_projectileless_delayed_damage();
                let _ = world.update_owned_projectile_impacts(LOGIC_FRAME_TIMESTEP);
            }
            assert_eq!(world.host_object(target).unwrap().health.current, 1000.0 - damage, "shooter {id:?}");
        }
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

//! Immutable projectile definitions owned by the receiving match content.
//! Parsing constructs definitions only; it never publishes a simulation owner.
use super::weapon_bootstrap::{HostProjectileFlight, HostProjectileLifecycle};
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::game_logic) enum ProjectileDefinitionMode {
    ReceivingContent,
    LegacyBootstrap,
}
#[derive(Debug, Clone)]
pub(in crate::game_logic) struct ProjectileProfile {
    pub(in crate::game_logic) flight: HostProjectileFlight,
    pub(in crate::game_logic) lifecycle: HostProjectileLifecycle,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct ProjectileDefinitions {
    entries: HashMap<String, ProjectileProfile>,
}
impl ProjectileDefinitions {
    pub(in crate::game_logic) fn get(&self, name: &str) -> Option<&ProjectileProfile> {
        self.entries.get(name)
    }
    /// Reconstruct projectile module data from the effective archive object row.
    /// Only definition parsing occurs here; factory callbacks are never executed.
    pub(in crate::game_logic) fn admit_definition(
        &mut self,
        name: &str,
        definition: &crate::assets::ini_parser::ObjectDefinition,
    ) -> bool {
        let mut text = format!("Object {name}\n");
        for (index, module) in definition.behavior_modules.iter().enumerate() {
            if !module.class_name.eq_ignore_ascii_case("MissileAIUpdate")
                && !module
                    .class_name
                    .eq_ignore_ascii_case("DumbProjectileBehavior")
            {
                continue;
            }
            text.push_str(&format!(
                " Behavior = {} ModuleTag_Owned{index}\n",
                module.class_name
            ));
            // Effective scalar attributes have no remaining source-order dependence.
            let mut attributes: Vec<_> = module.attributes.iter().collect();
            attributes.sort_by(|a, b| a.0.cmp(b.0));
            for (key, value) in attributes {
                text.push_str(&format!("  {key} = {value}\n"));
            }
            text.push_str(" End\n");
        }
        text.push_str("End\n");
        self.admit_object_ini(name, &text)
    }
    /// Existing ThingFactory parses the supplied immutable Object INI locally.
    pub(in crate::game_logic) fn admit_object_ini(&mut self, name: &str, text: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.entries.contains_key(name) {
            return false;
        }
        // Shared immutable module descriptors bootstrap the definition parser;
        // no mutable match or active-world slot is installed here.
        let _ = game_engine::common::thing::module_factory::init_module_factory();
        let mut factory = game_engine::common::thing::thing_factory::ThingFactory::new();
        if factory.load_ini_text(text) == 0 {
            return false;
        }
        let Some(template) = factory.find_template(name, false) else {
            return false;
        };
        for module in template.get_behavior_module_info().iter() {
            let class = module.name.as_str();
            let data = module.data.as_ref();
            let pair = if class.eq_ignore_ascii_case("MissileAIUpdate") {
                super::weapon_bootstrap::parse_missile_lifecycle(data).map(|lifecycle| {
                    ProjectileProfile {
                        flight: HostProjectileFlight::Missile(
                            super::weapon_bootstrap::parse_missile_flight(data),
                        ),
                        lifecycle,
                    }
                })
            } else if class.eq_ignore_ascii_case("DumbProjectileBehavior") {
                super::weapon_bootstrap::parse_dumb_projectile_lifecycle(data).map(|lifecycle| {
                    ProjectileProfile {
                        flight: HostProjectileFlight::Dumb(
                            super::weapon_bootstrap::parse_dumb_flight(data),
                        ),
                        lifecycle,
                    }
                })
            } else {
                continue;
            };
            let Some(profile) = pair else {
                return false;
            };
            self.entries.insert(name.into(), profile);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;
    fn world(delay: u32) -> (GameLogic, super::super::ObjectId, super::super::ObjectId) {
        let mut game = GameLogic::new();
        let mut owner = Player::new(0, Team::USA, "source", false);
        owner.alliance_team = 1;
        game.add_player(owner);
        let mut enemy = Player::new(1, Team::GLA, "target", false);
        enemy.alliance_team = 2;
        game.add_player(enemy);
        let text = format!(
            "Object SameMissile\n Behavior = MissileAIUpdate Profile\n IgnitionDelay = {delay}\n TryToFollowTarget = Yes\n End\nEnd"
        );
        let mut parser = crate::assets::ini_parser::IniParser::new();
        parser
            .parse_ini_content(&text, "receiving-profile.ini")
            .unwrap();
        let definitions: Vec<_> = parser
            .get_all_definitions()
            .iter()
            .map(|(name, row)| (name.clone(), row.clone()))
            .collect();
        let mut weapons = HashMap::new();
        assert_eq!(
            crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
                &mut weapons,
                "Weapon LocalProfileWeapon\n PrimaryDamage = 20\n PrimaryDamageRadius = 5\n AttackRange = 200\n WeaponSpeed = 300\n MinWeaponSpeed = 300\n ProjectileObject = SameMissile\n SuspendFXDelay = 233\n DelayBetweenShots = 0\n End",
                crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
            ),
            1
        );
        let native = weapons.remove("localprofileweapon").unwrap();
        let mut source = ThingTemplate::new("ProfileSource");
        source
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        source.set_primary_weapon_name(&native.name);
        game.admit_weapon_content_snapshot(vec![(native.name.clone(), native)], &definitions)
            .unwrap();
        // Invalid detached content cannot replace the admitted receiving owner.
        assert!(
            game.admit_weapon_content_snapshot(
                vec![(
                    " ".into(),
                    gamelogic::weapon::WeaponTemplate::new("bad".into())
                )],
                &definitions
            )
            .is_err()
        );
        game.templates.insert(source.name.clone(), source);
        let mut target = ThingTemplate::new("ProfileTarget");
        target
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        game.templates.insert(target.name.clone(), target);
        game.frame = 10;
        let source = game
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        let target = game
            .create_object_for_player("ProfileTarget", 1, Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        let admitted = game.host_object(source).unwrap();
        assert_eq!(admitted.selected_projectile_name(0), Some("SameMissile"));
        let weapon = admitted
            .weapon
            .as_ref()
            .expect("real factory must instantiate supplied native row");
        assert_eq!(weapon.damage, 20.0);
        assert_eq!(weapon.splash_radius, 5.0);
        assert_eq!(
            weapon.suspend_fx_frame, 17,
            "factory instantiation must use its receiving frame"
        );
        // Advance this receiving match independently of the peer match clock.
        game.frame = 40;
        (game, source, target)
    }
    #[test]
    fn unadmitted_native_projectile_rejects_before_ammo_and_effects() {
        let (mut owner, _, target) = world(0);
        owner.apply_receiving_weapon_overrides(&[
            "Weapon LocalProfileWeapon\n ProjectileObject = UnadmittedFlight\n ClipSize = 2\n End".into(),
        ]).unwrap();
        let source = owner
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        let object = owner.host_object(source).unwrap();
        let before = object.weapon.as_ref().unwrap().clone();
        let marker = object.last_fire_frame;
        assert_eq!(before.ammo, Some(2));
        assert!(!owner.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
        let object = owner.host_object(source).unwrap();
        let after = object.weapon.as_ref().unwrap();
        assert_eq!(after.ammo, before.ammo);
        assert_eq!(after.last_fire_time, before.last_fire_time);
        assert_eq!(object.last_fire_frame, marker);
        assert!(owner.combat_system.projectiles_snapshot().is_empty());
    }

    #[test]
    fn map_then_solo_weapon_overrides_are_receiving_owned_and_reset_to_base() {
        let (mut owner, _, _) = world(0);
        let (mut peer, _, _) = world(0);
        owner
            .apply_receiving_weapon_overrides(&[
                "Weapon LocalProfileWeapon\n PrimaryDamage = 0\n End".into(),
                "Weapon LocalProfileWeapon\n SecondaryDamage = 9\n End".into(),
            ])
            .unwrap();
        // A bad later draft leaves both the effective Map/Solo row and the
        // immutable startup base unchanged; no factory ever sees the NaN.
        assert!(
            owner
                .apply_receiving_weapon_overrides(&[
                    "Weapon LocalProfileWeapon\n PrimaryDamage = NaN\n End".into(),
                ])
                .is_err()
        );
        assert_eq!(
            owner.weapon_definition_base.as_ref().unwrap()["localprofileweapon"].primary_damage,
            20.0
        );
        let id = owner
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        let object = owner.host_object(id).unwrap();
        let rule = object.selected_native_weapon_rule(0).unwrap();
        assert_eq!(rule.primary_damage, 0.0);
        assert_eq!(rule.secondary_damage, 9.0);
        assert_eq!(
            rule.weapon_speed,
            300.0_f32 * (1.0_f32 / 30.0),
            "overrides inherit the receiving base row"
        );
        assert_eq!(
            object.weapon.as_ref().unwrap().damage,
            0.0,
            "zero is an allocated native weapon, not absent content"
        );
        let peer_id = peer
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        assert_eq!(
            peer.host_object(peer_id)
                .unwrap()
                .selected_native_weapon_rule(0)
                .unwrap()
                .primary_damage,
            20.0
        );
        owner.reset();
        let mut player = Player::new(0, Team::USA, "source", false);
        player.alliance_team = 1;
        owner.add_player(player);
        let fresh = owner
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        let rule = owner
            .host_object(fresh)
            .unwrap()
            .selected_native_weapon_rule(0)
            .unwrap();
        assert_eq!(rule.primary_damage, 20.0);
        assert_eq!(
            rule.secondary_damage, 0.0,
            "a fresh map does not inherit the preceding Solo override"
        );
    }

    #[test]
    fn encoded_save_reconstructs_receiving_native_rules_and_next_launch() {
        let (mut source_world, source, target) = world(0);
        assert!(source_world.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
        let builder = crate::save_load::snapshot::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source_world).unwrap();
        let bytes = bincode::serde::encode_to_vec(&snapshot, bincode::config::legacy()).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        // Definitions are external content, not mutable save state. This
        // independently admitted receiving match must retain its own rules.
        let (mut receiving, _, _) = world(1000);
        builder
            .restore_from_snapshot(&decoded, &mut receiving)
            .unwrap();
        let loaded = receiving.host_object(source).unwrap();
        let rule = loaded.selected_native_weapon_rule(0).unwrap();
        assert_eq!(rule.primary_damage, 20.0);
        assert_eq!(rule.projectile_name, "SameMissile");
        assert_eq!(loaded.weapon.as_ref().unwrap().suspend_fx_frame, 17);
        // Materialized flight continuation is independently missing from the
        // current save record (hq-k9p3l, Source307 negative runtime evidence).
        // This accepted control covers the saved firing owner and its NEXT
        // real launch through the receiving immutable profile, not live flight.
        assert_eq!(
            loaded.last_fire_frame,
            source_world.host_object(source).unwrap().last_fire_frame
        );
        assert!(receiving.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
        let shot = receiving
            .combat_system
            .projectiles_snapshot()
            .pop()
            .unwrap();
        let HostProjectileFlight::Missile(flight) = shot.flight.as_ref().unwrap() else {
            panic!("actual next receiving Missile launch");
        };
        assert_eq!(flight.ignition_delay_frames, 30);
        let HostProjectileFlight::Missile(profile) = &receiving
            .projectile_definitions
            .get("SameMissile")
            .unwrap()
            .flight
        else {
            panic!("receiving immutable profile");
        };
        assert_eq!(profile.ignition_delay_frames, 30);
    }

    #[test]
    fn same_name_receiving_profiles_are_isolated_and_reset_preserves_content() {
        let (mut owner, source, target) = world(0);
        let (mut peer, peer_source, peer_target) = world(1000);
        assert_eq!(source, peer_source);
        assert_eq!(target, peer_target);
        assert!(owner.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
        assert!(peer.fire_owned_passive_weapon(peer_source, peer_target, 0, Vec3::ZERO));
        let own = owner.combat_system.projectiles_snapshot().pop().unwrap();
        let other = peer.combat_system.projectiles_snapshot().pop().unwrap();
        assert_eq!(own.projectile_object_name, "SameMissile");
        assert_eq!(own.speed, (300.0_f32 * (1.0_f32 / 30.0)) * 30.0);
        let HostProjectileFlight::Missile(own) = own.flight.as_ref().unwrap() else {
            panic!("actual owned Missile profile")
        };
        let HostProjectileFlight::Missile(other) = other.flight.as_ref().unwrap() else {
            panic!("actual peer Missile profile")
        };
        assert_eq!(own.ignition_delay_frames, 0);
        assert_eq!(other.ignition_delay_frames, 30);
        owner.reset();
        let profile = owner.projectile_definitions.get("SameMissile").unwrap();
        let HostProjectileFlight::Missile(flight) = &profile.flight else {
            panic!("retained immutable Missile profile")
        };
        assert_eq!(flight.ignition_delay_frames, 0);
        let mut source_player = Player::new(0, Team::USA, "source", false);
        source_player.alliance_team = 1;
        owner.add_player(source_player);
        let mut target_player = Player::new(1, Team::GLA, "target", false);
        target_player.alliance_team = 2;
        owner.add_player(target_player);
        let source = owner
            .create_object_for_player("ProfileSource", 0, Vec3::ZERO)
            .unwrap();
        let target = owner
            .create_object_for_player("ProfileTarget", 1, Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        owner.frame = 40;
        assert!(owner.fire_owned_passive_weapon(source, target, 0, Vec3::ZERO));
        let shot = owner.combat_system.projectiles_snapshot().pop().unwrap();
        let HostProjectileFlight::Missile(flight) = shot.flight.as_ref().unwrap() else {
            panic!("reset actual materialization")
        };
        assert_eq!(flight.ignition_delay_frames, 0);
    }
}

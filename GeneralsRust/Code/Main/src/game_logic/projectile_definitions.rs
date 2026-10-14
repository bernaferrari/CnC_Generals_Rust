//! Immutable projectile definitions owned by the receiving match content.
//! Parsing constructs definitions only; it never publishes a simulation owner.
use super::weapon_bootstrap::{HostProjectileFlight, HostProjectileLifecycle};
use std::collections::HashMap;
#[derive(Debug, Clone)]
pub(in crate::game_logic) struct ProjectileProfile {
    pub(in crate::game_logic) flight: HostProjectileFlight,
    pub(in crate::game_logic) lifecycle: HostProjectileLifecycle,
}
#[derive(Debug, Clone, Default)]
pub(in crate::game_logic) struct ProjectileDefinitions {
    entries: HashMap<String, ProjectileProfile>,
}
impl ProjectileDefinitions {
    pub(in crate::game_logic) fn get(&self, name: &str) -> Option<&ProjectileProfile> {
        self.entries.get(name)
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
        assert!(
            game.projectile_definitions
                .admit_object_ini("SameMissile", &text)
        );
        assert!(
            !game
                .projectile_definitions
                .admit_object_ini("SameMissile", &text)
        );
        assert!(!game.projectile_definitions.admit_object_ini(" ", &text));
        let mut native = gamelogic::weapon::WeaponTemplate::new("LocalProfileWeapon".into());
        native.primary_damage = 20.0;
        native.primary_damage_radius = 5.0;
        native.attack_range = 200.0;
        native.weapon_speed = 10.0;
        native.min_weapon_speed = 10.0;
        native.projectile_name = "SameMissile".into();
        native.suspend_fx_delay = 7;
        let mut source = ThingTemplate::new("ProfileSource");
        source
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable);
        source.bind_native_weapon_row([Some(&native), None, None]);
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
        // Current zero-delay stat compatibility maps to a one-second
        // readiness interval; this profile test admits a legitimately ready
        // shot rather than changing that separately tracked timing policy.
        game.frame = 40;
        (game, source, target)
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
        assert_eq!(own.speed, 300.0);
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

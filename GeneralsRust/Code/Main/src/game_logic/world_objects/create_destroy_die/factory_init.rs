//! Object::initObject player-upgrade-sensitive host bootstrap.
//! Called only after the registered Object's semantic Drawable has been born
//! and bound. Constructor module identity, team and production veterancy remain
//! in the factory constructor phase; these are ordinary borrows, not mirrors.
use super::*;

impl GameLogic {
    pub(super) fn apply_factory_init_bootstrap_upgrades(
        &mut self,
        id: ObjectId,
        template_name: &str,
        team: Team,
        owner_player_id: Option<u32>,
        sentry_had_explicit_primary: bool,
    ) {
        let frame = self.frame;
        let no_players = std::collections::HashMap::new();
        let no_upgrade_mask = std::collections::HashSet::new();
        let Some(object) = self.objects.get_mut(&id) else {
            return;
        };
        let allow_upgrades = !object.status.under_construction && !object.status.destroyed;
        let players = if allow_upgrades {
            &self.players
        } else {
            &no_players
        };
        let has_anthrax_gamma = {
            use crate::game_logic::host_toxin_tractor::is_anthrax_gamma_upgrade_name;
            let player_has_gamma = |player: &Player| {
                player
                    .unlocked_sciences
                    .iter()
                    .chain(player.completed_upgrades.iter())
                    .any(|name| is_anthrax_gamma_upgrade_name(name))
            };
            if let Some(player) = owner_player_id.and_then(|pid| players.get(&pid)) {
                player_has_gamma(player)
            } else {
                players
                    .values()
                    .filter(|p| p.team == team)
                    .any(player_has_gamma)
            }
        };
        if has_anthrax_gamma {
            if let (Some(current), Some(upgraded)) = (
                object.mine_data.as_mut(),
                crate::game_logic::host_mines::residual_data_for_template(
                    template_name,
                    frame,
                    true,
                ),
            ) {
                // A weapon upgrade changes weapon/profile values, not the
                // module's already-running timer, immunity or trigger state.
                current.detonation_damage = upgraded.detonation_damage;
                current.detonation_radius = upgraded.detonation_radius;
                current.secondary_damage = upgraded.secondary_damage;
                current.secondary_radius = upgraded.secondary_radius;
                current.demo_trap_profile = upgraded.demo_trap_profile;
            }
        }

        if crate::game_logic::host_sentry_drone::sentry_spawn_is_detector(template_name) {
            // Retail WeaponSet Conditions=None has PRIMARY None until PLAYER_UPGRADE.
            // Strip kind-based Weapon::default fallback from resolve_primary_weapon.
            // C++ initObject → updateUpgradeModules applies completed player
            // WeaponSetUpgrade so later builds spawn with SentryDroneGun.
            use crate::game_logic::host_sentry_drone::UPGRADE_AMERICA_SENTRY_DRONE_GUN;
            let has_gun_upgrade = (allow_upgrades
                && object.has_upgrade_tag(UPGRADE_AMERICA_SENTRY_DRONE_GUN))
                || if let Some(player) = owner_player_id.and_then(|pid| players.get(&pid)) {
                    player.has_unlocked_upgrade(UPGRADE_AMERICA_SENTRY_DRONE_GUN)
                } else {
                    players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_SENTRY_DRONE_GUN)
                    })
                };
            if has_gun_upgrade {
                Self::equip_sentry_drone_gun(object);
            } else if !sentry_had_explicit_primary {
                object.weapon = None;
            }
        }

        // Host residual: China Dragon Tank primary flame weapon bind.
        // Fail-closed: FireWall secondary is host_firewall special-power residual.
        if crate::game_logic::host_dragon_tank::is_dragon_tank_template(template_name) {
            use crate::game_logic::host_dragon_tank::{
                dragon_flame_weapon, has_black_napalm_upgrade,
            };
            let upgraded = has_black_napalm_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            // Force residual flame stats when store/template leaves defaults.
            object.weapon = Some(dragon_flame_weapon(upgraded));
        }

        // Host residual: China Nuke Cannon neutron secondary is PLAYER_UPGRADE only.
        // Fail-closed: Upgrade_ChinaNeutronShells equips SECONDARY; without it, no secondary.
        // Explicit template.secondary_weapon_name (tests / seeds) still keeps a bound weapon.
        if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
            use crate::game_logic::host_neutron_shell::UPGRADE_CHINA_NEUTRON_SHELLS;
            use crate::game_logic::weapon_bootstrap::{
                NUKE_CANNON_NEUTRON_WEAPON, ensure_host_weapon_store,
            };
            let has_neutron = (allow_upgrades
                && object.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS))
                || (allow_upgrades && object.has_upgrade_tag("Upgrade_ChinaNeutronShells"))
                || players.values().any(|p| {
                    p.team == team && p.has_unlocked_upgrade(UPGRADE_CHINA_NEUTRON_SHELLS)
                });
            if has_neutron {
                ensure_host_weapon_store();
                if let Some(w) = ThingTemplate::weapon_from_store(NUKE_CANNON_NEUTRON_WEAPON) {
                    object.secondary_weapon = Some(w);
                }
                object.apply_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS);
            } else if object.thing().template.secondary_weapon_name.is_none()
                && object.thing().template.secondary_weapon.is_none()
            {
                // Strip residual map auto-equip; keep explicit test/seed secondaries.
                object.secondary_weapon = None;
            }
        }

        // Host residual: China Gattling Tank dual ground/AA + continuous-fire ramp state.
        // Fail-closed: not Overlord/Helix/building gattling payloads.
        if crate::game_logic::host_gattling_tank::is_gattling_tank_template(template_name) {
            use crate::game_logic::host_gattling_tank::{
                GattlingFireLevel, gattling_air_weapon, gattling_ground_weapon,
                has_chain_guns_upgrade,
            };
            let chain = has_chain_guns_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(gattling_ground_weapon(GattlingFireLevel::Base, chain));
            object.secondary_weapon = Some(gattling_air_weapon(GattlingFireLevel::Base, chain));
            reset_continuous_fire_state(object);
        }

        // Host residual: China Gattling Cannon structure dual ground/AA + continuous-fire ramp.
        // Fail-closed: not full CONTINUOUS_FIRE_* model-condition animation matrix.
        if crate::game_logic::host_base_defense::is_gattling_cannon_structure(template_name) {
            use crate::game_logic::host_base_defense::{
                gattling_building_air_weapon, gattling_building_ground_weapon,
                gattling_building_has_chain_guns,
            };
            use crate::game_logic::host_gattling_tank::GattlingFireLevel;
            let chain = gattling_building_has_chain_guns(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(gattling_building_ground_weapon(
                GattlingFireLevel::Base,
                chain,
            ));
            object.secondary_weapon =
                Some(gattling_building_air_weapon(GattlingFireLevel::Base, chain));
            reset_continuous_fire_state(object);
        }

        if crate::game_logic::host_base_defense::is_stinger_site_structure(template_name) {
            use crate::game_logic::host_base_defense::{
                stinger_air_weapon, stinger_ground_weapon, stinger_has_ap_rockets,
            };
            let ap = stinger_has_ap_rockets(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(stinger_ground_weapon(ap));
            object.secondary_weapon = Some(stinger_air_weapon(ap));
        }

        // Host residual: GLA Scorpion PRIMARY gun (+ secondary rocket if unlocked).
        // Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
        if crate::game_logic::host_scorpion::is_scorpion_template(template_name) {
            use crate::game_logic::host_scorpion::{
                has_ap_rockets_upgrade, has_scorpion_rocket_upgrade, salvage_tier_from_upgrades,
                scorpion_gun_weapon, scorpion_missile_weapon,
            };
            let tier = salvage_tier_from_upgrades(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(scorpion_gun_weapon(tier));
            if has_scorpion_rocket_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            }) {
                let ap = has_ap_rockets_upgrade(if allow_upgrades {
                    &object.applied_upgrades
                } else {
                    &no_upgrade_mask
                });
                object.secondary_weapon =
                    Some(scorpion_missile_weapon(ap, tier.dual_missile_clip()));
            }
        }

        // Host residual: USA Raptor PRIMARY jet missiles (+ Laser Missiles upgrade).
        // RETURN_TO_BASE ClipReload airfield rearm residual closed (dock + timer).
        if crate::game_logic::host_raptor::is_raptor_template(template_name) {
            use crate::game_logic::host_raptor::{
                has_laser_missiles_upgrade, is_king_raptor_template, raptor_weapon,
            };
            let king = is_king_raptor_template(template_name);
            let laser = has_laser_missiles_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(raptor_weapon(king, laser));
        }

        // Host residual: China MiG PRIMARY napalm / Nuke dual-radius missiles.
        // Fail-closed: not full RETURN_TO_BASE ClipReload / HistoricBonus Firestorm matrix.
        if crate::game_logic::host_mig::is_mig_template(template_name) {
            use crate::game_logic::host_mig::{is_nuke_mig_template, mig_loadout, mig_weapon};
            let loadout = mig_loadout(
                is_nuke_mig_template(template_name),
                if allow_upgrades {
                    &object.applied_upgrades
                } else {
                    &no_upgrade_mask
                },
            );
            object.weapon = Some(mig_weapon(loadout));
        }

        // Host residual: USA Comanche PRIMARY 20mm + SECONDARY anti-tank.
        // Retail rocket pods are a PLAYER_UPGRADE TERTIARY weapon; keep
        // anti-tank bound in SECONDARY and only expose pods after the team
        // owns the real upgrade.
        if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(template_name) {
            use crate::game_logic::host_comanche_rocket_pods::{
                UPGRADE_COMANCHE_ROCKET_PODS, comanche_antitank_weapon, comanche_cannon_weapon,
                comanche_rocket_pod_weapon,
            };
            object.weapon = Some(comanche_cannon_weapon());
            object.secondary_weapon = Some(comanche_antitank_weapon());
            let has_pods = (allow_upgrades && object.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS))
                || (allow_upgrades && object.has_upgrade_tag("Upgrade_ComancheRocketPods"))
                || players.values().any(|player| {
                    player.team == team && player.has_unlocked_upgrade(UPGRADE_COMANCHE_ROCKET_PODS)
                });
            if has_pods {
                object.tertiary_weapon = Some(comanche_rocket_pod_weapon());
                object.apply_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS);
                object.weapon_set_player_upgrade = true;
            } else {
                // The simple ObjectDefinition parser preserves the source
                // name but does not evaluate full WeaponSet Conditions.
                // Do not grant a condition-gated pod declaration early.
                object.tertiary_weapon = None;
            }
        }

        // Host residual: China Overlord / Emperor PRIMARY dual-radius tank gun.
        // Fail-closed: not full ClipSize=2 dual-volley / Nuclear Tanks death residual.
        if crate::game_logic::host_overlord_gun::is_overlord_gun_chassis(template_name) {
            use crate::game_logic::host_overlord_gun::{
                has_uranium_shells_upgrade, overlord_gun_weapon,
            };
            let uranium = has_uranium_shells_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(overlord_gun_weapon(uranium));
        }

        // Host residual: GLA Jarmen Kell PRIMARY sniper residual.
        // Pilot-snipe is AutoChooseSources SECONDARY NONE (button/special only).
        if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
            use crate::game_logic::host_jarmen_kell::{has_ap_bullets_upgrade, jarmen_kell_weapon};
            let ap = has_ap_bullets_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(jarmen_kell_weapon(ap));
            object.template_mut().apply_retail_button_only_auto_choose();
        }

        // Host residual: GLA Rebel PRIMARY machine gun residual.
        // Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
        if crate::game_logic::host_gla_rebel::is_gla_rebel_template(template_name) {
            use crate::game_logic::host_gla_rebel::{has_ap_bullets_upgrade, rebel_weapon};
            let ap = has_ap_bullets_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(rebel_weapon(ap));
        }

        // Host residual: USA Ranger PRIMARY rifle residual.
        // FlashBang secondary is PLAYER_UPGRADE only (Upgrade_AmericaRangerFlashBangGrenade)
        // — parity with neutron shells / rocket pods: residual map may name the weapon,
        // but create strips it unless research is unlocked or template explicitly seeds it.
        // Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
        if crate::game_logic::host_ranger::is_ranger_template(template_name) {
            use crate::game_logic::host_ranger::{
                UPGRADE_AMERICA_FLASHBANG, has_flashbang_equipped, ranger_flashbang_weapon,
                ranger_rifle_weapon,
            };
            // C++ WeaponSet::updateWeaponSet uses the authored primary.
            // Only supply the retail seed when this template has no rule.
            if object.thing().template.primary_weapon_name.is_none()
                && object.thing().template.primary_weapon.is_none()
                && !object.thing().template.primary_weapon_explicitly_none
            {
                object.weapon = Some(ranger_rifle_weapon());
            }
            let has_flashbang = has_flashbang_equipped(
                false,
                if allow_upgrades {
                    &object.applied_upgrades
                } else {
                    &no_upgrade_mask
                },
            ) || players
                .values()
                .any(|p| p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG));
            if has_flashbang {
                object.secondary_weapon = Some(ranger_flashbang_weapon());
                object.apply_upgrade_tag(UPGRADE_AMERICA_FLASHBANG);
            } else if object.thing().template.secondary_weapon_name.is_none()
                && object.thing().template.secondary_weapon.is_none()
            {
                // Strip residual map auto-equip; keep explicit test/seed secondaries.
                object.secondary_weapon = None;
            } else if object.secondary_weapon.is_some() {
                // Explicit seed/test secondary — normalize to residual flashbang stats.
                object.secondary_weapon = Some(ranger_flashbang_weapon());
            }
        }

        // Host residual: China MiniGunner dual ground/AA + continuous fire ramp.
        // Fail-closed: not full FiringTracker CONTINUOUS_FIRE_* anim / bayonet tertiary.
        if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
            use crate::game_logic::host_gattling_tank::GattlingFireLevel;
            use crate::game_logic::host_minigunner::{
                has_chain_guns_upgrade, minigunner_air_weapon, minigunner_ground_weapon,
            };
            let chain = has_chain_guns_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(minigunner_ground_weapon(
                GattlingFireLevel::Base,
                chain,
                false,
                false,
            ));
            object.secondary_weapon = Some(minigunner_air_weapon(
                GattlingFireLevel::Base,
                chain,
                false,
                false,
            ));
            reset_continuous_fire_state(object);
        }

        // C++ Object::initObject → updateUpgradeModules: PLAYER_UPGRADE mask
        // (Upgrade_GLAWorkerShoes) fires LocomotorSetUpgrade on new workers.
        // Live research only stamps objects alive at complete; inherit here.
        // Owner-player only — same-faction leak is not C++ getControllingPlayer.
        if crate::game_logic::host_gla_worker::is_gla_worker_template(template_name) {
            use crate::game_logic::host_gla_worker::{
                UPGRADE_GLA_WORKER_SHOES, worker_residual_speed,
            };
            let player_has_shoes = owner_player_id
                .and_then(|pid| players.get(&pid))
                .is_some_and(|p| p.has_unlocked_upgrade(UPGRADE_GLA_WORKER_SHOES));
            let shoes = (allow_upgrades && object.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES))
                || player_has_shoes;
            if shoes && !(allow_upgrades && object.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES)) {
                object.apply_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
                let _ =
                    crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_upgrade(
                        object,
                        UPGRADE_GLA_WORKER_SHOES,
                    );
            }
            object.movement.max_speed = worker_residual_speed(shoes);
        }

        // Host residual: GLA RPG Trooper / Tunnel Defender PRIMARY rocket residual.
        // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
        if crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(template_name) {
            use crate::game_logic::host_rpg_trooper::{has_ap_rockets_upgrade, rpg_trooper_weapon};
            let ap = has_ap_rockets_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(rpg_trooper_weapon(ap));
        }

        // Host residual: GLA Terrorist PRIMARY TerroristSuicideWeapon residual.
        // Chem Beta/Gamma + Demo death-weapon residual profiles.
        // Fail-closed: not ConvertToCarBomb full matrix / SlowDeath fling.
        if crate::game_logic::host_terrorist::is_terrorist_template(template_name) {
            use crate::game_logic::host_terrorist::{
                terrorist_death_profile, terrorist_suicide_weapon_for_profile,
            };
            let has_gamma = (allow_upgrades
                && object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma"))
                || (allow_upgrades && object.has_upgrade_tag("Upgrade_GLAAnthraxGamma"));
            let has_beta = (allow_upgrades && object.has_upgrade_tag("Upgrade_GLAAnthraxBeta"))
                || (allow_upgrades && object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxBeta"));
            let profile = terrorist_death_profile(template_name, has_gamma, has_beta);
            object.weapon = Some(terrorist_suicide_weapon_for_profile(profile));
            object.secondary_weapon = None;
        }

        // C++ Object::initObject → updateUpgradeModules: PLAYER_UPGRADE
        // Upgrade_AmericaDroneArmor fires MaxHealthUpgrade on new drones.
        // Live research only stamps drones alive at complete; inherit here.
        // Owner-player only — same-faction leak is not C++ getControllingPlayer.
        if let Some(kind) =
            crate::game_logic::host_slave_drones::slave_drone_kind_from_template(template_name)
        {
            use crate::game_logic::host_slave_drones::{
                UPGRADE_AMERICA_DRONE_ARMOR, apply_drone_armor_health,
            };
            let player_has_armor = owner_player_id
                .and_then(|pid| players.get(&pid))
                .is_some_and(|p| p.has_unlocked_upgrade(UPGRADE_AMERICA_DRONE_ARMOR));
            if player_has_armor
                && !(allow_upgrades && object.has_upgrade_tag(UPGRADE_AMERICA_DRONE_ARMOR))
            {
                object.apply_upgrade_tag(UPGRADE_AMERICA_DRONE_ARMOR);
                let mut max_h = object.max_health;
                let mut cur = object.health.current;
                let mut maximum = object.health.maximum;
                apply_drone_armor_health(kind, &mut max_h, &mut cur, &mut maximum);
                object.set_body_max_health(max_h);
                object.health.maximum = maximum;
                object.internal_change_health(cur - object.health.current);
            }
        }
    }
}

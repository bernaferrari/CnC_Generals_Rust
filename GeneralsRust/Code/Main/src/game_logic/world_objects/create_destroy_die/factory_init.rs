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

        // China Dragon Tank PRIMARY binds from the authored WeaponSet (C++
        // WeaponSet::updateWeaponSet from ThingTemplate), not a template-name
        // host table (hq-2qedt.158). Black Napalm refresh stays upgrade-time.

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
        // Keyed on the authored weapon, not the template name (hq-2qedt.266).
        if crate::game_logic::host_base_defense::is_authored_gattling_building(
            object.weapon_name_for_slot(0),
        ) {
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
        // C++ Object::initObject builds a fresh FiringTracker per object, so
        // continuous-fire ramp state starts cold for every chassis; Gattling
        // Tank/Cannon weapons bind from the authored WeaponSet (hq-2qedt.159/
        // .160), not a template-name gate (hq-2qedt.240). Chain-guns refresh
        // stays upgrade-time.
        reset_continuous_fire_state(object);

        // Stinger Site PRIMARY/SECONDARY bind from the authored WeaponSet
        // (C++ WeaponSet::updateWeaponSet), not a template-name peel
        // (hq-2qedt.163). AP rockets refresh stays on the upgrade path.

        // GLA Scorpion PRIMARY binds from the authored WeaponSet (C++
        // WeaponSet::updateWeaponSet from ThingTemplate), not a name peel
        // (hq-2qedt.154). Residual: PLAYER_UPGRADE rocket SECONDARY only.
        // Gate keys on authored PRIMARY (ScorpionTankGun family), not a
        // template-name sniff (hq-2qedt.208; C++ WeaponSetUpgrade).
        // Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
        if super::super::super::world_combat::ocl_and_scud::authored_scorpion_gun(object) {
            use crate::game_logic::host_scorpion::{
                has_ap_rockets_upgrade, has_scorpion_rocket_upgrade, salvage_tier_from_upgrades,
                scorpion_missile_weapon,
            };
            let tier = salvage_tier_from_upgrades(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
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

        // USA Raptor / King Raptor PRIMARY binds from the authored WeaponSet
        // (C++ WeaponSet::updateWeaponSet), not a template-name peel
        // (hq-2qedt.220). Laser Missiles refresh stays on the upgrade path.

        // China MiG PRIMARY binds from the authored WeaponSet (C++
        // WeaponSet::updateWeaponSet from ThingTemplate), not a name peel
        // (hq-2qedt.156). Black Napalm / Tactical Nuke upgrade refresh is unchanged.

        // USA Comanche PRIMARY/SECONDARY bind from the authored WeaponSet
        // (C++ WeaponSet::updateWeaponSet), not this name peel (hq-2qedt.109).
        // Retail rocket pods are a PLAYER_UPGRADE TERTIARY weapon; only expose
        // pods after the team owns the real upgrade.
        if crate::game_logic::host_comanche_rocket_pods::carries_comanche_weapon_set(
            object.authored_weapon_name_for_slot(0),
        ) {
            use crate::game_logic::host_comanche_rocket_pods::{
                UPGRADE_COMANCHE_ROCKET_PODS, comanche_rocket_pod_weapon,
            };
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

        // China Overlord / Emperor PRIMARY binds from the authored WeaponSet
        // (C++ WeaponSet::updateWeaponSet), not a template-name peel
        // (hq-2qedt.164). Uranium refresh stays on the upgrade path.

        // GLA Jarmen Kell PRIMARY binds from the authored WeaponSet
        // (C++ WeaponSet::updateWeaponSet), not a template-name peel
        // (hq-2qedt.165). AP refresh stays on the upgrade path.
        // Pilot-snipe is AutoChooseSources SECONDARY NONE (button/special only).
        if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
            object.template_mut().apply_retail_button_only_auto_choose();
        }

        // Host residual: GLA Rebel PRIMARY machine gun residual.
        // Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
        // Keys on the authored GLARebelMachineGun primary, not chassis name (hq-2qedt.243).
        if object.authored_weapon_name_for_slot(0).is_some_and(|w| {
            w.eq_ignore_ascii_case(crate::game_logic::host_gla_rebel::REBEL_MACHINE_GUN)
        }) {
            use crate::game_logic::host_gla_rebel::{has_ap_bullets_upgrade, rebel_weapon};
            let ap = has_ap_bullets_upgrade(if allow_upgrades {
                &object.applied_upgrades
            } else {
                &no_upgrade_mask
            });
            object.weapon = Some(rebel_weapon(ap));
        }

        // Host residual: USA Ranger FlashBang SECONDARY residual.
        // FlashBang secondary is PLAYER_UPGRADE only (Upgrade_AmericaRangerFlashBangGrenade)
        // — parity with neutron shells / rocket pods: residual map may name the weapon,
        // but create strips it unless research is unlocked or template explicitly seeds it.
        // Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
        // Keys on the authored RangerAdvancedCombatRifle / FlashBang slots, not chassis
        // name (hq-2qedt.244).
        if object.authored_weapon_name_for_slot(0).is_some_and(|w| {
            w.eq_ignore_ascii_case(crate::game_logic::host_ranger::RANGER_RIFLE_WEAPON)
        }) || object.authored_weapon_name_for_slot(1).is_some_and(|w| {
            w.eq_ignore_ascii_case(crate::game_logic::host_ranger::RANGER_FLASHBANG_WEAPON)
        }) {
            use crate::game_logic::host_ranger::{
                UPGRADE_AMERICA_FLASHBANG, has_flashbang_equipped, ranger_flashbang_weapon,
            };
            // PRIMARY binds from the authored WeaponSet above (C++
            // WeaponSet::updateWeaponSet), not a name peel (hq-2qedt.168).
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

        // China MiniGunner PRIMARY/SECONDARY bind from the authored WeaponSet
        // (C++ WeaponSet::updateWeaponSet), not a template-name peel
        // (hq-2qedt.166). Chain-guns refresh stays in refresh_minigunner_weapon.
        if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
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
                    crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_upgrade_in(
                        object,
                        UPGRADE_GLA_WORKER_SHOES,
                        self.locomotor_definitions.as_ref(),
                    );
            }
            if object
                .thing()
                .template
                .receiving_object_definition
                .is_none()
            {
                object.movement.max_speed = worker_residual_speed(shoes);
            }
        }

        // Host residual: GLA RPG Trooper / Tunnel Defender PRIMARY rocket residual.
        // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
        // RPG Trooper PRIMARY binds from the authored WeaponSet (C++
        // WeaponSet::updateWeaponSet), not a template-name peel (hq-2qedt.149).
        // AP Rockets stay on refresh_rpg_trooper_weapon.

        // GLA Terrorist PRIMARY binds from the authored WeaponSet above (C++
        // WeaponSet::updateWeaponSet), not a name peel (hq-2qedt.169).
        // Death-profile / demo charge residuals live in host_terrorist.

        // C++ Object::initObject -> updateUpgradeModules: the owning player's
        // completed upgrades run this template's authored MaxHealthUpgrade
        // modules (Composite Armor, Drone Armor, Aircraft Armor, ...).
        if let Some(player) = owner_player_id.and_then(|pid| players.get(&pid)) {
            object.run_authored_max_health_upgrades(|name| player.has_unlocked_upgrade(name));
        }
    }
}

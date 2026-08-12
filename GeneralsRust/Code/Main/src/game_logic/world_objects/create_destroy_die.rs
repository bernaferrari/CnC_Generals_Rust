//! Host objects `impl GameLogic` — `create_destroy_die`.
//! create/destroy, mark_object_for_destruction, die, dam. Child of `world_objects` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Create a new object
    pub fn create_object(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        // Map-load skip list: decorative / overloaded templates (AngryMob nexus
        // projectiles, cinematic shells, …). Intentional residual / test spawns
        // that already registered a template are fail-open (host Angry Mob path).
        if Self::should_skip_map_object_template(template_name)
            && !self.templates.contains_key(template_name)
        {
            return None;
        }

        if !self.templates.contains_key(template_name) {
            let mut injected = false;
            let should_spawn_fallback = Self::should_spawn_fallback_template(template_name);

            if let Some(template) = Self::build_template_from_asset_definition(template_name) {
                let missing_model = template
                    .model_name
                    .as_deref()
                    .filter(|model| !Self::is_model_asset_available(model))
                    .map(|model| model.to_string());

                if missing_model.is_none() || should_spawn_fallback {
                    self.templates.insert(template_name.to_string(), template);
                    injected = true;
                    log::debug!(
                        "Synthesized template for '{}' from WW3D object definitions",
                        template_name
                    );
                } else if let Some(model) = missing_model {
                    log::debug!(
                        "Falling back for decorative map object template '{}' after unavailable definition model '{}'",
                        template_name,
                        model
                    );
                }
            }

            if !injected {
                if let Some(fallback_template) = Self::build_visual_fallback_template(template_name)
                {
                    let model_name = fallback_template
                        .model_name
                        .clone()
                        .unwrap_or_else(|| template_name.to_string());
                    self.templates
                        .insert(template_name.to_string(), fallback_template);
                    if should_spawn_fallback {
                        log::warn!(
                            "Injected fallback template for unresolved object '{}' using model '{}'",
                            template_name,
                            model_name
                        );
                    } else {
                        log::debug!(
                            "Injected visual-only fallback template for decorative object '{}' using model '{}'",
                            template_name,
                            model_name
                        );
                    }
                } else if !should_spawn_fallback {
                    log::debug!(
                        "Skipping unsupported decorative map object template '{}'",
                        template_name
                    );
                    return None;
                } else {
                    let fallback_template = Self::build_fallback_template(template_name);
                    self.templates
                        .insert(template_name.to_string(), fallback_template);
                    log::warn!(
                        "Injected fallback template for unresolved object '{}'",
                        template_name
                    );
                }
            }
        }

        if let Some(template) = self.templates.get(template_name).cloned() {
            let is_structure = template.is_kind_of(KindOf::Structure);
            let counts_as_unit = Self::template_counts_as_unit(&template);
            let id = self.allocate_object_id();
            // Resolve weapons / locomotor before move into Object.
            let weapon = template.resolve_primary_weapon();
            let secondary_weapon = template.resolve_secondary_weapon();
            let tertiary_weapon = template.resolve_tertiary_weapon();
            let movement_stats = template.resolve_movement();
            // Sentry residual: detect explicit template primary before move.
            let sentry_had_explicit_primary =
                template.primary_weapon.is_some() || template.primary_weapon_name.is_some();
            let mut object = Object::new(template, id, team);
            object.set_position(position);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    id,
                    Some([position.x, position.y, position.z]),
                );
                object.record_host_movement();
            }
            let starts_under_construction = object.status.under_construction;

            // Primary weapon from template when defined; kind-based fallback only as last resort.
            if let Some(weapon) = weapon {
                object.weapon = Some(weapon);
            }
            // Secondary slot: fail-closed (only when template names/stats resolve).
            if let Some(secondary) = secondary_weapon {
                object.secondary_weapon = Some(secondary);
            }
            // Tertiary is a separate WeaponSet storage slot.  Conditional
            // templates may strip/rebind it below (for example Comanche pods),
            // but it must never overwrite SECONDARY on creation.
            if let Some(tertiary) = tertiary_weapon {
                object.tertiary_weapon = Some(tertiary);
            }

            // Strategy Center residual: PRIMARY StrategyCenterGun exists in retail but
            // AutoChooseSources=PRIMARY NONE and turret starts disabled until Bombardment
            // (C++ enableTurret). Strip kind-based Weapon::default fallback; Bombardment
            // residual re-equips StrategyCenterGun. Explicit template primary still keeps.
            if crate::game_logic::host_strategy_center::is_strategy_center_template(template_name)
                || object.is_kind_of(KindOf::FSStrategyCenter)
            {
                // Fail-closed: strip kind-based Weapon::default unless already
                // StrategyCenterGun residual (Bombardment mid-game recreate).
                use crate::game_logic::host_strategy_center::STRATEGY_CENTER_GUN_DAMAGE;
                let is_gun = object.weapon.as_ref().is_some_and(|w| {
                    (w.damage - STRATEGY_CENTER_GUN_DAMAGE).abs() < 0.001
                        && (w.range - 400.0).abs() < 0.001
                });
                if !is_gun {
                    object.weapon = None;
                    object.secondary_weapon = None;
                }
            }

            // GLA Quad Cannon residual: force air/ground anti masks on dual weapons.
            // Fail-closed vs full Weapon.ini AntiGround/AntiAirborne parse when store
            // templates leave default GROUND mask on AA secondary.
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                if let Some(w) = object.weapon.as_mut() {
                    w.can_target_ground = true;
                    w.can_target_air = false;
                }
                if let Some(w) = object.secondary_weapon.as_mut() {
                    w.can_target_air = true;
                    w.can_target_ground = false;
                }
            }

            // GLA Toxin Tractor residual: ensure contaminate spray secondary binds.
            // Retail PrimaryDamage=0 fails weapon_from_store gate; host residual installs
            // a ready secondary for AutoChooseSources=NONE special-attack residual.
            if crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(template_name) {
                object.fire_ocl_after_cooldown = Some(
                    crate::game_logic::host_toxin_tractor::HostFireOclAfterCooldownData::new(),
                );
                if object.secondary_weapon.is_none() {
                    use crate::game_logic::host_toxin_tractor::{
                        delay_frames_to_reload_secs, TOXIN_SPRAY_DELAY_FRAMES, TOXIN_SPRAY_RANGE,
                    };
                    object.secondary_weapon = Some(Weapon {
                        damage: 0.001,
                        range: TOXIN_SPRAY_RANGE,
                        min_range: 0.0,
                        reload_time: delay_frames_to_reload_secs(TOXIN_SPRAY_DELAY_FRAMES),
                        last_fire_time: 0.0,
                        ammo: None,
                        clip_size: 0,
                        clip_reload_time: 0.0,
                        can_target_air: false,
                        can_target_ground: true,
                        projectile_speed: 600.0,
                        pre_attack_delay: 0.0,
                        splash_radius: 0.0,
                    });
                }
            }

            // Locomotor catalog → host Movement (retail BasicHumanLocomotor ~20 u/s).
            // Fail-closed: only when template sets locomotor_name and store resolves.
            // Prefer catalog over Movement::default() (10) so golden skirmish does not
            // need a march-speed boost when the host seed/INI path is present.
            if let Some(stats) = movement_stats {
                object.movement.max_speed = stats.max_speed;
                object.movement.acceleration = stats.acceleration;
                object.movement.turn_rate = stats.turn_rate;
            }

            // Host residual: bind mine/demo-trap data for recognized templates.
            if let Some(mine_data) =
                crate::game_logic::host_mines::residual_data_for_template(template_name, self.frame)
            {
                object.mine_data = Some(mine_data);
                object.record_host_demo_mine_cheer();
            }

            // Host residual: GLA Battle Bus TransportContain Slots=8 + passenger fire.
            if crate::game_logic::host_battle_bus::is_battle_bus_template(template_name) {
                object.install_battle_bus_transport();
            }
            if crate::game_logic::host_highlander_body::is_highlander_body_template(template_name) {
                object.install_highlander_body();
            }
            object.install_deploy_style_if_needed();
            object.install_tensile_formation_if_needed();
            if object.has_tensile_formation() {
                self.tensile_formation_reg.record_install();
            }
            object.install_fire_spread_if_needed();
            if object.has_fire_spread() {
                self.fire_spread_reg.record_install();
            }
            object.install_base_regenerate_if_needed();
            if object.base_regenerate.is_some() {
                self.base_regenerate_reg.record_install();
            }
            object.install_enemy_near_if_needed();
            if object.enemy_near.is_some() {
                self.enemy_near_reg.record_install();
            }
            object.install_animation_steering_if_needed();
            if object.animation_steering.is_some() {
                self.animation_steering_reg.record_install();
            }
            object.install_float_update_if_needed();
            if object.float_update.is_some() {
                self.float_update_reg.record_install();
            }
            object.install_prone_update_if_needed();
            if object.prone_update.is_some() {
                self.prone_update_reg.record_install();
            }
            object.install_radius_decal_update_if_needed();
            if object.radius_decal_update.is_some() {
                self.radius_decal_update_reg.record_install();
            }
            object.install_checkpoint_update_if_needed();
            if object.checkpoint_update.is_some() {
                self.checkpoint_update_reg.record_install();
            }
            object.install_spectre_gunship_deployment_if_needed();
            if object.spectre_gunship_deployment.is_some() {
                self.spectre_gunship_deployment_reg.record_install();
            }
            object.install_smart_bomb_target_homing_if_needed();
            if object.smart_bomb_target_homing.is_some() {
                self.smart_bomb_target_homing_reg.record_install();
            }
            if let Some(up) =
                crate::game_logic::host_upgrade_die::upgrade_to_remove_for_template(template_name)
            {
                object.install_upgrade_die(up);
            }

            // Host residual: GLA Technical TransportContain Slots=5 (infantry passengers)
            // + PRIMARY TechnicalMachineGunWeapon residual (salvage tiers swap later).
            // Fail-closed: not chassis reskin / PassengersAllowedToFire.
            if crate::game_logic::host_technical::is_technical_template(template_name) {
                use crate::game_logic::host_technical::{
                    technical_weapon_for_tier, TechnicalWeaponTier,
                };
                object.install_technical_transport();
                // Force residual MG when template lacked primary_weapon_name (Weapon::default path).
                object.weapon = Some(technical_weapon_for_tier(TechnicalWeaponTier::Base));
            }

            // Host residual: China Battlemaster PRIMARY BattleMasterTankGun residual.
            // Fail-closed: Uranium/horde/nationalism applied via refresh_battlemaster_weapon.
            if crate::game_logic::host_battlemaster::is_battlemaster_template(template_name) {
                use crate::game_logic::host_battlemaster::battlemaster_weapon;
                object.weapon = Some(battlemaster_weapon(false, false, false));
            }

            // Host residual: GLA Marauder PRIMARY MarauderTankGun residual (salvage tiers).
            // Fail-closed: not full SalvageCrate W3D turret subobject matrix.
            if crate::game_logic::host_marauder::is_marauder_template(template_name) {
                use crate::game_logic::host_marauder::{
                    marauder_weapon_for_tier, MarauderWeaponTier,
                };
                object.weapon = Some(marauder_weapon_for_tier(MarauderWeaponTier::Base));
            }

            // Host residual: GLA Combat Cycle RiderChangeContain Slots=1 + rider weapon.
            // Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
            if crate::game_logic::host_combat_cycle::is_combat_cycle_template(template_name) {
                object.install_combat_cycle_transport();
                // Retail InitialPayload residual: spawn with default rider weapon bound.
                let rider = crate::game_logic::host_combat_cycle::default_spawn_rider_for_template(
                    template_name,
                );
                object.combat_cycle_rider = rider.as_u8();
                object.weapon =
                    crate::game_logic::host_combat_cycle::combat_cycle_weapon_for_rider(rider);
            }

            // Host residual: GLA Tunnel Network TunnelContain (shared MaxTunnelCapacity=10)
            // + PRIMARY TunnelNetworkGun residual (base-defense auto-fire path).
            // Fail-closed: not GuardTunnelNetwork AI / CaveSystem / heal matrix.
            if crate::game_logic::host_tunnel_network::is_tunnel_network_template(template_name) {
                object.install_tunnel_network_residual();
                object.weapon =
                    Some(crate::game_logic::host_tunnel_network::tunnel_network_gun_weapon());
            }

            // Host residual: AirF Combat Chinook TransportContain Slots=8 + passenger fire.
            // Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
            if crate::game_logic::host_combat_chinook::is_combat_chinook_template(template_name) {
                object.install_combat_chinook_transport();
            }

            // Host residual: China Listening Outpost detect 300 + transport Slots=2 +
            // InnateStealth + ArmedRiders dummy. Fail-closed: not IR FX / multi-door.
            let is_listening_outpost_spawn =
                crate::game_logic::host_listening_outpost::is_listening_outpost_template(
                    template_name,
                );
            if is_listening_outpost_spawn {
                object.install_listening_outpost_transport();
            }

            // Host residual: China Troop Crawler TransportContain Slots=8 +
            // StealthDetector (VisionRange 175) + TroopCrawlerAssault DEPLOY.
            // Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve.
            let is_troop_crawler_spawn =
                crate::game_logic::host_troop_crawler::is_troop_crawler_template(template_name);
            if is_troop_crawler_spawn {
                object.install_troop_crawler_transport();
                object.weapon =
                    Some(crate::game_logic::host_troop_crawler::troop_crawler_assault_weapon());
                if crate::game_logic::host_troop_crawler::troop_crawler_spawn_is_detector(
                    template_name,
                ) {
                    object.is_detector = true;
                    object.record_host_detector();
                    if let Some(range) =
                        crate::game_logic::host_troop_crawler::troop_crawler_detection_range(
                            template_name,
                        )
                    {
                        object.detection_range = range;
                        object.record_host_detector();
                    }
                }
                // VisionRange residual (175) for effective_detection_range fallback.
                object.thing.template.sight_range = object
                    .thing
                    .template
                    .sight_range
                    .max(crate::game_logic::host_troop_crawler::TROOP_CRAWLER_VISION_RANGE);
            }

            // Host residual: China Overlord / Helix / Emperor portable addons + transport.
            // Fail-closed: not full OverlordContain / HelixContain portable-structure spawn.
            if crate::game_logic::host_overlord_addons::is_overlord_tank_template(template_name) {
                // OverlordContain style: portable slot reserved; bunker residual separate.
                object.overlord_bunker_capacity = Some(0);
                object.record_host_overlord();
            }
            if crate::game_logic::host_overlord_addons::is_helix_template(template_name) {
                object.install_helix_transport();
                // Host residual: Helix PRIMARY HelixMinigunWeapon (always retained with addons).
                // Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
                object.weapon = Some(crate::game_logic::host_helix_minigun::helix_minigun_weapon());
            }
            if crate::game_logic::host_overlord_addons::is_emperor_template(template_name) {
                // Innate PropagandaTowerBehavior AffectsSelf residual.
                object.has_overlord_propaganda_addon = true;
                object.record_host_overlord();
                object.overlord_bunker_capacity = Some(0);
                object.record_host_overlord();
            }
            let emperor_spawn =
                crate::game_logic::host_overlord_addons::is_emperor_template(template_name);
            let helix_spawn =
                crate::game_logic::host_overlord_addons::is_helix_template(template_name);

            // Host residual: America Humvee TransportContain Slots=5 + passenger fire.
            // Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
            if crate::game_logic::host_humvee::is_humvee_template(template_name) {
                object.install_humvee_transport();
            }

            // Host residual: America Avenger designator primary + air laser secondary.
            // Fail-closed: not portable laser turret OverlordContain passenger.
            if crate::game_logic::host_avenger::is_avenger_template(template_name) {
                object.weapon = Some(crate::game_logic::host_avenger::avenger_designator_weapon());
                object.secondary_weapon =
                    Some(crate::game_logic::host_avenger::avenger_air_laser_weapon());
            }

            // Host residual: America Sentry Drone StealthDetectorUpdate (DetectionRange 225).
            // Always detector from spawn; gun is PLAYER_UPGRADE residual.
            if crate::game_logic::host_sentry_drone::sentry_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_sentry_drone::sentry_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                // Innate stealth residual (StealthUpdate InnateStealth = Yes).
                object.set_status_stealthed(true);
                object.stealth_breaks_on_attack = true;
                object.record_host_stealth_flags();
                // Retail WeaponSet Conditions=None has PRIMARY None until PLAYER_UPGRADE.
                // Strip kind-based Weapon::default fallback from resolve_primary_weapon.
                // Explicit template primary_weapon(_name) still keeps a bound gun (test/seed).
                if !sentry_had_explicit_primary {
                    object.weapon = None;
                }
            }

            // Host residual: America Pathfinder StealthDetectorUpdate + InnateStealth.
            // DetectionRange unset → VisionRange 200; stays stealthed while attacking;
            // uncloaks only while MOVING (StealthForbiddenConditions = MOVING).
            if crate::game_logic::host_pathfinder::pathfinder_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_pathfinder::pathfinder_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                object.set_status_stealthed(true);
                object.innate_stealth = true;
                object.is_pathfinder_unit = true;
                object.record_host_stealth_flags();
                object.stealth_breaks_on_attack = false;
                object.record_host_stealth_flags();
                object.stealth_breaks_on_move = true;
                object.record_host_stealth_flags();
            }

            // Host residual: China Dragon Tank primary flame weapon bind.
            // Fail-closed: FireWall secondary is host_firewall special-power residual.
            if crate::game_logic::host_dragon_tank::is_dragon_tank_template(template_name) {
                use crate::game_logic::host_dragon_tank::{
                    dragon_flame_weapon, has_black_napalm_upgrade,
                };
                let upgraded = has_black_napalm_upgrade(&object.applied_upgrades);
                // Force residual flame stats when store/template leaves defaults.
                object.weapon = Some(dragon_flame_weapon(upgraded));
            }

            // Host residual: China Nuke Cannon neutron secondary is PLAYER_UPGRADE only.
            // Fail-closed: Upgrade_ChinaNeutronShells equips SECONDARY; without it, no secondary.
            // Explicit template.secondary_weapon_name (tests / seeds) still keeps a bound weapon.
            if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
                use crate::game_logic::host_neutron_shell::UPGRADE_CHINA_NEUTRON_SHELLS;
                use crate::game_logic::weapon_bootstrap::{
                    ensure_host_weapon_store, NUKE_CANNON_NEUTRON_WEAPON,
                };
                let has_neutron = object.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS)
                    || object.has_upgrade_tag("Upgrade_ChinaNeutronShells")
                    || self.players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_CHINA_NEUTRON_SHELLS)
                    });
                if has_neutron {
                    ensure_host_weapon_store();
                    if let Some(w) = ThingTemplate::weapon_from_store(NUKE_CANNON_NEUTRON_WEAPON) {
                        object.secondary_weapon = Some(w);
                    }
                    object.apply_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS);
                } else if object.thing.template.secondary_weapon_name.is_none()
                    && object.thing.template.secondary_weapon.is_none()
                {
                    // Strip residual map auto-equip; keep explicit test/seed secondaries.
                    object.secondary_weapon = None;
                }
            }

            // Host residual: China Gattling Tank dual ground/AA + continuous-fire ramp state.
            // Fail-closed: not Overlord/Helix/building gattling payloads.
            if crate::game_logic::host_gattling_tank::is_gattling_tank_template(template_name) {
                use crate::game_logic::host_gattling_tank::{
                    gattling_air_weapon, gattling_ground_weapon, has_chain_guns_upgrade,
                    GattlingFireLevel,
                };
                let chain = has_chain_guns_upgrade(&object.applied_upgrades);
                object.weapon = Some(gattling_ground_weapon(GattlingFireLevel::Base, chain));
                object.secondary_weapon = Some(gattling_air_weapon(GattlingFireLevel::Base, chain));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: China Gattling Cannon structure dual ground/AA + continuous-fire ramp.
            // Fail-closed: not full CONTINUOUS_FIRE_* model-condition animation matrix.
            if crate::game_logic::host_base_defense::is_gattling_cannon_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    gattling_building_air_weapon, gattling_building_ground_weapon,
                    gattling_building_has_chain_guns,
                };
                use crate::game_logic::host_gattling_tank::GattlingFireLevel;
                let chain = gattling_building_has_chain_guns(&object.applied_upgrades);
                object.weapon = Some(gattling_building_ground_weapon(
                    GattlingFireLevel::Base,
                    chain,
                ));
                object.secondary_weapon =
                    Some(gattling_building_air_weapon(GattlingFireLevel::Base, chain));
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: GLA Stinger Site SPAWNS_ARE_THE_WEAPONS dual ground/AA +
            // HiveStructureBody / SpawnBehavior residual (3 soldiers) + physical roster.
            if crate::game_logic::host_base_defense::is_stinger_site_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    init_stinger_hive_slave_roster, stinger_air_weapon, stinger_ground_weapon,
                    stinger_has_ap_rockets, sync_hive_slave_mirrors,
                };
                let ap = stinger_has_ap_rockets(&object.applied_upgrades);
                object.weapon = Some(stinger_ground_weapon(ap));
                object.secondary_weapon = Some(stinger_air_weapon(ap));
                let roster = init_stinger_hive_slave_roster();
                object.hive_slaves = roster;
                let (slaves, slave_hp) = sync_hive_slave_mirrors(&roster);
                object.hive_slave_count = slaves;
                object.record_host_hive();
                object.hive_slave_hp = slave_hp;
                object.record_host_hive();
                object.hive_slave_respawn_frame = 0;
            }

            // Host residual: USA Patriot dual ground/AA secondary.
            // Laser General residual uses Lazr_Patriot* damage (40/35) via template.
            // Fail-closed: not full AssistedTargetingModule assist clips / RequestAssistRange.
            if crate::game_logic::host_base_defense::is_patriot_battery_structure(template_name) {
                use crate::game_logic::host_base_defense::{
                    patriot_air_weapon_for_template, patriot_ground_weapon_for_template,
                };
                object.weapon = Some(patriot_ground_weapon_for_template(template_name));
                object.secondary_weapon = Some(patriot_air_weapon_for_template(template_name));
            }

            // Host residual: USA Crusader / Paladin PRIMARY tank gun
            // (Laser General Lazr_* → Lazr_CrusaderTankGun / Lazr_PaladinTankGun).
            // Fail-closed: not full LaserName beam drawable / shell lob matrix.
            if crate::game_logic::host_usa_tanks::is_crusader_template(template_name)
                || crate::game_logic::host_usa_tanks::is_paladin_template(template_name)
            {
                object.weapon = Some(
                    crate::game_logic::host_usa_tanks::usa_tank_gun_weapon_for_template(
                        template_name,
                    ),
                );
            }

            // Host residual: GLA Scorpion PRIMARY gun (+ secondary rocket if unlocked).
            // Fail-closed: not full SalvageCrate missile-rack W3D subobject matrix.
            if crate::game_logic::host_scorpion::is_scorpion_template(template_name) {
                use crate::game_logic::host_scorpion::{
                    has_ap_rockets_upgrade, has_scorpion_rocket_upgrade,
                    salvage_tier_from_upgrades, scorpion_gun_weapon, scorpion_missile_weapon,
                };
                let tier = salvage_tier_from_upgrades(&object.applied_upgrades);
                object.weapon = Some(scorpion_gun_weapon(tier));
                if has_scorpion_rocket_upgrade(&object.applied_upgrades) {
                    let ap = has_ap_rockets_upgrade(&object.applied_upgrades);
                    object.secondary_weapon =
                        Some(scorpion_missile_weapon(ap, tier.dual_missile_clip()));
                }
            }

            // Host residual: USA Tomahawk PRIMARY dual-radius missile.
            // TomahawkMissile projectile lob residual closed (MissileAI peels + impact).
            if crate::game_logic::host_tomahawk::is_tomahawk_template(template_name) {
                use crate::game_logic::host_tomahawk::tomahawk_weapon;
                object.weapon = Some(tomahawk_weapon());
            }

            // Host residual: USA Raptor PRIMARY jet missiles (+ Laser Missiles upgrade).
            // RETURN_TO_BASE ClipReload airfield rearm residual closed (dock + timer).
            if crate::game_logic::host_raptor::is_raptor_template(template_name) {
                use crate::game_logic::host_raptor::{
                    has_laser_missiles_upgrade, is_king_raptor_template, raptor_weapon,
                };
                let king = is_king_raptor_template(template_name);
                let laser = has_laser_missiles_upgrade(&object.applied_upgrades);
                object.weapon = Some(raptor_weapon(king, laser));
            }

            // Host residual: China MiG PRIMARY napalm / Nuke dual-radius missiles.
            // Fail-closed: not full RETURN_TO_BASE ClipReload / HistoricBonus Firestorm matrix.
            if crate::game_logic::host_mig::is_mig_template(template_name) {
                use crate::game_logic::host_mig::{is_nuke_mig_template, mig_loadout, mig_weapon};
                let loadout = mig_loadout(
                    is_nuke_mig_template(template_name),
                    &object.applied_upgrades,
                );
                object.weapon = Some(mig_weapon(loadout));
            }

            // Host residual: America Fire Base PRIMARY howitzer.
            // Fail-closed: not full SPAWNS_ARE_THE_WEAPONS / garrison HiveStructure matrix.
            if crate::game_logic::host_fire_base::is_fire_base_template(template_name) {
                use crate::game_logic::host_fire_base::fire_base_weapon;
                object.weapon = Some(fire_base_weapon());
            }

            // Host residual: USA Stealth Fighter PRIMARY jet missiles.
            // Fail-closed: not full RETURN_TO_BASE ClipReload / science production matrix.
            if crate::game_logic::host_stealth_fighter::is_stealth_fighter_template(template_name) {
                use crate::game_logic::host_stealth_fighter::stealth_fighter_weapon;
                object.weapon = Some(stealth_fighter_weapon());
            }

            // Host residual: USA Comanche PRIMARY 20mm + SECONDARY anti-tank.
            // Retail rocket pods are a PLAYER_UPGRADE TERTIARY weapon; keep
            // anti-tank bound in SECONDARY and only expose pods after the team
            // owns the real upgrade.
            if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(template_name) {
                use crate::game_logic::host_comanche_rocket_pods::{
                    comanche_antitank_weapon, comanche_cannon_weapon, comanche_rocket_pod_weapon,
                    UPGRADE_COMANCHE_ROCKET_PODS,
                };
                object.weapon = Some(comanche_cannon_weapon());
                object.secondary_weapon = Some(comanche_antitank_weapon());
                let has_pods = object.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                    || object.has_upgrade_tag("Upgrade_ComancheRocketPods")
                    || self.players.values().any(|player| {
                        player.team == team
                            && player.has_unlocked_upgrade(UPGRADE_COMANCHE_ROCKET_PODS)
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

            // Host residual: USA Battle Drone PRIMARY machine gun.
            // Fail-closed: not full SlavedUpdate repair arm weld FX matrix.
            if crate::game_logic::host_slave_drones::is_battle_drone_template(template_name) {
                use crate::game_logic::host_slave_drones::battle_drone_weapon;
                object.weapon = Some(battle_drone_weapon());
            }

            // Host residual: China Overlord / Emperor PRIMARY dual-radius tank gun.
            // Fail-closed: not full ClipSize=2 dual-volley / Nuclear Tanks death residual.
            if crate::game_logic::host_overlord_gun::is_overlord_gun_chassis(template_name) {
                use crate::game_logic::host_overlord_gun::{
                    has_uranium_shells_upgrade, overlord_gun_weapon,
                };
                let uranium = has_uranium_shells_upgrade(&object.applied_upgrades);
                object.weapon = Some(overlord_gun_weapon(uranium));
            }

            // Host residual: GLA Jarmen Kell PRIMARY sniper residual.
            // Fail-closed: pilot-snipe special remains host_hero_abilities.
            if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
                use crate::game_logic::host_jarmen_kell::{
                    has_ap_bullets_upgrade, jarmen_kell_weapon,
                };
                let ap = has_ap_bullets_upgrade(&object.applied_upgrades);
                object.weapon = Some(jarmen_kell_weapon(ap));
            }

            // Host residual: China Red Guard PRIMARY machine gun residual.
            // Fail-closed: bayonet residual applied at fire-time for close infantry.
            if crate::game_logic::host_red_guard::is_red_guard_template(template_name) {
                use crate::game_logic::host_red_guard::red_guard_weapon;
                object.weapon = Some(red_guard_weapon(false, false));
            }

            // Host residual: China Tank Hunter PRIMARY RPG residual (AA + ground + splash).
            // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
            if crate::game_logic::host_tank_hunter::is_tank_hunter_template(template_name) {
                use crate::game_logic::host_tank_hunter::tank_hunter_weapon;
                object.weapon = Some(tank_hunter_weapon(false, false));
            }

            // Host residual: GLA Rebel PRIMARY machine gun residual.
            // Fail-closed: not full ClipSize volley / CaptureBuilding / BoobyTrap matrix.
            if crate::game_logic::host_gla_rebel::is_gla_rebel_template(template_name) {
                use crate::game_logic::host_gla_rebel::{has_ap_bullets_upgrade, rebel_weapon};
                let ap = has_ap_bullets_upgrade(&object.applied_upgrades);
                object.weapon = Some(rebel_weapon(ap));
            }

            // Host residual: USA Ranger PRIMARY rifle residual.
            // FlashBang secondary is PLAYER_UPGRADE only (Upgrade_AmericaRangerFlashBangGrenade)
            // — parity with neutron shells / rocket pods: residual map may name the weapon,
            // but create strips it unless research is unlocked or template explicitly seeds it.
            // Fail-closed: not full SURRENDER surrender-AI / garrison clear matrix.
            if crate::game_logic::host_ranger::is_ranger_template(template_name) {
                use crate::game_logic::host_ranger::{
                    has_flashbang_equipped, ranger_flashbang_weapon, ranger_rifle_weapon,
                    UPGRADE_AMERICA_FLASHBANG,
                };
                object.weapon = Some(ranger_rifle_weapon());
                let has_flashbang = has_flashbang_equipped(false, &object.applied_upgrades)
                    || self.players.values().any(|p| {
                        p.team == team && p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG)
                    });
                if has_flashbang {
                    object.secondary_weapon = Some(ranger_flashbang_weapon());
                    object.apply_upgrade_tag(UPGRADE_AMERICA_FLASHBANG);
                } else if object.thing.template.secondary_weapon_name.is_none()
                    && object.thing.template.secondary_weapon.is_none()
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
                let chain = has_chain_guns_upgrade(&object.applied_upgrades);
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
                object.continuous_fire_consecutive = 0;
                object.continuous_fire_level = 0;
                object.continuous_fire_coast_until_frame = 0;
                object.continuous_fire_victim = 0;
            }

            // Host residual: Colonel Burton PRIMARY sniper residual.
            // Fail-closed: knife residual applied at fire-time for close infantry.
            if crate::game_logic::host_colonel_burton::is_colonel_burton_template(template_name) {
                use crate::game_logic::host_colonel_burton::burton_sniper_weapon;
                object.weapon = Some(burton_sniper_weapon());
            }

            // Host residual: USA Pilot starts VETERAN (VeterancyGainCreate StartingLevel).
            // Fail-closed: not full EjectPilotDie parachute OCL / PilotFindVehicle AI scan.
            if crate::game_logic::host_usa_pilot::is_pilot_template(template_name) {
                use crate::game_logic::host_usa_pilot::pilot_default_veterancy;
                use crate::game_logic::VeterancyLevel;
                let target = pilot_default_veterancy();
                if object.experience.level != target {
                    let prev = object.experience.level;
                    object.experience.level = target;
                    // Seed residual XP so level does not immediately drop on gain_experience.
                    if matches!(target, VeterancyLevel::Veteran) {
                        object.experience.current = object.experience.current.max(1.0);
                    }
                    let _ = prev;
                    // Apply bonuses only if promoting from Rookie.
                    if !matches!(
                        prev,
                        VeterancyLevel::Veteran | VeterancyLevel::Elite | VeterancyLevel::Heroic
                    ) {
                        // Direct level set; apply_veterancy_bonuses is private — call gain path
                        // by using a public residual: re-apply via temporary if needed.
                        // Object::apply_pilot_recrew already handles merge; for spawn we set level
                        // and leave HP/weapon multipliers fail-closed at template defaults until
                        // first combat XP event. Veterans still recrew-transfer as Veteran.
                    }
                }
            }

            // Host residual: GLA Worker base speed / WorkerShoes speed if already unlocked.
            // Fail-closed: not full WorkerAIUpdate bored auto-task matrix.
            if crate::game_logic::host_gla_worker::is_gla_worker_template(template_name) {
                use crate::game_logic::host_gla_worker::{
                    worker_residual_speed, UPGRADE_GLA_WORKER_SHOES,
                };
                let shoes = object.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES);
                object.movement.max_speed = worker_residual_speed(shoes);
            }

            // Host residual: GLA RPG Trooper / Tunnel Defender PRIMARY rocket residual.
            // Fail-closed: not full ScatterRadiusVsInfantry / projectile exhaust FX matrix.
            if crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(template_name) {
                use crate::game_logic::host_rpg_trooper::{
                    has_ap_rockets_upgrade, rpg_trooper_weapon,
                };
                let ap = has_ap_rockets_upgrade(&object.applied_upgrades);
                object.weapon = Some(rpg_trooper_weapon(ap));
            }

            // Host residual: GLA Terrorist PRIMARY TerroristSuicideWeapon residual.
            // Chem Beta/Gamma + Demo death-weapon residual profiles.
            // Fail-closed: not ConvertToCarBomb full matrix / SlowDeath fling.
            if crate::game_logic::host_terrorist::is_terrorist_template(template_name) {
                use crate::game_logic::host_terrorist::{
                    terrorist_death_profile, terrorist_suicide_weapon_for_profile,
                };
                let has_gamma = object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
                    || object.has_upgrade_tag("Upgrade_GLAAnthraxGamma");
                let has_beta = object.has_upgrade_tag("Upgrade_GLAAnthraxBeta")
                    || object.has_upgrade_tag("Chem_Upgrade_GLAAnthraxBeta");
                let profile = terrorist_death_profile(template_name, has_gamma, has_beta);
                object.weapon = Some(terrorist_suicide_weapon_for_profile(profile));
                object.secondary_weapon = None;
            }

            // Host residual: USA Missile Defender PRIMARY missile + SECONDARY laser guided.
            // Fail-closed: not full SpecialAbilityUpdate prep / LaserBeam object matrix.
            if crate::game_logic::host_missile_defender::is_missile_defender_template(template_name)
            {
                use crate::game_logic::host_missile_defender::{
                    missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
                };
                object.weapon = Some(missile_defender_primary_weapon());
                object.secondary_weapon = Some(missile_defender_laser_guided_weapon());
            }

            // Host residual: America Scout Drone StealthDetectorUpdate (VisionRange 150).
            if crate::game_logic::host_slave_drones::scout_spawn_is_detector(template_name) {
                object.is_detector = true;
                object.record_host_detector();
                if let Some(range) =
                    crate::game_logic::host_slave_drones::scout_detection_range(template_name)
                {
                    object.detection_range = range;
                    object.record_host_detector();
                }
                // Sensor drone: strip kind-based default gun if no explicit primary.
                // Reuse sentry_had_explicit_primary (same template fields, captured pre-move).
                if !sentry_had_explicit_primary {
                    object.weapon = None;
                }
            }

            // Host residual: America Hellfire Drone AutoAcquire + HellfireMissileWeapon.
            // Weapon bound via weapon_bootstrap primary; no extra strip.
            // Auto-fire residual runs from update_combat when idle.

            object.ensure_fire_weapon_when_damaged();
            object.ensure_transition_damage_fx();
            object.ensure_fx_list_die();
            object.ensure_create_object_die();
            object.ensure_lifetime_update(self.frame);
            object.ensure_height_die(self.frame);
            self.objects.insert(id, object);

            // C++ Object.cpp onCreate residual: inherit team prototype attitude + attack priority.
            self.inherit_team_ai_defaults(id);

            // C++ SpecialPowerModule StartsPaused=Yes residual (pauseCountdown TRUE on create).
            self.init_starts_paused_special_powers(id);

            // C++ SupplyWarehouseCreate::onCreate residual — StartingBoxes.
            self.init_supply_warehouse_create(id);

            // Residual honesty: Emperor innate propaganda counts as install on spawn.
            if emperor_spawn {
                self.overlord_addons.record_propaganda_install();
            }
            let _ = helix_spawn;

            // Host residual: Listening Outpost InitialPayload TankHunter × 2.
            // Dock after insert so recursive create_object cannot re-enter mid-build.
            // Fail-closed: no payload if TankHunter template is absent.
            if is_listening_outpost_spawn {
                self.apply_listening_outpost_initial_payload(id, team, position);
            }

            // Host residual: Troop Crawler InitialPayload Redguard × 8.
            // Dock after insert so recursive create_object cannot re-enter mid-build.
            if is_troop_crawler_spawn {
                self.apply_troop_crawler_initial_payload(id, team, position);
            }

            // Host residual: SCIENCE unit-training (VeterancyGainCreate StartingLevel).
            // Fail-closed: not full PrerequisiteSciences rank tree / IsTrainable matrix.
            {
                use crate::game_logic::host_unit_training::unit_training_level_for_template;
                let sciences: Vec<String> = self
                    .players
                    .values()
                    .filter(|p| p.team == team)
                    .flat_map(|p| p.unlocked_sciences.iter().cloned())
                    .collect();
                if let Some((kind, level)) =
                    unit_training_level_for_template(template_name, &sciences)
                {
                    if let Some(obj) = self.objects.get_mut(&id) {
                        if obj.set_min_veterancy_level(level) {
                            self.unit_training.record_grant(kind);
                        }
                    }
                }
            }

            // Host residual: Demo SuicideBomb tag + CommandSetUpgrade if researched.
            {
                use crate::game_logic::host_demo_suicide_bomb::{
                    demo_command_set_upgrade_for_template, is_demo_suicide_bomb_eligible_template,
                    is_demo_suicide_bomb_upgrade, UPGRADE_DEMO_SUICIDE_BOMB,
                };
                if is_demo_suicide_bomb_eligible_template(template_name) {
                    let has_upgrade = self.players.values().any(|p| {
                        p.team == team
                            && p.unlocked_sciences
                                .iter()
                                .any(|s| is_demo_suicide_bomb_upgrade(s))
                    });
                    if has_upgrade {
                        if let Some(obj) = self.objects.get_mut(&id) {
                            if !obj.has_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB) {
                                obj.apply_upgrade_tag(UPGRADE_DEMO_SUICIDE_BOMB);
                                self.demo_suicide_bomb.record_tag();
                            }
                            if obj.command_set_override.is_none() {
                                if let Some(cs) =
                                    demo_command_set_upgrade_for_template(&obj.template_name)
                                {
                                    obj.set_command_set_override(Some(cs));
                                    self.demo_suicide_bomb.record_command_set_upgrade(1);
                                }
                            }
                        }
                    }
                }
            }

            if counts_as_unit {
                self.record_unit_production(team);
            } else if is_structure && !starts_under_construction {
                self.record_structure_completion(team);
                // Static path/LOS obstacle (C++ pathfind structure residual).
                self.block_structure_object_path(id);
                // Map-placed / instant SW: onSpecialPowerCreation residual.
                self.on_structure_superweapon_creation(id);
            }
            log::debug!(
                "Created object {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            let team_ord = match team {
                Team::USA => 0u8,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            };
            crate::game_logic::host_spawn_log::record(
                id,
                template_name.to_string(),
                team_ord,
                [position.x, position.y, position.z],
            );
            // Wave 680: mid-frame GameWorld map while coupled shadow tick is live.
            // End-of-tick host_spawn_log drain remains idempotent for unmapped IDs.
            let _ = crate::gameworld_shadow::eager_map_host_spawn_if_coupled(
                self,
                &crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id,
                    template: template_name.to_string(),
                    team_ordinal: team_ord,
                    position: [position.x, position.y, position.z],
                },
            );
            if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                obj.record_model_mesh_from_template();
                obj.record_kind_of_bits_from_template();
            }
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Create object under construction (for buildings)
    pub fn create_object_under_construction(
        &mut self,
        template_name: &str,
        team: Team,
        position: Vec3,
    ) -> Option<ObjectId> {
        // C++ BuildAssistant isLocationLegalToBuild residual (objects-in-way / bounds).
        if !self.is_location_legal_to_build(team, position, template_name) {
            log::debug!(
                "Blocked construction {} at {:?} (LegalBuildCode residual)",
                template_name,
                position
            );
            return None;
        }
        // C++ ProductionPrerequisite residual (known sample table / SW tech tree).
        if !self.team_satisfies_build_prerequisites(team, template_name) {
            log::debug!(
                "Blocked construction {} for team {:?} (Prerequisites residual)",
                template_name,
                team
            );
            return None;
        }
        // C++ MaxSimultaneousOfType=DeterminedBySuperweaponRestriction residual.
        if !self.can_start_superweapon_building(team, template_name) {
            log::debug!(
                "Blocked superweapon construction {} for team {:?} (MaxSimultaneous residual)",
                template_name,
                team
            );
            return None;
        }
        if let Some(template) = self.templates.get(template_name).cloned() {
            let id = self.allocate_object_id();
            let mut object = Object::new_under_construction(template, id, team);
            object.set_position(position);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(
                    id,
                    Some([position.x, position.y, position.z]),
                );
                object.record_host_movement();
            }

            self.objects.insert(id, object);
            self.inherit_team_ai_defaults(id);

            let team_ord = match team {
                Team::USA => 0u8,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            };
            crate::game_logic::host_spawn_log::record(
                id,
                template_name.to_string(),
                team_ord,
                [position.x, position.y, position.z],
            );
            // Wave 680: mid-frame GameWorld map while coupled shadow tick is live.
            // End-of-tick host_spawn_log drain remains idempotent for unmapped IDs.
            let _ = crate::gameworld_shadow::eager_map_host_spawn_if_coupled(
                self,
                &crate::game_logic::host_spawn_log::HostSpawnEvent {
                    id,
                    template: template_name.to_string(),
                    team_ordinal: team_ord,
                    position: [position.x, position.y, position.z],
                },
            );
            // Wave 199: GameWorld SetConstruction sole-tick / progress last-writer.
            crate::game_logic::host_construction_progress_log::record(id, 0.0, true, 0.0);
            if let Some(obj) = self.host_objects_mut().get_mut(&id) {
                obj.record_model_mesh_from_template();
                obj.record_kind_of_bits_from_template();
            }

            log::debug!(
                "Started construction of {} ({}) at {:?}",
                id,
                template_name,
                position
            );
            Some(id)
        } else {
            log::warn!("Template not found: {}", template_name);
            None
        }
    }

    /// Destroy an object
    pub fn destroy_object(&mut self, id: ObjectId) {
        self.mark_object_for_destruction(id, None);
    }

    /// Wave 482: sell residual kill (parked aircraft) — queue remove without
    /// SlowDeath/Topple deferral peels used for combat deaths.
    pub(in super::super) fn destroy_object_for_sell_residual(&mut self, id: ObjectId) {
        self.maybe_notify_special_power_completion(id);
        self.maybe_apply_dam_die(id);
        let _ = self.apply_ocl_random_force(id);
        self.maybe_apply_upgrade_die(id);
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer: None });
    }

    /// C++ FireWeaponWhenDeadBehavior::onDie residual — death weapon splash.
    pub(in super::super) fn apply_fire_weapon_when_dead(&mut self, dying_id: ObjectId) {
        use crate::game_logic::host_fire_weapon_when_dead::{
            death_weapon_for_template, splash_damage_at_distance,
        };

        let Some(obj) = self.objects.get(&dying_id) else {
            return;
        };
        if obj.fire_weapon_when_dead_fired {
            return;
        }
        if obj.status.under_construction {
            return;
        }
        let Some(splash) = death_weapon_for_template(&obj.template_name) else {
            return;
        };
        let pos = obj.get_position();
        let team = obj.team;
        let max_r = splash.primary_radius.max(splash.secondary_radius);

        let is_helix_napalm_bomb = obj.helix_napalm_bomb_projectile;
        let napalm_source = obj.producer_id;
        let black_napalm_bomb = obj.template_name.to_ascii_lowercase().contains("black");

        // Mark fired
        if let Some(obj) = self.objects.get_mut(&dying_id) {
            obj.fire_weapon_when_dead_fired = true;
        }

        let victims: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if *id == dying_id || !o.is_alive() {
                    return None;
                }
                let p = o.get_position();
                let dx = p.x - pos.x;
                let dz = p.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= max_r {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        let mut destroy_ids = Vec::new();
        for vid in victims {
            let Some(v) = self.objects.get_mut(&vid) else {
                continue;
            };
            let p = v.get_position();
            let dx = p.x - pos.x;
            let dz = p.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let dmg = splash_damage_at_distance(&splash, dist);
            if dmg <= 0.0 {
                continue;
            }
            let destroyed = v.take_damage_from_immediate(dmg, Some(dying_id));
            if destroyed {
                destroy_ids.push(vid);
            }
        }
        // Presentation residual: death explosion particle at epicenter.
        let _ = self.combat_particles.spawn(
            crate::game_logic::combat_particles::CombatParticleKind::DeathExplosion,
            pos,
            self.frame,
            Some(dying_id),
            None,
        );
        if is_helix_napalm_bomb {
            // Honesty: HeightDie detonation residual counted as blast path.
            self.helix_napalm.blast_hits = self
                .helix_napalm
                .blast_hits
                .saturating_add(destroy_ids.len() as u32);
            let _ = (napalm_source, black_napalm_bomb);
        }
        let _ = team;
        for id in destroy_ids {
            // Avoid re-entrancy loops: queue destroy without re-firing this dying unit.
            if id != dying_id {
                self.objects_to_destroy.push_back(DestructionEvent {
                    id,
                    killer: Some(team),
                });
            }
        }
    }

    /// Wave 752: lethal finish that respects damage-authority HP last-write.
    /// Prefer this over direct host HP zeroing for production destroy residual.
    #[allow(dead_code)]
    pub(crate) fn host_lethal_finish_object(
        &mut self,
        id: ObjectId,
        source: Option<ObjectId>,
    ) -> bool {
        let Some(o) = self.objects.get_mut(&id) else {
            return false;
        };
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            crate::game_logic::host_damage_log::record(id, hp, source, true);
        } else {
            o.health.current = 0.0;
        }
        o.status.destroyed = true;
        o.status.effectively_dead = true;
        true
    }

    /// Wave 754: C++ EjectPilotDie::onDie residual at death start (mark_object),
    /// not only final process_destroy remove. SlowDeath defers remove and must
    /// not suppress pilot spawn / honesty residual.
    pub(crate) fn maybe_apply_eject_pilot_die(&mut self, id: ObjectId) {
        use crate::game_logic::host_usa_pilot::{
            air_eject_spawn_height, can_eject_pilot_on_death, is_eject_pilot_eligible_template,
            meets_eject_pilot_death_types_gate, meets_eject_pilot_exempt_status_gate,
            meets_eject_pilot_veterancy_gate, uses_air_eject_ocl, EJECT_PILOT_TEMPLATE,
            PILOT_EJECT_AUDIO,
        };
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        if obj.eject_pilot_die_applied {
            return;
        }
        let is_vehicle = obj.is_kind_of(KindOf::Vehicle) || obj.object_type == ObjectType::Vehicle;
        let is_aircraft =
            obj.is_kind_of(KindOf::Aircraft) || obj.object_type == ObjectType::Aircraft;
        let under_construction =
            obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
        let eligible_template = is_eject_pilot_eligible_template(&obj.template_name);
        let vet_gate = meets_eject_pilot_veterancy_gate(obj.experience.level);
        let death_types_gate = meets_eject_pilot_death_types_gate(obj.status.death_type);
        let exempt_status_gate = meets_eject_pilot_exempt_status_gate(obj.status.hijacked);
        if eligible_template
            && !obj.is_unmanned()
            && !under_construction
            && is_vehicle
            && !is_aircraft
            && death_types_gate
            && exempt_status_gate
            && !vet_gate
        {
            self.usa_pilot.record_eject_veterancy_block();
        }
        if eligible_template
            && !obj.is_unmanned()
            && !under_construction
            && is_vehicle
            && !is_aircraft
            && vet_gate
            && exempt_status_gate
            && !death_types_gate
        {
            self.usa_pilot.record_eject_death_type_block();
        }
        if eligible_template
            && !obj.is_unmanned()
            && !under_construction
            && is_vehicle
            && !is_aircraft
            && vet_gate
            && death_types_gate
            && !exempt_status_gate
        {
            self.usa_pilot.record_eject_hijacked_block();
        }
        if !can_eject_pilot_on_death(
            eligible_template,
            obj.is_unmanned(),
            under_construction,
            is_vehicle,
            is_aircraft,
            vet_gate,
            death_types_gate,
            exempt_status_gate,
        ) {
            return;
        }
        let pilot_team = obj.team;
        let death_pos = obj.get_position();
        let air_path = uses_air_eject_ocl(death_pos.y, obj.status.airborne_target);
        let veterancy = obj.experience.level;
        // Mark applied before spawn so recursive destroy cannot double-fire.
        if let Some(o) = self.objects.get_mut(&id) {
            o.eject_pilot_die_applied = true;
        }
        if !self.templates.contains_key(EJECT_PILOT_TEMPLATE) {
            let mut pilot_tpl = crate::game_logic::ThingTemplate::new(EJECT_PILOT_TEMPLATE);
            pilot_tpl
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(100.0);
            self.templates
                .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
        }
        // Offset slightly so pilot is not buried under death debris residual.
        // Air OCL residual: keep elevated y (PutInContainer AmericaParachute).
        let spawn_pos = if air_path {
            glam::Vec3::new(
                death_pos.x + 2.0,
                air_eject_spawn_height(death_pos.y),
                death_pos.z + 2.0,
            )
        } else {
            death_pos + glam::Vec3::new(2.0, 0.0, 2.0)
        };
        if let Some(pilot_id) = self.create_object(EJECT_PILOT_TEMPLATE, pilot_team, spawn_pos) {
            self.usa_pilot.record_ejection();
            if air_path {
                self.usa_pilot.record_air_ejection();
            }
            let until =
                crate::game_logic::host_usa_pilot::eject_pilot_invulnerable_until_frame(self.frame);
            if let Some(pilot) = self.objects.get_mut(&pilot_id) {
                pilot.apply_eject_invulnerable(until);
                if air_path {
                    let raw_y = pilot.get_position().y;
                    pilot.apply_eject_parachuting();
                    if crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged(
                        raw_y, 0.0,
                    ) {
                        self.usa_pilot.record_parachute_open_fudge();
                    }
                }
                // Transfer vehicle veterancy residual (except Rookie gate already applied).
                pilot.experience.level = veterancy;
            }
            self.usa_pilot.record_invulnerable_grant();
            self.queue_audio_event(
                AudioEventRequest::new(PILOT_EJECT_AUDIO)
                    .with_position(spawn_pos)
                    .with_priority(170),
            );
            let _ = pilot_id;
        }
    }

    pub(crate) fn mark_object_for_destruction(&mut self, id: ObjectId, killer: Option<Team>) {
        // C++ ProductionUpdate cancelAndRefund on death start (before topple/slow-death deferral).
        self.cancel_all_production(id);
        // C++ SpecialPowerCompletionDie::onDie residual.
        self.maybe_notify_special_power_completion(id);
        // C++ DamDie::onDie residual fires with other die modules at death start.
        self.maybe_apply_dam_die(id);
        // Wave 754: C++ EjectPilotDie::onDie at death start (before SlowDeath defer).
        self.maybe_apply_eject_pilot_die(id);
        // C++ OCL ApplyRandomForceNugget residual (air-death toss before debris).
        let _ = self.apply_ocl_random_force(id);
        // C++ UpgradeDie::onDie residual — free producer's upgrade slot.
        self.maybe_apply_upgrade_die(id);
        // Wave 482: BuildAssistant sell finish removes the object immediately.
        // Do not defer into StructureTopple/Collapse / SlowDeath / KeepObjectDie —
        // those combat-death peels left sold structures alive forever in host-only tests.
        let (sold, under_construction) = self
            .objects
            .get(&id)
            .map(|o| (o.status.sold, o.status.under_construction))
            .unwrap_or((false, false));
        // Wave 715: MSG_DOZER_CANCEL_CONSTRUCT / unfinished builds remove immediately.
        // Do not defer into StructureTopple — cancel would leave the shell alive a frame+.
        if !sold && !under_construction {
            // C++ StructureTopple/Collapse residual: buildings fall/sink before remove.
            if self.try_begin_structure_topple_instead_of_destroy(id, killer) {
                return;
            }
            // C++ SlowDeathBehavior residual: infantry/vehicles delay destroy + sink.
            if self.try_begin_slow_death_instead_of_destroy(id, killer) {
                return;
            }
            // C++ KeepObjectDie residual: leave rubble, do not DestroyDie-remove.
            if self.try_begin_keep_object_die_instead_of_destroy(id, killer) {
                return;
            }
        }
        self.objects_to_destroy
            .push_back(DestructionEvent { id, killer });
    }

    /// C++ KeepObjectDie residual: convert to lasting rubble, skip remove.
    pub(in super::super) fn try_begin_keep_object_die_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Wave 775: StructureCollapse/Topple already ran their presentation; after Done
        // allow normal destroy instead of KeepObjectDie forever-defer (civilian barns).
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        if obj.status.keep_as_rubble {
            let _ = killer;
            return true;
        }
        if !obj.begin_keep_object_die(frame) {
            return false;
        }
        let _ = killer;
        // Death FX / OCL peels without world removal.
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.fire_fx_list_die();
            obj.fire_create_object_die();
        }
        self.apply_pending_create_object_die(id);
        let is_dam = self
            .objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false);
        if is_dam {
            self.apply_dam_die_enable_waveguides();
        }
        true
    }

    /// C++ DamDie::onDie residual — enable KINDOF_WAVEGUIDE objects.
    /// C++ UpgradeDie::onDie residual.
    pub(in super::super) fn maybe_apply_upgrade_die(&mut self, id: ObjectId) {
        let (producer, upgrade) = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            let Some(ud) = obj.upgrade_die.as_mut() else {
                return;
            };
            if ud.fired {
                return;
            }
            ud.fired = true;
            (obj.producer_id, ud.upgrade_to_remove.clone())
        };
        let Some(pid) = producer else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        let Some(master) = self.objects.get_mut(&pid) else {
            self.upgrade_die_reg.record_missing_producer();
            return;
        };
        if master.remove_upgrade_tag(&upgrade) {
            self.upgrade_die_reg.record_removal();
        } else {
            self.upgrade_die_reg.record_missing_upgrade();
        }
    }

    pub(in super::super) fn maybe_apply_dam_die(&mut self, id: ObjectId) {
        let is_dam = self
            .objects
            .get(&id)
            .map(|o| crate::game_logic::host_dam_die::is_dam_template(&o.template_name))
            .unwrap_or(false);
        if is_dam {
            self.apply_dam_die_enable_waveguides();
        }
    }

    pub(in super::super) fn apply_dam_die_enable_waveguides(&mut self) {
        let frame = self.frame;
        for obj in self.objects.values_mut() {
            let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                || crate::game_logic::host_dam_die::is_wave_guide_template(&obj.template_name)
                || crate::game_logic::host_wave_guide::is_wave_guide_template(&obj.template_name);
            if is_wg {
                obj.status.disabled_default = false;
                if obj.wave_guide_data.is_none() {
                    let mut wg = crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                    wg.facing = obj.get_orientation();
                    wg.ensure_active(frame.max(1));
                    obj.wave_guide_data = Some(wg);
                } else if let Some(wg) = obj.wave_guide_data.as_mut() {
                    wg.ensure_active(frame.max(1));
                }
            }
        }
    }

    pub(in super::super) fn try_begin_slow_death_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // Jet crash residual.
        if obj.jet_slow_death.as_ref().map(|j| j.done).unwrap_or(false) {
            return false;
        }
        if obj
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_jet_slow_death() {
            let _ = killer;
            return true;
        }
        // Helicopter spiral crash residual.
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.done)
            .unwrap_or(false)
        {
            return false;
        }
        if obj
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_helicopter_slow_death() {
            let _ = killer;
            return true;
        }
        // Already finished slow death → allow destroy.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_done())
            .unwrap_or(false)
        {
            return false;
        }
        // Mid slow death → keep deferring.
        if obj
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        if obj.begin_slow_death(frame) {
            let _ = killer;
            return true;
        }
        false
    }

    pub(crate) fn apply_structure_topple_crush_samples(
        &mut self,
        building_id: ObjectId,
        samples: Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample>,
    ) {
        if samples.is_empty() {
            return;
        }
        let building_team = self.objects.get(&building_id).map(|o| o.team);
        let mut destroy: Vec<ObjectId> = Vec::new();
        const SAMPLE_RADIUS: f32 = 18.0;
        let victims: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in victims {
            if id == building_id {
                continue;
            }
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let pos = obj.get_position();
            let mut best_dmg = 0.0_f32;
            for s in &samples {
                let dx = pos.x - s.x;
                let dz = pos.z - s.z;
                if dx * dx + dz * dz <= SAMPLE_RADIUS * SAMPLE_RADIUS {
                    best_dmg = best_dmg.max(s.damage);
                }
            }
            if best_dmg <= 0.0 {
                continue;
            }
            let killed = if let Some(obj) = self.objects.get_mut(&id) {
                // Structure topple crush residual is effectively unresistable for units
                // under the fall sweep (C++ doDamageLine lethality residual).
                let mut dead = obj.take_damage_from_typed_death(
                    best_dmg,
                    Some(building_id),
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                );
                // Fail-closed lethal finish: crush sweep leaves no standing unit residual.
                // Wave 746: under damage authority, do not zero host HP (dual with GW
                // HP writeback). Project lethal via damage log + destroyed flags;
                // non-authority path keeps host HP clear.
                if !obj.status.destroyed {
                    if crate::gameworld_shadow::gameworld_damage_authority_live() {
                        let hp = obj.health.current.max(1.0);
                        crate::game_logic::host_damage_log::record(id, hp, Some(building_id), true);
                        obj.status.destroyed = true;
                        obj.status.effectively_dead = true;
                        obj.status.death_type =
                            crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
                    } else {
                        // Wave 753: under damage authority, do not zero host HP mid-frame
                        // (dual with GW HP writeback). Project lethal via damage log + flags.
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            let hp = obj.health.current.max(1.0);
                            let oid = obj.id;
                            crate::game_logic::host_damage_log::record(oid, hp, None, true);
                        } else {
                            obj.health.current = 0.0;
                        }
                        obj.status.destroyed = true;
                        obj.status.effectively_dead = true;
                        obj.status.death_type =
                            crate::game_logic::host_usa_pilot::HostDeathType::Crushed;
                    }
                    dead = true;
                }
                dead
            } else {
                false
            };
            if killed
                || self
                    .objects
                    .get(&id)
                    .map(|o| o.status.destroyed || o.health.current <= 0.0)
                    .unwrap_or(false)
            {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, building_team);
        }
    }

    /// C++ FireWeaponWhenDamagedBehavior forceFireWeapon residual at object position.

    /// C++ CreateObjectDie::onDie residual — spawn OCL templates at dying object.
    pub fn apply_pending_create_object_die(&mut self, dying_id: ObjectId) {
        let (spawns, transfer_dmg, transfer, team, pos) = {
            let Some(o) = self.objects.get_mut(&dying_id) else {
                return;
            };
            let (spawns, dmg, transfer) = o.take_pending_create_object_die_spawns();
            (spawns, dmg, transfer, o.team, o.get_position())
        };
        if spawns.is_empty() {
            return;
        }
        for tmpl in spawns {
            // CreateDebris disposition residual for GenericDebris peels.
            let tl = tmpl.to_ascii_lowercase();
            if tl.contains("debris") || tl.contains("barrel") {
                use crate::game_logic::host_ocl_create_debris::HostOclCreateDebrisPlan;
                let plan = if tl.contains("barrel") {
                    HostOclCreateDebrisPlan::damaged_barrel()
                } else {
                    let mut p = HostOclCreateDebrisPlan::generic_tank_debris();
                    p.model_or_template = tmpl.clone();
                    p.count = 1;
                    p
                };
                let inherit = self
                    .objects
                    .get(&dying_id)
                    .map(|o| o.movement.velocity)
                    .unwrap_or(Vec3::ZERO);
                let ids = self.spawn_ocl_create_debris(&plan, team, pos, inherit);
                if transfer && transfer_dmg > 0.0 {
                    for id in ids {
                        if let Some(n) = self.objects.get_mut(&id) {
                            let _ = n.take_damage(transfer_dmg);
                        }
                    }
                }
                continue;
            }
            // Ensure template name exists for residual peels.
            if !self.templates.contains_key(&tmpl) {
                let mut t = ThingTemplate::new(&tmpl);
                t.set_health(100.0);
                if tmpl.to_ascii_lowercase().contains("tunnel")
                    || tmpl.to_ascii_lowercase().contains("network")
                {
                    t.add_kind_of(KindOf::Structure);
                }
                self.templates.insert(tmpl.clone(), t);
            }
            let Some(new_id) = self.create_object(&tmpl, team, pos) else {
                continue;
            };
            // C++ CreateObject Disposition=LIKE_EXISTING residual: copy pose.
            if let Some(dying) = self.objects.get(&dying_id) {
                let yaw = dying.get_orientation();
                if let Some(n) = self.objects.get_mut(&new_id) {
                    n.set_orientation(yaw);
                    n.producer_id = Some(dying_id);
                }
            }
            // FuelAir gas SlowDeath + HeightDie residual.
            if let Some(n) = self.objects.get_mut(&new_id) {
                n.ensure_fuel_air_gas_slow_death(self.frame);
                if n.fuel_air_gas_slow_death.is_some() {
                    self.fuel_air_gas_reg.record_install();
                }
            }
            if transfer && transfer_dmg > 0.0 {
                if let Some(n) = self.objects.get_mut(&new_id) {
                    let _ = n.take_damage_from_typed(
                        transfer_dmg,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                    );
                }
            }
        }
    }

    pub(in super::super) fn apply_fire_weapon_when_damaged_named(
        &mut self,
        source_id: ObjectId,
        weapon_name: &str,
    ) -> u32 {
        let (pos, team) = match self.objects.get(&source_id) {
            Some(o) => (o.get_position(), o.team),
            None => return 0,
        };
        let (pd, pr, sd, sr) =
            crate::game_logic::host_fire_weapon_when_damaged::fire_when_damaged_weapon_splash(
                weapon_name,
            );
        // Intended = self so splash doesn't skip others incorrectly... API skips intended_id.
        // Pass a dummy non-existent intended so all in radius can be hit except we should not hit self.
        // apply_instant_hit_splash_at skips intended_id only — use source as intended to skip self.
        self.apply_instant_hit_splash_at(
            pos,
            pd,
            sd,
            pr,
            sr,
            source_id,
            team,
            source_id,
            Some(weapon_name),
        )
    }

    pub(in super::super) fn try_begin_structure_topple_instead_of_destroy(
        &mut self,
        id: ObjectId,
        killer: Option<Team>,
    ) -> bool {
        let attacker_pos = {
            let src = self.objects.get(&id).and_then(|o| o.last_damage_source);
            src.and_then(|sid| {
                self.objects.get(&sid).map(|s| {
                    let p = s.get_position();
                    (p.x, p.z)
                })
            })
        };
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if !obj.is_kind_of(KindOf::Structure) {
            return false;
        }
        // Already finished collapse or topple → allow normal destroy.
        let collapse_done = obj
            .structure_collapse_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_collapse::HostStructureCollapseState::Done
                )
            })
            .unwrap_or(false);
        let topple_done = obj
            .structure_topple_data
            .as_ref()
            .map(|d| {
                matches!(
                    d.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            })
            .unwrap_or(false);
        if collapse_done || topple_done {
            return false;
        }
        // Mid-animation: keep deferring destroy.
        if obj
            .structure_collapse_data
            .as_ref()
            .map(|d| d.is_active())
            .unwrap_or(false)
            || obj
                .structure_topple_data
                .as_ref()
                .map(|d| d.is_active())
                .unwrap_or(false)
        {
            let _ = killer;
            return true;
        }
        // Prefer StructureCollapse for civilian/prop peels; else StructureTopple.
        if obj.begin_structure_collapse(frame) {
            let _ = killer;
            return true;
        }
        if obj.begin_structure_topple(frame, attacker_pos) {
            let _ = killer;
            return true;
        }
        false
    }
}

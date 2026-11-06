//! Host combat `impl GameLogic` — `base_defense_lasers`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn technical_residual_weapon_upgrades(&self) -> u32 {
        self.technical_residual_weapon_upgrades
    }

    pub fn technical_residual_loads(&self) -> u32 {
        self.technical_residual_loads
    }

    pub fn technical_residual_unloads(&self) -> u32 {
        self.technical_residual_unloads
    }

    /// Residual honesty: Toxin Tractor stream / spray / death field path.
    pub fn honesty_toxin_tractor_ok(&self) -> bool {
        self.toxin_tractor.honesty_host_path_ok()
    }

    pub fn honesty_toxin_tractor_stream_ok(&self) -> bool {
        self.toxin_tractor.honesty_stream_ok()
    }

    pub fn honesty_toxin_tractor_spray_ok(&self) -> bool {
        self.toxin_tractor.honesty_spray_ok()
    }

    pub fn honesty_toxin_tractor_death_field_ok(&self) -> bool {
        self.toxin_tractor.honesty_death_field_ok()
    }

    pub fn toxin_tractor_registry(
        &self,
    ) -> &crate::game_logic::host_toxin_tractor::HostToxinTractorRegistry {
        &self.toxin_tractor
    }

    /// Residual honesty: Marauder fire-rate salvage residual fired or upgraded.
    pub fn honesty_marauder_ok(&self) -> bool {
        self.marauder_residual_fires > 0
            || self.marauder_residual_weapon_upgrades > 0
            || self.marauder_shells_spawned > 0
            || self.marauder_scatter_applied > 0
    }

    /// Residual honesty: Marauder ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_marauder_scatter_ok(&self) -> bool {
        self.marauder_scatter_applied > 0 || self.marauder_scatter_misses > 0
    }

    pub fn honesty_marauder_weapon_upgrade_ok(&self) -> bool {
        self.marauder_residual_weapon_upgrades > 0
    }

    pub fn marauder_residual_fires(&self) -> u32 {
        self.marauder_residual_fires
    }

    pub fn marauder_residual_units_hit(&self) -> u32 {
        self.marauder_residual_units_hit
    }

    pub fn marauder_residual_weapon_upgrades(&self) -> u32 {
        self.marauder_residual_weapon_upgrades
    }

    pub fn honesty_scorpion_ok(&self) -> bool {
        self.scorpion_residual_fires > 0
            || self.scorpion_residual_rocket_upgrades > 0
            || self.scorpion_residual_salvage_upgrades > 0
            || self.scorpion_scatter_applied > 0
            || self.scorpion_shells_spawned > 0
    }

    /// Residual honesty: Scorpion ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_scorpion_scatter_ok(&self) -> bool {
        self.scorpion_scatter_applied > 0 || self.scorpion_scatter_misses > 0
    }

    pub fn honesty_scorpion_rocket_ok(&self) -> bool {
        self.scorpion_residual_rocket_upgrades > 0
    }

    pub fn honesty_scorpion_missile_ok(&self) -> bool {
        self.scorpion_residual_missile_fires > 0
    }

    pub fn scorpion_residual_fires(&self) -> u32 {
        self.scorpion_residual_fires
    }

    pub fn scorpion_residual_units_hit(&self) -> u32 {
        self.scorpion_residual_units_hit
    }

    pub fn honesty_tomahawk_ok(&self) -> bool {
        self.tomahawk_residual_fires > 0
            || self.tomahawk_missiles_spawned > 0
            || self.tomahawk_scatter_applied > 0
    }

    /// Residual honesty: Tomahawk ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_tomahawk_scatter_ok(&self) -> bool {
        self.tomahawk_scatter_applied > 0 || self.tomahawk_scatter_misses > 0
    }

    pub fn tomahawk_residual_fires(&self) -> u32 {
        self.tomahawk_residual_fires
    }

    pub fn tomahawk_residual_units_hit(&self) -> u32 {
        self.tomahawk_residual_units_hit
    }

    /// Residual honesty: USA Raptor jet missile residual fired or Laser Missiles applied.
    pub fn honesty_raptor_ok(&self) -> bool {
        self.raptor_residual_fires > 0
            || self.raptor_residual_laser_missiles_upgrades > 0
            || self.raptor_scatter_applied > 0
            || self.raptor_missiles_spawned > 0
    }

    /// Residual honesty: Raptor ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_raptor_scatter_ok(&self) -> bool {
        self.raptor_scatter_applied > 0 || self.raptor_scatter_misses > 0
    }

    pub fn honesty_raptor_laser_missiles_ok(&self) -> bool {
        self.raptor_residual_laser_missiles_upgrades > 0
    }

    pub fn raptor_residual_fires(&self) -> u32 {
        self.raptor_residual_fires
    }

    pub fn raptor_residual_units_hit(&self) -> u32 {
        self.raptor_residual_units_hit
    }

    pub fn raptor_residual_laser_missiles_upgrades(&self) -> u32 {
        self.raptor_residual_laser_missiles_upgrades
    }

    /// Residual honesty: China MiG napalm / Nuke residual fired or upgraded.
    pub fn honesty_mig_ok(&self) -> bool {
        self.mig_residual_fires > 0
            || self.mig_residual_black_napalm_upgrades > 0
            || self.mig_residual_tactical_nuke_upgrades > 0
            || self.mig_scatter_applied > 0
            || self.mig_scatter_misses > 0
    }

    /// Residual honesty: MiG ScatterRadiusVsInfantry peels applied.
    pub fn honesty_mig_scatter_ok(&self) -> bool {
        self.mig_scatter_applied > 0 || self.mig_scatter_misses > 0
    }

    pub fn honesty_mig_black_napalm_ok(&self) -> bool {
        self.mig_residual_black_napalm_upgrades > 0 || self.mig_residual_fire_fields > 0
    }

    pub fn honesty_mig_tactical_nuke_ok(&self) -> bool {
        self.mig_residual_tactical_nuke_upgrades > 0 || self.mig_residual_radiation_fields > 0
    }

    pub fn mig_residual_fires(&self) -> u32 {
        self.mig_residual_fires
    }

    pub fn mig_residual_units_hit(&self) -> u32 {
        self.mig_residual_units_hit
    }

    pub fn mig_residual_black_napalm_upgrades(&self) -> u32 {
        self.mig_residual_black_napalm_upgrades
    }

    pub fn mig_residual_tactical_nuke_upgrades(&self) -> u32 {
        self.mig_residual_tactical_nuke_upgrades
    }

    pub fn mig_residual_fire_fields(&self) -> u32 {
        self.mig_residual_fire_fields
    }

    pub fn mig_residual_radiation_fields(&self) -> u32 {
        self.mig_residual_radiation_fields
    }

    /// Residual honesty: America Fire Base howitzer residual fired.
    pub fn honesty_fire_base_ok(&self) -> bool {
        self.fire_base_residual_fires > 0
            || self.fire_base_shells_spawned > 0
            || self.fire_base_scatter_applied > 0
    }

    pub fn fire_base_residual_fires(&self) -> u32 {
        self.fire_base_residual_fires
    }

    pub fn fire_base_residual_units_hit(&self) -> u32 {
        self.fire_base_residual_units_hit
    }

    /// Residual honesty: Stealth Fighter missile residual fired.
    pub fn honesty_stealth_fighter_ok(&self) -> bool {
        self.stealth_fighter_residual_fires > 0
            || self.stealth_jet_missiles_spawned > 0
            || self.stealth_jet_scatter_applied > 0
    }

    /// Residual honesty: Stealth Jet ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_stealth_jet_scatter_ok(&self) -> bool {
        self.stealth_jet_scatter_applied > 0 || self.stealth_jet_scatter_misses > 0
    }

    pub fn stealth_fighter_residual_fires(&self) -> u32 {
        self.stealth_fighter_residual_fires
    }

    pub fn stealth_fighter_residual_units_hit(&self) -> u32 {
        self.stealth_fighter_residual_units_hit
    }

    /// Residual honesty: Comanche 20mm / anti-tank residual fired.
    pub fn honesty_comanche_ok(&self) -> bool {
        self.comanche_cannon_residual_fires > 0
            || self.comanche_antitank_residual_fires > 0
            || self.comanche_rocket_pod_residual_area_attacks > 0
    }

    pub fn honesty_comanche_cannon_ok(&self) -> bool {
        self.comanche_cannon_residual_fires > 0
    }

    pub fn honesty_comanche_antitank_ok(&self) -> bool {
        self.comanche_antitank_residual_fires > 0
            || self.comanche_at_scatter_applied > 0
            || self.comanche_at_scatter_misses > 0
    }

    /// Residual honesty: Comanche AT ScatterRadiusVsInfantry peels applied.
    pub fn honesty_comanche_at_scatter_ok(&self) -> bool {
        self.comanche_at_scatter_applied > 0 || self.comanche_at_scatter_misses > 0
    }

    /// Residual honesty: Helix PRIMARY minigun residual fired.
    pub fn honesty_helix_minigun_ok(&self) -> bool {
        self.helix_minigun_residual_fires > 0
    }

    pub fn helix_minigun_residual_fires(&self) -> u32 {
        self.helix_minigun_residual_fires
    }

    pub fn helix_minigun_residual_units_hit(&self) -> u32 {
        self.helix_minigun_residual_units_hit
    }

    /// Residual honesty: Inferno BlackNapalm upgraded fire field residual.
    pub fn honesty_inferno_black_napalm_ok(&self) -> bool {
        self.inferno_black_napalm_residual_upgrades > 0
            || self.inferno_black_napalm_residual_zones > 0
    }

    pub fn inferno_black_napalm_residual_upgrades(&self) -> u32 {
        self.inferno_black_napalm_residual_upgrades
    }

    pub fn inferno_black_napalm_residual_zones(&self) -> u32 {
        self.inferno_black_napalm_residual_zones
    }

    pub fn comanche_cannon_residual_fires(&self) -> u32 {
        self.comanche_cannon_residual_fires
    }

    pub fn comanche_cannon_residual_units_hit(&self) -> u32 {
        self.comanche_cannon_residual_units_hit
    }

    pub fn comanche_antitank_residual_fires(&self) -> u32 {
        self.comanche_antitank_residual_fires
    }

    pub fn comanche_antitank_residual_units_hit(&self) -> u32 {
        self.comanche_antitank_residual_units_hit
    }

    /// Residual honesty: Battle Drone attach / fire / repair residual path.
    pub fn honesty_battle_drone_ok(&self) -> bool {
        self.battle_drone_residual_attaches > 0
            || self.battle_drone_residual_fires > 0
            || self.battle_drone_residual_repairs > 0
    }

    pub fn honesty_battle_drone_attach_ok(&self) -> bool {
        self.battle_drone_residual_attaches > 0
    }

    pub fn honesty_battle_drone_fire_ok(&self) -> bool {
        self.battle_drone_residual_fires > 0
    }

    pub fn honesty_battle_drone_repair_ok(&self) -> bool {
        self.battle_drone_residual_repairs > 0
    }

    pub fn battle_drone_residual_attaches(&self) -> u32 {
        self.battle_drone_residual_attaches
    }

    pub fn battle_drone_residual_fires(&self) -> u32 {
        self.battle_drone_residual_fires
    }

    pub fn battle_drone_residual_units_hit(&self) -> u32 {
        self.battle_drone_residual_units_hit
    }

    pub fn battle_drone_residual_repairs(&self) -> u32 {
        self.battle_drone_residual_repairs
    }

    pub fn battle_drone_residual_repair_amount(&self) -> f32 {
        self.battle_drone_residual_repair_amount
    }

    /// Residual honesty: Overlord / Emperor main gun dual-radius / Uranium residual.
    pub fn honesty_overlord_gun_ok(&self) -> bool {
        self.overlord_gun_residual_fires > 0
            || self.overlord_gun_residual_uranium_upgrades > 0
            || self.overlord_scatter_applied > 0
            || self.overlord_shells_spawned > 0
    }

    /// Residual honesty: Overlord ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_overlord_scatter_ok(&self) -> bool {
        self.overlord_scatter_applied > 0 || self.overlord_scatter_misses > 0
    }

    pub fn honesty_overlord_gun_uranium_ok(&self) -> bool {
        self.overlord_gun_residual_uranium_upgrades > 0
    }

    pub fn overlord_gun_residual_fires(&self) -> u32 {
        self.overlord_gun_residual_fires
    }

    pub fn overlord_gun_residual_units_hit(&self) -> u32 {
        self.overlord_gun_residual_units_hit
    }

    /// Residual honesty: Jarmen Kell sniper / AP Bullets residual.
    pub fn honesty_jarmen_kell_ok(&self) -> bool {
        self.jarmen_kell_residual_fires > 0 || self.jarmen_kell_residual_ap_upgrades > 0
    }

    pub fn honesty_jarmen_kell_ap_ok(&self) -> bool {
        self.jarmen_kell_residual_ap_upgrades > 0
    }

    pub fn jarmen_kell_residual_fires(&self) -> u32 {
        self.jarmen_kell_residual_fires
    }

    pub fn jarmen_kell_residual_units_hit(&self) -> u32 {
        self.jarmen_kell_residual_units_hit
    }

    /// Residual honesty: Battlemaster tank gun / Uranium / horde / nationalism residual.
    pub fn honesty_battlemaster_ok(&self) -> bool {
        self.battlemaster_residual_fires > 0
            || self.battlemaster_residual_uranium_upgrades > 0
            || self.battlemaster_residual_horde_grants > 0
            || self.battlemaster_scatter_applied > 0
            || self.battlemaster_shells_spawned > 0
    }

    /// Residual honesty: Battlemaster ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_battlemaster_scatter_ok(&self) -> bool {
        self.battlemaster_scatter_applied > 0 || self.battlemaster_scatter_misses > 0
    }

    pub fn honesty_battlemaster_uranium_ok(&self) -> bool {
        self.battlemaster_residual_uranium_upgrades > 0
    }

    pub fn honesty_battlemaster_horde_ok(&self) -> bool {
        self.battlemaster_residual_horde_grants > 0
    }

    pub fn battlemaster_residual_fires(&self) -> u32 {
        self.battlemaster_residual_fires
    }

    pub fn battlemaster_residual_units_hit(&self) -> u32 {
        self.battlemaster_residual_units_hit
    }

    pub fn battlemaster_residual_uranium_upgrades(&self) -> u32 {
        self.battlemaster_residual_uranium_upgrades
    }

    pub fn battlemaster_residual_horde_grants(&self) -> u32 {
        self.battlemaster_residual_horde_grants
    }

    /// Residual honesty: Red Guard gun / bayonet / horde / nationalism residual.
    pub fn honesty_red_guard_ok(&self) -> bool {
        self.red_guard_residual_fires > 0
            || self.red_guard_residual_bayonet_kills > 0
            || self.red_guard_residual_horde_grants > 0
    }

    pub fn honesty_red_guard_horde_ok(&self) -> bool {
        self.red_guard_residual_horde_grants > 0
    }

    pub fn honesty_red_guard_bayonet_ok(&self) -> bool {
        self.red_guard_residual_bayonet_kills > 0
    }

    pub fn red_guard_residual_fires(&self) -> u32 {
        self.red_guard_residual_fires
    }

    pub fn red_guard_residual_bayonet_kills(&self) -> u32 {
        self.red_guard_residual_bayonet_kills
    }

    /// Residual honesty: Tank Hunter RPG / TNT / horde / nationalism residual.
    pub fn honesty_tank_hunter_ok(&self) -> bool {
        self.tank_hunter_residual_fires > 0
            || self.tank_hunter_residual_tnt_plants > 0
            || self.tank_hunter_residual_horde_grants > 0
            || self.tank_hunter_scatter_applied > 0
            || self.tank_hunter_missiles_spawned > 0
    }

    /// Residual honesty: Tank Hunter ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_tank_hunter_scatter_ok(&self) -> bool {
        self.tank_hunter_scatter_applied > 0 || self.tank_hunter_scatter_misses > 0
    }

    pub fn honesty_tank_hunter_tnt_ok(&self) -> bool {
        self.tank_hunter_residual_tnt_plants > 0
    }

    pub fn honesty_tank_hunter_horde_ok(&self) -> bool {
        self.tank_hunter_residual_horde_grants > 0
    }

    pub fn tank_hunter_residual_fires(&self) -> u32 {
        self.tank_hunter_residual_fires
    }

    pub fn tank_hunter_residual_units_hit(&self) -> u32 {
        self.tank_hunter_residual_units_hit
    }

    pub fn tank_hunter_residual_tnt_plants(&self) -> u32 {
        self.tank_hunter_residual_tnt_plants
    }

    /// Residual honesty: GLA Rebel gun / AP Bullets residual.
    pub fn honesty_rebel_ok(&self) -> bool {
        self.rebel_residual_fires > 0 || self.rebel_residual_ap_upgrades > 0
    }

    /// Residual honesty: USA Ranger rifle and/or FlashBang residual fire observed.
    pub fn honesty_ranger_ok(&self) -> bool {
        self.ranger_residual_rifle_fires > 0
            || self.ranger_residual_flashbang_fires > 0
            || self.flashbang_scatter_applied > 0
            || self.flashbang_grenades_spawned > 0
    }

    /// Residual honesty: Ranger FlashBang dual-radius residual fired.
    pub fn honesty_ranger_flashbang_ok(&self) -> bool {
        self.ranger_residual_flashbang_fires > 0 || self.flashbang_scatter_applied > 0
    }

    /// Residual honesty: Flashbang ScatterRadius applied at least once.
    pub fn honesty_flashbang_scatter_ok(&self) -> bool {
        self.flashbang_scatter_applied > 0 || self.flashbang_scatter_misses > 0
    }

    pub fn ranger_residual_rifle_fires(&self) -> u32 {
        self.ranger_residual_rifle_fires
    }

    pub fn ranger_residual_flashbang_fires(&self) -> u32 {
        self.ranger_residual_flashbang_fires
    }

    pub fn ranger_residual_units_hit(&self) -> u32 {
        self.ranger_residual_units_hit
    }

    /// Residual honesty: China Hacker DisableBuilding residual observed.
    pub fn honesty_hacker_disable_building_ok(&self) -> bool {
        self.hacker_disable_building_count > 0
    }

    pub fn hacker_disable_building_count(&self) -> u32 {
        self.hacker_disable_building_count
    }

    pub fn honesty_rebel_ap_ok(&self) -> bool {
        self.rebel_residual_ap_upgrades > 0
    }

    pub fn rebel_residual_fires(&self) -> u32 {
        self.rebel_residual_fires
    }

    pub fn rebel_residual_ap_upgrades(&self) -> u32 {
        self.rebel_residual_ap_upgrades
    }

    /// Residual honesty: China MiniGunner ground/AA / continuous fire / chain guns / horde.
    pub fn honesty_minigunner_ok(&self) -> bool {
        self.minigunner_residual_ground_fires > 0
            || self.minigunner_residual_aa_fires > 0
            || self.minigunner_residual_ramp_mean > 0
            || self.minigunner_residual_ramp_fast > 0
            || self.minigunner_residual_chain_gun_upgrades > 0
            || self.minigunner_residual_horde_grants > 0
    }

    pub fn honesty_minigunner_ramp_ok(&self) -> bool {
        self.minigunner_residual_ramp_mean > 0 || self.minigunner_residual_ramp_fast > 0
    }

    pub fn honesty_minigunner_aa_ok(&self) -> bool {
        self.minigunner_residual_aa_fires > 0
    }

    pub fn honesty_minigunner_horde_ok(&self) -> bool {
        self.minigunner_residual_horde_grants > 0
    }

    pub fn minigunner_residual_ground_fires(&self) -> u32 {
        self.minigunner_residual_ground_fires
    }

    pub fn minigunner_residual_aa_fires(&self) -> u32 {
        self.minigunner_residual_aa_fires
    }

    pub fn minigunner_residual_ramp_fast(&self) -> u32 {
        self.minigunner_residual_ramp_fast
    }

    /// Residual honesty: Colonel Burton sniper / knife residual.
    pub fn honesty_burton_ok(&self) -> bool {
        self.burton_residual_sniper_fires > 0 || self.burton_residual_knife_kills > 0
    }

    pub fn honesty_burton_knife_ok(&self) -> bool {
        self.burton_residual_knife_kills > 0
    }

    pub fn burton_residual_sniper_fires(&self) -> u32 {
        self.burton_residual_sniper_fires
    }

    pub fn burton_residual_knife_kills(&self) -> u32 {
        self.burton_residual_knife_kills
    }

    /// Residual honesty: GLA RPG Trooper rocket / AP Rockets residual.
    pub fn honesty_rpg_trooper_ok(&self) -> bool {
        self.rpg_trooper_residual_fires > 0
            || self.rpg_trooper_residual_ap_upgrades > 0
            || self.rpg_trooper_scatter_applied > 0
            || self.rpg_trooper_missiles_spawned > 0
    }

    /// Residual honesty: RPG Trooper ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_rpg_trooper_scatter_ok(&self) -> bool {
        self.rpg_trooper_scatter_applied > 0 || self.rpg_trooper_scatter_misses > 0
    }

    pub fn honesty_rpg_trooper_ap_ok(&self) -> bool {
        self.rpg_trooper_residual_ap_upgrades > 0
    }

    pub fn rpg_trooper_residual_fires(&self) -> u32 {
        self.rpg_trooper_residual_fires
    }

    pub fn rpg_trooper_residual_units_hit(&self) -> u32 {
        self.rpg_trooper_residual_units_hit
    }

    pub fn rpg_trooper_residual_ap_upgrades(&self) -> u32 {
        self.rpg_trooper_residual_ap_upgrades
    }

    /// Residual honesty: GLA Terrorist SuicideDynamitePack detonation residual.
    pub fn honesty_terrorist_ok(&self) -> bool {
        self.terrorist_residual_detonations > 0 && self.terrorist_residual_damage_dealt > 0.0
    }

    pub fn terrorist_residual_detonations(&self) -> u32 {
        self.terrorist_residual_detonations
    }

    pub fn terrorist_residual_units_hit(&self) -> u32 {
        self.terrorist_residual_units_hit
    }

    pub fn terrorist_residual_damage_dealt(&self) -> f32 {
        self.terrorist_residual_damage_dealt
    }

    /// Residual honesty: USA Missile Defender missile / laser guided residual.
    pub fn honesty_missile_defender_ok(&self) -> bool {
        self.missile_defender_residual_fires > 0
            || self.missile_defender_residual_laser_specials > 0
            || self.missile_defender_missiles_spawned > 0
            || self.missile_defender_scatter_applied > 0
    }

    /// Residual honesty: Missile Defender ScatterRadiusVsInfantry applied.
    pub fn honesty_missile_defender_scatter_ok(&self) -> bool {
        self.missile_defender_scatter_applied > 0 || self.missile_defender_scatter_misses > 0
    }

    pub fn honesty_missile_defender_laser_ok(&self) -> bool {
        self.missile_defender_residual_laser_specials > 0
            || self.missile_defender_laser_beams_spawned > 0
            || self.missile_defender_residual_laser_fires > 0
    }

    pub fn missile_defender_residual_fires(&self) -> u32 {
        self.missile_defender_residual_fires
    }

    pub fn missile_defender_residual_units_hit(&self) -> u32 {
        self.missile_defender_residual_units_hit
    }

    pub fn missile_defender_residual_laser_specials(&self) -> u32 {
        self.missile_defender_residual_laser_specials
    }

    pub fn missile_defender_residual_laser_fires(&self) -> u32 {
        self.missile_defender_residual_laser_fires
    }

    /// Residual honesty: Combat Cycle rider weapon residual path.
    pub fn honesty_combat_cycle_ok(&self) -> bool {
        self.combat_cycle_residual_fires > 0
            || self.combat_cycle_residual_rider_switches > 0
            || self.combat_cycle_residual_loads > 0
            || self.combat_cycle_residual_suicides > 0
    }

    pub fn honesty_combat_cycle_rider_switch_ok(&self) -> bool {
        self.combat_cycle_residual_rider_switches > 0
    }

    pub fn honesty_combat_cycle_fire_ok(&self) -> bool {
        self.combat_cycle_residual_fires > 0
    }

    pub fn combat_cycle_residual_fires(&self) -> u32 {
        self.combat_cycle_residual_fires
    }

    pub fn combat_cycle_residual_units_hit(&self) -> u32 {
        self.combat_cycle_residual_units_hit
    }

    pub fn combat_cycle_residual_rider_switches(&self) -> u32 {
        self.combat_cycle_residual_rider_switches
    }

    pub fn combat_cycle_residual_loads(&self) -> u32 {
        self.combat_cycle_residual_loads
    }

    pub fn combat_cycle_residual_suicides(&self) -> u32 {
        self.combat_cycle_residual_suicides
    }

    /// Residual honesty: Dragon Tank flame residual fired or BlackNapalm applied.
    pub fn honesty_dragon_tank_ok(&self) -> bool {
        self.dragon_tank_residual_fires > 0 || self.dragon_tank_residual_black_napalm_upgrades > 0
    }

    pub fn honesty_dragon_tank_black_napalm_ok(&self) -> bool {
        self.dragon_tank_residual_black_napalm_upgrades > 0
    }

    pub fn dragon_tank_residual_fires(&self) -> u32 {
        self.dragon_tank_residual_fires
    }

    pub fn dragon_tank_residual_units_hit(&self) -> u32 {
        self.dragon_tank_residual_units_hit
    }

    /// Residual honesty: Gattling Tank fired, ramped, or chain-gun upgraded.
    pub fn honesty_gattling_tank_ok(&self) -> bool {
        self.gattling_tank_residual_ground_fires > 0
            || self.gattling_tank_residual_aa_fires > 0
            || self.gattling_tank_residual_ramp_mean > 0
            || self.gattling_tank_residual_ramp_fast > 0
            || self.gattling_tank_residual_chain_gun_upgrades > 0
    }

    pub fn honesty_gattling_tank_ramp_ok(&self) -> bool {
        self.gattling_tank_residual_ramp_mean > 0 || self.gattling_tank_residual_ramp_fast > 0
    }

    pub fn honesty_gattling_tank_aa_ok(&self) -> bool {
        self.gattling_tank_residual_aa_fires > 0
    }

    pub fn gattling_tank_residual_ground_fires(&self) -> u32 {
        self.gattling_tank_residual_ground_fires
    }

    pub fn gattling_tank_residual_aa_fires(&self) -> u32 {
        self.gattling_tank_residual_aa_fires
    }

    pub fn gattling_tank_residual_ramp_fast(&self) -> u32 {
        self.gattling_tank_residual_ramp_fast
    }

    /// Residual honesty: China Gattling Cannon structure path exercised.
    pub fn honesty_gattling_building_ok(&self) -> bool {
        self.gattling_building_residual_ground_fires > 0
            || self.gattling_building_residual_aa_fires > 0
            || self.gattling_building_residual_ramp_mean > 0
            || self.gattling_building_residual_ramp_fast > 0
            || self.gattling_building_residual_chain_gun_upgrades > 0
    }

    /// Residual honesty: structure continuous-fire ramp reached MEAN or FAST.
    pub fn honesty_gattling_building_ramp_ok(&self) -> bool {
        self.gattling_building_residual_ramp_mean > 0
            || self.gattling_building_residual_ramp_fast > 0
    }

    /// Residual honesty: structure AA secondary residual fire.
    pub fn honesty_gattling_building_aa_ok(&self) -> bool {
        self.gattling_building_residual_aa_fires > 0
    }

    pub fn gattling_building_residual_ground_fires(&self) -> u32 {
        self.gattling_building_residual_ground_fires
    }

    pub fn gattling_building_residual_aa_fires(&self) -> u32 {
        self.gattling_building_residual_aa_fires
    }

    pub fn gattling_building_residual_ramp_fast(&self) -> u32 {
        self.gattling_building_residual_ramp_fast
    }

    /// Residual honesty: GLA Stinger Site dual ground/AA residual exercised.
    pub fn honesty_stinger_site_ok(&self) -> bool {
        self.stinger_site_residual_ground_fires > 0
            || self.stinger_site_residual_aa_fires > 0
            || self.stinger_site_residual_ap_rockets_upgrades > 0
    }

    /// Residual honesty: Stinger AA secondary residual fire.
    pub fn honesty_stinger_site_aa_ok(&self) -> bool {
        self.stinger_site_residual_aa_fires > 0
    }

    pub fn stinger_site_residual_ground_fires(&self) -> u32 {
        self.stinger_site_residual_ground_fires
    }

    pub fn stinger_site_residual_aa_fires(&self) -> u32 {
        self.stinger_site_residual_aa_fires
    }

    pub fn stinger_site_residual_ap_rockets_upgrades(&self) -> u32 {
        self.stinger_site_residual_ap_rockets_upgrades
    }

    /// Residual honesty: HiveStructureBody slave damage / kill / swallow path.
    pub fn honesty_stinger_hive_ok(&self) -> bool {
        self.stinger_hive_residual_slave_hits > 0
            || self.stinger_hive_residual_slave_kills > 0
            || self.stinger_hive_residual_swallows > 0
            || self.stinger_hive_residual_respawns > 0
            || self.stinger_hive_residual_closest_slave_hits > 0
            || self.stinger_slave_order_attack_count > 0
    }

    pub fn stinger_hive_residual_slave_hits(&self) -> u32 {
        self.stinger_hive_residual_slave_hits
    }

    pub fn stinger_hive_residual_slave_kills(&self) -> u32 {
        self.stinger_hive_residual_slave_kills
    }

    pub fn stinger_hive_residual_swallows(&self) -> u32 {
        self.stinger_hive_residual_swallows
    }

    pub fn stinger_hive_residual_respawns(&self) -> u32 {
        self.stinger_hive_residual_respawns
    }

    /// Residual honesty: getClosestSlave physical roster path used.
    pub fn honesty_stinger_closest_slave_ok(&self) -> bool {
        self.stinger_hive_residual_closest_slave_hits > 0
    }

    pub fn stinger_hive_residual_closest_slave_hits(&self) -> u32 {
        self.stinger_hive_residual_closest_slave_hits
    }

    /// Residual honesty: Stinger orderSlavesToAttackTarget residual.
    pub fn honesty_stinger_slave_order_ok(&self) -> bool {
        self.stinger_slave_order_attack_count > 0
    }

    pub fn stinger_slave_order_attack_count(&self) -> u32 {
        self.stinger_slave_order_attack_count
    }

    /// Residual honesty: Strategy Center TurretAI idle mood-target residual.
    pub fn honesty_strategy_center_turret_mood_target_ok(&self) -> bool {
        self.battle_plans.honesty_turret_mood_target_ok()
    }

    /// Residual honesty: StrategyCenterGun ScatterRadius peels applied.
    pub fn honesty_strategy_center_gun_scatter_ok(&self) -> bool {
        self.strategy_center_gun_scatter_applied > 0 || self.strategy_center_gun_scatter_misses > 0
    }

    /// Residual honesty: USA Patriot dual ground/AA residual exercised.
    pub fn honesty_patriot_ok(&self) -> bool {
        self.patriot_residual_ground_fires > 0
            || self.patriot_residual_aa_fires > 0
            || self.patriot_scatter_applied > 0
            || self.patriot_scatter_misses > 0
    }

    /// Residual honesty: Patriot ScatterRadiusVsInfantry peels applied.
    pub fn honesty_patriot_scatter_ok(&self) -> bool {
        self.patriot_scatter_applied > 0 || self.patriot_scatter_misses > 0
    }

    /// Residual honesty: Stinger ScatterRadiusVsInfantry peels applied.
    pub fn honesty_stinger_scatter_ok(&self) -> bool {
        self.stinger_scatter_applied > 0 || self.stinger_scatter_misses > 0
    }

    /// Residual honesty: Patriot AA secondary residual fire.
    pub fn honesty_patriot_aa_ok(&self) -> bool {
        self.patriot_residual_aa_fires > 0
    }

    pub fn patriot_residual_ground_fires(&self) -> u32 {
        self.patriot_residual_ground_fires
    }

    pub fn patriot_residual_aa_fires(&self) -> u32 {
        self.patriot_residual_aa_fires
    }

    /// Residual honesty: SupW EMP Patriot applied DISABLED_EMP at least once.
    pub fn honesty_supw_patriot_emp_ok(&self) -> bool {
        self.supw_patriot_emp_residual_grants > 0
            || self.supw_emp_scatter_applied > 0
            || self.supw_emp_scatter_misses > 0
    }

    /// Residual honesty: SupW EMPBlast ScatterRadiusVsInfantry peels applied.
    pub fn honesty_supw_emp_scatter_ok(&self) -> bool {
        self.supw_emp_scatter_applied > 0 || self.supw_emp_scatter_misses > 0
    }

    pub fn supw_patriot_emp_residual_grants(&self) -> u32 {
        self.supw_patriot_emp_residual_grants
    }

    pub fn supw_patriot_emp_spheroids_spawned(&self) -> u32 {
        self.supw_patriot_emp_spheroids_spawned
    }

    pub fn supw_patriot_emp_sparks_spawned(&self) -> u32 {
        self.supw_patriot_emp_sparks_spawned
    }

    /// Residual honesty: EMPPatriotEffectSpheroid + EMPSparks spawned.
    pub fn honesty_supw_patriot_emp_fx_ok(&self) -> bool {
        self.supw_patriot_emp_spheroids_spawned > 0
    }

    /// Residual honesty: AssistedTargetingUpdate request → accept → assist fire.
    pub fn honesty_patriot_assist_ok(&self) -> bool {
        self.patriot_assist_residual_requests > 0
            && self.patriot_assist_residual_accepts > 0
            && self.patriot_assist_residual_fires() > 0
    }

    /// Residual honesty: BinaryDataStream LaserFromAssisted + LaserToTarget spawned.
    pub fn honesty_patriot_assist_laser_ok(&self) -> bool {
        self.patriot_assist_laser_from_assisted > 0 && self.patriot_assist_laser_to_target > 0
    }

    pub fn patriot_assist_residual_requests(&self) -> u32 {
        self.patriot_assist_residual_requests
    }

    /// AssistingClipSize residual: real assist shots committed (fire commit,
    /// not command enqueue) plus legacy residual count.
    pub fn patriot_assist_residual_fires(&self) -> u32 {
        self.objects
            .registered_values()
            .map(|o| o.assist_clip_fire_commits)
            .fold(self.patriot_assist_residual_fires, u32::saturating_add)
    }

    pub fn patriot_assist_residual_accepts(&self) -> u32 {
        self.patriot_assist_residual_accepts
    }

    pub fn patriot_assist_laser_from_assisted(&self) -> u32 {
        self.patriot_assist_laser_from_assisted
    }

    pub fn patriot_assist_laser_to_target(&self) -> u32 {
        self.patriot_assist_laser_to_target
    }

    pub fn active_patriot_assist_lasers(
        &self,
    ) -> &[crate::game_logic::host_base_defense::ResidualPatriotAssistLaser] {
        &self.patriot_assist_lasers
    }

    /// Weapon.ini LaserName residual beams still live at the current frame.

    /// C++ ProjectileStreamUpdate presentation residual.
    pub fn projectile_stream_snapshot(
        &self,
    ) -> Vec<(
        crate::game_logic::ObjectId,
        String,
        Vec<glam::Vec3>,
        Option<crate::game_logic::ObjectId>,
    )> {
        self.projectile_streams
            .snapshot()
            .into_iter()
            .map(|(key, s)| (key.shooter, s.stream_name.clone(), s.points.clone(), s.target_id))
            .collect()
    }

    pub fn active_weapon_lasers(
        &self,
    ) -> &[crate::game_logic::host_weapon_laser::ResidualWeaponLaser] {
        &self.weapon_lasers
    }

    /// Presentation / test inject for LaserName residual beams.
    pub fn push_residual_weapon_laser_for_presentation(
        &mut self,
        laser: crate::game_logic::host_weapon_laser::ResidualWeaponLaser,
    ) {
        self.weapon_lasers.push(laser);
    }

    pub fn clear_residual_weapon_lasers_for_presentation(&mut self) {
        self.weapon_lasers.clear();
        self.weapon_laser_beams_spawned = 0;
    }

    /// Presentation / shell residual: inject host assist lasers for snapshot tests.
    ///
    /// Production combat still owns laser spawn via AssistedTargetingUpdate residual.
    /// This is a host-testable entry so PresentationFrame can freeze Line3D segments
    /// without requiring a full Patriot assist combat sequence.
    pub fn push_residual_patriot_assist_lasers_for_presentation(
        &mut self,
        lasers: impl IntoIterator<
            Item = crate::game_logic::host_base_defense::ResidualPatriotAssistLaser,
        >,
    ) {
        for laser in lasers {
            match laser.kind {
                crate::game_logic::host_base_defense::PatriotAssistLaserKind::FromAssisted => {
                    self.patriot_assist_laser_from_assisted =
                        self.patriot_assist_laser_from_assisted.saturating_add(1);
                }
                crate::game_logic::host_base_defense::PatriotAssistLaserKind::ToTarget => {
                    self.patriot_assist_laser_to_target =
                        self.patriot_assist_laser_to_target.saturating_add(1);
                }
            }
            self.patriot_assist_lasers.push(laser);
        }
    }

    /// Clear residual assist lasers (presentation dual-tick freeze test helper).
    pub fn clear_residual_patriot_assist_lasers_for_presentation(&mut self) {
        self.patriot_assist_lasers.clear();
    }

    /// Presentation residual inject: record host floating cash text for dual-tick freeze tests.
    ///
    /// Fail-closed: not full InGameUI GPU draw. Routes AutoDeposit residual into oil derrick
    /// registry (shared HostAutoDepositFloatingText type with black market).
    pub fn push_residual_auto_deposit_floating_text_for_presentation(
        &mut self,
        text: crate::game_logic::host_oil_derrick::HostAutoDepositFloatingText,
    ) {
        self.oil_derricks.record_floating_text(text);
    }

    /// Presentation residual inject: record MoneyPickUp Anim2D + money floating text.
    pub fn push_residual_money_pickup_presentation(
        &mut self,
        anim: crate::game_logic::host_money_crate::HostMoneyPickUpAnim,
        text: crate::game_logic::host_money_crate::HostMoneyFloatingText,
    ) {
        self.admit_money_pickup_animation(anim);
        self.host_money_crates.record_money_floating_text(text);
    }

    /// Clear residual floating text / world-anim host registries for dual-tick freeze tests.
    pub fn clear_residual_floating_text_for_presentation(&mut self) {
        self.oil_derricks.floating_texts.clear();
        self.oil_derricks.floating_texts_total = 0;
        self.black_markets.floating_texts.clear();
        self.black_markets.floating_texts_total = 0;
        self.hacker_income.floating_texts.clear();
        self.hacker_income.floating_texts_total = 0;
        self.cash_bounty.floating_texts.clear();
        self.cash_bounty.floating_texts_total = 0;
        self.host_money_crates.money_floating_texts.clear();
        self.host_money_crates.money_floating_texts_total = 0;
        self.session_view.world_animations.clear();
        self.host_money_crates
            .money_pickup_animation_history
            .clear();
        self.host_money_crates.money_pickup_animation_history_total = 0;
    }

    /// Residual honesty: StealthDetectorUpdate DetectionRate residual scan fired.
    pub fn honesty_stealth_detector_rate_ok(&self) -> bool {
        self.stealth_detector_rate_scans > 0
    }

    pub fn stealth_detector_rate_scans(&self) -> u32 {
        self.stealth_detector_rate_scans
    }

    /// DemoTrapUpdate weapon-slot mode residual (Proximity / Manual / Detonate).
    ///
    /// Returns true if mode applied. `DemoTrapMode::Detonate` also triggers
    /// manual detonation residual (PRIMARY detonation slot).
    pub fn set_demo_trap_mode(
        &mut self,
        trap_id: ObjectId,
        mode: crate::game_logic::host_mines::DemoTrapMode,
    ) -> bool {
        self.set_demo_trap_mode_with_effects(
            trap_id,
            mode,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn set_demo_trap_mode_with_effects(
        &mut self,
        trap_id: ObjectId,
        mode: crate::game_logic::host_mines::DemoTrapMode,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        use crate::game_logic::host_mines::HostMineKind;
        let is_detonate = mode.is_detonate_command();
        {
            let Some(obj) = self.objects.get_mut(&trap_id) else {
                return false;
            };
            let Some(md) = obj.mine_data.as_mut() else {
                return false;
            };
            if !matches!(md.kind, HostMineKind::DemoTrap) || md.detonated {
                return false;
            }
            if !md.set_demo_trap_mode(mode) {
                return false;
            }
        }
        if is_detonate {
            return self.manual_detonate_mine_with_effects(trap_id, presentation_effects);
        }
        true
    }

    /// Apply AP Rockets residual to a Stinger Site (PLAYER_UPGRADE damage residual × 1.25).
    pub fn apply_stinger_ap_rockets_upgrade(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_base_defense::{
            UPGRADE_GLA_AP_ROCKETS, is_stinger_site_structure, stinger_air_weapon,
            stinger_ground_weapon,
        };
        let Some(obj) = self.objects.get_mut(&object_id) else {
            return false;
        };
        if !is_stinger_site_structure(&obj.template_name) {
            return false;
        }
        obj.applied_upgrades
            .insert(UPGRADE_GLA_AP_ROCKETS.to_string());
        // AP Rockets is a C++ WeaponBonusUpgrade, not a new WeaponSet.  Keep
        // each live weapon's barrel cursor intact while refreshing its stats.
        obj.weapon = Some(stinger_ground_weapon(true));
        obj.secondary_weapon = Some(stinger_air_weapon(true));
        self.stinger_site_residual_ap_rockets_upgrades = self
            .stinger_site_residual_ap_rockets_upgrades
            .saturating_add(1);
        true
    }

    pub fn honesty_rocket_buggy_missile_projectile_ok(&self) -> bool {
        self.rocket_buggy_missiles_spawned > 0
    }

    pub fn apply_rocket_buggy_residual_at(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
    ) -> (u32, bool) {
        self.apply_rocket_buggy_residual_at_with_effects(
            impact,
            source,
            intended_target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_rocket_buggy_residual_at_with_effects(
        &mut self,
        impact: Vec3,
        source: Option<ObjectId>,
        intended_target: Option<ObjectId>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, bool) {
        use crate::game_logic::host_rocket_buggy::{
            BUGGY_DAMAGE_TYPE, BUGGY_DEATH_TYPE, BUGGY_FIRE_AUDIO, BUGGY_SECONDARY_RADIUS,
            is_legal_rocket_buggy_splash_target, rocket_buggy_damage_at, rocket_buggy_scatter_aim,
            rocket_buggy_scatter_misses_infantry,
        };

        // C++ BuggyRocketWeapon ScatterRadiusVsInfantry residual on instant apply (**20**).
        let mut impact = impact;
        let intended_is_infantry = intended_target
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let mut intended_scatter_miss = false;
        let mut scatter_misses = 0u32;
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
            let (new_impact, scattered) = rocket_buggy_scatter_aim(impact, true, seed);
            if scattered {
                self.rocket_buggy_scatter_applied =
                    self.rocket_buggy_scatter_applied.saturating_add(1);
                impact = new_impact;
            }
            if rocket_buggy_scatter_misses_infantry(true, seed, hit_r) {
                let intended_pos = intended_target
                    .and_then(|id| self.objects.get(&id))
                    .map(|o| o.get_position());
                if let Some(pos) = intended_pos {
                    let dx = impact.x - pos.x;
                    let dz = impact.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > BUGGY_SECONDARY_RADIUS {
                        self.rocket_buggy_residual_scatter_misses =
                            self.rocket_buggy_residual_scatter_misses.saturating_add(1);
                        scatter_misses = 1;
                        intended_scatter_miss = true;
                    }
                }
            }
        }

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
                if !is_legal_rocket_buggy_splash_target(
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
                // Scatter miss residual: intended infantry outside secondary is not force-hit.
                if is_intended && intended_scatter_miss {
                    return None;
                }
                // Primary radius 0: splash candidates within secondary; intended only if in ring.
                if dist <= BUGGY_SECONDARY_RADIUS {
                    Some((*id, dist, is_intended))
                } else {
                    None
                }
            })
            .collect();

        for (id, dist, is_intended) in candidates {
            let scatter = is_intended && intended_scatter_miss;
            let dmg = rocket_buggy_damage_at(is_intended, dist, scatter);
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
                        BUGGY_DAMAGE_TYPE,
                    ),
                    crate::game_logic::host_armor_residual::host_death_type_from_residual_name(
                        BUGGY_DEATH_TYPE,
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

        self.rocket_buggy_residual_fires = self.rocket_buggy_residual_fires.saturating_add(1);
        self.rocket_buggy_residual_units_hit =
            self.rocket_buggy_residual_units_hit.saturating_add(hits);
        let _ = scatter_misses; // counted at scatter peel above

        self.queue_audio_event(
            AudioEventRequest::new(BUGGY_FIRE_AUDIO)
                .with_position(impact)
                .with_priority(150),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact never spawns a second muzzle at the shooter.

        (hits, any_destroyed)
    }

    pub fn honesty_scud_missile_projectile_ok(&self) -> bool {
        self.scud_missiles_spawned > 0
    }
}

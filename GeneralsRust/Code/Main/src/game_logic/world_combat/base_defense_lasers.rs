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
            || self.battlemaster_residual_nationalism_upgrades > 0
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

    pub fn honesty_battlemaster_nationalism_ok(&self) -> bool {
        self.battlemaster_residual_nationalism_upgrades > 0
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
            || self.red_guard_residual_nationalism_upgrades > 0
            || self.red_guard_residual_horde_grants > 0
    }

    pub fn honesty_red_guard_horde_ok(&self) -> bool {
        self.red_guard_residual_horde_grants > 0
    }

    pub fn honesty_red_guard_nationalism_ok(&self) -> bool {
        self.red_guard_residual_nationalism_upgrades > 0
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
            || self.tank_hunter_residual_nationalism_upgrades > 0
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

    pub fn honesty_tank_hunter_nationalism_ok(&self) -> bool {
        self.tank_hunter_residual_nationalism_upgrades > 0
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
            || self.minigunner_residual_nationalism_upgrades > 0
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

    pub fn honesty_minigunner_nationalism_ok(&self) -> bool {
        self.minigunner_residual_nationalism_upgrades > 0
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

    /// Residual honesty: CamoNetting structure attack/damage reveal + re-cloak.
    pub fn honesty_camo_netting_structure_stealth_ok(&self) -> bool {
        self.camo_netting_structure_residual_reveals > 0
            || self.camo_netting_structure_residual_recloaks > 0
            || self.camo_netting_order_idle_enemies_count > 0
            || self.camo_netting_opacity_cloak_count > 0
            || self.camo_netting_opacity_reveal_count > 0
            || self.camo_netting_heat_vision_count > 0
            || self.camo_netting_sub_object_show_count > 0
    }

    /// Residual honesty: CamoNetting sub-object net mesh residual shown.
    pub fn honesty_camo_netting_sub_object_ok(&self) -> bool {
        self.camo_netting_sub_object_show_count > 0
    }

    pub fn camo_netting_sub_object_show_count(&self) -> u32 {
        self.camo_netting_sub_object_show_count
    }

    /// Residual honesty: Stinger orderSlavesToAttackTarget residual.
    pub fn honesty_stinger_slave_order_ok(&self) -> bool {
        self.stinger_slave_order_attack_count > 0
    }

    pub fn stinger_slave_order_attack_count(&self) -> u32 {
        self.stinger_slave_order_attack_count
    }

    /// Residual honesty: CamoNetting StealthLook heat-vision residual applied.
    pub fn honesty_camo_netting_heat_vision_ok(&self) -> bool {
        self.camo_netting_heat_vision_count > 0
    }

    pub fn camo_netting_heat_vision_count(&self) -> u32 {
        self.camo_netting_heat_vision_count
    }

    /// Residual honesty: Strategy Center TurretAI idle mood-target residual.
    pub fn honesty_strategy_center_turret_mood_target_ok(&self) -> bool {
        self.battle_plans.honesty_turret_mood_target_ok()
    }

    /// Residual honesty: StrategyCenterGun ScatterRadius peels applied.
    pub fn honesty_strategy_center_gun_scatter_ok(&self) -> bool {
        self.strategy_center_gun_scatter_applied > 0 || self.strategy_center_gun_scatter_misses > 0
    }

    pub fn camo_netting_structure_residual_reveals(&self) -> u32 {
        self.camo_netting_structure_residual_reveals
    }

    pub fn camo_netting_structure_residual_recloaks(&self) -> u32 {
        self.camo_netting_structure_residual_recloaks
    }

    /// Residual honesty: OrderIdleEnemiesToAttackMeUponReveal residual woke ≥1 unit.
    pub fn honesty_camo_netting_order_idle_enemies_ok(&self) -> bool {
        self.camo_netting_order_idle_enemies_count > 0
    }

    pub fn camo_netting_order_idle_enemies_count(&self) -> u32 {
        self.camo_netting_order_idle_enemies_count
    }

    /// Residual honesty: CamoNetting FriendlyOpacity residual applied.
    pub fn honesty_camo_netting_friendly_opacity_ok(&self) -> bool {
        self.camo_netting_opacity_cloak_count > 0 || self.camo_netting_opacity_reveal_count > 0
    }

    pub fn camo_netting_opacity_cloak_count(&self) -> u32 {
        self.camo_netting_opacity_cloak_count
    }

    pub fn camo_netting_opacity_reveal_count(&self) -> u32 {
        self.camo_netting_opacity_reveal_count
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
            && self.patriot_assist_residual_fires > 0
    }

    /// Residual honesty: BinaryDataStream LaserFromAssisted + LaserToTarget spawned.
    pub fn honesty_patriot_assist_laser_ok(&self) -> bool {
        self.patriot_assist_laser_from_assisted > 0 && self.patriot_assist_laser_to_target > 0
    }

    pub fn patriot_assist_residual_requests(&self) -> u32 {
        self.patriot_assist_residual_requests
    }

    pub fn patriot_assist_residual_fires(&self) -> u32 {
        self.patriot_assist_residual_fires
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
            .map(|(id, s)| (id, s.stream_name.clone(), s.points.clone(), s.target_id))
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
        self.host_money_crates.record_money_pickup_anim(anim);
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
        self.host_money_crates.money_pickup_anims.clear();
        self.host_money_crates.money_pickup_anims_total = 0;
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
            return self.manual_detonate_mine(trap_id);
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

    /// Apply Rocket Buggy residual (primary on intended + secondary splash ring).
    ///
    /// Returns (units_hit, any_destroyed).
    /// Fail-closed: not full projectile flight / AP mult / clip spacing.
    /// C++ RocketBuggyMissile ProjectileObject residual (MissileAI + impact splash).
    pub fn spawn_rocket_buggy_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_rocket_buggy::{
            BUGGY_MISSILE_FUEL_FRAMES, BUGGY_MISSILE_INITIAL_VELOCITY, BUGGY_MISSILE_MAX_HEALTH,
            BUGGY_MISSILE_PROJECTILE, BUGGY_PROJECTILE_SPEED,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(BUGGY_MISSILE_PROJECTILE) {
            let mut t = ThingTemplate::new(BUGGY_MISSILE_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(BUGGY_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(BUGGY_MISSILE_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on BuggyRocketWeapon vs infantry (**20**).
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_rocket_buggy::rocket_buggy_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.rocket_buggy_scatter_applied = self.rocket_buggy_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_rocket_buggy::rocket_buggy_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist > crate::game_logic::host_rocket_buggy::BUGGY_SECONDARY_RADIUS {
                        self.rocket_buggy_residual_scatter_misses =
                            self.rocket_buggy_residual_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + 8.0;
        let pid = self.create_object(BUGGY_MISSILE_PROJECTILE, team, start)?;
        let launch = BUGGY_MISSILE_INITIAL_VELOCITY / 30.0;
        let _cruise = BUGGY_PROJECTILE_SPEED / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        let vel = dir * launch;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.rocket_buggy_missile_projectile = true;
            o.rocket_buggy_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.rocket_buggy_missile_intended = intended.map(|id| id.0);
            o.rocket_buggy_missile_travelled = 0.0;
            o.rocket_buggy_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(BUGGY_MISSILE_FUEL_FRAMES));
            o.note_producer(source_id);
            o.health.maximum = BUGGY_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, BUGGY_MISSILE_MAX_HEALTH);
            o.movement.velocity = vel;
            o.set_orientation(dir.z.atan2(dir.x));
        }
        self.rocket_buggy_missiles_spawned = self.rocket_buggy_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_rocket_buggy_missile_projectiles(&mut self) {
        use crate::game_logic::host_rocket_buggy::{
            BUGGY_MISSILE_INITIAL_VELOCITY, BUGGY_MISSILE_TURN_DISTANCE, BUGGY_PROJECTILE_SPEED,
        };
        let frame = self.frame;
        let launch = BUGGY_MISSILE_INITIAL_VELOCITY / 30.0;
        let cruise = BUGGY_PROJECTILE_SPEED / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.rocket_buggy_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, Option<ObjectId>, Option<ObjectId>, glam::Vec3)> =
            Vec::new();
        for id in flying {
            let (source, intended, aim, pos, travelled, fuel_done) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .rocket_buggy_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let intended = o.rocket_buggy_missile_intended.map(ObjectId);
                let fuel_done = o
                    .rocket_buggy_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                (
                    o.producer_id,
                    intended,
                    aim,
                    o.get_position(),
                    o.rocket_buggy_missile_travelled,
                    fuel_done,
                )
            };
            // Prefer live intended target position (TryToFollowTarget Yes).
            let aim = intended
                .and_then(|tid| {
                    self.objects
                        .get(&tid)
                        .filter(|t| t.is_alive())
                        .map(|t| t.get_position())
                })
                .unwrap_or(aim);
            let speed = if travelled < BUGGY_MISSILE_TURN_DISTANCE {
                launch
            } else {
                cruise
            };
            let to_aim = aim - pos;
            let vel = if to_aim.length() > 0.001 {
                to_aim.normalize() * speed
            } else {
                glam::Vec3::new(0.0, -speed, 0.0)
            };
            let step = vel.length().max(speed);
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                o.set_position(pos + vel);
                o.rocket_buggy_missile_travelled += step;
                o.rocket_buggy_missile_aim = Some([aim.x, aim.y, aim.z]);
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let new_pos = pos + vel;
            let near = (aim - new_pos).length() < 8.0;
            if fuel_done || near {
                // Detonate at locked aim residual.
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
                o.rocket_buggy_missile_projectile = false;
                o.set_position(pos);
            }
            let _ = self.apply_rocket_buggy_residual_at(pos, source, intended);
            self.mark_object_for_destruction(id, team);
        }
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
                let destroyed = obj.take_damage_from_immediate_residual(
                    dmg,
                    source,
                    BUGGY_DAMAGE_TYPE,
                    BUGGY_DEATH_TYPE,
                );
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

        self.rocket_buggy_residual_fires = self.rocket_buggy_residual_fires.saturating_add(1);
        self.rocket_buggy_residual_units_hit =
            self.rocket_buggy_residual_units_hit.saturating_add(hits);
        let _ = scatter_misses; // counted at scatter peel above

        self.queue_audio_event(
            AudioEventRequest::new(BUGGY_FIRE_AUDIO)
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

    /// Apply SCUD launcher area residual at impact; toxin secondary also spawns poison.
    ///
    /// Returns (units_hit, any_destroyed).
    /// Fail-closed: not full SCUDMissile projectile / PreAttack animation matrix.
    /// C++ SCUDMissile ProjectileObject residual (lob + HeightDie/impact).
    pub fn spawn_scud_launcher_missile_projectile(
        &mut self,
        source_id: ObjectId,
        from: glam::Vec3,
        aim: glam::Vec3,
        intended: Option<ObjectId>,
        toxin_warhead: bool,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_height_die::HostHeightDieData;
        use crate::game_logic::host_scud_launcher::{
            SCUD_MISSILE_FUEL_FRAMES, SCUD_MISSILE_HEIGHT_DIE_TARGET,
            SCUD_MISSILE_INITIAL_VELOCITY, SCUD_MISSILE_LOFT_HEIGHT, SCUD_MISSILE_MAX_HEALTH,
            SCUD_PROJECTILE,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SCUD_PROJECTILE) {
            let mut t = ThingTemplate::new(SCUD_PROJECTILE);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SCUD_MISSILE_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(SCUD_PROJECTILE.to_string(), t);
        }
        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);

        // C++ ScatterRadiusVsInfantry residual on SCUDLauncherGun vs infantry (**30**).
        let target_is_infantry = intended
            .and_then(|id| self.objects.get(&id))
            .map(|o| o.is_kind_of(KindOf::Infantry))
            .unwrap_or(false);
        let seed = crate::game_logic::weapon_bootstrap::scatter_seed_for_shot(
            source_id.0,
            intended.map(|id| id.0).unwrap_or(0),
            self.frame,
        );
        let (aim, scattered) = crate::game_logic::host_scud_launcher::scud_launcher_scatter_aim(
            aim,
            target_is_infantry,
            seed,
        );
        if scattered {
            self.scud_launcher_scatter_applied =
                self.scud_launcher_scatter_applied.saturating_add(1);
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
            if crate::game_logic::host_scud_launcher::scud_launcher_scatter_misses_infantry(
                true, seed, hit_r,
            ) {
                if let Some(pos) = intended_pos {
                    let dx = aim.x - pos.x;
                    let dz = aim.z - pos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    let outer = if toxin_warhead {
                        crate::game_logic::host_scud_launcher::SCUD_TOX_SECONDARY_RADIUS
                    } else {
                        crate::game_logic::host_scud_launcher::SCUD_EXP_SECONDARY_RADIUS
                    };
                    if dist > outer {
                        self.scud_launcher_scatter_misses =
                            self.scud_launcher_scatter_misses.saturating_add(1);
                    }
                }
            }
        }

        let mut start = from;
        start.y = start.y.max(aim.y) + SCUD_MISSILE_LOFT_HEIGHT * 0.25;
        let pid = self.create_object(SCUD_PROJECTILE, team, start)?;
        let speed = SCUD_MISSILE_INITIAL_VELOCITY / 30.0;
        let to_aim = aim - start;
        let dist = to_aim.length().max(0.001);
        let dir = to_aim / dist;
        let mut vel = dir * speed;
        vel.y = vel.y.max(speed * 0.6);
        if let Some(o) = self.objects.get_mut(&pid) {
            o.scud_launcher_missile_projectile = true;
            o.scud_launcher_missile_toxin = toxin_warhead;
            o.scud_launcher_missile_aim = Some([aim.x, aim.y, aim.z]);
            o.scud_launcher_missile_travelled = 0.0;
            o.scud_launcher_missile_fuel_expires_frame =
                Some(self.frame.saturating_add(SCUD_MISSILE_FUEL_FRAMES));
            o.note_producer(source_id);
            o.health.maximum = SCUD_MISSILE_MAX_HEALTH;
            Self::write_object_health_authority_aware(o, SCUD_MISSILE_MAX_HEALTH);
            o.movement.velocity = vel;
            o.set_orientation(dir.z.atan2(dir.x));
            o.height_die = Some(HostHeightDieData::with_target(
                SCUD_MISSILE_HEIGHT_DIE_TARGET,
                true,
                self.frame.saturating_add(2),
            ));
            o.ensure_height_die(self.frame);
        }
        self.scud_missiles_spawned = self.scud_missiles_spawned.saturating_add(1);
        Some(pid)
    }

    pub fn update_scud_launcher_missile_projectiles(&mut self) {
        use crate::game_logic::host_scud_launcher::{
            SCUD_MISSILE_DIVE_DISTANCE, SCUD_MISSILE_INITIAL_VELOCITY, SCUD_MISSILE_LOFT_HEIGHT,
            SCUD_MISSILE_TURN_DISTANCE,
        };
        let frame = self.frame;
        let speed = SCUD_MISSILE_INITIAL_VELOCITY / 30.0;
        let flying: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.scud_launcher_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        let mut impact: Vec<(ObjectId, bool, Option<ObjectId>, glam::Vec3, Team)> = Vec::new();
        for id in flying {
            let (toxin, source, team, aim, pos, travelled, fuel_done) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let aim = o
                    .scud_launcher_missile_aim
                    .map(|a| glam::Vec3::new(a[0], a[1], a[2]))
                    .unwrap_or_else(|| o.get_position());
                let fuel_done = o
                    .scud_launcher_missile_fuel_expires_frame
                    .map(|f| f <= frame)
                    .unwrap_or(false);
                (
                    o.scud_launcher_missile_toxin,
                    o.producer_id,
                    o.team,
                    aim,
                    o.get_position(),
                    o.scud_launcher_missile_travelled,
                    fuel_done,
                )
            };
            let to_aim = aim - pos;
            let horiz = glam::Vec3::new(to_aim.x, 0.0, to_aim.z).length();
            let vel = if travelled < SCUD_MISSILE_TURN_DISTANCE {
                let dir = if to_aim.length() > 0.001 {
                    to_aim.normalize()
                } else {
                    glam::Vec3::Y
                };
                let mut v = dir * speed;
                if pos.y < aim.y + SCUD_MISSILE_LOFT_HEIGHT {
                    v.y = speed * 0.85;
                }
                v
            } else if horiz > SCUD_MISSILE_DIVE_DISTANCE {
                let loft_aim =
                    glam::Vec3::new(aim.x, aim.y + SCUD_MISSILE_LOFT_HEIGHT * 0.5, aim.z);
                let d = loft_aim - pos;
                if d.length() > 0.001 {
                    d.normalize() * speed
                } else {
                    glam::Vec3::new(0.0, -speed, 0.0)
                }
            } else {
                let d = aim - pos;
                if d.length() > 0.001 {
                    d.normalize() * speed
                } else {
                    glam::Vec3::new(0.0, -speed, 0.0)
                }
            };
            let step = vel.length().max(speed);
            if let Some(o) = self.objects.get_mut(&id) {
                o.movement.velocity = vel;
                let p = o.get_position();
                o.set_position(p + vel);
                o.scud_launcher_missile_travelled += step;
                o.set_orientation(vel.z.atan2(vel.x));
            }
            let new_pos = pos + vel;
            let near = glam::Vec3::new(aim.x - new_pos.x, 0.0, aim.z - new_pos.z).length() < 12.0
                && new_pos.y <= aim.y + 15.0;
            if fuel_done || near {
                impact.push((id, toxin, source, new_pos, team));
            }
        }
        for (id, toxin, source, pos, team) in impact {
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
                o.scud_launcher_missile_projectile = false;
            }
            let _ = self.apply_scud_area_at(pos, source, team, toxin);
            self.mark_object_for_destruction(id, Some(team));
        }

        // HeightDie residual: if freefall/low altitude kills missile, detonate warhead.
        let height_die_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.scud_launcher_missile_projectile && o.is_alive() {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in height_die_ids {
            let (sample_pos, name, ground) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                (o.get_position(), o.template_name.clone(), o.ground_height)
            };
            let terrain = self.height_die_terrain_at(sample_pos, &name, ground);
            let (toxin, source, team, pos, die) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let die = o.tick_height_die(frame, terrain);
                (
                    o.scud_launcher_missile_toxin,
                    o.producer_id,
                    o.team,
                    o.get_position(),
                    die,
                )
            };
            if die {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.scud_launcher_missile_projectile = false;
                }
                let _ = self.apply_scud_area_at(pos, source, team, toxin);
                self.mark_object_for_destruction(id, Some(team));
            }
        }
    }

    pub fn honesty_scud_missile_projectile_ok(&self) -> bool {
        self.scud_missiles_spawned > 0
    }
}

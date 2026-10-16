//! Pure condition-bank projection shared by live camera queries and frozen Draw inputs.
use super::*;

pub(super) struct DrawConditionFacts {
    pub active_weapon_slot: u8,
    pub ai_state_ordinal: u8,
    pub airborne_target: bool,
    pub anim_steer_turn: u8,
    pub armed: bool,
    pub armor_crate_upgrade: u8,
    pub attacking: bool,
    pub back_crushed: bool,
    pub body_damage_state: u8,
    pub captured: bool,
    pub cell_is_cliff: bool,
    pub cell_is_underwater: bool,
    pub combat_cycle_rider: u8,
    pub construction_percent: f32,
    pub continuous_fire_coast_until_frame: u32,
    pub continuous_fire_level: u8,
    pub death_type_name: String,
    pub destroyed: bool,
    pub disabled: bool,
    pub disguised: bool,
    pub enemy_near: bool,
    pub front_crushed: bool,
    pub is_deployed: bool,
    pub is_firing_weapon: bool,
    pub is_panicking: bool,
    pub is_structure: bool,
    pub is_surrendered: bool,
    pub is_unit: bool,
    pub jammed: bool,
    pub jet_slow_death_active: bool,
    pub logic_frame: u32,
    pub model_condition_bits: u128,
    pub moving: bool,
    pub moving_backwards: bool,
    pub object_type: PresentationObjectType,
    pub object_weather: i32,
    pub occupant_count: u16,
    pub over_water: bool,
    pub parachute_open: bool,
    pub parachuting: bool,
    pub production_door_phase: u8,
    pub prone: bool,
    pub radar_active: bool,
    pub radar_extend_complete: bool,
    pub second_life: bool,
    pub shock_allow_bounce: bool,
    pub shock_grounded_once: bool,
    pub shock_stun_frames: u32,
    pub shock_was_airborne: bool,
    pub stored_supplies: u32,
    pub structure_topple_matrix: Option<glam::Mat4>,
    pub topple_lean_radians: f32,
    pub turret_angle_deg: f32,
    pub under_construction: bool,
    pub user_1: bool,
    pub user_2: bool,
    pub using_ability: bool,
    pub veterancy: PresentationVeterancy,
    pub weapon_crate_upgrade: u8,
    pub weapon_fire_status: u8,
    pub weapon_set_player_upgrade: bool,
    pub world_is_night: bool,
    pub world_is_snow: bool,
}

mod lifecycle_bits;
mod motion_and_containment_bits;
mod weapon_and_damage_bits;

impl DrawConditionFacts {
    pub(super) fn bits(&self) -> u128 {
        let bits = self.motion_and_containment_bits(self.model_condition_bits);
        let bits = self.weapon_and_damage_bits(bits);
        self.lifecycle_bits(bits)
    }
}

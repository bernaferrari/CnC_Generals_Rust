//! Current immutable Drawable inputs at a camera observation point. No frame/event drain.
use super::*;

/// Same initial condition bank used by the frozen Drawable producer.
pub(super) fn current_base_condition_bits(
    logic: &GameLogic,
    obj: &crate::game_logic::Object,
) -> u128 {
    let health = logic
        .host_authoritative_health(obj.id)
        .unwrap_or(obj.health.current);
    let destroyed = obj.status.destroyed || health <= 0.01;
    let mut bits = obj.model_condition_bits;
    use crate::game_logic::host_enum_table_residual::{
        HostBodyDamageType, MC_BIT_ATTACKING, MC_BIT_DYING, MC_BIT_MOVING,
        host_apply_body_damage_model_bits,
    };
    use crate::game_logic::host_neutron_missile_slow_death::{MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED};
    let state = if destroyed {
        HostBodyDamageType::Rubble
    } else {
        obj.body_damage_state
    };
    bits = host_apply_body_damage_model_bits(bits, state);

    if obj.front_crushed {
        bits |= 1u128 << MC_BIT_FRONTCRUSHED;
    }
    if obj.back_crushed {
        bits |= 1u128 << MC_BIT_BACKCRUSHED;
    }
    if obj.status.moving {
        bits |= 1u128 << MC_BIT_MOVING;
    } else {
        bits &= !(1u128 << MC_BIT_MOVING);
    }
    if obj.status.attacking {
        bits |= 1u128 << MC_BIT_ATTACKING;
    } else {
        bits &= !(1u128 << MC_BIT_ATTACKING);
    }
    if destroyed {
        bits |= 1u128 << MC_BIT_DYING;
    } else {
        bits &= !(1u128 << MC_BIT_DYING);
    }
    use crate::game_logic::host_enum_table_residual::MC_BIT_DISGUISED;
    if obj.status.disguised {
        bits |= 1u128 << MC_BIT_DISGUISED;
    } else {
        bits &= !(1u128 << MC_BIT_DISGUISED);
    }

    bits
}

pub(super) fn receiving_local_team(logic: &GameLogic, local_player_id: u32) -> Team {
    let mut local_team = logic
        .get_player(local_player_id)
        .map(|p| p.team)
        .unwrap_or(Team::Neutral);
    // If the requested slot is missing or Neutral, own-team objects would
    // fall through to shroud HIDDEN. Prefer the first is_local non-Neutral
    // player so builders/CCs stay FULLY_VISIBLE (FOW still applies to
    // Neutral/unexplored enemies).
    if local_team == Team::Neutral {
        if let Some(team) = logic
            .get_players()
            .values()
            .find(|p| p.is_local && p.team != Team::Neutral)
            .map(|p| p.team)
        {
            local_team = team;
        }
    }
    // C++ always resolves a real local house (skirmish slot 0 = America):
    // there is no Neutral local controller.  Leaving local_team Neutral
    // makes the legacy team-fallback treat every object as hostile and
    // nothing as locally owned, so fail over to the default local house.
    if local_team == Team::Neutral {
        local_team = Team::USA;
    }
    local_team
}

pub(super) fn current_condition_facts(
    logic: &GameLogic,
    obj: &crate::game_logic::Object,
    local_player_id: u32,
) -> super::draw_conditions::DrawConditionFacts {
    let health = logic
        .host_authoritative_health(obj.id)
        .unwrap_or(obj.health.current);
    let destroyed = obj.status.destroyed || health <= 0.01;
    let body_damage_state = if destroyed {
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
    } else {
        obj.body_damage_state
    } as u8;
    let (construction_percent, under_construction) = logic
        .host_authoritative_construction(obj.id)
        .unwrap_or((obj.construction_percent, obj.status.under_construction));
    let (world_is_snow, world_is_night) = super::world_env::current_model_environment(logic);
    let first_detected = obj
        .contained_units()
        .first()
        .and_then(|id| logic.host_object(*id))
        .is_some_and(|o| o.status.detected);
    let hide_garrison = obj
        .building_data
        .as_ref()
        .is_some_and(|b| b.hide_garrisoned_state)
        && !first_detected
        && obj.team != receiving_local_team(logic, local_player_id);
    super::draw_conditions::DrawConditionFacts {
        active_weapon_slot: obj.active_weapon_slot,
        ai_state_ordinal: crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(
            &obj.ai_state(),
        ),
        airborne_target: obj.status.airborne_target,
        anim_steer_turn: obj
            .animation_steering
            .as_ref()
            .map(|s| s.current_turn_anim as u8)
            .unwrap_or(0),
        armed: obj.armed_riders_upgrade_weapon_set
            || (!obj.occupants.is_empty() && obj.passengers_allowed_to_fire),
        armor_crate_upgrade: obj.armor_crate_upgrade,
        attacking: obj.status.attacking,
        back_crushed: obj.back_crushed,
        body_damage_state: body_damage_state,
        captured: obj.has_captured_model_condition() || obj.is_private_captured(),
        cell_is_cliff: obj.cell_is_cliff,
        cell_is_underwater: obj.cell_is_underwater,
        combat_cycle_rider: obj.combat_cycle_rider,
        construction_percent: construction_percent.clamp(0.0, 1.0),
        continuous_fire_coast_until_frame: obj.continuous_fire_coast_until_frame,
        continuous_fire_level: obj.continuous_fire_level,
        death_type_name: if destroyed {
            obj.status.death_type.as_name().to_owned()
        } else {
            String::new()
        },
        destroyed: destroyed,
        disabled: obj.is_disabled(),
        disguised: obj.status.disguised,
        enemy_near: obj
            .enemy_near
            .as_ref()
            .is_some_and(|e| e.model_enemy_near || e.enemy_near),
        front_crushed: obj.front_crushed,
        is_deployed: obj.status.deployed,
        is_firing_weapon: obj.status.is_firing_weapon,
        is_panicking: obj.is_panicking,
        is_structure: obj.is_kind_of(KindOf::Structure),
        is_surrendered: obj.is_surrendered,
        is_unit: obj.is_kind_of(KindOf::Infantry)
            || obj.is_kind_of(KindOf::Vehicle)
            || obj.is_kind_of(KindOf::Aircraft),
        jammed: obj.status.weapons_jammed,
        jet_slow_death_active: obj.jet_slow_death.is_some(),
        logic_frame: logic.get_current_frame() as u32,
        model_condition_bits: current_base_condition_bits(logic, obj),
        moving: obj.status.moving,
        moving_backwards: obj.moving_backwards,
        object_type: PresentationObjectType::from_host(obj.object_type),
        object_weather: obj.object_weather,
        occupant_count: if hide_garrison {
            0
        } else {
            logic
                .host_authoritative_occupant_count(obj.id)
                .unwrap_or(obj.occupants.len().min(u16::MAX as usize) as u16)
        },
        over_water: obj.over_water,
        parachute_open: obj.is_parachute_open(),
        parachuting: obj.is_parachuting(),
        production_door_phase: obj.production_door_phase,
        prone: obj.prone_timer > 0.0,
        radar_active: obj.radar_active,
        radar_extend_complete: obj.radar_extend_complete,
        second_life: obj.armor_set_second_life,
        shock_allow_bounce: obj.shock_allow_bounce,
        shock_grounded_once: obj.shock_grounded_once,
        shock_stun_frames: obj.shock_stun_frames,
        shock_was_airborne: obj.shock_was_airborne,
        stored_supplies: obj.stored_resources.supplies,
        structure_topple_matrix: obj
            .structure_topple_data
            .as_ref()
            .map(|_| obj.get_transform_matrix()),
        topple_lean_radians: obj.presentation_topple_lean_radians(),
        turret_angle_deg: obj.turret_angle_deg,
        under_construction: under_construction,
        user_1: obj.model_condition_bits
            & (1u128 << crate::game_logic::host_enum_table_residual::user_1_model_bit())
            != 0,
        user_2: obj.model_condition_bits
            & (1u128 << crate::game_logic::host_enum_table_residual::user_2_model_bit())
            != 0,
        using_ability: obj.status.using_ability,
        veterancy: PresentationVeterancy::from_host(obj.experience.level),
        weapon_crate_upgrade: obj.weapon_crate_upgrade,
        weapon_fire_status: obj.weapon_fire_status as u8,
        weapon_set_player_upgrade: obj.weapon_set_player_upgrade,
        world_is_night: world_is_night,
        world_is_snow: world_is_snow,
    }
}

impl GameLogic {
    pub(crate) fn current_native_slave_source(
        &self,
        id: ObjectId,
        local_player_id: u32,
    ) -> Result<crate::game_logic::NativeSlaveSource, &'static str> {
        let obj = self.host_object(id).ok_or("missing source object")?;
        let template_name = super::build::direct_host_visual_template_name(obj);
        let receiving_draw_models = self
            .templates
            .get(&template_name)
            .and_then(|t| t.receiving_draw_models())
            .or_else(|| {
                (template_name == obj.get_template().name)
                    .then(|| obj.get_template().receiving_draw_models())
                    .flatten()
            })
            .ok_or("missing receiving Draw definitions")?;
        let mesh_scale = super::build::direct_host_visual_mesh_scale(self, obj, &template_name);
        if !mesh_scale.is_finite() {
            return Err("invalid receiving visual scale");
        }
        let facts = current_condition_facts(self, obj, local_player_id);
        let geometry = &(*obj.geometry_info());
        Ok(crate::game_logic::NativeSlaveSource {
            identity: self.live_draw_playback_identity(obj),
            mesh_scale,
            template_name,
            receiving_draw_models,
            condition_bits: facts.bits(),
            float_yaw: obj.float_update.as_ref().map_or(0.0, |f| f.yaw),
            float_pitch: obj.float_update.as_ref().map_or(0.0, |f| f.pitch),
            under_construction: facts.under_construction,
            construction_percent: facts.construction_percent,
            max_height_above_position: if geometry.authored {
                geometry.max_height_above_position()
            } else {
                0.0
            },
        })
    }
}

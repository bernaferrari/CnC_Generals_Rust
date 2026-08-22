use super::*;

fn leftover_auto_acquire_idle_bits(template_name: &str) -> u32 {
    let _ = template_name;
    0
}

fn leftover_auto_acquire_idle_yes(template_name: &str) -> bool {
    (leftover_auto_acquire_idle_bits(template_name)
        & gamelogic::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE)
        != 0
}

impl Object {
    pub fn new(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        Self::new_with_logic_frame(template, id, team, 0)
    }

    /// Construct an Object with the authoritative logic frame used by C++
    /// `Weapon::Weapon` when it schedules each temporary weapon's
    /// `SuspendFXFrame`.  The legacy constructor remains frame-zero for
    /// command/test callers that do not own a GameLogic clock.
    pub fn new_with_logic_frame(
        template: ThingTemplate,
        id: ObjectId,
        team: Team,
        logic_frame: u32,
    ) -> Self {
        let max_health = template.max_health;
        let position = Vec3::ZERO; // Default position
        let template_name = template.name.clone();
        let locomotor_set_names = template.locomotor_set_names.clone();
        let cur_locomotor_name = template.locomotor_name.clone();
        let crusher_level = template.crusher_level;
        let crushable_level = template.crushable_level;
        let has_squish_collide =
            crate::game_logic::host_squish_collide::template_has_squish_collide(&template_name);
        let temporary_weapon_runtime = crate::game_logic::host_temporary_weapon_behavior::
            TemporaryWeaponRuntimeBundle::from_thing_template(&template, logic_frame);
        // A normal player Enter requires a real Contain module.  Capture its
        // authored capacity before the template moves into `Thing`; do not
        // synthesize slots from the vehicle's footprint or selection radius.
        // Keep the older railed field as a snapshot compatibility bridge.
        let authored_transport_slots = if template.contain_module.kind.is_mobile_container()
            || template.contain_module.kind == crate::game_logic::ContainModuleKind::InternetHack
        {
            template.contain_module.slots
        } else {
            None
        }
        .or(template.railed_transport_slots)
        .unwrap_or(0);

        // Determine object type from template
        let object_type = if template.is_kind_of(KindOf::Infantry) {
            ObjectType::Infantry
        } else if template.is_kind_of(KindOf::Vehicle) {
            ObjectType::Vehicle
        } else if template.is_kind_of(KindOf::Aircraft) {
            ObjectType::Aircraft
        } else if template.is_kind_of(KindOf::Structure) {
            ObjectType::Building
        } else {
            ObjectType::Neutral
        };

        // C++ Object copies ThingTemplate::m_geometryInfo; pick radius is the
        // bounding circle (`Geometry.cpp:468-495`), not a KindOf table.
        let selection_radius = if template.geometry_info.authored {
            template.geometry_info.bounding_circle_radius()
        } else {
            match object_type {
                ObjectType::Infantry => 8.0,
                ObjectType::Vehicle => 15.0,
                ObjectType::Aircraft => 20.0,
                ObjectType::Building => 25.0,
                ObjectType::Neutral => 10.0,
                _ => 10.0,
            }
        };

        let mut building_data = if object_type == ObjectType::Building {
            let building_type = BuildingType::from_template_name(&template_name);
            Some(BuildingData::new(building_type))
        } else {
            None
        };

        // `GarrisonContain::ContainMax` is an authored containment interface,
        // not a building-name category.  HealContain uses the same ContainMax
        // body-count list (C++ OpenContain) so barracks/hospitals are enterable
        // without becoming GarrisonContain (garrison_contain_max stays None).
        let authored_garrison_capacity = if matches!(
            template.contain_module.kind,
            crate::game_logic::ContainModuleKind::Garrison
                | crate::game_logic::ContainModuleKind::Heal
        ) {
            template.contain_module.slots
        } else {
            None
        }
        .or(template.garrison_contain_max);
        if let (Some(building), Some(contain_max)) =
            (building_data.as_mut(), authored_garrison_capacity)
        {
            building.max_garrison = contain_max;
        }

        let special_power_cooldown = template.special_power_cooldown;
        let subdual_damage_cap = template.subdual_damage_cap.max(0.0);
        let subdual_heal_rate_frames = template.subdual_heal_rate_frames;
        let subdual_heal_amount = template.subdual_heal_amount.max(0.0);
        let pitch_roll_yaw_factor = if template.pitch_roll_yaw_factor.is_finite()
            && template.pitch_roll_yaw_factor > 0.0
        {
            template.pitch_roll_yaw_factor
        } else {
            2.0
        };
        let physics_mass = template.physics_mass.max(1.0e-4);
        let shock_resistance = template.shock_resistance.max(0.0);
        let forward_friction = template.forward_friction;
        let lateral_friction = template.lateral_friction;
        let z_friction = template.z_friction;
        let aerodynamic_friction = template.aerodynamic_friction;
        let center_of_mass_offset = template.center_of_mass_offset;
        let kill_when_resting_on_ground = template.kill_when_resting_on_ground;
        let min_fall_speed_for_damage = if template.min_fall_speed_for_damage.is_finite()
            && template.min_fall_speed_for_damage > 0.0
        {
            template.min_fall_speed_for_damage
        } else {
            Self::height_to_fall_speed(40.0)
        };
        let fall_height_damage_factor = if template.fall_height_damage_factor.is_finite() {
            template.fall_height_damage_factor
        } else {
            1.0
        };
        let allow_collide_force = template.allow_collide_force;
        let allow_bouncing = template.allow_bouncing;

        let (mut power_provided, mut power_consumed) = building_data
            .as_ref()
            .map(|data| (data.power_output, data.power_requirement))
            .unwrap_or((0, 0));
        // C++ `EnergyProduction` is a parsed Object INI field.  In
        // particular, do not derive power use from a Particle/Scud/Nuke
        // basename: a spoofed structure with those words owns no power delta.
        if let Some(energy) = template.energy_production {
            let (p, c) =
                crate::game_logic::host_superweapon_kindof::apply_energy_production_to_power(
                    energy,
                );
            power_provided = p;
            power_consumed = c;
        }

        let mut template = template;
        template.bind_weapon_set_from_live_assets();
        let tracker = template.weapon_tracker_bind();
        let dock_starting_boxes = template.dock_starting_boxes.unwrap_or(0);
        let vision_range = template.sight_range.max(0.0);
        let shroud_clearing_range = template.resolved_shroud_clearing_range().max(0.0);
        let locomotor_set_names = template.locomotor_set_names.clone();
        let locomotor_name = template.locomotor_name.clone();
        let turret = turret_spawn_for_template(&template_name);
        let is_fs_fake = template.is_kind_of(crate::game_logic::KindOf::FSFake);



        Self {
            thing: Thing::new(template),
            id,
            team,
            owner_player_id: None,
            team_instance_name: String::new(),
            name: String::new(),
            status: ObjectStatus::default(),
            object_status_bits: 0,
            script_unsellable: false,
            indestructible: false,

            eject_pilot_die_applied: false,
            model_condition_bits: 0,
            object_weather: 0,

            radar_extend_done_frame: 0,
            radar_extend_complete: false,
            radar_active: false,
            production_door_phase: 0,
            production_door_phase_end_frame: 0,
            production_door_hold_open: false,
            production_door_phases: [0; 4],
            production_door_phase_end_frames: [0; 4],
            production_door_active_index: 0,
            is_rebuild_hole: false,
            rebuild_template_name: None,
            rebuild_ready_frame: 0,
            rebuild_spawner_id: None,
            rebuild_worker_id: None,
            rebuild_reconstructing_id: None,
            producer_id: None,
            builder_id: None,
            dozer_task_build_target: None,
            dozer_task_build_order_frame: 0,
            dozer_task_repair_target: None,
            dozer_task_repair_order_frame: 0,
            dozer_dock_action: None,



            preferred_dock_id: None,
            supply_center_spawn_behavior_fired: false,
            supply_truck_state: SupplyTruckState::Idle,
            supply_truck_force_pending: false,
            supply_truck_next_dock_action_frame: 0,
            dock_active_docker: None,
            railed_in_transit: false,
            railed_waypoint_data_loaded: false,
            railed_current_path: crate::game_logic::RAILED_INVALID_PATH,
            railed_paths: Vec::new(),

            drawable_supply_boxes: dock_starting_boxes,
            drawable_supply_max_boxes: dock_starting_boxes,
            repair_dock_last_id: None,
            repair_dock_health_per_sec: 0.0,
            temporary_stealth_expires_frame: 0,

            highlander_body: false,
            upgrade_die: None,
            construction_complete_clear_frame: 0,
            sole_healing_benefactor: None,
            sole_healing_benefactor_expiration_frame: 0,
            idle_since_frame: 0,
            shock_stun_frames: 0,
            shock_yaw_rate: 0.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_allow_bounce: allow_bouncing,
            shock_was_airborne: false,
            shock_grounded_once: false,
            shock_up_z: 1.0,
            locomotor_surfaces: 0,
            cell_is_cliff: false,
            cell_is_underwater: false,
            kill_when_resting_on_ground,
            min_fall_speed_for_damage,
            fall_height_damage_factor,
            pending_ground_collide: false,
            immune_to_falling_damage: false,
            bounce_land_events: 0,
            last_bounce_fall_dy: 0.0,
            bounce_sound_name: String::new(),
            last_bounce_volume: 0.0,
            bounce_audio_pending: 0,
            crusher_level,
            crushable_level,
            has_squish_collide,
            topple_data: None,
            structure_topple_data: None,
            structure_collapse_data: None,
            keep_object_die: None,
            wave_guide_data: None,
            fire_weapon_when_dead_fired: false,
            bone_fx_damage: None,
            poisoned_behavior: None,
            defection_helper: None,
            fire_weapon_power: None,
            fire_weapon_when_damaged: None,
            temporary_weapon_runtime,
            pending_fire_when_damaged_weapon: None,
            transition_damage_fx: None,
            pending_transition_damage_fx: Vec::new(),
            fx_list_die: None,
            pending_death_fx: None,
            pending_death_audio: None,
            create_object_die: None,
            pending_create_object_die_spawns: Vec::new(),
            create_object_die_transfer_damage: 0.0,
            create_object_die_transfer_subdual: 0.0,
            create_object_die_transfer_source: None,
            pending_instant_death_weapon: None,
            crush_die: None,
            previous_health: max_health,
            lifetime_update: None,
            slow_death: None,
            height_die: None,
            fuel_air_gas_slow_death: None,
            neutron_missile_update: None,
            scud_storm_missile_flight: None,
            carpet_bomb_payload: false,
            carpet_bomb_transport: None,
            artillery_barrage_shell: false,
            artillery_barrage_transport: None,
            a10_strike_missile: false,
            a10_strike_transport: None,
            leaflet_transport_target: None,
            leaflet_container: false,
            paradrop_transport_target: None,
            paradrop_parachute: false,
            daisy_cutter_transport: None,
            daisy_cutter_bomb: false,
            anthrax_bomb_transport: None,
            anthrax_bomb_payload: false,
            sneak_tunnel_start: false,
            cluster_mines_transport: None,
            cluster_mines_bomb: false,
            emp_pulse_transport: None,
            emp_pulse_bomb: false,
            emp_pulse_spheroid: false,
            emp_pulse_spheroid_expires_frame: None,
            particle_trail_remnant: false,
            particle_trail_remnant_expires_frame: None,
            nuke_radiation_field: false,
            nuke_radiation_field_expires_frame: None,
            anthrax_toxin_field: false,
            anthrax_toxin_field_expires_frame: None,
            spectre_howitzer_shell: false,
            spectre_howitzer_shell_expires_frame: None,
            particle_orbital_laser: false,
            particle_orbital_laser_expires_frame: None,
            particle_connector_laser: false,
            particle_connector_laser_expires_frame: None,
            point_defense_laser_beam: false,
            point_defense_laser_beam_expires_frame: None,
            missile_defender_laser_beam: false,
            missile_defender_laser_beam_expires_frame: None,
            booby_trap_special: false,
            booby_trap_attached_to: None,
            countermeasure_flare: false,
            countermeasure_flare_expires_frame: None,
            angry_mob_member: false,
            angry_mob_nexus_id: None,
            weapon_laser_beam: false,
            weapon_laser_beam_expires_frame: None,
            comanche_rocket_pod_projectile: false,
            comanche_rocket_pod_projectile_expires_frame: None,
            stealth_jet_missile_projectile: false,
            stealth_jet_missile_aim: None,
            stealth_jet_missile_intended: None,
            stealth_jet_missile_travelled: 0.0,
            stealth_jet_missile_fuel_expires_frame: None,
            stealth_jet_missile_ignition_frame: None,
            stealth_jet_missile_expires_frame: None,
            helix_napalm_bomb_projectile: false,
            scud_launcher_missile_projectile: false,
            scud_launcher_missile_toxin: false,
            scud_launcher_missile_aim: None,
            scud_launcher_missile_travelled: 0.0,
            scud_launcher_missile_fuel_expires_frame: None,
            tomahawk_missile_projectile: false,
            tomahawk_missile_aim: None,
            tomahawk_missile_travelled: 0.0,
            tomahawk_missile_fuel_expires_frame: None,
            aurora_bomb_projectile: false,
            aurora_bomb_aim: None,
            aurora_bomb_mission_id: None,
            rocket_buggy_missile_projectile: false,
            rocket_buggy_missile_aim: None,
            rocket_buggy_missile_intended: None,
            rocket_buggy_missile_travelled: 0.0,
            rocket_buggy_missile_fuel_expires_frame: None,
            neutron_cannon_shell_projectile: false,
            neutron_shell_from: None,
            neutron_shell_aim: None,
            neutron_shell_launch_frame: None,
            neutron_shell_flight_frames: 0,
            nuke_cannon_shell_projectile: false,
            nuke_shell_from: None,
            nuke_shell_aim: None,
            nuke_shell_launch_frame: None,
            nuke_shell_flight_frames: 0,
            usa_tank_shell_projectile: false,
            usa_tank_shell_from: None,
            usa_tank_shell_aim: None,
            usa_tank_shell_launch_frame: None,
            usa_tank_shell_flight_frames: 0,
            usa_tank_shell_weapon_speed: 0.0,
            usa_tank_shell_intended: None,
            battlemaster_shell_projectile: false,
            battlemaster_shell_from: None,
            battlemaster_shell_aim: None,
            battlemaster_shell_launch_frame: None,
            battlemaster_shell_flight_frames: 0,
            battlemaster_shell_intended: None,
            overlord_shell_projectile: false,
            overlord_shell_from: None,
            overlord_shell_aim: None,
            overlord_shell_launch_frame: None,
            overlord_shell_flight_frames: 0,
            overlord_shell_intended: None,
            inferno_shell_projectile: false,
            inferno_shell_from: None,
            inferno_shell_aim: None,
            inferno_shell_launch_frame: None,
            inferno_shell_flight_frames: 0,
            inferno_shell_intended: None,
            inferno_shell_upgraded: false,
            marauder_shell_projectile: false,
            marauder_shell_from: None,
            marauder_shell_aim: None,
            marauder_shell_launch_frame: None,
            marauder_shell_flight_frames: 0,
            marauder_shell_intended: None,
            marauder_shell_weapon_speed: 0.0,
            fire_base_shell_projectile: false,
            fire_base_shell_from: None,
            fire_base_shell_aim: None,
            fire_base_shell_launch_frame: None,
            fire_base_shell_flight_frames: 0,
            fire_base_shell_intended: None,
            raptor_missile_projectile: false,
            raptor_missile_aim: None,
            raptor_missile_intended: None,
            raptor_missile_travelled: 0.0,
            raptor_missile_fuel_expires_frame: None,
            raptor_missile_ignition_frame: None,
            mig_missile_projectile: false,
            mig_missile_aim: None,
            mig_missile_intended: None,
            mig_missile_travelled: 0.0,
            mig_missile_fuel_expires_frame: None,
            mig_missile_ignition_frame: None,
            flashbang_grenade_projectile: false,
            flashbang_grenade_from: None,
            flashbang_grenade_aim: None,
            flashbang_grenade_launch_frame: None,
            flashbang_grenade_flight_frames: 0,
            flashbang_grenade_intended: None,
            humvee_tow_projectile: false,
            humvee_tow_air: false,
            humvee_tow_aim: None,
            humvee_tow_intended: None,
            humvee_tow_travelled: 0.0,
            humvee_tow_fuel_expires_frame: None,
            humvee_tow_ignition_frame: None,
            dragon_flame_projectile: false,
            dragon_flame_aim: None,
            dragon_flame_intended: None,
            dragon_flame_travelled: 0.0,
            dragon_flame_fuel_expires_frame: None,
            dragon_flame_ignition_frame: None,
            dragon_flame_shooter: None,
            toxin_stream_projectile: false,
            toxin_stream_aim: None,
            toxin_stream_intended: None,
            toxin_stream_travelled: 0.0,
            toxin_stream_fuel_expires_frame: None,
            toxin_stream_ignition_frame: None,
            toxin_stream_shooter: None,
            technical_rpg_missile_projectile: false,
            technical_rpg_missile_aim: None,
            technical_rpg_missile_intended: None,
            technical_rpg_missile_travelled: 0.0,
            technical_rpg_missile_fuel_expires_frame: None,
            technical_rpg_missile_ignition_frame: None,
            technical_cannon_shell_projectile: false,
            technical_cannon_shell_from: None,
            technical_cannon_shell_aim: None,
            technical_cannon_shell_launch_frame: None,
            technical_cannon_shell_flight_frames: 0,
            technical_cannon_shell_intended: None,
            ecm_missile_jammed: false,
            cleanup_stream_projectile: false,
            cleanup_stream_aim: None,
            cleanup_stream_intended: None,
            cleanup_stream_travelled: 0.0,
            cleanup_stream_fuel_expires_frame: None,
            cleanup_stream_ignition_frame: None,
            cleanup_stream_shooter: None,
            cleanup_stream_player_id: 0,
            angry_mob_projectile: false,
            angry_mob_projectile_kind: 0,
            angry_mob_projectile_from: None,
            angry_mob_projectile_aim: None,
            angry_mob_projectile_launch_frame: None,
            angry_mob_projectile_flight_frames: 0,
            angry_mob_projectile_intended: None,
            inferno_fire_field: false,
            inferno_fire_field_upgraded: false,
            inferno_fire_field_expires_frame: None,
            inferno_fire_field_zone_id: None,
            rpg_trooper_missile_projectile: false,
            rpg_trooper_missile_aim: None,
            rpg_trooper_missile_intended: None,
            rpg_trooper_missile_travelled: 0.0,
            rpg_trooper_missile_fuel_expires_frame: None,
            tank_hunter_missile_projectile: false,
            tank_hunter_missile_aim: None,
            tank_hunter_missile_intended: None,
            tank_hunter_missile_travelled: 0.0,
            tank_hunter_missile_fuel_expires_frame: None,
            missile_defender_missile_projectile: false,
            missile_defender_missile_aim: None,
            missile_defender_missile_intended: None,
            missile_defender_missile_travelled: 0.0,
            missile_defender_missile_fuel_expires_frame: None,
            missile_defender_missile_laser_slot: false,
            scorpion_shell_projectile: false,
            scorpion_shell_from: None,
            scorpion_shell_aim: None,
            scorpion_shell_launch_frame: None,
            scorpion_shell_flight_frames: 0,
            scorpion_shell_slot: 0,
            scorpion_missile_projectile: false,
            scorpion_missile_aim: None,
            scorpion_missile_intended: None,
            scorpion_missile_travelled: 0.0,
            scorpion_missile_fuel_expires_frame: None,
            scorpion_missile_slot: 0,
            airfield_rearm_ready_frame: None,
            jet_producer_location: None,
            jet_circling_dead_airfield: false,
            jet_circling_airfield_check_frame: 0,
            airfield_rearm_duration_frames: 0,
            return_to_base_requested: false,
            jet_ai: HostJetAi::default(),

            airfield_parking_space_index: None,
            frenzy_invisible_marker: false,
            ambush_fade_in: false,
            gps_scrambler_marker: false,
            emergency_repair_marker: false,
            spy_satellite_ping: false,
            spy_satellite_ping_expires_frame: None,
            radar_van_ping: false,
            radar_van_ping_expires_frame: None,
            firewall_segment: false,
            firewall_segment_expires_frame: None,
            firewall_segment_wall_id: None,
            firewall_segment_dir: None,
            tensile_formation: None,
            fire_spread: None,
            base_regenerate: None,
            default_auto_heal: None,
            enemy_near: None,

            animation_steering: None,
            float_update: None,
            prone_update: None,
            radius_decal_update: None,
            checkpoint_update: None,
            spectre_gunship_deployment: None,
            smart_bomb_target_homing: None,
            helicopter_slow_death: None,
            jet_slow_death: None,
            front_crushed: false,
            back_crushed: false,
            physics_current_overlap: None,
            physics_previous_overlap: None,
            ignore_collisions_with: None,
            last_collidee: None,
            allow_collide_force,
            can_path_through_units: false,
            ignore_collisions_until_frame: 0,
            is_blocked: false,
            is_blocked_and_stuck: false,
            cur_max_blocked_speed: f32::MAX,
            num_frames_blocked: 0,
            bump_speed_limit: f32::MAX,
            locomotor_set_names,
            cur_locomotor_name,
            is_panicking: false,
            physics_mass,
            shock_resistance,
            physics_accel: glam::Vec3::ZERO,
            motive_frames_remaining: 0,
            waiting_for_path: false,
            move_away_from: None,
            move_away_frames: 0,
            move_away_destination: None,
            request_other_move_away: None,
            forward_friction,
            lateral_friction,
            z_friction,
            aerodynamic_friction,
            extra_friction: 0.0,
            apply_friction_2d_when_airborne: false,
            velocity_magnitude_cache: -1.0,
            original_allow_bounce: allow_bouncing,
            stick_to_ground: false,
            allow_to_fall: false,
            was_airborne_last_frame: false,
            center_of_mass_offset,
            pitch_roll_yaw_factor,
            is_braking: false,
            braking_factor: 1.0,
            braking: 99999.0,
            loco_apply_2d_friction_airborne: false,
            allow_motive_force_while_airborne: false,
            loco_extra_2d_friction: 0.0,
            physics_turning: PhysicsTurningType::TurnNone,
            loco_behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            loco_preferred_height: 0.0,
            loco_preferred_height_damping: 1.0,
            maintain_pos_valid: false,
            maintain_pos: None,
            loco_appearance: LocomotorAppearance::Other,
            min_turn_speed: 0.0,
            min_speed: 0.0,
            ultra_accurate_slide_factor: 0.0,
            ultra_accurate: false,
            can_move_backward: false,
            moving_backwards: false,
            no_slow_down_as_approaching_dest: false,
            over_water: false,
            circling_radius: 0.0,
            precise_z_pos: false,
            is_dozer: false,
            on_invalid_movement_terrain: false,
            turn_pivot_offset: 0.0,
            wander_width_factor: 0.0,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            wander_offset_increasing: true,
            downhill_only: false,
            is_climbing: false,
            donut_timer: 0,
            max_lift: 0.0,
            max_lift_damaged: 0.0,
            speed_limit_z: 999999.0,
            group_speed_factor: 1.0,
            is_attack_path: false,
            is_exact_path: false,
            is_approach_path: false,
            is_safe_path: false,
            requested_victim_id: None,
            requested_destination: None,
            path_timestamp: 0,
            queue_for_path_frames: 0,
            max_shots_to_fire: -1,
            attack_substate: crate::game_logic::AttackSubState::AimAtTarget,
            approach_timestamp: 0,
            prev_victim_pos: None,
            temporary_move_frames: 0,
            body_damage_state:
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
            move_loop_audio: None,
            ambient_audio: None,
            health: Health::new(max_health),
            movement: Movement::default(),
            experience: Experience::default(),
            experience_sink: None,
            experience_scalar: 1.0,
            weapon: None,
            mine_clearing_primary_weapon: None,
            secondary_weapon: None,
            tertiary_weapon: None,
            target: None,
            capture_channel: None,
            hacker_disable_channel: None,
            charge_plant_unpack_remaining_seconds: None,

            construction_percent: 1.0, // Fully constructed by default
            building_data,
            stored_resources: Resources::default(),
            power_provided,
            power_consumed,
            selected: false,
            selection_flash_remaining: 0,
            ai_state: AIState::Idle,
            object_type,
            template_name: template_name.clone(),
            position,
            max_health,
            target_location: None,
            guard_position: None,
            guard_retaliate_victim: None,
            guard_retaliate_anchor: None,
            crate_created: None,
            hunting: false,
            hijack_vehicle_id: None,
            hijacker_in_vehicle: false,
            hijacker_update_active: false,
            hijacker_was_airborne: false,
            hijacker_eject_pos: None,
            drawable_hidden: false,

            weapon_crate_upgrade: 0,
            armor_crate_upgrade: 0,
            guard_target: None,
            guard_chase_phase: 0,
            guard_chase_give_up_frame: 0,

            force_attack: false,
            show_health_bar: true, // Show health bars by default
            selection_radius,
            ground_height: 0.0,
            ground_height_from_terrain: false,
            team_color: team.get_color(),
            occupants: Vec::new(),
            player_who_entered: String::new(),

            max_transport: authored_transport_slots,
            overlord_bunker_capacity: None,
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            weapon_bonus_player_upgrade: false,
            armor_set_player_upgrade: false,
            armor_set_veteran: false,
            armor_set_elite: false,
            armor_set_hero: false,
            weapon_set_veteran: false,
            weapon_set_elite: false,
            weapon_set_hero: false,
            weapon_bonus_veteran: false,
            weapon_bonus_elite: false,
            weapon_bonus_hero: false,
            locomotor_upgrade: false,
            terrain_decal_chemsuit: false,
            terrain_decal_type: if is_fs_fake {
                crate::game_logic::host_battlemaster::TERRAIN_DECAL_SHADOW_TEXTURE
            } else {
                8
            },
            terrain_decal_size: 0.0,
            terrain_decal_fade_target: 0.0,
            terrain_decal_fade_rate: 0.0,
            terrain_decal_opacity: if is_fs_fake {
                1.0
            } else {
                0.0
            },
            sub_object_visibility: crate::game_logic::host_battlemaster::leftover_horde_flag_visibility_for_template(
                &template_name,
            ),
            special_power_completion: None,
            power_plant_rods_extended: false,
            power_plant_rods_done_frame: 0,
            special_power_paused: std::collections::HashMap::new(),
            weapon_set_mine_clearing_detail: false,
            weapon_set_carbomb: false,
            weapon_set_vehicle_hijack: false,
            is_battle_bus_transport: false,
            battle_bus_body: None,
            armor_set_second_life: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            rider_change_active_slot: None,
            rider_change_model_condition_mask: 0,
            rider_change_object_status_mask: 0,
            rider_change_weapon_set: None,
            rider_change_locomotor_set: None,
            rider_change_locomotor_name: None,
            rider_change_scuttled_on_frame: 0,
            is_tunnel_network: false,
            cave_index: 0,
            is_cave_contain: false,
            is_combat_chinook_transport: false,
            chinook_ai: None,
            contained_by: None,
            cheer_timer: 0.0,
            prone_timer: 0.0,
            emoticon_name: String::new(),
            emoticon_frames_left: 0,
            is_surrendered: false,
            formation_id: 0,
            formation_offset: glam::Vec2::ZERO,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            weapon_lock_type: WeaponLockType::NotLocked,
            weapon_lock_slot: 0,
            weapon_fire_status: WeaponFireStatus::ReadyToFire,
            fire_sound_loop_until_frame: 0,
            fire_sound_loop_name: String::new(),
            weapon_barrel_states: std::array::from_fn(|_| WeaponBarrelState::default()),
            weapon_scatter_targets_unused: std::array::from_fn(|_| Vec::new()),
            weapon_scatter_targets_inited: [false; 3],
            pre_attack_target: None,
            pre_attack_ready_at: 0.0,
            consecutive_shot_target: None,
            consecutive_shots_at_target: 0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            last_fire_victim_host: 0,
            last_fire_slot: 0,
            last_fire_damage: 0.0,
            last_fire_range: 0.0,
            last_fire_sim_time: 0.0,
            last_fire_frame: 0,
            fire_intent_count: 0,
            last_weapon_discharge_sequence: 0,
            last_weapon_discharge_slot: 0,
            last_weapon_discharge_barrel: 0,
            last_weapon_discharge_frame: 0,
            visual_object_generation: 0,
            visual_draw_state_revision: 1,
            pending_weapon_visual_capture: None,
            guard_radius: 0.0,
            guard_mode: GuardMode::Normal,
            pending_evacuate_on_stop: false,
            pending_exit_after_evacuate: false,
            pending_stream_exit: false,

            frame_exit_not_busy: 0,
            which_exit_path: 0,

            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown,
            special_power_cooldown_remaining: 0.0,
            special_power_cooldowns: HashMap::new(),
            special_power_override_destination: None,
            special_power_override_type: None,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            next_detection_scan_frame: 0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            disguise_as_template: None,
            disguise_pending_template: None,
            disguise_pending_team: None,
            disguise_as_team: None,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_fanaticism: false,
            last_horde_refresh_frame: 0,
            horde_next_wake_frame: 0,
            horde_wake_initialized: false,

            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_until_frame: 0,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            weapon_bonus_drone_spotting: false,
            battle_plan_sight_scalar_applied: 1.0,
            continuous_fire_consecutive: 0,
            continuous_fire_level: 0,
            continuous_fire_one_shots: tracker.continuous_fire_one_shots,
            continuous_fire_two_shots: tracker.continuous_fire_two_shots,
            continuous_fire_coast_frames: tracker.continuous_fire_coast_frames,
            auto_reload_when_idle_frames: tracker.auto_reload_when_idle_frames,
            frame_to_force_reload: 0,
            continuous_fire_coast_until_frame: 0,
            fire_ocl_after_cooldown: None,
            continuous_fire_victim: 0,
            faerie_fire_until_frame: 0,
            subdual_damage: 0.0,
            subdual_damage_cap,
            subdual_heal_rate_frames,
            subdual_heal_amount,
            subdual_heal_countdown: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_pathfinder_unit: false,
            is_troop_crawler_transport: false,
            assault_transport: None,
            deploy_style: None,
            command_button_hunt: None,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            is_helix_transport: false,
            command_set_override: None,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            hive_slave_respawn_frame: 0,
            hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3],
            turret_angle_deg: turret.angle_deg,
            turret_pitch_deg: turret.pitch_deg,
            turret_idle_scan_next_frame: 0,
            turret_idle_scanning: false,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_holding: false,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_mood_target: false,
            turret_target_id: None,
            turret_force_attacking: false,
            turret_enabled: turret.enabled,
            turret_turn_rate_rad: turret.turn_rate_rad,
            turret_substate: TurretSubState::Idle,
            turret_rotating: false,
            turret_natural_angle_deg: turret.natural_angle_deg,
            turret_natural_pitch_deg: turret.natural_pitch_deg,
            turret_recenter_frames: turret.recenter_frames,
            ai_attitude: 0, // HostAiAttitude::Normal
            repulsor_until_frame: 0,
            last_damage_source: None,
            last_damage_timestamp: None,
            last_healing_timestamp: None,
            last_damage_fx_done: None,
            next_damage_fx_time: 0,
            last_damage_source_preferred: false,
            health_box_offset: [0.0, 0.0, 0.0],
            uses_inactive_body: false,
            inactive_body_die_called: false,
            next_mood_check_time: 0,
            mood_attack_check_rate: default_mood_attack_check_rate(),
            vision_range,
            shroud_clearing_range,
            shroud_range: 0.0,
            partition_cash_value: 0,
            partition_threat_value: 0,
            partition_last_affect: None,
            partition_last_look: None,

            auto_acquire_when_idle: leftover_auto_acquire_idle_yes(&template_name),
            auto_acquire_idle_bits: leftover_auto_acquire_idle_bits(&template_name),
            attack_priority_set: None,
            camo_friendly_opacity: 1.0,
            camo_opacity_pulse_phase: 0.0,
            camo_stealth_look: 0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
            drawable_fade_mode: 0,
            drawable_fade_start_frame: 0,
            drawable_fade_frames: 0,
            drawable_explicit_opacity: 1.0,
            drawable_instance_scale: 1.0,
            drawable_tint_status: 0,
            drawable_prev_tint_status: 0,
            drawable_expiration_date: 0,
            drawable_loco_pitch: 0.0,
            drawable_loco_pitch_rate: 0.0,
            drawable_loco_roll: 0.0,
            drawable_loco_roll_rate: 0.0,
            drawable_loco_yaw: 0.0,
            drawable_loco_accel_pitch: 0.0,
            drawable_loco_accel_pitch_rate: 0.0,
            drawable_loco_accel_roll: 0.0,
            drawable_loco_accel_roll_rate: 0.0,
            drawable_overlay_icons: Vec::new(),
        }
    }

    /// Alternative constructor for command system compatibility
    pub fn new_simple(id: ObjectId, object_type: ObjectType, template_name: String) -> Self {
        let mut template = ThingTemplate::new(&template_name);
        template.bind_weapon_set_from_live_assets();
        let tracker = template.weapon_tracker_bind();
        let team = Team::Neutral;
        let locomotor_set_names = template.locomotor_set_names.clone();
        let locomotor_name = template.locomotor_name.clone();
        let temporary_weapon_runtime = crate::game_logic::host_temporary_weapon_behavior::
            TemporaryWeaponRuntimeBundle::from_thing_template(&template, 0);
        let selection_radius = if template.geometry_info.authored {
            template.geometry_info.bounding_circle_radius()
        } else {
            match object_type {
                ObjectType::Infantry => 8.0,
                ObjectType::Vehicle => 15.0,
                ObjectType::Aircraft => 20.0,
                ObjectType::Building => 25.0,
                ObjectType::Neutral => 10.0,
                _ => 10.0,
            }
        };
        let turret = turret_spawn_for_template(&template_name);

        let pitch_roll_yaw_factor = if template.pitch_roll_yaw_factor.is_finite()
            && template.pitch_roll_yaw_factor > 0.0
        {
            template.pitch_roll_yaw_factor
        } else {
            2.0
        };
        let physics_mass = template.physics_mass.max(1.0e-4);
        let shock_resistance = template.shock_resistance.max(0.0);
        let forward_friction = template.forward_friction;
        let lateral_friction = template.lateral_friction;
        let z_friction = template.z_friction;
        let aerodynamic_friction = template.aerodynamic_friction;
        let center_of_mass_offset = template.center_of_mass_offset;
        let kill_when_resting_on_ground = template.kill_when_resting_on_ground;
        let min_fall_speed_for_damage = if template.min_fall_speed_for_damage.is_finite()
            && template.min_fall_speed_for_damage > 0.0
        {
            template.min_fall_speed_for_damage
        } else {
            Self::height_to_fall_speed(40.0)
        };
        let fall_height_damage_factor = if template.fall_height_damage_factor.is_finite() {
            template.fall_height_damage_factor
        } else {
            1.0
        };
        let allow_collide_force = template.allow_collide_force;
        let allow_bouncing = template.allow_bouncing;
        let dock_starting_boxes = template.dock_starting_boxes.unwrap_or(0);
        let crusher_level = template.crusher_level;
        let crushable_level = template.crushable_level;
        let has_squish_collide =
            crate::game_logic::host_squish_collide::template_has_squish_collide(&template_name);
        let is_fs_fake = template.is_kind_of(crate::game_logic::KindOf::FSFake);

        Self {
            thing: Thing::new(template),
            id,
            team,
            owner_player_id: None,
            team_instance_name: String::new(),
            name: String::new(),
            status: ObjectStatus::default(),
            object_status_bits: 0,
            script_unsellable: false,
            indestructible: false,

            eject_pilot_die_applied: false,
            model_condition_bits: 0,
            object_weather: 0,

            radar_extend_done_frame: 0,
            radar_extend_complete: false,
            radar_active: false,
            production_door_phase: 0,
            production_door_phase_end_frame: 0,
            production_door_hold_open: false,
            production_door_phases: [0; 4],
            production_door_phase_end_frames: [0; 4],
            production_door_active_index: 0,
            is_rebuild_hole: false,
            rebuild_template_name: None,
            rebuild_ready_frame: 0,
            rebuild_spawner_id: None,
            rebuild_worker_id: None,
            rebuild_reconstructing_id: None,
            producer_id: None,
            builder_id: None,
            dozer_task_build_target: None,
            dozer_task_build_order_frame: 0,
            dozer_task_repair_target: None,
            dozer_task_repair_order_frame: 0,
            dozer_dock_action: None,



            preferred_dock_id: None,
            supply_center_spawn_behavior_fired: false,
            supply_truck_state: SupplyTruckState::Idle,
            supply_truck_force_pending: false,
            supply_truck_next_dock_action_frame: 0,
            dock_active_docker: None,
            railed_in_transit: false,
            railed_waypoint_data_loaded: false,
            railed_current_path: crate::game_logic::RAILED_INVALID_PATH,
            railed_paths: Vec::new(),

            drawable_supply_boxes: dock_starting_boxes,
            drawable_supply_max_boxes: dock_starting_boxes,
            repair_dock_last_id: None,
            repair_dock_health_per_sec: 0.0,
            temporary_stealth_expires_frame: 0,

            highlander_body: false,
            upgrade_die: None,
            construction_complete_clear_frame: 0,
            sole_healing_benefactor: None,
            sole_healing_benefactor_expiration_frame: 0,
            idle_since_frame: 0,
            shock_stun_frames: 0,
            shock_yaw_rate: 0.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_allow_bounce: allow_bouncing,
            shock_was_airborne: false,
            shock_grounded_once: false,
            shock_up_z: 1.0,
            locomotor_surfaces: 0,
            cell_is_cliff: false,
            cell_is_underwater: false,
            kill_when_resting_on_ground,
            min_fall_speed_for_damage,
            fall_height_damage_factor,
            pending_ground_collide: false,
            immune_to_falling_damage: false,
            bounce_land_events: 0,
            last_bounce_fall_dy: 0.0,
            bounce_sound_name: String::new(),
            last_bounce_volume: 0.0,
            bounce_audio_pending: 0,
            crusher_level,
            crushable_level,
            has_squish_collide,
            topple_data: None,
            structure_topple_data: None,
            structure_collapse_data: None,
            keep_object_die: None,
            wave_guide_data: None,
            fire_weapon_when_dead_fired: false,
            bone_fx_damage: None,
            poisoned_behavior: None,
            defection_helper: None,
            fire_weapon_power: None,
            fire_weapon_when_damaged: None,
            temporary_weapon_runtime,
            pending_fire_when_damaged_weapon: None,
            transition_damage_fx: None,
            pending_transition_damage_fx: Vec::new(),
            fx_list_die: None,
            pending_death_fx: None,
            pending_death_audio: None,
            create_object_die: None,
            pending_create_object_die_spawns: Vec::new(),
            create_object_die_transfer_damage: 0.0,
            create_object_die_transfer_subdual: 0.0,
            create_object_die_transfer_source: None,
            pending_instant_death_weapon: None,
            crush_die: None,
            previous_health: 100.0,
            lifetime_update: None,
            slow_death: None,
            height_die: None,
            fuel_air_gas_slow_death: None,
            neutron_missile_update: None,
            scud_storm_missile_flight: None,
            carpet_bomb_payload: false,
            carpet_bomb_transport: None,
            artillery_barrage_shell: false,
            artillery_barrage_transport: None,
            a10_strike_missile: false,
            a10_strike_transport: None,
            leaflet_transport_target: None,
            leaflet_container: false,
            paradrop_transport_target: None,
            paradrop_parachute: false,
            daisy_cutter_transport: None,
            daisy_cutter_bomb: false,
            anthrax_bomb_transport: None,
            anthrax_bomb_payload: false,
            sneak_tunnel_start: false,
            cluster_mines_transport: None,
            cluster_mines_bomb: false,
            emp_pulse_transport: None,
            emp_pulse_bomb: false,
            emp_pulse_spheroid: false,
            emp_pulse_spheroid_expires_frame: None,
            particle_trail_remnant: false,
            particle_trail_remnant_expires_frame: None,
            nuke_radiation_field: false,
            nuke_radiation_field_expires_frame: None,
            anthrax_toxin_field: false,
            anthrax_toxin_field_expires_frame: None,
            spectre_howitzer_shell: false,
            spectre_howitzer_shell_expires_frame: None,
            particle_orbital_laser: false,
            particle_orbital_laser_expires_frame: None,
            particle_connector_laser: false,
            particle_connector_laser_expires_frame: None,
            point_defense_laser_beam: false,
            point_defense_laser_beam_expires_frame: None,
            missile_defender_laser_beam: false,
            missile_defender_laser_beam_expires_frame: None,
            booby_trap_special: false,
            booby_trap_attached_to: None,
            countermeasure_flare: false,
            countermeasure_flare_expires_frame: None,
            angry_mob_member: false,
            angry_mob_nexus_id: None,
            weapon_laser_beam: false,
            weapon_laser_beam_expires_frame: None,
            comanche_rocket_pod_projectile: false,
            comanche_rocket_pod_projectile_expires_frame: None,
            stealth_jet_missile_projectile: false,
            stealth_jet_missile_aim: None,
            stealth_jet_missile_intended: None,
            stealth_jet_missile_travelled: 0.0,
            stealth_jet_missile_fuel_expires_frame: None,
            stealth_jet_missile_ignition_frame: None,
            stealth_jet_missile_expires_frame: None,
            helix_napalm_bomb_projectile: false,
            scud_launcher_missile_projectile: false,
            scud_launcher_missile_toxin: false,
            scud_launcher_missile_aim: None,
            scud_launcher_missile_travelled: 0.0,
            scud_launcher_missile_fuel_expires_frame: None,
            tomahawk_missile_projectile: false,
            tomahawk_missile_aim: None,
            tomahawk_missile_travelled: 0.0,
            tomahawk_missile_fuel_expires_frame: None,
            aurora_bomb_projectile: false,
            aurora_bomb_aim: None,
            aurora_bomb_mission_id: None,
            rocket_buggy_missile_projectile: false,
            rocket_buggy_missile_aim: None,
            rocket_buggy_missile_intended: None,
            rocket_buggy_missile_travelled: 0.0,
            rocket_buggy_missile_fuel_expires_frame: None,
            neutron_cannon_shell_projectile: false,
            neutron_shell_from: None,
            neutron_shell_aim: None,
            neutron_shell_launch_frame: None,
            neutron_shell_flight_frames: 0,
            nuke_cannon_shell_projectile: false,
            nuke_shell_from: None,
            nuke_shell_aim: None,
            nuke_shell_launch_frame: None,
            nuke_shell_flight_frames: 0,
            usa_tank_shell_projectile: false,
            usa_tank_shell_from: None,
            usa_tank_shell_aim: None,
            usa_tank_shell_launch_frame: None,
            usa_tank_shell_flight_frames: 0,
            usa_tank_shell_weapon_speed: 0.0,
            usa_tank_shell_intended: None,
            battlemaster_shell_projectile: false,
            battlemaster_shell_from: None,
            battlemaster_shell_aim: None,
            battlemaster_shell_launch_frame: None,
            battlemaster_shell_flight_frames: 0,
            battlemaster_shell_intended: None,
            overlord_shell_projectile: false,
            overlord_shell_from: None,
            overlord_shell_aim: None,
            overlord_shell_launch_frame: None,
            overlord_shell_flight_frames: 0,
            overlord_shell_intended: None,
            inferno_shell_projectile: false,
            inferno_shell_from: None,
            inferno_shell_aim: None,
            inferno_shell_launch_frame: None,
            inferno_shell_flight_frames: 0,
            inferno_shell_intended: None,
            inferno_shell_upgraded: false,
            marauder_shell_projectile: false,
            marauder_shell_from: None,
            marauder_shell_aim: None,
            marauder_shell_launch_frame: None,
            marauder_shell_flight_frames: 0,
            marauder_shell_intended: None,
            marauder_shell_weapon_speed: 0.0,
            fire_base_shell_projectile: false,
            fire_base_shell_from: None,
            fire_base_shell_aim: None,
            fire_base_shell_launch_frame: None,
            fire_base_shell_flight_frames: 0,
            fire_base_shell_intended: None,
            raptor_missile_projectile: false,
            raptor_missile_aim: None,
            raptor_missile_intended: None,
            raptor_missile_travelled: 0.0,
            raptor_missile_fuel_expires_frame: None,
            raptor_missile_ignition_frame: None,
            mig_missile_projectile: false,
            mig_missile_aim: None,
            mig_missile_intended: None,
            mig_missile_travelled: 0.0,
            mig_missile_fuel_expires_frame: None,
            mig_missile_ignition_frame: None,
            flashbang_grenade_projectile: false,
            flashbang_grenade_from: None,
            flashbang_grenade_aim: None,
            flashbang_grenade_launch_frame: None,
            flashbang_grenade_flight_frames: 0,
            flashbang_grenade_intended: None,
            humvee_tow_projectile: false,
            humvee_tow_air: false,
            humvee_tow_aim: None,
            humvee_tow_intended: None,
            humvee_tow_travelled: 0.0,
            humvee_tow_fuel_expires_frame: None,
            humvee_tow_ignition_frame: None,
            dragon_flame_projectile: false,
            dragon_flame_aim: None,
            dragon_flame_intended: None,
            dragon_flame_travelled: 0.0,
            dragon_flame_fuel_expires_frame: None,
            dragon_flame_ignition_frame: None,
            dragon_flame_shooter: None,
            toxin_stream_projectile: false,
            toxin_stream_aim: None,
            toxin_stream_intended: None,
            toxin_stream_travelled: 0.0,
            toxin_stream_fuel_expires_frame: None,
            toxin_stream_ignition_frame: None,
            toxin_stream_shooter: None,
            technical_rpg_missile_projectile: false,
            technical_rpg_missile_aim: None,
            technical_rpg_missile_intended: None,
            technical_rpg_missile_travelled: 0.0,
            technical_rpg_missile_fuel_expires_frame: None,
            technical_rpg_missile_ignition_frame: None,
            technical_cannon_shell_projectile: false,
            technical_cannon_shell_from: None,
            technical_cannon_shell_aim: None,
            technical_cannon_shell_launch_frame: None,
            technical_cannon_shell_flight_frames: 0,
            technical_cannon_shell_intended: None,
            ecm_missile_jammed: false,
            cleanup_stream_projectile: false,
            cleanup_stream_aim: None,
            cleanup_stream_intended: None,
            cleanup_stream_travelled: 0.0,
            cleanup_stream_fuel_expires_frame: None,
            cleanup_stream_ignition_frame: None,
            cleanup_stream_shooter: None,
            cleanup_stream_player_id: 0,
            angry_mob_projectile: false,
            angry_mob_projectile_kind: 0,
            angry_mob_projectile_from: None,
            angry_mob_projectile_aim: None,
            angry_mob_projectile_launch_frame: None,
            angry_mob_projectile_flight_frames: 0,
            angry_mob_projectile_intended: None,
            inferno_fire_field: false,
            inferno_fire_field_upgraded: false,
            inferno_fire_field_expires_frame: None,
            inferno_fire_field_zone_id: None,
            rpg_trooper_missile_projectile: false,
            rpg_trooper_missile_aim: None,
            rpg_trooper_missile_intended: None,
            rpg_trooper_missile_travelled: 0.0,
            rpg_trooper_missile_fuel_expires_frame: None,
            tank_hunter_missile_projectile: false,
            tank_hunter_missile_aim: None,
            tank_hunter_missile_intended: None,
            tank_hunter_missile_travelled: 0.0,
            tank_hunter_missile_fuel_expires_frame: None,
            missile_defender_missile_projectile: false,
            missile_defender_missile_aim: None,
            missile_defender_missile_intended: None,
            missile_defender_missile_travelled: 0.0,
            missile_defender_missile_fuel_expires_frame: None,
            missile_defender_missile_laser_slot: false,
            scorpion_shell_projectile: false,
            scorpion_shell_from: None,
            scorpion_shell_aim: None,
            scorpion_shell_launch_frame: None,
            scorpion_shell_flight_frames: 0,
            scorpion_shell_slot: 0,
            scorpion_missile_projectile: false,
            scorpion_missile_aim: None,
            scorpion_missile_intended: None,
            scorpion_missile_travelled: 0.0,
            scorpion_missile_fuel_expires_frame: None,
            scorpion_missile_slot: 0,
            airfield_rearm_ready_frame: None,
            jet_producer_location: None,
            jet_circling_dead_airfield: false,
            jet_circling_airfield_check_frame: 0,
            airfield_rearm_duration_frames: 0,
            return_to_base_requested: false,
            jet_ai: HostJetAi::default(),

            airfield_parking_space_index: None,
            frenzy_invisible_marker: false,
            ambush_fade_in: false,
            gps_scrambler_marker: false,
            emergency_repair_marker: false,
            spy_satellite_ping: false,
            spy_satellite_ping_expires_frame: None,
            radar_van_ping: false,
            radar_van_ping_expires_frame: None,
            firewall_segment: false,
            firewall_segment_expires_frame: None,
            firewall_segment_wall_id: None,
            firewall_segment_dir: None,
            tensile_formation: None,
            fire_spread: None,
            base_regenerate: None,
            default_auto_heal: None,
            enemy_near: None,

            animation_steering: None,
            float_update: None,
            prone_update: None,
            radius_decal_update: None,
            checkpoint_update: None,
            spectre_gunship_deployment: None,
            smart_bomb_target_homing: None,
            helicopter_slow_death: None,
            jet_slow_death: None,
            front_crushed: false,
            back_crushed: false,
            physics_current_overlap: None,
            physics_previous_overlap: None,
            ignore_collisions_with: None,
            last_collidee: None,
            allow_collide_force,
            can_path_through_units: false,
            ignore_collisions_until_frame: 0,
            is_blocked: false,
            is_blocked_and_stuck: false,
            cur_max_blocked_speed: f32::MAX,
            num_frames_blocked: 0,
            bump_speed_limit: f32::MAX,
            locomotor_set_names,
            cur_locomotor_name: locomotor_name,
            is_panicking: false,
            physics_mass,
            shock_resistance,
            physics_accel: glam::Vec3::ZERO,
            motive_frames_remaining: 0,
            waiting_for_path: false,
            move_away_from: None,
            move_away_frames: 0,
            move_away_destination: None,
            request_other_move_away: None,
            forward_friction,
            lateral_friction,
            z_friction,
            aerodynamic_friction,
            extra_friction: 0.0,
            apply_friction_2d_when_airborne: false,
            velocity_magnitude_cache: -1.0,
            original_allow_bounce: allow_bouncing,
            stick_to_ground: false,
            allow_to_fall: false,
            was_airborne_last_frame: false,
            center_of_mass_offset,
            pitch_roll_yaw_factor,
            is_braking: false,
            braking_factor: 1.0,
            braking: 99999.0,
            loco_apply_2d_friction_airborne: false,
            allow_motive_force_while_airborne: false,
            loco_extra_2d_friction: 0.0,
            physics_turning: PhysicsTurningType::TurnNone,
            loco_behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            loco_preferred_height: 0.0,
            loco_preferred_height_damping: 1.0,
            maintain_pos_valid: false,
            maintain_pos: None,
            loco_appearance: LocomotorAppearance::Other,
            min_turn_speed: 0.0,
            min_speed: 0.0,
            ultra_accurate_slide_factor: 0.0,
            ultra_accurate: false,
            can_move_backward: false,
            moving_backwards: false,
            no_slow_down_as_approaching_dest: false,
            over_water: false,
            circling_radius: 0.0,
            precise_z_pos: false,
            is_dozer: false,
            on_invalid_movement_terrain: false,
            turn_pivot_offset: 0.0,
            wander_width_factor: 0.0,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            wander_offset_increasing: true,
            downhill_only: false,
            is_climbing: false,
            donut_timer: 0,
            max_lift: 0.0,
            max_lift_damaged: 0.0,
            speed_limit_z: 999999.0,
            group_speed_factor: 1.0,
            is_attack_path: false,
            is_exact_path: false,
            is_approach_path: false,
            is_safe_path: false,
            requested_victim_id: None,
            requested_destination: None,
            path_timestamp: 0,
            queue_for_path_frames: 0,
            max_shots_to_fire: -1,
            attack_substate: crate::game_logic::AttackSubState::AimAtTarget,
            approach_timestamp: 0,
            prev_victim_pos: None,
            temporary_move_frames: 0,
            body_damage_state:
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
            move_loop_audio: None,
            ambient_audio: None,
            health: Health::new(100.0),
            movement: Movement::default(),
            experience: Experience::default(),
            experience_sink: None,
            experience_scalar: 1.0,
            weapon: None,
            mine_clearing_primary_weapon: None,
            secondary_weapon: None,
            tertiary_weapon: None,
            target: None,
            capture_channel: None,
            hacker_disable_channel: None,
            charge_plant_unpack_remaining_seconds: None,

            construction_percent: 1.0,
            building_data: None,
            stored_resources: Resources::default(),
            power_provided: 0,
            power_consumed: 0,
            selected: false,
            selection_flash_remaining: 0,
            ai_state: AIState::Idle,
            object_type,
            template_name: template_name.clone(),
            position: Vec3::ZERO,
            max_health: 100.0,
            target_location: None,
            guard_position: None,
            guard_retaliate_victim: None,
            guard_retaliate_anchor: None,
            crate_created: None,
            hunting: false,
            hijack_vehicle_id: None,
            hijacker_in_vehicle: false,
            hijacker_update_active: false,
            hijacker_was_airborne: false,
            hijacker_eject_pos: None,
            drawable_hidden: false,

            weapon_crate_upgrade: 0,
            armor_crate_upgrade: 0,
            guard_target: None,
            guard_chase_phase: 0,
            guard_chase_give_up_frame: 0,

            force_attack: false,
            show_health_bar: true,
            selection_radius,
            ground_height: 0.0,
            ground_height_from_terrain: false,
            team_color: team.get_color(),
            occupants: Vec::new(),
            player_who_entered: String::new(),

            max_transport: 0,
            overlord_bunker_capacity: None,
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            weapon_bonus_player_upgrade: false,
            armor_set_player_upgrade: false,
            armor_set_veteran: false,
            armor_set_elite: false,
            armor_set_hero: false,
            weapon_set_veteran: false,
            weapon_set_elite: false,
            weapon_set_hero: false,
            weapon_bonus_veteran: false,
            weapon_bonus_elite: false,
            weapon_bonus_hero: false,
            locomotor_upgrade: false,
            terrain_decal_chemsuit: false,
            terrain_decal_type: if is_fs_fake {
                crate::game_logic::host_battlemaster::TERRAIN_DECAL_SHADOW_TEXTURE
            } else {
                8
            },
            terrain_decal_size: 0.0,
            terrain_decal_fade_target: 0.0,
            terrain_decal_fade_rate: 0.0,
            terrain_decal_opacity: if is_fs_fake {
                1.0
            } else {
                0.0
            },
            sub_object_visibility: crate::game_logic::host_battlemaster::leftover_horde_flag_visibility_for_template(
                &template_name,
            ),
            special_power_completion: None,
            power_plant_rods_extended: false,
            power_plant_rods_done_frame: 0,
            special_power_paused: std::collections::HashMap::new(),
            weapon_set_mine_clearing_detail: false,
            weapon_set_carbomb: false,
            weapon_set_vehicle_hijack: false,
            is_battle_bus_transport: false,
            battle_bus_body: None,
            armor_set_second_life: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            rider_change_active_slot: None,
            rider_change_model_condition_mask: 0,
            rider_change_object_status_mask: 0,
            rider_change_weapon_set: None,
            rider_change_locomotor_set: None,
            rider_change_locomotor_name: None,
            rider_change_scuttled_on_frame: 0,
            is_tunnel_network: false,
            cave_index: 0,
            is_cave_contain: false,
            is_combat_chinook_transport: false,
            chinook_ai: None,
            contained_by: None,
            cheer_timer: 0.0,
            prone_timer: 0.0,
            emoticon_name: String::new(),
            emoticon_frames_left: 0,
            is_surrendered: false,
            formation_id: 0,
            formation_offset: glam::Vec2::ZERO,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            weapon_lock_type: WeaponLockType::NotLocked,
            weapon_lock_slot: 0,
            weapon_fire_status: WeaponFireStatus::ReadyToFire,
            fire_sound_loop_until_frame: 0,
            fire_sound_loop_name: String::new(),
            weapon_barrel_states: std::array::from_fn(|_| WeaponBarrelState::default()),
            weapon_scatter_targets_unused: std::array::from_fn(|_| Vec::new()),
            weapon_scatter_targets_inited: [false; 3],
            pre_attack_target: None,
            pre_attack_ready_at: 0.0,
            consecutive_shot_target: None,
            consecutive_shots_at_target: 0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            last_fire_victim_host: 0,
            last_fire_slot: 0,
            last_fire_damage: 0.0,
            last_fire_range: 0.0,
            last_fire_sim_time: 0.0,
            last_fire_frame: 0,
            fire_intent_count: 0,
            last_weapon_discharge_sequence: 0,
            last_weapon_discharge_slot: 0,
            last_weapon_discharge_barrel: 0,
            last_weapon_discharge_frame: 0,
            visual_object_generation: 0,
            visual_draw_state_revision: 1,
            pending_weapon_visual_capture: None,
            guard_radius: 0.0,
            guard_mode: GuardMode::Normal,
            pending_evacuate_on_stop: false,
            pending_exit_after_evacuate: false,
            pending_stream_exit: false,

            frame_exit_not_busy: 0,
            which_exit_path: 0,

            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown: 10.0,
            special_power_cooldown_remaining: 0.0,
            special_power_cooldowns: HashMap::new(),
            special_power_override_destination: None,
            special_power_override_type: None,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            next_detection_scan_frame: 0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            disguise_as_template: None,
            disguise_pending_template: None,
            disguise_pending_team: None,
            disguise_as_team: None,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_fanaticism: false,
            last_horde_refresh_frame: 0,
            horde_next_wake_frame: 0,
            horde_wake_initialized: false,

            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_until_frame: 0,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            weapon_bonus_drone_spotting: false,
            battle_plan_sight_scalar_applied: 1.0,
            continuous_fire_consecutive: 0,
            continuous_fire_level: 0,
            continuous_fire_one_shots: tracker.continuous_fire_one_shots,
            continuous_fire_two_shots: tracker.continuous_fire_two_shots,
            continuous_fire_coast_frames: tracker.continuous_fire_coast_frames,
            auto_reload_when_idle_frames: tracker.auto_reload_when_idle_frames,
            frame_to_force_reload: 0,
            continuous_fire_coast_until_frame: 0,
            fire_ocl_after_cooldown: None,
            continuous_fire_victim: 0,
            faerie_fire_until_frame: 0,
            subdual_damage: 0.0,
            subdual_damage_cap: 0.0,
            subdual_heal_rate_frames: 0,
            subdual_heal_amount: 0.0,
            subdual_heal_countdown: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_pathfinder_unit: false,
            is_troop_crawler_transport: false,
            assault_transport: None,
            deploy_style: None,
            command_button_hunt: None,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            is_helix_transport: false,
            command_set_override: None,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            hive_slave_respawn_frame: 0,
            hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3],
            turret_angle_deg: turret.angle_deg,
            turret_pitch_deg: turret.pitch_deg,
            turret_idle_scan_next_frame: 0,
            turret_idle_scanning: false,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_holding: false,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_mood_target: false,
            turret_target_id: None,
            turret_force_attacking: false,
            turret_enabled: turret.enabled,
            turret_turn_rate_rad: turret.turn_rate_rad,
            turret_substate: TurretSubState::Idle,
            turret_rotating: false,
            turret_natural_angle_deg: turret.natural_angle_deg,
            turret_natural_pitch_deg: turret.natural_pitch_deg,
            turret_recenter_frames: turret.recenter_frames,
            ai_attitude: 0, // HostAiAttitude::Normal
            repulsor_until_frame: 0,
            last_damage_source: None,
            last_damage_timestamp: None,
            last_healing_timestamp: None,
            last_damage_fx_done: None,
            next_damage_fx_time: 0,
            last_damage_source_preferred: false,
            health_box_offset: [0.0, 0.0, 0.0],
            uses_inactive_body: false,
            inactive_body_die_called: false,
            next_mood_check_time: 0,
            mood_attack_check_rate: default_mood_attack_check_rate(),
            vision_range: default_vision_range(),
            shroud_clearing_range: default_vision_range(),
            shroud_range: 0.0,
            partition_cash_value: 0,
            partition_threat_value: 0,
            partition_last_affect: None,
            partition_last_look: None,


            auto_acquire_when_idle: leftover_auto_acquire_idle_yes(&template_name),
            auto_acquire_idle_bits: leftover_auto_acquire_idle_bits(&template_name),
            attack_priority_set: None,
            camo_friendly_opacity: 1.0,
            camo_opacity_pulse_phase: 0.0,
            camo_stealth_look: 0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
            drawable_fade_mode: 0,
            drawable_fade_start_frame: 0,
            drawable_fade_frames: 0,
            drawable_explicit_opacity: 1.0,
            drawable_instance_scale: 1.0,
            drawable_tint_status: 0,
            drawable_prev_tint_status: 0,
            drawable_expiration_date: 0,
            drawable_loco_pitch: 0.0,
            drawable_loco_pitch_rate: 0.0,
            drawable_loco_roll: 0.0,
            drawable_loco_roll_rate: 0.0,
            drawable_loco_yaw: 0.0,
            drawable_loco_accel_pitch: 0.0,
            drawable_loco_accel_pitch_rate: 0.0,
            drawable_loco_accel_roll: 0.0,
            drawable_loco_accel_roll_rate: 0.0,
            drawable_overlay_icons: Vec::new(),
        }
    }

    pub fn new_under_construction(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let mut obj = Self::new(template, id, team);
        obj.construction_percent = 0.0;
        obj.set_status_under_construction(true);
        // C++ DozerAIUpdate::construct (DozerAIUpdate.cpp:1706-1708):
        // newly constructed objects start at one hit point.
        obj.health.current = 1.0;

        obj
    }

    pub fn get_template(&self) -> &ThingTemplate {
        self.thing.get_template()
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.thing.is_kind_of(kind)
    }

    /// C++ PoisonedBehavior::onDamage residual.
    pub fn notify_poisoned_on_damage(
        &mut self,
        current_frame: u32,
        damage_type: crate::game_logic::combat::DamageType,
        damage_dealt: f32,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) {
        use crate::game_logic::host_poisoned_behavior::{
            is_poison_damage_type, HostPoisonedBehaviorData,
        };
        if !is_poison_damage_type(damage_type) || damage_dealt <= 0.0 {
            return;
        }
        if self.poisoned_behavior.is_none() {
            self.poisoned_behavior = Some(HostPoisonedBehaviorData::default());
        }
        if let Some(p) = self.poisoned_behavior.as_mut() {
            p.start_poisoned_effects(current_frame, damage_dealt, death_type);
        }
    }

    /// C++ PoisonedBehavior::onHealing residual.
    pub fn clear_poisoned_on_healing(&mut self) {
        if let Some(p) = self.poisoned_behavior.as_mut() {
            p.stop_poisoned_effects();
        }
    }

    /// C++ PoisonedBehavior::update residual. Returns DoT damage to apply.
    pub fn tick_poisoned_behavior(
        &mut self,
        current_frame: u32,
    ) -> Option<(f32, crate::game_logic::host_usa_pilot::HostDeathType)> {
        let alive = !self.status.destroyed
            && !self.status.effectively_dead
            && !self.status.keep_as_rubble
            && self.health.is_alive();
        let Some(p) = self.poisoned_behavior.as_mut() else {
            return None;
        };
        let dmg = p.tick(current_frame);
        if p.should_stop(current_frame) && alive {
            p.stop_poisoned_effects();
        }
        dmg
    }

    /// Presentation: poisoned tint residual.
    pub fn is_poison_tinted(&self) -> bool {
        self.poisoned_behavior
            .as_ref()
            .map(|p| p.tint_poisoned)
            .unwrap_or(false)
    }

    /// C++ Object::defect(team, detectionFrames) residual.
    pub fn defect(&mut self, new_team: Team, now: u32, detection_frames: u32) {
        self.set_team(new_team);
        self.begin_undetected_defection(now, detection_frames, true);
    }

    /// C++ Object::defect / friend_setUndetectedDefector + DefectionHelper timer.
    pub fn begin_undetected_defection(&mut self, now: u32, protection_frames: u32, with_fx: bool) {
        if self.defection_helper.is_none() {
            self.defection_helper =
                Some(crate::game_logic::host_defection_helper::HostDefectionHelperData::default());
        }
        if let Some(d) = self.defection_helper.as_mut() {
            crate::game_logic::host_defection_helper::defect_team_residual(
                d,
                now,
                protection_frames,
                with_fx,
            );
        }
    }

    pub fn is_undetected_defector(&self) -> bool {
        self.defection_helper
            .as_ref()
            .map(|d| d.is_undetected_defector())
            .unwrap_or(false)
    }

    pub fn blow_defector_cover(&mut self) {
        if let Some(d) = self.defection_helper.as_mut() {
            d.blow_cover();
        }
    }

    /// C++ ObjectDefectionHelper::update residual.
    pub fn tick_defection_helper(&mut self, now: u32) {
        let firing = self.status.is_firing_weapon;
        let dead =
            self.status.destroyed || self.status.effectively_dead || self.health.current <= 0.0;
        if let Some(d) = self.defection_helper.as_mut() {
            d.tick(now, firing, dead);
        }
    }

    /// C++ FireWeaponPower::doSpecialPower residual.
    pub fn activate_fire_weapon_power(&mut self, target: Option<(f32, f32)>) -> bool {
        if self.is_disabled() {
            return false;
        }
        let shots =
            crate::game_logic::host_fire_weapon_power::max_shots_for_template(&self.template_name);
        self.fire_weapon_power = Some(match target {
            Some((x, z)) => {
                crate::game_logic::host_fire_weapon_power::HostFireWeaponPowerRequest::at_location(
                    shots, x, z,
                )
            }
            None => crate::game_logic::host_fire_weapon_power::HostFireWeaponPowerRequest::at_self(
                shots,
            ),
        });
        // C++ reloadAllAmmo(TRUE) residual.
        self.reload_all_ammo();
        true
    }

    pub fn is_alive(&self) -> bool {
        if self.status.destroyed || self.status.effectively_dead || self.status.keep_as_rubble {
            return false;
        }
        // C++ Object::isEffectivelyDead is a status bit; HP comes from the
        // BodyModule (single store). When GameWorld is coupled, HashMap
        // health.current can lag writeback — use the mapped entity HP.
        let hp =
            crate::gameworld_shadow::coupled_entity_health(self.id).unwrap_or(self.health.current);
        if hp <= 0.0 {
            return false;
        }
        // C++ effectively-dead during SlowDeath / air crash sequences.
        if self
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            return false;
        }
        if self
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active())
            .unwrap_or(false)
        {
            return false;
        }
        if self
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            return false;
        }
        true
    }

    pub fn get_health_percentage(&self) -> f32 {
        // C++ BodyModule::getHealth() / getMaxHealth() — GW when coupled.
        let current =
            crate::gameworld_shadow::coupled_entity_health(self.id).unwrap_or(self.health.current);
        if self.health.maximum > 0.0 {
            current / self.health.maximum
        } else {
            0.0
        }
    }

    pub fn is_constructed(&self) -> bool {
        if let Some((pct, uc)) = crate::gameworld_shadow::coupled_entity_construction(self.id) {
            return !uc || pct + 1e-6 >= 1.0;
        }
        !self.status.under_construction && self.construction_percent >= 1.0
    }

    pub fn is_mobile(&self) -> bool {
        // C++-ish: infantry/vehicle/aircraft, plus Worker KindOf.
        // Do NOT call can_construct() here — that path can re-enter is_mobile.
        // Host dozer residual: treat non-structure templates named *Dozer* as mobile.
        if self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
            || self.is_kind_of(KindOf::Worker)
            || self.is_railed_transport()
        {
            return true;
        }

        if !self.is_kind_of(KindOf::Structure) {
            let name = self.template_name.to_ascii_lowercase();
            if name.contains("dozer") || name.contains("worker") || name.contains("construction") {
                return true;
            }
        }
        false
    }

    pub fn is_selectable(&self) -> bool {
        // C++ Object.cpp:3011-3020 — ALWAYS_SELECTABLE first; then m_isSelectable
        // (Selectable kind) + !OBJECT_STATUS_UNSELECTABLE + !effectivelyDead.
        // No contained / MASKED / hijacker / Docked / Garrisoned gate.
        if self.is_kind_of(KindOf::AlwaysSelectable) {
            return true;
        }
        self.is_alive()
            && self.is_kind_of(KindOf::Selectable)
            && !self.status.unselectable
    }

    pub fn is_worker(&self) -> bool {
        self.is_kind_of(KindOf::Worker)
            || self.is_kind_of(KindOf::Dozer)
            || self.template_name.contains("Dozer")
            || self.template_name.contains("Worker")
            || self.template_name.contains("Harvester")
            || self.template_name.contains("Collector")
    }

    /// C++ `KINDOF_HARVESTER`: semantic permission to collect supplies.
    ///
    /// Builders (`DOZER`) and the host's legacy `Worker` capability remain
    /// separate because neither one authorizes a resource Gather order on its
    /// own.  This intentionally avoids template-name classification on the
    /// live Gather path.
    #[inline]
    pub fn is_resource_collector(&self) -> bool {
        self.is_kind_of(KindOf::Harvester)
    }

    pub fn is_hero(&self) -> bool {
        self.is_kind_of(KindOf::Hero) || self.template_name.contains("Hero")
    }

    pub fn is_command_center(&self) -> bool {
        self.is_kind_of(KindOf::CommandCenter)
            || self.template_name.contains("CommandCenter")
            || self.template_name.contains("Headquarters")
    }

    pub fn is_faction_structure(&self) -> bool {
        self.is_kind_of(KindOf::FSBarracks)
            || self.is_kind_of(KindOf::FSWarFactory)
            || self.is_kind_of(KindOf::FSAirfield)
            || self.is_kind_of(KindOf::FSInternetCenter)
            || self.is_kind_of(KindOf::FSPower)
            || self.is_kind_of(KindOf::FSBaseDefense)
            || self.is_kind_of(KindOf::FSSupplyDropzone)
            || self.is_kind_of(KindOf::FSSupplyCenter)
            || self.is_kind_of(KindOf::FSSuperweapon)
            || self.is_kind_of(KindOf::FSStrategyCenter)
            || self.is_kind_of(KindOf::FSFake)
            || self.is_kind_of(KindOf::FSTechnology)
            || self.is_kind_of(KindOf::FSBlackMarket)
            || self.is_kind_of(KindOf::FSAdvancedTech)
            || self.is_command_center()
            || self.is_kind_of(KindOf::SupplyCenter)
            || self.is_kind_of(KindOf::PowerPlant)
            || self.template_name.contains("Barracks")
            || self.template_name.contains("WarFactory")
            || self.template_name.contains("Airfield")
            || self.template_name.contains("InternetCenter")
            || self.template_name.contains("PowerPlant")
            || self.template_name.contains("SupplyDropzone")
            || self.template_name.contains("SupplyCenter")
            || self.template_name.contains("Superweapon")
            || self.template_name.contains("StrategyCenter")
            || self.template_name.contains("BlackMarket")
            || self.template_name.contains("TechCenter")
    }

    pub fn is_non_faction_structure(&self) -> bool {
        self.is_kind_of(KindOf::Structure) && !self.is_faction_structure()
    }

    /// C++ parity (Object::isDisabled): returns true if the object is in any
    /// disabled state that prevents it from acting (attacking, producing, etc.)
    ///
    /// Note: `weapons_jammed` (ECM residual) is intentionally **not** full
    /// disabled — C++ DISABLED_SUBDUED on vehicles only blocks `canFireWeapon`;
    /// residual keeps movement. Check `is_weapons_jammed()` / `can_attack()` for fire.
    /// Structure `disabled_subdued` (Microwave residual) **is** full disable.
    pub fn is_disabled(&self) -> bool {
        self.status.disabled_underpowered
            || self.status.disabled_unmanned
            || self.status.disabled_hacked
            || self.status.disabled_emp
            || self.status.disabled_paralyzed
            || self.status.disabled_subdued
            || self.status.disabled_freefall
            || self.status.disabled_default
            || self.status.disabled_script_disabled
            || self.status.disabled_script_underpowered
            || self.status.disabled_held
            || self.status.under_construction
    }

    /// C++ KINDOF_SPAWNS_ARE_THE_WEAPONS residual (Stinger Site / hive).
    pub fn is_spawns_are_the_weapons(&self) -> bool {
        self.hive_slave_count > 0
            || self.hive_slaves.iter().any(|s| s.alive)
            || crate::game_logic::host_base_defense::is_stinger_site_structure(&self.template_name)
    }


    /// C++ DISABLED_FREEFALL residual.
    pub fn is_freefall_disabled(&self) -> bool {
        self.status.disabled_freefall
    }

    /// C++ DISABLED_UNMANNED residual (Jarmen Kell kill-pilot snipe).
    pub fn is_unmanned(&self) -> bool {
        self.status.disabled_unmanned
    }

    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack).
    pub fn is_hacked_disabled(&self) -> bool {
        self.status.disabled_hacked
    }

    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse).
    pub fn is_emp_disabled(&self) -> bool {
        self.status.disabled_emp
    }

    /// C++ PUC live-beam abort: UNDERPOWERED / EMP / SUBDUED / HACKED.
    pub fn puc_live_beam_abort_disabled(&self) -> bool {
        crate::game_logic::special_power_strikes::puc_disabled_aborts_live_beam(
            self.status.disabled_underpowered,
            self.status.disabled_emp,
            self.status.disabled_subdued,
            self.status.disabled_hacked,
        )
    }

    /// C++ DISABLED_PARALYZED residual (BattlePlanChangeParalyzeTime).
    pub fn is_paralyzed_disabled(&self) -> bool {
        self.status.disabled_paralyzed
    }

    /// Host ECM / jammer residual: weapons cannot fire while in jam radius.
    /// C++ DISABLED_SUBDUED / canFireWeapon residual (Microwave/ECM disabler).
    pub fn is_weapons_jammed(&self) -> bool {
        self.status.weapons_jammed
    }

    /// C++ DISABLED_SUBDUED residual (Microwave building disabler on structures).
    pub fn is_subdued_disabled(&self) -> bool {
        self.status.disabled_subdued
    }

    /// Apply / clear weapons-jam residual (ECM field coverage).
    pub fn set_weapons_jammed(&mut self, jammed: bool) {
        if jammed {
            self.set_status_weapons_jammed(true);
            // C++ canFireWeapon false while subdued: drop in-progress attack fire
            // but do not freeze movement (jam residual is weapons-only).
            self.status.attacking = false;
            self.set_status_force_attack(false);
        } else {
            self.set_status_weapons_jammed(false);
        }
    }

    // Apply / clear DISABLED_SUBDUED residual (Microwave structure cook).
    //
    // C++ ActiveBody::onSubdualChange → setDisabled(DISABLED_SUBDUED).
    // Structures stop production / attack while cooked; residual continuous
    // while microwave keeps attacking (not full subdual accumulate/heal).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, ObjectId, Team, ThingTemplate};

    #[test]
    fn always_selectable_is_selectable_even_when_dead() {
        // C++ Object.cpp:3011 — KINDOF_ALWAYS_SELECTABLE returns TRUE first.
        let mut template = ThingTemplate::new("UIBeacon");
        template.add_kind_of(KindOf::AlwaysSelectable);
        let mut obj = Object::new(template, ObjectId(1), Team::USA);
        obj.status.destroyed = true;
        obj.status.effectively_dead = true;
        assert!(!obj.is_alive());
        assert!(obj.is_selectable());
    }

    #[test]
    fn dead_selectable_without_always_selectable_is_not_selectable() {
        let mut template = ThingTemplate::new("Ranger");
        template.add_kind_of(KindOf::Selectable);
        let mut obj = Object::new(template, ObjectId(2), Team::USA);
        obj.status.destroyed = true;
        assert!(!obj.is_selectable());
    }

    #[test]
    fn is_selectable_keeps_contained_masked_and_hijacker_like_cpp() {
        // C++ Object.cpp:3001-3020 — no contained / MASKED / hijacker gate.
        use crate::game_logic::AIState;
        let mut template = ThingTemplate::new("Ranger");
        template.add_kind_of(KindOf::Selectable);
        let mut obj = Object::new(template, ObjectId(3), Team::USA);
        obj.set_contained_by(Some(ObjectId(99)));
        obj.set_ai_state(AIState::Garrisoned);
        obj.status.masked = true;
        obj.hijacker_in_vehicle = true;
        assert!(
            obj.is_selectable(),
            "garrisoned/transported/masked squad members stay isSelectable"
        );
        obj.status.unselectable = true;
        assert!(
            !obj.is_selectable(),
            "OBJECT_STATUS_UNSELECTABLE still drops isSelectable"
        );
        obj.set_ai_state(AIState::Docked);
        obj.status.unselectable = false;
        assert!(obj.is_selectable());
    }

    #[test]
    fn turret_only_spawns_on_units_that_author_one() {
        use crate::game_logic::host_strategy_center::{
            STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG, STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
            STRATEGY_CENTER_TURRET_TURN_RATE_DEG_PER_SEC,
        };
        use crate::game_logic::host_usa_tanks::USA_TANK_TURRET_TURN_RATE;

        let ranger = Object::new(ThingTemplate::new("AmericaInfantryRanger"), ObjectId(1), Team::USA);
        assert!(!ranger.turret_enabled);
        assert!((ranger.turret_angle_deg).abs() < 0.01);
        assert!((ranger.turret_pitch_deg).abs() < 0.01);

        let dozer = Object::new(ThingTemplate::new("AmericaVehicleDozer"), ObjectId(2), Team::USA);
        assert!(!dozer.turret_enabled);

        let crusader = Object::new(ThingTemplate::new("AmericaTankCrusader"), ObjectId(3), Team::USA);
        assert!(crusader.turret_enabled);
        assert!((crusader.turret_angle_deg).abs() < 0.01);
        assert!((crusader.turret_pitch_deg).abs() < 0.01);
        assert!(
            (crusader.turret_turn_rate_rad
                - turret_deg_per_sec_to_rad_per_frame(USA_TANK_TURRET_TURN_RATE))
                .abs()
                < 1e-5
        );

        let sc = Object::new(
            ThingTemplate::new("AmericaStrategyCenter"),
            ObjectId(4),
            Team::USA,
        );
        assert!(!sc.turret_enabled, "InitiallyDisabled until Bombardment");
        assert!((sc.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() < 0.01);
        assert!((sc.turret_pitch_deg - STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG).abs() < 0.01);
        assert!(
            (sc.turret_turn_rate_rad
                - turret_deg_per_sec_to_rad_per_frame(STRATEGY_CENTER_TURRET_TURN_RATE_DEG_PER_SEC))
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn humvee_authors_recenter_time_not_idle_scan() {
        use crate::game_logic::host_humvee::{
            HUMVEE_TURRET_RECENTER_FRAMES, HUMVEE_TURRET_TURN_RATE,
        };
        let spec = turret_spawn_for_template("AmericaVehicleHumvee");
        assert!(spec.has_turret);
        assert!(spec.enabled);
        assert_eq!(spec.recenter_frames, HUMVEE_TURRET_RECENTER_FRAMES);
        assert!((spec.max_idle_scan_angle_rad).abs() < 1e-6);
        assert!(
            (spec.turn_rate_rad - turret_deg_per_sec_to_rad_per_frame(HUMVEE_TURRET_TURN_RATE))
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn construct_defaults_auto_acquire_idle_bits_off() {
        let obj = Object::new(ThingTemplate::new("NoAcquireUnit"), ObjectId(9), Team::USA);
        assert_eq!(obj.auto_acquire_idle_bits, 0);
        assert!(!obj.auto_acquire_when_idle);
    }
}

use super::*;

impl PresentationFrame {
    /// Freeze the only clip record currently owned by a coupled GameWorld
    /// entity.  Its secondary/tertiary weapon state is not represented in the
    /// GameWorld entity, so returning host mirrors here would make the
    /// GameWorld overlay mix authority domains; leave those slots unset.
    fn projectile_clip_statuses_from_gameworld_entity(
        ent: &gamelogic::world::entities::Entity,
    ) -> [Option<PresentationProjectileClipStatus>; 3] {
        let shots_remaining = if ent.active_weapon_slot == 0
            && ent.weapon_fire_status
                == crate::game_logic::object::WeaponFireStatus::ReloadingClip as u8
        {
            0
        } else {
            ent.weapon_ammo
        };
        let primary = (ent.weapon_ammo != u32::MAX
            && ent.weapon_clip_size > 0
            && shots_remaining <= ent.weapon_clip_size)
            .then_some(PresentationProjectileClipStatus {
                shots_remaining,
                max_shots: ent.weapon_clip_size,
            });
        [primary, None, None]
    }

    /// Overlay health/position/destroyed from a GameWorld shadow session.
    ///
    /// Host still builds the frame (templates, FOW, selection); shadow is last
    /// writer for HP and world position when authority paths are active.
    /// Unmapped objects are left unchanged.
    pub fn overlay_gameworld_shadow(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        let mut updated = 0usize;
        for obj in &mut self.objects {
            let Some(eid) = shadow.entity_for_host(obj.id) else {
                continue;
            };
            let Some(ent) = shadow.world().entity(eid) else {
                // Destroyed on shadow — mark destroyed for presentation.
                if !obj.destroyed {
                    obj.destroyed = true;
                    obj.health_current = 0.0;
                    updated += 1;
                }
                continue;
            };
            let pos = glam::Vec3::new(
                ent.transform.position.x,
                ent.transform.position.y,
                ent.transform.position.z,
            );
            let h = ent.health.max(0.0);
            let destroyed = h <= 0.0 || ent.destroyed;
            // Always apply shadow last-writer residual for presentation identity.
            let mut dirty = false;
            if (obj.position - pos).length_squared() > 1e-6 {
                obj.position = pos;
                dirty = true;
            }
            if (obj.orientation - ent.transform.orientation).abs() > 1e-5 {
                obj.orientation = ent.transform.orientation;
                dirty = true;
            }
            let move_dest = ent.move_target.map(|d| glam::Vec3::new(d[0], d[1], d[2]));
            if obj.move_destination != move_dest {
                obj.move_destination = move_dest;
                dirty = true;
            }
            let rally = ent.rally_point.map(|d| glam::Vec3::new(d[0], d[1], d[2]));
            if obj.rally_point != rally {
                obj.rally_point = rally;
                dirty = true;
            }
            let atk = ent
                .attack_target
                .and_then(|tid| shadow.host_for_entity(tid));
            if obj.attack_target != atk {
                obj.attack_target = atk;
                dirty = true;
            }
            if (obj.health_current - h).abs() > 1e-3 {
                obj.health_current = h;
                dirty = true;
            }
            if (obj.health_max - ent.max_health).abs() > 1e-3 && ent.max_health > 0.0 {
                obj.health_max = ent.max_health;
                dirty = true;
            }
            if obj.destroyed != destroyed {
                obj.destroyed = destroyed;
                dirty = true;
            }
            // Wave 189: expand last-writer overlay for motion/selection/body identity.
            let vel = glam::Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
            if (obj.velocity - vel).length_squared() > 1e-6 {
                obj.velocity = vel;
                dirty = true;
            }
            if (obj.move_max_speed - ent.move_max_speed).abs() > 1e-4 && ent.move_max_speed >= 0.0 {
                obj.move_max_speed = ent.move_max_speed;
                dirty = true;
            }
            if obj.selected != ent.selected {
                obj.selected = ent.selected;
                dirty = true;
            }
            if obj.team_color != ent.team_color {
                obj.team_color = ent.team_color;
                dirty = true;
            }
            if obj.body_damage_state != ent.body_damage_state {
                obj.body_damage_state = ent.body_damage_state;
                dirty = true;
            }
            // Identity residual (template/team/type) from shadow — no live dual-read.
            if obj.template_name != ent.template.name {
                obj.template_name = ent.template.name.clone();
                dirty = true;
            }
            let team = match ent.team_ordinal {
                0 => crate::game_logic::Team::USA,
                1 => crate::game_logic::Team::China,
                2 => crate::game_logic::Team::GLA,
                _ => crate::game_logic::Team::Neutral,
            };
            if obj.team != team {
                obj.team = team;
                dirty = true;
            }
            let disguise_team = match ent.disguise_as_team_ordinal {
                0 => Some(crate::game_logic::Team::USA),
                1 => Some(crate::game_logic::Team::China),
                2 => Some(crate::game_logic::Team::GLA),
                3 => Some(crate::game_logic::Team::Neutral),
                _ => None,
            };
            if obj.disguise_as_team != disguise_team {
                obj.disguise_as_team = disguise_team;
                dirty = true;
            }
            let object_type = match ent.object_type_ordinal {
                0 => PresentationObjectType::Infantry,
                1 => PresentationObjectType::Vehicle,
                2 => PresentationObjectType::Aircraft,
                3 => PresentationObjectType::Building,
                4 => PresentationObjectType::Supply,
                5 => PresentationObjectType::Projectile,
                _ => PresentationObjectType::Neutral,
            };
            if obj.object_type != object_type {
                obj.object_type = object_type;
                dirty = true;
            }
            let is_structure =
                matches!(object_type, PresentationObjectType::Building) || ent.is_building;
            if obj.is_structure != is_structure {
                obj.is_structure = is_structure;
                dirty = true;
            }
            let is_unit = matches!(
                object_type,
                PresentationObjectType::Infantry
                    | PresentationObjectType::Vehicle
                    | PresentationObjectType::Aircraft
            );
            if obj.is_unit != is_unit {
                obj.is_unit = is_unit;
                dirty = true;
            }
            let is_mobile = is_unit;
            if obj.is_mobile != is_mobile {
                obj.is_mobile = is_mobile;
                dirty = true;
            }
            // Prefer is_building + not under construction for can_produce residual.
            let can_produce = ent.is_building && !ent.under_construction;
            if obj.can_produce != can_produce {
                obj.can_produce = can_produce;
                dirty = true;
            }
            let building_type = if ent.is_building {
                use PresentationBuildingType as P;
                match ent.building_type_ordinal {
                    0 => Some(P::CommandCenter),
                    1 => Some(P::Barracks),
                    2 => Some(P::WarFactory),
                    3 => Some(P::Airfield),
                    4 => Some(P::RepairPad),
                    5 => Some(P::HealPad),
                    6 => Some(P::SupplyCenter),
                    7 => Some(P::PowerPlant),
                    8 => Some(P::DefenseTurret),
                    9 => Some(P::SupplyDropZone),
                    10 => Some(P::Palace),
                    11 => Some(P::Propaganda),
                    12 => Some(P::Bunker),
                    _ => None,
                }
            } else {
                None
            };
            if obj.building_type != building_type {
                obj.building_type = building_type;
                dirty = true;
            }
            // Resolve by exact Object INI identity plus the GameWorld-frozen
            // condition bank.  Do not carry an older pristine `model_key`
            // over a damaged/construction state, and do not synthesize a
            // suffix after the source state resolver has chosen a model.
            let fallback_model_key = crate::assets::mesh_asset_resolve::model_key_from_presentation(
                (!ent.model_key.is_empty()).then_some(ent.model_key.as_str()),
                &ent.template.name,
            );
            let fallback_draw_models =
                (!fallback_model_key.trim().is_empty()).then(|| crate::assets::AuthoredDrawModel {
                    module_index: 0,
                    model_key: fallback_model_key.clone(),
                    ..Default::default()
                });
            let draw_models = crate::assets::resolve_presentation_draw_models_for_live_object(
                obj.id.0,
                &ent.template.name,
                fallback_draw_models.as_slice(),
                ent.model_condition_bits,
            );
            let model_key = draw_models.first().map(|model| model.model_key.clone());
            if obj.model_key != model_key {
                obj.model_key = model_key;
                dirty = true;
            }
            if obj.draw_models != draw_models {
                obj.draw_models = draw_models;
                dirty = true;
            }
            if crate::assets::authored_draw_adjusts_height_by_construction(&obj.draw_models) {
                let cpp_percent = if obj.under_construction || obj.construction_percent + 1e-4 < 1.0
                {
                    obj.construction_percent * 100.0
                } else {
                    -1.0
                };
                if let Some(dy) = crate::assets::construction_percent_height_delta(
                    cpp_percent,
                    obj.selection_radius.max(1.0),
                ) {
                    obj.position.y += dy;
                    dirty = true;
                }
            }
            if ent.mesh_scale.is_finite()
                && ent.mesh_scale > 0.0
                && (obj.mesh_scale - ent.mesh_scale).abs() > 1e-5
            {
                obj.mesh_scale = ent.mesh_scale;
                dirty = true;
            }
            // FOW + ground-height residual.
            {
                use crate::fow_rendering::ObjectVisibility;
                let vis = ObjectVisibility {
                    visibility_alpha: ent.fow_visibility_alpha,
                    is_explored: ent.fow_is_explored,
                    visibility_falloff: ent.fow_visibility_falloff,
                };
                if obj.fow_visibility != vis {
                    obj.fow_visibility = vis;
                    dirty = true;
                }
            }
            if (obj.ground_height - ent.ground_height).abs() > 1e-3 {
                obj.ground_height = ent.ground_height;
                dirty = true;
            }
            if obj.ground_height_from_terrain != ent.ground_height_from_terrain {
                obj.ground_height_from_terrain = ent.ground_height_from_terrain;
                dirty = true;
            }
            if obj.engine_bridged != ent.engine_bridged {
                obj.engine_bridged = ent.engine_bridged;
                dirty = true;
            }
            if obj.selected != ent.selected {
                obj.selected = ent.selected;
                dirty = true;
            }
            if obj.under_construction != ent.under_construction {
                obj.under_construction = ent.under_construction;
                dirty = true;
            }
            if obj.sold != ent.sold {
                obj.sold = ent.sold;
                dirty = true;
            }
            if obj.reconstructing != ent.reconstructing {
                obj.reconstructing = ent.reconstructing;
                dirty = true;
            }
            if obj.unselectable != ent.unselectable {
                obj.unselectable = ent.unselectable;
                dirty = true;
            }
            if obj.is_deployed != ent.deployed {
                obj.is_deployed = ent.deployed;
                dirty = true;
            }
            if (obj.construction_percent - ent.construction_percent).abs() > 1e-4 {
                obj.construction_percent = ent.construction_percent;
                dirty = true;
            }
            if obj.moving != ent.moving {
                obj.moving = ent.moving;
                dirty = true;
            }
            if obj.attacking != ent.attacking {
                obj.attacking = ent.attacking;
                dirty = true;
            }
            if obj.is_firing_weapon != ent.is_firing_weapon {
                obj.is_firing_weapon = ent.is_firing_weapon;
                dirty = true;
            }
            if obj.is_aiming_weapon != ent.is_aiming_weapon {
                obj.is_aiming_weapon = ent.is_aiming_weapon;
                dirty = true;
            }
            if obj.disabled_emp != ent.disabled_emp {
                obj.disabled_emp = ent.disabled_emp;
                dirty = true;
            }
            if obj.disabled_paralyzed != ent.disabled_paralyzed {
                obj.disabled_paralyzed = ent.disabled_paralyzed;
                dirty = true;
            }
            if obj.disabled_underpowered != ent.disabled_underpowered {
                obj.disabled_underpowered = ent.disabled_underpowered;
                dirty = true;
            }
            if obj.disabled_hacked != ent.disabled_hacked {
                obj.disabled_hacked = ent.disabled_hacked;
                dirty = true;
            }
            if obj.disabled_unmanned != ent.disabled_unmanned {
                obj.disabled_unmanned = ent.disabled_unmanned;
                dirty = true;
            }
            if obj.disabled_script_disabled != ent.disabled_script_disabled {
                obj.disabled_script_disabled = ent.disabled_script_disabled;
                dirty = true;
            }
            if obj.weapons_jammed != ent.weapons_jammed {
                obj.weapons_jammed = ent.weapons_jammed;
                dirty = true;
            }
            if obj.masked != ent.masked {
                obj.masked = ent.masked;
                dirty = true;
            }
            if obj.disguised != ent.disguised {
                obj.disguised = ent.disguised;
                dirty = true;
            }
            if obj.disabled_subdued != ent.disabled_subdued {
                obj.disabled_subdued = ent.disabled_subdued;
                dirty = true;
            }
            if obj.is_carbomb != ent.is_carbomb {
                obj.is_carbomb = ent.is_carbomb;
                dirty = true;
            }
            if obj.hijacked != ent.hijacked {
                obj.hijacked = ent.hijacked;
                dirty = true;
            }
            if obj.team_color != ent.team_color {
                obj.team_color = ent.team_color;
                dirty = true;
            }
            if (obj.selection_radius - ent.selection_radius).abs() > 1e-3 {
                obj.selection_radius = ent.selection_radius;
                dirty = true;
            }
            if obj.ignoring_stealth != ent.ignoring_stealth {
                obj.ignoring_stealth = ent.ignoring_stealth;
                dirty = true;
            }
            if obj.repulsor != ent.repulsor {
                obj.repulsor = ent.repulsor;
                dirty = true;
            }
            if obj.stealthed != ent.stealthed {
                obj.stealthed = ent.stealthed;
                dirty = true;
            }
            if obj.detected != ent.detected {
                obj.detected = ent.detected;
                dirty = true;
            }
            if obj.force_attack != ent.force_attack {
                obj.force_attack = ent.force_attack;
                dirty = true;
            }
            if obj.has_weapon != ent.has_weapon {
                obj.has_weapon = ent.has_weapon;
                dirty = true;
            }
            if (obj.weapon_range - ent.weapon_range).abs() > 1e-3 {
                obj.weapon_range = ent.weapon_range;
                dirty = true;
            }
            if (obj.weapon_damage - ent.weapon_damage).abs() > 1e-3 {
                obj.weapon_damage = ent.weapon_damage;
                dirty = true;
            }
            if (obj.weapon_min_range - ent.weapon_min_range).abs() > 1e-3 {
                obj.weapon_min_range = ent.weapon_min_range;
                dirty = true;
            }
            if (obj.weapon_reload_time - ent.weapon_reload_time).abs() > 1e-3 {
                obj.weapon_reload_time = ent.weapon_reload_time;
                dirty = true;
            }
            if obj.weapon_ammo != ent.weapon_ammo {
                obj.weapon_ammo = ent.weapon_ammo;
                dirty = true;
            }
            let projectile_clip_statuses =
                Self::projectile_clip_statuses_from_gameworld_entity(ent);
            if obj.projectile_clip_statuses != projectile_clip_statuses {
                obj.projectile_clip_statuses = projectile_clip_statuses;
                dirty = true;
            }
            if obj.weapon_can_target_air != ent.weapon_can_target_air {
                obj.weapon_can_target_air = ent.weapon_can_target_air;
                dirty = true;
            }
            if obj.weapon_can_target_ground != ent.weapon_can_target_ground {
                obj.weapon_can_target_ground = ent.weapon_can_target_ground;
                dirty = true;
            }
            if (obj.weapon_projectile_speed - ent.weapon_projectile_speed).abs() > 1e-3 {
                obj.weapon_projectile_speed = ent.weapon_projectile_speed;
                dirty = true;
            }
            if obj.armed_riders_upgrade_weapon_set != ent.armed_riders_upgrade_weapon_set {
                obj.armed_riders_upgrade_weapon_set = ent.armed_riders_upgrade_weapon_set;
                dirty = true;
            }
            if obj.weapon_set_player_upgrade != ent.weapon_set_player_upgrade {
                obj.weapon_set_player_upgrade = ent.weapon_set_player_upgrade;
                dirty = true;
            }
            if obj.second_life != ent.second_life {
                obj.second_life = ent.second_life;
                dirty = true;
            }
            if obj.front_crushed != ent.front_crushed {
                obj.front_crushed = ent.front_crushed;
                dirty = true;
            }
            if obj.back_crushed != ent.back_crushed {
                obj.back_crushed = ent.back_crushed;
                dirty = true;
            }
            if obj.user_1 != ent.user_1 {
                obj.user_1 = ent.user_1;
                dirty = true;
            }
            if obj.user_2 != ent.user_2 {
                obj.user_2 = ent.user_2;
                dirty = true;
            }
            if obj.weapon_crate_upgrade != ent.weapon_crate_upgrade {
                obj.weapon_crate_upgrade = ent.weapon_crate_upgrade;
                dirty = true;
            }
            if obj.armor_crate_upgrade != ent.armor_crate_upgrade {
                obj.armor_crate_upgrade = ent.armor_crate_upgrade;
                dirty = true;
            }
            if obj.enemy_near != ent.enemy_near {
                obj.enemy_near = ent.enemy_near;
                dirty = true;
            }
            if obj.armed != ent.armed {
                obj.armed = ent.armed;
                dirty = true;
            }
            if obj.command_set_override != ent.command_set_override {
                obj.command_set_override = ent.command_set_override.clone();
                dirty = true;
            }
            if obj.is_detector != ent.is_detector {
                obj.is_detector = ent.is_detector;
                dirty = true;
            }
            if obj.show_health_bar != ent.show_health_bar {
                obj.show_health_bar = ent.show_health_bar;
                dirty = true;
            }
            // Expanded Entity residual last-writer (presentation consumers).
            if obj.power_provided != ent.power_provided {
                obj.power_provided = ent.power_provided;
                dirty = true;
            }
            if obj.power_consumed != ent.power_consumed {
                obj.power_consumed = ent.power_consumed;
                dirty = true;
            }
            if (obj.experience_points - ent.experience_points).abs() > 1e-3 {
                obj.experience_points = ent.experience_points;
                dirty = true;
            }
            if obj.stored_supplies != ent.stored_supplies {
                obj.stored_supplies = ent.stored_supplies;
                dirty = true;
            }
            let gp = ent
                .guard_position
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if obj.guard_position != gp {
                obj.guard_position = gp;
                dirty = true;
            }
            // Movement / target residual.
            let tl = ent
                .target_location
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if obj.target_location != tl {
                obj.target_location = tl;
                dirty = true;
            }
            let gt = if ent.guard_target_host == 0 {
                None
            } else {
                Some(crate::game_logic::ObjectId(ent.guard_target_host))
            };
            if obj.guard_target != gt {
                obj.guard_target = gt;
                dirty = true;
            }
            if obj.using_ability != ent.using_ability {
                obj.using_ability = ent.using_ability;
                dirty = true;
            }
            if obj.airborne_target != ent.airborne_target {
                obj.airborne_target = ent.airborne_target;
                dirty = true;
            }
            if (obj.move_max_speed - ent.move_max_speed).abs() > 1e-3 {
                obj.move_max_speed = ent.move_max_speed;
                dirty = true;
            }
            let vel = glam::Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
            if (obj.velocity - vel).length_squared() > 1e-6 {
                obj.velocity = vel;
                dirty = true;
            }
            if obj.ai_state_ordinal != ent.ai_state_ordinal {
                obj.ai_state_ordinal = ent.ai_state_ordinal;
                dirty = true;
            }
            let rp = ent.rally_point.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
            if obj.rally_point != rp {
                obj.rally_point = rp;
                dirty = true;
            }
            if obj.max_garrison != ent.max_garrison as usize {
                obj.max_garrison = ent.max_garrison as usize;
                dirty = true;
            }
            if obj.has_secondary_weapon != ent.has_secondary_weapon {
                obj.has_secondary_weapon = ent.has_secondary_weapon;
                dirty = true;
            }
            if (obj.cheer_timer - ent.cheer_timer).abs() > 1e-4 {
                obj.cheer_timer = ent.cheer_timer;
                dirty = true;
            }
            if obj.overcharge_enabled != ent.overcharge_enabled {
                obj.overcharge_enabled = ent.overcharge_enabled;
                dirty = true;
            }
            if obj.shock_was_airborne != ent.shock_was_airborne {
                obj.shock_was_airborne = ent.shock_was_airborne;
                dirty = true;
            }
            if obj.shock_allow_bounce != ent.shock_allow_bounce {
                obj.shock_allow_bounce = ent.shock_allow_bounce;
                dirty = true;
            }
            if obj.shock_grounded_once != ent.shock_grounded_once {
                obj.shock_grounded_once = ent.shock_grounded_once;
                dirty = true;
            }
            if obj.shock_stun_frames != ent.shock_stun_frames {
                obj.shock_stun_frames = ent.shock_stun_frames;
                dirty = true;
            }
            if obj.power_plant_rods_extended != ent.power_plant_rods_extended {
                obj.power_plant_rods_extended = ent.power_plant_rods_extended;
                dirty = true;
            }
            if obj.power_plant_rods_done_frame != ent.power_plant_rods_done_frame {
                obj.power_plant_rods_done_frame = ent.power_plant_rods_done_frame;
                dirty = true;
            }
            if obj.jet_slow_death_active != ent.jet_slow_death_active {
                obj.jet_slow_death_active = ent.jet_slow_death_active;
                dirty = true;
            }
            if obj.anim_steer_turn != ent.anim_steer_turn {
                obj.anim_steer_turn = ent.anim_steer_turn;
                dirty = true;
            }
            if obj.active_weapon_slot != ent.active_weapon_slot {
                obj.active_weapon_slot = ent.active_weapon_slot;
                dirty = true;
            }
            if obj.weapon_fire_status != ent.weapon_fire_status {
                obj.weapon_fire_status = ent.weapon_fire_status;
                dirty = true;
            }
            if obj.is_panicking != ent.is_panicking {
                obj.is_panicking = ent.is_panicking;
                dirty = true;
            }
            if obj.moving_backwards != ent.moving_backwards {
                obj.moving_backwards = ent.moving_backwards;
                dirty = true;
            }
            if (obj.guard_radius - ent.guard_radius).abs() > 1e-3 {
                obj.guard_radius = ent.guard_radius;
                dirty = true;
            }
            if obj.special_power_ready != ent.special_power_ready {
                obj.special_power_ready = ent.special_power_ready;
                dirty = true;
            }
            if (obj.special_power_cooldown - ent.special_power_cooldown).abs() > 1e-3 {
                obj.special_power_cooldown = ent.special_power_cooldown;
                dirty = true;
            }
            if (obj.special_power_cooldown_remaining - ent.special_power_cooldown_remaining).abs()
                > 1e-3
            {
                obj.special_power_cooldown_remaining = ent.special_power_cooldown_remaining;
                dirty = true;
            }
            if (obj.detection_range - ent.detection_range).abs() > 1e-3 {
                obj.detection_range = ent.detection_range;
                dirty = true;
            }
            if obj.detection_rate_frames != ent.detection_rate_frames {
                obj.detection_rate_frames = ent.detection_rate_frames;
                dirty = true;
            }
            if obj.stealth_breaks_on_attack != ent.stealth_breaks_on_attack {
                obj.stealth_breaks_on_attack = ent.stealth_breaks_on_attack;
                dirty = true;
            }
            if obj.stealth_breaks_on_move != ent.stealth_breaks_on_move {
                obj.stealth_breaks_on_move = ent.stealth_breaks_on_move;
                dirty = true;
            }
            if obj.innate_stealth != ent.innate_stealth {
                obj.innate_stealth = ent.innate_stealth;
                dirty = true;
            }
            if obj.weapon_bonus_enthusiastic != ent.weapon_bonus_enthusiastic {
                obj.weapon_bonus_enthusiastic = ent.weapon_bonus_enthusiastic;
                dirty = true;
            }
            if obj.weapon_bonus_subliminal != ent.weapon_bonus_subliminal {
                obj.weapon_bonus_subliminal = ent.weapon_bonus_subliminal;
                dirty = true;
            }
            if obj.weapon_bonus_horde != ent.weapon_bonus_horde {
                obj.weapon_bonus_horde = ent.weapon_bonus_horde;
                dirty = true;
            }
            if obj.weapon_bonus_nationalism != ent.weapon_bonus_nationalism {
                obj.weapon_bonus_nationalism = ent.weapon_bonus_nationalism;
                dirty = true;
            }
            if obj.weapon_bonus_frenzy != ent.weapon_bonus_frenzy {
                obj.weapon_bonus_frenzy = ent.weapon_bonus_frenzy;
                dirty = true;
            }
            if obj.weapon_bonus_frenzy_level != ent.weapon_bonus_frenzy_level {
                obj.weapon_bonus_frenzy_level = ent.weapon_bonus_frenzy_level;
                dirty = true;
            }
            if obj.weapon_bonus_frenzy_until_frame != ent.weapon_bonus_frenzy_until_frame {
                obj.weapon_bonus_frenzy_until_frame = ent.weapon_bonus_frenzy_until_frame;
                dirty = true;
            }
            if obj.weapon_bonus_battle_plan_bombardment != ent.weapon_bonus_battle_plan_bombardment
            {
                obj.weapon_bonus_battle_plan_bombardment = ent.weapon_bonus_battle_plan_bombardment;
                dirty = true;
            }
            if obj.weapon_bonus_battle_plan_hold_the_line
                != ent.weapon_bonus_battle_plan_hold_the_line
            {
                obj.weapon_bonus_battle_plan_hold_the_line =
                    ent.weapon_bonus_battle_plan_hold_the_line;
                dirty = true;
            }
            if obj.weapon_bonus_battle_plan_search_and_destroy
                != ent.weapon_bonus_battle_plan_search_and_destroy
            {
                obj.weapon_bonus_battle_plan_search_and_destroy =
                    ent.weapon_bonus_battle_plan_search_and_destroy;
                dirty = true;
            }
            if (obj.battle_plan_sight_scalar_applied - ent.battle_plan_sight_scalar_applied).abs()
                > 1e-4
            {
                obj.battle_plan_sight_scalar_applied = ent.battle_plan_sight_scalar_applied;
                dirty = true;
            }
            if obj.continuous_fire_level != ent.continuous_fire_level {
                obj.continuous_fire_level = ent.continuous_fire_level;
                dirty = true;
            }
            if obj.continuous_fire_consecutive != ent.continuous_fire_consecutive {
                obj.continuous_fire_consecutive = ent.continuous_fire_consecutive;
                dirty = true;
            }
            if obj.continuous_fire_coast_until_frame != ent.continuous_fire_coast_until_frame {
                obj.continuous_fire_coast_until_frame = ent.continuous_fire_coast_until_frame;
                dirty = true;
            }
            if obj.faerie_fire_until_frame != ent.faerie_fire_until_frame {
                obj.faerie_fire_until_frame = ent.faerie_fire_until_frame;
                dirty = true;
            }
            if obj.is_humvee_transport != ent.is_humvee_transport {
                obj.is_humvee_transport = ent.is_humvee_transport;
                dirty = true;
            }
            if obj.is_listening_outpost_transport != ent.is_listening_outpost_transport {
                obj.is_listening_outpost_transport = ent.is_listening_outpost_transport;
                dirty = true;
            }
            if obj.is_troop_crawler_transport != ent.is_troop_crawler_transport {
                obj.is_troop_crawler_transport = ent.is_troop_crawler_transport;
                dirty = true;
            }
            if obj.is_helix_transport != ent.is_helix_transport {
                obj.is_helix_transport = ent.is_helix_transport;
                dirty = true;
            }
            if obj.has_overlord_gattling_addon != ent.has_overlord_gattling_addon {
                obj.has_overlord_gattling_addon = ent.has_overlord_gattling_addon;
                dirty = true;
            }
            if obj.has_overlord_propaganda_addon != ent.has_overlord_propaganda_addon {
                obj.has_overlord_propaganda_addon = ent.has_overlord_propaganda_addon;
                dirty = true;
            }
            // Expanded transport-kind / display residual.
            if obj.is_battle_bus_transport != ent.is_battle_bus_transport {
                obj.is_battle_bus_transport = ent.is_battle_bus_transport;
                dirty = true;
            }
            if obj.is_technical_transport != ent.is_technical_transport {
                obj.is_technical_transport = ent.is_technical_transport;
                dirty = true;
            }
            if obj.is_combat_cycle_transport != ent.is_combat_cycle_transport {
                obj.is_combat_cycle_transport = ent.is_combat_cycle_transport;
                dirty = true;
            }
            if obj.combat_cycle_rider != ent.combat_cycle_rider {
                obj.combat_cycle_rider = ent.combat_cycle_rider;
                dirty = true;
            }
            if obj.is_tunnel_network != ent.is_tunnel_network {
                obj.is_tunnel_network = ent.is_tunnel_network;
                dirty = true;
            }
            if obj.is_combat_chinook_transport != ent.is_combat_chinook_transport {
                obj.is_combat_chinook_transport = ent.is_combat_chinook_transport;
                dirty = true;
            }
            if obj.max_transport != ent.max_transport {
                obj.max_transport = ent.max_transport;
                dirty = true;
            }
            let bunker_cap = if ent.overlord_bunker_capacity == u16::MAX {
                usize::MAX
            } else {
                ent.overlord_bunker_capacity as usize
            };
            if obj.overlord_bunker_capacity != bunker_cap {
                obj.overlord_bunker_capacity = bunker_cap;
                dirty = true;
            }
            if obj.passengers_allowed_to_fire != ent.passengers_allowed_to_fire {
                obj.passengers_allowed_to_fire = ent.passengers_allowed_to_fire;
                dirty = true;
            }
            if obj.display_name != ent.display_name {
                obj.display_name = ent.display_name.clone();
                dirty = true;
            }
            if obj.demo_suicided_detonating != ent.demo_suicided_detonating {
                obj.demo_suicided_detonating = ent.demo_suicided_detonating;
                dirty = true;
            }
            if obj.hive_slave_count != ent.hive_slave_count {
                obj.hive_slave_count = ent.hive_slave_count;
                dirty = true;
            }
            if (obj.hive_slave_hp - ent.hive_slave_hp).abs() > 1e-3 {
                obj.hive_slave_hp = ent.hive_slave_hp;
                dirty = true;
            }
            if (obj.turret_angle_deg - ent.turret_angle_deg).abs() > 1e-3 {
                obj.turret_angle_deg = ent.turret_angle_deg;
                dirty = true;
            }
            if (obj.turret_pitch_deg - ent.turret_pitch_deg).abs() > 1e-3 {
                obj.turret_pitch_deg = ent.turret_pitch_deg;
                dirty = true;
            }
            if obj.turret_idle_scanning != ent.turret_idle_scanning {
                obj.turret_idle_scanning = ent.turret_idle_scanning;
                dirty = true;
            }
            if obj.turret_holding != ent.turret_holding {
                obj.turret_holding = ent.turret_holding;
                dirty = true;
            }
            if obj.ai_attitude != ent.ai_attitude {
                obj.ai_attitude = ent.ai_attitude;
                dirty = true;
            }
            if obj.last_damage_source_host != ent.last_damage_source_host {
                obj.last_damage_source_host = ent.last_damage_source_host;
                dirty = true;
            }
            let disguise = if ent.disguise_as_template.is_empty() {
                None
            } else {
                Some(ent.disguise_as_template.clone())
            };
            if obj.disguise_as_template != disguise {
                obj.disguise_as_template = disguise;
                dirty = true;
            }
            if obj.vision_spied_mask != ent.vision_spied_mask {
                obj.vision_spied_mask = ent.vision_spied_mask;
                dirty = true;
            }
            // Wave 994: vision / shroud-clear / crush residual last-writer.
            if (obj.vision_range - ent.vision_range).abs() > 1e-4 {
                obj.vision_range = ent.vision_range;
                dirty = true;
            }
            if (obj.shroud_clearing_range - ent.shroud_clearing_range).abs() > 1e-4 {
                obj.shroud_clearing_range = ent.shroud_clearing_range;
                dirty = true;
            }
            if obj.crusher_level != ent.crusher_level {
                obj.crusher_level = ent.crusher_level;
                dirty = true;
            }
            if obj.crushable_level != ent.crushable_level {
                obj.crushable_level = ent.crushable_level;
                dirty = true;
            }
            // Wave 995: captured / prone / poison / defector residual last-writer.
            if obj.captured != ent.private_captured {
                obj.captured = ent.private_captured;
                dirty = true;
            }
            if obj.prone != ent.prone_active {
                obj.prone = ent.prone_active;
                dirty = true;
            }
            let poison_tinted = ent.poison_damage_frame != 0;
            if obj.poison_tinted != poison_tinted {
                obj.poison_tinted = poison_tinted;
                dirty = true;
            }
            if obj.undetected_defector != ent.defection_undetected {
                obj.undetected_defector = ent.defection_undetected;
                dirty = true;
            }
            if obj.defector_flash != ent.defection_flash_this_frame {
                obj.defector_flash = ent.defection_flash_this_frame;
                dirty = true;
            }
            if obj.cell_is_cliff != ent.cell_is_cliff {
                obj.cell_is_cliff = ent.cell_is_cliff;
                dirty = true;
            }
            if obj.cell_is_underwater != ent.cell_is_underwater {
                obj.cell_is_underwater = ent.cell_is_underwater;
                dirty = true;
            }
            if obj.over_water != ent.cell_is_underwater {
                obj.over_water = ent.cell_is_underwater;
                dirty = true;
            }
            if obj.formation_id != ent.formation_id {
                obj.formation_id = ent.formation_id;
                dirty = true;
            }
            let form_off = glam::Vec2::new(ent.formation_offset[0], ent.formation_offset[1]);
            if (obj.formation_offset - form_off).length_squared() > 1e-8 {
                obj.formation_offset = form_off;
                dirty = true;
            }
            // Wave 999: surrender + emoticon residual last-writer.
            if obj.is_surrendered != ent.is_surrendered {
                obj.is_surrendered = ent.is_surrendered;
                dirty = true;
            }
            if obj.emoticon_name != ent.emoticon_name {
                obj.emoticon_name = ent.emoticon_name.clone();
                dirty = true;
            }
            if obj.emoticon_frames_left != ent.emoticon_frames_left {
                obj.emoticon_frames_left = ent.emoticon_frames_left;
                dirty = true;
            }
            // Wave 1001: FX name residual last-writer.
            if obj.damage_fx_name != ent.damage_fx_name {
                obj.damage_fx_name = ent.damage_fx_name.clone();
                dirty = true;
            }
            if obj.bone_fx_name != ent.bone_fx_name {
                obj.bone_fx_name = ent.bone_fx_name.clone();
                dirty = true;
            }
            if obj.death_fx_name != ent.death_fx_name {
                obj.death_fx_name = ent.death_fx_name.clone();
                dirty = true;
            }
            // Wave 996: topple lean + healing icon residual last-writer.
            if (obj.topple_lean_radians - ent.topple_lean_radians).abs() > 1e-5 {
                obj.topple_lean_radians = ent.topple_lean_radians;
                dirty = true;
            }
            if (obj.float_yaw - ent.float_yaw).abs() > 1e-5
                || (obj.float_pitch - ent.float_pitch).abs() > 1e-5
            {
                obj.float_yaw = ent.float_yaw;
                obj.float_pitch = ent.float_pitch;
                dirty = true;
            }
            let show_healing = ent.sole_healing_benefactor_expiration_frame != 0;
            if obj.show_healing != show_healing {
                obj.show_healing = show_healing;
                dirty = true;
            }
            {
                const STRUCTURE_BIT: u32 = 1 << 0;
                const VEHICLE_BIT: u32 = 1 << 2;
                let icon = if ent.kind_of_bits & STRUCTURE_BIT != 0 {
                    1u8
                } else if ent.kind_of_bits & VEHICLE_BIT != 0 {
                    2u8
                } else {
                    0u8
                };
                if obj.healing_icon_type != icon {
                    obj.healing_icon_type = icon;
                    dirty = true;
                }
            }
            if (obj.camo_friendly_opacity - ent.camo_friendly_opacity).abs() > 1e-4 {
                obj.camo_friendly_opacity = ent.camo_friendly_opacity;
                dirty = true;
            }
            if obj.camo_stealth_look != ent.camo_stealth_look {
                obj.camo_stealth_look = ent.camo_stealth_look;
                dirty = true;
            }
            // Path waypoints residual (presentation move lines).
            let path_wp: Vec<glam::Vec3> = ent
                .path_waypoints
                .iter()
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                .collect();
            if obj.path_waypoints != path_wp {
                obj.path_waypoints = path_wp;
                dirty = true;
            }
            if obj.path_len != ent.path_len {
                obj.path_len = ent.path_len;
                dirty = true;
            }
            if obj.path_index != ent.path_index {
                obj.path_index = ent.path_index;
                dirty = true;
            }
            if obj.occupant_count != ent.occupant_count {
                obj.occupant_count = ent.occupant_count;
                dirty = true;
            }
            // Production queue head residual.
            // Full production queue residual (not head-only).
            if !ent.production_queue_items.is_empty() {
                let q: Vec<PresentationProductionItem> = ent
                    .production_queue_items
                    .iter()
                    .map(|p| PresentationProductionItem {
                        template_name: p.template_name.clone(),
                        progress: p.progress,
                        total_time: p.total_time,
                        cost_supplies: p.cost_supplies,
                        is_upgrade: p.is_upgrade,
                        progress_ratio: if p.total_time <= 0.0 {
                            1.0
                        } else {
                            (p.progress / p.total_time).clamp(0.0, 1.0)
                        },
                    })
                    .collect();
                if obj.production_queue != q {
                    obj.production_queue = q;
                    dirty = true;
                }
                if obj.production_paused != ent.production_paused {
                    obj.production_paused = ent.production_paused;
                    dirty = true;
                }
            } else if !obj.production_queue.is_empty() || obj.production_paused {
                obj.production_queue.clear();
                if obj.production_paused != ent.production_paused {
                    obj.production_paused = ent.production_paused;
                }
                dirty = true;
            }
            let ent_producer = ent.producer_id.map(ObjectId);
            if obj.producer_id != ent_producer {
                obj.producer_id = ent_producer;
                dirty = true;
            }
            if obj.is_rebuild_hole != ent.is_rebuild_hole {
                obj.is_rebuild_hole = ent.is_rebuild_hole;
                dirty = true;
            }
            if obj.rebuild_template_name != ent.rebuild_template_name {
                obj.rebuild_template_name = ent.rebuild_template_name.clone();
                dirty = true;
            }
            if obj.rebuild_ready_frame != ent.rebuild_ready_frame {
                obj.rebuild_ready_frame = ent.rebuild_ready_frame;
                dirty = true;
            }
            let spawner = ent.rebuild_spawner_id.map(ObjectId);
            if obj.rebuild_spawner_id != spawner {
                obj.rebuild_spawner_id = spawner;
                dirty = true;
            }
            let worker = ent.rebuild_worker_id.map(ObjectId);
            if obj.rebuild_worker_id != worker {
                obj.rebuild_worker_id = worker;
                dirty = true;
            }
            let recon = ent.rebuild_reconstructing_id.map(ObjectId);
            if obj.rebuild_reconstructing_id != recon {
                obj.rebuild_reconstructing_id = recon;
                dirty = true;
            }
            if obj.reconstructing != ent.reconstructing {
                obj.reconstructing = ent.reconstructing;
                dirty = true;
            }
            if obj.has_secondary_weapon != ent.has_secondary_weapon {
                obj.has_secondary_weapon = ent.has_secondary_weapon;
                dirty = true;
            }
            if (obj.secondary_weapon_range - ent.secondary_weapon_range).abs() > 1e-3 {
                obj.secondary_weapon_range = ent.secondary_weapon_range;
                dirty = true;
            }
            if (obj.secondary_weapon_damage - ent.secondary_weapon_damage).abs() > 1e-3 {
                obj.secondary_weapon_damage = ent.secondary_weapon_damage;
                dirty = true;
            }
            if obj.has_mine != ent.has_mine_data {
                obj.has_mine = ent.has_mine_data;
                dirty = true;
            }
            // Contain / garrison residual.
            let contained = if ent.contained_by_host == 0 {
                None
            } else {
                Some(crate::game_logic::ObjectId(ent.contained_by_host))
            };
            if obj.contained_by != contained {
                obj.contained_by = contained;
                dirty = true;
            }
            let garrisoned: Vec<crate::game_logic::ObjectId> = ent
                .garrisoned_host_ids
                .iter()
                .copied()
                .map(crate::game_logic::ObjectId)
                .collect();
            if obj.garrisoned_units != garrisoned {
                obj.garrisoned_units = garrisoned;
                dirty = true;
            }
            // Disabled residual (any host disable flag).
            let disabled =
                ent.disabled_underpowered || ent.disabled_unmanned || ent.disabled_hacked;
            if obj.disabled != disabled {
                obj.disabled = disabled;
                dirty = true;
            }
            // Veterancy ordinal residual.
            let vet = match ent.veterancy_ordinal {
                1 => PresentationVeterancy::Veteran,
                2 => PresentationVeterancy::Elite,
                3 => PresentationVeterancy::Heroic,
                _ => PresentationVeterancy::Rookie,
            };
            if obj.veterancy != vet {
                obj.veterancy = vet;
                dirty = true;
            }
            // KindOf bitset residual → presentation ORDER vector.
            {
                use crate::game_logic::KindOf;
                const ORDER: &[KindOf] = &[
                    KindOf::Structure,
                    KindOf::Infantry,
                    KindOf::Vehicle,
                    KindOf::Aircraft,
                    KindOf::Projectile,
                    KindOf::Resource,
                    KindOf::Selectable,
                    KindOf::Attackable,
                    KindOf::CommandCenter,
                    KindOf::Worker,
                    KindOf::Hero,
                    KindOf::SupplyCenter,
                    KindOf::PowerPlant,
                    KindOf::FSBarracks,
                    KindOf::FSWarFactory,
                    KindOf::FSAirfield,
                    KindOf::FSInternetCenter,
                    KindOf::FSPower,
                    KindOf::FSBaseDefense,
                    KindOf::FSSupplyDropzone,
                    KindOf::FSSupplyCenter,
                    KindOf::FSSuperweapon,
                    KindOf::FSStrategyCenter,
                    KindOf::FSFake,
                    KindOf::FSTechnology,
                    KindOf::FSBlackMarket,
                    KindOf::FSAdvancedTech,
                    KindOf::Harvestable,
                    KindOf::Powered,
                    KindOf::IgnoredInGui,
                    // Must remain aligned with Object::presentation_kind_of_bits.
                    KindOf::Dozer,
                    KindOf::Harvester,
                ];
                let mut v: Vec<KindOf> = ORDER
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| ent.kind_of_bits & (1u32 << i) != 0)
                    .map(|(_, k)| *k)
                    .collect();
                v.truncate(32);
                if obj.kind_of != v {
                    obj.kind_of = v;
                    dirty = true;
                }
                if obj.unattackable != ent.unattackable {
                    obj.unattackable = ent.unattackable;
                    dirty = true;
                }
                let dock_kind = crate::game_logic::DockKind::from_ordinal(ent.dock_kind);
                if obj.dock_kind != dock_kind {
                    obj.dock_kind = dock_kind;
                    dirty = true;
                }
                if obj.capturable != ent.capturable {
                    obj.capturable = ent.capturable;
                    dirty = true;
                }
                if obj.immune_to_capture != ent.immune_to_capture {
                    obj.immune_to_capture = ent.immune_to_capture;
                    dirty = true;
                }
                if obj.capture_garrisonable != ent.capture_garrisonable {
                    obj.capture_garrisonable = ent.capture_garrisonable;
                    dirty = true;
                }
                let capture_power =
                    crate::game_logic::CapturePowerKind::from_ordinal(ent.capture_power);
                if obj.capture_power != capture_power {
                    obj.capture_power = capture_power;
                    dirty = true;
                }
                if obj.capture_power_ready != ent.capture_power_ready {
                    obj.capture_power_ready = ent.capture_power_ready;
                    dirty = true;
                }
                if obj.hacker_disable_building_capable != ent.hacker_disable_building_capable {
                    obj.hacker_disable_building_capable = ent.hacker_disable_building_capable;
                    dirty = true;
                }
                if obj.hacker_disable_building_ready != ent.hacker_disable_building_ready {
                    obj.hacker_disable_building_ready = ent.hacker_disable_building_ready;
                    dirty = true;
                }
                let special_power_ready_template_name =
                    (!ent.special_power_ready_template_name.is_empty())
                        .then(|| ent.special_power_ready_template_name.clone());
                if obj.special_power_ready_template_name != special_power_ready_template_name {
                    obj.special_power_ready_template_name = special_power_ready_template_name;
                    dirty = true;
                }
                let special_power_ready_template_id =
                    (!ent.special_power_ready_template_name.is_empty())
                        .then_some(ent.special_power_ready_template_id);
                if obj.special_power_ready_template_id != special_power_ready_template_id {
                    obj.special_power_ready_template_id = special_power_ready_template_id;
                    dirty = true;
                }
            }
            // Applied upgrade names residual.
            if obj.applied_upgrades != ent.applied_upgrade_names {
                obj.applied_upgrades = ent.applied_upgrade_names.clone();
                dirty = true;
            }
            // Effectively stealthed residual from shadow flags.
            let eff = ent.stealthed && !ent.detected && obj.disguise_as_template.is_none();
            if obj.effectively_stealthed != eff {
                obj.effectively_stealthed = eff;
                dirty = true;
            }
            if dirty {
                updated += 1;
            }
        }
        // Local player residual from shadow (presentation last-writer).
        // Prefer local_player_id slot (same dense index as host player id residual).
        let local_pid = gamelogic::world::PlayerId::from_index(self.local_player_id as u8);
        if let Some(p) = shadow.world().player(local_pid) {
            if crate::gameworld_shadow::gameworld_economy_authority_live() {
                self.local_supplies = p.supplies;
                self.local_power = p.power_available;
                self.local_power_produced = p.power_produced;
                self.local_power_consumed = p.power_consumed;
            }
            // Presentation-only player residual (always from shadow when mapped).
            self.local_is_alive = p.is_alive;
            self.local_radar_count = p.radar_count;
            self.local_radar_disabled = p.radar_disabled;
            self.local_cash_bounty_percent = p.cash_bounty_percent;
            self.local_color_rgb = p.color_rgb;
            self.local_rank_level = p.rank_level.max(1);
            self.local_skill_points = p.skill_points;
            self.local_science_purchase_points = p.science_purchase_points;
            self.local_unlocked_sciences = p.unlocked_sciences.clone();
            {
                use crate::game_logic::host_rank_ui_residual::{
                    RankSkillStateResidual, rank_level_down_threshold_residual,
                    rank_level_up_threshold_residual, rank_progress_percent_residual,
                };
                let state = RankSkillStateResidual {
                    rank_level: self.local_rank_level,
                    skill_points: self.local_skill_points,
                    science_purchase_points: self.local_science_purchase_points,
                    level_up: rank_level_up_threshold_residual(self.local_rank_level),
                    level_down: rank_level_down_threshold_residual(self.local_rank_level),
                };
                self.local_rank_progress_percent = rank_progress_percent_residual(&state);
            }
            // Superweapon PublicTimer remaining from the owning player's
            // SharedNSync ready frame (C++ SpecialPowerModule.cpp:756-765).
            // power_key is bare for the local player and `Type#id` for others.
            for timer in &mut self.superweapon_timers {
                if timer.power_key.is_empty() {
                    continue;
                }
                let (owner_id, key) = super::build::split_superweapon_power_key(
                    &timer.power_key,
                    self.local_player_id,
                );
                let pid = gamelogic::world::PlayerId::from_index(owner_id as u8);
                let Some(owner) = shadow.world().player(pid) else {
                    continue;
                };
                if let Some((_, rem)) = owner
                    .shared_special_power_cooldowns
                    .iter()
                    .find(|(k, _)| k == key)
                {
                    let rem = (*rem).max(0.0);
                    if (timer.remaining - rem).abs() > 1e-5 {
                        timer.remaining = rem;
                        updated += 1;
                    }
                    let ready = timer.unlocked && rem <= 0.0;
                    if timer.ready != ready {
                        timer.ready = ready;
                        updated += 1;
                    }
                }
            }
        }
        // Roster is_alive / color from shadow slots (dense host id ↔ PlayerId residual).
        for pi in &mut self.players {
            let pid = gamelogic::world::PlayerId::from_index(pi.id as u8);
            if let Some(p) = shadow.world().player(pid) {
                if pi.is_alive != p.is_alive {
                    pi.is_alive = p.is_alive;
                    updated += 1;
                }
                if pi.color_rgb != p.color_rgb {
                    pi.color_rgb = p.color_rgb;
                    updated += 1;
                }
            }
        }
        self.gameworld_overlay_stamped = updated;
        updated
    }
    /// Sparse RenderableObject from a GameWorld entity (Wave 192).
    ///
    /// Fills identity/pose/HP/motion/selection from the borrow-first entity store.
    /// Host-only presentation fields stay at safe defaults until a later cutover.
    /// Fail-closed: not full build_from_logic parity (weapons, FOW grid, FX, etc.).

    /// Wave 490: decode entity `kind_of_bits` using host presentation ORDER residual.
    fn kind_of_list_from_presentation_bits(bits: u32) -> Vec<crate::game_logic::KindOf> {
        use crate::game_logic::KindOf;
        const ORDER: &[KindOf] = &[
            KindOf::Structure,
            KindOf::Infantry,
            KindOf::Vehicle,
            KindOf::Aircraft,
            KindOf::Projectile,
            KindOf::Resource,
            KindOf::Selectable,
            KindOf::Attackable,
            KindOf::CommandCenter,
            KindOf::Worker,
            KindOf::Hero,
            KindOf::SupplyCenter,
            KindOf::PowerPlant,
            KindOf::FSBarracks,
            KindOf::FSWarFactory,
            KindOf::FSAirfield,
            KindOf::FSInternetCenter,
            KindOf::FSPower,
            KindOf::FSBaseDefense,
            KindOf::FSSupplyDropzone,
            KindOf::FSSupplyCenter,
            KindOf::FSSuperweapon,
            KindOf::FSStrategyCenter,
            KindOf::FSFake,
            KindOf::FSTechnology,
            KindOf::FSBlackMarket,
            KindOf::FSAdvancedTech,
            KindOf::Harvestable,
            KindOf::Powered,
            KindOf::IgnoredInGui,
            // Must remain aligned with Object::presentation_kind_of_bits.
            KindOf::Dozer,
            KindOf::Harvester,
        ];
        let mut out = Vec::new();
        for (i, k) in ORDER.iter().enumerate() {
            if i < 32 && (bits & (1u32 << i)) != 0 {
                out.push(*k);
            }
        }
        out
    }

    pub fn renderable_from_gameworld_entity(
        host_id: crate::game_logic::ObjectId,
        ent: &gamelogic::world::entities::Entity,
    ) -> RenderableObject {
        let team = match ent.team_ordinal {
            0 => crate::game_logic::Team::USA,
            1 => crate::game_logic::Team::China,
            2 => crate::game_logic::Team::GLA,
            _ => crate::game_logic::Team::Neutral,
        };
        let p = ent.transform.position;
        let pos = glam::Vec3::new(p.x, p.y, p.z);
        let vel = glam::Vec3::new(ent.velocity[0], ent.velocity[1], ent.velocity[2]);
        let move_destination = ent.move_target.map(|p| glam::Vec3::new(p[0], p[1], p[2]));
        let attack_target = ent
            .attack_target
            .map(|eid| crate::game_logic::ObjectId(eid.get()));
        let health_max = if ent.max_health > 0.0 {
            ent.max_health
        } else if ent.health > 0.0 {
            ent.health
        } else {
            1.0
        };
        let moving = vel.length_squared() > 1e-6 || move_destination.is_some();
        let fallback_model_key = crate::assets::mesh_asset_resolve::model_key_from_presentation(
            Some(ent.template.name.as_str()),
            &ent.template.name,
        );
        let fallback_draw_models =
            (!fallback_model_key.trim().is_empty()).then(|| crate::assets::AuthoredDrawModel {
                module_index: 0,
                model_key: fallback_model_key,
                ..Default::default()
            });
        let draw_models = crate::assets::resolve_presentation_draw_models_for_live_object(
            host_id.0,
            &ent.template.name,
            fallback_draw_models.as_slice(),
            ent.model_condition_bits,
        );
        let model_key = draw_models.first().map(|model| model.model_key.clone());
        RenderableObject {
            id: host_id,
            template_name: ent.template.name.clone(),
            team,
            // The GameWorld entity only carries its GameWorld PlayerId here;
            // overlay_host_fx_residual resolves the corresponding host ID.
            owner_player_id: None,
            team_color: ent.team_color,
            position: pos,
            orientation: ent.transform.orientation,
            float_yaw: ent.float_yaw,
            float_pitch: ent.float_pitch,
            // Wave 498: filled by overlay_host_fx_residual when host is available.
            topple_lean_radians: ent.topple_lean_radians,
            topple_dir_x: 1.0,
            topple_dir_y: 0.0,
            shadows_enabled: true,
            terrain_decal_type: 8,
            terrain_decal_size: 0.0,
            terrain_decal_opacity: 0.0,
            move_destination,
            // Wave 489: order/path/production presentation from GW entity.
            target_location: ent
                .target_location
                .map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            guard_target: if ent.guard_target_host != 0 {
                Some(crate::game_logic::ObjectId(ent.guard_target_host))
            } else {
                None
            },
            using_ability: ent.using_ability,
            airborne_target: ent.airborne_target,
            producer_id: ent.producer_id.map(ObjectId), // Wave 992: GameWorld entity producer residual.
            show_healing: ent.sole_healing_benefactor_expiration_frame != 0,
            healing_icon_type: {
                const STRUCTURE_BIT: u32 = 1 << 0;
                const VEHICLE_BIT: u32 = 1 << 2;
                if ent.kind_of_bits & STRUCTURE_BIT != 0 {
                    1
                } else if ent.kind_of_bits & VEHICLE_BIT != 0 {
                    2
                } else {
                    0
                }
            },
            parachuting: ent.parachuting,
            parachute_open: ent.parachute_open,
            object_weather: 0,

            captured: ent.private_captured,
            prone: ent.prone_active,
            emoticon_name: ent.emoticon_name.clone(),
            emoticon_frames_left: ent.emoticon_frames_left,
            is_surrendered: ent.is_surrendered,
            formation_id: ent.formation_id,
            formation_offset: glam::Vec2::new(ent.formation_offset[0], ent.formation_offset[1]), // Wave 998
            over_water: ent.cell_is_underwater,
            cell_is_cliff: ent.cell_is_cliff,
            cell_is_underwater: ent.cell_is_underwater,
            move_max_speed: ent.move_max_speed,
            velocity: vel,
            ai_state_ordinal: ent.ai_state_ordinal,
            attack_target,
            path_waypoints: ent
                .path_waypoints
                .iter()
                .map(|p| glam::Vec3::new(p[0], p[1], p[2]))
                .collect(),
            path_len: ent.path_len,
            path_index: ent.path_index,
            occupant_count: ent.occupant_count,
            production_queue: ent
                .production_queue_items
                .iter()
                .map(PresentationProductionItem::from_entity_item)
                .collect(),
            production_paused: ent.production_paused, // Wave 991: GameWorld entity pause residual.
            rally_point: ent.rally_point.map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            guard_position: ent
                .guard_position
                .map(|p| glam::Vec3::new(p[0], p[1], p[2])),
            // Wave 490: garrison/container presentation from GW entity.
            garrisoned_units: ent
                .garrisoned_host_ids
                .iter()
                .copied()
                .filter(|&id| id != 0)
                .map(crate::game_logic::ObjectId)
                .collect(),
            stealth_garrison_occupant_count: 0,

            max_garrison: ent.max_garrison as usize,
            power_provided: ent.power_provided,
            power_consumed: ent.power_consumed,
            stored_supplies: ent.stored_supplies,
            drawable_supply_boxes: 0,
            drawable_supply_max_boxes: 0,
            dock_kind: crate::game_logic::DockKind::from_ordinal(ent.dock_kind),
            capturable: ent.capturable,
            immune_to_capture: ent.immune_to_capture,
            capture_garrisonable: ent.capture_garrisonable,
            capture_power: crate::game_logic::CapturePowerKind::from_ordinal(ent.capture_power),
            capture_power_ready: ent.capture_power_ready,
            hacker_disable_building_capable: ent.hacker_disable_building_capable,
            hacker_disable_building_ready: ent.hacker_disable_building_ready,
            special_power_ready_template_name: (!ent.special_power_ready_template_name.is_empty())
                .then(|| ent.special_power_ready_template_name.clone()),
            special_power_ready_template_id: (!ent.special_power_ready_template_name.is_empty())
                .then_some(ent.special_power_ready_template_id),
            special_power_override_destination: None,
            health_current: ent.health.max(0.0),
            selection_flash_color: None,

            health_max,
            selected: ent.selected,
            is_deployed: ent.deployed,
            selection_flash_remaining: ent.selection_flash_remaining,
            destroyed: ent.destroyed || ent.health <= 0.0,
            // Wave 488: carry GW entity presentation channels (not hard-zero).
            model_condition_bits: ent.model_condition_bits,
            radar_active: ent.radar_active,
            radar_extend_complete: ent.radar_extend_complete,
            production_door_phase: ent.production_door_phase,
            body_damage_state: ent.body_damage_state,
            // Wave 498 defaults; overlay_host_fx_residual stamps live host FX residual.
            damage_fx_name: ent.damage_fx_name.clone(),
            bone_fx_name: ent.bone_fx_name.clone(),
            poison_tinted: ent.poison_damage_frame != 0,
            undetected_defector: ent.defection_undetected,
            defector_flash: ent.defection_flash_this_frame,
            death_fx_name: ent.death_fx_name.clone(),
            death_type_name: if ent.destroyed || ent.health <= 0.0 {
                crate::game_logic::host_usa_pilot::HostDeathType::from_ordinal(ent.death_type)
                    .as_name()
                    .to_string()
            } else {
                String::new()
            },
            under_construction: ent.under_construction,
            is_dozer_task_pending: false,
            construction_percent: ent.construction_percent,
            // Wave 1031: GW path has no host supply-drop OCL timer residual yet.
            ocl_timer_seconds: 0,
            sold: ent.sold,
            script_unsellable: false,
            single_use_command_used: false,
            unselectable: ent.unselectable,
            is_rebuild_hole: ent.is_rebuild_hole,
            rebuild_template_name: ent.rebuild_template_name.clone(),
            rebuild_ready_frame: ent.rebuild_ready_frame,
            rebuild_spawner_id: ent.rebuild_spawner_id.map(ObjectId),
            rebuild_worker_id: ent.rebuild_worker_id.map(ObjectId),
            rebuild_reconstructing_id: ent.rebuild_reconstructing_id.map(ObjectId),
            reconstructing: ent.reconstructing,
            // Wave 490: XP/veterancy from GW entity.
            veterancy: PresentationVeterancy::from_ordinal(ent.veterancy_ordinal),
            experience_points: ent.experience_points,
            moving: moving || ent.moving,
            attacking: attack_target.is_some() || ent.attacking,
            is_firing_weapon: ent.is_firing_weapon,
            is_aiming_weapon: ent.is_aiming_weapon,
            // Wave 490: disable/status presentation from GW entity.
            disabled_emp: ent.disabled_emp,
            disabled_paralyzed: ent.disabled_paralyzed,
            disabled_underpowered: ent.disabled_underpowered,
            disabled_hacked: ent.disabled_hacked,
            disabled_unmanned: ent.disabled_unmanned,
            disabled_freefall: false,
            disabled_default: false,
            disabled_script_underpowered: false,
            disabled_script_disabled: ent.disabled_script_disabled,
            hacking_packing_or_unpacking: false,
            weapons_jammed: ent.weapons_jammed,
            masked: ent.masked,
            unattackable: ent.unattackable,
            is_force_attackable: crate::game_logic::host_car_bomb::object_definition_has_kind(
                &ent.template.name,
                "FORCEATTACKABLE",
            ),
            always_selectable: crate::game_logic::host_car_bomb::object_definition_has_kind(
                &ent.template.name,
                "ALWAYS_SELECTABLE",
            ),

            is_crate: false,
            is_salvage_crate: false,
            ignoring_stealth: ent.ignoring_stealth,
            repulsor: ent.repulsor,
            // Wave 489: stealth/weapon presentation from GW entity.
            stealthed: ent.stealthed,
            detected: ent.detected,
            effectively_stealthed: ent.stealthed && !ent.detected,
            // GameWorld has no typed StealthUpdate module metadata. A host
            // overlay stamps this source capability for the active engine
            // path; standalone GameWorld presentation fails closed.
            can_disguise_as_team: false,
            friendly_stealth_opacity: 0.5,
            friendly_stealth_opacity_max: 1.0,
            disabled: ent.disabled_emp
                || ent.disabled_paralyzed
                || ent.disabled_hacked
                || ent.disabled_underpowered
                || ent.disabled_unmanned
                || ent.disabled_subdued
                || ent.disabled_script_disabled,
            contained_by: if ent.contained_by_host != 0 {
                Some(crate::game_logic::ObjectId(ent.contained_by_host))
            } else {
                None
            },
            force_attack: ent.force_attack,
            has_weapon: ent.has_weapon,
            weapon_range: ent.weapon_range,
            weapon_damage: ent.weapon_damage,
            weapon_min_range: ent.weapon_min_range,
            weapon_reload_time: ent.weapon_reload_time,
            weapon_ammo: ent.weapon_ammo,
            projectile_clip_statuses: Self::projectile_clip_statuses_from_gameworld_entity(ent),
            ammo_pip_total: ent.weapon_clip_size,
            ammo_pip_full: ent.weapon_ammo.min(ent.weapon_clip_size),
            weapon_ready_percent: if ent.weapon_reload_time > 1e-6 {
                ((((ent.weapon_reload_time - ent.weapon_last_fire_time.max(0.0))
                    / ent.weapon_reload_time)
                    .clamp(0.0, 1.0))
                    * 100.0) as u32
            } else if ent.has_weapon {
                100
            } else {
                0
            },
            weapon_can_target_air: ent.weapon_can_target_air,
            weapon_can_target_ground: ent.weapon_can_target_ground,
            weapon_projectile_speed: ent.weapon_projectile_speed,
            armed_riders_upgrade_weapon_set: ent.armed_riders_upgrade_weapon_set,
            weapon_set_player_upgrade: ent.weapon_set_player_upgrade,
            second_life: ent.second_life,
            front_crushed: ent.front_crushed,
            back_crushed: ent.back_crushed,
            user_1: ent.user_1,
            user_2: ent.user_2,
            weapon_crate_upgrade: ent.weapon_crate_upgrade,
            armor_crate_upgrade: ent.armor_crate_upgrade,
            enemy_near: ent.enemy_near,
            armed: ent.armed,
            camo_stealth_look: ent.camo_stealth_look,
            // Wave 490: disguise/detector/stealth-break presentation from GW entity.
            disguise_as_template: if ent.disguise_as_template.is_empty() {
                None
            } else {
                Some(ent.disguise_as_template.clone())
            },
            disguise_as_team: match ent.disguise_as_team_ordinal {
                0 => Some(Team::USA),
                1 => Some(Team::China),
                2 => Some(Team::GLA),
                _ => None,
            },
            disguised: ent.disguised,
            disabled_subdued: ent.disabled_subdued,
            is_carbomb: ent.is_carbomb,
            weapon_set_carbomb: false,
            bomb_type: 0,
            bomb_timer_seconds: 0,
            hijacked: ent.hijacked,
            disguise_transition_opacity: 1.0,
            detection_range: ent.detection_range,
            detection_rate_frames: ent.detection_rate_frames,
            stealth_breaks_on_attack: ent.stealth_breaks_on_attack,
            stealth_breaks_on_move: ent.stealth_breaks_on_move,
            innate_stealth: ent.innate_stealth,
            // Wave 490: continuous-fire / battle-plan timers from GW entity.
            weapon_bonus_frenzy_until_frame: ent.weapon_bonus_frenzy_until_frame,
            continuous_fire_consecutive: ent.continuous_fire_consecutive,
            continuous_fire_coast_until_frame: ent.continuous_fire_coast_until_frame,
            battle_plan_sight_scalar_applied: ent.battle_plan_sight_scalar_applied,
            special_power_ready: ent.special_power_ready,
            special_power_cooldown: ent.special_power_cooldown,
            special_power_cooldown_remaining: ent.special_power_cooldown_remaining,
            object_type: match ent.object_type_ordinal {
                0 => PresentationObjectType::Infantry,
                1 => PresentationObjectType::Vehicle,
                2 => PresentationObjectType::Aircraft,
                3 => PresentationObjectType::Building,
                4 => PresentationObjectType::Supply,
                5 => PresentationObjectType::Projectile,
                _ => PresentationObjectType::Neutral,
            },
            // Wave 490: applied upgrades from GW entity.
            applied_upgrades: ent.applied_upgrade_names.clone(),
            sub_object_visibility: {
                use crate::game_logic::host_sub_objects_upgrade::{
                    HostSubObjectVisibility, sub_objects_for_upgrade_tags,
                };
                let tags: std::collections::HashSet<String> =
                    ent.applied_upgrade_names.iter().cloned().collect();
                let applied = sub_objects_for_upgrade_tags(&tags, &ent.template.name);
                let mut vis = HostSubObjectVisibility::default();
                if applied.matched {
                    vis.apply_show_hide(&applied.show, &applied.hide);
                }
                vis
            },

            upgrade_cameo_names: {
                #[cfg(feature = "game_client")]
                {
                    game_client::gui::control_bar::leftover_authored_upgrade_cameo_names(
                        &ent.template.name,
                    )
                }
                #[cfg(not(feature = "game_client"))]
                {
                    Default::default()
                }
            },
            has_secondary_weapon: ent.has_secondary_weapon,
            secondary_weapon_range: ent.secondary_weapon_range,
            secondary_weapon_damage: ent.secondary_weapon_damage,
            // Wave 490: turret presentation from GW entity.
            turret_angle_deg: ent.turret_angle_deg,
            turret_pitch_deg: ent.turret_pitch_deg,
            turret_idle_scanning: ent.turret_idle_scanning,
            weapon_bonus_enthusiastic: ent.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: ent.weapon_bonus_subliminal,
            weapon_bonus_horde: ent.weapon_bonus_horde,
            weapon_bonus_nationalism: ent.weapon_bonus_nationalism,
            weapon_bonus_frenzy: ent.weapon_bonus_frenzy,
            // Wave 490: bonus/ai/hive presentation from GW entity.
            weapon_bonus_frenzy_level: ent.weapon_bonus_frenzy_level,
            weapon_bonus_battle_plan_bombardment: ent.weapon_bonus_battle_plan_bombardment,
            weapon_bonus_battle_plan_hold_the_line: ent.weapon_bonus_battle_plan_hold_the_line,
            weapon_bonus_battle_plan_search_and_destroy: ent
                .weapon_bonus_battle_plan_search_and_destroy,
            continuous_fire_level: ent.continuous_fire_level,
            faerie_fire_until_frame: ent.faerie_fire_until_frame,
            hive_slave_count: ent.hive_slave_count,
            hive_slave_hp: ent.hive_slave_hp,
            ai_attitude: ent.ai_attitude,
            camo_friendly_opacity: ent.camo_friendly_opacity,
            vision_spied_mask: ent.vision_spied_mask,
            vision_range: ent.vision_range,
            shroud_clearing_range: ent.shroud_clearing_range,
            crusher_level: ent.crusher_level,
            crushable_level: ent.crushable_level,
            cheer_timer: ent.cheer_timer,
            // Wave 490: transport/container role presentation from GW entity.
            is_humvee_transport: ent.is_humvee_transport,
            is_listening_outpost_transport: ent.is_listening_outpost_transport,
            is_troop_crawler_transport: ent.is_troop_crawler_transport,
            is_helix_transport: ent.is_helix_transport,
            has_overlord_gattling_addon: ent.has_overlord_gattling_addon,
            has_overlord_propaganda_addon: ent.has_overlord_propaganda_addon,
            is_battle_bus_transport: ent.is_battle_bus_transport,
            is_technical_transport: ent.is_technical_transport,
            is_combat_cycle_transport: ent.is_combat_cycle_transport,
            combat_cycle_rider: ent.combat_cycle_rider,
            is_tunnel_network: ent.is_tunnel_network,
            is_combat_chinook_transport: ent.is_combat_chinook_transport,
            max_transport: ent.max_transport as usize,
            overlord_bunker_capacity: if ent.overlord_bunker_capacity == u16::MAX {
                0
            } else {
                ent.overlord_bunker_capacity as usize
            },
            // GameWorld has not retained generic Object INI ContainModule
            // metadata yet.  Preserve only explicitly mirrored host roles;
            // arbitrary vehicles remain fail-closed for normal RMB Enter.
            contain_module_present: false,
            contain_module_kind: crate::game_logic::ContainModuleKind::None,
            contain_admission: crate::game_logic::ContainAdmission::Unsupported,
            rider_change_allowed_templates: Vec::new(),
            contain_allow_allies_inside: true,
            contain_allow_enemies_inside: true,
            contain_allow_neutral_inside: true,
            transport_slot_count: 0,
            is_faction_structure: false,
            passengers_allowed_to_fire: ent.passengers_allowed_to_fire,
            display_name: ent.template.name.clone(),
            // Wave 490: demo/turret-hold/command-set presentation from GW entity.
            demo_suicided_detonating: ent.demo_suicided_detonating,
            turret_holding: ent.turret_holding,
            last_damage_source_host: ent.last_damage_source_host,
            command_set_override: ent.command_set_override.clone(),
            // Wave 493: effective command set name falls back to override residual.
            command_set_name: ent.command_set_override.clone(),
            is_detector: ent.is_detector,
            active_weapon_slot: ent.active_weapon_slot,
            weapon_fire_status: ent.weapon_fire_status,
            is_panicking: ent.is_panicking,
            moving_backwards: ent.moving_backwards,
            overcharge_enabled: ent.overcharge_enabled,
            // GameWorld does not retain Main's typed Behavior metadata.  A
            // standalone overlay therefore fails closed; a frame built from
            // live GameLogic already carries the frozen source capability.
            can_toggle_overcharge: false,
            shock_was_airborne: ent.shock_was_airborne,
            shock_allow_bounce: ent.shock_allow_bounce,
            shock_grounded_once: ent.shock_grounded_once,
            shock_stun_frames: ent.shock_stun_frames,
            power_plant_rods_extended: ent.power_plant_rods_extended,
            power_plant_rods_done_frame: ent.power_plant_rods_done_frame,
            jet_slow_death_active: ent.jet_slow_death_active,
            anim_steer_turn: ent.anim_steer_turn,
            show_health_bar: true,
            // Wave 490: guard/mine/kind presentation from GW entity.
            guard_radius: ent.guard_radius,
            has_mine: ent.has_mine_data,
            kind_of: Self::kind_of_list_from_presentation_bits(ent.kind_of_bits),
            is_structure: matches!(ent.object_type_ordinal, 3) || ent.is_building,
            is_unit: matches!(ent.object_type_ordinal, 0 | 1 | 2),
            is_mobile: matches!(ent.object_type_ordinal, 0 | 1 | 2),
            safe_occlusion_frame: 0,

            can_produce: ent.is_building && !ent.under_construction,
            // Wave 490: building type from GW entity ordinal.
            building_type: PresentationBuildingType::from_ordinal(ent.building_type_ordinal),
            // GameWorld-primary path: preserve every exact source-authored
            // Draw model by Object identity. Later presentation/render code
            // must not apply suffix-based reselection.
            model_key,
            draw_models,
            // GameWorld carries the originating template's authored Object INI
            // scale.  Keep it frozen instead of recovering scale from a unit
            // name compatibility table.
            mesh_scale: if ent.mesh_scale.is_finite() && ent.mesh_scale > 0.0 {
                ent.mesh_scale
            } else {
                1.0
            },
            selection_radius: if ent.selection_radius > 0.0 {
                ent.selection_radius
            } else {
                10.0
            },
            health_box_width: {
                let r = if ent.selection_radius > 0.0 {
                    ent.selection_radius
                } else {
                    10.0
                };
                let size = (r + r).min(150.0).max(20.0);
                (size * 2.0).max(20.0)
            },
            health_box_z_offset: {
                let r = if ent.selection_radius > 0.0 {
                    ent.selection_radius
                } else {
                    10.0
                };
                r + 10.0
            },
            max_height_above_position: 0.0,

            // Wave 493: engine-bridge + ground height from GW entity residual.
            engine_bridged: ent.engine_bridged,
            fow_visibility: {
                // Entity FOW floats: alpha≈1 visible; explored-but-low alpha → fogged; else hidden.
                if ent.fow_visibility_alpha >= 0.95 {
                    crate::fow_rendering::ObjectVisibility::FULLY_VISIBLE
                } else if ent.fow_is_explored >= 0.5 || ent.fow_visibility_alpha > 0.05 {
                    crate::fow_rendering::ObjectVisibility {
                        visibility_alpha: ent.fow_visibility_alpha.clamp(0.0, 1.0),
                        is_explored: ent.fow_is_explored.clamp(0.0, 1.0),
                        visibility_falloff: ent.fow_visibility_falloff.clamp(0.0, 1.0).max(0.01),
                    }
                } else {
                    crate::fow_rendering::ObjectVisibility::HIDDEN
                }
            },
            // A GameWorld entity carries only scalar FOW presentation data;
            // it has no direct Drawable lifetime or raw ObjectShroudStatus.
            // Keep it unknown until the host overlay below can freeze those
            // exact facts for the matching object.
            drawable_shroud: PresentationDrawableShroudFacts::default(),
            ground_height: if ent.ground_height_from_terrain {
                ent.ground_height
            } else if ent.ground_height.abs() > 1e-6 {
                ent.ground_height
            } else {
                PRESENTATION_DEFAULT_GROUND_HEIGHT
            },
            ground_height_from_terrain: ent.ground_height_from_terrain,
            drawable_fade_mode: 0,
            drawable_fade_start_frame: 0,
            drawable_fade_frames: 0,
            gaining_subdual: ent.disabled_subdued,
            drawable_explicit_opacity: 1.0,
            camo_heat_vision_opacity: 0.0,
        }
    }

    /// Append sparse RenderableObjects for GameWorld entities not already on the
    /// host-built frame (Wave 192). Uses host ObjectId when the shadow map has one;
    /// otherwise synthesizes `ObjectId(0x8000_0000 | entity_id)`.
    ///
    /// Call after `overlay_gameworld_shadow`. Counts land in `gameworld_appended`.
    /// Fail-closed: not full `build_from_gameworld` cutover / playable_claim.
    pub fn append_missing_from_gameworld(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        let existing: std::collections::HashSet<u32> =
            self.objects.iter().map(|o| o.id.0).collect();
        let mut appended = 0usize;
        for ent in shadow.world().world().entities() {
            if ent.destroyed && ent.health <= 0.0 {
                continue;
            }
            let host_id = shadow
                .host_for_entity(ent.id)
                .unwrap_or_else(|| crate::game_logic::ObjectId(0x8000_0000 | ent.id.get()));
            if existing.contains(&host_id.0) {
                continue;
            }
            self.objects
                .push(Self::renderable_from_gameworld_entity(host_id, ent));
            appended += 1;
        }
        self.gameworld_appended = self.gameworld_appended.saturating_add(appended);
        appended
    }

    /// Wave 500: merge per-object damage/death/bone FX residual names into the
    /// presentation particle list so client/upload observe named FXLists without
    /// a live GameLogic dual-read during render.
    ///
    /// Host drain currently queues `FX:{name}` audio tags; this peels the same
    /// names into `PresentationParticleSystem` with `fx_list_name` set.
    /// Fail-closed: not full FXList.ini particle graph / bone-local offsets.
    pub fn append_object_residual_fx_particles(&mut self) -> usize {
        use crate::game_logic::CombatParticleKind;
        let frame = self.frame.0;
        let mut next_id = self
            .particle_systems
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        let mut appended = 0usize;
        for o in &self.objects {
            let candidates: [(Option<&str>, CombatParticleKind); 3] = [
                (
                    o.damage_fx_name.as_deref(),
                    CombatParticleKind::WeaponImpact,
                ),
                (
                    o.death_fx_name.as_deref(),
                    CombatParticleKind::DeathExplosion,
                ),
                (o.bone_fx_name.as_deref(), CombatParticleKind::WeaponImpact),
            ];
            for (name, kind) in candidates {
                let Some(name) = name else {
                    continue;
                };
                if name.is_empty() {
                    continue;
                }
                let already = self.particle_systems.iter().any(|p| {
                    (!p.fx_list_name.is_empty() && p.fx_list_name == name)
                        || (p.template_name == name && p.source_object == Some(o.id))
                });
                if already {
                    continue;
                }
                self.particle_systems.push(PresentationParticleSystem {
                    id: next_id,
                    kind,
                    template_name: name.to_string(),
                    position: o.position,
                    source_object: Some(o.id),
                    target_object: None,
                    spawned_frame: frame,
                    active: true,
                    client_system_id: None,
                    fx_list_name: name.to_string(),
                    ocl_list_name: String::new(),
                });
                next_id = next_id.saturating_add(1).max(1);
                appended += 1;
            }
        }
        if appended > 0 {
            // Keep dual-tick particle_count honest with expanded list.
            self.dual_tick.particle_count = self.particle_systems.len() as u32;
        }
        appended
    }

    /// Wave 498: re-stamp host-only FX presentation residual after GameWorld object rebuild.
    ///
    /// GW entities do not yet own TransitionDamageFX / BoneFX / poison tint / defector /
    /// death FX / topple lean. When objects are rebuilt from the entity store, those
    /// fields would otherwise hard-default. Overlay from the matching host Object by id.
    /// Fail-closed: not full GameWorld FX ownership / playable_claim.
    pub fn overlay_host_fx_residual(&mut self, logic: &GameLogic) -> usize {
        let mut stamped = 0usize;
        for ro in &mut self.objects {
            let Some(obj) = logic.host_object(ro.id) else {
                continue;
            };
            let mut dirty = false;
            if ro.special_power_override_destination != obj.special_power_override_destination {
                ro.special_power_override_destination = obj.special_power_override_destination;
                dirty = true;
            }
            if ro.owner_player_id != obj.owner_player_id {
                ro.owner_player_id = obj.owner_player_id;
                dirty = true;
            }
            let drawable_shroud = super::build::freeze_direct_object_shroud_facts(
                obj,
                self.local_player_id,
                self.fow_shell_bypass,
            );
            if ro.drawable_shroud != drawable_shroud {
                ro.drawable_shroud = drawable_shroud;
                dirty = true;
            }
            let can_disguise_as_team = super::build::freeze_direct_can_disguise_as_team(obj);
            if ro.can_disguise_as_team != can_disguise_as_team {
                ro.can_disguise_as_team = can_disguise_as_team;
                dirty = true;
            }
            let friendly_stealth_opacity = super::build::freeze_friendly_stealth_opacity(obj);
            if (ro.friendly_stealth_opacity - friendly_stealth_opacity).abs() > f32::EPSILON {
                ro.friendly_stealth_opacity = friendly_stealth_opacity;
                dirty = true;
            }
            let friendly_stealth_opacity_max = obj.thing.template.stealth_friendly_opacity_max;
            if (ro.friendly_stealth_opacity_max - friendly_stealth_opacity_max).abs() > f32::EPSILON
            {
                ro.friendly_stealth_opacity_max = friendly_stealth_opacity_max;
                dirty = true;
            }
            if ro.drawable_fade_mode != obj.drawable_fade_mode {
                ro.drawable_fade_mode = obj.drawable_fade_mode;
                dirty = true;
            }
            if ro.drawable_fade_start_frame != obj.drawable_fade_start_frame {
                ro.drawable_fade_start_frame = obj.drawable_fade_start_frame;
                dirty = true;
            }
            let gaining_subdual = obj.subdual_damage > 0.0;
            if ro.gaining_subdual != gaining_subdual {
                ro.gaining_subdual = gaining_subdual;
                dirty = true;
            }
            if ro.disabled_freefall != obj.status.disabled_freefall {
                ro.disabled_freefall = obj.status.disabled_freefall;
                dirty = true;
            }
            if ro.disabled_default != obj.status.disabled_default {
                ro.disabled_default = obj.status.disabled_default;
                dirty = true;
            }
            if ro.disabled_script_underpowered != obj.status.disabled_script_underpowered {
                ro.disabled_script_underpowered = obj.status.disabled_script_underpowered;
                dirty = true;
            }
            if ro.disabled_script_disabled != obj.status.disabled_script_disabled {
                ro.disabled_script_disabled = obj.status.disabled_script_disabled;
                dirty = true;
            }
            if ro.weapon_set_carbomb != obj.weapon_set_carbomb {
                ro.weapon_set_carbomb = obj.weapon_set_carbomb;
                dirty = true;
            }
            let (bomb_type, bomb_timer_seconds) =
                super::build::freeze_sticky_bomb_overlay(obj, logic.get_current_frame() as u32);
            if ro.bomb_type != bomb_type || ro.bomb_timer_seconds != bomb_timer_seconds {
                ro.bomb_type = bomb_type;
                ro.bomb_timer_seconds = bomb_timer_seconds;
                dirty = true;
            }
            // GameWorld is the object-roster authority in the active frame,
            // but it has not yet retained parsed ContainModule metadata.  Copy
            // the immutable host containment snapshot at frame construction
            // time so physical RMB remains presentation-only and does not
            // invent a transport from vehicle geometry.
            let contain_kind = obj.thing.template.contain_module.kind;
            let contain_present = contain_kind != crate::game_logic::ContainModuleKind::None;
            if ro.contain_module_present != contain_present {
                ro.contain_module_present = contain_present;
                dirty = true;
            }
            if ro.contain_module_kind != contain_kind {
                ro.contain_module_kind = contain_kind;
                dirty = true;
            }
            let contain_admission = obj.normal_enter_admission();
            if ro.contain_admission != contain_admission {
                ro.contain_admission = contain_admission;
                dirty = true;
            }
            let rider_change_allowed_templates: Vec<String> = obj
                .thing
                .template
                .contain_module
                .rider_change_riders
                .iter()
                .filter(|rider| rider.physical_enter_supported)
                .map(|rider| rider.template_name.clone())
                .collect();
            if ro.rider_change_allowed_templates != rider_change_allowed_templates {
                ro.rider_change_allowed_templates = rider_change_allowed_templates;
                dirty = true;
            }
            let allow_allies = obj.thing.template.contain_module.allow_allies_inside;
            if ro.contain_allow_allies_inside != allow_allies {
                ro.contain_allow_allies_inside = allow_allies;
                dirty = true;
            }
            let allow_enemies = obj.thing.template.contain_module.allow_enemies_inside;
            if ro.contain_allow_enemies_inside != allow_enemies {
                ro.contain_allow_enemies_inside = allow_enemies;
                dirty = true;
            }
            let allow_neutral = obj.thing.template.contain_module.allow_neutral_inside;
            if ro.contain_allow_neutral_inside != allow_neutral {
                ro.contain_allow_neutral_inside = allow_neutral;
                dirty = true;
            }
            let transport_slots = obj.transport_slot_count();
            if ro.transport_slot_count != transport_slots {
                ro.transport_slot_count = transport_slots;
                dirty = true;
            }
            let faction_structure = obj.is_faction_structure();
            if ro.is_faction_structure != faction_structure {
                ro.is_faction_structure = faction_structure;
                dirty = true;
            }
            let contained = obj.contained_units();
            if ro.garrisoned_units != contained {
                ro.garrisoned_units = contained;
                dirty = true;
            }
            let max_transport = obj.transport_capacity();
            if ro.max_transport != max_transport {
                ro.max_transport = max_transport;
                dirty = true;
            }
            let max_garrison = obj.garrison_capacity();
            if ro.max_garrison != max_garrison {
                ro.max_garrison = max_garrison;
                dirty = true;
            }
            let overlord_capacity = obj.overlord_bunker_capacity.unwrap_or(usize::MAX);
            if ro.overlord_bunker_capacity != overlord_capacity {
                ro.overlord_bunker_capacity = overlord_capacity;
                dirty = true;
            }
            let tunnel_network = obj.is_tunnel_network_style_container();
            if ro.is_tunnel_network != tunnel_network {
                ro.is_tunnel_network = tunnel_network;
                dirty = true;
            }
            // These are explicit specialized host containment roles, not
            // template-name discovery.  Stamp them with the same immutable
            // snapshot as the parsed module so GameWorld-primary presentation
            // can still classify an implemented Humvee/Helix/etc. correctly.
            macro_rules! stamp_containment_role {
                ($field:ident) => {
                    if ro.$field != obj.$field {
                        ro.$field = obj.$field;
                        dirty = true;
                    }
                };
            }
            stamp_containment_role!(is_humvee_transport);
            stamp_containment_role!(is_listening_outpost_transport);
            stamp_containment_role!(is_troop_crawler_transport);
            stamp_containment_role!(is_helix_transport);
            stamp_containment_role!(is_battle_bus_transport);
            stamp_containment_role!(is_technical_transport);
            stamp_containment_role!(is_combat_cycle_transport);
            stamp_containment_role!(is_combat_chinook_transport);
            let is_structure = obj.is_kind_of(crate::game_logic::KindOf::Structure);
            if ro.is_structure != is_structure {
                ro.is_structure = is_structure;
                dirty = true;
            }
            let topple = obj.presentation_topple_lean_radians();
            if (ro.topple_lean_radians - topple).abs() > 1e-5 {
                ro.topple_lean_radians = topple;
                dirty = true;
            }
            let (fy, fp) = obj
                .float_update
                .as_ref()
                .map(|f| (f.yaw, f.pitch))
                .unwrap_or((0.0, 0.0));
            if (ro.float_yaw - fy).abs() > 1e-5 || (ro.float_pitch - fp).abs() > 1e-5 {
                ro.float_yaw = fy;
                ro.float_pitch = fp;
                dirty = true;
            }
            let (tdx, tdy) = obj.presentation_topple_dir();
            if (ro.topple_dir_x - tdx).abs() > 1e-5 || (ro.topple_dir_y - tdy).abs() > 1e-5 {
                ro.topple_dir_x = tdx;
                ro.topple_dir_y = tdy;
                dirty = true;
            }
            let shadows = obj.presentation_shadows_enabled();
            if ro.shadows_enabled != shadows {
                ro.shadows_enabled = shadows;
                dirty = true;
            }
            if ro.terrain_decal_type != obj.terrain_decal_type
                || (ro.terrain_decal_size - obj.terrain_decal_size).abs() > 1e-5
                || (ro.terrain_decal_opacity - obj.terrain_decal_opacity).abs() > 1e-5
            {
                ro.terrain_decal_type = obj.terrain_decal_type;
                ro.terrain_decal_size = obj.terrain_decal_size;
                ro.terrain_decal_opacity = obj.terrain_decal_opacity;
                dirty = true;
            }
            let damage_fx = obj
                .pending_transition_damage_fx
                .last()
                .and_then(|e| e.fx_name.clone());
            if ro.damage_fx_name != damage_fx {
                ro.damage_fx_name = damage_fx;
                dirty = true;
            }
            let bone_fx = obj.bone_fx_damage.as_ref().and_then(|b| b.last_fx.clone());
            if ro.bone_fx_name != bone_fx {
                ro.bone_fx_name = bone_fx;
                dirty = true;
            }
            let poison = obj.is_poison_tinted();
            if ro.poison_tinted != poison {
                ro.poison_tinted = poison;
                dirty = true;
            }
            let undetected = obj.is_undetected_defector();
            if ro.undetected_defector != undetected {
                ro.undetected_defector = undetected;
                dirty = true;
            }
            let flash = obj
                .defection_helper
                .as_ref()
                .map(|d| d.flash_this_frame || d.final_white_flash)
                .unwrap_or(false);
            if ro.defector_flash != flash {
                ro.defector_flash = flash;
                dirty = true;
            }
            let death_fx = obj.pending_death_fx.clone();
            if ro.death_fx_name != death_fx {
                ro.death_fx_name = death_fx;
                dirty = true;
            }
            let is_crate = obj.is_kind_of(crate::game_logic::KindOf::Crate)
                || logic.host_money_crates.contains(obj.id);
            if ro.is_crate != is_crate {
                ro.is_crate = is_crate;
                dirty = true;
            }
            let is_salvage_crate = logic
                .host_money_crates
                .get(obj.id)
                .is_some_and(|entry| entry.is_salvage)
                || obj.template_name.eq_ignore_ascii_case("SalvageCrate");
            if ro.is_salvage_crate != is_salvage_crate {
                ro.is_salvage_crate = is_salvage_crate;
                dirty = true;
            }
            if ro.sub_object_visibility != obj.sub_object_visibility {
                ro.sub_object_visibility = obj.sub_object_visibility.clone();
                dirty = true;
            }
            if ro.selection_flash_color != obj.selection_flash_color {
                ro.selection_flash_color = obj.selection_flash_color;
                dirty = true;
            }
            if ro.selection_flash_remaining != obj.selection_flash_remaining {
                ro.selection_flash_remaining = obj.selection_flash_remaining;
                dirty = true;
            }

            if dirty {
                stamped += 1;
            }
        }
        stamped
    }

    /// Rebuild the entire `objects` list from the GameWorld entity store (Wave 193).
    ///
    /// Host ObjectIds are preferred when the shadow map has them; otherwise
    /// synthesizes `ObjectId(0x8000_0000 | entity_id)`. Counts land in
    /// `gameworld_rebuilt`. Default engine path when shadow is live (Wave 194).
    /// Fail-closed: sparse host-only FX/UI fields stay default unless host merge
    /// fills them; not full playable_claim cutover.
    pub fn rebuild_objects_from_gameworld(
        &mut self,
        shadow: &crate::gameworld_shadow::GameWorldShadow,
    ) -> usize {
        self.objects.clear();
        let mut n = 0usize;
        for ent in shadow.world().world().entities() {
            if ent.destroyed && ent.health <= 0.0 {
                continue;
            }
            let host_id = shadow
                .host_for_entity(ent.id)
                .unwrap_or_else(|| crate::game_logic::ObjectId(0x8000_0000 | ent.id.get()));
            self.objects
                .push(Self::renderable_from_gameworld_entity(host_id, ent));
            n += 1;
        }
        self.gameworld_rebuilt = n;
        self.gameworld_primary_objects = n > 0 || self.gameworld_primary_objects;
        // Overlay last-writer stamps player residual + any fields the sparse builder
        // still defaults (keeps one code path for shadow → presentation identity).
        let _ = self.overlay_gameworld_shadow(shadow);
        n
    }

    /// Build a PresentationFrame whose **object roster** is GameWorld-primary (Wave 193).
    ///
    /// When `host` is provided, non-object presentation residual (world_env, scripts,
    /// camera, FX packs) still comes from `build_from_logic`, then objects are rebuilt
    /// from the shadow. When `host` is `None`, a minimal shell frame is filled with
    /// GameWorld objects + local player residual only.
    ///
    /// Default engine path (Wave 194) rebuilds objects from GameWorld after host
    /// non-object residual. Fail-closed: not full authority cutover / playable_claim.
    pub fn build_from_gameworld(
        shadow: &crate::gameworld_shadow::GameWorldShadow,
        local_player_id: u32,
        host: Option<&GameLogic>,
    ) -> Self {
        let mut frame = if let Some(logic) = host {
            Self::build_from_logic(logic, local_player_id)
        } else {
            // Minimal shell — borrow-first empty presentation with local player id set.
            let mut f = Self::build_from_logic(&GameLogic::new(), local_player_id);
            f.objects.clear();
            f.events.clear();
            f
        };
        let host_n = frame.objects.len();
        let gw_n = frame.rebuild_objects_from_gameworld(shadow);
        // Wave 838: keep host objects when shadow yields nothing.
        if gw_n == 0 && host_n > 0 {
            if let Some(logic) = host {
                frame = Self::build_from_logic(logic, local_player_id);
                let _ = frame.overlay_gameworld_shadow(shadow);
            }
        }
        // Wave 498: host FX residual survives GameWorld object rebuild.
        if let Some(logic) = host {
            let _ = frame.overlay_host_fx_residual(logic);
        }
        // Wave 500: object FX residual names → particle list after host FX stamp.
        let _ = frame.append_object_residual_fx_particles();
        // Local player residual already stamped by overlay inside rebuild.
        frame
    }

    /// Engine-only GameWorld roster build retaining the revision-frozen terrain
    /// payload from the host presentation cache.
    pub(crate) fn build_from_gameworld_with_runtime_heightmap(
        shadow: &crate::gameworld_shadow::GameWorldShadow,
        local_player_id: u32,
        host: Option<&GameLogic>,
        runtime_heightmap: Option<std::sync::Arc<PresentationRuntimeHeightmap>>,
    ) -> Self {
        let mut frame = if let Some(logic) = host {
            Self::build_from_logic_with_runtime_heightmap(
                logic,
                local_player_id,
                runtime_heightmap.clone(),
            )
        } else {
            // A no-host shell frame cannot reuse a host terrain payload.
            let mut f = Self::build_from_logic_with_runtime_heightmap(
                &GameLogic::new(),
                local_player_id,
                None,
            );
            f.objects.clear();
            f.events.clear();
            f
        };
        let host_n = frame.objects.len();
        let gw_n = frame.rebuild_objects_from_gameworld(shadow);
        // Wave 838: keep host objects when shadow yields nothing.
        if gw_n == 0 && host_n > 0 {
            if let Some(logic) = host {
                frame = Self::build_from_logic_with_runtime_heightmap(
                    logic,
                    local_player_id,
                    runtime_heightmap,
                );
                let _ = frame.overlay_gameworld_shadow(shadow);
            }
        }
        // Wave 498: host FX residual survives GameWorld object rebuild.
        if let Some(logic) = host {
            let _ = frame.overlay_host_fx_residual(logic);
        }
        // Wave 500: object FX residual names → particle list after host FX stamp.
        let _ = frame.append_object_residual_fx_particles();
        frame
    }

    /// Standard engine presentation build (Wave 195).
    ///
    /// When a live `GameWorldShadow` is present and
    /// [`presentation_from_gameworld_enabled`] is true (default), this is equivalent to
    /// [`build_from_gameworld`] — host supplies non-object residual, objects come from
    /// the borrow-first entity store. Otherwise falls back to host
    /// [`build_from_logic`] plus overlay/append when a shadow is available.
    ///
    /// Callers must `sync_from_host` before this when a shadow is provided.
    /// Fail-closed: not full GameWorld authority cutover / playable_claim.
    pub fn build_for_engine(
        logic: &GameLogic,
        local_player_id: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
    ) -> Self {
        match shadow {
            Some(shadow) if presentation_from_gameworld_enabled() => {
                Self::build_from_gameworld(shadow, local_player_id, Some(logic))
            }
            Some(shadow) => {
                let mut frame = Self::build_from_logic(logic, local_player_id);
                let _ = frame.overlay_gameworld_shadow(shadow);
                let _ = frame.append_missing_from_gameworld(shadow);
                frame
            }
            None => Self::build_from_logic(logic, local_player_id),
        }
    }

    /// Engine-only presentation build retaining one cached full terrain payload
    /// for every frame in a terrain revision.
    pub(crate) fn build_for_engine_with_runtime_heightmap(
        logic: &GameLogic,
        local_player_id: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
        runtime_heightmap: Option<std::sync::Arc<PresentationRuntimeHeightmap>>,
    ) -> Self {
        match shadow {
            Some(shadow) if presentation_from_gameworld_enabled() => {
                Self::build_from_gameworld_with_runtime_heightmap(
                    shadow,
                    local_player_id,
                    Some(logic),
                    runtime_heightmap,
                )
            }
            Some(shadow) => {
                let mut frame = Self::build_from_logic_with_runtime_heightmap(
                    logic,
                    local_player_id,
                    runtime_heightmap,
                );
                let _ = frame.overlay_gameworld_shadow(shadow);
                let _ = frame.append_missing_from_gameworld(shadow);
                frame
            }
            None => Self::build_from_logic_with_runtime_heightmap(
                logic,
                local_player_id,
                runtime_heightmap,
            ),
        }
    }

    /// Engine tick presentation build with victory residual (Wave 195).
    ///
    /// Freezes victory via [`build_with_victory`], then applies the standard
    /// GameWorld object roster path when a shadow is live.
    pub fn build_with_victory_for_engine(
        logic: &mut GameLogic,
        local_player_id: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
    ) -> Self {
        let mut frame = Self::build_with_victory(logic, local_player_id);
        match shadow {
            Some(shadow) if presentation_from_gameworld_enabled() => {
                let host_n = frame.objects.len();
                let gw_n = frame.rebuild_objects_from_gameworld(shadow);
                // Wave 838: empty GameWorld shadow must not erase a non-empty host
                // roster (construct/train/map objects) or unit mesh collect stays 0.
                if gw_n == 0 && host_n > 0 {
                    frame = Self::build_with_victory(logic, local_player_id);
                    let _ = frame.overlay_gameworld_shadow(shadow);
                }
                // Wave 498: host FX residual after GW object rebuild.
                let _ = frame.overlay_host_fx_residual(logic);
                // Wave 500: object FX residual names → particle list.
                let _ = frame.append_object_residual_fx_particles();
            }
            Some(shadow) => {
                let _ = frame.overlay_gameworld_shadow(shadow);
                let _ = frame.append_missing_from_gameworld(shadow);
            }
            None => {}
        }
        frame
    }

    /// Engine-only victory variant carrying the revision-frozen terrain payload.
    pub(crate) fn build_with_victory_for_engine_with_runtime_heightmap(
        logic: &mut GameLogic,
        local_player_id: u32,
        shadow: Option<&crate::gameworld_shadow::GameWorldShadow>,
        runtime_heightmap: Option<std::sync::Arc<PresentationRuntimeHeightmap>>,
    ) -> Self {
        let mut frame = Self::build_with_victory_with_runtime_heightmap(
            logic,
            local_player_id,
            runtime_heightmap.clone(),
        );
        match shadow {
            Some(shadow) if presentation_from_gameworld_enabled() => {
                let host_n = frame.objects.len();
                let gw_n = frame.rebuild_objects_from_gameworld(shadow);
                // Wave 838: empty GameWorld shadow must not erase a non-empty host
                // roster (construct/train/map objects) or unit mesh collect stays 0.
                if gw_n == 0 && host_n > 0 {
                    frame = Self::build_with_victory_with_runtime_heightmap(
                        logic,
                        local_player_id,
                        runtime_heightmap,
                    );
                    let _ = frame.overlay_gameworld_shadow(shadow);
                }
                // Wave 498: host FX residual after GW object rebuild.
                let _ = frame.overlay_host_fx_residual(logic);
                // Wave 500: object FX residual names → particle list.
                let _ = frame.append_object_residual_fx_particles();
            }
            Some(shadow) => {
                let _ = frame.overlay_gameworld_shadow(shadow);
                let _ = frame.append_missing_from_gameworld(shadow);
            }
            None => {}
        }
        frame
    }
}

#[cfg(test)]
mod overlay_sw_owner_tests {
    use super::*;
    use crate::game_logic::{GameLogic, Player, Team};
    use crate::gameworld_shadow::GameWorldShadow;

    #[test]
    fn overlay_shared_cooldown_uses_owning_player_slot() {
        // C++ SpecialPowerModule.cpp:756-765: SharedNSync remaining is the
        // owning player's ready frame, not the local player's.
        let mut logic = GameLogic::new();
        let mut local = Player::new(0, Team::USA, "USA", true);
        local.alliance_team = 0;
        logic.add_player(local);
        let mut enemy = Player::new(1, Team::China, "China", false);
        enemy.alliance_team = 1;
        logic.add_player(enemy);

        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        if let Some(p) = shadow
            .world_mut()
            .player_mut(gamelogic::world::PlayerId::from_index(0u8))
        {
            p.shared_special_power_cooldowns = vec![("ParticleCannon".into(), 11.0)];
        }
        if let Some(p) = shadow
            .world_mut()
            .player_mut(gamelogic::world::PlayerId::from_index(1u8))
        {
            p.shared_special_power_cooldowns = vec![("ParticleCannon".into(), 77.0)];
        }

        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        frame.superweapon_timers.clear();
        frame.superweapon_timers.push(PresentationSuperweaponTimer {
            name: "PUC".into(),
            template_name: "T".into(),
            icon: "I".into(),
            recharge_time: 300.0,
            remaining: 1.0,
            unlocked: true,
            ready: false,
            power_key: "ParticleCannon".into(),
        });
        frame.superweapon_timers.push(PresentationSuperweaponTimer {
            name: "PUC (Enemy #CC2222)".into(),
            template_name: "T2".into(),
            icon: "I2".into(),
            recharge_time: 300.0,
            remaining: 1.0,
            unlocked: true,
            ready: false,
            power_key: "ParticleCannon#1".into(),
        });
        let _ = frame.overlay_gameworld_shadow(&shadow);
        let local = frame
            .superweapon_timers
            .iter()
            .find(|t| t.power_key == "ParticleCannon")
            .expect("local");
        assert!(
            (local.remaining - 11.0).abs() < 1e-5,
            "local remaining {}",
            local.remaining
        );
        let enemy = frame
            .superweapon_timers
            .iter()
            .find(|t| t.power_key == "ParticleCannon#1")
            .expect("enemy");
        assert!(
            (enemy.remaining - 77.0).abs() < 1e-5,
            "enemy remaining must come from player 1 shared frame, got {}",
            enemy.remaining
        );
    }
}

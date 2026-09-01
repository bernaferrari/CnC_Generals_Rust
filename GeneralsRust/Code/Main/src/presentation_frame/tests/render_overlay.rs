#[test]
fn unit_render_input_world_matrix_applies_mesh_scale() {
    use super::super::*;
    use glam::Vec3;
    let mut u = UnitRenderInput {
        id: ObjectId(1),
        template_name: "T".into(),
        model_key: "M".into(),
        draw_models: Vec::new(),
        projectile_clip_statuses: [None; 3],
        mesh_scale: 2.0,
        team: Team::USA,
        team_color: [1.0, 1.0, 1.0, 1.0],
        position: Vec3::new(10.0, 0.0, 20.0),
        orientation: 0.0,
        topple_lean_radians: 0.0,
        topple_dir_x: 1.0,
        topple_dir_y: 0.0,
        shadows_enabled: true,
        terrain_decal_type: 8,
        terrain_decal_size: 0.0,
        terrain_decal_opacity: 0.0,
        turret_angle_deg: 0.0,
        turret_pitch_deg: 0.0,
        selected: false,
        selection_radius: 5.0,
        selection_flash_remaining: 0,
        selection_flash_color: None,

        model_condition_bits: 0,
        production_door_phase: 0,
        is_structure: false,
        is_unit: true,
        moving: false,
        attacking: false,
        is_firing_weapon: false,
        active_weapon_slot: 0,
        weapon_fire_status: 0,
        is_panicking: false,
        moving_backwards: false,
        weapon_set_player_upgrade: false,
        second_life: false,
        front_crushed: false,
        back_crushed: false,
        user_1: false,
        user_2: false,
        weapon_crate_upgrade: 0,
        armor_crate_upgrade: 0,
        enemy_near: false,
        armed: false,
        shock_was_airborne: false,
        shock_allow_bounce: false,
        shock_grounded_once: false,
        shock_stun_frames: 0,
        power_plant_rods_extended: false,
        power_plant_rods_done_frame: 0,
        jet_slow_death_active: false,
        anim_steer_turn: 0,
        body_damage_state: 0,
        poison_tinted: false,
        defector_flash: false,
        is_deployed: false,
        radar_active: false,
        radar_extend_complete: false,
        effectively_stealthed: false,
        under_construction: false,
        construction_percent: 0.0,
        max_height_above_position: 0.0,

        disguised: false,
        disguise_as_template: None,
        occupant_count: 0,
        ai_state_ordinal: 0,
        combat_cycle_rider: 0,
        contained_by: None,
        parachuting: false,
        using_ability: false,
        airborne_target: false,
        object_type: PresentationObjectType::Neutral,
        velocity: Vec3::ZERO,
        veterancy: PresentationVeterancy::Rookie,
        over_water: false,
        cell_is_cliff: false,
        cell_is_underwater: false,
        disabled: false,
        parachute_open: false,
        world_is_snow: false,
        object_weather: 0,
        world_is_night: false,

        captured: false,
        overcharge_enabled: false,
        death_type_name: String::new(),
        continuous_fire_level: 0,
        prone: false,
        jammed: false,
        destroyed: false,
        continuous_fire_coast_until_frame: 0,
        logic_frame: 0,
        is_surrendered: false,
        engine_bridged: false,
        fow_visibility: ObjectVisibility::FULLY_VISIBLE,
        presentation_opacity: 1.0,
        second_material_pass_opacity: 0.0,
        status_tint: [0.0; 3],
        stored_supplies: 0,
        drawable_supply_boxes: 0,
        drawable_supply_max_boxes: 0,
        dock_kind: crate::game_logic::DockKind::None,
        drawable_shroud: PresentationDrawableShroudFacts::default(),
        sub_object_visibility: Default::default(),
    };
    let m = u.world_matrix();
    // Column-major: scale is on the diagonal of the upper 3x3 after T*R*S.
    let sx = m.x_axis.truncate().length();
    let sy = m.y_axis.truncate().length();
    let sz = m.z_axis.truncate().length();
    assert!((sx - 2.0).abs() < 1e-4 && (sy - 2.0).abs() < 1e-4 && (sz - 2.0).abs() < 1e-4);
    assert!((m.w_axis.x - 10.0).abs() < 1e-4 && (m.w_axis.z - 20.0).abs() < 1e-4);

    u.mesh_scale = 0.0; // invalid → treat as 1.0
    let m1 = u.world_matrix();
    assert!((m1.x_axis.truncate().length() - 1.0).abs() < 1e-4);
}

#[test]
fn aflame_bits_come_from_host_not_death_type_name() {
    use crate::game_logic::host_enum_table_residual::{aflame_model_bit, burned_model_bit};
    let flame = 1u128 << aflame_model_bit();
    let burn = 1u128 << burned_model_bit();
    let mut u = unit_render_input_fixture();
    u.model_condition_bits = flame;
    u.death_type_name.clear();
    u.destroyed = false;
    let bits = u.model_condition_bits_with_combat_flags();
    assert_ne!(
        bits & flame,
        0,
        "live AFLAME must survive empty death_type_name"
    );
    assert_eq!(bits & burn, 0, "death name must not invent BURNED");

    u.model_condition_bits = 0;
    u.death_type_name = "DEATH_BURNED".into();
    u.destroyed = true;
    let bits = u.model_condition_bits_with_combat_flags();
    assert_eq!(bits & flame, 0, "death type name must not stamp AFLAME");
}

#[test]
fn jet_afterburner_bits_survive_presentation_and_exhaust_not_invented() {
    use crate::game_logic::host_enum_table_residual::{
        jetafterburner_model_bit, jetexhaust_model_bit,
    };
    use crate::presentation_frame::PresentationObjectType;
    let ab = 1u128 << jetafterburner_model_bit();
    let ex = 1u128 << jetexhaust_model_bit();
    let mut u = unit_render_input_fixture();
    u.object_type = PresentationObjectType::Aircraft;
    u.model_condition_bits = ab;
    u.moving = true;
    u.airborne_target = true;
    u.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    u.jet_slow_death_active = false;
    let bits = u.model_condition_bits_with_combat_flags();
    assert_ne!(bits & ab, 0, "takeoff JETAFTERBURNER must not be wiped");
    assert_eq!(bits & ex, 0, "presentation must not invent JETEXHAUST");
    u.jet_slow_death_active = true;
    u.model_condition_bits = 0;
    let crash = u.model_condition_bits_with_combat_flags();
    assert_ne!(crash & ab, 0, "crash still stamps JETAFTERBURNER");
}

#[test]
fn topple_world_matrix_falls_along_crush_direction() {
    let mut u = unit_render_input_fixture();
    u.mesh_scale = 1.0;
    u.orientation = 0.0;
    u.topple_lean_radians = 0.4;
    u.topple_dir_x = 0.0;
    u.topple_dir_y = 1.0;
    let along_z = u.world_matrix();
    u.topple_dir_x = 1.0;
    u.topple_dir_y = 0.0;
    let along_x = u.world_matrix();
    assert!(
        (along_z.x_axis - along_x.x_axis).length() > 0.05
            || (along_z.z_axis - along_x.z_axis).length() > 0.05,
        "crush direction must change the fall axis"
    );
}

fn unit_render_input_fixture() -> UnitRenderInput {
    UnitRenderInput {
        id: ObjectId(1),
        template_name: "T".into(),
        model_key: "M".into(),
        draw_models: Vec::new(),
        projectile_clip_statuses: [None; 3],
        mesh_scale: 1.0,
        team: Team::USA,
        team_color: [1.0, 1.0, 1.0, 1.0],
        position: glam::Vec3::ZERO,
        orientation: 0.0,
        topple_lean_radians: 0.0,
        topple_dir_x: 1.0,
        topple_dir_y: 0.0,
        shadows_enabled: true,
        terrain_decal_type: 8,
        terrain_decal_size: 0.0,
        terrain_decal_opacity: 0.0,
        turret_angle_deg: 0.0,
        turret_pitch_deg: 0.0,
        selected: false,
        selection_radius: 5.0,
        selection_flash_remaining: 0,
        selection_flash_color: None,

        model_condition_bits: 0,
        production_door_phase: 0,
        is_structure: false,
        is_unit: true,
        moving: false,
        attacking: false,
        is_firing_weapon: false,
        active_weapon_slot: 0,
        weapon_fire_status: 0,
        is_panicking: false,
        moving_backwards: false,
        weapon_set_player_upgrade: false,
        second_life: false,
        front_crushed: false,
        back_crushed: false,
        user_1: false,
        user_2: false,
        weapon_crate_upgrade: 0,
        armor_crate_upgrade: 0,
        enemy_near: false,
        armed: false,
        shock_was_airborne: false,
        shock_allow_bounce: false,
        shock_grounded_once: false,
        shock_stun_frames: 0,
        power_plant_rods_extended: false,
        power_plant_rods_done_frame: 0,
        jet_slow_death_active: false,
        anim_steer_turn: 0,
        body_damage_state: 0,
        poison_tinted: false,
        defector_flash: false,
        is_deployed: false,
        radar_active: false,
        radar_extend_complete: false,
        effectively_stealthed: false,
        under_construction: false,
        construction_percent: 0.0,
        max_height_above_position: 0.0,

        disguised: false,
        disguise_as_template: None,
        occupant_count: 0,
        ai_state_ordinal: 0,
        combat_cycle_rider: 0,
        contained_by: None,
        parachuting: false,
        using_ability: false,
        airborne_target: false,
        object_type: PresentationObjectType::Neutral,
        velocity: glam::Vec3::ZERO,
        veterancy: PresentationVeterancy::Rookie,
        over_water: false,
        cell_is_cliff: false,
        cell_is_underwater: false,
        disabled: false,
        parachute_open: false,
        world_is_snow: false,
        object_weather: 0,
        world_is_night: false,
        captured: false,
        overcharge_enabled: false,
        death_type_name: String::new(),
        continuous_fire_level: 0,
        prone: false,
        jammed: false,
        destroyed: false,
        continuous_fire_coast_until_frame: 0,
        logic_frame: 0,
        is_surrendered: false,
        engine_bridged: false,
        fow_visibility: ObjectVisibility::FULLY_VISIBLE,
        presentation_opacity: 1.0,
        second_material_pass_opacity: 0.0,
        status_tint: [0.0; 3],
        stored_supplies: 0,
        drawable_supply_boxes: 0,
        drawable_supply_max_boxes: 0,
        dock_kind: crate::game_logic::DockKind::None,
        drawable_shroud: PresentationDrawableShroudFacts::default(),
        sub_object_visibility: Default::default(),
    }
}

#[test]
fn viewer_relative_stealth_matches_cxx_allies_and_inactive_local_rules() {
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut local = Player::new(0, Team::USA, "Local", true);
    local.alliance_team = 7;
    let mut allied = Player::new(1, Team::China, "Allied", false);
    allied.alliance_team = 7;
    let mut enemy = Player::new(2, Team::GLA, "Enemy", false);
    enemy.alliance_team = 8;
    logic.add_player(local);
    logic.add_player(allied);
    logic.add_player(enemy);

    let mut template = ThingTemplate::new("ViewerRelativeStealthUnit");
    template.set_health(100.0);
    logic
        .templates
        .insert("ViewerRelativeStealthUnit".into(), template);
    let allied_id = logic
        .create_object(
            "ViewerRelativeStealthUnit",
            Team::China,
            glam::Vec3::new(2.0, 0.0, 2.0),
        )
        .expect("allied object");
    let enemy_id = logic
        .create_object(
            "ViewerRelativeStealthUnit",
            Team::GLA,
            glam::Vec3::new(4.0, 0.0, 2.0),
        )
        .expect("enemy object");
    for (id, owner) in [(allied_id, 1), (enemy_id, 2)] {
        let object = logic.host_object_mut(id).expect("host object");
        object.owner_player_id = Some(owner);
        object.status.stealthed = true;
        object.status.detected = false;
        object.apply_stealth_update_pulse();
    }

    let active = PresentationFrame::build_from_logic(&logic, 0);
    let active_allied = active
        .objects
        .iter()
        .find(|object| object.id == allied_id)
        .expect("allied frozen object");
    let active_enemy = active
        .objects
        .iter()
        .find(|object| object.id == enemy_id)
        .expect("enemy frozen object");
    assert!(active_allied.effectively_stealthed && active_enemy.effectively_stealthed);
    assert!(
        !active.local_viewer_hides_stealthed(active_allied)
            && active.local_viewer_hides_stealthed(active_enemy),
        "C++ StealthUpdate keeps allied stealth visible and hides an undetected non-ally"
    );
    let active_inputs = active.unit_render_inputs();
    assert!(active_inputs.iter().any(|input| input.id == allied_id));
    assert!(
        !active_inputs.iter().any(|input| input.id == enemy_id),
        "the scene sync and ordinary WGPU mesh gate share the same invisible-enemy decision"
    );
    let allied_input = active_inputs
        .iter()
        .find(|input| input.id == allied_id)
        .expect("allied translucent input");
    let expected_allied = logic
        .host_object(allied_id)
        .map(|o| o.camo_friendly_opacity)
        .unwrap_or(1.0);
    assert!(
        (allied_input.fow_visibility.visibility_alpha - 1.0).abs() < f32::EPSILON
            && (allied_input.presentation_opacity - expected_allied).abs() < 0.001,
        "C++ VISIBLE_FRIENDLY uses host StealthUpdate pulse opacity"
    );

    logic
        .get_object_mut(allied_id)
        .expect("allied object")
        .status
        .detected = true;
    let detected_frame = PresentationFrame::build_from_logic(&logic, 0);
    let detected_input = detected_frame
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == allied_id)
        .expect("detected friendly stealth remains renderable");
    let expected_detected = logic
        .host_object(allied_id)
        .map(|o| o.camo_friendly_opacity)
        .unwrap_or(1.0);
    assert!(
        (detected_input.fow_visibility.visibility_alpha - 1.0).abs() < f32::EPSILON
            && (detected_input.presentation_opacity - expected_detected).abs() < 0.001,
        "C++ VISIBLE_FRIENDLY_DETECTED keeps the same host pulse"
    );

    let mut dead_enemy = active_enemy.clone();
    dead_enemy.drawable_shroud.effectively_dead = true;
    assert!(
        !active.local_viewer_hides_stealthed(&dead_enemy),
        "C++ StealthUpdate returns StealthLook::None before evaluating relations for dead objects"
    );

    logic.get_player_mut(1).expect("allied player").is_alive = false;
    let dead_allied_owner = PresentationFrame::build_from_logic(&logic, 0);
    let allied_object = dead_allied_owner
        .objects
        .iter()
        .find(|object| object.id == allied_id)
        .expect("allied frozen object with a dead owner");
    assert!(
        !dead_allied_owner.local_viewer_hides_stealthed(allied_object),
        "C++ Team::getRelationship does not make an allied object invisible because its owner died"
    );

    // C++ forces an inactive observer/dead local player to ALLIES for this
    // visual relationship. Do not reuse generic selection/ownership logic.
    logic.get_player_mut(0).expect("local player").is_alive = false;
    let inactive = PresentationFrame::build_from_logic(&logic, 0);
    assert!(!inactive.local_is_alive);
    for id in [allied_id, enemy_id] {
        let object = inactive
            .objects
            .iter()
            .find(|object| object.id == id)
            .expect("inactive frozen object");
        assert!(
            !inactive.local_viewer_hides_stealthed(object),
            "an inactive local viewer must not turn any stealthed object invisible"
        );
    }
    let inactive_inputs = inactive.unit_render_inputs();
    assert!(
        [allied_id, enemy_id]
            .into_iter()
            .all(|id| inactive_inputs.iter().any(|input| input.id == id)),
        "inactive local visual relation keeps both stealth objects in the mesh roster"
    );
}

#[test]
fn friendly_stealth_opacity_pulses_across_logic_frames() {
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    let mut template = ThingTemplate::new("PulseStealthUnit");
    template.set_health(100.0);
    logic.templates.insert("PulseStealthUnit".into(), template);
    let id = logic
        .create_object(
            "PulseStealthUnit",
            Team::USA,
            glam::Vec3::new(3.0, 0.0, 3.0),
        )
        .expect("unit");
    {
        let object = logic.host_object_mut(id).expect("host");
        object.owner_player_id = Some(0);
        object.status.stealthed = true;
        object.status.detected = false;
    }
    logic.frame = 0;
    let a = PresentationFrame::build_from_logic(&logic, 0)
        .unit_render_inputs()
        .into_iter()
        .find(|u| u.id == id)
        .expect("frame 0");
    logic.frame = 8;
    let b = PresentationFrame::build_from_logic(&logic, 0)
        .unit_render_inputs()
        .into_iter()
        .find(|u| u.id == id)
        .expect("frame 8");
    assert!(
        (a.presentation_opacity - b.presentation_opacity).abs() > 0.01,
        "friendly stealth must shimmer, not sit at a static min"
    );
    assert!(a.presentation_opacity >= 0.5 && a.presentation_opacity <= 1.0);
    assert!(b.presentation_opacity >= 0.5 && b.presentation_opacity <= 1.0);
}

#[test]
fn ocl_fade_in_multiplies_presentation_opacity() {
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    logic.frame = 10;
    let mut template = ThingTemplate::new("FadeSpawn");
    template.set_health(10.0);
    logic.templates.insert("FadeSpawn".into(), template);
    let id = logic
        .create_object("FadeSpawn", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("spawn");
    logic
        .host_object_mut(id)
        .expect("obj")
        .start_drawable_fade_in(10, 10);
    let input = PresentationFrame::build_from_logic(&logic, 0)
        .unit_render_inputs()
        .into_iter()
        .find(|u| u.id == id)
        .expect("fade input");
    assert!(
        input.presentation_opacity < 0.01,
        "C++ fadeIn starts at explicit opacity 0"
    );
}

#[test]
fn bomb_truck_prehalfpoint_disguise_stays_visible_opaque_through_gameworld_rebuild() {
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "Local", true));
    logic.add_player(Player::new(1, Team::GLA, "Enemy", false));

    let mut template = ThingTemplate::new("GLAVehicleBombTruck");
    template.set_health(100.0);
    logic
        .templates
        .insert("GLAVehicleBombTruck".into(), template);
    let id = logic
        .create_object(
            "GLAVehicleBombTruck",
            Team::GLA,
            glam::Vec3::new(4.0, 0.0, 2.0),
        )
        .expect("Bomb Truck object");
    {
        let object = logic.host_object_mut(id).expect("host Bomb Truck");
        object.owner_player_id = Some(1);
        // Presentation's mutable Object template bookkeeping must not affect
        // this source capability; C++ reads the immutable ThingTemplate's
        // StealthUpdate data instead.
        object.template_name = "PoisonedRuntimeName".into();
        object.apply_disguise("TestTank", Team::USA);
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // `build_from_gameworld` is the default engine object's roster path. It
    // first rebuilds from GameWorld, then the host overlay must restore the
    // immutable source capability.
    let frame = PresentationFrame::build_from_gameworld(&shadow, 0, Some(&logic));
    assert!(frame.gameworld_primary_objects);
    let object = frame
        .objects
        .iter()
        .find(|object| object.id == id)
        .expect("rebuilt Bomb Truck");
    assert_eq!(object.template_name, "PoisonedRuntimeName");
    assert!(object.effectively_stealthed);
    assert!(object.can_disguise_as_team);
    assert!(
        !frame.local_viewer_hides_stealthed(object),
        "C++ canDisguise returns StealthLook::None before the visual disguise halfpoint"
    );
    let input = frame
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == id)
        .expect("pre-halfpoint Bomb Truck remains renderable");
    assert!(
        (input.fow_visibility.visibility_alpha - 1.0).abs() < f32::EPSILON,
        "StealthLook::None stays opaque; it is not the friendly-stealth alpha path"
    );
}

#[test]
fn completed_allied_disguise_uses_the_direct_visual_template_after_gameworld_rebuild() {
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;

    let mut logic = GameLogic::new();
    let mut local = Player::new(0, Team::USA, "Local", true);
    local.alliance_team = 9;
    let mut allied_truck_owner = Player::new(1, Team::GLA, "Allied Truck", false);
    allied_truck_owner.alliance_team = 9;
    logic.add_player(local);
    logic.add_player(allied_truck_owner);

    let mut truck_template = ThingTemplate::new("GLAVehicleBombTruck");
    truck_template.set_health(100.0);
    logic
        .templates
        .insert("GLAVehicleBombTruck".into(), truck_template);
    let mut disguise_template = ThingTemplate::new("FriendlyDisguiseAppearance");
    disguise_template.set_asset_scale(0.7);
    logic
        .templates
        .insert("FriendlyDisguiseAppearance".into(), disguise_template);
    let id = logic
        .create_object(
            "GLAVehicleBombTruck",
            Team::GLA,
            glam::Vec3::new(4.0, 0.0, 2.0),
        )
        .expect("Bomb Truck object");
    {
        let object = logic.host_object_mut(id).expect("host Bomb Truck");
        object.owner_player_id = Some(1);
        object.apply_disguise("FriendlyDisguiseAppearance", Team::USA);
        for _ in 0..30 {
            object.tick_disguise_transition();
        }
        assert!(object.status.disguised, "reached visual disguise halfpoint");
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let frame = PresentationFrame::build_from_gameworld(&shadow, 0, Some(&logic));
    let object = frame
        .objects
        .iter()
        .find(|object| object.id == id)
        .expect("rebuilt Bomb Truck");
    assert!(frame.is_allied_with_local(object));
    let direct = frame
        .direct_host_drawables
        .iter()
        .find(|direct| direct.object.id == id)
        .expect("resident direct drawable");
    assert_eq!(direct.visual_template_name, "FriendlyDisguiseAppearance");
    assert!(
        (direct.visual_mesh_scale - 0.7).abs() < f32::EPSILON,
        "C++ visual replacement must carry the disguise template's asset scale"
    );
    let input = frame
        .unit_render_inputs()
        .into_iter()
        .find(|input| input.id == id)
        .expect("allied disguised mesh input");
    assert_eq!(
        input.template_name, "FriendlyDisguiseAppearance",
        "direct Drawable visual identity is independent of the viewer relationship"
    );
    assert!(
        (input.mesh_scale - 0.7).abs() < f32::EPSILON,
        "direct visual overlay must replace source scale before frustum/world-matrix use"
    );
    assert!(
        (input.world_matrix().x_axis.truncate().length() - 0.7).abs() < f32::EPSILON,
        "the replacement scale must reach the final world matrix, not just metadata"
    );
}

#[test]
fn drawable_shroud_facts_stay_frozen_and_host_overlay_stamps_gameworld_records() {
    use crate::game_logic::{GameLogic, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

    let mut logic = GameLogic::new();
    apply_skirmish_config(&mut logic, &golden_skirmish_config("DrawableShroudFreeze"))
        .expect("config");
    let mut template = ThingTemplate::new("DrawableShroudFreezeUnit");
    template.set_health(100.0);
    logic
        .templates
        .insert("DrawableShroudFreezeUnit".into(), template);
    let id = logic
        .create_object(
            "DrawableShroudFreezeUnit",
            Team::USA,
            glam::Vec3::new(2.0, 0.0, 2.0),
        )
        .expect("object");
    {
        let obj = logic.host_object_mut(id).expect("host object");
        // C++ own-force/no-partition source is Clear; effective death remains
        // a separate exact fact for the client-owned grace limit.
        obj.owner_player_id = Some(0);
        obj.status.effectively_dead = true;
    }

    let frozen = PresentationFrame::build_from_logic(&logic, 0);
    let frozen_object = frozen
        .objects
        .iter()
        .find(|object| object.id == id)
        .unwrap();
    assert_eq!(
        frozen_object.drawable_shroud.lifetime,
        PresentationDrawableLifetime::DirectHostObject
    );
    assert_eq!(
        frozen_object.drawable_shroud.raw_status,
        PresentationObjectShroudStatus::Clear
    );
    assert!(frozen_object.drawable_shroud.effectively_dead);
    assert_eq!(
        frozen_object.drawable_shroud.direct_game_client_status(),
        Some((gamelogic::common::types::ObjectShroudStatus::Clear, true))
    );

    logic
        .host_object_mut(id)
        .expect("host object")
        .status
        .effectively_dead = false;
    assert!(
        frozen
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .drawable_shroud
            .effectively_dead,
        "an installed presentation frame must not reread live GameLogic"
    );
    let fresh = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        !fresh
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .drawable_shroud
            .effectively_dead,
        "the next frame captures the new host value"
    );
    let mut changed_status = frozen.clone();
    changed_status
        .objects
        .iter_mut()
        .find(|object| object.id == id)
        .unwrap()
        .drawable_shroud
        .raw_status = PresentationObjectShroudStatus::Fogged;
    assert_ne!(
        frozen.presentation_hash(),
        changed_status.presentation_hash(),
        "raw shroud facts are part of deterministic presentation identity"
    );

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let mut gameworld_only = PresentationFrame::build_from_gameworld(&shadow, 0, None);
    let gameworld_object = gameworld_only
        .objects
        .iter()
        .find(|object| object.id == id)
        .expect("GameWorld object");
    assert_eq!(
        gameworld_object.drawable_shroud,
        PresentationDrawableShroudFacts::default(),
        "GameWorld scalar FOW cannot manufacture direct drawable facts"
    );
    assert!(
        !gameworld_object
            .drawable_shroud
            .requires_scene_shroud_material()
    );

    assert!(gameworld_only.overlay_host_fx_residual(&logic) >= 1);
    let host_stamped = gameworld_only
        .objects
        .iter()
        .find(|object| object.id == id)
        .expect("host-stamped object");
    assert_eq!(
        host_stamped.drawable_shroud.lifetime,
        PresentationDrawableLifetime::DirectHostObject
    );
    assert_eq!(
        host_stamped.drawable_shroud.raw_status,
        PresentationObjectShroudStatus::Clear
    );
    assert!(!host_stamped.drawable_shroud.effectively_dead);
}

#[test]
fn direct_host_shroud_facts_use_raw_membership_not_visibility_alpha() {
    use crate::game_logic::{GameLogic, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use gamelogic::system::shroud_manager::get_shroud_manager;

    let _shroud_test_guard = crate::fow_rendering::shroud_test_isolation_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut logic = GameLogic::new();
    apply_skirmish_config(
        &mut logic,
        &golden_skirmish_config("DirectShroudMembership"),
    )
    .expect("config");
    let mut template = ThingTemplate::new("DirectShroudMembershipUnit");
    template.set_health(100.0);
    logic
        .templates
        .insert("DirectShroudMembershipUnit".into(), template);
    let id = logic
        .create_object(
            "DirectShroudMembershipUnit",
            Team::GLA,
            glam::Vec3::new(3.0, 0.0, 3.0),
        )
        .expect("object");
    logic
        .host_object_mut(id)
        .expect("host object")
        .owner_player_id = Some(1);

    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(500.0, 500.0);
        shroud.mark_host_object_seen(0, id.0);
    }
    assert_eq!(
        PresentationFrame::build_from_logic(&logic, 0)
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .drawable_shroud
            .raw_status,
        PresentationObjectShroudStatus::Clear
    );

    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(500.0, 500.0);
        shroud.mark_host_object_seen(0, id.0);
        // Explored membership persists while current visible membership drops:
        // this is the raw Fogged branch, not a visibility-alpha threshold.
        shroud.clear_host_object_visibility(0);
    }
    assert_eq!(
        PresentationFrame::build_from_logic(&logic, 0)
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .drawable_shroud
            .raw_status,
        PresentationObjectShroudStatus::Fogged
    );

    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(500.0, 500.0);
        // Keep the FOW runtime active for the viewer but leave this object
        // absent from both raw membership sets.
        shroud.mark_host_object_seen(0, 0x00ff_0001);
    }
    assert_eq!(
        PresentationFrame::build_from_logic(&logic, 0)
            .objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .drawable_shroud
            .raw_status,
        PresentationObjectShroudStatus::Shrouded
    );

    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

#[test]
fn overlay_gameworld_shadow_copies_entity_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OverlayShadowRes");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("OvlU") {
        let mut t = ThingTemplate::new("OvlU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("OvlU".into(), t);
    }
    let id = logic
        .create_object("OvlU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    {
        use crate::game_logic::Weapon;
        let obj = logic./* Wave 948 */ /* Wave 950 */ host_object_mut(id).expect("obj");
        obj.selected = true;
        obj.status.stealthed = true;
        obj.status.detected = false;
        obj.command_set_override = Some("Command_ShadowOvl".into());
        obj.is_detector = true;
        obj.weapon = Some(Weapon {
            damage: 15.0,
            range: 120.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
        obj.force_attack = true;
        obj.show_health_bar = false;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Poison host after sync — overlay must use shadow residual.
    if let Some(obj) = logic.host_object_mut(id) {
        obj.position = glam::Vec3::new(999.0, 0.0, 999.0);
        obj.selected = false;
        obj.command_set_override = None;
        obj.status.stealthed = false;
    }
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    // Host freeze has poisoned values; overlay restores shadow.
    let n = frame.overlay_gameworld_shadow(&shadow);
    assert!(n >= 1, "overlay must update at least one object");
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!(
        (ro.position.x - 2.0).abs() < 0.1,
        "shadow pose wins, got {:?}",
        ro.position
    );
    assert!(ro.selected, "shadow selected residual");
    assert!(ro.stealthed && !ro.detected && ro.effectively_stealthed);
    assert_eq!(ro.command_set_override, "Command_ShadowOvl");
    assert!(ro.is_detector && ro.force_attack);
    assert!(!ro.show_health_bar);
    assert!((ro.weapon_range - 120.0).abs() < 0.01);
    // Deeper residual fields present in overlay path (source honesty).
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("obj.command_set_override = ent.command_set_override.clone()")
            && src.contains("obj.selected = ent.selected")
            && src.contains("obj.turret_angle_deg = ent.turret_angle_deg")
            && src.contains("obj.hive_slave_count = ent.hive_slave_count")
            && src.contains("obj.weapon_bonus_horde = ent.weapon_bonus_horde")
            && src.contains("obj.path_waypoints = path_wp")
            && src.contains("obj.has_mine = ent.has_mine_data")
            && src.contains("obj.garrisoned_units = garrisoned")
            && src.contains("obj.contained_by = contained")
            && src.contains("ent.kind_of_bits")
            && src.contains("ent.applied_upgrade_names")
            && src.contains("ent.production_queue_items")
            && src.contains("obj.template_name = ent.template.name.clone()")
            && src.contains("obj.disguise_as_team = disguise_team")
            && src.contains("ent.model_key")
            && src.contains("ent.mesh_scale")
            && src.contains("ent.fow_visibility_alpha")
            && src.contains("ent.ground_height")
            && src.contains("ent.engine_bridged")
            && src.contains("is_battle_bus_transport")
            && src.contains("ent.display_name")
            && src.contains("ent.weapon_min_range")
            && src.contains("ent.weapon_ammo")
            && src.contains("ent.guard_target_host")
            && src.contains("ent.ai_state_ordinal")
            && src.contains("ent.path_len")
            && src.contains("ent.occupant_count")
            && src.contains("shadow last-writer residual"),
        "overlay must copy expanded entity residual"
    );
}

#[test]
fn gameworld_primary_presentation_retains_unattackable_weaponset_override() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use crate::unit_control::UnitControlSystem;

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OverlayUnattackable");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut template = ThingTemplate::new("UnattackableVictim");
    template.set_health(100.0);
    template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Unattackable);
    logic
        .templates
        .insert("UnattackableVictim".into(), template);
    let id = logic
        .create_object(
            "UnattackableVictim",
            Team::GLA,
            glam::Vec3::new(2.0, 0.0, 2.0),
        )
        .expect("object");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let frame = PresentationFrame::build_from_gameworld(&shadow, 0, Some(&logic));
    let object = frame.objects.iter().find(|object| object.id == id).unwrap();
    assert!(object.unattackable);
    assert!(!UnitControlSystem::presentation_is_attackable(object));
}

#[test]
fn overlay_gameworld_shadow_applies_local_economy_power() {
    use crate::game_logic::GameLogic;
    use crate::gameworld_shadow::GameWorldShadow;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    // Ensure economy authority path is on for this process (gate default).
    crate::gameworld_shadow::ensure_gate_damage_authority();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OverlayEconPower");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Host starting cash from golden config.
    let host_cash = logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert!(host_cash > 0, "host must have cash");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Shadow last-writer: diverge economy/power and player residual from host.
    if let Some(p) = shadow
        .world_mut()
        .player_mut(gamelogic::world::PlayerId::from_index(0u8))
    {
        p.supplies = host_cash.saturating_add(1234);
        p.power_available = 77;
        p.power_produced = 88;
        p.power_consumed = 11;
        p.is_alive = false;
        p.radar_count = 3;
        p.radar_disabled = true;
        p.cash_bounty_percent = 0.25;
        p.color_rgb = (9, 8, 7);
        p.rank_level = 3;
        p.skill_points = 400;
        p.science_purchase_points = 2;
        p.unlocked_sciences = vec!["SCIENCE_TestRank".into()];
        p.shared_special_power_cooldowns =
            vec![("ParticleCannon".into(), 42.0), ("ScudStorm".into(), 0.0)];
    }
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.local_supplies, host_cash);
    let _ = frame.overlay_gameworld_shadow(&shadow);
    if crate::gameworld_shadow::gameworld_economy_authority_live() {
        assert_eq!(frame.local_supplies, host_cash.saturating_add(1234));
        assert_eq!(frame.local_power, 77);
        assert_eq!(frame.local_power_produced, 88);
        assert_eq!(frame.local_power_consumed, 11);
    }
    // Player residual always overlays from shadow when mapped.
    assert!(!frame.local_is_alive);
    assert_eq!(frame.local_radar_count, 3);
    assert!(frame.local_radar_disabled);
    assert!((frame.local_cash_bounty_percent - 0.25).abs() < 1e-5);
    assert_eq!(frame.local_color_rgb, (9, 8, 7));
    assert_eq!(frame.local_rank_level, 3);
    assert_eq!(frame.local_skill_points, 400);
    assert_eq!(frame.local_science_purchase_points, 2);
    assert!(
        frame
            .local_unlocked_sciences
            .iter()
            .any(|s| s == "SCIENCE_TestRank")
    );
    // Superweapon timers get remaining from shadow shared cooldowns by power_key.
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
        name: "SCUD".into(),
        template_name: "T2".into(),
        icon: "I2".into(),
        recharge_time: 360.0,
        remaining: 99.0,
        unlocked: true,
        ready: false,
        power_key: "ScudStorm".into(),
    });
    let _ = frame.overlay_gameworld_shadow(&shadow);
    let puc = frame
        .superweapon_timers
        .iter()
        .find(|t| t.power_key == "ParticleCannon")
        .expect("puc timer");
    assert!((puc.remaining - 42.0).abs() < 1e-5, "got {}", puc.remaining);
    assert!(!puc.ready);
    let scud = frame
        .superweapon_timers
        .iter()
        .find(|t| t.power_key == "ScudStorm")
        .expect("scud timer");
    assert!(scud.remaining <= 0.0, "got {}", scud.remaining);
    assert!(scud.ready);
    if let Some(pi) = frame.players.iter().find(|p| p.id == frame.local_player_id) {
        assert!(!pi.is_alive);
        assert_eq!(pi.color_rgb, (9, 8, 7));
    }
}

fn unit_display_info_carries_command_set_override_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HudCmdSetInfo");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("HudBarracks") {
        let mut t = ThingTemplate::new("HudBarracks");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("HudBarracks".into(), t);
    }
    let id = logic
        .create_object("HudBarracks", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    {
        use crate::game_logic::{BuildingData, BuildingType};
        let obj = logic.host_object_mut(id).expect("obj");
        obj.selected = true;
        obj.object_type = crate::game_logic::ObjectType::Building;
        obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
        obj.construction_percent = 1.0;
        obj.status.under_construction = false;
        obj.command_set_override = Some("CommandSetAmericaBarracks".into());
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let infos = frame.selected_unit_display_infos();
    let info = infos.iter().find(|i| i.object_id == id).expect("info");
    assert!(info.can_produce);
    assert_eq!(info.command_set_override, "CommandSetAmericaBarracks");
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("command_set_override: ro.command_set_override.clone()")
            && src.contains("can_produce: ro.can_produce"),
        "UnitDisplayInfo must freeze command_set/can_produce residual"
    );
}

fn presentation_freezes_detector_stealth_timing_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresDetStealth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PresDS") {
        let mut t = ThingTemplate::new("PresDS");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PresDS".into(), t);
    }
    let id = logic
        .create_object("PresDS", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.is_detector = true;
        obj.detection_range = 250.0;
        obj.detection_rate_frames = 15;
        obj.stealth_breaks_on_attack = true;
        obj.stealth_breaks_on_move = true;
        obj.innate_stealth = true;
        obj.weapon_bonus_frenzy_until_frame = 120;
        obj.continuous_fire_consecutive = 9;
        obj.continuous_fire_coast_until_frame = 40;
        obj.battle_plan_sight_scalar_applied = 1.25;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.innate_stealth_object_count(), 1);
    assert_eq!(frame.timed_detector_object_count(), 1);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!((ro.detection_range - 250.0).abs() < 0.01);
    assert_eq!(ro.detection_rate_frames, 15);
    assert!(ro.stealth_breaks_on_attack && ro.stealth_breaks_on_move && ro.innate_stealth);
    assert_eq!(ro.weapon_bonus_frenzy_until_frame, 120);
    assert_eq!(ro.continuous_fire_consecutive, 9);
    assert_eq!(ro.continuous_fire_coast_until_frame, 40);
    assert!((ro.battle_plan_sight_scalar_applied - 1.25).abs() < 0.01);
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("detection_rate_frames: obj.detection_rate_frames")
            && src.contains("innate_stealth: obj.innate_stealth")
            && src.contains("battle_plan_sight_scalar_applied: obj"),
        "freeze must copy detector/stealth timing residual"
    );
}

fn presentation_freezes_transport_kind_damage_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresTransportKind");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PresTK") {
        let mut t = ThingTemplate::new("PresTK");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("PresTK".into(), t);
    }
    let id = logic
        .create_object("PresTK", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    let src_id = logic
        .create_object("PresTK", Team::GLA, glam::Vec3::new(8.0, 0.0, 6.0))
        .expect("src");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.is_humvee_transport = true;
        obj.is_listening_outpost_transport = true;
        obj.is_troop_crawler_transport = true;
        obj.is_helix_transport = true;
        obj.has_overlord_gattling_addon = true;
        obj.has_overlord_propaganda_addon = true;
        obj.demo_suicided_detonating = true;
        obj.turret_holding = true;
        obj.last_damage_source = Some(src_id);
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.humvee_transport_object_count(), 1);
    assert_eq!(frame.overlord_gattling_object_count(), 1);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!(ro.is_humvee_transport && ro.is_helix_transport);
    assert!(ro.has_overlord_gattling_addon && ro.has_overlord_propaganda_addon);
    assert!(ro.demo_suicided_detonating && ro.turret_holding);
    assert_eq!(ro.last_damage_source_host, src_id.0);
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("is_humvee_transport: obj.is_humvee_transport")
            && src.contains("last_damage_source_host: obj.last_damage_source")
            && src.contains("has_overlord_gattling_addon: obj"),
        "freeze must copy transport-kind/damage residual"
    );
}

fn presentation_freezes_hive_continuous_camo_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresHiveCamo");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PresHC") {
        let mut t = ThingTemplate::new("PresHC");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PresHC".into(), t);
    }
    let id = logic
        .create_object("PresHC", Team::GLA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.continuous_fire_level = 2;
        obj.faerie_fire_until_frame = 77;
        obj.hive_slave_count = 3;
        obj.hive_slave_hp = 25.0;
        obj.ai_attitude = 1;
        obj.camo_friendly_opacity = 0.55;
        obj.vision_spied_mask = 0b110;
        obj.cheer_timer = 1.25;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.hive_object_count(), 1);
    assert_eq!(frame.continuous_fire_object_count(), 1);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert_eq!(ro.continuous_fire_level, 2);
    assert_eq!(ro.faerie_fire_until_frame, 77);
    assert_eq!(ro.hive_slave_count, 3);
    assert!((ro.hive_slave_hp - 25.0).abs() < 0.01);
    assert_eq!(ro.ai_attitude, 1);
    assert!((ro.camo_friendly_opacity - 0.55).abs() < 0.01);
    assert_eq!(ro.vision_spied_mask, 0b110);
    assert!((ro.cheer_timer - 1.25).abs() < 0.01);
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("hive_slave_count: obj.hive_slave_count")
            && src.contains("continuous_fire_level: obj.continuous_fire_level")
            && src.contains("camo_friendly_opacity: obj.camo_friendly_opacity"),
        "freeze must copy hive/continuous/camo residual"
    );
}

fn presentation_freezes_battle_plan_weapon_bonus_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresBattlePlan");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PresBP") {
        let mut t = ThingTemplate::new("PresBP");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PresBP".into(), t);
    }
    let id = logic
        .create_object("PresBP", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.weapon_bonus_battle_plan_bombardment = true;
        obj.weapon_bonus_battle_plan_hold_the_line = true;
        obj.weapon_bonus_battle_plan_search_and_destroy = true;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.battle_plan_bonus_object_count(), 1);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!(ro.weapon_bonus_battle_plan_bombardment);
    assert!(ro.weapon_bonus_battle_plan_hold_the_line);
    assert!(ro.weapon_bonus_battle_plan_search_and_destroy);
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("weapon_bonus_battle_plan_bombardment: obj")
            && src.contains("weapon_bonus_battle_plan_search_and_destroy"),
        "freeze must copy battle-plan bonus residual"
    );
}

fn apply_to_control_bar_syncs_command_set_from_presentation() {
    // Source honesty: apply path must call presentation command-set sync.
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("sync_command_set_from_presentation(self.selected_command_set_name())"),
        "apply_to_control_bar must feed ControlBar command-set residual"
    );
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    assert!(
        cb.contains("fn sync_command_set_from_presentation"),
        "ControlBar must expose presentation command-set residual"
    );
    assert!(
        cb.contains("Prefer this over live `OBJECT_REGISTRY`"),
        "must document OBJECT_REGISTRY dual-read avoidance"
    );
    // Runtime residual: selected override name is visible on the frame.
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CbCmdSetPres");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("CbCS") {
        let mut t = ThingTemplate::new("CbCS");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("CbCS".into(), t);
    }
    let id = logic
        .create_object("CbCS", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.selected = true;
        obj.command_set_override = Some("CommandSetAmericaDozer".into());
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(
        frame.selected_command_set_name(),
        Some("CommandSetAmericaDozer")
    );
}

fn presentation_freezes_command_set_detector_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresCmdDet");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PresCD") {
        let mut t = ThingTemplate::new("PresCD");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PresCD".into(), t);
    }
    let id = logic
        .create_object("PresCD", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.command_set_override = Some("Command_AmericaDozer".into());
        obj.is_detector = true;
        obj.active_weapon_slot = 1;
        obj.overcharge_enabled = true;
        obj.show_health_bar = false;
        obj.guard_radius = 175.0;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.detector_object_count(), 1);
    assert_eq!(frame.command_set_override_object_count(), 1);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert_eq!(ro.command_set_override, "Command_AmericaDozer");
    assert!(ro.is_detector);
    assert_eq!(ro.active_weapon_slot, 1);
    assert!(ro.overcharge_enabled);
    assert!(!ro.show_health_bar);
    assert!((ro.guard_radius - 175.0).abs() < 0.01);
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("command_set_override: obj")
            && src.contains("is_detector: obj.is_detector")
            && src.contains("guard_radius: obj.guard_radius"),
        "freeze must copy command-set/detector residual"
    );
}

fn presentation_freezes_turret_and_weapon_bonus_residual() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PresTurretBonus");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PresTB") {
        let mut t = ThingTemplate::new("PresTB");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PresTB".into(), t);
    }
    let id = logic
        .create_object("PresTB", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.turret_angle_deg = 33.0;
        obj.turret_pitch_deg = 12.0;
        obj.turret_idle_scanning = true;
        obj.weapon_bonus_enthusiastic = true;
        obj.weapon_bonus_horde = true;
        obj.weapon_bonus_frenzy = true;
        obj.weapon_bonus_frenzy_level = 2;
        obj.weapon_bonus_nationalism = true;
        obj.weapon_bonus_subliminal = true;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.turret_idle_scan_count(), 1);
    assert_eq!(frame.horde_bonus_object_count(), 1);
    let ro = frame.objects.iter().find(|o| o.id == id).expect("ro");
    assert!((ro.turret_angle_deg - 33.0).abs() < 0.01);
    assert!((ro.turret_pitch_deg - 12.0).abs() < 0.01);
    assert!(ro.turret_idle_scanning);
    assert!(ro.weapon_bonus_enthusiastic && ro.weapon_bonus_horde);
    assert_eq!(ro.weapon_bonus_frenzy_level, 2);
    let src = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    assert!(
        src.contains("turret_angle_deg: obj.turret_angle_deg")
            && src.contains("weapon_bonus_horde: obj.weapon_bonus_horde"),
        "freeze must copy turret/bonus residual"
    );
}

use super::super::*;
use crate::game_logic::{GameMode, KindOf, Player, ThingTemplate};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};

#[test]
fn upgrade_complete_freezes_into_presentation_events() {
    let mut logic = crate::game_logic::GameLogic::new();
    // Direct registry complete without full research path.
    let _ = logic
        .host_upgrades_mut()
        .record_complete("CaptureBuilding", 0, 1, 3);
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::UpgradeComplete {
                    name,
                    player_id: 0,
                    units_affected: 3,
                    ..
                } if name.to_ascii_lowercase().contains("capture")
            )
        }),
        "expected UpgradeComplete: {:?}",
        frame.events
    );
}

#[test]
fn apply_events_routes_upgrade_and_owner_to_hud() {
    let mut logic = crate::game_logic::GameLogic::new();
    let _ = logic
        .host_upgrades_mut()
        .record_complete("CaptureBuilding", 0, 1, 1);
    crate::game_logic::host_owner_log::clear();
    crate::game_logic::host_owner_log::record(
        crate::game_logic::ObjectId(3),
        crate::game_logic::Team::GLA,
    );
    let _ = crate::game_logic::host_owner_log::drain();
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    let mut hud = crate::ui::GameHUD::new();
    let before = hud.message_count_for_test();
    frame.apply_events_to_game_hud(&mut hud);
    assert_eq!(
        hud.message_count_for_test(),
        before,
        "upgrade/owner must not print overlay text (C++ audio/radar only)"
    );

    frame.events.push(PresentationEvent::RadarMessage {
        team: crate::game_logic::Team::Neutral,
        text: "RADAR:UnitUnderAttack".into(),
        position: glam::Vec3::ZERO,
        kind: 1,
    });
    let mut radar_hud = crate::ui::GameHUD::new();
    frame.apply_events_to_game_hud(&mut radar_hud);
    let after_radar = radar_hud.message_count_for_test();
    assert!(after_radar > 0, "RadarMessage must still reach the HUD");
    frame.apply_events_to_game_hud(&mut radar_hud);
    assert_eq!(
        radar_hud.message_count_for_test(),
        after_radar,
        "same freeze must not re-push radar every apply"
    );
}

#[test]
fn apply_events_queues_audio_for_destroy_and_attack() {
    crate::game_logic::host_attack_log::clear();
    crate::game_logic::host_attack_log::record(
        crate::game_logic::ObjectId(1),
        Some(crate::game_logic::ObjectId(2)),
    );
    let _ = crate::game_logic::host_attack_log::drain();
    let mut logic = crate::game_logic::GameLogic::new();
    // inject destroy event via construction of frame with attack only
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let n = frame.apply_events_to_audio(&mut logic);
    assert!(n >= 1, "expected audio queue from AttackTargeted, n={n}");
    assert!(logic.queued_audio_event_count_for_test() >= 1);
}

#[test]
fn heal_applied_freezes_from_last_drain() {
    crate::game_logic::host_heal_log::clear();
    crate::game_logic::host_heal_log::record(crate::game_logic::ObjectId(3), 88.0);
    let _ = crate::game_logic::host_heal_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::HealApplied { target, health }
                if target.0 == 3 && (*health - 88.0).abs() < 0.01
            )
        }),
        "expected HealApplied: {:?}",
        frame.events
    );
}

#[test]
fn economy_changed_freezes_from_last_drain() {
    crate::game_logic::host_economy_log::clear();
    crate::game_logic::host_economy_log::record(0, 12345, 7);
    let _ = crate::game_logic::host_economy_log::drain();
    let logic = crate::game_logic::GameLogic::new();
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame.events.iter().any(|e| {
            matches!(
                e,
                PresentationEvent::EconomyChanged {
                    player_id: 0,
                    supplies: 12345,
                    power_available: 7
                }
            )
        }),
        "expected EconomyChanged: {:?}",
        frame.events
    );
}

#[test]
fn supply_and_model_keys_freeze_from_host() {
    use crate::game_logic::{
        KindOf, Resources, Team, ThingTemplate,
        buildings::{BuildingData, BuildingType},
    };
    let mut logic = crate::game_logic::GameLogic::new();
    let mut ts = ThingTemplate::new("SupplyCenter");
    ts.set_health(1000.0);
    ts.add_kind_of(KindOf::Structure);
    ts.set_model("SCModel");
    logic.templates.insert("SupplyCenter".into(), ts);
    let mut tw = ThingTemplate::new("AmericaDozer");
    tw.set_health(200.0);
    tw.add_kind_of(KindOf::Vehicle);
    tw.add_kind_of(KindOf::Worker);
    tw.set_model("DozerModel");
    logic.templates.insert("AmericaDozer".into(), tw);
    let sc = logic
        .create_object("SupplyCenter", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let dz = logic
        .create_object("AmericaDozer", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(sc) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.building_data = Some(BuildingData::new(BuildingType::SupplyCenter));
        o.stored_resources = Resources {
            supplies: 1500,
            power: 0,
        };
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let s = frame.objects.iter().find(|o| o.id == sc).unwrap();
    assert_eq!(s.stored_supplies, 1500);
    assert_eq!(s.model_key.as_deref(), Some("SCModel"));
    let d = frame.objects.iter().find(|o| o.id == dz).unwrap();
    assert_eq!(d.model_key.as_deref(), Some("DozerModel"));
    assert_eq!(frame.supply_storage_structures().len(), 1);
    assert_eq!(frame.friendly_workers(Team::USA).len(), 1);
    let keys = frame.unique_model_keys();
    assert!(keys.iter().any(|k| k == "SCModel"));
    assert!(keys.iter().any(|k| k == "DozerModel"));
    assert_eq!(keys.len(), 2);
}

#[test]
fn building_type_freeze_from_host() {
    use crate::game_logic::{
        KindOf, Team, ThingTemplate,
        buildings::{BuildingData, BuildingType},
    };
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tb = ThingTemplate::new("WarFact");
    tb.set_health(1000.0);
    tb.add_kind_of(KindOf::Structure);
    logic.templates.insert("WarFact".into(), tb);
    let mut tc = ThingTemplate::new("CC");
    tc.set_health(2000.0);
    tc.add_kind_of(KindOf::Structure);
    logic.templates.insert("CC".into(), tc);
    let wf = logic
        .create_object("WarFact", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let cc = logic
        .create_object("CC", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(wf) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.building_data = Some(BuildingData::new(BuildingType::WarFactory));
    }
    if let Some(o) = logic.host_object_mut(cc) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.building_data = Some(BuildingData::new(BuildingType::CommandCenter));
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let w = frame.objects.iter().find(|o| o.id == wf).unwrap();
    assert_eq!(w.building_type, Some(PresentationBuildingType::WarFactory));
    assert!(w.can_produce);
    assert!(w.building_type.unwrap().is_unit_producer());
    let c = frame.objects.iter().find(|o| o.id == cc).unwrap();
    assert_eq!(
        c.building_type,
        Some(PresentationBuildingType::CommandCenter)
    );
    assert!(c.can_produce);
    assert!(!c.building_type.unwrap().is_unit_producer());
    // Prefer war factory over command center for unit production residual.
    assert_eq!(frame.first_constructed_producer_id(Team::USA), Some(wf));
    assert_eq!(frame.unit_producer_structures().len(), 1);
}

#[test]
fn mobile_and_producer_freeze_from_host() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = crate::game_logic::GameLogic::new();
    let mut tu = ThingTemplate::new("Humvee");
    tu.set_health(200.0);
    tu.add_kind_of(KindOf::Vehicle);
    tu.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Humvee".into(), tu);
    let mut tb = ThingTemplate::new("Barracks");
    tb.set_health(800.0);
    tb.add_kind_of(KindOf::Structure);
    tb.add_kind_of(KindOf::Selectable);
    logic.templates.insert("Barracks".into(), tb);
    let mut tw = ThingTemplate::new("Wall");
    tw.set_health(100.0);
    tw.add_kind_of(KindOf::Structure);
    logic.templates.insert("Wall".into(), tw);
    let u = logic
        .create_object("Humvee", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("Barracks", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let w = logic
        .create_object("Wall", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.host_object_mut(b) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        if o.building_data.is_none() {
            o.building_data = Some(crate::game_logic::BuildingData::new(
                crate::game_logic::BuildingType::Barracks,
            ));
        }
    }
    if let Some(o) = logic.host_object_mut(w) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        o.building_data = None;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let hu = frame.objects.iter().find(|o| o.id == u).unwrap();
    assert!(hu.is_mobile);
    assert!(!hu.can_produce);
    let hb = frame.objects.iter().find(|o| o.id == b).unwrap();
    assert!(!hb.is_mobile);
    assert!(hb.can_produce);
    let hw = frame.objects.iter().find(|o| o.id == w).unwrap();
    assert!(hw.is_structure);
    assert!(!hw.can_produce);
    assert_eq!(frame.first_mobile_friendly_id(Team::USA), Some(u));
    assert_eq!(frame.first_constructed_producer_id(Team::USA), Some(b));
    assert_eq!(frame.count_mobile_friendlies(Team::USA), 1);
}

#[test]
fn presentation_freezes_can_make_cameos_residual() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_MAXED_OUT_FOR_PLAYER, CANMAKE_OK,
    };
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;

    let mut logic = GameLogic::new();
    let mut p = Player::new(0, Team::USA, "USA", true);
    p.resources.supplies = 100_000;
    p.selected_objects.clear();
    logic.add_player(p);
    // Barracks + Burton residual templates.
    let mut bar = ThingTemplate::new("TestBarracks");
    bar.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("TestBarracks".into(), bar);
    // ensure building type barracks
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Hero)
        .set_health(200.0)
        .set_cost(1500, 0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(100, 0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let bid = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(crate::game_logic::BuildingData::new(
            crate::game_logic::BuildingType::Barracks,
        ));
    }
    if let Some(pl) = logic.get_player_mut(0) {
        pl.selected_objects = vec![bid];
    }

    // Direct residual probe before presentation freeze.
    assert_eq!(
        logic.can_make_unit(bid, "AmericaInfantryColonelBurton"),
        CANMAKE_OK,
        "direct can_make Burton"
    );
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(
        frame.can_make_producer_id,
        Some(bid.0),
        "producer id; cameos={:?}",
        frame.can_make_cameos
    );
    assert!(
        !frame.can_make_cameos.is_empty(),
        "cameos empty; producer={:?}",
        frame.can_make_producer_id
    );
    let burton_c = frame
        .can_make_cameos
        .iter()
        .find(|c| c.template_name.contains("Burton"))
        .unwrap_or_else(|| panic!("Burton cameo missing in {:?}", frame.can_make_cameos));
    assert!(burton_c.available, "burton={burton_c:?}");
    assert_eq!(burton_c.can_make, CANMAKE_OK);

    // Max out Burton residual.
    assert!(logic.enqueue_production(bid, "AmericaInfantryColonelBurton".into()));
    assert_eq!(
        logic.can_make_unit(bid, "AmericaInfantryColonelBurton"),
        CANMAKE_MAXED_OUT_FOR_PLAYER,
        "direct maxed after enqueue"
    );
    assert!(
        logic.get_player(0).is_some(),
        "player0 missing after enqueue"
    );
    assert!(
        logic
            .host_object(bid)
            .is_some_and(|o| o.building_data.is_some()),
        "barracks lost building_data"
    );
    // Keep selection residual for freeze.
    if let Some(pl) = logic.get_player_mut(0) {
        pl.selected_objects = vec![bid];
        pl.is_local = true;
    }
    let frame2 = PresentationFrame::build_from_logic(&logic, 0); // same local id residual
    assert_eq!(
        frame2.can_make_producer_id,
        Some(bid.0),
        "frame2 producer; local_sel={:?} objs={}",
        logic.get_player(0).map(|p| p.selected_objects.clone()),
        logic.host_objects().len()
    );
    let burton2 = frame2
        .can_make_cameos
        .iter()
        .find(|c| c.template_name.contains("Burton"))
        .unwrap_or_else(|| panic!("Burton cameo2 missing in {:?}", frame2.can_make_cameos));
    assert!(!burton2.available, "burton2={burton2:?}");
    assert_eq!(burton2.can_make, CANMAKE_MAXED_OUT_FOR_PLAYER);
    assert!(
        burton2
            .help_status
            .as_deref()
            .is_some_and(|s| s.contains("maximum"))
    );

    // apply_to_ui_state residual feed.
    let mut ui = crate::ui::GameUIState::default();
    frame2.apply_to_ui_state(&mut ui);
    assert_eq!(ui.can_make_producer_id, Some(bid.0));
    assert!(
        ui.can_make_cameos
            .iter()
            .any(|c| !c.available && c.template_name.contains("Burton"))
    );
}

#[test]
fn presentation_freezes_dozer_construct_can_make_cameos() {
    use crate::game_logic::host_production_buildable_command_residual::{
        CANMAKE_NO_MONEY, CANMAKE_OK,
    };
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;

    let mut logic = GameLogic::new();
    let mut p = Player::new(0, Team::USA, "USA", true);
    p.resources.supplies = 100_000;
    p.selected_objects.clear();
    logic.add_player(p);
    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t);
    let mut plant = ThingTemplate::new("AmericaPowerPlant");
    plant
        .add_kind_of(KindOf::Structure)
        .set_health(1000.0)
        .set_cost(800, 0);
    logic.templates.insert("AmericaPowerPlant".into(), plant);
    // BuildableStatus/Prereq residual: the shared sample prereq table couples
    // AmericaPowerPlant to a constructed AmericaCommandCenter
    // (host_production_buildable_command_residual PrereqSampleRow). C++
    // Player::canBuild (Player.cpp canBuild) runs the same ProductionPrerequisite
    // scan; a satisfied scan is oracle-equivalent to the retail plant whose
    // FactionBuilding.ini authors no Prerequisites line.
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(4000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    logic
        .create_object_for_player("AmericaCommandCenter", 0, glam::Vec3::ZERO)
        .expect("cc");

    let did = logic
        .create_object_for_player("AmericaVehicleDozer", 0, glam::Vec3::ZERO)
        .expect("dozer");
    if let Some(pl) = logic.get_player_mut(0) {
        pl.selected_objects = vec![did];
        pl.is_local = true;
        pl.resources.supplies = 100_000;
    }
    assert_eq!(
        logic.can_make_unit(did, "AmericaPowerPlant"),
        CANMAKE_OK,
        "dozer can_make PowerPlant with cash"
    );
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.can_make_producer_id, Some(did.0));
    let plant_c = frame
        .can_make_cameos
        .iter()
        .find(|c| c.template_name.contains("PowerPlant"))
        .unwrap_or_else(|| panic!("PowerPlant cameo missing in {:?}", frame.can_make_cameos));
    assert!(plant_c.available, "plant={plant_c:?}");
    assert_eq!(plant_c.can_make, CANMAKE_OK);

    if let Some(pl) = logic.get_player_mut(0) {
        pl.resources.supplies = 0;
        pl.selected_objects = vec![did];
    }
    assert_eq!(
        logic.can_make_unit(did, "AmericaPowerPlant"),
        CANMAKE_NO_MONEY
    );
    let frame2 = PresentationFrame::build_from_logic(&logic, 0);
    let plant2 = frame2
        .can_make_cameos
        .iter()
        .find(|c| c.template_name.contains("PowerPlant"))
        .expect("PowerPlant cameo2");
    assert!(!plant2.available);
    assert_eq!(plant2.can_make, CANMAKE_NO_MONEY);

    let mut bar = game_client::gui::control_bar::ControlBar::new();
    frame2.apply_to_control_bar(&mut bar);
    assert!(
        bar.presentation_can_make()
            .iter()
            .any(|(n, s)| n.contains("PowerPlant") && *s == CANMAKE_NO_MONEY),
        "ControlBar must receive dozer CanMake gray residual: {:?}",
        bar.presentation_can_make()
    );
}

fn presentation_freezes_public_timer_superweapons() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_superweapon_kindof::AMERICA_PARTICLE_CANNON_UPLINK;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut p = Player::new(0, Team::USA, "USA", true);
    p.apply_faction_intrinsic_sciences();
    // Shared map must not drive structure PublicTimer residual.
    p.shared_special_power_cooldowns
        .insert(SpecialPowerType::ParticleCannon, 999.0);
    logic.add_player(p);
    // Without SW structure: no PublicTimer PUC row residual.
    let frame0 = PresentationFrame::build_from_logic(&logic, 0);
    assert!(
        frame0
            .superweapon_timers
            .iter()
            .find(|t| t.name.contains("Particle"))
            .is_none(),
        "PUC timer requires structure residual"
    );
    // Build living PUC residual.
    let mut puc = ThingTemplate::new(AMERICA_PARTICLE_CANNON_UPLINK);
    puc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSuperweapon)
        .set_health(4000.0);
    logic
        .templates
        .insert(AMERICA_PARTICLE_CANNON_UPLINK.into(), puc);
    let id = logic
        .create_object(
            AMERICA_PARTICLE_CANNON_UPLINK,
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("puc");
    if let Some(o) = logic.host_object_mut(id) {
        o.status.under_construction = false;
        o.construction_percent = 1.0;
        // Structure module residual remaining (not SharedNSync player timer).
        o.special_power_cooldowns
            .insert(SpecialPowerType::ParticleCannon, 120.0);
        o.special_power_cooldown_remaining = 120.0;
        o.special_power_ready = false;
    }
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    let puc_t = frame
        .superweapon_timers
        .iter()
        .find(|t| t.name.contains("Particle"))
        .expect("PUC");
    assert!(puc_t.unlocked);
    assert!(
        (puc_t.remaining - 120.0).abs() < 0.5,
        "remaining {} from structure module residual",
        puc_t.remaining
    );
    assert!(!puc_t.ready);
    // Apply to construction panel residual.
    let mut panel = crate::ui::construction_panel::ConstructionPanel::new(0, 0);
    frame.apply_superweapon_timers_to_panel(&mut panel);
    assert!(!panel.superweapon_timers().is_empty());
}

#[test]
fn presentation_freezes_local_rank_skill_residual() {
    use crate::game_logic::{GameLogic, Player, Team};
    let mut logic = GameLogic::new();
    let mut p = Player::new(0, Team::USA, "USA", true);
    p.apply_faction_intrinsic_sciences();
    p.skill_points = 850; // rank 2 threshold 800
    p.rank_level = 1;
    // Recompute rank via add_skill_points path residual.
    let _ = p.add_skill_points(0); // no-op if 0
    // Force rank apply by setting skill then calling add with 0 won't promote;
    // set rank manually for freeze honesty.
    p.rank_level = 2;
    p.science_purchase_points = 3;
    logic.add_player(p);
    let frame = PresentationFrame::build_from_logic(&logic, 0);
    assert_eq!(frame.local_rank_level, 2);
    assert_eq!(frame.local_skill_points, 850);
    assert_eq!(frame.local_science_purchase_points, 3);
    assert!(frame.local_rank_progress_percent >= 0 && frame.local_rank_progress_percent <= 100);
    assert!(
        frame.local_has_science("SCIENCE_AMERICA")
            || frame
                .local_unlocked_sciences
                .iter()
                .any(|s| s.contains("AMERICA"))
    );
}

#[test]
fn presentation_applies_superweapon_timers_to_game_hud_residual() {
    use crate::game_logic::GameLogic;
    let logic = GameLogic::new();
    let mut frame = PresentationFrame::build_from_logic(&logic, 0);
    frame.superweapon_timers.clear();
    frame.superweapon_timers.push(PresentationSuperweaponTimer {
        name: "Particle Uplink Cannon".into(),
        template_name: "SPECIAL_PARTICLE_UPLINK_CANNON".into(),
        icon: "PUC".into(),
        recharge_time: 300.0,
        remaining: 12.5,
        unlocked: true,
        ready: false,
        power_key: "ParticleCannon".into(),
    });
    frame.superweapon_timers.push(PresentationSuperweaponTimer {
        name: "Locked".into(),
        template_name: "SPECIAL_SCUD_STORM".into(),
        icon: "SCUD".into(),
        recharge_time: 360.0,
        remaining: 360.0,
        unlocked: false,
        ready: false,
        power_key: "ScudStorm".into(),
    });
    let mut hud = crate::ui::GameHUD::new();
    frame.apply_to_game_hud(&mut hud);
    let timers = hud.presentation_superweapon_timers();
    assert_eq!(
        timers.len(),
        1,
        "locked PublicTimer must not freeze onto HUD"
    );
    assert_eq!(timers[0].name, "Particle Uplink Cannon");
    assert!((timers[0].remaining - 12.5).abs() < 0.01);
    assert!(!timers[0].ready);
    assert!(timers[0].unlocked);
}

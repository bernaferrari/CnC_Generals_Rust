//! Skirmish admission and publication of completed simulation frames.

#![allow(unused_imports)]

use super::imports::*;

pub(super) struct HostSession {
    pub hud: crate::ui::GameHUD,
    pub ui_state: crate::ui::GameUIState,
    pub rts: crate::ui::RTSInterface,
    pub command_panel: crate::ui::UnitCommandPanel,
    pub pres: crate::presentation_frame::PresentationFrame,
    pub select_id: Option<crate::game_logic::ObjectId>,
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub frames_ok: bool,
    pub presentation_ok: bool,
}

pub(super) fn run_host_session(
    logic: &mut GameLogic,
    frames: u32,
) -> generals_sim::GameLogicResult<HostSession> {
    // GameEngine::init brings up TheFileSystem, then loads GameData, Locomotor,
    // PlayerTemplate, Multiplayer and Object INI before any NewGame
    // (GameEngine.cpp:281-329, 428-458); NewGame requires that admitted content.
    {
        let common = logic.world_services.common();
        let mut file_system = common.application.file_system.lock().map_err(|error| {
            generals_sim::GameLogicError::Configuration(format!(
                "cannot lock the shell filesystem: {error}"
            ))
        })?;
        crate::subsystem_manager::FileSystemSubsystem::mount_common_files(&mut file_system)
            .map_err(|error| generals_sim::GameLogicError::Configuration(error.to_string()))?;
    }
    let definitions = crate::parallel_harness::RetailWorldDefinitions::load()
        .map_err(|error| generals_sim::GameLogicError::Configuration(error.to_string()))?;
    definitions
        .admit(logic)
        .map_err(generals_sim::GameLogicError::Configuration)?;

    let resolved = super::maps::resolve_host_map();
    let map_resolved = resolved.is_some();
    let map_id = resolved
        .as_ref()
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "HostSyntheticMap".into());
    let map_path = resolved.map(|(_, p)| p);

    // Production UI path only — no golden_skirmish_config fallback.
    let settings = logic.multiplayer_definitions.clone().ok_or_else(|| {
        generals_sim::GameLogicError::Configuration("missing shell multiplayer definitions".into())
    })?;
    if settings.color_definitions.is_empty() {
        return Err(generals_sim::GameLogicError::Configuration(
            "authored MultiplayerColorDefinition palette is unavailable at startup".into(),
        ));
    }
    let mut menu = SkirmishMenu::new();
    let menu_init_ok = menu.initialize_with_settings(&settings).is_ok();
    // Slot 0 is Human by default; configure slot 1 as Medium AI via menu cycling.
    let medium_ai_ok = menu.configure_slot_medium_ai(1);
    if map_resolved {
        menu.set_map_name(map_id.clone());
    }
    let (slots, rules, menu_map_name) = menu.get_game_config();
    let mut cfg = config_from_skirmish_menu(&menu_map_name, &rules, &slots, menu.random_seed());
    // This headless scenario has no saved preferences; C++ uses the authored
    // default cash when StartingCash is absent and applies the shroud setting.
    if !settings.got_default_starting_money {
        return Err(generals_sim::GameLogicError::Configuration(
            "authored default starting money is unavailable".into(),
        ));
    }
    cfg.rules.starting_cash = i32::try_from(settings.default_starting_money.count_money())
        .map_err(|_| {
            generals_sim::GameLogicError::Configuration(
                "authored default starting money exceeds headless menu range".into(),
            )
        })?;
    cfg.rules.fog_of_war = settings.is_shroud_in_multiplayer;
    let active = cfg.slots.iter().filter(|s| s.is_active).count();
    let has_human = cfg.slots.iter().any(|s| s.is_human);
    let has_ai = cfg.slots.iter().any(|s| !s.is_human && s.is_active);
    let menu_config_ok = menu_init_ok && medium_ai_ok && active >= 2 && has_human && has_ai;

    let apply_ok = crate::skirmish_config::apply_skirmish_config_with_multiplayer_settings(
        logic, &cfg, &settings,
    )
    .is_ok();
    let skirmish_config_ok = apply_ok
        && logic.get_players().len() >= 2
        && logic.host_ai_player_count() >= 1
        && logic.skirmish_rules().fog_of_war == settings.is_shroud_in_multiplayer;

    // Host is "constructed" only when production apply path succeeds — not a constant true.
    let host_constructed = skirmish_config_ok;

    let map_loaded = if let Some(ref path) = map_path {
        logic.load_map(&path.display().to_string())
    } else {
        false
    };

    let mut hud = GameHUD::new();
    let mut ui_state = GameUIState::default();
    let mut rts = RTSInterface::new();
    let mut command_panel = UnitCommandPanel::new();
    let mut publish = |logic: &mut GameLogic, with_victory| {
        let frame = PresentationFrame::try_publish(
            logic, 0, None, with_victory,
        )
        .map_err(generals_sim::GameLogicError::Configuration)?;
        frame.apply_to_shell_ui_consumers(&mut hud, &mut ui_state, &mut rts, &mut command_panel);
        Ok::<_, generals_sim::GameLogicError>(frame)
    };
    let seed_pres = publish(logic, false)?;
    let seed_ok = seed_pres.frame.0 == logic.get_frame()
        && (seed_pres.alive_object_count() > 0 || !map_loaded);

    let frame_before = logic.get_frame();
    let mut step_frames_ok = true;
    for _ in 0..frames.max(1) {
        let next_frame = logic.get_frame().wrapping_add(1);
        logic.update();
        logic.complete_presentation_observations();
        logic.headless_camera_view_offer(33)?;
        let presented = publish(logic, true)?;
        step_frames_ok &= presented.frame.0 == next_frame && presented.frame.0 == logic.get_frame();
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);
    let frames_ok = frames_advanced == frames.max(1);

    // Ensure at least one selectable unit is selected so selection health is exercised.
    let select_id = logic
        .host_objects()
        .registered_values()
        .find(|o| o.is_alive() && !o.status.destroyed)
        .map(|o| o.id);
    if let Some(id) = select_id {
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.host_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
    }

    let pres = publish(logic, true)?;
    let grid = pres.fow_grid();
    let fog_grid_ok = !grid.active
        || (grid.width > 0
            && grid.height > 0
            && grid.cell_count() == (grid.width as usize).saturating_mul(grid.height as usize));
    let presentation_ok = seed_ok
        && fog_grid_ok
        && step_frames_ok
        && pres.frame.0 == logic.get_frame()
        && (pres.alive_object_count() > 0 || !map_loaded)
        && !pres.objects.iter().any(|o| {
            // Map-authored audio emitters (`Amb_*`: `Draw = W3DDefaultDraw`, no
            // model) are drawn as nothing by design; any other object without a
            // model is a presentation gap.
            let authored_model_less = logic.host_object(o.id).is_some_and(|object| {
                object
                    .get_template()
                    .authored_draw_models_for_conditions(0)
                    .is_some_and(|models| models.is_empty())
            });
            o.model_key.is_none() && !o.destroyed && !authored_model_less
        });

    Ok(HostSession {
        hud,
        ui_state,
        rts,
        command_panel,
        pres,
        select_id,
        host_constructed,
        skirmish_config_ok,
        menu_config_ok,
        map_resolved,
        map_loaded,
        frames_advanced,
        frames_ok,
        presentation_ok,
    })
}

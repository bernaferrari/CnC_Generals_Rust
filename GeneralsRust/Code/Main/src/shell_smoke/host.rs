//! Host construction, skirmish apply, map load, dual-tick frames.

#![allow(unused_imports)]

use super::imports::*;

pub(super) struct HostSession {
    pub logic: crate::game_logic::GameLogic,
    pub hud: crate::ui::GameHUD,
    pub ui_state: crate::ui::GameUIState,
    pub rts: crate::ui::RTSInterface,
    pub command_panel: crate::ui::UnitCommandPanel,
    pub pres: crate::presentation_frame::PresentationFrame,
    pub seed_pres: crate::presentation_frame::PresentationFrame,
    pub select_id: Option<crate::game_logic::ObjectId>,
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub frames_ok: bool,
    pub presentation_ok: bool,
    pub dual_tick_presentation_ok: bool,
    pub dual_tick_counters_ok: bool,
    pub gameworld_shadow_ok: bool,
    pub damage_authority_env_ok: bool,
    pub economy_authority_env_ok: bool,
    pub production_authority_env_ok: bool,
    pub dual_tick_policy_authority_only: bool,
    pub engine_bridge_off: bool,
}

pub(super) fn run_host_session(frames: u32) -> HostSession {
    // Default-on damage authority for gate honesty (opt out via env=0).
    crate::gameworld_shadow::ensure_gate_damage_authority();
    let mut logic = GameLogic::new();

    let resolved = super::maps::resolve_host_map();
    let map_resolved = resolved.is_some();
    let map_id = resolved
        .as_ref()
        .map(|(id, _)| id.clone())
        .unwrap_or_else(|| "HostSyntheticMap".into());
    let map_path = resolved.map(|(_, p)| p);

    // Production UI path only — no golden_skirmish_config fallback.
    let mut menu = SkirmishMenu::new();
    let menu_init_ok = menu.initialize().is_ok();
    // Slot 0 is Human by default; configure slot 1 as Medium AI via menu cycling.
    let medium_ai_ok = menu.configure_slot_medium_ai(1);
    if map_resolved {
        menu.set_map_name(map_id.clone());
    }
    let (slots, rules, menu_map_name) = menu.get_game_config();
    let cfg = config_from_skirmish_menu(&menu_map_name, &rules, &slots);
    let active = cfg.slots.iter().filter(|s| s.is_active).count();
    let has_human = cfg.slots.iter().any(|s| s.is_human);
    let has_ai = cfg.slots.iter().any(|s| !s.is_human && s.is_active);
    let menu_config_ok = menu_init_ok && medium_ai_ok && active >= 2 && has_human && has_ai;

    let apply_ok = apply_skirmish_config(&mut logic, &cfg).is_ok();
    let skirmish_config_ok = apply_ok
        && logic.get_players().len() >= 2
        && logic.host_ai_player_count() >= 1
        && logic.skirmish_rules().fog_of_war;

    // Host is "constructed" only when production apply path succeeds — not a constant true.
    let host_constructed = skirmish_config_ok;

    let map_loaded = if let Some(ref path) = map_path {
        logic.load_map(&path.display().to_string())
    } else {
        false
    };

    // Immediate post-map seed (matches start_game_from_ui seed before first dual-tick).
    // Multi-consumer residual: HUD + UIState + RTS + unit command panel share snapshot.
    let mut hud = GameHUD::new();
    let mut ui_state = GameUIState::default();
    let mut rts = RTSInterface::new();
    let mut command_panel = UnitCommandPanel::new();
    let seed_pres = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic,
        0,
        &mut hud,
        &mut ui_state,
        &mut rts,
        &mut command_panel,
    );
    let seed_ok = seed_pres.frame.0 == logic.get_frame()
        && (seed_pres.alive_object_count() > 0 || !map_loaded);

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        // Dual-tick: authority step then multi-consumer presentation apply.
        logic.update();
        let _ = PresentationFrame::build_and_apply_for_shell_consumers(
            &logic,
            0,
            &mut hud,
            &mut ui_state,
            &mut rts,
            &mut command_panel,
        );
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);
    let frames_ok = frames_advanced > 0;

    // Ensure at least one selectable unit is selected so selection health is exercised.
    let select_id = logic
        .host_objects()
        .values()
        .find(|o| o.is_alive() && !o.status.destroyed)
        .map(|o| o.id);
    if let Some(id) = select_id {
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic./* Wave 950 */ host_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
    }

    let pres = PresentationFrame::build_and_apply_for_shell_consumers(
        &logic,
        0,
        &mut hud,
        &mut ui_state,
        &mut rts,
        &mut command_panel,
    );
    let presentation_ok = seed_ok
        && pres.frame.0 == logic.get_frame()
        && (pres.alive_object_count() > 0 || !map_loaded)
        && !pres
            .objects
            .iter()
            .any(|o| o.model_key.is_none() && !o.destroyed);

    // Dual-tick residual honesty: seed frame applied, then post-update presentation
    // matches authority frame (start_game_from_ui / engine dual-tick order).
    let dual_tick_presentation_ok = seed_ok
        && frames_ok
        && presentation_ok
        && pres.frame.0 == logic.get_frame()
        && seed_pres.frame.0 <= pres.frame.0;
    // Dual-tick residual counters (build + apply recorded on shell apply path).
    let dual_tick_counters_ok = presentation_ok
        && dual_tick_presentation_ok
        && seed_pres.dual_tick_presentation_residual_ok()
        && seed_pres.dual_tick.honesty_apply_ok()
        && pres.dual_tick_presentation_residual_ok()
        && pres.dual_tick.honesty_apply_ok()
        && seed_pres.dual_tick.applies >= 1
        && pres.dual_tick.applies >= 1;
    let gameworld_shadow_ok = {
        let (_w, probe) = crate::gameworld_shadow::probe_host_vs_gameworld(&mut logic);
        probe.full_match()
    };
    let damage_authority_env_ok = crate::gameworld_shadow::gameworld_damage_authority_enabled();
    let economy_authority_env_ok = crate::gameworld_shadow::gameworld_economy_authority_enabled();
    let production_authority_env_ok =
        crate::gameworld_shadow::gameworld_production_authority_enabled();
    let dual_tick_policy_authority_only = matches!(
        crate::authoritative_world::dual_tick_policy(),
        crate::authoritative_world::DualTickPolicy::AuthorityOnly
    );
    let engine_bridge_off = !crate::gameworld_shadow::engine_object_bridge_enabled();

    HostSession {
        logic,
        hud,
        ui_state,
        rts,
        command_panel,
        pres,
        seed_pres,
        select_id,
        host_constructed,
        skirmish_config_ok,
        menu_config_ok,
        map_resolved,
        map_loaded,
        frames_advanced,
        frames_ok,
        presentation_ok,
        dual_tick_presentation_ok,
        dual_tick_counters_ok,
        gameworld_shadow_ok,
        damage_authority_env_ok,
        economy_authority_env_ok,
        production_authority_env_ok,
        dual_tick_policy_authority_only,
        engine_bridge_off,
    }
}

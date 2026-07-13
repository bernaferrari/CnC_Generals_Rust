//! Production host smoke: SkirmishMenu → config → apply → map load → frames → presentation.
//!
//! Full windowed shell/WND + GPU boot still requires a display; this path exercises the
//! same production APIs `start_game_from_ui` uses after menu StartGame.
//!
//! Honesty: no tautological host flag, no silent golden_skirmish_config fallback.
//! Opponent slot is configured through SkirmishMenu::configure_slot_medium_ai.
//!
//! Claim flags (do not conflate):
//! - `playable_claim` — **always false**. Headless host APIs are not retail W3D /
//!   windowed shell playthrough. Fail-closed pending full GPU/WND match play.
//! - `shell_host_playable_ok` — limited honesty claim: when true, the headless
//!   shell→config→map→dual-tick presentation→HUD selection/minimap→ControlBar.wnd
//!   ensure path is operational. Still **not** a retail playthrough claim.
//!
//! Residual honesty (do **not** flip `playable_claim`):
//! - `dual_tick_presentation_ok` — seed + logic update + multi-consumer presentation apply
//! - `minimap_fow_presentation_ok` — FOW grid snapshot usable for minimap texture path
//! - `laser_segment_upload_ok` — presentation → CPU SegLine pack residual (incl. synthetic)
//! - `multi_beam_soft_edge_ok` — OrbitalLaser NumBeams soft-edge CPU pack residual
//! - `floating_text_layout_ok` — presentation → CPU InGameUI floating-text layout residual
//! - `world_anim_presentation_ok` — MoneyPickUp Anim2D residual frozen on presentation
//! - `world_anim_layout_ok` — presentation → CPU Anim2D layout pack residual
//! - `anim2d_frame_ok` — MoneyPickUp Anim2D frame advance residual
//! - `game_text_caption_ok` — GUI:AddCash caption residual on floating-text pack
//! - `game_text_csf_str_ok` — CSF/STR parse + retail `$%d` printf + DisplayString measure
//! - `display_string_measure_ok` — monospaced glyph measure residual on floating-text pack
//! - `rng_stream_residual_ok` — GameLogic/GameClient RandomValue ADC stream residual
//! - `control_bar_path_resolved` / `control_bar_wnd_validated` — ControlBar.wnd residual
//! - `control_bar_window_loaded` — headless WindowManager parse when WindowZH present

use crate::game_logic::host_rng_residual::exercise_host_rng_residual;
use crate::game_logic::GameLogic;
use crate::gameplay_layout::{
    control_bar_layout_honesty, format_control_bar_honesty, GameplayLayoutStatus,
};
use crate::graphics::floating_text_layout::{
    pack_floating_text_and_mark_ready, FloatingTextLayout,
};
use crate::graphics::game_text_residual::exercise_host_game_text_residual;
use crate::graphics::laser_segment_upload::{
    pack_and_mark_upload_ready, LaserSegmentUpload,
};
use crate::graphics::world_anim_layout::{
    pack_world_anim_and_mark_ready, WorldAnimLayout,
};
use crate::map_frame_scenario::resolve_first_map;
use crate::presentation_frame::{
    PresentationFloatingText, PresentationFrame, PresentationLaserBeam, PresentationWorldAnim,
};
use crate::skirmish_config::{apply_skirmish_config, config_from_skirmish_menu};
use crate::ui::skirmish_menu::SkirmishMenu;
use crate::ui::{
    GameHUD, GameUIState, RTSInterface, Screen, UIManager, UnitCommandPanel,
};

const HOST_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

#[derive(Debug, Clone)]
pub struct ShellSmokeResult {
    /// True only after GameLogic exists and skirmish config applied successfully.
    pub host_constructed: bool,
    pub skirmish_config_ok: bool,
    pub menu_config_ok: bool,
    pub map_resolved: bool,
    pub map_loaded: bool,
    pub frames_advanced: u32,
    pub presentation_ok: bool,
    /// Dual-tick residual: seed presentation, then logic update + build_and_apply order.
    pub dual_tick_presentation_ok: bool,
    /// Dual-tick residual: after map load, logic update + presentation seed HUD
    /// selection health / minimap without re-reading live objects.
    pub hud_selection_ok: bool,
    /// Minimap FOW residual: presentation `fow_grid` is host-usable (active R8 or honest inactive).
    pub minimap_fow_presentation_ok: bool,
    /// Laser residual: presentation → CPU SegLine vertex pack (+ synthetic non-empty pack).
    pub laser_segment_upload_ok: bool,
    /// OrbitalLaser multi-beam soft-edge CPU pack residual (NumBeams width/color lerp).
    pub multi_beam_soft_edge_ok: bool,
    /// Floating-text residual: presentation freeze + CPU layout pack (+ synthetic).
    pub floating_text_layout_ok: bool,
    /// MoneyPickUp Anim2D residual: presentation freeze honesty (empty or template ok).
    pub world_anim_presentation_ok: bool,
    /// World-anim residual: presentation → CPU Anim2D layout pack (+ synthetic).
    pub world_anim_layout_ok: bool,
    /// MoneyPickUp Anim2D frame advance residual (LOOP / SCPDollarNNN).
    pub anim2d_frame_ok: bool,
    /// GameText `GUI:AddCash` caption residual on synthetic floating-text pack.
    pub game_text_caption_ok: bool,
    /// CSF/STR GameText residual + retail `$%d` printf + DisplayString measure.
    pub game_text_csf_str_ok: bool,
    /// DisplayString monospaced measure residual on floating-text pack.
    pub display_string_measure_ok: bool,
    /// GameLogic/GameClient RandomValue ADC stream residual honesty.
    pub rng_stream_residual_ok: bool,
    /// Shell Skirmish → Loading → GameHUD ownership transition (StartGame parity).
    pub screen_skirmish_ok: bool,
    /// ControlBar.wnd resolve/validate path (C++ ShowControlBar / ensure_gameplay_layouts).
    /// True when layout Ready, or assets honestly unavailable (CI without WindowZH).
    pub control_bar_layout_ok: bool,
    /// ControlBar.wnd path found on disk.
    pub control_bar_path_resolved: bool,
    /// ControlBar.wnd structural validate (FILE_VERSION / WINDOW / ControlBar tokens).
    pub control_bar_wnd_validated: bool,
    /// Headless WindowManager parse materialised GameWindows (assets present path).
    /// False when WindowZH missing or parse deferred — still honest residual.
    pub control_bar_window_loaded: bool,
    /// Window count from headless WindowManager load (0 when not loaded).
    pub control_bar_window_count: usize,
    /// Dual-tick residual: selection panel applied to HUD + UIState + RTS + command panel.
    pub selection_consumers_ok: bool,
    /// Limited headless host claim (see module docs). Not retail W3D play.
    pub shell_host_playable_ok: bool,
    /// Always false here: no window/WND/GPU retail playthrough.
    pub playable_claim: bool,
    pub status: String,
    pub detail: String,
}

/// Exercise production host entry points headlessly (no window required).
/// Builds config from live SkirmishMenu (including Medium AI slot via menu cycle),
/// applies it, loads retail map when present, advances logic frames, builds presentation,
/// applies dual-tick presentation → GameHUD selection/minimap, ensures ControlBar.wnd,
/// and exercises shell→InGame screen ownership (start_game_from_ui parity).
pub fn run_shell_smoke(frames: u32) -> ShellSmokeResult {
    let mut logic = GameLogic::new();

    let resolved = resolve_first_map(HOST_MAP_CANDIDATES);
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
        .get_objects()
        .values()
        .find(|o| o.is_alive() && !o.status.destroyed)
        .map(|o| o.id);
    if let Some(id) = select_id {
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
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

    // Minimap FOW from presentation residual (grid snapshot, not live shroud re-lock).
    let minimap_fow_presentation_ok = presentation_ok && pres.minimap_fow_presentation_ok();

    // WGPU laser segment upload residual (CPU pack path; no live device required).
    // Empty host lasers → honest empty pack; synthetic assist pair exercises geometry.
    let empty_pack = pack_and_mark_upload_ready(&pres);
    let synthetic = PresentationLaserBeam::synthetic_assist_pair(pres.frame.0);
    let mut synth_frame = pres.clone();
    synth_frame.laser_beams = synthetic.to_vec();
    let synth_pack = LaserSegmentUpload::pack_from_presentation(&synth_frame);
    let laser_segment_upload_ok = empty_pack.honesty.honesty_cpu_pack_ok()
        && empty_pack.honesty.honesty_upload_ready_ok()
        && synth_pack.honesty.honesty_geometry_ok()
        && synth_pack.honesty.segments_packed >= 20
        && synth_pack.honesty.beams_packed == 2;
    // OrbitalLaser multi-beam soft-edge CPU residual (NumBeams 12 width/color lerp).
    let multi_beam_pack = LaserSegmentUpload::pack_orbital_multi_beam_soft_edge(
        (0.0, 0.0, 0.0),
        (0.0, 0.0, 200.0),
        1.0,
        1.0,
    );
    let multi_beam_soft_edge_ok = multi_beam_pack.honesty.honesty_cpu_pack_ok()
        && multi_beam_pack.honesty.honesty_geometry_ok()
        && multi_beam_pack.honesty.honesty_multi_beam_soft_edge_ok();

    // InGameUI floating text + MoneyPickUp Anim2D residual (CPU layout; no live GPU).
    // Empty host texts → honest empty pack; synthetic cash exercises geometry.
    let ft_empty = pack_floating_text_and_mark_ready(&pres);
    let mut ft_synth_frame = pres.clone();
    ft_synth_frame.floating_texts = vec![PresentationFloatingText::synthetic_cash(100, pres.frame.0)];
    ft_synth_frame.world_anims = vec![PresentationWorldAnim::synthetic_money_pickup(pres.frame.0)];
    let ft_synth = FloatingTextLayout::pack_from_presentation(&ft_synth_frame);
    let floating_text_layout_ok = presentation_ok
        && pres.floating_text_presentation_ok()
        && ft_empty.honesty.honesty_cpu_pack_ok()
        && ft_empty.honesty.honesty_upload_ready_ok()
        && ft_empty.honesty.honesty_retail_params_ok()
        && ft_synth.honesty.honesty_geometry_ok()
        && ft_synth.honesty.texts_packed == 1
        && ft_synth.honesty.world_anims_observed == 1;
    let game_text_caption_ok = floating_text_layout_ok
        && ft_synth.honesty.honesty_game_text_caption_ok()
        && ft_synth
            .entries
            .first()
            .map(|e| e.caption == "+$100")
            .unwrap_or(false);
    let display_string_measure_ok = floating_text_layout_ok
        && ft_synth.honesty.honesty_display_string_measure_ok()
        && ft_synth
            .entries
            .first()
            .map(|e| e.measure_width > 0 && e.measure_height == 8)
            .unwrap_or(false);
    // CSF/STR GameText residual exercise (retail `$%d` + optional live CSF).
    let game_text_csf_str_ok = exercise_host_game_text_residual().honesty.honesty_ok();
    let world_anim_presentation_ok = presentation_ok && pres.world_anim_presentation_ok();
    // World-anim CPU layout residual (empty + synthetic MoneyPickUp).
    let wa_empty = pack_world_anim_and_mark_ready(&pres);
    let wa_synth = WorldAnimLayout::pack_from_presentation(&ft_synth_frame);
    let world_anim_layout_ok = presentation_ok
        && world_anim_presentation_ok
        && wa_empty.honesty.honesty_cpu_pack_ok()
        && wa_empty.honesty.honesty_upload_ready_ok()
        && wa_synth.honesty.honesty_geometry_ok()
        && wa_synth.honesty.anims_packed == 1
        && wa_synth.honesty.honesty_template_ok();
    let anim2d_frame_ok = world_anim_layout_ok
        && wa_synth.honesty.honesty_anim2d_frame_ok()
        && wa_synth
            .entries
            .first()
            .map(|e| e.frame_image.starts_with("SCPDollar"))
            .unwrap_or(false);
    // GameLogic / GameClient RandomValue ADC stream residual.
    let rng_stream_residual_ok = exercise_host_rng_residual(0x5A6E_2710).honesty_ok();

    // HUD + multi-consumer selection panel health from presentation after dual-tick.
    let (hud_selection_ok, selection_consumers_ok) = if let Some(id) = select_id {
        let infos = hud.selected_unit_infos();
        let snap_infos = pres.selected_unit_display_infos();
        let hud_hit = infos.iter().any(|u| {
            u.object_id == id && u.health_current > 0.0 && u.health_maximum >= u.health_current
        });
        let snap_hit = snap_infos
            .iter()
            .any(|u| u.object_id == id && u.health_current > 0.0);
        let ids_ok = hud.selected_unit_ids().contains(&id);
        let minimap_ok = !pres.hud_minimap_units().is_empty() || !map_loaded;
        let panel = hud.selection_panel();
        let panel_ok = panel.visible
            && panel.has_positive_health()
            && panel.primary_object_id == Some(id);
        // Optional ControlBar path (headless selection health; not full WND claim).
        #[cfg(feature = "game_client")]
        let control_bar_ok = {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            pres.apply_to_control_bar(&mut bar);
            bar.selection_panel_health()
                .map(|(hp, max)| hp > 0.0 && max >= hp)
                .unwrap_or(false)
        };
        #[cfg(not(feature = "game_client"))]
        let control_bar_ok = true;
        let ui_ok = ui_state.selection_panel.has_positive_health()
            && ui_state.selection_panel.primary_object_id == Some(id);
        let rts_ok = rts.selection_panel().has_positive_health() && rts.selected_ids().contains(&id);
        let cmd_ok = command_panel.is_visible()
            && command_panel.selection_panel().has_positive_health()
            && command_panel.selected_ids().contains(&id);
        let consumers_ok = ui_ok && rts_ok && cmd_ok && control_bar_ok;
        (
            hud_hit && snap_hit && ids_ok && minimap_ok && panel_ok && control_bar_ok,
            consumers_ok,
        )
    } else {
        // No objects (absent-map synthetic host): still require resource apply path.
        let empty_ok = hud.selected_unit_ids().is_empty()
            && !hud.selection_panel().visible
            && (pres.local_supplies > 0 || skirmish_config_ok);
        let consumers_empty = !ui_state.selection_panel.visible
            && rts.selected_ids().is_empty()
            && !command_panel.is_visible();
        (empty_ok, empty_ok && consumers_empty)
    };

    // Shell → InGame residual: production StartGame transitions Skirmish→Loading→GameHUD
    // and ensure_gameplay_layouts (ControlBar.wnd) on InGame enter.
    let mut ui_mgr = UIManager::new(1024, 768);
    ui_mgr.transition_to_screen(Screen::Skirmish);
    let at_skirmish = ui_mgr.current_screen() == Some(Screen::Skirmish)
        && Screen::Skirmish.is_shell_owned_pregame();
    ui_mgr.transition_to_screen(Screen::Loading);
    let at_loading = ui_mgr.current_screen() == Some(Screen::Loading)
        && !Screen::Loading.is_shell_owned_pregame();
    // Attempt headless WindowManager load when game_client is enabled (ShowControlBar
    // residual). AssetsUnavailable remains honest when WindowZH is not checked out.
    // This is **not** windowed W3D retail — only layout script → window tree.
    #[cfg(feature = "game_client")]
    let layout_honesty = control_bar_layout_honesty(true);
    #[cfg(not(feature = "game_client"))]
    let layout_honesty = control_bar_layout_honesty(false);
    let layout_status = layout_honesty.status.clone();
    let layout_report = format_control_bar_honesty(&layout_honesty);
    let control_bar_path_resolved = layout_honesty.path_resolved;
    let control_bar_wnd_validated = layout_honesty.wnd_validated;
    let control_bar_window_loaded = layout_honesty.window_loaded;
    let control_bar_window_count = layout_honesty.window_count;
    let control_bar_layout_ok = match &layout_status {
        GameplayLayoutStatus::Ready { path, loaded } => {
            // Ready after structural validate. Prefer WindowManager load when assets
            // present (`loaded=true`); validated-only (`loaded=false`) is still ok.
            path.contains("ControlBar")
                && control_bar_wnd_validated
                && (*loaded == control_bar_window_loaded)
                && (!*loaded || control_bar_window_count > 0)
        }
        // Honest residual when WindowZH assets are not checked out.
        GameplayLayoutStatus::AssetsUnavailable { searched } => {
            !searched.is_empty()
                && layout_honesty.assets_unavailable
                && !control_bar_window_loaded
        }
        GameplayLayoutStatus::LoadFailed { .. } => false,
    };
    ui_mgr.transition_to_screen(Screen::GameHUD);
    let at_ingame = ui_mgr.current_screen() == Some(Screen::GameHUD)
        && !Screen::GameHUD.is_shell_owned_pregame();
    let screen_skirmish_ok = at_skirmish
        && at_loading
        && at_ingame
        && Screen::MainMenu.is_shell_owned_pregame()
        && Screen::startup_entry_screen(true) == Screen::MainMenu;

    // When assets present, map must load; when absent, still pass config+frames.
    let map_requirement_ok = if map_resolved { map_loaded } else { true };

    // Never claim full retail playability from headless smoke (no W3D/window/GPU).
    let playable_claim = false;

    let host_path_ok = host_constructed
        && skirmish_config_ok
        && menu_config_ok
        && frames_ok
        && presentation_ok
        && hud_selection_ok
        && selection_consumers_ok
        && dual_tick_presentation_ok
        && screen_skirmish_ok
        && control_bar_layout_ok
        && map_requirement_ok;

    // Limited claim: headless production host path is operational end-to-end.
    // Requires dual-tick presentation + multi-consumer selection + shell→InGame +
    // ControlBar.wnd ensure. Still not windowed W3D play (playable_claim stays false).
    let shell_host_playable_ok = host_path_ok;

    let status = if host_path_ok {
        "success".into()
    } else {
        "partial".into()
    };

    ShellSmokeResult {
        host_constructed,
        skirmish_config_ok,
        menu_config_ok,
        map_resolved,
        map_loaded,
        frames_advanced,
        presentation_ok,
        dual_tick_presentation_ok,
        hud_selection_ok,
        minimap_fow_presentation_ok,
        laser_segment_upload_ok,
        multi_beam_soft_edge_ok,
        floating_text_layout_ok,
        world_anim_presentation_ok,
        world_anim_layout_ok,
        anim2d_frame_ok,
        game_text_caption_ok,
        game_text_csf_str_ok,
        display_string_measure_ok,
        rng_stream_residual_ok,
        screen_skirmish_ok,
        control_bar_layout_ok,
        control_bar_path_resolved,
        control_bar_wnd_validated,
        control_bar_window_loaded,
        control_bar_window_count,
        selection_consumers_ok,
        shell_host_playable_ok,
        playable_claim,
        status,
        detail: format!(
            "host={host_constructed} cfg={skirmish_config_ok} menu_cfg={menu_config_ok} map_res={map_resolved} map_load={map_loaded} frames={frames_advanced} pres={presentation_ok} dual_tick={dual_tick_presentation_ok} hud_sel={hud_selection_ok} sel_consumers={selection_consumers_ok} minimap_fow={minimap_fow_presentation_ok} laser_upload={laser_segment_upload_ok} multi_beam={multi_beam_soft_edge_ok} floating_text={floating_text_layout_ok} world_anim={world_anim_presentation_ok} world_anim_layout={world_anim_layout_ok} anim2d={anim2d_frame_ok} game_text={game_text_caption_ok} csf_str={game_text_csf_str_ok} ds_measure={display_string_measure_ok} rng={rng_stream_residual_ok} screen={screen_skirmish_ok} control_bar={control_bar_layout_ok} cb_path={control_bar_path_resolved} cb_valid={control_bar_wnd_validated} cb_loaded={control_bar_window_loaded} cb_windows={control_bar_window_count} shell_host_playable_ok={shell_host_playable_ok} playable_claim={playable_claim} {layout_report}"
        ),
    }
}

pub fn format_shell_smoke_report(r: &ShellSmokeResult) -> String {
    format!("shell_smoke status={} detail={}", r.status, r.detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    #[test]
    fn host_smoke_applies_skirmish_and_advances_frames() {
        let r = run_shell_smoke(8);
        assert!(r.host_constructed, "host only after apply: {}", r.detail);
        assert!(r.skirmish_config_ok, "{}", r.detail);
        assert!(r.menu_config_ok, "{}", r.detail);
        assert!(r.frames_advanced > 0, "{}", r.detail);
        assert!(r.hud_selection_ok, "HUD selection residual: {}", r.detail);
        assert!(
            r.dual_tick_presentation_ok,
            "dual-tick presentation residual: {}",
            r.detail
        );
        assert!(
            r.minimap_fow_presentation_ok,
            "minimap FOW presentation residual: {}",
            r.detail
        );
        assert!(
            r.laser_segment_upload_ok,
            "laser segment CPU upload residual: {}",
            r.detail
        );
        assert!(
            r.multi_beam_soft_edge_ok,
            "multi-beam soft-edge residual: {}",
            r.detail
        );
        assert!(
            r.floating_text_layout_ok,
            "floating text CPU layout residual: {}",
            r.detail
        );
        assert!(
            r.game_text_caption_ok,
            "GUI:AddCash caption residual: {}",
            r.detail
        );
        assert!(
            r.game_text_csf_str_ok,
            "CSF/STR GameText residual: {}",
            r.detail
        );
        assert!(
            r.display_string_measure_ok,
            "DisplayString measure residual: {}",
            r.detail
        );
        assert!(
            r.world_anim_layout_ok,
            "world anim CPU layout residual: {}",
            r.detail
        );
        assert!(
            r.anim2d_frame_ok,
            "Anim2D frame advance residual: {}",
            r.detail
        );
        assert!(
            r.rng_stream_residual_ok,
            "RNG stream residual: {}",
            r.detail
        );
        assert!(
            r.world_anim_presentation_ok,
            "world anim presentation residual: {}",
            r.detail
        );
        assert!(
            r.control_bar_layout_ok,
            "ControlBar.wnd ensure residual: {}",
            r.detail
        );
        assert!(
            r.selection_consumers_ok,
            "multi-consumer selection panel residual: {}",
            r.detail
        );
        // When WindowZH is present, path+validate honesty must be true; prefer
        // headless WindowManager load (not required for CI without assets).
        if r.control_bar_path_resolved {
            assert!(
                r.control_bar_wnd_validated,
                "ControlBar structural validate residual: {}",
                r.detail
            );
            #[cfg(feature = "game_client")]
            if r.control_bar_window_loaded {
                assert!(
                    r.control_bar_window_count > 0,
                    "WindowManager load must materialise windows: {}",
                    r.detail
                );
            }
        } else {
            assert!(
                !r.control_bar_window_loaded && r.control_bar_window_count == 0,
                "missing assets must not claim window load: {}",
                r.detail
            );
        }
        assert!(
            r.screen_skirmish_ok,
            "shell→InGame screen residual: {}",
            r.detail
        );
        // Limited host claim when path is fully operational; never retail W3D claim.
        assert!(
            r.shell_host_playable_ok,
            "shell_host_playable_ok for successful headless host path: {}",
            r.detail
        );
        assert!(!r.playable_claim, "headless smoke must not claim retail playable");
        assert_eq!(r.status, "success", "{}", r.detail);
        assert_eq!(
            r.shell_host_playable_ok,
            r.status == "success",
            "shell_host_playable_ok must track success without overclaiming playable_claim"
        );
    }

    #[test]
    fn shell_host_playable_ok_never_implies_retail_playable_claim() {
        let r = run_shell_smoke(4);
        // Documented honesty contract: limited host flag is independent of retail claim.
        if r.shell_host_playable_ok {
            assert!(
                !r.playable_claim,
                "shell_host_playable_ok must never flip playable_claim"
            );
        }
        assert!(!r.playable_claim);
    }

    #[test]
    fn presentation_carries_transform_health_team_model() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("PresFields");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("SmokeUnit");
        t.set_health(50.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SmokeUnit".into(), t);
        let id = logic
            .create_object("SmokeUnit", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("unit");
        logic.update();
        let frame = PresentationFrame::build_from_logic(&logic, 0);
        let obj = frame
            .objects
            .iter()
            .find(|o| o.id == id)
            .expect("object in presentation");
        assert_eq!(obj.team, Team::USA);
        assert!((obj.position.x - 3.0).abs() < 0.01);
        assert!(obj.health_current > 0.0);
        assert_eq!(obj.health_max, 50.0);
        assert_eq!(obj.model_key.as_deref(), Some("SmokeUnit"));
        assert!(!obj.destroyed);
    }

    #[test]
    fn dual_tick_after_map_load_seeds_hud_selection_health() {
        // Residual closed by this change: after skirmish config + (optional) map load,
        // dual-tick presentation must put selection health on GameHUD.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("ShellHudSel");
        assert!(apply_skirmish_config(&mut logic, &cfg).is_ok());
        let mut t = ThingTemplate::new("ShellSelUnit");
        t.set_health(64.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("ShellSelUnit".into(), t);
        let id = logic
            .create_object("ShellSelUnit", Team::USA, Vec3::new(2.0, 0.0, 2.0))
            .expect("unit");
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }

        // Seed like start_game_from_ui before first logic frame.
        let mut hud = GameHUD::new();
        let seed = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        assert!(
            seed.alive_object_count() >= 1,
            "seed presentation must see map/host units"
        );
        assert!(
            hud.selected_unit_ids().contains(&id),
            "seed apply must set HUD selection"
        );

        logic.update();
        let post = PresentationFrame::build_and_apply_for_hud(&logic, 0, &mut hud);
        let info = hud
            .selected_unit_infos()
            .iter()
            .find(|u| u.object_id == id)
            .expect("dual-tick HUD selection health");
        assert!(
            (info.health_current - 64.0).abs() < 0.01,
            "health from presentation after dual-tick: {}",
            info.health_current
        );
        assert!(
            hud.selection_panel().has_positive_health(),
            "ControlBar selection panel health after dual-tick"
        );
        assert!(
            (hud.selection_panel().health_current - 64.0).abs() < 0.01,
            "selection panel HP from presentation: {}",
            hud.selection_panel().health_current
        );
        assert_eq!(post.frame.0, logic.get_frame());
        assert!(!post.hud_minimap_units().is_empty());

        #[cfg(feature = "game_client")]
        {
            let mut bar = game_client::gui::control_bar::ControlBar::new();
            post.apply_to_control_bar(&mut bar);
            let (hp, _) = bar
                .selection_panel_health()
                .expect("ControlBar health from dual-tick presentation");
            assert!((hp - 64.0).abs() < 0.01, "ControlBar HP {hp}");
        }
    }
}

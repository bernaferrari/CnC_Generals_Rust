//! Render / load-screen presentation-identity residual tests.

pub use super::*;

fn load_screen_init_prefers_presentation_roster() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains(
            "Prefer presentation roster when installed (InGame residual); live only boot/menu"
        ) && eng.contains("Boot residual only — no presentation roster yet")
            && eng.contains("frame.player_info(local_id)"),
        "load_screen_init_context must prefer presentation roster when frame installed"
    );
}

fn render_execute_passes_none_game_logic_when_presentation_installed() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("set_presentation_frame(self.last_presentation_frame.clone())")
            && eng.contains("PresentationFrame::build_from_logic")
            && eng.contains("last_presentation_frame.is_none()"),
        "engine must seed presentation before execute (no live GameLogic dual-read)"
    );
    // Structural: execute call site prefers None when presentation is Some.
    let idx = eng
        .find("self.render_pipeline.execute(")
        .expect("execute call");
    let window = &eng[idx..idx + 450];
    assert!(
        !window.contains("Some(&self.game_logic)"),
        "execute must not pass live GameLogic (presentation-only boundary): {window}"
    );
    assert!(
        eng.contains("PresentationFrame::build_from_logic")
            && eng.contains("last_presentation_frame.is_none()"),
        "engine must seed a presentation frame before execute when missing"
    );
    let rp = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    assert!(
        rp.contains("Boot residual only — presentation unit_render_inputs owns model/transform"),
        "pipeline live get_template must be boot residual only"
    );
}

fn defeat_alliance_prefer_presentation_no_live_dual_read_when_frame_installed() {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        src.contains(
            "Boot residual only — no live dual-read when a presentation frame is installed"
        ) && src.contains("Presentation installed but roster miss — fail-closed id-only residual")
            && src
                .contains("Prefer presentation roster team when installed; live only if no frame")
            && src.contains("else if self.last_presentation_frame.is_none()")
            && src.contains("Boot residual only — presentation local_team owns InGame Ctrl+A")
            && src.contains("Boot residual only — presentation local_team owns InGame Tab cycle")
            && src.contains(
                "Boot residual only — presentation local_team owns InGame similar-select"
            )
            && src.contains("Boot residual only — presentation local_team owns InGame box-select")
            && src
                .contains("Boot residual only — presentation local_team owns InGame attack-click")
            && src.contains(
                "Boot residual only — presentation local_team owns InGame control-group select"
            )
            && src.contains(
                "Boot residual only — presentation filter_alive_selectable_ids owns InGame"
            )
            && src.contains(
                "Boot residual only — presentation centroid_of_ids owns InGame double-tap"
            )
            && src
                .contains("Boot residual only — presentation box_select_unit_ids owns InGame path")
            && src.contains("Boot residual only — presentation similar_unit_ids owns InGame path"),
        "defeat/alliance/selection must not dual-read get_player when presentation frame is installed"
    );
}

fn legacy_render_stubs_prefer_presentation_identity() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    assert!(
        eng.contains("Prefer presentation identity when installed (no live get_objects dual-read)"),
        "render_game_objects stub must prefer presentation"
    );
    assert!(
        eng.contains(
            "Prefer presentation selected residual when installed (no live find_object dual-read)"
        ),
        "render_selection_indicators stub must prefer presentation"
    );
    assert!(
        eng.contains("selection_renderer is sole path"),
        "must document selection_renderer as production path"
    );
}

// Lifecycle: UI / render presentation residual latches, polled on every
// host status read (presentation frame, shell WND, world-draw mesh pass,
// retail sit-through flags).

/// Presentation honesty residual from host status every poll.
fn latch_presentation_frame_residuals(st: &mut SmokeRunState, snap: &StatusSnap) {
    // Presentation honesty residual from host status every poll.
    if snap.presentation_frame_ok {
        st.saw_presentation_frame_ok = true;
    }
    if snap.presentation_frame_ok && snap.presentation_live_fallback_reads == 0 {
        st.saw_presentation_live_fallback_ok = true;
    }
    if snap.gameworld_presentation_entities > 0 {
        st.saw_gameworld_presentation_entities_ok = true;
        st.max_gameworld_presentation_entities =
            st.max_gameworld_presentation_entities.max(snap.gameworld_presentation_entities);
    }
    if snap.gameworld_overlay_stamped > 0 {
        st.saw_gameworld_overlay_stamped_ok = true;
        st.max_gameworld_overlay_stamped =
            st.max_gameworld_overlay_stamped.max(snap.gameworld_overlay_stamped);
    }
    if snap.gameworld_appended > 0 {
        st.max_gameworld_appended = st.max_gameworld_appended.max(snap.gameworld_appended);
    }
    if snap.gameworld_rebuilt > 0 {
        st.saw_gameworld_rebuilt_ok = true;
        st.max_gameworld_rebuilt = st.max_gameworld_rebuilt.max(snap.gameworld_rebuilt);
    }
    if snap.presentation_frame_ok || snap.presentation_live_fallback_reads > 0 {
        st.presentation_detail = format!(
            "frame_ok={} live_fallback={}",
            snap.presentation_frame_ok, snap.presentation_live_fallback_reads
        );
    }
}

/// Retail shell WND residual: active shell with MainMenu/Skirmish layout.
fn latch_shell_wnd_residuals(st: &mut SmokeRunState, snap: &StatusSnap) {
    // Retail shell WND residual: active shell with MainMenu/Skirmish layout.
    let top = snap.shell_top_wnd.to_ascii_lowercase();
    let wnd_layout =
        top.contains("mainmenu.wnd") || top.contains("skirmish") || top.contains("menus/");
    if snap.shell_active && snap.shell_screen_count > 0 && wnd_layout {
        st.saw_shell_wnd_ok = true;
        st.shell_wnd_detail = format!(
            "active={} count={} top={}",
            snap.shell_active, snap.shell_screen_count, snap.shell_top_wnd
        );
    } else if snap.shell_screen_count > 0
        || snap.shell_active
        || !snap.shell_top_wnd.is_empty()
    {
        st.shell_wnd_detail = format!(
            "active={} count={} top={}",
            snap.shell_active, snap.shell_screen_count, snap.shell_top_wnd
        );
    }
}

/// InGame world-draw residual: peak + stability of mesh pass item count.
fn latch_render_item_residuals(st: &mut SmokeRunState, snap: &StatusSnap) {
    // InGame world-draw residual: peak + stability of mesh pass item count.
    if matches!(snap.state.as_str(), "InGame" | "Paused") {
        st.max_render_item_count = st.max_render_item_count.max(snap.render_item_count);
        st.max_render_alive_objects = st.max_render_alive_objects.max(snap.render_alive_objects);
        if snap.render_item_count > 0 {
            st.render_items_nonzero_polls = st.render_items_nonzero_polls.saturating_add(1);
        }
    }
}

/// Latch host residuals every poll — step boundaries can miss a one-frame
/// last_gameplay_cmd when the control loop is busy or a later command lands
/// first.
fn latch_retail_flag_residuals(st: &mut SmokeRunState, snap: &StatusSnap) {
    // Latch host residuals every poll — step boundaries can miss a one-frame
    // last_gameplay_cmd when the control loop is busy or a later command lands first.
    if snap.live_frame_ok {
        st.saw_live_frame_ok = true;
    }
    if snap.window_visible {
        st.saw_window_visible = true;
    }
    if snap.wnd_widget_tree_nav {
        st.saw_wnd_widget_tree_nav = true;
    }
    if snap.interactive_gameplay {
        st.saw_interactive_gameplay = true;
    }
    if snap.physical_build_and_produce {
        st.saw_physical_build_and_produce = true;
    }
    if snap.physical_gather_resources {
        st.saw_physical_gather_resources = true;
    }
    if snap.physical_save_load_continue {
        st.saw_physical_save_load_continue = true;
    }
}

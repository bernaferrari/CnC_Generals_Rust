// Lifecycle: shutdown — merge poll latches into the result, then apply the
// host vertical-slice / playable-claim gates.

/// Wave 839: ensure presentation honesty counters are latched before the
/// vertical gate. Always merge poll latches here — early child death /
/// partial exit skips the phase-2 assignment block, and must not drop
/// live_frame_ok / interactive_gameplay.
fn merge_shutdown_residuals(result: &mut ExecutableSmokeResult, st: &SmokeRunState) {
    result.presentation_frame_ok = result.presentation_frame_ok || st.saw_presentation_frame_ok;
    result.window_visible = result.window_visible || st.saw_window_visible;
    result.wnd_widget_tree_nav = result.wnd_widget_tree_nav || st.saw_wnd_widget_tree_nav;
    result.interactive_gameplay = result.interactive_gameplay || st.saw_interactive_gameplay;
    result.live_frame_ok = result.live_frame_ok || st.saw_live_frame_ok;
    result.physical_build_and_produce =
        result.physical_build_and_produce || st.saw_physical_build_and_produce;
    result.physical_gather_resources =
        result.physical_gather_resources || st.saw_physical_gather_resources;
    result.physical_save_load_continue =
        result.physical_save_load_continue || st.saw_physical_save_load_continue;
    // Fifth retail claim flag is interactive_gameplay alone (status `gameplay=` /
    // RMB latch). Host gameplay_cmd_ok remains a separate residual for the
    // vertical-slice / command chain and must NOT OR into playable_claim.
    result.presentation_live_fallback_ok =
        result.presentation_live_fallback_ok || st.saw_presentation_live_fallback_ok;
    result.shell_wnd_ok = result.shell_wnd_ok || st.saw_shell_wnd_ok;
    result.max_render_item_count = result.max_render_item_count.max(st.max_render_item_count);
    result.max_render_alive_objects = result
        .max_render_alive_objects
        .max(st.max_render_alive_objects);
    result.render_items_stable_ok = result.render_items_stable_ok
        || (result.reached_ingame && st.max_render_item_count > 0 && st.render_items_nonzero_polls >= 3);
    result.gameworld_presentation_entities_ok =
        result.gameworld_presentation_entities_ok || st.saw_gameworld_presentation_entities_ok;
    result.max_gameworld_presentation_entities = result
        .max_gameworld_presentation_entities
        .max(st.max_gameworld_presentation_entities);
    result.gameworld_overlay_stamped_ok =
        result.gameworld_overlay_stamped_ok || st.saw_gameworld_overlay_stamped_ok;
    result.max_gameworld_overlay_stamped = result
        .max_gameworld_overlay_stamped
        .max(st.max_gameworld_overlay_stamped);
    result.max_gameworld_appended = result.max_gameworld_appended.max(st.max_gameworld_appended);
    result.max_gameworld_rebuilt = result.max_gameworld_rebuilt.max(st.max_gameworld_rebuilt);
    result.gameworld_rebuilt_ok = result.gameworld_rebuilt_ok || st.saw_gameworld_rebuilt_ok;
    // Host command residuals observed during polls (even if chain cut short).
    result.gameplay_cmd_ok = result.gameplay_cmd_ok
        || (st.saw_select_ok && st.saw_move_ok && st.saw_attack_ok)
        || (st.saw_select_ok && st.saw_move_ok && st.saw_construct_ok && st.saw_train_ok)
        || (st.saw_construct_ok && st.saw_train_ok && st.saw_attack_ok)
        || (st.saw_construct_ok && (st.saw_attack_ok || st.saw_attack_move_ok || st.saw_combat_damage))
        || (st.saw_select_ok
            && st.saw_move_ok
            && (st.saw_attack_ok || st.saw_attack_move_ok || st.saw_construct_ok));
    result.construct_cmd_ok = result.construct_cmd_ok || st.saw_construct_ok;
    result.train_cmd_ok = result.train_cmd_ok || st.saw_train_ok;
    // Wave 176: presentation boundary + host vertical slice on the result itself.
    result.apply_host_vertical_slice_gate();
}

fn runtime_host_wnd_enabled() -> bool {
    !matches!(
        std::env::var("GENERALS_RUNTIME_HOST_WND")
            .unwrap_or_else(|_| "1".into())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

/// When WND push is enabled, limited host_ok also requires shell_wnd residual.
fn executable_host_ok_from_residuals(reached_ingame: bool, shell_wnd_ok: bool) -> bool {
    if !reached_ingame {
        return false;
    }
    if runtime_host_wnd_enabled() {
        shell_wnd_ok
    } else {
        true
    }
}

// Lifecycle: runtime-host status protocol — StatusSnap, parse_status, and
// control-file writes.

#[derive(Debug, Default, Clone)]
struct StatusSnap {
    state: String,
    ui_screen: String,
    /// Sticky host residual: skirmish menu was opened (survives InGame clear).
    skirmish_menu_ok: bool,
    map: String,
    frame: u32,
    startup_progress: f32,
    startup_phase: String,
    selected_count: u32,
    local_mobile_units: u32,
    under_construction: u32,
    match_damage_applied: f32,
    match_kills: u32,
    last_gameplay_cmd: String,
    match_over: bool,
    victory_label: String,
    presentation_frame_ok: bool,
    gameworld_presentation_entities: u32,
    gameworld_overlay_stamped: u32,
    gameworld_appended: u32,
    gameworld_rebuilt: u32,
    gameworld_primary_objects: bool,
    shell_screen_count: u32,
    shell_top_wnd: String,
    shell_active: bool,
    presentation_live_fallback_reads: u32,
    waypoint_mode: bool,
    live_frame_ok: bool,
    window_visible: bool,
    wnd_widget_tree_nav: bool,
    interactive_gameplay: bool,
    /// Physical workflow evidence. These intentionally have no fallback to
    /// `last_gameplay_cmd`, which is a runtime-host control-channel diagnostic.
    physical_build_and_produce: bool,
    physical_gather_resources: bool,
    physical_save_load_continue: bool,
    retail_sit_through_missing: String,
    render_item_count: u32,
    render_alive_objects: u32,
    render_fow_filtered: u32,
    render_frustum_culled: u32,
}

fn parse_status(path: &Path) -> Option<StatusSnap> {
    let text = fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    let mut snap = StatusSnap::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "state" => snap.state = v.trim().to_string(),
            "ui_screen" => snap.ui_screen = v.trim().to_string(),
            "skirmish_menu_ok" => {
                snap.skirmish_menu_ok =
                    matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes");
            }
            "map" => snap.map = v.trim().to_string(),
            "frame" => snap.frame = v.trim().parse().unwrap_or(0),
            "startup_progress" => snap.startup_progress = v.trim().parse().unwrap_or(0.0),
            "startup_phase" => snap.startup_phase = v.trim().to_string(),
            "selected_count" => snap.selected_count = v.trim().parse().unwrap_or(0),
            "local_mobile_units" => snap.local_mobile_units = v.trim().parse().unwrap_or(0),
            "under_construction" => snap.under_construction = v.trim().parse().unwrap_or(0),
            "match_damage_applied" => {
                snap.match_damage_applied = v.trim().parse().unwrap_or(0.0);
            }
            "match_kills" => snap.match_kills = v.trim().parse().unwrap_or(0),
            "last_gameplay_cmd" => snap.last_gameplay_cmd = v.trim().to_string(),
            "presentation_frame_ok" => {
                snap.presentation_frame_ok = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "gameworld_presentation_entities" => {
                snap.gameworld_presentation_entities = v.trim().parse().unwrap_or(0);
            }
            "gameworld_overlay_stamped" => {
                snap.gameworld_overlay_stamped = v.trim().parse().unwrap_or(0);
            }
            "gameworld_appended" => {
                snap.gameworld_appended = v.parse().unwrap_or(0);
            }
            "gameworld_rebuilt" => {
                snap.gameworld_rebuilt = v.trim().parse().unwrap_or(0);
            }
            "gameworld_primary_objects" => {
                snap.gameworld_primary_objects = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "shell_screen_count" => {
                snap.shell_screen_count = v.trim().parse().unwrap_or(0);
            }
            "shell_top_wnd" => snap.shell_top_wnd = v.trim().to_string(),
            "shell_active" => {
                snap.shell_active = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "presentation_live_fallback_reads" => {
                snap.presentation_live_fallback_reads = v.trim().parse().unwrap_or(0);
            }
            "waypoint_mode" => {
                snap.waypoint_mode = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "live_frame_ok" => {
                snap.live_frame_ok = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "window_visible" => {
                snap.window_visible = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "wnd_widget_tree_nav" => {
                snap.wnd_widget_tree_nav = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "gameplay" | "interactive_gameplay" => {
                snap.interactive_gameplay = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "physical_build_and_produce" => {
                snap.physical_build_and_produce = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "physical_gather_resources" => {
                snap.physical_gather_resources = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "physical_save_load_continue" => {
                snap.physical_save_load_continue = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "retail_sit_through_missing" => {
                snap.retail_sit_through_missing = v.trim().to_string();
            }
            "render_item_count" => {
                snap.render_item_count = v.trim().parse().unwrap_or(0);
            }
            "render_alive_objects" => {
                snap.render_alive_objects = v.trim().parse().unwrap_or(0);
            }
            "render_fow_filtered" => {
                snap.render_fow_filtered = v.trim().parse().unwrap_or(0);
            }
            "render_frustum_culled" => {
                snap.render_frustum_culled = v.trim().parse().unwrap_or(0);
            }
            "match_over" => snap.match_over = matches!(v.trim(), "true" | "1" | "True"),
            "victory_label" => snap.victory_label = v.trim().to_string(),
            _ => {}
        }
    }
    Some(snap)
}

fn write_control(path: &Path, lines: &[&str]) -> std::io::Result<()> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    for line in lines {
        writeln!(f, "{line}")?;
    }
    f.flush()
}

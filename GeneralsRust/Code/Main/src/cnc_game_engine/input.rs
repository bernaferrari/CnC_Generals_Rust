#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

/// Origin of a mouse-button event that reaches [`CnCGameEngine::handle_mouse_button_input`].
///
/// `Physical` is a real OS `WindowEvent::MouseInput`. `Injected` is a host
/// control-file / `inject_winit_equivalent_*` re-entry. Both share WM dispatch
/// and world orders; only `Physical` may latch playable_claim evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MouseInputOrigin {
    Physical,
    Injected,
}

impl CnCGameEngine {
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            if let Err(err) = ww3d_engine::resize(new_size.width.max(1), new_size.height.max(1)) {
                warn!("WW3D resize failed: {err:?}");
            }

            // Update projection matrix
            self.projection_matrix = Mat4::perspective_rh(
                DEFAULT_VIEW_FOV_RADIANS,
                new_size.width as f32 / new_size.height as f32,
                DEFAULT_VIEW_NEAR_CLIP,
                DEFAULT_VIEW_FAR_CLIP,
            );
            self.ui_manager.resize(new_size.width, new_size.height);
        }
    }

    /// Process platform-specific window events through message handler
    pub fn process_platform_event(&mut self, event: &Event<()>) -> Result<bool> {
        self.message_processor.process_event(event)
    }

    /// Check if quit has been requested through platform message handling
    pub fn is_quit_requested(&self) -> bool {
        self.message_processor.is_quit_requested()
    }

    /// Set fullscreen mode and notify platform handler
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        info!("🖥️ Setting fullscreen mode: {}", fullscreen);

        // Update the message processor's fullscreen state
        self.message_processor.set_fullscreen(fullscreen);

        // In a complete implementation, we would:
        // 1. Change the winit window to fullscreen/windowed
        // 2. Reconfigure the surface
        // 3. Update render targets

        if fullscreen {
            info!("Switching to fullscreen mode");
            // self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            info!("Switching to windowed mode");
            // self.window.set_fullscreen(None);
        }

        self.is_windowed = !fullscreen;

        Ok(())
    }

    /// Get current application focus state
    pub fn is_application_active(&self) -> bool {
        self.message_processor.is_active()
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        physical_key,
                        state,
                        ..
                    },
                ..
            } => {
                #[cfg(feature = "game_client")]
                {
                    let pressed = matches!(state, ElementState::Pressed);
                    self.inject_game_client_key(physical_key, pressed);
                }
                let pressed = matches!(state, ElementState::Pressed);
                #[cfg(feature = "game_client")]
                if pressed && self.current_state == GameState::Menu && !self.runtime_host_headless {
                    // C++ MainMenuInput GWM_CHAR first-run reveal on first real key.
                    let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
                }
                let wnd_used = self.dispatch_os_key_to_window_manager(physical_key, pressed);
                let route_keyboard_to_legacy_ui = !wnd_used
                    && matches!(
                        self.current_state,
                        GameState::InGame | GameState::Paused | GameState::Menu
                    );
                match state {
                    ElementState::Pressed => {
                        self.keys_pressed.insert(key.clone());
                        let mut construction_consumed = false;
                        if route_keyboard_to_legacy_ui {
                            if let Some(ui_key) = Self::to_ui_key_code(key) {
                                let _ = self.ui_manager.handle_key_press(ui_key);
                                // Dual HUD residual: engine GameHUD owns construction
                                // cameo hotkeys + placement cancel from presentation path.
                                // When construction panel consumes a build/cancel key, skip
                                // global command hotkeys (R repair vs R ranger, etc.).
                                use crate::ui::Interactive;
                                construction_consumed =
                                    Interactive::handle_key_press(&mut self.game_hud, ui_key);
                                for ev in self.game_hud.drain_pending_ui_events() {
                                    self.ui_manager.queue_event(ev);
                                }
                            }
                        }
                        // Retail numpad camera residual (physical keys).
                        match physical_key {
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad5,
                            ) => self.reset_camera_view_hotkey(),
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad4,
                            ) => {
                                self.camera_rotate_left_held = true;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad6,
                            ) => {
                                self.camera_rotate_right_held = true;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad8,
                            ) => {
                                self.camera_zoom_in_held = true;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad2,
                            ) => {
                                self.camera_zoom_out_held = true;
                            }
                            _ if wnd_used => {
                                // C++ WindowXlat consumed the key (shell or focused gadget).
                            }
                            _ if construction_consumed => {
                                // Construction cameo / Escape placement cancel residual.
                            }
                            _ => self.handle_key_press(key),
                        }
                    }
                    ElementState::Released => {
                        self.keys_pressed.remove(key);
                        match physical_key {
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad4,
                            ) => {
                                self.camera_rotate_left_held = false;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad6,
                            ) => {
                                self.camera_rotate_right_held = false;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad8,
                            ) => {
                                self.camera_zoom_in_held = false;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad2,
                            ) => {
                                self.camera_zoom_out_held = false;
                            }
                            _ => {}
                        }
                    }
                }
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                // OS window event — the only origin that may latch physical
                // playability evidence. Injected control-file clicks use
                // MouseInputOrigin::Injected and must not set those flags.
                let pressed = matches!(state, ElementState::Pressed);
                self.handle_mouse_button_input(*button, pressed, MouseInputOrigin::Physical);
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.apply_cursor_position(position.x as f32, position.y as f32);
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let x = self.mouse_position.0 as i32;
                let y = self.mouse_position.1 as i32;
                let wnd_used = self.dispatch_os_mouse_wheel(delta, x, y);
                if matches!(self.current_state, GameState::InGame | GameState::Paused) && !wnd_used
                {
                    self.handle_mouse_wheel(delta);
                }
                true
            }
            _ => false,
        }
    }

    /// Shared cursor move path for physical winit `CursorMoved` and host inject.
    pub(super) fn apply_cursor_position(&mut self, x: f32, y: f32) {
        self.mouse_position = (x, y);
        #[cfg(feature = "game_client")]
        self.inject_game_client_mouse_move(x, y);
        let mx = x as i32;
        let my = y as i32;
        let _ = self.dispatch_os_mouse_move(mx, my);
        self.ui_manager.handle_mouse_move(mx, my);
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.update_mouse_world_position();
            self.sync_context_mouse_cursor();
        }
    }

    /// Shared mouse-button path for physical winit `MouseInput` and host inject.
    ///
    /// Both origins run real WM dispatch + under-cursor hit + (InGame) world
    /// orders. Playability evidence (`note_menu_wnd_click` /
    /// `note_skirmish_path_gadget` / `note_gameplay_order`) latches only when
    /// `origin == Physical` and the runtime is not headless. Injected
    /// `winit_menu_nav` / `winit_gameplay_order` drive the same handler so
    /// automation can click, but they do **not** prove a human mouse
    /// playthrough.
    pub(super) fn handle_mouse_button_input(
        &mut self,
        button: MouseButton,
        pressed: bool,
        origin: MouseInputOrigin,
    ) {
        let x = self.mouse_position.0 as i32;
        let y = self.mouse_position.1 as i32;
        #[cfg(feature = "game_client")]
        {
            self.inject_game_client_mouse_button(button, pressed);
        }
        #[cfg(feature = "game_client")]
        if pressed && self.current_state == GameState::Menu && !self.runtime_host_headless {
            // C++ first-run reveal on first real mouse (CHAR path).
            let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
        }
        // Real WM process_mouse_event residual (Used / NotUsed) — never forged.
        let wnd_used_raw =
            self.dispatch_os_mouse_to_window_manager(button, pressed, x, y, origin);
        #[cfg(feature = "game_client")]
        let live_hit = game_client::gui::note_os_wnd_widget_tree_hit(x, y);
        #[cfg(not(feature = "game_client"))]
        let live_hit = false;
        let hit_wnd_widget = live_hit;
        let in_world = matches!(self.current_state, GameState::InGame | GameState::Paused);
        // InGame RMB: shell/HUD may soft-report Used without a live gadget under the
        // cursor (fullscreen residual). World orders require a real gadget hit to
        // steal the click — same honesty as physical: empty world → handle_right_click.
        let wnd_used = if in_world && matches!(button, MouseButton::Right) {
            wnd_used_raw && live_hit
        } else {
            wnd_used_raw
        };
        if matches!(button, MouseButton::Right) && in_world {
            log::info!(
                "RMB input pressed={pressed} wnd_used_raw={wnd_used_raw} live_hit={live_hit} wnd_used={wnd_used} pos=({x},{y})"
            );
        }

        // Shell marks every click as Used while active, so require an enabled
        // gadget hit as well before counting menu navigation.
        if pressed && matches!(button, MouseButton::Left) && self.current_state == GameState::Menu {
            // Physical OS mouse only. Injected control-file clicks must not
            // latch wnd_widget_tree_nav / playable_claim.
            let physical =
                matches!(origin, MouseInputOrigin::Physical) && !self.runtime_host_headless;
            self.interactive_playability
                .note_menu_wnd_click(physical, wnd_used, hit_wnd_widget);
            #[cfg(feature = "game_client")]
            if let Some(name) = game_client::gui::os_wnd_widget_under_cursor_name(x, y) {
                self.interactive_playability
                    .note_skirmish_path_gadget(physical, &name);
            }
        }
        // One input path:
        // - Shell/modal WND: WindowManager only (do not also soft UI).
        // - InGame world: handle_left/right_click + presentation picks.
        //   GameHUD cameo fallback only when WND did not consume.
        #[cfg(feature = "game_client")]
        let shell_active = game_client::gui::get_shell().is_shell_active();
        #[cfg(not(feature = "game_client"))]
        let shell_active = self.shell_menu_active;
        let mut hud_cameo_consumed = false;
        if let Some(ui_button) = Self::to_ui_mouse_button(button) {
            if shell_active {
                // Exclusive WND while shell is up.
            } else if in_world && !wnd_used {
                if pressed {
                    hud_cameo_consumed = self.ui_manager.handle_mouse_click(x, y, ui_button);
                }
                self.ui_manager.handle_mouse_move(x, y);
            } else if !in_world && !wnd_used {
                if pressed {
                    let _ = self.ui_manager.handle_mouse_click(x, y, ui_button);
                }
                self.ui_manager.handle_mouse_move(x, y);
            }
        }

        if in_world && !wnd_used && !hud_cameo_consumed {
            match (button, pressed) {
                (MouseButton::Left, true) => {
                    self.handle_left_click();
                }
                (MouseButton::Left, false) => {
                    self.handle_left_release();
                }
                (MouseButton::Right, true) => {
                    // Set anchor for drag-scroll; the actual right-click command
                    // is deferred to release if the mouse didn't move
                    // significantly (C++ LookAtXlat.cpp).
                    self.rmb_scroll_anchor = Some(self.mouse_position);
                    self.is_rmb_scrolling = true;
                    self.rmb_scroll_started_physically =
                        matches!(origin, MouseInputOrigin::Physical);
                }
                (MouseButton::Right, false) => {
                    let physical_rmb_gesture = matches!(origin, MouseInputOrigin::Physical)
                        && self.rmb_scroll_started_physically;
                    if self.is_rmb_scrolling {
                        const DRAG_THRESHOLD_SQ: f32 = 9.0; // 3px squared
                        if let Some(anchor) = self.rmb_scroll_anchor {
                            let dx = self.mouse_position.0 - anchor.0;
                            let dy = self.mouse_position.1 - anchor.1;
                            if dx * dx + dy * dy < DRAG_THRESHOLD_SQ {
                                let had_selection = !self.selected_objects.is_empty()
                                    || !self.ui_selected_ids(self.current_player_id).is_empty();
                                // Physical + inject share this order residual.
                                log::info!(
                                    "RMB world order: had_sel={had_selection} match={} headless={}",
                                    self.interactive_playability.match_started_from_menu_wnd,
                                    self.runtime_host_headless
                                );
                                self.handle_right_click(origin, physical_rmb_gesture);
                                self.interactive_playability.note_gameplay_order(
                                    matches!(origin, MouseInputOrigin::Physical)
                                        && !self.runtime_host_headless,
                                    had_selection,
                                );
                                log::info!(
                                    "RMB world order latched gameplay_order={}",
                                    self.interactive_playability.gameplay_order
                                );
                            }
                        }
                    }
                    self.rmb_scroll_anchor = None;
                    self.is_rmb_scrolling = false;
                    self.rmb_scroll_started_physically = false;
                }
                (MouseButton::Middle, true) => {
                    self.is_mmb_rotating = true;
                    self.mmb_anchor = Some(self.mouse_position);
                }
                (MouseButton::Middle, false) => {
                    self.is_mmb_rotating = false;
                    self.mmb_anchor = None;
                }
                _ => {}
            }
        }
    }

    /// Move the logical cursor as if a physical winit `CursorMoved` arrived.
    /// Used by windowed sit-through inject only — not by `drive_os_wnd_*`.
    ///
    /// Sets engine cursor residual + GameClient soft mouse. Does **not** send
    /// `MousePos` through `process_mouse_event` (op-drain hang class on shell).
    /// Button inject still hits WM at these coordinates with full LeftDown/Up
    /// dispatch + under-cursor residual.
    pub(super) fn inject_winit_equivalent_cursor_at(&mut self, x: f32, y: f32) {
        self.mouse_position = (x, y);
        #[cfg(feature = "game_client")]
        self.inject_game_client_mouse_move(x, y);
        self.ui_manager.handle_mouse_move(x as i32, y as i32);
    }

    /// Press/release a mouse button through the shared WM path as **injected**
    /// (control-file) input. Same dispatch as physical; does **not** latch
    /// playable_claim evidence.
    pub(super) fn inject_winit_equivalent_mouse_button(
        &mut self,
        button: MouseButton,
        pressed: bool,
    ) {
        self.handle_mouse_button_input(button, pressed, MouseInputOrigin::Injected);
    }

    /// Resolve screen center of a live named WND gadget that is **under-cursor
    /// hit-tested** at that center (enabled, not hidden, positive size).
    ///
    /// Returns `None` when the gadget is missing, hidden, disabled, zero-sized,
    /// or `get_window_under_cursor` does not hit the named window (or a
    /// descendant). Geometry-only residual is **not** enough.
    #[cfg(feature = "game_client")]
    pub(super) fn named_gadget_center_if_hittable(&self, name: &str) -> Option<(i32, i32)> {
        use game_client::gui::window_manager::with_window_manager_ref;
        use std::rc::Rc;

        with_window_manager_ref(|manager| {
            let window = manager.find_window_by_name(name)?;
            let (sx, sy, w, h) = {
                let guard = window.try_borrow().ok()?;
                if guard.is_hidden() || !guard.is_enabled() {
                    return None;
                }
                let (sx, sy) = guard.get_screen_position();
                let (w, h) = guard.get_size();
                if w <= 0 || h <= 0 {
                    return None;
                }
                (sx, sy, w, h)
            };
            let x = sx + w / 2;
            let y = sy + h / 2;
            let hit = manager.get_window_under_cursor(x, y, false)?;
            // Named gadget or a descendant under its own center (C++ hit residual).
            let is_named_or_desc = {
                if Rc::ptr_eq(&window, &hit) {
                    true
                } else {
                    let mut current = hit.try_borrow().ok()?.get_parent();
                    let mut found = false;
                    while let Some(parent) = current {
                        if Rc::ptr_eq(&parent, &window) {
                            found = true;
                            break;
                        }
                        current = parent.try_borrow().ok().and_then(|g| g.get_parent());
                    }
                    found
                }
            };
            if !is_named_or_desc {
                return None;
            }
            Some((x, y))
        })
    }

    #[cfg(not(feature = "game_client"))]
    pub(super) fn named_gadget_center_if_hittable(&self, _name: &str) -> Option<(i32, i32)> {
        None
    }

    /// Left-click a live named WND gadget center through the winit-equivalent path.
    ///
    /// Returns true only when:
    /// 1. the named window is found, enabled, and under-cursor hit-tested at center, and
    /// 2. the click went through [`Self::handle_mouse_button_input`] as
    ///    [`MouseInputOrigin::Injected`] (real WM dispatch).
    ///
    /// Does **not** use `drive_os_wnd_*`, does **not** call `note_*` directly,
    /// and does **not** latch `menu_wnd_click` / playable_claim (inject ≠ physical).
    pub(super) fn inject_winit_equivalent_named_gadget_click(&mut self, name: &str) -> bool {
        let Some((x, y)) = self.named_gadget_center_if_hittable(name) else {
            return false;
        };
        self.inject_winit_equivalent_cursor_at(x as f32, y as f32);
        self.inject_winit_equivalent_mouse_button(MouseButton::Left, true);
        self.inject_winit_equivalent_mouse_button(MouseButton::Left, false);
        log::info!(
            "inject_named_gadget_click({name}): state={:?} dispatched=true (inject, not physical)",
            self.current_state
        );
        true
    }

    /// Right-click (press+release, no drag) through the **same** path as physical
    /// RMB: world branch requires `!wnd_used`, calls `handle_right_click()`, then
    /// may latch `note_gameplay_order` when selection is present and menu→match
    /// already completed.
    pub(super) fn inject_winit_equivalent_gameplay_order_click(&mut self) {
        // Ensure shell layouts are marked hidden before RMB (match start already
        // called hide, but ticks can leave residual MainMenu hit-testable).
        #[cfg(feature = "game_client")]
        {
            let _ = game_client::gui::try_with_shell_mut(|shell| {
                shell.hide(true);
                shell.set_shell_active(false);
            });
            self.shell_menu_active = false;
        }
        // Aim at a world point the shell/HUD does not own. Fullscreen residual
        // often reports Used + live hit at center; probe several view points and
        // pick the first without a live under-cursor gadget so RMB reaches
        // handle_right_click + note_gameplay_order (fifth claim flag).
        let size = self.window.inner_size();
        let w = size.width.max(2) as f32;
        let h = size.height.max(2) as f32;
        // Prefer mid-world / lower thirds (away from top menu chrome and bottom HUD).
        let candidates: [(f32, f32); 8] = [
            (0.50, 0.42),
            (0.55, 0.35),
            (0.45, 0.38),
            (0.60, 0.45),
            (0.40, 0.48),
            (0.50, 0.55),
            (0.70, 0.40),
            (0.30, 0.40),
        ];
        let mut cx = w * candidates[0].0;
        let mut cy = h * candidates[0].1;
        #[cfg(feature = "game_client")]
        {
            let mut found_clear = false;
            for (fx, fy) in candidates {
                let x = w * fx;
                let y = h * fy;
                if !game_client::gui::note_os_wnd_widget_tree_hit(x as i32, y as i32) {
                    cx = x;
                    cy = y;
                    found_clear = true;
                    break;
                }
            }
            if !found_clear {
                // Last resort: deep into lower-right world residual.
                cx = w * 0.72;
                cy = h * 0.58;
                log::info!(
                    "inject_gameplay_order_click: no clear gadget-free point; last-resort ({cx:.0},{cy:.0})"
                );
            } else {
                log::info!("inject_gameplay_order_click: clear world cursor=({cx:.0},{cy:.0})");
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            log::info!("inject_gameplay_order_click: cursor=({cx:.0},{cy:.0})");
        }
        self.inject_winit_equivalent_cursor_at(cx, cy);
        // If still gadget-owned after probe, force a second nudge (honest live residual).
        #[cfg(feature = "game_client")]
        {
            if game_client::gui::note_os_wnd_widget_tree_hit(cx as i32, cy as i32) {
                cx = w * 0.78;
                cy = h * 0.62;
                log::info!("inject_gameplay_order_click: still hit, force nudge ({cx:.0},{cy:.0})");
                self.inject_winit_equivalent_cursor_at(cx, cy);
            }
        }
        self.inject_winit_equivalent_mouse_button(MouseButton::Right, true);
        self.inject_winit_equivalent_mouse_button(MouseButton::Right, false);
    }

    pub(super) fn to_ui_mouse_button(button: MouseButton) -> Option<crate::ui::MouseButton> {
        match button {
            MouseButton::Left => Some(crate::ui::MouseButton::Left),
            MouseButton::Right => Some(crate::ui::MouseButton::Right),
            MouseButton::Middle => Some(crate::ui::MouseButton::Middle),
            MouseButton::Back => Some(crate::ui::MouseButton::Other(4)),
            MouseButton::Forward => Some(crate::ui::MouseButton::Other(5)),
            MouseButton::Other(id) => Some(crate::ui::MouseButton::Other(id as u8)),
        }
    }

    pub(super) fn to_ui_key_code(key: &Key) -> Option<crate::ui::KeyCode> {
        match key {
            Key::Named(NamedKey::Escape) => Some(crate::ui::KeyCode::Escape),
            Key::Named(NamedKey::Enter) => Some(crate::ui::KeyCode::Enter),
            Key::Named(NamedKey::Space) => Some(crate::ui::KeyCode::Space),
            Key::Named(NamedKey::Tab) => Some(crate::ui::KeyCode::Tab),
            Key::Named(NamedKey::Backspace) => Some(crate::ui::KeyCode::Backspace),
            Key::Named(NamedKey::Delete) => Some(crate::ui::KeyCode::Delete),
            Key::Named(NamedKey::Home) => Some(crate::ui::KeyCode::Home),
            Key::Named(NamedKey::End) => Some(crate::ui::KeyCode::End),
            Key::Named(NamedKey::ArrowLeft) => Some(crate::ui::KeyCode::Left),
            Key::Named(NamedKey::ArrowRight) => Some(crate::ui::KeyCode::Right),
            Key::Named(NamedKey::ArrowUp) => Some(crate::ui::KeyCode::Up),
            Key::Named(NamedKey::ArrowDown) => Some(crate::ui::KeyCode::Down),
            Key::Named(NamedKey::F1) => Some(crate::ui::KeyCode::F1),
            Key::Named(NamedKey::F2) => Some(crate::ui::KeyCode::F2),
            Key::Named(NamedKey::F3) => Some(crate::ui::KeyCode::F3),
            Key::Named(NamedKey::F4) => Some(crate::ui::KeyCode::F4),
            Key::Named(NamedKey::F5) => Some(crate::ui::KeyCode::F5),
            Key::Named(NamedKey::F6) => Some(crate::ui::KeyCode::F6),
            Key::Named(NamedKey::F7) => Some(crate::ui::KeyCode::F7),
            Key::Named(NamedKey::F8) => Some(crate::ui::KeyCode::F8),
            Key::Named(NamedKey::F9) => Some(crate::ui::KeyCode::F9),
            Key::Named(NamedKey::F10) => Some(crate::ui::KeyCode::F10),
            Key::Named(NamedKey::F11) => Some(crate::ui::KeyCode::F11),
            Key::Named(NamedKey::F12) => Some(crate::ui::KeyCode::F12),
            Key::Character(ch) if ch.len() == 1 => {
                let c = ch.chars().next()?;
                match c.to_ascii_uppercase() {
                    'A' => Some(crate::ui::KeyCode::A),
                    'B' => Some(crate::ui::KeyCode::B),
                    'C' => Some(crate::ui::KeyCode::C),
                    'D' => Some(crate::ui::KeyCode::D),
                    'E' => Some(crate::ui::KeyCode::E),
                    'F' => Some(crate::ui::KeyCode::F),
                    'G' => Some(crate::ui::KeyCode::G),
                    'H' => Some(crate::ui::KeyCode::H),
                    'I' => Some(crate::ui::KeyCode::I),
                    'J' => Some(crate::ui::KeyCode::J),
                    'K' => Some(crate::ui::KeyCode::K),
                    'L' => Some(crate::ui::KeyCode::L),
                    'M' => Some(crate::ui::KeyCode::M),
                    'N' => Some(crate::ui::KeyCode::N),
                    'O' => Some(crate::ui::KeyCode::O),
                    'P' => Some(crate::ui::KeyCode::P),
                    'Q' => Some(crate::ui::KeyCode::Q),
                    'R' => Some(crate::ui::KeyCode::R),
                    'S' => Some(crate::ui::KeyCode::S),
                    'T' => Some(crate::ui::KeyCode::T),
                    'U' => Some(crate::ui::KeyCode::U),
                    'V' => Some(crate::ui::KeyCode::V),
                    'W' => Some(crate::ui::KeyCode::W),
                    'X' => Some(crate::ui::KeyCode::X),
                    'Y' => Some(crate::ui::KeyCode::Y),
                    'Z' => Some(crate::ui::KeyCode::Z),
                    '0' => Some(crate::ui::KeyCode::Key0),
                    '1' => Some(crate::ui::KeyCode::Key1),
                    '2' => Some(crate::ui::KeyCode::Key2),
                    '3' => Some(crate::ui::KeyCode::Key3),
                    '4' => Some(crate::ui::KeyCode::Key4),
                    '5' => Some(crate::ui::KeyCode::Key5),
                    '6' => Some(crate::ui::KeyCode::Key6),
                    '7' => Some(crate::ui::KeyCode::Key7),
                    '8' => Some(crate::ui::KeyCode::Key8),
                    '9' => Some(crate::ui::KeyCode::Key9),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            // C++ shell/loading progression is driven by the engine's fixed update cadence, not
            // by renderer timing. WW3D timing can remain effectively stuck during startup/menu
            // and starve shell scripts/model streaming if we trust it here.
            self.update_with_frame_clock();
            return;
        }
        let dt = self.apply_frame_timing(*timing);
        self.update_internal(dt);
    }

    /// Allows external orchestrators (e.g., integration diagnostics pipeline) to push
    /// the latest subsystem health snapshot for the in-game debug overlay.
    pub fn set_diagnostics_overlay(&mut self, stats: DiagnosticsOverlayStats) {
        self.diagnostics_overlay = Some(stats);
    }

    #[cfg(feature = "integration-diagnostics")]
    pub fn set_integration_diagnostics(&mut self, diag: &SystemDiagnostics) {
        self.diagnostics_overlay = Some(DiagnosticsOverlayStats::from_system(diag));
    }

    /// Clears any externally provided diagnostics snapshot, falling back to
    /// locally-derived estimates.
    pub fn clear_diagnostics_overlay(&mut self) {
        self.diagnostics_overlay = None;
    }

    /// Advance simulation using the internal fallback clock (no WW3D timing).
    pub fn update_with_frame_clock(&mut self) {
        const SHELL_MENU_STEP: Duration = Duration::from_nanos(33_333_333);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.menu_loading_last_tick);
        self.menu_loading_last_tick = now;
        self.menu_loading_tick_accumulator =
            (self.menu_loading_tick_accumulator + elapsed).min(Duration::from_millis(250));

        if self.menu_loading_tick_accumulator < SHELL_MENU_STEP {
            return;
        }
        self.menu_loading_tick_accumulator -= SHELL_MENU_STEP;

        let clock_timing = self.frame_clock.advance_fixed(SHELL_MENU_STEP);
        let timing = Self::to_engine_timing(clock_timing, Instant::now());
        let dt = self.apply_frame_timing(timing);
        static UC_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let uc_n = UC_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if uc_n < 15 || (uc_n < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "update_with_frame_clock #{} start state={:?}",
                uc_n, self.current_state
            );
        }
        self.update_internal(dt);
        if uc_n < 15 || (uc_n < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "update_with_frame_clock #{} done state={:?}",
                uc_n, self.current_state
            );
        }
    }

    pub fn update(&mut self, dt: f32) {
        let delta = Duration::from_secs_f32(dt.max(0.0));
        let clock_timing = self.frame_clock.advance_fixed(delta);
        let timing = Self::to_engine_timing(clock_timing, Instant::now());
        let adjusted_dt = self.apply_frame_timing(timing);
        self.update_internal(adjusted_dt);
    }

    pub(super) fn apply_frame_timing(&mut self, timing: FrameTiming) -> f32 {
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            // Shell/loading frame cadence is managed by update_with_frame_clock() and event-loop
            // pacing. Running gameplay script FPS spin-waits here can stall the UI thread.
            self.script_fps_limit_last_tick = None;
        } else {
            self.apply_script_frame_limit();
        }
        NetworkClock::override_with_duration(timing.total_time);
        let dt = timing.delta_seconds().max(0.0);
        self.last_frame_timing = Some(timing);
        let incoming_frame = timing.frame_number as u32;
        self.frame_counter = if incoming_frame > self.frame_counter {
            incoming_frame
        } else {
            self.frame_counter.saturating_add(1)
        };
        if timing.fps > 0.0 {
            self.fps = timing.fps;
        } else if dt > 0.0 {
            self.fps = 1.0 / dt;
        }
        dt
    }

    /// Get current game state
    pub fn get_state(&self) -> GameState {
        self.current_state
    }

    /// Request state transition - will be applied at next update cycle
    /// Matches C++ GameEngine::setQuitting() pattern for deferred state changes
    pub fn request_state_change(&mut self, new_state: GameState) {
        if new_state == self.current_state || self.pending_state == Some(new_state) {
            return;
        }

        info!(
            "State transition requested: {:?} -> {:?}",
            self.current_state, new_state
        );
        self.pending_state = Some(new_state);
    }

    pub fn is_state_change_pending(&self, state: GameState) -> bool {
        self.pending_state == Some(state)
    }

    /// Process pending state transitions
    /// Called at beginning of update cycle to handle state changes
    pub(super) fn process_state_transitions(&mut self) {
        if let Some(new_state) = self.pending_state.take() {
            self.transition_to_state(new_state);
        }
    }

    /// Execute state transition with proper setup/cleanup
    /// Matches C++ GameEngine reset() and initialization patterns
    /// Wave 612: via `host_transition_to_state`.
    pub(super) fn transition_to_state(&mut self, new_state: GameState) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_transition_to_state(new_state)
    }

    /// Execute state transition with proper setup/cleanup
    /// Matches C++ GameEngine reset() and initialization patterns
    pub(super) fn host_transition_to_state(&mut self, new_state: GameState) {
        // Wave 612: host residual helper.
        let old_state = self.current_state;

        info!("State transition: {:?} -> {:?}", old_state, new_state);

        // Exit current state
        match old_state {
            GameState::Menu => {
                debug!("Exiting Menu state");
                self.hide_shell_menu();
            }
            GameState::Loading => {
                debug!("Exiting Loading state");
                self.hide_shell_loading_overlay();
            }
            GameState::InGame => {
                debug!("Exiting InGame state");
                // Could pause audio, save state, etc.
            }
            GameState::Paused => {
                debug!("Exiting Paused state");
            }
            GameState::Victory | GameState::Defeat => {
                debug!("Exiting end-of-match screen state");
            }
            GameState::Exiting => {
                debug!("Already exiting");
            }
            GameState::Initializing => {}
        }

        // Enter new state
        match new_state {
            GameState::Menu => {
                info!("Entering Menu state — transition_to_state start");
                // Latch Menu *before* show_shell_menu so status/control can observe
                // Menu even if MainMenu.wnd init / map-cache work is slow. Without
                // this, a hang inside MainMenuInit leaves state stuck at Loading and
                // windowed inject never runs.
                self.current_state = GameState::Menu;
                // C++ shell menus keep the shell map simulation alive behind the UI.
                self.host_set_paused(false);
                self.active_menu_shell_hook = None;
                info!("Menu transition: calling hide_gameplay_layouts");
                self.hide_gameplay_layouts();
                self.ui_manager.suspend_for_shell_overlay();
                self.set_runtime_ui_state_projection(UISystemState::MainMenu);
                info!("Menu transition: calling prime_subsystems_before_menu_transition");
                self.prime_subsystems_before_menu_transition();
                self.show_shell_menu();
                info!("Menu transition: prime_subsystems done. transition_to_state complete.");
                self.last_slow_menu_tick_log = None;
            }
            GameState::Loading => {
                info!("Entering Loading state");
                // C++ drives loading through LoadScreen subclasses created from
                // .wnd files. Keep the temporary Rust UI manager suspended so it
                // cannot paint a second custom loading screen above/beside that
                // window-manager-owned surface.
                self.ensure_shell_loading_overlay();
                self.update_shell_loading_progress(0.0, Some("Loading assets..."));
                self.ui_manager.suspend_for_shell_overlay();
                self.set_runtime_ui_state_projection(UISystemState::Loading);
                self.last_slow_menu_tick_log = None;
            }
            GameState::InGame => {
                info!("Entering InGame state");
                // Start game logic, enable input
                self.host_set_paused(false);
                // C++ hides the shell when a match begins. Leaving layouts visible can
                // re-init MainMenu on shell ticks and bounce status mid-match. Prefer
                // hide_shell over a pop-loop (pop can re-enter layout shutdown/init).
                #[cfg(feature = "game_client")]
                {
                    let _ = game_client::gui::try_with_shell_mut(|shell| {
                        // Do not run layout shutdown/pop here — that can re-enter
                        // MainMenuInit and stall the first InGame frames.
                        shell.hide(true);
                        shell.set_shell_active(false);
                    });
                }
                self.ensure_gameplay_layouts();
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::GameHUD);
                self.set_runtime_ui_state_projection(UISystemState::InGame);
                self.runtime_host_ui_screen_override = None;
            }
            GameState::Paused => {
                info!("Entering Paused state");
                // Freeze game logic, show pause menu
                self.host_set_paused(true);
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::PauseMenu);
                self.set_runtime_ui_state_projection(UISystemState::PauseMenu);
            }
            GameState::Exiting => {
                info!("Entering Exiting state - beginning shutdown");
            }
            GameState::Victory => {
                info!("Entering Victory state - match won");
                self.host_set_paused(true);
                if self.ui_manager.current_screen() != Some(crate::ui::Screen::Victory) {
                    self.ui_manager
                        .show_match_result(true, self.current_player_id);
                }
                self.set_runtime_ui_state_projection(UISystemState::Victory);
            }
            GameState::Defeat => {
                info!("Entering Defeat state - match lost");
                self.host_set_paused(true);
                if self.ui_manager.current_screen() != Some(crate::ui::Screen::Victory) {
                    self.ui_manager
                        .show_match_result(false, self.current_player_id);
                }
                self.set_runtime_ui_state_projection(UISystemState::Victory);
            }
            GameState::Initializing => {
                info!("Entering Initializing state");
            }
        }

        self.current_state = new_state;
        if matches!(new_state, GameState::Menu | GameState::Loading) {
            self.menu_loading_tick_accumulator = Duration::ZERO;
            self.menu_loading_last_tick = Instant::now();
        }
        if new_state == GameState::Menu {
            self.menu_enter_frame = Some(self.current_startup_logic_frame());
            self.last_shell_prewarm_log = None;
            self.shell_prewarm_completion_logged = false;
        } else {
            self.menu_enter_frame = None;
            self.shell_ui_enqueued_frame = None;
            self.active_menu_shell_hook = None;
            self.last_shell_prewarm_log = None;
            self.shell_prewarm_completion_logged = false;
        }
    }

    /// Check if engine should quit
    /// Matches C++ GameEngine::isQuitting()
    pub fn is_quitting(&self) -> bool {
        self.current_state == GameState::Exiting
    }

    pub(super) fn network_frame_data_ready_gate(multiplayer_session_active: bool) -> Option<bool> {
        if !multiplayer_session_active {
            // C++ startup leaves `TheNetwork` unset until a live multiplayer session exists.
            return None;
        }

        let has_network_backend = crate::network::has_active_network_interface();
        if !has_network_backend {
            return None;
        }

        let frame_ready = crate::network::active_session_frame_data_ready().unwrap_or(true);

        let Some(subsystem_manager) = get_subsystem_manager() else {
            return Some(frame_ready);
        };

        let mut manager = subsystem_manager.lock();
        if manager.get::<NetworkSubsystem>().is_none() {
            let mut network = NetworkSubsystem::new();
            if let Err(err) = <NetworkSubsystem as SubsystemInterface>::init(&mut network) {
                warn!(
                    "Failed to lazily initialize the network subsystem during multiplayer gating: {}",
                    err
                );
                return Some(false);
            }
            let _ = manager.add_subsystem(network);
        }

        if let Some(network) = manager.get_mut::<NetworkSubsystem>() {
            network.set_session_state(true, frame_ready);
            Some(network.is_frame_data_ready())
        } else {
            Some(frame_ready)
        }
    }

    pub(super) fn should_update_game_logic_frame(
        game_paused: bool,
        network_frame_data_ready: Option<bool>,
    ) -> bool {
        match network_frame_data_ready {
            Some(frame_ready) => frame_ready,
            None => !game_paused,
        }
    }

    pub(super) fn update_runtime_subsystems(&mut self, dt: f32) {
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let mut guard = subsystem_manager.lock();
            if let Some(timing) = self.last_frame_timing {
                if let Err(e) = guard.update_all_with_timing(&timing) {
                    error!("Error updating subsystems: {}", e);
                }
            } else if let Err(e) = guard.update_all(dt) {
                error!("Error updating subsystems: {}", e);
            }
        }
    }

    pub(super) fn prime_subsystems_before_menu_transition(&mut self) {
        let started = Instant::now();
        let mut max_step = Duration::ZERO;
        let mut steps = 0usize;

        // Keep warmup bounded. This should absorb one-time startup work, not become a new stall.
        while steps < 2 && started.elapsed() < Duration::from_millis(900) {
            let step_started = Instant::now();
            self.update_runtime_subsystems(0.0);
            let elapsed = step_started.elapsed();
            max_step = max_step.max(elapsed);
            steps += 1;

            if elapsed < Duration::from_millis(50) {
                break;
            }
        }

        info!(
            "Shell subsystem warmup before menu: steps={} elapsed={:?} max_step={:?}",
            steps,
            started.elapsed(),
            max_step
        );
    }

    pub(super) fn update_internal(&mut self, dt: f32) {
        // Process any pending state transitions first
        self.process_state_transitions();

        // Early exit if we're shutting down
        if self.is_quitting() {
            return;
        }

        let dt = dt.max(0.0);
        // Wave 251: prefer presentation visual speed residual when a frame is installed.
        let visual_speed = self.presentation_or_boot_visual_speed();
        let visual_dt = dt * visual_speed.max(0.0);

        // Diagnostic: log first few Menu update_internal calls
        if matches!(self.current_state, GameState::Menu) && self.menu_world_frames_rendered < 5 {
            debug!("update_internal: Menu state, about to call update_runtime_subsystems (menu_frame={})", self.menu_world_frames_rendered);
        }
        // C++ parity: radar/audio/client/message/network/cd updates happen each frame.
        self.update_runtime_subsystems(dt);
        // A Control Bar click belongs to the world visible when the click was
        // made.  Drain it before a same-frame PopupSaveLoad request can swap
        // the authoritative world underneath it.
        #[cfg(feature = "game_client")]
        self.host_tick_control_bar_bridge();
        // PopupSaveLoad owns the retail WND sequence, but Main owns the only
        // Rust snapshot world.  Drain requests after input/UI callbacks have
        // unwound, before the frame's state-specific work observes a load.
        #[cfg(feature = "game_client")]
        self.host_tick_popup_save_load_bridge();
        if matches!(self.current_state, GameState::Menu) && self.menu_world_frames_rendered < 5 {
            debug!(
                "update_internal: Menu state, update_runtime_subsystems done, entering state match"
            );
        }
        // State-based update logic - matches C++ GameEngine::update() conditional updates
        match self.current_state {
            GameState::Menu => {
                // First-run C++ hides the dropdown until CHAR/mouse. If the
                // player is staring at terrain with no chrome, reveal after
                // ~1.5s so Single Player / Skirmish become visible.
                #[cfg(feature = "game_client")]
                if self.menu_world_frames_rendered == 45 {
                    let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
                }
                // Wave 605: Menu client residual via host helper.
                if self.host_tick_menu_client_residuals(visual_dt, dt) {
                    return;
                }
                return;
            }
            GameState::Loading => {
                // Wave 604: loading client residual via host helper.
                if let Err(err) = self.host_tick_loading_client_residuals() {
                    error!("Startup loading failed: {}", err);
                    self.request_state_change(GameState::Exiting);
                }
                return;
            }
            GameState::Paused => {
                // Wave 603: paused client residual via host helper.
                self.host_tick_paused_client_residuals(visual_dt, dt);
                return;
            }
            GameState::InGame => {
                // Full update - continue below
            }
            GameState::Exiting => {
                // Exiting: no updates needed
                return;
            }
            GameState::Victory | GameState::Defeat => {
                // Wave 603: endgame client residual via host helper.
                self.host_tick_endgame_client_residuals(visual_dt, dt);
                return;
            }
            GameState::Initializing => {
                return;
            }
        }

        // Full update cycle for InGame state (matches C++ GameEngine::update())

        // C++ parity gate:
        //   (Network == NULL && !isGamePaused()) || (Network && Network->isFrameDataReady()).
        let network_frame_data_ready =
            Self::network_frame_data_ready_gate(self.host_is_in_multiplayer_game());
        if Self::should_update_game_logic_frame(self.game_paused, network_frame_data_ready) {
            // Wave 602: host InGame logic+shadow+presentation residual via helper.
            self.host_run_ingame_logic_presentation_frame(dt);
        }

        // Wave 598: InGame HUD presentation residual via host helper.
        self.host_apply_ingame_hud_from_presentation(dt);
        // Wave 600: post-HUD camera/audio/UI/popup-music residual via host helper.
        self.host_tick_post_presentation_client_residuals(visual_dt, dt);
        // Wave 599: defeat/alliance/victory broadcast residual via host helper.
        self.host_broadcast_match_outcome_residuals();
    }

    /// Wave 542: mouse command classification is presentation-only when freeze installed.
    /// Wave 609: via `host_presentation_mouse_game_logic`.
    pub(super) fn presentation_mouse_game_logic(&self) -> Option<&crate::game_logic::GameLogic> {
        // Wave 609: thin wrapper — UI/presentation residual via host helper.
        // Wave 236: None when last_presentation_frame.is_some(); boot Some(&self.game_logic).
        if self.last_presentation_frame.is_some() {
            self.host_presentation_mouse_game_logic()
        } else {
            crate::game_logic::mouse_game_logic_for_process_mouse_input(false, &self.game_logic)
                .or(Some(&self.game_logic))
        }
    }

    /// Wave 542: mouse command classification is presentation-only when freeze installed.
    pub(super) fn host_presentation_mouse_game_logic(
        &self,
    ) -> Option<&crate::game_logic::GameLogic> {
        // Wave 609/841/906: always presentation-only mouse classification.
        // No live GameLogic dual-read for cursor/command classification.
        let _ = self;
        None
    }

    /// Last immutable presentation snapshot after the most recent logic step.
    pub fn last_presentation_frame(&self) -> Option<&crate::presentation_frame::PresentationFrame> {
        self.last_presentation_frame.as_ref()
    }

    /// Last presentation-overlaid UI state (selection health / minimap identity).
    pub fn last_ui_state(&self) -> Option<&GameUIState> {
        self.last_ui_state.as_ref()
    }

    /// Production HUD consumer: apply last presentation to GameHUD without re-reading live objects.
    pub fn apply_presentation_to_hud(&mut self) -> bool {
        let Some(pres) = self.last_presentation_frame.clone() else {
            return false;
        };
        self.apply_presentation_to_huds(&pres);
        self.sync_eva_messages_from_logic();
        #[cfg(feature = "game_client")]
        {
            pres.apply_to_control_bar(&mut self.control_bar);
            let _ = self
                .control_bar
                .update(std::time::Duration::from_millis(33));
        }
        true
    }

    /// ControlBar selection panel health from last presentation (headless-safe).
    #[cfg(feature = "game_client")]
    pub fn control_bar_selection_health(&self) -> Option<(f32, f32)> {
        self.control_bar.selection_panel_health()
    }

    /// After map load / load-game / skirmish StartGame: seed PresentationFrame + HUD so
    /// the first InGame frame has units/minimap/selection identity without waiting for
    /// the next dual-tick. Does not advance logic frames.
    /// Wave 590: via `host_seed_presentation_after_match_start`.
    pub(super) fn seed_presentation_after_match_start(&mut self) {
        self.host_seed_presentation_after_match_start();
    }

    pub fn render(&mut self) -> Result<()> {
        let render_started = Instant::now();
        #[cfg(feature = "game_client")]
        {
            let size = self.window.inner_size();
            if size.width > 0 && size.height > 0 {
                let _ = game_client::gui::ui_globals::with_ui_renderer_mut(|renderer| {
                    renderer.set_screen_size(size.width, size.height);
                });
            }
        }
        static RENDER_CALL_COUNT: std::sync::atomic::AtomicU32 =
            std::sync::atomic::AtomicU32::new(0);
        let render_call = RENDER_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if render_call < 30 || render_call.is_multiple_of(300) {
            info!(
                "render() called #{}, state={:?}",
                render_call, self.current_state
            );
        }

        if !matches!(self.current_state, GameState::Loading | GameState::Menu) {
            // Production presentation consumer: when a post-logic snapshot exists,
            // GameUIState is built from PresentationFrame only (no live object walks).
            // Wave 591: render UI presentation consumer via host helper.
            let mut ui_state = self.host_build_render_ui_state_from_presentation();
            // Wave 592: radar/script/clock/diag overlays via host helper.
            self.host_apply_render_ui_presentation_overlays(&mut ui_state);
            self.process_ui_events();
            // Wave 593: minimap/radar/victory UI residual via host helper.
            self.host_finalize_render_ui_state(&mut ui_state);
        }

        // Execute the main game render pipeline using the WW3D frame.
        // Wave 250: prefer presentation freeze residual when a frame is installed.
        let time_frozen = self.presentation_or_boot_time_frozen();
        // Wave 251: prefer presentation visual speed residual when a frame is installed.
        let visual_speed = self.presentation_or_boot_visual_speed();
        let render_time_delta = if time_frozen {
            0.0
        } else {
            self.last_frame_timing
                .map(|t| t.delta_seconds())
                .unwrap_or(0.0)
                * visual_speed.max(0.0)
        };
        let startup_frame = self.shell_start_frame();
        let current_startup_logic_frame = self.current_startup_logic_frame();
        let deferred_startup_model_load_budget = Self::startup_deferred_model_load_budget(
            self.current_state,
            startup_frame,
            current_startup_logic_frame,
        );
        let allow_sync_model_loads = deferred_startup_model_load_budget == 0;
        let skip_world_scene = self.should_skip_world_scene_for_shell_menu();

        #[cfg(feature = "game_client")]
        {
            if !skip_world_scene && matches!(self.current_state, GameState::Menu) {
                let prev = self.menu_world_frames_rendered;
                self.menu_world_frames_rendered += 1;
                if prev < 3 {
                    info!(
                        "Menu world render frame {}/3 (skip=false for first time)",
                        self.menu_world_frames_rendered
                    );
                }
            }
        }

        #[cfg(feature = "game_client")]
        {
            // C++ has a single render path: W3DDisplay::draw() which does 3D scene + UI + mouse.
            // We use render_pipeline.execute() below as that unified path.
            // game_client.draw_display() was a second competing render path that acquired
            // its own surface frame and called present(), stomping the render_pipeline output.
        }

        let render_pipeline_started = Instant::now();
        if render_call < 30 || (render_call < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "render() #{}, calling render_pipeline.execute skip_world={} state={:?}",
                render_call, skip_world_scene, self.current_state
            );
        }
        // Keep match camera orbit→view_matrix coherent every draw. Shell→InGame
        // residual: update_camera may skip apply when no input, leaving a stale
        // view_matrix from the shell map.
        if matches!(
            self.current_state,
            GameState::InGame | GameState::Paused | GameState::Menu
        ) {
            self.apply_camera_orbit_transform();
        }

        // Full presentation snapshot for render collect (transforms/model/selection/health).
        // Boot/Menu residual: if no frame yet, freeze one from host logic so execute
        // never dual-reads live GameLogic (immutable presentation boundary).
        // Wave 590: boot/render presentation seed via helper.
        self.host_ensure_presentation_frame_for_render();
        // First InGame frames: boot camera still stares at the origin while
        // Lone Eagle / ZH maps live hundreds of world units away.
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            let at_origin = {
                let t = self.camera_target;
                t.x * t.x + t.z * t.z < 80.0 * 80.0
            };
            if at_origin {
                self.snap_camera_to_local_units_if_needed();
            }
        }
        self.render_pipeline
            .set_presentation_frame(self.last_presentation_frame.clone());
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.render_pipeline
                .set_skybox_enabled(pres.world_env.skybox_enabled);
            if let Some(textures) = pres.world_env.skybox_textures.clone() {
                self.render_pipeline.set_skybox_hint(textures);
            }
        }

        // Production selection overlay: prefer PresentationFrame identity when available
        // (C++ W3DInGameUI selection circles / drag region after 3D scene setup).
        if !skip_world_scene && matches!(self.current_state, GameState::InGame | GameState::Paused)
        {
            let drag_rect = if self.is_dragging {
                self.selection_start_screen.map(|start| {
                    let size = self.window.inner_size();
                    crate::graphics::selection_renderer::DragSelectRect {
                        start: glam::Vec2::new(start.0, start.1),
                        end: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
                        window_width: size.width as f32,
                        window_height: size.height as f32,
                    }
                })
            } else {
                None
            };
            let ground_markers = self.collect_ground_marker_circles();
            crate::graphics::selection_renderer::enqueue_selection_render(
                &mut self.render_pipeline,
                &self.view_matrix,
                &self.projection_matrix,
                drag_rect.filter(|r| r.is_valid()),
                self.last_presentation_frame.as_ref(),
                ground_markers,
                self.show_move_lines,
                self.show_attack_lines,
            );
        }
        self.render_pipeline.execute(
            &mut self.graphics_system,
            &self.view_matrix,
            &self.projection_matrix,
            self.camera_position,
            render_time_delta,
            allow_sync_model_loads,
            deferred_startup_model_load_budget,
            skip_world_scene,
        )?;
        let render_pipeline_elapsed = render_pipeline_started.elapsed();

        let attachments_started = Instant::now();
        self.drain_renderer_attachments();
        let attachments_elapsed = attachments_started.elapsed();
        let total_elapsed = render_started.elapsed();
        if total_elapsed >= Duration::from_millis(200) {
            warn!(
                "Render breakdown: total={:?} pipeline={:?} attachments={:?} state={:?} render_items={} model_missing={} deferred_loads={}/{} startup_progress={:.0}%",
                total_elapsed,
                render_pipeline_elapsed,
                attachments_elapsed,
                self.current_state,
                self.render_pipeline.debug_render_item_count(),
                self.render_pipeline.debug_last_model_missing(),
                self.render_pipeline.debug_last_deferred_model_loads(),
                self.render_pipeline.debug_last_deferred_model_load_budget(),
                self.startup_last_reported_progress * 100.0
            );
        }
        if render_call < 30 || (render_call < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "render() #{} done, pipeline={:?} total={:?} state={:?}",
                render_call,
                render_pipeline_elapsed,
                render_started.elapsed(),
                self.current_state
            );
        }
        Ok(())
    }

    pub(super) fn update_minimap_viewport(&self, ui_state: &mut GameUIState) {
        // Prefer presentation world_env when installed (camera-relative minimap viewport).
        // Boot residual without a frame still uses host GameLogic bounds.
        let (world_min, world_max) = self.presentation_world_bounds();
        let world_extent_x = (world_max.x - world_min.x).max(1.0);
        let world_extent_z = (world_max.z - world_min.z).max(1.0);

        let half_width = 200.0 / self.camera_zoom.max(0.01);
        let half_height = 150.0 / self.camera_zoom.max(0.01);

        let min_x = ((self.camera_target.x - half_width) - world_min.x) / world_extent_x;
        let max_x = ((self.camera_target.x + half_width) - world_min.x) / world_extent_x;
        let min_y = ((self.camera_target.z - half_height) - world_min.z) / world_extent_z;
        let max_y = ((self.camera_target.z + half_height) - world_min.z) / world_extent_z;

        ui_state.minimap_viewport = crate::ui::normalized_minimap_rect(min_x, min_y, max_x, max_y);
    }

    /// Process UI events emitted by UIManager and apply to engine/game state.
    pub(super) fn process_ui_events(&mut self) {
        while let Some(event) = self.ui_manager.pop_event() {
            match event {
                UIEvent::StartGame {
                    mode,
                    faction,
                    map,
                    skirmish,
                } => {
                    self.start_game_from_ui(mode, faction, map, skirmish);
                }
                UIEvent::LoadGame(slot) => {
                    if slot == "quicksave" {
                        self.quick_load_from_hotkey("UI quick-load");
                    } else {
                        let _ = self.load_game_from_ui(&slot);
                    }
                }
                UIEvent::SaveGame { slot, display_name } => {
                    if slot == "quicksave" {
                        self.quick_save_from_hotkey("UI quick-save");
                    } else {
                        self.save_game_from_ui(&slot, &display_name);
                    }
                }
                UIEvent::RestartMission => {
                    self.restart_mission_from_ui();
                }
                UIEvent::PlaySoundEffectPath(path) => {
                    self.play_ui_sound_effect(path);
                }
                UIEvent::TogglePause => {
                    self.toggle_pause();
                }
                UIEvent::ExitToMenu => {
                    info!("UI requested exit to menu");
                    self.return_to_main_menu_after_match();
                }
                UIEvent::ExitGame => {
                    info!("UI requested exit");
                    self.request_state_change(GameState::Exiting);
                }
                UIEvent::ChangeScreen(screen) => {
                    if screen == Screen::Loading
                        && matches!(self.current_state, GameState::Menu | GameState::Loading)
                    {
                        self.ensure_shell_loading_overlay();
                        self.update_shell_loading_progress(0.0, Some("Loading assets..."));
                        self.ui_manager.suspend_for_shell_overlay();
                        self.set_runtime_ui_state_projection(UISystemState::Loading);
                        continue;
                    }

                    if self.current_state == GameState::Menu && screen.is_shell_owned_pregame() {
                        self.route_shell_owned_screen_change(screen);
                        continue;
                    }

                    self.ui_manager.transition_to_screen(screen);
                    match screen {
                        Screen::MainMenu => {
                            self.set_runtime_ui_state_projection(UISystemState::MainMenu)
                        }
                        Screen::Loading => {
                            self.set_runtime_ui_state_projection(UISystemState::Loading)
                        }
                        Screen::GameHUD => {
                            self.set_runtime_ui_state_projection(UISystemState::InGame)
                        }
                        Screen::PauseMenu => {
                            self.set_runtime_ui_state_projection(UISystemState::PauseMenu)
                        }
                        Screen::Victory => {
                            self.set_runtime_ui_state_projection(UISystemState::Victory)
                        }
                        _ => {}
                    }
                }
                UIEvent::FocusCamera(world_pos) => {
                    self.center_camera_on(world_pos);
                }
                UIEvent::QueueUnitProduction {
                    template_name,
                    quantity,
                } => {
                    // C++ ControlBar: Shift queues five; Ctrl queues to factory max residual.
                    let mut qty = quantity.max(1);
                    let shift = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                    let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
                    if ctrl {
                        qty = 9; // DEFAULT_PRODUCTION_QUEUE_LIMIT residual fill
                    } else if shift {
                        qty = qty.saturating_mul(5).max(5);
                    }
                    self.queue_unit_production_from_ui(&template_name, qty);
                }
                UIEvent::CancelUnitProduction { template_name } => {
                    self.cancel_unit_production_from_ui(&template_name);
                }
                UIEvent::IssueCommand { command_name } => {
                    self.issue_named_command_from_ui(&command_name);
                }
                UIEvent::BeginStructurePlacement { template_name } => {
                    self.begin_structure_placement_from_ui(&template_name);
                }
                UIEvent::PlaceStructureAt {
                    template_name,
                    location,
                } => {
                    self.place_structure_from_ui(&template_name, location);
                }
                UIEvent::CancelStructurePlacement => {
                    self.cancel_structure_placement_from_ui();
                }
                UIEvent::ShowOptions => {
                    // Retail OPTIONS residual from pause menu / shell.
                    if matches!(self.current_state, GameState::InGame) {
                        self.request_state_change(GameState::Paused);
                    }
                    self.ui_manager
                        .transition_to_screen(crate::ui::Screen::Options);
                    info!("UI requested options menu residual");
                }
                UIEvent::SettingsChanged => {
                    if let Some(v) = self.ui_manager.options_bool("game.show_health_bars") {
                        self.show_health_bars = v;
                        self.game_hud.set_show_selection_health(v);
                        self.ui_manager.game_hud_mut().set_show_selection_health(v);
                        info!("Settings: show_health_bars={v}");
                    }
                    if let Some(v) = self.ui_manager.options_bool("game.show_fps") {
                        self.show_fps = v;
                        info!("Settings: show_fps={v}");
                    }
                }
            }
        }
    }

    /// C++ ControlBar production cameo → QueueUnitCreate residual.
    /// Keep interactive UIManager HUD and engine GameHUD presentation-synced residual.
    ///
    /// Clicks route through `ui_manager.game_hud`; resources/selection presentation
    /// historically only updated `self.game_hud`. Dual-apply closes that gap.

    /// C++ TheEva residual → chat EVA lines when honesty counters advance.
    pub(super) fn sync_eva_messages_from_logic(&mut self) {
        // Prefer presentation-frozen EVA counters when a frame is installed
        // (no live GameLogic dual-read mid-HUD apply).
        if let Some(pres) = self.last_presentation_frame.clone() {
            self.sync_eva_messages_from_presentation(&pres);
            return;
        }
        // Wave 583: boot residual EVA counters via helper bundle.
        let (lp, funds, base, ally) = self.boot_eva_counter_bundle_from_host();
        self.sync_eva_messages_from_host_counts(lp, funds, base, ally);
    }

    pub(super) fn sync_eva_messages_from_presentation(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        // Wave 536/537: EvaAlert first (full matrix). Classic counter path runs after and
        // only fills gaps when no matching alert advanced last_* this frame (no double chat).
        self.apply_presentation_eva_alerts(pres);
        self.sync_eva_messages_from_host_counts(
            pres.eva_low_power_count,
            pres.eva_insufficient_funds_count,
            pres.eva_base_under_attack_count,
            pres.eva_ally_under_attack_count,
        );
    }

    /// Wave 536/537: map presentation `EvaAlert` names onto chat/HUD and GameClient Eva.
    ///
    /// Names arrive as `EVA_` + C++ `TheEvaMessageNames` token from `host_eva_log`.
    /// Client `EvaMessage::from_name` expects table tokens (`LOWPOWER`, …).
    /// Wave 538/539: presentation-only radar ping + GUIMessageReceived SFX.
    /// Fail-closed: does not dual-write GameLogic mid-frame.
    /// Wave 606: via `host_notify_presentation_ui_message`.
    pub(super) fn notify_presentation_ui_message(&mut self, message: &str) {
        // Wave 606: thin wrapper — presentation UI notify via host helper.
        self.host_notify_presentation_ui_message(message);
    }

    /// Wave 606: host presentation UI message residual.
    ///
    /// Wave 536/537/538/539: presentation-only radar ping + GUIMessageReceived SFX.
    /// Fail-closed: does not dual-write GameLogic mid-frame.
    pub(super) fn host_notify_presentation_ui_message(&mut self, message: &str) {
        // Wave 606: host presentation UI notify residual.
        self.game_hud
            .add_radar_message(message, None, crate::ui::RadarPingKind::Generic);
        self.ui_manager.game_hud_mut().add_radar_message(
            message,
            None,
            crate::ui::RadarPingKind::Generic,
        );
        let req =
            crate::game_logic::AudioEventRequest::new("GUIMessageReceived").with_priority(150);
        let _ = crate::subsystem_manager::with_subsystem_mut::<
            crate::subsystem_manager::AudioManagerSubsystem,
            _,
        >(|audio| audio.queue_event(req));
    }

    /// Wave 566/603: boot residual radar + GUIMessageReceived via host GameLogic.
    /// Presentation freeze must use `notify_presentation_ui_message` instead
    /// (no mid-frame GameLogic dual-write).
    pub(super) fn notify_boot_ui_message(
        &mut self,
        message: &str,
        team: Option<crate::game_logic::Team>,
    ) {
        // Wave 603: thin wrapper.
        self.host_notify_boot_ui_message(message, team);
    }

    /// Wave 603: host boot UI message residual (radar queue + GUIMessageReceived).
    pub(super) fn host_notify_boot_ui_message(
        &mut self,
        message: &str,
        team: Option<crate::game_logic::Team>,
    ) {
        // Wave 603/869/900: always route through presentation UI residual
        // (HUD radar + audio subsystem) — no GameLogic dual-write.
        let _ = team;
        self.host_notify_presentation_ui_message(message);
    }

    /// Classic-four tokens also advance `last_eva_*` so counter residual does not re-push.
    pub(super) fn apply_presentation_eva_alerts(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        use crate::presentation_frame::PresentationEvent;
        for ev in &pres.events {
            let PresentationEvent::EvaAlert { name } = ev else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            let token = Self::eva_alert_client_token(name);
            // Wave 537: absorb classic counter deltas covered by this alert pulse.
            match token.as_str() {
                "LOWPOWER" => {
                    self.last_eva_low_power_count =
                        self.last_eva_low_power_count.max(pres.eva_low_power_count);
                }
                "INSUFFICIENTFUNDS" => {
                    self.last_eva_insufficient_funds_count = self
                        .last_eva_insufficient_funds_count
                        .max(pres.eva_insufficient_funds_count);
                }
                "BASEUNDERATTACK" => {
                    self.last_eva_base_under_attack_count = self
                        .last_eva_base_under_attack_count
                        .max(pres.eva_base_under_attack_count);
                }
                "ALLYUNDERATTACK" => {
                    self.last_eva_ally_under_attack_count = self
                        .last_eva_ally_under_attack_count
                        .max(pres.eva_ally_under_attack_count);
                }
                _ => {}
            }

            let human = Self::eva_alert_human_message(name);
            self.chat_panel.add_eva_message(&human);
            self.game_hud.push_info_message(&human);
            self.ui_manager.game_hud_mut().push_info_message(&human);

            #[cfg(feature = "game_client")]
            {
                let _ = game_client::eva::simulate_eva_set_should_play_by_name(&token);
            }
        }
    }

    /// `EVA_LOWPOWER` / table token → chat line residual.
    pub(super) fn eva_alert_human_message(name: &str) -> String {
        let key = name
            .strip_prefix("EVA_")
            .unwrap_or(name)
            .trim()
            .to_ascii_uppercase();
        match key.as_str() {
            "LOWPOWER" => "Warning: low power".into(),
            "INSUFFICIENTFUNDS" => "Insufficient funds".into(),
            "BASEUNDERATTACK" => "Our base is under attack".into(),
            "ALLYUNDERATTACK" => "Ally under attack".into(),
            "BUILDINGLOST" => "Building lost".into(),
            "UNITLOST" => "Unit lost".into(),
            "BEACONDETECTED" => "Beacon detected".into(),
            "UPGRADECOMPLETE" => "Upgrade complete".into(),
            "GENERALLEVELUP" => "General level up".into(),
            "VEHICLESTOLEN" => "Vehicle stolen".into(),
            "BUILDINGSTOLEN" => "Building stolen".into(),
            "BUILDINGBEINGSTOLEN" => "Building being stolen".into(),
            "BUILDINGSABOTAGED" => "Building sabotaged".into(),
            "CASHSTOLEN" => "Cash stolen".into(),
            other => format!("EVA: {}", other.replace('_', " ").to_ascii_lowercase()),
        }
    }

    /// `EVA_LOWPOWER` → `LOWPOWER` for `EvaMessage::from_name` (C++ table token).
    pub(super) fn eva_alert_client_token(name: &str) -> String {
        name.strip_prefix("EVA_")
            .unwrap_or(name)
            .trim()
            .to_ascii_uppercase()
    }

    pub(super) fn sync_eva_messages_from_host_counts(
        &mut self,
        low_power: u32,
        funds: u32,
        base: u32,
        ally: u32,
    ) {
        let mut push = |msg: &str| {
            self.chat_panel.add_eva_message(msg);
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
        };
        if low_power > self.last_eva_low_power_count {
            self.last_eva_low_power_count = low_power;
            push("Warning: low power");
        }
        if funds > self.last_eva_insufficient_funds_count {
            self.last_eva_insufficient_funds_count = funds;
            push("Insufficient funds");
        }
        if base > self.last_eva_base_under_attack_count {
            self.last_eva_base_under_attack_count = base;
            push("Our base is under attack");
        }
        if ally > self.last_eva_ally_under_attack_count {
            self.last_eva_ally_under_attack_count = ally;
            push("Ally under attack");
        }
    }

    /// Retired dual-path residual: presentation events map to AudioManager via
    /// `PresentationFrame::dispatch_audio_events_direct`. Kept for call-site
    /// source tests; no-op so engine SFX does not double-play.
    /// Retired dual-path residual: presentation events map to AudioManager via
    /// `PresentationFrame::dispatch_audio_events_direct`. No-op so engine SFX
    /// does not double-play construction/production/upgrade cues.
    pub(super) fn play_presentation_event_sfx(
        &mut self,
        _frame: &crate::presentation_frame::PresentationFrame,
    ) {
        let _ = self;
    }

    pub(super) fn apply_presentation_to_huds(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        // Dual GameHUD residual: engine HUD + interactive UIManager HUD.
        pres.apply_to_game_hud(&mut self.game_hud);
        pres.apply_to_game_hud(self.ui_manager.game_hud_mut());
    }
}
